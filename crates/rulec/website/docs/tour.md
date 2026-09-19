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
| `import` | brings in the values of an enum (built-in, a `.proto`, or a JSON Schema) |
| `enum` | a closed enumeration |
| `group` | a named subset of an enum |
| `inputs` | the rule's arguments |
| `elements` | the fields of one element of a sequence the rule walks |
| `outputs` | its results |
| `derive` | a linear combination of inputs — **the only intermediate that can sit in a table column while still being a quantity** |
| `define` | a boolean, or a computed intermediate |
| `constraint` | a relation between inputs: which combinations **cannot happen** |
| `fold` | reduces a column of per-element verdicts to one answer |
| `count` | how many elements of the sequence meet one test |
| `sequence` | a named list of elements, for an example to walk |
| `table` | a decision table. The body of the language |
| `policy` | that table's hit policy (`unique` or `first`) |
| `overrides` | when a table above defines the same output, says that this table's rows take precedence over it. The line after `policy`; `table:label` names one row |
| `clause` | a one-line rule that does not fit a table, written as a sentence: `when <column> <cell> and …` (`when always` when there is no condition), `then <value>`, and `overrides` when needed |
| `source` | a document the rule transcribes: a law on e-Gov, the Japanese government's statute database (`law "<law id>" asof <date>`) or a file beside the rule (`file "<file>" sha256:…`). A table, clause, row, derive or define cites it at the end of its line: `@source 第20条` |
| `apply` | another rule file, applied with its inputs read as this rule's values: `<its input> = <this rule's value>`, `except <definitions not applied>`, `<its output> -> <name>` |
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

## Imports

Two lines start with `import`, and both bring in **the values of an
enum** — nothing else crosses a file boundary. A rule stays one file:
what arrives is a set of names, not rows and not amounts.

| line | what it brings | who owns the set |
|---|---|---|
| `import std/<name>` | a built-in enum | rulec, frozen |
| `import proto "<file>" <Enum> -> <enum of this rule>` | the values of an enum in a `.proto` | that `.proto`, outside this rule |
| `import jsonschema "<file>" "<pointer>" -> <enum of this rule>` | the values of an enum in a JSON Schema (OpenAPI included) | that file, outside this rule |

!!! note "`rulec import` is a different thing that shares the word"

    `rulec import csv` and `rulec import xlsx` are a **command**: they
    write a first draft of a `.rule` from a spreadsheet, once, and leave
    no line in the file. The two lines above are read again on every
    `rulec check`.

### Built-in enums

The built-in `std/都道府県` (47 values) arrives with
`import std/都道府県`.

### When the set belongs to somebody else

An enum like a member tier or a status is usually declared in a
`.proto`, and whether it gains a value is decided outside this rule. Say
where the set comes from, and the two are held together.

```rule
import proto "api/v1/order.proto" MemberTier -> 会員区分
enum 会員区分(tier) = 一般(basic) | ゴールド(gold) | プラチナ(platinum) default
```

The `.proto` owns **which values exist**; the `.rule` owns **what they
are called here and what each one costs**. A proto carries no Japanese,
so the names are yours to decide. Every `rulec check` reads that file and
holds the two together.

- A value on one side only is **E032**. It is usually the proto that
  gained one, and **on the wire that is a compatible change**.
- Once the sets agree, a value that no row names and no `default` marks
  is **E033**. For an enum you wrote yourself that state is a warning
  (W111); for an imported one it is an error, because the value arrived
  through a change nobody has read yet.

A table with a `-` row passes the completeness check when a new value
turns up, and the value quietly takes the default amount. That is what
E033 stops.

### From a JSON Schema or an OpenAPI document

The same binding, written with a JSON Pointer, because one document holds
hundreds of enums:

```rule
import jsonschema "api/openapi.json" "#/components/schemas/MemberTier" -> 会員区分
enum 会員区分(tier) = 一般(basic) | ゴールド(gold) | プラチナ(platinum) default
```

The pointer may land on the schema or on its `enum` array, and an enum
written inline in a property is reached the same way. The values become
the aliases **exactly** — nothing taken off the front, no case folded —
because a schema has no naming convention to earn a transformation from.

**YAML is not read.** A reader for the subset one file happens to use is
a reader that goes wrong quietly on the next one. Point at a JSON form of
the document; most toolchains can write one.

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
Ruby, a struct in Rust, Swift and Go, one column each in SQL, and the keys of the
answer's `observed` object from the Wasm module — and **rounding applies once per
output**.

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
Ruby rounds toward −∞ while in Rust, Swift, Go, TypeScript, JavaScript,
SQL and the Wasm module it truncates toward zero**. Left to the host language, one rule
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

## A main rule and its exceptions as two tables

A tariff usually comes with a main rule and an exception that takes precedence over it:
"a contract for more than 100,000 yen is taxed at the reduced rate until 31 March 2027". It
can be written as one table with a `軽減期間` column, but when the sources are two — the
appendix table of the Stamp Tax Act and Article 91 of the Special Taxation Measures Act — two
tables read better against them.

```rule
table 本則(base)  @法 別表第一
policy unique
   | 金額の記載あり | 契約金額          | -> 印紙税額(tax) : money[円] |
