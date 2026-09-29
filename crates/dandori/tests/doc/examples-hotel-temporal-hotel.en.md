# hotel_stay v1

When a stay is booked, hold an amount on the card, and capture it on the day of check-out. A rule decides the amount and whether the front desk looks first; the payment is a Stripe PaymentIntent. Written for Temporal: the Stripe calls are activities dandori writes, and the Transport gives them the secret key; Stripe's webhook tells the workflow of the booking that the customer's 3-D Secure step is over, by the booking's id (an event); and a booking the guest cancels releases its hold (on cancel)

`examples/hotel/temporal/hotel.flow`, drawn by `dandori doc`. Inputs: `booking: Booking`. Outputs: `outcome: Outcome`.

## flow

```mermaid
flowchart TD
    start(["hotel_stay v1"])
    s1[["quote = hold(…)<br>rule hold_amount.rule"]]
    s2{{"match quote.handling"}}
    s3(["succeed outcome = awaiting_review"])
    s4["pi ← create_intent(…)<br>POST stripe /v1/payment_intents<br>starts · retry 2 times every 2 seconds"]
    s5["pi ← confirm_intent(…)<br>POST stripe /v1/payment_intents/{intent}/confirm<br>sends confirm"]
    s6["pi ← get_intent(…)<br>GET stripe /v1/payment_intents/{intent}<br>observes · retry 3 times every 2 seconds"]
    s7{{"match pi.status"}}
    s8[/"pi ← customer_authenticated()<br>event<br>observes · timeout 1 hour"/]
    s9["pi ← get_intent(…)<br>GET stripe /v1/payment_intents/{intent}<br>observes · retry 3 times every 2 seconds"]
    s11{{"match pi.status"}}
    s12("wait until booking.check_out")
    s13["pi ← cancel_intent(…)<br>POST stripe /v1/payment_intents/{intent}/cancel<br>sends cancel"]
    s15(["fail CardDeclined<br>#quot;The card could not be held#quot;"])
    s16(["fail PaymentCanceled<br>#quot;The PaymentIntent was canceled#quot;"])
    s17["pi ← capture_intent(…)<br>POST stripe /v1/payment_intents/{intent}/capture<br>sends capture · retry 2 times every 5 seconds"]
    s18(["fail HoldExpired<br>#quot;The hold had expired by check-out#quot;"])
    s19{{"match pi.status"}}
    s20(["succeed outcome = stayed"])
    subgraph L21 ["repeat at most 12 times"]
        s22("wait 1 hour")
        s23["pi ← get_intent(…)<br>GET stripe /v1/payment_intents/{intent}<br>observes · retry 3 times every 2 seconds"]
        s24{{"match pi.status"}}
        s26(["break"])
    end
    s27{{"match pi.status"}}
    s28(["succeed outcome = stayed"])
    s29(["fail SettlementUnclear<br>#quot;The capture has no clear outcome; handing it o…<br>leaving pi"])
    start --> s1
    s1 --> s2
    s2 -->|"review"| s3
    s2 -->|"auto"| s4
    s4 --> s5
    s5 -.->|"on card_declined"| s6
    s5 --> s7
    s6 --> s7
    s7 -->|"requires_action"| s8
    s8 -.->|"on timeout"| s9
    s8 --> s11
    s9 --> s11
    s7 -->|"requires_capture, requires_payment_method, requires_confirmation, canceled"| s11
    s11 -->|"requires_capture"| s12
    s11 -->|"requires_payment_method, requires_confirmation, requires_action"| s13
    s13 --> s15
    s13 -.->|"on unexpected_state"| s15
    s11 -->|"canceled"| s16
    s12 --> s17
    s17 -.->|"on unexpected_state"| s18
    s17 --> s19
    s19 -->|"succeeded"| s20
    s19 -->|"processing"| s22
    s22 --> s23
    s23 --> s24
    s24 -->|"succeeded, requires_payment_method"| s26
    s24 -->|"processing"| s22
    L21 -->|"after 12 rounds"| s27
    s26 --> s27
    s27 -->|"succeeded"| s28
    s27 -->|"processing, requires_payment_method"| s29
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s3,s20,s28 ok
    class s15,s16,s18,s29 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## on failure

Runs when a call fails and nothing at the call handles the error: from the calls on lines 75, 78, 79, 80, 83, 84, 89, 93 and 100. When it runs to its end, the workflow fails with that error.

```mermaid
flowchart TD
    onf(["on failure"])
    s30{{"match pi.status"}}
    s32["pi ← cancel_intent(…)<br>POST stripe /v1/payment_intents/{intent}/cancel<br>sends cancel"]
    s34(["fail CleanupFailed<br>#quot;Releasing the hold failed; handing it over to …<br>leaving pi"])
    s35(["fail SettlementUnclear<br>#quot;Failed in the middle of the capture; handing i…<br>leaving pi"])
    onfEnd(["fails with the same error"])
    onf --> s30
    s30 -->|"requires_payment_method, requires_confirmation, requires_action, requires_capture"| s32
    s32 -.->|"on failure"| s34
    s30 -->|"processing"| s35
    s30 -->|"succeeded, canceled, none"| onfEnd
    s32 --> onfEnd
    s32 -.->|"on unexpected_state"| onfEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s34,s35 bad
