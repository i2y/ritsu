// A workflow and the .flow it runs as its child (`flow "<path>"`), both as dandori writes them, on
// one Temporal server (the dev server): the parent's worker and the child's, each on its own task
// queue, made by their generated worker.ts. Each run starts the parent with its input through the
// generated client.ts, and its end is written as the reference interpreter writes one. Nothing
// stands in for the child: the parent's call starts the child's workflow, and what the child ends
// with is what the parent gets. The child's worker can be the other language's: with
// DANDORI_CHILD_BY (a JSON command, to which the server's address is added), that runner serves
// the child until its input closes. The Python twin is ../temporal-python/children.py.
//
//   node tools/temporal/children.mjs <parent's generated dir> <child's generated dir> <runs.json> <results.json>
//   node tools/temporal/children.mjs --serve <child's generated dir> <address>
//
// runs.json: [ { "id", "input" } ]
// results.json: [ { "end": { "succeed": the outputs } | { "fail": { "error", "cause" } } } ]

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { WorkflowFailedError } from "@temporalio/client";
import { TestWorkflowEnvironment } from "@temporalio/testing";
import { DefaultLogger, NativeConnection, Runtime, Worker } from "@temporalio/worker";

Runtime.install({ logger: new DefaultLogger("WARN", (entry) => process.stderr.write(`${entry.level} ${entry.message}\n`)) });

const here = path.dirname(fileURLToPath(import.meta.url));

/** A copy of a build that Node can load, with its modules' extensions written out. */
function copy(dir) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-children-"));
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

/** A build's worker, on its own task queue; the flows here call nothing but each other, so no task is the user's. */
async function worker(work, connection) {
  const generated = await import(path.join(work, "worker.ts"));
  return Worker.create({ ...generated.workerOptions({}, { workflowsPath: path.join(fs.realpathSync(work), "workflow.ts") }), connection, bundlerOptions });
}

/** Serve the child on a server another runner started, until this process's input closes. */
async function serve(dir, address) {
  const work = copy(dir);
  const connection = await NativeConnection.connect({ address });
  const w = await worker(work, connection);
  const closed = new Promise((done) => {
    process.stdin.on("end", done);
    process.stdin.resume();
  });
  process.stdout.write("serving\n");
  await w.runUntil(closed);
  await connection.close();
  fs.rmSync(work, { recursive: true, force: true });
}

/** Start the other language's runner, serving the child; the function it gives back stops it. */
async function serveElsewhere(command, address) {
  const child = spawn(command[0], [...command.slice(1), address], { stdio: ["pipe", "pipe", "inherit"] });
  const exited = new Promise((ok, fail) => child.on("exit", (code) => (code === 0 ? ok() : fail(new Error(`the child's runner exited with ${code}`)))));
  await new Promise((ok, fail) => {
    child.stdout.on("data", (d) => String(d).includes("serving") && ok());
    exited.catch(fail);
  });
  return async () => {
    child.stdin.end();
    await exited;
  };
}

async function main(parentDir, childDir, runsFile, outFile) {
  const runs = JSON.parse(fs.readFileSync(runsFile, "utf8"));
  const childBy = process.env.DANDORI_CHILD_BY ? JSON.parse(process.env.DANDORI_CHILD_BY) : null;
  const parent = copy(parentDir);
  const child = childBy === null ? copy(childDir) : null;
  const env = await TestWorkflowEnvironment.createLocal({ server: { ui: false, log: { format: "pretty", level: "error" } } });
  const results = [];
  try {
    const { start } = await import(path.join(parent, "client.ts"));
    const stop = childBy === null ? null : await serveElsewhere(childBy, env.address);
    const workers = [await worker(parent, env.nativeConnection), ...(child === null ? [] : [await worker(child, env.nativeConnection)])];
    const runAll = async () => {
      for (const r of runs) {
        const handle = await start(env.client, r.id, r.input);
        let end;
        try {
          end = { succeed: (await handle.result()) ?? null };
        } catch (e) {
          if (!(e instanceof WorkflowFailedError)) throw e;
          end = { fail: { error: e.cause?.type ?? String(e.cause), cause: e.cause?.message || null } };
        }
        results.push({ end });
      }
    };
    // every worker runs until the runs are done
    let chain = runAll;
    for (const w of workers) {
      const inner = chain;
      chain = () => w.runUntil(inner());
    }
    await chain();
    if (stop !== null) await stop();
  } finally {
    await env.teardown();
    for (const w of [parent, child]) if (w !== null) fs.rmSync(w, { recursive: true, force: true });
  }
  fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
}

const args = process.argv.slice(2);
if (args[0] === "--serve") {
  await serve(args[1], args[2]);
} else {
  await main(...args);
}
process.exit(0);
