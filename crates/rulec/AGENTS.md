# Working with rulec

You are the first user of this tool. It exists so that a business rule — a shipping tariff, a
coupon policy, an eligibility test — can be written as one table, **proved** correct before
anyone runs it, and turned into ordinary Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm —
or, for a host that decides whole columns at once, into NumPy: the rule as data beside one fixed evaluator.

Your job is the middle of that: take a source of truth (a published policy, a spreadsheet, a
legacy implementation) and produce a `.rule` that passes `rulec check`, then generate the
code and show a person what changed.

Two roles stay with people, and neither is yours:

- **someone approves the table.** Amounts, rounding directions, and which of two conflicting
  readings is correct are business decisions. When you cannot derive an answer from the
  source, you ask — §6 says how.
- **someone owns the application** the generated function is called from.

Everything you need is reachable from the command line. `rulec --help` lists the commands,
`rulec <cmd> --help` explains one, `rulec explain <CODE>` explains a diagnostic, and
`--format json` gives you any of it as data. There is no step where you have to read rulec's
source. Where you have no shell, `rulec mcp` serves the same commands as MCP tools over
stdio — one tool per command, one argument per flag, the exit code at the end of every
result — and this document and the references as resources.

---

## 1. The loop

```
write  →  fmt  →  check  →  (fix, repeat)  →  examples  →  gen  →  test
                                                                    │
             CI: fmt --check, check --diff-base, gen --check, coverage, test
                                                                    │
       legacy implementation? → verify        past records? → replay, diff
                                                                    │
                                              a person approves → doc
```

### Write

The grammar is [docs/reference.md](docs/reference.md), complete. The shortest useful summary:
declare `inputs` and `outputs` with their units, write one `table` per decision, and let
`examples` state a few cases you know the answer to.

Two declarations are mandatory and are where most first drafts fail:

- **`range` on every numeric input and every `derive`.** It is the universe the completeness
  proof quantifies over, and the entry guard of the generated code. A rate may leave it out
  and is then 0% to 100%, guard included; a rate that can go above 100% declares its range.
- **`round` on every numeric output.** Without it the generated code would settle fractions
  silently.

A few shapes are worth knowing before the first draft:

- **Tables stack.** A table's output column is a column of any later table, to any depth, and
  one table may produce several output columns. That, plus a `derive` used as a column, is how
  a rule with interlocking conditions gets written — not by putting more into a cell.
- **An output returns the binding of its own name** — a `define` or a table output column
  called `送料` is what the output `送料` returns. `result` is sugar for the **first** output
  only: naming a later one is E015, and a second `result` line is E016.
- **Combinations that cannot happen are said once.** `constraint <input> <= <input>` states a
  relation the caller guarantees. Completeness then demands no row for what it excludes, every
  witness becomes a case somebody could really send, and the generated code refuses a
  violating input at the door (§6.1 of the grammar).
- **A case may carry a sequence.** Where the number of things is not fixed — the rows of a
  tariff sheet, the candidates a filter left — `elements` declares what one element carries
  and `fold` says what each verdict does next: `next`, `stop with <value>`, `take_unique`,
  `take_first`, `keep_max <value> by <key>`, plus the required `empty` and `exhausted`. The
  table that judges one element is checked exactly as any other table is, and an example
  names a `sequence` rather than holding one in a cell (§6.2). Every target but SQL
  generates it.
- **A `string` column takes a prefix and nothing else.** `starts_with "CH-", "CL-"` is the
  one test it accepts (E110 for any other): the prefixes a column's own cells name cut the
  strings into finitely many classes, which is what keeps completeness decidable. Bytes, no
  case folding. Where the values can be listed, an `enum` says more.
- **Counting the walk instead of folding it.** `count <name>(<alias>) over <sequence> where
  <column> = <value>` ends the walk with a number rather than with the answer, and the rule
  goes on as usual — so what turns "how many matched" into a class is an ordinary table, and
  its boundaries are checked like any other (§6.3). The `range` is required: it is the
  universe the completeness check quantifies over **and** the cap on the sequence. `sum
  <name>(<alias>) over <sequence> of <column>` ends it with a total instead, over a column
  that cannot go negative; an average does not, dividing by the count being division by a
  variable. A rule has a `fold` or a `count`/`sum`, never both (E031).

