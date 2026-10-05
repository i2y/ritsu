# インストール

rulec はランタイムも外部依存も持たない一つのバイナリです。リリースごとに macOS（arm64、x64）と Linux（x64、arm64）のバイナリを、それぞれの SHA-256 と一緒に[リリースのページ](https://github.com/i2y/rulec/releases)に置いています。Linux 版は静的にリンクしてあり、macOS 版がリンクするのは、どの Mac にもあるシステムのライブラリだけです。同じバイナリを Homebrew と、`.deb`・`.rpm` のパッケージからも入れられるので、そのマシンでふだん使っている入れ方を選べます。

Homebrew、`.deb` と `.rpm`、リリースのアーカイブ、CI のアクションは、いまも rulec 自身のリリースのものです。rulec はいま [ritsu](https://github.com/i2y/ritsu) の言語の一つで、ritsu の最初のリリースは rulec の番号を引き継ぎ（0.23.0）、ここで配っているものもそちらに移ります。ソースからは、ritsu のリポジトリでビルドします（[ソースから](#ソースから)）。

## Homebrew

macOS でも Linux でも使えます。

```console
$ brew install i2y/tap/rulec
$ rulec --version
rulec 0.22.1
```

formula が入れるのは、その環境向けのリリースのアーカイブで、`SHA256SUMS` の行と突き合わせてから入れます。formula はリリースのたびに書き換わります。書き換えるのは、brew が macOS と Linux の両方で実際に入れて、formula のテストが通ったあとです。次のリリースは `brew upgrade rulec` で入ります。

## Debian・Ubuntu・Fedora・RHEL

リリースごとに、x64 と arm64 の `.deb` と `.rpm` も置いています。中身はアーカイブと同じ静的リンクのバイナリなので、依存するパッケージはありません。

```console
$ v=0.22.1; a=amd64                  # ARM なら arm64
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/v$v/rulec_$v-1_$a.deb"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/v$v/SHA256SUMS" | grep "rulec_$v-1_$a.deb" | sha256sum -c
rulec_0.22.1-1_amd64.deb: OK
$ sudo apt install "./rulec_$v-1_$a.deb"
```

```console
$ v=0.22.1; a=x86_64                 # ARM なら aarch64
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/v$v/rulec-$v-1.$a.rpm"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/v$v/SHA256SUMS" | grep "rulec-$v-1.$a.rpm" | sha256sum -c
rulec-0.22.1-1.x86_64.rpm: OK
$ sudo dnf install "./rulec-$v-1.$a.rpm"
```

パッケージに署名はしていません。アーカイブと同じく、`SHA256SUMS` の行との突き合わせが検証の全部です。パッケージのリポジトリは用意していないので、`apt upgrade` や `dnf upgrade` では新しいリリースは入りません。次のリリースも同じ手順で入れてください。

<!-- crates.io は保留中（DESIGN §15.158）。クレートを出したら、この節のコメントを外し、最初の段落の入れ方に Cargo を戻す。

## Cargo

```console
$ cargo install rulec          # ビルドする。ほかに取りに行くものは無い
$ cargo binstall rulec         # ビルドせずに、リリースのアーカイブを取る
```

`cargo install` に要るのは新しめの stable な Rust だけです。rulec は依存を持たないからです。`cargo binstall` はリリースのページと同じアーカイブを取ってきます（Linux では静的リンクのバイナリなので、glibc でも musl でも動きます）。ただし `SHA256SUMS` との突き合わせはしません。Homebrew とこのページのほかの手順は突き合わせます。
-->

## リリースのバイナリ

```console
$ v=v0.22.1; t=aarch64-apple-darwin
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/$v/rulec-$v-$t.tar.gz"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/$v/SHA256SUMS" | grep "$t" | shasum -a 256 -c
rulec-v0.22.1-aarch64-apple-darwin.tar.gz: OK
$ tar -xzf "rulec-$v-$t.tar.gz" && install -m 755 rulec ~/.local/bin/
$ rulec --version
rulec 0.22.1
```

`t` は `aarch64-apple-darwin`・`x86_64-apple-darwin`・`x86_64-unknown-linux-musl`・`aarch64-unknown-linux-musl` のどれかです。Linux の二つは静的リンクなので、どのディストリビューションでも動きます。Linux では `sha256sum -c` を使います。走らせる前に `SHA256SUMS` と突き合わせる、この一行が検証の全部なので、ここは飛ばさないでください。

## ソースから

新しめの stable な Rust で、ritsu のリポジトリから入れます。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked rulec
```

これで rulec だけがビルドされます。rulec はほかの言語を使わず、標準ライブラリの外に**依存を一つも持たない**ので、何も取りに行きません。パッケージを `rulec` ではなく `ritsu` にすると、`ritsu` と ritsu の全部の言語が入り、このサイトのコマンドは全部 `ritsu rulec <コマンド>` で使えます。入力の範囲を koyomi の日付からとる規則（`range from koyomi`）は、`ritsu rulec check` で検査します。

ビルドディレクトリから直接動かすなら:

```console
$ git clone https://github.com/i2y/ritsu
$ cd ritsu
$ cargo build --release -p rulec
$ ./target/release/rulec --help
```

## ほかに要るもの

検査には何も要りません。`rulec check`・`fmt`・`gen`・`vectors`・`coverage`・`doc`・`api`・`explain`・`schema`・`adapter`・`fixtures lint` は全部これ一つで完結します。

外に出るのは二つだけです。

- **`rulec test`** — 生成した Python・TypeScript・JavaScript・Rust・Ruby・PHP・Go・Swift・Java・SQL・Wasm・NumPy を実際に走らせて、参照評価器（rulec の中にある「正解」の実装）と突き合わせます。`python3`・`node`・`rustc`・`ruby`・`go`・`swiftc` が要ります（SQL は同じ `python3` の中の `sqlite3` で走ります）。無ければ「どれを飛ばしたか」を言って、落ちはしません。
- **`rulec verify`** — アダプタを子プロセスとして起動するので、そのアダプタを書いた言語が要ります。

生成物を**型検査したい**場合だけ、さらにツールが要ります。生成 Python は `mypy --strict` を、生成 TypeScript は `tsc --strict` を通り、生成 Ruby には `steep` が読む `.rbs` が付いてきます。どれも**使うのに必要ではありません** — 生成物はそれ自体でそのまま動きます。

## エージェントスキル

rulec を実際に使うのはたいていエージェントです。`skills/rulec/`（ritsu のリポジトリの根）は、そのためのスキルです。

入っているのは、作業の手順、文法、検査を通る規則が十八本、データの形式、それと対応していない言語へ生成するやり方です。ツールの仕様をスキルに転記してはいません。`--help` と `--format json` と `rulec explain` で rulec 本体に聞くように書いてあるので、インストールしてあるバイナリが新しくなっても古びません。

`ritsu` のバイナリが、ritsu とほかの言語のスキルと一緒に持っています。

```console
$ ritsu skills install rulec
```

`.claude/skills/rulec/` の下に `SKILL.md` と六つのファイルが置かれます。フォルダの名前でスキルが見つかるので、中身をばらして置かないでください。一つのプロジェクトではなく全部で使うなら、`--user` を付けて `~/.claude/skills/` に置きます。`--dir <dir>` を付けると、ほかのエージェントがスキルを読む場所に置きます。Claude Code なら、プラグイン `ritsu` に八つのスキルが入っています（`/plugin marketplace add i2y/ritsu` のあと `/plugin install ritsu@ritsu`）。リポジトリのクローンから `skills/rulec` をコピーしても同じで、リリースごとの `ritsu-skills-v<版>.zip` にも八つが入っています。

要るのは `rulec` が PATH にあることだけです（上のどの入れ方でも構いません）。

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

**こちらは stdio だけです。** 渡しているのはファイルを読み書きするコマンドそのものなので、シェルの代わりとして同じマシンに置くものです。`--http` はありません。これは足りないのではなく、そう決めています。

**MCP サーバはもう一つあって、そちらは別物です。** ここまでは規則を**書く**エージェントのためのツールでした。規則を**呼ぶ**エージェントには、`gen` が規則そのものをサーバとしてモジュールの隣に書きます。そちらは stdio と、**MCP の Streamable HTTP**（`--http 8000`）の両方を話します。URL しか受け付けない相手のためです。**MCP Apps** を表示できるホストには、人が読むページをそのツールの view としても渡します: [規則をエージェントのツールにする](generate.md#規則をエージェントのツールにする)。

## 言語

出力の**既定は英語**です。設定は一つで、生成コードの中の文面まで含めて、出るものが全部日本語になります。

```console
$ rulec check rules/送料.rule --lang ja
$ RULEC_LANG=ja rulec check rules/送料.rule
```

優先順位は `--lang` → `RULEC_LANG` → `RITSU_LANG` → 既定。**システムのロケールは見ません。** 生成物は `gen --check` で照合され、CI のログは diff されるので、走らせたマシンで出力が変わってはいけないからです。

## CI に置く

`uses: i2y/rulec@v0.22.1` の一行で、そのリリースのバイナリが検査済みで runner の `PATH` に入ります。action を指す ref がそのままリリースなので、既定では二つがずれません（別のリリースを入れたいときだけ `with: { version: v0.4.0 }` で明示します）。`SHA256SUMS` との突き合わせは**必ず走ります** — その行が無いだけでも落ちます。アーカイブのハッシュを workflow 側にも書いて固定したいなら、`with: { sha256: … }` を足します。検査が一つ増えます。

**入れるのに要るのはその一行だけ**ですが、その前に `actions/checkout` が要ります — rulec が読むのは、あなたのリポジトリの `rules/` だからです。ジョブ全体ではこうなります。

```yaml
check:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v7
      with:
        fetch-depth: 0                   # --diff-base が origin/main を読む
    - uses: i2y/rulec@v0.22.1
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
    - uses: i2y/rulec@v0.22.1
    # 記録を $FIXTURES に置くところはご自身で: アーティファクトか、権限を絞った保管先から
    - run: rulec diff rules/送料.rule@origin/main rules/送料.rule --fixtures "$FIXTURES" --format markdown --terse > diff.md || [ $? -eq 1 ]
      env:
        RULEC_LANG: ja
    - run: gh pr comment "$PR" --body-file diff.md
      env:
        GH_TOKEN: ${{ github.token }}
        PR: ${{ github.event.pull_request.number }}
```

上のジョブは記録が要ります。下のジョブは二つの版だけで走るので、初日から PR ごとに回せます。コメントは二つ並べて読むものです——**何が動きうるか**と、**手元の記録のうち何件が動くか**。

```yaml
- run: rulec diff rules/送料.rule@origin/main rules/送料.rule --format markdown > region.md || [ $? -eq 1 ]
  env:
    RULEC_LANG: ja
- run: gh pr comment "$PR" --body-file region.md
```


規則が複数あるなら、`git diff --name-only --diff-filter=M origin/main...HEAD -- 'rules/*.rule'` がこの PR で変わったものを並べるので、同じ二行を規則ごとに回します。

## 次に読むもの

<div class="grid cards" markdown>

-   __[ルール(.rule)を書く](tour.md)__

    最初の一行から、検査を通る規則まで。

-   __[何を証明するか](checks.md)__

    七つの検査と、報告の読み方。

-   __[エージェント向け](agents.md)__

    手順の全体と、やってはいけない四つ。

</div>
