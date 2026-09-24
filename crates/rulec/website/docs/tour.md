# Write a table (.rule)

A `.rule` file **has one fixed shape, read from the top**. There is no
forward reference, so reading downwards is reading the dependencies in
order. **The keywords are English; the names and the cell values stay in
the language of the business** — English in the worked example this page runs on, and
Japanese in the sections that quote a rule transcribed from a Japanese statute, which is the
language it was published in.

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
| `sum` | the total of one column over the elements of a sequence |
| `sequence` | a named list of elements, for an example to walk |
| `table` | a decision table. The body of the language |
| `policy` | that table's hit policy (`unique` or `first`) |
| `overrides` | when a table above defines the same output, says that this table's rows take precedence over it. The line after `policy`; `table:label` names one row |
| `clause` | a one-line rule that does not fit a table, written as a sentence: `when <column> <cell> and …` (`when always` when there is no condition), `then <value>`, and `overrides` when needed |
| `source` | a document the rule transcribes: a law in a statute database (`law [<database>] "<id>" asof <date>`) or a file beside the rule (`file "<file>" sha256:…`). A table, clause, row, derive or define cites it at the end of its line: `@source 第20条` |
| `shape` | the shape of the caller's object, borrowed from the contract it already has (`jsonschema "<file>" "<pointer>"` or `proto "<file>" <Message>`). An input then says `from <shape>.<field>` at the end of its line |
| `apply` | another rule file, applied with its inputs read as this rule's values: `<its input> = <this rule's value>`, `except <definitions not applied>`, `<its output> -> <name>` |
| `result` | assembles an output |
| `examples` | an executable specification |

## A complete rule

This one is `tests/corpus/parcel_rate.rule`: it checks clean as it stands, and the
repository's test suite generates it and runs it in every target language on every commit.
The amounts are made up, but nothing else about it is.

```rule
rule parcel_rate v1
description "A parcel tariff in pounds and inches, written in English. A sketch, not a transcription: the amounts are made up"

# Nothing else in the corpus is priced in USD by weight, and nothing at all reached oz, lb
# or in — an unexercised unit is an unchecked unit (§15.9).

enum size_class = envelope | small | large
enum zone = domestic | canada | overseas

group north_america = domestic, canada

inputs
  weight    : mass[lb]    range >=1lb <=70lb
  girth     : length[in]  range >=1in <=130in
  dest      : zone
  signature : bool

outputs
  fee : money[USD, incl_tax]  round up(1USD)

# One table decides the class and the next one prices it: what the first produces is a
# column of the second.
table size_of
policy first
| girth  | -> size : size_class |
| <=22in | envelope             |
| <=60in | small                |
| -      | large                |

table base_rate
policy unique
| dest          | size     | weight  | -> base : money[USD, incl_tax] |
| north_america | envelope | -       | 6USD                           |
| north_america | small    | <=160oz | 12USD                          |
| north_america | small    | >160oz  | 18USD                          |
| north_america | large    | <=160oz | 22USD                          |
| north_america | large    | >160oz  | 30USD                          |
| overseas      | envelope | -       | 16USD                          |
| overseas      | small    | -       | 38USD                          |
| overseas      | large    | -       | 60USD                          |

# A fuel surcharge is a percentage of the base, which is what the rounding on the output is
# there to settle: 12USD at 5% is 12.60USD, and up(1USD) makes that 13USD.
table fuel_rate
policy unique
| dest          | -> fuel : rate[step 1%] |
| north_america | 5%                      |
| overseas      | 12%                     |

table signature_fee
policy unique
| signature | -> extra : money[USD, incl_tax] |
| true      | 4USD                            |
| false     | 0USD                            |

result fee = base + base × fuel + extra

