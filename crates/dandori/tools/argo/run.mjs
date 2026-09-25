// Runs a workflow that dandori generated for Argo Workflows on the kind cluster that
// tools/argo/setup.sh sets up, against scripted answers, and prints what it did in the shape
// the reference interpreter prints for Argo: every call with its arguments and the answer it
// got, and how the workflow ended; and with each run, how many nodes its Workflow made.
//
//   node tools/argo/run.mjs <generated dir> <runs.json> <results.json>
//
// runs.json: { "template": <the WorkflowTemplate as JSON>, "own": [ { "name", "callback", "declared" } ],
//              "children": [name], "http": [...], "aws": [...],
//              "runs": [ { "input": {...}, "answers": [ {"ok": value} | {"error": kind} ] } ] }
//
// The tasks that say `lambda`, `http` or `aws` run the generated caller (caller/ of the build)
// in node:24-alpine, with a transport.ts that takes its answers from the mock in the cluster
// (tools/argo/mock.mjs). The tasks with `image`, the rules and the workflows a task starts are
// stand-ins (tools/argo/stand-in.mjs). A wait is resumed at once; a callback's wait gets the
// scenario's answer with `argo node set` and is resumed. A task's own timeout cannot be
// scripted this way, so runs.json holds none of those. The rounds of `for … in parallel` run
// one at a time here, so that the calls come in the order the reference interpreter makes them.
//
// The argo command is DANDORI_ARGO, else `argo` on the PATH.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { execFile, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const run = promisify(execFile);
const here = path.dirname(fileURLToPath(import.meta.url));
const [dir, runsFile, outFile] = process.argv.slice(2);
const spec = JSON.parse(fs.readFileSync(runsFile, "utf8"));
const argo = process.env.DANDORI_ARGO ?? "argo";
const context = "kind-dandori";
const ns = "argo";

