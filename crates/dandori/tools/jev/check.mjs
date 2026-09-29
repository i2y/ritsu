// Sends Jev tasks' requests to TypeSafe's API for real, through the default Transport that dandori
// writes for TypeScript (io.ts), and reads each answer with io.jev, as the generated activities
// do. The Transport adds TypeSafe's key from TYPESAFE_API_KEY. For each case it writes down the
// status, the response's body, and what io.jev made of it: the value of the task's type, or the
// error it failed the call with (the task's own, for an answer less sure than the task asks).
//
//   node tools/jev/check.mjs <io.ts> <cases.json> <results.json>
//
// cases.json: [ { "request": <HttpRequest, as the call is rendered>, "spec": <JevTask> } ]

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
  const res = await transport.http({ ...c.request, typesafe: true });
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
