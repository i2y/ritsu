# fulfillment v1

Reserve stock for each line of an order, arrange the delivery, wait for the warehouse to pack it, and then tell the customer. The lines are reserved side by side, and when one is short, what was reserved is released. Written for Temporal: the warehouse's calls are activities dandori writes, by Connect; a workflow of the delivery team, on its own task queue, arranges the delivery (a child workflow, arrange_delivery.flow), and when it finds no next-day van the standard carrier is asked; and the packing request, the notice and the audit log are activities you write, the packing crew answering the callback by the update the generated client sends

`examples/fulfillment/temporal/fulfillment.flow` を `dandori doc` で描いたものです。入力は `order: Order`、出力は `reservations: list[Reservation]`, `tracking_number: string` です。

## flow

```mermaid
flowchart TD
    start(["fulfillment v1"])
    s1[["decision = urgency(…)<br>rule urgency.rule"]]
    subgraph L2 ["let results = for line in order.lines at most 50 in parallel, 10 at a time · yield r"]
        s3["r = reserve_stock(…)<br>connect warehouse StockService/Reserve<br>retry 2 times every 1 second on busy"]
    end
    s4["let any_short = false"]
    subgraph L5 ["for one in results at most 50"]
        s6{{"match one.stock"}}
        s7["let any_short = true"]
    end
    s9{{"match any_short"}}
    subgraph L10 ["for taken in results at most 50 in parallel"]
        s11{{"match taken.id"}}
        s12["release_stock(…)<br>connect warehouse StockService/Release"]
    end
    s14(["fail OutOfStock<br>#quot;Order {order.id} has lines the stock is short …"])
    s16["let recipient = none"]
    s17{{"match order.gift"}}
    s18["let recipient = gift.recipient"]
    s19{{"match gift.message"}}
    s20["audit(…)<br>自分で書くタスク"]
    s23["delivery = arrange_delivery(…)<br>flow arrange_delivery.flow"]
    s24["delivery = arrange_delivery(…)<br>flow arrange_delivery.flow"]
    s25(["fail DeliveryFailed<br>#quot;Could not arrange the delivery of order {order…"])
    s26(["fail DeliveryFailed<br>#quot;Could not arrange the delivery of order {order…"])
    s27[/"packed = wait_for_packing(…)<br>自分で書くタスク（応答はコールバック）<br>timeout 2 days"/]
    s28(["fail PackingLate<br>#quot;No word of the packing in two days#quot;"])
    s29["notify(…)<br>自分で書くタスク"]
    s31(["succeed reservations = results, tracking_number = deliv…"])
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
    s17 -->|"some gift"| s18
    s18 --> s19
    s19 -->|"some words"| s20
    s20 --> s23
    s19 -->|"none"| s23
    s17 -->|"none"| s23
    s23 -.->|"on NoVan"| s24
    s24 -.->|"on failure"| s25
    s23 -.->|"on failure"| s26
    s23 --> s27
    s24 --> s27
    s27 -.->|"on timeout"| s28
    s27 --> s29
    s29 --> s31
    s29 -.->|"on no_recipient"| s31
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s31 ok
    class s14,s25,s26,s28 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 79 | `decision = urgency(…)` | 規則 `urgency.rule` | 2 回（1 秒後と 2 秒後、failure） | — | `timeout`, `failure` → ワークフローが失敗する |
| 81 | `r = reserve_stock(…)` | `connect warehouse StockService/Reserve`, `key` | 1 秒おきに 2 回（busy） | — | `busy`, `timeout`, `failure` → そのイテレーションが失敗し、ワークフローも失敗する |
| 92 | `release_stock(…)` | `connect warehouse StockService/Release`, `idempotent` | — | — | `timeout`, `failure` → そのイテレーションが失敗し、ワークフローも失敗する |
| 101 | `audit(…)` | 自分で書くタスク, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |
| 104 | `delivery = arrange_delivery(…)` | `flow arrange_delivery.flow` | — | — | `NoVan` → 105 行目<br>`timeout`, `failure` → 108 行目 |
| 106 | `delivery = arrange_delivery(…)` | `flow arrange_delivery.flow` | — | — | `NoVan`, `timeout`, `failure` → 107 行目 |
| 109 | `packed = wait_for_packing(…)` | 自分で書くタスク（応答はコールバック） | — | 2 日 | `timeout` → 110 行目<br>`failure` → ワークフローが失敗する |
| 111 | `notify(…)` | 自分で書くタスク | — | — | `no_recipient` → 112 行目<br>`timeout`, `failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 94 | `fail OutOfStock` "Order {order.id} has lines the stock is short of" |
| 107 | `fail DeliveryFailed` "Could not arrange the delivery of order {order.id}" |
| 108 | `fail DeliveryFailed` "Could not arrange the delivery of order {order.id}" |
| 110 | `fail PackingLate` "No word of the packing in two days" |
| 113 | `succeed reservations = results, tracking_number = delivery.tracking_number` |

