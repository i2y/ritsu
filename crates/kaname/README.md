# kaname

A small language for writing the functional core of a program — the part that only computes and decides — in ritsu's types: functions, records, enums and lists, with no input or output of its own.
The rest of the program — reading a request, the clock or a database, and carrying out what the core decides — stays in the host, the imperative shell that calls it.
kaname checks the core by the rules ritsu's languages are checked by — units, currencies and tax, the steps of rates, ranges, rounding and int64 — and works out what each function can do (`total`, `fails …`, `diverges`).
Today it is shaped for the core around business rules: it calls the rules of rulec, the dates of koyomi and the transfers of chobo's books in ritsu's types. Extensions that would make it more general are being considered.
kaname is one of the nine languages of [ritsu](../../README.md): it reads the rules, the dates files and the books through ritsu, and `ritsu gen` writes it as TypeScript, Python and Go beside their code.

A rule decides, a dates file computes a day, a book moves an amount, and each is checked by its own language.
The code that joins them is host code: it puts a rule's inputs together from a cart, carries one answer into the next call and hands an amount to a book.
The package `ritsu gen` writes gives each unit one type, so the host's type checker stops a fee in dollars handed to a book of cents; but nothing checks what that code computes: a rounding left out, a sum outside the range a rule takes, running totals that break a rule's precondition, a product past int64.
kaname is that code — the functional core between them — written in ritsu's types and held to their rules:

```kn
module late_payment v1
description "What a supplier claims for an invoice paid late: the day it fell due, how many days late it was paid, and the fixed sum and the interest the rule gives for them"

use dates terms from "dates/payment_terms.cal"
use rule charges from "rules/late_payment_charges.rule"

record Invoice
  id       : string
  invoiced : date  range >=2026-01-01 <=2027-12-31
  amount   : money[GBPc]  range >=1GBPc <=100_000_000GBPc

record Claim
  due          : date
  days_late    : number  range >=1 <=1500
  compensation : money[GBP]
  interest     : money[GBPc]

fn claim(invoice: Invoice, paid_on: date range >=2026-01-01 <=2029-12-31, reference_rate: rate[step 0.01%] range >=0% <=15%) -> Claim? total
  description "None when the invoice was paid by the day it fell due"
  let due = terms.due(invoiced: invoice.invoiced)
  let late = paid_on - due
  if late >= 1
    let c = charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate)
    Claim { due: due, days_late: late, compensation: c.compensation, interest: c.interest }
  else
    none
```

koyomi says the day the invoice fell due, a rule of rulec what the supplier may claim for the days it was paid late, and kaname counts the days between and narrows them to the rule's range before it calls the rule.
`ritsu check` reads the three languages at once:

```console
$ ritsu check examples/late_payment --root examples/late_payment
ok examples/late_payment/rules/late_payment_charges.rule
ok examples/late_payment/rules/遅延の請求額.rule
examples/late_payment/dates/payment_terms.cal: ok — every date computes on all 730 days of invoiced (2026-01-01..2027-12-31); 2 examples match
examples/late_payment/dates/支払期限.cal: ok — every date computes on all 730 days of 請求日 (2026-01-01..2027-12-31); 2 examples match
ok examples/late_payment/late_payment.ja.kn: 1 function, total
ok examples/late_payment/late_payment.kn: 1 function, total
ritsu check: 6 files (rulec 2, koyomi 2, kaname 2): all pass; borders between the languages: 2 checked, 0 undecided
```

kaname finds that `paid_on - due` is −758 to 1,431 days, and 1 to 1,431 in the branch of `late >= 1`, inside the rule's 1 to 1,500; the debt and the rate it gives the rule are inside theirs; and ritsu finds that the day it gives koyomi is inside koyomi's range.
So `claim` answers every input in the ranges it declares, as `total` on its head says.

## What the checker says

A fee counted in dollars, given to the book that counts cents, is a type error, with the conversion to write:

```console
$ ritsu kaname check tests/docs/dollars.kn
error[E102]: tests/docs/dollars.kn:7:67: The argument `fee` of the transfer `ledger.processing_fee.do` is money[USDc]; this is money[USD]
     7 |   ledger.processing_fee.do(payment: payment, seller: seller, fee: fee)
  = The unit `USD scale 2` of the book `ledger` counts money[USDc].
  = `<value> in USDc` converts it: money[USD] and money[USDc] are two units of one dimension, and nothing converts one into the other unasked.
```

Without the branch, the days late reach the rule unnarrowed, and kaname shows an input that puts them outside its range:

```console
$ ritsu kaname check tests/docs/unnarrowed.kn
error[E301]: tests/docs/unnarrowed.kn:15:44: `late` can be outside >=1 <=1500, the range of the input `days_late` of the rule `charges`
    15 |   charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate).interest
  = For example, `interest(invoice: Invoice { id: "a", invoiced: 2026-01-01, amount: 50000000GBPc }, paid_on: 2026-01-01, reference_rate: 0%)` gives it -29.
  = Narrow it to that range with an `if` before it goes there, or widen the range it goes to.
```

