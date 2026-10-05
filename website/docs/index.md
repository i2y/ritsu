# ritsu

**Seven small languages, one toolchain. What one checks, the next can build on.**

Coding agents now write code faster, and more of it, than anyone can read line by line. That
makes one question sharper: what is the code there to carry out, whoever — or whatever — writes
it? The contracts between services and inside them. The business rules. The calendars and the
deadlines. The ledgers and the bounds on them. The skeleton of a workflow. Where each
requirement came from, and what still satisfies it.

ritsu gives each of those a small language of its own, in a file people can read. Each language
checks what it says as far as it can be checked — over every input, every day, every path, not a
sample — generates code where code is needed, and draws pages for whoever needs to understand
it. Because the seven share one toolchain, a proof does not stop at a language's edge: a
workflow knows the preconditions of the rules it calls, a rule knows the days a calendar can
come to, a ledger knows the amounts a rule can return.

Agents write the glue. ritsu holds what it all has to carry out.

[Try it in the browser](playground.md){ .md-button .md-button--primary }
[GitHub](https://github.com/i2y/ritsu){ .md-button }

## The seven languages

| | File | What it is for | Read more |
|---|---|---|---|
| rulec | `.rule` | business rules: tariffs, fees, eligibility, tax | [the rulec site](rulec/) |
| dandori | `.flow` | the skeleton of a workflow | [the dandori site](dandori/) |
| koyomi | `.cal` | dates: closing and payment days, business days, legal periods | [README](https://github.com/i2y/ritsu/blob/main/crates/koyomi/README.md) |
| chobo | `.book` | ledgers: accounts, their bounds, transfers, holds | [README](https://github.com/i2y/ritsu/blob/main/crates/chobo/README.md) |
| geas | `.geas` | claims about the code an agent wrote | [README](https://github.com/i2y/ritsu/blob/main/crates/geas/README.md) |
| yuen | `.req` | where requirements come from, and what satisfies them | [README](https://github.com/i2y/ritsu/blob/main/crates/yuen/README.md) |
| sakai | `.ctx` | the map of bounded contexts | [README](https://github.com/i2y/ritsu/blob/main/crates/sakai/README.md) |

Each language stands on its own: you can use rulec without writing a `.flow`. A file never mixes
two languages, because a tariff, a calendar, a ledger and a workflow are read by different
people.

- **rulec** writes a rule as a set of tables, with the calculations, the exceptions and the
  provisos around them. `rulec check` proves that every input in the declared domain gets exactly
  one answer, checks units and money, and writes a certificate that a proof in Lean rechecks. Only
  a rule that passes compiles, into plain functions in twelve languages.
- **dandori** says what a workflow calls, in what order, with which retries and timeouts, and what
  each call does to a case outside — an order, a payment, a booking. `dandori check` shows that on
  every path each case is left untouched or finished, never halfway, and the same file becomes
  Temporal, Step Functions, Argo Workflows, Lambda durable functions or pydantic-graph.
- **koyomi** writes a date as a contract or a law says it: thirty days after the invoice, moved to
  the next business day. `koyomi check` computes it on every day of the declared range and checks
  each claim on every one of them.
- **chobo** declares accounts, the bounds each must keep, and the transfers between them, checks
  that no sequence of transfers, holds and expiries can take an account past a bound, and generates
  clients for PostgreSQL and TigerBeetle.
- **geas** runs each claim about a program against it — a command line, an HTTP service, a page in
  a browser — and, given a diff, tells which claims it touches. Given an OpenSpec spec, it lists the
  scenarios that no claim of the same name checks.
- **yuen** says where each requirement came from, who owns it, and what satisfies and verifies it,
  with every link pinned by hashes at both ends. It pins the requirements of an OpenSpec spec the
  same way.
- **sakai** maps which context owns which files and who may depend on whom, and checks every
  reference that crosses contexts, in the files of every language here, in the OpenAPI and AsyncAPI
  documents services keep as their contracts, and in the code.

## Where the languages meet

The languages share one toolchain so that what one has checked, another can take as given.
Here a refund rule takes for granted that nobody asks for more than they paid:

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

A workflow calls it with whatever the customer asked for:

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int range >=0 <=10000
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

Each file passes its own language's check. Together they do not (the two files are the
reproduction that `ritsu explain E201` prints):

```console
$ ritsu check .
ok refund_check.rule
refund.flow: ok
error[ritsu E201]: refund.flow:21:1: The call of the rule check can give it values that break its precondition `asked <= paid`
    21 |   let decision = check(paid: paid, asked: asked)
  = `asked` is `>=0 <=10000` and `paid` is `>=0 <=10000`, and at asked = 10000, paid = 0 `asked <= paid` does not hold
  = The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. The ranges are dandori's, gathered from every place the values come from.
  = Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result).
ritsu check: 2 files (rulec 1, dandori 1): 1 fail (1 error); borders between the languages: 1 checked, 0 undecided
```

Every check that crosses languages comes to one of three answers: it holds, here is a case where
it does not, or it cannot be decided — and then it says why. Nothing passes in silence. Among
them:

- a rule's preconditions, at every call a workflow makes;
- the days a koyomi date can come to, as the range of a rule's input (`range from koyomi`), and
  against the range the rule declares;
- a rule's output as the amount of a chobo transfer, and the refusals that transfer can come to;
- a chobo hold's expiry, against the wait a workflow counts in business days;
- money and units, one type across rulec, dandori and chobo;
- yuen pins each table, date, claim and task one by one, and sakai checks every reference in
  every language, with its line.

## Pages for people

The same files give pages, in English or Japanese, for the people who need to understand what the
code is to carry out — those who run the business, finance and legal, operators, the developers
reviewing the code — so they can read what was written and check it against what they know.

- `rulec doc` — a rule's tables, provisos and what was proved of them, in Markdown or HTML;
- `dandori doc` — a workflow drawn with every scenario it runs;
- `koyomi doc` — a calendar, month by month;
- `chobo doc` — a ledger, its bounds and its transfers;
- `yuen trace` — a requirement back to its source and on to what satisfies it.

## Install

From source, with a recent stable Rust:

```
cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu check <dir>` runs every language's check, then the checks across them, and
`ritsu <language> …` a language's own commands, as in `ritsu rulec doc fee.rule`. Every release,
from 0.23.0 on (continuing rulec's numbering), carries archives for macOS and Linux and `.deb` and
`.rpm` packages, on the [releases page](https://github.com/i2y/ritsu/releases); an archive holds
`ritsu` and a link to it named for each language, which runs as that language. With Homebrew it is
`brew install i2y/tap/ritsu`, and in GitHub Actions `uses: i2y/ritsu@v0.23.0`.

The design of the whole is in [DESIGN.md](https://github.com/i2y/ritsu/blob/main/DESIGN.md), in
Japanese. ritsu is MIT OR Apache-2.0.

## For AI agents

ritsu and its languages are made to be used by AI agents, and the repository holds eight
[Agent Skills](https://agentskills.io) for them: [one for ritsu](https://github.com/i2y/ritsu/tree/main/skills/ritsu),
for a project of more than one language, and one for each of the seven languages
([skills/README.md](https://github.com/i2y/ritsu/blob/main/skills/README.md) lists them). There are
four ways to install them:

- **Claude Code**: this site publishes a plugin marketplace whose plugin `ritsu` holds the eight.
  Run `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json`, then `/plugin install ritsu@ritsu`;
  Claude Code fetches the folder `skills/` alone, not the whole repository.
- **Any agent, from the binary**: `ritsu skills install` writes them into the project's
  `.claude/skills/`; with `--user`, into `~/.claude/skills/`; with `--dir <dir>`, where another
  agent reads skills. Names after it (`ritsu skills install rulec dandori`) write only those, and
  `ritsu skills list` lists them.
- **By hand**: copy the folders you need from `skills/` into `~/.claude/skills/`, or into a
  project's `.claude/skills/`.
- **From a release**: `ritsu-skills-v<version>.zip` holds the eight folders; unzip it where your
  agent reads skills.
