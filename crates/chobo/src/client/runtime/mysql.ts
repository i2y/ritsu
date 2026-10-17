import { createHash } from "node:crypto";
//@@BOOK@@
// ── What follows is the same in every book chobo writes for MySQL, TiDB and Vitess ───────
//
// A call is one transaction of the database's. The accounts it moves that have bounds are
// locked in the order of their IDs and read, and the read says, row by row, the first move
// whose bound it would break; the writes work the new balances out themselves. The SQL is the
// book's, written by chobo. This part fills in the values (IDs as hex, amounts as digits, the
// caller's strings as the hex of their UTF-8) and picks the next statement from what the last
// one answered.

/** What a client needs of a connection of mysql2's promise API: its `query`. */
export interface Queryable {
  query(options: { sql: string; rowsAsArray: boolean }): Promise<[unknown, unknown]>;
}

/** What a client needs of a pool of mysql2's promise API: a connection to run a call on. */
export interface PoolLike {
  getConnection(): Promise<Queryable & { release(): void }>;
}

/** Where a hold is: held, posted, voided, or expired. */
export type HoldState = "held" | "posted" | "voided" | "expired";

/** An account's balance: what is posted, what holds put in, and what holds take out. */
export type Balance = { posted: bigint; held_in: bigint; held_out: bigint };

type RefDef = { kind: string; args: Array<{ param: number } | { literal: string }> };
type MoveDef = { amount: { param: number } | { literal: bigint }; from: RefDef; to: RefDef };
type CheckDef = { n: number; reason: Reason; sql: string };
type TransferSql = {
  key: string;
  lock: string | null;
  checks: CheckDef[];
  refuse: string;
  entries: string;
  apply: string | null;
  postApply: string[];
  postEntries: string;
  voidApply: string[];
  voidEntries: string;
  expireApply: string[];
  expireEntries: string;
};
type TransferDef = {
  params: Array<{ name: string; amount: boolean }>;
  key: number[];
  pending: boolean;
  definition: string;
  moves: MoveDef[];
  sql: TransferSql;
};
type AccountDef = { params: string[]; bounded: boolean; lower: bigint | null; upper: bigint | null; row: string };
type BookSql = {
  begin: string;
  readAccounts: string;
  makeAccounts: string;
  readCall: string;
  holdLock: string;
  status: string;
  balance: string;
  due: string;
  foldList: string;
  foldLock: string;
  foldRead: string;
  foldApply: string;
  foldMark: string;
};
type BookDef = { name: string; accounts: Record<string, AccountDef>; transfers: Record<string, TransferDef>; sql: BookSql };

const AMOUNT_LIMIT = (1n << 63n) - 1n;
const ID_LIMIT = (1n << 128n) - 1n;
/** How many times a call of its own transaction is made when the database gives up on it: about 1.5 seconds of waits between them (DESIGN 4.5). */
const ATTEMPTS = 30;
/** The errors after which such a call is made again with the same arguments (the key keeps it from moving twice): a deadlock, a lock not got in time, and TiDB's write conflicts. */
const AGAIN = new Set([1213, 1205, 9007, 8022, 8028]);
const DUPLICATE = 1062;
/** The server's status in an answer: a transaction is open; autocommit is on. */
const IN_TRANS = 1;
const AUTOCOMMIT = 2;

/** An ID: the first 16 bytes of a SHA-256 over the parts, each with its UTF-8 length in front. */
function chId(...parts: string[]): bigint {
  const h = createHash("sha256");
  for (const p of ["chobo/1", ...parts]) {
    const bytes = Buffer.from(p, "utf8");
    const length = Buffer.alloc(4);
    length.writeUInt32BE(bytes.length);
    h.update(length);
    h.update(bytes);
  }
  const v = BigInt("0x" + h.digest().subarray(0, 16).toString("hex"));
  return v === 0n ? 1n : v === ID_LIMIT ? ID_LIMIT - 1n : v;
}

function hex(v: bigint): string {
  return v.toString(16).padStart(32, "0");
}

