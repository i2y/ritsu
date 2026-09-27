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
| WorkflowTemplate を手元の kind のクラスタの Argo Workflows v4.1.4 で | `tools/argo/run.mjs` |
| グラフを pydantic-graph 2.51.0 で | `tools/pydantic-graph/run.py` |

## シナリオ

シナリオは、入力と、呼び出しが受け取る答えを順に並べたものです。シナリオを全部合わせると、すべての行き先と、エラーを受けるすべての箇所を通り、案件の状態の変わり方もすべて試します。リストには、空のもの、短いもの、ループの上限より長いものを入れます。一つのシナリオの中の値はどれも違えてあるので、二つの答えやループの回を取り違えるプラットフォームがあれば、突き合わせで分かります（範囲のある数は範囲の中から選ぶので、たまたま同じになることはあります）。どのプラットフォームも断らなければならない答えも入れます。形の合わない答え、ステートマシンが行き着かない状態、範囲を外れた数です。

Temporal、durable functions、pydantic-graph では、生成したタスクを、送るはずの要求を記録するだけのテスト用の `Transport` で動かし、記録した要求を Step Functions が送るものと比べます。自分で書くタスク、規則、子ワークフローは、シナリオどおりに答えるスタブに置き換えます。並列のループも、ランナーの中では一回ずつ順に回すので、呼び出しは参照インタプリタと同じ順に出ます。

## Temporal

一つのフローのすべての実行を同時に流し、それぞれを別のワークフロー ID で動かします。冪等キーにはこの ID が入ります。

- ランナーは、生成した `worker` と `client` でワーカーを作って実行を始め、コールバックに Update で答えます（二度目の答えが断られることも確かめます）。実行が終わったら、クエリと Search Attribute が示す案件の状態を確かめます。
- どの実行も、終わったあとで履歴を同じコードで再生します。`tests/histories` に残した以前の実行の履歴は、いまの生成器が生成するコードで再生します。走っているワークフローを壊すような生成器の変更があれば、ここで分かります。
- 一か所だけ違う二つのビルドを、Worker Deployment Versioning の同じデプロイの二つの版として動かします。一つ目の版で始まった実行は、新しい実行に引き継いだ先の回まで含めて、一つ目の版のまま終わります。
- サーバーは実際の時間で動くので、ランナーはテスト用にコードの複製を作り、タイマーを長くても 10 ms に縮め、アクティビティの期限を 5 秒にします。シナリオで期限切れになる呼び出しは、サーバーが期限切れにするまで返しません。複製ではどの履歴も長すぎると見なすので、フローの一番外のループは、実行の最初の回を除いて毎回新しい実行に引き継ぎます。ランナーは引き継いだ実行を順にたどり、すべてを再生します。
- どのフローも、ワークフローとアクティビティを別々の言語のワーカーで動かします。組み合わせは両方の向きで試します。
- `on cancel` のあるフローでは、呼び出しの途中でワークフローをキャンセルするシナリオも流します。
- ほかの `.flow` を子として走らせるワークフローは、スタブではなく子から生成したワークフローとも一緒に動かします。親と子が同じ言語の場合も、違う言語の場合も試します。

## Step Functions

ASL は `tools/asl-run.mjs` の上で JSONata 2.0.6 を使って動かし、asl-validator にも通します。

LocalStack の Step Functions でも動かします。こちらの JSONata は Java の実装です。使うのは Docker で動かす 4.14.0 のコミュニティ版で、アカウントなしで起動できる最後の版です。呼び出しへの答えは、どれも LocalStack のモックの仕組みで返します。実行を一つのテストケースとして流し、呼び出し・待ち・終わり方を実行の履歴から読み取ります。Map の回は一つずつ回します。Wait は待たずに進め、待つはずだった長さは、ランナーだけが読む変数に残します。リトライは実際に待ち、その長さを履歴の時刻で測ります。負荷の高い機械ではリトライの待ちだけが延びることがあるので、そうなった実行は一度だけ流し直します。

この版の LocalStack は HTTP Task とそのエラーに対応しておらず、モックの設定ファイルから Step Functions 自身のエラーを投げることもできません。そこでランナーは、HTTP Task の resource を、モックの答えをそのまま渡す AWS SDK の統合に置き換えます。Step Functions のエラーは別の名前で投げ、Retry と Catch もその名前で受けるように書き換えます。こうして投げるのは、タイムアウト、HTTP Task のエラー、それにモデルが断ったときにエージェントの Task を失敗させる `States.QueryEvaluationError` です（この版の LocalStack は、このエラーの代わりに実行全体を `States.Runtime` で失敗させてしまいます）。

## Argo Workflows

