import { createHash, randomBytes } from "node:crypto";
import { AccountFlags, CreateAccountStatus, CreateTransferStatus, TransferFlags, amount_max } from "tigerbeetle-node";
import type { Account, CreateAccountResult, CreateTransferResult, Transfer } from "tigerbeetle-node";
//@@BOOK@@
// ── What follows is the same in every book chobo writes for TigerBeetle ──────
//
// A call is one chain of transfers in one request: TigerBeetle takes the chain
// whole or not at all. The bounds TigerBeetle cannot keep with an account's flags are kept by
// more transfers in the chain (a probe for a lower bound above 0, the room and floor accounts
// for an upper bound and a lower bound below 0), laid out as PLAN 0.3 says.

/** What a client needs of a TigerBeetle client: tigerbeetle-node's `Client` has it. */
export interface TigerBeetleClient {
  createAccounts(batch: Account[]): Promise<CreateAccountResult[]>;
  createTransfers(batch: Transfer[]): Promise<CreateTransferResult[]>;
  lookupAccounts(batch: bigint[]): Promise<Account[]>;
  lookupTransfers(batch: bigint[]): Promise<Transfer[]>;
}

/** Where a hold is: held, posted, voided, or expired. */
export type HoldState = "held" | "posted" | "voided" | "expired";

/** An account's balance: what is posted, what holds put in, and what holds take out. */
export type Balance = { posted: bigint; held_in: bigint; held_out: bigint };

type Role = "main" | "probe" | "probe_void" | "floor_out" | "room_back" | "room_in" | "floor_back";
type RefDef = { kind: string; args: Array<{ param: number } | { literal: string }> };
type MoveDef = { amount: { param: number } | { literal: bigint }; from: RefDef; to: RefDef; roles: Role[] };
type TransferDef = {
  params: Array<{ name: string; amount: boolean }>;
  key: number[];
  pending: boolean;
  timeout: number;
  code: number;
  definition: string;
  moves: MoveDef[];
};
type BoundDef = { value: bigint; reason: Reason };
type AccountDef = { params: string[]; unit: string; code: number; flagged: boolean; lower: BoundDef | null; upper: BoundDef | null };
type BookDef = {
  name: string;
  units: Record<string, { ledger: number; sinkCode: number }>;
  accounts: Record<string, AccountDef>;
  transfers: Record<string, TransferDef>;
};

const ROLE_CODES: Record<Role, number> = { main: 1, probe: 2, probe_void: 3, floor_out: 4, room_back: 5, room_in: 6, floor_back: 7 };
const OPENING = 8;
/** The most events one request takes on every cluster (a replica started with --development). */
const REQUEST_MAX = 253;
const AMOUNT_LIMIT = (1n << 63n) - 1n;
const ID_LIMIT = (1n << 128n) - 1n;

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

