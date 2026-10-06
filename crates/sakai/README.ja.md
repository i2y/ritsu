# sakai

コンテキストマップのうち、成果物と突き合わせて確かめられる部分だけを書く小さな言語です。
どの成果物がどの境界づけられたコンテキストに属するか、関係が境界を越える参照をどこまで許すか、腐敗防止層が越えてくる値と語をどう読み替えるか、を書きます。
sakai は [ritsu](../../README.ja.md) の七つの言語の一つで、ほかの言語の成果物（規則、ワークフロー、カレンダー、帳簿、`.proto`）を ritsu を通して読みます。サービスが OpenAPI と AsyncAPI の文書に書いた契約も読みます。

コンテキストマップはたいてい図で描かれ、描いた翌週にはコードと合わなくなります。
sakai では、地図を成果物の隣に、コンテキストごとに一つのファイルとして置きます。ファイルはそのコンテキストのチームが持ちます。そのうえで、どの成果物もこの地図と突き合わせます。

```ctx
context 配送(delivery) v1
description "出荷の日を決め、運び方を選び、荷物を届ける手配をする"
owner "配送チーム"

owns
  dir "../delivery", "../proto/shop/delivery"
…
upstream 在庫 anticorruption layer
  through warehouse.v1
  layer dir "../py/delivery/acl/inventory", "../ts/delivery/acl/inventory"
  layer dir "../java/src/main/java/delivery/acl/inventory", "../go/delivery/acl/inventory"
  enum PackingStatus -> 出荷の可否
    PACKING_STATUS_WAITING -> 待つ
    PACKING_STATUS_PACKED  -> 出荷する
    PACKING_STATUS_SHORT   -> refuse "欠品のある箱は出荷しない。受注に戻す"

downstream 請求 supplier

shared kernel with 請求
  koyomi "../calendars/東京の営業日.cal"
```

キーワードは英語で、コンテキストや語や下流の値の名前は日本語で書けます。コンテキストには ASCII の別名（`delivery`）を必ず添えます。別名は、CML の名前と、import の検査の設定の名前になります。

```console
$ sakai check examples/shop.ja/通販.ctx --lang ja
examples/shop.ja/通販.ctx: ok — コンテキスト 5、関係 7。成果物 79 件は、どれも一つのコンテキストに属する。境界を越える参照 9 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 5）
```

境界を越える参照の 9 件は、成果物そのものが書いているものです。`.proto` の import、規則の `import proto` と `shape`、カレンダーの `use calendar`、ワークフローの `use rule`・`use proto`・`connect`・子のフローがそれにあたります。
sakai はこれを、それぞれの言語が自分のファイルを読んだ結果として、ritsu を通して同じプロセスの中で受け取ります。

## 検査が言うこと

在庫が `PackingStatus` に値を一つ足したとします。protobuf ではワイヤの上で互換な変更なので、どこも止めません。
配送の腐敗防止層にはその値の行が無いので、新しい値をどう扱うかをだれかが決めるまで、検査は通りません。

```text
エラー[E401]: contexts/配送.ctx:33:3: 「配送」の腐敗防止層の対応に、「在庫」の列挙 warehouse.v1.PackingStatus の値 PACKING_STATUS_DAMAGED がありません
    33 |   enum PackingStatus -> 出荷の可否
  = PACKING_STATUS_DAMAGED は proto/warehouse/v1/stock.proto:43 の値です。
  = 上流の列挙の値ごとに、下流の値か refuse（拒否）を書いてください。上流が値を足すと、その値をどう扱うかを決めるまで、検査は通りません。
  = 直した行: PACKING_STATUS_DAMAGED -> refuse "…"
```

二つのコンテキストが一つの語を違う意味で使うのは、おかしなことではありません。境界はそのためにあります。
困るのは、その語が境界を越えたときです。次の例では、受注が自分の「引当」を書いていて、そこに在庫の「引当」が、受注の import する `.proto` を通って越えてきます。

```text
エラー[E406]: contexts/受注.ctx:22:3: 「在庫」の「引当」が、「受注」の違う意味の「引当」とぶつかったまま、境界を越えています
    22 |   引当 "客の注文の一行に、届ける日を割り当てること"
  = 「在庫」の「引当」は「注文の一行のために、棚の在庫を出荷か取消まで押さえておくこと」で、proto "proto/warehouse/v1/stock.proto" message ReserveResponse を指します。
  = 「受注」の「引当」は「客の注文の一行に、届ける日を割り当てること」です。
  = 「受注」は「在庫」の順応者なので、対応を書いて読み替えることはできません。
```

