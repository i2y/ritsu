---
title: "Put bounds on accounts. Every transfer keeps them, in one write."
hide:
  - navigation
  - toc
---

<div class="cb-hero" markdown>
<img class="cb-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="cb-hero__mark" src="images/mark-light.svg#only-light" alt="">

# chobo

<p class="cb-hero__tag">Put bounds on accounts. Every transfer keeps them, in one write.</p>

<p class="cb-hero__lede">
<strong>A small language for the things you count and move between accounts</strong>: stock,
money, points, seats. A book declares the accounts, a lower or an upper bound on each, and the
kinds of transfer that move things between them, with the key that makes each call once only and
the holds that wait to be posted or voided.
</p>

<p class="cb-hero__lede">
<strong>The bound of one account is the only condition.</strong> A bound looks at one account, so
the write that changes the account can check it, with nothing to read in between. A condition
over two accounts, such as a refund that never exceeds the sale, is written as the bound of an
account of its own.
</p>

<p class="cb-hero__lede">
<strong>Every target, one meaning.</strong> chobo checks a book, runs it in a reference
interpreter, and builds it for PostgreSQL, for TigerBeetle or for the MySQL family (MySQL, TiDB
and Vitess), with a client in TypeScript, Python, Go or Rust. The tests run every scenario through every
client, and each must answer as the reference interpreter does.
</p>

