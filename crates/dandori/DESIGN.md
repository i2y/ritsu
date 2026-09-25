# dandori 設計文書

業務ルールを呼ぶワークフローのための、型の付いた小さな言語。書いたものを走らせる前に検査し、AWS Step Functions（ASL）、Temporal（TypeScript）、AWS Lambda durable functions（TypeScript）、Argo Workflows（WorkflowTemplate の YAML）へコンパイルする。判断そのものは rulec の規則に書き、dandori はそれを rulec の CLI を通して読む。

名前は段取り（手順を前もって組むこと）から取った。ファイルの拡張子は `.flow`。

## 0. 全体像

```
.flow ── 構文解析 ── 名前と型の解決（rulec の schema・certificate・api を読む）── 型を付けた構文木
                                                                              ├── 流れに沿った検査（案件の状態、代入、網羅、出口）
                                                                              ├── 参照インタプリタ ── シナリオの自動生成
                                                                              ├── Step Functions（ASL、JSONata）と Lambda をつなぐコード
                                                                              ├── Temporal（TypeScript）と、タスクと規則のアクティビティ
                                                                              ├── Lambda durable functions（TypeScript）と、規則を呼ぶ Lambda
                                                                              └── Argo Workflows（YAML）と、タスクと規則をコンテナで動かすプログラム
```

### 0.1 前提

- **P1**：判断は rulec に置く。dandori の式は値を組み立てられる（レコード、リスト、値を埋め込んだ文字列）が、比較・算術・論理演算を持たない。分岐は、列挙・bool・無いことがある値の `match` だけ。
- **P2**：rulec の中には入れない。dandori は rulec の CLI の出力（JSON）だけを読み、rulec は dandori を知らない。
- **P3**：意味を決めるのは参照インタプリタ一つ。四つの出力先は、どれもそれと突き合わせて確かめる。
- **P4**：ループは回数に上限を書く。再帰は無い。だから実行履歴の長さに上限が見積もれる。
- **P5**：静的に言えないこと（外の相手が何を返すか）は、境界で実行時に確かめる。確かめて外れたら、その場で失敗させる。

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
description "ホテルの予約で、カードの与信を取り、チェックアウトの日に確定する"

use rule 与信 from "rules/宿泊の与信額.rule"
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
  let 見積 = 与信(客室: 予約.客室, 泊数: 予約.泊数)
  match 見積.扱い
    確認 => succeed 結果 = 確認待ち
    自動 => pi <- create_intent(…)
  …

on failure
  …
```

字下げがブロックを表す。コメントは `#` から行末まで。名前には日本語が使え、生成物でもそのまま識別子になる（JavaScript と Step Functions の変数は Unicode の識別子を受け付ける）。

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

### 1.3 宣言

- `use rule <名前> from "<パス>"`：規則を読む。Step Functions と durable functions から呼ぶなら、下に `lambda "<関数>"` を書く。
- `task <名前>(<引数>) [-> <型>]`：外の相手への呼び出し。`->` を書かないタスクは何も答えない（答えは読まない）。下に次の項目を書く。
  - 呼び方（1.5）：`lambda "<関数>"`、`http <メソッド> "<URL>" [form]` と `connection "<EventBridge の接続>"`、`aws <サービス>:<操作>` のどれか一つ。子ワークフローとして呼ぶなら、出力先ごとに `workflow "<型>"`（Temporal）、`state machine "<ARN>"`（Step Functions）、`durable function "<関数>"`（durable functions）、`workflow template "<名前>"`（Argo）。Temporal では `queue "<タスクキュー>"` で送り先のキューを選べる。Argo で利用者のコンテナを動かすなら `image "<イメージ>"`。
  - `errors <名前> [= <HTTP ステータス> | = <例外の名前>], …`：業務のエラー。`http` のタスクではステータスを、`aws` のタスクでは API の例外（`ConditionalCheckFailedException`、書かなければエラーの名前そのもの）を書く。`timeout` と `failure`（それ以外の失敗のすべて）は宣言しなくても使える。
  - `retry <n> times [every <時間>] [backoff <k>] [on <エラー>, …]`：`on` を書かなければ、`failure` と `timeout` をやり直し、宣言したエラーはやり直さない。
  - `timeout <時間>`、`key`（冪等キーを送る。`aws` のタスクでは `key ClientToken` のように、キーを受け取る API の引数を書く）、`idempotent`（二度しても一度と同じ）、`callback`（トークンや ID を渡して答えを待つ）。
  - 案件に何をするか：`starts <規則>.<ステートマシン> [then <出来事>, …]`、`sends <出来事>`、`observes`。`refused as <エラー>` は、ステートマシンが出来事を断ったときに返ってくるエラー。