二つの定義が同じ意味かどうかを、sakai は決めません。同じ意味だと言えるのは、書いた人の `as` だけです。
ほかに確かめることは次のとおりです。範囲の成果物がどれもちょうど一つのコンテキストに属すること。参照が、関係の許すところでだけ境界を越えること（順応者は上流の公表された言語を読める、腐敗防止層は層の中からだけ、サービスを呼べるのは公開ホストサービスだけ）。パターンどうしが食い違わないこと（顧客には供給者の同意が要る、共有カーネルは両側が同じものを並べる）。別々の道では何も越えないこと。
診断のコードは、いつ出るか、どう直すか、最小の再現とともに [docs/codes.ja.md](docs/codes.ja.md) にあります（`sakai explain E401 --lang ja`）。

## サービスのあいだの契約：OpenAPI と AsyncAPI

HTTP やイベントでやりとりするサービスは、やりとりの中身を契約の文書に書きます。HTTP の API なら OpenAPI の文書、サービスが送ったり受けたりするチャネルなら AsyncAPI の文書です。
sakai は、これらの文書を `.proto` と同じく成果物として扱います。コンテキストは自分の文書を公表された言語にし、だれでも使ってよい HTTP の操作とチャネルを公開ホストサービスに並べます。

```ctx
published language payments.v1
  openapi "../payments/api/payments.yaml"
  asyncapi "../payments/events/payments.yaml"
  open host service createCharge, getCharge, paymentSucceeded, paymentFailed
```

境界を越える参照になるのは、文書に書いてあるものです。ほかのコンテキストの文書を指す `$ref` と、ほかのコンテキストのチャネルに送ったりそこから受けたりする操作です。
どれも、ほかの参照と同じ決まりで関係と突き合わせます。腐敗防止層は、文書の列挙も、`.proto` の列挙と同じく値ごとに読み替えます。

```ctx
upstream 決済 anticorruption layer
  through payments.v1
  layer dir "../shipping/acl"
  enum ChargeStatus -> enum ShipmentGate
    pending   -> hold
    succeeded -> release
    failed    -> refuse "課金に失敗した注文は出荷しない"
    refunded  -> refuse "返金した注文は出荷しない"
```

```console
$ sakai check examples/webshop.ja/ネットショップ.ctx --lang ja
examples/webshop.ja/ネットショップ.ctx: ok — コンテキスト 4、関係 5。成果物 9 件は、どれも一つのコンテキストに属する。境界を越える参照 7 件を確かめた（openapi 1、asyncapi 5、dandori 1）
```

決済が、配送の受け取っているチャネルを開くのをやめると、次のように言います。

```text
エラー[E210]: shipping/acl/payments.yaml:11:5: 「配送」の shipping/acl/payments.yaml が、「決済」の公開ホストサービスでないチャネル paymentFailed を使っています（receive）
    11 |     $ref: '../../payments/events/payments.yaml#/channels/paymentFailed'
  = 公表された言語 payments.v1 の公開ホストサービスは createCharge、getCharge、paymentSucceeded です。
  = 境界の向こうのチャネルに送ったりそこから受けたりできるのは、相手が `open host service` に並べたチャネルだけです（HTTP の操作も同じです）。相手の公表された言語の `open host service` に足してもらうか、相手が開いたものを使ってください。
  関わるもの:
      配送  shipping/acl/payments.yaml:11                                   $ref: ../../payments/events/payments.yaml#/channels/paymentFailed
      決済  asyncapi "payments/events/payments.yaml" channel paymentFailed  公表された言語 payments.v1 のもの
```

読める文書は、OpenAPI の 3.0、3.1、3.2 と、AsyncAPI の 3.0、3.1 で、JSON でも YAML でもかまいません。
YAML は、JSON と行き来できる書き方（RFC 9512 の 3.4 節。OpenAPI 3.2 と AsyncAPI 3.1 が勧めるもの）を読みます。その外の書き方（タグ、`?` で書くキー、二つ目の文書など）は、一部だけを読むことはせず、E108 で止めます。
契約に書いていない呼び出し（URL を文字列で持つ HTTP、文書の無いキュー）は、これまでどおり確かめず、`doc` のページにそう書きます。コードが契約のとおりに呼んでいるかも、sakai は確かめません。
このやりとりをする四つのサービスの地図が [examples/webshop.ja](examples/webshop.ja/README.ja.md) です。

## 鍵、暗号化しない通信、認証の指定

sakai は、ritsu のセキュリティの検査のうち三つを受け持ちます（9xx のコードは、ritsu のどの言語でも同じことを言います）。
どれも警告で、意図していることをファイルに書けば消えます。

