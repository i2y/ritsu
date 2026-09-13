# Generate and call

```console
$ rulec gen rules/ --out generated/
```

Out comes an ordinary Python module and an ordinary Go package. No
runtime to install, no configuration, and no dependency beyond the
standard library — that last one is a **checked** property, not a claim:
`rulec test` runs the Go side with `GOPROXY=off`.

A rule that does not pass `check` generates nothing.

## What the output looks like

Every row of every table becomes one branch, in order, with the original
cells quoted beside it.

```python
def fee_demo(dest: Prefecture, girth: Cm, weight: Gram) -> YenInclTax:
    """Rule 送料例 v1. Each branch corresponds 1:1 to a row of the rule source."""
    if not _isinstance(dest, Prefecture):
        raise RuleInputError(f"あて先 is not a value of enum Prefecture: {dest!r}")
    if not 1 <= girth <= 100:
        raise RuleInputError(f"三辺合計 is out of range: {girth}")
    # table サイズ判定 (policy first)
    if girth <= 60:  # row 1: <=60cm | S60
        サイズ = SizeClass.S60
    elif girth <= 80:  # row 2: <=80cm | S80
        サイズ = SizeClass.S80
    elif True:  # row 3: - | S100
        サイズ = SizeClass.S100
    else:
        raise AssertionError("unreachable: completeness was statically checked by rulec")
    ...
    return _round_up(運賃, 10)
```

```go
func FeeDemo(in Input) (YenInclTax, error) {
	if !in.Dest.Valid() {
		return 0, fmt.Errorf("あて先 is not a value of the enum: %d", in.Dest)
	}
	// table サイズ判定 (policy first)
	var サイズ SizeClass
	if int64(in.Girth) <= 60 { // row 1: <=60cm | S60
		サイズ = SizeClassS60
	} else if int64(in.Girth) <= 80 { // row 2: <=80cm | S80
		サイズ = SizeClassS80
	} else if true { // row 3: - | S100
		サイズ = SizeClassS100
	} else {
		panic("unreachable: completeness was statically checked by rulec")
	}
	...
	return YenInclTax(roundUp(int64(運賃), 10)), nil
}
```

Four rules keep it readable.

- **No cell is elided.** A condition an earlier branch already settled is
  still written (`elif True:`), because reading the generated code
  against the source side by side is the only way it is meant to be read.
- **Units live in the type.** `NewType` in Python, a defined type in Go.
  Confusing `YenInclTax` with `YenExclTax` stops at compile time.
- **Rounding goes through its own helper**, because Python's `//` rounds
  toward −∞ and Go's integer division toward zero.
- **No builtin is called bare.** An input aliased `min` or `list` does
  not break the output: `_min`, `_max` and `_isinstance` are generated.

That the reference evaluator and the two generated languages answer the
same is checked by streaming automatically generated boundary cases
through all three and comparing **canonical JSON byte for byte**.

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