r1 | false          | -                 | 200円                        |
r3 | true           | >=1万円 <=10万円  | 200円                        |
r4 | true           | >10万円 <=50万円  | 400円                        |
r5 | true           | >50万円 <=100万円 | 1000円                       |

table 軽減(reduced_rate)  @措置法 第91条
policy unique
overrides 本則
| 軽減期間 | 金額の記載あり | 契約金額          | -> 印紙税額 |
| true     | true           | >10万円 <=50万円  | 200円       |
| true     | true           | >50万円 <=100万円 | 500円       |
```

`overrides 本則` declares that the rows of this table take precedence over the rows of
table 本則 (an excerpt; the real tables have more rows). `r1` at the head of a row is a label,
which is how a single row is named: `overrides 本則:r4`.

The checks treat the tables that define one output **as one set**. Completeness is judged over
both together, and when there is a hole, which table gets the row is a person's decision. Where
rows meet, an `overrides` line settles it and the overlap passes; without one it is E105. A row
of the main table that the exception covers entirely is E102, and an `overrides` line whose rows
meet none of the other's is W117.

The generated code tries the later table first and takes the first row that applies. The trace
names the table the row was written in and its position there, plus the label when it has one.
The approver's page says, in one sentence, "table 軽減 takes precedence over table 本則; in
all 10 pairs that meet, the rows of 軽減 lie inside the other's (an exception)".

## A rule written as a sentence: clause

A one-line rule whose conditions do not line up as columns — a proviso, typically — is not
forced into a table. It is a `clause`.

```rule
clause 通常(regular) -> 送料  # Article 3(1), the main text
  when always
  then 基本運賃

clause 無料(free) -> 送料  # Article 3(2), the proviso
  when 注文金額 >=3900円 and 会員 true
  then 0円
  overrides 通常
```

`when` joins `<column> <cell>` pairs with `and`, the cell being any of the seven kinds a table
cell may hold. A rule with no condition writes `when always` (so that a forgotten line cannot be
mistaken for one, the same reason a blank cell is refused). `then` holds what an output cell
holds: a literal or a name.

A clause is treated as **a table with one row**: checked, generated and traced by the same
machinery, firing as `{"table":"無料","row":1}`. It mixes with tables through `overrides`.

## Where it was transcribed from: source and @

A rule transcribed from a law or a published policy can say where each part came from.

```rule
source 法 = law "342AC0000000023" asof 2026-04-01
  別表第一 sha256:0ba69792e960021e
source 措置法 = law "332AC0000000026" asof 2026-04-01
  第91条 sha256:85faf53f6f6e8196

define 軽減期間(reduced) : bool = 作成日 <= 2027-03-31  @措置法 第91条

