# chobo

**勘定に下限と上限を書く。どの振替も、一度の書き込みの中でそれを守る。**

在庫、お金、ポイント、予約の枠のように、数で持っていて勘定から勘定へ動かすものを書く、小さな言語です。帳簿（`.book`）には、勘定と、勘定ごとの下限や上限と、勘定のあいだで動かす振替の種類を書きます。振替には、同じ呼び出しを一度しか通さないためのキーと、確定か取消を待つ仮押さえも書けます。chobo は帳簿を検査し、参照インタプリタで動かし、PostgreSQL か TigerBeetle 向けにビルドします。呼び出すクライアントは TypeScript、Python、Go で出します。

帳簿に書ける条件は、一つの勘定の下限と上限だけです。これはわざとそうしています。一つの勘定の境界なら、その勘定を書き換える書き込みの中で確かめられ、あいだに別の読み取りが入りません。PostgreSQL では勘定の行をロックする関数が、TigerBeetle では勘定のフラグと、全部通るか一つも通らないかのどちらかになる振替のチェーンが、それを受け持ちます。二つの勘定にまたがる条件は、こうはいきません。同時に来た二つの呼び出しが、どちらも、もう一方が書き込む前の残高を読めてしまうからです。そこで、またがる条件は、勘定を一つ足して、その勘定の境界として書きます。「返金は売上を超えない」なら、注文ごとに「返金できる残り」の勘定を立て、売上で増やし、返金で減らし、0 を下回らないようにします。

```book
book 返金 v1
description "注文ごとに、返金できる残りを勘定にする。売上で増え、返金で減り、0 より下にはならない。返金は承認を待つあいだ押さえる"

unit 円

account 返金できる残り(注文: string) : 円
  description "その注文でまだ返金できる額"
  at least 0 refused as 返金超過
account 売上 : 円 outside
account 返金済み : 円 outside

transfer 売上計上(注文: string, 額: 円)
  key 注文
  move 額 from 売上 to 返金できる残り(注文)

transfer 返金(申請: string, 注文: string, 額: 円)
  description "承認を待つあいだ押さえる。承認されたら確定し、却下されたら取り消す"
  key 申請
  pending expires after 7 days
  move 額 from 返金できる残り(注文) to 返金済み
```

キーワードは英語で、勘定や振替の名前は日本語でも書けます。名前は帳簿（ちょうぼ）から取りました。

## 検査が言うこと

動かす前に、`chobo check` が、振替の種類と操作ごとに、拒否されうる理由を並べます。境界に付けた理由のほかに、chobo が決めている理由もあります。同じキーで中身の違う二度目、取り消したあとの確定、期限が切れたあとの確定などです。

```console
$ chobo check examples/refunds/refunds.ja.book --lang ja
examples/refunds/refunds.ja.book: ok
  売上計上.do  拒否されうる理由: key_conflict
  返金.hold    拒否されうる理由: 返金超過（返金できる残り(注文) at least 0）、key_conflict、already_refused
  返金.post    拒否されうる理由: key_conflict、already_voided、expired、over_hold、no_such_hold
  返金.void    拒否されうる理由: already_posted、expired、no_such_hold
  キー: 売上計上 は、注文 ごとに一度だけ動きます。キーが同じで 額 が違う二度目の呼び出しは、key_conflict で拒否されます。
  キー: 返金 は、申請 ごとに一度だけ動きます。キーが同じで 注文、額 のどれかが違う二度目の呼び出しは、key_conflict で拒否されます。仮押さえが終わったあとに同じキーでもう一度押さえると、done_before が返り、何も押さえません。
```

ここに載るのは、そこに至る操作の列を検査が見つけ、参照インタプリタで実際に流して確かめた理由だけです。操作の列そのものは `--format json` と `chobo api` が出します。ワークフローのタスクが、返しうるエラーを宣言するときに使えます。診断にも、そうなる例が付きます。次の帳簿は、マーケットプレイスの売上で、店の売上から手数料を取る移動を、代金を入れる移動より先に書いています（[tests/fixtures/順序.book](tests/fixtures/順序.book)）。

```text
警告[W103]: 順序.book:15:3: 1 つ目の移動が 店の売上(店) から取るのは、2 つ目の移動が 店の売上(店) へ入れるより前です。そのとき 店の売上(店) が足りないと、二つの移動を合わせれば足りる場合でも 売上不足 で拒否されます
    15 |   move 手数料 from 店の売上(店) to 手数料収入
  そうなる例:
       1  売上.do(注文: 注文-1, 店: 店-2, 代金: 1, 手数料: 1)  売上不足 で拒否される（1 つ目の移動が 店の売上(店-2) から 1 を取ろうとしたときの残高は、確定 0、出ていく仮押さえ 0）
  ヒント: 店の売上(店) へ入れる移動を先に書いてください
```

診断のコードは全部で 30 種類あり、[docs/codes.ja.md](docs/codes.ja.md) に `chobo explain --all` の出力をそのまま置いてあります。

## 二つの返金が同時に来る

