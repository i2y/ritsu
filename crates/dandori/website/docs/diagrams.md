# Draw a workflow

`dandori doc` draws a `.flow` for the person who reviews it. The picture has every call, match,
wait and loop of the flow, and beside it is what the checker knows that the text does not show:
what each call calls, how it is retried and where each of its errors goes, what each case can be
after a call, and every way the workflow can end, with the state each case is left in.

```text
dandori doc hotel.flow > hotel.md
dandori doc hotel.flow --format html > hotel.html
```

The picture is the same whatever the platform: it is drawn from the checked `.flow`, not from what
a build writes, and it is there before anything runs. Temporal has no picture of a workflow's code,
and the graphs Step Functions and Argo Workflows draw show every state and template a build adds,
such as the check of each answer and the steps of each loop.

## As Markdown, for a pull request

The Markdown draws the flow, and `on failure` and `on cancel`, as Mermaid flowcharts, which GitHub
draws in a pull request, an issue or a README. Under them, a table lists every call and another
every way the workflow can end. The review example
([examples/review/temporal](https://github.com/i2y/dandori/blob/main/examples/review/temporal/review.flow)),
whose tasks are all code of your own:

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

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes
from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and
a box around steps a loop. A dashed arrow is an error the call handles. The second line of a call
says how it is called, and the third what its declaration adds that the flow does not show: what it
does to a case, its retries, its timeout.

## As one page, with the runs lit up

`--format html` writes one page that needs nothing else: the picture is drawn by dandori itself, and
the page works without a network. The page of each example, as written for Temporal:
[hotel](doc/hotel.html), [order](doc/order.html), [fulfillment](doc/fulfillment.html),
[inquiry](doc/inquiry.html) and [review](doc/review.html).

- **Pick a step.** The pane on the right shows its lines of the `.flow`, what each case can be when
  a run gets there, what the call calls, its retries and timeout, where each of its errors goes,
  what the case can be after it, and the declaration of the task or the rule.
- **Pick a scenario.** On the left are the scenarios `dandori scenarios` writes, which together take
  every arm, every handler of an error and every way a case can move, grouped by how they end. The
  way a scenario's run goes lights up, with the number of times it passed a step beside the step,
  and the pane lists what each call answered.
- **Both.** With a step picked, the scenarios that pass it stand out, and the pane says how many of
  them do.

A call marked `!` has an error it does not handle, which goes on to `on failure`, or fails the
workflow; the mark lights up in a run where that happens. The address keeps what is picked
(`hotel.html#run=40&node=s23`), so a link can show one run.

## A workflow the checker finds errors in

`doc` draws a workflow whose check finds errors too, as long as its names and types resolve, and
exits with 1. The page lists the diagnostics instead of scenarios, and picking one lights up the run
that gets there: an E020 becomes the way on the picture that ends with the case unsettled. The
Markdown puts the diagnostics at its end, as `dandori check` prints them.

## Where each thing is drawn

| In the `.flow` | In the picture |
|---|---|
| a call of a task | a rectangle; a task whose value comes from outside (`event`, `callback`) is slanted |
| a call of a rule | a rectangle with a line down each side |
| `on <error> =>` under a call | a dashed arrow to the handler's steps, beside the call |
| `match` | a hexagon, with an arrow for each arm; the first arm that goes on stands under it |
| `wait`, `wait until` | a rounded box |
| `repeat`, `for` | a box around the steps it repeats: the way round is on its left, `break` leaves on its right, and a parallel `for` has no way round |
| `succeed`, `fail` | a green or a red end; `fail … leaving` says which case it hands over |
| `on failure`, `on cancel` | a picture of their own, from where they begin to how the workflow ends |
| `let x = <value>` | a rectangle |
| `pass` | nothing: the arrow goes on |
