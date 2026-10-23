# The kaname reference

kaname is a small general-purpose language for writing a Functional Core: the part of a program
that only computes and decides. A `.kn` module is functions over records, enums (whose values may
have fields), lists, dicts, sets and strings, with generics, `result` and function values: a state
moved by events, input checked and normalized, counts kept and compared, a tree reshaped. It has no
input or output: what it is given, it computes from, and the host that calls it reads, writes and
keeps state. It calls the rules of rulec, the dates of koyomi and the transfers of chobo's books in
ritsu's types too, and computes between them. `kaname check` holds every computation to the rules
the languages of ritsu are held to: units, currencies and tax, the steps of rates, ranges, rounding
and int64. It finds where a value can leave the range of where it goes, and says, function by
function, whether it answers every input (`total`), checks some places when it runs, or may not stop.
`ritsu gen` writes it as TypeScript, Python and Go, beside the code of the rules, dates and books it
calls.

This page is the whole language and the commands. [codes.md](codes.md) has every diagnostic, with
the smallest module that prints it. [DESIGN.md](../DESIGN.md) has the reasons.

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
| truth, text | `bool`, `string` | a string, a sequence of code points |
| enum | a module's `enum`, a rule's `charges.band` | the value's name |
| sum | a module's `enum` with a value that has fields | a value with no fields by its name (`"reopen"`), one with fields as `{"placed": {…}}` |
| a value's type | `Order.placed` | as a value of its sum |
| record | a module's `record`, another module's `pricing.Order` | an object of its fields |
| list | `list[T]` | an array |
| dict | `dict[K, V]` | an array of `[key, value]` pairs, in the order of the keys |
| set | `set[T]` | an array of its members, in the order of the keys |
| an entry of a dict, an element with its place | `entry[K, V]`, `indexed[T]` | `{"key", "value"}`, `{"index", "item"}` |
| optional | `T?` | the value, or `null` |
| result | `result[T, E]` | `{"ok": value}` or `{"err": value}` |
| a generic record or enum given types | `Page[Invoice]`, `Found[string]` | as the record or the sum, its type parameters given those types |
| a rule's outputs | `charges.outputs` | an object of its outputs |
| an element of a list a rule walks | `shipping.lines` | an object of its fields |
| a transfer | `ledger.capture.do` | `{"book", "transfer", "op", "args"}` |
| a running total | `running[T, W]` | `item`, `before`, `upto`, `total` |
| a function | `(T) -> U`, `(T, Adjustment) -> T total` | does not travel: a function value is given to a parameter of a function type, named by a `let`, and called |

### Generic functions, records and enums

A function, a record and an enum take type parameters in brackets after their names, and `K: key`
holds one to the types that can be keys:

```kn
fn first_or[T](xs: list[T], fallback: T) -> T
  match xs.first()
    none => fallback
    some x => x
```

```kn
fn largest[K: key](xs: list[K]) -> K?
  xs.sort().last()
```

A type is given its type arguments where it is written (`Page[Invoice]`; the wrong number of them
is E112), and a call works them out from its arguments, then from the place it is put in
(`empty_page[string]()` writes them where neither says; two types for one, or none at all, are
E112). A type whose values may be none (`number?`) is no type argument (E112): write `T?` where a
value may be none. In a generic function a value of a type parameter is passed on, held (in a
record, a list, a dict's values, an optional, a result) and compared with `=` and `!=`; ordering it,
sorting by it and making it a key need `K: key` (E114); arithmetic, `match` and fields on it are
E105, and a range on it is E108. A field of a type parameter held to keys counts as a key in a
record that is the key of an order (`xs.sort_by(p => p)` for a `list[Pair[K]]`, `indexed[K]`).

In a ring of calls (a function that calls itself, functions that call each other, a function given
by its name counting as a call), a generic function is given its caller's type parameters as they
are, in any order, or types that name none; a type argument made from a type parameter
(`wrap_deep(x: [x], …)` gives `list[T]`) is E112, since each time the calls went round it would grow
one deeper, which the generated Go cannot instantiate.

A generic function is an inner function: no host calls it, so it has no entry, no vectors, and
`run` does not run it. Where it is called, its body is read again with the values the call gives,
four calls deep at most and not in a ring of calls: the call answers what that reading says, cut to
what the result declares, and its effect is what that reading finds, not the function's own (a
generic function that may overflow on any number is total where it is called with 0 to 5).

