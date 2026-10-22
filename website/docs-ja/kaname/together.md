# 規則・日付・帳簿を呼ぶ

[rulec](../../rulec/ja/) の規則は判断を、koyomi の日付のファイルは日付を、[chobo](../../chobo/ja/) の帳簿は額の動きを受け持ち、それぞれの言語の検査を受けています。そのまわりのコードは、規則の入力を組み立て、一つの答えを次の呼び出しに渡し、額を帳簿に渡します。このコードをホストの言語で書いても、`ritsu gen` が書くパッケージは単位ごとに一つの型を持つので、ドルで数えた手数料をセントの帳簿に渡せば、ホストの言語の型の検査が止めます。けれども、そのコードが何を計算するかは、どの検査も読みません。丸めの書き忘れ、規則が受け取る範囲を外れる合計、規則の前提を破る途中までの合計、int64 を超える積がそうです。kaname で書けば、規則と日付と帳簿をそれぞれの型のまま呼び、呼ぶ相手が宣言した型と範囲で確かめられます。

## 遅れて払われた請求書の請求

遅れて払われた請求書について、売り手が請求できるものを出す例です（`crates/kaname/examples/late_payment`）。英国の Late Payment of Commercial Debts (Interest) Act 1998 と、GOV.UK のその手引きから転記しました。

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

期日は koyomi の日付が、遅れた日数に応じて請求できる額は rulec の規則が決めます。kaname が書くのは、そのあいだの日数の計算と、規則を呼ぶ前に日数を規則の入力の範囲に狭める分岐です。`ritsu check` は、三つの言語を一度に読みます。

```console
$ cd crates/kaname/examples/late_payment
$ ritsu check --lang ja
ok rules/late_payment_charges.rule
ok rules/遅延の請求額.rule
dates/payment_terms.cal: ok — invoiced 2026-01-01〜2027-12-31 の 730 日のすべてで、どの日付も計算できます。例 2 行も合っています
dates/支払期限.cal: ok — 請求日 2026-01-01〜2027-12-31 の 730 日のすべてで、どの日付も計算できます。例 2 行も合っています
ok late_payment.ja.kn: 関数 1 個、total
ok late_payment.kn: 関数 1 個、total
ritsu check: ファイル 6 個（rulec 2、koyomi 2、kaname 2）。どれも検査を通りました。言語の境目: 確かめた 2 か所、決められない 0 か所
```

`支払日 - 期日` は −758 日から 1,431 日で、`遅れ >= 1` の分岐の中では 1 日から 1,431 日になり、規則の入力の範囲（1 から 1,500）に収まります。規則に渡す債権額と基準金利も、それぞれの範囲に収まります。それぞれのモジュールが koyomi に渡す請求日が koyomi の範囲に収まることは ritsu が確かめ、最後の行の「確かめた 2 か所」はこの二つです。そのため `遅れの請求` は、宣言した範囲のどの入力にも答えます。見出しの `total` は、そのことを書いたものです。分岐を外すと、日数が規則の範囲の外になる入力を検査が挙げます（[検査が保証すること](checks.md)）。

`ritsu kaname run` は、規則と日付のファイルをつないで関数を走らせ、呼んだところごとに答えを出します。GOV.UK の手引きの例は、1,000 ポンドを 50 日遅れて払ったもので、基準金利は 0.5% です。

```console
$ cd crates/kaname
$ ritsu kaname run examples/late_payment/late_payment.ja.kn 遅れの請求 --input tests/inputs/late_payment.ja.json --lang ja
遅れの請求(請求書: 請求書 { 番号: "inv_1", 請求日: 2026-03-02, 金額: 100000GBPc }, 支払日: 2026-05-20, 基準金利: 0.5%)
  20 行目: 期限.期日(請求日: 2026-03-02) = 2026-03-31
  23 行目: 請求額(債権額: 100000GBPc, 遅れた日数: 50, 基準金利: 0.5%) = { 定額の補償: 70GBP, 利息: 1150GBPc }
= 請求 { 期日: 2026-03-31, 遅れた日数: 50, 定額の補償: 70GBP, 利息: 1150GBPc }
```

## それぞれの型のまま

ドルで数えた手数料を、セントで数える帳簿に渡すと型の誤りになり、書くべき換算が注に出ます。

```console
$ cd crates/kaname
$ ritsu kaname check tests/docs/dollars.kn --lang ja
エラー[E102]: tests/docs/dollars.kn:7:67: 振替 `ledger.processing_fee.do` の引数 `fee` は money[USDc] です。渡しているのは money[USD] です
     7 |   ledger.processing_fee.do(payment: payment, seller: seller, fee: fee)
  = 帳簿 `ledger` の単位 `USD scale 2` は、money[USDc] で数えます。
  = money[USD] と money[USDc] は、同じ次元の違う単位です。黙って換算はしないので、`<値> in USDc` のように換算を書いてください。
```

フィールドを持つ列挙の値、辞書と集合、`result`、関数の値は、そのままでは規則にも日付にも振替にも渡せません。渡すには、`match` で分けるか、規則が受け取る型のリストや値に直します。

## 言語の境目

ほかの言語にまたがるところは、それぞれの言語が確かめたことを使って ritsu が確かめます。売り手への支払いで、支払いから規則の手数料を引いた額を売り手に渡すと、chobo が受け取らない額になることがあります。

```console
$ cd crates/kaname
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

ritsu が言語の境目で確かめるのは、次のことです。

- モジュールが規則を呼ぶすべての場所で、規則の前提が成り立つか
- 規則の日付の入力に渡す koyomi の日が、規則の範囲に収まるか
- koyomi の日付に渡す日が、日付のファイルの範囲に収まるか
- 振替に渡す額が、chobo の受け取る範囲に収まるか（帳簿がその振替を拒否しうる理由も挙げます）

示せた場所はエフェクトから外れ、決められない場所は、コードが実行時に確かめます。

## パッケージの中で

`ritsu gen` は、モジュールのコードを、それが呼ぶ規則、日付、帳簿のコードと同じ、TypeScript、Python、Go のそれぞれ一つのパッケージに書き、モジュールのコードはそれらのコードを呼びます。振替は値で、その型には、帳簿がそれを拒否しうる理由が載ります。振替を組み立てるのはモジュールで、当てるのはホストです。ホストは、パッケージが帳簿ごとに書く関数を通して、chobo のクライアントで振替を当てます。

## ほかの例

- [payout](https://github.com/i2y/ritsu/tree/main/crates/kaname/examples/payout)：マーケットプレイスが売り手のためにカードの支払いを受け、決済の事業者の手数料を規則で出し、帳簿の振替を二つ組み立てます。振替を当てるのはホストです。
- [media_mail](https://github.com/i2y/ritsu/tree/main/crates/kaname/examples/media_mail)：カートの重さと、どの品物もメディアかを、Media Mail の料金の規則に渡します。米国郵便公社の料金表 Notice 123 から転記しました。
- [discount_split](https://github.com/i2y/ritsu/tree/main/crates/kaname/examples/discount_split)：請求書の値引きを、明細の金額の比で配ります。`allocate` を使う規則を明細ごとに一回ずつ呼び、途中までの合計を `running_totals` で渡します。バージニア州の値引きの規則 23VAC10-210-250 から転記しました。
