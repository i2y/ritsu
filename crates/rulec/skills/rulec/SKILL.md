---
name: rulec
description: Turn a table-shaped business rule into proved, dependency-free Python and Go with rulec. Use when a shipping tariff, fee schedule, discount or coupon policy, eligibility test, period classification, or any rule that is already written as a table has to become code; when writing, editing or reviewing a `.rule` file; when a rulec diagnostic (E001-E014, E101-E114, W105, W110, W111, W114) has to be fixed; or when a change to such a rule has to be shown to a person before it ships.
compatibility: Requires the `rulec` binary on PATH (https://github.com/i2y/rulec).
license: MIT
---

## When this applies

The rule **is a table, or could be written as one** — a table plus, where it needs it, a
little arithmetic over the inputs. Some are already tables: a tariff, a rate card, a fee
schedule. Others are prose or folklore and become a table once written down: when a discount
applies, whether a return is accepted, which period a date falls in, what rank a set of scores
earns. The source of truth may be a spreadsheet, a published policy, a wiki page, a legacy
implementation, or a person, and the job is to turn it into code someone can approve.

It does **not** apply to workflows with several steps and state, to judgements about a
collection ("any line is refrigerated", "three or more items"), to pattern matching on
strings, or to scoring and optimisation. Flatten collection facts at the boundary and pass
the scalar in; keep iteration in the caller.

Bundled with this skill, read on demand — do not read them all up front:

| | |
|---|---|
| [reference.md](reference.md) | the complete grammar |
| [examples.md](examples.md) | ten complete rules, smallest first, each with what it demonstrates |
| [formats.md](formats.md) | every machine-readable format: `--format json`, vectors, fixtures, the manifest, the adapter protocol |
| [generated-code.md](generated-code.md) | the shape and guarantees of the generated Python and Go, and how to call it |

Every diagnostic is `rulec explain <CODE>`, which is always current, so there is no bundled
copy of the ledger.

---

# Working with rulec

You are the first user of this tool. It exists so that a business rule — a shipping tariff, a
coupon policy, an eligibility test — can be written as one table, **proved** correct before
anyone runs it, and turned into ordinary Python and Go.

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
source.

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

The grammar is [reference.md](reference.md), complete. The shortest useful summary:
declare `inputs` and `outputs` with their units, write one `table` per decision, and let
`examples` state a few cases you know the answer to.

Two declarations are mandatory and are where most first drafts fail:

- **`range` on every numeric input and every `derive`.** It is the universe the completeness
  proof quantifies over, and the entry guard of the generated code.
- **`round` on every numeric output.** Without it the generated code would settle fractions
  silently.

### `rulec fmt <file>`

Run it before `check`, every time. It aligns the columns and rewrites `→ ・ 、 ≦` to ASCII,
so your diffs stay about the rule and not about whitespace.

### `rulec check <file> --format json`

The centre of the loop. One JSON object per finding
([formats.md](formats.md#check)). Read these fields:

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

The full ledger is `rulec explain --all`. The ones you will
meet while transcribing:

- **E101 completeness gap** — some input matches no row. The witness names it. Add a row that
  covers it; if the value should fall through to a catch-all, mark it `default` in the enum
  instead. One row closes the gap the witness names; run again for the next one.
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
- **E114 value off the column’s step** — `0.5%` in a `rate[step 1%]` column has no runtime
  representation. Write a value on the step, or declare a finer step (`rate[step 0.1%]`).
- **W111 unused declaration** — see §3.

### `examples`

Add them as soon as the tables check clean. They are the only thing that can catch an error
the reference evaluator, the generated Python and the generated Go all share — that really
happened once, with rounding for multiple outputs missing in all three. Take the cases from
the source: a published tariff's own worked examples are ideal.

### `rulec gen <file> --out generated/ --format json`

Writes Python, Go, and the vectors. It refuses to generate from a rule that does not pass
check. `rulec api <file>` tells you how to call the result — signatures, parameters with
units and ranges, enum member spellings, errors — so you never have to read the generated
code to integrate it ([generated-code.md](generated-code.md)).

### `rulec test generated/ --format json`

Runs both languages over the vectors and compares them with the reference evaluator, byte for
byte. This is the only step that needs a `python3` and a `go` toolchain.

### In CI

```yaml
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/
```

`--diff-base` reports only findings that are new since that revision, so pre-existing
warnings do not drown the ones this change introduced. `gen --check` fails when the generated
code is stale — that is the gate that keeps the committed output honest.

### If there is a legacy implementation: `rulec verify`

Write a 20-to-30-line adapter (`rulec adapter --template python|go` gives you the shape), and
rulec streams the vectors through it. Mismatches come back clustered by the rows that fired,
with counts, amount differences and a witness. A cluster whose differences are all smaller
than the output's rounding grid is flagged as a suspected rounding difference rather than a
real disagreement.

**A mismatch is not automatically your bug.** It is one of four things: a defect in the legacy
implementation, a transcription error in your table, dirty records, or a rounding convention.
The cluster and its witness are what tell them apart.

### If there are past records: `rulec fixtures lint`, `replay`, `diff`

`fixtures lint` first, always — it reports records whose shape disagrees with the rule
instead of quietly dropping them. Then `replay` compares the rule against what actually
happened, and `diff` compares two versions of the rule over the same records and reports
**how many change and by how much**. That is the number a person needs before approving.

### For the person who approves: `rulec doc`

`rulec doc <file> --lang ja` renders the rule as markdown with the facts the checker knows
that the text does not show — that a group of six values and its complement of 41 really do
cover all 47, which rows shadow which, where a rounding was assumed rather than sourced. It
is rendered in CI and pasted into the PR, never committed: a stale rendering that still looks
authoritative is the danger it is designed against.

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

It has no loops, no recursion, no state, no nested objects in a cell, and no date arithmetic.
Do not look for a way around these. They are the price of the checks terminating: because a
cell is a unary test on its own column, a row is a box, and completeness and overlap are
exactly decidable. Flatten nested data at the boundary; keep iteration in the caller.

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
| E105 between rows 3 and 7 with different outputs | 「この入力（証人）は 1,200 円と 800 円のどちらですか。両方の条件に当たります」 |
| W114 | 「この二つの条件を同時に満たす注文は実在しますか」 |
| A rounding you assumed | 「この丸めは規約に根拠がありません。仮に切り捨てにしています。出典はありますか」 |

Three things make such a question answerable: **a concrete case** (the witness), **what turns
on the answer** (the amount that moves), and **what you assumed in the meantime** so that
silence does not read as agreement.

Keep the assumption in the file, as a `#` comment where the declaration is. `rulec doc`
surfaces those comments to the approver, which is the only place an assumed rounding is seen
by the person who can overrule it.

---

## 7. Where to look

| | |
|---|---|
| [reference.md](reference.md) | the complete grammar |
| [examples.md](examples.md) | ten complete rules that all pass `check`, generate, and agree across three implementations |
| [formats.md](formats.md) | every machine-readable format: `--format json`, vectors, fixtures, the manifest, the adapter protocol |
| [generated-code.md](generated-code.md) | the shape and guarantees of the generated Python and Go, and how to call it |
| `rulec explain <CODE>` | one diagnostic: when it appears, how to fix it, a runnable reproduction. `--all` for every one, `--format json` for data |
| <https://github.com/i2y/rulec> | the source, the design document, and the tour written for a person |
