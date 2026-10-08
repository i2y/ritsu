# rulec machine-readable formats

Every `rulec` command that reports something can report it as JSON with `--format json`.
This file is the definition of those shapes. It is written for a program that drives rulec
without a person in the loop.

Three rules hold everywhere.

- **One JSON object per line.** No wrapping array, no pretty printing. A line is complete in
  itself, so a caller can stream it, `grep` it, or pipe it into `jq -c`.
- **Keys are English and never change with `--lang`.** Prose is confined to fields whose name
  says so (`title`, `notes`, `text`, `hint`, `what`). `--lang` moves the prose and nothing
  else. Numbers, names taken from the rule, and identifiers are the same bytes in both
  languages.
- **A number is an integer in the canonical unit** (§10.2): yen for `money[JPY, …]`, the
  declared unit for a quantity, the number of steps for a rate, `YYYY-MM-DD` as a string for
  a date, `true`/`false` for a boolean, the value's own name for an enum. A number whose value
  is whole is that integer wherever an input is read, written with a point or an exponent
  (`1000.0`, `1e3`) or not (§15.205). Where a value can be finer, the field says so: a match `rate` is a fraction, and the reports of `verify`, `replay`
  and `diff` keep the decimals of a difference or an answer finer than the rule's step.

Exit codes are unchanged by `--format json`: 0 notes only, 1 findings, 2 bad arguments or an
internal failure. **Read the exit code, not the emptiness of the output.**

**Skip a key you do not know.** From 1.0 a release adds fields and never removes one or
changes what one means ([compatibility.md](compatibility.md)). Where an object carries `v`, it
is the version of its shape — `check` is at 2 and the certificate at 1 — and one without `v`
is at 1.

---

## `check`

One object per diagnostic. This is version 2; every field version 1 had is still present and
still means the same thing, and `v` says which version wrote the line.

```json
{"v":2,"severity":"error","code":"E101","file":"rules/parcel_rate.rule","line":30,"column":7,
 "title":"Completeness gap: some input matches no row",
 "notes":["An input that matches no row: dest = overseas, size = small, weight = 1lb","An input producing this example: dest = overseas, girth = 23, signature = true, weight = 1","hint: …",…],
 "where":{"file":"rules/parcel_rate.rule","line":30,"column":7,"table":"base_rate"},
 "spans":[{"line":30,"column":7,"length":9,"label":"the input space is not fully covered"}],
 "witness":{"inputs":{"dest":"overseas","size":"small","weight":1}},
 "rows":[],
 "fix":{"kind":"add_row","text":"| overseas | small | 1lb | 6USD |"},
 "key":"…"}
```

| field | meaning |
|---|---|
| `v` | format version. `2` today |
| `severity` | `error` or `warning` |
| `code` | the stable code. `rulec explain <code>` describes it |
| `file`, `line`, `column` | the primary position. `column` is 1-based |
| `title`, `notes` | **prose** |
| `where` | `file`, `line`, `column`, and `table` / `row` when the finding is about a table row. `row` is 1-based |
| `spans` | every underlined range: `line`, `column`, `length` (bytes), `label` (**prose**) |
| `witness` | an assignment of values that exhibits the finding. `inputs`, `outputs` (what the rule really produces), `expected` (what an example said; E107 only). A finding about a state machine (E107 on a `scenario`, E124–E127) adds `trace`: the calls from the initial state, in order, each `{"inputs":{…},"outputs":{…},"rows":[{"table":…,"row":…}]}` — every one an input the rule takes — and `inputs` and `outputs` are then the last call's. Absent keys mean there is nothing to say, never "empty". A number is the integer it travels as (§10.2); a value the rule computes that falls between those integers — a tenth of a pound — travels on no wire, and is written as the number it is, in a string: `"1.1"` |
| `rows` | every row that takes part: `{"table":…,"row":…}` |
| `fix` | `kind` from the closed set below, and `text`: the rewritten form, ready to paste. Language independent, no prose |
| `key` | the identity `--diff-base` compares on. Two runs that name the same finding use the same key |

`fix.kind` is one of `add_row`, `remove_row`, `add_rounding`, `add_range`, `widen_range`,
`add_alias`, `mark_default`, `mark_contract_only`, `change_policy`, `add_expected`, `pin_source`,
`flip_bound`, `narrow_contract`, `rewrite_literal`, `none`. `rewrite_literal` is one literal as
it has to be written: `1,000JPY` becomes `1000JPY` (E049).
`none` means no single mechanical edit is right; the reason is in `notes`. `narrow_contract` is
the one edit that is not to the `.rule`: its `text` is what to write in the contract the input is
read from — a Protovalidate option, or JSON Schema keywords — as they are written there (E122).

**`fix.text` is a form, not a decision.** It parses and it removes the code, and that much is
tested. It does not know the right amount, the right rounding direction or the right grid —
those are business decisions. The caveat is in `notes`, because `notes` is prose and
`fix.text` is bytes.

## `explain`

`rulec explain <CODE> --format json`, or `--all` for one object per code.

```json
{"code":"E101","severity":"error","title":"…","when":"…","fix":"…",
 "example":"rule t(t) v1\n…","related":["E102","E105"],"lang":"en"}
```

`title`, `when` and `fix` are **prose**; `example` is a `.rule` that really produces the code
(a test runs every one of them). `budget` appears only on E109, whose example needs
`--budget 1` to reproduce. `also` appears only on E102: the other forms the entry reproduces,
each a `what` (prose, the heading `explain` prints over it) and an `example` that really
produces the code in that form.

## `fmt`

One object for the run.

```json
{"unformatted":["rules/member_shipping_fee.rule"],"formatted":[]}
```

`--check` fills `unformatted` and writes nothing; without it, the files rewritten are listed
in `formatted`. Both keys are always present.

## `gen`

One object for the run.

```json
{"written":["generated/python/member_shipping_fee.py"],"stale":[],"missing":[]}
```

`stale` is a file that exists but differs from a fresh generation; `missing` is one that is
not there at all. `--check` writes nothing and fills those two; without it the files actually
written are in `written`. A file already up to date appears in none of the three.

## `coverage`

One object per rule file.

```json
{"file":"rules/member_shipping_fee.rule","vectors":70,"refused":0,
 "criteria":[{"name":"row","satisfied":7,"total":7,"missing":[]},
             {"name":"boundary_pair","satisfied":4,"total":4,"missing":[]},
             {"name":"shadow_pair","satisfied":3,"total":3,"missing":[]},
             {"name":"value_pair","satisfied":1,"total":1,"missing":[]},
             {"name":"rounding_tie","satisfied":0,"total":0,"missing":[]},
             {"name":"fold_transition","satisfied":0,"total":0,"missing":[]},
             {"name":"machine_transition","satisfied":0,"total":0,"missing":[]}]}
```

`name` is one of `row`, `boundary_pair`, `shadow_pair`, `value_pair`, `rounding_tie`,
`fold_transition`, `machine_transition`. Every obligation is read off the rule, never off what
the suite happened to reach. `value_pair` has one for each row that returns a computed value —
a name in its output cell, unless the row itself pins down everything that name is computed
from, or the value can only be one value there (an output that rounds to the same amount
whatever comes in) — and for each output a `define` or a `result` line computes: two vectors on
the row, one input apart, with the value moved as `test` and `verify` compare it (an output
after rounding) (§15.151, §15.152). `rounding_tie` has one for each output that declares a
rounding, unless the rule shows that no input takes the output half a step off its grid. Both
are shown by the rule's arithmetic, where each row wins — its own cells, including those on
derived columns and on boolean definitions that compare a number, the rows before it under
`policy first`, the `constraint`s — and, where that cannot say and few enough inputs are
involved, by evaluating every one of them (§15.153, §15.154).
`fold_transition` has obligations only for a rule that walks a sequence
(§15.56); `machine_transition` only for a rule with a `machine` (§15.148): every transition a
case can make, and every two that can follow one another, each met by a trace of the suite that
makes it. An entry of `missing` is
`{"what": …, "hint": …}`, both **prose**. `refused` counts the cases in the suite that the
reference evaluator has **no answer** for; they discharge obligations like any other case, and
what is asked of the generated code there is that it refuse them too (below). The inputs the
door refuses, which `gen` writes beside them (§15.204), discharge none and are not counted here.

## `test`

One object for the run.

```json
{"results":[{"rule":"member_shipping_fee","lang":"python","via":"runner","vectors":70,"refused":16,
              "ok":true,"ran":true,"first_diff":null,"error":null},
             {"rule":"member_shipping_fee","lang":"python","via":"mcp","vectors":70,"refused":16,
              "ok":true,"ran":true,"first_diff":null,"error":null},
             {"rule":"member_shipping_fee","lang":"go","via":"runner","vectors":70,"refused":16,
              "ok":false,"ran":false,"first_diff":null,
              "error":"does not compile:\n…"}],
 "skipped":[]}
```

| field | meaning |
|---|---|
| `via` | how the generated code was reached: `runner`, the vectors piped through the generated runner; `mcp`, one `tools/call` per vector through the generated server over stdio; `mcp-http`, the same conversation over the same server's Streamable HTTP ([generated-code.md](generated-code.md#the-rule-as-an-mcp-tool)); `connect-asgi`, `connect-asgi-get`, `connect-wsgi` and `connect-wsgi-get`, one call per vector through the generated Connect service — the ASGI application and the WSGI one, each by POST and by GET ([generated-code.md](generated-code.md#the-rule-as-a-connect-service)); `wasi`, the Rust runner compiled for `wasm32-wasip1` and run under wasmtime ([generated-code.md](generated-code.md#the-rust-runner-as-a-wasi-module)); `function`, the rule as a function on a real PostgreSQL, called once per vector by argument name through `psql` ([generated-code.md](generated-code.md#sql)); or `proof`, the generated Rust read by a model checker ([generated-code.md](generated-code.md#the-proofs)) — the one way that is not the vectors, so its `vectors` is the number of harnesses and its `refused` is 0; the `wasm/` target itself is a language of its own in this list, reached through its runner, so `wasm` names a language here and `wasi` a way of reaching one |
| `vectors` | how many vectors were put to it — or, when `via` is `proof`, how many harnesses the checker read |
| `refused` | how many of the inputs the reference evaluator refuses were put to it, all in one run: each has to be answered with a refusal of the same kind (`input` or `contradiction`) and the same sentence the evaluator gives, in the place of the record (§15.204) |
| `calls` | how many calls a machine's traces made (§15.148): each one passed the state the language under test answered to the call before. 0 for a rule without a `machine` |
| `ok` | the generated code and the reference evaluator agreed on every vector, and refused every input the evaluator refuses, for the reason it gives |
| `ran` | whether the generated code ran far enough to be compared **at all** |
| `first_diff` | `null`, or `{"line":12,"generated":"…","expected":"…"}` — the first line of the canonical JSON they disagreed on |
| `error` | `null`, or **prose** for a failure with no single line to point at |
| `skipped` | **prose** reasons a language was not run at all (no toolchain) |

**`ran` is the field to branch on.** `ok:false` with `ran:false` is a machine that could not
build or start the code — a missing toolchain, a compile error, a process that died — and says
nothing about the rule. `ok:false` with `ran:true` is a real disagreement. The text rendering
splits them the same way: `disagrees with the reference evaluator` against `could not be run`.

## `verify`, `replay`, `diff`

The same shape for all three: they differ only in what the rule is compared against.

```json
{"compared":96,"matched":79,"rate":0.82292,"counterpart":"legacy@fake-1","unanswered":0,
 "clusters":[{"rows":[{"table":"size_of","row":1},{"table":"base_rate","row":6},
                      {"table":"fuel_rate","row":2},{"table":"signature_fee","row":1}],
              "count":10,
              "delta":{"fee":{"min":-1,"max":-1,"uniform":true,"total":-10}},
              "witness":{"in":{"dest":"overseas","girth":1,"signature":true,"weight":1},
                         "ours":{"fee":22},"theirs":{"fee":23}},
              "records":[{"line":10,"tag":""},{"line":65,"tag":""},{"line":66,"tag":""},…],
              "suspect_rounding":false},…],
 "moved":[],"refused":[],"excluded":{},"out_of_reach":{},"filled":{"count":0,"by_field":{},"defaults":{}},"cases":null}
```

