# geas

**エージェントが書いたコードに、人が読んで確かめた主張を守らせる。**

geas は、エージェントが書いたコードを受け入れてよいかを確かめるための、小さな言語です。エージェントにコードを書かせる側には仕様書（Spec Kit や Kiro）がありますが、仕様書は文章で、そのままでは確かめられません。コードを書いたエージェントが自分で書いたテストは、自分の答案を自分で採点するのと変わりません。

geas の主張のファイルには、コマンドが何を出力するか、サービスが何を返すか、画面に何が出るかといった、プログラムを外から見て分かることを書きます。一行は、声に出して読めるくらいの短さです。geas は、どの主張も実際のプログラムに対して走らせ、人にはテキストで、エージェントには JSON で報告します。プログラムが何の言語で書かれていてもかまいません。

**人は 50 行の主張を読み、エージェントは 5,000 行のコードを書き、ゲートがその二つを突き合わせる。**

```geas
# 小さな HTTP サービスの主張（greeter.geas の日本語版）。主張ごとに新しいサーバーを
# 別々のポートで起動するので、下の「足した数が積み上がる」は、ほかの主張に左右されない。

target api {
  serve "python3 server.py {port}"
  port auto
}

# レスポンスの日時とサーバーの名前は毎回変わるもので、振る舞いではない。
mask header "date"
mask header "server"

claim "名前で挨拶する" {
  when api.get("/greet?name=Alice")
  then status is 200
  and  body json ".message" is "Hello, Alice"
}

claim "空の名前は断る" {
  when api.get("/greet?name=")
  then status is 400
}

claim "足した数が積み上がる" {
  when api.post("/reset")
  then status is 200
  when api.post("/add", body: "5")
  when api.post("/add", body: "7")
  when api.get("/total")
  then body is "12"
}

claim "知らないパスには 404 を返す" {
  when api.get("/nope")
  then status is 404
}
```

キーワードは英語で、主張の名前やコメントは日本語でも書けます。主張には、関数もファイルも CSS セレクターも行番号も出てきません。geas がプログラムに触れるのは、人が触れられるところだけです。だから主張のファイルはどんなコードベースの横にも置けて、コードのほうは何も変えずに済みます。

- **ループ**：`geas check` が主張を一つずつ、毎回新しく起動したプログラムに対して走らせます。成り立たなかった主張には、そこに至るまでの実行が付きます。
- **主張のない変化**：`geas drift` がもう一度走らせて、人が見て分かる変化を全部出し、どの主張も約束していない変化に印を付けます。
- **変更が関わる主張**：`geas map` が、ランタイムにもともとあるカバレッジ（Python、Node、Go、Rust）を使って、主張ごとに通った行を記録します。`geas affected` は差分をその記録と突き合わせ、何も走らせずに、読み直すべき主張と、どの主張も通らない変更を挙げます。
- **画面**：ヘッドレスの Chrome で開いたページ、pixie のアプリ、ドライバーの先にある GUI について、スクリーンリーダーに伝わる内容（ロール、名前、値、状態）で主張を書けます。
- **いつ走らせても同じ**：環境変数、タイムゾーン、ロケール、時刻、乱数のシードを固定できます。`--jobs` で主張を並列に走らせても、一つずつ走らせたときとバイト単位で同じ出力になります。
- **AI エージェント向け**：どの診断にもコードと、そこに至る実行が付き、`geas explain` が直し方を説明します。`geas skill` は、エージェントに要る手引きを出します。

## ループ

[examples/greeter](examples/greeter) は小さな HTTP サービスです。最初の `server.py` は、エージェントがよく書くようなコードでした。もっともらしく、きれいで、空の名前を断るチェックだけが抜けています。主張がそれを見つけます。

