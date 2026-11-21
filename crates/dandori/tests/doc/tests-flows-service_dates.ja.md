# service_dates v1

Rent a car through a service that holds its days as google.type.Date: the run reads the days of the rental and of the drivers' licences as dates, hands them to the desk, and waits for the desk to answer with the day the car came back

`tests/flows/service_dates.flow` を `dandori doc` で描いたものです。入力は `car: string`・`pickup: date`・`dropoff: date`・`closed: list[date]`・`driver: Driver`・`others: list[Driver]` です。このワークフローは `rentals.v1.RentalService`（`protos/rentals.proto`）を実装します。

## flow

```mermaid
flowchart TD
    start(["service_dates v1"])
    s1["hand_over(…)<br>lambda rental-desk"]
    subgraph L2 ["for o in others at most 3"]
        s3["hand_over(…)<br>lambda rental-desk"]
    end
    s4[/"back = wait_for_return(…)<br>lambda rental-return · callback<br>timeout 7 days"/]
    s5(["fail late<br>#quot;the car was not back within 7 days#quot;"])
    s6["close(…)<br>lambda rental-close"]
    fin(["終わり（成功）"])
    start --> s1
    s1 --> s3
    s3 -->|"次のイテレーション"| s3
    L2 -->|"最後の項目のあと"| s4
    s4 -.->|"on timeout"| s5
    s4 --> s6
    s6 --> fin
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s5 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## サービス

サービスのメソッドと、それぞれが実行に対して何をするかです。

| メソッド | 何をするか | タスク |
|---|---|---|
| `Rent` | 実行を始めます。失敗の名前は `late` です | — |
| `AnswerReturn` | コールバックに応答します | `wait_for_return` |

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 43 | `hand_over(…)` | `lambda rental-desk`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 45 | `hand_over(…)` | `lambda rental-desk`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 46 | `back = wait_for_return(…)` | `lambda rental-return · callback` | — | ID を渡すのに 1 分、そのあと 7 日 | `timeout` → 47 行目<br>`failure` → ワークフローが失敗する |
| 48 | `close(…)` | `lambda rental-close`・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 47 | `fail late` "the car was not back within 7 days" |
| 48 | flow が最後まで走り、ワークフローは成功する |

