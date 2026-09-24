# Generate and call

```console
$ rulec gen rules/ --out generated/
```

Out comes an ordinary module in Python, TypeScript, JavaScript, Rust, Ruby, PHP and Swift, a class in Java, an ordinary Go package, a query and a function in SQL,
one Wasm module, and a plan for NumPy. No runtime to install, no configuration, and no
dependency beyond the standard library — that last one is a **checked**
property, not a claim: `rulec test` runs the Go side with `GOPROXY=off`. There are two
exceptions. The NumPy plan is not code but the rule itself, read by one fixed evaluator that
needs `numpy`, which a host deciding whole columns at once already has; and the Connect
service needs `connectrpc` (below).

A rule that does not pass `check` generates nothing.

## Output languages

Twelve are supported today — Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift,
Java, SQL, Wasm and NumPy. The point is that one table should be able to give the
front end, the back end, the mobile app and the database the same answer,
and that this is *provable* through the agreement check that already
exists.

| | status | needs |
|---|---|---|
| Python | supported | `python3` |
| TypeScript | supported | just `node` — no build step, no tsconfig |
| JavaScript | supported | just `node`, or a browser — the TypeScript with its types taken off, as an ES module |
| Rust | supported | just `rustc` — no cargo, no crates |
| Ruby | supported | `ruby` 3.x or 4.x — `json` is standard library, so no gem, and a `.rbs` ships beside the module |
| PHP | supported | `php` 8.2 or newer — `ext/json` is built in, so no composer; native enums, typed parameters, and `intdiv` for every division |
| Go | supported | `go` |
| Swift | supported | just `swiftc` — no SwiftPM, no `Package.swift`; units ride in the type as they do in Rust |
| Java | supported | a JDK — `javac` and `java`, no Maven and no Gradle. Built at `--release 17`, so 17, 21 and 25 all take it; Kotlin and Scala call the class as it stands |
| SQL | supported | `python3`, whose standard-library `sqlite3` runs the agreement check for the query, and a `psql` that reaches a PostgreSQL for the function beside it; both are written for PostgreSQL. **A rule that walks a sequence is the one thing it does not get** |
| Wasm | supported | `rustc` with the `wasm32-unknown-unknown` target, and `node` for the agreement check. The module itself imports nothing |
| NumPy | supported | `python3` and `numpy`. Not generated code but the rule as data beside one fixed evaluator, which builds a closure over whole columns when the plan is loaded. **A rule that walks a sequence is the one thing it does not get** |

One rule governs all of them: **a language that cannot join the
byte-for-byte agreement check does not go in.** Generated code that
cannot be held against the reference evaluator sits outside the claim
this tool makes. Adding the third, TypeScript, cost about 700 lines in
the generator, and the fifth, Ruby, cost the same. So did the sixth, Swift. What differed was
everything outside the generator: Ruby took twenty-odd files edited by hand — seven lists of
the languages in the code and the tests, two diagram generators and nine documents — where
Swift, once those lists had become the one registry the tool now keeps of its own targets,
took one row in it and nothing else in the code.

### A target that is not on the list

You are not limited to the list, and waiting for a backend is not the only
way in. A rule already publishes everything a generator needs — `rulec api`
gives the names, units, ranges and rounding, `rulec schema` gives the wire
— so you can emit whatever your target needs from your own tool.

**The comparison comes with you.** Wrap the result in the adapter protocol
and `rulec verify` will run it against every case built from the rule's own
boundaries, exactly as it does for the supported ones. Nothing in rulec changes.

[Other targets](backends.md) runs that loop end to end against SQL, by
hand, as it was done before SQL had a backend of its own: 88 cases, all
agreeing — and then one threshold broken on purpose, to show the report
naming the rows and the case that proves it.

### SQL is a query, and the same query as a function

SQL gives you two doors on one rule. The first is **one query over a
relation of inputs**: provide `shipping_fee_input` with a column `_id`
and one column per input under its alias, and out come `_id`, the
inputs, the outputs, and one column per table with the number of the row
that matched. A row of the table is a `WHEN`; the rounding is arithmetic
in the final `SELECT`; a million rows go through in one statement, which
is what a closing batch, a recalculation or an analyst's figures need and
what the per-row functions cannot do. Make it a view and the rule is a
table in the database.