### Results and `try`

`result[T, E]` is `ok` with a value of `T` or `err` with a value of `E`, made by `ok(x)` and
`err(e)` (their type is the place's) and split by `ok x =>` and `err e =>`:

```kn
fn parse_item(sku: string count <=12, qty: string count <=20) -> result[Item, string]
  let n = try parse_qty(text: qty)
  if sku.count() = 0
    err("no sku")
  else
    ok(Item { sku: sku, qty: n })
```

`let x = try e` takes the value of an `ok`, and makes an `err` the function's answer; of an
optional, it takes the value there and makes `none` the answer. It is written only in a `let` of a
block the function answers with, and the function answers a `result` of the same error type or an
optional (E115 otherwise). `try` is a keyword, and `ok` and `err` are builtins no function takes
the name of (E006). A result is a business answer: what the checks could not show stays a failure
of the effect, which nothing catches.

### Function values

A parameter of a function type takes a lambda or a function given by its name, and calls it:

```kn
fn total_by[T](xs: list[T], amount: (T) -> money[USDc]) -> money[USDc]
  xs.sum(x => amount(x))
```

```kn
fn order_total(lines: list[Line] count <=50) -> money[USDc]
  let line_total = (l: Line) => l.unit * l.qty
  total_by(xs: lines, amount: line_total)
```

A function type is `(T, U) -> R`, and may end with an effect (`(T) -> R total`,
`(T) -> R fails range`). A lambda takes the types of its parameters from the function type of the
place it is put in; where none says them (a `let` with no type), they are written,
`(l: Line) => …` (E112 otherwise). A function of the module goes by its name where a function type
goes (`total_by(xs: lines, amount: line_cost)`, `lines.map(line_cost)`); a builtin does not (E109:
wrap it, `x => isqrt(x)`), nor a function that takes functions (E113). A function value is called
with its arguments in order, `amount(x)` (E107 with names), and is used for nothing else: no
function answers one, no field, list, dict, set or optional holds one, `=` does not compare them,
and no function type is written inside another type (E113); a type parameter takes no function type
(E112). A lambda reads the names in scope where it is written, as they were when it was made.

A function that takes a function is an inner function, and is read again where it is called with
the values and the functions it is given, as a generic one is. The places of a lambda's body are the
places of the function that wrote it, wherever it is read, and a function given by its name is
called by the function that gives it, for its effect and for its rings. A call of a parameter of a
function type takes the effect written on the type into the function's own effect; with none
written it takes none, and the effect of the function given goes on to the function that gives it
(`"through"` in the JSON). A function given that does more than the effect written on its
parameter's type is E402.

### Numbers and units

Types and arithmetic are rulec's. A unit is part of the type: `JPY` and `円` are one unit, `kg` and
`g` two, tax included and tax excluded two types. Values of one type add, subtract and compare; a
mix of units or of tax is E102. A number or a rate multiplies an amount or a quantity, and the
product of a number and a rate is a rate. Multiplying two dimensions, or dividing a number by a
quantity, is E105; a temperature and a sound are compared and ranged only.

A literal is a whole number of the unit it is written in (`0.5USD` is E104; a share is a rate,
`40%`), and is read in the unit of what it meets: `15lb` compared with a `mass[oz]` is 240 ounces,
`40GBP` added to a `money[GBP, incl_tax]` is tax included, and `30℃` compared with a
`temperature[℉]` is 86℉ (and `-20℃` is −4℉). A literal that is no whole number of that unit is
E104. A bare `0` is the 0 of any unit. A minus right before a constant makes a constant below zero,
of any unit: `t <= -18℃`, `level < -10dB`; a value of a temperature or a sound takes no `-` (E105).

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

An enum may also have its values under it, a line each, and a value written so may have fields
under it, written as a record's. An enum with a value that has fields is a **sum**:

```kn
enum Event
  place
    on : date  range >=2026-01-01 <=2027-12-31
  pay
    cents : money[USDc]  range >=1USDc <=2_000_000USDc
  cancel
    reason : string
  reopen
```

A value with fields is built with them (`Event.pay { cents: 500USDc }`, or `pay { cents: 500USDc }`
where an `Event` goes), and one with none by its name (`Event.reopen`); written the other way, it
is E107 or E105. An expression that builds a value is of the sum, whose fields are read once a
`match` names the value (`.` on a value of a sum is E105). Each value has a type of its own,
`Event.pay`, that goes where the sum goes, and is written for a parameter, a field or a `let`; a
value of the sum does not go where a value's type goes (E102). A sum has no order, and may hold
itself (below). The code names the type of a value by the enum and the value (`EventPay`): a record
or another type of that name is E006, and a field cannot be named `kind`, by which TypeScript
tells the values apart (E007).

### Records, lists and optionals

```kn
record Invoice
  id       : string
  invoiced : date  range >=2026-01-01 <=2027-12-31
  amount   : money[GBPc]  range >=1GBPc <=100_000_000GBPc
```

A record is built with every field (`Invoice { id: …, invoiced: …, amount: … }`; a field of type
`T?` may be left out, and is `none`), and a field is read with `.`. A record may hold itself
(below). A record, or a value with fields, is made from another of its type with `..` right after
the `{`: the fields not written are taken from it, and each field is held to its range as when it
is built:

```kn
fn moved_to(p: Order.placed, on: date range >=2026-01-01 <=2027-12-31) -> Order
  Order.placed { ..p, on: on }
```

The value after `..` is of the type built (E102 otherwise).

A list is built by a literal (`[a, b]`) and by the builtins, and read at a place, from 0, with
`xs.at(i)`, which is `none` past its ends. An optional `T?` is split with `match` into `none` and
`some x`, and takes no arithmetic and no order. An optional of an optional is one optional.

### Types that hold themselves

A record or a sum may hold itself, through a list, a dict, a set, an optional, a `result`, or
another record or sum, and two types may hold each other:

```kn
record Category
  name     : string
  children : list[Category]

fn size(c: Category) -> number
  1 + c.children.sum(x => size(c: x))
```

Such a type needs a value that does not hold it: a field that holds it optional, a list that may be
empty, or a value of the sum that does not hold it. One with none (a record whose field `next` is
of the record itself) is E108, as is a generic type that gives a type of its cycle a type argument
made from its own (`Wrap[list[T]]`), which would make the types grow without end. A host may give a
value nested 200 deep, counted along each path, one for each value of such a type; a deeper one
fails with `input` before the function starts. A call of kaname does not check inside such a value:
its fields were held to what they declare where it was built or came in. A function that walks one
recurses, and `diverges`. The code writes the types as any other: Go holds them through `*T`,
slices, maps and the interface of a sum (`Children []Category`).

### Dicts and sets

A dict holds a value for each of its keys, and a set holds each of its members once:

```kn
fn diff(before: dict[string, number] count <=4, after: dict[string, number] count <=4) -> list[Change]
  let skus = before.keys().to_set().union(after.keys().to_set()).to_list()
  skus.filter_map(sku => change(sku: sku, before: before.get(sku), after: after.get(sku)))
```

A key of a dict and a member of a set are of a type that can be a key: a number, an amount, a
quantity, a rate with a step, a date, a string, a bool, or an enum whose values have no fields (a
rule's enum too). Any other type there is E114, and so is one as the key of an order (`sort_by`,
`max_by`, …), which may also be a record whose fields can all be keys, compared field by field.
kaname puts the keys of each type in one order: numbers, amounts, quantities and rates from the
least, dates from the earliest, strings by their code points, `false` before `true`, and the values
of an enum in the order they are declared (an enum written with `|` too, which still has no `<`).
A dict and a set go round in that order, whatever order they were built in: `keys`, `values`,
`entries`, `to_list` and the builtins that walk them answer in it, and so does the code in every
target. What a dict or a set holds is rounded, as a field's value is (E103).

A dict is written `["a": 1, "b": 2]`, and `[:]` is the empty one where the place says its type
(`let d = [:]` is E102). A key written twice as a constant is E107; of two keys that are values, the
later pair wins. A set is made from a list: `[a, b].to_set()`, `[].to_set()`. Two dicts are equal
when they hold the same pairs, two sets when they hold the same members. Every builtin answers a
new dict or set and leaves the one it is given as it was. `dict`, `set`, `entry` and `indexed` are
type words only before `[`, so a record may still be called `set`.

### Ranges and counts

A number or a date may carry a range, a list a count of its elements, and a string a count of its
code points:

```kn
fn split(lines: list[Line] count >=1 <=10, discount: money[USDc] range >=0USDc <=10_000_000USDc) -> list[Share] total
```

A range written is what the host promises: a function's code checks the arguments against it
before it computes, an argument outside it fails with `input`, which no effect counts, and an input
inside it that makes the function fail is an error with that input. A value whose range is not
written takes the whole of its type: all of int64, in its unit, for a number, an amount or a
quantity; every day from 0001-01-01 to 9999-12-31 for a date; 0 to 2⁶³ − 1 elements for a list, a
dict or a set, and code points for a string.
An end not written, of a range with one end, is the type's. The code checks an argument against the
whole of its type too, and an overflow that comes from the end of a type is no error: it is not
said, the code checks it where it happens, and the function's effect has `overflow`.

```kn
fn next_id(last: number) -> number
  last + 1
```

A range is written in the unit of its type (`range <=15lb` on a `mass[oz]` is 240 ounces); `>` and
`<` mean a step inside (`>0` on a whole number is `>=1`, `<15%` on a `rate[step 0.01%]` is
`<=14.99%`). A rate with a step may leave its range out, and is then 0% to 100%. A rate carries its
step wherever it comes in or goes out: a parameter, a result, a field (E108 without it); only a
`let` may leave it out. The `count` of a dict counts its pairs and of a set its members, and the
`range` of a dict is its values', of a list and a set their elements'. A range on a string, a truth
value, an enum or a record, a `count` on what is not a list, a dict, a set or a string, an empty
range and an end written twice are E108.

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

`private` before `fn`, `record` or `enum` keeps it to its module: another module that names it is
E101, no host calls a `private` function (it is an inner function), and a parameter, an answer or a
field of what is not `private` takes no `private` type (E108).

- `if <cond>` with the block under it, `else if`, and `else` (E111 without it); `if a then b else c`
  on one line.
- `match <value>` with an arm a line: `gold => 0USD`, `silver, basic => 3USD` for an enum, `true`
  and `false` for a truth value, `none` and `some x` for an optional. An arm may have a block under
  it. An arm takes a value of a sum and gives it a name (`placed p =>`, `p` of the type
  `Order.placed`); `else =>`, the last arm, takes every value the arms above leave; and a number, an
  amount, a quantity, a date or a string is split by literals, with `else`. A value left out, one an
  arm above already took, an `else` with nothing left, an arm after `else`, and a split by literals
  without `else` are E106; an arm is first that takes the value.

```kn
fn reset(o: Order) -> Order
  match o
    placed p => Order.draft { items: p.items }
    paid y => Order.draft { items: y.items }
    else => o
```

```kn
fn size(n: number range >=-5 <=1000) -> string
  match n
    -1 => "minus one"
    0 => "none"
    1 => "one"
    else => "more"
```

- Comparisons are one at a time, joined by `and`, `or` and `not` (`a < b < c` is E001). `!=` is there;
  `==`, `&&` and `||` are E001, which says what to write. `=` and `!=` compare two values of one type
  whole, of any type: a record field by field, a list element by element, a value of a sum by its
  value and its fields, an optional (`x = none`); two types are E102. `<`, `<=`, `>`, `>=`, `min`
  and `max` order numbers, amounts, quantities, rates, dates and an enum in an order only (E105).
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
calls, and of the lambdas and the functions it gives by their names; a function given where the
parameter's type writes an effect keeps within it (E402).

### Builtins

A builtin is called as a function, `sum(xs, x => x.qty)`, or written after the value it takes
first, with a `.`: `xs.sum(x => x.qty)`. The builtins of the first table are written either way;
those of dicts, sets, lists and strings below are written after a value only, and `numbers`,
`char`, `ok` and `err` as a function only. A builtin written the way it is not is E101, whose note shows the way. A module's function may
take the name of a builtin written after a value (`fn get`); one called as a function is E006.

| Function | Answers |
|---|---|
| `min(a, b)`, `max(a, b)`, `isqrt(x)`, `allocate(t, c, s)` | as rulec's |
| `mod(x, n)` | the remainder, from 0 to one less than the size of `n`, whatever the signs; `n` is a whole number, a constant or a value, and a value that may be 0 is a place of `zero` |
| `numbers(a, b)` | the numbers from `a` to `b`, both in, and none where `b` is less than `a`; how many there are is a place of `overflow` |
| `char(n)` | the string of the one code point `n`, a whole number written as a constant that is a Unicode scalar value (`char(10)` is a new line); anything else is E109 |
| `up`, `down`, `half_up`, `half_down`, `half_even(x, step)` | `x` rounded to the step |
| `count(xs)`, `count(xs, x => cond)` | how many elements, how many keep the condition; `count(d)`, `count(s)` and `count(t)` count the pairs of a dict, the members of a set and the code points of a string |
| `sum(xs, x => expr)` | the sum, 0 for no element |
| `any(xs, x => cond)`, `all(xs, x => cond)` | `false` and `true` for no element; they stop at the element that decides |
| `map(xs, x => expr)`, `filter(xs, x => cond)` | a list |
| `first(xs, x => cond)`, `first(xs)` | the first element that keeps the condition, or the first with no condition; `none` where there is none |
| `max_of(xs, x => expr)`, `min_of(xs, x => expr)` | the largest and the smallest, or `none` for no element |
| `running_totals(xs, x => weight)` | a `running[T, W]` for each element: `item`, `before` (the sum before it), `upto` (the sum to it), `total` |
| `reduce(xs, first, (acc, x) => expr)` | for what the others cannot write |
| `starts_with(s, p)`, `concat(s, …)`, `text(x)` | text: a prefix, a join, a value as kaname writes it; `concat` joins lists too |
| `ok(x)`, `err(e)` | a `result` of the type the place gives |

A dict `d`:

| Function | Answers |
|---|---|
| `d.get(k)` | its value, or `none` |
| `d.put(k, v)`, `d.remove(k)` | the dict with the pair put in (or its value put in place), and without the key (the same dict where it has none) |
| `d.has(k)` | whether it has the key |
| `d.keys()`, `d.values()`, `d.entries()` | lists, in the order of the keys; an entry is an `entry[K, V]` of `key` and `value` |
| `d.count()` | how many pairs (`count(d)` too) |
| `d.map_values(v => expr)`, `d.filter(e => cond)` | a dict of the same keys; of the entries that keep the condition |
| `d.merge(o)`, `d.merge_with(o, (a, b) => expr)` | the pairs of both: for a key of both, `o`'s value, or the function's answer |

A set `s`:

| Function | Answers |
|---|---|
| `s.has(x)`, `s.add(x)`, `s.remove(x)` | whether it has the member; the set with it; without it |
| `s.union(o)`, `s.intersect(o)`, `s.difference(o)` | the members of either; of both; of `s` and not of `o` |
| `s.to_list()`, `s.count()` | its members in the order of the keys; how many |

A list `xs`:

| Function | Answers |
|---|---|
| `xs.append(x)`, `xs.concat(ys, …)` | the list with `x` after it; with the lists after it |
| `xs.at(i)` | the element at `i`, from 0, or `none` past the ends |
| `xs.first()`, `xs.last()`, `xs.last(x => cond)` | the first, the last, the last that keeps the condition, or `none` |
| `xs.slice(from, to)`, `xs.take(n)`, `xs.drop(n)` | the elements from `from` up to `to`, the first `n`, all but the first `n`; a place past the ends is cut to them, and fails nothing |
| `xs.reverse()` | the list from the end |
| `xs.sort()`, `xs.sort_by(x => key)`, `xs.sort_by_desc(x => key)` | in the order of the keys, from the least or from the greatest; two of one key keep their order (`sort` takes elements that can be keys) |
| `xs.distinct()` | the first of each element (elements that can be keys) |
| `xs.contains(x)` | whether an element is equal to `x` |
| `xs.flatten()`, `xs.flat_map(x => list)` | the lists one after another |
| `xs.filter_map(x => optional)` | the values that are there |
| `xs.indexed()` | an `indexed[T]` of each, its `index` from 0 and its `item` |
| `xs.max_by(x => key)`, `xs.min_by(x => key)` | the greatest and the least by the key, the first of one key, or `none` |
| `xs.to_set()` | a set of the elements |
| `xs.to_dict(x => key, x => value)`, `xs.group_by(x => key)` | a dict: a later element of one key wins; the elements of each key, in the order of the list |
| `xs.join(sep)` | the strings of the list, with `sep` between them |

A string `s`:

| Function | Answers |
|---|---|
| `s.count()` | how many code points |
| `s.chars()`, `s.code_points()` | a string of each code point; the number of each, 0 to 1114111 |
| `xs.to_text()` | the string of a list of code points, or `none` where one is no Unicode scalar value |
| `s.slice(from, to)`, `s.take(n)`, `s.drop(n)` | the code points from `from` up to `to`, the first `n`, all but the first `n`; a place past the ends is cut to them |
| `s.index_of(t)` | where `t` first stands, in code points from 0: 0 for an empty `t`, `none` where it is not there |
| `s.contains(t)`, `s.starts_with(t)`, `s.ends_with(t)` | whether `t` is in it, at its start, at its end |
| `s.split(sep)` | the pieces between the `sep`s, cut from the left and none overlapping, the empty ones at the ends kept; one for each code point where `sep` is empty |
| `s.words()` | the runs of what is not white space |
| `s.replace(old, new)` | every `old`, from the left and none overlapping, made `new`; `s` where `old` is empty |
| `s.trim()`, `s.trim_start()`, `s.trim_end()` | without the white space at both ends, at the start, at the end |
| `s.lower_ascii()`, `s.upper_ascii()` | `A` to `Z` made `a` to `z`, and the other way; nothing else changed |
| `s.pad_start(width, fill)`, `s.pad_end(width, fill)` | `fill` repeated before or after `s` to make it `width` code points long, the last repetition cut; `s` where `fill` is empty or `s` is that long already |
| `s.parse_number()` | the number `s` writes: a `+` or a `-` at most, then the digits `0` to `9`, within int64; `none` for anything else, white space too |
| `s.parse_date()` | the date `s` writes as `YYYY-MM-DD`, a day of the years 0001 to 9999; `none` for anything else |

A function is given to a builtin where it is written (`l => l.weight × l.qty`), and nowhere else
(E109).

A string is a sequence of code points, the scalar values of Unicode. Its count, its cuts, where a
string stands in it, and the order of strings (`<`, `max`, `sort`, the keys of a dict) are all in
code points, the same in every target. Two strings are equal when their code points are, and
nothing is normalized: `e` followed by U+0301 is not the one code point `é`. White space is the 25
code points of Unicode's White_Space, and case is `A` to `Z` and `a` to `z` alone. A literal is
read as it is written, with no escapes; a code point that cannot be written in one is `char(n)`.
No builtin of a string fails: a cut past the ends is cut to them, and what cannot be read is `none`.

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

A place for int64 whose value can leave int64 only from the end of a type, a value whose range is
not written, comes to none of the three: nothing is said and no example is looked for, the
generated code checks it when it runs, and the function's effect has `overflow`.

An example is said only for an input found: the analysis looks among the function's vectors, and
for a value that is a sum of its parameters, at the inputs that put it a step outside. The ends of
a range the analysis finds are not examples, because they may not happen.

The checks that cross into the other languages are ritsu's (below), and come back as the effects of
the functions. Then come the decisions written in code (W501), and the `examples`.

### Decisions written in code

A decision belongs in a table, where rulec checks it is complete and without overlap and the
person who approves it reads it on its page. W501 warns of two shapes in code: an `if` that
chooses a constant of the business (an amount, a quantity, a rate, a date, an enum's value, `true`
or `false`, a record of them) by the values it compares, or a `match` that chooses an amount, a
quantity, a rate, a date or a record of them (a value with fields too),
and a comparison with a constant of the business that is kept as a value (`let free = total >=
35USD`). It does not warn of an answer `none`, of a branch that narrows a value to a rule's range
before the rule is called (`if late >= 1`) while the other answers no constant, of a `match`
that answers an enum's values, `true` or `false`, or numbers (kaname holds a `match` to every value
once, E106), of the default of an optional, of a comparison with 0, or of an `if` that answers a
number. The warning has no mark to quiet it.

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
| a sum | the union of its values' types | a `Union` of them | an interface its values' structs have |
| a value's type `Order.placed` | an `interface` `OrderPlaced` with `kind: "placed"` | `@dataclass(frozen=True)` `OrderPlaced` | a struct `OrderPlaced` |
| `list[T]`, `T?` | `readonly T[]`, `T \| null` | `list[T]`, `T \| None` | `[]T`, `*T` |
| `dict[K, V]`, `set[T]` | `ReadonlyMap<K, V>`, `ReadonlySet<T>` | `Mapping[K, V]`, `AbstractSet[T]` (made a `dict` and a `frozenset`) | `map[K]V`, `map[T]struct{}` |
| `entry[K, V]`, `indexed[T]` | the helper's `Entry<K, V>`, `Indexed<T>` | the helper's `Entry[K, V]`, `Indexed[T]` | the helper's `Entry[K, V]`, `Indexed[T]` |
| `result[T, E]` | the helper's `Result<T, E>`: `{ kind: "ok", value }` or `{ kind: "err", error }` | the helper's `Ok`, `Err` and `Result` | the helper's `Result[T, E]`, `Ok[T, E]`, `Err[T, E]` |
| `record Page[T]`, `fn first_or[T]` | `interface Page<T>`, `export function _first_or<T>` | `class Page(Generic[T])` with `T = TypeVar("T")`, `def _first_or(*, …)` | `type Page[T any] struct`, `func FirstOr[T any]` |
| a function type `(T) -> U` | `(x: T) => U` | `Callable[[T], U]` | `func(T) U` |

