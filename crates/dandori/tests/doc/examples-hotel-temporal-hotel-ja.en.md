# 宿泊 v1

宿泊の予約を受けたらカードに与信を取り、チェックアウトの日に売上を確定する。与信の額と、フロントが先に確認するかどうかは規則が決め、決済は Stripe の PaymentIntent で行う。Temporal 向けの版：Stripe の呼び出しは dandori が生成するアクティビティで、秘密鍵は Transport が付ける。お客さまの 3-D セキュアの手続きが済んだことは、Stripe の Webhook が予約の ID を宛先にして知らせる（event）。お客さまが予約を取り消すと、与信も取り消す（on cancel）

`examples/hotel/temporal/hotel.ja.flow`, drawn by `dandori doc`. Inputs: `予約: 予約`. Outputs: `結果: 結果`.

## flow

```mermaid
flowchart TD
    start(["宿泊 v1"])
    s1[["見積 = 与信(…)<br>rule 宿泊の与信額.rule"]]
    s2{{"match 見積.扱い"}}
    s3(["succeed 結果 = 確認待ち"])
    s4["決済 ← 決済を作る(…)<br>POST stripe /v1/payment_intents<br>starts · retry 2 times every 2 seconds"]
    s5["決済 ← 与信を取る(…)<br>POST stripe /v1/payment_intents/{intent}/confirm<br>sends confirm"]
    s6["決済 ← 決済を見る(…)<br>GET stripe /v1/payment_intents/{intent}<br>observes · retry 3 times every 2 seconds"]
    s7{{"match 決済.status"}}
    s8[/"決済 ← 本人認証の知らせ()<br>event<br>observes · timeout 1 hour"/]
    s9["決済 ← 決済を見る(…)<br>GET stripe /v1/payment_intents/{intent}<br>observes · retry 3 times every 2 seconds"]
    s11{{"match 決済.status"}}
    s12("wait until 予約.チェックアウト")
    s13["決済 ← 与信を取り消す(…)<br>POST stripe /v1/payment_intents/{intent}/cancel<br>sends cancel"]
    s15(["fail 与信不可<br>#quot;カードの与信が取れませんでした#quot;"])
    s16(["fail 決済の取消<br>#quot;PaymentIntent が取り消されていました#quot;"])
    s17["決済 ← 売上を確定する(…)<br>POST stripe /v1/payment_intents/{intent}/capture<br>sends capture · retry 2 times every 5 seconds"]
    s18(["fail 与信の有効期限切れ<br>#quot;チェックアウトまでに与信の有効期限が切れていました#quot;"])
    s19{{"match 決済.status"}}
    s20(["succeed 結果 = 宿泊済"])
    subgraph L21 ["repeat at most 12 times"]
        s22("wait 1 hour")
        s23["決済 ← 決済を見る(…)<br>GET stripe /v1/payment_intents/{intent}<br>observes · retry 3 times every 2 seconds"]
        s24{{"match 決済.status"}}
        s26(["break"])
    end
    s27{{"match 決済.status"}}
    s28(["succeed 結果 = 宿泊済"])
    s29(["fail 確定の結果不明<br>#quot;売上の確定の結果が分かりません。担当者に引き渡します#quot;<br>leaving 決済"])
    start --> s1
    s1 --> s2
    s2 -->|"確認"| s3
    s2 -->|"自動"| s4
    s4 --> s5
    s5 -.->|"on カード拒否"| s6
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
    s13 -.->|"on 状態の不一致"| s15
    s11 -->|"canceled"| s16
    s12 --> s17
    s17 -.->|"on 状態の不一致"| s18
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
    s30{{"match 決済.status"}}
    s32["決済 ← 与信を取り消す(…)<br>POST stripe /v1/payment_intents/{intent}/cancel<br>sends cancel"]
    s34(["fail 取消の失敗<br>#quot;与信を取り消せませんでした。担当者に引き渡します#quot;<br>leaving 決済"])
    s35(["fail 確定の結果不明<br>#quot;売上の確定の途中で失敗しました。担当者に引き渡します#quot;<br>leaving 決済"])
    onfEnd(["fails with the same error"])
    onf --> s30
    s30 -->|"requires_payment_method, requires_confirmation, requires_action, requires_capture"| s32
    s32 -.->|"on failure"| s34
    s30 -->|"processing"| s35
    s30 -->|"succeeded, canceled, none"| onfEnd
    s32 --> onfEnd
    s32 -.->|"on 状態の不一致"| onfEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s34,s35 bad
```

