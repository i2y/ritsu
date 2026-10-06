# chobo

**Put bounds on accounts. Every transfer keeps them, in one write.**

chobo is a small language for the things you count and move between accounts: stock, money,
points, seats. A book (`.book`) declares the accounts, a lower or an upper bound on each, and the
kinds of transfer that move things between them, with the key that makes each call once only and
the holds that wait to be posted or voided. chobo checks the book, runs it in a reference
interpreter, and builds it for PostgreSQL or TigerBeetle, with a client in TypeScript, Python or
Go.

The bound of one account is the only condition a book can write, and that is on purpose. A
bound looks at one account, so the write that changes the account can check it, with nothing to
read in between: on PostgreSQL, a function that locks the account's row; on TigerBeetle, a flag
of the account and a chain of transfers applied whole or not at all. A rule over two accounts
cannot be kept that way, since two calls at the same time can each read the other's balance
before either writes, so it is written as the bound of an account of its own. *A refund never
exceeds the sale* becomes an account of what is left to refund on each order, which the sale
fills, a refund takes from, and which never goes below 0:

```book
book refunds v1
description "What is left to refund on each order is an account of its own: the sale adds to it, a refund takes from it, and it never goes below 0. A refund is held while it waits for approval"

unit USD scale 2

account refundable(order: string) : USD
  description "what is left to refund on the order"
  at least 0 refused as refund_exceeds_sale
account sales : USD outside
account refunded : USD outside

transfer sale(order: string, amount: USD)
  key order
  move amount from sales to refundable(order)

transfer refund(request: string, order: string, amount: USD)
  description "held while it waits for approval: posted when it is approved, voided when it is turned down"
  key request
  pending expires after 7 days
  move amount from refundable(order) to refunded
```

The name comes from 帳簿 (chōbo), a book of accounts.

## What the checker says

Before anything runs, `chobo check` lists what each operation of each transfer can be refused
with: the reasons the bounds give, and the ones chobo gives itself, for a key used again with
other arguments, a hold posted after it was voided, or one that has expired.

```console
$ chobo check examples/refunds/refunds.book
examples/refunds/refunds.book: ok
  sale.do      may be refused: key_conflict
  refund.hold  may be refused: refund_exceeds_sale (refundable(order) at least 0.00), key_conflict, already_refused
  refund.post  may be refused: key_conflict, already_voided, expired, over_hold, no_such_hold
  refund.void  may be refused: already_posted, expired, no_such_hold
  key: sale once per order; a second call that differs only in amount is refused with key_conflict
  key: refund once per request; a second call that differs only in order or amount is refused with key_conflict; holding again with the same key after the hold has ended answers done_before and holds nothing
```

Each reason is listed because the checker found the operations that get there and ran them in
the reference interpreter; `--format json` gives those operations, and so does `chobo api`, for
a workflow whose task declares the errors it comes back with. A diagnostic comes with them too.
Here a sale takes the marketplace's fee from the shop's balance before it puts the price in
([tests/fixtures/split.book](tests/fixtures/split.book)):

```text
warning[W103]: split.book:13:3: move 1 takes from shop_balance(shop) before move 2 puts into it: when shop_balance(shop) is short at that point, the call is refused with insufficient_balance, even when the two moves together would leave enough
    13 |   move fee from shop_balance(shop) to fees
  the operations that get there:
       1  sale.do(order: order-1, shop: shop-2, price: 1, fee: 1)  refused: insufficient_balance (move 1 takes 1 from shop_balance(shop-2): posted 0, held out 0)
  hint: write the move that puts into shop_balance(shop) first
```

[docs/codes.md](docs/codes.md) has all 30 codes, as `chobo explain --all` prints them.

## Two refunds at the same time

Two refunds of 20.00 come for a sale of 30.00 at the same moment
([examples/refunds/refunds.more.json](examples/refunds/refunds.more.json); the amounts are in
cents, the smallest step of `USD scale 2`). The reference interpreter tries every order the two
can be dealt with in, and says every way it can come out:

```console
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json
examples/refunds/refunds.book (more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00): 3 steps, 2 possible outcomes
outcome 1:
    1  sale.do(order: A-100, amount: 3000)                                done
    2  together
         caller 1: refund.hold(request: R-1, order: A-100, amount: 2000)  done
         caller 2: refund.hold(request: R-2, order: A-100, amount: 2000)  refused: refund_exceeds_sale
    3  refund.hold(request: R-3, order: A-100, amount: 1000)              done
  accounts:
    refundable(A-100)  posted 3000, held out 3000, held in 0
    sales              posted -3000, held out 0, held in 0
    refunded           posted 0, held out 0, held in 3000
  holds:
    refund(R-1)  held
    refund(R-3)  held
outcome 2:
…
```

Whichever comes first is held, the other is refused, and a refund of the 10.00 left still goes
through. Each database must answer one of these outcomes, and the tests see that it does.

## Money with tax or without

A unit named for a currency is money, and it may end with whether its amounts are with tax or
without:

```book
unit 円 incl_tax
```

Inside the book nothing changes: amounts are counted, bounded and built as they are without it.
It is for where an amount comes into the book from a rule or a workflow of the same toolchain.
The book hands its units over as the units the rules use, so a unit with tax is a rule's
`money[円, incl_tax]`: it is to take amounts with tax, a unit without tax amounts without, and a
unit that says neither amounts that say neither, so that one account does not end up holding
both. A unit that is not money takes neither (E014): `pcs` and `seats` are counts with a name and
nothing else, never the same as each other, and `kg` is the kilogram the rules know.

## Targets

```console
$ chobo build examples/refunds/refunds.book --target postgres --out db
```