table 本則(base)  @法 別表第一
```

`source` declares a document: for a law, its id on e-Gov (the Japanese government's statute
database) and the date whose text is meant
(`asof`); for a file beside the rule, `file "料金表.pdf" sha256:…`. A table, a clause, a row, a
derive or a define cites it at the end of its line, `@source 箇所`, naming the place the way the
document does: `第20条`, `第20条の2`, `第20条第2項第3号`, `別表第一`.

`rulec source fetch` brings a copy of every cited place from e-Gov into `sources/` beside the
rule, and `rulec source pin` writes the digest of each copy on the line under `source`. From
then on every `rulec check` confirms that the copies are there and that their digests are what
the rule says. When a refreshed copy differs, the check stops and names the tables, clauses and
rows that cite that place (E038), which is all there is to reread. `check` itself never reads
the network; whether a later amendment changes a cited place is what `rulec source outdated`
asks e-Gov, and it belongs in a scheduled CI job.

The approver's page quotes the cited text from the copies.

## Applying another rule: apply

"The provisions of Article 20 apply to part-time staff. In this case, 'years of service' shall
be read as 'period in office'." A statute written this way is saying that the rule of Article
20 is used once more with its inputs replaced. `apply` writes exactly that.

```rule
apply 退職手当(retirement) = "退職手当.rule" sha256:b58648ea2767ebbd  # Article 31
  勤続年数 = 在職期間
  退職事由 = 任期終了事由 with 任期満了 -> 定年, 辞職 -> 自己都合
  基本給 = 報酬月額
  except 減額
  手当 -> 非常勤手当
```

The heading names the rule file being applied and the digest of that file. The lines under it
are the substitutions: every input of the applied rule, without exception, and what this rule
passes for it (an input, a derive, a define, an earlier table's output, or a literal). Two enums
are matched value by value with `with`; a value spelled the same on both sides needs no entry.
`except` leaves definitions of the applied rule out ("Article 20 (excluding paragraph 2)").
The applied rule's outputs become values of this rule, renamed with `->`.

`rulec check` first checks the applied rule whole, on its own ground, then expands its tables
and clauses into this rule under names like `退職手当:支給表` and checks the result as one
rule — so completeness and overlaps are proved on the rule as applied. Three things more are
checked: that no substitution is missing (E041), that the types agree (E042), and that **what
this rule passes stays inside the applied rule's ranges** (E043). Declare the period in office
from 0 and it stops with "在職期間 = 0 is outside range >=1 <=40 of 勤続年数 in 退職手当.rule":
the applied rule's completeness was proved over that range and no further, and whether to narrow
the range or to define the excess in a clause of this rule is a business decision.

When the applied rule is amended, the digest no longer matches and the check stops with E040.
`rulec diff` shows how many answers of this rule move and by how much; once that is accepted,
`rulec source pin` writes the new digest. Rows of the applied rule's tables that this rule's
ranges never reach are not errors: the approver's page lists them as unused by this apply, and
only a table none of whose rows is reached draws W118.

The generated code carries the applied rule expanded, and the trace says
`{"table":"退職手当:支給表","row":1,"label":"短期"}`. An apply goes one level, and a rule
that walks a sequence cannot be applied (E044).

## Saying which combinations cannot happen

A relation between two inputs, guaranteed by the caller.

```rule
constraint 適用開始日 <= 適用終了日
```

It computes nothing. It says **which combinations of inputs can happen**,
and three things follow from the one line.

- **The completeness check stops demanding rows for what cannot happen.**
  A table covering everything reachable is complete.
- **A witness becomes a case somebody could really send.** Every input the
  checks construct satisfies the constraints.
- **The generated code refuses a violating input at the door.** The proof
  assumed the constraint, so the code has to insist on it.

The shape is `constraint <input> <comparison> <input>`, with one of `<=`,
`<`, `>=`, `>`, an input on each side, and both of a type that has an
order — money, a quantity, a rate, a `number` or a `date`. Several lines
hold at once, and `A = B` is the two lines `A <= B` and `A >= B`. An
example that breaks a constraint is an error (E019), not a case.

## Taking a sequence whose length is not fixed

Every rule so far took a fixed number of values and decided once. When the
case carries **a sequence instead** — the rows of a tariff sheet, the
candidates a filter left — `elements` declares what one element carries
and `fold` declares how the walk ends.

```rule
elements 運賃行(fee_rows)
  行ゾーン(row_zone) : ゾーン区分
  閾値(threshold)    : money[円, incl_tax]  range >=0円 <=100万円
  行運賃(row_fee)    : money[円, incl_tax]  range >=0円 <=10万円

table 行判定(row_of)
policy unique
| 行ゾーン | 閾値     | -> 採用(verdict) : 採用区分 |
| 近畿圏   | <=1000円 | 確定                        |
| …

