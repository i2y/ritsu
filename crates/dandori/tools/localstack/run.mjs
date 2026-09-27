// Runs the state machines dandori generated for Step Functions on LocalStack's Step Functions,
// with the answers of every call mocked, and prints what each run did in the shape the reference
// interpreter prints: every call as it went out, with the answer the scenario gave it, every wait
// and retry wait, and how the execution ended.
//
//   node tools/localstack/run.mjs <spec.json> <results.json>
//
// spec.json: { "image": the LocalStack image,
//              "flows": [ { "name", "definition": <the ASL as JSON>,
//                           "runs": [ { "id", "input", "answers": [ {"ok": value} | {"error": kind, "as": ASL error name} ] } ] } ] }
// results.json: { "flows": [ { "runs": [ { "steps", "end" } ], "http": the HTTP Tasks that got another resource } ] }
//
// The runner starts the image in Docker with Step Functions alone, and uses its mocked service
// integrations (SFN_MOCK_CONFIG, the mock configuration of Step Functions Local): every run is a
// test case of its state machine, started as `<state machine ARN>#<run id>`, and every Task state
// it goes through takes its answers from the file in turn, as the service integration would give
// them ({ "Payload": … } for Lambda, { "ResponseBody": … } for HTTP, the API's answer for an agent,
// { "Output": … } for a nested execution; ../asl-run.mjs's `wrap`) or as the error the reference
// names. Which state gets which answer comes from playing the run with ../asl-run.mjs: the n-th
// call a state makes there gets the n-th answer of that state here. If LocalStack goes another
// way, a state runs out of answers or gets another's, and the run comes out different.
//
// Every run of every flow goes at once. The calls, the waits and the end are read from the
// execution's history. The definitions run with these changes, and no others:
// - The rounds of a Map run one at a time (MaxConcurrency 1), so that the calls come in the order
//   the reference interpreter makes them.
// - A Wait goes on at once (Seconds 0), and assigns a variable of its own the time it would wait
//   for (its Seconds, or the value of its Timestamp), which the history shows.
// - LocalStack's mock file cannot throw an error whose name begins with States. (it takes a mocked
//   error for the task's own, whose names may not), so a task's timeout (States.Timeout), the
//   HTTP Task's errors (States.Http.StatusCode.<status>) and an agent's failure (below) are thrown
//   with Dandori. before the name, the Retry and the Catch name them so, and the end names them
//   back. Retry and Catch take them as they take the real ones: the definitions do not name
//   States.TaskFailed, the one name that tells a timeout from other errors.
// - LocalStack 4.14.0 does not know the HTTP Task (arn:aws:states:::http:invoke), and refuses a
//   definition that has one. Then those Task states get the resource of an AWS SDK integration
//   instead, of an API that takes no arguments and whose answer has none of the HTTP Task's keys
//   (arn:aws:states:::aws-sdk:sts:getCallerIdentity). LocalStack evaluates their arguments, keeps
//   them all in the history, and hands the mocked response and error on as they are. (API
//   Gateway's resource, the HTTP Task's nearest, does neither: it leaves out the arguments it does
//   not take, and names every mocked error ApiGateway.FailureEventException.)
// A retry waits as long as its Retry says, since LocalStack sleeps, and the runner times it from
// the history: from the end of one try to the start of the next, to the millisecond.
//
// An agent's failure is thrown as the error the model's refusal fails its Task with on Step
// Functions: reading the refusal raises `$error`, and an expression that raises an error throws
// States.QueryEvaluationError, which Retry and Catch take (the Step Functions Developer Guide,
// "Handling expression errors"). LocalStack 4.14.0 fails the execution with States.Runtime
// instead, which nothing takes, so the refusal itself is not played here; ../asl-run.mjs plays it.
//
// The runner says on stderr how far it got, gives up on a request to LocalStack after two minutes
// and on the whole after fifteen. DANDORI_LOCALSTACK_KEEP=1 leaves the container running, to look
// at the executions.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFile, execFileSync } from "node:child_process";
import { promisify } from "node:util";
import { play, wire, wrap } from "../asl-run.mjs";

