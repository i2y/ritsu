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

**When**: Where a workflow calls a rule, the ranges dandori knows for the values it gives hold a combination that breaks one of the rule's preconditions, a relation between two inputs (`constraint`) (X2, DESIGN 7.4). The ranges are dandori's, gathered from every place a value comes from, read as dandori's E014 reads them. A date input whose range is `range from koyomi` and is given the day of a koyomi date is this error too when one of those days is not one of the rule's (X3 (a)). The rule's generated code refuses such a call at its door, so it would fail only when the workflow runs. The notes give the ranges and the combination or the day that breaks it.

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

**When**: Where a workflow calls a rule, whether a precondition holds cannot be decided (X2): a value comes from a place with no range (a task's result without `range`, say), the precondition bounds the total or the length of a list (dandori knows no list's length), or it is the days of a koyomi date and the value can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`). The workflow's code that `ritsu dandori build` writes checks such a precondition when the workflow runs, as soon as the values are made, and fails a run that breaks it with `Dandori.BrokenPrecondition` (dandori's DESIGN 1.17).

**Fix**: Give the place the value comes from a range (the `range` of a task's result or of the workflow's input). Where none can be given, leave it: the workflow's code checks it at run time.

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

**When**: Where a workflow gives the day of a koyomi date (`due.day`) to a rule's date input, a day koyomi counts that date coming to lies outside the range the rule declares for the input (X3 (a), DESIGN 7.5). koyomi computes the date on every input of its range, so the day outside is an exact example; the notes give it, with the input at which koyomi comes to it. An input whose range is `range from koyomi` takes the days as a precondition of the rule, which E201 and W201 hold the call to.

**Fix**: Widen the rule input's range, or make it `range from koyomi`, so that koyomi's days are the range (rulec's §15.174).

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

See also: [W202](#w202), [E205](#e205)

<a id="w202"></a>

## W202 — Whether the days of a koyomi date stay inside a rule input's range cannot be decided

**When**: The value given to a rule's date input can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`) as well as from a koyomi date, or koyomi does not count the days of the date (more input combinations than it checks, or an input where the computation stops) (X3 (a)). The rule's generated code checks the day at its door when the workflow runs.

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

See also: [E202](#e202)

<a id="e203"></a>

## E203 — A rule's output can be an amount chobo does not take

**When**: Where a workflow gives a rule's numeric output to a chobo transfer as its amount, the output can be below 0 or above 2⁶³ − 1 (X4, DESIGN 7.6; chobo takes 0 to 2⁶³ − 1). rulec counts what the output comes to (the numbers the rows write, or its intervals). chobo fails such a call rather than refusing it as a business outcome. The notes give an input of the rule that comes to that amount, from the rule's vectors.

**Fix**: Make the rule's amounts 0 or more (a negative one, such as a refund, is a transfer the other way), or branch before the transfer.

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

See also: [W203](#w203), [E204](#e204)

<a id="w203"></a>

## W203 — Whether chobo takes a rule's output as an amount cannot be decided

**When**: Where a workflow gives a rule's numeric output to a transfer as its amount, the output's range has an open end, or the value can also come from somewhere with no range (a task's answer without one, say) (X4). An amount chobo does not take fails the call when the workflow runs.

**Fix**: Give the places the value comes from a range (the `range` of a task's answer or of the workflow's input).

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

See also: [E203](#e203)

<a id="e204"></a>

## E204 — The task does not handle a refusal the transfer can come to

**When**: Where a `do` or a `hold` is given a rule's output as its amount, chobo's search, with the amounts held to the range the call gives, finds a run in which the operation is refused for a reason the task does not handle as an error it declares (X4). Only the reasons of the book's bounds (an account's `refused as`) are compared, the refusals that turn on the amounts; one that turns on the calls made before, such as a key used again with other arguments, is not. The search goes as deep as chobo's check goes. A `post` and a `void` are refused for the state their hold is in, which dandori follows with the case (dandori's E022).

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

**When**: The task handles a reason of the book's bounds, and chobo's search, with the amounts held to the range the call gives, finds no run that comes to it (X4). The search goes only as deep as chobo's check does, so all it shows is that the reason does not come within that depth. A case that cannot be decided at all (no range for the amounts, a book that does not answer) is this warning too.

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

**When**: Where a workflow calls a koyomi date, the day it gives the date input can be outside the range of that input (X6, DESIGN 7.8). A day of another koyomi date is held to it with every day koyomi counts for that date. A day inside the range is one koyomi's own check has held to the data of its calendar wherever it asks the calendar (koyomi's E203). The notes give the day outside and the input at which koyomi comes to it.

**Fix**: Widen the koyomi input's range (and the calendar's data), or give it a day that stays inside.

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

See also: [W205](#w205), [E202](#e202)

<a id="w205"></a>

## W205 — Whether a day given to a koyomi date stays inside its range cannot be decided

**When**: dandori does not know the day a workflow gives a koyomi date's input (X6): a day that comes from an input of the workflow, a task's answer or `now` says nothing of what day it is, and dandori has no way yet to write the range of a date. koyomi's generated code refuses a day outside its range when the workflow runs.

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

See also: [E205](#e205)

<a id="e206"></a>

## E206 — A hold has always expired when a call on it comes

**When**: Where a workflow follows a chobo hold as a case (`case … follows <book>.<transfer>`) and posts or voids it, the fewest seconds from making the hold to the call are at least the hold's expiry (the transfer's `pending expires after`) (X5, DESIGN 7.7). The book refuses the call with `expired` on every run, and what follows it going through never runs. dandori counts the time from the statements of the flow: a `wait` of a fixed time takes that time, and a `wait until` the time of a koyomi date, whose date input was given `now` read after the hold, takes from the fewest to the most days koyomi counts from the input to the date, at the date's time. The notes give the fewest and the statements that make them up.

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

**When**: The time from making a hold to posting or voiding it can fall either side of the hold's expiry, or nothing bounds it (X5). The most comes from the `timeout` and the retries of the task that makes the hold and of the tasks between; a task with no `timeout`, a call of a rule or a date (the flow gives them no limit), or a `wait until` a time nothing bounds leaves no most. An expired hold is refused with `expired`, which the flow handles (dandori's E022). When the call is shown to come before the hold expires (the most is shorter than the expiry), nothing is said.

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
