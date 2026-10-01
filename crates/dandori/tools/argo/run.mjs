// Runs the workflows dandori generated for Argo Workflows on the kind cluster that
// tools/argo/setup.sh sets up, against scripted answers, and prints what each did in the shape
// the reference interpreter prints for Argo: every call with its arguments and the answer it
// got, how the workflow ended, and how many nodes its Workflow made.
//
//   node tools/argo/run.mjs <generated dir> <runs.json> <results.json>
//
// runs.json: { "template": <the WorkflowTemplate as JSON>, "own": [ { "name", "callback", "declared" } ],
//              "children": [name], "connectRules": [the activity name of each rule called at its service],
//              "http": [...], "aws": [...],
//              "runs": [ { "input": {...}, "answers": [ {"ok": value} | {"error": kind} ] } ],
//              "real": [index of a run to play again with real pods] }
// results.json: { "runs": [ { steps, end, nodes, platform } ], "real": [ the same, for "real" ] }
//
// The Argo controller runs the generated WorkflowTemplate as it is, but this runner plays the
// pods of the runs. Their pods wait for a scheduler that does not exist, and the runner does what
// their containers and Argo's executor would do: it runs the generated caller (caller/call.ts)
// or the stand-in (stand-in.mjs) on this machine with the call Argo put in the pod, writes the
// outputs the template declares as the pod's WorkflowTaskResult, and ends the pod with the exit
// code and the termination message the container would leave. A resource step starts its
// workflow and waits for it, as the executor would; `dd-end` fails with its message. No
// container starts, so a call takes about a second, and every run goes at the same time. A
// played run's Workflow holds the template's spec itself: from a workflowTemplateRef, the
// controller's first validation of each Workflow reads the WorkflowTemplate from the API server
// again for every template it resolves, and a hundred runs of a large template swamp the API
// server. The runs with real pods are made from the WorkflowTemplate, as `argo submit --from` does.
//
// A task's `timeout` is the pod's `activeDeadlineSeconds`, which counts the pod's start; the copy of the template
// that runs here has 120 seconds more (see DEADLINE_ROOM), since no scenario runs a task into its timeout.
//
// The runs listed in `real` are played again with real pods: node:24-alpine running the caller
// with the test transport and the stand-ins, answered by the mock in the cluster
// (tools/argo/mock.mjs). A real run whose pod the platform could not run (Error, or Unknown with
// exit code 255: containerd in the kind node was seen to crash under load) is played again, at
// most twice.
//
// A wait goes on at once (its duration is set to 0); a callback's wait gets the scenario's answer
// with `argo node set` and is resumed. A task's own timeout cannot be scripted this way, so
// runs.json holds none of those. The rounds of `for … in parallel` run one at a time here, so
// that the calls come in the order the reference interpreter makes them.
//
// The argo command is DANDORI_ARGO, else `argo` on the PATH. DANDORI_ARGO_KEEP=1 keeps the
// workflows, to look at them; DANDORI_ARGO_MINUTES bounds the runs (10 minutes);
// DANDORI_ARGO_DUMP=<directory> writes, for every run with real pods, what the cluster says of its
// pods and nodes when it ends, and the logs of the pods that failed.

