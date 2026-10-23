# Run scenarios

The reference interpreter says what a book means. `chobo run` runs a scenario in it: a list of
operations, with the clock moving on between them and callers coming at the same time, and prints
what each operation answered and the accounts and holds at the end. `chobo scenarios` writes the
scenarios that try a book's bounds, keys and holds, and every database chobo builds for is held to
the reference interpreter on all of them.

## A scenario

The refunds example has one written by hand beside it,
[examples/refunds/refunds.more.json](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/refunds/refunds.more.json):
a sale of 30.00, two refunds of 20.00 at the same time, and then one of 10.00.

```json
[
  {
    "name": "more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00",
    "book": "refunds",
    "steps": [
      {"op": "do", "kind": "sale", "args": {"order": "A-100", "amount": 3000}},
      {"op": "together", "callers": [
        [{"op": "hold", "kind": "refund", "args": {"request": "R-1", "order": "A-100", "amount": 2000}}],
        [{"op": "hold", "kind": "refund", "args": {"request": "R-2", "order": "A-100", "amount": 2000}}]
      ]},
      {"op": "hold", "kind": "refund", "args": {"request": "R-3", "order": "A-100", "amount": 1000}}
    ]
  }
]
```

- A step is an operation (`do`, `hold`, `post` or `void`) of a transfer (`kind`), with its
  arguments by name: `do` and `hold` take every parameter, `post` and `void` the key alone, and
  `post` takes `amounts` too, to post less than the hold holds.
- An amount is a whole number in the smallest step of its unit: 3000 is 30.00 in `USD scale 2`.
- `{"op": "pass", "duration": "31 minutes"}` moves the clock on.
- `{"op": "together", "callers": [[…], […]]}` holds the operations of each caller, in order: eight
  at most, and no `pass` or `together` among them.

A file can hold one scenario, or a list of them, as `chobo scenarios` prints them.

## Callers at the same time

Each operation is dealt with whole, one at a time, so two callers at the same moment come out as one
order or the other. The reference interpreter tries every order the callers' operations can
interleave in, keeping each caller's own order, and gives every distinct way the scenario can come
out:

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
    1  sale.do(order: A-100, amount: 3000)                                done
    2  together
         caller 1: refund.hold(request: R-1, order: A-100, amount: 2000)  refused: refund_exceeds_sale
         caller 2: refund.hold(request: R-2, order: A-100, amount: 2000)  done
    3  refund.hold(request: R-3, order: A-100, amount: 1000)              done
  accounts:
    refundable(A-100)  posted 3000, held out 3000, held in 0
    sales              posted -3000, held out 0, held in 0
    refunded           posted 0, held out 0, held in 3000
  holds:
    refund(R-2)  held
    refund(R-3)  held
```

Whichever comes first is held, and the other is refused; the refund of the 10.00 left goes through
either way. The balances are three numbers: what is posted, what holds take out (held out) and what
they put in (held in). What a refund holds stays held out of `refundable(A-100)` until the hold
ends, so the next refund is checked with it counted. When a database runs this scenario, with the two callers on two connections at once,
its answers must be one of the two outcomes.

## Time going by

The interpreter's clock is in seconds from the start of the scenario, and only `pass` moves it.
When it reaches a hold's expiry, the hold expires then, and what it held goes back. A hold of the
English test book `stock_reservation` lasts 30 minutes, the same as the inventory example's:

```console
$ chobo run tests/books/stock_reservation.book --scenario tests/books/stock_reservation.more.json
…
tests/books/stock_reservation.book (more: reserve expired, then posted for more than it held): 4 steps
  1  receive.do(delivery: delivery-1, sku: sku-2, qty: 5)  done
  2  reserve.hold(order: order-3, sku: sku-2, qty: 2)      done
  3  pass 31 minutes                                       reserve(order-3, sku-2) expired
  4  reserve.post(order: order-3, sku: sku-2, qty: 3)      refused: expired
accounts:
  stock(sku-2)  posted 5, held out 0, held in 0
  suppliers     posted -5, held out 0, held in 0
  customers     posted 0, held out 0, held in 0
holds:
  reserve(order-3, sku-2)  expired
…
```

A post after the expiry is refused with `expired`, whatever it asks for. On TigerBeetle a hold
expires on the cluster's clock; PostgreSQL and the MySQL family have none that acts by itself, so
a hold past its expiry answers `expired` at once, and what it holds goes back when `expire()` is
next called ([Build for a database](databases.md#postgresql)).

## The scenarios chobo writes

`chobo scenarios` writes the scenarios of a book, as one list or, with `--out`, a file each
(`001.json`, …):

```console
$ chobo scenarios examples/refunds/refunds.book
[
  {
    "name": "bound: refund.hold takes refundable(order) to 0.01, one above `at least 0.00`",
…
    "name": "key: refund.hold refused with refund_exceeds_sale, then again, and again once refundable(order) has enough",
…
    "name": "hold: refund posted before it is held, then held and posted",
…
    "name": "pass: refund held again with the same key after it expired",
…
    "name": "together: two callers take the last 0.01 of refundable(order) with refund.hold",
…
$ chobo scenarios examples/refunds/refunds.book --out scenarios
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json --format json
```

What each tries is the start of its name:

- `bound:` each bound, with the account just before it, at it and past it.
- `key:` each key twice with the same arguments and with others, and a key a bound refused, called
  again, and again once the account has enough.
- `hold:` every way a hold ends: posted in full and in part, voided, posted then voided and voided
  then posted, posted for more than it holds, posted before it is held, posted twice.
- `pass:` a hold that expires, and is then posted and voided; and held again with its key.
- `moves:` a transfer of two moves or more, refused partway through, with nothing moved.
- `together:` two callers after the last of something, at the same moment.
- `same_account:` a move whose two accounts come out the same, refused with `same_account`.

Each scenario is built by running it, and kept only when it comes out as its name says. The string
values are a parameter's name and a number, numbered as they first appear in the scenario, so that no two
meet by chance; the numbers in a name are written in the unit's decimal places. The page
`chobo doc` writes shows every one of them, step by step ([Pages for people](pages.md)).

## What a database is sent

`--show postgres`, `--show tigerbeetle` and `--show mysql` print, in place of the answers, what the
clients `chobo build` writes send each target for the scenario: the SQL and its arguments for
PostgreSQL; for TigerBeetle the accounts made, the opening transfers and the chain of each
operation, field by field; and for the MySQL family the statements of each operation, with the
values in place ([Build for a database](databases.md#mysql-tidb-and-vitess)).

```console
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json --show postgres
examples/refunds/refunds.book (more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00): 3 steps, as sent to PostgreSQL (tenant "")
  1  sale.do(order: A-100, amount: 3000)
       select * from "refunds"."sale_do"('', 'A-100', 3000)
  2  together, caller 1: refund.hold(request: R-1, order: A-100, amount: 2000)
       select * from "refunds"."refund_hold"('', 'R-1', 'A-100', 2000)
  2  together, caller 2: refund.hold(request: R-2, order: A-100, amount: 2000)
       select * from "refunds"."refund_hold"('', 'R-2', 'A-100', 2000)
  3  refund.hold(request: R-3, order: A-100, amount: 1000)
       select * from "refunds"."refund_hold"('', 'R-3', 'A-100', 1000)
```

The tests hold the clients of all four languages to it on every scenario, so the four send the
same calls, and on TigerBeetle the same IDs, which are made from the keys. [Formats](formats.md) has
the JSON of a scenario, of its result (`--format json`) and of what is sent.
