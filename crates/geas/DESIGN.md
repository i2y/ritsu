# geas — design sheet (working name; naming open)

Status: spike, 2026-08-29. Everything in `README.md` marked as run output
was actually run on this machine (macOS/arm64, rustc 1.97.1, Python 3.14).
This file is the design; the README is the evidence.

2026-10-02: the design of v1 is added as §6–§16 — the diff half of the
gate, GUI targets, the matchers, parallel runs, quoting, pinning, the
diagnostics and the skill. §0–§5 stay as the spike wrote them; where v1
changes a v0 decision, the v1 section says so. `PLAN.md` is the order of
work. v1 builds with the stable toolchain (rustc 1.94.1, edition 2024)
and stays dependency-free. §16 lists what was measured to settle the
design, and how.

2026-10-03: stage B is built (the diagnostics, `geas map`, `geas
affected`, the four runtimes); §6, §7, §13 and §16 say what building it
decided.

2026-10-03: the first half of stage C is built: command strings (§11),
the matchers and the `header` subject (§9), pins (§12), and parallel runs
with process groups (§10, §10.1). Each section ends with what building it
decided; §7.2 says why the Node hook changed, and §16 what was measured.

2026-10-03: the second half of stage C is built: the screen and its checks
(§8.1, §8.6), the driver protocol (§8.5), pixie (§8.3) and Chrome (§8.4),
and E012, E013 and E034-E037 (§13). §6 and each section of §8 end with what
building them decided, and §16 says what was measured.

2026-10-03: stage D is built: the skill and `geas skill` (§14), two more
examples (`web-greeter`, a page in Chrome with every pin, and
`pixie-greeter`) and the greeter's claims named in Japanese, README.md
rewritten around v1, README.ja.md, and the tests that hold the pages to the
tool. §14 ends with what building it decided, §15 gathers what is still
out, and §16 says what was measured. v1 is now what §6–§14 describe.

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

(v1's surface is §6. Every v0 file stays valid, but for a command string
holding a quote or a backslash, §11.)

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

(v1 builds the GUI adapters in §8: pixie, Chrome, and a driver protocol for
any other GUI. The FFI harness stays out, §15.)

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

(v1 builds all of these but the last two: §7–§12. Why claim
dependencies and Windows stay out is in §15.)

## 5. Next, in order

1. **Static diff-to-claim binding** — complete the gate for agent PRs
   (drift covers the runtime half; this reads the diff).
2. **GUI adapter** over an accessibility dump, which makes desktop apps
   claimable with the same five subjects.
3. **Distribution through the agent channel**: ship the CLI with a skill
   file so an agent is fluent on first contact; the human only ever reads
   the claims. (The cute-skill workflow, generalized into a GTM.)
4. Naming, licensing, publication — owner calls.

v1 designs 1–3 (§7, §8, §14). Of 4, the owner chose the license on
2026-10-03: MIT OR Apache-2.0, as for rulec and dandori. Naming and
publication stay with the owner.

## 6. Surface, v1

Every v0 file stays valid, with one exception (§11: a quote or a
backslash in a command string now means something). The whole grammar
after this round:

```
file    := (target | claim | mask | pin)*
target  := "target" IDENT "{" tprop* "}"
tprop   := "run" STR | "serve" STR | "port" (NUM | "auto")
         | "pixie" STR | "driver" STR | "serial" | pin
pin     := "env" STR STR | "env" "clean" | "env" "pass" STR
         | "tz" STR | "locale" STR
         | "clock" STR ["env" STR] | "seed" NUM ["env" STR]
claim   := "claim" STR "{" step* "}"            ; first step must be `when`
step    := "when" IDENT "." call | ("then" | "and") check
call    := "run" "(" [STR ("," STR)*] ")"
         | "get" "(" STR ")" | "post" "(" STR ["," "body" ":" STR] ")"
         | "open" "(" [STR] ")"
         | "click" "(" STR ["," "nth" ":" NUM] ")"
         | "input" "(" STR ["," place] ")"
         | "submit" "(" [place] ")"
         | "press" "(" STR ")" | "advance" "(" NUM ")"
place   := "field" ":" NUM | "into" ":" STR ["," "nth" ":" NUM]
check   := subject matcher
subject := "stdout" | "stderr" | "exit" | "status" | "header" STR
         | "body" | "body" "json" STR | "screen"
matcher := "is" ["not"] value
         | "is" ("above" | "below" | "at" "least" | "at" "most") NUM
         | "is" "between" NUM "and" NUM
         | "contains" thing | "does" "not" "contain" thing
         | "matches" STR | "does" "not" "match" STR
         | "exists" | "does" "not" "exist"
value   := STR | NUM | "true" | "false" | "null"
thing   := STR                                  ; on a text subject
         | node                                 ; on `screen`
node    := ["exactly" NUM] ROLE [label] ["with" "value" STR] [state]
           ["in" ROLE [label]]
label   := STR | "containing" STR | "matching" STR
state   := "disabled" | "enabled" | "checked" | "unchecked"
mask    := "mask" "header" STR | "mask" "body" "json" STR
         | "mask" "screen" node
```

ROLE is a role name of WAI-ARIA 1.2, or `text` (§8.1). A pin at the top
level applies to every target, and one inside a target overrides it there
(§12).

What each call observes, which decides the subjects its `then`s may name:

| call | on a target with | observes |
|---|---|---|
| `run` | `run` | stdout, stderr, exit |
| `get`, `post` | `serve` | status, header, body, body json |
| `open`, `click`, `input`, `submit`, `press`, `advance` | `pixie`, `driver`, or `serve` (in a browser) | screen |

Rules beyond the grammar, each new in v1:

- Target names and claim names are unique in a file (E003). v0 let a
  second target of the same name shadow the first, and drift keyed its
  baseline by claim name, so two claims of one name overwrote each
  other's records.
- A `then` naming a subject its `when` cannot observe is a spec error
  (E007). v0 found it at run time and reported a failed check.
- An HTTP answer sent with `Transfer-Encoding: chunked` is read as the body
  it carries. An HTTP/1.1 client has to read one, and Node's `http` sends one
  whenever a handler sets no `Content-Length`; the spike took the chunk sizes
  for part of the body.
- A spec's files under `.geas/` are named after it:
  `<stem>.journal.jsonl`, `<stem>.baseline.jsonl`, `<stem>.map.jsonl`.
  v0's `journal.jsonl` and `baseline.jsonl` collided when two specs
  shared a directory. A v0 `baseline.jsonl` is not read; when `drift`
  finds none under the new name, E050 points at the old one. `check`,
  `snap`, `drift` and `map` take several specs; they run one after
  another, and the exit status is the worst of theirs.

The language grows from five subjects and two matchers to eight subjects
and thirteen matcher forms, every one of which reads aloud as English;
what each is for is in §8–§12.

As built, for the GUI forms:

- `exactly` takes a whole number from 0; `nth:` and `field:` count from 1,
  and 0 is E013 once the target is known. A count or a place that is not a
  whole number is E001. `nth:` goes with `into:` only (E002 otherwise:
  without `into:`, `field: n` says the place), and `field:` with `into:` is
  E002. A mask's node takes no count (E001).
- A node pattern ends before the words that start what follows a check
  (`when`, `then`, `and`, `}`, and what the top level holds); a word there
  that is none of them is read as a state geas does not know (E002). After
  `contains`, a word that is not `true`, `false` or `null` starts a node, so
  `stdout contains text "5"` is E008 (a node on a text), not E001.
- A pin may sit in a `pixie` or a `driver` target as in any other. `clock`
  and `seed` without `env` are fine on a driver, which is handed every pin,
  and on a service some claim opens as a page, whose clock and random
  numbers geas keeps itself (§12); on a command, a pixie app, or a service
  no claim opens, they are E011 as before.

## 7. Diff to claims: `geas map` and `geas affected`

`drift` reads the run: what a person could observe changed where no
claim promises anything. The other half reads the diff: which claims this
change touches, and which changed code no claim runs. A diff names files
and lines and a claim names neither, so the two meet through a record of,
for each claim, the lines of the implementation its run executed.

```
geas map <spec.geas> [--root <dir>] [--out <file>]
geas affected <spec.geas> <diff | -> [--map <file>]... [--root <dir>]
```

`map` is `check` with each language runtime's coverage switched on: it
runs every claim, prints the same report, and writes the record to
`.geas/<stem>.map.jsonl`. `affected` reads a unified diff (`git diff`, or
`diff -u`) and the record, runs nothing, and says what the change
touches.

### 7.1 How this squares with black-box claims

Nothing in a claim changes. No claim names a file, a function or a line,
and the record is never written by hand.

- The record is measured by each language's own coverage machinery,
  switched on from outside: environment variables the runtime reads, and,
  for the compiled languages, the toolchain's coverage build. geas never
  edits, wraps, imports or rebuilds the implementation.
- The record never decides a verdict; `check` and `drift` do not read it.
  It decides where a reviewer looks: which claims to read again for this
  change, and which code to ask about. A wrong record misroutes a review;
  it cannot turn a claim green.
- The record is derived and disposable, like the journal. It may be
  cached or committed; nobody audits it.

### 7.2 Four languages

geas sets every switch below for every process it starts during `map`,
and each runtime picks up its own. A target does not say what it is
written in, and a Python service that runs a Go command is recorded in
both.

