# Targeting a language rulec does not generate

`rulec gen` writes Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy. This page is about the
thirteenth target — a language nobody planned for, a workflow engine's expression language, a
spreadsheet formula, a database.

The short answer: **you do not have to modify rulec, and you do not have to give up the
evidence.** A rule already publishes everything a generator needs, and `rulec verify` will
stand any process up and hold it to the rule over every generated case. The rest of this page
is that loop, run end to end against SQL by hand — as it was done before SQL had a backend of
its own (`rulec gen` writes one now, in a different shape); the loop is the same for any
target.

---

## The two paths

| | Generating from outside | A built-in backend |
|---|---|---|
| Where the code lives | your own generator, any language | `src/codegen.rs` |
| Changes to rulec | none | a `Lang` variant and a file of its own, about 1,100 lines |
| Reaches | anything you can run | anything with a runner |
| Gets `rulec gen`, `rulec test`, `rulec api` | no | yes |
| Evidence | `rulec verify` over the generated cases | the agreement test in the suite |

Generating from outside is the general answer and the one to reach for first. Building a
backend in is what a target earns after it has been used enough to be worth carrying;
[the last section](#a-built-in-backend) says what that costs.

## What makes a target trustworthy

The emitter is the easy half. Whatever the target, a rule is a handful of conditions and a
handful of amounts, and turning those into `CASE` or `if` or a formula is mechanical.

The half that matters is the evidence, because **generated code outside the comparison is
outside the claim this tool makes**. A row transcribed with `<` where the rule says `<=`, a
rounding applied in the wrong place, a rate read as a whole number — none of those look wrong
on the page. They look wrong only when something runs both implementations on the same inputs
and compares the answers.

So the order below is: take the inventory, emit, and then make the target answer questions.
The last step is not optional decoration; it is the reason the first two are worth doing.

---

## Generating from outside, step by step

The worked example is [`tests/corpus/ec261.rule`](../tests/corpus/ec261.rule) — Article 7 of
Regulation (EC) No 261/2004, three stacked tables and a rate — targeting SQLite.

### 1. Take the inventory

```console
$ rulec api rules/ec261.rule
```

One JSON object per language with the module and function names, the parameters in order with
their brands, units and ranges, the outputs with their rounding, the enum members, and the
errors the code can raise. Read the names from here rather than from the `.rule` source: the
inventory already applies the ASCII aliases and the per-language spelling rules, so a name you
take from it is the name the other implementations use.

For the table logic itself, read the rule's own structure with `rulec check --format json` or
parse the `.rule`; it is a fixed grammar, described in [reference.md](reference.md).

### 2. Take the wire

```console
$ rulec schema rules/ec261.rule
```

The JSON Schema of the `in` and `out` objects. Everything on the wire is **an integer in the
canonical unit** — no decimals anywhere. Two consequences bite generators:

- **A rate travels as a count of steps.** A `rate[step 50%]` column carries `1` for 50% and
  `2` for 100%. Multiply by it and the product is held in halves; divide that out at the end.
- **Rounding is the last thing that happens**, on the value in its own scale, and the
  direction is declared in the rule. `round down` is toward zero, not toward negative
  infinity: `-4.8 EUR` rounds down to `-4 EUR`. [generated-code.md](generated-code.md) has the
  five modes.

### 3. Emit whatever your target needs

Nothing here is rulec's business — this is your generator. For SQL the shape is a chain of
CTEs, one per table, each adding its output column:

```sql
WITH input AS (
  SELECT :distance AS distance, :intra_eu AS intra_eu, :delay AS delay
),
band_of AS (
  SELECT *, CASE
    WHEN distance <= 1500                                       THEN 'short'   -- row 1
    WHEN distance >  1500 AND intra_eu                          THEN 'medium'  -- row 2
    WHEN distance >  1500 AND distance <= 3500 AND NOT intra_eu THEN 'medium'  -- row 3
    WHEN distance >  3500 AND NOT intra_eu                      THEN 'long'    -- row 4
  END AS band FROM input
),
amount AS (
  SELECT *, CASE band
    WHEN 'short'  THEN 250  -- row 1
    WHEN 'medium' THEN 400  -- row 2
    WHEN 'long'   THEN 600  -- row 3
  END AS base FROM band_of
),
reduction AS (
  -- factor is a rate at step 50%, so it travels as a count of steps: 1 = 50%.
  SELECT *, CASE
    WHEN distance <= 1500                                       AND delay <= 2 THEN 1  -- row 1
    WHEN distance <= 1500                                       AND delay >  2 THEN 2  -- row 2
    WHEN distance >  1500 AND intra_eu                          AND delay <= 3 THEN 1  -- row 3
    WHEN distance >  1500 AND intra_eu                          AND delay >  3 THEN 2  -- row 4
    WHEN distance >  1500 AND distance <= 3500 AND NOT intra_eu AND delay <= 3 THEN 1  -- row 5
    WHEN distance >  1500 AND distance <= 3500 AND NOT intra_eu AND delay >  3 THEN 2  -- row 6
    WHEN distance >  3500 AND NOT intra_eu                      AND delay <= 4 THEN 1  -- row 7
    WHEN distance >  3500 AND NOT intra_eu                      AND delay >  4 THEN 2  -- row 8
  END AS factor FROM amount
)
-- result compensation = base × factor, held in halves of a EUR, then
-- round down(1EUR): toward zero, which is ABS/g*g carrying the sign back.
SELECT (CASE WHEN base * factor < 0 THEN -1 ELSE 1 END)
       * (ABS(base * factor) / 2 * 2) / 2 AS compensation
FROM reduction;
```

Keep the row comments. Reading the target side by side with the rule is how a person checks
it, and it is what makes a mismatch report legible when one arrives — the built-in backends
do the same thing for the same reason.

### 4. Wrap it so it answers questions

```console
$ rulec adapter rules/ec261.rule --template python
```

A 20-to-30 line template that exchanges JSON Lines over stdin/stdout: a handshake, then one
line in and one line out per case. [formats.md](formats.md) has the protocol. Filled in for
the SQL above, using nothing but the Python standard library:

```python
"""Wrap the generated SQL so `rulec verify` can hold it to the rule."""
import json, sqlite3, sys, pathlib

SQL = pathlib.Path(__file__).with_name("ec261.sql").read_text()
db = sqlite3.connect(":memory:")

sys.stdin.readline()  # handshake
print(json.dumps({"ok": True, "impl": "sqlite3/ec261.sql"}), flush=True)

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    d = req["in"]
    row = db.execute(SQL, {"distance": d["distance"],
                           "intra_eu": int(d["intra_eu"]),
                           "delay": d["delay"]}).fetchone()
    print(json.dumps({"id": req["id"], "out": {"compensation": row[0]}}), flush=True)
```

The adapter is the only part that has to speak the protocol, and it can be written in
whatever language can start your target. A target that is not a program at all — a spreadsheet,
a rules engine hosted somewhere — needs an adapter that drives it; that is the whole of the
work, and [the section after next](#when-the-target-cannot-be-run-at-all) is about what to do
when it cannot be done.

### 5. Hold it to the rule

```console
$ rulec verify rules/ec261.rule --adapter python3 adapter.py
Compared 94 / matched 94 (100.000%)
Counterpart: sqlite3/ec261.sql
No mismatches.
```

Almost none of those 94 cases is hand-written. rulec builds them from the rule's own
boundaries — both sides of every comparison, one case per row, and the pairs where one row
shadows another — and adds the rule's own `examples`, which are the six a person did write.
It is the same suite `rulec gen` writes next to the generated code and `rulec coverage` audits.
You can also take it directly with `rulec vectors`.

Exit code 0 is agreement, 1 is mismatches, 2 is a bad argument or an adapter that would not
start. `--format json` gives the same result as data.

### 6. Check that the evidence bites

A check that has never failed has not been shown to work. Break one thing and look. Article
7(2)(c) allows four hours for the longest band; here the generator wrote three, which is the
threshold from the band above and exactly the kind of slip a transcription makes:

```console
$ rulec verify rules/ec261.rule --adapter python3 adapter.py
Compared 94 / matched 92 (97.872%)
Counterpart: sqlite3/ec261.sql

Affected 2 (2.128%)  amount -600
  table band_of row 4 / table amount row 3 / table reduction row 7     2 records  difference -300 uniform  total -600
    Example: delay=4, distance=3501, intra_eu=false → rule compensation=300 / legacy compensation=600
```

It names the rows that fired in all three tables, how much money the difference is worth, and
a concrete case that shows it. That report is the deliverable: it is what you hand to whoever
signs the rule off.

---

## What the evidence covers, and what it does not

**Covers.** Every case rulec builds from the rule's boundaries, compared answer for answer
against the reference evaluator. `rulec coverage` says what that suite reaches — every row,
both sides of every boundary, every shadow pair, every rounding tie — and it is the same suite the built-in
backends are held to.

**Does not cover.** It is a comparison over a finite suite, not a proof of equivalence; the
static proofs (completeness, overlap, units, overflow) are about the *rule*, and they are done
before any of this and hold whatever you generate. It says nothing about whether the table
matches the world, and nothing about the parts of your target the rule does not describe:
where the value comes from, what happens when the input is outside the declared range, or
anything the host does around the call. The declared range in particular is an obligation your
target inherits — the built-in backends emit an entry guard for it, and yours should too.

## When the target cannot be run at all

Some targets cannot be executed where you are: an engine you have no licence for, a
spreadsheet nobody can drive from a script, a DSL whose interpreter is somebody's service.
There is then no adapter to write and no comparison to run.

**Generate it anyway, and say on its face that it carries no evidence.** Put a header in the
output that says so, name the rule and version it came from, and keep it out of any list of
verified artifacts:

```
-- Generated from ec261 v1 by <generator>.
-- NOT VERIFIED: this target was not compared against the rule.
-- The static proofs hold for the rule; nothing here has been checked against them.
```

The reasoning is that the alternative is worse. Refusing to generate does not stop anyone from
needing the rule in that system — it sends them to transcribe it by hand, which is the failure
this tool exists to prevent, and it leaves no marker at all. An unverified artifact that says
it is unverified is the honest form of that situation. It is still outside the claim: do not
describe it as proved, and do not list it beside a target that passed `verify`.

This is a deliberate widening of the rule in DESIGN §15.13 ("a language that cannot join the
agreement check does not go in"), which governs **built-in** backends and still does. What
you generate from outside is yours, not rulec's, and the labelling is what keeps the two
apart.

---

## A built-in backend

Worth doing when a target is used often enough that regenerating it should be one command, and
when its output should be held by the suite rather than by you. It buys `rulec gen`,
`rulec test`, an entry in `rulec api`, and a place in the agreement test.

What it costs, measured four times. The third target, TypeScript, took **about 700 lines of
emitter**, and the fifth, Ruby, about 500.

The tenth took 1,098 and the eleventh 1,293, both in a file of their own under
`src/codegen/` — and most of what is above the earlier figures is the doc comments that say
why each decision went the way it did. Plus one row elsewhere.

That "one row" is recent. Adding Ruby meant editing twenty-two files, because the set of
backends was written out again in seven separate lists. It now lives once, in
`src/backend.rs`, and `gen`, `test`, `api` and the test suites all walk it. What is left
outside is prose, and a test holds that to the registry too: a paragraph that names three of
the targets and not the rest fails.

The pieces, in the order they are usually written:

1. A `Lang` variant, and its spellings in `Lang::spelling()` — the comment marker, how two
   conditions are joined, how an `if` opens, what closes the block. Shared code reads those
   rather than matching on the language.
2. `Gen::<lang>()` — the module: brands or their absence, enums, the rounding helpers, the
   entry guards, one branch per row with the row quoted in a comment.
3. `Gen::<lang>_runner()` — reads the wire JSON on stdin, writes the canonical JSON on
   stdout. This is what lets the target join the agreement test.
4. `round_tests_<lang>()` — the five rounding modes against the reference values.
5. An arm in `Gen::cell()`, and an entry in `Gen::api()`.
6. **A row in `src/backend.rs`**: the id, the name, the toolchain, the files it writes, how
   to run them, and — where the files are not named after the rule's alias, as Java's are
   not — what it does name them. Nothing else has to be told about the language — `rulec gen` writes it,
   `rulec test` runs it, the agreement test compares it, and the test that holds the
   documents to the registry starts requiring the prose to name it.

Whatever the language's own type system can carry, carry it — and say plainly what it cannot.
Rust, Swift and Go hold the unit in the type; TypeScript brands a `bigint`; Python declares a
`NewType` that a type checker enforces and `mypy --strict` is run over the output to prove it;
Ruby, PHP, JavaScript, Java and SQL cannot hold a unit at all, so there it is documented
instead, and the `.rbs` that ships with the Ruby module says so too; the Wasm module is the
Rust one, so the unit rides in it and the `.wit` states it for the wire; the NumPy plan has no
type system to lean on at all, so the unit and the scale are fields of the plan and `rulec api`
prints them. Carrying *something*
still pays where the unit cannot ride: PHP and Java declare the kind of every parameter, so
their entry guard is the one Go, Rust and Swift emit — the range alone — while the three
languages that declare nothing have to ask at the door whether a number is an integer.

The rule that decides whether a target may be built in has not moved: **it must be able to
join the byte-for-byte agreement check.** A generated artifact the suite cannot run is outside
the claim, and inside the repository that is not allowed. Outside it, generated from outside
and labelled as unverified, it is.

---

## A plugin for the host's own system

A fair question, once there are twelve targets: why not a mode that packages the output as a
plugin for each one's own ecosystem — an npm package, a gem, a composer package, a Maven
artifact, a WordPress plugin, a Rails engine?

The question turns out to be two questions, and they have opposite answers.

**A package manifest is configuration.** The package name, the version, the licence, the
namespace, the organisation's scope: none of that is in the `.rule`, so the tool would have to
invent it or grow a configuration file to be told it — and "no runtime and no configuration"
is the whole promise of the output. It is also the part `verify` cannot hold to anything: an
agreement check compares answers, not metadata. The line that is already drawn explains it:
**a manifest is written only when the file will not run without one.** `go.mod` is there
because `go run` needs it; a `Cargo.toml` was considered and dropped for the same reason in
reverse. Neither of the two newest targets needed one — a `require_once` and a `javac`
invocation are enough. Packaging a *library* of rules is a real job, but it is one the
library's repository does once, not one the generator does per rule.

**A host's plugin shape moves.** Shopify Functions changed target, entry point and input
transport inside a year ([below](#a-wasm-host-shopify-functions)). A page that goes stale says
so to anyone who reads the date; generated scaffolding that goes stale looks authoritative.
That is why the boundary belongs in your code and this page holds the recipe.

What the tool does generate is a **door**, and a door has to meet three conditions: it speaks
the wire the tool already speaks (JSON), it needs no SDK, and `rulec test` can drive it over
the vectors. The MCP server, the approver's page and the canonical-ABI Wasm module are the
doors that met all three. Anything that does not is a recipe — the two below are the worked
ones — plus `verify`, which holds whatever you build to the rule just the same.

## A Wasm host: Shopify Functions

Two shapes of Wasm come out of `rulec gen`, and a platform takes one or the other. The
`wasm/` target is a module that exports a function — `call: func(input: string) -> string`
in the canonical ABI, with a `.wit` that makes a component of it — for a host that calls
into it: a script host, an Extism plugin, a component runtime
([generated-code.md](generated-code.md#wasm)). The other shape is a WASI command that reads
one JSON document on stdin and writes one on stdout; that is what the Rust runner is, and it
compiles for `wasm32-wasip1` unchanged
([generated-code.md](generated-code.md#the-rust-runner-as-a-wasi-module)).

A Shopify Function is on the first side, through a door of the platform's own. It is built
for `wasm32-unknown-unknown`, and for every extension target it exports one function that
takes no arguments and returns nothing — `cart.lines.discounts.generate.run`, say. It reads
the input and writes the operations through imported host calls rather than through stdio:
the Shopify Wasm API, which the `shopify_function` crate wraps. The module has to stay under
256 kB, the run under 11 million instructions, and the `input_query` that shapes the input
under 3,000 bytes. No network anywhere. None of that is a shape `gen` emits, and none of it
has to be: the decision itself is small — the `wasm/` module for 送料 is 40 kB, built with
`rustc` alone — so what the budget goes on is the boundary, not the table.

The rule stays a rule: flat inputs, one decision. What the function adds is the boundary —
the twenty lines that pull the scalars the rule wants out of the input, call the generated
function, and turn its outputs into operations. Call the generated Rust module by its typed
signature (`rust/<alias>.rs`, the same file the `wasm/` target wraps); the JSON wire of that
target would cost a parse and a serialisation against the instruction budget and buy nothing
here. "Three or more refrigerated items" and "the dearest line" are written in the rule
(`count`, and `elements` with `fold`), so the boundary passes the lines through and does not
decide anything. Keep it in the function's crate beside the generated module, with ordinary
tests of its own; the generated part is the part the vectors cover.

The boundary is also where the platform's own shape lives, and that shape moves: the input
query, the operations and the API version are Shopify's, on Shopify's release cycle — the
target used to be `wasm32-wasip1` and the input used to arrive on stdin. That is why the
boundary belongs in the function's crate and not in this tool. `function-runner` runs the
built module over one input document and reports the instructions it took, which is how the
boundary earns the same kind of evidence the rule already has.

What the platform decides is out of the rule's reach. A shipping *rate* is not set by a
function (the delivery functions rename, reorder and hide options; rates come from a carrier
service), so a tariff like ゆうパック運賃 is served from the generated code behind an HTTP
endpoint, and a discount table like クーポン割引 is the function itself.

## A spreadsheet again: Google Apps Script

`rulec import xlsx` is the way in from a workbook. This is the way back, and it closes the
loop: the person who owns the policy goes on working in the sheet, but the cell now calls
something with a completeness proof and a `rulec doc` rendering behind it.

Apps Script runs V8, so the generated JavaScript module is the module — with two things to
know.

**There are no ES modules.** Apps Script concatenates `.gs` files into one global scope, and
`export` is a syntax error there. Strip it as a build step, never by hand, so that
`gen --check` still guards the file it came from:

```console
$ sed 's/^export //' generated/javascript/送料.mjs > gas/送料.gs
```

**Every integer in the module is a `bigint`**, because a JavaScript `number` is exact only to
2^53 and the overflow proof (E108) is against int64. A cell gives a `number` and takes one, so
the wrapper converts at both ends:

```js
/**
 * 送料（円）。
 * @param {string} 届け先
 * @param {number} 重量      kg
 * @param {number} 注文金額  円
 * @param {string} 会員
 * @return {number} 円、10円単位に切り上げ
 * @customfunction
 */
function 送料(届け先, 重量, 注文金額, 会員) {
  return Number(shipping_fee(String(届け先), BigInt(重量), BigInt(注文金額), String(会員)));
}
```

`BigInt(1.5)` throws, which is the right answer: the rule refuses a non-integer at the door
anyway. An error thrown inside a custom function reaches the cell as `#ERROR!` carrying the
message, so the entry guard ends up stating its refusal where the person who typed the value
can read it. `Number()` on the way out would lose precision above 2^53 — a yen amount never
reaches that, and saying so is cheaper than leaving a reader to wonder. A second custom
function around `shipping_fee_traced` puts the row that fired in the next column, which is
what a sheet wants for the same reason a log line does.

**Where the evidence stops.** The module is the one the JavaScript pass of `rulec test`
proves — the `sed` removes a keyword and nothing else — and the wrapper is the four lines
outside it. That is the whole boundary, and it is small enough to read. If you want it inside
as well, `clasp run` calls a deployed function from the command line, and that is a process
`rulec verify` can drive over the vectors; it needs your credentials and the network, so it
belongs on your machine rather than in CI.

None of this is a backend. A `.gs` that nothing on the machine can run cannot ride in
`rulec test`, and a target outside the comparison is outside the claim — so the recipe lives
here, and `gen` writes what it can already prove.

## Where to read next

- [reference.md](reference.md) — the grammar, so a generator can read a `.rule`
- [formats.md](formats.md) — the wire, the vectors, and the adapter protocol in full
- [generated-code.md](generated-code.md) — what the built-in backends emit and guarantee
- [codes.md](codes.md) — every diagnostic, for a generator that reports back
