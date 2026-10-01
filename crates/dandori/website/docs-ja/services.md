# サービスを実装する

ワークフローの入口は、標準の proto3 のサービスとして書けます。一回の実行が何を受け取って始まり、何を返して終わるか、どんな名前で失敗しうるか、途中で何を受け取るかを、サービスとして書きます。`.flow` にはそのサービスを実装すると書き、検査は、タスクを呼び先の API の記述と照らし合わせるのと同じように、ワークフローをサービスと照らし合わせます。`.proto` は普通の `.proto` のままなので、protoc や buf でそのまま扱え、ほかの言語のクライアントも、型をこの `.proto` から作れます。dandori の印は、dandori 専用のオプションで付けます。

## dandori のオプション

オプションは、リポジトリの [proto/dandori/v1/options.proto](https://github.com/i2y/dandori/blob/main/proto/dandori/v1/options.proto) にあります。このファイルを、自分の proto のルートに `dandori/v1/options.proto` としてコピーし、サービスを書いたファイルから import してください。dandori 自身は、Google の well-known types と同じく、コピーがなくてもこのファイルを知っています。コピーが要るのは protoc と buf です。このファイルは buf の標準の lint を通ります。

サービスには、それを実装するワークフローの名前を書きます。メソッドには、実行にどう届くかを、次の四つのオプションのどれか一つで書きます。

| オプション | メソッドがすること |
|---|---|
| `(dandori.v1.start)` | 実行を始める。リクエストがワークフローの入力、レスポンスが出力になり、`fails` には `.flow` の `fail` の名前を並べる |
| `(dandori.v1.event)` | `event` のタスクが待つ値を、実行に送る。リクエストがその値になる |
| `(dandori.v1.answer)` | `callback` のタスクに応答する。リクエストが応答になる |
| `(dandori.v1.status)` | 実行がいまどこにいるかを聞く。レスポンスは `dandori.v1.Status` か、同じフィールドを持つメッセージ |

引当と発送の例は、次のサービスを実装しています
（[fulfillment.proto](https://github.com/i2y/dandori/blob/main/examples/fulfillment/specs/fulfillment.proto)）。

```proto
import "dandori/v1/options.proto";

service FulfillmentService {
  option (dandori.v1.workflow) = {name: "fulfillment", version: 1};

  rpc Fulfill(FulfillRequest) returns (FulfillResponse) {
    option (dandori.v1.start) = {fails: ["OutOfStock", "DeliveryFailed", "PackingLate"]};
  }

  rpc AnswerPacking(AnswerPackingRequest) returns (AnswerPackingResponse) {
    option (dandori.v1.answer) = {task: "wait_for_packing"};
  }
}
```

どの版も、最初の行にそう書いてあります。

```flow
workflow fulfillment v1 implements shop.FulfillmentService
use proto shop from "../specs/fulfillment.proto"
```

`shop` は `use proto` で `.proto` に付けた名前で、`FulfillmentService` はその中のサービスです。一つのワークフローが実装するサービスは一つです。この `.proto` を呼ぶタスクは無いので、`url` は要りません。

日本語の版は、JSON での名前（`json_name`）を日本語にした別の記述（`fulfillment.ja.proto`）のサービスを実装します。ワークフローや入力の名前が日本語の版と同じになるようにするためです。

## 何を照らし合わせるか

検査は、ワークフローとサービスの食い違いを、一つずつ別の診断で知らせます（E017）。

- サービスの `(dandori.v1.workflow)` が、`workflow` の行と同じワークフローの名前とバージョンを書いていること。
- どのメソッドにも四つのオプションのどれか一つがあり、メッセージを一つ受け取って一つ返すこと（ストリームは使えない）。実行を始めるメソッドは一つだけ。
- 実行を始めるメソッドのリクエストのフィールドが、protobuf の JSON での名前（`json_name`、無ければ lowerCamelCase）で入力と一つずつ対応し、入力がフィールドの値を読めること。型の比べ方は、タスクを API の記述と照らし合わせるときと同じです。64 ビットの整数は `string` で読み、列挙はフィールドがとりうる値をすべて持ちます。
- そのレスポンスのフィールドが出力と一つずつ対応し、出力の値をフィールドに書けること。無いことがある出力（`T?`）は、設定されているかどうかが分かるフィールドにしか書けません。
- `fails` に、`.flow` が失敗するときの名前がすべて並び、それ以外の名前が無いこと。`.proto` にはエラーを書く場所が無いので、クライアントは失敗の名前をここから知ります。
- `(dandori.v1.event)` のメソッドには `event` のタスクの名前を、`(dandori.v1.answer)` のメソッドには `callback` のタスクの名前を書き、そのタスクの結果の型でメソッドのリクエストを読めること。メソッドは何も返さないこと。一つのタスクの名前を書けるメソッドは一つだけ。
- `(dandori.v1.status)` のメソッドが何も受け取らず、`dandori.v1.Status` と同じものを返すこと。
- ファイルが `dandori/v1/options.proto` を import していること（protoc と buf がそれを求めます）。

サービスとワークフローがずれてしまった下書き
（[tests/fixtures/service.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/service.flow)）を `--lang ja` で検査すると、たとえば次の診断が出ます。

<div class="dd-term" markdown>

```text
エラー[E017]: tests/fixtures/service.flow:1:36: ワークフローは `PackingLate` で失敗することがありますが、`Fulfill` の `fails` にありません
     1 | workflow fulfillment v1 implements shop.FulfillmentService
エラー[E017]: tests/fixtures/service.flow:1:36: `Fulfill` の `fails` に `Lost` がありますが、ワークフローがその名前で失敗することはありません
     1 | workflow fulfillment v1 implements shop.FulfillmentService
```

</div>

型は名前ではなく形で比べます。入力を `.proto` のメッセージ（`shop.Order`）にすれば、そのまま合います。`.flow` のレコードでもかまいません。引当と発送の注文がそうで、金額は範囲の付いた `money[JPY, incl_tax]` ですが、`.proto` の側は単位を持たない `int32` です。

サービスに、ワークフローが待つイベントとコールバックの全部を書く必要はありません。書くのは、そのサービスのクライアントが送るものだけです。たとえば、規則の日本語の状態を運ぶイベント（試験用のフローにあります）は、値が ASCII の名前である `.proto` の列挙には書けません。一方、クライアントが受け取るもの（出力、失敗の名前、実行がいまどこにいるか）は、いつも全部を書きます。

## 受け取るものと返すもの

値はどれも JSON で実行に届き、実行から返ります。形は、protobuf の JSON がそのメッセージに与える形です。protobuf の JSON では、`optional` の付かないフィールド（メッセージ型を除く）は、ゼロ値（空の文字列、0、false、列挙の最初の値、空のリストやマップ）のとき省かれます。そこで、生成したコードは、どのプラットフォームでも、受け取ったものを確かめる前に、`.proto` をもとにゼロ値を埋めます。埋めるのは、実行の入力と、サービスのメソッドが送ってくるイベントの値とコールバックの応答で、その中のメッセージや、リストの中のメッセージにも埋めます。

値が無くてもよいかは、ワークフローが決めます。設定されているかどうかが分かるフィールド（メッセージ型のフィールド、`optional` を付けたフィールド、`oneof` の一つ）を、`T?` でない入力で読んでもかまいません。そのフィールドの無いリクエストが来たら、実行はその場で `Dandori.BadInput` で失敗します。ただし `json` の入力は別で、設定されていない `google.protobuf.Value` は protobuf の JSON から省かれるので、無ければ null として読みます。

返すものは、ゼロ値も含めてすべて書きます。protobuf の JSON を読む側は、書いてあるゼロ値も読めます。サービスを実装するワークフローの実行は、出力が無いときも `{}` で終わります。protobuf は `null` からメッセージを読まないからです。

## ほかの言語のクライアント

値は JSON で、Temporal では `json/plain` のペイロードになります。ほかの言語のクライアントは、メッセージを protobuf の JSON にして、それを JSON の値として渡します。Go なら `json.RawMessage(protojson.Marshal(m))`、Python なら `json_format.MessageToDict(m)` です。文字列のまま渡すと、ワークフローには文字列が届き、実行は `Dandori.BadInput` で失敗します。protobuf 独自のペイロード（`json/protobuf`）は読みません。Temporal の Go の SDK は、メッセージをそのまま渡すと、この形のペイロードにします。

## Temporal では

メソッドは、クライアントがもともとしていることに対応します。ワークフローの型は `<名前>_v<バージョン>`（`fulfillment_v1`）で、イベントは Update の `dandori.event`、コールバックへの応答は Update の `dandori.answer`、実行がいまどこにいるかはクエリの `dandori.status` です。生成する `client.ts` と `client.py` には、ほかに次のものが入ります。

- `SERVICE`：サービスの完全な名前
- メッセージごとの型。名前は `.proto` のメッセージの名前で、`FulfillRequest` はワークフローの入力、`FulfillResponse` は出力、イベントとコールバックのリクエストはそのタスクの結果の型です。
- メソッドごとの関数。名前はメソッドの名前から作り（`fulfill`、`answerPacking`。Python では `answer_packing`）、中で `start`・`send`・`answer`・`status` を呼びます。クライアントにもともとある関数と同じになるメソッド（`Start` なら `start`）には、関数を足しません。

## ほかのプラットフォームでは

実行の始め方とコールバックへの応答は、どのプラットフォームでも同じです。Step Functions、Lambda durable functions、Argo Workflows、pydantic-graph でも、入力と応答は入口でゼロ値を埋めてから読みます。イベントは、いつもどおり Temporal だけのものです。実行がいまどこにいるかを聞くメソッドも Temporal だけで、ほかのプラットフォームには実行が答える問い合わせが無いので、そのメソッドを持つサービスはエラーになります（E050）。それらのプラットフォーム向けのサービスからは、このメソッドを外してください。

## buf の lint

buf の標準の lint は、メソッドのリクエストとレスポンスにメソッドの名前を付け（`GetStatusRequest`、`GetStatusResponse`）、モジュール全体でほかのメソッドと共有しないことを求めます。そのため、検査はメソッドのメッセージを、名前ではなくフィールドで比べます。lint を通すには、`dandori.v1.Status` の三つのフィールドを、そのメソッド専用のレスポンスにコピーします。

```proto
message GetStatusResponse {
  optional int32 at = 1;
  map<string, google.protobuf.Value> cases = 2;
  repeated string events = 3;
}
```

lint を気にしなければ、`dandori.v1.Status` をそのままレスポンスにしてかまいません。
