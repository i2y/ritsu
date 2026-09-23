# rulec

**A small language for table-shaped business rules, and a harness for the agent that turns
them into code.**

Write the table, and out come Python, NumPy, TypeScript, JavaScript, Rust, Ruby, PHP, Go,
Swift, Java, SQL and Wasm — ordinary functions with no runtime, no configuration and no
dependencies. **The proof is finished before the code exists**: a rule that cannot be proved
does not generate.

```rule
rule fee_demo v1
description "The README's example. Passes rulec check as written"

enum size_class = envelope | small | large
enum zone = domestic | canada | overseas

group north_america = domestic, canada

inputs
  dest      : zone
  girth     : length[in]  range >=1in <=130in
  weight    : mass[lb]    range >=1lb <=70lb   contract_only

outputs
  fee : money[USD, incl_tax]  round up(1USD)

table size_of
policy first
| girth  | -> size : size_class |
| <=22in | envelope             |
| <=60in | small                |
| -      | large                |

table fee_table
policy unique
| dest          | size     | -> fee : money[USD, incl_tax] |
| north_america | envelope | 6USD                          |
| north_america | small    | 12USD                         |
| north_america | large    | 22USD                         |
| overseas      | envelope | 16USD                         |
| overseas      | small    | 38USD                         |
| overseas      | large    | 60USD                         |

examples
| dest     | girth | weight | -> fee |
| domestic | 10in  | 5lb    | 6USD   |
| overseas | 40in  | 12lb   | 38USD  |
```

