# dandori 設計文書

業務ルールを呼ぶワークフローのための、型の付いた小さな言語。書いたものを走らせる前に検査し、AWS Step Functions（ASL）、Temporal（TypeScript）、AWS Lambda durable functions（TypeScript）へコンパイルする。判断そのものは rulec の規則に書き、dandori はそれを rulec の CLI を通して読む。

名前は仮で、段取り（手順を前もって組むこと）から取った。ファイルの拡張子は `.flow`。

## 0. 全体像

```
.flow ── 構文解析 ── 名前と型の解決（rulec の schema・certificate・api を読む）── 型を付けた構文木
                                                                              ├── 流れに沿った検査（案件の状態、代入、網羅、出口）
                                                                              ├── 参照インタプリタ ── シナリオの自動生成
                                                                              ├── Step Functions（ASL、JSONata）と Lambda をつなぐコード
                                                                              ├── Temporal（TypeScript）とアクティビティをつなぐコード
                                                                              └── Lambda durable functions（TypeScript）と、規則を呼ぶ Lambda
```

### 0.1 前提

- **P1**：条件と計算は rulec に置く。dandori の式は、変数・そのフィールド・リテラルだけで、算術も比較も持たない。分岐は、列挙か bool の `match` だけ。
- **P2**：rulec の中には入れない。dandori は rulec の CLI の出力（JSON）だけを読み、rulec は dandori を知らない。
- **P3**：意味を決めるのは参照インタプリタ一つ。三つの出力先は、どれもそれと突き合わせて確かめる。
- **P4**：ループは回数に上限を書く。再帰は無い。だから実行履歴の長さに上限が見積もれる。
- **P5**：静的に言えないこと（外の相手が何を返すか）は、境界で実行時に確かめる。確かめて外れたら、その場で失敗させる。

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
| `money[円, incl_tax]` など | 単位の付いた整数。rulec の書き方のまま書き、単位が違えば渡せない |
| `string` | 文字列 |
| `bool` | 真偽 |
| `timestamp` | UTC の RFC 3339（`2026-10-01T10:00:00Z`）。Step Functions の Wait が受け付ける形に合わせた |
| 列挙 | `enum 結果 = 宿泊済 \| 確認待ち`、または規則の列挙 `payment_intent.status` |
| レコード | `record` で宣言する。規則の呼び出しは、その規則の出力のレコードを返す |

リテラルの整数は単位の付いた型にも渡せる。変数どうしは、単位が同じでなければ渡せない。

### 1.3 宣言

- `use rule <名前> from "<パス>"`：規則を読む。Step Functions から呼ぶなら、下に `lambda "<関数>"` を書く。
- `task <名前>(<引数>) -> <型>`：外の相手への呼び出し。下に次の項目を書く。
  - `lambda "<関数>"`、または `http <メソッド> "<URL>" [form]` と `connection "<EventBridge の接続>"`。URL の `{id}` は引数で埋まる。
  - `errors <名前> [= <HTTP ステータス>], …`：業務のエラー。`timeout` と `failure`（それ以外の失敗のすべて）は宣言しなくても使える。
  - `retry <n> times [every <時間>] [backoff <k>] [on <エラー>, …]`：`on` を書かなければ、`failure` と `timeout` をやり直し、宣言したエラーはやり直さない。
  - `timeout <時間>`、`key`（冪等キーを送る）、`idempotent`（二度しても一度と同じ）、`callback`（トークンを渡して答えを待つ）。
  - 案件に何をするか：`starts <規則>.<ステートマシン> [then <出来事>, …]`、`sends <出来事>`、`observes`。`refused as <エラー>` は、ステートマシンが出来事を断ったときに返ってくるエラー。
- `case <名前> : <レコード> follows <規則>.<ステートマシン>`：rulec のステートマシンに従う案件。下に `held <入力> = <値>`、`external <出来事>, …`、`state <フィールド>`、`refused when <出力> = <値>` を書く。
- `inputs`・`outputs`：ワークフローの入力と出力。
- `flow`：本体。`on failure`：処理されなかったタスクのエラーのあとに走るブロック。

### 1.4 文

