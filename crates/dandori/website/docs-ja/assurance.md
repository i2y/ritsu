# どうやって確かめているか

`.flow` の意味を決めるのは参照インタプリタです。テストはすべての例からシナリオを作り、一つ一つを下の表の八通りで動かします。どれも同じ呼び出しを同じ引数と冪等キーで出し、同じ終わり方をしなければなりません。

| どう動かすか | 使うもの |
|---|---|
| 参照インタプリタ | `dandori run` |
| ASL を JSONata 2.0.6 で | `tools/asl-run.mjs` |
| ASL を LocalStack の Step Functions で | `tools/localstack/run.mjs` |
| TypeScript の Temporal のワークフローを Temporal のサーバーで | `tools/temporal/run.mjs`（Temporal CLI の dev server） |
| Python の Temporal のワークフローを Temporal のサーバーで | `tools/temporal-python/run.py` |
| durable function を SDK のローカルのテストランナーで | `tools/durable/run.mjs` |
| WorkflowTemplate をローカルの kind のクラスタの Argo Workflows v4.1.4 で | `tools/argo/run.mjs` |
| グラフを pydantic-graph 2.51.0 で | `tools/pydantic-graph/run.py` |

## シナリオ

シナリオは、入力と、各呼び出しが受け取る結果を順に並べたものです。シナリオを全部合わせると、すべての分岐と、エラーを処理するすべての箇所を通り、案件の状態の変わり方もすべて試します。リストには、空のもの、短いもの、ループの上限より長いものを入れます。一つのシナリオの中の値はどれも違えてあるので、二つの結果やループのイテレーションを取り違えるプラットフォームがあれば、突き合わせで分かります（範囲のある数は範囲の中から選ぶので、たまたま同じになることはあります）。どのプラットフォームも拒否しなければならない結果も入れます。形の合わない結果、ステートマシンが行き着かない状態、範囲を外れた数です。サービスを実装するワークフローには、入力と、サービスが送ってくるイベントの値やコールバックの応答から、ゼロ値のフィールドを protobuf の JSON のとおりに省いたものも入れます。ゼロ値を埋めないプラットフォームは、参照インタプリタと違う入力や結果を読むことになり、突き合わせで分かります。設定されているかどうかが分かるフィールドを、ワークフローが `T?` でない入力で読むときは、そのフィールドの無い入力も入れます。どのプラットフォームも、その実行をその場で `Dandori.BadInput` で終えなければなりません。

Temporal、durable functions、pydantic-graph では、生成したタスクを、送るはずのリクエストを記録するだけのテスト用の `Transport` で動かし、記録したリクエストを Step Functions が送るものと比べます。自分で書くタスク、規則、子ワークフローは、シナリオどおりの結果を返すスタブに置き換えます。並列のループも、ランナーの中ではイテレーションを一つずつ順に回すので、呼び出しは参照インタプリタと同じ順に出ます。

## Temporal

一つのフローのすべての実行を同時に流し、それぞれを別のワークフロー ID で動かします。冪等キーにはこの ID が入ります。

- ランナーは、生成した `worker` と `client` でワーカーを作って実行を始め、コールバックに Update で応答します（二度目の応答が拒否されることも確かめます）。実行が終わったら、クエリと Search Attribute が示す案件の状態を確かめます。
- どの実行も、終わったあとで履歴を同じコードで再生します。`tests/histories` に残した以前の実行の履歴は、いまのジェネレーターが生成するコードで再生します。走っているワークフローを壊すようなジェネレーターの変更があれば、ここで分かります。
- 一か所だけ違う二つのビルドを、Worker Deployment Versioning の同じデプロイの二つのバージョンとして動かします。一つ目のバージョンで始まった実行は、新しい実行に引き継いだ先のイテレーションまで含めて、一つ目のバージョンのまま終わります。
- サーバーは実際の時間で動くので、ランナーはテスト用にコードのコピーを作り、タイマーを長くても 10 ms に縮め、アクティビティのタイムアウトを 5 秒にします。シナリオでタイムアウトする呼び出しは、サーバーがタイムアウトさせるまで返しません。コピーではどの履歴も長すぎると見なすので、フローの一番外のループは、実行の最初のイテレーションを除いて毎回新しい実行に引き継ぎます。ランナーは引き継いだ実行を順にたどり、すべてを再生します。
- どのフローも、ワークフローとアクティビティを別々の言語のワーカーで動かします。組み合わせは両方の向きで試します。
- `on cancel` のあるフローでは、呼び出しの途中でワークフローをキャンセルするシナリオも流します。
- ほかの `.flow` を子として走らせるワークフローは、スタブではなく子から生成したワークフローとも一緒に動かします。親と子が同じ言語の場合も、違う言語の場合も試します。

