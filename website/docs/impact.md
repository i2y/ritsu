# Where a change goes

`ritsu impact --base <revision>` compares two revisions of a project and says, in one report, whether
any answer moved and who the change concerns. With `--route <file.rule>`, a rule of your own says where
the change goes: straight to merge, to the owner to read, or nowhere until it is fixed. Who wrote the
change makes no difference to it.

```
ritsu impact --base <revision> [--head <revision>] [--route <file.rule>]
             [--format text|markdown|json] [--budget <n>] [--root <dir>] [--lang ja|en]
```

The head is the working tree unless `--head` names a revision. The root is found as `ritsu check` finds
it, and the project needs no `ritsu.mod`. `ritsu impact` runs no claim and writes nothing: one side of
the change need not be on the disk, and a report should not start a service or change a journal.

## What it says

Every part of the report is what a language already answers. `ritsu impact` gathers them; it does not
say anything of its own about what a file means.

- **The files**: each changed file, added, removed, renamed or changed, as a file of a language (a
  spec), a generated file (its first lines say a generator wrote it), code written by hand (by its
  extension), prose (`.md`, `.markdown`, `.rst`, `.adoc`, outside a hidden directory, with no name in
  capitals but README, CHANGELOG, CHANGES and CONTRIBUTING) or something else. The lines of code written
  by hand are counted.
- **The check**: `ritsu check` of both sides, with `.geas` files read and their claims not run, and the
  errors and warnings only the head has. A diagnostic whose line moved is not new. A `contract` a `.geas`
  file names is read too, so a call its document does not have counts as a new error.
- **What moved**: what `ritsu mod diff` says of the two sides, so every input whose answer differs in a
  rule, every date that differs in a dates file, and every combination a gate decides differently. That
  includes what a file reads: a gate reading a calendar moves when the calendar does, and a gate with a
  date condition shows nothing when an unrelated file changes.
- **The owners**: `yuen affected` of the change: the requirements it reaches, their owners, and the
  changes no requirement reaches. A project with no `.req` is told as one.
- **The claims**: `geas affected` of the change: the claims it touches, and the changed lines no claim
  runs. Without a record that fits the code (`.geas/<name>.map.jsonl`), the report says it does not know,
  and every changed line of code counts as one no claim runs.
- **The generated files**: `ritsu gen --check` when `ritsu.mod` has a `target` line and the head is the
  working tree. Otherwise the report says they were not checked.
- **What it cannot compare**: `.flow`, `.kn`, `.ctx`, `.geas` and `.proto` files have no difference of
  behavior to show, so they are counted, not read. Anything else that could not be compared or decided
  is listed with why.

