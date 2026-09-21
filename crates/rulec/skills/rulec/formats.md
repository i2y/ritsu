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
`add_alias`, `mark_default`, `mark_contract_only`, `change_policy`, `add_expected`, `pin_source`, `none`.
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
{"file":"rules/送料.rule","vectors":70,"refused":0,
 "criteria":[{"name":"row","satisfied":7,"total":7,"missing":[]},
             {"name":"boundary_pair","satisfied":4,"total":4,"missing":[]},
             {"name":"shadow_pair","satisfied":3,"total":3,"missing":[]},
             {"name":"rounding_tie","satisfied":0,"total":0,"missing":[]},
             {"name":"fold_transition","satisfied":0,"total":0,"missing":[]}]}
```

`name` is one of `row`, `boundary_pair`, `shadow_pair`, `rounding_tie`, `fold_transition` — the last one has obligations only for a rule that walks a sequence (§15.56). An entry of `missing` is
`{"what": …, "hint": …}`, both **prose**. `refused` counts the cases in the suite that the
reference evaluator has **no answer** for; they discharge obligations like any other case, and
what is asked of the generated code there is that it refuse them too (below).

## `test`

One object for the run.

```json
{"results":[{"rule":"shipping_fee","lang":"python","via":"runner","vectors":68,"refused":0,
              "ok":true,"ran":true,"first_diff":null,"error":null},
             {"rule":"shipping_fee","lang":"python","via":"mcp","vectors":68,"refused":0,
              "ok":true,"ran":true,"first_diff":null,"error":null},
             {"rule":"shipping_fee","lang":"go","via":"runner","vectors":68,"refused":0,
              "ok":false,"ran":false,"first_diff":null,
              "error":"does not compile:\n…"}],
 "skipped":[]}