## on cancel

Runs when the workflow is cancelled, from the call or the wait the run is at. When it runs to its end, the workflow ends cancelled.

```mermaid
flowchart TD
    onc(["on cancel"])
    s36{{"match 決済.status"}}
    s38["決済 ← 与信を取り消す(…)<br>POST stripe /v1/payment_intents/{intent}/cancel<br>sends cancel"]
    s40(["fail 取消の失敗<br>#quot;与信を取り消せませんでした。担当者に引き渡します#quot;<br>leaving 決済"])
    s41(["fail 確定の結果不明<br>#quot;売上の確定の途中でキャンセルされました。担当者に引き渡します#quot;<br>leaving 決済"])
    oncEnd(["ends cancelled"])
    onc --> s36
    s36 -->|"requires_payment_method, requires_confirmation, requires_action, requires_capture"| s38
    s38 -.->|"on failure"| s40
    s36 -->|"processing"| s41
    s36 -->|"succeeded, canceled, none"| oncEnd
    s38 --> oncEnd
    s38 -.->|"on 状態の不一致"| oncEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s40,s41 bad
```

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails | Case after |
|---:|---|---|---|---|---|---|
| 75 | `見積 = 与信(…)` | rule `宿泊の与信額.rule` | 2 times, after 1 second and 2 (failure) | — | `timeout`, `failure` → `on failure` | — |
| 78 | `決済 ← 決済を作る(…)` | `POST stripe /v1/payment_intents`, `starts payment_intent.payment then attach`, `key` | 2 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `決済`: `requires_confirmation` |
| 79 | `決済 ← 与信を取る(…)` | `POST stripe /v1/payment_intents/{intent}/confirm`, `sends confirm`, `key` | — | — | `カード拒否` → line 80<br>`状態の不一致`, `timeout`, `failure` → `on failure` | `決済`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 80 | `決済 ← 決済を見る(…)` | `GET stripe /v1/payment_intents/{intent}`, `observes`, `idempotent` | 3 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `決済`: `requires_payment_method`, `requires_confirmation`, `requires_action`, `requires_capture`, `canceled` |
| 83 | `決済 ← 本人認証の知らせ()` | `event`, `observes` | — | 1 hour | `timeout` → line 84<br>`failure` → `on failure` | `決済`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 84 | `決済 ← 決済を見る(…)` | `GET stripe /v1/payment_intents/{intent}`, `observes`, `idempotent` | 3 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `決済`: `requires_payment_method`, `requires_action`, `requires_capture`, `canceled` |
| 89 | `決済 ← 与信を取り消す(…)` | `POST stripe /v1/payment_intents/{intent}/cancel`, `sends cancel`, `key` | — | — | `状態の不一致` → line 90<br>`timeout`, `failure` → `on failure` | `決済`: `canceled` |
| 93 | `決済 ← 売上を確定する(…)` | `POST stripe /v1/payment_intents/{intent}/capture`, `sends capture`, `key` | 2 times every 5 seconds (failure, timeout) | — | `状態の不一致` → line 94<br>`timeout`, `failure` → `on failure` | `決済`: `processing`, `succeeded` |
| 100 | `決済 ← 決済を見る(…)` | `GET stripe /v1/payment_intents/{intent}`, `observes`, `idempotent` | 3 times every 2 seconds (failure, timeout) | — | `timeout`, `failure` → `on failure` | `決済`: `requires_payment_method`, `processing`, `succeeded` |
| 112 | `決済 ← 与信を取り消す(…)` | `POST stripe /v1/payment_intents/{intent}/cancel`, `sends cancel`, `key` | — | — | `状態の不一致` → line 113<br>`timeout`, `failure` → line 114 | `決済`: `canceled` |
| 122 | `決済 ← 与信を取り消す(…)` | `POST stripe /v1/payment_intents/{intent}/cancel`, `sends cancel`, `key` | — | — | `状態の不一致` → line 123<br>`timeout`, `failure` → line 124 | `決済`: `canceled` |

