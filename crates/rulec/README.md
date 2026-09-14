# rulec

**A harness for an agent turning table-shaped business rules into code.**

Write the table, and out come Python, TypeScript, Rust, Ruby and Go functions. **The proof is
finished before the code exists.**

```rule
table 運賃表(fee_table)
policy unique
| あて先      | サイズ | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏      | S60    | 990円                              |
| not: 近畿圏 | S60    | 880円                              |
```

↓ `rulec gen`

```python
def fee_demo(dest: Prefecture, girth: Cm, weight: Gram) -> YenInclTax:
    """Rule 送料例 v1. Each branch corresponds 1:1 to a row of the rule source."""
    if not _isinstance(dest, Prefecture):
        raise RuleInputError(f"あて先 is not a value of enum Prefecture: {dest!r}")
    ...
    if dest in _kinki and size == SizeClass.S60:  # row 1: 近畿圏 | S60 | 990円
        fee = 990
    ...
    return _round_up(fee, 10)
```

Seven things are settled before anything is generated. **Five are proved statically** —
every input matches some row, no input matches two rows, no row matches nothing, units are
never confused, and every intermediate fits in int64. **One is a declaration that has to be
there** — how fractions are settled, because which way is right is a business decision and
the tool does not make it. **One is run** — every worked example holds. If any of the seven
cannot be shown, nothing is generated.

What is **not** proved matters just as much.

1. **That the table matches reality.** Transcribe the tariff wrong and everything stays
   green. What is proved is only what can be said about the table as written.
2. **That the generated code answers like the table.** That is a *test*, not a proof: test
   cases built from the boundaries are run through the reference evaluator and every
   generated language, and compared byte for byte. Strong evidence, not an equivalence
   proof.
3. **Row pairs the overlap proof could not reach.** When neither an input matching both
   rows nor its impossibility could be constructed, **W114 names the pair and moves the
   check into a runtime guard** — the one place with no static proof. It returns an error
   rather than silently picking a side.

No runtime and no configuration: what comes out is ordinary dependency-free functions.

