# What `chobo build` writes

A book becomes code for one of two databases, PostgreSQL or TigerBeetle, and a client that calls
it from TypeScript, Python or Go. Every target answers every scenario as the reference
interpreter does: the tests run each scenario of the examples and the test books on all seven,
and compare what each client sent with `chobo run --show`.

| Target | What it writes |
|---|---|
| `postgres` | `<book>.sql`: a schema, its tables and constraints, and a function for each operation |
| `postgres-typescript` | `<book>.ts`: a client that calls those functions through `pg` |
| `postgres-python` | `<book>.py`: the same, through a DB-API connection (psycopg 3 or psycopg2) |
| `postgres-go` | `<package>/book.go` and `runtime.go`: the same, through pgx |
| `tigerbeetle-typescript` | `<book>.ts`: a client that sends TigerBeetle one chain of transfers for each operation, through `tigerbeetle-node` |
| `tigerbeetle-python` | `<book>.py`: the same, through `tigerbeetle` (`ClientSync`) |
| `tigerbeetle-go` | `<package>/book.go` and `runtime.go`: the same, through `tigerbeetle-go` |

The clients of the two databases have the same names, arguments and answers, so changing the
database changes how the book value is made, and nothing else. `chobo api` lists the names each
target uses, for a tool that calls them without reading the code ([formats.md](formats.md)).

## PostgreSQL

```console
$ chobo build examples/inventory/inventory.book --target postgres --out db
```

writes `db/inventory.sql`, which can run any number of times: it makes what is missing and
replaces the functions. A book is a schema (`"inventory"`) with four tables: `accounts` (a row an
account, with its balance and the bounds it was made with), `holds`, `keys` (every key used, what
it was called with, and what came of it) and `entries` (every change to an account, so that a
balance can be added up again). Each operation is a function, `"<transfer>_<operation>"`, taking
a tenant (a set of balances of its own; `''` for one book of balances) and the transfer's
parameters, and answering a `result` and a `reason`. With psql:

```sql
select * from inventory.receive_do('', 'd-1', 'A-1', 3);
select * from inventory.reserve_hold('', 'o-1', 'A-1', 2);
select * from inventory.reserve_hold('', 'o-2', 'A-1', 2);
select * from inventory.reserve_post('', 'o-1', 'A-1');
select inventory.reserve_status('', 'o-1', 'A-1') as state;
select * from inventory.balance_stock('', 'A-1');
```

```text
 result | reason 
--------+--------
 done   | 
(1 row)

 result | reason 
--------+--------
 done   | 
(1 row)

 result  |    reason    
---------+--------------
 refused | out_of_stock
(1 row)

 result | reason 
--------+--------
 done   | 
(1 row)

 state  
--------
 posted
(1 row)

 posted | held_in | held_out 
--------+---------+----------
      1 |       0 |        0
(1 row)
```

- **One write.** A function locks the rows of the accounts it moves, in the order of their IDs,
  checks every move against the balances it read, and writes them only when every move goes
  through. READ COMMITTED is enough: the check reads nothing but the rows it locked. Since every
  function takes the account rows in the same order, two of them never wait on each other in a
  circle.
- **Your transaction.** A function runs in the caller's transaction, so an order and the hold of
  its stock can commit together. The clients neither commit nor roll back. Under REPEATABLE READ
  or SERIALIZABLE, a call can fail to serialize; the clients retry it with the same arguments, up
  to thirty times, about a second and a half of waits in all (the key keeps it from moving twice).
- **Expiry.** PostgreSQL has no clock that acts on its own. A hold past its expiry answers
  `expired` to a post or a void at once, but what it holds still counts until `expire()` lets it
  go: call `select inventory.expire()` from pg_cron or a job of your own, one at a time. It lets
  go of up to 1000 holds a call, and answers how many.
- **A last guard.** Two `CHECK` constraints on `accounts` stop a write that does not go through
  the functions from breaking a bound. An account row whose bounds differ from the book's (a
  bound changed without a new account kind, which `chobo check --diff-base` refuses) stops every
  call that names it with the error `CB001`.
