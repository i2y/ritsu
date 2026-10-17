# 例で見る

例は四つあり、どれも英語の版の横に、名前を全部日本語にした日本語の版（`<名前>.ja.book`）があります。日本語の版の単位は `個`、`pt`、`円` です。どの例も `chobo check` を警告なしに通り、テストは、`chobo scenarios` が作るシナリオの全部をすべてのクライアントで流しています（[どうやって確かめているか](assurance.md)）。リポジトリでは、帳簿の横に `chobo doc` が描いたページ `doc.md` と `doc.ja.md` があり、GitHub で読めます。

| 例 | 見せるもの | 帳簿 | 描いたページ |
|---|---|---|---|
| 注文のために 30 分押さえ、出荷で確定し、キャンセルで取り消す在庫 | 期限の切れる仮押さえ、二つの引数でできたキー | [在庫の引当](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/inventory/inventory.ja.book) · [英語の版](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/inventory/inventory.book) | [ページ](doc/inventory.html) |
| 返品の期間が過ぎるまで押さえて付与し、支払いが通るまで押さえて使い、期間の終わりに失効するポイント | 下限と上限、期限の切れない仮押さえ | [ポイント](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/points/points.ja.book) · [英語の版](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/points/points.book) | [ページ](doc/points.html) |
| 売上を超えない返金 | 二つの勘定にまたがる条件を、一つの勘定の境界にしたもの。同時に来る二つの返金 | [返金](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/refunds/refunds.ja.book) · [英語の版](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/refunds/refunds.book) | [ページ](doc/refunds.html) |
| 買い手が払った額を、店の取り分と手数料に分けるマーケットプレイス | 全部通るか一つも通らない二つの移動、呼ぶ側が計算する額 | [マーケットプレイス](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/marketplace/marketplace.ja.book) · [英語の版](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/marketplace/marketplace.book) | [ページ](doc/marketplace.html) |

## 在庫の引当

SKU ごとの在庫です。入荷で増え、注文のために 30 分押さえ、出荷で仮押さえを確定し、キャンセルで取り消します。返品は棚に戻します。

```book
account 在庫(sku: string) : 個
  description "倉庫の棚にある数"
  at least 0 refused as 在庫切れ
…
transfer 引当(注文: string, sku: string, 数: 個)
  description "出荷で確定し、キャンセルで取り消す"
  key 注文, sku
  pending expires after 30 minutes
  move 数 from 在庫(sku) to 客
```

キーは注文と SKU の組なので、三つの SKU を頼んだ注文は、SKU ごとに一度ずつ、三回押さえます。ある SKU の最後の一つを二つの注文が同時に押さえようとすると、一方が押さえ、もう一方は `在庫切れ` で拒否されます。30 分そのままにされた仮押さえは期限が切れ、押さえていた品は棚に戻ります。

## ポイント

会員ごとのポイントで、0 から 100 万までです。購入で付くポイントは、返品の期間が過ぎるまで押さえ、この仮押さえには期限がありません。会計で使うポイントは、支払いが通るまで 15 分押さえます。期間の終わりに使われずに残った分は失効します。

```book
account ポイント(会員: string) : pt
  at least 0 refused as ポイント不足
  at most 1000000 refused as 上限超え
…
transfer 付与(購入: string, 会員: string, 数: pt)
  description "返品の期間が過ぎたら確定し、返品されたら取り消す"
  key 購入
  pending never expires
  move 数 from 発行 to ポイント(会員)
…
transfer 失効(会員: string, 期間: string, 数: pt)
  description "その期間に使われなかった分"
  key 会員, 期間
  move 数 from ポイント(会員) to 失効済み
```

付与は上限で、利用と失効は下限で拒否されます。押さえて入ってくるポイントは、上限に対しては数えられますが、使えるポイントとしては数えられません。だから、購入で付くポイントは、返品の期間が過ぎて仮押さえが確定するまで使えません。

## 返金

注文ごとに、返金できる残りを勘定にしています。売上で増え、返金で減り、0 を下回りません。返金は承認を待つあいだ、長くても 7 日押さえます。帳簿の全体はホームに載せています。

```book
transfer 返金(申請: string, 注文: string, 額: 円)
  description "承認を待つあいだ押さえる。承認されたら確定し、却下されたら取り消す"
  key 申請
  pending expires after 7 days
  move 額 from 返金できる残り(注文) to 返金済み
```

横には、手で書いたシナリオ `refunds.ja.more.json` があります。30000 円の売上に 20000 円の返金が二つ同時に来て、そのあと 10000 円を返金するもので、[シナリオを流す](scenarios.md#同時に来る呼び出し元)で流しています。テストは、このシナリオもすべての出力先で流します。

## マーケットプレイス

店ごとに売上金を持ちます。売上は、買い手が払った額を店の売上金と手数料に分け、その額は呼ぶ側が計算して渡します。返金は両方を戻すか、どちらも戻しません。振込は、銀行が受け付けるまで押さえます。

```book
transfer 返金(返金番号: string, 店: string, 店の取り分: 円, 手数料: 円)
  description "手数料と店の取り分を、両方とも戻すか、どちらも戻さない"
  key 返金番号
  move 手数料 from 手数料収入 to 買い手
  move 店の取り分 from 売上金(店) to 買い手
…
transfer 振込(振込番号: string, 店: string, 額: 円)
  description "銀行振込を始めたときに押さえる。銀行が受け付けたら確定し、失敗したら取り消す"
  key 振込番号
  pending never expires
  move 額 from 売上金(店) to 銀行
```

店の売上金を 0 より下にする返金は 2 つ目の移動で拒否され、1 つ目の移動で戻した手数料も、一緒に元に戻ります。この帳簿は、[帳簿を書く](tour.md)で一行ずつ読んでいます。

## テスト用の帳簿

[tests/books](https://github.com/i2y/ritsu/tree/main/crates/chobo/tests/books) の帳簿は、例で試していないことを試します。どれにも、英語の名前の版があります。

- `安全在庫`（`safety_stock`）：0 より上の下限
- `与信`（`credit`）：マイナスまで使える後払いの枠と、前払いの上限
- `ウォレット`（`wallet`）：上限のあるウォレット
- `自分あて`（`to_itself`）：同じ種類の二つの勘定のあいだの移動
- `分配`（`allocation`）：移動が三つある売上と、円からドルへの両替

`在庫` とその英語の版 `stock_reservation` には、手で書いたシナリオが横にあります。[tests/fixtures](https://github.com/i2y/ritsu/tree/main/crates/chobo/tests/fixtures) には、一つから数個の診断を出す帳簿が、日本語の名前のものと英語の名前のもので、対になって並んでいます。
