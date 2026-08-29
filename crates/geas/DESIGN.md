# geas — design sheet (working name; naming open)

Status: spike, 2026-08-29. Everything in `README.md` marked as run output
was actually run on this machine (macOS/arm64, rustc 1.97.1, Python 3.14).
This file is the design; the README is the evidence.

## 0. Thesis

Agent-written code has a verification gap: specs (Spec Kit, Kiro) drive
the *generation* half and are prose; tests written by the same agent that
wrote the code are circular. geas is a small language for the *acceptance*
half. A human audits a page of **claims** about observable behavior; agents
write the implementation in any language; a deterministic runner binds the
two and reports in both human and machine form.

The division of labor it sells: **you audit 50 lines of claims; the agent
writes 5,000 lines of code; the gate binds them.**

Design constraints that follow:

- The claim language must stay small enough to read aloud. Its unit is an
  observation, not a hook into implementation code (Gherkin died of step
  definitions; agents now do that binding work, so the language can stay
  black-box).
- The runner must be deterministic and leave evidence (a replayable
  journal), because its consumer is often another agent loop.
- Adoption must be additive: a claims file *beside* any codebase, in any
  language. No rewrite is ever requested.

## 1. Surface, v0

```
target calc {
  run "python3 calc.py"            # process adapter: base argv
}

target api {
  serve "python3 server.py 8123"   # service adapter: spawn + wait for port
  port 8123
}

claim "adds two integers" {
  when calc.run("2", "+", "3")
  then stdout is "5"
  and  exit is 0
}

claim "totals accumulate across requests" {
  when api.post("/reset")
  then status is 200
  when api.post("/add", body: "5")
  when api.post("/add", body: "7")
  when api.get("/total")
  then body is "12"
}
```

Grammar (v0):

```
file    := (target | claim)*
target  := "target" IDENT "{" prop* "}"
prop    := "run" STR | "serve" STR | "port" NUM
claim   := "claim" STR "{" step* "}"          ; first step must be `when`
step    := "when" IDENT "." method "(" args ")"
         | ("then" | "and") check
method  := "run" (STR,*)  | "get" (STR)  | "post" (STR ["," "body" ":" STR])
check   := subject matcher value
subject := "stdout" | "stderr" | "exit" | "status" | "body" | "body" "json" STR
matcher := "is" | "contains"
value   := STR | NUM
```

Comments `#` to end of line. A `then` binds to the most recent `when`'s
observation; `and` is the same word for the same observation.

## 2. Semantics decided in v0

- **Claim isolation.** Each claim gets a fresh instance of every `serve`
  target it touches (spawned lazily, killed at claim end). Claims are
  therefore order-independent and individually replayable. Within one
  claim the instance persists, so stateful scenarios are expressible.
- **Observations.** A `run` call observes `{stdout, stderr, exit}`. A
  `get`/`post` observes `{status, body}`. `body json "<path>"` reads into
  a JSON body with `.field` / `[index]` steps.
- **Matching.** `is` on stdout/stderr/body ignores exactly one trailing
  newline; `is` on exit/status/json-numbers is numeric; `contains` is
  substring and only valid on string subjects.
- **Failure shapes.** A failed *check* marks the claim FAIL and evaluation
  continues (all divergences reported). A failed *when* (spawn error,
  timeout, port never opened) marks the claim ERROR and the rest of the
  claim is blocked.
- **Determinism scope (v0).** Fixed timeouts (5 s per step, 5 s
  ready-wait), one claim at a time, fresh service state per claim, target
  commands run with cwd = the spec file's directory. Wall-clock never
  enters a comparison.
- **Evidence.** Every interaction and verdict is appended to
  `.geas/journal.jsonl` next to the spec — the machine-readable trace an
  agent (or a human diffing two runs) consumes.
- **Exit codes.** 0 all claims hold · 1 some claim failed · 2 the spec
  itself is malformed. `--json` swaps the human report for a machine one.

## 3. Adapter contract

An adapter turns a `when` into an observation map. v0 ships two:

| adapter | spawn | when | observes |
|---|---|---|---|
| process (`run`) | per `when` | argv appended to base | stdout, stderr, exit |
| service (`serve` + `port`) | per claim | HTTP/1.1 over loopback, `Connection: close` | status, body |

The contract is deliberately dumb: no adapter exposes implementation
internals; everything is observed at a boundary a user could also observe.
Planned adapters keep that rule: GUI via the accessibility-tree dump
(donor: pixie's `PIXIE_SCRIPT`/a11y machinery), library via FFI harness.

## 3.5 Drift — the second layer (built later the same day)

`geas snap` records every interaction's **full** observation (status,
headers, body / stdout, stderr, exit) to `.geas/baseline.jsonl`;
`geas drift` replays the same interactions against the current
implementation and classifies each divergence:

- **claimed** — some check in that claim covers the diverged field
  (subject-level for status/exit/text, segment-wise path-level for
  JSON bodies), so `geas check` is the authority on it;
- **unclaimed** — observable behavior changed where no claim promises
  anything. This is the information a test runner structurally cannot
  produce, and the reason the journal records more than the checks
  assert.

Noise is handled by declaration, not heuristics: `mask header "date"` /
`mask body json ".request_id"` name volatility in the audited file and
apply at compare time (changing masks needs no re-snapshot);
`content-length` is auto-ignored as derived. JSON bodies diff
structurally, so a claim pinning `.message` does not silence a new
`.debug` sibling. Exit: 0 quiet · 1 drift · 2 baseline missing or a
replay error. Golden/snapshot testing fails toward noise (every change
is a failure); assertion testing fails toward silence (unasserted
change is invisible); the claims/journal split is the midpoint, priced
at one `mask` line per genuinely volatile field.

## 4. Non-goals in v0 (deliberate)

Static diff-to-claim binding ("this PR touches claims 3 and 7" read
from the code diff alone; the runtime half exists as `drift`). Matchers
beyond `is`/`contains`. Parallel claims. Quoting in command strings.
Env/clock/seed pinning knobs. Claim dependencies. Windows.

## 5. Next, in order

1. **Static diff-to-claim binding** — complete the gate for agent PRs
   (drift covers the runtime half; this reads the diff).
2. **GUI adapter** over an accessibility dump, which makes desktop apps
   claimable with the same five subjects.
3. **Distribution through the agent channel**: ship the CLI with a skill
   file so an agent is fluent on first contact; the human only ever reads
   the claims. (The cute-skill workflow, generalized into a GTM.)
4. Naming, licensing, publication — owner calls.
