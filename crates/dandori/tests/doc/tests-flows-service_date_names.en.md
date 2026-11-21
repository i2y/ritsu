# service_date_names v1

Rent a car as service_dates does, with the records of the driver and of the return named fill and read_day, the names the code of Python has for reading the days of a request

`tests/flows/service_date_names.flow`, drawn by `dandori doc`. Inputs: `car: string`, `pickup: date`, `dropoff: date`, `closed: list[date]`, `driver: fill`, `others: list[fill]`. It implements `date_names.v1.DateNames` (`protos/date_names.proto`).

## flow

```mermaid
flowchart TD
    start(["service_date_names v1"])
    s1["hand_over(…)<br>lambda rental-desk"]
    subgraph L2 ["for o in others at most 3"]
        s3["hand_over(…)<br>lambda rental-desk"]
    end
    s4[/"back = wait_for_return(…)<br>lambda rental-return · callback<br>timeout 7 days"/]
    s5(["fail late<br>#quot;the car was not back within 7 days#quot;"])
    s6["close(…)<br>lambda rental-close"]
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
| `Rent` | starts a run; it can fail with `late` | — |
| `AnswerReturn` | answers the callback | `wait_for_return` |

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 41 | `hand_over(…)` | `lambda rental-desk`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 43 | `hand_over(…)` | `lambda rental-desk`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 44 | `back = wait_for_return(…)` | `lambda rental-return · callback` | — | 1 minute to hand the id on, then 7 days | `timeout` → line 45<br>`failure` → the workflow fails |
| 46 | `close(…)` | `lambda rental-close`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 45 | `fail late` "the car was not back within 7 days" |
| 46 | the flow runs to its end, and the workflow succeeds |

