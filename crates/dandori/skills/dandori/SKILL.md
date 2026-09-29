---
name: dandori
description: Write, check and build dandori workflows (`.flow` files), typed workflows that call APIs, rules, agents, TypeSafe's Jev and code of your own, checked before they run and built for Temporal (TypeScript or Python), AWS Step Functions, AWS Lambda durable functions, Argo Workflows and pydantic-graph. Use when a workflow (take a payment now and capture it later, reserve and ship an order, route an inquiry, wait for a person's approval) has to be written or changed as a `.flow`; when a dandori diagnostic (E001-E050, W030, W032, W101-W104) has to be fixed; when a workflow has to be shown to the person who reviews it, drawn; or when a `.flow` has to be built for a platform and its generated code wired up.
compatibility: Requires the `dandori` binary on PATH (`cargo install --path .` in a clone of https://github.com/i2y/dandori). A workflow that uses rules (`use rule`) also needs `rulec` (`brew install i2y/tap/rulec`).
license: MIT OR Apache-2.0
---

## When this applies

The job is a **workflow**: steps that call other systems, wait, retry, and move something in
another system from state to state. A payment authorized when a stay is booked and captured at
check-out; the lines of an order reserved side by side, packed and shipped; an inquiry read,
routed and answered; an application scored and, when the score says so, approved by a person.
dandori writes it once as a `.flow`, checks it before it runs, and builds it for the platform the
team runs.

It does not apply to the decisions themselves when they are tables: a fee schedule, an
eligibility test, a routing policy. Those are rulec rules, which a `.flow` calls (rulec has a
skill of its own). Nor does it apply to computing things: a `.flow` has no comparison and no
arithmetic, so whatever compares or computes is a rule or a task.

The files bundled with this skill are listed in §7. Read them when you need them, not all up
front.

---

# Working with dandori

Your part is to write the `.flow`, get it past `dandori check`, and build it. Two things stay with
people:

- **what the other systems guarantee**: whether a call can be repeated safely, which errors it
  returns, which events happen on their side by themselves. That comes from their documentation
  or their owners, and §3 says when to ask.
- **the platform and the code around the workflow**: which platform runs it, the credentials, and
  the code of the tasks that are yours to write.

Everything is reachable from the command line: `dandori --help` lists the commands, and
`dandori check --format json` gives the diagnostics as data (§5). There is no step where you have
to read dandori's source.

## 1. The loop

1. **Find the workflow.** List what it calls, what it waits for, what it returns, and the things
   in other systems whose state it moves (a PaymentIntent, an order). Settle the platform (§6);
   Temporal is the main one.
2. **Write the `.flow`** (§2): the types, the tasks and the cases first, then the flow.
3. **Check it:** `dandori check <file.flow>`. Every diagnostic comes with the run that gets there;
   fix it (§4) and check again, until it prints `<file.flow>: ok`. A warning does not stop a
   build, but it says something true about the workflow.
4. **Watch it run:** `dandori scenarios <file.flow> --out <dir>` writes scenarios (`001.json`,
   `002.json`, …) that together take every arm, every handler of an error, and every way a case
   can move. `dandori run <file.flow> --scenario <dir>/001.json` plays one in the reference
   interpreter and prints each call with its answer, and how the run ended.
5. **Show it to a person:** `dandori doc <file.flow> > <file>.md` draws the flow as Mermaid
   charts, with a table of every call (what it calls, its retries, where each of its errors goes)
   and of every way the workflow can end, with the state each case is left in, and the rules it
   calls as `rulec doc` renders them. Put it in the pull request, where GitHub draws the charts, for
   the person who reviews the workflow. `--format html` writes one page on which each scenario
   lights up the way its run goes, and each rule's page opens with a case to try on it.
   [diagrams.md](diagrams.md) says what is on the picture.
6. **Build it:** `dandori build <file.flow> --target temporal --out <dir>`. A build refuses what its
   platform cannot do (E050) and a run that can outgrow the platform (E040).
7. **Wire it up:** write the tasks that are yours (the `OwnTasks` the generated code asks for), give
   the `Transport` (`io.ts`, `io.py`) its credentials, and start the generated worker, or deploy what
   the target wrote. When the
   workflow calls rules, `rulec gen <rule> --out rulec` writes the code the generated wrappers
   import. [platforms.md](platforms.md) says what each target writes.

## 2. The language on one page

A whole workflow: the review example. Jev scores an application and says how sure it is, a rulec
rule weighs the verdict and how sure it is, and a person approves what the rule sends on.

```flow
workflow review v1
description "Jev scores an application and says how sure it is, and a rule decides whether the verdict is acted on at once or goes to a person for approval: approving at once asks more certainty than rejecting at once. Written for Temporal: the scoring is an activity dandori writes, which calls Jev with TypeSafe's key (TYPESAFE_API_KEY) on the workers of its own task queue, which may be written in another language, and so does the notice, an activity you write; the rule runs in the worker of the workflow; the approver's tool answers the callback by the update the generated client sends"

use rule policy from "../rules/review_policy.rule"

record Application
  id      : string
  amount  : int
  purpose : string

record Score
  verdict : policy.verdict
  sure    : rate[step 1%]  range >=0 <=100

record Approval
  approver : string

inputs
  application : Application

outputs
  verdict : policy.verdict

# Jev places what the applicant wrote on the scale of verdicts, from the lowest level, and how
# sure it is of the verdict comes as a rate, for the rule to weigh.
task score(purpose: string) -> Score
  jev
    verdict score "How clearly is the money for running the business?"
      reject "It is for something personal or speculative, or against the law"
      hold "It is for the business, but the statement leaves unclear what it pays for"
      approve "It pays for a named part of running the business, such as stock, equipment, staff or premises"
    sure confidence of verdict
  model "jev-1.13.0"
  queue "scoring"
  errors busy = 429, overloaded = 529
  retry 2 times every 10 seconds on busy, overloaded
  timeout 10 seconds

# Someone approves in a tool of their own, and the answer comes back with the callback's id.
task ask_for_approval(application_id: string, amount: int, sure: rate[step 1%]) -> Approval
  callback
  timeout 3 days

task notify(application_id: string, text: string)
  queue "notify"
  idempotent

flow
  let r = score(purpose: application.purpose)
    on failure => fail Unscorable "Could not score application {application.id}"
  let d = policy(verdict: r.verdict, sure: r.sure)
  match d.decision
    approve, reject => pass
    ask =>
      let a = ask_for_approval(application_id: application.id, amount: application.amount, sure: r.sure)
        on timeout => fail NoAnswer "No approval in three days"
      notify(application_id: application.id, text: "{a.approver} approved application {application.id}")
      succeed verdict = approve
  notify(application_id: application.id, text: "Application {application.id} was reviewed: {r.verdict}")
  succeed verdict = r.verdict
```

Indentation makes the blocks. `#` starts a comment.

### Declarations

```text
workflow <name> v<N>                      the name and version (on Temporal, both are in the
                                          workflow type and the task queue: review_v1)
description "<text>"
kind standard | express                   Step Functions; an Express workflow has limits (E031)
use rule <name> from "<file.rule>"        a rulec rule, read through rulec; under it,
                                          lambda "<function>" (Step Functions, durable functions)
                                          and local (a local activity on Temporal)
use openapi|smithy|proto <name> from "<file>"   an API description; under it, url "<base>"
                                          (a proto needs one)
enum <Name> = <value> | <value> | …
record <Name>                             then one field a line, <name> : <type>
inputs, outputs                           the same, for what a run takes and gives back
task <name>(<param>: <type>, …) [-> <type>]
case <name> : <Record> follows <rule>.<machine>
flow                                      the steps
on failure                                runs when a task fails and nothing handled it, then the
                                          run fails with the same error
on cancel                                 runs when the workflow is cancelled (Temporal only)
```

The types are `int`, `string`, `bool`, `timestamp`, `json` (passed along without being looked
into), `list[T]`, `T?` (a value that may be absent), numbers with a unit as rulec has them
(`money[JPY, incl_tax]`, `duration[h]`), a rule's enums and records (`hold.room`), and the file's
own. A number can say what it may be: `nights : int  range >=1 <=30`, either end may be left out,
and the ends are integers in the type's unit.

### Tasks

A task is one call. Under it, one clause a line:

```text
how it is called: one of these, or none for a task you write
  http GET|POST|PUT|PATCH|DELETE "<url>" [form]      http POST <api> "<path>"  (an OpenAPI operation)
  connect <api> "<Service>/<Method>"                 aws <service>:<action>
  lambda "<function>"                                agent "<instructions>"  |  agent claude "<instructions>"
  jev "<question>"  |  jev score "<question>"  |  jev   (TypeSafe's Jev; see below)
  event                                              a value sent to the workflow by name (Temporal)
a child workflow
  flow "<child.flow>"          a .flow of its own, held to this task (E015); Step Functions and
                               durable functions also need state machine "<arn>" / durable function "<arn>"
  workflow "<type>"  state machine "<arn>"  durable function "<arn>"  workflow template "<name>"
                               a child that is not a .flow: Temporal, Step Functions, durable functions, Argo
what it does to a case
  starts <rule>.<machine> [then <event>, …]          sends [<column> =] <event>       observes
errors and retries
  errors <name> [= <status | exception | code>], …    refused as <error>
  retry <n> times [every <duration>] [backoff <x>] [on <error>, …]
  timeout <duration>       key [<parameter>]          idempotent
the rest
  callback                 queue "<task queue>"       image "<image>" (Argo, a task you write)
  connection "<EventBridge connection>"               model "<model>"     url "<base>" (agents)
  effort none|minimal|low|medium|high|xhigh|max       how hard an agent's model reasons (Claude: low and up)
  confidence <0 to 1> else <error>                    a Jev answer less sure than this fails with <error>
```

A `jev` task asks what its answer type asks, and writes the meanings under the `jev` line: an enum
is a choice (`<value> "<what it means>"` for some or all values), `jev score` a scale of the enum's
values from the lowest (every one with its meaning, 2 to 10), `bool` a yes or no (`true "…"` and
`false "…"`, or neither), and a record `jev` alone with a line a field (`<field> "<question>"`,
`<field> score "<question>"`, or `<field> confidence of <field>` for a `rate[step <n>%]` field that
takes how sure Jev is). Pin the model's version (`model "jev-1.13.0"`) when a confidence is used
(W032); a threshold that depends on the action belongs in a rule's table, fed the confidence.

A duration is `10 seconds`, `1 minute`, `2 hours`, `3 days`. The hotel booking's tasks, held to
Stripe's OpenAPI document:

```flow
task create_intent(amount: money[JPY, incl_tax] range >=50 <=99999999, currency: string, payment_method: string, capture_method: payment_intent.capture_method) -> PaymentIntent
  http POST stripe "/v1/payment_intents"
  starts payment_intent.payment then attach
  key
  retry 2 times every 2 seconds

task confirm_intent(intent: string) -> PaymentIntent
  http POST stripe "/v1/payment_intents/{intent}/confirm"
  sends confirm
  errors card_declined = 402, unexpected_state = 400
  refused as unexpected_state
  key

task get_intent(intent: string) -> PaymentIntent
  http GET stripe "/v1/payment_intents/{intent}"
  observes
  idempotent
  retry 3 times every 2 seconds
```

`retry` repeats failures and timeouts, but not the errors the task declares; `retry … on <name>, …`
repeats only what it names, which may be `failure` or `timeout` too. A task that changes something and is retried needs `key` (an idempotency key dandori makes from
the run and the place of the call), or `idempotent` when doing it twice is the same as once.
[tasks.md](tasks.md) has every way of calling and what each becomes on every platform,
[agents.md](agents.md) the agents, and [jev.md](jev.md) Jev.

### Cases

A case is something in another system whose state a workflow moves, following a rulec state
machine. It needs rulec.

```flow
case pi : PaymentIntent follows payment_intent.payment
  held capture_method = manual
  held confirmation_method = automatic
  external authenticate, settle, expire
  refused when refused = true
```

`held` fixes a value the machine's table reads, for this case. `external` names the events that
happen on the other system's side by themselves; the checker follows them too. `refused when`
says which output of the machine means an event was refused. `state <field>` names the record's
field that holds the state, when more than one field has the state's type. When the workflow
ends, every case it started must be in a final state (E020), unless `fail … leaving <case>` hands
it over.

### The flow

```text
let x = <task or rule>(…)          call, and keep the answer
let x = <expression>               build a value
<case> <- <task>(…)                call a task on a case, and take the case back
<task>(…)                          call, and drop the answer
  on <error> => …                  under a call: a declared error; also on failure =>, on timeout =>
match <value>                      arms a, b => …   none => …   some v => …   true => …   false => …
wait <duration>                    wait until <timestamp>
repeat at most <n> times           break leaves it
for v in <list> at most <n> [in parallel[, <k> at a time]]
let xs = for v in <list> at most <n>     with yield <value> as the last line of the body
succeed [<output> = <value>, …]    fail <Name> ["<text>"] [leaving <case>, …]
pass
```

Every value a `match` can meet needs an arm, and there is no default arm (E010): a value no arm
names fails the run with `Dandori.UnexpectedValue`. The expressions are names and their fields
(`booking.room`), literals, records (`{order_id: order.id, reservations: results}`), lists,
`none`, and strings with values put in (`"Order {order.id}"`); nothing compares, adds or combines
conditions. The rounds of a parallel `for` keep their own variables; when one fails, the others
still run to their end, and the first failure in the list's order decides. From the fulfillment
of an order:

```flow
  let results = for line in order.lines at most 50 in parallel, 10 at a time
    let r = reserve_stock(sku: line.sku, quantity: line.quantity)
    yield r
  …
  let packed = wait_for_packing(QueueUrl: "https://sqs.ap-northeast-1.amazonaws.com/123456789012/packing", MessageBody: {order_id: order.id, reservations: results})
    on timeout => fail PackingLate "No word of the packing in two days"
```

[tour.md](tour.md) reads the hotel example from the first line to the last.

## 3. What stays with a person

Ask instead of guessing:

- **Whether a call changes something.** `idempotent` silences W030 and E030; written on a call
  that is not, a retry can charge a card twice. Whether an API takes an idempotency key, and in
  which parameter (`key <parameter>`), is the API's to say.
- **The errors a call comes back with**, and as what: an HTTP status, an AWS exception, a Connect
  code. When the API has a description, read it with `use openapi`, `use smithy` or `use proto`,
  and the checker holds the task to it (E016).
- **The events that happen by themselves** on a case (`external`). They come from the other
  system's documentation; leaving one out hides the runs it causes.
- **What becomes of a case left unfinished**: settled in `on failure` or `on cancel`, or handed
  over to people with `fail … leaving`. Both are business decisions.
- **The numbers**: timeouts, retry counts, loop bounds (which also bound the history, E040),
  ranges.
- **A decision that must have no gaps.** A `.flow` branches on what a rule or a task answered: an
  API, an agent, Jev, your own code, a person. When the decision is a policy (a fee, an eligibility,
  a routing, how sure is enough), write it as a rulec rule, where rulec proves it complete, rather
  than leaving it to an agent.

Ask with the run the checker gives, in the reader's terms: "If the card is declined, the workflow
fails with the PaymentIntent still requires_payment_method. Should it cancel the PaymentIntent
first, or hand it to staff as it is?" The page `dandori doc --format html` writes lights that run
up on the picture, when a person would rather see it.

## 4. From a diagnostic to a fix

A diagnostic names the code, the place and what is wrong, and then the run that gets there:

```text
error[E020]: tests/fixtures/hotel_naive.flow:91:1: the workflow can fail here with the case `pi` in requires_payment_method, which is not final (succeeded, canceled are); settle it first, or write `leaving pi` to hand it over as it is
    91 |     requires_payment_method => fail CardDeclined "The card was declined"
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_payment_method
      91  match pi.status: requires_payment_method
      91  fail CardDeclined
```

| Code | What it finds | The usual fix |
|---|---|---|
| E001 to E006 | syntax, names, types, arguments, rules rulec could not read, a name declared twice | what the message says |
| E007 | the clauses of a task that do not go together | the message names the clause |
| E008 | a case declared wrongly, or a task doing to a case what it cannot | follow the machine's table |
| E009 | a statement where it cannot be | `yield` last in its `for`; no `break`, `succeed` or call on a case inside a parallel round |
| E010 | a value no arm of a `match` takes | add the arm; there is no default arm, on purpose |
| E011 | an arm no value reaches | remove it |
| E012 | a variable read where it may not have a value yet | give it a value on every way there, or read it later |
| E013 | a task on a case not started yet, or a case started twice | start it first, once |
| E014, W104 | a value that can be outside a range, or whose range is unknown | say the range where the value comes from (the input, the task's answer) |
| E015 | a task that does not fit the child `.flow` it runs | match the child's inputs, outputs and `fail`s |
| E016 | a task that does not fit its API's description | follow it: `T?` for what the answer may leave out, every value of an enum |
| E020 | a case the workflow can leave in a state that is not final | settle it before the end, or `fail … leaving <case>` |
| E021, E022 | an event refused in every state; a refusal no handler takes | do not send it there; handle the `refused as` error |
| E030, W030 | a call that changes something, retried without `key` | `key`, or `idempotent` if twice is the same as once |
| E031 | what an Express workflow cannot do | `kind standard`, or less |
| E040 | a run that can outgrow the platform's history | lower the loop bounds; on Temporal, a loop at the top of the flow goes on in a new run |
| E050 | what the platform lacks or cannot do | give it what it needs (`connection`, `image`, a way of calling), or another platform |
| W101 | a failure that can leave a case unfinished | handle the error at the call, or settle the case in `on failure` |
| W102 | an `on <refusal>` that cannot happen | remove it |
| W103 | a task that starts a case without `key` | add `key` |

[codes.md](codes.md) has the whole list, and [checks.md](checks.md) what the checker looks at.

## 5. For a machine

- `dandori check --format json <file.flow>…` prints one JSON object per file, one after another:
  `{"file": …, "diagnostics": [{"code", "severity", "line", "col", "message", "notes", "path"}]}`,
  where `path` is the run that gets there, as `[{"line", "step"}]`.
- The exit code is 0 when there is no error (warnings may be), 1 when there is, and 2 for bad
  arguments or a file that cannot be read.
- `--lang ja`, or `DANDORI_LANG=ja`, gives the messages in Japanese.
- `DANDORI_RULEC` names the rulec binary; it is run only for a workflow that uses rules.

## 6. Platforms

| Target | What `build` writes | What to know |
|---|---|---|
| `temporal` | the workflow, activities, a worker and a client, in TypeScript | the main platform; the only one with `on cancel` and `event` |
| `temporal-python` | the same with Temporal's Python SDK | named as in TypeScript, so a worker in one language can serve the other |
| `asl` | an ASL state machine with JSONata, and a Lambda handler for every rule | no code of your own (every task needs a way of calling); `http`, `agent` and `jev` need `connection` |
| `durable` | a Lambda durable function in TypeScript | a task of your own is a step |
| `argo` | a WorkflowTemplate and the caller image | a task of your own is a container of its `image` |
| `pydantic-graph` | a graph in Python | runs in your own process, and keeps nothing when it stops |

[platforms.md](platforms.md) has the details, and [examples.md](examples.md) the five examples, each
written for Temporal, for AWS and for pydantic-graph.

## 7. The files bundled with this skill

| File | What is in it |
|---|---|
| [tour.md](tour.md) | the language, through the hotel booking from its first line to its last |
| [tasks.md](tasks.md) | every way a task can call, what each becomes on each platform, API descriptions, child flows |
| [agents.md](agents.md) | agent tasks: typed answers, OpenAI, Open Responses, Claude |
| [jev.md](jev.md) | Jev tasks: the answer type as the question, confidence, rates for a rule |
| [checks.md](checks.md) | what the checker looks at, with a diagnostic |
| [codes.md](codes.md) | every diagnostic code and what it finds |
| [diagrams.md](diagrams.md) | `dandori doc`: the workflow drawn for the person who reviews it |
| [commands.md](commands.md) | the commands, the flags, the exit codes |
| [platforms.md](platforms.md) | what each target writes and how it runs |
| [examples.md](examples.md) | the five examples and how their versions differ |
| [design.md](design.md) | the six principles the language keeps |

They are copies of the pages of <https://i2y.github.io/dandori/>, which has them in Japanese too.
