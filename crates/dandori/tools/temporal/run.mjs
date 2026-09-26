// Runs a workflow that dandori generated for Temporal on a real Temporal server — the dev server
// of the Temporal CLI, which the SDK's testing package starts — against scripted answers, and
// prints what it did in the shape the reference interpreter prints for Temporal: every call with
// its arguments and the answer it got, and how the workflow ended. With each run, it also prints
// what the query dandori.status says of the cases at the end (`cases`), and the search attribute
// DandoriCases (`shown`). The workers, the starts, the callbacks' answers and the query go
// through the generated worker.ts and client.ts.
//
//   node tools/temporal/run.mjs <generated dir> <runs.json> <results.json> [<histories dir>]
//   node tools/temporal/run.mjs --replay <generated dir> <histories dir> <results.json>
//   node tools/temporal/run.mjs --serve <generated dir> <runs.json> <steps.json> <server address>
//
// With DANDORI_ACTIVITIES_BY set to a command (a JSON list), the workers here run only the
// workflows, and the activities are the other language's: the command, with <steps.json> and
// the server's address added, serves them (tools/temporal-python/run.py --serve) until its
// input closes, and then writes the calls each run made to <steps.json>. With --serve, this
// runner is that command for the Python one: it serves the activities of the generated code,
// with the stand-ins, on the server at the address.
//
// Once the runs are over, the history of each is replayed with the same code, which must not
// find it nondeterministic; with a histories directory, the histories are also written there as
// JSON, one file a run: <workflow id>.json, and for each run that went on from it after a
// Continue-As-New, <workflow id>.<n>.json (the second is 2). With --replay, it only replays the
// histories of a directory (written by this runner, or by the Python one) with the code, and
// writes for each file the error, or null.
//
// runs.json: { "workflow": name, "own": [ { "method", "callback" } ], "rules": [activity],
//              "children": [ { "type", "queue" } ], "queues": [queue], "http": [...], "aws": [...],
//              "callbacks": [the proxy of each task that hands on a callback's id],
//              "runs": [ { "id": workflow id, "input": {...},
//                          "answers": [ {"ok": value} | {"error": kind} | {"cancel": true} ],
//                          "events": { "<the index of an answer>": the event it is for } } ] }
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
// request to cancel. An event (a task that says `event`) is sent with the generated client's
// `send` once the query dandori.status says the workflow waits for it; an event the scenario
// times out is not sent, and is written down when the next call comes, or when the run is over.
// While the workflow waits for one event, another it has must be refused.
//
// The server keeps real time, so the copy of the generated code that runs here waits far less:
// every duration of the workflow's timers (dd.ms) is at most 10 ms, and an activity or a child
// workflow gets 5 seconds before it times out (2 were too few when the whole test suite kept the
// machine busy), but for one that hands on a callback's id: the scenarios never time it out, and
// its stand-in answers the callback and sees a second answer refused before it returns, which
// takes a few workflow tasks. An event is waited for 5 seconds at most, long enough for the
// runner to see the wait and send it. The rounds of `for … in parallel` run one at a
// time here, so that the calls come in the order the reference interpreter makes them. A loop at
// the top of the flow goes on in a new run (Continue-As-New) at every round but the first of a
// run, since the history counts as long from one event on here (dd.CONTINUE_AT).

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { TestWorkflowEnvironment } from "@temporalio/testing";
import { DefaultLogger, NativeConnection, Runtime, Worker } from "@temporalio/worker";
import { Context } from "@temporalio/activity";
import { ApplicationFailure, CancelledFailure, SearchAttributeType, defineSearchAttributeKey } from "@temporalio/common";
import { historyFromJSON, historyToJSON } from "@temporalio/common/lib/proto-utils.js";
import { Client, Connection, WorkflowFailedError, WorkflowNotFoundError, WorkflowUpdateFailedError } from "@temporalio/client";
import { spawn } from "node:child_process";
import { makeTransport } from "../transport.mjs";

// the worker's log goes to stderr; the results go to a file
Runtime.install({ logger: new DefaultLogger("WARN", (entry) => process.stderr.write(`${entry.level} ${entry.message}\n`)) });

