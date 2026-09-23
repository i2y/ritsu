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
| [E017](#e017) | error | A `constraint` is not shaped like this |
| [E018](#e018) | error | A `constraint` relates two inputs |
| [E019](#e019) | error | An example breaks a constraint |
| [E020](#e020) | error | The `elements` declaration is not right |
| [E021](#e021) | error | The `fold` is not written correctly |
| [E022](#e022) | error | The answer for a sequence with no elements is not declared |
| [E023](#e023) | error | The answer for a walk that reached the end is not declared |
| [E024](#e024) | error | Some verdict has no arm |
| [E025](#e025) | error | The examples have no column for the sequence |
| [E026](#e026) | error | The `sequence` is not written correctly |
| [E027](#e027) | error | The example names a sequence that is not there |
| [E028](#e028) | error | The `count` is not written correctly |
| [E029](#e029) | error | This column cannot be counted |
| [E030](#e030) | error | A `count` needs a range |
| [E031](#e031) | error | A rule cannot have both a `fold` and a `count` |
| [E032](#e032) | error | The declared enum and the imported one disagree |
| [E033](#e033) | error | A value of an imported enum has neither a row nor `default` |
| [E034](#e034) | error | A row label appears twice |
| [E035](#e035) | error | The target of `overrides` does not exist |
| [E036](#e036) | error | The target of `overrides` does not define the same output |
| [E037](#e037) | error | A cited fragment is not pinned |
| [E038](#e038) | error | A source fragment has changed |
| [E039](#e039) | error | There is no copy of a source |
| [E040](#e040) | error | The callee differs from its pinned digest |
| [E041](#e041) | error | The bindings of an apply do not match the callee |
| [E042](#e042) | error | A binding does not agree in type |
| [E043](#e043) | error | A value passed leaves the callee's range or constraint |
| [E044](#e044) | error | That rule cannot be applied |
| [E045](#e045) | error | A table that shares an output has two or more output columns |
| [E046](#e046) | error | A `clause` is not shaped like this |
| [E047](#e047) | error | Extra token after the declaration |
| [E048](#e048) | error | This type has no arithmetic |
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
| [E116](#e116) | error | A row's amount is not in the copy it cites |
| [E117](#e117) | error | A share without what a share needs |
| [E118](#e118) | error | The call is not written correctly |
| [E120](#e120) | error | A `from` does not fit the input's type |
| [E121](#e121) | error | The contract has no such path |
| [E119](#e119) | error | A row's boundary falls on the other side from the copy it cites |
| [W122](#w122) | warning | No input is projected from that shape |
| [E122](#e122) | error | The contract lets through a value the rule refuses |
| [W123](#w123) | warning | A row is reached only by values the contract does not let through |
| [W105](#w105) | warning | Shadowing that needs review: an earlier row hides part of a later one |
| [W110](#w110) | warning | A `first` table with no overlaps |
| [W111](#w111) | warning | A declaration is never used |
| [W116](#w116) | warning | No example uses this sequence |
| [W119](#w119) | warning | A pinned fragment is not cited |
| [W120](#w120) | warning | The copy states a value no row uses |
| [W118](#w118) | warning | No row of an applied table is reached in this apply |
| [W117](#w117) | warning | An exception with no effect |
| [W121](#w121) | warning | An alias collides with a word in a target language |
| [W115](#w115) | warning | No element can land on this verdict |
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
  %x(x) : bool
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

**Fix.** Start the line with a declaring word (`description / import / enum / group / inputs / elements / outputs / derive / define / constraint / table / fold / count / sum / sequence / result / examples / policy / overrides / clause / source / apply / shape`). A table row starts with `|`.

**Smallest reproduction**:

```rule
rule t(t) v1

= 1
```

Related codes: [E003](#e003), [E005](#e005)

## E005

`error` — **A word that cannot appear at this position**

**When.** The word at the head of the line is not in the vocabulary. The vocabulary has no synonyms: one English spelling each (§1.1).

**Fix.** Correct it to one of `description / import / enum / group / inputs / elements / outputs / derive / define / constraint / table / fold / count / sum / sequence / result / examples / policy / overrides / clause / source / apply / shape`. Business words belong in names and cells, not at the head of a line.

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
| - | true           |
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
|   | true           |
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
| w         | -> r(r) : bool |
| 0g..1000g | true           |
| >1000g    | false          |
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
| -    | true           |
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
| - | true           |
```

Related codes: [E011](#e011), [E013](#e013)

## E013

`error` — **No such import**

**When.** The target of `import` is not there. There are three kinds: the built-in namespace (`std/都道府県`, 47 values, is the only one for now), an enum in a `.proto` (`import proto "<file>" <Enum> -> <enum of this rule>`), and an enum in a JSON Schema (`import jsonschema "<file>" "<pointer>" -> <enum of this rule>`, OpenAPI included). For the last two it appears when the file cannot be read, when it holds no such enum, or when the line is not that shape. YAML is not read; point at a JSON form of it.

**Fix.** Correct it to `import std/都道府県`, or declare the enum in this file with `enum`. When importing from a file, the path is followed from the directory of the rule file, so write it relative to that. A JSON Schema pointer looks like `#/components/schemas/<name>`, and one that does not resolve comes back with the keys that are there.

**Smallest reproduction**:

```rule
rule t(t) v1

import std/nope
```

Related codes: [E012](#e012), [E032](#e032)

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
| r    | -> o(o) : money[円, incl_tax] |
| <=5% | 0円                           |
| -    | p × r                         |
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
| - | 100円                         |

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
| - | 100円                         |

result a = p
result a = p + 100円
```

Related codes: [E015](#e015)

## E017

`error` — **A `constraint` is not shaped like this**

**When.** A `constraint` line is not `input comparison input`: either there is no comparison, or a side is not a single name.

**Fix.** Write `constraint <input> <= <input>`; the comparisons are `<=`, `<`, `>=` and `>`. For `A = B`, write the two lines `A <= B` and `A >= B`.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : number  range >=0 <=10
  b(b) : number  range >=0 <=10

constraint a

outputs
  r(r) : bool

table j(j)
policy unique
| a | -> r(r) : bool |
| - | true           |
```

Related codes: [E018](#e018)

## E018

`error` — **A `constraint` relates two inputs**

**When.** A side of a `constraint` is not an input, or has a type with no order. A constraint says which combinations of the values the caller passes can happen, so both sides name something in `inputs`, and each is money, a quantity, a rate, a number or a date.

**Fix.** Name inputs on both sides. A derived or defined value is computed from inputs, so write the relation between those inputs; an enum and a boolean have no order.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : number  range >=0 <=10

constraint a <= r

outputs
  r(r) : bool

table j(j)
policy unique
| a | -> r(r) : bool |
| - | true           |
```

Related codes: [E017](#e017), [W111](#w111)

## E019

`error` — **An example breaks a constraint**

**When.** An example's inputs do not satisfy a `constraint`. The constraint declares that the combination does not happen, the completeness check believed it and demanded no row there, and the generated code refuses that input at the door. It is not an input an answer can be claimed for.

**Fix.** Correct the example's values — or, if that combination really does happen, the constraint is what is wrong and it goes.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : number  range >=0 <=10
  b(b) : number  range >=0 <=10

constraint a <= b

outputs
  r(r) : bool

table j(j)
policy unique
| a | b | -> r(r) : bool |
| - | - | true           |

examples
| a | b | -> r |
| 5 | 1 | true |
```

Related codes: [E017](#e017), [E018](#e018), [E101](#e101)

## E020

`error` — **The `elements` declaration is not right**

**When.** An `elements` line has no name, or there are two of them. A rule walks one sequence, and the fields of one of its elements are declared there (§15.56).

**Fix.** Write `elements 運賃行(fee_rows)`, and the fields of one element under it, declared the way `inputs` are. Two sequences mean two rules.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  k(k) : number  range >=0 <=10

elements ys(ys)
  m(m) : number  range >=0 <=10

outputs
  r(r) : bool

table j(j)
policy unique
| n | -> r(r) : bool |
| - | true           |
```

Related codes: [E021](#e021)

## E021

`error` — **The `fold` is not written correctly**

**When.** The heading is not `fold <verdict column> over <sequence>`, or an arm is not one of `next`, `stop`, `stop with <value>`, `take_unique <value>`, `take_first <value>`, `keep_max <value> by <key>`, or the column being folded is not an enum.

**Fix.** Correct the heading and the arms. A bare `take` cannot be written: whether **one and only one** element may be taken, or the first of several, is for the author to choose (§15.56).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  k(k) : number  range >=0 <=10

outputs
  r(r) : bool

table j(j)
policy unique
| n | -> r(r) : bool |
| - | true           |

fold r
  empty -> false
```

Related codes: [E020](#e020), [E022](#e022), [E023](#e023), [E024](#e024)

## E022

`error` — **The answer for a sequence with no elements is not declared**

**When.** A `fold` has no `empty -> <value>`. An empty sequence always turns up, and it is the case a hand-written loop most often forgets — usually by reading the first element and falling over.

**Fix.** Add `empty -> <value>`. What to answer is a business decision, and not one the tool can make.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  exhausted -> held
```

Related codes: [E023](#e023), [E024](#e024)

## E023

`error` — **The answer for a walk that reached the end is not declared**

**When.** A `fold` has no `exhausted -> <value>`: the answer when the sequence ran out and no element ended the walk. Answering with the value that was held is a choice, and it is made by writing it.

**Fix.** Add `exhausted -> <value>`; to answer with what is held, that is `exhausted -> held`.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
```

Related codes: [E022](#e022), [E024](#e024)

## E024

`error` — **Some verdict has no arm**

**When.** A verdict the table can produce has no arm in the `fold`: when an element lands on it, the walk has no move. It is the table's own completeness check, applied to the fold (§15.56).

**Fix.** Add the arm, or stop the table producing that value. The other direction — an arm for a verdict nothing can reach — is W115.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  empty -> 0円
  exhausted -> held
```

Related codes: [E022](#e022), [E023](#e023), [W115](#w115)

## E025

`error` — **The examples have no column for the sequence**

**When.** A rule that walks a sequence has `examples`, but the header has no column named after its `elements`. An example that does not say which sequence it walks is an example with no answer (§15.56).

**Fix.** Write the list with `sequence <name>`, add a column for the sequence to the examples header, and name it in the cell. A `sequence` with no rows is the example for a sequence with nothing in it.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

examples
| -> r |
| 0円  |
```

Related codes: [E026](#e026), [E027](#e027)

## E026

`error` — **The `sequence` is not written correctly**

**When.** The columns of a `sequence` do not line up with the fields of `elements`: a column that is not a field, a field left out, a `->`, no sequence to be a list of, two blocks with the same name, or a cell that is not a value (a range or a `-`).

**Fix.** Make the header the fields of `elements` as they are, and write one element's values per row. This is not a table: it is the list of values as they would really be passed.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

sequence s(s)
| m   |
| 3円 |
```

Related codes: [E025](#e025), [E020](#e020)

## E027

`error` — **The example names a sequence that is not there**

**When.** The cell in the sequence column names a `sequence` that is not declared, or holds something that is not a name at all.

**Fix.** Write a `sequence` under that name, or correct the cell to one that is written.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

examples
| xs   | -> r |
| nope | 0円  |
```

Related codes: [E025](#e025), [E026](#e026)

## E028

`error` — **The `count` is not written correctly**

**When.** The line is not `count <name>(<alias>) over <sequence> where <column> = <value>`: no `over`, no `where`, a sequence that `elements` does not declare, or an `=` with nothing on its right (§15.58).

**Fix.** Write it in that shape. The `= <value>` may be left out only for a bool column.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  b(b) : bool

outputs
  r(r) : bool

count h(h) where b  range >=0 <=10

table j(j)
policy unique
| n | -> r(r) : bool |
| - | true           |
```

Related codes: [E029](#e029), [E030](#e030), [E020](#e020)

## E029

`error` — **This column cannot be counted**

**When.** The column `where` names is not a value of one element (an input or a derived value is one per call, so counting it could only answer 0 or 1); or its values are not a closed set; or the value written is not one of that enum's; or an enum column was given no `= <value>` (§15.58).

**Fix.** Name a field of an element, or a column a per-element table produces. Writing the classification as a table is what puts the classification itself under the completeness check.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10
  ok(ok) : bool

elements xs(xs)
  b(b) : bool

outputs
  r(r) : bool

count h(h) over xs where ok  range >=0 <=10

table j(j)
policy unique
| h | -> r(r) : bool |
| - | true           |
```

Related codes: [E028](#e028), [E012](#e012)

## E030

`error` — **A `count` needs a range**

**When.** The `count` line has no `range >=0 <=<max>`, or no upper bound, or a negative lower one. The range means two things: the universe the completeness check quantifies over once the count is a column, and **the cap on the sequence** (§15.58).

**Fix.** Write it as `range >=0 <=100`. The generated code refuses a longer sequence at the door, the way it refuses a number outside its range.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  b(b) : bool

outputs
  r(r) : bool

count h(h) over xs where b

table j(j)
policy unique
| h | -> r(r) : bool |
| - | true           |
```

Related codes: [E028](#e028), [E112](#e112)

## E031

`error` — **A rule cannot have both a `fold` and a `count`**

**When.** One rule has both. They are two endings for the same walk, and a `fold` can stop partway: what a count means on a walk that stopped is not decided (§15.58).

**Fix.** To count, drop the `fold` and let a table judge the count. To fold, drop the `count`.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : number  range >=0 <=10

outputs
  r(r) : number  round down(1)

table j(j)
policy unique
| k   | -> d(d) : v |
| <=5 | a           |
| >5  | b           |

count h(h) over xs where d = a  range >=0 <=10

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0
  exhausted -> held
```

Related codes: [E021](#e021), [E029](#e029)

## E032

`error` — **The declared enum and the imported one disagree**

**When.** The values of the enum named by `import proto` or `import jsonschema` and the ASCII aliases of the rule's `enum` are not the same set. Values on either side alone are named, in both directions. It is usually the proto that gained one, and **it shipped as a compatible change made outside this rule** (§15.59).

**Fix.** Add the new value to the rule's `enum`. The proto has no Japanese in it, so the name is yours to decide. A value the contract dropped goes from the rule too. Once it is added, the table asks whether it needs a row (E033).

**Smallest reproduction**:

```rule
rule t(t) v1

import proto "tier.proto" Tier -> v
enum v(v) = one(one)

inputs
  x(x) : v

outputs
  r(r) : bool

table j(j)
policy unique
| x   | -> r(r) : bool |
| one | true           |
```

With `tier.proto` beside it:

```proto
syntax = "proto3";

enum Tier {
  TIER_UNSPECIFIED = 0;
  TIER_ONE = 1;
  TIER_TWO = 2;
}
```

Related codes: [E013](#e013), [E033](#e033), [E101](#e101)

## E033

`error` — **A value of an imported enum has neither a row nor `default`**

**When.** A value of an imported enum appears in no row and is not marked `default`. For a value you wrote yourself that is a forgotten line (W111); for a value that came through the contract it means **a change from elsewhere that nobody has read yet**, so it stops. With a default row the completeness check passes and the new value quietly takes the default amount (§15.59).

**Fix.** Add a row for it, or mark the value `default` in the declaration. `default` is a signature saying that falling through to the default row is what is meant — that someone decided the amount.

**Smallest reproduction**:

```rule
rule t(t) v1

import proto "tier.proto" Tier -> v
enum v(v) = one(one) | two(two)

inputs
  x(x) : v

outputs
  r(r) : bool

table j(j)
policy first
| x   | -> r(r) : bool |
| one | true           |
| -   | false          |
```

With `tier.proto` beside it:

```proto
syntax = "proto3";

enum Tier {
  TIER_UNSPECIFIED = 0;
  TIER_ONE = 1;
  TIER_TWO = 2;
}
```

Related codes: [E032](#e032), [W111](#w111), [E101](#e101)

## E034

`error` — **A row label appears twice**

**When.** Two rows of one table carry the same label before their first `|`. A label is how an `overrides` line, a record's trace and a later version name the row, so it is unique within its table.

**Fix.** Change one of the two labels.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
   | a     | -> x  |
r1 | true  | true  |
r1 | false | false |
```

Related codes: [E009](#e009), [E035](#e035)

## E035

`error` — **The target of `overrides` does not exist**

**When.** An `overrides` line names a table that does not exist, or one declared below this table, or a row label (`table:label`) the table has no row of. A line whose shape cannot be read is reported the same way. The exception is written after what it excepts, so a target is always above.

**Fix.** Name a table declared above, or one of its rows (label the row at its head and write `table:label`). Write the side that takes precedence later.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
overrides 無い表
| a     | -> x  |
| true  | true  |
| false | false |
```

Related codes: [E034](#e034), [E036](#e036)

## E036

`error` — **The target of `overrides` does not define the same output**

**When.** The table an `overrides` line names defines a different output from this table. Precedence exists only between definitions of the same output.

**Fix.** Name a table that defines the same output, or make this table's output column the same one.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table 甲(ko)
| a | -> x |
| - | true |

table 乙(otsu)
overrides 甲
| a | -> y |
| - | true |
```

Related codes: [E035](#e035), [E045](#e045)

## E037

`error` — **A cited fragment is not pinned**

**When.** A fragment cited with `@source fragment` has no `  fragment sha256:…` pin line under its `source` line; for a `file` source, the line carries no `sha256:…`. A law cited with no article (`@法` alone), and a fragment name, a citation or a `source` line whose shape cannot be read, are reported the same way (a file beside the rule may be cited whole, `@郵便`). A document's fragments are its tables, so `表3` (the third table in document order) and `table3` are the only names read (§15.82). A fragment the language cannot read as one word is quoted (`@osha "§1910.157"`). Without a pin, a revised copy passes check in silence (§15.68).

**Fix.** Once the transcribed rows are checked against the document, paste the `fix.text` line or run `rulec source pin <file.rule>` to pin the copy's digest.

**Smallest reproduction**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第1条
| a | -> x |
| - | true |
```

With `sources/law/000AC0000000001@2026-04-01/MainProvision-Article_1.xml` beside it:

```proto
<Article Num="1"><ArticleTitle>第一条</ArticleTitle><Paragraph Num="1"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>
```

Related codes: [E038](#e038), [E039](#e039), [W119](#w119)

## E038

`error` — **A source fragment has changed**

**When.** The pinned digest differs from the digest of the copy beside the rule. It fails in the pull request that refreshed the copy, and names the tables, clauses and rows that cite the fragment, which is all there is to reread.

**Fix.** Read the copy's diff; if the transcribed rows still hold, rewrite the pin line as `fix.text` says (`rulec source pin` writes it too). If the rows have to change, change them first.

**Smallest reproduction**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01
  第1条 sha256:0000000000000000

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第1条
| a | -> x |
| - | true |
```

With `sources/law/000AC0000000001@2026-04-01/MainProvision-Article_1.xml` beside it:

```proto
<Article Num="1"><ArticleTitle>第一条</ArticleTitle><Paragraph Num="1"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>
```

Related codes: [E037](#e037), [E039](#e039)

## E039

`error` — **There is no copy of a source**

**When.** The copy of a cited fragment is not beside the rule — `sources/law/<law id>@<date>/<element>.xml` for a law, `料金表.md.fragments/表3.tsv` beside the document for a document's table — or the `file` source itself cannot be read. check reads neither the network nor the document, so without a copy there is nothing to compare.

**Fix.** `rulec source fetch <file.rule>` fetches the fragment from e-Gov, the Japanese government's statute database, and takes the cited tables out of a document, into the copies beside the rule. Commit the copies.

**Smallest reproduction**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第2条
| a | -> x |
| - | true |
```

Related codes: [E037](#e037), [E038](#e038)

## E040

`error` — **The callee differs from its pinned digest**

**When.** The `apply` heading carries no `sha256:…`, or the digest it carries differs from the digest of the callee file beside the rule. When the callee is amended, the answers of the rule that applies it change too; without a pin, that change passes with nobody approving it (§15.69).

**Fix.** See with `rulec diff <old> <new>` how many answers of this rule move and by how much; once the movement is approved, rewrite the heading as `fix.text` says or run `rulec source pin <file.rule>` to pin the callee again.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule"
  a = n
  x -> y
```

With `呼び先.rule` beside it:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

Related codes: [E037](#e037), [E038](#e038), [E044](#e044)

## E041

`error` — **The bindings of an apply do not match the callee**

**When.** A callee input is left unbound, a binding or an output line names something the callee does not have, the name given to a callee output is already declared in this rule, or the `apply` block is not shaped as one. Substitution is written by binding every input explicitly, so a missing binding is a substitution left unwritten (§15.69).

**Fix.** Bind each callee input with one `<callee input> = <value>` line, and rename an output with `<callee output> -> <name>`. The callee's input and output names are listed in the message.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule" sha256:369102b8f803dc28
  b = n
  x -> y
```

With `呼び先.rule` beside it:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

Related codes: [E040](#e040), [E042](#e042), [E043](#e043)

## E042

`error` — **A binding does not agree in type**

**When.** The type of a bound value differs from the callee input's (unit, tax kind, an enum against a number). Between two enums: a value of this rule's enum stands for no value of the callee's (values spelled the same on both sides map by themselves), a `with` names a value neither side has, or a literal is not a value of the callee's type.

**Fix.** Make the types agree. For enums, `<input> = <value> with <this rule's value> -> <callee's value>, …` maps every value of this rule's enum; that is the shape of "'retirement' is read as 'end of term'".

**Smallest reproduction**:

```rule
rule t(t) v1

enum 種別(kind) = 甲(a) | 乙(b) | 丙(c)

inputs
  k(k) : 種別

outputs
  y(y) : number  round down(1)

apply 呼(c) = "区分の呼び先.rule" sha256:2e6f2e04c21cdbb3
  a = k
  x -> y
```

With `区分の呼び先.rule` beside it:

```proto
rule 区分の呼び先(enum_callee) v1

enum 区分(kind) = 甲(a) | 乙(b)

inputs
  a(a) : 区分

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
| a  | -> x |
| 甲 | 1    |
| 乙 | 2    |
```

Related codes: [E041](#e041), [E043](#e043)

## E043

`error` — **A value passed leaves the callee's range or constraint**

**When.** The interval of a bound value reaches outside the callee input's `range` (the point outside is shown as the witness), or a `constraint` of the callee does not follow from this rule's declarations. The callee's completeness was proved over that range and constraint; outside them there is no definition. There is no `fix.text`: adding a row to the callee belongs to another unit of approval (§15.69).

**Fix.** Narrow this rule's input range to the callee's, or define the region outside it in a clause of this rule; which is a business decision. For a constraint, declare the same relation with `constraint`.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule" sha256:369102b8f803dc28
  a = n
  x -> y
```

With `呼び先.rule` beside it:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

Related codes: [E041](#e041), [E042](#e042), [E101](#e101)

## E044

`error` — **That rule cannot be applied**

**When.** The callee cannot be read, does not pass `check` (the codes are listed), itself has an `apply`, walks a sequence (`elements`, `fold`, `count`), or has a table or clause that produces one of its own enums. A broken rule expanded is a broken rule, and a provision applied through another is written flattened to one level.

**Fix.** Fix the callee first. To apply a rule that itself applies another or walks a sequence, write its expansion into this rule.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "壊れた呼び先.rule" sha256:c4f9eba5b2949205
  a = n
  x -> y
```

With `壊れた呼び先.rule` beside it:

```proto
rule 壊れた呼び先(broken) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
| a   | -> x |
| <=5 | 1    |
```

Related codes: [E040](#e040), [E041](#e041)

## E045

`error` — **A table that shares an output has two or more output columns**

**When.** An output is defined by two or more tables (or tables are joined by `overrides`), and one of them has two or more output columns. A row that bundles two definitions leaves the other value's origin undecided when only one is overridden, and the generated code would have to write the row's condition twice.

**Fix.** Move the second output to a table of its own.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table 甲(ko)
| a | -> x | y    |
| - | true | true |

table 乙(otsu)
overrides 甲
| a    | -> x  |
| true | false |
```

Related codes: [E036](#e036), [E105](#e105)

## E046

`error` — **A `clause` is not shaped like this**

**When.** The `clause` heading lacks a name, the `->` or the output; the `when` or `then` line is missing or appears twice; or the `when` condition is not `<column> <cell> and …` (a part without a column, a column twice, a column without a condition). A clause is a one-row table, so it needs exactly one condition and one value.

**Fix.** Under `clause <name>(<alias>) -> <output>`, write one `when <column> <cell> and …` line (`when always` when there is no condition) and one `then <value>` line. Add `overrides <target>` when it takes precedence over something.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

clause 例外(exception) -> x
  then true
```

Related codes: [E008](#e008), [E035](#e035), [E045](#e045)

## E047

`error` — **Extra token after the declaration**

**When.** A declaration line — an input, an output, a `derive` or a `count` — holds a word that belongs to none of `range`, `round` and `contract_only`. The readers look along the line for the word they want and step over everything else, so such a word used to be dropped in silence: a tax flag written after the range, as in `range >=0円 <=10000円 incl_tax`, or what is left of a bound whose unit did not lex as one. A range is the universe the completeness proof quantifies over and the entry guard of the generated code, so a bound lost this way is answered "complete" with one side missing.

**Fix.** Remove the word, or write it in the form the declaration takes. A tax flag or a step goes inside the type's brackets (`money[円, incl_tax]`, `rate[step 0.1%]`); a range is `range >=<value> <=<value>`; a rounding is `round <mode>(<grid>)`.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : money[円, incl_tax]  range >=0円 <=10000円 incl_tax

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

Related codes: [E011](#e011), [E103](#e103), [E104](#e104)

## E048

`error` — **This type has no arithmetic**

**When.** A type that is ordered but has no arithmetic is used with `+ - × ÷`. There are three: `date`, `temperature[℃]`/`temperature[℉]`, and `sound[dB]`. A ℃ has a displaced zero, so `気温 × 2` means nothing; a decibel is a logarithm, so adding two of them is not two sounds' worth; and a date is a calendar day, with no type to hold the result of subtracting one. All three appear in rules only as thresholds, so comparison and `range` are kept and the arithmetic is dropped.

**Fix.** Compare it against a threshold, or write it in a `range`. Where a difference or a multiple is itself the rule, take the computed value as an input, or look it up in a table. The days between two dates are counted on the calling side and passed in as a `number` or a `duration`.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  甲(a) : temperature[℃]  range >=0℃ <=40℃
  乙(b) : temperature[℃]  range >=0℃ <=40℃

outputs
  x(x) : bool

derive 差(gap) : temperature[℃] = 甲 - 乙  range >=-40℃ <=40℃

table 表(t1)
policy unique
| 差 | -> x |
| -  | true |
```

Related codes: [E103](#e103), [E112](#e112), [E115](#e115)

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
| a | true           |
| b | false          |
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
| w       | -> r(r) : bool |
| -       | true           |
| <=1000g | false          |
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
| - | 100円                         |

result r = p + w
```

Related codes: [E108](#e108), [E112](#e112)

## E104

`error` — **A numeric output declares no rounding**

**When.** A quantity, money or rate output has no `round`. Unless the fraction is declared, the generated code settles it silently. When the expression can produce a fraction, the message shows in yen how far the choice moves the answer.

**Fix.** Add rounding to the output declaration, e.g. `round up(10円)`. There are five directions (`up`, `down`, `half_up`, `half_down`, `half_even`), pinned down for negative values as well (§7.3).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]

table j(j)
policy unique
| x     | -> r(r) : money[円, incl_tax] |
| true  | 100円                         |
| false | 200円                         |
```

Related codes: [E106](#e106), [E103](#e103)

## E105

`error` — **Overlapping rows: the same input matches two or more rows**

**When.** In a `policy unique` table, an input matching both rows was actually constructed. An overlap that could not be constructed falls to W114 instead. A pair that meets only on a combination the tables above never produce together is not reported, which is the reading that also lets E102 call such a row dead.

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
| x | y     | -> r(r) : money[円, incl_tax] |
| a | -     | 100円                         |
| - | true  | 200円                         |
| b | false | 300円                         |
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
| x     | -> r(r) : money[円, incl_tax] |
| true  | 1451円                        |
| false | 1000円                        |
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
| x     | -> r(r) : money[円, incl_tax] |
| true  | 100円                         |
| false | 200円                         |

examples
| x    | -> r  |
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
| -   | 0円                           |
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
| a | true           |
| b | false          |
| c | true           |
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
  s(s) : string?

outputs
  r(r) : bool

table j(j)
policy unique
| s               | -> r(r) : bool |
| starts_with "a" | true           |
```

Related codes: [E101](#e101), [E105](#e105)

## E111

`error` — **The examples have no column for an output**

**When.** `examples` writes only some of the declared outputs. Agreement across the implementations stays green when they all carry the same mistake, so **only a human-written expectation can break it**. Rounding for multiple outputs really did go missing in every one at once.

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
| x     | -> ok(ok) : bool | fee(fee) : money[円, incl_tax] |
| true  | true             | 100円                          |
| false | false            | 0円                            |

examples
| x    | -> ok |
| true | true  |
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
| gap   | -> r(r) : bool |
| <=0円 | false          |
| >0円  | true           |
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
| true   | true           |
| false  | false          |
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
| r      | -> o(o) : bool |
| <=0.5% | true           |
| -      | false          |
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
| - | true           |
```

Related codes: [E103](#e103), [E108](#e108)

## E116

`error` — **A row's amount is not in the copy it cites**

**When.** The output value of a row is nowhere in the copy the row or its table cites with `@source 表1` (§15.82). Only amounts are compared: a threshold is rewritten as it is transcribed (`1,949,000円まで` becomes `<=1949000円`) and an amount is not. The copy is what `rulec source fetch` took out of the document; check does not read the document itself.

**Fix.** Reread the copy and correct the amount. For a mistyped digit W120 usually comes with it, naming the value left unused. If the value came from somewhere else — a later notice, a correction, an answer from a person — take the citation off this row and write where it came from in a comment at the end of it, which `rulec doc` shows to the approver.

**Smallest reproduction**:

```rule
rule t(t) v1

source 料金表 = file "料金表.md" sha256:75465b330d123ab8
  表1 sha256:0a95cedbd7311274

inputs
  a(a) : bool

outputs
  x(x) : money[円]  round down(1円)

table 表(t1)  @料金表 表1
policy unique
| a     | -> x  |
| true  | 990円 |
| false | 890円 |
```

With `料金表.md` beside it:

```proto
# 料金表

| あて先 | 運賃 |
|---|---|
| 近畿 | 990円 |
| 関東 | 880円 |
```

With `料金表.md.fragments/表1.tsv` beside it:

```proto
あて先	運賃
近畿	990円
関東	880円
```

Related codes: [W120](#w120), [E038](#e038), [E107](#e107)

## E117

`error` — **A share without what a share needs**

**When.** The three of `allocate(<amount>, <running total>, <whole>)` are not the shape a share needs. All three are names with declared ranges, the amount and the running total cannot be negative, the whole is positive, and a `constraint` says the running total never passes the whole (§15.102).

**Fix.** Write what is missing. Without `constraint <running total> <= <whole>` a share can exceed the amount being handed out and the lines no longer add up to the total. Below zero the targets disagree about which way to round (§7.1).

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  値引き(off) : money[円]  range >=0円 <=1000円
  ここまで(upto) : money[円]  range >=0円 <=1000円
  合計(base) : money[円]  range >=1円 <=1000円

outputs
  o(o) : money[円]  round down(1円)

derive 配分(share) : money[円] = allocate(値引き, ここまで, 合計)  range >=0円 <=1000円

result o = 配分
```

Related codes: [E115](#e115), [E108](#e108)

## E118

`error` — **The call is not written correctly**

**When.** A call to a function that does not exist, or with the wrong number of arguments. The calls are `min(a, b)`, `max(a, b)`, `allocate(<amount>, <running total>, <whole>)` and the five rounding modes (`down(x, 1円)` and the rest) (§2.3).

**Fix.** Check the spelling and the count. A spare argument is dropped on the floor and a missing one leaves no answer — both passed unnoticed until §15.102, and only the generator ran out of cases.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=100
  d(d) : number  range >=1 <=100

outputs
  o(o) : number  round down(1)

define r(r) : number = min(n, d, n)

result o = r
```

Related codes: [E103](#e103), [E115](#e115)

## E120

`error` — **A `from` does not fit the input's type**

**When.** What a `from` yields does not fit the input that takes it (§15.125). `any` and `all` yield a `bool` and `count` yields a `number`. It is also this code when the type at the end of the path does not fit the input — a field the contract calls a string taken by a `number` input — when `any`, `all` or `count` would walk something that is not a collection, and when the value of a `where` does not fit the field. A contract says how a value travels: an enum and a date arrive as strings, and money and a quantity as whole numbers in the unit the rule declares.

**Fix.** Correct the type or the `from`. A count is taken by a `number` input with a range, whether the elements passed by a `bool`, and the value itself by `from <shape>.<field>`. A `where` value carries no unit: a contract has none, and comparing a scaled number with a raw one is the thing this must not do quietly.

**Smallest reproduction**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool  from count order.lines

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

With `order.json` beside it:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","items":{"type":"object","properties":{"category":{"type":"string"}}}}}}}}
```

Related codes: [E121](#e121), [W122](#w122), [E103](#e103)

## E121

`error` — **The contract has no such path**

**When.** A `from` path is not in the contract of the `shape` it starts at (§15.125). Three shapes of it: the first word is not the name of a `shape`, a field along the way is not there, or the field a `where` tests is not a field of an element. The message says how far it resolved and which names were there. The contract may be a `.proto` or a JSON Schema, is read on every `check` like `import proto`, and carries no pin.

**Fix.** Correct the spelling, or rewrite the path if the contract moved. **This firing is the point**: a contract that renamed a field goes unnoticed in hand-written glue until it runs, and stops the build here.

**Smallest reproduction**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool  from order.nope

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

With `order.json` beside it:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","items":{"type":"object","properties":{"category":{"type":"string"}}}}}}}}
```

Related codes: [E120](#e120), [W122](#w122), [E032](#e032)

## E119

`error` — **A row's boundary falls on the other side from the copy it cites**

**When.** A threshold of a row that cites puts its boundary value on the other side from the copy (§15.124). A threshold is rewritten as it is transcribed (`1,949,000円まで` becomes `<=1949000円`) so the text cannot be compared; what is compared is **which of the two bands the boundary value falls in**. The copy's `60cm以下` and `60cmを超え` both put 60cm in the band below, and so do `<=60cm` and `>60cm`. A number the copy bounds with no word (`18 to 20`, `60〜80`), with the word in another column (`円以上` over its own column, as an insurance premium table writes it), or with words on both sides, is left alone.

**Fix.** Reread the copy and correct it. `fix.text` is this cell with that one boundary's side swapped and nothing else: the direction is the table's geometry, not the copy's to decide, so only `<` and `<=` are exchanged. One boundary mistranscribed is reported on both of the rows that share it. If the boundary came from somewhere else — a later notice, a proviso in the text — take the citation off this row and say in a comment at the end of it where it came from.

**Smallest reproduction**:

```rule
rule t(t) v1

source 寸法表 = file "寸法表.md" sha256:a19333e262c10371
  表1 sha256:a5c05813ad703b8e

inputs
  a(a) : length[cm]  range >=1cm <=80cm

outputs
  x(x) : money[円]  round up(10円)

table 表(t1)  @寸法表 表1
policy unique
| a             | -> x   |
| <60cm         | 1410円 |
| >=60cm <=80cm | 1710円 |
```

With `寸法表.md` beside it:

```proto
# 寸法表

| サイズ | 運賃 |
|---|---|
| 60cmまで | 1410円 |
| 60cmを超え80cm以下 | 1710円 |
```

With `寸法表.md.fragments/表1.tsv` beside it:

```proto
サイズ	運賃
60cmまで	1410円
60cmを超え80cm以下	1710円
```

Related codes: [E116](#e116), [W120](#w120), [E105](#e105)

## W122

`warning` — **No input is projected from that shape**

**When.** A `shape` is declared and no input says `from <that name>.…` (§15.125). The contract is read and holds nothing.

**Fix.** Use it or delete it. A contract that is only read makes the next reader believe this rule is held to it; what is held to it is the inputs that say `from`.

**Smallest reproduction**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

With `order.json` beside it:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","items":{"type":"object","properties":{"category":{"type":"string"}}}}}}}}
```

Related codes: [E121](#e121), [W111](#w111)

## E122

`error` — **The contract lets through a value the rule refuses**

**When.** A value read with `from` can pass the contract's validation and still be refused by the input's declaration (§15.132). What is compared: the range of a number (Protovalidate's `gte`, `lte` and the rest; JSON Schema's `minimum` and `maximum`), the length of a collection (`min_items` and `max_items`; `minItems` and `maxItems`), the values of an enum (`string.in`; `enum`), and JSON Schema's `required`. A proto3 number field with no rule arrives as 0 when it is left unset, so an input that does not take 0 stops here. A message field that is not `required`, and an `optional` field, may be left unset, and the value under it then arrives as its default (0, "", no elements) with no rule applied; a date read from a `.proto` string is held to whether "" passes (§15.133). A rule this does not read, such as a CEL expression, is read as not there: the contract is then read wider than it is, so this may speak where it did not need to, and never stays quiet where it should have spoken.

**Fix.** Which side to change is a person's decision. If the value cannot occur, narrow the contract: `fix.text` is the annotation to write there (`narrow_contract`). If it can, widen the rule's range or add the value to the enum, and decide what it answers. Some preconditions cannot be written in a contract, such as a floor on how many elements a `where` picks out; `fix.kind` is then `none`. When the value read may be missing, make the input `T?`: a missing value is then read as none.

**Smallest reproduction**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : number  range >=0 <=5  from count order.lines

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

With `order.json` beside it:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","maxItems":10,"items":{"type":"object"}}},"required":["lines"]}}}
```

Related codes: [W123](#w123), [E121](#e121), [E032](#e032)

## W123

`warning` — **A row is reached only by values the contract does not let through**

**When.** A cell of a row tests an input read with `from`, and nothing the cell accepts passes the contract's validation (§15.132). Only what passed the contract arrives, so no request or message reaches the row. Only a column of the input itself is compared; a column derived from it is not.

**Fix.** If the contract will not widen, delete the row and bring the input's range in line with the contract. If the row is kept for a widening that is planned, leave it: `check --diff-base` in CI reports only the ones that are new.

**Smallest reproduction**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : number  range >=0 <=20  from count order.lines

outputs
  x(x) : bool

table 表(t1)
policy unique
| a    | -> x  |
| <=10 | true  |
| >10  | false |
```

With `order.json` beside it:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","maxItems":10,"items":{"type":"object"}}},"required":["lines"]}}}
```

Related codes: [E122](#e122), [E102](#e102), [W111](#w111)

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
| x | y    | -> r(r) : money[円, incl_tax] |
| a | -    | 100円                         |
| - | true | 200円                         |
| - | -    | 300円                         |
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
| a | true           |
| b | false          |
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
| x     | -> r(r) : bool |
| true  | true           |
| false | false          |
```

Related codes: [E101](#e101), [E012](#e012)

## W116

`warning` — **No example uses this sequence**

**When.** A `sequence` is written and no example names it. A sequence runs only when an example names it, so this one never runs (§15.56).

**Fix.** Add the example that walks it, or drop the sequence. Written and unused is usually the trace of an example left unwritten.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

sequence s(s)
| k   |
| 3円 |
```

Related codes: [E027](#e027), [W111](#w111)

## W119

`warning` — **A pinned fragment is not cited**

**When.** A pin line sits under a `source` line, but no `@` in the rule cites that fragment. It is what remains after a citation was removed.

**Fix.** Remove the pin line; `rulec source pin` does.

**Smallest reproduction**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01
  第1条 sha256:ce31217424a10206
  第2条 sha256:0000000000000000

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第1条
| a | -> x |
| - | true |
```

With `sources/law/000AC0000000001@2026-04-01/MainProvision-Article_1.xml` beside it:

```proto
<Article Num="1"><ArticleTitle>第一条</ArticleTitle><Paragraph Num="1"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>
```

Related codes: [E037](#e037)

## W120

`warning` — **The copy states a value no row uses**

**When.** A cell of the copy a table cites whole with `@source 表1` is nothing but a number, and no row uses that value (§15.82). A dropped row does not show up in the completeness check: its inputs fall into one of the rows that remain. Only cells that are nothing but a number are asked about, so `2026年4月1日改定` is not counted as an amount, and a row that merges what the copy lists — `<=3kg` over its `1kg`, `2kg` and `3kg` — accounts for all of them.

**Fix.** Compare the table with the copy and check that no row was left out; if a revision added a row, transcribe it. If the table transcribes only part of the fragment — one origin of a tariff sheet that lists several — move the citation from the `table` line onto the end of each row that came from it. A row's citation says only where that row came from, so the rest goes unasked while the amounts are still held to the copy (E116).

**Smallest reproduction**:

```rule
rule t(t) v1

source 料金表 = file "料金表.md" sha256:75465b330d123ab8
  表1 sha256:0a95cedbd7311274

inputs
  a(a) : bool

outputs
  x(x) : money[円]  round down(1円)

table 表(t1)  @料金表 表1
policy unique
| a | -> x  |
| - | 990円 |
```

With `料金表.md` beside it:

```proto
# 料金表

| あて先 | 運賃 |
|---|---|
| 近畿 | 990円 |
| 関東 | 880円 |
```

With `料金表.md.fragments/表1.tsv` beside it:

```proto
あて先	運賃
近畿	990円
関東	880円
```

Related codes: [E116](#e116), [W119](#w119)

## W118

`warning` — **No row of an applied table is reached in this apply**

**When.** **Every** row of an applied table or clause is unreachable in this rule: what is bound never reaches its conditions, or other definitions of this rule (a clause with `overrides apply:table`, say) take precedence over all of it. Rows unreachable one by one draw no word: a callee's table is usually written for a wider range than this rule's, and `doc` lists those rows as unused by this apply (§15.69).

**Fix.** If this apply does not need the table, leave it out with `except <table>`. If it should be used, look at the bindings and the `with` mapping.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule" sha256:369102b8f803dc28
  a = n
  x -> y

clause 特例(special) -> 呼:x
  when always
  then 3
  overrides 呼:表
```

With `呼び先.rule` beside it:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

Related codes: [E102](#e102), [W117](#w117)

## W117

`warning` — **An exception with no effect**

**When.** No row of this table meets a row of what its `overrides` line names. Precedence decides which wins when an input matches both, so with no meeting rows the line decides nothing. It is the usual sign of a proviso transcribed so that it no longer carves out part of the main rule.

**Fix.** Compare with the source and fix the conditions of this table's rows or of the target's. If they really never meet, drop the `overrides` line.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 甲(ko)
   | a     | -> x  |
r1 | true  | true  |
r2 | false | false |

table 乙(otsu)
overrides 甲:r1, 甲:r2
| a    | -> x  |
| true | false |
```

Related codes: [E035](#e035), [E105](#e105)

## W121

`warning` — **An alias collides with a word in a target language**

**When.** An ASCII alias is a keyword of one of the targets, or a name that language already uses (§15.103). An alias becomes a function, a parameter, a type or a member there.

**Fix.** A keyword means the generated code for that language does not compile (`type` as an input's alias breaks Rust). A name that is taken means the rule's function or an enum's type hides it (`sum` as the rule's alias hides Python's builtin). A parameter or a local shadows nothing outside its own body, so there the warning is raised only for a keyword. For a target you do not generate, leave it.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  種別(type) : number  range >=0 <=10

outputs
  o(o) : number  round down(1)

table j(j)
policy first
| 種別 | -> o(o) : number |
| >=5  | 1                |
| -    | 0                |
```

Related codes: [E009](#e009), [E011](#e011)

## W115

`warning` — **No element can land on this verdict**

**When.** A `fold` has an arm for a verdict no row produces. It is the other side of E024: not a hole but an arm nothing reaches, and more often the trace of a table that changed than of an arm written by mistake (§15.56).

**Fix.** Look again at the table's rows, or drop the arm. Which of the two is right is decided by reading the table, not this message.

**Smallest reproduction**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k | -> d(d) : v |
| - | a           |

fold d over xs
  a -> take_first k
  b -> next
  empty -> 0円
  exhausted -> held
```

Related codes: [E024](#e024)

## W114

`warning` — **Unconfirmed overlap: an input may match both rows**

**When.** Two rows of a `policy unique` table may overlap, but no input producing that was constructed and infeasibility was not proven either. Derived values sharing an input (§15.126) and the thresholds inside a boolean `define` (§15.127) no longer fall here: the elimination decides them. What is left is what it cannot decide because it works over the rationals — `倍` above is always even and never exactly `5円`, but `2.5円` is a rational. A system past the cap of 400 inequalities and one that spans two units are the same: not proven, which is not the same as possible.

**Fix.** If an order satisfying both conditions can exist, fix the rows: the outputs differ, so a match is a contradiction. If none can exist, leave it — the generated code carries a guard that returns an error rather than silently picking the earlier row.

**Smallest reproduction**:

```rule
rule t(t) v1

inputs
  a(a) : money[円]  range >=0円 <=10万円

outputs
  r(r) : bool

derive 倍(d) : money[円] = a + a  range >=0円 <=20万円

define 上(up) : bool = 倍 >= 5円
define 下(dn) : bool = 倍 <= 5円

table j(j)
policy unique
| 上    | 下    | -> r(r) : bool |
| true  | -     | true           |
| -     | true  | false          |
| false | false | false          |
```

Related codes: [E105](#e105), [W105](#w105), [E109](#e109)
