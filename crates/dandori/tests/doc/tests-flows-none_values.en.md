# none_values v1

Count a topic's posts up to a day, post a note on it and pin the post, where the day and the note may not be there

`tests/flows/none_values.flow`, drawn by `dandori doc`. Inputs: `topic: string`, `until: date?`, `note: string?`. Outputs: `count: int range >=0 <=1000`, `pinned: bool`.

## flow

```mermaid
flowchart TD
    start(["none_values v1"])
    s1["found = count_posts(…)<br>GET board /posts"]
    s2["posted = post(…)<br>POST board /posts"]
    s3["pinned = pin(…)<br>POST board /posts/{id}/pin"]
    s4(["succeed count = found.count, pinned = pinned.pinned"])
    start --> s1
    s1 --> s2
    s2 --> s3
    s3 --> s4
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s4 ok
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 46 | `found = count_posts(…)` | `GET board /posts`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |
| 47 | `posted = post(…)` | `POST board /posts` | — | — | `timeout`, `failure` → the workflow fails |
| 48 | `pinned = pin(…)` | `POST board /posts/{id}/pin` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 49 | `succeed count = found.count, pinned = pinned.pinned` |

