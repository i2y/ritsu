# Diagnostic codes

Written by `ritsu explain --all --format markdown`; do not edit.

<a id="e101"></a>

## E101 — A file that does not read as a .proto

**When**: A `.proto` of the project that ritsu's one reader of `.proto` files (ritsu-proto) cannot read: a `{` that is never closed, a statement without its `;`, a `syntax` it does not know, a proto2 `group`, a file that is not UTF-8. Every language reads a `.proto` with that reader, so none of them can read the file; a language that reads it says so where it does, in its own code too (rulec's E013, dandori's E016, sakai's E106, yuen's E205).

**Fix**: Correct it where it points, as a proto3 `.proto`; a file `buf build` builds, ritsu's reader reads.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`shop.proto`:

```proto
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
```

<a id="e201"></a>

## E201 — A call can give a rule values that break its precondition

**When**: Where a workflow calls a rule, the ranges dandori knows for the values it gives hold a combination that breaks one of the rule's preconditions, a relation between two inputs (`constraint`) (X2, DESIGN 7.4). The ranges are dandori's, gathered from every place a value comes from, read as dandori's E014 reads them. The rule's generated code refuses such a call at its door, so it would fail only when the workflow runs. The notes give the two ranges and the combination that breaks it.

**Fix**: Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result). If the precondition is the part that is wrong, correct the rule's `constraint`.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`refund_check.rule`:

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

`refund.flow`:

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int range >=0 <=10000
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

See also: [W201](#w201)

<a id="w201"></a>

## W201 — Whether a call keeps a rule's precondition cannot be decided

**When**: Where a workflow calls a rule, whether a precondition holds cannot be decided (X2): a value comes from a place with no range (a task's result without `range`, say), the precondition bounds the total or the length of a list (dandori knows no list's length), or it is the days of a koyomi date (dandori carries no range of dates). The rule's generated code checks such a precondition at its door when the workflow runs.

**Fix**: Give the place the value comes from a range (the `range` of a task's result or of the workflow's input). Where none can be given, leave it: the rule refuses at run time.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`refund_check.rule`:

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

`refund.flow`:

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

See also: [E201](#e201)

<a id="e202"></a>

## E202 — The days a koyomi date comes to fall outside a rule input's range

**When**: Where a workflow gives the result of a koyomi date to a rule's date input, a day koyomi counts that date coming to lies outside the range the rule declares for the input (X3 (a), DESIGN 7.5). koyomi computes the date on every input of its range, so the day outside is an exact example.

**Fix**: Widen the rule input's range, or make it `range from koyomi`, so that koyomi's days are the range (rulec's §15.174).

**Reproduction**: ritsu does not print this code yet; it has no reproduction

See also: [W202](#w202), [E205](#e205)

<a id="w202"></a>

## W202 — Whether the days of a koyomi date stay inside a rule input's range cannot be decided

**When**: koyomi does not count the days the date comes to (more input combinations than it checks, or an input where the computation stops), or the rule gives no range for the input (X3 (a)).

**Fix**: Narrow the inputs of the koyomi file so that koyomi can count them.

**Reproduction**: ritsu does not print this code yet; it has no reproduction

See also: [E202](#e202)

<a id="e203"></a>

## E203 — A rule's output can be an amount chobo does not take

**When**: Where a workflow gives a rule's numeric output to a chobo transfer as its amount, the output can be below 1 or above 2⁶³ − 1 (X4, DESIGN 7.6). chobo fails such a call rather than refusing it as a business outcome. The notes give an input of the rule that comes to that amount, from the rule's vectors.

**Fix**: Make the rule's amounts 1 or more (a negative one, such as a refund, is a transfer the other way), or branch before the transfer.

**Reproduction**: ritsu does not print this code yet; it has no reproduction

See also: [E204](#e204), [W204](#w204)

<a id="e204"></a>

## E204 — The task does not handle a refusal the transfer can come to

**When**: chobo's search finds a run in which an operation of the transfer is refused for a reason, with its amounts in the range of the rule's output, and the task that calls the transfer does not handle that reason as an error it declares (X4). The search goes as deep as chobo's check goes, and each reason it finds comes with the operations that lead to it.

**Fix**: Declare the reason as an error of the task, and handle it.

**Reproduction**: ritsu does not print this code yet; it has no reproduction

See also: [E203](#e203), [W204](#w204)

<a id="w204"></a>

## W204 — chobo's search finds no run for a refusal the task handles

**When**: The task handles a reason as a refusal of the transfer, and chobo's search finds no run that comes to it with the amounts in the range of the rule's output (X4). The search goes only as deep as chobo's check does, so it does not happen as far as that depth. A case that cannot be decided at all (no range for the amount, a book that does not pass) is this warning too.

**Fix**: If the reason cannot happen, drop it from the task's errors; if it happens only after a longer run, leave it.

**Reproduction**: ritsu does not print this code yet; it has no reproduction

See also: [E204](#e204)

<a id="e205"></a>

## E205 — A date given to a koyomi date is outside its input's range or its calendar's data

**When**: Where a workflow calls a koyomi date, the range of the date it gives falls outside the range of the koyomi input or of the days its calendar has data for (X6, DESIGN 7.8). The notes give the day outside and what it is outside of.

**Fix**: Widen the koyomi input's range (and the calendar's data), or narrow the range of the date given.

**Reproduction**: ritsu does not print this code yet; it has no reproduction

See also: [W205](#w205), [E202](#e202)

<a id="w205"></a>

## W205 — Whether a date given to a koyomi date stays inside its range cannot be decided

**When**: dandori does not know the range of the date a workflow gives a koyomi date (X6). koyomi refuses a date outside its range when the workflow runs.

**Fix**: Give the place the date comes from a range.

**Reproduction**: ritsu does not print this code yet; it has no reproduction

See also: [E205](#e205)
