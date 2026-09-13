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
- **A number is an integer in the canonical unit** (§10.2): yen for `money[円, …]`, the
  declared unit for a quantity, the number of steps for a rate, `YYYY-MM-DD` as a string for
  a date, `true`/`false` for a boolean, the value's own name for an enum.

Exit codes are unchanged by `--format json`: 0 notes only, 1 findings, 2 bad arguments or an
internal failure. **Read the exit code, not the emptiness of the output.**

---

## `check`

One object per diagnostic. This is version 2; every field version 1 had is still present and
still means the same thing, and `v` says which version wrote the line.

```json
{"v":2,"severity":"error","code":"E101","file":"rules/送料.rule","line":34,"column":7,
 "title":"Completeness gap: some input matches no row",
 "notes":["An input that matches no row: あて先 = 山梨県, サイズ = S60","hint: …"],
 "where":{"file":"rules/送料.rule","line":34,"column":7,"table":"運賃表"},
 "spans":[{"line":34,"column":7,"length":9,"label":"the input space is not fully covered"}],
 "witness":{"inputs":{"あて先":"山梨県","サイズ":"S60"}},
 "rows":[],
 "fix":{"kind":"add_row","text":"| 山梨県 | S60 | 820円 |"},
 "key":"…"}
```

| field | meaning |
|---|---|
| `v` | format version. `2` today |
| `severity` | `error` or `warning` |
| `code` | the stable code. `rulec explain <code>` has the ledger entry |
| `file`, `line`, `column` | the primary position. `column` is 1-based |
| `title`, `notes` | **prose** |
| `where` | `file`, `line`, `column`, and `table` / `row` when the finding is about a table row. `row` is 1-based |
| `spans` | every underlined range: `line`, `column`, `length` (bytes), `label` (**prose**) |
| `witness` | an assignment of values that exhibits the finding. `inputs`, `outputs` (what the rule really produces), `expected` (what an example said; E107 only). Absent keys mean there is nothing to say, never "empty" |
| `rows` | every row that takes part: `{"table":…,"row":…}` |
| `fix` | `kind` from the closed set below, and `text`: the rewritten form, ready to paste. Language independent, no prose |
| `key` | the identity `--diff-base` compares on. Two runs that name the same finding use the same key |

`fix.kind` is one of `add_row`, `remove_row`, `add_rounding`, `add_range`, `widen_range`,
`add_alias`, `mark_default`, `mark_contract_only`, `change_policy`, `add_expected`, `none`.
`none` means no single mechanical edit is right; the reason is in `notes`.

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
`--budget 1` to reproduce.

## `fmt`

One object for the run.

```json
{"unformatted":["rules/送料.rule"],"formatted":[]}
```

`--check` fills `unformatted` and writes nothing; without it, the files rewritten are listed
in `formatted`. Both keys are always present.

## `gen`

One object for the run.

```json
{"written":["generated/python/shipping_fee.py"],"stale":[],"missing":[]}
```

`stale` is a file that exists but differs from a fresh generation; `missing` is one that is
not there at all. `--check` writes nothing and fills those two; without it the files actually
written are in `written`. A file already up to date appears in none of the three.

## `coverage`

One object per rule file.

```json
{"file":"rules/送料.rule","vectors":68,
 "criteria":[{"name":"row","satisfied":7,"total":7,"missing":[]},
             {"name":"boundary_pair","satisfied":4,"total":4,"missing":[]},
             {"name":"shadow_pair","satisfied":3,"total":3,"missing":[]}]}
```

`name` is one of `row`, `boundary_pair`, `shadow_pair`. An entry of `missing` is
`{"what": …, "hint": …}`, both **prose**.

## `test`

One object for the run.

```json
{"results":[{"rule":"shipping_fee","lang":"python","vectors":68,"ok":true,"first_diff":null}],
 "skipped":[]}
```

`first_diff` is `null` when the run matched, and otherwise
`{"line":12,"generated":"…","expected":"…"}` — the first line of the canonical JSON on which
the generated code and the reference evaluator disagreed. `skipped` holds **prose** reasons a
language was not run at all (no toolchain).

## `verify`, `replay`, `diff`

The same shape for all three: they differ only in what the rule is compared against.

```json
{"compared":207,"matched":182,"rate":0.87923,"counterpart":"legacy@fake-1",
 "clusters":[{"rows":[{"table":"サイズ判定","row":1},{"table":"運賃表","row":36}],
              "count":7,
              "delta":{"運賃":{"min":-10,"max":-10,"uniform":true,"total":-70}},
              "witness":{"in":{"あて先":"沖縄県","三辺合計":1,"重量":1},
                         "ours":{"運賃":1450},"theirs":{"運賃":1460}},
              "suspect_rounding":false}],
 "excluded":{},"filled":{"count":0,"by_field":{},"defaults":{}},"unanswered":0}
```

| field | meaning |
|---|---|
| `compared` | observed records compared. **Filled records are not counted here** (§10.3) |
| `matched` | of those, how many agreed |
| `rate` | `matched / (compared − unanswered)`, as a fraction |
| `counterpart` | who the rule was compared against: an adapter's self-reported id, a fixtures path, or `送料@v3 → 送料@v4` |
| `unanswered` | records the counterpart declared it could not answer. Excluded from the denominator |
| `clusters` | mismatches grouped by the rows that fired |
| `excluded` | records dropped before comparison, keyed by a stable reason: `missing_field`, `bad_format` |
| `filled` | `count`, `by_field` (field → how many records were filled), `defaults` (field → the value used). §10.3 requires the report to carry this |

A cluster's `rows` entry is `{"table":…,"row":…}` for `verify` and `replay`; for `diff` it is
`{"table":…,"from":…,"to":…}`, the transition of the fired row between the two versions. A
row that exists on one side only has `from` or `to` set to `null`.

`delta` is keyed by output name, because a rule can have several outputs and they move
independently. `uniform` is true when every record in the cluster moved by the same amount,
which is what folds a cluster into one line in the text rendering.

`suspect_rounding` is true when **every** differing record in the cluster differs by less
than the output's own rounding grid — a difference in rounding convention rather than in the
values (§10.4). It is a conjunction over the whole cluster, so values that really differ are
never blamed on rounding.

## `fixtures lint`

One object for the run.

```json
{"file":"replay/2025-08.jsonl","records":208,"observed":208,"filled":0,"dropped":0,
 "problems":[{"kind":"bad_input","field":"あて先","count":1,
              "example":{"line":21,"tag":"order:b3"},
              "what":"`in.あて先`: `江戸` is not a value of enum 都道府県",
              "hint":"The type or range disagrees with the declaration."}]}
```

`kind` is one of `not_json`, `no_in`, `no_observed`, `unknown_field`, `bad_input`,
`bad_observed`, `missing_observed`. `field` is the field of the record at fault, or absent
when the problem is about the record as a whole. `what` and `hint` are **prose**; `kind`,
`field`, `count` and `example` are not.

## `schema`, `adapter`, `vectors`

These three are already machine-readable and take no `--format`.

- `schema` prints one JSON Schema for the wire an adapter speaks.
- `adapter` prints a Python or Go source template.
- `vectors` prints JSON Lines: one test case per line.

## `doc`

`doc` has no `--format json`. It renders for the person who approves a change, and markdown
is that shape.
