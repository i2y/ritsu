# 取消 v1

キャンセルのエッジケース：案件を片付ける on cancel、リトライとエラーの処理を素通りするキャンセル、並列のイテレーションの中のキャンセル、コールバックを待つあいだのキャンセル、on failure の最中のキャンセル。Temporal 向け

`tests/flows/cancel.flow`, drawn by `dandori doc`. Inputs: `注文ID: string`, `品目: list[string]`.

## flow

```mermaid
flowchart TD
    start(["取消 v1"])
    s1["受注 ← 注文を見る(…)<br>a task you write<br>observes"]
    subgraph L2 ["let 引当 = for 一品 in 品目 at most 3 in parallel · yield r.id"]
        s3["r = 引き当てる(…)<br>a task you write<br>retry 1 times every 1 second"]
        s4(["fail NotReserved<br>#quot;{一品} を引き当てられませんでした#quot;<br>leaving 受注"])
    end
    s5[/"受注 ← 配達を待つ(…)<br>a task you write, answered by a callback<br>observes · timeout 7 days"/]
    s6(["fail DeliveryLate<br>#quot;七日たっても配達の知らせがありません#quot;<br>leaving 受注"])
    s7{{"match 受注.状態"}}
    s9(["fail NotDelivered<br>#quot;配達されていません#quot;<br>leaving 受注"])
    fin(["end: succeeds"])
    start --> s1
    s1 --> s3
    s3 -.->|"on failure"| s4
    L2 -->|"every round done"| s5
    s5 -.->|"on timeout"| s6
    s5 --> s7
    s7 -->|"受付, 入金済, 出荷済"| s9
    s7 -->|"配達済, 取消"| fin
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s4,s6,s9 bad
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## on failure

Runs when a call fails and nothing at the call handles the error: from the calls on lines 46 and 51. When it runs to its end, the workflow fails with that error.

```mermaid
flowchart TD
    onf(["on failure"])
    s10["知らせる(…)<br>a task you write"]
    s12(["fail Stopped<br>#quot;途中で止まりました#quot;<br>leaving 受注"])
    onf --> s10
    s10 --> s12
    s10 -.->|"on failure"| s12
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s12 bad
```

## on cancel

Runs when the workflow is cancelled, from the call or the wait the run is at. When it runs to its end, the workflow ends cancelled.

```mermaid
flowchart TD
    onc(["on cancel"])
    s13{{"match 受注.状態"}}
    s15["受注 ← 取消を頼む(…)<br>a task you write<br>sends 取消依頼"]
    s17(["fail CancelFailed<br>#quot;注文 {受注.id} を取り消せませんでした#quot;<br>leaving 受注"])
    s18["知らせる(…)<br>a task you write"]
    s20(["fail ShippedAlready<br>#quot;注文 {受注.id} はもう出荷されています#quot;<br>leaving 受注"])
    oncEnd(["ends cancelled"])
    onc --> s13
    s13 -->|"受付, 入金済"| s15
    s15 -.->|"on failure"| s17
    s15 --> s18
    s15 -.->|"on 断られた"| s18
    s13 -->|"出荷済"| s20
    s13 -->|"none"| oncEnd
    s18 --> oncEnd
    s18 -.->|"on failure"| oncEnd
    s13 -->|"配達済, 取消"| oncEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s17,s20 bad