## Step Functions

ASL は `tools/asl-run.mjs` の上で JSONata 2.0.6 を使って動かし、asl-validator にも通します。

LocalStack の Step Functions でも動かします。こちらの JSONata は Java の実装です。使うのは Docker で動かす 4.14.0 のコミュニティ版で、アカウントなしで起動できる最後のバージョンです。呼び出しへのレスポンスは、どれも LocalStack のモックの仕組みで返します。実行を一つのテストケースとして流し、呼び出し・待ち・終わり方を実行の履歴から読み取ります。Map のイテレーションは一つずつ回します。Wait は待たずに進め、待つはずだった長さは、ランナーだけが読む変数に残します。リトライは実際に待ち、その長さを履歴の時刻で測ります。負荷の高いマシンではリトライの待ちだけが延びることがあるので、そうなった実行は一度だけ流し直します。

このバージョンの LocalStack は HTTP Task とそのエラーに対応しておらず、モックの設定ファイルから Step Functions 自身のエラーを投げることもできません。そこでランナーは、HTTP Task の resource を、モックのレスポンスをそのまま渡す AWS SDK の統合に置き換えます。Step Functions のエラーは別の名前で投げ、Retry と Catch もその名前で捕捉するように書き換えます。こうして投げるのは、タイムアウト、HTTP Task のエラー、それにモデルが応答を拒否したときにエージェントの Task を失敗させる `States.QueryEvaluationError` です（このバージョンの LocalStack は、このエラーの代わりに実行全体を `States.Runtime` で失敗させてしまいます）。

## Argo Workflows

WorkflowTemplate はコントローラーが実際に動かしますが、Pod の中身はランナーが肩代わりします。Pod はどれも、クラスタに無いスケジューラーを待つように作らせ、コンテナと Argo の executor がするはずのことを、ランナーがローカルで行います。具体的には、生成した caller（テスト用の `Transport` を付けたもの）か、ほかのタスクのスタブを動かし、テンプレートが宣言した出力を渡し、コンテナが残すはずの終了コードと終了メッセージで Pod を終わらせます。コンテナを立てないので、すべてのフローのすべての実行を同時に流せます。フローごとに一つの実行は、本物の Pod でも流し直します。そこでは node:24-alpine のコンテナの中で caller とスタブが動き、クラスタの中のモックに結果を問い合わせます。これでコンテナの側も確かめられます。本物の Pod をプラットフォームが動かせなかったとき（Error で終わったときや、終了コード 255 の Unknown になったとき。kind 0.29.0 のノードイメージの containerd では、負荷がかかるとときどき起きました）は、その実行を二回まで流し直し、テストの出力にもそう書きます。

## エージェント、Jev、既定の Transport

エージェントの呼び出しは、`Transport` に届いた形のまま記録し、モデルの代わりにスタブが `{"answer": …}` を返します。ASL のランナーは、Responses API や Messages API への HTTP Task に、その API の形のレスポンスを返します。失敗は、モデルが応答を拒否したというレスポンスとして返します。そのため、Task を失敗させるのは、ステートマシンの中でレスポンスを読む式です。Claude のエージェントには、列挙の値の大文字と小文字を変えた応答も返します。どのプラットフォームも、それを宣言した値として読まなければなりません。

