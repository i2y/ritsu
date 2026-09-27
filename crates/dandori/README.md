# dandori

A small typed language for workflows that call business rules. A `.flow` is checked
before it runs — types, every arm of every match, every state a case can be left in,
retries that could repeat a change, how long the execution history can grow on the platform
it is built for — and
compiled to **AWS Step Functions** (ASL with JSONata), to **Temporal** (TypeScript or
Python), to **AWS Lambda durable functions** (TypeScript), to **Argo Workflows** (a
WorkflowTemplate) and to **pydantic-graph** (a graph that runs in the process that calls it).

The decisions themselves are written in [rulec](https://github.com/i2y/rulec): tables
that rulec proves complete and free of overlaps. dandori reads them through rulec's
command line, and uses a rule's state machine as the type of the thing a workflow
drives — a Stripe PaymentIntent, an order in a warehouse. Reading and writing — pulling
the fields out of a customer's message, drafting a reply — can go to an agent, OpenAI's or
Claude, whose answer comes back in a declared type ([Agents](#agents)).

The name comes from 段取り (dandori), arranging the steps of a job beforehand.

## What the checker says

In a hotel booking that authorizes a card and captures at check-out ([tests/fixtures/hotel_naive.flow](tests/fixtures/hotel_naive.flow), a first draft of the example):

```
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

The transitions come from `payment_intent.rule`, a transcription of Stripe's documents
into a rulec state machine. The workflow says which events happen on their own
(`external authenticate, settle, expire`), and the checker follows them too: waiting
until check-out, the authorization can expire, and then the capture is refused.

## A workflow

From the hotel booking, as written for Temporal ([examples/hotel/temporal](examples/hotel/temporal/hotel.flow)):

```
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

Lists, values that may be absent, and anything a task can call, from the fulfillment of an order
as written for AWS ([examples/fulfillment/aws](examples/fulfillment/aws/fulfillment.flow)):

```
task wait_for_packing(QueueUrl: string, MessageBody: PackingRequest) -> Packing
  aws sqs:sendMessage
  callback
  timeout 2 days

flow
  let results = for line in order.lines at most 50 in parallel, 10 at a time
    let r = reserve_stock(sku: line.sku, quantity: line.quantity)
    yield r
  …
  match order.gift
    some gift => let recipient = gift.recipient
    none => pass
  let packed = wait_for_packing(QueueUrl: "https://sqs.…/packing", MessageBody: {order_id: order.id, reservations: results})
    on timeout => fail PackingLate "No word of the packing in two days"
  notify(TopicArn: "arn:aws:sns:…:orders", Message: "Order {order.id} went out by {decision.carrier} (…)")
```

- Conditions and arithmetic live in rulec rules. A `.flow` builds values — records,
  lists, strings with values put in — but has no comparison or arithmetic, and branches
  only by matching an enum, a bool, or a value that may be absent (`none` / `some x`).
- Types: `int`, units such as `money[JPY, incl_tax]`, `string`, `bool`, `timestamp`, enums,
  records, `list[T]`, `T?` for a value that may be absent, and `json` for a value that is
  passed along without being looked into. A number can say what it may be
  ([Ranges](#ranges)).
- A task declares its errors, its retries, whether it is idempotent or takes an
  idempotency `key`, and what it does to a case: `starts`, `sends <event>`, `observes`.
- Loops have a bound (`repeat at most 12 times`, `for x in xs at most 50`), so the history
  size has one. The rounds of `for … in parallel` run at the same time; each keeps its
  own variables, and when a round fails, the others still run to their end and the first
  failure in the list's order decides.
- `fail … leaving pi` hands an unfinished case over on purpose; `on failure` settles
  cases when a task fails and nothing handled it, and `on cancel` when the workflow is
  cancelled (on Temporal, which asks a workflow to stop and lets it clean up; the other
  platforms stop a run at once, and refuse `on cancel` with E050). The checker enters
  `on cancel` from every call and wait a cancellation can stop, and wants the cases settled
  there too.

### Ranges

A number can say what it may be, as a rulec rule's inputs do: `nights : int  range >=1 <=30`.
A range goes on an input, an output, a field of a record, and a task's parameter or answer
(`-> int  range >=0 <=10`); either end may be left out. The ends are whole numbers in the
type's own unit, written without it (`>=0`, not `>=0JPY`), as the values travel in JSON.

- What comes in is checked when the workflow runs. An input, or a task's or a rule's answer,
  with a number outside its range fails the run with `Dandori.BadInput` or
  `Dandori.BadResponse`, as a value of the wrong type does, on every platform. A rule's answer
  is held to the range rulec gives it, in case the function that runs the rule is not the
  version the workflow was checked with. An agent's answer schema carries the range:
  `minimum` and `maximum` for OpenAI, and words in the description for Claude, whose
  structured outputs take neither.
- What goes out is checked before the workflow runs. A value given to a rule's input, a task's
  parameter, a field of a record written out, or an output must fit the range there (E014).
  A value whose range nothing says is a warning (W104), which a range where the value comes
  from ends: that range is then checked as the value comes in.

In a draft of the hotel booking whose stays run longer than the rule for the hold takes
([tests/fixtures/hotel_ranges.flow](tests/fixtures/hotel_ranges.flow)):

```
error[E014]: tests/fixtures/hotel_ranges.flow:20:1: `booking.nights` can be outside `>=1 <=30`, the range of `nights` of the rule `hold`: it is `>=1 <=60`
    20 |   let quote = hold(room: booking.room, nights: booking.nights)
warning[W104]: tests/fixtures/hotel_ranges.flow:21:1: nothing says what range `extension` is in (the input `extension` has no range), and `nights` of the rule `hold` takes `>=1 <=30`
    21 |   let longer = hold(room: booking.room, nights: extension)
```

A variable's range is that of every value put in it, anywhere in the flow. A `.flow` has no
arithmetic, so a range travels as it is, from where the value comes to where it goes. On
Temporal, adding a range or narrowing one changes what a running workflow does when its values
fall outside, so it ships as a new version (`v2`) or through Worker Deployment Versioning.

### Child flows

A task whose child workflow is written in dandori too says where its `.flow` is:
`flow "arrange_delivery.flow"`. The checker checks the child on its own and holds the task to it
(E015): the task passes every input the child needs, of a type and in a range the input takes;
its answer is a record whose fields are the child's outputs; and every error it declares is one
the child fails with. The two files declare their records and enums each on their own, so the
types are compared by shape: a record by its fields, an enum by its values, a range by what it
holds, in the direction the values go.

In a draft of the fulfillment whose delivery task does not fit its child
([tests/fixtures/fulfillment_child.flow](tests/fixtures/fulfillment_child.flow)):

```
error[E015]: tests/fixtures/fulfillment_child.flow:18:1: the parameter `carrier` is not what `arrange_delivery` takes as its input `carrier`: `drone` of `Carrier` is not a value of `carrier`
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
error[E015]: tests/fixtures/fulfillment_child.flow:18:1: `arrange_delivery` answers `tracking_number` with what the field `tracking_number` of `Delivery` does not take: `string` is not `int`
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
```

The child's names come from its `.flow`: on Temporal the task starts the child's workflow type
(`arrange_delivery_v1`) on its task queue (the same name, where the child's generated worker
polls), and on Argo a Workflow of the child's WorkflowTemplate. Step Functions and Lambda
durable functions still need the ARN the child is deployed as (`state machine`, `durable
function`). A flow that runs itself, directly or through others, is refused: there is no
recursion.

### API descriptions

When the API a task calls is described, the `.flow` reads the description and the checker holds
the task to it (E016):

```
use openapi stripe from "specs/stripe.json"
use smithy sns from "specs/sns.json"
use proto warehouse from "specs/warehouse.proto"
  url "https://warehouse.example.com"

task confirm_intent(intent: string) -> PaymentIntent
  http POST stripe "/v1/payment_intents/{intent}/confirm"

task notify(TopicArn: string, Message: string) -> Sent
  aws sns:publish
  errors no_recipient = NotFoundException

task reserve_stock(sku: string, quantity: int) -> Reservation
  connect warehouse "StockService/Reserve"
  errors busy = resource_exhausted
```

- An OpenAPI 3 document (JSON) gives an `http` task its operation: the URL is the document's
  server and the path, and the document says whether the body is URL-encoded.
- An AWS API's Smithy model (the JSON AST AWS publishes) is used by the service's name, so
  `aws sns:publish` is held to the model of SNS's `Publish`.
- A `.proto` (proto3) gives `connect` its method, called by the Connect protocol with JSON: a
  POST to `<url>/<package>.<Service>/<Method>` with `Connect-Protocol-Version: 1`. A Connect
  error is declared by its code (`not_found`) and told apart by the code's HTTP status.

The task passes no parameter the operation does not take and every one it requires, of a type
and in a range it takes; it reads the answer into a type the answer fits, with `T?` for a field
the answer may leave out or send as null, and an enum with every value the answer may have; and
its errors are ones the operation answers with. An AWS API's `key <parameter>` must be the
operation's idempotency token.

In a draft of calls to Stripe and to a stock service
([tests/fixtures/api_calls.flow](tests/fixtures/api_calls.flow)):

```
error[E016]: tests/fixtures/api_calls.flow:23:1: what `stripe` POST /v1/payment_intents answers is not `PaymentIntent`: in `created`, an integer is not `timestamp`
    23 | task create_intent(amount: int, currency: string, capture_method: CaptureMethod) -> PaymentIntent
error[E016]: tests/fixtures/api_calls.flow:26:1: what `warehouse` StockService/Reserve answers is not `Reservation`: in `count`, a 64-bit integer comes as a string in protobuf's JSON; declare it `string`
    26 | task reserve_stock(sku: string, quantity: int) -> Reservation
```

protobuf's JSON leaves out a field without presence when it holds its zero value (an empty
string, 0, false, an enum's first value, an empty list or map), and whoever reads it with
protobuf reads the zero value there. A `connect` task's answer is read the same way on every
platform: the code dandori writes fills the zero values in from the `.proto` — in the answer,
in its messages, and in the messages of its lists — before the answer's type is checked. The
checker reads a description only as far as the task's types go, so Stripe's 8 MB document
costs what is looked at; the examples keep cut-down copies ([tools/specs/trim.py](tools/specs/trim.py)).

### What a task calls

| The task says | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| `lambda "<function>"` | Lambda Task | an activity dandori writes, invoking the function | a step dandori writes, invoking the function | a container running the code dandori writes | a function dandori writes, invoking the function |
| `http POST "<url>"` | HTTP Task | an activity dandori writes, with `fetch` | a step dandori writes, with `fetch` | the same, with `fetch` | the same, with urllib |
| `connect <api> "<Service>/<Method>"` ([API descriptions](#api-descriptions)) | HTTP Task, Connect's JSON | an activity dandori writes, with `fetch` | a step dandori writes, with `fetch` | the same, with `fetch` | the same, with urllib |
| `aws sns:publish` | AWS SDK integration | an activity dandori writes, with the AWS SDK | a step dandori writes, with the AWS SDK | the same, with the AWS SDK | the same, with boto3 |
| `agent "<instructions>"` and `model "<model>"` | HTTP Task to OpenAI's Responses API | an activity dandori writes, with OpenAI's Agents SDK | a step dandori writes, with the Agents SDK | the same, with the Agents SDK | the same, with the Agents SDK for Python |
| `agent "<instructions>"`, `model "<model>"` and `url "<server>"` | HTTP Task to that server's Open Responses API | an activity dandori writes, sending the same request over HTTP | a step dandori writes, the same | the same | the same, with urllib |
| `agent claude "<instructions>"` and `model "<model>"` | HTTP Task to Claude's Messages API | an activity dandori writes, with Anthropic's SDK | a step dandori writes, with Anthropic's SDK | the same, with Anthropic's SDK | the same, with Anthropic's SDK for Python |
| `state machine "<arn>"` | nested execution (`startExecution.sync:2`) | | | | |
| `flow "<path>"` ([Child flows](#child-flows)) | nested execution, with `state machine` | child workflow `<name>_v<n>` on the child's task queue | invoke, with `durable function` | a Workflow from the child's WorkflowTemplate | a function you write |
| `workflow "<type>"` | | child workflow | | | |
| `durable function "<arn>"` | | | invoke of another durable function | | |
| `workflow template "<name>"` | | | | a Workflow from that WorkflowTemplate | |
| `image "<image>"` | | | | a container of your image | |
| `event` | cannot build (E050) | nothing is called: the workflow waits for a value sent to it by name | cannot build (E050) | cannot build (E050) | cannot build (E050) |
| none of these | cannot build (E050) | an activity you write (`OwnTasks`) | a step running code you write (`OwnTasks`) | cannot build (E050) | a function you write (`OwnTasks`) |

The code dandori writes for Temporal, durable functions, Argo and pydantic-graph sends what
Step Functions sends, through a `Transport` (`io.ts`, `io.py`) whose credentials and clients
are yours to set. On pydantic-graph, a task that another platform runs as a workflow is a
function you write.
`queue "<name>"` sends a Temporal activity or child workflow to that task queue, and `local`
under `use rule` has Temporal call the rule as a local activity, in the worker that runs the
workflow: a call leaves one marker in the history, not an activity's six events, for a flow
that decides many times in a loop, and the rule ships with that worker. Elsewhere it changes
nothing. A
`callback` task hands on a token (Step Functions), a callback id (durable functions), or
an id the answer comes back with as an Update (Temporal) or through `argo node set` (Argo);
with `aws sqs:sendMessage` the token travels in the message.

A task that says `event` calls nothing: the workflow waits for a value sent to it by its id and
the task's name, as an approval tool or a carrier's webhook would, knowing only the order it is
about (`client.ts`'s `send(client, workflowId, "delivered", { ok: … })`, an Update). The
workflow takes an event only while it waits for it, and refuses any other, so the one who sends
it learns so and sends it again later; the query `dandori.status` says which events it waits
for. Only Temporal can be sent a value by name, so the other platforms refuse the task (E050):
a feature whose meaning a platform cannot keep is built only where it can be, and one that
changes only how a thing is called (`queue`, `image`, `local`) does nothing where it does not
apply.

On Argo Workflows every task runs in a container. The code dandori writes for `lambda`,
`http`, `aws` and `agent` goes into an image built from `caller/`; a task with `image` runs your
image, which reads the call from `DANDORI_CALL`, writes the answer to
`/tmp/dandori/answer.json`, and for a declared error writes `{"error", "message"}` to
`/tmp/dandori/error.json` and exits with 3. The WorkflowTemplate keeps the flow's variables
in global output parameters, and the YAML starts with a comment that names each
variable's parameter.

### Agents

An `agent` task gives a model its arguments and takes back a value of the task's type. The
model reads and writes; the rules decide. [examples/inquiry](examples/inquiry/temporal/inquiry.flow)
reads a customer's message with a model the company runs on its own Ollama, routes it with a
rulec rule, and drafts the reply with Claude:

```
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, take out the order number if one is written, …"
  model "gpt-oss:20b"
  url "http://ollama.internal:11434/v1"
  timeout 60 seconds
  retry 2 times every 10 seconds

task draft_reply(kind: routing.kind, point: string, order_id: string?, within: duration[h]) -> string
  agent claude "Draft the first reply to the inquiry, politely, in three sentences at most. …"
  model "claude-sonnet-5"
  timeout 60 seconds

flow
  let reading = read_inquiry(text: inquiry.text)
    on failure => …
  let decision = routing(kind: reading.kind, member: inquiry.member)
```

- The answer's type becomes a JSON Schema in the strict form of OpenAI's Structured Outputs —
  every field of a record required, `T?` a choice with null, an enum its values — around
  `{"answer": …}`, since the top must be an object. Claude's structured outputs take the same
  schema. The answer is then checked against the type like any other.
- Every target asks the model the same thing: the instructions, the arguments as the same
  JSON text, and the schema, and no model settings — dandori adds none, and keeps the Agents
  SDK from adding its defaults. Step Functions sends it to the Responses API from an HTTP
  Task, with the API key in the EventBridge connection the task names (`connection`, in the
  version for AWS). The code dandori writes for the other targets runs it with
  OpenAI's Agents SDK through the `Transport`, which reads `OPENAI_API_KEY`, or takes a run
  configuration of your own (another model provider, for one).
- An agent that is not Claude's speaks Open Responses: the open specification of OpenAI's
  Responses API, which OpenAI, Hugging Face, OpenRouter, Ollama, vLLM, LM Studio and Vercel
  took up in January 2026. Without `url`, the agent's call goes to OpenAI. With
  `url "<base>"`, it goes to `<base>/responses` on that server — Ollama, vLLM, LM Studio,
  OpenRouter, Hugging Face — as the same request Step Functions sends, which the code dandori
  writes sends over HTTP (`fetch`, urllib) with no SDK, since an SDK may send what the
  specification does not have. The server's credentials come from the `Transport`'s `headers`,
  as an HTTP task's do. Its limits on a schema are its own, so the checker holds the answer
  to OpenAI's only when the call goes to OpenAI; the answer's check tells the rest.
- A Claude agent gets the instructions as the system prompt, the arguments' JSON text as the
  user's message, and the schema as `output_config.format`, with `max_tokens` 16000 (the
  Messages API wants one) — from an HTTP Task on Step Functions, with the key in the connection
  as `x-api-key`, and with Anthropic's SDK elsewhere, which reads `ANTHROPIC_API_KEY`. Claude may
  give an enum's value in another case, so a Claude agent's enum values are taken without regard
  to case; an enum whose values differ only in case cannot be in such an answer (E007).
- Neither SDK's client retries by itself in the default `Transport`: the workflow retries, as
  the task's `retry` says, as Step Functions does.
- An agent changes nothing on the other side, so it takes no `key`, and retrying it is
  always safe. It declares no errors: a refusal, or a call that fails, is `failure`.
- The checker refuses an answer the schema cannot say (`json`, a record that contains itself
  through others) or one larger than the provider takes (E007): for Claude, more than 16 values
  that may be absent. Step Functions refuses an agent without `connection`, an HTTP Task
  whose `timeout` is over the 60 seconds it gives a request, and one that is not sent over
  HTTPS: the HTTP Task calls a server under a public name with a publicly trusted certificate,
  a private one too (E050). A connection always holds a key, so give it one even for a server
  that wants none.

## Examples

Five examples, each written for Temporal, for AWS and for pydantic-graph: the same flow, with its
tasks called and its news brought in the way the platform does. Temporal is dandori's main
platform, and its version is the one to read first.

| Example | For Temporal | For AWS (Step Functions, Lambda durable functions) | For pydantic-graph |
|---|---|---|---|
| a hotel booking that holds a card and captures at check-out, held to Stripe's OpenAPI document | [temporal](examples/hotel/temporal/hotel.flow) | [aws](examples/hotel/aws/hotel.flow) | [pydantic-graph](examples/hotel/pydantic-graph/hotel.flow) |
| an order in a warehouse's system, reminded, shipped, delivered | [temporal](examples/order/temporal/order.flow) | [aws](examples/order/aws/order.flow) | [pydantic-graph](examples/order/pydantic-graph/order.flow) |
| reserving the lines of an order side by side, packing, delivery: the warehouse called by Connect, and the delivery a child flow, [arrange_delivery](examples/fulfillment/arrange_delivery.flow), written once for every platform | [temporal](examples/fulfillment/temporal/fulfillment.flow) | [aws](examples/fulfillment/aws/fulfillment.flow) | [pydantic-graph](examples/fulfillment/pydantic-graph/fulfillment.flow) |
| agents that read an inquiry and draft a reply, and a rule that routes it | [temporal](examples/inquiry/temporal/inquiry.flow) | [aws](examples/inquiry/aws/inquiry.flow) | [pydantic-graph](examples/inquiry/pydantic-graph/inquiry.flow) |
| an application scored by other workers, and a person's approval; also [for Argo Workflows](examples/review/argo/review.flow) | [temporal](examples/review/temporal/review.flow) | [aws](examples/review/aws/review.flow) (Lambda durable functions) | [pydantic-graph](examples/review/pydantic-graph/review.flow) |

- **For Temporal**, a call to an HTTP API is an activity dandori writes (the `Transport` adds
  the credentials, so there is no `connection`), and the rest are activities you write. News
  from outside the workflow comes as an `event`, sent to the workflow by its id (Stripe's
  webhook in hotel, the carrier in order); a request that is answered later stays a `callback`,
  answered by the Update the generated client sends. Hotel and order release what they hold
  when the workflow is cancelled (`on cancel`); fulfillment and review send work to other task
  queues, and fulfillment falls back to the standard carrier when its child flow finds no
  next-day van (the child's `fail NoVan`, which the task declares); inquiry calls its rule as a
  local activity; order's reminder loop goes on in a new run as its history grows.
- **For AWS**, the tasks call Lambda functions, HTTP APIs through EventBridge connections, and
  SNS and SQS, as Step Functions does; a callback hands on a task token. Lambda durable
  functions runs the same versions, and the code dandori writes for the other platforms makes
  the same calls, so these build for all five. Review's is the exception: every one of its
  tasks is your own code, which Step Functions cannot run, so on AWS it is for Lambda durable
  functions alone.
- **For Argo Workflows**, review's tasks are containers of your images (`image`). The other
  examples run on Argo as they are written for AWS, with their calls made by the caller image
  dandori builds.
- **For pydantic-graph**, the graph runs in the Python process that takes the input: a call to
  an HTTP API or an agent is a function dandori writes, the rules run in the process, the rest
  are functions you write, and a callback is answered in the same process (`Deps.callbacks`).
  Waits hold the process, and a run the process loses is lost, so this is the shape for a
  prototype, or a short flow inside an agent.

Only a flow that runs as it is on every platform sits beside the versions: fulfillment's child,
whose calls are HTTP ones dandori writes for each. The versions share the rules (`rules/`) and
the API descriptions (`specs/`). Each version is played by the runners of the platform it is
written for; the versions for AWS and the flows beside them by every platform. The design, in
Japanese, is in [DESIGN.md](DESIGN.md).

## Install

dandori builds with a recent stable Rust: `cargo install --path .` in a clone (its one
dependency is serde_json). It reads the rules through [rulec](https://github.com/i2y/rulec),
found through `DANDORI_RULEC`, else on the PATH: `brew install i2y/tap/rulec`, or a binary from
rulec's [releases](https://github.com/i2y/rulec/releases). dandori is tested with rulec 0.20.0
and 0.21.1.

## Commands

```
dandori check <file.flow>...
dandori build <file.flow> --target asl|temporal|temporal-python|durable|argo|pydantic-graph [--out <dir>]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target asl|temporal|temporal-python|durable|argo|pydantic-graph]
```

`--lang ja` prints the messages in Japanese. rulec is found through `DANDORI_RULEC`, else
on the PATH. `build` also refuses what the platform cannot do (E050), and a workflow whose one
run can outgrow the platform (E040): its history on Step Functions and Temporal, its operations
on durable functions, its nodes on Argo.

- `build --target asl` writes the state machine and, for every rule it calls, a Lambda
  handler around the Python rulec generates.
- `build --target temporal` writes `workflow.ts`, `types.ts`, `activities.ts`
  (`makeActivities(own, transport)`: the tasks dandori writes, with the ones you write),
  `io.ts` (the `Transport`), `rules.ts` (the rules as activities around rulec's
  TypeScript), `runtime.ts`, `worker.ts` (`makeWorker(own)`) and `client.ts`. The workflow's
  type and task queue carry the `.flow`'s version (`hotel_stay_v1`), so a new version runs
  beside the old one; `makeWorker(own, { deployment })` also puts the worker on Worker
  Deployment Versioning, with a hash of the code as its build id, and pins each run to the
  build it started on. `client.ts` has `start` (which never reuses a workflow id, since the
  idempotency keys are made from it), `answer` (a callback's answer as an Update, which the
  workflow refuses for a callback it does not wait for, or a second time) and `status` (the
  query `dandori.status`: the line the workflow waits at, each case's state, and the events it
  waits for), and `send` for the events. Started with
  `{ searchAttributes: true }`, the workflow also keeps the search attribute `DandoriCases`
  (`"pi=requires_capture"`, …) up to date, so runs can be found by their cases' states.
  To change the code of a version without Worker Deployment Versioning, give the histories
  of the runs that are going on (`client.ts`'s `histories`) to `worker.ts`'s `replay` first.
  A `repeat`, or a `for` that is not parallel, at the top of the flow goes on in a new run
  (Continue-As-New) at the start of a round once the history is long — 10,000 events, or
  sooner when the server suggests it (the dev server did past 4,096) — and hands on the
  variables, the round, and the loop's list and what it has yielded. The workflow id stays, and
  so do the idempotency keys and the ids of callbacks and child workflows; the new run stays on
  the build under Worker Deployment Versioning. A task without `timeout` gets as long as the other platforms
  would give it — 60 seconds for `http` and `agent`, as an HTTP Task has, 900 for `lambda`,
  and no limit of its own for the rest — and every activity the workflow's worker serves
  heartbeats, so that a worker that went away is noticed within 30 seconds. A rule's activity
  gets 10 seconds, and is retried when it runs out.
- `build --target temporal-python` writes the same for Temporal's Python SDK, as a package
  named after the workflow: `workflow.py` (the workflow, and `workflows` to give the
  worker), `types.py`, `activities.py` (`make_activities(own, transport)`), `io.py`,
  `rules.py` (around rulec's Python), `runtime.py`, `worker.py` (`make_worker`) and
  `client.py` (`start`, `answer`, `status`). The workflow type, the activities, the
  callback's update and signal, the query and the ids are named as in the TypeScript, so a
  worker in one language can serve the other.
- `build --target durable` writes `workflow.ts` with `makeHandler(own, transport)`,
  `types.ts`, `tasks.ts`, `io.ts` and `runtime.ts`, and for every rule it calls, the same
  Lambda handler as `asl`, which the durable function invokes.
- `build --target argo` writes `<workflow>.argo.yaml`, a WorkflowTemplate that takes the
  input as the parameter `input` and leaves the outputs in the global parameter
  `dd_output`, and `caller/`: the program that runs the `lambda`, `http`, `aws` and `agent`
  tasks and the rules in the workflow's containers, with its `package.json` and `Dockerfile`.
- `build --target pydantic-graph` writes a package with `graph.py` (`graph`, and its
  `State` and `Deps`), `types.py`, `tasks.py` (`make_tasks(own, transport)`), `io.py`,
  `rules.py` and `runtime.py`. Every statement is a node, and the return type of each node
  names where the flow can go, so `graph.render()` draws the flow. The run lives in the
  process that runs it: the waits sleep by `Deps.clock`, a callback's answer comes to
  `Deps.callbacks`, and pydantic-graph 2.x keeps nothing anywhere else, so a run the process
  loses is lost.
- `scenarios` writes inputs and scripted answers that together take every arm, every
  handler, every way a case can move, and lists that are empty, short, and longer than
  their loop takes; `run` plays one through the reference interpreter.

## Checks

Each diagnostic comes with a run that gets there, as in
[What the checker says](#what-the-checker-says). `check` finds all but E040 and E050, which
`build` finds for the platform it builds for.

| Code | What it finds |
|---|---|
| E001 | a syntax error |
| E002 | a name that is not there: a type, a variable, a field, a rule or a task (a unit of the wrong kind, too) |
| E003 | types that do not match: a value that may be absent used as it is, `none` where it cannot go, a list of lists, a `{…}` or `[]` whose type cannot be told, a range on what is not a number, a range no number is in |
| E004 | too many or too few arguments or outputs, a field left out of a `{…}` record |
| E005 | a rule rulec could not read |
| E006 | a name declared twice |
| E007 | a task's clauses that do not go together: two ways of calling, a `flow` task with another one or with an `image`, `form` on a call to an OpenAPI operation, a Connect error code that is not one, an HTTP error without its status or two errors of one status, a `key` or a `callback` the way of calling cannot have (an agent takes no `key`), an AWS service it does not know, `model` without an `agent`, an agent without the type of its answer or with declared errors, an answer its schema cannot say, a provider it does not know, enum values that differ only in case for Claude, an `event` task's parameters, calls, `retry` or `key`, or one that starts a case, a `url` on a task that is not an agent or on Claude's, a `url` that is not http or https |
| E008 | a case declared wrong, or a task that does to a case what it cannot |
| E009 | a statement where it cannot be: a `yield` that is not the last line of the body of `let <name> = for …`, or such a `for` without one; `break`, `succeed`, a case's call or an event's wait in a round of `for … in parallel`; `succeed` in `on failure` or `on cancel`; a variable given a value both inside a round and outside; a rule called without `let` |
| E010 | a value that no arm of a `match` takes |
| E011 | an arm that can never be taken |
| E012 | a variable read where it may have no value yet |
| E013 | a task called on a case that has not started, or a case started twice |
| E014 | a value that can be outside the range where it goes: a rule's input, a task's parameter, a field of a record written out, an output |
| E015 | a task that runs another `.flow` and does not fit it: its parameters and the child's inputs, its answer and the child's outputs, its errors and the child's `fail`s (a child that cannot be read or does not pass the checks, and a flow that runs itself, too) |
| E016 | a task that does not fit the API description it calls: an operation that is not there, a parameter it does not take or one it needs left out, a type, range or enum that differs, a field the answer may leave out that is not `T?`, a status or an exception it does not answer with, a `key` that is not its idempotency token, a method that streams (and a description that cannot be read, or a `.proto` without `url`) |
| E020 | the workflow can end with a case in a state that is not final (also when `on cancel` ends it as cancelled) |
| E021 | an event sent that every state refuses |
| E022 | an event sent that can be refused, with nothing to handle the refusal |
| E030 | a call that changes the other side, retried without a `key` |
| E031 | what an Express workflow cannot do: a wait longer than five minutes, a callback, a nested execution, a call that changes the other side without a `key` |
| E040 | one run can grow too large for the platform: its history over the limit (25,000 events on Step Functions, 51,200 on Temporal, 3,000 operations on Lambda durable functions), or on Argo Workflows more than 10,000 nodes |
| E050 | what the platform needs is missing, or the platform cannot do it: on Step Functions, a way of calling or a `connection` (an agent's too), a nested execution's declared errors, a `timeout` over 60 seconds on `http` and `agent`, a destination that is not HTTPS; off Temporal, `on cancel` and `event` tasks; on Step Functions and Lambda durable functions, a called rule's `lambda`; on Lambda durable functions, a `timeout` on a function it invokes; on Argo, a way of calling or an `image`, a `workflow template`'s declared errors, `retry` on a `callback` task |
| W030 | a call that may change the other side, retried without a `key` |
| W101 | an error nothing handles can fail the run with a case in a state that is not final (while `on failure` or `on cancel` settles cases, too) |
| W102 | an `on <refusal>` that can never happen |
| W103 | a task that starts a case without a `key` |
| W104 | a value whose range nothing says, where a range is |

## How the output is checked

The reference interpreter defines what a `.flow` means. The tests generate the scenarios
of every example and run each of them eight ways — the reference interpreter, the
generated ASL under JSONata 2.0.6 (`tools/asl-run.mjs`) and on LocalStack's Step Functions
(`tools/localstack/run.mjs`), the generated Temporal workflow
on a Temporal server (the Temporal CLI's dev server), in TypeScript (`tools/temporal/run.mjs`)
and in Python (`tools/temporal-python/run.py`), the generated durable function in the
SDK's local test runner (`tools/durable/run.mjs`), the generated WorkflowTemplate on Argo
Workflows v4.1.4 in a local kind cluster (`tools/argo/run.mjs`), and the generated graph
with pydantic-graph 2.51.0 (`tools/pydantic-graph/run.py`) — and require the same calls,
with the same arguments and idempotency keys, and the same end. No two values in a
scenario are alike, so a target that mixes up two answers or the rounds of a loop shows it
(a number with a range is picked inside it, where two may meet). The scenarios also give answers
every target must refuse: of the wrong shape, with a state the machine does not lead to, and
with a number outside its range.
The examples are written in English, and most of the flows, rules and fixtures under `tests/`
with Japanese names, on purpose: they see that names outside ASCII come through all five
platforms as identifiers, keys and URL paths.
On Temporal, durable functions and pydantic-graph, the tasks dandori writes run with a stand-in
`Transport` that records what they would send, so the comparison is with what Step
Functions sends; the tasks the user writes, the rules and the child workflows are
stand-ins that answer from the scenario. The rounds of a parallel loop run one at a time in
the runners, so the calls come in the reference's order.

On Temporal, every run of a flow goes at once, each as a workflow of its own id, which the
idempotency keys carry. The runners make their workers and start their runs with the generated
`worker` and `client`, answer callbacks with its Update (and see that a second answer is
refused), check what the query and the search attribute say of the cases at the end, and
replay every run's history with the same code. Histories kept in `tests/histories` are
replayed with the code dandori writes now, so a change of the generator that would break a
running workflow shows. Two builds that differ in one text run as versions of one deployment
under Worker Deployment Versioning, and a run that started on the first ends on it, the
round it goes on to in a new run too. The server keeps real time, so the copy of the code the
runners run waits at most 10 ms on a timer and gives an activity 5 seconds, and a call the
scenario times out is kept busy until the server times it out. The copy also counts every
history as long, so a loop at the top of the flow goes on in a new run at every round but the
first of a run, and the runners follow each run to the next to replay them all. Every flow
also runs with its workflow in one language and its activities in the other, both ways: the
other language's runner serves the activities on the same server, and answers the callbacks
with its own client. In a flow with `on cancel`, the scenarios
also cancel the workflow during a call: the stand-in asks the server to cancel it, and holds
its activity until the server cancels that too. A workflow that runs another `.flow` as its
child ([tests/children](tests/children)) also runs with the child's generated workflow in place of
a stand-in, on one server, with the two in one language and in the two crossed: each run must end
as the reference interpreter says when it is given the child's end, which the reference
interpreter decides too, from the input the parent passes.

On LocalStack, the community image of 4.14.0 in Docker (the last that starts without an account),
the generated state machines run on its Step Functions, whose JSONata is the Java one, with every
call's answer mocked: each run is a test case of LocalStack's mocked service integrations, and
the calls, the waits and the end are read from the execution's history. The rounds of a Map run
one at a time, a Wait goes on at once and keeps what it would have waited for in a variable of its
own, and a retry really waits, timed from the history (a run whose retry waits alone came out long, as
they do on a loaded machine, is played once more). That version knows neither the HTTP Task
nor its errors, and its mock file cannot throw Step Functions' own errors. So the runner gives an
HTTP Task the resource of an AWS SDK integration whose answers LocalStack hands on as they are, and
throws Step Functions' errors under another name, which the Retry and the Catch use too: a
timeout, an HTTP Task's error, and the `States.QueryEvaluationError` that an agent's refusal fails
its Task with (that version would fail the whole execution with `States.Runtime` instead).

On Argo, the controller runs the generated WorkflowTemplate, but the runner plays the pods:
each pod waits for a scheduler the cluster does not have, and the runner does what its
container and Argo's executor would do. It runs the generated caller (with the stand-in
`Transport`) or a stand-in for the other tasks, the rules and the workflows a task starts,
reports the outputs the template declares as a WorkflowTaskResult, and ends the pod with the
exit code and the termination message the container would leave. No container starts, so every
run of every flow goes at once. One run of each flow is played again with real pods — the
caller and the stand-ins in node:24-alpine, answered by a mock in the cluster — so that the
containers' side is run too.

An agent's call is recorded as the `Transport` gets it, and the stand-in answers
`{"answer": …}` as the model would. The ASL runner answers an HTTP Task to the Responses API or
the Messages API with a response of the API's shape, and plays a failure as the model refusing,
so the state machine's reading of the answer is what fails the Task. The scenarios give a
Claude agent answers whose enum values are in another case, and every target must take them as
the values. The agent call of the default `Transport` itself, in TypeScript and in Python, runs
with OpenAI's Agents SDK and a scripted model in place of OpenAI's, and with Anthropic's SDK
against a stand-in of the Messages API on this machine (`tools/agents`): the model must be
asked what Step Functions asks for the same call, and an error status must fail the call
without a retry. An agent on another server of Open Responses goes to a stand-in of that server
on this machine, which must get the very request Step Functions sends. Nothing goes to OpenAI or
Anthropic. When Ollama runs on this machine, one call of each such agent also goes to it for
real, from both languages, and each answer must fit the task's type (`DANDORI_OLLAMA` names
where it is, `DANDORI_OLLAMA_MODEL` the model; else the smallest one it has).

The rest of the default `Transport` — `fetch` and the AWS SDK in TypeScript, the standard
library and boto3 in Python — sends every HTTP, Lambda and AWS call of the scenarios to
stand-ins on this machine (`tools/wire`): a server that answers HTTP and Lambda's Invoke as
the runners' stand-in `Transport` would, with the URL's scheme and host replaced by its own,
and moto for SNS and SQS. What arrives must be the call, in the same text from both languages,
and what comes back must be what the stand-in gives; an AWS error comes back by the
name the task declares (`NotFoundException`).

The durable functions test runner cannot time a call out on cue, so the scenarios with a
timeout are left out there. Argo and the graph's runner cannot time out a task either, but a
callback's timeout can be played: on pydantic-graph by not answering it, on Argo by answering
the wait with what its running out gives. On Temporal, both can. The ASL is also validated with
asl-validator, the TypeScript for Temporal, durable functions and Argo's caller passes
`tsc --strict`, and the code between each platform and a rule answers every vector
`rulec vectors` produces as rulec says.

```
npm install --prefix tools
npm install --prefix tools/temporal
npm install --prefix tools/durable
uv venv --python 3.13 tools/temporal-python/.venv
uv pip install --python tools/temporal-python/.venv/bin/python -r tools/temporal-python/requirements.txt
uv venv --python 3.13 tools/pydantic-graph/.venv
uv pip install --python tools/pydantic-graph/.venv/bin/python -r tools/pydantic-graph/requirements.txt
npm install --prefix tools/agents
uv venv --python 3.13 tools/agents/.venv
uv pip install --python tools/agents/.venv/bin/python -r tools/agents/requirements.txt
npm install --prefix tools/wire
uv venv --python 3.13 tools/wire/.venv
uv pip install --python tools/wire/.venv/bin/python -r tools/wire/requirements.txt
sh tools/argo/setup.sh        # a kind cluster with Argo Workflows (docker, kind 0.33+, kubectl)
docker pull localstack/localstack:4.14.0
DANDORI_RULEC=/path/to/rulec cargo test
```

A test that cannot find rulec, Node, the tools, the cluster, the `argo` command or the image of
LocalStack prints a `SKIP:` line. The whole `cargo test` takes about two minutes; `tools/argo/setup.sh`
sets Argo's controller up for it (it looks at a workflow again a second after a change, not
ten) on the node image of kind 0.33.0. When the platform could not run one of a real run's
pods (it ended in Error, or Unknown with exit code 255, as containerd in the node image of
kind 0.29.0 made them now and then under load), the Argo runner plays the run again, at most
twice, and the test says so. `DANDORI_FLOW=<part of a path>`
runs only the flows whose path has it.

## Design in brief

A `.flow` is parsed, its names and types are resolved (a rule's from what `rulec schema`,
`rulec certificate` and `rulec api` print), and the typed tree it becomes is checked along the
flow: the states of its cases, what is given a value where, every arm, every way out. The
reference interpreter runs it, the scenarios play it, and five generators build it. Six
principles hold it together:

- **P1.** The decisions live in rulec. A `.flow`'s expressions build values (records, lists,
  strings with values put in) but have no comparison, arithmetic or logic, and a flow branches
  only by matching an enum, a bool, or a value that may be absent.
- **P2.** dandori stays outside rulec. It reads only the JSON rulec's command line prints, and
  rulec does not know dandori.
- **P3.** One reference interpreter says what a `.flow` means, and what each platform runs is
  held to it.
- **P4.** A loop says how many times it may go round, and there is no recursion, so the length
  of a run's history has a bound.
- **P5.** What cannot be known before the run, such as what the other side answers, is checked
  where it comes in, and a value that does not fit fails the run there.
- **P6.** A feature whose meaning would differ between the platforms is built only where it can
  mean the same, and refused elsewhere with E050: cleaning up after a cancellation (`on cancel`)
  and events sent to a workflow by name (`event`) are Temporal's for now. A clause that only
  changes how or at what cost something runs (`queue`, `image`, a rule's `local`) does nothing
  where it does not apply.

[DESIGN.md](DESIGN.md), in Japanese, gives the reasons for these and for every other decision,
the designs that were dropped, what is left, and what was run to check it all.

## Status

Early. Not yet: Parallel with different branches, OpenAPI documents in YAML, protobuf's binary
encoding and Connect's streams,
cases the workflow holds itself, a rule's preconditions checked at the task that produced
the value, runs on AWS and on a production Temporal cluster or Temporal Cloud (the tests run on
the Temporal CLI's dev server), the caller image run against real Lambda, HTTP and AWS endpoints
from Argo, and agents run against OpenAI and Anthropic themselves. The design, the decisions
and what is left are in [DESIGN.md](DESIGN.md), in Japanese.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option. The cut-down copies of Stripe's OpenAPI document and
of the Smithy models of Amazon SNS and SQS under `examples/` keep their own licenses
([THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
