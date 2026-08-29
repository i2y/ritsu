# geas

*A small language for holding agent-written code to human-audited claims.*
(working name; spike)

Specs (Spec Kit, Kiro) drive the **generation** half of agent coding and
are prose. Tests written by the same agent that wrote the code are
circular. geas is the **acceptance** half: a claims file small enough to
read aloud, a deterministic runner that binds any program to it through
black-box adapters, and a journal an agent loop can consume.

**You audit 50 lines of claims; the agent writes the code; the gate binds
them.**

```
claim "totals accumulate across requests" {
  when api.post("/reset")
  then status is 200
  when api.post("/add", body: "5")
  when api.post("/add", body: "7")
  when api.get("/total")
  then body is "12"
}
```

Design and v0 semantics: [DESIGN.md](DESIGN.md).

## Quickstart

```console
$ cargo build
$ ./target/debug/geas check examples/calc/calc.geas
$ ./target/debug/geas check examples/greeter/greeter.geas
```

Requires Python 3 on PATH (the example targets are Python; geas itself
does not care what language a target is written in).

## The loop, actually run (2026-08-29, macOS/arm64)

`examples/greeter` is a small HTTP service. The first implementation was
written the way an agent writes one — plausible, clean, and missing a
validation. The claims caught it:

```console
$ geas check examples/greeter/greeter.geas
ok 1 - greets by name
not ok 2 - rejects an empty name
    examples/greeter/greeter.geas:17: status is 400 — got 200
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 3 ok · 1 failed · journal: examples/greeter/.geas/journal.jsonl
$ echo $?
1
```

The fix is the three lines the claim demanded (`if not name: 400`), and
then:

```console
$ geas check examples/greeter/greeter.geas
ok 1 - greets by name
ok 2 - rejects an empty name
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 4 ok · 0 failed · journal: examples/greeter/.geas/journal.jsonl
$ echo $?
0
```

`--json` swaps the human report for a machine one (this is what an agent
loop reads):

```json
{"ok":true,"file":"examples/greeter/greeter.geas","claims":[{"name":"greets by name","line":9,"status":"ok","error":null,"checks":[{"line":11,"check":"status is","expected":"200","actual":"200","ok":true},...]}]}
```

Every interaction and verdict also lands in `.geas/journal.jsonl` next to
the spec — the replayable evidence:

```json
{"claim":"greets by name","event":"when","target":"api","call":"get(\"/greet?name=Alice\")","obs":{"status":200,"body":"{\"message\": \"Hello, Alice\"}"}}
{"claim":"greets by name","event":"check","line":11,"check":"status is","expected":"200","actual":"200","ok":true}
```

Malformed specs exit 2 with a position (`2:3: `then` before any `when``);
a service that dies before opening its port is reported as a claim ERROR
with the exit code, distinct from a failed check.

## Unclaimed drift, actually run (2026-08-29)

The difference from a scenario runner is the second layer: `geas snap`
records **full observations** (status, headers, body / stdout, stderr,
exit) as a baseline; `geas drift` replays the same interactions and
classifies every divergence as **claimed** (some check covers that
field — `geas check` is the authority) or **unclaimed** (behavior
changed where nobody promised anything). A test runner cannot say the
second thing; golden diffing says it about everything and drowns.

The drowning was demonstrated first. With the implementation
**unchanged**, replaying a few seconds later:

```console
$ geas drift examples/greeter/greeter.geas
claim "greets by name" when#1 get("/greet?name=Alice")
  ~ header `date`: "Sat, 29 Aug 2026 00:22:56 GMT" → "Sat, 29 Aug 2026 00:23:10 GMT"   [unclaimed]
claim "rejects an empty name" when#1 get("/greet?name=")
  ~ header `date`: ...                                        (every interaction, same noise)
```

(The very first replay happened to land in the same second and was
silent — noise is intermittent, which is exactly what makes it
poisonous.) The fix is two lines in the audited file, applied at
compare time, no re-snapshot needed:

```
mask header "date"
mask header "server"
```

After that, drift on unchanged code is provably quiet (`0 drifted`,
exit 0). Then the implementation was swapped for an agent-style
refactor (`server_refactored.py`): debug info added to a JSON
response, the 404 message capitalized, charset made explicit.

```console
$ geas check examples/greeter/greeter.geas     # what a test runner sees
ok 1 - greets by name
ok 2 - rejects an empty name
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 4 ok · 0 failed
$ echo $?
0

$ geas drift examples/greeter/greeter.geas     # what the journal sees
claim "greets by name" when#1 get("/greet?name=Alice")
  + body json ".debug": appeared: {"handler":"greet_v2"}   [unclaimed]
claim "rejects an empty name" when#1 get("/greet?name=")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [unclaimed]
claim "unknown paths are 404" when#1 get("/nope")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [unclaimed]
  ~ body: "not found" → "Not Found"   [unclaimed]
drift: 7 interactions compared · 6 drifted · 7 unclaimed change(s) · 0 claimed
$ echo $?
1
```

All claims green; seven observable changes nobody promised — including
an internals leak (`.debug`) that no reviewer asked for. The converse
also holds: a real bug in a claimed field (`/total` off by one) shows
up in drift as `~ body: 12 → 13   [claimed — geas check is the
authority]`, and `geas check` goes red on the same line. JSON bodies
are compared structurally (path-level), so a claim on `.message` does
not silence a new `.debug` sibling; `content-length` is auto-ignored
as derived.

## What the spike proves, and what it doesn't

Proved here: the claim surface reads as intended at this size; one runner
binds two adapter shapes (process argv/stdout/exit and HTTP over
loopback) with per-claim isolation, so stateful scenarios stay
order-independent; failures carry file:line plus expected/got; the same
run feeds humans (TAP-ish), agents (`--json`) and diffing (journal);
and the two-layer design detects **unclaimed behavioral change** that a
green test suite ships silently, with declared masks keeping the
replay quiet on unchanged code.

Not yet built, by design (see DESIGN.md §4–5): static diff-to-claim
binding ("this PR touches claims 3 and 7" from the code diff alone),
the GUI adapter over an accessibility dump, matchers beyond
`is`/`contains`, parallelism, Windows. Naming and licensing are open.

The runner is ~1,900 lines of dependency-free Rust; both example
implementations are agent-written, which is the point.
