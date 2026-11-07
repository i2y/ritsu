# 通販

五つの境界づけられたコンテキストからなる小さな通販を、sakai の地図（`通販.ctx`）で書いた例です。
地図の下にあるのは、ritsu のほかの言語の本物の成果物です。ワークフロー（dandori）、規則（rulec）、日付（koyomi）、帳簿（chobo）、それに各コンテキストの公表された言語の `.proto` があります。
英語の名前で書いた同じ例が [`../shop`](../shop/README.md) にあります。二つは名前が違うだけで、成果物も関係も同じで、同じ参照を見つけ、同じ診断を出します。

```console
$ sakai check examples/shop.ja/通販.ctx --lang ja
examples/shop.ja/通販.ctx: ok — コンテキスト 5、関係 7。成果物 79 件は、どれも一つのコンテキストに属する。境界を越える参照 10 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 6）
```

地図が規則、カレンダー、ワークフローを含むので、`ritsu sakai`（か、同じ名前のリンクの `sakai`）で確かめます。sakai は、それらのファイルが何を参照しているかを、同じプロセスの中で、それぞれの言語に尋ねます。
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
| 受注 → 在庫 | 順応者。`warehouse.v1` を通る | `fulfillment.proto` が `stock.proto` を import する。`受注.flow` が `stock.proto` を読み、`StockService` の `Reserve`・`Release`・`Post` を呼ぶ |
| 請求 → 受注 | 腐敗防止層。`shop.ordering.v1` を通り、層は `請求の要否.rule` と各言語の `billing/acl/ordering` | `請求の要否.rule` が `import proto` で `OrderStatus` を取り込む。受注の「キャンセル」は規則の値「受注で取消」になり、請求の「キャンセル」とは別のものとして読まれる |
| 配送 → 在庫 | 腐敗防止層。`warehouse.v1` を通り、層は各言語の `delivery/acl/inventory` | 言語の成果物の参照は無い。`PackingStatus` の対応は `配送.ctx` に値ごとに書き、`PACKING_STATUS_SHORT` は拒否する |
| 請求 → 配送 | 顧客と供給者。`shop.delivery.v1` を通る | `出荷の送料.rule` の `shape` が `CreateShipmentRequest` を読む |
| 請求と配送 | 共有カーネル。`東京の営業日.cal` と、koyomi がそれから書くコード | `出荷日.cal` がそのカレンダーを使う |
| 受注と配送 | パートナーシップ | `受注.flow` が規則 `出荷の急ぎ` を Connect で呼び、`配送の手配.flow` を子として走らせる |
| レビューと請求 | 別々の道 | 何も越えない。何も越えないことを sakai が確かめる |

## 引当から確定まで

注文の明細は、明細の ID をキーに、14 日のあいだ帳簿に押さえます（`inventory/在庫の引当.book` の `pending expires after 14 days`）。
倉庫の梱包の担当者が答えるまでの 2 日にも、受注から出荷日までの最長 7 日（`delivery/出荷日.cal`）にも足りる長さです。
`受注.flow` は、梱包が済むとお客さまに知らせる前に、押さえたぶんを確定し（`StockService/Post`）、押さえたあとで何かが失敗したら、押さえたぶんを全部戻します（`StockService/Release`）。
`chobo run` で、押さえが持ちこたえなければならない場面、梱包に 2 日かかる場合を流すと、次のようになります。

```text
inventory/在庫の引当.book (梱包に 2 日かかり、そのあと出荷する): 5 ステップ
  1  入荷.do(納品書: d1, sku: A, 数: 1)  通る
  2  引当.hold(明細: l1, sku: A, 数: 1)  通る
  3  pass 2 days
  4  引当.post(明細: l1)                 通る
  5  引当.hold(明細: l2, sku: A, 数: 1)  在庫切れ で拒否される
```

押さえが前の 30 分のままなら、同じ手順が違う結末になります。3 では押さえが切れて棚に戻り（`引当(o1, A) expired`）、4 は `refused: expired`、5 では別の注文がその 1 個を押さえます（`done`）。
`ritsu run` で、ワークフローの五つの終わり方を流しました（2 明細の注文、`l1` は sku A、`l2` は sku B、配送は通常便）。押さえたぶんに対する呼び出しは次のとおりです。

| 終わり方 | 押さえたあとの倉庫への呼び出し |
|---|---|
| 成功 | `引当を確定する(l1)`、`引当を確定する(l2)`、そのあと `知らせる` |
| `在庫不足`（`l2` が `STOCK_SHORT` か `STOCK_CONFLICT`） | `引当を戻す(l1)` |
| `配送の手配の失敗` | `引当を戻す(l1)`、`引当を戻す(l2)` |
| `梱包の遅れ`（2 日たっても知らせがない） | `引当を戻す(l1)`、`引当を戻す(l2)` |
| `梱包の欠品`（梱包の担当者がエラー `欠品` で応答） | `引当を戻す(l1)`、`引当を戻す(l2)` |

予約の答えが `STOCK_CONFLICT` になるのは、帳簿が `key_conflict` を返したとき、つまり同じ明細を前に別の sku や数で押さえていたときです。
`done_before` は、帳簿の意味どおり、サービスは成功として答えます。

## ファイルの出どころ

成果物は、ritsu の各言語の例をコピーし、パスをこの例の置き方に合わせたものです。コピーしたファイルは、頭のコメントに、コピー元と直したところを書いています。

- `ordering/受注.flow`、`delivery/配送の手配.flow`、`delivery/rules/出荷の急ぎ.rule`、`proto/dandori/v1/options.proto`：dandori の例。
- `inventory/在庫の引当.book`：chobo の例。キーを明細（注文と sku ではなく）にし、押さえを 14 日にした。
- `billing/rules/決済手数料.rule`、`billing/rules/出荷の送料.rule`、`proto/shop/delivery/v1/shipment.proto`：rulec の corpus。
- `billing/支払条件.cal`、`calendars/東京の営業日.cal`、`calendars/data/syukujitsu.csv`：koyomi の例。
- この例のために書いたもの：`billing/rules/請求の要否.rule`、`delivery/出荷日.cal`、`proto/shop/ordering/v1/order.proto`、`proto/warehouse/v1/stock.proto`（dandori の `warehouse.proto` に、梱包のサービスと `Post`、予約への明細の ID、`STOCK_CONFLICT`、数量と金額の範囲を足したもの）。

`py/`、`ts/`、`java/`、`go/` のコードは、本物のコードの代わりです。どのファイルも数行の手書きで、境界に関わる import だけを持ちます。
本物なら生成するコード（`.proto` から、あるいは rulec、koyomi、chobo が書くもの）は、どのコマンドが本物を書くかをファイルの頭に書いています。
import の検査が見るのは境界で、コードが何をするかではないからです。
コードの横の設定（`py/.importlinter`、`ts/.dependency-cruiser.cjs`、`java/src/test/java/SakaiContextsTest.java`、`go/.go-arch-lint.yml`）は、この地図から `sakai gen … --lang ja` が書いたもので、テストが地図と突き合わせます。
