# variable_names v1

Variables that the code of a target would write as one name, and variables named as the code's own: class and class_, _x and v_x, type and type_, and names that begin with dd_. The code of each target keeps them apart, and the flow runs on every platform

`tests/flows/variable_names.flow` を `dandori doc` で描いたものです。入力は `id: string`・`_x: int`、出力は `text: string`・`counts: string` です。

## flow

```mermaid
flowchart TD
    start(["variable_names v1"])
    s1["class = note(…)<br>lambda note"]
    s2["class_ = note(…)<br>lambda note"]
    s3["v_x = count(…)<br>lambda count"]
    s4["type = note(…)<br>lambda note"]
    s5["type_ = note(…)<br>lambda note"]
    s6["dd_error = count(…)<br>lambda count"]
    s7["dd_v = count(…)<br>lambda count"]
    s8["let dd_x = 3"]
    s9(["succeed text = #quot;{class} {class_} {type} {type_}#quot;, count…"])
    start --> s1
    s1 --> s2
    s2 --> s3
    s3 --> s4
    s4 --> s5
    s5 --> s6
    s6 --> s7
    s7 --> s8
    s8 --> s9
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s9 ok
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 22 | `class = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 23 | `class_ = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 25 | `v_x = count(…)` | `lambda count`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 27 | `type = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 28 | `type_ = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 30 | `dd_error = count(…)` | `lambda count`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 31 | `dd_v = count(…)` | `lambda count`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 33 | `succeed text = "{class} {class_} {type} {type_}", counts = "{_x} {v_x} {dd_error} {dd_v} {dd_x}"` |

