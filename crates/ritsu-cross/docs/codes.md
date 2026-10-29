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

**When**: Where a workflow or a function of kaname calls a rule, it can give the rule values that break one of its preconditions (a relation between inputs, `constraint`; a bound on the total or the length of the list it walks; the days of `range from koyomi`). For a workflow, the ranges dandori knows for the values it gives hold a combination that breaks it. The ranges are dandori's, gathered from every place a value comes from, read as dandori's E014 reads them. A date input whose range is `range from koyomi` and is given the day of a koyomi date is this error too when one of those days is not one of the rule's. For a function of kaname, a run of the function's vectors gives values that break it: kaname's ranges are an estimate from above, so a corner of them alone is no example. The rule's generated code refuses such a call at its door, so it would fail only when it runs. The notes give the ranges and the combination or the day that breaks it, or the function's input that does.

**Fix**: Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result; in kaname, an `if` before the call). If the precondition is the part that is wrong, correct the rule's `constraint`.

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

`refund.kn`:

```kn
module refund v1
description "Whether a refund is paid at once or reviewed, as the rule decides for the amount the customer asks for"

use rule check from "refund_check.rule"

fn route(paid: number range >=0 <=10000, asked: number range >=0 <=10000) -> check.path
  check(paid: paid, asked: asked).route
```

See also: [W201](#w201)

<a id="w201"></a>

## W201 — Whether a call keeps a rule's precondition cannot be decided

**When**: Where a workflow or a function of kaname calls a rule, whether a precondition holds cannot be decided. For a workflow: a value comes from a place with no range (a task's result without `range`, say), the precondition bounds the total or the length of a list (dandori knows no list's length), or it is the days of a koyomi date and the value can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`). The workflow's code that `ritsu dandori build` writes checks such a precondition when the workflow runs, as soon as the values are made, and fails a run that breaks it with `Dandori.BrokenPrecondition`. For a function of kaname: neither the ranges kaname finds nor a relation it keeps between the inputs shows it, and no run of the function's vectors breaks it. kaname's generated code checks it there, and the function fails with `rule` where the rule refuses the call.

**Fix**: Give the place the value comes from a range (the `range` of a task's result or of the workflow's input). Where none can be given, leave it: the workflow's code checks it at run time. In kaname, branch on the precondition with an `if` before the call, or give values whose relation kaname keeps (the running totals of `running_totals`).

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

`refund.kn`:

```kn
module refund v1
description "Whether a refund is paid at once or reviewed, as the rule decides for the amount asked for, capped at what was paid"

use rule check from "refund_check.rule"

fn route(paid: number range >=0 <=10000, asked: number range >=0 <=10000) -> check.path
  check(paid: paid, asked: min(asked, paid)).route
```

See also: [E201](#e201)

<a id="e202"></a>

## E202 — A day given to a rule's date input falls outside its range

**When**: Where a workflow or a function of kaname gives a day to a rule's date input, the day can lie outside the range the rule declares for the input. For a workflow, the day of a koyomi date (`due.day`) is given, and a day koyomi counts that date coming to lies outside: koyomi computes the date on every input of its range, so the day outside is an exact example, and the notes give it with the input at which koyomi comes to it. For a function of kaname, a run of the function's vectors gives a day outside, of the days of the koyomi dates the value comes from or of the range kaname finds; the notes give the function's input and the day. An input whose range is `range from koyomi` takes the days as a precondition of the rule, which E201 and W201 hold the call to.

**Fix**: Widen the rule input's range, or make it `range from koyomi`, so that koyomi's days are the range.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`batch.rule`:

```rule
rule batch v1
description "The billing batch a payment day falls in"

enum run = spring | autumn

inputs
  pay_day : date  range >=2026-03-01 <=2027-01-31

outputs
  batch : run

table pick
policy unique
| pay_day      | -> batch : run |
| <=2026-08-31 | spring         |
| >=2026-09-01 | autumn         |
```

`billing.flow`:

```flow
workflow billing v1
description "Bills an order in the batch the rule picks for the day its payment is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use rule batch from "batch.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:batch"

inputs
  order    : string
  received : date

task bill(order: string, due: date, run: batch.run)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:bill"
  key

flow
  let due = terms.payment(received: received)
  let pick = batch(pay_day: due.day)
  bill(order: order, due: due.day, run: pick.batch)
```

`billing.kn`:

```kn
module billing v1
description "The billing batch of the day an order's payment is due"

use dates terms from "payment_terms.cal"
use rule batch from "batch.rule"

fn pick(received: date range >=2026-01-01 <=2026-12-20) -> batch.run
  let due = terms.payment(received: received)
  batch(pay_day: due).batch
```

See also: [W202](#w202), [E205](#e205)

<a id="w202"></a>

## W202 — Whether a day given to a rule's date input stays inside its range cannot be decided

**When**: The value given to a rule's date input can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`) as well as from a koyomi date, or koyomi does not count the days of the date (more input combinations than it checks, or an input where the computation stops). For a function of kaname, it is also the days kaname's ranges find holding one outside, which no run of the function's vectors gives. The rule's generated code checks the day at its door when it runs.

**Fix**: Give the input days of koyomi dates only, and it can be decided; where koyomi does not count them, narrow the inputs of its file.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`batch.rule`:

```rule
rule batch v1
description "The billing batch a payment day falls in"

enum run = spring | autumn

inputs
  pay_day : date  range >=2026-02-01 <=2027-01-31

outputs
  batch : run

table pick
policy unique
| pay_day      | -> batch : run |
| <=2026-08-31 | spring         |
| >=2026-09-01 | autumn         |
```

`billing.flow`:

```flow
workflow billing v1
description "Bills an order in the batch the rule picks for the day its payment is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use rule batch from "batch.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:batch"

inputs
  order    : string
  received : date
  at_once  : bool

task bill(order: string, due: date, run: batch.run)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:bill"
  key

flow
  let due = terms.payment(received: received)
  match at_once
    true => let day = received
    false => let day = due.day
  let pick = batch(pay_day: day)
  bill(order: order, due: day, run: pick.batch)
```

`billing.kn`:

```kn
module billing v1
description "The billing batch of an order: that of the day the customer promised to pay, when it is no earlier than the payment day, and that of the payment day otherwise"

use dates terms from "payment_terms.cal"
use rule batch from "batch.rule"

fn pick(received: date range >=2026-01-01 <=2026-12-20, promised: date range >=2026-01-01 <=2027-01-31) -> batch.run
  let due = terms.payment(received: received)
  let grace = promised - due
  if grace >= 0
    batch(pay_day: promised).batch
  else
    batch(pay_day: due).batch
```

See also: [E202](#e202)

<a id="e203"></a>

## E203 — An amount given to a transfer can be one chobo does not take

**When**: An amount of a transfer can be below 0 or above 2⁶³ − 1 (chobo takes 0 to 2⁶³ − 1), and chobo fails such a call rather than refusing it as a business outcome. For a workflow, it is checked where a rule's numeric output is given to a chobo transfer as its amount, and it is this error where an input from the rule's vectors comes to such an amount; an end of the range rulec counts, or a number a row writes, is not enough (W203). The notes give that input and the amount. For a function of kaname, every amount is checked, a rule's output or one kaname computes, and it is this error where a run of the function's vectors comes to such an amount; the notes give the function's input and the amount.

**Fix**: Make the amounts 0 or more (a negative one, such as a refund, is a transfer the other way). In kaname, the amount can also be narrowed with an `if` before the transfer.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs; a handback gives seats back"

enum kind = handback | workshop | talk

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| handback   | -20                |
| workshop   | 30                 |
| talk       | 80                 |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do

flow
  let need = seats(event_kind: kind)
  give_seats(event: event, count: need.needed)
```

`booking.kn`:

```kn
module booking v1
description "The seats to give an event, as the rule says for its kind: the transfer the host makes"

use rule seats from "seats.rule"
use book hall from "hall.book"

fn give(event: string, kind: seats.kind) -> hall.assign.do
  hall.assign.do(event: event, count: seats(event_kind: kind).needed)
```

See also: [W203](#w203), [E204](#e204)

<a id="w203"></a>

## W203 — Whether chobo takes an amount given to a transfer cannot be decided

**When**: Where a workflow gives a rule's numeric output to a transfer as its amount, the output's range has an open end, or the value can also come from somewhere with no range (a task's answer without one, say). It is also this warning where rulec counts the output's range outside what chobo takes and no input from the rule's vectors comes to such an amount: rulec bounds an output by moving each input over its own range, and does not narrow the bound by a `constraint` that ties two inputs, so the output may never come to one. An amount chobo does not take fails the call when the workflow runs. For a function of kaname, the amounts kaname's ranges find do not stay inside what chobo takes, and no run of the function's vectors comes to one outside; kaname's generated code checks the amount where it builds the transfer, and the function fails with `range` for one chobo does not take.

**Fix**: Give the places the value comes from a range (the `range` of a task's answer or of the workflow's input). Where rulec's range of the output reaches outside, write the rule so that rulec counts the output as 0 or more (`max(…, 0)`, say), or make a negative amount a transfer the other way. In kaname, narrow the amount with an `if` before the transfer.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs"

enum kind = workshop | talk

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| workshop   | 30                 |
| talk       | 80                 |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do

task ask_organiser(event: string) -> int
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-organiser"
  idempotent

flow
  let need = seats(event_kind: kind)
  let count = need.needed
  match kind
    workshop => let count = ask_organiser(event: event)
    talk => pass
  give_seats(event: event, count: count)
```

`booking.kn`:

```kn
module booking v1
description "The seats to give an event: what the rule says its kind needs, less those it holds already, which are never more than it needs"

use rule seats from "seats.rule"
use book hall from "hall.book"

fn give(event: string, kind: seats.kind, held: number range >=0 <=50) -> hall.assign.do
  let needed = seats(event_kind: kind).needed
  hall.assign.do(event: event, count: needed - min(held, needed))
```

See also: [E203](#e203)

<a id="e204"></a>

## E204 — The task does not handle a refusal the transfer can come to

**When**: Where a `do` or a `hold` is given a rule's output as its amount, chobo's search, with the amounts held to the range the call gives, finds a run in which the operation is refused for a reason the task does not handle as an error it declares. Only the reasons of the book's bounds (an account's `refused as`) are compared, the refusals that turn on the amounts; one that turns on the calls made before, such as a key used again with other arguments, is not. The search goes as deep as chobo's check goes. A `post` and a `void` are refused for the state their hold is in, which dandori follows with the case (dandori's E022). A transfer a function of kaname builds does not get this error: the host that applies it handles a refusal, and the reasons the search finds go on the type kaname generates for the transfer (W204).

**Fix**: Declare the reason as an error of the task, and handle it.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs"

enum kind = workshop | talk | concert

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| workshop   | 30                 |
| talk       | 80                 |
| concert    | 400                |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do

flow
  let need = seats(event_kind: kind)
  give_seats(event: event, count: need.needed)
```

See also: [E203](#e203), [W204](#w204)

<a id="w204"></a>

## W204 — Which refusals a transfer can come to cannot be decided

**When**: The task handles a reason of the book's bounds, and chobo's search, with the amounts held to the range the call gives, finds no run that comes to it. The search goes only as deep as chobo's check does, so all it shows is that the reason does not come within that depth. A case that cannot be decided at all (no range for the amounts, a book that does not answer) is this warning too. For a `do` or a `hold` a function of kaname builds, the host that applies the transfer handles a refusal, and the reasons the search finds go on the type kaname generates for the transfer; only a case that cannot be decided is this warning, and the type then lists every reason of the book's bounds.

**Fix**: If the reason cannot happen, drop it from the task's errors; if it happens only after a longer run, leave it.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs"

enum kind = workshop | talk

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| workshop   | 30                 |
| talk       | 80                 |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do
  errors over_capacity

flow
  let need = seats(event_kind: kind)
  give_seats(event: event, count: need.needed)
    on over_capacity => fail OverCapacity "the hall cannot seat the event"
```

See also: [E204](#e204)

<a id="e205"></a>

## E205 — A day given to a koyomi date is outside its input's range

**When**: Where a workflow or a function of kaname calls a koyomi date, the day it gives the date input can be outside the range of that input. A day of another koyomi date is held to it with every day koyomi counts for that date. A day inside the range is one koyomi's own check has held to the data of its calendar wherever it asks the calendar (koyomi's E203). For a workflow, the notes give the day outside and the input at which koyomi comes to it; for a function of kaname, a run of the function's vectors gives a day outside, and the notes give the function's input and the day.