/** An ID as SQL. */
const idLit = (v: bigint): string => `X'${hex(v)}'`;
/** A string of the caller's as SQL: the hex of its UTF-8, read as utf8mb4, so that nothing in it is ever read as SQL. */
const strLit = (s: string): string => `_utf8mb4 X'${Buffer.from(s, "utf8").toString("hex")}'`;
/** IDs as an SQL list: each once, in the order of their hex. */
const idList = (ids: bigint[]): string => [...new Set(ids)].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0)).map(idLit).join(", ");
const jsonStrings = (vs: string[]): string => "[" + vs.map((v) => JSON.stringify(v)).join(",") + "]";

/** Put the values in a statement: each `{NAME}` by the value of NAME, in one pass. */
function fill(template: string, values: Record<string, string>): string {
  return template.replace(/\{([A-Z0-9_]+)\}/g, (m, name: string) => (name in values ? values[name] : m));
}

function str(name: string, v: unknown): string {
  if (typeof v !== "string") throw new TypeError(`chobo: ${name} is a string`);
  return v;
}

function amt(name: string, v: unknown): bigint {
  if (typeof v !== "bigint") throw new TypeError(`chobo: ${name} is an amount, a bigint`);
  if (v < 0n || v > AMOUNT_LIMIT) throw new RangeError(`chobo: ${name} is ${v}, and an amount is from 0 to 2^63 - 1`);
  return v;
}

function refused(reason: Reason): Result {
  return { result: "refused", reason };
}

/** What a `do` or `hold` is called with, as the key compares it: the arguments in the order of the parameters. */
function contentOf(t: TransferDef, v: unknown[]): string {
  return "{" + t.params.map((p, i) => JSON.stringify(p.name) + ":" + (p.amount ? String(v[i]) : JSON.stringify(v[i]))).join(",") + "}";
}

const errno = (e: unknown): number => Number((e as { errno?: unknown }).errno);

function backoff(attempt: number): Promise<void> {
  return new Promise((r) => setTimeout(r, Math.random() * 2 ** Math.min(attempt - 1, 7)));
}

type Rows = unknown[][];

/** The statements of one call on one connection: in a transaction of the call's own (`own`), or in the caller's. */
class Tx {
  readonly c: Queryable;
  readonly own: boolean;
  readonly batch: boolean;
  readonly begin: string;

  constructor(c: Queryable, own: boolean, batch: boolean, begin: string) {
    this.c = c;
    this.own = own;
    this.batch = batch;
    this.begin = begin;
  }

  /** Run `sql` in order and answer the rows of each: one request (`batch`), or one request a statement, stopping at the first error. */
  private async exec(sql: string[]): Promise<Rows[]> {
    if (sql.length === 0) return [];
    if (this.batch && sql.length > 1) {
      const [res] = await this.c.query({ sql: sql.join("; "), rowsAsArray: true });
      return (res as unknown[]).map((r) => (Array.isArray(r) ? (r as Rows) : []));
    }
    const out: Rows[] = [];
    for (const s of sql) {
      const [res] = await this.c.query({ sql: s, rowsAsArray: true });
      out.push(Array.isArray(res) ? (res as Rows) : []);
    }
    return out;
  }

  /** Statements outside the transaction: when the call has its own, before it starts. */
  plain(sql: string[]): Promise<Rows[]> {
    return this.exec(sql);
  }

  /** Statements of the transaction, which starts with the first of them when the call has its own, and ends after the last with `commit`. */
  async run(sql: string[], o: { begin?: boolean; commit?: boolean } = {}): Promise<Rows[]> {
    const start = o.begin && this.own ? [this.begin] : [];
    const end = o.commit && this.own ? ["commit"] : [];
    const rows = await this.exec([...start, ...sql, ...end]);
    return rows.slice(start.length, start.length + sql.length);
  }

  /** The end of a call that writes nothing more: its own transaction rolled back. */
  async quit(): Promise<void> {
    if (this.own) await this.exec(["rollback"]);
  }
}

type Placed = { id: bigint; kind: string; args: string[] };

