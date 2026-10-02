---
name: geas
description: Hold code to claims a person has read, with geas. A `.geas` file holds claims about what a program does as anyone could observe it from outside (what a command prints and exits with, what an HTTP service answers, what a screen shows), and geas runs every claim against the real program, whatever language it is written in. Use when writing or changing code that a `.geas` claims file holds; when running the gate on a change (`geas check`, `geas drift`, `geas map` and `geas affected`); when a claim fails or a geas diagnostic (E001-E081, W060-W061) has to be fixed; or when behavior no claim covers has to be proposed to the person as new claims.
compatibility: Requires the `geas` binary on PATH (`cargo install --path .` in a clone of the geas repository), and whatever the project's own programs need to run. `geas map` records Python 3.12 and later, Node, Go built with `-cover`, and Rust built with `-C instrument-coverage` (with rustup's llvm-tools). A page in a browser needs Chrome or Chromium; a pixie target needs a built pixie app.
license: MIT OR Apache-2.0
---

## When this applies

The project keeps a **claims file** (`*.geas`) beside its code, or a person wants one. Each claim
says what the program does as anyone could see it from outside: what a command prints and exits
with, what an HTTP service answers, what a screen shows to a screen reader. The person reads and
accepts the claims; you write the code; `geas check` runs every claim against the real program and
says which hold.

It does not apply to tests of internal functions: a claim never names a file, a function, a class,
a CSS selector or a line, and geas never reaches into the code. A library is claimed through a
small command or service around it.

The files bundled with this skill are listed in §10. Read them when you need them, not all up
front.

---

# Working with geas

## 1. Who writes what

geas splits the work: the person reads about fifty lines of claims, you write the code, and geas
holds the code to the claims. That split is the whole point, so keep to it.

- **The claims file is the person's.** Every line of it: the claims, the targets, the masks, the
  pins. Do not change it to make the gate pass. Write a change you think it needs as text in your
  answer, and put it in the file once the person says yes. When there is no claims file yet,
  propose a first one the same way, from what the person asked for.
- **A failing claim is fixed in the code**, unless the person says the claim is wrong. A claim
  that cannot be true of any sensible program is a question for the person, not something to
  loosen.
- **A missing claim is proposed, not written.** When `geas affected` lists changed code no claim
  runs, when `geas drift` lists a change no claim covers, or when the person asks for behavior no
  claim states, write the claim you propose, say why, and let the person accept it.
- **A new baseline and a new mask are asked for.** `geas snap` changes what `drift` compares
  against, and a `mask` line stops drift from looking at a field; both can hide a change, so the
  person decides.
- **The files under `.geas/` are geas's.** The journal, the baseline and the record are written
  by geas; never edit them by hand.

Everything is reachable from the command line: `geas --help` lists the commands, `geas explain
<code>` explains a diagnostic, and the commands that report take `--json`. There is no step where
you have to read geas's source.

## 2. The loop

1. **Check:** `geas check <spec.geas>` runs every claim. Read each `not ok` (§4), fix the code,
   and check again until every claim holds (exit 0).
2. **Snap, once the person accepts the behavior:** `geas snap <spec.geas>` runs the claims and
   keeps every observation, not only what the checks look at, as the baseline in
   `.geas/<stem>.baseline.jsonl`. Ask before you snap.
3. **On every later change:**
   - `geas check <spec.geas>`: every claim still holds.
   - `geas drift <spec.geas>`: what changed since the baseline. A change marked `[unclaimed]`
     is behavior no claim promises; tell the person, with a claim to add or a reason not to.
   - `geas map <spec.geas>`, then `git diff | geas affected <spec.geas> -`: which claims the
     change touches, and which changed lines no claim runs. `map` runs the claims with coverage
     on and records, for each claim, the lines it ran; `affected` reads the diff and that record
     and runs nothing. Run `map` on the changed code: added lines need a record of the code after
     the change (E063 otherwise). A file the change adds is in `git diff` only once git knows it,
     so `git add -N <file>` it first.
4. **Report** what the person has to read: the claims the change touches, the changed code no
   claim runs, and drift's unclaimed changes, each with the claim you propose or the reason it
   needs none.

