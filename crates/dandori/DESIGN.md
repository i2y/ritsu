# dandori 設計文書

業務ルールを呼ぶワークフローのための、型の付いた小さな言語。書いたものを走らせる前に検査し、AWS Step Functions（ASL）、Temporal（TypeScript と Python）、AWS Lambda durable functions（TypeScript）、Argo Workflows（WorkflowTemplate の YAML）、pydantic-graph（Python）へコンパイルする。判断そのものは rulec の規則に書き、dandori はそれを rulec の CLI を通して読む。

名前は段取り（手順を前もって組むこと）から取った。ファイルの拡張子は `.flow`。

## 0. 全体像

```
.flow ── 構文解析 ── 名前と型の解決（rulec の schema・certificate・api を読む）── 型を付けた構文木
                                                                              ├── 流れに沿った検査（案件の状態、代入、網羅、出口）
                                                                              ├── 参照インタプリタ ── シナリオの自動生成
                                                                              ├── Step Functions（ASL、JSONata）と Lambda をつなぐコード
                                                                              ├── Temporal（TypeScript と Python）と、タスクと規則のアクティビティ
                                                                              ├── Lambda durable functions（TypeScript）と、規則を呼ぶ Lambda
                                                                              ├── Argo Workflows（YAML）と、タスクと規則をコンテナで動かすプログラム
                                                                              └── pydantic-graph（Python）の、文ごとの節点のグラフ
```

### 0.1 前提

- **P1**：判断は rulec に置く。dandori の式は値を組み立てられる（レコード、リスト、値を埋め込んだ文字列）が、比較・算術・論理演算を持たない。分岐は、列挙・bool・無いことがある値の `match` だけ。
- **P2**：rulec の中には入れない。dandori は rulec の CLI の出力（JSON）だけを読み、rulec は dandori を知らない。
- **P3**：意味を決めるのは参照インタプリタ一つ。五つの出力先は、どれもそれと突き合わせて確かめる。
- **P4**：ループは回数に上限を書く。再帰は無い。だから実行履歴の長さに上限が見積もれる。
- **P5**：静的に言えないこと（外の相手が何を返すか）は、境界で実行時に確かめる。確かめて外れたら、その場で失敗させる。
- **P6**：出力先によって意味が変わる機能は、その意味を出せる出力先でだけ作り、ほかの出力先では E050 で断る。キャンセルの後始末（`on cancel`）と、名前で送られてくる出来事（`event`）は、いまは Temporal だけが作れる。意味を変えず、呼び方や費用だけを変える項目（`queue`、`image`、規則の `local`）は、効かない出力先では何もしない。

P6 は、Temporal を主な出力先にするときに足した。それまでは、どの出力先でも書けることだけを言語に入れていた。Temporal でしか意味を持たない機能を言語から締め出すと、Temporal で書ける流れが狭くなる。かといって、ほかの出力先で黙って別の意味にすれば、P3 が崩れる。そこで、作れる出力先では参照インタプリタと同じ意味で作り、作れない出力先では断ることにした。参照インタプリタが決める意味は一つのままである。

P1 は、はじめ「式は変数・そのフィールド・リテラルだけ」としていた。条件と計算を rulec の表に置けば、網羅と重なりが rulec の証明書つきで閉じるからである。しかし、タスクに渡すレコードを組み立てる、通知の文面に注文番号を入れる、といった形を整えるだけの操作は何も判断しない。これまで禁じると、形を整えるためだけのタスクや規則が要る。そこで、分岐と計算にかかわるもの（比較・算術・論理演算）だけを rulec に残し、値の組み立ては許すことにした（6 章）。

### 0.2 rulec との分担

rulec は、ワークフローを動かすこと（段階そのもの、段階のあいだの待ち、外への副作用）を対象の外に置いている。rulec のステートマシンは状態を呼び出し側に持たせ、生成する関数は純関数のままである。dandori は、その外側に置いた半分を受け持つ。

**決定**：別の言語として、rulec の外に作る。

理由は二つ。一つ、待ちと外への副作用と再試行は、rulec の表の行にならない。rulec がステートマシンを別の言語にせず宣言一つで済ませられたのは、遷移を表の行として書けたからで、ここでは逆になる。二つ、rulec は小さいことで証明できる言語であり、ワークフローを入れればその小ささが崩れる。

**費用**：検査・生成・確かめ方を、もう一揃い作ることになる。判断の中身の検査（網羅、重なり、単位、丸め）は rulec に残るので、dandori の検査はワークフローの形と案件の状態に絞れる。

つなぎ目は rulec の既存の出口だけにした。

| rulec のコマンド | dandori が読むもの |
|---|---|
| `rulec schema` | 入力と出力の名前、JSON の型、範囲 |
| `rulec certificate` | 列挙、列の型、ステートマシンの表の軸と、各行が受け付ける座標、行が書く値、行き先、held |
| `rulec api` | ステートマシンの要約、前提、生成された Python と TypeScript の呼び方 |

rulec の実行ファイルは `DANDORI_RULEC`、無ければ PATH の `rulec` を使う。

## 1. 言語

### 1.1 ファイルの形

```
workflow hotel_stay v1
description "When a stay is booked, hold an amount on the card, and capture it on the day of check-out"

use rule hold from "rules/hold_amount.rule"
  lambda "arn:aws:lambda:ap-northeast-1:123456789012:function:hold-amount"
use rule payment_intent from "rules/payment_intent.rule"

record PaymentIntent
  id     : string
  status : payment_intent.status

task confirm_intent(id: string) -> PaymentIntent
  http POST "https://api.stripe.com/v1/payment_intents/{id}/confirm" form
  connection "arn:aws:events:…"
  sends confirm
  errors card_declined = 402, unexpected_state = 400
  refused as unexpected_state
  key

case pi : PaymentIntent follows payment_intent.payment
  held capture_method = manual
  held confirmation_method = automatic
  external authenticate, settle, expire
  refused when refused = true

flow
  let quote = hold(room: booking.room, nights: booking.nights)
  match quote.handling
    review => succeed outcome = awaiting_review
    auto => pi <- create_intent(…)
  …

on failure
  …
```

字下げがブロックを表す。コメントは `#` から行末まで。名前には日本語が使え、生成物でもそのまま識別子になる（JavaScript と Step Functions の変数は Unicode の識別子を受け付ける）。`examples` の例は英語で書き、`tests/flows` のフローと `tests/rules` の規則は日本語の名前で書いてある（5 章）。

### 1.2 型

| 型 | 値 |
|---|---|
| `int` | 整数。rulec の単位の無い `number` もこれになる |
| `money[円, incl_tax]` など | 単位の付いた整数。rulec の書き方のまま書き、単位が違えば渡せない。単位の種類は rulec の九つ（mass・length・area・volume・duration・temperature・sound・money・rate）で、それ以外は E002 |
| `string` | 文字列 |
| `bool` | 真偽 |
| `timestamp` | UTC の RFC 3339（`2026-10-01T10:00:00Z`）。Step Functions の Wait が受け付ける形に合わせた |
| 列挙 | `enum 結果 = 宿泊済 \| 確認待ち`、または規則の列挙 `payment_intent.status` |
| レコード | `record` で宣言する。規則の呼び出しは、その規則の出力のレコードを返す |
| `list[T]` | リスト。リストのリストは書けない（下の注） |
| `T?` | 無いことがある値。フィールドが無いときも null のときも「無い」として読む |
| `json` | どんな JSON でもよい値。中を見ずに受け渡すだけで、フィールドを読むことも `match` することもできない |

リテラルの整数は単位の付いた型にも渡せる。変数どうしは、単位が同じでなければ渡せない。`T` の値は `T?` のところに、どんな値も `json` のところに渡せる。`T?` の値を `T` のところに渡すには、先に `match` で `none` と `some x` に分ける。

リストのリストを書けないのは、JSONata の配列の組み立てが、中の配列を平らにしてしまうからである（`[$a, $b]` は、`$a` が配列なら要素が一段上に出る）。内側のリストはレコードに入れれば運べる。`json` の値は配列のこともあるので、それをリストに入れるところ（リストのリテラル、`yield`）では、生成する JSONata が `$type` で配列かどうかを見て包む。

### 1.3 範囲

数には、取りうる値の範囲を書ける。書けるところは、入力と出力、レコードのフィールド、タスクの引数と答えである。書き方は rulec と同じで、型のあとに `range` と端を書く。端は片方だけでもよい。

```
record Booking
  nights : int  range >=1 <=30

task create_intent(amount: money[JPY, incl_tax] range >=50 <=99999999, …) -> PaymentIntent
task count() -> int  range >=0 <=10
```

- 端は、型の単位で数えた整数を、単位を付けずに書く（`>=0円` ではなく `>=0`。単位を付けると E001）。JSON で運ぶ数そのものである。rulec は `range >=1g <=40kg` のように単位を付けて書き、違う単位を換算するが、dandori は値を JSON の数のまま運ぶので、換算しない。規則の入力と出力の範囲は、`rulec schema` の `minimum` と `maximum` から、同じ数え方で読む。
- `T?` や `list[T]` に書いた範囲は、中の数の範囲になる。
- 数でないものに書いた範囲と、数が一つも入らない範囲（`>=10 <=5`）は E003。

範囲は型ではなく、値を置くところ（フィールド、引数、入力）に付けた。rulec の列と同じ形で、型が等しいか、値を渡せるかの規則はそのままにできる。

範囲は二つのところで使う。

**入ってくる値は、走らせたときに確かめる（P5）。** ワークフローの入力、タスクの答え（コールバックと出来事を含む）、規則の答えの数が範囲の外なら、型の合わない値と同じく `Dandori.BadInput` か `Dandori.BadResponse` で失敗する。どの出力先でも、型を確かめる式に範囲の比べを足してある。レコードなら、フィールドの範囲も確かめる。規則の答えの範囲は rulec が出すもので、rulec はそれを証明している。それでも確かめるのは、呼ぶ Lambda や関数が、ワークフローを検査したときと別の版の規則に替わっていることがあるからである。エージェントの答えでは、OpenAI の Schema に `minimum` と `maximum` を書く。Claude の構造化出力は数の範囲を受け付けないので、Schema の説明（`description`）に `>=1 <=30` と書き、答えが来てから確かめる。

範囲を足したり狭めたりすると、Temporal で走っている実行のうち、値がその外にあるものは再生で失敗に変わる。だから、その変更は版を上げるか Worker Deployment Versioning で出す（5 章の再生）。

**出ていく値は、検査で確かめる。** 規則の入力、タスクの引数、書き出したレコードのフィールド、ワークフローの出力のように、範囲のあるところに値を渡すと、検査はその値の範囲を求め、収まるかを見る。

- 値の範囲は、リテラルならその数、入力・フィールド・タスクの答え・規則の出力なら、そこに書かれた範囲（規則なら rulec の出す範囲）である。`for` の変数は、回すリストの要素の範囲になり、`some x` の `x` は、分けた値の範囲になる。
- 変数の範囲は、その変数に入れるすべての値の範囲を合わせたもの。流れのどこで入れたかは区別しない。`match` の一方で 3、もう一方で 40 を入れた変数は、どこで読んでも `>=3 <=40` である。だから、実際には渡らない値のせいで E014 になることがある。そのときは変数を分ける。
- 収まらないことがあれば E014。範囲の分からない値（範囲を書いていない入力・フィールド・答え）が混じっていれば W104。W104 は、値の出どころに範囲を書けば消える。書いた範囲は、値が入ってくるところで走らせたときに確かめるので、そこから先はその範囲にあると言える。

dandori の式は算術を持たない（P1）ので、値の範囲は、出どころの範囲をそのまま運ぶだけで求まる。計算で範囲が広がることはない。

`tests/fixtures/ranges.flow` の検査の一部：

```
エラー[E014]: tests/fixtures/ranges.flow:35:1: `予約.泊数` は `>=1 <=60` で、規則 `与信` の `泊数` の範囲 `>=1 <=30` を外れることがあります
    35 |   let a = 与信(客室: 予約.客室, 泊数: 予約.泊数)
警告[W104]: tests/fixtures/ranges.flow:38:1: `延長` の範囲が分かりません（入力 `延長` に範囲がありません）。規則 `与信` の `泊数` が受け取るのは `>=1 <=30` です
    38 |   let c = 与信(客室: standard, 泊数: 延長)
```

### 1.4 宣言

- `use openapi|smithy|proto <名前> from "<パス>"`：呼ぶ API の記述を読む（1.10）。下に `url "<場所>"` を書くと、API の場所になる。
- `use rule <名前> from "<パス>"`：規則を読む。Step Functions と durable functions から呼ぶなら、下に `lambda "<関数>"` を書く。下に `local` を書くと、Temporal ではローカルアクティビティとして呼ぶ（4.2）。ほかの出力先では何も変わらない。
- `task <名前>(<引数>) [-> <型>]`：外の相手への呼び出し。`->` を書かないタスクは何も答えない（答えは読まない）。下に次の項目を書く。
  - 呼び方（1.6）：`lambda "<関数>"`、`http <メソッド> "<URL>" [form]` と `connection "<EventBridge の接続>"`、`aws <サービス>:<操作>`、`agent [openai | claude] "<指示>"` と `model "<モデル>"`（1.7。Step Functions では `connection` も）のどれか一つ。子ワークフローとして呼ぶなら、出力先ごとに `workflow "<型>"`（Temporal）、`state machine "<ARN>"`（Step Functions）、`durable function "<関数>"`（durable functions）、`workflow template "<名前>"`（Argo）。子も dandori で書いたなら `flow "<パス>"`（1.9）。Temporal では `queue "<タスクキュー>"` で送り先のキューを選べる。Argo で利用者のコンテナを動かすなら `image "<イメージ>"`。
  - `errors <名前> [= <HTTP ステータス> | = <例外の名前>], …`：業務のエラー。`http` のタスクではステータスを、`aws` のタスクでは API の例外（`ConditionalCheckFailedException`、書かなければエラーの名前そのもの）を書く。`timeout` と `failure`（それ以外の失敗のすべて）は宣言しなくても使える。
  - `retry <n> times [every <時間>] [backoff <k>] [on <エラー>, …]`：`on` を書かなければ、`failure` と `timeout` をやり直し、宣言したエラーはやり直さない。
  - `timeout <時間>`（書かなければ、出力先がそのタスクに許すだけ動く。4 章）、`key`（冪等キーを送る。`aws` のタスクでは `key ClientToken` のように、キーを受け取る API の引数を書く）、`idempotent`（二度しても一度と同じ）、`callback`（トークンや ID を渡して答えを待つ）。
  - 案件に何をするか：`starts <規則>.<ステートマシン> [then <出来事>, …]`、`sends <出来事>`、`observes`。`refused as <エラー>` は、ステートマシンが出来事を断ったときに返ってくるエラー。
- `case <名前> : <レコード> follows <規則>.<ステートマシン>`：rulec のステートマシンに従う案件。下に `held <入力> = <値>`、`external <出来事>, …`、`state <フィールド>`、`refused when <出力> = <値>` を書く。
- `inputs`・`outputs`：ワークフローの入力と出力。`T?` の出力は `succeed` で省ける。
- `flow`：本体。`on failure`：処理されなかったタスクのエラーのあとに走るブロック。`on cancel`：ワークフローがキャンセルされたときに走るブロック（2.6。Temporal だけ）。

### 1.5 文と式

| 文 | 意味 |
|---|---|
| `let x = t(a: …)` | タスクか規則を呼び、答えを `x` に入れる |
| `t(a: …)` | タスクを呼び、答えは読まない |
| `c <- t(a: …)` | 案件 `c` に対するタスクを呼び、答えを `c` に入れる |
| `let x[: T] = <値>` | 値を `x` に入れる。`{…}` のレコードには型を書く |
| `on <エラー>, … =>` | 呼び出しの下に書き、そのエラーを受ける。受けたあとは呼び出しの次へ進む |
| `match x.f` | 行き先 `<値>, … =>` を並べる。案件の状態で分けるときは、始まっていない案件の `none` も書ける。無いことがある値では `none =>` と、列挙や bool なら値を、それ以外なら `some y =>`（`y` にその値が入る）を書く |
| `wait <n> seconds\|minutes\|hours\|days`、`wait until <timestamp>` | 待つ |
| `repeat at most <n> times` | 上限つきのループ。`break` で抜ける |
| `[let r =] for x in <リスト> at most <n> [in parallel[, <k> at a time]]` | リストの要素ごとに本体を回す。要素が `n` を超えていれば `Dandori.TooManyItems` で失敗する。`let r =` を付けると、本体の最後の行の `yield <値>` を集めたリストが `r` に入る |
| `pass` | 何もしない |
| `succeed <出力> = <値>, …` | 成功で終わる |
| `fail <エラー> ["<理由>"] [leaving <案件>, …]` | 失敗で終わる。`leaving` は、終わっていない案件をそのまま引き渡すという宣言 |

値に書けるのは、変数とそのフィールド、文字列・数・`true`・`false`・列挙の値、`none`（無いことがある値のところだけ）、`{フィールド: 値, …}`（型は渡す先で決まる）、`[値, …]`、そして値を埋め込んだ文字列（`"注文 {受注.id} を出荷しました"`、波括弧そのものは `{{` と `}}`）である。文字列に埋め込めるのは文字列・数・bool・列挙・timestamp で、無いことがある値は先に `match` で分ける。`fail` の理由にも埋め込める。

変数の型は、値を入れるすべての場所から決まる。一つの名前は一つの型を持ち、`T` と `T?` の両方を入れる名前は `T?` になる。

### 1.6 タスクの呼び方

タスクを出力先がどう呼ぶかは、タスクに書いた呼び方で決まる。

| 呼び方 | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| `lambda` | Lambda の Task | dandori が書くアクティビティが関数を呼ぶ | dandori が書く step の中で関数を呼ぶ | dandori が書く実装をコンテナで動かす | dandori が書く関数が関数を呼ぶ |
| `http` | HTTP Task | dandori が書くアクティビティが `fetch` で送る | dandori が書く step の中で `fetch` で送る | 同じく、コンテナで `fetch` | 同じく、urllib で |
| `connect`（1.10） | HTTP Task（Connect の JSON） | dandori が書くアクティビティが `fetch` で送る | dandori が書く step の中で `fetch` で送る | 同じく、コンテナで `fetch` | 同じく、urllib で |
| `aws` | AWS SDK 統合 | dandori が書くアクティビティが AWS SDK で呼ぶ | dandori が書く step の中で AWS SDK で呼ぶ | 同じく、コンテナで AWS SDK | 同じく、boto3 で |
| `agent` | HTTP Task で OpenAI の Responses API を呼ぶ（Claude なら Messages API） | dandori が書くアクティビティが OpenAI の Agents SDK で呼ぶ（Claude なら Anthropic の SDK） | dandori が書く step の中で、同じ SDK で呼ぶ | 同じく、コンテナで | 同じく、Python の Agents SDK か Anthropic の SDK で |
| `state machine` | 入れ子の実行（`startExecution.sync:2`） | | | | |
| `flow`（1.9） | 入れ子の実行（`state machine` と） | 子の `<名前>_v<版>` の子ワークフローを、子のタスクキューで | invoke（`durable function` と） | 子の WorkflowTemplate から Workflow を作る | 利用者が書く関数 |
| `workflow` | | 子ワークフロー（`executeChild`） | | | |
| `durable function` | | | 別の durable function の invoke | | |
| `workflow template` | | | | その WorkflowTemplate から Workflow を作る | |
| `image` | | | | 利用者のイメージのコンテナ | |
| `event` | 作れない（E050） | 何も呼ばず、ワークフローに名前で送られてくる値を待つ（Update `dandori.event`） | 作れない（E050） | 作れない（E050、まだ） | 作れない（E050、まだ） |
| どれも無い | 作れない（E050） | 利用者が書くアクティビティ（`OwnTasks`） | 利用者が書く実装を step の中で呼ぶ（`OwnTasks`） | 作れない（E050） | 利用者が書く関数（`OwnTasks`） |

