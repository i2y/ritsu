# 通販

五つの境界づけられたコンテキストからなる小さな通販を、sakai の地図（`通販.ctx`）で書いた例です。
地図の下にあるのは、ritsu のほかの言語の本物の成果物です。ワークフロー（dandori）、規則（rulec）、日付（koyomi）、帳簿（chobo）、それに各コンテキストの公表された言語の `.proto` があります。
英語の名前で書いた同じ例が [`../shop`](../shop/README.md) にあります。二つは名前が違うだけで、成果物も関係も同じで、同じ参照を見つけ、同じ診断を出します。

```console
$ sakai check examples/shop.ja/通販.ctx --lang ja
examples/shop.ja/通販.ctx: ok — コンテキスト 5、関係 7。成果物 79 件は、どれも一つのコンテキストに属する。境界を越える参照 9 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 5）
```

地図が規則、カレンダー、ワークフローを含むので、`ritsu sakai`（か、同じ名前のリンクの `sakai`）で確かめます。sakai は、それらのファイルが何を名指しているかを、同じプロセスの中で、それぞれの言語に尋ねます。
`sakai doc examples/shop.ja/通販.ctx --lang ja` で地図のページを書けます。[`tests/golden/doc/通販.ja.md`](../../tests/golden/doc/通販.ja.md) がそのページです。

## コンテキスト

| コンテキスト | 持ち主 | 持っている成果物 |
|---|---|---|
| 受注（`ordering`） | 受注チーム | `ordering/受注.flow`（dandori）、`proto/shop/ordering/v1/`（`order.proto`、`fulfillment.proto`） |
| 在庫（`inventory`） | 倉庫チーム | `inventory/在庫の引当.book`（chobo）、`proto/warehouse/v1/stock.proto` |
| 配送（`delivery`） | 配送チーム | `delivery/配送の手配.flow`（dandori）、`delivery/rules/出荷の急ぎ.rule`（rulec）、`delivery/出荷日.cal`（koyomi）、`proto/shop/delivery/v1/shipment.proto` |
| 請求（`billing`） | 経理チーム | `billing/rules/`（`請求の要否.rule`、`決済手数料.rule`、`出荷の送料.rule`）、`billing/支払条件.cal`、`calendars/東京の営業日.cal` |
| レビュー（`reviews`） | 受注チーム | コードだけ |

どのコンテキストも、`py/`、`ts/`、`java/`、`go/` の下に自分のコードのディレクトリを持ちます。

## 関係

| 関係 | パターン | 越えていくもの |
|---|---|---|
| 受注 → 在庫 | 順応者。`warehouse.v1` を通る | `fulfillment.proto` が `stock.proto` を import する。`受注.flow` が `stock.proto` を読み、`StockService` の `Reserve` と `Release` を呼ぶ |
| 請求 → 受注 | 腐敗防止層。`shop.ordering.v1` を通り、層は `請求の要否.rule` と各言語の `billing/acl/ordering` | `請求の要否.rule` が `import proto` で `OrderStatus` を取り込む。受注の「キャンセル」は規則の値「受注で取消」になり、請求の「キャンセル」とは別のものとして読まれる |
| 配送 → 在庫 | 腐敗防止層。`warehouse.v1` を通り、層は各言語の `delivery/acl/inventory` | 言語の成果物の参照は無い。`PackingStatus` の対応は `配送.ctx` に値ごとに書き、`PACKING_STATUS_SHORT` は断る |
| 請求 → 配送 | 顧客と供給者。`shop.delivery.v1` を通る | `出荷の送料.rule` の `shape` が `CreateShipmentRequest` を読む |
| 請求と配送 | 共有カーネル。`東京の営業日.cal` と、koyomi がそれから書くコード | `出荷日.cal` がそのカレンダーを使う |
| 受注と配送 | パートナーシップ | `受注.flow` が規則 `出荷の急ぎ` を Connect で呼び、`配送の手配.flow` を子として走らせる |
| レビューと請求 | 別々の道 | 何も越えない。何も越えないことを sakai が確かめる |

## ファイルの出どころ

成果物は、ritsu の各言語の例を写し、パスをこの例の置き方に合わせたものです。写したファイルは、頭のコメントに、写した元と直したところを書いています。

- `ordering/受注.flow`、`delivery/配送の手配.flow`、`delivery/rules/出荷の急ぎ.rule`、`proto/dandori/v1/options.proto`：dandori の例。
- `inventory/在庫の引当.book`：chobo の例。
- `billing/rules/決済手数料.rule`、`billing/rules/出荷の送料.rule`、`proto/shop/delivery/v1/shipment.proto`：rulec の corpus。
- `billing/支払条件.cal`、`calendars/東京の営業日.cal`、`calendars/data/syukujitsu.csv`：koyomi の例。
- この例のために書いたもの：`billing/rules/請求の要否.rule`、`delivery/出荷日.cal`、`proto/shop/ordering/v1/order.proto`、`proto/warehouse/v1/stock.proto`（dandori の `warehouse.proto` に梱包のサービスを足したもの）。

`py/`、`ts/`、`java/`、`go/` のコードは、本物のコードの代わりです。どのファイルも数行の手書きで、境界に関わる import だけを持ちます。
本物なら生成するコード（`.proto` から、あるいは rulec、koyomi、chobo が書くもの）は、どのコマンドが本物を書くかをファイルの頭に書いています。
import の検査が見るのは境界で、コードが何をするかではないからです。
コードの横の設定（`py/.importlinter`、`ts/.dependency-cruiser.cjs`、`java/src/test/java/SakaiContextsTest.java`、`go/.go-arch-lint.yml`）は、この地図から `sakai build … --lang ja` が書いたもので、テストが地図と突き合わせます。