```

## on cancel

Runs when the workflow is cancelled, from the call or the wait the run is at. When it runs to its end, the workflow ends cancelled.

```mermaid
flowchart TD
    onc(["on cancel"])
    s36{{"match pi.status"}}
    s38["pi ← cancel_intent(…)<br>POST stripe /v1/payment_intents/{intent}/cancel<br>sends cancel"]
    s40(["fail CleanupFailed<br>#quot;Releasing the hold failed; handing it over to …<br>leaving pi"])
    s41(["fail SettlementUnclear<br>#quot;Cancelled in the middle of the capture; handin…<br>leaving pi"])
    oncEnd(["ends cancelled"])
    onc --> s36
    s36 -->|"requires_payment_method, requires_confirmation, requires_action, requires_capture"| s38
    s38 -.->|"on failure"| s40
    s36 -->|"processing"| s41
    s36 -->|"succeeded, canceled, none"| oncEnd
    s38 --> oncEnd
    s38 -.->|"on unexpected_state"| oncEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s40,s41 bad
```

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails | Case after |
|---:|---|---|---|---|---|---|
| 75 | `quote = hold(…)` | rule `hold_amount.rule` | 2 times, after 1 second and 2 (failure) | — | `timeout`, `failure` → `on failure` | — |
| 78 | `pi ← create_intent(…)` | `POST stripe /v1/payment_intents`, `starts payment_intent.payment then attach`, `key` | 2 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `pi`: `requires_confirmation` |
| 79 | `pi ← confirm_intent(…)` | `POST stripe /v1/payment_intents/{intent}/confirm`, `sends confirm`, `key` | — | — | `card_declined` → line 80<br>`unexpected_state`, `timeout`, `failure` → `on failure` | `pi`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 80 | `pi ← get_intent(…)` | `GET stripe /v1/payment_intents/{intent}`, `observes`, `idempotent` | 3 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `pi`: `requires_payment_method`, `requires_confirmation`, `requires_action`, `requires_capture`, `canceled` |
| 83 | `pi ← customer_authenticated()` | `event`, `observes` | — | 1 hour | `timeout` → line 84<br>`failure` → `on failure` | `pi`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 84 | `pi ← get_intent(…)` | `GET stripe /v1/payment_intents/{intent}`, `observes`, `idempotent` | 3 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `pi`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 89 | `pi ← cancel_intent(…)` | `POST stripe /v1/payment_intents/{intent}/cancel`, `sends cancel`, `key` | — | — | `unexpected_state` → line 90<br>`timeout`, `failure` → `on failure` | `pi`: `canceled` |
| 93 | `pi ← capture_intent(…)` | `POST stripe /v1/payment_intents/{intent}/capture`, `sends capture`, `key` | 2 times every 5 seconds (failure, timeout) | — | `unexpected_state` → line 94<br>`timeout`, `failure` → `on failure` | `pi`: `processing`, `succeeded` |
| 100 | `pi ← get_intent(…)` | `GET stripe /v1/payment_intents/{intent}`, `observes`, `idempotent` | 3 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `pi`: `requires_payment_method`, `processing`, `succeeded` |
| 112 | `pi ← cancel_intent(…)` | `POST stripe /v1/payment_intents/{intent}/cancel`, `sends cancel`, `key` | — | — | `unexpected_state` → line 113<br>`timeout`, `failure` → line 114 | `pi`: `canceled` |
| 122 | `pi ← cancel_intent(…)` | `POST stripe /v1/payment_intents/{intent}/cancel`, `sends cancel`, `key` | — | — | `unexpected_state` → line 123<br>`timeout`, `failure` → line 124 | `pi`: `canceled` |

## Ends

Every way the workflow can end, and what each case can be then, the events on the other side included.

| Line | End | `pi` |
|---:|---|---|
| 77 | `succeed outcome = awaiting_review` | not started |
| 91 | `fail CardDeclined` "The card could not be held" | `canceled` |
| 92 | `fail PaymentCanceled` "The PaymentIntent was canceled" | `canceled` |
| 94 | `fail HoldExpired` "The hold had expired by check-out" | `canceled` |
| 96 | `succeed outcome = stayed` | `succeeded` |
| 105 | `succeed outcome = stayed` | `succeeded` |
| 106 | `fail SettlementUnclear` "The capture has no clear outcome; handing it over to staff" `leaving pi` | handed over as it is: `requires_payment_method`, `processing`, `succeeded` |
| 114 | `fail CleanupFailed` "Releasing the hold failed; handing it over to staff" `leaving pi` | handed over as it is: `requires_payment_method`, `requires_confirmation`, `requires_action`, `requires_capture`, `canceled` |
| 115 | `fail SettlementUnclear` "Failed in the middle of the capture; handing it over to staff" `leaving pi` | handed over as it is: `requires_payment_method`, `processing`, `succeeded` |
| 115 | `on failure` runs to its end, and the workflow fails with the error that started it | not started, or `succeeded`, `canceled` |
| 124 | `fail CleanupFailed` "Releasing the hold failed; handing it over to staff" `leaving pi` | handed over as it is: `requires_payment_method`, `requires_confirmation`, `requires_action`, `requires_capture`, `canceled` |
| 125 | `fail SettlementUnclear` "Cancelled in the middle of the capture; handing it over to staff" `leaving pi` | handed over as it is: `requires_payment_method`, `processing`, `succeeded` |
| 125 | `on cancel` runs to its end, and the workflow ends cancelled | not started, or `succeeded`, `canceled` |

