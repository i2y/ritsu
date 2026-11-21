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

**When**: Where a workflow or a function of kaname calls a rule, it can give the rule values that break one of its preconditions (a relation between inputs, `constraint`; a combination that cannot happen, `never`; a bound on the total or the length of the list it walks, or on how few of its elements may count; a field no two elements of the list share, `unique`; the days of `range from koyomi`; the range a value it works out from the inputs is accepted in, `define … range`). For a workflow, the ranges dandori knows for the values it gives hold a combination that breaks it. The ranges are dandori's, from what the runs that reach the call last put in the values, read as dandori's E014 reads them. A truth value or an enum's value a `never` line reads counts as meeting its condition when every value it can be on the runs that reach the call does (a value written out, the values the arms of a `match` leave, the values a `range` lets in; for a variable, what those runs last put in it). Where the call gives one value that meets a condition of the line (`express: true` written out, or `express: fast` after `let fast = true`), a note says that value and, from the `never` line, what to give for the line to hold. A date input whose range is `range from koyomi` and is given the day of a koyomi date, or a date the `.flow` holds to a range (`range >=2026-01-01 <=2026-12-31`), is this error too when one of those days is not one of the rule's. For a function of kaname, a run of the function's vectors gives values that break it: kaname's ranges are an estimate from above, so a corner of them alone is no example. The rule's generated code refuses such a call at its door (the range of a value it works out, where it works the value out), so it would fail only when it runs. The notes give the ranges and the combination or the day that breaks it, or the function's input that does.

**Fix**: Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result; in kaname, an `if` before the call). Where a value written out meets a `never` line, give the value the note says. If the precondition is the part that is wrong, correct the rule's `constraint` or `never` line, or the range of its `define`.

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

**When**: Where a workflow or a function of kaname calls a rule, whether a precondition holds cannot be decided. For a workflow: a value comes from a place with no range (a task's result without `range`, say), the precondition bounds the total or the length of a list or how few of its elements may count, or asks that no two of its elements share a `unique` field (dandori knows neither a list's length nor what it holds), it is a `never` line that reads a truth value or an enum, and the value the call gives can meet its condition or not (written out, given in the arm of a `match` on it, or a variable the runs that reach the call put one value in, it is decided), or it is the days of a koyomi date and the value can come from somewhere that says nothing of what day it is (an input or a task's answer with no range, `now`). The workflow's code that `ritsu dandori gen` writes checks such a precondition as soon as the values are made, and so does `ritsu run`; a run that breaks it fails with `Dandori.BrokenPrecondition`. The range a value the rule works out is accepted in is the exception: the workflow's code does not repeat the rule's arithmetic, and the rule's generated code refuses a call whose value falls outside it where it works the value out. For a function of kaname: neither the ranges kaname finds nor a relation it keeps between the inputs shows it, and no run of the function's vectors breaks it. kaname's generated code checks it there, and the function fails with `rule` where the rule refuses the call.

**Fix**: Give the place the value comes from a range (the `range` of a task's result or of the workflow's input; a date takes one too, `range >=2026-01-01 <=2026-12-31`). Where none can be given, leave it: the workflow's code and `ritsu run` check it where the values are made (the range of a value the rule works out, the rule's code checks). In kaname, branch on the precondition with an `if` before the call, or give values whose relation kaname keeps (the running totals of `running_totals`).

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

**When**: Where a workflow or a function of kaname gives a day to a rule's date input, the day can lie outside the range the rule declares for the input. For a workflow, the day of a koyomi date (`due.day`) is given, and a day koyomi counts that date coming to lies outside: koyomi computes the date on every input of its range, so the day outside is an exact example, and the notes give it with the input at which koyomi comes to it. A date the `.flow` holds to a range (`range >=2026-01-01 <=2026-12-31` on an input, a field or a task's answer) is this error too when a day of the range lies outside: dandori checks the range when the workflow runs, and any day of it can come; the notes give the day and the range. For a function of kaname, a run of the function's vectors gives a day outside, of the days of the koyomi dates the value comes from or of the range kaname finds; the notes give the function's input and the day. An input whose range is `range from koyomi` takes the days as a precondition of the rule, which E201 and W201 hold the call to.

**Fix**: Widen the rule input's range, or make it `range from koyomi`, so that koyomi's days are the range. For a day of a date held to a range, narrowing that range does it too.

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

**When**: The value given to a rule's date input can come from somewhere that says nothing of what day it is (an input or a task's answer with no range, `now`) as well as from a koyomi date or a date held to a range, or koyomi does not count the days of the date (more input combinations than it checks, or an input where the computation stops). For a function of kaname, it is also the days kaname's ranges find holding one outside, which no run of the function's vectors gives. The rule's generated code checks the day at its door when it runs.

