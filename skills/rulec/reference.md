# The `.rule` language

The complete definition of the syntax. It is meant to be read by something that has to write
a `.rule` correctly the first time, so it states what is allowed rather than motivating it.
The reasoning behind each restriction is in `DESIGN.md`; the diagnostics you get for breaking
one are in `rulec explain --all`, and `rulec explain <CODE>` prints any of them.

Two things to know before anything else.

- **Keywords are English and there is exactly one spelling of each.** No synonyms and no
  abbreviations. Names and cell values are the business's own words: the rules in this
  repository are written with English ones, and each one transcribed from a Japanese document
  has its Japanese original beside it.
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
import enum <enum> from "<file>" [-> <enum of this rule>]
source <name> = law [<database>] "<law id>" asof <date>
  <fragment> sha256:<digest>
source <name> = file "<file>" [url "<url>"] sha256:<digest>
shape <name>(<alias>) = jsonschema "<file>" "<pointer>"
shape <name>(<alias>) = proto "<file>" <Message>
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
overrides …
clause …          (a one-row definition written as prose; interleaves with the three above)
apply  …          (another rule, applied with its inputs bound; interleaves too)
  <callee input> = <value>
  except <definition>, …
  <callee output> -> <name>
result …
prerequisites <name>(<alias>) over <enum>   (which values of the enum need which, §6.5)
  start <value>, …
  <value> requires <value>, <value> or <value>, …
  <value> requires nothing "<reason>"
  ordered by <table>
machine <name>(<alias>) over <table>   (the rule is one step of a state machine, §6.4)
  carry   <input> -> <output>
  initial <state>
  final   <state>, …
  never   <state>, … after <state>, …
  once    <output> <cell>
scenario <name>(<alias>)               (a sequence of calls from the initial state, §6.4)
  | … |
examples
  …
```

- `rule` is the first line of the file. `v1` is the version; it is free text and appears in
  the generated header.
- `description` is one line, in quotes.
- `inputs` and `outputs` are followed by indented declarations, one per line.
- `derive`, `define` and `table` are a pipeline: each may use anything declared above it.
- `policy` belongs to the `table` immediately above it, and so does `overrides`: the tables or
  labelled rows, declared above, that every row of this table takes precedence over (§7).
- `apply` is an item like `derive`, `define` and `table`: another rule file, applied with
  every one of its inputs bound to a value of this rule (§7, "Applying another rule"). The
  lines under it are indented by two spaces.
- `machine` names the table that decides the state it carries; `scenario` blocks follow it.
- `examples` comes last. There may be more than one section of them (§9).

A second kind of file starts with `enums <name>(<alias>) v<version>` instead of `rule`. It holds
enums that several rules share, and nothing that decides: `description`, `import`, `enum`,
`group` and `prerequisites` lines only (§3.1, "Enums several rules share"; §6.5).

A blank line separates sections. `#` starts a comment that runs to the end of the line;
comments may appear anywhere, including at the end of a table row and on a line of their own
in the middle of a block — **a line that holds nothing but a comment does not end the
section**, where a blank one does. A comment at the end of a declaration, of a `table` line or
of a row is shown on the page `rulec doc` renders, which is where the source a table or a row
was transcribed from belongs.

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

An alias that is **a word one of the targets has taken** is W121. A builtin is the quiet case:
the rule's alias names the generated function and an enum's alias names a type, so `sum` or
`list` there hides Python's. A keyword is the hard one — `type` as an input's alias generates
Rust that does not compile — and an alias stays as written, since it is the identifier its
author chose. A name with no alias is its own identifier, and there the generated code does
the renaming: an input, an output, a `derive`, a `define`, a table's output column, a sequence
or one of its fields called `type`, `class`, `def` or `from` — a keyword in some of the
languages that write a name as it is — is written with `_` after it in every language
(`class_`), `rulec api` names it so, and the name a value travels under — the keys of the input
JSON, the records — stays the one declared. W121 says which languages took the word, and an
alias chooses the identifier instead. A parameter or a local hides nothing outside its own
body, so the warning is raised for those only when the word is a keyword. It is a warning
because a target you do not generate cannot hurt you.

**Numbers** are digits, optionally with `_` as a separator, optionally preceded by `-`,
optionally followed by a multiplier and then a unit:

| part | values |
|---|---|
| multiplier | `万` (10⁴), `億` (10⁸), `兆` (10¹²) |
| unit | mass `mg` `g` `kg` `t` `oz` `lb` · length `mm` `cm` `m` `km` `in` `ft` `yd` `mi` · rate `%` · money `円` `銭`, or an ISO 4217 code, whose hundredth is that code plus `c` (`USD` and `USDc`). **Two currencies never convert**: there is no exchange rate here, so mixing them is E103 |

`1000万円` is 10,000,000 yen, which `10_000_000JPY` writes as well. **A number in a cell must carry its unit**: a bare `2000` where
a quantity is expected is an error. `2kg` and `2000g` are the same value; the stored integer
is always in the unit the type declares. In an expression a number is read in the unit it is
written in, and it has to be a value of it: a bare number is a `number`, which is whole, so a
fraction is written as the rate it is — `amount * 40%`, not `amount * 0.4` — and `0.5JPY` is
no amount at all. Either is E103, and for a bare decimal its `fix.text` is the `%` form.

**A thousands separator is not written** (E049): `,` separates the members of a set, so the
`1,000JPY` a document prints would be two values. Write `1000JPY`, or `1_000JPY` to group the
digits; the diagnostic's `fix.text` is the literal rewritten.

**Dates** are written `YYYY-MM-DD`. They are held as day ordinals, which is what makes an
interval over dates exact.

**Strings** are double-quoted and must close on the same line (E001). There is no escape: a
string holds every character up to the next `"`, a backslash included, so it cannot hold a `"`.

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

Fourteen, and no others.

| type | written | notes |
|---|---|---|
| boolean | `bool` | |
| enum | the enum's name | a **closed** finite set, declared with `enum` or brought in with `import` |
| mass | `mass[g]`, `mass[lb]`, … | the unit is part of the type. `mg` `g` `kg` `t` `oz` `lb` |
| length | `length[cm]`, `length[in]`, … | `mm` `cm` `m` `km` `in` `ft` `yd` `mi` |
| area | `area[m2]`, `area[坪]`, … | `mm2` `cm2` `m2` `a` `ha` `km2` `坪` `in2` `ft2` `yd2` `mi2` `ac`. A dimension of its own: `width * depth` is E103, because there is no dimensional analysis here (§2.1) |
| volume | `volume[m3]`, `volume[L]`, … | `mm3` `cm3` `m3` `mL` `L` `kL`. `cm3` and `mL` are the same size, and so are `m3` and `kL`. **There is no gallon** — the US one is 3.785411784 L and the imperial one 4.54609 L |
| duration | `duration[h]`, `duration[min]`, … | `ms` `s` `min` `h` `d` `w`. A minute is `min`, because `m` is the metre. This is a span of time; a calendar day is `date` |
| temperature | `temperature[℃]`, `temperature[℉]` | `℃` `℉`. **Ordered, not arithmetic**: comparison and `range` only (E048). 41℉ is exactly 5℃, and a literal converts between them; a difference of two temperatures is not written at all |
| sound | `sound[dB]` | `dB`, a sound pressure level. **Ordered, not arithmetic** (E048): a decibel is a logarithm, so adding two of them is not two sounds' worth |
| money | `money[JPY, incl_tax]`, `money[USD, excl_tax]` | currency **and** tax flag are both part of the type. Any ISO 4217 code, or `円`; the hundredth of a currency is its code plus `c`, so `money[USD]` counts dollars and `money[USDc]` counts cents. **Two currencies never convert** — there is no exchange rate here, and mixing them is E103 |
| rate | `rate[step 1%]`, `rate[step 0.1%]`, `rate` | the stored integer counts steps. **An input declares its step** (E103 without one): what the caller passes is that integer, and its meaning cannot change when a row is added. **An output travels at the step it declares**, or with none declared at its rounding grid, which every answer sits on — never at whatever its rows happen to write, so no row can change what the integer counts. A rounding grid that is not a whole number of the declared step is E114. A computed rate that is not an output may leave the step out; it is held at whatever its literals need |
| number | `number` | a whole number with no unit — a count of things, a number of days, a score |
| date | `date` | comparison and range only. **There is no date arithmetic** (E048) |
| string | `string` | **cannot be a table column** (E110). Use it for an output, or for an input that only passes through. A value that decides a branch belongs in an `enum` |
| optional | `member_kind?` | any of the above, plus the absent value, which the cell `none` tests. Its `range` and its step are those of the type it wraps and hold the value when there is one; it never appears in an expression or a `constraint`. A comparison, a literal and a set of values match only a value that is there, while `-` and `not:` match the absent value too. Completeness asks for a row for the absent value and for the whole declared range beside it (E101), and the generated code guards a value that is there as it guards the type it wraps |

Every quantity, money, rate, number and date is an **integer** internally. No floating point appears
anywhere in the tool or in the generated code.

Money of different currencies or different tax flags cannot be added or compared, and neither
can values of different units (E103). A conversion is written as a table, never as a formula.

There are no compound dimensions. `weight * length` and `width * depth` are both E103, as `amount * amount`
always was: this tool does no dimensional analysis, and a dimension invented to hold a product
would be one nobody declared. Where the product itself is what a rule decides on — a floor
area, a volume of water — take it as an input, or look it up in a table. For the same reason a
divisor carrying a unit has to be written in the **left side's** unit: a divisor is read at the
unit it is written in and never converted, so `weight / 2kg`, with `weight` a `mass[g]`, is E103 rather than a
division by 2000.

A value is read **through its unit**, wherever it is written. A range bound, a table cell, an
expected value in `examples`, a rounding grid, a `step` inside a type's brackets, an answer a
`fold` gives, a member of a `group`: each is a value of the thing it sits in, and one that is
not is E103 (E012 for a name). `round up(10USDc)` on a `money[USD]` output is refused, and
`round up(1000USDc)` is accepted as the ten dollars it is worth — the unit is read, not merely
recognised.

Three types are **ordered but not arithmetic**: `date`, `temperature` and `sound`. They are
written in cells, compared, and given a `range`, and that is all — `+ - × ÷` over any of them
is E048. A ℃ is a scale whose zero is displaced, so `temp * 2` means nothing; a decibel is a
logarithm, so adding two of them is not two sounds' worth; a date is a calendar day, and there
is no type to hold the result of subtracting one. Where a difference is itself what the rule
decides on, compute it on the calling side and pass it in — as a `duration`, or a `number`.

### Declaring an enum

```rule
enum member_kind = basic | gold | platinum
```

A value may be marked `default`:

```rule
enum member_kind = basic default | gold | platinum
```

which declares "this value needs no row of its own; being caught by a `-` row is correct" and
silences W111 for it.

**An ordered enum** separates its values with `<` instead of `|`, lowest first:

```rule
enum tier = basic < silver < gold < platinum
```

A cell of a column of it may then compare against a value: `>=gold` is gold and platinum,
`<gold` is basic and silver, and `>=silver <platinum` is the two between (§7). An expression
may compare two values of it (`campaign > member`) and take the later or the earlier of two
(`max(member, campaign)`, `min(…)`); a value of the enum itself is written in a cell, not in an
expression (E012). An input of the enum may declare a `range` (`range >=basic <=gold`, §4): a
value outside it is refused at the door as a number outside its range is, and the tables are
asked to cover only the values inside. The two separators do not mix in one declaration, and
nothing compares the values of an enum declared with `|`, whose order means nothing (E069).
Completeness and overlap are checked over the values, as for any enum.

