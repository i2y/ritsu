# Diagnostic codes

Written by `kaname explain --all --format markdown`; do not edit.

<a id="e001"></a>

## E001 — This cannot be read as kaname

**When**: A character the language does not have, a string or a bracket not closed, a tab, a full-width space, a date that does not exist, a unit the table of units does not have, or a thousands separator (`1,000USD`). The `==`, `&&`, `||` and `!` of other languages, and comparisons chained (`a < b < c`), are not kaname either; nor is a word or a sign where an expression or a type cannot take it.

**Fix**: Correct it where it points. Equal is `=`, not equal `!=`, and the others are `and`, `or` and `not`; two comparisons are joined, as in `a < b and b < c`, and `_` groups digits (`1_000USD`).

**Example**:

```kn
module example v1

fn same(a: number range >=0 <=9, b: number range >=0 <=9) -> bool
  a == b
```

See also: [E002](#e002)

<a id="e002"></a>

## E002 — A keyword cannot be a name

**When**: One of the thirteen words an expression or a line reads as something else (`let` `if` `then` `else` `match` `some` `none` `true` `false` `and` `or` `not` `in`) is written where a name goes. The other keywords are read only where they stand, and may be names (`total`, `count`, `description`).

**Fix**: Choose another name.

**Example**:

```kn
module example v1

fn f() -> number
  let then = 3
  3
```

See also: [E001](#e001), [E006](#e006)

<a id="e003"></a>

## E003 — The module is not put together as kaname reads it

**When**: The first line is not `module <name> v1`; the lines at the left margin are out of their order (`module`, `description`, `use`, the declarations (`enum`, `record`, `fn`), `examples`); a line written once (`module`, `description`, `examples`) is written twice; a line at the left margin starts with a word kaname does not know; an enum has `default` or `open`; or a `use` line is not of its form.

**Fix**: Write the module in its order: `module`, `description`, the `use` lines, the declarations, `examples`. The `enum`, `record` and `fn` declarations may come in any order among themselves.

**Example**:

```kn
module example v1

fn one() -> number
  1

use module pricing from "pricing.kn"
```

See also: [E004](#e004)

<a id="e004"></a>

## E004 — The indentation does not line up

**When**: The lines of one block are not indented alike, or an indented line follows a line that takes none.

**Fix**: Indent with spaces, every line of a block by the same amount.

**Example**:

```kn
module example v1

record Line
  sku : string
   qty : number  range >=1 <=20
```

See also: [E003](#e003)

<a id="e005"></a>

## E005 — A name the generated code takes needs an ASCII alias

**When**: The name of a module, a function, a parameter, a record, a field, an enum or a value of one is not ASCII and has no alias; or an alias is no ASCII name (a letter or `_`, then letters, digits and `_`). A `let` and a parameter of a function given to a builtin need none.

**Fix**: Write an alias in parentheses right after the name: `請求日(invoiced)`.

**Example**:

```kn
module example v1

fn café_price() -> money[USDc]
  350USDc
```

See also: [E007](#e007)

<a id="e006"></a>

## E006 — The same name is given twice

**When**: In a module, two of the `use` names and the declarations (`enum`, `record`, `fn`) share a name or an alias; or two fields of a record, two parameters of a function or two values of an enum do. So does a function or a `use` given a builtin's name (`sum`, `map`), and a `let` or a `some` whose name is taken where it stands.

**Fix**: Rename one of them.

**Example**:

```kn
module example v1

fn quote() -> number
  1

fn quote() -> number
  2
```

See also: [E005](#e005), [E110](#e110)

<a id="e007"></a>

## E007 — A name the generated code cannot take

**When**: A name the generated code takes (the alias, or the name with none) is a reserved word of TypeScript, Python or Go (`class`, `def`, `func`), or a name of kaname's helper module (`kaname`, `KanameError`).

**Fix**: Choose another name or alias.

**Example**:

```kn
module example v1

record Seat
  class : string
```

See also: [E005](#e005)

<a id="e101"></a>

## E101 — Nothing has this name

**When**: A name in an expression, a type, a function, an output of a rule, a field of a record, a value of an enum or a date of a dates file names nothing. A value of an enum is written `<enum>.<value>` where no type says which enum.

**Fix**: Check the spelling. What a `use` reads is written `<use name>.<name>`.

**Example**:

```kn
module example v1

fn total(price: money[USDc] range >=0USDc <=10_000USDc) -> money[USDc]
  price + shipping
```

See also: [E110](#e110)

<a id="e102"></a>

## E102 — The types do not agree

**When**: A value is not of the type of where it goes (a function's parameter or answer, a field of a record, an input of a rule or a date, an argument of a transfer). A unit and a tax are part of the type: two units of one dimension (`money[USD]` and `money[USDc]`) do not agree, nor do with tax and without. So do the two sides of a sum or a comparison, the answers of an `if` or a `match`, and the elements of a list, that are of two types.

**Fix**: Give a value of the type; a conversion is written, as in `fee in USDc` (kaname converts nothing unasked).

**Example**:

```kn
module example v1

fn fee(amount: money[USD] range >=0USD <=1000USD) -> money[USDc]
  amount
```

See also: [E103](#e103), [E104](#e104)

<a id="e103"></a>

## E103 — An unrounded value is given where a rounded one goes

**When**: An unrounded value — an amount times a rate, a quantity divided by a constant, anything divided by a value — is given to a function's parameter or answer, a field of a record, an input of a rule or a date, an argument of a transfer, or a weight of `running_totals` (the weights go into the fields of the records it answers). These take a whole number of their unit, or a rate on its step; a weight that is a rate is taken on the step its type says, and is unrounded only once a division by a value has left that step unknown. A `let` takes an unrounded value.

**Fix**: Write the way of rounding and the step (`half_up(amount * 3%, 1USDc)`); the five ways are `up`, `down`, `half_up`, `half_down` and `half_even`.

**Example**:

```kn
module example v1

fn fee(amount: money[USDc] range >=1USDc <=100_000USDc) -> money[USDc]
  amount * 3%
```

See also: [E102](#e102), [E104](#e104)

<a id="e104"></a>

## E104 — A grid, a step or a conversion does not fit

**When**: A literal is no whole number of the unit it is written in (`0.5USD`, a bare `0.4`), or of the unit it is read in (`1lb` compared with a `mass[g]`); `in` is asked to convert across currencies or dimensions; a step of a rate or of a rounding is not positive; or an end of a range or a value is not on a rate's step.

**Fix**: Write it in a smaller unit (`50USDc`), or a fraction as the rate it is (`40%`); two currencies have no exchange rate here, so a conversion between them belongs in a table.

**Example**:

```kn
module example v1

fn fee(amount: money[USD] range >=0USD <=1000USD) -> money[GBP]
  amount in GBP
```

See also: [E102](#e102), [E103](#e103)

<a id="e105"></a>

## E105 — The type has no such operation

**When**: An operation the type does not have: adding to a date (`paid_on + 30`: the one computation kaname does with dates is the difference of two), multiplying two amounts or a weight by a length, ordering strings or adding them, ordering the values of an enum with no order, arithmetic on or comparing a value that may be none, calling a value, splitting numbers with `match`.

**Fix**: Write the addition to a date in koyomi and call its date; split a value that may be none with `match`; join strings with `concat`.

**Example**:

```kn
module example v1

fn due(invoiced: date range >=2026-01-01 <=2026-12-31) -> date
  invoiced + 30
```

See also: [E102](#e102)

<a id="e106"></a>

## E106 — The match leaves values out

**When**: The arms of a `match` leave out a value of the enum, `true` or `false`, or `none` or `some x`; or an arm repeats a value an arm above takes.

**Fix**: Add an arm for each value left out, or keep each value in one arm.

**Example**:

```kn
module example v1

enum tier = basic | silver | gold

fn points(t: tier) -> number
  match t
    basic => 1
    silver => 2
```

See also: [E105](#e105)

<a id="e107"></a>

## E107 — The call does not give what it takes

**When**: A call of a function, a rule or a date misses an argument, gives one it does not take, gives one twice, or gives one with no name; a builtin is given a named argument or the wrong number of them; a record is built with a field missing or given twice; a function or a rule is written as a value and not called.

**Fix**: Give every argument once, by name (`charges(debt: d, days_late: n)`); a builtin takes its arguments in order, with no names.

**Example**:

```kn
module example v1

fn double(n: number range >=0 <=9) -> number
  n * 2

fn four() -> number
  double()
```

See also: [E204](#e204)

<a id="e108"></a>

## E108 — A declaration is missing or cannot hold

**When**: A number or a date of a function's parameters declares no range (`range`), a list no most elements (`count`), a rate no step (`rate[step 0.01%]`); so must every field of a record a parameter's type reaches (through lists, optionals and other records), while a record only answered with may leave its ranges out. The rate of a function's answer and of any field of a record declares its step too (only a `let` may leave it out). Also a range that holds no value, an end written twice, a `range` on what is no number or date, a `count` on what is no list, and a record that holds itself.

**Fix**: Declare the range (`qty : number range >=1 <=20`, `lines : list[Line] count >=1 <=30`): it is the promise to the host, which the generated code holds the arguments to.

**Example**:

```kn
module example v1

fn double(n: number) -> number
  n * 2
```

See also: [E104](#e104)

<a id="e109"></a>

## E109 — A builtin function is used in a way it has not

**When**: A function (`x => …`) is written where no builtin takes it; a builtin is given something that is no function where it takes one, or a function of the wrong number of parameters; `text` is given a rate or an unrounded value; the divisor of `mod` is no positive whole constant; or a builtin is written as a value and not called.

**Fix**: Write a function only where a builtin takes it, as in `sum(xs, x => …)`; a computation used elsewhere is a `fn` with a name.

**Example**:

```kn
module example v1

fn f() -> number
  let twice = x => x * 2
  1
```

See also: [E107](#e107)

<a id="e110"></a>

## E110 — A word reads two ways

**When**: Where a value of an enum goes, a word reads both as the value and as the name of a parameter or a `let`.

**Fix**: Rename one of them, or write the value `<enum>.<value>`.

**Example**:

```kn
module example v1

enum tier = basic | gold

fn is_gold(t: tier, gold: bool) -> bool
  t = gold
```

See also: [E006](#e006), [E101](#e101)

<a id="e111"></a>

## E111 — A block has no answer

**When**: The last line of a block is no expression with a value (it ends with a `let`, or there is no body); an `if` has no `else`; there are lines after a block's answer; or an `if`, an `else`, an arm of a `match` or a `let x =` has no block under it.

**Fix**: End a block with the expression it answers, and give an `if` its `else`; `none` says there is no value.

**Example**:

```kn
module example v1

fn late(days: number range >=0 <=9) -> number?
  if days >= 1
    days
```

See also: [E004](#e004)

<a id="e201"></a>

## E201 — A file this module uses cannot be read

**When**: A file a `use` reads is not there, its extension is not its kind's (`use rule` reads a `.rule`, `use dates` a `.cal`, `use book` a `.book`, `use module` a `.kn`), or it does not pass its language's check (the notes give what rulec, koyomi, chobo or kaname says of it).

**Fix**: Check the path (it is written from the directory of the module's file); correct a file that does not pass until its language's check passes it.

**Example**:

```kn
module example v1

use module pricing from "pricing.kn"
```

See also: [E202](#e202), [E203](#e203)

<a id="e202"></a>

## E202 — Rules, dates and books are read with ritsu kaname

**When**: A module that reads a rule, a dates file or a book is checked with the binary of kaname's own crate, which holds no rulec, koyomi or chobo. The exit code is 2.

**Fix**: Run the same command as `ritsu kaname …` (`ritsu kaname check payout.kn`).

**Example**:

```kn
module example v1

use rule card_fee from "card_fee.rule"
```

See also: [E201](#e201)

<a id="e203"></a>

## E203 — The modules use each other in a ring

**When**: Following the `use module` lines comes back to a module being read; every module in the ring says it.

**Fix**: Make the modules read one way only: what both need can go in a third module both read. Functions that call each other are written in one module.

**Example**:

```kn
module example v1

use module other from "other.kn"
```

`other.kn`:

```
module other v1

use module example from "example.kn"
```

See also: [E201](#e201)

<a id="e204"></a>

## E204 — The transfer is not one of the book's

**When**: A transfer or an operation the book does not have is written (a transfer done at once takes `do`, one that holds first `hold`, `post` and `void`); an argument of the transfer is missing, or one it does not take is given; or a `post` of a part gives only some of the amounts.

**Fix**: Write the book's transfer and operation as `<book>.<transfer>.<operation>(<argument>: …)`: `do` and `hold` give every argument, `post` and `void` the arguments of the key.

**Example**:

```kn
module example v1

use book ledger from "ledger.book"

fn take(payment: string, seller: string, amount: money[USDc] range >=1USDc <=100_000USDc) -> ledger.capture.do
  ledger.capture.hold(payment: payment, seller: seller, amount: amount)
```

`ledger.book`:

```
book ledger v1
description "A marketplace's money, in cents"

unit USD scale 2

account seller_balance(seller: string) : USD
  at least 0 refused as insufficient_balance
account cards : USD outside

transfer capture(payment: string, seller: string, amount: USD)
  key payment
  move amount from cards to seller_balance(seller)
```

See also: [E107](#e107)

<a id="e301"></a>

## E301 — A value can be outside the range of where it is given

**When**: An input of the function is found for which a value given to an input of a rule (a number, or an ordered enum the rule holds to a range over its values), an integer input of a date, a function's parameter or answer, a field of a record or `isqrt` is outside its range; the notes give the input and the value. The vectors of the function are run to look for one where the analysis of ranges cannot show the value stays in.

**Fix**: Narrow the value to the range with an `if` or a `match` before it goes there, or widen the range it goes to.

**Example**:

```kn
module example v1

fn charge(days: number range >=1 <=30) -> number
  days

fn late(paid: date range >=2026-01-01 <=2026-12-31, due: date range >=2026-01-01 <=2026-12-31) -> number
  charge(days: paid - due)
```

See also: [W301](#w301), [E401](#e401)

<a id="w301"></a>

## W301 — Whether a value stays in the range of where it is given cannot be decided

**When**: The analysis of ranges cannot show that the value stays in the range, and no vector of the function takes it out; the generated code checks it there when it runs. The analysis counts in intervals, so an expression that uses one value twice (`side * (10 - side)`) or the answer of a recursion is estimated wider than it is.

**Fix**: Narrow the value with an `if` or a `match`, which the analysis reads, and it is shown.

**Example**:

```kn
module example v1

fn area(side: number range >=1 <=8) -> number range >=0 <=24
  side * (10 - side)
```

See also: [E301](#e301)

<a id="e302"></a>

## E302 — A value can leave int64

**When**: An input of the function is found for which a value on the way, as a fraction in lowest terms, has a numerator or a denominator outside int64; the notes give the input.

**Fix**: Narrow the ranges of the inputs, or round sooner.

**Example**:

```kn
module example v1

fn double(n: number range >=0 <=5_000_000_000_000_000_000) -> number
  n * 2
```

See also: [W302](#w302)

<a id="w302"></a>

## W302 — Whether a value stays in int64 cannot be decided

**When**: The analysis of ranges cannot show that a value on the way stays in int64, and no vector of the function takes it out; the answer of a recursion with no range written is counted as any value. The generated code checks it when it runs.

**Fix**: Narrow the ranges of the inputs, or round sooner; give a recursive function the range of its answer.

**Example**:

```kn
module example v1

fn steps(n: number range >=0 <=100) -> number
  if n <= 0 then 0 else steps(n: n - 1) + 1
```

See also: [E302](#e302)

<a id="e303"></a>

## E303 — A divisor can be 0

**When**: An input of the function is found for which a divisor (the third argument of `allocate` too) is 0; the notes give the input.

**Fix**: Leave 0 out with an `if` before the division, or out of the divisor's range.

**Example**:

```kn
module example v1

fn each(amount: money[USDc] range >=0USDc <=1_000USDc, people: number range >=0 <=10) -> money[USDc]
  down(amount / people, 1USDc)
```

See also: [W303](#w303)

<a id="w303"></a>

## W303 — Whether a divisor stays away from 0 cannot be decided

**When**: The analysis of ranges cannot show that a divisor stays away from 0, and no vector of the function makes it 0. A value with a name is narrowed by an `if` that compares it, but no relation between two expressions is read (that `filter` keeps an element when `any` holds). The generated code checks it before it divides.

**Fix**: Give the divisor a name, and leave 0 out with an `if` before the division.

**Example**:

```kn
module example v1

record Line
  amount : money[USDc]  range >=0USDc <=1_000USDc
  media  : bool

fn media_average(lines: list[Line] count >=0 <=10) -> money[USDc]?
  if any(lines, l => l.media)
    down(sum(lines, l => l.amount) / count(filter(lines, l => l.media)), 1USDc)
  else
    none
```

See also: [E303](#e303)

<a id="e401"></a>

## E401 — The function does more than the effect written on it

**When**: A function written `total` has a place that can fail when it runs, or calls that go round (a recursion); or a function can fail with a kind its `fails` does not list, or recurs with no `diverges`. The effects of the functions it calls count. A place the checks across the languages decide (ritsu's X2, X3, X4 and X6) counts by their answer: one they do not show, or find an example for, can fail when it runs, as any other place; where the run has no answer, the place waits for one and does not count.

**Fix**: Make the places the notes give shown (write the branch that narrows the value), or widen the effect written; a recursive function is written `diverges`.

**Example**:

```kn
module example v1

fn steps(n: number range >=0 <=100) -> number range >=0 <=100 total
  if n <= 0 then 0 else min(steps(n: n - 1) + 1, 100)
```

See also: [W301](#w301), [W302](#w302), [W303](#w303)

<a id="w501"></a>

## W501 — A decision is written in code; a table should hold it

**When**: An `if` or a `match` answers a constant of the business (an amount, a quantity, a rate, a date, a value of an enum, `true` or `false`, a record of those; in a `match`, a number too); or a comparison of a value of the business with a threshold that is not 0 (a literal amount, quantity, rate, date or number) is made a value elsewhere than in the condition of an `if` or what a `match` splits. A branch that answers `none`, a `match` that only splits `none` from `some x`, one that maps the values of an enum to those of the same names of another, and a comparison with 0 are not warned of.

**Fix**: Write the decision in a rulec table and call the rule: there, rulec checks it is complete and without overlap, and the person who approves it reads it on its page.

**Example**:

```kn
module example v1

fn shipping(total: money[USD] range >=0USD <=10_000USD) -> money[USD]
  if total >= 35USD then 0USD else 5USD
```

<a id="e601"></a>

## E601 — An example does not hold

**When**: The function of a line of `examples`, run in the reference interpreter as a host would call it, answers other than the line writes; or does not fail with the kind written, is given an argument outside the function's ranges (the failure `input`), or does not stop, its calls going too deep.

**Fix**: Correct the function, or the answer the example writes. A failure is written with its kind, as in `fails range`.

**Example**:

```kn
module example v1

fn double(n: number range >=0 <=9) -> number
  n * 2

examples
  double(n: 2) = 5
```

See also: [E602](#e602)

<a id="e602"></a>

## E602 — An example is not a call with an answer of its type

**When**: A line of `examples` is no call of a function of the module, has neither `= <answer>` nor `fails <kind>` after the call, or gives an answer that is not a value of the function's type.

**Fix**: Write `<function>(<parameter>: <value>, …) = <answer>`, or `<function>(…) fails <kind>`; it may go on on the lines below, indented deeper.

**Example**:

```kn
module example v1

fn double(n: number range >=0 <=9) -> number
  n * 2

examples
  double(n: 2) = true
```

See also: [E601](#e601)

<a id="w901"></a>

## W901 — A value in the shape of a key is written

**When**: Somewhere in a `.kn`, in a string or a comment, there is a value in a shape its provider fixes for a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, a PEM private key. It is said of a module that does not read too. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line.

**Fix**: Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line.

**Example**:

```kn
module maps v1

# the maps API key of the staging site: AIzaSyD-ritsu-fake-key-for-tests-000000
fn region(code: string) -> string
  code
```
