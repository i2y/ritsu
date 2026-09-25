// Runs a state machine that dandori generated, with JSONata 2.0.6, against scripted
// answers, and prints what it did in the shape the reference interpreter prints:
// every call as it went out and the answer it got, every wait and retry wait, and how
// the execution ended. It runs only what dandori writes: Pass, Task, Choice, Wait,
// Succeed and Fail, with QueryLanguage JSONata, variables, Retry and Catch.
//
//   node tools/asl-run.mjs <definition.asl.json> <run.json> [execution name]
//
// run.json: { "input": {...}, "answers": [ {"ok": value} | {"error": kind, "as": ASL error name} ] }

import fs from "node:fs";
import jsonata from "jsonata";

const def = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const run = JSON.parse(fs.readFileSync(process.argv[3], "utf8"));
const execution = process.argv[4] ?? "test";

const vars = {};
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

async function assign(block, states) {
  // every expression reads the values as they were on entry; then all are set
  const values = {};
  for (const [k, x] of Object.entries(block)) values[k] = await evaluate(x, states);
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
  return answer;
}

async function main() {
  let name = def.StartAt;
  let input = run.input;
  const context = { Execution: { Name: execution }, Task: { Token: "token" } };
  for (let n = 0; n < 20000; n++) {
    const st = def.States[name];
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
      case "Succeed":
        throw new Stop({ succeed: st.Output !== undefined ? await evaluate(st.Output, states) : null });
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
  throw new Error("more than 20000 transitions");
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