**The keywords are English; the names and the cell values stay in the language of the
business** — English here, and Japanese in the rules transcribed from Japanese terms and
statutes, which are on the [examples page](https://i2y.github.io/rulec/examples/) with the
rest. This example passes `rulec check` as it stands — the repository's tests run it on
every commit. `examples` is an executable specification, and a row that does not hold is
reported with the rows that fired. A cell tests **its own column and nothing else**, which is
what makes a row a box and the completeness and overlap checks exact; complicated rules are
written by **stacking tables**, as above, where what one table produces is a column of the
next.

Out of `rulec gen`, in Python:

```python
def fee_demo(dest: Zone, girth: Inch, weight: Pound) -> USDInclTax:
    """Rule fee_demo v1: the same decision as fee_demo_traced, without the rows that matched."""
    out, _ = fee_demo_traced(dest, girth, weight)
    return out


def fee_demo_traced(dest: Zone, girth: Inch, weight: Pound) -> tuple[USDInclTax, list[Fired]]:
    if not _isinstance(dest, Zone):
        raise RuleInputError("dest is not a value of enum Zone", dest)
    if not 1 <= girth <= 130:
        raise RuleInputError("girth is out of range", girth)
    trace: _Trace = []
    # table size_of (policy first)
    if girth <= 22:  # row 1: <=22in | envelope
        size = SizeClass.ENVELOPE
        trace.append(Fired("size_of", 1))
    elif girth <= 60:  # row 2: <=60in | small
        size = SizeClass.SMALL
        trace.append(Fired("size_of", 2))
    ...
    # table fee_table (policy unique)
    if dest in _north_america and size == SizeClass.ENVELOPE:  # row 1: north_america | envelope | 6USD
        fee = 6
        trace.append(Fired("fee_table", 1))
    ...
    else:
        raise AssertionError("unreachable: completeness was statically checked by rulec")
    return USDInclTax(_round_up(fee, 1)), trace
```

One row of the table becomes one branch, with the cells it came from beside it as a comment,
and **no cell is dropped** — a condition an earlier branch already settled is still written
out (`elif True:`), because reading the output against the table is the only way it is meant
to be read. The function you call is `fee_demo`, and its signature does not change; beside it
`fee_demo_traced` returns **the rows that matched** — one per table, as the table's name and
its row number — which is what a log line or an answer to "why this fee" needs. Beside those
come a record writer for the fixtures format, the rule as one MCP tool over stdio and
Streamable HTTP, the rule as a Connect service with the `.proto` it is called through, and a
page an approver can try a case on.

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
   as written. Cite the document a table was transcribed from (`@source table1`) and an amount
   that disagrees with the copy does fail (E116, W120), as does a boundary the copy puts on the
   other side of itself (E119) — but even then what is shown is agreement with the copy, not
   with the world. With no citation, transcribe the tariff wrong and everything stays green.
2. **That the generated code answers like the table.** That is a *test*, not a proof: test
   cases built from the boundaries are run through the reference evaluator and every generated
   language, and compared byte for byte. Strong evidence, not an equivalence proof.
3. **Row pairs the overlap proof could not reach.** When neither an input matching both rows
   nor its impossibility could be constructed, **W114 names the pair and moves the check into a
   runtime guard** — the one place with no static proof. It returns an error rather than
   silently picking a side. Derived values that share an input and the thresholds inside a
   boolean definition used to land here and no longer do: Fourier–Motzkin elimination decides
   both. What is left is what it cannot decide because it works over the rationals.
4. **That the checker itself is right.** The proofs above come out of rulec's own
   implementation, which has not itself been proved correct.

Three things are built against (4), and none of them shares code with the checker. `gen`
writes proof harnesses for [Kani](https://model-checking.github.io/kani/) beside the Rust,
behind `#[cfg(kani)]`, which decide over **every** input in the declared domain rather than
over the test cases; on the corpus, 111 of them verify in 175 seconds. `rulec certificate`
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
$ v=v0.17.0; t=aarch64-apple-darwin     # or x86_64-apple-darwin, x86_64-unknown-linux-musl, aarch64-unknown-linux-musl
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/$v/rulec-$v-$t.tar.gz"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/$v/SHA256SUMS" | grep "$t" | shasum -a 256 -c
$ tar -xzf "rulec-$v-$t.tar.gz" && install -m 755 rulec ~/.local/bin/
$ rulec --version
rulec 0.17.0
```

Or from source, with a recent stable Rust: `cargo install --path .` fetches nothing, because
there are no dependencies. In CI, `uses: i2y/rulec@v0.17.0` does the download and the check
([In CI](#in-ci)).

## Using it

```console
$ rulec check rules/*.rule                         # --format json, --terse, --diff-base HEAD
$ rulec fmt --check rules/*.rule                   # the gofmt convention
$ rulec gen rules/*.rule --out generated [--check]
$ rulec vectors | coverage | test                  # the test cases, their coverage, the run
$ rulec adapter | schema | verify                  # against a legacy implementation
$ rulec fixtures lint | replay | diff              # against past records
$ rulec diff rules/parcel.rule@HEAD rules/parcel.rule  # …and with no records: which inputs move
$ rulec doc rules/parcel.rule --lang ja            # for whoever approves the table; --format html
$ rulec certificate rules/parcel.rule              # the evidence, for another program to re-check
$ rulec graph rules/parcel.rule                    # what decides each value, and what reads it
$ rulec source fetch | pin | outdated              # the copies of the documents a rule cites
$ rulec api rules/parcel.rule                      # how to call the generated code, without reading it
$ rulec explain E101                               # when it appears, how to fix it, a repro
$ rulec import xlsx tariff.xlsx --sheet standard   # a first draft from the workbook; every guess is marked
$ rulec mcp                                        # the same commands as MCP tools, for an agent with no shell
```

Transcribe a tariff, leave one of its bands out, and the gap comes back with the input that
falls through it:

```
error[E101]: Completeness gap: some input matches no row
  --> rules/parcel.rule:30 table base_rate
   |
30 | table base_rate
   |       ^^^^^^^^^ the input space is not fully covered
   |
 An input that matches no row: dest = overseas, size = small, weight = 1lb
 hint: add a row that matches this input.
 The shape of the row to add: `| overseas | small | 1lb | 6USD |`. Its output values are copied from the first row to give a shape that parses; they are not the right amounts. Decide whether the written rule, the spreadsheet or the legacy implementation is the source, and take them from there. One row closes the gap this witness names; if more is left, the next run names the next one.
```

**No legacy implementation and no past data are needed for that.** Every command carries
`--format json`, where a finding is data — `where`, `witness`, `rows`, `fix` — with the keys
fixed in English whatever language `--lang` puts the prose in. An unknown flag is refused with
exit 2 rather than ignored. Every diagnostic is defined once, in `src/codes.rs`, and
[`docs/codes.md`](docs/codes.md) is literally the `rulec explain --all` output.

## In CI

```yaml
- uses: actions/checkout@v7                  # with fetch-depth: 0, so --diff-base can read origin/main
- uses: i2y/rulec@v0.17.0                     # the release binary, verified against its checksum
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
src/              46 modules, and 6 more under codegen/: kw, i18n, lex, parse, types, defset
                  (the tables that define one output, as one set), region, eval, fmt, json,
                  codegen, backend, vectors, coverage, verify, fixtures, replay, report, doc,
                  cert (the certificate), graph (the rule as one graph of what decides
                  what), vdiff (two versions compared over the whole input space),
                  import and xlsx (a draft from a sheet: ZIP, deflate,
                  the number formats), proto and jsonschema (the enums whose values are
                  declared outside the rule), enums, mcp (the command table as MCP tools),
                  codegen/tool (the rule as an MCP tool and its view), codegen/connect (the
                  rule as a Connect service: the .proto and what stands behind it),
                  codegen/sql (one query,
                  and the same query as a function), sources (a law on e-Gov, cited and
                  pinned), apply (a rule applied to another case), vfs (reading at a git
                  revision), sha256, wasm (the checker as the site's playground)
tests/corpus/     47 rules, and the copies of the documents they cite
tests/mutants/    94 files, each with one mistake planted in it
tests/golden/     the diagnostic prose snapshot by snapshot: 44 in Japanese, 36 in English
tests/oracle/     two premium tables transcribed grade by grade from their published PDFs,
                  which tests/library.rs replays the rules over
tests/            and the properties: threeway (every language agrees), readme, docs,
                  website, skill, codes, json_v2, formats, api, coverage, m3, budget, library,
                  mcp, import, xlsx, proto and jsonschema (an enum held to the file it is
                  declared in), tool (the rule as an MCP tool), connect (the rule as a
                  Connect service), sql, wasm (the site's
                  playground answers what the binary answers)
```

47 rules — 36 taken from real published terms, 11 written to reach the rest of the language — are checked, generated and run on every commit, and all 83 diagnostics are implemented.
Those rules come from **public information**: Japan Post's tariff, Yamato's size classes, the coupon
terms of Rakuten and Yahoo, Article 7 of EU Regulation 261/2004, the National Tax Agency's
income-tax and stamp-duty tables, the Stamp Tax Act and the Special Taxation Measures Act as
e-Gov publishes them, the premium tables of 協会けんぽ and 日本年金機構, GOV.UK's minimum wage,
income tax and stamp duty rates, the IRS rate tables, three sections of the US Code of Federal
Regulations as the eCFR publishes them, and PayPal's own merchant fees — or are sketches
written to reach the corners of the language, two of them in English. None of it is private
data. The two premium tables are also held,
grade by grade, to the amounts printed in them.

```console
$ cargo test          # 519 tests; python3, node, rustc, ruby, php, go, swiftc, a JDK and protoc are used where present
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