Inside a function, a value whose step is whole is the language's integer and any other an exact
fraction, rounded where the module rounds it. A `match` of a sum is a chain of `if` on `kind` in
TypeScript and on `isinstance` in Python, and a type switch in Go; a record made from another is
`{ ...p, on: … }`, `dataclasses.replace` and a copy. Two values compared whole go through the helper
module's `eq` in TypeScript (and in Python for a sum), and through a function the module writes
for their type in Go. A dict and a set hold their keys and values as they travel (a date as its
`YYYY-MM-DD` string, an enum by its value's name), and are walked in the order of their keys through
the helper module, which orders each type of key as kaname does: so a `Map`, a `dict` and a Go `map`
go round in one order, and a sort is each language's stable one. `mod` is the helper's, the
remainder that is never below 0. A string is the language's, and where the language counts in
something else (UTF-16 units in TypeScript, bytes in Go) the helper module counts, cuts and orders
it in code points; it also holds the table of white space and the ways a number and a date are read.
The function a host calls checks that each string it is given is well-formed Unicode (no lone
surrogate in TypeScript and Python, valid UTF-8 in Go) and within its `count`.

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

**A generic function** is one function, which the code of every module calls and no host: it is
exported, and checks nothing at an entry. A type parameter held to keys takes its order as a hidden
first argument (`kaname.Order<K>` in TypeScript, a key function in Python, `func(a, b K) int` in
Go), which the caller gives for the type it calls with; values of a type parameter are compared by
their contents through the helper module (`eq` in TypeScript, `==` in Python, `Equal` in Go).

**A function value** is an arrow function in TypeScript, a nested `def` in Python and a function
literal in Go, and a function given by its name is one that calls it. A function that takes
functions is one function, exported as a generic one is; in Go a failure of a function value is
carried as any other, to the function the host calls. **What is `private`** is not exported in
TypeScript, and its name starts with `_` in Python and in Go.

**A transfer** is a value of its operation's type (`CaptureDo`, with `book`, `transfer`, `op` and
`args`), with the union of the reasons the book can refuse it with at the amounts the modules give
(`CaptureDoRefusal`). The function of the book's `_apply` module makes one through chobo's client
and answers what the client answers (`done`, `done_before`, `refused` with its reason).