**A display name** is a string after a value:

```rule
enum zone = local "Same city" < regional "Same region" < national "Rest of the country" < overseas "Abroad"
```

It is the name the value is shown by: on the page `rulec doc` renders, in the article for
customers, and through one function or method in each language's generated code
(`zone_label(Zone.NATIONAL)` is `"Rest of the country"` in Python; generated-code.md lists
each language's). A value without one is shown by its declared name. A display name is text:
anything but a line break, not empty, and no two values of one enum share one (E069). It never
travels: the wire and the records carry the value's name.

**`open`** written after the last value — `enum building = farm | market | temple  open` —
marks an enum whose list may be extended from outside its file. It is read and kept; nothing
in this version extends a list, so within a rule the enum is closed as every enum is.

Two enums may have values of the same name — Stripe's `capture_method` and
`confirmation_method` both have `automatic` and `manual`. A value is read in the enum of the
place it is written: the column of a cell, the output of an output cell, the carried enum in
the lines under `machine`. A value of one enum written where another is expected is E103,
in an output cell as in any other. A value may be spelled like one of the reserved words
(§12), except the eleven its own position would read as something else.

### Declaring a group

```rule
group north_america = domestic, canada
```

A group is a named subset of an enum and may be used in a cell wherever a value may. Groups
are always expanded before checking, so a hole in a table written with groups is still found.
Its members are values of one enum (E103 when they mix two); when two enums both have every
member, the group belongs to the enum of the first column it is written in, and a column of
the other enum is E103.

## 3.1 import

Four lines start with `import`, and each brings in **the values of an enum** — nothing else
crosses a file boundary. A rule is still one file: what is imported is a set of names, not
rows, not amounts, not another rule.

| line | what it brings | who owns the set |
|---|---|---|
| `import std/<name>` | a built-in enum | rulec, frozen |
| `import proto "<file>" <Enum> -> <enum of this rule>` | the value set of an enum in a `.proto` | that `.proto`, outside this rule |
| `import jsonschema "<file>" "<pointer>" -> <enum of this rule>` | the value set of an enum in a JSON Schema, OpenAPI included | that file, outside this rule |
| `import enum <enum> from "<file>" [-> <name>]` | an enum of an `enums` file, whole: its values, their order, their display names and `default` marks, and its groups | that `enums` file, which several rules read |

> `rulec import csv` and `rulec import xlsx` are a different thing that shares the word: a
> **command** that writes a first draft of a `.rule` from a spreadsheet, once. It leaves no
> line in the file and nothing is read again afterwards. These two lines are read on every
> `rulec check`.

### Built-in enums

`import std/<country>/<kind>` brings in the first-level divisions of a country: ISO 3166-1's two
letters in lower case, and the kind of division. The enum is named `<country>_<kind, singular>`.

| import | enum | values | generated type |
|---|---|---|---|
| `std/us/states` | `us_state` | 56 (the 50 states, DC, 5 territories) | `UsState` |
| `std/gb/nations` | `gb_nation` | 4 | `GbNation` |
| `std/cn/provinces` | `cn_province` | 33 (Taiwan is `std/tw/divisions`) | `CnProvince` |
| `std/tw/divisions` | `tw_division` | 22 | `TwDivision` |
| `std/kr/provinces` | `kr_province` | 17 | `KrProvince` |
| `std/in/states` | `in_state` | 36 (28 states, 8 union territories) | `InState` |
| `std/fr/regions` | `fr_region` | 18 | `FrRegion` |
| `std/es/communities` | `es_community` | 19 (17 communities, Ceuta, Melilla) | `EsCommunity` |
| `std/it/regions` | `it_region` | 20 | `ItRegion` |
| `std/de/states` | `de_state` | 16 | `DeState` |
| `std/au/states` | `au_state` | 8 (6 states, 2 territories) | `AuState` |
| `std/br/states` | `br_state` | 27 (26 states, the Federal District) | `BrState` |
| `std/jp/prefectures` | `jp_prefecture` | 47 | `Prefecture` |

A value is the division's English name in ASCII, with `_` for spaces and hyphens (`California`,
`New_York`, `Bavaria`, `Ile_de_France`, `Sao_Paulo`, `Tokyo`); that is the spelling every
output, the wire and the generated code use. Inside a rule a division may also be written in
the country's own language (`Bayern`, `北京`, `서울특별시`, `Île_de_France`, `São_Paulo`) or by
its ISO 3166-2 code without the country (`CA`, `BY`, `NSW`; a code that starts with a digit,
such as Japan's, Korea's and Italy's, cannot be a name). Every spelling of a division is the
same value: completeness, overlap, the vectors and the certificate see one value.

```rule
import std/us/states

group west = CA, OR, WA, Nevada

inputs
  state : us_state
```

`import std/都道府県` brings in the same 47 prefectures as `std/jp/prefectures`, as the enum
`都道府県` with the values spelled in Japanese (`東京都`); both accept both spellings, and the
generated code is the same. One rule imports one country once (E013). A word that is no value
gets the closest spelling, or the namespace of the country that has it, in E012's note; a code
two imported countries share (`WA` is Washington and Western Australia) is E012 too, and is
written by its name instead. The names come from Unicode CLDR (see `THIRD_PARTY_NOTICES`).
Anything else after `import std/` is E013.

### An enum a `.proto` owns

```rule
import proto "api/v1/order.proto" MemberTier -> tier
enum tier = basic | gold | platinum default
```

When the values come from a service contract, the set is not the rule's to decide. The line
above says where it comes from, and `rulec check` reads that file on every run and holds the
two together. The path is followed from the directory of the `.rule`.

The two sides carry different things, and neither can be derived from the other. The `.proto`
owns **which values exist**; the `.rule` owns **what they are called here** and how each one is
treated (a value named in Japanese takes its name from the rule; the proto has only the ASCII one).
So what is checked is that they agree:

- the value names are matched by the ASCII alias, with the enum's own name taken off the front
  (`MEMBER_TIER_GOLD` is `gold`), which is the prefix convention `buf lint` enforces. The prefix
  is the name split into words the way buf splits it, so `HTTPMethod` asks for `HTTP_METHOD_`
  and `Tier2` for `TIER2_`. It is taken off only when what is left begins with a letter:
  `SIZE_60` is `size_60`, since an alias cannot begin with a digit;
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
import jsonschema "api/openapi.json" "#/components/schemas/MemberTier" -> tier
enum tier = basic | gold | platinum default
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

### Enums several rules share

```rule
enums catalog v1
description "The product categories and the delivery zones that the shipping and the returns rules share"

enum category = food "Food & drink" | apparel "Clothing & shoes" | electronics "Electronics" | furniture "Furniture"
enum zone = local "Same city" < regional "Same region" < national "Rest of the country" < overseas "Abroad"

group fragile = electronics, furniture
```

```rule
rule catalog_shipping v1
import enum category from "shared/catalog.rule"
import enum zone from "shared/catalog.rule"
```

A file that starts with `enums` holds enums and decides nothing: `description`, `import`, `enum`
and `group` lines, and any other section is E066. It generates no function. A rule reads one of
its enums with `import enum <enum> from "<file>"`, the path followed from the rule's directory,
and `-> <name>` gives the enum another name in the rule. What comes is the whole enum — its
values, their order, their display names, their `default` marks, and every group of it in the
file — and none of it is written again. The file is read and checked on every `check` and
pinned by nothing, as a `.proto` is: a value it gains reaches every rule that reads it, and
every table there that does not look at the value fails completeness. A file that cannot be
read, is not an `enums` file, or holds no such enum is E013; an enum or a group the rule
declares under a name an import brings is E034.

The generated code of a rule declares none of these types. Each language imports them from the
one module the `enums` file becomes, which `rulec gen` writes beside the rule's whether or not
the file is named on the command line (generated-code.md), so an app that calls two rules hands
both the same value. `rulec api` of an `enums` file names that module in each language, and
`rulec doc` renders its enums with their order and display names. The commands that are about
a decision — `schema`, `adapter`, `verify`, `replay`, `fixtures lint`, `diff`, and `doc` as
HTML or for customers — refuse one.

## 3.2 source — the documents a rule transcribes

```rule
source stamp_act = law "342AC0000000023" asof 2026-04-01
  別表第一 sha256:0ba69792e960021e
source measures_act = law "332AC0000000026" asof 2026-04-01
  第91条 sha256:85faf53f6f6e8196
source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269
source japanpost = file "yupack_tariff.pdf" sha256:9e4edb5b6a1c0f42
source terms = file "tariff.md" url "https://raw.githubusercontent.com/o/r/a1b2c3d/docs/tariff.md" sha256:4f1e0a77b2c3d5e6
  table1 sha256:a583ec8586bbf596

table base  @stamp_act 別表第一
policy unique
| stated | amount | -> tax |
| false  | -      | 200JPY |  @stamp_act 別表第一  # no amount stated
```

A `source` names a document, after `import`. A `law` is a law in a statute database, by the
id that database gives it, read as of a date: the API returns one fragment at a time, so each
fragment the rule cites is kept as a copy beside the rule
(`sources/law/<law id>@<date>/<element>.xml`) and pinned by its digest on the line under the
`source`.

**Which database** is the word after `law`, and there are two.

| word | the database | the id | a fragment |
|---|---|---|---|
| (none), or `egov` | e-Gov, the Japanese government's statute database | the law id, `342AC0000000023` | `第91条`, `第20条の2第3項`, `別表第一`, `附則第3条` |
| `ecfr` | the Electronic Code of Federal Regulations: US federal regulations as in force on a date | a title and a part, `29 CFR 1910` | a section, `§1910.157` (or `1910.157`) |

The word is left out for e-Gov, so a rule written before there was a choice reads the same.
What differs between the two is the shape of the id, the shape of a fragment, and where a
copy comes from — not what a copy is, nor what it is held to. A fragment the language cannot
read as one word is **quoted**, in the citation and on the pin line alike: `@osha "§1910.157"`.
A paragraph of a CFR section (`(d)(2)`) is not addressed yet, because the eCFR serves a
section at a time and cutting the copy up here would make it worse evidence than the one the
government served. A `file` is a document beside the rule — a tariff
sheet, a workbook, a policy in Markdown — pinned whole on its own line. A `url` on
it says where that copy came from, so `rulec source fetch` can bring it again and `rulec source
outdated` can ask whether the original has moved on; a document that arrived from a person has
no address and leaves it out.

**A document's fragments are its tables.** `table1` is the first table of the document in document
order and `表1` is the same name in Japanese; for a workbook a table is a sheet. A cited table
is taken out of the document by `rulec source fetch`, written beside it as
`<document>.fragments/table1.tsv` (`tariff.md.fragments/table1.tsv`), and pinned under the `source` line exactly as a law's articles
are — so a revision of the document that moves a cited table fails the check, and one that does
not, does not. The formats read here are csv, md, xlsx and docx — a workbook's table is a sheet, a Word
document's is a `w:tbl`, and a merged cell leaves the column it took empty rather than being
filled in, because the copy is evidence. A PDF or a scan needs an extractor that is not this
program: `rulec source fetch --via <cmd>` runs one as a child process and reads the tables it
hands back ([formats.md](formats.md), the extraction protocol), and the name it gives itself is
kept beside the copies and shown on the page for people. A document with no extractor to hand is
cited whole.

**The rows are then held to that copy**, which is the one thing a completeness proof cannot do:
it says the table is consistent with itself, not that it says what the document says. An amount
a row writes that the copy does not show is **E116**. Where the copy has a heading that says a
word of the row's cells — `関東`, or `一般` for a row the copy heads `一般の事業` — the amount is
looked for in that heading's row and column only, since the amount of the next row down is in
the copy too; and a rate the copy writes per thousand, `5/1,000` or `1,000分の5`, reads as the
0.5% it is. A number the copy states as a
whole cell that no row of the citing table uses is **W120**, and the two together are what a
mistyped digit looks like (`890円` is nowhere in the copy, and the copy's `880円` is used by
nobody). A threshold is rewritten as it is
transcribed — `1,949,000円まで` becomes `<=1949000円` — so its text cannot be compared, but one
thing survives the rewriting: **which of the two bands the boundary value itself falls in**.
The copy's `60cm以下` and `60cmを超え` both put 60cm in the band below, and so do `<=60cm` and
`>60cm`; a row that puts it in the other band is **E119**, reported on both of the rows that
share the boundary. The word is read beside the number (`60cm以下`, `Under 18`, `Not over
$11,925`) or in a heading over its column (`円以上` and `円未満`, as a premium table writes
them). A boundary the copy words neither way — `18 to 20`, `60〜80` — is left alone, because
naming the numbers that bound a band does not say which band holds them. W120 asks only about cells that are nothing but a
number, and a row that merges what the copy lists one by one (`<=3kg` over its `1kg`, `2kg` and
`3kg`) accounts for all of them. A table that transcribes only part of a fragment moves its
citation from the `table` line onto the rows that came from it: a row's citation says only where
that row came from, so the rest goes unasked while the amounts are still checked.

`@<source> <fragment>` at the end of a `table`, `clause`, `derive` or `define` line, or after
the last bar of a row, says which fragment the definition transcribes: `第91条`, `第20条の2`,
`第20条第2項`, `第20条第2項第3号`, `別表第一`, and the supplementary provisions as `附則第3条` (the law's
own) or `附則（令和七年三月三一日法律第一三号）第3条` (an amending law's, its number spelled as the law's
heading spells it); several are separated by `,`. A `file` is cited
whole, `@japanpost`, or by one of its tables, `@japanpost table1`; a law cited with no article is E037, and so
is a document fragment named anything but `表<n>` or `table<n>`. The citation goes before the `#` comment. `check` holds the pins to the copies and never reads the network: a cited
fragment without a pin is E037 (the fix is the pin line), a pin that differs from the copy is
E038 (naming the definitions that cite it), a fragment with no copy is E039, and a pin no
citation uses is W119. `rulec source fetch` brings the copies — from e-Gov for a law, from the `url` for a file, and
out of the document itself for a cited table —
`rulec source pin` writes the pins, and `rulec source outdated` asks whether the original has
moved on, which is the one question `check` cannot answer offline. For a law it asks e-Gov
whether an amendment enforced after the date changes the text of a cited fragment (a revision
that only re-marks the XML is not a change).

For a file it depends on what the `url` names, and the difference is worth knowing before
writing one. **A URL that names a commit cannot go stale**, so the question is asked of the
repository instead: on a GitHub raw URL whose revision is a commit
(`raw.githubusercontent.com/<owner>/<repo>/<commit>/<path>`), `outdated` asks what has touched
that path since, and answers with the commits, their dates and their subjects, and the URL to
pin next. A URL that names a branch, or any other URL, can only be fetched and compared with
the pin: the answer is then that the bytes differ, which for a PDF or a spreadsheet is all
anything can say — and a page that changes its footer says it too. A commit is to a file what
`asof` is to a law, and pinning one is what makes this question worth asking. `GITHUB_TOKEN`
or `GH_TOKEN` is passed on when it is set; without one the API allows sixty requests an hour. The
page for people quotes the fragment under the definition that cites it: an article as its text,
with the date that text came into force and the amending law; a document's table as the table
itself, beside the rows transcribed from it. Every generated file names the sources in its header
(`Cites: measures_act = law 332AC0000000026 asof 2026-04-01 (第91条 sha256:…)`), and `rulec api` lists
them under `sources`, a file source carrying its `url` in both.

## 3.3 shape and from — where the caller's object holds an input

An input may say where it comes from, and a `shape` says which contract the object it comes
from is already described by:

```rule
shape order = jsonschema "api/order.json" "#/$defs/Order"
shape order = proto "api/v1/order.proto" shop.v1.Order

inputs
  dest  : jp_prefecture  from order.shipping.prefecture
  cold  : bool           from any order.lines where category = "chilled"
  lines : number         range >=0 <=200  from count order.lines
```

**It changes no check of the table.** What comes out of a projection is a scalar input like
any other: the region analysis never learns that it was projected, and completeness, overlap,
units and overflow are decided exactly as they would be without it. What the two declarations
buy is that the glue between the caller's object and the rule's flat inputs is **generated**
rather than written by hand, and that a field the contract renamed is E121 rather than a
`KeyError` in production.

`from` comes last on the line and runs to its end. Four shapes, and no more:

| written | what it yields |
|---|---|
| `from <shape>.<field>…` | the value at the path |
| `from any <shape>.<collection> where <field> = <value>` | `bool` — some element passed |
| `from all <shape>.<collection> where <field> = <value>` | `bool` — every element did |
| `from count <shape>.<collection> [where …]` | `number` — how many passed |

**One collection, and a unary test on a field of an element.** A join, a nested quantifier
and a path inside a cell are all refused, for the reason §0 gives: the cell language is where
this tool's boundary is, and a path in a cell would put the caller's object model inside the
checks. The test after `where` takes the same forms a cell does (`= a`, `= a, b`, `not: a`,
`<=100`), and it compares the **raw** value the contract carries, so a literal in it carries
no unit — a contract has none, and comparing a scaled number with a raw one is refused (E120).
From a `.proto`, the raw value is what protojson writes: an enum is its value's name
(`where temp = TEMP_FROZEN`), a 64-bit integer is compared as the number it is though it
arrives as a string, and `bytes` or a message is nothing a `where` can compare (E120).

The contract is read on every `check`, resolved against the directory of the rule, and
carries **no digest** — the same footing as `import proto` (§3.1): what holds the two together
is the paths. A JSON Schema is read far enough to resolve a path: `properties` wherever an
object may have them — directly, or under `allOf`, `anyOf`, `oneOf` and `if`/`then`/`else` —
`items`, and a `$ref` that stays inside the document. The keywords written beside a `$ref`
count as well from JSON Schema 2019-09 on, which OpenAPI 3.1 follows, and are ignored before,
as draft-07 and OpenAPI 3.0 say. A `.proto` is read far enough for the same: messages and
their fields, `repeated` included, with `map` skipped rather than guessed at; the members of a
`oneof` are fields of the message, each with presence of its own like an `optional` field.
A path that cannot be resolved is **E121**, which says how far it got and which names were
there; a type that does not fit is **E120**; a `shape` no input projects from is **W122**.

**The contract says which values can come, too, and that is held to the input.** A path says
where a value comes from; the contract's validation says which values pass — Protovalidate's
`(buf.validate.field)` rules on a field of a `.proto` (`gte`, `lte`, `in` and the rest, the count
of a `repeated`, the listed values of a `string`), and a schema's `minimum`, `maximum`,
`exclusiveMinimum`, `exclusiveMaximum`, `enum`, `minItems`, `maxItems` and `required`. Each is
compared with what the input takes — its `range`, the values of its enum, the range of a
`count` — and a value that passes the contract but not the input is **E122**: the generated
code would refuse, at the door, something the caller's own validation let through. A proto3
number field with no rule lets 0 through, because that is what an unset field is; an input
that does not take 0 stops there. A schema that lets a value be null — `"type": ["integer",
"null"]`, or OpenAPI 3.0's `nullable` — is E122 for an input that is not optional. A row whose
cell on a projected input admits nothing the contract lets through is **W123**. Rules this does
not read — a predefined rule, a pattern, the part of a CEL expression below — are read as not
there, which reads the contract as wider than it is: a finding it did not need to make is
possible, a missed one is not. `fix.text` of an E122 is the option or keywords to write in the
contract (`narrow_contract`); whether the contract or the rule is the side to change is a
person's decision. Still no check of the table moves: what is compared is the contract with
the input's own declaration.

**Conditions across fields are read too.** A contract relates fields to each other: a CEL
expression on a `.proto` message (`(buf.validate.message).cel`, `cel_expression`) or on a field,
a `oneof` or a `(buf.validate.message).oneof` that lets one of its fields be set, JSON Schema's
`allOf`, `anyOf`, `oneOf`, `not` and `if`/`then`/`else`. Of CEL, the part that is a condition
on whole numbers, strings and booleans is read: sums and differences of fields and constants,
a product with a constant, the six comparisons, `in` against a list of literals, `size()` of a
repeated field, `has()`, `!`, `&&`, `||` and `? :`, and a string-valued rule that passes when it
returns `""`. What is not — division, `%`, string functions, macros such as `all`, a double — is
taken as true once every negation has been pushed down to the conditions it applies to, so an
unread part can only widen what the contract lets through. Three things follow. A condition
on another field that narrows this one narrows what E122 compares (`this.w >= 1 && this.w <=
100` on the message is a range). A `constraint` between two inputs of one contract that the
contract does not keep — some request passes its validation, inside the inputs' ranges, and
breaks the constraint — is **E123**, with that request as the example and, for a `.proto`, the
`(buf.validate.message).cel` that would promise it as `fix.text`; JSON Schema has no way to
compare two fields, so there it has no fix on the contract's side. A row whose cells, each a
value the contract lets through, ask for a combination the contract's conditions never let
through — express above 5 kg where the contract says `!this.express || this.weight_g <= 5000`,
two members of one `oneof` both set — is **W124**.

A field the contract lets an object leave out can only be taken by an optional input (`T?`),
and the projection function reads a missing one — or a missing object on the way to it — as
`none`. In a JSON Schema that is a field not in `required`, and a required input read from one
is E122, since the function could not read it.

**A `.proto` shape is read in the JSON form protojson gives it.** A field is read under its
JSON name — `json_name`, or the lowerCamelCase of its name, so `zone_code` is `zoneCode` — or
under the name the `.proto` writes, which protojson's readers accept as well. A field protojson
leaves out is read as proto reads it: a number as 0, a string as `""`, a `bool` as false, a
`repeated` as no elements, an enum as its value numbered 0, and a message as one with every
field left out. The defaults arrive, then, and they are held to the input like any other value.
Under a message field that is not `required`, and in an `optional` field, Protovalidate
validates nothing while it is unset, so the default arrives whatever the rules on the field
say; an input that does not take it is E122, with `(buf.validate.field).required = true` as the
fix. An optional input (`T?`) reads an unset message or `optional` field as `none` instead — a
field without presence cannot be missing apart from its default. A date read from a string is
held to whether `""` passes, since protojson leaves an empty string out and `""` is not a date.

A contract says how a value **travels**, and the rule says what it **means**: an enum and a
date arrive as strings, and money and a quantity as whole numbers in the unit the rule
declares.

**Five of the twelve targets generate the projection function** — Python, TypeScript,
JavaScript, Ruby and PHP, where the caller's object is a plain map and needs no name. Go,
Swift, Java and Rust hold it as a type, and the only ways to name that type would be to
generate it (this tool makes no domain object model) or to follow the caller's own; SQL takes
a relation of flat columns, NumPy takes columns, and the Wasm ABI takes one JSON object of the
rule's own inputs. **The path check applies to every target equally** — it happens in `check`,
before anything is generated. `rulec api` lists the contracts, the path of every projected
input, and the function in each language that reads them, under `projection`.

## 4. inputs and outputs

```rule
inputs
  dest   : jp_prefecture
  weight : mass[g]  range >=1g <=40kg
  total  : money[JPY, incl_tax]  range >=0JPY <=10_000_000JPY
  member : member_kind

outputs
  fee : money[JPY, incl_tax]  round up(10JPY)
```

### `range` — required on every numeric input and every `derive`

One declaration does three jobs.

1. **Overflow proof.** The reachable interval of every intermediate value is computed from
   the declared ranges and steps, and must fit in int64 (E108).
2. **The universe of the completeness check.** "Every input matches some row" means every
   input *within these ranges*.
3. **The entry guard of the generated code.** A call outside the range returns an error
   instead of silently computing something.

The form is `range` followed by one or two bounds: `range >=0JPY <=10_000_000JPY`, `range >=1g`.
A `derive` whose declared range does not contain what it can actually reach is E112, and the
message states the interval to widen to. An input of an ordered enum (§3) may declare a range
over its values, `range >=basic <=gold`: the bounds are values of the enum, and the three jobs
are the same over them. A rate input may leave `range` out: it is then
`>=0% <=100%`, and the guard enforces that, so a rate that can exceed 100% declares its range
like any other number. A table's numeric output column needs no range of its own — its cells
are its range.

`contract_only` marks an input that is only ever an entry check and appears in no table:

```
  weight : mass[g]  range >=1g <=25kg  contract_only
```

Without it, an input no table uses is W111.

### `range from koyomi` — the days a koyomi date comes to

A date input can take its range from a date of a [koyomi](https://github.com/i2y/ritsu) file
instead of from bounds. koyomi computes the date on every input of its own range, so the set of
days it comes to is known exactly, and the rule is checked over those days and no other:

```rule
inputs
  pay_day : date  range from koyomi "payment_terms.cal" date payment
```

What follows `from` names the date the way every language of ritsu names one: `koyomi`, the
file (a path from the rule's directory), `date` and the date's name. With `payment_terms.cal`
closing on the 20th and paying on the 10th of the next month, `pay_day` takes twelve days, the
10th of each month, and a table needs no row for the days between them:

```rule
table pick
policy unique
| pay_day                   | -> batch : run |
| <=2026-06-30              | first_half     |
| >=2026-07-10 <=2026-12-10 | second_half    |
| >=2027-01-01              | year_end       |
```

Over every day from 2026-02-10 to 2027-01-10 this table leaves 2026-07-01 to 07-09 uncovered;
over the payment days it is complete. The four jobs a range does all follow the set:

1. **Completeness, overlaps and unreachable rows** are about the days in the set. A day of the
   set no row takes is E101 with that day as the example; a row that takes none of them is E102.
2. **The examples and the vectors** use those days only. An example on another day is E019.
3. **The generated code refuses any other day at its door**, in every backend.
4. **The certificate** carries the days and where they come from — the koyomi file and its date
   as the rule names them, and the file's SHA-256 — and both re-checkers (`tools/recheck.py`
   and the Lean one in `proofs/`) rule a box out when it holds none of the days.

The set is read through ritsu's port of dates, so the rule has to be checked with koyomi joined:
`ritsu rulec check` or `ritsu check`. The binary of rulec's own crate has no koyomi; there the
file is not checked over every day instead, and the run says so (E129, exit 2). A set that cannot be had — the file is not there, does not pass koyomi's
check, has no such date, or comes to no day — is E130, with what koyomi says. Only a `date`
input takes `range from koyomi` (E065); an output, a `derive` or a field of an element cannot
borrow its range from another file. The days are listed as a precondition in `rulec api`
(`"kind": "days"`), and the page for people lists them beside the input.

### `round` — required on every numeric output

```
  fee : money[JPY, incl_tax]  round up(10JPY)
```

Five modes, each pinned down for negative values:

| mode | direction | at grid 1 |
|---|---|---|
| `up` | away from zero | −4.2 → −5 |
| `down` | toward zero | −4.8 → −4 |
| `half_up` | an exact half goes away from zero | −4.5 → −5 |
| `half_down` | an exact half goes toward zero | 4.5 → 4, 4.6 → 5 |
| `half_even` | an exact half goes to the even neighbour | 2.5 → 2, 3.5 → 4 |

The value in parentheses is the **grid**: `up(10JPY)` rounds to a multiple of 10 yen. Rounding
is applied once per output, last. A missing one is E104; an output literal that is not a
multiple of the grid is E106.

## 5. derive

A linear combination of inputs, which **can be used as a table column while still being a
quantity**.

```rule
derive net : money[JPY, incl_tax] = subtotal - discount  range >=0JPY <=1_000_000JPY
```

The right-hand side may use inputs, `+`, `-`, and multiplication by a constant. `range` is
required. What the derive can actually come to is its expression computed over the inputs'
ranges, the `constraint` lines and the values the tables above write: the `range` has to contain
it (E112), and a row that asks the derive only for values outside it is E102, however wide the
`range` is written. So is a row that asks it, beside the value a table above writes into another
column, only for values it never comes to while the rows that write that value fire: when `low`
is written only for an amount up to 50 pounds, `low` beside an excess over the limit past 50
pounds is E102, and E101 asks no row for it. A constant that
is a fraction puts the values between whole units — `amount * 10%` comes in tenths of a pound —
and the checks read them on that step: rows `<=1GBP` and `>=2GBP` leave `1.1GBP` to no row, and
E101 says so with the amount that makes it (`amount = 11`).

## 6. define

A named boolean or intermediate value. A boolean `define` may be used as a table column.

```rule
define bulk : bool = total >= 30_000JPY
define a_earlier : bool = a_due <= b_due
define rate_off : money[JPY, incl_tax] = list * rate
define effective : tier = max(member, campaign)
```

The condition of a boolean `define` must be one of exactly two shapes (E113):

1. a **unary test on one input or one derived value** — that value compared with a constant;
2. a comparison of **two values of a type whose difference cannot be derived** (two dates,
   for instance, or two values of an ordered enum, §3).

A direct comparison of two numbers is neither. Declare the difference as a `derive` and
compare that with a constant; the analysis is exact that way, and the message says so.

A `define` that computes a number may be a table column too. What it can come to is its
expression computed over the intervals of what it reads — the inputs' ranges, what the derives
and the other defines come to, the values a table above writes — whatever the expression's shape
(a product, a rounding, `min`, a remainder, a root): `define share : rate = score / 200` with `score` from 0 to 200
comes to 0% up to 100%. The checks read that as they read a derive's reach: E101 asks no row for a
share past 100%, and a row that asks only for one is E102.

A boolean `define` of one comparison is read the same way for the truth value it takes. Where the
intervals of its two sides settle the comparison over the inputs' ranges — `made <= 2027-03-31`
with `made` declared up to the end of 2026 — it is always true, or always false: E101 asks no row
for the other value, a row that asks only for that is E102, and two rows that meet only there do
not overlap.

A `define` of a number whose expression only adds, subtracts and multiplies by constants is an
equation as well as an interval, and the checks read it as one, as they read a derive's
(§15.209 of DESIGN.md). Each column alone may come to the value a row asks for, and the columns
still never come to them together: with `excess = added - limit`, an excess of one or more over a
limit of one or more is an `added` of two or more, so a row asking for that excess, that limit
and an `added` of at most one is E102, and E101 asks no row for it. The certificate carries the
equation as a fact of the `define` (formats.md).

Where a row asks a computed column (a derive, a `define`) for values and neither an input that
reaches the row nor a proof that none does was found, the check says the row is undecided
(W128) rather than passing it as reached: the product of two numbers is 997 only as one box of
997 or 997 boxes of one, which no candidate the search walks need be. An example that reaches the
row decides it, and the warning goes. So does the gap the row leaves: taken out of the table, the
row leaves inputs no other row covers, the completeness check finds one, and that input — run
through the rule to confirm it — reaches the row; the vectors take it too. A row of inputs and
enums alone is never undecided: its point is an input.

## 6.1 constraint

A relation between two inputs that the caller guarantees. It computes nothing; it says
**which combinations of inputs can happen**.

```rule
constraint all_matches <= name_matches
constraint valid_from <= valid_to
```

The shape is `constraint <input> <comparison> <input>`, with one of `<=`, `<`, `>=`, `>`
(E017), an input on each side (E018), and both of a type that has an order — money, a
quantity, a rate, a `number` or a `date`. Several lines all hold at once, and `A = B`, when
it is ever wanted, is the two lines `A <= B` and `A >= B`.

One side may add inputs up:

```rule
constraint used + requested <= quota
constraint roll < challenger_strength + holder_strength
```

Only inputs joined by `+`, and on one side only: `a + b <= c + d` and a constant or a factor in
the sum (`a + 3 <= b`, `2 * a <= b`) are E017, and something other than an input is E018. The
inputs of a sum are of the other side's type (E103), and a date is not added (E048). A sum is
read everywhere a pair is: the completeness check drops what the inputs' ranges show it rules
out and takes it into the elimination with the other linear relations, so a table needs no row
for a remainder below zero when `used + requested <= quota`; the witnesses and the vectors keep
to it; the generated code refuses an input that breaks it at the door; an example that breaks it
is E019.

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
excludes, and an `examples` row that breaks one is an error (E019) rather than a case. Two
dates are compared by the days they are, in all of these as at the door.

Writing `-` in a cell says "this column does not matter here". Before constraints it also
had to stand in for "this cannot happen", and the two read the same on the page. A constraint
is how the second one is said out loud.

## 6.2 elements and fold

A rule is one decision about one case. Sometimes the case carries a **sequence** — the rows
of a tariff sheet, the candidates a predicate left — and the answer comes from walking it.
`elements` declares what one element carries; `fold` declares how the walk ends. A case may
carry more than one sequence — an order's lines and the payments made against it — and each is
declared under an `elements` of its own, with a name, and walked by the lines that name it after
`over` (§6.3). A sequence nothing walks is E020.

```rule
elements fee_rows
  row_zone  : zone
  threshold : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  row_fee   : money[JPY, incl_tax]  range >=0JPY <=100_000JPY

table row_of
policy unique
| row_zone | threshold | -> verdict : pick |
| kinki    | <=1000JPY | take              |
| …

fold verdict over fee_rows
  skip      -> next
  halt      -> stop with 0JPY
  take      -> take_unique row_fee
  hold      -> keep_max row_fee by threshold
  empty     -> 0JPY
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
| `exhausted -> <value>` | the answer when the walk reached the end. **Required**; `held` is the value taken (`take_unique`, `take_first`), else the value kept (`keep_max`), else the `empty` answer |

Four things are checked, and each of them is a loop somebody has written by hand and got
wrong:

1. the answer for an **empty** sequence is declared (E022);
2. the answer for a walk that **reached the end** is declared (E023);
3. every verdict the table can produce **has an arm** (E024), and an arm nothing can reach is
   named (W115);
4. **`take_unique` or `take_first`** — there is no bare `take`. Whether a second matching
   element is an error or is ignored is a decision, and the grammar makes it one (E021).

### Naming a fold's answer

Without more, a fold answers the rule's first output. `-> <name>` after the heading gives the
answer a name, and the items below read it like any other value (§15.219 of DESIGN.md):

```rule
fold pick over lines -> discount
  top       -> take_first top_off
  rest      -> next
  empty     -> 0USD
  exhausted -> held

define balance : money[USD] = subtotal - discount - paid
```

A name that is an output is that output, and the answer is of its type. A name that is not one
is a value of the rule's own, and takes its type after a colon: `fold flag over items ->
any_flagged : bool`. Every arm's value is read in that type. It is E021 when the name is already
an input, a field of an element or a value declared above, when a name that is no output has no
type, when an output's name is given another type, and when another item decides the same name —
a fold with no `->` answers the first output, so a table or a `sum` that decides that output
too leaves two answers for one name. A numeric answer that the items below read takes the hull
of the values the fold answers with as its range, the way a `sum` takes its declared one, so a
`define` over it is held to int64 like any other (E108).

### Before the walks, on each walk, after them

A rule that walks sequences runs each of its items at one place, and which one is derived from
what the item reads rather than declared (§15.208 and §15.219 of DESIGN.md):

- **before the walks**, once: an item that reads neither a value of one element nor what a walk
  answers — a table over the ordinary inputs, a `define` computed from them;
- **on each element of one walk**: an item that reads a value of one element (a field of
  `elements`, or an item that reads one). It runs on the first walk over its sequence after the
  walks that find every summary it reads: an item of one element that reads nothing a walk
  answers runs on the first walk, and one that reads a summary — a `max` the first walk found —
  runs on a **second walk** over the sequence, once that summary is known;
- **after a walk**, once: an item that reads what the walks answer — a fold's answer, a `count`,
  a `sum`, a `max` or a `min` (§6.3) — and no value of one element. It runs once the last walk
  it reads from is over.

A summary ends the walk it is on: every summary of one sequence whose column is there on the
first walk is found on that walk, each reading every element, and one whose column is worked
out from a summary is found on the walk after. Nothing in the rule says which walk comes first.
How many walks there are is bounded by how many summaries there are, whatever the length of the
sequence, and no walk runs inside another.

```rule
define opening : number = reserve + fee          # before: inputs only
define offer   : number = max(amount, opening)   # on each element: `amount` is a field

fold verdict over bids
  counted   -> keep_max offer by offer
  ignored   -> next
  empty     -> opening                           # decided before any element
  exhausted -> held

table review_of                                  # after: it reads `best`, the answer
policy unique
| best  | -> review : bool |
| >=500 | true             |
| <500  | false            |
```

So `opening` exists when the sequence is empty, an element's item can read it, and a table can
read what the walk answered.

A second walk, in an order's settlement: the highest price is known only once every line has
been read, and the line that takes the discount is the one at that price.

```rule
sum subtotal over lines of line_total  range >=0USD <=25_000USD   # walk 1 over the lines
max top_price over lines of price empty 0USD                       # walk 1 over the lines
sum paid over payments of amount  range >=0USD <=25_000USD         # walk 1 over the payments

define gap : money[USD] = top_price - price                       # walk 2: a field and a max

table pick_line                                                   # walk 2
policy unique
| gap    | -> pick : line_pick |
| <=0USD | top                 |
| >0USD  | rest                |

fold pick over lines -> discount                                  # walk 2
  top       -> take_first top_off
  rest      -> next
  empty     -> 0USD
  exhausted -> held

define balance : money[USD] = subtotal - discount - paid          # after walk 2
```

What cannot be written is said where it is written:

- an item that reads the elements of two sequences (E031): a value of one element exists only
  while its sequence is walked, so reading both would walk one sequence again at every element
  of the other — a walk inside a walk, whose number of walks grows with the lengths;
- an output that is a value of one element (E031), and a `result` line that reads one: once the
  walk is over there is no one value to give;
- an answer that reads what is not there where it is decided (E021): `empty` and `exhausted`
  reading a value of one element, `held` anywhere but `exhausted`, and any answer or key reading
  what is worked out from the fold's own answer — a fold whose verdict waits for its answer.

The fold's answers are of the answer's type — the type of the name after `->`, or of the first
output: a word that is a value of its enum is that value (`empty -> unavailable`), a word that
is neither a value nor a declared name is E012, and a value of another type — `true` for an
enum — is E103.

### Why a fold does not cost the checks their decidability

The checks work because a cell narrows its own column and nothing else, so a row is a box in
the input space and gaps and overlaps are arithmetic. An arbitrary loop would end that. A
fold does not: the table is complete and unique, so **every element lands on exactly one
verdict**, and the verdict's type is a finite enum. The walk is therefore a reduction of a
string over a finite alphabet — a small automaton — and how many elements there are at run
time does not change what can be said about it.

### What a fold generates

`rulec gen` writes the walk in **Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java and Wasm** —
every target but SQL and NumPy, which are refused by name below.
The sequence is an argument like any other input — an array of objects on the wire (§10.2),
each element's fields integers in their canonical unit — and every field gets the same entry
guard an input gets. A rule that walks two sequences takes two such arguments, in the order the
`elements` are declared. Each walk is one loop: the summaries of one sequence found on one walk
share it, and a second walk is a second loop over the same sequence, which works out again the
values of each element it needs. `rulec test` runs the generated walks over the vectors and holds
them to the reference evaluator, the same as for any rule.

**SQL is refused by name.** One query has nowhere to carry a value from row to row and stop
early. A window function or a recursive CTE can be made to look like a walk, but only by
giving up the 1:1 between a branch of the generated query and a row of the rule — which is the
reason the SQL backend exists. So `rulec gen` names SQL, skips it, and writes the other seven.

### Writing an example of a walk

An example is a row of cells and a sequence does not fit in one, so the sequence is written
once, under a name, and the cell names it:

```rule
sequence near
| row_zone | threshold | row_fee |
| kinki    | 500JPY    | 800JPY  |
| kinki    | 2000JPY   | 1500JPY |

sequence no_rows
| row_zone | threshold | row_fee |

examples
| dest  | total   | fee_rows | -> fee |
| kinki | 5000JPY | near     | 800JPY |
| kinki | 5000JPY | no_rows  | 0JPY   |
```

The columns of a `sequence` are the fields of `elements`, all of them, and every cell is a
**value** — a range or a `-` is how a table's cell is written, and this is one element as the
caller would really pass it (E026). A block with no rows is the empty sequence, which is a
case worth writing: it is the one a hand-written loop forgets.

The examples of a rule that walks a sequence must carry a column for it — a rule that walks two
carries a column for each (E025) — the cell must name a `sequence` of the right sequence's
fields that exists (E027), and a `sequence` no example names is reported (W116).
Each example runs through the reference evaluator at `rulec check` like any other (E107), and
each one joins the generated vector suite.

### An input with no answer

A sequence where `take_unique` matches twice is a contradiction: the reference evaluator has
no answer and the generated code raises. Such an input is still part of the suite — it goes to
`vectors/<alias>.refused.jsonl`, and `rulec test` requires every generated language to refuse
it, as the contradiction it is. That is what makes the last fold transition covered rather than
merely named. The same file holds an input for every reason the door turns one away — the
sequence and the fields of its elements among them — and every language has to refuse each for
that reason, with the sentence the reference evaluator says
([formats.md](formats.md#vectors-rulec-vectors-generatedvectorsaliasjsonl), §15.204).

## 6.3 count, sum, max and min

`fold` ends a walk with one answer. `count` and `sum` end it with **a number**, `max` and `min`
with **one value of a column**, and the rule goes on from there like any other.

```rule
count hits over candidates where hit = same  range >=0 <=100
```

The shape is `count <name>(<alias>) over <sequence> where <column> = <value>`, with `range`
(E028, E030). The column is one of **one element** — a field of `elements`, or a column a
per-element table produces — and its values have to be a closed set, a bool or an enum
(E029). A bool column needs no `= <value>`: `where name_match` counts the elements where it
is true.

A rule that counts runs at the places a fold does (§6.2), and which item belongs to which is
derived rather than declared: **an item is part of a walk exactly when it reads something that
only one element has.** So the table that classifies an element runs once per element, what
reads only the ordinary inputs runs once before the walks (an element's item can read it), and
what reads the counts runs once, after.

```rule
table row_of  # the walk: it reads a field of an element
policy unique
| name_match | addr_match | -> hit : match_kind |
| true       | true       | same                |
| true       | false      | diff                |
| false      | -          | diff                |

count hits over candidates where hit = same  range >=0 <=100

table verdict_of  # after the walk: it reads the count
policy unique
| hits | -> result : outcome |
| 0    | no_match            |
| 1    | one                 |
| >=2  | several             |
```

**The range is required, and it says two things.** It is the universe the completeness check
quantifies over once the count is a column — without it the check would ask for a row
covering a count of −1 — and it is **the cap on the sequence**: the generated code refuses a
sequence longer than the smallest bound any count over it declares, the way it refuses a number
outside its range. A count cannot leave the space the proof was made over.

### sum

`sum` totals one column of the elements instead of counting them.

```rule
elements lines
  amount : money[JPY]  range >=0JPY <=100_000JPY

sum total over lines of amount  range >=0JPY <=1_000_000JPY
```

The shape is `sum <name>(<alias>) over <sequence> of <column>`, with `range`. The column is
one of **one element**, the same as a count's, and it has to be a number — an amount, a
quantity, `number` or `rate` (E029). The total keeps that column's type, so `total` above is
`money[JPY]` and a table over it is written in yen.

**The summed column has to be non-negative** (`range >=0…`, E029). That is what lets the
running total move one way only: the walk refuses the moment it passes the declared maximum,
so one test at the end is a test everywhere, and the accumulator never leaves `maximum plus
one element` — which is what keeps E108's int64 claim true of a sequence whose length nothing
caps. To take a difference, sum two non-negative columns and subtract.

### max and min

`max` and `min` answer the largest and the smallest value of one column over the elements a
`where` lets in (every element without one), or the `empty` value when there is none:

```rule
max most_up   over adjustments of shift where raises empty 0
min most_down over adjustments of shift where lowers empty 0
min earliest  over tasks of due where done = false empty 2027-12-31
```

The shape is `max <name>(<alias>) over <sequence> of <column> [where <column> [= <value>]] empty
<value>` (E028). The column is one of **one element**, and a number, an amount, a date or a
value of an ordered enum (`enum grade = bronze < silver < gold`; E029 otherwise, with the title
"This column cannot be summarised"); the `where` tests a column of one element the way `count`
does. **`empty` is required** (E022): it is the answer for an empty sequence and for one where no
element passes the `where`, and what to answer then is the author's to say. Two elements of the
same value leave one answer, since the answer is the value; which element holds it is a fold's
question (`keep_max`).

The range needs no writing. It is the hull of the column's range and the `empty` value, since
the answer is one of those — `min earliest` above takes the range of `due` with 2027-12-31
inside it — and the tables over the answer are checked over that range. A range written on the
line has to hold the hull (E112, which writes the one that would), and then it is the universe,
the way a declared one is; a `max` of an enum takes no range (E069).

### More than one summary

Nothing else accumulates. A fold's arms choose an element; a count adds one per element that
passes a test; a sum adds one column; a `max` and a `min` keep one. There is no average arm and
no arm that carries a number forward. An average is a sum over a count, divided after the walk
inside a rounding (§8): `define avg : money[JPY] = half_up(total / max(n, 1), 1JPY)`.

A rule may have any number of `count`, `sum`, `max`, `min` and `fold` lines, over one sequence
or several, each over the sequence its `over` names (§15.219 of DESIGN.md). The summaries found
on one walk read every element: a `fold` that stops ends its own reading, and the count beside
it still sees the elements after. A `count`, a `sum`, a `max` or a `min` named like an output is
that output, and answers it. Every target but SQL and NumPy generates them, for the reason SQL
gets no walk at all.

## 6.4 machine and scenario

A rule decides one call. Some rules are one **step** of something that goes on: an order that
is paid, shipped and delivered, an application that is filed, reviewed and decided, a
membership that moves up a tier each year. The state lives with the caller — in the order's row
of a database — and every call is passed the state and answers the next one. `machine` says so:

```rule
machine order over step
  carry   state -> next_state
  held    amount_paid
  initial received
  final   delivered, cancelled
  never   shipped after cancelled
  once    refund >0JPY
```

| line | what it says |
|---|---|
| `machine <name>(<alias>) over <table>` | the table whose rows decide the carried output: its rows are the transitions |
| `carry <input> -> <output>` | the output the caller passes back as that input on the next call. Both are one enum. **Required**. A second and later `carry` line carries a further value along with the state (below) |
| `held <input>, …` | inputs one case passes with the same value on every call, from its first to its last: the amount of an order, the class of the person who applied (E056 when one is not an input, or is the carried one) |
| `initial <state>` | the state a case starts in. **Required** |
| `final <state>, …` | the states a case ends in |
| `never <state>, … after <state>, …` | no sequence of calls reaches the first states once the case has been in one of the second |
| `once <output> <cell>` | in one case, at most one call answers an output the cell accepts: a refund paid, a point granted |
| `initial <input> <value>` | where a further carried value starts. One for every `carry` line after the first (E072) |
| `always <input> <cell> and … when <state>, …` | in every state a case can reach — every one on the `when` list, when there is one — each carried value named fits its cell |

`carry` is the whole of what is new about the meaning. **The generated function does not
change** — it takes the state as an argument and answers the next one, and keeps nothing — so
a rule with a `machine` is still a pure function (DESIGN P3). What the section adds is claims
about every **sequence** of calls the caller can make, and `rulec check` proves them:

| check | what it finds | code |
|---|---|---|
| completeness of the table | a state and an event with no answer — *what happens when a cancel request arrives after shipment?* — including the calls that leave the state where it is | E101 |
| overlap | a state and an event with two answers | E105 |
| final states | a call that moves a case out of a `final` state | E124 |
| finishing | a state a case can reach from which no `final` state can be reached | E125 |
| `never` | a sequence of calls that reaches the first states after the second | E126 |
| `once` | a sequence of calls in which two calls answer what the cell accepts | E127 |
| `always` | a sequence of calls that reaches a state the line speaks of with a carried value its cell does not accept | E135 |
| a carried number's range | a sequence of calls whose last call answers a carried number outside its input's range | E134 |
| reach | a state no sequence of calls reaches, and a row that applies only in such states | W125, W126 |

Without `held`, every input may take any value on every call, and a sequence that changes the
amount of an order halfway can come back as a counterexample nobody could send. With it, a
case keeps those inputs' values from its first call to its last, and the claims are about the
sequences that keep them: the check walks each **world** — each class the rule's own
boundaries cut the held inputs into — on its own. A row that applies only with a value a case reaching its state never has is
W126 then, with a note saying so.

A broken claim comes back with the **shortest sequence of calls that breaks it**, every call an
input the rule takes (`witness.trace` in the JSON, one line per call in the text):

```console
error[E126]: A sequence of calls reaches shipped after cancelled
  --> rules/order_lifecycle.rule:37 machine order
 Calls (from received):
   1. at received, event = cancel, amount_paid = 0JPY → cancelled (table step row 2)
   2. at cancelled, event = pay, amount_paid = 0JPY → paid (table step row 10)
   3. at paid, event = ship, amount_paid = 0JPY → shipped (table step row 4)
```

### Carrying a number, a truth value or another enum

A case often holds more than its state: the days left of a grace period, the attempts made, a
mark that a free trial was taken. A `carry` line after the first carries such a value along,
and an `initial` line with the input's name says where it starts:

```rule
machine subscription over step
  carry   status -> next_status
  carry   days_left -> next_days_left
  initial active
  initial days_left 0
  final   suspended
  never   active after suspended
  always  days_left >=1 <=7 when in_grace
  always  days_left 0 when active, suspended
```

What can be carried is a number whose input declares **both ends** of its range (`number`, an
amount, a quantity, a rate), a truth value, or another enum; the input and the output of a line
are one type (E051), and every carried value has its `initial` (E072). The first `carry` line is
the state: the one-word `initial`, `final` and `never` speak of it, and the other values ride
along. A row writes a carried value the way it writes any output — a value (`7`), the input
itself (`days_left`, kept as it was), or a computed name (`one_less` from
`define one_less : number = days_left - 1`). There is no special way to count down: the row for
the last day has to be written, and that is where a reader sees what happens when the count
runs out.

What a case is in is then the state together with those values, and every claim — `final`,
`never`, `once`, finishing, the states no case reaches — is about that. Two more come with it:

- **`always`** names carried inputs with a cell each, joined by `and`, and the states it holds
  in after `when` (every state, without it). A sequence of calls that reaches one of those
  states with a value the cell does not accept is E135, the shortest one attached.
- **a carried number stays within its range.** A call from a state a case reaches that answers
  it outside the input's range leaves a value no next call can be passed: E134, with the calls
  that get there. A row that would do so only from values no case holds is W126 instead — in
  the example, a night of grace at zero days, since a grace period always starts at seven.
  Such a case has nowhere to go, and E134 is what says so: neither `always` (E135) nor
  finishing (E125) looks at a value past its range again.

While every combination of the state and the carried values fits the budget, each value of a
number is a place of its own and the claims are exact. Past that, a number is cut into
intervals at the boundaries that matter to it — the cells of the tables, the values the rows
write, the `always` cells and its start — and a call from an interval goes to every interval its
next value can fall in. That is more calls than happen, so a claim that holds there holds; a
sequence that breaks a claim there is looked for again among calls that really happen, from
where a case starts, and is reported as broken only when it is found. Otherwise the claim is
W127, and so is whether a case can still finish, which the intervals cannot show. `--budget`, or
a narrower range, brings the walk back to one value at a time.

### Why the claims stay decidable

The state is a finite enum, and every other input is cut by the rule's own boundaries into
finitely many classes — the cells `rulec diff` walks (§15.122). Each class is settled by an
input that realizes it, and what the rule answers there is a transition from the state the
class has to the state the rule answers. The claims are then searches over a finite graph.
The price is the lines this language does not have, and they are left out on purpose:

- **no number without a range is carried from one call to the next.** A total so far — refunds
  paid, a balance with no limit — is kept by the caller and passed in, as it always was. A
  number with no end to its range would make the state a counter, and whether a counter
  reaches a value is not decidable in general; with both ends, the values are finitely many.
- **one machine per rule.** Two machines running side by side is where message order and races
  live; that is what TLA+ and P are for.
- **no calendar.** "Within 14 days" is an input — the days elapsed, counted by the caller —
  compared with a constant, as every date comparison is.

`once` counts a call whose output the cell accepts. Where the output is read off an input or
a computed value, the value's own boundaries are cut at the cell's, so that one input stands
for its whole class; a claim that turns on a class no input was built for and none shown
impossible is reported as undecided (W127), never as holding. A space larger than the budget
is E128: `--budget` counts fifty region nodes per cell.

A `held` input is followed by the class its own column cuts it into. Every value of a class
allows the same calls as long as nothing else reads the input; when a computed column or a
`constraint` reads it too, one value may allow a call another value of its class does not. The
claims that nothing bad is reached still hold then, since the walk of a class takes in every
value's calls, but whether a case can always still reach a final state is reported as
undecided (W127), and so is a counterexample that could only be followed by changing the value
partway.

### scenario

A `scenario` is the `examples` of a machine: a sequence of calls from the initial state, one
row per call, with what each call answers.

```rule
scenario late_pay
| event  | amount_paid | -> next_state | refund  | accepted |
| pay    | 3000JPY     | paid          | 0JPY    | true     |
| cancel | 3000JPY     | cancelled     | 3000JPY | true     |
| pay    | 3000JPY     | cancelled     | 0JPY    | false    |
```

The carried input has **no column**, and neither has any further carried value: the first call
starts in the `initial` state with each carried value at its `initial`, and every later one in
what the call before it answered. Every other input and every output has a column (E055, E111). `rulec check` runs the calls in order through the reference evaluator; a
call that does not answer what its row says is E107, reported with the calls that led to it.

### What a machine generates

The function is the one every rule gets. Beside it, each language with a module of its own
gets the constants a caller needs to keep the state: where a case starts, and whether it has
ended.

| language | initial | final states | has it ended |
|---|---|---|---|
| Python | `INITIAL` | `FINAL` | `is_final(state)` |
| NumPy | `rule.machine["initial"]` | `rule.machine["final"]` | — |
| TypeScript, JavaScript | `INITIAL` | `FINAL` | `isFinal(state)` |
| Rust | `INITIAL` | `FINAL` | `is_final(state)` |
| Ruby | `INITIAL` | `FINAL` | `final?(state)` |
| PHP | `INITIAL` | `FINAL_STATES` | `is_final($state)` |
| Go | `Initial` | `Final` | `IsFinal(s)` |
| Swift | `initialState` | `finalStates` | `isFinal(_:)` |
| Java | `INITIAL` | `FINAL` | `isFinal(state)` |
| SQL, Wasm | — | — | — |

`final` is a keyword in PHP whatever its case, and in Swift, hence the two other spellings.

A further carried value gets the value it starts at, named after its input: `INITIAL_DAYS_LEFT`
in Python, TypeScript, JavaScript, Rust, Ruby, PHP and Java, `InitialDaysLeft` in Go,
`initialDaysLeft` in Swift, and `machine.more[k].initial` in the NumPy plan. An amount, a
quantity or a rate is the language's branded type (`JPY(-5000)` in Python), a truth value its
`True` or `true`, an enum's value its member. SQL and Wasm have no place for it either: `rulec
api` gives it under `machine.carried`, with each language's name for it.

The NumPy plan is data, so the state names are the wire form. A target with one door and no
module has no place for a constant; its caller reads the two off `rulec api`, which lists every
language's spelling under `machine.constants`.

The vector suite gets **traces** as well as single calls: for every transition a case can make
and every two that can follow one another, the shortest sequence of calls from the initial
state that ends with them, and every scenario. `rulec test` has each language's runner play
them — **the state one call answers is handed to the next call as that language holds it** —
and prints its constants first, so the traces hold the hand-over and the constants as well as
the answers (docs/formats.md). A further carried value is handed over the same way: the trace's
first call is passed where each one starts, and every later one what the language answered.

## 6.5 prerequisites

Some values of an enum can be had only after others: a course after the courses it requires, a
certification after the one below it, a module of a product after the modules it builds on. A
`prerequisites` block says which, once, for every value of one enum:

```rule
prerequisites enrollment over course
  start             intro_programming, discrete_math, calculus
  statistics        requires calculus
  algorithms        requires data_structures, discrete_math
  machine_learning  requires algorithms, statistics or linear_algebra
  mainframe_cobol   requires nothing "withdrawn from the catalog in 2026"
  ordered by level_of
```

| line | what it says |
|---|---|
| `prerequisites <name>(<alias>) over <enum> [open]` | the block, its name — which names the generated functions, so a name that is not ASCII takes an alias (E011) — and the enum whose values it is about. `open` marks a block other files may add lines to, for the values they add; it is read and kept, and changes nothing yet |
| `start <value>, …` | the values that need nothing. Every other value is reached from them. **Required** |
| `<value> requires <value>, …` | what a case has to hold before it can take the value. `,` separates groups that are all needed; `or` joins the values of a group, any one of which is enough: `a, b or c` is a, and b or c |
| `<value> requires nothing "<reason>"` | a value no case can take, and why: a retired course, a certification no longer examined |
| `ordered by <table>` | a table of this rule, declared above the block, whose answer for a value is never below its answers for what the value needs. It tests one column of the enum and writes one value of an ordered enum (§3) or one `number` in each row |

Every value of the enum is in exactly one place: on `start`, or on a line of its own (E071). A
value added to the enum without its prerequisites stops the check rather than passing as one that
needs nothing.

The block stands in a rule or in an `enums` file (§3.1); `ordered by` only in a rule, since an
`enums` file has no tables. None of its words is reserved: `start`, `requires`, `or`, `nothing`
and `ordered` are read by their place in the line, so a value may be called any of them (`start
requires …` is the line of a value called `start`, and `nothing` is the word only before a
string), and a declaration may be called `prerequisites`.

What `rulec check` proves of a block:

| check | what it finds | code |
|---|---|---|
| shape | a name that is not a value, a value in two places or in none, no `start`, an `ordered by` table that is not there, is below the block or has another shape | E071 |
| cycle | a value that, through what it needs, needs itself — the values of an `or` followed too | E136 |
| reach | a value that cannot be reached from `start`, because it waits on one no case can take | E137 |
| order | a value whose `ordered by` answer is below the answer for something it needs | E138 |
| redundancy | a prerequisite another prerequisite of the same value already needs (the values of an `or` left out) | W130 |

The graph is finite, so each of these is decided by walking it, and the answer is exact. A cycle
is said once for each set of values that reach one another; what cannot be reached while there
is a cycle is not said on top. A value that waits on one no case can take is said where the wait
starts, with the values behind it in a note:

```console
error[E137]: operating_systems cannot be reached from `start`
  --> tests/mutants/m_e137.rule:40 prerequisites enrollment
   |
40 |   operating_systems requires data_structures, mainframe_cobol
   |                                               ^^^^^^^^^^^^^^^ no case can take this
   |
 operating_systems needs mainframe_cobol, which no case can take.
 mainframe_cobol: `requires nothing "withdrawn from the catalog in 2026"`
 Until operating_systems can be reached, neither can networks.
 Drop the prerequisite no case can take, or if operating_systems cannot be had either, write `operating_systems requires nothing "<reason>"`.
```

### What a block generates

Two functions, in the module of the file that holds the block — a rule's, or an `enums` file's —
in each language with modules:

- `<name>_requires(value)`: what the value needs, as a list of groups, each a list of values any
  one of which is enough. A value on `start` has no group (`[]`); one no case can take has one
  group with no value in it (`[[]]`), which nothing held can meet.
- `<name>_available(value, held)`: whether the value can be taken now: it is not in `held`, and
  every group has a value in `held`.

| language | what a value needs | whether it can be taken |
|---|---|---|
| Python, Rust, Ruby, PHP | `enrollment_requires(value)` | `enrollment_available(value, held)` |
| TypeScript, JavaScript, Swift, Java | `enrollmentRequires(value)` | `enrollmentAvailable(value, held)` |
| Go | `EnrollmentRequires(value)` | `EnrollmentAvailable(value, held)` |
| NumPy | `rule.requires("enrollment", value)` | `rule.available("enrollment", value, held)` |
| SQL, Wasm | — | — |

SQL and Wasm have one entry and no room for a second function: `rulec gen` says so when it writes
them, `rulec api` names them under `without`, and `rulec test` notes that nothing was compared
there.

NumPy's plan carries the groups as data, and its runtime answers both; an `enums` file has no plan
there.

The vector suite holds every language to the reference evaluator over a handful of cases per value
— nothing held, one value of each group, that less the value of one group, each other value of an
`or`, the value itself besides, every other value — and the certificate carries the graph with the
orders that show it has no cycle and that every value is reached (docs/formats.md).

## 7. Tables

```rule
table base
policy unique
| dest        | weight  | -> base : money[JPY, incl_tax] |
| remote      | <=2000g | 1200JPY                        |
| remote      | >2000g  | 1800JPY                        |
| not: remote | <=2000g | 800JPY                         |
| not: remote | >2000g  | 1100JPY                        |
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

### Row labels

A row may carry a label before its first bar:

```rule
       | stated | amount                   | -> tax : money[JPY] |
exempt | true   | <10_000JPY               | 0JPY                |
r3     | true   | >=10_000JPY <=100_000JPY | 200JPY              |
```

A label names the row wherever a name is needed — in an `overrides` line, in the trace a
generated function returns (`{"table":"base","row":2,"label":"exempt"}`), in a later version —
and it is unique within its table (E034). Rows without one are still counted by position, and
`rulec fmt` aligns the labels as a column of their own.

### Tables that share an output

Several tables may define the same output: a main rule and its special cases, each transcribed
from its own source. Each then has exactly one output column (E045), and which takes
precedence is written on the table that wins, right after `policy`:

```rule
table reduced_rate
policy unique
overrides base
| reduced | stated | amount                   | -> tax |
| true    | true   | >100_000JPY <=500_000JPY | 200JPY |
```

`overrides` names tables declared above, or one labelled row of one (`base:r3`), and says that
every row of this table takes precedence over them. The exception is written after what it
excepts: a target that is missing or declared below is E035, one that defines a different
output is E036. The tables of one output are checked together — completeness over their
union (E101), every overlap either ordered by an `overrides` line or reported (E105 / W114),
and a row that the rows taking precedence over it cover entirely (E102). An `overrides` line
whose rows meet none of its target's is W117. The generated code tries the tables from the last
declared to the first and takes the first row that applies, and the trace names the table the
row was written in.

### Clauses

A definition whose conditions do not line up as columns — a proviso, a main rule written as
one sentence — is a `clause`: one row, written as prose.

```rule
clause regular -> fee
  when always
  then base

clause free -> fee
  when total >=3900JPY and member true
  then 0JPY
  overrides regular
```

The heading names the clause and the output it defines (an `outputs` name, an intermediate an
earlier table or clause introduced, or a new one with `-> name : type`). `when` is a list of
`<column> <cell>` joined by `and`, the cell being any of the seven kinds above, the column
anything a table's column may be; a clause with no condition writes `when always` (leaving the
line out is E046, for the reason a blank cell is E008). `then` holds what an output cell holds:
a literal or a name. `overrides` is the same line a table may carry. A clause is a table of one
row whose other columns are `-`: it is checked, evaluated and generated as one, fires in the
trace as `{"table":"free","row":1}`, and the page shows its condition and its value as written.

### Applying another rule

A provision applied mutatis mutandis — "the rule of Article 20 applies, reading 'years of
service' as 'period in office'" — is an `apply`: another rule file, used once more with its
inputs bound to values of this rule.

```rule
apply retirement = "retirement_pay.rule" sha256:b58648ea2767ebbd  # Source: Article 31
  years = tenure
  reason = end_reason with term_end -> retirement_age, resignation -> voluntary
  base_pay = monthly_pay
  except reduction
  allowance -> part_time_allowance
```

The heading names the apply, the callee (a path relative to this file) and the digest of the
callee file, which `rulec source pin` writes and `check` holds the callee to (E040: a callee
that changed is looked at with `rulec diff` before the new digest is pasted). Under it, one
line per callee input binds it to an input, a derived value, a definition, a table or clause
output of this rule, or a literal — every input, exactly once (E041). The types agree (E042);
two enums are mapped with `with`, which covers every value of this rule's enum, values spelled
the same on both sides mapping by themselves. What this rule passes has to stay inside the
callee's declared ranges and satisfy its constraints (E043, proved by interval arithmetic and
never fixed by adding a row: the callee is another unit of approval). `except` leaves a table,
a clause or a labelled row (`table:row`) of the callee out; a hole that leaves in the main rule is
this rule's E101 to fill. The callee's outputs become definitions of this rule, under their
own names or renamed with `->`, rounded as the callee declares and then, if one is an output
here, rounded once more as this rule declares.

