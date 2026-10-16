# Build for a database

`chobo build` writes a book for one of two databases, PostgreSQL or TigerBeetle, and a client that
calls it from TypeScript, Python or Go:

| Target | What it writes |
|---|---|
| `postgres` | `<book>.sql`: a schema, its tables and constraints, and a function for each operation |
| `postgres-typescript`, `postgres-python`, `postgres-go` | a client that calls those functions |
| `tigerbeetle-typescript`, `tigerbeetle-python`, `tigerbeetle-go` | a client that sends TigerBeetle one chain of transfers for each operation |

```console
$ chobo build examples/inventory/inventory.book --target postgres --out db
$ chobo build examples/inventory/inventory.book --target postgres-typescript --out db
$ chobo build examples/inventory/inventory.book --target tigerbeetle-go --out db
```

Whichever the target, an operation is one write: it checks every move against the bounds and makes
them all, or refuses and makes none, with nothing read in between that another call could change.
That is what the bound of one account makes possible, since it looks at nothing but the account a
move changes. [Targets](targets.md) has each target in full, with calls from all three languages and
what they printed.

## PostgreSQL

`--target postgres` writes one file of SQL, which can run any number of times: it makes what is
missing and replaces the functions. A book is a schema with four tables: `accounts` (a row an
account, with its balance and the bounds it was made with), `holds`, `keys` (every key used, what
it was called with and what came of it) and `entries` (every change to an account, so that a
balance can be added up again). An operation is a function, `"<transfer>_<operation>"`, which takes
a tenant (a set of balances of its own; `''` when the book keeps one set) and the transfer's
parameters, and
answers a `result` and a `reason`.

The function locks the rows of the accounts the operation moves, in the order of their IDs, reads
their balances, and checks the moves against them in the order the book writes them. Of
`reserve.hold` in the inventory example:

<!-- chobo build examples/inventory/inventory.book --target postgres -->
```sql
  -- (3) the moves, in the order they are written, each checked against the locked rows as the
  -- moves before it left them. Nothing is written until every move has gone through.
  -- move qty from stock(sku) to customers
  v_from := array_position(v_ids, v_from_1);
  v_to := array_position(v_ids, v_to_1);
  if v_posted[v_from] - v_held_out[v_from] - "p_qty" < 0 then
    -- refused by a bound: nothing has moved, and the key stays used
    update "inventory".keys k set result = 'refused', reason = 'out_of_stock'
     where k.tenant = p_tenant and k.kind = 'reserve' and k.op = 'hold' and k.key = v_key;
    return ('refused', 'out_of_stock')::"inventory".result;
  end if;
  v_held_out[v_from] := v_held_out[v_from] + "p_qty";
  v_held_in[v_to] := v_held_in[v_to] + "p_qty";
```

- **READ COMMITTED is enough.** The check reads nothing but the rows the function locked, and a
  second call on the same accounts waits for the first to commit, then checks what it left. Since
  every function takes the rows in the same order, two of them never wait on each other in a
  circle.
- **The caller's transaction.** A function runs in the transaction it is called in, so an order and
  the hold of its stock can commit together, which only PostgreSQL offers. The clients neither
  commit nor roll back. Under REPEATABLE READ or SERIALIZABLE a call can fail to serialize, and the
  clients try it again with the same arguments, thirty times at most; the key keeps it from moving
  twice.
- **Expiry.** PostgreSQL has no clock that acts by itself. A hold past its expiry answers `expired`
  to a post or a void at once, but what it holds counts until `expire()` lets it go: call it from
  pg_cron or a job of your own, one at a time.
- **A last guard.** Two `CHECK` constraints on `accounts` stop a write that does not go through the
  functions from breaking a bound, and a row whose bounds differ from the book's stops every call
  that names it with the error `CB001`.

## TigerBeetle

TigerBeetle runs no code of yours, so the client does the work: for each operation it sends one
chain of linked transfers in one request, which TigerBeetle applies whole or not at all.

- Every account of the book is an account of TigerBeetle, whose balance is its credits less its
  debits, and a move is a transfer from one to the other. A unit is a ledger.