fold 採用 over 運賃行
  スキップ  -> next
  打ち切り  -> stop with 0円
  確定      -> take_unique 行運賃
  持ち越し  -> keep_max 行運賃 by 閾値
  empty     -> 0円
  exhausted -> held
```

What judges an element is **an ordinary table**. One element is one case,
so completeness, overlap and units are proved over it as they always were,
and the fields are declared exactly like `inputs`, ranges and units
included. The one difference is that the caller fills them in once per
element.

What `fold` adds is a single line per verdict: what the walk does next.

| arm | what it does |
|---|---|
| `next` | leave this element, look at the next |
| `stop` | end the walk; the answer is what `exhausted` says |
| `stop with <value>` | end the walk with this answer |
| `take_unique <value>` | take this element's value; a second element that also takes is a run-time error |
| `take_first <value>` | take the first, ignore any later one |
| `keep_max <value> by <key>` | hold this element's value, replacing what is held when the key is larger |
| `empty -> <value>` | the answer when there are no elements. **Required** |
| `exhausted -> <value>` | the answer when the walk reached the end. **Required**; `held` is the value being held |

**Every verdict the table can produce needs an arm** (E024): the table's
own completeness check, applied to the fold. An arm for a verdict nothing
can reach is W115.

None of this costs the checks their termination. The table is complete and
unique, so every element lands on exactly one verdict, the sequence
becomes a string of verdicts, and how the walk reads them is a matter of
finitely many states — independent of how many elements arrive at run
time.

An example **names the sequence**, because a cell holds one value.

```rule
sequence 近い一件(near)
| 行ゾーン | 閾値   | 行運賃 |
| 近畿圏   | 500円  | 800円  |
| 近畿圏   | 2000円 | 1500円 |

examples
| 運賃行   | -> 運賃 |
| 近い一件 | 800円   |
```

A `sequence` with no rows is the example for a sequence with nothing in
it. What comes out is the same function with one more argument — for
every target **but SQL**, where one query has no place to carry a value
from row to row and stop partway.

A rule that runs is in [Examples](examples.md), under "A sequence walked
into one answer".

### Counting instead: `count`

A `fold` turns a sequence into **one answer**. A `count` turns it into **one
number** and hands the rule back to the tables.

```rule
count 一致数(hits) over 候補 where 照合結果 = 一致  range >=0 <=50
```

What `where` names is **a column of one element** — a field, or a column a
per-element table produces — whose values are a closed set. For an enum,
`= <value>` says which one to count; a bool column needs nothing after it
(`where 冷蔵品`).

From there the count is a `number`, so **it can be a column**.

```rule
| 一致数 | 自動確定可 | -> 手続き(action) : 次の手 |
| 0      | -          | 新規登録                   |
| 1      | true       | 自動確定                   |
| 1      | false      | 目視確認                   |
| >=2    | -          | 目視確認                   |
```

That is why `count` exists beside `fold`: **when an ordinary table turns the
number into a decision, the boundaries of that decision are checked** — a gap
or an overlap between `0`, `1` and `>=2` stops the rule as any other would.

**The `range` is required and says two things**: the universe the completeness
check quantifies over, and **the cap on the sequence**. A longer sequence is
refused at the door by the generated code, for the reason a number outside its
range is — the proof was made over what was declared.

**Nothing accumulates across elements.** A count counts; there is no sum and no
average, and one belongs before the call, as a value. A rule cannot hold both a
`fold` and a `count` (E031): two endings for one walk, and a fold may stop
partway.

A rule that runs is in [Examples](examples.md), under "Counting a sequence, and
deciding from the count".

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
- Iteration anywhere you like, and recursion — a sequence is walked once,
  by `fold` (the section above); every other repetition, a stack of
  coupons applied in order among them, stays with the caller.
- Adding up across elements — a sum or an average carries a value from one
  element to the next. Compute it before the call and pass it in (**a count
  is written with `count`**).
- Date arithmetic — comparison and range only.

Allowing these would stop the completeness and overlap checks from
terminating. **What cannot be written is the price of the checks
finishing.**

---

The complete grammar, down to the lexical rules and the reserved words,
is in [Grammar](reference.md).

[What it proves](checks.md){ .md-button .md-button--primary }
[Grammar](reference.md){ .md-button }
