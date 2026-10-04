// Runs the operations of books of chobo's through the default Transport that dandori writes in
// TypeScript (io.ts's `book`), on the TypeScript clients chobo writes, against PostgreSQL or
// TigerBeetle, for tests/examples.rs (books_run_on_postgres_and_tigerbeetle). The Python twin is
// run.py; the Go one the test writes itself, as it builds it with the packages it calls.
//
//   node run.ts <input.json>
//
// The test copies this file beside the clients, where node_modules is chobo's tools/runner's (pg
// and tigerbeetle-node), as chobo's own runner finds them.
//
// input.json:
//   { "backend": "postgres" | "tigerbeetle",
//     "postgres"?: { "host", "port", "database", "user" }, "tigerbeetle"?: { "cluster", "addresses" },
//     "runs": [ { "io": the io.ts of a flow, "clients": { book's name: its client },
//                 "tenant", "calls": [ the operations, as the reference interpreter shows them ] } ] }
// What comes out, on standard output: { "runs": [ { "answers": [ what transport.book answered, or
// { "error" } ] } ] }. The runs go at once, each on a tenant of its own; a run's calls one after
// another.

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import pg from "pg";
import { createClient } from "tigerbeetle-node";

type Json = any;
const input: Json = JSON.parse(readFileSync(process.argv[2], "utf8"));

let pool: Json = null;
let client: Json = null;
if (input.backend === "postgres") {
  const c = input.postgres;
  pool = new pg.Pool({ host: c.host, port: c.port, database: c.database, user: c.user, max: 10 });
} else {
  const c = input.tigerbeetle;
  client = createClient({ cluster_id: BigInt(c.cluster), replica_addresses: c.addresses });
}

async function run(r: Json): Promise<Json> {
  const io = await import(pathToFileURL(r.io).href);
  const books: Record<string, unknown> = {};
  for (const [name, file] of Object.entries(r.clients as Record<string, string>)) {
    const mod = await import(pathToFileURL(file).href);
    books[name] = pool !== null ? mod.postgres(pool, { tenant: r.tenant }) : mod.tigerbeetle(client, { tenant: r.tenant });
  }
  const transport = io.transport({ books });
  const answers: Json[] = [];
  for (const call of r.calls) {
    try {
      answers.push(await transport.book(call));
    } catch (e) {
      answers.push({ error: String((e as Error)?.stack ?? e) });
    }
  }
  return { answers };
}

try {
  const runs = await Promise.all(input.runs.map(run));
  process.stdout.write(JSON.stringify({ runs }) + "\n");
} finally {
  await pool?.end();
  client?.destroy();
}