```console
$ geas check examples/greeter/greeter.ja.geas --lang ja
ok 1 - 名前で挨拶する
not ok 2 - 空の名前は断る
    examples/greeter/greeter.ja.geas:21: status is 400 のはずが、実際は 200
        21 |   then status is 400
      ここまでの実行:
          20  when api.get("/greet?name=")  →  200, body "{\"message\": \"Hello, \"}"
ok 3 - 足した数が積み上がる
ok 4 - 知らないパスには 404 を返す
主張 4 件 · 成り立った 3 件 · 成り立たなかった 1 件 · ジャーナル: examples/greeter/.geas/greeter.ja.journal.jsonl
$ echo $?
1
```

成り立たなかったチェックには、何を期待して実際に何が返ってきたかと、主張のファイルの行と、ここまでの実行が付きます。ここまでの実行とは、そのチェックまでの `when` と、それぞれにサービスが返したものです。主張が求めている 3 行（`if not name: 400`）を足すと、こうなります。

```console
$ geas check examples/greeter/greeter.ja.geas --lang ja
ok 1 - 名前で挨拶する
ok 2 - 空の名前は断る
ok 3 - 足した数が積み上がる
ok 4 - 知らないパスには 404 を返す
主張 4 件 · 成り立った 4 件 · 成り立たなかった 0 件 · ジャーナル: examples/greeter/.geas/greeter.ja.journal.jsonl
$ echo $?
0
```

`--json` を付けると、同じ報告をエージェント向けのデータとして出します。やりとりとチェックの結果は、どの実行でもジャーナル（`.geas/greeter.ja.journal.jsonl`）に残ります。

メッセージは、`--lang ja` を付けるか `GEAS_LANG=ja` を設定すると日本語になり、どちらもなければ英語です。`ok` と `not ok` は TAP の書き方なので英語のままで、JSON のキーも英語のままです。

## 主張のない変化

テストランナーに分かるのは、チェックが成り立つかどうかだけで、チェックしていないところで何が変わったかは分かりません。`geas snap` は、実行の結果を、ヘッダーの全部とボディの全体まで含めて、すべてベースラインに残します。`geas drift` はもう一度走らせて、その全部を比べます。エージェントがサービスをリファクタリングしたとしましょう。

```console
$ geas snap examples/greeter/greeter.ja.geas --lang ja
主張 4 件 · 成り立った 4 件 · 成り立たなかった 0 件 · ベースライン: やりとり 7 件 → examples/greeter/.geas/greeter.ja.baseline.jsonl
$ cp examples/greeter/server_refactored.py examples/greeter/server.py
$ geas check examples/greeter/greeter.ja.geas --lang ja
ok 1 - 名前で挨拶する
ok 2 - 空の名前は断る
ok 3 - 足した数が積み上がる
ok 4 - 知らないパスには 404 を返す
主張 4 件 · 成り立った 4 件 · 成り立たなかった 0 件 · ジャーナル: examples/greeter/.geas/greeter.ja.journal.jsonl
$ geas drift examples/greeter/greeter.ja.geas --lang ja
主張 "名前で挨拶する" の when#1 api.get("/greet?name=Alice")
  + body json ".debug": 現れた: {"handler":"greet_v2"}   [主張なし]
主張 "空の名前は断る" の when#1 api.get("/greet?name=")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [主張なし]
主張 "足した数が積み上がる" の when#1 api.post("/reset")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [主張なし]
主張 "足した数が積み上がる" の when#2 api.post("/add", body: "5")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [主張なし]
主張 "足した数が積み上がる" の when#3 api.post("/add", body: "7")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [主張なし]
主張 "知らないパスには 404 を返す" の when#1 api.get("/nope")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [主張なし]
  ~ body: "not found" → "Not Found"   [主張なし]
ドリフト: 比べたやりとり 7 件 · 変わったやりとり 6 件 · 主張なしの変化 7 件 · 主張ありの変化 0 件
$ echo $?
1
```