class Runtime {
  readonly book: BookDef;
  readonly db: Queryable | PoolLike;
  readonly tenant: string;
  readonly batch: boolean;
  /** The accounts this value has made sure of in a transaction of a call's own, where their rows were committed at once: rows are never removed, nor their bounds changed. */
  readonly sure: Set<bigint> = new Set();
  private readonly base: Record<string, string>;

  constructor(book: BookDef, db: Queryable | PoolLike, tenant: string, batch: boolean) {
    this.book = book;
    this.db = db;
    this.tenant = str("the tenant", tenant);
    this.batch = batch;
    this.base = { TENANT: idLit(chId("tenant", book.name, this.tenant)), TENANT_NAME: strLit(this.tenant) };
  }

  /**
   * Run `fn` in a transaction of the call's own, made again with the same arguments when the
   * database gives up on it, or in the caller's. On a pool, the call takes a connection and the
   * transaction is its own. A connection the caller gives is asked first: the answer to
   * `set @chobo = 0` carries the server's status, and Vitess's vtgate answers it itself, without a
   * shard the caller's transaction would then be tied to. In a transaction (one the caller began,
   * or any with autocommit off), the call is part of it; else the call has a transaction of its
   * own on that connection.
   */
  private async tx<T>(fn: (x: Tx) => Promise<T>): Promise<T> {
    const db = this.db;
    let take: () => Promise<{ c: Queryable; done: () => void }>;
    if ("getConnection" in db) {
      take = async () => {
        const c = await db.getConnection();
        return { c, done: () => c.release() };
      };
    } else {
      const [answer] = await db.query({ sql: "set @chobo = 0", rowsAsArray: true });
      const status = (answer as { serverStatus?: unknown } | null)?.serverStatus;
      if (typeof status !== "number") throw new Error("chobo: the connection does not say whether it is in a transaction: the answer to `set @chobo = 0` has no serverStatus");
      if ((status & IN_TRANS) !== 0 || (status & AUTOCOMMIT) === 0) return fn(new Tx(db, false, this.batch, this.book.sql.begin));
      take = async () => ({ c: db, done: () => {} });
    }
    for (let attempt = 1; ; attempt++) {
      const { c, done } = await take();
      const x = new Tx(c, true, this.batch, this.book.sql.begin);
      try {
        return await fn(x);
      } catch (e) {
        await c.query({ sql: "rollback", rowsAsArray: true }).catch(() => {});
        if (AGAIN.has(errno(e)) && attempt < ATTEMPTS) {
          await backoff(attempt);
          continue;
        }
        throw e;
      } finally {
        done();
      }
    }
  }

  /** One statement on its own, which needs no transaction: on a pool's connection, or on the caller's as it is. */
  private async read(sql: string): Promise<Rows> {
    const db = this.db;
    if (!("getConnection" in db)) return (await new Tx(db, false, this.batch, this.book.sql.begin).plain([sql]))[0];
    const c = await db.getConnection();
    try {
      return (await new Tx(c, true, this.batch, this.book.sql.begin).plain([sql]))[0];
    } finally {
      c.release();
    }
  }

  private account(r: RefDef, v: unknown[]): Placed {
    const args = r.args.map((a) => ("param" in a ? String(v[a.param]) : a.literal));
    return { id: chId("account", this.book.name, this.tenant, r.kind, ...args), kind: r.kind, args };
  }