- **Names.** A schema, a function or a parameter has to fit PostgreSQL's 63 bytes (21 characters
  of Japanese), or the build stops (E061): PostgreSQL would cut the name short without a word.

## Calling it from TypeScript, Python and Go

`tigerbeetle(client, { tenant })` and `postgres(db, { tenant })` make the book value in
TypeScript; in Python, `tigerbeetle(client, tenant="")` and `postgres(conn, tenant="")` take the
same names as keyword arguments; in Go, `TigerBeetle(c, tenant)` and `Postgres(q, tenant)` take a
`context.Context` first in every call. An amount is a `bigint` in TypeScript, an `int` in Python
and an `int64` in Go. A refusal comes back as an answer; a failure (no connection, a wrong
argument) as an exception, or an `error` in Go.

TypeScript, on a TigerBeetle replica (`chobo build examples/inventory/inventory.book --target
tigerbeetle-typescript`):

```ts
import { createClient } from "tigerbeetle-node";
import { tigerbeetle } from "./inventory.ts";

const client = createClient({ cluster_id: 0n, replica_addresses: ["127.0.0.1:3399"] });
const book = tigerbeetle(client);
console.log(await book.receive.do({ delivery: "d-1", sku: "A-1", qty: 3n }));
console.log(await book.reserve.hold({ order: "o-1", sku: "A-1", qty: 2n }));
console.log(await book.reserve.hold({ order: "o-2", sku: "A-1", qty: 2n }));
console.log(await book.reserve.post({ order: "o-1", sku: "A-1" }, { qty: 1n }));
console.log(await book.reserve.status({ order: "o-1", sku: "A-1" }));
console.log(await book.balance.stock({ sku: "A-1" }));
client.destroy();
```

```text
{ result: 'done' }
{ result: 'done' }
{ result: 'refused', reason: 'out_of_stock' }
{ result: 'done' }
posted
{ posted: 2n, held_in: 0n, held_out: 0n }
```

The post of 1 of the 2 held lets the other one go back on the shelf. Python, on PostgreSQL
(`--target postgres-python`):

```python
import psycopg
from inventory import postgres

with psycopg.connect("host=/tmp/pg port=5432 dbname=shop", autocommit=True) as conn:
    book = postgres(conn, tenant="shop-2")
    print(book.receive.do(delivery="d-1", sku="A-1", qty=3))
    print(book.reserve.hold(order="o-1", sku="A-1", qty=2))
    print(book.reserve.hold(order="o-1", sku="A-1", qty=2))
    print(book.reserve.hold(order="o-1", sku="A-1", qty=1))
    print(book.reserve.void(order="o-1", sku="A-1"))
    print(book.reserve.status(order="o-1", sku="A-1"))
    print(book.balance.stock(sku="A-1"))
```

```text
Result(result='done', reason=None)
Result(result='done', reason=None)
Result(result='done_before', reason=None)
Result(result='refused', reason='key_conflict')
Result(result='done', reason=None)
voided
Balance(posted=3, held_in=0, held_out=0)
```

Go, on PostgreSQL (`--target postgres-go`), with a pgx pool:

```go
	b := inventory.Postgres(pool, "shop-3")
	fmt.Println(b.Receive.Do(ctx, inventory.ReceiveArgs{Delivery: "d-1", Sku: "A-1", Qty: 3}))
	fmt.Println(b.Reserve.Hold(ctx, inventory.ReserveArgs{Order: "o-1", Sku: "A-1", Qty: 5}))
	fmt.Println(b.Reserve.Hold(ctx, inventory.ReserveArgs{Order: "o-2", Sku: "A-1", Qty: 3}))
	fmt.Println(b.Reserve.Post(ctx, inventory.ReserveKey{Order: "o-2", Sku: "A-1"}, nil))
	fmt.Println(b.Reserve.Status(ctx, inventory.ReserveKey{Order: "o-2", Sku: "A-1"}))
	fmt.Println(b.Balance.Stock(ctx, inventory.StockAccount{Sku: "A-1"}))
	fmt.Println(b.Expire(ctx))
```