examples
| weight | girth | dest     | signature | -> fee |
| 5lb    | 10in  | domestic | false     | 7USD   |
| 5lb    | 40in  | canada   | false     | 13USD  |
| 20lb   | 40in  | domestic | true      | 23USD  |
| 5lb    | 10in  | overseas | false     | 18USD  |
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
are no parentheses anywhere — as in the rule above, and in
[Article 7 of Regulation (EC) No
261/2004](examples.md#a-rule-written-in-english--eu-air-passenger-rights), which is
transcribed into the corpus in English, in EUR and km.

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

Ten, and no others.

| type | written | the thing to know |
|---|---|---|
| boolean | `bool` | |
| enum | `size_class` | a **closed** finite set. Declared with `enum` or brought in with `import` |
| quantity | `mass[g]` `length[cm]` `area[m2]` `volume[L]` `duration[h]` | **the unit is part of the type**. `2kg` is sugar for `2000g`; at run time the value is one integer in the declared unit. Mass is `mg g kg t oz lb`, length `mm cm m km in ft yd mi`, area `mm2 cm2 m2 a ha km2 坪 in2 ft2 yd2 mi2 ac`, volume `mm3 cm3 m3 mL L kL`, duration `ms s min h d w`. **Dimensions do not multiply into one another** — an area is its own type, and `縦 × 横` is E103 |
| ordered quantity | `temperature[℃]` `sound[dB]` | comparison and `range` only: **they do not add** (E048). 41℉ is exactly 5℃ and a literal converts between them, but the difference of two temperatures is not a temperature, and a decibel is a logarithm, so two of them added are not two sounds' worth |
| money | `money[円, incl_tax]` `money[USD, excl_tax]` | **branded twice**, by currency and by tax flag. `incl_tax` and `excl_tax` do not add. The currency is `円` or any ISO 4217 code; its hundredth is the code plus `c`, so `money[USD]` counts dollars and `money[USDc]` cents. **Two currencies never convert** — there is no exchange rate here, and mixing them is E103 |
| rate | `rate[step 1%]` `rate` | an integer throughout, counting steps (`10%` is 10 with `rate[step 1%]`). An input declares its step; a computed rate may leave it out, and then it comes from the literals in its column |
| number | `number` | a whole number with no unit — a count of things, a number of days, a score. Dividing money by money in the same currency drops the unit and lands here |
| date | `date` | comparison and range only. **There is no date arithmetic** |
| string | `string` | **cannot be a table column** (E110). Use it for an output, or for an input that only passes through. A value that decides a branch belongs in an `enum` |
| optional | `size_class?` | consumed only by the cell `none` |

Quantities, money, rates and dates are **all integers** — a date is a day
ordinal, a rate a count of steps. No floating point appears anywhere:
how a fraction is settled is decided by `round` below, never by the
host language's division.

**Enums are closed, and there is no open enum.** That is the point: when
a value is added, every table that has not accounted for it breaks the
completeness check.

```rule
enum member_tier = basic default | gold default | platinum
```

`default` declares "this value needs no row of its own; being caught by
a `-` row is correct". Without it, a value no row names is reported.

A **group** is a named subset of an enum and may be used in a cell
wherever a value may. Groups are always expanded before checking, so a
hole in a table written with groups is still found.

```rule
group north_america = domestic, canada
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
import proto "api/v1/order.proto" MemberTier -> member_tier
enum member_tier = basic | gold | platinum default
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
import jsonschema "api/openapi.json" "#/components/schemas/MemberTier" -> member_tier
enum member_tier = basic | gold | platinum default
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
  weight    : mass[lb]    range >=1lb <=70lb
  girth     : length[in]  range >=1in <=130in
  dest      : zone
  signature : bool

outputs
  fee : money[USD, incl_tax]  round up(1USD)
```

There may be several outputs. They become a `NamedTuple` in Python, an
`interface` in TypeScript, a plain object in JavaScript, a `Struct` in
Ruby, a final class in PHP, a record in Java, a struct in Rust, Swift and Go, one array per
output in NumPy, one column
each in SQL, and the keys of the
answer's `observed` object from the Wasm module — and **rounding applies once per
output**.

```rule
outputs
  accepted : bool
  raw_fee  : money[USD, excl_tax]  round down(1USDc)
```

An output returns **the binding of its own name** — a `define` or a table
output column called `raw_fee` is what the output `raw_fee` returns. `result`
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

What is in the parentheses is the **grid**: `up(10USDc)` rounds to a
multiple of ten cents, so −4.2 cents becomes −10 cents.

The negative direction is pinned because **integer division in Python and
Ruby rounds toward −∞ while in Rust, Swift, Go, Java, TypeScript, JavaScript, NumPy,
PHP's `intdiv`, SQL and the Wasm module it truncates toward zero**. Left to the host language, one rule
would answer differently in each. The generated code goes through its own helper, and
that they all agree is checked by unit vectors on every run.

## Inputs taken from the caller's object: shape and from

A rule's inputs are flat values, and what the caller holds is usually a nested object — an
API request, a message on a queue. When its shape is already described by a JSON Schema or a
`.proto`, `shape` borrows that description, and a `from` at the end of an input's line says
where in it the value stands.

```rule
shape order = jsonschema "contracts/order.schema.json" "#/$defs/Order"

inputs
  dest    : zone    from order.shipping.zone
  chilled : bool    from any order.lines where chilled = true
  lines   : number  range >=1 <=50  from count order.lines
```

A `.proto` is named by its file and message:
`shape shipment = proto "contracts/shipment.proto" shop.v1.CreateShipmentRequest`.

`from` comes in four shapes:

| written | what it yields |
|---|---|
| `from order.shipping.zone` | the value of that field |
| `from any order.lines where chilled = true` | `bool` — whether some element passes |
| `from all order.lines where chilled = true` | `bool` — whether every element does |
| `from count order.lines` | `number` — how many elements there are (with `where`, how many pass) |

Four things follow.

- **The code that reads the inputs out is generated.** Beside the rule's own function,
  `rulec gen` writes a second one that takes the whole order object. If the rule's function
  is `order_shipping`, this one is `order_shipping_from(order)`: it reads the inputs out as
  the `from`s say, calls the rule's function and returns its answer, so the caller hands it
  the order as it is. It is written for Python, TypeScript, JavaScript, Ruby and PHP. In
  each of them, parsed JSON is usually used as it comes, as a plain map (a `dict` in Python,
  a `Hash` in Ruby), so the function takes a plain map too. Go, Swift, Java, Rust, SQL,
  NumPy and Wasm do not get it. From a `.proto`, it reads the JSON protojson writes: a field
  under its lowerCamelCase name or its `.proto` name, and one left out as its proto default.
  How to call it, and why the other seven do not get it, are in [Generate and call](generate.md).
- **The paths are held to the contract.** Every `rulec check` reads the contract's file. A
  path it does not have is E121, which says how far the path got and which fields were
  there; a type that does not fit is E120; a `shape` no input reads from is W122. A field
  the contract renames stops CI instead of raising a `KeyError` in production.
- **The contract's validation is held to the inputs' declarations.** A value the contract
  lets through and an input refuses is E122: without a `maxItems` on the contract's `lines`,
  an order of 51 lines passes the contract, and `lines`, declared `range >=1 <=50`, refuses
  it. `fix.text` is the annotation or keyword to add to the contract. A row reached only by
  values the contract never lets through is W123.
- **The contract's conditions across fields are held to the rule.** A CEL expression on the
  message, a `oneof` and JSON Schema's combinators relate fields to each other. A
  `constraint` the contract does not keep is E123, and a row asking for a combination the
  contract never lets through is W124.

No check of the table changes. What comes out of a projection is a scalar input like any
other, and completeness and overlap are decided as they would be without `from`.

**One collection, and a unary test on a field of an element, is as far as it goes.** A join,
a nested quantifier and a path inside a cell cannot be written: the cell language is where
this tool's boundary is.

[Examples](examples.md) has two worked rules with their contracts beside them, one on a JSON
Schema and one on a `.proto`; the details are in the
[grammar](reference.md#33-shape-and-from--where-the-callers-object-holds-an-input).

## Tables

```rule
table base_rate
policy unique
| dest          | size     | weight  | -> base : money[USD, incl_tax] |
| north_america | envelope | -       | 6USD                           |
| north_america | small    | <=160oz | 12USD                          |
| north_america | small    | >160oz  | 18USD                          |
| north_america | large    | <=160oz | 22USD                          |
| north_america | large    | >160oz  | 30USD                          |
| overseas      | envelope | -       | 16USD                          |
| overseas      | small    | -       | 38USD                          |
| overseas      | large    | -       | 60USD                          |
```

Left of `->` are input columns, right of it output columns. A column may
name an input, a `derive`, a boolean or enum intermediate, **or an output
of an earlier table**.

That last one is how tables stack, and stacking is how a complicated rule
gets written: in the rule above, `table size_of` produces `size`, which is
a column of `table base_rate`. There is no limit on the depth, and one
table may produce several output columns.

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
| `12USD` `160oz` `true` `2026-04-01` | equality with a literal. A quantity or an amount **must carry its unit** (a bare `160` is an error) |
| `domestic, canada` | a set. Each element is a literal or a group name |
| `not: north_america` | the complement |
| `<=160oz` | comparison — `<=`, `>=`, `<`, `>` |
| `>=10USD <200USD` | an interval (two comparisons side by side mean "and") |
| `none` | an optional that is absent |

The symbols are ASCII. `→`, `・` and `、` are still read, and `rulec fmt`
rewrites them to `->` and `,`; outside names and cell values, no IME is
needed.

**Range notation with `..` is a syntax error.** "Up to 2000g" does not
say whether the endpoint is included; a comparison operator does. A
boundary stitched together wrongly is caught by the overlap check, with
a witness.

## derive, define, result

A table holds the branching and nothing else. The arithmetic lives in three places
outside it.

| | what it may hold | can it be a column? |
|---|---|---|
| `derive` | a linear combination of inputs — `+`, `-`, multiplication by a constant | **yes**, and it stays a quantity |
| `define` | a boolean (two shapes), or a computed intermediate value | a boolean or an enum one can |
| `result` | `+ - * /`, parentheses, `min` and `max`, and the five rounding modes as functions | — |

A rate can be multiplied in (`base_fee × pay_rate`); it stays a rate to the end and the
rounding happens once. **Everything is an integer — no floating point anywhere.**

**A derive** is a linear combination of inputs only, and **can sit in a
table column while still being a quantity**.

```rule
derive net : money[USD, incl_tax] = subtotal - discount  range >=0USD <=10000USD
```

Policies that judge on the amount *after* a discount are real ("if the
post-coupon total is at least 3,980 yen…"). If that could not be a
column, the most error-prone subtraction in the whole rule would live as
one bare line on the calling side.

**A define** names a boolean or an intermediate value. A boolean one may
sit in a column.

```rule
define bulk : bool = subtotal >= 300USD
define a_earlier : bool = a_due <= b_due
```

Its condition holds either one value compared with a constant, or a
comparison of **two values whose difference cannot be subtracted** (two
dates, say). Comparing two numbers directly is refused, with a message
asking you to declare the difference as a `derive` — that way the
analysis is exact.

**A result** assembles an output.

```rule
result fee = base + base × fuel + extra
```

The operations are addition and subtraction, multiplication by a
constant, multiplication by a rate, `min`, `max`, `allocate`, and the
five rounding modes. **There is no loop and no recursion.**

**`allocate` hands an amount out over a run of lines**, in the ratio of
their prices. It is the one place a rule divides by something that is not
a constant.

```rule
constraint price_upto <= price_total

derive share_upto : money[USD] = allocate(discount_total, price_upto, price_total)  range >=0USD <=10000USD

result share = share_upto - share_before
```

Each line gets the share up to it minus the share up to the line before,
so the odd yen lands on the last line and **the parts add up to the
amount exactly** — which `proofs/` states and proves rather than leaving
to the examples. It asks for three names with declared ranges, none of
them negative, a positive whole, and the `constraint` above; anything
missing is E117.

## Something complicated is written by stacking tables

Because a cell can only see its own column, **tables stack as deep as you like**. What one
table produces is written as a column of the next.

<div class="rc-overview" markdown>
![What one table produces is a column of the next: band_of turns the distance and whether the flight is intra-EU into a band, and amount turns that band into the compensation. Not every table is in the chain — reduction reads the rule's inputs directly — and result puts the two together](images/stack.svg?v=9abfb225#only-dark)
![What one table produces is a column of the next: band_of turns the distance and whether the flight is intra-EU into a band, and amount turns that band into the compensation. Not every table is in the chain — reduction reads the rule's inputs directly — and result puts the two together](images/stack-light.svg?v=9abfb225#only-light)
</div>

What to look at is **the word that appears twice**. `band` leaves the first table and arrives
as a column of the second. Not every table is in the chain: `reduction` reads the rule's
inputs directly, because Article 7(2) restates the distance conditions rather than referring
back to them.

Depth costs no visibility. When a check fails it names the row that fired in each table.

```
Fired rows: table band_of row 4 / table amount row 3 / table reduction row 8
```

Four things matter when stacking.

| | |
|---|---|
| **A table's output is a column of any later table** | There is no limit on the depth; only the check's budget stops it, at E109 |
| **One table may produce several output columns** | one table above produces the fee and the rate that scales it at once |
| **A `derive` can be a column** | "Judge on the amount after the discount" becomes one column instead of one bare line of arithmetic |
| **Completeness is checked across the stack** | The second form of E102 is "the upstream table never emits that value" |

A rule that does this, and runs, is [Examples](examples.md) → "Three tables stacked, two
outputs returned".


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

A rule transcribed from a published policy or a statute can say where each part came from:
`source` declares a document, and `@source` at the end of a table, clause, row, derive or define
line cites it.

```rule
source 郵便 = file "ゆうパック基本運賃.pdf" sha256:9e4edb5b6a1c0f42
source 措置法 = law "332AC0000000026" asof 2026-04-01
  第91条 sha256:85faf53f6f6e8196
source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269

define 軽減期間(reduced) : bool = 作成日 <= 2027-03-31  @措置法 第91条

table 運賃表(fee_table)  @郵便
table distance          @osha "§1910.157"
```

There are two kinds of document, cited and copied a little differently.

| Document | Declared as | Cited as | Its copy |
|---|---|---|---|
| **A file beside the rule** (a policy PDF, a tariff sheet, a company rule in Word) | `source 郵便 = file "<file>" sha256:<digest>` | `@郵便`, or **`@郵便 表1` to say which table of it was transcribed** | the file itself; `rulec source pin` writes its digest on the `source` line, and a cited table is taken out of the document and kept beside it |
| **A statute** | `source 法 = law [<database>] "<id>" asof <date>`, the date saying which text is meant | always with the fragment, named the way that database names one | `rulec source fetch` brings each cited fragment into `sources/` beside the rule; `rulec source pin` writes each copy's digest on the line under `source` |

**Two statute databases**, and the word after `law` says which.

| word | the database | the id | a fragment |
|---|---|---|---|
| (none), or `egov` | e-Gov, the Japanese government's statute database | `342AC0000000023` | `第91条`, `第20条の2第3項`, `別表第一`, `附則第3条`, an amending law's as `附則（令和七年三月三一日法律第一三号）第3条` |
| `ecfr` | the Electronic Code of Federal Regulations — US federal regulations as in force on a date | a title and a part, `29 CFR 1910` | a section, `§1910.157` |

A fragment the language cannot read as one word is quoted, in the citation and on the pin line
alike: `@osha "§1910.157"`. A paragraph of a CFR section (`(d)(2)`) is not addressed yet, because
the eCFR serves a section at a time.

From then on every `rulec check` confirms that the copies are there and that their digests
are what the rule says. When a copy differs — the file was replaced, or the article was
fetched again after an amendment — the check stops and names the tables, clauses and rows
that cite it (E038), which is all there is to reread. `check` itself never reads the network.

`rulec source outdated` asks whether the original moved on. For a statute it asks the database
whether an amendment after `asof` changes the text of a cited fragment — e-Gov by the revision's
enforcement date, the eCFR by the amendment date of that very section, and in both a re-issue
that only moved the markup does not count; for a file with a `url "…"`, it looks where the file came from and says **whether a
cited table changed, or only something this rule does not transcribe**. `check` cannot know of
an amendment until the copy is fetched again, so this belongs in a scheduled CI job.

### Cite a table and its amounts are held to the copy

`@郵便 表1` on a file source makes `rulec source fetch` take that table out of the document and
write it beside it. A sheet is a table in a workbook (`.xlsx`), a table is a table in a Word
file (`.docx`), and Markdown and CSV are what they look like. A PDF or a scan cannot be read
here: hand an extractor (docling and the like) to `rulec source fetch --via <cmd>`, or cite the
document whole as `@郵便`.

What the copy then holds is **the amounts the table writes**.

```console
$ rulec check rules/shipping_fee.rule
error[E116]: The amount of row 4 is not in the copy it cites
   |
24 | | not: 遠隔地 | >2000g  | 1000円      |
   |                           ^^^^^^ not in the copy: 1000円
   |
 The copy cited: 規約 表1

warning[W120]: The copy of 表1 states values no row uses
 Stated in the copy, used by no row: 1100円
```

A mistyped digit brings out both halves at once, and W120 alone catches **a row that was never
transcribed**. Only amounts are compared: a threshold is rewritten as it is transcribed
(`1,949,000円まで` becomes `<=1949000円`) and an amount is not.

The approver's page quotes the cited text, or the cited table, from the copies.

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
`rulec diff` shows which inputs of this rule move — with past records, how many of them and by how much; once that is accepted,
`rulec source pin` writes the new digest. Rows of the applied rule's tables that this rule's
ranges never reach are not errors: the approver's page lists them as unused by this apply, and
only a table none of whose rows is reached draws W118.

The generated code carries the applied rule expanded, and the trace says
`{"table":"退職手当:支給表","row":1,"label":"短期"}`. An apply goes one level, and a rule
that walks a sequence cannot be applied (E044).

## Saying which combinations cannot happen

A relation between two inputs, guaranteed by the caller.

```rule
constraint valid_from <= valid_to
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

**A count counts and a `sum` adds one column up** — `sum 合計(total) over 明細
of 金額`. An average does not follow: dividing by a count is dividing by a
variable, so it belongs before the call, as a value. A rule cannot hold both a
`fold` and a `count` or `sum` (E031): two endings for one walk, and a fold may
stop partway.

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

- Nested objects (`order.destination.state`) — flatten at the boundary and
  pass the scalar in.
- Iteration anywhere you like, and recursion — a sequence is walked once,
  by `fold` (the section above); every other repetition, a stack of
  coupons applied in order among them, stays with the caller.
- An average over the elements — it divides by how many there are, which is
  dividing by a variable. Compute it before the call and pass it in (**the
  count and the total are written with `count` and `sum`**).
- Date arithmetic — comparison and range only.

Allowing these would stop the completeness and overlap checks from
terminating. **What cannot be written is the price of the checks
finishing.**

---

The complete grammar, down to the lexical rules and the reserved words,
is in [Grammar](reference.md).

[What it proves](checks.md){ .md-button .md-button--primary }
[Grammar](reference.md){ .md-button }
