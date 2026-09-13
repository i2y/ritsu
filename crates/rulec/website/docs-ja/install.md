# インストール

rulec はランタイムも外部依存も持たない一つのバイナリです。いまはソースから建てます。公開されたリリースはまだありません。

## ソースから

```console
$ git clone https://github.com/i2y/rulec
$ cd rulec
$ cargo install --path .
$ rulec --version
rulec 0.1.0
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

- **`rulec test`** — 生成した Python と Go を実際に走らせて参照評価器と突き合わせます。`python3` と `go` が要ります。無ければ「どちらを飛ばしたか」を言って、落ちはしません。
- **`rulec verify`** — アダプタを子プロセスとして起動するので、そのアダプタを書いた言語が要ります。

## 言語

出力の**既定は英語**です。一つの設定で日本語に戻ります — 生成コードの中の文面まで含めて、全部の面が戻ります。

```console
$ rulec check rules/送料.rule --lang ja
$ RULEC_LANG=ja rulec check rules/送料.rule
```

優先順位は `--lang` → `RULEC_LANG` → 既定。**システムのロケールは見ません。** 生成物は `gen --check` で照合され、CI のログは diff されるので、走らせた機械で出力が変わってはいけないからです。

## CI に置く

```yaml
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/
```

この五行が門です。過去再生は記録を持つ環境の別ジョブにします。そちらは**人に見せるものを作る**ので、言語を日本語に倒します。

```yaml
- run: rulec diff 送料@v3 送料@v4 --fixtures "$FIXTURES" --format markdown > diff.md
  env:
    RULEC_LANG: ja
- run: gh pr comment "$PR" --body-file diff.md
```

## 次に読むもの

<div class="grid cards" markdown>

-   __[表を書く](tour.md)__

    最初の一行から、検査を通る規則まで。

-   __[何を証明するか](checks.md)__

    七つの検査と、報告の読み方。

-   __[エージェント向け](agents.md)__

    手順の全体と、やってはいけない四つ。

</div>
