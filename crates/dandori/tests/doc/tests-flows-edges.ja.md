# edges v1

言語のエッジケース：配列の json をリストに入れる、yield した json を集める、並列の中の並列、理由の無い fail

`tests/flows/edges.flow` を `dandori doc` で描いたものです。入力は `箱: list[箱]`, `一つ: json`、出力は `集め: list[json]` です。

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
    L2 -->|"すべてのイテレーションが終わったら"| s8p
    s8p -->|"次のイテレーション"| s8p
    L8 -->|"最後の項目のあと"| s9
    s9 --> s10
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s10 ok
    class s7 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 28 | `置く(…)` | `lambda put`, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |
| 30 | `答え = 見る(…)` | `lambda look`, `idempotent` | — | — | `timeout`, `failure` → そのイテレーションが失敗し、ワークフローも失敗する |
| 33 | `置く(…)` | `lambda put`, `idempotent` | — | — | `timeout`, `failure` → そのイテレーションが失敗し、ワークフローも失敗する |
| 38 | `置く(…)` | `lambda put`, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 34 | `fail NoLabel` |
| 39 | `succeed 集め = 集め` |

