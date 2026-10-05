// Runs scenarios through the TypeScript clients chobo writes, for tests/backends.rs:
//
//   node runner.ts <input.json>
//
// The input names the database, and for each book the client file and the scenarios, each with
// a tenant of its own. Every scenario of every book runs at once; within a scenario the steps
// run in order, and the callers of a `together` at the same time, each through a connection (or
// a TigerBeetle client) of its own. What comes out, on standard output, is for each scenario its
// result as `chobo run --format json` writes one (PLAN 0.3), and every request the client sent,
// operation by operation, for the test to compare with `chobo run --show`.
//
// `pass` waits until the expiry of every hold the scenario has made is past, then, on
// PostgreSQL, calls the book's expire(); on TigerBeetle, it reads the accounts again until what
// they hold is what the holds that never expire still hold (10 seconds at most).
//
// The reference lets no time pass but at a `pass`, and the holds expire after a few seconds of
// real time: a scenario with a step or a read that long after a hold it made since its last
// `pass` is `late`. The reference does not say what that run answers, and the test runs the
// scenario again when it disagrees.

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import pg from "pg";
import { createClient } from "tigerbeetle-node";
import type { Account, Client, Transfer } from "tigerbeetle-node";

type Json = any;
type Input = {
  backend: "postgres" | "tigerbeetle";
  postgres?: { host: string; port: number; database: string; user: string };
  tigerbeetle?: { cluster: string; addresses: string[] };
  books: Array<{ name: string; client: string; expiry: number; keys: Record<string, string[]>; scenarios: ScenarioIn[] }>;
};
type ScenarioIn = {
  tenant: string;
  steps: Json[];
  accounts: Array<{ account: string; args: string[]; params: Record<string, string> }>;
  holds: Array<{ kind: string; key: string[]; args: Record<string, string> }>;
};

const input: Input = JSON.parse(readFileSync(process.argv[2], "utf8"));
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const hex = (v: bigint) => v.toString(16).padStart(32, "0");

const ACCOUNT_FLAGS = ["linked", "debits_must_not_exceed_credits", "credits_must_not_exceed_debits", "history", "imported", "closed"];
const TRANSFER_FLAGS = ["linked", "pending", "post_pending_transfer", "void_pending_transfer", "balancing_debit", "balancing_credit", "closing_debit", "closing_credit", "imported"];
const flagNames = (names: string[], bits: number) => names.filter((_, i) => bits & (1 << i));

function accountJson(a: Account): Json {
  return { id: hex(a.id), ledger: a.ledger, code: a.code, flags: flagNames(ACCOUNT_FLAGS, a.flags), user_data_128: hex(a.user_data_128), user_data_64: Number(a.user_data_64), user_data_32: a.user_data_32 };
}

function transferJson(x: Transfer): Json {
  return {
    id: hex(x.id),
    debit_account_id: hex(x.debit_account_id),
    credit_account_id: hex(x.credit_account_id),
    amount: x.amount.toString(),
    pending_id: hex(x.pending_id),
    user_data_128: hex(x.user_data_128),
    user_data_64: Number(x.user_data_64),
    user_data_32: x.user_data_32,
    timeout: x.timeout,
    ledger: x.ledger,
    code: x.code,
    flags: flagNames(TRANSFER_FLAGS, x.flags),
  };
}

/** A value the clients pass PostgreSQL, as the test compares it: an amount as its decimal digits. */
const param = (v: unknown) => (typeof v === "bigint" || typeof v === "number" ? String(v) : v === undefined ? null : v);

/** The requests of one operation, written down as the client sends them. */
class Recorder {
  current: Json[] | null = null;
}

let pool: pg.Pool | null = null;
let expiring: Promise<unknown> = Promise.resolve();
const clients: Client[] = [];
let next = 0;

function connection(rec: Recorder): unknown {
  if (input.backend === "postgres") {
    return {
      query: (text: string, values?: unknown[]) => {
        rec.current?.push({ sql: text, params: (values ?? []).map(param) });
        return pool!.query(text, values);
      },
    };
  }
  const c = clients[next++ % clients.length];
  return {
    createAccounts: (batch: Account[]) => {
      rec.current?.push({ create_accounts: batch.map(accountJson) });
      return c.createAccounts(batch);
    },
    createTransfers: (batch: Transfer[]) => {
      rec.current?.push({ create_transfers: batch.map(transferJson) });
      return c.createTransfers(batch);
    },
    lookupAccounts: (ids: bigint[]) => {
      rec.current?.push({ lookup_accounts: ids.map(hex) });
      return c.lookupAccounts(ids);
    },
    lookupTransfers: (ids: bigint[]) => {
      rec.current?.push({ lookup_transfers: ids.map(hex) });
      return c.lookupTransfers(ids);
    },
  };
}

/** Numbers in the scenario are amounts: the clients take them as bigint. */
function amounts(o: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(o).map(([k, v]) => [k, typeof v === "number" ? BigInt(v) : v]));
}

type Pending = { debit: string; amount: bigint; timeout: number };