30000 円の売上に、20000 円の返金が二つ同時に来たとします（[examples/refunds/refunds.ja.more.json](examples/refunds/refunds.ja.more.json)）。参照インタプリタは、二つを処理する順序をすべて試し、とりうる結果を全部出します。

```console
$ chobo run examples/refunds/refunds.ja.book --scenario examples/refunds/refunds.ja.more.json --lang ja
examples/refunds/refunds.ja.book (more: 30000 円の売上に 20000 円の返金が二つ同時に来て、そのあと 10000 円を返金する): 3 ステップ、とりうる結果は 2 通り
結果 1:
    1  売上計上.do(注文: A-100, 額: 30000)                           通る
    2  together
         呼び出し元 1: 返金.hold(申請: R-1, 注文: A-100, 額: 20000)  通る
         呼び出し元 2: 返金.hold(申請: R-2, 注文: A-100, 額: 20000)  返金超過 で拒否される
    3  返金.hold(申請: R-3, 注文: A-100, 額: 10000)                  通る
  勘定:
    返金できる残り(A-100)  確定 30000、出ていく仮押さえ 30000、入ってくる仮押さえ 0
    売上                   確定 -30000、出ていく仮押さえ 0、入ってくる仮押さえ 0
    返金済み               確定 0、出ていく仮押さえ 0、入ってくる仮押さえ 30000
  仮押さえ:
    返金(R-1)  押さえ中
    返金(R-3)  押さえ中
結果 2:
…
```

先に処理された方が押さえられ、もう一方は拒否されます。残りの 10000 円の返金は、どちらの結果でも通ります。データベースの答えはこのどちらかでなければならず、テストがそれを確かめています。

## 税込と税抜

通貨の名前の単位はお金の単位で、最後に税込か税抜かを書けます。

```book
unit 円 incl_tax
```

帳簿の中では何も変わりません。額の数え方も、境界も、生成するコードも、書かないときと同じです。区別を書くのは、同じ処理系の規則やワークフローから額を受け取るところのためです。帳簿は単位を規則と同じ単位として渡すので、税込の単位は規則の `money[円, incl_tax]` と同じ単位になります。税込の単位は税込の額を、税抜の単位は税抜の額を、区別を書かない単位は区別の無い額を受け取るものとし、一つの勘定に税込の額と税抜の額が混ざらないようにします。お金でない単位には、どちらも書けません（E014）。`個` や `席` は名前だけの数え方で、互いに同じ単位ではなく、`kg` は規則と同じキログラムです。

## 出力先

```console
$ chobo build examples/refunds/refunds.ja.book --target postgres --out db
```

| ターゲット | `build` が書くもの |
|---|---|
| `postgres` | スキーマ、テーブルと制約、操作ごとの関数（SQL） |
| `postgres-typescript`、`postgres-python`、`postgres-go` | その関数を呼ぶクライアント |
| `tigerbeetle-typescript`、`tigerbeetle-python`、`tigerbeetle-go` | 操作ごとに、振替のチェーンを一つ TigerBeetle に送るクライアント |

二つのデータベースのクライアントは、名前も引数も結果も同じです。拒否されたことは例外ではなく、`{ result: "refused", reason: "返金超過" }` という結果で返ります。同じ呼び出しの二度目は `done_before` を返すので、リトライで二重に動くことはありません。PostgreSQL では、関数が呼ぶ側のトランザクションの中で動くので、注文の行と在庫の仮押さえを一緒にコミットできます。TigerBeetle は利用者のコードを中で動かせないので、クライアントがチェーンを組み立てます。下限 0 は勘定のフラグで守り、ほかの境界は、chobo が同じチェーンに足す振替で守ります。出力先ごとに何を書き、三つの言語からどう呼ぶかは [docs/targets.md](docs/targets.md)（英語）にあります。

## 経理や運用の人に見せる

`chobo doc` は、帳簿を、経理や運用の人が読むページにします。載るのは、勘定と境界、勘定のあいだの流れの図、振替ごとの移動とキーと操作ごとの拒否されうる理由、仮押さえのライフサイクル、そしてシナリオごとの、ステップのたびの残高です。GitHub が描く Mermaid の図を入れた Markdown か、一つの HTML で出します。HTML では図を chobo が自分で描き、シナリオを一ステップずつ進めて見られます。`--lang ja` で日本語になります。例の横には、それぞれのページを置いてあります。たとえば [返金](examples/refunds/doc.ja.md) と、その英語の版の [refunds](examples/refunds/doc.md) です。

## AI エージェント向け

