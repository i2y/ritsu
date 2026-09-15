# Generate and call

```console
$ rulec gen rules/ --out generated/
```

Out comes an ordinary module in Python, TypeScript, JavaScript, Rust, Ruby and Swift,
and an ordinary Go package. No runtime to install, no configuration, and no
dependency beyond the standard library — that last one is a **checked**
property, not a claim: `rulec test` runs the Go side with `GOPROXY=off`.

A rule that does not pass `check` generates nothing.

## Output languages

Seven are supported today — Python, TypeScript, JavaScript, Rust, Ruby, Go and Swift — and **Java,
Kotlin and SQL are planned**. The point is that one table should be able to give the
front end, the back end, the mobile app and the database the same answer,
and that this is *provable* through the agreement check that already
exists.

| | status | needs |
|---|---|---|
| Python | supported | `python3` |
| TypeScript | supported | just `node` — no build step, no tsconfig |
| JavaScript | supported | just `node`, or a browser — the TypeScript with its types taken off, as an ES module |
| Rust | supported | just `rustc` — no cargo, no crates |
| Ruby | supported | `ruby` 3.x or 4.x — `json` is standard library, so no gem, and a `.rbs` ships beside the module |
| Go | supported | `go` |
| Swift | supported | just `swiftc` — no SwiftPM, no `Package.swift`; units ride in the type as they do in Rust |
| Java | planned | a JDK; single-file execution means the runner needs no build tool |
| Kotlin | planned | kotlinc |
| SQL | planned, shape undecided | a row becomes a `CASE` arm rather than a branch, so the dialect and the shape get settled first |

One rule governs all of them: **a language that cannot join the
byte-for-byte agreement check does not go in.** Generated code that
cannot be held against the reference evaluator sits outside the claim
this tool makes. Adding the third, TypeScript, cost about 700 lines in
the generator, and the fifth, Ruby, cost the same. So did the sixth, Swift —
but Ruby also took twenty-odd files edited by hand, where Swift took one row
in the registry the tool now keeps of its own targets, and nothing else.

### A target that is not on the list

You are not limited to the list, and waiting for a backend is not the only
way in. A rule already publishes everything a generator needs — `rulec api`
gives the names, units, ranges and rounding, `rulec schema` gives the wire
— so you can emit whatever your target needs from your own tool.

**The comparison comes with you.** Wrap the result in the adapter protocol
and `rulec verify` will run it against every case built from the rule's own
boundaries, exactly as it does for the supported ones. Nothing in rulec changes.

[Other targets](backends.md) runs that loop end to end against SQL, which
is not one of them: 88 cases, all agreeing — and then one threshold
broken on purpose, to show the report naming the rows and the case that
proves it.

## What the output looks like

Every row of every table becomes one branch, in order, with the original
cells quoted beside it.

```python
def fee_demo(dest: Prefecture, girth: Cm, weight: Gram) -> YenInclTax:
    """Rule 送料例 v1: the same decision as fee_demo_traced, without the rows that matched."""
    out, _ = fee_demo_traced(dest, girth, weight)
    return out


def fee_demo_traced(dest: Prefecture, girth: Cm, weight: Gram) -> tuple[YenInclTax, list[Fired]]:
    if not _isinstance(dest, Prefecture):
        raise RuleInputError(f"あて先 is not a value of enum Prefecture: {dest!r}")
    if not 1 <= girth <= 100:
        raise RuleInputError(f"三辺合計 is out of range: {girth}")
    trace: _Trace = []
    # table サイズ判定 (policy first)
    if girth <= 60:  # row 1: <=60cm | S60
        size = SizeClass.S60
        trace.append(Fired("サイズ判定", 1))
    elif girth <= 80:  # row 2: <=80cm | S80
        size = SizeClass.S80
        trace.append(Fired("サイズ判定", 2))
    elif True:  # row 3: - | S100
        size = SizeClass.S100
        trace.append(Fired("サイズ判定", 3))
    else:
        raise AssertionError("unreachable: completeness was statically checked by rulec")
    ...
    return YenInclTax(_round_up(fee, 10)), trace
```

```go
func FeeDemo(in Input) (YenInclTax, error) {
	out, _, err := FeeDemoTraced(in)
	return out, err
}

func FeeDemoTraced(in Input) (YenInclTax, []Fired, error) {
	if !in.Dest.Valid() {
		return 0, nil, fmt.Errorf("あて先 is not a value of the enum: %d", in.Dest)
	}
	var trace []Fired
	// table サイズ判定 (policy first)
	var size SizeClass
	if int64(in.Girth) <= 60 { // row 1: <=60cm | S60
		size = SizeClassS60
		trace = append(trace, Fired{"サイズ判定", 1})
	} else if int64(in.Girth) <= 80 { // row 2: <=80cm | S80
		size = SizeClassS80
		trace = append(trace, Fired{"サイズ判定", 2})
	} else if true { // row 3: - | S100
		size = SizeClassS100
		trace = append(trace, Fired{"サイズ判定", 3})
	} else {
		panic("unreachable: completeness was statically checked by rulec")
	}
	...
	return YenInclTax(roundUp(int64(fee), 10)), trace, nil
}
```

