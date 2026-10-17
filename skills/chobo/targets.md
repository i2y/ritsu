# What `chobo build` writes

A book becomes code for PostgreSQL, for TigerBeetle or for the MySQL family (MySQL, TiDB and
Vitess), and a client that calls it from TypeScript, Python or Go. Every target answers every
scenario as the reference interpreter does: the tests run each scenario of the examples and the
test books through every client, on PostgreSQL, TigerBeetle, MySQL, TiDB and Vitess, and compare
what each client sent with `chobo run --show`.

| Target | What it writes |
|---|---|
| `postgres` | `<book>.sql`: a schema, its tables and constraints, and a function for each operation |
| `postgres-typescript` | `<book>.ts`: a client that calls those functions through `pg` |
| `postgres-python` | `<book>.py`: the same, through a DB-API connection (psycopg 3 or psycopg2) |
| `postgres-go` | `<package>/book.go` and `runtime.go`: the same, through pgx |
| `tigerbeetle-typescript` | `<book>.ts`: a client that sends TigerBeetle one chain of transfers for each operation, through `tigerbeetle-node` |
| `tigerbeetle-python` | `<book>.py`: the same, through `tigerbeetle` (`ClientSync`) |
| `tigerbeetle-go` | `<package>/book.go` and `runtime.go`: the same, through `tigerbeetle-go` |
| `mysql` | `<book>.sql`: the book's three tables and their constraints, for MySQL, TiDB and Vitess |
| `mysql-typescript` | `<book>.ts`: a client that makes each operation one transaction on those tables, through `mysql2` |
| `mysql-python` | `<book>.py`: the same, through a DB-API connection (PyMySQL or mysqlclient) |
| `mysql-go` | `<package>/book.go` and `runtime.go`: the same, through database/sql and go-sql-driver/mysql |

The clients of every target have the same names, arguments and answers, so changing the database
changes how the book value is made, and nothing else. `chobo api` lists the names each target
uses, for a tool that calls them without reading the code ([formats.md](formats.md)). In a project
of ritsu's languages, `ritsu gen --books postgres`, `--books tigerbeetle` or `--books mysql`
(PostgreSQL unless it is given) writes the clients of every book into the project's package for
each language, under `books/`, and the SQL of PostgreSQL or of the MySQL family beside them as
`books/<book>.sql`.

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

## MySQL, TiDB and Vitess

```console
$ chobo build examples/refunds/refunds.book --target mysql --out db
```

writes `db/refunds.sql`, which can run any number of times: it makes what is missing. MySQL 8.4 or
later, TiDB 8.5 or later and Vitess all speak MySQL's protocol and read this SQL alike. It holds no
function: TiDB has no stored procedures, and a service that runs Vitess may allow neither stored
routines nor making a database. So the client sends the statements of each call itself, in one
transaction. The checks are in those statements, which chobo writes, and the client only fills in
the values and picks the next statement from what the last one answered. The three tables are made
in the database the SQL runs in, named after the book:

- `refunds_accounts`: a row an account, with its balance (`posted`, `held_in`, `held_out`) and the
  bounds it was made with, under two checks, `refunds_within_lower` and `refunds_within_upper`;
- `refunds_calls`: a row each `do` and `hold` that used its key, with what it was called with and
  what came of it; the row of a hold is the hold, with its state, its moves, its expiry and what
  was posted;
- `refunds_entries`: every change to an account, so that a balance can be added up again.

`chobo run --show mysql` prints what a client sends for each operation of a scenario, one
statement at a time, with the values in place. Before the first call that names an account, a book
value makes sure of the account: it reads the row, and makes it when it is not there.