- `case <名前> : <レコード> follows <規則>.<ステートマシン>`：rulec のステートマシンに従う案件。下に `held <入力> = <値>`、`external <出来事>, …`、`state <フィールド>`、`refused when <出力> = <値>` を書く。
- `inputs`・`outputs`：ワークフローの入力と出力。`T?` の出力は `succeed` で省ける。
- `flow`：本体。`on failure`：処理されなかったタスクのエラーのあとに走るブロック。

### 1.4 文と式

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

### 1.5 タスクの呼び方

タスクを出力先がどう呼ぶかは、タスクに書いた呼び方で決まる。

| 呼び方 | Step Functions | Temporal | Lambda durable functions | Argo Workflows |
|---|---|---|---|---|
| `lambda` | Lambda の Task | dandori が書くアクティビティが関数を呼ぶ | dandori が書く step の中で関数を呼ぶ | dandori が書く実装をコンテナで動かす |
| `http` | HTTP Task | dandori が書くアクティビティが `fetch` で送る | dandori が書く step の中で `fetch` で送る | 同じく、コンテナで `fetch` |
| `aws` | AWS SDK 統合 | dandori が書くアクティビティが AWS SDK で呼ぶ | dandori が書く step の中で AWS SDK で呼ぶ | 同じく、コンテナで AWS SDK |
| `state machine` | 入れ子の実行（`startExecution.sync:2`） | | | |
| `workflow` | | 子ワークフロー（`executeChild`） | | |
| `durable function` | | | 別の durable function の invoke | |
| `workflow template` | | | | その WorkflowTemplate から Workflow を作る |
| `image` | | | | 利用者のイメージのコンテナ |
| どれも無い | 作れない（E050） | 利用者が書くアクティビティ（`OwnTasks`） | 利用者が書く実装を step の中で呼ぶ（`OwnTasks`） | 作れない（E050） |

Temporal、durable functions、Argo で dandori が書く実装は、Step Functions が送るのと同じものを送る。Lambda の payload、HTTP のメソッド・URL・ヘッダ・本文、AWS の API の引数は、四つの出力先で一字も違わない（5 章で突き合わせる）。送り方は `Transport`（`io.ts`）を通すので、認証のヘッダ、AWS SDK のクライアントの設定、テストでの代わりは利用者が渡す。既定の `Transport` は `fetch` と AWS SDK for JavaScript v3 を使い、SDK は使うときに読み込む。既定で知っている AWS のサービスは 13 で、ほかのサービスは利用者の `Transport` が呼ぶ。

`callback` のタスクは、呼び方が `lambda` か `aws sqs:sendMessage` か、どれも無いもの。答えの戻り方は出力先ごとに違う。

- Step Functions：`.waitForTaskToken` で、トークンを `task_token` として payload に入れる。SQS ではメッセージの本文（レコードか `json` の `MessageBody`）に入れる。答えは `SendTaskSuccess`。
- Temporal：送る側（アクティビティ）に `callback_id` を渡し、答えはシグナル `dandori.callback` で `{ callback_id, ok }` か `{ callback_id, error, message }` として受ける。どのワークフローに送るかは `callback_id` から分かる（`io.workflowOf`）。
- durable functions：`createCallback` の ID を `callback_id` として渡し、答えは `SendDurableExecutionCallbackSuccess`。
- Argo：送る側のコンテナに `callback_id` を渡し、そのあとの suspend のステップで待つ。答えは `argo node set` で待ちのステップの出力 `answer` に入れ、`argo resume` で進める（4.4）。

AWS の API のエラーは、Step Functions では「サービスの接頭辞.例外の名前」になる（`DynamoDb.ConditionalCheckFailedException`）。接頭辞はサービスの名前を大文字にしただけではないので（`dynamodb` は `DynamoDb`、`sesv2` は `SesV2`）、Step Functions の開発者ガイドの一覧（417 のサービス）を `src/aws.rs` に表として持つ。一覧に無いサービスは E007 とする。

### 1.6 並列の回

`for … in parallel` の各回は同時に回る。それでも四つの出力先で意味がそろうよう、次のように決めた。

- 各回は自分の変数を持つ。回の中で値を入れる変数は、その回のものになり、回の外の変数に値を入れることはできない（中と外の両方で値を入れる名前は E009）。回の外の変数は読める。案件は動かせない（E009）。`break` と `succeed` は書けない（E009）。
- ある回が失敗しても、ほかの回は最後まで回る。すべての回が終わってから、失敗した回があれば、リストの順で最初の回の失敗でループが失敗する。タスクのエラーならそのあと `on failure` が走り、`fail` や答えの検査の失敗ならそのまま終わる。どの回が先に失敗したかは出力先によって違いうるので、時間の順ではなくリストの順で決める。
- `yield` を集めたリストは、リストの順に並ぶ。

