# The `.rule` language

The complete definition of the syntax. It is meant to be read by something that has to write
a `.rule` correctly the first time, so it states what is allowed rather than motivating it.
The reasoning behind each restriction is in `DESIGN.md`; the diagnostics you get for breaking
one are in `rulec explain --all`, and `rulec explain <CODE>` prints any of them.

Two things to know before anything else.

- **Keywords are English and there is exactly one spelling of each.** No synonyms and no
  abbreviations. Names and cell values are the business's own words and are written in
  Japanese in every rule in this repository.
- **The file is read top to bottom and there is no forward reference.** A name is declared
  above every line that uses it.

---

## 1. File structure

One `.rule` is one rule, and becomes one generated function. Sections appear in this order;
every one except `rule` is optional, but an order that differs from this one is an error.

```rule
rule <name>(<alias>) v<version>
description "<one line>"
import std/<name>
enum   …
group  …
inputs
  …
outputs
  …
derive …          ┐
define …          │  these three interleave freely, in dependency order
table  …          ┘
policy …
result …
examples
  …
```

- `rule` is the first line of the file. `v1` is the version; it is free text and appears in
  the generated header.
- `description` is one line, in quotes.
- `inputs` and `outputs` are followed by indented declarations, one per line.
- `derive`, `define` and `table` are a pipeline: each may use anything declared above it.
- `policy` belongs to the `table` immediately above it.
- `examples` comes last.

A blank line separates sections. `#` starts a comment that runs to the end of the line;
comments may appear anywhere, including at the end of a table row.

## 2. Lexical structure

**Encoding** is UTF-8 without a BOM. Identifiers are normalised to NFC. A cell never contains
a line break.