import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { execFile, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { mock as makeMock } from "./mock.mjs";

const run = promisify(execFile);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const here = path.dirname(fileURLToPath(import.meta.url));
const [dir, runsFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(runsFile, "utf8"));
const argo = process.env.DANDORI_ARGO ?? "argo";
const context = "kind-dandori";
const ns = "argo";
// the scheduler the played pods wait for, which the cluster does not have: they stay Pending
// until the runner ends them
const PLAYER = "dandori-played";
const PLAYED = JSON.stringify({ schedulerName: PLAYER });

function kubectlOnce(args, input) {
  if (input === undefined) return run("kubectl", ["--context", context, "-n", ns, ...args], { maxBuffer: 1 << 28 }).then((r) => r.stdout);
  return new Promise((resolve, reject) => {
    const p = spawn("kubectl", ["--context", context, "-n", ns, ...args]);
    let out = "";
    let err = "";
    p.stdout.on("data", (d) => (out += d));
    p.stderr.on("data", (d) => (err += d));
    p.on("close", (code) => (code === 0 ? resolve(out) : reject(new Error(err))));
    p.stdin.end(input);
  });
}

// An API server busy with many runs at once can time a request out; `apply`, `get` and `delete`
// come out the same when they are sent again, so they are, a few times, a little later each time.
const BUSY = /Timeout|timed out|deadline exceeded|unable to return a response|etcdserver|connection refused|i\/o timeout|EOF/i;
async function kubectl(args, input) {
  const again = ["apply", "get", "delete"].includes(args[0]);
  for (let attempt = 1; ; attempt++) {
    try {
      return await kubectlOnce(args, input);
    } catch (e) {
      const said = String(e?.stderr ?? e?.message ?? e);
      if (!again || attempt >= 5 || !BUSY.test(said)) throw e;
      await new Promise((r) => setTimeout(r, 1000 * attempt));
    }
  }
}

async function argoCli(args) {
  return (await run(argo, ["--context", context, "-n", ns, ...args], { maxBuffer: 1 << 24 })).stdout;
}

// The API server, and through it the mock in the cluster, by `kubectl proxy` (a port-forward came
// apart under the runs' calls); the proxy is started again when a request cannot reach it.
let proxy = null;
let api = null;
function startProxy() {
  const p = spawn("kubectl", ["--context", context, "proxy", "--port=0"]);
  const ready = new Promise((resolve, reject) => {
    p.stdout.on("data", (d) => {
      const m = /127\.0\.0\.1:(\d+)/.exec(String(d));
      if (m) resolve(`http://127.0.0.1:${m[1]}`);
    });
    p.on("close", () => reject(new Error("kubectl proxy ended")));
  });
  proxy = { p, ready };
  p.on("close", () => {
    if (proxy?.p === p) proxy = null;
  });
}

const JSON_BODY = { "content-type": "application/json" };

async function request(pathname, options = {}) {
  for (let attempt = 1; ; attempt++) {
    let res;
    try {
      if (proxy === null) startProxy();
      api = await proxy.ready;
      res = await fetch(`${api}${pathname}`, options);
    } catch (e) {
      if (attempt >= 5) throw e;
      proxy?.p.kill();
      proxy = null;
      await sleep(500 * attempt);
      continue;
    }
    const text = await res.text();
    // An API server busy with many runs at once answers 5xx or 429; every request here comes out
    // the same when it is sent again (an object is created under its own name, a status set), so
    // it is, a little later each time. A creation that went through the first time is taken from
    // the server when sending it again finds it there.
    if ((res.status >= 500 || res.status === 429) && attempt < 5) {
      await sleep(1000 * attempt);
      continue;
    }
    if (res.status === 409 && attempt > 1 && options.method === "POST") {
      const made = JSON.parse(options.body)?.metadata?.name;
      if (made) return request(`${pathname}/${made}`);
    }
    if (res.status === 404 && options.missingIsFine) return null;
    if (!res.ok) throw new Error(`${options.method ?? "GET"} ${pathname}: ${res.status} ${text.slice(0, 400)}`);
    return text ? JSON.parse(text) : null;
  }
}

const workflows = `/apis/argoproj.io/v1alpha1/namespaces/${ns}/workflows`;

// how many nodes a workflow made; Argo compresses the list when the Workflow grows large
function nodesOf(w) {
  if (w.status?.nodes) return w.status.nodes;
  if (w.status?.compressedNodes) return JSON.parse(zlib.gunzipSync(Buffer.from(w.status.compressedNodes, "base64")).toString("utf8"));
  return {};
}

// The pods the platform could not run: a pod whose init or wait container failed is Error, and a
// container the runtime lost ends Unknown with exit code 255. The containers of ours exit with 0,
// 1, 2 or 3. A real run with such a pod did not play its scenario.
function podErrors(w) {
  return Object.values(nodesOf(w))
    .filter((n) => n.type === "Pod" && (n.phase === "Error" || /Unknown \(exit code 255\)/.test(n.message ?? "")))
    .map((n) => `${w.metadata.name} ${n.displayName}: ${n.message ?? ""}`);
}

const wt = spec.template;
const name = wt.metadata.name;
// seconds added to the deadline of every pod of a template under test, for the pod to start in (see below)
const DEADLINE_ROOM = 120;
// how long a pod the runner played may stay un-noticed by the controller before its run is played again
const STUCK_MS = Number(process.env.DANDORI_ARGO_STUCK_SECONDS ?? 90) * 1000;
const suffix = Math.random().toString(36).slice(2, 7);
const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-argo-"));

// The code the real pods run: the generated caller with the test transport, and the stand-ins.
const code = path.join(work, "code");
fs.mkdirSync(code);
for (const f of fs.readdirSync(path.join(dir, "caller"))) {
  if (f.endsWith(".ts")) fs.copyFileSync(path.join(dir, "caller", f), path.join(code, f));
}
fs.copyFileSync(path.join(here, "pod-transport.ts"), path.join(code, "transport.ts"));
fs.copyFileSync(path.join(here, "..", "transport.mjs"), path.join(code, "transport.mjs"));
fs.copyFileSync(path.join(here, "stand-in.mjs"), path.join(code, "stand-in.mjs"));
const declared = {};
for (const t of spec.own ?? []) declared[t.name] = t.declared ?? [];
fs.writeFileSync(path.join(code, "spec.json"), JSON.stringify({ http: spec.http ?? [], aws: spec.aws ?? [], declared }));

// The same code for the pods this runner plays: the caller writes its answer and its
// termination message where DANDORI_OUT and DANDORI_TERMINATION say, not in the container's paths.
const played = path.join(work, "played");
fs.cpSync(code, played, { recursive: true });
const call = fs.readFileSync(path.join(played, "call.ts"), "utf8");
fs.writeFileSync(
  path.join(played, "call.ts"),
  call.replace(/"\/tmp\/dandori(\/[^"]*)?"/g, (_, rest) => `(process.env.DANDORI_OUT + "${rest ?? ""}")`).replace(/"\/dev\/termination-log"/g, "process.env.DANDORI_TERMINATION"),
);