pydantic-graph では、ほかの出力先で子ワークフローとして呼ぶタスクも、利用者が書く関数になる。

Temporal、durable functions、Argo、pydantic-graph で dandori が書く実装は、Step Functions が送るのと同じものを送る。Lambda の payload、HTTP のメソッド・URL・ヘッダ・本文、AWS の API の引数は、五つの出力先で一字も違わない（5 章で突き合わせる）。送り方は `Transport`（`io.ts`、Python では `io.py`）を通すので、認証のヘッダ、AWS SDK のクライアントの設定、テストでの代わりは利用者が渡す。既定の `Transport` は `fetch` と AWS SDK for JavaScript v3 を使い、SDK は使うときに読み込む。既定で知っている AWS のサービスは 13 で、ほかのサービスは利用者の `Transport` が呼ぶ。

`callback` のタスクは、呼び方が `lambda` か `aws sqs:sendMessage` か、どれも無いもの。答えの戻り方は出力先ごとに違う。

- Step Functions：`.waitForTaskToken` で、トークンを `task_token` として payload に入れる。SQS ではメッセージの本文（レコードか `json` の `MessageBody`）に入れる。答えは `SendTaskSuccess`。
- Temporal：送る側（アクティビティ）に `callback_id` を渡し、答えは Update `dandori.answer`（`client.ts` の `answer`）で `{ callback_id, ok }` か `{ callback_id, error, message }` として受ける（シグナル `dandori.callback` でも受ける）。どのワークフローに送るかは `callback_id` から分かる（`io.workflowOf`）。
- durable functions：`createCallback` の ID を `callback_id` として渡し、答えは `SendDurableExecutionCallbackSuccess`。
- Argo：送る側のコンテナに `callback_id` を渡し、そのあとの suspend のステップで待つ。答えは `argo node set` で待ちのステップの出力 `answer` に入れ、`argo resume` で進める（4.4）。
- pydantic-graph：送る側の関数に `callback_id` を渡し、答えは同じプロセスの中で `Deps.callbacks.answer(callback_id, …)` として受ける（4.5）。

`event` のタスクは何も呼ばない。ワークフローは、そのタスクの名前の出来事が、ワークフロー ID を宛先にして送られてくるのを待つ（Temporal の Update `dandori.event`。`client.ts` の `send(client, workflowId, "<名前>", { ok: 値 })`）。承認の画面や運送会社の通知のように、送る側が注文番号のような業務の ID しか知らないときに、コールバックの ID を先に渡さずに済む。

```
# The carrier tells the workflow of the order what became of it, as often as it has news.
task 配達の知らせ() -> 注文
  event
  observes
  timeout 7 days
```

- 引数は取らない（何も送らないので）。呼び方、`callback`、`retry`、`key`、案件を始めることや出来事を送ることも書けない（E007）。答えの型、宣言したエラー（送る側がエラーの名前を送る）、`observes`、`timeout` は、コールバックと同じに書ける。`timeout` が無ければ、コールバックと同じく一日で期限切れになる。
- ワークフローは、待っているあいだの出来事だけを受け取る。待っていない出来事と、もう来た出来事は、Update の validator が断り、送った側に `WorkflowUpdateFailedError` が返る。送る側は、ワークフローが待つようになってから送り直す。いま何を待っているかは、クエリ `dandori.status` の `events` が答える。
- 並列の回の中では待てない（E009）。どの回も同じ名前の出来事を待つことになり、送る側がどの回に宛てるかを言えないからである。
- ほかの出力先では作れない（E050、P6）。Step Functions と durable functions が実行に値を送れるのは、タスクが渡すトークンや ID を通してだけである。Argo（名前で再開する suspend のステップ）と pydantic-graph（`Deps` に名前で届く値）は作れるはずだが、まだ書いていない。

AWS の API のエラーは、Step Functions では「サービスの接頭辞.例外の名前」になる（`DynamoDb.ConditionalCheckFailedException`）。接頭辞はサービスの名前を大文字にしただけではないので（`dynamodb` は `DynamoDb`、`sesv2` は `SesV2`）、Step Functions の開発者ガイドの一覧（417 のサービス）を `src/aws.rs` に表として持つ。一覧に無いサービスは E007 とする。

### 1.7 エージェント

`agent "<指示>"` のタスクは、引数をモデルに読ませ、タスクの型の値を答えさせる。問い合わせの文面から種類と注文番号を取り出す、返事を下書きする、といった読むことと書くことをモデルに任せ、決めることは rulec の規則に残す（`examples/inquiry`）。エージェントの答えも、外の相手が返すほかの値と同じに扱う。流れが分かれるのは、その値を `match` するところか、規則に渡したところだけなので、P1 は変わらない。

エージェントは OpenAI の Responses API を話すもの（`agent "<指示>"`、`agent openai "<指示>"` と書いても同じ）か、Claude のもの（`agent claude "<指示>"`）。前者は、`url` が無ければ OpenAI に、あれば、そのサーバーに送る（下の Open Responses の項）。例の AWS 向けの版では、読むのが OpenAI、書くのが Claude である。

```
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, take out the order number if one is written, …"
  model "gpt-5.4-mini"
  connection "arn:aws:events:…:connection/openai/…"
  timeout 60 seconds
  retry 2 times every 10 seconds

task draft_reply(kind: routing.kind, point: string, order_id: string?, within: duration[h]) -> string
  agent claude "Draft the first reply to the inquiry, politely, in three sentences at most. …"
  model "claude-sonnet-5"
  connection "arn:aws:events:…:connection/claude/…"
  timeout 60 seconds
```

- **答えの型を JSON Schema にして、モデルをそれに従わせる。** OpenAI の Structured Outputs の strict の形で書く。レコードのフィールドはすべて必須でほかは許さず、`T?` は null との選択、列挙はその値、`timestamp` は Step Functions の Wait が受け付ける形の正規表現、単位の付いた数は単位を説明に書く。Structured Outputs は一番外がオブジェクトであることを求めるので、どの型でも `{"answer": …}` に包む。Claude の構造化出力にも同じ Schema を渡す。この形は Claude も受け付け、`timestamp` の正規表現も、Claude が受け付ける単純なもの（`^…$`、`[0-9]`、`{4}` のような回数、グループ）に収まる（Claude の文書で確かめた）。答えは、ほかのタスクと同じく型で確かめ、外れれば `Dandori.BadResponse` になる。
- **どの出力先でも、モデルには同じものを渡す。** 渡すのは指示、引数を JSON にした文字列（引数は宣言の順）、答えの Schema の三つで、モデルの設定は何も足さない。Step Functions は HTTP Task から Responses API に送り、API キーは EventBridge の接続に置く（HTTP Task の定義には `Authorization` のヘッダを書けない）。ほかの出力先で dandori が書く実装は、`Transport` の `agent` を通し、OpenAI の Agents SDK で走らせる。Agents SDK は GPT-5 系のモデルに推論の強さなどの設定を自分で足すので、既定の `Transport` は空の設定を渡して、Step Functions と同じ要求にしている。Agents SDK が使う OpenAI のクライアントは、既定では失敗を二度までやり直す（走らせて確かめた。500 を返すと三回送った）。既定の `Transport` はやり直さないクライアントを渡し、やり直しはタスクの `retry` だけにする（Step Functions の HTTP Task も自分ではやり直さない）。API キーは `OPENAI_API_KEY` から読む。ほかのモデルの提供元を使うときは、Agents SDK の実行の設定を `Transport` に渡す。
- **Open Responses を話すほかのサーバーにも送れる（`url`）。** Open Responses は、OpenAI の Responses API を、特定の会社に寄らない仕様として書き出したものである（2026 年 1 月。OpenAI・Hugging Face・OpenRouter・Ollama・vLLM・LM Studio・Vercel が加わり、Anthropic は加わっていない）。dandori が OpenAI に送る要求（`model`・`instructions`・`input`・`text.format` の `json_schema`）は、この仕様の形そのものなので、送り先を変えればそのまま通る。タスクに `url "<ベース URL>"` を書くと、`<ベース URL>/responses` に送る。
  - Step Functions では、HTTP Task の送り先が変わるだけである。ほかの出力先で dandori が書く実装は、SDK を通さず、Step Functions と同じ本文を HTTP（`fetch`、urllib）で送る。SDK は仕様に無いものを送ることがあり、それをどのサーバーも受け付けるとは限らないからである。
  - 答えは Step Functions の JSONata と同じく、メッセージの最初の `output_text` から読む。それが無ければ（断りのときも）`failure`、エラーのステータスも `failure` で、やり直さない（`AgentStopped`・`AgentHttpError`）。
  - サーバーの鍵は、HTTP のタスクと同じく `Transport` の `headers` で足す。OpenAI の鍵をほかのサーバーに送ることはない。
  - サーバーが受け付ける Schema の大きさは分からないので、OpenAI の上限（E007）は OpenAI に送るときだけ当て、ほかは答えの検査に任せる。
  - Step Functions の HTTP Task は HTTPS の API しか呼べない。非公開の API でも、公開のドメイン名と広く信頼された証明書が要る。`http://` の送り先は E050 である（ふつうの `http` のタスクも同じにした）。EventBridge の接続はいつも鍵を持つので、鍵の要らないサーバーでも何かを入れる。
  - Claude のエージェントには `url` を書けない（E007）。Anthropic は Open Responses に加わっておらず、Ollama の Anthropic 互換の `/v1/messages` は `output_config` の `effort` しか読まず、答えの形（`format`）を読まない（ソースで確かめた）。
  - Ollama（0.32.13）は、`text.format` の `json_schema` で生成そのものを縛る。文書の項目の一覧に `text` は無いが、ソースでは受けている。走らせると、列挙に無い値で答えるよう指示しても、答えは列挙の値にとどまった。Schema を外すと、ただの文章で答えた。
- **Claude には、同じ三つを Messages API の形で渡す。** 指示はシステムプロンプトに、引数の JSON の文字列は user のメッセージ一つに、Schema は `output_config.format` に入れる。Messages API は答えの長さの上限（`max_tokens`）を必ず求めるので、どの出力先も同じ 16,000 を送る。モデルが考えるのに使うトークンもこれに含まれる。ヘッダは `anthropic-version: 2023-06-01`。Step Functions の API キーは、EventBridge の接続に `x-api-key` のヘッダとして置く。ほかの出力先で dandori が書く実装は、Anthropic の SDK（`@anthropic-ai/sdk`、Python では `anthropic`）で送り、API キーは `ANTHROPIC_API_KEY` から読む。この SDK も既定で失敗を二度までやり直すので、既定の `Transport` はそれを切る。
- **Claude の答えの列挙は、大文字と小文字を区別せずに読む。** Claude の構造化出力は、列挙の値の大文字と小文字を守らないことがある（Claude の文書にそう書かれ、エラーも特別な終わり方も無く返る）。そこで、Claude のエージェントの答えにある列挙の値は、宣言した値と大文字と小文字だけが違えば、その値として読む。Step Functions では答えを読む JSONata の式で、ほかの出力先では dandori が書く実装（`io.fold`）で直す。大文字と小文字だけが違う値を並べた列挙を答えに含むと、どちらの値か決められないので E007。
- **エージェントは相手の側を何も変えない。** だから `key` は書けず（E007）、`key` なしでやり直しても E030 にならない。ツールを持たせないので、読んで答える一往復で終わる。
- **エラーは宣言しない**（E007）。モデルが断ったとき、答えを読めなかったとき、呼び出しが失敗したときは `failure`、時間を過ぎたときは `timeout` になる。Claude では、答えが一つの手番として終わらなかったとき（`stop_reason` が `end_turn` でないとき。断りと、上限で切れた答え）も `failure` になる。断りも `failure` なので、`retry` でやり直せ、`on failure` で受けられる。
- `callback` にはできず、案件に何かするタスク（`starts`・`sends`・`observes`）にもできない（E007）。答えの型を書かないエージェントも E007。
- 答えの型が Schema に書けないもの（`json`、ほかのレコードを通して自分を含むレコード）と、提供元の受け付ける大きさを超えるものも E007 にする。OpenAI の Structured Outputs は、オブジェクトの入れ子が 10 段、プロパティが 5,000 個、列挙の値が 1,000 個まで。Claude の構造化出力は、null との選択（`anyOf`）が一つの要求の中で 16 個までなので、`T?` が 16 個を超える答えは書けない。Claude には文書に数の無い上限（Schema から作る文法の大きさ）もあり、これは送るまで分からない。
- 提供元は `openai` と `claude` だけで、ほかの名前は E007。`url` の誤り（エージェントでないタスクの `url`、Claude の `url`、http:// でも https:// でもない `url`）も E007。

### 1.8 並列の回

`for … in parallel` の各回は同時に回る。それでも五つの出力先で意味がそろうよう、次のように決めた。

- 各回は自分の変数を持つ。回の中で値を入れる変数は、その回のものになり、回の外の変数に値を入れることはできない（中と外の両方で値を入れる名前は E009）。回の外の変数は読める。案件は動かせない（E009）。`break` と `succeed` は書けない（E009）。
- ある回が失敗しても、ほかの回は最後まで回る。すべての回が終わってから、失敗した回があれば、リストの順で最初の回の失敗でループが失敗する。タスクのエラーならそのあと `on failure` が走り、`fail` や答えの検査の失敗ならそのまま終わる。どの回が先に失敗したかは出力先によって違いうるので、時間の順ではなくリストの順で決める。
- `yield` を集めたリストは、リストの順に並ぶ。

Step Functions の Map は、どれか一つの回が失敗すると Map ごと失敗してほかの回を止める。そこで、回の中のエラーは回の中ですべて受け、回は「yield した値」か「どう失敗したか」を答えとして終わる。Map のあとで、その答えの並びから最初の失敗を探す。

### 1.9 ほかの `.flow` を子として走らせる

子ワークフローも dandori で書いたなら、タスクに `flow "<パス>"` と書く（パスは親の `.flow` のある場所から）。検査は子の `.flow` を、それだけのときと同じに検査し、タスクをその入力・出力・失敗に合わせる（E015）。子が検査を通らなければ、その一つ目の誤りを添えて E015 にする。

```
task arrange_delivery(order_id: string, carrier: urgency.carrier, recipient: string?, extra: json) -> Delivery
  flow "../arrange_delivery.flow"
  errors NoVan
```

- **引数**：子の入力の名前で渡す。子の入力のうち `T?` でないものは、どれも渡す。子に無い名前の引数は書けない。
- **答え**：子は出力を一つのレコードとして答える。タスクの答えの型はレコードで（`json` でもよく、答えを読まないなら書かない）、そのフィールドは子の出力になければならない（`T?` のフィールドは無くてもよい）。子の出力のうち、タスクが読まないものはかまわない。
- **エラー**：タスクが宣言するエラーは、子の `fail` の名前のどれかでなければならない。宣言しない名前で子が失敗すれば、親では `failure` になる。
- **型は形で比べる。** 二つのファイルは、レコードと列挙をそれぞれ自分で宣言するからである。レコードはフィールドごとに、列挙は値で比べる。値の渡る向き（親の引数から子の入力へ、子の出力から親の答えへ）に、渡す側の列挙の値が受ける側の値に含まれていればよい。無いことがある値は、無いことがある値を受けるところにしか渡せない。
- **範囲も同じ向きに比べる**（1.3）。渡す側の範囲が、受ける側の範囲に収まっていなければならない。受ける側にだけ範囲があるときも E015 である（渡す側は、その外の値を渡しうる）。
- 自分を、直接にもほかのフローを通しても走らせるフローは E015（P4）。検査は、いま検査しているフローの並びを持っていて、子がその中にあれば止める。
- `flow` のタスクは、ほかに何も呼ばない（`lambda`・`http`・`aws`・`agent` と `image` は E007）。`callback`・`event`・`key` も、ほかの子ワークフローと同じく書けない。

呼び方は、子の `.flow` から決まる。

- Temporal：子ワークフローの型は子の `<名前>_v<版>` で、タスクキューも同じ名前にする。子の `worker` が待つキューである（4.2）。`workflow "<型>"` を書けば、その型で呼ぶ（キューは `queue` で選ぶ）。
- Argo：子の WorkflowTemplate（名前は子のワークフローの名前から作る。4.4）から Workflow を作り、その `dd_output` を答えとして読む。
- Step Functions と durable functions：子を配った先の ARN は `.flow` からは分からないので、`state machine "<ARN>"` と `durable function "<ARN>"` を書く。書かなければ、ほかのタスクと同じく E050。
- pydantic-graph：ほかの子ワークフローと同じく、利用者が書く関数になる（子のグラフを走らせる関数など）。契約の検査は同じにかかる。
- 子の `fail` の名前を宣言したエラーとして受けられるのは、Temporal（子の `fail` は、その名前の `ApplicationFailure` で子ワークフローを終える）と durable functions（`ErrorData` に名前が入る。4.3）。Step Functions と Argo では、子のエラーを宣言したタスクは、これまでどおり E050。

### 1.10 API の記述

呼ぶ API に記述があれば、`use` で読み、タスクをそれに合わせる（E016）。

```
use openapi stripe from "specs/stripe.json"
use smithy sns from "specs/sns.json"
use proto warehouse from "specs/warehouse.proto"
  url "https://warehouse.example.com"

task confirm_intent(intent: string) -> PaymentIntent
  http POST stripe "/v1/payment_intents/{intent}/confirm"

task notify(TopicArn: string, Message: string) -> Sent
  aws sns:publish
  errors no_recipient = NotFoundException

task reserve_stock(sku: string, quantity: int) -> Reservation
  connect warehouse "StockService/Reserve"
  errors busy = resource_exhausted
```

- **OpenAPI**（OpenAPI 3 の JSON）：`http <メソッド> <API> "<パス>"` は、記述のその操作を呼ぶ。URL は記述の最初のサーバー（`use` の下に `url` を書けば置き換わる）とパスで、本文を URL エンコードするかは記述が言う（`form` は書かない。書けば E007）。
- **Smithy**（AWS が公開している API モデルの JSON AST）：`aws <サービス>:<操作>` は、サービスの名前で `use smithy` した記述があれば、その操作に合わせる（`publish` は `Publish`）。
- **protobuf**（proto3 の `.proto`）：`connect <API> "<サービス>/<メソッド>"` は、そのメソッドを Connect のプロトコルで、JSON で呼ぶ。`.proto` はサービスの場所を言わないので、`use proto` の下に `url` を書く（無ければ E016）。

合わせるもの。

- **引数**：操作が受け取らない名前は書けない。操作が必ず要るものは渡す（無いことがある値では足りない）。型と範囲は、値の渡る向きに比べる（1.9 と同じ）。GET と DELETE では、パスに入らない引数をクエリの引数と比べ、ほかでは本文と比べる。
- **答え**：タスクの答えの型は、操作の答えの形に合わなければならない。レコードのフィールドは答えにあるもので、答えが省くことがあるもの（必須でないもの、null のことがあるもの）は `T?` にする。列挙は、操作が答えうる値をすべて持つ。答えのレコードの食い違いは、フィールドごとに一つずつ言う。
- **エラー**：OpenAPI では、宣言したエラーのステータスで操作が答えること（`4XX` や `default` も見る）。Smithy では、宣言した例外が、操作かサービスのエラーにあること。Connect では、エラーを Connect のコード（`not_found` など 16 個）で書き、コードの HTTP ステータスで見分ける（Connect のプロトコルの文書の対応表のとおり）。同じステータスになるコードを二つ宣言すると（`invalid_argument`・`failed_precondition`・`out_of_range` はどれも 400）、ほかの HTTP のタスクと同じく E007。
- **Smithy の `key <引数>`**：その引数が、操作の冪等トークン（`@idempotencyToken`）であること。

記述ごとの読み方。

