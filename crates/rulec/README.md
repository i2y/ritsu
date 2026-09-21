# rulec

**A harness for an agent turning table-shaped business rules into code.**

Write the table, and out come Python, NumPy, TypeScript, JavaScript, Rust, Ruby, PHP, Go,
Swift, Java, SQL and Wasm — ordinary functions with no runtime, no configuration and no
dependencies. **The proof is finished before the code exists**: a rule that cannot be proved
does not generate.

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
business.** This example passes `rulec check` as it stands — the repository's tests run it on
every commit. `examples` is an executable specification, and a row that does not hold is
reported with the rows that fired. A cell tests **its own column and nothing else**, which is
what makes a row a box and the completeness and overlap checks exact; complicated rules are
written by **stacking tables**, as above, where what one table produces is a column of the
next.

Out of `rulec gen`, in Python:

```python
def fee_demo(dest: Prefecture, girth: Cm, weight: Gram) -> YenInclTax:
    """Rule 送料例 v1: the same decision as fee_demo_traced, without the rows that matched."""
    out, _ = fee_demo_traced(dest, girth, weight)
    return out


def fee_demo_traced(dest: Prefecture, girth: Cm, weight: Gram) -> tuple[YenInclTax, list[Fired]]:
    if not _isinstance(dest, Prefecture):
        raise RuleInputError("あて先 is not a value of enum Prefecture", dest)
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
    ...
    # table 運賃表 (policy unique)
    if dest in _kinki and size == SizeClass.S60:  # row 1: 近畿圏 | S60 | 990円
        fee = 990
        trace.append(Fired("運賃表", 1))
    ...
    else:
        raise AssertionError("unreachable: completeness was statically checked by rulec")
    return YenInclTax(_round_up(fee, 10)), trace
```

One row of the table becomes one branch, with the cells it came from beside it as a comment,
and **no cell is dropped** — a condition an earlier branch already settled is still written
out (`elif True:`), because reading the output against the table is the only way it is meant
to be read. The function you call is `fee_demo`, and its signature does not change; beside it
`fee_demo_traced` returns **the rows that matched** — one per table, as the table's name and
its row number — which is what a log line or an answer to "why this fee" needs. Beside those
come a record writer for the fixtures format, the rule as one MCP tool over stdio and
Streamable HTTP, and a page an approver can try a case on.