| コード | 見つけるもの | 意図の書き方 |
|---|---|---|
| W901 | 地図か context のファイルに書いた、形の決まった鍵（AWS、GitHub、Slack、Stripe、OpenAI、Anthropic、Google、PEM の秘密鍵） | 同じ行のコメントに `ritsu: test secret` |
| W902 | 地図の文書のサーバーが、ほかの機械へ暗号化しない通信をする（OpenAPI の `http://` と `ws://`、AsyncAPI の `http`、`ws`、`amqp`、`mqtt`、`stomp`、`kafka`） | サーバーに `x-ritsu-plaintext: "<理由>"` |
| W903 | 公表された言語の OpenAPI の操作に `security` が無い（文書にも無い）、または AsyncAPI のチャネルのサーバーに `security` が無く、チャネルの操作にも無い | 操作か文書かサーバーに `security: []` |

鍵の診断は、鍵の種類と接頭辞と長さだけを言い、鍵そのものは出しません。ほかの診断が見せる行の鍵も伏せます。
文書と `.proto` に書いた鍵は、ほかの言語もそのファイルを読むので、`ritsu check` が一度だけ言います。
どの操作をだれに許すか（認可）は、ここでは見ません。
決済の API の `security` を消すと、次のように言います。

```text
警告[W903]: payments/api/payments.yaml:9:7: 公表された言語 payments.v1 の操作 createCharge に、認証の指定がありません
     9 |       operationId: createCharge
  = 操作にも文書にも `security` が無いので、契約を読む人には、クライアントがどう認証すればよいかが分かりません。
  = 操作か文書全体に `security` を書いてください。だれでも呼べるようにわざとしている操作なら、その操作に `security: []` と書いてください。
  関わるもの:
      決済  openapi "payments/api/payments.yaml" operation createCharge  POST /charges
```

## 地図のページ

`sakai doc` は、コードが実現すべきものを理解し、確かめる人のためのページを書きます。事業を回す人、システムを運用する人、コードを読む開発者が読み手です。
ページには、コンテキストマップの図と、コンテキストごとの成果物（それぞれの言語が言う中身つき。規則なら入力と出力、帳簿なら勘定と振替、カレンダーならデータの範囲）、公表された言語、用語集、関係ごとの越える参照、腐敗防止層の対応の表が並びます。規則の `import proto` が決めている対応も、表に出ます。
Markdown では図を Mermaid で描くので、GitHub がそのまま表示します。HTML は外のものを何も読まない一枚のファイルで、図のコンテキストを押すと、そのコンテキストの節に移ります。
例のページは [tests/golden/doc/通販.ja.md](tests/golden/doc/通販.ja.md) です。

## コードの import

sakai はコードの import を読みません。どの言語にも、チームがもう CI で走らせている import の検査のツールがあるので、`sakai build` が地図からその設定を書きます。

```console
$ sakai build examples/shop.ja/通販.ctx --target import-linter --check --lang ja
examples/shop.ja/py/.importlinter: いまの地図から書くものと同じ（契約 11 件）
$ sakai build examples/shop.ja/通販.ctx --target go-arch-lint --check --lang ja
examples/shop.ja/go/.go-arch-lint.yml: いまの地図から書くものと同じ（コンポーネント 11 件）
```

- Python は import-linter です。パッケージの下にある `__init__.py` の無いディレクトリ（protoc が書く名前空間のパッケージ）を import-linter は読まないので、sakai はそれを別の根として並べます。
- TypeScript と JavaScript は dependency-cruiser です。CI では `--output-type err` で走らせます。`json` は違反があっても exit 0 で終わります。TypeScript の版に気をつけてください。dependency-cruiser 16 が読めるのは 6 より前の TypeScript で、TypeScript 6 以降だと `.ts` のファイルを一つも読まずに、何も言わずに通ります。テストは TypeScript 5.9.3 で回しています。
- Java は ArchUnit です。組んだクラスが使うものを読むので、import の文を書かずに完全な名前で使ったクラスも見つけ、使っていない import は見ません。ほかの三つは import の文を読みます。
- Go は go-arch-lint です。
- Rust にはツールを使いません。`sakai check` が Cargo にクレートを尋ね、その依存を地図と突き合わせます。

設定は `--lang` の言語で規則の説明を書くので、CI の `--check` は、設定を書いたときと同じ `--lang` で走らせます。
設定の中身、ツールごとに捕まえるものの違い、Context Mapper の CML への書き出し（`sakai export cml`）は [docs/targets.md](docs/targets.md)（英語）にあります。

## AI エージェント向けのスキル

[skills/sakai](../../skills/sakai/SKILL.md) は、地図を書いたり直したりするエージェントのためのスキルです。書く流れ、言語の一枚の要約、人に聞くこと、診断ごとの直し方への案内があります。
`SKILL.md` のほかのページは `docs/` からコピーしたもので、`skills/sync.sh` がコピーします。

## インストール

