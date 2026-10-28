// Runs the limits of a rule's call on Step Functions (the tests of the limits, tests/limits): the
// state machine dandori generated, on LocalStack with its Lambda, which runs each function in a
// container of its own (docker's socket is lent to LocalStack). In place of each Lambda function a
// run times out, a function that waits 15 seconds before it answers, past the 10 seconds of the
// Task's TimeoutSeconds. LocalStack's mock file cannot do this: it cannot throw an error whose name
// begins with States., and a mocked answer takes no time (see run.mjs).
//
//   node tools/localstack/limits.mjs <spec.json> <results.json>
//
// spec.json:    { "image": LocalStack's image, "definition": the state machine,
//                 "functions": [the ARN of each Lambda function that waits],
//                 "runs": [ { "id", "input" } ] }
// results.json: { "runs": [ { "steps", "end", "tries": [ { "started", "ended" } ], "late": n } ] }
//
// The definition runs as it is generated, but for its HTTP Tasks: LocalStack 4.14.0 does not know
// the HTTP Task, and refuses a definition that has one, so those get the resource run.mjs gives
// them, and no run here goes through them. Every run is an execution of its own, and they go at
// once. The calls and the end are read from the execution's history, in the shape the reference
// interpreter prints; a retry's wait, from the end of one try to the start of the next; and each
// try, from its start to its end, in seconds from the start of the execution. LocalStack writes the
// answer that a timed-out try gets later (TaskSucceeded) into the history too, after the timeout,
// and so in the middle of the next try: an answer that comes sooner after the start of the try
// going on than the function gives one, or when none is going on, belongs to no try (`late`
// counts them).
//
// The function and the state machine are made in the account and the region of the function's
// ARN, which LocalStack takes from the access key and the region the requests are signed for. The
// runner says on stderr how far it got, and takes LocalStack and the containers of its Lambda
// away at the end.

import fs from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { wire } from "../asl-run.mjs";

const exec = promisify(execFile);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const [specFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(specFile, "utf8"));

const HTTP = "arn:aws:states:::http:invoke";
const STAND_IN = "arn:aws:states:::aws-sdk:sts:getCallerIdentity";
/** How long the function waits before it answers. */
const WAITS_MS = 15000;
const container = `dandori-limits-${process.pid}`;
const began = Date.now();

function say(what) {
  process.stderr.write(`${((Date.now() - began) / 1000).toFixed(1)}s ${what}\n`);
}

async function docker(args) {
  return (await exec("docker", args, { maxBuffer: 1 << 26 })).stdout.trim();
}

// the account and the region of the first function's ARN: arn:aws:lambda:<region>:<account>:function:<name>
const [, , , region, account] = spec.functions[0].split(":");
let endpoint;

/** A request signed for the account and the region, as far as LocalStack reads a signature. */
function auth(service) {
  const day = new Date().toISOString().slice(0, 10).replaceAll("-", "");
  return `AWS4-HMAC-SHA256 Credential=${account}/${day}/${region}/${service}/aws4_request, SignedHeaders=host, Signature=0`;
}

async function call(service, method, url, headers, body) {
  const res = await fetch(endpoint + url, { method, headers: { authorization: auth(service), ...headers }, body, signal: AbortSignal.timeout(5 * 60 * 1000) });
  const text = await res.text();
  if (!res.ok) throw new Error(`${service} ${method} ${url}: ${res.status} ${text.slice(0, 2000)}`);
  return text ? JSON.parse(text) : {};
}

function sfn(op, body) {
  return call("states", "POST", "", { "content-type": "application/x-amz-json-1.0", "x-amz-target": `AWSStepFunctions.${op}` }, JSON.stringify(body));
}

function lambda(method, path, body) {
  return call("lambda", method, `2015-03-31/functions${path}`, { "content-type": "application/json" }, body === undefined ? undefined : JSON.stringify(body));
}

// A zip of one file, stored (Lambda takes the code of a function as a zip)
const CRC = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function zipOf(name, text) {
  const data = Buffer.from(text);
  const file = Buffer.from(name);
  const crc = crc32(data);
  const local = Buffer.alloc(30);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(data.length, 18);
  local.writeUInt32LE(data.length, 22);
  local.writeUInt16LE(file.length, 26);
  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(data.length, 20);
  central.writeUInt32LE(data.length, 24);
  central.writeUInt16LE(file.length, 28);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(1, 8);
  end.writeUInt16LE(1, 10);
  end.writeUInt32LE(central.length + file.length, 12);
  end.writeUInt32LE(local.length + file.length + data.length, 16);
  return Buffer.concat([local, file, data, central, file, end]);
}

const HANDLER = `export const handler = async () => {\n  await new Promise((r) => setTimeout(r, ${WAITS_MS}));\n  return {};\n};\n`;

