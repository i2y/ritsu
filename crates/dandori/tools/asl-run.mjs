// Runs a state machine that dandori generated, with JSONata 2.0.6, against scripted
// answers, and prints what it did in the shape the reference interpreter prints:
// every call as it went out and the answer it got, every wait and retry wait, and how
// the execution ended. It runs only what dandori writes: Pass, Task, Choice, Wait,
// Succeed, Fail and inline Map, with QueryLanguage JSONata, variables, Retry and Catch.
//
// A Map's rounds run one after another. A round reads the variables outside it and keeps
// its own; assigning a variable an outer scope has is refused, as Step Functions refuses it.
//
//   node tools/asl-run.mjs <definition.asl.json> <run.json> [execution name]
//
// run.json: { "input": {...}, "answers": [ {"ok": value} | {"error": kind, "as": ASL error name} ] }

import fs from "node:fs";
import jsonata from "jsonata";

const def = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const run = JSON.parse(fs.readFileSync(process.argv[3], "utf8"));
const execution = process.argv[4] ?? "test";

let vars = {};
const steps = [];
let next = 0;

class Stop extends Error {
  constructor(end) {
    super("stop");
    this.end = end;
  }
}

async function evaluate(v, states) {
  if (typeof v === "string") {
    const m = /^\{%([\s\S]*)%\}$/.exec(v);
    if (!m) return v;
    let r;
    try {
      r = await jsonata(m[1]).evaluate({}, { ...vars, states });
    } catch (e) {
      throw new Stop({ fail: { error: "States.QueryEvaluationError", cause: String(e.message ?? e) } });
    }
    if (r === undefined) {
      throw new Stop({ fail: { error: "States.QueryEvaluationError", cause: `the expression returned nothing: ${m[1].trim()}` } });
    }
    return plain(r);
  }
  if (Array.isArray(v)) {
    const out = [];
    for (const x of v) out.push(await evaluate(x, states));
    return out;
  }
  if (v !== null && typeof v === "object") {
    const out = {};
    for (const [k, x] of Object.entries(v)) out[k] = await evaluate(x, states);
    return out;
  }
  return v;
}

// JSONata hands back arrays with extra properties; the state machine sees plain JSON
function plain(x) {
  return x === undefined ? x : JSON.parse(JSON.stringify(x));
}

// the names the scopes around the current one assigned, which it must not assign
let outer = new Set();

async function assign(block, states) {
  // every expression reads the values as they were on entry; then all are set
  const values = {};
  for (const [k, x] of Object.entries(block)) values[k] = await evaluate(x, states);
  for (const k of Object.keys(values)) {
    if (outer.has(k)) throw new Stop({ fail: { error: "States.Runtime", cause: `a Map round assigns ${k}, a variable of an outer scope` } });
  }
  Object.assign(vars, values);
}

function matches(names, err) {
  return names.some((n) => n === err || (n === "States.ALL" && err !== "States.Runtime" && err !== "States.DataLimitExceeded") || (n === "States.TaskFailed" && err !== "States.Timeout"));
}

function wire(resource, args) {
  if (resource.startsWith("arn:aws:states:::lambda:invoke")) {
    const payload = { ...(args.Payload ?? {}) };
    delete payload.task_token;
    return { lambda: args.FunctionName, payload };
  }
  if (resource.startsWith("arn:aws:states:::aws-sdk:")) {
    return { aws: resource.slice("arn:aws:states:::aws-sdk:".length), args };
  }
  if (resource === "arn:aws:states:::sqs:sendMessage.waitForTaskToken") {
    const body = { ...(args.MessageBody ?? {}) };
    delete body.task_token;
    return { aws: "sqs:sendMessage", args: { ...args, MessageBody: body } };
  }
  if (resource === "arn:aws:states:::states:startExecution.sync:2") {
    return { state_machine: args.StateMachineArn, input: args.Input };
  }
  if (resource === "arn:aws:states:::http:invoke") {
    const w = { http: args.Method, url: args.ApiEndpoint };
    if (args.Headers !== undefined) w.headers = args.Headers;
    if (args.RequestBody !== undefined) w.body = args.RequestBody;
    if (args.QueryParameters !== undefined) w.query = args.QueryParameters;
    return w;
  }
  return { resource, args };
}

function wrap(resource, answer) {
  if (resource === "arn:aws:states:::lambda:invoke") return { Payload: answer, StatusCode: 200 };
  if (resource === "arn:aws:states:::http:invoke") return { ResponseBody: answer, StatusCode: 200 };
  if (resource === "arn:aws:states:::states:startExecution.sync:2") return { Output: answer, Status: "SUCCEEDED" };
  return answer;
}

let transitions = 0;