```

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails | Case after |
|---:|---|---|---|---|---|---|
| 46 | `受注 ← 注文を見る(…)` | a task you write, `observes`, `idempotent` | — | — | `timeout`, `failure` → `on failure` | `受注`: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 48 | `r = 引き当てる(…)` | a task you write, `idempotent` | 1 time every 1 second (failure, timeout) | — | `timeout`, `failure` → line 49 | — |
| 51 | `受注 ← 配達を待つ(…)` | a task you write, answered by a callback, `observes` | — | 7 days | `timeout` → line 52<br>`failure` → `on failure` | `受注`: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 58 | `知らせる(…)` | a task you write, `key` | — | — | `timeout`, `failure` → line 59 | — |
| 67 | `受注 ← 取消を頼む(…)` | a task you write, `sends 取消依頼`, `key` | — | — | `断られた` → line 68<br>`timeout`, `failure` → line 69 | `受注`: `取消` |
| 70 | `知らせる(…)` | a task you write, `key` | — | — | `timeout`, `failure` → line 71 | — |

## Ends

Every way the workflow can end, and what each case can be then, the events on the other side included.

| Line | End | `受注` |
|---:|---|---|
| 49 | `fail NotReserved` "{一品} を引き当てられませんでした" `leaving 受注` | handed over as it is: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 52 | `fail DeliveryLate` "七日たっても配達の知らせがありません" `leaving 受注` | handed over as it is: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 55 | `fail NotDelivered` "配達されていません" `leaving 受注` | handed over as it is: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 55 | the flow runs to its end, and the workflow succeeds | `配達済`, `取消` |
| 60 | `fail Stopped` "途中で止まりました" `leaving 受注` | handed over as it is: not started, or `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 69 | `fail CancelFailed` "注文 {受注.id} を取り消せませんでした" `leaving 受注` | handed over as it is: `受付`, `入金済`, `取消` |
| 72 | `fail ShippedAlready` "注文 {受注.id} はもう出荷されています" `leaving 受注` | handed over as it is: `出荷済`, `配達済` |
| 73 | `on cancel` runs to its end, and the workflow ends cancelled | not started, or `配達済`, `取消` |

## Rules

The rules this workflow calls, as `rulec doc` renders them for whoever approves them.

<details>
<summary><code>注文の状態</code> · 注文の状態 v1 · <code>../../examples/order/rules/注文の状態.rule</code></summary>

<!-- Generated by rulec 0.22.0 from 注文の状態.rule (sha256:a78989a24f22). This is a read-only rendering; the source of truth is the .rule file. Edits cannot be carried back (§1.6). -->
# Rule 注文の状態 v1

通販の注文が、入金・出荷・配達・取消依頼のイベントでどの状態に移り、いくら返金するか。状態は呼び出す側が持ち、規則は一回のイベントだけを判定する。書き下ろしの例

## Inputs

| Name | Type | Range | Notes |
|---|---|---|---|
| 状態 | 状態 (5 values) |  |  |
| イベント | イベント (4 values) |  |  |
| 支払額 | money[円, incl_tax] | 0円 〜 100万円 |  |

## Outputs

| Name | Type | Rounding | Notes |
|---|---|---|---|
| 次の状態 | 状態 (5 values) |  |  |
| 返金額 | money[円, incl_tax] | down(1円) |  |
| 受理 | bool |  |  |

## Types

An enum is a **closed** finite set. Add a value, and every table that does not look at it fails the completeness check.

- **状態** (5 values) — 受付, 入金済, 出荷済, 配達済, 取消
- **イベント** (4 values) — 入金, 出荷, 配達, 取消依頼

## Table 遷移 (policy unique)

| Column | Source |
|---|---|
| 状態 | Input |
| イベント | Input |
| → 次の状態 | Output of this rule |
| → 返金額 | Output of this rule |
| → 受理 | Output of this rule |

| # | 状態 | イベント | → 次の状態 (状態) | → 返金額 (money[円, incl_tax] / down(1円)) | → 受理 (bool) | Notes |
|---|---|---|---|---|---|---|
| 1 | 受付 | 入金 | 入金済 | 0円 | true |  |
| 2 | 受付 | 取消依頼 | 取消 | 0円 | true |  |
| 3 | 受付 | 出荷, 配達 | 状態 | 0円 | false |  |
| 4 | 入金済 | 出荷 | 出荷済 | 0円 | true |  |
| 5 | 入金済 | 取消依頼 | 取消 | 支払額 | true |  |
| 6 | 入金済 | 入金, 配達 | 状態 | 0円 | false |  |
| 7 | 出荷済 | 配達 | 配達済 | 0円 | true |  |
| 8 | 出荷済 | 入金, 出荷, 取消依頼 | 状態 | 0円 | false | 出荷のあとの取消は、返品の手続きで受ける |
| 9 | 配達済 | - | 状態 | 0円 | false |  |
| 10 | 取消 | - | 状態 | 0円 | false | 取消のあとに届いた入金の通知では、注文を戻さない |