<div class="cb-hero__cta" markdown>
[Try it in the browser](../playground/#project=chobo/refunds){ .md-button .md-button--primary }
[Install](install.md){ .md-button }
[Write a book](tour.md){ .md-button }
[Examples](examples.md){ .md-button }
[GitHub](https://github.com/i2y/ritsu/tree/main/crates/chobo){ .md-button }
</div>
</div>

<div class="cb-chart" markdown>
![The refunds book as chobo doc draws it. Each sale moves its amount from sales, an account outside the book, into refundable(order), which is at least 0.00 and refuses with refund_exceeds_sale. A refund, a transfer that holds first and so a dashed arrow, takes from refundable(order) into refunded, outside the book too](images/chart.svg#only-dark)
![The refunds book as chobo doc draws it. Each sale moves its amount from sales, an account outside the book, into refundable(order), which is at least 0.00 and refuses with refund_exceeds_sale. A refund, a transfer that holds first and so a dashed arrow, takes from refundable(order) into refunded, outside the book too](images/chart-light.svg#only-light)
</div>

The book below, as `chobo doc` draws it: a box is an account, and an arrow a transfer. A dashed
box is outside the book, with no bounds; a dashed arrow holds first, and moves when the hold is
posted.

---

## What chobo does

<div class="cb-row" markdown>
<div markdown>

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

</div>
<div markdown>

### A condition over two accounts becomes the bound of one

*A refund never exceeds the sale* compares two amounts, and two refunds at the same time could
each read the sale before the other writes. So the book keeps what is left to refund on each
order as an account of its own: the sale fills it, a refund takes from it, and it never goes
below 0. Two refunds now change one account, and the second is checked against what the first
left. [Write a book](tour.md)

</div>
</div>

<div class="cb-row cb-row--flip" markdown>
<div markdown>

```console
$ chobo check examples/refunds/refunds.book
examples/refunds/refunds.book: ok
  sale.do      may be refused: key_conflict
  refund.hold  may be refused: refund_exceeds_sale (refundable(order) at least 0.00), key_conflict, already_refused
  refund.post  may be refused: key_conflict, already_voided, expired, over_hold, no_such_hold
  refund.void  may be refused: already_posted, expired, no_such_hold
…
```

</div>
<div markdown>

### Every refusal, found before anything runs

`chobo check` lists what each operation of each transfer can be refused with: the reasons the
bounds give, and the ones chobo gives itself, for a key used again with other arguments, a hold
posted after it was voided, or one that has expired. A reason is listed because the checker
found the operations that get there and ran them in the reference interpreter.
[What it checks](checks.md)

</div>
</div>

<div class="cb-row" markdown>
<div markdown>

```console
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json
examples/refunds/refunds.book (more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00): 3 steps, 2 possible outcomes
outcome 1:
    1  sale.do(order: A-100, amount: 3000)                                done
    2  together
         caller 1: refund.hold(request: R-1, order: A-100, amount: 2000)  done
         caller 2: refund.hold(request: R-2, order: A-100, amount: 2000)  refused: refund_exceeds_sale
    3  refund.hold(request: R-3, order: A-100, amount: 1000)              done
…
```

</div>
<div markdown>

### Two callers at the same moment

Two refunds of 20.00 come for a sale of 30.00 at once. The reference interpreter tries every
order the two can be dealt with in, and gives every way it can come out: whichever comes first
is held, and the other is refused. Each database must answer one of these outcomes, and the
tests see that it does. [Run scenarios](scenarios.md)

</div>
</div>

<div class="cb-row cb-row--flip" markdown>
<div markdown>

```console
$ chobo build examples/refunds/refunds.book --target postgres --out db
$ chobo build examples/refunds/refunds.book --target postgres-typescript --out db
$ chobo build examples/refunds/refunds.book --target tigerbeetle-python --out db
$ chobo build examples/refunds/refunds.book --target mysql --out db
$ chobo build examples/refunds/refunds.book --target mysql-go --out db
$ chobo build examples/refunds/refunds.book --target postgres-rust --out db
```

</div>
<div markdown>

### One book, three kinds of database, four languages

On PostgreSQL, each operation is a function that locks the rows of its accounts, checks every
move and writes them in one transaction, the caller's if there is one. On TigerBeetle, the client
sends one chain of transfers that is applied whole or not at all. On MySQL, TiDB and Vitess, the
client sends the statements chobo wrote in one transaction, the caller's if it is given one, and a
locking read of the accounts says which move would break a bound. The clients of all three have
the same names, arguments and answers in every language, and a refusal is an answer, not an
exception.
[Build for a database](databases.md)

</div>
</div>

<div class="cb-row" markdown>
<div markdown>

```console
$ chobo doc examples/refunds/refunds.book --out pages
$ chobo doc examples/refunds/refunds.book --format html --out pages
```

</div>
<div markdown>

### A page for the people who keep the accounts

`chobo doc` writes the book for accounting and operations: the accounts and their bounds, the
chart above, each transfer with what it can be refused with and why, the life of a hold, and every
scenario with the balances after each step. In Markdown, with charts GitHub draws, or as one HTML
page whose scenarios step through. [The refunds, drawn](doc/refunds.html) ·
[Pages for people](pages.md)

</div>
</div>

<div class="cb-row cb-row--flip" markdown>
<div markdown>

```flow
use book stock from "books/stock.book"
…
task reserve(order: string, sku: string, qty: int) -> stock.reserve
  book stock.reserve.hold
  starts stock.reserve
  errors out_of_stock
…
case hold : stock.reserve follows stock.reserve
```

</div>
<div markdown>

### Called from a workflow, guarded by a gate

A dandori workflow runs a book's operations as tasks, declares the reasons they can be refused
with as the task's errors, and follows a hold as a case whose expiry the checker counts. A sekisho
gate guards each operation, a yuen requirement can be satisfied by one, and an amount a rulec rule
works out can be a transfer's, where `ritsu check` holds the task to every refusal that amount can
come to.
[With the other languages](together.md)

</div>
</div>

## Status

Early. Not yet: changing the bound of an account kind that already has balances (a new kind and a
transfer that moves the balance do it for now); databases other than PostgreSQL, TigerBeetle and
the MySQL family (MariaDB, whose `CHECK` and JSON behave otherwise than MySQL's, is not one of the
family); a service that runs Vitess, where the tests have not been (they run on vttestserver);
TigerBeetle in its production layout of six replicas (the tests use one, started
with `--development`), and how much the IDs made from keys slow it down; reading the entries and
transfers an account has had; and the two answers on which TigerBeetle differs from the reference
interpreter, which [Targets](targets.md#tigerbeetle) describes. The design, the decisions and what
is left are in [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/chobo/DESIGN.md), in
Japanese; its main decisions are on [Design](design.md). chobo is one of the languages of
[ritsu](../), and is licensed under either of the Apache License 2.0 or the MIT license, at your
option; its table of the characters a Rust name cannot hold is made from the Unicode Character
Database, under the Unicode License V3
([THIRD_PARTY_NOTICES](https://github.com/i2y/ritsu/blob/main/crates/chobo/THIRD_PARTY_NOTICES)).

## Where to read next

| Page | What is on it |
|---|---|
| [Write a book](tour.md) | the language, through the marketplace example: units, accounts and bounds, transfers, keys and holds |
| [What it checks](checks.md) | what each operation can be refused with, the diagnostics, and changes the data cannot follow |
| [Run scenarios](scenarios.md) | the reference interpreter, callers at the same time, and the scenarios chobo writes |
| [Build for a database](databases.md) | what PostgreSQL, TigerBeetle and the MySQL family are given, and how a client is called |
| [Money and units](units.md) | units, money with tax or without, and the units the other languages see |
| [Pages for people](pages.md) | `chobo doc`, and the four examples drawn |
| [With the other languages](together.md) | a workflow, a gate, a requirement and a rule around a book |
| [Examples](examples.md) | stock, points, refunds and a marketplace, in English and in Japanese |
| [For agents](agents.md) | the agent skill |
| [How it is checked](assurance.md) | every client, on PostgreSQL, TigerBeetle, MySQL, TiDB and Vitess, against one reference interpreter |
| [Design](design.md) | the decisions behind the language, and why |