  /** Make sure every account is there, with the book's bounds, before a call locks the ones with bounds: read them, and make the missing ones. In the caller's transaction, a row made goes with the caller's rollback, so every call there reads them all, and what it makes is not taken as sure. */
  private async makeSure(x: Tx, accounts: Placed[]): Promise<void> {
    const want = new Map<bigint, Placed>();
    for (const a of accounts) if (!x.own || !this.sure.has(a.id)) want.set(a.id, a);
    if (want.size === 0) return;
    const ids = [...want.keys()];
    const [found] = await x.plain([fill(this.book.sql.readAccounts, { ...this.base, IDS: idList(ids) })]);
    const there = new Set<bigint>();
    for (const [h, lower, upper] of found as Array<[string, string | null, string | null]>) {
      const id = BigInt("0x" + h);
      const a = want.get(id);
      if (a === undefined) continue;
      const def = this.book.accounts[a.kind];
      if ((lower === null ? null : BigInt(lower)) !== def.lower || (upper === null ? null : BigInt(upper)) !== def.upper) {
        const e = new Error(`chobo: the account ${a.kind}(${a.args.join(", ")}) of ${this.book.name} has other bounds in the database than in the book`);
        (e as { code?: string }).code = "CB001";
        throw e;
      }
      there.add(id);
    }
    const missing = [...want.values()].filter((a) => !there.has(a.id)).sort((p, q) => (p.id < q.id ? -1 : 1));
    if (missing.length > 0) {
      const rows = missing.map((a) => fill(this.book.accounts[a.kind].row, { ...this.base, ID: idLit(a.id), ARGS: strLit(jsonStrings(a.args)) }));
      await x.plain([fill(this.book.sql.makeAccounts, { ROWS: rows.join(", ") })]);
    }
    if (x.own) for (const id of ids) this.sure.add(id);
  }

  /** `do` and `hold`: `v` has every argument, in the order of the parameters. */
  async move(kind: string, op: "do" | "hold", v: unknown[]): Promise<Result> {
    const t = this.book.transfers[kind];
    const placed = t.moves.map((m) => [this.account(m.from, v), this.account(m.to, v)] as const);
    // a move from an account to itself: refused before anything is sent, and the key is not used
    if (placed.some(([from, to]) => from.id === to.id)) return refused("same_account");
    const key = t.key.map((i) => v[i] as string);
    const content = contentOf(t, v);
    const vals: Record<string, string> = {
      ...this.base,
      CALL: idLit(chId("transfer", this.book.name, this.tenant, kind, op, ...key, "0")),
      KEY: strLit(jsonStrings(key)),
      CONTENT: strLit(content),
    };
    const bounded: bigint[] = [];
    for (const [i, m] of t.moves.entries()) {
      const k = i + 1;
      const [from, to] = placed[i];
      vals[`FROM${k}`] = idLit(from.id);
      vals[`TO${k}`] = idLit(to.id);
      vals[`FROMX${k}`] = hex(from.id);
      vals[`TOX${k}`] = hex(to.id);
      vals[`AMOUNT${k}`] = String("param" in m.amount ? (v[m.amount.param] as bigint) : m.amount.literal);
      if (this.book.accounts[from.kind].bounded) bounded.push(from.id);
      if (this.book.accounts[to.kind].bounded) bounded.push(to.id);
    }
    vals.BOUNDED = idList(bounded);
    const locked = new Set(bounded).size;
    return this.tx(async (x) => {
      await this.makeSure(x, placed.flat());
      const first = [fill(t.sql.key, vals)];
      if (t.sql.lock !== null) first.push(fill(t.sql.lock, vals));
      let rows: Rows[];
      try {
        rows = await x.run(first, { begin: true });
      } catch (e) {
        if (errno(e) !== DUPLICATE) throw e;
        // the key was used: by this call made before, or by another
        const [[used]] = await x.run([fill(this.book.sql.readCall, vals)]);
        await x.quit();
        const [result, , was, definition] = used as [string, string | null, string, string];
        if (result === "refused") return refused("already_refused");
        return was === content && definition === t.definition ? { result: "done_before" } : refused("key_conflict");
      }
      if (t.sql.lock !== null) {
        let lockRows = rows[1];
        if (lockRows.length !== locked) {
          // an account this value made sure of is not there: its row was taken away outside the
          // clients; make it again, and lock again
          for (const id of bounded) this.sure.delete(id);
          await this.makeSure(x, placed.flat());
          [lockRows] = await x.run([fill(t.sql.lock, vals)]);
          if (lockRows.length !== locked) throw new Error(`chobo: ${lockRows.length} of the ${locked} accounts of ${kind}.${op} with bounds are in the database`);
        }
        let first = 0;
        for (const r of lockRows) {
          const n = Number(r[1]);
          if (n > 0 && (first === 0 || n < first)) first = n;
        }
        if (first > 0) {
          const check = t.sql.checks.find((c) => c.n === first)!;
          await x.run([fill(t.sql.refuse, { ...vals, REASON: check.sql })], { commit: true });
          return refused(check.reason);
        }
      }
      const writes = [fill(t.sql.entries, vals)];
      if (t.sql.apply !== null) writes.push(fill(t.sql.apply, vals));
      await x.run(writes, { commit: true });
      return { result: "done" };
    });
  }