```console
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json --show mysql
examples/refunds/refunds.book (more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00): 3 steps, as sent to MySQL (tenant "")
  1  sale.do(order: A-100, amount: 3000)
       makes sure of the accounts: 069c57ca75a83bb1b62d3735a168e8ae, 5e57413e175e6a7135612aa241f9cdb7
       begin /*T! pessimistic */
       insert into `refunds_calls` (`tenant`, `id`, `tenant_name`, `kind`, `op`, `key`, `content`, `definition`, `result`, `at`) values (X'e73bedd05928880def5276190c9048f5', X'be4165a032199c2adfdc384159a28ab2', _utf8mb4 X'', 'sale', 'do', _utf8mb4 X'5b22412d313030225d', _utf8mb4 X'7b226f72646572223a22412d313030222c22616d6f756e74223a333030307d', '82c5af592e8dc8dc57e86ce5aad47be5', 'done', utc_timestamp(6))
       insert into `refunds_entries` (`tenant`, `call_id`, `n`, `account`, `at`, `kind`, `op`, `key`, `d_posted`, `d_held_in`, `d_held_out`, `folded`) values (X'e73bedd05928880def5276190c9048f5', X'be4165a032199c2adfdc384159a28ab2', 1, X'069c57ca75a83bb1b62d3735a168e8ae', utc_timestamp(6), 'sale', 'do', _utf8mb4 X'5b22412d313030225d', -3000, 0, 0, 0), (X'e73bedd05928880def5276190c9048f5', X'be4165a032199c2adfdc384159a28ab2', 2, X'5e57413e175e6a7135612aa241f9cdb7', utc_timestamp(6), 'sale', 'do', _utf8mb4 X'5b22412d313030225d', 3000, 0, 0, 1)
       update `refunds_accounts` set `posted` = `posted` + (case when `id` = X'5e57413e175e6a7135612aa241f9cdb7' then 3000 else 0 end) where `tenant` = X'e73bedd05928880def5276190c9048f5' and `id` in (X'5e57413e175e6a7135612aa241f9cdb7')
       commit
…
  3  refund.hold(request: R-3, order: A-100, amount: 1000)
       makes sure of the accounts: 5e57413e175e6a7135612aa241f9cdb7, 32282c2ff4dcedae062c27e267ffe99c
       begin /*T! pessimistic */
       insert into `refunds_calls` (`tenant`, `id`, `tenant_name`, `kind`, `op`, `key`, `content`, `definition`, `result`, `at`, `state`, `moves`, `deadline`) values (X'e73bedd05928880def5276190c9048f5', X'005e0f73c51630b29d539a3d028722a2', _utf8mb4 X'', 'refund', 'hold', _utf8mb4 X'5b22522d33225d', _utf8mb4 X'7b2272657175657374223a22522d33222c226f72646572223a22412d313030222c22616d6f756e74223a313030307d', 'dbcdce25aa3073392b15088650849fad', 'done', utc_timestamp(6), 'held', '[{"from":"5e57413e175e6a7135612aa241f9cdb7","to":"32282c2ff4dcedae062c27e267ffe99c","amount":"1000"}]', utc_timestamp(6) + interval 604800 second)
       select hex(`id`), case when `id` = X'5e57413e175e6a7135612aa241f9cdb7' and `posted` - (`held_out`) - 1000 < 0 then 1 else 0 end from `refunds_accounts` where `tenant` = X'e73bedd05928880def5276190c9048f5' and `id` in (X'5e57413e175e6a7135612aa241f9cdb7') order by `id` for update
       insert into `refunds_entries` (`tenant`, `call_id`, `n`, `account`, `at`, `kind`, `op`, `key`, `d_posted`, `d_held_in`, `d_held_out`, `folded`) values (X'e73bedd05928880def5276190c9048f5', X'005e0f73c51630b29d539a3d028722a2', 1, X'5e57413e175e6a7135612aa241f9cdb7', utc_timestamp(6), 'refund', 'hold', _utf8mb4 X'5b22522d33225d', 0, 0, 1000, 1), (X'e73bedd05928880def5276190c9048f5', X'005e0f73c51630b29d539a3d028722a2', 2, X'32282c2ff4dcedae062c27e267ffe99c', utc_timestamp(6), 'refund', 'hold', _utf8mb4 X'5b22522d33225d', 0, 1000, 0, 0)
       update `refunds_accounts` set `held_out` = `held_out` + (case when `id` = X'5e57413e175e6a7135612aa241f9cdb7' then 1000 else 0 end) where `tenant` = X'e73bedd05928880def5276190c9048f5' and `id` in (X'5e57413e175e6a7135612aa241f9cdb7')
       commit
```

- **One write.** A call is one transaction. A `do` or a `hold` first inserts its own row into
  `refunds_calls`: a key used before stops it there, with a duplicate key (1062), and the client
  reads the earlier call to answer `done_before`, `key_conflict` or `already_refused`. Then it
  locks the rows of the accounts with bounds, in the order of their IDs, and the read itself says,
  for each row, the first move that would take it past a bound (the `case`, 0 for none); the
  earliest of them is the move refused. The update after it works the new balances out itself. A
  transfer with no bound to check, such as the sale above, reads nothing: its update locks the rows
  as it writes them. Every read in a transaction is a locking read, which sees the latest commit,
  so the calls work alike under READ COMMITTED and REPEATABLE READ.