The callee is expanded into this rule: its tables, clauses, derives and defines appear under
`<apply>:<name>` (`retirement:schedule`), which is how a clause of this rule takes precedence over
one of them (`overrides retirement:reduction`, defining `retirement:allowance`) and how the trace names a
row (`{"table":"retirement:schedule","row":1,"label":"short"}`). Rows of the callee this rule never
reaches — its ranges are usually narrower — are silent, and the page lists them under the
apply; a whole table none of whose rows is reached is W118. A callee that itself applies a
rule, walks a sequence, produces one of its own enums, or does not pass `check` cannot be
applied (E044). The callee's sources come along with its definitions, and the page draws its
tables from its own file, under a heading that says what was applied and how. Generated files
name the callee and its digest in their header, `rulec api` lists them under `applies`, and
`rulec diff old@rev new` reads the callee at the same revision as the rule.

### The eight kinds of cell

| written | means |
|---|---|
| `-` | any value. **A blank cell is a syntax error** (E008): a blank cannot be told from a forgotten entry |
| `1200JPY` `2000g` `true` `2026-04-01` `"abc"` | equality with a literal. A quantity must carry its unit |
| `basic, gold` | a set. Each element is a literal or a group name; on a column of numbers each is a value of its own (`100, 200`) |
| `not: basic` | the complement of a set |
| `<=2000g` | comparison. `<=`, `>=`, `<`, `>`. On a column of an ordered enum (§3) it is against a value: `>=gold` |
| `>=1000JPY <20000JPY` | an interval — two comparisons side by side mean "and" |
| `starts_with "CH-"` | a prefix, on a `string` column. Two or more are separated by a comma |
| `none` | an optional that is absent. `-` and `not:` match it too; a comparison, a literal and a set of values do not |

