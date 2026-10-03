// Runs a state machine that dandori generated, with JSONata 2.0.6, against scripted
// answers, and prints what it did in the shape the reference interpreter prints:
// every call as it went out and the answer it got, every wait and retry wait, and how
// the execution ended. It runs only what dandori writes: Pass, Task, Choice, Wait,
// Succeed, Fail and inline Map, with QueryLanguage JSONata, variables, Retry and Catch.
//
// A Map's rounds run one after another. A round reads the variables outside it and keeps
// its own; assigning a variable an outer scope has is refused, as Step Functions refuses it.
//
// A Task whose Assign or Output cannot be evaluated fails with States.QueryEvaluationError,
// which its Retry and Catch take, as on Step Functions. An HTTP Task to OpenAI's Responses API
// or Claude's Messages API (an `agent` task) gets its answer as the API gives it, { "answer":
// <value> } as the text; a failure is played as the model refusing, with nothing to read.
//
//   node tools/asl-run.mjs <definition.asl.json> <run.json> [execution name]
//
// run.json: { "input": {...}, "answers": [ {"ok": value} | {"error": kind, "as": ASL error name} ] }
//
// It is a module too: `play` runs one execution and gives, besides what it prints, the Task
// state that made each call (tools/localstack/run.mjs mocks each state's answers with it), and
// `wire` and `wrap` are how a call is written down and how its answer comes back.

import fs from "node:fs";
import { fileURLToPath } from "node:url";
import jsonata from "jsonata";

class Stop extends Error {
  constructor(end) {
    super("stop");
    this.end = end;
  }
}

// $parse is Step Functions' own function
function expression(text) {
  const x = jsonata(text);
  x.registerFunction("parse", (s) => JSON.parse(s), "<s:x>");
  return x;
}

// JSONata hands back arrays with extra properties; the state machine sees plain JSON
function plain(x) {
  return x === undefined ? x : JSON.parse(JSON.stringify(x));
}

function matches(names, err) {
  return names.some((n) => n === err || (n === "States.ALL" && err !== "States.Runtime" && err !== "States.DataLimitExceeded") || (n === "States.TaskFailed" && err !== "States.Timeout"));
}

