# インストール

chobo は [ritsu](https://github.com/i2y/ritsu) の言語の一つで、ritsu の一部として配っています。ritsu のリリースに入っているのは、`ritsu` という一つのバイナリと、言語ごとにそれを指すリンクです。`chobo` という名前のリンクから呼ぶと、chobo として動きます。`ritsu chobo <コマンド>` も同じコマンドです。chobo はほかの言語のファイルを読まないので、chobo だけをビルドすることもできます。

インストールする前に試すなら、ritsu のサイトの[ブラウザで試す](../../ja/playground/#project=chobo/refunds.ja)ページを開いてください。ページの中で ritsu が動き、例の帳簿を書き換えながら、検査やビルドや人が読むページを試せます。

## ritsu のリリース

リリースのたびに、macOS（arm64、x64）と Linux（x64、arm64）の `ritsu` を、ritsu の[リリースのページ](https://github.com/i2y/ritsu/releases)に置いています。ファイルごとの SHA-256 は `SHA256SUMS` に載せています。アーカイブには `ritsu` と、`chobo` を含むリンクが入っているので、PATH にあるディレクトリに展開すれば、インストールはそれで終わりです。Homebrew なら `brew install i2y/tap/ritsu` です。Debian と Fedora では、リリースの `.deb` と `.rpm` が `/usr/bin/ritsu` を同じリンクと一緒に入れます。GitHub Actions では、`uses: i2y/ritsu@v<バージョン>` がリリースをランナーの PATH に置きます。それぞれの手順は [rulec のインストールのページ](../../rulec/ja/install/)にあります。コマンドは ritsu のどの言語でも同じです。

リリースには npm のパッケージ `@i2y/ritsu` も付いています。WebAssembly 向けに組んだ ritsu で、ネイティブのバイナリが無くても Node で動きます。このパッケージの `chobo` コマンドは、git を走らせる `check --diff-base` のほかは、ネイティブのバイナリと同じものを出力します（[Node で使う](../../ja/node/)）。

## ソースから

最近の stable の Rust で、ritsu のリポジトリからビルドします。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

これで `ritsu` と全部の言語が入ります。このサイトに載せたコマンドは、どれも `ritsu chobo <コマンド>` で走ります。`chobo` という名前で `ritsu` を指すリンクを置いても同じです。chobo だけを入れるなら、次のとおりです。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked chobo
```

chobo が外から取ってくる依存は serde_json だけです。

## 動くか確かめる

例は、リポジトリの `crates/chobo/examples` にあります。リポジトリをクローンし（`git clone https://github.com/i2y/ritsu`）、`crates/chobo` で、例を一つ検査してビルドしてみます。

```console
$ chobo check examples/inventory/inventory.ja.book --lang ja
examples/inventory/inventory.ja.book: ok
…
$ chobo build examples/inventory/inventory.ja.book --target postgres --out db
```

`--lang ja`（または環境変数 `CHOBO_LANG=ja`、`RITSU_LANG=ja`）を付けると、メッセージが日本語になります。JSON のキーは英語のままです。

## AI エージェント向けのスキル

ritsu のリポジトリの根にある `skills/chobo` は、Claude Code のような AI コーディングエージェント向けの [Agent Skill](https://agentskills.io) です。下書きからビルドまでの手順、一ページにまとめた言語、人に聞くこと、診断ごとの直し方が入っています。このサイトのリファレンスにある四つのページも、一緒に入っています。`ritsu` のバイナリが、ritsu とほかの言語のスキルと一緒に持っています。

```console
$ ritsu skills install chobo                                # 一つのプロジェクトの .claude/skills/ に入れる
$ ritsu skills install chobo --user                         # このマシンのすべてのプロジェクトで使う
$ ritsu skills install chobo --dir path/to/skills           # ほかのエージェントがスキルを読む場所に入れる
```

Claude Code なら、プラグイン `ritsu` に十のスキルが入っています。`/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json` を実行してから、`/plugin install ritsu@ritsu` を実行します。スキルの中身は[エージェント向け](agents.md)に書いています。

## 生成したコードを動かすのに要るもの

ビルドが書くコードは、それを使うプログラムが、データベースのクライアントを渡して呼びます。

| ターゲット | 動かすのに要るもの |
|---|---|
| `postgres` | PostgreSQL。SQL がスキーマとテーブルと操作ごとの関数を作る。何度流してもよい |
| `postgres-typescript` | `pg` の `Client`、`Pool`、`PoolClient` のどれか |
| `postgres-python` | psycopg 3 か psycopg2 の DB-API の接続 |
| `postgres-go` | pgx の `*pgx.Conn`、`*pgxpool.Pool`、`pgx.Tx` のどれか |
| `postgres-rust` | sqlx の `PgPool`、または `tokio::sync::Mutex` に入れた sqlx の接続かトランザクション。クレートは sqlx 0.9.0 と tokio で、モジュールの頭に書いてある |
| `tigerbeetle-typescript` | `tigerbeetle-node` 0.17.9 のクライアント |
| `tigerbeetle-python` | `tigerbeetle` 0.17.9 の `ClientSync` |
| `tigerbeetle-go` | `tigerbeetle-go` v0.17.9 のクライアント（cgo を使う） |
| `tigerbeetle-rust` | クレート `tigerbeetle` 0.17.9 の `Client` |
| `mysql` | MySQL 8.4 以降、TiDB 8.5 以降、Vitess のどれか。SQL が、流したデータベースに帳簿の三つのテーブルを作る。何度流してもよい。TiDB では `tidb_enable_check_constraint` を ON にして流す |
| `mysql-typescript` | `mysql2`（`mysql2/promise`）のプールか接続 |
| `mysql-python` | PyMySQL か mysqlclient の DB-API の接続 |
| `mysql-go` | database/sql の `*sql.DB`、`*sql.Conn`、`*sql.Tx` のどれか（ドライバは `go-sql-driver/mysql`） |
| `mysql-rust` | sqlx の `MySqlPool`、または `tokio::sync::Mutex` に入れた sqlx の接続かトランザクション。クレートは sqlx 0.9.0 と tokio |

テストは、PostgreSQL 18.0、TigerBeetle 0.17.9、MySQL 8.4.11、TiDB 8.5.8、Vitess 24.0.5 の上でクライアントを動かしています。ドライバのバージョンは[出力先](targets.md#the-clients-own-needs)（英語）にあります。ターゲットごとに何を書くかは[データベース向けのビルド](databases.md)に、三つの言語からの呼び方は[出力先](targets.md)（英語）にあります。
