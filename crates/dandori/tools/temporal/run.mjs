// Runs a workflow that dandori generated for Temporal, in the SDK's time-skipping test
// environment, against scripted answers, and prints what it did in the shape the
// reference interpreter prints for Temporal: every call with its arguments and the answer
// it got, and how the workflow ended.
//
//   node tools/temporal/run.mjs <generated dir> <runs.json> <results.json>
//
// runs.json: { "workflow": name, "own": [ { "method", "callback" } ], "rules": [activity],
//              "children": [ { "type", "queue" } ], "queues": [queue], "http": [...], "aws": [...],
//              "runs": [ { "input": {...}, "answers": [ {"ok": value} | {"error": kind} ] } ] }
//
// The tasks that say `lambda`, `http` or `aws` run the generated code, with a Transport that
// writes down what it would send (tools/transport.mjs). The tasks the user writes, the rules
// and the child workflows are stand-ins that answer from the scenario. A callback's answer
// comes as the signal the workflow waits for; a callback that the scenario times out gets
// none, and the time-skipping environment lets its wait run out. A task's own timeout cannot
// be scripted this way, so runs.json holds none of those.
//
// The rounds of `for … in parallel` run one at a time here, so that the calls come in the
// order the reference interpreter makes them.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { TestWorkflowEnvironment } from "@temporalio/testing";
import { DefaultLogger, Runtime, Worker } from "@temporalio/worker";
import { ApplicationFailure } from "@temporalio/common";
import { WorkflowFailedError } from "@temporalio/client";
import { makeTransport } from "../transport.mjs";

// the worker's log goes to stderr; the results go to a file
Runtime.install({ logger: new DefaultLogger("WARN", (entry) => process.stderr.write(`${entry.level} ${entry.message}\n`)) });

const here = path.dirname(fileURLToPath(import.meta.url));
const [dir, runsFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(runsFile, "utf8"));

// A copy of the generated code: Node runs the activities' TypeScript by stripping the types
// and wants the extension on a relative import; the rounds of a parallel loop run one at a time.
const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-temporal-"));
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith(".ts")) continue;
  let text = fs.readFileSync(path.join(dir, f), "utf8").replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`);
  if (f === "workflow.ts") text = text.replace(/dd\.atATime\(\d+\)/g, "dd.atATime(1)");
  fs.writeFileSync(path.join(work, f), text);
}
fs.symlinkSync(path.join(here, "node_modules"), path.join(work, "node_modules"));

// the child workflows: stand-ins that ask an activity for the scenario's answer
let children = `export * from "./workflow.ts";\nimport { proxyActivities, ApplicationFailure, ActivityFailure } from "@temporalio/workflow";\n`;
children += `const { dd_test_child } = proxyActivities({ startToCloseTimeout: "60 seconds", retry: { maximumAttempts: 1 } });\n`;
for (const c of spec.children ?? []) {
  children += `export async function ${c.type}(input) {\n  try {\n    return await dd_test_child({ type: ${JSON.stringify(c.type)}, args: input });\n  } catch (e) {\n    if (e instanceof ActivityFailure && e.cause instanceof ApplicationFailure) throw ApplicationFailure.create({ type: e.cause.type, message: e.cause.message, nonRetryable: true });\n    throw e;\n  }\n}\n`;
}
fs.writeFileSync(path.join(work, "workflows.ts"), children);

const { makeActivities } = await import(path.join(work, "activities.ts"));

let current = null;
let env = null;

function take(label) {
  const ans = current.answers[current.next++];
  if (ans === undefined) throw ApplicationFailure.create({ type: "Dandori.Test.NoAnswer", message: `no answer for call ${current.next} (${label})`, nonRetryable: true });
  return ans;
}

function recorded(ans) {
  return "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error };
}

function scripted(kind) {
  return ApplicationFailure.create({ type: kind === "failure" ? "Dandori.Test.Failure" : kind, message: "scripted", nonRetryable: true });
}

/** Answer a callback with a signal to the workflow the id names; a timeout gets no answer. */
function answerLater(callbackId, ans) {
  if (!("ok" in ans) && ans.error === "timeout") return;
  const [workflowId] = JSON.parse(callbackId);
  const signal = "ok" in ans ? { callback_id: callbackId, ok: ans.ok } : { callback_id: callbackId, error: ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, message: "scripted" };
  setTimeout(() => {
    env.client.workflow
      .getHandle(workflowId)
      .signal("dandori.callback", signal)
      .catch((e) => process.stderr.write(`could not answer the callback: ${e}\n`));
  }, 0);
}

// the stand-in transport writes every call down with the answer it takes
const run = {
  take: (call) => {
    const ans = take(JSON.stringify(call).slice(0, 80));
    current.steps.push({ call, answer: recorded(ans) });
    return ans;
  },
  answerLater: (id, ans) => answerLater(id, ans),
};

const own = {};
for (const t of spec.own ?? []) {
  own[t.method] = async (args) => {
    const ans = take(t.method);
    if (t.callback) {
      const { callback_id, ...rest } = args;
      current.steps.push({ call: { activity: t.name, args: rest }, answer: recorded(ans) });
      answerLater(callback_id, ans);
      return;
    }
    current.steps.push({ call: { activity: t.name, args }, answer: recorded(ans) });
    if ("ok" in ans) return ans.ok;
    throw scripted(ans.error);
  };
}
const activities = { ...makeActivities(own, makeTransport(spec, run)) };
for (const name of spec.rules ?? []) {
  activities[name] = async (args) => {
    const ans = take(name);
    current.steps.push({ call: { activity: name, args }, answer: recorded(ans) });
    if ("ok" in ans) return ans.ok;
    throw scripted(ans.error);
  };
}
activities.dd_test_child = async ({ type, args }) => {
  const ans = take(type);
  current.steps.push({ call: { child_workflow: type, args }, answer: recorded(ans) });
  if ("ok" in ans) return ans.ok;
  throw scripted(ans.error);
};

env = await TestWorkflowEnvironment.createTimeSkipping();
const results = [];
try {
  const queues = ["dandori", ...(spec.queues ?? []).filter((q) => q !== "dandori")];
  const workers = [];
  for (const taskQueue of queues) {
    workers.push(
      await Worker.create({
        connection: env.nativeConnection,
        taskQueue,
        // the bundler loses its way when the path goes through a symbolic link (/var on macOS)
        workflowsPath: path.join(fs.realpathSync(work), "workflows.ts"),
        activities,
        bundlerOptions: {
          webpackConfigHook: (config) => {
            config.resolve = config.resolve ?? {};
            config.resolve.modules = [...(config.resolve.modules ?? ["node_modules"]), path.join(here, "node_modules")];
            config.resolve.extensionAlias = { ".js": [".ts", ".js"] };
            return config;
          },
        },
      }),
    );
  }
  const runAll = async () => {
    for (const r of spec.runs) {
      current = { answers: r.answers, next: 0, steps: [] };
      let end;
      try {
        const out = await env.client.workflow.execute(spec.workflow, { args: [r.input], taskQueue: "dandori", workflowId: "test" });
        end = { succeed: out ?? null };
      } catch (e) {
        if (!(e instanceof WorkflowFailedError)) throw e;
        const c = e.cause;
        end = { fail: { error: c?.type ?? String(c), cause: c?.message || null } };
      }
      results.push({ steps: current.steps, end });
    }
  };
  // every worker runs until the runs are done
  let chain = runAll;
  for (const w of workers) {
    const inner = chain;
    chain = () => w.runUntil(inner);
  }
  await chain();
} finally {
  await env.teardown();
  fs.rmSync(work, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
