# rulec

**A harness for an agent turning table-shaped business rules into code.**

Write the table, and out come Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java and SQL functions, a Wasm module, and a NumPy plan. **The
proof is finished before the code exists.**

```rule
table 運賃表(fee_table)
policy unique
| あて先      | サイズ | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏      | S60    | 990円                              |
| not: 近畿圏 | S60    | 880円                              |
```

↓ `rulec gen`

```python
def fee_demo_traced(dest: Prefecture, girth: Cm, weight: Gram) -> tuple[YenInclTax, list[Fired]]:
    if not _isinstance(dest, Prefecture):
        raise RuleInputError("あて先 is not a value of enum Prefecture", dest)
    ...
    if dest in _kinki and size == SizeClass.S60:  # row 1: 近畿圏 | S60 | 990円
        fee = 990
        trace.append(Fired("運賃表", 1))
    ...
    return YenInclTax(_round_up(fee, 10)), trace
```

Seven things are settled before anything is generated. **Five are proved statically** —
every input matches some row, no input matches two rows, no row matches nothing, units are
never confused, and every intermediate fits in int64. **One is a declaration that has to be
there** — how fractions are settled, because which way is right is a business decision and
the tool does not make it. **One is run** — every worked example holds. If any of the seven
cannot be shown, nothing is generated.

None of that is type checking. A type says a value **has the right shape** — a member of the
enum, an integer, the unit it claims — and a right shape says nothing about a right answer.
What is proved here is a property of the table, shown exhaustively over the declared input
space rather than sampled.

What is **not** proved matters just as much.

1. **That the table matches reality.** What is proved is only what can be said about the
   table as written. Cite the document a table was transcribed from (`@source 表1`) and an
   amount that disagrees with the copy does fail (E116, W120) — but even then what is shown is
   agreement with the copy, not with the world. With no citation, transcribe the tariff wrong
   and everything stays green.
2. **That the generated code answers like the table.** That is a *test*, not a proof: test
   cases built from the boundaries are run through the reference evaluator and every
   generated language, and compared byte for byte. Strong evidence, not an equivalence
   proof.
3. **Row pairs the overlap proof could not reach.** When neither an input matching both
   rows nor its impossibility could be constructed, **W114 names the pair and moves the
   check into a runtime guard** — the one place with no static proof. It returns an error
   rather than silently picking a side.
4. **That the checker itself is right.** The proofs above come out of rulec's own
   implementation, which has not itself been proved correct. The evidence is 79 deliberately
   broken rules each producing the diagnostic it should, and 30 rules — 28 transcribed from real
   published terms checked, generated and run on every commit. Evidence, not proof.