**A `string` column takes a prefix and nothing else** (E110). Strings cannot be enumerated,
so equality and sets have no finite reading here — where the values *can* be listed, make it
an `enum`. A prefix can: the prefixes a column's cells name cut the strings into one class per
prefix, plus "under none of them", and that is all §6.2's compression asks of a column. So
completeness and overlap work on a string column exactly as they do on an enum, and the
witness for a gap is a string you can paste. The match is on the bytes: no case folding and no
Unicode normalization, so all eleven targets answer the same. (NumPy declines a table with a
string column, for the reason it declines a walk: the runtime there reads integer columns.)

A cell tests **its own column only**. There is no expression, no reference to another column,
and no function call inside a cell; that restriction is what makes a row a box and the
completeness and overlap checks exact.

**`..` is a syntax error** (E010). `0g..1000g` does not say whether 1000g is included; a
comparison operator does.

An output cell holds a literal or the name of a value declared above (an input, a `derive` or
a `define`). It never holds an expression.

A word in an output cell is read in the type of its column (§15.208 of DESIGN.md). In the
column of an enum, a value of that enum is the value, even where an output or an input of
another type has the same name (`gold`, a value of `outcome`, beside an output `gold` that is a
number). Where a name **of that same enum** is spelled the same — an input `current : plan`
beside the value `current` of `plan` — the word reads either way, and the check asks for one of
them to be renamed (E075); the answers of a `fold` are read the same way. A cell that passes an
input through (`| review, approved | false | current |`) writes only what that row's own cell
on the input allows, here `review` and `approved`, and the checks of the tables below read it
so. The expected value of an example is always a value.

