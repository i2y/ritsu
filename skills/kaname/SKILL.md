---
name: kaname
description: Write, check and fix kaname modules (`.kn`), a small general-purpose language for the functional core of a program — the part with no input or output that only computes and decides, such as a state moved by events, input checked and normalized, counts compared in dicts and sets, a tree reshaped, and the code around business rules that calls rulec's rules, koyomi's dates and chobo's books — in ritsu's types, held to the rules of units, rounding, ranges and int64, each function's effect (total, checked when it runs, or may not stop) in its type, and generated as TypeScript, Python, Go and Rust by `ritsu gen` for a host to call. Use when such a core has to be written, checked or called from a host; when a kaname diagnostic (E001-E605, W301-W605, W901) or ritsu's E201-W205 across the languages has to be fixed; when a claim about every input of a function is to be written as a `property` and proved in Lean; or when a person has to be shown which functions are total and which decisions are in tables.
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
4. Generate the code into the host's package (`ritsu gen --target <language>`, once for each
   language, or the `target` line of ritsu.mod), call it from the host, and have CI run the same
   `ritsu gen` with `--check`.
5. Write the page with `ritsu kaname doc`, and show it to the people who approve the decisions.
6. For a claim about every input of a function, write a `property` and, if it is to be shown, a proof in
   Lean (section 5).

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
- What a property says, and which is wrong when it has a counterexample (E603), the function or the
  property: the claim is a promise, and whether to loosen it is a person's.

## 4. From a diagnostic to a fix

`ritsu kaname explain <code>` (and `codes.md`) gives each code's cause, its fix and its smallest
reproduction. The usual ones:

| Code | What it means | What to do |
|---|---|---|
| E001-E004 | a word, a line, a part's order or the indentation the language does not read | fix the line |
| E005-E007 | a name without an ASCII alias, given twice, or one the generated code cannot take | add or change the alias |
| W001, E008 | a module named `crate`, `self`, `Self`, `mod` or `door`, which the generated Rust alone cannot take: `kaname check` warns, and `ritsu gen --target rust` stops | if Rust is generated, change the name or the alias; else leave it |
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
| E603 | a property that is false for an input the check found (it names the input and the `let` values) | find why the function breaks it; ask whether the function or the property is wrong |
| E604 | `--lean` did not take the proof of a property | read the reason (the build, a missing theorem, a statement that differs, an axiom, a thing a proof file cannot hold) and fix the proof, never the statement |
| E605 | a parameter of a property that is a function | give the property the values the function would be made from |
| W603 | a property that holds on its vectors and is not shown | write the proof, or run `--lean`; for a rule's answers, a proof may use only what the port says |
| W604 | a property that uses what is not written for Lean yet | leave it to its vectors, or write it with what is written |
| W605 | the `when` lines of a property hold on none of its vectors | look at the `when` lines and the ranges again |
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

## 5. Properties and proofs

A `property` is a claim about every input of a function, which `examples` cannot be: the shares
of a refund come to the refund, no line gets a negative share. It is written like a function, with
`description`, `when` lines and a last line that must be `true`:

```kn
property shares_add_up(lines: list[Line] count >=1 <=10, refund: money[USDc] range >=0USDc <=10_000_000USDc)
  description "The shares come to the refund"
  sum(split(lines: lines, refund: refund), s => s.share) = refund
```

**Writing one.**

- The parameters' ranges and counts are what the claim speaks of, and what a proof may assume: write
  them as tight as the host promises, and no tighter.
- A `when` line narrows the inputs (`when refund <= sum(lines, l => l.left)`). Check that some
  input makes it true: W605 says that none of the vectors does, and a property that holds for no input
  can be proved whatever it says. Take W605 as a fault in the property.
- A property is total: if the body can fail (E401), narrow it with `when` or an `if`. It cannot be called,
  cannot be `private`, and a parameter cannot be a function (E605).