**A host** calls a module's functions as it calls any of the package: it imports `glue/<module>`
(`generated.glue.<module>` in Python, the module path given to `ritsu gen --module` and
`glue/<package>` in Go), builds the arguments in the types above, and reads the answer, splitting a
sum on `kind`, on `isinstance` or with a type switch, and a `result` on `Ok` and `Err`. A Python
`frozenset` and a Go `map` have no order, so a host that prints a set or a dict sorts its keys
itself. The examples `order_flow`, `stock_diff`, `handles` and `catalog` each have a host in the
three languages under `host/`, which print the same.

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
| at the start of a line | `module` `description` `use` `enum` `record` `fn` `examples`, and `private` before `enum`, `record` and `fn` |
| after `use` | `rule` `dates` `book` `module` `from` |
| types | `number` `bool` `string` `date` `rate` `money` `mass` `length` `area` `volume` `duration` `temperature` `sound` `list` `running` `dict` `set` `entry` `indexed` `result` `step` `incl_tax` `excl_tax` `range` `count` |
| after a type parameter | `key` |
| at the end of a function's head, and of a function type | `total` `fails` `diverges`, and the kinds `rule` `date` `range` `overflow` `zero` |
| in a body | `description` `let` `try` `if` `then` `else` `match` `some` `none` `true` `false` `and` `or` `not` `in` |

