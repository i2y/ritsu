# inquiry v1

Jev picks the kind of a customer's inquiry and says how sure it is; an agent reads the inquiry for an order number and its point, and for its kind too, which the flow takes when Jev is not sure enough of its own. A rule decides the desk that takes it and how soon it is answered; another agent drafts the first reply; and a ticket goes into the desk's system. Jev picks, agents read and write (a model the company runs itself reads, Claude writes), and the rule decides. Written for Temporal: Jev and the agents are activities dandori writes, Jev called with TypeSafe's key (TYPESAFE_API_KEY), the reading sent to the company's Ollama, which serves Open Responses (`url`), and the draft to Claude with the key the worker has; the rule runs in the worker of the workflow as a local activity, its answer kept in the history as a marker; and filing the ticket is an activity you write

`examples/inquiry/temporal/inquiry.flow`, drawn by `dandori doc`. Inputs: `inquiry: Inquiry`. Outputs: `ticket_id: string`, `desk: routing.desk`.

## flow

```mermaid
flowchart TD
    start(["inquiry v1"])
    s1["reading = read_inquiry(…)<br>agent · gpt-oss:20b · http://ollama.internal:11434/v1<br>retry 2 times every 10 seconds · timeout 1 minute"]
    s2["by_hand = file_ticket(…)<br>a task you write<br>retry 2 times every 5 seconds"]
    s3(["succeed ticket_id = by_hand.ticket_id, desk = general"])
    s4["kind = pick_kind(…)<br>jev · jev-1.13.0 · confidence 0.8 else unsure<br>retry 2 times every 1 second · timeout 10 seconds"]
    s5["let kind = reading.kind"]
    s6[["decision = routing(…)<br>rule inquiry_routing.rule · local"]]
    s7["let subject = #quot;An inquiry about {kind}#quot;"]
    s8{{"match reading.order_id"}}
    s9["let subject = #quot;An inquiry about {kind} (order {order_id…"]
    s11["draft = draft_reply(…)<br>agent claude · claude-sonnet-5<br>timeout 1 minute"]
    s12["let draft = none"]
    s13["t = file_ticket(…)<br>a task you write<br>retry 2 times every 5 seconds"]
    s14(["succeed ticket_id = t.ticket_id, desk = decision.desk"])
    start --> s1
    s1 -.->|"on failure"| s2
    s2 --> s3
    s1 --> s4
    s4 -.->|"on unsure, failure"| s5
    s4 --> s6
    s5 --> s6
    s6 --> s7
    s7 --> s8
    s8 -->|"some order_id"| s9
    s9 --> s11
    s8 -->|"none"| s11
    s11 -.->|"on failure"| s12
    s11 --> s13
    s12 --> s13
    s13 --> s14
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s3,s14 ok
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 61 | `reading = read_inquiry(…)` | `agent · gpt-oss:20b · http://ollama.internal:11434/v1` | 2 times every 10 seconds (failure, timeout) | 1 minute | `timeout`, `failure` → line 62 |
| 63 | `by_hand = file_ticket(…)` | a task you write, `key` | 2 times every 5 seconds (failure, timeout) | — | `timeout`, `failure` → the workflow fails |
| 66 | `kind = pick_kind(…)` | `jev · jev-1.13.0 · confidence 0.8 else unsure` | 2 times every 1 second (failure, timeout) | 10 seconds | `unsure`, `timeout`, `failure` → line 67 |
| 68 | `decision = routing(…)` | rule `inquiry_routing.rule`, a local activity on Temporal | 2 times, after 1 second and 2 (failure) | — | `timeout`, `failure` → the workflow fails |
| 73 | `draft = draft_reply(…)` | `agent claude · claude-sonnet-5` | — | 1 minute | `timeout`, `failure` → line 74 |
| 75 | `t = file_ticket(…)` | a task you write, `key` | 2 times every 5 seconds (failure, timeout) | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 64 | `succeed ticket_id = by_hand.ticket_id, desk = general` |
| 76 | `succeed ticket_id = t.ticket_id, desk = decision.desk` |