| 文 | 意味 |
|---|---|
| `let x = t(a: …)` | タスクか規則を呼び、答えを `x` に入れる |
| `c <- t(a: …)` | 案件 `c` に対するタスクを呼び、答えを `c` に入れる |
| `on <エラー>, … =>` | 呼び出しの下に書き、そのエラーを受ける。受けたあとは呼び出しの次へ進む |
| `match x.f` | 行き先 `<値>, … =>` を並べる。案件の状態で分けるときは、始まっていない案件の `none` も書ける |
| `wait <n> seconds\|minutes\|hours\|days`、`wait until <timestamp>` | 待つ |
| `repeat at most <n> times` | 上限つきのループ。`break` で抜ける |
| `pass` | 何もしない |
| `succeed <出力> = <値>, …` | 成功で終わる |
| `fail <エラー> ["<理由>"] [leaving <案件>, …]` | 失敗で終わる。`leaving` は、終わっていない案件をそのまま引き渡すという宣言 |

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
| E002 | 名前が無い |
| E003 | 型が合わない |
| E004 | 引数や出力の過不足 |
| E005 | 規則を rulec で読めなかった |
| E006 | 二度の宣言 |
| E007 | タスクの項目の誤り（ステータスの付け忘れ、同じステータスの二つのエラーなど） |
| E008 | 案件の宣言や、案件に何をするかの誤り |
| E009 | 文の置き場所の誤り |
| E010 | `match` に行き先の無い値がある |
| E011 | 通らない行き先 |
| E012 | 値を持っていないことがある変数を読んだ |
| E013 | 始まっていない案件にタスクを呼んだ、始まった案件をもう一度始めた |
| E020 | 終わりでない状態の案件を残して終わる |
| E021 | どの状態でも断られる出来事を送った |
| E022 | 断られることがある出来事の断りを受けていない |
| E030 | 相手の側を変える呼び出しを、`key` なしでやり直す |
| E031 | Express でできないこと（五分を超える待ち、コールバック、`key` なしで相手を変える呼び出し） |
| E040 | 実行履歴が上限を超えうる（Step Functions は 25,000 件、Temporal は 51,200 件、Lambda durable functions は 3,000 操作） |
| E050 | 出力先に出すのに要るものが無い（Step Functions では `lambda` か `http` と `connection`、Step Functions と durable functions では呼ばれる規則の `lambda`） |
| W030 | 相手の側を変えるかもしれない呼び出しを `key` なしでやり直す |
| W101 | 処理されないエラーで、案件を終わりでない状態に残して失敗することがある |
| W102 | 動くことのない `on <断り>` |
| W103 | 案件を始めるタスクに `key` が無い |
| W104 | Step Functions 向けに作れないタスク |

診断には、そうなる例（その点までの短い流れ）が付く。例は、状態・変数・`match` で絞った値ごとに持っているので、例の中で値が途中で変わることはない。

実行履歴の件数は、一回の呼び出し、再試行、Choice、Wait などごとに多めに見積もった件数を足して出す（`check.rs` の `ASL_COST`・`TEMPORAL_COST`・`DURABLE_COST`）。多めに見積もるので、上限の近くでは言い過ぎる側に倒れる。

## 4. 出力先

### 4.1 Step Functions

- `QueryLanguage` は JSONata。JSONPath は出さない。
- ワークフローの変数は全部、ステートマシンの変数にし、始まりの Pass で null にする。入力はそこで `$states.input` から入れ、次の Choice で型を確かめる。外れたら `Dandori.BadInput` で失敗する。
- 呼び出しは Task と、その答えを確かめる Choice の組にする。型が合わなければ `Dandori.BadResponse`、案件の状態が検査の言った集合の外なら `Dandori.UnexpectedState`。
- `match` は Choice にし、どの行き先の条件も明示する。Default は `Dandori.UnexpectedValue` の Fail で、黙って最後の行き先へ流れることはない。
- `retry` は Retry にする。`on` が無いときは、宣言したエラーを `MaxAttempts: 0` で止める retrier を先に置き、そのあとに `States.ALL` を置く。
- `on failure` は、各 Task の最後の Catch（`States.ALL`）から入る。エラーは変数 `dd_error` に入り、ブロックの終わりの Fail がそれで失敗し直す。
- 冪等キーは `実行名/呼び出しの場所/各ループの回数`。HTTP では `Idempotency-Key` ヘッダ、Lambda では `idempotency_key` で渡す。
- `callback` は `lambda:invoke.waitForTaskToken` で、トークンは `task_token` で渡す。
- 規則を呼ぶ Lambda 関数のコード（Python）も出す。rulec が生成した Python を import し、JSON を列挙と整数に直して呼び、答えを JSON に戻す。

