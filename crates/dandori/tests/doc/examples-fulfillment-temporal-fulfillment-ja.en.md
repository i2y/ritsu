# 引当と発送 v1

注文の明細ごとに在庫を引き当て、配送を手配し、倉庫の梱包を待ってから、お客さまに知らせる。明細は並べて引き当て、足りない明細があれば、引き当てたぶんを戻す。Temporal 向けの版：倉庫の呼び出しは dandori が Connect で生成するアクティビティ。配送チームのワークフローが専用のタスクキューで配送を手配し（子ワークフロー、arrange_delivery.ja.flow）、翌日便の車がなければ通常便で頼み直す。梱包の依頼、お知らせ、記録は自分で書くアクティビティで、梱包の担当者は、生成したクライアントが送る Update でコールバックに応答する

`examples/fulfillment/temporal/fulfillment.ja.flow`, drawn by `dandori doc`. Inputs: `注文: 注文`. Outputs: `引当: list[引当]`, `追跡番号: string`.

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
    s20["記録する(…)<br>a task you write"]
    s23["配送 = 配送を手配する(…)<br>flow arrange_delivery.ja.flow"]
    s24["配送 = 配送を手配する(…)<br>flow arrange_delivery.ja.flow"]
    s25(["fail 配送の手配の失敗<br>#quot;注文 {注文.id} の配送を手配できませんでした#quot;"])
    s26(["fail 配送の手配の失敗<br>#quot;注文 {注文.id} の配送を手配できませんでした#quot;"])
    s27[/"箱 = 梱包を待つ(…)<br>a task you write, answered by a callback<br>timeout 2 days"/]
    s28(["fail 梱包の遅れ<br>#quot;二日たっても梱包の知らせがありません#quot;"])
    s29["知らせる(…)<br>a task you write"]
    s31(["succeed 引当 = 結果, 追跡番号 = 配送.追跡番号"])
    start --> s1
    s1 --> s3
    L2 -->|"every round done"| s4
    s4 --> s6
    s6 -->|"short"| s7
    s7 -->|"next round"| s6
    s6 -->|"secured"| s6
    L5 -->|"after the last item"| s9
    s9 -->|"true"| s11
    s11 -->|"some id"| s12
    L10 -->|"every round done"| s14
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

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 79 | `判定 = 急ぎ(…)` | rule `出荷の急ぎ.rule` | 2 times, after 1 second and 2 (failure) | — | `timeout`, `failure` → the workflow fails |
| 81 | `答え = 在庫を引き当てる(…)` | `connect warehouse StockService/Reserve`, `key` | 2 times every 1 second (混雑) | — | `混雑`, `timeout`, `failure` → the round fails, and then the workflow |
| 92 | `引当を戻す(…)` | `connect warehouse StockService/Release`, `idempotent` | — | — | `timeout`, `failure` → the round fails, and then the workflow |
| 101 | `記録する(…)` | a task you write, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 104 | `配送 = 配送を手配する(…)` | `flow arrange_delivery.ja.flow` | — | — | `翌日便の空きなし` → line 105<br>`timeout`, `failure` → line 108 |
| 106 | `配送 = 配送を手配する(…)` | `flow arrange_delivery.ja.flow` | — | — | `翌日便の空きなし`, `timeout`, `failure` → line 107 |
| 109 | `箱 = 梱包を待つ(…)` | a task you write, answered by a callback | — | 2 days | `timeout` → line 110<br>`failure` → the workflow fails |
| 111 | `知らせる(…)` | a task you write | — | — | `宛先なし` → line 112<br>`timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 94 | `fail 在庫不足` "注文 {注文.id} には在庫の足りない明細があります" |
| 107 | `fail 配送の手配の失敗` "注文 {注文.id} の配送を手配できませんでした" |
| 108 | `fail 配送の手配の失敗` "注文 {注文.id} の配送を手配できませんでした" |
| 110 | `fail 梱包の遅れ` "二日たっても梱包の知らせがありません" |
| 113 | `succeed 引当 = 結果, 追跡番号 = 配送.追跡番号` |