`check`, `snap`, `drift` and `map` take several specs; `--jobs N` runs up to N claims at once
(§8). Exit codes: 0 all held or nothing to report, 1 something failed or changed, 2 the spec, a
file or the arguments are wrong.

## 3. The language on one page

A whole claims file, `examples/greeter/greeter.geas`:

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

A **target** says how to start the program. A **claim** is `when` steps, each an action on a
target, and `then` checks of what the last `when` observed; `and` is another check of the same
observation. Each claim starts its own instance of every service it touches, so claims do not see
each other's state and can run in any order. `#` starts a comment. The forms, at a glance:

```text
target calc { run "python3 calc.py" }           a command, run once per `when`
target api  { serve "node server.ts {port}"     a service, started once per claim,
              port auto }                       on a free port geas picks; or `port 8123`
target app  { pixie "greeter" }                 a pixie app, driven headless
target ui   { driver "python3 my_driver.py" }   any other GUI, through a driver program
serial                                          (in a target) its claims never run side by side

when calc.run("2", "+", "3")                    arguments after the target's command
when api.get("/path")   when api.post("/path", body: "5")
when web.open("/")      when app.open()         a page of a serve target, or the app's first screen
when app.click("Save")  when app.click("Save", nth: 2)
when app.input("Ada")   when app.input("Ada", field: 2)   when web.input("Ada", into: "Your name")
when app.submit()       when app.press("cmd-s")           when app.advance(500)

then stdout | stderr | exit                     after run
then status | header "etag" | body | body json ".a[0].b"   after get or post
then screen                                     after a GUI action

  is "5" | is 0 | is true | is null | is not "x"
  is above 1 | is below 10 | is at least 0 | is at most 3 | is between 200 and 299
  contains "part" | does not contain "part"
  matches "\d+ items?" | does not match "error.*"
  exists | does not exist                       for header and body json

  contains button "Save" disabled               on screen: a node by role, name, value, state
  contains exactly 2 listitem in list "Items"
  contains text containing "Hello" | contains text matching "Hello, .+!"
  contains textbox "Name" with value "Ada" | does not contain dialog

mask header "date"                              drift does not compare it
mask body json ".request_id"
mask screen text containing "Updated"

env "NAME" "value" | env clean | env pass "HOME" | tz "Asia/Tokyo" | locale "ja-JP"
clock "2026-08-29T09:00:00+09:00" [env "NOW"] | seed 7 [env "SEED"]
```

A role is a WAI-ARIA role name (`button`, `textbox`, `heading`, `link`, `checkbox`, `listitem`,
`dialog`, …) or `text`, and the states are `disabled`, `enabled`, `checked` and `unchecked`. A
pattern (`matches`, `matching`) is held against the whole value. In a string, `\n`, `\t`, `\"`
and `\\` are escapes, and a pattern keeps its backslashes as written (`"\d+"`). geas splits a
command into words itself, never through a shell: `'…'` and `"…"` keep blanks inside a word, and a
shell is named when one is wanted (`run "sh -c 'cd tools && ./gen'"`). [language.md](language.md)
has every form, with what each one means.

## 4. Reading the answers

`geas check` prints one line a claim, `ok` or `not ok`, then a summary. A failed check says what it
expected and what came back, with the line of the spec and the run that gets there:

```text
not ok 2 - rejects an empty name
    examples/greeter/greeter.geas:22: status is 400 — got 200
        22 |   then status is 400
      the run that gets there:
          21  when api.get("/greet?name=")  →  200, body "{\"message\": \"Hello, \"}"
```

A `not ok … (error)` is a claim whose `when` could not run: a program that would not start (E030),
a service that never opened its port (E032), a control the screen does not have (E035). It comes
as a diagnostic with its code, and the notes say what geas saw. A failed `screen` check shows the
screen it looked at, one node a line. With `--json`, the report is one object a spec:
`{"geas":1,"ok":…,"file":…,"claims":[{"name","line","status","error","checks","run"}]}`.

`geas drift` prints each change under the `when` it comes from: `+` appeared, `-` disappeared, `~`
changed, then the tag. `[claimed — `geas check` is the authority]` means some check looks at that
field; `[unclaimed]` means no claim promises anything about it:

```text
claim "greets by name" when#1 api.get("/greet?name=Alice")
  + body json ".debug": appeared: {"handler":"greet_v2"}   [unclaimed]
```

`geas affected` lists the claims the change touches, each with the changed lines it runs. Lines
that every claim starting a target runs (imports, route tables) are listed once, under `every claim
that starts` the target. Then come the changed code no claim runs, deleted files, files outside the
record, and changes to the spec or its baseline. `removed 10-11 (next to lines they run)` is a
removal placed by the code around it. Exit 1 when some changed code is run by no claim, or the spec
or the baseline changed. [map.md](map.md) explains every part, and [commands.md](commands.md) every
`--json` shape.

## 5. From a diagnostic to a fix

A diagnostic has a code, a place, notes (`= …`) and the run that gets there.

| Code | What it finds | The usual fix |
|---|---|---|
| E001, E002 | a token the grammar does not take there; a word geas does not know | the message lists what fits |
| E003 | two targets or two claims with one name; one thing pinned twice | rename one; keep one pin |
| E004, E005 | a target whose lines do not fit; a claim with no step, or a `then` before any `when` | what the message says |
| E006, E007 | a call the target does not take; a check of something the `when` does not observe | the forms in §3 |
| E008, E009 | a matcher that does not fit its subject or value; a pattern or JSON path that does not parse | the message points at the character |
| E010 | a command that cannot be split into words; `{port}` on a target without a port | close the quote |
| E011 | a pin geas cannot keep for that target | give `clock` and `seed` an `env "NAME"` on a process |
| E012, E013 | another target's `when` between two pixie actions; an argument an action cannot take there | move the `when`; see [gui.md](gui.md) |
| E030 to E033 | a program that would not start, a `run` over 5 s, a service that never opened its port, an HTTP exchange that failed | fix the program or the target's command; the notes hold its stderr |
| E034 to E037 | no Chrome; an action the app refused; a page that did not load or settle; a GUI driver geas cannot read | the notes show the screen and what it could reach |
| E050, E051 | no baseline, or one that cannot be read | ask the person, then `geas snap` |
| E060 to E064 | no record, a record of another spec, a stale record, added lines with only a record of the code before, a diff that fits neither side | `geas map` on the code the diff ends at |
| E065, E066 | a coverage tool missing or failing; a service that stopped without writing its record | [map.md](map.md) |
| E080, E081 | arguments the command does not take; a file that cannot be read or written | `geas --help` |
| W060, W061 | a target that gave `map` no record; a Rust profile from a program geas did not start | build it with coverage; start the binary itself |

`geas explain <code>` prints when a code appears, what usually fixes it, and the smallest spec that
gives it; [codes.md](codes.md) has every one. `--lang ja` (or `GEAS_LANG=ja`) gives the messages in
Japanese, for a person who reads them in Japanese.

## 6. What `geas map` needs from each language

`map` sets every switch below on every process it starts, and each runtime picks up its own. A
target does not say what it is written in.

| Language | The target has to | geas reads |
|---|---|---|
| Python 3.12 and later | nothing | its own `sitecustomize.py`, put on `PYTHONPATH` |
| Node (JavaScript, and TypeScript by type stripping) | nothing | `NODE_V8_COVERAGE`, with a hook on `NODE_OPTIONS` |
| Go | be built with `go build -cover -coverpkg=./...`; as a service, return from `main` on SIGTERM | `GOCOVERDIR`, `go tool covdata` |
| Rust | be built with `-C instrument-coverage`, and be the program the command starts; as a service, exit normally on SIGTERM | `LLVM_PROFILE_FILE`, rustup's llvm-tools |

A service is stopped with SIGTERM and given 5 s; one that has to be killed is E066. A program a
switch does not reach (a shell script, Python before 3.12, a binary built without coverage) adds
nothing, and `map` warns (W060). Build with coverage for `map` only: under `check`, such a build
prints a warning (Go) or leaves profiles in its directory (Rust). The record is the exact code it
was measured on, by git blob hash, and `affected` refuses one that does not fit the diff (E062), so
run `map` again after changing the code. [map.md](map.md) has the details and CI recipes.