### 4.2 Temporal

- `types.ts`（型と検査）、`activities.ts`（タスクのインタフェース）、`rules.ts`（規則を包むアクティビティ）、`runtime.ts`（共通の部品）、`workflow.ts` を出す。
- **再試行は Temporal にさせない。** アクティビティは `maximumAttempts: 1` で呼び、ワークフローのコードが ASL と同じ retrier の並びでやり直す。待ちは `sleep` で、間隔も Step Functions と同じ計算。どの出力先でも、同じエラーを同じ回数だけやり直すためである。
- 宣言したエラーは、アクティビティが投げる `ApplicationFailure` の `type` で見分ける。`TimeoutFailure` は `timeout`、ほかは `failure`。
- `callback` はアクティビティの非同期完了（`CompleteAsyncError` と `client.activity.complete`）で表す。
- `on failure` は、処理されなかったタスクのエラー（`TaskError`）のときだけ走り、`fail` や答えの検査の失敗では走らない。Step Functions で Fail の状態に Catch が効かないのと同じにした。
- 規則は普通のアクティビティとして呼ぶ。rulec の TypeScript を import し、`parse<列挙>` と `BigInt` で直して呼ぶ。規則は純関数なので、決定性のためだけならワークフローのコードで直接呼べるが、そうしない。理由は三つ。
  - 判定の入力と答えが、実行履歴に残る（ActivityTaskScheduled と ActivityTaskCompleted）。どの入力でどう判定したかを後から読め、rulec の `replay` と `diff` に渡す記録にもなる。
  - 規則を直してワーカーを出し直しても、実行中のワークフローの再生が履歴と食い違わない。再生では答えを履歴から読む。
  - 規則のアクティビティを別のワーカーに置けば、ワークフローに触れずに規則だけを出し直せる。Step Functions で Lambda を呼ぶのと同じ形になる。

  費用は、一回につきタスクキューの往復と履歴 6 件。業務のワークフローで判定が数回なら問題にならない。

### 4.3 Lambda durable functions

Temporal と同じく、コードを書き、チェックポイントと再生で続きから動かす形なので、Temporal の生成器と骨組みを共有する（型、答えの検査、やり直しの並び、エラーの種類、`on failure`）。違うのは、タスク・待ち・コールバックの呼び方である。

- `types.ts`（Temporal と同じ）、`tasks.ts`（タスクのインタフェース）、`runtime.ts`、`workflow.ts` と、呼ばれる規則ごとの Lambda 関数（Step Functions 向けと同じ Python）を出す。`workflow.ts` は `makeHandler(tasks)` を出し、利用者は `export const handler = makeHandler(自分のタスク)` と書く。
- **タスクは、利用者の実装を step の中で呼ぶ。** `lambda` を書いたタスクも `context.invoke` にはしない。ローカルのテストランナーでは、invoke の先で投げたエラーの名前が `InvokeError` の中で `Error` になり、宣言したエラーを見分けられなかった。step の中で投げたエラーは、`StepError` の `cause.name` に名前が残る。
- **規則は `context.invoke` で Lambda を呼ぶ。** 規則は宣言したエラーを持たないので、上の問題に当たらない。Temporal で普通のアクティビティにしたのと同じ理由で、入力と答えがチェックポイントに残り、規則だけを出し直せる。
- **再試行は SDK にさせない。** step には `retryStrategy: () => ({ shouldRetry: false })` を渡す（渡さないと SDK の既定のやり直しが効く）。ワークフローのコードが、Step Functions と同じ retrier の並びでやり直し、あいだは `context.wait` で待つ。
- **相手の側を変えて `key` の無いタスクは、「多くとも一回」の step にする**（`StepSemantics.AtMostOncePerRetry`）。既定の「少なくとも一回」では、答えを記録する前に落ちると同じ呼び出しがもう一度走る。「多くとも一回」の step は、中断されると結果の分からない失敗になり、検査はそれを「出来事が起きたか分からない失敗」として扱っている。
- **`callback`** は `context.createCallback` で ID を作り、別の step の中で利用者の実装に `callback_id` を渡す。相手は `SendDurableExecutionCallbackSuccess` で答えを JSON にして返す。期限切れは `CallbackTimeoutError` で、`timeout` になる。
- **今の時刻は step の中で読む。** `wait until` は、step で読んだ時刻（チェックポイントに残る）から待つ秒数を出す。Temporal と違って、ワークフローの中の `Date.now()` は再生のたびに変わる。
- 冪等キーは `context.executionContext.durableExecutionArn` から作る。
- 一回の実行で使える操作は 3,000 回まで（引き上げられない）。E040 はこれも見積もる。

