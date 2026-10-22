# kaname

Functional Core を書くための、小さな汎用の言語です。
プログラムのうち、入出力を持たずに計算して決める部分を書きます。イベントで状態を進めること、入力を確かめてサービスが持つ形にすること、数を集めて比べること、木の形のデータを作り替えること、業務の規則のまわりの計算を、レコード、フィールドを持つ値のある列挙、リスト、辞書、集合、文字列の上の関数として書き、ジェネリクス、`result`、関数の値も使えます。
リクエストや時刻やデータベースを読み、状態を持ち、決めたことを行うほかの部分は、kaname を呼ぶホストの言語（Imperative Shell）に残ります。
コアは、ritsu の言語と同じ決まり（単位、通貨と税区分、率の刻み、範囲、丸め、int64）で確かめられ、関数ごとに何が起こりうるか（`total`、`fails …`、`diverges`）も検査が推し量ります。
rulec の規則、koyomi の日付、chobo の帳簿の振替も ritsu の型のまま呼べ、`ritsu gen` が、それらのコードの隣に、何にも依存しない TypeScript、Python、Go のコードを書きます。
kaname は [ritsu](../../README.ja.md) の九つの言語の一つです。
ritsu のサイトの **[kaname のページ](https://i2y.github.io/ritsu/ja/kaname/)** では、モジュールの書き方、下の Functional Core の四つの例と TypeScript、Python、Go のホスト、検査が保証すること、規則・日付・帳簿の呼び方を順に読めます。

## Functional Core

注文は、イベントが来るたびに状態から状態へ進みます。
状態ごとに持つフィールドが違うので、状態を、値の下にフィールドを書いた列挙（直和型）の値にし、次の状態か、イベントを拒む理由を、状態とイベントの `match` で答えます。

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

`match` が注文のどの値にもイベントのどの値にも一度だけ答えることは、kaname が確かめます（E106）。
明細を足すのは `c.明細.count() < 50` の中だけなので、カートの明細は状態の宣言する 50 件に収まります。確定済みの注文は明細を 1 件以上持ち、明細 50 件、数量 100、単価 1,000,000 セントの合計も int64 に収まります。kaname はこれを示し、どの関数も、受け取るどの入力にも答えます。

```console
$ ritsu check examples/order_flow --root examples/order_flow --lang ja
ok examples/order_flow/order_flow.ja.kn: 関数 5 個、total
ok examples/order_flow/order_flow.kn: 関数 5 個、total
ritsu check: ファイル 2 個（kaname 2）。どれも検査を通りました。言語の境目: 確かめた 0 か所、決められない 0 か所
```

注文を持ち、イベントを受け取り、`ritsu gen` がこのモジュールから書いたコードを呼ぶのはホストです。
TypeScript のホスト（`examples/order_flow/host/main.ts`。横にある Python と Go のホストも、同じものを出力します）：

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

```text
add: cart
add: cart
place: placed
pay: refused: the amount is not the total
pay: paid
ship: shipped
cancel: refused: the order is closed
```

範囲を書かない値は、型の全体をとります。数なら int64 の全体です。
そういう値が int64 を超えうる場所は、生成したコードが確かめ、関数のエフェクトがそう言います。`total` と書いてあれば、エラーになります。

```console
$ ritsu kaname check tests/docs/ids.kn --lang ja
エラー[E401]: tests/docs/ids.kn:4:1: 関数 `next_id` には `total` と書いてありますが、`overflow` で失敗することがあります
     4 | fn next_id(last: number) -> number total
  = 5 行目: `last + 1` の値は、範囲を書かない値から来るので、int64 を超えることがあります（`overflow`）
  = 書いたエフェクトに収まるよう、失敗しうる場所を示せるようにする（渡す値を `if` で範囲に狭める）か、エフェクトを広げて書いてください。
```

## 規則・日付・帳簿を ritsu の型のまま呼ぶ

規則は判断を、日付のファイルは日付を、帳簿は額の動きを受け持ち、それぞれの言語の検査を受けています。
それらをつなぐのは、カートから規則の入力を組み立てる、規則の答えを次の呼び出しに渡す、額を帳簿に渡す、といったコードです。
ホストの言語で書くと、`ritsu gen` が書くパッケージが単位ごとに一つの型を持つので、ドルで数えた手数料をセントの帳簿に渡せば、ホストの言語の型の検査が止めます。
それでも、そのコードが計算するものはどの検査も読みません。丸めの書き忘れ、規則が受け取る範囲を外れる合計、規則の前提を破る途中までの合計、int64 を超える積がそうです。
kaname で書くと、規則と日付と帳簿をそれぞれの型のまま呼び、同じ決まりで確かめられます。

```kn
module 支払いの遅れ(late_payment_ja) v1
description "遅れて払われた請求書について、売り手が請求するもの。期日、何日遅れて払われたか、規則がそれに付ける定額と利息。late_payment.kn の日本語の版"

use dates 期限 from "dates/支払期限.cal"
use rule 請求額 from "rules/遅延の請求額.rule"

record 請求書(Invoice)
  番号(id)         : string
  請求日(invoiced) : date  range >=2026-01-01 <=2027-12-31
  金額(amount)     : money[GBPc]  range >=1GBPc <=100_000_000GBPc

record 請求(Claim)
  期日(due)                : date
  遅れた日数(days_late)    : number  range >=1 <=1500
  定額の補償(compensation) : money[GBP]
  利息(interest)           : money[GBPc]

fn 遅れの請求(claim)(請求書(invoice): 請求書, 支払日(paid_on): date range >=2026-01-01 <=2029-12-31, 基準金利(reference_rate): rate[step 0.01%] range >=0% <=15%) -> 請求? total
  description "期日までに払われていれば none"
  let 期日 = 期限.期日(請求日: 請求書.請求日)
  let 遅れ = 支払日 - 期日
  if 遅れ >= 1
    let 額 = 請求額(債権額: 請求書.金額, 遅れた日数: 遅れ, 基準金利: 基準金利)
    請求 { 期日: 期日, 遅れた日数: 遅れ, 定額の補償: 額.定額の補償, 利息: 額.利息 }
  else
    none
```

期日は koyomi の日付が、遅れた日数に応じて請求できる額は rulec の規則が決めます。
kaname が書くのは、そのあいだの日数の計算と、規則を呼ぶ前に日数を規則の入力の範囲に狭める分岐です。
`ritsu check` は、三つの言語を一度に読みます。

```console
$ ritsu check examples/late_payment --root examples/late_payment --lang ja
ok examples/late_payment/rules/late_payment_charges.rule
ok examples/late_payment/rules/遅延の請求額.rule
examples/late_payment/dates/payment_terms.cal: ok — invoiced 2026-01-01〜2027-12-31 の 730 日のすべてで、どの日付も計算できます。例 2 行も合っています
examples/late_payment/dates/支払期限.cal: ok — 請求日 2026-01-01〜2027-12-31 の 730 日のすべてで、どの日付も計算できます。例 2 行も合っています
ok examples/late_payment/late_payment.ja.kn: 関数 1 個、total
ok examples/late_payment/late_payment.kn: 関数 1 個、total
ritsu check: ファイル 6 個（rulec 2、koyomi 2、kaname 2）。どれも検査を通りました。言語の境目: 確かめた 2 か所、決められない 0 か所
```

`支払日 - 期日` は −758 日から 1,431 日で、`遅れ >= 1` の分岐の中では 1 日から 1,431 日です。規則の入力の範囲（1 から 1,500）に収まります。
規則に渡す債権額と基準金利もそれぞれの範囲に収まり、koyomi に渡す請求日が koyomi の範囲に収まることは ritsu が確かめます。
そのため `遅れの請求` は、宣言した範囲のどの入力にも答えます。見出しの `total` は、そのことを書いたものです。

## 検査が言うこと

ドルで数えた手数料を、セントで数える帳簿に渡すと型の誤りになり、書くべき換算が注に出ます。

```console
$ ritsu kaname check tests/docs/dollars.kn --lang ja
エラー[E102]: tests/docs/dollars.kn:7:67: 振替 `ledger.processing_fee.do` の引数 `fee` は money[USDc] です。渡しているのは money[USD] です
     7 |   ledger.processing_fee.do(payment: payment, seller: seller, fee: fee)
  = 帳簿 `ledger` の単位 `USD scale 2` は、money[USDc] で数えます。
  = money[USD] と money[USDc] は、同じ次元の違う単位です。黙って換算はしないので、`<値> in USDc` のように換算を書いてください。
```

分岐を外すと、遅れた日数が狭めないまま規則に渡ります。kaname は、日数が規則の範囲の外になる入力を挙げます。

```console
$ ritsu kaname check tests/docs/unnarrowed.kn --lang ja
エラー[E301]: tests/docs/unnarrowed.kn:15:44: `late` が、規則 `charges` の入力 `days_late` の範囲 >=1 <=1500 の外になることがあります
    15 |   charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate).interest
  = たとえば `interest(invoice: Invoice { id: "a", invoiced: 2026-01-01, amount: 50000000GBPc }, paid_on: 2026-01-01, reference_rate: 0%)` のとき、-29 になります。
  = 渡す前に `if` でその範囲に狭めるか、渡す先の範囲を広げてください。
```

エラーには、関数のベクタの中から見つけたか、式を解いて作った入力が必ず付きます。kaname が見積もった範囲の端は、実際には起きないことがあるので、例にはしません。
示すことも例を見つけることもできない場所は警告になり、生成したコードが走らせたときにそこで確かめます。
ほかの言語にまたがる確かめは ritsu が受け持ちます。支払いから規則の手数料を引いた額を売り手に渡すと、chobo が受け取らない額になることがあります。

```console
$ ritsu check tests/docs/net.kn examples/payout/rules/card_fee.rule examples/payout/books/ledger.book --root . --lang ja
ok examples/payout/rules/card_fee.rule
examples/payout/books/ledger.book: ok
  capture.do         拒否されうる理由: key_conflict
  processing_fee.do  拒否されうる理由: insufficient_balance（seller_balance(seller) at least 0.00）、key_conflict、already_refused
  キー: capture は、payment ごとに一度だけ動きます。キーが同じで seller、amount のどれかが違う二度目の呼び出しは、key_conflict で拒否されます。
  キー: processing_fee は、payment ごとに一度だけ動きます。キーが同じで seller、fee のどれかが違う二度目の呼び出しは、key_conflict で拒否されます。
エラー[kaname E401]: tests/docs/net.kn:13:1: 関数 `capture` には `total` と書いてありますが、`range` で失敗することがあります
    13 | fn capture(payment: Payment) -> ledger.capture.do total
  = 15 行目: 振替 `ledger.capture.do` の `amount` に渡す `payment.amount - fee` が、chobo の受け取らない額になることがあります（ritsu E203）
  = 書いたエフェクトに収まるよう、失敗しうる場所を示せるようにする（渡す値を `if` で範囲に狭める）か、エフェクトを広げて書いてください。
エラー[ritsu E203]: tests/docs/net.kn:15:74: 振替 `ledger.capture.do` の `amount` に渡す `payment.amount - fee` が、chobo の受け取らない額になることがあります
    15 |   ledger.capture.do(payment: payment.id, seller: payment.seller, amount: payment.amount - fee)
  = たとえば `capture(payment: Payment { id: "a", seller: "a", amount: 1USDc, international: true })` のとき、-48 になります。
  = chobo が受け取る額は、単位のいちばん小さい刻みで 0 から 9223372036854775807 までです。
  = kaname の生成したコードは、振替を組み立てるところで額を確かめます。関数は、そこで `range` で失敗します。
  = 振替の前に `if` で額を狭めるか、負の額は向きの違う振替に分けてください。
ritsu check: ファイル 3 個（rulec 1、chobo 1、kaname 1）。検査を通らないもの 1 個（エラー 2 件）。言語の境目: 確かめた 2 か所、決められない 0 か所
```

## エフェクト

関数の型には、どこまで確かめたかが載ります。
**total** な関数は、規則や koyomi の日付と同じく、宣言した範囲のどの入力にも答えます。
**fails** の関数には、検査が示せなかった場所があり、`rule`、`date`、`range`、`overflow`、`zero` のどれで失敗しうるかも型に載ります。
**diverges** の関数は、木をたどる関数（[examples/catalog](examples/catalog)）のように再帰を持ちます。止まることは確かめません。
エフェクトは kaname が推論し、見出しに書いてあれば、それに収まるかを確かめます（E401）。呼んだ関数のエフェクトは、呼んだ側に伝わります。ほかの関数に渡した関数のエフェクトは、渡した側に伝わります。
生成するコードの形もエフェクトで決まります。Go では失敗しうる関数だけが `(T, error)` を返し、TypeScript と Python では、関数ごとに失敗の種類の型が付きます。

## 判断は表に書く

kaname は計算ができるので、判断をコードに書けてしまいます。たとえば、合計を比べて送料を選ぶ `if` です。
表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。
kaname はこれを警告します。規則を呼ぶ前に、値を規則の範囲に狭めるだけの分岐は警告しません。

```console
$ ritsu kaname check tests/doc/decisions.kn --lang ja
警告[W301]: tests/doc/decisions.kn:20:24: `weight` が、規則 `media_mail` の入力 `weight` の範囲 >=1oz <=240oz に収まるかを決められません
    20 |     media_mail(weight: weight, media_only: all(lines, l => l.media)).price
  = 範囲の解析では、値は >=0oz <=240oz です。関数のベクタにも、外れる入力はありませんでした。
  = 生成したコードが、走らせたときにこの場所で確かめます。
  = `if` で範囲に狭めると、範囲の解析がそれを読んで示せるようになります。
警告[W501]: tests/doc/decisions.kn:25:3: この `if` は、比べた値によって業務の定数（`0USD`、`5USD`）を選んでいます。表に書く判断です
    25 |   if total >= 35USD then 0USD else 5USD
  = 判断を rulec の表に書いて、規則を呼んでください。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。
ok tests/doc/decisions.kn: 関数 3 個: total 2 個、fails rule 1 個
```

## 言語

- **値**：単位付きの数、お金、量、刻みのある率、日付、真偽、文字列（コードポイントの並び）、レコード、列挙と、値の下にフィールドを持つ列挙（直和型）、リスト、辞書、集合、値の無いことがある値、業務として拒むことを値で返す `result[T, E]`。
- **範囲**：引数とフィールドに範囲や要素の数を書けます（`数量(qty) : number range >=1 <=100`）。書かない値は、型の全体をとります。
- **式**：`let`、`if`、`match`（`else` とリテラルの分岐も）、ほかのレコードの一部を替えたレコード（`分類 { ..分類, 下の分類: 残す }`）、どの型の値も比べられる `=`、`err` か `none` をそのまま関数の答えにする `let x = try e`。
- **組み込みの関数**は、関数の形でも、値のあとに書く形でも書けます（`xs.sort_by(x => x.due)`、`在庫.get(品番)`、`a.union(b)`、`入力.trim()`、`テキスト.parse_number()`）。辞書と集合は鍵の順に回り、文字列はコードポイントで数え、切り、並べます。どの出力先でも同じです。
- **関数**：型の引数を持つ総称の関数（鍵にできる型には `K: key`）、関数を受け取る関数（ラムダも、名前で渡す関数も）、再帰、モジュールの中だけで使う `private`。
- rulec の規則、koyomi の日付、chobo の振替を、ritsu の型のまま**呼べます**。
- **持たないもの**：入出力、書き換えられる変数とループ、浮動小数点、関数を答える関数、正規表現（[DESIGN の 29 章](DESIGN.md)）。

言語の全部は [docs/reference.md](docs/reference.md)（英語）にあります。

## 生成するコード

`ritsu gen` は、TypeScript、Python、Go のパッケージごとに、モジュールのコードを `glue/` の下に書きます。正確な分数、int64 の確かめ、五つの丸め、コードポイントで扱う文字列、鍵の順に回す辞書と集合を持つ補助のモジュールも一緒に書き、ほかには何にも依存しません。
直和型の値は、TypeScript では `kind` を持つオブジェクト、Python では凍結したデータクラス、Go ではインターフェースを満たす構造体で、`result` は補助のモジュールの `Ok` と `Err` です。お金と量は、規則と帳簿と同じく、パッケージの単位ごとの型で受け渡します。
ホストが呼ぶ関数は、引数が宣言に収まるかを最初に確かめます。範囲の解析が示せなかった場所は、生成したコードがそこで確かめ、参照インタプリタと同じ種類の `KanameError` で失敗します。
振替は値で、その型には、帳簿がそれを拒否しうる理由が載ります。帳簿ごとの関数が、chobo のクライアントでその振替を当てます。
Functional Core の四つの例は、`host/` の下に TypeScript、Python、Go のホストを持ちます。どれも `ritsu gen` が書くパッケージを呼び、同じものを出力します。

## 人が読むページ

`ritsu kaname doc` は、判断を承認する人と、関数がどこまで確かめられたかを読むエンジニアのためのページを書きます。
呼ぶ規則と日付のファイル（rulec と koyomi が描いたそれぞれのページ付き）、組み立てる振替、関数（total なもの、実行時に確かめるものとその場所と理由、止まらないかもしれないもの）、コードにある判断、例の答えが載ります。
Markdown はプルリクエストに貼るためのもので、HTML は外のものを何も読まない一つのファイルです。
例のページは [遅延の請求のページ](../ritsu/tests/golden/kaname-doc/late_payment.ja.md) にあります（規則と日付のページは除いてあります）。

## AI エージェント向けのスキル

[skills/kaname](../../skills/kaname/SKILL.md) は、モジュールを書いたり直したりするエージェントのためのスキルです。書く流れ、言語の一枚の要約、人に聞くこと、kaname の診断と ritsu の言語をまたぐ診断の直し方があります。
`SKILL.md` のほかのページは `docs/` からコピーしたもので、`skills/sync.sh` がコピーします。

## インストール

kaname は ritsu と一緒に入ります。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu kaname …` で走らせます。
`kaname` という名前のリンクから呼ぶと、ritsu は kaname として動き、rulec、koyomi、chobo がつながります。リリースのアーカイブにはリンクが入っています。手で作るなら、ritsu の隣で `ln -s ritsu kaname` とします。
kaname のクレートのバイナリはほかの言語を持たないので、規則、日付のファイル、帳簿を読むモジュールでは E202 で止まります。

## コマンド

```console
$ ritsu kaname check examples/order_flow/order_flow.ja.kn --lang ja
$ ritsu kaname run examples/order_flow/order_flow.ja.kn 順に進める --input events.ja.json --trace --lang ja
$ ritsu kaname vectors examples/order_flow/order_flow.ja.kn --function 次 --lang ja
$ ritsu kaname doc examples/late_payment/late_payment.ja.kn --root examples/late_payment --format html --lang ja
$ ritsu kaname api examples/late_payment/late_payment.ja.kn --root examples/late_payment
$ ritsu kaname explain E301 --lang ja
```

`run` は、JSON で渡した入力で関数を一つ参照インタプリタに流します。`vectors` は、関数ごとに選んだ入力を、その答えと一緒に書き出します。
どのコマンドも `--lang ja|en`（無ければ `KANAME_LANG`、次に `RITSU_LANG`、どちらも無ければ英語）と `--help` を受け付けます。
終了コードは、エラーが無ければ 0、エラーがあれば 1、引数の誤り、読めないファイル、つながっていない言語のファイルなら 2 です。
言語とコマンドの全部は [docs/reference.md](docs/reference.md)（英語）にあります。

## 例

どの例も英語の版があり、その隣に日本語の版（`<名前>.ja.kn`）があります。
初めの四つは、ほかの言語を読まない Functional Core の例で、`host/` の下に TypeScript、Python、Go のホストを持ちます。

- [examples/order_flow](examples/order_flow)：イベントで注文を状態から状態へ進める例です。状態はフィールドを持つ列挙の値で、次の状態か、イベントを拒む理由を `result` で返します。
- [examples/stock_diff](examples/stock_diff)：品番から棚にある数への辞書で、倉庫の二回の棚卸しを品番ごとに比べる例です。一日の入出庫を品番ごとにまとめて棚卸しの数に足すこと、発注する品番の集合も求めます。
- [examples/handles](examples/handles)：人が入力したハンドルを、サービスが持つ形（空白、大文字と小文字）にしてから、決まりに合うかを確かめ、合わない理由を全部返す例です。テキストからハンドルと番号の組を `try` で読みます。
- [examples/catalog](examples/catalog)：自分を含むレコードで書いた、カタログの分類の木の例です。品物ごとのパス、空の分類を除いた木、呼ぶ側が渡す関数で名前を替えた木、総称の関数が分類ごとに選んだものを求めます。

あとの四つは、規則・日付・帳簿を呼ぶ例で、日本語の版は日本語の版の内側のファイルを読みます。規則は、転記した文書を、隣に置いて固定したコピーから引いています。

- [examples/payout](examples/payout)：マーケットプレイスが売り手のためにカードの支払いを受ける例です。決済の事業者の手数料を rulec の規則で出し、chobo の帳簿の振替を二つ組み立てて返します。振替を当てるのはホストです。手数料は、カードの決済の事業者が公開している加盟店の手数料から転記しました。
- [examples/late_payment](examples/late_payment)：遅れて払われた請求書について、売り手が請求できるものを出す例です。期日を koyomi の日付のファイルで、定額と利息を規則で決め、kaname が遅れた日数を数えます。英国の Late Payment of Commercial Debts (Interest) Act 1998 と、GOV.UK の手引きから転記しました。
- [examples/media_mail](examples/media_mail)：カートの重さ（品物の重さに数量を掛けて足したもの）と、どの品物もメディアかを、Media Mail の料金の規則に渡す例です。規則は、Media Mail にならない小包には none を答えます。米国郵便公社の料金表 Notice 123 から転記しました。
- [examples/discount_split](examples/discount_split)：請求書の値引きを、明細の金額の比で配る例です。`allocate` を使う規則を明細ごとに一回ずつ呼び、途中までの合計を `running_totals` で渡すので、配った分の合計がちょうど値引きになります。バージニア州の値引きの規則 23VAC10-210-250 から転記しました。

## どう確かめているか

`cargo test -p kaname` が、言語を例とテストのモジュールに当て、生成したコードを走らせます。

- 丸めは rulec の丸めと 360,600 通りの値と刻みで、日付は ritsu-ports、koyomi、rulec と 3,652,059 日のすべてで、同じ答えになることを確かめます。
- rulec のコーパスの `derive` と `define` の全部（85 のファイルの 143 行）を kaname の式として読み、rulec と同じ型になり、rulec のベクタ 20,033 件で同じ値になることを確かめます。
- 範囲の解析を参照インタプリタと突き合わせます。例とテストのモジュールのすべての関数で、ベクタと、引数の宣言から選んだ 200 件の入力を流し（139,754 回の評価）、5,009 の式に来た値 3,452,025 個が、どれも解析の言う範囲に入り、失敗した 1,617 回が、どれも関数のエフェクトの言う種類で失敗することを確かめます。
- 評価と範囲の解析を Lean で書き直し（ritsu の一つの Lean のパッケージの `proofs/KanameCore`）、範囲の解析が健全であることと、total と推論した関数が失敗しないことを、関数を引数に取らない関数について証明しています（関数を引数に取る関数は、試験でモデルと突き合わせていますが、証明はまだありません）。`cargo test -p ritsu-model` が、59 のモジュールの 14,092 の評価で、モデルを参照インタプリタと範囲の解析に突き合わせます。
- TypeScript、Python、Go に生成したコードが、52 のモジュールのすべてのベクタと、引数の宣言の外の入力と、自分を含む型の値をホストが渡せるいちばん深くまで入れ子にした値とその一段深い値に、参照インタプリタと同じ答えを返すことを、英語と日本語の両方で確かめます（TypeScript は 16,717 件、Python は 16,633 件、Go は 16,613 件のすべてが一致。型の全体の外の入力は、その値を持てる言語で流します）。生成したコードは `tsc --strict --erasableSyntaxOnly`、`mypy --strict`、gofmt と `go vet` を通ります。
- どの診断のコードにも、それを出す一番小さなモジュールが英語と日本語の両方であります。`doc` のページは golden で、中の規則と日付のページは rulec と koyomi が描いたものです。HTML は Chrome で開いて確かめます。
- Functional Core の四つの例のホスト（TypeScript、Python、Go）が、`ritsu gen` が書くパッケージを呼び、`host/output.txt` のとおりに出力することを確かめます（ritsu の `tests/kaname.rs`）。
- この README と `docs/` とスキルに載せた出力、診断、`.kn` とホストの行は本物です。`tests/docs.rs` が突き合わせ、コマンドは ritsu の `tests/kaname.rs` が走らせます。サイトの kaname のページも、ritsu の `tests/website.rs` が同じように確かめます。

## 状態

言語、`check`、`run`、`vectors`、`doc`、`api`、`explain`、`ritsu gen` が書くコードはできています（[DESIGN.md](DESIGN.md)）。
まだ作っていないもの：dandori のフローから kaname の関数を呼ぶこと、koyomi のカレンダーと日付の時刻、渡す入力に合わせて規則の出力の範囲を細かくすること、サイトのブラウザで試すページでモジュールを扱うこと（DESIGN の 19 章）。

## ライセンス

[Apache License, Version 2.0](../../LICENSE-APACHE) と [MIT license](../../LICENSE-MIT) のどちらかを選んで使えます。