Step Functions の Map は、どれか一つの回が失敗すると Map ごと失敗してほかの回を止める。そこで、回の中のエラーは回の中ですべて受け、回は「yield した値」か「どう失敗したか」を答えとして終わる。Map のあとで、その答えの並びから最初の失敗を探す。

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

## 3. 検査の一覧

| コード | 内容 |
|---|---|
| E001 | 構文 |
| E002 | 名前が無い（型・変数・フィールド・規則・タスク。単位の種類の誤りも） |
| E003 | 型が合わない（無いことがある値をそのまま使う、`none` の置き場所、リストのリスト、型の分からない `{…}` や `[]` も） |
| E004 | 引数や出力の過不足、`{…}` のレコードのフィールドの不足 |
| E005 | 規則を rulec で読めなかった |
| E006 | 二度の宣言 |
| E007 | タスクの項目の誤り（ステータスの付け忘れ、同じステータスの二つのエラー、呼び方の重なり、`aws` の `key` の引数、`callback` にできない呼び方、知らない AWS のサービス、子ワークフローの `key` など） |
| E008 | 案件の宣言や、案件に何をするかの誤り |
| E009 | 文の置き場所の誤り（`yield`、並列の回の中の `break`・`succeed`・案件の呼び出し、中と外の両方で値を入れる変数、規則を `let` なしで呼ぶことも） |
| E010 | `match` に行き先の無い値がある |
| E011 | 通らない行き先 |
| E012 | 値を持っていないことがある変数を読んだ |
| E013 | 始まっていない案件にタスクを呼んだ、始まった案件をもう一度始めた |
| E020 | 終わりでない状態の案件を残して終わる |
| E021 | どの状態でも断られる出来事を送った |
| E022 | 断られることがある出来事の断りを受けていない |
| E030 | 相手の側を変える呼び出しを、`key` なしでやり直す |
| E031 | Express でできないこと（五分を超える待ち、コールバック、入れ子の実行、`key` なしで相手を変える呼び出し） |
| E040 | 実行履歴が上限を超えうる（Step Functions は 25,000 件、Temporal は 51,200 件、Lambda durable functions は 3,000 操作）。Argo Workflows では、一回の実行のノードが目安の 10,000 個を超えうる |
| E050 | 出力先に出すのに要るものが無い、または出力先でできないこと（Step Functions では呼び方か `connection`、入れ子の実行の宣言したエラー。Step Functions と durable functions では呼ばれる規則の `lambda`。durable functions では invoke する関数の `timeout`。Argo では呼び方か `image`、`workflow template` の宣言したエラー、`callback` のタスクの `retry`） |
| W030 | 相手の側を変えるかもしれない呼び出しを `key` なしでやり直す |
| W101 | 処理されないエラーで、案件を終わりでない状態に残して失敗することがある |
| W102 | 動くことのない `on <断り>` |
| W103 | 案件を始めるタスクに `key` が無い |

診断には、そうなる例（その点までの短い流れ）が付く。例は、状態・変数・`match` で絞った値ごとに持っているので、例の中で値が途中で変わることはない。

実行履歴の件数は、一回の呼び出し、再試行、Choice、Wait、並列の回などごとに多めに見積もった件数を足して出す（`check.rs` の `ASL_COST`・`TEMPORAL_COST`・`DURABLE_COST`・`ARGO_COST`）。多めに見積もるので、上限の近くでは言い過ぎる側に倒れる。`for` は上限の数だけ回るとして数える。

Argo には履歴の件数の上限が無い。限りは Workflow の大きさで、ノードの状態を含めて 1 MiB まで（`MAX_WORKFLOW_SIZE`。超えるとノードの状態を圧縮し、それでも超えれば、状態をデータベースに置く設定が無いかぎり保存できない）。`tests/flows/edges.flow` の実行では、ノード一つが JSON で 510〜612 バイト、圧縮して 60 バイトほどだった。60 バイトなら 15,000 個ほど入るが、テストより大きな値を運ぶワークフローでは減るので、目安を 10,000 個にした。WorkflowTemplate も、Workflow の中に二度（`storedTemplates` と `storedWorkflowTemplateSpec`）圧縮されずに入る。例でいちばん大きいホテルの予約では、それぞれ 122 KB と 119 KB で、ノードが 56 個の実行の Workflow は全体で 270 KB だった。テンプレートを小さくするため、値を計算する式は、同じ値を二度読むところを式の頭の `let` 一つにまとめ、呼び出しの終わり方（エラーの種類、答えの検査、受ける `on`）も出力ごとに `let` で一度だけ求める。