### 4.4 三つの出力先でそろえたこと

| こと | Step Functions | Temporal | Lambda durable functions |
|---|---|---|---|
| 再試行 | Retry | ワークフローのコード（同じ並び、同じ間隔） | ワークフローのコード（同じ並び、あいだは `context.wait`） |
| 答えの検査 | Choice と Fail | `is_<型>` と `dd.fail` | Temporal と同じ |
| 冪等キー | `$states.context.Execution.Name` から | `workflowInfo().workflowId` から | `durableExecutionArn` から |
| タスク | Task（Lambda か HTTP） | アクティビティ | step の中の利用者の実装 |
| 規則 | Lambda と、Python をつなぐコード | アクティビティと、TypeScript をつなぐコード | `context.invoke` と、Step Functions と同じ Lambda |
| on failure | Task の Catch | `TaskError` を受ける catch | Temporal と同じ |

実行中の案件と規則の版の関係は、どの出力先でも同じになる。規則は Lambda かアクティビティとして呼ぶので、規則を直すと、実行中の案件も途中から新しい版で判定される。

## 5. 確かめ方

- **参照インタプリタ**（`dandori run`）：シナリオ（入力と、呼び出しが受け取る答えの並び）に沿って型を付けた構文木を実行し、呼び出しを出力先から見える形（Lambda の関数と payload、HTTP のメソッドと URL とヘッダと本文、Temporal のアクティビティと引数、durable functions の step と引数・コールバックの submit・invoke の先と payload）で書き出す。
- **シナリオの自動生成**（`dandori scenarios`）：選んだ答えの記録を持ってワークフローを実行し直す。選び方は、選択の場所ごとに、まだ使われていない選択肢を先に取る。何か新しい所に着いた実行を土台にして、その途中の選択を一つずつ変えた実行を試し、そこから先は同じ場所で同じ選択を繰り返す（ループが回り切る流れはこれで出る）。答えの検査に落ちる答え（型の合わない答え、ありえない状態）も入れる。
- **Step Functions の突き合わせ**：生成した ASL を `tools/asl-run.mjs` が JSONata 2.0.6 で実行し、参照インタプリタと同じ形で書き出す。全シナリオで一致を見る。定義は asl-validator 4.0.0 にも通す。
- **Temporal の突き合わせ**：生成した TypeScript を `tools/temporal/run.mjs` が SDK 1.24.0 の時間を飛ばすテスト環境で実行し、シナリオどおりに答えるアクティビティで呼び出しを書き出す。タイムアウトはこの環境のシナリオでは起こせないので、タイムアウトを含むシナリオは外す。
- **Lambda durable functions の突き合わせ**：生成した TypeScript を `tools/durable/run.mjs` が SDK 2.4.0 のローカルのテストランナー（`@aws/durable-execution-sdk-js-testing` 1.1.4、時間を飛ばす）で実行する。タスクはシナリオどおりに答える実装で、規則は `registerFunction` で登録した関数で、コールバックはその ID に答えを送って置き換える。このランナーは答えの来ないコールバックを期限切れにしないので、タイムアウトを含むシナリオは外す。
- **つなぐコード**：Lambda の Python と `rules.ts` を、rulec が生成した Python と TypeScript と一緒に動かし、`rulec vectors` の全件で rulec の期待値と比べる。durable functions 向けに出す規則の Lambda が、Step Functions 向けのものと一字も違わないことも確かめる。
- **診断**：`tests/fixtures` の各ファイルの診断を、英語と日本語の両方で固定する（`DANDORI_BLESS=1` で書き直す）。