> **Documentation site — [i2y.github.io/rulec](https://i2y.github.io/rulec/)**
> Everything below at length, in English and Japanese.
> 日本語のドキュメントは **[i2y.github.io/rulec/ja/](https://i2y.github.io/rulec/ja/)** にあります。

---

## Install

```console
$ cargo install --path .
$ rulec --help
```

## Write a table (.rule)

Nothing stops you writing one by hand. But the shape this is built for is an agent
transcribing from a published policy or a spreadsheet, and a person reading the table it
produced and approving it — so the thing worth taking from this section is how to *read*
one. Reading is not unaided either: `rulec doc` renders the table for an approver with the
facts the checker knows that the text does not show, and `rulec diff` says how many records
a change moves, and by how much, before it ships.

```rule
rule 送料例(fee_demo) v1
description "The README's example. Passes rulec check as written"

import std/都道府県

enum サイズ区分(size_class) = S60(s60) | S80(s80) | S100(s100)
group 近畿圏(kinki) = 滋賀県, 京都府, 大阪府, 兵庫県, 奈良県, 和歌山県

inputs
  あて先(dest)    : 都道府県
  三辺合計(girth) : length[cm]  range >=1cm <=100cm
  重量(weight)    : mass[g]   range >=1g <=25kg  contract_only

outputs
  運賃(fee) : money[円, incl_tax]  round up(10円)

table サイズ判定(size_of)
policy first
| 三辺合計 | -> サイズ(size) : サイズ区分 |
| <=60cm   | S60                          |
| <=80cm   | S80                          |
| -        | S100                         |

table 運賃表(fee_table)
policy unique
| あて先      | サイズ | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏      | S60    | 990円                              |
| 近畿圏      | S80    | 1310円                             |
| 近畿圏      | S100   | 1620円                             |
| not: 近畿圏 | S60    | 880円                              |
| not: 近畿圏 | S80    | 1200円                             |
| not: 近畿圏 | S100   | 1500円                             |

examples
| あて先 | 三辺合計 | 重量 | -> 運賃 |
| 大阪府 | 55cm     | 1kg  | 990円   |
| 東京都 | 90cm     | 3kg  | 1500円  |
```

**The keywords are English; the names and the cell values stay in the language of the
business.** This example passes `rulec check` as it stands — the repository's tests run it
on every commit. `examples` is an executable specification, and a row that does not hold is
reported with the rows that fired.

A cell tests **its own column and nothing else**, which is what makes a row a box and the
completeness and overlap checks exact. Complicated rules are written by **stacking tables**:
what one table produces is a column of the next. The whole language is in
[Write a table (.rule)](https://i2y.github.io/rulec/tour/) and, exhaustively, in
[`docs/reference.md`](docs/reference.md).

## The generated code

One row of the table becomes one branch, with the cells it came from beside it as a
comment. This is what the example above generates.

```python
def fee_demo(dest: Prefecture, girth: Cm, weight: Gram) -> YenInclTax:
    """Rule 送料例 v1. Each branch corresponds 1:1 to a row of the rule source."""
    if not _isinstance(dest, Prefecture):
        raise RuleInputError(f"あて先 is not a value of enum Prefecture: {dest!r}")
    if not 1 <= girth <= 100:
        raise RuleInputError(f"三辺合計 is out of range: {girth}")
    # table サイズ判定 (policy first)
    if girth <= 60:  # row 1: <=60cm | S60
        size = SizeClass.S60
    elif girth <= 80:  # row 2: <=80cm | S80
        size = SizeClass.S80
    elif True:  # row 3: - | S100
        size = SizeClass.S100
    else:
        raise AssertionError("unreachable: completeness was statically checked by rulec")
    ...
    return _round_up(fee, 10)
```

```go
func FeeDemo(in Input) (YenInclTax, error) {
	if !in.Dest.Valid() {
		return 0, fmt.Errorf("あて先 is not a value of the enum: %d", in.Dest)
	}
	// table サイズ判定 (policy first)
	var size SizeClass
	if int64(in.Girth) <= 60 { // row 1: <=60cm | S60
		size = SizeClassS60
	} else if int64(in.Girth) <= 80 { // row 2: <=80cm | S80
		size = SizeClassS80
	} else if true { // row 3: - | S100
		size = SizeClassS100
	} else {
		panic("unreachable: completeness was statically checked by rulec")
	}
	...
	return YenInclTax(roundUp(int64(fee), 10)), nil
}
```

Four rules keep it readable. **No cell is dropped** — a condition an earlier branch already
settled is still written out (`elif True:`), because reading the output against the table is
the only way it is meant to be read. **Units ride in the type**: `NewType` in Python, a
branded bigint in TypeScript, a defined type in Go, so confusing `YenInclTax` with
`YenExclTax` stops at compile time. **Rounding goes through a helper of its own**, because
Python's `//` truncates toward −∞ and Go's integer division toward zero. **Nothing builtin
is called bare**, so an input aliased `min` or `list` cannot break the output.

How to call it is a question `rulec api` answers without reading the code — module and
function name, arguments with their units and ranges, outputs with their rounding, the enum
spellings in that language, and the exceptions it can raise, as one JSON document that tests
hold to the real output. The shape and the guarantees are in
[`docs/generated-code.md`](docs/generated-code.md).

## Using it

```console
$ rulec check rules/*.rule
$ rulec check rules/送料.rule --format json      # ready for GitHub annotations
$ rulec check rules/送料.rule --terse             # heading, position, the input: three lines
$ rulec check rules/送料.rule --diff-base HEAD    # only what is newly introduced
$ rulec fmt --check rules/*.rule                  # the gofmt convention
$ rulec gen rules/*.rule --out generated [--check]
$ rulec vectors | coverage | test                 # the test cases, their coverage, the run
$ rulec adapter | schema | verify                 # against a legacy implementation
$ rulec fixtures lint | replay | diff             # against past records
$ rulec doc rules/送料.rule --lang ja             # for whoever approves the table
$ rulec explain E101                              # when it appears, how to fix it, a repro
```

Transcribe a tariff, drop one prefecture out of forty-seven, and the gap comes back with the
input that falls through it:

```
error[E101]: Completeness gap: some input matches no row
  --> rules/ゆうパック運賃.rule:34 table 運賃表
   |
34 | table 運賃表(fee_table)
   |       ^^^^^^ the input space is not fully covered
   |
 An input that matches no row: あて先 = 山梨県, サイズ = S60
 hint: add a row that matches this input.
 The shape of the row to add: `| 山梨県 | S60 | 820円 |`. Its output values are copied from the first row to give a shape that parses; they are not the right amounts. Decide whether the written rule, the spreadsheet or the legacy implementation is the source, and take them from there. One row closes the gap this witness names; if more is left, the next run names the next one.
```

**No legacy implementation and no past data are needed for that.** Every command carries
`--format json`, where a finding is data — `where`, `witness`, `rows`, `fix` — with the keys
fixed in English whatever language `--lang` puts the prose in. An unknown flag is refused
with exit 2 rather than ignored. The formats are defined in
[`docs/formats.md`](docs/formats.md). Every diagnostic is defined once, in `src/codes.rs`,
and [`docs/codes.md`](docs/codes.md) is literally the `rulec explain --all` output.

## In CI

```yaml
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/        # the only step that needs python3, node, rustc and go
```

Those logs are read by machines and developers, so they stay in the default English. What
goes to a person — `rulec diff` on a pull request, `rulec doc` for an approver — is built in
the same job with `RULEC_LANG` set to their language.

## What is in this repository

```
AGENTS.md         the procedure an agent follows
DESIGN.md         the design record: every decision, and what was discarded with it
docs/             reference.md (the grammar), formats.md (machine-readable output),
                  generated-code.md, backends.md (targeting another language),
                  codes.md / codes.ja.md (every diagnostic, generated),
website/          the documentation site (Zensical): docs/ English, docs-ja/ Japanese
skills/rulec/     an agent skill for using rulec — copy the folder into .claude/skills/
src/              25 modules: kw, i18n, lex, parse, types, region, eval, fmt, json,
                  codegen, vectors, coverage, verify, fixtures, replay, report, doc
tests/corpus/     13 rules transcribed from real published terms
tests/mutants/    19 files, each with one mistake planted in it
tests/golden/     21 snapshots of diagnostic prose, in both languages
tests/            and the properties: threeway (every language agrees), readme, docs,
                  website, skill, codes, json_v2, formats, api, coverage, m3, budget
```

Every one of those thirteen rules comes from **public information** — Japan Post's tariff,
Yamato's size classes, the coupon terms of Rakuten and Yahoo, and Article 7 of EU Regulation
261/2004. None of it is private data.

```console
$ cargo test          # 200 tests; python3, node, rustc and go are used where present
```

## Where it stands

Thirteen rules taken from real published terms are checked, generated and run on every commit, and all 35 diagnostics are implemented. What is built:

| | |
|---|---|
| **the checker** | completeness, overlap, unreachable rows, units, rounding, overflow, examples — each with the input that causes it |
| **the generators** | Python, TypeScript, Rust, Ruby and Go, with the agreement between the reference evaluator and every generated language checked byte for byte on canonical JSON. The test cases are built from the boundaries, and a separate judge checks that the set of them meets three coverage criteria |
| **`verify`** | stand the legacy implementation up as a process and see whether it answers the same |
| **`replay`** | validate past records, replay them, diff two versions, write the Markdown for a pull request |

### Output languages

**Python, TypeScript, Rust, Ruby and Go** today; **Java, Kotlin, Swift and SQL** are planned.

You do not have to wait for the list, and nothing here has to change. A target outside it —
another language, a workflow engine's expression language, SQL — can be generated from what
`rulec api` and `rulec schema` already publish, and `rulec verify` will hold the result to the
rule over every case built from its own boundaries, exactly as the four above are held.
[`docs/backends.md`](docs/backends.md) runs that loop end to end against SQL.

| | | |
|---|---|---|
| Python | shipped | `python3` |
| TypeScript | shipped | `node` alone — the output is erasable syntax, so no build step and no tsconfig |
| Rust | shipped | `rustc` alone — no cargo, no crates |
| Go | shipped | `go` |
| Java / Kotlin / Swift | planned | a JDK / kotlinc / swiftc |
| SQL | planned, shape undecided | a row becomes a `CASE` arm rather than a branch, and there is no stdin/stdout runner, so the dialect and the shape come first |

**A language that cannot join the agreement check does not get added**: output that cannot
be compared byte for byte against the reference evaluator sits outside the word "proved".
The third one, TypeScript, cost about 700 lines in the generator, and the differences
between languages concentrated in four places — type names, zero values, brackets around
branches, and the runner (DESIGN §15.13).

## Where to read next

| | |
|---|---|
| **[The documentation site](https://i2y.github.io/rulec/)** | all of this at length, in English and [日本語](https://i2y.github.io/rulec/ja/) |
| [`AGENTS.md`](AGENTS.md) | the procedure for an agent: write → check → fix → generate → integrate → show the impact → ask a person |
| [`docs/reference.md`](docs/reference.md) | the complete grammar |
| [`docs/codes.md`](docs/codes.md) / [`docs/codes.ja.md`](docs/codes.ja.md) | every diagnostic code, as `rulec explain --all` prints it |
| [`docs/formats.md`](docs/formats.md) | every machine-readable format: `--format json`, vectors, fixtures, manifests, the adapter protocol |
| [`docs/generated-code.md`](docs/generated-code.md) | the shape of the output, its guarantees, and how to call it |
| [`docs/backends.md`](docs/backends.md) | targeting a language rulec does not generate, without losing the comparison |
| `DESIGN.md` | why each decision was made and what was discarded with it (Japanese, 1,483 lines) |

The five references under `docs/` are carried into the site verbatim: holding the same text
twice is how one of the copies goes stale.

## About the design

The decision-table semantics and the hit-policy vocabulary are borrowed from DMN, and the
detection of overlap and gaps follows the formulation of Calvanese et al. Not borrowed: the
XML interchange format, the runtime engine, the GUI modeller. What DMN does not cover —
units, rounding, code generation for several languages, replay against past records — is
where this tool differs.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
