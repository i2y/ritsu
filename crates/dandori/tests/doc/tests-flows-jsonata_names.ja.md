# jsonata_names v1

A variable for each function of JSONata the state machine calls, and the input `type`: no variable hides a function, and the input keeps its key in JSON. `v_count` and `v_states` are variables of their own beside `count` and `states`. Runs on every platform

`tests/flows/jsonata_names.flow` を `dandori doc` で描いたものです。入力は `type: string`・`lines: list[Line]`、出力は `type: string`・`count: int`・`v_count: int`・`states: string`・`v_states: string`・`skus: list[string]`・`notes: list[string]`・`text: string` です。

## flow

```mermaid
flowchart TD
    start(["jsonata_names v1"])
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
    s21["merge = first_line(…)<br>lambda first-line"]
    s22{{"match merge.note"}}
    s23["note(…)<br>lambda note"]
    subgraph L25 ["for map in lines at most 3"]
        s26["lookup = look(…)<br>GET https://items.example.com/v1/items/{sku}/counts/{qty}"]
    end
    subgraph L27 ["let filter = for each in lines at most 3"]
        s27p["yield each.sku"]
    end
    subgraph L28 ["let reduce = for keys in lines at most 3 in parallel · yield join"]
        s29["join = note(…)<br>lambda note"]
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
    s26 -->|"次のイテレーション"| s26
    L25 -->|"最後の項目のあと"| s27p
    s27p -->|"次のイテレーション"| s27p
    L27 -->|"最後の項目のあと"| s29
    L28 -->|"すべてのイテレーションが終わったら"| s30
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s30 ok
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 60 | `merge = first_line(…)` | `lambda first-line`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 62 | `note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 65 | `lookup = look(…)` | `GET https://items.example.com/v1/items/{sku}/counts/{qty}`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 69 | `join = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → そのイテレーションが失敗し、ワークフローも失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 71 | `succeed type = type, count = count, v_count = v_count, states = states, v_states = v_states, skus = filter, notes = reduce, text = "{abs}{append}{ceil}{contains}{encodeUrlComponent}{error}{exists}{floor}{lowercase}{max}{not}{number}{parse}{sift}{substring}{toMillis}"` |

