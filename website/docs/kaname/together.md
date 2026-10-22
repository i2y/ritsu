# Rules, dates and books

A rule of [rulec](../rulec/) decides, a dates file of koyomi computes a day, and a book of
[chobo](../chobo/) moves an amount; each is checked by its own language. The code around them puts a
rule's inputs together, carries one answer into the next call and hands an amount to a book. Written
in the host's language, the package `ritsu gen` writes gives each unit one type, so the host's type
checker stops a fee in dollars handed to a book of cents. Nothing checks what that code computes,
though: a rounding left out, a sum outside the range a rule takes, running totals that break a rule's
precondition, a product past int64. Written in kaname, it calls the rules, the dates and the books
in their own types, and is held to what they declare.

## A claim for an invoice paid late

What a supplier may claim for an invoice paid late (`crates/kaname/examples/late_payment`, from the
UK's Late Payment of Commercial Debts (Interest) Act 1998 and GOV.UK's guide to it):

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

koyomi says the day the invoice fell due, the rule what the supplier may claim for the days it was
paid late, and kaname counts the days between and narrows them to the rule's range before it calls
the rule. `ritsu check` reads the three languages at once:

```console
$ cd crates/kaname/examples/late_payment
$ ritsu check
ok rules/late_payment_charges.rule
ok rules/遅延の請求額.rule
dates/payment_terms.cal: ok — every date computes on all 730 days of invoiced (2026-01-01..2027-12-31); 2 examples match
dates/支払期限.cal: ok — every date computes on all 730 days of 請求日 (2026-01-01..2027-12-31); 2 examples match
ok late_payment.ja.kn: 1 function, total
ok late_payment.kn: 1 function, total
ritsu check: 6 files (rulec 2, koyomi 2, kaname 2): all pass; borders between the languages: 2 checked, 0 undecided
```

kaname finds that `paid_on - due` is −758 to 1,431 days, and 1 to 1,431 in the branch of
`late >= 1`, inside the rule's 1 to 1,500; the debt and the rate it gives the rule are inside theirs;
and ritsu finds that the day each module gives koyomi is inside koyomi's range, the two borders the
last line counts. So `claim` answers every input in the ranges it declares, as `total` on its head
says. Without the branch, the check finds an input that takes the days outside the rule's range
([What the check guarantees](checks.md)).

`ritsu kaname run` runs the function with the rule and the dates file joined, and shows each call
with its answer. GOV.UK's own example is £1,000 paid 50 days late, at a base rate of 0.5%:

```console
$ cd crates/kaname
$ ritsu kaname run examples/late_payment/late_payment.kn claim --input tests/inputs/late_payment.json
claim(invoice: Invoice { id: "inv_1", invoiced: 2026-03-02, amount: 100000GBPc }, paid_on: 2026-05-20, reference_rate: 0.5%)
  line 20: terms.due(invoiced: 2026-03-02) = 2026-03-31
  line 23: charges(debt: 100000GBPc, days_late: 50, reference_rate: 0.5%) = { compensation: 70GBP, interest: 1150GBPc }
= Claim { due: 2026-03-31, days_late: 50, compensation: 70GBP, interest: 1150GBPc }
```

## In their own types

A fee counted in dollars, given to a book that counts cents, is a type error, with the conversion
to write:

```console
$ cd crates/kaname
$ ritsu kaname check tests/docs/dollars.kn
error[E102]: tests/docs/dollars.kn:7:67: The argument `fee` of the transfer `ledger.processing_fee.do` is money[USDc]; this is money[USD]
     7 |   ledger.processing_fee.do(payment: payment, seller: seller, fee: fee)
  = The unit `USD scale 2` of the book `ledger` counts money[USDc].
  = `<value> in USDc` converts it: money[USD] and money[USDc] are two units of one dimension, and nothing converts one into the other unasked.
```

A value of an enum with fields, a dict or a set, a `result` and a function value go to no rule,
date or transfer as they are: a module splits them with `match`, or makes them a list or a value of
the type the rule takes.

## Across the languages

What crosses into another language is checked by ritsu, with what each language has checked of its
own. The amount a payout gives a seller, the payment less the rule's fee, can be one chobo does not
take:

```console
$ cd crates/kaname
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

ritsu holds a rule's preconditions at every call a module makes, a day of koyomi given to a rule's
date input against the rule's range, a day given to a date of koyomi against the dates file's range,
and an amount given to a transfer against what chobo takes, with the reasons the book can refuse the
transfer with. What it shows leaves the effect; what it cannot decide is checked by the code when it
runs.

## In the package

`ritsu gen` writes a module beside the code of the rules, the dates and the books it calls, in the
one package of each of TypeScript, Python and Go, and the module's code calls theirs. A transfer is a
value whose type carries the reasons the book can refuse it with; a module builds it and the host
makes it, through the function the package writes for the book, which calls chobo's client.

## The other examples

- [payout](https://github.com/i2y/ritsu/tree/main/crates/kaname/examples/payout): a card payment
  taken for a seller of a marketplace, with the processor's fee from a rule, as two transfers of a
  book that the host makes.
- [media_mail](https://github.com/i2y/ritsu/tree/main/crates/kaname/examples/media_mail): the weight
  of a cart and whether every item is media, given to a rule of Media Mail's prices, from the United
  States Postal Service's Notice 123.
- [discount_split](https://github.com/i2y/ritsu/tree/main/crates/kaname/examples/discount_split): a
  discount spread over the lines of a bill by their amounts, a line at a time through a rule that
  `allocate`s, with the running totals kept by `running_totals`, from Virginia's regulation
  23VAC10-210-250.
