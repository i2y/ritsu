# Build for a database

`chobo build` writes a book for PostgreSQL, for TigerBeetle or for the MySQL family (MySQL, TiDB and
Vitess), and a client that calls it from TypeScript, Python, Go or Rust:

| Target | What it writes |
|---|---|
| `postgres` | `<book>.sql`: a schema, its tables and constraints, and a function for each operation |
| `postgres-typescript`, `postgres-python`, `postgres-go`, `postgres-rust` | a client that calls those functions |
| `tigerbeetle-typescript`, `tigerbeetle-python`, `tigerbeetle-go`, `tigerbeetle-rust` | a client that sends TigerBeetle one chain of transfers for each operation |
| `mysql` | `<book>.sql`: the book's three tables and their constraints, for MySQL, TiDB and Vitess |
| `mysql-typescript`, `mysql-python`, `mysql-go`, `mysql-rust` | a client that makes each operation one transaction on those tables |

```console
$ chobo build examples/inventory/inventory.book --target postgres --out db
$ chobo build examples/inventory/inventory.book --target postgres-typescript --out db
$ chobo build examples/inventory/inventory.book --target tigerbeetle-go --out db
$ chobo build examples/inventory/inventory.book --target mysql --out db
$ chobo build examples/inventory/inventory.book --target mysql-python --out db
$ chobo build examples/inventory/inventory.book --target tigerbeetle-rust --out db
```

Whichever the target, an operation is one write: it checks every move against the bounds and makes
them all, or refuses and makes none, with nothing read in between that another call could change.
That is what the bound of one account makes possible, since it looks at nothing but the account a
move changes. [Targets](targets.md) has each target in full, with calls from all four languages and
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
  the hold of its stock can commit together. The clients neither commit nor roll back. Under
  REPEATABLE READ or SERIALIZABLE a call can fail to serialize, and the clients try it again with
  the same arguments, thirty times at most; the key keeps it from moving twice.
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

## MySQL, TiDB and Vitess

`--target mysql` writes one file of SQL for MySQL 8.4 or later, TiDB 8.5 or later and Vitess, which
can run any number of times: it makes the book's three tables, named after the book, in the database
it runs in. There is no function to call. TiDB has no stored procedures, and a service that runs
Vitess may allow neither stored routines nor making a database, so the client runs each operation
as one transaction of statements chobo wrote, and fills in nothing but the values. The table of the
accounts, in the refunds example:

<!-- chobo build examples/refunds/refunds.book --target mysql -->
```sql
-- the accounts: one row each, made the first time a call names it. An account with a bound
-- holds its balance here, and one without holds what expire() has added up of its entries. The
-- two checks stop a write that does not go through the clients, which check the bounds themselves.
create table if not exists `refunds_accounts` (
…
  `posted` bigint not null default 0,
  `held_in` bigint not null default 0,
  `held_out` bigint not null default 0,
  primary key (`tenant`, `id`),
  constraint `refunds_within_lower` check (`lower_bound` is null or `posted` - `held_out` >= least(`lower_bound`, 0)),
  constraint `refunds_within_upper` check (`upper_bound` is null or `posted` + `held_in` <= greatest(`upper_bound`, 0))
) default charset = utf8mb4 collate = utf8mb4_bin;
```

The other two are `refunds_calls`, a row each call that used its key (the row of a hold is the
hold), and `refunds_entries`, every change to an account. `--show mysql` prints the statements a
client sends, with the values in place, after the accounts it makes sure of: a book value reads
each the first time it names it, and makes it when it is not there. The last refund of the
scenario of two at the same time:

```console
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json --show mysql
examples/refunds/refunds.book (more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00): 3 steps, as sent to MySQL (tenant "")
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

- **The key first.** A `do` or a `hold` puts its own row in first. A key used before stops the call
  there, with a duplicate key, and the row of the earlier call says whether it answers
  `done_before`, `key_conflict` or `already_refused`.
- **One locking read.** Then the rows of the accounts with bounds are locked in the order of their
  IDs and read once, and the read itself says which move would break a bound first (the `case`).
  Nothing else is read before the write. An account without bounds, such as `refunded` here, is
  never locked: what moves into it is an entry, which `expire()` adds to its row, so refunds of
  other orders never wait on one another.
- **The caller's transaction, or one of its own.** A call is part of the transaction its
  connection is in, so an order and the hold of its stock commit together. On a pool, or on a
  connection in no transaction, each call is a transaction of its own, made again with the same
  arguments after a deadlock, a lock wait that timed out or a write conflict of TiDB's. A
  connection of mysql2 is asked first whether it is in one (`set @chobo = 0`); PyMySQL's keeps
  the server's status and needs no asking; on mysqlclient's, a transaction of yours is one with
  autocommit off. A connection serves one call at a time: give calls at the same time a pool.
  With `batch`, the statements a call sends in a row go in one request, and a call that checks a
  bound keeps its locks over one round trip.
- **Expiry.** As on PostgreSQL, a hold past its expiry answers `expired` at once, and what it holds
  counts until `expire()` lets it go: call it from a job of your own, one at a time.
- **Keys as bytes, names in characters.** A key and a tenant are compared as the bytes of their
  IDs, whatever the collation, and the values are written in hex, whatever the `sql_mode`. A name
  is 64 characters at most, with none beyond the Basic Multilingual Plane, so the build stops for a
  book's name the tables and checks could not be named after (E062).
- **TiDB and Vitess.** On TiDB, run the SQL with `tidb_enable_check_constraint` ON, or the tables
  keep no check; the clients keep the bounds either way. The SQL and the clients keep to what a
  service that runs Vitess allows. In a sharded keyspace, shard the three tables by `tenant`: a
  tenant's rows, and every call, are then on one shard, and rows of yours that commit with a call
  have to be on it too. The tests run on Vitess 24.0.5 (vttestserver), in one shard and in two.

## The clients

The clients of every database have the same names, arguments and answers, so changing the database
changes how the book value is made, and nothing else. A call answers `done`, `done_before` or a
refusal with its reason; the reasons are a type of their own, the bounds' first:

<!-- chobo build examples/inventory/inventory.book --target postgres-typescript -->
```ts
/** Why a call of inventory can be refused: the bounds' own reasons, then the ones chobo gives itself. */
export type Reason = "out_of_stock" | "key_conflict" | "already_refused" | "same_account" | "no_such_hold" | "already_posted" | "already_voided" | "expired" | "over_hold";

/** What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason says why. */
export type Result = { result: "done" } | { result: "done_before" } | { result: "refused"; reason: Reason };
```

In Rust the reasons are an enum, and the answer is another, a refusal one of its variants:

<!-- chobo build examples/inventory/inventory.book --target postgres-rust -->
```rust
/// What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason
/// says why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Done,
    DoneBefore,
    Refused(Reason),
}
```

- **A refusal is an answer, not an exception.** There is no stock: the caller handles
  `out_of_stock` as it handles any other answer. A failure is something else, an exception (an
  `error` in Go, an `Err` in Rust): a database that cannot be reached, an argument of the wrong
  type, an amount out of range.
- **A retry is safe.** The key is the book's, made of the transfer's own arguments, so the same call
  again answers `done_before` and moves nothing, whichever service or language sends it.
- **The book value.** In TypeScript, `postgres(db, { tenant })`, `tigerbeetle(client, { tenant })`
  and `mysql(db, { tenant, batch })` make it; in Python, `postgres(conn, tenant="")`,
  `tigerbeetle(client, tenant="")` and `mysql(conn, tenant="", batch=False)`, with the arguments
  as keyword arguments; in Go, `Postgres(q, tenant)`, `TigerBeetle(c, tenant)` and
  `MySQL(q, tenant, batch)`, with a `context.Context` first in every call; in Rust,
  `postgres(db, tenant)`, `tigerbeetle(client, tenant)` and `mysql(db, tenant, batch)`, every
  call an `async fn` (`book.reserve().hold(ReserveArgs { order: "o-1", sku: "A-1", qty: 2 })`). An
  amount is a `bigint`, an `int`, an `int64` or an `i64`.
- **Holds.** A transfer that holds has `hold`, `post` (all of it, or the amounts given), `void` and
  `status`; every account has a `balance`. On PostgreSQL and the MySQL family, the book value also
  has `expire()`.

A client imports the language's standard library and the database's own client library (pgx and
go-sql-driver/mysql in Go, sqlx and tokio in Rust, and TigerBeetle's in the four languages), and
nothing else: in TypeScript and Python, the clients of PostgreSQL and the MySQL family work through
the connection they are given. The TypeScript runs on Node.js as it is written and compiles under
`tsc --strict`; the Python passes `mypy --strict`; the Go passes `gofmt` and `go vet`; the Rust
builds on the 2021 and the 2024 editions with no warning, clippy's lints among them, and its head
says the lines of Cargo.toml it needs.

## IDs

On TigerBeetle, every ID is made from what the book says, hashed with SHA-256: an account's from the
book's name, the tenant, the account's kind and its arguments, and a transfer's from the book's
name, the tenant, the transfer's kind, the operation, the values of its key and its place in the
chain. So a retry sends the same IDs, and TigerBeetle answers that the transfer exists. The four
languages and the reference interpreter make the same IDs, and the tests see that they do, on every
operation of every scenario. On PostgreSQL and the MySQL family the accounts and holds carry the same
IDs; on the MySQL family a call and a tenant have IDs made the same way, so that keys and tenants
are compared as bytes. `chobo api` gives the recipe, for a tool that works one out itself
([Formats](formats.md#the-api-of-a-book)).