## 4. 出力先

### 4.1 Step Functions

- `QueryLanguage` は JSONata。JSONPath は出さない。
- ワークフローの変数は全部、ステートマシンの変数にし、始まりの Pass で null にする。入力はそこで `$states.input` から入れ、次の Choice で型を確かめる。外れたら `Dandori.BadInput` で失敗する。並列の回が持つ変数は、回の始まりで null にする（外の変数と同じ名前を回の中で入れることを Step Functions は許さない）。
- 呼び出しは Task と、その答えを確かめる Choice の組にする。型が合わなければ `Dandori.BadResponse`、案件の状態が検査の言った集合の外なら `Dandori.UnexpectedState`。
- 無いことがあるフィールドを読む式は `($exists(x) ? x : null)` にして、式が undefined にならないようにする。
- `match` は Choice にし、どの行き先の条件も明示する。Default は `Dandori.UnexpectedValue` の Fail で、黙って最後の行き先へ流れることはない。`some y` の行き先は、Pass で `y` に値を入れてから本体へ進む。
- `retry` は Retry にする。`on` が無いときは、宣言したエラーを `MaxAttempts: 0` で止める retrier を先に置き、そのあとに `States.ALL` を置く。
- `on failure` は、各 Task の最後の Catch（`States.ALL`）から入る。エラーは変数 `dd_error` に入り、ブロックの終わりの Fail がそれで失敗し直す。
- 冪等キーは `実行名/呼び出しの場所/各ループの回数`。HTTP では `Idempotency-Key` ヘッダ、Lambda では `idempotency_key`、AWS の API では `key` に書いた引数で渡す。
- `callback` は、Lambda なら `lambda:invoke.waitForTaskToken` でトークンを `task_token` で渡し、SQS なら `sqs:sendMessage.waitForTaskToken` でメッセージの本文に入れる。
- `aws` は `arn:aws:states:::aws-sdk:<サービス>:<操作>`。引数と答えは API のまま（PascalCase）で、型は利用者がタスクに書く（HTTP のタスクと同じ）。
- `state machine` は `startExecution.sync:2` で、答えは子の出力（`Output`）。子の失敗は親には `States.TaskFailed` として届き、子の `fail` の名前は Cause の中にしかない。Cause の形は文書で確かめられなかったので、入れ子の実行に宣言したエラーは E050 とし、`failure` で受けてもらう。
- `for` は、Pass と Choice で数えるループにする。要素の数は最初の Choice で確かめる。`yield` は `$append` で集める。
- `for … in parallel` は INLINE の Map にする。回の始まりの Pass が、ItemSelector で渡した要素と番号（`$states.context.Map.Item.Index` は ItemSelector でしか読めない）を回の変数に入れる。回の中の Task には、受けていないエラーを `{"fail": {Error, Cause, task: true}}` に変えて回を終える Catch を付け、`fail` や検査の失敗も同じ形で回を終える。Map のあと、答えの並びに失敗があれば最初のものを取り出し、タスクのエラーなら `on failure` へ、そうでなければ Fail へ進む。Fail の Cause は文字列でなければならないので、理由の無い `fail` は Cause を書かない Fail に分ける。
- 規則を呼ぶ Lambda 関数のコード（Python）も出す。rulec が生成した Python を import し、JSON を列挙と整数に直して呼び、答えを JSON に戻す。

### 4.2 Temporal

