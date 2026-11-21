# 日付の名前のサービス v1

service_dates と同じように車を貸す。運転者と返却のレコードに、Python のコードがリクエストの日を読むための名前 fill と read_day を付ける

`tests/flows/service_date_names.ja.flow`, drawn by `dandori doc`. Inputs: `car: string`, `pickup: date`, `dropoff: date`, `closed: list[date]`, `driver: fill`, `others: list[fill]`. It implements `date_names.v1.DateNamesJa` (`protos/date_names.proto`).

## flow

```mermaid
flowchart TD
    start(["日付の名前のサービス v1"])
    s1["引き渡す(…)<br>lambda rental-desk"]
    subgraph L2 ["for ほか in others at most 3"]
        s3["引き渡す(…)<br>lambda rental-desk"]
    end
    s4[/"戻り = 返却を待つ(…)<br>lambda rental-return · callback<br>timeout 7 days"/]
    s5(["fail 返却の遅れ<br>#quot;7 日たっても車が戻りません#quot;"])
    s6["締める(…)<br>lambda rental-close"]
    fin(["end: succeeds"])
    start --> s1
    s1 --> s3
    s3 -->|"next round"| s3
    L2 -->|"after the last item"| s4
    s4 -.->|"on timeout"| s5
    s4 --> s6
    s6 --> fin
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s5 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Service

What each method of the service does to a run.

| Method | What it does | Task |
|---|---|---|
| `Rent` | starts a run; it can fail with `返却の遅れ` | — |
| `AnswerReturn` | answers the callback | `返却を待つ` |

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 41 | `引き渡す(…)` | `lambda rental-desk`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 43 | `引き渡す(…)` | `lambda rental-desk`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 44 | `戻り = 返却を待つ(…)` | `lambda rental-return · callback` | — | 1 minute to hand the id on, then 7 days | `timeout` → line 45<br>`failure` → the workflow fails |
| 46 | `締める(…)` | `lambda rental-close`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 45 | `fail 返却の遅れ` "7 日たっても車が戻りません" |
| 46 | the flow runs to its end, and the workflow succeeds |

