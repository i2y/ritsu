# The examples

The geas repository has seven examples under `examples/`, each a claims file beside a small program,
with a README that says what it shows and how to run it. The programs were written the way an agent
writes them; the claims are what a person reads.

| Example | The program | What it shows |
|---|---|---|
| `calc` | a command-line calculator in Python | `run` targets: `stdout`, `stderr` and `exit`; one process per `when` |
| `greeter` | an HTTP service in Python | `serve` and `port auto`, a claim that builds state over several requests, `body json`, masks; `server_refactored.py` is an agent's refactor that keeps every claim and changes seven things drift reports; `greeter.ja.geas` is the same claims, named in Japanese |
| `tally-node` | an HTTP service in TypeScript, run by Node | `geas map` on Node with nothing to build |
| `tally-go` | the same service in Go | `map` on a Go service built with `-cover`, stopping on SIGTERM |
| `tally-rust` | a command line in Rust | `map` on a program built with `-C instrument-coverage` |
| `web-greeter` | a page and its service in Python | a page in Chrome: typing, clicking, Enter, the screen's nodes, `into:`, states, `matching`, and claims on the same service's API; all four pins (`locale`, `tz`, `clock`, `seed`), since the page writes the date, greets by the time of day and draws a fortune at random |
| `pixie-greeter` | pixie's greeter, a desktop app | a pixie target driven headless: `open()`, `input`, `click`, `submit()`, `field: 2`; the app is built with pixie and put beside the claims file |

For each example but `pixie-greeter`, `tests/changes/<example>/` in the repository holds an
agent-style change: the changed files (`after/`), and for the five whose code `map` records, the
change as a git diff (`change.diff`) and as a plain one (`change.plain.diff`). The tests run `map`
before and after the change and `affected` on both diffs. The change to `web-greeter` turns its
Greet button into a `<div>` with a click handler, which the screen shows as text and the claims
refuse.

## Running them

From the repository's root, with `geas` on PATH:

```console
$ geas check examples/calc/calc.geas
$ geas check examples/greeter/greeter.geas
$ geas check examples/web-greeter/web-greeter.geas
```

`tally-go` and `tally-rust` are built first (their READMEs give the commands), and `pixie-greeter`
needs the app built with pixie. A claims file's commands run in its own directory, so each example
runs from anywhere.

## Patterns worth copying

- **A claim builds its own state.** `greeter`'s "totals accumulate across requests" resets, adds 5
  and 7, and reads 12, all in one claim; no claim depends on another having run.
- **Volatile fields are masked, by name.** `mask header "date"` in the greeter: drift on unchanged
  code is quiet, and every other header is still compared.
- **The environment is pinned when output depends on it.** The web greeter pins the locale, the
  time zone, the clock and the random seed, and its claims name the exact date and greeting.
- **Screens are claimed by role and name.** `screen contains button "Greet" disabled`, never a
  selector: a claim fails when a person with a screen reader could not do what it says.