  /** The values of the end of a hold, from its moves as the database has them. */
  private ending(t: TransferDef, moves: Array<{ from: string; to: string; amount: string }>, posted: bigint[]): Record<string, string> {
    const vals: Record<string, string> = {};
    const bounded: bigint[] = [];
    for (const [i, m] of moves.entries()) {
      const k = i + 1;
      vals[`FROM${k}`] = `X'${m.from}'`;
      vals[`TO${k}`] = `X'${m.to}'`;
      vals[`HELD${k}`] = m.amount;
      vals[`POSTED${k}`] = String(posted[i] ?? 0n);
      if (this.book.accounts[t.moves[i].from.kind].bounded) bounded.push(BigInt("0x" + m.from));
      if (this.book.accounts[t.moves[i].to.kind].bounded) bounded.push(BigInt("0x" + m.to));
    }
    vals.BOUNDED = idList(bounded);
    return vals;
  }

  /** `post` and `void`: `key` in the order of the key; `amounts` in the order of the amounts, or null for all of it. */
  async end(kind: string, op: "post" | "void", key: string[], amounts: bigint[] | null): Promise<Result> {
    const t = this.book.transfers[kind];
    const hold = { ...this.base, HOLD: idLit(chId("transfer", this.book.name, this.tenant, kind, "hold", ...key, "0")) };
    return this.tx(async (x) => {
      const [[found]] = await x.run([fill(this.book.sql.holdLock, hold)], { begin: true });
      const answer = async (r: Result): Promise<Result> => {
        await x.quit();
        return r;
      };
      const row = found as [string, string | null, string | null, string | null, string | null, unknown] | undefined;
      if (row === undefined || row[0] === "refused") return answer(refused("no_such_hold"));
      const [, state, movesJson, postedJson, endedDefinition, past] = row;
      const moves = JSON.parse(movesJson!) as Array<{ from: string; to: string; amount: string }>;
      const holding = moves.map((m) => BigInt(m.amount));
      let posted = holding.map(() => 0n);
      if (op === "post") {
        const amountParams = t.params.flatMap((p, i) => (p.amount ? [i] : []));
        posted = amounts === null ? holding : t.moves.map((m) => ("param" in m.amount ? amounts[amountParams.indexOf(m.amount.param)] : m.amount.literal));
      }
      const postedText = "[" + posted.map((p) => `"${p}"`).join(",") + "]";
      if (state === "posted") {
        if (op === "void") return answer(refused("already_posted"));
        return answer(postedJson === postedText && endedDefinition === t.definition ? { result: "done_before" } : refused("key_conflict"));
      }
      if (state === "voided") {
        if (op === "post") return answer(refused("already_voided"));
        return answer(endedDefinition === t.definition ? { result: "done_before" } : refused("key_conflict"));
      }
      if (state === "expired" || Number(past) === 1) return answer(refused("expired"));
      if (op === "post" && posted.some((p, i) => p > holding[i])) return answer(refused("over_hold"));
      const vals = {
        ...hold,
        ...this.ending(t, moves, posted),
        CALL: idLit(chId("transfer", this.book.name, this.tenant, kind, op, ...key, "0")),
        KEY: strLit(jsonStrings(key)),
      };
      const [apply, entries]: [string[], string] = op === "post" ? [t.sql.postApply, t.sql.postEntries] : [t.sql.voidApply, t.sql.voidEntries];
      await x.run([...apply.map((s) => fill(s, vals)), fill(entries, vals)], { commit: true });
      return { result: "done" };
    });
  }