sakai は ritsu と一緒に入ります。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu sakai …` で走らせます。
`sakai` という名前のリンクから呼ぶと、ritsu は sakai として動き、sakai が読む言語は全部つながります。リリースのアーカイブにはリンクが入っています。手で作るなら、ritsu の隣で `ln -s ritsu sakai` とします。
sakai のクレートのバイナリはほかの言語を持たないので、規則やカレンダーやワークフローを含む地図では E104 で止まります。

## コマンド

```console
$ sakai check examples/shop.ja/通販.ctx --lang ja
$ sakai build examples/shop.ja/通販.ctx --target archunit --lang ja
$ sakai export cml examples/shop.ja/通販.ctx --lang ja
$ sakai doc examples/shop.ja/通販.ctx --format html --out site --lang ja
$ sakai api examples/shop.ja/通販.ctx
$ sakai explain E401 --lang ja
```

`check` は地図のファイルとディレクトリを受け取り、`--format json` も付けられます。`--root` は、パスを数える起点のディレクトリを替えます（無ければ、上にたどって `.git` のある一番近いディレクトリ）。
どのコマンドも `--lang ja|en`（無ければ `SAKAI_LANG`、次に `RITSU_LANG`、どちらも無ければ英語）と `--help` を受け付けます。
終了コードは、エラーが無ければ 0、エラーがあれば 1、引数の誤り、読めないファイル、つながっていない言語の成果物なら 2 です。

## 例

- [examples/shop.ja](examples/shop.ja/README.ja.md)：五つのコンテキストと七つの関係からなる小さな通販です。六つのパターンが全部出てきます。本物のワークフロー、規則、カレンダー、帳簿、`.proto` があり、コードは Python、TypeScript、Java、Go の四つです。
- [examples/shop](examples/shop/README.md)：同じ通販を英語の名前で書いたものです。
- [examples/webshop.ja](examples/webshop.ja/README.ja.md)：HTTP とイベントでやりとりするネットショップの四つのサービスを、OpenAPI と AsyncAPI の文書でつないだ地図です。腐敗防止層が、文書の列挙を値ごとに読み替えます。[examples/webshop](examples/webshop/README.md) は、同じものを英語の名前で書いたものです。
- ritsu の根の [ritsu.ctx](../../ritsu.ctx) と [contexts/](../../contexts)：ritsu 自身のクレートを十二のコンテキストに分けた地図です。CI が `ritsu check ritsu.ctx` で、クレートの依存をこの地図と突き合わせています。

## どう確かめているか

`cargo test -p sakai` が、言語を fixture と例に当て、本物のツールを走らせます。
macOS（Apple シリコン）で `cargo test -p sakai -- --nocapture` を一度走らせた結果は、ツールが全部そろい SKIP の無い状態で、テスト 191 件、ビルドのあと 26 秒でした。

- 168 の変異（fixture か例を一か所だけ変えたもの）が出す診断を、英語と日本語の両方で golden と突き合わせます。日本語の名前のもの 82 には、それぞれ英語の名前の対があり、ほかに Rust のものが 4 あります。
- 四つの import の検査のツールを、56 のコピー（ツールごとに 14）で走らせます。コピーは、例と、生成したコードがコンテキストの内側にある地図の二つを、そのままのものと、地図が許さない import を足したものにし、日本語と英語の名前の両方で作ります。どのツールも、そのままのコピーを通し、足した import をどれも捕まえます。
- 例の CML を、Context Mapper 6.12.0 の検証に全部の検査で通し、何も言われないことを確かめます。
- ritsu の YAML の読み手を、YAML のテストスイート（data-2022-01-17 の版、402 ケース）にかけます。204 ケースはスイートの JSON と同じ値に読み、JSON と行き来できる書き方の外の 104 ケースは読まずに止め、YAML でない 94 ケースはどれも読みません。違う値に読んだケースはありません。
- 例と fixture の `.proto` を sakai が読んだ結果を、buf の結果と比べます。
- `doc` のページは golden です。Mermaid 11 と 12 で図を描き、Chrome で HTML を開いて、図のコンテキストを押すとその節に移ることを確かめます。
- この README と `docs/` とスキルに載せた出力、診断、設定、`.ctx` の行は本物です。`tests/docs.rs` が走らせて突き合わせます。

## 状態

言語、`check`、`build`、`export cml`、`doc`、`api`、`explain` はできています（[DESIGN.md](DESIGN.md)）。
まだ作っていないもの：持ち主を CODEOWNERS と突き合わせること、規則の `import jsonschema` を対応の先にすること、対応から読み替えのコードを生成すること、契約に書いていない実行時の呼び出しを成果物として書くこと（DESIGN の 14 章と 15.11）。

## ライセンス

[Apache License, Version 2.0](LICENSE-APACHE) と [MIT license](LICENSE-MIT) のどちらかを選んで使えます。
例の中にある内閣府の祝日の表のコピーは、それ自身の条件に従います（[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)）。