```text
{done } <nil>
{refused out_of_stock} <nil>
{done } <nil>
{done } <nil>
posted <nil>
{0 0 0} <nil>
0 <nil>
```

The outputs are what these printed, against a throwaway PostgreSQL cluster and a TigerBeetle
replica started with `--development`; the Python connection string stands for the one they ran
with. A name of the book that is Japanese is exported in Go with an `X` in front
(`b.X引当.Hold`), and a name that is a keyword of Python gets a `_` after it. No transfer kind is
called `balance` or `expire` in a client, whatever the book calls it.

## TigerBeetle

TigerBeetle runs no code of yours, so the client does the work: for each operation it sends one
chain of linked transfers in one request, which TigerBeetle applies whole or not at all.

- **The accounts.** Every account of the book is an account of TigerBeetle, whose balance is
  credits less debits, as chobo's is what came in less what went out. A move is a transfer from
  the account it takes from (debit) to the one it puts into (credit). A unit is a ledger, and the
  IDs are made from the keys, so that a retry sends the same IDs and TigerBeetle answers
  `exists`.
- **The bounds.** A lower bound of 0 is the flag `debits_must_not_exceed_credits`. Any other
  bound is kept with accounts and transfers chobo adds to the chain: a lower bound above 0 by a
  hold of that much made and voided right after the move, a lower bound below 0 by an account of
  the room down to it, and an upper bound by an account of the room up to it, each moved in the
  same chain as the move. One move becomes six transfers at the most.
- **Before a post or a void**, the client reads the hold; a hold that is not there answers
  `no_such_hold` without sending anything, so that a post that comes before its hold never makes
  the hold impossible to post. The accounts are made before the first operation that names them.
- **Expiry** is TigerBeetle's own: a hold's transfers carry its timeout, and TigerBeetle lets
  them go when it passes.
- **The state of a hold** is read by sending a chain TigerBeetle always refuses: a void of the
  hold, linked to a transfer from an account to itself. Why the void would have been refused
  says whether the hold is held, posted, voided or expired, and nothing is written.
- **Size.** A chain has to fit one request: 253 transfers on a replica started with
  `--development`, which is what chobo is tested on, and 8189 on one started without it. The
  build stops at 253 (E060), so that a book that builds also runs on the replica it can be tried
  on. Past it, split the transfer into kinds with fewer moves.

Two answers differ from the reference interpreter, and the tests do not reach either:

- When a key a bound refused comes again **with other amounts**, the reference interpreter
  answers `already_refused`. TigerBeetle forbids only the ID of the transfer it refused, so a
  transfer before it in the new chain can be refused first, by its own bound, and the client
  answers that bound's reason. Nothing moves either way; only the name of the reason differs. A
  retry with the same arguments answers `already_refused` on both.
- TigerBeetle keeps balances of 128 bits, so an operation that takes a balance past what 64 bits
  hold goes through, and reading the balance with `balance` then fails; the reference
  interpreter and PostgreSQL fail the operation itself.

## The clients' own needs

| Target | What the client is given |
|---|---|
| `postgres-typescript` | something with `query(text, values)`: a `pg` `Client`, `Pool` or `PoolClient` |
| `postgres-python` | a DB-API connection of psycopg 3 or psycopg2 |
| `postgres-go` | something with `QueryRow`: pgx's `*pgx.Conn`, `*pgxpool.Pool` or `pgx.Tx` |
| `tigerbeetle-typescript` | a client of `tigerbeetle-node` 0.17.9 |
| `tigerbeetle-python` | a `ClientSync` of `tigerbeetle` 0.17.9 |
| `tigerbeetle-go` | a client of `github.com/tigerbeetle/tigerbeetle-go` v0.17.9, which uses cgo |

The TypeScript clients use only types that can be stripped as they are, so Node.js runs them as
written, and `tsc --strict` compiles them. The tests run them with Node.js 23.11, Python 3.13 and
Go 1.25.5, on PostgreSQL 18.0 and TigerBeetle 0.17.9, with `pg` 8.23.1, psycopg 3.3.6 and pgx
5.11.0.