const exec = promisify(execFile);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const [specFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(specFile, "utf8"));

const HTTP = "arn:aws:states:::http:invoke";
const STAND_IN = "arn:aws:states:::aws-sdk:sts:getCallerIdentity";
const MOCKS = "/tmp/dandori-mocks.json";
const ROLE = "arn:aws:iam::000000000000:role/dandori";
const container = `dandori-localstack-${process.pid}`;

const began = Date.now();
let doing = "starting LocalStack";

// how far the runner got, on stderr, which the test shows when the runner fails
function say(what) {
  doing = `after: ${what}`;
  process.stderr.write(`${((Date.now() - began) / 1000).toFixed(1)}s ${what}\n`);
}

// Node leaves when nothing it knows of is pending. Once, in the whole test suite, it left while
// main() was still waiting (before the first state machine was made, with nothing on stderr);
// this timer keeps the runner up until main() is done, and ends it after fifteen minutes.
const alive = setInterval(() => {
  if (Date.now() - began < 15 * 60 * 1000) return;
  say(`gave up after fifteen minutes, ${doing}`);
  if (!process.env.DANDORI_LOCALSTACK_KEEP) execFileSync("docker", ["rm", "-f", container], { stdio: "ignore" });
  process.exit(1);
}, 1000);

async function docker(args) {
  return (await exec("docker", args, { maxBuffer: 1 << 26 })).stdout.trim();
}

let endpoint;

async function sfn(op, body) {
  const res = await fetch(endpoint, { method: "POST", headers: { "content-type": "application/x-amz-json-1.0", "x-amz-target": `AWSStepFunctions.${op}` }, body: JSON.stringify(body), signal: AbortSignal.timeout(2 * 60 * 1000) });
  const text = await res.text();
  if (!res.ok) throw new Error(`${op}: ${res.status} ${text.slice(0, 4000)}`);
  return JSON.parse(text);
}

// f on every item, n at a time
async function inTurns(items, n, f) {
  const out = new Array(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.min(n, items.length) }, async () => {
      while (next < items.length) {
        const i = next++;
        out[i] = await f(items[i], i);
      }
    }),
  );
  return out;
}

// every state of a definition by name, the rounds' too; a state machine's names are unique
function statesOf(def) {
  const out = {};
  const walk = (states) => {
    for (const [name, st] of Object.entries(states)) {
      if (name in out) throw new Error(`two states are named ${name}`);
      out[name] = st;
      if (st.Type === "Map") walk(st.ItemProcessor.States);
    }
  };
  walk(def.States);
  return out;
}

// the definition as it runs here (the head of this file says how), and the variables of its waits
function forLocalStack(def) {
  const d = structuredClone(def);
  const waits = {};
  for (const [name, st] of Object.entries(statesOf(d))) {
    if (st.Type === "Wait") {
      const variable = `dd_waited_${Object.keys(waits).length + 1}`;
      waits[name] = { variable, kind: st.Seconds !== undefined ? "wait" : "wait_until" };
      st.Assign = { ...(st.Assign ?? {}), [variable]: st.Seconds ?? st.Timestamp };
      delete st.Timestamp;
      st.Seconds = 0;
    }
    if (st.Type === "Map") st.MaxConcurrency = 1;
    for (const r of [...(st.Retry ?? []), ...(st.Catch ?? [])]) r.ErrorEquals = r.ErrorEquals.map(renamed);
  }
  return { definition: d, waits };
}

// Step Functions' errors that a scenario has a task end with, as they are named here
const THROWN = /^States\.(Timeout$|QueryEvaluationError$|Http\.)/;
const RENAMED = "Dandori.";
const renamed = (e) => (THROWN.test(e) ? RENAMED + e : e);
const named = (e) => (e?.startsWith(RENAMED) && THROWN.test(e.slice(RENAMED.length)) ? e.slice(RENAMED.length) : e);