// The runs: each one played, and the ones in `real` again with real pods.
const plays = [];
spec.runs.forEach((r, i) => plays.push({ i, real: false, base: `dd-${name}-${suffix}-${i + 1}`, name: `dd-${name}-${suffix}-${i + 1}`.slice(0, 60), platform: [] }));
for (const i of spec.real ?? []) plays.push({ i, real: true, base: `dd-${name}-${suffix}-real${i + 1}`, name: `dd-${name}-${suffix}-real${i + 1}`.slice(0, 60), platform: [] });
const byName = new Map(plays.map((p) => [p.name, p]));
// the names of the runs that were played again under another: what is left of them is not played
const replaced = new Set();
const made = plays.map((p) => p.name);

// the mock the played pods take their answers from, on this machine
const local = makeMock();
const server = http.createServer(local.handle);
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const localMock = `http://127.0.0.1:${server.address().port}`;

async function clusterMock(pathname, body) {
  return request(`/api/v1/namespaces/${ns}/services/dandori-mock:80/proxy${pathname}`, body === undefined ? {} : { method: "POST", headers: JSON_BODY, body: JSON.stringify(body) });
}

// what the mock wrote down for a run; the mock in the cluster is asked at most once a second a run
async function stateOf(p, fresh = false) {
  if (!p.real) {
    const r = local.runs.get(p.name);
    return r ? { steps: r.steps, pending: r.pending } : { steps: [], pending: {} };
  }
  if (fresh || p.state === undefined || Date.now() - p.asked >= 1000) {
    p.state = await clusterMock(`/state?workflow=${encodeURIComponent(p.name)}`);
    p.asked = Date.now();
  }
  return p.state;
}

async function answered(p, callbackId) {
  if (!p.real) delete local.runs.get(p.name)?.pending[callbackId];
  else await clusterMock("/answered", { workflow: p.name, callback_id: callbackId });
}