const here = path.dirname(fileURLToPath(import.meta.url));
const replaying = process.argv[2] === "--replay";
const serving = process.argv[2] === "--serve";
const [dir, runsFile, outFile, historiesDir] = replaying ? [process.argv[3], null, process.argv[5], process.argv[4]] : serving ? process.argv.slice(3) : process.argv.slice(2);
const spec = replaying ? JSON.parse(fs.readFileSync(path.join(historiesDir, "spec.json"), "utf8")) : JSON.parse(fs.readFileSync(runsFile, "utf8"));
/** The other language's runner, serving the activities: the command, when the workers here run only the workflows. */
const activitiesBy = serving ? null : JSON.parse(process.env.DANDORI_ACTIVITIES_BY ?? "null");

/** How long an activity or a child workflow gets here before it times out. */
const TIMEOUT = "5 seconds";
/** How long an event is waited for here, at most: long enough for the runner to see the wait and send it. */
const EVENT_MS = 5000;
/** How long a stand-in that the scenario times out keeps its activity busy. */
const LATE_MS = 10000;

// A copy of the generated code: Node runs the activities' TypeScript by stripping the types and
// wants the extension on a relative import; the rounds of a parallel loop run one at a time; the
// timers are short, and so are the activities' and the child workflows' timeouts; every history
// is long enough to go on in a new run.
const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-temporal-"));
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith(".ts")) continue;
  let text = fs.readFileSync(path.join(dir, f), "utf8").replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`);
  // the rules are stand-ins here, so the worker needs none of rulec's modules
  if (f === "rules.ts") text = "export const rules = {};\n";
  if (f === "workflow.ts") {
    text = text.replace(/dd\.atATime\(\d+\)/g, "dd.atATime(1)");
    const keep = (l) => (spec.callbacks ?? []).some((c) => l.startsWith(`const ${c} = `));
    text = text
      .split("\n")
      .map((l) => (keep(l) ? l : l.replace(/startToCloseTimeout: "\d+ seconds"/g, `startToCloseTimeout: "${TIMEOUT}"`)))
      .join("\n");
    text = text.replace(/workflowExecutionTimeout: "\d+ seconds"/g, `workflowExecutionTimeout: "${TIMEOUT}"`);
  }
  if (f === "runtime.ts") {
    if (!text.includes("return seconds * 1000;")) throw new Error("runtime.ts has no ms() to shorten");
    text = text.replace("return seconds * 1000;", "return Math.min(seconds * 1000, 10);");
    if (!text.includes("export const CONTINUE_AT = 10000;")) throw new Error("runtime.ts has no CONTINUE_AT to lower");
    text = text.replace("export const CONTINUE_AT = 10000;", "export const CONTINUE_AT = 1;");
    // an event's wait is not a timer to shorten to nothing: the runner has to see it, and send the event
    const eventWait = "got = await condition(() => events.has(name), ms(seconds));";
    if (!text.includes(eventWait)) throw new Error("runtime.ts has no event's wait to shorten");
    text = text.replace(eventWait, `got = await condition(() => events.has(name), Math.min(seconds * 1000, ${EVENT_MS}));`);
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

// the bundler loses its way when the path goes through a symbolic link (/var on macOS)
const workflowsPath = path.join(fs.realpathSync(work), "workflows.ts");
const bundlerOptions = {
  webpackConfigHook: (config) => {
    config.resolve = config.resolve ?? {};
    config.resolve.modules = [...(config.resolve.modules ?? ["node_modules"]), path.join(here, "node_modules")];
    config.resolve.extensionAlias = { ".js": [".ts", ".js"] };
    return config;
  },
};

// the rules are stand-ins here, so the worker needs none of rulec's modules
const rulesFile = path.join(work, "rules.ts");
if (fs.existsSync(rulesFile)) fs.writeFileSync(rulesFile, "export const rules = {};\n");
const { replay } = await import(path.join(work, "worker.ts"));

/** The workflow id of a history's file: <workflow id>.json, or <workflow id>.<n>.json for a run that went on from it. */
function workflowIdOf(file) {
  return file.replace(/(\.\d+)?\.json$/, "");
}

if (replaying) {
  // the generated worker's replay, which tells the workflows it finds a nondeterministic run of
  const files = fs.readdirSync(historiesDir).filter((f) => f.endsWith(".json") && f !== "spec.json").sort();
  const failed = await replay(
    files.map((f) => ({ workflowId: workflowIdOf(f), history: historyFromJSON(JSON.parse(fs.readFileSync(path.join(historiesDir, f), "utf8"))) })),
    { workflowsPath, bundlerOptions },
  );
  const why = new Map(failed.map((x) => [x.workflowId, x.error]));
  fs.writeFileSync(outFile, JSON.stringify(files.map((f) => ({ file: f, error: why.get(workflowIdOf(f)) ?? null })), null, 2) + "\n");
  fs.rmSync(work, { recursive: true, force: true });
  process.exit(0);
}

const { makeActivities } = await import(path.join(work, "activities.ts"));
const { workerOptions } = await import(path.join(work, "worker.ts"));
const { start, answer, status, send, EVENTS, histories: listed, TASK_QUEUE, WORKFLOW_TYPE } = await import(path.join(work, "client.ts"));
const CASES = defineSearchAttributeKey("DandoriCases", SearchAttributeType.KEYWORD_LIST);

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

function timesOut(ans) {
  return !("ok" in ans) && ans.error === "timeout";
}

/** Write down the events the workflow let run out of time before the call that takes the next answer. */
function settleEvents(run) {
  while (run.events?.[run.next] !== undefined && timesOut(run.answers[run.next])) {
    run.steps.push({ call: { event: run.events[run.next] }, answer: recorded(run.answers[run.next]) });
    run.next++;
  }
}

/**
 * Send the run's events, each once the workflow waits for it (the query's `events`): the value,
 * the error, or instead a request to cancel the workflow. Before, every other event must be
 * refused, since the workflow does not wait for it. The query may tell of a wait that is just
 * over, or of a run that is closing to go on in a new one (Continue-As-New): an event that the
 * workflow refuses so, or that finds the run closed, is sent again when it waits.
 */
async function sendEvents(run) {
  const waitsFor = async (name) => {
    try {
      return (await status(env.client, run.id)).events.includes(name);
    } catch {
      return false; // the workflow has not answered a query yet
    }
  };
  /** Whether the workflow took the event: not when it refused it, nor when the run it reached closed as it came. */
  const taken = async (name, a) => {
    try {
      await send(env.client, run.id, name, a);
      return true;
    } catch (e) {
      if (e instanceof WorkflowUpdateFailedError || e instanceof WorkflowNotFoundError) return false;
      throw e;
    }
  };
  while (!run.done) {
    const name = run.events?.[run.next];
    const ans = run.answers[run.next];
    if (name !== undefined && !timesOut(ans) && (await waitsFor(name))) {
      // the answer is the event's before it goes: once the workflow takes it, the next call may come at once
      run.next++;
      run.steps.push({ call: { event: name }, answer: recorded(ans) });
      if (ans.cancel === true) {
        await env.client.workflow.getHandle(run.id).cancel();
        continue;
      }
      const a = "ok" in ans ? { ok: ans.ok } : { error: ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, message: "scripted" };
      for (const other of EVENTS.filter((e) => e !== name)) {
        if (await taken(other, a)) throw new Error(`the workflow took the event ${other} while it waited for ${name}`);
      }
      if (!(await taken(name, a))) {
        // not taken: the workflow still waits for it, and nothing else has come
        run.next--;
        run.steps.pop();
      }
      continue;
    }
    await new Promise((ok) => setTimeout(ok, 20));
  }
}

function take(run, label) {
  settleEvents(run);
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
async function reply(run, ans) {
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
  const a = "ok" in ans ? { ok: ans.ok } : { error: ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, message: "scripted" };
  await answer(env.client, callbackId, a);
  await mustRefuse(callbackId, a, "a second answer to a callback");
  if (!refusalsChecked) {
    refusalsChecked = true;
    const [workflowId] = JSON.parse(callbackId);
    await mustRefuse(JSON.stringify([workflowId, "0"]), a, "an answer to a callback it does not wait for");
  }
}

// a second answer to a callback, and an answer to one the workflow never waits for, must be refused
let refusalsChecked = false;

async function mustRefuse(callbackId, a, why) {
  try {
    await answer(env.client, callbackId, a);
  } catch (e) {
    if (e instanceof WorkflowUpdateFailedError) return;
    throw e;
  }
  throw new Error(`the workflow took ${why}`);
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
    return reply(run, ans);
  };
}
const transport = makeTransport(spec, transportRun);
const activities = {};
for (const name of spec.rules ?? []) {
  activities[name] = async (args) => {
    const run = current();
    const ans = take(run, name);
    run.steps.push({ call: { activity: name, args }, answer: recorded(ans) });
    return reply(run, ans);
  };
}
activities.dd_test_child = async ({ type, args }) => {
  const run = current();
  const ans = take(run, type);
  run.steps.push({ call: { child_workflow: type, args }, answer: recorded(ans) });
  return reply(run, ans);
};

if (serving) {
  // the activities alone, on the server at the address, until the input closes; then the calls of each run
  const address = historiesDir;
  const connection = await NativeConnection.connect({ address });
  env = { client: new Client({ connection: await Connection.connect({ address }) }) };
  for (const r of spec.runs) runs.set(r.id, { id: r.id, answers: r.answers, events: {}, next: 0, steps: [] });
  const { workflowsPath: _, ...base } = workerOptions(own, { transport, workflowsPath });
  const queues = [TASK_QUEUE, ...(spec.queues ?? []).filter((q) => q !== TASK_QUEUE)];
  const workers = [];
  for (const taskQueue of queues) {
    workers.push(
      await Worker.create({
        ...base,
        connection,
        taskQueue,
        activities: { ...base.activities, ...activities },
        defaultHeartbeatThrottleInterval: "100 milliseconds",
        maxHeartbeatThrottleInterval: "100 milliseconds",
      }),
    );
  }
  const closed = new Promise((done) => {
    process.stdin.on("end", done);
    process.stdin.resume();
  });
  process.stdout.write("serving\n");
  await Promise.all(workers.map((w) => w.runUntil(closed)));
  fs.writeFileSync(outFile, JSON.stringify(Object.fromEntries([...runs].map(([id, r]) => [id, r.steps])), null, 2) + "\n");
  await connection.close();
  fs.rmSync(work, { recursive: true, force: true });
  process.exit(0);
}

/** Start the other language's runner, serving the activities; `done` closes its input, waits for it, and reads the calls of each run. */
async function serveActivities(address) {
  const stepsFile = path.join(work, "steps.json");
  const child = spawn(activitiesBy[0], [...activitiesBy.slice(1), stepsFile, address], { stdio: ["pipe", "pipe", "inherit"] });
  const exited = new Promise((ok, fail) => child.on("exit", (code) => (code === 0 ? ok() : fail(new Error(`the activities' runner exited with ${code}`)))));
  // its workers poll before the first run starts, so that no activity waits for them
  await new Promise((ok, fail) => {
    child.stdout.on("data", (d) => String(d).includes("serving") && ok());
    exited.catch(fail);
  });
  return async () => {
    child.stdin.end();
    await exited;
    return JSON.parse(fs.readFileSync(stepsFile, "utf8"));
  };
}

