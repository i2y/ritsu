# 生成する物とサーバー

*申請を受けるサーバーを作るエンジニアと、動かすエンジニアのためのページです。*

ritsu が書くのは申請のコードまでです。そのコードは純粋な関数のライブラリで、時計も保存先もネットワークも持ちません。チケット、イベント、組織、時刻を渡すと、次のチケットと、サーバーがすべきことを返します。チケットを保存すること、期限の時計を動かすこと、知らせを送ること、画面を出すことは、サーバーの受け持ちです。ritsu がサーバーのために書くのは契約です。関数と、それを説明する文書と、サーバーを試すベクタです。この契約で動くサーバーには、Go のフレームワーク [ringi-server](https://github.com/i2y/ringi-server) があり、[数個のコマンド](#ringi-server-で動かす)で始められます。

## 言語ごとのパッケージに入るもの

`ritsu gen --target typescript`（`python`、`go`、`rust` も同じ）は、プロジェクトの申請の種類ごとに、下ろした先の規則と日付のコード、それをチケットにつなぐモジュールを書きます。チケットの機械そのものは、プロジェクトに一つ書きます。その隣に、申請の種類の一覧（`ticket_kinds.ts`、`ticket_kinds.py`、`ticketkinds/ticketkinds.go`、`ticket_kinds.rs`）を置きます。

| 入口 | 受け取るもの | 返すもの |
|---|---|---|
| `start` | 出された申請（チケットの ID、申請者、フォーム）、組織、時刻 | 新しいチケットとすべきこと。受け付けないときはその理由 |
| `next` | それまでのチケット、イベント、組織、時刻 | 同上 |
| `worked` | 途中まで入れたフォーム | いま表示し必須にするフィールドと、計算で決まるフィールドの値 |

どの入口も JSON を受け取って JSON を返し、その形はどの種類の申請でも同じです。そのため、サーバーはこの一覧から、種類ごとのコードを書かずにどの種類でも動かせます。すべきことはリストで返り、どれもほかと重ならないキーを持ちます。だれそれに知らせる、この時計を始める・止める、この帳簿でこの額を仮押さえする・確定する・外す、このワークフローを始める、といったものです。

## `<out>/requests/` に入るもの

`ritsu gen --target requests` は、どの言語のコードでもない形で、申請をサーバー、画面、エージェントに説明するファイルを、プロジェクトに一組書きます。画面は、描くのに使うコンポーネントのカタログを URL で指します。その URL は `--catalog` で渡します。

```console
$ cd crates/ringi/examples
$ ritsu gen leave.ja.ringi --target requests --catalog https://example.com/ritsu/catalog.json --out generated --lang ja
生成しました: generated/requests/types/leave_ja.json
生成しました: generated/requests/openapi.json
生成しました: generated/requests/mcp/tools.json
生成しました: generated/requests/organization/schema.json
生成しました: generated/requests/organization/columns.md
生成しました: generated/requests/organization/find.jsonl
生成しました: generated/requests/vectors/leave_ja.jsonl
生成しました: generated/requests/vectors/worked/leave_ja.jsonl
生成しました: generated/requests/a2ui/leave_ja/form.en.json
生成しました: generated/requests/a2ui/leave_ja/form.ja.json
生成しました: generated/requests/a2ui/leave_ja/card.en.json
生成しました: generated/requests/a2ui/leave_ja/card.ja.json
生成しました: generated/requests/a2ui/leave_ja/list.en.json
生成しました: generated/requests/a2ui/leave_ja/list.ja.json
生成しました: generated/requests/a2ui/leave_ja/mine.en.json
生成しました: generated/requests/a2ui/leave_ja/mine.ja.json
生成しました: generated/requests/a2ui/leave_ja/history.en.json
生成しました: generated/requests/a2ui/leave_ja/history.ja.json
生成しました: generated/requests/a2ui/leave_ja/offered.en.json
生成しました: generated/requests/a2ui/leave_ja/offered.ja.json
生成しました: generated/requests/a2ui/catalog.json
生成しました: generated/requests/a2ui/functions.js
```

| ファイル | 中身 |
|---|---|
| `types/<名前>.json` | 申請の種類ごとの、フォーム、ステップと、場合ごとに答える人と期限、人ができること、使うワークフロー |
| `openapi.json` | サーバーの API（OpenAPI 3.1）。種類ごとの申請の提出、だれかを待っている申請の一覧、承認、差し戻し、取り下げなど |
| `mcp/tools.json` | 同じ操作を、エージェントのツールとして書いたもの。アシスタントが渡された書類からフォームを埋めたり、使う人を待っている申請を答えたりできます |
| `a2ui/<名前>/` | 種類ごとの六つの画面（フォーム、承認者が答えるカード、待っている申請の一覧、申請者自身の一覧、履歴、いつから申請できるかのお知らせ）を、英語と日本語の A2UI 1.0 のテンプレートで。カタログとその関数（`a2ui/catalog.json`、`a2ui/functions.js`）も入ります |
| `organization/schema.json`、`organization/columns.md` | 組織のデータの形を、JSON と CSV のファイルの列で |
| `vectors/<名前>.jsonl`、`vectors/worked/<名前>.jsonl`、`organization/find.jsonl` | ベクタ |

フォームの検査と計算は、生成するコード、画面の関数、参照インタプリタの三か所で動きます。三つとも、下ろした同じ規則から書いたものです。

## 一つのモジュールと一つの zip

サーバーは、申請のコードを自分で組まずに動かすこともできます。`--extra wasm` は、Rust のパッケージを WebAssembly の一つのモジュール `requests/kinds.wasm` に組みます。全部の申請の種類が一つのモジュールに入り、呼び方は `requests/wasm.json` に書きます。`--extra bundle` は、`requests/` と申請の種類のもとのファイルを一つの zip にまとめます。サーバーは、申請の種類の一つの版を一つのファイルで受け取れます。

```console
$ cd crates/ringi/examples
$ ritsu gen leave.ja.ringi --root . --target requests --catalog https://example.com/ritsu/catalog.json --out generated --lang ja --extra wasm --extra bundle
生成しました: generated/requests/types/leave_ja.json
…
生成しました: generated/requests/wasm.json
生成しました: generated/requests/bundle.json
生成しました: generated/requests/kinds.wasm
生成しました: generated/requests/source/leave.ja.ringi
生成しました: generated/requests/source/shared/company.gate
生成しました: generated/requests/source/shared/company.ringi
生成しました: generated/requests/source/shared/leave.book
生成しました: generated/requests/source/shared/manager_approval.ringi
生成しました: generated/requests/source/shared/office.cal
生成しました: generated/requests.zip
```

モジュールは何も読み込みません。サーバーはこれを読み込み、`wasm.json` の SHA-256 と合うかを確かめ、申請の種類ごとの `start`、`next`、`worked` を、パッケージの入口と同じ JSON で呼びます。例のどのベクタにも、英語でも日本語でも、参照インタプリタと同じ答えを返します。組むには、Rust と `wasm32-unknown-unknown` の標準ライブラリ（`rustup target add wasm32-unknown-unknown`）が要ります。ネットワークは使いません。`--check` は組まないので、Rust を入れていない所でも回せます。

`--diff-from` に前のバンドルを渡すと、zip に、変更で何が変わるかも入ります。ここでは、経路を `route 標準(long: 日数 > 3)` に変えました。

```console
$ cd crates/ringi/examples
$ ritsu gen leave.ja.ringi --root . --target requests --catalog https://example.com/ritsu/catalog.json --out next --lang ja --extra wasm --extra bundle --diff-from generated/requests.zip
生成しました: next/requests/types/leave_ja.json
…
生成しました: next/requests/wasm.json
生成しました: next/requests/bundle.json
生成しました: next/requests/diff/leave_ja.en.md
生成しました: next/requests/diff/leave_ja.ja.md
生成しました: next/requests/kinds.wasm
生成しました: next/requests/source/leave.ja.ringi
生成しました: next/requests/source/shared/company.gate
生成しました: next/requests/source/shared/company.ringi
生成しました: next/requests/source/shared/leave.book
生成しました: next/requests/source/shared/manager_approval.ringi
生成しました: next/requests/source/shared/office.cal
生成しました: next/requests.zip
```

`diff/leave_ja.ja.md` と `diff/leave_ja.en.md` は `ringi diff` のページです。営業日が 4 日か 5 日の申請にステップ「部長」が来るようになること、それで組織の版ごとにどの申請者の承認者が変わるかを言います。`bundle.json` には、`source/` の下のファイルとそのダイジェスト、比べた種類と違うかどうかが入ります。

## ベクタ

`ritsu ringi vectors` は、申請の種類ごとに、シナリオのすべての行と、ファイルから作り出した場合のすべてを、一行ずつ JSON で書きます。作り出す場合は、定足数と範囲のそれぞれのぎりぎりの値、差し戻しの上限、組織の全員を申請者にした場合、代理、組織の二つの版にまたがるチケットなどです。一行には、それまでのチケット、イベント、組織、時刻、参照インタプリタの答えが入ります。上の `vectors/` のファイルがこれです。サーバーは、各行を自分のコードに渡して答えを比べることで、契約どおりかを確かめます。

ritsu が四つの言語で書くコードは、例のどのベクタにも、受け付けない場合を含めて、参照インタプリタと同じ答えを返します。

## サーバーがすること

サーバーは、チケットとその履歴を保存し、イベント（人の操作、時計、ワークフローの答え）を受け取って、チケットごとに一つずつ `next` に渡します。渡す前に、その人がしてよいことかをゲートに尋ねます。渡した後は、チケットを保存してから、すべきことをキーで重ねずに一度ずつ行います。`next` には、その時点で有効な組織の版から、チケットに要る部分を切り出して渡します。期限と催促の時計もサーバーが動かします。

利用者のサインインの方法、チケットの保存先、知らせの送り方、添付の置き場所、組織のデータの出どころ、ワークフローの動かし方は、会社ごとに違うので、サーバーが選びます。

## ringi-server で動かす

[ringi-server](https://github.com/i2y/ringi-server) は、ここまでのことをする Go のフレームワークです。チケットと添付を PostgreSQL に保管し、OpenID Connect のプロバイダーで人をログインさせ、時計を動かします。どの種類の申請も同じコードで受け、人がブラウザで使うページ（JavaScript があってもなくても使えます）、`openapi.json` の API、A2UI の画面、`mcp/tools.json` のツールを出します。flow は、別のモジュールが Temporal で動かします。プロジェクトが渡すのは、会社ごとに違うものです。ゲート、知らせ、組織のデータ、帳簿、flow です。

`ritsu ringi init` は、その上のプロジェクトの最初のファイルを書きます。申請の種類を一つと、それが読む組織、カレンダー、ゲート、そこから生成したコード、サーバー、その試験、README です。動かすには Go 1.26 以降を使います。

```sh
ritsu ringi init requests --module example.com/requests --lang ja
cd requests
go mod tidy
go run ./cmd/server -dev-login
```

`http://127.0.0.1:8080/pages/` を開き、だれとして使うかを選びます。`-dev-login` は本人を確かめないので、サーバーは自分のコンピューターにだけ答えます。自分の申請の種類は `project/` に書き、`ritsu check project` と `go generate ./...` を実行します。

[ringi-server のチュートリアル](https://i2y.github.io/ringi-server/ja/start/)では、この手順を一つずつ進めてから、申請の種類を書き換え、通知をメールで送ります。自分の会社のデータを入れる、会社のログインをつなぐ、社員に使ってもらうサーバーを動かす、といった手引きは、[ringi-server のサイト](https://i2y.github.io/ringi-server/ja/)にあります。

## サーバーなしで見る

`ritsu ringi run` は、シナリオを参照インタプリタで動かします（[ringi](index.md) に例があります）。`ritsu ringi api` は、申請の種類が宣言していること（型と範囲を添えたフォーム、ステップ、チケットの操作、使える日）を JSON で表示します。
