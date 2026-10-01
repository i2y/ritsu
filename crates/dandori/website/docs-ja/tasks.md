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

protobuf の JSON では、`optional` の付かないフィールド（メッセージ型を除く）は、値がゼロ値（空の文字列、0、false、列挙の最初の値、空のリストやマップ）のとき省かれます。protobuf で読む側は、省かれたフィールドをゼロ値として読みます。`connect` のタスクのレスポンスも、どのプラットフォームでも同じように読みます。生成したコードは、レスポンスのメッセージと、その中やリストの中のメッセージに、`.proto` をもとにゼロ値を埋めてから、結果の型を確かめます。値が設定されていない `google.protobuf.Value` も省かれますが、こちらは `json` のフィールドになり、無ければ null として読みます。

検査が記述から読むのは、タスクの型に関わる部分だけです。そのため、8 MB ある Stripe の文書もそのまま読めます。例には、要る部分だけを残した抜粋を置いています。

### .proto から型を作る

`use proto` で読んだ `.proto` のメッセージと列挙は、`use` に付けた名前のあとに書けば、そのまま型になります。`warehouse.ReserveResponse` のように書きます。書ける場所はほかの型と同じです（レコードのフィールド、タスクの引数と結果、`inputs` と `outputs`、`let x: T`、`list[…]` と `T?` の中、案件のレコード）。記述がすでに言っていることを、レコードに手で写さずに済みます。引当と発送の例では、倉庫の記述がこう言っています。

```proto
enum Stock {
  unspecified = 0;
  secured = 1;
  short = 2;
}

message ReserveResponse {
  string sku = 1;
  Stock stock = 2;
  optional string id = 3;
}
```

フローはこれをそのまま使います。

```flow
record PackingRequest
  order_id     : string
  reservations : list[warehouse.ReserveResponse]

task reserve_stock(sku: string, quantity: int) -> warehouse.ReserveResponse
  connect warehouse "StockService/Reserve"
  errors busy = resource_exhausted
```

`warehouse.ReserveResponse` は、`sku : string`、`stock : warehouse.Stock`、`id : string?` を持つレコードで、`warehouse.Stock` は `secured | short` の列挙です。このタスクは、これだけで記述と合います。型を作る表と、タスクを記述と突き合わせる表は同じものだからです。

| `.proto` | フローの型 |
|---|---|
| `string`、`bytes` | `string` |
| `bool` | `bool` |
| 32 ビットの整数（`int32`、`uint32`、`sint32`、`fixed32`、`sfixed32`） | `int` |
| 64 ビットの整数 | `string`（protobuf の JSON が文字列で書くため） |
| `float`、`double`、`map` | `json` |
| 列挙 | 列挙。値は `.proto` の名前のまま |
| メッセージ | レコード。フィールドの名前は protobuf の JSON の名前（`json_name`、なければ lowerCamelCase） |
| `repeated T` | `list[T]` |
| `optional` を付けたフィールド、メッセージ型のフィールド、`oneof` の一つ | `T?` |
| `google.protobuf.Timestamp` | `timestamp` |
| `Struct`、`Value`、`ListValue`、`Any` | `json` |
| `Empty` | フィールドの無いレコード |
| `Duration`、`FieldMask` | `string` |
| ラッパー（`Int32Value` など） | 中の型。`Int64Value` は `string`、`DoubleValue` は `json` |

- 入れ子のメッセージは `warehouse.Order.Line` と書きます。ファイルが import しているほかのパッケージの型は、パッケージから書きます（`warehouse.common.v1.Money`）。作るのは、フローが名前を書いた型と、そこからたどれる型だけです。自分自身を含むメッセージ（`repeated Tree children` を持つ木のようなもの）は、直接でもほかのメッセージを通してでも、レコードにできず、E003 になります。自分を含むところを `json` にしたレコードを、自分で書いてください。`.flow` に書いたレコードも、同じ理由で自分自身を含められません。シナリオの生成と dandori が書くコードは型を終わりまでたどりますが、自分を含む型には終わりがないからです。
- `.proto` にない名前は E002 になり、`.proto` にあるメッセージと列挙の名前が添えられます。サービスの名前も E002 です。OpenAPI の文書や Smithy のモデルの名前も同じで、この二つからは型を作りません。