- `types.ts`（型と検査）、`activities.ts`（タスクの型、dandori が書くタスク、利用者が書く `OwnTasks`、それらを合わせる `makeActivities(own, transport)`）、`io.ts`（`Transport`）、`rules.ts`（規則を包むアクティビティ）、`runtime.ts`（共通の部品）、`workflow.ts` を出す。
- **再試行は Temporal にさせない。** アクティビティは `maximumAttempts: 1` で呼び、ワークフローのコードが ASL と同じ retrier の並びでやり直す。待ちは `sleep` で、間隔も Step Functions と同じ計算。どの出力先でも、同じエラーを同じ回数だけやり直すためである。
- 宣言したエラーは、アクティビティが投げる `ApplicationFailure` の `type` で見分ける。`TimeoutFailure` は `timeout`、ほかは `failure`。dandori が書くアクティビティは、Lambda の `errorType`、HTTP のステータス、AWS の API の例外の名前を、宣言したエラーの名前に直して投げる。
- `queue` は、アクティビティと子ワークフローの `taskQueue` になる。別のワーカー（ほかの言語の SDK で書いたものでも）が持つアクティビティを、名前と引数の形が合えばそのまま呼べる。
- `workflow` は `executeChild` で呼ぶ。ワークフロー ID は `親の ID/呼び出しの場所/各ループの回数/何度目か`。子の失敗は `ChildWorkflowFailure` の `cause` に `ApplicationFailure` として届くので、宣言したエラーをその `type` で見分けられる。
- **`callback` はシグナルで答えを受ける。** 送る側のアクティビティに `callback_id`（ワークフロー ID と呼び出しの場所を JSON にしたもの）を渡し、ワークフローはシグナル `dandori.callback` でその ID の答えが来るのを `condition` で待つ。`timeout` を過ぎれば `timeout` になる。
- `for … in parallel` は、`dd.rounds` が `k` 個ずつ回を走らせる。ワークフローのコードは決定的に動くので、Promise で並べても再生で食い違わない。回の変数は回の関数の中で宣言する。
- `on failure` は、処理されなかったタスクのエラー（`TaskError`）のときだけ走り、`fail` や答えの検査の失敗では走らない。Step Functions で Fail の状態に Catch が効かないのと同じにした。
- 規則は普通のアクティビティとして呼ぶ。rulec の TypeScript を import し、`parse<列挙>` と `BigInt` で直して呼ぶ。規則は純関数なので、決定性のためだけならワークフローのコードで直接呼べるが、そうしない。理由は三つ。
  - 判定の入力と答えが、実行履歴に残る（ActivityTaskScheduled と ActivityTaskCompleted）。どの入力でどう判定したかを後から読め、rulec の `replay` と `diff` に渡す記録にもなる。
  - 規則を直してワーカーを出し直しても、実行中のワークフローの再生が履歴と食い違わない。再生では答えを履歴から読む。
  - 規則のアクティビティを別のワーカーに置けば、ワークフローに触れずに規則だけを出し直せる。Step Functions で Lambda を呼ぶのと同じ形になる。

  費用は、一回につきタスクキューの往復と履歴 6 件。業務のワークフローで判定が数回なら問題にならない。

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

- `<ワークフロー>.argo.yaml`（WorkflowTemplate）と、`caller/`（`lambda`・`http`・`aws` のタスクと規則をコンテナの中で動かすプログラム。`types.ts`・`tasks.ts`・`io.ts`・`transport.ts`・`rules.ts`・`call.ts`・`package.json`・`Dockerfile`）を出す。入力はパラメータ `input` に JSON で渡し、出力はグローバル出力パラメータ `dd_output` に残る。
- **タスクはどれもコンテナで動く。** `image` のタスクは利用者のイメージ、`lambda`・`http`・`aws` のタスクと規則は `caller/` から作るイメージで動く。中の実装は Temporal と durable functions 向けと同じもので、同じ `Transport` を通す。呼び出しは環境変数 `DANDORI_CALL` で渡る。答えは `/tmp/dandori/answer.json` に書き、宣言したエラーなら `/tmp/dandori/error.json` に `{"error", "message"}` を書いて終了コード 3 で終える。期限で止められた Pod（終了コード 143 か 137）は `timeout`、ほかの終わり方は `failure`。
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

### 4.5 四つの出力先でそろえたこと

| こと | Step Functions | Temporal | Lambda durable functions | Argo Workflows |
|---|---|---|---|---|
| 再試行 | Retry | ワークフローのコード（同じ並び、同じ間隔） | ワークフローのコード（同じ並び、あいだは `context.wait`） | `retryStrategy`（`expression` でエラーを選ぶ） |
| 答えの検査 | Choice と Fail | `is_<型>` と `dd.fail` | Temporal と同じ | 出力の式（`type()` と `matches`） |
| 冪等キー | `$states.context.Execution.Name` から | `workflowInfo().workflowId` から | `durableExecutionArn` から | `workflow.name` から |
| `lambda`・`http`・`aws` のタスク | Task（Lambda、HTTP、AWS SDK 統合） | dandori が書くアクティビティ（`Transport` を通す） | dandori が書く実装を step の中で | dandori が書く実装をコンテナで |
| それ以外のタスク | 作れない | 利用者のアクティビティ | 利用者の実装を step の中で | `image` のコンテナ（無ければ作れない） |
| 子ワークフロー | 入れ子の実行 | `executeChild` | `context.invoke` | WorkflowTemplate から Workflow を作る |
| コールバック | `.waitForTaskToken` | シグナル | `createCallback` | suspend と `argo node set` |
| 並列の回 | INLINE の Map、回の答えで失敗を運ぶ | `dd.rounds` | `context.map`、回の答えで失敗を運ぶ | `withSequence` と `parallelism`、回の出力で失敗を運ぶ |
| 規則 | Lambda と、Python をつなぐコード | アクティビティと、TypeScript をつなぐコード | `context.invoke` と、Step Functions と同じ Lambda | コンテナと、TypeScript をつなぐコード |
| on failure | Task の Catch | `TaskError` を受ける catch | Temporal と同じ | `dd_ctl` が `task` のときに走るステップ |

