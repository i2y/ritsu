# 請求 v1

注文の品を支払期日まで押さえ、支払われたら出荷し、支払われなければ戻す

`examples/invoice/invoice.ja.flow`, drawn by `dandori doc`. Inputs: `受注: 注文`. Outputs: `結果: 終わり方`, `期日: date?`.

## flow

```mermaid
flowchart TD
    start(["請求 v1"])
    s1["押さえ ← 引き当てる(…)<br>book 在庫.引当.hold<br>starts"]
    s2(["succeed 結果 = 在庫切れ, 期日 = none"])
    s3[["期限 = 支払条件.支払日(…)<br>dates 支払条件.cal"]]
    s4("wait until 期限.at")
    s5["入金 = 支払を確かめる(…)<br>GET https://payments.example.com/orders/{注文}/payment<br>retry 2 times every 10 seconds"]
    s6{{"match 入金.済み"}}
    s7["押さえ ← 出荷する(…)<br>book 在庫.引当.post<br>sends post"]
    s8(["succeed 結果 = 未払い, 期日 = 期限.day"])
    s9(["succeed 結果 = 出荷済, 期日 = 期限.day"])
    s10["押さえ ← 戻す(…)<br>book 在庫.引当.void<br>sends void"]
    s12(["succeed 結果 = 未払い, 期日 = 期限.day"])
    start --> s1
    s1 -.->|"on 在庫切れ"| s2
    s1 --> s3
    s3 --> s4
    s4 --> s5
    s5 --> s6
    s6 -->|"true"| s7
    s7 -.->|"on expired"| s8
    s7 --> s9
    s6 -->|"false"| s10
    s10 --> s12
    s10 -.->|"on expired"| s12
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s2,s8,s9,s12 ok
```

A rectangle is a task, one with a line down each side a rule or a date of a dates file, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## on failure

Runs when a call fails and nothing at the call handles the error: from the calls on lines 56, 58, 60, 63 and 67. When it runs to its end, the workflow fails with that error.

```mermaid
flowchart TD
    onf(["on failure"])
    s13{{"match 押さえ.state"}}
    s15["押さえ ← 戻す(…)<br>book 在庫.引当.void<br>sends void"]
    s18(["fail 戻せない<br>#quot;注文のために押さえた品を戻せませんでした#quot;<br>leaving 押さえ"])
    onfEnd(["fails with the same error"])
    onf --> s13
    s13 -->|"held"| s15
    s15 -.->|"on failure"| s18
    s13 -->|"none"| onfEnd
    s15 --> onfEnd
    s15 -.->|"on expired"| onfEnd
    s15 -.->|"on already_posted"| onfEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s18 bad
```

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails | Case after |
|---:|---|---|---|---|---|---|
| 56 | `押さえ ← 引き当てる(…)` | `book 在庫.引当.hold`, `starts 在庫.引当` | — | — | `在庫切れ` → line 57<br>`timeout`, `failure` → `on failure` | `押さえ`: `held` |
| 58 | `期限 = 支払条件.支払日(…)` | the date `支払日` of `支払条件.cal` | 2 times, after 1 second and 2 (failure) | — | `timeout`, `failure` → `on failure` | — |
| 60 | `入金 = 支払を確かめる(…)` | `GET https://payments.example.com/orders/{注文}/payment`, `idempotent` | 2 times every 10 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | — |
| 63 | `押さえ ← 出荷する(…)` | `book 在庫.引当.post`, `sends post` | — | — | `expired` → line 64<br>`timeout`, `failure` → `on failure` | `押さえ`: `posted` |
| 67 | `押さえ ← 戻す(…)` | `book 在庫.引当.void`, `sends void` | — | — | `expired` → line 68<br>`already_posted`, `timeout`, `failure` → `on failure` | `押さえ`: `voided` |
| 75 | `押さえ ← 戻す(…)` | `book 在庫.引当.void`, `sends void` | — | — | `expired` → line 76<br>`already_posted` → line 77<br>`timeout`, `failure` → line 78 | `押さえ`: `voided` |

## Ends

Every way the workflow can end, and what each case can be then, the events on the other side included.

| Line | End | `押さえ` |
|---:|---|---|
| 57 | `succeed 結果 = 在庫切れ, 期日 = none` | not started |
| 64 | `succeed 結果 = 未払い, 期日 = 期限.day` | `expired` |
| 65 | `succeed 結果 = 出荷済, 期日 = 期限.day` | `posted` |
| 69 | `succeed 結果 = 未払い, 期日 = 期限.day` | `voided`, `expired` |
| 78 | `fail 戻せない` "注文のために押さえた品を戻せませんでした" `leaving 押さえ` | handed over as it is: `held`, `posted`, `voided`, `expired` |
| 78 | `on failure` runs to its end, and the workflow fails with the error that started it | not started, or `posted`, `voided`, `expired` |

