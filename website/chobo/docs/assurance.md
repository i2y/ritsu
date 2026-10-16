# How it is checked

The reference interpreter says what a book means: `chobo run` is it. Everything else chobo makes,
the SQL and the six clients, is held to it by running the same scenarios on the real databases,
and what the checker and the page for people say of a book is what it found by running the
reference interpreter.

## Seven targets, one meaning

The tests make the scenarios of every example and every test book, 468 of them with the ones
written by hand, 38 with callers at the same time, and run each one on seven targets:

| Target | How it is run |
|---|---|
| the SQL itself | each operation as a call of its function, through psql |
| the PostgreSQL clients in TypeScript, Python and Go | each through its driver: `pg`, psycopg, pgx |
| the TigerBeetle clients in TypeScript, Python and Go | each through TigerBeetle's own client |

Each answer, each balance at the end and the state of each hold must be the reference
interpreter's; with callers at the same time, one of the ways it can come out. The callers of a
`together` are sent at the same moment, from connections (and on TigerBeetle, clients) of their
own. Every client must send what `chobo run --show` says, field by field, so that the three
languages make the same calls and the same IDs.

The databases are real ones, started for the tests and thrown away after: a PostgreSQL cluster
listening on a socket alone, and one TigerBeetle replica started with `--development`. Each
scenario runs under a tenant of its own, so all of them run at once. A hold in the tests lasts 3
seconds whatever the book says, and the runners really wait for it to expire. On a slow machine, a
step can come after a hold's expiry where the reference interpreter's clock says it does not; a
scenario that differs for that reason is run again, with the others that were late, and fails only
when it is late and differs again.

## What only one database has

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
  three languages.
- A book too large for one TigerBeetle request does not build (E060), nor one with a name too long
  for PostgreSQL (E061).

## The code it writes

The generated TypeScript passes `tsc --strict`, with `erasableSyntaxOnly` and `noUnusedLocals`; the
Python `py_compile`, and ritsu's tests put it through `mypy --strict`; the Go `gofmt` and `go vet`.
The tests run them with Node.js 23.11, Python 3.13 and Go 1.26, on PostgreSQL 18.0 and TigerBeetle
0.17.9.

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
the counts they give, such as the 468 scenarios above and the 30 codes, are the counts.

## Running them

```console
$ cargo test -p chobo
```

in ritsu's repository uses what it finds of PostgreSQL (on the path, or `CHOBO_PG_BIN`),
TigerBeetle (`crates/chobo/tools/tigerbeetle/fetch.sh`, or `CHOBO_TIGERBEETLE`), Node.js and Python
with what `crates/chobo/tools/runner` lists, Go, Chrome (`CHOBO_CHROME`) and Mermaid
(`npm ci --prefix crates/chobo/tools/mermaid`), and prints a `SKIP:` line for each it does not
find.