- OpenAPI：`$ref` は、記述の中を指すものだけを読む。`anyOf` と `oneOf` は、送る値はどれか一つに合えばよく、答えはどれに合っても読めなければならない。Stripe の展開できるフィールド（`x-expansionResources`。ID か、展開したオブジェクトのどちらか）は、展開を頼まないので ID として読む。YAML の記述は読まない（JSON に直して渡す）。
- Smithy：AWS のモデルは、答えのメンバーが必ずあるとはほとんど言わない。だから答えのメンバーを `T?` にしろとは言わない（無ければ、走らせたときに答えの検査が断る）。SQS の `sendMessage` の `MessageBody` には、レコードや `json` を渡せる（Step Functions も dandori が書く実装も、JSON の文字列にして送る）。コールバックのタスクの答えは、コールバックが持ってくるものなので、操作の答えとは比べない。
- protobuf：答えの JSON の名前は、フィールドの名前を lowerCamelCase にしたもの（`json_name` があればそれ）なので、タスクの答えのレコードもその名前で書く（別の名前なら、JSON の名前を示して E016）。送るときは、フィールドの名前でも JSON の名前でもよい（protobuf の JSON を読む側はどちらも読む）。64 ビットの整数は JSON では文字列で来るので `string` で、小数は dandori に型が無いので `json` で読む。ストリームのメソッドは呼べない。
- 読むのは、タスクの型がたどるところだけで、記述の全体を型にはしない。8 MB ある Stripe の記述でも、見るところだけの手間で済む。

**protobuf の JSON が省く既定値。** proto3 のフィールドのうち、値があるかどうかを持たないもの（`optional` でも、メッセージでも、`oneof` の一つでもないもの）は、既定値（空の文字列、0、false、列挙の最初の値、空のリストとマップ）のとき JSON から省かれる。protobuf で読む側は、省かれたフィールドを既定値として読む。dandori も同じに読む。`connect` のタスクの答えは、省かれた（または null の）フィールドを既定値で埋めてから型を確かめる。答えの中のメッセージと、メッセージのリストの中でも埋める。埋める表は `.proto` から作って生成するコードに書き込み、Step Functions では答えを読む JSONata の式で、ほかでは `io.fill` で埋める。値があるかを持つフィールドは埋めない（タスクの答えでは `T?` にする）。列挙の最初の値（慣わしでは `…_UNSPECIFIED`）は、タスクの列挙に無くてもよい。そのときは、その値の答えを答えの検査が `Dandori.BadResponse` で断る。

**Connect の呼び出し。** POST で `<url>/<パッケージ>.<サービス>/<メソッド>` に、JSON の本文とヘッダ `Connect-Protocol-Version: 1` を送る。Step Functions の HTTP Task も、dandori が書く実装も同じものを送る（5 章で比べる）。`key` は、ほかの HTTP のタスクと同じく `Idempotency-Key` ヘッダで送る。

## 2. 案件とステートマシン

### 2.1 案件の状態を追う

案件は、外の相手が持つ実体で、rulec のステートマシンに従う。たとえば Stripe の PaymentIntent を、rulec に書き写した `payment_intent.rule` に従わせる。

検査は、実行の各点で案件ごとに次を持つ。

- 始まったか（はい・いいえ・どちらもありうる）
- 最後に分かった状態の集合と、その状態ごとに、そこへ着く短い例

`sends e` の呼び出しでは、いまの状態の集合から、ステートマシンの表で `e` のあとの状態を求める。表の軸のうち、状態と出来事と held で決めたもの以外は、外の相手が決める値として全部の座標を試す。行が受け付ける座標は rulec の証明書にあるので、`policy first` の表で前の行に取られる行も数えない。

### 2.2 相手の側の出来事

`external` に書いた出来事は、ワークフローが何もしなくても起きる（客の認証、銀行の結果、与信の期限切れ）。ワークフローが案件に次にタスクを呼ぶときと、ワークフローが終わるときには、最後に分かった状態の集合を、その出来事で移れる状態まで広げてから考える。

これで、チェックアウトまで待つあいだに与信の期限が切れて `canceled` になり、capture が断られる、という流れが検査に入る。

### 2.3 断りと失敗

- ステートマシンが出来事を断る状態があれば、その呼び出しは `refused as` のエラーで返ってくる。断る状態が一つでもあるのに受けていなければ E022、どの状態でも断るなら E021。
- 断り以外で失敗したときは、相手の側で出来事が起きたかどうかが分からない。検査は、起きた場合と起きなかった場合の両方を持って先へ進む。

### 2.4 まだ始まっていない案件を見る

始まっていない案件に `observes` のタスクを当てると、すでにある案件を読み込む意味になる。状態は、ステートマシンが始まりの状態からたどり着けるどれでもありうる。倉庫のシステムにある注文を読むときがこの形である。

### 2.5 終わり方

`succeed`・`fail`・flow の終わりで、始まった案件がどれも終わりの状態（rulec の `final`）にいるかを見る。相手の側の出来事で移れる先も含めて、終わりでない状態が残れば E020 とする。`fail … leaving c` は、終わっていない `c` を承知で引き渡す宣言である。

処理されなかったタスクのエラーで失敗する場合も数える。`on failure` があればそのブロックを検査し、無ければ案件ごとに W101 を一つ出す。

### 2.6 キャンセル

Temporal では、ワークフローに止まるよう頼める（キャンセル）。頼まれたワークフローは、後始末をしてから、キャンセルされたとして終わる。`on cancel` はその後始末を書くブロックである。

```
on cancel
  match 受注.状態
    none => pass
    受付, 入金済 =>
      受注 <- 取消を頼む(id: 受注.id)
        on 断られた => pass
    出荷済 => fail ShippedAlready "もう出荷されています" leaving 受注
    配達済, 取消 => pass
```

- キャンセルは、呼び出しの答えを待つあいだか、待ち（`wait`、コールバック）のあいだに届く。`flow` の中でも `on failure` の中でも届く。流れはそこで止まり、`on cancel` が走り、ワークフローはキャンセルで終わる。`on cancel` の中の `fail` は、ワークフローを失敗で終える。`succeed` は書けない（E009）。
- `on cancel` の中にはキャンセルは届かない。後始末の呼び出しは、キャンセルに止められずに最後まで走る。`on cancel` の中で受けられなかったタスクのエラーは、`on failure` を通らずにワークフローを失敗させる。
- キャンセルはタスクのエラーではない。`retry` はやり直さず、`on failure =>` などの受けも受けない。並列の回の中で届いたキャンセルは、すべての回を止め、それより前の回が失敗していても、キャンセルのほうが決める。
- `on cancel` が無ければ、ワークフローはキャンセルが届いたところで終わる。Step Functions の `StopExecution` と同じく、案件は片付けられない。
- 検査は、キャンセルが届きうるところ（`on cancel` の外のすべての呼び出しと待ち）から `on cancel` に入る。呼び出しの途中で届いたキャンセルでは、相手の側で出来事が起きたかどうかが分からないので、失敗のときと同じく両方を持つ。`on cancel` の終わりで案件が終わりの状態にいなければ E020、後始末の呼び出しが失敗しうれば W101。
- Temporal のほかの出力先では、`on cancel` は E050。Step Functions と durable functions は、実行を止めるとその場で終え、あとに何も走らせない。Argo の `argo stop` は exit handler を走らせるが、`on cancel` をそれとして書くのはまだである。pydantic-graph も同じくまだである。

## 3. 検査の一覧

| コード | 内容 |
|---|---|
| E001 | 構文 |
| E002 | 名前が無い（型・変数・フィールド・規則・タスク。単位の種類の誤りも） |
| E003 | 型が合わない（無いことがある値をそのまま使う、`none` の置き場所、リストのリスト、型の分からない `{…}` や `[]`、数でないものの範囲、数の入らない範囲も） |
| E004 | 引数や出力の過不足、`{…}` のレコードのフィールドの不足 |
| E005 | 規則を rulec で読めなかった |
| E006 | 二度の宣言 |
| E007 | タスクの項目の誤り（`flow` のタスクのほかの呼び方と `image`、OpenAPI の操作を呼ぶタスクの `form`、Connect のエラーコードの誤り、ステータスの付け忘れ、同じステータスの二つのエラー、呼び方の重なり、`aws` の `key` の引数、`callback` にできない呼び方、知らない AWS のサービス、子ワークフローの `key`、エージェントの `model` の書き忘れ・宣言したエラー・`key`・Schema に書けない答え・知らない提供元・Claude で大文字と小文字だけが違う列挙の値、`event` のタスクの引数・呼び方・`retry`・`key`・案件を始めることなど、エージェントでないタスクや Claude の `url`、http でも https でもない `url`） |
| E008 | 案件の宣言や、案件に何をするかの誤り |
| E009 | 文の置き場所の誤り（`yield`、並列の回の中の `break`・`succeed`・案件の呼び出し・出来事の待ち、`on failure` と `on cancel` の中の `succeed`、中と外の両方で値を入れる変数、規則を `let` なしで呼ぶことも） |
| E010 | `match` に行き先の無い値がある |
| E011 | 通らない行き先 |
| E012 | 値を持っていないことがある変数を読んだ |
| E013 | 始まっていない案件にタスクを呼んだ、始まった案件をもう一度始めた |
| E014 | 範囲のあるところ（規則の入力、タスクの引数、書き出したレコードのフィールド、出力）に、範囲を外れることのある値を渡した |
| E015 | ほかの `.flow` を走らせるタスクが、子と合わない（引数と入力、答えと出力、宣言したエラーと子の `fail`。子を読めない、子が検査を通らない、自分を走らせるフローも） |
| E016 | タスクが API の記述と合わない（無い操作、受け取らない引数、要る引数の不足、型・範囲・列挙の違い、答えが省くフィールドを `T?` にしていない、答えないステータス、無い例外、冪等トークンでない `key`、ストリームのメソッド。記述を読めない、`.proto` に `url` が無いときも） |
| E020 | 終わりでない状態の案件を残して終わる（`on cancel` が終わってキャンセルで終わるときも） |
| E021 | どの状態でも断られる出来事を送った |
| E022 | 断られることがある出来事の断りを受けていない |
| E030 | 相手の側を変える呼び出しを、`key` なしでやり直す |
| E031 | Express でできないこと（五分を超える待ち、コールバック、入れ子の実行、`key` なしで相手を変える呼び出し） |
| E040 | 出力先で、一回の実行が大きくなりすぎうる（ビルドが出力先ごとに見る）。実行履歴が上限を超えうる（Step Functions は 25,000 件、Temporal は 51,200 件、Lambda durable functions は 3,000 操作）。Argo Workflows では、一回の実行のノードが目安の 10,000 個を超えうる |
| E050 | 出力先に出すのに要るものが無い、または出力先でできないこと（Step Functions では呼び方か `connection`（エージェントにも要る）、入れ子の実行の宣言したエラー、`http` と `agent` の 60 秒を超える `timeout`、HTTPS でない送り先。Temporal のほかでは `on cancel` と `event` のタスク。Step Functions と durable functions では呼ばれる規則の `lambda`。durable functions では invoke する関数の `timeout`。Argo では呼び方か `image`、`workflow template` の宣言したエラー、`callback` のタスクの `retry`） |
| W030 | 相手の側を変えるかもしれない呼び出しを `key` なしでやり直す |
| W101 | 処理されないエラーで、案件を終わりでない状態に残して失敗することがある（`on failure` や `on cancel` の片付けの最中も） |
| W102 | 動くことのない `on <断り>` |
| W103 | 案件を始めるタスクに `key` が無い |
| W104 | 範囲のあるところに、範囲の分からない値を渡した |

診断には、そうなる例（その点までの短い流れ）が付く。例は、状態・変数・`match` で絞った値ごとに持っているので、例の中で値が途中で変わることはない。

実行履歴の件数は、一回の呼び出し、再試行、Choice、Wait、並列の回などごとに多めに見積もった件数を足して出す（`check.rs` の `ASL_COST`・`TEMPORAL_COST`・`DURABLE_COST`・`ARGO_COST`）。多めに見積もるので、上限の近くでは言い過ぎる側に倒れる。`for` は上限の数だけ回るとして数える。

E040 は `check` ではなく、出力先ごとのビルドで出す。上限は出力先ごとに違い、Temporal だけに出すワークフローが Step Functions の上限で断られることのないようにするためである。Temporal では、フローの一番外のループが履歴の長くなったところで新しい実行で続ける（4.2）ので、そのループは、続ける目安の 10,000 件にループの一回分を足した件数と、回り切ったときの件数の小さいほうで数える。それでも上限を超えるのは、ループの外の部分か、ループの一回が大きすぎるときで、診断はそう言う（`tests/fixtures/history_parallel.flow` と `history_round.flow`）。

pydantic-graph には実行の大きさの上限が無いので、見積もらない。

Argo には履歴の件数の上限が無い。限りは Workflow の大きさで、ノードの状態を含めて 1 MiB まで（`MAX_WORKFLOW_SIZE`。超えるとノードの状態を圧縮し、それでも超えれば、状態をデータベースに置く設定が無いかぎり保存できない）。`tests/flows/edges.flow` の実行では、ノード一つが JSON で 510〜612 バイト、圧縮して 60 バイトほどだった。60 バイトなら 15,000 個ほど入るが、テストより大きな値を運ぶワークフローでは減るので、目安を 10,000 個にした。WorkflowTemplate も、Workflow の中に二度（`storedTemplates` と `storedWorkflowTemplateSpec`）圧縮されずに入る。例でいちばん大きいホテルの予約では、それぞれ 122 KB と 119 KB で、ノードが 56 個の実行の Workflow は全体で 270 KB だった。テンプレートを小さくするため、値を計算する式は、同じ値を二度読むところを式の頭の `let` 一つにまとめ、呼び出しの終わり方（エラーの種類、答えの検査、受ける `on`）も出力ごとに `let` で一度だけ求める。

## 4. 出力先

### 4.1 Step Functions

- `QueryLanguage` は JSONata。JSONPath は出さない。
- ワークフローの変数は全部、ステートマシンの変数にし、始まりの Pass で null にする。入力はそこで `$states.input` から入れ、次の Choice で型を確かめる。外れたら `Dandori.BadInput` で失敗する。並列の回が持つ変数は、回の始まりで null にする（外の変数と同じ名前を回の中で入れることを Step Functions は許さない）。
- 出力は、終わりの Succeed の `Output` に書く。出力を持たないワークフローの終わりも、`Output` を JSONata の `null` にして、実行の出力を null にする（ほかの出力先と同じ）。`Output` の無い Succeed は入力をそのまま出すので、最後の呼び出しの答えが実行の出力になってしまう（LocalStack の突き合わせで見つけた。5.1）。asl-validator は `null` をそのまま書くと断るので、式にする。
- 呼び出しは Task と、その答えを確かめる Choice の組にする。型が合わなければ `Dandori.BadResponse`、案件の状態が検査の言った集合の外なら `Dandori.UnexpectedState`。
- 無いことがあるフィールドを読む式は `($exists(x) ? x : null)` にして、式が undefined にならないようにする。
- `match` は Choice にし、どの行き先の条件も明示する。Default は `Dandori.UnexpectedValue` の Fail で、黙って最後の行き先へ流れることはない。`some y` の行き先は、Pass で `y` に値を入れてから本体へ進む。
- `retry` は Retry にする。`on` が無いときは、宣言したエラーを `MaxAttempts: 0` で止める retrier を先に置き、そのあとに `States.ALL` を置く。
- `on failure` は、各 Task の最後の Catch（`States.ALL`）から入る。エラーは変数 `dd_error` に入り、ブロックの終わりの Fail がそれで失敗し直す。
- 冪等キーは `実行名/呼び出しの場所/各ループの回数`。HTTP では `Idempotency-Key` ヘッダ、Lambda では `idempotency_key`、AWS の API では `key` に書いた引数で渡す。
- `callback` は、Lambda なら `lambda:invoke.waitForTaskToken` でトークンを `task_token` で渡し、SQS なら `sqs:sendMessage.waitForTaskToken` でメッセージの本文に入れる。
- `aws` は `arn:aws:states:::aws-sdk:<サービス>:<操作>`。引数と答えは API のまま（PascalCase）で、型は利用者がタスクに書く（HTTP のタスクと同じ）。
- `agent` は HTTP Task で `https://api.openai.com/v1/responses` に送る。本文は `model`、`instructions`、`input`（引数を `$string` で JSON の文字列にしたもの）、`text.format`（`json_schema`、名前は `answer`、`strict`）。答えは、Responses API の出力のうち `message` の `output_text` を `$parse` で読み、`answer` を取り出す。モデルが断ると `output_text` が無いので、`$error` で Task を `States.QueryEvaluationError` で失敗させる。JSONata の式の失敗はその Task の Retry と Catch で受けられる（Step Functions の文書で確かめた）ので、断りは `failure` としてやり直せ、`on failure` で受けられる。答えを `let` で受けない呼び出しでも、同じ式を Task の `Output` に置いて答えを読み、断りを失敗にする。
- Claude の `agent` は HTTP Task で `https://api.anthropic.com/v1/messages` に送る。ヘッダは `anthropic-version`、本文は `model`、`max_tokens`、`system`（指示）、`messages`（user のメッセージ一つに、引数を `$string` で JSON の文字列にしたもの）、`output_config.format`（`json_schema`）。答えは、`content` のうち `text` のブロックをつないで `$parse` で読み、`answer` を取り出し、その中の列挙の値を大文字と小文字を区別せずに宣言した値に直す（直す式は答えの型から作る）。`stop_reason` が `end_turn` でなければ、`$error` で Task を失敗させる。あとは OpenAI のときと同じ。
- HTTP Task のリクエストは 60 秒で打ち切られるので、`http` と `agent` のタスクの `timeout` が 60 秒を超えれば E050 にする。
- `state machine` は `startExecution.sync:2` で、答えは子の出力（`Output`）。子の失敗は親には `States.TaskFailed` として届き、子の `fail` の名前は Cause の中にしかない。Cause の形は文書で確かめられなかったので、入れ子の実行に宣言したエラーは E050 とし、`failure` で受けてもらう。
- `for` は、Pass と Choice で数えるループにする。要素の数は最初の Choice で確かめる。`yield` は `$append` で集める。
- `for … in parallel` は INLINE の Map にする。回の始まりの Pass が、ItemSelector で渡した要素と番号（`$states.context.Map.Item.Index` は ItemSelector でしか読めない）を回の変数に入れる。回の中の Task には、受けていないエラーを `{"fail": {Error, Cause, task: true}}` に変えて回を終える Catch を付け、`fail` や検査の失敗も同じ形で回を終える。Map のあと、答えの並びに失敗があれば最初のものを取り出し、タスクのエラーなら `on failure` へ、そうでなければ Fail へ進む。Fail の Cause は文字列でなければならないので、理由の無い `fail` は Cause を書かない Fail に分ける。
- 規則を呼ぶ Lambda 関数のコード（Python）も出す。rulec が生成した Python を import し、JSON を列挙と整数に直して呼び、答えを JSON に戻す。

### 4.2 Temporal

