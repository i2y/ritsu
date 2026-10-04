---
name: ritsu
description: Work on a project that holds files of more than one of ritsu's seven small languages, which are rules (`.rule`, rulec), workflows (`.flow`, dandori), dates (`.cal`, koyomi), ledgers (`.book`, chobo), claims about code (`.geas`, geas), requirements and their sources (`.req`, yuen) and maps of bounded contexts (`.ctx`, sakai). `ritsu check` checks each file with its own language and then across them, which shows that a workflow's call keeps a rule's preconditions, that the days a date comes to fit a rule's range, that a rule's output is an amount a ledger takes, and that a hold has not always expired by the time it is posted. Use when a project has files of two or more of these languages, when `ritsu check` prints a diagnostic of ritsu's own (E101, E201-E206, W201-W206), when a workflow has to be run with its rules, dates and books computed, or when one package of TypeScript, Python or Go has to be generated for the whole project. To write the files of one language, read that language's own skill.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`). That installs `ritsu` alone, so a language's own command is `ritsu <language> …`; a release archive also holds a link to it named for each language (`rulec`, `dandori`, …).
license: MIT OR Apache-2.0
---

## When this applies

The job is a **project of more than one language**: a workflow that calls rules, a rule over the
days of a calendar, a ledger whose amounts come from a rule, requirements that point at all of
them, a map of which part may depend on which. Each language checks its own files over every
input, every day and every path it can reach. ritsu adds the checks **between** them, which no
language can make alone, because what one has shown (a rule's preconditions, the days a date comes
to, the bounds of an account) is what the next is held to.

It also applies when the project holds the files of one language only and a diagnostic names
`ritsu`, or when a command of one language has to read another (`ritsu dandori`, `ritsu yuen`,
`ritsu sakai`, §6). It does not replace the language skills: to write a rule, a workflow, a date,
a ledger, a claim, a requirement or a map, read the skill of that language (§5).

A file never mixes two languages. A tariff, a calendar, a ledger and a workflow are read by
different people, so each is a file of its own, and the files meet by name: a workflow's `use rule`,
`use dates` and `use book`, a rule's `range from koyomi`, a requirement that names what satisfies it.

Everything is reachable from the command line: `ritsu --help` lists the commands,
`ritsu <command> --help` says what each takes, and `ritsu explain <CODE>` explains a diagnostic.
There is no step where you have to read ritsu's source.

---

# Working with ritsu

Your part is to keep the project passing `ritsu check`, to read what it says across the languages
and fix it, and to run and generate from it. Three things stay with people (§7): what the ranges
and bounds are, which warnings to leave, and what a fix would change in the meaning of a file.

## 1. The loop

1. **Check the project.** `ritsu check` in the project's directory reads every `.rule`, `.flow`,
   `.cal`, `.book`, `.geas`, `.req`, `.ctx` and `.proto` under it, or the paths you give
   (`ritsu check rules/ flows/order.flow`). A `.geas` file is checked by running its claims, which
   starts the program they are about.
2. **Read the last line first.** It says how many files each language had, how many failed, and how
   many checks between the languages were made and how many could not be decided (§2).
3. **Fix what a language says** with that language's skill. A diagnostic's heading names the tool,
   `error[rulec E101]`, because every language numbers its own codes: `ritsu rulec explain E101`
   explains that one (`rulec explain E101` is the same where the link `rulec` exists).
4. **Fix what ritsu says across the languages** (§3). `ritsu explain E201` explains one with the
   smallest files that print it.
5. **Check again**, until it ends `all pass`. A warning does not fail the check; it says what could
   not be decided, and why.
6. **Then run and generate**: `ritsu run` plays a workflow with its rules, dates and books worked
   out (§4); `ritsu gen [<path>...]` writes one package for the whole project (§4); each
   language's pages (`rulec doc`, `dandori doc`, `koyomi doc`, `chobo doc`, `yuen trace`) are for
   whoever has to understand what the code is to carry out and check it against what they know.

## 2. Reading what `ritsu check` prints

```text
ok refund_check.rule
refund.flow: ok
error[ritsu E201]: refund.flow:21:1: The call of the rule check can give it values that break its precondition `asked <= paid`
    21 |   let decision = check(paid: paid, asked: asked)
  = `asked` is `>=0 <=10000` and `paid` is `>=0 <=10000`, and at asked = 10000, paid = 0 `asked <= paid` does not hold
  = The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. The ranges are dandori's, gathered from every place the values come from.
  = Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result).
ritsu check: 2 files (rulec 1, dandori 1): 1 fail (1 error); borders between the languages: 1 checked, 0 undecided
```

- A file that passes prints the line its language prints (`ok refund_check.rule`,
  `refund.flow: ok`). A diagnostic of a language is what that language prints, with the tool added
  in the brackets.
