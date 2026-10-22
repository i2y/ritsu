---
hide:
  - toc
---

# kaname

**A small general-purpose language for writing a Functional Core: the part of a program that only
computes and decides, with no input or output of its own. kaname checks it by the rules ritsu's
languages are checked by, says of each function how far it is checked, and writes it as TypeScript,
Python and Go for the program around it to call.**

A program reads requests, the clock and a database, keeps state, and carries out what it decides.
What it decides needs none of that: the next state of an order, whether what a person typed can be
a handle, what changed between two counts of a stockroom are functions of the values they are
given. A module of kaname (a `.kn` file) is that part. The rest stays in the host, the program in
TypeScript, Python or Go that reads and writes, keeps the state, and calls the code `ritsu gen`
writes of the module.

An order moved from state to state by events is the first of the examples
(`crates/kaname/examples/order_flow`). Each state has the fields it needs, so the states are the
values of an enum with fields under them, and the next state, or why an event is refused, is a
`match` on the state and the event:

```kn
enum Order
  cart
    lines : list[Line]  count <=50
  placed
    lines : list[Line]  count >=1 <=50
    total : money[USDc]
    on    : date
  paid
    lines : list[Line]  count >=1 <=50
    on    : date
  shipped
    lines    : list[Line]  count >=1 <=50
    tracking : string
  cancelled
    reason : string
…
# the order an event moves it to, or why the event is refused
fn next(order: Order, event: Event) -> result[Order, string]
  match order
    cart c =>
      match event
        add a =>
          if c.lines.count() < 50
            ok(Order.cart { lines: c.lines.append(a.line) })
          else
            err("a cart holds 50 lines")
        remove r => ok(Order.cart { lines: c.lines.filter(l => l.sku != r.sku) })
        place p =>
          if c.lines.count() = 0
            err("an empty cart is not placed")
          else
            ok(Order.placed { lines: c.lines, total: total(lines: c.lines), on: p.on })
        cancel x => ok(Order.cancelled { reason: x.reason })
        else => err("a cart is placed first")
    placed p =>
      match event
        pay y =>
          if y.amount = p.total
            ok(Order.paid { lines: p.lines, on: p.on })
          else
            err("the amount is not the total")
        cancel x => ok(Order.cancelled { reason: x.reason })
        else => err("a placed order is paid or cancelled")
    paid d =>
      match event
        ship s => ok(Order.shipped { lines: d.lines, tracking: s.tracking })
        else => err("a paid order is shipped")
    shipped, cancelled => err("the order is closed")
```

kaname holds the `match` to every value of the order and of the event, once each (E106). It finds
that the lines of a cart stay within the 50 its state declares, since a line is added only where
`c.lines.count() < 50`; that a placed order has a line at least; and that the total of 50 lines of
100 at 1,000,000 cents stays inside int64. So every function of the module answers every input it
takes, which is what `total` says:

```console
$ cd crates/kaname/examples/order_flow
$ ritsu check
ok order_flow.ja.kn: 5 functions, total
ok order_flow.kn: 5 functions, total
ritsu check: 2 files (kaname 2): all pass; borders between the languages: 0 checked, 0 undecided
```

`ritsu gen` writes the module into the package it writes for each of TypeScript, Python and Go.
The host keeps the order and hands the code each event as it comes (`host/main.ts`; the Python and
the Go beside it print the same):

```ts
let order: Order = { kind: "cart", lines: [] };
for (const event of events) {
  const r = next(order, event);
  if (r.kind === "err") {
    console.log(`${event.kind}: refused: ${r.error}`);
    continue;
  }
  order = r.value;
  console.log(`${event.kind}: ${order.kind}`);
}
```

```text
add: cart
add: cart
place: placed
pay: refused: the amount is not the total
pay: paid
ship: shipped
cancel: refused: the order is closed
```

A refusal is an answer of the module, an `err` the host prints, and not a failure: the payment of
650 cents is refused, the one of 700 moves the order on, and nothing moves a shipped order.

## What it is written for

- **A state moved by events.** The states are the values of an enum with fields, the events those
  of another, and the step from one state to the next is a `match` (`order_flow`).
- **Counts kept and compared.** Dicts and sets go round in the order of their keys, in the
  reference interpreter and in the code of every target (`stock_diff`).
- **Input checked and normalized.** A string is counted and cut in code points, and a `result`
  answers a value or every reason it is refused (`handles`).
- **A tree reshaped.** A record may hold itself, and a function may take a function (`catalog`).
- **The code around rules, dates and books.** A rule's inputs put together, one answer carried into
  the next call, an amount handed to a book, each in its own types, and held to the ranges of what it
  calls ([Rules, dates and books](together.md)).

[Four functional cores](examples.md) shows the first four with their hosts in TypeScript, Python
and Go, and [Writing kaname](language.md) what they are written with.

## What the check says

- Each function's type says how far it was checked: **total** (it answers every input in the ranges
  it declares), **fails** with the kinds of the places its code checks when it runs, or **diverges**
  (it recurses, and that it stops is not checked).
- An error comes with an input that shows it. A place the check can neither show nor break is a
  warning, and the generated code checks it there.
- The code generated in TypeScript, Python and Go answers what the reference interpreter answers,
  on every vector of 52 modules of the examples and the tests, in English and in Japanese.
- The evaluation and the analysis of ranges are written again in Lean, with proofs that the analysis
  is sound and that a function it finds total does not fail.

[What the check guarantees](checks.md) shows each, with what `kaname check` prints.

## Read more

- [kaname's README](https://github.com/i2y/ritsu/blob/main/crates/kaname/README.md): the language at
  a glance, its commands and how it is tested.
- [The reference](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/reference.md): the
  whole language, the commands, and what `ritsu gen` writes.
- [The diagnostics](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/codes.md): for each
  code, when it comes, how to fix it, and the smallest module that gives it (`ritsu kaname explain`
  prints one).
- [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/kaname/DESIGN.md), in Japanese: why a
  general language with no input or output, what it leaves out, and how it is checked.
- [The skill](https://github.com/i2y/ritsu/blob/main/skills/kaname/SKILL.md), for an agent that
  writes or fixes a module.
