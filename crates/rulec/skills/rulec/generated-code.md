# The generated code

`rulec gen` writes ordinary Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm — a module in each,
a package in Go's case, and one query plus one function in SQL's. NumPy is the twelfth and the odd one out: it is
not generated code at all but the rule as data, read by one fixed evaluator (see below). There is no runtime to install and nothing to
configure: a function takes the declared inputs and returns the declared outputs, and the
query takes a relation of them. This file says what shape that code has, what it guarantees,
and how to call it.

To get the calling convention without reading the code at all, ask for it:

```console
$ rulec api rules/クーポン一枚.rule
```

One JSON object with the module and function names, the parameters in order with their
brands, units and ranges, the outputs with their rounding, the enum members under the
spelling each language gives them, and the errors the code can raise. The shape is defined in
[formats.md](formats.md), and a test holds every name in it to the file the generator wrote.

---

## What it guarantees

**No dependencies.** The generated Python imports `enum` and `typing`; the generated
TypeScript and JavaScript import nothing at all; the generated Rust imports nothing outside `std` and needs
no `Cargo.toml`; the generated Ruby requires nothing at all and needs no gem; the generated
PHP requires nothing at all and needs no composer; the generated
Swift imports nothing at all and needs no package manifest; the generated Go imports `fmt`;
the generated Java imports `java.util.List` and `java.util.ArrayList` and needs no Maven and
no Gradle; the
generated Wasm module imports nothing, not even WASI.
The `go.mod` lists nothing but the module itself. `rulec test` runs the Go side with `GOPROXY=off`, so "no dependencies"
is a checked property rather than a claim. The server that offers the rule as an MCP tool
([below](#the-rule-as-an-mcp-tool)) imports the standard library alone in Python and node's
own modules alone in JavaScript, and the generated SQL calls no function of its own: its
rounding is arithmetic inside the query, in the file that declares a function as much as in
the one that does not. **NumPy is the one exception, and a deliberate one**: the plan is
data, and the evaluator that reads it imports `numpy`. That is the target — a host that wants
whole columns decided at once already has numpy, and the dependency is its own.

**Deterministic.** The same `.rule` and the same rulec version produce the same bytes. The
formatter is built in — no `gofmt` or `black` runs afterwards, because that would make the
output depend on the version of a tool installed on the machine. `gofmt -l` being empty and
`ruff check --select E,W` being silent (line length aside) are both tested.

**Branches match the rule line for line.** Every row of every table becomes one branch, in
order, with the original cells quoted in a comment (`# row 3: 近畿圏 | S100 | 1620円`) — in
SQL, one `WHEN` of one `CASE`, with the same comment. A
condition that an earlier branch already settled is still written out (`elif True:`), because
reading the generated code against the rule side by side is the only way it is meant to be
read. The branches are written once, in the twin that also returns
[the rows that matched](#the-rows-that-matched); the function you call delegates to it.

**Units live in the type** wherever the language has one to hold them. Rust uses a newtype,
Swift a one-field struct, Go a defined type, TypeScript a branded `bigint`, Python a
`NewType`, and the Wasm module is the Rust one; Ruby, PHP, JavaScript, Java, SQL and the NumPy
plan have nowhere to put a unit, so they document it instead — for NumPy in the plan itself,
where every column carries its unit and its scale, and in what `rulec api` prints. PHP and Java still declare the *kind* of
every parameter — `int`, `string`, `bool`, the enum itself — which is why their entry guard
asks only about the range.
`YenInclTax` and `YenExclTax` are different types, and mixing them fails to compile in Rust,
Swift, Go and TypeScript, and fails type checking in Python. Every value is an integer in its declared unit; no floating point appears
anywhere.

**Rounding is explicit and settled for negatives too.** Python's `//` rounds toward −∞ and
Go's integer division toward zero, so neither language's division is used. Each side carries
its own `_round_up` / `_round_down` / `_round_half` / `_round_bankers` (`roundUp`, … in Go),
and a unit-vector file next to the generated code checks them against the reference on every
run. The query carries no helper: each mode is arithmetic over the bound value — `CASE`,
`ABS` and `%`, which mean the same in both dialects — and the same unit vectors run over
those expressions.

---

## Calling it

### Python

```python
def coupon_step(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> Output:
```

Parameters are the rule's inputs in declaration order, named by their ASCII aliases. With one
output the function returns that value; with two or more it returns a `NamedTuple` called
`Output` whose fields are the outputs in declaration order.

```python
from coupon_step import coupon_step, CouponKind

out = coupon_step(
    subtotal=10000,
    applied=0,
    kind=CouponKind.PERCENT,
    rate=10,
    face=0,
    dup=False,
)
print(out.ok, out.raw)
```

An enum member is the alias in upper case (`CouponKind.PERCENT`), and its value is the
Japanese name from the rule (`"率引き"`), which is what the wire format and the logs use.

Two exceptions can come out, and the difference between them matters:

- **`RuleInputError`** (a `ValueError`) — the caller broke the contract: a value outside its
  declared range, or something that is not a member of the enum. Fix the call site.
- **`RuleContradictionError`** (an `AssertionError`) — the *rule* contradicted itself. This
  is the guard described below. It is never the caller's fault.

### TypeScript

```ts
export function coupon_step(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: boolean): Output
```

**Every number is a `bigint`.** The overflow proof (E108) is against int64, and a JavaScript
`number` is exact only to 2^53, so using one would put a silently wrong answer above nine
quadrillion into the one place this tool exists to keep honest.

Each unit is a *branded* bigint — `type YenInclTax = bigint & { readonly __rulec: "YenInclTax" }`
— which costs nothing at run time and still refuses a tax-inclusive amount where a
tax-exclusive one was meant. Construct one with `as`.

```ts
import { coupon_step, CouponKind } from "./coupon_step.ts";
import type { YenInclTax, Rate } from "./coupon_step.ts";

const out = coupon_step(
  10000n as YenInclTax,
  0n as YenInclTax,
  CouponKind.PERCENT,
  10n as Rate,
  0n as YenInclTax,
  false,
);
console.log(out.ok, out.raw);
```

An enum is a frozen object plus a union type, not a TypeScript `enum`. That keeps the whole
file to **erasable syntax**, so `node file.ts` runs it with no build step and no `tsconfig`;
a `tsc` build works just as well. The member spelling is the alias in upper case
(`CouponKind.PERCENT`), and its value is the Japanese name, the same as in Python.

The two error classes are `RuleInputError` and `RuleContradictionError`, with the same
meanings as in Python.

### JavaScript

```js
export function coupon_step(subtotal, applied, kind, rate, face, dup)
```

The TypeScript with its types taken off, as an ES module (`coupon_step.mjs`): the same
branches, the same helpers, the same `bigint` for every number, held to the same vectors by
`rulec test`. It runs with `node` alone and in a browser as it stands. The enum objects,
`parseCouponKind`, and the two error classes are the same as in TypeScript; what is gone is
the brand, so nothing catches a tax-exclusive amount passed where a tax-inclusive one was
meant — the position Ruby is in.

```js
import { coupon_step, CouponKind } from "./coupon_step.mjs";

const out = coupon_step(10000n, 0n, CouponKind.PERCENT, 10n, 0n, false);
console.log(out.ok, out.raw);
```

### Rust

```rust
pub fn coupon_step(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> Result<Output, RuleError>
```

Every number is an `i64`, which is the type the overflow proof (E108) is stated in. A unit is
a newtype over it — `pub struct YenInclTax(pub i64)` — so the compiler refuses a
tax-exclusive amount where a tax-inclusive one was meant, at no run-time cost. Construct one
with `YenInclTax(10000)` and read it back with `.0`.

```rust
use coupon_step::{coupon_step, CouponKind, YenInclTax, Rate};

let out = coupon_step(
    YenInclTax(10000),
    YenInclTax(0),
    CouponKind::Percent,
    Rate(10),
    YenInclTax(0),
    false,
)?;
println!("{} {}", out.ok, out.raw.0);
```

An enum is a plain Rust enum whose members are the aliases in PascalCase
(`CouponKind::Percent`); `as_str()` gives the Japanese name that the wire format uses, and
`CouponKind::parse(&str)` reads one back.

**There is no entry guard on an enum input**, unlike Python, TypeScript, JavaScript, Ruby, SQL and NumPy — the last of which checks a whole column with one `np.isin`. A value
of a Rust enum type is one of its variants by construction, so the check the others have to
make at run time is already made by the compiler. Swift, PHP, Java and Go are in the same
position, and so is
the Wasm module, which is the Rust one behind a door that turns an unknown value into an
`error` line before the module is reached.

Errors come back as `Err(RuleError)`, whose two variants carry the same distinction as
Python's two exception classes: `RuleError::Input { what, value }` is a contract violation by
the caller, and `RuleError::Contradiction { what }` is the runtime guard described below.
`what` is the sentence, `value` the number that was refused (`None` when the refusal is not
about one), and `Display` puts them together — nothing is formatted on the refusing path,
which is what lets a model checker walk it (below).

It compiles with `rustc` alone — `rustc --edition 2021 -O coupon_step_runner.rs` builds both
the rule and its runner through a `#[path] mod`, with no project file and nothing to fetch.

#### The proofs

Beside the module, `coupon_step_proof.rs` holds proof harnesses for the [Kani Rust
Verifier](https://model-checking.github.io/kani/). Everything in it is behind `#[cfg(kani)]`,
so `rustc` never reads it; `kani coupon_step_proof.rs` does, and so does `rulec test
--proofs` — a pass of its own, skipped and said so when `kani` is not on PATH. It is behind
a flag because it is the one pass whose cost is noticeable: on the corpus of 48 rules it
adds about 210 seconds to a run that otherwise takes seconds. `rulec api` names
the file under `rust.proof` and every harness under `rust.harnesses`.

What it holds, over **every** input in the declared domain rather than the vectors:

| what | how it is checked |
|---|---|
| completeness (E101) | the `unreachable!` that closes every table — a reachable panic is a gap |
| the W114 guards | that `RuleError::Contradiction` is never returned: the pair the checker could not decide either closes here or comes back as a counterexample |
| int64 (§7.4) | Kani checks arithmetic overflow by default, on the code that ships |
| overlap (E105) | `rows_<table>` counts the rows that match: `policy unique` demands exactly one, `policy first` at least one |

The last one is separate because the overlap is *not* in the artifact: the if/else chain has
already settled the priority, so the rows are counted beside it, from the same conditions.
The harness assumes the declared domain — every `range`, and every `constraint` — and calls
the generated function unchanged.

A rule that walks a sequence gets the first three; its per-element tables and the ones that
read a count cannot be replayed outside the walk, so no `rows_` is written for them. Two
kinds of rule get no harness at all, and the file says which: one with a `string` input,
which the harness cannot quantify over, and one that works out a share with `allocate`,
where two divisions by a value rather than by a constant do not come back from the model
checker (§15.102) — a harness that hangs being worse than one that is not written.

What this is *not*: a proof about the table, or about the checker. It is a proof about this
Rust over the declared domain, by a different tool than the one that
proved the table — which is worth having precisely because the two are independent.

### Ruby

```ruby
CouponStep.coupon_step(subtotal, applied, kind, rate, face, dup)
```

A module named after the rule, with one module method. Every number is a plain `Integer`, and
Ruby's `Integer` is exact at any size, so the overflow the proof (E108) rules out cannot
happen quietly here either.

**The unit is not in the type.** Ruby has no zero-cost brand, so `money[円, incl_tax]` and
`mass[g]` are both `Integer`, and which is which is stated in the comment above the method
and in `rulec api`. This is the one guarantee Ruby gives up relative to the three targets
whose type systems can hold a unit — the proof still holds, but nothing will catch a caller
that swaps two same-typed arguments. Python is in the same position, with `NewType` doing the job only when
a type checker is run.

```ruby
require_relative "coupon_step"

out = CouponStep.coupon_step(10000, 0, CouponStep::CouponKind::PERCENT, 10, 0, false)
puts out.ok, out.raw          #=> true, 1000
```

An enum is a module of frozen constants named after the aliases in upper case
(`CouponKind::PERCENT`), and each one *is* the source name as a string — which is also what
the wire format carries, so nothing has to be converted in either direction. `CouponKind::ALL`
is the list, and the entry guard uses it.

With one output the method returns that value; with two or more it returns a `Struct` named
`Output` whose members are the outputs. `Struct` rather than `Data` so that the module runs
unchanged on every 3.x as well as 4.x.

Errors are `RuleInputError < ArgumentError` for a contract violation by the caller and
`RuleContradictionError < RuntimeError` for the runtime guard below — the same split as
Python's two exception classes.

It needs no gem: the module itself requires nothing, and the runner requires only `json` and
`date`, both standard library.

**A signature ships with it.** `sig/<rule>.rbs` sits beside the module, where `steep` looks
by default, and it is checked against the module itself — a method the signature forgot fails
as loudly as a wrong type. It refuses a caller that passes a string where a number is
declared, that gets the argument count wrong, or that passes a value which is not one of an
enum's: an enum is typed as the union of its own values (`"率引き" | "額引き" | "送料無料"`),
which is stronger than the plain Ruby, where every member is a `String` until the entry guard
fires.

It still does not refuse grams where yen were meant. RBS has no newtype either, and
`type yen = Integer` is *the same type* as `type gram = Integer` — measured with `steep`, not
assumed (§15.23). Units stay a matter of the declaration and the comment.

### PHP

```php
CouponStep\coupon_step($subtotal, $applied, $kind, $rate, $face, $dup);
```

One file, in a namespace named after the rule, with free functions in it. `declare(strict_types=1)`
is on, and every parameter has a declared type — `int`, `string`, `bool`, or the enum itself —
so a float where an integer was asked for is a `TypeError` at the door rather than a value
that quietly rounds.

**Every division is `intdiv`.** PHP's `/` returns a *float* as soon as the division is not
exact, and a float is exact only to 2^53 where the overflow proof (E108) is about int64. That
would lose the low digits of a yen amount silently, at the one place the proof cannot see, so
the operator never appears: `intdiv` truncates toward zero, which is what the generator's
own `//` means wherever it writes one.

**The unit is not in the type**, as in Ruby. `money[円, incl_tax]` and `mass[g]` are both
`int`, and which is which is stated in the doc comment above the function and in `rulec api`.
What PHP *can* hold is the kind, which is why the entry guard here asks about the range alone.

```php
require_once __DIR__ . '/coupon_step.php';

$out = CouponStep\coupon_step(10000, 0, CouponStep\CouponKind::PERCENT, 10, 0, false);
echo $out->ok, ' ', $out->raw;      // true 1000
```

An enum is a native backed enum whose cases are the aliases in upper case
(`CouponKind::PERCENT`) and whose backing value *is* the source name — which is also what the
wire carries, so `CouponKind::from($s)` is the whole conversion and nothing keeps a second
table.

With one output the function returns that value; with two or more it returns an `Output`, a
final class of promoted readonly properties. The traced twin returns `[$value, $trace]`, which
the caller destructures.

Errors are `RuleInputError extends \InvalidArgumentException` for a contract violation by the
caller and `RuleContradictionError extends \RuntimeException` for the runtime guard below —
the same split as Python's two exception classes.

It needs no composer and no autoloader: one `require_once` is enough, `ext/json` has been
compiled into every build since 8.0, and the file uses nothing newer than 8.1. The floor is
**8.2**, because 8.1 is where security support ended.

**One thing PHP does that the other ten do not**: an integer that overflows becomes a float
rather than wrapping. E108 proves no intermediate leaves int64, so a rule that passed `check`
cannot reach it — but it is worth knowing which side of the proof the language sits on.

### Go

```go
func CouponStep(in Input) (Output, error)
```

Inputs are bundled into an `Input` struct rather than a parameter list, so that a row of
same-typed integers cannot be passed in the wrong order. Fields are the aliases in
PascalCase. With one output the first return value is that value; with two or more it is an
`Output` struct.

```go
out, err := couponstep.CouponStep(couponstep.Input{
    Subtotal: 10000,
    Applied:  0,
    Kind:     couponstep.CouponKindPercent,
    Rate:     10,
    Face:     0,
    Dup:      false,
})
if err != nil {
    return err
}
fmt.Println(out.Ok, out.Raw)
```

A Go enum member is the type name followed by the alias
(`couponstep.CouponKindPercent`). Each enum also gets `Valid()`, `String()` (which returns
the Japanese name) and `ParseXxx(string)`.

Go returns an `error` where Python raises. A contract violation and a contradiction in the
rule both arrive as an `error`; the message says which.

### Swift

```swift
func couponStep(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: Bool) throws -> Output
```

Every number is an `Int64`, which is the type the overflow proof (E108) is stated in — `Int`
is the platform's word and only happens to be 64 bits everywhere Swift runs today. A unit is
a struct with one stored property over it — `public struct YenInclTax { public var value:
Int64 }` — which Swift lays out as the integer itself, so the compiler refuses a
tax-exclusive amount where a tax-inclusive one was meant at no run-time cost, exactly as
Rust's newtype does. Construct one with `YenInclTax(10000)` and read it back with `.value`.

```swift
let out = try couponStep(
    subtotal: YenInclTax(10000),
    applied: YenInclTax(0),
    kind: .percent,
    rate: Rate(10),
    face: YenInclTax(0),
    dup: false
)
print(out.ok, out.raw.value)   // true 1000
```

Identifiers are in Swift's own spelling: the function, the parameters and the enum members
are the aliases in lowerCamelCase (`shipping_fee` becomes `shippingFee`), and an alias that
lands on one of the language's keywords is written in backticks. `rulec api` states the
spelling it used, so nothing has to be guessed. Types keep their PascalCase.

An enum is a `String`-backed Swift enum whose raw value *is* the source name — which is also
what the wire format carries — so `.rawValue` and `init?(rawValue:)` are the whole conversion
in both directions and no parser is generated. It is `CaseIterable`, so `.allCases` is the
list. **There is no entry guard on an enum input**, for the reason given under Rust.

With one output the function returns that value; with two or more it returns a struct named
`Output`. Both it and the brands are `Hashable` and `Sendable`, and `Output` declares a public
memberwise initializer, since the one Swift writes for a public struct is internal and a
caller in another module could not reach it.

Errors are thrown rather than returned: `RuleError.input` is a contract violation by the
caller and `RuleError.contradiction` is the runtime guard described below — the same split as
Python's two exception classes. `RuleError` is `CustomStringConvertible`, so printing one
gives the message.

It compiles with `swiftc` alone — `swiftc coupon_step.swift coupon_step_runner.swift -o
coupon_step` builds the rule and its runner together, with no `Package.swift` and nothing to
fetch. The runner carries `@main` rather than being called `main.swift`, because top-level
code is only allowed in a file of that name and the rule has to be able to sit beside it.

### Java

```java
CouponStep.couponStep(subtotal, applied, kind, rate, face, dup);
```

One public class named after the rule, with everything nested inside it: the enums, the
errors, `Fired`, `Output` and the static methods. Nested, not top-level, because two rules
generated into the same directory would otherwise each want to be `RuleInputError.java`.

**`long` is the int64 the proof is about.** No widening, no `BigInteger`, no check at the
door: what E108 proves about every intermediate is exactly what the machine word holds.
Overflow wraps rather than trapping, as it does in Go and Rust.

**The unit is not in the type.** Java has no zero-cost wrapper, so `money[円, incl_tax]` and
`mass[g]` are both `long` and the javadoc says which is which. The kind *is* declared, so the
entry guard asks about the range alone.

```java
var out = CouponStep.couponStep(10000, 0, CouponStep.CouponKind.PERCENT, 10, 0, false);
System.out.println(out.ok() + " " + out.raw());   // true 1000
```

An enum is a Java enum whose constants are the aliases in upper case
(`CouponKind.PERCENT`); `value()` gives the source name the wire carries, and
`CouponKind.from(String)` reads one back, refusing an unknown value at the door.

With one output the method returns that value; with two or more it returns `Output`, a
record. The traced twin returns `Traced`, a record of `value()` and `trace()` — Java has no
tuple, and a pair with the concrete type in it reads better at the call site than a generic
one.

Errors are `RuleInputError extends IllegalArgumentException` and
`RuleContradictionError extends RuntimeException`.

**It builds with the JDK alone**: `javac --release 17 -encoding UTF-8 -d classes *.java`, then
`java -cp classes`. No Maven, no Gradle, no dependency — `java.util.List` and
`java.util.ArrayList` are the only imports, and the runner reads the wire with a JSON reader
written into it, because the JDK still has none (JEP 540 is an incubator proposed for a later
release). `--release 17` is not the newest LTS but the **floor**: what a generated artifact
has to decide is the oldest release it runs on, and `rulec test` compiles at that floor rather
than at whatever JDK is installed. Kotlin and Scala call the class as it stands.

Two things are pinned that would otherwise follow the machine: `-encoding UTF-8`, because a
JDK before 18 reads source in the platform's charset and a Japanese identifier would arrive as
mojibake; and `Locale.ROOT` on every `String.format`, because `%04d` under a locale with its
own digits would not write the bytes the other ten targets write.

### SQL

```sql
CREATE VIEW shipping_fee AS
WITH "_c0" AS (SELECT "_id", "dest", CAST("weight" AS BIGINT) AS "weight", … FROM "shipping_fee_input"),
…
SELECT "_id", "dest", "weight", "total", "member", … AS "fee", "base_fee_row", "payer_row" FROM "_c5" ORDER BY "_id";
```

**Two doors on one query.** `shipping_fee.sql` is **one query over a relation of inputs**: provide
`shipping_fee_input` with a column `_id` (anything that identifies the row; it comes back
unchanged) and one column per input under its alias, and out come `_id`, the inputs, the
outputs, and one column per table with the number of the row that matched (`base_fee_row`).
The header of the file lists every column with its type and what goes in it. A row of the
table is a `WHEN`; the rows that matched are columns rather than a list; the rounding is
arithmetic in the final `SELECT`. Many rows go through in one statement, which is what a
closing batch or a recalculation needs and what the per-row functions cannot do.

The values are the wire's: every number is an integer in its declared unit, a rate a count
of its steps, an enum its name as text, a boolean a boolean, and a date the number of days
since 1970-01-01 (`some_date - DATE '1970-01-01'` in PostgreSQL). The query is written for
PostgreSQL — the inputs are cast to `BIGINT` on the way in, because the product of two
`int4` columns overflows where the proof (E108) assumed int64 — and it stays inside what
SQLite runs as well, which is how `rulec test` holds it to the reference evaluator with
nothing but `python3`: the runner beside it loads the vectors into an in-memory SQLite and
prints the same records the other runners print. `min` and `max` are `LEAST` and `GREATEST`,
which the runner registers for SQLite.

**The dialect is part of the claim.** Postgres and SQLite agree on the one thing the rounding
leans on — `/` between integers truncates toward zero — which is why proving on the second
says something about the first. A warehouse does not necessarily agree. BigQuery's `/` always
returns `FLOAT64` (its integer division is `DIV`), and Snowflake's returns a scaled `NUMBER`
rather than truncating, so a grid like `(ABS("_raw_fee") / 20 + 1) * 20` stops being the
rounding it was written as, silently, in exactly the place the proof exists to watch.
ClickHouse is the measured case: `/` is `Float64` there (`intDiv` is the integer one), and
this file runs on it **without an error** — 26 of the rules in `tests/corpus/` were tried, 21
answered correctly, and 5 returned fractions where the rule declares an integer amount
(`0.00055` for `0`, `3809` for `3800`, `14.5` for `14`). The ones that pass do so because
their products happen to be even, which is exactly how this class of mistake stays hidden.
So if the rule has to run in a warehouse, port it deliberately and hold the port to the rule
with `rulec verify` against the real engine ([backends.md](backends.md)) — nothing else will
catch it.

A query cannot stop, so what the other languages raise, this one returns as a column. The
entry guard is `_input_error`: NULL for a row inside the declared domain, and otherwise the
same sentence the others raise — a missing input, a value outside its range, a name that is
not a member of the enum, a number that is not an integer (18.3 in a column of steps is
refused, not truncated to 18). Where two rows of a `unique` table could not be proved
exclusive (W114), a `_contradiction` column names them when both match. The runner stops on
either, as the other languages raise.

**The other door — `shipping_fee_function.sql`.** The same query, asked for one case at a time:

```sql
CREATE FUNCTION "shipping_fee"("dest" text, "weight" bigint, "total" bigint, "member" text)
RETURNS TABLE ("fee" bigint, "base_fee_row" int, "payer_row" int)
LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE
```

The arguments are the inputs in order and what comes back is the outputs followed by the row
that matched in every table; `rulec api` gives the same line under `sql.function`. The body is
the query above, **unchanged**, with one CTE in front of it binding the arguments into a
one-row `shipping_fee_input` — a `WITH` name hides a table of the same name, so nothing below
knows which door it was entered by and the two shapes cannot drift apart.

Where the relation returns a column, the function **raises**: `RAISE EXCEPTION` with SQLSTATE
`22023` for an input outside the declaration and `P0001` for a contradiction, carrying the same
sentence the other eleven raise. The difference is the reason both exist. A relation is read by
something that has the whole row in front of it; a function called over HTTP is read by a client
that may look at one field, and handing that client a number that looks like an answer beside a
column it ignored is the worse of the two failures.

PostgreSQL only — SQLite has no `CREATE FUNCTION`, so this is the one file the query's own
runner cannot stand in for. `rulec test` runs it on a real PostgreSQL through `psql`, taking the
connection from libpq's own environment (`PGHOST`, `PGDATABASE`), and skips that pass with a
note when there is no server to reach. The runner creates the function, calls it once per vector
**by argument name**, holds the answers to the reference evaluator byte for byte like every other
runner, and drops it again, so the run leaves nothing behind in the database.

Call it by argument name yourself: `SELECT * FROM "shipping_fee"("dest" => '北海道', "weight" =>
1200, "total" => 0, "member" => '一般');`. Positionally it is ambiguous whenever the rule's alias
is also the name of a built-in (`rank`), and a named argument is the one form a variadic built-in
cannot answer to. In PostgREST — which is what Supabase runs — a function in an exposed
schema is an endpoint with no server code of its own. Installing this file and nothing else
answers:

```
$ curl -X POST localhost:3000/rpc/shipping_fee -H 'Content-Type: application/json' \
       -d '{"dest":"北海道","weight":1200,"total":0,"member":"一般"}'
[{"fee":1200,"base_fee_row":1,"payer_row":3}]                                  200

$ curl -X POST localhost:3000/rpc/shipping_fee -H 'Content-Type: application/json' \
       -d '{"dest":"北海道","weight":0,"total":0,"member":"一般"}'
{"code":"22023","details":null,"hint":null,"message":"重量 is out of range"}    400
```

The raising is what makes the second one a 400 rather than a 200 carrying a number no proof
covers, and `GET /rpc/shipping_fee?dest=…` answers too, since the function is `IMMUTABLE`. Ask
for one object instead of an array with `Accept: application/vnd.pgrst.object+json`. Hasura
tracks a function only when it returns `SETOF` a table it already tracks, so there a table or
view of that shape has to be tracked first.

### Wasm

```
wasm/
├── shipping_fee.rs           the same module rust/ gets
├── shipping_fee_wasm.rs      the crate root: that module behind the canonical ABI
├── shipping_fee.wit          the world, for the component model
├── shipping_fee_runner.mjs   the Node runner `rulec test` drives
└── _round.rs, _round_test.mjs
```

One file to build, with `rustc` alone — `rulec api` prints this line under `wasm.build`:

```console
$ rustc --edition 2021 -C opt-level=s -C lto -C panic=abort -C strip=symbols \
    --target wasm32-unknown-unknown --crate-type cdylib shipping_fee_wasm.rs -o shipping_fee.wasm
```

The module exports the canonical ABI of one function, `call: func(input: string) -> string`:
`cabi_realloc` to place the input in the module's memory, `call(ptr, len)`, which returns a
pointer to a (pointer, length) pair holding the answer, and `cabi_post_call(ret)` to free it.
The input is a JSON object with the inputs by name, in the wire form of
[formats.md](formats.md) (a record with them under `"in"` is read the same way); the answer
is the record line the other languages' `_record` writes, or `{"error":"…"}` for an input
outside the contract — an unknown enum value included, so a host is answered rather than
trapped. The module imports nothing, so it instantiates with an empty import object anywhere
WebAssembly runs; the shipping rule is forty kilobytes.

```js
const { instance } = await WebAssembly.instantiate(bytes, {});
const ex = instance.exports;
function call(text) {
  const b = new TextEncoder().encode(text);
  const ptr = ex.cabi_realloc(0, 0, 1, b.length);
  new Uint8Array(ex.memory.buffer, ptr, b.length).set(b);
  const ret = ex.call(ptr, b.length);
  const [p, n] = new Uint32Array(ex.memory.buffer, ret, 2); // views after the call: the memory may have grown
  const out = new TextDecoder().decode(new Uint8Array(ex.memory.buffer, p, n));
  ex.cabi_post_call(ret);
  return out;
}
call('{"届け先":"北海道","重量":2500,"注文金額":12000,"会員":"ゴールド"}');
// {"in":{…},"observed":{"送料":1800},"trace":[{"table":"基本送料","row":2},{"table":"負担判定","row":3}]}
```

The `.wit` names the same function as the export of a world, so the module becomes a
component with no change to it, and a component host calls it like any other:

```console
$ wasm-tools component embed shipping_fee.wit shipping_fee.wasm -o shipping_fee.embedded.wasm
$ wasm-tools component new shipping_fee.embedded.wasm -o shipping_fee.component.wasm
$ wasmtime run --invoke 'call("{\"届け先\":\"北海道\",\"重量\":1,\"注文金額\":0,\"会員\":\"一般\"}")' shipping_fee.component.wasm
```

`rulec test` builds the module and holds it to the vectors through the runner
(`ok shipping_fee (Wasm) 70 vectors`), and skips the language with a note when `node` or the
`wasm32-unknown-unknown` standard library (`rustup target add wasm32-unknown-unknown`) is
missing.

---

## A rule that walks a sequence

A rule with `elements` and `fold` ([reference §6.2](reference.md#62-elements-and-fold)) takes
one more argument, last: the sequence, as a list of `Element`. `Element` is a record of the
element's own fields, generated beside `Output`, and each field carries the same entry guard
an input of that type carries.

```python
class Element(NamedTuple):
    row_zone: Zone
    threshold: YenInclTax
    row_fee: YenInclTax


def freight(dest: Zone, total: YenInclTax, freight_rows: list[Element]) -> YenInclTax: ...
```

The body is a loop over that list, with the rule's own tables inside it and one branch per
verdict. What comes out of the loop goes through the same rounding and the same return the
rule would have had without it. `rulec api` lists the sequence as the last parameter, with the
element's fields under `elements`, and the record written for one call carries the sequence as
an array of objects. Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java and Wasm are
generated; SQL and NumPy are refused by name, because one query has nowhere to carry a value from
row to row, and a walk is not a column operation.

## A rule whose inputs are projected from the caller's object

A rule whose inputs say `from`
([reference §3.3](reference.md#33-shape-and-from--where-the-callers-object-holds-an-input))
gets one more function beside the others: the same call, taking the caller's object instead of
the scalars.

```python
def order_shipping_from(order: dict[str, Any]) -> Yen:
    """Reads the inputs out of the caller's object and calls this rule. …"""
    return order_shipping(
        Zone(order["shipping"]["zone"]),
        any(_e["chilled"] for _e in order["lines"]),
        len(order["lines"]),
    )
```

```typescript
export function order_shipping_from(order: _Obj): Yen {
  return order_shipping(
    parseZone(String(order["shipping"]["zone"])),
    (order["lines"] as _Row[]).some((_e) => _e["chilled"] === true),
    BigInt((order["lines"] as _Row[]).length),
  );
}
```

It is the glue an application writes by hand otherwise, once per input per language, with
nothing checking it. Here the paths were held to the contract by `rulec check` before the
function was written, so a field the contract renamed is E121 at build time rather than a
`KeyError` in production. The object is **read and never named**: the parameter is a plain map,
because a type for it would be a domain object model and this tool makes none.

A rule may project some inputs and pass the rest: the ones with no `from` stay parameters of
the projection function, after the objects, and a sequence stays last.

From a JSON Schema, an optional input (`T?`) reads its field so that a missing one — or a
missing object on the way to it — is none: `_dig(order, "coupon", "kind")` in the Python module,
and the same in each of the other four. Every other input reads straight into the object, which
is why `rulec check` requires a field a required input reads to be `required` in the contract
(E122).

From a `.proto`, the object is taken in the JSON form protojson gives it, and every read goes
through one helper, `_proto`, given each step's JSON name and `.proto` name and what to read
when protojson left the field out:

```python
Zone(_proto(order, (("shipping", "shipping"), ("zoneCode", "zone_code")), "", "")),
any(_e.get("chilled", False) for _e in _proto(order, (("lines", "lines"),), [], [])),
len(_proto(order, (("lines", "lines"),), [], [])),
```

A step is looked up under its JSON name first — `json_name`, or the lowerCamelCase of the name —
and then under the name the `.proto` writes. A field that is not there reads as its default — 0, `""`, false, no
elements, an enum's value numbered 0 — and an optional input reads an unset message or
`optional` field as none. A 64-bit integer, which protojson writes as a string, is made the
number it is before a `where` compares it (a BigInt in TypeScript and JavaScript), and an enum
is compared by its value's name.

**Python, TypeScript, JavaScript, Ruby and PHP are generated.** Go, Swift, Java and Rust hold
the caller's object as a type, and naming that type would mean generating it or following the
caller's own; SQL takes a relation of flat columns, NumPy takes columns, and the Wasm ABI takes
one JSON object of the rule's own inputs. The **path check applies to every target equally** —
it happens in `check`. `rulec api` lists it all under `projection`: the contracts, the path of
every projected input, and the function name and signature in each of the five.

## The digest in the header

Every generated file names its source in its header — `rule 送料 v4, sha256:d98b4f699db8` — and
`rulec api` gives the whole digest as `source_sha256`. It is the SHA-256 of the rule file's
bytes, computed by rulec itself, so `gen --check` and a reader of the header agree on what was
generated from what. A rule transcribed from a document names it too — `Cites: 措置法 = law
332AC0000000026 asof 2026-04-01 (第91条 sha256:85faf53f6f6e8196)` — one line per `source`, and
`rulec api` lists the same under `sources`, so the file says which text of the law it was made
from. A file source carries its `url` there too when it has one, and the tables taken out of it
(`Cites: 規約 = file tariff.md url https://raw.githubusercontent.com/o/r/a1b2c3d/docs/tariff.md
sha256:… (表1 sha256:…)`), which is what lets a reader of the generated code go and look at the
document, and at the table, the rows were transcribed from. A law from a database other than
e-Gov names it in both places — `Cites: osha = law ecfr 29 CFR 1910 asof 2026-01-01
("§1910.157" sha256:…)`, and `"db": "ecfr"` beside the id under `sources`.

## The rows that matched

Beside every function there is a twin with `_traced` on its name, spelled `Traced` wherever the
language's own convention capitalises it (the table below says which).
It takes the same inputs and returns, beside the outputs, the rows that matched: one per
table, in order, each as the table's name and its 1-based row number. The plain function
calls it and drops the trace, so the branches exist once, in the traced one.

| | the twin | the row |
|---|---|---|
| Python | `def coupon_step_traced(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> tuple[Output, list[Fired]]:` | `Fired`, a `NamedTuple` of `table`, `row` and `label` (`""` when the row has none) |
| TypeScript | `coupon_step_traced(…): [Output, Fired[]]` | `{ table: string; row: number; label?: string }` |
| JavaScript | `coupon_step_traced(…)`, returning `[out, trace]` | `{ table, row, label? }` |
| Rust | `coupon_step_traced(…) -> Result<(Output, Vec<Fired>), RuleError>` | `Fired { table: &'static str, row: u32, label: &'static str }` |
| Ruby | `CouponStep.coupon_step_traced(…)`, returning `[output, trace]` | `Fired`, a `Struct` of `table`, `row` and `label` |
| PHP | `function coupon_step_traced(int $subtotal, int $applied, CouponKind $kind, int $rate, int $face, bool $dup): array`, returning `[$out, $trace]` | `Fired`, a final class of readonly `table`, `row` and `label` |
| Go | `func CouponStepTraced(in Input) (Output, []Fired, error)` | `Fired{Table, Row, Label}` |
| Swift | `couponStepTraced(…) throws -> (Output, [Fired])` | `Fired(table:row:label:)`, `label` defaulting to `""` |
| Java | `public static Traced couponStepTraced(long subtotal, long applied, CouponKind kind, long rate, long face, boolean dup)` | `Fired`, a record of `table`, `row` and `label`; `Traced` is the pair of `value()` and `trace()` |
| SQL | none: the answer is the row | one column per table, `decide_row`, holding the row number; NULL for a table that another table of the same output beat |
| Wasm | none: the answer of `call` is the record line, `trace` beside `observed` | `{"table":…,"row":…}` objects in that line, with `"label"` when the row has one |
| NumPy | `rule.traced(**{column: sequence})`, returning `(outputs, fired)` | one `(picked, rows)` pair per definition set: `picked[i]` indexes `rows`, and each entry is the `{"table":…,"row":…,"label"?:…}` the row was written in |

The row numbers are the ones `rulec doc` prints in its `#` column and the ones a `verify` or
`replay` report clusters by, so a trace taken from a log reads against the approved document
directly. A row is numbered as it is written, whatever order the branches are tried in: when
several tables define one output, the branches of the table that takes precedence come first
and the row still reports the table it was written in and its position there. A `clause` is
one branch and fires as row 1 of a table named after it. `rulec test` compares these rows as well as the values: a generated function that
produced the right amount from the wrong row fails there. `rulec api` names the twin under
`traced` and gives its signature under `traced_signature`.

## A record of one call

Every module also has a function with `_record` on its name, `Record` where the language
capitalises. It
takes the inputs, what the rule returned, the rows that matched and a tag, and gives back
one line in the fixtures format of [formats.md](formats.md#fixtures-rulec-fixtures-lint-replay-diff):

```json
{"tag":"order:1234567","in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},"observed":{"送料":800},"trace":[{"table":"基本送料","row":3},{"table":"負担判定","row":3}]}
```

Write that line to a log and the records `replay` and `diff` need come out of the generated
code itself, in the wire form of §10.2 — an enum as its name, a date as `YYYY-MM-DD`, a
number as an integer in its declared unit — with nothing to extract or convert afterwards.
An empty tag is left out. The function builds the line itself, so no language gains an import
for it.

| | the function |
|---|---|
| Python | `def coupon_step_record(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool, out: Output, trace: _Trace, tag: str = "") -> str:` |
| TypeScript | `coupon_step_record(…, out: Output, trace: Fired[], tag = ""): string` |
| JavaScript | `coupon_step_record(…, out, trace, tag = "")` |
| Rust | `coupon_step_record(…, out: Output, trace: &[Fired], tag: &str) -> String` |
| Ruby | `CouponStep.coupon_step_record(…, out, trace, tag = "")` |
| PHP | `function coupon_step_record(…, Output $out, array $trace, string $tag = ''): string` |
| Go | `func CouponStepRecord(in Input, out Output, trace []Fired, tag string) string` |
| Swift | `couponStepRecord(…, out: Output, trace: [Fired], tag: String = "") -> String` |
| Java | `public static String couponStepRecord(…, Output out, List<Fired> trace, String tag)` |
| SQL | none: the answer is the row, and the runner writes the record from it |
| Wasm | none: the record line is what `call` returns |
| NumPy | none: the plan names no function, and the runner writes the record from the columns |

The generated runner prints exactly this line for every vector, and the expected file `gen`
writes beside the vectors is in the same format, so `rulec test` holds the record function
— the wire form of every input, dates included — to the reference evaluator in every language.
`rulec api` names the function under `record` and gives its signature under
`record_signature`.

## The two guards

**The entry guard** enforces at run time what the proof assumed. Every numeric input is
checked against its declared `range`, and every enum input against its set of values. If the
check were absent, a caller outside the declared domain would get a silently wrong number
instead of an error — and the completeness proof says nothing about inputs the rule never
declared.

```python
if not 0 <= subtotal <= 1000000:
    raise RuleInputError("商品合計 is out of range", subtotal)
```

The sentence and the value travel apart. The error carries both — `what` is the sentence,
`value` the number that was refused — and puts them together only when it is printed, so
the message is what it always was and a caller can react to the value without parsing it
back out of a string. The Rust error is a pair of struct variants for the same reason, and
nothing on the refusing path formats anything, which is what lets a model checker walk it
([the proofs](#the-proofs)).

A number that is not an integer is refused before the range is looked at, in every language
where a caller can pass one. A float sits inside any range: 18.3 for a rate declared in steps
of 0.1% would pass the check above, be taken as 1.83%, and be answered without a word. The
three statically typed targets need no such line; their types are the line. In SQL the guard
is a column, `_input_error`, because a query cannot stop: NULL inside the domain, the
sentence outside it ([below](#sql)).

```python
if not _isinstance(rate, int) or _isinstance(rate, bool):
    raise RuleInputError("料率 is not an integer", rate)
```

`rulec api` states the same bounds, taken from the same place, so an integration built from
the inventory cannot send values the guard rejects.

**The contradiction guard** is the other half of W114. When two rows of a `policy unique`
table might overlap and the checker could neither construct an input that proves it nor prove
that none exists, it does not pretend either way: it warns, and the generated code carries a
guard that stops rather than silently picking the earlier row.

```python
# guard: W114 (table 判定, row 1 × row 2): a pair of rows whose exclusivity could not be proven statically
if up and dn:
    raise RuleContradictionError("table 判定: row 1 and row 2 matched at the same time")
```

`up` and `dn` compare a doubled amount with an odd boundary, so no whole value reaches it —
but the elimination that decides these pairs works over the rationals and stops half way.
Derived values that share an input, and the thresholds inside a boolean definition, get no
guard any more: they are proved apart.

If this ever fires in production, it is evidence — the overlap the checker could not decide is
real, and the rule needs fixing.

---

## The rule as an MCP tool

Beside the module, `gen` writes the rule as one MCP server: `<alias>_mcp.py` next to the
Python module, `<alias>_mcp.mjs` next to the JavaScript one, and the same server with its
types on in the `typescript` directory. It is for the agent that **calls** the rule, where
`rulec mcp` is for the agent that writes one. Registered as a stdio server, the rule is one
tool named after its alias:

```console
$ claude mcp add shipping_fee -- python3 generated/python/shipping_fee_mcp.py
```

**The same server speaks MCP's Streamable HTTP**, which is what the places that only accept a
URL need — a chat client's custom connectors, an agent builder, a workflow product's MCP
node:

```console
$ python3 generated/python/shipping_fee_mcp.py --http 8000
http://127.0.0.1:8000/mcp
```

One `POST` carries one message and the answer comes back as `application/json`; a message
with no id is answered with `202` and nothing else; `GET` is `405`, because the server never
sends anything unasked; `DELETE` ends the session it was given. The session id handed out at
`initialize` comes back on every later message.

Two things it does **not** carry, and they are the caller's to put in front of it: **TLS and
authentication**. It listens on `127.0.0.1` alone unless a host is given (`--http 0.0.0.0:8000`),
and it refuses a request whose `Origin` is not local unless that origin is named with
`--origin https://example.com` — the check that keeps a page open in somebody's browser from
reaching a server running on their machine.

The tool speaks the wire. Its `inputSchema` is the `in` object `rulec schema` prints — every
input, required, nothing extra; an integer in the declared unit, with the unit and, for a
rate, the step in its description; an enum as its listed names; a date as `YYYY-MM-DD` — and
its result is the line the record function writes, as text and as `structuredContent`:

```json
{"in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},"observed":{"送料":800},"trace":[{"table":"基本送料","row":3},{"table":"負担判定","row":3}]}
```

So an answer carries the rows that decided it, and one call is one fixtures record. Started
with `--record calls.jsonl`, the server appends every answered call to that file, which
`rulec fixtures lint`, `replay` and `diff` read as it stands: what the agent asked becomes
the record the next revision is measured against.

A call the rule cannot take is refused, not answered: `isError` is set and the text names the
argument — one missing, one extra, a value outside its range, a name that is not a member of
the enum, a number that is not an integer (18.3 for a rate in steps of 0.1% is refused, not
read as 1.83%). The module's two error classes are what reach the caller, under their names.

### The answer, with the table beside it

A host that renders **MCP Apps** ([SEP-1865](https://blog.modelcontextprotocol.io/posts/2025-11-21-mcp-apps/))
gets more than the record. Beside the server, `gen` writes `<alias>_page.html` — the page
`rulec doc --format html` renders, byte for byte — and the server offers it as the tool's
view:

```json
{"uri":"ui://shipping_fee/table","name":"shipping_fee","mimeType":"text/html;profile=mcp-app"}
```

The tool carries `_meta.ui.resourceUri` pointing at it, the host reads it with
`resources/read`, and renders it in a sandboxed frame. The page then **opens on the case the
tool was just called with**: the fields filled in, the answer shown, and the rows that
decided it lit up — the same page a person opens from a file, running the same generated
JavaScript. So the reader of a chat sees what the agent asked, what came back, and *which
rows of which table* said so.

The page is drawn in the host's theme. When the host's answer to `ui/initialize`, or a later
`ui/notifications/host-context-changed`, carries `theme` as `light` or `dark`, the page takes
it; opened as a file, it follows the reader's own light or dark setting.

It is a view and not a client: it needs no network and declares no external origin, so the
restrictive default CSP a host applies is enough for it.

Two things follow the specification rather than taste. The view is offered **only to a host
that said it can render one** (the `io.modelcontextprotocol/ui` extension in the client's
capabilities); to anything else the tool is what it was, and the record is the whole answer.
And the page is a file: delete it and the server keeps serving the tool, without a view.

`rulec test` drives the server as a client would — `initialize`, `tools/list`, then one
`tools/call` per vector — and holds what comes back to the same expected records the runner
is held to, so the server sits inside the same claim as the module. **It does that twice, once
over each transport** (`via` is `mcp` and `mcp-http`): the conversation is the same, so an
answer that changed with the carrying would be a disagreement. Nothing beyond `python3` or
`node` is needed to run it.

---

## The rule as a Connect service

The MCP tool above is for an agent. This is the other caller — a service that another team's
code calls over a wire it already speaks — and `gen` writes it as
[Connect](https://connectrpc.com/):

```
generated/proto/rulec/shipping_fee/v4/shipping_fee.proto   the whole of the contract
generated/proto/buf.yaml                                   the tree is a buf module
generated/proto/buf.gen.yaml                               how the stubs are generated
generated/python/shipping_fee_service.py                   what stands behind it
generated/python/shipping_fee_connect_runner.py            the same vectors, over the wire
```

The `.proto` is one file for every language, so it sits beside the language directories
rather than inside one, and **its path spells its package** — what buf's
`PACKAGE_DIRECTORY_MATCH` asks for — so it can be dropped into a buf module as it stands.
`buf lint` finds nothing in it.

```proto
package rulec.shipping_fee.v4;

message DecideRequest {
  // 届け先
  Prefecture dest = 1;
  // 重量: an integer, in g. 1 to 40000
  int64 weight = 2;
  // 注文金額: an integer, in 円 (tax included). 0 to 10000000
  int64 total = 3;
  // 会員
  MemberKind member = 4;
}

message DecideResponse {
  // 送料: an integer, in 円 (tax included)
  int64 fee = 1;

  // The rows that matched, one per table, in order.
  repeated Fired trace = 100;
}

service ShippingFeeService {
  // Decides 送料 from 届け先, 重量, 注文金額, 会員.
  // The rule is a pure function, so this method has no side effects and can be
  // called with GET.
  rpc Decide(DecideRequest) returns (DecideResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}
```

Four things there are decisions rather than transcription.

**The package carries the rule's major version.** The package names the *wire*, and what a
change to the wire does is exactly what `buf breaking` is there to say.

**The method declares that it has no side effects**, which is not a hint but something
already proved: the same inputs give the same answer, forever, for one version of the table.
Connect lets such a method be called with `GET`, which is what makes an answer cacheable.

**The answer carries the rows that decided it.** `trace` is the same list the record function
writes, so one call is one fixtures record. Its field number is far from the outputs so that
an output added to a table later takes the next small number and leaves the trace where it
was.

**Only the enums that cross the wire are declared**, and one whose values belong to a contract
outside the rule (`import proto`, [reference.md](reference.md#an-enum-a-proto-owns)) is
imported rather than copied — the rule cites that file and `rulec check` holds the two
together, so the service speaks the contract's own type instead of a second one that means
the same thing.

The stubs are generated the way [connect-py](https://github.com/connectrpc/connect-py)'s own
documentation generates them — with [buf](https://buf.build/), configured by the two files
beside the `.proto`:

```console
$ uv add connectrpc
$ cd generated/proto && buf generate     # the messages, the client and the server base, into ../python/stubs
```

They land in a `stubs/` package of their own, because the plugin writes an `__init__.py` at
the root of wherever it generates and the directory beside it is not a package.
[`connectrpc`](https://pypi.org/project/connectrpc/) is the one dependency anything `gen`
writes has, and it is confined to the service file: the module it calls imports nothing, and
deleting the service leaves the rule where it was.

**The service is written as both applications**, because the rule is a pure function with
nothing to await and the two are two doors on one body:

```console
$ uvicorn shipping_fee_service:app --port 8080         # ASGI: uvicorn, hypercorn, daphne
$ gunicorn 'shipping_fee_service:wsgi_app'             # WSGI: gunicorn, uWSGI
$ python3 generated/python/shipping_fee_service.py --http 127.0.0.1:8080   # the standard library alone
http://127.0.0.1:8080
$ curl -sS -X POST -H 'Content-Type: application/json' \
    -d '{"dest":"PREFECTURE_KAGOSHIMA","weight":800,"total":4200,"member":"MEMBER_KIND_BASIC"}' \
    http://127.0.0.1:8080/rulec.shipping_fee.v4.ShippingFeeService/Decide
{"fee":"800","trace":[{"table":"\u57fa\u672c\u9001\u6599","row":3},{"table":"\u8ca0\u62c5\u5224\u5b9a","row":3}]}
```

Three things about that answer are protobuf's JSON mapping rather than rulec's: the field
names are lowerCamelCase, an `int64` is a **string** because a JSON number cannot hold one,
and non-ASCII is escaped. A generated client hands you a Python `int` and the table's real
name either way; it is only the bytes on the wire that look like that.

The third line is the standard library's own server, there so that the service can be tried
with nothing installed beyond `connectrpc`; it serves the WSGI side. As with the MCP server,
**TLS and authentication go in front**: none of these carries either.

An input the rule cannot take is refused rather than answered, with the code that says whose
mistake it was:

| what happened | code | HTTP |
|---|---|---|
| outside the declared domain — out of range, not an integer, not a member of the enum | `invalid_argument` | 400 |
| the runtime guard of a W114 pair fired (§8.1) | `internal` | 500 |

```console
$ curl … -d '{"dest":"PREFECTURE_KAGOSHIMA","weight":0,…}'
{"code": "invalid_argument", "message": "\u91cd\u91cf is out of range: 0"}
```

The message names the argument by the rule's own name for it (`重量`), as the module's own
error does. The second row is the one place with no static proof, and it answers 500 on
purpose: which of two rows wins is the table's to decide and no caller can fix it, so it
belongs where a service's own failures are counted.

Every answer carries `rulec-source-sha256`, the digest of the rule the service was generated
from — which version of the table answered, for a caller that keeps the answer. And
`--record calls.jsonl` appends one fixtures record per call, so a running service becomes the
file `rulec replay` and `rulec diff` read when the table is revised.

`rulec test` puts every vector through the service and holds what comes back to the same
expected records as the runner — **four times: each application, asked by POST and by GET**
(`via` is `connect-asgi`, `connect-asgi-get`, `connect-wsgi` and `connect-wsgi-get`). Both
applications are generated, and a door nobody drove would be a claim nobody checked; both
methods, for the same reason both MCP transports are driven — a method that declares itself
free of side effects may be called either way, and a carrying that changed an answer is the
thing worth catching. Without buf, the two plugins or the runtime the whole pass is skipped with a note saying
which is missing, and without `uvicorn` the ASGI half alone is.

There is a door in the other direction too. When the implementation that runs today **is** a
Connect service, `rulec adapter --template connect-python` prints the twenty lines that put
its answers in front of `rulec verify` ([formats.md](formats.md#the-adapter-protocol-rulec-verify)).

---

## The Rust runner as a WASI module

Apart from the `wasm/` target above, the Rust runner itself compiles unchanged for
`wasm32-wasip1`: it reads stdin and writes stdout through the standard library, which is what
a WASI command does — the shape a host that speaks through stdio gives a rule, such as Fastly
Compute, Spin, or an ordinary batch step in a sandbox. When `wasmtime` is on the PATH and that
target's standard library is installed (`rustup target add wasm32-wasip1`), `rulec test` runs
the runner that way too and holds its answers to the same expected records (`via` is `wasi`,
the line reads `(Rust, WASI)`). Without either, that pass is skipped with a note; it is not
counted as a missing language, but `--require-all` still fails on it, because a skipped pass
is a narrower claim either way.

A Shopify Function is **not** this shape: it exports a named function and reads its input
through the platform's own host calls ([backends.md](backends.md#a-wasm-host-shopify-functions)).

```console
$ rustc --edition 2021 -O --target wasm32-wasip1 shipping_fee_runner.rs -o shipping_fee_runner.wasm
$ wasmtime shipping_fee_runner.wasm < ../vectors/shipping_fee.jsonl
```

Those two lines are what `rulec api` carries, so nothing here has to be copied by hand. They
sit inside the Rust entry, because the shape is a property of a backend and Rust is the one
that has it:

```json
"rust": { "module": "shipping_fee.rs", "function": "shipping_fee", …,
          "wasi": {
            "source": "shipping_fee_runner.rs",
            "module": "shipping_fee_runner.wasm",
            "build":  "rustc --edition 2021 -O --target wasm32-wasip1 shipping_fee_runner.rs -o shipping_fee_runner.wasm",
            "run":    "wasmtime shipping_fee_runner.wasm",
            "wire":   "one vectors line on stdin, one fixtures record per line on stdout",
            "needs":  ["wasmtime", "rustup target add wasm32-wasip1"] } }
```

**The column is closed at one.** It is there to prove the *shape* — a rule reached as a
command that reads stdin and writes stdout — and one language proving it is the whole claim.
TinyGo, Javy or ruby.wasm alongside would add a toolchain to `rulec test` and widen nothing,
so the absence of a `wasi` entry under every other language is a decision, not a gap.

[backends.md](backends.md#a-wasm-host-shopify-functions) says which of the two shapes a
platform takes and where its input ends and the rule's begins.

## Keeping it in step with the rule

The generated files are committed to git and CI re-derives them:

```console
$ rulec gen rules/ --out generated/ --check
```

`gen --check` writes nothing and exits 1 if any file differs from a fresh generation or is
missing, so a rule edited without regenerating, or a generated file edited by hand, both stop
the build. **Never edit generated code by hand**: the header says `DO NOT EDIT`, and the next
`gen` overwrites it. Anything the generated code lacks belongs either in the `.rule` or on the
calling side.

`rulec test generated/` goes one step further and actually runs every generated language over the
generated vectors, comparing them with the reference evaluator by canonical JSON, byte for
byte. That is the only step that needs a `python3` and a `go` toolchain.

## Where the values come from

The wire representation is the same everywhere — in the vectors, in the fixtures, in the
adapter protocol, and in `rulec api`: an integer in the canonical unit for a quantity, money
or rate; the value's own name for an enum; `true`/`false` for a boolean; `YYYY-MM-DD` for a
date. So a value read out of one of those files can be passed to the generated function
unchanged, and a value the function returns can be compared against a recorded one without
conversion.