## Ends

Every way the workflow can end, and what each case can be then, the events on the other side included.

| Line | End | `決済` |
|---:|---|---|
| 77 | `succeed 結果 = 確認待ち` | not started |
| 91 | `fail 与信不可` "カードの与信が取れませんでした" | `canceled` |
| 92 | `fail 決済の取消` "PaymentIntent が取り消されていました" | `canceled` |
| 94 | `fail 与信の有効期限切れ` "チェックアウトまでに与信の有効期限が切れていました" | `canceled` |
| 96 | `succeed 結果 = 宿泊済` | `succeeded` |
| 105 | `succeed 結果 = 宿泊済` | `succeeded` |
| 106 | `fail 確定の結果不明` "売上の確定の結果が分かりません。担当者に引き渡します" `leaving 決済` | handed over as it is: `requires_payment_method`, `processing`, `succeeded` |
| 114 | `fail 取消の失敗` "与信を取り消せませんでした。担当者に引き渡します" `leaving 決済` | handed over as it is: `requires_payment_method`, `requires_confirmation`, `requires_action`, `requires_capture`, `canceled` |
| 115 | `fail 確定の結果不明` "売上の確定の途中で失敗しました。担当者に引き渡します" `leaving 決済` | handed over as it is: `requires_payment_method`, `processing`, `succeeded` |
| 115 | `on failure` runs to its end, and the workflow fails with the error that started it | not started, or `succeeded`, `canceled` |
| 124 | `fail 取消の失敗` "与信を取り消せませんでした。担当者に引き渡します" `leaving 決済` | handed over as it is: `requires_payment_method`, `requires_confirmation`, `requires_action`, `requires_capture`, `canceled` |
| 125 | `fail 確定の結果不明` "売上の確定の途中でキャンセルされました。担当者に引き渡します" `leaving 決済` | handed over as it is: `requires_payment_method`, `processing`, `succeeded` |
| 125 | `on cancel` runs to its end, and the workflow ends cancelled | not started, or `succeeded`, `canceled` |

## Rules

The rules this workflow calls, as `rulec doc` renders them for whoever approves them.

<details>
<summary><code>与信</code> · 宿泊の与信額 v1 · <code>../rules/宿泊の与信額.rule</code></summary>

<!-- Generated by rulec 0.22.0 from 宿泊の与信額.rule (sha256:7c4298374720). This is a read-only rendering; the source of truth is the .rule file. Edits cannot be carried back (§1.6). -->
# Rule 宿泊の与信額 v1

予約のときにカードで押さえる額と、フロントの確認に回すかどうか。客室の一泊の額に泊数を掛ける。15 泊以上は確認に回す。書き下ろしの例

## Inputs

| Name | Type | Range | Notes |
|---|---|---|---|
| 客室 | 客室 (3 values) |  |  |
| 泊数 | number | 1 〜 30 |  |

## Outputs

| Name | Type | Rounding | Notes |
|---|---|---|---|
| 与信額 | money[円, incl_tax] | down(1円) |  |
| 扱い | 扱い (2 values) |  |  |

## Types

An enum is a **closed** finite set. Add a value, and every table that does not look at it fails the completeness check.

- **客室** (3 values) — standard, deluxe, suite
- **扱い** (2 values) — 自動, 確認

## Derived Values and Definitions

Intermediate values that can be placed in a table column. The expressions are as written in the source file, and the ranges are the declared ones.

| Name | Kind | Expression | Range | Notes |
|---|---|---|---|---|
| 与信額 | Definition | `一泊 × 泊数` |  |  |

## Table 一泊の額 (policy unique)

| Column | Source |
|---|---|
| 客室 | Input |
| → 一泊 | (this table only) |

| # | 客室 | → 一泊 (money[円, incl_tax]) |
|---|---|---|
| 1 | standard | 12000円 |
| 2 | deluxe | 18000円 |
| 3 | suite | 40000円 |