主張はどれも成り立ったままですが、人が見て分かる変化が七つあります。JSON のレスポンスに `.debug` として内部の情報が出るようになり、テキストのレスポンスには charset が付き、404 の本文は `not found` から `Not Found` に変わりました。どれにも `[主張なし]` が付いています。そこを約束している主張がないので、このまま出してよいかは人が決めます。チェックしているフィールドが変わったときは、`[主張あり — 判定は geas check]` が付きます。

JSON のボディはパスごとに比べるので、`.message` の主張があっても、その横に増えた `.debug` を見逃しません。`date` と `server` のヘッダーは実行のたびに変わるので、主張のファイルの二行の `mask` で比べないようにしてあります。マスクを足しても、ベースラインを取り直す必要はありません。

## 変更が関わる主張

`drift` が読むのは実行です。もう半分は差分を読み、この変更がどの主張に関わるか、変わったコードのうちどの主張も通らないのはどこかを示します。`geas map` は、各ランタイムのカバレッジを外から有効にして主張を走らせ、主張ごとに通った行を記録します。`geas affected` は、unified 形式の差分とその記録を読み、何も走らせずに報告します。

今度は、エージェントが `server.py` に手を入れました。名前の前後の空白を取ってからチェックし、`/health` への応答を足し、`log_message` のオーバーライドを消しています（[tests/changes/greeter/change.diff](tests/changes/greeter/change.diff)）。

```console
$ geas map examples/greeter/greeter.ja.geas --lang ja
ok 1 - 名前で挨拶する
ok 2 - 空の名前は断る
ok 3 - 足した数が積み上がる
ok 4 - 知らないパスには 404 を返す
主張 4 件 · 成り立った 4 件 · 成り立たなかった 0 件 · ジャーナル: examples/greeter/.geas/greeter.ja.journal.jsonl
記録: ファイル 2 件 · コード 40 行 · どれかの主張が通った行 38 行 → examples/greeter/.geas/greeter.ja.map.jsonl
$ git diff | geas affected examples/greeter/greeter.ja.geas - --lang ja
差分: <stdin> · 記録: examples/greeter/.geas/greeter.ja.map.jsonl（変更後のコード）
この変更が関わる主張:
  1 - 名前で挨拶する
      examples/greeter/server.py: 21
  2 - 空の名前は断る
      examples/greeter/server.py: 21
  4 - 知らないパスには 404 を返す
      examples/greeter/server.py: 29
  `api` を起動するすべての主張: 1、2、3、4
      examples/greeter/server.py: 削除した行 10-11（通る行の隣）
どの主張も通らないコードの変更:
  examples/greeter/server.py: 30
主張 4 件 · 関わる主張 4 件 · どの主張も通らない変更 1 行
$ echo $?
1
```

21 行目（名前の空白を取る行）は、名前を送る二つの主張が通る行なので、読み直すのはこの二つです。30 行目（`/health` への応答）は、どの主張も通りません。誰も約束していない振る舞いなので、エージェントは次に、これを確かめる主張を人に提案します。削除した 2 行は、サービスを起動したときにどの主張も通る行に挟まれていました。

記録には、ソースファイルごとに git の blob ハッシュが入っています。`affected` は、別のコードで取った記録を推測で使うことはせず、断ります（E062）。記録は判定を変えません。変えるのは、レビューでどこを読むかだけです。

| 言語 | geas が有効にするもの | プログラムの側に要ること |
|---|---|---|
| Python 3.12 以降 | geas の `sitecustomize.py` を `PYTHONPATH` に置く | なし |
| Node（JavaScript と、型を取り除いて動かす TypeScript） | `NODE_V8_COVERAGE` と、`NODE_OPTIONS` に入れるフック | なし |
| Go | `GOCOVERDIR` | `go build -cover` でビルドする。サービスなら、SIGTERM で `main` から戻る |
| Rust | `LLVM_PROFILE_FILE` | `-C instrument-coverage` でビルドする。サービスなら、SIGTERM で普通に終わる |