The symbols are `=` `!=` `<` `<=` `>` `>=` `+` `-` `*` `/` `×` `÷` `−` `≦` `≧` `->` `=>` `:` `,`
`.` `..` `(` `)` `[` `]` `{` `}`, and `#` for a comment. The builtins (`sum`, `map`, …) are names a
call reads, not keywords.

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
  `--trace` adds the value of every `let`. A failure is an answer: exit 0. The input is read as deep
  as the arguments can nest, a value of a type that holds itself one deeper than the 200 a host may
  give (and so failing `input`), at least 256 levels of JSON and at most 1,024.
- `vectors <file.kn> [--function <name>] [--trace]`: inputs chosen for each function, with what the
  reference interpreter answers, a JSON object a line (below). The inputs are the ends of each range,
  0, the literals a parameter is compared with and a step either side, the ends of the ranges of the
  rules' and dates' inputs a parameter goes to and a step outside them, and the inputs that reach the
  boundary of a comparison of sums of parameters; where a range is not written, the ends of the
  type, and 1 and −1 for a number; each value of a sum at its base fields, and each field of it
  moved in turn; a type that holds itself at a value that holds no other, and at one that holds
  one; up to 2,000 a function.
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
  "kaname": "0.36.0",
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
`undecided`, `unbounded` (an overflow from the end of a type, with no code), or `waiting` where no
verdict came. An inner function (a generic one, one that takes functions, a `private` one), which no
host calls, has `"inner": true`; the effect of one that takes functions has `"through"`, its
parameters of a function type with no effect written, whose function's effect goes on to its
caller.

