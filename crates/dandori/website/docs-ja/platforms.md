# プラットフォーム別のビルド

```console
$ dandori build <file.flow> --target temporal|temporal-python|asl|durable|argo|pydantic-graph [--out <dir>]
```

ビルドは、一つのプラットフォーム向けのコードを書き出します。そのプラットフォームにできないこと（E050）と、一回の実行がプラットフォームの上限を超えうるワークフロー（E040）はエラーにします。dandori の主なプラットフォームは Temporal です。

## Temporal（TypeScript）

`--target temporal` は、次のファイルを書き出します。

- `workflow.ts`：ワークフロー
- `types.ts`：型
- `activities.ts`：`makeActivities(own, transport)`。生成したタスクと、自分で書くタスクをまとめます
- `io.ts`：`Transport`
- `rules.ts`：rulec の TypeScript を包んで、規則をアクティビティにしたもの
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

### 長い実行

フローの一番外にある `repeat` や、並列でない `for` は、履歴が長くなると、次のイテレーションの始めで新しい実行に引き継ぎます（Continue-As-New）。目安は 10,000 件で、サーバーが勧めればそれより早く引き継ぎます（dev server は 4,096 件を過ぎると勧めてきました）。引き継ぐ先には、変数、何回目のイテレーションか、ループのリスト、それまでに `yield` した値を渡します。ワークフロー ID は変わらず、冪等キー、コールバックと子ワークフローの ID も同じままです。Worker Deployment Versioning を使っていれば、引き継いだ実行も同じビルドで動きます。

### タイムアウト

`timeout` を書かないタスクには、ほかのプラットフォームと同じだけの時間を与えます。`http` と `agent` は HTTP Task と同じ 60 秒、`lambda` は 900 秒で、それ以外には独自のタイムアウトを付けません。ワークフロー自身のワーカーが受け持つアクティビティ（`queue` の無いタスク）はどれもハートビートを送るので、ワーカーがいなくなっても 30 秒以内に気づきます。規則のアクティビティのタイムアウトは 10 秒で、タイムアウトしたらリトライします。

## Temporal（Python）

`--target temporal-python` は、同じものを Temporal の Python SDK 向けに、ワークフローの名前を付けたパッケージとして書き出します。中身は `workflow.py`（ワークフローと、ワーカーに渡す `workflows`）、`types.py`、`activities.py`（`make_activities(own, transport)`）、`io.py`、`rules.py`（rulec の Python を包んだもの）、`runtime.py`、`worker.py`（`make_worker`）、`client.py`（`start`、`answer`、`status`）です。ワークフローの型、アクティビティ、コールバックの Update とシグナル、クエリ、ID の名前を TypeScript 版とそろえてあるので、ワークフローとアクティビティを別々の言語のワーカーで動かせます。

## AWS Step Functions

`--target asl` は、JSONata を使う ASL のステートマシンを書き出します。呼ぶ規則ごとに、rulec が生成する Python を包んだ Lambda のハンドラーも書き出します。タスクは Lambda 関数、EventBridge の接続を通した HTTP の API、SDK の統合を通した AWS のサービスを呼び、コールバックにはタスクトークンを渡します。ステートマシンには自分で書くコードを置けないので、そのどれにも当たらないタスクはエラーにします（E050）。Temporal でしか意味を持たない機能（`on cancel`、`event`）も同じくエラーにします。

## AWS Lambda durable functions

`--target durable` は、`workflow.ts`（`makeHandler(own, transport)` を持つ）、`types.ts`、`tasks.ts`、`io.ts`、`runtime.ts` を書き出します。呼ぶ規則ごとに `asl` と同じ Lambda のハンドラーも書き出し、durable function はそれを invoke します。自分で書くタスクは、そのコードを動かすステップになります。

## Argo Workflows

`--target argo` は、`<workflow>.argo.yaml` と `caller/` を書き出します。

- `<workflow>.argo.yaml` は WorkflowTemplate です。入力をパラメータ `input` で受け取り、出力をグローバルなパラメータ `dd_output` に残します。フローの変数もグローバルな出力パラメータに持ち、どの変数がどのパラメータかは、YAML の先頭のコメントに書いてあります。
- `caller/` は、ワークフローのコンテナの中で `lambda`・`http`・`aws`・`agent` のタスクと規則を動かすプログラムで、`package.json` と `Dockerfile` が付きます。

自分で書くタスクは、自分のイメージ（`image`）のコンテナで動きます。

## pydantic-graph

`--target pydantic-graph` は、`graph.py`（`graph` と、その `State` と `Deps`）、`types.py`、`tasks.py`（`make_tasks(own, transport)`）、`io.py`、`rules.py`、`runtime.py` を持つパッケージを書き出します。文の一つ一つがノードで、各ノードの戻り値の型に次のノードが書いてあるので、`graph.render()` でフローの図を描けます。実行の状態は、それを動かすプロセスの中にしかありません。待つときは `Deps.clock` で眠り、コールバックへの応答は `Deps.callbacks` に届きます。pydantic-graph 2.x は実行の状態をどこにも残さないので、プロセスが落ちればその実行も失われます。試作や、エージェントの中の短いフローに向いています。

## シナリオと参照インタプリタ

```console
$ dandori scenarios <file.flow> [--out <dir>]
$ dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|durable|argo|pydantic-graph]
```

`scenarios` は、入力と、各呼び出しが受け取る結果を並べたシナリオを書き出します。全部を合わせると、すべての分岐と、エラーを処理するすべての箇所を通り、案件の状態の変わり方もすべて試します。リストには、空のもの、短いもの、ループの上限より長いものを入れます。`run` は、シナリオを一つ参照インタプリタで動かし、呼び出しをターゲットが出す形で出力します。ビルドしたコードをこれとどう突き合わせているかは、[どうやって確かめているか](assurance.md)を見てください。
