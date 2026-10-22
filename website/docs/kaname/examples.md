# Four functional cores

The first four examples of kaname read no other language. Each is in `crates/kaname/examples/`, in
English and, beside it, in Japanese (`<name>.ja.kn`), and each has a host under `host/`: a program in
TypeScript, one in Python and one in Go, which calls the code `ritsu gen` writes of the module and
prints what `host/output.txt` holds. ritsu's tests build each package and run the twelve programs
against it, the TypeScript through `tsc --strict`, the Python through `mypy --strict` and the Go
through `go vet`.

## The package a host calls

In the directory of an example, `ritsu gen` writes a package for each of TypeScript, Python and Go:

```console
$ cd crates/kaname/examples/order_flow
$ ritsu gen --root . --out gen --module example.com/order_flow
generated: gen/typescript/glue/kaname.ts
generated: gen/typescript/glue/order_flow_ja.ts
generated: gen/typescript/glue/order_flow.ts
…
generated: gen/python/generated/glue/order_flow.py
…
generated: gen/go/glue/orderflow/orderflow.go
…
```

A module is under `glue/`, beside the helper module `kaname`, which holds the exact fractions, the
arithmetic checked against int64, the roundings, the strings counted in code points and the dicts
and sets walked in the order of their keys. The package uses nothing but each language's standard
library. A host is put beside it: the TypeScript in `gen/typescript/`, the Python in `gen/python/`,
where the package is `generated`, and the Go in a directory of its own in the Go module `--module`
names. The values travel in each language's own types:

| kaname | TypeScript | Python | Go |
|---|---|---|---|
| `number` | `bigint` | `int` | `int64` |
| `money[USDc]` | the package's `USDc` | `USDc` | `units.USDc` |
| `date` | `"2026-10-22"` | `datetime.date` | `kaname.Date` |
| a record | a read-only object | a frozen data class | a struct |
| a value of an enum with fields | an object with its `kind` | a data class (`EventAdd`) | a struct (`EventAdd`) |
| `result[T, E]` | `{ kind: "ok", value }`, `{ kind: "err", error }` | `kaname.Ok`, `kaname.Err` | `kaname.Ok[T, E]`, `kaname.Err[T, E]` |
| `dict[K, V]`, `set[T]` | `ReadonlyMap`, `ReadonlySet` | a `dict`, a `frozenset` | `map[K]V`, `map[T]struct{}` |

The function a host calls checks its arguments against what they declare before it computes, and
fails with `input` outside them. Every failure is a `KanameError` with its kind: `input`, or one of
the kinds the function's effect says. A refusal of the business is not a failure but a `result`,
which the host reads.

## An order and its events

`order_flow` moves an order through five states (the module is on [the first page](index.md)). The
host keeps the order and hands `next` each event as it comes, keeping the order `next` answers, or
printing why the event is refused:

```kn
# the order an event moves it to, or why the event is refused
fn next(order: Order, event: Event) -> result[Order, string]
…
# the events in turn, from an empty cart: the first refusal stops them
fn replay(events: list[Event]) -> result[Order, string]
  let start : result[Order, string] = ok(Order.cart { lines: [] })
  events.reduce(start, (r, e) => after(r: r, event: e))
```

