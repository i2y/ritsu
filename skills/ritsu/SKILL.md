---
name: ritsu
description: Work on a project with files of two or more of ritsu's nine small languages, rules (`.rule`, rulec), workflows (`.flow`, dandori), dates (`.cal`, koyomi), ledgers (`.book`, chobo), claims about code (`.geas`, geas), requirements (`.req`, yuen), maps of bounded contexts (`.ctx`, sakai), who may do what (`.gate`, sekisho) and functional cores (`.kn`, kaname). `ritsu check` checks each file with its own language and then across them, which shows that a call keeps a rule's preconditions, that a date's days fit a rule's range, that a ledger takes the amounts it is given, and that a hold has not always expired when it is posted. Use when a project has files of two or more of these languages, when `ritsu check` prints a diagnostic of ritsu's own (E101, E201-W206, E301-W303, W901, E905-W909), when a workflow has to be run with its rules, dates and books computed, or when one package of TypeScript, Python or Go is to be generated for the whole project. To write one language's files, read that language's skill.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`), which runs each language as `ritsu <language> …`; a link to ritsu named for a language (`rulec`, `dandori`, …), as a release archive holds, runs the same. `ritsu skills install` writes the ten skills into a project's `.claude/skills/`.
license: MIT OR Apache-2.0
---

## When this applies

The job is a **project of more than one language**: a workflow that calls rules, a rule over the
days of a calendar, a ledger whose amounts come from a rule, requirements that point at all of
them, a map of which part may depend on which, a gate that says who may call each operation, a
functional core that calls rules, dates and ledgers. Each language checks its own files over every
input, every day and every path it can reach. ritsu adds the checks **between** them, which no
language can make alone, because what one has shown (a rule's preconditions, the days a date comes
to, the bounds of an account) is what the next is held to.

It also applies when the project holds the files of one language only and a diagnostic names
`ritsu`, when a command of one language has to read another (`ritsu dandori`, `ritsu yuen`,
`ritsu sakai`, `ritsu sekisho`, `ritsu kaname`, §6), and when the project is a module (a
`ritsu.mod`) that requires the files of other modules or is to be published (§9). It does not replace the language skills: to
write a rule, a workflow, a date, a ledger, a claim, a requirement, a map, a gate or a functional
core, read the skill of that language (§5).

A file never mixes two languages. A tariff, a calendar, a ledger and a workflow are read by
different people, so each is a file of its own, and the files meet by name: a workflow's `use rule`,
`use dates` and `use book`, a rule's `range from koyomi`, a requirement that names what satisfies it,
a gate's `use rule` and `guards`, a module's `use rule`, `use dates` and `use book`.