既定の `Transport` を通るエージェントの呼び出しは、TypeScript でも Python でも、OpenAI の Agents SDK に、応答を前もって決めたテスト用のモデルを OpenAI のモデルの代わりに渡して動かします。Anthropic の SDK には、ローカルに立てたモックの Messages API へ送らせます。モデルへのリクエストの中身は、Step Functions が同じ呼び出しで送るものと同じでなければならず、エラーのステータスが返れば、リトライせずに失敗しなければなりません。Open Responses のエンドポイントに送るエージェントは、ローカルに立てたモックのエンドポイントに送らせ、Step Functions が送るのとまったく同じリクエストが届くことを確かめます。OpenAI にも Anthropic にも何も送りません。テストを動かすマシンで Ollama が動いていれば、そうしたエージェントごとに一回は実際に送り、応答がタスクの型に合うことも確かめます。このときはエフォートを外します。推論しないモデルでは、付けると拒否されるからです。Ollama の場所は `DANDORI_OLLAMA` で、モデルは `DANDORI_OLLAMA_MODEL` で指定でき、モデルを指定しなければ Ollama にあるいちばん小さいものを使います。

既定の `Transport` のそれ以外の部分（TypeScript の `fetch` と AWS SDK、Python の標準ライブラリと boto3）は、シナリオの HTTP・Lambda・AWS の呼び出しを、ローカルに立てたモックへ送ります。HTTP と Lambda の Invoke に応答するサーバーと、SNS と SQS を受け持つ moto です。届いたリクエストは呼び出しのとおりでなければならず、二つの言語は同じ文字列を送らなければなりません。AWS のエラーは、タスクが宣言した名前（`NotFoundException`）で返ってこなければなりません。

サービスで呼ぶ規則（`use rule` の下の `connect`）の呼び出しは、どのプラットフォームでも HTTP のリクエストです。シナリオは、サービスが書く形のレスポンスを返します。選んだレコードを protobuf の JSON にして、ゼロ値を省き、数を文字列にしたものです。false と 0（と、0 番が規則の値の列挙の 0 番の値）を省いたレスポンス、0 番が規則の値でない列挙を省いたレスポンス、範囲の外の数、形の合わない本文、失敗も返します。どのプラットフォームも、レスポンスを規則のレコードとして読み戻すか、参照インタプリタと同じに呼び出しを終えなければなりません。

dandori が規則のサービスについて `rulec api` から読むもの（パス、リクエストとレスポンスのフィールドの JSON の名前と種類、列挙の値と 0 番の値、レスポンスが省くゼロ値）は、同じ規則について `rulec gen` が書く `.proto` と比べます。`.proto` は dandori 自身の読み手で読み、例の規則すべてと、列挙を `.proto` から取り込む（`import proto`）規則二つについて確かめます。一つは値が契約の接頭辞を持つもの、もう一つは値に接頭辞が無く、0 番もほかと同じ一つの値（`ACTIVE = 0`）のものです。

これらの規則ごとに `rulec gen` が書くサービスは、そのまま動かします（`--http`、標準ライブラリのサーバーです。サービスが import するスタブは、`tools/connect/.venv` のプラグインで buf に書かせます）。`rulec vectors` が書くベクタをすべて、dandori がリクエストを書くとおりに、入力を省かずに送ります。dandori がレスポンスから読んだ結果がベクタの出力と同じであること、レスポンスのヘッダ `rulec-source-sha256` が規則の `source_sha256` と同じことを確かめます。範囲の上限を超える入力、列挙のどの値でもない名前、リクエストに無いフィールド、入力を一つ省いたリクエストは、どれも `invalid_argument`（ステータスは 400）で断られなければなりません。13 の規則のベクタ 350 件を送り（うち 114 件はゼロ値の入力を含みます）、サービスは、範囲を超える入力 9 件、名前 11 件、フィールド 13 件、入力を省いたリクエスト 13 件を断りました。0.21.2 までの rulec（サービスが列挙の値を何と呼ぶかを `rulec api` が言わないもの）では、`.proto` から列挙を取り込む規則二つと、その rulec のサービスが断らない、知らないフィールドと省いた入力を飛ばします。

サービスのレスポンスは、四つの場所で規則のレコードとして読まれます。参照インタプリタ、dandori が書く TypeScript と Python、ステートマシンの JSONata です。四つの規則の 200 通りのレスポンスを、四つのどこでも同じに読まなければなりません。サービスが書かないはずのレスポンスも入れています。十進でない数や 2^53 − 1 を超える数、どの列挙にもない名前（`constructor`、`__proto__`）、種類の違うフィールド、null、オブジェクトでない本文です。どれも例外を投げてはいけません。