=== "TypeScript"

    ```ts
    import { next, type Event, type Order } from "./glue/order_flow";
    import type { USDc } from "./units";

    const events: Event[] = [
      { kind: "add", line: { sku: "pen", qty: 2n, price: 150n as USDc } },
      { kind: "add", line: { sku: "ink", qty: 1n, price: 400n as USDc } },
      { kind: "place", on: "2026-10-22" },
      { kind: "pay", amount: 650n as USDc },
      { kind: "pay", amount: 700n as USDc },
      { kind: "ship", tracking: "1Z-42" },
      { kind: "cancel", reason: "too late" },
    ];

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

=== "Python"

    ```python
    import datetime

    from generated.glue import kaname, order_flow as flow
    from generated.units import USDc

    events: list[flow.Event] = [
        flow.EventAdd(line=flow.Line(sku="pen", qty=2, price=USDc(150))),
        flow.EventAdd(line=flow.Line(sku="ink", qty=1, price=USDc(400))),
        flow.EventPlace(on=datetime.date(2026, 10, 22)),
        flow.EventPay(amount=USDc(650)),
        flow.EventPay(amount=USDc(700)),
        flow.EventShip(tracking="1Z-42"),
        flow.EventCancel(reason="too late"),
    ]
    # the name of each value, as the module writes it
    names = {
        flow.EventAdd: "add", flow.EventPlace: "place", flow.EventPay: "pay", flow.EventShip: "ship", flow.EventCancel: "cancel",
        flow.OrderCart: "cart", flow.OrderPlaced: "placed", flow.OrderPaid: "paid", flow.OrderShipped: "shipped", flow.OrderCancelled: "cancelled",
    }

    order: flow.Order = flow.OrderCart(lines=[])
    for event in events:
        r = flow.next(order=order, event=event)
        if isinstance(r, kaname.Err):
            print(f"{names[type(event)]}: refused: {r.error}")
            continue
        order = r.value
        print(f"{names[type(event)]}: {names[type(order)]}")
    ```

=== "Go"

    ```go
    package main

    import (
        "fmt"

        "example.com/order_flow/glue/kaname"
        flow "example.com/order_flow/glue/orderflow"
        "example.com/order_flow/units"
    )

    …

    func main() {
        events := []flow.Event{
            flow.EventAdd{Line: flow.Line{Sku: "pen", Qty: 2, Price: units.USDc(150)}},
            flow.EventAdd{Line: flow.Line{Sku: "ink", Qty: 1, Price: units.USDc(400)}},
            flow.EventPlace{On: kaname.Date{Year: 2026, Month: 10, Day: 22}},
            flow.EventPay{Amount: units.USDc(650)},
            flow.EventPay{Amount: units.USDc(700)},
            flow.EventShip{Tracking: "1Z-42"},
            flow.EventCancel{Reason: "too late"},
        }
        var order flow.Order = flow.OrderCart{}
        for _, event := range events {
            switch r := flow.Next(order, event).(type) {
            case kaname.Err[flow.Order, string]:
                fmt.Printf("%s: refused: %s\n", name(event), r.Error)
            case kaname.Ok[flow.Order, string]:
                order = r.Value
                fmt.Printf("%s: %s\n", name(event), name(order))
            }
        }
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

A value of an enum with fields is split on its `kind` in TypeScript, with `isinstance` in Python and
with a type switch in Go, and a `result` the same way. `replay` runs a list of events through
`next` in the module itself, and stops at the first refusal, where the host goes on to the next
event.

## Two counts of a stockroom

`stock_diff` keeps a count as a dict from a code to how many are on the shelf, and compares two
counts code by code. It also lays the moves of a day on a count, and answers the codes to reorder
as a set:

```kn
record Move
  sku : string  count >=1 <=20
  qty : number  range >=-10_000 <=10_000

enum Change
  added
    sku : string
    qty : number
  removed
    sku : string
    qty : number
  changed
    sku  : string
    was : number
    now : number
…
fn changes(before: dict[string, number] range >=0 <=1_000_000, after: dict[string, number] range >=0 <=1_000_000) -> list[Change]
…
# a count with the moves of a day laid on it; a code that comes to none is left out
fn moved(stock: dict[string, number] count <=10_000 range >=0 <=1_000_000, moves: list[Move] count <=1_000) -> dict[string, number]
  stock.merge_with(net(moves: moves), (a, b) => a + b).filter(e => e.value != 0)

# the codes on the shelf below what is kept of them
fn to_reorder(stock: dict[string, number] range >=0 <=1_000_000, floor: number range >=0 <=1_000_000) -> set[string]
  stock.filter(e => e.value < floor).keys().to_set()
```

=== "TypeScript"

    ```ts
    import { changes, moved, to_reorder } from "./glue/stock_diff";

    const before = new Map([["ink", 4n], ["pen", 10n], ["pad", 2n]]);
    const after = new Map([["pen", 7n], ["pad", 2n], ["tape", 3n]]);
    for (const c of changes(before, after)) {
      switch (c.kind) {
        case "added":
          console.log(`added ${c.sku}: ${c.qty}`);
          break;
        case "removed":
          console.log(`removed ${c.sku}: ${c.qty}`);
          break;
        case "changed":
          console.log(`changed ${c.sku}: ${c.was} -> ${c.now}`);
          break;
      }
    }
    const today = moved(after, [{ sku: "pen", qty: -7n }, { sku: "glue", qty: 12n }]);
    console.log(`after the day: ${[...today].map(([sku, n]) => `${sku} ${n}`).join(", ")}`);
    console.log(`to reorder: ${[...to_reorder(today, 5n)].join(", ")}`);
    ```

=== "Python"

    ```python
    from generated.glue import stock_diff as stock

    before = {"ink": 4, "pen": 10, "pad": 2}
    after = {"pen": 7, "pad": 2, "tape": 3}
    for c in stock.changes(before=before, after=after):
        if isinstance(c, stock.ChangeAdded):
            print(f"added {c.sku}: {c.qty}")
        elif isinstance(c, stock.ChangeRemoved):
            print(f"removed {c.sku}: {c.qty}")
        else:
            print(f"changed {c.sku}: {c.was} -> {c.now}")
    today = stock.moved(stock=after, moves=[stock.Move(sku="pen", qty=-7), stock.Move(sku="glue", qty=12)])
    print("after the day: " + ", ".join(f"{sku} {n}" for sku, n in today.items()))
    print("to reorder: " + ", ".join(sorted(stock.to_reorder(stock=today, floor=5))))
    ```

=== "Go"

    ```go
    package main

    import (
        "fmt"
        "maps"
        "slices"
        "strings"

        stock "example.com/stock_diff/glue/stockdiff"
    )

    func main() {
        before := map[string]int64{"ink": 4, "pen": 10, "pad": 2}
        after := map[string]int64{"pen": 7, "pad": 2, "tape": 3}
        for _, c := range stock.Changes(before, after) {
            switch c := c.(type) {
            case stock.ChangeAdded:
                fmt.Printf("added %s: %d\n", c.Sku, c.Qty)
            case stock.ChangeRemoved:
                fmt.Printf("removed %s: %d\n", c.Sku, c.Qty)
            case stock.ChangeChanged:
                fmt.Printf("changed %s: %d -> %d\n", c.Sku, c.Was, c.Now)
            }
        }
        today := stock.Moved(after, []stock.Move{{Sku: "pen", Qty: -7}, {Sku: "glue", Qty: 12}})
        var counts []string
        for _, sku := range slices.Sorted(maps.Keys(today)) {
            counts = append(counts, fmt.Sprintf("%s %d", sku, today[sku]))
        }
        fmt.Println("after the day: " + strings.Join(counts, ", "))
        fmt.Println("to reorder: " + strings.Join(slices.Sorted(maps.Keys(stock.ToReorder(today, 5))), ", "))
    }
    ```

```text
removed ink: 4
changed pen: 10 -> 7
added tape: 3
after the day: glue 12, pad 2, tape 3
to reorder: pad, tape
```

The changes come in the order of the codes in all three, since a dict goes round in the order of its
keys. A Python `frozenset` and a Go `map` have no order of their own, so the Python host sorts the
codes to reorder, and the Go host sorts both what it prints.

## What a person typed

`handles` makes a handle a person typed the one a service keeps, then holds it to what a handle is:
3 to 20 characters, a to z, 0 to 9 and `-`, starting with a letter, and not a word the service keeps.
It answers every reason a handle is refused, and reads a tag, a handle and a number:

```kn
record Tag
  handle : string
  number : number  range >=1 <=9999
…
# each reason a handle is not one the service takes, in order
fn problems(handle: string) -> list[string]
  let n = handle.count()
  let short = if n < 3 then "shorter than 3 characters" else none
  let long = if n > 20 then "longer than 20 characters" else none
  let odd = if handle.chars().all(c => "abcdefghijklmnopqrstuvwxyz0123456789-".contains(c)) then none else "a character other than a to z, 0 to 9 and -"
  let start = if "abcdefghijklmnopqrstuvwxyz".contains(handle.take(1)) then none else "not starting with a letter"
  let taken = if ["admin", "help", "root", "support"].contains(handle) then "a word the service keeps" else none
  [short, long, odd, start, taken].filter_map(x => x)

# the handle a person typed, as it is kept, or every reason it cannot be
fn handle(raw: string) -> result[string, list[string]]
  let h = normalized(raw: raw)
  let why = problems(handle: h)
  if why.count() = 0 then ok(h) else err(why)
…
fn tag(text: string) -> result[Tag, list[string]]
```

=== "TypeScript"

    ```ts
    import { handle, tag } from "./glue/handles";

    for (const typed of [" Grace Hopper", "9", "Root", "ünïcode"]) {
      const r = handle(typed);
      console.log(r.kind === "ok" ? `"${typed}" -> ${r.value}` : `"${typed}" -> refused: ${r.error.join("; ")}`);
    }
    for (const typed of ["Ada Lovelace#42", "ada#0"]) {
      const r = tag(typed);
      console.log(r.kind === "ok" ? `"${typed}" -> ${r.value.handle} #${r.value.number}` : `"${typed}" -> refused: ${r.error.join("; ")}`);
    }
    ```

=== "Python"

    ```python
    from generated.glue import handles, kaname

    for typed in [" Grace Hopper", "9", "Root", "ünïcode"]:
        r = handles.handle(raw=typed)
        print(f'"{typed}" -> {r.value}' if isinstance(r, kaname.Ok) else f'"{typed}" -> refused: {"; ".join(r.error)}')
    for typed in ["Ada Lovelace#42", "ada#0"]:
        t = handles.tag(text=typed)
        print(f'"{typed}" -> {t.value.handle} #{t.value.number}' if isinstance(t, kaname.Ok) else f'"{typed}" -> refused: {"; ".join(t.error)}')
    ```

=== "Go"

    ```go
    package main

    import (
        "fmt"
        "strings"

        "example.com/handles/glue/handles"
        "example.com/handles/glue/kaname"
    )

    func main() {
        for _, typed := range []string{" Grace Hopper", "9", "Root", "ünïcode"} {
            switch r := handles.Handle(typed).(type) {
            case kaname.Ok[string, []string]:
                fmt.Printf("%q -> %s\n", typed, r.Value)
            case kaname.Err[string, []string]:
                fmt.Printf("%q -> refused: %s\n", typed, strings.Join(r.Error, "; "))
            }
        }
        for _, typed := range []string{"Ada Lovelace#42", "ada#0"} {
            switch r := handles.TagFn(typed).(type) {
            case kaname.Ok[handles.Tag, []string]:
                fmt.Printf("%q -> %s #%d\n", typed, r.Value.Handle, r.Value.Number)
            case kaname.Err[handles.Tag, []string]:
                fmt.Printf("%q -> refused: %s\n", typed, strings.Join(r.Error, "; "))
            }
        }
    }
    ```

```text
" Grace Hopper" -> grace-hopper
"9" -> refused: shorter than 3 characters; not starting with a letter
"Root" -> refused: a word the service keeps
"ünïcode" -> refused: a character other than a to z, 0 to 9 and -; not starting with a letter
"Ada Lovelace#42" -> ada-lovelace #42
"ada#0" -> refused: the number is not from 1 to 9999
```

The generated code counts, cuts and compares a string in code points in all three targets, though a
TypeScript string is made of units of UTF-16 and a Go string of bytes: `"ünïcode"` is 7 characters,
and 9 bytes in Go. `ü` and `ï` are not among the characters a handle takes, and `ü` is not one of
the letters a to z it has to start with. In Go, `tag` is `TagFn`, as the module has a record `Tag`
too.

## A tree of categories

`catalog` keeps a catalog as a tree: a category holds its items and the categories under it. The
module answers the path of every item, the tree with its empty categories taken out, the tree with
its names made fit for a web address, every name, and the cheapest item:

```kn
record Item
  sku   : string
  name  : string
  price : money[USDc]  range >=1USDc <=1_000_000USDc
