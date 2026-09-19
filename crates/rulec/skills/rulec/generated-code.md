# The generated code

`rulec gen` writes ordinary Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift, SQL and Wasm — a module in each,
a package in Go's case, and one query in SQL's. There is no runtime to install and nothing to
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
Swift imports nothing at all and needs no package manifest; the generated Go imports `fmt`; the
generated Wasm module imports nothing, not even WASI.
The `go.mod` lists nothing but the module itself. `rulec test` runs the Go side with `GOPROXY=off`, so "no dependencies"
is a checked property rather than a claim. The server that offers the rule as an MCP tool
([below](#the-rule-as-an-mcp-tool)) imports the standard library alone in Python and node's
own modules alone in JavaScript, and the generated SQL defines no function: its rounding is
arithmetic inside the query.

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
`NewType`, and the Wasm module is the Rust one; Ruby, JavaScript and SQL have nowhere to put a unit, so they document it instead.
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

**There is no entry guard on an enum input**, unlike Python, TypeScript, JavaScript, Ruby, Go and SQL. A value
of a Rust enum type is one of its variants by construction, so the check the others have to
make at run time is already made by the compiler. Swift is in the same position, and so is
the Wasm module, which is the Rust one behind a door that turns an unknown value into an
`error` line before the module is reached.

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

### SQL

```sql
CREATE VIEW shipping_fee AS
WITH "_c0" AS (SELECT "_id", "dest", CAST("weight" AS BIGINT) AS "weight", … FROM "shipping_fee_input"),
…
SELECT "_id", "dest", "weight", "total", "member", … AS "fee", "base_fee_row", "payer_row" FROM "_c5" ORDER BY "_id";
```

SQL gets no function. `shipping_fee.sql` is **one query over a relation of inputs**: provide
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

A query cannot stop, so what the other languages raise, this one returns as a column. The
entry guard is `_input_error`: NULL for a row inside the declared domain, and otherwise the
same sentence the others raise — a missing input, a value outside its range, a name that is
not a member of the enum, a number that is not an integer (18.3 in a column of steps is
refused, not truncated to 18). Where two rows of a `unique` table could not be proved
exclusive (W114), a `_contradiction` column names them when both match. The runner stops on
either, as the other languages raise.

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
an array of objects. Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and Wasm are
generated; SQL is refused by name, because one query has nowhere to carry a value from row to row.

## The rows that matched

Beside every function there is a twin with `_traced` on its name (`Traced` in Go and Swift).
It takes the same inputs and returns, beside the outputs, the rows that matched: one per
table, in order, each as the table's name and its 1-based row number. The plain function
calls it and drops the trace, so the branches exist once, in the traced one.

| | the twin | the row |
|---|---|---|
| Python | `def coupon_step_traced(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> tuple[Output, list[Fired]]:` | `Fired`, a `NamedTuple` of `table` and `row` |
| TypeScript | `coupon_step_traced(…): [Output, Fired[]]` | `{ table: string; row: number }` |
| JavaScript | `coupon_step_traced(…)`, returning `[out, trace]` | `{ table, row }` |
| Rust | `coupon_step_traced(…) -> Result<(Output, Vec<Fired>), RuleError>` | `Fired { table: &'static str, row: u32 }` |
| Ruby | `CouponStep.coupon_step_traced(…)`, returning `[output, trace]` | `Fired`, a `Struct` of `table` and `row` |
| Go | `func CouponStepTraced(in Input) (Output, []Fired, error)` | `Fired{Table, Row}` |
| Swift | `couponStepTraced(…) throws -> (Output, [Fired])` | `Fired(table:row:)` |
| SQL | none: the answer is the row | one column per table, `decide_row`, holding the row number |
| Wasm | none: the answer of `call` is the record line, `trace` beside `observed` | `{"table":…,"row":…}` objects in that line |

The row numbers are the ones `rulec doc` prints in its `#` column and the ones a `verify` or
`replay` report clusters by, so a trace taken from a log reads against the approved document
directly. `rulec test` compares these rows as well as the values: a generated function that
produced the right amount from the wrong row fails there. `rulec api` names the twin under
`traced` and gives its signature under `traced_signature`.

## A record of one call

Every module also has a function with `_record` on its name (`Record` in Go and Swift). It
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
| Go | `func CouponStepRecord(in Input, out Output, trace []Fired, tag string) string` |
| Swift | `couponStepRecord(…, out: Output, trace: [Fired], tag: String = "") -> String` |
| SQL | none: the answer is the row, and the runner writes the record from it |
| Wasm | none: the record line is what `call` returns |

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
    raise RuleInputError(f"商品合計 is out of range: {subtotal}")
```

A number that is not an integer is refused before the range is looked at, in every language
where a caller can pass one. A float sits inside any range: 18.3 for a rate declared in steps
of 0.1% would pass the check above, be taken as 1.83%, and be answered without a word. The
three statically typed targets need no such line; their types are the line. In SQL the guard
is a column, `_input_error`, because a query cannot stop: NULL inside the domain, the
sentence outside it ([below](#sql)).

```python
if not _isinstance(rate, int) or _isinstance(rate, bool):
    raise RuleInputError(f"料率 is not an integer: {rate!r}")
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

## The Rust runner as a WASI module

Apart from the `wasm/` target above, the Rust runner itself compiles unchanged for
`wasm32-wasip1`: it reads stdin and writes stdout through the standard library, which is what
a WASI command does, and it is the shape a Shopify Function has. When `wasmtime` is on the
PATH and that target's standard library is installed (`rustup target add wasm32-wasip1`),
`rulec test` runs the runner that way too and holds its answers to the same expected records
(`via` is `wasm`, the line reads `(Rust, Wasm)`). Without either, that pass is skipped with a
note and not counted as a missing language.

```console
$ rustc --edition 2021 -O --target wasm32-wasip1 shipping_fee_runner.rs -o shipping_fee_runner.wasm
$ wasmtime shipping_fee_runner.wasm < ../vectors/shipping_fee.jsonl
```

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