Everything is reachable from the command line: `ritsu --help` lists the commands,
`ritsu <command> --help` says what each takes, and `ritsu explain <CODE>` explains a diagnostic.
There is no step where you have to read ritsu's source. In a JavaScript or TypeScript project
without the binary, the npm package `@i2y/ritsu` (a release's `.tgz`, from 0.26.0) runs the same
commands with Node, as `npx ritsu check` and `npx rulec …`; there the commands that start another
program or reach the network (`geas check`, `rulec test`, `source fetch`) stop and say to run the
native binary.

---

# Working with ritsu

Your part is to keep the project passing `ritsu check`, to read what it says across the languages
and fix it, and to run and generate from it. Three things stay with people (§7): what the ranges
and bounds are, which warnings to leave, and what a fix would change in the meaning of a file.

## 1. The loop

1. **Check the project.** `ritsu check` in the project's directory reads every `.rule`, `.flow`,
   `.cal`, `.book`, `.geas`, `.req`, `.ctx`, `.gate`, `.kn` and `.proto` under it, or the paths you give
   (`ritsu check rules/ flows/order.flow`). A `.geas` file is checked by running its claims, which
   starts the program they are about.
2. **Read the last line first.** It says how many files each language had, how many failed, and how
   many checks between the languages were made and how many could not be decided (§2).
3. **Fix what a language says** with that language's skill. A diagnostic's heading names the tool,
   `error[rulec E101]`, because every language numbers its own codes: `ritsu rulec explain E101`
   explains that one (through a link to ritsu named rulec, `rulec explain E101` is the same).
4. **Fix what ritsu says across the languages** (§3). `ritsu explain E201` explains one with the
   smallest files that print it.
5. **Check again**, until it ends `all pass`. A warning does not fail the check; it says what could
   not be decided, and why.
6. **Then run and generate**: `ritsu run` plays a workflow with its rules, dates and books worked
   out (§4); `ritsu gen [<path>...]` writes one package for the whole project (§4); the pages
   for people that the languages write (`rulec doc`, `dandori doc`, `koyomi doc`, `chobo doc`,
   `yuen doc`, `sakai doc`, `sekisho doc`, `kaname doc`, each run as `ritsu <language> doc`) are for whoever has to understand
   what the code is to carry out and check it against what they know.
7. **Have a person confirm it.** Hand those pages to the people who own what they say (§7). What
   they find wrong you fix in the file of its language, and the loop starts again.

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
  `.git` or `ritsu.mod`, or the path given. `--root <dir>` says it.
- `--lang ja` (or `RITSU_LANG=ja`) prints the text in Japanese.
- `--budget <n>` raises (or lowers) what the checks that walk a space may walk: the nodes rulec's
  check of one rule visits (E109, or E128 for a machine) and the combinations sekisho walks for one
  action (E307).

## 3. The diagnostics across the languages

`ritsu explain --all` prints every one of these, each with a reproduction you can run. An error
comes with an example that breaks it; a warning says what could not be decided.

| Code | What it says | What to do |
|---|---|---|
| E101 | a `.proto` of the project does not read as proto3 | correct it where it points; a file `buf build` builds, ritsu reads |
| E201 | a workflow's call of a rule, or a kaname module's, can give it values that break one of its preconditions (a `constraint`), or a koyomi date's days one of the rule's | branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or of a task's result); if the precondition is what is wrong, correct the rule's `constraint` |
| W201 | whether a call keeps a precondition cannot be decided: a value comes from a place with no range, or the precondition bounds a total or a length | give the place a range; where none can be given, leave it: the workflow `ritsu dandori build` writes checks it when it runs, and fails a run that breaks it with `Dandori.BrokenPrecondition` |
| E202 | the days a koyomi date comes to fall outside the range of the rule input they are given to | widen the input's range, or make it `range from koyomi`, so that koyomi's days are the range |
| W202 | whether those days stay inside the range cannot be decided: the value can also come from an input, a task's answer or `now` | give the input days of koyomi dates only; or narrow the inputs of its file |
| E203 | a rule's output a workflow gives a chobo transfer as its amount, or an amount a kaname module gives one, can be below 0 or above 2⁶³ − 1 (chobo takes 0 to 2⁶³ − 1) | make the amounts 0 or more (a negative one, a refund say, is a transfer the other way), or branch before the transfer |
| W203 | whether chobo takes the output as an amount cannot be decided: its range has an open end, or the value also comes from a place with no range | give the places the value comes from a range |
| E204 | the transfer can be refused for a reason of the book's bounds (an account's `refused as`) that the task does not handle | declare the reason as an error of the task, and handle it |
| W204 | the task handles a reason that the book's search finds no run for within its depth, or what the amounts are cannot be decided | if the reason cannot happen, drop it from the task's errors; if it happens only after a longer run, leave it |
| E205 | the day a workflow or a kaname module gives a koyomi date can be outside the range of its input | widen the koyomi input's range (and the calendar's data), or give it a day that stays inside |
| W205 | whether the day stays inside the range cannot be decided: it comes from an input, a task's answer or `now` | give it the day of another koyomi date; otherwise leave it, koyomi refuses a day outside its range when the workflow runs |
| E206 | a hold has always expired by the time the flow posts or voids it | make the hold last longer (the transfer's `pending expires after`), or make the call sooner |
| W206 | whether the hold has expired by then cannot be decided: nothing bounds the time between making the hold and the call | give the task that makes the hold, and the tasks between, a `timeout`; where none can be given, leave it |
| E301 | a `ritsu.mod` does not read: a word it does not know, a word with no value, a word written twice that is written once, no `module` first, bytes that are not UTF-8 | correct the line it points at; the first statement is `module` |
| E302 | a value of a `ritsu.mod` is not of its form (a name, a version, an absolute `path`, a dependency's name twice), or a `require … path` leads to a directory with no `ritsu.mod` | write the value in its form; a dependency by a path names a directory with a `ritsu.mod`, from the module's root |
| E303 | a dependency comes from a registry and the module has no `ritsu.lock`, or one that does not read | run `ritsu mod lock`; do not write the lock by hand |
| E304 | `ritsu.lock` does not match a `ritsu.mod`: a version below the least a `require` asks for, a module it has no line for, a source of another kind or path | run `ritsu mod lock` |
| E305 | the files of a dependency in the store or in the module's copy (`.ritsu/modules/`) differ from the lock's digest | `ritsu mod vendor` for the copy; `ritsu mod clean`, then `ritsu mod fetch`, for the store; correct a dependency in its own module |
| E306 | a module of one name comes from two places (two paths, or a `ritsu.mod` fetched that names another module) | read the module from one place |
| E307 | a file or a directory right under a module's root has a name that starts with `@`, which names a dependency there | rename it |
| E308 | a module from a registry has a `require … path`, which cannot be read where it is fetched | tell its author; meanwhile put it in the working tree and read it with `require … path` |
| W301 | no file of a module names a file of a dependency it requires (an `import` of a `.proto` that comes to the dependency counts); said when the whole module is checked | delete the `require` (`ritsu mod remove <name>`), or name a file of `@<name>/…` where it is used |
| W302 | `ritsu.lock` has a line for a module nothing requires now | run `ritsu mod lock` |
| W303 | a file names a file of a dependency of a dependency its module does not require (`@pricing/@tax/…`, or an `import` of a `.proto` that comes to one) | require that module in the module's own `ritsu.mod` |
| W901 | a key is written in a contract: a `.proto` of the project, or an OpenAPI, AsyncAPI or JSON Schema document a language reads (the kind, its prefix and its length; never the key) | take it out and read it from where the code runs; if it is real, tell the person: it has to be revoked with its provider; a value for tests carries `ritsu: test secret` in a comment on its line (`//` in a `.proto`, `#` in YAML) |
| E905 | a workflow sends a value a contract marks secret to a file outside the map, or to a context the map does not relate to the one that marked it | send a reference instead; or ask whether the map is to relate the two, or whether the task is to say `discloses <parameter> "<why>"` |
| W905 | where in the map such a value goes cannot be decided: the map does not pass sakai's check | correct the map so that `sakai check` passes |
| E907 | a context opens an operation (`open host service`) that no action of a gate guards, while its other operations are guarded | write an action that guards it in a `.gate` of that context; if anyone may call it on purpose, write `security: []` on the operation |
| W907 | a context opens operations and no action guards any of them: it has written no authorization with sekisho yet | write a `.gate` of the context, and guard each operation it opens with an action |
| E908 | a workflow calls an operation whose action allows the workflow in no combination: every run that comes to the call is denied | write a permit that allows the workflow (`principal is workflow <name>`), or take the call out |
| W908 | a workflow is allowed an action whose operations its flow never calls | take the action out of the permits that allow the workflow |
| W909 | a call of a workflow can be denied, or whether it is allowed cannot be decided, and the task declares no error for a denial | declare `errors denied = 403` (or `permission_denied` for Connect) on the task, and handle it under the call |

A few things to know before you act on one:

- An **error** is an example, so reproduce it first: the notes give the numbers (the range, the
  value, the day) that break the check, and the line is the call.
- A **warning** is not an error: the project passes. What could not be decided is left to the run,
  where the code of that language refuses or fails it (a rule's code refuses a call that breaks a
  precondition at its door, koyomi's a day outside its range, a book a call on an expired hold).
  Giving a place a range makes the warning decidable, but a range is a claim about the business
  (what a task can return, what days an input can be), so ask the person who owns it (§7) rather
  than narrowing it to silence the warning.
- The ranges in E201 and W201 are dandori's, gathered from every place a value comes from, or, for a
  call of a module of kaname, kaname's, which its analysis of ranges finds and its branches narrow.
  Narrowing one place can turn a warning into a pass or into an error. For a module's call, an error's
  example is an input of the function, found among its vectors, and a warning that cannot be decided
  is checked by the code kaname generates when it runs.

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
tasks) with `books`, what was done to the books before the run. `ritsu dandori scenarios
<file.flow>` writes scenarios that take every arm; the calls of the flow's own rules are rulec's to
answer there too (`"rules": "rulec"`), so each runs under `ritsu run` as it is. A scenario that
answers those calls itself, written before, stops `ritsu run` at the first of them (exit 2). `--target` shows the calls as a platform would make them,
`--format json` prints one object. Exit codes: 0 the run came to its end (succeeded, failed or was
cancelled), 1 the flow has errors or the run could not go on (the scenario ran out of answers, say),
2 bad arguments or a scenario that cannot be run.

**`ritsu gen [<path>...] [--target <language>] [--out <dir>]`** writes one package for each of
TypeScript, Python and Go (`<out>/<language>/`, `generated` by default): the rules, the dates, the
clients of the books, the workflows, the code that asks the gates and the functional cores
(kaname's modules, under `glue/`) of the project, where a workflow and a module read their rules, dates and books,
and a gate's code its rules and dates, from the package itself; the gates' Cedar goes once into `<out>/cedar/`. An amount of money or of a quantity is of
the package's one type of its unit, declared in `units` and taken and returned in it by the rules
and the clients of the books alike, so code written by hand that gives a book what a rule returns,
with no workflow between them, is held to the book's unit by the type checker of the language
(`USDc` from a rule is what the book's `sale` takes; `USD` is not). Run it after `ritsu check` passes. `--check`
writes nothing and exits 1 if a package is stale, for CI; `--format json` prints one object instead
of lines, every file of the packages with what became of it (`written`, `same` or `removed`; under
`--check`, `same`, `missing`, `stale` or `left`), or why nothing was generated with the diagnostics
of the files that stopped it; `--books postgres|tigerbeetle|mysql` says what the clients of the
books call (`mysql` is MySQL, TiDB or Vitess); `--authorizer cedar|avp` says whether a gate's code
asks the language's own Cedar in the process or Amazon Verified Permissions; `--name` and
`--module` name the package. Each language's own `gen` or `build` still writes that language's code alone (`rulec
gen`, `koyomi gen`, `chobo build`, `dandori build`, `sekisho gen`).

## 5. Which skill to read

Each language has a skill of its own, with the language on one page, what to ask a person, and the
fix for each of the language's own diagnostics. `ritsu skills install` writes all ten into the
project's `.claude/skills/` (`--user` for `~/.claude/skills/`, `--dir <dir>` for another agent's
folder; name the ones you want, `ritsu skills install rulec dandori`). They are also in ritsu's
repository at the paths below, to copy beside this one.

| Language | Files | Skill | Read it to |
|---|---|---|---|
| rulec | `.rule` | `skills/rulec/` | write a rule, fix rulec's diagnostics, call the generated function |
| dandori | `.flow` | `skills/dandori/` | write a workflow, write scenarios, build for Temporal, Step Functions, Argo and the others |
| koyomi | `.cal` | `skills/koyomi/` | write a date and a calendar, fix koyomi's diagnostics |
| chobo | `.book` | `skills/chobo/` | write a ledger, fix chobo's diagnostics, call the client |
| geas | `.geas` | `skills/geas/` | write claims about code an agent wrote |
| yuen | `.req` | `skills/yuen/` | write requirements and what they come from |
| sakai | `.ctx` | `skills/sakai/` | write a map of bounded contexts |
| sekisho | `.gate` | `skills/sekisho/` | write who may do what, fix sekisho's diagnostics, generate Cedar and the code that asks it |
| kaname | `.kn` | `skills/kaname/` | write a functional core, general code with no input or output that may call rules, dates and ledgers, fix kaname's diagnostics, show which functions are total |

## 6. A language that reads another

`ritsu <language> …` is that language's own command, with the languages it reads joined in the same
process. `dandori`, `yuen`, `sakai`, `sekisho` and `kaname` read others, so a command of theirs on
a file that uses another language's files is run as `ritsu dandori …`, `ritsu yuen …`, `ritsu sakai
…`, `ritsu sekisho …` or `ritsu kaname …`. A link to ritsu named for the language does the same. Built alone from its own
crate, each reads no other language and says so: dandori's E018, yuen's E206, sakai's E104,
sekisho's E209 and kaname's E202 name the command to run instead. `rulec` reads koyomi's days (`range from koyomi`), so `ritsu rulec check` is what checks
such a rule.

## 7. What stays with people

- **What the ranges and bounds are.** The range of an input or of a task's result, the bounds of an
  account, how long a hold lasts, which days a calendar closes: each is a claim about the business,
  and every check between the languages stands on it. Ask, do not guess one to make a check pass.
- **Which warnings to leave.** A warning that cannot be decided is true; whether it matters is a
  decision (§3).
- **What a fix would change.** E201's other fix, correcting a rule's `constraint`, changes what the
  rule promises everyone who calls it; E206's, a longer hold, changes how long stock or money is
  set aside.
- **Which version of a dependency to take, and what to publish.** A new version of a module changes
  the answers of every file that reads it: show the person what `ritsu mod report` says, and let
  them decide. Pushing and signing a module are theirs too: they log in to the registry
  (`docker login`) and choose the identity its signature names (§9).
- **Reading the pages.** The pages for people that the languages write are for whoever has to
  understand what the code is to carry out and check it against what they know: the people who run the business,
  finance and legal, operators and the developers reviewing the code. Hand them the page; what they
  find wrong is for you to fix. ritsu's site also runs ritsu in the browser
  (<https://i2y.github.io/ritsu/playground/>): a person can open a project there and see what
  `ritsu check` says, the generated code and the pages for people, with nothing installed and
  nothing sent anywhere.

## 8. A project that keeps its behavior in OpenSpec too

OpenSpec (`openspec/specs/`, with changes proposed under `openspec/changes/` and applied by
`openspec archive`) is where people and agents agree, in prose, on what a system should do. The
languages hold the rest, each its own part; keep each thing where it belongs.

| Where | What it holds | Who decides |
|---|---|---|
| OpenSpec | the behavior in prose: requirements (SHALL, MUST) and their scenarios; the changes proposed, reviewed and archived | the people who agree on the behavior; the agent drafts |
| geas (`.geas`) | each scenario as a claim run against the real program, named as the scenario is | the person reads the claims; the agent writes the code and proposes claims |
| rulec, koyomi, chobo, dandori | what a requirement needs exactly: a tariff or an eligibility rule, a closing or payment day, the bounds of an account, a workflow | the people who own each |
| yuen (`.req`) | where each requirement comes from (a requirement of an OpenSpec spec, an article of a law, a decision), who owns it, what meets it and what checks it, each link recorded with the hashes of its ends | the owner of each requirement |

With a change in OpenSpec:

1. **Before it is archived.** `ritsu geas scenarios <spec.geas> --openspec openspec/changes/<id>`
   lists the scenarios the change asks for that no claim runs; propose a claim for each
   (`--draft` gives the frame). `ritsu yuen source outdated <path>` says which requirements of the
   project, which owners and how many links the change will reach.
2. **The work.** Write the code, and each rule, date, account or workflow the change needs in its
   own language; `ritsu check` holds them together.
3. **After `openspec archive`.** `ritsu yuen check` stops on each requirement the change rewrote
   (E103, with the diff). Show it to the owner; `ritsu yuen source pin` then pins the new blocks,
   and the links below them are marked until the people who own them look (`ritsu yuen review`,
   only when they ask). A requirement the change added that nothing reads is W102: ask whether to
   read it, or to leave it out with waivers whose reasons the owner gives.

Do not copy a requirement's prose into a rule or a claim to make the two look alike. A rule holds
the exact table the prose describes, and a claim holds a case anyone could observe; yuen keeps the
link between them and the prose, and says when either moves.

## 9. Modules

A directory with a `ritsu.mod` is a **module**: its name, its version, and the modules it requires.
Its files name a dependency's files by the name `ritsu.mod` gives the dependency, `@<name>/…`
(`use rule shipping from "@pricing/rules/shipping_fee.rule"`,
`use calendar "@holidays/uk/england_and_wales.cal"`), and `ritsu check` checks them with the
project's own files, so the checks across the languages reach into a dependency as into the
project. `ritsu mod --help` lists the commands; `ritsu mod <command> --help` says what each takes.

**Require a module** with `ritsu mod add <name> <source>`: a directory beside the project
(`../pricing`) or a module on an OCI registry (`ghcr.io/i2y/ritsu-holidays@1.0.0`).
`ritsu mod get <name>@<version>` raises it, and `ritsu mod remove <name>` drops it. They write
`ritsu.mod` and `ritsu.lock`. Do not write the lock by hand: `ritsu mod lock` writes it again, with
the least version of each module that meets every `require` of it.

**Fetch** what the lock pins with `ritsu mod fetch`, into a store kept across runs (`RITSU_MODULES`,
else the user's cache directory). Only `ritsu mod` reaches the network: `check`, `gen`, `run` and the
languages' commands read the store, and the module's own copy that `ritsu mod vendor` puts in
`.ritsu/modules/` for a build that cannot reach the registry. Every command holds what it reads to
the lock's digests (E305). `ritsu mod list` says where each module comes from and whether its files
are here, and `ritsu mod where @<name>/<path>` where a file is on the disk.

**What `ritsu check` prints** in a module: its files as before, and a line for the dependencies,
whose files are checked too:

```text
ok checkout/basket.kn: 2 functions, total
orders/place_order.flow: ok
dependencies: 2 modules, 3 files (rulec 3): all pass
ritsu check: 2 files (kaname 1, dandori 1): all pass; borders between the languages: 0 checked, 0 undecided
```

That line counts a dependency's warnings, and `--deps-warnings` shows each. A dependency's `.geas`
claims are not run. An error in a dependency's own files is its author's to fix: tell the person,
and do not copy the files into the project to change them.

**Raising a version** changes the answers of the files that read the module, so show what it
changes before it is merged. `ritsu mod diff <module>@<old> <module>@<new>` compares two versions of
a module: the files, the requires, every input of a rule, a date or a gate that gets another answer,
and the interfaces (exit 1 when it shows a difference of behavior or of an interface, 2 when it
cannot compare). `ritsu mod report --base origin/main --format markdown` writes what a pull request
that changes the lock changes: the check with each lock, and the diff of each module that changed
(exit 1 on an error with the new lock, or, with `--fail-on-change`, on a difference of behavior; 2
when a module that changed cannot be compared). `rulec diff`, `koyomi diff` and `sekisho diff` take
a dependency's file as `@<name>/<path>` (the lock's version) or `@<name>@<version>/<path>` (a version
in the store).

**Publishing** is the person's (§7). `ritsu mod push --dry-run` checks and builds the module without
sending it. `ritsu mod push` sends it with the credentials of Docker's configuration, refuses a
module whose files hold a key (W901), and prints the `cosign sign` command that signs it; never ask
for a token or print one. A `signature` line in a `ritsu.mod` makes every fetch of that module
check its signature with cosign before keeping it.

**A site that lists modules**: `ritsu mod site <list> --out <dir>` writes a page for each version of
each module a list file names, with the pages of its files and what changed from the version
before. A line of the list starts with `title`, `description`, `playground` (the page that runs
ritsu in the browser; `--playground <URL>` gives it too), `module` (`"<registry>/<repository>"`),
`signature` (as in `ritsu.mod`, checked on every version) or `local` (a directory not pushed yet,
from the list's directory). It exits 1 when a version cannot be verified (its page says so), and 2,
writing nothing, when the list does not read (it names each wrong line), a registry cannot be
reached, or cosign is needed and missing.

**In the npm package**, which reaches no network, the commands that reach a registry or run git or
cosign (`add`, `get`, `fetch`, `versions`, `push`, `report` and `site`, and `lock`, `remove` and
`diff` when they have to fetch) stop and say to run the native ritsu. When it says a dependency is
neither in the store nor in the module's copy, run `ritsu mod fetch` with the native ritsu; if it is
fetched already and the API's `dirs` opens only some directories, add the store to `dirs`, or put
the copy in the module with `ritsu mod vendor`.
