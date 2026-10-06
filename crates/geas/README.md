# geas

**Hold agent-written code to claims a person has read.**

geas is a small language for the acceptance half of coding with agents. Specs (Spec Kit, Kiro,
OpenSpec) drive the generation half and are prose; tests written by the agent that wrote the code are
circular. A claims file says, in lines short enough to read aloud, what a program does as anyone
could observe it from outside: what a command prints, what a service answers, what a screen shows.
geas runs every claim against the real program, whatever language it is written in, and reports
in text for the person and in JSON for the agent.

**You audit 50 lines of claims; the agent writes 5,000 lines of code; the gate binds them.**

```geas
# Claims for a small HTTP service. Each claim gets a fresh server
# instance, on a port of its own, so the stateful scenario below is
# self-contained.

target api {
  serve "python3 server.py {port}"
  port auto
}

# Declared volatility: these fields are noise by nature, not behavior.
mask header "date"
mask header "server"

claim "greets by name" {
  when api.get("/greet?name=Alice")
  then status is 200
  and  body json ".message" is "Hello, Alice"
}

claim "rejects an empty name" {
  when api.get("/greet?name=")
  then status is 400
}

claim "totals accumulate across requests" {
  when api.post("/reset")
  then status is 200
  when api.post("/add", body: "5")
  when api.post("/add", body: "7")
  when api.get("/total")
  then body is "12"
}

claim "unknown paths are 404" {
  when api.get("/nope")
  then status is 404
}
```

A claim never names a function, a file, a CSS selector or a line. geas reaches the program only
where a person could, so the claims file sits beside any codebase, and nothing in the code changes
for it.

- **The loop.** `geas check` runs every claim, each against a fresh instance of the program, and a
  failure comes with the run that gets there.
- **What no claim promised.** `geas drift` runs the claims again and reports everything a person
  could observe that changed, marking what no claim promises.
- **The claims a change touches.** `geas map` records the lines each claim runs, through the
  coverage each runtime already has (Python, Node, Go, Rust); `geas affected` reads a diff against
  that record, runs nothing, and names the claims to read again and the changed code no claim runs.
- **Screens.** A page in a headless Chrome, a pixie app, or any GUI behind a driver, claimed through
  what a screen reader is told: roles, names, values, states.
- **The same run every time.** Pins for the environment, the time zone, the locale, the clock and
  the random seed; claims side by side with `--jobs`, printing the same bytes as one at a time.
- **For agents.** Every diagnostic has a code and the run that leads to it, `geas explain` says how
  to fix it, and `geas skill` carries the guide an agent needs.

## The loop

[examples/greeter](examples/greeter) is a small HTTP service. Its first `server.py` was written the
way an agent writes one: plausible, clean, and without a check for an empty name. The claims caught
it:

```console
$ geas check examples/greeter/greeter.geas
ok 1 - greets by name
not ok 2 - rejects an empty name
    examples/greeter/greeter.geas:22: status is 400 — got 200
        22 |   then status is 400
      the run that gets there:
          21  when api.get("/greet?name=")  →  200, body "{\"message\": \"Hello, \"}"
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 3 ok · 1 failed · journal: examples/greeter/.geas/greeter.journal.jsonl
$ echo $?
1
```

A failed check says what it expected and what came back, then the line of the claims file and the
run that gets there: each `when` up to it, with what the service answered. The fix is the three
lines the claim asked for (`if not name: 400`), and then:

```console
$ geas check examples/greeter/greeter.geas
ok 1 - greets by name
ok 2 - rejects an empty name
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 4 ok · 0 failed · journal: examples/greeter/.geas/greeter.journal.jsonl
$ echo $?
0
```

`--json` gives the same report as data for an agent's loop, and every interaction and every check
goes into the journal, `.geas/greeter.journal.jsonl`, the evidence of the run.

## What no claim promised

A test runner says whether its checks hold, and nothing about what changed where no check looks.
`geas snap` keeps every observation of a run, every header and the whole body, as the baseline;
`geas drift` runs the claims again and compares everything. Here an agent has refactored the
service:

```console
$ geas snap examples/greeter/greeter.geas
4 claims · 4 ok · 0 failed · baseline: 7 interactions → examples/greeter/.geas/greeter.baseline.jsonl
$ cp examples/greeter/server_refactored.py examples/greeter/server.py
$ geas check examples/greeter/greeter.geas
ok 1 - greets by name
ok 2 - rejects an empty name
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 4 ok · 0 failed · journal: examples/greeter/.geas/greeter.journal.jsonl
$ geas drift examples/greeter/greeter.geas
claim "greets by name" when#1 api.get("/greet?name=Alice")
  + body json ".debug": appeared: {"handler":"greet_v2"}   [unclaimed]
claim "rejects an empty name" when#1 api.get("/greet?name=")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [unclaimed]
claim "totals accumulate across requests" when#1 api.post("/reset")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [unclaimed]
claim "totals accumulate across requests" when#2 api.post("/add", body: "5")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [unclaimed]
claim "totals accumulate across requests" when#3 api.post("/add", body: "7")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [unclaimed]
claim "unknown paths are 404" when#1 api.get("/nope")
  ~ header `content-type`: "text/plain" → "text/plain; charset=utf-8"   [unclaimed]
  ~ body: "not found" → "Not Found"   [unclaimed]
drift: 7 interactions compared · 6 drifted · 7 unclaimed change(s) · 0 claimed
$ echo $?
1
```

Every claim still holds, and seven things a person could observe changed: internals leak out as a
`.debug` object, the plain-text answers gained a charset, and the text of the 404 is capitalized.
Each is `[unclaimed]`, since no claim promises anything there, so a person decides whether it may
ship. A change to a field some check looks at is marked `[claimed — `geas check` is the authority]`
instead. JSON bodies are compared path by path, so a claim on `.message` does not hide a new
`.debug` beside it. The service's `date` and `server` headers change on every run; the two `mask`
lines in the claims file keep drift quiet on unchanged code, and adding a mask needs no new
baseline.

## The claims a change touches

`drift` reads the run. The other half reads the diff: which claims this change touches, and which
changed code no claim runs. `geas map` runs the claims with each runtime's coverage switched on from
outside and records, for each claim, the lines it ran. `geas affected` reads a unified diff and that
record, runs nothing, and reports. Here an agent changed `server.py` again: it strips the name
before checking it, adds a `/health` route, and drops an override
([tests/changes/greeter/change.diff](tests/changes/greeter/change.diff)).

```console
$ geas map examples/greeter/greeter.geas
ok 1 - greets by name
ok 2 - rejects an empty name
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 4 ok · 0 failed · journal: examples/greeter/.geas/greeter.journal.jsonl
map: 2 files · 40 lines of code, 38 run by some claim → examples/greeter/.geas/greeter.map.jsonl
$ git diff | geas affected examples/greeter/greeter.geas -
diff: <stdin> · record: examples/greeter/.geas/greeter.map.jsonl (the code after the change)
claims the change touches:
  1 - greets by name
      examples/greeter/server.py: 21
  2 - rejects an empty name
      examples/greeter/server.py: 21
  4 - unknown paths are 404
      examples/greeter/server.py: 29
  every claim that starts `api`: 1, 2, 3, 4
      examples/greeter/server.py: removed 10-11 (next to lines they run)
changed code no claim runs:
  examples/greeter/server.py: 30
4 claims · 4 touched · 1 changed line no claim runs
$ echo $?
1
```

Line 21, the stripped name, is run by the two claims that send a name: those are the claims to read
again. Line 30, the answer to `/health`, is run by no claim. That is behavior nobody has promised,
and the agent's next step is to propose a claim for it. The removed override sat among the lines
every claim runs as the service starts. The record holds each source file's git blob hash, and
`affected` refuses a record of other code (E062) instead of guessing; the record never decides a
verdict, only where a reviewer looks.

