# 引当と発送 v1

注文の明細ごとに在庫を引き当て、配送を手配し、倉庫の梱包を待ってから、お客さまに知らせる。明細は並べて引き当て、足りない明細があれば、引き当てたぶんを戻す。Temporal 向けの版：倉庫の呼び出しは dandori が Connect で生成するアクティビティ。配送チームのワークフローが専用のタスクキューで配送を手配し（子ワークフロー、arrange_delivery.ja.flow）、翌日便の車がなければ通常便で頼み直す。梱包の依頼、お知らせ、記録は自分で書くアクティビティで、梱包の担当者は、生成したクライアントが送る Update でコールバックに応答する

`examples/fulfillment/temporal/fulfillment.ja.flow` を `dandori doc` で描いたものです。入力は `注文: 注文`、出力は `引当: list[warehouse.ReserveResponse]`, `追跡番号: string` です。このワークフローは `shop.ja.v1.FulfillmentService`（`../specs/fulfillment.ja.proto`）を実装します。

## flow

```mermaid
flowchart TD
    start(["引当と発送 v1"])
    s1[["判定 = 急ぎ(…)<br>rule 出荷の急ぎ.rule"]]
    subgraph L2 ["let 結果 = for 明細 in 注文.明細 at most 50 in parallel, 10 at a time · yield 答え"]
        s3["答え = 在庫を引き当てる(…)<br>connect warehouse StockService/Reserve<br>retry 2 times every 1 second on 混雑"]
    end
    s4["let 足りない = false"]
    subgraph L5 ["for 一つ in 結果 at most 50"]
        s6{{"match 一つ.stock"}}
        s7["let 足りない = true"]
    end
    s9{{"match 足りない"}}
    subgraph L10 ["for 戻す in 結果 at most 50 in parallel"]
        s11{{"match 戻す.id"}}
        s12["引当を戻す(…)<br>connect warehouse StockService/Release"]
    end
    s14(["fail 在庫不足<br>#quot;注文 {注文.id} には在庫の足りない明細があります#quot;"])
    s16["let 宛名 = none"]
    s17{{"match 注文.贈り物"}}
    s18["let 宛名 = 贈り物.宛名"]
    s19{{"match 贈り物.メッセージ"}}
    s20["記録する(…)<br>自分で書くタスク"]
    s23["配送 = 配送を手配する(…)<br>flow arrange_delivery.ja.flow"]
    s24["配送 = 配送を手配する(…)<br>flow arrange_delivery.ja.flow"]
    s25(["fail 配送の手配の失敗<br>#quot;注文 {注文.id} の配送を手配できませんでした#quot;"])
    s26(["fail 配送の手配の失敗<br>#quot;注文 {注文.id} の配送を手配できませんでした#quot;"])
    s27[/"箱 = 梱包を待つ(…)<br>自分で書くタスク（応答はコールバック）<br>timeout 2 days"/]
    s28(["fail 梱包の遅れ<br>#quot;二日たっても梱包の知らせがありません#quot;"])
    s29["知らせる(…)<br>自分で書くタスク"]
    s31(["succeed 引当 = 結果, 追跡番号 = 配送.追跡番号"])
    start --> s1
    s1 --> s3
    L2 -->|"すべてのイテレーションが終わったら"| s4
    s4 --> s6
    s6 -->|"short"| s7
    s7 -->|"次のイテレーション"| s6
    s6 -->|"secured"| s6
    L5 -->|"最後の項目のあと"| s9
    s9 -->|"true"| s11
    s11 -->|"some id"| s12
    L10 -->|"すべてのイテレーションが終わったら"| s14
    s9 -->|"false"| s16
    s16 --> s17
    s17 -->|"some 贈り物"| s18
    s18 --> s19
    s19 -->|"some 一言"| s20
    s20 --> s23
    s19 -->|"none"| s23
    s17 -->|"none"| s23
    s23 -.->|"on 翌日便の空きなし"| s24
    s24 -.->|"on failure"| s25
    s23 -.->|"on failure"| s26
    s23 --> s27
    s24 --> s27
    s27 -.->|"on timeout"| s28
    s27 --> s29
    s29 --> s31
    s29 -.->|"on 宛先なし"| s31
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s31 ok
    class s14,s25,s26,s28 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## サービス

サービスのメソッドと、それぞれが実行に対して何をするかです。

| メソッド | 何をするか | タスク |
|---|---|---|
| `Fulfill` | 実行を始めます。失敗の名前は `在庫不足`・`配送の手配の失敗`・`梱包の遅れ` | — |
| `AnswerPacking` | コールバックに応答します | `梱包を待つ` |

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 73 | `判定 = 急ぎ(…)` | 規則 `出荷の急ぎ.rule` | 2 回（1 秒後と 2 秒後、failure） | — | `timeout`, `failure` → ワークフローが失敗する |
| 75 | `答え = 在庫を引き当てる(…)` | `connect warehouse StockService/Reserve`, `key` | 1 秒おきに 2 回（混雑） | — | `混雑`, `timeout`, `failure` → そのイテレーションが失敗し、ワークフローも失敗する |
| 86 | `引当を戻す(…)` | `connect warehouse StockService/Release`, `idempotent` | — | — | `timeout`, `failure` → そのイテレーションが失敗し、ワークフローも失敗する |
| 95 | `記録する(…)` | 自分で書くタスク, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |
| 98 | `配送 = 配送を手配する(…)` | `flow arrange_delivery.ja.flow` | — | — | `翌日便の空きなし` → 99 行目<br>`timeout`, `failure` → 102 行目 |
| 100 | `配送 = 配送を手配する(…)` | `flow arrange_delivery.ja.flow` | — | — | `翌日便の空きなし`, `timeout`, `failure` → 101 行目 |
| 103 | `箱 = 梱包を待つ(…)` | 自分で書くタスク（応答はコールバック） | — | 2 日 | `timeout` → 104 行目<br>`failure` → ワークフローが失敗する |
| 105 | `知らせる(…)` | 自分で書くタスク | — | — | `宛先なし` → 106 行目<br>`timeout`, `failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 88 | `fail 在庫不足` "注文 {注文.id} には在庫の足りない明細があります" |
| 101 | `fail 配送の手配の失敗` "注文 {注文.id} の配送を手配できませんでした" |
| 102 | `fail 配送の手配の失敗` "注文 {注文.id} の配送を手配できませんでした" |
| 104 | `fail 梱包の遅れ` "二日たっても梱包の知らせがありません" |
| 107 | `succeed 引当 = 結果, 追跡番号 = 配送.追跡番号` |