- A Japanese name needs an ASCII alias (E005); the alias is its name in Lean.
- `ritsu kaname check` tries the property on its vectors. E603 names an input that breaks it: either
  the function or the property is wrong, which is a person's to say. Do not narrow a `when` line to
  make a counterexample go away.

**Proving one.** `check` never says a property is shown; only Lean does. A property is proved in a
file beside the `.kn`, `<module alias>.lean`, which is the work of whoever writes the proof:

1. Run `ritsu gen --target lean`. It writes `generated/lean/Kaname/<alias>.lean` (the definitions,
   and one `abbrev <property> : Prop` for the statement of each property) and
   `generated/lean/samples/<alias>.lean`, a draft with a `sorry` for each proof.
2. Copy the draft beside the `.kn` as `<alias>.lean`, without its first three comment lines, and
   write a proof in place of each `sorry`. A theorem is named `Proofs.<alias>.<property>` and its
   type is the statement itself (`Kaname.<alias>.<property>`); write the type as the draft does, never
   as an opened `∀ …`, which is "states something else". A lemma may precede it in the same file.
3. `intro` the parameters and the range hypotheses the statement lists in order, `unfold` the
   function (the definitions are in `Kaname/<alias>.lean`), and finish with `omega`, `simp`,
   `decide` or `grind`. Only Lean's core is there, with no Mathlib (below), and the lemmas of
   `Kaname/Prelude.lean` for sums, `allocate` and `running_totals` (below): use them before proving
   the same again.
4. To try as you go, run `lake build` in `generated/lean/`; its lakefile reads the proof file where it
   is (run `ritsu gen --target lean` again after adding the file or changing a property). The Lean
   kaname writes builds with no warning, so a warning `lake build` prints is about the proof file
   (an unused `simp` argument, say); it does not stop `--lean`, and an error does.
5. At the end, run `ritsu check --lean`. It needs no `lake build` before it: it writes the package
   again from the `.kn` in a temporary directory. Only that says a property is shown; E604 says why
   a proof was not taken.
6. Never edit `generated/lean/`: `--lean` writes it again from the `.kn` and does not read it.
   A proof that cannot be found is news: say what is stuck, and try a counterexample (change the
   ranges, the `when` lines) before assuming the property is true.

**The lemmas of the prelude.** `Kaname/Prelude.lean` proves these, on the three axioms `--lean`
takes. They are in `namespace Kaname`, which the draft opens with the module
(`open Kaname Kaname.<alias>`); a name the module shares with the prelude (`Frac`, `Running`) is
then "Ambiguous term", and is written in full (`Kaname.<alias>.Frac`). kaname writes
`sum(xs, x => …)` as `(List.map (fun x => …) xs).sum`, the form of the first three.

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

Write a hypothesis as the goal reads, with the function applied: for
`runningTotals lines (fun l => l.left)`, `mem_runningTotals` takes `hw : ∀ l ∈ lines, 0 ≤ l.left`,
finds `w` from it, and answers `r.upto - r.before = r.item.left`. Written with `w` put in by hand,
`0 ≤ (fun l => l.left) l`, the hypothesis holds a value `omega` does not take for `l.left`, and its
proof needs `show 0 ≤ l.left` before `omega`.

**Without Mathlib.** Lean 4.34.0's core has no `set`, `ring`, `linarith`, `nlinarith`, `norm_num`,
`positivity`, `push_neg` or `use`: each is "unknown tactic". What to write instead, each tried on
4.34.0:

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

`grind` proves linear facts, an identity of polynomials (`(a + b) * (a + b) = a * a + 2 * a * b + b * b`),
`b * a < 4` from `a * b ≤ 3`, membership in `xs.map f`, `0 < (x :: xs).sum` from `1 ≤ x` and
`0 ≤ xs.sum`, and a definition by cases named to it (`grind [step]`). It does no induction, and does not get the sign
of a product or a quotient from the signs of its parts: `0 ≤ xs.sum` from its terms is `sum_nonneg`,
`0 ≤ a * b` is `Int.mul_nonneg`, and `0 ≤ allocate t c s` is `allocate_mono` with
`rw [allocate_zero]`. What it proves rests on `propext`, `Classical.choice` and `Quot.sound`, which
`--lean` takes.

