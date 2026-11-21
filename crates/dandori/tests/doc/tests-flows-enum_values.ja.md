# enum_values v1

Writes values of an enum where nothing says their type: the lists a for goes through, one round after another and in parallel, a list put in a variable with no type, whose other item says its type, and a value put in a variable with no type. Each is a value of the one enum that has it, and the code of every platform writes its type

`tests/flows/enum_values.flow` を `dandori doc` で描いたものです。入力は `claim_id: string`・`first: role`、出力は `asked: list[role]`・`last: role` です。

## flow

```mermaid
flowchart TD
    start(["enum_values v1"])
    subgraph L1 ["for r in [manager, accounting] at most 2"]
        s2["ask(…)<br>lambda ask"]
    end
    subgraph L3 ["let asked = for party in [legal, manager] at most 2 in parallel · yield party"]
        s4["ask(…)<br>lambda ask"]
    end
    s5["let reviewers = [legal, first]"]
    subgraph L6 ["for reviewer in reviewers at most 2"]
        s7["ask(…)<br>lambda ask"]
    end
    s8["let last = accounting"]
    s9(["succeed asked = asked, last = last"])
    start --> s2
    s2 -->|"次のイテレーション"| s2
    L1 -->|"最後の項目のあと"| s4
    L3 -->|"すべてのイテレーションが終わったら"| s5
    s5 --> s7
    s7 -->|"次のイテレーション"| s7
    L6 -->|"最後の項目のあと"| s8
    s8 --> s9
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s9 ok
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 19 | `ask(…)` | `lambda ask` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 21 | `ask(…)` | `lambda ask` | — | — | `timeout`・`failure` → そのイテレーションが失敗し、ワークフローも失敗する |
| 25 | `ask(…)` | `lambda ask` | — | — | `timeout`・`failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 27 | `succeed asked = asked, last = last` |