**Fix**: Give the input days of koyomi dates, or of dates held to a range (a date takes `range >=2026-01-01 <=2026-12-31` where it comes from), only, and it can be decided; where koyomi does not count them, narrow the inputs of its file.

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

**When**: An amount of a transfer can be below 0 or above 2⁶³ − 1 (chobo takes 0 to 2⁶³ − 1), and chobo fails such a call rather than refusing it as a business outcome. For a workflow, it is checked where a rule's numeric output is given to a chobo transfer as its amount, and it is this error where an input from the rule's vectors, among the values the flow can give the rule in that call (the range of a number, and the values of an enum or a bool the arms of a `match` before the transfer leave), comes to such an amount; an end of the range rulec counts, or a number a row writes, is not enough (W203). The notes give that input and the amount. For a function of kaname, every amount is checked, a rule's output or one kaname computes, and it is this error where a run of the function's vectors comes to such an amount; the notes give the function's input and the amount.

**Fix**: Make the amounts 0 or more (a negative one, such as a refund, is a transfer the other way). A workflow can also leave the transfer out where it gives the rule an input that comes to such an amount: branch with `match` before the transfer on the variable of an enum or a bool the call gave the rule, with no other value put in it between, and give the transfer the output only in the arms of the values that do not come to one; for a number, narrow the `range` of where it comes from. In kaname, the amount can also be narrowed with an `if` before the transfer.

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

**When**: Where a workflow gives a rule's numeric output to a transfer as its amount, the output's range has an open end, or the value can also come from somewhere with no range (a task's answer without one, say). It is also this warning where rulec counts the output's range, over what the flow can give the rule in that call, outside what chobo takes, and no input from the rule's vectors there comes to such an amount: rulec bounds an output by moving each input over its own range, and does not narrow the bound by a `constraint` that ties two inputs, so the output may never come to one. An amount chobo does not take fails the call when the workflow runs. For a function of kaname, the amounts kaname's ranges find do not stay inside what chobo takes, and no run of the function's vectors comes to one outside; kaname's generated code checks the amount where it builds the transfer, and the function fails with `range` for one chobo does not take.

**Fix**: Give the places the value comes from a range (the `range` of a task's answer or of the workflow's input). Where rulec's range of the output reaches outside, write the rule so that rulec counts the output as 0 or more (`max(…, 0)`, say), or make a negative amount a transfer the other way. Holding what the flow gives the rule to inputs that do not come to such an amount (a `match` before the transfer, the `range` of where a number comes from) can decide it too. In kaname, narrow the amount with an `if` before the transfer.

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

**When**: Where a workflow or a function of kaname calls a koyomi date, the day it gives the date input can be outside the range of that input. A day of another koyomi date is held to it with every day koyomi counts for that date; a day that comes from a date or a time the `.flow` holds to a range, or from `now` in a workflow that says the period it runs in, with every day of that range, a time and `now` read as days at the offset of the dates file's calendar. A day inside the range is one koyomi's own check has held to the data of its calendar wherever it asks the calendar (koyomi's E203). For a workflow, the notes give the day outside and the input at which koyomi comes to it; for a function of kaname, a run of the function's vectors gives a day outside, and the notes give the function's input and the day.

**Fix**: Widen the koyomi input's range (and the calendar's data), or give it a day that stays inside (in kaname, narrow it with an `if` before the call); for a day from a range or from the period the workflow runs in, narrow that.

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

**When**: dandori does not know the day a workflow gives a koyomi date's input: a day that comes from an input, a field or a task's answer with no range, from `now` in a workflow that says no period it runs in or in `on failure` and `on cancel`, or from a rule's date output says nothing of what day it is; koyomi's generated code refuses a day outside its range when the workflow runs. For a function of kaname, the days kaname's ranges find hold one outside the range, which no run of the function's vectors gives; kaname's generated code checks it there, and the function fails with `date` where koyomi refuses the day.

**Fix**: Give the place the value comes from a range (`received : date  range >=2026-01-01 <=2027-12-31`), or the workflow the period it runs in (`now range >=2026-01-01 <=2027-12-31` under `workflow`), and it can be decided; so it can with the day of another koyomi date. Otherwise leave it: koyomi refuses at run time.

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

**When**: The time from making a hold to posting or voiding it can fall either side of the hold's expiry, or nothing bounds it. The most comes from the `timeout` and the retries of the task that makes the hold and of the tasks between; a call of a rule or a date takes 33 seconds at the most (ten seconds a try on every target, retried twice, one second apart and then two); each try of a callback's task takes a minute more than its wait at the most, the time every target gives it to hand its id on; a task with `timeout until` waits until its time, counted as a `wait until` is. A task with no `timeout`, or a `wait until` or a `timeout until` a time nothing bounds, leaves no most. When koyomi's days make the most, a note gives the day that makes it the longest. An expired hold is refused with `expired`, which the flow handles (dandori's E022). When the call is shown to come before the hold expires (the most is shorter than the expiry), nothing is said.

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