  async status(kind: string, key: string[]): Promise<HoldState | null> {
    const hold = { ...this.base, HOLD: idLit(chId("transfer", this.book.name, this.tenant, kind, "hold", ...key, "0")) };
    const [row] = (await this.read(fill(this.book.sql.status, hold))) as Array<[string, string | null, unknown]>;
    if (row === undefined || row[0] === "refused" || row[1] === null) return null;
    return row[1] === "held" && Number(row[2]) === 1 ? "expired" : (row[1] as HoldState);
  }

  async balance(kind: string, args: string[]): Promise<Balance> {
    const id = chId("account", this.book.name, this.tenant, kind, ...args);
    const [row] = (await this.read(fill(this.book.sql.balance, { ...this.base, ID: idLit(id) }))) as Array<[string, string, string]>;
    if (row === undefined) return { posted: 0n, held_in: 0n, held_out: 0n };
    const b = { posted: BigInt(row[0]), held_in: BigInt(row[1]), held_out: BigInt(row[2]) };
    for (const x of [b.posted, b.held_in, b.held_out]) {
      if (x > AMOUNT_LIMIT || x < -AMOUNT_LIMIT - 1n) throw new RangeError(`chobo: the balance of ${kind}(${args.join(", ")}) is past a 64-bit integer`);
    }
    return b;
  }

  /** Give back what the holds past their expiry hold, each in a transaction of its own, and add the entries of the accounts without bounds to their rows; answers how many holds it ended. */
  async expire(): Promise<number> {
    let ended = 0;
    const due = (await this.read(this.book.sql.due)) as Array<[string, string, string, string]>;
    for (const [tenant, id, kind, keyText] of due) {
      const t = this.book.transfers[kind];
      if (t === undefined) continue;
      const hold = { TENANT: `X'${tenant.toLowerCase()}'`, HOLD: `X'${id.toLowerCase()}'` };
      const done = await this.tx(async (x) => {
        const [[found]] = await x.run([fill(this.book.sql.holdLock, hold)], { begin: true });
        const row = found as [string, string | null, string | null, string | null, string | null, unknown] | undefined;
        if (row === undefined || row[1] !== "held" || Number(row[5]) !== 1) {
          await x.quit();
          return false;
        }
        const moves = JSON.parse(row[2]!) as Array<{ from: string; to: string; amount: string }>;
        const vals = { ...hold, ...this.ending(t, moves, []), CALL: idLit(chId("expire", id.toLowerCase())), KEY: strLit(keyText) };
        await x.run([...t.sql.expireApply.map((s) => fill(s, vals)), fill(t.sql.expireEntries, vals)], { commit: true });
        return true;
      });
      if (done) ended++;
    }
    const unfolded = (await this.read(this.book.sql.foldList)) as Array<[string, string]>;
    for (const [tenant, account] of unfolded) {
      const at = { TENANT: `X'${tenant.toLowerCase()}'`, ID: `X'${account.toLowerCase()}'` };
      await this.tx(async (x) => {
        const [, entries] = await x.run([fill(this.book.sql.foldLock, at), fill(this.book.sql.foldRead, at)], { begin: true });
        if (entries.length === 0) return x.quit();
        let [posted, heldIn, heldOut] = [0n, 0n, 0n];
        for (const r of entries as Array<[string, unknown, string, string, string]>) {
          posted += BigInt(r[2]);
          heldIn += BigInt(r[3]);
          heldOut += BigInt(r[4]);
        }
        const rows = (entries as Array<[string, unknown]>).map((r) => `(X'${r[0].toLowerCase()}', ${Number(r[1])})`).join(", ");
        await x.run(
          [fill(this.book.sql.foldApply, { ...at, POSTED: String(posted), HELD_IN: String(heldIn), HELD_OUT: String(heldOut) }), fill(this.book.sql.foldMark, { ...at, ROWS: rows })],
          { commit: true },
        );
      });
    }
    return ended;
  }
}
