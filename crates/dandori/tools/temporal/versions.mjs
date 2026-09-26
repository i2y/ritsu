// Worker Deployment Versioning on a real Temporal server (the dev server): two builds of one
// workflow, A and B, whose workers are versions of one deployment through the generated
// worker.ts. A run that starts on A and waits while B becomes the current version must end on
// A's code, the rounds it runs in a new run after a Continue-As-New too; a run that starts after
// must run on B's. The workflow is tests/versions/approvals.flow: each round waits for an
// approval (a callback) and then notifies, with a text that differs between A's code and B's.
// Its second round starts in a new run, since the copies here count every history as long.
//
//   node tools/temporal/versions.mjs <generated dir A> <generated dir B> <results.json>
//
// results.json: { "buildIds": [A's, B's], "notified": { "<run>": [the texts it notified] },
//                 "runs": { "<run>": how many runs it took } }

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

/** A copy of a build that Node can load, with its modules' extensions written out, and every history long enough to go on in a new run. */
function copy(dir) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-versions-"));
  for (const f of fs.readdirSync(dir)) {
    if (!f.endsWith(".ts")) continue;
    let text = fs.readFileSync(path.join(dir, f), "utf8").replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`);
    if (f === "runtime.ts") {
      if (!text.includes("export const CONTINUE_AT = 10000;")) throw new Error("runtime.ts has no CONTINUE_AT to lower");
      text = text.replace("export const CONTINUE_AT = 10000;", "export const CONTINUE_AT = 1;");
    }
    fs.writeFileSync(path.join(work, f), text);
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

function add(map, id, x) {
  map.set(id, [...(map.get(id) ?? []), x]);
}

/** The tasks of tests/versions/approvals.flow. */
const own = {
  async 承認を求める(args) {
    add(callbacks, Context.current().info.workflowExecution.workflowId, args.callback_id);
  },
  async 通知する(args) {
    add(notified, Context.current().info.workflowExecution.workflowId, args["本文"]);
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

  /** Make the build's worker, a version of the deployment. */
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

  /** How many runs a workflow took: the first, and each that went on from the one before (Continue-As-New). */
  async function runsOf(id, runId) {
    let n = 0;
    while (runId) {
      n++;
      const history = await env.client.workflow.getHandle(id, runId).fetchHistory();
      runId = history.events.at(-1)?.workflowExecutionContinuedAsNewEventAttributes?.newExecutionRunId;
    }
    return n;
  }

  const input = { 申込: { id: "申込-1" } };
  const workerA = await version(a);
  const workerB = await version(b);
  await workerA.runUntil(async () => {
    await current(a);
    const first = await a.client.start(env.client, "run-a", input);
    await until("the first run waits for its approval", async () => callbacks.get("run-a")?.length === 1);
    await workerB.runUntil(async () => {
      await current(b);
      const second = await b.client.start(env.client, "run-b", input);
      await until("the second run waits for its approval", async () => callbacks.get("run-b")?.length === 1);
      // the first run, pinned to A, goes on with A's code, in its next run too; the second, started on B, with B's
      await a.client.answer(env.client, callbacks.get("run-a")[0], { ok: { 承認者: "a" } });
      await b.client.answer(env.client, callbacks.get("run-b")[0], { ok: { 承認者: "b" } });
      await until("both runs wait for their second approval", async () => callbacks.get("run-a")?.length === 2 && callbacks.get("run-b")?.length === 2);
      await a.client.answer(env.client, callbacks.get("run-a")[1], { ok: { 承認者: "a" } });
      await b.client.answer(env.client, callbacks.get("run-b")[1], { ok: { 承認者: "b" } });
      await Promise.all([first.result(), second.result()]);
      out.runs = { "run-a": await runsOf("run-a", first.firstExecutionRunId), "run-b": await runsOf("run-b", second.firstExecutionRunId) };
    });
  });
  out.notified = Object.fromEntries(notified);
} finally {
  await env.teardown();
  for (const w of works) fs.rmSync(w, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(out, null, 2) + "\n");
