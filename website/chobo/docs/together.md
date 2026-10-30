# With the other languages

A book stands on its own, and chobo reads no other language. In a project of ritsu's, the book is
also where other files meet: a dandori workflow runs its operations as tasks and follows its holds
as cases, a rulec rule works out the amounts the workflow (or code written by hand) hands it, a
sekisho gate says who may run
each operation, a yuen requirement names the operations that satisfy it, and a sakai map says which
context keeps the book. `ritsu check` reads them together, and checks where they meet.

## A workflow runs the operations

dandori's invoice example holds an order's goods until its payment is due, ships them once paid,
and puts them back when not
([crates/dandori/examples/invoice/invoice.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/invoice/invoice.flow)).
Its stock is a book like the inventory example, whose holds last 60 days:

```flow
use book stock from "books/stock.book"
…
task reserve(order: string, sku: string, qty: int) -> stock.reserve
  book stock.reserve.hold
  starts stock.reserve
  errors out_of_stock

task ship(order: string, sku: string) -> stock.reserve
  book stock.reserve.post
  sends post
  errors expired

task put_back(order: string, sku: string) -> stock.reserve
  book stock.reserve.void
  sends void
  errors expired, already_posted
…
case hold : stock.reserve follows stock.reserve

flow
  hold <- reserve(order: order.id, sku: order.sku, qty: order.quantity)
    on out_of_stock => succeed outcome = out_of_stock, due = none
```