async function runScenario(mod: Json, book: Input["books"][number], sc: ScenarioIn): Promise<Json> {
  const factory = input.backend === "postgres" ? mod.postgres : mod.tigerbeetle;
  const sent: Json[] = [];
  const doneHolds: number[] = [];
  // when the first hold since the last `pass` was asked for, and whether a step or a read came
  // after its expiry
  let heldSince: number | null = null;
  let late = false;
  const outlived = () => {
    if (heldSince !== null && Date.now() - heldSince >= book.expiry * 1000) late = true;
  };
  // on TigerBeetle: what each hold of the scenario still holds, by the hold's kind and key
  const pending = new Map<string, Pending[]>();
  const debited = new Set<string>();
  const holdKey = (kind: string, args: Record<string, unknown>) => kind + JSON.stringify(book.keys[kind].map((k) => args[k]));

  const caller = () => {
    const rec = new Recorder();
    return { rec, book: factory(connection(rec), { tenant: sc.tenant }) };
  };
  const main = caller();

  async function call(c: { rec: Recorder; book: Json }, op: Json, step: number, callerNo: number | null): Promise<Json> {
    const requests: Json[] = [];
    c.rec.current = requests;
    const asked = Date.now();
    let r: Json;
    try {
      const calls = c.book[op.kind];
      if (op.op === "do") r = await calls.do(amounts(op.args));
      else if (op.op === "hold") r = await calls.hold(amounts(op.args));
      else if (op.op === "post") r = await calls.post(op.args, op.amounts === undefined ? undefined : amounts(op.amounts));
      else r = await calls.void(op.args);
    } finally {
      c.rec.current = null;
    }
    const at: Json = { step };
    if (callerNo !== null) at.caller = callerNo;
    sent.push({ ...at, op: op.op, kind: op.kind, requests });
    if (r.result === "done" && op.op === "hold") {
      doneHolds.push(Date.now());
      heldSince ??= asked;
      const chain = requests.filter((q) => q.create_transfers).at(-1)?.create_transfers ?? [];
      const ps: Pending[] = chain
        .filter((x: Json) => x.flags.includes("pending") && x.user_data_32 !== 2)
        .map((x: Json) => ({ debit: x.debit_account_id, amount: BigInt(x.amount), timeout: x.timeout }));
      for (const p of ps) debited.add(p.debit);
      pending.set(holdKey(op.kind, op.args), ps);
    }
    if (r.result === "done" && (op.op === "post" || op.op === "void")) pending.delete(holdKey(op.kind, op.args));
    const out: Json = { op: op.op, kind: op.kind, result: r.result };
    if (r.result === "refused") out.reason = r.reason;
    return out;
  }

  async function pass(): Promise<void> {
    outlived();
    await expireAll();
    heldSince = null;
  }

  async function expireAll(): Promise<void> {
    const until = Math.max(0, ...doneHolds) + book.expiry * 1000 + 300;
    await sleep(Math.max(0, until - Date.now()));
    if (input.backend === "postgres") {
      // one expire() at a time, as one job would call it: an expire() skips the holds another is
      // giving back, and returns before that one commits
      const mine = expiring.then(() => main.book.expire());
      expiring = mine.catch(() => {});
      await mine;
      return;
    }
    for (const [k, ps] of pending) {
      const left = ps.filter((p) => p.timeout === 0);
      if (left.length === ps.length) continue;
      if (left.length === 0) pending.delete(k);
      else pending.set(k, left);
    }
    const want = new Map<string, bigint>([...debited].map((d) => [d, 0n]));
    for (const ps of pending.values()) for (const p of ps) want.set(p.debit, want.get(p.debit)! + p.amount);
    const ids = [...debited];
    for (let tries = 0; ; tries++) {
      const found = ids.length === 0 ? [] : await clients[0].lookupAccounts(ids.map((d) => BigInt("0x" + d)));
      if (found.every((a) => a.debits_pending === want.get(hex(a.id)))) return;
      if (tries > 200) throw new Error(`the holds past their expiry were not given back within 10 seconds`);
      await sleep(50);
    }
  }

  const steps: Json[] = [];
  for (const [i, step] of sc.steps.entries()) {
    if (step.op === "pass") {
      await pass();
      steps.push({ op: "pass" });
    } else if (step.op === "together") {
      const callers = step.callers.map(() => caller());
      const outs = await Promise.all(
        step.callers.map(async (ops: Json[], ci: number) => {
          const rs: Json[] = [];
          for (const op of ops) rs.push(await call(callers[ci], op, i + 1, ci + 1));
          return rs;
        }),
      );
      steps.push({ op: "together", callers: outs });
    } else {
      steps.push(await call(main, step, i + 1, null));
    }
  }
  const accounts: Json[] = [];
  for (const a of sc.accounts) {
    const fn = main.book.balance[a.account];
    const b = Object.keys(a.params).length === 0 ? await fn() : await fn(a.params);
    accounts.push({ account: a.account, args: a.args, posted: Number(b.posted), held_in: Number(b.held_in), held_out: Number(b.held_out) });
  }
  const holds: Json[] = [];
  for (const h of sc.holds) {
    const state = await main.book[h.kind].status(h.args);
    if (state !== null) holds.push({ kind: h.kind, key: h.key, state });
  }
  outlived();
  return { tenant: sc.tenant, result: { steps, accounts, holds }, sent, late, error: null };
}

async function main(): Promise<void> {
  if (input.backend === "postgres") {
    const c = input.postgres!;
    pool = new pg.Pool({ host: c.host, port: c.port, database: c.database, user: c.user, max: 20 });
  } else {
    const c = input.tigerbeetle!;
    for (let i = 0; i < 4; i++) clients.push(createClient({ cluster_id: BigInt(c.cluster), replica_addresses: c.addresses }));
  }
  try {
    const books = await Promise.all(
      input.books.map(async (book) => {
        const mod = await import(pathToFileURL(book.client).href);
        const scenarios = await Promise.all(
          book.scenarios.map((sc) => runScenario(mod, book, sc).catch((e: unknown) => ({ tenant: sc.tenant, result: null, sent: [], error: String((e as Error)?.stack ?? e) }))),
        );
        return { name: book.name, scenarios };
      }),
    );
    process.stdout.write(JSON.stringify({ books }) + "\n");
  } finally {
    await pool?.end();
    for (const c of clients) c.destroy();
  }
}

await main();
