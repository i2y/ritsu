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
| `types/<名前>.json` | 申請の種類ごとの、フォーム、ステップと答える人、人ができること、使うワークフロー |
| `openapi.json` | サーバーの API（OpenAPI 3.1）。種類ごとの申請の提出、だれかを待っている申請の一覧、承認、差し戻し、取り下げなど |
| `mcp/tools.json` | 同じ操作を、エージェントのツールとして書いたもの。アシスタントが渡された書類からフォームを埋めたり、使う人を待っている申請を答えたりできます |
| `a2ui/<名前>/` | 種類ごとの六つの画面（フォーム、承認者が答えるカード、待っている申請の一覧、申請者自身の一覧、履歴、いつから申請できるかのお知らせ）を、英語と日本語の A2UI 1.0 のテンプレートで。カタログとその関数（`a2ui/catalog.json`、`a2ui/functions.js`）も入ります |
| `organization/schema.json`、`organization/columns.md` | 組織のデータの形を、JSON と CSV のファイルの列で |
| `vectors/<名前>.jsonl`、`vectors/worked/<名前>.jsonl`、`organization/find.jsonl` | ベクタ |

フォームの検査と計算は、生成するコード、画面の関数、参照インタプリタの三か所で動きます。三つとも、下ろした同じ規則から書いたものです。

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

ページの様子と、プロジェクトがサーバーに渡すものは [ringi-server の README](https://github.com/i2y/ringi-server/blob/main/README.ja.md) に、設定、バックアップ、版の上げ方は[運用の手引き](https://github.com/i2y/ringi-server/blob/main/docs/operations.ja.md)にあります。

## サーバーなしで見る

`ritsu ringi run` は、シナリオを参照インタプリタで動かします（[ringi](index.md) に例があります）。`ritsu ringi api` は、申請の種類が宣言していること（型と範囲を添えたフォーム、ステップ、チケットの操作、使える日）を JSON で表示します。
