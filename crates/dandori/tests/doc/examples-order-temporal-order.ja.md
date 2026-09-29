# ship_order v1

For an order in the warehouse's system, ask for the payment, have it shipped, and wait for word of the delivery. The warehouse's system keeps the order's state, which moves as rulec's rule order_state says. Written for Temporal: the warehouse's calls are activities dandori writes, and the notice is an activity you write; the carrier's system tells the workflow of the order about the delivery, by the order's id (an event); and an order the shop cancels on the way is cancelled at the warehouse too (on cancel). The loop over the reminders goes on in a new run when its history grows long

`examples/order/temporal/order.flow` を `dandori doc` で描いたものです。入力は `order_id: string`、出力は `carrier: urgency.carrier` です。

## flow

```mermaid
flowchart TD
    start(["ship_order v1"])
    s1["order ← get_order(…)<br>GET https://warehouse.example.com/v1/orders/{id}<br>observes · retry 3 times every 1 second on busy"]
    subgraph L2 ["repeat at most 3 times"]
        s3{{"match order.state"}}
        s4["r = notify(…)<br>自分で書くタスク<br>retry 2 times every 5 seconds"]
        s5(["fail NoContact<br>leaving order"])
        s6("wait 1 day")
        s7["order ← get_order(…)<br>GET https://warehouse.example.com/v1/orders/{id}<br>observes · retry 3 times every 1 second on busy"]
        s8(["break"])
    end
    s9{{"match order.state"}}
    s10["order ← request_cancel(…)<br>POST https://warehouse.example.com/v1/orders/{id}/cancellations<br>sends cancel"]
    s12(["fail NotPaid<br>#quot;No payment came in three days#quot;"])
    s13(["fail Canceled<br>#quot;The order was canceled#quot;"])
    s14(["fail AlreadyShipped<br>#quot;The order has shipped already#quot;<br>leaving order"])
    s16[["decision = urgency(…)<br>rule urgency.rule"]]
    s17["order ← request_shipment(…)<br>POST https://warehouse.example.com/v1/orders/{id}/shipments<br>sends ship"]
    s18(["fail Canceled<br>#quot;The order was canceled before it shipped#quot;"])
    s19{{"match decision.urgent"}}
    s20["n = notify(…)<br>自分で書くタスク<br>retry 2 times every 5 seconds"]
    s22[/"order ← delivered()<br>event<br>observes · timeout 7 days"/]
    s23(["fail DeliveryLate<br>#quot;No word of the delivery in seven days#quot;<br>leaving order"])
    s24{{"match order.state"}}
    s25(["succeed carrier = decision.carrier"])
    s26(["fail DeliveryLate<br>#quot;Still shipped after word of the delivery#quot;<br>leaving order"])
    start --> s1
    s1 --> s3
    s3 -->|"received"| s4
    s4 -.->|"on no_recipient"| s5
    s4 --> s6
    s6 --> s7
    s3 -->|"paid, shipped, delivered, cancelled"| s8
    s7 -->|"次の回"| s3
    L2 -->|"3 回終えたら"| s9
    s8 --> s9
    s9 -->|"received"| s10
    s10 --> s12
    s10 -.->|"on conflict"| s12
    s9 -->|"cancelled"| s13
    s9 -->|"shipped, delivered"| s14
    s9 -->|"paid"| s16
    s16 --> s17
    s17 -.->|"on conflict"| s18
    s17 --> s19
    s19 -->|"true"| s20
    s20 --> s22
    s19 -->|"false"| s22
    s22 -.->|"on timeout"| s23
    s22 --> s24
    s24 -->|"delivered"| s25
    s24 -->|"shipped"| s26
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s25 ok
    class s5,s12,s13,s14,s18,s23,s26 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## on failure

呼び出しが失敗し、そのエラーをその場で処理しないときに走ります（61・65・68・72・78・79・82・84 行目の呼び出しから）。最後まで走ると、ワークフローはそのエラーで失敗します。

```mermaid
flowchart TD
    onf(["on failure"])
    s27(["fail Stopped<br>#quot;Stopped on the way; the order stays in the war…<br>leaving order"])
    onf --> s27
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s27 bad
```

## on cancel

ワークフローがキャンセルされると、そのとき待っている呼び出しや wait から、ここに来ます。最後まで走ると、ワークフローはキャンセルで終わります。

```mermaid
flowchart TD
    onc(["on cancel"])
    s28{{"match order.state"}}
    s30["order ← request_cancel(…)<br>POST https://warehouse.example.com/v1/orders/{id}/cancellations<br>sends cancel"]
    s32(["fail CancelFailed<br>#quot;Could not cancel order {order.id} at the wareh…<br>leaving order"])
    s33(["fail ShippedAlready<br>#quot;Order {order.id} has shipped already#quot;<br>leaving order"])
    oncEnd(["キャンセルで終わる"])
    onc --> s28
    s28 -->|"received, paid"| s30
    s30 -.->|"on failure"| s32
    s28 -->|"shipped"| s33
    s28 -->|"cancelled, none"| oncEnd
    s30 --> oncEnd
    s30 -.->|"on conflict"| oncEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s32,s33 bad