// A run as the history tells it, in the shape the reference interpreter prints.
function trace(events) {
  const t0 = events[0].timestamp;
  const steps = [];
  const tries = [];
  let end = null;
  let late = 0;
  let task = null;
  // the try going on, and when the last one ended
  let going = null;
  let tried = null;
  for (const e of events) {
    switch (e.type) {
      case "TaskStateEntered":
        task = spec.states[e.stateEnteredEventDetails.name];
        tried = null;
        break;
      case "TaskScheduled": {
        if (tried !== null) steps.push({ retry_wait: Math.round((e.timestamp - tried) * 1000) / 1000 });
        tried = null;
        going = { step: { call: wire(task.Resource, JSON.parse(e.taskScheduledEventDetails.parameters)), answer: null }, started: null };
        steps.push(going.step);
        break;
      }
      case "TaskStarted":
        if (going) going.started = e.timestamp - t0;
        break;
      case "TaskSucceeded":
      case "TaskFailed":
      case "TaskTimedOut": {
        // an answer sooner than the function gives one is a late answer of an earlier try
        if (!going || (e.type !== "TaskTimedOut" && e.timestamp - t0 - going.started < WAITS_MS / 1000 - 0.5)) {
          late++;
          break;
        }
        if (e.type === "TaskSucceeded") going.step.answer = { ok: JSON.parse(e.taskSucceededEventDetails.output).Payload ?? null };
        else if (e.type === "TaskTimedOut") going.step.answer = { error: "timeout", as: e.taskTimedOutEventDetails.error };
        else going.step.answer = { error: e.taskFailedEventDetails.error, as: e.taskFailedEventDetails.error };
        tries.push({ started: going.started, ended: e.timestamp - t0 });
        going = null;
        tried = e.timestamp;
        break;
      }
      case "ExecutionSucceeded":
        end = { succeed: JSON.parse(e.executionSucceededEventDetails.output ?? "null") };
        break;
      case "ExecutionFailed": {
        const d = e.executionFailedEventDetails;
        end = { fail: { error: d.error ?? null, cause: d.cause ?? null } };
        break;
      }
    }
  }
  return { steps, end, tries, late };
}

async function main() {
  await docker(["run", "-d", "--rm", "--name", container, "-p", "127.0.0.1::4566", "-v", "/var/run/docker.sock:/var/run/docker.sock", "-e", "SERVICES=stepfunctions,lambda", "-e", "EAGER_SERVICE_LOADING=1", "-e", "DISABLE_EVENTS=1", spec.image]);
  endpoint = `http://${await docker(["port", container, "4566/tcp"])}/`;
  for (let i = 0; ; i++) {
    try {
      const health = await (await fetch(`${endpoint}_localstack/health`)).json();
      if (["stepfunctions", "lambda"].every((s) => ["available", "running"].includes(health.services?.[s]))) break;
    } catch {
      // not up yet
    }
    if (i > 600) throw new Error("LocalStack did not come up in a minute");
    await sleep(100);
  }
  say(`started ${spec.image} at ${endpoint}`);

  // the functions that wait, made in the account of their ARNs; LocalStack fetches the runtime's image first
  for (const arn of spec.functions) {
    const name = arn.split(":").at(-1);
    await lambda("POST", "", { FunctionName: name, Runtime: "nodejs22.x", Role: `arn:aws:iam::${account}:role/dandori`, Handler: "index.handler", Code: { ZipFile: zipOf("index.mjs", HANDLER).toString("base64") }, Timeout: 60 });
    for (let i = 0; ; i++) {
      const got = await lambda("GET", `/${encodeURIComponent(name)}`);
      if (got.Configuration?.State === "Active") break;
      if (got.Configuration?.State === "Failed") throw new Error(`the function ${name} failed: ${got.Configuration.StateReasonCode} ${got.Configuration.StateReason}`);
      if (i > 3000) throw new Error(`the function ${name} is not active after five minutes`);
      await sleep(100);
    }
    say(`made the function ${name}`);
  }

  const definition = JSON.parse(JSON.stringify(spec.definition));
  spec.states = {};
  const walk = (states) => {
    for (const [name, st] of Object.entries(states)) {
      spec.states[name] = JSON.parse(JSON.stringify(st));
      if (st.Type === "Task" && st.Resource === HTTP) st.Resource = STAND_IN;
      if (st.Type === "Map") walk(st.ItemProcessor.States);
    }
  };
  walk(definition.States);
  const { stateMachineArn } = await sfn("CreateStateMachine", { name: "dandori-limits", definition: JSON.stringify(definition), roleArn: `arn:aws:iam::${account}:role/dandori`, type: "STANDARD" });

  const results = await Promise.all(
    spec.runs.map(async (r) => {
      const { executionArn } = await sfn("StartExecution", { stateMachineArn, name: r.id, input: JSON.stringify(r.input) });
      for (let i = 0; ; i++) {
        const d = await sfn("DescribeExecution", { executionArn });
        if (d.status !== "RUNNING") break;
        if (i > 1800) throw new Error(`${r.id} has not ended after three minutes`);
        await sleep(100);
      }
      // the late answers of the tries that timed out come after the end; they are waited for, to be counted
      await sleep(WAITS_MS);
      const events = [];
      let nextToken;
      do {
        const page = await sfn("GetExecutionHistory", { executionArn, maxResults: 1000, ...(nextToken ? { nextToken } : {}) });
        events.push(...page.events);
        nextToken = page.nextToken;
      } while (nextToken);
      return trace(events);
    }),
  );
  say(`played ${results.length} run(s)`);
  fs.writeFileSync(outFile, JSON.stringify({ runs: results }, null, 2) + "\n");
}

async function clean() {
  await docker(["rm", "-f", container]).catch(() => {});
  // the containers LocalStack's Lambda started, which it names after its own
  const left = await docker(["ps", "-aq", "--filter", `name=${container}-lambda`]).catch(() => "");
  if (left) await docker(["rm", "-f", ...left.split("\n")]).catch(() => {});
}

try {
  await main();
} catch (e) {
  process.stderr.write(`${e.stack ?? e}\n`);
  await clean();
  process.exit(1);
}
await clean();
process.exit(0);