**Identifiers** start with a letter or `_` and run to the next delimiter. Kanji, kana and
Latin letters are all identifier characters; `-` never is (it is always an operator or the
don't-care cell). Anything else at the start of a name is E002.

**ASCII aliases** are written in parentheses after the name: `届け先(dest)`. They become the
identifiers the generated code uses.

An alias is **required** wherever a **non-ASCII** name reaches the public surface — the rule
name, an input, an output — because a kanji has no uppercase and so cannot begin an exported
Go identifier. A missing one is E011. **A name that is already ASCII needs no alias**: it is
its own identifier, so a rule written entirely in ASCII carries no parentheses at all.

```rule
rule bulk_fee v1            # no alias: the name is already an identifier
inputs
  weight : mass[g]  range >=1g <=40kg
```

Everywhere else an alias is **optional**, and writing one changes what the generated code
calls the value: `derive 残余(margin)` becomes `margin`, `group 遠隔地(remote)` becomes
`_remote` / `isRemote`, and a table output column `-> サイズ(size)` becomes `size`. Leave the
alias out and the declared name is used as it stands — every target language accepts a
Japanese identifier for something that does not have to be exported.

The alias on a **`table`** is the one exception: it is accepted and currently unused, because
a table is inlined into the one generated function rather than becoming a function of its
own. It is kept for SQL generation, where a table will need a name of its own.

**Numbers** are digits, optionally with `_` as a separator, optionally preceded by `-`,
optionally followed by a multiplier and then a unit:

| part | values |
|---|---|
| multiplier | `万` (10⁴), `億` (10⁸), `兆` (10¹²) |
| unit | `円` `銭` (money) · `g` `kg` (mass) · `cm` `m` (length) · `%` (rate) |

`1000万円` is 10,000,000 yen. **A number in a cell must carry its unit**: a bare `2000` where
a quantity is expected is an error. `2kg` and `2000g` are the same value; the stored integer
is always in the unit the type declares.

**Dates** are written `YYYY-MM-DD`. They are held as day ordinals, which is what makes an
interval over dates exact.

**Strings** are double-quoted and must close on the same line (E001).

**Symbols.** The canonical spellings are ASCII:

| written | means |
|---|---|
| `->` | the boundary between input and output columns |
| `,` | the separator inside a set, and between type attributes |
| `-` | don't care (a cell of its own) |
| `<= >= < >` | comparison |
| `+ - * /` | arithmetic in an expression |
| `\|` | a table cell boundary |
| `:` `( )` `[ ]` `?` | declarations |

`→`, `・`, `、`, `，`, `≦`, `≧`, `×`, `÷`, `−`, and fullwidth digits are all read, and
`rulec fmt` rewrites them to the ASCII forms. `..` is never read: see §7.

## 3. Types

Nine, and no others.

| type | written | notes |
|---|---|---|
| boolean | `bool` | |
| enum | the enum's name | a **closed** finite set, declared with `enum` or brought in with `import` |
| mass | `mass[g]`, `mass[kg]` | the unit is part of the type |
| length | `length[cm]`, `length[m]` | |
| money | `money[円, incl_tax]`, `money[円, excl_tax]` | currency **and** tax flag are both part of the type |
| rate | `rate`, `rate[step 1%]`, `rate[step 0.1%]` | with a step, the stored integer counts steps; without one, the step comes from the literals in the column |
| number | `number` | a whole number with no unit — a count of things, a number of days, a score |
| date | `date` | comparison and range only. **There is no date arithmetic** |
| string | `string` | **cannot be a table column** (E110). Use it for an output, or for an input that only passes through. A value that decides a branch belongs in an `enum` |
| optional | `会員区分?` | any of the above, plus the absent value. Consumed by the cell `none` |

Every quantity, money, rate, number and date is an **integer** internally. No floating point appears
anywhere in the tool or in the generated code.

Money of different currencies or different tax flags cannot be added or compared, and neither
can values of different units (E103). A conversion is written as a table, never as a formula.

### Declaring an enum

```rule
enum 会員区分(member_kind) = 一般(basic) | ゴールド(gold) | プラチナ(platinum)
```

A value may be marked `default`:

```rule
enum 会員区分(member_kind) = 一般(basic) default | ゴールド(gold) | プラチナ(platinum)
```

which declares "this value needs no row of its own; being caught by a `-` row is correct" and
silences W111 for it.

### Declaring a group

```rule
group 近畿圏(kinki) = 滋賀県, 京都府, 大阪府, 兵庫県, 奈良県, 和歌山県
```

A group is a named subset of an enum and may be used in a cell wherever a value may. Groups
are always expanded before checking, so a hole in a table written with groups is still found.

### Built-in enums

`import std/都道府県` brings in the 47 prefectures. It is the only built-in today (E013 for
anything else).

## 4. inputs and outputs

```rule
inputs
  届け先(dest)    : 都道府県
  重量(weight)    : mass[g]              range >=1g <=40kg
  注文金額(total) : money[円, incl_tax]   range >=0円 <=1000万円
  会員(member)    : 会員区分

outputs
  送料(fee) : money[円, incl_tax]  round up(10円)
```

### `range` — required on every numeric input and every `derive`

One declaration does three jobs.

1. **Overflow proof.** The reachable interval of every intermediate value is computed from
   the declared ranges and steps, and must fit in int64 (E108).
2. **The universe of the completeness check.** "Every input matches some row" means every
   input *within these ranges*.
3. **The entry guard of the generated code.** A call outside the range returns an error
   instead of silently computing something.

The form is `range` followed by one or two bounds: `range >=0円 <=1000万円`, `range >=1g`.
A `derive` whose declared range does not contain what it can actually reach is E112, and the
message states the interval to widen to.

`contract_only` marks an input that is only ever an entry check and appears in no table:

```
  重量(weight) : mass[g]  range >=1g <=25kg  contract_only
```

Without it, an input no table uses is W111.

### `round` — required on every numeric output

```
  送料(fee) : money[円, incl_tax]  round up(10円)
```

Four modes, each pinned down for negative values:

| mode | direction | at grid 1 |
|---|---|---|
| `up` | away from zero | −4.2 → −5 |
| `down` | toward zero | −4.8 → −4 |
| `half_up` | an exact half goes away from zero | −4.5 → −5 |
| `half_even` | an exact half goes to the even neighbour | 2.5 → 2, 3.5 → 4 |

The value in parentheses is the **grid**: `up(10円)` rounds to a multiple of 10 yen. Rounding
is applied once per output, last. A missing one is E104; an output literal that is not a
multiple of the grid is E106.

## 5. derive

A linear combination of inputs, which **can be used as a table column while still being a
quantity**.

```rule
derive 適用後金額(net) : money[円, incl_tax] = 商品合計 - 割引額  range >=0円 <=100万円
```

The right-hand side may use inputs, `+`, `-`, and multiplication by a constant. `range` is
required and behaves exactly as it does for an input.

## 6. define

A named boolean or intermediate value. A boolean `define` may be used as a table column.

```rule
define 大口(bulk) : bool = 注文金額 >= 3万円
define Aが早いか同じ(a_earlier) : bool = A期限 <= B期限
define 率割引(rate_off) : money[円, incl_tax] = 元価 × 割引率
```

The condition of a boolean `define` must be one of exactly two shapes (E113):

1. a **unary test on one input or one derived value** — that value compared with a constant;
2. a comparison of **two values of a type whose difference cannot be derived** (two dates,
   for instance).

A direct comparison of two numbers is neither. Declare the difference as a `derive` and
compare that with a constant; the analysis is exact that way, and the message says so.

## 7. Tables

```rule
table 基本送料(base_fee)
policy unique
| 届け先      | 重量    | -> 基本送料(base) : money[円, incl_tax] |
| 遠隔地      | <=2000g | 1200円                                  |
| 遠隔地      | >2000g  | 1800円                                  |
| not: 遠隔地 | <=2000g | 800円                                   |
| not: 遠隔地 | >2000g  | 1100円                                  |
```

The first row is the header. Left of `->` are input columns, right of it output columns. A
column may name an input, a `derive`, a boolean or enum `define`, or an output of an earlier
table. An output column that introduces a new name declares its type there.

### Policies

| policy | meaning |
|---|---|
| `unique` (the default) | no two rows may overlap. Reordering the rows cannot change the meaning |
| `first` | the earliest matching row wins |

There is no Any, Priority or Collect, and **completeness cannot be waived**: a table with a
hole is E101 whichever policy it uses.

### The seven kinds of cell

| written | means |
|---|---|
| `-` | any value. **A blank cell is a syntax error** (E008): a blank cannot be told from a forgotten entry |
| `1200円` `2000g` `true` `2026-04-01` `"abc"` | equality with a literal. A quantity must carry its unit |
| `北海道, 沖縄県` | a set. Each element is a literal or a group name |
| `not: 遠隔地` | the complement of a set |
| `<=2000g` | comparison. `<=`, `>=`, `<`, `>` |
| `>=1000円 <20000円` | an interval — two comparisons side by side mean "and" |
| `none` | an optional that is absent |

A cell tests **its own column only**. There is no expression, no reference to another column,
and no function call inside a cell; that restriction is what makes a row a box and the
completeness and overlap checks exact.

**`..` is a syntax error** (E010). `0g..1000g` does not say whether 1000g is included; a
comparison operator does.

An output cell holds a literal or the name of a value declared above (an input, a `derive` or
a `define`). It never holds an expression.

## 8. result

Assembles the first output when it is not simply looked up from a table.

```rule
result 送料 = 基本送料 × 負担率
```

`result` is sugar for the **first** output and reaches no other. Every output — the first one
included — is otherwise taken from the binding of its own name: a `define` or a table output
column called `送料` is what the output `送料` returns. Naming a later output in a `result` is
E015; a second `result` line is E016.

Operators, from loosest to tightest: comparison (`<= >= < > =`), then `+ -`, then `* /`.
Parentheses group. The two functions are `min(a, b)` and `max(a, b)`, and the four rounding
modes may also be called as functions: `down(x, 1円)`, `up(x, 10円)`, `half_up(x, 1円)`,
`half_even(x, 1円)`.

**There is no loop and no recursion.**

## 9. examples

```rule
examples
| 届け先 | 重量  | 注文金額 | 会員     | -> 送料 |
| 沖縄県 | 2500g | 40000円  | 一般     | 0円     |
| 東京都 | 1999g | 12000円  | プラチナ | 400円   |
```

`examples` is an **executable specification**: `rulec check` runs every row through the
reference evaluator, and a row that does not hold is E107, reported with the rows that fired.

**Every output must have a column** (E111). With two or more outputs, writing `->` before the
later output columns is optional; `rulec fmt` folds it to the canonical form.

## 10. What cannot be written

- Nested objects (`注文.配送先.都道府県`). Flatten at the boundary and pass the scalar in.
- Collections and iteration. A rule is one decision; the order and the repetition belong to
  the caller.
- Date arithmetic. Comparison and range only.

Allowing any of the three would make the completeness and overlap checks unable to terminate.

## 11. The canonical form

`rulec fmt` is the one and only formatter and it is idempotent. It

- aligns the columns of every table (East Asian width, so Japanese names line up in a
  terminal),
- rewrites `→ ・ 、 ， ≦ ≧` and fullwidth digits to their ASCII forms,
- folds the `->` of the second and later output columns of `examples`,
- leaves the inside of a comment alone.

`rulec fmt --check` names the files that are not in canonical form and exits 1, which is how
it belongs in CI.

## 12. Reserved words

These cannot be used as a name or an alias (E009). A declaration whose name is one of them
would be read by the line-oriented parser as the start of a section and silently dropped,
which is why it is caught at parse time.

<!-- RESERVED -->
| | |
|---|---|
| line heads | `rule` `description` `import` `enum` `group` `inputs` `outputs` `derive` `define` `table` `policy` `result` `examples` |
| modifiers | `range` `round` `contract_only` `default` |
| cells | `not` `none` `true` `false` |
| rounding | `up` `down` `half_up` `half_even` |
| functions | `min` `max` |
<!-- /RESERVED -->

`step` (inside `rate[step 1%]`), `unique`, `first`, the type words (`money` `mass` `length`
`rate` `number` `bool` `date` `string`), the money attributes (`incl_tax` `excl_tax`) and `std` are
part of the vocabulary but are told apart by position, so they are not reserved as names.

A test holds this table to `src/kw.rs`, which is the single place the vocabulary is defined.
