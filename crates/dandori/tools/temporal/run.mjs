// Runs a workflow that dandori generated for Temporal on a real Temporal server — the dev server
// of the Temporal CLI, which the SDK's testing package starts — against scripted answers, and
// prints what it did in the shape the reference interpreter prints for Temporal: every call with
// its arguments and the answer it got, and how the workflow ended.
//
//   node tools/temporal/run.mjs <generated dir> <runs.json> <results.json>
//
// runs.json: { "workflow": name, "own": [ { "method", "callback" } ], "rules": [activity],
//              "children": [ { "type", "queue" } ], "queues": [queue], "http": [...], "aws": [...],
//              "runs": [ { "id": workflow id, "input": {...},
//                          "answers": [ {"ok": value} | {"error": kind} | {"cancel": true} ] } ] }
//
// Every run goes at once, each as the workflow with its own id; a stand-in finds its run by the
// id of the workflow that called it (a child workflow's id starts with its parent's). The tasks
// that say `lambda`, `http` or `aws` run the generated code, with a Transport that writes down
// what it would send (tools/transport.mjs). The tasks the user writes, the rules and the child
// workflows are stand-ins that answer from the scenario. A callback's answer comes as the signal
// the workflow waits for, sent before the task that hands the id on returns; a callback that
// the scenario times out gets none. A call that the scenario times out gets no answer either:
// its stand-in keeps the activity busy until the server times it out. A call during which the
// scenario cancels the workflow asks the server to cancel it, and keeps the activity busy until
// the server tells it that it is cancelled; a callback's answer that is a cancellation is the
// request to cancel.
//
// The server keeps real time, so the copy of the generated code that runs here waits far less:
// every duration of the workflow's timers (dd.ms) is at most 10 ms, and an activity or a child
// workflow gets 2 seconds before it times out. The rounds of `for … in parallel` run one at a
// time here, so that the calls come in the order the reference interpreter makes them.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { TestWorkflowEnvironment } from "@temporalio/testing";
import { DefaultLogger, Runtime, Worker } from "@temporalio/worker";
import { Context } from "@temporalio/activity";
import { ApplicationFailure, CancelledFailure } from "@temporalio/common";
import { WorkflowFailedError } from "@temporalio/client";
import { makeTransport } from "../transport.mjs";

// the worker's log goes to stderr; the results go to a file
Runtime.install({ logger: new DefaultLogger("WARN", (entry) => process.stderr.write(`${entry.level} ${entry.message}\n`)) });

const here = path.dirname(fileURLToPath(import.meta.url));
const [dir, runsFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(runsFile, "utf8"));

/** How long an activity or a child workflow gets here before it times out. */
const TIMEOUT = "2 seconds";
/** How long a stand-in that the scenario times out keeps its activity busy. */
const LATE_MS = 5000;

