# Dates and books

A workflow often counts days and moves quantities: it holds an order's goods until the payment is
due, ships them once paid, and puts them back when not. A `.flow` does neither itself, since it has
no arithmetic. The days are counted by a dates file of
[koyomi](https://github.com/i2y/ritsu/tree/main/crates/koyomi) (`.cal`), and the quantities kept by
a book of [chobo](https://github.com/i2y/ritsu/tree/main/crates/chobo) (`.book`). A `.flow` reads
them as it reads rulec's rules: it calls a date as it calls a rule, and runs an operation of a book
as a task.

The invoice example does all of it, and runs as it is on every platform
([examples/invoice](https://github.com/i2y/dandori/blob/main/examples/invoice/invoice.flow)):

```flow
use dates terms from "dates/payment_terms.cal"
  lambda "arn:aws:lambda:eu-west-2:123456789012:function:payment-terms"
use book stock from "books/stock.book"
  lambda "arn:aws:lambda:eu-west-2:123456789012:function:stock"
…
task reserve(order: string, sku: string, qty: int) -> stock.reserve
  book stock.reserve.hold
  starts stock.reserve
  errors out_of_stock
…
case hold : stock.reserve follows stock.reserve

flow
  hold <- reserve(order: order.id, sku: order.sku, qty: order.quantity)
    on out_of_stock => succeed outcome = out_of_stock, due = none
  let due = terms.payment(received: now)
  wait until due.at
  let payment = check_payment(order: order.id, due: due.day)
  match payment.paid
    true =>
      hold <- ship(order: order.id, sku: order.sku)
        on expired => succeed outcome = not_paid, due = due.day
      succeed outcome = shipped, due = due.day
  …
```

koyomi checks the dates file on every day of its range, and chobo checks the book's bounds and
the reasons each operation can be refused with. dandori reads what they found through ritsu's
ports, as it reads the rules: `ritsu dandori` reads the dates files and the books in the same
process, and the `dandori` binary alone says to run a workflow that uses them that way (E018).

## Dates

The dates file of the example closes on the 20th and pays on the 10th of the next month at 09:00,
or on the business day before:

```text
dates payment_terms v1
description "Closes on the 20th; pays on the 10th of the next month, at 09:00, or on the business day before when that day is closed"
use calendar "../calendars/weekdays.cal"

inputs
  received : date  range >=2026-01-01 <=2027-12-31

date closing = received
  close day 20          # "closes on the 20th"

date payment = closing
  day 10 of month +1    # "pays on the 10th of the next month"
  roll preceding        # "on the business day before when that day is closed"
  at 09:00
…
```

`use dates <name> from "<file.cal>"` reads it, and each date of the file is called by its name,
like a rule: `terms.payment(received: …)`. It answers a record of two fields: `day`, the date, and,
when the date says a time of day (`at 09:00`), `at`, that time in UTC, ready for `wait until`. A
date that says no time answers `day` alone.

- A date input takes a `date`. When the file's calendar says its UTC offset (`offset +09:00`), it
  takes a `timestamp` too, read as the day that time falls on there: `2026-03-31T15:30:00Z` is
  April 1 at `+09:00`. A time given to a date whose calendar says no offset is E003, and the note
  says to give the calendar one, or to pass a day.
- An integer input takes an `int`, held to the range the file declares (E014, W104), as a rule's
  input is.
- Each date of a file is called on its own, with the inputs it reads.
- Under `use dates`, `lambda "<function>"` is the Lambda function that computes the dates on Step
  Functions and Lambda durable functions, and `local` has Temporal call them as local activities,
  as for a rule.

A date is called as a rule is, in an activity (a call on each platform), although koyomi's code is
a pure function. A calendar's holidays change every year: computed in the workflow's code, a date
would replay otherwise on a worker whose table was updated. As with a rule, the inputs and the
answer stay in the run's history.

`date` is a type: a day, written `YYYY-MM-DD`. An input, an output, a field, a task's parameter or
answer can be one, and a string can have one put in it (`"{order.id} ships on {ship.day}"`). It is
checked where it comes in, like every other type, and never compared or added to: counting days is
what a dates file is for.

`now` is the time the statement runs at, in UTC, to the second: a `timestamp`. It can go to a date,
to a task, to `wait until`, and into a string (`"received at {now}"`). Each platform reads it so that
a replay or a retry reads the same time again (the table below).

## Books

The book of the example keeps the stock of each SKU. A delivery adds to it, and an order holds what
it takes until it ships, for 60 days at most:

```text
book stock v1
…
unit pcs

account shelf(sku: string) : pcs
  …
  at least 0 refused as out_of_stock
account suppliers : pcs outside
account customers : pcs outside

transfer receive(delivery: string, sku: string, qty: pcs)
  key delivery, sku
  move qty from suppliers to shelf(sku)

transfer reserve(order: string, sku: string, qty: pcs)
  …
  key order, sku
  pending expires after 60 days
  move qty from shelf(sku) to customers
```

`use book <name> from "<file.book>"` reads it. A task runs an operation of one of its transfers with
`book <book>.<transfer>.<operation>`, where the operation is chobo's own: `do` for a transfer done at
once, and `hold`, `post` or `void` for one that holds first.

- The task's parameters are the transfer's, by name: all of them for `do` and `hold`, those of the
  key for `post` and `void`. A `post` that takes the amounts too posts that much of the hold, and
  gives the rest back. An amount is an `int`, a whole number in the book's unit.
- `hold`, `post` and `void` answer the hold: a record named after the transfer (`stock.reserve`),
  with the key's parameters and the hold's `state`. `do` answers nothing.
- The reasons the book refuses with are the task's errors, as they are: `errors out_of_stock`,
  `errors expired`. A reason the book cannot refuse that operation with is E016. A refusal the task
  does not declare fails the call (`failure`).
- A book's operation happens once for its key, so it takes no `key`: done again, it answers that it
  was done before, which reads as done, and a retry is safe. Nor does it take `refused as` (below).
- Under `use book`, `lambda "<function>"` is the Lambda function that runs the book's operations
  on Step Functions.

[`tests/flows/dates_and_books.flow`](https://github.com/i2y/dandori/blob/main/tests/flows/dates_and_books.flow)
posts part of a hold:

```flow
task 一部を出す(注文: string, sku: string, 数: int) -> 倉庫.引当
  book 倉庫.引当.post
  sends post
  errors expired
```

### A hold as a case

A hold has a life: it is held, then posted, voided, or expired. The book says it as a state
machine, so `case <name> : <book>.<transfer> follows <book>.<transfer>` makes a hold a case, and the
checker follows it as it follows a rule's machine. The task that holds says `starts <book>.<transfer>`,
the one that posts `sends post`, and the one that voids `sends void` (E008 otherwise).

When the transfer's holds expire, `expire` happens on the other side by itself: the checker counts
it before every post and void, with no `external` to write. A refusal is one of the book's reasons,
which differ from state to state, so the case takes no `refused when`. The checker says when a
reason can come back unhandled:

```text
error[E022]: tests/fixtures/book_refusals.flow:41:1: the book may refuse `post` here with `expired` (when `押さえ` is in expired); declare `expired` in the `errors` of `出荷する`, and handle it with `on expired =>`
    41 |       押さえ <- 出荷する(注文: 受注.id, sku: 受注.sku)
  the run that gets there:
      38  引き当てる: 押さえ starts in held
      40  match 受注.急ぎ: true
          `expire` happens on the other side: 押さえ held → expired
```

A workflow that can end with the goods still held is E020, as with any case; the invoice puts them
back in `on failure`.

## On each platform

| | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| a date | a Lambda Task; dandori writes the function around the Python koyomi writes | an activity around koyomi's TypeScript, Python or Go (`local`: a local activity) | an invoke of the same Lambda function | the caller image, around koyomi's TypeScript | a function around koyomi's Python |
| an operation of a book | the book's Lambda Task; dandori writes the function around chobo's Python client, and makes the hold from the arguments | an activity dandori writes, through the `Transport`'s `book`, on the chobo client you give it | the same, in a step | the same, in the caller image | the same, through `Deps.tasks` |
| `now` | the time the state was entered (`$states.context.State.EnteredTime`, to the second) | the workflow's clock, which a replay reads again the same | a step that reads the clock, its answer checkpointed (one operation, counted by E040) | `now()` in the expression of the template that computes the value, evaluated once | `Deps.clock` |

What you put beside the generated code:

- **The dates.** `koyomi gen <file.cal> --out koyomi` writes the code of a dates file, which the
  generated wrappers import from `koyomi/` beside them. On Temporal in Go, each dates file is a Go
  module of its own (`koyomi/go/<package>`), which your `go.mod` requires and replaces, as
  `rules.go` says at its top.
- **The books.** chobo writes a client for the book (`chobo build <file.book> --target
  postgres-typescript`, `tigerbeetle-python`, `postgres-go`, …). Give the `Transport` the book
  value it makes, by the book's name: `transport({ books: { stock: postgres(pool, { tenant }) } })`
  in TypeScript, `transport(books={"stock": postgres(connection)})` in Python, and
  `TransportOptions{Books: map[string]any{"stock": stock.Postgres(pool, tenant)}}` in Go. On Step
  Functions, deploy `lambda/book_stock_handler.py` with chobo's Python client, made by
  `handler = make_handler(lambda: tigerbeetle(ClientSync(…)))`.

## How it is checked

- Every platform plays every scenario against the reference interpreter, as for every other call:
  a scenario answers a date with a day (and a time), and an operation of a book with done, done
  before, or a refusal. Each platform's clock is set to one time, so that `now` reads the same
  everywhere.
- The code dandori writes around koyomi's (the Lambda function, `rules.ts`, `rules.py`, `rules.go`)
  runs with the code koyomi writes, on every input of the file's range, and must answer what koyomi
  says each date comes to; one input in seven goes as a time of that day instead, read at the
  calendar's offset.
- The operations of every flow's runs go through the `Transport` dandori writes in TypeScript,
  Python and Go, and through its Lambda function for Step Functions, to the clients chobo writes,
  on PostgreSQL and on TigerBeetle, and must answer what chobo's reference interpreter answers:
  on an empty book, on one a delivery has filled, and once more on the same book, where each
  operation was done before.
