# The generated code

`rulec gen` writes ordinary Python, TypeScript, Rust, Ruby, Go and Swift — a module in each,
and a package in Go's case. There is no runtime to install and nothing to configure: a function
takes the declared inputs and returns the declared outputs. This file says what shape that
code has, what it guarantees, and how to call it.

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
TypeScript imports nothing at all; the generated Rust imports nothing outside `std` and needs
no `Cargo.toml`; the generated Ruby requires nothing at all and needs no gem; the generated
Swift imports nothing at all and needs no package manifest; the generated Go imports `fmt`.
The `go.mod` lists nothing but the module itself. `rulec test` runs the Go side with `GOPROXY=off`, so "no dependencies"
is a checked property rather than a claim.

**Deterministic.** The same `.rule` and the same rulec version produce the same bytes. The
formatter is built in — no `gofmt` or `black` runs afterwards, because that would make the
output depend on the version of a tool installed on the machine. `gofmt -l` being empty and
`ruff check --select E,W` being silent (line length aside) are both tested.

**Branches match the rule line for line.** Every row of every table becomes one branch, in
order, with the original cells quoted in a comment (`# row 3: 近畿圏 | S100 | 1620円`). A
condition that an earlier branch already settled is still written out (`elif True:`), because
reading the generated code against the rule side by side is the only way it is meant to be
read. The branches are written once, in the twin that also returns
[the rows that matched](#the-rows-that-matched); the function you call delegates to it.

**Units live in the type** wherever the language has one to hold them. Rust uses a newtype,
Swift a one-field struct, Go a defined type, TypeScript a branded `bigint`, Python a
`NewType`; Ruby is the one target with nowhere to put a unit, so it documents it instead.
`YenInclTax` and `YenExclTax` are different types, and mixing them fails to compile in Rust,
Swift, Go and TypeScript, and fails type checking in Python. Every value is an integer in its declared unit; no floating point appears
anywhere.

**Rounding is explicit and settled for negatives too.** Python's `//` rounds toward −∞ and
Go's integer division toward zero, so neither language's division is used. Each side carries
its own `_round_up` / `_round_down` / `_round_half` / `_round_bankers` (`roundUp`, … in Go),
and a unit-vector file next to the generated code checks them against the reference on every
run.

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

**There is no entry guard on an enum input**, unlike Python, TypeScript, Ruby and Go. A value
of a Rust enum type is one of its variants by construction, so the check the others have to
make at run time is already made by the compiler. Swift is in the same position.

Errors come back as `Err(RuleError)`, whose two variants carry the same distinction as
Python's two exception classes: `RuleError::Input` is a contract violation by the caller, and
`RuleError::Contradiction` is the runtime guard described below.

It compiles with `rustc` alone — `rustc --edition 2021 -O coupon_step_runner.rs` builds both
the rule and its runner through a `#[path] mod`, with no project file and nothing to fetch.

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

---

## The rows that matched

Beside every function there is a twin with `_traced` on its name (`Traced` in Go and Swift).
It takes the same inputs and returns, beside the outputs, the rows that matched: one per
table, in order, each as the table's name and its 1-based row number. The plain function
calls it and drops the trace, so the branches exist once, in the traced one.

| | the twin | the row |
|---|---|---|
| Python | `def coupon_step_traced(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> tuple[Output, list[Fired]]:` | `Fired`, a `NamedTuple` of `table` and `row` |
| TypeScript | `coupon_step_traced(…): [Output, Fired[]]` | `{ table: string; row: number }` |
| Rust | `coupon_step_traced(…) -> Result<(Output, Vec<Fired>), RuleError>` | `Fired { table: &'static str, row: u32 }` |
| Ruby | `CouponStep.coupon_step_traced(…)`, returning `[output, trace]` | `Fired`, a `Struct` of `table` and `row` |
| Go | `func CouponStepTraced(in Input) (Output, []Fired, error)` | `Fired{Table, Row}` |
| Swift | `couponStepTraced(…) throws -> (Output, [Fired])` | `Fired(table:row:)` |

The row numbers are the ones `rulec doc` prints in its `#` column and the ones a `verify` or
`replay` report clusters by, so a trace taken from a log reads against the approved document
directly. `rulec test` compares these rows as well as the values: a generated function that
produced the right amount from the wrong row fails there. `rulec api` names the twin under
`traced` and gives its signature under `traced_signature`.

## The two guards

**The entry guard** enforces at run time what the proof assumed. Every numeric input is
checked against its declared `range`, and every enum input against its set of values. If the
check were absent, a caller outside the declared domain would get a silently wrong number
instead of an error — and the completeness proof says nothing about inputs the rule never
declared.

```python
if not 0 <= subtotal <= 1000000:
    raise RuleInputError(f"商品合計 is out of range: {subtotal}")
```

`rulec api` states the same bounds, taken from the same place, so an integration built from
the inventory cannot send values the guard rejects.

**The contradiction guard** is the other half of W114. When two rows of a `policy unique`
table might overlap and the checker could neither construct an input that proves it nor prove
that none exists, it does not pretend either way: it warns, and the generated code carries a
guard that stops rather than silently picking the earlier row.

```python
# guard: W114 (table 適用判定, row 1 × row 2): a pair of rows whose exclusivity could not be proven statically
if 残高A <= 1000 and 残高B >= 3980:
    raise RuleContradictionError("table 適用判定: row 1 and row 2 matched at the same time")
```

If this ever fires in production, it is evidence — the overlap the checker could not decide is
real, and the rule needs fixing.

---

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
