# コマンド

```text
dandori check <file.flow>...
dandori build <file.flow> --target asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph [--out <dir>]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph]
dandori doc <file.flow> [--format html] [--out <dir>]
dandori explain <CODE> | --all [--format markdown|json]
dandori <command> --help
dandori --version
```

規則を使うワークフローは、`ritsu dandori <コマンド>` で走らせます。規則を同じプロセスの中で読みます（[インストール](../install.md)）。`dandori` のコマンドだけでは、規則を使わないワークフローを検査し、ビルドします。

| コマンド | すること |
|---|---|
| `check` | 一つ以上の `.flow` を検査する。型、すべての分岐、案件が残りうる状態、リトライを見て、診断をそこへ至る実行の例付きで出す。通ったファイルには `ok` と出す |
| `build` | 一つのプラットフォーム向けのコードを `--out`（無ければ `out/`）に書き出し、書き出したファイルのパスを出す。プラットフォームにできないこと（E050）と、一回の実行がプラットフォームの上限を超えうるワークフロー（E040）はエラーにする。[プラットフォーム別のビルド](../platforms.md) |
| `scenarios` | シナリオ（入力と、各呼び出しが受け取る結果）を書き出す。全部を合わせると、すべての分岐とエラーを処理するすべての箇所を通り、案件の状態の変わり方もすべて試す。`--out` があれば一つずつファイルに書き、無ければ全部を JSON で標準出力に出す |
| `run` | シナリオを一つ参照インタプリタで動かし、呼び出しをターゲットが出す形で、待ちと終わり方と一緒に出す |
| `doc` | ワークフローを、レビューする人のために図にする。流れと、各呼び出しのすることとエラーの行き先、ワークフローの終わり方のすべてを出す。Mermaid の図を入れた Markdown か、シナリオごとに実行の通るところが光る HTML のページ一枚（`--format html`）を、`--out` があればそこに `<名前>.md` か `<名前>.html` として書き、無ければ標準出力に出す。呼び出す規則は、`rulec doc` が描いたとおりに入れる。検査でエラーが見つかるワークフローも図にし、終了コードは 1 になる。[ワークフローを図にする](../diagrams.md) |
| `explain` | 診断のコードを引く。いつ出るか、直し方、そのコードを出す最小の `.flow` を、隣に置くファイルと一緒に出す。`--all` で全部のコードを出し、`--format markdown` なら Markdown、`--format json` ならツール向けの JSON で出す。[診断コード](codes.md) |

## フラグと環境変数

| フラグ・環境変数 | 意味 |
|---|---|
| `--target <ターゲット>` | ビルドするプラットフォーム。`run` では、呼び出しをどのターゲットの形で出すか（既定は `reference`） |
| `--out <dir>` | `build`（既定は `out/`）と `scenarios`、`doc` の書き出し先 |
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
| 1 | エラーがある |
| 2 | 引数が正しくないか、ファイルが読めない |