- **Handing one amount out over several lines.** `allocate(<amount>, <running total>,
  <whole>)` is `<amount> × <running total> ÷ <whole>` rounded down. Write one line's share
  as the share up to it minus the share up to the line before: the parts then add up to the
  amount exactly, odd yen included. Three names with declared ranges, nothing negative, a
  positive whole, and a `constraint` that the running total never passes it — else E117.

- **A main rule and its special case are two tables, or a table and a clause.** Several
  tables may define the same output, each transcribed from its own source, and the one that
  takes precedence says so with `overrides <table>` right after `policy` (`overrides 本則:r3`
  for one labelled row). A row is labelled by a word before its first bar (`r3 | … |`), which
  is also how the trace names it. A proviso whose conditions do not line up as columns is a
  `clause`: `when <column> <cell> and …` (or `when always`), `then <value>`, and `overrides`
  — one row written as prose, checked and generated as a one-row table. The definitions of
  one output are checked together — completeness over their union, every overlap either
  ordered by an `overrides` line or reported (E105) — and `doc` says which is the exception
  (§7 of the grammar).

- **A source is declared, cited and pinned.** `source 法 = law "342AC0000000023" asof
  2026-04-01` names a law on e-Gov and `law ecfr "29 CFR 1910"` one in the US eCFR; the word
  after `law` picks the database, e-Gov when left out. `@法 別表第一` at the end of a table,
  clause, row, derive or define line says what it transcribes, quoted if not one word; `rulec source fetch` puts a copy of each cited
  fragment beside the rule and `rulec source pin` writes its digest under the `source` line.
  `check` then holds the rule to the copies (E037–E039, W119) without reading the network,
  and `doc` quotes the fragment under the definition. Any other document is
  `source 郵便 = file "…" sha256:…`, and a `url "…"` on it says where that copy came from, so
  `fetch` can bring it again and `outdated` can ask whether it moved on — pinned to a commit
  (`raw.githubusercontent.com/<owner>/<repo>/<commit>/<path>`) the answer names the commits
  that touched it, otherwise it can only say that the bytes differ. **A document's fragments
  are its tables**: `@郵便 表1` cites the first table of the document in document order (a
  sheet, in a workbook), `fetch` takes it out and writes it beside the document as
  `<document>.fragments/表1.tsv` (`料金表.md.fragments/表1.tsv`), and from there it is pinned, checked and quoted exactly as a
  law's article is. Cite the table the rows came from and a revision that moves it fails the
  check, naming the rows to reread. The formats read are csv, md, xlsx and docx; for a PDF
  or a scan, `fetch --via <cmd>` runs an extractor (docling and the like) as a child process
  and pins what it produced, so the extraction happens once and is reviewed once —
  `rulec adapter <file.rule> --template docling` prints the shape. **The rows are then held to that copy**: an
  amount the copy does not show is E116, and a number the copy states that no row uses is
  W120 — the two halves of a mistyped digit, and the one check that looks outside the rule
  at all.

- **A provision applied mutatis mutandis is an `apply`.** `apply 退職手当 = "退職手当.rule"
  sha256:…` uses another rule with every input bound (`勤続年数 = 在職期間`, enums mapped with
  `with 任期満了 -> 定年`), definitions left out with `except 減額`, and outputs taken under a
  name (`手当 -> 非常勤手当`). The callee is expanded into the rule under `退職手当:…`, held to
  its digest (E040, `rulec source pin` writes it), and what is passed has to stay inside its
  ranges (E043). `check` and `doc` show the applied tables as the callee wrote them.

- **An enum may belong to somebody else.** When the values come from a service contract,
  `import proto "<file>" <Enum> -> <enum of this rule>` (or `import jsonschema "<file>"
  "<pointer>" -> <enum of this rule>`, for a JSON Schema or an OpenAPI document in JSON)
  says so, and every `check` reads that file and holds the two together: the `.proto` owns which values exist, the `.rule` owns what
  they are called here and what each one costs. A value on one side only is E032. Once the sets
  agree, a value that no row names and no `default` marks is E033 — not the warning W111, because
  a value that arrived through the contract is a change nobody has read yet, and a table with a
  `-` row would pass completeness while the new value quietly takes the default amount.

