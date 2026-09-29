# Write a workflow

A `.flow` file is one workflow: what it takes and gives, the rules and APIs it uses, the tasks it
calls, the things it drives from state to state, and the flow itself. This page walks through the
hotel booking as written for Temporal,
[examples/hotel/temporal/hotel.flow](https://github.com/i2y/dandori/blob/main/examples/hotel/temporal/hotel.flow):
when a stay is booked, it holds an amount on the card, and captures it on the day of check-out.

## The declarations

```flow
workflow hotel_stay v1
description "When a stay is booked, hold an amount on the card, and capture it on the day of check-out. …"

use rule hold from "../rules/hold_amount.rule"
use rule payment_intent from "../rules/payment_intent.rule"
use openapi stripe from "../specs/stripe.json"

record Booking
  id        : string
  room      : hold.room
  nights    : int  range >=1 <=30
  card      : string
  check_out : timestamp

enum Outcome = stayed | awaiting_review

inputs
  booking : Booking

outputs
  outcome : Outcome
```

- `workflow hotel_stay v1` names the workflow and its version. On Temporal the version is part of
  the workflow type and the task queue (`hotel_stay_v1`), so a new version runs beside the old one
  instead of replacing the code of the runs that are going on.
- `use rule` reads a rule written in [rulec](https://github.com/i2y/rulec). `hold` decides how much
  to hold and whether the front desk looks first; `payment_intent` is Stripe's PaymentIntent written
  down as a rulec state machine. Its enums and records are types here: `hold.room`.
- `use openapi` reads an API description, and the tasks that call it are held to it
  ([API descriptions](tasks.md#api-descriptions)).
- `inputs` and `outputs` are what a run starts with and ends with.

The types are `int`, numbers with a unit such as `money[JPY, incl_tax]` (as rulec has them),
`string`, `bool`, `timestamp`, enums, records, `list[T]`, `T?` for a value that may be absent, and
`json` for a value passed along without being looked into. A number can also say what it may be,
with a [range](#ranges).

## Tasks

A task is one call. It says what it takes and answers, how it is called, what it does to a case,
the errors it can come back with, and how it is retried.

```flow
task create_intent(amount: money[JPY, incl_tax] range >=50 <=99999999, currency: string, payment_method: string, capture_method: payment_intent.capture_method) -> PaymentIntent
  http POST stripe "/v1/payment_intents"
  starts payment_intent.payment then attach
  key
  retry 2 times every 2 seconds

task confirm_intent(intent: string) -> PaymentIntent
  http POST stripe "/v1/payment_intents/{intent}/confirm"
  sends confirm
  errors card_declined = 402, unexpected_state = 400
  refused as unexpected_state
  key

task get_intent(intent: string) -> PaymentIntent
  http GET stripe "/v1/payment_intents/{intent}"
  observes
  idempotent
  retry 3 times every 2 seconds
```

- **How it is called**: `http`, `lambda`, `aws`, `connect`, an `agent`, another `.flow`, or code
  you write. [What a task calls](tasks.md) has them all, and what each becomes on each platform.
- **What it does to a case**: `starts` a case in a state machine (here followed at once by the event
  `attach`), `sends` an event to it, or `observes` it. The checker moves the case's states along the
  state machine's table.
- **Errors**: each declared error has a name, and for an HTTP call the status it comes with
  (`card_declined = 402`). `refused as unexpected_state` says which error comes back when the state
  machine refuses the event.
- **Retries and keys**: a task that changes something on the other side and is retried needs an
  idempotency `key`, which dandori makes from the run and the place of the call; a task that changes
  nothing says `idempotent`. `retry 2 times every 2 seconds` retries a failure or a timeout, but
  not the task's declared errors; `retry … on busy` retries only what it names.
- `timeout 2 days` bounds a call, and `callback` makes a task wait for an answer that comes later
  (an approval, a packing crew's report).

## Cases

A case is something on the other side that the workflow drives from state to state: a
PaymentIntent, an order in a warehouse's system. It follows a rulec state machine.

```flow
case pi : PaymentIntent follows payment_intent.payment
  held capture_method = manual
  held confirmation_method = automatic
  external authenticate, settle, expire
  refused when refused = true
```

- `held` fixes, for this case, a value the state machine's table reads: this workflow always
  captures by hand, so only the rows for `manual` apply. The other values are the other side's to
  decide, and the checker tries them all.
- `external` lists the events that happen on the other side without the workflow asking: the
  customer's authentication, the bank's settlement, the hold's expiry. Before each call on the case,
  and at the end, the checker lets them happen.
- `refused when refused = true` says which of the machine's outputs marks a refused event.

When the workflow ends — by `succeed`, by `fail`, or at the end of the flow — every case it started
must be in a final state of its machine, or the checker reports the run that leaves it elsewhere
(E020). `fail … leaving pi` hands an unfinished case over on purpose.

## The flow

```flow
flow
  let quote = hold(room: booking.room, nights: booking.nights)
  match quote.handling
    review => succeed outcome = awaiting_review
    auto => pi <- create_intent(amount: quote.amount, currency: "jpy", payment_method: booking.card, capture_method: manual)
  pi <- confirm_intent(intent: pi.id)
    on card_declined => pi <- get_intent(intent: pi.id)
  match pi.status
    requires_action =>
      pi <- customer_authenticated()
        on timeout => pi <- get_intent(intent: pi.id)
    requires_capture, requires_payment_method, requires_confirmation, canceled => pass
  match pi.status
    requires_capture => wait until booking.check_out
    requires_payment_method, requires_confirmation, requires_action =>
      pi <- cancel_intent(intent: pi.id)
        on unexpected_state => pass
      fail CardDeclined "The card could not be held"
    canceled => fail PaymentCanceled "The PaymentIntent was canceled"
```

- `let x = task(…)` calls a task or a rule and keeps the answer; `pi <- task(…)` calls a task on the
  case `pi` and takes the case as it comes back.
- `match` branches on an enum, a bool, or a value that may be absent (`none`, `some x`). Every value
  must have an arm, and there is no default arm: a value no arm names is refused by the checker
  (E010), and at run time fails with `Dandori.UnexpectedValue` rather than slip into the last arm.
  A `.flow` has no comparison and no arithmetic: a condition is a rule's, or a task answers it (an
  API, an agent, your own code).
- `on <error> =>` handles a declared error of the call above it; `on failure =>` handles any failure,
  and `on timeout =>` a timeout.
- `wait 1 hour` and `wait until booking.check_out` wait. `succeed outcome = …` ends the run with its
  outputs, and `fail Name "why"` ends it as failed.

Loops always say how far they may go, so the size of a run's history has a bound:

```flow
      repeat at most 12 times
        wait 1 hour
        pi <- get_intent(intent: pi.id)
        match pi.status
          processing => pass
          succeeded, requires_payment_method => break
```

`for line in order.lines at most 50` goes through a list, and
`for line in order.lines at most 50 in parallel, 10 at a time` runs its rounds at the same time.
Each round keeps its own variables, and when a round fails, the others still run to their end, and
the first failure in the list's order decides. `let results = for … ` with `yield r` as the last
line of its body collects a list.

## When something fails, or the workflow is cancelled

```flow
on failure
  match pi.status
    none, succeeded, canceled => pass
    requires_payment_method, requires_confirmation, requires_action, requires_capture =>
      pi <- cancel_intent(intent: pi.id)
        on unexpected_state => pass
        on failure => fail CleanupFailed "Releasing the hold failed; handing it over to staff" leaving pi
    processing => fail SettlementUnclear "Failed in the middle of the capture; handing it over to staff" leaving pi
```

`on failure` runs when a task fails and nothing handled it, to settle the cases; the run then fails
with that error. `on cancel` is the same for a cancellation: on Temporal a workflow can be asked to
stop, and it cleans up before it ends as cancelled. The other platforms stop a run at once, so they
refuse `on cancel` (E050). The checker enters `on cancel` from every call and every wait a
cancellation can stop, and wants the cases settled there too.

## Ranges

A number can say what it may be, as a rulec rule's inputs do: `nights : int  range >=1 <=30`. A
range goes on an input, an output, a field of a record, and a task's parameter or answer
(`-> int  range >=0 <=10`); either end may be left out. The ends are whole numbers in the type's own
unit, written without it (`>=0`, not `>=0JPY`), as the values travel in JSON.

- What comes in is checked when the workflow runs. An input, or a task's or a rule's answer, with a
  number outside its range fails the run with `Dandori.BadInput` or `Dandori.BadResponse`, on every
  platform. A rule's answer is held to the range rulec gives it, in case the function that runs the
  rule is not the version the workflow was checked with.
- What goes out is checked before the workflow runs. A value given to a rule's input, a task's
  parameter, a field of a record written out, or an output must fit the range there (E014). A value
  whose range nothing says is a warning (W104).

In a draft of the hotel booking whose stays run longer than the rule for the hold takes
([tests/fixtures/hotel_ranges.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/hotel_ranges.flow)):

```text
error[E014]: tests/fixtures/hotel_ranges.flow:20:1: `booking.nights` can be outside `>=1 <=30`, the range of `nights` of the rule `hold`: it is `>=1 <=60`
    20 |   let quote = hold(room: booking.room, nights: booking.nights)
warning[W104]: tests/fixtures/hotel_ranges.flow:21:1: nothing says what range `extension` is in (the input `extension` has no range), and `nights` of the rule `hold` takes `>=1 <=30`
    21 |   let longer = hold(room: booking.room, nights: extension)
```

A variable's range is that of every value put in it, anywhere in the flow. Since a `.flow` has no
arithmetic, a range travels as it is, from where a value comes to where it goes. On Temporal, adding
a range or narrowing one changes what a running workflow does when its values fall outside, so it
ships as a new version or through Worker Deployment Versioning.

## See it drawn

`dandori doc` draws the workflow this page has read: [the hotel booking, drawn](https://i2y.github.io/dandori/doc/hotel.html),
where each scenario lights up the way its run goes. [Draw a workflow](diagrams.md) says what is on
the picture.