export function wire(resource, args) {
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

const OPENAI_URL = "https://api.openai.com/v1/responses";
const CLAUDE_URL = "https://api.anthropic.com/v1/messages";

// an agent's call: to OpenAI's Responses API or Claude's Messages API, or to another server of
// Open Responses, whose request is the Responses API's (a body with text.format)
function isAgent(resource, args) {
  if (resource !== "arn:aws:states:::http:invoke") return false;
  const openResponses = String(args.ApiEndpoint).endsWith("/responses") && args.RequestBody?.text?.format?.type === "json_schema";
  return args.ApiEndpoint === OPENAI_URL || args.ApiEndpoint === CLAUDE_URL || openResponses;
}

// the Responses API's answer: a reasoning item, then the message, whose content is `content`
function openaiResponse(content) {
  return {
    ResponseBody: {
      id: "resp_test",
      object: "response",
      status: "completed",
      output: [
        { id: "rs_test", type: "reasoning", summary: [] },
        { id: "msg_test", type: "message", role: "assistant", status: "completed", content: [content] },
      ],
    },
    StatusCode: 200,
    StatusText: "OK",
  };
}

// the Messages API's answer: the model's thinking (whose text the API leaves out), then the
// text, and how the turn ended; a refusal has no content, as the API reference shows it
function claudeResponse(model, text) {
  const refused = text === null;
  return {
    ResponseBody: {
      id: "msg_test",
      type: "message",
      role: "assistant",
      model,
      content: refused
        ? []
        : [
            { type: "thinking", thinking: "", signature: "test" },
            { type: "text", text },
          ],
      stop_reason: refused ? "refusal" : "end_turn",
      stop_sequence: null,
      stop_details: refused ? { type: "refusal", category: null, explanation: null } : null,
      usage: { input_tokens: 10, output_tokens: 10 },
    },
    StatusCode: 200,
    StatusText: "OK",
  };
}

// an agent's answer as its API gives it, or its refusal
function agentResponse(args, answer, refused) {
  if (args.ApiEndpoint === CLAUDE_URL) return claudeResponse(args.RequestBody.model, refused ? null : JSON.stringify({ answer }));
  return refused ? openaiResponse({ type: "refusal", refusal: "scripted" }) : openaiResponse({ type: "output_text", text: JSON.stringify({ answer }), annotations: [] });
}

export function wrap(resource, args, answer) {
  if (isAgent(resource, args)) return agentResponse(args, answer, false);
  if (resource === "arn:aws:states:::lambda:invoke") return { Payload: answer, StatusCode: 200 };
  if (resource === "arn:aws:states:::http:invoke") return { ResponseBody: answer, StatusCode: 200 };
  if (resource === "arn:aws:states:::states:startExecution.sync:2") return { Output: answer, Status: "SUCCEEDED" };
  return answer;
}

// Run one execution of `def` on `run`: what the reference interpreter prints of it (`steps` and
// `end`), and `calls`, for each call the Task state that made it, its resource and arguments,
// and the answer it got.
export async function play(def, run, execution = "test") {
  let vars = {};
  const steps = [];
  const calls = [];
  let next = 0;
  // the names the scopes around the current one assigned, which it must not assign
  let outer = new Set();
  let transitions = 0;

  async function evaluate(v, states) {
    if (typeof v === "string") {
      const m = /^\{%([\s\S]*)%\}$/.exec(v);
      if (!m) return v;
      let r;
      try {
        r = await expression(m[1]).evaluate({}, { ...vars, states });
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

  async function assign(block, states) {
    set(await values(block, states));
  }

  // every expression reads the values as they were on entry; then all are set
  async function values(block, states) {
    const out = {};
    for (const [k, x] of Object.entries(block)) out[k] = await evaluate(x, states);
    return out;
  }

  function set(values) {
    for (const k of Object.keys(values)) {
      if (outer.has(k)) throw new Stop({ fail: { error: "States.Runtime", cause: `a Map round assigns ${k}, a variable of an outer scope` } });
    }
    Object.assign(vars, values);
  }

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
          const out = st.Output !== undefined ? await evaluate(st.Output, states) : input;
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
          let error = null;
          let done;
          for (;;) {
            const ans = run.answers[next++];
            if (ans === undefined) throw new Error(`no answer for call ${next} (${name})`);
            let result;
            if ("ok" in ans) result = wrap(st.Resource, args, ans.ok);
            else if (isAgent(st.Resource, args) && ans.error === "failure") result = agentResponse(args, null, true);
            error = "ok" in ans || result !== undefined ? null : ans.as;
            if (result !== undefined) {
              // the answer is read into the variables and the output, or the Task fails
              try {
                const s2 = { ...states, result };
                done = { values: st.Assign ? await values(st.Assign, s2) : {}, output: st.Output !== undefined ? await evaluate(st.Output, s2) : result };
              } catch (e) {
                if (!(e instanceof Stop) || e.end.fail?.error !== "States.QueryEvaluationError") throw e;
                error = "States.QueryEvaluationError";
              }
            }
            steps.push({ call: w, answer: "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: error } });
            calls.push({ state: name, resource: st.Resource, args, answer: ans });
            if (error === null) break;
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
            set(done.values);
            output = done.output;
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

  const context = { Execution: { Name: execution }, Task: { Token: "token" } };
  try {
    await runStates(def.States, def.StartAt, run.input, context, true);
  } catch (e) {
    if (e instanceof Stop) return { steps, end: e.end, calls };
    throw e;
  }
}

if (fs.realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const def = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
  const run = JSON.parse(fs.readFileSync(process.argv[3], "utf8"));
  try {
    const { steps, end } = await play(def, run, process.argv[4] ?? "test");
    process.stdout.write(JSON.stringify({ steps, end }, null, 2) + "\n");
  } catch (e) {
    process.stderr.write(String(e.stack ?? e) + "\n");
    process.exit(2);
  }
}
