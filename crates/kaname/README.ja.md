# kaname

Functional Core を書くための小さな言語です。
プログラムのうち、計算して決めるだけの部分を、ritsu の型のまま、関数、レコード、列挙、リストで書きます。kaname 自身は入出力を持ちません。
リクエストや時刻やデータベースを読み、決めたことを行うほかの部分は、kaname を呼ぶホストの言語（Imperative Shell）に残ります。
コアは、ritsu の言語と同じ決まり（単位、通貨と税区分、率の刻み、範囲、丸め、int64）で確かめられ、関数ごとに何が起こりうるか（`total`、`fails …`、`diverges`）も検査が推し量ります。
いまの kaname は、業務の決まりのまわりのコアに向けた形で、rulec の規則、koyomi の日付、chobo の帳簿の振替を ritsu の型のまま呼べます。より汎用に使えるようにする機能の拡張も検討しています。
kaname は [ritsu](../../README.ja.md) の九つの言語の一つです。規則と日付のファイルと帳簿を ritsu を通して読み、`ritsu gen` がそれらのコードの隣に TypeScript、Python、Go のコードを書きます。

規則は判断を、日付のファイルは日付を、帳簿は額の動きを受け持ち、それぞれの言語の検査を受けています。
ところが、それらをつなぐコードは、ふつうはホストの言語で書きます。カートから規則の入力を組み立てる、規則の答えを次の呼び出しに渡す、額を帳簿に渡す、といったコードです。
`ritsu gen` が書くパッケージは単位ごとに一つの型を持つので、ドルで数えた手数料をセントの帳簿に渡せば、ホストの言語の型の検査が止めます。
それでも、そのコードが計算するものはどの検査も読みません。丸めの書き忘れ、規則が受け取る範囲を外れる合計、規則の前提を破る途中までの合計、int64 を超える積がそうです。
kaname では、そのコードを、それらのあいだの Functional Core として ritsu の型で書き、同じ決まりで確かめます。

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
**diverges** の関数は再帰を持ちます。
エフェクトは kaname が推論し、見出しに書いてあれば、それに収まるかを確かめます（E401）。呼んだ関数のエフェクトは、呼んだ側に伝わります。
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

## 生成するコード