// the WorkflowTemplate, as generated, but for the stand-ins, the waits and the rounds
const own = new Map((spec.own ?? []).map((t) => [t.name, t]));
const mount = { name: "dandori-code", mountPath: "/app" };
for (const t of wt.spec.templates) {
  if ((t.steps ?? []).some((group) => group.some((s) => s.withSequence !== undefined))) t.parallelism = 1;
  // a wait goes on at once; a callback's wait is answered by the runner
  if (t.suspend && !(t.inputs?.parameters ?? []).some((p) => p.name === "callback_id")) t.suspend.duration = "0";
  // A pod's deadline (`activeDeadlineSeconds`, a task's `timeout`) runs from the moment the kubelet takes the pod up,
  // before its containers start: with every run at once, that is seconds in this cluster. A pod that ends by itself
  // just as the deadline passes is marked DeadlineExceeded, and Argo puts that message in the node in place of the
  // container's, in which the retryStrategy looks for the declared error's name: a `busy` is then not retried. No
  // scenario here runs a task into its timeout (they are left out), so the deadline is given room for the start.
  if (t.activeDeadlineSeconds !== undefined) t.activeDeadlineSeconds = Number(t.activeDeadlineSeconds) + DEADLINE_ROOM;
  const c = t.container;
  if (!c || !c.env) continue;
  const task = c.env.find((e) => e.name === "DANDORI_TASK")?.value;
  if (task === undefined) continue;
  const cmd = c.command ?? [];
  // a rule whose code goes with the workflow is a stand-in; one called at its Connect service runs the generated caller, which sends through the transport
  if (cmd[1] === "/app/call.ts" && String(cmd[2]).startsWith("rule_") && !(spec.connectRules ?? []).includes(cmd[2])) c.command = ["node", "/app/stand-in.mjs", "rule", task];
  else if (cmd[1] !== "/app/call.ts") c.command = ["node", "/app/stand-in.mjs", own.get(task)?.callback ? "callback" : "task", task];
  c.image = "node:24-alpine";
  c.imagePullPolicy = "IfNotPresent";
  c.volumeMounts = [mount];
}
const cm = `dandori-code-${name}`.slice(0, 60);
wt.spec.volumes = [{ name: "dandori-code", configMap: { name: cm } }];
if (plays.some((p) => p.real)) {
  const files = fs.readdirSync(code).flatMap((f) => ["--from-file", `${f}=${path.join(code, f)}`]);
  await kubectl(["apply", "-f", "-"], await kubectl(["create", "configmap", cm, ...files, "--dry-run=client", "-o", "json"]));
}
// the workflows a task starts: stand-ins that answer with dd_output, or fail, each under a name of
// this runner's, since another flow of the test may be the child itself, with its own template of
// that name in the cluster at the same time
const stubName = (child) => `${child}-${suffix}`;
for (const t of wt.spec.templates) {
  if (!t.resource?.manifest) continue;
  for (const child of spec.children ?? []) {
    t.resource.manifest = t.resource.manifest.replace(`workflowTemplateRef:\n    name: ${child}\n`, `workflowTemplateRef:\n    name: ${stubName(child)}\n`);
  }
}
await kubectl(["apply", "-f", "-"], JSON.stringify(wt));
for (const child of spec.children ?? []) {
  const stub = {
    apiVersion: "argoproj.io/v1alpha1",
    kind: "WorkflowTemplate",
    metadata: { name: stubName(child) },
    spec: {
      entrypoint: "main",
      arguments: { parameters: [{ name: "input", value: "{}" }] },
      volumes: [{ name: "dandori-code", configMap: { name: cm } }],
      templates: [
        {
          name: "main",
          container: {
            image: "node:24-alpine",
            imagePullPolicy: "IfNotPresent",
            command: ["node", "/app/stand-in.mjs", "child", child],
            env: [
              { name: "DANDORI_CALL", value: "{{workflow.parameters.input}}" },
              { name: "DANDORI_WORKFLOW", value: "{{workflow.labels.dandori-parent}}" },
            ],
            volumeMounts: [mount],
          },
          outputs: { parameters: [{ name: "output", globalName: "dd_output", valueFrom: { path: "/tmp/dandori/answer.json", default: "null" } }] },
        },
      ],
    },
  };
  await kubectl(["apply", "-f", "-"], JSON.stringify(stub));
}

// DANDORI_ARGO_DUMP: the nodes and the pods of a run with real pods, as the cluster has them when the run
// ends: each pod's phase and containers (exit code, reason, termination message), and the log of the
// main container of a pod that did not succeed.
async function dump(p, w) {
  const where = process.env.DANDORI_ARGO_DUMP;
  if (!where || !p.real) return;
  fs.mkdirSync(where, { recursive: true });
  const nodes = Object.values(nodesOf(w)).map((n) => ({ id: n.id, displayName: n.displayName, type: n.type, phase: n.phase, message: n.message, startedAt: n.startedAt, finishedAt: n.finishedAt, outputs: n.outputs }));
  let pods = [];
  try {
    const listed = JSON.parse(await kubectl(["get", "pods", "-l", `workflows.argoproj.io/workflow=${p.name}`, "-o", "json"]));
    pods = await Promise.all(
      listed.items.map(async (pod) => {
        const out = { name: pod.metadata.name, node: pod.metadata.annotations?.["workflows.argoproj.io/node-name"], created: pod.metadata.creationTimestamp, startTime: pod.status?.startTime, deadline: pod.spec?.activeDeadlineSeconds, conditions: pod.status?.conditions?.map((c) => `${c.type}=${c.status}@${c.lastTransitionTime}`), phase: pod.status?.phase, reason: pod.status?.reason, message: pod.status?.message, init: pod.status?.initContainerStatuses?.map((c) => ({ name: c.name, state: c.state })), containers: pod.status?.containerStatuses?.map((c) => ({ name: c.name, state: c.state })) };
        if (pod.status?.phase !== "Succeeded") out.log = await kubectl(["logs", pod.metadata.name, "-c", "main", "--tail=40"]).catch((e) => `no log: ${String(e?.message ?? e).slice(0, 200)}`);
        return out;
      }),
    );
  } catch (e) {
    pods = [{ error: String(e?.message ?? e).slice(0, 300) }];
  }
  const state = await stateOf(p, true).catch(() => null);
  const dumped = { workflow: p.name, run: p.i + 1, phase: w.status.phase, message: w.status.message, answers: spec.runs[p.i].answers.length, recorded: state?.steps?.length ?? null, nodes, pods };
  fs.writeFileSync(path.join(where, `${p.name}.json`), JSON.stringify(dumped, null, 2));
}