## 規則

このワークフローが呼ぶ規則を、`rulec doc` が承認する人向けに描いたものです。

<details>
<summary><code>急ぎ</code> · 出荷の急ぎ v1 · <code>../../order/rules/出荷の急ぎ.rule</code></summary>

<!-- rulec 0.22.0 が 出荷の急ぎ.rule (sha256:3f566b643eb8) から生成。これは読み取り専用の資料で、本物は .rule のほうです。編集しても戻せません（§1.6）。 -->
# 規則 出荷の急ぎ v1

注文を急ぎで出すかどうかと、使う便。会員はいつも急ぎ、それ以外は三万円以上なら急ぎ。書き下ろしの例

## 入力

| 名前 | 型 | 範囲 | 注記 |
|---|---|---|---|
| 会員 | bool |  |  |
| 金額 | money[円, incl_tax] | 0円 〜 100万円 |  |

## 出力

| 名前 | 型 | 丸め | 注記 |
|---|---|---|---|
| 急ぎ | bool |  |  |
| 便 | 便（2 値） |  |  |

## 型

列挙は**閉じた**有限集合です。値を足すと、それを見ていない表が完全性検査で割れます。

- **便**（2 値）— 通常便、翌日便

## 表 急ぎの判定（policy unique）

| 列 | 出どころ |
|---|---|
| 会員 | 入力 |
| 金額 | 入力 |
| → 急ぎ | この規則の出力 |
| → 便 | この規則の出力 |

| # | 会員 | 金額 | → 急ぎ（bool） | → 便（便） |
|---|---|---|---|---|
| 1 | true | - | true | 翌日便 |
| 2 | false | >=30000円 | true | 翌日便 |
| 3 | false | <30000円 | false | 通常便 |

**`rulec check` が確かめたこと**

- どの入力の組合せも、いずれかの行に当てはまります（E101 完全性）
- どの入力にも当てはまらない行はありません（E102）
- 二つ以上の行に同時に当てはまる入力はありません（E105 重なり）。行の並べ替えは意味を変えません

## 例（検証済み）

| 会員 | 金額 | → 急ぎ | 便 |
|---|---|---|---|
| true | 1000円 | true | 翌日便 |
| false | 5000円 | false | 通常便 |
| false | 30000円 | true | 翌日便 |

この 3 件は `rulec check` が参照評価器で実行し、すべて宣言どおりの値になりました（E107）。例は**実行される仕様**です。

</details>

