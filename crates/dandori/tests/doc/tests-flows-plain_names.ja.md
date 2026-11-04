# plain_names v1

Names the generated code has no claim on, though they read like its own: the nouns a Go package might have, and the builtins of Python and the globals of JavaScript the code never reads, as types and variables. Runs on every platform

`tests/flows/plain_names.flow` を `dandori doc` で描いたものです。入力は `id: string`、出力は `text: string`・`everything: Everything` です。

## flow

```mermaid
flowchart TD
    start(["plain_names v1"])
    s1["everything = gather(…)<br>lambda gather"]
    s2["bytes = note(…)<br>lambda note"]
    s3["callable = note(…)<br>lambda note"]
    s4["filter = note(…)<br>lambda note"]
    s5["format = note(…)<br>lambda note"]
    s6["frozenset = note(…)<br>lambda note"]
    s7["hasattr = note(…)<br>lambda note"]
    s8["hash = note(…)<br>lambda note"]
    s9["iter = note(…)<br>lambda note"]
    s10["map = note(…)<br>lambda note"]
    s11["object = note(…)<br>lambda note"]
    s12["open = note(…)<br>lambda note"]
    s13["print = note(…)<br>lambda note"]
    s14["repr = note(…)<br>lambda note"]
    s15["reversed = note(…)<br>lambda note"]
    s16["setattr = note(…)<br>lambda note"]
    s17["slice = note(…)<br>lambda note"]
    s18["sum = note(…)<br>lambda note"]
    s19["vars = note(…)<br>lambda note"]
    s20(["succeed text = #quot;{bytes}{callable}{filter}{format}{froze…"])
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
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s20 ok
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 114 | `everything = gather(…)` | `lambda gather`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 115 | `bytes = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 116 | `callable = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 117 | `filter = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 118 | `format = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 119 | `frozenset = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 120 | `hasattr = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 121 | `hash = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 122 | `iter = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 123 | `map = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 124 | `object = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 125 | `open = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 126 | `print = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 127 | `repr = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 128 | `reversed = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 129 | `setattr = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 130 | `slice = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 131 | `sum = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 132 | `vars = note(…)` | `lambda note`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 133 | `succeed text = "{bytes}{callable}{filter}{format}{frozenset}{hasattr}{hash}{iter}{map}{object}{open}{print}{repr}{reversed}{setattr}{slice}{sum}{vars}", everything = everything` |

