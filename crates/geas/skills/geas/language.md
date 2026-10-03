# The geas language

A claims file (`*.geas`) holds targets, claims, masks and pins, in any order. A target says how to
start a program; a claim says what the program does, as anyone could observe it from outside; a
mask names a field drift does not compare; a pin fixes part of what the programs see. `#` starts a
comment that runs to the end of the line. The words of the language are English; a claim's name is
any text, in any language.

## Targets

```geas
target calc {
  run "python3 calc.py"
}

target api {
  serve "python3 server.py {port}"
  port auto
}

target legacy {
  serve "./server --port 8123"
  port 8123
  serial
}

claim "adds two integers" {
  when calc.run("2", "+", "3")
  then stdout is "5"
}

claim "answers on its own port" {
  when api.get("/health")
  then status is 200
  when legacy.get("/health")
  then status is 200
}
```

| Line | What it means |
|---|---|
| `run "<command>"` | a command: geas starts it once for each `when`, with the call's arguments after its words |
| `serve "<command>"` and `port <n>` | a service: geas starts it once per claim that touches it, waits up to 5 s for the port to open, and stops it when the claim ends |
| `port auto` | geas picks a free port on 127.0.0.1 for each instance and writes it where the command, or an `env` value, says `{port}` |
| `serial` | claims that touch this target never run at the same time, for state geas cannot see |
| `pixie "<app>"` | a pixie app, driven headless ([gui.md](gui.md)) |
| `driver "<command>"` | any other GUI, through a program that speaks geas's driver protocol ([gui.md](gui.md)) |

A target has one of `run`, `serve`, `pixie` and `driver`. Commands run in the spec's directory. A
target may hold pins (below), which then apply to it alone.

Each claim gets a fresh instance of every service it touches, started when the claim first needs
it and stopped at the claim's end, with its process group. So a claim builds its own state with its
own `when`s, and claims can run in any order, one at a time or side by side. A claim cannot rely on
another: that would hide from the reader the setup that makes it true.

## Commands

geas splits a command into words itself; no shell sees it.

- Blanks separate words.
- `'…'` keeps everything inside as it is, blanks included: `run "python3 'my calc.py'"`.
- `"…"` keeps blanks and reads `\"` and `\\`. In a `.geas` string a double quote is itself written
  `\"`, so single quotes are the easy form.
- Outside quotes, a backslash keeps the next character as it is.
- Nothing else is special: no variables, `~`, globs, pipes or redirections. When a shell is wanted,
  name it: `run "sh -c 'cd tools && ./gen'"`.
- `{port}` is replaced after splitting, on a target with a port; elsewhere it is E010.

An open quote, a trailing backslash or a command with no word is E010, pointing at the character.

## Claims

```geas
target api {
  serve "python3 server.py {port}"
  port auto
}

claim "totals accumulate across requests" {
  when api.post("/reset")
  then status is 200
  when api.post("/add", body: "5")
  when api.post("/add", body: "7")
  when api.get("/total")
  then body is "12"
  and  header "content-type" contains "text/plain"
}
```

A claim is a name in quotes and a block of steps. The first step is a `when`: an action on a target,
written `<target>.<call>(…)`. A `then` checks what the last `when` observed; `and` is the same word
for another check. A failed check marks the claim as failed, and the claim goes on, so every
divergence is reported. A `when` that cannot run (the program does not start, the port never opens,
the screen has no such button) ends the claim as an error, with a diagnostic. Names of targets and
of claims are unique in a file (E003).

## Calls and what they observe

| Call | On a target with | Observes |
|---|---|---|
| `run("a", "b", …)` | `run` | `stdout`, `stderr`, `exit` |
| `get("/path")`, `post("/path", body: "…")` | `serve` | `status`, `header "<name>"`, `body`, `body json "<path>"` |
| `open()`, `open("/path")` | `pixie`, `driver`, or `serve` (a page in Chrome) | `screen` |
| `click("<name>")`, `click("<name>", nth: n)` | the same | `screen` |
| `input("<text>")`, `input("<text>", field: n)`, `input("<text>", into: "<name>")` | the same | `screen` |
| `submit()`, `submit(field: n)`, `submit(into: "<name>")` | the same | `screen` |
| `press("<key>")` | the same | `screen` |
| `advance(<ms>)` | the same | `screen` |