```

| field | meaning |
|---|---|
| `via` | how the generated code was reached: `runner`, the vectors piped through the generated runner; `mcp`, one `tools/call` per vector through the generated server over stdio; `mcp-http`, the same conversation over the same server's Streamable HTTP ([generated-code.md](generated-code.md#the-rule-as-an-mcp-tool)); `connect-asgi`, `connect-asgi-get`, `connect-wsgi` and `connect-wsgi-get`, one call per vector through the generated Connect service — the ASGI application and the WSGI one, each by POST and by GET ([generated-code.md](generated-code.md#the-rule-as-a-connect-service)); `wasi`, the Rust runner compiled for `wasm32-wasip1` and run under wasmtime ([generated-code.md](generated-code.md#the-rust-runner-as-a-wasi-module)); `function`, the rule as a function on a real PostgreSQL, called once per vector by argument name through `psql` ([generated-code.md](generated-code.md#sql)); or `proof`, the generated Rust read by a model checker ([generated-code.md](generated-code.md#the-proofs)) — the one way that is not the vectors, so its `vectors` is the number of harnesses and its `refused` is 0; the `wasm/` target itself is a language of its own in this list, reached through its runner, so `wasm` names a language here and `wasi` a way of reaching one |
| `vectors` | how many vectors were put to it — or, when `via` is `proof`, how many harnesses the checker read |
| `refused` | how many inputs with no answer were put to it. Each one is given on its own, and what is asked is that the run stop without an answer |
| `ok` | the generated code and the reference evaluator agreed on every vector, and refused every input the evaluator refuses |
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
 "python":{"module":"coupon_step","mcp":"coupon_step_mcp.py","page":"coupon_step_page.html","function":"coupon_step",
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
 "typescript":{"module":"coupon_step.ts","mcp":"coupon_step_mcp.ts","page":"coupon_step_page.html","function":"coupon_step",
               "signature":"export function coupon_step(subtotal: YenInclTax, …): Output",
               "params":[…],"returns":"Output","outputs":[…],
               "enums":[{"name":"クーポン種別","alias":"CouponKind",
                         "values":[{"name":"率引き","alias":"PERCENT"}]}],
               "errors":["RuleInputError","RuleContradictionError"]},
 "javascript":{"module":"coupon_step.mjs","mcp":"coupon_step_mcp.mjs","page":"coupon_step_page.html","function":"coupon_step",
               "signature":"export function coupon_step(subtotal, applied, kind, rate, face, dup)",
               "params":[…],"returns":"Output","outputs":[…],"enums":[…],
               "errors":["RuleInputError","RuleContradictionError"]},
 "rust":{"module":"coupon_step.rs","function":"coupon_step",
         "signature":"pub fn coupon_step(subtotal: YenInclTax, …) -> Result<Output, RuleError>",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"クーポン種別","alias":"CouponKind",
                   "values":[{"name":"率引き","alias":"Percent"}]}],
         "errors":["RuleError::Input","RuleError::Contradiction"],
         "proof":"coupon_step_proof.rs","harnesses":["coupon_step_answers","applicable_rows","raw_discount_rows"]},
 "ruby":{"module":"CouponStep","function":"coupon_step",
         "signature":"CouponStep.coupon_step(subtotal, applied, …)",
         "rbs":"sig/coupon_step.rbs",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"クーポン種別","alias":"CouponKind",
                   "values":[{"name":"率引き","alias":"PERCENT"}]}],
         "errors":["RuleInputError","RuleContradictionError"]},
 "php":{"module":"coupon_step.php","namespace":"CouponStep","function":"coupon_step",
        "signature":"function coupon_step(int $subtotal, int $applied, …): Output",
        "params":[…],"returns":"Output","outputs":[…],
        "enums":[{"name":"クーポン種別","alias":"CouponKind",
                  "values":[{"name":"率引き","alias":"CouponKind::PERCENT"}]}],
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
 "java":{"module":"CouponStep.java","class":"CouponStep","function":"couponStep",
         "signature":"public static Output couponStep(long subtotal, long applied, …)",
         "build":"javac --release 17 -encoding UTF-8 -d classes *.java",
         "params":[…],"returns":"Output","outputs":[…],
         "enums":[{"name":"クーポン種別","alias":"CouponKind",
                   "values":[{"name":"率引き","alias":"CouponKind.PERCENT"}]}],
         "errors":["RuleInputError","RuleContradictionError"]},
 "connect":{"proto":"proto/rulec/coupon_step/v1/coupon_step.proto","package":"rulec.coupon_step.v1",
            "service":"CouponStepService","method":"Decide",
            "path":"/rulec.coupon_step.v1.CouponStepService/Decide",
            "request":"DecideRequest","response":"DecideResponse",
            "idempotency_level":"NO_SIDE_EFFECTS","trace":"trace",
            "source_header":"rulec-source-sha256",
            "stubs":"cd proto && buf generate",
            "buf_yaml":"proto/buf.yaml","buf_gen_yaml":"proto/buf.gen.yaml",
            "json_names":"lowerCamelCase","json_int64":"string",
            "request_fields":[{"name":"商品合計","field":"subtotal","type":"int64"}],
            "response_fields":[{"name":"素割引","field":"raw","type":"int64"}],
            "python":{"module":"coupon_step_service.py",
                      "class":"CouponStep","sync_class":"CouponStepSync",
                      "asgi":"app","wsgi":"wsgi_app",
                      "serve_asgi":"uvicorn coupon_step_service:app --port 8080",
                      "serve_wsgi":"gunicorn 'coupon_step_service:wsgi_app'",
                      "client":"CouponStepServiceClientSync",
                      "runner":"coupon_step_connect_runner.py",
                      "needs":["connectrpc","buf"]}},
 "sql":{"file":"coupon_step.sql","input":"coupon_step_input","id":"_id","guard":"_input_error",
        "dialect":"postgresql","runs_on":["postgresql","sqlite"],
        "columns":[{"name":"商品合計","alias":"subtotal","type":"bigint","unit":"円",
                    "range":{"min":0,"max":1000000},"optional":false}],
        "outputs":[…],"rows":[{"table":"適用判定","column":"decide_row"}],
        "function":{"file":"coupon_step_function.sql","name":"coupon_step",
                    "signature":"\"coupon_step\"(\"subtotal\" bigint, …, \"dup\" boolean) RETURNS TABLE (\"ok\" boolean, …, \"raw_discount_row\" int)",
                    "language":"plpgsql","runs_on":["postgresql"],"raises":"22023"}},
 "wasm":{"source":"coupon_step_wasm.rs","module":"coupon_step.wasm",
         "build":"rustc --edition 2021 -C opt-level=s -C lto -C panic=abort -C strip=symbols --target wasm32-unknown-unknown --crate-type cdylib coupon_step_wasm.rs -o coupon_step.wasm",
         "wit":"coupon_step.wit","package":"rulec:coupon-step@1.0.0","world":"coupon-step",
         "call":"call","call_signature":"call: func(input: string) -> string",
         "post_return":"cabi_post_call","realloc":"cabi_realloc","memory":"memory",
         "runner":"coupon_step_runner.mjs",
         "component":"wasm-tools component embed coupon_step.wit coupon_step.wasm -o coupon_step.embedded.wasm && wasm-tools component new coupon_step.embedded.wasm -o coupon_step.component.wasm"}}
```