The function you call is `fee_demo`, and its signature does not change. The branches live in
`fee_demo_traced`, which returns the rows that matched beside the value — one per table, in
order, as the table's name and its row number — which is what a log line or an answer to
"why this fee" needs. The agreement check holds those rows to the reference evaluator as
well as the values.

Four rules keep it readable.

- **No cell is elided.** A condition an earlier branch already settled is
  still written (`elif True:`), because reading the generated code
  against the source side by side is the only way it is meant to be read.
- **Units live in the type** wherever there is a type to hold them: a
  newtype in Rust, a one-field struct in Swift, a defined type in Go, a
  branded `bigint` in TypeScript, a `NewType` in Python. Confusing
  `YenInclTax` with `YenExclTax` stops at compile time. Ruby and JavaScript have
  nowhere to put a unit, so there it is documented instead.
- **Rounding goes through its own helper**, because Python's and Ruby's
  integer division rounds toward −∞ while Rust, Swift, Go and TypeScript
  truncate toward zero.
- **No builtin is called bare.** An input aliased `min` or `list` does
  not break the output: `_min`, `_max` and `_isinstance` are generated.

That the reference evaluator and every generated language answer the same
is checked by streaming automatically generated boundary cases through all
of them and comparing **canonical JSON byte for byte**.

## How to call it, without reading it

```console
$ rulec api rules/クーポン一枚.rule | jq -r .python.signature
def coupon_step(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> Output:
```

One JSON object: module and function names, the parameters in order with
their brands, units and ranges, the outputs with their rounding, the
enum members **under the spelling each language gives them**
(`CouponKind.PERCENT` in Python, `couponstep.CouponKindPercent` in Go),
and the errors that can come out.

A calling convention written by hand goes quietly wrong the day a name
changes, so this one is built next to the emitters and held to the
generated files by tests: every name it states must occur in the file;
Python imports the module and compares `inspect.signature`; and Go
**builds a calling program out of the inventory alone** and runs
`go vet` over it, which does not compile if a single name is wrong.

[The generated code in detail](generated-code.md){ .md-button }

## Two guards

**The entry guard** enforces at run time what the proof assumed. Every
numeric input is checked against its declared range and every enum input
against its values. Without it, a caller outside the declared domain
would get a silently wrong number — and the completeness proof says
nothing about inputs that were never declared.

**The contradiction guard** is the other half of W114. Where the checker
could not decide whether two rows of a `unique` table can overlap, the
generated code stops rather than silently picking one:

```python
# guard: W114 (table 適用判定, row 1 × row 2): a pair of rows whose exclusivity could not be proven statically
if 残高A <= 1000 and 残高B >= 3980:
    raise RuleContradictionError("table 適用判定: row 1 and row 2 matched at the same time")
```

## Running the generated code

```console
$ rulec test generated/
ok    shipping_fee (Python) 68 vectors
ok    shipping_fee (Go) 68 vectors
ok    rounding helper (Python) unit vectors
ok    rounding helper (Go) unit vectors

All 4 matched.
```

The vectors are built from the boundaries of the rule, not from the
generated code, and the rounding helpers get their own unit vectors —
table-level agreement alone would hide a helper bug in a table that
never produces fractions.

## Is the vector suite itself complete?

```console
$ rulec coverage rules/送料.rule
68 vectors
  row coverage              7 / 7     satisfied
  boundary-pair coverage    4 / 4     satisfied
  shadow-pair coverage      3 / 3     satisfied
```

`coverage` is **a completeness check on the test suite**. The three
obligations are derived from the rule rather than from the generated
vectors, and anything missing is named — which row, which boundary,
which shadow pair — with exit 1.

## Keeping it in step

The generated files are committed, and CI re-derives them:

```console
$ rulec gen rules/ --out generated/ --check
```

`--check` writes nothing and exits 1 if a file differs from a fresh
generation or is missing. **Never edit generated code by hand**: the
header says `DO NOT EDIT`, and the next `gen` overwrites it.

Formatting is built into the generator — no `gofmt` or `black` runs
afterwards, because the moment the output depends on the version of a
tool installed on the machine, generation stops being deterministic.
That `gofmt -l` is empty and `ruff check --select E,W` is silent (line
length aside) is tested instead.

---

[Compare and replay](compare.md){ .md-button .md-button--primary }
[Formats](formats.md){ .md-button }
