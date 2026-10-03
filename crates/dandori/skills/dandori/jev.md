# Jev

[Jev](https://docs.typesafe.ai/introduction) is TypeSafe AI's model for decisions. It writes no
text: it answers typed questions about what it is given, each with how sure it is. A `jev` task asks
it what the task's answer type asks, in one request, and gets back a value of that type. The flow
matches the answer like any other, or gives it to a rule. An answer that is not sure enough fails
the call with an error the task names, and the flow handles it where it calls.

The inquiry example asks Jev for the kind of an inquiry, and when Jev is not sure, takes the kind an
agent read along with the rest
([examples/inquiry](https://github.com/i2y/dandori/blob/main/examples/inquiry/temporal/inquiry.flow)):

```flow
task pick_kind(text: string) -> routing.kind
  jev "Which kind of inquiry is this?"
    returns "The customer wants to send an item back or exchange it"
    delivery "A parcel is late, lost or damaged, or the customer asks where it is"
    billing "A charge, an invoice, a payment or a refund"
    other "None of the above"
  model "jev-1.13.0"
  confidence 0.8 else unsure
  timeout 10 seconds
  retry 2 times every 1 second

flow
  …
  let kind = pick_kind(text: inquiry.text)
    on unsure, failure => let kind = reading.kind
  let decision = routing(kind: kind, member: inquiry.member)
```

## The answer's type is the question

| The task answers | Jev is asked | The answer is |
|---|---|---|
| an enum, with `jev "<question>"` | a choice among the enum's values; under it, what each value means (a value left out goes with none) | the value Jev chooses |
| an enum, with `jev score "<question>"` | a score: the values are the levels, listed from the lowest, each with what it means (2 to 10 of them) | the level nearest where Jev puts it, a half going up |
| `bool`, with `jev "<question>"` | yes or no; under it, what `true` and `false` mean, both or neither | yes when the probability of yes is over one half |
| a record, with `jev` alone | a question for each field, each as above, all in one request | a record of the answers |

The arguments go to Jev as its state, one field for each parameter. Jev tells the values apart by
their names and what each means, and the levels of a score by what each means alone, so write each
as a situation Jev can recognize, not a degree.

A field of a record can take how sure Jev is of another field's answer, as a rate. The review example
has Jev place an application on a scale, and hands how sure it is to a rule
([examples/review](https://github.com/i2y/dandori/blob/main/examples/review/temporal/review.flow)):

```flow
record Score
  verdict : policy.verdict
  sure    : rate[step 1%]  range >=0 <=100

task score(purpose: string) -> Score
  jev
    verdict score "How clearly is the money for running the business?"
      reject "It is for something personal or speculative, or against the law"
      hold "It is for the business, but the statement leaves unclear what it pays for"
      approve "It pays for a named part of running the business, such as stock, equipment, staff or premises"
    sure confidence of verdict
  model "jev-1.13.0"
```

`rate[step 1%]` counts 1% steps, rounded down: Jev 0.87 sure is 87. That is how a rulec rate
travels, so the value goes to a rule as it is.

Jev writes no text, so an answer of another type is refused
([tests/fixtures/jev.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/jev.flow)):

```text
error[E007]: tests/fixtures/jev.flow:38:3: Jev answers a choice among an enum's values, a place on a scale of them (`score`), yes or no (`bool`), or a record of such answers; it writes no text; the answer is `string`
    38 |   jev "要点は何か"
```

## How sure is sure enough

A choice and a score come with how sure Jev is (`confidence`, from 0 to 1, which TypeSafe works out
from how the probabilities spread). A yes or no has no such number, and dandori takes Jev to be as
sure of the answer as the probability of that answer: 0.9 for yes is a yes, 0.9 sure, and 0.3 for
yes is a no, 0.7 sure.

`confidence 0.8 else unsure` fails a call whose answers are not all that sure with the error
`unsure`, which the clause declares. The flow handles it where it calls, like any declared error,
and the checker holds the flow to that. Asked again, Jev answers much the same, so `retry` does not
take the error.

When how sure is enough depends on what the answer would do, give the confidence to a rule and let
its table decide. In the review example, the rule approves at once from 90%, rejects at once from
80%, and sends everything else to a person; rulec proves the table leaves no case out.

```flow
  let r = score(purpose: application.purpose)
    on failure => fail Unscorable "Could not score application {application.id}"
  let d = policy(verdict: r.verdict, sure: r.sure)
  match d.decision
    approve, reject => pass
    ask =>
```

```text
table act
policy unique
| verdict | sure  | -> decision |
| approve | >=90% | approve     |
| approve | <90%  | ask         |
| reject  | >=80% | reject      |
| reject  | <80%  | ask         |
| hold    | -     | ask         |
```

How sure one version of Jev is means something else to the next, so a task that sets a confidence or
reads one names the version it was set for. `jev-latest` and `jev-preview` are aliases, which move
to a new version by themselves, and the checker warns of them:

```text
warning[W032]: tests/fixtures/jev.flow:192:3: `jev-latest` is an alias, which moves to a new version of Jev without a change here, and how sure one version is means something else to the next; name the version the confidence is set for, as `model "jev-1.13.0"`
   192 |   model "jev-latest"
```

## Who is called

Every platform sends the same request: a POST to `https://api.typesafe.ai/v1/systemone` with the
state, the model and the questions, and every platform reads the answer the same way.

- Step Functions sends it from an HTTP Task, with TypeSafe's API key in the EventBridge connection the
  task names (`connection`), as the header `Authorization: Bearer <key>`. An HTTP Task gives a request
  60 seconds at most, so a longer `timeout` cannot be built (E050).
- The code dandori writes for the other platforms sends it through the `Transport`, which adds the key
  from `TYPESAFE_API_KEY`, or from its `typesafe` option.

TypeSafe's API says an error by its HTTP status: 429 at the rate limit, 529 when it is overloaded.
Declare the ones to handle or retry by name (`errors busy = 429, overloaded = 529`); any other is
`failure`. An answer that is not there or not of its kind, such as a value the enum does not have or
a choice without its confidence, does not fit the type, and ends the run as any such answer does.

## Before you rely on it

- Jev is in early access (September 2026), and its API may still change.
- TypeSafe says Jev is most accurate in English. Other languages, Japanese among them, are handled,
  but less well; try it on your own text. In one try (jev-1.13.0, 29 September 2026), Jev put clear
  inquiries in their kind at 0.99 or more in both languages. "The shoes don't fit, and I want my money
  back" came back as `returns`, 0.96 sure, but its Japanese as `billing`, 0.36 sure (returns 0.48),
  which `confidence 0.8` sends to the agent's reading.
- Jev is weak at arithmetic, counting and comparing dates. Leave those to your code or to a rule, and
  ask Jev for the judgement.
- The tests answer Jev's calls with a stand-in, in the shape of TypeSafe's API reference, and check
  the default `Transport`'s requests on this machine. With `TYPESAFE_API_KEY` set, they also send each
  Jev task once to TypeSafe for real ([How it is checked](https://i2y.github.io/dandori/assurance/)).