**What `rulec check` verified**

- Every combination of inputs matches some row (E101 completeness)
- There is no row that can never match (E102 unreachable row)
- No input matches two or more rows at once (E105 overlap). Reordering the rows does not change the meaning

## Table 扱いの判定 (policy unique)

| Column | Source |
|---|---|
| 泊数 | Input |
| → 扱い | Output of this rule |

| # | 泊数 | → 扱い (扱い) |
|---|---|---|
| 1 | <=14 | 自動 |
| 2 | >=15 | 確認 |

**What `rulec check` verified**

- Every combination of inputs matches some row (E101 completeness)
- There is no row that can never match (E102 unreachable row)
- No input matches two or more rows at once (E105 overlap). Reordering the rows does not change the meaning

## Examples (verified)

| 客室 | 泊数 | → 与信額 | 扱い |
|---|---|---|---|
| standard | 2 | 24000円 | 自動 |
| suite | 15 | 600000円 | 確認 |

`rulec check` ran these 2 examples through the reference evaluator, and every one produced the declared values (E107). The examples are an **executable specification**.

</details>

<details>
<summary><code>payment_intent</code> · payment_intent v1 · <code>../rules/payment_intent.rule</code></summary>

<!-- Generated by rulec 0.22.0 from payment_intent.rule (sha256:ec6477bfd9ec). This is a read-only rendering; the source of truth is the .rule file. Edits cannot be carried back (§1.6). -->
# Rule payment_intent v1

Where a Stripe PaymentIntent's status goes when it is confirmed, authenticated, captured or canceled, and when a delayed payment settles. Transcribed from Stripe's documentation

## Inputs

| Name | Type | Range | Notes |
|---|---|---|---|
| status | status (7 values) |  |  |
| event | event (7 values) |  | attach: a payment method is attached without confirming |
| limit_reached | bool |  | this confirmation is past the limit Stripe puts on one PaymentIntent, which varies |
| needs_action | bool |  | the payment method asks for a further step, such as 3D Secure |
| approved | bool |  | the issuer or the bank lets the attempt through; for a cancel, Stripe accepts it |
| delayed | bool |  | the payment method confirms success only days later, as a bank debit does |
| capture_method | capture_method (3 values) |  |  |
| confirmation_method | confirmation_method (2 values) |  |  |

## Outputs

| Name | Type | Rounding | Notes |
|---|---|---|---|
| next_status | status (7 values) |  |  |
| funds | funds (4 values) |  |  |
| refused | bool |  | the status does not take this event: an API call gets payment_intent_unexpected_state |

## Types

An enum is a **closed** finite set. Add a value, and every table that does not look at it fails the completeness check.

- **status** (7 values) — requires_payment_method, requires_confirmation, requires_action, processing, requires_capture, succeeded, canceled
- **event** (7 values) — attach, confirm, authenticate, settle, capture, cancel, expire
- **capture_method** (3 values) — automatic, automatic_async, manual
- **confirmation_method** (2 values) — automatic, manual
- **funds** (4 values) — untouched, held, captured, released

## Groups

A group is a named subset of an enum. One word written in a table cell stands for all the values below.

- **unconfirmed** (2 values) — requires_payment_method, requires_confirmation

This group covers 2 values of the 7 values of status; the following belong to no group: requires_action, processing, requires_capture, succeeded, canceled (counted from the declarations by this rendering).

## Table transition (policy unique)

lifecycle, status, and the API reference for each call

| Column | Source |
|---|---|
| status | Input |
| event | Input |
| limit_reached | Input |
| needs_action | Input |
| approved | Input |
| delayed | Input |
| capture_method | Input |
| confirmation_method | Input |
| → next_status | Output of this rule |
| → funds | Output of this rule |
| → refused | Output of this rule |

