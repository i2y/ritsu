---
name: kaname
description: Write, check and fix kaname modules (`.kn`), a small general-purpose language for the functional core of a program — the part with no input or output that only computes and decides, such as a state moved by events, input checked and normalized, counts compared in dicts and sets, a tree reshaped, and the code around business rules that calls rulec's rules, koyomi's dates and chobo's books — in ritsu's types (records, enums with fields, lists, dicts, sets, strings, generics, `result`, function values), held to the rules of units, rounding, ranges and int64, with each function's effect (total, the places checked when it runs, or may not stop) in its type, and generated as TypeScript, Python and Go by `ritsu gen` for a host to call. Use when such a core has to be written, checked or called from a host; when a kaname diagnostic (E001-E602, W301-W501, W901) or ritsu's E201-W205 across the languages has to be fixed; or when a person has to be shown which functions are total and which decisions are in tables.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run kaname as `ritsu kaname <command>`, or as `kaname <command>` through a link to ritsu named for it. A module has no input or output of its own; the host reads and writes, keeps state, and makes the transfers a function returns.
license: MIT OR Apache-2.0
---

## When this applies

The job is **a functional core**: the part of a program that takes values and answers values, with
no input or output. The next state of an order for an event, the reasons a form is refused, the
difference of two counts of a stockroom, the paths of a tree of categories are each a function of
it. So is the code between the decisions of other languages: a rule of rulec decides, a dates file
of koyomi computes a day, a book of chobo moves an amount, and something has to put a rule's inputs
together, carry an answer into the next call, spread an amount over lines and hand an amount to a
book. kaname writes that code in ritsu's types, checks it by the rules the other languages are
checked by, and says for each function whether it answers every input it declares (`total`), which
places the code checks when it runs, or that it recurses and may not stop. The host calls the code
`ritsu gen` writes of it.

It does not apply to input and output of any kind, to the state a program keeps between calls, or
to the clock (the host's: it reads them and passes them in), to a decision of the business itself (a
threshold, a fee, a category chosen by a value: that is a rulec table), to a day computed from a day
(koyomi), to a balance kept by a book (chobo), or to waiting and side effects in order (a dandori
workflow).

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

1. Read the host's data the code is given and what it does with the answers, and the rules, the
   dates files and the books the code will call, if any (`ritsu rulec doc`, `ritsu koyomi doc`,
   `ritsu chobo doc`).
2. Write the module: the heading, the `use` lines if it calls other languages, the records and
   enums (a state with fields of its own is an enum whose values have fields), with the ranges the
   host promises where it can promise them, the functions, and `examples` from a case a person can
   check.
3. Run the check and fix what it says, one diagnostic at a time, from the top; `ritsu kaname explain
   <code>` says how. Then `ritsu check` on the project, which adds ritsu's checks across the
   languages (X2, X3, X4, X6) and hands kaname their verdicts.
4. Generate the code into the host's package (`ritsu gen`), call it from the host, and have CI run
   `ritsu gen --check`.
5. Write the page with `ritsu kaname doc`, and show it to the people who approve the decisions.

```console
$ ritsu kaname check examples/order_flow/order_flow.kn
$ ritsu kaname explain E301
$ ritsu kaname run examples/order_flow/order_flow.kn replay --input events.json --trace
$ ritsu kaname doc examples/late_payment/late_payment.kn --root examples/late_payment --format html
```

Run every command as `ritsu kaname <command>` (through a link to ritsu named kaname, `kaname
<command>` is the same): it reads the rules, the dates files and the books a module names in the
same process, where kaname built alone from its crate reads no other language and stops with E202.
Add `--lang ja` for Japanese.

## 2. The language on one page

