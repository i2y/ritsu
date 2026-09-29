# タスクが呼ぶもの

タスクには、何をどう呼ぶかを一行で書きます。呼び出しのコードは、dandori が各プラットフォーム向けに生成します。Temporal、Lambda durable functions、Argo Workflows、pydantic-graph 向けに生成するコードは、AWS Step Functions が送るのと同じリクエストを送ります。そのため、同じ `.flow` はどこで動かしても同じリクエストを出します。

## 呼び出し方

| タスクに書くもの | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| `lambda "<関数>"` | Lambda の Task | 生成したアクティビティが関数を呼ぶ | 生成したステップが関数を呼ぶ | 生成したコードをコンテナで動かす | 生成した関数が関数を呼ぶ |
| `http POST "<URL>"` | HTTP Task | 生成したアクティビティ（`fetch`） | 生成したステップ（`fetch`） | 同じく `fetch` | 同じく urllib |
| `connect <API> "<Service>/<Method>"` | HTTP Task（Connect の JSON） | 生成したアクティビティ（`fetch`） | 生成したステップ（`fetch`） | 同じく `fetch` | 同じく urllib |
| `aws sns:publish` | AWS SDK の統合 | 生成したアクティビティ（AWS SDK） | 生成したステップ（AWS SDK） | 同じく AWS SDK | 同じく boto3 |
| `agent …`（[エージェント](agents.md)） | モデルの API への HTTP Task | 生成したアクティビティ | 生成したステップ | 同じ | 同じ |
| `jev …`（[Jev](jev.md)） | TypeSafe の API への HTTP Task | 生成したアクティビティ（`fetch`） | 生成したステップ（`fetch`） | 同じく `fetch` | 同じく urllib |
| `state machine "<ARN>"` | ネストした実行（`startExecution.sync:2`） | | | | |
| `flow "<パス>"`（[子の .flow](#子の-flow)） | ネストした実行（`state machine` も書く） | 子のタスクキューで子ワークフロー `<名前>_v<バージョン>` を始める | invoke（`durable function` も書く） | 子の WorkflowTemplate から Workflow を作る | 自分で書く関数 |
| `workflow "<型>"` | | 子ワークフロー | | | |
| `durable function "<ARN>"` | | | ほかの durable function を invoke する | | |
| `workflow template "<名前>"` | | | | その WorkflowTemplate から Workflow を作る | |
| `image "<イメージ>"` | | | | 自分のイメージのコンテナ | |
| `event` | ビルドできない（E050） | 何も呼ばず、名前を宛先にして送られてくる値を待つ | ビルドできない（E050） | ビルドできない（E050） | ビルドできない（E050） |
| どれも書かない | ビルドできない（E050） | 自分で書くアクティビティ（`OwnTasks`） | 自分で書くコードを動かすステップ（`OwnTasks`） | ビルドできない（E050） | 自分で書く関数（`OwnTasks`） |

生成した呼び出しは、どれも `Transport`（`io.ts`、`io.py`）を通ります。HTTP の API のヘッダ、AWS SDK のクライアント、エージェントの API キー、Jev のための TypeSafe の API キーといった認証や接続の設定は、ここで渡します。テストでは、ここを差し替えます。

`queue "<名前>"` を書くと、Temporal のアクティビティや子ワークフローを、そのタスクキューへ送ります。そのキューは、どちらの言語のワーカーでも受け持てます。

`use rule` の下に `local` と書くと、Temporal は規則をローカルアクティビティとして、ワークフローを動かしているワーカーの中で呼びます。一回の呼び出しで履歴に残るのは、通常のアクティビティなら六つになるイベントではなく、マーカー一つだけです。ループの中で何度も規則を呼ぶフローで効きます。

`callback` のタスクは、呼び出し先に応答の返し先を渡し、応答が後から届くのを待ちます。返し先は、Step Functions ならタスクトークン、durable functions ならコールバックの ID です。Temporal でも ID を渡し、応答は Update で届きます。Argo では、応答は `argo node set` で届きます。`aws sqs:sendMessage` なら、トークンをメッセージに入れて送ります。

`event` と書いたタスクは、何も呼びません。ワークフローは、自分のワークフロー ID とタスクの名前を宛先にして送られてくる値を待ちます。承認のツールや運送会社の Webhook のように、どの注文についての通知かは分かっていても、コールバックのトークンは持っていない送り手に向いています。ワークフローは、待っているあいだに届いたものだけを受け取り、それ以外は拒否します。送った側は拒否されたことが分かるので、あとで送り直せます。実行に名前で値を送れるのは Temporal だけなので、ほかのプラットフォーム向けにはビルドできません（E050）。

Argo Workflows では、どのタスクもコンテナで動きます。生成した呼び出しは、`caller/` から作るイメージに入ります。`image` を書いたタスクは、そのイメージのコンテナで動きます。コンテナは呼び出しの中身を環境変数 `DANDORI_CALL` から読み、結果を `/tmp/dandori/answer.json` に書きます。宣言したエラーで終えるときは、`{"error", "message"}` を `/tmp/dandori/error.json` に書き、終了コード 3 で終わります。

## API の記述

タスクが呼ぶ API に記述があれば、`.flow` でその記述を読み込み、タスクが記述と合うかを検査できます（E016）。

```flow
use openapi stripe from "../specs/stripe.json"
use smithy sns from "../specs/sns.json"
use proto warehouse from "../specs/warehouse.proto"
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

読める記述は三種類です。

- **OpenAPI 3 の文書（JSON）**：`http` のタスクが呼ぶ操作を、この文書から引きます。URL は文書のサーバーとパスから決まり、本文をフォームの形式（URL エンコード）で送るかどうかも文書に従います。
- **AWS の API の Smithy のモデル**（AWS が公開している JSON の AST）：`aws` のタスクとは、サービスの名前で結びつきます。`aws sns:publish` なら、SNS の `Publish` の定義と照らし合わせます。
- **`.proto`（proto3）**：`connect` のタスクが呼ぶメソッドを、ここから引きます。呼び出しは JSON を使う Connect プロトコルで、`<url>/<パッケージ>.<Service>/<Method>` に `Connect-Protocol-Version: 1` を付けて POST します。Connect のエラーは `resource_exhausted` のようなコードで宣言し、実行時には、コードに対応する HTTP ステータスで見分けます。

検査は、引数が操作と合うか（受け取らない引数を渡していないか、要る引数がそろっているか、型と範囲が合うか）と、結果の型が操作のレスポンスに合うかを確かめます。レスポンスが省くことのあるフィールドや、null になることのあるフィールドは `T?` にし、列挙には、レスポンスに現れうる値をすべて持たせます。宣言するエラーは、操作が実際に返すものに限られます。AWS の API で `key <引数>` に使えるのは、その操作の冪等トークンだけです。

Stripe と在庫のサービスを呼ぶ下書き
（[tests/fixtures/api_calls.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/api_calls.flow)）を `--lang ja` で検査すると、こうなります。

<div class="dd-term" markdown>

```text
エラー[E016]: tests/fixtures/api_calls.flow:23:1: `stripe` の POST /v1/payment_intents のレスポンスは `PaymentIntent` に合いません。`created` で、整数は `timestamp` ではありません
    23 | task create_intent(amount: int, currency: string, capture_method: CaptureMethod) -> PaymentIntent
エラー[E016]: tests/fixtures/api_calls.flow:26:1: `warehouse` の StockService/Reserve のレスポンスは `Reservation` に合いません。`count` で、64 ビットの整数は、protobuf の JSON では文字列で来ます。`string` にしてください
    26 | task reserve_stock(sku: string, quantity: int) -> Reservation
```

</div>

protobuf の JSON では、`optional` の付かないフィールド（メッセージ型を除く）は、値がゼロ値（空の文字列、0、false、列挙の最初の値、空のリストやマップ）のとき省かれます。protobuf で読む側は、省かれたフィールドをゼロ値として読みます。`connect` のタスクのレスポンスも、どのプラットフォームでも同じように読みます。生成したコードは、レスポンスのメッセージと、その中やリストの中のメッセージに、`.proto` をもとにゼロ値を埋めてから、結果の型を確かめます。

検査が記述から読むのは、タスクの型に関わる部分だけです。そのため、8 MB ある Stripe の文書もそのまま読めます。例には、要る部分だけを残した抜粋を置いています。

## 子の .flow

子ワークフローも dandori で書いたなら、タスクにその `.flow` のパスを書けます（`flow "arrange_delivery.flow"`）。このとき検査は、子そのものを検査したうえで、タスクが子と合うかを確かめます（E015）。タスクは、子が要る入力をすべて、子が受け取れる型と範囲で渡さなければなりません。結果の型は、子の出力をフィールドに持つレコードにします。宣言するエラーは、どれも子が `fail` するものでなければなりません。親と子はレコードや列挙を別々に宣言するので、型は名前ではなく中身で比べます。レコードはフィールドを、列挙は値を、範囲はその広さを比べ、値が渡る向きに収まるかを見ます。

配達のタスクが子と合っていない、引当と発送の下書き
（[tests/fixtures/fulfillment_child.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/fulfillment_child.flow)）を検査すると、こうなります。

<div class="dd-term" markdown>

```text
エラー[E015]: tests/fixtures/fulfillment_child.flow:18:1: 引数 `carrier` は、`arrange_delivery` が入力 `carrier` として受け取るものと合いません。`Carrier` の `drone` は `carrier` の値にありません
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
エラー[E015]: tests/fixtures/fulfillment_child.flow:18:1: `arrange_delivery` が返す `tracking_number` は、`Delivery` のフィールド `tracking_number` と合いません。`string` は `int` ではありません
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
```

</div>

子の名前は、子の `.flow` から決まります。Temporal では、タスクは子のワークフローの型（`arrange_delivery_v1`）を、同じ名前のタスクキューで始めます。そのキューでは、子から生成したワーカーが待っています。Argo では、子の WorkflowTemplate から Workflow を作ります。Step Functions と Lambda durable functions では、子をデプロイした先の ARN も書きます（`state machine`、`durable function`）。自分自身を走らせるフローは、直接でも、ほかのフローを経由してもエラーになります。再帰は書けません。