| # | status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status (status) | → funds (funds) | → refused (bool) | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | unconfirmed | attach | - | - | - | - | - | - | requires_confirmation | untouched | false | lifecycle; from requires_confirmation, assumed the same |
| 2 | unconfirmed | confirm | true | - | - | - | - | - | canceled | untouched | false | confirm |
| 3 | unconfirmed | confirm | false | true | - | - | - | - | requires_action | untouched | false | confirm |
| 4 | unconfirmed | confirm | false | false | false | - | - | - | requires_payment_method | untouched | false | confirm |
| 5 | unconfirmed | confirm | false | false | true | - | manual | - | requires_capture | held | false | confirm |
| 6 | unconfirmed | confirm | false | false | true | true | automatic, automatic_async | - | processing | untouched | false | lifecycle |
| 7 | unconfirmed | confirm | false | false | true | false | automatic, automatic_async | - | succeeded | captured | false | confirm |
| 8 | unconfirmed | cancel | - | - | - | - | - | - | canceled | untouched | false | cancel |
| 9 | unconfirmed | authenticate, settle, capture, expire | - | - | - | - | - | - | status | untouched | true | capture, errors |
| 10 | requires_action | authenticate | - | - | false | - | - | - | requires_payment_method | untouched | false | 3ds |
| 11 | requires_action | authenticate | - | - | true | - | - | manual | requires_confirmation | untouched | false | confirm |
| 12 | requires_action | authenticate | - | - | true | - | manual | automatic | requires_capture | held | false | 3ds |
| 13 | requires_action | authenticate | - | - | true | true | automatic, automatic_async | automatic | processing | untouched | false | lifecycle |
| 14 | requires_action | authenticate | - | - | true | false | automatic, automatic_async | automatic | succeeded | captured | false | 3ds, lifecycle |
| 15 | requires_action | cancel | - | - | - | - | - | - | canceled | untouched | false | cancel |
| 16 | requires_action | attach, confirm, settle, capture, expire | - | - | - | - | - | - | status | untouched | true | errors; for attach and confirm the pages say nothing, assumed refused |
| 17 | processing | settle | - | - | true | - | - | - | succeeded | captured | false | lifecycle, status |
| 18 | processing | settle | - | - | false | - | - | - | requires_payment_method | untouched | false | lifecycle |
| 19 | processing | cancel | - | - | true | - | - | - | canceled | untouched | false | lifecycle: ACH, ACSS, AU BECS, BACS, NZ BECS and SEPA, inside a window |
| 20 | processing | cancel | - | - | false | - | - | - | status | untouched | true | lifecycle; cancel says "in rare cases" |
| 21 | processing | attach, confirm, authenticate, capture, expire | - | - | - | - | - | - | status | untouched | true | errors |
| 22 | requires_capture | capture | - | - | - | false | - | - | succeeded | captured | false | capture, lifecycle |
| 23 | requires_capture | capture | - | - | - | true | - | - | processing | untouched | false | lifecycle names no method; hold says bank debits cannot be held |
| 24 | requires_capture | cancel | - | - | - | - | - | - | canceled | released | false | cancel |
| 25 | requires_capture | expire | - | - | - | - | - | - | canceled | released | false | hold, capture |
| 26 | requires_capture | attach, confirm, authenticate, settle | - | - | - | - | - | - | status | untouched | true | errors |
| 27 | succeeded | - | - | - | - | - | - | - | status | untouched | true | lifecycle, status: refunds go through the Refunds API |
| 28 | canceled | - | - | - | - | - | - | - | status | untouched | true | cancel, lifecycle |

Groups appearing in the cells of this table: **unconfirmed** (2 values). Their members are in the "Groups" section.

**What `rulec check` verified**

- Every combination of inputs matches some row (E101 completeness)
- There is no row that can never match (E102 unreachable row)
- No input matches two or more rows at once (E105 overlap). Reordering the rows does not change the meaning

## State machine: payment

This rule is one step of a state machine. Every call is passed the state (input status) and answers the next one (output next_status). **The caller keeps the state; the generated code keeps nothing.** Where a case goes is decided by table transition.

| Initial state | Final states |
|---|---|
| requires_payment_method | succeeded, canceled |

One case passes capture_method, confirmation_method with the same value from its first call to its last (`held`). The claims below are about the sequences of calls that keep it.

