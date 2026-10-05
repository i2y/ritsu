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

## The seven languages

| | File | What it is for | What it generates |
|---|---|---|---|
| [rulec](#rulec--business-rules) | `.rule` | business rules: tariffs, fees, eligibility, tax | functions in twelve languages |
| [dandori](#dandori--the-skeleton-of-a-workflow) | `.flow` | the skeleton of a workflow | Temporal, Step Functions, Argo and more |
| [koyomi](#koyomi--dates) | `.cal` | dates: closing and payment days, business days, legal periods | TypeScript, Python, Go, Rust, SQL |
| [chobo](#chobo--ledgers) | `.book` | ledgers: accounts, their bounds, transfers, holds | clients for PostgreSQL and TigerBeetle |
| [geas](#geas--claims-about-the-code-an-agent-wrote) | `.geas` | claims about the code an agent wrote | — |
| [yuen](#yuen--where-requirements-come-from) | `.req` | where requirements come from, and what satisfies them | ReqIF, W3C PROV |
| [sakai](#sakai--the-map-of-bounded-contexts) | `.ctx` | the map of bounded contexts | settings for import linters; Context Mapper |

Each language stands on its own: you can use rulec without writing a `.flow`. A file never mixes
two languages, because a tariff, a calendar, a ledger and a workflow are read by different
people.

### rulec — business rules

A rule is a set of tables, with the calculations, the exceptions and the provisos around them.
`rulec check` proves that every input in the declared domain gets exactly one answer, checks
units and money, and writes a certificate that a proof in Lean rechecks. A figure can be pinned
to the published source it was copied from, so that a change in the source stops the check.
Only a rule that passes compiles, into plain functions in Python, NumPy, TypeScript,
JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm.

```rule
rule paypal_fee v1
description "The PayPal Checkout fee on one payment in the United States. Transcribed from PayPal's published merchant fees"

source paypal = file "sources/paypal-us-fees.md" sha256:0318950a982c3c7d  # PayPal's own published figures
  table1 sha256:5e481ef40e570eeb

inputs
  amount        : money[USDc]  range >=1USDc <=100000000USDc
  international : bool

# The fee has fractions of a cent in it, and the page does not say which way they settle, so
# the direction here is a placeholder — the thing a person has to decide before this ships.
outputs
  fee : money[USDc]  round half_up(1USDc)

# The page prints the domestic rate and, separately, what an international transaction adds.
# It does not print the sum, so neither does this: the row for a domestic payment adds
# nothing, and that row carries no citation because `0%` is not a figure the copy shows.
table surcharge
policy unique
| international | -> extra : rate[step 0.01%] |
| false         | 0%                          |
| true          | 1.5%                        |  @paypal table1

define fee : money[USDc] = amount × 3.49% + amount × extra + 49USDc  @paypal table1

examples
| amount    | international | -> fee  |
| 10000USDc | false         | 398USDc |
| 10000USDc | true          | 548USDc |
```

### dandori — the skeleton of a workflow

A workflow says what is called, in what order, with which retries and timeouts, and what each
call does to a case outside — an order, a payment, a booking. `dandori check` shows that on
every path, each case the flow does not hand on is left untouched or finished, never halfway.
The same file becomes Temporal (TypeScript, Python, Go), Step Functions, Argo Workflows, Lambda
durable functions or pydantic-graph, and each is run against dandori's reference interpreter.

```flow
workflow arrange_delivery v1
description "Book the delivery of an order with the carrier the urgency rule chose, and answer with its tracking number. Written once for every platform, since its calls are HTTP ones that dandori writes for each (`connection` is what Step Functions needs of them): each version of fulfillment runs it as its child, a child workflow on Temporal, an invoked durable function on Lambda durable functions, a workflow of this WorkflowTemplate on Argo, a nested execution on Step Functions"

enum carrier = standard | next_day

record Booking
  tracking_number : string

inputs
  order_id  : string
  carrier   : carrier
  recipient : string?
  extra     : json

outputs
  tracking_number : string

# The next-day carrier sends a van for the parcels; on a busy day it has no van left.
task book_next_day(order_id: string, recipient: string?) -> Booking
  http POST "https://next-day.example.com/v1/pickups"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/next-day/2a3b4c"
  errors no_van = 409
  key
  retry 2 times every 5 seconds

task book_standard(order_id: string, recipient: string?, extra: json) -> Booking
  http POST "https://post.example.com/v1/parcels"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/post/3b4c5d"
  key
  retry 2 times every 5 seconds

flow
  match carrier
    next_day =>
      let booked = book_next_day(order_id: order_id, recipient: recipient)
        on no_van => fail NoVan "No next-day van is left for order {order_id}"
      succeed tracking_number = booked.tracking_number
    standard =>
      let booked = book_standard(order_id: order_id, recipient: recipient, extra: extra)
      succeed tracking_number = booked.tracking_number
```

### koyomi — dates

A date is written as it is said in a contract or a law: thirty days after the invoice, moved to
the next business day. `koyomi check` computes it on every day of the declared range and checks
each claim on every one of them. The calendar comes from a pinned source — here GOV.UK's bank
holidays for England and Wales. It generates TypeScript, Python, Go, Rust and SQL.

```cal
dates net30 v1
description "Net 30: due 30 days after the invoice date, moved to the next business day in England and Wales when that day is closed. The claims are this example's own, written as they read"
use calendar "calendars/england_and_wales.cal"

inputs
  invoice_date : date  range >=2026-01-01 <=2028-11-29

date due = invoice_date
  + 30 days             # "Net 30": due 30 days after the invoice date
  roll following        # on the next business day when that day is closed

claims
  due_on_a_business_day   : due is open
  at_least_30_days        : due >= invoice_date + 30 days
  later_invoice_later_due : due is monotonic
```

```console
$ cd crates/koyomi/examples
$ koyomi check net30.cal
net30.cal: ok — 3 claims hold on all 1,064 days of invoice_date (2026-01-01..2028-11-29)
```

### chobo — ledgers

A book declares accounts and the bounds each must keep, and the transfers between them. chobo
checks that no sequence of transfers, holds and expiries can take an account past a bound, and
generates clients in which every transfer is one write that keeps them, on PostgreSQL or on
TigerBeetle, in TypeScript, Python and Go.

```book
# A refund never exceeds the sale: what is left to refund on each order is an account of its own.
book refunds v1
description "What is left to refund on each order is an account of its own: the sale adds to it, a refund takes from it, and it never goes below 0. A refund is held while it waits for approval"

unit USD scale 2

account refundable(order: string) : USD
  description "what is left to refund on the order"
  at least 0 refused as refund_exceeds_sale
account sales : USD outside
account refunded : USD outside

transfer sale(order: string, amount: USD)
  key order
  move amount from sales to refundable(order)

transfer refund(request: string, order: string, amount: USD)
  description "held while it waits for approval: posted when it is approved, voided when it is turned down"
  key request
  pending expires after 7 days
  move amount from refundable(order) to refunded
```

### geas — claims about the code an agent wrote

The code is the agent's; the claims are what a person reads. geas runs each claim against the
program — a command line, an HTTP service, a page in a browser — and, given a diff, tells which
claims it touches. Given an OpenSpec spec, `geas scenarios` lists the scenarios that no claim of the
same name checks.

```geas
# The human-auditable half. The implementation (calc.py) is agent-written;
# these claims are what a reviewer actually reads.

target calc {
  run "python3 calc.py"
}

claim "adds two integers" {
  when calc.run("2", "+", "3")
  then stdout is "5"
  and  exit is 0
}

claim "multiplies" {
  when calc.run("6", "*", "7")
  then stdout is "42"
}

claim "refuses division by zero" {
  when calc.run("1", "/", "0")
  then exit is 1
  and  stderr contains "division by zero"
}

claim "rejects an unknown operator" {
  when calc.run("1", "%", "2")
  then exit is 2
  and  stderr contains "unknown op"
}
```

### yuen — where requirements come from

Each requirement says where it came from (an article of a law, a document, a decision), who owns
it, and what satisfies and verifies it. Every link is pinned by hashes at both ends: when the
source, the requirement or the thing that satisfies it changes, the links after it stop until
someone looks again. yuen reads laws from the US eCFR and Japan's e-Gov, and exports ReqIF and
W3C PROV. It pins the requirements of an [OpenSpec](https://github.com/Fission-AI/OpenSpec) spec
the same way, and stops when a change rewrites one.

```req
requirements osha v1
description "A requirement read from 29 CFR 1910.157, for the tests. Written as an example; it does not say how the regulation is to be read"

role safety "decides how the regulation reads"

source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269

requirement extinguisher_distance
  text "No employee travels more than 75 feet to a portable fire extinguisher for Class A fires"
  owner safety
  from @osha "§1910.157"
    reviewed 2026-10-03 by safety sha256:c2a9ce966c7e2269 -> sha256:08a4819829372b9b
  not satisfied "The test material names no artifact"
    approved 2026-10-03 by safety sha256:08a4819829372b9b
  not verified "The test material names no claim"
    approved 2026-10-03 by safety sha256:08a4819829372b9b
```

### sakai — the map of bounded contexts

A map says which context owns which files, which language each publishes, and who may depend on
whom, through what. sakai checks every reference that crosses contexts, in the files of every
language here, in the OpenAPI and AsyncAPI documents services keep as their contracts, and in the
code — handing imports to import-linter, dependency-cruiser, ArchUnit and go-arch-lint, and reading
Rust's crates from Cargo. ritsu maps its own crates this way; this is one of its contexts:

```ctx
context Workflows(dandori) v1
description "Workflows that call the rules, made into Temporal, Step Functions, Argo and more (dandori)"

owns
  dir "../crates/dandori"

published language dandori
  crate "../crates/dandori"

# A language stands on the base, and reads another language only through the ports.
upstream Base conformist
  through ritsu_base, ritsu_units, ritsu_proto, ritsu_emit, ritsu_ports
```

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
reviewing the code — so they can read what was written and check it against what they know. A
team that signs off on its rules can sign off on these pages too.

- `rulec doc` — a rule's tables, provisos and what was proved of them, in Markdown or HTML;
- `dandori doc` — a workflow drawn with every scenario it runs;
- `koyomi doc` — a calendar, month by month;
- `chobo doc` — a ledger, its bounds and its transfers;
- `yuen trace` — a requirement back to its source and on to what satisfies it;
- `explain <code>` — every diagnostic, with a reproduction you can run.

## Proofs

rulec's certificates are rechecked in Lean 4. Lean models of chobo, koyomi, dandori's core and
ritsu's checks across languages sit beside the Rust, and the two are run on the same generated
inputs — millions of lines — and compared answer by answer. No proof rests on `sorry` or an
axiom of its own.

## For AI agents

ritsu and its languages are made to be used by AI agents, and [skills/](skills) holds eight
[Agent Skills](https://agentskills.io) for them: [skills/ritsu](skills/ritsu) for a project of more
than one language (the loop from `ritsu check` to a run and a package, how to read each diagnostic
across the languages and fix it, and which language's skill to read for the rest), and one for each
of the seven languages, in `skills/<language>/`. [skills/README.md](skills/README.md) lists them.
There are four ways to install them:

- **Claude Code**: ritsu's site publishes a plugin marketplace whose plugin `ritsu` holds the
  eight. Run `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json`, then
  `/plugin install ritsu@ritsu`; Claude Code fetches the folder `skills/` alone, not the whole
  repository.
- **Any agent, from the binary**: `ritsu skills install` writes them into the project's
  `.claude/skills/`; with `--user`, into `~/.claude/skills/`; with `--dir <dir>`, where another
  agent reads skills. Names after it (`ritsu skills install rulec dandori`) write only those, and
  `ritsu skills list` lists them.
- **By hand**: copy the folders you need from `skills/` into `~/.claude/skills/`, or into a
  project's `.claude/skills/`.
- **From a release**: `ritsu-skills-v<version>.zip` holds the eight folders; unzip it where your
  agent reads skills.

## Commands

```
ritsu check <dir>         every language's check, then the checks across them
ritsu run <flow> …        a workflow run with its rules evaluated, dates computed, books moved
ritsu gen <dir> --out …   one package of TypeScript, Python or Go for the whole project
ritsu explain <code>      what a diagnostic means, with a reproduction
ritsu skills install      the Agent Skills, written where an agent reads them
ritsu <language> …        a language's own commands, as in ritsu rulec doc fee.rule
```

A release archive holds `ritsu` and seven links to it — `rulec`, `dandori`, `koyomi`, `chobo`,
`geas`, `yuen`, `sakai` — and each runs as its language. `cargo install` installs the `ritsu`
binary alone: call a language as `ritsu <language> …`, or make the link yourself, named for the
language (`ln -s "$(command -v ritsu)" ~/.cargo/bin/rulec`). Add `--lang ja` for Japanese.

## Install

From source, with a recent stable Rust:

```
cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

Only one language — rulec, say, which needs no other:

```
cargo install --git https://github.com/i2y/ritsu --locked rulec
```

Every release, from 0.23.0 on (continuing rulec's numbering), carries archives for macOS and
Linux and `.deb` and `.rpm` packages, on the [releases page](https://github.com/i2y/ritsu/releases).
With Homebrew it is `brew install i2y/tap/ritsu`, and in GitHub Actions `uses: i2y/ritsu@v0.23.0`.

## Repository

- `crates/<language>` — each language, with its own README and design notes:
  [rulec](crates/rulec/README.md), [dandori](crates/dandori/README.md),
  [koyomi](crates/koyomi/README.md), [chobo](crates/chobo/README.md),
  [geas](crates/geas/README.md), [yuen](crates/yuen/README.md),
  [sakai](crates/sakai/README.md)
- `crates/ritsu-*` — the shared base, units, ports, project reading, the checks across
  languages, the `.proto` reader, the emitters, the browser build
- `proofs/` — the Lean models
- `website/` — the site: ritsu's pages and the playground, with rulec's site in `website/rulec` and dandori's in `website/dandori`
- `DESIGN.md`, `PLAN.md` — the design of the whole (in Japanese)
- `SECURITY.md` — how to report a vulnerability

## License

MIT OR Apache-2.0
