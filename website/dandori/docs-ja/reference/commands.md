# コマンド

```text
dandori check <file.flow>...
dandori gen <file.flow> --target asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph... [--out <dir>] [--check]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph]
dandori doc <file.flow> [--format html] [--out <dir>]
dandori api <file.flow>
dandori explain <CODE> | --all [--format markdown|json]
dandori <command> --help
dandori --version
```

規則、日付のファイル、帳簿（`use rule`、`use dates`、`use book`）を使うワークフローは、`ritsu dandori <コマンド>` で走らせます。`ritsu dandori` は、それらを同じプロセスの中で読みます（[インストール](../install.md)）。`dandori` のコマンドだけでは、どれも使わないワークフローを検査し、そのコードを書き出します。

| コマンド | すること |
|---|---|
| `check` | 一つ以上の `.flow` を検査する。型、すべての分岐、案件が残りうる状態、リトライを見て、診断をそこへ至る実行の例付きで出す。通ったファイルには `ok` と出す |
| `gen` | `--target` で指定したプラットフォーム（一つ以上）ごとに、`<out>/<target>/`（無ければ `generated/` の下）へコードを書き出し、書き出したファイルのパスを出す。書く順は、指定の順ではなく上の使い方に並べた順。プラットフォームにできないこと（E050）と、一回の実行がプラットフォームの上限を超えうるワークフロー（E040）はエラーにし、一つでもエラーになれば何も書かない。`--check` を付けると何も書かず、無いファイルと違うファイルを出し、一つでもあれば 1 で終わる。[プラットフォーム別のビルド](../platforms.md) |
| `scenarios` | シナリオ（入力と、各呼び出しが受け取る結果）を書き出す。全部を合わせると、すべての分岐とエラーを処理するすべての箇所を通り、案件の状態の変わり方もすべて試す。範囲を書いた入力の一つずつには、そこだけを範囲のすぐ外の値にした入力のシナリオも書く。`--out` があれば一つずつファイルに書き、無ければ全部を JSON で標準出力に出す |
| `run` | シナリオを一つ参照インタプリタで動かし、呼び出しをターゲットが出す形で、待ちと終わり方と一緒に出す |
| `doc` | ワークフローを、レビューする人のために図にする。流れと、各呼び出しのすることとエラーの行き先、ワークフローの終わり方のすべてを出す。Mermaid の図を入れた Markdown か、シナリオごとに実行の通るところが光る HTML のページ一枚（`--format html`）を、`--out` があればそこに `<名前>.md` か `<名前>.html` として書き、無ければ標準出力に出す。呼び出す規則は、`rulec doc` が描いたとおりに入れる。検査でエラーが見つかるワークフローも図にし、終了コードは 1 になる。[ワークフローを図にする](../diagrams.md) |
| `api` | ワークフローが宣言していることを、ソースを読まずに分かるよう、一つの JSON で出す。[形式](#api-の-json)。検査を通るワークフローが要る。通らないものには `check` と同じ診断を出し、1 で終わる |
| `explain` | 診断のコードを引く。いつ出るか、直し方、そのコードを出す最小の `.flow` を、隣に置くファイルと一緒に出す。`--all` で全部のコードを出し、`--format markdown` なら Markdown、`--format json` ならツール向けの JSON で出す。[診断コード](codes.md) |

## `api` の JSON

```console
$ ritsu dandori api examples/hotel/temporal/hotel.flow | jq '.tasks[] | select(.name == "confirm_intent") | {kind, calls, errors, refused_as, retry, key}'
{
  "kind": "http",
  "calls": {
    "method": "POST",
    "url": "https://api.stripe.com/v1/payment_intents/{intent}/confirm",
    "form": true,
    "api": "../specs/stripe.json"
  },
  "errors": [
    {
      "name": "card_declined",
      "status": 402,
      "exception": null
    },
    {
      "name": "unexpected_state",
      "status": 400,
      "exception": null
    }
  ],
  "refused_as": "unexpected_state",
  "retry": null,
  "key": true
}
```

生成器が読むのと同じ、検査を通ったプログラムから作るので、どのプラットフォーム向けに書いたワークフローでも同じことを言います。キーは次のとおりです。

| キー | 中身 |
|---|---|
| `dandori`、`file`、`workflow` | dandori の版、ファイル、ワークフローの `name`、`version`、`description`、`sha256`、`kind`（`standard` か `express`） |
| `implements` | ワークフローが実装する `.proto` のサービス。`api`、`file`、`name`、`methods`（それぞれ `request`、`response`、`marks`。`start` は失敗しうる名前の `fails`、`event` と `answer` は `task`、`status`）。実装しなければ `null` |
| `period`、`history_encrypted` | 動く期間（`text`、`min`、`max`、`line`）と、`history encrypted` の行。無ければ `null` |
| `uses` | `use` の行ごとに、`kind`（`rule`、`dates`、`book`、`openapi`、`smithy`、`proto`）、`name`、書いたとおりの `file`、`line`。規則には、規則の名前と版、`sha256`、型と範囲つきの `inputs` と `outputs`、`lambda`・`local`・`connect` の URL・`connection` のどれで呼ぶか。日付のファイルには、日付と引数。帳簿には、振替 |
| `inputs`、`outputs` | `name`、`type`、`optional`、`range`、`secret`、`line`。入力とレコードのフィールドは、`fed by` があれば、その参照を `fed_by` に持ちます。`range` は `.flow` に書くとおりの `text` と、データとしての `min`・`max`（数、日、時刻、順序のある列挙なら値。通す `values` の全部も） |
| `enums`、`records` | 値とフィールド。列挙の `from` は、`import enum` で読んだファイルと、そこでの名前（フローに書いた列挙は null）。レコードの `origin` は、ワークフロー自身のもの、規則の出力、仮押さえ、`.proto` のメッセージのどれか |
| `tasks` | `kind`（`http`、`connect`、`aws`、`lambda`、`agent`、`jev`、`book`、`workflow`、`state_machine`、`durable_function`、`argo_template`、`image`、`event`、自分で実装する `own`）、呼び先の `calls`、`params` と `result`（型、範囲、秘密）、`timeout`、`deadline`、`retry`、`key`、`idempotent`、`changes_things`、`callback`、ステータスか例外つきの `errors`、`refused_as`、`machine`、`discloses` |
| `cases` | 持つ `record`、追う状態機械 `follows`、`states` と `final`、`held`、`external` のイベント、`refused_when` |
| `children` | タスクが動かす子の `.flow` ごとに、タスク、`file`、ワークフロー、`inputs`、`outputs` |
| `fails`、`succeeds` | ワークフローの終わり方のすべて。`fail` ごとに `error`、`message`、残す案件 `leaving`、ブロック `in`（`flow`、`on failure`、`on cancel`）、`line`。`succeed` ごとに、書いたとおりの設定する `outputs` |
| `on_failure`、`on_cancel` | `on failure` があるか、`on cancel` の行 |

## フラグと環境変数

| フラグ・環境変数 | 意味 |
|---|---|
| `--target <ターゲット>` | `gen` が書き出すプラットフォーム（何度でも書ける）。`run` では、呼び出しをどのターゲットの形で出すか（既定は `reference`）。0.40.0 までは、Temporal の TypeScript が `temporal`、`dandori gen` が `dandori build` だった。どちらも、いまは新しい名前を言って 2 で終わる |
| `--out <dir>` | `gen`（既定は `generated/`。プラットフォームごとにディレクトリを分ける）と `scenarios`、`doc` の書き出し先 |
| `--check` | `gen` が何も書かず、同じ `--target` で書くはずのファイルが無いか違えば 1 で終わる（CI 用） |
| `--scenario <file.json>` | `run` が動かすシナリオ |
| `--format json` | `check` の診断を、ツール向けの JSON で出す |
| `--format html` | `doc` のページを HTML で書く。無ければ Markdown |
| `--all`、`--format markdown`、`--format json` | `explain` で全部のコードを出す。Markdown か JSON で出す |
| `--lang ja` か `--lang en` | メッセージの言語。無ければ `DANDORI_LANG`、次に `RITSU_LANG`、どちらも無ければ英語 |
| `--help` | そのコマンドが受け取るもの、終了コード、例を出す（`dandori help <command>` も同じページ）。`dandori --help` はコマンドの一覧を出す |
| `--version` | `dandori --version` はバージョンを出す |

## 終了コード

| コード | 意味 |
|---|---|
| 0 | エラーなし（警告だけか、何も無い） |
| 1 | エラーがある。`gen` では、プラットフォームのどれかがワークフローを受け取れない（何も書かない）か、`--check` で無いか違うファイルがある |
| 2 | 引数が正しくない（`--target` の無い `gen` など）か、ファイルが読めない |