## 8. result

Assembles the first output when it is not simply looked up from a table.

```rule
result fee = base * pay_rate
```

`result` is sugar for the **first** output and reaches no other. Every output — the first one
included — is otherwise taken from the binding of its own name: a `define` or a table output
column called `fee` is what the output `fee` returns. Naming a later output in a `result` is
E015; a second `result` line is E016.

Operators, from loosest to tightest: comparison (`<= >= < > =`), then `+ -`, then `* /`.
Parentheses group. The functions are `min(a, b)`, `max(a, b)`, `allocate(t, c, s)`, `mod(x, n)`
and `isqrt(x)`, and the five rounding modes may also be called as functions: `down(x, 1JPY)`,
`up(x, 10JPY)`, `half_up(x, 1JPY)`, `half_down(x, 1JPY)`, `half_even(x, 1JPY)`. There are no
others, and each takes the number of arguments written here: anything else is E118. `mod` and
`isqrt` are read as functions only where a call is written, which a name never is, so they are not
reserved: a rule may still call an input `mod` ([compatibility](compatibility.md), item 2).

A divisor is a constant (E115), with two exceptions: `allocate`, below, and the division a
rounding makes of its own argument.

### allocate — one line's share of an amount

```rule
constraint before <= upto
constraint upto <= base

derive to_before : money[JPY] = allocate(total_off, before, base)  range >=0JPY <=1_000_000JPY
derive to_upto   : money[JPY] = allocate(total_off, upto, base)    range >=0JPY <=1_000_000JPY

result share = to_upto - to_before
```