When the source is a spreadsheet, `rulec import xlsx <file.xlsx>` writes a first draft from
the workbook as it is — no export step, `--sheet <name>` to pick the sheet, and the first
one by default; `rulec import csv <file.csv>` does the same from a CSV. The columns become
inputs, the last column the output, the values an enum or a range, and each guess is marked
`# 推定` — which is then yours to correct: a numeric column is copied as equalities and is
usually meant as thresholds, and no range, rounding or unit in it is decided. A date, a
percentage and a unit that lived in the cell's number format are read back out of it. It
saves the typing, not the reading.

One habit pays for itself at the first revision:

- **Write down where each table came from.** A comment at the end of the `table` line names
  the source — the document, its edition or date, the page — and a row taken from somewhere
  else (a later notice, a correction, an answer from a person) carries its own comment at the
  end of the row. `rulec doc` shows both to the approver, so a review becomes "compare this
  row with that cell" rather than "read the whole table again", and when the source is
  revised, the rows to re-read are the ones that cite it. The `description` line stays a
  one-line summary of what the rule decides.

### `rulec fmt <file>`

Run it before `check`, every time. It aligns the columns and rewrites `→ ・ 、 ≦` to ASCII,
so your diffs stay about the rule and not about whitespace.

### `rulec check <file> --format json`

