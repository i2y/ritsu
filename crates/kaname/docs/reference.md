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
`ritsu gen` writes it as TypeScript, Python, Go and Rust, beside the code of the rules, dates and books it
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
`Date`, `Rat`, `glue`), is E007. A keyword of Rust is not: the generated Rust writes it as a raw
identifier (`r#type`), or with `_` after it where no raw identifier writes it (`self_`). The one
thing the generated Rust cannot take is a module named or aliased `crate`, `self`, `Self`, `mod` or
`door`, which would name the module's file: no Rust module can take the first three, Rust reads
`mod.rs` as a directory's own file, and `door.rs` is the package's door. The other targets take
them, so `check` warns (W001), and only `ritsu gen --target rust` stops, with E008. One name
or alias given twice in one space is E006: the names of a module's `use` lines and its
declarations, the fields of a record, the parameters of a function, the values of an enum, and the
names inside a function, which also cannot be one a `use` line gives. A function cannot take the
name of a builtin.

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
directory above the first file given that holds `.git` or `ritsu.mod`, else that file's directory.

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
weight that is 0 or more, which a rule's preconditions ask. It knows too that each `total` is the
`sum` of the same list by the same field, whatever the lambdas' parameters are named: in the `else`
of `if amount > sum(lines, l => l.left)`, a call in `map(running_totals(lines, x => x.left), r => …)`
that gives `amount` and `r.total` to a rule keeps its precondition `amount <= total`. The list is a
name, the weight a field of the element (or the element itself), and the totals are the list a
builtin walks where they are made, as `map` does there, not one a `let` names. The weights go into
the fields of the records it answers, and so are rounded (E103): a whole number of their unit, or a
rate on the step its type says (`x => 1 / x` is E103; `x => half_up(1 / x, 1)` is not).

## Calling rules, dates and books