**What `rulec check` verified**

- Every combination of inputs matches some row (E101 completeness)
- There is no row that can never match (E102 unreachable row)
- No input matches two or more rows at once (E105 overlap). Reordering the rows does not change the meaning

## State machine: 注文

This rule is one step of a state machine. Every call is passed the state (input 状態) and answers the next one (output 次の状態). **The caller keeps the state; the generated code keeps nothing.** Where a case goes is decided by table 遷移.

| Initial state | Final states |
|---|---|
| 受付 | 配達済, 取消 |

One case passes 支払額 with the same value from its first call to its last (`held`). The claims below are about the sequences of calls that keep it.

```mermaid
stateDiagram-v2
  state "受付" as s0
  state "入金済" as s1
  state "出荷済" as s2
  state "配達済" as s3
  state "取消" as s4
  [*] --> s0
  s0 --> s1: 入金
  s0 --> s4: 取消依頼
  s1 --> s2: 出荷
  s1 --> s4: 取消依頼
  s2 --> s3: 配達
  s3 --> [*]
  s4 --> [*]
```

### Where each state goes

| State | Goes to (row of table 遷移) |
|---|---|
| 受付 | 入金（row 1） → 入金済 / 取消依頼（row 2） → 取消 / 出荷, 配達（row 3） → stays |
| 入金済 | 出荷（row 4） → 出荷済 / 取消依頼（row 5） → 取消 / 入金, 配達（row 6） → stays |
| 出荷済 | 配達（row 7） → 配達済 / 入金, 出荷, 取消依頼（row 8） → stays |
| 配達済 | any call（row 9） → stays |
| 取消 | any call（row 10） → stays |

### What `rulec check` proved about every sequence of calls

- From 受付 a case can reach 受付, 入金済, 出荷済, 配達済, 取消. Every state is reached.
- No call moves a case out of the final states 配達済, 取消.
- Whatever state a case reaches, a way to a final state remains.
- `never 出荷済 after 取消`: no sequence of calls reaches 出荷済 after 取消.
- `once 返金額 >0円`: 返金額 is answered >0円 at most once in a case.

### Scenarios (verified)

**配達まで**

| # | 状態 | イベント | 支払額 | → 次の状態 | 返金額 | 受理 |
|---|---|---|---|---|---|---|
| 1 | 受付 | 入金 | 3000円 | 入金済 | 0円 | true |
| 2 | 入金済 | 出荷 | 3000円 | 出荷済 | 0円 | true |
| 3 | 出荷済 | 配達 | 3000円 | 配達済 | 0円 | true |

**取消のあとの入金**

| # | 状態 | イベント | 支払額 | → 次の状態 | 返金額 | 受理 |
|---|---|---|---|---|---|---|
| 1 | 受付 | 入金 | 3000円 | 入金済 | 0円 | true |
| 2 | 入金済 | 取消依頼 | 3000円 | 取消 | 3000円 | true |
| 3 | 取消 | 入金 | 3000円 | 取消 | 0円 | false |

The first call starts in 受付, and every later one in the state the call before it answered (the left column). `rulec check` ran them in order through the reference evaluator, and every one produced the declared values (E107).

## Examples (verified)

| 状態 | イベント | 支払額 | → 次の状態 | 返金額 | 受理 |
|---|---|---|---|---|---|
| 受付 | 取消依頼 | 0円 | 取消 | 0円 | true |
| 出荷済 | 取消依頼 | 3000円 | 出荷済 | 0円 | false |

`rulec check` ran these 2 examples through the reference evaluator, and every one produced the declared values (E107). The examples are an **executable specification**.

</details>

