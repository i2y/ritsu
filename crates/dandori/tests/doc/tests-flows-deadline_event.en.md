# deadline_event v1

Waits for an event until a time (`timeout until`, Temporal): the approval comes to the workflow by its id, until 17:00 on the second business day after the request. The time is the event task's one parameter, which nothing sends. On the other platforms, E050

`tests/flows/deadline_event.flow`, drawn by `dandori doc`. Inputs: `order: string`. Outputs: `approved_by: string`.

## flow

```mermaid
flowchart TD
    start(["deadline_event v1"])
    s1[["due = terms.due(…)<br>dates answer_terms.cal"]]
    s2[/"a = approval(…)<br>event<br>timeout until by"/]
    s3(["fail Rejected<br>#quot;Order {order} was rejected#quot;"])
    s4(["fail NoApproval<br>#quot;No approval for order {order} by {due.at}#quot;"])
    s5(["succeed approved_by = a.approver"])
    start --> s1
    s1 --> s2
    s2 -.->|"on rejected"| s3
    s2 -.->|"on timeout"| s4
    s2 --> s5
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s5 ok
    class s3,s4 bad
```

A rectangle is a task, one with a line down each side a rule or a date of a dates file, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 22 | `due = terms.due(…)` | the date `due` of `answer_terms.cal` | 2 times, after 1 second and 2 (failure, timeout) | 10 seconds | `timeout`, `failure` → the workflow fails |
| 23 | `a = approval(…)` | `event` | — | until `due.at` | `rejected` → line 24<br>`timeout` → line 25<br>`failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 24 | `fail Rejected` "Order {order} was rejected" |
| 25 | `fail NoApproval` "No approval for order {order} by {due.at}" |
| 26 | `succeed approved_by = a.approver` |

