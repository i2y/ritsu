# 書き方

モジュール（`.kn` のファイル）には、まず名前と、何のためのモジュールかを書き、次に読むファイル、そのあとにレコード、列挙、関数を好きな順に書きます。最後の `examples` には、関数が返すべき答えを書きます。関数は `let` の行と、答えの式でできています。文も、書き換える変数も、ループもなく、一度作った値は変わりません。このページでは、Functional Core の四つの例が使っているものを順に見ていきます。言語の全部は[リファレンス](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/reference.md)（英語）にあります。

## 範囲を書く値、書かない値

型は ritsu のものです。数、単位の付いたお金と量、刻みのある率、日付、真偽、文字列があります。フィールドと引数には値の範囲（`range`）を、リストと文字列には要素の数（`count`）を書けます。書かなくてもかまいません。注文の明細には書いてあります。

```kn
record Line
  sku   : string       count >=1 <=20
  qty   : number       range >=1 <=100
  price : money[USDc]  range >=1USDc <=1_000_000USDc
```

書いた範囲は、ホストとの約束です。ホストが呼ぶ関数のコードは、計算を始める前に引数をその範囲で確かめ、外れていれば `input` で失敗します。範囲を書かない値は、型の全体をとります。数、お金、量ならその単位で数えた int64 の全体、日付なら 0001-01-01 から 9999-12-31 です。確定済みの注文の合計と日付、発送済みの注文の追跡番号には、範囲を書いていません。そういう値から計算した値も、ほかの値と同じく int64 に収まるかを確かめます。収まると示せないところは、コードが実行時に確かめます（[検査が保証すること](checks.md)）。

## フィールドを持つ列挙と `match`

列挙は、値をその下に並べます。値ごとに、自分のフィールドを持てます。注文のイベントがそうです。

```kn
enum Event
  add
    line : Line
  remove
    sku : string
  place
    on : date
  pay
    amount : money[USDc]
  ship
    tracking : string
  cancel
    reason : string
```

値はフィールドと一緒に作り（`Order.placed { lines: c.lines, total: …, on: p.on }`）、`match` で分けます。分岐は一行に一つで、受けた値に名前を付けると、そのフィールドを読めます（`placed p` と書いて `p.total`）。一つの分岐で二つ以上の値を受けてもよく、`else` は、上の分岐が受けなかった値を全部受けます。

```kn
# whether an order takes more events
fn open(order: Order) -> bool
  match order
    shipped, cancelled => false
    else => true
```

どの分岐も受けない値があるとき、上の分岐がすでに受けた値をもう一度受けるとき、受ける値の残っていない `else` を書いたときは、E106 です。数、お金、日付、文字列は、リテラルの分岐と `else` で分けられます。`=` と `!=` は、同じ型の二つの値を中身で比べます。レコードはフィールドごとに、リストは要素ごとに、列挙の値は値とフィールドで比べます。`{ ..c, children: kept }` は、ほかのレコードのフィールドの一部を替えて作ったレコードです。

## 辞書と集合

`dict[K, V]` は鍵ごとに値を一つ持ち、`set[T]` は同じ要素を一つだけ持ちます。鍵にできるのは、数、お金、量、刻みのある率、日付、文字列、真偽と、フィールドを持たない列挙の値です。辞書と集合は、作った順にかかわらず、鍵の順に回ります。組み込みの関数は、初めの引数のあとに `.` で続けて書けます。

```kn
# every code that differs, in the order of the codes: a dict and a set go round in their keys' order
fn changes(before: dict[string, number] range >=0 <=1_000_000, after: dict[string, number] range >=0 <=1_000_000) -> list[Change]
  let skus = before.keys().to_set().union(after.keys().to_set())
  skus.to_list().filter_map(sku => change(sku: sku, before: before.get(sku), after: after.get(sku)))

# the moves of a day, netted for each code
fn net(moves: list[Move] count <=1_000) -> dict[string, number]
  moves.group_by(m => m.sku).map_values(ms => ms.sum(m => m.qty))
```

辞書に書いた範囲は、その値の範囲です。ここでは、どの棚卸しの数も 0 から 1,000,000 です。`ritsu kaname run` は、JSON で渡した入力で、関数を参照インタプリタに流します。この入力は棚卸しの数を組で書き、品番の順とは違う順に並べています（`[["ink", 4], ["pen", 10], ["pad", 2]]`）。`--trace` を付けると、`let` ごとの値も出ます。

```console
$ cd crates/kaname
$ ritsu kaname run examples/stock_diff/stock_diff.kn changes --input tests/inputs/stock_diff.json --trace --lang ja
changes(before: ["ink": 4, "pad": 2, "pen": 10], after: ["pad": 2, "pen": 7, "tape": 3])
  34 行目: let skus = ["ink", "pad", "pen", "tape"].to_set()
= [Change.removed { sku: "ink", qty: 4 }, Change.changed { sku: "pen", was: 10, now: 7 }, Change.added { sku: "tape", qty: 3 }]
```

