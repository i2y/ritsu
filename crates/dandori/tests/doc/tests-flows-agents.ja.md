# 経費 v1

エージェントの端の振る舞い：リスト・単位・時刻・無いことがある値・範囲のある数を持つ答え、答えを読まない呼び出し、並列の回の中のエージェント、大文字と小文字の違う列挙の値で答える Claude のエージェント、OpenAI のほかの Open Responses のサーバーで読むエージェント

`tests/flows/agents.flow` を `dandori doc` で描いたものです。入力は `写したち: list[string]`、出力は `領収書たち: list[領収書]`, `急ぎ: 急ぎ`, `印: list[急ぎ]` です。

## flow

```mermaid
flowchart TD
    start(["経費 v1"])
    s1["確かめる(…)<br>agent openai · gpt-5.4-mini<br>timeout 30 seconds"]
    subgraph L2 ["let 領収書たち = for 写し in 写したち at most 3 in parallel · yield r"]
        s3["r = 読み取る(…)<br>agent · gpt-oss:20b · https://llm.example.com/v1<br>retry 1 times every 5 seconds"]
    end
    s4["仕 = 仕分ける(…)<br>agent claude · claude-sonnet-5<br>retry 1 times every 5 seconds · timeout 1 minute"]
    s5{{"match 仕.急ぎ"}}
    s6(["succeed 領収書たち = 領収書たち, 急ぎ = Urgent, 印 = 仕.印"])
    s7(["succeed 領収書たち = 領収書たち, 急ぎ = 仕.急ぎ, 印 = 仕.印"])
    start --> s1
    s1 --> s3
    L2 -->|"全部の回が終わったら"| s4
    s4 --> s5
    s5 -->|"Urgent"| s6
    s5 -->|"low, normal"| s7
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s6,s7 ok
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 57 | `確かめる(…)` | `agent openai · gpt-5.4-mini` | — | 30 秒 | `timeout`, `failure` → ワークフローが失敗する |
| 59 | `r = 読み取る(…)` | `agent · gpt-oss:20b · https://llm.example.com/v1` | 5 秒おきに 1 回（failure, timeout） | — | `timeout`, `failure` → その回が失敗し、ワークフローも失敗する |
| 61 | `仕 = 仕分ける(…)` | `agent claude · claude-sonnet-5` | 5 秒おきに 1 回（failure, timeout） | 1 分 | `timeout`, `failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 63 | `succeed 領収書たち = 領収書たち, 急ぎ = Urgent, 印 = 仕.印` |
| 64 | `succeed 領収書たち = 領収書たち, 急ぎ = 仕.急ぎ, 印 = 仕.印` |

