// Worker Deployment Versioning on a real Temporal server (the dev server): two builds of one
// workflow, A and B, whose workers are versions of one deployment through the generated
// worker.ts. A run that starts on A and waits while B becomes the current version must end on
// A's code; a run that starts after must run on B's. The workflow is examples/review: its
// scoring answers "保留", so that each run waits for an approval (a callback), and then it
// notifies, with a text that differs between A's code and B's.
//
//   node tools/temporal/versions.mjs <generated dir A> <generated dir B> <results.json>
//
// results.json: { "buildIds": [A's, B's], "notified": { "<run>": [the texts it notified] } }

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { TestWorkflowEnvironment } from "@temporalio/testing";
import { DefaultLogger, Runtime, Worker } from "@temporalio/worker";
import { Context } from "@temporalio/activity";

Runtime.install({ logger: new DefaultLogger("WARN", (entry) => process.stderr.write(`${entry.level} ${entry.message}\n`)) });

const here = path.dirname(fileURLToPath(import.meta.url));
const [dirA, dirB, outFile] = process.argv.slice(2);
const DEPLOYMENT = "dandori-versions";

/** A copy of a build that Node can load, with its modules' extensions written out. */
function copy(dir) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-versions-"));
  for (const f of fs.readdirSync(dir)) {
    if (!f.endsWith(".ts")) continue;
    fs.writeFileSync(path.join(work, f), fs.readFileSync(path.join(dir, f), "utf8").replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`));
  }
  fs.symlinkSync(path.join(here, "node_modules"), path.join(work, "node_modules"));
  return work;
}

const bundlerOptions = {
  webpackConfigHook: (config) => {
    config.resolve = config.resolve ?? {};
    config.resolve.modules = [...(config.resolve.modules ?? ["node_modules"]), path.join(here, "node_modules")];
    config.resolve.extensionAlias = { ".js": [".ts", ".js"] };
    return config;
  },
};

// what each run did: the ids of its callbacks, and the texts it notified
const callbacks = new Map();
const notified = new Map();

/** The tasks of examples/review, as the scenario wants them: scoring holds each application. */
const own = {
  async 審査する() {
    return { 点数: 50, 判断: "保留" };
  },
  async 承認を求める(args) {
    callbacks.set(Context.current().info.workflowExecution.workflowId, args.callback_id);
  },
  async 通知する(args) {
    const id = Context.current().info.workflowExecution.workflowId;
    notified.set(id, [...(notified.get(id) ?? []), args["本文"]]);
  },
};

async function until(what, ok) {
  for (let i = 0; i < 300; i++) {
    const got = await ok();
    if (got) return got;
    await new Promise((done) => setTimeout(done, 100));
  }
  throw new Error(`gave up waiting until ${what}`);
}

const works = [copy(dirA), copy(dirB)];
const env = await TestWorkflowEnvironment.createLocal({ server: { ui: false, log: { format: "pretty", level: "error" }, extraArgs: ["--dynamic-config-value", 'history.timerProcessorMaxTimeShift="10ms"'] } });
const out = {};
try {
  const [a, b] = await Promise.all(works.map(async (w) => ({ worker: await import(path.join(w, "worker.ts")), client: await import(path.join(w, "client.ts")), work: w })));
  out.buildIds = [a.worker.BUILD_ID, b.worker.BUILD_ID];

  /** Make the build's worker, a version of the deployment, and let it run until `done` is. */
  async function version(build) {
    return Worker.create({
      ...build.worker.workerOptions(own, { deployment: DEPLOYMENT, workflowsPath: path.join(fs.realpathSync(build.work), "workflow.ts") }),
      connection: env.nativeConnection,
      bundlerOptions,
      // no workflow kept in the worker's cache: a run goes where the server sends it, not where it was
      maxCachedWorkflows: 0,
    });
  }

  /** Make the build the current version of the deployment, once the server has seen its worker. */
  async function current(build) {
    await until(`${build.worker.BUILD_ID} can be the current version`, async () => {
      try {
        await env.client.workflowService.setWorkerDeploymentCurrentVersion({ namespace: "default", deploymentName: DEPLOYMENT, buildId: build.worker.BUILD_ID });
        return true;
      } catch {
        return false; // the server has not seen the worker's pollers yet
      }
    });
  }

  const input = { 申込: { id: "申込-1", 金額: 1000 } };
  const workerA = await version(a);
  const workerB = await version(b);
  await workerA.runUntil(async () => {
    await current(a);
    const first = await a.client.start(env.client, "run-a", input);
    await until("the first run waits for its approval", async () => callbacks.get("run-a"));
    await workerB.runUntil(async () => {
      await current(b);
      const second = await b.client.start(env.client, "run-b", input);
      await until("the second run waits for its approval", async () => callbacks.get("run-b"));
      // the first run, pinned to A, goes on with A's code; the second, started on B, with B's
      await a.client.answer(env.client, callbacks.get("run-a"), { ok: { 承認者: "a" } });
      await b.client.answer(env.client, callbacks.get("run-b"), { ok: { 承認者: "b" } });
      await Promise.all([first.result(), second.result()]);
    });
  });
  out.notified = Object.fromEntries(notified);
} finally {
  await env.teardown();
  for (const w of works) fs.rmSync(w, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(out, null, 2) + "\n");