| Target | What `build` writes |
|---|---|
| `postgres` | a schema, its tables and constraints, and a function for each operation, in SQL |
| `postgres-typescript`, `postgres-python`, `postgres-go` | a client that calls those functions |
| `tigerbeetle-typescript`, `tigerbeetle-python`, `tigerbeetle-go` | a client that sends TigerBeetle one chain of transfers for each operation |

The clients of both databases have the same names, arguments and answers. A refusal is an
answer, `{ result: "refused", reason: "refund_exceeds_sale" }`, not an exception; the same call
again answers `done_before`, so a retry never moves twice. On PostgreSQL, a function runs in the
caller's transaction, so an order and the hold of its stock commit together. TigerBeetle runs no
code of yours, so its client builds the chain itself: a lower bound of 0 is a flag of the
account, and any other bound is kept by transfers chobo adds to the same chain.
[docs/targets.md](docs/targets.md) has what each target writes and how to call it, from all
three languages.

## For the people who keep the accounts

`chobo doc` writes the book as a page for accounting and operations: the accounts and their
bounds, a chart of how things move between them, each transfer with its moves, its key and what
each operation can be refused with, the life of a hold, and every scenario with the balances
after each step. In Markdown, with Mermaid charts that GitHub draws, or as one HTML page whose
charts chobo draws itself and whose scenarios can be stepped through, in English or, with
`--lang ja`, Japanese. Each example has its page beside it: the
[refunds](examples/refunds/doc.md), and the same book in Japanese, [返金](examples/refunds/doc.ja.md).

## For AI agents

[skills/chobo](../../skills/chobo) is an [Agent Skill](https://agentskills.io) for using chobo: the
loop from a first draft to a build, the language on one page, what to ask a person (the bounds
and the names of their reasons, the keys, how long a hold lasts, which accounts are outside the
book), and the fix for each diagnostic. Copy it into `~/.claude/skills/`, or into a project's
`.claude/skills/`; [skills/README.md](skills/README.md) says more.

## Install

chobo is one of the languages of [ritsu](https://github.com/i2y/ritsu), and is built from its
repository with a recent stable Rust. To have all eight, and `ritsu check` for a project that
holds the files of more than one:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu chobo <command>` is then every command below, and a link to `ritsu` named `chobo` does the
same. To have chobo alone, which reads no other language:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked chobo
```

chobo's one dependency is serde_json.

## Commands

```
chobo check <file.book>... [--format json] [--diff-base <rev>]
chobo run <file.book> --scenario <file.json> [--show postgres|tigerbeetle] [--format json]
chobo scenarios <file.book> [--out <dir>]
chobo build <file.book> --target <target> [--out <dir>]
chobo doc <file.book> [--format html] [--out <dir>]
chobo api <file.book>
chobo explain <code> | --all [--format markdown]
```

`--lang ja` prints the messages in Japanese. `--diff-base <rev>` compares the book with the one
at a git revision, and refuses a change the balances and holds already in a database could not
follow. `chobo api` prints how to call the book, what each operation can be refused with and the
life of a hold as a state machine, as JSON for another tool ([docs/formats.md](docs/formats.md)).

## Examples

[examples/](examples/) has four, each in English with its Japanese twin beside it
(`inventory.ja.book`): the stock of an order held for 30 minutes, posted when it ships and voided
when it is cancelled; loyalty points, earned on a hold that waits out the return period, spent
on a hold that waits for the payment, and lapsing; refunds that never exceed the sale; and a
marketplace that splits what a buyer paid between the shop and its fee, in two moves, all or
none. The whole language is on [docs/reference.md](docs/reference.md).

## How it is checked

The reference interpreter defines what a book means. The tests make the scenarios of every
example and every test book, 468 of them with the ones written by hand, 38 with callers at the
same time, and run each one on seven targets: the SQL itself through psql, the PostgreSQL
clients in TypeScript, Python and Go, and the TigerBeetle clients in the three. Each answer, each
balance at the end and the state of each hold must be the reference interpreter's (one of its
outcomes, when callers come at the same time), and every client must send what `chobo run
--show` says, field by field, so the three languages make the same IDs. PostgreSQL is also run
with four sessions crossing the same accounts, to see that no deadlock comes. The generated
TypeScript passes `tsc --strict`, the Python `py_compile`, and the Go `gofmt` and `go vet`. The
pages `chobo doc` writes are golden files; their Mermaid charts draw in Mermaid 11 and 12, and in
Chrome an HTML page shows the balances the reference interpreter left after each step.

```console
$ cargo test
```

uses what it finds of PostgreSQL (on the PATH, or `CHOBO_PG_BIN`), TigerBeetle
(`tools/tigerbeetle/fetch.sh`, or `CHOBO_TIGERBEETLE`), Node.js and Python with what
`tools/runner` lists, Go, Chrome (`CHOBO_CHROME`) and Mermaid (`npm ci --prefix tools/mermaid`),
and prints a `SKIP:` line for each it does not find.

## Status

Early. Not yet: changing the bound of an account kind that already has balances (a new kind and
a transfer that moves the balance do it for now); the check that holds an amount a rule hands over
to the tax of the unit it goes into (the units carry the tax already); dandori's side, a workflow that calls a book's
transfers as tasks and follows its holds as cases (`chobo api` writes the state machine for it);
databases other than PostgreSQL and TigerBeetle; TigerBeetle in its production layout of six
replicas (the tests use one, started with `--development`), and how much the IDs made from keys
slow it down; reading the entries and transfers an account has had; and the two answers on
which TigerBeetle differs from the reference interpreter, which [docs/targets.md](docs/targets.md)
describes. The design, the decisions and what is left are in [DESIGN.md](DESIGN.md), in
Japanese.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