The query is written for PostgreSQL and kept inside what SQLite also
runs, which is how `rulec test` holds it to the reference evaluator with
nothing but `python3`. The values are the wire's — integers in the
declared unit, an enum as its name, a date as days since 1970-01-01 —
and the header of the file says which for every column. A query cannot
stop, so the entry guard is a column too: `_input_error` is NULL for a
row inside the declared domain and carries the sentence for one outside
it.

The second door is `shipping_fee_function.sql`: the same query as a
function, for one case asked for by name. Its body is the query above,
unchanged, with one CTE in front binding the arguments into a one-row
input relation — so the two cannot drift apart. Unlike the query it
**raises** where the others raise, because a query cannot stop and an RPC
client that reads one field should not be handed a number that looks like
an answer beside an error column it ignored. Put it in a schema PostgREST
or Supabase exposes and the rule is an endpoint with no server code of its
own: `POST /rpc/shipping_fee` with `{"dest": "北海道", …}` answers
`[{"fee":1200,…}]`, and an input outside the declaration comes back as a
**400** carrying the rule's own sentence. This one is
PostgreSQL only — SQLite has no `CREATE FUNCTION` — so `rulec test` runs
it on a real PostgreSQL through `psql`, and says so when it skips it for
want of a server.

**A rule that walks a sequence is not generated for SQL.** One query has
no place to carry a value from row to row and stop partway. That is a
decision rather than a gap, so `gen` says so for that rule and writes the
other targets.

## What the output looks like

