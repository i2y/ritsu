---
title: "Write the workflow. Check it. Build it."
hide:
  - navigation
  - toc
---

<div class="dd-hero" markdown>
<img class="dd-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="dd-hero__mark" src="images/mark-light.svg#only-light" alt="">

# dandori

<p class="dd-hero__tag">Write the workflow. Check it. Build it.</p>

<p class="dd-hero__lede">
<strong>A small typed language for workflows that call business rules.</strong> A workflow books
a hotel stay, reserves the lines of an order, answers a customer's inquiry: it calls APIs and
rules, waits, retries, and drives things like a Stripe PaymentIntent from state to state.
</p>

<p class="dd-hero__lede">
<strong>Checked before it runs.</strong> Types, every arm of every match, every state a payment or
an order can be left in when the workflow ends, retries that could repeat a change on the other
side, and how long a run's history can grow on the platform it is built for.
</p>

<p class="dd-hero__lede">
<strong>Built for five platforms.</strong> Temporal (TypeScript or Python), AWS Step Functions,
AWS Lambda durable functions, Argo Workflows and pydantic-graph. What each of them runs is played
against one reference interpreter, on every scenario the tests generate.
</p>

<div class="dd-hero__cta" markdown>
[Install](install.md){ .md-button .md-button--primary }
[Write a workflow](tour.md){ .md-button }
[Examples](examples.md){ .md-button }
[GitHub](https://github.com/i2y/dandori){ .md-button }
</div>
</div>

<div class="dd-overview" markdown>
![A .flow and the rulec rules it calls go into the checker, which looks at the types and every arm of every match, every state a case can be left in, retries that could repeat a change, the history's size on each platform, ranges, child .flows and API descriptions, and what each platform can do. The reference interpreter gives the one meaning, and scenarios take every arm and every error. The build writes for Temporal (the main platform), AWS Step Functions, Lambda durable functions, Argo Workflows and pydantic-graph, and each is run on every scenario and held to the reference](images/overview.svg#only-dark)
![A .flow and the rulec rules it calls go into the checker, which looks at the types and every arm of every match, every state a case can be left in, retries that could repeat a change, the history's size on each platform, ranges, child .flows and API descriptions, and what each platform can do. The reference interpreter gives the one meaning, and scenarios take every arm and every error. The build writes for Temporal (the main platform), AWS Step Functions, Lambda durable functions, Argo Workflows and pydantic-graph, and each is run on every scenario and held to the reference](images/overview-light.svg#only-light)
</div>

---

## What dandori does

<div class="dd-row" markdown>
<div markdown>

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
    auto => pi <- create_intent(amount: quote.amount, …)
  pi <- confirm_intent(intent: pi.id)
    on card_declined => pi <- get_intent(intent: pi.id)
```

</div>
<div markdown>

### Decisions come from outside the workflow

A `.flow` has no comparison and no arithmetic of its own. It branches only by matching an enum, a
bool, or a value that may be absent, which a rule or a task answered: an API, an agent, your own
code, a person's approval. A decision that must have no gaps can be written in
[rulec](https://github.com/i2y/rulec), as a table that rulec proves complete and free of overlaps,
and a rule's state machine becomes the type of what the workflow drives, here Stripe's
PaymentIntent.

</div>
</div>

<div class="dd-row dd-row--flip" markdown>
<div markdown>

<div class="dd-term" markdown>

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

</div>

</div>
<div markdown>

### The checker follows every way it can go

A first draft of the hotel booking waits until check-out and then captures the payment. The
checker follows the transitions of Stripe's PaymentIntent, including the ones that happen on
Stripe's side without the workflow's asking, and finds a run that ends with the payment neither
settled nor released. Each diagnostic comes with the run that gets there.
[What it checks](checks.md)

</div>
</div>

<div class="dd-row" markdown>
<div markdown>

```text
dandori build hotel.flow --target temporal
dandori build hotel.flow --target temporal-python
dandori build hotel.flow --target asl
dandori build hotel.flow --target durable
dandori build hotel.flow --target argo
dandori build hotel.flow --target pydantic-graph
```

</div>
<div markdown>

### One workflow, built for the platform you run

Temporal is the main platform: dandori writes the workflow, the activities that make its HTTP, AWS
and agent calls, the worker and the client, in TypeScript or in Python. The same `.flow` also builds
for AWS Step Functions, Lambda durable functions, Argo Workflows and pydantic-graph, and the code
dandori writes sends the same requests on each. A build refuses what its platform cannot do.
[Build for a platform](platforms.md)

</div>
</div>

<div class="dd-row dd-row--flip" markdown>
<div markdown>

```flow
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, …"
  model "gpt-oss:20b"
  effort low
  url "http://ollama.internal:11434/v1"
  timeout 60 seconds
  retry 2 times every 10 seconds
```

</div>
<div markdown>

### Agents read, write and choose

A task can be an agent: a model that gets the task's arguments and gives back a value of the
task's type, checked like any other answer. The flow can match that answer, or hand it to a rule
to decide. OpenAI's models, Claude, and any Open Responses endpoint (Ollama, vLLM, LM Studio,
OpenRouter, …) can be called.
[Agents](agents.md)

</div>
</div>

<div class="dd-row" markdown>
<div markdown>

```text
dandori doc hotel.flow > hotel.md
dandori doc hotel.flow --format html > hotel.html
```

</div>
<div markdown>

### Drawn for the person who reviews it

`dandori doc` draws a workflow: every call, match, wait and loop, with what each call does, where
its errors go, what a case can be after it, and every way the workflow can end. As Markdown, it is a
Mermaid flowchart that GitHub draws in a pull request; as one HTML page, each scenario lights up the
way its run goes. [The hotel booking, drawn](doc/hotel.html) · [Draw a workflow](diagrams.md)

</div>
</div>

## Status

Early. Not yet: Parallel with different branches, OpenAPI documents in YAML, protobuf's binary
encoding and Connect's streams, cases the workflow holds itself, a rule's preconditions checked at
the task that produced the value, runs on AWS and on a production Temporal cluster or Temporal
Cloud, the caller image run against real Lambda, HTTP and AWS endpoints from Argo, and agents run
against OpenAI and Anthropic themselves. The design, the decisions and what is left are in
[DESIGN.md](https://github.com/i2y/dandori/blob/main/DESIGN.md), in Japanese; its principles are
on [Design](design.md). dandori is licensed under either of the Apache License 2.0 or the MIT
license, at your option.
