# インストール

dandori は [ritsu](https://github.com/i2y/ritsu) の言語の一つで、ritsu のリポジトリからビルドします。規則を使う（`use rule` がある）ワークフローは `ritsu dandori` で走らせます。規則は rulec が、日付のファイルと帳簿（`use dates`、`use book`）は koyomi と chobo が、同じプロセスの中で読みます。規則を使わないワークフローなら、`dandori` のコマンドだけでも動き、生成したコードに rulec のものは入りません。

インストールする前に試すなら、[ブラウザで試す](playground.md)を開いてください。dandori がページの中で動きます。

## ritsu

最近の安定版の Rust でビルドします。dandori が外から取ってくる依存は serde_json だけです。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu dandori <コマンド>` は、規則を rulec で読む dandori のコマンドです（`ritsu dandori check`、`ritsu dandori build` など）。`ritsu` の代わりにパッケージ `dandori` を入れると、`dandori` のコマンドだけが入ります。規則も日付のファイルも帳簿も使わないワークフローのためのもので、それらを使うワークフローを渡すと、`ritsu dandori` で走らせるよう言います。

## rulec

生成したコードが呼ぶ規則のコードは、`rulec gen` が書き出します。規則を Connect のサービスとして呼ぶこともでき、そのサービスも `rulec gen` が書きます。ritsu では `ritsu rulec gen` で走らせます。rulec だけを入れるなら、パッケージ `rulec` を指定します。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked rulec
```

規則を使わない場合、あきらめるものが二つあります。一つは、規則による判断です。`.flow` には比較も計算も無いので、分岐はタスクの結果（列挙、bool、オプショナルな値）の `match` だけになり、金額や日付を比べるような判断はタスク（API、エージェント、自分で書くコード）に任せることになります。その判断に抜けが無いことは、誰も証明してくれません。もう一つは案件です。案件は規則のステートマシンに従うので、規則が無ければ書けず、案件にかかわる検査（E013、E020〜E022、E030、W101〜W103）も働きません。引当と発送の例の子のフロー（`examples/fulfillment/arrange_delivery.flow`）は規則を使わずに書いてあり、入力の運送会社（列挙）で分かれます。

## 動くか確かめる

例は、リポジトリの `crates/dandori/examples` にあります。リポジトリをクローンし（`git clone https://github.com/i2y/ritsu`）、`crates/dandori` で、例を一つ検査して、ビルドしてみます。

```console
$ ritsu dandori check examples/hotel/temporal/hotel.flow
examples/hotel/temporal/hotel.flow: ok
$ ritsu dandori build examples/hotel/temporal/hotel.flow --target temporal --out out/hotel
```

ホテルの予約の例は規則を使います。`dandori` だけで試すなら、代わりに規則を使わないフローを検査してみてください。

```console
$ dandori check examples/fulfillment/arrange_delivery.flow
examples/fulfillment/arrange_delivery.flow: ok
```

`--lang ja`（または `DANDORI_LANG=ja`、`RITSU_LANG=ja`）を付けると、検査のメッセージが日本語になります。

## AI エージェント向けのスキル

リポジトリの `crates/dandori/skills/dandori` は、Claude Code のような AI コーディングエージェント向けの [Agent Skill](https://agentskills.io) です。dandori を使う場面、下書きからビルドまでの手順、言語の要点、人に聞くべきこと、診断ごとの直し方をまとめてあり、参照するサイトのページも一緒に入っています。

```console
$ cp -r crates/dandori/skills/dandori ~/.claude/skills/                  # このマシンのすべてのプロジェクトで使う
$ cp -r crates/dandori/skills/dandori <your-project>/.claude/skills/     # 一つのプロジェクトで使い、一緒にコミットする
```

スキルは、規則を使うワークフローでは PATH にある `ritsu dandori` を、規則を使わないワークフローでは `dandori` を動かします。

## 生成したコードを動かすのに要るもの

生成したコードが使うのは各プラットフォームの SDK だけで、dandori のライブラリは要りません。

| ターゲット | 動かすのに要るもの |
|---|---|
| `temporal` | Temporal の TypeScript SDK（`@temporalio/*`）。AWS SDK、OpenAI の Agents SDK、Anthropic の SDK は、それを通して呼ぶタスクがあるときだけ |
| `temporal-python` | Temporal の Python SDK（`temporalio`）。boto3 やエージェントの SDK も同じく、使うときだけ |
| `temporal-go` | Temporal の Go SDK（`go.temporal.io/sdk`。Go 1.26 が要る）。AWS SDK for Go v2、OpenAI の Go のクライアント、Anthropic の Go の SDK は、それを通して呼ぶタスクがあるときだけ |
| `asl` | AWS Step Functions。規則は、rulec が生成する Python を包んだ Lambda 関数になるか、サービスを呼ぶ HTTP Task になる |
| `durable` | AWS Lambda durable functions（`@aws/durable-execution-sdk-js`） |
| `argo` | Argo Workflows と、生成した `caller/` から作るイメージ |
| `pydantic-graph` | pydantic-graph 2.x。自分の Python のプロセスの中で動く |

ターゲットごとに何が書き出されるかは、[プラットフォーム別のビルド](platforms.md)にあります。
