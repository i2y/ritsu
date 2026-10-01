// Reads what a rule's Connect service answered with the code dandori writes for it in TypeScript
// (io.ts's `rule`, which sends through a Transport, here a stand-in that gives the answer) and in
// JSONata (the expression a state machine reads an HTTP Task's answer with), and writes down what
// each read. tests/examples.rs gives it answers the services would not give, as well as ones they
// do, and holds both readings to the reference's (`render::rule_read`).
//
//   node read.mjs <io.ts> <cases.json> <results.json>
//
// cases.json:   [ { "wire": <the RuleWire of the rule>, "body": <the answer>, "jsonata": "<the expression, reading $body>" } ]
// results.json: [ { "ts": <what io.ts's rule gives, or {"threw": …}>, "jsonata": <what the expression gives, or {"threw": …}> } ]

import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import jsonata from "jsonata";

const [ioFile, casesFile, outFile] = process.argv.slice(2);
const { rule } = await import(pathToFileURL(path.resolve(ioFile)).href);
const cases = JSON.parse(fs.readFileSync(casesFile, "utf8"));

// what the state machine sees: plain JSON, with nothing undefined
const plain = (x) => (x === undefined ? { undefined: true } : JSON.parse(JSON.stringify(x)));
const fail = (kind, message) => {
  throw new Error(`${kind}: ${message}`);
};

const results = [];
for (const c of cases) {
  const r = {};
  try {
    r.ts = plain(await rule({ http: async () => ({ status: 200, body: c.body }) }, c.wire, {}, fail));
  } catch (e) {
    r.ts = { threw: String(e.message ?? e) };
  }
  try {
    r.jsonata = plain(await jsonata(c.jsonata).evaluate({}, { body: c.body }));
  } catch (e) {
    r.jsonata = { threw: String(e.message ?? e) };
  }
  results.push(r);
}
fs.writeFileSync(outFile, JSON.stringify(results));
