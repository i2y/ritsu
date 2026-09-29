# hotel_stay v1

When a stay is booked, hold an amount on the card, and capture it on the day of check-out. A rule decides the amount and whether the front desk looks first; the payment is a Stripe PaymentIntent

`tests/fixtures/hotel_naive.flow`, drawn by `dandori doc`. Inputs: `booking: Booking`. Outputs: `outcome: Outcome`.

**`dandori check` finds 4 error(s) in this workflow**; they are at the end, each with the run that gets there.

## flow

```mermaid
flowchart TD
    start(["hotel_stay v1"])
    s1[["quote = hold(…)<br>rule hold_amount.rule"]]
    s2{{"match quote.handling"}}
    s3(["succeed outcome = awaiting_review"])
    s4["pi ← create_intent(…)<br>POST https://api.stripe.com/v1/payment_intents form<br>starts · retry 2 times every 2 seconds"]
    s5["pi ← confirm_intent(…)<br>POST https://api.stripe.com/v1/payment_intents/{id}/confirm form<br>sends confirm"]
    s6["pi ← get_intent(…)<br>GET https://api.stripe.com/v1/payment_intents/{id}<br>observes · retry 3 times every 2 seconds"]
    s7{{"match pi.status"}}
    s8[/"pi ← wait_for_customer(…)<br>lambda stripe-webhook-inbox · callback<br>observes · timeout 1 hour"/]
    s9("wait until booking.check_out")
    s10(["fail CardDeclined<br>#quot;The card was declined#quot;"])
    s11(["fail PaymentCanceled<br>#quot;The PaymentIntent was canceled#quot;"])
    s12["pi ← capture_intent(…)<br>POST https://api.stripe.com/v1/payment_intents/{id}/capture form<br>sends capture · retry 2 times every 5 seconds"]
    s13(["fail HoldExpired<br>#quot;The hold had expired#quot;"])
    s14(["succeed outcome = stayed"])
    start --> s1
    s1 --> s2
    s2 -->|"review"| s3
    s2 -->|"auto"| s4
    s4 --> s5
    s5 -.->|"on card_declined"| s6
    s5 --> s7
    s6 --> s7
    s7 -->|"requires_action"| s8
    s7 -->|"requires_capture"| s9
    s7 -->|"requires_payment_method"| s10
    s7 -->|"canceled"| s11
    s8 --> s12
    s9 --> s12
    s12 -.->|"on unexpected_state"| s13
    s12 --> s14
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s3,s14 ok
    class s10,s11,s13 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails | Case after |
|---:|---|---|---|---|---|---|
| 81 | `quote = hold(…)` | rule `hold_amount.rule` | 2 times, after 1 second and 2 (failure) | — | `timeout`, `failure` → the workflow fails | — |
| 84 | `pi ← create_intent(…)` | `POST https://api.stripe.com/v1/payment_intents form`, `starts payment_intent.payment then attach`, `key` | 2 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → the workflow fails | `pi`: `requires_confirmation` |
| 85 | `pi ← confirm_intent(…)` | `POST https://api.stripe.com/v1/payment_intents/{id}/confirm form`, `sends confirm`, `key` | — | — | `card_declined` → line 86<br>`unexpected_state`, `timeout`, `failure` → the workflow fails | `pi`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 86 | `pi ← get_intent(…)` | `GET https://api.stripe.com/v1/payment_intents/{id}`, `observes`, `idempotent` | 3 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → the workflow fails | `pi`: `requires_payment_method`, `requires_confirmation`, `requires_action`, `requires_capture`, `canceled` |
| 89 | `pi ← wait_for_customer(…)` | `lambda stripe-webhook-inbox · callback`, `observes` | — | 1 hour | `timeout`, `failure` → the workflow fails | `pi`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 93 | `pi ← capture_intent(…)` | `POST https://api.stripe.com/v1/payment_intents/{id}/capture form`, `sends capture`, `key` | 2 times every 5 seconds (failure, timeout) | — | `unexpected_state` → line 94<br>`timeout`, `failure` → the workflow fails | `pi`: `processing`, `succeeded` |

## Ends

Every way the workflow can end, and what each case can be then, the events on the other side included.

| Line | End | `pi` |
|---:|---|---|
| 83 | `succeed outcome = awaiting_review` | not started |
| 91 | `fail CardDeclined` "The card was declined" | `requires_payment_method` |
| 92 | `fail PaymentCanceled` "The PaymentIntent was canceled" | `canceled` |
| 94 | `fail HoldExpired` "The hold had expired" | `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 95 | `succeed outcome = stayed` | `requires_payment_method`, `processing`, `succeeded` |

## What check says

What `dandori check` says of this workflow, each with the run that gets there.

```text
warning[W104]: tests/fixtures/hotel_naive.flow:81:1: nothing says what range `booking.nights` is in (the field `nights` of `Booking` has no range), and `nights` of the rule `hold` takes `>=1 <=30`
    81 |   let quote = hold(room: booking.room, nights: booking.nights)
warning[W101]: tests/fixtures/hotel_naive.flow:85:1: if `confirm_intent` fails, the workflow fails with the case `pi` in requires_payment_method, requires_confirmation, requires_action, requires_capture; 4 call(s) can fail like this. Handle the error at the call, or add `on failure` to settle the case
    85 |   pi <- confirm_intent(id: pi.id)
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_payment_method
      85  confirm_intent fails (timeout, failure)
error[E010]: tests/fixtures/hotel_naive.flow:87:1: `match` has no arm for requires_confirmation, which `pi.status` can be here
    87 |   match pi.status
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      86  confirm_intent fails: on card_declined
      86  get_intent: pi is requires_confirmation
error[E020]: tests/fixtures/hotel_naive.flow:91:1: the workflow can fail here with the case `pi` in requires_payment_method, which is not final (succeeded, canceled are); settle it first, or write `leaving pi` to hand it over as it is
    91 |     requires_payment_method => fail CardDeclined "The card was declined"
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_payment_method
      91  match pi.status: requires_payment_method
      91  fail CardDeclined
error[E020]: tests/fixtures/hotel_naive.flow:94:1: the workflow can fail here with the case `pi` in requires_payment_method, requires_action, requires_capture, which is not final (succeeded, canceled are); settle it first, or write `leaving pi` to hand it over as it is
    94 |     on unexpected_state => fail HoldExpired "The hold had expired"
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_action
      88  match pi.status: requires_action
          `authenticate` happens on the other side: pi requires_action → requires_payment_method
      89  wait_for_customer: pi is requires_payment_method
      93  capture_intent: capture is refused (unexpected_state)
      94  fail HoldExpired
error[E020]: tests/fixtures/hotel_naive.flow:95:1: the workflow can end here with the case `pi` in requires_payment_method, processing, which is not final (succeeded, canceled are)
    95 |   succeed outcome = stayed
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_capture
      90  match pi.status: requires_capture
      90  wait until booking.check_out
      93  capture_intent: pi requires_capture → processing
          `settle` happens on the other side: pi processing → requires_payment_method
      95  succeed
```