The first sentence of the report is the conclusion. In the example in
[`crates/ritsu/examples/impact`](https://github.com/i2y/ritsu/tree/main/crates/ritsu/examples/impact),
moving a band of `refund_band.rule` from 20 items to 30 (and recording the review with `yuen review`)
gives:

```
ritsu impact: base → the working tree

The answers moved (rule_changes 1, allow_gained 1, policies_moved 2); route: route = owner_reads, person_reads = true; owners: finance.

files: 6 changed (2 spec, 0 generated, 0 code, 0 doc, 4 other); 0 changed lines of code written by hand
  changed: refund_band.rule (spec, rulec)
  changed: refunds.req (spec, yuen)
```

and, further down, the region of the rule and the combination of the gate that reads it:

```
      items >=21 <=30
        band: large → small
        rows: table band row 2 → table band row 1
        example: items=21
```

```
      allow gained (deny → allow): 1 combination
        deciding policies: none → clerks_refund_small_amounts
        - User holding clerk, Order, size: small, in_period: yes (in the old version: size: large)
```

## The rule that says where it goes

The inputs of the rule are named after the facts below, by their names or their aliases, and it passes
the same checks as any other rule. Its outputs are all given. This is the example's:

```rule
define moved : number = rule_changes + date_changes + allow_gained + allow_lost + policies_moved + book_changes + interface_changes + interface_breaks + requirements_moved
define unread : number = undecided + generated_unchecked + opaque_specs + unclaimed_lines + other_files + new_warnings

table route
policy unique
| generated_stale | new_errors | moved | unread | -> route : destination |
| true            | -          | -     | -      | stop                   |
| false           | >=1        | -     | -      | stop                   |
| false           | 0          | >=1   | -      | owner_reads            |
| false           | 0          | 0     | >=1    | owner_reads            |
| false           | 0          | 0     | 0      | merge                  |

table answers
policy unique
| moved | -> answers_moved : bool |
| >=1   | true                    |
| 0     | false                   |

table who_reads
policy unique
| answers_moved | route             | -> person_reads : bool |
| true          | owner_reads, stop | true                   |
| false         | owner_reads, stop | true                   |
| false         | merge             | false                  |
```

The last table holds a promise: it has no row for a change that moved an answer and goes to merge, so
a `route` table that sends one there stops `ritsu check` with E101. Because the check covers every input,
that holds for every change, not for the examples alone.

What this rule lets through to merge is what the report can vouch for: prose, a spec whose answers are
shown to be the same, and code that reaches no requirement and whose every added line a claim runs.
Whatever the report knows nothing of goes to a person: a contract a spec reads, a build file, a workflow
(`other_files`), a generated file nothing checked (`generated_unchecked`). So does code a claim runs when
a requirement rests on that claim (`requirements_moved`), because a claim running a line does not say the
claim would notice the line going wrong.

A rule that cannot take the facts stops the run with exit 2, before anything is compared: an input named
after no fact (every name is listed), an input of another type than its fact. A value outside the range
of an input is said, with its name, its value and the range, and is not rounded: the report still comes
out, with no route, and the exit is 2.

## The facts

Counts are `number` inputs, 0 or more, and the rest are `bool`.

| Fact | What it is |
|---|---|
| `spec_files`, `generated_files`, `code_files`, `doc_files`, `other_files` | the changed files of each kind; a rename counts once |
| `code_lines` | the changed lines of code written by hand, blank lines left out; one line edited is one, and a file whose diff has no line (a rename) is one |
| `new_errors`, `new_warnings` | the diagnostics only the head has |
| `rule_changes` | the regions of input where a rule answers differently, and the rules added or removed |
| `date_changes` | the dates and calendar days that differ, and the dates files added or removed |
| `allow_gained`, `allow_lost` | the combinations a gate now allows or denies that it did not |
| `policies_moved` | the combinations with the same answer from other policies, what only one version of a gate has, and the gates added or removed |
| `book_changes` | the changes to what a `.book` offers, and the books added or removed |
| `interface_changes`, `interface_breaks` | the changes to what files offer the code that uses them, and those that can break it. A `fed by` line added, dropped or moved counts as a change here, because dropping one loosens what the check holds |
| `undecided` | what could not be compared or decided |
| `opaque_specs` | the changed `.flow`, `.kn`, `.ctx`, `.geas` and `.proto` files, whose contents are not compared |
| `requirements_moved`, `requirements_unknown` | the requirements that moved, and the changed specs and code no requirement reaches |
| `claims_touched`, `unclaimed_lines` | the claims the change touches, and the changed lines of code no claim is known to run: those it does not run, an added line the record holds no statement on (a table, a constant; a comment on its own line is left out), and a file whose diff has no line |
| `claims_known` | whether every `.geas` has a record that fits the change |
| `generated_stale`, `generated_checked` | whether a generated file is stale, and whether that was checked at all |
| `generated_unchecked` | the changed generated files `ritsu gen --check` did not hold, its head being all that says a generator wrote them |

## The exit, and the JSON

- 1 when the head has an error the base does not.
- 2 when that is not so and something could not be compared or decided, or the rule could not take the
  facts. A wrong argument and a rule that does not pass its check are also 2.
- 0 otherwise. Where the rule sends the change does not change the exit.

The JSON comes to its end in every case but a wrong argument or a rule that cannot be read:

```
{ritsu, base, head, files, check, behavior, interface, requirements, claims, generated, undecided,
 facts: {<name>: <value>}, route: {file, outputs} | null, markdown}
```

`route.outputs` holds each output by its public name, and an enum's value by its public name too, so a
rule with names in Japanese gives a pipeline the same words. `markdown` is the report for a comment, at
most 60,000 characters, with every `@` in code.

## In a pull request

One run gives both the comment and the route, which `jq` reads:

```yaml
# .github/workflows/ritsu-impact.yml
name: ritsu impact
on:
  pull_request:
permissions:
  contents: read
  pull-requests: write
jobs:
  impact:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: i2y/ritsu@v0.46.0
      - name: Say what the change moved and where it goes
        env:
          GH_TOKEN: ${{ github.token }}
          PR: ${{ github.event.pull_request.number }}
        run: |
          base="origin/$GITHUB_BASE_REF"
          git show "$base:route.rule" > "$RUNNER_TEMP/route.rule"
          code=0
          ritsu impact --base "$base" --route "$RUNNER_TEMP/route.rule" --format json > impact.json || code=$?
          jq -r .markdown impact.json > impact.md
          gh pr comment "$PR" --body-file impact.md
          route=$(jq -r '.route.outputs.route // empty' impact.json)
          if [ -z "$route" ]; then exit "$code"; fi
          if [ "$route" = owner_reads ]; then gh pr edit "$PR" --add-label "owner reads"; fi
          if [ "$route" = stop ]; then exit 1; fi
```

- The rule is read from the base, so a pull request cannot change where it sends itself; a change to
  `route.rule` is a change like any other, and shows as `rule_changes` when `route.rule` is in the project.
  Merge the rule before adding the workflow.
- `fetch-depth: 0` fetches the history, so that `origin/<base>` can be read.
- With a `ritsu.mod`, `ritsu impact` fetches the modules both locks pin, as `ritsu mod report` does. In a
  repository that holds several modules, give the directory of each to `--root`: a change inside a module
  of its own is listed as not compared when the root is above it.
- The label has to exist in the repository. A pull request from a fork gets a token that cannot write.
- For a record of the claims to count, the job has to run `geas` first and leave `.geas/<name>.map.jsonl`
  of the code it is given.
- `merge` says no person has to read the change, and `ritsu impact` runs no claim: run the tests and the
  claims in the job as well, and merge on both.
- The workflow that runs is the one in the pull request, so a change to it can change this step. It counts
  in `other_files`; have the repository's settings ask for a person's review of `.github/` as well.
- `--lang ja` writes the report in Japanese.
