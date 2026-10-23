---
hide:
  - toc
---

# kaname

**Functional Core（プログラムのうち、入出力を持たずに計算して決める部分）を書くための、小さな汎用の言語です。kaname は、それを ritsu のほかの言語と同じ決まりで確かめ、関数ごとにどこまで確かめられたかを言い、まわりのプログラムから呼べる TypeScript、Python、Go のコードにします。**

プログラムは、リクエストや時刻やデータベースを読み、状態を持ち、決めたことを行います。けれども、決めることそのものには、そのどれも要りません。注文の次の状態、人が入力した文字列がハンドルとして使えるか、倉庫の二回の棚卸しで何が変わったかは、渡された値だけで決まります。kaname のモジュール（`.kn` のファイル）に書くのは、この部分です。残りはホストが受け持ちます。ホストは TypeScript、Python、Go のプログラムで、読み書きをし、状態を持ち、`ritsu gen` がモジュールから書いたコードを呼びます。

最初の例は、イベントで注文を状態から状態へ進めるものです（`crates/kaname/examples/order_flow`）。状態ごとに持つフィールドが違うので、状態を、値の下にフィールドを書いた列挙（直和型）の値にします。次の状態か、イベントを拒む理由は、状態とイベントの `match` で決めます。コードは英語の版（`order_flow.kn`）を見せます。日本語の名前で書いた版（`order_flow.ja.kn`）も隣にあります。

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

`match` が、注文のどの値にもイベントのどの値にも一度ずつ答えることは、kaname が確かめます（E106）。明細を足すのは `c.lines.count() < 50` の中だけなので、カートの明細は、状態が宣言する 50 件に収まります。確定済みの注文は明細を 1 件以上持ち、明細 50 件、数量 100、単価 1,000,000 セントの合計も int64 に収まります。kaname はこれを示すので、どの関数も、受け取るどの入力にも答えます。`total` はそのことを言っています。

```console
$ cd crates/kaname/examples/order_flow
$ ritsu check --lang ja
ok order_flow.ja.kn: 関数 5 個、total
ok order_flow.kn: 関数 5 個、total
ritsu check: ファイル 2 個（kaname 2）。どれも検査を通りました。言語の境目: 確かめた 0 か所、決められない 0 か所
```

`ritsu gen` は、このモジュールを、TypeScript、Python、Go のそれぞれのパッケージに書きます。注文を持ち、イベントが来るたびにそのコードを呼ぶのはホストです。次は TypeScript のホスト（`host/main.ts`）で、横にある Python と Go のホストも同じものを出力します。ホストは英語の版（`order_flow.kn`）から生成したコードを呼ぶので、拒む理由も英語で出ます。

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

イベントを拒むときも、モジュールは答え（`err`）を返します。これは失敗ではありません。650 セントの支払いは拒まれ、700 セントの支払いで注文は先へ進み、発送済みの注文はもう動きません。

## 何を書く言語か

- **イベントで進む状態**：状態をフィールドを持つ列挙の値に、イベントをもう一つの列挙の値にし、次の状態を `match` で決めます（`order_flow`）。
- **数を集めて比べる**：辞書と集合は、参照インタプリタでも、どの出力先のコードでも、鍵の順に回ります（`stock_diff`）。
- **入力を確かめて形を整える**：文字列はコードポイントで数えて切ります。`result` で、整えた値か、受け付けない理由の全部を返します（`handles`）。
- **木の形のデータを作り替える**：自分を含むレコードを書け、関数に関数を渡せます（`catalog`）。
- **規則・日付・帳簿のまわりのコード**：規則の入力を組み立て、一つの答えを次の呼び出しに渡し、額を帳簿に渡すコードを、それぞれの型のまま書き、呼ぶ相手の範囲で確かめます（[規則・日付・帳簿を呼ぶ](together.md)）。

初めの四つの例は、TypeScript、Python、Go のホストと一緒に [Functional Core の四つの例](examples.md)で見られます。その例が使っている書き方は、[書き方](language.md)にまとめました。

## 検査が言うこと

- 関数の型には、どこまで確かめられたかが載ります。**total**（宣言した範囲のどの入力にも答える）、**fails**（実行時にコードが確かめる場所があり、その種類も載る）、**diverges**（再帰があり、止まることは確かめない）のどれかです。
- エラーには、それを起こす入力が付きます。示すことも、外れる入力を見つけることもできない場所は警告になり、生成したコードがそこで確かめます。
- TypeScript、Python、Go に生成したコードは、例とテストの 52 のモジュールのすべてのベクタで、参照インタプリタと同じ答えを返します。英語の版でも日本語の版でも同じです。
- 評価と範囲の解析は Lean でも書いてあり、範囲の解析が健全であることと、total と推論した関数が失敗しないことを証明しています。

それぞれを、`kaname check` の出力と一緒に[検査が保証すること](checks.md)で説明します。

## もっと読む

- [kaname の README](https://github.com/i2y/ritsu/blob/main/crates/kaname/README.ja.md)：言語のあらまし、コマンド、どう確かめているか
- [リファレンス](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/reference.md)（英語）：言語の全部、コマンド、`ritsu gen` が書くもの
- [診断の一覧](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/codes.ja.md)：コードごとに、いつ出るか、どう直すか、それを出す最小のモジュール（`ritsu kaname explain` でも引けます）
- [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/kaname/DESIGN.md)：入出力を持たない汎用の言語にした理由、入れないもの、どう確かめているか
- [スキル](https://github.com/i2y/ritsu/blob/main/skills/kaname/SKILL.md)：モジュールを書いたり直したりするエージェントのためのもの
