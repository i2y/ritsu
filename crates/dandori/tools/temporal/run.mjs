// Runs a workflow that dandori generated for Temporal, in the SDK's time-skipping test
// environment, against scripted answers, and prints what it did in the shape the
// reference interpreter prints for Temporal: every activity it called with its arguments
// and the answer it got, and how the workflow ended.
//
//   node tools/temporal/run.mjs <generated dir> <runs.json> <results.json>
//
// runs.json: { "workflow": name, "activities": [names], "runs": [ { "input": {...},
//              "answers": [ {"ok": value} | {"error": kind} ] } ] }
//
// A declared error is thrown as the ApplicationFailure the activity would throw; "failure"
// as one of a type the workflow does not declare. A timeout cannot be scripted this way,
// so runs.json holds no timeouts.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { TestWorkflowEnvironment } from "@temporalio/testing";
import { DefaultLogger, Runtime, Worker } from "@temporalio/worker";
import { ApplicationFailure } from "@temporalio/common";
import { WorkflowFailedError } from "@temporalio/client";

// the worker's log goes to stderr; the results go to a file
Runtime.install({ logger: new DefaultLogger("WARN", (entry) => process.stderr.write(`${entry.level} ${entry.message}\n`)) });

const here = path.dirname(fileURLToPath(import.meta.url));
const [dir, runsFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(runsFile, "utf8"));

let current = null;
const activities = {};
for (const name of spec.activities) {
  activities[name] = async (args) => {
    const ans = current.answers[current.next++];
    if (ans === undefined) throw ApplicationFailure.create({ type: "Dandori.Test.NoAnswer", message: `no answer for call ${current.next} (${name})`, nonRetryable: true });
    if ("ok" in ans) {
      current.steps.push({ call: { activity: name, args }, answer: { ok: ans.ok } });
      return ans.ok;
    }
    current.steps.push({ call: { activity: name, args }, answer: { error: ans.error, as: ans.error } });
    const type = ans.error === "failure" ? "Dandori.Test.Failure" : ans.error;
    throw ApplicationFailure.create({ type, message: "scripted", nonRetryable: true });
  };
}

const env = await TestWorkflowEnvironment.createTimeSkipping();
const results = [];
try {
  const worker = await Worker.create({
    connection: env.nativeConnection,
    taskQueue: "dandori",
    // the bundler loses its way when the path goes through a symbolic link (/var on macOS)
    workflowsPath: path.join(fs.realpathSync(dir), "workflow.ts"),
    activities,
    bundlerOptions: {
      webpackConfigHook: (config) => {
        config.resolve = config.resolve ?? {};
        config.resolve.modules = [...(config.resolve.modules ?? ["node_modules"]), path.join(here, "node_modules")];
        return config;
      },
    },
  });
  await worker.runUntil(async () => {
    for (const run of spec.runs) {
      current = { answers: run.answers, next: 0, steps: [] };
      let end;
      try {
        const out = await env.client.workflow.execute(spec.workflow, { args: [run.input], taskQueue: "dandori", workflowId: "test" });
        end = { succeed: out ?? null };
      } catch (e) {
        if (!(e instanceof WorkflowFailedError)) throw e;
        const c = e.cause;
        end = { fail: { error: c?.type ?? String(c), cause: c?.message || null } };
      }
      results.push({ steps: current.steps, end });
    }
  });
} finally {
  await env.teardown();
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
