# Write a table (.rule)

A `.rule` file **has one fixed shape, read from the top**. There is no
forward reference, so reading downwards is reading the dependencies in
order. **The keywords are English; the names and the cell values stay in
the language of the business** — Japanese, in every example here.

!!! note "Usually an agent writes this"

    Nothing stops you writing a `.rule` by hand, and this page is also how. But the shape
    this tool is built for is an agent transcribing from a published policy or a
    spreadsheet, and a person reading the table it produced and approving it. So what this
    page is most worth for is learning to **read** one — writing follows from the same
    material.

    And you are not left alone with the file when you read it. `rulec doc` renders the
    document for approval, and adds what reading the table cannot tell you: what a single
    word in the table actually stands for, which rows are hidden by the rows above them,
    and which roundings are placeholders rather than decisions. The same document comes
    as one HTML page too (`--format html`), where typing a case lights up the rows that
    matched and shows the result. For a change, how many records move and by how much
    comes out before it ships. Both are things to ask the agent for, and so is an
    explanation of any row. From a spreadsheet, `rulec import xlsx` writes the first
    draft out of the workbook itself, with every guess marked. Its own procedure is in [For agents](agents.md).

These are all the words that may start a line.

| word | what it declares |
|---|---|
| `rule` | the first line: the rule's name and version |
| `description` | one line of prose |
| `import` | brings in a built-in enum |
| `enum` | a closed enumeration |
| `group` | a named subset of an enum |
| `inputs` | the rule's arguments |
| `outputs` | its results |
| `derive` | a linear combination of inputs — **the only intermediate that can sit in a table column while still being a quantity** |
| `define` | a boolean, or a computed intermediate |
| `table` | a decision table. The body of the language |
| `policy` | that table's hit policy (`unique` or `first`) |
| `result` | assembles an output |
| `examples` | an executable specification |

## A complete rule

This one checks clean as it stands; the repository's test suite runs it
on every commit.

```rule
rule 送料例(fee_demo) v1
description "The README's example. Passes rulec check as written"

import std/都道府県

enum サイズ区分(size_class) = S60(s60) | S80(s80) | S100(s100)
group 近畿圏(kinki) = 滋賀県, 京都府, 大阪府, 兵庫県, 奈良県, 和歌山県

inputs
  あて先(dest)    : 都道府県
  三辺合計(girth) : length[cm]  range >=1cm <=100cm
  重量(weight)    : mass[g]   range >=1g <=25kg  contract_only

outputs
  運賃(fee) : money[円, incl_tax]  round up(10円)

table サイズ判定(size_of)
policy first
| 三辺合計 | -> サイズ(size) : サイズ区分 |
| <=60cm   | S60                          |
| <=80cm   | S80                          |
| -        | S100                         |

table 運賃表(fee_table)
policy unique
| あて先      | サイズ | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏      | S60    | 990円                              |
| 近畿圏      | S80    | 1310円                             |
| 近畿圏      | S100   | 1620円                             |
| not: 近畿圏 | S60    | 880円                              |
| not: 近畿圏 | S80    | 1200円                             |
| not: 近畿圏 | S100   | 1500円                             |

examples
| あて先 | 三辺合計 | 重量 | -> 運賃 |
| 大阪府 | 55cm     | 1kg  | 990円   |
| 東京都 | 90cm     | 3kg  | 1500円  |
```

## Names and ASCII aliases

What is in the parentheses is the **ASCII alias**, and it becomes the
public name in the generated code — a kanji cannot be an
exported Go identifier.

```rule
enum 会員区分(member_kind) = 一般(basic) | ゴールド(gold) | プラチナ(platinum)
```

An alias is required only where a **non-ASCII** name reaches the public
surface — the rule name, the inputs, the outputs — because a kanji has
no uppercase and cannot begin an exported Go identifier. **A name that
is already ASCII needs none**: write the whole rule in English and there
are no parentheses anywhere.

Elsewhere the alias is optional, and writing one decides what the
generated code calls the value: `derive 残余(margin)` becomes `margin`,
`group 遠隔地(remote)` becomes `_remote` / `isRemote`, a table output
column `-> サイズ(size)` becomes `size`. Leave it out and the declared
name is the identifier — every target language takes a Japanese one for
something that is not exported.

The alias on a `table` is the exception: accepted, and currently unused,
because a table is inlined into the one generated function instead of
becoming a function of its own. It is kept for SQL generation.

## Types

Eight, and no others.