// A Workflow from the template, made through the API: `argo submit --from` checks the template on
// this side first, which takes seconds for a template of hundreds of templates.
async function submit(p) {
  const answers = spec.runs[p.i].answers;
  if (p.real) await clusterMock("/scenario", { workflow: p.name, answers });
  else local.runs.set(p.name, { answers, next: 0, steps: [], pending: {} });
  const input = { name: "input", value: JSON.stringify(spec.runs[p.i].input) };
  const w = {
    apiVersion: "argoproj.io/v1alpha1",
    kind: "Workflow",
    metadata: { name: p.name, labels: { "dandori-batch": suffix } },
    spec: p.real
      ? { workflowTemplateRef: { name }, arguments: { parameters: [input] }, podMetadata: { labels: { "dandori-batch": suffix } } }
      : {
          ...wt.spec,
          arguments: { parameters: (wt.spec.arguments?.parameters ?? []).map((q) => (q.name === "input" ? input : q)) },
          podMetadata: { labels: { "dandori-batch": suffix } },
          podSpecPatch: PLAYED,
        },
  };
  await request(workflows, { method: "POST", headers: JSON_BODY, body: JSON.stringify(w) });
  p.running = true;
}

// A played pod runs a program of ours on this machine: the caller or a stand-in. At most so
// many at a time, so that a hundred runs starting together do not starve each other.
let slots = Math.max(4, os.cpus().length);
const queue = [];
async function slot(f) {
  if (slots === 0) await new Promise((r) => queue.push(r));
  else slots--;
  try {
    return await f();
  } finally {
    const next = queue.shift();
    if (next) next();
    else slots++;
  }
}

function program(file, args, env) {
  return slot(() => {
    const out = fs.mkdtempSync(path.join(work, "pod-"));
    const termination = path.join(out, "termination-log");
    return new Promise((resolve) => {
      const p = spawn(process.execPath, ["--no-warnings", file, ...args], {
        cwd: played,
        env: { ...process.env, ...env, DANDORI_MOCK: localMock, DANDORI_OUT: out, DANDORI_TERMINATION: termination },
        stdio: ["ignore", "ignore", "pipe"],
      });
      let stderr = "";
      p.stderr.on("data", (d) => (stderr += d));
      p.on("close", (code) => {
        const read = (f) => {
          try {
            return fs.readFileSync(path.join(out, f), "utf8");
          } catch {
            return undefined;
          }
        };
        resolve({ code: code ?? 1, message: read("termination-log"), files: { "answer.json": read("answer.json"), "error.json": read("error.json") }, stderr });
      });
    });
  });
}

// A resource step: the workflow it creates (its pods played too), and its end by the success
// and failure conditions dandori writes, which read the workflow's phase.
async function resource(tmpl) {
  const r = tmpl.resource;
  if (r.action !== "create" || r.successCondition !== "status.phase == Succeeded" || r.failureCondition !== "status.phase in (Failed, Error)") {
    throw new Error(`the runner does not play this resource step: ${JSON.stringify(r).slice(0, 200)}`);
  }
  const child = JSON.parse(await kubectl(["create", "--dry-run=client", "-o", "json", "-f", "-"], r.manifest));
  child.metadata.labels = { ...(child.metadata.labels ?? {}), "dandori-batch": suffix };
  child.spec.podMetadata = { labels: { "dandori-batch": suffix } };
  child.spec.podSpecPatch = PLAYED;
  const got = await request(workflows, { method: "POST", headers: JSON_BODY, body: JSON.stringify(child) });
  const childName = got.metadata.name;
  made.push(childName);
  for (;;) {
    const w = await request(`${workflows}/${childName}`);
    const phase = w.status?.phase;
    if (phase === "Succeeded" || phase === "Failed" || phase === "Error") {
      const params = {};
      for (const p of tmpl.outputs?.parameters ?? []) {
        if (p.valueFrom?.jsonPath === undefined) continue;
        const v = await kubectl(["get", "workflow", childName, "-o", `jsonpath=${p.valueFrom.jsonPath}`]);
        if (v !== "") params[p.name] = v;
      }
      return phase === "Succeeded" ? { code: 0, files: {}, params } : { code: 1, message: `the workflow ${childName} ended ${phase}`, files: {}, params };
    }
    await sleep(200);
  }
}

