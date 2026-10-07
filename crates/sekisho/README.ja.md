# sekisho

だれが何をしてよいかを書く小さな言語です。
principal、役割、属性、関係に加えて、業務の規則と日付の答えを条件に使えます。
書いた範囲のすべての組み合わせで確かめてから、[Cedar](https://www.cedarpolicy.com/) のポリシーを生成します。
sekisho は [ritsu](../../README.ja.md) の八つの言語の一つで、rulec の規則、koyomi の日付とカレンダー、dandori のワークフローを ritsu を通して読みます。サービスが OpenAPI、AsyncAPI、`.proto` に書いた契約も読みます。

認可のポリシーは、全体を通して読める人がいないことがよくあります。
どの役割が何をできるかはポリシーのあちこちに散らばり、「返金の期間のあいだ」「係の上限まで」のような条件は、サービスのどこかで計算されて、事実としてリクエストに入ってきます。
sekisho では、サービスの一つの部分のポリシーを一つの `.gate` に書き、守る操作の契約の隣に置きます。条件は、それを決める言語から受け取ります。

```gate
action 返金する(refund_order)
  description "注文の全部か一部を返金する"
  guards 注文 refundOrder
  principal 職員, Workflow
  resource 注文 from orderId
  input
    金額(amount) : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  context
    返金の区分(refund_band)  = 返金の上限(金額: 金額, 上限: principal.返金できる額).区分
    期間内(in_period)        = today <= 返金の期限.最終日(支払日: resource.支払日)
    営業日(business_day)     = today is open in 英国
…
permit 係は上限まで返金できる(clerks_refund_within_their_limit)
  description "係は、返金の期間のあいだ、自分の上限まで返金できる"
  principal in 係
  action 返金する
  when 返金の区分 is 上限まで
  when 期間内
…
forbid 停止中の職員は何もできない(suspended_staff_do_nothing)
  description "停止中の職員は何もできない"
  principal is 職員
  action any
  when principal.停止中
…
expect deny 係は上限を超えて返金しない
  description "責任者でない係は、自分の上限を超えて返金することがない"
  principal in 係
  action 返金する
  unless principal in 責任者
  when 返金の区分 is 上限超え
```

キーワードは英語で、役割や型や action やポリシーの名前は日本語で書けます。
Cedar に出る名前には、ASCII の別名（`refund_order`）を添えます。
`返金の区分` は rulec の規則が、`期間内` と `営業日` は koyomi の日付のファイルとイングランドとウェールズの祝日が答える値です。
`sekisho check` は、金額の区間ごとに返金の区分がどの値になりうるかを rulec に、どの答えが同じ日にそろいうるかを koyomi に尋ね、起こりうる役割、属性、金額、答えの組み合わせを全部たどります。

```console
$ sekisho check examples/refunds/refunds.ja.gate --root examples/refunds --lang ja
examples/refunds/refunds.ja.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1
```

三つの action の組み合わせは 1,078 通りで、どれも Cedar と同じ決まりで答えを決めます。どのポリシーも許さないリクエストは拒み、当てはまる forbid が一つでもあれば、permit があっても拒みます。
三つの期待は選んだ組み合わせのすべてで成り立ち、職務の分離も成り立ち、どの役割も `can` の行に書いた action だけを許されています。

## 検査が言うこと

係に向けて書いた forbid が、責任者にも当たってしまうことがあります。責任者は係を含むからです。
Cedar はこれを止めないので、責任者の permit は黙って何も許さなくなります。

```text
エラー[E302]: tests/mutants/E302_forbidに覆われるpermit.gate:94:1: permit `責任者は期間内なら返金できる` は何も許しません。許すはずの組み合わせを、どれも forbid が拒みます
    94 | permit 責任者は期間内なら返金できる(managers_refund_in_period)
  = 許すはずの組み合わせは 256 通りで、`係は返金しない` が 256 通り、`監査は返金しない` が 128 通り、`停止中の職員は何もできない` が 128 通り、`二度は返金しない` が 64 通りを拒みます。
  = 例：責任者を持つ職員（停止中：いいえ）、注文（状態：支払済）、金額：1GBP〜50GBP、返金の区分：上限まで、期間内：はい、営業日：いいえ。`係は返金しない` が拒みます。
  = forbid を狭めるか、だれにも許さないつもりなら permit を消してください。
```

期待は、人が決めたことをポリシーの隣に書いたものです。
責任者は期間内に返金できる、という期待から二行を外すと成り立たなくなり、検査は成り立たない組み合わせを一つ示します。

```text
エラー[E304]: tests/mutants/E304_成り立たない期待.gate:147:1: 期待 `責任者は期間内に返金できる` が成り立ちません。選んだ 192 通りのうち 144 通りが拒まれます
   147 | expect allow 責任者は期間内に返金できる
  = 例：責任者を持つ職員（停止中：はい）、注文（状態：支払済）、金額：1GBP〜50GBP、返金の区分：上限まで、期間内：はい、営業日：いいえ。`停止中の職員は何もできない` が拒みます。
  = ポリシーを直すか、期待が言いすぎているなら期待を直してください。
```

ほかにも、次のものを見つけます。

- どの permit も許さない action
- 条件が同時には成り立たないポリシー
- ほかの permit だけで足りている permit
- `separate` で分けた二つの職務を、一人が両方許されること
- 役割が、`can` の行より多く、または少なく許されること
- 契約に無い操作と、操作が受け取らない入力
- 規則が受け取る範囲を超える値と、規則の前提を破る値
- ゲートに問い合わせうる日のデータを持たないカレンダー

どの検査も、成り立つ、成り立たない組み合わせがある、決められない（理由つき）の三つのどれかを答えます。
コードの一覧は [docs/codes.ja.md](docs/codes.ja.md) にあり、いつ出るか、どう直すか、それを出す最小のファイルが書いてあります（`sekisho explain E302 --lang ja`）。

## Cedar と、Cedar に尋ねるコード

検査を通ったゲートからだけ生成します。
`gen --target cedar` は、Cedar のスキーマとポリシーを、テキストと JSON の両方で書きます。

```console
$ sekisho gen examples/refunds/refunds.ja.gate --target cedar --root examples/refunds --out generated --lang ja
生成しました: generated/cedar/refunds_ja.cedar
生成しました: generated/cedar/refunds_ja.cedarschema
生成しました: generated/cedar/refunds_ja.cedarschema.json
生成しました: generated/cedar/refunds_ja.policies.json
```

```cedar
@id("refunds_ja/clerks_refund_within_their_limit")
@name("係は上限まで返金できる")
@doc("係は、返金の期間のあいだ、自分の上限まで返金できる")
permit (
  principal in ShopJa::Role::"clerk",
  action == ShopJa::Action::"refund_order",
  resource is ShopJa::Order
)
when { context has refund_band && context.refund_band == "within_limit" }
when { context.in_period };
```

Cedar は、リクエストに書いてある値をそのまま信じます。
呼ぶ側が `refund_band` を送れるなら、自分の返金を自分で許せてしまいます。
そこで `gen --target typescript`、`python`、`go` は、action ごとに関数を書きます。関数は、principal と注文をサービス自身のデータから読み、返金の区分と日付を rulec と koyomi が生成したコードで計算してリクエストを組み立て、Cedar に尋ねます。尋ねる先は、プロセスの中の cedar-wasm、cedarpy、cedar-go か、`--authorizer avp` なら Amazon Verified Permissions です。
関数は、計算した値を呼ぶ側から受け取りません。データがゲートの宣言と合わないときや、ゲートを確かめた範囲の外の日には、Cedar に尋ねる前に拒みます。
`ritsu gen` は、同じものをプロジェクトのパッケージに、読む規則と日付のコードと並べて書きます。

## 言語をまたいで確かめること

`ritsu check` は、プロジェクトのゲートをほかの言語と一緒に読み、言語をまたぐ確かめを二つ足します。

- sakai の地図のコンテキストが公開する操作には、どれにもそれを守る action があること（契約がだれでも呼べると書いた操作を除く。ritsu の E907 と W907）
- ワークフローが、フローが呼ぶ操作を許されていて、拒まれうる呼び出しでは拒まれたときの手当てがあり、呼ばない操作を許されていないこと（E908、W909、W908）

例では、ワークフロー `返品` が `refundOrder` を呼びます。ゲートは返品済の注文を 50 ポンドまで返金することをこのワークフローに許していて、タスクは拒まれたときのエラーを宣言しています。

```console
$ ritsu check examples/refunds --lang ja
ok examples/refunds/rules/refund_limit.rule
ok examples/refunds/rules/返金の上限.rule
examples/refunds/calendars/england_and_wales.cal: ok — カレンダー england_and_wales。表 bank_holidays 83 行、データの範囲 2019-01-01〜2028-12-31
examples/refunds/dates/refund_terms.cal: ok — 3 つの条件が、paid_on 2026-01-01〜2028-11-25 の 1,060 日のすべてで成り立ちます。例 2 行も合っています
examples/refunds/dates/返金の期限.cal: ok — 3 つの条件が、支払日 2026-01-01〜2028-11-25 の 1,060 日のすべてで成り立ちます。例 2 行も合っています
examples/refunds/flows/returns.flow: 検査を通りました
examples/refunds/refunds.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1
examples/refunds/refunds.ja.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1
ritsu check: ファイル 8 個（rulec 2、koyomi 3、dandori 1、sekisho 2）。どれも検査を通りました。言語の境目: 確かめた 2 か所、決められない 0 か所
```

手で書いた Cedar のファイルも、地図か要件がそれを指し、スキーマの `@guards` にどの action がどの操作を守るかを書いてあれば、同じ確かめを受けます。

## 人が読むページ

`sekisho doc` は、だれが何をしてよいかを決める人のためのページを書きます。
action ごとに、許す組み合わせと拒む組み合わせの表と、それを決めたポリシーがあります。
ポリシーごとに、`.gate` の行と生成した Cedar を並べます。
計算した値には、rulec と koyomi が描いた規則と日付のページが付きます。
期待、職務の分離、役割、ワークフローとそのフローが呼ぶ操作、守る操作も載ります。
Markdown はプルリクエストに貼るためのもので、HTML は外のものを何も読まない一つのファイルです。
例のページは [tests/golden/doc/refunds.ja.ja.md](tests/golden/doc/refunds.ja.ja.md) にあります（規則と日付のページは除いてあります）。

## AI エージェント向けのスキル

[skills/sekisho](../../skills/sekisho/SKILL.md) は、ゲートを書いたり直したりするエージェントのためのスキルです。書く流れ、言語の一枚の要約、人に聞くこと、sekisho の診断と ritsu の言語をまたぐ診断の直し方があります。
`SKILL.md` のほかのページは `docs/` からコピーしたもので、`skills/sync.sh` がコピーします。

## インストール

sekisho は ritsu と一緒に入ります。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu sekisho …` で走らせます。
`sekisho` という名前のリンクから呼ぶと、ritsu は sekisho として動き、sekisho が読む言語は全部つながります。リリースのアーカイブにはリンクが入っています。手で作るなら、ritsu の隣で `ln -s ritsu sekisho` とします。
sekisho のクレートのバイナリはほかの言語を持たないので、規則、日付のファイル、カレンダー、帳簿、フローを読むゲートでは E209 で止まります。

## コマンド

```console
$ sekisho check examples/refunds/refunds.ja.gate --root examples/refunds --lang ja
$ sekisho gen examples/refunds/refunds.ja.gate --target python --root examples/refunds --lang ja
$ sekisho doc examples/refunds/refunds.ja.gate --root examples/refunds --format html --lang ja
$ sekisho vectors examples/refunds/refunds.ja.gate --root examples/refunds --action 返金する
$ sekisho api examples/refunds/refunds.ja.gate --root examples/refunds
$ sekisho explain E302 --lang ja
```

`--root` は、ゲートがほかのファイルを指すときのパスの起点です（無ければ、最初に渡したファイルから上にたどって `.git` のある一番近いディレクトリ）。sekisho、yuen、sakai が、同じ操作を同じ書き方で指せるようにするためです。
`vectors` は、すべての組み合わせを `cedar run-tests` のテストとして書き出します。
どのコマンドも `--lang ja|en`（無ければ `SEKISHO_LANG`、次に `RITSU_LANG`、どちらも無ければ英語）と `--help` を受け付けます。
終了コードは、エラーが無ければ 0、エラーがあれば 1、引数の誤り、読めないファイル、つながっていない言語のファイルなら 2 です。
言語とコマンドの全部は [docs/reference.md](docs/reference.md)（英語）にあります。

## 例

- [examples/refunds](examples/refunds)：店の注文を見ること、返金すること、返金の記録を書き出すことを、だれがしてよいかを書いた例です。係の上限を決める rulec の規則、返金の期間を決める koyomi の日付のファイル、イングランドとウェールズの祝日、操作を書いた OpenAPI の文書、返品された注文を返金する dandori のワークフローを読みます。
  `refunds.ja.gate` は、同じゲートを日本語の名前で書いたもので、日本語の版の規則と日付のファイルを読みます。

## どう確かめているか

`cargo test -p sekisho` が、言語を変異と例に当て、本物の Cedar を走らせます。

- 120 の変異（例か fixture を一か所だけ変えたもの）が出す診断を、英語と日本語の両方で golden と突き合わせます。日本語の名前のもの 60 には、それぞれ英語の名前の対があります。
- 例とテストの材料のうち検査を通るすべてのゲートから生成した Cedar を、公式の Cedar の CLI 4.13.0 にかけます。`cedar validate` の strict の検証が何も言わないこと、`cedar format --check` が通ること、`cedar run-tests` がすべての組み合わせを sekisho の参照の評価と同じ答え、同じ決めたポリシーで答えること、JSON の形が `cedar translate-schema` と `translate-policy` の出力と一字も違わないことを確かめます。
- 手で書いた Cedar に sekisho が返す答えも、公式の CLI の答えと同じです。ポリシーとスキーマの組（`tests/cedar_in/` に置いたものと、それを一か所ずつ変えたもの、検査を通るすべてのゲートから生成した Cedar を手で書いたものとして読ませたもの）について、sekisho が数えるすべての組み合わせをリクエストとエンティティにして `cedar run-tests` にかけ、許すか拒むかと決めたポリシーが同じになることを確かめます。sekisho が決められない問いでは、ポリシーのどの部分を数えないのかを理由に挙げます。
- TypeScript、Python、Go に生成したコードで、すべての組み合わせのリクエストを生の値から組み立て、cedar-wasm 4.13.0、cedarpy 4.12.1、cedar-go v1.8.0 に尋ね、参照の評価と同じ答えになることを確かめます。生成したコードは `tsc --strict`、`mypy --strict`、`go vet` と gofmt を通ります。
- `doc` のページは golden で、中の規則と日付のページは rulec と koyomi が描いたものです。HTML は Chrome で開いて確かめます。
- この README と `docs/` とスキルに載せた出力、診断、`.gate` と Cedar の行は本物です。`tests/docs.rs` が走らせて突き合わせ、上の `ritsu check` は ritsu の `tests/sekisho.rs` が走らせます。

## 状態

言語、`check`、Cedar と TypeScript と Python と Go の `gen`、`doc`、`vectors`、`api`、`explain` はできています（[DESIGN.md](DESIGN.md)）。
まだ作っていないもの：二段以上の関係、規則の数の出力を条件にすること、`--authorizer avp` のコードを本物の Verified Permissions で走らせること（いまは型の検査だけ）、手で書いた Cedar のワークフロー（DESIGN の 15 章）。

## ライセンス

[Apache License, Version 2.0](../../LICENSE-APACHE) と [MIT license](../../LICENSE-MIT) のどちらかを選んで使えます。