| type | written | the thing to know |
|---|---|---|
| boolean | `bool` | |
| enum | `会員区分` | a **closed** finite set. Declared with `enum` or brought in with `import` |
| quantity | `mass[g]` `length[cm]` | **the unit is part of the type**. `2kg` is sugar for `2000g`; at run time the value is one integer in the declared unit. Mass is `mg g kg t oz lb`, length `mm cm m km in ft yd mi` |
| money | `money[円, incl_tax]` `money[USD, excl_tax]` | **branded twice**, by currency and by tax flag. `incl_tax` and `excl_tax` do not add. The currency is `円` or any ISO 4217 code; its hundredth is the code plus `c`, so `money[USD]` counts dollars and `money[USDc]` cents. **Two currencies never convert** — there is no exchange rate here, and mixing them is E103 |
| rate | `rate[step 1%]` `rate` | an integer throughout. With a step, the integer counts steps (`10%` is 10); without one, the step comes from the literals in the column |
| date | `date` | comparison and range only. **There is no date arithmetic** |
| string | `string` | **cannot be a table column** (E110). Use it for an output, or for an input that only passes through. A value that decides a branch belongs in an `enum` |
| optional | `会員区分?` | consumed only by the cell `none` |

Quantities, money, rates and dates are **all integers** — a date is a day
ordinal, a rate a count of steps. No floating point appears anywhere:
how a fraction is settled is decided by `round` below, never by the
host language's division.

**Enums are closed, and there is no open enum.** That is the point: when
a value is added, every table that has not accounted for it breaks the
completeness check.

```rule
enum 会員区分(member_kind) = 一般(basic) default | ゴールド(gold) default | プラチナ(platinum)
```

`default` declares "this value needs no row of its own; being caught by
a `-` row is correct". Without it, a value no row names is reported.

A **group** is a named subset of an enum and may be used in a cell
wherever a value may. Groups are always expanded before checking, so a
hole in a table written with groups is still found.

```rule
group 遠隔地(remote) = 北海道, 沖縄県
```

The built-in `std/都道府県` (47 values) arrives with
`import std/都道府県`.

## Inputs and outputs

```rule
inputs
  届け先(dest)    : 都道府県
  重量(weight)    : mass[g]              range >=1g <=40kg
  注文金額(total) : money[円, incl_tax]   range >=0円 <=1000万円
  会員(member)    : 会員区分

outputs
  送料(fee) : money[円, incl_tax]  round up(10円)
```

There may be several outputs. They become a `NamedTuple` in Python, an
`interface` in TypeScript, a plain object in JavaScript, a `Struct` in
Ruby, a struct in Rust, Swift and Go, and one column each in SQL — and
**rounding applies once per output**.

```rule
outputs
  可否(ok)    : bool
  素割引(raw) : money[円, incl_tax]  round down(1円)
```

An output returns **the binding of its own name** — a `define` or a table
output column called `素割引` is what the output `素割引` returns. `result`
is sugar for that, and it reaches **the first output only**: naming a later
one is E015, and a second `result` line is E016.

## `range` and `round` — the two you are not allowed to forget

These are not decoration. They are where this language is aimed.

### `range`, on every numeric input and every `derive`

One declaration does three jobs.

1. **Overflow proof.** Whether an intermediate fits in int64 is computed
   from the declared ranges and steps.
2. **The universe of the completeness check.** "Every input matches some
   row" means every input *within these ranges*.
3. **The entry guard of the generated code.** Called outside the range,
   it returns an error instead of silently computing.

A `derive` whose range does not contain the interval it can actually
reach is an error — if inputs of 0 to 1,000,000 yen can produce
−100,000, then −100,000 has to be in the range.

An input that only ever acts as an entry check takes `contract_only`:
"this does not appear in any table's conditions, but the contract still
holds it to its range". That silences the unused warning, and nothing
else does.

### `round`, on every numeric output

Without it, the generated code would settle fractions on its own. Four
modes, each **pinned down for negative values too**.

| mode | direction | at grid 1 yen |
|---|---|---|
| `up` | away from zero | −4.2 → −5 |
| `down` | toward zero | −4.8 → −4 |
| `half_up` | an exact half goes away from zero | −4.5 → −5 |
| `half_down` | an exact half goes toward zero — the payroll deduction rule of the social insurance tables (50銭以下切り捨て) | 4.5 → 4, 4.6 → 5 |
| `half_even` | an exact half goes to the even neighbour | 2.5 → 2, 3.5 → 4 |

What is in the parentheses is the **grid**: `up(10円)` rounds to a
multiple of 10 yen, so −4.2 yen becomes −10 yen.

The negative direction is pinned because **integer division in Python and
Ruby rounds toward −∞ while in Rust, Swift, Go, TypeScript, JavaScript
and SQL it truncates toward zero**. Left to the host language, one rule
would answer differently in each. The generated code goes through its own helper, and
that they all agree is checked by unit vectors on every run.

## Tables