`allocate(<amount>, <running total>, <whole>)` is `<amount> × <running total> ÷ <whole>`
rounded **down** to a whole unit. It is the one place a rule may divide by something that is
not a constant, and it is allowed because of what it is for: handing an amount out over a
run of lines in the ratio of their prices.

The subtraction above is the whole point. Each line is given the share up to it minus the
share up to the line before, so the remainders telescope: **the parts add up to the amount
exactly**, with nothing left over and nothing conjured, and the line that carries the odd yen
is the last one. `proofs/` states and proves that (`runTotal_exact`), which is what makes it
a property of the rule rather than a thing the examples happen to show.

What it asks for, all of it E117: three **names with declared ranges**, an amount and a
running total that cannot be negative, a **positive** whole, and a `constraint` saying the
running total never passes the whole. The last one is not decoration — without it a share
could exceed the amount being handed out, and the interval E108 is proved against would be
the product of two ranges rather than the amount. Chains count: two `constraint` lines that
meet in the middle say what a single one would.

One call decides one line. The loop stays with the caller, as everything else here does.

### Dividing by a value inside a rounding

```rule
define average_price : money[JPY] = half_up(total / max(units, 1), 1JPY)
define win_permille : number = half_even(attack * 1000 / (attack + defense), 1)
```

Inside one of the five roundings, the outermost operation of the first argument may divide by a
value. The quotient is the exact fraction, and the rounding puts it on its grid the way its mode
says, below zero as above it — the same in every generated language, since none of them is left
to its own division. One division to a rounding, and it has to be the outermost operation:
`down(fee * used / days, 1JPY)` divides by a value, `down(fee * (used / days), 1JPY)` is E115.