**Fix**: Widen the koyomi input's range (and the calendar's data), or give it a day that stays inside (in kaname, narrow it with an `if` before the call).

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`reminders.cal`:

```cal
dates reminders v1
description "A reminder a week before a payment is due"

inputs
  due : date  range >=2026-02-01 <=2026-12-31

date reminder = due
  - 7 days              # a week before
```

`reminding.flow`:

```flow
workflow reminding v1
description "Reminds a customer a week before the payment of an order is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use dates reminders from "reminders.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:reminders"

inputs
  order    : string
  received : date

task remind(order: string, on: date)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:remind"
  key

flow
  let due = terms.payment(received: received)
  let note = reminders.reminder(due: due.day)
  remind(order: order, on: note.day)
```

`reminding.kn`:

```kn
module reminding v1
description "The day to remind a customer, a week before an order's payment is due"

use dates terms from "payment_terms.cal"
use dates reminders from "reminders.cal"

fn remind(received: date range >=2026-01-01 <=2026-12-20) -> date
  let due = terms.payment(received: received)
  reminders.reminder(due: due)
```

See also: [W205](#w205), [E202](#e202)

<a id="w205"></a>

## W205 — Whether a day given to a koyomi date stays inside its range cannot be decided

**When**: dandori does not know the day a workflow gives a koyomi date's input: a day that comes from an input of the workflow, a task's answer or `now` says nothing of what day it is, and dandori has no way yet to write the range of a date; koyomi's generated code refuses a day outside its range when the workflow runs. For a function of kaname, the days kaname's ranges find hold one outside the range, which no run of the function's vectors gives; kaname's generated code checks it there, and the function fails with `date` where koyomi refuses the day.

**Fix**: Give it the day of another koyomi date, and it can be decided; otherwise leave it: koyomi refuses at run time.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`reminders.cal`:

```cal
dates reminders v1
description "A reminder a week before a payment is due"

inputs
  due : date  range >=2026-02-01 <=2027-01-31

date reminder = due
  - 7 days              # a week before
```

`reminding.flow`:

```flow
workflow reminding v1
description "Reminds a customer a week before the payment of an order is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use dates reminders from "reminders.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:reminders"

inputs
  order    : string
  received : date

task remind(order: string, on: date)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:remind"
  key

flow
  let due = terms.payment(received: received)
  let note = reminders.reminder(due: due.day)
  remind(order: order, on: note.day)
```

`reminding.kn`:

```kn
module reminding v1
description "The day to remind a customer: a week before the day the customer promised to pay, when it is no earlier than the payment day, and a week before the payment day otherwise"

use dates terms from "payment_terms.cal"
use dates reminders from "reminders.cal"

fn remind(received: date range >=2026-01-01 <=2026-12-20, promised: date range >=2026-01-01 <=2027-01-31) -> date
  let due = terms.payment(received: received)
  let grace = promised - due
  if grace >= 0
    reminders.reminder(due: promised)
  else
    reminders.reminder(due: due)
```

See also: [E205](#e205)

<a id="e206"></a>

## E206 — A hold has always expired when a call on it comes

**When**: Where a workflow follows a chobo hold as a case (`case … follows <book>.<transfer>`) and posts or voids it, the fewest seconds from making the hold to the call are at least the hold's expiry (the transfer's `pending expires after`). The book refuses the call with `expired` on every run, and what follows it going through never runs. dandori counts the time from the statements of the flow: a `wait` of a fixed time takes that time; a `wait until` the time of a koyomi date, whose date input was given `now` read after the hold, takes from the fewest to the most days koyomi counts from the input to the date, at the date's time; and the `on timeout` of a task that waits for an event runs once the task's `timeout` (a day without one) has passed, or, with `timeout until` the time of such a koyomi date, once that time has come. The notes give the fewest and the statements that make them up.

**Fix**: Make the hold last longer (the transfer's `pending expires after`), or make the call sooner.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`weekdays.cal`:

```cal
calendar weekdays v1
description "A business that keeps its days in UTC, open Monday to Friday"
offset +00:00

closed weekly sat, sun
```

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month at 09:00, or on the business day before when that day is closed"
use calendar "weekdays.cal"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
  roll preceding        # or on the business day before
  at 09:00
```

`stock.book`:

```book
book stock v1
description "Stock per SKU. An order holds what it takes for 14 days at most; shipping posts the hold"

unit pcs

account shelf(sku: string) : pcs
  description "what is on the shelves"
  at least 0 refused as out_of_stock
account suppliers : pcs outside
account customers : pcs outside

transfer receive(delivery: string, sku: string, qty: pcs)
  key delivery, sku
  move qty from suppliers to shelf(sku)

transfer reserve(order: string, sku: string, qty: pcs)
  description "posted when the order ships"
  key order, sku
  pending expires after 14 days
  move qty from shelf(sku) to customers
```

`invoice.flow`:

```flow
workflow invoice v1
description "Holds an order's goods until its payment is due, then ships them"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use book stock from "stock.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:stock"

inputs
  order : string
  sku   : string
  qty   : int  range >=1 <=100

task reserve(order: string, sku: string, qty: int) -> stock.reserve
  book stock.reserve.hold
  starts stock.reserve
  errors out_of_stock

task ship(order: string, sku: string) -> stock.reserve
  book stock.reserve.post
  sends post
  errors expired

case goods : stock.reserve follows stock.reserve

flow
  goods <- reserve(order: order, sku: sku, qty: qty)
    on out_of_stock => fail OutOfStock "nothing left on the shelf"
  let due = terms.payment(received: now)
  wait until due.at
  goods <- ship(order: order, sku: sku)
    on expired => fail Expired "the hold expired before the payment was due"
```

See also: [W206](#w206)

<a id="w206"></a>

## W206 — Whether a hold has expired when a call on it comes cannot be decided

**When**: The time from making a hold to posting or voiding it can fall either side of the hold's expiry, or nothing bounds it. The most comes from the `timeout` and the retries of the task that makes the hold and of the tasks between; a call of a rule or a date takes 33 seconds at the most (ten seconds a try on every target, retried twice, one second apart and then two); a task with `timeout until` waits until its time, counted as a `wait until` is. A task with no `timeout`, or a `wait until` or a `timeout until` a time nothing bounds, leaves no most. When koyomi's days make the most, a note gives the day that makes it the longest. An expired hold is refused with `expired`, which the flow handles (dandori's E022). When the call is shown to come before the hold expires (the most is shorter than the expiry), nothing is said.

**Fix**: Give the task that makes the hold, and the tasks between, a `timeout`, and the most is known; where none can be given, leave it.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`weekdays.cal`:

```cal
calendar weekdays v1
description "A business that keeps its days in UTC, open Monday to Friday"
offset +00:00

closed weekly sat, sun
```

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month at 09:00, or on the business day before when that day is closed"
use calendar "weekdays.cal"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
  roll preceding        # or on the business day before
  at 09:00
```

`stock.book`:

```book
book stock v1
description "Stock per SKU. An order holds what it takes for 60 days at most; shipping posts the hold"

unit pcs

account shelf(sku: string) : pcs
  description "what is on the shelves"
  at least 0 refused as out_of_stock
account suppliers : pcs outside
account customers : pcs outside

transfer receive(delivery: string, sku: string, qty: pcs)
  key delivery, sku
  move qty from suppliers to shelf(sku)

transfer reserve(order: string, sku: string, qty: pcs)
  description "posted when the order ships"
  key order, sku
  pending expires after 60 days
  move qty from shelf(sku) to customers
```

`invoice.flow`:

```flow
workflow invoice v1
description "Holds an order's goods until its payment is due, then ships them"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use book stock from "stock.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:stock"

inputs
  order : string
  sku   : string
  qty   : int  range >=1 <=100

task reserve(order: string, sku: string, qty: int) -> stock.reserve
  book stock.reserve.hold
  starts stock.reserve
  errors out_of_stock

task ship(order: string, sku: string) -> stock.reserve
  book stock.reserve.post
  sends post
  errors expired

case goods : stock.reserve follows stock.reserve

flow
  goods <- reserve(order: order, sku: sku, qty: qty)
    on out_of_stock => fail OutOfStock "nothing left on the shelf"
  let due = terms.payment(received: now)
  wait until due.at
  goods <- ship(order: order, sku: sku)
    on expired => fail Expired "the hold expired before the payment was due"
```

See also: [E206](#e206)

<a id="e301"></a>

## E301 — A `ritsu.mod` that does not read

**When**: A module's `ritsu.mod` that ritsu cannot read: a line that starts with a word ritsu does not know, a word with no value, an escape in a string other than `\"` and `\\`, a second statement of a word written once at most (`module`, `version`, `description`, `ritsu`, `license`, `source`), no `module` or a `module` that is not the first statement, a full-width space outside a string, a control character other than a tab, bytes that are not UTF-8, a target of `library` ritsu does not know. When the `ritsu.mod` of a module of the tree does not read, the tree of dependencies is not known, and ritsu stops before any language checks a file (exit 1).

**Fix**: Correct the line it points at. A statement starts with one of `module`, `version`, `description`, `ritsu`, `license`, `source`, `require`, `exclude`, `signature` and `library`, and the first is `module`.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module shop
version 1.0.0

requires tax path "tax"
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

See also: [E302](#e302)

<a id="e302"></a>

## E302 — A value of a `ritsu.mod` is not of its form

**When**: A value of a `ritsu.mod` that is not of its form: a module's name, a version, a dependency's name, an absolute path in `require … path`, the start of the names a `signature` covers, a name in `library`, the version of `ritsu >=`, a dependency's name written twice, an empty `license`, a `source` that is not https, the path of an `exclude` or of a key file, the URL and the `dir` of a `require … git`. A git URL is https or ssh (the scp form too); `git://`, `file://`, a local path, the form `<transport>::`, plain http to anything but this machine and a value that starts with `-` are not taken, nor a URL and a `dir` that give a module name with an uppercase letter. An https URL with a user or a password, an ssh URL with a password and a URL with a query may carry a credential, so the diagnostic shows neither the URL nor the line. A `require … path` that leads to a directory with no `ritsu.mod` is this error too, as a value that names the wrong thing. ritsu stops before any language checks a file (exit 1).

**Fix**: Write the value in the form of its statement. A dependency's name is a word of up to 64 lowercase letters, digits and `_` that starts with a letter; a version is of the form of SemVer, `1.4.0`; a dependency by a path names a directory with a `ritsu.mod` in it, by a path from the module's root. Keep the credentials of a dependency from git out of `ritsu.mod`, and leave them to git's credential helper or an ssh key.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module shop

require tax path "tax"
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

`tax/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 20%                     |
| reduced  | 5%                      |
```

See also: [E301](#e301)

<a id="e303"></a>

## E303 — A dependency from a registry or a git repository, and no `ritsu.lock`

**When**: A module of the tree takes a dependency from a registry or a git repository (`require … oci` or `require … git`), and the module at hand has no `ritsu.lock`, or one that does not read. `ritsu.lock` pins the version and the digests of each such dependency (and the commit of one from git), so without it which files to read is not known. A lock does not read when it is not of its form (a line ritsu does not know, a digest that is not one). ritsu stops before any language checks a file (exit 1).

**Fix**: Run `ritsu mod lock` to write `ritsu.lock`; do not write the lock by hand.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module example.com/acme/shop
version 1.0.0

require tax oci "example.com/acme/tax" 1.0.0
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

See also: [E304](#e304)

<a id="e304"></a>

## E304 — `ritsu.lock` does not match `ritsu.mod`

**When**: `ritsu.lock` does not match the `ritsu.mod` of a module of the tree: a module the lock has no line for, a version in the lock below the least a `require` asks for, a source of another kind or another path, a `require … git` and a lock line of two URLs, a version in the lock other than the `version` of the `ritsu.mod` fetched, a line for the module at hand itself, no line for a dependency by a path where there is a lock, an `oci` or a `git` line for a module read from a path. It comes when a `ritsu.mod` was edited by hand and the lock was not written again. ritsu stops before any language checks a file (exit 1).

**Fix**: Run `ritsu mod lock` to write the lock again.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module example.com/acme/shop
version 1.0.0

require tax oci "example.com/acme/tax" 1.2.0
```

`ritsu.lock`:

```
# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.
lock 1
ritsu 0.36.0
oci example.com/acme/tax 1.0.0 sha256:ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524 files sha256:371cad84ba72971bb3282231b913a7301282eb675fffbf647f18ccda43b6bcf1
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

See also: [E303](#e303), [W302](#w302)

<a id="e305"></a>

## E305 — The files in the store or in the module's copy differ from the lock's digest

**When**: The files of a dependency from a registry, read from the store or from the module's own copy of it (`.ritsu/modules/`, which `ritsu mod vendor` makes), have a digest other than the `files` digest of `ritsu.lock`: someone edited a file of the copy, or a file is damaged. They are not the contents the lock pins, so ritsu does not read them, and stops before any language checks a file (exit 1).

**Fix**: For the module's copy, run `ritsu mod vendor` to copy the files again; for the store, run `ritsu mod clean`, then `ritsu mod fetch` to fetch them again. A dependency is corrected in its own module; to try a correction here first, make its `require` a `require … path` to the corrected directory.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module example.com/acme/shop
version 1.0.0

require tax oci "example.com/acme/tax" 1.0.0
```

`ritsu.lock`:

```
# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.
lock 1
ritsu 0.36.0
oci example.com/acme/tax 1.0.0 sha256:ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524 files sha256:371cad84ba72971bb3282231b913a7301282eb675fffbf647f18ccda43b6bcf1
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

`.ritsu/modules/sha256/ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524/files/ritsu.mod`:

```
module example.com/acme/tax
version 1.0.0
```

`.ritsu/modules/sha256/ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524/files/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 25%                     |
| reduced  | 5%                      |
```

See also: [E304](#e304)

<a id="e306"></a>

## E306 — A module of one name comes from two places

**When**: Within the tree, a module of one name comes from two places: two `require … path` lead to two directories whose `ritsu.mod` give the same `module`; a `ritsu.mod` fetched gives a `module` other than the repository it was fetched from (from git, the name the URL and the `dir` give); the files fetched have no `ritsu.mod`. Two dependencies that require a module of one name from two sources (a registry and git, two URLs), with neither written in the module at hand, are this error too. A tag whose `ritsu.mod`, read by `ritsu mod lock` to choose the versions, gives another `version` is this error too: the two places disagree. A module is known by its name, so one name has one content. ritsu stops before any language checks a file (exit 1).

**Fix**: Read the module from one place: give a directory that is a copy a name of its own in `module`, or keep one `require`; where dependencies require it from two sources, a `require` of the module at hand that writes the source chooses it; for a module fetched, tell its author.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module shop

require tax path "tax"
require vat path "vat"
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

`tax/ritsu.mod`:

```
module tax
```

`tax/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 20%                     |
| reduced  | 5%                      |
```

`vat/ritsu.mod`:

```
module tax
```

`vat/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 20%                     |
| reduced  | 5%                      |
```

<a id="e307"></a>

## E307 — A name that starts with `@` at the root of a module

**When**: Right under the root of a module, a file or a directory has a name that starts with `@`. Within a module, `@<name>` names a dependency, so a file of that name cannot be read: in ritsu's layer of files, a dependency is seen there. The tree is still read and the other files are checked as ever, and the error makes the exit 1.

**Fix**: Rename it to a name that does not start with `@`.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module shop
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

`@drafts/fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

<a id="e308"></a>

## E308 — A module fetched has a dependency it cannot have

**When**: The `ritsu.mod` of a module from a registry or a git repository has a `require … path`, or one of a module from a registry has a `require … git`. A dependency by a path is read only within one working tree, and where the module is fetched, nothing is at the end of the path; the one who takes a module from a registry may not reach the git repository, and a dependency from git has no signature to check. ritsu stops before any language checks a file (exit 1).

**Fix**: Tell the author of the module: a module pushed to a registry writes each of its dependencies as `require … oci`, and one served from git writes them with a source that is not a path. Until a version that does is out, the module can be put in the working tree and read with `require … path`.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module example.com/acme/shop
version 1.0.0

require tax oci "example.com/acme/tax" 1.0.0
```

`ritsu.lock`:

```
# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.
lock 1
ritsu 0.36.0
oci example.com/acme/tax 1.0.0 sha256:ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524 files sha256:a744d0c29acba47340ec3ca4332ad67719ddb5996c3ec96672573273b8036a85
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

`.ritsu/modules/sha256/ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524/files/ritsu.mod`:

```
module example.com/acme/tax
version 1.0.0

require rates path "../rates"
```

`.ritsu/modules/sha256/ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524/files/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 20%                     |
| reduced  | 5%                      |
```

<a id="e309"></a>

## E309 — A `signature` line covers a module from a git repository

**When**: In the `ritsu.mod` of the module at hand, or of a module that requires it, the start of a `signature` line covers the name of a module from a git repository. ritsu checks the signatures of modules from registries only, and a dependency from git has no signature to check; a module a line says to check is not read unchecked, so `ritsu mod fetch` stops before it fetches anything (exit 1), and a check stops before any language checks a file (exit 1). A `signature` line of the list of `ritsu mod site` makes the list one that cannot be read (exit 2).

**Fix**: Narrow the start so that it covers no dependency from git (a line can cover one module, as `signature "example.com/acme/pricing" …` does); take a module whose signature is to be checked from a registry.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module example.com/acme/shop
version 1.0.0

require tax git "https://example.com/acme/tax" 1.0.0

signature "example.com/acme/" key "cosign.pub"
```

`ritsu.lock`:

```
# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.
lock 1
ritsu 0.36.0
git example.com/acme/tax 1.0.0 sha256:ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524 files sha256:371cad84ba72971bb3282231b913a7301282eb675fffbf647f18ccda43b6bcf1 commit 9fceb02d0ae598e95dc970b74767f19372d61af8 "https://example.com/acme/tax"
```

`cosign.pub`:

```
-----BEGIN PUBLIC KEY-----
-----END PUBLIC KEY-----
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

`.ritsu/modules/sha256/ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524/files/ritsu.mod`:

```
module example.com/acme/tax
version 1.0.0
```

`.ritsu/modules/sha256/ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524/files/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 20%                     |
| reduced  | 5%                      |
```

See also: [E308](#e308)

<a id="w301"></a>

## W301 — A dependency no file uses

**When**: No file of a module names a file of a dependency its `ritsu.mod` requires. To name is a language's reference (`apply`, `use rule … from` and the others), the `to` and the `after` of a `contribution`, and the `import` of a `.proto`: an `import` writes no `@`, and one that comes to a `.proto` of the dependency counts as a use of it. It is said only when the whole module is checked, since a check of some of its files does not read what the others name. A W301 of a dependency's module is counted with that dependency's warnings.

**Fix**: If the module does not use it, delete the `require` (`ritsu mod remove <name>`); if it is to, name a file of `@<name>/…` where it does.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module shop

require tax path "tax"
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

`tax/ritsu.mod`:

```
module tax
```

`tax/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 20%                     |
| reduced  | 5%                      |
```

See also: [W303](#w303)

<a id="w302"></a>

## W302 — `ritsu.lock` has a line for a module nothing requires

**When**: `ritsu.lock` has a line for a module that the `require` of the tree, followed from the module at hand, do not come to. It comes when a `require` was deleted from a `ritsu.mod` by hand and the lock was not written again. The tree is still read.

**Fix**: Run `ritsu mod lock` to write the lock again.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module shop
```

`ritsu.lock`:

```
# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.
lock 1
ritsu 0.36.0
path tax "tax"
```

`fee.rule`:

```rule
rule fee v1
description "The fee of an order: 5 GBP below 100 GBP, none from 100 GBP"

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| subtotal    | -> fee : money[GBPc] |
| <10000GBPc  | 500GBPc              |
| >=10000GBPc | 0GBPc                |
```

See also: [E304](#e304)

<a id="w303"></a>

## W303 — A path to a file of a module the module does not require, through a dependency

**When**: A file names a file of a dependency of a dependency that its own module does not require, as `@pricing/@tax/…` does; the path is taken as the file's module writes it from its own root. An `import` of a `.proto` writes no `@`, and one that comes to a file of a dependency of a dependency is said on its line all the same. A version of the dependency that no longer uses that module breaks the file. One said of a dependency's file is counted with that dependency's warnings.

**Fix**: Require that module in the module's own `ritsu.mod` (`ritsu mod add`); then its file is named as `@<name>/…`.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`ritsu.mod`:

```
module shop

require pricing path "pricing"
```

`receipt.rule`:

```rule
rule receipt v1
description "The total of an order with gift wrapping: the order's total, and 3 GBP for the wrapping with its VAT"

enum category = standard | reduced

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc
  category : category

outputs
  total : money[GBPc]  round half_up(1GBPc)

apply order = "@pricing/total.rule" meaning:caa5be4f9a41c851
  subtotal = subtotal
  category = category
  total -> order_total

apply vat = "@pricing/@tax/vat_rate.rule" meaning:f2850d644389817c
  category = category
  rate -> rate

result total = order_total + 300GBPc + 300GBPc * rate
```

`pricing/ritsu.mod`:

```
module pricing

require tax path "tax"
```

`pricing/total.rule`:

```rule
rule total v1
description "The total of an order: its subtotal and its VAT"

enum category = standard | reduced

inputs
  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc
  category : category

outputs
  total : money[GBPc]  round half_up(1GBPc)

apply vat = "@tax/vat_rate.rule" meaning:f2850d644389817c
  category = category
  rate -> rate

result total = subtotal + subtotal * rate
```

`pricing/tax/ritsu.mod`:

```
module tax
```

`pricing/tax/vat_rate.rule`:

```rule
rule vat_rate v1
description "The rate of VAT for a category of goods"

enum category = standard | reduced

inputs
  category : category

outputs
  rate : rate[step 1%]  round down(1%)

table rate_by_category
policy unique
| category | -> rate : rate[step 1%] |
| standard | 20%                     |
| reduced  | 5%                      |
```

See also: [W301](#w301)

<a id="w901"></a>

## W901 — A key is written in a contract

**When**: Somewhere in a `.proto` of the project, or in an OpenAPI, AsyncAPI or JSON Schema document a language reads (a `.json`, `.yaml` or `.yml` that dandori's `use openapi` and `use smithy`, rulec's `import jsonschema` and `shape … jsonschema`, or the `openapi` and `asyncapi` of a published language of sakai's map refer to), in a value or a comment alike, there is a value in the shape of a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, or a PEM private key, each a shape its provider fixes. More than one language can read the same contract, so ritsu looks at each once, rather than each language that reads it; a key in a file of a language is that language's W901. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line.

**Fix**: Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line (`//` in a `.proto`, `#` in YAML). A JSON document has no comments: put an example value in its place (an AWS access key ID that ends in `EXAMPLE`, or one character over and over after the prefix).

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`maps.proto`:

```proto
syntax = "proto3";

package maps.v1;

// the Google Maps API key the service is called with: AIzaSyD-ritsu-fake-key-for-tests-000000
message Place {
  string id = 1;
}
```

<a id="e905"></a>

## E905 — A secret value goes outside the map, or to a context unrelated to the one that marked it

**When**: A workflow gives a value a contract marks secret (`debug_redact` in a `.proto`; `x-data-classification`, `x-sensitive-data` or `format: password` in an OpenAPI schema; `secret` in the `.flow`) to a file of the project (an OpenAPI document, a `.proto`, a rule called at its Connect service, a child `.flow`, a book, a dates file), and that file belongs to no context of the map, or to a context the map does not relate to the one that marked the value; `separate ways` relates nothing. The context that marked a value is the one the file of the mark belongs to; a mark the `.flow` writes, and one in a file no context holds, are the flow's own context's. A flow that belongs to no context of any map is not looked at. Sending a secret outside the project (a model's provider, a decision model's API, a URL, an AWS service) is dandori's E906.

**Fix**: Send a reference (an ID, the name of a secret) instead, and have the other side fetch the value. If the other side may hold it, add the relationship to the map, or give the file to a context (`owns`). If sending it there is intended, write `discloses <parameter> "<why>"` under the task.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`shop.ctx`:

```ctx
map Shop(shop) v1
description "Payments charges the card, ordering takes orders, and notices write to the customer"

use context "contexts/payments.ctx"
use context "contexts/ordering.ctx"
use context "contexts/notices.ctx"

covers "payments", "ordering", "notices"
```

`contexts/payments.ctx`:

```ctx
context Payments(payments) v1
description "Charges the customer's card"
owner "Payments team"

owns
  dir "../payments"

published language payments.v1
  proto "../payments/v1/card.proto"

partnership with Ordering
```

`contexts/ordering.ctx`:

```ctx
context Ordering(ordering) v1
description "Takes orders, and has the customer told of them"
owner "Ordering team"

owns
  dir "../ordering"

partnership with Payments
partnership with Notices
```

`contexts/notices.ctx`:

```ctx
context Notices(notices) v1
description "Writes to the customer"
owner "Customer care team"

owns
  dir "../notices"

published language notices.v1
  openapi "../notices/api/notices.json"

partnership with Ordering
```

`payments/v1/card.proto`:

```proto
syntax = "proto3";

package payments.v1;

message Card {
  string id = 1;
  string number = 2 [debug_redact = true];
}
```

`notices/api/notices.json`:

```
{
  "openapi": "3.1.0",
  "info": {
    "title": "Notices",
    "version": "1.0.0"
  },
  "servers": [
    {
      "url": "https://notices.example.com"
    }
  ],
  "security": [
    {
      "bearer": []
    }
  ],
  "paths": {
    "/notices": {
      "post": {
        "operationId": "tell",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "order",
                  "card"
                ],
                "properties": {
                  "order": {
                    "type": "string"
                  },
                  "card": {
                    "type": "object",
                    "properties": {
                      "id": {
                        "type": "string"
                      },
                      "number": {
                        "type": "string"
                      }
                    }
                  }
                }
              }
            }
          }
        },
        "responses": {
          "204": {
            "description": "told"
          }
        }
      }
    }
  },
  "components": {
    "securitySchemes": {
      "bearer": {
        "type": "http",
        "scheme": "bearer"
      }
    }
  }
}
```

`ordering/checkout.flow`:

```flow
workflow checkout v1
  history encrypted
description "Takes an order paid by card, and has the customer told"

use proto pay from "../payments/v1/card.proto"
use openapi notices from "../notices/api/notices.json"

inputs
  order : string
  card  : pay.Card

task tell(order: string, card: pay.Card)
  http POST notices "/notices"
  key

flow
  tell(order: order, card: card)
```

See also: [W905](#w905)

<a id="w905"></a>

## W905 — Where in the map a secret value goes cannot be decided

**When**: A workflow gives a secret value to a file of the project, and the map does not pass sakai's check, so sakai cannot say which context the flow or the file belongs to.

**Fix**: Correct the map so that `sakai check` passes.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`shop.ctx`:

```ctx
map Shop(shop) v1
description "Payments charges the card, ordering takes orders, and notices write to the customer"

use context "contexts/payments.ctx"
use context "contexts/ordering.ctx"
use context "contexts/notices.ctx"
use context "contexts/billing.ctx"

covers "payments", "ordering", "notices"
```

`contexts/payments.ctx`:

```ctx
context Payments(payments) v1
description "Charges the customer's card"
owner "Payments team"

owns
  dir "../payments"

published language payments.v1
  proto "../payments/v1/card.proto"

partnership with Ordering
```

`contexts/ordering.ctx`:

```ctx
context Ordering(ordering) v1
description "Takes orders, and has the customer told of them"
owner "Ordering team"

owns
  dir "../ordering"

partnership with Payments
partnership with Notices
```

`contexts/notices.ctx`:

```ctx
context Notices(notices) v1
description "Writes to the customer"
owner "Customer care team"

owns
  dir "../notices"

published language notices.v1
  openapi "../notices/api/notices.json"

partnership with Ordering
```

`payments/v1/card.proto`:

```proto
syntax = "proto3";

package payments.v1;

message Card {
  string id = 1;
  string number = 2 [debug_redact = true];
}
```

`notices/api/notices.json`:

```
{
  "openapi": "3.1.0",
  "info": {
    "title": "Notices",
    "version": "1.0.0"
  },
  "servers": [
    {
      "url": "https://notices.example.com"
    }
  ],
  "security": [
    {
      "bearer": []
    }
  ],
  "paths": {
    "/notices": {
      "post": {
        "operationId": "tell",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "order",
                  "card"
                ],
                "properties": {
                  "order": {
                    "type": "string"
                  },
                  "card": {
                    "type": "object",
                    "properties": {
                      "id": {
                        "type": "string"
                      },
                      "number": {
                        "type": "string"
                      }
                    }
                  }
                }
              }
            }
          }
        },
        "responses": {
          "204": {
            "description": "told"
          }
        }
      }
    }
  },
  "components": {
    "securitySchemes": {
      "bearer": {
        "type": "http",
        "scheme": "bearer"
      }
    }
  }
}
```

`ordering/checkout.flow`:

```flow
workflow checkout v1
  history encrypted
description "Takes an order paid by card, and has the customer told"

use proto pay from "../payments/v1/card.proto"
use openapi notices from "../notices/api/notices.json"

inputs
  order : string
  card  : pay.Card

task tell(order: string, card: pay.Card)
  http POST notices "/notices"
  key

flow
  tell(order: order, card: card)
```

See also: [E905](#e905)

<a id="e907"></a>

## E907 — No action guards an operation a context opens

**When**: An operation a context of the map opens with `open host service` (a method of a service of a `.proto`, an operation of an OpenAPI document) is guarded by no action of a `.gate` (`guards`), nor by the `@guards` of a schema of Cedar written as `cedar "…"`, and its document does not say that anyone may call it (`security: []`); when an action guards another operation of the same context. When none of them is guarded, it is W907. A channel of an AsyncAPI document and a rule's Connect service are not asked for: `guards` cannot name them.

**Fix**: Write an action that guards the operation in a `.gate` of the context. If anyone may call an OpenAPI operation on purpose, write `security: []` on it in its document.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`shop.ctx`:

```ctx
map Shop(shop) v1
description "A shop that takes orders"

use context "contexts/orders.ctx"

covers "orders"
```

`contexts/orders.ctx`:

```ctx
context Orders(orders) v1
description "Takes the customers' orders, and refunds them"
owner "Orders team"

owns
  dir "../orders"

published language orders.v1
  openapi "../orders/api/orders.json"
  open host service getOrder, refundOrder
```

`orders/api/orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "Orders", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "The order"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "Refunded"}, "403": {"description": "Not allowed"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`orders/refunds.gate`:

```
gate refunds v1
description "Who may refund an order"

use openapi orders from "api/orders.json"

role clerk
  description "Answers the customers, and refunds"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User
  resource Order from orderId

permit clerks_refund
  description "A clerk refunds an order"
  principal in clerk
  action refund_order
```

See also: [W907](#w907)

<a id="w907"></a>

## W907 — A context guards none of the operations it opens

**When**: No action guards any operation a context of the map opens (as E907 counts them); once a context. The context has written no authorization with sekisho yet. A project with no `.gate`, and no Cedar its maps or requirements name, is not asked.

**Fix**: Write a `.gate` of the context, and guard each operation it opens with an action.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`shop.ctx`:

```ctx
map Shop(shop) v1
description "A shop that takes orders and charges cards"

use context "contexts/orders.ctx"
use context "contexts/payments.ctx"

covers "orders", "payments"
```

`contexts/orders.ctx`:

```ctx
context Orders(orders) v1
description "Takes the customers' orders, and refunds them"
owner "Orders team"

owns
  dir "../orders"

published language orders.v1
  openapi "../orders/api/orders.json"
  open host service getOrder, refundOrder
```

`contexts/payments.ctx`:

```ctx
context Payments(payments) v1
description "Charges the customers' cards"
owner "Payments team"

owns
  dir "../payments"

published language payments.v1
  openapi "../payments/api/payments.json"
  open host service createCharge
```

`orders/api/orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "Orders", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "The order"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "Refunded"}, "403": {"description": "Not allowed"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`payments/api/payments.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "Payments", "version": "1.0.0"},
  "servers": [{"url": "https://payments.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/charges": {
      "post": {
        "operationId": "createCharge",
        "responses": {"201": {"description": "The charge"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`payments/charges.gate`:

```
gate charges v1
description "Who may charge a card"

use openapi payments from "api/payments.json"

role cashier
  description "Takes the payment for an order"

principal User
  description "A member of the staff"
  roles cashier

resource Charge
  description "A charge of a card"

action create_charge
  description "Charge a card"
  guards payments createCharge
  principal User
  resource Charge

permit cashiers_charge
  description "A cashier charges a card"
  principal in cashier
  action create_charge
```

See also: [E907](#e907)

<a id="e908"></a>

## E908 — A gate allows a workflow an operation it calls in no combination

**When**: A workflow a `.gate` writes with `workflow … from` calls an operation an action of the gate guards (`http` on a `use openapi`, `connect` on a `use proto`), and the action allows the workflow in no combination: every run that comes to the call is denied there.

**Fix**: Write a permit in the gate that allows the workflow (`principal is workflow <name>`), or take the call out.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "Orders", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "The order"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "Refunded"}, "403": {"description": "Not allowed"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`returns.flow`:

```flow
workflow returns v1
description "Refunds an order once the item is back"

use openapi orders from "orders.json"

inputs
  order : string

task refund_order(orderId: string)
  http POST orders "/orders/{orderId}/refunds"
  key

flow
  refund_order(orderId: order)
```

`refunds.gate`:

```
gate refunds v1
description "Who may look at an order and refund it"

use openapi orders from "orders.json"

enum order_status = paid | returned

role clerk
  description "Answers the customers, and refunds"

principal User
  description "A member of the staff"
  roles clerk

workflow returns from "returns.flow"
  description "Refunds an order once the item is back"

resource Order
  description "An order"
  attributes
    status : order_status

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId

permit clerks_refund
  description "A clerk refunds an order"
  principal in clerk
  action refund_order
```

See also: [W909](#w909), [W908](#w908)

<a id="w908"></a>

## W908 — A workflow is allowed an action whose operations it never calls

**When**: A workflow a `.gate` writes with `workflow … from` is allowed an action of the gate (in some combinations, or in all), and its flow calls none of the operations the action guards: the workflow is allowed more than it needs.

**Fix**: Take the action out of the permits that allow the workflow.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "Orders", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "The order"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "Refunded"}, "403": {"description": "Not allowed"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`returns.flow`:

```flow
workflow returns v1
description "Refunds an order once the item is back"

use openapi orders from "orders.json"

inputs
  order : string

task refund_order(orderId: string)
  http POST orders "/orders/{orderId}/refunds"
  errors denied = 403
  key

flow
  refund_order(orderId: order)
    on denied => fail Denied "the gate did not let the workflow refund"
```

`refunds.gate`:

```
gate refunds v1
description "Who may look at an order and refund it"

use openapi orders from "orders.json"

enum order_status = paid | returned

role clerk
  description "Answers the customers, and refunds"

principal User
  description "A member of the staff"
  roles clerk

workflow returns from "returns.flow"
  description "Refunds an order once the item is back"

resource Order
  description "An order"
  attributes
    status : order_status

action view_order
  description "Look at an order"
  guards orders getOrder
  principal User, Workflow
  resource Order from orderId

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId

permit clerks_refund
  description "A clerk refunds an order"
  principal in clerk
  action refund_order

permit clerks_look
  description "A clerk looks at an order"
  principal in clerk
  action view_order

permit returns_look_and_refund
  description "The returns workflow looks at any order, and refunds it"
  principal is workflow returns
  action view_order, refund_order
```

See also: [E908](#e908)

<a id="w909"></a>

## W909 — A call that can be denied declares no error for it

**When**: The action that guards an operation a workflow calls denies the workflow in some combinations (or whether it does cannot be decided), and the task that calls it declares no error for a denial (403 for HTTP, `permission_denied` for Connect): a run that is denied stops with a failure the workflow does not declare.

**Fix**: Declare the error on the task, `errors denied = 403` (`errors denied = permission_denied` for Connect), and say under the call what happens when it is denied (`on denied => …`).

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "Orders", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "The order"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "Refunded"}, "403": {"description": "Not allowed"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`returns.flow`:

```flow
workflow returns v1
description "Refunds an order once the item is back"

use openapi orders from "orders.json"

inputs
  order : string

task refund_order(orderId: string)
  http POST orders "/orders/{orderId}/refunds"
  key

flow
  refund_order(orderId: order)
```

`refunds.gate`:

```
gate refunds v1
description "Who may look at an order and refund it"

use openapi orders from "orders.json"

enum order_status = paid | returned

role clerk
  description "Answers the customers, and refunds"

principal User
  description "A member of the staff"
  roles clerk

workflow returns from "returns.flow"
  description "Refunds an order once the item is back"

resource Order
  description "An order"
  attributes
    status : order_status

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId

permit clerks_refund
  description "A clerk refunds an order"
  principal in clerk
  action refund_order

permit returns_refunds_returned_orders
  description "The returns workflow refunds an order that has come back"
  principal is workflow returns
  action refund_order
  when resource.status is returned
```

See also: [E908](#e908)