Everything here is a name or a number the generated code really uses, so nothing in it moves
with `--lang`. Every language's entry carries `traced` and `traced_signature` as the Python
one does — the twin that returns the rows that matched beside the outputs — and `record`
and `record_signature`, the function that writes one call as a fixtures record
([generated-code.md](generated-code.md#the-rows-that-matched)). `range` states the bounds **the entry guard enforces**, and `alias` states the
member spelling **that language** uses (`CouponKind.PERCENT` in Python and TypeScript,
`CouponKind.PERCENT` in JavaScript too, `CouponKind::Percent` in Rust, `CouponKind::PERCENT` in Ruby
and in PHP, `CouponKind.PERCENT` in Java, `couponstep.CouponKindPercent` in Go,
`CouponKind.percent` in Swift; SQL spells no member, an
enum being its own name there, and the Wasm module and the NumPy plan read and write the name itself, as the wire does). `unit`, `range` and `rounding` are absent when the
type has none. The NumPy entry is shaped differently from the rest, because it names no
function: it carries `plan` and `runtime`, the two files, `load`, `call` and `traced`, and
`columns` and `outputs` in place of parameters. The Ruby entry also carries `rbs`, the path of the signature file that ships
with the module, and an entry whose language gets a server carries `mcp`, the file beside the
module that serves the rule as one MCP tool
([generated-code.md](generated-code.md#the-rule-as-an-mcp-tool)). A rule that walks a
sequence (§15.56) has one more parameter, last in the list and last in every signature, with
the fields one element carries under `elements`:

```json
{"name":"運賃行","alias":"freight_rows","type":"list[Element]","optional":false,
 "elements":[{"name":"閾値","alias":"threshold","type":"YenInclTax","unit":"円",
              "range":{"min":0,"max":1000000},"optional":false}]}
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

## `certificate`

One object per rule: the **evidence** behind all five things `check` proves — completeness,
the overlaps, the unreachable rows, the units and int64 — small enough that a program which
shares no code with rulec can re-check it in milliseconds. Two such programs read it.
`tools/recheck.py` has no dependencies and fits in one file. `proofs/` is a Lean 4
development that states the meaning of a rule, writes the checks as functions, and
**proves** that a `true` from each one settles the matching claim; the program `lake build`
produces runs those very functions, so what it prints is the theorems applied to one
document. The tests hold both to forged certificates as well as to the corpus.

```json
{"rule":"クーポン併用","alias":"coupon_stack","version":"1","source_sha256":"094dba24753a…","rulec":"0.15.0",
 "ranges":{"合計":["0","1000000"],"割引A":["0","100000"],"残高A":["-100000","1000000"]},
 "constraints":[],
 "values":[{"name":"残高A","expr":{"op":"-","l":{"name":"合計"},"r":{"name":"割引A"}},
            "interval":["-100000","1000000"],"scale":1,"stored_max":"1000000"}],
 "tables":[{"table":"適用判定","policy":"unique",
   "axes":[{"column":"残高A","kind":"derived","coords":["999円","1000円","1001円"],"step":"1",
            "bounds":[[null,"1000"],["1000","1000"],["1000",null]]}],
   "outputs":1,
   "rows":[{"row":1,"label":"","cells":["<= 1000円","-"],
            "tests":[{"cell":"cmp","tests":[{"op":"<=","value":"1000"}]},{"cell":"any"}],
            "origin":"適用判定","line":21,
            "source":[{"line":21,"col":2,"len":9,"text":"<=1000円"},{"line":21,"col":15,"len":1,"text":"-"}],
            "accepts":[[0,1],[0,1,2]]}],
   "disjoint":[{"a":1,"b":3,"axis":0},{"a":2,"b":3,"axis":0}],
   "undecided":[{"a":1,"b":2}],
   "reach":[{"row":1,"at":[0,0],"values":{"残高A":999,"残高B":3979},"at_values":["999","3979"]}],
   "unused":[],"unreachable":[],
   "constraints":[],
   "cover":{"split":[{"row":1},{"row":1},{"split":[{"row":3},{"row":2},{"row":2}]}]}}]}
```

| field | meaning |
|---|---|
| `types` | every name's declared type, and `groups` every group's members. A row's box and a value's type are **derived** from these by the re-checker, not taken from the certificate |
| `ranges` | every name's declared range, as exact rationals (`"7/2"`, an open end `null`). The int64 claim is re-checked from these |
| `constraints` | the `constraint` lines of the rule, once for the whole document: `left`, `op`, `right`. They are what the caller guarantees and the entry guard enforces (§15.55), and a value's interval can rest on one — the share of `allocate` is bounded by the amount only because a running total never passes the whole (§15.102). A re-checker reads them the way rulec does, chains included, and refuses an interval it cannot then derive. Each table repeats the ones its own columns are about, under `tables[].constraints` |
| `values` | every value the rule computes — `derive`, `define` and `result` alike: the type the rule declares for it, the expression as a tree, the interval the ranges and the guarantees force it into, the scale it is stored at, and the integer that interval reaches. A value with no interval to state (a truth value, an enum) says `null` for all three, and a re-checker refuses that for a type that is stored as an integer. Two things are re-checked from this: the units (§2.1, E103), by deriving each node's type from the leaves up — a name's from `types`, a literal's from the `type` it carries — and int64 (§7.4, E108), by interval arithmetic over the same expression. A literal also carries the value its unit resolves to, so the re-checker does arithmetic and not units |
| `axes` | the universe, one axis per column of the table, each with the coordinates the boundaries compress it to (§6.2), for a numeric axis each coordinate as a closed interval in `bounds` and the `step` its values sit on, and for a `string` axis the prefix each coordinate stands for in `prefixes` (`null` for the one coordinate that is under none of them). `kind` is `input`, `derived`, `define`, `walk` (what a `count` or a `sum` left behind) or `upstream`: a point on an axis of inputs is a value a caller can send, and on any other axis it is a point the feasibility sieve could not rule out, which is weaker. A re-checker holds a numeric axis to §6.2's construction: the coordinates run from the declared range's low end to its high end, each touching the next or one `step` past it, with nothing between — so a coordinate cannot be quietly removed and the gap under it left uncovered |
| `rows` | each row as a **box**: the coordinates it accepts on each axis, in `accepts`, beside the cells it was written with and, in `tests`, those cells resolved as far as their units — `{"cell":"cmp","tests":[{"op":"<=","value":"1000"}]}`, `{"cell":"is","words":["近畿圏"]}`, `{"cell":"prefix","words":["CH-"]}`, `{"cell":"any"}`. The re-checker **recomputes** the box from `tests` and the axis bounds and refuses a box that is not what the cell describes |
| `origin`, `line` | the table the row was written in, and the line it is written on. Rows of one table have a cell in the same columns and in no others, are all written in this file or all brought in by an `apply`, and take a run of lines in row order that no other table's rows fall inside — all of which a re-checker holds them to |
| `source` | where each cell stands in the `.rule` file — `line`, byte `col`, byte `len` — and the text that stands there. With `--rule` a re-checker reads the file and compares, and the span has to be that cell's own place: every cell of a row is on the row's `line`, they are that line's `|`-separated fields, all of them (`outputs` says how many of the line's fields are answers rather than cells), and none of them is empty. `null` for a row an `apply` brought in, which is written in another file; `null` for one cell where there is nothing to point at — a column a `clause` does not mention, or one a merged member table does not have, and then every row of that table has to agree |
| `disjoint` | `unique` only: for each pair of rows, one axis on which their coordinates do not meet. Re-checking one entry is one set intersection |
| `undecided` | the pairs the check could not settle either way — the W114 warning, stated rather than proved. A pair in neither list is a certificate that does not hold |
| `reach` | for each row, a point inside it: `at` is the coordinate on every axis, `values` the same point in the table's columns, and `at_values` those values as plain numbers on the axes' own scale — which is what lets the re-checker show the point is one the sieve admits, and not merely one inside the row's box. Under `policy first` the point is also outside every row above it |
| `unused`, `unreachable` | rows outside the reachability claim, named rather than passed over: ones an `apply` brought in that this rule's bindings leave unused (§15.69), and ones the sieve rules out entirely — E102 does not look at the sieve, so `check` passes those and the certificate says so. A row called unused has to be one written in another file, which the `source` of that row shows |
| `cover` | completeness (E101) as the walk of §6.3, written down. A `split` has one child per coordinate of the axis at its depth — so the children tile the axis by shape, not by a claim — and every leaf is `{"row":n}`, a row that takes the whole subtree, or a box no input reaches: `{"constraint":k}`, the `constraint` that cannot hold there, `{"derived_axis":i}`, a derived value whose coordinate lies outside its declared range, or `{"every_point_ruled_out":true}`, a box whose points the sieve rules out one at a time (§15.98). `{"upstream":…}` is **stated, not proved**: re-checking one needs the upstream table's own region, which this certificate does not carry. `null` when the walk ran past the budget |
| `constraints` | the `constraint` lines a cover leaf points at |

**What "proved" means here.** `proofs/RulecCert/Semantics.lean` says what a table claims:
under `unique`, every point the rule is **asked about** is taken by exactly one row; every
row answers somewhere; and a value keeps the type it is declared with and fits int64. Asked
about means the values behind the point satisfy every `constraint` the rule declares and put
every derived column inside the interval its own expression forces — which is what rulec
decides, and no more: whether some real input produces a given derived value is not settled
by anything here. Each check is then a theorem: `Certified.unique`, `Certified.complete`,
`Certified.reached`, `eval_type_of_typeOf`, `stored_in_i64`. One of them,
`mem_boxOf_cmp_iff`, is what ties the boxes to the cells: a coordinate is taken exactly when
every value in it satisfies the cell, **provided** no value a cell compares against falls
strictly inside a coordinate — §6.2's construction, which the checkers verify rather than
assume.

**Where it stops.** The file is tied to the document by its digest and, cell by cell, by the
byte spans above. Everything else the certificate says about the rule is **its own word**,
and no re-checker can go behind it without parsing the `.rule` file — which neither does, on
purpose: a checker that reads a rule the way rulec reads it is not independent of it. So
these are stated, not derived: the declared `ranges` and `types`, the `groups`, the
`constraints`, each value's `expr` and `scale`, and how many `outputs` a table has. A forged
one of those is a forged rule, not a forged proof about the rule in front of you.

Five more things are named in the run rather than proved, and both programs end with a line
that lists them rather than printing a clean "ok": a cover leaf resting on an upstream table,
the pairs W114 could not settle, rows an `apply` brought in, rows the sieve rules out, and a
point handed over with no values behind it. One thing is counted: a cell literal written in a
unit the axis does not write its own coordinates in (`2kg` against an axis of grams), where
pinning the number would take the lexer's unit table. A literal in the axis's own unit has to
be one of its boundaries or lie outside it altogether (§6.2).

`tools/recheck.py` parses the cell text it reads out of the file and holds the certificate's
operators and numbers to it; the Lean program does not, and recomputes the box from the
parsed form instead. A rule that does not pass `check` produces no certificate at all.

```console
$ rulec certificate rules/健康保険料.rule > cert.json
$ python3 tools/recheck.py --rule rules/健康保険料.rule cert.json
健康保険料 (kenpo_premium v1, sha256:5d4974d65bdb) — certificate by rulec 0.15.0
  units: 4 values keep the type the rule declares
  int64: 4 values fit
  等級: unique, 50 rows — 1225 pairs disjoint, 50 rows reached, 101 boxes covered, 50 boxes read back from their cells, 1 axes tiled
  適用料率: unique, 2 rows — 1 pairs disjoint, 2 rows reached, 2 boxes covered, 2 boxes read back from their cells, 0 axes tiled
  the digest is rules/健康保険料.rule's
  the file says the same: 52 cells read back from it
  every claim this program states was proved

$ (cd proofs && lake build) && proofs/.lake/build/bin/rulec-recheck --rule rules/健康保険料.rule cert.json
健康保険料 (0.15.0), re-checked against the Lean proofs
  values: 4 typed, 4 held to int64, 0 not re-checked
  等級: 50 rows — complete, 50 rows reached, no two rows meet, 1 axes tiled
    50 boxes read back from the cells they were written as
  適用料率: 2 rows — complete, 2 rows reached, no two rows meet
    2 boxes read back from the cells they were written as
  the digest is rules/健康保険料.rule's, and 52 cells are read back out of it
OK: every claim this program states was proved, by the theorems in RulecCert.Sound.
```

Exit code 0 when every table holds and 1 when a claim does not; `tools/recheck.py` answers 2
for a certificate it cannot read at all.

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

`--audience customer` renders the same rule as the article a help centre publishes: the
inputs in plain words, the tables with `-` as "any" and `not:` as "other than", the rounding
as a sentence, and **the cases on either side of every threshold** — the boundary-pair
vectors of `rulec vectors`, one line per pair. Aliases, declared ranges, diagnostic codes and
the list of what `rulec check` verified are left out: they are for the approver. It is
markdown only (`--format html` cannot be combined with it), and under `--out` it is written
as `<alias>.customer.md`, beside the approver's `<alias>.md`.

## `mcp`

`rulec mcp` speaks the Model Context Protocol over stdio — and only over stdio, because what
it hands out is the commands that read and write your files. One JSON-RPC 2.0 message per
line in and out. It is the command table in another syntax, so nothing here is a second
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
server: `gen` writes `<alias>_mcp.py` and `<alias>_mcp.mjs` beside the module, a server with
one tool named after the alias. Its `inputSchema` is the `in` object of `schema` above; its
result is one fixtures record (below), as text and as `structuredContent`; a call the rule
cannot take comes back with `isError` and the argument named; and `--record <file.jsonl>`
appends every answered call to that file as a record. **Unlike this one it is not stdio
only**: `--http <port>` serves the same tool over MCP's Streamable HTTP, and where the host
renders MCP Apps it offers the approver's page as the tool's view.
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

A rule that walks a sequence (§15.56) can have inputs the reference evaluator **refuses**:
two elements both taking under `take_unique` is a contradiction, and there is no answer to
expect. Those go to a third file, `<alias>.refused.jsonl`, written only when there are any:

```json
{"in":{"運賃行":[{"行ゾーン":"近畿圏","閾値":1000,"行運賃":100000},
                 {"行ゾーン":"近畿圏","閾値":1000,"行運賃":100000}]},
 "refused":"contradiction","why":"確定 then 確定"}
```

`in` is the same shape the vectors file uses, so the same runner reads it. `rulec test` puts
each of them in **on its own** — the generated code raises on the first one it is given — and
the run is green only if every language stops without an answer. A generated MCP server is
asked the same thing and has to answer `isError`. `rulec verify` does not use this file: it
asks an implementation for answers, and here there is none to compare.

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
| `by` | no | where an input came from, when it was not read as it stood ([below](#where-a-value-came-from-by)). **rulec does not read it** |
| `trace` | no | the rows that matched when the record was made, `{"table":…,"row":…}` each, in table order, with `"label"` added for a row that carries one; a `clause` is the one row of a table named after it. The generated code's record function writes it ([generated-code.md](generated-code.md#a-record-of-one-call)); `lint` checks that every table exists and every row is one the table has |

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

### Where a value came from (`by`)

Not every input is read from a system. Some are **decided**: a class picked out of the input's
own enum by a model, a yes/no that came back as a probability, a grade somebody typed. The rule
is a pure function either way — what was decided is passed in as a value, the way the time and
the stock are (§10.3) — so the record already keeps *what* the value was. `by` keeps **who gave
it**, and how sure they were:

```json
{"ts":"2026-09-17T09:12:33+09:00","tag":"ticket:88231",
 "in":{"確信度":934,"区分":"請求"},
 "by":{"区分":{"src":"jev","ver":"2026-09-16","conf":934}},
 "observed":{"扱い":"自動"},
 "trace":[{"table":"振り分け表","row":2}]}
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

**When the legacy implementation is a Connect service** it is not a process to start but an
endpoint to call, and `--template connect-python` prints that shape instead: the same
JSON Lines on stdin and stdout, with a client in the middle. Two places are left to fill in,
and both are the other side's own messages. A `ConnectError` becomes `err`, so a record that
service says it cannot answer leaves the denominator like any other.

```console
$ rulec adapter rules/送料.rule --template connect-python > adapter.py
$ rulec verify rules/送料.rule --adapter python3 adapter.py https://pricing.internal
Compared 96 / matched 96 (100.000%)
Counterpart: connect@https://pricing.internal
```

## The extraction protocol (`rulec source fetch --via`)

A PDF or a scan needs an extractor that rulec is not: the formats it reads itself are csv, md,
xlsx and docx. The extractor is a child process, as the legacy implementation is — one
direction and one shot, because an extraction is one question.

```
rulec → ./extract.py 料金表.pdf
extractor ← {"rulec":"extract/1","impl":"docling 2.4.0"}
extractor ← {"block":"table","page":12,"grid":[["あて先","運賃"],["近畿","990円"]]}
extractor ← {"done":true}
```

1. rulec runs the command with the document's path appended to it.
2. The first line names the protocol and **the extractor itself**. `impl` is required and is
   written to `<document>.fragments/extractor.txt`, where `rulec doc` reads it and tells the
   approver who read the document — a table a model read out of a scan is evidence of a
   different kind from one that was already a grid.
3. Each `{"block":"table",…}` line carries a `grid` of rows of strings, in document order; the
   `n`th of them is the fragment `表n`. `page` is optional and appears in the report. Blocks of
   any other kind are read and let go, so an extractor that also reports headings needs no
   flag.
4. `{"done":true}` ends the stream. **It is required**: an extractor that died half way would
   otherwise hand back the tables it managed, and `表3` would quietly be a different table.
   A missing handshake, a missing `done` and a non-zero exit all fail the command (exit 2)
   and leave the copies that were there untouched.

`rulec adapter <file.rule> --template docling` prints a template to fill in, naming the
documents of that rule that need one. The extractor is used **only** where rulec cannot read
the document itself: the formats it does read are read the same way every time, and a `--via`
that quietly rewrote those copies would make the pins depend on who ran it.