// the HTTP Tasks with the stand-in resource; how many there were
function withoutHttpTasks(def) {
  let n = 0;
  for (const st of Object.values(statesOf(def))) {
    if (st.Type === "Task" && st.Resource === HTTP) {
      st.Resource = STAND_IN;
      n++;
    }
  }
  return n;
}

// what the mock file gives a call: the service integration's response, or the error the reference names
function mocked(c) {
  if ("ok" in c.answer) return { Return: wrap(c.resource, c.args, c.answer.ok) };
  return { Throw: { Error: renamed(c.answer.as), Cause: "scripted" } };
}

function machineName(name) {
  return name.replace(/[^A-Za-z0-9_-]/g, "-").slice(0, 80);
}

// A run as the history tells it, in the shape the reference interpreter prints.
function trace(events, flow, answers) {
  const steps = [];
  let end = null;
  const made = {};
  // the Task state the calls belong to, and when its last try ended
  let task = null;
  let tried = null;
  for (const e of events) {
    switch (e.type) {
      case "TaskStateEntered":
        task = e.stateEnteredEventDetails.name;
        tried = null;
        break;
      case "TaskScheduled": {
        // a try after a try of the same state is a retry, and the time between is its wait
        if (tried !== null) steps.push({ retry_wait: Math.round((e.timestamp - tried) * 1000) / 1000 });
        tried = null;
        const n = made[task] ?? 0;
        made[task] = n + 1;
        const a = answers[task]?.[n];
        const call = wire(flow.states[task].Resource, JSON.parse(e.taskScheduledEventDetails.parameters));
        steps.push({ call, answer: a === undefined ? null : "ok" in a ? { ok: a.ok } : { error: a.error, as: a.as } });
        break;
      }
      case "TaskSucceeded":
      case "TaskFailed":
      case "TaskTimedOut":
        tried = e.timestamp;
        break;
      case "WaitStateExited": {
        const d = e.stateExitedEventDetails;
        const w = flow.waits[d.name];
        steps.push({ [w.kind]: JSON.parse(d.assignedVariables[w.variable]) });
        break;
      }
      case "ExecutionSucceeded":
        end = { succeed: JSON.parse(e.executionSucceededEventDetails.output ?? "null") };
        break;
      case "ExecutionFailed": {
        const d = e.executionFailedEventDetails;
        end = { fail: { error: named(d.error ?? null), cause: d.cause ?? null } };
        break;
      }
      case "ExecutionAborted":
      case "ExecutionTimedOut":
        end = { stopped: e.type };
        break;
    }
  }
  return { steps, end };
}

async function histories(arn) {
  const events = [];
  let nextToken;
  do {
    const page = await sfn("GetExecutionHistory", { executionArn: arn, maxResults: 1000, ...(nextToken ? { nextToken } : {}) });
    events.push(...page.events);
    nextToken = page.nextToken;
  } while (nextToken);
  return events;
}