- The lines that start with `=` are the notes: the numbers (here the ranges and the values that
  break the precondition), why it matters, and how to fix it.
- The last line counts the files of each language, the ones that fail, and the **borders**: the
  checks between the languages. Each comes to one of three answers: it holds, here is a case where
  it does not (an error), or it cannot be decided, and then it says why (a warning). Nothing
  passes in silence.
- **Exit codes**: 0 no errors (warnings possible), 1 an error in a language or across them, 2 bad
  arguments, a file that cannot be read, or one a language could not check.
- `--format json` prints one object: `ritsu` (the version), `root`, `ok`, `files`, `diagnostics`
  (each with `tool`, `code`, `severity`, `file`, `line`, `col`, `message`, `notes`) and `borders`
  (`held`, `failed`, `undecided`). Read that rather than the text, if you have to parse it.
- Paths are counted from the root: the nearest directory above the first path given that holds
  `.git`, or the path given. `--root <dir>` says it.
- `--lang ja` (or `RITSU_LANG=ja`) prints the text in Japanese.

## 3. The diagnostics across the languages

`ritsu explain --all` prints every one of these, each with a reproduction you can run. An error
comes with an example that breaks it; a warning says what could not be decided.

| Code | What it says | What to do |
|---|---|---|
| E101 | a `.proto` of the project does not read as proto3 | correct it where it points; a file `buf build` builds, ritsu reads |
| E201 | a workflow's call of a rule can give it values that break one of its preconditions (a `constraint`), or a koyomi date's days one of the rule's | branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or of a task's result); if the precondition is what is wrong, correct the rule's `constraint` |
| W201 | whether a call keeps a precondition cannot be decided: a value comes from a place with no range, or the precondition bounds a total or a length | give the place a range; where none can be given, leave it, and the workflow's code checks it when it runs |
| E202 | the days a koyomi date comes to fall outside the range of the rule input they are given to | widen the input's range, or make it `range from koyomi`, so that koyomi's days are the range |
| W202 | whether those days stay inside the range cannot be decided: the value can also come from an input, a task's answer or `now` | give the input days of koyomi dates only; or narrow the inputs of its file |
| E203 | a rule's output, given to a chobo transfer as its amount, can be below 0 or above 2⁶³ − 1 (chobo takes 0 to 2⁶³ − 1) | make the amounts 0 or more (a negative one, a refund say, is a transfer the other way), or branch before the transfer |
| W203 | whether chobo takes the output as an amount cannot be decided: its range has an open end, or the value also comes from a place with no range | give the places the value comes from a range |
| E204 | the transfer can be refused for a reason of the book's bounds (an account's `refused as`) that the task does not handle | declare the reason as an error of the task, and handle it |
| W204 | the task handles a reason that the book's search finds no run for within its depth, or what the amounts are cannot be decided | if the reason cannot happen, drop it from the task's errors; if it happens only after a longer run, leave it |
| E205 | the day given to a koyomi date can be outside the range of its input | widen the koyomi input's range (and the calendar's data), or give it a day that stays inside |
| W205 | whether the day stays inside the range cannot be decided: it comes from an input, a task's answer or `now` | give it the day of another koyomi date; otherwise leave it, koyomi refuses a day outside its range when the workflow runs |
| E206 | a hold has always expired by the time the flow posts or voids it | make the hold last longer (the transfer's `pending expires after`), or make the call sooner |
| W206 | whether the hold has expired by then cannot be decided: nothing bounds the time between making the hold and the call | give the task that makes the hold, and the tasks between, a `timeout`; where none can be given, leave it |

A few things to know before you act on one:

- An **error** is an example, so reproduce it first: the notes give the numbers (the range, the
  value, the day) that break the check, and the line is the call.
- A **warning** is not an error: the project passes. What could not be decided is left to the run,
  where the code of that language refuses or fails it (a rule's code refuses a call that breaks a
  precondition at its door, koyomi's a day outside its range, a book a call on an expired hold).
  Giving a place a range makes the warning decidable, but a range is a claim about the business
  (what a task can return, what days an input can be), so ask the person who owns it (§7) rather
  than narrowing it to silence the warning.
- The ranges in E201 and W201 are dandori's, gathered from every place a value comes from. Narrowing
  one place can turn a warning into a pass or into an error.

## 4. Running and generating

**`ritsu run <file.flow> --scenario <file.json>`** plays one scenario through dandori's reference
interpreter with the rules computed by rulec, the dates by koyomi and the operations of the books
by chobo. Every other task is answered by the scenario. Use it when you want to see what a workflow
does with its real rules, dates and ledgers, rather than with answers someone wrote down for them:

```text
invoice.flow (scenario scenarios/invoice_paid.json, from 2026-04-15T10:00:00Z)
before the run, book stock:
  receive.do(delivery: "D-1", sku: "pen", qty: 10)  done
the run:
  reserve(order: "A-1", sku: "pen", qty: 3)        chobo     done
  method(member: true, amount: 120)                rulec     {"billing":"invoice"}
  terms.payment(received: "2026-04-15T10:00:00Z")  koyomi    {"day":"2026-05-08","at":"2026-05-08T09:00:00Z"}
  wait until 2026-05-08T09:00:00Z                            22 days 23:00:00, to 2026-05-08T09:00:00Z
  check_payment(order: "A-1", due: "2026-05-08")   scenario  {"paid":true}
  ship(order: "A-1", sku: "pen")                   chobo     done
end: succeed {"outcome":"shipped","due":"2026-05-08"}
book stock (books/stock.book) at the end:
  accounts:
    shelf(pen)  posted 7, held out 0, held in 0
    suppliers   posted -10, held out 0, held in 0
    customers   posted 3, held out 0, held in 0
  holds:
    reserve(A-1, pen)  posted
```

The third column says who answered each call (`chobo`, `rulec`, `koyomi`, or `scenario`); a wait
takes the time it says, a hold expires as the run's time goes by, and the end of each book is
printed. The scenario is the one `dandori run` reads (an input, `now`, the answers of the other
tasks) with `books`, what was done to the books before the run. `dandori scenarios <file.flow>`
writes scenarios that take every arm. `--target` shows the calls as a platform would make them,
`--format json` prints one object. Exit codes: 0 the run came to its end (succeeded, failed or was
cancelled), 1 the flow has errors or the run could not go on (the scenario ran out of answers, say),
2 bad arguments or a scenario that cannot be run.

**`ritsu gen [<path>...] [--target <language>] [--out <dir>]`** writes one package for each of
TypeScript, Python and Go (`<out>/<language>/`, `generated` by default): the rules, the dates, the
clients of the books and the workflows of the project, where a workflow reads its rules, dates and
books from the package itself. Run it after `ritsu check` passes. `--check` writes nothing and exits
1 if a package is stale, for CI; `--books postgres|tigerbeetle` says what the clients of the books
call; `--name` and `--module` name the package. Each language's own `gen` or `build` still writes
that language's code alone (`rulec gen`, `koyomi gen`, `chobo build`, `dandori build`).

## 5. Which skill to read

Each language has a skill of its own, with the language on one page, what to ask a person, and the
fix for each of the language's own diagnostics. They are in this repository at the paths below;
copy the folders you need into `~/.claude/skills/`, or a project's `.claude/skills/`, beside this
one.

| Language | Files | Skill | Read it to |
|---|---|---|---|
| rulec | `.rule` | `skills/rulec/` | write a rule, fix rulec's diagnostics, call the generated function |
| dandori | `.flow` | `skills/dandori/` | write a workflow, write scenarios, build for Temporal, Step Functions, Argo and the others |
| koyomi | `.cal` | `skills/koyomi/` | write a date and a calendar, fix koyomi's diagnostics |
| chobo | `.book` | `skills/chobo/` | write a ledger, fix chobo's diagnostics, call the client |
| geas | `.geas` | `skills/geas/` | write claims about code an agent wrote |
| yuen | `.req` | `skills/yuen/` | write requirements and what they come from |
| sakai | `.ctx` | `skills/sakai/` | write a map of bounded contexts |

## 6. A language that reads another

`ritsu <language> …` is that language's own command, with the languages it reads joined in the same
process. `dandori`, `yuen` and `sakai` read others, so a command of theirs on a file that uses
another language's files is run as `ritsu dandori …`, `ritsu yuen …` or `ritsu sakai …`. Installed
alone, each reads no other language and says so: dandori's E018, yuen's E206 and sakai's E104 name
the command to run instead. `rulec` reads koyomi's days (`range from koyomi`), so
`ritsu rulec check` is what checks such a rule.

## 7. What stays with people

- **What the ranges and bounds are.** The range of an input or of a task's result, the bounds of an
  account, how long a hold lasts, which days a calendar closes: each is a claim about the business,
  and every check between the languages stands on it. Ask, do not guess one to make a check pass.
- **Which warnings to leave.** A warning that cannot be decided is true; whether it matters is a
  decision (§3).
- **What a fix would change.** E201's other fix, correcting a rule's `constraint`, changes what the
  rule promises everyone who calls it; E206's, a longer hold, changes how long stock or money is
  set aside.
- **Reading the pages.** The pages the languages draw are for whoever has to understand what the
  code is to carry out and check it against what they know: the people who run the business,
  finance and legal, operators and the developers reviewing the code. Hand them the page; what they
  find wrong is for you to fix.