// Play one pod: what its container would do, the outputs Argo's executor would report from it,
// and the pod's end.
async function play(pod) {
  const wf = pod.metadata.labels["workflows.argoproj.io/workflow"];
  const nodeId = pod.metadata.annotations["workflows.argoproj.io/node-id"];
  const containers = [...(pod.spec.initContainers ?? []), ...pod.spec.containers];
  const tmpl = JSON.parse(containers.flatMap((c) => c.env ?? []).find((e) => e.name === "ARGO_TEMPLATE").value);
  const main = pod.spec.containers.find((c) => c.name === "main");
  const env = Object.fromEntries((main.env ?? []).filter((e) => typeof e.value === "string").map((e) => [e.name, e.value]));
  const cmd = [...(main.command ?? []), ...(main.args ?? [])];
  const argv = cmd.includes("--") ? cmd.slice(cmd.indexOf("--") + 1) : cmd;
  let result;
  if (tmpl.resource) result = await resource(tmpl);
  else if (argv[0] === "node" && argv[1] === "/app/call.ts") result = await program(path.join(played, "call.ts"), argv.slice(2), env);
  else if (argv[0] === "node" && argv[1] === "/app/stand-in.mjs") result = await program(path.join(played, "stand-in.mjs"), argv.slice(2), env);
  else if (env.DD_ERROR !== undefined) result = { code: 1, message: env.DD_ERROR, files: {} };
  else if (argv.length === 1 && argv[0] === "true") result = { code: 0, files: {} };
  else throw new Error(`the runner does not play the container of ${pod.metadata.name}: ${argv.join(" ")}`);
  // the outputs the template declares, as the executor reports them: the declaration (with its
  // globalName) and its value, from the file, else the default
  const parameters = (tmpl.outputs?.parameters ?? []).map((p) => {
    let value = result.params?.[p.name];
    if (value === undefined && p.valueFrom?.path !== undefined) value = result.files[path.basename(p.valueFrom.path)];
    if (value === undefined) value = p.valueFrom?.default;
    return value === undefined ? { ...p } : { ...p, value };
  });
  const owner = (pod.metadata.ownerReferences ?? []).filter((o) => o.kind === "Workflow");
  await request(`/apis/argoproj.io/v1alpha1/namespaces/${ns}/workflowtaskresults`, {
    method: "POST",
    headers: JSON_BODY,
    body: JSON.stringify({
      apiVersion: "argoproj.io/v1alpha1",
      kind: "WorkflowTaskResult",
      metadata: { name: nodeId, labels: { "workflows.argoproj.io/workflow": wf, "workflows.argoproj.io/report-outputs-completed": "true" }, ownerReferences: owner },
      outputs: parameters.length > 0 ? { parameters } : {},
    }),
  });
  const now = new Date().toISOString().replace(/\.\d+Z$/, "Z");
  const ended = (c, exitCode, message) => ({
    name: c,
    image: "played",
    imageID: "",
    ready: false,
    restartCount: 0,
    started: false,
    state: { terminated: { exitCode, reason: exitCode === 0 ? "Completed" : "Error", ...(message ? { message } : {}), startedAt: now, finishedAt: now } },
  });
  const status = {
    phase: result.code === 0 ? "Succeeded" : "Failed",
    initContainerStatuses: (pod.spec.initContainers ?? []).map((c) => ended(c.name, 0)),
    containerStatuses: pod.spec.containers.map((c) => (c.name === "main" ? ended("main", result.code, result.message) : ended(c.name, 0))),
  };
  // a pod the controller has taken away since it was listed (the list comes from the API server's cache) has nothing to play
  await request(`/api/v1/namespaces/${ns}/pods/${pod.metadata.name}/status`, {
    method: "PATCH",
    headers: { "content-type": "application/merge-patch+json" },
    body: JSON.stringify({ status }),
    missingIsFine: true,
  });
  const run = byName.get(wf);
  if (run) {
    run.lastPlayed = Date.now();
    run.lastNode = nodeId;
  }
}

async function answer(p, callbackId, ans) {
  const value = "ok" in ans ? JSON.stringify({ ok: ans.ok }) : ans.error === "timeout" ? "dandori:timeout" : JSON.stringify({ error: ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, message: "scripted" });
  const sel = `inputs.parameters.callback_id.value=${callbackId}`;
  // The API server, busy with the callbacks of every run at once, times a request out now and then (the
  // Workflow of a flow that waits for a callback in each of its runs is large), and the request may have
  // gone through. `node set` that finds the answer set already says so, and that is that it went through;
  // a resume that went through shows in the wait having ended, which is looked for before it is sent again.
  const waiting = async () => {
    const w = await request(`${workflows}/${p.name}`, { missingIsFine: true });
    return w !== null && Object.values(nodesOf(w)).some((n) => n.type === "Suspend" && n.phase === "Running" && (n.inputs?.parameters ?? []).some((q) => q.name === "callback_id" && q.value === callbackId));
  };
  const said = (e) => String(e?.stderr ?? e?.message ?? e);
  for (let attempt = 1; ; attempt++) {
    try {
      await argoCli(["node", "set", p.name, "--output-parameter", `answer=${value}`, "--node-field-selector", sel]);
      break;
    } catch (e) {
      if (attempt > 1 && /already set/.test(said(e))) break;
      if (attempt >= 4 || !BUSY.test(said(e))) throw e;
      await sleep(1000 * attempt);
    }
  }
  for (let attempt = 1; ; attempt++) {
    try {
      await argoCli(["resume", p.name, "--node-field-selector", sel]);
      break;
    } catch (e) {
      if (attempt >= 4 || !BUSY.test(said(e))) throw e;
      await sleep(1000 * attempt);
      if (!(await waiting())) break;
    }
  }
  await answered(p, callbackId);
}