Jev のタスクの呼び出しは HTTP のリクエストで、スタブは TypeSafe の API リファレンスにある形の答えを返します。質問ごとに、選んだ値、段階の位置、はいの確率のどれかと、確信度が入った答えです。シナリオは、Jev の呼び出しごとに、タスクの `confidence` とちょうど同じ確信度の答え、それをわずかに下回る答え、型に合わない答えを返し、どのプラットフォームの答えの読み方も、確信度の境目も、参照インタプリタと比べます。確信度が足りない答えのあとは、変数に前の値が残っていなければならず、次の呼び出しがその値を送るので、答えを変数に入れてしまうプラットフォームがあれば食い違いとして出ます。既定の `Transport` が Jev に送るリクエストもローカルのモックに送り、`TYPESAFE_API_KEY` から読んだキーが `Authorization: Bearer <キー>` として届くことを確かめます。

`TYPESAFE_API_KEY` があれば、例と試験用のフローにある Jev のタスクを一つずつ、TypeScript と Python の既定の `Transport` から本物の TypeSafe にも送ります。レスポンスには質問のすべてに答えがなければならず、どの答えもタスクの型に読めるか、確信度が足りなければタスクのエラーで呼び出しを失敗させなければなりません。キーが無ければ、TypeSafe には何も送りません。

## サービス

サービスを実装するワークフローが受け取るものと返すものは、protobuf そのものでも確かめます。サービスの `.proto` を、dandori のオプションのファイルにリポジトリの `proto/dandori/v1/options.proto` を使って protoc で記述子にし、protobuf の Python（知らないフィールドを拒否する `json_format`）に、それを使って次のものを読ませます。シナリオの入力はどれも、実行を始めるメソッドのリクエストとして読みます。参照インタプリタが実行を終えたときの出力は、そのレスポンスとして読みます。サービスのメソッドが送るイベントの値とコールバックの応答は、そのメソッドのリクエストとして、クエリ `dandori.status` が返すものは `dandori.v1.Status` として読みます。protobuf がゼロ値を省いて書き直した入力は、ゼロ値を埋めると元の入力に戻ります。ほかに、dandori のオプションのファイルが buf の標準の lint を通ることと、例とテストにあるサービスの `.proto` がどれも protoc で組み立てられることを確かめます。

## 図

`dandori doc` が、例、tests/flows のフロー、エラーのある下書きについて書く Markdown は、golden のファイルと一字ずつ比べます。このサイトに置いた例のページも、いまの出力と同じでなければなりません。どちらにも、呼び出す規則が `rulec doc` の描いたとおりに入っています。どのシナリオでも、実行が光らせるところはつながっていなければなりません。光ったステップには光った辺が入り、選んだ分岐、入ったハンドラ、ループの戻りと出口には、光った辺がなければなりません。Mermaid の図は、rulec が描くステートマシンの図も含めて、headless Chrome の中で Mermaid 11 と 12 に描かせ、すべて描けることを確かめます。Chrome でページを開くと、ページのデータどおりに光ることと、規則ごとに `rulec doc` が描いたページが開くことも確かめます。

## ブラウザで試すページ

[ブラウザで試す](playground.md)ページは、wasm32 にした dandori を動かし、例が読むものを `presets.json` から読みます。例のファイルと、例の規則について rulec が出力したもの（`rulec doc` が描いたものも）です。どちらもリポジトリに置いてあり、どちらもリポジトリと突き合わせます。

- `presets.json` は、いま例を検査して読むものと、いまの rulec の出力でなければなりません。
- ページで開けるどのフローでも、ディスクから読んで rulec を動かすコマンドが出力するもの、書くものが、`presets.json` から読むページの答えと同じでなければなりません。`check`、六つの出力先への `build`、二つの形式の `doc` を比べます。
- 「規則」のタブには対になるコマンドがありません。`presets.json` から読んだ答えが、ディスクから読んで rulec を動かしたときの答えと同じで、どの規則にもページがなければなりません。
- wasm のモジュールは、どの問い合わせにも、ライブラリと同じ答えを返さなければなりません。例のフローそのままのほか、それでは通らないところを通る編集（構文の誤り、ページに無い規則と子のフロー、自分を子として走らせるフロー）も試します。
- 英語と日本語のどちらのページも、Chrome で開くと動き出し、下書きについて `check` が出力するものを表示し、フロー・タブ・出力先を指すリンクのとおりに開き、規則ごとに本文とページへのリンクを出さなければなりません。

