# dandori

A small typed language for workflows that call business rules. A workflow books a hotel stay,
reserves the lines of an order, answers a customer's inquiry: it calls APIs and rules, waits,
retries, and drives things like a Stripe PaymentIntent from state to state.

- **Checked before it runs.** Types, every arm of every match, every state a payment or an order
  can be left in when the workflow ends, retries that could repeat a change on the other side, the
  service of a `.proto` the workflow implements, and how long a run's history can grow on the
  platform it is built for.
- **Built for five platforms.** Temporal (TypeScript, Python or Go), AWS Step Functions (ASL with
  JSONata), AWS Lambda durable functions, Argo Workflows and pydantic-graph. What each of them
  runs is played against one reference interpreter, on every scenario the tests generate.
- **Decisions come from outside the workflow.** A `.flow` branches only on what a rule or a task
  answered: an API, an agent (OpenAI's models, Claude, or any Open Responses endpoint) whose
  answer comes back in a declared type, TypeSafe's Jev, which answers typed questions with how sure
  it is, your own code, a person's approval. A decision that must
  have no gaps can be a table in [rulec](https://github.com/i2y/ritsu/tree/main/crates/rulec), which proves it complete
  and free of overlaps, and a rule's state machine can be the type of the thing a workflow drives.
- **Due dates and stock come from outside too.** A due date can be a date of koyomi's, checked on
  every day of its range, and stock a book of chobo's, whose bounds hold in every write. A workflow
  calls a date as it calls a rule, holds, posts and voids stock as tasks, and follows a hold as a
  case whose expiry the checker counts.

**Documentation: <https://i2y.github.io/ritsu/dandori/>**, in English and Japanese. The pages are also
readable here, in [website/dandori/docs](../../website/dandori/docs) and
[website/dandori/docs-ja](../../website/dandori/docs-ja).

**Try it in the browser: <https://i2y.github.io/ritsu/dandori/playground/>**. The checker, the builds
and `dandori doc`, compiled to wasm32 and run in the page, on the examples or on a flow you edit,
with the rules each calls; nothing is sent anywhere.

The name comes from 段取り (dandori), arranging the steps of a job beforehand.

## What the checker says

In a hotel booking that authorizes a card and captures at check-out
([tests/fixtures/hotel_naive.flow](tests/fixtures/hotel_naive.flow), a first draft of the example):

```text
error[E020]: tests/fixtures/hotel_naive.flow:95:1: the workflow can end here with the case `pi` in requires_payment_method, processing, which is not final (succeeded, canceled are)
    95 |   succeed outcome = stayed
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_capture
      90  match pi.status: requires_capture
      90  wait until booking.check_out
      93  capture_intent: pi requires_capture → processing
          `settle` happens on the other side: pi processing → requires_payment_method
      95  succeed
```

The transitions come from `payment_intent.rule`, a transcription of Stripe's documents into a
rulec state machine. The workflow says which events happen on their own
(`external authenticate, settle, expire`), and the checker follows them too: waiting until
check-out, the authorization can expire, and then the capture is refused. Every diagnostic comes
with a run that gets there; [Diagnostics](https://i2y.github.io/ritsu/dandori/reference/codes/) lists
all 31 codes.

## A workflow

From the hotel booking, as written for Temporal
([examples/hotel/temporal](examples/hotel/temporal/hotel.flow)):

```flow
case pi : PaymentIntent follows payment_intent.payment
  held capture_method = manual
  held confirmation_method = automatic
  external authenticate, settle, expire
  refused when refused = true

flow
  let quote = hold(room: booking.room, nights: booking.nights)
  match quote.handling
    review => succeed outcome = awaiting_review
    auto => pi <- create_intent(amount: quote.amount, currency: "jpy", payment_method: booking.card, capture_method: manual)
  pi <- confirm_intent(intent: pi.id)
    on card_declined => pi <- get_intent(intent: pi.id)
  …
```

A `.flow` has no comparison or arithmetic. It branches only by matching an enum, a bool, or a
value that may be absent, which a rule or a task answered. A task says how it is called,
the errors it comes back with, how it is retried, whether it takes an idempotency `key`, and what
it does to a case (`starts`, `sends`, `observes`). What a task takes and answers is written as
records and enums, or taken from the description of the API it calls: a message of a `.proto` is a
type as it is (`warehouse.ReserveResponse`), and the task is held to the same description. The
workflow's own entry can be a service of a `.proto`, from which clients in other languages are made
(`workflow fulfillment v1 implements shop.FulfillmentService`); the checker holds the workflow to
it. Every loop has a bound, so a run's history has one too.
[Write a workflow](https://i2y.github.io/ritsu/dandori/tour/) reads the whole example.

## Install

dandori is one of the languages of [ritsu](https://github.com/i2y/ritsu), and is built from its
repository with a recent stable Rust:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu dandori <command>` runs dandori with the rules a workflow uses (`use rule`) read by rulec, and
its dates files and books (`use dates`, `use book`) read by koyomi and chobo, in the same process.
`rulec gen` writes the code of each rule, and a Connect service for it that a workflow can call
instead; the package `rulec` in place of `ritsu` installs rulec alone, and the package `dandori`
installs the `dandori` command alone: it checks and builds a workflow that uses no rules, dates
files or books as `ritsu dandori` does, and tells you to run one that uses them with
`ritsu dandori`. dandori's one outside dependency is serde_json. A workflow without rules has no
cases, since a case follows a rule's state machine, and its branches can only match what its tasks
answer.

## For AI agents

[skills/dandori](../../skills/dandori) is an [Agent Skill](https://agentskills.io) for using dandori: the
loop from a first draft to a build, the language on one page, what to ask a person, and the fix for
each diagnostic, with the reference pages it needs. Copy it into `~/.claude/skills/`, or into a
project's `.claude/skills/`; [skills/README.md](skills/README.md) says more.

## Commands

```
dandori check <file.flow>...
dandori build <file.flow> --target temporal|temporal-python|temporal-go|asl|durable|argo|pydantic-graph [--out <dir>]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph]
dandori doc <file.flow> [--format html] [--out <dir>]
dandori explain <CODE> | --all [--format markdown|json]
dandori <command> --help
dandori --version
```

For a workflow that uses rules, dates files or books, run each as `ritsu dandori <command>`; then
`build`, `run`, `scenarios` and `doc` also check, when the workflow runs, a rule's precondition that
`ritsu check` cannot decide where the flow calls the rule (its W201): as soon as the values are
made, a run whose values break it fails with `Dandori.BrokenPrecondition`. `dandori <command> --help`
prints what one command takes, and what its exit codes mean; `dandori explain E014` says when a
diagnostic comes, how to fix it, and the smallest `.flow` that gets it. `--lang ja`
prints the messages in Japanese (else `DANDORI_LANG`, then `RITSU_LANG`). `build` refuses what its platform cannot do (E050),
and a workflow whose one run can outgrow the platform (E040). `doc` draws the workflow for the
person who reviews it: Mermaid charts that GitHub draws in a pull request, with tables of every
call and every way the workflow can end, or one HTML page on which each scenario lights up the way
its run goes ([the hotel booking, drawn](https://i2y.github.io/ritsu/dandori/doc/hotel.html)). The rules
it calls come with it, each as the page for people that `rulec doc` renders.

| Target | What `build` writes |
|---|---|
| `temporal` | the workflow, its activities, a worker and a client, in TypeScript |
| `temporal-python` | the same in Python, named alike, so a worker in one language can serve another |
| `temporal-go` | the same in Go, as one package, named alike too |
| `asl` | the state machine, in ASL with JSONata, and a Lambda handler for every rule and date it calls by Lambda and every book it runs operations on |
| `durable` | a Lambda durable function in TypeScript |
| `argo` | a WorkflowTemplate, and the caller image that makes its calls |
| `pydantic-graph` | a graph that runs in your own Python process |

[Build for a platform](https://i2y.github.io/ritsu/dandori/platforms/) has what each of them writes, and
[What a task calls](https://i2y.github.io/ritsu/dandori/tasks/) what a task becomes on each.

## Examples

[examples/](examples/) has six. Five are each written for Temporal, for AWS and for pydantic-graph: a
hotel booking held to Stripe's OpenAPI document; an order in a warehouse's system, whose AWS version
calls a rule at the rule's own Connect service; the fulfillment of an order, with a child flow, which
implements a service of a `.proto` and takes the types of the warehouse's answers from the
warehouse's `.proto`; an inquiry sorted by Jev and read and answered by agents; and an application
scored by Jev and, when a rule says so, approved by a person. The sixth, an invoice, is written once
for every platform: it holds an order's goods in a book of chobo's until the payment is due by a date
of koyomi's, then ships them once paid or puts them back
([Dates and books](https://i2y.github.io/ritsu/dandori/dates-and-books/)). Every example has a Japanese
twin beside it (`hotel.ja.flow`), with Japanese names everywhere but where an API description fixes
them.
[Examples](https://i2y.github.io/ritsu/dandori/examples/) says how the versions differ.

## How it is checked

The reference interpreter defines what a `.flow` means. The tests generate the scenarios of every
example and run each of them nine ways: in the reference interpreter, the ASL under JSONata 2.0.6
and on LocalStack's Step Functions, the Temporal workflow in TypeScript, in Python and in Go on the
Temporal CLI's dev server, the durable function in the SDK's local test runner, the
WorkflowTemplate on Argo Workflows in a kind cluster, and the graph with pydantic-graph. Each must
make the same calls, with the same arguments and idempotency keys, and end the same way. The page
on the site that runs dandori in the browser must answer every example as the command does. The
operations of books that the runs make also go through the code dandori writes to chobo's clients,
on PostgreSQL and TigerBeetle, and must answer what chobo's reference interpreter answers.
[How it is checked](https://i2y.github.io/ritsu/dandori/assurance/) tells the rest, and how to run the
tests.

Most of the flows and fixtures under `tests/` have Japanese names, on purpose: they see that
names outside ASCII come through all five platforms as identifiers, keys and URL paths.

## Status

Early. Not yet: Parallel with different branches, OpenAPI documents in YAML, types made from an
OpenAPI document or a Smithy model (a `.proto` makes them), protobuf's binary encoding and Connect's
streams, the clients of a service a workflow implements written, for the languages dandori does not
build for, by a plugin of protoc, cases the workflow holds itself, a rule that walks a list of elements, runs on AWS and on a production Temporal cluster or Temporal
Cloud (the tests run on the Temporal CLI's dev server), the caller image run against real Lambda,
HTTP and AWS endpoints from Argo, agents run against OpenAI and Anthropic themselves, and the checks
that cross into koyomi and chobo: a hold's expiry against the waits before it is posted, and the days
a workflow passes against a dates file's range. The
design, the decisions and what is left are in [DESIGN.md](DESIGN.md), in Japanese; its principles
are on [Design](https://i2y.github.io/ritsu/dandori/design/).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option. The cut-down copies of Stripe's OpenAPI document and
of the Smithy models of Amazon SNS and SQS under `examples/` keep their own licenses
([THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
