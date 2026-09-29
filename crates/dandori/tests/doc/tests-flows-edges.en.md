# edges v1

言語のエッジケース：配列の json をリストに入れる、yield した json を集める、並列の中の並列、理由の無い fail

`tests/flows/edges.flow`, drawn by `dandori doc`. Inputs: `箱: list[箱]`, `一つ: json`. Outputs: `集め: list[json]`.

## flow

```mermaid
flowchart TD
    start(["edges v1"])
    s1["置く(…)<br>lambda put"]
    subgraph L2 ["let 集め = for 一箱 in 箱 at most 3 in parallel · yield 答え.付帯"]
        s3["答え = 見る(…)<br>lambda look"]
        subgraph L4 ["for 小箱 in 箱 at most 3 in parallel"]
            s5{{"match 小箱.ラベル"}}
            s6["置く(…)<br>lambda put"]
            s7(["fail NoLabel"])
        end
    end
    subgraph L8 ["let 順に = for 箱2 in 箱 at most 3"]
        s8p["yield 箱2.中身"]
    end
    s9["置く(…)<br>lambda put"]
    s10(["succeed 集め = 集め"])
    start --> s1
    s1 --> s3
    s3 --> s5
    s5 -->|"some ラベル"| s6
    s5 -->|"none"| s7
    L2 -->|"every round done"| s8p
    s8p -->|"next round"| s8p
    L8 -->|"after the last item"| s9
    s9 --> s10
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s10 ok
    class s7 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 28 | `置く(…)` | `lambda put`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 30 | `答え = 見る(…)` | `lambda look`, `idempotent` | — | — | `timeout`, `failure` → the round fails, and then the workflow |
| 33 | `置く(…)` | `lambda put`, `idempotent` | — | — | `timeout`, `failure` → the round fails, and then the workflow |
| 38 | `置く(…)` | `lambda put`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 34 | `fail NoLabel` |
| 39 | `succeed 集め = 集め` |