<a id="e207"></a>

## E207 — What can come from a place's source is not what the place takes

**When**: A line of an input or a field writes `fed by <naming>`, and the line does not take something that can come from there. A number is compared by its unit, its step and its range (another unit does not hold); an enum by the public names of its values, a date by its days, a bool and a string by their type. A string an OpenAPI document or a JSON Schema marks `format: date-time` is read as a moment, a day with a time of day: it never comes to a line that takes a date, and a workflow's `timestamp` takes it (as it does another workflow's `timestamp` and a `.proto`'s `google.protobuf.Timestamp`). A `.proto`'s `google.type.Date` is read as a date, known by its name as dandori knows it. A `.proto` value's name is read as rulec's `import proto` reads it (the enum's own name taken off the front and the rest in lower case; the names of one number are one value, and the place holds it by any one of them; rulec's reference has the table, under `import proto`). A `repeated` field is a list of values, which never comes to a line that takes one. The message gives an example value and, from a rule or a dates file, the input that gives it. A contract (proto, OpenAPI, JSON Schema) with no upper end, and a book with none (up to chobo's limit of 2⁶³ − 1), let that value come, so they are examples too; so is a contract's property that can be left out or be null, and a `.proto`'s field that may be left unset (`optional`, in a `oneof`, or a message, unless a rule or proto2 marks it `required`), where the place is not `T?`. An end the place does not write is not compared. `fed by` changes no generated code and does not check that the hand-written code carries the value: it checks the range of the values.

**Fix**: Widen what the place takes, or narrow what the source gives. Which of the two is a person's decision. Where a string a contract marks `format: date-time` carries only a day, the contract marks it `format: date`.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`income.rule`:

```rule
rule income v1
description "The food a province gives, by its farms"

inputs
  farms : number  range >=0 <=3

outputs
  food : number  round down(1)

table food_of
policy unique
| farms | -> food : number |
| 0     | 2                |
| >=1   | 20               |
```

`population.rule`:

```rule
rule population v1
description "Whether a province grows on the food it gets"

inputs
  food : number  range >=0 <=12  fed by rulec "income.rule" output food

outputs
  grows : bool

table grows_of
policy unique
| food | -> grows : bool |
| >=6  | true            |
| <6   | false           |
```

See also: [W207](#w207), [E208](#e208)

<a id="w207"></a>

## W207 — Whether a place takes what can come from its source cannot be decided

**When**: The source of a `fed by` does not say all of its values: rulec does not count an end of a rule's output, or the values of an enum a rule gives; what it counts lies outside what the place takes, and none of the rule's vectors comes to such a value (rulec bounds an output by moving each input over its own range); the source's language does not pass its file (which that language says); a contract's date where the place writes a range (a contract carries a date as a string or as a `.proto`'s `google.type.Date`, and lists no days); two records or lists.

**Fix**: For a rule, write it so that rulec counts the output's ends (`min(…, <the limit>)`, say), or widen what the place takes; for an enum whose values rulec does not count, take the values the place leaves out too. For a contract, list its values (`enum` in a schema, `in` in Protovalidate); a `.proto`'s `google.type.Date` lists no days, and a string field that lists them can be decided. For a workflow or a module of kaname, write the range on the source's line. Two records or lists are not compared value by value, and nothing on the source's line decides them.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`carried_over.rule`:

```rule
rule carried_over v1
description "What is carried over to the next season"

inputs
  total : number  range >=0 <=300
  cost  : number  range >=1 <=200

outputs
  ahead : number  round down(1)

define left : number = total - cost

table finish
policy unique
| left | -> ahead : number |
| >=0  | 0                 |
| <0   | total             |
```

`research.rule`:

```rule
rule research v1
description "Whether the research has come far"

inputs
  progress : number  range >=0 <=250  fed by rulec "carried_over.rule" output ahead

outputs
  far : bool

table far_of
policy unique
| progress | -> far : bool |
| >=100    | true          |
| <100     | false         |
```

See also: [E207](#e207)

<a id="e208"></a>

## E208 — The source of a `fed by` is not there

**When**: The file a `fed by` names is not there, or does not hold what it names (a rule's output, an account, a date, a field of a message, a property of a schema, a workflow's output or a field of its record, an input or an attribute that takes the value first), or a document or a `.proto` does not read. The form of the naming, and that it names something with a value, are said by the check of the language it is written in. That check reads no other file, so that the thing is there is said by `ritsu check`.

**Fix**: A path goes from the directory of the file it is written in. Put the name right, or add what it names.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`income.rule`:

```rule
rule income v1
description "The food a province gives, by its farms"

inputs
  farms : number  range >=0 <=3

outputs
  food : number  round down(1)

table food_of
policy unique
| farms | -> food : number |
| 0     | 2                |
| >=1   | 20               |
```

`population.rule`:

```rule
rule population v1
description "Whether a province grows on the food it gets"

inputs
  food : number  range >=0 <=60  fed by rulec "income.rule" output wheat

outputs
  grows : bool

table grows_of
policy unique
| food | -> grows : bool |
| >=6  | true            |
| <6   | false           |
```

See also: [E207](#e207)

<a id="e209"></a>

## E209 — A ticket's hold can expire before its round posts or voids it

**When**: A hold a request makes with `on submitted hold` can reach the book's expiry (the transfer's `pending expires after`) before the ending of its round (approved, rejected, sent back, withdrawn, expired) posts or voids it. How long the hold stays open is the business days ringi's check counts: one round and the pause (`pause at most`), and where a stage can send the ticket back to an earlier stage (`send back to any earlier stage`), which starts no new round, the round once more each time. The days are laid on the calendar from every day a request can be submitted on, and a note gives the day that makes the stretch the longest: from 00:00 of that day to the end of the last business day. The business days are the upper bound ringi's check holds its claims to (`ends within`), which a ticket need not take. The book refuses a post or a void of an expired hold with `expired`: the ticket would be approved with nothing held for it. A hold that never expires (`pending never expires`), and one shown to end before it expires, are not said.

**Fix**: Make the hold last longer (the transfer's `pending expires after`), or give the stages shorter deadlines.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`purchase.ringi`:

```
request purchase v1 "Purchase request"
effective from 2026-04-01

use calendar office from "office.cal"
use book budget from "budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  dept   : text "Department"  count >=1 <=20

route
  check  finance

who
  finance = team finance

every stage
  due in 10 business days at end of day
  when late expire

ticket
  on submitted  hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  on approved   post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

`office.cal`:

```cal
calendar office v1
description "The business days of the office: Saturdays and Sundays are closed"
offset +00:00

closed weekly sat, sun
```

`budget.book`:

```book
book budget v1
description "The budget of each department: a purchase request holds its amount while it is approved"

unit JPY

account dept_budget(dept: string) : JPY
  at least 0 refused as over_budget
account allocations : JPY outside
account payees : JPY outside

transfer allocate(dept: string, period: string, amount: JPY)
  key dept, period
  move amount from allocations to dept_budget(dept)

transfer reserve(request: string, attempt: string, dept: string, amount: JPY)
  key request, attempt
  pending expires after 7 days
  move amount from dept_budget(dept) to payees
```

See also: [W209](#w209), [E206](#e206)

<a id="w209"></a>

## W209 — Whether a ticket's hold expires cannot be decided, or some requesters cannot make it

**When**: Two cases. One: whether a hold reaches its expiry before it is posted or voided cannot be decided. A round has a wait with no end (a stage with no deadline, a stage that stays when it is late); the business days are counted in several calendars, and the stretch over the days all of them open reaches the expiry (which a ticket need not take); or a calendar's data does not cover the business days of a round. The other: an argument of the hold reads a team above the requester (`dept: requester.department`), and some of the people who may submit the request have no team of that kind above them. The hold cannot be made for them, so their `submit` is refused. The notes give, for each version of the organization, how many of how many, and their IDs; where the organization has no data, it is undecided. A line whose `when` says `requester.department is not none` makes the hold for the requesters who have the team alone, and nothing is said.

**Fix**: For the expiry: give every stage a deadline (`due in …`), or make the hold last longer. For the argument: if those people do not submit the request, leave them out (`submitted by … except …`); if they do, give the hold a value every requester has, or write `when requester.<kind> is not none` on the line, so that their requests hold nothing.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`purchase.ringi`:

```
request purchase v1 "Purchase request"

use book budget from "budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  dept   : text "Department"  count >=1 <=20

route
  check  finance

who
  finance = team finance

ticket
  on submitted  hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  on approved   post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected   void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

`budget.book`:

```book
book budget v1
description "The budget of each department: a purchase request holds its amount while it is approved"

unit JPY

account dept_budget(dept: string) : JPY
  at least 0 refused as over_budget
account allocations : JPY outside
account payees : JPY outside

transfer allocate(dept: string, period: string, amount: JPY)
  key dept, period
  move amount from allocations to dept_budget(dept)

transfer reserve(request: string, attempt: string, dept: string, amount: JPY)
  key request, attempt
  pending expires after 7 days
  move amount from dept_budget(dept) to payees
```

See also: [E209](#e209)

<a id="e210"></a>

## E210 — What a ticket and a workflow hand each other does not fit

**When**: An input of a flow does not take something a request gives it. A `launch` (`on … launch`) gives what its call writes; an agent's way (`agent <flow>, else …`) and a summary (`note from <flow>`) are given the ticket's ID (`request_id`) and the fields of the form the flow declares as its inputs. A number and an amount are compared by unit and range, a date by its days, a choice by the names of its values, the rest by type; a value that can be absent where the input is not `T?` does not hold either. A value is read as the place the flow is given it holds it: for an agent and a summary, the rows of the route's table that give the stage; on a line or a stage with a `when`, the comparisons of fields of the form with constants (joined with `and`). An input a `launch` does not give, a value it gives that the flow has no input for, and an input of an agent's or a summary's flow that is neither `request_id` nor a field of the form, are this error too. An agent's flow answers in the outputs `verdict` (an enum whose values are `ok` and `doubt`) and `reason` (a string), and a summary's flow has at least one output, each of a type a note holds: text (a string, a date, a timestamp, an enum), a whole number (an int, or a number with a unit), a truth (a bool), rows (a list of records whose fields are of these types), or a `T?` of these, and not a record on its own, a list of what is not a record, or `json`; a flow that does not is this error. The other way, where a flow makes a request through the server's API (the `openapi.json` `ritsu gen --target requests` writes), the operation the flow calls is compared with the one ringi writes from the request now: where the operation is, what its body takes (the form's schema), and the request it names (the name and the version of `x-ritsu-request`). Where they differ, the document was written from the request as it was, and what dandori held the flow to is not the request now: this error too, as is a call of an operation the request no longer has. Where the document is the request's now, the types and the ranges of what the flow sends are dandori's to compare with the document (dandori's E016).

**Fix**: Hold the value to what the flow takes (the form's `range`, a `when` on the `on` line), or widen the flow's input. Which of the two is a person's decision. Where the API a flow calls is not the request's now, write it again with `ritsu gen --target requests`, and check again.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`purchase.ringi`:

```
request purchase v1 "Purchase request"

use flow order from "order.flow"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
  check  finance

who
  finance = team finance

ticket
  on approved  launch order(request_id: ticket.id, amount: amount)
```

`order.flow`:

```flow
workflow purchase_order v1
description "The order of an approved purchase request, placed with the supplier"

inputs
  request_id : string
  amount     : money[JPY]  range >=1JPY <=500_000JPY

outputs
  reference : string

record Order
  reference : string

task place_order(request_id: string, amount: money[JPY]) -> Order
  timeout 1 day
  idempotent

flow
  let o = place_order(request_id: request_id, amount: amount)
  succeed reference = o.reference
```

See also: [W210](#w210), [E207](#e207)

<a id="w210"></a>

## W210 — Whether what a ticket and a workflow hand each other fits cannot be decided

**When**: An input of the flow is not shown to take what it is given: the request gives a value ringi does not follow; the two types are not compared value by value (a `rows` field and a list of records); the organization has no data, so that whether `requester.<kind>` has a value is not decided; or the values look as if they do not fit, at a place whose condition ringi does not follow. Such a condition is a `when` with `or` or `not`, a `when` that reads a field the value is counted with, the rows where a rule's output names the way, a stage another line sends there, a shared route. Two types or units that differ are E210 whatever the condition. Where a flow makes a request through the server's API, it cannot be decided either when the request's file an operation of the API names is not the project's (outside the root, not there, or no request's file), or ringi cannot write the API of the request (a request that reads no organization): the flow is held to the document alone.

**Fix**: Write the `when` as comparisons of fields of the form with constants (`amount <= 500_000JPY`, joined with `and`), which ringi follows. Where that cannot be done, widen the flow's input to take the form's range. Where the request of the API a flow calls is in this project, write the API again with `ritsu gen --target requests` from the project's root.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`purchase.ringi`:

```
request purchase v1 "Purchase request"

use flow order from "order.flow"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
  check  finance

who
  finance = team finance

ticket
  on approved  launch order(request_id: ticket.id, amount: amount)  when not (amount > 500_000JPY)
```

`order.flow`:

```flow
workflow purchase_order v1
description "The order of an approved purchase request, placed with the supplier"

inputs
  request_id : string
  amount     : money[JPY]  range >=1JPY <=500_000JPY

outputs
  reference : string

record Order
  reference : string

task place_order(request_id: string, amount: money[JPY]) -> Order
  timeout 1 day
  idempotent

flow
  let o = place_order(request_id: request_id, amount: amount)
  succeed reference = o.reference
```

`requests.json`:

```
{
  "openapi": "3.1.0",
  "info": { "title": "Requests", "version": "1" },
  "paths": {
    "/requests/supplies/tickets": {
      "post": {
        "operationId": "submit_supplies",
        "requestBody": {
          "required": true,
          "content": { "application/json": { "schema": {
            "type": "object",
            "required": ["form"],
            "properties": {
              "form": {
                "type": "object",
                "required": ["item", "count"],
                "properties": {
                  "item": { "type": "string", "maxLength": 100 },
                  "count": { "type": "integer", "minimum": 1, "maximum": 100 }
                },
                "additionalProperties": false
              }
            },
            "additionalProperties": false
          } } }
        },
        "responses": {
          "200": { "description": "The ticket", "content": { "application/json": { "schema": {
            "type": "object",
            "required": ["ticket"],
            "properties": { "ticket": { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" } } } }
          } } } }
        },
        "x-ritsu-request": { "file": "supplies.ringi", "name": "supplies", "version": 1 }
      }
    }
  }
}
```

`restock.flow`:

```flow
workflow restock v1
description "Asks for the supplies that run low, through the server of the requests"

use openapi requests from "requests.json"
  url "https://requests.example.com"

record Supplies
  item  : string
  count : int  range >=1 <=50

record Ticket
  id : string

record Moved
  ticket : Ticket

inputs
  supplies : Supplies

outputs
  ticket : string

task submit_supplies(form: Supplies) -> Moved
  http POST requests "/requests/supplies/tickets"
  key

flow
  let moved = submit_supplies(form: supplies)
  succeed ticket = moved.ticket.id
```

See also: [E210](#e210)

<a id="e301"></a>

## E301 — A `ritsu.mod` that does not read

**When**: A module's `ritsu.mod` that ritsu cannot read: a line that starts with a word ritsu does not know, a word with no value, an escape in a string other than `\"` and `\\`, a second statement of a word written once at most (`module`, `version`, `description`, `ritsu`, `license`, `source`, `target`), no `module` or a `module` that is not the first statement, a full-width space outside a string, a control character other than a tab, bytes that are not UTF-8, a target of `library` or `target` ritsu does not know, a target written twice on the `target` line. When the `ritsu.mod` of a module of the tree does not read, the tree of dependencies is not known, and ritsu stops before any language checks a file (exit 1).

**Fix**: Correct the line it points at. A statement starts with one of `module`, `version`, `description`, `ritsu`, `license`, `source`, `require`, `exclude`, `signature`, `library` and `target`, and the first is `module`.

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
ritsu 0.55.2
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
ritsu 0.55.2
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
ritsu 0.55.2
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
ritsu 0.55.2
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

<a id="e310"></a>

## E310 — The number on the Connect wire of a value of an enum or of a field changed since the old version

**When**: A value of an enum of a rule, or a field of one of its Connect messages, in both versions `ritsu mod diff` compares, has another number on the rule's Connect service; and so in the two versions of a changed module `ritsu mod report` compares. A rule that does not write `implements` is numbered by rulec in the order things are written: the values of an enum the rule declares from 1; the fields of the request, the inputs from 1 and the sequences after them; the fields of the response, the outputs from 1; and the fields of an element of a sequence from 1. One added in the middle, or two that change places, move the numbers of those after, and since the sequences come after the inputs, an input added anywhere moves theirs. A rule that writes `implements proto "<file>" <Service>` has the contract's numbers, and so does an enum read from a `.proto` (`import proto`): those move only when the contract moves them, and a rule whose file did not change is compared all the same when a `.proto` of its module did. protobuf's binary form sends a value and a field by its number: what the code generated from one version writes, the code of the other reads as another value or another field, or skips. Neither fails, and the answer is another (the JSON form sends the names, which do not move). Values and fields are matched by their names in the rule. When the method's path (`/<package>.<service>/<method>`) differs between the two versions, a caller of the old one does not reach the new one, and nothing is read as another value or another field, so it is not this error: the change of the path is shown as a candidate for a change that may break a user. The path changes with the rule's version (`rule … v2`) for a rule that does not write `implements`, and with the contract's package, service or method for one that does. `ritsu mod diff` shows it among the differences of the interface (exit 1), and `ritsu mod report` counts it as a new error of the module that changed (exit 1).

**Fix**: Have a contract keep the numbers: with `implements proto "<file>" <Service>` at the head of the rule, the numbers are the contract's, whatever order the rule writes things in. The first contract can be the `.proto` `ritsu rulec gen` writes for the rule now, copied out: the path and every number stay as they were. Give a new value or field, in the contract, a number nothing has used, and where the contract moved a number, put the old one back. In a rule that does not write `implements`, add a new value, input, output or field of an element at the end: the numbers of the others stay. Where the order has to change or something has to go, raise the rule's version (`rule … v2`; for a rule that writes `implements`, the contract's package), so that the new version is another service, and keep the old service until nothing calls it.

**Reproduction**: put the files below in one directory, and run `ritsu mod diff old new` there.

`old/ritsu.mod`:

```
module shop
version 1.0.0
```

`old/fee.rule`:

```rule
rule fee v1
description "The fee for a parcel, by the region it goes to"

enum region = domestic | world

inputs
  region : region

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| region   | -> fee : money[GBPc] |
| domestic | 395GBPc              |
| world    | 1850GBPc             |
```

`new/ritsu.mod`:

```
module shop
version 1.1.0
```

`new/fee.rule`:

```rule
rule fee v1
description "The fee for a parcel, by the region it goes to"

enum region = domestic | eu | world

inputs
  region : region

outputs
  fee : money[GBPc]  round up(1GBPc)

table pick
policy unique
| region   | -> fee : money[GBPc] |
| domestic | 395GBPc              |
| eu       | 1250GBPc             |
| world    | 1850GBPc             |
```

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
ritsu 0.55.2
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

**When**: A workflow a `.gate` writes with `workflow … from` calls an operation an action of the gate guards (`http` on a `use openapi`, `connect` on a `use proto`), and the action allows the workflow in no combination the call can come to: every run that comes to the call is denied there. What the call can come to is what it gives the operation (the action's `input`s), the values of the flow the gate's `resource` lines under the workflow say the resource's attributes are, and what the arms of a `match` on the way leave of each (of the answer of a rule the flow called, too).

**Fix**: Make the call where the gate allows the workflow, or change the gate's policies. Where no permit allows the workflow, write one (`principal is workflow <name>`); or take the call out.

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

<a id="e909"></a>

## E909 — The gate reads a number a call gives in another unit or step

**When**: A workflow a `.gate` writes with `workflow … from` calls an operation an action of the gate guards, and gives an argument a number whose type is counted in another unit, step or tax than the action's `input` of its name: `fee: rate[step 0.1%]` in the task and `fee : rate[step 1%]` in the gate, or `money[GBP]` and `money[GBPc]`. The code generated from the flow sends the number in its type, and the code generated from the gate reads the same number in its own, so the two read one call as two values (the flow's 0.5% is 5% to the gate). The types are compared as `fed by` compares a place with its source. The gate's answer at the call does not read such an argument: of it, only E908 is said (allowed in no combination, whatever the argument is), and whether the call can be denied (W909) is asked again once the types agree.

**Fix**: Write one type as the other: the task's parameter, or the action's `input` (and the ends of its range, where they are written without a unit).

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "Orders", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "requestBody": {"required": true, "content": {"application/json": {"schema": {"type": "object", "required": ["fee"], "properties": {"fee": {"type": "integer", "minimum": 0, "maximum": 100}}}}}},
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
description "Refunds an order once the item is back, keeping a restocking fee"

use openapi orders from "orders.json"

inputs
  order : string

task refund_order(orderId: string, fee: rate[step 0.1%] range >=0% <=10%)
  http POST orders "/orders/{orderId}/refunds"
  errors denied = 403
  key

flow
  refund_order(orderId: order, fee: 0.5%)
    on denied => fail Denied "the gate did not let the workflow refund"
```

`refunds.gate`:

```
gate refunds v1
description "Who may refund an order"

use openapi orders from "orders.json"

role clerk
  description "Answers the customers, and refunds"

principal User
  description "A member of the staff"
  roles clerk

workflow returns from "returns.flow"
  description "Refunds an order once the item is back, keeping a restocking fee"

resource Order
  description "An order"

action refund_order
  description "Refund an order, keeping a restocking fee"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId
  input
    fee : rate[step 1%]  range >=0% <=10%

permit clerks_refund
  description "A clerk refunds an order"
  principal in clerk
  action refund_order

permit returns_keeps_a_small_fee
  description "The returns workflow refunds an order, keeping a fee of up to 1%"
  principal is workflow returns
  action refund_order
  when fee <= 1%
```

See also: [E908](#e908), [W909](#w909)

<a id="w909"></a>

## W909 — A call that can be denied declares no error for it

**When**: The action that guards an operation a workflow calls denies the workflow in some of the combinations the call can come to, and the task that calls it declares no error for a denial (403 for HTTP, `permission_denied` for Connect): a run that is denied stops with a failure the workflow does not declare. Also where the answer reads attributes of the resource and the gate says nothing of what the flow holds of them (a `resource` line under the workflow); where every call is allowed only as far as those `resource` lines hold (nothing checks them of the resource the service reads); and where whether it denies cannot be decided.

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

<a id="e911"></a>

## E911 — A gate does not say what a request's route says

**When**: A gate a request reads (`use gate`) does not say what the route of the request says. Three things are asked. One: the gate declares an action for every operation of a ticket and of the server's administration, and none of them takes inputs of its own (`input`). The operations of a ticket are `submit`, `approve`, `reject`, `add_approver`, `hand_over`, `comment`, `see`, which is asked before a ticket is shown to someone not on it, and `withdraw`, `send_back`, `pause` and `retract` where the request has them; those of the administration, `change_request_type` and `read_audit`, which switch a version of a request type and read its record, and `import_organization` and `read_organization_audit`, which import the organization's data and read its record. The server asks the gate before it takes an operation: an action the gate does not declare is allowed to no one, and since the server gives the gate the person, the resource and the time only, an action that takes inputs of its own is denied on every ticket. Two: the approvers the route decides are allowed what an approver does, asked with the principal in `resource.approvers` and not the requester. Allowed in no combination is the error; allowed in some holds, as with a gate that denies a suspended member of the staff. Three, self approval. Where the route does not allow it (`self approval not allowed`, or no such line), nothing that decides a stage (`approve`, `reject`, `send_back`) is allowed in any combination where `resource.requester` is the principal and `resource.self_approval` is false. Where the route allows it, some combination is allowed where the requester is among the approvers and `resource.self_approval` is true. The gate is asked with the attributes ringi hands it held to those values, over every value of the rest (the roles, the other attributes), so the organization's data is not read.

**Fix**: In the gate, declare the missing action, take the `input` lines off an action an operation asks, write a permit for the approvers of a stage (`when principal in resource.approvers`), and a forbid that keeps a requester from deciding their own request (`when resource.requester is principal`, `unless resource.self_approval`). Or change the `self approval` line of the request. Which of the two is right is a person's decision.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`purchase.ringi`:

```
request purchase v1 "Purchase request"

use gate "company.gate"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

`company.gate`:

```
gate company v1
description "Who does what to a ticket: the approvers of a stage answer it"
namespace Ringi

principal Staff
resource Approvers
resource Ticket
resource RequestType
resource Organization
resource Stage
  attributes
    requester     : Staff
    approvers     : Approvers
    self_approval : bool

action submit
  principal Staff
  resource Ticket
action see
  principal Staff
  resource Ticket
action approve
  principal Staff
  resource Stage
action reject
  principal Staff
  resource Stage
action add_approver
  principal Staff
  resource Stage
action hand_over
  principal Staff
  resource Stage
action comment
  principal Staff
  resource Stage
action change_request_type
  principal Staff
  resource RequestType
action read_audit
  principal Staff
  resource RequestType
action import_organization
  principal Staff
  resource Organization
action read_organization_audit
  principal Staff
  resource Organization

permit staff_submit_and_administer
  principal is Staff
  action submit, see, add_approver, hand_over, change_request_type, read_audit, import_organization, read_organization_audit

permit staff_comment
  principal is Staff
  action comment

permit approvers_answer_their_stage
  principal is Staff
  action approve, reject
  when principal in resource.approvers
```

See also: [W911](#w911), [E908](#e908)

<a id="w911"></a>

## W911 — Whether a gate says what a request's route says cannot be decided

**When**: sekisho does not answer the second or the third question of E911: the gate declares an attribute ringi hands it with another type (a `self_approval` that is not a bool, a `requester` or an `approvers` that does not point to an entity), or the gate's answer rests on a combination of a condition another language is asked, which that language does not decide. A note says why.

**Fix**: Declare `requester` (an entity of the principal's type), `approvers` (an entity the approvers are members of) and `self_approval` (a bool) on the gate's resource.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`purchase.ringi`:

```
request purchase v1 "Purchase request"

use gate "company.gate"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

`company.gate`:

```
gate company v1
description "Who does what to a ticket: the approvers of a stage answer it, and no one decides their own request unless its route lets them"
namespace Ringi

enum own_decision = permitted | not_permitted

principal Staff
resource Approvers
resource Ticket
resource RequestType
resource Organization
resource Stage
  attributes
    requester     : Staff
    approvers     : Approvers
    self_approval : own_decision

action submit
  principal Staff
  resource Ticket
action see
  principal Staff
  resource Ticket
action approve
  principal Staff
  resource Stage
action reject
  principal Staff
  resource Stage
action add_approver
  principal Staff
  resource Stage
action hand_over
  principal Staff
  resource Stage
action comment
  principal Staff
  resource Stage
action change_request_type
  principal Staff
  resource RequestType
action read_audit
  principal Staff
  resource RequestType
action import_organization
  principal Staff
  resource Organization
action read_organization_audit
  principal Staff
  resource Organization

permit staff_submit_and_administer
  principal is Staff
  action submit, see, add_approver, hand_over, change_request_type, read_audit, import_organization, read_organization_audit

permit staff_comment
  principal is Staff
  action comment

permit approvers_answer_their_stage
  principal is Staff
  action approve, reject
  when principal in resource.approvers

forbid no_one_decides_their_own
  principal is Staff
  action approve, reject
  when resource.requester is principal
  unless resource.self_approval is permitted
```

See also: [E911](#e911)
