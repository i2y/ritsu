# インストール

dandori は実行ファイル一つです。rulec が要るのは、ワークフローが規則を使う（`use rule` がある）ときだけです。規則を使わないワークフローなら、検査もビルドもシナリオの実行も、rulec が無くてもできます。生成したコードにも rulec のものは入りません。

## dandori

リポジトリをクローンし、最近の安定版の Rust でビルドします。依存は serde_json だけです。

```console
$ git clone https://github.com/i2y/dandori
$ cd dandori
$ cargo install --path .
```

## rulec

規則を使うワークフローでは、dandori が rulec を動かして規則を読みます。生成したコードが呼ぶ規則のコードも、`rulec gen` が書き出します。dandori は `DANDORI_RULEC` で指定した rulec を動かし、指定が無ければ PATH から探します。macOS か Linux で Homebrew を使っているなら、次の一行で入ります。

```console
$ brew install i2y/tap/rulec
```

rulec の[リリース](https://github.com/i2y/rulec/releases)から実行ファイルをダウンロードしてもかまいません。dandori は rulec 0.20.0 と 0.21.1 でテストしています。

規則を使わない場合、あきらめるものが二つあります。一つは、規則による判断です。`.flow` には比較も計算も無いので、分岐はタスクの答え（列挙、bool、無いことがある値）の `match` だけになり、金額や日付を比べるような判断はタスク（API、エージェント、自分で書くコード）に任せることになります。その判断に抜けが無いことは、誰も証明してくれません。もう一つは案件です。案件は規則のステートマシンに従うので、規則が無ければ書けず、案件にかかわる検査（E013、E020〜E022、E030、W101〜W103）も働きません。審査の例（`examples/review`）は規則を使わずに書いてあり、申し込みに点を付けるタスクが `approve`、`reject`、`hold` のどれかを答え、フローはそれで分かれます。

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

## 生成したコードを動かすのに要るもの

生成したコードが使うのは各プラットフォームの SDK だけで、dandori のライブラリは要りません。

| ターゲット | 動かすのに要るもの |
|---|---|
| `temporal` | Temporal の TypeScript SDK（`@temporalio/*`）。AWS SDK、OpenAI の Agents SDK、Anthropic の SDK は、それを通して呼ぶタスクがあるときだけ |
| `temporal-python` | Temporal の Python SDK（`temporalio`）。boto3 やエージェントの SDK も同じく、使うときだけ |
| `asl` | AWS Step Functions。規則は、rulec が生成する Python を包んだ Lambda 関数になる |
| `durable` | AWS Lambda durable functions（`@aws/durable-execution-sdk-js`） |
| `argo` | Argo Workflows と、生成した `caller/` から作るイメージ |
| `pydantic-graph` | pydantic-graph 2.x。自分の Python のプロセスの中で動く |

ターゲットごとに何が書き出されるかは、[プラットフォーム別のビルド](platforms.md)にあります。
