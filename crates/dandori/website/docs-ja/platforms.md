# プラットフォーム別のビルド

```console
$ dandori build <file.flow> --target temporal|temporal-python|temporal-go|asl|durable|argo|pydantic-graph [--out <dir>]
```

ビルドは、一つのプラットフォーム向けのコードを書き出します。そのプラットフォームにできないこと（E050）と、一回の実行がプラットフォームの上限を超えうるワークフロー（E040）はエラーにします。dandori の主なプラットフォームは Temporal です。

## Temporal（TypeScript）

`--target temporal` は、次のファイルを書き出します。

- `workflow.ts`：ワークフロー
- `types.ts`：型
- `activities.ts`：`makeActivities(own, transport)`。生成したタスクと、自分で書くタスクをまとめます
- `io.ts`：`Transport`
- `rules.ts`：rulec の TypeScript を包んで、規則をアクティビティにしたもの。`use rule` の下に `connect` を書いた規則はここに入りません。そのアクティビティは `activities.ts` にあり、規則のサービスに送ります（[規則をサービスとして呼ぶ](tasks.md#規則をサービスとして呼ぶ)）
- `runtime.ts`
- `worker.ts`：`makeWorker(own)`
- `client.ts`：クライアント

### バージョン

ワークフローの型とタスクキューの名前には、`.flow` のバージョンが入ります（`hotel_stay_v1`）。そのため、新しいバージョンを古いバージョンと並べて動かせます。`makeWorker(own, { deployment })` を使えば、Worker Deployment Versioning も使えます。ビルド ID はコードのハッシュで、実行は始まったときのビルドに固定されます。Worker Deployment Versioning を使わずに同じバージョンのコードを変えるときは、走っている実行の履歴（`client.ts` の `histories` で取り出せます）を `worker.ts` の `replay` にかけ、再生できることを先に確かめてください。

### クライアント

`client.ts` には次の関数があります。

- `start`：実行を始めます。冪等キーをワークフロー ID から作るので、一度使った ID は使い回しません。
- `answer`：コールバックへの応答を Update で送ります。待っていないコールバックへの応答や二度目の応答は、ワークフローが拒否します。
- `status`：クエリ `dandori.status` を読みます。ワークフローが待っている行、案件ごとの状態、待っているイベントが分かります。
- `send`：イベントを送ります。

`{ searchAttributes: true }` を付けて始めると、ワークフローは案件の状態が変わるたびに Search Attribute の `DandoriCases`（`"pi=requires_capture"` など）を書き換えます。これで、案件の状態から実行を検索できます。

サービスを実装するワークフローの `client.ts` には、サービスのメソッドごとの関数（`fulfill`、`answerPacking`）と、`.proto` のメッセージの名前を付けた型（`FulfillRequest`）も入ります。詳しくは[サービスを実装する](services.md)を見てください。

### 長い実行

フローの一番外にある `repeat` や、並列でない `for` は、履歴が長くなると、次のイテレーションの始めで新しい実行に引き継ぎます（Continue-As-New）。目安は 10,000 件で、サーバーが勧めればそれより早く引き継ぎます（dev server は 4,096 件を過ぎると勧めてきました）。引き継ぐ先には、変数、何回目のイテレーションか、ループのリスト、それまでに `yield` した値を渡します。ワークフロー ID は変わらず、冪等キー、コールバックと子ワークフローの ID も同じままです。Worker Deployment Versioning を使っていれば、引き継いだ実行も同じビルドで動きます。

### タイムアウト

`timeout` を書かないタスクには、ほかのプラットフォームと同じだけの時間を与えます。`http`・`agent`・`jev` は HTTP Task と同じ 60 秒、`lambda` は 900 秒で、それ以外には独自のタイムアウトを付けません。ワークフロー自身のワーカーが受け持つアクティビティ（`queue` の無いタスク）はどれもハートビートを送るので、ワーカーがいなくなっても 30 秒以内に気づきます。規則のアクティビティのタイムアウトは 10 秒で、タイムアウトしたらリトライします。

## Temporal（Python）

`--target temporal-python` は、同じものを Temporal の Python SDK 向けに、ワークフローの名前を付けたパッケージとして書き出します。中身は `workflow.py`（ワークフローと、ワーカーに渡す `workflows`）、`types.py`、`activities.py`（`make_activities(own, transport)`）、`io.py`、`rules.py`（rulec の Python を包んだもの）、`runtime.py`、`worker.py`（`make_worker`）、`client.py`（`start`、`answer`、`status`。サービスを実装するワークフローなら、メソッドごとの関数も入り、名前はスネークケースの `answer_packing` のようになります）です。ワークフローの型、アクティビティ、コールバックの Update とシグナル、クエリ、ID の名前を TypeScript 版とそろえてあるので、ワークフローとアクティビティを別々の言語のワーカーで動かせます。

## Temporal（Go）

`--target temporal-go` は、同じものを Temporal の Go SDK 向けに、ワークフローの名前を付けた Go のパッケージ一つとして書き出します。`go.mod` は付けないので、自分のモジュールの中に置き、`doc.go` に書いてあるモジュールを `go get` してください。Go の import パスには ASCII の文字しか使えないので、ワークフローの名前が ASCII でないときは `.flow` のファイル名から名前を付けます（`hotel.ja.flow` なら `hotel_ja`）。それも使えなければ `workflow` にします。

- `workflow.go`：ワークフロー（`Workflow`）
- `types.go`：型と、値が型に合うかを確かめる関数
- `activities.go`：自分で書くタスクのインターフェース `OwnTasks` と、`Activities(own, transport)`
- `io.go`：`Transport`
- `rules.go`：rulec の Go を包んで、規則をアクティビティにしたもの
- `runtime.go`、`values.go`
- `worker.go`：`NewWorker(client, own, transport, deployment)` と `Replay`
- `client.go`：`Start`、`Answer`、`Send`、`Status`、`Histories`。サービスを実装するワークフローなら、メソッドごとの関数（`Fulfill`、`AnswerPacking`）も入ります

ワークフローの型やアクティビティなど、外から見える名前は TypeScript 版とそろえてあります。

### 値は JSON のまま運ぶ

ワークフローは、値を JSON の値（`map[string]any`、`[]any`、`float64`、`string`、`bool`、`nil`）のまま運びます。Python 版と同じです。構造体に読むと、フィールドが無いことと null であることの区別がなくなり、構造体の知らないフィールドは落ち、形の違う結果は読む段階で失敗してしまいます。そうした結果はワークフローが自分で見て拒否しなければならないので、構造体には読みません。`types.go` には、レコードごとの構造体と、列挙ごとの文字列の型もあります。`Decode` を使えば、タスクの引数をそれらに読めます。自分で書くタスクは、引数を `map[string]any` で受け取り、SDK が JSON に書ける値なら何でも返せます。

### 規則

`rulec gen <rule> --out <パッケージのディレクトリ>/rulec` は、規則ごとの Go を、それぞれ独立したモジュール（`rulec/go/holdamount`）として書き出します。`rules.go` の先頭に書いてあるとおり、自分の `go.mod` でそのモジュールを require し、replace でそのディレクトリを指してください。

### 既定の Transport

HTTP には `net/http`、Lambda と AWS の API には AWS SDK for Go v2、OpenAI のエージェントには OpenAI の Go のクライアント、Claude には Anthropic の Go の SDK を使います。パッケージが import するのは、フローが使う SDK だけです。OpenAI には Go の Agents SDK が無いので、Go のクライアントで、Step Functions と同じリクエストを Responses API に送ります。

### Go のバージョン

生成したコードが前提にしている Temporal の Go SDK 1.49.0 は、Go 1.26 を求めます。手元の Go がそれより古ければ、`go` コマンドが Go 1.26 を自分で取ってきます。

## AWS Step Functions

`--target asl` は、JSONata を使う ASL のステートマシンを書き出します。Lambda 関数として呼ぶ規則ごとに、rulec が生成する Python を包んだ Lambda のハンドラーも書き出します。`connect` を書いた規則は、HTTP Task で規則のサービスを呼びます。`connection` と、HTTPS の URL が要り、Lambda 関数は書きません。タスクは Lambda 関数、EventBridge の接続を通した HTTP の API、SDK の統合を通した AWS のサービスを呼び、コールバックにはタスクトークンを渡します。ステートマシンには自分で書くコードを置けないので、そのどれにも当たらないタスクはエラーにします（E050）。Temporal でしか意味を持たない機能（`on cancel`、`event`、実装するサービスの、実行がいまどこにいるかを聞くメソッド）も同じくエラーにします。

## AWS Lambda durable functions

`--target durable` は、`workflow.ts`（`makeHandler(own, transport)` を持つ）、`types.ts`、`tasks.ts`、`io.ts`、`runtime.ts` を書き出します。Lambda 関数として呼ぶ規則ごとに `asl` と同じ Lambda のハンドラーも書き出し、durable function はそれを invoke します。`connect` を書いた規則は、規則のサービスに送るステップになります。自分で書くタスクは、そのコードを動かすステップになります。

## Argo Workflows

`--target argo` は、`<workflow>.argo.yaml` と `caller/` を書き出します。

- `<workflow>.argo.yaml` は WorkflowTemplate です。入力をパラメータ `input` で受け取り、出力をグローバルなパラメータ `dd_output` に残します。フローの変数もグローバルな出力パラメータに持ち、どの変数がどのパラメータかは、YAML の先頭のコメントに書いてあります。
- `caller/` は、ワークフローのコンテナの中で `lambda`・`http`・`aws`・`agent`・`jev` のタスクと規則を動かすプログラムで、`package.json` と `Dockerfile` が付きます。

自分で書くタスクは、自分のイメージ（`image`）のコンテナで動きます。

タスクの `timeout` は、Pod の `activeDeadlineSeconds` になります。Kubernetes は、Pod を受け付けた時点から数えるので、コンテナが起動するまでの時間も含まれます。混んでいるクラスターでは、Pod の起動に数秒かかることがあるので、その分の余裕を見込んだ `timeout` にしてください。期限が過ぎるのと同時に終わった Pod は、期限を過ぎたものとして扱われ、Argo はタスクが返したエラーの名前を読めません。そのエラーに対する `retry` は行われません。

## pydantic-graph

`--target pydantic-graph` は、`graph.py`（`graph` と、その `State` と `Deps`）、`types.py`、`tasks.py`（`make_tasks(own, transport)`）、`io.py`、`rules.py`、`runtime.py` を持つパッケージを書き出します。文の一つ一つがノードで、各ノードの戻り値の型に次のノードが書いてあるので、`graph.render()` でフローの図を描けます。実行の状態は、それを動かすプロセスの中にしかありません。待つときは `Deps.clock` で眠り、コールバックへの応答は `Deps.callbacks` に届きます。pydantic-graph 2.x は実行の状態をどこにも残さないので、プロセスが落ちればその実行も失われます。試作や、エージェントの中の短いフローに向いています。

Lambda durable functions、Argo Workflows、pydantic-graph も、`event` のタスクと、サービスの、実行がいまどこにいるかを聞くメソッドはエラーにします（E050）。サービスを実装するワークフローは、どのプラットフォームでも、入力と、サービスのメソッドが送ってくるコールバックの応答に、protobuf の JSON が省いたゼロ値を埋めてから読みます。

## シナリオと参照インタプリタ

```console
$ dandori scenarios <file.flow> [--out <dir>]
$ dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph]
```

`scenarios` は、入力と、各呼び出しが受け取る結果を並べたシナリオを書き出します。全部を合わせると、すべての分岐と、エラーを処理するすべての箇所を通り、案件の状態の変わり方もすべて試します。リストには、空のもの、短いもの、ループの上限より長いものを入れます。`run` は、シナリオを一つ参照インタプリタで動かし、呼び出しをターゲットが出す形で出力します。ビルドしたコードをこれとどう突き合わせているかは、[どうやって確かめているか](assurance.md)を見てください。
