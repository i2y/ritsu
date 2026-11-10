# kaname

Functional Core を書くための、小さな汎用の言語です。
プログラムのうち、入出力を持たずに計算して決める部分を書きます。イベントで状態を進めること、入力を確かめてサービスが持つ形にすること、数を集めて比べること、木の形のデータを作り替えること、業務の規則のまわりの計算を、レコード、フィールドを持つ値のある列挙、リスト、辞書、集合、文字列の上の関数として書き、ジェネリクス、`result`、関数の値も使えます。
リクエストや時刻やデータベースを読み、状態を持ち、決めたことを行うほかの部分は、kaname を呼ぶホストの言語（Imperative Shell）に残ります。
コアは、ritsu の言語と同じ決まり（単位、通貨と税区分、率の刻み、範囲、丸め、int64）で確かめられ、関数ごとに何が起こりうるか（`total`、`fails …`、`diverges`）も検査が推し量ります。
rulec の規則、koyomi の日付、chobo の帳簿の振替も ritsu の型のまま呼べ、`ritsu gen` が、それらのコードの隣に、何にも依存しない TypeScript、Python、Go、Rust のコードを書きます。Rust のコードは、どの言語のホストからも JSON のオブジェクト一つで呼べる一つの Wasm のモジュールにも組めます。
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
警告[kaname W603]: examples/order_flow/order_flow.ja.kn:92:1: 性質 `閉じた注文はイベントを受け付けない` は、`when` の行が成り立つ 170 個のベクタ（782 個のうち）のすべてで成り立ちましたが、示されていません。order_flow_ja.lean の証明は `--lean` で確かめます
    92 | property 閉じた注文はイベントを受け付けない(a_closed_order_takes_no_event)(注文(order): 注文, イベント(event): イベント)
ok examples/order_flow/order_flow.ja.kn: 関数 5 個、total
警告[kaname W603]: examples/order_flow/order_flow.kn:92:1: 性質 `a_closed_order_takes_no_event` は、`when` の行が成り立つ 170 個のベクタ（782 個のうち）のすべてで成り立ちましたが、示されていません。order_flow.lean の証明は `--lean` で確かめます
    92 | property a_closed_order_takes_no_event(order: Order, event: Event)
ok examples/order_flow/order_flow.kn: 関数 5 個、total
ritsu check: ファイル 2 個（kaname 2）。どれも検査を通りました（警告 2 件）。言語の境目: 確かめた 0 か所、決められない 0 か所
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
警告[kaname W603]: examples/late_payment/late_payment.ja.kn:28:1: 性質 `期日までに払えば請求は無い` は、`when` の行が成り立つ 10 個のベクタ（19 個のうち）のすべてで成り立ちましたが、示されていません。late_payment_ja.lean の証明は `--lean` で確かめます
    28 | property 期日までに払えば請求は無い(nothing_is_claimed_when_paid_on_time)(請求書(invoice): 請求書, 支払日(paid_on): date range >=2026-01-01 <=2029-12-31, 基準金利(reference_rate): rate[step 0.01%] range >=0% <=15%)
ok examples/late_payment/late_payment.ja.kn: 関数 1 個、total
警告[kaname W603]: examples/late_payment/late_payment.kn:28:1: 性質 `nothing_is_claimed_when_paid_on_time` は、`when` の行が成り立つ 10 個のベクタ（19 個のうち）のすべてで成り立ちましたが、示されていません。late_payment.lean の証明は `--lean` で確かめます
    28 | property nothing_is_claimed_when_paid_on_time(invoice: Invoice, paid_on: date range >=2026-01-01 <=2029-12-31, reference_rate: rate[step 0.01%] range >=0% <=15%)
ok examples/late_payment/late_payment.kn: 関数 1 個、total
ritsu check: ファイル 6 個（rulec 2、koyomi 2、kaname 2）。どれも検査を通りました（警告 2 件）。言語の境目: 確かめた 2 か所、決められない 0 か所
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

