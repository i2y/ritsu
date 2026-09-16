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
| `code` | the stable code. `rulec explain <code>` describes it |
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
{"results":[{"rule":"shipping_fee","lang":"python","via":"runner","vectors":68,
              "ok":true,"ran":true,"first_diff":null,"error":null},
             {"rule":"shipping_fee","lang":"python","via":"mcp","vectors":68,
              "ok":true,"ran":true,"first_diff":null,"error":null},
             {"rule":"shipping_fee","lang":"go","via":"runner","vectors":68,
              "ok":false,"ran":false,"first_diff":null,
              "error":"does not compile:\n…"}],
 "skipped":[]}
```

| field | meaning |
|---|---|
| `via` | how the generated code was reached: `runner`, the vectors piped through the generated runner, or `mcp`, one `tools/call` per vector through the generated server ([generated-code.md](generated-code.md#the-rule-as-an-mcp-tool)) |
| `ok` | the generated code and the reference evaluator agreed on every vector |
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
{"compared":207,"matched":182,"rate":0.87923,"counterpart":"legacy@fake-1",
 "clusters":[{"rows":[{"table":"サイズ判定","row":1},{"table":"運賃表","row":36}],
              "count":7,
              "delta":{"運賃":{"min":-10,"max":-10,"uniform":true,"total":-70}},
              "witness":{"in":{"あて先":"沖縄県","三辺合計":1,"重量":1},
                         "ours":{"運賃":1450},"theirs":{"運賃":1460}},
              "suspect_rounding":false}],
 "moved":[],
 "excluded":{},"filled":{"count":0,"by_field":{},"defaults":{}},"unanswered":0}
```

| field | meaning |
|---|---|
| `compared` | observed records compared. **Filled records are not counted here** (§10.3) |
| `matched` | of those, how many agreed |
| `rate` | `matched / (compared − unanswered)`, as a fraction |
| `counterpart` | who the rule was compared against: an adapter's self-reported id, a fixtures path, or `送料@v3 → 送料@v4` |
| `unanswered` | records the counterpart declared it could not answer. Excluded from the denominator |
| `clusters` | mismatches grouped by the rows that matched |
| `moved` | `replay` only: records whose values matched but whose recorded rows differ from the rule's, in the same shape as `clusters` with an empty `delta`. Only a record carrying a `trace` can appear here. They count as matched |
| `excluded` | records dropped before comparison, keyed by a stable reason: `missing_field`, `bad_format` |
| `filled` | `count`, `by_field` (field → how many records were filled), `defaults` (field → the value used). §10.3 requires the report to carry this |

A cluster's `rows` entry is `{"table":…,"row":…}` for `verify` and for `replay` over records
without a `trace`; for `diff` it is `{"table":…,"from":…,"to":…}`, the transition of the row
that matched between the two versions, and `replay` uses the same transition for a record
that carries a `trace` — `from` is the recorded row, `to` the rule's. A row that exists on one
side only has `from` or `to` set to `null`.

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
`bad_observed`, `missing_observed`, `bad_trace`. `field` is the field of the record at fault, or absent
when the problem is about the record as a whole. `what` and `hint` are **prose**; `kind`,
`field`, `count` and `example` are not.

## `api`

One object. It says how to call the generated code without reading it; the field-by-field
meaning is in [generated-code.md](generated-code.md).