- `types.ts`（型と検査）、`activities.ts`（タスクの型、dandori が書くタスク、利用者が書く `OwnTasks`、それらを合わせる `makeActivities(own, transport)`）、`io.ts`（`Transport`）、`rules.ts`（規則を包むアクティビティ）、`runtime.ts`（共通の部品）、`workflow.ts`、`worker.ts`（ワーカー）、`client.ts`（始め方、コールバックの答え方、状態の聞き方）を出す。
- **ワークフローの型に版を入れる。** 型は `<名前>_v<版>`（`hotel_stay_v1`）で、ワーカーのタスクキューも同じ名前にする。`.flow` の版を上げると別のワークフローになり、古い版の実行は、古いコードのワーカーが持つ古いキューで最後まで走る。同じ版のまま dandori を上げてコードが変わったときのために、`worker.ts` は Worker Deployment Versioning にも乗れる。`deployment` を渡すと、ワーカーはそのデプロイの一つの版になり、ビルド ID は dandori が書いたコードのハッシュ（`BUILD_ID`）になる。ワークフローは始まった版に留まる（PINNED）ので、書き直されたコードが古い実行を再生することはない。Worker Deployment Versioning を使わずに同じ版のコードを差し替えるなら、`client.ts` の `histories`（いま走っている実行の履歴）を `worker.ts` の `replay` にかけ、新しいコードが古い実行を再生できるかを先に確かめる。
- **`client.ts` の `start` は、ワークフロー ID を二度使わせない**（`REJECT_DUPLICATE`）。冪等キーはワークフロー ID から作るので、ID を使い回すと、新しい実行の呼び出しが前の実行の呼び出しと同じキーを持ち、相手に同じ要求として扱われる。前の実行が失敗していても同じである。
- **ワークフローの今を、クエリ `dandori.status` で答える。** 答えるのは、いま待っている呼び出しか待ちの行（`at`）と、案件ごとの状態（`cases`、始まっていなければ null）と、いま待っている出来事（`events`）。`start` に `searchAttributes: true` を渡すと、案件の状態を search attribute `DandoriCases`（キーワードのリスト、`"pi=requires_capture"` の形）にも出し、案件が動くたびに書き直す。これで「確定待ちの与信を持つ予約」を Temporal の一覧から探せる。`DandoriCases` は名前空間に前もって登録しておく必要がある（登録していない search attribute を書くと、ワークフロータスクが失敗し続けて止まる）ので、既定では書かない。オンにしたかどうかは memo（`dandori.cases`）で伝える（始めるときに空のリストを渡すと、属性が無いのと区別できない）。
- **再試行は Temporal にさせない。** アクティビティは `maximumAttempts: 1` で呼び、ワークフローのコードが ASL と同じ retrier の並びでやり直す。待ちは `sleep` で、間隔も Step Functions と同じ計算。どの出力先でも、同じエラーを同じ回数だけやり直すためである。
- 宣言したエラーは、アクティビティが投げる `ApplicationFailure` の `type` で見分ける。`TimeoutFailure` は `timeout`、ほかは `failure`。dandori が書くアクティビティは、Lambda の `errorType`、HTTP のステータス、AWS の API の例外の名前を、宣言したエラーの名前に直して投げる。
- `queue` は、アクティビティと子ワークフローの `taskQueue` になる。別のワーカー（ほかの言語の SDK で書いたものでも）が持つアクティビティを、名前と引数の形が合えばそのまま呼べる。
- `workflow` は `executeChild` で呼ぶ。ワークフロー ID は `親の ID/呼び出しの場所/各ループの回数/何度目か`。子の失敗は `ChildWorkflowFailure` の `cause` に `ApplicationFailure` として届くので、宣言したエラーをその `type` で見分けられる。
- **`timeout` を書かないタスクには、出力先が許す以上の期限を付けない。** Step Functions の Task に `TimeoutSeconds` を書かないのと同じにする。Temporal のアクティビティは期限を持たなければならないので、`http` と `agent` には HTTP Task と同じ 60 秒、`lambda` には Lambda が関数を動かせる 900 秒、ほかのタスクには一年を付ける。代わりに、ワークフローのワーカーが持つタスク（`queue` の無いもの）は、動いているあいだ 10 秒ごとにハートビートを送り、Temporal は 30 秒来なければそのアクティビティを期限切れ（`timeout`）にする。ワーカーが落ちても 30 秒で分かり、`retry` があればやり直せる。`queue` のタスクは別のワーカーが持ち、ハートビートを送るとは限らないので、`timeout` が無ければ 60 秒にする。コールバックの ID を渡すアクティビティも 60 秒。
- **規則のアクティビティは 10 秒で、期限切れもやり直す。** 規則は純関数で、すぐに終わる。期限切れは、動かしていたワーカーが落ちたことを意味するので、失敗と同じくやり直す。規則のやり直しの並びはどの出力先でも同じで、期限切れも入る（Temporal のほかでは、規則の呼び出しに期限は無い）。
- **`callback` は Update で答えを受ける。** 送る側のアクティビティに `callback_id`（ワークフロー ID と呼び出しの場所を JSON にしたもの）を渡し、ワークフローはその ID の答えが来るのを `condition` で待つ。答えは Update `dandori.answer` で受ける（`client.ts` の `answer`）。Update は、答えを受け取ったかどうかを答えた側に返す。ワークフローが待っていない ID（渡していない ID や、期限切れで待つのをやめた ID）への答えと、二度目の答えは、validator が断る。シグナル `dandori.callback` でも受けるが、待っていない ID への答えは捨てる。`timeout` を過ぎれば `timeout` になる。
- `for … in parallel` は、`dd.rounds` が `k` 個ずつ回を走らせる。ワークフローのコードは決定的に動くので、Promise で並べても再生で食い違わない。回の変数は回の関数の中で宣言する。
- `on failure` は、処理されなかったタスクのエラー（`TaskError`）のときだけ走り、`fail` や答えの検査の失敗では走らない。Step Functions で Fail の状態に Catch が効かないのと同じにした。
- **`on cancel` は、キャンセルの届かない範囲で走らせる**（2.6）。TypeScript では、流れと `on failure` を `try` で包み、`isCancellation` で届いたキャンセルを見分けて、`CancellationScope.nonCancellable` の中で `on cancel` を走らせ、同じキャンセルを投げ直す。投げ直すと、ワークフローは Canceled で終わる。Python では `except BaseException` で受け、`temporalio.exceptions.is_cancelled_exception` で見分け、`on cancel` をそのまま走らせて `raise` する（Python の SDK はキャンセルを一度しか届けないので、受けたあとのアクティビティは止められない）。`dd.attempt` はキャンセルをタスクのエラーに直さずにそのまま通し、`dd.rounds` はキャンセルで止まった回があれば、ほかの回の失敗よりそれを先に投げる。
- **フローの一番外のループは、履歴が長くなると新しい実行で続ける（Continue-As-New）。** 一番外の `repeat` と並列でない `for` は、回の始まりに履歴が 10,000 件に達していれば（`dd.CONTINUE_AT`）、またはサーバーが勧めれば、その回を新しい実行で始める。新しい実行には、入力と一緒に、一番外の何番目のループの何回目からか、変数、`for` のリストとそれまでに `yield` した値を渡す。続きの実行は、そのループより前の文を飛ばし、変数を受け取ってその回から回る。ワークフロー ID は変わらないので、冪等キー、子ワークフローの ID、コールバックの ID は、続けなかったときと同じになる。一つの実行の中で最初に回す回では続けないので、どの実行も少なくとも一回は進む（入力が大きくてサーバーが初めから勧め続けても、止まらずに回る）。
  - サーバーの勧めは、dev server（Server 1.32.0）では、履歴が 4,096 件を超えたところで来た（4,020 件の時点ではまだで、4,123 件で来た）。10,000 件は、名前空間の設定で勧めが遅いか、来ないときの歯止めである。
  - 続きの実行は、Worker Deployment Versioning でも、始まった版に留まる（サーバーの既定）。渡す値の形（何番目のループか、どの変数か）はビルドごとのものなので、`initialVersioningBehavior` を `AUTO_UPGRADE` にして新しい版に移すことはしない。memo（`dandori.cases`）と search attribute は、サーバーが続きの実行に引き継ぐ。
  - 並列の回のループと、ループの中のループでは続けない。並列の回を途中で切ると、走っている回を新しい実行に渡せないからである。それらの大きさは E040 で見る。
- **`event` のタスクは、Update `dandori.event` を待つ。** アクティビティは呼ばない。`dd.awaitEvent` が、待っている出来事の名前を覚えたうえで、その名前の値が来るのを `condition` で待ち（期限はコールバックと同じ）、待ち終わったら名前を消す。Update の validator は、待っていない名前と、もう来た名前を断る。値は `{ event, ok }` か `{ event, error, message }` で、エラーの名前は宣言したエラーとして読む（コールバックと同じ）。`client.ts` は、出来事の名前の一覧（`EVENTS`）と、それを送る `send` を持つ。一番外のループの回で待つ出来事は、新しい実行で続けた先でも同じ名前で待つので、送る側はワークフロー ID だけを知っていればよい。
- **エージェントは一つのアクティビティにする。** Temporal には OpenAI の Agents SDK との公式の統合があり、モデルやツールの呼び出しをそれぞれアクティビティにして、エージェントの実行そのものを durable にできる。何往復もしてツールを呼ぶエージェントのための形である。dandori のエージェントはツールを持たずに一往復で答えるので、タスクの `timeout` と `retry` がそのまま効く一つのアクティビティにした。
- 規則は普通のアクティビティとして呼ぶ。rulec の TypeScript を import し、`parse<列挙>` と `BigInt` で直して呼ぶ。規則は純関数なので、決定性のためだけならワークフローのコードで直接呼べるが、そうしない。理由は三つ。
  - 判定の入力と答えが、実行履歴に残る（ActivityTaskScheduled と ActivityTaskCompleted）。どの入力でどう判定したかを後から読め、rulec の `replay` と `diff` に渡す記録にもなる。
  - 規則を直してワーカーを出し直しても、実行中のワークフローの再生が履歴と食い違わない。再生では答えを履歴から読む。
  - 規則のアクティビティを別のワーカーに置けば、ワークフローに触れずに規則だけを出し直せる。Step Functions で Lambda を呼ぶのと同じ形になる。

  費用は、一回につきタスクキューの往復と履歴 6 件。業務のワークフローで判定が数回なら問題にならない。

  **`use rule` の下に `local` を書いた規則は、ローカルアクティビティで呼ぶ**（`proxyLocalActivities`、Python では `workflow.execute_local_activity`）。ワークフローを走らせているワーカーの中で走り、答えは履歴にマーカー一件で残る（ワークフロータスクの中で終われば、それだけ）。一回の判定が履歴 6 件から 1 件になり、タスクキューの往復も無い。判定の入力と答えは、マーカーとして履歴に残る。代わりに、規則はワークフローと同じワーカーに置くことになり、規則だけを出し直すことはできない。ループの中で何度も判定するワークフローのための形で、E040 の見積もりも、一回を 4 件（マーカーと、続くかもしれないワークフロータスク）、やり直しを 9 件で数える（`tests/fixtures/history_local.flow` は、`local` なら通り、無ければ Temporal でも E040 になる）。期限（10 秒）とやり直しは、ほかの規則と同じ。
  - ローカルアクティビティのエラーは、アクティビティのエラーに包まれずに届く。失敗は `ApplicationFailure`（Python では `ApplicationError`）のまま届き、期限切れは、初めて走るときは `ActivityFailure` に包まれた `TimeoutFailure` で、再生では包まれない `TimeoutFailure` で届く（TypeScript の SDK 1.24.0 と Python の SDK 1.33.0 の両方で、dev server の上で確かめた）。生成するコードは、包まれていてもいなくても同じ種類に読む。そうしないと、期限切れを受ける `on timeout` が、再生では失敗と読まれて別の道に進み、非決定的と言われる（`tests/flows/local_rules.flow` は、期限切れを受けてタスクを呼ぶので、これが起きれば再生で分かる。直す前は、実行が終わったあとのクエリの再生で `Nondeterminism error` になった）。

**Python SDK 向け**（`--target temporal-python`）も出す。ワークフローの名前のパッケージに、`types.py`、`activities.py`（`make_activities(own, transport)`）、`io.py`、`rules.py`、`runtime.py`、`workflow.py`（ワークフローのクラスと、ワーカーに渡す `workflows`）を置く。`worker.py`（`worker_options` と `make_worker`）と `client.py`（`start`・`answer`・`status`）も同じ形で出す。意味は TypeScript 版と同じで、線に乗る名前もそろえた。ワークフローの型、アクティビティの名前、コールバックの Update とシグナル、クエリ、search attribute、子ワークフローとコールバックの ID が同じなので、TypeScript のワーカーが持つアクティビティを Python のワークフローから呼べ、その逆もできる（5 章で、全部のシナリオを両方向に走らせて確かめている）。書き方が違うのは次のところ。

- Python にはラベルを付けた `break` が無い。`on` のある呼び出しは `try` にし、`except` でタスクのエラーを受けて、合う `on` が無ければ投げ直す。答えの検査は `else` に置く。ループの `break` は Python の `break` のまま使える。コードの中のループは `.flow` のループだけだからである。
- 並列の回は、回ごとの `async def` を `asyncio.gather` で `k` 個ずつ走らせる。Temporal の Python SDK はワークフローの中の asyncio を決定的に動かす。
- 待ちは `asyncio.sleep`（SDK が durable なタイマーにする）、今の時刻は `workflow.now()`。
- 値は JSON のままの `dict` と `list` で運び、型は `TypedDict` と `Literal` の注釈にとどめる。フィールドの名前が日本語でも、そのまま鍵になる。流れの名前が Python の予約語や組み込みと重なるときは、後ろに `_` を付ける。
- 規則は、rulec の Python をパッケージの中の `rulec/python/` から読むアクティビティにする。Step Functions の Lambda と同じく、列挙はその型で、数は `int` で渡す。
- SDK は、空のエラーメッセージを受け取ると「Application error」と読む。ワークフローの中でエラーの理由を読むときは、届いた failure の `message` をそのまま読む。
- 新しい実行で続けるのは `workflow.continue_as_new` で、履歴の長さとサーバーの勧めは `workflow.info()` の `get_current_history_length()` と `is_continue_as_new_suggested()` で読む。続きの実行に渡すものは TypeScript 版と同じ形の `dict` で、`run` の二つ目の引数で受け取る。

### 4.3 Lambda durable functions

Temporal と同じく、コードを書き、チェックポイントと再生で続きから動かす形なので、Temporal の生成器と骨組みを共有する（型、答えの検査、やり直しの並び、エラーの種類、`on failure`、`io.ts`）。違うのは、タスク・待ち・コールバック・子の呼び方である。

- `types.ts`、`tasks.ts`（タスクの型、dandori が書くタスク、`OwnTasks`、`makeTasks`）、`io.ts`、`runtime.ts`、`workflow.ts` と、呼ばれる規則ごとの Lambda 関数（Step Functions 向けと同じ Python）を出す。`workflow.ts` は `makeHandler(own, transport)` を出し、利用者は `export const handler = makeHandler(自分のタスク)` と書く。
- **タスクは step の中で呼ぶ。** `lambda` のタスクも `context.invoke` にはせず、dandori が書く実装が step の中で AWS SDK から Lambda の Invoke を呼ぶ。`context.invoke` では、invoke の先で投げたエラーの名前が `InvokeError` の中で落ちる（SDK 2.4.0 のソースで、`ErrorType` を捨てて `ErrorMessage` と `ErrorData` だけを残していることを確かめた）。Invoke の答えでは、関数が投げたエラーの型が `errorType` に残る。
- **`durable function` は `context.invoke` で呼ぶ。** 長く動く durable function を待つにはこれしかない。エラーの名前が落ちる問題は、`ErrorData` で渡して避ける。dandori が出す durable function は、`fail` のとき `ErrorData` に `{"error": <名前>}` を入れて失敗する（`Failure`、SDK の `DurableOperationError` を継ぐ）。呼ぶ側はそれを読んで宣言したエラーを見分ける。dandori 以外の関数も、同じ形で `ErrorData` を入れれば見分けられ、入れなければ `failure` になる。invoke には期限の設定が無いので、`durable function` のタスクの `timeout` は E050。
- **規則は `context.invoke` で Lambda を呼ぶ。** 規則は宣言したエラーを持たないので、上の問題に当たらない。Temporal で普通のアクティビティにしたのと同じ理由で、入力と答えがチェックポイントに残り、規則だけを出し直せる。
- **再試行は SDK にさせない。** step には `retryStrategy: () => ({ shouldRetry: false })` を渡す（渡さないと SDK の既定のやり直しが効く）。ワークフローのコードが、Step Functions と同じ retrier の並びでやり直し、あいだは `context.wait` で待つ。
- **相手の側を変えて `key` の無いタスクは、「多くとも一回」の step にする**（`StepSemantics.AtMostOncePerRetry`）。既定の「少なくとも一回」では、答えを記録する前に落ちると同じ呼び出しがもう一度走る。「多くとも一回」の step は、中断されると結果の分からない失敗になり、検査はそれを「出来事が起きたか分からない失敗」として扱っている。
- **`callback`** は `context.createCallback` で ID を作り、別の step の中でタスクに `callback_id` を渡す。相手は `SendDurableExecutionCallbackSuccess` で答えを JSON にして返す。期限切れは `CallbackTimeoutError` で、`timeout` になる。
- **`for … in parallel`** は `context.map` にし、回の中のコードは回の子の context を通して呼ぶ。回は、yield した値か、どう失敗したかを JSON にして答える（チェックポイントに残すため）。`map` のあとで最初の失敗を投げ直す。
- **今の時刻は step の中で読む。** `wait until` は、step で読んだ時刻（チェックポイントに残る）から待つ秒数を出す。Temporal と違って、ワークフローの中の `Date.now()` は再生のたびに変わる。
- 冪等キーは `context.executionContext.durableExecutionArn` から作る。
- 一回の実行で使える操作は 3,000 回まで（引き上げられない）。E040 はこれも見積もる。

### 4.4 Argo Workflows

- `<ワークフロー>.argo.yaml`（WorkflowTemplate）と、`caller/`（`lambda`・`http`・`aws`・`agent` のタスクと規則をコンテナの中で動かすプログラム。`types.ts`・`tasks.ts`・`io.ts`・`transport.ts`・`rules.ts`・`call.ts`・`package.json`・`Dockerfile`）を出す。入力はパラメータ `input` に JSON で渡し、出力はグローバル出力パラメータ `dd_output` に残る。
- **タスクはどれもコンテナで動く。** `image` のタスクは利用者のイメージ、`lambda`・`http`・`aws`・`agent` のタスクと規則は `caller/` から作るイメージで動く（エージェントがあれば、`package.json` に `@openai/agents` か `@anthropic-ai/sdk` が入る）。中の実装は Temporal と durable functions 向けと同じもので、同じ `Transport` を通す。呼び出しは環境変数 `DANDORI_CALL` で渡る。答えは `/tmp/dandori/answer.json` に書き、宣言したエラーなら `/tmp/dandori/error.json` に `{"error", "message"}` を書いて終了コード 3 で終える。期限で止められた Pod（終了コード 143 か 137）は `timeout`、ほかの終わり方は `failure`。
- **Argo には変数が無いので、ワークフローの状態をグローバル出力パラメータに置く。** 変数ごとに `v<番号>`（値は JSON）、流れがどう進むかを表す `dd_ctl`（`next`・`break`・`succeed`・`fail`・`task`）、失敗の `dd_error`、出力の `dd_output`。並列の回が持つ変数は、回ごとに接尾辞を付けた別のパラメータにする。YAML の頭のコメントに、どの変数がどのパラメータかを書く。
- **グローバル出力パラメータを読むのは、値を計算するテンプレートの出力の式だけにした。** Argo は、ステップの `when` と引数を、そのステップが動いているあいだ、ワークフローを見に来るたびに評価し直す。また、ステップを並べたテンプレートは、グローバル出力パラメータを、そのテンプレートに来たときの値で持っていて、自分のステップがそのあと入れた値は見えない。コントローラのソース（v4.1.4 の `executeSteps` と `executeStepGroup`）でこの二つを確かめ、kind の上でも起こした。そこで、ステップが一つで、それが決して走らないテンプレートを作り、値はその出力の式で計算する。Argo はこの式を、そのテンプレートに来たときに一度だけ、それまでに入った値をすべて見て評価する。ほかのステップはこの出力を読む。出力は一度入ると変わらない。
  - 呼び出しの引数、`match` の行き先、ループの回数は、その直前にこの形のテンプレートで計算する。
  - 流れを止めうる文（呼び出し、`match`、ループ、`fail`・`succeed`・`break`）は出力 `go` を持つ。ブロックの中でそのあとに続く文は、`go` が `true` のときだけ走る。走らなかった文の `go` は既定の `false` になるので、止まった流れはそのまま止まっていく。