```kn
# the handle a person typed, as it is kept, or every reason it cannot be
fn handle(raw: string) -> result[string, list[string]]
  let h = normalized(raw: raw)
  let why = problems(handle: h)
  if why.count() = 0 then ok(h) else err(why)

# a tag written `<handle>#<number>`: the handle made as `handle` makes it, the number from 1 to 9999
fn tag(text: string) -> result[Tag, list[string]]
  match text.index_of("#")
    none => err(["no # between the handle and the number"])
    some i =>
      let h = try handle(raw: text.take(i))
      match text.drop(i + 1).parse_number()
        none => err(["the number is not written in digits"])
        some n => if n >= 1 and n <= 9999 then ok(Tag { handle: h, number: n }) else err(["the number is not from 1 to 9999"])
```

```kn
# every code that differs, in the order of the codes: a dict and a set go round in their keys' order
fn changes(before: dict[string, number] range >=0 <=1_000_000, after: dict[string, number] range >=0 <=1_000_000) -> list[Change]
  let skus = before.keys().to_set().union(after.keys().to_set())
  skus.to_list().filter_map(sku => change(sku: sku, before: before.get(sku), after: after.get(sku)))
```

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
  `fn` in any order, then `examples`. Lines of a block are indented with spaces. `private` before an
  `enum`, a `record` or a `fn` keeps it to its module: another module naming it is E101, and what is
  not `private` takes no `private` type (E108).
- Types and arithmetic are rulec's: `money[GBP]`, `mass[oz]`, `rate[step 0.01%]`, `number`, `date`,
  `bool`, `string`, `T?`, enums; and `list[T]`, `dict[K, V]`, `set[T]`, records, a rule's
  `outputs`, a transfer's `ledger.capture.do`. A dict (`["a": 1, "b": 2]`) and a set
  (`xs.to_set()`) hold keys of a type kaname orders (a number, an amount, a quantity, a rate with a
  step, a date, a string, a bool, an enum whose values have no fields; else E114), and go round in
  that order in every target. Units never convert unasked: `fee in USDc`. A minus before a
  constant is a constant below zero in any unit (`t <= -18℃`). A string is a sequence of code
  points: its count, its cuts and its order are in code points, the same in every target.
- An enum may have its values under it, a line each, and a value its fields under it, as a
  record's: a **sum**. Build a value with its fields (`Order.placed { items: xs, on: d }`), and read
  them by splitting it with `match`, naming the value an arm takes (`placed p => p.on`); `else =>`
  takes the rest, and numbers, dates and strings are split by literals with an `else`. Make a record
  or a value from another with `{ ..x, f: v }`. `=` and `!=` compare two values of any one type
  whole.
- A record or a sum may **hold itself** (`children : list[Category]`, `next : Node?`), and two
  types may hold each other; each needs a value that does not (a field that holds it optional, a
  list that may be empty, a value of the sum without it; else E108). A host gives a value nested
  at most 200 deep (a deeper one fails with `input`), and a function that walks one recurses, and
  `diverges`.
- A function, a record and an enum take **type parameters** (`fn first_or[T](…) -> T`,
  `record Page[T]`); `K: key` holds one to the types that can be keys, which ordering, sorting and
  keys need (E114). A call works them out from its arguments and its place, or writes
  them (`empty_page[string]()`); E112 says when it cannot, or when a type may be none (`number?`).
  A generic function is called by kaname's code alone, never by a host, and is read again where it
  is called: its answer and its effect there are those of the values the call gives.
- A parameter of a **function type** (`amount: (T) -> money[USDc]`, `test: (number) -> bool total`)
  takes a lambda or a function by its name (`lines.map(line_cost)`), and calls it with its
  arguments in order (`amount(x)`). A lambda's parameters take their types from the place it is put
  in, or are written (`(l: Line) => …`; E112 where neither). A function value is given, named by a
  `let` and called, and nothing else: no answer, field, list, optional or `=` holds one (E113), and a
  builtin is no value (E109: wrap it in a lambda). A function that takes functions is read again
  where it is called, with what it is given: the places of a lambda are its writer's, the effect of
  what is given goes on to the caller, and a type that writes an effect holds what is given to it
  (E402).
- `result[T, E]` is `ok(x)` or `err(e)`, split by `ok x =>` and `err e =>`. `let x = try e` takes
  the value of an `ok` (or of an optional) and makes an `err` (or `none`) the function's answer, in a
  `let` of a block the function answers with (E115 elsewhere). A result is a business refusal; a
  place the checks could not show stays a failure of the effect.
- A value is **unrounded** when it is off the step of its type (an amount times a rate, a value
  divided by a value), and goes nowhere until a rounding puts it on one: `half_up(x, 1USDc)` (E103).
- A number, a date, a list and a string may carry a range or a `count` (a string's counts its code
  points). Written, it is the host's promise,
  checked before a function computes (`input`). Not written, the value takes the whole of its type
  (all of int64 in its unit, 0001-01-01 to 9999-12-31), and an overflow that comes from the end of
  the type is said by no code: the code checks it where it happens, and the effect has `overflow`. A
  rate carries its step (E108).
- A rule is called with its inputs by name and answers a record of its outputs (`.fee`), each in
  the range rulec holds it to (an output that reads an input with no range takes the whole of its
  type); a date of koyomi answers a day; a transfer is a value the host makes. A date has a
  difference (`b - a`, in days) and no `+`.
- Builtins: `count`, `sum`, `any`, `all`, `map`, `filter`, `first`, `max_of`, `min_of`,
  `running_totals`, `reduce`, with a function written where it is given; rulec's `min`, `max`,
  `mod`, `isqrt`, `allocate` and the five roundings; `starts_with`, `concat`, `text`. Each may be
  written after the value it takes first (`xs.sum(x => x.qty)`); those of dicts, sets, lists and
  strings are written so only (`d.get(k)`, `xs.sort_by(x => x.due)`, `xs.group_by(x => x.sku)`,
  `s.trim()`), and `numbers(a, b)` and `char(10)` as a function only (E101 says which). `mod`
  divides by a value too; its remainder is from 0 to one less than the divisor's size.
- A string has `count`, `slice`, `take`, `drop`, `index_of`, `contains`, `starts_with`,
  `ends_with`, `split`, `words`, `replace`, `trim`, `trim_start`, `trim_end`, `lower_ascii`,
  `upper_ascii`, `pad_start`, `pad_end`, `parse_number`, `parse_date`, `chars` and `code_points`;
  a list of strings `join`, of code points `to_text`. None fails: a cut past the ends is cut, and
  what cannot be read is `none`. White space is Unicode's White_Space, and case is ASCII's alone. A
  literal has no escapes: a new line is `char(10)`.
- `running_totals` is how an amount is spread: kaname knows `before <= upto <= total`, which a
  rule's preconditions ask, and that each `total` is the `sum` of the same list by the same field,
  so `if amount > sum(lines, l => l.left)` keeps `amount <= total` in its `else`.
- A name that is not ASCII carries an alias for the generated code: `遅れの請求(claim)`.
- The effect at the end of a function's head is checked when written: `total`, `fails rule, range`,
  `diverges`.

## 3. What to ask a person

- Who decides each choice a function would make, before writing an `if` that answers a constant:
  it may belong in a rule (W501).
- What range the host can promise for each parameter, and the most lines a list can have: a range
  written lets kaname show the places it bounds.
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
| E101 | a name names nothing, or a builtin written the way it is not (`keys(d)`) | fix the spelling, or declare it; write the builtin as the note shows (`d.keys()`) |
| E102 | types or units that do not agree, a currency or a tax that differs | convert with `in` where the dimension is one; else the value is the wrong one |
| E103 | an unrounded value where a rounded one goes | round it with the step the place takes; ask which way |
| E104 | a step, a rounding's step or a conversion that does not fit | write a whole number of the unit, or a step of the rate |
| E105 | an operation the type does not have (`date + 30`, strings added with `+`, a field of a sum read with `.`, arithmetic on a value of a type parameter) | use koyomi for days; join strings with `concat`; split a sum with `match` and read the value the arm names; compute with a type parameter's value where its type is one, at the caller |
| E106 | a `match` that leaves values out, or whose `else`, literals or names are not where they go | add the arm or an `else =>` (a split by literals needs one); keep `else` last and alone |
| E107 | a call that does not give what it takes | give every input by its name; a builtin's and a function value's arguments in order |
| E108 | a rate without its step, a range or a count that cannot hold, or a type that holds itself with no value that does not | give the rate its step; write a range with values in it; make a field that holds the type optional, or a list that may be empty |
| E112 | a type argument that cannot be worked out, is given twice, in the wrong number, may be none or is a function type; a lambda's parameter whose type nothing says; a type argument made from a type parameter in a ring of calls (`wrap_deep(x: [x], …)`) | write the type where the value goes (`let p : Page[number] = …`) or after the function's name; convert a unit with `in`; give `T` the type without `?`; write the lambda's types (`(l: Line) => …`); in a ring of calls, give the type parameters as they are or types that name none |
| E113 | a function value where none goes: an answer, a field, an element, an optional, a side of `=` | give the function where a function type is a parameter; write a function that takes the argument too, instead of one that answers a function |
| E114 | a type that cannot be a key of a dict, a member of a set or the key of an order; a type parameter ordered or made a key without `key` | key by a field that tells the values apart (`line.sku`); write `[K: key]` |
| E115 | `try` where it cannot go, on what is neither a result nor an optional, or with another error type | write it in a `let` of a block the function answers with; make the function answer a `result` of the same error type |
| E201 | a file a `use` reads does not pass its check or cannot be read, or a rule with no outputs (an `enums` file) called or its outputs written as a type | fix that file with its language; use an `enums` file for its enums only (`regions.region`) |
| E202 | run without the other languages | run it as `ritsu kaname` |
| E204 | a transfer or an operation the book does not have | follow the book |
| E301, W301 | a value can be outside the range of where it goes, or it cannot be decided | narrow it with an `if` before it goes (a value of an enum a rule holds to a range, with a `match` or an `if` that leaves a value out); ask whether the range should change |
| E302, W302 | a value can leave int64 (one that comes from a value whose range is not written is said by no code, and is in the effect) | narrow the ranges, or round earlier |
| E303, W303 | a divisor can be 0 | branch on it before dividing |
| E401 | a function does more than the effect written on it | make its places shown, or write the wider effect after asking |
| E402 | a function given does more than the effect its parameter's type writes | narrow the values with an `if` in the lambda, or widen the effect on the type after asking |
| W501 | a decision written in code | move it into a rule, after asking |
| E601, E602 | an example that does not hold, or is not a call | fix the function or the example; ask which is right |
| W901 | a key written in the `.kn` | take it out and keep it where the code runs; tell the person to revoke it if it is real (the history keeps it) |

`ritsu check` adds ritsu's codes across the languages, which `ritsu explain <code>` looks up:

| Code | What it means | What to do |
|---|---|---|
| E201, W201 | a call can break a rule's precondition, or it cannot be decided | give values kaname knows the relation of (`running_totals`, or a `sum` of the same list an `if` compared), or branch on the precondition |
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

The host calls a module's functions from the package: `import { next } from "./glue/order_flow"` in
TypeScript, `from generated.glue import order_flow` in Python, and in Go the path given to `ritsu gen
--module` with `/glue/orderflow` after it. A value of a sum is an object with `kind`, a frozen data class or a struct of an interface; a
`result` is the helper's `Ok` or `Err`. Python's `frozenset` and Go's `map` have no order: sort the
keys to print them. The examples `order_flow`, `stock_diff`, `handles` and `catalog` of the
repository each have a host in the three languages (`host/`).

`ritsu kaname doc` writes the page for people: the rules and dates files called, with the pages
their languages draw, and the transfers built; the functions, total, checked when they run (each
place and why), or that may not stop; the decisions in code; and the examples' answers. The
Markdown is for a pull request; the HTML is one file that reads nothing from outside. Change the
module, not the page.