/** The histories of a workflow's runs, from the first: a run that went on in a new one (Continue-As-New) is followed by that one. */
async function runsOf(workflowId, firstRunId) {
  const out = [];
  for (let runId = firstRunId; runId; ) {
    const history = await env.client.workflow.getHandle(workflowId, runId).fetchHistory();
    out.push(history);
    runId = history.events.at(-1)?.workflowExecutionContinuedAsNewEventAttributes?.newExecutionRunId;
  }
  return out;
}

// the server fires a timer up to a second late unless told to shift its timers less
let histories = [];
env = await TestWorkflowEnvironment.createLocal({
  server: { ui: false, log: { format: "pretty", level: "error" }, extraArgs: ["--dynamic-config-value", 'history.timerProcessorMaxTimeShift="10ms"'], searchAttributes: [CASES] },
});
const results = [];
try {
  const base = workerOptions(own, { transport, workflowsPath });
  // with the other language's activities, a worker here serves the workflow and the child workflows' stand-ins only
  const served = activitiesBy === null ? null : await serveActivities(env.address);
  const queues =
    served === null ? [TASK_QUEUE, ...(spec.queues ?? []).filter((q) => q !== TASK_QUEUE)] : [TASK_QUEUE, ...new Set((spec.children ?? []).map((c) => c.queue).filter((q) => q && q !== TASK_QUEUE))];
  const workers = [];
  for (const taskQueue of queues) {
    workers.push(
      await Worker.create({
        ...base,
        connection: env.nativeConnection,
        taskQueue,
        activities: served === null ? { ...base.activities, ...activities } : {},
        // heartbeats go out at once, so that a held activity hears soon that it is over
        defaultHeartbeatThrottleInterval: "100 milliseconds",
        maxHeartbeatThrottleInterval: "100 milliseconds",
        bundlerOptions,
      }),
    );
  }
  const runAll = async () => {
    const ends = await Promise.all(
      spec.runs.map(async (r) => {
        const run = { id: r.id, answers: r.answers, events: r.events ?? {}, next: 0, steps: [], done: false };
        runs.set(r.id, run);
        const handle = await start(env.client, r.id, r.input, { searchAttributes: true });
        const sending = Object.keys(run.events).length > 0 ? sendEvents(run) : Promise.resolve();
        let end;
        try {
          const out = await handle.result();
          end = { succeed: out ?? null };
        } catch (e) {
          if (!(e instanceof WorkflowFailedError)) throw e;
          const c = e.cause;
          if (c instanceof CancelledFailure) end = { cancel: null };
          else end = { fail: { error: c?.type ?? String(c), cause: c?.message || null } };
        } finally {
          run.done = true;
        }
        await sending;
        settleEvents(run);
        // what the query and the search attribute say of the cases once the run is over
        const cases = (await status(env.client, r.id)).cases;
        const shown = (await handle.describe()).typedSearchAttributes.get(CASES) ?? null;
        return { end, cases, shown, histories: await runsOf(r.id, handle.firstExecutionRunId) };
      }),
    );
    spec.runs.forEach((r, i) => results.push({ steps: runs.get(r.id).steps, end: ends[i].end, cases: ends[i].cases, shown: ends[i].shown }));
    histories = spec.runs.flatMap((r, i) => ends[i].histories.map((history, n) => ({ workflowId: r.id, file: n === 0 ? `${r.id}.json` : `${r.id}.${n + 1}.json`, history })));
  };
  // every worker runs until the runs are done
  let chain = runAll;
  for (const w of workers) {
    const inner = chain;
    chain = () => w.runUntil(inner);
  }
  await chain();
  if (served !== null) {
    const steps = await served();
    for (const [i, r] of spec.runs.entries()) results[i].steps = steps[r.id];
  }
  // the generated client finds every run by the workflow's type, the ones that went on in a new run too (the server lists them a moment late)
  for (let tries = 0; ; tries++) {
    const found = [];
    for await (const h of listed(env.client, `WorkflowType = '${WORKFLOW_TYPE}'`)) found.push(h.workflowId);
    if (found.length === histories.length) break;
    if (tries === 50) throw new Error(`the client's histories found ${found.length} run(s) of ${histories.length}`);
    await new Promise((ok) => setTimeout(ok, 100));
  }
  // the same code, replaying what it did with the generated worker's replay, must find it deterministic
  const failed = await replay(histories, { workflowsPath, bundlerOptions });
  if (failed.length > 0) throw new Error(`replaying with the same code: ${failed.map((x) => `${x.workflowId}: ${x.error}`).join("; ")}`);
  if (historiesDir) {
    fs.mkdirSync(historiesDir, { recursive: true });
    for (const h of histories) fs.writeFileSync(path.join(historiesDir, h.file), historyToJSON(h.history) + "\n");
    fs.writeFileSync(path.join(historiesDir, "spec.json"), JSON.stringify({ children: spec.children ?? [] }) + "\n");
  }
} finally {
  await env.teardown();
  fs.rmSync(work, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