- **Your transaction, or one of its own.** A call is part of the transaction its connection is
  in, and a transaction of its own when the connection is in none. Given a pool (a `Pool` of
  mysql2, a `*sql.DB`) or a `*sql.Conn`, each call is a transaction of its own; a `*sql.Tx` is the
  caller's. A connection of mysql2 is asked first, at every call that writes (`set @chobo = 0`,
  one round trip more), and a DB-API connection says it itself: in a transaction the caller
  began, or with autocommit off, the call is part of it; with autocommit on and no transaction
  open, it is one of its own. mysqlclient does not say whether `begin()` opened a transaction, so
  on its connection a transaction of yours is one with autocommit off. In a transaction of its
  own, after a deadlock (1213), a lock wait timeout (1205) or a write conflict of TiDB's (9007,
  8022, 8028), the client rolls it back and makes it again with the same arguments, up to thirty
  times (the key keeps it from moving twice). In the caller's, it sends neither `begin` nor
  `commit`, and an error comes back as it is, for the caller to roll back. An order and the hold
  of its stock commit together, or neither does. The book value then makes sure of the accounts
  at every call, since a rollback may take away a row it made. A connection, like a transaction,
  serves one call at a time: two calls at once on one would mix their statements, so give calls
  at the same time a pool.
- **`batch`.** A book value made with `batch` sends the statements a call sends in a row as one
  request, which the pool or the connection has to allow (`multipleStatements` in mysql2,
  `CLIENT.MULTI_STATEMENTS` in PyMySQL, `multiStatements=true` in go-sql-driver/mysql). A `do` or
  a `hold` that checks a bound then keeps its locks over one round trip, and any other call over
  none. The answers are the same.
- **Expiry.** As on PostgreSQL, nothing acts on the clock by itself: a hold past its expiry answers
  `expired` to a post or a void at once, but what it holds still counts until `expire()` lets it
  go. Call it from a job of your own, one at a time. It lets go of up to 1000 holds a call, each in
  a transaction of its own, and answers how many.
- **The accounts without bounds.** No check reads an account without bounds, such as one outside
  the book, so the clients never lock it: a move into it or out of it is an entry, and its balance
  is its row with the entries not yet added to it. `expire()` adds them. Two refunds wait on each
  other only when they take from the same order's `refundable`, never for `refunded`, which every
  refund puts into.
- **Keys as bytes.** A key and a tenant are compared as the bytes of their IDs, never as text:
  MySQL's default collation takes `A-1` and `a-1` for one key, and `utf8mb4_bin` ignores spaces at
  the end. The values in a statement are hex and digits, so it means the same under any
  `sql_mode`, `ANSI_QUOTES` and `NO_BACKSLASH_ESCAPES` among them, whatever a key holds.
- **A last guard.** The two checks stop a write that does not go through the clients from breaking
  a bound. An account row whose bounds differ from the book's stops every call that names it with
  the error `CB001`.
- **Names.** A name in MySQL is 64 characters at most, and holds no character beyond the Basic
  Multilingual Plane (𠮷 and the like), which MySQL writes as `?` with only a warning. The tables
  and the checks are named after the book, so the build stops (E062) for a book's name of more
  than 51 characters (`_within_lower` follows it), or with such a character in it. A character of
  Japanese counts as one, so a name can be longer than on PostgreSQL.
- **TiDB.** Run the SQL with `tidb_enable_check_constraint` ON, a global variable that is OFF by
  default: a table made without it keeps no check, and turning it on later adds none. The clients
  keep the bounds either way. `begin /*T! pessimistic */` starts a pessimistic transaction on TiDB
  whatever `tidb_txn_mode` says, and MySQL and Vitess read what follows `begin` as a comment.
- **Vitess.** The SQL and the clients keep to what a service that runs Vitess allows: they make no
  database, call no stored routine, and need no `sql_mode` that changes how a statement is read,
  which vtgate lets no session set. In a sharded keyspace, shard the three tables by `tenant`, the
  first column of each (a vindex such as `xxhash`): a tenant's rows are then on one shard, and so
  is every call. Vitess does not commit across shards all or nothing by default, so a transaction
  of yours that writes rows of your own with a call keeps them on the tenant's shard too. The tests
  run every scenario on Vitess 24.0.5 (vttestserver), in one shard and in two by the tenant, where
  a transaction across shards fails.

## Calling it from TypeScript, Python and Go

`postgres(db, { tenant })`, `tigerbeetle(client, { tenant })` and `mysql(db, { tenant, batch })`
make the book value in TypeScript; in Python, `postgres(conn, tenant="")`,
`tigerbeetle(client, tenant="")` and `mysql(conn, tenant="", batch=False)` take the same names as
keyword arguments; in Go, `Postgres(q, tenant)`, `TigerBeetle(c, tenant)` and
`MySQL(q, tenant, batch)` take a `context.Context` first in every call. An amount is a `bigint` in
TypeScript, an `int` in Python and an `int64` in Go. A refusal comes back as an answer; a failure
(no connection, a wrong argument) as an exception, or an `error` in Go.

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

The post of 1 of the 2 held lets the other one go back on the shelf. The same calls on MySQL
(`--target mysql-typescript`, beside the tables of `--target mysql` in a database `shop`) answer
the same; only the book value is made otherwise, here from a pool of mysql2 that allows more than
one statement a request, so that the calls go in batches:

