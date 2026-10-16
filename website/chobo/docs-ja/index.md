---
title: "勘定に下限と上限を書く。どの振替も、一度の書き込みの中でそれを守る。"
hide:
  - navigation
  - toc
---

<div class="cb-hero" markdown>
<img class="cb-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="cb-hero__mark" src="images/mark-light.svg#only-light" alt="">

# chobo

<p class="cb-hero__tag">勘定に下限と上限を書く。どの振替も、一度の書き込みの中でそれを守る。</p>

<p class="cb-hero__lede">
<strong>数で持っていて、勘定から勘定へ動かすものを書く小さな言語です。</strong>在庫、お金、ポイント、予約の枠などがそうです。帳簿には、勘定と、勘定ごとの下限や上限と、勘定のあいだで動かす振替の種類を書きます。振替には、同じ呼び出しを一度しか通さないためのキーと、確定か取消を待つ仮押さえも書けます。
</p>

<p class="cb-hero__lede">
<strong>書ける条件は、一つの勘定の境界だけです。</strong>境界が見るのはその勘定だけなので、勘定を書き換える書き込みの中で確かめられ、あいだにほかの読み取りが入りません。「返金は売上を超えない」のように二つの勘定にまたがる条件は、勘定を一つ足して、その勘定の境界として書きます。
</p>

<p class="cb-hero__lede">
<strong>データベースが二つでも、帳簿の意味は一つです。</strong>chobo は帳簿を検査し、参照インタプリタで動かし、PostgreSQL か TigerBeetle 向けにビルドします。呼び出すクライアントは TypeScript、Python、Go で出します。テストはどのシナリオも七つの出力先で流し、どの出力先も、参照インタプリタと同じ結果を返さなければなりません。
</p>