フィールドと引数は、手書きのコードが値を取ってくる出どころを書けます（[examples/order_bill](examples/order_bill)）。

```kn
  数量(qty) : number  range >=1 <=20  fed by jsonschema "schemas/order.ja.schema.json" def 明細 property 数量
```

`ritsu check` が相手のファイルに、とりうる値を尋ね、モジュールが受け取らない値があれば止まります。確かめるのは値の範囲です。手書きのコードが本当にその値を運ぶかは何も確かめず、ページにもそう書きます。

## エフェクト

関数の型には、どこまで確かめたかが載ります。
**total** な関数は、規則や koyomi の日付と同じく、宣言した範囲のどの入力にも答えます。
**fails** の関数には、検査が示せなかった場所があり、`rule`、`date`、`range`、`overflow`、`zero` のどれで失敗しうるかも型に載ります。
**diverges** の関数は、木をたどる関数（[examples/catalog](examples/catalog)）のように再帰を持ちます。止まることは確かめません。
エフェクトは kaname が推論し、見出しに書いてあれば、それに収まるかを確かめます（E401）。呼んだ関数のエフェクトは、呼んだ側に伝わります。ほかの関数に渡した関数のエフェクトは、渡した側に伝わります。
生成するコードの形もエフェクトで決まります。Go では失敗しうる関数だけが `(T, error)` を、Rust では `Result<T, KanameError>` を返し、TypeScript と Python では、関数ごとに失敗の種類の型が付きます。

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

## 性質と証明

関数には性質を書けます。`examples` が挙げる入力だけでなく、入力の全部についての主張です。
`property` は関数と同じ形で書き、`description`、性質が語る入力を絞る `when` の行、`true` になるべき最後の行を持ちます（[examples/refund_split](examples/refund_split) は、注文の明細に返金を配る例です）。

```kn
property 配分の合計は返金額(shares_add_up)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc)
  description "配った分を足すと返金額になる"
  sum(按分する(明細: 明細, 返金: 返金), 一件 => 一件.配分) = 返金

property 残りより多くは配らない(no_share_is_more_than_its_left)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc)
  description "残りの合計を超えない返金なら、どの明細にも、その明細の残りより多くは配らない"
  when 返金 <= sum(明細, 一件 => 一件.残り)
  all(按分する(明細: 明細, 返金: 返金), 一件 => 一件.配分 <= 一件.残り)
```

読み方は次のとおりです。引数が宣言した範囲に入り、どの `when` の行も成り立つ入力なら、最後の行は `true` になります。
`ritsu check` は、性質をいつも引数のベクタ（`kaname vectors` が選ぶ入力）で確かめます。
成り立たない入力があれば、その入力を挙げてエラーにします。

```console
$ ritsu kaname check tests/properties/shares.ja.kn --lang ja
エラー[E603]: tests/properties/shares.ja.kn:12:1: 性質 `どの明細にも配る` は、明細: [明細 { 品番: "a", 残り: 500000USDc }, 明細 { 品番: "a", 残り: 500000USDc }], 返金: 1USDc で成り立ちません
    12 | property どの明細にも配る(every_line_gets_a_share)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc)
  = 15 行目: `配分` は [0USDc, 1USDc]
  = `check` は、性質を引数のベクタで参照インタプリタに流します。この入力では、`when` の行がどれも `true` で、最後の行が `false` になりました。
  = 関数を直すか、`when` の行か引数の範囲で性質を狭めるか、性質を直してください。
```

すべてのベクタで成り立っても、性質を示したことにはなりません。ベクタは入力の一部だからです。
`check` は W603 で、足りないものを言います（証明を書く前の `refund_split` の三つのうち最初の一つを、短くして載せます）。

