# How it is checked

The reference interpreter says what a book means: `chobo run` is it. Everything else chobo makes,
the SQL and the twelve clients, is held to it by running the same scenarios on the real databases,
and what the checker and the page for people say of a book is what it found by running the
reference interpreter.

## Every client, one meaning

The tests make the scenarios of every example and every test book, 468 of them with the ones
written by hand, 38 with callers at the same time, and run each one through every client:

| Target | How it is run |
|---|---|
| the SQL itself | each operation as a call of its function, through psql |
| the PostgreSQL clients in TypeScript, Python, Go and Rust | each through its driver: `pg`, psycopg, pgx, sqlx |
| the TigerBeetle clients in TypeScript, Python, Go and Rust | each through TigerBeetle's own client |
| the clients of the MySQL family in TypeScript, Python, Go and Rust | each through its driver (mysql2, PyMySQL, go-sql-driver/mysql, sqlx), on MySQL, on TiDB and on Vitess in one shard and in two, one statement at a time and then in batches |

Each answer, each balance at the end and the state of each hold must be the reference
interpreter's; with callers at the same time, one of the ways it can come out. The callers of a
`together` are sent at the same moment, from connections (and on TigerBeetle, clients) of their
own. Every client must send what `chobo run --show` says, field by field and statement by
statement, so that the four languages make the same calls and the same IDs; batches are held to
the answers alone, as a request of them also carries the statements an error keeps from running.

The databases are real ones: a PostgreSQL cluster listening on a socket alone, one TigerBeetle
replica started with `--development`, a MySQL server and two of Vitess's vttestserver, each started
for the tests and thrown away after, and a TiDB cluster the machine runs, in which the tests make
databases of their own and drop them. One Vitess keeps its keyspace in one shard; the other shards
the tables by the tenant into two, and fails a transaction that crosses shards, so that a call
that did not keep to its tenant's shard would fail. At the end, every balance on PostgreSQL is
what the account's entries add up to, and every row of an account on MySQL, TiDB and Vitess is
what the entries added to it add up to. Each
scenario runs under a tenant of its own, so all of them run at once. A hold in the tests lasts 3
seconds whatever the book says, and the runners really wait for it to expire. On a slow machine, a
step can come after a hold's expiry where the reference interpreter's clock says it does not; a
scenario that differs for that reason is run again, with the others that were late, and fails only
when it is late and differs again.

## What only one kind of database has

- **PostgreSQL**: four sessions move between the same accounts in opposite directions, a hundred
  times each, and no deadlock comes. Under REPEATABLE READ, four callers put into one account fifty
  times each, and every client tries a failed serialization again until all two hundred calls are
  done. An `UPDATE` that does not go through the functions is stopped by the `CHECK` constraints,
  and a row whose bounds were changed stops a call with `CB001`. `expire()` gives back what it
  says, and every balance is the sum of the account's `entries`. The IDs `chobo_id` makes are the
  ones the other targets make.
- **TigerBeetle**: a book whose upper bound was changed, called with the same tenant, fails on the
  opening transfer of the bound. A post of a hold that is not there answers `no_such_hold` without
  sending anything, and the hold can be made and posted with the same key after it. Both in all
  four languages.
- **MySQL, TiDB and Vitess** (in two shards), through the TypeScript client, with a book in English
  and one in Japanese,
  whose tables have Japanese names: four callers move between the same accounts in opposite
  directions, a hundred times each, and no deadlock comes and no call is made again. An `UPDATE`
  that does not go through the clients is stopped by the checks (on TiDB, in tables made with
  `tidb_enable_check_constraint` ON; made with it OFF, the tables keep none), and a row whose
  bounds were changed stops a call with `CB001`. `expire()` gives back what it says and adds the
  entries of the accounts without bounds to their rows. Eight callers after the last 100 pieces,
  fifty calls each, get exactly 100 `done`, one statement at a time and in batches. A call given a
  connection in the caller's transaction commits and rolls back with it; in TypeScript, Python and
  Go, a call on a connection in no transaction is a transaction of its own, and one in a
  transaction the caller began, or with autocommit off, goes with its rollback, an account row it
  made too, which the next call makes again; in Rust, a call on a connection of the pool in no
  transaction is a transaction of its own, and one on a `Transaction` of sqlx's goes with its
  rollback, with the caller's own row. A batch answers as one statement at a time does, in
  fewer requests. Keys that differ only in case, in spaces at the end or in a dakuten are other
  calls, and tenants that differ only in case are other balances; keys with quotes and backslashes
  go through under the `sql_mode`s `ANSI_QUOTES` and `NO_BACKSLASH_ESCAPES` (Vitess lets no session
  set them: there they go through under the mode it keeps).
