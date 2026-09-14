<!-- Output of `rulec explain --all --format markdown --lang en`. Do not edit by hand. -->

# rulec diagnostics

Every code rulec can print, what makes it appear, and how to fix it. The code and the JSON shape are a stable API; only the prose improves (DESIGN §11 principle 5). For one of them: `rulec explain E101`.

| Code | Severity | Title |
|---|---|---|
| [E001](#e001) | error | Unterminated string |
| [E002](#e002) | error | Unreadable character |
| [E003](#e003) | error | The file does not start with a `rule` line |
| [E004](#e004) | error | The line does not start with a word |
| [E005](#e005) | error | A word that cannot appear at this position |
| [E006](#e006) | error | The declaration has no `=` |
| [E007](#e007) | error | No such policy |
| [E008](#e008) | error | Empty cell |
| [E009](#e009) | error | A declared name collides with a reserved word |
| [E010](#e010) | error | The `..` range notation is not allowed |
| [E011](#e011) | error | A public name has no ASCII alias |
| [E012](#e012) | error | The name is not declared |
| [E013](#e013) | error | No such import |
| [E014](#e014) | error | An output cell cannot hold an expression |
| [E015](#e015) | error | `result` can only assemble the first output |
| [E016](#e016) | error | There can be only one `result` |
| [E101](#e101) | error | Completeness gap: some input matches no row |
| [E102](#e102) | error | Unreachable row: the row never matches |
| [E103](#e103) | error | Unit mismatch: values of different types are being mixed |
| [E104](#e104) | error | A numeric output declares no rounding |
| [E105](#e105) | error | Overlapping rows: the same input matches two or more rows |
| [E106](#e106) | error | An output literal is not on the rounding grid |
| [E107](#e107) | error | An example does not match |
| [E108](#e108) | error | Cannot prove an intermediate value fits in int64 |
| [E109](#e109) | error | The check exceeded its budget, so completeness could not be proven |
| [E110](#e110) | error | A column has a type the check cannot handle |
| [E111](#e111) | error | The examples have no column for an output |
| [E112](#e112) | error | The range of a derived value does not contain the values it can reach |
| [E113](#e113) | error | The condition of a boolean definition is neither of the two allowed forms |
| [E114](#e114) | error | A cell value does not sit on the column's step |
| [E115](#e115) | error | Cannot divide by a variable |
| [W105](#w105) | warning | Shadowing that needs review: an earlier row hides part of a later one |
| [W110](#w110) | warning | A `first` table with no overlaps |
| [W111](#w111) | warning | A declaration is never used |
| [W114](#w114) | warning | Unconfirmed overlap: an input may match both rows |

## E001

`error` — **Unterminated string**

**When.** A string opened with `"` is not closed on the same line. Neither a cell nor a `description` may contain a line break.

**Fix.** Close the `"` on the same line. A long description either fits on one line or moves into a `#` comment.

**Smallest reproduction**:

```rule
rule t(t) v1
description "unterminated
```

Related codes: [E002](#e002)

## E002

`error` — **Unreadable character**

**When.** A character that is neither a letter nor `_` appears where a name is expected. The check exists so that a typo does not quietly become a name.

**Fix.** Delete the character. A name starts with a letter or `_` (`@x(x)` becomes `x(x)`).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  @x(x) : bool
```

Related codes: [E001](#e001), [E009](#e009)

## E003

`error` — **The file does not start with a `rule` line**

**When.** One `.rule` is one rule, and its first line carries the name and the version. Anything else comes before it.

**Fix.** Add `rule 規則名(alias) v1` as the first line.

**Smallest reproduction**:

```rule
inputs
  x(x) : bool
```

Related codes: [E004](#e004), [E011](#e011)

## E004

`error` — **The line does not start with a word**

**When.** A line that is neither a table row, a comment nor blank starts with a symbol. The syntax is line-oriented: the first word of a line decides what is being declared.

**Fix.** Start the line with a declaring word (`description / import / enum / group / inputs / outputs / derive / define / table / result / examples / policy`). A table row starts with `|`.

**Smallest reproduction**:

```rule
rule t(t) v1

= 1
```

Related codes: [E003](#e003), [E005](#e005)

## E005

`error` — **A word that cannot appear at this position**

**When.** The word at the head of the line is not in the vocabulary. The vocabulary has no synonyms: one English spelling each (§1.1).

**Fix.** Correct it to one of `description / import / enum / group / inputs / outputs / derive / define / table / result / examples / policy`. Business words belong in names and cells, not at the head of a line.

**Smallest reproduction**:

```rule
rule t(t) v1

foo bar
```

Related codes: [E004](#e004), [E009](#e009)

## E006

`error` — **The declaration has no `=`**

**When.** `enum`, `group`, `derive`, `define` and `result` separate the name from the body with `=`. It is missing.

**Fix.** Insert the `=`, e.g. `enum k(k) = a(a) | b(b)`.

**Smallest reproduction**:

```rule
rule t(t) v1

enum k(k) a(a) | b(b)
```

Related codes: [E005](#e005)

## E007

`error` — **No such policy**

**When.** The word after `policy` is not unique or first. DMN's Any, Priority and Collect are not adopted (§4).

**Fix.** Write either `policy unique` (every overlap is an error) or `policy first` (the earlier row wins).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : bool

table j(j)
policy any
| x | -> r(r) : bool |
| - | true |
```

Related codes: [W110](#w110), [E105](#e105), [W105](#w105)

## E008

`error` — **Empty cell**

**When.** A cell of a table holds only whitespace. **A blank is indistinguishable from a forgotten entry**, so it is a syntax error (§3).

**Fix.** If any value is meant, write `-`. If a value was forgotten, write the value.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
|   | true |
```

Related codes: [E101](#e101)

## E009

`error` — **A declared name collides with a reserved word**

**When.** A declared name (or alias) is the same as a word of the vocabulary. Otherwise the line-oriented parser reads the line as the start of a section and drops the declaration silently.

**Fix.** Rename it (`enum range(kind)` becomes `enum 範囲区分(range_kind)`). The reserved words are fixed by the one table in `src/kw.rs`.

**Smallest reproduction**:

```rule
rule t(t) v1

enum range(kind) = a(a) | b(b)
```

Related codes: [E005](#e005), [E011](#e011)

## E010

`error` — **The `..` range notation is not allowed**

**When.** A cell contains `..`, as in `0g..1000g`. Whether "up to 1000g" includes the endpoint cannot be read off the text (§3.1).

**Fix.** Rewrite it with comparison operators: `0g..1000g` is either `<=1000g` or `<1000g`. To fix both ends, write `>=0g <=1000g`.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : bool

table j(j)
policy unique
| w | -> r(r) : bool |
| 0g..1000g | true |
| >1000g | false |
```

Related codes: [E105](#e105), [E101](#e101)

## E011

`error` — **A public name has no ASCII alias**

**When.** The rule name, an input or an output has a **non-ASCII name** and no alias in parentheses. The alias becomes the public name in the generated code (a kanji has no uppercase and cannot begin an exported Go identifier). A name that is already ASCII is its own public name and needs no alias.

**Fix.** Add the alias in parentheses (`重量 : bool` becomes `重量(weight) : bool`). On a derived value, a definition, a group or a table the alias is optional: write one and the generated code uses it, leave it out and the declared name is used as it stands.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  重量 : bool

outputs
  r(r) : bool

table j(j)
policy unique
| 重量 | -> r(r) : bool |
| - | true |
```

Related codes: [E009](#e009), [E012](#e012)

## E012

`error` — **The name is not declared**

**When.** A column or an expression names something that is declared nowhere. There is no forward reference: a name is declared above the line that uses it.

**Fix.** Match the spelling to the declaration, or declare the input, derived value or definition above.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : bool

table j(j)
policy unique
| y | -> r(r) : bool |
| - | true |
```

Related codes: [E011](#e011), [E013](#e013)

## E013

`error` — **No such import**

**When.** The target of `import` is not in the built-in namespace. The only one for now is `std/都道府県` (47 values).

**Fix.** Correct it to `import std/都道府県`, or declare the enum in this file with `enum`.

**Smallest reproduction**:

```rule
rule t(t) v1

import std/nope
```

Related codes: [E012](#e012)

## E014

`error` — **An output cell cannot hold an expression**

**When.** A cell to the right of `->` holds two or more words. Only one value or one name may be written there (§3.2). Taking just the first word and skipping the rest would emit generated code with the multiplication silently dropped.

**Fix.** Give the calculation a name on a `define` line and leave only that name in the table (`| - | 率割引 |`). A table holds the branching and nothing else.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=1万円
  r(r) : rate[step 1%]  range >=0% <=100%

outputs
  o(o) : money[円, incl_tax]  round down(1円)

table j(j)
policy first
| r | -> o(o) : money[円, incl_tax] |
| <=5% | 0円 |
| - | p × r |
```

Related codes: [E008](#e008), [E012](#e012)

## E015

`error` — **`result` can only assemble the first output**

**When.** A `result` names an output other than the first. `result` is sugar for the first output, and both the evaluator and the generated code apply it only there (§1.2). The name used to be ignored, so a `number` could land in a `money` slot without an E103.

**Fix.** Write a `define` of the same name as that output (`define 付与点(pts) : number = 基本点 × 倍率`); outputs are taken, in declaration order, from the binding of their own name. To assemble it with `result` instead, move that output to the top of `outputs`.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=1万円

outputs
  a(a) : money[円, incl_tax]  round down(1円)
  b(b) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| p | -> a(a) : money[円, incl_tax] |
| - | 100円 |

result b = p
```

Related codes: [E016](#e016), [E103](#e103)

## E016

`error` — **There can be only one `result`**

**When.** A file has more than one `result` line. Only the first output can be assembled, so a second `result` merely replaces the first — which it used to do in silence.

**Fix.** Keep one. The other outputs are taken from a `define` of the same name as the output.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=1万円

outputs
  a(a) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| p | -> a(a) : money[円, incl_tax] |
| - | 100円 |

result a = p
result a = p + 100円
```

Related codes: [E015](#e015)

## E101

`error` — **Completeness gap: some input matches no row**

**When.** The union of the rows does not cover the declared input space. Completeness cannot be waived and is always required (§4). A concrete input that matches no row is always attached.

**Fix.** Add a row that matches the witness. If a new enum value caused it, add a row for that value or a `-` row that catches everything. If the value needs no row of its own, mark it `default` in the enum declaration.

**Smallest reproduction**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b) | c(c)

inputs
  x(x) : k

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
| a | true |
| b | false |
```

Related codes: [E102](#e102), [E105](#e105), [W111](#w111)

## E102

`error` — **Unreachable row: the row never matches**

**When.** Every input the row would take is already taken by an earlier row, or the row names a value that the upstream table never produces. The two forms are told apart in the wording.

**Fix.** If the row is the newer intent, move it above the row that covers it. If it is dead, delete it. If it names a value the upstream never emits, either add a row upstream that emits it, or delete this row.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : bool

table j(j)
policy first
| w | -> r(r) : bool |
| - | true |
| <=1000g | false |
```

Related codes: [E101](#e101), [W105](#w105), [W110](#w110)

## E103

`error` — **Unit mismatch: values of different types are being mixed**

**When.** An expression or a cell adds or compares values whose unit, currency or tax flag differ. `money[円, incl_tax]` and `money[円, excl_tax]` are different types too (§2.3).

**Fix.** Move one side into a table. "A surcharge that depends on weight" is `table 重量加算 | 重量 | -> 加算額 : money[円, incl_tax] |`. A tax conversion is also written as a table, never as a formula.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg
  p(p) : money[円, incl_tax]  range >=0円 <=1万円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| w | -> r(r) : money[円, incl_tax] |
| - | 100円 |

result r = p + w
```

Related codes: [E108](#e108), [E112](#e112)

## E104

`error` — **A numeric output declares no rounding**

**When.** A quantity, money or rate output has no `round`. Unless the fraction is declared, the generated code settles it silently. When the expression can produce a fraction, the message shows in yen how far the choice moves the answer.

**Fix.** Add rounding to the output declaration, e.g. `round up(10円)`. There are four directions (`up`, `down`, `half_up`, `half_even`), pinned down for negative values as well (§7.3).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]

table j(j)
policy unique
| x | -> r(r) : money[円, incl_tax] |
| true | 100円 |
| false | 200円 |
```

Related codes: [E106](#e106), [E103](#e103)

## E105

`error` — **Overlapping rows: the same input matches two or more rows**

**When.** In a `policy unique` table, an input matching both rows was actually constructed. An overlap that could not be constructed falls to W114 instead.

**Fix.** If the outputs differ, decide which is right and fix the rows; to let the order decide, declare `policy first`. If even the outputs agree, delete one of the rows.

**Smallest reproduction**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k
  y(y) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x | y | -> r(r) : money[円, incl_tax] |
| a | - | 100円 |
| - | true | 200円 |
| b | false | 300円 |
```

Related codes: [W105](#w105), [W114](#w114), [E102](#e102)

## E106

`error` — **An output literal is not on the rounding grid**

**When.** A literal in an output cell is not a multiple of the declared rounding grid. This is where a mistyped digit — `1451円` in a table rounded `up(10円)` — is stopped (§7.2).

**Fix.** Put the literal on the grid (`1451円` becomes `1450円` or `1460円`). If the amount really is right, change the grid instead.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]  round up(10円)

table j(j)
policy unique
| x | -> r(r) : money[円, incl_tax] |
| true | 1451円 |
| false | 1000円 |
```

Related codes: [E104](#e104)

## E107

`error` — **An example does not match**

**When.** Running a row of `examples` through the reference evaluator gives something other than the value written. **Which row of which table fired** is attached. `examples` is an executable specification.

**Fix.** If the table is right, fix the expected value. If the expected value is the business truth, fix the row that fired. Which one to fix is settled by the source: the written rule, the spreadsheet, or the legacy implementation.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x | -> r(r) : money[円, incl_tax] |
| true | 100円 |
| false | 200円 |

examples
| x | -> r |
| true | 200円 |
```

Related codes: [E111](#e111), [E105](#e105)

## E108

`error` — **Cannot prove an intermediate value fits in int64**

**When.** The reachable interval computed from the declared ranges and steps exceeds int64. With a rate step of 1%, the stored integer is 100 times the value.

**Fix.** Narrow the input ranges, or insert one rounding step along the way. Where to round is a business decision that moves yen, so the tool does not decide it (§7.1).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=100000000000000000円
  q(q) : rate[step 1%]  range >=0% <=100%

outputs
  r(r) : money[円, incl_tax]  round down(1円)

define off(off) : money[円, incl_tax] = p × q

table j(j)
policy unique
| off | -> r(r) : money[円, incl_tax] |
| - | 0円 |
```

Related codes: [E112](#e112), [E103](#e103)

## E109

`error` — **The check exceeded its budget, so completeness could not be proven**

**When.** The region check visited more nodes than `--budget` allows. Failing to prove something is never green here, so this is an error and not a warning.

**Fix.** Split the table to reduce the number of columns, or raise `--budget`. The cost is the product of the columns, so chaining tables in a linear pipeline is cheaper than piling columns into one table (§5.1).

**Smallest reproduction** (with `--budget 1`):

```rule
rule t(t) v1

enum k(k) = a(a) | b(b) | c(c)

inputs
  x(x) : k

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
| a | true |
| b | false |
| c | true |
```

Related codes: [E101](#e101), [W114](#w114)

## E110

`error` — **A column has a type the check cannot handle**

**When.** The column's type cannot be lowered into the region IR. **Seeing this is a bug in rulec itself.** It is the internal breakwater that stops a table from being skipped silently, put in after the same accident happened twice, with dates and with optional (§6.3).

**Fix.** Change the column to a type the check handles today (bool, enum, quantity, money, rate, date, optional). Then report it: this check exists because stopping is better than passing silently.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  s(s) : string

outputs
  r(r) : bool

table j(j)
policy unique
| s | -> r(r) : bool |
| "a" | true |
```

Related codes: [E101](#e101), [E105](#e105)

## E111

`error` — **The examples have no column for an output**

**When.** `examples` writes only some of the declared outputs. Three-way agreement (evaluator, Python, Go) stays green when all three share the same mistake, so **only a human-written expectation can break it**. Rounding for multiple outputs really did go missing in all three at once.

**Fix.** Add the missing output column to `examples`. With two or more outputs, writing `->` before the later columns is optional.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  ok(ok) : bool
  fee(fee) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x | -> ok(ok) : bool | fee(fee) : money[円, incl_tax] |
| true | true | 100円 |
| false | false | 0円 |

examples
| x | -> ok |
| true | true |
```

Related codes: [E107](#e107)

## E112

`error` — **The range of a derived value does not contain the values it can reach**

**When.** The interval computed from the input ranges falls outside the `range` declared on the derived value. With too narrow a range, the completeness check answers "complete" without ever looking at values that really occur.

**Fix.** Widen the `range` to the reachable interval the message states (it is written out, e.g. `range >=-110万円 <=100万円`). Widening past what is reachable costs nothing: the check sifts the infeasible part out.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : money[円, incl_tax]  range >=0円 <=100万円
  b(b) : money[円, incl_tax]  range >=0円 <=100万円

outputs
  r(r) : bool

derive gap(gap) : money[円, incl_tax] = a - b  range >=0円 <=100万円

table j(j)
policy unique
| gap | -> r(r) : bool |
| <=0円 | false |
| >0円 | true |
```

Related codes: [E108](#e108), [E101](#e101)

## E113

`error` — **The condition of a boolean definition is neither of the two allowed forms**

**When.** The condition of `define … : bool` is neither one input or derived value compared with a constant, nor a comparison of two values whose difference cannot be subtracted (two dates, say). Comparing two numbers directly is the usual case (§5.3).

**Fix.** Declare the difference as a derived value and compare that against a constant. `define bigger : bool = a >= b` becomes `derive gap(gap) : money[円, incl_tax] = a - b  range …` and the cell `>=0円`. The analysis is exact that way.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : money[円, incl_tax]  range >=0円 <=100万円
  b(b) : money[円, incl_tax]  range >=0円 <=100万円

outputs
  r(r) : bool

define bigger(bigger) : bool = a >= b

table j(j)
policy unique
| bigger | -> r(r) : bool |
| true | true |
| false | false |
```

Related codes: [E112](#e112), [E103](#e103)

## E114

`error` — **A cell value does not sit on the column's step**

**When.** A value that is not a whole number of the declared step is written in the column, such as `0.5%` where the type says `rate[step 1%]`. At runtime the value is one integer count of that step (§2.1), so this one has no representation.

**Fix.** Write a value on the step, or declare a finer step (`rate[step 0.1%]`). Quietly moving it to the nearest step would make the boundary on the page differ from the boundary in the generated code.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  r(r) : rate[step 1%]  range >=0% <=100%

outputs
  o(o) : bool

table j(j)
policy first
| r | -> o(o) : bool |
| <=0.5% | true |
| - | false |
```

Related codes: [E103](#e103), [E106](#e106)

## E115

`error` — **Cannot divide by a variable**

**When.** The right of `÷` is not a constant. A divisor is a positive whole constant, or a constant amount or quantity in the same unit (§2.3).

**Fix.** If the divisor is business data, take it as a rate input or look the constant up in a table. Without a statically known step the generated code falls back on the language's own division, and Python rounding toward -inf and Go toward zero disagree (§7.1).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=100
  d(d) : number  range >=1 <=100

outputs
  o(o) : bool

define r(r) : number = n ÷ d

table j(j)
policy first
| r | -> o(o) : bool |
| - | true |
```

Related codes: [E103](#e103), [E108](#e108)

## W105

`warning` — **Shadowing that needs review: an earlier row hides part of a later one**

**When.** In a `policy first` table, two rows partially intersect and disagree on the output. Structural shadowing (the staircase) and equivalent shadowing are folded into a count line; only the pairs that need review are listed (§4).

**Fix.** If it is intended, leave it: `check --diff-base` in CI reports only newly created pairs. To let the later row win, move it above the earlier one. `--show-shadow` lists every pair.

**Smallest reproduction**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k
  y(y) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy first
| x | y | -> r(r) : money[円, incl_tax] |
| a | - | 100円 |
| - | true | 200円 |
| - | - | 300円 |
```

Related codes: [E105](#e105), [W110](#w110), [E102](#e102)

## W110

`warning` — **A `first` table with no overlaps**

**When.** The table is `policy first`, yet no two rows overlap. The order carries no meaning, and `unique` is the stronger guarantee.

**Fix.** Change it to `policy unique`. From then on, the check guarantees that reordering the rows cannot change the meaning.

**Smallest reproduction**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k

outputs
  r(r) : bool

table j(j)
policy first
| x | -> r(r) : bool |
| a | true |
| b | false |
```

Related codes: [W105](#w105), [E105](#e105)

## W111

`warning` — **A declaration is never used**

**When.** An input, a derived value, a group or an enum value appears in no cell of any table. It can be the symptom of a forgotten column, or a legitimate contract. Values of an imported type are not checked this way.

**Fix.** If the input serves only as a range check at the entry, mark the declaration `contract_only`. If an enum value is meant to fall through to the default row, mark the value `default`. Otherwise either add the column to a table, or delete the declaration.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
| true | true |
| false | false |
```

Related codes: [E101](#e101), [E012](#e012)

## W114

`warning` — **Unconfirmed overlap: an input may match both rows**

**When.** Two rows of a `policy unique` table may overlap, but no input producing that was constructed and infeasibility was not proven either. It happens when derived values share inputs: sifting independent intervals does not see the dependency.

**Fix.** If an order satisfying both conditions can exist, fix the rows: the outputs differ, so a match is a contradiction. If none can exist, leave it — the generated code carries a guard that returns an error rather than silently picking the earlier row.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  total(total) : money[円, incl_tax]  range >=0円 <=100万円
  d1(d1) : money[円, incl_tax]  range >=0円 <=100万円
  d2(d2) : money[円, incl_tax]  range >=0円 <=100万円

outputs
  r(r) : bool

derive restA(rest_a) : money[円, incl_tax] = total - d1  range >=-100万円 <=100万円
derive restB(rest_b) : money[円, incl_tax] = total - d1 - d2  range >=-200万円 <=100万円

table j(j)
policy unique
| restA | restB | -> r(r) : bool |
| <=1000円 | - | false |
| >1000円 | <3980円 | false |
| >1000円 | >=3980円 | true |
| <=1000円 | >=3980円 | true |
```

Related codes: [E105](#e105), [W105](#w105), [E109](#e109)