```text
警告[W603]: examples/refund_split/refund_split.ja.kn:18:1: 性質 `配分の合計は返金額` は、12 個のベクタのすべてで成り立ちましたが、示されていません。証明がありません（refund_split_ja.lean に、この性質の定理がありません）
    18 | property 配分の合計は返金額(shares_add_up)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc)
  = `ritsu gen --target lean` が、定理の名前と `sorry` だけの下書き `samples/refund_split_ja.lean` を書きます。それを .kn の隣に refund_split_ja.lean として置き、`sorry` を証明に置き換えてください。
…
ok examples/refund_split/refund_split.ja.kn: 関数 1 個、total
```

示す方法は証明だけで、証明は kaname の外で書きます。
`ritsu gen --target lean` は、関数を Lean 4 の定義に、性質を定理の文に書き出し、証明を `sorry` にした下書きも書きます。

```console
$ ritsu gen --target lean
```

```text
generated: generated/lean/lean-toolchain
generated: generated/lean/lakefile.toml
generated: generated/lean/.gitignore
generated: generated/lean/Kaname/Prelude.lean
generated: generated/lean/Kaname/refund_split.lean
generated: generated/lean/samples/refund_split.lean
```

`Kaname/refund_split.lean` に定義と性質ごとの文が入ります。たとえば次のとおりです（英語の版のモジュールの出力で、日本語の版は名前の空間が `Kaname.refund_split_ja` になります）。

```lean
abbrev shares_add_up : Prop :=
  ∀ (lines : List Line) (refund : Int),
    1 ≤ lines.length → lines.length ≤ 10 → (∀ e₁ ∈ lines, e₁.Within) →
    0 ≤ refund → refund ≤ 10000000 →
    ((List.map (fun (s : Share) => s.share) (split lines refund)).sum) = refund
```

人か、エージェントが、.kn の隣の `refund_split.lean` に証明を書きます。`samples/refund_split.lean` を写し、`sorry` を証明に置き換えます。
定理は `Proofs.<モジュール>.<性質>` という名前で、型は `ritsu gen` が書いた文そのものです。

```lean
import Kaname.refund_split

namespace Proofs.refund_split

open Kaname Kaname.refund_split

/-- The shares, each the refund up to its line's end less the refund before it, telescope to the
refund of the whole. -/
theorem shares_add_up : Kaname.refund_split.shares_add_up := by
  intro lines refund hlen _ hwithin _ _
  obtain ⟨S, hS⟩ : ∃ S, S = (lines.map fun l => l.left).sum := ⟨_, rfl⟩
  have hT : 0 < S := hS ▸ sum_pos hlen fun l hl => (hwithin l hl).1
  have key : (split lines refund).map (fun s => s.share)
      = (runningTotals lines (fun l => l.left)).map (fun r => allocate refund r.upto S - allocate refund r.before S) := by
    unfold split
    rw [List.map_map]
    apply List.map_congr_left
    intro r hr
    simp only [Function.comp, runningTotals_total _ _ r hr, hS]
  rw [key, runningTotals_telescope lines (fun l => l.left) (fun x => allocate refund x S), ← hS]
  rw [allocate_self refund hT, allocate_zero]
  omega

/-- A share is the refund up to its line's end less the refund before it, and `allocate` does not go
down as its share grows. -/
theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by
  intro lines refund hlen _ hwithin hr0 _ s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hs
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have hleft := (hwithin r.item hitem).1
  have hw : ∀ l ∈ lines, 0 ≤ l.left := fun l hl => by have := (hwithin l hl).1; omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hmono := allocate_mono (t := refund) hr0 (htot ▸ hT) (show r.before ≤ r.upto by omega)
  simp only
  omega

/-- What one line gets is at most its width of the whole, when the refund is no more than the whole. -/
theorem no_share_is_more_than_its_left : Kaname.refund_split.no_share_is_more_than_its_left := by
  intro lines refund hlen _ hwithin hr0 _ hle s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hs
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have hleft := (hwithin r.item hitem).1
  have hw : ∀ l ∈ lines, 0 ≤ l.left := fun l hl => by have := (hwithin l hl).1; omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hsub := allocate_sub_le (t := refund) hr0 (htot ▸ hle) (htot ▸ hT) (show r.before ≤ r.upto by omega)
  simp only
  omega

end Proofs.refund_split
```

