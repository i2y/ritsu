# 列挙の値 v1

型を書いていないところに列挙の値を書く。for が回す並び（一回ずつと、同時に）、型を書かない変数に入れる並び（ほかの要素が型を決める）、型を書かない変数に入れる値。どれも、その名前を値に持つただ一つの列挙の値で、どの出力先のコードもその型を書く。enum_values.flow の日本語の版

`tests/flows/enum_values.ja.flow`, drawn by `dandori doc`. Inputs: `申請番号: string`, `最初: 役割`. Outputs: `尋ねた役割: list[役割]`, `最後: 役割`.

## flow

```mermaid
flowchart TD
    start(["列挙の値 v1"])
    subgraph L1 ["for 相手 in [上長, 経理] at most 2"]
        s2["尋ねる(…)<br>lambda ask"]
    end
    subgraph L3 ["let 尋ねた役割 = for 宛先 in [法務, 上長] at most 2 in parallel · yield 宛先"]
        s4["尋ねる(…)<br>lambda ask"]
    end
    s5["let 確認の順 = [法務, 最初]"]
    subgraph L6 ["for 確認者 in 確認の順 at most 2"]
        s7["尋ねる(…)<br>lambda ask"]
    end
    s8["let 最後 = 経理"]
    s9(["succeed 尋ねた役割 = 尋ねた役割, 最後 = 最後"])
    start --> s2
    s2 -->|"next round"| s2
    L1 -->|"after the last item"| s4
    L3 -->|"every round done"| s5
    s5 --> s7
    s7 -->|"next round"| s7
    L6 -->|"after the last item"| s8
    s8 --> s9
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s9 ok
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 19 | `尋ねる(…)` | `lambda ask` | — | — | `timeout`, `failure` → the workflow fails |
| 21 | `尋ねる(…)` | `lambda ask` | — | — | `timeout`, `failure` → the round fails, and then the workflow |
| 25 | `尋ねる(…)` | `lambda ask` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 27 | `succeed 尋ねた役割 = 尋ねた役割, 最後 = 最後` |

