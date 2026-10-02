# What chobo reads and writes, as JSON

chobo reads one kind of JSON, a scenario, and writes four: the result of a scenario, the
diagnostics and the report of `chobo check --format json`, what `chobo api` says about a book,
and what a client sends a target (`chobo run --show`). The keys are English whatever `--lang`
says; only the prose in them (a diagnostic's title, its hint) follows the language. Every output
on this page is what chobo prints for the examples, cut where `…` stands.

## A scenario

`chobo scenarios <book>` prints the scenarios it makes as one list (or writes them one a file
with `--out <dir>`, from `001.json`), and `chobo run <book> --scenario <file>` takes one of them
or the whole list.

```console
$ chobo scenarios examples/inventory/inventory.book
[
  {
    "name": "bound: reserve.hold takes stock(sku) to 1, one above `at least 0`",
    "book": "inventory",
    "steps": [
      {
        "op": "do",
        "kind": "receive",
        "args": {
          "delivery": "delivery-1",
          "sku": "sku-2",
          "qty": 2
        }
      },
      {
        "op": "hold",
        "kind": "reserve",
        "args": {
          "order": "order-3",
          "sku": "sku-2",
          "qty": 1
        }
      }
    ]
  },
…
```

- A step is an operation (`do`, `hold`, `post`, `void`) on a transfer (`kind`), with its
  arguments by name. `do` and `hold` take every parameter; `post` and `void` take the key alone.
  `post` takes `amounts` too, every amount parameter or none (none posts all of it).
- An amount is a JSON integer, in the smallest step of its unit (cents for `USD scale 2`).
- `{"op": "pass", "duration": "31 minutes"}` moves the clock on: seconds, minutes, hours or
  days.
- `{"op": "together", "callers": [[…], […]]}` holds the operations of each caller, in order; a
  `together` holds eight operations at most, and no `pass` or `together`.
- The name of a scenario chobo makes starts with what it tries: `bound:`, `key:`, `hold:`,
  `pass:`, `moves:`, `together:` or `same_account:`. The string values are the parameter's name
  and a number, numbered in the order they first appear, so that no two values meet by chance.
- A scenario written by hand beside a book, as `<book>.more.json`, runs on every target too
  ([examples/refunds/refunds.more.json](../examples/refunds/refunds.more.json)).

## The result of a scenario

```console
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json --format json
{
  "outcomes": [
    {
      "steps": [
        {
          "op": "do",
          "kind": "sale",
          "result": "done"
        },
        {
          "op": "together",
          "callers": [
            [
              {
                "op": "hold",
                "kind": "refund",
                "result": "done"
              }
            ],
            [
              {
                "op": "hold",
                "kind": "refund",
                "result": "refused",
                "reason": "refund_exceeds_sale"
              }
            ]
          ]
        },
…
      "accounts": [
        {
          "account": "refundable",
          "args": [
            "A-100"
          ],
          "posted": 3000,
          "held_in": 0,
          "held_out": 3000
        },
…
      "holds": [
        {
          "kind": "refund",
          "key": [
            "R-1"
          ],
          "state": "held"
        },
…
```

- `steps` has one entry a step, in the order of the scenario; a `together` has its callers'
  answers in the order they are written, not the order they were dealt with.
- `accounts` has every account a `do` or a `hold` of the scenario names (outside accounts too,
  and those of refused calls), in the order the book declares the kinds, then of the arguments.
- `holds` has every hold made, in the order of the transfer kinds, then of the keys; its `state`
  is `held`, `posted`, `voided` or `expired`.
- With `together`, the result is `{"outcomes": [...]}`: every distinct way it can come out. A
  database's answer must be one of them.

## Diagnostics and the report

```console
$ chobo check tests/fixtures/split.book --format json
{
  "v": 1,
  "files": [
    {
      "file": "tests/fixtures/split.book",
      "ok": true,
      "diagnostics": [
        {
          "v": 1,
          "severity": "warning",
          "code": "W103",
          "file": "tests/fixtures/split.book",
          "line": 13,
          "column": 3,
          "title": "move 1 takes from shop_balance(shop) before move 2 puts into it: when shop_balance(shop) is short at that point, the call is refused with insufficient_balance, even when the two moves together would leave enough",
          "excerpt": "  move fee from shop_balance(shop) to fees",
          "notes": [],
          "operations": [
            {
              "op": "do",
              "kind": "sale",
              "args": {
                "order": "order-1",
                "shop": "shop-2",
                "price": 1,
                "fee": 1
              },
              "result": "refused",
              "reason": "insufficient_balance"
            }
          ],
          "hint": "write the move that puts into shop_balance(shop) first"
        }
      ],
      "report": [
        {
          "kind": "sale",
          "op": "do",
          "refusals": [
            {
              "name": "insufficient_balance",
              "because": "bound",
              "spends_key": true,
              "example": [
…
              "account": "shop_balance",
              "as_written": "shop_balance(shop)",
              "bound": "at least 0.00",
              "move": 1
            },
…
```

- A diagnostic has its `code`, `severity` (`error` or `warning`), `line` and `column`, the
  `title` and the `hint` in the language asked for, the source line it is about (`excerpt`), and
  the `operations` that get there, as scenario steps with what each answered.
- `ok` is false when the file has an error; the exit code is then 1.
- `report` is there when the book has no error: for each transfer and operation, the reasons it
  can be refused with (`refusals`), each with an `example` that gets there. `because` is `bound`,
  `key` (`key_conflict`, `already_refused`), `account` (`same_account`) or `hold`; a bound's
  refusal names the account kind, the account as the move writes it, the bound and the move.
  `spends_key` says whether the key is used up. A `do` or a `hold` has a `key` too: its
  parameters, and examples of a call that differs only outside the key (`key_conflict`) and of a
  hold made again with the key of one that ended (`done_before`).

## The API of a book

`chobo api` is for another tool that calls the book, dandori among them: how to call it, what
each operation can be refused with, the life of a hold as a state machine, how IDs are made, and
what each target names its functions and types.

```console
$ chobo api examples/refunds/refunds.book
{
  "v": 1,
  "book": "refunds",
  "version": 1,
…
  "units": [
    {
      "name": "USD",
      "scale": 2,
      "ledger": 1838891465
    }
  ],
  "accounts": [
    {
      "name": "refundable",
      "params": [
        "order"
      ],
      "unit": "USD",
      "outside": false,
      "lower": {
        "value": 0,
        "refused_as": "refund_exceeds_sale"
      },
      "upper": null,
      "description": "what is left to refund on the order",
      "code": 25116
    },
…
  "machines": [
    {
      "name": "refund",
      "over": "refund",
      "carry": {
        "input": "state",
        "output": "next_state",
        "enum": "state"
      },
      "held": [],
      "never": [],
      "once": [],
      "states": [
        "held",
        "posted",
        "voided",
        "expired"
      ],
      "initial": "held",
      "final": [
        "posted",
        "voided",
        "expired"
      ],
      "events": [
        "post",
        "void",
        "expire"
      ],
      "external": [
        "expire"
      ],
      "certificate": {
…
```

- `units`, `accounts` and `transfers` are the book; a transfer has its `operations`, each with
  its `refusals` as the report has them, its `code`, and its `definition`, the hash of what it
  moves, which a target keeps beside each key.
- `machines` has one state machine a transfer that holds: the states, the events, which of them
  happen outside the caller's hands (`expire`, unless the hold never expires), and a
  `certificate` with the table of every event in every state, in the shape rulec's
  `certificate` gives a rule's state machine. dandori reads rulec's machines in this shape; it
  does not read chobo's yet.
- `ids` says how an ID is made, for a tool that works one out itself; `targets` says what each
  target names: the PostgreSQL schema, functions and parameters, the members and types of the
  TypeScript, Python and Go clients, and the chain TigerBeetle is sent for each operation.

## What a client sends

`chobo run --show postgres` or `--show tigerbeetle` prints, for each operation of a scenario,
what the clients `chobo build` writes send that target: the SQL and its arguments for
PostgreSQL; for TigerBeetle, the hold read before a post or a void, the accounts made, the
opening transfers, and the chain, field by field. The tests hold the clients of all three
languages to it, on every scenario. [targets.md](targets.md) says what the clients do.

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
$ chobo run examples/refunds/refunds.book --scenario examples/refunds/refunds.more.json --show tigerbeetle
examples/refunds/refunds.book (more: two refunds of 20.00 at the same time on a sale of 30.00, then one of 10.00): 3 steps, as sent to TigerBeetle (tenant "")
  1  sale.do(order: A-100, amount: 3000)
       accounts:
         account  sales              069c57ca75a83bb1b62d3735a168e8ae
         account  refundable(A-100)  5e57413e175e6a7135612aa241f9cdb7  debits_must_not_exceed_credits
       chain:
         0  main  sales → refundable(A-100)  3000  be4165a032199c2adfdc384159a28ab2
  2  together, caller 1: refund.hold(request: R-1, order: A-100, amount: 2000)
       accounts:
         account  refundable(A-100)  5e57413e175e6a7135612aa241f9cdb7  debits_must_not_exceed_credits
         account  refunded           32282c2ff4dcedae062c27e267ffe99c
       chain:
         0  main  refundable(A-100) → refunded  2000  8bce0c8ed52efa84945d40f7587de959  pending timeout 604800
…
```

The IDs are made from the keys (the tenant is `""` here), so a retry sends the same ones; with
`--format json`, the same is written field by field, as the tests compare it with what each
client sent.