- **Its type** is a division's (§3): an amount over an amount of the same unit is a ratio, which
  is a `number` or a `rate` as the grid says (`half_up(a / b, 1)`, `half_up(a / b, 0.1%)`), and
  anything over a `number` or a rate keeps its type. The grid is written in the quotient's unit
  (E103).
- **The divisor is never zero** (E132). Its interval comes from what it reads — the inputs'
  ranges, the derives and defines, what a table above writes — and has to stay above zero or
  below it. The message names an input that makes it zero. The ways out: narrow a range, divide
  by `max(units, 1)`, or put a table that picks another divisor where it would be zero and
  divide by that table's output. A row that writes one of its own table's columns writes only
  what its cell there lets through, so `| >=1 | units |` beside `| 0 | 1 |` makes a column that
  is never zero (`tests/corpus/average_unit_price_split.rule`). A `constraint` is not read for
  this: under `constraint a < b`, divide by `max(b - a, 1)`, which is the same value there.
- **The two integers the generated code divides** — the dividend and the divisor, brought to one
  step — have to fit int64 (E108).
- **What the answer can come to** is the quotient's interval (the four corners of the two
  intervals) pushed out to the grid, and a table can use it as a column.

Any other division by a value is E115, as it was.

### mod — the remainder

```rule
define turn : number = mod(week - first_week, 3)
```