```mermaid
stateDiagram-v2
  state "requires_payment_method" as s0
  state "requires_confirmation" as s1
  state "requires_action" as s2
  state "processing" as s3
  state "requires_capture" as s4
  state "succeeded" as s5
  state "canceled" as s6
  [*] --> s0
  s0 --> s1: event attach
  s0 --> s5: event confirm, limit_reached false, needs_action false, approved true, delayed false, capture_method automatic, automatic_async
  s0 --> s4: event confirm, limit_reached false, needs_action false, approved true, capture_method manual
  s0 --> s3: event confirm, limit_reached false, needs_action false, approved true, delayed true, capture_method automatic, automatic_async
  s0 --> s2: event confirm, limit_reached false, needs_action true
  s0 --> s6: event confirm, limit_reached true / event cancel
  s1 --> s0: event confirm, limit_reached false, needs_action false, approved false
  s1 --> s5: event confirm, limit_reached false, needs_action false, approved true, delayed false, capture_method automatic, automatic_async
  s1 --> s4: event confirm, limit_reached false, needs_action false, approved true, capture_method manual
  s1 --> s3: event confirm, limit_reached false, needs_action false, approved true, delayed true, capture_method automatic, automatic_async
  s1 --> s2: event confirm, limit_reached false, needs_action true
  s1 --> s6: event confirm, limit_reached true / event cancel
  s2 --> s0: event authenticate, approved false
  s2 --> s5: event authenticate, approved true, delayed false, capture_method automatic, automatic_async, confirmation_method automatic
  s2 --> s1: event authenticate, approved true, confirmation_method manual
  s2 --> s4: event authenticate, approved true, capture_method manual, confirmation_method automatic
  s2 --> s3: event authenticate, approved true, delayed true, capture_method automatic, automatic_async, confirmation_method automatic
  s2 --> s6: event cancel
  s3 --> s0: event settle, approved false
  s3 --> s5: event settle, approved true
  s3 --> s6: event cancel, approved true
  s4 --> s5: event capture, delayed false
  s4 --> s3: event capture, delayed true
  s4 --> s6: event cancel / event expire
  s5 --> [*]
  s6 --> [*]
```

### Where each state goes

| State | Goes to (row of table transition) |
|---|---|
| requires_payment_method | event attach（row 1） → requires_confirmation / event confirm, limit_reached true（row 2） → canceled / event confirm, limit_reached false, needs_action true（row 3） → requires_action / event confirm, limit_reached false, needs_action false, approved false（row 4） → stays / event confirm, limit_reached false, needs_action false, approved true, capture_method manual（row 5） → requires_capture / event confirm, limit_reached false, needs_action false, approved true, delayed true, capture_method automatic, automatic_async（row 6） → processing / event confirm, limit_reached false, needs_action false, approved true, delayed false, capture_method automatic, automatic_async（row 7） → succeeded / event cancel（row 8） → canceled / event authenticate, settle, capture, expire（row 9） → stays |
| requires_confirmation | event attach（row 1） → stays / event confirm, limit_reached true（row 2） → canceled / event confirm, limit_reached false, needs_action true（row 3） → requires_action / event confirm, limit_reached false, needs_action false, approved false（row 4） → requires_payment_method / event confirm, limit_reached false, needs_action false, approved true, capture_method manual（row 5） → requires_capture / event confirm, limit_reached false, needs_action false, approved true, delayed true, capture_method automatic, automatic_async（row 6） → processing / event confirm, limit_reached false, needs_action false, approved true, delayed false, capture_method automatic, automatic_async（row 7） → succeeded / event cancel（row 8） → canceled / event authenticate, settle, capture, expire（row 9） → stays |
| requires_action | event authenticate, approved false（row 10） → requires_payment_method / event authenticate, approved true, confirmation_method manual（row 11） → requires_confirmation / event authenticate, approved true, capture_method manual, confirmation_method automatic（row 12） → requires_capture / event authenticate, approved true, delayed true, capture_method automatic, automatic_async, confirmation_method automatic（row 13） → processing / event authenticate, approved true, delayed false, capture_method automatic, automatic_async, confirmation_method automatic（row 14） → succeeded / event cancel（row 15） → canceled / event attach, confirm, settle, capture, expire（row 16） → stays |
| processing | event settle, approved true（row 17） → succeeded / event settle, approved false（row 18） → requires_payment_method / event cancel, approved true（row 19） → canceled / event cancel, approved false（row 20） → stays / event attach, confirm, authenticate, capture, expire（row 21） → stays |
| requires_capture | event capture, delayed false（row 22） → succeeded / event capture, delayed true（row 23） → processing / event cancel（row 24） → canceled / event expire（row 25） → canceled / event attach, confirm, authenticate, settle（row 26） → stays |
| succeeded | any call（row 27） → stays |
| canceled | any call（row 28） → stays |

