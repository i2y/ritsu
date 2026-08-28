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

## What the spike proves, and what it doesn't

Proved here: the claim surface reads as intended at this size; one runner
binds two adapter shapes (process argv/stdout/exit and HTTP over
loopback) with per-claim isolation, so stateful scenarios stay
order-independent; failures carry file:line plus expected/got; the same
run feeds humans (TAP-ish), agents (`--json`) and diffing (journal).

Not yet built, by design (see DESIGN.md §4–5): diff-to-claim coverage
("this PR touches claims 3 and 7"), the GUI adapter over an
accessibility dump, matchers beyond `is`/`contains`, parallelism,
Windows. Naming and licensing are open.

The runner is ~1,100 lines of dependency-free Rust; both example
implementations are agent-written, which is the point.