### 5.1 走らせた（2026-09-25）

- 例は二つ。ホテルの予約（Stripe の PaymentIntent）と、倉庫の注文の出荷（rulec の「注文の状態」）。
- シナリオは 49 本と 33 本。Step Functions では 82 本すべてが参照インタプリタと一致した。Temporal と Lambda durable functions では、タイムアウトを含む 9 本を除く 73 本すべてが一致した。
- 生成器にわざと誤りを入れると（ASL の冪等キーの区切りを一文字変える、`match` の条件から値を一つ落とす、durable functions の冪等キーの呼び出しの場所をずらす）、どれも一本目のシナリオで食い違いとして出た。
- つなぐコードは、与信額の規則のベクタ 24 件と、急ぎの規則のベクタ 16 件で、rulec の期待値と一致した。
- 検査が例を書く途中で見つけたこと。チェックアウトまで待つあいだに与信の期限が切れると capture が断られる。capture のあとは `processing` を経て `requires_payment_method` に戻ることがある。`requires_action` を見てから cancel するまでに、客が認証を済ませて期限が切れると、cancel も断られる。入金がないので注文の取消を頼むあいだに、客が自分で取り消していると、倉庫はその取消依頼を断る。

## 6. 捨てたもの

- **rulec に workflow の宣言を足すこと。** 0.2 のとおり。
- **JSONPath を出すこと。** 変数と JSONata があれば、ResultPath の組み替えなしにデータを運べる。
- **Temporal の再試行に任せること。** Temporal は、型を名指しして「これだけをやり直す」とは書けない（やり直さないものを名指しする）。Step Functions と同じ回数にならない。
- **`match` の最後の行き先を Default にすること。** 検査の外の値が、黙ってその行き先へ流れる。
- **条件式を dandori に入れること。** P1 のとおり。条件は rulec の表に書けば、網羅と重なりが rulec の証明書つきで閉じる。
- **Temporal で、規則をワークフローのコードで直接呼ぶこと。** 履歴は増えないが、判定の記録が残らず、規則を直すたびにワーカーの版の固定（Worker Deployment Versioning の `PINNED`）が要る。固定しないと、実行中のワークフローの再生が新しい規則で判定し直し、答えが変わった分岐で止まる。
- **Temporal で、規則をローカルアクティビティにすること**（いまは）。往復と履歴は減るが、規則をワークフローと同じワーカーに置くことになり、規則だけを出し直せなくなる。
- **durable functions で、`lambda` のタスクを `context.invoke` で呼ぶこと。** 利用者のコードが要らなくなるが、4.3 のとおり宣言したエラーを見分けられない（ローカルのテストランナーで確かめた。AWS の上での振る舞いは確かめていない）。
- **durable functions で、step の `retryStrategy` にやり直しを任せること。** step だけなら関数一つで Step Functions と同じ並びを書けるが、`context.invoke` にはやり直しの設定が無い。規則の呼び出しはどのみちワークフローのコードでやり直すので、step もそれにそろえた。

## 7. まだやっていないこと

- リストと、Map・Parallel。案件を二つの枝から同時に動かすことは、並行の問題になるので断る予定。
- AWS SDK の統合と、AWS が公開している Smithy の API モデルからの型の取り込み。
- ワークフローが自分で持つ案件（rulec の生成した関数を呼んで次の状態を得る形）。
- 規則の前提（rulec の `preconditions`）を、判断に渡す値を作ったタスクの直後で確かめること。
- 規則を ASL の中に JSONata として埋めること。rulec の側の新しい出力先になり、数を 2^53 までに収める証明が要る。
- Temporal と durable functions のシナリオでタイムアウトを起こすこと。
- durable functions の Python（`aws_durable_execution_sdk_python`）への出力。
- AWS の上（Step Functions の TestState と、durable functions の CloudDurableTestRunner）と Temporal のサーバーの上での確認。
- 実行中の案件が、規則やワークフローの改定でどうなるか（rulec の `diff` と `replay` を実行履歴に当てる）。
- `dandori explain` と、`build` の JSON 出力。
- ループの中で何度も判定して履歴の上限に近づくワークフローのために、規則をローカルアクティビティで呼ぶ形を選べるようにすること。