// Run states from `start` until a Succeed (its output comes back) or a Fail (Stop is thrown).
// At the top, a Succeed ends the execution; in a Map round, it ends the round.
async function runStates(all, start, input, context, top) {
  let name = start;
  for (;;) {
    if (++transitions > 20000) throw new Error("more than 20000 transitions");
    const st = all[name];
    if (!st) throw new Stop({ fail: { error: "States.Runtime", cause: `no state ${name}` } });
    const states = { input, context };
    let output = input;
    switch (st.Type) {
      case "Pass": {
        if (st.Assign) await assign(st.Assign, states);
        if (st.Output !== undefined) output = await evaluate(st.Output, states);
        name = st.Next;
        break;
      }
      case "Choice": {
        let to = st.Default;
        for (const c of st.Choices) {
          if ((await evaluate(c.Condition, states)) === true) {
            to = c.Next;
            break;
          }
        }
        if (to === undefined) throw new Stop({ fail: { error: "States.NoChoiceMatched", cause: name } });
        if (st.Assign) await assign(st.Assign, states);
        name = to;
        break;
      }
      case "Wait": {
        if (st.Seconds !== undefined) steps.push({ wait: await evaluate(st.Seconds, states) });
        else steps.push({ wait_until: await evaluate(st.Timestamp, states) });
        name = st.Next;
        break;
      }
      case "Succeed": {
        const out = st.Output !== undefined ? await evaluate(st.Output, states) : top ? null : input;
        if (top) throw new Stop({ succeed: out });
        return out;
      }
      case "Map": {
        const items = await evaluate(st.Items, states);
        if (!Array.isArray(items)) throw new Stop({ fail: { error: "States.Runtime", cause: `the Map's items are not a list` } });
        const results = [];
        const saved = { ...vars };
        const savedOuter = outer;
        for (let i = 0; i < items.length; i++) {
          const ctx = { ...context, Map: { Item: { Index: i, Value: items[i] } } };
          const sel = st.ItemSelector !== undefined ? await evaluate(st.ItemSelector, { input, context: ctx }) : items[i];
          // the round sees the variables outside it, and its own go when it ends
          vars = { ...saved };
          outer = new Set([...savedOuter, ...Object.keys(saved)]);
          try {
            results.push(await runStates(st.ItemProcessor.States, st.ItemProcessor.StartAt, sel, context, false));
          } finally {
            vars = { ...saved };
            outer = savedOuter;
          }
        }
        const s2 = { ...states, result: results };
        if (st.Assign) await assign(st.Assign, s2);
        output = st.Output !== undefined ? await evaluate(st.Output, s2) : results;
        name = st.Next;
        break;
      }
      case "Fail":
        throw new Stop({ fail: { error: await evaluate(st.Error, states), cause: st.Cause !== undefined ? await evaluate(st.Cause, states) : null } });
      case "Task": {
        const args = await evaluate(st.Arguments ?? {}, states);
        const w = wire(st.Resource, args);
        const retriers = st.Retry ?? [];
        const counts = retriers.map(() => 0);
        let result;
        let error = null;
        for (;;) {
          const ans = run.answers[next++];
          if (ans === undefined) throw new Error(`no answer for call ${next} (${name})`);
          if ("ok" in ans) {
            steps.push({ call: w, answer: { ok: ans.ok } });
            result = wrap(st.Resource, ans.ok);
            error = null;
            break;
          }
          steps.push({ call: w, answer: { error: ans.error, as: ans.as } });
          error = ans.as;
          let again = false;
          for (let i = 0; i < retriers.length; i++) {
            const r = retriers[i];
            if (matches(r.ErrorEquals, error)) {
              const max = r.MaxAttempts ?? 3;
              if (counts[i] < max) {
                steps.push({ retry_wait: (r.IntervalSeconds ?? 1) * Math.pow(r.BackoffRate ?? 2.0, counts[i]) });
                counts[i]++;
                again = true;
              }
              break;
            }
          }
          if (!again) break;
        }
        if (error === null) {
          const s2 = { ...states, result };
          if (st.Assign) await assign(st.Assign, s2);
          output = st.Output !== undefined ? await evaluate(st.Output, s2) : result;
          name = st.Next;
        } else {
          const errorOutput = { Error: error, Cause: "scripted" };
          const c = (st.Catch ?? []).find((c) => matches(c.ErrorEquals, error));
          if (!c) throw new Stop({ fail: { error, cause: "scripted" } });
          if (c.Assign) await assign(c.Assign, { ...states, errorOutput });
          output = c.Output !== undefined ? await evaluate(c.Output, { ...states, errorOutput }) : errorOutput;
          name = c.Next;
        }
        break;
      }
      default:
        throw new Error(`the runner does not run ${st.Type} states`);
    }
    input = output;
  }
}

async function main() {
  const context = { Execution: { Name: execution }, Task: { Token: "token" } };
  await runStates(def.States, def.StartAt, run.input, context, true);
}

try {
  await main();
} catch (e) {
  if (e instanceof Stop) {
    process.stdout.write(JSON.stringify({ steps, end: e.end }, null, 2) + "\n");
  } else {
    process.stderr.write(String(e.stack ?? e) + "\n");
    process.exit(2);
  }
}
