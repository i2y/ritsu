//@@BOOK@@
// ── What follows is the same in every book chobo writes for PostgreSQL ───────
//
// A call is one SQL function, which does it in one transaction. The function
// answers a refusal as a row; an error is a mistake in the call, or the database's own.

/** What a client needs of a PostgreSQL connection: pg's `Client`, `Pool` and `PoolClient` have it. */
export interface Queryable {
  query(text: string, values?: unknown[]): Promise<{ rows: any[] }>;
}

/** Where a hold is: held, posted, voided, or expired. */
export type HoldState = "held" | "posted" | "voided" | "expired";

/** An account's balance: what is posted, what holds put in, and what holds take out. */
export type Balance = { posted: bigint; held_in: bigint; held_out: bigint };

const AMOUNT_LIMIT = (1n << 63n) - 1n;
/** How many times a call is made when PostgreSQL answers a serialization failure or a deadlock. */
const ATTEMPTS = 10;

/** A wait before the next try: up to 1, 2, 4 … 128 ms, at random, so that the calls that failed together do not meet again. */
function backoff(attempt: number): Promise<void> {
  return new Promise((r) => setTimeout(r, Math.random() * 2 ** Math.min(attempt - 1, 7)));
}

function str(name: string, v: unknown): string {
  if (typeof v !== "string") throw new TypeError(`chobo: ${name} is a string`);
  return v;
}

/** An amount as the SQL takes it: its decimal digits. */
function amt(name: string, v: unknown): string {
  if (typeof v !== "bigint") throw new TypeError(`chobo: ${name} is an amount, a bigint`);
  if (v < 0n || v > AMOUNT_LIMIT) throw new RangeError(`chobo: ${name} is ${v}, and an amount is from 0 to 2^63 - 1`);
  return v.toString();
}

class Runtime {
  readonly db: Queryable;
  readonly tenant: string;

  constructor(db: Queryable, tenant: string) {
    this.db = db;
    this.tenant = str("the tenant", tenant);
  }

  /** One row of `sql`. A serialization failure (40001) or a deadlock (40P01) is tried again with the same arguments: the key keeps the call from moving twice. Inside a transaction of the caller's, the second try finds the transaction aborted (25P02), and the first error is thrown. */
  private async row(sql: string, values: unknown[]): Promise<any> {
    let first: unknown = null;
    for (let attempt = 1; ; attempt++) {
      try {
        const { rows } = await this.db.query(sql, values);
        return rows[0];
      } catch (e) {
        const code = (e as { code?: unknown }).code;
        if (first !== null && code === "25P02") throw first;
        if ((code === "40001" || code === "40P01") && attempt < ATTEMPTS) {
          first ??= e;
          await backoff(attempt);
          continue;
        }
        throw e;
      }
    }
  }

  async call(sql: string, values: unknown[]): Promise<Result> {
    const r = await this.row(sql, values);
    if (r.result === "done" || r.result === "done_before") return { result: r.result };
    if (r.result === "refused") return { result: "refused", reason: r.reason };
    throw new Error(`chobo: the function answered ${JSON.stringify(r)}`);
  }

  async status(sql: string, values: unknown[]): Promise<HoldState | null> {
    const r = await this.row(sql, values);
    return r.state ?? null;
  }

  async balance(sql: string, values: unknown[]): Promise<Balance> {
    const r = await this.row(sql, values);
    return { posted: BigInt(r.posted), held_in: BigInt(r.held_in), held_out: BigInt(r.held_out) };
  }

  /** Give back what the holds past their expiry hold; answers how many holds it ended. */
  async expire(sql: string): Promise<number> {
    const r = await this.row(sql, []);
    return Number(r.expired);
  }
}