実行中の案件と規則の版の関係は、どの出力先でも同じになる。規則は Lambda かアクティビティとして呼ぶので、規則を直すと、実行中の案件も途中から新しい版で判定される。

## 5. 確かめ方

- **参照インタプリタ**（`dandori run`）：シナリオ（入力と、呼び出しが受け取る答えの並び）に沿って型を付けた構文木を実行し、呼び出しを出力先から見える形（Lambda の関数と payload、HTTP のメソッドと URL とヘッダと本文、AWS の API と引数、入れ子の実行・子ワークフロー・invoke の先と入力、Temporal のアクティビティと引数、durable functions の step と引数・コールバックの submit）で書き出す。並列の回は、リストの順に一つずつ回す。
- **シナリオの自動生成**（`dandori scenarios`）：選んだ答えの記録を持ってワークフローを実行し直す。選び方は、選択の場所ごとに、まだ使われていない選択肢を先に取る。何か新しい所に着いた実行を土台にして、その途中の選択を一つずつ変えた実行を試し、そこから先は同じ場所で同じ選択を繰り返す（ループが回り切る流れはこれで出る）。答えの検査に落ちる答え（型の合わない答え、ありえない状態）も入れる。リストの長さは `for` が読むときに選び、空・一つ・二つ・上限を一つ超える長さを試す。無いことがある値は `match` が読むときに、無い場合とある場合を選ぶ。
- **Step Functions の突き合わせ**：生成した ASL を `tools/asl-run.mjs` が JSONata 2.0.6 で実行し、参照インタプリタと同じ形で書き出す。Map の回は一つずつ回し、回の中で外の変数に値を入れたら失敗させる（Step Functions の規則）。全シナリオで一致を見る。定義は asl-validator 4.0.0 にも通す。呼び方を持たないタスクのあるワークフロー（Temporal と durable functions 向けのもの）は、E050 を表示して外す。
- **Temporal の突き合わせ**：生成した TypeScript を `tools/temporal/run.mjs` が SDK 1.24.0 の時間を飛ばすテスト環境で実行する。dandori が書くアクティビティは本物を走らせ、`Transport` だけを、送るものを書き出してシナリオどおりに答える代わり（`tools/transport.mjs`）に差し替える。だから Step Functions が送るものと一字ずつ比べられる。利用者のタスク、規則、子ワークフローは、シナリオどおりに答える代わりを置く（子ワークフローは、答えをアクティビティに聞く小さなワークフロー）。`queue` ごとにワーカーを立てる。コールバックの答えはシグナルで送り、タイムアウトのシナリオでは送らずに、待ちの時間を飛ばして切れさせる。アクティビティのタイムアウトはこの環境では起こせないので、それを含むシナリオは外す。並列の回は一つずつ回す（`dd.atATime` を 1 に書き換える）。
- **Lambda durable functions の突き合わせ**：生成した TypeScript を `tools/durable/run.mjs` が SDK 2.4.0 のローカルのテストランナー（`@aws/durable-execution-sdk-js-testing` 1.1.4、時間を飛ばす）で実行する。`Transport` の差し替えは Temporal と同じ。規則は `registerFunction`、`durable function` の先は `registerDurableFunction` で登録した代わりで答え、後者は失敗するとき `ErrorData` を入れる。コールバックはその ID に答えを送って置き換える。このランナーは答えの来ないコールバックを期限切れにしないので、タイムアウトを含むシナリオは外す。
- **Argo Workflows の突き合わせ**：生成した WorkflowTemplate を、kind の上の Argo Workflows v4.1.4（`tools/argo/setup.sh` が用意する）で `tools/argo/run.mjs` が実行する。`caller/` は、Temporal と同じ差し替えの `Transport` を持たせて Pod の中で動かす。`image` のタスク、規則、`workflow template` の先は、クラスタの中のモック（`tools/argo/mock.mjs`）にシナリオの答えを聞く代わりのコンテナ（`tools/argo/stand-in.mjs`）にする。コールバックの答えは `argo node set` と `argo resume` で渡し、タイムアウトのシナリオでは、期限が来たときと同じ値を渡す。タスクのタイムアウトは起こせないので、それを含むシナリオは外す。並列の回は一つずつ回す（`withSequence` を持つテンプレートの `parallelism` を 1 にする）。
- **つなぐコード**：Lambda の Python と `rules.ts` を、rulec が生成した Python と TypeScript と一緒に動かし、`rulec vectors` の全件で rulec の期待値と比べる。durable functions 向けに出す規則の Lambda が、Step Functions 向けのものと一字も違わないことも確かめる。
- **診断**：`tests/fixtures` の各ファイルの診断を、英語と日本語の両方で固定する（`DANDORI_BLESS=1` で書き直す）。
- 突き合わせるのは `examples` の例と、言語の端の振る舞いを通すための `tests/flows` のフロー。

