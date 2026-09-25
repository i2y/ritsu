// Runs in a pod of a workflow under test, in place of a task with `image`, a rule, or the
// workflow a task with `workflow template` starts: it writes the call down at the mock
// (tools/argo/mock.mjs), takes the scenario's answer, and ends the way the container
// protocol says (answer.json and exit 0; error.json and exit 3 for a declared error; exit 1).
//
//   node stand-in.mjs task|callback|rule|child <name>

import fs from "node:fs";

const [kind, name] = process.argv.slice(2);
const call = JSON.parse(process.env.DANDORI_CALL ?? "{}");
const workflow = process.env.DANDORI_WORKFLOW;
const spec = JSON.parse(fs.readFileSync(new URL("./spec.json", import.meta.url), "utf8"));

let view;
let callbackId;
if (kind === "callback") {
  const { callback_id, ...rest } = call;
  callbackId = callback_id;
  view = { callback: name, args: rest };
} else if (kind === "rule") {
  view = { rule: name, args: call };
} else if (kind === "child") {
  view = { workflow_template: name, args: call };
} else {
  view = { task: name, args: call };
}

const res = await fetch("http://dandori-mock.argo.svc/call", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ workflow, call: view, callback_id: callbackId ?? null }),
});
const got = await res.json();
if (!res.ok) {
  process.stderr.write(`${got.error}\n`);
  process.exit(2);
}
const ans = got.answer;
fs.mkdirSync("/tmp/dandori", { recursive: true });
if (kind === "callback" || "ok" in ans) {
  fs.writeFileSync("/tmp/dandori/answer.json", JSON.stringify(kind === "callback" ? null : ans.ok));
  process.exit(0);
}
const declared = spec.declared?.[name] ?? [];
if (declared.includes(ans.error)) {
  fs.writeFileSync("/tmp/dandori/error.json", JSON.stringify({ error: ans.error, message: "scripted" }));
  try {
    fs.writeFileSync("/dev/termination-log", ans.error);
  } catch {
    // not in a container
  }
  process.exit(3);
}
fs.writeFileSync("/tmp/dandori/error.json", JSON.stringify({ error: "Dandori.Test.Failure", message: "scripted" }));
process.exit(1);
