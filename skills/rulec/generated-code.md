# The generated code

`rulec gen` writes ordinary Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm — a module in each,
a package in Go's case, and one query plus one function in SQL's. NumPy is the twelfth and the odd one out: it is
not generated code at all but the rule as data, read by one fixed evaluator (see below). There is no runtime to install and nothing to
configure: a function takes the declared inputs and returns the declared outputs, and the
query takes a relation of them. This file says what shape that code has, what it guarantees,
and how to call it.

To get the calling convention without reading the code at all, ask for it:

```console
$ rulec api rules/single_coupon.rule
```

One JSON object with the module and function names, the parameters in order with their
brands, units and ranges, the outputs with their rounding and the range written on them or given
by their tables, the enum members under the spelling each language gives them, and the errors the
code can raise. The shape is defined in [formats.md](formats.md), and a test holds every name in
it to the file the generator wrote.

---

## What it guarantees

**No dependencies.** The generated Python imports `enum` and `typing` (and a rule that reads
an `enums` file imports the module of that file, which is generated beside it,
[below](#enums-several-rules-share-their-order-and-display-names)); the generated
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
the one that does not — but for a whole square root, a search no single expression holds,
which calls `rulec_isqrt`, defined in the rule's own function file ([SQL](#sql)). **NumPy is the one exception, and a deliberate one**: the plan is
data, and the evaluator that reads it imports `numpy`. That is the target — a host that wants
whole columns decided at once already has numpy, and the dependency is its own.

**Deterministic.** The same `.rule` and the same rulec version produce the same bytes. The
formatter is built in — no `gofmt` or `black` runs afterwards, because that would make the
output depend on the version of a tool installed on the machine. `gofmt -l` being empty and
`ruff check --select E,W` being silent (line length aside) are both tested.

**Branches match the rule line for line.** Every row of every table becomes one branch, in
order, with the original cells quoted in a comment (`# row 3: tokyo | S100 | 1450JPY`) — in
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
`JPYInclTax` and `JPYExclTax` are different types, and mixing them fails to compile in Rust,
Swift, Go and TypeScript, and fails type checking in Python. Every value is an integer in its declared unit; no floating point appears
anywhere.

**Rounding is explicit and settled for negatives too.** Python's `//` rounds toward −∞ and
Go's integer division toward zero, so neither language's division is used. Each side carries
its own `_round_up` / `_round_down` / `_round_half` / `_round_bankers` (`roundUp`, … in Go),
and a unit-vector file next to the generated code checks them against the reference on every
run. The query carries no helper: each mode is arithmetic over the bound value — `CASE`,
`ABS` and `%`, which mean the same in both dialects — and the same unit vectors run over
those expressions.

**A division by a value, a remainder and a root are helpers too** (§15.213 to §15.215 of
DESIGN.md). A rule that divides by a value inside a rounding gets one helper for each mode it
rounds a quotient with — `_div_round_half(n, d)` in Python, Ruby and PHP, `divRoundHalf` in Go and
Java, `_divRoundHalf` in TypeScript, JavaScript and Swift, `quot_half` in Rust — which divides the
two integers exactly and rounds the quotient by the mode, turning both signs over first when the
divisor is below zero. It never meets a zero divisor: the check proved there is none (E132). A
rule that takes `mod` gets the remainder from zero up — `_mod`, `modFloor` in Go, `mod_floor` (over
`rem_euclid`) in Rust, `Math.floorMod` in Java — and one that takes `isqrt` the same binary search
over whole numbers in every language (`_isqrt`, `isqrtInt` in Go, `isqrtLong` in Java, `isqrt_i64`
in Rust, and so in Wasm, whose module is the Rust one). Python's `math.isqrt` is not called, so the module still imports nothing but `enum` and
`typing`. Only the helpers a rule calls are written into its module. The unit-vector file, which
every rule generated into the directory shares, carries its own copy of every one of them (Go's,
written per package, those the package has) and holds each to the reference as it does the five
modes: dividends and divisors of both signs, exact halves, quotients with nothing left over and
the ends of int64; remainders of negative numbers; squares, their neighbours and the largest root
int64 holds. The SQL query divides and takes the remainder as
arithmetic, as it rounds; the root is the one function it calls ([below](#sql)). The NumPy plan
names the same three, and `rulec_np.py` computes them over whole int64 columns.

---

## Calling it

### Python

```python
def single_coupon(subtotal: JPYInclTax, applied: JPYInclTax, kind: CouponKind, rate: Rate, face: JPYInclTax, dup: bool) -> Output:
```

Parameters are the rule's inputs in declaration order, named by their ASCII aliases. A name
with no alias that is a keyword in one of the languages — an input called `class`, `type`,
`def` — is written with `_` after it in every language (`class_`, or `class_2` where `class_` is
another name's identifier already), and `rulec api` names it so; the keys of the wire and of the
records keep the declared name (W121). With one output the function
returns that value; with two or more it returns a `NamedTuple` called `Output` whose fields are
the outputs in declaration order.

```python
from single_coupon import single_coupon, CouponKind

out = single_coupon(
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
name from the rule (`"percent"`), which is what the wire format and the logs use.

Two exceptions can come out, and the difference between them matters:

- **`RuleInputError`** (a `ValueError`) — the caller broke the contract: a value outside its
  declared range, or something that is not a member of the enum. Fix the call site.
- **`RuleContradictionError`** (an `AssertionError`) — the *rule* contradicted itself. This
  is the guard described below. It is never the caller's fault.

### TypeScript

```ts
export function single_coupon(subtotal: JPYInclTax, applied: JPYInclTax, kind: CouponKind, rate: Rate, face: JPYInclTax, dup: boolean): Output
```

**Every number is a `bigint`.** The overflow proof (E108) is against int64, and a JavaScript
`number` is exact only to 2^53, so using one would put a silently wrong answer above nine
quadrillion into the one place this tool exists to keep honest.

Each unit is a *branded* bigint — `type JPYInclTax = bigint & { readonly __rulec: "JPYInclTax" }`
— which costs nothing at run time and still refuses a tax-inclusive amount where a
tax-exclusive one was meant. Construct one with `as`.

```ts
import { single_coupon, CouponKind } from "./single_coupon.ts";
import type { JPYInclTax, Rate } from "./single_coupon.ts";

const out = single_coupon(
  10000n as JPYInclTax,
  0n as JPYInclTax,
  CouponKind.PERCENT,
  10n as Rate,
  0n as JPYInclTax,
  false,
);
console.log(out.ok, out.raw);
```

An enum is a frozen object plus a union type, not a TypeScript `enum`. That keeps the whole
file to **erasable syntax**, so `node file.ts` runs it with no build step and no `tsconfig`;
a `tsc` build works just as well. The member spelling is the alias in upper case
(`CouponKind.PERCENT`), and its value is the name from the rule (`"percent"`), the same as in Python.

The two error classes are `RuleInputError` and `RuleContradictionError`, with the same
meanings as in Python.

### JavaScript

```js
export function single_coupon(subtotal, applied, kind, rate, face, dup)
```

The TypeScript with its types taken off, as an ES module (`single_coupon.mjs`): the same
branches, the same helpers, the same `bigint` for every number, held to the same vectors by
`rulec test`. It runs with `node` alone and in a browser as it stands. The enum objects,
`parseCouponKind`, and the two error classes are the same as in TypeScript; what is gone is
the brand, so nothing catches a tax-exclusive amount passed where a tax-inclusive one was
meant — the position Ruby is in.

```js
import { single_coupon, CouponKind } from "./single_coupon.mjs";

const out = single_coupon(10000n, 0n, CouponKind.PERCENT, 10n, 0n, false);
console.log(out.ok, out.raw);
```

### Rust

```rust
pub fn single_coupon(subtotal: JPYInclTax, applied: JPYInclTax, kind: CouponKind, rate: Rate, face: JPYInclTax, dup: bool) -> Result<Output, RuleError>
```

Every number is an `i64`, which is the type the overflow proof (E108) is stated in. A unit is
a newtype over it — `pub struct JPYInclTax(pub i64)` — so the compiler refuses a
tax-exclusive amount where a tax-inclusive one was meant, at no run-time cost. Construct one
with `JPYInclTax(10000)` and read it back with `.0`.

```rust
use single_coupon::{single_coupon, CouponKind, JPYInclTax, Rate};

let out = single_coupon(
    JPYInclTax(10000),
    JPYInclTax(0),
    CouponKind::Percent,
    Rate(10),
    JPYInclTax(0),
    false,
)?;
println!("{} {}", out.ok, out.raw.0);
```

An enum is a plain Rust enum whose members are the aliases in PascalCase
(`CouponKind::Percent`); `as_str()` gives the name from the rule that the wire format uses, and
`CouponKind::parse(&str)` reads one back.

**There is no entry guard on an enum input's value**, unlike Python, TypeScript, JavaScript, Ruby, Go, SQL and NumPy — the last of which checks a whole column with one `np.isin`. A value
of a Rust enum type is one of its variants by construction, so the check the others have to
make at run time is already made by the compiler. Swift, PHP and Java are in the same
position, and so is
the Wasm module, which is the Rust one behind a door that turns an unknown value into an
`error` line before the module is reached. Go's enum is an integer type, so its guard asks
`Valid()`. An input of an ordered enum with a `range` is guarded in every language all the
same: which of its values the range lets in is a question of the run, as a number's range is
(`if campaign < Tier::Basic || campaign > Tier::Gold`, §15.210).

Errors come back as `Err(RuleError)`, whose two variants carry the same distinction as
Python's two exception classes: `RuleError::Input { what, value }` is a contract violation by
the caller, and `RuleError::Contradiction { what }` is the runtime guard described below.
`what` is the sentence, `value` the number that was refused (`None` when the refusal is not
about one), and `Display` puts them together — nothing is formatted on the refusing path,
which is what lets a model checker walk it (below).

It compiles with `rustc` alone — `rustc --edition 2021 -O single_coupon_runner.rs` builds both
the rule and its runner through a `#[path] mod`, with no project file and nothing to fetch.

#### The proofs

Beside the module, `single_coupon_proof.rs` holds proof harnesses for the [Kani Rust
Verifier](https://model-checking.github.io/kani/). Everything in it is behind `#[cfg(kani)]`,
so `rustc` never reads it; `kani single_coupon_proof.rs` does, and so does `rulec test
--proofs` — a pass of its own, skipped and said so when `kani` is not on PATH. It is behind
a flag because it is the one pass whose cost is noticeable: on the corpus of 143 rules it
adds about 800 seconds to a run that otherwise takes seconds. `rulec api` names
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
SingleCoupon.single_coupon(subtotal, applied, kind, rate, face, dup)
```

A module named after the rule, with one module method. Every number is a plain `Integer`, and
Ruby's `Integer` is exact at any size, so the overflow the proof (E108) rules out cannot
happen quietly here either.

**The unit is not in the type.** Ruby has no zero-cost brand, so `money[JPY, incl_tax]` and
`mass[g]` are both `Integer`, and which is which is stated in the comment above the method
and in `rulec api`. This is the one guarantee Ruby gives up relative to the three targets
whose type systems can hold a unit — the proof still holds, but nothing will catch a caller
that swaps two same-typed arguments. Python is in the same position, with `NewType` doing the job only when
a type checker is run.

```ruby
require_relative "single_coupon"

out = SingleCoupon.single_coupon(10000, 0, SingleCoupon::CouponKind::PERCENT, 10, 0, false)
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
enum's: an enum is typed as the union of its own values (`"percent" | "fixed" | "free_ship"`),
which is stronger than the plain Ruby, where every member is a `String` until the entry guard
fires.

It still does not refuse grams where yen were meant. RBS has no newtype either, and
`type yen = Integer` is *the same type* as `type gram = Integer` — measured with `steep`, not
assumed (§15.23). Units stay a matter of the declaration and the comment.

### PHP

```php
SingleCoupon\single_coupon($subtotal, $applied, $kind, $rate, $face, $dup);
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

**The unit is not in the type**, as in Ruby. `money[JPY, incl_tax]` and `mass[g]` are both
`int`, and which is which is stated in the doc comment above the function and in `rulec api`.
What PHP *can* hold is the kind, which is why the entry guard here asks about the range alone.

```php
require_once __DIR__ . '/single_coupon.php';

$out = SingleCoupon\single_coupon(10000, 0, SingleCoupon\CouponKind::PERCENT, 10, 0, false);
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
func SingleCoupon(in Input) (Output, error)
```

Inputs are bundled into an `Input` struct rather than a parameter list, so that a row of
same-typed integers cannot be passed in the wrong order. Fields are the aliases in
PascalCase. With one output the first return value is that value; with two or more it is an
`Output` struct.

```go
out, err := singlecoupon.SingleCoupon(singlecoupon.Input{
    Subtotal: 10000,
    Applied:  0,
    Kind:     singlecoupon.CouponKindPercent,
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
(`singlecoupon.CouponKindPercent`). Each enum also gets `Valid()`, `String()` (which returns
the name from the rule) and `ParseXxx(string)`.

Go returns an `error` where Python raises, and it is a pointer to one of two types:
`*RuleInputError` for a contract violation by the caller and `*RuleContradictionError` for the
runtime guard below — the same split as Python's two exception classes. `errors.As` tells them
apart; the first carries `What`, `Value` and `HasValue` (whether `Value` holds the refused
value), the second `What`, and printing either gives the message.

```go
var refused *singlecoupon.RuleInputError
if errors.As(err, &refused) {
    fmt.Println(refused.What, refused.Value)
}
```

### Swift

```swift
func singleCoupon(subtotal: JPYInclTax, applied: JPYInclTax, kind: CouponKind, rate: Rate, face: JPYInclTax, dup: Bool) throws -> Output
```

Every number is an `Int64`, which is the type the overflow proof (E108) is stated in — `Int`
is the platform's word and only happens to be 64 bits everywhere Swift runs today. A unit is
a struct with one stored property over it — `public struct JPYInclTax { public var value:
Int64 }` — which Swift lays out as the integer itself, so the compiler refuses a
tax-exclusive amount where a tax-inclusive one was meant at no run-time cost, exactly as
Rust's newtype does. Construct one with `JPYInclTax(10000)` and read it back with `.value`.

```swift
let out = try singleCoupon(
    subtotal: JPYInclTax(10000),
    applied: JPYInclTax(0),
    kind: .percent,
    rate: Rate(10),
    face: JPYInclTax(0),
    dup: false
)
print(out.ok, out.raw.value)   // true 1000
```

Identifiers are in Swift's own spelling: the function, the parameters and the enum members
are the aliases in lowerCamelCase (`member_shipping_fee` becomes `memberShippingFee`), and an alias that
lands on one of the language's keywords is written in backticks. `rulec api` states the
spelling it used, so nothing has to be guessed. Types keep their PascalCase.

An enum is a `String`-backed Swift enum whose raw value *is* the source name — which is also
what the wire format carries — so `.rawValue` and `init?(rawValue:)` are the whole conversion
in both directions and no parser is generated. It is `CaseIterable`, so `.allCases` is the
list. **There is no entry guard on an enum input's value**, for the reason given under Rust; one
with a `range` is held to it.

With one output the function returns that value; with two or more it returns a struct named
`Output`. Both it and the brands are `Hashable` and `Sendable`, and `Output` declares a public
memberwise initializer, since the one Swift writes for a public struct is internal and a
caller in another module could not reach it.

Errors are thrown rather than returned: `RuleError.input` is a contract violation by the
caller and `RuleError.contradiction` is the runtime guard described below — the same split as
Python's two exception classes. `RuleError` is `CustomStringConvertible`, so printing one
gives the message.

It compiles with `swiftc` alone — `swiftc single_coupon.swift single_coupon_runner.swift -o
single_coupon` builds the rule and its runner together, with no `Package.swift` and nothing to
fetch. The runner carries `@main` rather than being called `main.swift`, because top-level
code is only allowed in a file of that name and the rule has to be able to sit beside it.

### Java

```java
SingleCoupon.singleCoupon(subtotal, applied, kind, rate, face, dup);
```

One public class named after the rule, with everything nested inside it: the enums, the
errors, `Fired`, `Output` and the static methods. Nested, not top-level, because two rules
generated into the same directory would otherwise each want to be `RuleInputError.java`.

**`long` is the int64 the proof is about.** No widening, no `BigInteger`, no check at the
door: what E108 proves about every intermediate is exactly what the machine word holds.
Overflow wraps rather than trapping, as it does in Go and Rust.

**The unit is not in the type.** Java has no zero-cost wrapper, so `money[JPY, incl_tax]` and
`mass[g]` are both `long` and the javadoc says which is which. The kind *is* declared, so the
entry guard asks about the range alone.

```java
var out = SingleCoupon.singleCoupon(10000, 0, SingleCoupon.CouponKind.PERCENT, 10, 0, false);
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
CREATE VIEW member_shipping_fee AS
WITH "_c0" AS (SELECT "_id", "dest", CAST("weight" AS BIGINT) AS "weight", … FROM "member_shipping_fee_input"),
…
SELECT "_id", "dest", "weight", "total", "member", … AS "fee", "base_row", "payer_row" FROM "_c5" ORDER BY "_id";
```

**Two doors on one query.** `member_shipping_fee.sql` is **one query over a relation of inputs**: provide
`member_shipping_fee_input` with a column `_id` (anything that identifies the row; it comes back
unchanged) and one column per input under its alias, and out come `_id`, the inputs, the
outputs, and one column per table with the number of the row that matched (`base_row`).
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
which the runner registers for SQLite, passing over a NULL as PostgreSQL's do — a row the door
refuses is still worked out, and a value outside an enum leaves one behind. A division by a
value is the same arithmetic as a rounding, over a divisor wrapped in `NULLIF(…, 0)`, so a row
the door refuses with a zero in it is worked out to NULL rather than stopping the query; a
remainder is `%` turned into the one from zero up. A whole square root is a search, which no
single expression holds: the query calls `rulec_isqrt`, which the function file (the other
door, below) defines in PL/pgSQL at its top — create that first on PostgreSQL, as the comment at
the top of the query says — and which the runner registers for SQLite, as it does `LEAST`.

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
exclusive (W114), a `_contradiction` column names them when both match. The runner prints
either as the refusal line the other languages' runners print, in the place of the record
(below).

**The other door — `member_shipping_fee_function.sql`.** The same query, asked for one case at a time:

```sql
CREATE FUNCTION "member_shipping_fee"("dest" text, "weight" bigint, "total" bigint, "member" text)
RETURNS TABLE ("fee" bigint, "base_row" int, "payer_row" int)
LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE
```

The arguments are the inputs in order and what comes back is the outputs followed by the row
that matched in every table; `rulec api` gives the same line under `sql.function`. The body is
the query above, **unchanged**, with one CTE in front of it binding the arguments into a
one-row `member_shipping_fee_input` — a `WITH` name hides a table of the same name, so nothing below
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
note when there is no server to reach. The runner creates the function in a schema of its own,
made for the run, calls it once per vector **by argument name**, holds the answers to the reference
evaluator byte for byte like every other runner, and drops the schema with everything in it. Two
runs on one database do not meet, a function of the same name the database already has is left as
it is, and the run leaves nothing behind. The role it connects as needs `CREATE` on the database,
to make that schema.

Call it by argument name yourself: `SELECT * FROM "member_shipping_fee"("dest" => 'Hokkaido', "weight" =>
1200, "total" => 0, "member" => 'basic');`. Positionally it is ambiguous whenever the rule's alias
is also the name of a built-in (`rank`), and a named argument is the one form a variadic built-in
cannot answer to. In PostgREST — which is what Supabase runs — a function in an exposed
schema is an endpoint with no server code of its own. Installing this file and nothing else
answers:

```
$ curl -X POST localhost:3000/rpc/member_shipping_fee -H 'Content-Type: application/json' \
       -d '{"dest":"Hokkaido","weight":1200,"total":0,"member":"basic"}'
[{"fee":1200,"base_row":1,"payer_row":3}]                                  200

$ curl -X POST localhost:3000/rpc/member_shipping_fee -H 'Content-Type: application/json' \
       -d '{"dest":"Hokkaido","weight":0,"total":0,"member":"basic"}'
{"code":"22023","details":null,"hint":null,"message":"weight is out of range"}  400
```

The raising is what makes the second one a 400 rather than a 200 carrying a number no proof
covers, and `GET /rpc/member_shipping_fee?dest=…` answers too, since the function is `IMMUTABLE`. Ask
for one object instead of an array with `Accept: application/vnd.pgrst.object+json`. Hasura
tracks a function only when it returns `SETOF` a table it already tracks, so there a table or
view of that shape has to be tracked first.

### Wasm

```
wasm/
├── member_shipping_fee.rs           the same module rust/ gets
├── member_shipping_fee_wasm.rs      the crate root: that module behind the canonical ABI
├── member_shipping_fee.wit          the world, for the component model
├── member_shipping_fee_runner.mjs   the Node runner `rulec test` drives
└── _round.rs, _round_test.mjs
```

One file to build, with `rustc` alone — `rulec api` prints this line under `wasm.build`:

```console
$ rustc --edition 2021 -C opt-level=s -C lto -C panic=abort -C strip=symbols \
    --target wasm32-unknown-unknown --crate-type cdylib member_shipping_fee_wasm.rs -o member_shipping_fee.wasm
```

The module exports the canonical ABI of one function, `call: func(input: string) -> string`:
`cabi_realloc` to place the input in the module's memory, `call(ptr, len)`, which returns a
pointer to a (pointer, length) pair holding the answer, and `cabi_post_call(ret)` to free it.
The input is a JSON object with the inputs by name, in the wire form of [formats.md](formats.md)
(a record with them under `"in"` is read the same way); the answer is the record line the other
languages' `_record` writes, or, for an input outside the contract — an unknown enum value, an
input that is not there, a number that is not a whole number and a date that is not `YYYY-MM-DD`
included — the refusal line the runners print, `{"refused":"input","error":"weight is out of
range: 0"}`: the sentence every language says, followed by the value where the module has one as
a number, and `"contradiction"` in the place of `"input"` for a case the rule itself cannot
answer. A host is answered rather than trapped, and never computed on a value it did not send
(§15.151, §15.204); `rulec api` lists the two shapes under `answers`, with the keys of each and
the values `refused` takes (§15.206). Any JSON encoder will do: a name or a value written with
`\u` escapes reads as the characters it stands for. The module imports nothing, so it
instantiates with an empty import object anywhere WebAssembly runs; the shipping rule is
thirty-seven kilobytes.

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
call('{"dest":"Hokkaido","weight":2500,"total":12000,"member":"gold"}');
// {"in":{…},"observed":{"fee":1800},"trace":[{"table":"base","row":2},{"table":"payer","row":3}]}
```

The `.wit` names the same function as the export of a world, so the module becomes a
component with no change to it, and a component host calls it like any other:

```console
$ wasm-tools component embed member_shipping_fee.wit member_shipping_fee.wasm -o member_shipping_fee.embedded.wasm
$ wasm-tools component new member_shipping_fee.embedded.wasm -o member_shipping_fee.component.wasm
$ wasmtime run --invoke 'call("{\"dest\":\"Hokkaido\",\"weight\":1,\"total\":0,\"member\":\"basic\"}")' member_shipping_fee.component.wasm
```

`rulec test` builds the module and holds it to the vectors through the runner
(`ok member_shipping_fee (Wasm) 70 vectors`), and skips the language with a note when `node` or the
`wasm32-unknown-unknown` standard library (`rustup target add wasm32-unknown-unknown`) is
missing.

---

## A rule that walks a sequence

A rule with `elements` and `fold` ([reference §6.2](reference.md#62-elements-and-fold)) takes
one more argument, last: the sequence, as a list of `Element`. `Element` is a record of the
element's own fields, generated beside `Output`, and each field carries the same entry guard
an input of that type carries. A rule that walks two sequences or more takes one list for each,
in the order the `elements` are declared, and each record is named after its sequence:
`LinesElement` and `PaymentsElement` (§15.219).

```python
class Element(NamedTuple):
    """one of freight_rows"""
    row_zone: Zone
    threshold: JPYInclTax
    row_fee: JPYInclTax


def nationwide_freight(freight_rows: list[Element]) -> JPYInclTax:
```

The body is a loop over that list, with one branch per verdict, and the rule's items at the
places the reference evaluator runs them
([reference §6.2](reference.md#before-the-walks-on-each-walk-after-them)): what reads neither an
element nor the walk's answer is computed once, above the loop; what reads a value of one element,
inside it; what reads the answer, once, below it, after the answer is put in the first output.
The rule of [reference §6.2](reference.md#before-the-walks-on-each-walk-after-them), the best
offer among some bids, comes out as:

```python
    opening = reserve + fee  # definition
    answer: int = opening  # the answer for no elements
    stopped = False
    kept: int | None = None
    best_: int | None = None
    for elem in bids:
        ...
        offer = _max(elem.amount, opening)  # definition
        # table judge (policy unique)
        ...
        if verdict is BidVerdict.COUNTED:  # counted
            key = offer
            if best_ is None or key > best_:
                best_ = key
                kept = offer
        if verdict is BidVerdict.IGNORED:  # ignored
            pass
    if _len(bids) > 0 and not stopped:
        held = kept if kept is not None else opening
        answer = held
    best = answer
    # table review_of (policy unique)
    if best >= 500:  # row 1: >=500 | true
```

`held` is the value taken (`take_unique`, `take_first`), else the value kept (`keep_max`), else
the `empty` answer, in every language as in the reference evaluator. A `count` and a `sum` are
written the same way, their totals started above the loop and read below it. What comes out
goes through the same rounding and the same return the rule would have had without the loop.

### Two sequences and a second walk

Each walk is one loop (§15.219). The summaries of one sequence found on one walk share its loop;
a walk over another sequence is a loop of its own; and a walk that reads what an earlier one
found is a second loop over the same list, below the first. A `max` and a `min` start from their
`empty` answer, with a flag that says whether any element has been looked at, and the first
element the `where` lets in replaces it. `tests/corpus/order_settlement.rule` — the lines' total
and their highest price on the first walk, the payments on a walk of their own, and the line at
the highest price on a second walk over the lines — comes out as (the entry guards cut):

```python
def order_settlement(lines: list[LinesElement], payments: list[PaymentsElement]) -> Output:
    ...
    subtotal = 0  # sum
    top_price = 0  # max
    top_price_set = False
    for elem in lines:
        ...
        line_total = elem.quantity * elem.price  # definition
        subtotal += line_total  # subtotal
        if not top_price_set or elem.price > top_price:
            top_price = elem.price
            top_price_set = True
    if subtotal > 25000:
        raise RuleInputError("subtotal is out of range", _repr(subtotal))
    paid = 0  # sum
    for elem2 in payments:
        ...
        paid += elem2.amount  # paid
    if paid > 25000:
        raise RuleInputError("paid is out of range", _repr(paid))
    answer: int = 0  # the answer for no elements
    stopped = False
    taken: int | None = None
    for elem in lines:
        ...
        line_total = elem.quantity * elem.price  # definition
        gap = top_price - elem.price  # definition
        # table pick_line (policy unique)
        if gap <= 0:  # row 1: <=0USD | top
            pick = LinePick.TOP
            trace.append(Fired("pick_line", 1))
        elif gap > 0:  # row 2: >0USD | rest
            pick = LinePick.REST
            trace.append(Fired("pick_line", 2))
        ...
        if pick is LinePick.TOP:  # top
            if taken is None:
                taken = top_off
        if pick is LinePick.REST:  # rest
            pass
    if _len(lines) > 0 and not stopped:
        held = taken if taken is not None else 0
        answer = held
    discount = answer
    balance = (subtotal - discount) - paid  # definition
```

The second loop works the values of one element out again (`line_total`) and runs the entry
guard again, so nothing it reads is carried over from the first; the trace lists the rows the
second walk fires, element by element. A `fold` that is alone on its walk ends the loop with
`break` at `stop`, as before; one that shares the walk with another summary only stops reading,
and the loop goes on for the others. A fold's answer named with `-> <name>` is that name from
then on (`discount = answer`); without one it is the first output.

Every condition the Python module writes is a truth value — a comparison, `isinstance`, or a
name the module only ever gives `True` or `False` — but in the functions that take the inputs
out of the caller's object (`<alias>_from`, §15.207), which read any JSON. A sequence, a number, a
string or an optional value is compared explicitly (`_len(bids) > 0`, `r != 0`, `tag != ""`,
`signature is True`), never tested for being empty or zero. `rulec api` lists the sequence as the last parameter, with the
element's fields under `elements` — a rule that walks two lists each, in order — and the record
written for one call carries each sequence as an array of objects. Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java and Wasm are
generated; SQL and NumPy are refused by name, because one query has nowhere to carry a value from
row to row, and a walk is not a column operation.

## A machine

A rule with a `machine` section ([reference §6.4](reference.md#64-machine-and-scenario)) is
generated as the function every rule gets: it takes the state as an argument, answers the next
one, and keeps nothing. Beside it, the module says where a case starts and whether one has
ended:

```python
INITIAL: State = State.RECEIVED

FINAL: frozenset[State] = frozenset({State.DELIVERED, State.CANCELLED})


def is_final(state: State) -> bool:
    """Whether a case in this state has ended."""
    return state in FINAL
```

The state lives with the caller — in the order's row of a database, say. A new case starts at
`INITIAL`; every call is passed the stored state, and what it answers is stored in its place:

```python
out = order_state(order.state, event, paid)
order.state = out.next_state
```

What `check` proved about the machine is a claim about exactly this: every sequence of calls a
caller can make this way, from `INITIAL`. `is_final` is where one ends: an event that arrives
after that still has a row, which completeness sees to, and the row keeps the state where it
is, which E124 sees to. Each language's
spelling of the three is in the grammar's table and under `machine.constants` in `rulec api`.
The NumPy plan carries them as data, and a target with one door and no module carries none.

The vector suite has **traces** besides the single calls: sequences from the initial state that
take every transition a case can make and every two that can follow one another, and every
`scenario`. `rulec test` has each language's runner play them, handing the state one call
answered to the next call **as that language holds it** — an enum member, not the string it
is written as on the wire — so what is compared is the hand-over a caller does, as well as the
answers. The MCP tool, the Connect service and the PostgreSQL function are one call each, like
the function, and are played the same way. On the page for people the machine is drawn beside
the form, and a button puts the state the call answered back into the form for the next event.

## Enums several rules share, their order and display names

An enum declared with `<` is ordered, and a value may carry a display name
([reference §3](reference.md#declaring-an-enum)). An `enums` file holds enums for several rules
to read ([reference §3.1](reference.md#enums-several-rules-share)). The code they generate:

| language | the module an `enums` file becomes | how a rule's module reads it | the place of a value (ordered enums) | the display name |
|---|---|---|---|---|
| Python | `catalog.py` | `from catalog import Category, Zone`; `from .catalog import …` in a package of `ritsu gen` | `zone_rank(z)`, from 0 | `category_label(c)` |
| TypeScript | `catalog.ts` | `import { Category, Zone } from "./catalog.ts"` and `export { Category, Zone }`; `"./catalog"` in a package | `zoneRank(z)` | `categoryLabel(c)` |
| JavaScript | `catalog.mjs` | `import { … } from "./catalog.mjs"` | `zoneRank(z)` | `categoryLabel(c)` |
| Rust | `catalog.rs` | `pub use crate::catalog::{Category, Zone};`, the module declared beside the rule's by the crate that builds them (the runner: `#[path = "catalog.rs"] mod catalog;`) | `z.rank()`; the enum derives `PartialOrd` and `Ord` in its order | `c.label()` |
| Ruby | `catalog.rb`, `module Catalog`, with `sig/catalog.rbs` | `require_relative "catalog"` and `Category = Catalog::Category` (and the same aliases in the `.rbs`) | `Catalog.zone_rank(z)` | `Catalog.category_label(c)` |
| PHP | `catalog.php`, namespace `Catalog` | `require_once __DIR__ . '/catalog.php';` and `use Catalog\Category;` | `\Catalog\zone_rank($z)` | `\Catalog\category_label($c)` |
| Go | package `catalog` (`catalog/catalog.go` and its `go.mod`) | imports the package — in `rulec gen`'s output by `replace catalog => ../catalog` in the rule's `go.mod`, in a package of `ritsu gen` by the `--module` path — and declares `type Category = catalog.Category` with the constants and `ParseCategory` beside it | the constant itself: the values are declared in their order, so `z >= ZoneNational` compares places | `c.Label()` |
| Swift | `catalog.swift` | the same module: built with the rule's file (`rulec test` adds it to the `swiftc` line from `vectors/<rule>.owners.json`) | `z.rank` | `c.label` |
| Java | `Catalog.java`, the enums nested in `public final class Catalog` | the same package: `Catalog.Category` | `z.ordinal()`, and `compareTo` in the conditions | `c.label()` |
| SQL | none: an enum is the text of its name | — | a `CASE` in the query | in the file that declares a function, one function per enum with display names, `"catalog_shipping_category_label"(v text)` |
| NumPy | none: the plan carries the values | — | the plan's `rank` cells | the plan's `labels`, read by `rule.label(enum, value)` |
| Wasm | none: the types are written into the rule's own crate | — | in the crate | none: the module answers over JSON, which carries values only |

A rule named like an enum it reads (`rule size` reading `enum size`) has a Go package that
declares `func Size`, and a type of that name cannot be declared beside it: the package then
names the type by the package that owns it, `sizes.Size`, as Java names it by its class, and
declares no alias of it — the constants and `ParseSize` are there as for any other enum, and
`rulec api` gives the type of the input's field as `sizes.Size` (§15.239). A rule named like an
enum it declares itself cannot be written apart in Go, where the function and the type share the
package, nor in Java, where the enum is nested in the rule's class: `rulec check` says so (W121).

A value without a display name is shown by its declared name, by every one of these. `rulec
gen` writes the module of each `enums` file a rule reads, whether or not the file is named on
the command line, once however many of the rules given read it; two different files that would
write one module are refused. A cell that compares an ordered enum (`>=gold`) is
written as a comparison of places, `tier_rank(effective) >= tier_rank(Tier.GOLD)` in Python,
on the same line of the rule as any other condition; `max(a, b)` and `min(a, b)` over one are
a helper each, and the entry guard of an input with a `range` over one compares places too.

A machine that carries more than its state (reference §6.4) gets one more constant for each
further carried value: where a case starts, named after its input and typed as the input is.
From `grace_period.rule`:

```python
INITIAL: Subscription = Subscription.ACTIVE

FINAL: frozenset[Subscription] = frozenset({Subscription.SUSPENDED})

INITIAL_DAYS_LEFT: int = 0
```

The caller keeps those values beside the state, starts a case with them, and stores what each
call answers for them in their place:

```python
out = grace_period(sub.status, event, sub.days_left)
sub.status, sub.days_left = out.next_status, out.next_days_left
```

An amount, a quantity or a rate is the branded type (`INITIAL_BALANCE: JPY = JPY(-5000)`), a
truth value is `bool`, another enum's value its member. Python, TypeScript, JavaScript, Rust,
Ruby, PHP and Java spell the constant `INITIAL_DAYS_LEFT`, Go `InitialDaysLeft`, Swift
`initialDaysLeft`; the NumPy plan lists each under `machine.more` with its wire value; SQL and
Wasm, with one door and no module, have no constant, and `rulec api` lists the value and every
language's spelling under `machine.carried`. The traces hand these values over as they hand the
state: the first call of a trace is passed where each starts, every later one what the language
itself answered.

## The units of a package of ritsu's

`ritsu gen` writes a project's rules into one package with its dates and the clients of its
books. There, the brand of an amount of money or of a quantity is not declared by each rule's
module: the package has one type of each unit, in its `units` module, and each rule's module
imports it under the name it would have declared, and hands it on. The names a caller writes are
the ones above. For an amount in cents (`money[USDc]`), the package's `units.ts`, `units.py` and
`units/units.go` declare

```ts
export type USDc = bigint & { readonly __rulec: "USDc" }; // money[USDc]
```

```python
USDc = NewType("USDc", int)  # money[USDc]
```

```go
type USDc int64
```

and the rule's module, in each, takes it from there:

```ts
import type { USDc } from "../units";
export type { USDc };
```

```python
from ..units import USDc as USDc
```

```go
type USDc = units.USDc // money[USDc]
```

One rule's amount is then what another rule takes, however the two write the unit. A price one
rule returns in `money[円, incl_tax]` goes to a rule that takes `money[JPY, incl_tax]`: `units`
declares `YenInclTax`, from the spelling the table gives the unit first, and `JPYInclTax` as
another name of it, and each rule's module names the type as its rule writes it. The clients of
the books in the package take their amounts in the same types, so code written by hand that gives
a book's call what a rule returns is held to the book's unit by the type checker of its language.
A rate keeps its brand in the rule's module, as its integer counts the step that rule declares.
`rulec gen` writes no `units` module, and its modules declare their brands themselves.

## Which values need which before them

A `prerequisites` block ([reference §6.5](reference.md#65-prerequisites)) is generated as two
functions in the module of the file that holds it — a rule's, or an `enums` file's, which a rule
reads its enum from. From `tests/corpus/shared/certifications.rule`, in Python:

```python
def exam_path_requires(value: Certification) -> list[list[Certification]]:
    """What value needs before it can be taken: every group, each by any one of its values. A value that needs nothing has no group; one no case can take has a group with no value in it."""
    if value is Certification.FOUNDATIONS:
        return []
    elif value is Certification.ASSOCIATEDEVELOPER:
        return [[Certification.FOUNDATIONS]]
    elif value is Certification.ASSOCIATEOPERATIONS:
        return [[Certification.FOUNDATIONS]]
    elif value is Certification.PROFESSIONALDEVELOPER:
        return [[Certification.ASSOCIATEDEVELOPER]]
    elif value is Certification.PROFESSIONALARCHITECT:
        return [[Certification.ASSOCIATEDEVELOPER, Certification.ASSOCIATEOPERATIONS]]
    elif value is Certification.SPECIALTYSECURITY:
        return [[Certification.PROFESSIONALARCHITECT]]
    else:
        return [[Certification.PROFESSIONALDEVELOPER, Certification.PROFESSIONALARCHITECT]]


def exam_path_available(value: Certification, held: list[Certification]) -> bool:
    """Whether value can be taken now: it is not in held yet, and every group of what it needs has a value in held."""
    if value in held:
        return False
    for group in exam_path_requires(value):
        met = False
        for p in group:
            if p in held:
                met = True
        if not met:
            return False
    return True
```

The groups are a list of lists returned by a chain over the values, one branch per value in the
enum's order, rather than a constant map: a compiler of typed Python takes it as it stands, and
`mypy --strict` and `pyright --strict` say nothing of it. A value on `start` returns no group; one
written `requires nothing` returns one group with no value in it, which no `held` meets, so
`available` needs no branch of its own for it. What a case holds is the caller's to keep, as a
machine's state is; the functions keep nothing.

| language | the two functions |
|---|---|
| Python | `exam_path_requires(value: Certification) -> list[list[Certification]]`, `exam_path_available(value: Certification, held: list[Certification]) -> bool` |
| TypeScript | `examPathRequires(value: Certification): Certification[][]`, `examPathAvailable(value: Certification, held: readonly Certification[]): boolean` |
| JavaScript | `examPathRequires(value)`, `examPathAvailable(value, held)` |
| Rust | `exam_path_requires(value: Certification) -> Vec<Vec<Certification>>` (a `match` over every value), `exam_path_available(value: Certification, held: &[Certification]) -> bool` |
| Ruby | `Certifications.exam_path_requires(value)`, `Certifications.exam_path_available(value, held)`, with their signatures in the `.rbs` |
| PHP | `\Certifications\exam_path_requires(Certification $value): array`, `\Certifications\exam_path_available(Certification $value, array $held): bool` |
| Go | `ExamPathRequires(value Certification) [][]Certification`, `ExamPathAvailable(value Certification, held []Certification) bool` |
| Swift | `examPathRequires(_ value: Certification) -> [[Certification]]`, `examPathAvailable(_ value: Certification, held: [Certification]) -> Bool` |
| Java | `Certifications.examPathRequires(Certification value)`, `Certifications.examPathAvailable(Certification value, java.util.List<Certification> held)`, the list type written in full so that the class of an `enums` file needs no import |
| NumPy | `rule.requires("<name>", value)` and `rule.available("<name>", value, held)` over the plan's `prerequisites`, for a block a rule holds; an `enums` file, like this one, has no plan |
| SQL, Wasm | none: one entry, and no room for a second function. `rulec gen` says so as it writes them, `rulec api` names them under `without`, and `rulec test` notes that nothing was compared there |

`rulec gen` writes `vectors/<alias>.prereq.tsv` and `.prereq.expected` beside the vectors: one
line per case — a block, a value and what is held — and what the two functions answer for it,
from the reference evaluator. An `enums` file with a block gets them too, with a runner of its own
beside its module in each language. `rulec test` asks every runner with `--prerequisites` and
compares the answers byte for byte (docs/formats.md), and so does the corpus's agreement suite
(`tests/threeway.rs`) for the rules with a block.

## How likely each answer is

A rule with `random` inputs ([reference §6.6](reference.md#66-random-and-chances)) generates one
more function beside the rule's own: it takes the inputs that are not random, runs the rule on
every point of the random ones, and counts each answer. From `tests/corpus/prize_lottery.rule`, in
Python:

```python
class Chances(NamedTuple):
    """How many draws give each answer (ways), and how many draws there are (total). A chance is ways over total (rulec's §15.225)."""
    total: int
    ways: list[tuple[Prize, int]]


def prize_lottery_chances(campaign: bool) -> Chances:
    """The answers of rule prize_lottery when ticket is drawn uniformly: each answer with how many of the draws give it, and how many draws there are. An answer's chance is its count over the total."""
    total = 0
    ways: list[tuple[Prize, int]] = []
    last: RuleInputError | None = None
    _i0 = -1
    while _i0 < 999:
        _i0 += 1
        ticket = _i0
        try:
            out = prize_lottery(ticket, campaign)
        except RuleInputError as e:
            last = e
            continue
        total += 1
        k = 0
        while k < _len(ways) and ways[k][0] != out:
            k += 1
        if k == _len(ways):
            ways.append((out, 1))
        else:
            ways[k] = (out, ways[k][1] + 1)
    if total == 0 and last is not None:
        raise last
    return Chances(total, ways)
```

`prize_lottery_chances(False)` answers `Chances(total=1000, ways=[(<Prize.FIRST: 'first'>, 1),
(<Prize.SECOND: 'second'>, 9), (<Prize.THIRD: 'third'>, 50), (<Prize.NOPRIZE: 'no_prize'>, 940)])`:
the chance of the third prize out of a campaign is 50/1000. The counts are
whole numbers, so the chance is exact, and a caller turns it into a fraction or a percentage as it
needs. It is the rule's own function that is run, so the counts never disagree with it: a point
the function refuses — one a `constraint` rules out, a drawn value past the weights — is no point,
and when no point is left, the function raises the last refusal, as the rule would. The answers
come in the order they are first met: the first random input outermost, each from its lowest value
(an enum's values in their order, `false` before `true`).

A rule whose one random input is read by nothing but the `draw` line of its pick by weight is
counted from the weights instead (§15.226). The walk is run once, in a private function the rule's
own functions call too, which hands back the running totals of the weights; then the rule is run
once for each element weighed, at the first drawn value that takes it, and its count is its weight.
From `tests/corpus/case_routing.rule`:

```python
def case_routing_chances(reviewers: list[Element]) -> Chances:
    """The answers of rule case_routing when roll is drawn uniformly: each answer with how many of the draws give it, and how many draws there are. An answer's chance is its count over the total."""
    total = 0
    ways: list[tuple[int, int]] = []
    _, _, cuts = _case_routing_walked(0, reviewers)
    if _len(cuts) == 0 or cuts[_len(cuts) - 1] == 0:
        return Chances(10000, [(case_routing(0, reviewers), 10000)])
    low = 0
    for cut in cuts:
        if cut > low and low <= 9999:
            n = _min(cut, 10000) - low
            out = case_routing(low, reviewers)
            k = 0
            while k < _len(ways) and ways[k][0] != out:
                k += 1
            if k == _len(ways):
                ways.append((out, n))
            else:
                ways[k] = (out, ways[k][1] + n)
            total += n
        if cut > low:
            low = cut
    return Chances(total, ways)
```

The walk itself adds each weight to the running total as the element lands on the arm, takes the
first element the drawn value is below, and refuses the call once the walk is over when the drawn
value is not below the total:

```python
        if presence is Presence.INOFFICE:  # in_office
            weight = elem.capacity
            weights += weight
            cuts.append(weights)
            if taken is None and roll < weights:
                taken = elem.reviewer_id
    if weights > 0 and roll >= weights:
        raise RuleInputError("roll is not below the total of the weights", _repr(roll))
```

The private function is `_case_routing_walked` in Python, Rust (and the Wasm build of it), Ruby and
PHP, `_caseRoutingWalked` in TypeScript, JavaScript, Swift and Java, `caseRoutingWalked` in Go;
NumPy and SQL do not walk a sequence at all. The rule's public functions are the ones they always
were.

| language | the function | what it answers |
|---|---|---|
| Python | `prize_lottery_chances(campaign: bool) -> Chances` | `class Chances(NamedTuple)`: `total: int`, `ways: list[tuple[Prize, int]]` — a rule of several outputs counts its `Output` |
| TypeScript | `prizeLotteryChances(campaign: boolean): Chances` | `interface Chances { readonly total: bigint; readonly ways: [Prize, bigint][] }` |
| JavaScript | `prizeLotteryChances(campaign)` | `{ total, ways }`, the counts as `bigint` |
| Rust | `prize_lottery_chances(campaign: bool) -> Result<Chances, RuleError>` | `pub struct Chances { pub total: i64, pub ways: Vec<(Prize, i64)> }` |
| Ruby | `PrizeLottery.prize_lottery_chances(campaign)` | `Chances = Struct.new(:total, :ways)`, typed in the `.rbs` |
| PHP | `\PrizeLottery\prize_lottery_chances(bool $campaign): Chances` | `final class Chances { int $total; list<array{0: Prize, 1: int}> $ways }` |
| Go | `PrizeLotteryChances(in Input) (Chances, error)` | `Chances{Total int64; Ways []Way}`, `Way{Out Prize; Count int64}`. It takes the rule's own `Input`, whose random fields it does not read |
| Swift | `prizeLotteryChances(campaign: Bool) throws -> Chances` | `struct Chances { total: Int64; ways: [Way] }`, `struct Way { out: Prize; count: Int64 }` |
| Java | `PrizeLottery.prizeLotteryChances(boolean campaign)` | `record Chances(long total, List<Way> ways)`, `record Way(Prize out, long count)` |
| NumPy | `rule.chances(**fixed)` | `(total, [(outputs, count), …])`: the random axes as one column of every point, evaluated at once |
| SQL, Wasm | none | a query that walked the points would give up the one-to-one of its branches and the rule's rows; Wasm has one entry, and no room for a second function. `rulec gen` says so as it writes them, `rulec api` names them under `without`, and `rulec test` notes that nothing was compared there |

One call costs as many runs of the rule as there are points of the random inputs, and `rulec api`
says how many (`chances.runs`); past a million, `rulec check` says so (W131). A pick by weight
counted from its weights runs the walk once and the rule once per element.

`rulec gen` writes `vectors/<alias>.chances.jsonl` and `.chances.expected` beside the vectors: the
inputs of a call, one line per combination of the inputs that are not random the vectors meet, and
what the reference evaluator counted for it (docs/formats.md). Every runner answers `--chances`:
for each line it calls the function and prints `{"total":N}`, then a line per answer, its count, a
tab and its outputs as the record writes them; `rulec test` holds that to the second file.

## A rule whose inputs are projected from the caller's object

A rule whose inputs say `from`
([reference §3.3](reference.md#33-shape-and-from--where-the-callers-object-holds-an-input))
gets one more function beside the others: the same call, taking the caller's object instead of
the scalars.

```python
def order_shipping_from(order: dict[str, Any]) -> Yen:
    """Reads the inputs out of the caller's object and calls this rule. …"""
    return order_shipping(
        _zone_of(_at(_at(order, "shipping", "注文"), "zone", "注文.shipping"), "注文.shipping.zone is not a value of enum 地域"),
        _any([_bool_at(_e, "chilled", "注文.lines[]") for _e in _seq_at(order, "lines", "注文")]),
        _len(_seq_at(order, "lines", "注文")),
    )
```

```typescript
export function order_shipping_from(order: _Obj): Yen {
  return order_shipping(
    (_enumOf(Object.values(Zone), _at(_at(order, "shipping", "注文"), "zone", "注文.shipping"), "注文.shipping.zone is not a value of enum 地域") as Zone),
    (_seqAt(order, "lines", "注文").reduce((_a, _e) => (_boolAt(_e, "chilled", "注文.lines[]") === true) || _a, false) as boolean),
    BigInt(_seqAt(order, "lines", "注文").length),
  );
}
```

It is the glue an application writes by hand otherwise, once per input per language, with
nothing checking it. Here the paths were held to the contract by `rulec check` before the
function was written, so a field the contract renamed is E121 at build time rather than a
`KeyError` in production. The object is **read and never named**: the parameter is a plain map,
because a type for it would be a domain object model and this tool makes none.

**Every value it reads is held to what the rule takes, as the rule's own door holds an argument**
(§15.234 of DESIGN.md). A number is a JSON integer, or a number written with a point that is
whole (`1500.0`, as at the door, §15.205); a truth value is JSON's `true` or `false`; a string is a
string; a date is a `YYYY-MM-DD` string of a day the calendar has, read as its day number; a value
of an enum is one of the enum's. A field a `where` tests is held to what the contract carries
there. A value that is not is refused with the module's `RuleInputError`, the same sentence in all
five, naming the value's path as the rule writes it — `order.lines[].amount_usd is not an
integer`, with the value beside it where the language's error carries one (not in PHP, whose
error holds an integer). A place on the way that is not an object (`order.lines[] is not an
object`), a value that is not there or is `null` (`order.lines[].fragile is missing`), and a
collection that is not an array (`order.lines is not a sequence`) are refused the same way. The
function reads in one order in every language: the inputs as the rule declares them, then the
sequences; along a collection, element by element, the value the `where` tests and then, when the
element passes, the value added up or compared; `any` and `all` test every element. So one
object stops at the same value in each.

The readers are small functions written beside it, only the ones the rule uses: `_at` (the value at
a key of an object), `_int_at`, `_bool_at`, `_str_at`, `_date_at`, `_num_at` and `_seq_at` (the
value at a key, held to its kind), the same ending in `_of` for a value already read, `_int_pb` for
an integer a `.proto` carries, `_opt` for an optional input and `_pget` for a field of an element of
a `.proto` collection. A value of an enum is read by a function of that enum's own, named after its
type and answering that type: `_zone_of` for `Zone`, with one more `_` where the module already has
the name (`_seq_of_` for an enum `Seq`, since `_seq_of` reads a collection). One function taking the
type as an argument would need a type variable, a statement at the top of the module that is
computed when the module runs, which a compiler of typed Python that builds the module ahead of
time cannot take.

TypeScript and JavaScript write the others in camelCase (`_intAt`, `_seqOf`) and read every enum
through one `_enumOf`, given the enum's values.

Ruby writes them as methods of the module, with their signatures in the `.rbs`, and PHP as
functions of its namespace.

A rule may project some inputs and pass the rest: the ones with no `from` stay parameters of
the projection function, after the objects, and a sequence the object does not hold stays last.

A `sum`, a `max` or a `min` reads one field of every element that passes, and a sequence taken
from the object is built where the function calls the rule — an element of the rule's own type
for each element of the collection. From `order_handling_fee` in the corpus, in Python:

```python
def order_handling_fee_from(order: dict[str, Any]) -> USD:
    return order_handling_fee(
        USD(_sum(_int_at(_e, "amount_usd", "order.lines[]") for _e in _seq_at(order, "lines", "order"))),
        USD(_largest([_int_at(_e, "amount_usd", "order.lines[]") for _e in _seq_at(order, "lines", "order") if _bool_at(_e, "fragile", "order.lines[]")], 0)),
        [Element(quantity=_int_at(_e, "quantity", "order.lines[]"), amount=USD(_int_at(_e, "amount_usd", "order.lines[]"))) for _e in _seq_at(order, "lines", "order")],
    )
```

`_largest` and `_smallest` are two helpers written beside it when the rule needs them: the
largest (smallest) value of a list, or the `empty` value when the list is empty. Each of the
other four has the same sum and the same two helpers or their equivalent, and reads a date as
its day number, as it is read everywhere. A sequence taken from the object is no longer a
parameter: the function takes the objects, the inputs no `from` reaches and the sequences no
`from` takes, in that order.

TypeScript and JavaScript take the values of the elements that pass with `flatMap`, add the
BigInts up with `reduce`, and make each element an object literal (`{ quantity: …, amount: … }`,
of the element's interface in TypeScript).

Ruby takes them with `flat_map`, adds up with `sum` and writes `max || <empty>` and
`min || <empty>` for the two helpers; PHP takes them with `array_map`, flattens with
`array_merge` and adds up with `array_sum`. Ruby makes each element the element's `Struct` and
PHP the element's class, both given the fields in the order they are declared.

From a JSON Schema, an optional input (`T?`) reads its field so that a missing one — or a
missing object on the way to it — is none: `_opt(order, ("coupon", "kind"), "order")` in the
Python module, and the same in each of the other four; a value that is there is held to its kind
like any other. Every other input reads its path through `_at`, which refuses a value that is not
there, which is why `rulec check` requires a field a required input reads to be `required` in the
contract (E122).

From a `.proto`, the object is taken in the JSON form protojson gives it, and a path goes through
one helper, `_proto`, given each step's JSON name and `.proto` name, what to read when protojson
left the field out (or wrote `null`, which protojson reads as the default), and where the path
starts, so that a place on the way that is not an object is refused by its path:

```python
_region_of(_proto(shipment, (("destination", "destination"), ("region", "region")), "", "", "出荷"), "出荷.destination.region is not a value of enum 地域"),
_any([_str_of(_pget(_e, "handling", "handling", "HANDLING_STANDARD", "出荷.parcels[]"), "出荷.parcels[].handling") == "HANDLING_FRAGILE" for _e in _seq_of(_proto(shipment, (("parcels", "parcels"),), [], [], "出荷"), "出荷.parcels")]),
_len(_seq_of(_proto(shipment, (("parcels", "parcels"),), [], [], "出荷"), "出荷.parcels")),
Yen(_int_pb(_proto(shipment, (("declaredValueJpy", "declared_value_jpy"),), 0, 0, "出荷"), "出荷.declared_value_jpy")),
```

A step is looked up under its JSON name first — `json_name`, or the lowerCamelCase of the name —
and then under the name the `.proto` writes. A field that is not there reads as its default — 0, `""`, false, no
elements, an enum's value numbered 0 — and an optional input reads an unset message or
`optional` field as none. A 64-bit integer, which protojson writes as a string, is read from the
decimal string it is or from a number, and nothing else (`_int_pb`), before a `where` compares it
(a BigInt in TypeScript and JavaScript), and an enum is compared by its value's name.

**Python, TypeScript, JavaScript, Ruby and PHP are generated.** Go, Swift, Java and Rust hold
the caller's object as a type, and naming that type would mean generating it or following the
caller's own; SQL takes a relation of flat columns, NumPy takes columns, and the Wasm ABI takes
one JSON object of the rule's own inputs; `rulec gen` names each of the seven as it writes the
rest, with the reason. The **path check applies to every target equally** —
it happens in `check`. `rulec api` lists it all under `projection`: the contracts, the path of
every projected input, every sequence taken from an object with the path of each field of its
elements (`elements`, empty when there is none), the function name and signature in each of
the five, and under `without` the seven that have none, each with its reason.

## A rule merged with contributions

What `rulec gen` writes for a rule given with contributions to it (`rulec gen order_discount.rule
contrib/order_discount/`, reference.md §6.7) is one function, under the rule's name, in every
language: the rule with the contributions merged into it. Its parameters are the rule's inputs, then
the inputs each contribution adds, in the order they were merged. A contribution's tables, clauses
and definitions are inside it, as an `apply`'s are, their names prefixed with the contribution's —
`regional_campaign:campaign_parts` in the trace and on the page, `regional_campaign_campaign_off`
where the language needs an identifier — and an open value is a local of the function, computed at
the place the rule writes it, from the empty value of a `max` or a `min` and every term:

```python
# Code generated by rulec 0.38.0. DO NOT EDIT.
# Source: order_discount.rule (rule order_discount v1, sha256:67459a43383579d2)
# Contributions: regional_campaign = contrib/order_discount/regional_campaign.rule (signature:fe7307da2694d75a, sha256:614de68c6eea81c6)
…
def order_discount(subtotal: USD, member: bool, coupon: bool, via: Channel, north: bool) -> USD:
…
    stackable = (member_off + channel_off) + regional_campaign_campaign_off  # definition
    exclusive = _max(_max(0, coupon_off), regional_campaign_voucher_off)  # definition
    cap = _min(50, regional_campaign_campaign_cap)  # definition
    discount = _min(_min((stackable + exclusive), cap), subtotal)  # definition
```

The header names each contribution with its file from the rule's directory, the signature it
pinned the rule's shape at, and the SHA-256 of its bytes (the first 16 hex digits), so the file says
which contributions it was made with. A contribution given without its rule generates the rule it
adds to, merged with it and with the contributions it comes after; a contribution has no module of
its own. A value a contribution adds to an enum of an `enums` file is a member of that file's
module, whose header names the contribution. A rule that applies a rule a contribution was merged
into takes the contribution with the applied rule's rows, and its header names it under the apply's
name (`elig:m_elig`, §15.237). Under `ritsu gen`, each rule's module is the rule
merged with every contribution of the project that reaches it. The functions beside the rule's are
the merged rule's too: the one that counts the chances of a rule with random inputs takes an input a
contribution adds among its parameters (`prize_lottery_chances(campaign, holiday)`), and the one that
reads a rule's inputs out of the caller's object reads a contribution's input that says `from`, and
takes one that does not after the objects (`handling_fee_from(order, rush)`).

`rulec api` gives the same under `contributions` and the open values under `opens`
([formats.md](formats.md#api)); the vectors, the certificate and the page are those of the merged
rule.

## The digest in the header

Every generated file names its source in its header — `rule member_shipping_fee v4, sha256:0b6b99dbccf4` — and
`rulec api` gives the whole digest as `source_sha256`. It is the SHA-256 of the rule file's
bytes, computed by rulec itself, so `gen --check` and a reader of the header agree on what was
generated from what. A rule transcribed from a document names it too — `Cites: measures_act = law
332AC0000000026 asof 2026-04-01 (第91条 sha256:85faf53f6f6e8196)` — one line per `source`, and
`rulec api` lists the same under `sources`, so the file says which text of the law it was made
from. A file source carries its `url` there too when it has one, and the tables taken out of it
(`Cites: terms = file tariff.md url https://raw.githubusercontent.com/o/r/a1b2c3d/docs/tariff.md
sha256:… (table1 sha256:…)`), which is what lets a reader of the generated code go and look at the
document, and at the table, the rows were transcribed from. A law from a database other than
e-Gov names it in both places — `Cites: osha = law ecfr 29 CFR 1910 asof 2026-01-01
("§1910.157" sha256:…)`, and `"db": "ecfr"` beside the id under `sources`.


A rule that applies another names the callee and its pin, one line per `apply`: `Applies: retirement
= retirement_pay.rule (meaning:aadc7468b292ff52)`, or `(sha256:…)` for a heading pinned at the
callee's bytes as before §15.230. A contribution merged into the callee is part of what came in,
and is named after the rule's own contributions, under the apply's name as the names an apply
brings in are written (§15.237): `Contributions: elig:m_elig = ../mods/m/elig.rule
(signature:27c066c70c2eb169, sha256:59b421dbd9bf48e4)` in a rule that applies `elig`.
## The rows that matched

Beside every function there is a twin with `_traced` on its name, spelled `Traced` wherever the
language's own convention capitalises it (the table below says which).
It takes the same inputs and returns, beside the outputs, the rows that matched: one per
table, in order, each as the table's name and its 1-based row number. The plain function
calls it and drops the trace, so the branches exist once, in the traced one.

| | the twin | the row |
|---|---|---|
| Python | `def single_coupon_traced(subtotal: JPYInclTax, applied: JPYInclTax, kind: CouponKind, rate: Rate, face: JPYInclTax, dup: bool) -> tuple[Output, list[Fired]]:` | `Fired`, a `NamedTuple` of `table`, `row` and `label` (`""` when the row has none) |
| TypeScript | `single_coupon_traced(…): [Output, Fired[]]` | `{ table: string; row: number; label?: string }` |
| JavaScript | `single_coupon_traced(…)`, returning `[out, trace]` | `{ table, row, label? }` |
| Rust | `single_coupon_traced(…) -> Result<(Output, Vec<Fired>), RuleError>` | `Fired { table: &'static str, row: u32, label: &'static str }` |
| Ruby | `SingleCoupon.single_coupon_traced(…)`, returning `[output, trace]` | `Fired`, a `Struct` of `table`, `row` and `label` |
| PHP | `function single_coupon_traced(int $subtotal, int $applied, CouponKind $kind, int $rate, int $face, bool $dup): array`, returning `[$out, $trace]` | `Fired`, a final class of readonly `table`, `row` and `label` |
| Go | `func SingleCouponTraced(in Input) (Output, []Fired, error)` | `Fired{Table, Row, Label}` |
| Swift | `singleCouponTraced(…) throws -> (Output, [Fired])` | `Fired(table:row:label:)`, `label` defaulting to `""` |
| Java | `public static Traced singleCouponTraced(long subtotal, long applied, CouponKind kind, long rate, long face, boolean dup)` | `Fired`, a record of `table`, `row` and `label`; `Traced` is the pair of `value()` and `trace()` |
| SQL | none: the answer is the row | one column per table, `decide_row`, holding the row number; NULL for a table that another table of the same output beat |
| Wasm | none: the answer of `call` is the record line, `trace` beside `observed` | `{"table":…,"row":…}` objects in that line, with `"label"` when the row has one |
| NumPy | `rule.traced(**{column: sequence})`, returning `(outputs, fired)` | one `(picked, rows)` pair per definition set: `picked[i]` indexes `rows`, and each entry is the `{"table":…,"row":…,"label"?:…}` the row was written in |

The row numbers are the ones `rulec doc` prints in its `#` column and the ones a `verify` or
`replay` report clusters by, so a trace taken from a log reads against the page for people
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
{"tag":"order:1234567","in":{"dest":"Kagoshima","weight":800,"total":4200,"member":"basic"},"observed":{"fee":800},"trace":[{"table":"base","row":3},{"table":"payer","row":3}]}
```

Write that line to a log and the records `replay` and `diff` need come out of the generated
code itself, in the wire form of §10.2 — an enum as its name, a date as `YYYY-MM-DD`, a
number as an integer in its declared unit — with nothing to extract or convert afterwards.
An empty tag is left out. The function builds the line itself, so no language gains an import
for it.

| | the function |
|---|---|
| Python | `def single_coupon_record(subtotal: JPYInclTax, applied: JPYInclTax, kind: CouponKind, rate: Rate, face: JPYInclTax, dup: bool, out: Output, trace: _Trace, tag: str = "") -> str:` |
| TypeScript | `single_coupon_record(…, out: Output, trace: Fired[], tag = ""): string` |
| JavaScript | `single_coupon_record(…, out, trace, tag = "")` |
| Rust | `single_coupon_record(…, out: Output, trace: &[Fired], tag: &str) -> String` |
| Ruby | `SingleCoupon.single_coupon_record(…, out, trace, tag = "")` |
| PHP | `function single_coupon_record(…, Output $out, array $trace, string $tag = ''): string` |
| Go | `func SingleCouponRecord(in Input, out Output, trace []Fired, tag string) string` |
| Swift | `singleCouponRecord(…, out: Output, trace: [Fired], tag: String = "") -> String` |
| Java | `public static String singleCouponRecord(…, Output out, List<Fired> trace, String tag)` |
| SQL | none: the answer is the row, and the runner writes the record from it |
| Wasm | none: the record line is what `call` returns |
| NumPy | none: the plan names no function, and the runner writes the record from the columns |

The generated runner prints exactly this line for every vector, and the expected file `gen`
writes beside the vectors is in the same format, so `rulec test` holds the record function
— the wire form of every input, dates included — to the reference evaluator in every language.
An input the door refuses gets a line of its own in the place of the record, and the runner
reads on: which of the two errors the module raised, and its sentence (§15.204).

```json
{"refused":"input","error":"weight is out of range"}
```

Every runner reads the wire the way the module's door would hold it: an input that is not
there, or `null` for one that is not optional, is `weight is missing`; a value of another kind
is the sentence of its kind (`is not an integer`, `is not a date`, `is not a value of enum
zone` — the enum by the name the rule declares it under — `is not a boolean`, `is not a
string`), and the sequence a walk reads `is not a sequence` or has `an element of … that is not
an object`. A string that holds a number is not one, a date the calendar does not have
(2026-02-30) is not a date, and 1000.5 is not a whole count of its unit, where `1000.0` and
`1e3` are 1000 (§15.205). The sentence is the
same in every language and is the one the reference evaluator says of the same input.
`rulec api` names the function under `record` and gives its signature under
`record_signature`.

## An output that may have no value

An optional output (`price : money[USDc]?  round down(1USDc)`) answers in each language the
way that language writes an optional input, so an input that may have no value passes through
to an output unchanged:

| | no value | a value |
|---|---|---|
| Python | `None` | the type it wraps (`USDc \| None`) |
| TypeScript, JavaScript | `null` | the type it wraps (`USDc \| null`) |
| Rust | `None` | `Some` of the type it wraps (`Option<USDc>`) |
| Ruby | `nil` | the value |
| PHP | `null` | the value (`?int`) |
| Go | `nil` | a pointer to the type it wraps (`*USDc`) |
| Swift | `nil` | the type it wraps (`USDc?`) |
| Java | `null` | the boxed type (`Long`, `Boolean`) |
| SQL | `NULL` | the column's value |
| Wasm | `null` in the answer | the value |
| NumPy | `None` in an array of objects | the value, in the same array |
| MCP, the record | `null` | the value |
| Connect | the field left unset | the field set |

A row that answers `none` writes the absent value, and every other row a value of the type the
output wraps, read in that type. The rounding and the unit's type are put on a value only where
there is one:

```python
    # table price_of (policy unique)
    price: int | None
    if not media_only:  # row 1: false | - | none
        price = None
        trace.append(Fired("price_of", 1))
    elif media_only and weight <= 16:  # row 2: true | <=1lb | 439USDc
        price = 439
        trace.append(Fired("price_of", 2))
```

```python
    return Output(None if price is None else USDc(_round_down(price, 1)), None if zone_number is None else _round_down(zone_number, 1), kind, signed, ships, note), trace
```

Rust maps the `Option` (`price.map(|__v| USDc(round_down(__v.0, 1)))`), and Go takes no
address of a constant, so a row puts its value where `new` points and the rounding makes a
pointer of its own only where there is a value:

```go
	var priceOut *USDc
	if price != nil {
		v := USDc(roundDown(int64(*price), 1))
		priceOut = &v
	}
```

## The two guards

**The entry guard** enforces at run time what the proof assumed. Every numeric input is
checked against its declared `range`, and every enum input against its set of values — an
ordered enum's with a `range` against the values the range lets in (§15.210); then
every `constraint` — between two inputs, or between inputs added up and one more, the sum taken
on the step the inputs share, which E108 has shown fits in int64 — and a date input that takes
its range from a koyomi date against the days that date comes to. If the check were absent, a caller outside the
declared domain would get a silently wrong number instead of an error — and the completeness
proof says nothing about inputs the rule never declared.

```python
if not 0 <= subtotal <= 1000000:
    raise RuleInputError("subtotal is out of range", _repr(subtotal))
```

The sentence and the value travel apart. The error carries both — `what` is the sentence,
`value` the number that was refused — and puts them together only when it is printed, so
the message is what it always was and a caller can react to the value without parsing it
back out of a string. The Rust error is a pair of struct variants for the same reason, and
nothing on the refusing path formats anything, which is what lets a model checker walk it
([the proofs](#the-proofs)). In Python `value` is a string, the `repr()` of what was
refused (`None` when the refusal is about no one value); the message reads the same. The
functions that hold the rule's names call the builtins under names the module gives them
(`_isinstance = isinstance`, `_int = int`, `_repr = repr`, one line each, above the
functions), so an input called `int` or `len` does not stand in their way.

A number that is not an integer is refused before the range is looked at, in every language
where a caller can pass one. A float sits inside any range: 18.3 for a rate declared in steps
of 0.1% would pass the check above, be taken as 1.83%, and be answered without a word. The
three statically typed targets need no such line; their types are the line. In SQL the guard
is a column, `_input_error`, because a query cannot stop: NULL inside the domain, the
sentence outside it ([below](#sql)).

```python
if not _isinstance(rate, _int) or _isinstance(rate, _bool):
    raise RuleInputError("rate is not an integer", _repr(rate))
```

A number whose value is whole is the integer it is, at every door (§15.205): `1000.0`, `1e3`,
`1.5e3` and `-0.0`, which JSON does not tell apart from `1000`, `1000`, `1500` and `0`, and which
a caller's `float` writes. The door takes it as that integer and holds it to the range and the
step like any other; `1000.5` is still not an integer, and the string `"1000"` still not a
number. The module of a language a caller can pass anything to takes it at its own door too — a
float, or a whole `number` where the type says `bigint` — and goes on with the integer, in an
element of a walk as in an input:

```python
if _isinstance(rate, _float) and _float(rate).is_integer():
    rate = Rate(_int(rate))
if not _isinstance(rate, _int) or _isinstance(rate, _bool):
    raise RuleInputError("rate is not an integer", _repr(rate))
```

The readers that keep a number as its digits — Go's `json.Number`, and the readers the Rust and
Java runners and the Wasm entry carry — work the value out of the digits and the exponent
exactly and take only what fits in 64 bits. The ones that hold it as a double — the runners of
Python, Ruby, PHP, Swift, NumPy and SQL — take a double whose value is whole, and read a number
past 2^53 written with a point as the double nearest it; PHP and Swift refuse what does not fit
in 64 bits, as before. TypeScript and JavaScript took it already, since `JSON.parse` makes
`1000.0` the number `1000`. PHP's module keeps its declared `int` (§15.203), so a caller with
`declare(strict_types=1)` gets PHP's own `TypeError` for a float, and the runner hands it the
integer. A record says the input as the integer it read.

A truth value is JSON's `true` or `false` and nothing else, at every door that reads one: the
module's entry guard where a caller can pass any value, and every reader of the wire — the
runners, the MCP servers, the Connect runner, the Wasm entry, the runners of the query and of
the function, and the NumPy plan. The string `"false"`, the numbers `0` and `1` and any other
value are refused the way a number that is not an integer is, with the same sentence in every
language and in the language the code was generated in; an optional truth value takes `null`
as the absent value and holds anything else to the same. The projection functions hand what
the caller's object holds to that guard as it came. Before, `"false"` was true in five
languages and false in seven, and only the MCP servers refused it (§15.203).

```python
if not _isinstance(express, _bool):
    raise RuleInputError("express is not a boolean", _repr(express))
```

`rulec api` states the same bounds, taken from the same place, so an integration built from
the inventory cannot send values the guard rejects. It names each language's two errors too:
`errors` gives the names to catch, and `error_types` says which one the entry guard raises
(`input`) and which one the guard below raises (`contradiction`), with the fields a caller reads
from each ([formats.md](formats.md#api)).

An optional input is held to the same checks whenever it has a value, and passes the door
when it has none, on its way to the row the table writes for the absent value. Its range and
its step are those of the type it wraps:

```python
if ship_on is not None:
    if not _isinstance(ship_on, _int) or _isinstance(ship_on, _bool):
        raise RuleInputError("ship_on is not an integer", _repr(ship_on))
    if not 20454 <= ship_on <= 20818:
        raise RuleInputError("ship_on is out of range", _repr(ship_on))
```

A cell of an optional column tests a value that is there: a comparison, a literal and a set
of values are false of the absent value in every language, and `-` and `not:` take it, as
`check` reads them. Before each comparison asked first whether there was a value, the absent
value raised in some languages and was read as a value in others.

The NumPy plan carries the same guard as data, in the language the code was generated in, and
its evaluator checks a whole column at a time in the same order: each column's type, enum
and range, then each constraint, then the days. It refuses the first element that fails,
with the sentence the other languages raise and the place of that element, so a caller
can tell which one it was without parsing the text:

```text
rulec_np.RuleInputError: the constraint does not hold: declared <= cover (row 1)
```

A value that is not a whole number is refused there too; a float that is a whole number is the
integer it equals, as every door reads it (§15.205). The error carries the place as `row` beside
`what` and `value` (`None` when a whole column is refused, missing or of another length), and
`rulec api` lists the two errors and their fields under the `numpy` entry.

**The contradiction guard** is the other half of W114. When two rows of a `policy unique`
table might overlap and the checker could neither construct an input that proves it nor prove
that none exists, it does not pretend either way: it warns, and the generated code carries a
guard that stops rather than silently picking the earlier row.

```python
# guard: W114 (table decide, row 1 × row 2): a pair of rows whose exclusivity could not be proven statically
if high and low:
    raise RuleContradictionError("table decide: row 1 and row 2 matched at the same time")
```

`high` and `low` compare a doubled amount with an odd boundary, so no whole value reaches it —
but the elimination that decides these pairs works over the rationals and stops half way.
Derived values that share an input, and the thresholds inside a boolean definition, get no
guard any more: they are proved apart.

If this ever fires in production, it is evidence — the overlap the checker could not decide is
real, and the rule needs fixing. The NumPy plan names each such pair by the places of its rows,
and its evaluator raises `RuleContradictionError` with the same sentence and the place of the
element both rows matched.

---

## The rule as an MCP tool

Beside the module, `gen` writes the rule as one MCP server: `<alias>_mcp.py` next to the
Python module, `<alias>_mcp.mjs` next to the JavaScript one, and the same server with its
types on in the `typescript` directory. It is for the agent that **calls** the rule, where
`rulec mcp` is for the agent that writes one. Registered as a stdio server, the rule is one
tool named after its alias:

```console
$ claude mcp add member_shipping_fee -- python3 generated/python/member_shipping_fee_mcp.py
```

**The same server speaks MCP's Streamable HTTP**, which is what the places that only accept a
URL need — a chat client's custom connectors, an agent builder, a workflow product's MCP
node:

```console
$ python3 generated/python/member_shipping_fee_mcp.py --http 8000
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
rate, the step in its description; an enum as its listed names, and an ordered enum's input
with a `range` as the names the range lets in (§15.251); a date as `YYYY-MM-DD` — and
its result is the line the record function writes, as text and as `structuredContent`:

```json
{"in":{"dest":"Kagoshima","weight":800,"total":4200,"member":"basic"},"observed":{"fee":800},"trace":[{"table":"base","row":3},{"table":"payer","row":3}]}
```

So an answer carries the rows that decided it, and one call is one fixtures record. Started
with `--record calls.jsonl`, the server appends every answered call to that file, which
`rulec fixtures lint`, `replay` and `diff` read as it stands: what the agent asked becomes
the record the next revision is measured against.

A call the rule cannot take is refused, not answered: `isError` is set, the text is the error's
class and the sentence every door says, the value after it (`RuleInputError: weight is out of
range: 0`), and it names the
argument — one missing, one extra, a value outside its range, a name that is not a member of
the enum, a truth value that is not `true` or `false`, a number that is not an integer (18.3 for a rate in steps of 0.1% is refused, not
read as 1.83%). The module's two error classes are what reach the caller, under their names.

### The answer, with the table beside it

A host that renders **MCP Apps** ([SEP-1865](https://blog.modelcontextprotocol.io/posts/2025-11-21-mcp-apps/))
gets more than the record. Beside the server, `gen` writes `<alias>_page.html` — the page
`rulec doc --format html` renders, byte for byte — and the server offers it as the tool's
view:

```json
{"uri":"ui://member_shipping_fee/table","name":"member_shipping_fee","mimeType":"text/html;profile=mcp-app"}
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
generated/proto/rulec/member_shipping_fee/v4/member_shipping_fee.proto   the whole of the contract
generated/proto/buf.yaml                                                  the tree is a buf module
generated/proto/buf.gen.yaml                                              how the stubs are generated
generated/python/member_shipping_fee_service.py                           what stands behind it
generated/python/member_shipping_fee_connect_runner.py                    the same vectors, over the wire
```

The `.proto` is one file for every language, so it sits beside the language directories
rather than inside one, and **its path spells its package** — what buf's
`PACKAGE_DIRECTORY_MATCH` asks for — so it can be dropped into a buf module as it stands.
`buf lint` finds nothing in it.

```proto
package rulec.member_shipping_fee.v4;

message DecideRequest {
  optional Prefecture dest = 1;
  // weight: an integer, in g. 1 to 40000
  optional int64 weight = 2;
  // total: an integer, in JPY (tax included). 0 to 10000000
  optional int64 total = 3;
  optional MemberKind member = 4;
}

message DecideResponse {
  // fee: an integer, in JPY (tax included)
  int64 fee = 1;

  // The rows that matched, one per table, in order.
  repeated Fired trace = 100;
}

service MemberShippingFeeService {
  // Decides fee from dest, weight, total, member.
  // The rule is a pure function, so this method has no side effects and can be
  // called with GET.
  rpc Decide(DecideRequest) returns (DecideResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}
```

Five things there are decisions rather than transcription.

**The package carries the rule's major version.** The package names the *wire*, and what a
change to the wire does is exactly what `buf breaking` is there to say.

**The method declares that it has no side effects**, which is not a hint but something
already proved: the same inputs give the same answer, forever, for one version of the table.
Connect lets such a method be called with `GET`, which is what makes an answer cacheable.

**The answer carries the rows that decided it.** `trace` is the same list the record function
writes, so one call is one fixtures record. Its field number is far from the outputs so that
an output added to a table later takes the next small number and leaves the trace where it
was.

**Every field of the request is `optional`.** proto3 reads a field left out as its zero, so
without the mark a request that named nothing would be decided as one that said 0, `false`
and the first value of every enum. Marked, a field left out is told apart from one set to
zero, and a request that leaves out an input the rule needs is refused (below); a `T?` input
says in its comment that it may be left unset. A caller therefore sends every input, `false`
and `0` included — which a generated client does when the field is set, and which is what the
JSON has to say when it is written by hand. The answer keeps the plain form: it always has
every output, so an output at its zero is left out of the JSON as usual.

**Only the enums that cross the wire are declared**, and one whose values belong to a contract
outside the rule (`import proto`, [reference.md](reference.md#an-enum-a-proto-owns)) is
imported rather than declared again — the rule cites that file and `rulec check` holds the two
together, so the service speaks the contract's own type instead of a second one that means
the same thing. So that the module builds as it stands, `gen` puts the contract in it: the file
`check` read, byte for byte, at the path its package gives it (`proto/shop/v1/order.proto` for
`package shop.v1`), which is also the path the generated `.proto` imports. A file the contract
imports in turn comes with it when it is found where its import path says, and the well-known
types are buf's own.

An import found nowhere on disk is a file of a BSR module — protovalidate's
`buf/validate/validate.proto`, say — and the module gets it the way the contract's own
workspace does. `gen` reads the `buf.yaml` nearest above the contract and the `buf.lock` beside
it, declares in `proto/buf.yaml` the module the import comes from, and writes the lock's pins to
`proto/buf.lock`. It fetches nothing itself: buf fetches the module when it builds, and
`buf.gen.yaml` writes the stubs of its files with the rest (`include_imports`), since the
contract's stub imports them. With no `buf.yaml` beside the contract, or a lock still in v1's
shape — whose shake256 digests a v2 module does not read — there is nothing to pin with, so
`gen` declares the module, writes no lock, and says to run `buf dep update` in `proto/`. Two
rules may import one contract, but two different files that would land on one path, or two
locks that pin one module at two commits, stop `gen` with an error.

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
$ uvicorn member_shipping_fee_service:app --port 8080         # ASGI: uvicorn, hypercorn, daphne
$ gunicorn 'member_shipping_fee_service:wsgi_app'             # WSGI: gunicorn, uWSGI
$ python3 generated/python/member_shipping_fee_service.py --http 127.0.0.1:8080   # the standard library's server
http://127.0.0.1:8080
$ curl -sS -X POST -H 'Content-Type: application/json' \
    -d '{"dest":"PREFECTURE_KAGOSHIMA","weight":800,"total":4200,"member":"MEMBER_KIND_BASIC"}' \
    http://127.0.0.1:8080/rulec.member_shipping_fee.v4.MemberShippingFeeService/Decide
{"fee":"800","trace":[{"table":"base","row":3},{"table":"payer","row":3}]}
```

Three things about that answer are protobuf's JSON mapping rather than rulec's: the field
names are lowerCamelCase, an `int64` is a **string** because a JSON number cannot hold one,
and non-ASCII is escaped. A generated client hands you a Python `int` and the table's real
name either way; it is only the bytes on the wire that look like that.

The third line is the standard library's own server, there so that the service can be tried
without uvicorn or gunicorn; it serves the WSGI side, and like the other two it needs
`connectrpc` and the stubs. As with the MCP server, **TLS and authentication go in front**:
none of these carries either.

An input the rule cannot take is refused rather than answered, with the code that says whose
mistake it was:

| what happened | code | HTTP |
|---|---|---|
| outside the declared domain — out of range, not an integer, not a member of the enum | `invalid_argument` | 400 |
| an input the rule needs was left out — `member is missing`, `amount is missing (lines[2])` for a field of one element | `invalid_argument` | 400 |
| a field or an enum value the message does not have — a misspelt name | `invalid_argument` | 400 |
| the runtime guard of a W114 pair fired (§8.1) | `internal` | 500 |

```console
$ curl … -d '{"dest":"PREFECTURE_KAGOSHIMA","weight":0,…}'
{"code": "invalid_argument", "message": "weight is out of range: 0"}
```

The message names the argument by the rule's own name for it (`weight`), as the module's own
error does. The third row's message is the JSON reader's own and names the field or the value
it did not know: Connect's default is to drop it, and the service turns that off, because a
dropped input would then be decided as zero — and where a contract puts a real value at 0, a
misspelt enum value would be decided as that value. The last row is the one place with no
static proof, and it answers 500 on purpose: which of two rows wins is the table's to decide
and no caller can fix it, so it belongs where a service's own failures are counted.

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

### A service a contract declares

The numbers above are the order the rule declares its values in, so a value added in the middle
moves the numbers after it, and a caller built from the earlier `.proto` sends its values to the
wrong fields. A rule that names a contract's service with `implements proto "<file>" <Service>`
([reference §3.4](reference.md#34-implements--the-service-a-contract-declares)) is served as that
service instead, and `rulec check` holds the rule to it (E076):

```
generated/proto/shop/v1/member_shipping.proto              the contract, byte for byte
generated/proto/buf.yaml                                   the tree is a buf module
generated/proto/buf.gen.yaml                               how the stubs are generated
generated/python/member_tier_shipping_service.py           the same service, under the contract's names
generated/python/member_tier_shipping_connect_runner.py    the same vectors, over the wire
```

`gen` writes no `.proto` of the rule's own. It copies the contract with the files it imports, the
way it copies a contract an enum is imported from, and the stubs buf writes from it are the ones
the service and the runner import. What a caller agrees to is then the contract's — the path,
the messages and the numbers — and the rest is as above: the method free of side effects, the
rows in `trace`, `rulec-source-sha256` on every answer, a request that leaves out an input the
rule needs refused, and `rulec test` putting every vector through the service four times.

```console
$ python3 generated/python/member_tier_shipping_service.py --http 127.0.0.1:8080
http://127.0.0.1:8080
$ curl -sS -X POST -H 'Content-Type: application/json' \
    -d '{"member":"TIER_SILVER","campaign":"TIER_BASIC","subtotal":10}' \
    http://127.0.0.1:8080/shop.v1.MemberShippingService/Quote
{"shipping":"5","effective":"TIER_SILVER","trace":[{"table":"shipping_fee","row":3}]}
```

`rulec api` says the same names and numbers under `connect`, with the file under `contract`
([formats.md](formats.md)). A message the contract declares in another file, or nests in another
message, is read from the module protobuf-py writes for that file, under the class it nests it in.

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
$ rustc --edition 2021 -O --target wasm32-wasip1 member_shipping_fee_runner.rs -o member_shipping_fee_runner.wasm
$ wasmtime member_shipping_fee_runner.wasm < ../vectors/member_shipping_fee.jsonl
```

Those two lines are what `rulec api` carries, so nothing here has to be copied by hand. They
sit inside the Rust entry, because the shape is a property of a backend and Rust is the one
that has it:

```json
"rust": { "module": "member_shipping_fee.rs", "function": "member_shipping_fee", …,
          "wasi": {
            "source": "member_shipping_fee_runner.rs",
            "module": "member_shipping_fee_runner.wasm",
            "build":  "rustc --edition 2021 -O --target wasm32-wasip1 member_shipping_fee_runner.rs -o member_shipping_fee_runner.wasm",
            "run":    "wasmtime member_shipping_fee_runner.wasm",
            "wire":   "one vectors line on stdin, one fixtures record per line on stdout, or {\"refused\":…,\"error\":…} in the place of the record for a line it refuses",
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
