# JSONataの名前 v1

ステートマシンが呼ぶ JSONata の関数ごとの変数と、入力 `type` です。変数は関数を隠さず、入力の JSON のキーは書いた名前のままです。`v_count` と `v_states` は、`count` と `states` の隣の別の変数です。どのプラットフォームでも動きます

`tests/flows/jsonata_names.ja.flow`, drawn by `dandori doc`. Inputs: `type: string`, `明細の列: list[明細]`. Outputs: `type: string`, `count: int`, `v_count: int`, `states: string`, `v_states: string`, `品番の列: list[string]`, `注記の列: list[string]`, `文: string`.

## flow

```mermaid
flowchart TD
    start(["JSONataの名前 v1"])
    s1["let count = 1"]
    s2["let v_count = 2"]
    s3["let states = #quot;states#quot;"]
    s4["let v_states = #quot;v_states#quot;"]
    s5["let abs = #quot;abs#quot;"]
    s6["let append = #quot;append#quot;"]
    s7["let ceil = #quot;ceil#quot;"]
    s8["let contains = #quot;contains#quot;"]
    s9["let encodeUrlComponent = #quot;encodeUrlComponent#quot;"]
    s10["let error = #quot;error#quot;"]
    s11["let exists = #quot;exists#quot;"]
    s12["let floor = #quot;floor#quot;"]
    s13["let lowercase = #quot;lowercase#quot;"]
    s14["let max = #quot;max#quot;"]
    s15["let not = #quot;not#quot;"]
    s16["let number = #quot;number#quot;"]
    s17["let parse = #quot;parse#quot;"]
    s18["let sift = #quot;sift#quot;"]
    s19["let substring = #quot;substring#quot;"]
    s20["let toMillis = #quot;toMillis#quot;"]
    s21["merge = 最初の明細(…)<br>lambda first-line"]
    s22{{"match merge.注記"}}
    s23["書く(…)<br>lambda note"]
    subgraph L25 ["for map in 明細の列 at most 3"]
        s26["lookup = 見る(…)<br>GET https://items.example.com/v1/items/{品番}/counts/{数}"]
    end
    subgraph L27 ["let filter = for each in 明細の列 at most 3"]
        s27p["yield each.品番"]
    end
    subgraph L28 ["let reduce = for keys in 明細の列 at most 3 in parallel · yield join"]
        s29["join = 書く(…)<br>lambda note"]
    end
    s30(["succeed type = type, count = count, v_count = v_count, …"])
    start --> s1
    s1 --> s2
    s2 --> s3
    s3 --> s4
    s4 --> s5
    s5 --> s6
    s6 --> s7
    s7 --> s8
    s8 --> s9
    s9 --> s10
    s10 --> s11
    s11 --> s12
    s12 --> s13
    s13 --> s14
    s14 --> s15
    s15 --> s16
    s16 --> s17
    s17 --> s18
    s18 --> s19
    s19 --> s20
    s20 --> s21
    s21 --> s22
    s22 -->|"some string"| s23
    s23 --> s26
    s22 -->|"none"| s26
    s26 -->|"next round"| s26
    L25 -->|"after the last item"| s27p
    s27p -->|"next round"| s27p
    L27 -->|"after the last item"| s29
    L28 -->|"every round done"| s30
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s30 ok
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 60 | `merge = 最初の明細(…)` | `lambda first-line`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 62 | `書く(…)` | `lambda note`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 65 | `lookup = 見る(…)` | `GET https://items.example.com/v1/items/{品番}/counts/{数}`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 69 | `join = 書く(…)` | `lambda note`, `idempotent` | — | — | `timeout`, `failure` → the round fails, and then the workflow |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 71 | `succeed type = type, count = count, v_count = v_count, states = states, v_states = v_states, 品番の列 = filter, 注記の列 = reduce, 文 = "{abs}{append}{ceil}{contains}{encodeUrlComponent}{error}{exists}{floor}{lowercase}{max}{not}{number}{parse}{sift}{substring}{toMillis}"` |

