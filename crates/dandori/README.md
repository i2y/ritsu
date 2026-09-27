# dandori

A small typed language for workflows that call business rules. A workflow books a hotel stay,
reserves the lines of an order, answers a customer's inquiry: it calls APIs and rules, waits,
retries, and drives things like a Stripe PaymentIntent from state to state.

- **Checked before it runs.** Types, every arm of every match, every state a payment or an order
  can be left in when the workflow ends, retries that could repeat a change on the other side,
  and how long a run's history can grow on the platform it is built for.
- **Built for five platforms.** Temporal (TypeScript or Python), AWS Step Functions (ASL with
  JSONata), AWS Lambda durable functions, Argo Workflows and pydantic-graph. What each of them
  runs is played against one reference interpreter, on every scenario the tests generate.
- **Decisions come from outside the workflow.** A `.flow` branches only on what a rule or a task
  answered: an API, an agent (OpenAI's models, Claude, or any Open Responses endpoint) whose
  answer comes back in a declared type, your own code, a person's approval. A decision that must
  have no gaps can be a table in [rulec](https://github.com/i2y/rulec), which proves it complete
  and free of overlaps, and a rule's state machine can be the type of the thing a workflow drives.

**Documentation: <https://i2y.github.io/dandori/>**, in English and Japanese. The pages are also
readable here, in [website/docs](website/docs) and [website/docs-ja](website/docs-ja).

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
with a run that gets there; [Diagnostics](https://i2y.github.io/dandori/reference/codes/) lists
all 28 codes.

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
it does to a case (`starts`, `sends`, `observes`). Every loop has a bound, so a run's history has
one too. [Write a workflow](https://i2y.github.io/dandori/tour/) reads the whole example.

## Install

```console
$ git clone https://github.com/i2y/dandori
$ cd dandori
$ cargo install --path .
```

dandori builds with a recent stable Rust, and its one dependency is serde_json.

rulec is needed only for a workflow that uses rules (`use rule`): dandori reads them through rulec,
found through `DANDORI_RULEC`, else on the PATH, and `rulec gen` writes the code of each rule. Install
it with `brew install i2y/tap/rulec`, or take a binary from its
[releases](https://github.com/i2y/rulec/releases); dandori is tested with rulec 0.20.0 and 0.21.1.
A workflow without rules is checked and built without rulec, but its branches can only match what
its tasks answer, and it has no cases, since a case follows a rule's state machine.

## For AI agents

[skills/dandori](skills/dandori) is an [Agent Skill](https://agentskills.io) for using dandori: the
loop from a first draft to a build, the language on one page, what to ask a person, and the fix for
each diagnostic, with the reference pages it needs. Copy it into `~/.claude/skills/`, or into a
project's `.claude/skills/`; [skills/README.md](skills/README.md) says more.

## Commands

```
dandori check <file.flow>...
dandori build <file.flow> --target temporal|temporal-python|asl|durable|argo|pydantic-graph [--out <dir>]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|durable|argo|pydantic-graph]
```

`--lang ja` prints the messages in Japanese. `build` refuses what its platform cannot do (E050),
and a workflow whose one run can outgrow the platform (E040).

| Target | What `build` writes |
|---|---|
| `temporal` | the workflow, its activities, a worker and a client, in TypeScript |
| `temporal-python` | the same in Python, named alike, so a worker in one language can serve the other |
| `asl` | the state machine, in ASL with JSONata, and a Lambda handler for every rule it calls |
| `durable` | a Lambda durable function in TypeScript |
| `argo` | a WorkflowTemplate, and the caller image that makes its calls |
| `pydantic-graph` | a graph that runs in your own Python process |

[Build for a platform](https://i2y.github.io/dandori/platforms/) has what each of them writes, and
[What a task calls](https://i2y.github.io/dandori/tasks/) what a task becomes on each.

## Examples

[examples/](examples/) has five, each written for Temporal, for AWS and for pydantic-graph: a hotel
booking held to Stripe's OpenAPI document, an order in a warehouse's system, the fulfillment of an
order with a child flow, agents that read an inquiry and draft the reply, and an application
scored by other workers and approved by a person. [Examples](https://i2y.github.io/dandori/examples/)
says how the versions differ.

## How it is checked

The reference interpreter defines what a `.flow` means. The tests generate the scenarios of every
example and run each of them eight ways: in the reference interpreter, the ASL under JSONata 2.0.6
and on LocalStack's Step Functions, the Temporal workflow in TypeScript and in Python on the
Temporal CLI's dev server, the durable function in the SDK's local test runner, the
WorkflowTemplate on Argo Workflows in a kind cluster, and the graph with pydantic-graph. Each must
make the same calls, with the same arguments and idempotency keys, and end the same way.
[How it is checked](https://i2y.github.io/dandori/assurance/) tells the rest, and how to run the
tests.

Most of the flows, rules and fixtures under `tests/` have Japanese names, on purpose: they see that
names outside ASCII come through all five platforms as identifiers, keys and URL paths.

## Status

Early. Not yet: Parallel with different branches, OpenAPI documents in YAML, protobuf's binary
encoding and Connect's streams, cases the workflow holds itself, a rule's preconditions checked at
the task that produced the value, runs on AWS and on a production Temporal cluster or Temporal
Cloud (the tests run on the Temporal CLI's dev server), the caller image run against real Lambda,
HTTP and AWS endpoints from Argo, and agents run against OpenAI and Anthropic themselves. The
design, the decisions and what is left are in [DESIGN.md](DESIGN.md), in Japanese; its principles
are on [Design](https://i2y.github.io/dandori/design/).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option. The cut-down copies of Stripe's OpenAPI document and
of the Smithy models of Amazon SNS and SQS under `examples/` keep their own licenses
([THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