`ritsu gen` は、TypeScript、Python、Go のパッケージごとに、モジュールのコードを `glue/` の下に書きます。正確な分数、int64 の確かめ、五つの丸めを持つ補助のモジュールも一緒に書き、ほかには何にも依存しません。
お金と量は、規則と帳簿と同じく、パッケージの単位ごとの型で受け渡します。ホストが呼ぶ関数は、引数が宣言に収まるかを最初に確かめます。範囲の解析が示せなかった場所は、生成したコードがそこで確かめ、参照インタプリタと同じ種類の `KanameError` で失敗します。
振替は値で、その型には、帳簿がそれを拒否しうる理由が載ります。帳簿ごとの関数が、chobo のクライアントでその振替を当てます。

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
$ ritsu kaname check examples/late_payment/late_payment.ja.kn --lang ja
$ ritsu kaname run examples/late_payment/late_payment.ja.kn 遅れの請求 --input claim.ja.json --trace --lang ja
$ ritsu kaname vectors examples/late_payment/late_payment.ja.kn --function 遅れの請求 --lang ja
$ ritsu kaname doc examples/late_payment/late_payment.ja.kn --root examples/late_payment --format html --lang ja
$ ritsu kaname api examples/late_payment/late_payment.ja.kn --root examples/late_payment
$ ritsu kaname explain E301 --lang ja
```

`run` は、JSON で渡した入力で関数を一つ参照インタプリタに流します。`vectors` は、関数ごとに選んだ入力を、その答えと一緒に書き出します。
どのコマンドも `--lang ja|en`（無ければ `KANAME_LANG`、次に `RITSU_LANG`、どちらも無ければ英語）と `--help` を受け付けます。
終了コードは、エラーが無ければ 0、エラーがあれば 1、引数の誤り、読めないファイル、つながっていない言語のファイルなら 2 です。
言語とコマンドの全部は [docs/reference.md](docs/reference.md)（英語）にあります。

## 例

どの例も英語の版があり、その隣に日本語の版（`<名前>.ja.kn`。日本語の版の内側のファイルを読む）があります。
規則は、転記した文書を、隣に置いて固定したコピーから引いています。

- [examples/payout](examples/payout)：マーケットプレイスが売り手のためにカードの支払いを受ける例です。決済の事業者の手数料を rulec の規則で出し、chobo の帳簿の振替を二つ組み立てて返します。振替を当てるのはホストです。手数料は、カードの決済の事業者が公開している加盟店の手数料から転記しました。
- [examples/late_payment](examples/late_payment)：遅れて払われた請求書について、売り手が請求できるものを出す例です。期日を koyomi の日付のファイルで、定額と利息を規則で決め、kaname が遅れた日数を数えます。英国の Late Payment of Commercial Debts (Interest) Act 1998 と、GOV.UK の手引きから転記しました。
- [examples/media_mail](examples/media_mail)：カートの重さ（品物の重さに数量を掛けて足したもの）と、どの品物もメディアかを、Media Mail の料金の規則に渡す例です。規則は、Media Mail にならない小包には none を答えます。米国郵便公社の料金表 Notice 123 から転記しました。
- [examples/discount_split](examples/discount_split)：請求書の値引きを、明細の金額の比で配る例です。`allocate` を使う規則を明細ごとに一回ずつ呼び、途中までの合計を `running_totals` で渡すので、配った分の合計がちょうど値引きになります。バージニア州の値引きの規則 23VAC10-210-250 から転記しました。

## どう確かめているか

`cargo test -p kaname` が、言語を例とテストのモジュールに当て、生成したコードを走らせます。

- 丸めは rulec の丸めと 360,600 通りの値と刻みで、日付は ritsu-ports、koyomi、rulec と 3,652,059 日のすべてで、同じ答えになることを確かめます。
- rulec のコーパスの `derive` と `define` の全部（85 のファイルの 143 行）を kaname の式として読み、rulec と同じ型になり、rulec のベクタ 20,033 件で同じ値になることを確かめます。
- 範囲の解析を参照インタプリタと突き合わせます。例とテストのモジュールのすべての関数で、ベクタと、引数の宣言から選んだ 200 件の入力を流し（19,054 回の評価）、361 の式に来た値 489,656 個が、どれも解析の言う範囲に入り、失敗した 1,450 回が、どれも関数のエフェクトの言う種類で失敗することを確かめます。
- 評価と範囲の解析を Lean で書き直し（ritsu の一つの Lean のパッケージの `proofs/KanameCore`）、範囲の解析が健全であることと、total と推論した関数が失敗しないことを証明しています。`cargo test -p ritsu-model` が、30 のモジュールの 723 の評価で、モデルを参照インタプリタと範囲の解析に突き合わせます。
- TypeScript、Python、Go に生成したコードが、22 のモジュールのすべてのベクタと、引数の宣言の外の入力に、参照インタプリタと同じ答えを返すことを、英語と日本語の両方で確かめます（996 件のすべてが一致）。生成したコードは `tsc --strict --erasableSyntaxOnly`、`mypy --strict`、gofmt と `go vet` を通ります。
- どの診断のコードにも、それを出す一番小さなモジュールが英語と日本語の両方であります。`doc` のページは golden で、中の規則と日付のページは rulec と koyomi が描いたものです。HTML は Chrome で開いて確かめます。
- この README と `docs/` とスキルに載せた出力、診断、`.kn` の行は本物です。`tests/docs.rs` が突き合わせ、コマンドは ritsu の `tests/kaname.rs` が走らせます。

## 状態

言語、`check`、`run`、`vectors`、`doc`、`api`、`explain`、`ritsu gen` が書くコードはできています（[DESIGN.md](DESIGN.md)）。
まだ作っていないもの：dandori のフローから kaname の関数を呼ぶこと、koyomi のカレンダーと日付の時刻、渡す入力に合わせて規則の出力の範囲を細かくすること（DESIGN の 19 章）。

## ライセンス

[Apache License, Version 2.0](../../LICENSE-APACHE) と [MIT license](../../LICENSE-MIT) のどちらかを選んで使えます。