報告の各部分がどう決まるか、プルリクエストで二つをどう走らせるかは [skills/geas/map.md](../../skills/geas/map.md)（英語）にあります。

## 画面

GUI の主張が見るのは、スクリーンリーダーに伝わる画面、つまりロール、名前、値、状態を持つノードの木です。[examples/web-greeter](examples/web-greeter) は、ページとその裏のサービスで、ヘッドレスの Chrome で開きます。

```geas
target web {
  serve "python3 server.py {port}"
  port auto
}

# The page writes the date, greets by the time of day and draws a fortune at
# random. Fixed here, they read the same on every machine and in every run.
locale "en-US"
tz "Asia/Tokyo"
clock "2026-08-29T09:00:00+09:00"
seed 7
…
claim "greets the name typed in, by the time of day" {
  when web.open("/")
  when web.input("Ada", into: "Your name")
  then screen contains button "Greet" enabled
  when web.click("Greet")
  then screen contains text "Good morning, Ada!" in status
  and  screen contains text "Ada" in list "Greeted"
  and  screen contains textbox "Your name" with value ""
}
```

```console
$ geas check examples/web-greeter/web-greeter.geas --lang ja
ok 1 - the first screen asks for a name
ok 2 - greets the name typed in, by the time of day
ok 3 - draws a fortune
ok 4 - the service keeps the names the page greets
ok 5 - a name sent to the service shows on the page
ok 6 - the service refuses an empty name
主張 6 件 · 成り立った 6 件 · 成り立たなかった 0 件 · ジャーナル: examples/web-greeter/.geas/web-greeter.journal.jsonl
```

主張ごとに、サービスもブラウザーコンテキストも新しくなります。ページは仮想時間で動き、固定した時刻から始まります。そのため、日付や挨拶はどのマシンでも同じになり、ページのタイマーも実際の時間を待たずに進みます。乱数のシードを固定してあるので、占いの文句も毎回同じです。

ここで、エージェントが Greet ボタンを、クリックで動く `<div>` に作り替えたとします。見た目は変わらず、マウスでも押せます。

```console
$ geas check examples/web-greeter/web-greeter.geas --lang ja
not ok 1 - the first screen asks for a name
    examples/web-greeter/web-greeter.geas:27: screen contains button "Greet" disabled のはずが、実際は当てはまるノードがありません
        27 |   and  screen contains button "Greet" disabled
      画面:
        heading "Greeter"
        text "Today is Saturday, August 29, 2026."
        form
          text "Your name"
          textbox "Your name"
          text "Greet"
        status
        list "Greeted"
        text "Your fortune: A pleasant surprise is waiting for you."
…
not ok 2 - greets the name typed in, by the time of day（エラー）
    エラー[E035]: examples/web-greeter/web-greeter.geas:35:3: `web` のページに、"Greet" という名前のボタン、リンク、チェックボックス、ラジオボタン、スイッチ、タブ、メニュー項目、選択肢がありません
        35 |   when web.click("Greet")
…
      = その画面にクリックできるものはありません
…
主張 6 件 · 成り立った 4 件 · 成り立たなかった 2 件 · ジャーナル: examples/web-greeter/.geas/web-greeter.journal.jsonl
```

ボタンがあった場所には、"Greet" という文字列しか残っていません。`<div>` はスクリーンリーダーにとってボタンではないので、主張にとってもボタンではありません。直すのはページのほうです。

デスクトップのアプリにも、同じ種類の主張が使えます。pixie は GUI を書くための言語で、pixie のアプリは操作のスクリプトをヘッドレスで再生できます。[examples/pixie-greeter](examples/pixie-greeter) は、pixie の greeter についての主張です。geas は主張ごとの操作を一つのスクリプトにまとめて走らせ、操作のたびにアクセシビリティツリーを読みます。

