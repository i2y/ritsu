# The kaname reference

kaname writes the code that joins rules, dates and books. A `.kn` module calls the rules of rulec,
the dates of koyomi and the transfers of chobo's books in ritsu's types, and computes between them:
it puts inputs together and adds them up, spreads an amount over lines, carries one answer into the
next call, and builds the transfers a host makes. `kaname check` holds that computation to the
rules the inner languages are held to: units, currencies and tax, the steps of rates, ranges,
rounding and int64. It finds where a value can leave the range of where it goes, and says, function
by function, whether it answers every input (`total`) or checks some places when it runs. `ritsu
gen` writes it as TypeScript, Python and Go beside the code of the rules, dates and books it calls.
A module has no input or output: what it is given, it computes from.

This page is the whole language and the commands. [codes.md](codes.md) has every diagnostic, with
the smallest module that prints it. DESIGN.md has the reasons.

## Files

A `.kn` is one module, in UTF-8. `#` starts a comment that runs to the end of the line, outside a
string. The lines of a block are indented with spaces under the line that opens it, and line up
(E004; a tab is E001). Inside round, square and curly brackets, line breaks and indentation are not
read, so a long call, record or list goes on over lines.

The parts of a module come in this order (E003 otherwise); all but the heading may be left out:

| Part | What it is |
|---|---|
| `module <name> v<N>` | the heading: the module's name and version |
| `description "…"` | what the module is for, once |
| `use …` | the files it reads, any number |
| `enum`, `record`, `fn` | its declarations, in any order among them |
| `examples` | calls of its functions with their answers |

A module refers to its enums, records and functions before or after they are declared, so two
functions may call each other. Inside a function, a `let` refers only to what is above it.

### Names and aliases

A name may be written in kanji, kana or Latin letters, and is compared as written. A name the
generated code takes (the module, a function and its parameters, a record and its fields, an enum
and its values) that is not an ASCII identifier carries an ASCII alias in parentheses (E005 without
one): `fn 遅れの請求(claim)(請求書(invoice): 請求書, …)`. A declaration may be referred to by its name
or by its alias. The names of `let`, of a parameter of a function given to a builtin and of `some`
need no alias.

Thirteen words cannot be names (E002): `let` `if` `then` `else` `match` `some` `none` `true`
`false` `and` `or` `not` `in`. The other keywords stand where nothing else can, so a field may be
`total` and a parameter `description`. An alias that is a reserved word of the TypeScript, Python
or Go it is generated in, or a name of kaname's helper module (`kaname`, `KanameError`, `Q`,
`Date`, `Rat`, `glue`), is E007. One name or alias given twice in one space is E006: the names of
a module's `use` lines and its declarations, the fields of a record, the parameters of a function,
the values of an enum, and the names inside a function, which also cannot be one a `use` line
gives. A function cannot take the name of a builtin.

The English and the Japanese versions of an example (`<name>.kn` and `<name>.ja.kn`) give their
declarations the same aliases and differ in the module's alias (`late_payment` and
`late_payment_ja`), so the two go into one package.

## The heading

```kn
module late_payment v1
description "What a supplier claims for an invoice paid late: the day it fell due, how many days late it was paid, and the fixed sum and the interest the rule gives for them"
```

`v1` is for people: it goes into the head of what `gen` writes and into `api`.

## Reading other files

```kn
use dates terms from "dates/payment_terms.cal"
use rule charges from "rules/late_payment_charges.rule"
```

`use rule`, `use dates`, `use book` and `use module` read a rule of rulec, a dates file of koyomi, a
book of chobo and another module of kaname. The word after `use` is the first word of the file it
reads, the name after it is what the module calls it by (`charges(…)`, `terms.due(…)`,
`ledger.capture.do(…)`), and the path is written from the module's directory with `/`. A file that
is not there, whose extension does not fit the word, or that does not pass its own language's
check is E201, with what that language says. The modules read one another without a ring (E203).
A rule's contribution is not read: `ritsu check` and `ritsu gen` put a project's contributions into
its rules, and kaname reads the rules as they stand then.

kaname reads the inner languages through ritsu. Run as `ritsu kaname …`, or as `kaname …` through
the link a release carries, it has rulec, koyomi and chobo joined. The binary of kaname's own crate
has none of them: a module that reads a rule, a dates file or a book stops it with E202 (exit 2),
and a module that reads only modules is read in full.