- A lower bound of 0 is the flag `debits_must_not_exceed_credits`. Any other bound is kept by
  accounts and transfers chobo adds to the chain: a lower bound above 0 by a hold of that much,
  made and voided right after the move; a lower bound below 0, and an upper bound, by an account of
  the room left to the bound, moved in the same chain. One move becomes six transfers at the most.
- Before a post or a void, the client reads the hold, and answers `no_such_hold` without sending
  anything when it is not there: a post sent before its hold would make the hold impossible to
  post.
- A chain fits one request: 253 transfers on a replica started with `--development`, which is what
  chobo is tested on, so the build stops there (E060).

```console
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json --show tigerbeetle
examples/refunds/refunds.book (more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00): 3 steps, as sent to TigerBeetle (tenant "")
  1  sale.do(order: A-100, amount: 3000)
       accounts:
         account  sales              069c57ca75a83bb1b62d3735a168e8ae
         account  refundable(A-100)  5e57413e175e6a7135612aa241f9cdb7  debits_must_not_exceed_credits
       chain:
         0  main  sales → refundable(A-100)  3000  be4165a032199c2adfdc384159a28ab2
…
```

TigerBeetle answers otherwise than the reference interpreter in two cases the tests do not reach: a
retry with other amounts of a key a bound refused, and a balance past what 64 bits hold.
[Targets](targets.md#tigerbeetle) says what each does.

## The clients

The clients of both databases have the same names, arguments and answers, so changing the database
changes how the book value is made, and nothing else. A call answers `done`, `done_before` or a
refusal with its reason; the reasons are a type of their own, the bounds' first:

<!-- chobo build examples/inventory/inventory.book --target postgres-typescript -->
```ts
/** Why a call of inventory can be refused: the bounds' own reasons, then the ones chobo gives itself. */
export type Reason = "out_of_stock" | "key_conflict" | "already_refused" | "same_account" | "no_such_hold" | "already_posted" | "already_voided" | "expired" | "over_hold";

/** What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason says why. */
export type Result = { result: "done" } | { result: "done_before" } | { result: "refused"; reason: Reason };
```

- **A refusal is an answer, not an exception.** There is no stock: the caller handles
  `out_of_stock` as it handles any other answer. A failure is something else, an exception (an
  `error` in Go): a database that cannot be reached, an argument of the wrong type, an amount out
  of range.
- **A retry is safe.** The key is the book's, made of the transfer's own arguments, so the same call
  again answers `done_before` and moves nothing, whichever service or language sends it.
- **The book value.** In TypeScript, `postgres(db, { tenant })` and `tigerbeetle(client, { tenant })`
  make it; in Python, `postgres(conn, tenant="")` and `tigerbeetle(client, tenant="")`, with the
  arguments as keyword arguments; in Go, `Postgres(q, tenant)` and `TigerBeetle(c, tenant)`, with a
  `context.Context` first in every call. An amount is a `bigint`, an `int` or an `int64`.
- **Holds.** A transfer that holds has `hold`, `post` (all of it, or the amounts given), `void` and
  `status`; every account has a `balance`. On PostgreSQL the book value also has `expire()`.

A client imports the language's standard library and the database's own client library (pgx in
Go, and TigerBeetle's in the three languages), and nothing else. The TypeScript runs on Node.js as
it is written and compiles under `tsc --strict`; the Python passes `mypy --strict`; the Go passes
`gofmt` and `go vet`.

## IDs

On TigerBeetle, every ID is made from what the book says, hashed with SHA-256: an account's from the
book's name, the tenant, the account's kind and its arguments, and a transfer's from the book's
name, the tenant, the transfer's kind, the operation, the values of its key and its place in the
chain. So a retry sends the same IDs, and TigerBeetle answers that the transfer exists. The three languages and the reference interpreter
make the same IDs, and the tests see that they do, on every operation of every scenario; on
PostgreSQL the accounts and holds carry the same IDs. `chobo api` gives the recipe, for a tool that
works one out itself ([Formats](formats.md#the-api-of-a-book)).
