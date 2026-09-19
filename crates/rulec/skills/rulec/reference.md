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
import proto "<file>" <Enum> -> <enum of this rule>
import jsonschema "<file>" "<pointer>" -> <enum of this rule>
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
comments may appear anywhere, including at the end of a table row. A comment at the end of a
declaration, of a `table` line or of a row is shown to the approver by `rulec doc`, which is
where the source a table or a row was transcribed from belongs.

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
| unit | mass `mg` `g` `kg` `t` `oz` `lb` · length `mm` `cm` `m` `km` `in` `ft` `yd` `mi` · rate `%` · money `円` `銭`, or an ISO 4217 code, whose hundredth is that code plus `c` (`USD` and `USDc`). **Two currencies never convert**: there is no exchange rate here, so mixing them is E103 |

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
| mass | `mass[g]`, `mass[lb]`, … | the unit is part of the type. `mg` `g` `kg` `t` `oz` `lb` |
| length | `length[cm]`, `length[in]`, … | `mm` `cm` `m` `km` `in` `ft` `yd` `mi` |
| money | `money[円, incl_tax]`, `money[USD, excl_tax]` | currency **and** tax flag are both part of the type. Any ISO 4217 code, or `円`; the hundredth of a currency is its code plus `c`, so `money[USD]` counts dollars and `money[USDc]` counts cents. **Two currencies never convert** — there is no exchange rate here, and mixing them is E103 |
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

## 3.1 import

Two lines start with `import`, and both bring in **the values of an enum** — nothing else
crosses a file boundary. A rule is still one file: what is imported is a set of names, not
rows, not amounts, not another rule.

| line | what it brings | who owns the set |
|---|---|---|
| `import std/<name>` | a built-in enum | rulec, frozen |
| `import proto "<file>" <Enum> -> <enum of this rule>` | the value set of an enum in a `.proto` | that `.proto`, outside this rule |
| `import jsonschema "<file>" "<pointer>" -> <enum of this rule>` | the value set of an enum in a JSON Schema, OpenAPI included | that file, outside this rule |

> `rulec import csv` and `rulec import xlsx` are a different thing that shares the word: a
> **command** that writes a first draft of a `.rule` from a spreadsheet, once. It leaves no
> line in the file and nothing is read again afterwards. These two lines are read on every
> `rulec check`.

### Built-in enums

`import std/都道府県` brings in the 47 prefectures. It is the only built-in today (E013 for
anything else).

### An enum a `.proto` owns

```rule
import proto "api/v1/order.proto" MemberTier -> 会員区分
enum 会員区分(tier) = 一般(basic) | ゴールド(gold) | プラチナ(platinum) default
```

When the values come from a service contract, the set is not the rule's to decide. The line
above says where it comes from, and `rulec check` reads that file on every run and holds the
two together. The path is followed from the directory of the `.rule`.

The two sides carry different things, and neither can be derived from the other. The `.proto`
owns **which values exist**; the `.rule` owns **what they are called here** and what each one
costs — a proto has no Japanese in it. So what is checked is that they agree:

- the value names are matched by the ASCII alias, with the enum's own name taken off the front
  (`MEMBER_TIER_GOLD` is `gold`), which is the prefix convention `buf lint` enforces;
- the zero value is proto3's "not set" when it is named `…_UNSPECIFIED`, so it is not a value
  the table answers for — the generated code refuses it at the entry like any other non-member.
  A zero value named anything else is a value like any other;
- a value on one side only is **E032**, in either direction;
- and once the sets agree, a value that no row names and no `default` marks is **E033**.

E033 is the reason this exists. Adding a value to an enum is a compatible change on the wire,
so the tools that guard the contract let it through; a table with a `-` row then passes the
completeness check, and the new tier quietly takes the default amount. For a value you wrote
yourself that state is a warning (W111). For a value that arrived through the contract it is an
error, because nobody has read it yet. Marking it `default` is how you say you did.

### An enum a JSON Schema owns

```rule
import jsonschema "api/openapi.json" "#/components/schemas/MemberTier" -> 会員区分
enum 会員区分(tier) = 一般(basic) | ゴールド(gold) | プラチナ(platinum) default
```

The same binding, for the other place a value set is declared. Everything about E032 and E033
is the same; two things differ.

**How the enum is named.** One document holds hundreds of enums, so one is named by a JSON
Pointer rather than by a name. The pointer may land on the schema (its `enum` is then read) or
on the array itself, and a leading `#` and a leading `/` are both optional. An enum written
inline in a property — the usual shape in OpenAPI — is reached the same way
(`#/components/schemas/Order/properties/status`). A pointer that does not resolve comes back
with the keys that were there.

**How the values map.** They are the aliases **exactly**: nothing is taken off the front and
no case is folded. A `.proto` earns its transformation from a convention `buf lint` enforces;
a schema has no such convention, and inventing one would mean a rule and a schema that look
like they agree while sending different strings. An enum of numbers or nulls is refused by
name: the values of a rule's enum are names.

