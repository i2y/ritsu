---
name: kaname
description: Write, check and fix kaname modules (`.kn`), the code that joins rulec's rules, koyomi's dates and chobo's books — putting a rule's inputs together, adding up a cart, spreading an amount over lines by running totals, carrying one answer into the next call, building the transfers a host makes — in ritsu's types, held to the same rules of units, rounding, ranges and int64, with each function's effect (total, or the places checked when it runs) in its type, and generated as TypeScript, Python and Go by `ritsu gen`. Use when the code between rules, dates and books has to be written or checked; when a kaname diagnostic (E001-E602, W301-W501, W901) or ritsu's E201-W205 across the languages has to be fixed; or when a person has to be shown which functions are total and which decisions are in tables.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run kaname as `ritsu kaname <command>`, or as `kaname <command>` through a link to ritsu named for it. A module has no input or output of its own; the host reads and writes, and makes the transfers a function returns.
license: MIT OR Apache-2.0
---

## When this applies

The job is **the code between the decisions**: a rule of rulec decides, a dates file of koyomi
computes a day, a book of chobo moves an amount, and something has to put a rule's inputs together
from the host's data, carry an answer into the next call, spread an amount over lines, and hand an
amount to a book. kaname writes that code in ritsu's types, checks it by the rules the other
languages are checked by, and says for each function whether it answers every input it declares
(`total`) or which places the code checks when it runs.

It does not apply to a decision itself (a threshold, a fee, a category chosen by a value: that is a
rulec table), to a day computed from a day (koyomi), to a balance kept by a book (chobo), to
waiting and side effects in order (a dandori workflow), or to input and output of any kind (the
host's).

The files bundled with this skill are `reference.md` (the whole language and the commands) and
`codes.md` (every diagnostic). Read them when you need them, not all up front.

---

# Working with kaname

Your part is to write the module, get it past `ritsu kaname check` and `ritsu check`, and generate
its code with `ritsu gen`. Three things stay with people:

1. **The decisions.** A choice of an amount, a rate or a category by a value belongs in a rulec
   table, where it is checked complete and shown to the person who approves it. Never write one in
   kaname to make a check pass; W501 says where one is.
2. **The ranges a function declares.** They are what the host promises, and what the analysis
   reads. Widening one to make a rule's call pass changes the promise: ask.
3. **What a function promises.** `total` on a function's head is a promise to the host (and, later,
   to a workflow). Whether a function may fail at run time, and how the host handles it, is a
   person's to say.

## 1. The loop

1. Read the rules, the dates files and the books the code will call (`ritsu rulec doc`, `ritsu
   koyomi doc`, `ritsu chobo doc`), and the host's data the code is given.
2. Write the module: the heading, the `use` lines, the records with the ranges the host promises,
   the functions, and `examples` from a case a person can check.
3. Run the check and fix what it says, one diagnostic at a time, from the top; `ritsu kaname explain
   <code>` says how. Then `ritsu check` on the project, which adds ritsu's checks across the
   languages (X2, X3, X4, X6) and hands kaname their verdicts.
4. Generate the code into the host's package (`ritsu gen`), and have CI run `ritsu gen --check`.
5. Write the page with `ritsu kaname doc`, and show it to the people who approve the decisions.

```console
$ ritsu kaname check examples/late_payment/late_payment.kn
$ ritsu kaname explain E301
$ ritsu kaname run examples/late_payment/late_payment.kn claim --input claim.json --trace
$ ritsu kaname doc examples/late_payment/late_payment.kn --root examples/late_payment --format html
```

Run every command as `ritsu kaname <command>` (through a link to ritsu named kaname, `kaname
<command>` is the same): it reads the rules, the dates files and the books a module names in the
same process, where kaname built alone from its crate reads no other language and stops with E202.
Add `--lang ja` for Japanese.

## 2. The language on one page

```kn
module late_payment v1
description "What a supplier claims for an invoice paid late: the day it fell due, how many days late it was paid, and the fixed sum and the interest the rule gives for them"

use dates terms from "dates/payment_terms.cal"
use rule charges from "rules/late_payment_charges.rule"

record Invoice
  id       : string
  invoiced : date  range >=2026-01-01 <=2027-12-31
  amount   : money[GBPc]  range >=1GBPc <=100_000_000GBPc

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

```kn
record Settlement
  capture : ledger.capture.do
  fee     : ledger.processing_fee.do

fn split(lines: list[Line] count >=1 <=10, discount: money[USDc] range >=0USDc <=10_000_000USDc) -> list[Share] total
  map(running_totals(lines, l => l.amount), r =>
    Share { label: r.item.label, share: discount_share(discount: discount, before: r.before, upto: r.upto, total: r.total).share })