```

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき | 呼び出しのあとの案件 |
|---:|---|---|---|---|---|---|
| 61 | `order ← get_order(…)` | `GET https://warehouse.example.com/v1/orders/{id}`, `observes`, `idempotent` | 1 秒おきに 3 回（busy） | — | `busy`, `timeout`, `failure` → `on failure` | `order`: `received`, `paid`, `shipped`, `delivered`, `cancelled` |
| 65 | `r = notify(…)` | 自分で書くタスク, `key` | 5 秒おきに 2 回（failure, timeout） | — | `no_recipient` → 66 行目<br>`timeout`, `failure` → `on failure` | — |
| 68 | `order ← get_order(…)` | `GET https://warehouse.example.com/v1/orders/{id}`, `observes`, `idempotent` | 1 秒おきに 3 回（busy） | — | `busy`, `timeout`, `failure` → `on failure` | `order`: `received`, `paid`, `cancelled` |
| 72 | `order ← request_cancel(…)` | `POST https://warehouse.example.com/v1/orders/{id}/cancellations`, `sends cancel`, `key` | — | — | `conflict` → 73 行目<br>`timeout`, `failure` → `on failure` | `order`: `cancelled` |
| 78 | `decision = urgency(…)` | 規則 `urgency.rule` | 2 回（1 秒後と 2 秒後、failure） | — | `timeout`, `failure` → `on failure` | — |
| 79 | `order ← request_shipment(…)` | `POST https://warehouse.example.com/v1/orders/{id}/shipments`, `sends ship`, `key` | — | — | `conflict` → 80 行目<br>`timeout`, `failure` → `on failure` | `order`: `shipped` |
| 82 | `n = notify(…)` | 自分で書くタスク, `key` | 5 秒おきに 2 回（failure, timeout） | — | `no_recipient`, `timeout`, `failure` → `on failure` | — |
| 84 | `order ← delivered()` | `event`, `observes` | — | 7 日 | `timeout` → 85 行目<br>`failure` → `on failure` | `order`: `shipped`, `delivered` |
| 99 | `order ← request_cancel(…)` | `POST https://warehouse.example.com/v1/orders/{id}/cancellations`, `sends cancel`, `key` | — | — | `conflict` → 100 行目<br>`timeout`, `failure` → 101 行目 | `order`: `cancelled` |

## 終わり方

ワークフローの終わり方のすべてと、そのとき各案件がとりうる状態です。外部のサービスで起きるイベントも含めています。

| 行 | 終わり方 | `order` |
|---:|---|---|
| 66 | `fail NoContact` `leaving order` | そのまま引き渡す: `received`, `paid`, `cancelled` |
| 74 | `fail NotPaid` "No payment came in three days" | `cancelled` |
| 75 | `fail Canceled` "The order was canceled" | `cancelled` |
| 76 | `fail AlreadyShipped` "The order has shipped already" `leaving order` | そのまま引き渡す: `shipped`, `delivered` |
| 80 | `fail Canceled` "The order was canceled before it shipped" | `cancelled` |
| 85 | `fail DeliveryLate` "No word of the delivery in seven days" `leaving order` | そのまま引き渡す: `shipped`, `delivered` |
| 87 | `succeed carrier = decision.carrier` | `delivered` |
| 88 | `fail DeliveryLate` "Still shipped after word of the delivery" `leaving order` | そのまま引き渡す: `shipped`, `delivered` |
| 91 | `fail Stopped` "Stopped on the way; the order stays in the warehouse's system as it is" `leaving order` | そのまま引き渡す: 始まっていないか、`received`, `paid`, `shipped`, `delivered`, `cancelled` |
| 101 | `fail CancelFailed` "Could not cancel order {order.id} at the warehouse" `leaving order` | そのまま引き渡す: `received`, `paid`, `cancelled` |
| 102 | `fail ShippedAlready` "Order {order.id} has shipped already" `leaving order` | そのまま引き渡す: `shipped`, `delivered` |
| 102 | `on cancel` が最後まで走り、ワークフローはキャンセルで終わる | 始まっていないか、`cancelled` |