`run --format json` prints `{"kaname", "file", "module", "function", "input", "calls", "result" |
"failure" | "diverged"}`, and `lets` after `calls` with `--trace`. `vectors` prints a line an input:

```json
{"function":"claim","in":{"invoice":{"id":"a","invoiced":"2026-01-01","amount":50000000},"paid_on":"2026-01-01","reference_rate":0},"out":null,"why":"paid_on: the lower end; reference_rate: the lower end"}
```

A value is as it travels: an amount in its unit, a rate as a count of its step, a date as
`YYYY-MM-DD`, an enum by its value's name, a value with fields as an object of one pair, its name and
its fields (`{"placed": {"items": […], "on": "2026-01-01"}}`), a result as `{"ok": …}` or
`{"err": …}`, a record by its fields' names, a
dict as its `[key, value]` pairs and a set as its members (in the order of the keys; given in any
order, and a key given twice fails with `input`), `none` as `null`, a
transfer as `{"book", "transfer", "op", "args"}`. A failure is `"fails": {"kind", "module", "line",
"message"}` in place of `out`, and a recursion cut short `"diverged"`. `why` says why the input was
chosen.

`api` prints `{"kaname", "file", "module", "generated", "uses", "enums", "records", "functions",
"transfers"}`: the module with its SHA-256, the files of its code, the files it reads, its
declarations with their types, ranges and counts (and, for a sum, each value's fields and the name
the code gives its type), each function with its effect inferred and
written, the calls it makes into the other languages with what each check across them came to,
the places kaname could not show, and the names the generated code gives it; and each operation of
a transfer with the reasons it can be refused with. A generic declaration has its `type_params`
(each a name, an alias and whether it is held to keys), an inner function `"inner": true`, and what
is `private` `"private": true`.

## Environment

| Variable | What it does |
|---|---|
| `KANAME_LANG` | `ja` or `en`, the language of the text when `--lang` is not given (not read inside `ritsu check`) |
| `RITSU_LANG` | the same, for every language of ritsu, after `KANAME_LANG` |
