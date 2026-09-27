# インストール

dandori は実行ファイル一つです。規則を rulec で読むので、rulec も一緒に入れます。

## dandori

リポジトリをクローンし、最近の安定版の Rust でビルドします。依存は serde_json だけです。

```console
$ git clone https://github.com/i2y/dandori
$ cd dandori
$ cargo install --path .
```

## rulec

規則を読むのは rulec です。dandori は `DANDORI_RULEC` で指定した rulec を動かし、指定が無ければ PATH から探します。macOS か Linux で Homebrew を使っているなら、次の一行で入ります。

```console
$ brew install i2y/tap/rulec
```

rulec の[リリース](https://github.com/i2y/rulec/releases)から実行ファイルをダウンロードしてもかまいません。dandori は rulec 0.20.0 と 0.21.1 でテストしています。

## 動くか確かめる

例を一つ検査して、ビルドしてみます。

```console
$ dandori check examples/hotel/temporal/hotel.flow
examples/hotel/temporal/hotel.flow: ok
$ dandori build examples/hotel/temporal/hotel.flow --target temporal --out out/hotel
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
