// Runs a handler that dandori generated for AWS Lambda durable functions, in the SDK's local
// test runner with time skipped, against scripted answers, and prints what it did in the
// shape the reference interpreter prints for durable functions: every task (a step), every
// callback task (its submit) with its arguments, every rule (an invoke) with its payload,
// the answer each got, and how the execution ended.
//
//   node tools/durable/run.mjs <generated dir> <runs.json> <results.json>
//
// runs.json: { "tasks": [ { "name": n, "method": m, "callback": bool } ], "rules": [arn],
//              "runs": [ { "input": {...}, "answers": [ {"ok": value} | {"error": kind} ] } ] }
//
// A declared error is thrown as an Error with the error's name; "failure" as one with a name
// the workflow does not declare. A callback task's answer is sent with the callback's id. The
// local runner does not time a callback out, so runs.json holds no timeouts.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { LocalDurableTestRunner } from "@aws/durable-execution-sdk-js-testing";

const [dir, runsFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(runsFile, "utf8"));

// Node runs TypeScript by stripping the types, and wants the extension on a relative import.
const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-durable-"));
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith(".ts")) continue;
  const text = fs.readFileSync(path.join(dir, f), "utf8").replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`);
  fs.writeFileSync(path.join(work, f), text);
}
// the generated code imports the SDK; let it find the runner's copy
fs.symlinkSync(path.join(path.dirname(new URL(import.meta.url).pathname), "node_modules"), path.join(work, "node_modules"));
const { makeHandler } = await import(path.join(work, "workflow.ts"));

let current = null;
let runner = null;

function take(label) {
  const ans = current.answers[current.next++];
  if (ans === undefined) throw new Error(`no answer for call ${current.next} (${label})`);
  return ans;
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

const tasks = {};
for (const t of spec.tasks) {
  if (t.callback) {
    tasks[t.method] = async (args) => {
      const { callback_id, ...rest } = args;
      const ans = take(t.name);
      current.steps.push({ call: { callback: t.name, args: normalize(rest) }, answer: "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error } });
      setTimeout(() => deliver(callback_id, ans).catch((e) => process.stderr.write(`${e}\n`)), 0);
    };
  } else {
    tasks[t.method] = async (args) => {
      const ans = take(t.name);
      current.steps.push({ call: { task: t.name, args: normalize(args) }, answer: "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error } });
      if ("ok" in ans) return ans.ok;
      throw scriptedError(ans.error);
    };
  }
}

await LocalDurableTestRunner.setupTestEnvironment({ skipTime: true });
const results = [];
try {
  for (const run of spec.runs) {
    current = { answers: run.answers, next: 0, steps: [] };
    runner = new LocalDurableTestRunner({ handlerFunction: makeHandler(tasks) });
    for (const arn of spec.rules) {
      runner.registerFunction(arn, async (event) => {
        const ans = take(arn);
        current.steps.push({ call: { invoke: arn, payload: event }, answer: "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error } });
        if ("ok" in ans) return ans.ok;
        throw scriptedError(ans.error);
      });
    }
    const r = await runner.run({ payload: run.input });
    let end;
    if (r.getStatus() === "SUCCEEDED") {
      end = { succeed: r.getResult() ?? null };
    } else {
      const e = r.getError();
      end = { fail: { error: e?.errorType ?? String(r.getStatus()), cause: e?.errorMessage || null } };
    }
    results.push({ steps: current.steps, end });
  }
} finally {
  await LocalDurableTestRunner.teardownTestEnvironment();
  fs.rmSync(work, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
process.exit(0);
