# Functional Core の四つの例

kaname の例の初めの四つは、ほかの言語を読みません。どれも `crates/kaname/examples/` にあり、英語の版の隣に日本語の版（`<name>.ja.kn`）があります。どの例も `host/` の下に、`ritsu gen` がモジュールから書いたコードを呼ぶホストのプログラムを、TypeScript、Python、Go で一つずつ持ち、三つとも `host/output.txt` と同じものを出力します。ritsu のテストは、例ごとにパッケージを作り、12 のプログラムを、TypeScript は `tsc --strict` に、Python は `mypy --strict` に、Go は `go vet` に通してから走らせます。

このページのモジュール、ホストのコード、出力は、どれも英語の版のものです。

## ホストが呼ぶパッケージ

例のディレクトリで `ritsu gen` を走らせると、TypeScript、Python、Go のパッケージができます。

```console
$ cd crates/kaname/examples/order_flow
$ ritsu gen --root . --out gen --module example.com/order_flow --lang ja
生成しました: gen/typescript/glue/kaname.ts
生成しました: gen/typescript/glue/order_flow_ja.ts
生成しました: gen/typescript/glue/order_flow.ts
…
生成しました: gen/python/generated/glue/order_flow.py
…
生成しました: gen/go/glue/orderflow/orderflow.go
…
```

モジュールのコードは `glue/` の下にあり、その横に補助のモジュール `kaname` があります。補助のモジュールには、正確な分数、int64 を超えないかの確かめ、丸め、コードポイントで扱う文字列、鍵の順に回す辞書と集合が入っています。パッケージは、それぞれの言語の標準ライブラリのほかに何も使いません。ホストはパッケージの横に置きます。TypeScript は `gen/typescript/` に、Python は `gen/python/`（パッケージの名前は `generated`）に、Go は `--module` に渡した名前の Go のモジュールの中の、自分のディレクトリに置きます。値は、それぞれの言語の型で受け渡します。

| kaname | TypeScript | Python | Go |
|---|---|---|---|
| `number` | `bigint` | `int` | `int64` |
| `money[USDc]` | パッケージの `USDc` | `USDc` | `units.USDc` |
| `date` | `"2026-10-22"` | `datetime.date` | `kaname.Date` |
| レコード | 読み取り専用のオブジェクト | 凍結したデータクラス | 構造体 |
| フィールドを持つ列挙の値 | `kind` を持つオブジェクト | データクラス（`EventAdd`） | 構造体（`EventAdd`） |
| `result[T, E]` | `{ kind: "ok", value }`、`{ kind: "err", error }` | `kaname.Ok`、`kaname.Err` | `kaname.Ok[T, E]`、`kaname.Err[T, E]` |
| `dict[K, V]`、`set[T]` | `ReadonlyMap`、`ReadonlySet` | `dict`、`frozenset` | `map[K]V`、`map[T]struct{}` |

ホストが呼ぶ関数は、計算を始める前に引数を宣言で確かめ、外れていれば `input` で失敗します。失敗はどれも種類を持つ `KanameError` で、種類は `input` か、関数のエフェクトが言う種類のどれかです。業務として受け付けないことは、失敗ではなく `result` で、ホストがそれを読みます。

## 注文とイベント

`order_flow` は、注文を五つの状態のあいだで進めます（モジュールは[最初のページ](index.md)にあります）。ホストは注文を持ち、イベントが来るたびに `next` を呼びます。答えが次の注文ならそれに置き換え、拒まれたら理由を出力します。

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

フィールドを持つ列挙の値は、TypeScript では `kind` で、Python では `isinstance` で、Go では型の switch で分けます。`result` も同じです。`replay` は、イベントのリストをモジュールの中で順に `next` に渡し、最初に拒まれたところで止まります。ホストのほうは、拒まれても次のイベントへ進みます。

## 倉庫の二回の棚卸し

`stock_diff` は、棚卸しの数を、品番から棚にある数への辞書で持ち、二回の棚卸しを品番ごとに比べます。一日の入出庫を棚卸しの数に足すことと、発注する品番を集合で返すこともします。

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

違いは、三つとも品番の順に並びます。辞書が鍵の順に回るからです。Python の `frozenset` と Go の `map` には決まった順が無いので、Python のホストは発注する品番を、Go のホストは出力するものを両方とも並べ替えています。

## 人が入力したハンドル

`handles` は、人が入力したハンドルをサービスが持つ形にしてから、ハンドルの決まりに合うかを確かめます。決まりは、長さが 3 文字から 20 文字で、使える文字は a〜z、0〜9、`-` だけ、英字で始まり、サービスが使う語ではないことです。合わない理由は全部返します。ハンドルと番号の組（タグ）も読みます。

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

生成したコードは、三つの出力先のどれでも、文字列をコードポイントで数え、切り、比べます。TypeScript の文字列は UTF-16 の単位で、Go の文字列はバイトでできていますが、`"ünïcode"` はどれでも 7 文字です（Go では 9 バイト）。`ü` と `ï` はハンドルに使えない文字で、`ü` は、ハンドルの先頭に来るべき a〜z の英字でもありません。Go では、モジュールにレコード `Tag` もあるので、関数 `tag` は `TagFn` になります。

## カタログの分類の木

`catalog` は、カタログを木の形で持ちます。分類はそれぞれ、品物と下の分類を持ちます。モジュールは、品物ごとのパス、空の分類を除いた木、名前をウェブのアドレスに使う形にした木、名前の一覧、いちばん安い品物を返します。

```kn
record Item
  sku   : string
  name  : string
  price : money[USDc]  range >=1USDc <=1_000_000USDc

record Category
  name     : string
  items    : list[Item]
  children : list[Category]

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

どの関数も木をたどって再帰するので、エフェクトは `diverges` です。たどることが止まるかを kaname は確かめず、検査もそう言います（`関数 9 個、diverges`）。関数を受け取る `relabel` と `collect` は、ホストが呼ぶものではありません。ホストが呼ぶのは、それを呼ぶ `slugged`、`names`、`cheapest`です（[書き方](language.md)）。値が無いことは、TypeScript では `null`、Python では `None`、Go では `nil` のポインタで表します。