- **A rule**: `charges(debt: …, days_late: …, reference_rate: …)` answers the record
  `charges.outputs`, one field an output, even for a rule with one output (`.fee`). Each input is
  given by its name or its alias; an optional input may be left out. A type or unit that does not
  fit is E102 (ritsu's X1, held by kaname), and a value outside the input's range is kaname's to
  find (E301, W301). A list a rule walks is given as a list of its elements,
  `shipping(lines: map(xs, x => shipping.lines { amount: x.amount }), …)`. An output is read in
  the range rulec holds it to: the one written on its line, what the rows of its tables write, or
  what its `result` reaches. An output with none, one that reads an input with no range, is a value
  of the whole of its type, as the generated code returns it.
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

## Properties

A property is a claim about every input of a function, where `examples` give the answer for chosen ones. It is written like a function: a name, parameters with their types and ranges, and a body of a `description`, `when` lines, `let` lines, and a last line that must be `true`.

```kn
property no_share_is_more_than_its_left(lines: list[Line] count >=1 <=10, refund: money[USDc] range >=0USDc <=10_000_000USDc)
  description "A refund no larger than what is left in all gives no line more than what is left on it"
  when refund <= sum(lines, l => l.left)
  all(split(lines: lines, refund: refund), s => s.share <= s.left)
```

- **Meaning.** For every input in the ranges the parameters declare, and on which every `when` line is `true`, the last line is `true`. `property` begins a declaration and `when` begins a line of a property's body; neither is a reserved word anywhere else. A property is declared among the functions, after them or between them, and cannot be `private` (E003).
- **Names.** A property shares its namespace with the functions: a name used twice is E006, and a Japanese name needs an ASCII alias (E005), which is its name in Lean. A property is not a value: calling one is E101, with a note that says so. It appears in no generated code, in `run`, or in the list of functions of `api`.
- **Total.** A property is read as a `total` function that answers `bool`, so a body that can fail (a division by a value that may be 0, a call of a function that can fail) is E401: a failure could be read as "false" or as "does not apply", and the two make two properties. A `when` line narrows the values of the body as the condition of an `if` does. A parameter's type cannot be a function (E605), since the vectors cannot make one.
- **`examples` stay.** They keep an answer a person can read; a property does not replace them.

### What `check` says of a property

| Answer | When | Diagnostic |
|---|---|---|
| counterexample | on one of the vectors, every `when` line holds and the last line is `false` | E603 (an error: the input, and the `let` values) |
| shown | `check --lean` takes a proof of it | none (`shown` in JSON) |
| not decided | no counterexample, and not shown | W603, or W604 |

The vectors are `kaname vectors`' (every combination of the candidates up to 256, else each moved one at a time and in pairs; at most 2,000 for a property). Rules and dates are called for real, as `examples` call them. A counterexample is the first input found, in the order of the vectors.

- **W603** says why a property is not shown: there is no proof of it; a proof is beside the module, and `--lean` did not run; or it calls a rule or a dates file, whose answers a proof knows only as far as the port says.
- **W604** says what Lean cannot yet be written with, and where: a dict or a set, a builtin of strings other than `=` and `!=`, `sort_by`, `distinct`, `max_by`, `min_by`, `to_dict`, `group_by`, a type parameter held to keys (`K: key`). The property is still tried on its vectors.
- **W605** says that the `when` lines hold on none of the vectors. A property whose `when` lines hold for no input can be proved whatever it says.
- **E604** says that `--lean` did not take the proof, and why (below).

`check` exits 1 for E603 and E604, as for any error, and 0 for warnings. `--lean` exits 2 when `lake` is neither on the `PATH` nor in `~/.elan/bin`.

### Lean

`ritsu gen --target lean` writes `<out>/lean/` (`generated/lean/` unless `--out` is given), like the other targets; `ritsu.mod` can list it in `target`, and `--check` does not read `.lake/` and `lake-manifest.json`, which `lake` writes:

```text
generated/lean/
  lean-toolchain             leanprover/lean4:v4.34.0
  lakefile.toml              the library Kaname, and a library for each proof file beside a .kn
  .gitignore                 /.lake
  Kaname/Prelude.lean        roundings, allocate, isqrt, running_totals, fractions, lemmas for proofs
  Kaname/Rules.lean          the types of the rules the modules call (when there are any)
  Kaname/Books.lean          the structures of the transfers the modules build (when there are any)
  Kaname/refund_split.lean   the module's definitions, and the statement of each property
  samples/refund_split.lean  the theorems' names with `sorry` for their proofs (not built)
```

- **Names.** The module is `Kaname.<its alias>` (a name with no alias is its own alias; the Japanese version of the example is `Kaname.refund_split_ja`); functions, types, fields and properties are named by their aliases.
- **Types.** An amount, a quantity or a number is an `Int` counted in the type's unit, a rate an `Int` count of its step, a date the number of a day; `list[T]` is `List T` and its `count` a hypothesis on its length; a record is a `structure` whose fields' ranges are a predicate `Within`; an enum is an `inductive`; `T?` is `Option T`, a result is `Except`. A value divided by a value is a fraction `Kaname.Frac`, never a `Rat`, so that the form of a term does not change under proof.
- **Functions.** Every function that stops is written, including the ones no property calls. A function that can fail returns `Except`. One that may not stop, and one that uses what is not written for Lean, is left out, with the reason in a comment. A rule or a dates file a module calls is given from outside as a structure `Inner`, whose `Ok` holds the facts the port tells.
- **Statements.** `abbrev <property> : Prop := ∀ <parameters>, <ranges> → <when lines> → <body>`. An `abbrev`, so that `intro` and `decide` work without unfolding it.

The statement of a property, from `Kaname/refund_split.lean`:

```lean
abbrev shares_add_up : Prop :=
  ∀ (lines : List Line) (refund : Int),
    1 ≤ lines.length → lines.length ≤ 10 → (∀ e₁ ∈ lines, e₁.Within) →
    0 ≤ refund → refund ≤ 10000000 →
    ((List.map (fun (s : Share) => s.share) (split lines refund)).sum) = refund
```

### The proof file

The proofs of a module are in `<its alias>.lean` beside the `.kn` (`refund_split_ja.lean` for the Japanese version). kaname never writes it. For each property, it holds a theorem `Proofs.<module>.<property>` whose type is the statement itself, `Kaname.<module>.<property>`; a lemma may be in the same file. To start, copy `samples/<alias>.lean` beside the `.kn` without its first three lines, and put a proof in place of each `sorry`. To try as you write, run `lake build` in `generated/lean/`: its `lakefile.toml` reads the proof file where it is. Run `ritsu gen --target lean` again when a proof file is added or a property changed. The Lean kaname writes builds with no warning, so what `lake build` warns of is in the proof file; a warning does not stop `--lean`, and an error does. `--lean` needs no `lake build` before it: it writes the package again from the `.kn` (2 below).

`check --lean` does the following, and stops at the first that fails:

1. tries the properties on the vectors, as without `--lean` (those with a counterexample or W604 do not go to Lean);
2. writes the package again from the `.kn` into a temporary directory, so that nothing in `generated/lean/` counts;
3. reads each proof file as text, with its comments and strings taken out, and refuses what it must not hold (below); copies what it read into the package, so that what was read is what is built;
4. runs `lake build`, for at most 10 minutes;
5. runs `lake env leanchecker` over the proofs, which passes each declaration through the kernel again;
6. loads the module and the proofs and checks, for each property, that the theorem is there, that its statement is the generated constant, that its type is that constant itself, and that it rests on no axiom but `propext`, `Classical.choice` and `Quot.sound`.

A proof file cannot hold:

- an `import` of anything outside `Kaname.`;
- a `#` command other than `#check`, `#print`, `#reduce` and `#guard_msgs` (`#eval` and the others run code);
- `run_cmd`, `run_elab`, `run_meta`, `initialize`, `builtin_initialize`, `syntax`, `macro`, `macro_rules`, `elab`, `elab_rules`, `declare_syntax_cat`;
- `native_decide`, `decide +native`, and the `Lean.reduceBool`, `Lean.ofReduceBool`, `Lean.reduceNat`, `Lean.ofReduceNat` and `Lean.trustCompiler` that run compiled code in the build;
- an attribute other than `simp`, `grind`, `reducible`, `irreducible` and `instance` (with `local` or `scoped`);
- a `set_option` other than `maxHeartbeats`, `maxRecDepth`, `synthInstance.*`, `linter.*`, `pp.*` and `trace.*`.

The reason is that a proof written by someone else, built in CI, would run what it contains. E604 gives one of these reasons: the build fails (with Lean's error, at the line of the proof file), the theorem is missing or in another module, it states something else, it rests on another axiom (named; `sorry` is `sorryAx`), the kernel does not accept it, it holds what a proof file may not (what, and the line), or the build takes more than 10 minutes.

With every proof of `refund_split` in place:

```console
$ ritsu check --lean examples/refund_split
ok examples/refund_split/refund_split.ja.kn: 1 function, total
ok examples/refund_split/refund_split.kn: 1 function, total
ritsu check: 2 files (kaname 2): all pass; borders between the languages: 0 checked, 0 undecided
```

With the proof of `no_share_is_negative` left as `sorry`:

```text
error[kaname E604]: examples/refund_split/refund_split.lean:26:1: The proof of `no_share_is_negative` rests on `sorryAx`, besides `propext`, `Classical.choice` and `Quot.sound`
    26 | theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by
  = Write the proof in place of `sorry`.
  = `--lean` builds the proofs against the Lean written again from refund_split.kn, and takes for each property a theorem `Proofs.refund_split.<property>` whose type is `Kaname.refund_split.<property>` and which rests on no axiom but `propext`, `Classical.choice` and `Quot.sound`.
```

The first `--lean` downloads the Lean toolchain (`leanprover/lean4:v4.34.0`, about 2.7 GB with its library) through [elan](https://github.com/leanprover/elan), once.

### What a proof is made of

No Mathlib is used. A proof is made of Lean's core (`intro`, `unfold`, `rw`, `simp`, `omega`, `decide`, `grind`, `cases`, `induction`) and the lemmas of `Kaname/Prelude.lean`, which rest on the three axioms `--lean` takes. They are in `namespace Kaname`, which the draft opens with the module (`open Kaname Kaname.<alias>`); a name the module shares with the prelude (`Frac`, `Running`) is then "Ambiguous term", and is written in full (`Kaname.<alias>.Frac`). kaname writes `sum(xs, x => …)` as `(List.map (fun x => …) xs).sum`, the form of the first three.

| Lemma | Says |
|---|---|
| `sum_nonneg (hw : ∀ x ∈ xs, 0 ≤ w x)` | `0 ≤ (xs.map w).sum` |
| `sum_pos (hn : 0 < xs.length) (hw : ∀ x ∈ xs, 0 < w x)` | `0 < (xs.map w).sum`; the `1 ≤ xs.length` of a `count >=1` is `hn` as it is |
| `sum_mul_left c xs w` | `(xs.map fun x => c * w x).sum = c * (xs.map w).sum` |
| `allocate_self t (h : 0 < s)` | `allocate t s s = t` |
| `allocate_zero t s` | `allocate t 0 s = 0` |
| `allocate_mono (ht : 0 ≤ t) (hs : 0 < s) (hc : c ≤ c')` | `allocate t c s ≤ allocate t c' s` |
| `allocate_sub_le (ht : 0 ≤ t) (hts : t ≤ s) (hs : 0 < s) (hc : c ≤ c')` | `allocate t c' s - allocate t c s ≤ c' - c` |
| `runningTotals_length xs w` | `(runningTotals xs w).length = xs.length` |
| `runningTotals_items xs w` | `(runningTotals xs w).map (·.item) = xs` |
| `runningTotals_total xs w` | `∀ r ∈ runningTotals xs w, r.total = (xs.map w).sum` |
| `runningTotals_telescope xs w f` | `((runningTotals xs w).map fun r => f r.upto - f r.before).sum = f (xs.map w).sum - f 0` |
| `mem_runningTotals (hw : ∀ x ∈ xs, 0 ≤ w x)` | `∀ r ∈ runningTotals xs w, r.upto - r.before = w r.item ∧ 0 ≤ r.before ∧ r.upto ≤ r.total` |

A hypothesis is written as the goal reads, with the function applied: for `runningTotals lines (fun l => l.left)`, `mem_runningTotals` takes `hw : ∀ l ∈ lines, 0 ≤ l.left`, finds `w` from it, and answers `r.upto - r.before = r.item.left`. Written with `w` put in by hand, `0 ≤ (fun l => l.left) l`, the hypothesis holds a value `omega` does not take for `l.left`.

The tactics of Mathlib (`set`, `ring`, `linarith`, `nlinarith`, `norm_num`, `positivity`, `push_neg`, `use`) are "unknown tactic". What stands in for them, each tried on 4.34.0:

| Instead of | Write |
|---|---|
| `set S := e with hS` | `obtain ⟨S, hS⟩ : ∃ S, S = e := ⟨_, rfl⟩` and `rw [← hS] at h ⊢`, or `generalize hS : e = S at h ⊢` |
| `linarith` | `omega`, for linear facts over `Int` and `Nat`; it takes a sum or a product of two values as one value, and `¬ a ≤ b` as it is |
| `a * b` where a fact has `b * a` | `rw [Int.mul_comm]`, or `simp only [Int.mul_comm] at h ⊢` before `omega`: to `omega` they are two values |
| `ring` | `grind`, or `simp only [Int.mul_add, Int.mul_comm]` and then `omega` |
| `positivity`, `nlinarith` | `Int.mul_nonneg h1 h2` (`0 ≤ a * b`), `Int.mul_le_mul_of_nonneg_left h2 h1` (`a * b ≤ a * c`) |
| `norm_num` | `decide`, or `omega` |
| `push_neg at h` | `simp only [Classical.not_not] at h` for `¬¬p`, `simp only [Classical.not_forall, Int.not_le] at h` for `¬ ∀ x ∈ xs, 0 ≤ x` |
| `use 3` | `⟨3, rfl⟩`, or `exists 3` |

`grind` proves linear facts, an identity of polynomials (`(a + b) * (a + b) = a * a + 2 * a * b + b * b`), `b * a < 4` from `a * b ≤ 3`, membership in `xs.map f`, `0 < (x :: xs).sum` from `1 ≤ x` and `0 ≤ xs.sum`, and a definition by cases named to it (`grind [step]`). It does no induction, and does not get the sign of a product or a quotient from the signs of its parts: `0 ≤ xs.sum` from its terms is `sum_nonneg`, `0 ≤ a * b` is `Int.mul_nonneg`, and `0 ≤ allocate t c s` is `allocate_mono` with `rw [allocate_zero]`. What it proves rests on `propext`, `Classical.choice` and `Quot.sound`.

A function that maps `running_totals`, as `split` does, is opened with `unfold`. `List.map_map` joins its map to the map of the property, `List.map_congr_left` leaves one element `r` with `r ∈ runningTotals …`, and `runningTotals_total` puts the whole weight for `r.total`: the map is then the one `runningTotals_telescope` speaks of. The proof of `shares_add_up` in `examples/refund_split/refund_split.lean` goes this way.

A property of every element of what such a function answers (`∀ s ∈ split lines refund, …`) is shown of one element. After `unfold split at hs`, `obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hs` gives the element `r` of `runningTotals` that `s` was made from and puts the record made of it for `s`; `simp only` then puts the record's field in the goal. What is known of `r` comes from its item: `List.mem_map_of_mem` with `(·.item)` and `runningTotals_items` give `r.item ∈ lines`, and the `Within` of the item gives the ranges of its fields, joined with `∧` in their order (`simp only [Line.Within] at h` hands them all to `omega`). `mem_runningTotals` says how wide `r` is, not that the width is above zero. The proofs of `no_share_is_negative` and `no_share_is_more_than_its_left` go this way.

`sum(lines, l => price * l.qty)` is `(List.map (fun l => price * l.qty) lines).sum`, and `sum_mul_left` takes `price` out of it: the proof of `bill_is_the_price_times_the_quantities` in `examples/order_bill/order_bill.lean` is `exact sum_mul_left price lines _` after `intro` and `unfold bill`. With the number on the right of each term (`l.qty * price`), `simp only [Int.mul_comm _ price]` turns the terms round first.

A property of a value of an enum, with fields or without, is shown value by value: `cases` makes a goal for each value, and `simp_all` with the functions the goal calls, named, closes it. The proof of `a_closed_order_takes_no_event` in `examples/order_flow/order_flow.lean` is `cases order <;> simp_all [«open», next]` after its `intro`. A name that is a word of Lean, such as `open`, is written in `«»`, as the generated module writes it.

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
warning[W603]: examples/late_payment/late_payment.kn:28:1: The property `nothing_is_claimed_when_paid_on_time` held on all 10 of its 19 vectors on which its `when` lines are true, but it is not shown: the proof in late_payment.lean is checked with `--lean`
    28 | property nothing_is_claimed_when_paid_on_time(invoice: Invoice, paid_on: date range >=2026-01-01 <=2029-12-31, reference_rate: rate[step 0.01%] range >=0% <=15%)
ok examples/late_payment/late_payment.kn: 1 function, total
```

It counts the functions by their effects, as their heads would write them: `3 functions: 2 total;
1 fails range`.

## What `gen` writes

kaname's code is written by `ritsu gen`, into the package it writes for each of TypeScript, Python,
Go and Rust, beside the code of the rules, the dates and the books the modules call (ritsu's DESIGN
9.3). There is no `kaname gen`: a module's code calls theirs. `--target lean` writes no code a host
calls: it writes the definitions and the statements of the properties for Lean (Properties, above).

| | TypeScript | Python | Go | Rust |
|---|---|---|---|---|
| a module | `glue/<module>.ts` | `<name>/glue/<module>.py` | `glue/<package>/<package>.go` | `glue/<module>.rs` |
| the helper module | `glue/kaname.ts` | `glue/kaname.py` | `glue/kaname/kaname.go` | `glue/kaname.rs` |
| the transfers of a book | `glue/books/<book>.ts` | `glue/books/<book>.py` | `glue/books/<package>/…` | `glue/books/<book>.rs` |
| the function that makes them | `glue/books/<book>_apply.ts` | `glue/books/<book>_apply.py` | `glue/books/<package>apply/…` | `glue/books/<book>_apply.rs` |

The helper module has the exact fractions, the arithmetic checked against int64, the five
roundings, the dates, and `KanameError`; it uses nothing beyond each language's standard library.

| kaname | TypeScript | Python | Go | Rust |
|---|---|---|---|---|
| money, a quantity | the package's `units` type (`units.USDc`) | `units`' type | `units`' type | `units`' type (`units::USDc`) |
| `number`, a rate | `bigint` (a rate as a count of its step) | `int` | `int64` | `i64` |
| `date` | `"YYYY-MM-DD"` `string` | `datetime.date` | the helper's `Date` | the helper's `Date` |
| a module's enum | an object of values and their union | `enum.Enum` | a named `string` with constants | an `enum` |
| a record | a read-only `interface` | `@dataclass(frozen=True)` | a struct | a struct |
| a sum | the union of its values' types | a `Union` of them | an interface its values' structs have | an `enum` with a struct in each variant (`Order::Placed(OrderPlaced { … })`) |
| a value's type `Order.placed` | an `interface` `OrderPlaced` with `kind: "placed"` | `@dataclass(frozen=True)` `OrderPlaced` | a struct `OrderPlaced` | a struct `OrderPlaced` |
| `list[T]`, `T?` | `readonly T[]`, `T \| null` | `list[T]`, `T \| None` | `[]T`, `*T` | `Vec<T>`, `Option<T>` |
| `dict[K, V]`, `set[T]` | `ReadonlyMap<K, V>`, `ReadonlySet<T>` | `Mapping[K, V]`, `AbstractSet[T]` (made a `dict` and a `frozenset`) | `map[K]V`, `map[T]struct{}` | `BTreeMap<K, V>`, `BTreeSet<T>` |
| `entry[K, V]`, `indexed[T]` | the helper's `Entry<K, V>`, `Indexed<T>` | the helper's `Entry[K, V]`, `Indexed[T]` | the helper's `Entry[K, V]`, `Indexed[T]` | the helper's `Entry<K, V>`, `Indexed<T>` |
| `result[T, E]` | the helper's `Result<T, E>`: `{ kind: "ok", value }` or `{ kind: "err", error }` | the helper's `Ok`, `Err` and `Result` | the helper's `Result[T, E]`, `Ok[T, E]`, `Err[T, E]` | the standard `Result<T, E>` |
| `record Page[T]`, `fn first_or[T]` | `interface Page<T>`, `export function _first_or<T>` | `class Page(Generic[T])` with `T = TypeVar("T")`, `def _first_or(*, …)` | `type Page[T any] struct`, `func FirstOr[T any]` | `struct Page<T: Clone + PartialEq>`, `pub fn _first_or<T: Clone + PartialEq>` |
| a function type `(T) -> U` | `(x: T) => U` | `Callable[[T], U]` | `func(T) U` | `&dyn Fn(&T) -> Result<U, kaname::KanameError>` (`-> U` when the type says `total`) |

Rust takes a value that is `Copy` by value and borrows the rest: a parameter of a record is
`&Invoice`, of a string `&str`, of a list `&[T]`, of an optional string `Option<&str>`.

Inside a function, a value whose step is whole is the language's integer and any other an exact
fraction, rounded where the module rounds it. A `match` of a sum is a chain of `if` on `kind` in
TypeScript and on `isinstance` in Python, a type switch in Go and a `match` in Rust; a record made
from another is `{ ...p, on: … }`, `dataclasses.replace`, a copy and `OrderPlaced { on, ..p.clone() }`.
Two values compared whole go through the helper module's `eq` in TypeScript (and in Python for a
sum), through a function the module writes for their type in Go, and through the `PartialEq` they
derive in Rust. A dict and a set hold their keys and values as they travel (a date as its
`YYYY-MM-DD` string, an enum by its value's name), and are walked in the order of their keys through
the helper module, which orders each type of key as kaname does: so a `Map`, a `dict` and a Go `map`
go round in one order, and a sort is each language's stable one. In Rust a key keeps its type, since
the `Ord` of each type a key can be is kaname's order, and a `BTreeMap` goes round in it. `mod` is
the helper's, the remainder that is never below 0. A string is the language's, and where the
language counts in something else (UTF-16 units in TypeScript, bytes in Go and Rust) the helper
module counts, cuts and orders it in code points; it also holds the table of white space and the
ways a number and a date are read. The function a host calls checks that each string it is given is
well-formed Unicode (no lone surrogate in TypeScript and Python, valid UTF-8 in Go; a Rust string
always is) and within its `count`.

**Names.** A function is its alias, in Pascal case in Go, and in Go one named like a type of the
module takes `Fn` (`fn claim` with `record Claim` is `ClaimFn`; in Rust they are `claim` and
`Claim`). A record and an enum are in Pascal case. A parameter and a local are their aliases and
names in TypeScript, Python and Rust, and in mixed caps in Go (`paid_on` is `paidOn`). One spelled
like a word the language's code reaches for (Go's `len`, Python's `int`, TypeScript's `BigInt`), a
name of the module's code or an earlier one is moved aside: with `_` after it in TypeScript, Python
and Rust (a parameter `len_`, a local `int_2`), with a number after it in Go (`len2`). Python takes
the parameters by keyword only, and the function a host calls takes them by their aliases. In Rust,
a keyword is a raw identifier (`r#type`), and what no raw identifier writes and a type named like
one of the standard library's take `_` (`self_`, `Option_`).

**Each function** is two: the one a host calls, which checks its arguments against what they
declare and fails with `input` outside it, and the one the module's own functions call. Its effect
shapes it: in Go a function that can fail returns `(T, error)` and in Rust
`Result<T, kaname::KanameError>`, and a total one returns `T` (and panics with `input`;
`Check<Function>` in Go and `check_<function>` in Rust say so first); in TypeScript and Python each
has the union of the kinds it can fail with, `ClaimFails`, in its type and its comment.

```ts
export function claim(invoice: Invoice, paid_on: string, reference_rate: bigint): Claim | null {
```

```rust
pub fn claim(invoice: &Invoice, paid_on: kaname::Date, reference_rate: i64) -> Option<Claim> {
```

The code checks the places the analysis did not show, where kaname's reference interpreter checks
them, in the same order, and fails with `KanameError` of the same kind, line and message (in the
language of `--lang`). A call of a rule or a date it did not show is wrapped, and what the rule's
code throws becomes `rule` with the rule's error as its cause. The code is held to kaname's
reference interpreter on every vector of every function, in English and in Japanese, and passes
`tsc --strict --erasableSyntaxOnly`, `mypy --strict`, gofmt, `go vet`, and
`cargo clippy --all-targets -- -D warnings` on the editions 2021 and 2024 of Rust.

**A generic function** is one function, which the code of every module calls and no host: it is
exported, and checks nothing at an entry. A type parameter held to keys takes its order as a hidden
first argument (`kaname.Order<K>` in TypeScript, a key function in Python, `func(a, b K) int` in
Go), which the caller gives for the type it calls with; in Rust it is held to `K: Ord + Clone` and
takes nothing more. Values of a type parameter are compared by their contents through the helper
module (`eq` in TypeScript, `==` in Python, `Equal` in Go), and by their `PartialEq` in Rust.

**A function value** is an arrow function in TypeScript, a nested `def` in Python, a function
literal in Go and a closure in Rust, and a function given by its name is one that calls it. A
function that takes functions is one function, exported as a generic one is; in Go a failure of a
function value is carried as any other, to the function the host calls. **What is `private`** is not
exported in TypeScript and Rust, and its name starts with `_` in Python, Go and Rust.

**A transfer** is a value of its operation's type (`CaptureDo`, with `book`, `transfer`, `op` and
`args`; in Rust a struct of the arguments, with the constants `BOOK`, `TRANSFER` and `OP`), with the
union of the reasons the book can refuse it with at the amounts the modules give
(`CaptureDoRefusal`; in Rust an enum, and `as_str()` is the name the book writes). The function of
the book's `_apply` module makes one through chobo's client and answers what the client answers
(`done`, `done_before`, `refused` with its reason).

**A host** calls a module's functions as it calls any of the package: it imports `glue/<module>`
(`generated.glue.<module>` in Python, the module path given to `ritsu gen --module` and
`glue/<package>` in Go, `generated::glue::<module>` in Rust), builds the arguments in the types
above, and reads the answer, splitting a sum on `kind`, on `isinstance`, with a type switch or with
a `match`, and a `result` on `Ok` and `Err`. A Python `frozenset` and a Go `map` have no order, so a
host that prints a set or a dict sorts its keys itself. The examples `order_flow`, `stock_diff`,
`handles` and `catalog` each have a host in TypeScript, Python and Go under `host/`, which print the
same.

### The package of Rust

`ritsu gen --target rust` writes one crate into `<out>/rust/`, named by `--name` (`generated` unless
it is given):

```text
Cargo.toml, lib.rs
rules.rs, rules/<alias>.rs
dates.rs, dates/<alias>.rs
books.rs, books/<book>.rs
glue.rs, glue/<module>.rs, glue/kaname.rs, glue/door.rs
glue/books.rs, glue/books/<book>.rs, glue/books/<book>_apply.rs
deps.rs, deps/<name>.rs, deps/<name>/…
units.rs
wasm.rs, <name>.wit
```

The crates it depends on are those of the clients of the books, and only outside wasm32; with no
book it depends on nothing. A package of Rust holds no workflow and no code that asks a gate, which
dandori and sekisho do not write in Rust. With a flow or a gate among the files, `ritsu gen` says
what it leaves out on a line for each file, writes the rest, and exits 0:

```text
warning: A package of Rust holds no workflow, so it leaves out `orders/order.flow`; the other parts are written
```

`--library` writes no Rust (ritsu's DESIGN 17.16). A host takes the crate as a dependency by its
path, here the package of the example `late_payment`, with `check_claim` for an invoice it does not
trust:

```toml
[dependencies]
generated = { path = "generated/rust" }
```

```rust
use generated::glue::kaname::Date;
use generated::glue::late_payment::{Invoice, check_claim, claim};
use generated::units::GBPc;

let invoice = Invoice { id: "inv_1".to_string(), invoiced: Date { year: 2026, month: 3, day: 2 }, amount: GBPc(100_000) };
let paid_on = Date { year: 2026, month: 5, day: 20 };
check_claim(&invoice, paid_on, 50)?;
println!("{:?}", claim(&invoice, paid_on, 50));
```

```text
Some(Claim { due: Date { year: 2026, month: 3, day: 31 }, days_late: 50, compensation: GBP(70), interest: GBPc(1150) })
```

### The package's Wasm

The same crate with modules of kaname carries `wasm.rs` and a `.wit`, and the head of its
`Cargo.toml` has the line that builds it, for rustc's wasm32-unknown-unknown target, as one Wasm
module with no imports:

```sh
cargo rustc --lib --release --target wasm32-unknown-unknown --crate-type cdylib --config 'profile.release.opt-level="s"' --config 'profile.release.lto=true' --config 'profile.release.panic="abort"' --config 'profile.release.strip="symbols"'
```

The module exports `call: func(input: string) -> string`, as rulec's and koyomi's do, and answers
for every function a host calls in every module of the package. The input is
`{"module": <module>, "function": <function>, "in": {<parameter>: <value>, …}}`, each name as the
module writes it (a module of a dependency is `@pricing/fees`) and each value as `kaname vectors`
writes it; with one module of such functions in the package, `module` may be left out, so a line of
`kaname vectors` goes in as it is. The answer is that line without its `why`, with `fails` in the
place of `out` where the function fails. An input that cannot be read as a call is answered
`{"refused":"input","error":"…"}`. No input traps, but for a call the reference interpreter says
does not stop, which may use up the module's stack. The `.wit` makes it a component (here the
package of the example `late_payment`, with the built module and the `.wit` in one directory):

```console
$ wasm-tools component embed generated.wit generated.wasm -o generated.embedded.wasm
$ wasm-tools component new generated.embedded.wasm -o generated.component.wasm
$ wasmtime run --invoke 'call("{\"module\":\"late_payment\",\"function\":\"claim\",\"in\":{\"invoice\":{\"id\":\"inv_1\",\"invoiced\":\"2026-03-02\",\"amount\":100000},\"paid_on\":\"2026-05-20\",\"reference_rate\":50}}")' generated.component.wasm
"{\"function\":\"claim\",\"in\":{\"invoice\":{\"id\":\"inv_1\",\"invoiced\":\"2026-03-02\",\"amount\":100000},\"paid_on\":\"2026-05-20\",\"reference_rate\":50},\"out\":{\"due\":\"2026-03-31\",\"days_late\":50,\"compensation\":70,\"interest\":1150}}"
$ wasmtime run --invoke 'call("{\"module\":\"late_payment\",\"function\":\"claim\",\"in\":{\"invoice\":{\"id\":\"inv_1\",\"invoiced\":\"2026-03-02\",\"amount\":100000},\"paid_on\":\"2030-05-20\",\"reference_rate\":50}}")' generated.component.wasm
"{\"function\":\"claim\",\"in\":{\"invoice\":{\"id\":\"inv_1\",\"invoiced\":\"2026-03-02\",\"amount\":100000},\"paid_on\":\"2030-05-20\",\"reference_rate\":50},\"fails\":{\"kind\":\"input\",\"module\":\"late_payment\",\"line\":18,\"message\":\"The argument `paid_on` of `claim` is 2030-05-20, outside its range >=2026-01-01 <=2029-12-31\"}}"
```

A module's transfers are answered as the values they are (`{"book", "transfer", "op", "args"}`).
The clients of the books and the functions of the `_apply` modules wait on a database's driver and
are not in the Wasm, so a host makes a transfer with a client of its own. `kaname api` says under
`wasm` how each module is called.

## Across the languages

Some checks need two languages at once, and are ritsu's (ritsu's DESIGN 7), as for the calls of a
workflow of dandori:

| Check | What it holds |
|---|---|
| X2 | a rule's preconditions, at the call: from the relations kaname keeps (`before <= upto` of `running_totals`), else the ranges, else the runs of the function's vectors |
| X3 (a) | a day of koyomi given to a rule's date input, against the rule's range |
| X4 | an amount given to a transfer, against what chobo takes; and the reasons the book can refuse each `do` and `hold` with |
| X6 | a day given to a date's input, against koyomi's range |
| X17 | a field or a parameter with `fed by`, against what its source can come to |

They are said by `ritsu check` in ritsu's codes (E201 to W205), and come back to kaname through
ritsu's port: a place they show leaves the effect, and one they cannot decide or find an example of
is checked when the code runs (`ritsu W203` in the note of an E401). `ritsu kaname`, `ritsu check`
and `ritsu gen` hand kaname those verdicts. The binary of kaname's own crate has none, and its
places wait: their kinds are in the effect, but not held against what is written.

### `fed by` — where a value comes from

Hand-written code hands the module values it took from somewhere else. `fed by` at the end of a
record's field, or of a parameter of a function, says where:

```kn
record Line
  sku : string  fed by jsonschema "schemas/order.schema.json" def Line property sku
  qty : number  range >=1 <=20  fed by jsonschema "schemas/order.schema.json" def Line property qty

fn bill(
  lines: list[Line] count >=1 <=30,
  price: money[USDc] range >=1USDc <=1000USDc  fed by jsonschema "schemas/order.schema.json" def Product property price
) -> money[USDc] total
```

The rest of the line is a naming, in the form the whole toolchain names things; the path goes from
the directory of the file it is written in. A parameter with one stands on a line of its own, since
the naming reads to the end of the line, and a `,` after it ends it. Anywhere else, or when the
naming does not read or names something that holds no value, it is E009. `fed` and `by` are not
reserved: they are the form only when the two stand together at the end of a line.

| The value comes from | Named as |
|---|---|
| an output of a rule | `rulec "<file>" output <name>` |
| an account of a book | `chobo "<file>" account <name>` |
| a date of a dates file | `koyomi "<file>" date <name>` |
| a field of a `.proto` message | `proto "<file>" message <name> field <name>` |
| a property of an OpenAPI schema | `openapi "<file>" schema <name> property <name>` |
| what an operation of OpenAPI writes in place (a request body, a parameter) | `openapi "<file>" pointer "<pointer>"` |
| a property of a JSON Schema | `jsonschema "<file>" def <name> property <name>`, or `jsonschema "<file>" pointer <pointer>` |
| an output of a workflow, or a field of its record | `dandori "<file>" output <name>`, `dandori "<file>" record <name> field <name>` |
| a field of a kaname record | `kaname "<file>" record <name> field <name>` |
| a place that took the value first | `rulec "<file>" input <name>`, `koyomi "<file>" input <name>`, `dandori "<file>" input <name>`, `sekisho "<file>" resource <name> attribute <name>` (or `principal`), `sekisho "<file>" action <name> input <name>` |

A contract often leaves a bound to the code, which checks it by hand and then gives the value to a gate and to a workflow. The second can name the first: what comes from a place is what its own line writes. Two places that name each other say they take the same values, so whichever of the two moves, one of them stops.

`ritsu check` asks the language on each side, and comes to one of three answers:

- **It holds.** The place takes everything the source can come to. A place wider than its source is fine.
- **E207.** The source can come to a value the place refuses. The message gives the value, and the input that gives it when the source can say.
- **W207.** It cannot be decided, and the message says why: rulec counts an end of a rule's output that none of the rule's vectors reaches, or a contract carries a date as a string and lists no days.

E208 says the source is not there: its file, or the name in it.

Numbers are compared in one unit and one step, and another unit does not hold. The values of two enums are the same when they share a name, as a file writes it or as its public name, since the code that carries one turns one spelling into the other; a date by its days, a bool or a string by its type alone. An end the place does not write is not compared. A contract with no upper or no lower bound lets through a value beyond what the place takes, as with a rule's `from` (E122), and does not hold. So does an account of a book with no `at most`, which goes as far as chobo's limit (2⁶³ − 1).

**What is not checked.** Nothing checks that the hand-written code really carries that value. The
page of `kaname doc` says so: Comes from `…` (`ritsu check` holds the range of its values; the hand-written code that carries the value is not checked). Nothing `ritsu gen` writes changes, and `kaname api` gives
the naming as the field's or the parameter's `fed_by`.

## The page for people

`ritsu kaname doc <file.kn>` writes a page for the person who approves the decisions and the
engineer who reads how far each function is checked: the head with the module's digest and what it
reads; the decisions in tables, with the page rulec and koyomi draw of each rule and dates file it
calls and each transfer it builds with the reasons it can be refused with; the functions in three
parts, total, checked when they run (each place and why), and those that may not stop; the
its properties, each with its `when` lines and last line and whether a proof file is beside the `.kn` (not their answers, which are `check`'s); the
decisions written in code; and the examples with what they answer. The Markdown is for a pull
request, the pages of the rules and the dates folded in it; the HTML is one file that reads nothing
from outside, the pages opening over it.

## The keywords

| Where | Words |
|---|---|
| at the start of a line | `module` `description` `use` `enum` `record` `fn` `property` `examples`, and `private` before `enum`, `record` and `fn` |
| after `use` | `rule` `dates` `book` `module` `from` |
| types | `number` `bool` `string` `date` `rate` `money` `mass` `length` `area` `volume` `duration` `temperature` `sound` `list` `running` `dict` `set` `entry` `indexed` `result` `step` `incl_tax` `excl_tax` `range` `count` |
| after a type parameter | `key` |
| at the end of a function's head, and of a function type | `total` `fails` `diverges`, and the kinds `rule` `date` `range` `overflow` `zero` |
| in a body | `description` `when` `let` `try` `if` `then` `else` `match` `some` `none` `true` `false` `and` `or` `not` `in` |
| at the end of a field's line and of a parameter's | `fed` `by` |

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
$ ritsu check --lean examples/refund_split
```

- `check <file.kn>... [--format json] [--root <dir>] [--lean]`: everything above, and the line that
  says a module passes. `--lean` also has Lean take the proofs of the properties (Properties, above).
- `run <file.kn> <function> --input <file.json|-> [--trace] [--format json] [--root <dir>]`: one
  function in the reference interpreter on an input of JSON, each argument by its name or alias
  (`-` reads standard input), each value written as `vectors` writes its `in` (below; `run --help`
  lists the types one a line). It prints the call, the rules and dates it calls with their answers
  and the transfers it builds, in order, and the answer, or the failure with its kind and line. The
  failure `input` is of the input, so its line is the one the function is declared on, and it is
  said so: ``fails: input (`claim`, declared on line 18)``. A value of the wrong kind says how its
  type is written, with an example made from the type: `` `amount` is money[JPY], given as a whole
  number with no unit (`"amount": 1` is 1JPY); it is given a string ``. `--trace` adds the value of
  every `let`. A failure is an answer: exit 0. The input is read as deep as the arguments can nest,
  a value of a type that holds itself one deeper than the 200 a host may give (and so failing
  `input`), at least 256 levels of JSON and at most 1,024.
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
  "kaname": "0.51.0",
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
the languages that decides it, by the word `ritsu check --format json` gives the border
(`precondition`, `rule_date_input`, `transfer_amount` or `koyomi_date_input`), `code` the diagnostic
said of it, and `outcome` `example`,
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

`api` prints `{"kaname", "file", "module", "generated", "wasm", "uses", "enums", "records",
"functions", "properties", "transfers"}` (`properties` only for a module that writes some: each a
`name`, an `alias`, a `line`, a `description`, its `params` as a function's, and the number of its
`when` lines): the module with its SHA-256, the files of its code, how the package's
Wasm calls it (the exports, the keys of a call and the `module` that picks this module, the
functions a call reaches and the shapes of what comes back; `null` for a module with no function a
host calls), the files it reads, its
declarations with their types, ranges and counts (and, for a sum, each value's fields and the name
the code gives its type; a field or a parameter with `fed by`, its naming as `fed_by`), each function with its effect inferred and
written, the calls it makes into the other languages with what each check across them came to,
the places kaname could not show, the names the generated code gives it, and the functions a host
that does not trust its input calls first (`check`: Go's `Check<Name>` and Rust's
`check_<name>`, `null` for an inner or private function); and each operation of a transfer with
the reasons it can be refused with. A generic declaration has its `type_params`
(each a name, an alias and whether it is held to keys), an inner function `"inner": true`, and what
is `private` `"private": true`.

## Environment

| Variable | What it does |
|---|---|
| `KANAME_LANG` | `ja` or `en`, the language of the text when `--lang` is not given (not read inside `ritsu check`) |
| `RITSU_LANG` | the same, for every language of ritsu, after `KANAME_LANG` |