`sum_pos`、`allocate_self`、`runningTotals_telescope` などの補題は、`ritsu gen` が証明と一緒に書く `Kaname/Prelude.lean` にあります。
二つ目と三つ目の証明は、配分を一つ取り出して示します。`List.mem_map.mp` で、その配分のもとになった `running_totals` の要素を取り出し、`runningTotals_items` でその要素の明細が `lines` にあることを示すと、明細の範囲が `Within` から分かります。
証明は Lean の core だけで書き、Mathlib は使いません。`linarith` の代わりは `omega` で、`set` のように和に名前を付けるには `obtain ⟨S, hS⟩ : ∃ S, S = … := ⟨_, rfl⟩` と書きます。
補題の一覧と、Mathlib のほかの tactic の代わりは [docs/reference.md](docs/reference.md#what-a-proof-is-made-of)（英語）にあります。
書きながら試すには、`generated/lean/` で `lake build` を走らせます。kaname が書く Lean はビルドしても警告を出さないので、出た警告は証明のものです。
`ritsu check --lean` は .kn からパッケージを作り直すので、その前に `lake build` を走らせる必要はありません。

`ritsu check` は証明を探さず、`--lean` が無ければ証明を読みません。
`ritsu check --lean` は、.kn から Lean のパッケージを作り直し、隣の証明のファイルを Lean に通して、どの定理も文と同じ型で、Lean の三つの公理（`propext`、`Classical.choice`、`Quot.sound`）のほかに頼らないことを確かめます。`sorry`、自分で置いた公理、`native_decide` を使った証明は E604 です。
性質を**示した**と言うのは、Lean が証明を受け付けたときだけです。
証明のファイルは、Lean がビルドする前に文字として読みます。ビルドの中で何かが走らないように、`Kaname` の外の import、コードを走らせるコマンド、自分で足した構文を持つファイルは、先に断ります。

証明のファイルを置いたあとも、`--lean` の無い `check` は W603 を出します。理由は「refund_split.lean の証明は `--lean` で確かめます」に変わります。
`--lean` を付けて、Lean がすべての証明を受け付ければ、W603 は消えます。

```console
$ ritsu check --lean examples/refund_split --lang ja
ok examples/refund_split/refund_split.ja.kn: 関数 1 個、total
ok examples/refund_split/refund_split.kn: 関数 1 個、total
ritsu check: ファイル 2 個（kaname 2）。どれも検査を通りました。言語の境目: 確かめた 0 か所、決められない 0 か所
```

`sorry` のまま残した証明（ここでは `no_share_is_negative` の証明）は E604 です。

```text
エラー[kaname E604]: examples/refund_split/refund_split.lean:26:1: `no_share_is_negative` の証明は、`propext`・`Classical.choice`・`Quot.sound` のほかに `sorryAx` に頼っています
    26 | theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by
  = `sorry` を証明に置き換えてください。
  = `--lean` は、refund_split.kn から書き直した Lean に対して証明をビルドし、性質ごとに、型が `Kaname.refund_split.<性質>` で、`propext`・`Classical.choice`・`Quot.sound` のほかの公理に頼らない定理 `Proofs.refund_split.<性質>` を受け付けます。
```

初めて `--lean` を使うときは、Lean が要ります。
Lean のバージョン管理の道具 [elan](https://github.com/leanprover/elan) を入れてください。elan が最初に走らせる `lake` が、証明が使う toolchain（`leanprover/lean4:v4.34.0`、ライブラリと合わせて約 2.7 GB）を一度だけダウンロードします。
`lake` が無ければ、`--lean` は理由を言って終了コード 2 で止まります。
ほかに Lean を使う所はありません。`--lean` の無い `ritsu check` と、ほかの言語への `ritsu gen` は、Lean を走らせません。

まだ Lean に書けないものは W604 になります。性質のベクタでの確かめはします。書けないのは、辞書、集合、`=` 以外の文字列の組み込み、`sort_by`・`distinct`・`max_by`・`min_by`・`to_dict`・`group_by`、キーに限った型の引数（`K: key`）です。
規則や日付のファイルの答えに頼る性質も示せません。証明が使えるのは、規則や日付の口が教える事実だけだからです（W603 がそう言います）。
止まらないかもしれない関数は書かず、失敗しうる関数についての性質は書けません（E401）。
`when` の行がどのベクタでも成り立たなければ W605 です。そういう性質は、何を言っていても証明できてしまいます。

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

`ritsu gen` は、TypeScript、Python、Go、Rust のパッケージごとに、モジュールのコードを `glue/` の下に書きます。正確な分数、int64 の確かめ、五つの丸め、コードポイントで扱う文字列、鍵の順に回す辞書と集合を持つ補助のモジュールも一緒に書き、ほかには何にも依存しません。
直和型の値は、TypeScript では `kind` を持つオブジェクト、Python では凍結したデータクラス、Go ではインターフェースを満たす構造体、Rust では構造体を持つ `enum` で、`result` は補助のモジュールの `Ok` と `Err`（Rust では標準の `Result`）です。お金と量は、規則と帳簿と同じく、パッケージの単位ごとの型で受け渡します。
ホストが呼ぶ関数は、引数が宣言に収まるかを最初に確かめます。範囲の解析が示せなかった場所は、生成したコードがそこで確かめ、参照インタプリタと同じ種類の `KanameError` で失敗します。
振替は値で、その型には、帳簿がそれを拒否しうる理由が載ります。帳簿ごとの関数が、chobo のクライアントでその振替を当てます。
Functional Core の四つの例は、`host/` の下に TypeScript、Python、Go のホストを持ちます。どれも `ritsu gen` が書くパッケージを呼び、同じものを出力します。

Rust のパッケージ（`ritsu gen --target rust`）は一つのクレートで、ホストはパスで依存に足します。ワークフローとゲートに尋ねるコードは入らず、入らないファイルをファイルごとに警告で言います。
kaname のモジュールがあれば、`Cargo.toml` の頭に、クレートを import の無い一つの Wasm のモジュールに組む行があり、横の `.wit` でそれを component にできます。`call` は `{"module", "function", "in"}` を受け取り、パッケージのホストが呼ぶ関数のどれにも、その入力の `kaname vectors` の行で答えます。振替は値で答えます。
全体は、言語ごとのコードと一緒に [docs/reference.md](docs/reference.md#what-gen-writes) にあります。

## 人が読むページ

`ritsu kaname doc` は、判断を承認する人と、関数がどこまで確かめられたかを読むエンジニアのためのページを書きます。
呼ぶ規則と日付のファイル（rulec と koyomi が描いたそれぞれのページ付き）、組み立てる振替、関数（total なもの、実行時に確かめるものとその場所と理由、止まらないかもしれないもの）、性質（.kn の隣に証明のファイルがあるか）、コードにある判断、例の答えが載ります。
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
$ ritsu check --lean examples/refund_split
```

`run` は、JSON で渡した入力で関数を一つ参照インタプリタに流します。`vectors` は、関数ごとに選んだ入力を、その答えと一緒に書き出します。
どのコマンドも `--lang ja|en`（無ければ `KANAME_LANG`、次に `RITSU_LANG`、どちらも無ければ英語）と `--help` を受け付けます。
終了コードは、エラーが無ければ 0、エラーがあれば 1、引数の誤り、読めないファイル、つながっていない言語のファイルなら 2 です。
言語とコマンドの全部は [docs/reference.md](docs/reference.md)（英語）にあります。

## 例

どの例も英語の版があり、その隣に日本語の版（`<名前>.ja.kn`）があります。
初めの四つは、ほかの言語を読まない Functional Core の例で、`host/` の下に TypeScript、Python、Go のホストを持ちます。九つ目の [examples/order_bill](examples/order_bill) もそうで、ホストは持たず、値の出どころを `fed by` で書きます。十個目の [examples/refund_split](examples/refund_split) もホストを持たず、関数の性質を書きます。

- [examples/order_flow](examples/order_flow)：イベントで注文を状態から状態へ進める例です。状態はフィールドを持つ列挙の値で、次の状態か、イベントを拒む理由を `result` で返します。
- [examples/stock_diff](examples/stock_diff)：品番から棚にある数への辞書で、倉庫の二回の棚卸しを品番ごとに比べる例です。一日の入出庫を品番ごとにまとめて棚卸しの数に足すこと、発注する品番の集合も求めます。
- [examples/handles](examples/handles)：人が入力したハンドルを、サービスが持つ形（空白、大文字と小文字）にしてから、決まりに合うかを確かめ、合わない理由を全部返す例です。テキストからハンドルと番号の組を `try` で読みます。
- [examples/catalog](examples/catalog)：自分を含むレコードで書いた、カタログの分類の木の例です。品物ごとのパス、空の分類を除いた木、呼ぶ側が渡す関数で名前を替えた木、総称の関数が分類ごとに選んだものを求めます。
- [examples/refund_split](examples/refund_split)：注文の明細に、まだ払われていない残りの比で返金を配る例です。配った分の合計が返金額になること、負の額は配らないこと、明細の残りより多くは配らないことを、関数の性質として書きます。証明は Lean で、モジュールの隣に置きます。

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
- TypeScript、Python、Go、Rust に生成したコードとパッケージの Wasm が、52 のモジュールのすべてのベクタと、引数の宣言の外の入力と、自分を含む型の値をホストが渡せるいちばん深くまで入れ子にした値とその一段深い値に、参照インタプリタと同じ答えを返すことを、英語と日本語の両方で確かめます（TypeScript は 16,874 件、Python は 16,790 件、Go と Rust と Wasm はそれぞれ 16,770 件のすべてが一致。型の全体の外の入力は、その値を持てる言語で流します）。生成したコードは `tsc --strict --erasableSyntaxOnly`、`mypy --strict`、gofmt、`go vet`、`cargo clippy -- -D warnings` を通ります。パッケージの component は wasmtime の下で、モジュールが Node の下で答えるのと同じに答えます。ritsu のテストは、テストのプロジェクトで `ritsu gen --target rust` が書くものを、帳簿の三つの置き場所のそれぞれと wasm32 で組みます。
- どの診断のコードにも、それを出す一番小さなモジュールが英語と日本語の両方であります。`doc` のページは golden で、中の規則と日付のページは rulec と koyomi が描いたものです。HTML は Chrome で開いて確かめます。
- Functional Core の四つの例のホスト（TypeScript、Python、Go）が、`ritsu gen` が書くパッケージを呼び、`host/output.txt` のとおりに出力することを確かめます（ritsu の `tests/kaname.rs`）。
- この README と `docs/` とスキルに載せた出力、診断、`.kn` とホストの行は本物です。`tests/docs.rs` が突き合わせ、コマンドは ritsu の `tests/kaname.rs` が走らせます。サイトの kaname のページも、ritsu の `tests/website.rs` が同じように確かめます。

## 状態

言語、`check`、`run`、`vectors`、`doc`、`api`、`explain`、`ritsu gen` が書くコードはできています（[DESIGN.md](DESIGN.md)）。
まだ作っていないもの：dandori のフローから kaname の関数を呼ぶこと、koyomi のカレンダーと日付の時刻、渡す入力に合わせて規則の出力の範囲を細かくすること、サイトのブラウザで試すページでモジュールを扱うこと、モジュールを Rust のライブラリにすること（DESIGN の 19 章）。

## ライセンス

[Apache License, Version 2.0](../../LICENSE-APACHE) と [MIT license](../../LICENSE-MIT) のどちらかを選んで使えます。
