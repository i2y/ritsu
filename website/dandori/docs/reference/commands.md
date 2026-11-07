# Commands

```text
dandori check <file.flow>...
dandori gen <file.flow> --target asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph... [--out <dir>] [--check]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph]
dandori doc <file.flow> [--format html] [--out <dir>]
dandori api <file.flow>
dandori explain <CODE> | --all [--format markdown|json]
dandori <command> --help
dandori --version
```

A workflow that uses rules, dates files or books (`use rule`, `use dates`, `use book`) runs as
`ritsu dandori <command>`, which reads them in the same process ([Install](../install.md)); the
`dandori` command alone checks a workflow that uses none of them, and writes its code.

| Command | What it does |
|---|---|
| `check` | Checks one or more `.flow` files: types, every arm, every state a case can be left in, retries. Prints the diagnostics, each with the run that gets there, and `ok` for a file that passes. |
| `gen` | Writes the code for each platform its `--target` names (one or more), each into `<out>/<target>/` (`generated/` by default), in the order of the platforms above whatever the order given, and prints each file's path. Also refuses what a platform cannot do (E050), and a workflow whose one run can outgrow the platform (E040); when one platform refuses, nothing is written. With `--check` it writes nothing, prints each file that is missing or differs, and exits 1 if there is one. [Build for a platform](../platforms.md) |
| `scenarios` | Writes scenarios (an input and the answers the calls get) that together take every arm, every handler and every way a case can move, and for each place of the inputs held to a range, one whose input is just outside it there: one file each into `--out`, or all of them as JSON on standard output. |
| `run` | Plays one scenario through the reference interpreter, and prints every call as the target would make it, every wait, and how the run ends. |
| `doc` | Draws the workflow for the person who reviews it: the flow, what each call does and where its errors go, and every way the workflow can end. Markdown with Mermaid charts, or one HTML page (`--format html`) where each scenario lights up the way its run goes; into `<name>.md` or `<name>.html` in `--out`, else on standard output. The rules the workflow calls are put in as `rulec doc` renders them. It draws a workflow whose check finds errors too, and exits with 1. [Draw a workflow](../diagrams.md) |
| `api` | Prints what the workflow declares as one JSON object, without the source being read: [the format](#the-json-of-api). It needs a workflow that passes `check`; one that does not is given the diagnostics `check` gives, and exit 1. |
| `explain` | Looks a diagnostic code up: when it comes, how to fix it, and the smallest `.flow` that gets it, with what goes beside it. `--all` prints every code; `--format markdown` prints Markdown, `--format json` the machine-facing JSON. [Diagnostics](codes.md) |

## The JSON of `api`

```console
$ ritsu dandori api examples/hotel/temporal/hotel.flow | jq '.tasks[] | select(.name == "confirm_intent") | {kind, calls, errors, refused_as, retry, key}'
{
  "kind": "http",
  "calls": {
    "method": "POST",
    "url": "https://api.stripe.com/v1/payment_intents/{intent}/confirm",
    "form": true,
    "api": "../specs/stripe.json"
  },
  "errors": [
    {
      "name": "card_declined",
      "status": 402,
      "exception": null
    },
    {
      "name": "unexpected_state",
      "status": 400,
      "exception": null
    }
  ],
  "refused_as": "unexpected_state",
  "retry": null,
  "key": true
}
```

It is built from the checked program, as the generators read it, so it says the same whichever
platform the workflow is written for. The keys, in order:

| Key | What it holds |
|---|---|
| `dandori`, `file`, `workflow` | the version of dandori, the file, and the workflow's `name`, `version`, `description`, `sha256` and `kind` (`standard` or `express`) |
| `implements` | the service of a `.proto` the workflow implements: `api`, `file`, `name`, and the `methods`, each with its `request`, `response` and `marks` (`start` with the names it can `fails` with, `event` and `answer` with their `task`, `status`); else `null` |
| `period`, `history_encrypted` | the period the workflow runs in (`text`, `min`, `max`, `line`), and the line of `history encrypted`; else `null` |
| `uses` | each `use` line: `kind` (`rule`, `dates`, `book`, `openapi`, `smithy`, `proto`), `name`, `file` as written and `line`; for a rule also its name, version, `sha256`, `inputs` and `outputs` with their types and ranges, and whether it is called as a `lambda`, `local`, at a `connect` URL or through a `connection`; for a dates file its dates with their parameters; for a book its transfers |
| `inputs`, `outputs` | each with `name`, `type`, `optional`, `range`, `secret` and `line`. A `range` is `text` as the `.flow` writes it and `min` and `max` as data (a number, a day, a moment, or the value of an ordered enum, with all the `values` it lets in) |
| `enums`, `records` | each with its values or fields; a record's `origin` says whether it is the workflow's own, a rule's outputs, a hold, or a message of a `.proto` |
| `tasks` | each with `kind` (`http`, `connect`, `aws`, `lambda`, `agent`, `jev`, `book`, `workflow`, `state_machine`, `durable_function`, `argo_template`, `image`, `event`, or `own` for the implementation you write), what it `calls`, `params` and `result` (type, range, secret), `timeout`, `deadline`, `retry`, `key`, `idempotent`, `changes_things`, `callback`, `errors` with their status or exception, `refused_as`, `machine`, and `discloses` |
| `cases` | each with the `record` it holds, the machine it `follows`, its `states` and `final` states, what is `held`, the `external` events, and `refused_when` |
| `children` | each child `.flow` a task runs: the task, the `file`, the workflow, its `inputs` and `outputs` |
| `fails`, `succeeds` | every way the workflow ends: each `fail` with its `error`, `message`, the cases it `leaves`, the block it is `in` (`flow`, `on failure`, `on cancel`) and its `line`; each `succeed` with the `outputs` it sets, as written |
| `on_failure`, `on_cancel` | whether there is an `on failure` block, and the line of `on cancel` |

## Flags and environment

| | |
|---|---|
| `--target <target>` | a platform `gen` writes for (give it more than once), or the one whose calls `run` prints (`reference` by default). Up to 0.40.0 the TypeScript of Temporal was `temporal`, and `dandori gen` was `dandori build`; both stop now with exit 2, saying the name of now |
| `--out <dir>` | where `gen` writes (`generated/` by default, a directory for each platform), and where `scenarios` and `doc` write |
| `--check` | `gen` writes nothing, and exits 1 if a file the same `--target` write is missing or differs (for CI) |
| `--scenario <file.json>` | the scenario `run` plays |
| `--format json` | the diagnostics of `check` as JSON, for tools |
| `--format html` | the page `doc` writes, as HTML; Markdown by default |
| `--all`, `--format markdown` or `--format json` | every code, for `explain`; as Markdown, or as JSON |
| `--lang ja` or `--lang en` | the language of the messages; else `DANDORI_LANG`, then `RITSU_LANG`, else English |
| `--help` | what the command takes, its exit codes and examples (`dandori help <command>` is the same page); `dandori --help` lists the commands |
| `--version` | `dandori --version` prints the version |

## Exit codes

| Code | Means |
|---|---|
| 0 | no errors (warnings only, or nothing) |
| 1 | errors found; for `gen`, also a platform that cannot take the workflow (nothing is written), or under `--check` a file that is missing or differs |
| 2 | bad arguments (`gen` without `--target`, among them), or a file that cannot be read |