/** An ID no one else has: for the transfers that read a hold and are never kept. */
function randomId(): bigint {
  const v = BigInt("0x" + randomBytes(16).toString("hex"));
  return v === 0n || v === ID_LIMIT ? 1n : v;
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

function tbAccount(id: bigint, ledger: number, code: number, flagged: boolean, userData128: bigint, userData32: number): Account {
  return {
    id,
    debits_pending: 0n,
    debits_posted: 0n,
    credits_pending: 0n,
    credits_posted: 0n,
    user_data_128: userData128,
    user_data_64: 0n,
    user_data_32: userData32,
    reserved: 0,
    ledger,
    code,
    flags: flagged ? AccountFlags.debits_must_not_exceed_credits : AccountFlags.none,
    timestamp: 0n,
  };
}

function tbTransfer(fields: Partial<Transfer>): Transfer {
  return {
    id: 0n,
    debit_account_id: 0n,
    credit_account_id: 0n,
    amount: 0n,
    pending_id: 0n,
    user_data_128: 0n,
    user_data_64: 0n,
    user_data_32: 0,
    timeout: 0,
    ledger: 0,
    code: 0,
    flags: TransferFlags.none,
    timestamp: 0n,
    ...fields,
  };
}

/** A transfer of a chain, with the move (from 0) and the role it is there for. */
type Planned = { transfer: Transfer; role: Role; move: number };

/** Every transfer of the chain but the last is linked to the next. */
function link(chain: Planned[]): void {
  for (const x of chain.slice(0, -1)) x.transfer.flags |= TransferFlags.linked;
}

/** What a `do` or `hold` is called with, as the IDs hash it: the arguments in the order of the parameters. */
function contentOf(t: TransferDef, v: unknown[]): string {
  return "{" + t.params.map((p, i) => JSON.stringify(p.name) + ":" + (p.amount ? String(v[i]) : JSON.stringify(v[i]))).join(",") + "}";
}

type Placed = { id: bigint; kind: string; def: AccountDef };

class Runtime {
  readonly book: BookDef;
  readonly client: TigerBeetleClient;
  readonly tenant: string;
  /** The accounts and openings this value has made sure of: TigerBeetle never removes them. */
  readonly made: Set<bigint> = new Set();

  constructor(book: BookDef, client: TigerBeetleClient, tenant: string) {
    this.book = book;
    this.client = client;
    this.tenant = str("the tenant", tenant);
  }

  private account(r: RefDef, v: unknown[]): Placed {
    const def = this.book.accounts[r.kind];
    const args = r.args.map((a) => ("param" in a ? String(v[a.param]) : a.literal));
    return { id: chId("account", this.book.name, this.tenant, r.kind, ...args), kind: r.kind, def };
  }

  private kindId(kind: string): bigint {
    return chId("kind", this.book.name, this.tenant, kind);
  }

  private transferId(kind: string, op: string, key: string[], position: number): bigint {
    return chId("transfer", this.book.name, this.tenant, kind, op, ...key, String(position));
  }

  /** `do` and `hold`: `v` has every argument, in the order of the parameters. */
  async move(kind: string, op: "do" | "hold", v: unknown[]): Promise<Result> {
    const t = this.book.transfers[kind];
    const placed = t.moves.map((m) => [this.account(m.from, v), this.account(m.to, v)] as const);
    // a move from an account to itself: refused before anything is sent, and the key is not used
    if (placed.some(([from, to]) => from.id === to.id)) return refused("same_account");
    const key = t.key.map((i) => v[i] as string);
    const content = chId("content", this.book.name, this.tenant, kind, op, t.definition, contentOf(t, v));
    const accounts: Account[] = [];
    const openings: Transfer[] = [];
    const chain: Planned[] = [];
    const add = (a: Account): void => {
      if (!accounts.some((x) => x.id === a.id)) accounts.push(a);
    };
    for (const [i, m] of t.moves.entries()) {
      const [from, to] = placed[i];
      const unit = from.def.unit;
      const ledger = this.book.units[unit].ledger;
      const sink = chId("sink", this.book.name, this.tenant, unit);
      add(tbAccount(from.id, ledger, from.def.code, from.def.flagged, this.kindId(from.kind), 0));
      add(tbAccount(to.id, ledger, to.def.code, to.def.flagged, this.kindId(to.kind), 0));
      if (m.roles.length > 1) add(tbAccount(sink, ledger, this.book.units[unit].sinkCode, false, 0n, 3));
      // the floor and room accounts the move's bounds need, each with the transfer that opens it
      for (const role of m.roles) {
        let what: "floor" | "room";
        let owner: Placed;
        let opening: bigint;
        if (role === "floor_out") [what, owner, opening] = ["floor", from, -from.def.lower!.value];
        else if (role === "room_back") [what, owner, opening] = ["room", from, from.def.upper!.value];
        else if (role === "room_in") [what, owner, opening] = ["room", to, to.def.upper!.value];
        else if (role === "floor_back") [what, owner, opening] = ["floor", to, -to.def.lower!.value];
        else continue;
        const xid = chId(what, hex(owner.id));
        if (accounts.some((x) => x.id === xid)) continue;
        add(tbAccount(xid, ledger, owner.def.code, true, this.kindId(owner.kind), what === "floor" ? 1 : 2));
        openings.push(tbTransfer({ id: chId("opening", hex(xid)), debit_account_id: sink, credit_account_id: xid, amount: opening, user_data_32: OPENING, ledger, code: owner.def.code }));
      }
      const amount = "param" in m.amount ? (v[m.amount.param] as bigint) : m.amount.literal;
      const flags = op === "hold" ? TransferFlags.pending : TransferFlags.none;
      const timeout = op === "hold" ? t.timeout : 0;
      for (const role of m.roles) {
        const head = { id: this.transferId(kind, op, key, chain.length), user_data_128: content, user_data_32: ROLE_CODES[role] };
        let fields: Partial<Transfer>;
        switch (role) {
          case "main":
            fields = { debit_account_id: from.id, credit_account_id: to.id, amount, flags, timeout, ledger, code: t.code };
            break;
          case "probe":
            fields = { debit_account_id: from.id, credit_account_id: sink, amount: from.def.lower!.value, flags: TransferFlags.pending, ledger, code: t.code };
            break;
          case "probe_void":
            fields = { pending_id: chain[chain.length - 1].transfer.id, flags: TransferFlags.void_pending_transfer };
            break;
          case "floor_out":
            fields = { debit_account_id: chId("floor", hex(from.id)), credit_account_id: sink, amount, flags, timeout, ledger, code: t.code };
            break;
          case "room_back":
            fields = { debit_account_id: sink, credit_account_id: chId("room", hex(from.id)), amount, flags, timeout, ledger, code: t.code };
            break;
          case "room_in":
            fields = { debit_account_id: chId("room", hex(to.id)), credit_account_id: sink, amount, flags, timeout, ledger, code: t.code };
            break;
          case "floor_back":
            fields = { debit_account_id: sink, credit_account_id: chId("floor", hex(to.id)), amount, flags, timeout, ledger, code: t.code };
            break;
        }
        chain.push({ transfer: tbTransfer({ ...head, ...fields }), role, move: i });
      }
    }
    link(chain);
    await this.ensure(accounts, openings);
    const results = await this.client.createTransfers(chain.map((x) => x.transfer));
    return this.read(kind, op, chain, results);
  }

  /** Make sure the accounts and the openings are there before a chain needs them: TigerBeetle refuses a transfer to an account it does not have, and never takes its ID again. */
  private async ensure(accounts: Account[], openings: Transfer[]): Promise<void> {
    const newAccounts = accounts.filter((a) => !this.made.has(a.id));
    for (let i = 0; i < newAccounts.length; i += REQUEST_MAX) {
      const batch = newAccounts.slice(i, i + REQUEST_MAX);
      const results = await this.client.createAccounts(batch);
      for (const [j, r] of results.entries()) {
        if (r.status !== CreateAccountStatus.created && r.status !== CreateAccountStatus.exists) {
          throw new Error(`chobo: TigerBeetle has the account ${hex(batch[j].id)} made another way (${CreateAccountStatus[r.status]}): the book's accounts have changed since it was made`);
        }
      }
      for (const a of batch) this.made.add(a.id);
    }
    const newOpenings = openings.filter((x) => !this.made.has(x.id));
    for (let i = 0; i < newOpenings.length; i += REQUEST_MAX) {
      const batch = newOpenings.slice(i, i + REQUEST_MAX);
      const results = await this.client.createTransfers(batch);
      for (const [j, r] of results.entries()) {
        if (r.status === CreateTransferStatus.exists_with_different_amount) {
          throw new Error(`chobo: the account ${hex(batch[j].credit_account_id)} was opened with another bound: the book's bounds have changed since it was made`);
        }
        if (r.status !== CreateTransferStatus.created && r.status !== CreateTransferStatus.exists) {
          throw new Error(`chobo: TigerBeetle answered ${CreateTransferStatus[r.status]} for the opening ${hex(batch[j].id)}`);
        }
      }
      for (const x of batch) this.made.add(x.id);
    }
  }

  /** What a chain's results answer: the first that is neither created nor linked_event_failed decides. */
  private read(kind: string, op: string, chain: Planned[], results: CreateTransferResult[]): Result {
    const t = this.book.transfers[kind];
    for (const [i, r] of results.entries()) {
      const name = CreateTransferStatus[r.status];
      switch (r.status) {
        case CreateTransferStatus.created:
        case CreateTransferStatus.linked_event_failed:
          continue;
        case CreateTransferStatus.exists:
          // the first transfer there already: the same call, made before; a later one: a chain of another shape
          return i === 0 ? { result: "done_before" } : refused("key_conflict");
        case CreateTransferStatus.id_already_failed:
          return refused("already_refused");
        case CreateTransferStatus.exceeds_credits: {
          const m = t.moves[chain[i].move];
          const bound = chain[i].role === "room_in" ? this.book.accounts[m.to.kind].upper : this.book.accounts[m.from.kind].lower;
          if (bound !== null) return refused(bound.reason);
          break;
        }
        case CreateTransferStatus.accounts_must_be_different:
          return refused("same_account");
        case CreateTransferStatus.pending_transfer_already_posted:
          return refused("already_posted");
        case CreateTransferStatus.pending_transfer_already_voided:
          return refused("already_voided");
        case CreateTransferStatus.pending_transfer_expired:
          return refused("expired");
        case CreateTransferStatus.exceeds_pending_transfer_amount:
          return refused("over_hold");
        default:
          if (name.startsWith("exists_with_different_")) return refused("key_conflict");
      }
      throw new Error(`chobo: TigerBeetle answered ${name} for transfer ${i} (${chain[i].role}) of ${kind}.${op}`);
    }
    return { result: "done" };
  }

  /** The transfers of the chain that held for `key`: their roles, moves and IDs. */
  private held(kind: string, key: string[]): Array<{ role: Role; move: number; id: bigint }> {
    const out: Array<{ role: Role; move: number; id: bigint }> = [];
    for (const [i, m] of this.book.transfers[kind].moves.entries()) {
      for (const role of m.roles) out.push({ role, move: i, id: this.transferId(kind, "hold", key, out.length) });
    }
    return out;
  }

  /** `post` and `void`: `key` in the order of the key; `amounts` in the order of the amounts, or null for all of it. */
  async end(kind: string, op: "post" | "void", key: string[], amounts: bigint[] | null): Promise<Result> {
    const t = this.book.transfers[kind];
    const held = this.held(kind, key);
    const mains = held.filter((h) => h.role === "main");
    // read the hold first: a post or void of a hold TigerBeetle does not have would use up its ID for good
    const found = await this.client.lookupTransfers(mains.map((h) => h.id));
    if (found.length === 0) return refused("no_such_hold");
    if (found.length !== mains.length) throw new Error(`chobo: TigerBeetle has only part of the hold ${kind}(${key.join(", ")})`);
    const holding = mains.map((h) => found.find((x) => x.id === h.id)!.amount);
    let posted: bigint[] = [];
    if (op === "post") {
      const amountParams = t.params.flatMap((p, i) => (p.amount ? [i] : []));
      posted = amounts === null ? holding : t.moves.map((m) => ("param" in m.amount ? amounts[amountParams.indexOf(m.amount.param)] : m.amount.literal));
      // TigerBeetle checks the amount before it checks whether the hold has ended: ask where the hold is instead
      if (posted.some((p, i) => p > holding[i])) {
        const state = await this.state(mains[0].id);
        return state === "held" ? refused("over_hold") : state === "posted" ? refused("key_conflict") : state === "voided" ? refused("already_voided") : state === "expired" ? refused("expired") : refused("no_such_hold");
      }
    }
    const content = chId("content", this.book.name, this.tenant, kind, op, t.definition, op === "post" ? "[" + posted.join(",") + "]" : "null");
    const chain: Planned[] = [];
    for (const h of held) {
      if (h.role === "probe" || h.role === "probe_void") continue;
      const amount = op === "void" ? 0n : amounts === null ? amount_max : posted[h.move];
      const flags = op === "post" ? TransferFlags.post_pending_transfer : TransferFlags.void_pending_transfer;
      chain.push({ transfer: tbTransfer({ id: this.transferId(kind, op, key, chain.length), pending_id: h.id, amount, user_data_128: content, user_data_32: ROLE_CODES[h.role], flags }), role: h.role, move: h.move });
    }
    link(chain);
    const results = await this.client.createTransfers(chain.map((x) => x.transfer));
    return this.read(kind, op, chain, results);
  }

  /** Where the hold of the pending transfer `id` is, as TigerBeetle would answer a void of it now: a void and a transfer that cannot go through, linked, so that nothing is kept. */
  private async state(id: bigint): Promise<HoldState | null> {
    const results = await this.client.createTransfers([
      tbTransfer({ id: randomId(), pending_id: id, flags: TransferFlags.linked | TransferFlags.void_pending_transfer }),
      tbTransfer({ id: randomId(), debit_account_id: 1n, credit_account_id: 1n, ledger: 1, code: 1 }),
    ]);
    switch (results[0].status) {
      case CreateTransferStatus.linked_event_failed:
        if (results[1].status === CreateTransferStatus.accounts_must_be_different) return "held";
        break;
      case CreateTransferStatus.pending_transfer_already_posted:
        return "posted";
      case CreateTransferStatus.pending_transfer_already_voided:
        return "voided";
      case CreateTransferStatus.pending_transfer_expired:
        return "expired";
      case CreateTransferStatus.pending_transfer_not_found:
        return null;
    }
    throw new Error(`chobo: TigerBeetle answered ${CreateTransferStatus[results[0].status]}, ${CreateTransferStatus[results[1].status]} when asked where a hold is`);
  }

  async status(kind: string, key: string[]): Promise<HoldState | null> {
    return this.state(this.held(kind, key)[0].id);
  }

  async balance(kind: string, args: string[]): Promise<Balance> {
    const id = chId("account", this.book.name, this.tenant, kind, ...args);
    const [a] = await this.client.lookupAccounts([id]);
    if (a === undefined) return { posted: 0n, held_in: 0n, held_out: 0n };
    const b = { posted: a.credits_posted - a.debits_posted, held_in: a.credits_pending, held_out: a.debits_pending };
    for (const v of [b.posted, b.held_in, b.held_out]) {
      if (v > AMOUNT_LIMIT || v < -AMOUNT_LIMIT - 1n) throw new RangeError(`chobo: the balance of ${kind}(${args.join(", ")}) is past a 64-bit integer`);
    }
    return b;
  }
}