async function kubectl(args, input) {
  if (input === undefined) return (await run("kubectl", ["--context", context, "-n", ns, ...args], { maxBuffer: 1 << 28 })).stdout;
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

async function argoCli(args) {
  return (await run(argo, ["--context", context, "-n", ns, ...args], { maxBuffer: 1 << 24 })).stdout;
}

// the mock, through `kubectl proxy` and the API server (a port-forward came apart under the
// runs' calls); the proxy is started again when a call cannot reach it
let proxy = null;
let base = null;
async function startProxy() {
  const p = spawn("kubectl", ["--context", context, "proxy", "--port=0"]);
  proxy = p;
  p.on("close", () => {
    if (proxy === p) proxy = null;
  });
  base = await new Promise((resolve, reject) => {
    p.stdout.on("data", (d) => {
      const m = /127\.0\.0\.1:(\d+)/.exec(String(d));
      if (m) resolve(`http://127.0.0.1:${m[1]}/api/v1/namespaces/${ns}/services/dandori-mock:80/proxy`);
    });
    p.on("close", () => reject(new Error("kubectl proxy ended")));
  });
}
async function mock(pathname, body) {
  for (let attempt = 1; ; attempt++) {
    try {
      if (proxy === null) await startProxy();
      const res = await fetch(`${base}${pathname}`, body === undefined ? {} : { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
      return await res.json();
    } catch (e) {
      if (attempt >= 5) throw e;
      proxy?.kill();
      proxy = null;
      await new Promise((r) => setTimeout(r, 1000 * attempt));
    }
  }
}

// how many nodes a workflow made; Argo compresses the list when the Workflow grows large
function nodeCount(w) {
  if (w.status?.nodes) return Object.keys(w.status.nodes).length;
  if (w.status?.compressedNodes) return Object.keys(JSON.parse(zlib.gunzipSync(Buffer.from(w.status.compressedNodes, "base64")).toString("utf8"))).length;
  return 0;
}

const wt = spec.template;
const name = wt.metadata.name;
const suffix = Math.random().toString(36).slice(2, 7);

// the code the pods run: the generated caller with the test transport, and the stand-ins
const code = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-argo-"));
for (const f of fs.readdirSync(path.join(dir, "caller"))) {
  if (f.endsWith(".ts")) fs.copyFileSync(path.join(dir, "caller", f), path.join(code, f));
}
fs.copyFileSync(path.join(here, "pod-transport.ts"), path.join(code, "transport.ts"));
fs.copyFileSync(path.join(here, "..", "transport.mjs"), path.join(code, "transport.mjs"));
fs.copyFileSync(path.join(here, "stand-in.mjs"), path.join(code, "stand-in.mjs"));
const declared = {};
for (const t of spec.own ?? []) declared[t.name] = t.declared ?? [];
fs.writeFileSync(path.join(code, "spec.json"), JSON.stringify({ http: spec.http ?? [], aws: spec.aws ?? [], declared }));
const cm = `dandori-code-${name}`.slice(0, 60);
const files = fs.readdirSync(code).flatMap((f) => ["--from-file", `${f}=${path.join(code, f)}`]);
await kubectl(["apply", "-f", "-"], await kubectl(["create", "configmap", cm, ...files, "--dry-run=client", "-o", "json"]));

// the template, with the stand-ins in place of the containers, one round at a time
const own = new Map((spec.own ?? []).map((t) => [t.name, t]));
const mount = { name: "dandori-code", mountPath: "/app" };
for (const t of wt.spec.templates) {
  if ((t.steps ?? []).some((group) => group.some((s) => s.withSequence !== undefined))) t.parallelism = 1;
  const c = t.container;
  if (!c || !c.env) continue;
  const task = c.env.find((e) => e.name === "DANDORI_TASK")?.value;
  if (task === undefined) continue;
  const cmd = c.command ?? [];
  if (cmd[1] === "/app/call.ts" && String(cmd[2]).startsWith("rule_")) c.command = ["node", "/app/stand-in.mjs", "rule", task];
  else if (cmd[1] !== "/app/call.ts") c.command = ["node", "/app/stand-in.mjs", own.get(task)?.callback ? "callback" : "task", task];
  c.image = "node:24-alpine";
  c.imagePullPolicy = "IfNotPresent";
  c.volumeMounts = [mount];
}
wt.spec.volumes = [{ name: "dandori-code", configMap: { name: cm } }];
await kubectl(["apply", "-f", "-"], JSON.stringify(wt));
// the workflows a task starts: stand-ins that answer with dd_output, or fail
for (const child of spec.children ?? []) {
  const stub = {
    apiVersion: "argoproj.io/v1alpha1",
    kind: "WorkflowTemplate",
    metadata: { name: child },
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

// the runs, a few at a time
const names = spec.runs.map((_, i) => `dd-${name}-${suffix}-${i + 1}`.slice(0, 60));
const ends = new Array(spec.runs.length);
const nodes = new Array(spec.runs.length).fill(0);
const waiting = [];
let nextRun = 0;
const running = new Set();
const answered = new Set();

// a Workflow from the template, made with kubectl: `argo submit --from` checks the template on
// this side first, which takes seconds for a template of hundreds of templates
async function submit(i) {
  const wf = names[i];
  await mock("/scenario", { workflow: wf, answers: spec.runs[i].answers });
  const workflow = {
    apiVersion: "argoproj.io/v1alpha1",
    kind: "Workflow",
    metadata: { name: wf },
    spec: { workflowTemplateRef: { name }, arguments: { parameters: [{ name: "input", value: JSON.stringify(spec.runs[i].input) }] } },
  };
  await kubectl(["create", "-f", "-"], JSON.stringify(workflow));
  running.add(i);
}

async function answer(wf, callbackId, ans) {
  const value = "ok" in ans ? JSON.stringify({ ok: ans.ok }) : ans.error === "timeout" ? "dandori:timeout" : JSON.stringify({ error: ans.error === "failure" ? "Dandori.Test.Failure" : ans.error, message: "scripted" });
  const sel = `inputs.parameters.callback_id.value=${callbackId}`;
  await argoCli(["node", "set", wf, "--output-parameter", `answer=${value}`, "--node-field-selector", sel]);
  await argoCli(["resume", wf, "--node-field-selector", sel]);
  await mock("/answered", { workflow: wf, callback_id: callbackId });
}

const started = Date.now();
while (running.size > 0 || nextRun < spec.runs.length) {
  while (running.size < 12 && nextRun < spec.runs.length) await submit(nextRun++);
  await new Promise((r) => setTimeout(r, 1500));
  const list = JSON.parse(await kubectl(["get", "workflows", "-o", "json"]));
  for (const w of list.items) {
    const i = names.indexOf(w.metadata.name);
    if (i < 0 || !running.has(i)) continue;
    const phase = w.status?.phase;
    if (phase === "Succeeded" || phase === "Failed" || phase === "Error") {
      running.delete(i);
      nodes[i] = nodeCount(w);
      const params = Object.fromEntries((w.status.outputs?.parameters ?? []).map((p) => [p.name, p.value]));
      if (phase === "Succeeded") ends[i] = { succeed: JSON.parse(params.dd_output ?? "null") };
      else if (params.dd_error && params.dd_error !== "null") {
        const e = JSON.parse(params.dd_error);
        ends[i] = { fail: { error: e.Error, cause: e.Cause ?? null } };
      } else ends[i] = { fail: { error: `Argo.${phase}`, cause: w.status.message ?? null } };
      continue;
    }
    for (const n of Object.values(w.status?.nodes ?? {})) {
      if (n.type !== "Suspend" || n.phase !== "Running") continue;
      const key = `${w.metadata.name}/${n.id}`;
      if (answered.has(key)) continue;
      const id = (n.inputs?.parameters ?? []).find((p) => p.name === "callback_id")?.value;
      if (id === undefined) {
        // a wait: go on at once
        answered.add(key);
        await argoCli(["resume", w.metadata.name, "--node-field-selector", `id=${n.id}`]);
        continue;
      }
      const state = await mock(`/state?workflow=${encodeURIComponent(w.metadata.name)}`);
      const ans = state.pending?.[id];
      if (ans === undefined) continue;
      answered.add(key);
      await answer(w.metadata.name, id, ans);
    }
  }
  const limit = Number(process.env.DANDORI_ARGO_MINUTES ?? 45);
  if (Date.now() - started > limit * 60 * 1000) throw new Error(`the runs took more than ${limit} minutes: ${[...running].map((i) => names[i]).join(", ")}`);
}

const results = [];
for (let i = 0; i < spec.runs.length; i++) {
  const state = await mock(`/state?workflow=${encodeURIComponent(names[i])}`);
  // the keys carry the workflow's name; the reference names the execution "test"
  const steps = JSON.parse(JSON.stringify(state.steps).split(`"${names[i]}/`).join('"test/'));
  results.push({ steps, end: ends[i], nodes: nodes[i] });
}
// DANDORI_ARGO_KEEP=1 keeps the workflows, to look at them
if (!process.env.DANDORI_ARGO_KEEP) {
  for (const wf of names) await argoCli(["delete", wf]).catch(() => {});
  // and the workflows their tasks started
  if ((spec.children ?? []).length > 0) await kubectl(["delete", "workflows", "-l", `dandori-parent in (${names.join(",")})`]).catch(() => {});
}
proxy?.kill();
fs.rmSync(code, { recursive: true, force: true });
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
process.exit(0);
