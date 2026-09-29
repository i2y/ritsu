# review v1

Send an application to be scored, and when the score says hold, wait for a person's approval. Every task is one you write. Written for Temporal: scoring runs on the workers of its own task queue, which may be written in another language, and so does the notice; the approver's tool answers the callback by the update the generated client sends

`examples/review/temporal/review.flow`, drawn by `dandori doc`. Inputs: `application: Application`. Outputs: `verdict: Verdict`.

## flow

```mermaid
flowchart TD
    start(["review v1"])
    s1["r = score(…)<br>a task you write<br>retry 2 times every 10 seconds"]
    s2(["fail Unscorable<br>#quot;Could not score application {application.id}#quot;"])
    s3{{"match r.verdict"}}
    s5[/"a = ask_for_approval(…)<br>a task you write, answered by a callback<br>timeout 3 days"/]
    s6(["fail NoAnswer<br>#quot;No approval in three days#quot;"])
    s7["notify(…)<br>a task you write"]
    s8(["succeed verdict = approve"])
    s9["notify(…)<br>a task you write"]
    s10(["succeed verdict = r.verdict"])
    start --> s1
    s1 -.->|"on unscorable"| s2
    s1 --> s3
    s3 -->|"hold"| s5
    s5 -.->|"on timeout"| s6
    s5 --> s7
    s7 --> s8
    s3 -->|"approve, reject"| s9
    s9 --> s10
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s8,s10 ok
    class s2,s6 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 40 | `r = score(…)` | a task you write, `idempotent` | 2 times every 10 seconds (failure, timeout) | — | `unscorable` → line 41<br>`timeout`, `failure` → the workflow fails |
| 45 | `a = ask_for_approval(…)` | a task you write, answered by a callback | — | 3 days | `timeout` → line 46<br>`failure` → the workflow fails |
| 47 | `notify(…)` | a task you write, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 49 | `notify(…)` | a task you write, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 41 | `fail Unscorable` "Could not score application {application.id}" |
| 46 | `fail NoAnswer` "No approval in three days" |
| 48 | `succeed verdict = approve` |
| 50 | `succeed verdict = r.verdict` |