`Transport` の既定の実装（`fetch` と AWS SDK で本当に送る部分）は、突き合わせでは走らない。確かめているのは、dandori が書く実装が何を送り、答えをどう読むかである。

### 5.1 走らせた（2026-09-25、Argo は 2026-09-26）

- 例は四つ。ホテルの予約（Stripe の PaymentIntent）、倉庫の注文の出荷（rulec の「注文の状態」）、注文の引当と発送（リスト、並列の回、無いことがある値、`json`、HTTP・SNS・SQS のコールバック・子ワークフロー）、申し込みの審査（利用者が書くタスクだけで、`queue` と利用者のコールバック。Temporal、durable functions、Argo（`image`）向け）。ほかに `tests/flows/edges.flow` が、配列の `json` をリストに入れること、並列の中の並列、理由の無い `fail` を通る。
- シナリオは 49 本・33 本・18 本・10 本と、edges の 10 本。Step Functions では、審査を除く 110 本すべてが参照インタプリタと一致した。Temporal では 120 本すべてが一致した（コールバックのタイムアウトもシグナルを送らないことで起こせるので、外したものは無い）。Lambda durable functions では、タイムアウトを含む 12 本を除く 108 本すべてが一致した。
- Argo Workflows（v4.1.4、kind の上）では、120 本すべてが参照インタプリタと一致した。タスクのタイムアウトを含むシナリオは無く、コールバックのタイムアウトは、待ちの出力に期限切れのときの値を入れて起こしたので、外したものは無い。一回の実行のノードの数は、多いもので、ホテルの予約が 567 個（E040 の見積もりは 939 個）、倉庫の出荷が 239 個（446 個）、edges が 248 個（593 個）、引当と発送が 171 個（4,746 個）、審査が 74 個（136 個）で、どれも見積もりの中に収まった。テスト全体でおよそ 28 分かかり、そのほとんどが Argo の突き合わせである。
- 生成器にわざと誤りを入れると（ASL の冪等キーの区切りを一文字変える、`match` の条件から値を一つ落とす、durable functions の冪等キーの呼び出しの場所をずらす）、どれも一本目のシナリオで食い違いとして出た（2026-09-25）。Argo の生成器でも、冪等キーの区切りを一文字変える、`match` の条件から値を一つ落とすと、どちらもホテルの予約の一本目で食い違いとして出た（2026-09-26）。
- つなぐコードは、与信額の規則のベクタ 24 件と、急ぎの規則のベクタ 16 件で、rulec の期待値と一致した。
- 検査が例を書く途中で見つけたこと。チェックアウトまで待つあいだに与信の期限が切れると capture が断られる。capture のあとは `processing` を経て `requires_payment_method` に戻ることがある。`requires_action` を見てから cancel するまでに、客が認証を済ませて期限が切れると、cancel も断られる。入金がないので注文の取消を頼むあいだに、客が自分で取り消していると、倉庫はその取消依頼を断る。

## 6. 捨てたもの