| language | geas sets | the target has to | read with |
|---|---|---|---|
| Python 3.12 and later | `PYTHONPATH` gains a directory under `.geas/` holding geas's `sitecustomize.py` | nothing | the JSON that file writes |
| Node: JavaScript, and TypeScript by Node's type stripping | `NODE_V8_COVERAGE`; `NODE_OPTIONS` gains `--require` of geas's hook | nothing | V8's coverage JSON |
| Go | `GOCOVERDIR` | be built with `go build -cover` (`-coverpkg=./...` to take in its own packages); as a service, return from `main` on SIGTERM | `go tool covdata textfmt` |
| Rust | `LLVM_PROFILE_FILE` | be built with `-C instrument-coverage` and be the program the command starts; as a service, exit normally on SIGTERM | `llvm-profdata merge`, `llvm-cov export -format=lcov` (rustup's `llvm-tools`) |

- **Python.** The `sitecustomize.py` geas writes uses only the standard
  library: `sys.monitoring` in its coverage slot, LINE and PY_START
  events, each callback returning DISABLE after the first hit, so a line
  costs one callback per process. The lines that are code come from
  `co_lines()` of every code object of every module the run loaded under
  the root (line 0, the frame's start, is dropped). It writes at exit and
  on SIGTERM, for which it installs a handler that writes and exits with
  143. The `sitecustomize` the Python would have run without geas's, the
  project's own or the installation's (Homebrew's Python has one, which
  adds its `site-packages`), is found on `sys.path` without geas's
  directory and run after the monitor starts. A Python older than 3.12 has
  no `sys.monitoring` and gives no record; `map` warns (W060).
- **Node.** V8 writes one JSON a process when the process exits normally,
  not when a signal kills it (measured: none after SIGTERM). The hook
  geas preloads listens for SIGTERM and exits with 143, which is a normal
  exit, when the program has no SIGTERM handler of its own; a program that
  has one shuts down by its own code and exits normally too (measured,
  both ways). The hook does not call `v8.takeCoverage()`: that resets the
  counts, and the exit then writes a file without them, named by the pid
  and the millisecond, so when the two writes fell in the same
  millisecond the counts were lost (33 runs of 60, measured in stage C;
  stage B's hook called it, and a record of `tally-node` came out without
  one claim's lines). A line counts as run when the innermost V8 range
  holding its first non-blank character has a count above zero; every
  non-blank line of a loaded file counts as code. Node blanks TypeScript's
  types instead of moving the code, so the offsets fall on the `.ts`
  file's own lines (measured). V8 names an ES module by its `file://` URL
  and a CommonJS script by its plain path (measured); both are read.
- **Go.** Counters are written when the program exits normally (`os.Exit`,
  or returning from `main`); a service killed by SIGTERM leaves only its
  metadata file (measured), so a Go service is recorded when it handles
  SIGTERM, as the usual `signal.NotifyContext` and `Server.Shutdown` do
  (measured). The text format names import paths
  (`example.com/calc/main.go`); geas maps them to files through the
  `go.mod` files under the root. `go tool covdata` is built into the Go
  build cache the first time it is used. Metadata without counters is a
  program that did not exit normally: E066. Built with `-cover`, a program
  prints `warning: GOCOVERDIR not set, no coverage data emitted` on stderr
  when `GOCOVERDIR` is not set (measured), and `check`, `snap` and `drift`
  do not set it, so a claim on such a command's stderr would see it there:
  the `-cover` build is for `map`.
- **Rust.** The profile is written by an exit hook: `std::process::exit`
  writes it, a signal does not, and Rust's standard library cannot catch
  SIGTERM, so a Rust service needs code or a crate of its own to stop on
  it; the example is a command-line program. `llvm-cov` needs the
  instrumented binary. A profile is named `rust-<pid>-<signature>.profraw`,
  and geas knows the pid of every process it starts and the program its
  first word starts (resolved as the OS would): a profile whose pid is a
  process geas started, running a program that carries LLVM's coverage
  mapping (`__llvm_covmap`), comes from that program, and so does one with
  the same signature (a process the program forked). Any other profile
  came from a program geas did not start itself, through a script or
  `cargo run`, and is left out with W061; the command has to start the
  binary itself. Handed a program without coverage mapping, `llvm-cov`
  refuses the whole export (measured), which is why the mapping is looked
  for. The LLVM tools are looked for once a profile is found: from
  `GEAS_LLVM_BIN` when it is set, and otherwise in the toolchain's sysroot
  (`rustc --print sysroot`, then `lib/rustlib/<host>/bin`), then on PATH;
  missing, they are E065. Built with `-C instrument-coverage`, a program
  writes `default_*.profraw` into its working directory when
  `LLVM_PROFILE_FILE` is not set (measured), as under `check`: the
  instrumented build is for `map`.

### 7.3 One claim, one slice

Every process geas starts for a claim writes into a directory of its own
under the claim's (`.geas/cover/<n>/<k>/` for the k-th, removed once the
claim has been read), so a claim's slice is the union of what its
processes ran and nothing else, and each target's part of it is known.
A `serve` target already gets a fresh instance per claim (§2), so a
service's record is per claim by construction. A process the target
starts in turn inherits the switches and lands in the same directory,
with one exception: a Rust program geas did not start itself, whose
binary `llvm-cov` would need (W061). A target whose processes reported no
line under the root, in any claim, is W060, unless W061, E065 or E066
already says why.

In `map`, a service is stopped with SIGTERM, given 5 s to exit, then
killed, its process group with it (§10.1); the runtimes write on a normal
exit. (`check` still kills at once,
as in v0: nothing needs writing there.) SIGTERM goes through the C
library's `kill`, declared in an `extern` block, so geas stays without
dependencies. A service that has to be killed after the 5 s, or a Go
service that leaves metadata without counters, may have stopped without
writing its record, and the claim's slice cannot be trusted: E066.

Startup code (imports, route tables, the `def` lines Python runs on
import) runs in every claim that starts the target. `affected` shows a
change there as touching "every claim that starts `api`" rather than as a
list.

### 7.4 The record

`.geas/<stem>.map.jsonl`, one JSON object a line, every path relative to
the root and written with `/`, no absolute path anywhere:

- a header: the format version, the spec's path, the root (relative to the
  spec's directory), and the claims in order with how each ended in the
  mapped run (ok, fail, error) and the targets each started;
- one line per source file under the root, loaded or not: its path, its
  git blob hash, its language by extension (`.py`; `.js .mjs .cjs .ts
  .mts .cts`; `.go`; `.rs`), and the lines its runtime calls code, or
  `null` where no runtime reported the file (Python and Node report only
  the files a run loaded, Go and Rust only code built with coverage);
- one line per claim, target and file the claim ran through that target:
  the lines, as ranges (`1-4,6,9-11`).

The lines are sorted (files by path; claims in spec order, then targets
in spec order, then paths), so one run on one tree writes the same bytes
(the tests run `map` twice and compare). The root is the nearest
directory above the spec that holds `.git`, found by looking, not by
running git; else the spec's own directory; `--root` overrides it. Paths
with a component `.geas`, `.git`, `node_modules`, `site-packages`,
`__pycache__` or `target` are left out: tool state, installed packages,
byte code, and Cargo's build directory, which holds generated `.rs` files
(and, for a Rust project's own tests, their scratch directories) that
would make every record stale. Symbolic links are not followed: git keeps
a link as the path it points to, not as that file's bytes.

### 7.5 How old a record is

A record is exactly the code it was measured on: every source file in it
carries its git blob hash (SHA-1 of `blob <size>\0<bytes>`, computed by
geas; git is not needed). `affected` holds the record to the diff file by
file, and refuses one that does not fit (E062, exit 2). It never answers
"no claim touched" from a record of other code.

- A source file the diff does not touch must still have the recorded hash
  on disk.
- A source file the diff touches must have, in the record, the hash of one
  side of the diff: the code before the change or after it. That side is
  the record's side, for that file.
- A file the diff adds is absent from a record of the code before, and
  present in a record of the code after; a file it deletes, the reverse.

The hashes of a diff's two sides come from its `index <before>..<after>`
lines when it has them (a `git diff`; an abbreviated hash matches by
prefix). A diff without them (`diff -u`) is held to the file on disk: if
the disk agrees with the diff's after side (every context and `+` line
where its hunk puts it), the disk is the after side, and the before side
is the disk with the hunks undone; the reverse when the disk agrees with
the before side; neither is E064. A plain diff has no renames: its two
names are one file, the `+++` one (`diff -u calc.py.orig calc.py`), and
`a/` and `b/` are dropped when both lines have them, as Mercurial writes
them. A hunk with fewer lines of context after its changes than before
them ran into the end of the file, as `patch` reads it, so a file that
goes on past such a hunk is not that side: without this, a diff that
removes a file's last line fits the file before the change as well as
after it.

Re-anchoring an old record onto moved lines was discarded. A heuristic
there would make the list of touched claims quietly wrong, which is the
one failure this command must not have; recording again is cheap, since
it is a run of the claims.

### 7.6 What a change touches

Each changed line is looked up on the side a record covers. A blank line
is never code in any of the four languages and counts for nothing.

- An added or rewritten line (`+`) needs a record of the code after the
  change (E063 otherwise: record the changed code). There it is run by
  some claims, which it touches; or it is code no claim runs, which is
  unclaimed code; or it is not code (blank, a comment, a line the runtime
  does not report), which counts for nothing.
- A removed line (`-`) is looked up in a record of the code before the
  change when one is given: `--map` can be given twice, one record per
  side, each matched to its side by its hashes. With only a record of the
  code after, a run of removed lines that no added line replaces touches
  the claims that run the nearest code above and below where it was,
  marked "near", and is unclaimed when none does; removed lines that added
  lines replace are looked up through those.
- A line that every claim starting a target runs through that target, when
  two claims or more start it, is shown once, as touching "every claim that
  starts `<target>`", instead of under each claim: imports and route
  tables, and the code every request passes.
- A deleted file is listed apart. With a record of the code before the
  change, its lines are looked up as any removed lines; without one, which
  claims ran it is not known, and that makes the exit status 1.
- A source file in a mapped language that no runtime reported (§7.4) is
  "run by no claim" as a whole, and every added line of it is unclaimed.
- Other files (documents, configuration, data) are listed as outside the
  record; they do not change the exit status.

Two files are reported apart from the code, because a change to them
needs a person whatever else the diff does: the spec (the claims
themselves changed; read them again) and its baseline (drift now compares
against something new, and re-snapping is the way a change drift would
have reported disappears). Either makes the exit status 1.

The report, for a person and as JSON (`--json`, Appendix A of PLAN.md):
the records and the side each is of; the touched claims in spec order,
each with the files and lines that touch it (numbers of the code after
the change, or "removed" and the numbers before it, "next to lines it
runs" where a removal was attributed by its neighbours), then each
target's startup group; unclaimed code, by file and lines ("no runtime
reported this file" where none did); deleted files; files outside the
record; changes to the spec and the baseline; and a last line of counts.
Exit 0 when no changed code is unclaimed, no deleted file's claims are
unknown, and neither the spec nor its baseline changed; 1 otherwise; 2
when a record is missing or stale, or the diff cannot be read.

`affected` never runs claims, and `check` never runs fewer because of it:
the record routes review and never narrows acceptance.

### 7.7 Limits

- Lines are the unit. A line holding two branches counts as run when
  either ran, and Node counts a comment inside a block that ran. Node
  decides a line by its first character, so `} else if (…) {` counts as
  run when the block its `}` closes ran, whatever the new condition does:
  in `tests/changes/tally-node`, the line that adds a `/health` route is
  shown as touching the two claims that add a number.
- Python and Node report only the files a run loaded; a file nothing
  loaded is known by its extension, not by its lines.
- A process whose runtime the switches do not reach (a Go binary built
  without `-cover`, Python before 3.12, a shell script) adds nothing;
  `map` warns for each target that gave no record at all (W060).
- Templates, SQL, configuration and generated files are outside the
  record. pixie apps and the scripts of a page in the browser are not
  recorded in v1 (§15).

Discarded:

- Reading the implementation (call graphs, imports) to guess which claims
  reach a line: a parser per language, unsound under dynamic dispatch, and
  geas would be reading the code its claims treat as a black box. What
  each claim ran is measured instead.
- Claims that say what code they cover: the hook into the implementation
  §0 rules out.
- coverage.py, c8 or nyc as dependencies: not installed everywhere. geas
  carries its own small Python collector and reads what Node and the Go
  and Rust toolchains already write.
- git as a dependency: the blob hash is computed, and `index` lines are
  read when a diff has them.

## 8. GUI targets: the screen

A GUI target turns each `when` into an action on an app, and observes the
screen once the app has settled: its accessibility tree, which is what a
screen reader is told. Claims say what the screen contains. They never
name a widget class, a CSS selector, an element id or a pixel, and a
control a claim cannot reach by its role and name is one a person using a
screen reader cannot reach either, so that failure says something true
about the app.

```
target app {
  pixie "build/greeter"
}

claim "greets the name typed in" {
  when app.input("Ada")
  when app.click("greet")
  then screen contains text "Hello, Ada!"
  and  screen contains textbox "type here" with value "Ada"
  and  screen does not contain button "retry"
}
```

§5 meant the GUI adapter to fit "the same five subjects". It adds one,
`screen`, instead: the five are text and numbers out of a process or an
HTTP exchange, and a screen is a tree whose claims are about nodes. Put
into `body contains`, a claim would depend on how the tree happens to be
printed.

### 8.1 The contract

Whatever drives the app, geas hands it actions and gets back, after each,
a tree of one shape: a node has a role, a name, a value, states and
children.

- The role is a role name of WAI-ARIA 1.2 (`button`, `textbox`, `heading`,
  `checkbox`, `dialog`, `listitem`, …), or `text` for static text, which
  ARIA has no role for. A driver maps its own words onto these, so one
  claim reads the same against a pixie app and a web page.
- The name is the accessible name; the value is what a screen reader
  reads after it (a field's contents, a slider's number); the states are
  those of `disabled`, `checked`, `unchecked` the driver knows.
- A container that only lays things out reports nothing and hands its
  children up (pixie's rule, and Chrome's `generic`); a node named by its
  own text drops the text children that repeat the name; empty text
  reports nothing.

The actions:

| action | what it does |
|---|---|
| `open()`, `open("/path")` | the app's first screen; in a browser, that path of the target |
| `click("<name>", nth: n)` | press the n-th (by default the first) button, link, checkbox, radio, switch, tab, menuitem or option of that name |
| `input("<text>", field: n)` | type into the n-th text field; `into: "<name>"` picks the field by its name |
| `submit(…)` | Enter in a field, picked the same way |
| `press("<key>")` | a key or chord: `enter`, `escape`, `tab`, `backspace`, `up`, `cmd-s`, `ctrl-a`, `shift-tab` |
| `advance(<ms>)` | move the app's clock forward |

Each claim gets a fresh app, as it gets a fresh service (§2).

Discarded: comparing screenshots (fonts and antialiasing make noise, and a
picture cannot say that a button is disabled); selectors and ids in claims
(a renamed class breaks the claim while nothing a person sees changed);
pixie's element tree (`Column[Text(…)]`) as the observation (it is the
app's layout, not what the app tells a person).

As built:

- The screen is a node without a role whose children are what the app
  shows at its top level. A node without a role reports nothing and hands its
  children up, wherever it is, so a driver's root may come with or without
  one: without, it is the screen; with one (`window "Greeter"`), it is the
  screen's one node. Each driver maps its own root to the screen: pixie's
  window `group`, Chrome's `RootWebArea`.
- The children a node drops are text leaves whose name is the node's name
  or its value, so a button keeps no `text "Add"` under it and a field no
  `text "milk"`.
- States are written `disabled`, `checked`, `unchecked`, then any other a
  driver reports, in that order.
- `input` makes the text what the field holds, as pixie's `input:` does: a
  browser selects what is there before it types. `click` reaches the eight
  roles above; `input` and `submit` the text fields, `textbox`,
  `searchbox` and `spinbutton`, counted together. A `combobox` is not one:
  Chrome reports a `<select>` as one, and typing into it does nothing.

### 8.2 Live drivers, and a replayed one

A live driver keeps the app running for the whole claim and acts as each
`when` comes, so another target's `when` can come between two actions. A
replayed driver runs a whole script in one process: geas collects the
claim's actions on that target, runs them as one script with a tree read
after each, and hands each `when` its tree. So within one claim, no other
target's `when` may come between two actions on a replayed target (E012):
the app is not running between them. Before the first and after the last
is fine.

As built, E012 is found before anything runs, at the `when` that comes
between, with the lines of the actions before and after it. A pixie app
starts once in a claim, so `open()` on it after another action on it is
E013: the first screen comes before the others.

### 8.3 pixie, the first driver (replayed)

A pixie app reads `PIXIE_SCRIPT`, a comma-separated list of steps, runs it
headless, and prints its element tree at the start and at the end; a step
`a11y` prints the accessibility tree at that point (pixie's README,
`script.rs`, `a11y.rs`). geas:

- always sets `PIXIE_SCRIPT`, since an app without it opens a window, and
  sets `PIXIE_DUMP` to a file under `.geas/`: newer pixie kernels write
  the transcript there, apart from what the app itself prints, and older
  ones ignore it and print to stdout, which geas reads instead;
- translates the actions: `click("x", nth: n)` to `click:x`
  (`click@<n-1>:x` for n > 1), `input("t", field: n)` to `input:t`
  (`input@<n-1>:t`), `submit(field: n)` to `submit` (`submit@<n-1>`),
  `press("k")` to `key:k`, `advance(ms)` to `advance:<ms>`, each followed
  by `a11y`; `open()` is the starting screen, an `a11y` alone. Commas and
  backslashes in text are written `\,` and `\\`;
- reads the transcript: the lines that start with `group` are the
  accessibility trees, one per action, in order; any other count is E037;
- maps pixie's roles onto §8.1's: `label` to `text`, `textInput` to
  `textbox`, `image` to `img`, `listItem` to `listitem`, `progress` to
  `progressbar`, `comboBox` to `combobox`, `radioGroup` to `radiogroup`,
  `tabList` to `tablist`, the rest as they are; a checkbox's or a
  switch's value `true` or `false` becomes the state `checked` or
  `unchecked`;
- refuses `into:` on a pixie target (E013): pixie reaches text fields by
  position only.

When a step names something the app does not have, pixie exits with 101
and prints nothing about it, and newer kernels print a script's trees only
at its end, so the trees before the refused step are lost as well
(measured). geas then runs the script's prefixes until the first that
fails, which finds the refused action and gives the screen just before
it, so the message can say which buttons and links that screen had. Two
steps of the greeter took 32 ms in all, and this is done only on a
failure.

pixie prints names and values without escaping them, so a name holding
`", ` or `]` can be misread (measured: a value "a, b" prints as `=a, b`
followed by `, ` and the next node). geas reads with pixie's role names
as anchors and checks that what it read prints back to the same line,
which catches most such cases but not all; an escaped dump in pixie would
close it (§15).

As built:

- A name or a value may hold a line break, and pixie prints it as it is
  (measured: a field given `a` and `b` on two lines printed its tree on two
  lines), so the trees are read out of the transcript as a whole, not line
  by line: a tree starts a line with `group` and runs to the `]` that
  closes it. Each name and value is read shortest first, going back when the
  rest of the tree does not read, at most 16 readings a place; a tree no
  reading fits is E037. A reading prints back to the text by construction,
  so what is left unknown is which of two readings a tree meant, and the
  shortest is taken.
- The transcript goes to `.geas/<stem>.pixie-<n>.txt` (`n` the claim's
  number, so claims side by side do not meet), named absolutely since the
  app runs in the spec's directory, and the file is removed once read.
- An app has 5 s for each action of its script. One that does not finish is
  stopped (E037), and so is one that ends with an exit code other than 0 and
  101. A script refused once that runs to its end when it is run again, as
  the last prefix, is E037 too: the app does not do the same on every run.
- The command's first word is the app: a file of that name in the spec's
  directory (`pixie "greeter"` runs `./greeter`), and otherwise a program
  on PATH, since an app is something built rather than installed.
- A refused action's message gives the screen just before it, and what on
  that screen the action could reach: the buttons and links for a click,
  the text fields for `input` and `submit`. `tests/pixie/greeter.geas`
  (four claims) ran in 0.03 s, and so did `tests/pixie/fails.geas`, a
  refused click and the prefixes it took among its runs.
- pixie's greeter has nothing bound to Enter, so `press("enter")` is
  refused there (measured), and `submit()` is how a claim sends Enter to a
  field.

### 8.4 Chrome, the second driver (live)

A `serve` target can be driven in headless Chrome: `open("/path")` loads
`http://127.0.0.1:<port>/path`, and the other actions act on that page.
`get` and `post` still reach the same service, in the same claim.

- Chrome is found through `GEAS_CHROME`, then the macOS application, then
  `google-chrome`, `chromium` or `chromium-browser` on PATH. Not found,
  the claims that need it end in E034, and the tests print SKIP.
- One Chrome per run (one per worker under `--jobs`), headless, with
  `--remote-debugging-port=0` and a profile directory under `.geas/`; the
  port is read from `DevToolsActivePort` in that directory. geas stops it
  and removes the profile when it exits or panics.
- geas speaks the DevTools Protocol over one WebSocket on 127.0.0.1, with
  a session per page (`Target.attachToTarget` with `flatten`). The client
  is standard library only: the RFC 6455 handshake (SHA-1 and base64,
  written in geas; the SHA-1 is §7.5's), masked frames, 16- and 64-bit
  lengths, continuation frames, pings answered. HTTP on the debugging port
  is read by Content-Length, with a timeout: Chrome keeps the connection
  open after it answers, and reading to the end hung (measured).
- Each claim gets a browser context of its own
  (`Target.createBrowserContext`), so cookies and storage do not pass
  from one claim to the next (measured: a second context read `null`
  where the first had stored a value).
- Settling is by virtual time, always. Before a page loads, geas pauses
  the page's clock (`Emulation.setVirtualTimePolicy`, starting at the
  pinned clock, or else now); after each action it grants one second of
  the page's own time (policy `pauseIfNetworkFetchesPending`) and waits
  for `Emulation.virtualTimeBudgetExpired`. Timers due within that second
  fire, a fetch holds the clock until it returns, and after k actions the
  page's clock reads its start plus k seconds (measured: started at
  2026-08-29T00:00:00Z, the page read 00:00:02.000Z after two). A page
  whose budget does not run out within 10 s of real time is E036.
  `advance(ms)` grants that much more. Settling by polling until the tree
  stopped changing was discarded: a timer longer than the polling window
  slips through it.
- `click` finds the node by role and name, scrolls it into view, and
  presses the mouse at the centre of its box, so whatever a person's
  click would land on is what gets it, including an overlay. `input`
  focuses the field and inserts the text (`Input.insertText`); `submit`
  and `press` send key events (Enter submitted a form, measured).
- The tree is `Accessibility.getFullAXTree`, normalized: ignored nodes,
  and nodes without a name whose role is `generic`, `none`,
  `presentation`, `paragraph`, `LabelText`, `ListMarker`, `InlineTextBox`
  or `LineBreak`, hand their children up; `StaticText` becomes `text`;
  the root (`RootWebArea`) is the screen; a field's text child repeating
  its value is dropped.

As built:

- `GEAS_CHROME`, when it is set, is the only Chrome looked at: one that is
  not a file is E034, not a reason to look elsewhere.
- Chrome names its `InlineTextBox`, `ListMarker` and `LineBreak` nodes
  (the text again, `• `, a line break; measured), so those are dropped,
  with what is under them, named or not. Unnamed `MenuListPopup` (a closed
  `<select>`'s list), `sectionheader` and `sectionfooter` (a header and a
  footer inside `<main>`) hand their children up with the layout roles
  above. `image` becomes `img`. The states are read from the properties
  `disabled` and `checked` (`true`, `false`). Chrome's own roles that are not
  ARIA's (`DescriptionList`, `DisclosureTriangle`, `Iframe`) stay as they
  are. Text keeps Chrome's runs: a sentence with a bold word or a link in it
  is several `text` nodes, which `containing` reaches into.
- `Page.navigate` is answered once the service begins to answer the page
  (measured): a path the service never answers is E036 when the navigation
  is not answered within 10 s, and one Chrome refuses (its `errorText`) is
  E036 at once.
- `advance(ms)` grants exactly that much of the page's time, as pixie's
  `advance:` moves its clock; the one second follows the other actions.
- `input` selects what the field holds (`select()`, or a range over the
  node) and inserts the text; an empty text is a Backspace. `press` sends a
  letter, a digit, `enter`, `escape`, `tab`, `backspace`, `delete`,
  `space`, the arrows, `home`, `end`, `pageup`, `pagedown`, `f1` to `f12`,
  and chords of them with `cmd`, `ctrl`, `alt` and `shift`; any other key is
  E013 before anything runs.
- On a `port auto` service, the page's names and values have the service's
  own address written `{port}`, as its HTTP answers do (§10).
- A JavaScript dialog would stop the page; geas accepts it.
- Chrome's profile is `.geas/chrome-<worker>/`. When a signal ends geas, the
  handler kills the groups geas started, Chrome's among them, and returns:
  a handler may not walk a directory. The run, its programs gone, winds
  down, starting nothing new, prints nothing more, removes the profile on
  its way out, and exits with 128 and the signal's number. A thread
  watching for the signal removes the profile and ends geas itself should
  the run not have ended 2 s after it.
- A page that pins no `locale`, `tz` or `clock` sees the machine's
  (measured: on a machine set to Japanese, a date printed as
  `2026年10月3日 3:14`), so a spec whose screens should read the same on
  every machine pins them.
- A spec of one claim that opens a page ran in 0.42 to 0.45 s, Chrome's
  start and stop and the service's included; the nine claims of
  `tests/browser/page.geas` ran in 1.8 to 2.0 s.

### 8.5 Any other GUI: a driver of your own (live)

```
target app {
  driver "python3 my_driver.py"
}
```

§8.1 as a protocol. geas starts the command once per claim (in the spec's
directory, its environment pinned as §12 says) and speaks JSON lines on
its stdin and stdout:

- first `{"geas":1,"pins":{…}}` with the pins of §12 that are set; the
  driver answers `{"ok":true}`, or `{"error":"…"}` for a pin it cannot
  keep (E011);
- then one action a line, `{"do":"open","path":"/"}`,
  `{"do":"click","name":"greet","nth":1}`,
  `{"do":"input","text":"Ada","field":1}` (or `"into"`), `{"do":"submit",…}`,
  `{"do":"press","key":"enter"}`, `{"do":"advance","ms":500}`, each
  answered by one line, `{"screen":<node>}`, or
  `{"error":"…","screen":<node>}` when the app refused the action (E035).
  A node is `{"role":…,"name":…,"value":…,"states":[…],"children":[…]}`
  without the members that are empty;
- last `{"do":"close"}`, and the driver exits. An answer that does not
  come within 5 s, or is not of that shape, is E037.

This is how a GUI that geas does not drive itself is reached (an
accessibility driver for macOS, one for Android) without geas growing.
The tests use a fake driver of a few dozen lines of Python, so the screen
checks, the errors and the interleaving are held to the contract on any
machine with Python.

As built:

- The pins line carries the pins as the journal's `env` event writes them
  (`{}` when there are none), and the driver gets the same environment a
  process of the target would (`env`, `tz`, `locale`).
- `click` always sends `nth` (1 when the claim does not say), `input` and
  `submit` send `"field":1` when the claim names no field, and `into` comes
  with `nth`.
- An answer with `error` is E035, and the screen it carries is in the
  message's notes. A driver that exits or closes its stdout before
  answering is E037, with the last lines of its stderr.
- At the claim's end the driver gets `{"do":"close"}` and 5 s to exit; what
  is left of its process group is then killed.

macOS's accessibility API is not built in. A process may read another
app's tree only after a person grants it Accessibility in System Settings;
a test run cannot, and nothing in this session could verify it. A driver
can be written against this protocol and verified where the permission
can be given.

### 8.6 Screen checks

`screen contains <node>` holds when some node of the screen matches;
`does not contain`, when none does; `contains exactly n <node>`, when n
do. A node pattern matches a node by role; then by its label, the whole
name, or a part (`containing "…"`), or a pattern (`matching "…"`, §9);
then by `with value "…"`, by a state, and, with `in <role> ["<name>"]`,
by an ancestor. `mask screen <node>` takes the matching nodes and what is
under them out of drift.

The screen goes into the journal and the baseline as JSON, and into
messages as text, one node a line, two spaces a level: `button "greet"`,
`textbox "type here" value "Ada"`, `button "retry" disabled`. `drift`
compares two screens as trees: the children of a node are aligned by role
and name (a longest common subsequence), and a node that appears or
disappears, or whose value or state changed, is a change, claimed when a
check of that `when` has a pattern matching the node, before or after.

As built:

- A failed `screen` check says how many nodes matched (`got no such node`,
  `got 2 such nodes`) and shows the screen under the check, cut to 40 nodes
  with the matching part first; a screen that two failed checks of one
  `when` share is shown once (`the screen: as above`). In `--json`, a
  failed `screen` check carries the screen it saw as `screen`.
- Every node that appears or disappears is a change of its own, so a dialog
  that appears with two buttons is three changes, each claimed or not by
  itself: claiming the dialog claims nothing about a button no check names.
  A change is placed by its parent (`screen in dialog "Confirm"`), and what
  went comes before what came.
- A node a `mask screen` pattern matches is written as its role alone in the
  journal and the baseline (`{"role":"text","masked":true}`). In drift, a
  node masked on either side, or matched by a mask now, is not compared, so
  adding a mask needs no new snap.

## 9. Matchers and subjects

| matcher | applies to |
|---|---|
| `is`, `is not` | every subject but `screen`; `true`, `false` and `null` are values now, for JSON |
| `is above`, `is below`, `is at least`, `is at most`, `is between a and b` (both ends in) | exit, status, a JSON number, a text that is a number once trimmed |
| `contains`, `does not contain` | a text (a part of it), the screen (a node, §8.6) |
| `matches`, `does not match` | a text, a JSON string: the whole value against a pattern |
| `exists`, `does not exist` | `body json "…"`, `header "…"` |

- `is` and `matches` compare a text after dropping one trailing newline,
  as v0's `is` did; `contains` looks in the whole text, as v0's did.
- A number matcher reads a text as a number when, trimmed, it is written as
  one: a sign, digits, a fraction and an exponent, each but the digits
  optional. `inf` and `NaN` are not numbers here, though Rust would parse
  them.
- `header "<name>"` is a subject now, the name compared without regard to
  case. v0 recorded headers for drift, but no check could name one, so
  every header change was unclaimed by construction. Several headers of
  one name read as one, their values joined by `, ` as RFC 9110 joins
  them.
- A value that is not there fails every matcher but `does not exist`: a
  header not sent, a path that leads nowhere. A body that is not JSON
  fails every `body json` check, `does not exist` included: a check is
  about a value, and a broken answer is not one. A failed check shows why:
  `<no header "etag">`, `"many" (not a number)`, `[1] (not a string)`.
- Patterns: literals, `.`, classes (`[a-z]`, `[^0-9]`), `\d \w \s` and
  their negations, groups, `|`, `* + ?`, `{n}`, `{n,}`, `{n,m}`; no
  backreferences, no lookaround. A pattern matches the whole value, by a
  Thompson-style simulation whose time is linear in the text. A pattern
  that does not parse is E009, before anything runs. Globs were discarded
  (they cannot say "digits"), and so were backtracking engines (some
  patterns take exponential time).
- As built: `.` is any character, a line break included, since the whole
  value is matched and a value may hold several lines; `\d` is `0-9`,
  `\w` a letter, a digit or `_` (letters of any script), `\s` white
  space; `\n`, `\t` and `\r` are what they are in a string, and a
  backslash before a character that is not a letter or a digit keeps it.
  `^` and `$` are E009, not anchors: the whole value is always matched.
  A count is at most 1000, and a pattern whose counts spelled out make a
  program of more than 100,000 steps is E009 too. `(a|aa)*b` against
  30,000 `a`s takes milliseconds (a unit test).
- In a claims file, a backslash in a pattern is written as it is:
  `matches "\d+"`. Any other string still refuses a backslash before a
  character other than `n`, `t`, `"` and `\` (E001, as in B), and says
  that a pattern would take it. Writing every pattern's backslash twice,
  `"\\d+"`, was discarded: patterns are where backslashes are dense, and
  the person who reads the claims reads them too.
- A JSON path is checked before anything runs, after `body json` and
  after `mask body json`: one that does not read is E009, pointing at
  the character where reading stops. B found it at run time, as a failed
  check.
- A matcher that does not fit its subject or its value is E008, before
  anything runs, as v0 already did for `contains` on a number: a text
  matcher (`contains`, `matches`, `exists`) on a number; `exists` on a
  subject that is always there; a number compared with a string,
  `true`, `false` or `null`, and a text with `true`, `false` or `null`;
  a number matcher given anything but a number, or `is between` with its
  larger end first; `contains` given anything but a string; `matches`
  given anything but a pattern in quotes. One E008 a check, the first
  that applies.

Discarded: JSONPath or jq filters (a second language inside the claims;
the path stays `.a[0].b`); arithmetic between observations (claims stay
observations).

## 10. Running claims side by side

`--jobs N` (`-j N`, or `GEAS_JOBS`) runs up to N claims at once. The
default stays 1: v0 promised one claim at a time (§2), and a spec whose
targets share something geas cannot see, a file or a database, would turn
flaky without a word.

- The report, the journal and the baseline come out in claim order,
  whatever finishes first, so a run with `-j4` prints and writes, byte for
  byte, what `-j1` does.
- Ports are what geas can see. `port auto` gives each instance a free port
  on 127.0.0.1: geas binds port 0, reads the number, closes it, and never
  hands one number to two live instances. `{port}` in the target's command
  and in its `env` values is replaced by it. Claims that touch a fixed
  `port 8123` never run at the same time (a lock per fixed port); other
  claims run beside them.
- `serial` on a target runs the claims that touch it one at a time, for
  state geas cannot see.
- On a `port auto` target, `127.0.0.1:<port>`, `localhost:<port>` and
  `[::1]:<port>` in what it answers are written `{port}` before checks,
  the journal and drift see them: the port belongs to the environment,
  not to the program's behavior, and a redirect would otherwise drift on
  every run. A claim writes `{port}` where the port appears.
- Each worker has its own Chrome (§8.4).

Discarded: parallel by default (above); isolating claims in containers or
namespaces (per platform, and the implementation would run somewhere
other than where the claims say).

As built:

- A free worker takes the first claim, in spec order, that need not wait,
  and holds its fixed ports and `serial` targets for the claim's whole run.
  The report, the journal, the baseline, the record and `map`'s warnings
  are put together in claim order afterwards, so `-j4` gives the bytes
  `-j1` gives on every example (`tests/languages.rs` holds `map`, `check`
  and `snap` to it, `tests/parallel.rs` the greeter, `check --json` and
  `drift` included). In `map`, each worker reads its claim's coverage as
  soon as the claim ends; the LLVM tools are looked for once, by whichever
  claim needs them first.
- `-j4` is taken as `-j 4`, as make and cargo take it. `--jobs 0` is E080;
  a `GEAS_JOBS` that is not a whole number from 1 counts as unset, as a
  `GEAS_LANG` geas does not know does.
- A port the system handed out may be taken by another program between
  geas closing it and the service binding it; the service then fails to
  start, E032. A message names an auto port as "the port geas gave it",
  and gives the command with `{port}` in it, since the number changes
  from run to run and the output must not.
- geas could also keep such a port open itself. A program being started
  holds a copy of every descriptor geas has open until it execs; when
  another worker had the socket `port auto` reads its number from open at
  that moment, the copy kept the port listening after geas closed it,
  with nothing to accept on it. The claim given the port found its
  service ready on a connection to the copy, and its request was cut off
  when the copy went: E033, "closed the connection without answering",
  with nothing on the service's stderr. It showed on GitHub's runner, on
  `snap -j4` of tally-go. On Linux it came back with tally-node in 4 runs
  of 14, the listening socket on the port held by no process when the
  claim connected. Picking a port and starting a program now take one
  lock (`proc::starting`), so no program starts while that socket is
  open, and the same runs came back clean 10 times of 10. A start takes
  a moment, and claims still run side by side.
- `port auto` on a target whose command and `env` values never say
  `{port}` is E004: the service would have no way to learn its port.
- A masked value, of a header or of a JSON path in a body that is JSON, is
  written `"<masked>"` in the journal and the baseline: a value the spec
  declares to be noise would otherwise make two runs, or `-j1` and `-j4`,
  differ where nothing did. Checks see the value itself. Adding a mask
  still needs no new snap; removing one does, since the baseline holds
  `<masked>` where the value was.

### 10.1 Process groups

Every program geas starts runs in a process group of its own, and is
stopped as a group: killed in `check`, `snap` and `drift`; in `map`, sent
SIGTERM, given 5 s, then killed; and geas waits until no process of the
group is left. So a server behind a wrapper (`sh -c 'python3 server.py
{port}; true'`, `npm start`, `uv run`) is stopped with it, and in `map` it
gets the SIGTERM it needs to write what it ran. Before, only the wrapper
got the signal: the server went on running after geas, holding its port,
and gave no record. A `run` command that leaves a program running in the
background has it stopped when the command ends, as soon as the command
ends rather than when the 5 s run out, since the program held the
command's output open. A program that leaves its group (a daemon calling
`setsid`) is out of reach, as before.

A terminal's Ctrl-C reaches only geas's own group then, so geas catches
SIGINT, SIGTERM and SIGHUP, kills the groups it started, and exits with
128 and the signal's number. The handler reads only atomics and calls
`kill` and `_exit`, declared in the `extern` block beside `kill`, so geas
stays without dependencies. `tests/parallel.rs` interrupts a claim and
finds neither the shell, nor the server behind it, nor a `sleep` alive.

Discarded: leaving a wrapper's children to the wrapper, which most shells
do not signal; and walking the process table to find a program's
children, which differs from system to system and races with programs
that start more.

## 11. Command strings

`run` and `serve` strings are split into words by geas, never by a shell.
Blanks (white space, what v0 split on) separate words; `'…'` keeps
everything in it as it is; `"…"` keeps blanks and reads `\"` and `\\`;
outside quotes, a backslash keeps the next character as it is. Nothing
else is special: no variables, globs, `~`, pipes or redirections.
`{port}` is replaced after splitting. In a `.geas` string a double quote
is written `\"`, so single quotes are the easy form:
`run "python3 'my calc.py'"`. A shell, when one is wanted, is named:
`run "sh -c 'cd tools && ./gen'"`. An open quote or a trailing backslash
is E010, pointing at the character inside the string where the problem
opens, and so is a command with no word at all, found as the spec is
read (v0 found an empty command when a claim ran it).

This is the one change to v0's meaning: v0 split on blanks only and passed
quotes and backslashes on as characters. Commands without them split as
before.

Discarded: running commands through `sh -c`. Expansion and quoting would
then depend on the machine's shell, and a claims file would mean
different things on different machines.

## 12. Pinning what a target sees

A pin fixes part of what a target sees. At the top level it applies to
every target; inside a target it overrides the top level there.

| pin | a process (`run`, `serve`), a pixie app | a page in Chrome | a driver |
|---|---|---|---|
| `env "NAME" "value"` | sets NAME | (the service's process gets it) | sets NAME |
| `env clean` | the environment is `PATH` and what the spec sets or passes | (the service's process) | the same |
| `env pass "NAME"` | under `env clean`, NAME as geas has it | (the service's process) | the same |
| `tz "Asia/Tokyo"` | `TZ` | `Emulation.setTimezoneOverride` | in the pins line |
| `locale "ja-JP"` | `LANG` and `LC_ALL` as `ja_JP.UTF-8` | `Emulation.setLocaleOverride` | in the pins line |
| `clock "2026-08-29T09:00:00+09:00"` | E011, unless `env "NAME"` follows | where virtual time starts (§8.4) | in the pins line |
| `seed 7` | E011, unless `env "NAME"` follows | `Math.random` replaced, before any of the page's scripts, by a generator seeded with 7 | in the pins line |

`env clean` keeps `PATH` so the program can be found; anything else, HOME
included, is passed by name. With `env "NAME"` after it, `clock` and
`seed` set NAME to the time (as written) or the number, for a program that
reads them there.

geas cannot set the clock or the random numbers of a process from
outside: no switch is read by every runtime. Faking them inside the
process was discarded, whether by libfaketime through
`DYLD_INSERT_LIBRARIES` or `LD_PRELOAD` (per platform, and macOS refuses
it for its own binaries) or by patching `time` and `random` through the
coverage hook: either is the reach into the implementation §0 rules out.
A program whose output depends on the time or on chance takes them through
its boundary, as an environment variable the spec names, or its claims do
not depend on them and drift masks them. A page is different: geas runs
the page's world, so it can keep the page's clock and `Math.random`
(measured: the same seed gave the same first number in two browser
contexts, and `de-DE` printed 1234.5 as `1.234,5`).

Python's hash randomization changes the order of sets and of some
dictionaries' keys in output. The skill tells agents to pin
`env "PYTHONHASHSEED" "0"` for Python targets; geas does not set it on its
own, because a pin the spec does not show is one its reader cannot audit.

The journal records each target's pins at the start of each claim, and
`drift` notes when they differ from the baseline's.

As built:

- A pin inside a target replaces the same pin outside it (`tz` by `tz`,
  `env "A"` by `env "A"`); `env clean` and `env pass` add up. One place
  pinning one thing twice is E003, and so are two pins that set one
  variable of a target (`tz` and `env "TZ"`, wherever each is written):
  which one wins would be written nowhere a reader looks.
- The `env "NAME"` after `clock` or `seed` is read on the same line only,
  so an `env` pin on the next line is never taken for it.
- A pin outside any target is checked on every target and said once,
  naming the targets it fails on: `clock` without `env` is one E011 for
  `calc` and `report`. `{port}` in an `env` value on a target without a
  port is E010, as in a command.
- E011 also covers a value geas cannot read: a `clock` that is not RFC
  3339 (`T` and `Z` may be lower case, as RFC 3339 allows; an offset is
  `+09:00`), a `locale` not written `ll-RR` (`ja_JP.UTF-8` is told to
  write `ja-JP`), an empty `tz`, a `seed` that is not a whole number
  from 0, a variable's name that is empty or holds `=`.
- In `map`, the coverage switches are added on top of what the pins
  give: `PYTHONPATH` gains geas's hook in front of what the program would
  have seen, after `env clean` as well, and `NODE_OPTIONS` the hook after
  it, so a pinned target is recorded like any other (`tests/pins.rs`).
- The pins as the journal's `env` event and the baseline's first line
  write them: the members that are set, in a fixed order (Appendix A of
  PLAN.md). A baseline without that line, written before pins, is read
  as one whose targets had none.
- A service some claim opens as a page (any action on it, in any claim)
  takes `clock` and `seed` without `env`: they keep the page's clock and
  `Math.random`, and its process gets nothing for them. With `env "NAME"`,
  the process gets NAME as well. A driver takes them without `env` too, in
  its pins line, and says with `{"error":"…"}` when it cannot keep one,
  which is E011 when the claim runs; so is a `tz` or a `locale` Chrome
  refuses.
- A pixie app is a process for its pins: `env`, `tz` and `locale` reach it,
  and `clock` and `seed` need `env`.

## 13. Diagnostics, in two languages

Every problem geas reports about a spec, a run, a baseline or a record has
a code, a place, and the input or the run that leads to it, in the shape
dandori and rulec use. The first line is
`error[<code>]: <file>:<line>:<col>: <message>`; the line of the spec
follows; then notes, each `= …`; then the run that gets there, which for a
claim is its `when` lines up to the problem, each with what it observed,
cut to one line.

- A failed check is a verdict, not a diagnostic: `not ok <n> - <name>` as
  in v0, then the check, what it expected and what came back, and the run
  that gets there.
- `--lang ja`, or `GEAS_LANG=ja`, or `RITSU_LANG=ja` (the variable every
  language of ritsu reads, after `GEAS_LANG`), prints the prose in Japanese;
  English is the default. `--lang` takes `en` or `ja` and refuses anything
  else (E080); a variable with another value is passed over, since a variable
  left in an environment should not stop every command (`ja_JP` reads as
  Japanese, `fr` as no choice at all). `ok` and `not ok` stay as they are (they are TAP), and so
  do JSON keys. geas has two readers: an agent, which reads English and
  JSON, and the person who audits the claims, whose claims may well be
  named in Japanese. rulec and dandori make the same split.
- `--json` gives the diagnostics as data, with the same keys as dandori's:
  `code`, `severity`, `line`, `col`, `message`, `notes`, `path`, and one
  more, `file`: a diagnostic can point into a file other than the spec (a
  line of the baseline for E051), and without it that line would be read as
  the spec's.
- `geas explain <code>` prints when a code appears, what usually fixes it,
  and a spec that produces it; `geas explain --all` prints every one, and
  the skill carries that output. With `--json` each repro comes as files
  and arguments a program can run, and the tests run every one of them.

| code | what it reports |
|---|---|
| E001 | a character, a string or a token the grammar does not allow there |
| E002 | an unknown name: a target, a call, a subject, a matcher, a role, a state, a pin |
| E003 | two targets, or two claims, with one name; one thing pinned twice |
| E004 | a target whose lines do not fit together: no `run`, `serve`, `pixie` or `driver`, or two of them; `serve` without `port`; `port` without `serve`; `port auto` with no `{port}` in the command or an `env` value |
| E005 | a claim that cannot run: no steps, or a `then` before any `when` |
| E006 | a call its target does not take |
| E007 | a check of something its `when` does not observe |
| E008 | a matcher that does not fit its subject or its value |
| E009 | a pattern, or a JSON path, that does not parse |
| E010 | a command that cannot be split into words, or has none; `{port}` on a target without a port |
| E011 | a pin geas cannot keep for this target, or a time, a locale, a seed or a variable's name it cannot read; a pin a driver or Chrome refuses when the claim runs |
| E012 | another target's `when` between two actions on a replayed (pixie) target |
| E013 | an argument an action cannot take there: `into:` on pixie, `nth: 0`, `field: 0`, `advance(0)`, a path on pixie or `open()` after another action on it, a path without `/` in a browser, a browser action before `open`, a key geas does not send to a page |
| E030 | a program that could not be started |
| E031 | a `run` that did not finish within 5 s |
| E032 | a service that exited before opening its port, or did not open it within 5 s |
| E033 | an HTTP exchange that failed: refused, cut off, malformed, or no answer within 5 s |
| E034 | Chrome not found, not started, or not answering on its port |
| E035 | an action the app refused: no control of that name, pixie's exit 101, a driver's error |
| E036 | a page that did not load, or did not settle within 10 s |
| E037 | output of a GUI driver geas cannot read: pixie's trees, a protocol line, no answer within 5 s |
| E050 | no baseline: run `geas snap` first |
| E051 | a baseline that cannot be read |
| E060 | no record: run `geas map` first |
| E061 | a record that cannot be read, of another format version, or of another spec |
| E062 | a stale record: a file's hash on disk or in the diff is not the recorded one |
| E063 | added lines with only a record of the code before the change |
| E064 | a diff that cannot be read, or that fits neither side of the file on disk |
| E065 | a coverage converter missing or failing (`go tool covdata`, `llvm-profdata`, `llvm-cov`), or a coverage file that does not read |
| E066 | a service that stopped without writing its record: killed after 5 s of SIGTERM, or a Go program's metadata without counters |
| E080 | arguments the command does not take |
| E081 | a file that cannot be read or written |
| W060 | a target that gave no record at all in `map`, unless W061, E065 or E066 says why |
| W061 | a Rust profile from a program geas did not start itself |

### 13.1 What ritsu's base layer took over

geas now lives in ritsu, one workspace for seven small languages, and the
code it shared with the others is ritsu-base's and ritsu-testkit's (ritsu's
PLAN, C.6): the two languages of the prose (`Text`, `tr!`, `Lang`), the part
of a diagnostic every language has (code, severity, place, message, notes,
the text and the JSON), and finding the root (the nearest directory with a
`.git`). The sentences were turned from `t(en, ja)` into
`tr!("日本語", "English")` mechanically, and not one of them changed: every
golden file, in both languages, is what it was. What stays geas's is its
part of a diagnostic (the run that gets there, and the excerpt of the line
with its control characters made visible and cut to 120 characters), the
command line and its help (a hand-written page and a table of its own, as
rulec's), the explanations of the codes (each repro a command, files, an
exit code, the programs it needs), SHA-1 for git's blob ids, which files a
record covers, and the JSON reader.

The JSON reader stays because of what it reads: the output of the program
under test, whose numbers geas compares as floats and which may write them
with an exponent (`2e3`), and whose values geas writes back into baselines
and drift reports in its own form. ritsu-base's reader is rulec's, written
for JSON the suite's own tools print: it refuses an exponent and a key given
twice, and keeps integers exact. The tests read what geas prints with
ritsu-base's reader.

What a person or a program sees changed in four places:

- A diagnostic with no line, such as E081 for a file that is not there,
  now has `"line": null` and `"col": null` in its JSON, where it had `0`:
  a line number is not the way to say there is none, and every language of
  ritsu says it the same way. The golden file
  `tests/golden/en/errors/E081-missing.json` was written again. The report
  of `geas check --json`, which is geas's own format, still writes `0`.
- A diagnostic's JSON is written by ritsu-base, as serde_json writes JSON:
  a backspace or a form feed in a message is `\b` or `\f`, where geas
  wrote `\u0008` and `\u000c`. Both read as the same string.
- `RITSU_LANG` is read after `GEAS_LANG`, and a value such as `ja_JP` reads
  as Japanese.
- The tests' SKIP lines are ritsu-testkit's, `SKIP: geas: <reason>` on
  standard output, and their scratch directories are under the system's
  temporary directory, not under `target/`.

In the first part of ritsu's stage D, geas became a library and a command:
`src/lib.rs` holds every module, and `src/main.rs` is the command line over
it. A program that holds geas as a library reads a spec through ritsu's
ports (`src/ports.rs`; ritsu's DESIGN 3.2): the claims, each with the line
it starts on and its steps as written; the record `geas map` keeps, read
where `geas map` writes it when no `--out` says otherwise
(`.geas/<stem>.map.jsonl` beside the spec); and the claims as items, each
defined by its block (the `claim` line and its steps, each line without the
spaces around it, the blank lines and the comments left out). Nothing is
run. The command line, its exit codes, its output and its JSON did not
change.

In the last part of ritsu's stage D, the port of claims also answers what a
diff comes to for a spec's claims (`affected`): yuen follows the claims it
touches to the requirements that name them (yuen's DESIGN 8). The answer is
the one `geas affected` prints, from the same function (`answer_for`):
which side of the change each record is of, the claims the change touches
and the lines, the changed lines no claim runs, and the files deleted,
outside the source and of the spec. Reading a unified diff moved into
ritsu-base (`ritsu_base::udiff`), since yuen reads diffs too; what a side's
git blob is stays here (`src/diff.rs`). The command line, its exit codes,
its output and its JSON did not change.

In the first part of ritsu's stage E, the command moved from `src/main.rs`
into the library, as `geas::cli::run`: the binary of this crate calls it,
and so does `ritsu geas` (ritsu's DESIGN 8.2). For `ritsu check`,
`geas::cli::checked` runs `geas check` on a project's specs and hands back
what the command prints, a piece at a time (ritsu's DESIGN 8.3): a line for
each claim, from the same function (`report::claim`, which `report::claims`
now calls a claim at a time), the diagnostic of a claim that could not run
or of a spec that does not parse, each with its JSON as `--json` writes a
diagnostic, and the summary line. The claims run as `geas check` runs them,
and the journal is written beside the spec. The command line, its exit
codes, its output and its JSON did not change.

## 14. Distribution through the agent channel

The skill is the other half of §0's division of labor: the person reads
only the claims, and the agent arrives already knowing the CLI.

- `skills/geas/` is an Agent Skill (agentskills.io): `SKILL.md`, written
  by hand, and reference pages for the language, the commands, the codes
  (the output of `geas explain --all`), GUI targets, the record and
  `affected`, pins and parallel runs. The README links into these pages
  instead of repeating them, so each exists once.
- The binary carries the same files (`include_str!`). `geas skill` prints
  `SKILL.md`; `geas skill --install <dir>` writes the folder as
  `<dir>/geas/`, for a project's `.claude/skills` or a user's own skills
  directory; the first lines of `geas --help` say so, so an agent that has
  only the binary finds them. This is cute's `install-skills`, kept to the
  one open folder format. Writing into other tools' configuration (a
  Cursor rules file, a global `AGENTS.md`) was discarded: it can overwrite
  what a person wrote there.
- What the skill teaches is the loop and its rules. The agent writes the
  implementation, never the claims: a failing claim is fixed in the code
  unless the person says the claim is wrong. A claim the agent thinks is
  missing (code `affected` calls unclaimed, a change `drift` calls
  unclaimed) is proposed to the person as text, to be read and accepted,
  not written in. A `mask` line and a re-snap change what the claims
  promise or what drift compares against, so they are asked for too. The
  skill also says how to run each command and read each answer, and what
  each language needs for `map`.
- `tests/skill.rs` holds the skill to the tool: the copy in the binary is
  the folder; every command and flag it names is in `geas --help`; every
  code it names exists; every `geas` block in it parses; the frontmatter
  has `name` and `description`; no link leaves the folder.

The frontmatter's `license` field is the repository's, MIT OR Apache-2.0
(§5, item 4).

As built:

- The folder has seven pages: `SKILL.md` (when it applies, who writes
  what, the loop, the language on one page, reading the answers, from a
  diagnostic to a fix, what `map` needs from each language, GUI targets,
  pins and `--jobs`, what to ask the person), `language.md`,
  `commands.md`, `codes.md`, `gui.md`, `map.md` and `examples.md`. Pins
  and parallel runs have no page of their own: they are in `language.md`,
  `commands.md` and the SKILL's §8, where an agent looks for them.
  `skills/README.md` says how to install the folder, and how to point an
  agent that does not read Agent Skills at `geas skill` from its own
  instructions file.
- Every page but `codes.md` is written by hand. `skills/sync.sh` writes
  `codes.md` from `geas explain --all`; the geas it runs is `$GEAS`, else
  `target/debug/geas`, else the one on PATH.
- `geas skill --install <dir>` makes the directories it needs and prints
  `wrote the geas skill to <dir>/geas (7 files)`. A `<dir>/geas` that is
  already there is E081 unless `--force` is given; with it, the skill's
  files are written again and any other file in the folder is left as it
  is, since geas does not remove what it did not write. A file given to
  `skill`, and `--force` without `--install`, are E080. `--help` names
  `geas skill` on its second line.
- The frontmatter has `name`, `description` and `compatibility` (the
  binary, and what each optional feature needs).
- What §14 gave `tests/skill.rs` is split in two. `tests/skill.rs`:
  `geas skill` prints `SKILL.md`; what `--install` writes is the folder,
  file for file and byte for byte; the refusal and `--force`; `codes.md`
  is what `sync.sh` writes; no link leaves the folder; the frontmatter;
  and the driver `gui.md` gives runs, with the claims it gives for it.
  `tests/docs.rs`, over README.md, README.ja.md, `skills/README.md`, the
  skill's pages and the examples' READMEs: every block fenced `geas`
  parses, one that starts as an example's claims file is that file as it
  is, and one with `…` lines is an excerpt of a claims file in the
  repository; every console block that shows output, and every text
  block that starts as geas's output does, is lines of a golden file,
  `$ ` lines included, with `…` standing for lines left out; every code
  named is in the table, and the count of codes the READMEs give is the
  table's; every geas command and option named is in `geas --help`; every
  relative link leads to a file. Whether a block parses is asked of
  `geas drift --json`, which reads the spec and, with no baseline, stops
  at E050 before running anything.
- `tests/readme.rs` runs the stories the READMEs tell, as they tell them,
  and keeps each transcript as a golden under `tests/golden/{en,ja}/readme/`:
  the `$ ` lines a person would type, what each command printed, and the
  exit status as `$ echo $?` where the page shows it. The greeter's story
  runs in a git repository of its own, so `git diff | geas affected …`
  runs as it would in a project; a step the test takes itself (writing an
  agent's change into a file) has no `$ ` line, and the page says it in
  words.
- English counts in reports are singular for one (`1 claim`,
  `1 changed line`), where every count was plural before; the summary of
  `affected` is on the README.
- `examples/web-greeter` pins `locale`, `tz`, `clock` and `seed`, since its
  page writes the date, greets by the hour and draws a fortune with
  `Math.random`; with them, its claims name the exact date and greeting,
  and drift on the unchanged page is quiet. Its change in
  `tests/changes/web-greeter/` turns the Greet button into a `<div>` with a
  click handler, which the screen shows as `text "Greet"` and the claims
  refuse. `examples/pixie-greeter` holds no app: the greeter is a 57 MB
  binary, so its README says how pixie builds it, and `.gitignore` keeps
  `examples/pixie-greeter/greeter` out. `examples/greeter/greeter.ja.geas`
  is the greeter's claims named in Japanese, which README.ja.md runs with
  `--lang ja`.

## 15. Still out, and why

- **Windows.** The process and service adapters rest on Unix behavior:
  SIGTERM to ask a service to stop, and so to write its record (§7.3).
  Windows has no SIGTERM, and stopping a console program gracefully goes
  through console events. Nothing here can run Windows to check it, and an
  adapter that has not been run is not offered.
- **Claim dependencies.** Each claim builds its own state with its own
  `when`s and can be run and replayed alone (§2). A claim that relied on
  another's run would make one verdict depend on another, would break the
  isolation `--jobs` (§10) and the per-claim record (§7.3) rest on, and
  would hide from the reader the setup that makes the claim true.
- **macOS accessibility.** The permission (§8.5).
- **Recording pixie apps and page scripts.** A pixie app is Rust that pixie
  generated, and its lines are not the `.pix` source anyone wrote. A
  page's scripts could be recorded through the DevTools Protocol in the
  same V8 shape as Node's, but the URLs a page loads do not name files
  under the root; joining them needs a declared mapping, or source maps
  for bundled code. Both wait for that mapping to be designed.
- **Choosing an option** (`select`). pixie's choosers and a browser's
  native `<select>` popup act differently enough that one action for both
  needs a design of its own; until then a select is reached with `press`.
- **A library through an FFI harness** (§3). A library is claimed through a
  small command or service around it, which the agent writes.
- **Requests to pixie** that would make its driver exact: an escaped
  accessibility dump (or a JSON one), and the refusal's message on stderr
  when an app runs a script by itself. Until then, the test of E037 on a
  pixie app is the reader's unit tests: no app prints a tree geas cannot
  read.
- **A program that leaves its process group** (a daemon calling `setsid`)
  is out of reach of the group geas stops (§10.1). Following it would mean
  walking the process table, which §10.1 discarded.
- **A Chrome that dies in the middle of a run** is not started again: the
  later claims of that worker end in E034, which says so. Whether a new
  one should take over, and what that would say about the claims the old
  one ran, is not settled.
- **What is inside an `iframe`.** Chrome reports the frame as one node,
  and geas does not read the tree of the document in it.
- **A literal `{port}`.** On a `port auto` target, the port in what a
  service answers is written `{port}` (§10), so a service that prints
  `{port}` itself cannot be told from one that prints its address, and a
  `content-length` no longer counts a body the port was replaced in. There
  is no way yet to write either.
- **A `map` stopped by a signal** leaves `.geas/hook/` and `.geas/cover/`;
  the next `map` removes them.
- **A file the change adds, with only a record of the code after it.**
  `git diff` leaves out files git does not track, so `affected` does not
  see them; `git add -N` puts them in the diff, and the skill says so. A
  record of the code before notices such a file (E062).
- **Naming and publication** stay with the owner (§5, item 4). The
  license is chosen: MIT OR Apache-2.0, in `Cargo.toml`, LICENSE-MIT and
  LICENSE-APACHE, the READMEs and the skill's frontmatter.

## 16. Measured to settle the design (2026-10-02, macOS arm64)

Each was run once, small, in a scratch directory outside the repository;
none of it is geas code.

- The spike, built with rustc 1.94.1, ran both examples as the README
  shows, and drift on the unchanged greeter was quiet (exit 0).
- Python 3.14.6: a `sitecustomize.py` on `PYTHONPATH` recorded the lines
  `calc.py` ran (`1 / 0` ran the division branch and not the others), and
  the lines a service ran before SIGTERM; the service exited with 143 and
  the record was written.
- Node v23.11.0: a `.ts` command run with `NODE_V8_COVERAGE` gave ranges
  that fall on the `.ts` file's lines; a service killed by SIGTERM wrote
  nothing; with the SIGTERM hook preloaded through `NODE_OPTIONS`, a
  service with no handler of its own and one that closes gracefully both
  wrote theirs.
- Go 1.25.5: `go build -cover -trimpath`, `GOCOVERDIR`. A command wrote
  its counters, and `go tool covdata textfmt` gave its blocks, in import
  paths; a service killed by SIGTERM left only `covmeta`; one that stops
  through `signal.NotifyContext` and `Shutdown` wrote both. The probes
  grew the Go build cache by about 0.2 GB.
- Rust 1.94.1: `rustc -C instrument-coverage` and `LLVM_PROFILE_FILE`. A
  command ending in `std::process::exit(1)` wrote its profile, and
  `llvm-profdata` and `llvm-cov export -format=lcov` from rustup's
  llvm-tools turned it into lines.
- SHA-1 written in standard-library Rust gave
  `ce013625030ba8dba906f756967f9e9ca394464a` for `blob 6\0hello\n`, which
  is what `shasum` gives for the same bytes.
- Chrome 154.0.8037.93, headless, driven by a standard-library Rust client
  (WebSocket, SHA-1, base64, and geas's JSON module): it started Chrome,
  connected, loaded a page, typed into a field, clicked a button found by
  its accessible name, and read the tree before and after, in 372 to
  543 ms all told, Chrome being up in 161 to 234 ms. Over one socket with
  a session per page: a browser context per claim kept storage apart; a
  seeded `Math.random` gave the same number in two contexts; virtual time
  started at 2026-08-29T00:00:00Z read 00:00:01.000Z after one budget;
  `de-DE` printed 1234.5 as `1.234,5`; Enter submitted a form; the launch
  and two claims took 653 ms.
- pixie: a greeter built with a newer pixie kernel than the one in pixie's
  own tree, run with `PIXIE_SCRIPT="input:Ada,click:greet"`, printed its
  starting and final trees in 32 ms; `a11y` printed the accessibility
  tree; a step naming a missing button exited with 101, printed nothing
  on stderr, and lost the trees of the `a11y` steps before it;
  `PIXIE_DUMP` moved the transcript into a file; `\,` carried a comma into
  a field, and the tree printed it without escaping.

Measured while building stage B (2026-10-03, the same machine):

- Homebrew's Python 3.14.6 has a `sitecustomize.py` of its own, which adds
  Homebrew's `site-packages`; geas's hook runs it after starting the
  monitor.
- Node v23.11.0 named a CommonJS script in its coverage by its plain path
  and an ES module by a `file://` URL. Its `http` module answered with
  `Transfer-Encoding: chunked` when the handler set no `Content-Length`.
- Go 1.25.5: a binary built with `-cover` and run without `GOCOVERDIR`
  printed `warning: GOCOVERDIR not set, no coverage data emitted`. With a
  fresh build cache, building the tally service took 2.9 s and the first
  `go tool covdata` 1.8 s, and the cache grew to 157 MB; the tests keep it
  in their scratch directory. A service left to Go's default handling of
  SIGTERM wrote `covmeta` and no counters.
- Rust 1.94.1: an instrumented binary run without `LLVM_PROFILE_FILE`
  wrote `default_<signature>_0_<pid>.profraw` into its working directory.
  `llvm-cov export` given a program without coverage data refused the whole
  export (`no coverage data found`), and given macOS's `/bin/sh`, a
  universal binary, asked for `-arch`.

Measured while building the first half of stage C (2026-10-03, the same
machine):

- Node v23.11.0 names a coverage file `coverage-<pid>-<ms>-0.json`, the
  last number staying 0. `v8.takeCoverage()` followed at once by
  `process.exit()` wrote two files 1 ms apart, the second without the
  script's counts; in 33 runs of 60 they fell in one millisecond and the
  second replaced the first. `process.exit(143)` alone, from a SIGTERM
  handler, kept the counts in 30 runs of 30, and so did a program shutting
  down through a handler of its own.
- `(a|aa)*b` against 30,000 `a`s finishes within the unit test's bound of
  one second, in the debug build; a backtracking engine takes time
  exponential in the length on it.
- The suite, with every tool present: 181 tests (94 unit tests, 87 that
  run the binary), no SKIP, 50 s.

Measured while building the second half of stage C (2026-10-03, the same
machine):

- Chrome 154.0.8037.93 gave names to `InlineTextBox` (the text again),
  `ListMarker` (`• `, `1. `) and `LineBreak` (a line break), and reported a
  closed `<select>`'s list as `MenuListPopup`, a header and a footer inside
  `<main>` as `sectionheader` and `sectionfooter`, an `<img>` as `image`,
  `checked` as a property `true` or `false`, and `disabled` as one `true`.
  A typed field held the text twice, as its value and as a `StaticText`
  under a `generic`.
- `Page.navigate` to a path the service never answered was not answered
  either, until the 10 s ran out. To a service that closed every connection
  at once, Chrome answered with an `errorText` that changed from run to run
  (`net::ERR_SOCKET_NOT_CONNECTED`, `net::ERR_EMPTY_RESPONSE`); an answer
  with two `Content-Length` headers gave
  `net::ERR_RESPONSE_HEADERS_MULTIPLE_CONTENT_LENGTH` in each of four runs,
  so the tests and E036's repro use that.
- Chrome started eleven processes (helpers, renderers), all in its process
  group; killing the group left no process naming the profile.
- An unpinned page on this machine, whose language is Japanese, printed a
  date as `2026年10月3日 3:14`; with `locale "de-DE"`, `tz "Asia/Tokyo"` and
  `clock "2026-08-29T00:00:00Z"` it printed `29. August 2026 um 09:00` and
  `1.234,5`, and with `seed 7`, `Math.random()` was `0.000441` (to six
  places) in every claim.
- pixie's greeter, given a field's text with a line break in it, printed
  its accessibility tree across two lines; it refused `key:enter` (exit
  101), having nothing bound to it, and took `submit`.
- The suite, with every tool present and `GEAS_PIXIE_GREETER` naming
  pixie's greeter: 215 tests (117 unit tests, 98 that run the binary), no
  SKIP, 72 s, of which `tests/browser.rs` took 14 s (10 s of them a page
  that never loads). Without the greeter, the two tests of `tests/pixie.rs`
  print SKIP and nothing else does.

Measured while building stage D (2026-10-03, the same machine):

- The suite, with every tool present and `GEAS_PIXIE_GREETER` naming
  pixie's greeter: 236 tests (119 unit tests, 117 that run the binary), no
  SKIP, 79 s; `tests/readme.rs`, which records the READMEs' transcripts,
  took 6 s of it. Without the greeter, four lines say SKIP: the two tests
  of `tests/pixie.rs` and the two pixie stories of `tests/readme.rs`. The
  release binary is 2,327,616 bytes, and a release build from nothing took
  2.7 s.
- `examples/web-greeter`'s six claims ran in 1.8 s, Chrome's start and stop
  included, and drift on the unchanged page was quiet across runs with
  every pin; `examples/pixie-greeter`'s four ran in 0.04 s.
- A `.gitignore` of `**/.geas/*` and `!**/.geas/*.baseline.jsonl` left git
  seeing the baseline and nothing else of `.geas/`. The pull-request recipe
  of `skills/geas/map.md`, run in a scratch repository with a branch
  holding the greeter's change (a record of the base made with `--out`,
  then one of the branch), named the claims and lines of the report
  `tests/languages.rs` keeps for two records.
- Twelve deliberate breakages of the pages each failed the test that
  should catch them (PLAN.md, D6).