**YAML is not read.** It is the usual spelling of an OpenAPI document, and the reason for the
refusal is not that a reader would be hard to start: a reader for the subset one file happens
to use is a reader that goes wrong quietly on the next one, and a value set that is quietly
wrong is the one thing this must never be. Point at a JSON form of the document, which most
toolchains can write.

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
message states the interval to widen to. A rate input may leave `range` out: it is then
`>=0% <=100%`, and the guard enforces that, so a rate that can exceed 100% declares its range
like any other number. A table's numeric output column needs no range of its own — its cells
are its range.

`contract_only` marks an input that is only ever an entry check and appears in no table:

```
  重量(weight) : mass[g]  range >=1g <=25kg  contract_only
```

Without it, an input no table uses is W111.

### `round` — required on every numeric output

```
  送料(fee) : money[円, incl_tax]  round up(10円)
```

Five modes, each pinned down for negative values:

| mode | direction | at grid 1 |
|---|---|---|
| `up` | away from zero | −4.2 → −5 |
| `down` | toward zero | −4.8 → −4 |
| `half_up` | an exact half goes away from zero | −4.5 → −5 |
| `half_down` | an exact half goes toward zero | 4.5 → 4, 4.6 → 5 |
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

## 6.1 constraint

A relation between two inputs that the caller guarantees. It computes nothing; it says
**which combinations of inputs can happen**.

```rule
constraint 全条件一致数 <= 会社名一致数
constraint 適用開始日 <= 適用終了日
```

The shape is `constraint <input> <comparison> <input>`, with one of `<=`, `<`, `>=`, `>`
(E017), an input on each side (E018), and both of a type that has an order — money, a
quantity, a rate, a `number` or a `date`. Several lines all hold at once, and `A = B`, when
it is ever wanted, is the two lines `A <= B` and `A >= B`.

Three things follow from one line.

- **The completeness check stops demanding rows for combinations that cannot happen.** A
  table that covers everything reachable is complete, and the gap E101 used to report — with
  a witness nobody could ever produce — is gone.
- **A witness is one of the combinations that can happen.** Every input the checks construct
  satisfies the constraints, so what a diagnostic hands back is a case somebody could really
  send.
- **The generated code refuses a violating input at the door**, with the constraint quoted,
  the way it refuses a number outside its range. The proof assumed the constraint, so the
  code has to insist on it.

The vectors follow the same line: `rulec vectors` never produces a combination a constraint
excludes, and an `examples` row that breaks one is an error (E019) rather than a case.

Writing `-` in a cell says "this column does not matter here". Before constraints it also
had to stand in for "this cannot happen", and the two read the same on the page. A constraint
is how the second one is said out loud.

## 6.2 elements and fold

A rule is one decision about one case. Sometimes the case carries a **sequence** — the rows
of a tariff sheet, the candidates a predicate left — and the answer comes from walking it.
`elements` declares what one element carries; `fold` declares how the walk ends.

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

The fields of an element are declared exactly like `inputs`, and a table may use them as
columns; what differs is that the caller passes them once per element. **The table's own
checks are unchanged** — one element is one case, and completeness, overlap, units and
overflow are proved over it as they always were.

The arms:

| arm | what it does |
|---|---|
| `next` | leave this element, look at the next |
| `stop` | end the walk; the answer is what `exhausted` says |
| `stop with <value>` | end the walk with this answer |
| `take_unique <value>` | take this element's value; a second element that also takes is an error at run time |
| `take_first <value>` | take the first, ignore any later one |
| `keep_max <value> by <key>` | hold this element's value, replacing what is held when the key is larger |
| `empty -> <value>` | the answer when there are no elements. **Required** |
| `exhausted -> <value>` | the answer when the walk reached the end. **Required**; `held` is the value being held |

Four things are checked, and each of them is a loop somebody has written by hand and got
wrong:

1. the answer for an **empty** sequence is declared (E022);
2. the answer for a walk that **reached the end** is declared (E023);
3. every verdict the table can produce **has an arm** (E024), and an arm nothing can reach is
   named (W115);
4. **`take_unique` or `take_first`** — there is no bare `take`. Whether a second matching
   element is an error or is ignored is a decision, and the grammar makes it one (E021).

### Why a fold does not cost the checks their decidability

The checks work because a cell narrows its own column and nothing else, so a row is a box in
the input space and gaps and overlaps are arithmetic. An arbitrary loop would end that. A
fold does not: the table is complete and unique, so **every element lands on exactly one
verdict**, and the verdict's type is a finite enum. The walk is therefore a reduction of a
string over a finite alphabet — a small automaton — and how many elements there are at run
time does not change what can be said about it.

### What a fold generates

`rulec gen` writes the walk in **Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and Wasm** —
every target but SQL, which is refused by name below.
The sequence is an argument like any other input — an array of objects on the wire (§10.2),
each element's fields integers in their canonical unit — and every field gets the same entry
guard an input gets. `rulec test` runs the generated walk over the vectors and holds it to the
reference evaluator, the same as for any rule.