The centre of the loop. One JSON object per finding
([docs/formats.md](docs/formats.md#check)). Read these fields:

| field | what to do with it |
|---|---|
| `code` | decides the fix. `rulec explain <code>` if you do not recognise it |
| `where.table`, `where.row` | which table and row, without parsing the sentence |
| `witness.inputs` | a concrete input that exhibits the problem. **This is the thing to reason about** |
| `rows` | every row that takes part |
| `fix.kind`, `fix.text` | the rewritten form, ready to paste — see the warning in §3 |
| `notes` | prose, including the caveats that `fix.text` cannot carry |

`--terse` cuts each finding to heading, position and witness when you are scanning many.

Exit codes: **0** no errors, **1** errors, **2** bad arguments or an unreadable file. Read
the exit code — not whether the output looks empty.

### Fixing, by code

Every code is in `rulec explain --all` ([docs/codes.md](docs/codes.md)). The ones you will
meet while transcribing:

- **E101 completeness gap** — some input matches no row, and the witness names it. Add a row
  that covers it, or mark the value `default` in the enum to let it fall through to a
  catch-all. One row per run; run again for the next gap.
- **E105 overlap** / **W105 shadowing** — two rows match the same input. Under `policy
  unique` that is an error; under `policy first` the earlier row wins and you are being asked
  whether that is intended.
- **E102 unreachable row** — a row nothing can reach, either because earlier rows cover it or
  because the upstream table never produces the value it names.
- **E103 unit mismatch** — you are adding or comparing values of different units, currencies
  or tax flags. The fix is a table, not a cast.
- **E104 no rounding** / **E106 off the grid** — see §3.
- **E107 example does not hold** — the rule and your expected value disagree. **Decide which
  one is wrong from the source**, not from which is easier to change.
- **E112 derived range too narrow** — the message states the interval to widen to.
- **E014 expression in an output cell** — a cell to the right of `->` holds one value or one
  name. Give the calculation a name on a `define` line and put that name in the table.
- **E116 an amount that is not in the copy** / **W120 a value of the copy that no row uses**
  — the row and the table it cites disagree. Reread the copy: a mistyped digit raises both
  and names both halves. An amount that came from elsewhere loses its citation and gains a
  comment saying where it came from.
- **E114 value off the column’s step** — `0.5%` in a `rate[step 1%]` column has no runtime
  representation. Write a value on the step, or declare a finer step (`rate[step 0.1%]`).
- **E022 / E023 / E024 a fold with a hole** — `empty` and `exhausted` are both required, and
  every verdict the table can produce needs an arm. The three are the completeness argument,
  applied to the walk.
- **E028 / E029 / E030 a count that cannot be counted** — the shape is `count <name> over
  <sequence> where <column> = <value>`; the column has to be one an element has, with values
  that are a closed set; and the range is required. E030's message says why it is two things
  at once.
- **W111 unused declaration** — see §3.

### `examples`

Add them as soon as the tables check clean. They are the only thing that can catch an error
the reference evaluator and every generated language share — which happened once, with
rounding for multiple outputs missing in all of them. Take the cases from the source: a
published tariff's own worked examples are ideal.

**They ride in the vector suite.** `gen` writes each out as a case of its own, so `rulec test`
runs them through every generated language and `rulec coverage` counts what they reach. A row
goes in only when it names every input.

### `rulec gen <file> --out generated/ --format json`

Writes Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm, NumPy, and the vectors. It refuses to generate
from a rule that does not pass check. `rulec api <file>` tells you how to call the result —
signatures, parameters with units and ranges, enum member spellings, errors — so you never have
to read the generated code to integrate it ([docs/generated-code.md](docs/generated-code.md)).
Beside each function is a twin with `_traced` on its name that also returns the rows that
matched, one per table in order — the row numbers `rulec doc` prints, and what an answer to
"why this amount" needs; `rulec test` holds those rows to the reference evaluator as well as
the values. A third, `_record`, turns one call into one line of the fixtures format, so what
`replay` and `diff` need comes out of the generated code itself. A fourth file beside the module,
`<alias>_mcp.py` (`.mjs` in the JavaScript directory), serves the rule as one MCP tool for an
agent that will *call* it: the arguments are the wire form, the answer is the record line, and
`--record <file.jsonl>` keeps every call. It speaks stdio, and with `--http <port>` MCP's
Streamable HTTP — put TLS and authentication in front of that one; where the host renders MCP
Apps it also offers the approver's page (`<alias>_page.html`) as the tool's view. The `wasm/`
directory holds the rule as one module for any host, behind `call: func(input: string) ->
string` in the canonical ABI, with a `.wit` that makes a component of it. The `sql/` directory
holds two doors on one query: `<alias>.sql` over a relation of inputs, and
`<alias>_function.sql`, the same query as a PostgreSQL function asked one case at a time — an
RPC endpoint the moment it sits in a schema PostgREST or Supabase exposes. How to build and
call each is in [docs/generated-code.md](docs/generated-code.md), under its own heading.

Beside the Rust module, `<alias>_proof.rs` holds proof harnesses for the
[Kani](https://model-checking.github.io/kani/) model checker, behind `#[cfg(kani)]` so `rustc`
never reads them: over **every** input in the declared domain rather than over the vectors, no
table falls through, no contradiction guard fires, nothing overflows an `i64`, and each
`unique` table's rows cover the domain exactly once. `rulec test --proofs` runs them where
`kani` is installed ([docs/generated-code.md](docs/generated-code.md)).

For a target none of the eleven covers — another language, a workflow engine's expression
language, a spreadsheet formula — you need no backend and give up no comparison: generate from
`rulec api`, wrap it in the adapter protocol, and hold it to the rule with `rulec verify`
([docs/backends.md](docs/backends.md) runs that loop end to end).

### `rulec certificate <file.rule>`

The evidence behind all five proofs, as one JSON object something other than rulec can
re-check: the tree tiling the input space, the axis each pair of rows of a `unique` table
parts on, a point reaching each row with its values, every computed value's interval and
type, and where each cell stands in the file. `tools/recheck.py` re-checks it with no
dependencies; `proofs/` proves in Lean 4 that those checks imply the claims and runs them.
`--rule <file>` ties it to that text, cell by cell ([docs/formats.md](docs/formats.md)).

### `rulec test generated/ --format json`

Runs every generated language over the vectors and compares them with the reference
evaluator, byte for byte. This is the only step that reaches outside: it wants `python3`,
`node`, `rustc`, `ruby`, `php`, `go`, `swiftc` and a JDK, plus the `wasm32-unknown-unknown` target for the
Wasm side, `numpy` under `python3` for the NumPy one, a `psql` that reaches a PostgreSQL for the function beside the query, and, under `--proofs`, `kani` for the proof harnesses `gen` writes beside the `rustc` module (§15.95 — that pass is not vectors: it holds the generated code over **every** input in the declared domain, and costs seconds per rule where the vectors cost milliseconds, which is why it is asked for), and skips-and-reports the side whose toolchain is missing. A skipped language narrows what the run proved, so the summary says how many were
skipped and **`--require-all` fails when any was** — that is the form for CI, where green has
to mean the agreement held across all of them.

### In CI

```yaml
- uses: actions/checkout@v7                  # with fetch-depth: 0, so --diff-base can read origin/main
- uses: i2y/rulec@v0.13.0                     # the release binary, verified against its checksum
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/ --require-all
```

`--diff-base` reports only findings that are new since that revision, so pre-existing
warnings do not drown the ones this change introduced. `gen --check` fails when the generated
code is stale — that is the gate that keeps the committed output honest.

### If there is a legacy implementation: `rulec verify`

Write a 20-to-30-line adapter (`rulec adapter --template python|go` gives you the shape) and
rulec streams the vectors through it. Mismatches come back clustered by the rows that fired,
with counts, amount differences and a witness; a cluster whose differences are all smaller
than the output's rounding grid is flagged as a suspected rounding difference.

**A mismatch is not automatically your bug.** It is one of four things: a defect in the legacy
implementation, a transcription error in your table, dirty records, or a rounding convention.
The cluster and its witness are what tell them apart.

### If there are past records: `rulec fixtures lint`, `replay`, `diff`

`fixtures lint` first, always — it reports records whose shape disagrees with the rule instead
of quietly dropping them. The generated `_record` writes that shape already; only a log of some
other implementation has to be extracted. Then `replay` compares the rule against what actually
happened — and, for records carrying the rows that matched, row by row: an amount that agrees
under a different row is reported apart, as a moved row. `diff` compares two versions over the
same records and reports **how many change and by how much**, which is the number a person
needs before approving. A version is named by its file, by its git tag (`送料@v3` is the tag
`rules/送料/v3`, or failing that the revision `v3`), or by a path at a revision — on a pull
request the old version is `rules/送料.rule@origin/main`. `--format markdown` is what gets
posted, and `--terse` keeps every value of a record out of it.

### For the person who approves: `rulec doc`

`rulec doc <file> --lang ja` renders the rule as markdown with the facts the checker knows
that the text does not show — that a group of six values and its complement of 41 really do
cover all 47, which rows shadow which, where a rounding was assumed rather than sourced. It
is rendered in CI and pasted into the PR, never committed: a stale rendering that still looks
authoritative is the danger it is designed against. `--format html` renders the same document
as one page with a form on it: the approver types a case, the rows that matched light up, the
outputs appear, and the line the generated code would log is shown — it is the generated
JavaScript itself that runs, so the page says nothing the code does not. Same rule: built in
CI per change, never committed.

### For the customer: `rulec doc --audience customer`

`rulec doc <file> --lang ja --audience customer` renders the rule as the article a help centre
publishes, so that whatever answers customers — a person, a search, a model reading retrieved
pages — reads a page that is complete, carries the version, and spells out both sides of every
threshold. Those come from the boundary-pair vectors: the article says "60cm → 1410円, 61cm →
1710円" where the table says `<=60cm`, and aliases, ranges and codes are left out. Regenerate it
on every change; like the approver's page, it is a product of the file, never a source.

---

## 2. Language

Output is English by default. `--lang ja` (or `RULEC_LANG=ja`) switches every surface —
diagnostics, reports, the rendered document, the prose inside generated code — to Japanese.
Names and cell values come from the rule and are unchanged either way.

Use the default for yourself. Use `--lang ja` for anything a Japanese-reading person will
read: the `doc` rendering and the `diff` comment on a PR.

**JSON keys never move with the language**, and neither do `witness` or `fix.text`. Only
fields whose name says prose (`title`, `notes`, `what`, `hint`, `text` in a note) change.

---

## 3. What not to do

**Do not edit generated code.** The header says `DO NOT EDIT` and `gen --check` will fail on
it in CI. Anything missing belongs in the `.rule` or on the calling side.

**Do not decide a rounding yourself.** `round down(1円)` will make E104 go away, and
`fix.text` will hand it to you, but *which direction and which grid* moves real money. If the
source says, use what it says. If it does not, ask — and write the answer's provenance in a
`#` comment next to the declaration, because that comment is the only place the approver will
see that the rounding was assumed.

**Do not widen a range to make a check pass.** Widening a `derive` range to the interval E112
names is correct: that interval is what the value can actually reach. Widening an *input*
range so a hole disappears is not — it changes the contract the generated guard enforces, and
it hides the hole instead of closing it.

**Do not silence a warning to get to green.** `default` on an enum value declares "this value
intentionally has no row of its own"; `contract_only` on an input declares "this only ever
guards a range". Both are statements about the business. Using either because a warning was
in the way makes the next reader believe something untrue.

**Do not add a row with a made-up amount.** `fix.text` for E101 gives you the row's *shape*,
with output values copied from the first row so that it parses. The amount has to come from
the source. If you do not have it, that is a question for a person (§6), not a value to
invent.

**Do not paper over E107 by changing the expected value.** Half the time the table is wrong.
Decide from the source which side is.

**Do not reach for `policy first` to make an overlap go away.** `first` says "the order of
these rows is meaningful", which is a claim about the business. If the rows genuinely should
not overlap, fix the rows.

---

## 4. What rulec will not do, and why that is the point

It has no loops in an expression, no recursion, no state, no nested objects in a cell, and no
date arithmetic. A sequence is walked once, by a `fold`, and that is the whole of the
iteration there is: nothing accumulates across elements, so a total or a count is computed
before the call and passed in. Do not look for a way around these. They are the price of the
checks terminating: because a cell is a unary test on its own column, a row is a box, and
completeness and overlap are exactly decidable. Flatten nested data at the boundary; keep
the rest of the iteration in the caller.

It will not print a green result it cannot prove. When a check runs out of budget (E109) or
cannot decide an overlap (W114), it says so rather than approximating.

---

## 5. Reading a diagnostic as data

A worked example. Ask for JSON, act on the fields, never on the sentence:

```console
$ rulec check rules/ゆうパック運賃.rule --format json | jq -c 'select(.code=="E101") | {table:.where.table, witness:.witness.inputs, fix:.fix}'
{"table":"運賃表","witness":{"あて先":"山梨県","サイズ":"S60"},"fix":{"kind":"add_row","text":"| 山梨県 | S60 | 820円 |"}}
```

The prose in `title` and `notes` is allowed to improve between versions; the codes and the
JSON shape are a stable API. Anything you build that parses a sentence will break, and it
will break silently.

---

## 6. Asking a person

The checks turn vague gaps into specific questions, and a specific question is the one a
person can answer in a sentence. Convert the structured finding, not the prose.

| what you have | what to ask |
|---|---|
| E101, `witness.inputs = {あて先: 山梨県, サイズ: S60}` | 「山梨県あての S60 サイズの運賃はいくらですか」 |
| E104 on output `送料`, notes saying the spread is 9 yen | 「送料の端数はどちら向きに丸めますか。切り上げと切り捨てで最大 9 円変わります。規約に記載はありますか」 |
| E105 between rows 3 and 7 with different outputs | 「この入力は 1,200 円と 800 円のどちらですか。両方の条件に当てはまります」 |
| W114 | 「この二つの条件を同時に満たす注文は実在しますか」 |
| A rounding you assumed | 「この丸めは規約に根拠がありません。仮に切り捨てにしています。出典はありますか」 |

Three things make such a question answerable: **a concrete case** (the witness), **what turns
on the answer** (the amount that moves), and **what you assumed in the meantime**, so that
silence does not read as agreement. Keep that assumption in the file — a `#` comment where
the declaration is, or, for an amount taken from outside the table's own source, at the end
of its row. `rulec doc` shows those comments to the approver, the one place a person who can
overrule them will see them.

---

## 7. Where to look

| | |
|---|---|
| [docs/reference.md](docs/reference.md) | the complete grammar |
| [docs/codes.md](docs/codes.md) | every diagnostic: when it appears, how to fix it, a runnable reproduction |
| [docs/formats.md](docs/formats.md) | every machine-readable format: `--format json`, vectors, fixtures, the manifest, the adapter protocol |
| [docs/generated-code.md](docs/generated-code.md) | the shape and guarantees of the generated code in each language, and how to call it |
| [docs/backends.md](docs/backends.md) | targeting a language rulec does not generate, without losing the comparison |
| `proofs/` (Lean 4) | what a table means, the checks a certificate has to pass, and the theorems that a `true` from each one settles the matching claim. `lake build` checks them; the program it builds re-checks certificates |
| `DESIGN.md` (Japanese) | why each decision was made and what was rejected |
| `README.md` | the worked example, the excerpts of generated code, and how to run the suite |
| <https://i2y.github.io/rulec/> | the same references as pages, plus the tour and the worked rules written for a person — English, and Japanese under `/ja/` |