- `book <book>.<transfer>.<operation>` makes a task an operation of the book: `do`, or `hold`,
  `post` and `void`. Its parameters are the transfer's, by name, and an amount is the book's unit as
  [Money and units](units.md#as-the-other-languages-see-a-unit) says.
- The reasons the book can refuse the operation with are the task's errors, as they are:
  `errors out_of_stock`. A reason the book cannot give for that operation is an error of the
  workflow's checker, and a refusal the task does not declare fails the call.
- An operation happens once for its key, so the task takes no idempotency key: done again, it
  answers that it was done before, which the workflow reads as done, and a retry is safe.
- `case hold : stock.reserve follows stock.reserve` follows a hold through the state machine of its
  life, which `chobo api` gives too. A hold can expire by itself, so the checker counts that it may
  have expired before every post and void, and a workflow that can end with the goods still held is
  an error.

dandori reads the book through ritsu's port of books, as it reads a rule through the port of rules:
`ritsu dandori` and `ritsu check` hand it what chobo knows of the book, and the code dandori writes
calls the client `chobo build` writes, on every platform it builds for. dandori's
[Dates and books](../dandori/dates-and-books/) says how.

## A rule and the book, without a workflow

A book is not always called from a workflow. A checkout works the amounts out and records the
sale in the one request it serves. `ritsu gen` writes the rule's module and the book's client into
one package, and there an amount of money or of a quantity is of the package's one type of its
unit: the type the rule returns is the type the book's operation takes. In ritsu's tests, the
marketplace splits a sale with a rule, in cents, and its checkout, written by hand, gives the book
what the rule returns
([crates/ritsu/tests/projects/marketplace](https://github.com/i2y/ritsu/tree/main/crates/ritsu/tests/projects/marketplace)):

```ts
import { books, rules, units } from "../generated/typescript/index";
…
export async function checkout(book: books.marketplace.Book, order: string, shop: string, plan: rules.sale_split.Plan, paid: units.USDc): Promise<books.marketplace.Result> {
  const split = rules.sale_split.sale_split(plan, paid);
  return book.sale.do({ order, shop, to_shop: split.to_shop, fee: split.fee });
}
```

The rule's `fee` and `to_shop` are `units.USDc`, and so are the amounts the book's `sale` takes
(`unit USD scale 2`). What lies between the two is code of no language of ritsu's, so `ritsu
check` does not see this border; the type checker of the code's language holds it instead. Were
the rule to count in whole dollars (`money[USD]`), tsc would stop the checkout where the dollars
meet the book's cents:

<div class="cb-term" markdown>

```text
service/checkout.ts(10,51): error TS2345: Argument of type 'USDc' is not assignable to parameter of type 'USD'.
  Type 'USDc' is not assignable to type '{ readonly __rulec: "USD"; }'.
    Types of property '__rulec' are incompatible.
      Type '"USDc"' is not assignable to type '"USD"'.
service/checkout.ts(11,38): error TS2322: Type 'USD' is not assignable to type 'USDc'.
  Type 'USD' is not assignable to type '{ readonly __rulec: "USDc"; }'.
    Types of property '__rulec' are incompatible.
      Type '"USD"' is not assignable to type '"USDc"'.
service/checkout.ts(11,62): error TS2322: Type 'USD' is not assignable to type 'USDc'.
  Type 'USD' is not assignable to type '{ readonly __rulec: "USDc"; }'.
    Types of property '__rulec' are incompatible.
      Type '"USD"' is not assignable to type '"USDc"'.
```

</div>

mypy stops the Python checkout the same way (`Argument "to_shop" to "do" of "_saleTransfer" has
incompatible type "USD"; expected "USDc"`), and go vet the Go one. An amount of a count (`pcs`,
`seats`) is a plain integer, as it is everywhere, and so is a balance the client reads. In a
workflow, `ritsu check` holds the same amounts itself, and what tsc cannot see besides: the range
of the amounts and the refusals they can come to, below.

## Where they meet, checked

Each check that crosses a language holds, finds a case where it does not, or says that it cannot be
decided, and why. Three of them are about a book: the amounts a rule hands a transfer, the reasons
the transfer can be refused with for those amounts, and a hold against the wait before it is posted.
The files of each example below are the project `ritsu explain <code>` prints.

**A refusal the task does not handle.** A rule works out the seats an event needs, up to 400 for a
concert, and the workflow gives them to a hall whose account of seats given is at most 300. chobo's
search, with the amounts held to what the rule can return, finds that the transfer can be refused:

<div class="cb-term" markdown>

```text
error[ritsu E204]: booking.flow:18:1: The book can refuse `assign.do` with `over_capacity`, which the task `give_seats` does not handle
    18 |   give_seats(event: event, count: need.needed)
  = chobo's search, with the amounts held to 30 to 400 (the range of the amounts the call gives), finds runs in which `assign.do` is refused with `over_capacity`
  = Declare it as an error of the task and handle it (`errors over_capacity` and `on over_capacity =>`).
```

</div>

**An amount chobo does not take.** In another version of the rule, a handback gives 20 seats back,
as −20. chobo takes an amount from 0 to 2⁶³ − 1, and a call with another fails rather than being
refused:

<div class="cb-term" markdown>

```text
error[ritsu E203]: booking.flow:18:1: The task `give_seats` can give `count` the amount -20, which chobo does not take
    18 |   give_seats(event: event, count: need.needed)
  = `need.needed` is the output `needed` of the rule "seats.rule", which comes to -20 at event_kind = handback
  = chobo takes an amount from 0 to 9223372036854775807 in the unit's smallest step, and a call with any other fails rather than being refused.
  = Make the rule's amounts 0 or more (a negative one is a transfer the other way), or branch on `kind` with `match` before the transfer and give the transfer the output only in the arms without `handback`.
```

</div>

The workflow can keep the rule as it is: `ritsu check` counts the rule's output over what the runs
that reach the transfer can give the call, so a `match kind` before the transfer that gives the
seats only in the arms of a workshop and a talk passes.

**A hold that has always expired.** A workflow holds goods for 14 days and posts the hold after it
waits for a payment day that koyomi counts, 18 days or more later:

<div class="cb-term" markdown>

```text
error[ritsu E206]: invoice.flow:31:1: The hold of the case `goods` has always expired when `ship` posts it
    31 |   goods <- ship(order: order, sku: sku)
  = `ship` comes 17 days 9 hours or more after the hold is made (line 27), and the hold expires 14 days after it is made (`reserve` of "stock.book")
  = the wait until `due.at` (line 30) lasts until the time of koyomi's date `payment`: 18 days after `received` at the fewest (from 2026-02-20 to 2026-03-10), at 09:00, and the `now` given to the date was read after the hold (line 29)
  = The book refuses it with `expired` on every run, so what follows `ship` going through never runs.
  = Make the hold last longer (the transfer's `pending expires after`), or make the call sooner.
```

</div>

When the rule's amounts all fit the hall, no run comes to `over_capacity`, and a task that declares
it anyway gets a warning rather than a pass: chobo's search goes only as deep as its check does, so
all it can say is that the refusal does not come within that depth.

**A hold, and an answer waited for until a time.** In another version, the workflow asks for an
approval instead of waiting for the payment day, waits for the answer until 09:00 of that day
(`timeout until due.at`), and puts the goods back when none comes. How long that takes depends on
the day the date is given, so whether the hold has expired cannot be decided, and the warning says
the day that makes the wait the longest. Every call between the hold and `put_back` has an upper
bound: the approval's id is handed on within a minute and its wait ends at the date's time, and a call
of a rule or of a date takes 33 seconds at the most on every platform.

<div class="cb-term" markdown>

```text
warning[ritsu W206]: invoice.flow:45:1: Whether the hold of the case `goods` has expired when `put_back` voids it cannot be decided
    45 |       goods <- put_back(order: order, sku: sku)
  = `put_back` comes 0 seconds to 51 days 9 hours 1 minute 1 second after the hold is made (line 40), and the hold expires 14 days after it is made
  = The longest is when the `now` given to `payment` (line 42) falls on 2026-07-21: `approval` (line 43) waits until `due.at`, 2026-09-10 at 09:00, 51 days after it
  = The book refuses a call on an expired hold with `expired`, which the flow handles here.
```

</div>

## A run with the book in it

`ritsu run` runs a workflow with its rules computed by rulec, its dates by koyomi and its book by
chobo, and every other task answered by a scenario. It shows the book's balances and holds at the
end, as `chobo run` does (`crates/ritsu/tests/projects/invoice`):

<div class="cb-term" markdown>

```text
invoice.flow (scenario scenarios/invoice_paid.json, from 2026-04-15T10:00:00Z)
before the run, book stock:
  receive.do(delivery: "D-1", sku: "pen", qty: 10)  done
the run:
  reserve(order: "A-1", sku: "pen", qty: 3)        chobo     done
  method(member: true, amount: 120)                rulec     {"billing":"invoice"}
  terms.payment(received: "2026-04-15T10:00:00Z")  koyomi    {"day":"2026-05-08","at":"2026-05-08T09:00:00Z"}
  wait until 2026-05-08T09:00:00Z                            22 days 23:00:00, to 2026-05-08T09:00:00Z
  check_payment(order: "A-1", due: "2026-05-08")   scenario  {"paid":true}
  ship(order: "A-1", sku: "pen")                   chobo     done
end: succeed {"outcome":"shipped","due":"2026-05-08"}
book stock (books/stock.book) at the end:
  accounts:
    shelf(pen)  posted 7, held out 0, held in 0
    suppliers   posted -10, held out 0, held in 0
    customers   posted 3, held out 0, held in 0
  holds:
    reserve(A-1, pen)  posted
```

</div>

## A gate guards each operation

In the small shop of ritsu's playground, the warehouse's gate says who may move its stock
([website/playground/shop](https://github.com/i2y/ritsu/tree/main/website/playground/shop)). Its
actions guard the book's operations, `<transfer>.<operation>`:

```gate
use book inventory from "../warehouse/inventory.book"
…
action hold_stock
  description "Hold the stock an order takes, for 30 minutes at most"
  guards inventory reserve.hold
  principal Staff, System
  resource Stock from sku
…
permit the_system_keeps_the_holds
  description "The warehouse's system holds the stock an order takes, posts the hold as the order ships, and voids it when the order is cancelled"
  principal is System
  action hold_stock, post_hold, void_hold

expect deny holds_are_not_moved_by_hand
  description "No member of the staff holds, posts or voids the stock of an order by hand: the holds go with the orders"
  principal is Staff
  action hold_stock, post_hold, void_hold
```

`sekisho check` walks every combination of principal, action and resource, and decides each as
Cedar does; an operation the book does not have, or one two actions guard, is an error. The service
that keeps the book asks the gate before it moves the book, through the Cedar and the code
`sekisho gen` writes. A workflow's own calls of the book are not asked of a gate: chobo refuses for
reasons of the book, none of which stands for a denial.

## A requirement it satisfies

The shop's requirements say where the warehouse's decisions come from, and name what meets them,
down to one operation of one transfer:

```req
requirement holds_go_with_the_orders
  text "The stock an order takes is held for it by the warehouse's system, which posts the hold when the order ships and voids it when the order is cancelled; no one moves a hold by hand"
…
  satisfied by chobo "../warehouse/inventory.book" transfer reserve operation hold
    reviewed 2026-10-07 by warehouse_lead sha256:3bb72bea70d71547 -> sha256:ef505e9c8dbb3bf1
```

The hash after the arrow is of the definition chobo hands over for the operation: the transfer as
`chobo api` describes it, in a fixed form, with the operation and the names of the reasons it can be
refused with. A
change to the transfer's moves, key or hold, or to what it can be refused with, makes yuen say that
the link is no longer what was reviewed.

## The map of contexts

The shop's map gives the warehouse's context the directory that holds the book
(`owns dir "../warehouse"`), and sakai checks the references that cross from one context into
another, in the files of every language. A `use book` and a transfer task are not among them: the
host's client of the book applies a transfer, and another context uses a book through the service
in front of it.

The shop opens in the browser, with the book among its files:
[the shop's book in the playground](../playground/#project=shop&file=warehouse/inventory.book).