An error has such an input, found among the function's vectors or solved for: the end of a range kaname estimates is never one, since it may not happen.
A place kaname can neither show nor break is a warning, and the generated code checks it when it runs.
What crosses into another language is ritsu's to hold: the amount a payout gives the seller, the payment less the rule's fee, can be one chobo does not take:

```console
$ ritsu check tests/docs/net.kn examples/payout/rules/card_fee.rule examples/payout/books/ledger.book --root .
ok examples/payout/rules/card_fee.rule
examples/payout/books/ledger.book: ok
  capture.do         may be refused: key_conflict
  processing_fee.do  may be refused: insufficient_balance (seller_balance(seller) at least 0.00), key_conflict, already_refused
  key: capture once per payment; a second call that differs only in seller or amount is refused with key_conflict
  key: processing_fee once per payment; a second call that differs only in seller or fee is refused with key_conflict
error[kaname E401]: tests/docs/net.kn:13:1: `capture` is written `total`, but it can fail with `range`
    13 | fn capture(payment: Payment) -> ledger.capture.do total
  = Line 15: `payment.amount - fee`, given to `amount` of the transfer `ledger.capture.do`, can be an amount chobo does not take (ritsu E203)
  = Make the places shown (narrow the values with an `if` before they go there), or write a wider effect.
error[ritsu E203]: tests/docs/net.kn:15:74: `payment.amount - fee`, given to `amount` of the transfer `ledger.capture.do`, can be an amount chobo does not take
    15 |   ledger.capture.do(payment: payment.id, seller: payment.seller, amount: payment.amount - fee)
  = For example, `capture(payment: Payment { id: "a", seller: "a", amount: 1USDc, international: true })` gives it -48.
  = chobo takes an amount from 0 to 9223372036854775807 in the unit's smallest step.
  = kaname's generated code checks the amount where it builds the transfer, and the function fails there with `range`.
  = Narrow the amount with an `if` before the transfer, or make a negative amount a transfer the other way.
ritsu check: 3 files (rulec 1, chobo 1, kaname 1): 1 fail (2 errors); borders between the languages: 2 checked, 0 undecided
```

## Effects

Each function's type says how far it was checked.
A **total** one answers every input in the ranges it declares, as a rule and a date of koyomi do.
One that **fails** has places the checks could not show, and says with what: `rule`, `date`, `range`, `overflow`, `zero`.
One that **diverges** recurses.
kaname infers the effect and holds a function to the one its head writes (E401); a call carries the callee's effect to the caller.
The effect shapes the code: in Go only a function that can fail returns `(T, error)`, and in TypeScript and Python each has the union of what it can fail with.

## Decisions belong in tables

kaname computes, so a decision can be written in its code: an `if` that chooses a fee by the total it compares.
In a table, rulec would check it is complete and without overlap, and the person who approves it would read it on its page.
kaname warns of it, and a function that only narrows a value to a rule's range before calling the rule is not warned of:

```console
$ ritsu kaname check tests/doc/decisions.kn
warning[W301]: tests/doc/decisions.kn:20:24: Whether `weight` stays in >=1oz <=240oz, the range of the input `weight` of the rule `media_mail`, cannot be decided
    20 |     media_mail(weight: weight, media_only: all(lines, l => l.media)).price
  = The analysis of ranges finds the value in >=0oz <=240oz, and no input among the function's vectors takes it out.
  = The generated code checks it there when it runs.
  = Narrow it with an `if`, which the analysis reads, and it is shown.
warning[W501]: tests/doc/decisions.kn:25:3: This `if` chooses a constant of the business by the values it compares (`0USD`, `5USD`): a decision, which belongs in a table
    25 |   if total >= 35USD then 0USD else 5USD
  = Write the decision in a rulec table and call the rule: there, rulec checks it is complete and without overlap, and the person who approves it reads it on its page.
ok tests/doc/decisions.kn: 3 functions: 2 total; 1 fails rule
```

## The code it generates

`ritsu gen` writes each module under `glue/` in the package it writes for each of TypeScript, Python and Go, with a helper module of exact fractions, int64 and the five roundings, and no dependency.
Amounts travel in the package's one type of each unit, as the rules and the books take them; a function the host calls checks its arguments against what they declare, and the code checks the places the analysis did not show, failing with `KanameError` of the kind the reference interpreter fails with.
A transfer is a value whose type carries the reasons the book can refuse it with, and a function of the book makes it through chobo's client.

## The page for people

`ritsu kaname doc` writes a page for the person who approves the decisions and the engineer who reads how far each function was checked: the rules and the dates files it calls, with the pages rulec and koyomi draw of them, and the transfers it builds; its functions, total, checked when they run (each place, and why), or that may not stop; the decisions written in its code; and what its examples answer.
The Markdown is for a pull request; the HTML is one file that reads nothing from outside.
[The page of the late payment](../ritsu/tests/golden/kaname-doc/late_payment.md) is the example's, with the pages of the rule and the dates file left out.

