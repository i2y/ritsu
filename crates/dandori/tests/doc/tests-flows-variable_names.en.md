# variable_names v1

Variables that the code of a target would write as one name, and variables named as the code's own: class and class_, _x and v_x, type and type_, and names that begin with dd_. The code of each target keeps them apart, and the flow runs on every platform

`tests/flows/variable_names.flow`, drawn by `dandori doc`. Inputs: `id: string`, `_x: int`. Outputs: `text: string`, `counts: string`.

## flow

```mermaid
flowchart TD
    start(["variable_names v1"])
    s1["class = note(…)<br>lambda note"]
    s2["class_ = note(…)<br>lambda note"]
    s3["v_x = count(…)<br>lambda count"]
    s4["type = note(…)<br>lambda note"]
    s5["type_ = note(…)<br>lambda note"]
    s6["dd_error = count(…)<br>lambda count"]
    s7["dd_v = count(…)<br>lambda count"]
    s8["let dd_x = 3"]
    s9(["succeed text = #quot;{class} {class_} {type} {type_}#quot;, count…"])
    start --> s1
    s1 --> s2
    s2 --> s3
    s3 --> s4
    s4 --> s5
    s5 --> s6
    s6 --> s7
    s7 --> s8
    s8 --> s9
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s9 ok
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 22 | `class = note(…)` | `lambda note`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 23 | `class_ = note(…)` | `lambda note`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 25 | `v_x = count(…)` | `lambda count`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 27 | `type = note(…)` | `lambda note`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 28 | `type_ = note(…)` | `lambda note`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 30 | `dd_error = count(…)` | `lambda count`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 31 | `dd_v = count(…)` | `lambda count`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 33 | `succeed text = "{class} {class_} {type} {type_}", counts = "{_x} {v_x} {dd_error} {dd_v} {dd_x}"` |

