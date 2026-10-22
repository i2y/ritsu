---
hide:
  - toc
---

# kaname

**Functional Core（プログラムのうち、入出力を持たずに計算して決める部分）を書くための、小さな汎用の言語です。kaname は、それを ritsu のほかの言語と同じ決まりで確かめ、関数ごとにどこまで確かめられたかを言い、まわりのプログラムから呼べる TypeScript、Python、Go のコードにします。**

プログラムは、リクエストや時刻やデータベースを読み、状態を持ち、決めたことを行います。けれども、決めることそのものには、そのどれも要りません。注文の次の状態、人が入力した文字列がハンドルとして使えるか、倉庫の二回の棚卸しで何が変わったかは、渡された値だけで決まります。kaname のモジュール（`.kn` のファイル）に書くのは、この部分です。残りはホストが受け持ちます。ホストは TypeScript、Python、Go のプログラムで、読み書きをし、状態を持ち、`ritsu gen` がモジュールから書いたコードを呼びます。

最初の例は、イベントで注文を状態から状態へ進めるものです（`crates/kaname/examples/order_flow`）。状態ごとに持つフィールドが違うので、状態を、値の下にフィールドを書いた列挙（直和型）の値にします。次の状態か、イベントを拒む理由は、状態とイベントの `match` で決めます。ここでは日本語の版（`order_flow.ja.kn`）を見せます。

```kn
enum 注文(Order)
  カート(cart)
    明細(lines) : list[明細]  count <=50
  確定済み(placed)
    明細(lines) : list[明細]  count >=1 <=50
    合計(total) : money[USDc]
    日付(on)    : date
  支払い済み(paid)
    明細(lines) : list[明細]  count >=1 <=50
    日付(on)    : date
  発送済み(shipped)
    明細(lines)        : list[明細]  count >=1 <=50
    追跡番号(tracking) : string
  取り消し済み(cancelled)
    理由(reason) : string
…
# イベントで進んだ注文か、イベントを拒む理由
fn 次(next)(注文(order): 注文, イベント(event): イベント) -> result[注文, string]
  match 注文
    カート c =>
      match イベント
        追加 a =>
          if c.明細.count() < 50
            ok(注文.カート { 明細: c.明細.append(a.明細) })
          else
            err("カートに入る明細は 50 件までです")
        削除 r => ok(注文.カート { 明細: c.明細.filter(一件 => 一件.品番 != r.品番) })
        確定 p =>
          if c.明細.count() = 0
            err("空のカートは確定できません")
          else
            ok(注文.確定済み { 明細: c.明細, 合計: 合計(明細: c.明細), 日付: p.日付 })
        取り消し x => ok(注文.取り消し済み { 理由: x.理由 })
        else => err("カートは先に確定します")
    確定済み p =>
      match イベント
        支払い y =>
          if y.金額 = p.合計
            ok(注文.支払い済み { 明細: p.明細, 日付: p.日付 })
          else
            err("金額が合計と違います")
        取り消し x => ok(注文.取り消し済み { 理由: x.理由 })
        else => err("確定済みの注文は、支払うか取り消します")
    支払い済み d =>
      match イベント
        発送 s => ok(注文.発送済み { 明細: d.明細, 追跡番号: s.追跡番号 })
        else => err("支払い済みの注文は発送します")
    発送済み, 取り消し済み => err("注文はもう閉じています")
```

`match` が、注文のどの値にもイベントのどの値にも一度ずつ答えることは、kaname が確かめます（E106）。明細を足すのは `c.明細.count() < 50` の中だけなので、カートの明細は、状態が宣言する 50 件に収まります。確定済みの注文は明細を 1 件以上持ち、明細 50 件、数量 100、単価 1,000,000 セントの合計も int64 に収まります。kaname はこれを示すので、どの関数も、受け取るどの入力にも答えます。`total` はそのことを言っています。

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