```ts
import { createPool } from "mysql2/promise";
import { mysql } from "./inventory.ts";

const pool = createPool({ host: "127.0.0.1", port: 33316, user: "root", database: "shop", multipleStatements: true });
const book = mysql(pool, { tenant: "shop-4", batch: true });
console.log(await book.receive.do({ delivery: "d-1", sku: "A-1", qty: 3n }));
console.log(await book.reserve.hold({ order: "o-1", sku: "A-1", qty: 2n }));
console.log(await book.reserve.hold({ order: "o-2", sku: "A-1", qty: 2n }));
console.log(await book.reserve.post({ order: "o-1", sku: "A-1" }, { qty: 1n }));
console.log(await book.reserve.status({ order: "o-1", sku: "A-1" }));
console.log(await book.balance.stock({ sku: "A-1" }));
console.log(await book.expire());
await pool.end();
```

```text
{ result: 'done' }
{ result: 'done' }
{ result: 'refused', reason: 'out_of_stock' }
{ result: 'done' }
posted
{ posted: 2n, held_in: 0n, held_out: 0n }
0
```

No hold was past its expiry, so `expire()` let none go. Python, on PostgreSQL
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

Python, on TiDB (`--target mysql-python`), in the caller's transaction: a connection of PyMySQL
has autocommit off unless it is asked for, so each call is part of the transaction the caller
commits or rolls back, and the hold rolled back is not there to read.

```python
import pymysql
from inventory import mysql

conn = pymysql.connect(host="127.0.0.1", port=4016, user="root", database="shop")
book = mysql(conn, tenant="shop-5")
print(book.receive.do(delivery="d-1", sku="A-1", qty=3))
conn.commit()
print(book.reserve.hold(order="o-1", sku="A-1", qty=2))
conn.rollback()
print(book.reserve.status(order="o-1", sku="A-1"))
print(book.reserve.hold(order="o-1", sku="A-1", qty=2))
conn.commit()
print(book.reserve.status(order="o-1", sku="A-1"))
print(book.balance.stock(sku="A-1"))
```

```text
Result(result='done', reason=None)
Result(result='done', reason=None)
None
Result(result='done', reason=None)
held
Balance(posted=3, held_in=0, held_out=2)
```

The key of the hold rolled back was never used, so the same hold goes through after it. Go, on
PostgreSQL (`--target postgres-go`), with a pgx pool:

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

The outputs are what these printed, against a throwaway PostgreSQL cluster, a TigerBeetle replica
started with `--development`, a MySQL 8.4.11 server, and a TiDB 8.5.8 cluster whose tables were
made with `tidb_enable_check_constraint` ON; the connection string of psycopg stands for the one
they ran with. A name of the book that is Japanese is exported in Go with an `X` in front
(`b.X引当.Hold`), and a name that is a keyword of Python gets a `_` after it. No transfer kind is
called `balance` or `expire` in a client, whatever the book calls it.

## The clients' own needs

| Target | What the client is given |
|---|---|
| `postgres-typescript` | something with `query(text, values)`: a `pg` `Client`, `Pool` or `PoolClient` |
| `postgres-python` | a DB-API connection of psycopg 3 or psycopg2 |
| `postgres-go` | something with `QueryRow`: pgx's `*pgx.Conn`, `*pgxpool.Pool` or `pgx.Tx` |
| `tigerbeetle-typescript` | a client of `tigerbeetle-node` 0.17.9 |
| `tigerbeetle-python` | a `ClientSync` of `tigerbeetle` 0.17.9 |
| `tigerbeetle-go` | a client of `github.com/tigerbeetle/tigerbeetle-go` v0.17.9, which uses cgo |
| `mysql-typescript` | a pool or a connection of `mysql2`'s promise API (`mysql2/promise`); for `batch`, one made with `multipleStatements` |
| `mysql-python` | a DB-API connection of PyMySQL or mysqlclient; for `batch`, one made with `CLIENT.MULTI_STATEMENTS` |
| `mysql-go` | database/sql's `*sql.DB`, `*sql.Conn` or `*sql.Tx` through `github.com/go-sql-driver/mysql`, which the client imports to read the error numbers; for `batch`, `multiStatements=true` in the DSN |

The TypeScript clients use only types that can be stripped as they are, so Node.js runs them as
written, and `tsc --strict` compiles them. The tests run them with Node.js 23.11, Python 3.13 and
Go 1.26, on PostgreSQL 18.0, TigerBeetle 0.17.9, MySQL 8.4.11, TiDB 8.5.8 and Vitess 24.0.5, with
`pg` 8.23.1, psycopg 3.3.6, pgx 5.11.0, `mysql2` 3.24.5, PyMySQL 1.2.3 and go-sql-driver/mysql
1.10.1; mysqlclient 2.3.0 answered as PyMySQL does in a run by hand.
