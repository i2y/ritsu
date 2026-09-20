# インストール

rulec はランタイムも外部依存も持たない一つのバイナリです。リリースごとに macOS（arm64、x64）と Linux（x64、arm64）の静的バイナリを、それぞれの SHA-256 と一緒に[リリースのページ](https://github.com/i2y/rulec/releases)に置いています。

## リリースのバイナリ

```console
$ v=v0.9.0; t=aarch64-apple-darwin
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/$v/rulec-$v-$t.tar.gz"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/$v/SHA256SUMS" | grep "$t" | shasum -a 256 -c
rulec-v0.9.0-aarch64-apple-darwin.tar.gz: OK
$ tar -xzf "rulec-$v-$t.tar.gz" && install -m 755 rulec ~/.local/bin/
$ rulec --version
rulec 0.9.0
```

`t` は `aarch64-apple-darwin`・`x86_64-apple-darwin`・`x86_64-unknown-linux-musl`・`aarch64-unknown-linux-musl` のどれかです。Linux の二つは静的リンクなので、どのディストリビューションでも動きます。Linux では `sha256sum -c` を使います。走らせる前に `SHA256SUMS` と突き合わせる、この一行が検証の全部なので、ここは飛ばさないでください。

## ソースから

```console
$ git clone https://github.com/i2y/rulec
$ cd rulec
$ cargo install --path .
$ rulec --version
rulec 0.9.0
```

新しめの stable な Rust があれば足ります。rulec は標準ライブラリの外に**依存を一つも持たない**ので、`cargo install` は何も取りに行きません。

ビルドディレクトリから直接動かすなら:

```console
$ cargo build --release
$ ./target/release/rulec --help
```

## ほかに要るもの

検査には何も要りません。`rulec check`・`fmt`・`gen`・`vectors`・`coverage`・`doc`・`api`・`explain`・`schema`・`adapter`・`fixtures lint` は全部これ一つで完結します。

外に出るのは二つだけです。

- **`rulec test`** — 生成した Python・TypeScript・JavaScript・Rust・Ruby・PHP・Go・Swift・Java・SQL・Wasm・NumPy を実際に走らせて、参照評価器（rulec の中にある「正解」の実装）と突き合わせます。`python3`・`node`・`rustc`・`ruby`・`go`・`swiftc` が要ります（SQL は同じ `python3` の中の `sqlite3` で走ります）。無ければ「どれを飛ばしたか」を言って、落ちはしません。
- **`rulec verify`** — アダプタを子プロセスとして起動するので、そのアダプタを書いた言語が要ります。

生成物を**型検査したい**場合だけ、さらに道具が要ります。生成 Python は `mypy --strict` を通り、生成 Ruby には `steep` が読む `.rbs` が付いてきます。どちらも**使うのに必要ではありません** — 生成物はそれ自体でそのまま動きます。

## エージェントスキル

rulec を実際に使うのはたいていエージェントです。`skills/rulec/` は、そのためのスキルです。

入っているのは、作業の手順、文法、検査を通る規則が十八本、データの形式、それと対応していない言語へ生成するやり方です。道具の仕様をスキルに書き写してはいません。`--help` と `--format json` と `rulec explain` で rulec 本体に聞くように書いてあるので、手元のバイナリが新しくなっても古びません。

フォルダごとコピーします。

```console
$ git clone https://github.com/i2y/rulec /tmp/rulec
$ mkdir -p .claude/skills
$ cp -r /tmp/rulec/skills/rulec .claude/skills/
```

`.claude/skills/rulec/` の下に `SKILL.md` と五つのファイルが置かれます。フォルダの名前でスキルが見つかるので、中身をばらして置かないでください。一つのプロジェクトではなく全部で使うなら、`~/.claude/skills/` に置きます。

要るのは `rulec` が PATH にあることだけです（上の節のとおり）。

あとは「この運賃表から `.rule` を書いて」「この E101 を直して」のように頼めば、たいていはスキルが自動で使われます。**確実に使わせたいときは「rulec のスキルを使って」と名指しで頼んでください。**

## MCP サーバ

エージェントにシェルが無いとき（チャットの画面、MCP で話す IDE の補助）は、同じコマンドがツールとして使えます。

```console
$ claude mcp add rulec -- rulec mcp
```

どのクライアントでも、コマンドが `rulec mcp` の stdio サーバとして登録できます。

```json
{ "mcpServers": { "rulec": { "command": "rulec", "args": ["mcp"] } } }
```

コマンド一つがツール一つ（`rulec_check`、`rulec_gen`、`rulec_doc` …）、フラグ一つが引数一つで、結果の最後に exit code が付きます。手順書とリファレンスはリソースとして出るので、このリポジトリを読めないエージェントでも、まず `rulec://docs/agents.md` を読めます。形は[形式](formats.md#mcp)にあります。

これは規則を書くエージェントのためのツールです。規則を呼ぶエージェントには、`gen` が規則そのものを MCP サーバとしてモジュールの隣に書きます: [規則をエージェントのツールにする](generate.md#規則をエージェントのツールにする)。

## 言語

出力の**既定は英語**です。設定は一つで、生成コードの中の文面まで含めて、出るものが全部日本語になります。

```console
$ rulec check rules/送料.rule --lang ja
$ RULEC_LANG=ja rulec check rules/送料.rule
```

優先順位は `--lang` → `RULEC_LANG` → 既定。**システムのロケールは見ません。** 生成物は `gen --check` で照合され、CI のログは diff されるので、走らせた機械で出力が変わってはいけないからです。

## CI に置く

`uses: i2y/rulec@v0.9.0` の一行で、そのリリースのバイナリが検査済みで runner の `PATH` に入ります。action を指す ref がそのままリリースなので、既定では二つがずれません（別のリリースを入れたいときだけ `with: { version: v0.4.0 }` で明示します）。`SHA256SUMS` との突き合わせは**必ず走ります** — その行が無いだけでも落ちます。アーカイブのハッシュを workflow 側にも書いて固定したいなら、`with: { sha256: … }` を足します。検査が一つ増えます。

**入れるのに要るのはその一行だけ**ですが、その前に `actions/checkout` が要ります — rulec が読むのは、あなたのリポジトリの `rules/` だからです。ジョブ全体ではこうなります。

```yaml
check:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v7
      with:
        fetch-depth: 0                   # --diff-base が origin/main を読む
    - uses: i2y/rulec@v0.9.0
    - run: rulec fmt --check rules/
    - run: rulec check rules/ --diff-base origin/main
    - run: rulec gen rules/ --out generated/ --check
    - run: rulec coverage rules/
    - run: rulec test generated/
```

走るのは Linux（x86_64 / aarch64）と macOS（x86_64 / arm64）の runner です。リリースがその四つしか無いので、ほかの runner では `no rulec release is built for …` と言って止まります。

この五つの `run:` がゲートです。過去再生は記録を持つ環境の別ジョブにします。変更が目に見えるのはこちらで、PR に「何件がいくら動くか」のコメントが付きます。わざとそうしている所が四つあります。

- 旧の版は `rules/送料.rule@origin/main`、つまり base ブランチにあるままのファイルです。checkout でそのブランチを取ってきておきます。
- `diff` は影響があると exit 1 を返します。ここではそれは失敗ではなく情報なので、1 では先へ進み、2 でだけ止めます。
- `--terse` は入力例の列を出しません。コメントはリポジトリを読める全員が見るもので、本番の記録の値を置く場所ではないからです。
- 文面の言語は、その PR を読む人の言語にします。貼る先がそこだからです。

```yaml
replay:
  if: github.event_name == 'pull_request'
  runs-on: ubuntu-latest
  permissions:
    contents: read
    pull-requests: write
  steps:
    - uses: actions/checkout@v7
      with:
        fetch-depth: 0                   # 旧の版は origin/main から読む
    - uses: i2y/rulec@v0.9.0
    # 記録を $FIXTURES に置くところはご自身で: アーティファクトか、権限を絞った保管先から
    - run: rulec diff rules/送料.rule@origin/main rules/送料.rule --fixtures "$FIXTURES" --format markdown --terse > diff.md || [ $? -eq 1 ]
      env:
        RULEC_LANG: ja
    - run: gh pr comment "$PR" --body-file diff.md
      env:
        GH_TOKEN: ${{ github.token }}
        PR: ${{ github.event.pull_request.number }}
```

規則が複数あるなら、`git diff --name-only --diff-filter=M origin/main...HEAD -- 'rules/*.rule'` がこの PR で変わったものを並べるので、同じ二行を規則ごとに回します。

## 次に読むもの

<div class="grid cards" markdown>

-   __[表(.rule)を書く](tour.md)__

    最初の一行から、検査を通る規則まで。

-   __[何を証明するか](checks.md)__

    七つの検査と、報告の読み方。

-   __[エージェント向け](agents.md)__

    手順の全体と、やってはいけない四つ。

</div>
