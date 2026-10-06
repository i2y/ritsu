# payout v1

Pay a seller what their sales came to: the bank's API pays the account the seller keeps on file, and a model drafts the notice. The flow carries the account's id, and never its number or the name it is held in, which the bank's contract marks secret

`examples/payout/payout.flow`, drawn by `dandori doc`. Inputs: `seller_id: string`, `account_id: string`, `amount: money[JPY, incl_tax]`. Outputs: `payout_id: string`.

## flow

```mermaid
flowchart TD
    start(["payout v1"])
    s1["paid = pay(…)<br>connect bank PayoutService/Pay"]
    s2(["fail Refused<br>#quot;the bank refused the payout to the account {ac…"])
    s3["notice = draft_notice(…)<br>agent openai · gpt-5.4-mini<br>timeout 1 minute"]
    s4["send_notice(…)<br>lambda send-notice"]
    s5(["succeed payout_id = paid.payoutId"])
    start --> s1
    s1 -.->|"on refused"| s2
    s1 --> s3
    s3 --> s4
    s4 --> s5
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s5 ok
    class s2 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 34 | `paid = pay(…)` | `connect bank PayoutService/Pay`, `key` | — | — | `refused` → line 35<br>`timeout`, `failure` → the workflow fails |
| 36 | `notice = draft_notice(…)` | `agent openai · gpt-5.4-mini` | — | 1 minute | `timeout`, `failure` → the workflow fails |
| 37 | `send_notice(…)` | `lambda send-notice`, `idempotent` | — | — | `timeout`, `failure` → the workflow fails |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 35 | `fail Refused` "the bank refused the payout to the account {account_id}" |
| 38 | `succeed payout_id = paid.payoutId` |

