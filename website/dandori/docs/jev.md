# Decision models

A decision model writes no text: it answers typed questions about what it is given, each with how
sure it is. A `jev` task asks one what the task's answer type asks, in one request, and gets back a
value of that type. The flow matches the answer like any other, or gives it to a rule. An answer that
is not sure enough fails the call with an error the task names, and the flow handles it where it
calls.

A `jev` task asks one of three kinds of server:

| The task says | It asks | at | with the key |
|---|---|---|---|
| `jev "<question>"` | [Jev](https://docs.typesafe.ai/introduction), TypeSafe AI's model for decisions, by its System One API | `https://api.typesafe.ai/v1/systemone` | TypeSafe's, from `TYPESAFE_API_KEY` |
| `jev "<question>"` and `url "<base URL>"` | another server of the System One API, such as [Ollama](https://docs.ollama.com/api/systemone) (0.35 and later) with a decision model of its own | `<base URL>/systemone` | none: a server that wants one gets it from the `Transport`'s headers |
| `jev openai "<question>"` | [OpenAI's Decisions API](https://developers.openai.com/api/docs/guides/decisions) | `https://api.openai.com/v1/decisions` | OpenAI's, from `OPENAI_API_KEY` |
| `jev openai "<question>"` and `url "<base URL>"` | another server of the Decisions API, such as a gateway | `<base URL>/decisions` | none, as above |

The base URL ends where the API's path begins, as an agent's `url` does (`http://localhost:11434/v1`
for Ollama). `jev typesafe "…"` is the same as `jev "…"`. Every platform sends the same request to
the same place, and reads the answer the same way.

The AWS version of the inquiry example asks Jev for the kind of an inquiry, and when Jev is not sure,
takes the kind an agent read along with the rest
([examples/inquiry](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/inquiry/aws/inquiry.flow)):

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

Its Temporal version asks the same question of a decision model on the company's own server of the
System One API (in this example, Ollama), which wants no key
([examples/inquiry](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/inquiry/temporal/inquiry.flow)):

```flow
task pick_kind(text: string) -> routing.kind
  jev "Which kind of inquiry is this?"
    …
  model "nimble:9b-q4_K_M"
  url "http://ollama.internal:11434/v1"
  plaintext "The model server is reached only inside the cluster network, which the service mesh encrypts"
  confidence 0.8 else unsure
```

## The answer's type is the question

| The task answers | The model is asked | The answer is |
|---|---|---|
| an enum, with `jev "<question>"` | a choice among the enum's values; under it, what each value means (a value left out goes with none) | the value the model chooses |
| an enum, with `jev score "<question>"` | a score: the values are the levels, listed from the lowest, each with what it means (2 to 10 of them on TypeSafe's API) | the level nearest where the model puts it, a half going up |
| `bool`, with `jev "<question>"` | yes or no; under it, what `true` and `false` mean, both or neither (the Decisions API takes no meanings: say them in the question) | yes when the probability of yes is over one half |
| a record, with `jev` alone | a question for each field, each as above, all in one request | a record of the answers |

The arguments go to the model as what it reads, one field for each parameter: as the System One API's
state, or as the JSON text of the Decisions API's input, the text an agent gets. The model tells the
values apart by their names and what each means, and the levels of a score by what each means, so
write each as a situation the model can recognize, not a degree. On the Decisions API, a score's
level is the enum's value as its label, with what it means as its description.

A field of a record can take how sure the model is of another field's answer, as a rate. The review
example has Jev place an application on a scale, and hands how sure it is to a rule
([examples/review](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/review/temporal/review.flow)):

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

`rate[step 1%]` counts 1% steps, rounded down: 0.87 sure is 87. That is how a rulec rate travels, so
the value goes to a rule as it is.

A decision model writes no text, so an answer of another type is refused
([tests/fixtures/jev.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/tests/fixtures/jev.flow)):

<div class="dd-term" markdown>

```text
error[E007]: tests/fixtures/jev.flow:38:3: Jev answers a choice among an enum's values, a place on a scale of them (`score`), yes or no (`bool`), or a record of such answers; it writes no text; the answer is `string`
    38 |   jev "要点は何か"
```

</div>

## When the model will not answer

The Decisions API may refuse a question: the answer to it is `{"type": "refusal", "name": …}`, while
the other questions of the request may still be answered. `refusal else <error>` fails the call with
the error, which the clause declares, and the flow handles it where it calls. Without the clause,
the call fails with `Dandori.Refused`, which `on failure` takes. A refusal is read before how sure the
answers are, and asked again, the same input is mostly refused again, so no retry takes it
([tests/flows/decisions.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/tests/flows/decisions.flow)):

```flow
task read(text: string) -> Reading
  jev openai
    kind "Which kind of inquiry is this?"
      returns "The customer wants to send an item back or exchange it"
      billing "A charge, an invoice, a payment or a refund"
    urgency score "How urgent is it for the customer?"
      low "The customer does not say it is urgent"
      medium "The customer names a date, but not a near one"
      high "The customer asks for an answer today"
    person "Does the customer ask to talk to a person?"
    sure confidence of kind
  model "gpt-6-luna"
  confidence 0.6 else unsure
  refusal else declined
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/openai/9c0d1e2f"

flow
  …
  let r = read(text: text)
    on declined => fail Declined "The model would not read the inquiry"
```

The System One API answers every question, so `refusal` there is an error:

<div class="dd-term" markdown>

```text
error[E007]: tests/fixtures/decisions.flow:18:16: the System One API (TypeSafe's Jev, and the servers `url` names) answers every question and refuses none; `refusal` is for a `jev openai` task
    18 |   refusal else declined
```

</div>

## How sure is sure enough

A choice and a score come with how sure the model is (`confidence`, from 0 to 1). A yes or no has no
such number, and dandori takes the model to be as sure of the answer as the probability of that
answer: 0.9 for yes is a yes, 0.9 sure, and 0.3 for yes is a no, 0.7 sure.

`confidence 0.8 else unsure` fails a call whose answers are not all that sure with the error
`unsure`, which the clause declares. The flow handles it where it calls, like any declared error,
and the checker holds the flow to that. Asked again, the model answers much the same, so `retry` does
not take the error.

How sure a model says it is means something of its own on each server. TypeSafe works it out from how
the probabilities spread; Ollama's is how much one candidate stands out from the others, which its
documentation says is not how likely the answer is to be right; OpenAI does not say how it works its
number out. The same inquiry, asked the same question of the four kinds of the inquiry example on 9
October 2026, came back this way:

| The inquiry | Ollama, `tev1:0.8b` | OpenAI, `gpt-6-luna` |
|---|---|---|
| "My parcel arrived broken and I want my money back." | `returns` 0.59, `billing` 0.31; 0.30 sure | `delivery` 0.51, `billing` 0.45; 0.35 sure |
| 「靴のサイズが合わないので、代金を返してほしいです」 | `returns` 0.68; 0.48 sure | `billing` 0.85; 0.80 sure |

So a floor of how sure belongs to a server and a model. When how sure is enough depends on what the
answer would do, give the confidence to a rule and let its table decide; when the task asks another
server or model, set the table again against real answers. In the review example, the rule approves
at once from 90%, rejects at once from 80%, and sends everything else to a person; rulec proves the
table leaves no case out.

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

For the same reason, a task that sets a confidence or reads one names the model it was set for, and
the checker warns of a name that moves to another model by itself: an alias of Jev (`jev-latest`,
`jev-preview`), a name without a tag or with `:latest` on a server of the System One API (as Ollama
names models), and any model of the Decisions API, which names no version of its model.

<div class="dd-term" markdown>

```text
warning[W032]: tests/fixtures/jev.flow:192:3: `jev-latest` is an alias, which moves to a new version of Jev without a change here, and how sure one version is means something else to the next; name the version the confidence is set for, as `model "jev-1.13.0"`
   192 |   model "jev-latest"
```

```text
warning[W032]: tests/fixtures/decisions.flow:108:3: `nimble` has no tag, so it moves to a newer model when the server pulls the model again, with no change here (as Ollama names models, it is `:latest`); how sure one model is means something else to the next, so name the model the confidence is set for with its tag, as `model "tev1:0.8b"`
   108 |   model "nimble"
```

```text
warning[W032]: tests/fixtures/decisions.flow:120:3: OpenAI's Decisions API names no version of its model (`gpt-6-luna`), so how sure an answer is may come to mean something else when the model changes, with no change here; check the floor of how sure, and the fields that take it, against real answers from time to time
   120 |   model "gpt-6-luna"
```

</div>

## Who is called

Every platform sends the same request to the task's server, and reads the answer the same way.

- Step Functions sends it from an HTTP Task, with the key in the EventBridge connection the task names
  (`connection`), as the header `Authorization: Bearer <key>`; a server that wants no key still needs
  a connection, which holds some value. An HTTP Task calls HTTPS APIs only, and gives a request 60
  seconds at most, so a server at an `http://` URL, or a longer `timeout`, cannot be built (E050).
- The code dandori writes for the other platforms sends it through the `Transport`, which adds
  TypeSafe's key from `TYPESAFE_API_KEY` (or its `typesafe` option), and OpenAI's from `OPENAI_API_KEY`
  (or its `openai` option). It sends neither key to a server `url` names.
- A server at `http://` on another host is warned of (W902), as an agent's is; `plaintext "<why>"`
  under the task says it is meant. A secret given to a task goes outside the project when the task
  asks TypeSafe, OpenAI, or a server `url` names on another host (E906).

The APIs say an error by its HTTP status: 429 at the rate limit, and on TypeSafe's, 529 when it is
overloaded. Declare the ones to handle or retry by name (`errors busy = 429, overloaded = 529`); any
other is `failure`. An answer that is not there or not of its kind, such as a value the enum does not
have or a choice without its confidence, does not fit the type, and ends the run as any such answer
does.

## Before you rely on it

- Jev is in early access (September 2026), and OpenAI's Decisions API is in public beta (October
  2026); both APIs may still change. OpenAI's API has one model, `gpt-6-luna`, and its documentation
  gives neither its limits nor its errors.
- TypeSafe says Jev is most accurate in English. Other languages, Japanese among them, are handled,
  but less well; try any decision model on your own text. In one try (jev-1.13.0, 29 September 2026),
  Jev put clear inquiries in their kind at 0.99 or more in both languages. "The shoes don't fit, and
  I want my money back" came back as `returns`, 0.96 sure, but its Japanese as `billing`, 0.36 sure
  (returns 0.48), which `confidence 0.8` sends to the agent's reading.
- Decision models are weak at arithmetic, counting and comparing dates. Leave those to your code or
  to a rule, and ask the model for the judgement.
- Images are not sent: both APIs take them (OpenAI's in a user's message, Ollama's for its Clef
  models), but a task's arguments are text and values.
- The tests answer every call with a stand-in in the shape of each API's reference, and check the
  default `Transport`'s requests on this machine. They also send each task once for real: to TypeSafe
  with `TYPESAFE_API_KEY` set, to OpenAI with `OPENAI_API_KEY` set, and to the Ollama on this machine
  when it has a decision model ([How it is checked](assurance.md)).
