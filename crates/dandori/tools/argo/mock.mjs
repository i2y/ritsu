// The scenarios' answers, served to the calls of the workflows under test: inside the cluster to
// the pods (node mock.mjs listens on 8080), and on this machine to the pods the runner plays
// (it serves `mock()` itself). Each workflow (by name) takes the answers of its scenario in
// order; every call is written down with the answer it took, and a callback's answer waits here
// until the runner hands it to the workflow.
//
//   POST /scenario { workflow, answers }        set a workflow's answers
//   POST /call { workflow, call, callback_id }  write a call down and take the next answer
//   GET  /state?workflow=<name>                 { steps, pending: { <callback_id>: answer } }
//   POST /answered { workflow, callback_id }    the runner has answered the callback

import fs from "node:fs";
import http from "node:http";
import { fileURLToPath } from "node:url";

function recorded(ans) {
  return "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error };
}

function body(req) {
  return new Promise((resolve, reject) => {
    let text = "";
    req.on("data", (c) => (text += c));
    req.on("end", () => {
      try {
        resolve(text ? JSON.parse(text) : {});
      } catch (e) {
        reject(e);
      }
    });
  });
}

/** The mock: its request handler, and the runs it keeps, by workflow. */
export function mock() {
  const runs = new Map();
  const handle = async (req, res) => {
    const url = new URL(req.url, "http://mock");
    const send = (code, v) => {
      res.writeHead(code, { "content-type": "application/json" });
      res.end(JSON.stringify(v));
    };
    try {
      if (req.method === "POST" && url.pathname === "/scenario") {
        const b = await body(req);
        runs.set(b.workflow, { answers: b.answers, next: 0, steps: [], pending: {} });
        return send(200, { ok: true });
      }
      if (req.method === "POST" && url.pathname === "/call") {
        const b = await body(req);
        const r = runs.get(b.workflow);
        if (!r) return send(404, { error: `no scenario for ${b.workflow}` });
        const ans = r.answers[r.next++];
        if (ans === undefined) return send(409, { error: `no answer for call ${r.next} of ${b.workflow}` });
        r.steps.push({ call: b.call, answer: recorded(ans) });
        if (b.callback_id !== undefined && b.callback_id !== null) r.pending[b.callback_id] = ans;
        return send(200, { answer: ans });
      }
      if (req.method === "GET" && url.pathname === "/state") {
        const r = runs.get(url.searchParams.get("workflow"));
        return send(200, r ? { steps: r.steps, pending: r.pending } : { steps: [], pending: {} });
      }
      if (req.method === "POST" && url.pathname === "/answered") {
        const b = await body(req);
        const r = runs.get(b.workflow);
        if (r) delete r.pending[b.callback_id];
        return send(200, { ok: true });
      }
      send(404, { error: "no such path" });
    } catch (e) {
      send(500, { error: String(e) });
    }
  };
  return { handle, runs };
}

// run as a program (in the cluster, from a ConfigMap whose files are links), not imported
if (process.argv[1] && fs.realpathSync(process.argv[1]) === fs.realpathSync(fileURLToPath(import.meta.url))) {
  http.createServer(mock().handle).listen(8080);
}
