# invoice v1

An order holds its goods until its payment is due, then ships them once paid, or puts them back

`examples/invoice/invoice.flow`, drawn by `dandori doc`. Inputs: `order: Order`. Outputs: `outcome: Outcome`, `due: date?`. It runs in the period `now range >=2026-01-01 <=2027-12-31`.

## flow

```mermaid
flowchart TD
    start(["invoice v1"])
    s1["hold ← reserve(…)<br>book stock.reserve.hold<br>starts"]
    s2(["succeed outcome = out_of_stock, due = none"])
    s3[["due = terms.payment(…)<br>dates payment_terms.cal"]]
    s4("wait until due.at")
    s5["payment = check_payment(…)<br>GET https://payments.example.com/orders/{order}/payment<br>retry 2 times every 10 seconds"]
    s6{{"match payment.paid"}}
    s7["hold ← ship(…)<br>book stock.reserve.post<br>sends post"]
    s8(["succeed outcome = not_paid, due = due.day"])
    s9(["succeed outcome = shipped, due = due.day"])
    s10["hold ← put_back(…)<br>book stock.reserve.void<br>sends void"]
    s12(["succeed outcome = not_paid, due = due.day"])
    start --> s1
    s1 -.->|"on out_of_stock"| s2
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

Runs when a call fails and nothing at the call handles the error: from the calls on lines 59, 61, 63, 66 and 70. It runs too when `now` on line 61 is outside the period the workflow runs in, `>=2026-01-01 <=2027-12-31`. When it runs to its end, the workflow fails with that error.

```mermaid
flowchart TD
    onf(["on failure"])
    s13{{"match hold.state"}}
    s15["hold ← put_back(…)<br>book stock.reserve.void<br>sends void"]
    s18(["fail NotPutBack<br>#quot;the goods held for the order could not be put …<br>leaving hold"])
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
| 59 | `hold ← reserve(…)` | `book stock.reserve.hold`, `starts stock.reserve` | — | — | `out_of_stock` → line 60<br>`timeout`, `failure` → `on failure` | `hold`: `held` |
| 61 | `due = terms.payment(…)` | the date `payment` of `payment_terms.cal` | 2 times, after 1 second and 2 (failure, timeout) | 10 seconds | `timeout`, `failure` → `on failure` | — |
| 63 | `payment = check_payment(…)` | `GET https://payments.example.com/orders/{order}/payment`, `idempotent` | 2 times every 10 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | — |
| 66 | `hold ← ship(…)` | `book stock.reserve.post`, `sends post` | — | — | `expired` → line 67<br>`timeout`, `failure` → `on failure` | `hold`: `posted` |
| 70 | `hold ← put_back(…)` | `book stock.reserve.void`, `sends void` | — | — | `expired` → line 71<br>`already_posted`, `timeout`, `failure` → `on failure` | `hold`: `voided` |
| 78 | `hold ← put_back(…)` | `book stock.reserve.void`, `sends void` | — | — | `expired` → line 79<br>`already_posted` → line 80<br>`timeout`, `failure` → line 81 | `hold`: `voided` |

## Ends

Every way the workflow can end, and what each case can be then, the events on the other side included.

| Line | End | `hold` |
|---:|---|---|
| 60 | `succeed outcome = out_of_stock, due = none` | not started |
| 67 | `succeed outcome = not_paid, due = due.day` | `expired` |
| 68 | `succeed outcome = shipped, due = due.day` | `posted` |
| 72 | `succeed outcome = not_paid, due = due.day` | `voided`, `expired` |
| 81 | `fail NotPutBack` "the goods held for the order could not be put back" `leaving hold` | handed over as it is: `held`, `posted`, `voided`, `expired` |
| 81 | `on failure` runs to its end, and the workflow fails with the error that started it | not started, or `posted`, `voided`, `expired` |