### The root

`--root` is the directory the paths in the JSON and on the page are written from; else the nearest
directory above the first file given that holds `.git`, else that file's directory.

## Types

| Type | Written | Travels as |
|---|---|---|
| number | `number` | a whole number |
| money, quantity | `money[GBP]`, `money[USDc, excl_tax]`, `mass[oz]`, `duration[d]`, any of rulec's | a whole number of the unit declared |
| rate | `rate[step 0.01%]` | a whole number of its step; `rate` without a step only in a `let` |
| ordered quantity | `temperature[℃]`, `sound[dB]` | a whole number, compared and ranged only |
| date | `date` | `"YYYY-MM-DD"` |
| truth, text | `bool`, `string` | |
| enum | a module's `enum`, a rule's `charges.band` | the value's name |
| record | a module's `record`, another module's `pricing.Order` | an object of its fields |
| list | `list[T]` | an array |
| optional | `T?` | the value, or `null` |
| a rule's outputs | `charges.outputs` | an object of its outputs |
| an element of a list a rule walks | `shipping.lines` | an object of its fields |
| a transfer | `ledger.capture.do` | `{"book", "transfer", "op", "args"}` |
| a running total | `running[T, W]` | `item`, `before`, `upto`, `total` |

There is no function as a value: a function is given only to a builtin, where it is written.

### Numbers and units

Types and arithmetic are rulec's. A unit is part of the type: `JPY` and `円` are one unit, `kg` and
`g` two, tax included and tax excluded two types. Values of one type add, subtract and compare; a
mix of units or of tax is E102. A number or a rate multiplies an amount or a quantity, and the
product of a number and a rate is a rate. Multiplying two dimensions, or dividing a number by a
quantity, is E105; a temperature and a sound are compared and ranged only.

A literal is a whole number of the unit it is written in (`0.5USD` is E104; a share is a rate,
`40%`), and is read in the unit of what it meets: `15lb` compared with a `mass[oz]` is 240 ounces,
`40GBP` added to a `money[GBP, incl_tax]` is tax included, and `30℃` compared with a
`temperature[℉]` is 86℉. A literal that is no whole number of that unit is E104. A bare `0` is
the 0 of any unit.

Unlike rulec, kaname divides by a value anywhere: `a / b` is unrounded, and a divisor the analysis
of ranges cannot keep from 0 is checked when the code runs (`zero`). An amount divided by an amount
of its unit is a ratio. `x in USDc` is `x` in another unit of its dimension, by the exact factor of
ritsu's table of units, and a temperature with its unit's zero too (`t in ℉` is `t × 9/5 + 32`);
there is no conversion between currencies, between tax included and
excluded, or between dimensions (E104), and no conversion that is not written: dollars given where
cents go are E102.

### Rounding

A value is rounded when it lands on the step of its type: a whole number of its unit, or of a
rate's step. An amount times a rate, a value divided by a value, a quantity converted to a larger
unit are unrounded, and an unrounded value cannot be given to a parameter or a result, a field, a
rule's or a date's input, a transfer's argument or a weight of `running_totals` (E103), until a
rounding puts it on a step: `up`, `down`, `half_up`, `half_down` and `half_even`, rulec's five,
with the step after the value (`half_up(amount × 3%, 1USDc)`). An unrounded value is an exact fraction and compares as it is; a
`let` may hold one. Rounding goes as rulec's does for a negative value too: `up` away from 0, `down`
towards it, a half away from 0 for `half_up`, towards it for `half_down`, to the even for
`half_even`.

`let x : T = …` writes a type for a value whose type does not say it: whether a value without a
dimension is a `number` or a `rate`, and the tax of an amount that has none. That is all a type on
a `let` does, as on rulec's `define`.

### Dates

`date` is a day of the calendar, with no time zone. `b - a` is the number of days from `a` to `b`,
a `number`. A date has no `+` (E105): a day after another is koyomi's to compute, over every day of
its range.

### Enums