…
# every item with the names of the categories it is under: `Garden / Tools / Spade`
fn paths(c: Category) -> list[string]
  paths_under(c: c, above: [])

fn paths_under(c: Category, above: list[string]) -> list[string]
  let here = above.append(c.name)
  c.items.map(i => here.append(i.name).join(" / ")).concat(c.children.flat_map(x => paths_under(c: x, above: here)))
…
# the cheapest item of the tree, the first of those of one price
fn cheapest(c: Category) -> Item?
  items(c: c).min_by(i => i.price)
```

=== "TypeScript"

    ```ts
    import { cheapest, names, paths, pruned, slugged, type Category } from "./glue/catalog";
    import type { USDc } from "./units";

    const garden: Category = {
      name: "Garden",
      items: [],
      children: [
        { name: "Hand Tools", items: [{ sku: "sp-1", name: "Spade", price: 1999n as USDc }, { sku: "tr-2", name: "Trowel", price: 899n as USDc }], children: [] },
        { name: "Seeds", items: [], children: [{ name: "Bulbs", items: [], children: [] }] },
        { name: "Hoses", items: [{ sku: "hr-1", name: "Hose reel", price: 4500n as USDc }], children: [] },
      ],
    };
    for (const p of paths(garden)) {
      console.log(p);
    }
    const kept = pruned(garden);
    console.log(`kept: ${kept === null ? "nothing" : names(kept).join(", ")}`);
    console.log(`slugs: ${names(slugged(garden)).join(", ")}`);
    const c = cheapest(garden);
    console.log(`cheapest: ${c === null ? "none" : `${c.name}, ${c.price} cents`}`);
    ```

=== "Python"

    ```python
    from generated.glue import catalog
    from generated.units import USDc

    garden = catalog.Category(
        name="Garden",
        items=[],
        children=[
            catalog.Category(
                name="Hand Tools",
                items=[catalog.Item(sku="sp-1", name="Spade", price=USDc(1999)), catalog.Item(sku="tr-2", name="Trowel", price=USDc(899))],
                children=[],
            ),
            catalog.Category(name="Seeds", items=[], children=[catalog.Category(name="Bulbs", items=[], children=[])]),
            catalog.Category(name="Hoses", items=[catalog.Item(sku="hr-1", name="Hose reel", price=USDc(4500))], children=[]),
        ],
    )
    for p in catalog.paths(c=garden):
        print(p)
    kept = catalog.pruned(c=garden)
    print("kept: " + ("nothing" if kept is None else ", ".join(catalog.names(c=kept))))
    print("slugs: " + ", ".join(catalog.names(c=catalog.slugged(c=garden))))
    c = catalog.cheapest(c=garden)
    print("cheapest: " + ("none" if c is None else f"{c.name}, {c.price} cents"))
    ```

=== "Go"

    ```go
    package main

    import (
        "fmt"
        "strings"

        "example.com/catalog/glue/catalog"
        "example.com/catalog/units"
    )

    func main() {
        garden := catalog.Category{
            Name: "Garden",
            Children: []catalog.Category{
                {Name: "Hand Tools", Items: []catalog.Item{{Sku: "sp-1", Name: "Spade", Price: units.USDc(1999)}, {Sku: "tr-2", Name: "Trowel", Price: units.USDc(899)}}},
                {Name: "Seeds", Children: []catalog.Category{{Name: "Bulbs"}}},
                {Name: "Hoses", Items: []catalog.Item{{Sku: "hr-1", Name: "Hose reel", Price: units.USDc(4500)}}},
            },
        }
        for _, p := range catalog.Paths(garden) {
            fmt.Println(p)
        }
        if kept := catalog.Pruned(garden); kept == nil {
            fmt.Println("kept: nothing")
        } else {
            fmt.Println("kept: " + strings.Join(catalog.Names(*kept), ", "))
        }
        fmt.Println("slugs: " + strings.Join(catalog.Names(catalog.Slugged(garden)), ", "))
        if c := catalog.Cheapest(garden); c == nil {
            fmt.Println("cheapest: none")
        } else {
            fmt.Printf("cheapest: %s, %d cents\n", c.Name, c.Price)
        }
    }
    ```

```text
Garden / Hand Tools / Spade
Garden / Hand Tools / Trowel
Garden / Hoses / Hose reel
kept: Garden, Hand Tools, Hoses
slugs: garden, hand-tools, seeds, bulbs, hoses
cheapest: Trowel, 899 cents
```

The functions walk the tree, so they recurse, and their effect is `diverges`: kaname does not check
that a walk stops, and the check says so (`9 functions, diverges`). `relabel` and `collect`, which
take functions, are no host's to call; the host calls `slugged`, `names` and `cheapest`, which call
them ([Writing kaname](language.md)). An optional is `null` in TypeScript, `None` in Python and a
`nil` pointer in Go.
