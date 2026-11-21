# enum_values v1

Writes values of an enum where nothing says their type: the lists a for goes through, one round after another and in parallel, a list put in a variable with no type, whose other item says its type, and a value put in a variable with no type. Each is a value of the one enum that has it, and the code of every platform writes its type

`tests/flows/enum_values.flow`, drawn by `dandori doc`. Inputs: `claim_id: string`, `first: role`. Outputs: `asked: list[role]`, `last: role`.

## flow

```mermaid
flowchart TD
    start(["enum_values v1"])
    subgraph L1 ["for r in [manager, accounting] at most 2"]
        s2["ask(…)<br>lambda ask"]
    end
    subgraph L3 ["let asked = for party in [legal, manager] at most 2 in parallel · yield party"]
        s4["ask(…)<br>lambda ask"]
    end
    s5["let reviewers = [legal, first]"]
    subgraph L6 ["for reviewer in reviewers at most 2"]
        s7["ask(…)<br>lambda ask"]
    end
    s8["let last = accounting"]
    s9(["succeed asked = asked, last = last"])
    start --> s2
    s2 -->|"next round"| s2
    L1 -->|"after the last item"| s4
    L3 -->|"every round done"| s5
    s5 --> s7
    s7 -->|"next round"| s7
    L6 -->|"after the last item"| s8
    s8 --> s9
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s9 ok
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 19 | `ask(…)` | `lambda ask` | — | — | `timeout`, `failure` → the workflow fails |
| 21 | `ask(…)` | `lambda ask` | — | — | `timeout`, `failure` → the round fails, and then the workflow |
| 25 | `ask(…)` | `lambda ask` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 27 | `succeed asked = asked, last = last` |