| Language | geas switches on | The program has to |
|---|---|---|
| Python 3.12 and later | a `sitecustomize.py` of its own, on `PYTHONPATH` | nothing |
| Node: JavaScript, and TypeScript by type stripping | `NODE_V8_COVERAGE`, and a hook on `NODE_OPTIONS` | nothing |
| Go | `GOCOVERDIR` | be built with `go build -cover`; as a service, return from `main` on SIGTERM |
| Rust | `LLVM_PROFILE_FILE` | be built with `-C instrument-coverage`; as a service, exit normally on SIGTERM |

[skills/geas/map.md](../../skills/geas/map.md) says how each part of the report is decided, and how to
run both halves on a pull request.

## OpenSpec's scenarios

[OpenSpec](https://github.com/Fission-AI/OpenSpec) keeps what a system does as specs in Markdown:
each requirement comes with scenarios, a GIVEN, a WHEN and a THEN in prose, the cases a person and
an agent agreed on before the code was written. A claim is the same case, run against the program.
`geas scenarios` holds each scenario to the claim of its name and says which scenarios no claim
answers; it runs nothing. [examples/greeter/openspec](examples/greeter/openspec) is the greeter's
spec, whose scenarios are named as its claims are, and a change not yet archived, `trim-names`,
which trims the name and adds a health check:

```console
$ geas scenarios examples/greeter/greeter.geas --openspec examples/greeter/openspec/specs
examples/greeter/openspec/specs/greeting/spec.md
  Greeting by name
    greets by name: claim 1 of examples/greeter/greeter.geas (line 14)
    rejects an empty name: claim 2 of examples/greeter/greeter.geas (line 20)
  Running total
    totals accumulate across requests: claim 3 of examples/greeter/greeter.geas (line 25)
  Unknown paths
    unknown paths are 404: claim 4 of examples/greeter/greeter.geas (line 34)
claims no scenario names: none
4 scenarios · 4 with a claim · 0 with none
$ geas scenarios examples/greeter/greeter.geas --openspec examples/greeter/openspec/changes/trim-names
examples/greeter/openspec/changes/trim-names/specs/greeting/spec.md (a change's delta spec)
  MODIFIED Greeting by name
    greets by name: claim 1 of examples/greeter/greeter.geas (line 14)
    rejects an empty name: claim 2 of examples/greeter/greeter.geas (line 20)
    rejects a name of spaces: no claim
  ADDED Health check
    answers the health check: no claim
claims no scenario names:
  "totals accumulate across requests": claim 3 of examples/greeter/greeter.geas (line 25)
  "unknown paths are 404": claim 4 of examples/greeter/greeter.geas (line 34)
4 scenarios · 2 with a claim · 2 with none
$ echo $?
1
$ geas scenarios examples/greeter/greeter.geas --openspec examples/greeter/openspec/changes/trim-names --draft
# examples/greeter/openspec/changes/trim-names/specs/greeting/spec.md
# Requirement: Greeting by name
#   Scenario: rejects a name of spaces
#   - **WHEN** a client asks for `/greet?name=%20%20`
#   - **THEN** the status is 400
claim "rejects a name of spaces" {
  # when <target>.<call>(…)
  # then <subject> <matcher>
}

# examples/greeter/openspec/changes/trim-names/specs/greeting/spec.md
# Requirement: Health check
#   Scenario: answers the health check
#   - **WHEN** a client asks for `/health`
#   - **THEN** the status is 200
claim "answers the health check" {
  # when <target>.<call>(…)
  # then <subject> <matcher>
}
```

The change asks for two scenarios no claim answers yet. `--draft` writes a claim to fill in for
each, with the scenario quoted above it. Its body is only comments, and geas refuses a claim with no
steps (E005), so a draft pasted as it is fails `geas check` until a person writes the steps, or
reads the ones an agent proposes. A claim is matched by its name as written, case and spaces
included; a claim of a near name is pointed out. `--openspec` takes a spec, a change's delta spec
(its ADDED and MODIFIED requirements), or a directory of them, leaving out `archive/`.

## Screens

A GUI claim observes the screen as a screen reader is told it: nodes with a role, a name, a value
and states. [examples/web-greeter](examples/web-greeter) is a page and its service, opened in a
headless Chrome:

```geas
target web {
  serve "python3 server.py {port}"
  port auto
}

# The page writes the date, greets by the time of day and draws a fortune at
# random. Fixed here, they read the same on every machine and in every run.
locale "en-US"
tz "Asia/Tokyo"
clock "2026-08-29T09:00:00+09:00"
seed 7
…
claim "greets the name typed in, by the time of day" {
  when web.open("/")
  when web.input("Ada", into: "Your name")
  then screen contains button "Greet" enabled
  when web.click("Greet")
  then screen contains text "Good morning, Ada!" in status
  and  screen contains text "Ada" in list "Greeted"
  and  screen contains textbox "Your name" with value ""
}
…
claim "the service keeps the names the page greets" {
  when web.open("/")
  when web.input("Ada")
  when web.submit()
  when web.get("/api/greetings")
  then status is 200
  and  header "content-type" is "application/json"
  and  body json ".names[0]" is "Ada"
  and  body json ".names[1]" does not exist
}
```

```console
$ geas check examples/web-greeter/web-greeter.geas
ok 1 - the first screen asks for a name
ok 2 - greets the name typed in, by the time of day
ok 3 - draws a fortune
ok 4 - the service keeps the names the page greets
ok 5 - a name sent to the service shows on the page
ok 6 - the service refuses an empty name
6 claims · 6 ok · 0 failed · journal: examples/web-greeter/.geas/web-greeter.journal.jsonl
```

Each claim gets a fresh service and a fresh browser context. The page runs on virtual time,
starting at the pinned clock, so the date and the greeting read the same on any machine, and a
timer on a page fires without the claim waiting for it in real time. Then an agent restyles the
Greet button as a `<div>` with a click handler. It looks the same, and a mouse can click it:

```console
$ geas check examples/web-greeter/web-greeter.geas
not ok 1 - the first screen asks for a name
    examples/web-greeter/web-greeter.geas:27: screen contains button "Greet" disabled — got no such node
        27 |   and  screen contains button "Greet" disabled
      the screen:
        heading "Greeter"
        text "Today is Saturday, August 29, 2026."
        form
          text "Your name"
          textbox "Your name"
          text "Greet"
        status
        list "Greeted"
        text "Your fortune: A pleasant surprise is waiting for you."
…
not ok 2 - greets the name typed in, by the time of day (error)
    error[E035]: examples/web-greeter/web-greeter.geas:35:3: the page of `web` has no button, link, checkbox, radio, switch, tab, menu item or option named "Greet"
        35 |   when web.click("Greet")
…
      = nothing on it can be clicked
…
6 claims · 4 ok · 2 failed · journal: examples/web-greeter/.geas/web-greeter.journal.jsonl
```

Where the button was, the screen has the text "Greet". A `<div>` is not a button to a screen
reader, and so not to the claim either: the page is what needs fixing.

Claims of the same kind reach a desktop app. [examples/pixie-greeter](examples/pixie-greeter) holds
claims for the greeter of pixie, a GUI language whose apps can replay a script of actions headless;
geas runs each claim's actions as one script and reads the tree after each:

```console
$ geas check examples/pixie-greeter/greeter.geas
ok 1 - asks for a name
ok 2 - greets the name typed in
ok 3 - greets on Enter too
ok 4 - the note is a field of its own
4 claims · 4 ok · 0 failed · journal: examples/pixie-greeter/.geas/greeter.journal.jsonl
```

Any other GUI (an accessibility API, an emulator) is reached through a driver: a program that takes
the actions as JSON lines and answers each with the screen.
[skills/geas/gui.md](../../skills/geas/gui.md) has the actions and the protocol.

## The language

A `when` is an action on a target; `then` and `and` check what it observed.

| Target | Actions | Checks |
|---|---|---|
| `run "<command>"`, a command run for each `when` | `run("a", "b")` | `stdout`, `stderr`, `exit` |
| `serve "<command>"` and `port <n>` or `port auto`, a service started for each claim | `get("/path")`, `post("/path", body: "…")` | `status`, `header "<name>"`, `body`, `body json ".a[0].b"` |
| a `serve` target in Chrome, `pixie "<app>"`, `driver "<command>"` | `open`, `click`, `input`, `submit`, `press`, `advance` | `screen` |

The matchers read as English: `is`, `is not`, `is above`, `is below`, `is at least`, `is at most`,
`is between 200 and 299`, `contains`, `does not contain`, `matches "\d+ items?"` (a pattern held
against the whole value), `exists`, `does not exist`, and on the screen
`contains exactly 2 listitem in list "Items"`. A command is split into words by geas, never by a
shell, with `'…'` to keep blanks in a word. A pin fixes part of what a target sees, for every
target when it is written outside them, or for one when it is written inside: `env`, `env clean`,
`env pass`, `tz`, `locale`, and, for a page or a program that reads them from a variable, `clock`
and `seed`. geas does not reach into a process to fake its clock, so it says so:

```text
error[E011]: E011-clock-without-env.geas:2:1: `clock` cannot be kept for `calc` and `report`: no switch sets the time of a program from outside, so geas passes it in a variable the program reads
     2 | clock "2026-08-29T09:00:00+09:00"
  = name that variable on the same line: `clock "2026-08-29T09:00:00+09:00" env "NOW"`
```

Every diagnostic has a code, the place, and what leads to it: a claim that could not run carries the
run that gets there, as E035 above. `geas explain <code>` says when a code appears, what usually
fixes it, and the smallest claims file that gives it; there are 36 codes, and `--lang ja` prints
them all in Japanese. `--jobs 4` runs up to four claims at once, each service on a port of its own
(`port auto`); the report, the journal and the baseline come out in claim order, the same bytes as
with one, and `serial` keeps a target's claims apart when they share state geas cannot see.
[skills/geas/language.md](../../skills/geas/language.md) has every form.

## For AI agents

The person reads the claims; the agent should know geas from its first run. `geas skill` prints the
guide, an [Agent Skill](https://agentskills.io) that says how to run the gate, how to read each
answer, which diagnostic means what, and what to leave to the person: the agent writes the code,
never the claims, and proposes the claims it finds missing. The binary carries it:

```console
$ geas skill --install .claude/skills
wrote the geas skill to .claude/skills/geas (7 files)
```

The same folder is [skills/geas](../../skills/geas); [skills/README.md](skills/README.md) says more.

## Install

geas is one of the languages of [ritsu](https://github.com/i2y/ritsu), and is built from its
repository with a recent stable Rust. To have all eight, and `ritsu check` for a project that
holds the files of more than one (it runs the claims of a `.geas` file as `geas check` does):

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu geas <command>` is then every command below, and a link to `ritsu` named `geas` does the
same. To have geas alone, which reads no other language:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked geas
```

geas has no dependencies. What it starts is what the claims
need: the project's own programs, Chrome or Chromium for pages, and, for `geas map`, the toolchains
of the table above.

## Commands

```console
$ geas --help
geas: hold agent-written code to claims a person has read
Coding agents: `geas skill` prints the guide; `geas skill --install <dir>` installs it.

usage:
  geas check <spec.geas>...           run every claim; exit 0 when all hold
  geas snap <spec.geas>...            run them and keep every observation as the baseline
  geas drift <spec.geas>...           run them again and report what changed since the baseline
  geas map <spec.geas>...             run them with coverage on and record the lines each claim runs
  geas affected <spec.geas> <diff|->  the claims a diff touches, and the changed code no claim runs
  geas scenarios <spec.geas>... --openspec <path>...
                                      the scenarios of OpenSpec specs, each with the claims of its name
  geas explain <code>... | --all      what a code means and how to fix it
  geas skill [--install <dir>]        the guide for coding agents, or the guide written as a skill folder

options:
  --json            the answer as JSON
  --lang ja         messages in Japanese (also GEAS_LANG=ja)
  --jobs N, -j N    run up to N claims at once (also GEAS_JOBS); default 1
  --root <dir>      map, affected: the directory the record's paths are relative to
  --out <file>      map: where to write the record
  --map <file>      affected: a record to read; give two for both sides of the diff
  --openspec <path> scenarios: a spec, a change's delta spec, or a directory of them
  --draft           scenarios: a claim to fill in for each scenario no claim answers
  --install <dir>   skill: write the skill's files to <dir>/geas
  --force           skill: write over a <dir>/geas that is already there
  --help, -h        this text
  --version         the version

exit: 0 all held, or nothing to report · 1 something failed or changed · 2 the spec, a file, or the arguments are wrong
```

| Variable | What it does |
|---|---|
| `GEAS_LANG` | `ja` gives the messages in Japanese |
| `GEAS_JOBS` | the default of `--jobs` |
| `GEAS_CHROME` | the Chrome to start for pages, instead of looking for one |
| `GEAS_LLVM_BIN` | where `llvm-profdata` and `llvm-cov` are, for Rust in `geas map` |

[skills/geas/commands.md](../../skills/geas/commands.md) has every option, the exit codes, and the files
and JSON geas writes.

## Examples

| Example | What it shows |
|---|---|
| [calc](examples/calc) | a command-line program in Python: `stdout`, `stderr`, `exit` |
| [greeter](examples/greeter) | the HTTP service above, its refactor for drift, the same claims named in Japanese, and its OpenSpec spec for `geas scenarios` |
| [tally-node](examples/tally-node) | a service in TypeScript, recorded by `geas map` with nothing to build |
| [tally-go](examples/tally-go) | the same service in Go, built with `-cover` |
| [tally-rust](examples/tally-rust) | a command line in Rust, built with `-C instrument-coverage` |
| [web-greeter](examples/web-greeter) | a page in Chrome and its service, every pin |
| [pixie-greeter](examples/pixie-greeter) | a pixie desktop app, built with pixie and put beside the claims |

Each has a README saying what it shows, how to run it, and what to look for.

## How it is checked

`cargo test` runs the unit tests of the parts that start no program (the lexer and parser,
patterns, JSON, hashes, line sets, diffs, screens, the coverage readers on recorded outputs) and
tests that run the built binary: every code's smallest spec gives that code, every example holds,
`map` and `affected` run on an agent-style change to each of the five examples in the four runtimes
and on both kinds of diff, pages load in Chrome, pixie's greeter is driven, `--jobs 4` prints the
same bytes as one at a time, no process a test started is left running, and the stories on this
page run as they are told. The outputs on this page and in the skill are lines of the golden files
those tests keep, this page's stories as whole transcripts, and `tests/docs.rs` holds the pages to
them: every claims file here parses, every output is what geas printed, and every command, option,
code and link exists.

On one run on macOS on Apple silicon, with every tool at hand, `cargo test` ran 238 tests (111
unit tests and 127 that run the binary) in 77 seconds, none skipped; the release binary is 2.3 MB.
A test whose tool is missing (python3, node, go, rustc with the llvm-tools component, Chrome, git,
or a built pixie greeter named by `GEAS_PIXIE_GREETER`) prints `SKIP:` and passes, and this lists
what was not run:

```console
$ cargo test -- --nocapture 2>&1 | grep '^SKIP'
```

`GEAS_BLESS=1 cargo test` writes the golden files again.

## Status

Early. Not yet: Windows, whose processes have no SIGTERM to stop a service gracefully and write its
coverage; macOS's accessibility API, which needs a person to grant a permission (a driver can do it
where that permission can be given); recording what a pixie app or a page's scripts run; choosing an
option of a native `<select>`; and a library claimed through an FFI harness rather than through a
small command or service around it. Claims that depend on one another are left out on purpose.
geas is a working name, and the name is still to be chosen. The design and every decision are in
[DESIGN.md](DESIGN.md); the order the work was done in is [PLAN.md](PLAN.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