<div class="dd-term" markdown>

```text
エラー[E002]: tests/fixtures/proto_types.flow:27:11: 型 `types.Stok` はありません
    27 |   stock : types.Stok
  = `types` のメッセージと列挙は Box・Everything・Everything.Nested・GetRequest・Line・Mode・Status・Ticket です
```

</div>

- 数の範囲は、Protovalidate の指定から読みます。`(buf.validate.field).int32` の下の `gte`・`gt`・`lte`・`lt`・`const` を読み、一つのオプションにまとめて書いても、一つずつ書いても同じです。リストの中の数は `repeated.items` から読みます。0 のときは検証しない指定（`IGNORE_IF_ZERO_VALUE`）があるフィールドは、0 も範囲に入ります。低いほうの端が高いほうの端より大きい指定は「その外側」の意味になり、一つの範囲にはならないので、範囲としては読みません。読んだ範囲はレコードのそのフィールドの範囲になるので、[範囲](tour.md#範囲)の決まりがそのまま当てはまります。範囲を外れることのある値は E014、範囲が分からない値は W104 で、範囲のあるフィールドに、範囲のない値を送るタスクは E016 です。`optional` やメッセージ型のフィールドでも、`required` が付いていれば `T?` ではなく `T` になります。ほかの規則（文字列の長さ、リストの件数、CEL）は、無いものとして読みます。

<div class="dd-term" markdown>

```text
エラー[E014]: tests/fixtures/proto_ranges.flow:35:1: `line.quantity` は `>=1 <=99` で、`few` の引数 `n` の範囲 `>=1 <=5` を外れることがあります
    35 |   few(n: line.quantity)
エラー[E014]: tests/fixtures/proto_ranges.flow:41:1: `many` は `>=1 <=200` で、`types.Box` のフィールド `count` の範囲 `>=1 <=10` を外れることがあります
    41 |   let too_many: types.Box = {count: many}
```

</div>

- 列挙のゼロ値を外すのは、その名前が「値が無い」ことを言っているときだけです。列挙の名前を大文字にした接頭辞（`Stock` なら `STOCK_`）を取ったあとが、大文字小文字を問わず `unspecified` になる名前がそうです。protobuf では、フィールドが設定されていないことを表す印で、`match` で分ける値ではありません。この値が返ってきたら、`Dandori.BadResponse` で失敗します。`ACTIVE = 0` のようにほかの名前のゼロ値は本当の値なので、残します。
- 値が設定されていない `Value` は、protobuf の JSON から省かれます。そのため、`json` のフィールドは無いことがあります。無ければ、`T?` と同じように null として読み、そのフィールドを引数に取るタスクには `null` を渡します。レコードをそのまま渡すときは、フィールドが無いまま渡ります。ワークフローの `json` の入力も、始めるときに省かれていれば、null として読みます。
- 生成するコードでは、型の名前の `.` を `_` にします（`warehouse_ReserveResponse`）。フローのほかの型が同じ名前になるときは E006 です。`dandori doc` では、`.flow` に書いたとおりの名前で表示します。
- `url` が要るのは、タスクがサービスを呼ぶとき（`connect`）だけです。型を作るためだけに読む `.proto` には要りません。
- `.proto` が、ディスクにないファイルを import していることがあります。たとえば `google/api/annotations.proto` はオプションのために import するもので、フローが使う型は何も持っていません。そのため `.proto` はそのまま読み、そのファイルは飛ばします。飛ばしたファイルだけが持つ型は分かりません。その型のフィールドを持つメッセージから型は作れず（E002）、その型を受け取る、または返すメソッドをタスクと合わせることもできません（E016）。診断には、読めなかったファイルの名前が添えられます。

<div class="dd-term" markdown>

```text
エラー[E002]: tests/fixtures/proto_unread.flow:11:12: `catalog.Priced` を作れません。フィールド `price` の型 `google.type.Money` が分かりません。レコードを自分で書き、`price` は `json` にしてください
    11 |   priced : catalog.Priced
  = `google/api/annotations.proto`・`google/type/money.proto`・`shop/v2/cancel.proto` を読めなかったので、そこにある型は使えません
```

</div>

## 規則をサービスとして呼ぶ

規則のコードは、既定ではワークフローと一緒に出します。Step Functions では rulec が生成した Python を包んだ Lambda 関数、Temporal ではその TypeScript か Python を包んだアクティビティです。もう一つの形は、`rulec gen` が規則のために書く Connect のサービスを、どのプラットフォームからも呼ぶ形です。規則が一か所にあるので、規則を直すと、それを呼ぶすべてのワークフローに届きます。

```flow
use rule urgency from "../rules/urgency.rule"
  connect "https://rules.example.com"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/rules/5e6f7a"
```

`connect` はサービスの場所です。`connection` は、Step Functions の HTTP Task がサービスを呼ぶときに通す EventBridge の接続で、`connect` と一緒にだけ書けます。規則の呼び出し方は `lambda` か `connect` のどちらか一つで、両方は書けません（E007）。`local` は `connect` と一緒に書けて、Temporal ではローカルアクティビティになります。

<div class="dd-term" markdown>

```text
エラー[E007]: tests/fixtures/rule_connect.flow:6:3: この規則の呼び出し方はもう書かれています（5 行目）。規則の呼び出し方は `lambda` か `connect` のどちらか一つです
     6 |   connect "https://rules.example.com"
```

</div>

dandori は、規則のほかの情報と同じように、サービスについても `rulec api` だけを読みます。`rulec gen` が書く `.proto` は読みません。読むのは、メソッドのパス、リクエストとレスポンスのフィールド、列挙の値ごとにサービスが使う名前です。呼び出しは、URL とパスに `Connect-Protocol-Version: 1` のヘッダを付けた POST で、本文は protobuf の JSON の形で書いた JSON です。フィールド名は lowerCamelCase、数は十進の文字列（rulec の数は 64 ビットの整数です）、列挙の値は `rulec api` が言う `.proto` での名前です（`next_day` なら `CARRIER_NEXTDAY`）。入力は、ゼロ値（`false` や `"0"`）でも省かずに書きます。サービスは、省いた入力とゼロ値を区別するからです。急ぎの規則のために `rulec gen` が書いたサービスに、実際に送ったものと、返ってきたものは次のとおりです。

```text
POST /rulec.urgency.v1.UrgencyService/Decide
Connect-Protocol-Version: 1
Content-Type: application/json

{"member":false,"amount":"5000"}

200 OK
rulec-source-sha256: 5ff6efc93a9b39d25225c10aa67460104da8e393a052635b225d497c51b7078f

{"carrier":"CARRIER_STANDARD","trace":[{"table":"decide","row":3}]}
```

- サービスは、ゼロ値のフィールドをレスポンスの JSON から省きます（protobuf の JSON と同じです）。この例では、false の `urgent` がありません。dandori は、protobuf が読むのと同じように、省かれたものを戻します（false、0、空の文字列、列挙の 0 番の値）。そのうえで、規則自身のレコードとして読みます。数は十進の文字列から、列挙の値は規則での名前から読み替えます。便の列挙の 0 番は `CARRIER_UNSPECIFIED` で、規則のどの値でもありません。合わないものはそのまま残します。すべての規則の結果が通す検査（型と、rulec が出力ごとに出す範囲）が、`Dandori.BadResponse` で呼び出しを終えます。たとえば、規則の値でない列挙の 0 番、どの値にもない名前、十進でない数、2^53 − 1 を超える数、オブジェクトでない本文です。当たった行を表す `trace` は読みません。
- 200 以外のステータスと、リクエストが届かないことは、呼び出しの失敗です。規則には宣言するエラーがないので、エラーは `failure` だけです。ほかの規則の呼び出しと同じようにリトライし、回数は 2 回、間隔は 1 秒と 2 秒です。
- レスポンスを返したサービスの規則のバージョン（ヘッダ `rulec-source-sha256`）は見ません。検査したのと違う規則で動いているサービスでも、呼び出しは失敗しません。サービスを一か所にするのは、規則を直したときにワークフローを出し直さずに済ませるためだからです。守られているのは、レスポンスのたびに確かめる型と範囲と、パスの `v1` です。`v1` は契約のバージョンで、`buf breaking` が見張ります。

| プラットフォーム | サービスで呼ぶ規則 |
|---|---|
| Step Functions | `connection` を通す HTTP Task（`connection` が無いとき、URL が HTTPS でないときは E050）。Lambda 関数は書きません |
| Temporal | 生成したアクティビティ。名前は規則のコードを同梱するときと同じ `rule_<規則>` のままで、`Transport` を通して送ります。ワークフローも、残る履歴も、同梱のときと変わりません |
| Lambda durable functions | `Transport` を通して送るステップ |
| Argo Workflows | ほかの規則と同じく caller のイメージ。`Transport` を通して送ります |
| pydantic-graph | `Deps.tasks` の関数。`Transport` を通して送ります |

### 契約から取り込んだ列挙

規則は、列挙を `.proto` から取り込むことがあります（規則の `import proto`）。その値の名前は契約が決めるので、buf の慣わしに従うとは限りません。0 番が、ほかと同じ一つの値のこともあります。

```proto
enum Status {
  ACTIVE = 0;
  CLOSED = 1;
}
```

そのため dandori は、この名前を列挙の名前から組み立てずに、`rulec api` から読みます。サービスは、0 番の値の列挙を、ほかのゼロ値と同じくレスポンスから省きます。0 番が規則の値なら、dandori は、列挙の無いレスポンスをその値として読みます。送るときは、`ACTIVE` もほかの値と同じく名前を書きます。`tests/fixtures/rules/account_fee.rule` の規則はこの列挙を取り込んでいて、そのサービスは、状態が `ACTIVE` のままで手数料の掛からない結果を、当たった行だけで返します。

```text
POST /rulec.account_fee.v1.AccountFeeService/Decide
Connect-Protocol-Version: 1
Content-Type: application/json

{"state":"ACTIVE","balance":"20000"}

200 OK
rulec-source-sha256: bdb2f090c3a1a0c471d0b94dd4f23db7aa7a4d9bd8c23bdd786effd3b0db16ba

{"trace":[{"table":"手数料表","row":1}]}
```

`rulec api` が `connect.enums` でこの名前を言うのは rulec 0.22.0 からで、0.21.2 までの rulec は言いません。その rulec では、契約から列挙を取り込んだ規則をサービスで呼べず、`check` がそう言います。規則自身の列挙は、その rulec が付ける名前で、いままでどおり呼べます。

<div class="dd-term" markdown>

```text
エラー[E005]: tests/flows/connect_rules_contract.flow:4:10: 規則 `../fixtures/rules/account_fee.rule` を読めませんでした
     4 | use rule 手数料 from "../fixtures/rules/account_fee.rule"
  = 規則の列挙 `口座の状態` は契約（`Status`）から取り込んだものですが、この rulec の `rulec api` は、規則のサービスがその値を何と呼ぶかを言いません。`connect` で呼ぶには、rulec 0.22.0 以降が要ります（`rulec api` の `connect.enums` がそれを言います）
```

</div>

### サービスが断るもの

rulec 0.22.0 以降が書くサービスは、範囲の外の入力、列挙に無い名前、リクエストに無いフィールド、入力を省いたリクエストを、`invalid_argument`（400）で断ります。dandori は、検査したのと同じ規則で動くサービスには、どれも送りません。入力の名前や列挙の値を変えた別のバージョンの規則で動くサービスは、ゼロ値で判断せずに呼び出しを断ります。呼び出しは、200 以外のステータスのときと同じく失敗します。rulec 0.21.2 が書くサービスは、知らないフィールドを読み飛ばし、省いた入力をゼロ値として読み、列挙に無い名前を 0 番の値として読みます。最後のものを断るのは、0 番が「何も設定されていない」を意味する列挙のときだけです。

要素の並びをたどる規則（規則の `elements`）は、サービスでも、規則のコードを同梱しても呼べません。dandori はまだ、規則に並びを渡せないからです（E005）。

AWS 向けの注文の例は、急ぎの規則をこの形で呼びます（[order.flow](https://github.com/i2y/dandori/blob/main/examples/order/aws/order.flow)）。`rulec gen` が書くサービスに、dandori が送るものを実際に送って確かめるテストもあります（[どうやって確かめているか](assurance.md)）。

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