**Opening a function over a list.** A function that maps `running_totals`, as `split` does, is
opened and its map put in the form of a lemma. From the proof of `shares_add_up` (`refund_split`):

```lean
  obtain ⟨S, hS⟩ : ∃ S, S = (lines.map fun l => l.left).sum := ⟨_, rfl⟩
  have hT : 0 < S := hS ▸ sum_pos hlen fun l hl => (hwithin l hl).1
  have key : (split lines refund).map (fun s => s.share)
      = (runningTotals lines (fun l => l.left)).map (fun r => allocate refund r.upto S - allocate refund r.before S) := by
    unfold split
    rw [List.map_map]
    apply List.map_congr_left
    intro r hr
    simp only [Function.comp, runningTotals_total _ _ r hr, hS]
  rw [key, runningTotals_telescope lines (fun l => l.left) (fun x => allocate refund x S), ← hS]
```

The sum is named `S` first, so that `rw` and `omega` see one value. `unfold split` shows the map that
makes the shares, `List.map_map` joins it to the map of the property, and `List.map_congr_left`
leaves one element `r` with `hr : r ∈ runningTotals …`. There `runningTotals_total _ _ r hr` puts the
whole sum for `r.total`, and the map is now the one `runningTotals_telescope` speaks of.

**An element of what such a function answers.** A property of every element,
`∀ s ∈ split lines refund, …`, is shown of the element of `running_totals` that `s` was made from.
From the proof of `no_share_is_negative` (`refund_split`), whole but for its first line:

```lean
  intro lines refund hlen _ hwithin hr0 _ s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hs
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have hleft := (hwithin r.item hitem).1
  have hw : ∀ l ∈ lines, 0 ≤ l.left := fun l hl => by have := (hwithin l hl).1; omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hmono := allocate_mono (t := refund) hr0 (htot ▸ hT) (show r.before ≤ r.upto by omega)
  simp only
  omega
```

- `unfold split at hs` shows `s ∈ List.map (fun r => …) (runningTotals …)`. `List.mem_map.mp hs` gives
  an element `r` with `hr : r ∈ runningTotals …` and the record the map makes of it, and
  `obtain ⟨r, hr, rfl⟩` puts that record for `s`. The goal is then a field of the record
  (`{ sku := …, left := …, share := … }.share ≥ 0`), and `simp only` puts the field's value in its
  place.
- What is known of `r` comes from its item. `List.mem_map_of_mem` with `(·.item)` and
  `runningTotals_items` give `r.item ∈ lines`, and `hwithin r.item hitem` the ranges of its fields:
  `Within` joins them with `∧` in the order of the fields, so `.1` is `1 ≤ r.item.left` here, and
  `simp only [Line.Within] at h` hands `omega` all of them. `mem_runningTotals` says how wide `r` is
  (`hwide : r.upto - r.before = r.item.left`), not that the width is above zero: without the item's
  range, `omega` fails, and the constraints it prints put `r.item.left` below zero.
- `no_share_is_more_than_its_left` goes the same way, with `allocate_sub_le` for `allocate_mono`.

**A number times each term.** `sum(lines, l => price * l.qty)` is
`(List.map (fun l => price * l.qty) lines).sum`, and `sum_mul_left` takes the number out of the sum.
The proof of `bill_is_the_price_times_the_quantities` (`order_bill`), whole but for its first line:

```lean
  intro lines price _ _ _ _ _
  unfold bill
  exact sum_mul_left price lines _
```

Lean finds the weight, `fun l => l.qty`, for the `_`; `rw [sum_mul_left]` does the same. With the
number on the right of each term (`l.qty * price`), `simp only [Int.mul_comm _ price]` turns the terms
round first.

