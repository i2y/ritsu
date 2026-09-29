# コマンド

```text
dandori check <file.flow>...
dandori build <file.flow> --target asl|temporal|temporal-python|durable|argo|pydantic-graph [--out <dir>]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|durable|argo|pydantic-graph]
dandori doc <file.flow> [--format html] [--out <dir>]
```

| コマンド | すること |
|---|---|
| `check` | 一つ以上の `.flow` を検査する。型、すべての分岐、案件が残りうる状態、リトライを見て、診断をそこへ至る実行の例付きで出す。通ったファイルには `ok` と出す |
| `build` | 一つのプラットフォーム向けのコードを `--out`（無ければ `out/`）に書き出し、書き出したファイルのパスを出す。プラットフォームにできないこと（E050）と、一回の実行がプラットフォームの上限を超えうるワークフロー（E040）はエラーにする。[プラットフォーム別のビルド](../platforms.md) |
| `scenarios` | シナリオ（入力と、各呼び出しが受け取る結果）を書き出す。全部を合わせると、すべての分岐とエラーを処理するすべての箇所を通り、案件の状態の変わり方もすべて試す。`--out` があれば一つずつファイルに書き、無ければ全部を JSON で標準出力に出す |
| `run` | シナリオを一つ参照インタプリタで動かし、呼び出しをターゲットが出す形で、待ちと終わり方と一緒に出す |
| `doc` | ワークフローを、レビューする人のために図にする。流れと、各呼び出しのすることとエラーの行き先、ワークフローの終わり方のすべてを出す。Mermaid の図を入れた Markdown か、シナリオごとに実行の通るところが光る HTML のページ一枚（`--format html`）を、`--out` があればそこに `<名前>.md` か `<名前>.html` として書き、無ければ標準出力に出す。検査でエラーが見つかるワークフローも図にし、終了コードは 1 になる。[ワークフローを図にする](../diagrams.md) |

## フラグと環境変数

| フラグ・環境変数 | 意味 |
|---|---|
| `--target <ターゲット>` | ビルドするプラットフォーム。`run` では、呼び出しをどのターゲットの形で出すか（既定は `reference`） |
| `--out <dir>` | `build`（既定は `out/`）と `scenarios`、`doc` の書き出し先 |
| `--scenario <file.json>` | `run` が動かすシナリオ |
| `--format json` | `check` の診断を、ツール向けの JSON で出す |
| `--format html` | `doc` のページを HTML で書く。無ければ Markdown |
| `--lang ja` か `--lang en` | メッセージの言語。無ければ `DANDORI_LANG`、それも無ければ英語 |
| `DANDORI_RULEC` | rulec の実行ファイル。規則を使うワークフローのときだけ動かす。無ければ PATH の `rulec` |

## 終了コード

| コード | 意味 |
|---|---|
| 0 | エラーなし（警告だけか、何も無い） |
| 1 | エラーがある |
| 2 | 引数が正しくないか、ファイルが読めない |