```rule
table 基本送料(base_fee)
policy unique
| 届け先      | 重量    | -> 基本送料(base) : money[円, incl_tax] |
| 遠隔地      | <=2000g | 1200円                                  |
| 遠隔地      | >2000g  | 1800円                                  |
| not: 遠隔地 | <=2000g | 800円                                   |
| not: 遠隔地 | >2000g  | 1100円                                  |
```

Left of `->` are input columns, right of it output columns. A column may
name an input, a `derive`, a boolean or enum intermediate, **or an output
of an earlier table**.

That last one is how tables stack, and stacking is how a complicated rule
gets written: `table 重さ判定` produces `区分`, which is a column of
`table 帯判定`, whose `帯` is a column of the next table again. There is no
limit on the depth, and one table may produce several output columns.

**There are exactly two policies.**

- **`unique`** (the default) — any overlap is an error. The order
  carries no meaning, so reordering the rows cannot change the answer.
- **`first`** — the earliest matching row wins. It exists to accept the
  way the business actually writes: the exceptions first, the general
  case last.

DMN's Any, Priority and Collect are not adopted, and **completeness
cannot be waived — it is always required**. A table that tolerates a
hole cannot be written.

## What a cell may hold

Seven kinds, and no others.

| written | means |
|---|---|
| `-` | any value. **A blank is a syntax error**: a blank cannot be told from a forgotten entry |
| `1200円` `2000g` `true` `2026-04-01` | equality with a literal. A quantity or an amount **must carry its unit** (a bare `2000` is an error) |
| `北海道, 沖縄県` | a set. Each element is a literal or a group name |
| `not: 遠隔地` | the complement |
| `<=2000g` | comparison — `<=`, `>=`, `<`, `>` |
| `>=1000円 <20000円` | an interval (two comparisons side by side mean "and") |
| `none` | an optional that is absent |

The symbols are ASCII. `→`, `・` and `、` are still read, and `rulec fmt`
rewrites them to `->` and `,`; outside names and cell values, no IME is
needed.

**Range notation with `..` is a syntax error.** "Up to 2000g" does not
say whether the endpoint is included; a comparison operator does. A
boundary stitched together wrongly is caught by the overlap check, with
a witness.

## derive, define, result

**A derive** is a linear combination of inputs only, and **can sit in a
table column while still being a quantity**.

```rule
derive 適用後金額(net) : money[円, incl_tax] = 商品合計 - 割引額  range >=0円 <=100万円
```

Policies that judge on the amount *after* a discount are real ("if the
post-coupon total is at least 3,980 yen…"). If that could not be a
column, the most error-prone subtraction in the whole rule would live as
one bare line on the calling side.

**A define** names a boolean or an intermediate value. A boolean one may
sit in a column.

```rule
define 大口(bulk) : bool = 注文金額 >= 3万円
define Aが早いか同じ(a_earlier) : bool = A期限 <= B期限
```

Its condition holds either one value compared with a constant, or a
comparison of **two values whose difference cannot be subtracted** (two
dates, say). Comparing two numbers directly is refused, with a message
asking you to declare the difference as a `derive` — that way the
analysis is exact.

**A result** assembles an output.

```rule
result 送料 = 基本送料 × 負担率
```

The operations are addition and subtraction, multiplication by a
constant, multiplication by a rate, `min`, `max`, and the five rounding
modes. **There is no loop and no recursion.**

## Examples

```rule
examples
| 届け先 | 重量  | 注文金額 | 会員     | -> 送料 |
| 沖縄県 | 2500g | 40000円  | 一般     | 0円     |
| 東京都 | 1999g | 12000円  | プラチナ | 400円   |
```

`examples` is an **executable specification**. `rulec check` runs every
row through the reference evaluator and reports a failure **with the
rows that fired**.

**Every output must have a column.** Dropping one is an error, and the
reason is worth knowing: the agreement of the reference evaluator, the
generated Python and the generated Go stays green **when all three share
the same mistake**. That happened — rounding for multiple outputs was
missing in all of them at once, and the agreement check was green to
the end. The only thing that can break it is an expectation a person
wrote.

## What cannot be written

- Nested objects (`注文.配送先.都道府県`) — flatten at the boundary and
  pass the scalar in.
- Collections and iteration — a variable number of stacked coupons is
  handled by fixing the rule at "one decision" and leaving the order and
  the repetition to the caller.
- Date arithmetic — comparison and range only.

Allowing any of the three would stop the completeness and overlap checks
from terminating. **What cannot be written is the price of the checks
finishing.**

---

The complete grammar, down to the lexical rules and the reserved words,
is in [Grammar](reference.md).

[What it proves](checks.md){ .md-button .md-button--primary }
[Grammar](reference.md){ .md-button }