**SQL is refused by name.** One query has nowhere to carry a value from row to row and stop
early. A window function or a recursive CTE can be made to look like a walk, but only by
giving up the 1:1 between a branch of the generated query and a row of the rule — which is the
reason the SQL backend exists. So `rulec gen` names SQL, skips it, and writes the other seven.

### Writing an example of a walk

An example is a row of cells and a sequence does not fit in one, so the sequence is written
once, under a name, and the cell names it:

```rule
sequence 近い一件(near)
| 行ゾーン | 閾値   | 行運賃 |
| 近畿圏   | 500円  | 800円  |
| 近畿圏   | 2000円 | 1500円 |

sequence 空(none)
| 行ゾーン | 閾値 | 行運賃 |

examples
| あて先 | 注文金額 | 運賃行   | -> 運賃 |
| 近畿圏 | 5000円   | 近い一件 | 800円   |
| 近畿圏 | 5000円   | 空       | 0円     |
```

The columns of a `sequence` are the fields of `elements`, all of them, and every cell is a
**value** — a range or a `-` is how a table's cell is written, and this is one element as the
caller would really pass it (E026). A block with no rows is the empty sequence, which is a
case worth writing: it is the one a hand-written loop forgets.

The examples of a rule that walks a sequence must carry a column for it (E025), the cell must
name a `sequence` that exists (E027), and a `sequence` no example names is reported (W116).
Each example runs through the reference evaluator at `rulec check` like any other (E107), and
each one joins the generated vector suite.

### An input with no answer

A sequence where `take_unique` matches twice is a contradiction: the reference evaluator has
no answer and the generated code raises. Such an input is still part of the suite — it goes to
`vectors/<alias>.refused.jsonl`, and `rulec test` requires every generated language to refuse
it. That is what makes the last fold transition covered rather than merely named.

## 6.3 count

`fold` ends a walk with one answer. `count` ends it with **a number**, and the rule goes on
from there like any other.

```rule
count 一致数(hits) over 候補 where 照合 = 一致  range >=0 <=100
```

The shape is `count <name>(<alias>) over <sequence> where <column> = <value>`, with `range`
(E028, E030). The column is one of **one element** — a field of `elements`, or a column a
per-element table produces — and its values have to be a closed set, a bool or an enum
(E029). A bool column needs no `= <value>`: `where 会社名一致` counts the elements where it
is true.

A rule that counts runs in two phases, and which item belongs to which is derived rather
than declared: **an item is part of the walk exactly when it reads something that only one
element has.** So the table that classifies an element runs once per element, and everything
that reads only the counts and the ordinary inputs runs once, after.

```rule
table 候補判定(row_of)          # the walk: it reads a field of an element
policy unique
| 会社名一致 | 住所一致 | -> 照合(hit) : 照合結果 |
| true       | true     | 一致                    |
| true       | false    | 不一致                  |
| false      | -        | 不一致                  |

count 一致数(hits) over 候補 where 照合 = 一致  range >=0 <=100

table 結果判定(verdict_of)      # after the walk: it reads the count
policy unique
| 一致数 | -> 結果(result) : 判定 |
| 0      | 該当なし               |
| 1      | 一件                   |
| >=2    | 複数                   |
```

**The range is required, and it says two things.** It is the universe the completeness check
quantifies over once the count is a column — without it the check would ask for a row
covering a count of −1 — and it is **the cap on the sequence**: the generated code refuses a
sequence longer than the smallest bound any count declares, the way it refuses a number
outside its range. A count cannot leave the space the proof was made over.

Nothing else accumulates. A fold's arms choose an element; a count adds one per element that
passes a test. There is no sum, no average and no arm that carries a number forward:
computing one belongs before the call, where its result is an ordinary input.

A rule cannot have both a `fold` and a `count` (E031): they are two endings for the same
walk, and a `fold` may stop partway, which leaves the meaning of a count on that walk
undecided. Every target but SQL generates a count, for the reason SQL gets no walk at all.

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
| line heads | `rule` `description` `import` `enum` `group` `inputs` `elements` `outputs` `derive` `define` `constraint` `table` `fold` `count` `sequence` `policy` `result` `examples` |
| modifiers | `range` `round` `contract_only` `default` |
| cells | `not` `none` `true` `false` |
| rounding | `up` `down` `half_up` `half_down` `half_even` |
| functions | `min` `max` |
| fold arms | `over` `next` `stop` `with` `take_unique` `take_first` `keep_max` `by` `empty` `exhausted` `held` |
| count | `where` (and `over`, above) |
<!-- /RESERVED -->

`step` (inside `rate[step 1%]`), `unique`, `first`, the type words (`money` `mass` `length`
`rate` `number` `bool` `date` `string`), the money attributes (`incl_tax` `excl_tax`) and `std` are
part of the vocabulary but are told apart by position, so they are not reserved as names.

A test holds this table to `src/kw.rs`, which is the single place the vocabulary is defined.