A call a target does not take is E006, and a check of something its `when` does not observe is
E007, both before anything runs. `nth:` and `field:` count from 1. [gui.md](gui.md) says what each
GUI action does.

HTTP is HTTP/1.1 over 127.0.0.1, one connection a request; a chunked answer is read as the body it
carries. On a `port auto` service, `127.0.0.1:<port>`, `localhost:<port>` and `[::1]:<port>` in
what it answers are written `{port}` before anything sees them, so a claim writes `{port}` where
the port appears (in a redirect's `location`, say).

## Subjects and matchers

| Matcher | On | Holds when |
|---|---|---|
| `is <value>`, `is not <value>` | every subject but `screen` | the value is (or is not) the one written |
| `is above n`, `is below n`, `is at least n`, `is at most n` | `exit`, `status`, a JSON number, a text that is a number | the number compares so |
| `is between a and b` | the same | a ≤ the number ≤ b |
| `contains "…"`, `does not contain "…"` | a text | the text holds (or does not hold) that part |
| `matches "<pattern>"`, `does not match "<pattern>"` | a text, a JSON string | the whole value fits the pattern |
| `exists`, `does not exist` | `header "<name>"`, `body json "<path>"` | the value is there |
| `contains <node>`, `does not contain <node>`, `contains exactly n <node>` | `screen` | some node (no node, n nodes) fits |

A value is a string, a number, `true`, `false` or `null`. The texts are `stdout`, `stderr`, `body`,
a header, and a JSON string; the numbers are `exit`, `status` and a JSON number.

- `is` and `matches` drop one trailing newline from a text before they compare, so `stdout is "5"`
  holds for a program that prints `5` and a line break; `contains` looks at the whole text.
- A number matcher reads a text as a number when, trimmed, it is written as one: a sign, digits, a
  fraction, an exponent.
- `header "<name>"` ignores case in the name; several headers of one name read as one, joined by
  `, `.
- A value that is not there fails every matcher but `does not exist`: a header not sent, a JSON path
  that leads nowhere. A body that is not JSON fails every `body json` check.
- A matcher that does not fit its subject or its value is E008 before anything runs: `contains` on
  `exit`, `exit is "0"`, `is between 299 and 200`, `exists` on `stdout`.

## Patterns

`matches`, `does not match` and a node's `matching` take a pattern, held against the whole value:
literals; `.` (any character, a line break included); classes `[a-z]`, `[^0-9]`; `\d` (`0-9`), `\w`
(a letter of any script, a digit or `_`), `\s` (white space) and `\D`, `\W`, `\S`; groups `( )`;
`|`; `*`, `+`, `?`; `{n}`, `{n,}`, `{n,m}` (a count at most 1000). There are no anchors (`^` and `$`
are E009: the whole value is always matched), no backreferences and no lookaround. A pattern runs
in time linear in the text. In a claims file, a backslash in a pattern is written once:
`matches "\d+ items?"`. Any other string takes only the escapes `\n`, `\t`, `\"` and `\\` (E001).

## JSON paths

`body json "<path>"` reads into a JSON body: `.field`, `[index]`, chained (`.items[0].name`). A path
that does not parse is E009 before anything runs.

## The screen

A GUI action observes the screen: a tree of nodes, each with a role, a name, a value and states,
the way a screen reader is told it.

```text
<node>  := [exactly n] <role> [<label>] [with value "…"] [<state>] [in <role> [<label>]]
<label> := "the whole name" | containing "part" | matching "<pattern>"
<state> := disabled | enabled | checked | unchecked
```

- The role is a WAI-ARIA 1.2 role name (`button`, `link`, `textbox`, `searchbox`, `checkbox`,
  `radio`, `switch`, `tab`, `heading`, `list`, `listitem`, `dialog`, `alert`, `status`, `img`,
  `progressbar`, `combobox`, …) or `text`, for static text. A role outside these is E002.
- `in <role> [<label>]` asks for an ancestor: `text "Ada" in list "Greeted"`.
- `exactly n` goes with `contains` and counts the nodes that fit: `contains exactly 0 listitem`.
- A container that only lays things out reports nothing and hands its children up, so a pattern
  never has to name one.

```geas
target web {
  serve "python3 server.py {port}"
  port auto
}

claim "greets the name typed in" {
  when web.open("/")
  when web.input("Ada", into: "Your name")
  then screen contains button "Greet" enabled
  when web.click("Greet")
  then screen contains text "Good morning, Ada!" in status
  and  screen contains text "Ada" in list "Greeted"
  and  screen contains textbox "Your name" with value ""
  and  screen does not contain alert
  and  screen contains exactly 1 listitem
  and  screen contains text matching "Your fortune: .+"
}
```

## Masks

```geas
mask header "date"
mask body json ".request_id"
mask screen text containing "Last updated"

target api {
  serve "python3 server.py {port}"
  port auto
}

claim "answers" {
  when api.get("/")
  then status is 200
}
```

A mask names something drift does not compare: a header, a JSON path in a body, or the screen nodes
a pattern fits, with what is under them. A mask applies when drift compares, so adding one needs no
new snap; the journal and the baseline write a masked value as `"<masked>"`. Checks still see the
value itself. A mask says something about the program (this field changes by nature), so the
person who reads the claims decides on it.

## Pins

```geas
env clean
env pass "HOME"
env "PYTHONHASHSEED" "0"
tz "Asia/Tokyo"
locale "en-US"

target report {
  run "python3 report.py"
  clock "2026-08-29T09:00:00+09:00" env "REPORT_NOW"
  seed 7 env "REPORT_SEED"
}

target web {
  serve "python3 server.py {port}"
  port auto
  clock "2026-08-29T09:00:00+09:00"
  seed 7
}

claim "the report is dated by the clock it is given" {
  when report.run("daily")
  then stdout contains "2026-08-29"
}

claim "the page shows the pinned date" {
  when web.open("/")
  then screen contains text containing "2026"
}
```

| Pin | A process (`run`, `serve`, a pixie app) | A page in Chrome | A driver |
|---|---|---|---|
| `env "NAME" "value"` | sets NAME | its service's process gets it | sets NAME |
| `env clean` | the environment is `PATH` and what the spec sets or passes | (the service's process) | the same |
| `env pass "NAME"` | under `env clean`, NAME as geas has it | (the service's process) | the same |
| `tz "Asia/Tokyo"` | `TZ` | the page's time zone | in the pins line |
| `locale "ja-JP"` | `LANG` and `LC_ALL` as `ja_JP.UTF-8` | the page's locale | in the pins line |
| `clock "<RFC 3339>"` | only with `env "NAME"`: NAME is set to the time as written | the page's clock starts there | in the pins line |
| `seed n` | only with `env "NAME"`: NAME is set to n | `Math.random` is seeded with n | in the pins line |

A pin at the top level applies to every target; one inside a target replaces the same pin there.
Pinning one thing twice in one place, or setting one variable twice (`tz` and `env "TZ"`), is
E003. A pin geas cannot keep for a target is E011: `clock` or `seed` without `env` on a process, a
`locale` not written as a language and a region, a `clock` that is not an RFC 3339 time. A page
that pins no `locale`, `tz` or `clock` sees the machine's, so a spec whose screens should read the
same everywhere pins them. The journal records a target's pins when a claim first uses it, and
drift notes pins that differ from the baseline's.

## The grammar

```text
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
