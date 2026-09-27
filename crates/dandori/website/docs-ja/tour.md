# ワークフローを書く

`.flow` のファイル一つが、ワークフロー一つです。ファイルには、受け取るものと返すもの、使う規則と API、呼び出すタスク、状態を追う案件（あとで説明します）、処理の流れを書きます。このページでは、Temporal 向けに書いたホテルの予約の例
[examples/hotel/temporal/hotel.flow](https://github.com/i2y/dandori/blob/main/examples/hotel/temporal/hotel.flow)
を順に読みます。宿泊の予約を受けたらカードに与信を取り、チェックアウトの日に支払いを確定するワークフローです。

## 宣言

```flow
workflow hotel_stay v1
description "When a stay is booked, hold an amount on the card, and capture it on the day of check-out. …"

use rule hold from "../rules/hold_amount.rule"
use rule payment_intent from "../rules/payment_intent.rule"
use openapi stripe from "../specs/stripe.json"

record Booking
  id        : string
  room      : hold.room
  nights    : int  range >=1 <=30
  card      : string
  check_out : timestamp

enum Outcome = stayed | awaiting_review

inputs
  booking : Booking

outputs
  outcome : Outcome
```

最初の行で、ワークフローの名前と版を決めます（`workflow hotel_stay v1`）。Temporal では、版がワークフローの型とタスクキューの名前（`hotel_stay_v1`）に入ります。そのため新しい版は古い版と並んで動き、走っている実行のコードが入れ替わることはありません。

`use rule` は [rulec](https://github.com/i2y/rulec) で書いた規則を読みます。`hold` は与信の額と、フロントが先に見るかどうかを決める規則です。もう一つの `payment_intent` には、Stripe の PaymentIntent を rulec のステートマシンとして書き写してあります。規則の列挙とレコードは、ここでそのまま型として使えます（`hold.room`）。

`use openapi` は API の記述を読みます。その API を呼ぶタスクは、記述と合っているかを検査されます（[API の記述](tasks.md#api-の記述)）。`inputs` と `outputs` は、一回の実行が受け取るものと返すものです。

型は `int`、rulec と同じ単位の付いた数（`money[JPY, incl_tax]` など）、`string`、`bool`、`timestamp`、列挙、レコード、`list[T]`、無いことがある値を表す `T?`、中を見ずにそのまま渡す値の `json` です。数には、取りうる[範囲](#範囲)も書けます。

## タスク

タスクは一回の呼び出しです。何を受け取って何を答えるか、どう呼ぶか、案件に何をするか、どんなエラーで返ってくるか、どうやり直すかを書きます。

```flow
task create_intent(amount: money[JPY, incl_tax] range >=50 <=99999999, currency: string, payment_method: string, capture_method: payment_intent.capture_method) -> PaymentIntent
  http POST stripe "/v1/payment_intents"
  starts payment_intent.payment then attach
  key
  retry 2 times every 2 seconds

task confirm_intent(intent: string) -> PaymentIntent
  http POST stripe "/v1/payment_intents/{intent}/confirm"
  sends confirm
  errors card_declined = 402, unexpected_state = 400
  refused as unexpected_state
  key

task get_intent(intent: string) -> PaymentIntent
  http GET stripe "/v1/payment_intents/{intent}"
  observes
  idempotent
  retry 3 times every 2 seconds
```

- **呼び方**：`http`、`lambda`、`aws`、`connect`、エージェント、ほかの `.flow`、自分で書くコードのどれかです。呼び方の全部と、それぞれが各プラットフォームで何になるかは[タスクが呼ぶもの](tasks.md)にまとめています。
- **案件に何をするか**：`starts` は、そのステートマシンに従う案件を始めます（この例では、続けて出来事 `attach` が起きます）。`sends` は案件に出来事を送り、`observes` は案件を読むだけです。検査は、ステートマシンの表に沿って案件の状態の変わり方を追います。
- **エラー**：宣言するエラーには名前を付け、HTTP の呼び出しなら返ってくるステータスも書きます（`card_declined = 402`）。`refused as unexpected_state` には、ステートマシンが出来事を断ったときに返ってくるエラーを書きます。
- **やり直しとキー**：外部のデータを変えるタスクをやり直すなら、冪等キー（`key`）が要ります。キーは、実行と呼び出しの場所から dandori が作ります。何も変えないタスクには `idempotent` と書きます。`retry 2 times every 2 seconds` は失敗とタイムアウトをやり直しますが、タスクが宣言したエラーはやり直しません。`retry … on busy` は、名前を挙げたものだけをやり直します。
- **期限と待ち**：`timeout 2 days` は一回の呼び出しの期限です。`callback` を付けたタスクは、あとから届く答え（承認や、梱包の担当者からの知らせ）を待ちます。

## 案件

Stripe の PaymentIntent や倉庫のシステムの注文のように、外部のサービスの中にあって、ワークフローが呼び出すたびに状態が変わっていくものを、dandori では案件と呼びます。案件の状態の変わり方は、rulec で書いたステートマシンに従います。

```flow
case pi : PaymentIntent follows payment_intent.payment
  held capture_method = manual
  held confirmation_method = automatic
  external authenticate, settle, expire
  refused when refused = true
```

`held` には、ステートマシンの表が読む値のうち、この案件では決まっているものを書きます。このワークフローは支払いをいつも手で確定するので、`manual` の行だけが当てはまります。ほかの値は外部のサービスが決めるので、検査はそのすべてを試します。

ワークフローが何もしなくても外部のサービスの側で起きる出来事は、`external` に並べます。お客さんの本人認証、銀行の処理の結果、与信の期限切れがこれに当たります。検査は、案件に次のタスクを呼ぶ前と、ワークフローが終わるときに、これらが起きた場合も考えに入れます。ステートマシンのどの出力が出来事の断りを表すかは、`refused when refused = true` のように書きます。

ワークフローが `succeed` や `fail` や flow の終わりで終わるとき、始めた案件はどれも、ステートマシンの終わりの状態にいなければなりません。そうでない状態に残す実行があれば、検査はその実行を示します（E020）。終わっていない案件を承知のうえで引き渡すときは、`fail … leaving pi` と書きます。

## 流れ

```flow
flow
  let quote = hold(room: booking.room, nights: booking.nights)
  match quote.handling
    review => succeed outcome = awaiting_review
    auto => pi <- create_intent(amount: quote.amount, currency: "jpy", payment_method: booking.card, capture_method: manual)
  pi <- confirm_intent(intent: pi.id)
    on card_declined => pi <- get_intent(intent: pi.id)
  match pi.status
    requires_action =>
      pi <- customer_authenticated()
        on timeout => pi <- get_intent(intent: pi.id)
    requires_capture, requires_payment_method, requires_confirmation, canceled => pass
  match pi.status
    requires_capture => wait until booking.check_out
    requires_payment_method, requires_confirmation, requires_action =>
      pi <- cancel_intent(intent: pi.id)
        on unexpected_state => pass
      fail CardDeclined "The card could not be held"
    canceled => fail PaymentCanceled "The PaymentIntent was canceled"
```

`let x = task(…)` はタスクか規則を呼び、答えを取っておきます。`pi <- task(…)` は案件 `pi` に対してタスクを呼び、返ってきた案件を受け取ります。

分岐は `match` だけで、列挙・bool・無いことがある値（`none`、`some x`）で分かれます。どの値にも行き先が要り、残りをまとめて引き受ける行き先はありません。行き先の無い値は検査が断ります（E010）。実行中にそういう値が来たら、最後の行き先に流れ込むことはなく、`Dandori.UnexpectedValue` で失敗します。`.flow` には比較も計算も無いので、条件は規則に書くか、タスク（API、エージェント、自分で書くコード）に答えてもらいます。

`on <エラー> =>` はすぐ上の呼び出しが宣言したエラーを受けます。`on failure =>` はどんな失敗でも受け、`on timeout =>` はタイムアウトを受けます。`wait 1 hour` と `wait until booking.check_out` は待ちます。`succeed outcome = …` は出力を返して実行を終え、`fail Name "理由"` は失敗で終えます。

ループには必ず回数の上限を書きます。そのため、一回の実行履歴の大きさにも上限が決まります。

```flow
      repeat at most 12 times
        wait 1 hour
        pi <- get_intent(intent: pi.id)
        match pi.status
          processing => pass
          succeeded, requires_payment_method => break
```

`for line in order.lines at most 50` はリストを順にたどります。`for line in order.lines at most 50 in parallel, 10 at a time` と書くと、回を同時に走らせます。変数は回ごとに別で、ある回が失敗してもほかの回は最後まで走り、リストの順でいちばん前の失敗が結果を決めます。`let results = for …` の本体の最後の行に `yield r` と書けば、回ごとの値がリストに集まります。

## 失敗したとき、キャンセルされたとき

```flow
on failure
  match pi.status
    none, succeeded, canceled => pass
    requires_payment_method, requires_confirmation, requires_action, requires_capture =>
      pi <- cancel_intent(intent: pi.id)
        on unexpected_state => pass
        on failure => fail CleanupFailed "Releasing the hold failed; handing it over to staff" leaving pi
    processing => fail SettlementUnclear "Failed in the middle of the capture; handing it over to staff" leaving pi
```

タスクが失敗し、そのエラーをどこでも受けなかったときは、`on failure` が走って案件を片付けます。そのあと実行は同じエラーで失敗します。

キャンセルされたときに同じことをするのが `on cancel` です。Temporal では、ワークフローに止まるよう頼めます。頼まれたワークフローは後始末をしてから、キャンセルとして終わります。ほかのプラットフォームは実行をその場で止めてしまうので、`on cancel` を断ります（E050）。検査は、キャンセルが届きうるすべての呼び出しと待ちから `on cancel` に入り、そこでも案件が片付くかを見ます。

## 範囲

数には、取りうる範囲を書けます。書き方は rulec の規則の入力と同じです（`nights : int  range >=1 <=30`）。範囲は入力、出力、レコードのフィールド、タスクの引数と答え（`-> int  range >=0 <=10`）に付けられ、どちらかの端は省けます。値は JSON で運ばれるので、端は型の単位での整数とし、単位は付けません（`>=0JPY` ではなく `>=0`）。

- **入ってくる値は、動いているときに確かめます。** 入力や、タスクや規則の答えの数が範囲を外れていれば、どのプラットフォームでも `Dandori.BadInput` か `Dandori.BadResponse` で実行が失敗します。規則の答えも、rulec が示す範囲で確かめます。規則を動かす関数が、検査したときの版と同じとは限らないからです。
- **出ていく値は、動かす前に確かめます。** 規則の入力、タスクの引数、書き出すレコードのフィールド、出力に渡す値は、そこの範囲に収まっていなければなりません（E014）。範囲が何も書かれていない値を渡すと警告になります（W104）。

ホテルの予約の下書きで、泊数を規則が受け取るより長くしたもの
（[tests/fixtures/hotel_ranges.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/hotel_ranges.flow)）を `--lang ja` で検査すると、こうなります。

<div class="dd-term" markdown>

```text
エラー[E014]: tests/fixtures/hotel_ranges.flow:20:1: `booking.nights` は `>=1 <=60` で、規則 `hold` の `nights` の範囲 `>=1 <=30` を外れることがあります
    20 |   let quote = hold(room: booking.room, nights: booking.nights)
警告[W104]: tests/fixtures/hotel_ranges.flow:21:1: `extension` の範囲が分かりません（入力 `extension` に範囲がありません）。規則 `hold` の `nights` が受け取るのは `>=1 <=30` です
    21 |   let longer = hold(room: booking.room, nights: extension)
```

</div>

変数の範囲は、フローのどこかでその変数に入れるすべての値を合わせたものです。`.flow` には計算が無いので、範囲は値の出どころから行き先まで、そのまま運ばれます。Temporal では、範囲を足したり狭めたりすると、走っている実行のうち、値がその範囲の外にあるものの振る舞いが変わります。そのため、この変更は新しい版として出すか、Worker Deployment Versioning で出します。