- **エラーは Argo の失敗にしない。** 呼び出しのステップは `continueOn` で先へ進み、次のテンプレートが、終わり方（ステップの状態、終了コード、error.json）からエラーの種類を決めて、`dd_ctl` と `dd_error` に書く。受ける `on` があれば、その本体が走る。流れが失敗で終わると、最後のコンテナが `dd_error` の名前と理由を出して失敗し、ワークフローを失敗させる。
- **タスクの `timeout` は Pod の `activeDeadlineSeconds` にする。** テンプレートの `timeout` は、Pod が始まる前に切れると、周りのステップグループを `continueOn` に関係なく失敗させる（取れないイメージの Pod で確かめた）。`activeDeadlineSeconds` なら、始まらなかった Pod も終了コード 137 の失敗になり、`timeout` として読める。
- `retry` は `retryStrategy` にする。やり直すエラーは `expression` で、終了コードと、コンテナが終了メッセージに残した宣言したエラーの名前から選ぶ。回数は Step Functions と同じだが、間隔の `backoff` は、実機では 10 秒ほどの刻みで待ったので、同じ秒数にはならない。
- `for` と `repeat` は、`withSequence` で回を並べ、`parallelism: 1` で一つずつ回す。再帰にしないのは、Argo が入れ子の深さを 100 までに限るからである。`break` のあとの回も並んでいるので、その回は始まりのテンプレートで止まり、本体には入らない。`for … in parallel` は `parallelism` を `k` にする。各回は自分の `dd_ctl`・`dd_error`・変数を持ち、yield した値か、どう失敗したかを出力に残す。回がすべて終わってから、その並び（リストの順）で最初の失敗を探す。
- `callback` は、ID を渡すステップのあとの suspend のステップで待つ。答える側は `argo node set` で待ちのステップの出力 `answer` に `{"ok": 値}` か `{"error": 名前, "message": …}` を入れ、`argo resume` で進める。どちらも `inputs.parameters.callback_id.value=<ID>` でステップを選ぶ。期限が来れば `answer` は既定の値になり、`timeout` として読む。Argo がやり直せるのは ID を渡すステップだけなので、`callback` のタスクの `retry` は E050。
- `workflow template` は、そのテンプレートから Workflow を作る resource のステップにする（`action: create` で作り、成功と失敗の条件で終わりを待つ）。答えは子の `dd_output`。子の失敗は `failure` としてしか届かないので、宣言したエラーは E050 にした。子を作るには、ワークフローのサービスアカウントに Workflow を作って見る権限が要る。
- `wait until` は、待つ秒数を計算するテンプレートで一度だけ時計を読み、その秒数で suspend する。suspend の `duration` に `now()` を書くと、見に来るたびに計算し直される（`executeTemplate` は、動いているノードでもテンプレートの引数を埋め直す）。待ちの期限は、始まった時刻にその秒数を足して決まるので、期限が逃げていく。
- 冪等キーは `ワークフロー名/呼び出しの場所/各ループの回数`。
- 一回の実行が作るノードの数は E040 で見積もる（3 章）。

### 4.5 pydantic-graph

- ワークフローの名前のパッケージに、`graph.py`（`State`、`Deps`、節点、`graph`）、`types.py`、`tasks.py`（`make_tasks(own, transport)`）、`io.py`、`rules.py`、`runtime.py` を出す。相手は pydantic-graph 2.x（2.51.0 で確かめた）で、グラフは `GraphBuilder` で組み、節点は `BaseNode` で書く。
- **文ごとに節点を一つ置く。** 節点の `run` の返り値の型に、次に行ける節点を全部書く。pydantic-graph はこの型から辺を作るので、`graph.render()` がそのまま `.flow` の流れ図になる。`match` は行き先ごとの節点へ分かれ、`on` のある呼び出しは受けたエラーの本体の最初の節点へ、ループの本体の終わりは次の回の節点（`S<場所>_again`）へ進む。
- 変数は `State`（dataclass）のフィールドで、値は JSON のままの `dict` と `list`。ループの回数（冪等キーに入る）も `State` に置く。
- 外への呼び出しは `Deps` を通す。`tasks`（dandori が書く `lambda`・`http`・`aws` の実装と、利用者の関数）、`rules`、`clock`、`callbacks`、`run_id`（冪等キーとコールバックの ID に入る、実行の名前）。利用者の関数は、宣言したエラーを `TaskFailure(kind, message)` で投げる。
- 再試行、答えの検査、`on failure` の意味は Temporal と同じで、書き方も Python 版とそろえた（`dd.attempt`）。待ちとやり直しの間隔は `Deps.clock` で眠り、タスクの `timeout` は `asyncio.wait_for` で測る。
- **並列の回は、回の本体を別のグラフにする。** 回ごとに `State` の写しを作ってそのグラフを走らせ、`k` 個ずつ並べる。回の中で受けていないエラーは回のグラフの外に出て、ループの節点が、全部の回が終わってから、リストの順で最初のものを扱う。回の本体を節点の並びのまま、回ごとの変数の写しで走らせられるので、この形にした。
- **実行はプロセスの中にしかない。** pydantic-graph 2.x は状態をどこにも残さない（1.x にあった `FileStatePersistence` などは無くなった）ので、プロセスが止まれば実行は失われる。何日もの `wait` やコールバックも書けるが、そのあいだプロセスが生きている必要がある。止まっても続く実行が要るなら、ほかの出力先を使う。

### 4.6 五つの出力先でそろえたこと

Temporal の列は、TypeScript 版と Python 版の両方のこと。

| こと | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| 再試行 | Retry | ワークフローのコード（同じ並び、同じ間隔） | ワークフローのコード（同じ並び、あいだは `context.wait`） | `retryStrategy`（`expression` でエラーを選ぶ） | グラフのコード（同じ並び、あいだは `Deps.clock`） |
| 答えの検査 | Choice と Fail | `is_<型>` と `dd.fail` | Temporal と同じ | 出力の式（`type()` と `matches`） | Temporal の Python 版と同じ |
| 冪等キー | `$states.context.Execution.Name` から | `workflowInfo().workflowId` から | `durableExecutionArn` から | `workflow.name` から | `Deps.run_id` から |
| `lambda`・`http`・`aws` のタスク | Task（Lambda、HTTP、AWS SDK 統合） | dandori が書くアクティビティ（`Transport` を通す） | dandori が書く実装を step の中で | dandori が書く実装をコンテナで | dandori が書く関数（`Transport` を通す） |
| `agent` のタスク | HTTP Task（Responses API か Messages API）、答えは `$parse` で読む | dandori が書くアクティビティ（`Transport` を通し、Agents SDK か Anthropic の SDK で） | dandori が書く実装を step の中で | dandori が書く実装をコンテナで | dandori が書く関数（`Transport` を通し、Agents SDK か Anthropic の SDK で） |
| それ以外のタスク | 作れない | 利用者のアクティビティ | 利用者の実装を step の中で | `image` のコンテナ（無ければ作れない） | 利用者の関数 |
| 子ワークフロー | 入れ子の実行 | `executeChild` | `context.invoke` | WorkflowTemplate から Workflow を作る | 利用者の関数 |
| コールバック | `.waitForTaskToken` | シグナル | `createCallback` | suspend と `argo node set` | `Deps.callbacks` |
| 並列の回 | INLINE の Map、回の答えで失敗を運ぶ | `dd.rounds` | `context.map`、回の答えで失敗を運ぶ | `withSequence` と `parallelism`、回の出力で失敗を運ぶ | 回ごとのグラフを、状態の写しで |
| 規則 | Lambda と、Python をつなぐコード | アクティビティ（`local` ならローカルアクティビティ）と、TypeScript か Python をつなぐコード | `context.invoke` と、Step Functions と同じ Lambda | コンテナと、TypeScript をつなぐコード | 関数と、Python をつなぐコード |
| on failure | Task の Catch | `TaskError` を受ける catch | Temporal と同じ | `dd_ctl` が `task` のときに走るステップ | `on failure` の本体の節点へ進む |
| 出来事（`event`） | 作れない（E050） | Update `dandori.event` を `condition` で待つ | 作れない（E050） | 作れない（E050、まだ） | 作れない（E050、まだ） |

実行中の案件と規則の版の関係は、どの出力先でも同じになる。規則は Lambda かアクティビティとして呼ぶので、規則を直すと、実行中の案件も途中から新しい版で判定される。

## 5. 確かめ方

- **参照インタプリタ**（`dandori run`）：シナリオ（入力と、呼び出しが受け取る答えの並び）に沿って型を付けた構文木を実行し、呼び出しを出力先から見える形（Lambda の関数と payload、HTTP のメソッドと URL とヘッダと本文、AWS の API と引数、エージェントの名前・提供元・モデル・指示・引数・Schema（Step Functions では Responses API か Messages API への HTTP の要求）、入れ子の実行・子ワークフロー・invoke の先と入力、Temporal のアクティビティと引数、durable functions の step と引数・コールバックの submit）で書き出す。並列の回は、リストの順に一つずつ回す。
- **シナリオの自動生成**（`dandori scenarios`）：選んだ答えの記録を持ってワークフローを実行し直す。選び方は、選択の場所ごとに、まだ使われていない選択肢を先に取る。何か新しい所に着いた実行を土台にして、その途中の選択を一つずつ変えた実行を試し、そこから先は同じ場所で同じ選択を繰り返す（ループが回り切る流れはこれで出る）。答えの検査に落ちる答え（型の合わない答え、ありえない状態、範囲の外の数を持つ答え）も入れる。Connect の答えには、既定値のフィールドを省いた答え（答えの中、中のメッセージ、メッセージのリストのどこでも）も入れ、どの出力先も既定値を埋めて読むことを確かめる。Claude のエージェントの答えには、列挙の値の大文字と小文字を変えた答え（どのリストにも項目が二つあり、無いことがある値もある形）も入れる。`on cancel` のあるワークフローでは、`on cancel` の外のどの呼び出しにも、答えの代わりにキャンセル（`{"cancel": true}`）を選べる。並列の回の一つが失敗したあとに別の回でキャンセルが届く実行も、別のところとして数える。リストの長さは `for` が読むときに選び、空・一つ・二つ・上限を一つ超える長さを試す。無いことがある値は `match` が読むときに、無い場合とある場合を選ぶ。二回以上回ったループと、一回だけのループを別のところとして数えるので、二つの回が済んでその先の終わりまで行く実行も残る。答えと入力の文字列と数は、一つの実行の中でどれも違う値にする（`id-5`、`id-6`、`1003` など）。同じ値ばかりだと、出力先が二つの答えや並列の回の結果を取り違えても、突き合わせに出ない。範囲のある数は範囲の中から選ぶ（範囲が狭ければ、同じ数になることもある）。
- **Step Functions の突き合わせ**：生成した ASL を `tools/asl-run.mjs` が JSONata 2.0.6 で実行し、参照インタプリタと同じ形で書き出す。Map の回は一つずつ回し、回の中で外の変数に値を入れたら失敗させる（Step Functions の規則）。全シナリオで一致を見る。定義は asl-validator 4.0.0 にも通す。呼び方を持たないタスクのあるワークフロー（Temporal と durable functions 向けのもの）は、E050 を表示して外す。
- **LocalStack の Step Functions の突き合わせ**：同じ ASL を、Docker で立てた LocalStack の Step Functions で `tools/localstack/run.mjs` が実行する。版は 4.14.0 で、アカウントなしで立つコミュニティ版の最後の版である（これより後の版は認証トークンが無いと起動しない）。JSONata は dashjoin の Java 版（0.9.7）で、`tools/asl-run.mjs` の JavaScript 版とは別の実装になる。
  - 呼び出しの答えは、LocalStack のモックの仕組み（Step Functions Local と同じ形のモックの設定ファイル、`SFN_MOCK_CONFIG`）で返す。シナリオを一つのテストケースにし、Task ステートごとに、受け取る答えを順に並べる。どのステートがどの答えを受け取るかは、同じシナリオを `tools/asl-run.mjs` で流して決める。LocalStack が別の道を通れば、答えが足りなくなるか別の答えを受け取るので、食い違いとして出る。
  - 全部のフローの全部の実行を同時に流し、呼び出し・待ち・終わり方を、実行の履歴から読む。
  - 定義は次の点だけ変える。Map の回は一つずつ回す（MaxConcurrency を 1 にする。AWS の Step Functions Local の文書もモックではそう勧める）。Wait はすぐ進め（Seconds を 0 にする）、待つはずだった長さ（Seconds か Timestamp の値）を自分だけの変数に入れて、履歴から読む。Retry の待ちは本当に待ち、履歴の時刻で測る。測った待ちは、参照より短くなく（履歴の時計の 50 ms は除く）、長すぎる分が 0.9 秒か参照の四分の一の大きいほうより小さければ、同じと見る。負荷の高い機械では待ちが延び、倍率が一段ずれた待ち（倍か半分）は通らない。間隔そのものは、`tools/asl-run.mjs` が定義から秒まで読んで確かめる。呼び出しと終わり方が参照と同じで、Retry の待ちだけがそれより長かった実行は、新しいコンテナでもう一度だけ流す。テスト全体の始めは機械が混み、LocalStack のスレッドが 1 秒遅れて起きることがある（Argo のランナーが Pod を走らせ直すのと同じ考え）。
  - モックの設定ファイルからは、名前が `States.` で始まるエラーを投げられない（LocalStack はモックのエラーをタスク自身のエラーとして作り、その名前は `States.` で始められない）。タスクのタイムアウト（`States.Timeout`）、HTTP Task のエラー（`States.Http.StatusCode.<状態>`）、エージェントの失敗（下）は、名前の前に `Dandori.` を付けて投げ、Retry と Catch に書く名前もそう替え、終わり方の名前は元に戻す。定義は `States.TaskFailed`（タイムアウトとほかのエラーを分ける、ただ一つの名前）を使わないので、Retry と Catch の受け方は変わらない。
  - 4.14.0 は HTTP Task を知らず、それを持つ定義を断る。そのときは HTTP Task の resource を、引数を取らず答えに HTTP Task の鍵を持たない API の AWS SDK の統合（`arn:aws:states:::aws-sdk:sts:getCallerIdentity`）に替える。LocalStack はその引数を全部評価して履歴に残し、モックの答えもエラーもそのまま渡す。HTTP Task にいちばん近い API Gateway の統合は、どちらもしない（受け取らない引数を履歴から落とし、モックのエラーをどれも `ApiGateway.FailureEventException` にする）。
  - エージェントの失敗は、モデルの断りが Step Functions でその Task を失敗させるエラー（`States.QueryEvaluationError`）として投げる。断りを読む式は `$error` を起こし、Step Functions は式の評価の失敗を `States.QueryEvaluationError` にして Retry と Catch に渡す（4.1）。4.14.0 は、式の評価の失敗を（型のエラーでも）`States.Runtime` にして実行ごと失敗させ、Retry も Catch も受けないので、断りそのものはここでは演じない（`tools/asl-run.mjs` が演じる）。
  - Docker か、このイメージ（`docker pull localstack/localstack:4.14.0`）が無ければ SKIP にする。
- **Temporal の突き合わせ**：生成した TypeScript を `tools/temporal/run.mjs` が SDK 1.24.0 で、本物の Temporal サーバーの上で実行する。サーバーは、SDK のテスト用のパッケージが立てる Temporal CLI の dev server（1.9.1、Server 1.32.0）。
  - dandori が書くアクティビティは本物を走らせ、`Transport` だけを、送るものを書き出してシナリオどおりに答える代わり（`tools/transport.mjs`）に差し替える。だから Step Functions が送るものと一字ずつ比べられる。利用者のタスク、規則、子ワークフローは、シナリオどおりに答える代わりを置く（子ワークフローは、答えをアクティビティに聞く小さなワークフロー）。
  - ワーカーは生成した `worker.ts` の `workerOptions` で作り（規則と子ワークフローだけ代わりに差し替える）、`queue` ごとにも立てる。実行は `client.ts` の `start` で search attribute をオンにして始め、コールバックには `answer`（Update）で答える。答えるたびに、同じ答えをもう一度送って断られることを、初めの一度だけは、待っていない ID への答えが断られることも確かめる。実行が終わると、クエリ `dandori.status` の `cases` と search attribute `DandoriCases` を読み、参照インタプリタが終わりに持つ案件の状態と比べる。
  - シナリオはどれも同時に流し、それぞれ ID `run-<n>` のワークフローにする。ID は冪等キーに入るので、参照インタプリタも同じ名前で走らせる。代わりは、自分を呼んだアクティビティのワークフロー ID から、どの回の呼び出しかを知る（子ワークフローの ID は親の ID で始まる）。
  - サーバーは本当の時間で動くので、走らせるコードの写しでは待ちを縮める。タイマーの長さ（`dd.ms`）は 10 ms まで、アクティビティと子ワークフローの期限は 5 秒にする（はじめは 2 秒だったが、テスト全体を走らせて機械が混むと、答えを返すだけのアクティビティが 2 秒を超えて期限切れになった）。ただし、コールバックの ID を渡すアクティビティは、生成したとおりの期限のままにする。シナリオがそれを期限切れにすることは無く、その代わりは、コールバックに答えて二度目の答えが断られるのを確かめてから返るので、ワークフロータスクを何度か待つ。dev server は 10 ms のタイマーでも 1 秒遅れて発火させるので、`history.timerProcessorMaxTimeShift` を 10 ms にする（ホテルの予約の、29 回呼び出す一本が 26 秒から 4 秒になった）。
  - タイムアウトのシナリオでは、代わりが答えずにアクティビティをふさぎ、サーバーに期限切れにさせる。コールバックの答えは、ID を渡すアクティビティが返る前に送り終える（返ったあとに送ると、短くした待ちが先に切れる）。コールバックのタイムアウトでは送らない。ふさいだアクティビティは、ハートビートの応えで期限切れやキャンセルを知って終わる（テストのワーカーはハートビートを 100 ms ごとに送る）。
  - キャンセルのシナリオでは、代わりがサーバーにそのワークフローのキャンセルを頼み、ハートビートを送りながら、自分のアクティビティがキャンセルされるまで答えない。コールバックでは、答えの代わりにキャンセルを頼む。
  - 出来事は、ランナーが 20 ms ごとにクエリ `dandori.status` の `events` を見て、ワークフローが待っていれば、生成した `client` の `send` で送る（キャンセルのシナリオでは、送る代わりにキャンセルを頼む）。送る前に、ほかの出来事を送って断られることを確かめる。送った出来事の答えは、送る前に取っておく（ワークフローが受け取ると、次の呼び出しがすぐ来る）。クエリは、終わったばかりの待ちや、新しい実行に続く直前の実行のことを答えることがあるので、断られたり、閉じていく実行に当たったりした出来事は、答えを戻して送り直す。写しでは、出来事の待ちを 5 秒までにする（10 ms では、ランナーが待ちを見る前に期限が来る）。期限切れのシナリオでは送らず、次の呼び出しが来たとき（または実行が終わったとき）に、その出来事を期限切れとして書き出す。出来事のあるフローは、言語をまたぐアクティビティの確かめからは外す（出来事はワークフローに届くので、アクティビティの言語にかかわらない）。
  - 並列の回は一つずつ回す（`dd.atATime` を 1 に書き換える）。
  - `local` の規則を持つフローでは、履歴に、その規則のアクティビティのタスクが一つも無く、ローカルアクティビティのマーカー（`core_local_activity`）があることも確かめる。
  - 写しでは、履歴を一件目から長いと数える（`dd.CONTINUE_AT` を 1 に書き換える）。フローの一番外のループは、一つの実行の最初の回を除くどの回も新しい実行で始めるので、回の境目のどこで続けても、呼び出しと終わり方が参照インタプリタと同じになることを確かめられる。実行が終わると、最初の実行から、続きの実行を一つずつたどって履歴を取る。
  - フローごとに dev server を立て、全部のフローを同時に流す。ただし、Temporal の二つの突き合わせと durable functions の突き合わせ（これもフローを同時に流す）は交代で走らせる。どれもフローの数だけプロセスを立てるので、重なると互いを遅くし、横で走る Argo の突き合わせも遅くする。いちばん長い Argo の突き合わせは、交代を待たずに始める（待たせると、テスト全体が 20 秒ほど延びた）。