`mod(x, n)` is what is left of `x` once the multiples of `n` below it are taken out: from 0 up to
`n − 1`, whatever the sign of `x` (`mod(-1, 3)` is 2). `n` is a positive whole constant (E115),
and `x` a whole `number` (E103) — not an amount, a quantity, a rate or a date. What it can come to
is `x`'s range moved down by the multiples of `n` below it when that range stays inside one
period of `n`, and 0 to `n − 1` otherwise, so a table over `turn` above needs rows for 0, 1 and 2.
There is no `%` operator: `7%` is a rate.

### isqrt — the whole square root

```rule
define quantity : number = isqrt(down(2 * yearly_demand * order_cost / holding_cost, 1))
```

`isqrt(x)` is the largest whole number whose square does not pass `x`. `x` is a `number` — not an
amount, a quantity, a rate or a date (E103); of a fraction it is the root of the whole part — and
cannot go below zero (E139, with an input that takes it there; `max(x, 0)` is a way out). What it
can come to is the roots of the ends of `x`'s range. There is no `sqrt` that
answers a fraction and no `pow`: a power is a product, `floor(n^1.5)` is `isqrt(n * n * n)`, and a
root to two places is `isqrt(x * 10000)`, which counts hundredths.

**There is no loop and no recursion.**

## 9. examples

```rule
| dest    | weight | total     | member   | -> fee |
| Okinawa | 2500g  | 40_000JPY | basic    | 0JPY   |
| Tokyo   | 1999g  | 12_000JPY | platinum | 400JPY |
```

`examples` is an **executable specification**: `rulec check` runs every row through the
reference evaluator, and a row that does not hold is E107, reported with the rows that fired.

**Every output must have a column** (E111). With two or more outputs, writing `->` before the
later output columns is optional; `rulec fmt` folds it to the canonical form.

A rule may have **more than one `examples` section** — the published worked examples in one,
the cases a person added in another. Each has a header of its own, and every one runs.

Its cells are held to the types of the columns they sit under, exactly as a table's are: a
heading that names nothing is E012, a value that is not one of the column's is E012, and a
literal the column cannot hold — `1lb` under `mass[g]`, a bare number where a unit is
required, an expected `800kg` under a `money[JPY]` output — is E103. An expected value that
cannot be read would otherwise leave nothing to hold the rule to.

An example naming an enum value is **not** a row for it: W111 still asks which values no table
names.

## 10. What cannot be written

- Nested objects (`order.shipping.prefecture`). Flatten at the boundary and pass the scalar in.
- Collections and iteration. A rule is one decision; the order and the repetition belong to
  the caller.
- Date arithmetic. Comparison and range only.
- State kept between calls. A `machine` makes claims about the calls a caller makes one after
  another, and the state stays with the caller: the function is passed it and answers the next
  one (§6.4).

Allowing any of the first three would make the completeness and overlap checks unable to
terminate; the fourth would take the replay of past records with it.

## 11. The canonical form

`rulec fmt` is the one and only formatter and it is idempotent. It

- aligns the columns of every table by East Asian width — a CJK or fullwidth character, and
  ℃ and ℉, are two columns — so Japanese lines up in a terminal and in any face that draws
  Japanese twice as wide as Latin,
- rewrites `→ ・ 、 ， ≦ ≧` and fullwidth digits to their ASCII forms,
- folds the `->` of the second and later output columns of `examples`,
- indents the lines under `machine` by two spaces and lines their values up in one column,
- indents the lines under `prerequisites` by two spaces and pads each value, and `start`, to the
  widest, so that `requires` and the start values line up in one column (§6.5),
- leaves the inside of a comment alone.

`rulec fmt --check` names the files that are not in canonical form and exits 1, which is how
it belongs in CI.

## 12. Reserved words

These cannot be used as a name (E009). A declaration whose name is one of them would be read
by the line-oriented parser as the start of a section and silently dropped, which is why it is
caught at parse time. An ASCII alias is not read by that parser, so it may be one; W121 says
when a target language takes it badly.

An enum **value** never starts a line, so it may be any of them but eleven: `not` `none`
`true` `false` `starts_with` (the words of a cell), `default` (the mark after a value in its
`enum` line), `after` (which divides a `never` line), and `empty` `exhausted` `by` `with`
(the words of a `fold` arm). A state may be called `initial`, `held` or `final`.

<!-- RESERVED -->
| | |
|---|---|
| line heads | `rule` `description` `import` `enum` `group` `inputs` `elements` `outputs` `derive` `define` `constraint` `table` `fold` `count` `sum` `sequence` `policy` `overrides` `clause` `source` `apply` `result` `machine` `scenario` `examples` |
| modifiers | `range` `round` `contract_only` `default` |
| cells | `not` `none` `starts_with` `true` `false` |
| rounding | `up` `down` `half_up` `half_down` `half_even` |
| functions | `min` `max` `allocate` |
| fold arms | `over` `next` `stop` `with` `take_unique` `take_first` `keep_max` `by` `empty` `exhausted` `held` |
| count and sum | `where` `of` (and `over`, above) |
| clause body | `when` `then` `always` |
| apply body | `except` (and `with`, above) |
| machine body | `carry` `initial` `final` `never` `after` `once` (and `over` and `held`, above; `always` and `when`, which a `clause` has too) |
<!-- /RESERVED -->

`step` (inside `rate[step 1%]`), `unique`, `first`, the type words (`money` `mass` `length`
`area` `volume` `duration` `temperature` `sound` `rate` `number` `bool` `date` `string`), the
money attributes (`incl_tax` `excl_tax`), `std`, `enums` (the heading of a file of enums, read
on its first line only) and `open` (the mark after an enum's last value, §3) are part of the
vocabulary but are told apart by position, so they are not reserved as names. A value may be
called `open` too: the mark is only the word that follows the last value. So are `prerequisites`
— a line head outside every block, and a name inside one, as `prerequisites(p) : number` under
`inputs` — and the words under it, `start`, `requires`, `or`, `nothing` and `ordered` (§6.5).

A test holds this table to `src/kw.rs`, which is the single place the vocabulary is defined.