> **Documentation site — [i2y.github.io/rulec](https://i2y.github.io/rulec/)**
> All of this at length, in English and Japanese: the whole language, what is proved and how,
> generating and calling it, comparing against what runs today, worked examples, and the
> agent's own procedure.
> 日本語のドキュメントは **[i2y.github.io/rulec/ja/](https://i2y.github.io/rulec/ja/)** にあります。
>
> **Try it first — [i2y.github.io/rulec/playground/](https://i2y.github.io/rulec/playground/)**
> The checker itself, compiled to wasm and running in the page: paste a table and the gap
> comes back with the input that falls through it. Nothing is sent anywhere, and nothing is
> installed.

## What is proved, and what is not

Seven things are settled before anything is generated. **Five are proved statically** — every
input matches some row, no input matches two rows, no row matches nothing, units are never
confused, and every intermediate fits in int64. **One is a declaration that has to be there** —
how fractions are settled, because which way is right is a business decision and the tool does
not make it. **One is run** — every worked example holds. If any of the seven cannot be shown,
nothing is generated.

None of that is type checking. A type says a value **has the right shape** — a member of the
enum, an integer, the unit it claims — and a right shape says nothing about a right answer.
What is proved here is a property of the table, shown exhaustively over the declared input
space rather than sampled.

What is **not** proved matters just as much.

1. **That the table matches reality.** What is proved is only what can be said about the table
   as written. Cite the document a table was transcribed from (`@source 表1`) and an amount
   that disagrees with the copy does fail (E116, W120) — but even then what is shown is
   agreement with the copy, not with the world. With no citation, transcribe the tariff wrong
   and everything stays green.
2. **That the generated code answers like the table.** That is a *test*, not a proof: test
   cases built from the boundaries are run through the reference evaluator and every generated
   language, and compared byte for byte. Strong evidence, not an equivalence proof.
3. **Row pairs the overlap proof could not reach.** When neither an input matching both rows
   nor its impossibility could be constructed, **W114 names the pair and moves the check into a
   runtime guard** — the one place with no static proof. It returns an error rather than
   silently picking a side.
4. **That the checker itself is right.** The proofs above come out of rulec's own
   implementation, which has not itself been proved correct.

Three things are built against (4), and none of them shares code with the checker. `gen`
writes proof harnesses for [Kani](https://model-checking.github.io/kani/) beside the Rust,
behind `#[cfg(kani)]`, which decide over **every** input in the declared domain rather than
over the test cases; on the corpus, 89 of them verify in 160 seconds. `rulec certificate`
prints what all five proofs rest on — the boxes that tile the input space, the axis each pair
of rows parts on, the interval every computed value is forced into, and where each cell stands
in your file, down to the byte — and `tools/recheck.py`, one dependency-free file, holds it to
those claims in milliseconds without asking the reader to search. And `proofs/` is a Lean 4
development proving that the checks a certificate has to pass imply the claims; writing it,
and reading it back adversarially, found a soundness bug in the completeness check, a witness
that could break the rule's own `constraint`, and eleven ways a forged certificate got past a
re-checker.

**What each layer reaches, and where it stops, is laid out in
[How it is checked](https://i2y.github.io/rulec/assurance/).**

## Install

One binary, no runtime. Every release publishes a static binary for macOS (arm64, x64) and
Linux (x64, arm64), with the SHA-256 of each beside it:

```console
$ v=v0.13.0; t=aarch64-apple-darwin     # or x86_64-apple-darwin, x86_64-unknown-linux-musl, aarch64-unknown-linux-musl
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/$v/rulec-$v-$t.tar.gz"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/$v/SHA256SUMS" | grep "$t" | shasum -a 256 -c
$ tar -xzf "rulec-$v-$t.tar.gz" && install -m 755 rulec ~/.local/bin/
$ rulec --version
rulec 0.13.0
```

Or from source, with a recent stable Rust: `cargo install --path .` fetches nothing, because
there are no dependencies. In CI, `uses: i2y/rulec@v0.13.0` does the download and the check
([In CI](#in-ci)).

## Using it

```console
$ rulec check rules/*.rule                         # --format json, --terse, --diff-base HEAD
$ rulec fmt --check rules/*.rule                   # the gofmt convention
$ rulec gen rules/*.rule --out generated [--check]
$ rulec vectors | coverage | test                  # the test cases, their coverage, the run
$ rulec adapter | schema | verify                  # against a legacy implementation
$ rulec fixtures lint | replay | diff              # against past records
$ rulec doc rules/送料.rule --lang ja               # for whoever approves the table; --format html
$ rulec certificate rules/送料.rule                 # the evidence, for another program to re-check
$ rulec source fetch | pin | outdated              # the copies of the documents a rule cites
$ rulec api rules/送料.rule                         # how to call the generated code, without reading it
$ rulec explain E101                               # when it appears, how to fix it, a repro
$ rulec import xlsx 運賃表.xlsx --sheet 本則       # a first draft from the workbook; every guess is marked
$ rulec mcp                                        # the same commands as MCP tools, for an agent with no shell
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
fixed in English whatever language `--lang` puts the prose in. An unknown flag is refused with
exit 2 rather than ignored. Every diagnostic is defined once, in `src/codes.rs`, and
[`docs/codes.md`](docs/codes.md) is literally the `rulec explain --all` output.

## In CI

```yaml
- uses: actions/checkout@v7                  # with fetch-depth: 0, so --diff-base can read origin/main
- uses: i2y/rulec@v0.13.0                     # the release binary, verified against its checksum
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/ --require-all   # the only step that needs the toolchains
```

Those logs are read by machines and developers, so they stay in the default English. What goes
to a person — `rulec diff` on a pull request, `rulec doc` for an approver — is built in the
same job with `RULEC_LANG` set to their language. The job that puts the diff on the pull
request is on the [install page](https://i2y.github.io/rulec/install/#in-ci).

## What is in this repository

```
AGENTS.md         the procedure an agent follows
DESIGN.md         the design record, in Japanese: every decision, and what was discarded with it
docs/             reference.md (the grammar), formats.md (machine-readable output),
                  generated-code.md, backends.md (targeting another language),
                  codes.md / codes.ja.md (every diagnostic, generated)
website/          the documentation site (Zensical): docs/ English, docs-ja/ Japanese
skills/rulec/     an agent skill for using rulec — copy the folder into .claude/skills/;
                  `rulec mcp` serves the same commands as MCP tools where there is no shell
proofs/           the Lean 4 development: what a table means, the checks a certificate has to
                  pass, the theorems that each check settles its claim, and the re-checker
                  built from those very functions
src/              42 modules, and 5 more under codegen/: kw, i18n, lex, parse, types, defset
                  (the tables that define one output, as one set), region, eval, fmt, json,
                  codegen, backend, vectors, coverage, verify, fixtures, replay, report, doc,
                  cert (the certificate), import and xlsx (a draft from a sheet: ZIP, deflate,
                  the number formats), proto and jsonschema (the enums whose values are
                  declared outside the rule), enums, mcp (the command table as MCP tools),
                  codegen/tool (the rule as an MCP tool and its view), codegen/sql (one query,
                  and the same query as a function), sources (a law on e-Gov, cited and
                  pinned), apply (a rule applied to another case), vfs (reading at a git
                  revision), sha256, wasm (the checker as the site's playground)
tests/corpus/     39 rules, and the copies of the documents they cite
tests/mutants/    83 files, each with one mistake planted in it
tests/golden/     the diagnostic prose snapshot by snapshot: 37 in Japanese, 35 in English
tests/oracle/     two premium tables transcribed grade by grade from their published PDFs,
                  which tests/library.rs replays the rules over
tests/            and the properties: threeway (every language agrees), readme, docs,
                  website, skill, codes, json_v2, formats, api, coverage, m3, budget, library,
                  mcp, import, xlsx, proto and jsonschema (an enum held to the file it is
                  declared in), tool (the rule as an MCP tool), sql, wasm (the site's
                  playground answers what the binary answers)
```

39 rules — 32 taken from real published terms, 7 written to reach the rest of the language — are checked, generated and run on every commit, and all 77 diagnostics are implemented.
Those rules come from **public information**: Japan Post's tariff, Yamato's size classes, the coupon
terms of Rakuten and Yahoo, Article 7 of EU Regulation 261/2004, the National Tax Agency's
income-tax and stamp-duty tables, the Stamp Tax Act and the Special Taxation Measures Act as
e-Gov publishes them, the premium tables of 協会けんぽ and 日本年金機構, GOV.UK's minimum wage and
income tax rates, the IRS rate tables, and a section of the US Code of Federal Regulations as
the eCFR publishes it — or are sketches
written to reach the corners of the language, two of them in English. None of it is private
data. The two premium tables are also held,
grade by grade, to the amounts printed in them.

```console
$ cargo test          # 400 tests; python3, node, rustc, ruby, php, go, swiftc and a JDK are used where present
```

## Where to read next

| | |
|---|---|
| **[The documentation site](https://i2y.github.io/rulec/)** | all of this at length, in English and [日本語](https://i2y.github.io/rulec/ja/) |
| [Where to start, by what you have](https://i2y.github.io/rulec/#where-to-start-by-what-you-have) | a spreadsheet, an implementation that runs today, or past records — the first move for each |
| [`AGENTS.md`](AGENTS.md) | the procedure for an agent: write → check → fix → generate → integrate → show the impact → ask a person |
| [`docs/reference.md`](docs/reference.md) | the complete grammar |
| [`docs/codes.md`](docs/codes.md) / [`docs/codes.ja.md`](docs/codes.ja.md) | every diagnostic code, as `rulec explain --all` prints it |
| [`docs/formats.md`](docs/formats.md) | every machine-readable format: `--format json`, vectors, fixtures, manifests, the adapter protocol |
| [`docs/generated-code.md`](docs/generated-code.md) | the shape of the output, its guarantees, and how to call it |
| [`docs/backends.md`](docs/backends.md) | targeting a language rulec does not generate, without losing the comparison |
| `DESIGN.md` | why each decision was made and what was discarded with it (Japanese) |

The five references under `docs/` are carried into the site verbatim: holding the same text
twice is how one of the copies goes stale.

## About the design

The decision-table semantics and the hit-policy vocabulary are borrowed from DMN, and the
detection of overlap and gaps follows the formulation of Calvanese et al. Not borrowed: the
XML interchange format, the runtime engine, the GUI modeller. What DMN does not cover — units,
rounding, code generation for several languages, replay against past records — is where this
tool differs.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