<div class="cb-hero__cta" markdown>
[ブラウザで試す](../../ja/playground/#project=chobo/refunds.ja){ .md-button .md-button--primary }
[インストール](install.md){ .md-button }
[帳簿を書く](tour.md){ .md-button }
[例で見る](examples.md){ .md-button }
[GitHub](https://github.com/i2y/ritsu/tree/main/crates/chobo){ .md-button }
</div>
</div>

<div class="cb-chart" markdown>
![chobo doc が描いた返金の帳簿。売上計上は、帳簿の外の勘定「売上」から「返金できる残り(注文)」へ額を入れる。返金できる残りは 0 以上で、下回る振替は「返金超過」で拒否される。返金は先に押さえる振替なので破線の矢印で、返金できる残りから、帳簿の外の勘定「返金済み」へ動かす](images/chart-ja.svg#only-dark)
![chobo doc が描いた返金の帳簿。売上計上は、帳簿の外の勘定「売上」から「返金できる残り(注文)」へ額を入れる。返金できる残りは 0 以上で、下回る振替は「返金超過」で拒否される。返金は先に押さえる振替なので破線の矢印で、返金できる残りから、帳簿の外の勘定「返金済み」へ動かす](images/chart-ja-light.svg#only-light)
</div>

この図は、下の帳簿を `chobo doc` が描いたものです。四角が勘定、矢印が振替です。破線で角の丸い四角は帳簿の外の勘定で、境界はありません。破線の矢印は先に押さえる振替で、確定したときに動きます。

---

## chobo がすること

<div class="cb-row" markdown>
<div markdown>

```book
book 返金 v1
description "注文ごとに、返金できる残りを勘定にする。売上で増え、返金で減り、0 より下にはならない。返金は承認を待つあいだ押さえる"

unit 円

account 返金できる残り(注文: string) : 円
  description "その注文でまだ返金できる額"
  at least 0 refused as 返金超過
account 売上 : 円 outside
account 返金済み : 円 outside

transfer 売上計上(注文: string, 額: 円)
  key 注文
  move 額 from 売上 to 返金できる残り(注文)

transfer 返金(申請: string, 注文: string, 額: 円)
  description "承認を待つあいだ押さえる。承認されたら確定し、却下されたら取り消す"
  key 申請
  pending expires after 7 days
  move 額 from 返金できる残り(注文) to 返金済み
```

</div>
<div markdown>

### 二つの勘定にまたがる条件を、一つの勘定の境界にする

「返金は売上を超えない」は、二つの額を比べる条件です。二つの返金が同時に来ると、どちらも、相手が書き込む前の売上を読めてしまいます。そこでこの帳簿は、注文ごとに、返金できる残りを勘定にしています。売上で増え、返金で減り、0 を下回りません。こうすると二つの返金は同じ勘定を書き換えることになり、あとの返金は、先の返金が残した額で確かめられます。[帳簿を書く](tour.md)

</div>
</div>

<div class="cb-row cb-row--flip" markdown>
<div markdown>

```console
$ chobo check examples/refunds/refunds.ja.book --lang ja
examples/refunds/refunds.ja.book: ok
  売上計上.do  拒否されうる理由: key_conflict
  返金.hold    拒否されうる理由: 返金超過（返金できる残り(注文) at least 0）、key_conflict、already_refused
  返金.post    拒否されうる理由: key_conflict、already_voided、expired、over_hold、no_such_hold
  返金.void    拒否されうる理由: already_posted、expired、no_such_hold
…
```

</div>
<div markdown>

### 拒否されうる理由を、動かす前に全部並べる

`chobo check` は、振替の操作ごとに、拒否されうる理由を並べます。境界に付けた理由のほかに、chobo が決めている理由もあります。同じキーで引数の違う二度目の呼び出し、取り消したあとの確定、期限の切れた仮押さえの確定などを拒否するときの理由です。どの理由も、そこに至る操作の列を検査が見つけ、参照インタプリタで実際に流して確かめたものです。[何を検査するか](checks.md)

</div>
</div>

<div class="cb-row" markdown>
<div markdown>

```console
$ chobo run examples/refunds/refunds.ja.book --scenario examples/refunds/refunds.ja.more.json --lang ja
examples/refunds/refunds.ja.book (more: 30000 円の売上に 20000 円の返金が二つ同時に来て、そのあと 10000 円を返金する): 3 ステップ、とりうる結果は 2 通り
結果 1:
    1  売上計上.do(注文: A-100, 額: 30000)                           通る
    2  together
         呼び出し元 1: 返金.hold(申請: R-1, 注文: A-100, 額: 20000)  通る
         呼び出し元 2: 返金.hold(申請: R-2, 注文: A-100, 額: 20000)  返金超過 で拒否される
    3  返金.hold(申請: R-3, 注文: A-100, 額: 10000)                  通る
…
```

</div>
<div markdown>

### 同じ瞬間に来る二つの呼び出し

30000 円の売上に、20000 円の返金が二つ同時に来ます。参照インタプリタは、二つを処理する順序をすべて試し、とりうる結果を全部出します。先に処理された方が押さえられ、もう一方は拒否されます。どちらのデータベースも、このどちらかの結果を返さなければならず、テストがそれを確かめています。[シナリオを流す](scenarios.md)

</div>
</div>

<div class="cb-row cb-row--flip" markdown>
<div markdown>

```console
$ chobo build examples/refunds/refunds.ja.book --target postgres --out db
$ chobo build examples/refunds/refunds.ja.book --target postgres-typescript --out db
$ chobo build examples/refunds/refunds.ja.book --target tigerbeetle-python --out db
$ chobo build examples/refunds/refunds.ja.book --target tigerbeetle-go --out db
```

</div>
<div markdown>

### 一つの帳簿から、二つのデータベースと三つの言語へ

PostgreSQL では、操作ごとに関数を一つ作ります。関数は勘定の行をロックし、移動を全部確かめてから、一つのトランザクションの中で書き込みます。呼ぶ側がトランザクションを開いていれば、そのトランザクションの中で動きます。TigerBeetle では、クライアントが操作ごとに振替のチェーンを一つ送り、TigerBeetle はそれを全部通すか、一つも通さないかのどちらかにします。二つのクライアントは名前も引数も結果も同じで、拒否は例外ではなく、結果の一つとして返ります。[データベース向けのビルド](databases.md)

</div>
</div>

<div class="cb-row" markdown>
<div markdown>

```console
$ chobo doc examples/refunds/refunds.ja.book --lang ja --out pages
$ chobo doc examples/refunds/refunds.ja.book --format html --lang ja --out pages
```

</div>
<div markdown>

### 経理や運用の人が読むページ

`chobo doc` は、帳簿を経理や運用の人が読むページにします。載るのは、勘定と境界、上の図、振替ごとの拒否されうる理由とそのわけ、仮押さえのライフサイクル、それにシナリオごとの、ステップのたびの残高です。GitHub が図を描く Markdown か、シナリオを一ステップずつ進められる一枚の HTML で出します。[返金のページ](doc/refunds.html) · [人が読むページ](pages.md)

</div>
</div>

<div class="cb-row cb-row--flip" markdown>
<div markdown>

```flow
use book 在庫 from "books/在庫.book"
…
task 引き当てる(注文: string, sku: string, 数: int) -> 在庫.引当
  book 在庫.引当.hold
  starts 在庫.引当
  errors 在庫切れ
…
case 押さえ : 在庫.引当 follows 在庫.引当
```

</div>
<div markdown>

### ワークフローから呼び、ゲートで守る

dandori のワークフローは、帳簿の操作をタスクとして呼び、拒否されうる理由をタスクのエラーとして宣言し、仮押さえを案件として追います。dandori の検査は、確定の前に仮押さえの期限が切れる場合も考えに入れます。sekisho のゲートは、操作ごとに、だれが呼んでよいかを決めます。yuen の要件には、それを満たすものとして操作を書けます。rulec の規則が計算した額を振替の額にすることもでき、そのときは、その額で拒否されうる理由をタスクが全部処理しているかを `ritsu check` が確かめます。[ほかの言語と組み合わせる](together.md)

</div>
</div>

## いまの状態

まだ始めたばかりです。次のことは、まだできません。

- 残高のある勘定の種類について、境界を変えること。いまは、新しい勘定の種類を作り、残高を移す振替を書きます。
- PostgreSQL と TigerBeetle のほかのデータベース。
- TigerBeetle を本番の構成（レプリカ六つ）で動かすこと（テストは `--development` で立てたレプリカ一つで流します）と、キーから作る ID でどれだけ遅くなるかの測定。
- 勘定が受けた記録や振替を読む操作。
- TigerBeetle の結果が参照インタプリタと違う二つの場合を、そろえること（[出力先](targets.md#tigerbeetle)に書いてあります）。

設計の理由、決めたこと、残っていることは [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/chobo/DESIGN.md) にあり、主な決めごとは[設計](design.md)にまとめています。chobo は [ritsu](../../ja/) の言語の一つで、ライセンスは Apache License 2.0 と MIT ライセンスのどちらかを選べます。

## 次に読むページ

| ページ | 書いてあること |
|---|---|
| [帳簿を書く](tour.md) | マーケットプレイスの例を通して読む言語。単位、勘定と境界、振替、キー、仮押さえ |
| [何を検査するか](checks.md) | 操作ごとの拒否されうる理由、診断、データベースにある残高や仮押さえと合わなくなる変更 |
| [シナリオを流す](scenarios.md) | 参照インタプリタ、同時に来る呼び出し元、chobo が作るシナリオ |
| [データベース向けのビルド](databases.md) | PostgreSQL と TigerBeetle に何を書くか、クライアントの呼び方 |
| [お金と単位](units.md) | 単位、税込と税抜のお金、ほかの言語から見た単位 |
| [人が読むページ](pages.md) | `chobo doc` と、四つの例を描いたページ |
| [ほかの言語と組み合わせる](together.md) | 帳簿のまわりのワークフロー、ゲート、要件、規則 |
| [例で見る](examples.md) | 在庫、ポイント、返金、マーケットプレイスの四つ（英語の版と日本語の版） |
| [エージェント向け](agents.md) | エージェントのスキル |
| [どうやって確かめているか](assurance.md) | 一つの参照インタプリタと、七つの出力先の突き合わせ |
| [設計](design.md) | 言語の決めごとと、その理由 |