| field | meaning |
|---|---|
| `compared` | observed records compared. **Filled records are not counted here** (§10.3) |
| `matched` | of those, how many agreed |
| `rate` | `matched / (compared − unanswered)`, as a fraction |
| `counterpart` | who the rule was compared against: an adapter's self-reported id, a fixtures path, or `rules/parcel_rate.rule@v3 → rules/parcel_rate.rule@v4` |
| `unanswered` | records the counterpart declared it could not answer. Excluded from the denominator |
| `clusters` | mismatches grouped by the rows that matched |
| `clusters[].records` | **every** record in the cluster, in the order they came in — `line` is the record's line in the fixtures file (for `verify`, the vector's place in the stream) and `tag` is its label, empty when it has none. `witness` shows one of them; this names them all, which is what something that has to act on the records needs. `count` is its length |
| `moved` | `replay` only: records whose values matched but whose recorded rows differ from the rule's, in the same shape as `clusters` with an empty `delta`. Only a record carrying a `trace` can appear here. They count as matched |
| `refused` | `diff` only (empty for the other two): records whose input the old version takes and the new one does not, grouped by the reason — `{"kind":…,"field":…,"count":…,"witness":{"in":{…},"theirs":{…}},"records":[{"line":…,"tag":…},…],"what":…}`. They count as compared and never as matched, and the run exits 1 when there are any. `kind` is one of `input_range` (outside the new version's range), `enum_value` (a value its enum does not have), `input_type` (`null` for an input that is not optional, or a type the value does not carry into), `input_step` (no whole number of its step or unit), `input_day` (not a day its koyomi date comes to), `constraint` (a `constraint` that does not hold); `field` is the input at fault, absent for a constraint. `witness.in` is the record's input as the record has it and `theirs` the old version's answer; the new version gives none. `what` is **prose**, without the record's own value except an enum's |
| `excluded` | records dropped before comparison, keyed by a stable reason: `missing_field`, `bad_format`. `bad_format` is a record whose form is wrong — a type, an enum's value, an input outside its declared range — and never an answer the rule does not give (`out_of_reach`). For `diff`, `bad_format` is a record the **old** version does not read: `diff` reads the records the way the version that wrote them reads them |
| `out_of_reach` | `verify` and `replay` (empty for `diff`): per output, how many records the counterpart answered with a value outside what the rule can produce, `{"fee":3}`. They are compared and count as mismatches — the counterpart and the rule disagree there — and are never excluded. For records, the other reading is a record written before a step or a unit changed, which `--read-as` reads ([Fixtures](#fixtures-rulec-fixtures-lint-replay-diff)) |
| `filled` | `count`, `by_field` (field → how many records were filled), `defaults` (field → the value used). §10.3 requires the report to carry this |
| `cases` | `replay` and `diff` of a rule with a `machine` (§15.148), when the records carry a `tag`; `null` otherwise. The records that share a tag are **one case's calls**, in the order they came in, and each case is played again from its first record with the version carrying its own answer from one call to the next: `total`, `followed` (every call answered as before — the record's answer for `replay`, the old version's for `diff`), `diverged` and `refused` (`[{"tag":…,"line":…}]`, the first call where the answer parts, or that the version refuses — a state it no longer has, a record that changes a `held` input, which one case does not do, or for `diff` an input the new version does not take, as in `refused` above), `stranded` (`[{"tag":…,"state":…}]`, left where the version reaches no final state), `ended` (how many end in a final state) |

A cluster's `rows` entry is `{"table":…,"row":…}` for `verify` and for `replay` over records
without a `trace`; for `diff` it is `{"table":…,"from":…,"to":…}`, the transition of the row
that matched between the two versions, and `replay` uses the same transition for a record
that carries a `trace` — `from` is the recorded row, `to` the rule's. A row that exists on one
side only has `from` or `to` set to `null`.

`diff` reads the records the way the **old** version reads them — its names, types, steps,
units, ranges and enums — because the old version's system wrote them, and it reads only
their inputs: `observed` and `trace` play no part in comparing two versions, and are neither
required nor checked (run `fixtures lint` against the old version for that). Each input is
then brought to the new version's type and held to what the new version's generated code
takes at its entry; a record it does not take is reported under `refused` instead of being
dropped (DESIGN §15.197).

A record is named by `line` and `tag` together, the same pair `fixtures lint` uses, because
a record need not carry a tag: the line always finds it. Nothing else in this report names a
record, so `--terse` — which keeps production values out of a pull-request comment — does not
apply here; it cannot be combined with `--format json` at all.

`delta` is keyed by output name, because a rule can have several outputs and they move
independently. `uniform` is true when every record in the cluster moved by the same amount,
which is what folds a cluster into one line in the text rendering.

**Two answers are compared as the values they are, never at either side's step** (DESIGN
§15.199): an amount in another unit of its dimension is the same amount (yen and sen), and an
answer a coarser step rounds is a different one (12.3% is not 12%). A difference is the rule's
answer less the counterpart's, in the rule's wire unit — for `diff`, the new version's; for a
rate, its steps — and exact: a whole number when both answers sit on the rule's step, a decimal
where the counterpart's is finer (`-0.3` for a version at 1% that answers 12% where the old one
answered 12.3%, `-0.5` for a recorded `820.5`), and in the rare case no decimal writes it — 6℃ against an old
version's 42℉ — the fraction as a string (`"4/9"`). `min`, `max` and `total` are
written the same way.

`witness.ours` is the rule's answer as its wire writes it. `witness.theirs` is the
counterpart's as its own side writes it: for `diff` the old version's wire, its step and its
unit (as `old` in [`diff` with no records](#diff-with-no-records-the-region)); for `replay` the
recorded value in the rule's wire unit, with its decimals if it has them (`123.4` for 12.34% at
a step of 0.1%); for `verify` the adapter's answer as it came. The text and Markdown renderings
write the two answers as a person reads them, unrounded, and put the unit beside each when the
two versions count in different units.

`suspect_rounding` is true when **every** differing record in the cluster differs by less
than the output's own rounding grid, compared exactly — a difference in rounding convention
rather than in the values (§10.4). It is a conjunction over the whole cluster, so values that really differ are
never blamed on rounding.

## `diff` with no records: the region

`rulec diff <old> <new>` **without** `--fixtures` answers a different question with a
different shape: not "how many of these records moved" but "which inputs get a different
answer, and is there anything outside them" (DESIGN §15.122).

```json
{"rule":"parcel_rate","old":"rules/parcel_rate.rule@v3","new":"rules/parcel_rate.rule@v4",
 "old_version":"3","new_version":"4",
 "over_budget":false,"total":true,
 "cells":210,"feasible":210,"same":194,"differing":16,"unsettled":0,"unrealized":0,
 "domain":[],
 "changes":[{"region":[{"column":"signature","kind":"input","accepts":["false"],
                        "text":"signature = false"},
                       {"column":"dest","kind":"input","accepts":["domestic","canada"],
                        "text":"dest = north_america"},
                       {"column":"weight","kind":"input",
                        "accepts":[{"from":"11","to":"69"},{"from":"70","to":"70"}],
                        "text":"weight >=11lb <=70lb"},
                       {"column":"girth","kind":"input",
                        "accepts":[{"from":"61","to":"129"},{"from":"130","to":"130"}],
                        "text":"girth >=61in <=130in"}],
             "text":"signature = false  and  dest = north_america  and  weight >=11lb <=70lb  and  girth >=61in <=130in",
             "outputs":[{"output":"fee","old":"32","new":"33"}],
             "uniform":true,
             "old_rows":[{"table":"size_of","row":3},{"table":"base_rate","row":5},
                         {"table":"fuel_rate","row":1},{"table":"signature_fee","row":2}],
             "new_rows":[{"table":"size_of","row":3},{"table":"base_rate","row":5},
                         {"table":"fuel_rate","row":1},{"table":"signature_fee","row":2}],
             "witness":[{"input":"dest","value":"domestic"},{"input":"girth","value":"61"},
                        {"input":"signature","value":"false"},{"input":"weight","value":"11"}],
             "cells":8},
            …],
 "unknown":[],"machine":null}
```

| field | meaning |
|---|---|
| `total` | **whether "outside the reported regions the two versions answer alike" is a claim this run earned.** False when the space was over the budget, when the rule folds a sequence, or when anything landed in `unknown` |
| `cells` | cells of the common refinement of the two versions' axes |
| `feasible` | of those, how many an input could be built for. The rest are combinations of coordinates no caller can send |
| `same` / `differing` / `unsettled` | settled alike / settled apart / neither |
| `unrealized` | cells that were not shown to be impossible and that no input could be built for. They are also listed in `unknown`. A cell the arithmetic proves no input reaches is not one of these: it is passed over like a cell the shape ruled out |
| `domain` | what the rule **accepts**, where that changed: `{"what":…,"name":…,"old":…,"new":…}`. `what` is one of `input_added`, `input_removed`, `input_type`, `input_range`, `enum_added`, `enum_removed`, `enum_value_added`, `enum_value_removed`, `output_added`, `output_removed`, `output_rounding`, `output_type`, `output_step`. A wider door is not a different answer, so it is reported apart from `changes`. `output_type` (`money[USD]` to `money[USDc]`) and `output_step` (a rate's step, `0.1%` to `1%`) are what callers of the generated code receive changing: answers are compared as values, so a version that only counts an output in another unit or at another step moves no answer, and is told here |
| `changes` | the regions where the two answer differently |
| `unknown` | the regions that could not be settled, each with `why` |
| `blocked` | present only when the two cannot be compared cell by cell at all, with the reason. A rule that folds a sequence is the case that exists today: its answer depends on the whole sequence, so it is not a function of finitely many columns |
| `machine` | when both versions are one step of a state machine carrying the same input back as the same output (§15.148), what the change does to **sequences of calls**; `null` otherwise. `carry` (`input`, `output`); `initial` (`old`, `new`); `shortest`, the shortest sequence of calls the two answer differently — every call but the last answered alike, each `{"inputs":{…},"old":{…},"new":{…}}` in the wire form, the `held` inputs the same on every call, empty when no case can meet the change; `unreached`, how many regions of `changes` lie in states no case reaches under the old version; `migration`, what the new version does to a case **in progress**, `[{"state":…,"kind":…}]` for a state the old version can reach, `kind` one of `removed` (the new version has no such state, and refuses it at the door), `stranded` (no final state can be reached from it any more), `no_longer_final`, `now_final` |

A region is a **box over the rule's columns**, one entry per column that says anything; a
column the region leaves alone is absent. `kind` is `input`, `derived` or `walk` — a point
on an input is a value a caller sends, one on a derived column or a walk's summary is a
value those columns take, which is weaker and says so. `accepts` holds the coordinates: a
word for an enum, a boolean or a string class, and `{"from":…,"to":…}` (either end absent
for unbounded) for a number or a date, as **closed** intervals of true values. `text` is the
same thing worded in the rule's own notation, in the language `--lang` selects; everything
else is fixed in English.

`outputs` is the transition **at the witness**, and `uniform` says whether every cell of
the box moves that way. `old` and `new` are each version's own wire integers; whether an
answer moved is decided by value, as with records, so a version that only changes a step or a
unit moves nothing (DESIGN §15.199). The text rendering writes the two answers as a person reads
them — `rate: 12.3% → 12%`, with the unit beside each when the two versions count in different
units. It is false where the amounts come out of an expression rather
than off the row, and then the numbers are the witness's own and not the box's.

`old_rows` and `new_rows` are the rows that fired, in the shape a fixture's `trace` uses.
`cells` on a change is how many cells of the refinement actually differ inside that box —
a box may be widened over cells no input reaches, which is what keeps a column that decides
nothing out of the description.

The two answers are meant to be held against each other: every record whose answer moves
under `--fixtures` falls inside one of these regions. `tests/vdiff.rs` does exactly that
over the corpus.

## `fixtures lint`

One object for the run.

```json
{"file":"replay/2025-08.jsonl","records":24,"observed":24,"filled":0,"dropped":0,
 "problems":[{"kind":"bad_input","field":"dest","count":1,
              "example":{"line":25,"tag":"order:b3"},
              "what":"`in.dest`: `mars` is not a value of enum zone",
              "hint":"The type or range disagrees with the declaration."}],
 "out_of_reach":[]}
```

`kind` is one of `not_json`, `no_in`, `no_observed`, `unknown_field`, `bad_input`,
`bad_observed`, `missing_observed`, `bad_trace`. `field` is the field of the record at fault, or absent
when the problem is about the record as a whole — or, for a `bad_input` that is a `constraint`
not holding, about two of its inputs. `what` and `hint` are **prose**; `kind`,
`field`, `count` and `example` are not.

The problems are exactly the records `replay` leaves out for their form: what `lint` passes,
`replay` compares, and the exit code depends on `problems` alone. `bad_observed` is an observed
value of the wrong form — a type, an enum's value, something that is not a number. A value
outside what the rule can produce is not one of them: `replay` compares it and counts the
mismatch. `out_of_reach` lists those values apart, by output, in the shape of a problem
(`field`, `count`, `example`, `what`, `hint`), because a record written before a step or a unit
changed looks the same and `--read-as` reads it (DESIGN §15.199).

## `api`

One object. It says how to call the generated code without reading it; the field-by-field
meaning is in [generated-code.md](generated-code.md).

**`preconditions` is the half `schema` cannot carry.** A caller that validates its input
against `rulec schema` has done everything JSON Schema can express and is still not done:
three things the generated code refuses at the door are not shapes at all. Each entry names
one, and a caller reads them by `kind` rather than by prose — the sentence the refusal
carries moves with `--lang`, these do not.

| `kind` | fields | what the generated code refuses |
|---|---|---|
| `constraint` | `left`, `op`, `right` | a declared relation between two inputs that does not hold (§15.55) |
| `sum` | `name`, `over`, `of`, `max` | a sequence whose total over that column passes `max` (§15.100) |
| `length` | `sequence`, `max` | a sequence with more than `max` elements (§15.58) |

The list is empty where the rule has none, and never absent: a caller has to be able to tell
"there are none" from "this tool does not say". Where the caller is a step of a workflow
that fetched the sequence, checking these before returning it is the difference between a
value refused at its own boundary and one refused after it is recorded (§15.116).

```json
{"rule":"single_coupon","alias":"single_coupon","version":"1","source_sha256":"…",
 "python":{"module":"single_coupon","mcp":"single_coupon_mcp.py","page":"single_coupon_page.html","function":"single_coupon",
           "signature":"def single_coupon(subtotal: JPYInclTax, …) -> Output:",
           "traced":"single_coupon_traced",
           "traced_signature":"def single_coupon_traced(subtotal: JPYInclTax, …) -> tuple[Output, list[Fired]]:",
           "record":"single_coupon_record",
           "record_signature":"def single_coupon_record(subtotal: JPYInclTax, …, out: Output, trace: _Trace, tag: str = \"\") -> str:",
           "params":[{"name":"subtotal","alias":"subtotal","type":"JPYInclTax","unit":"JPY",
                      "range":{"min":0,"max":1000000},"optional":false},…],
           "returns":"Output",
           "outputs":[{"name":"ok","alias":"ok","type":"bool","optional":false},
                      {"name":"raw","alias":"raw","type":"JPYInclTax","unit":"JPY",
                       "optional":false,"rounding":{"mode":"down","grid":1}}],
           "enums":[{"name":"coupon_kind","alias":"CouponKind",
                     "values":[{"name":"percent","alias":"PERCENT"},…]}],
           "errors":["RuleInputError","RuleContradictionError"],
           "error_types":[{"kind":"input","name":"RuleInputError","fields":["what","value"]},
                          {"kind":"contradiction","name":"RuleContradictionError","fields":["what"]}]},
 "typescript":{"module":"single_coupon.ts","mcp":"single_coupon_mcp.ts","page":"single_coupon_page.html","function":"single_coupon",
               "signature":"export function single_coupon(subtotal: JPYInclTax, …): Output",
               "params":[…],"returns":"Output","outputs":[…],
               "enums":[{"name":"coupon_kind","alias":"CouponKind",
                         "values":[{"name":"percent","alias":"PERCENT"},…]}],
               "errors":["RuleInputError","RuleContradictionError"]},
 "javascript":{"module":"single_coupon.mjs","mcp":"single_coupon_mcp.mjs","page":"single_coupon_page.html","function":"single_coupon",
               "signature":"export function single_coupon(subtotal, applied, kind, rate, face, dup)",
               "params":[…],"returns":"Output","outputs":[…],"enums":[…],
               "errors":["RuleInputError","RuleContradictionError"]},
 "rust":{"module":"single_coupon.rs","function":"single_coupon",
         "signature":"pub fn single_coupon(subtotal: JPYInclTax, …) -> Result<Output, RuleError>",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"coupon_kind","alias":"CouponKind",
                   "values":[{"name":"percent","alias":"Percent"},…]}],
         "errors":["RuleError::Input","RuleError::Contradiction"],
         "proof":"single_coupon_proof.rs","harnesses":["single_coupon_answers","applicable_rows","raw_discount_rows"]},
 "ruby":{"module":"SingleCoupon","function":"single_coupon",
         "signature":"SingleCoupon.single_coupon(subtotal, applied, …)",
         "rbs":"sig/single_coupon.rbs",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"coupon_kind","alias":"CouponKind",
                   "values":[{"name":"percent","alias":"PERCENT"},…]}],
         "errors":["RuleInputError","RuleContradictionError"]},
 "php":{"module":"single_coupon.php","namespace":"SingleCoupon","function":"single_coupon",
        "signature":"function single_coupon(int $subtotal, int $applied, …): Output",
        "params":[…],"returns":"Output","outputs":[…],
        "enums":[{"name":"coupon_kind","alias":"CouponKind",
                  "values":[{"name":"percent","alias":"CouponKind::PERCENT"},…]}],
        "errors":["RuleInputError","RuleContradictionError"]},
 "go":{"package":"singlecoupon","func":"SingleCoupon",
       "signature":"func SingleCoupon(in Input) (Output, error)",
       "input_type":"Input","input_fields":[…],
       "output_type":"Output","output_fields":[…],
       "enums":[{"name":"coupon_kind","alias":"CouponKind",
                 "values":[{"name":"percent","alias":"CouponKindPercent"},…]}],
       "errors":["*RuleInputError","*RuleContradictionError"],
       "error_types":[{"kind":"input","name":"*RuleInputError","fields":["What","Value","HasValue"]},
                      {"kind":"contradiction","name":"*RuleContradictionError","fields":["What"]}]},
 "swift":{"module":"single_coupon.swift","function":"singleCoupon",
          "signature":"func singleCoupon(subtotal: JPYInclTax, …) throws -> Output",
          "params":[…],"returns":"Output","outputs":[…],
          "enums":[{"name":"coupon_kind","alias":"CouponKind",
                    "values":[{"name":"percent","alias":"percent"},…]}],
          "errors":["RuleError.input","RuleError.contradiction"]},
 "java":{"module":"SingleCoupon.java","class":"SingleCoupon","function":"singleCoupon",
         "signature":"public static Output singleCoupon(long subtotal, long applied, …)",
         "build":"javac --release 17 -encoding UTF-8 -d classes *.java",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"coupon_kind","alias":"CouponKind",
                   "values":[{"name":"percent","alias":"CouponKind.PERCENT"},…]}],
         "errors":["RuleInputError","RuleContradictionError"]},
 "numpy":{"plan":"single_coupon.json","runtime":"rulec_np.py",
          "load":"rule = rulec_np.load(\"single_coupon.json\")",
          "call":"rule(**{column: sequence}) -> {output: ndarray}",
          "traced":"rule.traced(**{column: sequence}) -> (outputs, fired)",
          "wire":"One equal-length sequence per column. …",
          "columns":[{"name":"subtotal","alias":"subtotal","type":"ndarray[int64]","unit":"JPY",
                      "range":{"min":0,"max":1000000},"optional":false},…],
          "outputs":[…],
          "errors":["RuleInputError","RuleContradictionError"],
          "error_types":[{"kind":"input","name":"RuleInputError","fields":["what","value","row","word"]},
                         {"kind":"contradiction","name":"RuleContradictionError","fields":["what","row","word"]}],
          "needs":["numpy"]},
 "connect":{"proto":"proto/rulec/single_coupon/v1/single_coupon.proto","package":"rulec.single_coupon.v1",
            "service":"SingleCouponService","method":"Decide",
            "path":"/rulec.single_coupon.v1.SingleCouponService/Decide",
            "request":"DecideRequest","response":"DecideResponse",
            "idempotency_level":"NO_SIDE_EFFECTS","trace":"trace",
            "source_header":"rulec-source-sha256",
            "stubs":"cd proto && buf generate",
            "buf_yaml":"proto/buf.yaml","buf_gen_yaml":"proto/buf.gen.yaml","deps":[],
            "json_names":"lowerCamelCase","json_int64":"string",
            "request_fields":[{"name":"subtotal","field":"subtotal","type":"int64","optional":false},
                              {"name":"applied","field":"applied","type":"int64","optional":false},
                              {"name":"kind","field":"kind","type":"CouponKind","optional":false,
                               "enum":"coupon_kind"},…],
            "element_fields":null,
            "response_fields":[{"name":"ok","field":"ok","type":"bool","optional":false},
                               {"name":"raw","field":"raw","type":"int64","optional":false}],
            "enums":[{"name":"coupon_kind","alias":"CouponKind","contract":null,
                      "unset":"COUPON_KIND_UNSPECIFIED",
                      "values":[{"name":"percent","alias":"COUPON_KIND_PERCENT","number":1},…]}],
            "python":{"module":"single_coupon_service.py",
                      "class":"SingleCoupon","sync_class":"SingleCouponSync",
                      "asgi":"app","wsgi":"wsgi_app",
                      "serve_asgi":"uvicorn single_coupon_service:app --port 8080",
                      "serve_wsgi":"gunicorn 'single_coupon_service:wsgi_app'",
                      "client":"SingleCouponServiceClientSync",
                      "runner":"single_coupon_connect_runner.py",
                      "needs":["connectrpc","buf"]}},
 "sql":{"file":"single_coupon.sql","input":"single_coupon_input","id":"_id","guard":"_input_error",
        "dialect":"postgresql","runs_on":["postgresql","sqlite"],
        "columns":[{"name":"subtotal","alias":"subtotal","type":"bigint","unit":"JPY",
                    "range":{"min":0,"max":1000000},"optional":false},…],
        "outputs":[…],"rows":[{"table":"applicable","column":"applicable_row"},
                              {"table":"raw_discount","column":"raw_discount_row"}],
        "function":{"file":"single_coupon_function.sql","name":"single_coupon",
                    "signature":"\"single_coupon\"(\"subtotal\" bigint, …, \"dup\" boolean) RETURNS TABLE (\"ok\" boolean, …, \"raw_discount_row\" int)",
                    "language":"plpgsql","runs_on":["postgresql"],"raises":"22023"}},
 "wasm":{"source":"single_coupon_wasm.rs","module":"single_coupon.wasm",
         "build":"rustc --edition 2021 -C opt-level=s -C lto -C panic=abort -C strip=symbols --target wasm32-unknown-unknown --crate-type cdylib single_coupon_wasm.rs -o single_coupon.wasm",
         "wit":"single_coupon.wit","package":"rulec:single-coupon@1.0.0","world":"single-coupon",
         "call":"call","call_signature":"call: func(input: string) -> string",
         "post_return":"cabi_post_call","realloc":"cabi_realloc","memory":"memory",
         "runner":"single_coupon_runner.mjs",
         "component":"wasm-tools component embed single_coupon.wit single_coupon.wasm -o single_coupon.embedded.wasm && wasm-tools component new single_coupon.embedded.wasm -o single_coupon.component.wasm",
         "answers":[{"kind":"record","keys":["in","observed","trace"]},
                    {"kind":"refusal","keys":["refused","error"],"refused":["input","contradiction"]}]}}
```

Everything here is a name or a number the generated code really uses, so nothing in it moves
with `--lang`.

**`errors` and `error_types`** (DESIGN §15.202). The entry of every language but SQL and Wasm —
the languages whose code raises or returns errors of its own, NumPy among them — names its two
errors under `errors`, the input error first, and describes them under `error_types`. `kind` is `input` for an input the entry
guard refuses (a value of the wrong type, not a member of its enum, outside its range, a
`constraint` that does not hold, a day its koyomi date does not come to, a break of one of the
`preconditions`) and `contradiction` for the W114 guard, which is never the caller's fault.
`name` is the name to catch or to match, spelled as `errors` spells it: `RuleError::Input` in
Rust, `RuleError.input` in Swift, `*RuleInputError` in Go, where `errors.As` tells the two
apart. `fields` are what a caller reads from it. `what` is the sentence, in the language the
code was generated in, and `value` the value refused; where the refusal is about no one value
— a `constraint` between two inputs, an input that is missing — `value` holds the language's
own mark of none: `_NOVALUE` in the Python module and in `rulec_np`, the exported `NO_VALUE`
in TypeScript and JavaScript, `RuleInputError::NO_VALUE` in Ruby, `null` in PHP and Java,
`None` in Rust and `nil` in Swift, and Go says it with `HasValue`. NumPy adds `row`, the place
of the element refused (`None` when a whole column is refused: missing, or of another length),
and `word`, the word its sentence calls a place by (`row`, `行`). The `connect`, `sql` and
`wasm` entries have no errors of their own and neither key: a Connect call fails with
`invalid_argument` or `internal`, the SQL query answers with its `guard` column and the
function raises the SQLSTATE under `raises`, and the Wasm module answers with the refusal
line the runners print, `{"refused":…,"error":…}` (§15.204), which its entry describes under
`answers` (below). Every language's entry carries `traced` and `traced_signature` as the Python
one does — the twin that returns the rows that matched beside the outputs — and `record`
and `record_signature`, the function that writes one call as a fixtures record
([generated-code.md](generated-code.md#the-rows-that-matched)). `range` states the bounds **the entry guard enforces**, and `alias` states the
member spelling **that language** uses (`CouponKind.PERCENT` in Python and TypeScript,
`CouponKind.PERCENT` in JavaScript too, `CouponKind::Percent` in Rust, `CouponKind::PERCENT` in Ruby
and in PHP, `CouponKind.PERCENT` in Java, `singlecoupon.CouponKindPercent` in Go,
`CouponKind.percent` in Swift; SQL spells no member, an
enum being its own name there, and the Wasm module and the NumPy plan read and write the name itself, as the wire does). `unit`, `range` and `rounding` are absent when the
type has none. `range` and `rounding.grid` are integers the way the value itself travels, so
for a rate they count its steps: a rate stored in tenths of a percent with `round
half_up(0.1%)` says `"grid":1`. The NumPy entry is shaped differently from the rest, because it names no
function: it carries `plan` and `runtime`, the two files, `load`, `call` and `traced`, and
`columns` and `outputs` in place of parameters, beside `errors` and `error_types` as the languages' entries have them. The Ruby entry also carries `rbs`, the path of the signature file that ships
with the module, and an entry whose language gets a server carries `mcp`, the file beside the
module that serves the rule as one MCP tool
([generated-code.md](generated-code.md#the-rule-as-an-mcp-tool)). A rule that walks a
sequence (§15.56) has one more parameter, last in the list and last in every signature, with
the fields one element carries under `elements`:

```json
{"name":"freight_rows","alias":"freight_rows","type":"list[Element]","optional":false,
 "elements":[{"name":"row_zone","alias":"row_zone","type":"Zone","optional":false},
             {"name":"threshold","alias":"threshold","type":"JPYInclTax","unit":"JPY",
              "range":{"min":0,"max":1000000},"optional":false},
             {"name":"row_fee","alias":"row_fee","type":"JPYInclTax","unit":"JPY",
              "range":{"min":0,"max":100000},"optional":false}]}
```

Those fields get the same entry guards the inputs get, so their `range` means what a
parameter's `range` means. SQL has no entry for such a rule — `rulec gen` does not write one
(§15.56). The `sql` entry describes the relation first: the file, the relation the query
reads (`input`) and its `id` column, the `guard` column that carries the entry guard's
sentence, the `columns` of that relation as the query declares them, the `outputs`, and under
`rows` the column that carries each table's matched row. Under `function` is the other door —
the file that declares it, the name to call, the `signature` as it is written, the language it
is written in, and `raises`, the SQLSTATE it raises with when an input is outside the
declaration. Its arguments are `columns` in order and what it returns is `outputs` followed by
`rows`, so neither list is written out twice
([generated-code.md](generated-code.md#sql)). The `wasm` entry
names no function in a language either: it gives the source and the module it builds into
(`build` is the whole command), the `.wit` with its `package` and `world`, the exports a host
calls (`call`, `post_return`, `realloc`) and the `memory`, the `runner` that `rulec test`
drives, and the `component` line that wraps the module for the component model
([generated-code.md](generated-code.md#wasm)).

**`answers`** (DESIGN §15.206). The module raises nothing, so what `call` answers is the one way
it has of saying a refusal, and the `wasm` entry lists the two shapes in that order: the record,
`kind` `record`, with the `keys` every language's record function writes; and in its place the
line of a refusal, `kind` `refusal`, with its `keys` and the values `refused` takes — `input` for
an input the door refuses and `contradiction` for the W114 guard, the `kind`s of `error_types`.
`error` is the sentence, with the value after it where the module holds one as a number. The
`wire` of the Rust entry's `wasi` says the same line for the WASI runner.

The `connect` entry is the wire a caller writes by hand, without the stubs: the method's
`path`, the two message names, and under `request_fields` and `response_fields` each field by
the rule's own name (`name`), the name the message has it under (`field`, which the JSON writes
in `json_names` form) and its `type` there. `optional` is the rule's `T?` — a value that may be
absent — and not the `.proto`'s label, which every field a caller sends carries so that a
field left out is refused rather than read as zero
([generated-code.md](generated-code.md#the-rule-as-a-connect-service)). A field whose type is
an enum names the rule's enum under `enum`, which is what `enums` is keyed by: one contract's
enum may be imported into two of the rule's, and the type alone would not say which. A rule
that walks a sequence describes the fields of one element under `element_fields`, in the same
shape, and has `null` there otherwise. `enums` lists every enum that crosses the wire, each
value with the name the wire spells it by (`alias`) and its `number`. For the rule's own enum
the values are numbered from 1 and `unset` names value 0, which means "not set" and is
refused; `contract` is `null`. For one imported from a `.proto` (`import proto`), names and
numbers are the contract's, `contract` gives the file as the rule cites it and where `gen` put
the copy the module imports (`proto`), and `unset` is `null` when the contract's value 0 is a
value of the rule's — it is then the value numbered 0, which is also what an answer that
leaves the field out means. `deps` names the BSR modules the rule's contracts import files
from — protovalidate's, say. With any, the module's `buf.yaml` declares them, a `buf.lock` pins
them, and the stubs have to include the files they import, as the generated `buf.gen.yaml`
does with `include_imports`.

A rule that is one step of a state machine (§15.148) carries `machine` at the top level, and
`null` there otherwise:

```json
"machine":{"name":"order","over":"step",
           "carry":{"input":"state","output":"next_state","enum":"order_state"},
           "held":["amount_paid"],
           "states":["received","paid","shipped","delivered","cancelled"],
           "initial":"received","final":["delivered","cancelled"],
           "never":[{"states":["shipped"],"after":["cancelled"]}],
           "once":[{"output":"refund","cell":">0JPY"}],
           "constants":{"python":{"initial":"INITIAL","final":"FINAL","is_final":"is_final"},
                        "php":{"initial":"INITIAL","final":"FINAL_STATES","is_final":"is_final"},
                        "swift":{"initial":"initialState","final":"finalStates","is_final":"isFinal"},…}}
```

`carry` names the input a caller hands back and the output it takes it from, both of the enum
`enum`; `held`, the inputs a case passes with the same value on every call (§15.149);
`states` are that enum's values in declaration order, in the wire form. `never` and
`once` are the claims as declared, so a caller can show them; `check` has already held the
table to them. `constants` gives, for each language with a module of its own, the names of the
three things the module adds beside the function — the initial state, the set of final states,
and the test for one ([generated-code.md](generated-code.md#a-machine)). The NumPy plan carries
the same as data (`carry`, `initial`, `final`); a target with one door and no module has no
place for a constant, and no entry.

## `graph`

One object: the rule as a graph of **what decides each value and which values it reads**.

Nothing in it is new knowledge — `doc` already prints, under every table, the columns it
reads and where each one comes from. This is that same relation gathered into one place,
which is what it takes to see the shape of a rule rather than read it a table at a time. A
rule whose two upstream tables cut the same input at different thresholds is a diamond, and
a diamond is something you notice; in the text it is two tables you have to read side by
side.

**It is a graph of dependency, not of time.** Every edge says "this value is read while
that one is decided", and all of it happens in one call: there is no step between two nodes
for anything to happen in. What happens outside is at the edge of the picture, which is why
the crossings carry their guards and `preconditions` comes along.

```json
{"rule":"cart_shipping_fee","alias":"","version":"1","source_sha256":"…",
 "nodes":[{"name":"tier","kind":"input","type":"member_tier"},
          {"name":"lines","kind":"sequence"},
          {"name":"amount","kind":"element","of":"lines","type":"money[JPY]",
           "range":{"min":0,"max":100000}},
          {"name":"total","kind":"value","type":"money[JPY]",
           "range":{"min":0,"max":1000000},
           "by":[{"kind":"sum","over":"lines","of":"amount"}]},
          {"name":"fee","kind":"value","type":"money[JPY]","range":{"min":0,"max":500},"output":true,
           "by":[{"kind":"table","name":"fee_table","policy":"unique","rows":4}]}],
 "edges":[{"from":"amount","to":"total","kind":"walk"},
          {"from":"total","to":"fee","kind":"reads","via":"fee_table"},
          {"from":"tier","to":"fee","kind":"reads","via":"fee_table"}],
 "preconditions":[{"kind":"sum","name":"total","over":"lines","of":"amount","max":1000000}],
 "carry":null}
```

**A node is a value, not an item.** A table is not a node: it is how one or more values are
decided, and it rides on them in `by`. That keeps the graph in the reader's own vocabulary
— the names in a rule are values — and it is what lets one value carry two deciders where a
clause takes precedence over a table.

| field | |
|---|---|
| `kind` | `input`, `sequence` (the list a walk runs over), `element` (a field of one element of it), or `value` |
| `of` | for an `element`, the sequence it belongs to |
| `type`, `range` | what the value is, and what it is held to. On an `input` or an `element` these are the guard the caller is held to at the door |
| `output` | present and true where the rule declares the value as an output |
| `per_element` | present and true where the value is decided once **per element** rather than once per call. A `sum`, a `count` and a `fold` are the three ways out of that frame, and nothing in the rule's text says which side of the line a value is on |
| `from_apply` | the `apply` a value came in through. Its name is `<apply>:<name>`, and everything under one apply is one subgraph |
| `by` | how the value is decided, one entry per decider, in the order precedence is declared: `{"kind":"table"\|"clause","name":…,"policy":…,"rows":…,"overrides":[…]}`, `{"kind":"derive"\|"define","expr":…}`, `{"kind":"sum"\|"count","over":…,"of":…,"where":…}`, `{"kind":"fold","verdict":…,"over":…}`, `{"kind":"result","expr":…}` |

`expr` is the line as the author wrote it, with the declared range and the citation taken
off — it is what `doc` prints in the expression column, from the same place. A value decided
by a table has no `expr`: the table is the expression.

An edge is `{"from":…,"to":…,"kind":…}` with `via` naming the decider that reads it, where
one is named. `kind` is `reads`, or `walk` for the edge that crosses the element frame —
many elements in, one value out.

`carry` is a state machine's carried pair (§15.148), `{"output":…,"input":…}`: the output the
caller passes back as that input on the next call. It is not an edge, because it crosses from
one call to the next and the caller is the one who carries it. `null` for a rule with no
`machine`.

**`graph` asks less than `check`.** Which value is read while which other is decided is
settled once the names and the types resolve, so a table with a gap in it has the same
edges as one without and still gets a graph; the picture is most wanted while a rule is
still being fixed. A rule whose names do not resolve gets none, because then there is
nothing to draw an edge between.

## `certificate`

One object per rule: the **evidence** behind all five things `check` proves — completeness,
the overlaps, the unreachable rows, the units and int64 — and, for each contract the rule
reads its inputs from, why what the contract lets through is what the rule takes. It is
small enough that a program which shares no code with rulec can re-check it in milliseconds. Two such programs read it.
`tools/recheck.py` has no dependencies and fits in one file. `proofs/` is a Lean 4
development that states the meaning of a rule, writes the checks as functions, and
**proves** that a `true` from each one settles the matching claim; the program `lake build`
produces runs those very functions, so what it prints is the theorems applied to one
document. The tests hold both to forged certificates as well as to the corpus.

```json
{"v":1,"rule":"coupon_stacking","alias":"coupon_stacking","version":"1","source_sha256":"200912693e01…","rulec":"0.25.0",
 "ranges":{"disc_a":["0","100000"],"disc_b":["0","100000"],"rest_a":["-100000","1000000"],"rest_b":["-200000","1000000"],"total":["0","1000000"]},
 "constraints":[],"inputs":["total","disc_a","disc_b"],"walks":[],
 "values":[{"name":"rest_a","of":"derive","type":"money[JPY, incl_tax]","expr":{"op":"-","l":{"name":"total"},"r":{"name":"disc_a"}},
            "interval":["-100000","1000000"],"scale":1,"stored_max":"1000000"},…],
 "tables":[{"table":"decide","policy":"unique",
   "axes":[{"column":"rest_a","kind":"derived","coords":["999JPY","1000JPY","1001JPY"],"step":"1","prefixes":null,
            "bounds":[[null,"1000"],["1000","1000"],["1000",null]]},…],
   "outputs":1,
   "rows":[{"row":1,"label":"","cells":["<= 1000JPY","-"],
            "tests":[{"cell":"cmp","tests":[{"op":"<=","value":"1000"}]},{"cell":"any"}],
            "origin":"decide","line":24,
            "source":[{"line":24,"col":2,"len":9,"text":"<=1000JPY"},{"line":24,"col":14,"len":1,"text":"-"}],
            "accepts":[[0,1],[0,1,2]],"produces":["no"]},…],
   "disjoint":[{"a":1,"b":3,"axis":0},{"a":2,"b":3,"axis":1}],
   "refuted":[{"a":1,"b":2,"farkas":[{"fact":0,"y":"1/2980"},{"fact":6,"y":"1/2980"},{"fact":11,"y":"1/2980"},
     {"coord":{"axis":0,"hi":true,"at":"1000","open":false},"y":"1/2980"},
     {"coord":{"axis":1,"hi":false,"at":"3980","open":false},"y":"1/2980"}]}],
   "undecided":[],
   "reach":[{"row":1,"at":[0,0],"values":{"rest_a":0,"rest_b":-100000},"at_values":["0","-100000"],
             "extra_values":["0","100000","0"]},…],
   "unused":[],"unreachable":[],
   "constraints":[],"above":{"never":[],"apart":[]},
   "linear":{"extra":["disc_a","disc_b","total"],
             "facts":[{"derive":"rest_b","le":true},{"derive":"rest_b","le":false},{"range":"rest_b","hi":false},…]},
   "cover":{"split":[{"row":1},{"row":1},{"split":[{"row":3},{"row":2},{"row":2}]}]}}]}
```

| field | meaning |
|---|---|
| `v` | the version of this shape, `1`. A re-checker refuses a certificate whose `v` it was not written for rather than checking something else, as both of the ones here do; an absent `v` is `1` |
| `types` | every name's declared type, and `groups` every group's members. A row's box and a value's type are **derived** from these by the re-checker, not taken from the certificate |
| `enums` | every enum's values, which is what a contract's strings are held to |
| `ranges` | every name's declared range, as exact rationals (`"7/2"`, an open end `null`). The int64 claim is re-checked from these. For a value the rule computes and for a column a table writes numbers into, it is the range rulec worked out — the interval of the expression, the hull of what the rows write — and a re-checker works it out again rather than reading it from here: a value's from `values`, a column's from what each row writes (`rows[].writes`), held to contain every one of those values (§15.196) |
| `inputs`, `walks` | the rule's inputs, and the names its walks leave behind (`count`, `sum`). With `values[].of` and the tables' `decides`, what a re-checker works the `kind` of every axis out from, and holds the certificate's word to (§15.196); their ranges are the declared ones |
| `scales` | the scale each numeric name is stored at (§7.1): its value is a whole number of `1/scale`. With the values' `expr`, this is what a re-checker works out the step of each column's values from (§15.190) |
| `constraints` | the `constraint` lines of the rule, once for the whole document: `left`, `op`, `right`. They are what the caller guarantees and the entry guard enforces (§15.55), and a value's interval can rest on one — the share of `allocate` is bounded by the amount only because a running total never passes the whole (§15.102). A re-checker reads them the way rulec does, chains included, and refuses an interval it cannot then derive. Each table repeats the ones its own columns are about, under `tables[].constraints` |
| `values` | every value the rule computes — `derive`, `define` and `result` alike, which `of` says: the type the rule declares for it, the expression as a tree, the interval the ranges and the guarantees force it into, the scale it is stored at, and the integer that interval reaches. A value with no interval to state (a truth value, an enum) says `null` for all three, and a re-checker refuses that for a type that is stored as an integer. Two things are re-checked from this: the units (§2.1, E103), by deriving each node's type from the leaves up — a name's from `types`, a literal's from the `type` it carries — and int64 (§7.4, E108), by interval arithmetic over the same expression. A literal also carries the value its unit resolves to, so the re-checker does arithmetic and not units; a date literal carries its day number, on the scale `ranges` gives a date input, which is what a boolean `define` comparing a date with it is read with (§15.196) |
| `contracts` | one entry per `shape` the rule reads inputs from: the condition the contract places on those values, and why the rule's **door** keeps it (§15.142). The claim is inclusion — every value the contract lets through is one the door takes — and the door is what the rule asks of the same values: each numeric input inside its declared range, at the `scale` between the rule's value and the integer the contract carries; each `constraint` between two of them; each enum input among its enum's values. `file` is the contract, named relative to the rule, and `sha256` its digest, which a re-checker given `--rule` holds the file beside the rule to. `vars` names the values the condition speaks of, by kind (`num`, `str`, `bool`): the inputs first, then the contract's other fields a condition mentions, as `@` and their path. An optional input, and one counted or tested with `where`, is not among them; the field-by-field comparison (E122) is what covers those. `atoms` are the conditions: `{"num":{name:coef,…},"k":…,"rel":"le"}` is `Σ coef·name + k <= 0` (`"lt"` is `<`, `"eq"` is `=`), already in whole-number form — `2x < 5` is written `x − 2 <= 0` — so that a sum over the rationals can show a boundary that holds only over the integers; `{"str":name,"values":[…],"in":true}` is a string among those values (`false`: among none); `{"bool":name,"value":v}` a truth value. `cases` opens the condition: a value gets through when it satisfies every atom of some case. `null` means it opens into too many cases, and nothing is claimed. `unread` says a rule of the contract could not be read and was taken as true, so the condition is the contract read wider than it is. `doors` lists what the door asks, `{"range":input,"hi":false}`, `{"constraint":k}` or `{"member":input}`, each with one proof per case: `{"farkas":[…]}` multipliers over the case's atoms (`{"atom":i,"part":0,"y":…}`, `part` 1 being the other half of an equality) and the negation of what is asked (`{"door":true,"y":…}`) that add up to a contradiction, as under `refuted`; `{"clash":name}`, strings or truth values of the case that cannot all hold; `{"within":true}`, every string the case lets the input be is one of the enum's. `proofs` is `null` for a thing the certificate cannot show. A re-checker **builds the door again** from `ranges`, `constraints`, `types` and `enums`, so a thing the list leaves out is named as not shown rather than passed |
| `axes` | the universe, one axis per column of the table, each with the coordinates the boundaries compress it to (§6.2), for a numeric axis each coordinate as a closed interval in `bounds` and the `step` its values sit on, and for a `string` axis the prefix each coordinate stands for in `prefixes` (`null` for the one coordinate that is under none of them). `kind` is `input`, `derived`, `define` (of any type), `walk` (what a `count` or a `sum` left behind) or `upstream` (what a table above decides, and a field of the elements a fold walks): a point on an axis of inputs is a value a caller can send, and on any other axis it is a point the feasibility sieve could not rule out, which is weaker. A re-checker works the kind out itself from `inputs`, `walks`, `values[].of` and the tables' `decides`, and refuses an axis that says another (§15.196); it reads a column's reach from whether `values` gives an expression it can work an interval out of. A re-checker holds a numeric axis to §6.2's construction: the coordinates run from the declared range's low end to its high end, each touching the next or one `step` past it, with nothing between, and each end of the range is a point of its own, since an interval leaves its ends out — so a coordinate cannot be quietly removed and the gap under it left uncovered. The axis of an optional input (`T?`) starts with the absent value, `none`, whose `bounds` is `null`; a re-checker refuses an optional input's axis without it, and holds the coordinates after it to the range. A comparison and a set of numbers never take that coordinate, and `not:` always does (§15.201). "Nothing between" is only so on the step the column's values really take, so it also works that step out from `values` and `scales` the way rulec does (§15.190) — a name on its own step, a literal on its value, a sum on the step both sides share, a product on the product of theirs — and holds `step` to divide it, and every end of a coordinate to lie on `step`: `amount * 10%` comes in tenths of a pound, and an axis of it cut every whole pound is refused |
| `decides` | the columns this table writes, in the order `rows[].produces` lists their values. A table below names one of these as an axis of kind `upstream`, and that is the link from a fact to the rows that settle it |
| `rows` | each row as a **box**: the coordinates it accepts on each axis, in `accepts`, the value it writes into each column of `decides` in `produces` (`null` where the cell is not a plain value word), beside the cells it was written with, and, for a table that writes numbers, in `writes`, what the row writes into each column of `decides` that holds a number — `{"expr":{"num":"50%","value":"1/2","type":"rate"},"source":{"line":24,"col":50,"len":3,"text":"50%"}}`, a literal or a name in the tree `values` uses, and where the cell stands (`null` for a column that holds no number, and `source` `null` in a row an `apply` brought in). A re-checker works out from these the range of the column, and with the file reads each cell back and holds the literal to the number it stands for (§15.196). In `tests`, the cells it was written with are resolved as far as their units — `{"cell":"cmp","tests":[{"op":"<=","value":"1000"}]}`, `{"cell":"is","words":["north_america"]}`, `{"cell":"prefix","words":["CH-"]}`, `{"cell":"any"}`, and for a set on a column of numbers its values, each a point of the axis — `{"cell":"in","values":["100","200"]}`, `{"cell":"not_in","values":["300"]}`. The re-checker **recomputes** the box from `tests` and the axis bounds and refuses a box that is not what the cell describes |
| `origin`, `line` | the table the row was written in, and the line it is written on. Rows of one table have a cell in the same columns and in no others, are all written in this file or all brought in by an `apply`, and take a run of lines in row order that no other table's rows fall inside — all of which a re-checker holds them to |
| `source` | where each cell stands in the `.rule` file — `line`, byte `col`, byte `len` — and the text that stands there. With `--rule` a re-checker reads the file and compares, and the span has to be that cell's own place: every cell of a row is on the row's `line`, they are that line's `|`-separated fields, all of them (`outputs` says how many of the line's fields are answers rather than cells), and none of them is empty. `null` for a row an `apply` brought in, which is written in another file; `null` for one cell where there is nothing to point at — a column a `clause` does not mention, or one a merged member table does not have, and then every row of that table has to agree |
| `disjoint` | `unique` only: for each pair of rows, one axis on which their coordinates do not meet. Re-checking one entry is one set intersection |
| `refuted` | `unique` only: the pairs whose boxes meet on every axis and which the table's **linear model** parts (§15.141): nothing both rows take satisfies it. `farkas` is the proof — the inequalities that take part, each with a multiplier at least zero. An inequality is a fact of the model, by its index in `linear.facts`, or an end of the coordinates both rows take on one axis: `{"coord":{"axis":i,"hi":false,"at":"3980","open":false}}` is `v >= 3980` (`>` where `open`), and a re-checker holds every coordinate both rows take there to it. Added up, each times its multiplier, the inequalities have to cancel every name and leave a constant that is false — positive, or zero where a strict one took part. That is all a re-checker does: addition |
| `days_apart`, `above_apart` | `unique` only, and only in a table that has them: the pairs whose boxes meet on every axis and which koyomi's days part (`{"a":1,"b":2,"axis":i}`: on the axis of days, every coordinate both rows take holds none of them, §15.174), or the rows of a table above part (§15.196): `{"a":1,"b":2,"axis":i,"at":[{"coord":c,"above_rows":{…}}]}`, where `axis` is a column a table above decides and `at` gives, for every value both rows take there, the rows of that table that write it with why each fires nowhere in what both rows take, in the shape of the cover's `above_rows` leaf. A re-checker counts those rows itself and checks every reason |
| `undecided` | the pairs whose boxes meet on every axis and which neither the axes, the linear model, koyomi's days nor the rows of a table above part, so the certificate does not claim them apart. A W114 the check could not settle lands here, and so do a pair a declared precedence orders and a pair the sieve ruled out point by point: the certificate states the weaker thing rather than a proof it cannot carry. A pair in none of these lists is a certificate that does not hold |
| `reach` | for each row, a point inside it: `at` is the coordinate on every axis, `values` the same point in the table's columns, and `at_values` those values as plain numbers on the axes' own scale — which is what lets the re-checker show the point is one the sieve admits, and not merely one inside the row's box: each value of a derived or `define` column inside the interval its expression is forced into, worked out again by the re-checker. Where the table has a linear model, the values are solved from it, and `extra_values` gives the model's other names in the order `linear.extra` lists them: the point has to satisfy every fact of the model too. Where the point takes a value of a column decided above that the cover rests on the rows of (`above_rows`), one of the rows that write that value has to fire on the values (§15.195). Under `policy first` the point is also outside every row above it |
| `linear` | the table's linear model (§15.141): every `derive` equation, declared range and `constraint` its numeric columns reach, closed over the names they tie together — a chain of constraints through an input no table has a column for included. `facts` names each one by where it comes from: `{"derive":name,"le":true}` is `name − expr <= 0` and `"le":false` the other half; `{"range":name,"hi":false}` one end of the declared range; `{"constraint":k}` the `constraint` at that place in the document's list. A re-checker **builds each fact again** from `values`, `ranges` and `constraints` — a `derive` has to be linear, names of its own type, literals, sums and differences, and products and quotients by a number or a rate; the range of a value the rule computes is the interval the re-checker works out from its expression, not the one `ranges` states (§15.195) — and refuses one it cannot build. `extra` names the model's names that are not columns of the table. Empty for a table whose model would say nothing the ranges do not |
| `unused`, `unreachable` | rows outside the reachability claim, named rather than passed over: ones an `apply` brought in that this rule's bindings leave unused (§15.69), and ones the sieve rules out entirely — E102 reads the sieve only where the value of a derived or `define` column takes part (§15.189, §15.195), so `check` passes the rest (a row only a `constraint` rules out, for one, or one the tables above rule out together with a `constraint`) and the certificate says so. A row called unused has to be one written in another file, which the `source` of that row shows |
| `above` | what the tables above rule out, written on this table's own axes (§15.115). `never` is a list of `{"axis":i,"coord":c}`: no row of the table that decides that column writes that value at all. `apart` is a list of pairs that cannot stand together — `a` and `b` as coordinates, `input` the column the two decided columns share, and `spans` the span each of them leaves it, as two ends for a number or a list of values otherwise. A re-checker earns both back from the rows of the deciding tables and rests its verdict on what it recomputed; the written spans have to **contain** those, so a narrower one cannot make two things that meet look apart |
| `cover` | completeness (E101) as the walk of §6.3, written down. A `split` has one child per coordinate of the axis at its depth — so the children tile the axis by shape, not by a claim — and every leaf is `{"row":n}`, a row that takes the whole subtree, or a box no input reaches: `{"constraint":k}`, the `constraint` that cannot hold there, `{"derived_axis":i}`, a derived value whose coordinate lies outside what its expression reaches, `{"define_axis":i}`, the same of a `define` that computes a number (§15.195) — a re-checker works the interval out again from `values`, and reads an interval coordinate without its ends, so `>100%` lies outside a share that ends at 100% — `{"truth_axis":i}`, a boolean `define` of one comparison at the truth value it never takes over the intervals of its two sides (§15.196), which a re-checker works out again from the comparison in `values` — `{"farkas":[…]}`, a box the linear model leaves no values in, with the multipliers that say so exactly as under `refuted` — the box being the coordinates the path to the leaf has fixed, and every coordinate of the axes it has not (§15.141) — or `{"every_point_ruled_out":true}`, a box whose points the sieve rules out one at a time (§15.98). `{"above_rows":{"axis":i,"column":…,"value":…,"rows":[…]}}` is a box whose path holds a column a table above decides at a value that table writes only in rows none of which fires in the box (§15.195): `rows` gives each of those rows by its number there, with `{"clash":j}`, an axis `j` of this table, a column of words both tables cut, on which the box takes none of the words the row lets in, or `{"farkas":[…]}`, multipliers as above that may also name an end of what the row takes on a numeric column of its own table, `{"cond":{"name":n,"hi":true,"at":"50","open":false}}` (`n <= 50`), which a re-checker holds every coordinate that row takes on that column to; the name has to be one this table numbers a value for, a numeric axis or a name of `linear.extra`. A re-checker counts the rows that write the value itself, from that table's `produces`, and refuses the leaf unless it gives a reason that holds for every one of them. `{"upstream":…}` is a box the tables above cannot produce, and rests on this table's `above` facts: a re-checker settles it by finding a fact the box's coordinates trigger, and the fact itself by recomputing it from the rows of the table that decides the column. `null` when the walk ran past the budget |
| `constraints` | the `constraint` lines a cover leaf points at |
| `machine` | a rule that is one step of a state machine (§15.148) carries the claims its `machine` section makes, laid on the rows of the table named by `over`: `{"table":"","why":…}` when they cannot be (the state not an axis of that table, say), and otherwise the following. `axis` is the state's axis of that table, `states` the enum's values, and `initial` and `finals` indices into them. `rows` says what each row does to the state — `{"row":n,"to":i}`, or `{"row":n,"stay":true}` for a row that hands the carried input back — with `source`, where that answer is written. A re-checker reads **where a call can go** off the rows themselves: from `s`, every row whose box takes `s` on `axis`, to where that row sends it. That over-approximates the calls that really happen, which is the safe direction for the closures below. `never` and `once` say what each line claims — `{"states":[…],"after":[…]}`; the output, the test its cell resolves to (`lo`/`hi` in the column's integer, or `words`) and the value each row writes (`null` for one computed, which then counts). `held_inputs` are the inputs a case holds (§15.149), and `held` the axes of this table among them. **The claims are laid on the rows once per world**, each world an element of `worlds` with `at`, a coordinate for each axis of `held` — every combination of them, once each, and a single world with `at: []` when `held` is empty. In a world a call only takes the rows whose box holds its coordinates there, and: `reach` is a set of states holding `initial` and closed under a call; `final_certified` claims that no call leaves a final state of `reach`; `never` gives, per line, the pairs (state, has the case been in `after` yet) closed under a call from `(initial, …)` with none in `states` with the flag set, or `null`; `once` gives, per line, the pairs (state, how many counted calls so far, up to two) closed under a call, none with two, or `null`; `finish` is the other direction: for every state of `reach` that is not final, a `path` of calls to a final state, each a point handed over the way `reach` under `tables` is — in the row's box, at the state, with the world's coordinates, asked about, and under `first` outside every row above — so a case really can finish from there. A path is taken as one case's only while nothing but its own column reads a held input: when the table reads a column that is not an input, or a `constraint` names a held input, the certificate gives no paths and a re-checker says the claim is not laid on the rows. `uncertified` names the claims this document does not carry, and a re-checker lists them rather than passing them |

**What "proved" means here.** `proofs/RulecCert/Semantics.lean` says what a table claims:
under `unique`, every point the rule is **asked about** is taken by exactly one row; every
row answers somewhere; and a value keeps the type it is declared with and fits int64. Asked
about means the values behind the point satisfy every `constraint` the rule declares, put
every derived column and every column of a `define` that computes a number inside the
interval its own expression forces, and — where the table has a linear model — satisfy every
fact of it, with values for the model's other names as well; and where the point takes a value
of a column decided above that the cover rests on the rows of, one of the rows that write that
value fires on those values (§15.195). That is what rulec decides, and no more: whether some real input produces a given
point is settled over the rationals, not the integers. Each check is then a theorem:
`Certified.unique`, `Certified.complete`, `Certified.reached`, `eval_type_of_typeOf`,
`stored_in_i64`. A refutation's is `farkas_sound` — multipliers that pass the check leave no
values at all — and `not_asked_of_farkas`, which turns that into "no point of this box is asked
about"; `not_asked_of_aboveRuledOut` says the same of a box whose rows above each come with a
clash or a refutation, and `not_asked_of_derived` of a coordinate outside a column's reach. A contract's is `included_sound`: when every proof passes, any values that satisfy
some case of the condition satisfy everything the door asks. One of them,
`mem_boxOf_cmp_iff`, is what ties the boxes to the cells: a coordinate is taken exactly when
every value in it satisfies the cell, **provided** no value a cell compares against falls
strictly inside a coordinate — §6.2's construction, which the checkers verify rather than
assume. `mem_boxOf_in_iff` and `mem_boxOf_notIn_iff` say the same of a set of numbers: in the
box exactly when the value is one of the set's, or none of them. `mem_boxOf_absent_iff` says it
of the absent value of an optional input: the first coordinate is in a cell's box exactly when
the cell takes the absent value (`CellTest.takesAbsent`: `-`, `none`, `not:`, and a set of words
that names `none`).

A machine's claims are `proofs/RulecCert/Machine.lean`. There a case is what the rows allow —
it starts at `initial`, and each call goes wherever a row that takes the state sends it — and
each check is a theorem about every sequence of calls at once: `reaches_mem` (every state a
case can reach is in `reach`), `final_stays` (no call leaves a final state), `never_after`
(once a case has been in a state of `after`, it is never in one of `states`),
`once_below_two` (no case makes two of the counted calls), and `reachable_finishes` (from
every state a case can reach, calls that really happen lead to a final state). The first four
hold of the over-approximation, so they hold of the rule; the last is built from points the
certificate hands over, so it holds of the rule too. A world is `Machine.world`, the machine
with the rows that do not take its coordinates left out, and `stepIn_world` and
`reachesIn_world` say that every call and every state of a case of the world is one of that
machine's — so the theorems above, applied to it, speak for the cases of the world.

**Where it stops.** The file is tied to the document by its digest and, cell by cell, by the
byte spans above. Everything else the certificate says about the rule is **its own word**,
and no re-checker can go behind it without parsing the `.rule` file — which neither does, on
purpose: a checker that reads a rule the way rulec reads it is not independent of it. So
these are stated, not derived: the declared `ranges` and `types`, the `groups`, the
`enums`, the `constraints`, each value's `expr` and `scale`, the `scales` of the names, and how
many `outputs` a table has. A forged one of those is a forged rule, not a forged proof about the rule in front of
you. The same holds for a contract: its digest ties the section to one text, and the rest
of the section — which inputs the contract feeds and at what scale, the atoms, how they
open into cases — is the certificate's reading, since neither re-checker reads CEL or a
schema. An input left out of the section is one whose door is not checked.

Four more things are named in the run rather than proved, and both programs end with a line
that lists them rather than printing a clean "ok": the pairs the axes do not part, rows an
`apply` brought in, rows the sieve rules out, and a point handed over with no values behind
it. A certificate with no `scales`, as rulec wrote before §15.190, adds a fifth: the `step` of
an axis whose column's step cannot be worked out without them. A contract adds three: a condition with parts taken as true (`unread`), one that opens
into too many cases, and a thing the door asks that the certificate gives no proof for. One thing is counted: a cell literal written in a
unit the axis does not write its own coordinates in (`2kg` against an axis of grams), where
pinning the number would take the lexer's unit table. A literal in the axis's own unit has to
be one of its boundaries or lie outside it altogether (§6.2).

`tools/recheck.py` parses the cell text it reads out of the file and holds the certificate's
operators and numbers to it; the Lean program does not, and recomputes the box from the
parsed form instead. A rule that does not pass `check` produces no certificate at all.

```console
$ rulec certificate rules/health_insurance_premium.rule > cert.json
$ python3 tools/recheck.py --rule rules/health_insurance_premium.rule cert.json
health_insurance_premium (health_insurance_premium v1, sha256:a5c5cf4eaf62) — certificate by rulec 0.25.0
  units: 4 values keep the type the rule declares
  int64: 4 values fit
  grade: unique, 50 rows — 1225 pairs disjoint, 50 rows reached, 101 boxes covered, 50 boxes read back from their cells, 1 axes tiled
  applied: unique, 2 rows — 1 pairs disjoint, 2 rows reached, 2 boxes covered, 2 boxes read back from their cells, 0 axes tiled
  the digest is rules/health_insurance_premium.rule's
  the file says the same: 52 cells read back from it
  every claim this program states was proved

$ (cd proofs && lake build) && proofs/.lake/build/bin/rulec-recheck --rule rules/health_insurance_premium.rule cert.json
health_insurance_premium (0.25.0), re-checked against the Lean proofs
  values: 4 typed, 4 held to int64
  grade: 50 rows — complete, 50 rows reached, no two rows meet, 1 axes tiled
    50 boxes read back from the cells they were written as
  applied: 2 rows — complete, 2 rows reached, no two rows meet
    2 boxes read back from the cells they were written as
  the digest is rules/health_insurance_premium.rule's, and 52 cells are read back out of it
OK: every claim this program states was proved, by the theorems of RulecCert.
```

Exit code 0 when every table holds and 1 when a claim does not; `tools/recheck.py` answers 2
for a certificate it cannot read at all.

## `schema` and `adapter`

Already machine-readable and take no `--format`.

- `schema` prints one JSON Schema for the wire an adapter speaks (see below). The
  description of an integer property says its unit and, for a rate, its step, so that a
  caller who never reads the rule knows that 18.3% at a step of 0.1% travels as 183.
  Where the rule has preconditions this shape cannot state, a `$comment` says so and names
  their kinds — a document that passes validation and is refused anyway has to say that
  about itself. The preconditions themselves are in `api` under `preconditions`.
  `--keys alias` names every property by its ASCII alias instead of the rule's name, with
  the name as the property's `title`, which is the shape an HTTP request body or a form
  wants; the wire itself, and the tool built on it, keep the names.
- `adapter` prints a Python or Go source template, 20 to 30 lines, to wrap a legacy
  implementation.

## `doc`

`doc` has no `--format json`. It renders for people — those who read a change to understand
and check what the code is to carry out — and markdown is that shape; `--format html` is the
same document as one page with a form on it, where the generated JavaScript runs the case a
reader types in.

The HTML page is laid out as a board. The left pane holds the form, the inputs, the
outputs and the types; the middle is a canvas that scrolls both ways, with one card per
**decider** — a table, a `derive`, a `define` — placed left to right by how far it is from
the values that arrive from the caller; a dock opens below a card you select, holding that
table's "column / where it comes from" and what `rulec check` verified about it. Both
borders can be dragged.

A card carries the decision table itself, moved out of the body of the document rather than
drawn again, so the two cannot disagree. The trace that lights a row lights it inside the
card, and the card's heading says which row fired and what it decided; cards you have not
selected are monochrome. It is `rulec graph` laid out, held to it by a test. The arrows are
not an order of events — everything is decided in one call — so nothing in the caption says
"next" or "then". Without JavaScript the page is the document it always was.

`--audience customer` renders the same rule as the article a help centre publishes: the
inputs in plain words, the tables with `-` as "any" and `not:` as "other than", the rounding
as a sentence, and **the cases on either side of every threshold** — the boundary-pair
vectors of `rulec vectors`, one line per pair. Aliases, declared ranges, diagnostic codes and
the list of what `rulec check` verified are left out: they belong on the page for people (the
default, `--audience approver`). It is markdown only (`--format html` cannot be combined with
it), and under `--out` it is written as `<alias>.customer.md`, beside the page for people,
`<alias>.md`.

## `mcp`

`rulec mcp` speaks the Model Context Protocol over stdio — and only over stdio, because what
it hands out is the commands that read and write your files. One JSON-RPC 2.0 message per
line in and out. It is the command table in another syntax, so nothing here is a second
implementation of a command.

- **Tools.** One per command, named `rulec_<command>` (`rulec_check`, `rulec_gen`, …). The
  positional arguments become `files` (a list) or `file`, `dir`, `code`, `old` and `new`,
  `fixtures` and `rule`, and a command whose usage opens with a choice takes it as
  `subcommand`, an `enum` of those words, ahead of `file` (`import`: `csv` or `xlsx`; `source`:
  `fetch`, `pin` or `outdated`); every flag becomes a property named after it with `-` as `_`
  (`diff_base`, `require_all`), a boolean for a flag without a value, a list for a flag that
  may repeat (`fill`), and an `enum` where the flag's values are a closed set. `lang` is
  accepted everywhere. The result is two texts: what the command printed (stdout, then stderr
  if any), and `exit code N`. `isError` is true only for exit 2 — findings are exit 1 and are
  not errors. An argument the table does not know is refused with a JSON-RPC error before
  anything runs.
- **Resources.** `rulec://docs/agents.md` (the procedure — read it first),
  `rulec://docs/reference.md`, `rulec://docs/formats.md`, `rulec://docs/generated-code.md`
  and `rulec://docs/backends.md`, embedded in the binary at build time so they can never be
  a version other than the one the tools implement. Links between them point at the resource
  URIs.

To register it, add a stdio server whose command is `rulec mcp` — for Claude Code,
`claude mcp add rulec -- rulec mcp`; elsewhere, `{"mcpServers":{"rulec":{"command":"rulec","args":["mcp"]}}}`.

**The rule as a tool** is the other direction, and it is generated code rather than this
server: `gen` writes `<alias>_mcp.py` and `<alias>_mcp.mjs` beside the module, a server with
one tool named after the alias. Its `inputSchema` is the `in` object of `schema` above; its
result is one fixtures record (below), as text and as `structuredContent`; a call the rule
cannot take comes back with `isError` and the argument named; and `--record <file.jsonl>`
appends every answered call to that file as a record. **Unlike this one it is not stdio
only**: `--http <port>` serves the same tool over MCP's Streamable HTTP, and where the host
renders MCP Apps it offers the page for people as the tool's view.
[generated-code.md](generated-code.md#the-rule-as-an-mcp-tool) has it.

---

# The data files

The three formats below are not command output: they are files that rulec reads and writes,
and they all use the same wire representation for a value.

## Vectors (`rulec vectors`, `generated/vectors/<alias>.jsonl`)

JSON Lines, one test case per line, generated from the boundaries of the rule (§9).

```json
{"in":{"order_date":"2026-03-31"},"out":{"kind":"before"},
 "trace":["table pick row 1"],
 "why":"boundary pair: table pick row 1 order_date 2026-03-31 inside"}
```

| field | meaning |
|---|---|
| `in` | the inputs, keyed by the rule's own names |
| `out` | the outputs the reference evaluator produces |
| `trace` | the rows that fired, in order |
| `why` | which coverage obligation this case was generated for, or `example row N` for a case the rule's own `examples` wrote. **Prose** |

`gen` writes a second file, `<alias>.expected.jsonl`, with one **fixtures record** per
vector in the same order — `in`, the expected values as `observed`, and the rows that
matched as `trace` (below). The generated runner prints the same record through the module's
own record function, and that is what `rulec test` compares, byte for byte: the agreement is
checked row by row, and over the wire form of every input. Being a fixtures file, it is also
what `rulec fixtures lint` and `replay` accept.

The inputs the reference evaluator **refuses** go to a third file, `<alias>.refused.jsonl`: an
input for every reason the generated code's door has to turn one away, and, for a rule that
walks a sequence (§15.56), the walks it has no answer for — two elements both taking under
`take_unique` is a contradiction.

```json
{"in":{"freight_rows":[{"row_zone":"kinki","threshold":1000,"row_fee":100000},
                       {"row_zone":"kinki","threshold":1000,"row_fee":100000}]},
 "refused":"contradiction","error":"fold verdict: two elements matched a take_unique",
 "why":"take then take"}
{"in":{"weight":1,"express":"false","declared":0,"cover":0},
 "refused":"input","error":"express is not a boolean",
 "why":"express as the string \"false\", not a truth value"}
```

| field | meaning |
|---|---|
| `in` | the inputs, in the shape the vectors file uses, so the same runner reads it |
| `refused` | `input`, an input outside the contract, which the generated code raises as `RuleInputError`; or `contradiction`, a case the rule cannot answer, raised as `RuleContradictionError` |
| `error` | the sentence the reference evaluator says, in the language the files were generated in; every language says the same |
| `why` | which reason the case is in the file for. **Prose** |

Each is a vector of the suite with one thing changed (§15.204): for each input, the input left
out and a value of another kind; for a number, one that is not whole (off its step, for a
rate) and one past each end of its range, an optional input's with a value; for a date, a day
the calendar does not have, one past each end and a day its koyomi date does not come to; for
an enum, a word it does not have; for a truth value, the string `"false"` and the number `1`;
for each `constraint`, a combination it rules out; and for the sequence a walk reads, the
same of the sequence and of the fields of one element, one element past what a `count` over
it can be, and a `sum` over a field past the top of its range. A case the reference evaluator
answers is not written.

`rulec test` puts them all in **one run**: every generated runner answers an input its door
refuses with a line of its own in the place of the record — `{"refused":…,"error":…}` — and
reads on, and the run is green only if every language refuses every one of them with the kind
and the sentence the file gives (the value the language prints after the sentence is not
compared). A generated MCP server is asked the same and has to answer `isError` with the same
class and sentence. A line without `refused` or `error` — one written by hand — asks only that
the input be refused. `rulec verify` does not use this file: it asks an implementation for
answers, and here there is none to compare.

A rule that is one step of a state machine (§15.148) gets two more files, `<alias>.traces.jsonl`
and `<alias>.traces.expected.jsonl`: sequences of calls, where the single vectors are single
calls. `rulec vectors --out` writes the first beside its own file; on stdout it prints the single
calls only.

```json
{"machine":"meta","carry":{"in":"state","out":"next_state"}}
{"in":{"event":"ship","amount_paid":0},"step":"start","state":"received",
 "why":"transition pair: received -[table step row 3]-> received then received -[table step row 1]-> paid"}
{"in":{"event":"pay","amount_paid":0},"step":"next"}
```

The first line asks the runner for its constants. Every other line is one call and carries
every input but the carried one: `"step":"start"` begins a sequence in `state`, and `"step":"next"`
makes the next call **from the state the call before it answered**, as the language holds that
value — the runner does not read it back out of JSON. There is a sequence for every transition
a case can make and every two that can follow one another, each the shortest from the initial
state that ends with them and each passing the `held` inputs the same value on every call, and
one for each `scenario`; `why` says which. The expected file
starts with `{"initial":…,"final":[…]}`, what the runner has to print for the first line, and
then has one fixtures record per call, the carried input filled in, so a runner that hands the
state over wrongly shows up as a record whose `in` differs.

## Fixtures (`rulec fixtures lint`, `replay`, `diff`)

JSON Lines, one past record per line. Extracting them from production logs is the user's job;
rulec only validates types and ranges (§10.2).

```json
{"ts":"2025-08-14T09:12:33+09:00","tag":"order:1234567",
 "in":{"weight":5,"girth":40,"dest":"canada","signature":false},
 "observed":{"fee":13},
 "trace":[{"table":"size_of","row":2},{"table":"base_rate","row":2},
          {"table":"fuel_rate","row":1},{"table":"signature_fee","row":2}]}
```

| field | required | meaning |
|---|---|---|
| `in` | yes | the inputs as they were at the time |
| `observed` | yes | the values that actually came out. **Every output is required**. `diff` does not read it: it compares two versions' answers to `in` |
| `tag` | no | a label for the record, shown in witnesses. For a rule that is one step of a state machine, it also says **which case** the record belongs to: the records that share a `tag` are one case's calls, in the order the file has them, and `replay` and `diff` count cases as well as records (`cases` in their JSON) |
| `ts` | no | when it happened |
| `by` | no | where an input came from, when it was not read as it stood ([below](#where-a-value-came-from-by)). **rulec does not read it** |
| `trace` | no | the rows that matched when the record was made, `{"table":…,"row":…}` each, in table order, with `"label"` added for a row that carries one; a `clause` is the one row of a table named after it. The generated code's record function writes it ([generated-code.md](generated-code.md#a-record-of-one-call)); `lint` checks that every table exists and every row is one the table has |

The generated code writes this line itself: every module has a record function that takes
the inputs, the outputs and the rows that matched and returns the record, so a log of the
generated code needs no extraction. What still has to be extracted is a log of an
implementation rulec did not generate.

An input must be an integer in the canonical unit; a decimal is refused, naming the field,
unless its value is whole: `1000.0` is 1000 (§15.205).
An observed value is read as the value it is, decimals included: `11.0` is 11, and an
implementation that computed finer than the output's step (`820.5` yen, `123.4` for 12.34% at a
step of 0.1%) answered something the rule does not, which `replay` compares exactly — below the
output's rounding grid, a suspected rounding difference. A field the rule does not know is an error, not something to ignore — discarding it silently
would turn a misspelling into "filled with the default value", and only the match rate would
move.

**A record does not say what its integers count.** They are in the steps and the units of the
version of the rule that wrote them, so a record written before a declared step or unit
changed no longer reads as what it meant. An input that falls outside its declared range
excludes the record; an observed value that falls outside what the rule can produce is compared
like any answer the rule does not give, counts as a mismatch, and is reported apart
(`out_of_reach`) with this reading suggested. `--read-as <file.rule[@rev]>` (on `fixtures lint`, `replay`, and
`diff` with records) reads every number at the steps and in the units of that version — a
file, or a git revision such as `rules/rates.rule@v1` — and brings it to the one the records
are read as: the rule for `lint` and `replay`, the old version for `diff`. A rate kept in
whole percents is read as the rate it was, yen become sen. It has to be the same rule. A
replay that compared nothing because every record was excluded says so and exits 1
(§15.144, §15.145).

An input is held to what the generated code takes at its entry, not to its type and range
alone: a date outside its range, a date that is not one of the days a koyomi date comes to
(`range from koyomi`), and a combination a `constraint` rules out are problems of the record
too (`bad_input`; a constraint names two inputs, so the problem has no `field`).

**Fixtures are not committed to a repository**: they hold order amounts. Pass them to CI as
an artifact or from protected storage.

### Where a value came from (`by`)

Not every input is read from a system. Some are **decided**: a class picked out of the input's
own enum by a model, a yes/no that came back as a probability, a grade somebody typed. The rule
is a pure function either way — what was decided is passed in as a value, the way the time and
the stock are (§10.3) — so the record already keeps *what* the value was. `by` keeps **who gave
it**, and how sure they were:

```json
{"ts":"2026-09-17T09:12:33+09:00","tag":"ticket:88231",
 "in":{"confidence":934,"category":"billing"},
 "by":{"category":{"src":"jev","ver":"2026-09-16","conf":934}},
 "observed":{"handling":"auto"},
 "trace":[{"table":"triage","row":2}]}
```

| key | meaning |
|---|---|
| `src` | what decided the value: a short, stable identifier — `jev`, `agent`, `ops` — not prose |
| `ver` | the version of that thing, spelled the way it spells its own versions |
| `conf` | how sure it was, as **an integer count of 0.1% steps**: 934 is 93.4%. The same wire a rate uses (§10.2), so a record carries one kind of number and not two. Leave it out for anything that has no confidence, such as a person |

Only the inputs that were decided need an entry; an input with none was read as it stood.

**rulec reads none of this.** `lint` does not check it, `replay` does not filter on it, and the
match rate does not account for it. It is a place to put provenance that survives the pipeline
unchanged, so that "the records whose class the model was less than 90% sure of" is a line of
`jq` a month later instead of a guess — and it is deliberately not a claim the tool makes about
those records.

## The replay manifest (`--manifest`)

One JSON object declaring the default values used to fill a missing field (§10.3).

```json
{"rule":"parcel_rate","fills":{"weight":5,"signature":false}}
```

`rule` is optional and, when present, must match the rule being replayed. Every key of
`fills` must be an input of the rule, and its value is in the canonical unit. For `diff`, a
key is an input of the old version, read at its types as the records are, or an input only
the new version takes: the old version's records do not have it, so its value, read at the
new version's types, is what every record is answered with, and every such record is a filled
record. A record
missing a field that `fills` does not cover is **excluded outright** rather than guessed at;
a record filled from `fills` is marked a filled record, counted separately, and kept out of
the headline match rate. `--fill weight=5` overrides one entry on the command line, for a
sensitivity run; it applies after the manifest.

The manifest holds only field names and default values, so unlike the fixtures it can be
committed.

## The adapter protocol (`rulec verify`)

The legacy implementation is started as a child process and JSON Lines flow over
stdin/stdout. No FFI and no network: a process and line-oriented JSON is the smallest surface
that is written the same way in every language (DESIGN §10.1 has the reasoning).

```
rulec → {"rulec":"adapter/1","rule":"parcel_rate","in":["weight","girth","dest","signature"],"out":["fee"]}
legacy ← {"ok":true,"impl":"legacy@fake-1"}
rulec → {"id":0,"in":{"weight":1,"girth":1,"dest":"domestic","signature":true}}
legacy ← {"id":0,"out":{"fee":11}}
rulec → {"id":9,"in":{"weight":1,"girth":1,"dest":"overseas","signature":true}}
legacy ← {"id":9,"err":"unsupported: signature to overseas"}
```

1. rulec writes one handshake line naming the rule and its input and output names.
2. The adapter answers `{"ok":true,"impl":"…"}`. `impl` identifies the build being compared
   and is reported as `counterpart`. Anything but `ok: true` aborts the run.
3. For each vector rulec writes `{"id":…,"in":{…}}` and reads one line back.
4. The answer is either `{"id":…,"out":{…}}` or `{"id":…,"err":"…"}`. A record the adapter
   declares it cannot answer is **excluded from the match-rate denominator** and reported as
   `unanswered`, so an adapter cannot raise the rate by refusing the hard cases.

Names and values on the wire are the rule's own names and integers in the canonical unit.
Every line is read as JSON, so any encoder's output will do: a key or a value written with
`\u` escapes, as Python's `json.dumps` and PHP's `json_encode` write them by default, reads
the same as one written out. An answer is read as the value it is, as a recorded `observed` is:
`11.0`, as a float is written, is 11, and a decimal finer than the output's step is compared
exactly and counts as a mismatch. An optional output with no value is `null`, the way the vectors
write it. A line that is not JSON, an answer carrying another record's `id`, and an answer
with neither `out` nor `err` stop the run with exit 2, naming the record (§15.151).
`rulec schema` prints the JSON Schema of `in` and `out`, and `rulec adapter --template
python|go` prints a template to fill in.

**When the legacy implementation is a Connect service** it is not a process to start but an
endpoint to call, and `--template connect-python` prints that shape instead: the same
JSON Lines on stdin and stdout, with a client in the middle. Two places are left to fill in,
and both are the other side's own messages. A `ConnectError` becomes `err`, so a record that
service says it cannot answer leaves the denominator like any other.

```console
$ rulec adapter rules/parcel_rate.rule --template connect-python > adapter.py
$ rulec verify rules/parcel_rate.rule --adapter python3 adapter.py https://pricing.internal
Compared 96 / matched 96 (100.000%)
Counterpart: connect@https://pricing.internal
```

## The extraction protocol (`rulec source fetch --via`)

A PDF or a scan needs an extractor that rulec is not: the formats it reads itself are csv, md,
xlsx and docx. The extractor is a child process, as the legacy implementation is — one
direction and one shot, because an extraction is one question.

```
rulec → ./extract.py tariff.pdf
extractor ← {"rulec":"extract/1","impl":"docling 2.4.0"}
extractor ← {"block":"table","page":12,"grid":[["Destination","Fee"],["Kinki","990 yen"]]}
extractor ← {"done":true}
```

1. rulec runs the command with the document's path appended to it.
2. The first line names the protocol and **the extractor itself**. `impl` is required and is
   written to `<document>.fragments/extractor.txt`, where `rulec doc` reads it and names on the
   page for people what read the document — a table a model read out of a scan is evidence of
   a different kind from one that was already a grid.
3. Each `{"block":"table",…}` line carries a `grid` of rows of strings, in document order; the
   `n`th of them is the fragment `table<n>`. `page` is optional and appears in the report. Blocks of
   any other kind are read and let go, so an extractor that also reports headings needs no
   flag.
4. `{"done":true}` ends the stream. **It is required**: an extractor that died half way would
   otherwise hand back the tables it managed, and `table3` would quietly be a different table.
   A missing handshake, a missing `done` and a non-zero exit all fail the command (exit 2)
   and leave the copies that were there untouched.

`rulec adapter <file.rule> --template docling` prints a template to fill in, naming the
documents of that rule that need one. The extractor is used **only** where rulec cannot read
the document itself: the formats it does read are read the same way every time, and a `--via`
that quietly rewrote those copies would make the pins depend on who ran it.