- **Temporal（Python）の突き合わせ**：生成したパッケージを `tools/temporal-python/run.py` が SDK 1.33.0 で、同じ dev server の上で実行する（ワークフローは SDK のサンドボックスの中で動く）。やり方は TypeScript 版と同じで、`Transport` の差し替え（`tools/transport.py`）も、利用者のタスク・規則・子ワークフローの代わりも、待ちの縮め方も、生成した `worker.py` と `client.py` を通すことも、クエリと search attribute を確かめることも、そろえてある。
- **言語をまたぐアクティビティ**：同じフローの TypeScript 版と Python 版を作り、一方のワークフローを、もう一方のアクティビティで走らせる。ワークフローの側のランナーは、dev server を立て、ワークフローと子ワークフローの代わりだけを持つワーカーを動かす。アクティビティは、もう一方の言語のランナーが同じサーバーにつないで受け持つ（`--serve`。dandori が書くアクティビティ、利用者のタスクと規則と子ワークフローの答えの代わり、差し替えの `Transport`）。コールバックの答えとキャンセルの依頼も、アクティビティの側の言語の `client` から送る。呼び出しは、アクティビティの側が書き出したものを、終わってから受け取って参照インタプリタと比べる。`local` の規則を持つフローは外す（ローカルアクティビティはワークフローのワーカーの中で走るので、ワークフローと同じ言語になる）。
- **Temporal の再生**：ランナーは、実行が終わるたびにその履歴（続きの実行があればそれぞれの）を取り、生成した `replay` で同じコードにかけて、非決定的と言われないことを確かめる。生成した `histories` がワークフローの型で全部の実行（続きの実行も）を見つけることも確かめる。`tests/histories` には、ホテルの予約・引当と発送・審査・倉庫の出荷の Temporal 向けの版と cancel の五つのフローについて、呼び出しのいちばん多い一本の履歴を TypeScript と Python で一つずつ置き、いま生成したコードで再生する。倉庫の出荷の一本は三つの実行に、引当と発送の一本は二つの実行に続いたもので、どれも置く（倉庫の出荷のファイルは `run-45.json`・`run-45.2.json`・`run-45.3.json`）。履歴を置くときは、記録した機械の名前とパス（ワーカーの名前、スティッキーなタスクキュー、スタックトレースの中のこのリポジトリと一時ディレクトリの場所）を中立の値（`@localhost`、`/work/dandori/`、`/tmp/`）に置き換える。再生はどれも読まない。生成器を直して、同じ `.flow` の走っている実行を再生できなくなると、ここで分かる（そうするしかない変更は `DANDORI_BLESS=1` で記録し直し、版を上げるか Worker Deployment Versioning で出すよう書く）。
  - 引当と発送の子を `flow`（1.9）にしたときには、その一本が再生できなくなった。子ワークフローの型が `arrange_delivery` から子の `.flow` の名前と版の `arrange_delivery_v1` に変わったからで、再生は `Child workflow type of scheduled event 'arrange_delivery' does not match child workflow type of command 'arrange_delivery_v1'` と言った。子の型やキューを変えるのも、走っている実行にとって互換でない変更なので、版を上げるか Worker Deployment Versioning で出す。記録は取り直した。
  - 範囲（1.3）を足したときには、ホテルの予約の一本が再生できなくなった。記録した実行の入力の泊数（1002）が、例に書き足した範囲 `>=1 <=30` の外で、規則の答えの額（1004 円）も、rulec の範囲（12,000〜1,200,000 円）の外だったからである。新しいコードは入力を `Dandori.BadInput` で断り、履歴にあるアクティビティの予定と食い違う。後者はシナリオが作った答えで、本物の規則は返さない（rulec が範囲を証明している）ので、規則の答えを確かめるようにしたことは、範囲を書いていないフローの走っている実行を変えない。前者は本物の実行でも起こる。範囲を足したり狭めたりすると、走っている実行のうち値がその外にあるものは、再生で失敗に変わる。だから、その変更は版を上げるか Worker Deployment Versioning で出す。記録は取り直した。
- **子の `.flow` の突き合わせ**：`tests/children` の受付（親）と査定（子）を、dandori が書いた二つのワークフローのまま、一つの dev server の上で走らせる（`tools/temporal/children.mjs`、`tools/temporal-python/children.py`）。親と子のワーカーは、生成した `worker` でそれぞれのタスクキューに立て、子の代わりは置かない。TypeScript どうし、Python どうし、TypeScript の親と Python の子、その逆の四通りで、申込の区分ごとに一本ずつ走らせ、終わり方を参照インタプリタと比べる。参照インタプリタは、親が渡す入力で子を走らせて子の終わり方を求め、それを親の呼び出しの答えとして親を走らせる。
- **Worker Deployment Versioning**：`tests/versions/approvals.flow`（一番外のループの回ごとに承認を待ってから知らせる）から、通知の文面だけが違う二つのビルド A と B を作り、生成した `worker` で一つのデプロイの版にして dev server で動かす。二回目の回は新しい実行で始まる（写しの `CONTINUE_AT` を 1 にする）。A で始まって承認を待つあいだに B を current にしても、その実行は、新しい実行で始まった二回目の回も含めて A のコードで終わり、そのあと始めた実行は B のコードで走ることを確かめる。続けるときに `AUTO_UPGRADE` を付けると、二回目の回が B のコードで走り、このテストが落ちる（TypeScript と Python の両方で、埋めて確かめた）。ワーカーはワークフローをキャッシュしない（キャッシュにいたから A に残った、ということが無いように）。同じコードのビルド ID が同じで、違うコードのビルド ID が違うことも確かめる。
- **生成した TypeScript の型**：Temporal、durable functions、Argo の caller に出す TypeScript を、rulec が生成する規則のモジュールと一緒に、`tsc --strict`（TypeScript 7.0.2）に通す。既定の `Transport` が使うときに読み込むパッケージ（AWS SDK のクライアント、エージェントの SDK）は、何の型でもよいモジュールとして宣言する。
- **pydantic-graph の突き合わせ**：生成したパッケージを `tools/pydantic-graph/run.py` が pydantic-graph 2.51.0 で実行する。`Transport` の差し替えは Temporal の Python 版と同じもの（`tools/transport.py`）。利用者のタスクと規則はシナリオどおりに答える代わりを `Deps` に渡し、時計は眠らずに時刻を進めるものに替える。コールバックの答えは `Deps.callbacks` に入れ、タイムアウトのシナリオでは入れずに、待ちを時計で切れさせる。タスクのタイムアウトは起こせないので、それを含むシナリオは外す。並列の回は一つずつ回す（`dd.at_a_time` を 1 に書き換える）。
- **Lambda durable functions の突き合わせ**：生成した TypeScript を `tools/durable/run.mjs` が SDK 2.4.0 のローカルのテストランナー（`@aws/durable-execution-sdk-js-testing` 1.1.4、時間を飛ばす）で実行する。`Transport` の差し替えは Temporal と同じ。規則は `registerFunction`、`durable function` の先は `registerDurableFunction` で登録した代わりで答え、後者は失敗するとき `ErrorData` を入れる。コールバックはその ID に答えを送って置き換える。このランナーは答えの来ないコールバックを期限切れにしないので、タイムアウトを含むシナリオは外す。
- **Argo Workflows の突き合わせ**：生成した WorkflowTemplate を、kind の上の Argo Workflows v4.1.4（`tools/argo/setup.sh` が用意する）で `tools/argo/run.mjs` が実行する。コントローラーは本物を使い、Pod はランナーが演じる。
  - 演じる回の Pod は、クラスタに無いスケジューラーを待つように作らせる。
  - ランナーは、その Pod のコンテナと Argo の executor がするはずのことを代わりにする。Pod に書かれた呼び出しで、生成した `caller/call.ts`（差し替えの `Transport` を持たせる）か、`image` のタスク・規則・`workflow template` の先の代わり（`tools/argo/stand-in.mjs`）を手元で走らせる。テンプレートが宣言した出力は WorkflowTaskResult に書き、コンテナが残すはずの終了コードと終了メッセージで Pod を終わらせる。
  - `workflow template` の resource のステップは、子の Workflow を作って終わりを待つ。
  - コンテナを立てないので、全部のフローの全部の実行を同時に流せる。
  - 演じる回の Workflow は、テンプレートの中身をそのまま持つ。`workflowTemplateRef` だと、コントローラーは各 Workflow の最初の検証で、テンプレートを解決するたびに WorkflowTemplate を API サーバーから読み直す。大きなテンプレートで百本を同時に流すと、API サーバーが詰まった。
  - フローごとに一本（宣言したエラーを通るものがあればそれ）は、本物の Pod でも走らせる。`argo submit --from` と同じく WorkflowTemplate から作り、代わりのコンテナがクラスタの中のモック（`tools/argo/mock.mjs`）に答えを聞く。
  - コールバックの答えは `argo node set` と `argo resume` で渡し、タイムアウトのシナリオでは、期限が来たときと同じ値を渡す。待ちは長さを 0 にしてすぐ進める。
  - タスクのタイムアウトは起こせないので、それを含むシナリオは外す。並列の回は一つずつ回す（`withSequence` を持つテンプレートの `parallelism` を 1 にする）。