### What `rulec check` proved about every sequence of calls

- From requires_payment_method a case can reach requires_payment_method, requires_confirmation, requires_action, processing, requires_capture, succeeded, canceled. Every state is reached.
- No call moves a case out of the final states succeeded, canceled.
- Whatever state a case reaches, a way to a final state remains.
- `never succeeded after canceled`: no sequence of calls reaches succeeded after canceled.
- `once funds captured`: funds is answered captured at most once in a case.

### Scenarios (verified)

**card_with_3ds**

| # | status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status | funds | refused |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | requires_payment_method | confirm | false | true | false | false | automatic_async | automatic | requires_action | untouched | false |
| 2 | requires_action | authenticate | false | false | true | false | automatic_async | automatic | succeeded | captured | false |

**authorize_then_capture**

| # | status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status | funds | refused |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | requires_payment_method | confirm | false | false | true | false | manual | automatic | requires_capture | held | false |
| 2 | requires_capture | capture | false | false | true | false | manual | automatic | succeeded | captured | false |

**hold_expires**

| # | status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status | funds | refused |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | requires_payment_method | confirm | false | false | true | false | manual | automatic | requires_capture | held | false |
| 2 | requires_capture | expire | false | false | false | false | manual | automatic | canceled | released | false |
| 3 | canceled | capture | false | false | true | false | manual | automatic | canceled | untouched | true |

**debit_fails_card_retries**

| # | status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status | funds | refused |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | requires_payment_method | confirm | false | false | true | true | automatic | automatic | processing | untouched | false |
| 2 | processing | cancel | false | false | false | true | automatic | automatic | processing | untouched | true |
| 3 | processing | settle | false | false | false | true | automatic | automatic | requires_payment_method | untouched | false |
| 4 | requires_payment_method | confirm | false | false | true | false | automatic | automatic | succeeded | captured | false |

**manual_confirmation**

| # | status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status | funds | refused |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | requires_payment_method | attach | false | false | false | false | automatic | manual | requires_confirmation | untouched | false |
| 2 | requires_confirmation | confirm | false | true | false | false | automatic | manual | requires_action | untouched | false |
| 3 | requires_action | authenticate | false | false | true | false | automatic | manual | requires_confirmation | untouched | false |
| 4 | requires_confirmation | confirm | false | false | true | false | automatic | manual | succeeded | captured | false |

**too_many_confirmations**

| # | status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status | funds | refused |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | requires_payment_method | confirm | false | false | false | false | automatic | automatic | requires_payment_method | untouched | false |
| 2 | requires_payment_method | confirm | true | false | false | false | automatic | automatic | canceled | untouched | false |
| 3 | canceled | confirm | false | false | true | false | automatic | automatic | canceled | untouched | true |

The first call starts in requires_payment_method, and every later one in the state the call before it answered (the left column). `rulec check` ran them in order through the reference evaluator, and every one produced the declared values (E107).

## Examples (verified)

| status | event | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | → next_status | funds | refused |
|---|---|---|---|---|---|---|---|---|---|---|
| requires_capture | cancel | false | false | false | false | manual | automatic | canceled | released | false |
| succeeded | cancel | false | false | false | false | automatic | automatic | succeeded | untouched | true |
| processing | cancel | false | false | true | true | automatic | automatic | canceled | untouched | false |

`rulec check` ran these 3 examples through the reference evaluator, and every one produced the declared values (E107). The examples are an **executable specification**.

</details>