```json
{"rule":"クーポン一枚","alias":"coupon_step","version":"1","source_sha256":"…",
 "python":{"module":"coupon_step","mcp":"coupon_step_mcp.py","function":"coupon_step",
           "signature":"def coupon_step(subtotal: YenInclTax, …) -> Output:",
           "traced":"coupon_step_traced",
           "traced_signature":"def coupon_step_traced(subtotal: YenInclTax, …) -> tuple[Output, list[Fired]]:",
           "record":"coupon_step_record",
           "record_signature":"def coupon_step_record(subtotal: YenInclTax, …, out: Output, trace: _Trace, tag: str = \"\") -> str:",
           "params":[{"name":"商品合計","alias":"subtotal","type":"YenInclTax","unit":"円",
                      "range":{"min":0,"max":1000000},"optional":false}],
           "returns":"Output",
           "outputs":[{"name":"素割引","alias":"raw","type":"YenInclTax","unit":"円",
                       "optional":false,"rounding":{"mode":"down","grid":1}}],
           "enums":[{"name":"クーポン種別","alias":"CouponKind",
                     "values":[{"name":"率引き","alias":"PERCENT"}]}],
           "errors":["RuleInputError","RuleContradictionError"]},
 "typescript":{"module":"coupon_step.ts","mcp":"coupon_step_mcp.ts","function":"coupon_step",
               "signature":"export function coupon_step(subtotal: YenInclTax, …): Output",
               "params":[…],"returns":"Output","outputs":[…],
               "enums":[{"name":"クーポン種別","alias":"CouponKind",
                         "values":[{"name":"率引き","alias":"PERCENT"}]}],
               "errors":["RuleInputError","RuleContradictionError"]},
 "javascript":{"module":"coupon_step.mjs","mcp":"coupon_step_mcp.mjs","function":"coupon_step",
               "signature":"export function coupon_step(subtotal, applied, kind, rate, face, dup)",
               "params":[…],"returns":"Output","outputs":[…],"enums":[…],
               "errors":["RuleInputError","RuleContradictionError"]},
 "rust":{"module":"coupon_step.rs","function":"coupon_step",
         "signature":"pub fn coupon_step(subtotal: YenInclTax, …) -> Result<Output, RuleError>",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"クーポン種別","alias":"CouponKind",
                   "values":[{"name":"率引き","alias":"Percent"}]}],
         "errors":["RuleError::Input","RuleError::Contradiction"]},
 "ruby":{"module":"CouponStep","function":"coupon_step",
         "signature":"CouponStep.coupon_step(subtotal, applied, …)",
         "rbs":"sig/coupon_step.rbs",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"クーポン種別","alias":"CouponKind",
                   "values":[{"name":"率引き","alias":"PERCENT"}]}],
         "errors":["RuleInputError","RuleContradictionError"]},
 "go":{"package":"couponstep","func":"CouponStep",
       "signature":"func CouponStep(in Input) (Output, error)",
       "input_type":"Input","input_fields":[…],
       "output_type":"Output","output_fields":[…],
       "enums":[{"name":"クーポン種別","alias":"CouponKind",
                 "values":[{"name":"率引き","alias":"CouponKindPercent"}]}]},
 "swift":{"module":"coupon_step.swift","function":"couponStep",
          "signature":"func couponStep(subtotal: YenInclTax, …) throws -> Output",
          "params":[…],"returns":"Output","outputs":[…],
          "enums":[{"name":"クーポン種別","alias":"CouponKind",
                    "values":[{"name":"率引き","alias":"percent"}]}],
          "errors":["RuleError.input","RuleError.contradiction"]},
 "sql":{"file":"coupon_step.sql","input":"coupon_step_input","id":"_id","guard":"_input_error",
        "dialect":"postgresql","runs_on":["postgresql","sqlite"],
        "columns":[{"name":"商品合計","alias":"subtotal","type":"bigint","unit":"円",
                    "range":{"min":0,"max":1000000},"optional":false}],
        "outputs":[…],"rows":[{"table":"適用判定","column":"decide_row"}]}}
```