Every row of every table becomes one branch, in order, with the original
cells quoted beside it.

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
		trace = append(trace, Fired{"サイズ判定", 1})
	} else if int64(in.Girth) <= 80 { // row 2: <=80cm | S80
		size = SizeClassS80
		trace = append(trace, Fired{"サイズ判定", 2})
	} else if true { // row 3: - | S100
		size = SizeClassS100
		trace = append(trace, Fired{"サイズ判定", 3})
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
fixtures format that `replay` and `diff` read, so a log written by the generated code is
already the records the next revision is diffed over.

Four rules keep it readable.

- **NumPy is outside all of this**, because it is not generated code: the rule travels as
  data and one fixed evaluator reads it.
- **No cell is elided.** A condition an earlier branch already settled is
  still written (`elif True:`), because reading the generated code
  against the source side by side is the only way it is meant to be read.
- **Units live in the type** wherever there is a type to hold them: a
  newtype in Rust, a one-field struct in Swift, a defined type in Go, a
  branded `bigint` in TypeScript, a `NewType` in Python, and the Rust
  newtype again in the Wasm module. Confusing `YenInclTax` with
  `YenExclTax` stops at compile time. Ruby, PHP, JavaScript, Java and SQL
  have nowhere to put a unit, so there it is documented instead — though
  PHP and Java still declare every parameter's kind, which is why their
  entry guard asks about the range alone.
- **Rounding goes through its own helper**, because Python's and Ruby's
  integer division rounds toward −∞ while Rust, Swift, Go, TypeScript,
  JavaScript and SQL truncate toward zero.
- **No builtin is called bare.** An input aliased `min` or `list` does
  not break the output: `_min`, `_max` and `_isinstance` are generated.

That the reference evaluator and every generated language answer the same
is checked by streaming automatically generated boundary cases through all
of them and comparing **canonical JSON byte for byte**.

## How to call it, without reading it

```console
$ rulec api rules/クーポン一枚.rule | jq -r .python.signature
def coupon_step(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> Output:
```

One JSON object: module and function names, the parameters in order with
their brands, units and ranges, the outputs with their rounding, the
enum members **under the spelling each language gives them**
(`CouponKind.PERCENT` in Python, `couponstep.CouponKindPercent` in Go),
and the errors that can come out.

A calling convention written by hand goes quietly wrong the day a name
changes, so this one is built next to the emitters and held to the
generated files by tests: every name it states must occur in the file;
Python imports the module and compares `inspect.signature`; and Go
**builds a calling program out of the inventory alone** and runs
`go vet` over it, which does not compile if a single name is wrong.

[The generated code in detail](generated-code.md){ .md-button }

## Calling it with the caller's own object

A rule whose inputs say `from` (see [Inputs taken from the caller's object](tour.md) in
the walkthrough) gets one more function beside the rule's own: the same call, taking the
caller's object instead. Its name is the rule's function with `_from` on the end.

```python
from order_shipping import order_shipping_from

order = {
    "shipping": {"zone": "okinawa"},
    "lines": [{"sku": f"s{i}", "chilled": i == 3, "amount_jpy": 100} for i in range(11)],
}
print(order_shipping_from(order))  # 2200
```

From a `.proto`, hand it the JSON protojson wrote, as it is: the names may stay in
lowerCamelCase, a field at its default may be left out, and an int64 may arrive as a string.

```python
import json
from shipment_fee import shipment_fee_from

body = """{"destination": {"region": "okinawa"},
 "parcels": [{"weightG": "1200"}, {"weightG": "800"}, {"weightG": "500"}],
 "declaredValueJpy": "200000", "deliveryWindow": "evening"}"""
print(shipment_fee_from(json.loads(body)))  # 5100
```

It is written for five of the twelve targets. In Python, TypeScript, JavaScript, Ruby and
PHP, parsed JSON is usually used as it comes, as a plain map (a `dict` in Python, a `Hash`
in Ruby), so the function takes a plain map too, and never needs to know the caller's type.
In Go, Swift, Java and Rust the caller holds the object as a struct or a class, and a
function that takes it has to write that type's name. That type is the caller's, so rulec
neither generates a copy of it nor writes code against the one protoc or another generator
made. In these four, the caller writes the part that reads the inputs out and
calls the rule's function. SQL and NumPy take columns, and the Wasm module takes one JSON object of
the rule's own inputs, so there is no object to project from. The check that holds the paths
to the contract happens in `rulec check`, and so it applies to all twelve.

`rulec api` lists, under `projection`, the contracts borrowed, the path of every projected
input, and the function's name and signature in each of the five.

## Two guards

**The entry guard** enforces at run time what the proof assumed. Every
numeric input is checked against its declared range and every enum input
against its values. Without it, a caller outside the declared domain
would get a silently wrong number — and the completeness proof says
nothing about inputs that were never declared. A number that is not an
integer is refused before the range is looked at, in the languages where
a caller can pass one: a float sits inside any range, and 18.3 for a
rate in steps of 0.1% would otherwise be taken as 1.83%.

**The contradiction guard** is the other half of W114. Where the checker
could not decide whether two rows of a `unique` table can overlap, the
generated code stops rather than silently picking one:

```python
# guard: W114 (table 適用判定, row 1 × row 2): a pair of rows whose exclusivity could not be proven statically
if 残高A <= 1000 and 残高B >= 3980:
    raise RuleContradictionError("table 適用判定: row 1 and row 2 matched at the same time")
```


### A second opinion on the Rust

Beside the Rust module, `gen` writes `<alias>_proof.rs`: proof harnesses
for the [Kani Rust Verifier](https://model-checking.github.io/kani/),
behind `#[cfg(kani)]` so `rustc` never reads them. `kani
<alias>_proof.rs` — or `rulec test --proofs` — holds the
generated code over **every** input in the declared domain rather than
over the vectors: no table falls through, no contradiction guard fires,
no `i64` overflows, and the rows of each `unique` table cover the domain
exactly once.

It is worth running because the model checker and the checker that
proved the table share no code. Where they agree, two unrelated tools
say the same thing; where they disagree, one of them is wrong and you
get the input that shows it. On the corpus of 48 rules, 114 harnesses
verify in 175 seconds.

What it does not say: anything about the table itself, or about any
target but this one. And two kinds of rule get no harness at all, the
file saying which: one with a `string` input, which a harness cannot
quantify over, and one that works out a share with `allocate`, where
two divisions by a value rather than by a constant do not come back
from the model checker.

## The rule as a tool for an agent

`rulec mcp` is for the agent that writes a rule. For the agent that
**calls** one — "may this order be returned", "what is this member's
fee rate", asked in the middle of something else — the answer should be
the table.

This is not "do not let a model decide". It is that a decision which is
**already made** — written in the terms, printed in the tariff, signed
off — should not be re-derived on every call, wavering at the
boundaries and leaving nothing to audit. What is *not* decided yet —
which class this is, how severe that is — is a person's to settle or a
model's; hand that answer in **as a value** and the table takes it from
there.

So `gen` writes the rule as one MCP server beside the module:
`shipping_fee_mcp.py` next to the Python one, `shipping_fee_mcp.mjs`
next to the JavaScript one. Register it and the rule is one tool named
after its alias:

```console
$ claude mcp add shipping_fee -- python3 generated/python/shipping_fee_mcp.py
```

That is the agent on this machine. The places that only accept a URL —
a chat client's custom connectors, an agent builder, a workflow
product's MCP node — want MCP's Streamable HTTP, and it is **the same
server**:

```console
$ python3 generated/python/shipping_fee_mcp.py --http 8000
http://127.0.0.1:8000/mcp
```

It listens on `127.0.0.1` alone unless told otherwise, and refuses a
request whose `Origin` is not local unless that origin is named with
`--origin`. **TLS and authentication are not in it**; put them in
front. `rulec test` drives both transports over every vector, so an
answer that changed with the carrying would be a disagreement.

Where the host renders **MCP Apps**, the same server hands over one
thing more: the approver's page, as the tool's view. `gen` writes it
beside the server (`shipping_fee_page.html`, the page
`rulec doc --format html` renders), and the host shows it opened on the
case that was just asked — the fields filled in, the answer, and the
rows that decided it lit up. The reader of the chat sees which rows of
which table said so, not only the number. When the host says whether it
is light or dark, the page is drawn to match.

The tool speaks the wire: its `inputSchema` is the `in` object of
`rulec schema`, with the unit of every integer and the step of every
rate in its description, and its answer is the record line the module
writes — the inputs, the outputs, and **the rows that decided it**:

```json
{"in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},"observed":{"送料":800},"trace":[{"table":"基本送料","row":3},{"table":"負担判定","row":3}]}
```

Two things follow. An answer can be audited, because it names the rows.
And one call is one fixtures record: started with `--record calls.jsonl`,
the server keeps every answered call in a file that `replay` and `diff`
read as it stands, so what the agent asked becomes what the next
revision is measured against.

A call the rule cannot take is refused with the argument named — a value
outside its range, a name that is not in the enum, a number that is not
an integer. 18.3 for a rate declared in steps of 0.1% is refused, not
read as 1.83%.

The server is generated code like everything else here: nothing to
install beyond `python3` or `node`, and `rulec test` drives it over the
same vectors as the runner and holds its answers to the reference
evaluator.

[The generated code in detail](generated-code.md#the-rule-as-an-mcp-tool){ .md-button }

## The rule as a service another team calls

An agent is one caller. The other is ordinary code in another
repository, often in another language, that already speaks a wire. For
that one, `gen` writes the rule as a
[Connect](https://connectrpc.com/) service: the contract as one
`.proto`, and the conversion that stands between it and the module.

```proto
service ShippingFeeService {
  // The rule is a pure function, so this method has no side effects and
  // can be called with GET.
  rpc Decide(DecideRequest) returns (DecideResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}
```

Three things in that file are worth saying out loud.

**The path spells the package**, which is what a buf module asks for, so
the `.proto` can be dropped into one as it stands — `buf lint` finds
nothing in it.

**The method declares that it has no side effects.** That is not a hint
here but something already proved: the same inputs give the same answer,
forever, for one version of the table. Connect lets such a method be
called with `GET`, which is what makes an answer cacheable.

**The answer carries the rows that decided it**, so one call is one
fixtures record, and every answer carries `rulec-source-sha256` — which
version of the table said so.

The stubs are generated the way [connect-py](https://github.com/connectrpc/connect-py)'s own
documentation generates them, with buf — `gen` writes the `buf.yaml`
and the `buf.gen.yaml` that configure it, so the tree is a buf module
as it stands:

```console
$ uv add connectrpc
$ cd generated/proto && buf generate
```

The service is written as **both applications**, because a rule is a pure
function with nothing to await and the two are two doors on one body:

```console
$ uvicorn shipping_fee_service:app --port 8080      # ASGI
$ gunicorn 'shipping_fee_service:wsgi_app'          # WSGI
```

Apart from the `numpy` the NumPy plan's evaluator needs, `connectrpc` is the one dependency
anything `gen` writes has, and it is confined to the service file: the module it calls still
imports nothing.
`rulec test` puts every vector through the service four times — each
application, asked by POST and by GET — and holds all four to the
reference evaluator.

And when the implementation that runs **today** is a Connect service,
the arrow turns around: `rulec adapter --template connect-python` prints
the twenty lines that put its answers in front of `rulec verify`, and
you get the match rate and the rows you disagree on.

[The generated code in detail](generated-code.md#the-rule-as-a-connect-service){ .md-button }

## The input checked from the same table

The generated function guards its own entry, but a value usually
arrives earlier: at a form, at an HTTP endpoint, at a queue. `rulec
schema` prints the JSON Schema of the wire — every input with its type,
unit, range and enum members — so that check can be built from the
table as well, and cannot drift from the guard:

```console
$ rulec schema rules/送料.rule                # keyed by the rule's own names (届け先)
$ rulec schema rules/送料.rule --keys alias   # keyed by the ASCII aliases (dest), each with its name as title
```

JSON Schema is what an OpenAPI parameter or request body takes as it
stands. For a typed model, the usual converters read it — `datamodel-codegen`
for pydantic, `json-schema-to-zod` for zod — so none of that is a rulec
backend: the schema is the source, and a generator that already reads
JSON Schema is not something this tool needs to carry.

Two things about the wire matter on the way in. Every number is an
integer in the declared unit — 円 as yen, a rate as a count of its
steps — and the property's description says which, so a form that shows
18.3% has to send 183. And an enum travels as its name, the one written
in the rule.

## Running the generated code

```console
$ rulec test generated/
ok    shipping_fee (Python) 68 vectors
ok    shipping_fee (Go) 68 vectors
ok    rounding helper (Python) unit vectors
ok    rounding helper (Go) unit vectors

All 4 matched.
```

The vectors are built from the boundaries of the rule, not from the
generated code, and the rounding helpers get their own unit vectors —
table-level agreement alone would hide a helper bug in a table that
never produces fractions.

## Wasm: one module for any host

The ninth target is a module rather than a function in a language. `wasm/` holds the same
Rust module `rust/` gets, a crate root that puts it behind the canonical ABI of
`call: func(input: string) -> string`, a `.wit` that names that function as the export of a
world, and the Node runner `rulec test` drives. `rustc` alone builds it, with no cargo and no
crate; the shipping rule comes to forty kilobytes and imports nothing.

```console
$ rustc --edition 2021 -C opt-level=s -C lto -C panic=abort -C strip=symbols \
    --target wasm32-unknown-unknown --crate-type cdylib shipping_fee_wasm.rs -o shipping_fee.wasm
```

A host writes a JSON object of the inputs into the module's memory, calls `call`, and reads
the record line back — the same line every other language's `_record` writes, or
`{"error":"…"}` for an input outside the contract. With the `.wit`, `wasm-tools component new`
makes a component of the module without a change, and a component runtime such as wasmtime
invokes it as `call("{…}")`. [Generated code](generated-code.md#wasm) shows the host, the
component step, and what `rulec api` says under `wasm`.

`rulec test` builds the module and holds it to the vectors like every other language:

```console
$ rulec test generated/
ok    shipping_fee (Rust) 68 vectors
ok    shipping_fee (Rust, WASI) 68 vectors
ok    shipping_fee (Wasm) 68 vectors
…
```

The second line is a different thing from the third: the Rust runner itself compiled for
`wasm32-wasip1` and run under wasmtime, the shape a platform that speaks through stdin and
stdout takes, such as Fastly Compute or Spin. [Targeting a language rulec does not
generate](backends.md#a-wasm-host-shopify-functions) says which shape a platform takes — a
Shopify Function takes the third, a named export — and where its boundary runs.

## Is the vector suite itself complete?

```console
$ rulec coverage rules/送料.rule
70 vectors
  row coverage                7 / 7     satisfied
  boundary-pair coverage      4 / 4     satisfied
  shadow-pair coverage        3 / 3     satisfied
  rounding-tie coverage       0 / 0     satisfied
  fold-transition coverage    0 / 0     satisfied
```

`coverage` is **a completeness check on the test suite**. The five
obligations are derived from the rule rather than from the generated
vectors, and anything missing is named — which row, which boundary,
which shadow pair, which rounding tie, which fold transition — with exit 1.

A rounding tie is the value exactly half a step off the grid, the one
point where `half_up` and `half_down` part company. It raises an
obligation only where the rule can actually reach it: 18.3% of a standard
monthly remuneration is always an even number of yen, so the halved
amount has no fraction, and that rule shows 0 / 0.

## Keeping it in step

The generated files are committed, and CI re-derives them:

```console
$ rulec gen rules/ --out generated/ --check
```

`--check` writes nothing and exits 1 if a file differs from a fresh
generation or is missing. **Never edit generated code by hand**: the
header says `DO NOT EDIT`, and the next `gen` overwrites it.

Formatting is built into the generator — no `gofmt` or `black` runs
afterwards, because the moment the output depends on the version of a
tool installed on the machine, generation stops being deterministic.
That `gofmt -l` is empty and `ruff check --select E,W` is silent (line
length aside) is tested instead.

---

[Compare and replay](compare.md){ .md-button .md-button--primary }
[Formats](formats.md){ .md-button }