## 外しているもの

durable functions のテストランナーは、狙った呼び出しをタイムアウトさせられません。そのため、タイムアウトを含むシナリオはそこでは外します。Argo と pydantic-graph のランナーもタスクをタイムアウトさせられませんが、コールバックのタイムアウトは起こせます。pydantic-graph では応答しないことで、Argo では待ちにタイムアウトしたときの値を渡すことで起こします。Temporal ではどちらも起こせます。

ほかに、Temporal、durable functions、Argo の caller 向けに生成した TypeScript を `tsc --strict` に通します。規則を包むコードは、`rulec vectors` が出すすべての例に、rulec のとおりの結果を返すことを確かめます。

本物の AWS、本番構成の Temporal や Temporal Cloud、本物の OpenAI や Anthropic の API の上では、どれも動かしていません。

## テストを動かす

```console
$ npm install --prefix tools
$ npm install --prefix tools/temporal
$ npm install --prefix tools/durable
$ uv venv --python 3.13 tools/temporal-python/.venv
$ uv pip install --python tools/temporal-python/.venv/bin/python -r tools/temporal-python/requirements.txt
$ uv venv --python 3.13 tools/pydantic-graph/.venv
$ uv pip install --python tools/pydantic-graph/.venv/bin/python -r tools/pydantic-graph/requirements.txt
$ npm install --prefix tools/agents
$ uv venv --python 3.13 tools/agents/.venv
$ uv pip install --python tools/agents/.venv/bin/python -r tools/agents/requirements.txt
$ npm install --prefix tools/wire
$ uv venv --python 3.13 tools/wire/.venv
$ uv pip install --python tools/wire/.venv/bin/python -r tools/wire/requirements.txt
$ uv venv --python 3.13 tools/connect/.venv
$ uv pip install --python tools/connect/.venv/bin/python -r tools/connect/requirements.txt
$ npm install --prefix tools/mermaid
$ sh tools/argo/setup.sh        # Argo Workflows の入った kind のクラスタ（docker、kind 0.33 以上、kubectl）
$ docker pull localstack/localstack:4.14.0
$ DANDORI_RULEC=/path/to/rulec cargo test
```

テストは rulec 0.22.0 以降で動かしてください。`dandori doc` の golden ファイルとサイトの例のページ、ブラウザで試すページの `presets.json` には、規則について rulec が出力したもの（`rulec doc` も）が入っていて、そこに rulec の版の文字列があります。0.21.2 までの rulec で動かすと、その違いだけで三つのテストが落ちます。`dandori doc` の出力をそれらのファイルと比べる二つと、ブラウザで試すページの答えをコマンドの出力と比べる一つです。`connect.enums` を出力するのは 0.22.0 からなので、それが要るテストは `SKIP:` の行を出して飛ばします。

rulec、Node、`tools/` の中身、buf、protoc、クラスタ、`argo` コマンド、LocalStack のイメージ、Chrome のどれかが見つからないテストは、`SKIP:` の行を出して通ってしまいます。`-- --nocapture` を付けて出力を読んでください。`cargo test` 全体は 2〜4 分かかります。`tools/argo/setup.sh` は、kind 0.33.0 のノードイメージの上で、Argo のコントローラーがワークフローの変化を見直すまでの間隔を 10 秒から 1 秒に縮めます。`DANDORI_FLOW=<パスの一部>` を付けると、パスにそれを含むフローだけを流します。`DANDORI_BLESS=1` を付けると、診断と図の golden ファイルと、サイトの例のページを書き直し、残しておく履歴と、ブラウザで試すページの `presets.json` を取り直します。`check`・`build`・`doc` の答えが変わる変更をしたら、`website/tools/make_wasm.sh` でページの wasm のモジュールを作り直します（rustup の `wasm32-unknown-unknown` のターゲットが要ります）。