## For AI agents

[skills/kaname](../../skills/kaname/SKILL.md) is a skill for an agent that writes or fixes a module: the loop, the language on one page, what to ask a person, and how to fix each diagnostic, kaname's and ritsu's across the languages.
Its pages besides `SKILL.md` are copies of `docs/`, made by `skills/sync.sh`.

## Install

kaname comes with ritsu:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

Run it as `ritsu kaname …`.
Called through a link named `kaname`, ritsu is kaname, with rulec, koyomi and chobo joined; the archives of a release carry the link, and `ln -s ritsu kaname` beside the binary makes one.
The binary of kaname's own crate joins no other language: it stops with E202 at a module that reads a rule, a dates file or a book.

## Commands

```console
$ ritsu kaname check examples/late_payment/late_payment.kn
$ ritsu kaname run examples/late_payment/late_payment.kn claim --input claim.json --trace
$ ritsu kaname vectors examples/late_payment/late_payment.kn --function claim
$ ritsu kaname doc examples/late_payment/late_payment.kn --root examples/late_payment --format html
$ ritsu kaname api examples/late_payment/late_payment.kn --root examples/late_payment
$ ritsu kaname explain E301
```

`run` runs a function in the reference interpreter on an input of JSON, and `vectors` prints the inputs it chooses for each function with their answers.
Every command takes `--lang ja|en` (else `KANAME_LANG`, then `RITSU_LANG`, else English) and `--help`.
The exit code is 0 for no errors, 1 for errors, and 2 for bad arguments, a file that cannot be read, or a language that is not joined.
[docs/reference.md](docs/reference.md) has the whole language and every command.

## Examples

Each is in English, and in Japanese beside it as `<name>.ja.kn` reading the Japanese versions of its files.
The rules cite the documents they transcribe, from copies beside them that they pin.

- [examples/payout](examples/payout): a card payment taken for a seller of a marketplace, with the processor's fee from a rule of rulec, as two transfers of a chobo book that the host makes. From a card processor's published merchant fees.
- [examples/late_payment](examples/late_payment): what a supplier claims for an invoice paid late: the day it fell due from a dates file of koyomi, the days late, and the fixed sum and the interest from a rule. From the UK's Late Payment of Commercial Debts (Interest) Act 1998 and GOV.UK's guide.
- [examples/media_mail](examples/media_mail): the weight of a cart, each item's weight times its quantity, and whether every item is media, given to a rule of Media Mail's prices, whose answer is none for a parcel that is not Media Mail. From the United States Postal Service's Notice 123.
- [examples/discount_split](examples/discount_split): a discount spread over the lines of a bill by their amounts, a line at a time through a rule that `allocate`s, the running totals kept by `running_totals`, so that the shares come to the discount. From Virginia's regulation on discounts, 23VAC10-210-250.

## How it is checked

`cargo test -p kaname` runs the language on its examples and its test modules, and runs the generated code.

- Its rounding is held to rulec's on 360,600 values and steps, and its dates to ritsu-ports', koyomi's and rulec's on every one of 3,652,059 days.
- Every `derive` and `define` of rulec's corpus, 143 lines in 85 files, reads as kaname with rulec's type and gives rulec's value on rulec's 20,033 vectors.
- The analysis of ranges is held to the reference interpreter: 513,899 values of 361 expressions, seen on 24,135 runs of every function of the examples and the test modules (its vectors, and 200 inputs drawn from what its parameters declare), are inside what it says, and each of the 1,455 runs that fail fails in a way the function's effect says.
- Its evaluation and its analysis of ranges are written again in Lean (`proofs/KanameCore`, in ritsu's one Lean package), with proofs that the analysis is sound and that a function it finds total does not fail; `cargo test -p ritsu-model` holds the model to the reference interpreter and the analysis on 804 runs of 32 modules.
- The code generated in TypeScript, Python and Go answers every vector of 24 modules and every input outside what their parameters declare as the reference interpreter does, 1,111 of 1,111 in English and in Japanese, and passes `tsc --strict --erasableSyntaxOnly`, `mypy --strict`, gofmt and `go vet`.
- Every diagnostic code has the smallest module that prints it, in English and in Japanese; the pages of `doc` are golden files, the pages of the rules and dates in them are what rulec and koyomi draw, and Chrome opens the HTML.
- The outputs, diagnostics and lines of `.kn` in this README, `docs/` and the skill are real: `tests/docs.rs` compares them, and ritsu's `tests/kaname.rs` runs the commands.

## Status

The language, `check`, `run`, `vectors`, `doc`, `api`, `explain`, and the code `ritsu gen` writes are done, as [DESIGN.md](DESIGN.md) says.
Not yet: calls from dandori's flows, a calendar of koyomi and the time of a date, and the narrowing of a rule's outputs to the inputs given (DESIGN 19).

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT), at your option.