```

- The parts come in this order (E003): `module`, `description`, `use`, then `enum`, `record` and
  `fn` in any order, then `examples`. Lines of a block are indented with spaces.
- Types and arithmetic are rulec's: `money[GBP]`, `mass[oz]`, `rate[step 0.01%]`, `number`, `date`,
  `bool`, `string`, `T?`, enums; and `list[T]`, records, a rule's `outputs`, a transfer's
  `ledger.capture.do`. Units never convert unasked: `fee in USDc`.
- A value is **unrounded** when it is off the step of its type (an amount times a rate, a value
  divided by a value), and goes nowhere until a rounding puts it on one: `half_up(x, 1USDc)` (E103).
- Every number, date and list a parameter reaches carries its range or `count` (E108); a rate its
  step. These are the host's promise, checked before a function computes (`input`).
- A rule is called with its inputs by name and answers a record of its outputs (`.fee`); a date of
  koyomi answers a day; a transfer is a value the host makes. A date has a difference (`b - a`, in
  days) and no `+`.
- Builtins: `count`, `sum`, `any`, `all`, `map`, `filter`, `first`, `max_of`, `min_of`,
  `running_totals`, `reduce`, with a function written where it is given; rulec's `min`, `max`,
  `mod`, `isqrt`, `allocate` and the five roundings; `starts_with`, `concat`, `text`.
- `running_totals` is how an amount is spread: kaname knows `before <= upto <= total`, which a
  rule's preconditions ask.
- A name that is not ASCII carries an alias for the generated code: `遅れの請求(claim)`.
- The effect at the end of a function's head is checked when written: `total`, `fails rule, range`,
  `diverges`.

## 3. What to ask a person

- Who decides each choice a function would make, before writing an `if` that answers a constant:
  it may belong in a rule (W501).
- What range the host can promise for each parameter, and the most lines a list can have.
- Whether a function must be `total`, when a place cannot be shown (W301, W302, W303, and ritsu's
  W201-W205): narrow the value with an `if`, or say the function `fails` and let the host handle it.
- Whether a rule's input should take more, when a value kaname gives it can be outside its range
  (E301): the rule is a person's.
- Which way a rounding goes, when the documents do not say.
- What the host does with each reason a transfer can be refused with.

## 4. From a diagnostic to a fix

`ritsu kaname explain <code>` (and `codes.md`) gives each code's cause, its fix and its smallest
reproduction. The usual ones:

| Code | What it means | What to do |
|---|---|---|
| E001-E004 | a word, a line, a part's order or the indentation the language does not read | fix the line |
| E005-E007 | a name without an ASCII alias, given twice, or one the generated code cannot take | add or change the alias |
| E101 | a name names nothing | fix the spelling, or declare it |
| E102 | types or units that do not agree, a currency or a tax that differs | convert with `in` where the dimension is one; else the value is the wrong one |
| E103 | an unrounded value where a rounded one goes | round it with the step the place takes; ask which way |
| E104 | a step, a rounding's step or a conversion that does not fit | write a whole number of the unit, or a step of the rate |
| E105 | an operation the type does not have (`date + 30`, a string compared) | use koyomi for days; compare what compares |
| E106 | a `match` that leaves values out | add the arm |
| E107 | a call that does not give what it takes | give every input by its name; a builtin's arguments in order |
| E108 | a range, a count or a step missing or impossible | write what the host promises |
| E201 | a file a `use` reads does not pass its check, or cannot be read | fix that file with its language |
| E202 | run without the other languages | run it as `ritsu kaname` |
| E204 | a transfer or an operation the book does not have | follow the book |
| E301, W301 | a value can be outside the range of where it goes, or it cannot be decided | narrow it with an `if` before it goes (a value of an enum a rule holds to a range, with a `match` or an `if` that leaves a value out); ask whether the range should change |
| E302, W302 | a value can leave int64 | narrow the ranges, or round earlier |
| E303, W303 | a divisor can be 0 | branch on it before dividing |
| E401 | a function does more than the effect written on it | make its places shown, or write the wider effect after asking |
| W501 | a decision written in code | move it into a rule, after asking |
| E601, E602 | an example that does not hold, or is not a call | fix the function or the example; ask which is right |
| W901 | a key written in the `.kn` | take it out and keep it where the code runs; tell the person to revoke it if it is real (the history keeps it) |

`ritsu check` adds ritsu's codes across the languages, which `ritsu explain <code>` looks up:

| Code | What it means | What to do |
|---|---|---|
| E201, W201 | a call can break a rule's precondition, or it cannot be decided | give values kaname knows the relation of (`running_totals`), or branch on the precondition |
| E202, W202 | a day given to a rule's date input can be outside its range | narrow the day with an `if`, or ask whether the rule should take it |
| E203, W203 | an amount given to a transfer can be one chobo does not take (below 0) | narrow the amount, or make a negative one a transfer the other way |
| W204 | the reasons a transfer can be refused with cannot be decided | the type takes every reason of the book's bounds; the host handles them |
| E205, W205 | a day given to a koyomi date can be outside its range | narrow it, or give the date the day it takes |

Fix the first diagnostic first: an error of words, names or types stops the checks after it.

## 5. What it generates, and the page

`ritsu gen` writes each module under `glue/` in the package of each of TypeScript, Python and Go,
beside the code of the rules, dates and books it calls, with a helper module and no dependency.
Each function is one a host calls, which checks its arguments, and one the module's functions call.
Amounts travel in the package's one type of each unit; a rate as a count of its step. The code
checks the places the analysis did not show and fails with `KanameError` (`input`, `rule`, `date`,
`range`, `overflow`, `zero`), the line of the `.kn` and its message. In Go, only a function that can
fail returns `(T, error)`. A transfer is a value whose type lists the reasons it can be refused
with; the book's `apply` function makes it through chobo's client.

`ritsu kaname doc` writes the page for people: the rules and dates files called, with the pages
their languages draw, and the transfers built; the functions, total, checked when they run (each
place and why), or that may not stop; the decisions in code; and the examples' answers. The
Markdown is for a pull request; the HTML is one file that reads nothing from outside. Change the
module, not the page.