`enum size = small | large`, and `enum tier = basic < silver < gold` for one whose values are in an
order, which compares and takes `min` and `max`. A value may carry a label (`gold "Gold"`). rulec's
`default` and `open` are E003: kaname has no table for them to mean anything in. An enum is the
place it is declared: a module's `tier` and a rule's `charges.band` are two types even when their
values are spelled alike, and a `match` maps one to the other. Two rules that read one enum of an
`enums` file whole (rulec's `import enum … from`) give one type, and a rule's enum whose values are
in an order compares. A word that reads two ways in one place, a name and a value, is E110.

### Records, lists and optionals

```kn
record Invoice
  id       : string
  invoiced : date  range >=2026-01-01 <=2027-12-31
  amount   : money[GBPc]  range >=1GBPc <=100_000_000GBPc
```

A record is built with every field (`Invoice { id: …, invoiced: …, amount: … }`; a field of type
`T?` may be left out, and is `none`), and a field is read with `.`. A record cannot hold itself
(E108). A list is built by a literal (`[a, b]`) and by the builtins; there is no index. An optional
`T?` is split with `match` into `none` and `some x`, and takes no arithmetic. An optional of an
optional is one optional.

### Ranges and counts

A number or a date a parameter reaches (itself, a field of a record it takes, an element of a list
it takes) carries a range, a list a parameter reaches carries the most elements it may have, and a
rate carries its step (E108 without them):

```kn
fn split(lines: list[Line] count >=1 <=10, discount: money[USDc] range >=0USDc <=10_000_000USDc) -> list[Share] total
```

A range is written in the unit of its type (`range <=15lb` on a `mass[oz]` is 240 ounces); `>` and
`<` mean a step inside (`>0` on a whole number is `>=1`, `<15%` on a `rate[step 0.01%]` is
`<=14.99%`). A rate with a step may leave its range out, and is then 0% to 100%. A function's
result, a `let`, and a field no parameter reaches need no range; one written there is held to. A
rate keeps its step in every field and result. A range on a string, a truth value, an enum or a
record, a `count` on what is not a list, an empty range and a range written twice are E108.

The ranges are what the host promises: a function's code checks them before it computes, and an
argument outside them fails with `input`, which no effect counts.

## Functions

```kn
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

A function has its parameters with their types, its result's type, and may write its effect last.
Its body is `let` lines and then the answer: an expression, an `if` block or a `match` block. A line
after the answer is E111. A function is called with its arguments by name
(`weight_of(lines: lines)`), as a rule is; a builtin with its arguments in order (E107 otherwise).

- `if <cond>` with the block under it, `else if`, and `else` (E111 without it); `if a then b else c`
  on one line.
- `match <value>` with an arm a line: `gold => 0USD`, `silver, basic => 3USD` for an enum, `true`
  and `false` for a truth value, `none` and `some x` for an optional. An arm may have a block under
  it. A value left out, or one an arm above already took, is E106.
- Comparisons are one at a time, joined by `and`, `or` and `not` (`a < b < c` is E001). `!=` is there;
  `==`, `&&` and `||` are E001, which says what to write.
- `×` `÷` `−` `≦` `≧` and full-width digits read as `*` `/` `-` `<=` `>=` and digits.

### Effects

A function's effect says how far it is checked:

| Effect | What it says | Written |
|---|---|---|
| total | it answers every input in the ranges it declares | `total` |
| fails | it has places the checks could not show, which the code checks when it runs; the kinds of failure are said | `fails rule, range` |
| diverges | it recurses, itself or through others; that it stops is not checked | `diverges` (`fails zero diverges`) |

The kinds are `rule` (a rule refuses what it is given: its range or its preconditions), `date` (a
date of koyomi refuses its input), `range` (a value leaves the range of a parameter, a result, a
field, or the amounts chobo takes, or `isqrt` is given less than 0), `overflow` (a value leaves
int64) and `zero` (a division by 0).

The effect is inferred, and checked where it is written: a function written `total` that has a
place not shown or a recursion is E401, as is one written `fails …` that can fail with a kind not
written, or that recurses without `diverges`. A function takes the effects of the functions it
calls.

### Builtins

| Function | Answers |
|---|---|
| `min(a, b)`, `max(a, b)`, `mod(x, n)`, `isqrt(x)`, `allocate(t, c, s)` | as rulec's; `mod` takes a positive whole constant |
| `up`, `down`, `half_up`, `half_down`, `half_even(x, step)` | `x` rounded to the step |
| `count(xs)`, `count(xs, x => cond)` | how many elements, how many keep the condition |
| `sum(xs, x => expr)` | the sum, 0 for no element |
| `any(xs, x => cond)`, `all(xs, x => cond)` | `false` and `true` for no element; they stop at the element that decides |
| `map(xs, x => expr)`, `filter(xs, x => cond)` | a list |
| `first(xs, x => cond)` | the first element that keeps the condition, or `none` |
| `max_of(xs, x => expr)`, `min_of(xs, x => expr)` | the largest and the smallest, or `none` for no element |
| `running_totals(xs, x => weight)` | a `running[T, W]` for each element: `item`, `before` (the sum before it), `upto` (the sum to it), `total` |
| `reduce(xs, first, (acc, x) => expr)` | for what the others cannot write |
| `starts_with(s, p)`, `concat(s, …)`, `text(x)` | text: a prefix, a join, a value as kaname writes it |

A function is given to a builtin where it is written (`l => l.weight × l.qty`), and nowhere else
(E109). Text has only what reads the same in every target: `=`, `!=`, a prefix, a join, and
`text`. There is no length, order, case or slice of a string.

`running_totals` is how a share is spread: given to a rule that `allocate`s a line's share between
`before` and `upto`, the shares come to the whole, and kaname knows `before <= upto <= total` for a
weight that is 0 or more, which a rule's preconditions ask. The weights go into the fields of the
records it answers, and so are rounded (E103): a whole number of their unit, or a rate on the step
its type says (`x => 1 / x` is E103; `x => half_up(1 / x, 1)` is not).

## Calling rules, dates and books

- **A rule**: `charges(debt: …, days_late: …, reference_rate: …)` answers the record
  `charges.outputs`, one field an output, even for a rule with one output (`.fee`). Each input is
  given by its name or its alias; an optional input may be left out. A type or unit that does not
  fit is E102 (ritsu's X1, held by kaname), and a value outside the input's range is kaname's to
  find (E301, W301). A list a rule walks is given as a list of its elements,
  `shipping(lines: map(xs, x => shipping.lines { amount: x.amount }), …)`.
- **A date**: `terms.due(invoiced: …)` answers the day; a date input takes a `date`, a whole input a
  `number`. A date a module gives is held to the date's range by ritsu's X6; a whole number, by
  kaname.
- **A transfer**: `ledger.capture.do(payment: …, seller: …, amount: …)` answers a value of the type
  `ledger.capture.do`: which transfer of the book, which operation, with which arguments. kaname
  writes no book. The host makes the transfer with chobo's client, and a function that builds
  several returns them all, each its own write. `do` and `hold` take every argument, `post` and
  `void` the key, and `post` every amount or none. The amount is in the book's unit
  (`unit USD scale 2` is `money[USDc]`), and dollars given to a book of cents are E102. A transfer
  or an operation the book does not have, or an argument missing or too many, is E204.

## Examples

```kn
examples
  claim(invoice: Invoice { id: "inv_1", invoiced: 2026-03-02, amount: 100_000GBPc }, paid_on: 2026-05-20, reference_rate: 0.5%)
    = Claim { due: 2026-03-31, days_late: 50, compensation: 70GBP, interest: 1150GBPc }
  claim(invoice: Invoice { id: "inv_2", invoiced: 2026-03-02, amount: 100_000GBPc }, paid_on: 2026-03-31, reference_rate: 0.5%)
    = none
```

Each line calls a function of the module with constants and writes its answer, `= <value>`, or the
kind it fails with, `fails rule`. A line indented deeper goes on with the one above. `check` runs
each in the reference interpreter as a host would call it and compares (E601, which names the
fields of a record that differ); a line that is no call, or an answer not of the function's type,
is E602.

## What `check` says

`check` reads the words, the names and the files a module reads, and the types, units and rounding
(E0xx, E1xx, E2xx). Then it finds what each value can be, from the ranges the parameters declare,
narrowed by the branches of `if` and `match` and by a call that answered, and keeps the relations a
condition says between two values. It holds every place a value is given against the range of what
takes it: a rule's input (a number to its range, and an ordered enum the rule holds to a range over
its values, `campaign : tier  range >=basic <=gold`, to the values the range lets in), a whole input
of a date, a parameter of a function, a result, a field, `isqrt`'s argument; and every operation
against int64 and every divisor against 0. Each place comes to one of three:

- **shown**: nothing is said, and the generated code does not check it;
- **an example**: an input of the function breaks it (an error, E301, E302, E303, with the input
  and what it gives);
- **undecided**: neither (a warning, W301, W302, W303); the generated code checks it when it runs,
  and the function's effect has its kind.

An example is said only for an input found: the analysis looks among the function's vectors, and
for a value that is a sum of its parameters, at the inputs that put it a step outside. The ends of
a range the analysis finds are not examples, because they may not happen.

The checks that cross into the other languages are ritsu's (below), and come back as the effects of
the functions. Then come the decisions written in code (W501), and the `examples`.

### Decisions written in code

A decision belongs in a table, where rulec checks it is complete and without overlap and the
person who approves it reads it on its page. W501 warns of two shapes in code: an `if` or a
`match` that chooses a constant of the business (an amount, a quantity, a rate, a date, an enum's
value, `true` or `false`, a record of them, and a number in a `match`) by the values it compares,
and a comparison with a constant of the business that is kept as a value (`let free = total >=
35USD`). It does not warn of an answer `none`, of a branch that narrows a value to a rule's range
before the rule is called (`if late >= 1`) while the other answers no constant, of a `match`
that maps an enum's values to another's of the same names, of the default of an optional, of a
comparison with 0, or of an `if` that answers a number. The warning has no mark to quiet it.

### A key written in a module

W901 warns of a key in a shape its provider fixes (AWS, GitHub, Slack, Stripe, OpenAI, Anthropic,
Google, a PEM private key) anywhere in a `.kn`, in a string or a comment, and in a module that
does not read too, as every language of ritsu looks at its own files. It says the kind of key, its
prefix and its length, and quotes neither the key nor its line; another diagnostic that quotes the
line shows the key masked. A value for tests is marked `ritsu: test secret` in a comment on the
same line.

### The line that says a module passes

```console
$ ritsu kaname check examples/late_payment/late_payment.kn
ok examples/late_payment/late_payment.kn: 1 function, total
```

It counts the functions by their effects, as their heads would write them: `3 functions: 2 total;
1 fails range`.

## What `gen` writes

kaname's code is written by `ritsu gen`, into the package it writes for each of TypeScript, Python
and Go, beside the code of the rules, the dates and the books the modules call (ritsu's DESIGN
9.3). There is no `kaname gen`: a module's code calls theirs.

| | TypeScript | Python | Go |
|---|---|---|---|
| a module | `glue/<module>.ts` | `<name>/glue/<module>.py` | `glue/<package>/<package>.go` |
| the helper module | `glue/kaname.ts` | `glue/kaname.py` | `glue/kaname/kaname.go` |
| the transfers of a book | `glue/books/<book>.ts` | `glue/books/<book>.py` | `glue/books/<package>/…` |
| the function that makes them | `glue/books/<book>_apply.ts` | `glue/books/<book>_apply.py` | `glue/books/<package>apply/…` |

The helper module has the exact fractions, the arithmetic checked against int64, the five
roundings, the dates, and `KanameError`; it uses nothing beyond each language's standard library.

| kaname | TypeScript | Python | Go |
|---|---|---|---|
| money, a quantity | the package's `units` type (`units.USDc`) | `units`' type | `units`' type |
| `number`, a rate | `bigint` (a rate as a count of its step) | `int` | `int64` |
| `date` | `"YYYY-MM-DD"` `string` | `datetime.date` | the helper's `Date` |
| a module's enum | an object of values and their union | `enum.Enum` | a named `string` with constants |
| a record | a read-only `interface` | `@dataclass(frozen=True)` | a struct |
| `list[T]`, `T?` | `readonly T[]`, `T \| null` | `list[T]`, `T \| None` | `[]T`, `*T` |

Inside a function, a value whose step is whole is the language's integer and any other an exact
fraction, rounded where the module rounds it.

**Names.** A function is its alias, in Pascal case in Go, and in Go one named like a type of the
module takes `Fn` (`fn claim` with `record Claim` is `ClaimFn`). A record and an enum are in Pascal
case. A parameter and a local are their aliases and names in TypeScript and Python, and in mixed
caps in Go (`paid_on` is `paidOn`). One spelled like a word the language's code reaches for (Go's
`len`, Python's `int`, TypeScript's `BigInt`), a name of the module's code or an earlier one is
moved aside: with `_` after it in TypeScript and Python (a parameter `len_`, a local `int_2`), with
a number after it in Go (`len2`). Python takes the parameters by keyword only, and the function a
host calls takes them by their aliases.

**Each function** is two: the one a host calls, which checks its arguments against what they
declare and fails with `input` outside it, and the one the module's own functions call. Its effect
shapes it: in Go a function that can fail returns `(T, error)` and a total one returns `T` (and
panics with `input`; `Check<Function>` says so first); in TypeScript and Python each has the union
of the kinds it can fail with, `ClaimFails`, in its type and its comment.

```ts
export function claim(invoice: Invoice, paid_on: string, reference_rate: bigint): Claim | null {
```

The code checks the places the analysis did not show, where kaname's reference interpreter checks
them, in the same order, and fails with `KanameError` of the same kind, line and message (in the
language of `--lang`). A call of a rule or a date it did not show is wrapped, and what the rule's
code throws becomes `rule` with the rule's error as its cause. The code is held to kaname's
reference interpreter on every vector of every function, in English and in Japanese, and passes
`tsc --strict --erasableSyntaxOnly`, `mypy --strict`, gofmt and `go vet`.

**A transfer** is a value of its operation's type (`CaptureDo`, with `book`, `transfer`, `op` and
`args`), with the union of the reasons the book can refuse it with at the amounts the modules give
(`CaptureDoRefusal`). The function of the book's `_apply` module makes one through chobo's client
and answers what the client answers (`done`, `done_before`, `refused` with its reason).

## Across the languages

Some checks need two languages at once, and are ritsu's (ritsu's DESIGN 7), as for the calls of a
workflow of dandori:

| Check | What it holds |
|---|---|
| X2 | a rule's preconditions, at the call: from the relations kaname keeps (`before <= upto` of `running_totals`), else the ranges, else the runs of the function's vectors |
| X3 (a) | a day of koyomi given to a rule's date input, against the rule's range |
| X4 | an amount given to a transfer, against what chobo takes; and the reasons the book can refuse each `do` and `hold` with |
| X6 | a day given to a date's input, against koyomi's range |

They are said by `ritsu check` in ritsu's codes (E201 to W205), and come back to kaname through
ritsu's port: a place they show leaves the effect, and one they cannot decide or find an example of
is checked when the code runs (`ritsu W203` in the note of an E401). `ritsu kaname`, `ritsu check`
and `ritsu gen` hand kaname those verdicts. The binary of kaname's own crate has none, and its
places wait: their kinds are in the effect, but not held against what is written.

## The page for people

`ritsu kaname doc <file.kn>` writes a page for the person who approves the decisions and the
engineer who reads how far each function is checked: the head with the module's digest and what it
reads; the decisions in tables, with the page rulec and koyomi draw of each rule and dates file it
calls and each transfer it builds with the reasons it can be refused with; the functions in three
parts, total, checked when they run (each place and why), and those that may not stop; the
decisions written in code; and the examples with what they answer. The Markdown is for a pull
request, the pages of the rules and the dates folded in it; the HTML is one file that reads nothing
from outside, the pages opening over it.

## The keywords

| Where | Words |
|---|---|
| at the start of a line | `module` `description` `use` `enum` `record` `fn` `examples` |
| after `use` | `rule` `dates` `book` `module` `from` |
| types | `number` `bool` `string` `date` `rate` `money` `mass` `length` `area` `volume` `duration` `temperature` `sound` `list` `running` `step` `incl_tax` `excl_tax` `range` `count` |
| at the end of a function's head | `total` `fails` `diverges`, and the kinds `rule` `date` `range` `overflow` `zero` |
| in a body | `description` `let` `if` `then` `else` `match` `some` `none` `true` `false` `and` `or` `not` `in` |

The symbols are `=` `!=` `<` `<=` `>` `>=` `+` `-` `*` `/` `×` `÷` `−` `≦` `≧` `->` `=>` `:` `,`
`.` `(` `)` `[` `]` `{` `}`, and `#` for a comment. The builtins (`sum`, `map`, …) are names a call
reads, not keywords.

## Commands

```console
$ ritsu kaname check examples/late_payment/late_payment.kn
$ ritsu kaname run examples/late_payment/late_payment.kn claim --input claim.json --trace
$ ritsu kaname vectors examples/late_payment/late_payment.kn --function claim
$ ritsu kaname doc examples/late_payment/late_payment.kn --root examples/late_payment --format html
$ ritsu kaname api examples/late_payment/late_payment.kn --root examples/late_payment
$ ritsu kaname explain E301
```

- `check <file.kn>... [--format json] [--root <dir>]`: everything above, and the line that says a
  module passes.
- `run <file.kn> <function> --input <file.json|-> [--trace] [--format json] [--root <dir>]`: one
  function in the reference interpreter on an input of JSON, each argument by its name or alias
  (`-` reads standard input). It prints the call, the rules and dates it calls with their answers
  and the transfers it builds, in order, and the answer, or the failure with its kind and line.
  `--trace` adds the value of every `let`. A failure is an answer: exit 0.
- `vectors <file.kn> [--function <name>] [--trace]`: inputs chosen for each function, with what the
  reference interpreter answers, a JSON object a line (below). The inputs are the ends of each range,
  0, the literals a parameter is compared with and a step either side, the ends of the ranges of the
  rules' and dates' inputs a parameter goes to and a step outside them, and the inputs that reach the
  boundary of a comparison of sums of parameters; up to 2,000 a function.
- `doc <file.kn> [--format markdown|html] [--root <dir>]`: the page for people.
- `api <file.kn> [--root <dir>]`: JSON for other tools (below).
- `explain <code> | --all [--format markdown|json]`: when a diagnostic is printed, how to fix it,
  and the smallest module that prints it.

`run` and `vectors` take a module that passes check; `doc` and `api` one whose names and types pass,
whatever else its check finds, since where it can fail is what they show. Else they print its
diagnostics and exit 1. A recursion of more than 200 calls stops the reference interpreter: `run`
says so and exits 1. Every command takes `--lang ja|en` (else `KANAME_LANG`, then `RITSU_LANG`,
else English) and `--help`.

### Exit codes

| Code | When |
|---|---|
| 0 | no errors (there may be warnings); run to its end; printed |
| 1 | at least one error; a function of `run` that did not stop |
| 2 | bad arguments, a file that cannot be read, an input that is not JSON, no such function, or a module that reads a rule, a dates file or a book run with kaname's own binary (E202) |

## JSON

`check --format json` prints one object a file, the functions with their effects and the places
not shown:

```json
{
  "kaname": "0.34.1",
  "file": "late_payment.kn",
  "ok": true,
  "summary": "1 function, total",
  "functions": [
    {
      "function": "claim",
      "effect": {
        "fails": [],
        "diverges": false
      },
      "written": {
        "total": true,
        "fails": [],
        "diverges": false
      },
      "places": [],
      "calls": []
    }
  ],
  "diagnostics": []
}
```

A place is `{"line", "kind", "check", "code", "outcome"}`: `check` is `kaname` or the check across
the languages that decides it (`X4`), `code` the diagnostic said of it, and `outcome` `example`,
`undecided`, or `waiting` where no verdict came.

`run --format json` prints `{"kaname", "file", "module", "function", "input", "calls", "result" |
"failure" | "diverged"}`, and `lets` after `calls` with `--trace`. `vectors` prints a line an input:

```json
{"function":"claim","in":{"invoice":{"id":"a","invoiced":"2026-01-01","amount":50000000},"paid_on":"2026-01-01","reference_rate":0},"out":null,"why":"paid_on: the lower end; reference_rate: the lower end"}
```

A value is as it travels: an amount in its unit, a rate as a count of its step, a date as
`YYYY-MM-DD`, an enum by its value's name, a record by its fields' names, `none` as `null`, a
transfer as `{"book", "transfer", "op", "args"}`. A failure is `"fails": {"kind", "module", "line",
"message"}` in place of `out`, and a recursion cut short `"diverged"`. `why` says why the input was
chosen.

`api` prints `{"kaname", "file", "module", "generated", "uses", "enums", "records", "functions",
"transfers"}`: the module with its SHA-256, the files of its code, the files it reads, its
declarations with their types, ranges and counts, each function with its effect inferred and
written, the calls it makes into the other languages with what each check across them came to,
the places kaname could not show, and the names the generated code gives it; and each operation of
a transfer with the reasons it can be refused with.

## Environment

| Variable | What it does |
|---|---|
| `KANAME_LANG` | `ja` or `en`, the language of the text when `--lang` is not given (not read inside `ritsu check`) |
| `RITSU_LANG` | the same, for every language of ritsu, after `KANAME_LANG` |