**A model checker reads the Rust.** Beside the generated module, `gen` writes proof
harnesses for [Kani](https://model-checking.github.io/kani/), behind `#[cfg(kani)]` so
`rustc` never sees them. Over **every** input in the declared domain rather than over the
test cases: no table falls through, no runtime guard of (3) fires, nothing overflows an
`i64`, and the rows of each `unique` table cover the domain exactly once. It shares no code
with the checker that proved the table, so where the two agree, two unrelated tools say the
same thing — and where they disagree, one of them is wrong and the input that shows it comes
back. On the corpus, 73 harnesses verify in 155 seconds. What it does not reach: the table
itself (1), and every target but this one. `rulec test --proofs` runs it where `kani` is
installed.

No runtime and no configuration: what comes out is ordinary dependency-free functions.

> **Documentation site — [i2y.github.io/rulec](https://i2y.github.io/rulec/)**
> Everything below at length, in English and Japanese.
> 日本語のドキュメントは **[i2y.github.io/rulec/ja/](https://i2y.github.io/rulec/ja/)** にあります。
>
> **Try it first — [i2y.github.io/rulec/playground/](https://i2y.github.io/rulec/playground/)**
> The checker itself, compiled to wasm and running in the page: paste a table and the gap
> comes back with the input that falls through it. Nothing is sent anywhere, and nothing
> is installed.

---

## Where to start, by what you have

What you already have decides the first move. None of the three changes anything that
runs today, and the second is the one to start with when there is code already: nothing
is deployed, and what comes back is a match rate and the disagreements, clustered.

| you have | the first move | the command |
|---|---|---|
| **a spreadsheet or a published policy** | Transcribe it into a `.rule` and check it. From a workbook, a first draft is read straight out of the file, with every guess marked. No data and no old implementation are needed: a gap or a contradiction comes back with the input that causes it — and with the document cited (`@source 表1`), a mistyped amount comes back too | `rulec import xlsx`, then `rulec check` — [What it proves](https://i2y.github.io/rulec/checks/) |
| **an implementation that runs today** | Hand the existing function to the agent. It transcribes it into a `.rule` and wraps the old code in a 20-to-30-line adapter whose shape rulec prints; `verify` streams the cases built from the rule's own boundaries through both and returns where they disagree, clustered by the rows that matched, with counts and an example. The code that runs today is not touched | `rulec verify` — [Compare and replay](https://i2y.github.io/rulec/compare/) |
| **past records** | Validate the records, then replay the rule over them. For a change, how many records move and by how much comes out before it ships | `rulec fixtures lint`, then `rulec replay` / `rulec diff` — [Compare and replay](https://i2y.github.io/rulec/compare/) |

## Install

One binary, no runtime. Every release publishes a static binary for macOS (arm64, x64) and
Linux (x64, arm64), with the SHA-256 of each beside it:

```console
$ v=v0.10.0; t=aarch64-apple-darwin     # or x86_64-apple-darwin, x86_64-unknown-linux-musl, aarch64-unknown-linux-musl
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/$v/rulec-$v-$t.tar.gz"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/$v/SHA256SUMS" | grep "$t" | shasum -a 256 -c
$ tar -xzf "rulec-$v-$t.tar.gz" && install -m 755 rulec ~/.local/bin/
$ rulec --version
rulec 0.10.0
```

Or from source, with a recent stable Rust: `cargo install --path .` fetches nothing, because
there are no dependencies. In CI, `uses: i2y/rulec@v0.10.0` does the download and the check
([In CI](#in-ci)).

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

An enum is a **closed** set, which is what makes "somebody added a value" break every table
that has not seen it. When the set is not yours to decide — a member tier, a status, anything
declared in a `.proto` or a JSON Schema — `import proto "api/v1/order.proto" MemberTier ->
会員区分` (or `import jsonschema "api/openapi.json" "#/components/schemas/MemberTier" ->
会員区分`) binds the two, and every `rulec check` reads that file. Adding a value to an enum is a compatible change
on the wire, so the tools that guard the contract pass it; here it is **E032** until the rule
carries the value and **E033** until a row names it or `default` marks it. A table with a `-`
row would otherwise stay green while the new tier quietly took the default amount.

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
    """Rule 送料例 v1: the same decision as fee_demo_traced, without the rows that matched."""
    out, _ = fee_demo_traced(dest, girth, weight)
    return out


def fee_demo_traced(dest: Prefecture, girth: Cm, weight: Gram) -> tuple[YenInclTax, list[Fired]]:
    if not _isinstance(dest, Prefecture):
        raise RuleInputError("あて先 is not a value of enum Prefecture", dest)
    if not _isinstance(girth, int) or _isinstance(girth, bool):
        raise RuleInputError("三辺合計 is not an integer", girth)
    if not 1 <= girth <= 100:
        raise RuleInputError("三辺合計 is out of range", girth)
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
		return 0, nil, &RuleInputError{What: "あて先 is not a value of the enum", Value: int64(in.Dest), HasValue: true}
	}
	var trace []Fired
	// table サイズ判定 (policy first)
	var size SizeClass
	if int64(in.Girth) <= 60 { // row 1: <=60cm | S60
		size = SizeClassS60
		trace = append(trace, Fired{"サイズ判定", 1, ""})
	} else if int64(in.Girth) <= 80 { // row 2: <=80cm | S80
		size = SizeClassS80
		trace = append(trace, Fired{"サイズ判定", 2, ""})
	} else if true { // row 3: - | S100
		size = SizeClassS100
		trace = append(trace, Fired{"サイズ判定", 3, ""})
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
well as the values. A third function, `fee_demo_record`, writes one call as one line of the
fixtures format, so the records `replay` and `diff` need come out of the generated code
itself rather than out of an extraction job. A fourth file, `fee_demo_mcp.py` beside the
Python module and `fee_demo_mcp.mjs` beside the JavaScript one, serves the rule as one MCP
tool for an agent that calls it rather than the application that embeds it: the arguments are
the wire, the answer is that record line, rows included, and `rulec test` drives the server
over the same vectors as the runner — over **stdio and over Streamable HTTP**, because the
places that would call a rule (a chat client's connectors, an agent builder, a workflow
product) only accept a URL. A fifth file, `fee_demo_page.html`, is the page an approver
reads; where the host renders MCP Apps the server offers it as the tool's view, **opened on
the case that was just asked**, with the rows that decided it lit up.

In SQL the same rule is one query over a relation of inputs — a row of the table is a `WHEN`,
the rows that matched come back as columns — which is what a closing batch or an analyst's
recalculation needs: many rows in one statement. Beside it is the same query as a function,
for the other way round: one case, asked for by name. A function in a schema PostgREST or
Supabase exposes is an endpoint with no server code of its own, and unlike the query it
**raises** what the others raise, so a client that reads one field is not handed a number
that looks like an answer.

Four rules keep it readable. **No cell is dropped** — a condition an earlier branch already
settled is still written out (`elif True:`), because reading the output against the table is
the only way it is meant to be read. **Units ride in the type wherever the language has one
to ride in**: a newtype in Rust, a one-field struct in Swift, a defined type in Go, a branded
bigint in TypeScript, a `NewType` in Python that `mypy --strict` is run over, and the Rust newtype again in
the Wasm module, which is the Rust one. Ruby, PHP, JavaScript, Java and SQL have
none, so there the unit is declared in the signature, or in the header of the query, and stated in a comment, and the `.rbs`
that ships with the Ruby module says as much. The NumPy plan has no signature to carry one:
the unit and the scale are fields of the plan itself, and `rulec api` prints them. **Rounding goes through a helper of its own**, because integer division does
not agree between them — Python and Ruby floor toward −∞, Go and Rust truncate toward zero.
**Nothing builtin is called bare**, so an input aliased `min` or `list` cannot break the
output.

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
$ rulec doc rules/送料.rule --lang ja --format html  # the same, as a page they can try a case on
$ rulec explain E101                              # when it appears, how to fix it, a repro
$ rulec mcp                                       # the same commands as MCP tools, for an agent without a shell
$ rulec import xlsx 運賃表.xlsx --sheet 本則    # a first draft from the workbook itself; every guess is marked
$ rulec import csv tariff.csv > rules/tariff.rule  # the same from a CSV
```

Transcribe a tariff, drop one prefecture out of forty-seven, and the gap comes back with the
input that falls through it:

```
error[E101]: Completeness gap: some input matches no row
  --> rules/ゆうパック運賃.rule:34 table 運賃表
   |
34 | table 運賃表(fee_table)  # 出典: 日本郵便 基本運賃表（東京）
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
- uses: actions/checkout@v7                  # with fetch-depth: 0, so --diff-base can read origin/main
- uses: i2y/rulec@v0.10.0                     # the release binary, verified against its checksum
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/ --require-all   # the only step that needs the toolchains
```

Those logs are read by machines and developers, so they stay in the default English. What
goes to a person — `rulec diff` on a pull request, `rulec doc` for an approver — is built in
the same job with `RULEC_LANG` set to their language. The job that puts the diff on the pull
request is on the [install page](https://i2y.github.io/rulec/install/#in-ci): the old
version is `rules/送料.rule@origin/main`, the file as it is on the base branch, and `--terse`
keeps the values of the records out of the comment.

## What is in this repository

```
AGENTS.md         the procedure an agent follows
DESIGN.md         the design record: every decision, and what was discarded with it
docs/             reference.md (the grammar), formats.md (machine-readable output),
                  generated-code.md, backends.md (targeting another language),
                  codes.md / codes.ja.md (every diagnostic, generated),
website/          the documentation site (Zensical): docs/ English, docs-ja/ Japanese
skills/rulec/     an agent skill for using rulec — copy the folder into .claude/skills/;
                  `rulec mcp` serves the same commands as MCP tools where there is no shell
src/              38 modules: kw, i18n, lex, parse, types, defset (the tables that define
                  one output, as one set), region, eval, fmt, json,
                  codegen, backend, vectors, coverage, verify, fixtures, replay, report, doc,
                  import (a draft from a sheet), xlsx (reading the workbook: ZIP, deflate,
                  the number formats), proto and jsonschema (the enums whose values are
                  declared outside the rule), enums (holding the two together),
                  mcp (the command table as MCP tools),
                  codegen/tool (the rule as an MCP tool and its view), codegen/sql (one query,
                  and the same query as a function),
                  sources (a law on e-Gov, cited and pinned), apply (a rule applied to
                  another case), vfs (reading at a git revision), sha256,
                  wasm (the checker as the site's playground)
tests/corpus/     30 rules: 28 transcribed from real published terms, 2 written to use the
                  words a transcription never reaches; and the copies of the
                  statute text they cite
tests/mutants/    33 files, each with one mistake planted in it
tests/golden/     35 snapshots of diagnostic prose, in both languages
tests/oracle/     two premium tables transcribed grade by grade from their published PDFs,
                  which tests/library.rs replays the rules over
tests/            and the properties: threeway (every language agrees), readme, docs,
                  website, skill, codes, json_v2, formats, api, coverage, m3, budget, library,
                  mcp, import, xlsx, proto and jsonschema (an enum held to the file it is
                  declared in), tool (the rule as an MCP tool), sql, wasm (the site's
                  playground answers what the binary answers)
```

Those rules come from **public information** — Japan Post's tariff, Yamato's size classes,
the coupon terms of Rakuten and Yahoo, Article 7 of EU Regulation 261/2004, the National
Tax Agency's income-tax and stamp-duty tables, the Stamp Tax Act and the Special Taxation
Measures Act as e-Gov publishes them, and the premium tables of 協会けんぽ and 日本年金機構 —
or are sketches from the design record. None of it is private data. The two premium tables are also
held, grade by grade, to the amounts printed in them.

```console
$ cargo test          # 400 tests; python3, node, rustc, ruby, php, go, swiftc and a JDK are used where present
```

## Where it stands

30 rules — 28 taken from real published terms, 2 written to reach the rest of the language — are checked, generated and run on every commit, and all 74 diagnostics are implemented. What is built:

| | |
|---|---|
| **the checker** | completeness, overlap, unreachable rows, units, rounding, overflow, examples — each with the input that causes it; a main rule and its exceptions checked as one set, a statute's text pinned to its copy, a transcribed table held to the amounts that copy shows, a rule applied to another case held to the applied rule's ranges |
| **the generators** | Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy, with the agreement between the reference evaluator and every generated language checked byte for byte on canonical JSON, the rows that matched included. The test cases are built from the boundaries, and a separate judge checks that the set of them meets three coverage criteria |
| **`verify`** | stand the legacy implementation up as a process and see whether it answers the same |
| **`replay`** | validate past records, replay them, diff two versions, write the Markdown for a pull request |

### Output languages

**Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy** — twelve
targets, all shipped. NumPy is the one that is not a language: the rule travels as data and a
fixed evaluator decides whole columns at once.

Kotlin and Scala are not on the list because they do not need to be: Kotlin and Scala are not on the list because they do not need to be:
the generated Java is an ordinary class on the classpath, and the JVM does the rest.

You do not have to wait for the list, and nothing here has to change. A target outside it —
another language, a workflow engine's expression language, a spreadsheet formula — can be
generated from what `rulec api` and `rulec schema` already publish, and `rulec verify` will
hold the result to the rule over every case built from its own boundaries, exactly as the
shipped ones are held. [`docs/backends.md`](docs/backends.md) runs that loop end to end
against SQL, by hand, as it was done before SQL had a backend of its own.

| | | |
|---|---|---|
| Python | shipped | `python3` |
| TypeScript | shipped | `node` alone — the output is erasable syntax, so no build step and no tsconfig |
| JavaScript | shipped | `node` alone, or a browser — the TypeScript with its types taken off, as an ES module (`.mjs`) |
| Rust | shipped | `rustc` alone — no cargo, no crates |
| Ruby | shipped | `ruby` 3.x or 4.x — `json` is standard library, so no gem, and an `.rbs` ships alongside |
| PHP | shipped | `php` 8.2 or newer — `ext/json` is compiled in, so no composer; native enums and typed parameters, and every division is `intdiv` |
| Go | shipped | `go` |
| Swift | shipped | `swiftc` alone — no SwiftPM and no `Package.swift`; units ride in the type as they do in Rust |
| Java | shipped | a JDK — `javac` and `java`, no Maven and no Gradle. Compiled at `--release 17`, so it runs on 17, 21 and 25 alike; `long` is the int64 the overflow proof is about |
| SQL | shipped | `python3`, whose standard-library `sqlite3` is where the agreement check runs the query, and a `psql` that reaches a PostgreSQL for the function beside it. Written for PostgreSQL. Two doors on one query: `<alias>.sql` over a relation of inputs — a row of the table is a `WHEN`, the rows that matched are columns, a million rows in one statement — and `<alias>_function.sql`, the same query as a function for one case at a time, which PostgREST or Supabase turns into an RPC endpoint. The function raises where the query returns a column |
| Wasm | shipped | `rustc` with the `wasm32-unknown-unknown` target — one module that exports `call: func(input: string) -> string` in the canonical ABI, with a `.wit` that makes a component of it; `node` runs the agreement check |
| NumPy | shipped | `python3` and `numpy` — not generated code but the rule as data (`<alias>.json`) beside one fixed evaluator (`rulec_np.py`), which builds a closure over whole columns when the plan is loaded. A table is one `np.select`; `unique` needs no runtime uniqueness test because E105 settled it, and int64 is safe to compute in because E108 did. A rule that walks a sequence is refused by name |

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