Everything here is a name or a number the generated code really uses, so nothing in it moves
with `--lang`. Every language's entry carries `traced` and `traced_signature` as the Python
one does — the twin that returns the rows that matched beside the outputs — and `record`
and `record_signature`, the function that writes one call as a fixtures record
([generated-code.md](generated-code.md#the-rows-that-matched)). `range` states the bounds **the entry guard enforces**, and `alias` states the
member spelling **that language** uses (`CouponKind.PERCENT` in Python and TypeScript,
`CouponKind.PERCENT` in JavaScript too, `CouponKind::Percent` in Rust, `CouponKind::PERCENT` in Ruby,
`couponstep.CouponKindPercent` in Go, `CouponKind.percent` in Swift; SQL spells no member, an
enum being its own name there). `unit`, `range` and `rounding` are absent when the
type has none. The Ruby entry also carries `rbs`, the path of the signature file that ships
with the module, and an entry whose language gets a server carries `mcp`, the file beside the
module that serves the rule as one MCP tool
([generated-code.md](generated-code.md#the-rule-as-an-mcp-tool)). The `sql` entry has no
function to name: it gives the file, the relation the query reads (`input`) and its `id`
column, the `guard` column that carries the entry guard's sentence, the `columns` of that
relation as the query declares them, the `outputs`, and under `rows` the column that carries
each table's matched row ([generated-code.md](generated-code.md#sql)).

## `schema` and `adapter`

Already machine-readable and take no `--format`.

- `schema` prints one JSON Schema for the wire an adapter speaks (see below). The
  description of an integer property says its unit and, for a rate, its step, so that a
  caller who never reads the rule knows that 18.3% at a step of 0.1% travels as 183.
  `--keys alias` names every property by its ASCII alias instead of the rule's name, with
  the name as the property's `title`, which is the shape an HTTP request body or a form
  wants; the wire itself, and the tool built on it, keep the names.
- `adapter` prints a Python or Go source template, 20 to 30 lines, to wrap a legacy
  implementation.

## `doc`

`doc` has no `--format json`. It renders for the person who approves a change, and markdown
is that shape; `--format html` is the same document as one page with a form on it, where the
generated JavaScript runs the case the approver types in.

## `mcp`

`rulec mcp` speaks the Model Context Protocol over stdio: one JSON-RPC 2.0 message per line
in and out. It is the command table in another syntax, so nothing here is a second
implementation of a command.

- **Tools.** One per command, named `rulec_<command>` (`rulec_check`, `rulec_gen`, …). The
  positional arguments become `files` (a list) or `file`, `dir`, `code`, `old` and `new`,
  `fixtures` and `rule`; every flag becomes a property named after it with `-` as `_`
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
server: `gen` writes `<alias>_mcp.py` and `<alias>_mcp.mjs` beside the module, a stdio
server with one tool named after the alias. Its `inputSchema` is the `in` object of `schema`
above; its result is one fixtures record (below), as text and as `structuredContent`; a call
the rule cannot take comes back with `isError` and the argument named; and `--record
<file.jsonl>` appends every answered call to that file as a record.
[generated-code.md](generated-code.md#the-rule-as-an-mcp-tool) has it.

---

# The data files

The three formats below are not command output: they are files that rulec reads and writes,
and they all use the same wire representation for a value.

## Vectors (`rulec vectors`, `generated/vectors/<alias>.jsonl`)

JSON Lines, one test case per line, generated from the boundaries of the rule (§9).

```json
{"in":{"注文日":"2026-03-31"},"out":{"区分":"改定前"},
 "trace":["table 期間判定 row 1"],
 "why":"boundary pair: table 期間判定 row 1 注文日 20543 inside"}
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

## Fixtures (`rulec fixtures lint`, `replay`, `diff`)

JSON Lines, one past record per line. Extracting them from production logs is the user's job;
rulec only validates types and ranges (§10.2).

```json
{"ts":"2025-08-14T09:12:33+09:00","tag":"order:1234567",
 "in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},
 "observed":{"送料":800},
 "trace":[{"table":"基本送料","row":3},{"table":"負担判定","row":3}]}
```

| field | required | meaning |
|---|---|---|
| `in` | yes | the inputs as they were at the time |
| `observed` | yes | the values that actually came out. **Every output is required** |
| `tag` | no | a label for the record, shown in witnesses |
| `ts` | no | when it happened |
| `trace` | no | the rows that matched when the record was made, `{"table":…,"row":…}` each, in table order. The generated code's record function writes it ([generated-code.md](generated-code.md#a-record-of-one-call)); `lint` checks that every table exists and every row is one the table has |

The generated code writes this line itself: every module has a record function that takes
the inputs, the outputs and the rows that matched and returns the record, so a log of the
generated code needs no extraction. What still has to be extracted is a log of an
implementation rulec did not generate.

A number must be an integer in the canonical unit; a decimal is refused, naming the field.
A field the rule does not know is an error, not something to ignore — discarding it silently
would turn a misspelling into "filled with the default value", and only the match rate would
move.

**Fixtures are not committed to a repository**: they hold order amounts. Pass them to CI as
an artifact or from protected storage.

## The replay manifest (`--manifest`)

One JSON object declaring the default values used to fill a missing field (§10.3).

```json
{"rule":"送料","fills":{"重量":1000,"会員":"一般"}}
```

`rule` is optional and, when present, must match the rule being replayed. Every key of
`fills` must be an input of the rule, and its value is in the canonical unit. A record
missing a field that `fills` does not cover is **excluded outright** rather than guessed at;
a record filled from `fills` is marked a filled record, counted separately, and kept out of
the headline match rate. `--fill 欄=値` overrides one entry on the command line, for a
sensitivity run; it applies after the manifest.

The manifest holds only field names and default values, so unlike the fixtures it can be
committed.

## The adapter protocol (`rulec verify`)

The legacy implementation is started as a child process and JSON Lines flow over
stdin/stdout. No FFI and no network: a process and line-oriented JSON is the smallest surface
that is written the same way in every language (DESIGN §10.1 has the reasoning).

```
rulec → {"rulec":"adapter/1","rule":"送料","in":["届け先","重量","注文金額","会員"],"out":["送料"]}
legacy ← {"ok":true,"impl":"legacy/shipping.py@a1b2c3d"}
rulec → {"id":1,"in":{"届け先":"北海道","重量":2500,"注文金額":12000,"会員":"ゴールド"}}
legacy ← {"id":1,"out":{"送料":1800}}
legacy ← {"id":2,"err":"unsupported: 離島"}
```

1. rulec writes one handshake line naming the rule and its input and output names.
2. The adapter answers `{"ok":true,"impl":"…"}`. `impl` identifies the build being compared
   and is reported as `counterpart`. Anything but `ok: true` aborts the run.
3. For each vector rulec writes `{"id":…,"in":{…}}` and reads one line back.
4. The answer is either `{"id":…,"out":{…}}` or `{"id":…,"err":"…"}`. A record the adapter
   declares it cannot answer is **excluded from the match-rate denominator** and reported as
   `unanswered`, so an adapter cannot raise the rate by refusing the hard cases.

Names and values on the wire are the rule's own names and integers in the canonical unit.
`rulec schema` prints the JSON Schema of `in` and `out`, and `rulec adapter --template
python|go` prints a template to fill in.