WorkflowTemplate はコントローラーが実際に動かしますが、Pod の中身はランナーが肩代わりします。Pod はどれも、クラスタに無いスケジューラーを待つように作らせ、コンテナと Argo の executor がするはずのことを、ランナーが手元で行います。具体的には、生成した caller（テスト用の `Transport` を付けたもの）か、ほかのタスクのスタブを動かし、テンプレートが宣言した出力を渡し、コンテナが残すはずの終了コードと終了メッセージで Pod を終わらせます。コンテナを立てないので、すべてのフローのすべての実行を同時に流せます。フローごとに一つの実行は、本物の Pod でも流し直します。そこでは node:24-alpine のコンテナの中で caller とスタブが動き、クラスタの中のモックに答えを問い合わせます。これでコンテナの側も確かめられます。本物の Pod をプラットフォームが動かせなかったとき（Error で終わったときや、終了コード 255 の Unknown になったとき。kind 0.29.0 のノードイメージの containerd では、負荷がかかるとときどき起きました）は、その実行を二回まで流し直し、テストの出力にもそう書きます。

## エージェントと既定の Transport

エージェントの呼び出しは、`Transport` に届いた形のまま記録し、モデルの代わりにスタブが `{"answer": …}` を返します。ASL のランナーは、Responses API や Messages API への HTTP Task に、その API の形の答えを返します。失敗は、モデルが断ったという答えとして返します。そのため、Task を失敗させるのは、ステートマシンの中で答えを読む式です。Claude のエージェントには、列挙の値の大文字と小文字を変えた答えも返します。どのプラットフォームも、それを宣言した値として読まなければなりません。

既定の `Transport` を通るエージェントの呼び出しは、TypeScript でも Python でも、OpenAI の Agents SDK に、答えを前もって決めたテスト用のモデルを OpenAI のモデルの代わりに渡して動かします。Anthropic の SDK には、手元に立てたモックの Messages API へ送らせます。モデルに頼む中身は、Step Functions が同じ呼び出しで頼むものと同じでなければならず、エラーのステータスが返れば、やり直さずに失敗しなければなりません。Open Responses のエンドポイントに送るエージェントは、手元に立てたモックのエンドポイントに送らせ、Step Functions が送るのとまったく同じ要求が届くことを確かめます。OpenAI にも Anthropic にも何も送りません。テストを動かす機械で Ollama が動いていれば、そうしたエージェントごとに一回は実際に送り、答えがタスクの型に合うことも確かめます。Ollama の場所は `DANDORI_OLLAMA` で、モデルは `DANDORI_OLLAMA_MODEL` で指定でき、モデルを指定しなければ Ollama にあるいちばん小さいものを使います。

既定の `Transport` のそれ以外の部分（TypeScript の `fetch` と AWS SDK、Python の標準ライブラリと boto3）は、シナリオの HTTP・Lambda・AWS の呼び出しを、手元に立てたモックへ送ります。HTTP と Lambda の Invoke に答えるサーバーと、SNS と SQS を受け持つ moto です。届いた要求は呼び出しのとおりでなければならず、二つの言語は同じ文字列を送らなければなりません。AWS のエラーは、タスクが宣言した名前（`NotFoundException`）で返ってこなければなりません。

## 外しているもの

durable functions のテストランナーは、狙った呼び出しを期限切れにできません。そのため、期限切れを含むシナリオはそこでは外します。Argo と pydantic-graph のランナーもタスクを期限切れにはできませんが、コールバックの期限切れは起こせます。pydantic-graph では答えないことで、Argo では待ちに期限切れのときの値を渡すことで起こします。Temporal ではどちらも起こせます。

ほかに、Temporal、durable functions、Argo の caller 向けに生成した TypeScript を `tsc --strict` に通します。規則を包むコードは、`rulec vectors` が出すすべての例に、rulec のとおりに答えることを確かめます。

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
$ sh tools/argo/setup.sh        # Argo Workflows の入った kind のクラスタ（docker、kind 0.33 以上、kubectl）
$ docker pull localstack/localstack:4.14.0
$ DANDORI_RULEC=/path/to/rulec cargo test
```

rulec、Node、`tools/` の中身、クラスタ、`argo` コマンド、LocalStack のイメージのどれかが見つからないテストは、`SKIP:` の行を出して通ってしまいます。`-- --nocapture` を付けて出力を読んでください。`cargo test` 全体は 2〜4 分かかります。`tools/argo/setup.sh` は、kind 0.33.0 のノードイメージの上で、Argo のコントローラーがワークフローの変化を見直すまでの間隔を 10 秒から 1 秒に縮めます。`DANDORI_FLOW=<パスの一部>` を付けると、パスにそれを含むフローだけを流します。`DANDORI_BLESS=1` を付けると、診断の golden ファイルを書き直し、残しておく履歴を取り直します。