```console
$ geas check examples/pixie-greeter/greeter.geas --lang ja
ok 1 - asks for a name
ok 2 - greets the name typed in
ok 3 - greets on Enter too
ok 4 - the note is a field of its own
主張 4 件 · 成り立った 4 件 · 成り立たなかった 0 件 · ジャーナル: examples/pixie-greeter/.geas/greeter.journal.jsonl
```

ほかの GUI（デスクトップのアクセシビリティ API やエミュレーター）は、ドライバーを通して操作します。ドライバーは、操作を JSON の行で受け取り、そのたびに画面を返すプログラムです。操作と、ドライバーとのやりとりの形は [skills/geas/gui.md](../../skills/geas/gui.md)（英語）にあります。

## 言語

`when` はターゲットの呼び出しで、`then` と `and` がその結果をチェックします。

| ターゲット | 呼び出し | チェックするもの |
|---|---|---|
| `run "<コマンド>"`。`when` のたびにコマンドを起動する | `run("a", "b")` | `stdout`、`stderr`、`exit` |
| `serve "<コマンド>"` と `port <番号>` か `port auto`。主張ごとにサービスを起動する | `get("/path")`、`post("/path", body: "…")` | `status`、`header "<名前>"`、`body`、`body json ".a[0].b"` |
| Chrome で開く `serve` のターゲット、`pixie "<アプリ>"`、`driver "<コマンド>"` | `open`、`click`、`input`、`submit`、`press`、`advance` | `screen` |

比べ方は英語の文として読めるようにしてあり、`is`、`is not`、`is above`、`is below`、`is at least`、`is at most`、`is between 200 and 299`、`contains`、`does not contain`、`matches "\d+ items?"`（値の全体とパターンを照らし合わせる）、`exists`、`does not exist`、画面なら `contains exactly 2 listitem in list "Items"` のように書きます。コマンドは、シェルを通さず geas が語に分けます。空白を含む語は `'…'` で囲みます。

固定は、ターゲットの外に書けばすべてのターゲットに、ターゲットの中に書けばそのターゲットだけに効きます。`env`、`env clean`、`env pass`、`tz`、`locale` のほか、ページか、環境変数で受け取るプログラムなら `clock` と `seed` も固定できます。geas はプロセスの中に入り込んで時計をごまかすことはしないので、できないときはそう言います。

```text
エラー[E011]: E011-clock-without-env.geas:2:1: `calc`、`report` では、`clock` で時刻を固定できません。プログラムの時刻を外から設定する方法はないので、geas はプログラムが読む環境変数で時刻を渡します
     2 | clock "2026-08-29T09:00:00+09:00"
  = その環境変数の名前を、同じ行に書いてください（`clock "2026-08-29T09:00:00+09:00" env "NOW"`）
```

どの診断にも、コードと場所と、そうなる例が付きます。最後まで走れなかった主張なら、上の E035 のように、そこまでの実行が例になります。`geas explain <code>` は、そのコードがいつ出るか、ふつうはどう直すか、そのコードが出る最小の主張のファイルを出します。診断のコードは全部で 34 種類あり、`geas explain --all --lang ja` で全部を日本語で読めます。

`--jobs 4` を付けると、主張を四つまで並列に走らせます。`port auto` のサービスはそれぞれ別のポートで動きます。報告もジャーナルもベースラインも主張の順に書くので、一つずつ走らせたときとバイト単位で同じです。あるターゲットを使う主張どうしが、geas から見えない状態（ファイルやデータベース）を共有しているなら、`serial` を書いて並列に走らないようにします。言語の全体は [skills/geas/language.md](../../skills/geas/language.md)（英語）にあります。

## AI エージェント向け

