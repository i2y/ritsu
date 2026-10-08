// Sends decision tasks' requests for real — to TypeSafe's API, OpenAI's Decisions API, or a server
// of the System One API such as Ollama — through the default Transport that dandori writes for
// TypeScript (io.ts), and reads each answer with io.jev, as the generated activities do. The
// Transport adds the key the case names: TypeSafe's from TYPESAFE_API_KEY, OpenAI's from
// OPENAI_API_KEY, or none. For each case it writes down the status, the response's body, and what
// io.jev made of it: the value of the task's type, or the error it failed the call with (the
// task's own, for an answer less sure than the task asks, or for a refused question).
//
//   node tools/jev/check.mjs <io.ts> <cases.json> <results.json>
//
// cases.json: [ { "request": <HttpRequest, as the call is rendered>, "spec": <JevTask>,
//                 "key": "typesafe" | "openai" | null (TypeSafe's when left out) } ]

import fs from "node:fs";

const [ioFile, casesFile, outFile] = process.argv.slice(2);
const io = await import(ioFile);
const cases = JSON.parse(fs.readFileSync(casesFile, "utf8"));
const transport = io.transport();

function fail(kind, message) {
  const e = new Error(message);
  e.name = kind;
  throw e;
}

const results = [];
for (const c of cases) {
  const began = Date.now();
  const key = c.key === undefined ? "typesafe" : c.key;
  const res = await transport.http({ ...c.request, ...(key ? { [key]: true } : {}) });
  const r = { status: res.status, body: res.body, ms: Date.now() - began };
  if (res.status >= 200 && res.status < 300) {
    try {
      r.value = io.jev(res.body, c.spec, fail);
    } catch (e) {
      r.error = { kind: e.name, message: e.message };
    }
  }
  results.push(r);
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