- **rulec に workflow の宣言を足すこと。** 0.2 のとおり。
- **JSONPath を出すこと。** 変数と JSONata があれば、ResultPath の組み替えなしにデータを運べる。
- **Temporal の再試行に任せること。** Temporal は、型を名指しして「これだけをやり直す」とは書けない（やり直さないものを名指しする）。Step Functions と同じ回数にならない。
- **`match` の最後の行き先を Default にすること。** 検査の外の値が、黙ってその行き先へ流れる。
- **条件式を dandori に入れること。** P1 のとおり。条件は rulec の表に書けば、網羅と重なりが rulec の証明書つきで閉じる。
- **値の組み立ても書けないこと。** はじめの P1 は、引数に変数・フィールド・リテラルしか書けず、レコードを組み立てるにも文字列に値を入れるにもタスクか規則が要った。判断にかかわらない操作まで外に出させる理由は無いので、組み立ては許した（0.1）。
- **Temporal で、規則をワークフローのコードで直接呼ぶこと。** 履歴は増えないが、判定の記録が残らず、規則を直すたびにワーカーの版の固定（Worker Deployment Versioning の `PINNED`）が要る。固定しないと、実行中のワークフローの再生が新しい規則で判定し直し、答えが変わった分岐で止まる。
- **Temporal で、規則をローカルアクティビティにすること**（いまは）。往復と履歴は減るが、規則をワークフローと同じワーカーに置くことになり、規則だけを出し直せなくなる。
- **Temporal で、コールバックをアクティビティの非同期完了で受けること。** はじめはこの形だった。答える側がアクティビティのタスクトークンを持つ必要があり、待つあいだアクティビティが開いたままになる。シグナルなら、答える側はワークフローを指す ID だけを持てばよく、待ちは `condition` の期限で表せ、テスト環境で時間を飛ばしてタイムアウトまで確かめられる。
- **durable functions で、`lambda` のタスクを `context.invoke` で呼ぶこと。** 4.3 のとおり、宣言したエラーを見分けられない。step の中で Invoke を呼ぶ実装を dandori が書けば、利用者のコードも要らない。
- **durable functions で、step の `retryStrategy` にやり直しを任せること。** step だけなら関数一つで Step Functions と同じ並びを書けるが、`context.invoke` にはやり直しの設定が無い。規則の呼び出しはどのみちワークフローのコードでやり直すので、step もそれにそろえた。
- **Step Functions で、入れ子の実行の失敗を Cause から読み分けること。** 子の `fail` の名前は Cause の JSON の中にあると思われるが、その形を文書で確かめられなかった。確かめられない形に頼る代わりに、宣言したエラーを E050 にした。
- **並列の回の失敗を、先に起きた順で決めること。** 出力先ごとに並べ方も速さも違うので、同じシナリオでも結果が変わる。リストの順なら、どの出力先でも同じ失敗になる。
- **Argo で、ステップの `when` や引数からグローバル出力パラメータを読むこと。** はじめはこの形で、変数を読む式をそのまま `when` と引数に書いていた。4.4 のとおり、同じテンプレートの前のステップが入れた値が見えない。kind の上では、受けていないエラーのあとの文まで走った。動いているステップの `when` が途中で偽に変わると、そのステップが進まなくなることも、ソースで確かめた。
- **Argo で、変数を引数と出力で次のステップへ運ぶこと。** グローバル出力パラメータを使わずに済むが、すべての変数がすべてのノードの入力と出力に載り、Workflow の大きさ（1 MiB まで）を早く使い切る。グローバル出力パラメータなら、値は Workflow の出力に一つずつと、値を入れたノードにだけ載る。
- **Argo で、ループを再帰で回すこと。** 回の数だけ入れ子が深くなり、Argo の深さの上限 100 に当たる。
- **Argo で、タスクの期限をテンプレートの `timeout` にすること。** 4.4 のとおり、Pod が始まる前に切れると `continueOn` が効かない。

## 7. まだやっていないこと

- 別々の本体を同時に走らせる Parallel。案件を二つの枝から同時に動かすことは、並行の問題になるので断る予定。
- AWS が公開している Smithy の API モデルからの、`aws` のタスクの型の取り込み（いまは利用者が書く）。
- 入れ子の実行が宣言したエラーを、Step Functions で見分けること（AWS の上で Cause の形を確かめてから）。
- ワークフローが自分で持つ案件（rulec の生成した関数を呼んで次の状態を得る形）。
- 規則の前提（rulec の `preconditions`）を、判断に渡す値を作ったタスクの直後で確かめること。
- 規則を ASL の中に JSONata として埋めること。rulec の側の新しい出力先になり、数を 2^53 までに収める証明が要る。
- durable functions のシナリオでタイムアウトを起こすこと。Temporal のアクティビティのタイムアウトも同じ。
- durable functions の Python（`aws_durable_execution_sdk_python`）への出力。
- AWS の上（Step Functions の TestState と、durable functions の CloudDurableTestRunner）と Temporal のサーバーの上での確認。`Transport` の既定の実装の確認も含む。
- 実行中の案件が、規則やワークフローの改定でどうなるか（rulec の `diff` と `replay` を実行履歴に当てる）。
- `dandori explain` と、`build` の JSON 出力。
- ループの中で何度も判定して履歴の上限に近づくワークフローのために、規則をローカルアクティビティで呼ぶ形を選べるようにすること。
- Argo で、タスクのタイムアウトを含むシナリオを起こすこと（Pod を期限で止める代わりが要る）。
- Argo で、`workflow template` の子が宣言したエラーを見分けること。子の `dd_error` は子の Workflow の出力に残るので、resource のステップで読めるはずだが、失敗した resource のステップが出力を集めるかを確かめていない。
- `caller/` のイメージを、本物の Lambda・HTTP・AWS に向けて Argo から動かす確認。
- Argo で、`break` のあとの回もノードを作ること。回の数だけ Workflow が大きくなる。
