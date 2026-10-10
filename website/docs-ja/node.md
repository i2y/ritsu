# Node で使う

0.26.0 から、ritsu のリリースには npm のパッケージ `@i2y/ritsu` も付きます。中身は同じ `ritsu` を WebAssembly（WASI）向けに組んだもので、Node が動かします。ほかのプログラムやネットワークが要るいくつかのコマンド（[下に書きます](#node-では動かないコマンド)）のほかは、ネイティブのバイナリと同じものを出力します。JavaScript のプログラムから呼ぶこともできます。ネイティブのバイナリが要らず、依存するパッケージも無く、入れるときに走るスクリプトもありません。そのため、ネイティブのバイナリを置きにくいところで使えます。たとえば、JavaScript や TypeScript のプロジェクトの CI、知っているリポジトリを検査する Node のサービス、たくさんのリポジトリをクローンして `npm install --ignore-scripts` でパッケージを入れ、ネットワークなしでコマンドを走らせ、変わったところがあればプルリクエストを出す仕組みです。

## 入れ方

このパッケージは npm のレジストリにはありません。リリースに `i2y-ritsu-<バージョン>.tgz` として付いていて、その SHA-256 はリリースの `SHA256SUMS` に載っています（[リリースのページ](https://github.com/i2y/ritsu/releases)）。そこから入れます。

```console
$ npm install --save-dev https://github.com/i2y/ritsu/releases/download/v0.26.0/i2y-ritsu-0.26.0.tgz
```

ファイルをダウンロードして `SHA256SUMS` と照らし合わせてから、そのファイルを入れてもかまいません（`npm install --save-dev ./i2y-ritsu-0.26.0.tgz`）。この入れ方は `--ignore-scripts` を付けても、ネットワークが無くても（`--offline`）動きます。Node 22 以降が要り、Linux と macOS で動きます。確かめているのは Node 22、24、26 です。

Node 22 では、パッケージを読み込むと、プロセス全体で V8 の fast API の呼び出しを止めます（`v8.setFlagsFromString("--no-turbo-fast-api-calls")`）。Node 22 の WASI は、その形で呼ばれるとプロセスを落とすことがあるためです。Node 23 以降では起きません。変わるのは呼び出しの速さだけで、結果は変わりません。ただし、Node 22 でこのパッケージを読み込むアプリも、このフラグの下で動きます。Node 23 以降では何も立てません。

## コマンド

パッケージは `node_modules/.bin` に九つのコマンドを置きます。`ritsu` と、言語の名前の `rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai`、`sekisho` です。言語の名前のコマンドは、リリースのアーカイブのリンクと同じく、`ritsu <言語> …` と同じものです。規則、カレンダー、日付のファイル、帳簿、ワークフローが一つずつあるプロジェクトで走らせると、次のようになります。

<!-- run in crates/ritsu/tests/projects/stockroom -->
```console
$ npx ritsu check --lang ja
ok rules/delivery.rule
…
ritsu check: ファイル 5 個（rulec 1、koyomi 2、chobo 1、dandori 1）。どれも検査を通りました（警告 4 件）。言語の境目: 確かめた 4 か所、決められない 4 か所
$ npx rulec check rules/delivery.rule --lang ja
ok rules/delivery.rule
$ npx ritsu gen --target typescript --lang ja
生成しました: generated/typescript/rules/delivery.ts
生成しました: generated/typescript/dates/weekdays.ts
…
```

終了コードはネイティブのコマンドと同じで、0 は問題なし（警告は出ることがある）、1 はエラーあり、2 は引数の誤りか読めないファイルです。標準入力、標準出力、標準エラーはプロセスのものをそのまま使います。`git diff | npx geas affected spec.geas -` のようにパイプから読めますし、出力も順に出ていきます。

## `ritsu gen --format json` の出力

`ritsu gen --check` は何も書かずに、パッケージのファイルのうち、無いもの、古いもの、手で変えられたもの、もう生成しないのに残っているものを挙げます。`--format json` を付けると、同じことをプログラムが読める形で出します。リポジトリごとに、生成したコードが今の状態に合っているかを記録しておく、といった使い方ができます。

```console
$ echo '// 手で足した行' >> generated/typescript/index.ts
$ npx ritsu gen --target typescript --check --lang ja
生成物が古いか手で編集されています: generated/typescript/index.ts
$ npx ritsu gen --target typescript --check --format json --lang ja
{
…
  "ok": false,
  "check": true,
  "out": "generated",
  "targets": [
    "typescript"
  ],
  "files": [
    {
      "target": "typescript",
      "path": "generated/typescript/rules/delivery.ts",
      "from": "rules/delivery.rule",
      "state": "same"
    },
…
    {
      "target": "typescript",
      "path": "generated/typescript/index.ts",
      "from": null,
      "state": "stale"
    },
…
  ],
  "diagnostics": [],
  "error": null
}
```

最初の三つのキーは `ritsu check --format json` と同じで、バージョン、ルート、問題が無いか（終了コードが 0 か）です。そのあとに、`--check` を付けたか、`--out`、頼んだ言語が続きます。`files` には、ファイルごとに、パッケージの言語（ゲートの Cedar なら `cedar`）、テキストの出力と同じ書き方のパス、生成のもとになったプロジェクトのファイル（インデックスや依存を書くファイルのように、パッケージそのものの部分なら null）、状態が入ります。状態は、書いたときは `written`、`same`、`removed` のどれか、`--check` のときは `same`、`missing`、`stale`、`left` のどれかです。何も生成しなかったときは、`files` が空になり、`error` にその理由と終了コードが入り、`diagnostics` に検査を通らなかったファイルの診断が `ritsu check --format json` と同じ形で入ります。この JSON は、パッケージでなくネイティブのバイナリでも同じものが出ます。

## JavaScript から呼ぶ

```js
import { run, runSync, version, wasmPath } from "@i2y/ritsu";
```

`run(args, options?)` は `ritsu <args>` をワーカースレッドで走らせ、`{ code, stdout, stderr }` を返す Promise を返します。走っているあいだもイベントループは止まらず、いくつ同時に呼んでもかまいません。`runSync(args, options?)` は呼んだスレッドで走らせます。`args` にはプログラムの名前を入れません（`["check", ".", "--format", "json"]`。言語のコマンドは `["rulec", "doc", "fee.rule"]` のように書きます）。`options.cwd` はコマンドが作業するディレクトリ（既定は `process.cwd()`）、`options.env` は環境変数（既定は `process.env`。渡すとそれに置き換わります）、`options.stdin` は標準入力に渡す中身（既定は空）です。`options.dirs` を渡すと、モジュールが開けるのはそのディレクトリだけになり、それぞれが同じパスで見えます（相対パスは `options.cwd` から読みます。既定はネイティブのバイナリと同じく、ファイルシステムの全部）。`options.cwd` はそのどれかの中でなければなりません。渡されたリポジトリを検査するサービスなら、そのリポジトリだけを渡せます。すると、リポジトリの外へ出るパスは、名前で出るもの（`import proto "../../other.proto"`）も、シンボリックリンク（ファイルへのものも、途中のディレクトリへのものも）で出るものも、読むことも書くこともできなくなります。コマンドは読めないと言い、外にあるものの中身は診断に出ません。これは、信用している ritsu が読むファイルを絞るためのもので、信用できないコードを Node の WASI で閉じ込めるものではありません。Node の文書も、そういう使い方には頼らないよう書いています。`version` はモジュールを組んだ ritsu のバージョン、`wasmPath` はモジュールのパスです。上と同じプロジェクトで走らせると、次のようになります（パッケージを `--lang ja` で生成したので、`--check` も同じ言語で走らせます。生成物のコメントの言語が違うと、古いと言われます）。

```js
import { run } from "@i2y/ritsu";

const { code, stdout } = await run(["gen", "--target", "typescript", "--check", "--format", "json", "--lang", "ja"]);
const report = JSON.parse(stdout);
console.log(code, report.ok, report.files.filter((f) => f.state !== "same").map((f) => `${f.state} ${f.path}`));
```

```text
1 false [ 'stale generated/typescript/index.ts' ]
```

CommonJS からも、Node 22.12 以降なら `require("@i2y/ritsu")` で同じものが使えます。

## Node では動かないコマンド

WASI 向けの WebAssembly のモジュールは、ほかのプログラムを起動することも、ネットワークに接続することもできません。それが要るコマンドは、そのことと、ネイティブの ritsu（リリースのアーカイブ、Homebrew、`.deb`、`.rpm`）で走らせてほしいことを言って止まります。終了コードは、要るプログラムが見つからないときのそのコマンドのものと同じです。

| コマンド | 要るもの |
|---|---|
| `rulec source fetch`、`rulec source outdated`、`koyomi` と `yuen` の同じコマンド | ネットワーク（e-Gov、eCFR、GOV.UK、URL）。`curl` を使う |
| `rulec test` | 生成したコードを動かすツールチェーン |
| `rulec verify --adapter …`、`rulec source fetch --via …` | 渡したプログラム |
| `rulec mcp` | ツールを呼ぶたびに起動する `rulec` |
| `rulec check --diff-base`、`<ファイル>@<リビジョン>` を読む `rulec diff` と `rulec replay`、`chobo check --diff-base` | `git` |
| `geas check`、`geas snap`、`geas drift`、`geas map` | 主張が動かすプログラム |
| `code rust` を書いた地図の `sakai check`（と `ritsu check`） | `cargo metadata`。sakai は E107 を出し、ほかの検査は続ける |

ほかのコマンドは、ネイティブのバイナリと同じに動きます。各言語の `check`、言語をまたぐ `ritsu check`、`ritsu gen` と `ritsu run`、すべての `doc`、`api`、`vectors`、`export`、`trace`、`affected`、`scenarios`、`explain`、それに `ritsu skills install` です。リリースのたびに、八つの言語の例、ritsu のテストのプロジェクト、[ブラウザで試すページ](playground.md)のプロジェクトで、そこで走らせるコマンドを英語と日本語で、パッケージとネイティブのバイナリの両方で走らせ、出力、終了コード、書いたファイルが一字も違わないことを確かめています。

Windows では、パッケージはすぐに理由を言って止まります。作業するディレクトリとパスを、そのままの形では WebAssembly のモジュールに渡せないからです。ネイティブの ritsu か、WSL を使ってください。

## ライセンス

ネイティブのバイナリと同じく `(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause` です。モジュールには、バイナリが持つ他者のもののほかに、WASI の C ライブラリである wasi-libc の一部が入ります。それぞれの出どころとライセンスの文は [THIRD_PARTY_NOTICES](https://github.com/i2y/ritsu/blob/main/THIRD_PARTY_NOTICES) にあり、このファイルはパッケージにも入っています。