**A case for each value of an enum.** A property of a value of an enum, with fields or without, is
shown value by value: `cases` makes a goal for each value, and `simp_all` with the functions the
goal calls, named, closes it. The proof of `a_closed_order_takes_no_event` (`order_flow`), whole but
for its first line:

```lean
  intro order event _ _ closed
  cases order <;> simp_all [«open», next]
```

On an open state, `«open»` answers `true`, and the hypothesis of the `when` line,
`closed : ¬(«open» order) = true`, is false; on a closed one, `next` answers the error whatever the
event. A name that is a word of Lean, such as `open`, is written in `«»`, as the generated module
writes it.

**What a proof file cannot hold** (it is read as text before Lean builds it, so that nothing it says
runs in the build): an import from outside `Kaname.`; `#eval` and every `#` command but `#check`,
`#print`, `#reduce` and `#guard_msgs`; `macro`, `syntax`, `elab`, `run_cmd` and the other commands
that add syntax or run code; `native_decide` and `decide +native`; an attribute other than `simp`,
`grind`, `reducible`, `irreducible` and `instance`; a `set_option` other than `maxHeartbeats`,
`maxRecDepth`, `synthInstance.*`, `linter.*`, `pp.*` and `trace.*`. `sorry` and an axiom of your own
are E604.

**What cannot be a property yet** (W604, which is still tried on the vectors): a dict or a set, a
builtin of strings other than `=` and `!=`, `sort_by`, `distinct`, `max_by`, `min_by`, `to_dict`,
`group_by`, a type parameter held to keys. A property that rests on what a rule or a dates file
answers is W603 "rests on the answers of the rule": a proof knows only the facts the port tells.

`--lean` needs Lean: [elan](https://github.com/leanprover/elan) and, on the first run, a download of
the toolchain (about 2.7 GB). Without `lake` it stops with exit code 2.

## 6. What it generates, and the page

`ritsu gen` writes each module under `glue/` in the package of each of TypeScript, Python, Go and Rust,
beside the code of the rules, dates and books it calls, with a helper module and no dependency.
Each function is one a host calls, which checks its arguments, and one the module's functions call.
Amounts travel in the package's one type of each unit; a rate as a count of its step. The code
checks the places the analysis did not show and fails with `KanameError` (`input`, `rule`, `date`,
`range`, `overflow`, `zero`), the line of the `.kn` and its message. In Go, only a function that can
fail returns `(T, error)`, and in Rust `Result<T, KanameError>`. A transfer is a value whose type lists the reasons it can be refused
with; the book's `apply` function makes it through chobo's client.

The host calls a module's functions from the package: `import { next } from "./glue/order_flow"` in
TypeScript, `from generated.glue import order_flow` in Python, in Go the path given to `ritsu gen
--module` with `/glue/orderflow` after it, and `use generated::glue::order_flow` in Rust, with the crate
as a dependency by its path. A value of a sum is an object with `kind`, a frozen data class, a struct of
an interface or an `enum` of structs; a `result` is the helper's `Ok` or `Err`, in Rust the standard
`Result`. Python's `frozenset` and Go's `map` have no order: sort the
keys to print them. The examples `order_flow`, `stock_diff`, `handles` and `catalog` of the
repository each have a host in TypeScript, Python and Go (`host/`).

The package of Rust is one crate. It holds no workflow and no code that asks a gate (a warning names
each file it leaves out), and `--library` does not write it. With modules of kaname, the head of its
`Cargo.toml` has the line that builds it as one Wasm module, whose `call` takes
`{"module", "function", "in"}` and answers the line of `kaname vectors` for that input
([reference.md](reference.md#the-packages-wasm)).

`ritsu kaname doc` writes the page for people: the rules and dates files called, with the pages
their languages draw, and the transfers built; the functions, total, checked when they run (each
place and why), or that may not stop; its properties and whether a proof is beside the .kn; the decisions in code; and the examples' answers. The
Markdown is for a pull request; the HTML is one file that reads nothing from outside. Change the
module, not the page.