async function main() {
  await docker(["run", "-d", "--rm", "--name", container, "-p", "127.0.0.1::4566", "-e", "SERVICES=stepfunctions", "-e", "EAGER_SERVICE_LOADING=1", "-e", "DISABLE_EVENTS=1", "-e", `SFN_MOCK_CONFIG=${MOCKS}`, spec.image]);
  endpoint = `http://${await docker(["port", container, "4566/tcp"])}/`;
  say(`started ${spec.image} at ${endpoint}`);

  // every run's calls, from playing it with ../asl-run.mjs
  const flows = [];
  for (const f of spec.flows) {
    const { definition, waits } = forLocalStack(f.definition);
    const plays = [];
    for (const r of f.runs) plays.push((await play(f.definition, r, r.id)).calls);
    flows.push({ name: machineName(f.name), definition, waits, plays, states: statesOf(f.definition), http: 0 });
  }
  say(`played ${spec.flows.length} flow(s) with ../asl-run.mjs`);

  for (let i = 0; ; i++) {
    const health = await fetch(`${endpoint}_localstack/health`, { signal: AbortSignal.timeout(5000) })
      .then((r) => r.json())
      .catch(() => null);
    if (["available", "running"].includes(health?.services?.stepfunctions)) break;
    if (i > 240) throw new Error("LocalStack did not start in two minutes");
    await sleep(500);
  }
  say("LocalStack is up");

  for (const flow of flows) {
    const create = () => sfn("CreateStateMachine", { name: flow.name, definition: JSON.stringify(flow.definition), roleArn: ROLE });
    try {
      flow.arn = (await create()).stateMachineArn;
    } catch (e) {
      if (!/Unknown service 'http'/.test(e.message)) throw e;
      flow.http = withoutHttpTasks(flow.definition);
      flow.arn = (await create()).stateMachineArn;
    }
  }

  // the answers of every run, by the Task state that gets them, in the mock file
  const config = { StateMachines: {}, MockedResponses: {} };
  for (const flow of flows) {
    const cases = {};
    flow.answers = flow.plays.map((calls, ri) => {
      const id = spec.flows[flows.indexOf(flow)].runs[ri].id;
      const byState = {};
      for (const c of calls) (byState[c.state] ??= []).push(c);
      cases[id] = {};
      for (const [state, list] of Object.entries(byState)) {
        const key = `${flow.name}/${id}/${state}`;
        cases[id][state] = key;
        config.MockedResponses[key] = Object.fromEntries(list.map((c, i) => [String(i), mocked(c)]));
      }
      return Object.fromEntries(Object.entries(byState).map(([state, list]) => [state, list.map((c) => c.answer)]));
    });
    config.StateMachines[flow.name] = { TestCases: cases };
  }
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-localstack-"));
  fs.writeFileSync(path.join(work, "mocks.json"), JSON.stringify(config));
  await docker(["cp", path.join(work, "mocks.json"), `${container}:${MOCKS}`]);
  say(`created ${flows.length} state machine(s) and copied the mock file in`);
  fs.rmSync(work, { recursive: true, force: true });

  const runs = flows.flatMap((flow, fi) => spec.flows[fi].runs.map((r, ri) => ({ flow, r, ri })));
  await inTurns(runs, 8, async (x) => {
    x.arn = (await sfn("StartExecution", { stateMachineArn: `${x.flow.arn}#${x.r.id}`, name: x.r.id, input: JSON.stringify(x.r.input) })).executionArn;
  });
  say(`started ${runs.length} execution(s)`);

  const deadline = Date.now() + 10 * 60 * 1000;
  for (;;) {
    const running = (await inTurns(flows, 4, async (flow) => (await sfn("ListExecutions", { stateMachineArn: flow.arn, statusFilter: "RUNNING", maxResults: 1000 })).executions)).flat();
    if (running.length === 0) break;
    if (Date.now() > deadline) throw new Error(`still running after ten minutes: ${running.map((e) => e.executionArn).join(", ")}`);
    await sleep(500);
  }
  say("every execution ended");

  const results = flows.map((flow) => ({ runs: [], http: flow.http }));
  await inTurns(runs, 8, async (x) => {
    results[flows.indexOf(x.flow)].runs[x.ri] = trace(await histories(x.arn), x.flow, x.flow.answers[x.ri]);
  });
  fs.writeFileSync(outFile, JSON.stringify({ flows: results }, null, 2) + "\n");
}

try {
  await main();
} finally {
  if (process.env.DANDORI_LOCALSTACK_KEEP) process.stderr.write(`LocalStack stays at ${endpoint} (docker rm -f ${container})\n`);
  else await docker(["rm", "-f", container]).catch(() => {});
  clearInterval(alive);
}
