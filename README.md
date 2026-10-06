# ritsu

**Eight small languages, one toolchain. What one checks, the next can build on.**

Coding agents now write code faster, and more of it, than anyone can read line by line. That
makes one question sharper: what is the code there to carry out, whoever — or whatever — writes
it? The contracts between services and inside them. The business rules. The calendars and the
deadlines. The ledgers and the bounds on them. The skeleton of a workflow. Who may do what.
Where each requirement came from, and what still satisfies it.

ritsu gives each of those a small language of its own, in a file people can read. Each language
checks what it says as far as it can be checked — over every input, every day, every path, not a
sample — generates code where code is needed, and draws pages for whoever needs to understand
it. Because the eight share one toolchain, a proof does not stop at a language's edge: a
workflow knows the preconditions of the rules it calls, a rule knows the days a calendar can
come to, a ledger knows the amounts a rule can return.

Agents write the glue. ritsu holds what it all has to carry out.

## The eight languages

| | File | What it is for | What it generates |
|---|---|---|---|
| [rulec](#rulec--business-rules) | `.rule` | business rules: tariffs, fees, eligibility, tax | functions in twelve languages |
| [dandori](#dandori--the-skeleton-of-a-workflow) | `.flow` | the skeleton of a workflow | Temporal, Step Functions, Argo and more |
| [koyomi](#koyomi--dates) | `.cal` | dates: closing and payment days, business days, legal periods | TypeScript, Python, Go, Rust, SQL |
| [chobo](#chobo--ledgers) | `.book` | ledgers: accounts, their bounds, transfers, holds | clients for PostgreSQL and TigerBeetle |
| [geas](#geas--claims-about-the-code-an-agent-wrote) | `.geas` | claims about the code an agent wrote | — |
| [yuen](#yuen--where-requirements-come-from) | `.req` | where requirements come from, and what satisfies them | ReqIF, W3C PROV |
| [sakai](#sakai--the-map-of-bounded-contexts) | `.ctx` | the map of bounded contexts | settings for import linters; Context Mapper |
| [sekisho](#sekisho--who-may-do-what) | `.gate` | who may do what: roles, attributes, relations, with rules and dates as conditions | Cedar, and the TypeScript, Python and Go that ask it |

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

### sekisho — who may do what

A gate says which principal may do which action on which resource, with roles, attributes and
relations, and with the answers of a rule and of dates as conditions. `sekisho check` asks rulec
which answers the rule can give and koyomi which dates can come together, walks every combination
that can happen, and decides each as Cedar does: an action no one can do, a permit a forbid covers,
an expectation that does not hold, two duties one person is allowed, all stop the check. Only a
gate that passes compiles, into Cedar's schema and policies, and into TypeScript, Python and Go that
compute the conditions from the service's own data and ask Cedar, so a caller cannot hand them in.

```gate
action refund_order
  description "Refund an order, in part or in whole"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId
  input
    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  context
    refund_band  = refund_limit(amount: amount, limit: principal.refund_limit).band
    in_period    = today <= refund_terms.last_day(paid_on: resource.paid_on)
    business_day = today is open in uk
…
permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
  principal in clerk
  action refund_order
  when refund_band is within_limit
  when in_period
…
expect deny clerks_never_refund_over_their_limit
  description "A clerk who is not a manager never refunds more than the clerk's own limit"
  principal in clerk
  action refund_order
  unless principal in manager
  when refund_band is over_limit
```

```console
$ cd crates/sekisho/examples/refunds
$ sekisho check refunds.gate
refunds.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
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
- every operation a context of sakai's map opens, held to the action of a gate that guards it, and
  every operation a workflow calls, to what its gate allows the workflow;
- yuen pins each table, date, claim and task one by one, and sakai checks every reference in
  every language, with its line.

## Security checks

The languages also read their files for what leaves a secret or a service exposed: a key written
into a file, a connection that is not encrypted, an operation with no authentication, a value a
contract marks secret that a workflow keeps in its history or sends away. None of it uses the
network or runs anything, and none of it looks dependencies up in a vulnerability database: it
reads what the project says.

| Code | What it finds | Where it shows |
|---|---|---|
| W901 | a key in the shape its provider gives it (AWS, GitHub, Slack, Stripe, OpenAI, Anthropic, Google, a PEM private key) written in a file, shown by its kind, prefix and length, never the key | each language's `check`, in its own files; `ritsu check`, in the project's `.proto` files and the documents a `.flow`, a `.rule` or a map's published language points at |
| W902 | a connection that is not encrypted, to a host that is not this machine | `dandori check`, for the URLs a task calls; `sakai check`, for the servers of a map's OpenAPI and AsyncAPI documents |
| W903 | an operation or a channel of a published language whose document has no `security` | `sakai check` |
| W904 | a secret the platform keeps in the history of a run: an input, an output, an argument, an answer | `dandori check` |
| E906 | a secret sent outside the project: to a model's provider, Jev, a host named by its URL alone, an AWS service | `dandori check` |
| E905, W905 | a secret sent to a file outside the map, or to a context the map does not relate to the one that marked it; or, with a map that does not pass sakai's check, where it goes cannot be decided | `ritsu check` |
| W910 | an input of a gate's action that a policy reads and that the contract of the operation it guards marks secret: the value goes into the request Cedar is asked, and stays in the record of the decision | `sekisho check` |
| E907, W907 | an operation a context opens (`open host service`) that no action of a gate guards; a context none of whose operations is guarded yet | `ritsu check`, in a project with a gate |
| E908, W908, W909 | a call of a workflow its gate allows in no combination; an action the workflow is allowed and never calls; a call that can be denied, or whether it is allowed cannot be decided, with no error declared for a denial | `ritsu check` |

A value is secret where a contract marks it (`debug_redact` in a `.proto`; `x-data-classification`,
`x-sensitive-data` or `format: password` in an OpenAPI schema) or where a `.flow` writes `secret`
after its type, and dandori follows it through every variable it goes into. Here a variant of the
example `crates/dandori/examples/payout`, from dandori's tests, has the model that drafts the
seller's notice read the name the bank account is held in:

```console
$ cd crates/dandori/tests/fixtures/security
$ dandori check E906_payout_holder.flow
…
error[E906]: E906_payout_holder.flow:41:1: the task `draft_notice` sends the secret `account.holder` to OpenAI, outside the project
    41 |   let notice = draft_notice(amount: amount, payout_id: paid.payoutId, holder: account.holder)
  = The mark is `debug_redact = true` at ../../../examples/payout/specs/payout.proto:35.
  = Send a reference or only what the other side needs. If sending it there is intended, write `discloses holder "<why>"` under the task.
…
```

What is meant is written beside what it is about, and the check takes it at its word:
`ritsu: test secret` in a comment on the line of a key for tests, `plaintext "<why>"` where a
`.flow` writes the URL, `x-ritsu-plaintext` in a server of a document, `security: []` on an
operation open to anyone, `history encrypted` under a workflow whose history is encrypted with a
key you hold (the Temporal code dandori generates for it then requires a payload codec), and
`discloses <parameter> "<why>"` under a task that is meant to send what it is given.

## Pages for people

The same files give pages, in English or Japanese, for the people who need to understand what the
code is to carry out — those who run the business, finance and legal, operators, the developers
reviewing the code — so they can read what was written and check it against what they know. A
team that signs off on its rules can sign off on these pages too.

- `rulec doc` — a rule's tables, provisos and what was proved of them, in Markdown or HTML;
- `dandori doc` — a workflow drawn with every scenario it runs;
- `koyomi doc` — a calendar, month by month;
- `chobo doc` — a ledger, its bounds and its transfers;
- `sekisho doc` — who may do what, action by action, beside the Cedar it compiles to;
- `yuen trace` — a requirement back to its source and on to what satisfies it;
- `explain <code>` — every diagnostic, with a reproduction you can run.

## Proofs

rulec's certificates are rechecked in Lean 4. Lean models of chobo, koyomi, dandori's core and
ritsu's checks across languages sit beside the Rust, and the two are run on the same generated
inputs — millions of lines — and compared answer by answer. No proof rests on `sorry` or an
axiom of its own.

## For AI agents

ritsu and its languages are made to be used by AI agents, and [skills/](skills) holds nine
[Agent Skills](https://agentskills.io) for them: [skills/ritsu](skills/ritsu) for a project of more
than one language (the loop from `ritsu check` to a run and a package, how to read each diagnostic
across the languages and fix it, and which language's skill to read for the rest), and one for each
of the eight languages, in `skills/<language>/`. [skills/README.md](skills/README.md) lists them.
There are four ways to install them:

- **Claude Code**: ritsu's site publishes a plugin marketplace whose plugin `ritsu` holds the
  nine. Run `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json`, then
  `/plugin install ritsu@ritsu`; Claude Code fetches the folder `skills/` alone, not the whole
  repository.
- **Any agent, from the binary**: `ritsu skills install` writes them into the project's
  `.claude/skills/`; with `--user`, into `~/.claude/skills/`; with `--dir <dir>`, where another
  agent reads skills. Names after it (`ritsu skills install rulec dandori`) write only those, and
  `ritsu skills list` lists them.
- **By hand**: copy the folders you need from `skills/` into `~/.claude/skills/`, or into a
  project's `.claude/skills/`.
- **From a release**: `ritsu-skills-v<version>.zip` holds the nine folders; unzip it where your
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

A release archive holds `ritsu` and eight links to it — `rulec`, `dandori`, `koyomi`, `chobo`,
`geas`, `yuen`, `sakai`, `sekisho` — and each runs as its language. `cargo install` installs the `ritsu`
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
  [sakai](crates/sakai/README.md), [sekisho](crates/sekisho/README.md)
- `crates/ritsu-*` — the shared base, units, ports, project reading, the checks across
  languages, the `.proto` reader, the emitters, the browser build
- `proofs/` — the Lean models
- `website/` — the site: ritsu's pages and the playground, with rulec's site in `website/rulec` and dandori's in `website/dandori`
- `DESIGN.md`, `PLAN.md` — the design of the whole (in Japanese)
- `SECURITY.md` — how to report a vulnerability

## License

MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Two of the
languages hold data from others: rulec the names of its built-in divisions, from Unicode CLDR
(Unicode-3.0), and koyomi a Shift_JIS table made from the WHATWG Encoding Standard
(BSD-3-Clause). So the `ritsu` binary is under
`(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause`. It also holds eight crates from
crates.io, under their own licenses, and, for the examples of `explain`, copies of an article of
Japan's Civil Code from e-Gov (PDL1.0) and of two sections of the US Code of Federal Regulations.
[THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) gives where each comes from and the text of its
license or its terms, and every archive and package of a release carries it.
