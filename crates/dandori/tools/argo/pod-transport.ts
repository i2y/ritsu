// transport.ts for the caller in a pod of a workflow under test: the stand-in transport
// (tools/transport.mjs), taking the scenario's answers from the mock (tools/argo/mock.mjs), in
// the cluster, or where DANDORI_MOCK says when the runner plays the pod on this machine.

import fs from "node:fs";
import { makeTransport } from "./transport.mjs";

const spec = JSON.parse(fs.readFileSync(new URL("./spec.json", import.meta.url), "utf8"));
const workflow = process.env.DANDORI_WORKFLOW;

export const transport = makeTransport(spec, {
  async take(call: unknown, callbackId?: string) {
    const res = await fetch(`${process.env.DANDORI_MOCK ?? "http://dandori-mock.argo.svc"}/call`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ workflow, call, callback_id: callbackId ?? null }),
    });
    const got = await res.json();
    if (!res.ok) throw new Error(got.error);
    return got.answer;
  },
  // the runner hands the callback's answer to the workflow
  answerLater() {},
});