主張は人が読み、エージェントは最初から geas を使いこなせる、という形にしてあります。`geas skill` が出す手引きは [Agent Skill](https://agentskills.io) の形をしていて、ゲートの走らせ方、出力の読み方、診断ごとの意味、人に任せることが書いてあります。エージェントが書くのはコードだけで、主張は書きません。足りないと思った主張は、人に提案します。手引きはバイナリに入っています。

```console
$ geas skill --install .claude/skills --lang ja
geas のスキルを .claude/skills/geas に書きました（ファイル 7 個）
```

同じフォルダーが [skills/geas](../../skills/geas) にあります。くわしくは [skills/README.md](skills/README.md)（英語）を見てください。

## インストール

geas は [ritsu](https://github.com/i2y/ritsu) の言語の一つで、ritsu のリポジトリから、最近の stable の Rust でビルドします。七つの言語を全部入れ、複数の言語のファイルがあるプロジェクトを `ritsu check` で確かめるなら、次のとおりです（`.geas` のファイルの主張は、`geas check` と同じように実行されます）。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

これで `ritsu geas <コマンド>` が、下のコマンドのどれにもなります（`geas` という名前で `ritsu` を指すリンクでも同じです）。ほかの言語を読まない geas だけを入れるなら、次のとおりです。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked geas
```

依存はありません。geas が起動するのは、主張に要るもの、つまりプロジェクト自身のプログラムと、ページなら Chrome か Chromium、`geas map` なら上の表にある各言語のツールだけです。

## コマンド

```console
$ geas --help --lang ja
geas: エージェントが書いたコードに、人が読んで確かめた主張を守らせる
コーディングエージェント向け: `geas skill` が手引きを出します。`geas skill --install <dir>` でスキルとしてインストールできます。

使い方:
  geas check <spec.geas>...           主張をすべて実行する。すべて成り立てば終了コード 0
  geas snap <spec.geas>...            主張を実行し、結果をすべてベースラインとして残す
  geas drift <spec.geas>...           もう一度実行し、ベースラインから変わったところを示す
  geas map <spec.geas>...             カバレッジを取りながら実行し、主張ごとに通った行を記録する
  geas affected <spec.geas> <diff|->  差分が関わる主張と、変わったコードのうちどの主張も通らないところを示す
  geas explain <code>... | --all      コードの意味と直し方
  geas skill [--install <dir>]        コーディングエージェント向けの手引きを出すか、スキルのフォルダーとして書く

オプション:
  --json            JSON で出力する
  --lang ja         メッセージを日本語で出す（GEAS_LANG=ja でも同じ）
  --jobs N, -j N    主張を一度に N 個まで並列に走らせる（GEAS_JOBS でも同じ）。既定は 1
  --root <dir>      map、affected: 記録のパスの基準にするディレクトリ
  --out <file>      map: 記録を書く先
  --map <file>      affected: 読む記録。変更前と変更後のコードの記録を二つ渡せる
  --install <dir>   skill: スキルのファイルを <dir>/geas に書く
  --force           skill: すでにある <dir>/geas に上書きする
  --help, -h        この説明を出す
  --version         バージョンを出す

終了コード: 0 すべて成り立った、または報告することがない · 1 成り立たない主張や変化、どの主張も通らないコードの変更がある · 2 主張のファイルや引数に誤りがあるか、ファイルを読み書きできない
```

| 環境変数 | 働き |
|---|---|
| `GEAS_LANG` | `ja` でメッセージを日本語にする |
| `GEAS_JOBS` | `--jobs` の既定値 |
| `GEAS_CHROME` | ページを開くのに使う Chrome。指定すると、ほかは探さない |
| `GEAS_LLVM_BIN` | `geas map` で Rust の記録を読む `llvm-profdata` と `llvm-cov` の場所 |

オプションの全部、終了コード、geas が書くファイルと JSON の形は [skills/geas/commands.md](../../skills/geas/commands.md)（英語）にあります。

## 例

| 例 | 見せるもの |
|---|---|
| [calc](examples/calc) | Python のコマンドラインのプログラム。`stdout`、`stderr`、`exit` |
| [greeter](examples/greeter) | 上の HTTP サービス、ドリフトを見るためのリファクタリング、日本語で名前を付けた主張 |
| [tally-node](examples/tally-node) | TypeScript のサービス。ビルドなしで `geas map` が記録を取る |
| [tally-go](examples/tally-go) | 同じサービスの Go 版。`-cover` を付けてビルドする |
| [tally-rust](examples/tally-rust) | Rust のコマンドライン。`-C instrument-coverage` を付けてビルドする |
| [web-greeter](examples/web-greeter) | Chrome で開くページとそのサービス。固定を全部使う |
| [pixie-greeter](examples/pixie-greeter) | pixie のデスクトップアプリ。pixie でビルドして、主張のファイルの横に置く |

どの例にも、何を見せるか、どう走らせるか、出力のどこを見ればよいかを書いた README（英語）があります。

## 確かめ方

`cargo test` は、プログラムを起動しない部分（字句解析と構文解析、パターン、JSON、ハッシュ、行の集合、差分、画面、記録済みの出力を使ったカバレッジの読み取り）の単体テストと、ビルドしたバイナリを走らせるテストを流します。後者では、どのコードも、その最小の主張のファイルを走らせると、そのコードが出ることを確かめます。五つの例には、エージェントが書くような変更を一つずつ用意してあり、四つのランタイムで `map` と `affected` を走らせ、git の差分と `diff -u` の差分の両方で結果を確かめます。ほかに、Chrome でページを開くこと、pixie の greeter を動かすこと、`--jobs 4` が一つずつ走らせたときと同じバイト列を出すこと、テストが起動したプロセスが残らないこと、そしてこのページが説明している流れが、書いてあるとおりに動くことを確かめます。

このページとスキルに載せた出力は、そのテストが golden のファイルとして残したものを、そのまま貼っています。`tests/docs.rs` が、ページとそれを突き合わせます。ここに載せた主張のファイルは geas が読めること、出力は geas が実際に出したものであること、名前を挙げたコマンド、オプション、コード、リンクはどれも実在することを確かめています。

macOS（Apple シリコン）で、ツールを全部そろえて `cargo test` を一度走らせたときは、テスト 236 件（単体テスト 119 件、バイナリを走らせるテスト 117 件）が 79 秒で通り、飛ばしたテストはありませんでした。リリースビルドのバイナリは 2.3 MB です。

ツールが無いテスト（python3、node、go、llvm-tools の入った rustc、Chrome、git、`GEAS_PIXIE_GREETER` で指定するビルド済みの pixie の greeter）は、`SKIP:` の行を出して通ります。走らなかったものは次で分かります。

```console
$ cargo test -- --nocapture 2>&1 | grep '^SKIP'
```

golden のファイルは `GEAS_BLESS=1 cargo test` で書き直します。

## いまの状態

まだ始めたばかりです。まだやっていないことは次のとおりです。

- Windows。geas はサービスに SIGTERM を送って終わらせ、そのときにカバレッジを書かせますが、Windows のプロセスには SIGTERM がありません。
- macOS のアクセシビリティ API。人がシステム設定で許可を与える必要があり、テストでは確かめられません（許可を与えられる環境なら、ドライバーとして書けます）。
- pixie のアプリやページのスクリプトがどの行を通ったかの記録。
- ネイティブの `<select>` で選択肢を選ぶ操作。
- FFI でライブラリを直接呼ぶ主張。いまは、ライブラリを包む小さなコマンドかサービスを通して主張します。

主張どうしが互いに依存する書き方は、わざと入れていません。主張はどれも、自分の `when` で必要な状態を作ります。だから一つだけでも並列にでも走らせられ、読む人にも前提が見えます。geas は仮の名前で、名前はまだ決めていません。設計と決めたことは [DESIGN.md](DESIGN.md)（英語）に、作業の順序は [PLAN.md](PLAN.md)（英語）にあります。

## ライセンス

[Apache License, Version 2.0](LICENSE-APACHE) と [MIT License](LICENSE-MIT) のどちらかを選んで使えます。