[skills/chobo](../../skills/chobo) は、chobo を使うための [Agent Skill](https://agentskills.io) です。最初の下書きからビルドまでの手順、一ページにまとめた言語、人に聞くこと（境界の数と理由の名前、キー、仮押さえの有効期限、どの勘定を外の勘定にするか）、診断ごとの直し方が入っています。`~/.claude/skills/` か、プロジェクトの `.claude/skills/` にコピーして使います。くわしくは [skills/README.md](skills/README.md) にあります。

## インストール

chobo は [ritsu](https://github.com/i2y/ritsu) の言語の一つで、ritsu のリポジトリから、最近の stable の Rust でビルドします。八つの言語を全部入れ、複数の言語のファイルがあるプロジェクトを `ritsu check` で確かめるなら、次のとおりです。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

これで `ritsu chobo <コマンド>` が、下のコマンドのどれにもなります（`chobo` という名前で `ritsu` を指すリンクでも同じです）。ほかの言語を読まない chobo だけを入れるなら、次のとおりです。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked chobo
```

依存は serde_json だけです。

## コマンド

```
chobo check <file.book>... [--format json] [--diff-base <rev>]
chobo run <file.book> --scenario <file.json> [--show postgres|tigerbeetle] [--format json]
chobo scenarios <file.book> [--out <dir>]
chobo build <file.book> --target <target> [--out <dir>]
chobo doc <file.book> [--format html] [--out <dir>]
chobo api <file.book>
chobo explain <code> | --all [--format markdown]
```

`--lang ja` を付けると、メッセージが日本語になります。`--diff-base <rev>` は、git のそのリビジョンの帳簿と比べ、データベースにすでにある残高や仮押さえと合わなくなる変更をエラーにします。`chobo api` は、帳簿の呼び方、操作ごとの拒否されうる理由、仮押さえのライフサイクルをステートマシンとして、ほかのツール向けの JSON で出します（[docs/formats.md](docs/formats.md)、英語）。

## 例

[examples/](examples/) に四つあり、どれも英語の版の横に日本語の版（`inventory.ja.book`）を置いてあります。注文のために在庫を 30 分押さえ、出荷で確定し、キャンセルで取り消す在庫の引当。返品の期間が過ぎるまで押さえて付与し、支払いが通るまで押さえて使い、期間の終わりに失効するポイント。売上を超えない返金。そして、買い手が払った額を店の取り分と手数料の二つの移動に分け、全部通るか一つも通らないかにするマーケットプレイスです。言語の全体は [docs/reference.md](docs/reference.md)（英語）にあります。

## 確かめ方

帳簿の意味を決めるのは参照インタプリタです。テストは、例とテスト用の帳簿の全部からシナリオを作り（手で書いたものを含めて 468 本、そのうち 38 本は呼び出し元が同時に来るもの）、どれも七つの出力先で流します。SQL そのもの（psql から関数を呼ぶ）、PostgreSQL を呼ぶ三つの言語のクライアント、TigerBeetle を呼ぶ三つの言語のクライアントです。操作ごとの結果、終わりの残高、仮押さえの状態が、参照インタプリタと同じ（同時に来るシナリオなら、とりうる結果のどれか）でなければなりません。三つの言語のクライアントが送ったものは、`chobo run --show` と一字ずつ比べるので、どの言語も同じ ID を作ることも確かめられます。PostgreSQL では、四つのセッションから同じ勘定に逆の向きの移し替えを流し、デッドロックが起きないことも見ます。生成した TypeScript は `tsc --strict`、Python は `py_compile`、Go は `gofmt` と `go vet` を通します。`chobo doc` のページはテストが期待する出力として固定し、Mermaid の図は Mermaid 11 と 12 で描けることを、HTML のページは Chrome で、各ステップのあとに参照インタプリタと同じ残高が出ることを確かめます。

```console
$ cargo test
```

PostgreSQL（PATH か `CHOBO_PG_BIN`）、TigerBeetle（`tools/tigerbeetle/fetch.sh` か `CHOBO_TIGERBEETLE`）、Node.js と Python と `tools/runner` に書いた依存、Go、Chrome（`CHOBO_CHROME`）、Mermaid（`npm ci --prefix tools/mermaid`）のうち、見つかったものを使います。見つからないものは、それぞれ `SKIP:` の行を出して飛ばします。

## いまの状態

まだ始めたばかりです。まだやっていないことは次のとおりです。

- 残高のある勘定の種類について、境界を変えること。いまは新しい勘定の種類を作り、残高を移す振替を書きます。
- 規則から受け取る額が、入れる先の単位と同じ税込か税抜かを確かめること（単位は、もう区別を持っています）。
- dandori の側。帳簿の振替をタスクとして呼び、仮押さえを案件として追うワークフロー（`chobo api` は、そのためのステートマシンを出しています）。
- PostgreSQL と TigerBeetle のほかのデータベース。
- TigerBeetle を本番の構成（レプリカ六つ）で動かすこと（テストは `--development` で立てたレプリカ一つで流します）と、キーから作る ID でどれだけ遅くなるかの測定。
- 勘定が受けた記録や振替を読む操作。
- TigerBeetle の答えが参照インタプリタと違う二つの場合をそろえること（[docs/targets.md](docs/targets.md) に書いてあります）。

設計と、決めたこと、残したことは [DESIGN.md](DESIGN.md) にあります。

## ライセンス

[Apache License, Version 2.0](LICENSE-APACHE) と [MIT License](LICENSE-MIT) のどちらかを選んで使えます。
