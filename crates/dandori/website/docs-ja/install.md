# インストール

dandori は実行ファイル一つです。rulec が要るのは、ワークフローが規則を使う（`use rule` がある）ときだけです。規則を使わないワークフローなら、検査もビルドもシナリオの実行も、rulec が無くてもできます。生成したコードにも rulec のものは入りません。

インストールする前に試すなら、[ブラウザで試す](playground.md)を開いてください。dandori がページの中で動きます。

## dandori

リポジトリをクローンし、最近の安定版の Rust でビルドします。依存は serde_json だけです。

```console
$ git clone https://github.com/i2y/dandori
$ cd dandori
$ cargo install --path .
```

## rulec

規則を使うワークフローでは、dandori が rulec を動かして規則を読みます。生成したコードが呼ぶ規則のコードも、`rulec gen` が書き出します。規則を Connect のサービスとして呼ぶこともでき、そのサービスも `rulec gen` が書きます。dandori は `DANDORI_RULEC` で指定した rulec を動かし、指定が無ければ PATH から探します。macOS か Linux で Homebrew を使っているなら、次の一行で入ります。

```console
$ brew install i2y/tap/rulec
```

rulec の[リリース](https://github.com/i2y/rulec/releases)から実行ファイルをダウンロードしてもかまいません。dandori は rulec 0.22.0 でテストしています。規則が `.proto` から取り込んだ列挙の値を、規則のサービスが何と呼ぶかは、0.22.0 から `rulec api` が言うようになりました。0.21.2 までの rulec はこれを言わないので、dandori は、そうした規則を、コードを同梱すれば呼べますが、サービスとしては呼べません（E005。[契約から取り込んだ列挙](tasks.md#契約から取り込んだ列挙)）。

規則を使わない場合、あきらめるものが二つあります。一つは、規則による判断です。`.flow` には比較も計算も無いので、分岐はタスクの結果（列挙、bool、オプショナルな値）の `match` だけになり、金額や日付を比べるような判断はタスク（API、エージェント、自分で書くコード）に任せることになります。その判断に抜けが無いことは、誰も証明してくれません。もう一つは案件です。案件は規則のステートマシンに従うので、規則が無ければ書けず、案件にかかわる検査（E013、E020〜E022、E030、W101〜W103）も働きません。審査の例（`examples/review`）は規則を使わずに書いてあり、申し込みに点を付けるタスクが `approve`、`reject`、`hold` のどれかを返し、フローはそれで分かれます。

## 動くか確かめる

例を一つ検査して、ビルドしてみます。

```console
$ dandori check examples/hotel/temporal/hotel.flow
examples/hotel/temporal/hotel.flow: ok
$ dandori build examples/hotel/temporal/hotel.flow --target temporal --out out/hotel
```

ホテルの予約の例は規則を使います。rulec を入れていなければ、代わりに審査の例を検査してみてください。

```console
$ dandori check examples/review/temporal/review.flow
examples/review/temporal/review.flow: ok
```

`--lang ja`（または `DANDORI_LANG=ja`）を付けると、検査のメッセージが日本語になります。

## AI エージェント向けのスキル

リポジトリの `skills/dandori` は、Claude Code のような AI コーディングエージェント向けの [Agent Skill](https://agentskills.io) です。dandori を使う場面、下書きからビルドまでの手順、言語の要点、人に聞くべきこと、診断ごとの直し方をまとめてあり、参照するサイトのページも一緒に入っています。

```console
$ cp -r skills/dandori ~/.claude/skills/                  # このマシンのすべてのプロジェクトで使う
$ cp -r skills/dandori <your-project>/.claude/skills/     # 一つのプロジェクトで使い、一緒にコミットする
```

スキルは PATH にある `dandori` を動かします。規則を使うワークフローでは `rulec` も動かします。

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
