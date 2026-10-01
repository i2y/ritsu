# 出荷の受付 v1

サービスを実装する（どのプラットフォームでも）：protos/shipping.proto のサービスの入口で、入力とコールバックの応答に、protobuf の JSON が省くゼロ値を埋めてから読む。箱のリストの中の品のリスト、中のメッセージ、ゼロ値が値でもある列挙を通る。値が設定されていない google.protobuf.Value は省かれるので、json の入力は無いことがあり、無ければ null として読む。出力は無く、実行は {} で終わる（protobuf の JSON を読む側は、null からメッセージを読まない）

`tests/flows/service.flow`, drawn by `dandori doc`. Inputs: `注文ID: string`, `箱: list[箱]`, `宛先: 宛先`, `区分: 受付.Kind`, `付帯: json`. It implements `shipping.v1.ShippingService` (`protos/shipping.proto`).

## flow

```mermaid
flowchart TD
    start(["出荷の受付 v1"])
    subgraph L1 ["for 一箱 in 箱 at most 3"]
        subgraph L2 ["for 一品 in 一箱.品 at most 3"]
            s3["記録する(…)<br>lambda shipping-log"]
        end
    end
    s4[/"済み = 梱包を待つ(…)<br>lambda packing · callback<br>timeout 1 day"/]
    s5(["fail 梱包の遅れ<br>#quot;一日たっても梱包の知らせがありません#quot;"])
    s6{{"match 済み.検品済み"}}
    s8(["fail 検品漏れ<br>#quot;注文 {注文ID} の検品が済んでいません#quot;"])
    s9{{"match 区分"}}
    s10["知らせる(…)<br>lambda shipping-notice"]
    s11(["succeed"])
    fin(["end: succeeds"])
    start --> s3
    s3 -->|"next round"| s3
    L2 -->|"after the last item"| s3
    L1 -->|"after the last item"| s4
    s4 -.->|"on timeout"| s5
    s4 --> s6
    s6 -->|"false"| s8
    s6 -->|"true"| s9
    s9 -->|"normal"| s10
    s9 -->|"express"| s11
    s10 --> fin
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s11 ok
    class s5,s8 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Service

What each method of the service does to a run.

| Method | What it does | Task |
|---|---|---|
| `Ship` | starts a run; it can fail with `梱包の遅れ`, `検品漏れ` | — |
| `AnswerPacking` | answers the callback | `梱包を待つ` |

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 48 | `記録する(…)` | `lambda shipping-log`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 49 | `済み = 梱包を待つ(…)` | `lambda packing · callback` | — | 1 day | `timeout` → line 50<br>`failure` → the workflow fails |
| 55 | `知らせる(…)` | `lambda shipping-notice`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 50 | `fail 梱包の遅れ` "一日たっても梱包の知らせがありません" |
| 53 | `fail 検品漏れ` "注文 {注文ID} の検品が済んでいません" |
| 56 | `succeed` |
| 56 | the flow runs to its end, and the workflow succeeds |