- **エージェントの突き合わせ**：`Transport` の差し替えは、エージェントの呼び出しを `Transport` に渡された形（名前・モデル・指示・引数・Schema）のまま書き出し、シナリオの値を `{"answer": …}` に包んで返す。失敗は投げる（Agents SDK が断りを投げるのと同じ）。ASL のランナーは、Responses API への HTTP Task に API の答えの形（推論の項目と、`output_text` を持つメッセージ）で答え、失敗は、モデルが断った答え（`output_text` の無いメッセージ）で返す。Messages API への HTTP Task には、思考のブロックと `text` のブロックを持ち `stop_reason` が `end_turn` の答えで答え、失敗は `stop_reason` が `refusal` の答えで返す。ステートマシンがそれを読む式で Task が失敗し、Retry と Catch に渡るところまでを確かめる。ランナーは、Task の Assign と Output の式の失敗を、Step Functions と同じく Retry と Catch に渡す。
- **既定の `Transport` のエージェント**：TypeScript の既定の実装（Temporal・durable functions・Argo の caller が同じ `io.ts` を使う）と Python の既定の実装（Temporal と pydantic-graph が同じ `io.py` を使う）を、OpenAI の Agents SDK（TypeScript 0.18.0、Python 0.22.3）で走らせる（`tools/agents`）。モデルには、OpenAI の代わりに、答えを前もって決めた SDK のテスト用のモデル（`ScriptedModel`）を渡すので、OpenAI には何も送らない。シナリオのエージェントの呼び出しすべてについて、モデルが受け取ったもの（モデルの名前・指示・入力の文字列・答えの Schema）が Step Functions の同じ呼び出しで送るものと同じで、モデルの設定もツールも空であること、答えがそのまま返ること、断りが `ModelRefusalError` になることを確かめる。また、`Transport` が作るクライアントを手元の Responses API の代わりに向け、500 が返ると一回だけ送って `InternalServerError` で失敗することを確かめる。Claude のエージェントは、Anthropic の SDK（TypeScript 0.128.0、Python 1.8.0）で、手元（127.0.0.1）に立てた Messages API の代わりに送らせる。代わりが受け取った要求（パス、`anthropic-version`、本文）が Step Functions の同じ呼び出しで送るものと同じで、一回だけであること、答えがそのまま返ること、断り（`stop_reason: refusal`）で `AgentStopped` を投げること、500 が返るとやり直さずに `InternalServerError` で失敗することを確かめる。Anthropic には何も送らない。Open Responses のほかのサーバーのエージェント（`url`）は、送り先のパスはそのままに、手元に立てたそのサーバーの代わりに送らせる。代わりが受け取った要求（メソッド、パス、本文）が Step Functions の同じ呼び出しで送るものと同じで、一回だけであること、答えがそのまま返ること、断りで `AgentStopped`、500 でやり直さずに `AgentHttpError` になることを確かめる。
- **本物の Open Responses のサーバー**：この機械で Ollama が動いていれば（`DANDORI_OLLAMA`、既定は `http://127.0.0.1:11434`）、`url` のあるエージェントごとに、シナリオで答えのある最初の呼び出しを、モデルだけを Ollama にあるもの（`DANDORI_OLLAMA_MODEL`、無ければいちばん小さいもの）に替えて、TypeScript と Python の既定の `Transport` から本当に送る。答えは、タスクの型に合わなければならない。Ollama が無ければ SKIP にする。テストの前に読み込まれていなかったモデルは、終わったら下ろす（Ollama は読み込んだモデルを 5 分置き、8B のモデルでは 8 GB を超える。テスト全体の途中でメモリが逼迫した）。
- **既定の `Transport`**：シナリオの呼び出しのうち `Transport` を通るもの（HTTP、Lambda、AWS の API）を、TypeScript の既定の実装（`fetch` と AWS SDK for JavaScript v3）と Python の既定の実装（標準ライブラリと boto3）から、手元の代わりに送る（`tools/wire`）。手元のサーバーは、HTTP と Lambda の Invoke に、ランナーの差し替えの `Transport` がそのシナリオの答えで返すものになる応答を返す。HTTP のタスクの URL は、スキームとホストをこのサーバーに置き換える。AWS の API は、AWS の API を手元でまねる moto（5.2.3）に渡す。届いたもの（HTTP ならメソッド・パス・クエリ・ヘッダ・本文、Lambda なら関数と payload、SNS と SQS なら moto のキューに届いたメッセージ）が呼び出しと同じで、`Transport` が返すものが差し替えの `Transport` と同じで、二つの言語が同じ文字列を送ることを確かめる。AWS のエラーは、moto に起こさせられるもの（無いトピックへの `sns:publish` の `NotFoundException`、無いキューへの `sqs:sendMessage` の `QueueDoesNotExist`）だけを試す。何も手元の外には出ない。
- **つなぐコード**：Lambda の Python、`rules.ts`、Python の二つの出力先の `rules.py` を、rulec が生成した Python と TypeScript と一緒に動かし、`rulec vectors` の全件で rulec の期待値と比べる。durable functions 向けに出す規則の Lambda が、Step Functions 向けのものと一字も違わないことも確かめる。
- **診断**：`tests/fixtures` の各ファイルの診断を、英語と日本語の両方で固定する（`DANDORI_BLESS=1` で書き直す）。
- **文書**：README とサイト（`website/docs`、`website/docs-ja`）に載せたものが実物と食い違わないことを、`tests/docs.rs` で確かめる。診断の抜粋は、どれも `tests/fixtures` の golden ファイルにそのまま含まれていること。` ```flow ` のブロックの行は、どれも `examples` か `tests` の `.flow` にある行であること（`…` で省いた行は、残りの部分がその順で一つの行に含まれていること）。二つの言語の診断コードの一覧が、ソースにあるコードとちょうど一致し、コードの数を書いたページがその数を書いていること。サイトのハイライトに使う `KEYWORDS` が、`src/syntax.rs` のものと一字一句同じであること。
- 突き合わせるのは `examples` の例と、言語の端の振る舞いを通すための `tests/flows` のフロー。例は、出力先ごとの版をディレクトリに分けて置く。主な出力先である Temporal 向けの `temporal/`（HTTP は dandori が書くアクティビティ、ほかは利用者が書くアクティビティ、外からの知らせは `event`、ほかのタスクキュー、`on cancel`、`local`）、Step Functions と Lambda durable functions に向けた `aws/`（Lambda、EventBridge の接続を通す HTTP、SNS と SQS を、Step Functions と同じに呼ぶ）、`pydantic-graph/`（プロセスの中の関数、規則もプロセスの中、コールバックも同じプロセスで答える）の三つで、審査には Argo Workflows 向けの `argo/`（タスクは利用者のイメージのコンテナ）もある。審査の `aws/` は、どのタスクも利用者が書くコードなので、Lambda durable functions だけが走らせる（Step Functions では E050）。例のディレクトリのすぐ下には、どの出力先でもそのまま動くものだけを置く（引当と発送の三つの版が子として走らせる `arrange_delivery.flow`）。`rules/` と `specs/` は版が共に読む。
- `temporal/`・`pydantic-graph/`・`argo/` の版は、その出力先のランナーだけで走らせる。`aws/` の版とすぐ下のフローは、どの出力先でも走らせる。`aws/` の版の Lambda・HTTP・AWS の呼び出しは、dandori がどの出力先にも書くので、ほかの出力先での確かめはこれで足りる。例は英語で書き、`tests/flows` のフローとそれが読む `tests/rules` の規則は日本語の名前で書いてあるので、日本語の名前が五つの出力先でそのまま識別子や鍵になることは、こちらで確かめる（`timeouts.flow` は、HTTP のパスにも日本語の値を入れる）。

`Transport` の既定の実装は、突き合わせでは走らない（差し替える）。突き合わせが確かめているのは、dandori が書く実装が何を送り、答えをどう読むかで、既定の実装が本当に送るところは、上の既定の `Transport` の確かめで、手元の代わりに向けて確かめる。本物の Stripe や AWS に向けて送るところは確かめていない。エージェントの既定の実装は、上のとおり OpenAI では Agents SDK の中まで、Claude では SDK が HTTP で送るところまで走らせるが、OpenAI と Anthropic の API が Schema を受け付けて答えるところは確かめていない。

### 5.1 走らせた（2026-09-26）

- 例は五つで、それぞれ Temporal 向け・AWS 向け・pydantic-graph 向けの三通り（審査は Argo 向けも。5 章）。ホテルの予約（Stripe の PaymentIntent。Stripe の OpenAPI の記述に合わせる）、倉庫の注文の出荷（rulec の規則 `order_state`）、注文の引当と発送（リスト、並列の回、無いことがある値、`json`、Connect で呼ぶ倉庫、Smithy のモデルに合わせた SNS と SQS、SQS のコールバック、子ワークフロー。子は dandori で書いた `arrange_delivery.flow` で、Temporal 向けの版は、子が翌日便の車を取れなかったとき（子の `fail NoVan`）に通常便で頼み直す）、問い合わせの振り分け（エージェント二つと規則一つ。読み取るのは OpenAI、下書きするのは Claude のエージェントで、読み取りに失敗すれば総合の窓口に起票し、下書きに失敗すれば下書きなしで起票する）、申し込みの審査（利用者が書くタスクだけで、利用者のコールバック。Temporal 向けは `queue`、Argo 向けは `image`、AWS 向けは durable functions だけが走らせる）。ほかに `tests/flows/edges.flow` が、配列の `json` をリストに入れること、並列の中の並列、理由の無い `fail` を通り、`tests/flows/agents.flow` が、リスト・単位・時刻・無いことがある値・範囲のある数を持つエージェントの答え、答えを読まないエージェントの呼び出し、並列の回の中のエージェントとそのやり直し、列挙の値を大文字と小文字を変えて答える Claude のエージェントを通り、`tests/flows/timeouts.flow` が、期限切れを受ける呼び出し、期限切れだけをやり直す呼び出し、`timeout` を書かないタスクの期限切れを通り、`tests/flows/local_rules.flow` が、`local` の規則（期限切れを受けてタスクを呼ぶ）と、同じ規則をふつうのアクティビティで呼ぶことを通り、`tests/flows/events.flow` が、`event` のタスク（承認の出来事とその断りと期限切れ、一番外のループの回ごとに待つ、案件の状態を知らせる配達の出来事）を通る（Temporal だけ）。`tests/flows/connect.flow` は、Connect の答えが省く既定値（空の文字列、0、false、列挙の最初の値、空のリストとマップ、文字列の 64 ビットの整数）を、答えの中、中のメッセージ、メッセージのリストで埋めて読み、次の呼び出しに渡す。
- シナリオは 50 本・38 本・20 本・13 本・10 本と、引当と発送の子の 9 本、connect の 7 本、edges の 12 本、agents の 17 本、timeouts の 7 本、local_rules の 12 本、events の 12 本。Step Functions では、審査と events を除く 185 本すべてが参照インタプリタと一致した（LocalStack の上でも同じ 185 本が一致した。下の項）。Temporal では、dev server の上で、`on cancel` を持つ cancel の 32 本、events の 12 本、例の Temporal 向けの版の 161 本（ホテルの予約と倉庫の出荷は、`on cancel` のキャンセルのシナリオを加えて 62 本と 49 本、引当と発送は子の `NoVan` を受けるところを加えて 27 本）を加えた 400 本すべてが一致した（アクティビティのタイムアウトはサーバーが期限切れにし、コールバックのタイムアウトは答えを送らないことで起こせるので、外したものは無い）。Temporal の Python 版でも、同じ 400 本すべてが一致した。どちらの言語でも、このうち 33 本（倉庫の出荷の 11 本と、その Temporal 向けの版の 13 本、引当と発送とその Temporal 向けの版の 1 本ずつ、edges と local_rules の 2 本ずつ、events の 3 本）が一番外のループの回で新しい実行に続き、続きの実行は合わせて 50 本だった（倉庫の出荷の 7 本と、その Temporal 向けの版の 8 本と、events の 2 本は、三つの実行にまたがる）。Lambda durable functions では、タイムアウトを含む 17 本を除く 178 本すべてが一致した。pydantic-graph では、タスクのタイムアウトを含む 5 本を除く 321 本（例の pydantic-graph 向けの版の 131 本を含む）すべてが一致した。agents の 17 本のうち 2 本は、Claude のエージェントが列挙の値の大文字と小文字を変えて答える（どちらも `Normal` と、`urgent` と `Low` のリスト）。どの出力先も、それを宣言した値として読んだ。
- Argo Workflows（v4.1.4、kind の上）では、タスクのタイムアウトを含む 5 本を除く 190 本すべてが参照インタプリタと一致し、フローごとに本物の Pod で走らせ直した一本（十一本）も一致した。コールバックのタイムアウトは、待ちの出力に期限切れのときの値を入れて起こした。一回の実行のノードの数は、多いもので、ホテルの予約が 567 個（E040 の見積もりは 939 個）、倉庫の出荷が 239 個（446 個）、edges が 295 個（593 個）、引当と発送が 229 個（4,746 個）、問い合わせが 94 個（153 個）、agents が 115 個（177 個）、審査が 74 個（136 個）、timeouts が 49 個（87 個）、local_rules が 117 個（272 個）、引当と発送の子が 44 個（76 個）で、どれも見積もりの中に収まった。テスト全体は 2 分前後で終わる（118〜142 秒。範囲を足したあとの二回は 126 秒と 136 秒、子の `.flow` を足したあとは 108〜138 秒、API の記述を足したあとは 175 秒、Open Responses を足したあとは 221 秒（ほかのプロセスで、機械の負荷の平均が 20 を超えていた）、LocalStack を足したあと全部が通った回は 131 秒。例の版を足す前は 96〜128 秒だった。Argo だけで 76〜88 秒、Temporal の突き合わせは TypeScript が 24 秒、Python が 17 秒、言語をまたぐアクティビティが 15 秒、durable functions が 16 秒。kind のノードは、何も走らせていなくても CPU を 3 割ほど使っていて、Argo の時間はそれに左右される）。
- 同じ日の昼、Argo の突き合わせがときどき食い違った。呼び出しは参照と同じで、終わり方だけが違い、その回の Pod が init の失敗（Error）や `Unknown (exit code 255)` で終わっていた。kind のノードの中の containerd（2.1.1）が SEGV で落ちては立ち上がり直していて、落ちた時刻が食い違いと重なった。落ちたときに動いていたコンテナは、どれもこう終わる。生成したワークフローは、その Pod の失敗を `failure` として正しく扱っていた。そこでランナーは、プラットフォームが動かせなかった Pod のある回を、名前を変えて二度まで走らせ直し、そのことを出力するようにした。いまはこれが本物の Pod の回にだけかかわる（次の項）。クラスタを kind 0.33.0 のノード（Kubernetes 1.37.0、containerd 2.3.4）で作り直してからは、containerd は一度も落ちていない。テスト全体を通したときも、ホテルの予約の 49 本すべてを本物の Pod で同時に走らせたとき（375 秒）も SEGV は無く、本物の Pod の 49 本は演じた回と一本残らず一致した。
- 生成器にわざと誤りを入れると（ASL の冪等キーの区切りを一文字変える、`match` の条件から値を一つ落とす、durable functions の冪等キーの呼び出しの場所をずらす）、どれも一本目のシナリオで食い違いとして出た（2026-09-25）。Argo の生成器でも、冪等キーの区切りを一文字変える、`match` の条件から値を一つ落とすと、どちらもホテルの予約の一本目で食い違いとして出た。Temporal の Python 版と pydantic-graph では、冪等キーの区切りを一文字変えると注文の引当と発送の四本目で、`match` の条件から値を一つ落とすとホテルの予約の一本目で出た（2026-09-26）。エージェントでも、ASL で答えを読まない呼び出しの `Output` を落とすと agents の二本目で、ASL で入力の引数の順を逆にすると問い合わせの一本目で、TypeScript で答えから `answer` を取り出さないと Temporal の問い合わせの一本目で、Python で別のタスクの Schema を渡すと pydantic-graph の問い合わせの一本目で、既定の `Transport` で Agents SDK に空の設定を渡さないと Agents SDK の確かめの一件目で、それぞれ食い違いとして出た（同日）。Claude のエージェントでは、答えの列挙の値を直す式を ASL から落とすと agents の七本目で、TypeScript の `io.fold` を呼ばないと Temporal の agents の七本目で、Python の `io.fold` で大文字と小文字を区別すると Temporal の Python 版の agents の七本目で、既定の `Transport` が送る `max_tokens` を一つ変えると確かめの四件目で、OpenAI のクライアントのやり直しを切らないと確かめの三件目で、それぞれ食い違いとして出た（同日）。
- 範囲（1.3）：シナリオには、範囲の外の数を持つ答えも入る（ホテルの予約では、与信額の規則が 11,999 円と答える 4 本目）。どの出力先も、それを `Dandori.BadResponse` で断り、参照インタプリタと一致した。生成器の範囲の比べをわざと落とすと、JSONata では、その答えを通して次の呼び出し（答えの用意されていない `create_intent`）に進み、TypeScript（Temporal）、Python（Temporal と pydantic-graph）、Argo の式では、終わり方の食い違いとして、どれもホテルの予約の 4 本目で出た（2026-09-26）。エージェントの答えの Schema の範囲（OpenAI の `minimum` と `maximum`、Claude の説明）は、既定の `Transport` の確かめで、Agents SDK からも Anthropic の SDK からも、Step Functions と同じ形で送られた。
- Open Responses（1.7）：既定の `Transport` の確かめでは、TypeScript と Python のどちらでも、Open Responses のエージェントの 24 件（問い合わせの Temporal 向けの版の 11 件と、agents の 13 件）で、手元の代わりのサーバーが受け取った要求が Step Functions の本文と同じで、一回だけだった。断りは `AgentStopped`、500 は `AgentHttpError` になった。手元の Ollama 0.32.13（Llama 3.1 Swallow 8B Instruct の 4 ビット、4.9 GB）に本物の呼び出しを送ると、二つのエージェント（問い合わせを読む、領収書を読む）の答えが、TypeScript と Python のどちらからも型に合った。領収書の答えには、時刻の正規表現、金額の範囲、列挙、null のことがある値が入る。`Transport` が送る指示に空白を一つ足すと（TypeScript）、答えを読むところをずらすと（Python）、どちらも agents の 8 件目で食い違いとして出た（2026-09-27）。
- LocalStack（4.14.0）の Step Functions では、185 本すべてが参照インタプリタと一致した（2026-09-27）。HTTP Task を持つ八つのフローの 26 の HTTP Task は、resource を替えて流した。テストは単独で 55 秒前後かかり、そのほとんどは Retry の本当の待ちである（いちばん長い実行は、問い合わせの 45 秒）。測った Retry の待ちは、単独で流すと参照より最大 0.37 秒長かった。テスト全体の中では、1 秒のはずが 2.04 秒、10 秒のはずが 11 秒になるなど、1 秒ほど長いものがあったので、待ちだけが長かった実行は流し直すことにした（5 章）。最後にテスト全体を流したとき（131 秒、メモリの空きは最も少なくて 28%）は、185 本のうち 11 本を流し直し、どれも一致した。幅の中で最も長かった待ちは、参照より 2.055 秒長かった（長い待ちでは、幅は待ちの四分の一になる）。はじめのころは、Open Responses のテストが Ollama に読み込ませたモデル（8.4 GB）が、使い終わってからも 5 分残ってほかのテストと重なり、メモリが逼迫していた。同じころ、Temporal と Argo の突き合わせも落ちた（Temporal ではキャンセルで終わるはずの一本が失敗で終わり、言語をまたぐ突き合わせでは dev server が 5 秒で立たず、Argo では一本が 10 分を超え、別のときは kind の etcd が時間切れになった）。Ollama のテストは、使い終わったモデルを下ろすようにした（5 章）。
  - 作る途中で、生成器の誤りが一つ見つかった。出力を持たないフローの ASL は、終わりの Succeed に `Output` が無く、最後の呼び出しの答え（local_rules では Lambda の `{"Payload": null, "StatusCode": 200}`）が実行の出力になっていた。参照インタプリタとほかの出力先は null で終わる。`tools/asl-run.mjs` は、一番外の `Output` の無い Succeed を null で終えていたので、誤りが隠れていた。どちらも直した（4.1。ランナーは Step Functions と同じく入力を出す）。直したランナーは、直す前の生成器を local_rules の一本目で食い違いとして捕まえる。
  - LocalStack 4.14.0 について分かったこと（どれも 5 章の扱いにした）。HTTP Task を知らない（定義が `Unknown service 'http'` で断られる）。HTTP Task のエラーの名前（`States.Http.StatusCode.409`）も知らず、Retry と Catch に書くと定義が断られる。モックの設定ファイルから `States.Timeout` を投げると、実行が `States.Runtime`（`Custom Error Names MUST NOT begin with the prefix 'States.'`）で終わる。API Gateway の統合は、モックのエラーを `ApiGateway.FailureEventException`（Cause は空）にする。JSONata の式の評価の失敗は、`$error` でも `1 + 'a'` の型のエラーでも `States.Runtime`（Cause は `JSONataException(('UNKNOWN', …))`）になる。Step Functions の文書は、型のエラーを `States.QueryEvaluationError` になる例に挙げている。
  - 生成器にわざと誤りを入れると、LocalStack の突き合わせでは、冪等キーの区切りを一文字変えるとホテルの予約の一本目で、`match` の条件から値を一つ落とすと同じ一本目（`PaymentCanceled` で終わるはずが `Dandori.UnexpectedValue`）で、Task に書く Retry の BackoffRate を 2 から 3 にすると、待ちだけが長い七本を流し直したあと二本目（規則の呼び出しの二回目の待ちが、2 秒のはずが 3 秒）で、上の `Output` を戻すと local_rules の一本目で、どれも食い違いとして出た（2026-09-27）。
- API の記述（1.10）：ホテルの予約の三つの版は、Stripe の OpenAPI の記述（spec3.json、API の版 2026-09-30.endive）の PaymentIntent の五つの操作に、引当と発送は SNS と SQS の Smithy のモデルと倉庫の `.proto` に合い、`tests/fixtures/apis.flow` の食い違い（受け取らない引数、要る引数の不足、受け取らない列挙の値、答えうる値を持たない列挙、整数を `timestamp` で読む、null のことがあるフィールド、無い操作、無い例外、冪等トークンでない `key`、64 ビットの整数と小数、JSON の名前の違い、ストリームのメソッド）はどれも E016 になった。生成器から既定値の埋め方をわざと落とすと、TypeScript（Temporal）、Python（Temporal）、JSONata のどれでも、connect の 6 本目（既定値を省いた答え）で食い違いとして出た（2026-09-26）。JSONata の埋め方は、はじめ答えの null の項目を消していて、ほかの言語（null を残し、既定値のあるものだけ埋める）と違ったので、そろえた。
- 子の `.flow`（1.9）：`tests/children` の受付と査定は、TypeScript どうし、Python どうし、TypeScript の親と Python の子、その逆の四通りのどれでも、三本（個人・法人・団体の申込）すべてが参照インタプリタと一致した。法人の申込では、子の `fail 要確認` が、親の宣言したエラーとして `on 要確認` に届き、団体の申込では、親が宣言していない `対象外` が `failure` として届いた。親が子ワークフローの失敗から子の `fail` の名前を読むところを、TypeScript と Python で一つずつ落とすと、どちらも同じ言語どうしの二本目（法人）で食い違いとして出た（2026-09-26）。子の `arrange_delivery.flow` を例に足したあと、Argo の突き合わせが一度、子の本物の Pod の回で `failed to resolve {{workflow.labels.dandori-parent}}` と食い違った。Argo のテストはすべてのフローを一つのクラスタで同時に走らせる。引当と発送のランナーが子の代わりに置く WorkflowTemplate と、子のフロー自身の WorkflowTemplate が同じ名前（`arrange-delivery`）で、先に置いたほうを上書きしていた。ランナーは子の代わりに自分の名前を付け、親のテンプレートの参照もそれに書き換えるようにした（生成したテンプレートは変えていない）。
- 文書の確かめ（5 章）：日本語のページの診断の一行を書き換える、英語の診断コードの一覧のコードを一つ別のものにする、ハイライトの `KEYWORDS` から一語を落とす、ページの `.flow` の一行の値を変える、README に書いたコードの数を変える、のどれでも `tests/docs.rs` が落ちた（2026-09-27）。
- `on cancel` を持つ `tests/flows/cancel.flow` の 32 本（うち 13 本でキャンセルが届く）は、Temporal の TypeScript と Python の dev server の上で、すべて参照インタプリタと一致した（2026-09-26）。生成器にわざと誤りを入れると、`dd.attempt` がキャンセルをタスクのエラーに直すと九本目で、`on cancel` を `CancellationScope.nonCancellable` の外で走らせると十一本目で、Python の `dd.attempt` がキャンセルをタスクのエラーに直すと九本目で、`dd.rounds` がキャンセルより前の回の失敗を先に投げると 23 本目で、食い違いとして出た。最後の誤りは、はじめ捕まらなかった。回が失敗したあとに別の回でキャンセルが届く実行がシナリオに無かったので、それを別のところとして数えるようにした。
- `event` の生成にわざと誤りを入れると、validator が待っていない出来事も受け取るようにした TypeScript と Python は、ランナーが送ったほかの出来事を受け取ったことで止まった。Python で出来事の待ちの期限切れを答えなしとして通すと events の一本目で、TypeScript で出来事が送ってきたエラーを値として読むと events の二本目で、食い違いとして出た（2026-09-26）。ランナーを作る途中では、出来事の待ちを写しで 10 ms に縮めていてランナーが送る前に期限が来たこと、同じ名前の出来事を次の回ですぐに待つので「待ちが終わるまで待つ」ランナーが止まったこと、送った出来事の答えを書き留める前に次のアクティビティがその答えを取ったことを、順に直した。
- Continue-As-New の生成にわざと誤りを入れると、TypeScript と Python のどちらでも、続きの実行が回を 0 から数え直すと倉庫の出荷の 15 本目で、ループより前の文を飛ばさないと同じ 15 本目で、変数を受け取らないと一本目で、クエリが答える案件の状態が参照と違うこととして出た。`yield` した値を渡さないと、edges の 10 本目で呼び出しの食い違いとして出た。続ける回を一つずらすと、残した倉庫の出荷の履歴の二つ目の実行が、再生で非決定的と言われた（2026-09-26）。
- Temporal の突き合わせは、生成した `worker` と `client` を通しても、TypeScript と Python の両方で 400 本すべてが一致し、どの実行でも、終わりのクエリ `dandori.status` と search attribute `DandoriCases` が参照インタプリタの案件の状態と同じだった。コールバックへの二度目の答えと、待っていない ID への答えは、Update がすべて断った（2026-09-26）。
- 既定の `Transport` では、シナリオの呼び出しのうち 456 件（HTTP が 408 件で、うち 146 件はエラーのステータス、Connect の呼び出しは 50 件でうち 17 件がエラー、Lambda が 45 件で、うち 18 件は関数のエラー、SNS と SQS が 3 件。例の三通りの版のものを含む）を、TypeScript と Python の両方から手元の代わりに送り、すべて呼び出しのとおりに届き、差し替えの `Transport` と同じものが返った（2026-09-26）。はじめ、Python の既定の実装が三か所で食い違った。
  - AWS のエラーを、線の上の符号（`NotFound`、`AWS.SimpleQueueService.NonExistentQueue`）で返していた。TypeScript の SDK と、タスクが宣言する名前は、API のモデルの名前（`NotFoundException`、`QueueDoesNotExist`）である。boto3 が投げる例外のクラスの名前がモデルの名前なので、それを返すようにした。
  - URL のパスに ASCII でない文字があると（例を英語にする前の倉庫の出荷の `/v1/orders/注文ID-1`。いまは `tests/flows/timeouts.flow` の `/v1/items/品番-2/check` で確かめる）、urllib が `UnicodeEncodeError` で送れなかった。fetch と同じく、UTF-8 でパーセントエンコードしてから送るようにした。
  - JSON の本文（HTTP の本文、Lambda の payload、SQS のメッセージ）を、`json.dumps` の既定のまま、空白を入れ、日本語を `\u` でエスケープして書いていた。値は同じだが、TypeScript と Step Functions の書き方（空白なし、文字はそのまま）にそろえた。
- 言語をまたぐアクティビティでは、TypeScript のワークフローと Python のアクティビティ、Python のワークフローと TypeScript のアクティビティの両方で、`local` の規則や `event` を持つフローを除く 252 本（例の Temporal 向けの版の引当と発送と審査を含む）すべてが参照インタプリタと一致した（コールバックの答え、キャンセル、タイムアウト、エージェントの呼び出しも、もう一方の言語を通った）。Python のアクティビティの名前に一字足すと、TypeScript のワークフローの側だけが食い違いとして出た（2026-09-26）。
- Temporal の再生は、TypeScript と Python の両方で、400 本すべて（続きの実行も）が同じコードで再生でき、`tests/histories` の十六の履歴も再生できた。Worker Deployment Versioning では、A で始まった実行が B を current にしたあとも、新しい実行で始まった二回目の回まで A のコードで終わり、B で始めた実行は B のコードで走った（TypeScript と Python。ビルド ID は、TypeScript が `dandori-c98507328717f2f4` と `dandori-5fe7c15ba38debcf`）。生成するワーカーの既定の振る舞いを PINNED から AUTO_UPGRADE にすると、どちらの言語でも A の実行が B のコードで終わり、食い違いとして出た（2026-09-26）。
- TypeScript の実行の履歴を Python のコードで再生すると、cancel の 32 本のうち 27 本が再生でき、Python の履歴を TypeScript で再生すると 31 本が再生できた。再生できなかったのは、キャンセルやタイムアウトやコールバックの待ちを通る実行で、タイマーとアクティビティの順か番号が二つの言語で違っていた。同じ言語の中では決定的なので、どちらかの言語のワーカーだけで一つのワークフローを持つかぎり問題は無い。二つの言語のワーカーを同じキューに混ぜると、こうした実行で止まる。
- 生成した TypeScript を `tsc --strict` に通すと、走らせても出なかった誤りが三つ見つかった。出力を持つフローの `run` の終わりの `return null`（出力の型に合わない。検査がそこへ来ないと言うので、失敗を投げるようにした）、値を入れる前は null で宣言している変数の読み（`!` を付けた）、rulec の TypeScript の単位の付いた数（ブランドの付いた bigint で、`BigInt(…)` をそのまま渡せない。rulec 自身のランナーと同じく `as` で型を付けた）。直したあとは、25 のビルドすべてが通る。
- 並列の回の結果の並びを逆にする誤りは、はじめ、どの出力先の突き合わせにも出なかった。シナリオの答えの値がどの呼び出しでも同じで、二回以上回ってその先まで行く実行も残っていなかったからである。値を一つずつ変え、二回以上回ったループを別に数えるようにすると、edges の十二本目で出た。この変更で、TypeScript の Temporal のランナーが、コールバックの答えのシグナルを、テスト環境が待ちの時間を飛ばしたあとに送ることがあると分かった。生成器ではなくランナーの競争だったので、ランナーを直した（5 章）。
- Argo の突き合わせは、はじめ全部の実行を本物の Pod で走らせていて、テスト全体で 30 分余りかかった。次のように縮めた。
  - コントローラーは既定では、変化のあと 10 秒おいてワークフローを見直す（`DEFAULT_REQUEUE_TIME`。Argo 自身のテストは 1 秒にしている）。これを 1 秒にした。
  - それでも kind の上では、Pod を一つ立てて終わるまでに約 4 秒かかる。呼び出しが 29 回続くホテルの予約の一本は、単独でも 3 分余りかかった。そこで Pod をランナーが演じる形にした（5 章）。この一本は 48 秒になった。
  - 150 本を同時に流すと、まだ 2 分を超えた。API サーバーへの要求を数えると、ホテルの予約・倉庫の出荷・問い合わせの三つ（97 本）を流したあいだに、WorkflowTemplate の GET が 9,181 回あった（5 章の、最初の検証での読み直し）。演じる回の Workflow にテンプレートの中身を持たせると、七つのフローを流しても 500 回（本物の Pod の回の分）になった。
  - ほかに、Argo の Kubernetes のイベントを止め、Pod を作る速さと API を呼ぶ回数の上限を上げた（`setup.sh`）。
  - テスト全体が 85 秒になった（kind 0.33.0 のノードで作り直したあとは 71 秒）。コントローラーの待ちを 300 ms にすると、かえって遅くなった（ホテルの予約の一本で 48 秒が 57 秒）。
- 演じる形でも、生成器にわざと入れた誤りは捕まる。`match` の条件から値を一つ落とす誤りと、caller が宣言したエラーで終わるときの終了コードを 3 から 1 に変える誤りは、どちらもホテルの予約の九本目で食い違いとして出た（2026-09-26）。
- つなぐコードは、与信額の規則（`hold_amount`）のベクタ 24 件、急ぎの規則（`urgency`）のベクタ 16 件、振り分けの規則（`inquiry_routing`）のベクタ 8 件で、rulec の期待値と一致した。Python 版の `rules.py` も同じベクタで一致した。
- 既定の `Transport` のエージェントは、問い合わせの 16 件と agents の 44 件のどれでも、TypeScript と Python の両方で、Step Functions と同じものをモデルに渡した（Agents SDK は TypeScript 0.18.0 と Python 0.22.3、Anthropic の SDK は TypeScript 0.128.0 と Python 1.8.0）。Claude の件では、手元の Messages API の代わりが受け取った本文が Step Functions の本文と同じだった。500 を返すと、どちらの提供元も一回だけ送って失敗した。空の設定を渡さないと、どちらの Agents SDK も gpt-5.4-mini に `reasoning.effort: none` と `text.verbosity: low` を足す（走らせて確かめた）。OpenAI のクライアントのやり直しを切らないと、500 に三回送った。
- 検査が例を書く途中で見つけたこと。チェックアウトまで待つあいだに与信の期限が切れると capture が断られる。capture のあとは `processing` を経て `requires_payment_method` に戻ることがある。`requires_action` を見てから cancel するまでに、客が認証を済ませて期限が切れると、cancel も断られる。入金がないので注文の取消を頼むあいだに、客が自分で取り消していると、倉庫はその取消依頼を断る。

## 6. 捨てたもの

- **rulec に workflow の宣言を足すこと。** 0.2 のとおり。
- **JSONPath を出すこと。** 変数と JSONata があれば、ResultPath の組み替えなしにデータを運べる。
- **Temporal の再試行に任せること。** Temporal は、型を名指しして「これだけをやり直す」とは書けない（やり直さないものを名指しする）。Step Functions と同じ回数にならない。
- **`match` の最後の行き先を Default にすること。** 検査の外の値が、黙ってその行き先へ流れる。
- **条件式を dandori に入れること。** P1 のとおり。条件は rulec の表に書けば、網羅と重なりが rulec の証明書つきで閉じる。
- **値の組み立ても書けないこと。** はじめの P1 は、引数に変数・フィールド・リテラルしか書けず、レコードを組み立てるにも文字列に値を入れるにもタスクか規則が要った。判断にかかわらない操作まで外に出させる理由は無いので、組み立ては許した（0.1）。
- **Temporal で、規則をワークフローのコードで直接呼ぶこと。** 履歴は増えないが、判定の記録が残らず、規則を直すたびにワーカーの版の固定（Worker Deployment Versioning の `PINNED`）が要る。固定しないと、実行中のワークフローの再生が新しい規則で判定し直し、答えが変わった分岐で止まる。
- **Temporal で、規則をいつもローカルアクティビティにすること。** 往復と履歴は減るが、規則をワークフローと同じワーカーに置くことになり、規則だけを出し直せなくなる。ループの中で何度も判定するワークフローのために、`use rule` の下の `local` で選べるようにした（4.2）。
- **Temporal で、コールバックをアクティビティの非同期完了で受けること。** はじめはこの形だった。答える側がアクティビティのタスクトークンを持つ必要があり、待つあいだアクティビティが開いたままになる。シグナルなら、答える側はワークフローを指す ID だけを持てばよく、待ちは `condition` の期限で表せ、テスト環境で時間を飛ばしてタイムアウトまで確かめられる。
- **durable functions で、`lambda` のタスクを `context.invoke` で呼ぶこと。** 4.3 のとおり、宣言したエラーを見分けられない。step の中で Invoke を呼ぶ実装を dandori が書けば、利用者のコードも要らない。
- **durable functions で、step の `retryStrategy` にやり直しを任せること。** step だけなら関数一つで Step Functions と同じ並びを書けるが、`context.invoke` にはやり直しの設定が無い。規則の呼び出しはどのみちワークフローのコードでやり直すので、step もそれにそろえた。
- **Step Functions で、入れ子の実行の失敗を Cause から読み分けること。** 子の `fail` の名前は Cause の JSON の中にあると思われるが、その形を文書で確かめられなかった。確かめられない形に頼る代わりに、宣言したエラーを E050 にした。
- **並列の回の失敗を、先に起きた順で決めること。** 出力先ごとに並べ方も速さも違うので、同じシナリオでも結果が変わる。リストの順なら、どの出力先でも同じ失敗になる。
- **Argo で、ステップの `when` や引数からグローバル出力パラメータを読むこと。** はじめはこの形で、変数を読む式をそのまま `when` と引数に書いていた。4.4 のとおり、同じテンプレートの前のステップが入れた値が見えない。kind の上では、受けていないエラーのあとの文まで走った。動いているステップの `when` が途中で偽に変わると、そのステップが進まなくなることも、ソースで確かめた。
- **Argo で、変数を引数と出力で次のステップへ運ぶこと。** グローバル出力パラメータを使わずに済むが、すべての変数がすべてのノードの入力と出力に載り、Workflow の大きさ（1 MiB まで）を早く使い切る。グローバル出力パラメータなら、値は Workflow の出力に一つずつと、値を入れたノードにだけ載る。
- **Argo で、ループを再帰で回すこと。** 回の数だけ入れ子が深くなり、Argo の深さの上限 100 に当たる。
- **Argo で、タスクの期限をテンプレートの `timeout` にすること。** 4.4 のとおり、Pod が始まる前に切れると `continueOn` が効かない。
- **エージェントを出力先にすること**（流れそのものを Agents SDK や、OpenAI が走らせる Agents API に任せること）。次に何をするかをモデルが決めるので、流れを前もって決め、走らせる前に検査するという dandori の芯が消える。エージェントは流れの中の一手、つまりタスクの呼び方の一つにした（1.7）。
- **Claude のエージェントを、Claude Agent SDK で走らせること。** Agent SDK は、呼び出しのたびに Claude Code の CLI を子プロセスとして起動する。答えの形も、Messages API の構造化出力のように生成を Schema で縛るのではなく、あとから確かめて合わなければ言い直させる（上限まで合わなければ `error_max_structured_output_retries`）。一回の呼び出しでモデルを何度も呼ぶことがあり、Step Functions が一回で送る要求とはそろわない。Agent SDK の文書自身も、道具を使わない一回きりの問い合わせには API を直接使うよう案内している。dandori のエージェントは道具を持たず一往復で答えるので、Anthropic の SDK で Messages API を呼ぶことにした。道具を持たせて何手も動かすエージェントを足すときには、Agent SDK が候補になる。
- **Step Functions から、Claude を Bedrock の統合で呼ぶこと**（いまは）。IAM で認証でき、API キーを接続に置かずに済む。しかし Bedrock の構造化出力は、Claude の文書の注記では Opus 4.6・Sonnet 4.6・Sonnet 4.5・Opus 4.5・Haiku 4.5 までで、5 系のモデルが無い。
- **Step Functions などでも `on cancel` を通し、そこでは走らないとすること。** Step Functions の `StopExecution` は、Temporal のキャンセルではなく強制終了（terminate）にあたる。キャンセルが届かない出力先では、`on cancel` は決して走らないと言えなくもない。しかし、後始末を書いた人が、Step Functions で止めても片付くと思い込むおそれがある。出力先ごとに意味が変わるものは E050 で断ってきたのにそろえた。
- **`on cancel` の無いワークフローにも、キャンセルのシナリオを入れること。** 呼び出しの数だけシナリオが増え、しかもキャンセルを演じられるのは Temporal だけで、ほかの出力先では外すことになる。キャンセルをタスクのエラーと取り違えない、という生成物の性質は、`on cancel` を持つフローで確かめられる。
- **Temporal のワークフローの型を、版なしの名前にすること。** はじめはこの形だった。Worker Deployment Versioning に任せれば同じ名前のままでも古い実行は古いコードに留まるが、それにはサーバーの側の設定と、版の切り替えの操作が要る。型とキューに版を入れれば、何もしなくても新しい版と古い版が並んで動く。同じ版のままのコードの違いは、Worker Deployment Versioning か再生の確認で扱う（1 章の版と 5 章）。
- **コールバックをシグナルだけで受けること。** はじめはこの形だった。シグナルは送りっぱなしで、答えた側は、ワークフローが受け取ったかも、もう待っていないかも分からない。Update なら、受け取ったかどうかが返り、知らない ID や二度目の答えをその場で断れる。
- **search attribute をオンにしたことを、始めるときに空のリストを渡して伝えること。** 空のリストは、属性が無いのと区別できなかった（走らせて確かめた）。memo で伝える。
- **Temporal の突き合わせを、時間を飛ばすテスト環境で走らせること。** はじめはこの形だった。待ちの時間を飛ばせるので速いが、アクティビティを期限切れにできず（そのシナリオは外していた）、キャンセル、Update、Worker Versioning のような本物のサーバーの振る舞いも確かめられない。dev server の上に移すと、待ちを縮め、タイマーのずれを直したうえで、TypeScript と Python の両方とも十数秒で終わった。
- **Temporal で、`timeout` の無いタスクに一律 60 秒を付けること。** はじめはこの形だった。Step Functions では期限の無いタスクが、Temporal では 60 秒で切れる。長い処理を書いた利用者が、Temporal でだけ `timeout` に出会う。ワーカーが落ちたことには、ハートビートで気づけばよい。
- **Temporal で、Agents SDK との公式の統合を使うこと**（いまは）。4.2 のとおり、ツールを持たない一往復のエージェントには、タスクの `timeout` と `retry` がそのまま効く一つのアクティビティのほうが合う。
- **Argo の突き合わせで、全部の実行を本物の Pod で走らせること。** テスト全体で 30 分余りかかった。kind のノードの containerd が負荷のもとで落ち、食い違いになることもあった。本物の Pod はフローごとに一本にし、ほかはランナーが Pod を演じる（5 章）。
- **Argo のコントローラーの待ちを 1 秒より短くすること。** 300 ms では、かえって遅くなった。Argo のソースにも、informer が追いつかないので 1 秒より短くしないよう書いてある。
- **Argo の突き合わせで、呼び出しを HTTP テンプレートやエグゼキュータのプラグインに置き換えること。** Pod を立てずに済むが、生成したワークフローが呼び出しの終わりを読む形（終了コードとファイルの出力）が変わり、確かめているものが生成物でなくなる。
- **出来事を、待つ前に来たものも受け取っておくこと。** 送る側は待ちの始まりを知らないので、先に来た出来事を貯めておけば、送り直さずに済む。しかし、貯めた出来事は新しい実行に続くときに運ばなければならず、待たれないまま残ることもある。二度目の出来事をどう扱うかも決めにくい。待っているあいだだけ受け取り、ほかは断れば、送る側は断られたことを知り、いつ送り直すかを自分で決められる（`status` の `events` で待ちが分かる）。
- **出来事をシグナルで受けること。** シグナルは送りっぱなしで、ワークフローが受け取ったかどうかを送った側が知れない。待っていない出来事を断ることもできない。
- **E040 を `check` で出すこと。** はじめはこの形だった。上限は出力先ごとに違うので、Temporal だけに出すワークフローが、Step Functions の上限で `check` を通らなかった。Temporal では一番外のループが新しい実行で続くので、同じワークフローでも出力先によって上限に当たるかどうかが変わる。
- **エージェントの答えを、型を包まずにそのまま Schema にすること。** レコードの答えなら包まなくても書けるが、Structured Outputs は一番外にオブジェクトしか取らないので、列挙や文字列やリストの答えは包むしかない。包むかどうかを型で変えると、答えの取り出し方も出力先ごとに二通りになるので、いつも `{"answer": …}` に包むことにした。

## 7. まだやっていないこと

- 別々の本体を同時に走らせる Parallel。案件を二つの枝から同時に動かすことは、並行の問題になるので断る予定。
- API の記述（1.10）の、まだ読まないところ。OpenAPI の YAML と、記述の外を指す `$ref`、文字列の長さや `pattern`（Smithy の `@length` や `@pattern` も）、Stripe の `expand` で展開を頼んだときの答え、protobuf のバイナリの符号化と、Connect の GET とストリーム、Connect のエラーの `details`。記述から型を作ること（いまはタスクの型を書き、記述に合わせる）。
- 入れ子の実行が宣言したエラーを、Step Functions で見分けること（AWS の上で Cause の形を確かめてから）。
- ワークフローが自分で持つ案件（rulec の生成した関数を呼んで次の状態を得る形）。
- 規則の前提（rulec の `preconditions`）を、判断に渡す値を作ったタスクの直後で確かめること。
- 変数の範囲を、流れに沿って（値を入れた場所ごとに）求めること。いまは、変数に入れるすべての値の範囲を合わせる（1.3）。
- 範囲の端に単位を付けて書くこと（rulec のように `>=1g <=40kg` と書き、型の単位に換算する）。
- 子の `.flow`（1.9）を、本物の子と親で走らせる確かめを、Temporal のほかの出力先でもすること（いまは子の代わりで突き合わせ、本物どうしは Temporal だけ）。pydantic-graph で、子のグラフを親のプロセスの中でそのまま走らせること（いまは利用者が書く関数）。
- 規則を ASL の中に JSONata として埋めること。rulec の側の新しい出力先になり、数を 2^53 までに収める証明が要る。
- durable functions のシナリオでタイムアウトを起こすこと。
- durable functions の Python（`aws_durable_execution_sdk_python`）への出力。
- AWS の上（Step Functions の TestState と、durable functions の CloudDurableTestRunner）での確認と、`Transport` の既定の実装を本物の AWS と HTTP の相手に向ける確認（いまは手元の代わりと moto まで）。Temporal は dev server の上で確かめているが、本番の構成や Temporal Cloud の上では確かめていない。
- LocalStack の今の版での突き合わせ。今の版のイメージ（2026.8.4）は HTTP Task の実装を持つので、resource を替えずに流せるかもしれないが、起動に認証トークン（無料の Hobby のプランでも、アカウント）が要る。4.14.0 の扱い（5 章）のうち、式の評価の失敗の名前とモックの `States.` のエラーが、今の版で直っているかも見ていない。
- 実行中の案件が、規則やワークフローの改定でどうなるか（rulec の `diff` と `replay` を実行履歴に当てる）。
- `dandori explain` と、`build` の JSON 出力。
- Argo Workflows（名前で再開する suspend のステップ）と pydantic-graph（`Deps` に名前で届く値）の `event`。
- 待ちの途中でも受け取る出来事（受けたら流れを変える、Temporal のシグナルやアップデートのハンドラにあたるもの）。いまの `event` は、流れの一か所で待つものだけである。
- Temporal で、一番外にないループ（ループの中のループ、`match` の中のループ）でも新しい実行で続けること。続きの実行に、外のループの回と、どの枝にいたかも渡すことになる。
- Argo で、タスクのタイムアウトを含むシナリオを起こすこと（Pod を期限で止める代わりが要る）。
- Argo の `on cancel`（`argo stop` で走る exit handler として）と、pydantic-graph の `on cancel`（グラフを走らせるタスクのキャンセルを受けて）。
- 待ち（`wait`）のあいだに届くキャンセルのシナリオ。いまのシナリオは、呼び出しのあいだに届くキャンセルだけを演じる（検査は待ちも数える）。
- Argo で、`workflow template` の子が宣言したエラーを見分けること。子の `dd_error` は子の Workflow の出力に残るので、resource のステップで読めるはずだが、失敗した resource のステップが出力を集めるかを確かめていない。
- `caller/` のイメージを、本物の Lambda・HTTP・AWS に向けて Argo から動かす確認。
- Argo の突き合わせのうち Pod を演じる回では、Argo の executor（init と wait のコンテナ、ファイルからの出力の読み取り）とコンテナの実行が走らない。それを通るのは、フローごとに一本の本物の Pod の回だけである。
- Argo で、`break` のあとの回もノードを作ること。回の数だけ Workflow が大きくなる。
- TypeScript と Python の生成したワークフローが、どの実行でも同じコマンドを同じ順で出すこと（いまはキャンセルやタイムアウトを通る実行で違う。5.1）。そうなれば、二つの言語のワーカーを同じキューに混ぜられる。
- pydantic-graph の実行を、止まったあとに続けること。2.x が状態を外に置く手段を持たないので、今はプロセスの中の実行だけにしている。
- エージェントを OpenAI と Anthropic の API に本当に向ける確認。構造化出力が生成した Schema を受け付けるか（Claude では、文法の大きさの上限に当たらないかも）、Step Functions の HTTP Task と EventBridge の接続で呼べるか、どちらも確かめていない。
- エージェントの断りや API のエラー（429 など）を、名前の付いたエラーとして受けること。いまは `failure` にまとめている。
- Claude のエージェントを、Anthropic の API のほかの場所（ゲートウェイなど）に送ること。Open Responses の適合テスト（openresponses.org の acceptance tests）を、dandori が送る要求と読む答えに当てること。
- OpenAI が走らせる Agents API（長く動くエージェント）を、コールバックで待つ呼び方。
- エージェントにツールを持たせること、指示をファイルから読むこと、モデルの設定（推論の強さなど）を `.flow` に書くこと。