- A book too large for one TigerBeetle request does not build (E060), nor one with a name too long
  for PostgreSQL (E061), nor one whose name MySQL cannot hold (E062), nor, for a Rust target, one
  with a name Rust cannot write (E063).

## The code it writes

The generated TypeScript passes `tsc --strict`, with `erasableSyntaxOnly` and `noUnusedLocals`; the
Python `py_compile`, and ritsu's tests put it through `mypy --strict`; the Go `gofmt` and `go vet`.
The Rust builds on the 2021 and the 2024 editions with no warning, clippy's lints among them, and
in a package of ritsu's too, where it takes the types of its units; the crates its head names are
the versions the tests lock, and each character E063 keeps out of a name is one rustc keeps out,
of every character a name of chobo's can hold. The tests run them with Node.js 23.11, Python 3.13,
Go 1.26 and Rust 1.94, on PostgreSQL 18.0, TigerBeetle 0.17.9, MySQL 8.4.11, TiDB 8.5.8 and
Vitess 24.0.5.

## The diagnostics

Every diagnostic of the test books under `tests/fixtures` is held to a golden file, in English and
in Japanese, and every code has a book of its own that `chobo explain` prints and the tests see
producing that code. Each test book with a Japanese name has an English twin, which must give the
same codes and stop or pass alike. The operations a diagnostic or the report gives are run in the
reference interpreter before they are said, and a reason that no operations reach is not listed.

## The pages for people

The pages `chobo doc` writes for the examples and the test books are golden files. Every step a
page shows has the balances the reference interpreter left after it. Every Mermaid chart draws in
Mermaid 11 and in Mermaid 12, and in Chrome, an HTML page moved to a step shows the balances its
data says for that step, and lights up the transfers the step called. The pages on this site
are what `chobo doc` writes now, byte for byte.

## Proofs in Lean

The reference interpreter is written again as a model in Lean 4 (`ChoboModel`, in ritsu's
`proofs/`), and the theorems are about that model:

- no call that goes through, and no time going by, takes an account across a bound in the
  direction the bound forbids, so an account within its bounds stays within them, whatever is
  called;
- a refused call changes no balance and no hold, every move or none;
- the same `do` or `hold` called again answers `done_before` and changes nothing, so a retry never
  moves twice.

The model is run beside the reference interpreter on every scenario of every example and test
book, and the two must answer the same, step by step, account by account and hold by hold. None of
the proofs rests on `sorry` or on an axiom of its own.

## The documents

The README, the reference, the agent skill and the pages of this site are held to the tool: every
diagnostic shown is in a golden file, every line of a book is a line of a real book, every `chobo`
command shown with its output prints that output now, every relative link leads somewhere, and
the counts they give, such as the 468 scenarios above and the 32 codes, are the counts.

## Running them

```console
$ cargo test -p chobo
```

in ritsu's repository uses what it finds of PostgreSQL (on the path, or `CHOBO_PG_BIN`),
TigerBeetle (`crates/chobo/tools/tigerbeetle/fetch.sh`, or `CHOBO_TIGERBEETLE`), MySQL
(`crates/chobo/tools/mysql/fetch.sh`, or `RITSU_MYSQL_BIN`), a TiDB cluster (`RITSU_TIDB`, its
`host:port`), Node.js and Python with what `crates/chobo/tools/runner` lists, Go, Rust with
clippy and the crates of `crates/chobo/tools/runner/rust` (cargo fetches them), Chrome
(`CHOBO_CHROME`) and Mermaid (`npm ci --prefix crates/chobo/tools/mermaid`), and prints a `SKIP:`
line for each it does not find.