生成したコードも、TypeScript の `Map`、Python の `dict`、Go の `map` を同じ順に回します。どの出力先でも、答えは同じ並びのリストです。

## 文字列と `result`

文字列は、コードポイントの並びです。長さも、切り出しも、二つの文字列の大小も、どの出力先でもコードポイントで決まります。空白は Unicode が空白とする文字で、大文字と小文字を替えるのは ASCII の英字だけです。文字列の組み込みの関数は、どれも失敗しません。たとえば `parse_number` は、数として読めない文字列には `none` を返します。

`result[T, E]` は、値（`ok`）か、業務としてそれを受け付けない理由（`err`）を返す型です。`let x = try e` は、`e` が `ok` なら値を取り出し、`err` ならそれをそのまま関数の答えにします。

```kn
# the handle as it is kept: trimmed, each run of white space inside one hyphen, A to Z as a to z
fn normalized(raw: string) -> string
  raw.words().join("-").lower_ascii()
…
# a tag written `<handle>#<number>`: the handle made as `handle` makes it, the number from 1 to 9999
fn tag(text: string) -> result[Tag, list[string]]
  match text.index_of("#")
    none => err(["no # between the handle and the number"])
    some i =>
      let h = try handle(raw: text.take(i))
      match text.drop(i + 1).parse_number()
        none => err(["the number is not written in digits"])
        some n => if n >= 1 and n <= 9999 then ok(Tag { handle: h, number: n }) else err(["the number is not from 1 to 9999"])
```

`err` は答えの一つで、失敗ではありません。`tag` も total です。kaname が失敗と呼ぶのは、検査が示せなかった場所で起きることです。それは関数のエフェクトに載り、言語の中で受け止める書き方はありません。

## ジェネリクスと関数の値

関数、レコード、列挙は、型の引数を持てます（`collect[T]`。鍵にできる型に限るなら `K: key`）。引数には関数も渡せます。ラムダでも、名前で書いた関数でもかまいません。

```kn
# the tree with each category's name made by the function given
fn relabel(c: Category, label: (string) -> string) -> Category
  Category { ..c, name: label(c.name), children: c.children.map(x => relabel(c: x, label: label)) }

# the names as the parts of a web address take them: words joined by hyphens, in lower case
fn slugged(c: Category) -> Category
  relabel(c: c, label: n => n.words().join("-").lower_ascii())

# what the function given picks from each category, from the top down
fn collect[T](c: Category, pick: (Category) -> list[T]) -> list[T]
  pick(c).concat(c.children.flat_map(x => collect(c: x, pick: pick)))

fn items(c: Category) -> list[Item]
  collect(c: c, pick: x => x.items)

fn names(c: Category) -> list[string]
  collect(c: c, pick: x => [x.name])
```

型の引数を持つ関数と、関数を受け取る関数は、モジュールの関数から呼ぶもので、ホストからは呼びません。ホストが呼ぶのは、型がすべて決まっている `slugged`、`items`、`names` です。検査は、そうした関数を呼ぶところごとに、渡した値と関数で本体を読み直します。渡した関数のエフェクトは、それを渡した関数に伝わります。関数の型に、許すエフェクトを書くこともでき（`(string) -> string total`）、それを超える関数を渡すと E402 です。関数を返す関数や、関数を入れたレコード、リスト、辞書は書けません。

## 自分を含む型

レコードと列挙は、リスト、辞書、集合、オプショナルな値、`result`、ほかの型を通して、自分自身を含められます。カタログの分類は、下の分類を持ちます。

```kn
record Category
  name     : string
  items    : list[Item]
  children : list[Category]
…
# the tree with every category that holds no item, under it or in it, taken out
fn pruned(c: Category) -> Category?
  let kept = c.children.filter_map(x => pruned(c: x))
  if c.items.count() = 0 and kept.count() = 0 then none else Category { ..c, children: kept }
```

どの値も同じ型の値をもう一つ持つことになる型（オプショナルでもリストでもない、自分の型のフィールドを持つレコードなど）は、E108 です。自分を含む値をたどる関数は再帰するので、エフェクトは `diverges` になります。止まることを、kaname は確かめません。ホストが渡せる値は 200 段までです。引数からたどれるどのパスでも、自分を含む型の値を一つ通るたびに一段と数え、それより深い値は、関数が始まる前に `input` で失敗します。

## 持たないもの

入出力、時刻、乱数、書き換えられる状態は、ホストのものです。ホストがそれを読み、値として関数に渡します。ほかにも、浮動小数点（値は int64 に収まる整数で受け渡し、丸めるまでは正確な分数で持ちます）、ループ（`reduce` と再帰で書きます）、関数を返す関数、正規表現、失敗を受け止める書き方（業務として受け付けないことは `result` で書きます）はありません。それぞれの理由は [DESIGN の 29 章](https://github.com/i2y/ritsu/blob/main/crates/kaname/DESIGN.md)にあります。