// the workflows this runner made go when it ends, also when it fails (unless they are kept)
async function clean() {
  if (process.env.DANDORI_ARGO_KEEP) return;
  await kubectl(["delete", "workflows", "-l", `dandori-batch=${suffix}`, "--wait=false"]).catch(() => {});
  // and the workflows the real runs' tasks started
  const real = made.filter((n) => byName.get(n)?.real);
  if ((spec.children ?? []).length > 0 && real.length > 0) await kubectl(["delete", "workflows", "-l", `dandori-parent in (${real.join(",")})`, "--wait=false"]).catch(() => {});
  // and the stand-ins of the workflows the tasks start
  if ((spec.children ?? []).length > 0) await kubectl(["delete", "workflowtemplates", ...spec.children.map(stubName), "--wait=false"]).catch(() => {});
}

// Start them all, then play the pods, answer the callbacks, and read the ends as they come.
const results = { runs: [], real: [] };
try {
  await Promise.all(plays.map((p) => submit(p)));
  const failures = [];
  const handled = new Set();
  const answering = new Set();
  const selector = encodeURIComponent(`dandori-batch=${suffix}`);
  const started = Date.now();
  const limit = Number(process.env.DANDORI_ARGO_MINUTES ?? 10);
  let pass = 0;
  while (plays.some((p) => p.running)) {
    if (failures.length > 0) throw failures[0];
    if (Date.now() - started > limit * 60 * 1000) {
      // what the runs that did not end were waiting for: their nodes that are not done, and their pods
      const lines = [];
      for (const p of plays.filter((p) => p.running)) {
        try {
          const w = await request(`${workflows}/${p.name}`);
          const waiting = Object.values(nodesOf(w))
            .filter((n) => !["Succeeded", "Skipped"].includes(n.phase))
            .map((n) => `${n.displayName} (${n.type}) ${n.phase}${n.message ? `: ${n.message}` : ""}`);
          const pods = (await kubectl(["get", "pods", "-l", `workflows.argoproj.io/workflow=${p.name}`, "--no-headers", "-o", "custom-columns=NAME:.metadata.name,PHASE:.status.phase,SCHEDULER:.spec.schedulerName,REASON:.status.reason"])).trim().split("\n").join("; ");
          lines.push(`${p.name}: ${w.status?.phase} ${w.status?.message ?? ""}\n  not done: ${waiting.join("; ")}\n  pods: ${pods || "none"}`);
        } catch (e) {
          lines.push(`${p.name}: ${String(e?.message ?? e).slice(0, 200)}`);
        }
      }
      throw new Error(`the runs took more than ${limit} minutes:\n${lines.join("\n")}`);
    }
    // the pods waiting for the played node: from the API server's cache, which is cheap, and once in a while from
    // the store itself, in case the cache is behind
    const fresh = pass % 50 === 49 ? "" : "resourceVersion=0&";
    const pods = await request(`/api/v1/namespaces/${ns}/pods?${fresh}labelSelector=${selector}&fieldSelector=${encodeURIComponent("status.phase=Pending")}`);
    for (const pod of pods.items ?? []) {
      if (pod.spec.schedulerName !== PLAYER || handled.has(pod.metadata.uid) || replaced.has(pod.metadata.labels["workflows.argoproj.io/workflow"])) continue;
      handled.add(pod.metadata.uid);
      play(pod).catch((e) => failures.push(e));
    }
    // A run whose last played pod the controller has not seen end for a while: in the whole test it has now and
    // then left the node Pending beside a pod that is done, and the run stood still for the 10 minutes it is given
    // (the nodes and the pod of such a run were what the run's error said). The run is played again under a new
    // name, once or twice, and the test says so.
    for (const p of plays) {
      if (!p.running || p.lastPlayed === undefined || p.checking || Date.now() - p.lastPlayed < STUCK_MS) continue;
      p.checking = true;
      (async () => {
        const w = await request(`${workflows}/${p.name}`, { missingIsFine: true });
        if (w === null) return;
        const node = nodesOf(w)[p.lastNode];
        const over = !node || ["Succeeded", "Failed", "Error", "Skipped", "Omitted"].includes(node.phase);
        if (over || !p.running || p.platform.length >= 2) {
          p.lastPlayed = Date.now();
          return;
        }
        p.platform.push(`the controller did not see the pod of ${node.displayName} end in ${STUCK_MS / 1000} seconds: the node stayed ${node.phase}`);
        replaced.add(p.name);
        await kubectl(["delete", "workflow", p.name, "--wait=false"]).catch(() => {});
        byName.delete(p.name);
        const tag = `-r${p.platform.length}`;
        p.name = p.base.slice(0, 60 - tag.length) + tag;
        p.lastPlayed = undefined;
        byName.set(p.name, p);
        made.push(p.name);
        await submit(p);
      })()
        .catch((e) => failures.push(e))
        .finally(() => {
          p.checking = false;
        });
    }
    // callbacks whose answer waits in the mock: answered once their wait is there, which is
    // looked for at most once a second a run (a Workflow is large, and the API server has many)
    for (const p of plays.filter((p) => p.running)) {
      const state = await stateOf(p);
      const ids = Object.keys(state.pending ?? {}).filter((id) => !answering.has(`${p.name}/${id}`));
      if (ids.length === 0 || Date.now() - (p.looked ?? 0) < 1000) continue;
      p.looked = Date.now();
      const w = await request(`${workflows}/${p.name}`, { missingIsFine: true });
      if (w === null) continue;
      for (const n of Object.values(nodesOf(w))) {
        if (n.type !== "Suspend" || n.phase !== "Running") continue;
        const id = (n.inputs?.parameters ?? []).find((q) => q.name === "callback_id")?.value;
        if (id === undefined || !ids.includes(id)) continue;
        answering.add(`${p.name}/${id}`);
        answer(p, id, state.pending[id]).catch((e) => failures.push(e));
      }
    }
    // the runs that ended, every few passes: their names only, by the label Argo puts on a finished workflow
    if (pass++ % 3 === 0) {
      const ended = await request(`${workflows}?resourceVersion=0&labelSelector=${encodeURIComponent(`dandori-batch=${suffix},workflows.argoproj.io/completed=true`)}`, {
        headers: { accept: "application/json;as=PartialObjectMetadataList;g=meta.k8s.io;v=v1" },
      });
      for (const item of ended.items ?? []) {
        const p = byName.get(item.metadata.name);
        if (!p || !p.running) continue;
        // a run that is played again meanwhile has no workflow of this name any more
        const w = await request(`${workflows}/${item.metadata.name}`, { missingIsFine: true });
        if (w === null || byName.get(item.metadata.name) !== p) continue;
        const errors = p.real ? podErrors(w) : [];
        if (errors.length > 0 && p.platform.length < 2) {
          await dump(p, w);
          // played again under a new name
          p.platform.push(...errors);
          replaced.add(p.name);
          byName.delete(p.name);
          const tag = `-r${p.platform.length}`;
          p.name = p.base.slice(0, 60 - tag.length) + tag;
          byName.set(p.name, p);
          made.push(p.name);
          await submit(p);
          continue;
        }
        await dump(p, w);
        p.running = false;
        p.nodes = Object.keys(nodesOf(w)).length;
        const params = Object.fromEntries((w.status.outputs?.parameters ?? []).map((q) => [q.name, q.value]));
        if (w.status.phase === "Succeeded") p.end = { succeed: JSON.parse(params.dd_output ?? "null") };
        else if (params.dd_error && params.dd_error !== "null") {
          const e = JSON.parse(params.dd_error);
          p.end = { fail: { error: e.Error, cause: e.Cause ?? null } };
        } else p.end = { fail: { error: `Argo.${w.status.phase}`, cause: w.status.message ?? null } };
      }
    }
    await sleep(100);
  }

  async function result(p) {
    const state = await stateOf(p, true);
    // the keys carry the workflow's name; the reference names the execution "test"
    const steps = JSON.parse(JSON.stringify(state.steps).split(`"${p.name}/`).join('"test/'));
    return { steps, end: p.end, nodes: p.nodes, platform: p.platform };
  }
  for (const p of plays) (p.real ? results.real : results.runs).push(await result(p));
} catch (e) {
  process.stderr.write(`${e.stack ?? e}\n`);
  await clean();
  proxy?.p.kill();
  process.exit(1);
}

await clean();
proxy?.p.kill();
server.close();
fs.rmSync(work, { recursive: true, force: true });
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
process.exit(0);