## 7. GUI targets

A GUI claim observes the screen as a screen reader is told it: nodes with a role, a name, a value
and states. Claims never name a widget class, a selector or a pixel, so a control a claim cannot
reach by its role and name is one a person with a screen reader cannot reach either. If a claim
says `click("Greet")` and the page made Greet a `<div>`, the page is what needs fixing.

- **A page in Chrome:** a `serve` target, then `open("/path")`. geas starts a headless Chrome (from
  `GEAS_CHROME`, the macOS application, or `google-chrome`, `chromium` or `chromium-browser` on
  PATH), gives each claim a fresh browser context, and settles the page on virtual time: a timer of
  300 ms has fired by the next check, without the claim waiting for it. `get` and `post` reach the
  same service in the same claim.
- **A pixie app:** `pixie "<app>"`, the built app beside the spec. geas replays each claim's actions
  on it as one headless script, so no other target's `when` may come between two of them (E012).
  It reaches text fields by position (`field: 2`), not by name.
- **Anything else:** `driver "<command>"`, a program of yours that speaks geas's JSON-lines protocol
  and drives the app (an accessibility API, an emulator).

[gui.md](gui.md) has the actions, how each driver behaves, and the protocol.

## 8. Pins and running side by side

A program whose output depends on its environment gets the same environment on every run through
pins, written in the claims file, at the top level for every target or inside one:

- `env "NAME" "value"`, `env clean` (only `PATH`, and what the spec sets or passes), `env pass
  "NAME"`, `tz "…"` (sets `TZ`), `locale "ja-JP"` (sets `LANG` and `LC_ALL`).
- `clock` and `seed`: a page in Chrome takes them as they are, since geas runs the page's clock and
  its `Math.random`. A process cannot be given a clock from outside, so it gets them only as an
  environment variable it reads: `clock "2026-08-29T09:00:00+09:00" env "NOW"`. Without `env`,
  they are E011 on a process.
- For a Python target, propose `env "PYTHONHASHSEED" "0"`: hash randomization changes the order of
  sets in output. geas does not set it on its own, since a pin the spec does not show is one its
  reader cannot check.

`--jobs N` (`-j N`, `GEAS_JOBS`) runs up to N claims at once; the report, the journal and the
baseline come out byte for byte as with one. `port auto` gives each instance a free port, written
`{port}` in the command and in `env` values; claims that share a fixed `port 8123` wait for each
other. A target whose claims share state geas cannot see (a file, a database) needs `serial`, or
`--jobs 1`; ask the person before running such a spec side by side.

## 9. What to ask the person

- **Whether a claim is wrong.** Quote it, with what came back and the run that gets there. Do not
  change the claim, and do not make the code answer differently only for the case it checks.
- **New claims.** For each line `affected` calls unclaimed and each unclaimed change from `drift`:
  the claim you propose, or why none is needed (logging, a refactor that changes nothing a person
  sees).
- **A new baseline**, after a change the person has accepted: `geas snap` replaces what drift
  compares against.
- **A mask**, for a field that changes by nature (a date, a request id), with the drift that shows
  it.
- **Pins**, when output depends on the time zone, the locale, the clock or chance.
- **`serial`**, before running a spec side by side when its targets share state.

Ask with what geas printed: "claim 2 expects status 400 for an empty name and got 200; should the
service refuse it, or is the claim wrong?"

## 10. The files bundled with this skill

| File | What is in it |
|---|---|
| [language.md](language.md) | every form of the language, what each means, and a claims file for each kind of target |
| [commands.md](commands.md) | the commands, their options, exit codes, environment variables, and the files and JSON geas writes |
| [codes.md](codes.md) | every diagnostic: when it appears, how to fix it, the smallest spec that gives it |
| [gui.md](gui.md) | the screen, the actions, pixie, Chrome, and the driver protocol |
| [map.md](map.md) | recording what each claim runs, `geas affected` in full, each language's needs, CI recipes |
| [examples.md](examples.md) | the examples in the geas repository, and what each shows |

`geas skill` prints this file, and `geas skill --install <dir>` writes the whole folder as
`<dir>/geas/`.