// A copy of the generated code: Node runs the activities' TypeScript by stripping the types and
// wants the extension on a relative import; the rounds of a parallel loop run one at a time; the
// timers are short, and so are the activities' and the child workflows' timeouts.
const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-temporal-"));
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith(".ts")) continue;
  let text = fs.readFileSync(path.join(dir, f), "utf8").replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`);
  if (f === "workflow.ts") {
    text = text.replace(/dd\.atATime\(\d+\)/g, "dd.atATime(1)");
    text = text.replace(/startToCloseTimeout: "\d+ seconds"/g, `startToCloseTimeout: "${TIMEOUT}"`);
    text = text.replace(/workflowExecutionTimeout: "\d+ seconds"/g, `workflowExecutionTimeout: "${TIMEOUT}"`);
  }
  if (f === "runtime.ts") {
    if (!text.includes("return seconds * 1000;")) throw new Error("runtime.ts has no ms() to shorten");
    text = text.replace("return seconds * 1000;", "return Math.min(seconds * 1000, 10);");
  }
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

/** Each run by its workflow's id: the answers it gets, the next one, and the calls it made. */
const runs = new Map();
let env = null;

/** The run of the activity being served: its workflow's id, or its parent's. */
function current() {
  const id = Context.current().info.workflowExecution.workflowId;
  const run = runs.get(id.split("/")[0]);
  if (!run) throw new Error(`no run for the workflow ${id}`);
  return run;
}

function take(run, label) {
  const ans = run.answers[run.next++];
  if (ans === undefined) throw ApplicationFailure.create({ type: "Dandori.Test.NoAnswer", message: `no answer for call ${run.next} (${label})`, nonRetryable: true });
  return ans;
}

function recorded(ans) {
  if (ans.cancel === true) return { cancel: true };
  return "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error };
}

function scripted(kind) {
  return ApplicationFailure.create({ type: kind === "failure" ? "Dandori.Test.Failure" : kind, message: "scripted", nonRetryable: true });
}

/** Heartbeat until the server says the activity is over (cancelled, or timed out), at most LATE_MS. */
async function holdOn() {
  const context = Context.current();
  const timer = setInterval(() => context.heartbeat(), 100);
  try {
    await Promise.race([context.cancelled, new Promise((ok) => setTimeout(ok, LATE_MS))]);
  } catch {
    // the server is done with it
  } finally {
    clearInterval(timer);
  }
}

/** A call the scenario times out: keep the activity busy past its timeout, so that the server times it out. */
async function late() {
  await holdOn();
  throw ApplicationFailure.create({ type: "Dandori.Test.Late", message: "the server should have timed this out", nonRetryable: true });
}

/** A call during which the scenario cancels the workflow: ask the server, and keep the activity until it is cancelled. */
async function cancelling(run) {
  await env.client.workflow.getHandle(run.id).cancel();
  await holdOn();
  throw ApplicationFailure.create({ type: "Dandori.Test.Cancelled", message: "the workflow was cancelled", nonRetryable: true });
}

/** A call the runner keeps from answering: one that times out, or one during which the workflow is cancelled. */
function hold(run, ans) {
  return ans.cancel === true ? cancelling(run) : late();
}

/** The answer of a stand-in: its value, the scripted error, no answer in time, or none before the workflow is cancelled. */
async function answer(run, ans) {
  if (ans.cancel === true) return cancelling(run);
  if ("ok" in ans) return ans.ok;
  if (ans.error === "timeout") return late();
  throw scripted(ans.error);
}

/**
 * Answer a callback with a signal to the workflow the id names, before the task that hands the
 * id on returns: then the answer is in the history before the workflow waits for it, and its
 * short wait cannot run out first. A timeout gets no answer.
 */
async function answerLater(callbackId, ans) {
  if (!("ok" in ans) && ans.error === "timeout") return;
  if (ans.cancel === true) {
    await env.client.workflow.getHandle(current().id).cancel();
    return;
  }
  const [workflowId] = JSON.parse(callbackId);
  const signal = "ok" in ans ? { callback_id: callbackId, ok: ans.ok } : { callback_id: callbackId, error: ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, message: "scripted" };
  await env.client.workflow.getHandle(workflowId).signal("dandori.callback", signal);
}

// the stand-in transport writes every call down with the answer it takes; a call it times out gets none
const transportRun = {
  take: (call) => {
    const run = current();
    const ans = take(run, JSON.stringify(call).slice(0, 80));
    run.steps.push({ call, answer: recorded(ans) });
    return ans;
  },
  answerLater: (id, ans) => answerLater(id, ans),
  hold: (ans) => hold(current(), ans),
};

const own = {};
for (const t of spec.own ?? []) {
  own[t.method] = async (args) => {
    const run = current();
    const ans = take(run, t.method);
    if (t.callback) {
      const { callback_id, ...rest } = args;
      run.steps.push({ call: { activity: t.name, args: rest }, answer: recorded(ans) });
      await answerLater(callback_id, ans);
      return;
    }
    run.steps.push({ call: { activity: t.name, args }, answer: recorded(ans) });
    return answer(run, ans);
  };
}
const activities = { ...makeActivities(own, makeTransport(spec, transportRun)) };
for (const name of spec.rules ?? []) {
  activities[name] = async (args) => {
    const run = current();
    const ans = take(run, name);
    run.steps.push({ call: { activity: name, args }, answer: recorded(ans) });
    return answer(run, ans);
  };
}
activities.dd_test_child = async ({ type, args }) => {
  const run = current();
  const ans = take(run, type);
  run.steps.push({ call: { child_workflow: type, args }, answer: recorded(ans) });
  return answer(run, ans);
};

// the server fires a timer up to a second late unless told to shift its timers less
env = await TestWorkflowEnvironment.createLocal({ server: { ui: false, log: { format: "pretty", level: "error" }, extraArgs: ["--dynamic-config-value", 'history.timerProcessorMaxTimeShift="10ms"'] } });
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
        // heartbeats go out at once, so that a held activity hears soon that it is over
        defaultHeartbeatThrottleInterval: "100 milliseconds",
        maxHeartbeatThrottleInterval: "100 milliseconds",
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
    const ends = await Promise.all(
      spec.runs.map(async (r) => {
        runs.set(r.id, { id: r.id, answers: r.answers, next: 0, steps: [] });
        try {
          const out = await env.client.workflow.execute(spec.workflow, { args: [r.input], taskQueue: "dandori", workflowId: r.id });
          return { succeed: out ?? null };
        } catch (e) {
          if (!(e instanceof WorkflowFailedError)) throw e;
          const c = e.cause;
          if (c instanceof CancelledFailure) return { cancel: null };
          return { fail: { error: c?.type ?? String(c), cause: c?.message || null } };
        }
      }),
    );
    spec.runs.forEach((r, i) => results.push({ steps: runs.get(r.id).steps, end: ends[i] }));
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
