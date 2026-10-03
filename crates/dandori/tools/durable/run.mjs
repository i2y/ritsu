// Runs a handler that dandori generated for AWS Lambda durable functions, in the SDK's local
// test runner with time skipped, against scripted answers, and prints what it did in the
// shape the reference interpreter prints for durable functions: every call with its
// arguments, the answer each got, and how the execution ended.
//
//   node tools/durable/run.mjs <generated dir> <runs.json> <results.json>
//
// runs.json: { "own": [ { "name", "method", "callback" } ], "rules": [arn], "children": [arn],
//              "http": [...], "aws": [...],
//              "runs": [ { "input": {...}, "answers": [ {"ok": value} | {"error": kind} ] } ] }
//
// The tasks that say `lambda`, `http` or `aws` run the generated code, with a Transport that
// writes down what it would send (tools/transport.mjs). The tasks the user writes, the rules
// and the durable functions a task invokes are stand-ins that answer from the scenario. A
// callback's answer is sent with the callback's id. The local runner does not time a
// callback out, so runs.json holds no timeouts.
//
// The rounds of `for … in parallel` run one at a time here, so that the calls come in the
// order the reference interpreter makes them.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { LocalDurableTestRunner } from "@aws/durable-execution-sdk-js-testing";
import { withDurableExecution } from "@aws/durable-execution-sdk-js";
import { makeTransport } from "../transport.mjs";

const [dir, runsFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(runsFile, "utf8"));

// Node runs TypeScript by stripping the types, and wants the extension on a relative import.
const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-durable-"));
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith(".ts")) continue;
  let text = fs.readFileSync(path.join(dir, f), "utf8").replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`);
  if (f === "workflow.ts") text = text.replace(/dd\.atATime\(\d+\)/g, "dd.atATime(1)");
  fs.writeFileSync(path.join(work, f), text);
}
// the generated code imports the SDK; let it find the runner's copy
fs.symlinkSync(path.join(path.dirname(new URL(import.meta.url).pathname), "node_modules"), path.join(work, "node_modules"));
const { makeHandler } = await import(path.join(work, "workflow.ts"));
const { Failure } = await import(path.join(work, "runtime.ts"));

let current = null;
let runner = null;

function take(label) {
  const ans = current.answers[current.next++];
  if (ans === undefined) throw new Error(`no answer for call ${current.next} (${label})`);
  return ans;
}

function recorded(ans) {
  return "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error };
}

function scriptedError(kind) {
  const e = new Error("scripted");
  e.name = kind === "failure" ? "Dandori.Test.Failure" : kind;
  return e;
}

// the execution's ARN is the local runner's own; the reference names the execution "test"
function normalize(args) {
  const a = { ...args };
  if (typeof a.idempotency_key === "string") a.idempotency_key = a.idempotency_key.replace(/^[^/]*/, "test");
  return a;
}

const keyParams = new Set((spec.aws ?? []).map((t) => t.keyParam).filter(Boolean));

function normalizeCall(call) {
  if (call.payload) return { ...call, payload: normalize(call.payload) };
  if (call.headers && typeof call.headers["Idempotency-Key"] === "string") {
    return { ...call, headers: { ...call.headers, "Idempotency-Key": call.headers["Idempotency-Key"].replace(/^[^/]*/, "test") } };
  }
  if (call.aws && call.args) {
    const a = { ...call.args };
    for (const k of keyParams) if (typeof a[k] === "string") a[k] = a[k].replace(/^[^/]*/, "test");
    return { ...call, args: a };
  }
  if (call.args) return { ...call, args: normalize(call.args) };
  return call;
}

async function deliver(callbackId, ans) {
  // the local runner's client for the durable API, the one its operations use
  const api = runner.durableApi;
  for (let i = 0; i < 50; i++) {
    try {
      if ("ok" in ans) {
        await api.sendCallbackSuccess({ CallbackId: callbackId, Result: Buffer.from(JSON.stringify(ans.ok)) });
      } else {
        await api.sendCallbackFailure({ CallbackId: callbackId, Error: { ErrorType: ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, ErrorMessage: "scripted" } });
      }
      return;
    } catch (e) {
      await new Promise((r) => setTimeout(r, 10));
    }
  }
  throw new Error(`could not answer the callback ${callbackId}`);
}

function answerLater(callbackId, ans) {
  setTimeout(() => deliver(callbackId, ans).catch((e) => process.stderr.write(`${e}\n`)), 0);
}

// the stand-in transport writes every call down with the answer it takes; the calls carry the
// local runner's execution ARN in their keys
const run = {
  take: (call) => {
    const ans = take(JSON.stringify(call).slice(0, 80));
    current.steps.push({ call: normalizeCall(call), answer: recorded(ans) });
    return ans;
  },
  answerLater,
};

const own = {};
for (const t of spec.own ?? []) {
  if (t.callback) {
    own[t.method] = async (args) => {
      const { callback_id, ...rest } = args;
      const ans = take(t.name);
      current.steps.push({ call: { callback: t.name, args: normalize(rest) }, answer: recorded(ans) });
      answerLater(callback_id, ans);
    };
  } else {
    own[t.method] = async (args) => {
      const ans = take(t.name);
      current.steps.push({ call: { task: t.name, args: normalize(args) }, answer: recorded(ans) });
      if ("ok" in ans) return ans.ok;
      throw scriptedError(ans.error);
    };
  }
}

await LocalDurableTestRunner.setupTestEnvironment({ skipTime: true });
const results = [];
try {
  for (const r of spec.runs) {
    current = { answers: r.answers, next: 0, steps: [] };
    runner = new LocalDurableTestRunner({ handlerFunction: makeHandler(own, makeTransport(spec, run)) });
    for (const arn of spec.rules ?? []) {
      runner.registerFunction(arn, async (event) => {
        const ans = take(arn);
        current.steps.push({ call: { invoke: arn, payload: event }, answer: recorded(ans) });
        if ("ok" in ans) return ans.ok;
        throw scriptedError(ans.error);
      });
    }
    // a durable function a task invokes: it fails as a dandori workflow does, with the error in ErrorData
    for (const arn of spec.children ?? []) {
      runner.registerDurableFunction(
        arn,
        withDurableExecution(async (event) => {
          const ans = take(arn);
          current.steps.push({ call: { invoke: arn, payload: event }, answer: recorded(ans) });
          if ("ok" in ans) return ans.ok;
          throw new Failure(ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, "scripted");
        }),
      );
    }
    const out = await runner.run({ payload: r.input });
    let end;
    if (out.getStatus() === "SUCCEEDED") {
      end = { succeed: out.getResult() ?? null };
    } else {
      const e = out.getError();
      end = { fail: { error: e?.errorType ?? String(out.getStatus()), cause: e?.errorMessage || null } };
    }
    results.push({ steps: current.steps, end });
  }
} finally {
  await LocalDurableTestRunner.teardownTestEnvironment();
  fs.rmSync(work, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
process.exit(0);
