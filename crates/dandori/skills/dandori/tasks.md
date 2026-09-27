# What a task calls

A task says how it is called in one line, and dandori writes that call for each platform. The code
it writes for Temporal, Lambda durable functions, Argo Workflows and pydantic-graph sends what AWS
Step Functions sends, so the same `.flow` makes the same requests wherever it runs.

## Ways of calling

| The task says | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| `lambda "<function>"` | Lambda Task | an activity dandori writes, invoking the function | a step dandori writes, invoking the function | a container running the code dandori writes | a function dandori writes, invoking the function |
| `http POST "<url>"` | HTTP Task | an activity dandori writes, with `fetch` | a step dandori writes, with `fetch` | the same, with `fetch` | the same, with urllib |
| `connect <api> "<Service>/<Method>"` | HTTP Task, Connect's JSON | an activity dandori writes, with `fetch` | a step dandori writes, with `fetch` | the same, with `fetch` | the same, with urllib |
| `aws sns:publish` | AWS SDK integration | an activity dandori writes, with the AWS SDK | a step dandori writes, with the AWS SDK | the same, with the AWS SDK | the same, with boto3 |
| `agent …` ([Agents](agents.md)) | HTTP Task to the model's API | an activity dandori writes | a step dandori writes | the same | the same |
| `state machine "<arn>"` | nested execution (`startExecution.sync:2`) | | | | |
| `flow "<path>"` ([Child flows](#child-flows)) | nested execution, with `state machine` | child workflow `<name>_v<n>` on the child's task queue | invoke, with `durable function` | a Workflow from the child's WorkflowTemplate | a function you write |
| `workflow "<type>"` | | child workflow | | | |
| `durable function "<arn>"` | | | invoke of another durable function | | |
| `workflow template "<name>"` | | | | a Workflow from that WorkflowTemplate | |
| `image "<image>"` | | | | a container of your image | |
| `event` | cannot build (E050) | nothing is called: the workflow waits for a value sent to it by name | cannot build (E050) | cannot build (E050) | cannot build (E050) |
| none of these | cannot build (E050) | an activity you write (`OwnTasks`) | a step running code you write (`OwnTasks`) | cannot build (E050) | a function you write (`OwnTasks`) |

The calls dandori writes go through a `Transport` (`io.ts`, `io.py`), whose credentials and clients
are yours to set: the headers of an HTTP API, the AWS SDK's clients, an agent's API key. In tests it
is where a stand-in goes.

- `queue "<name>"` sends a Temporal activity or child workflow to that task queue, where workers in
  either language can serve it.
- `local` under `use rule` has Temporal call the rule as a local activity, in the worker that runs
  the workflow: each call leaves one marker in the history rather than an activity's six events,
  which counts in a flow that decides many times in a loop.
- A `callback` task hands on a token (Step Functions), a callback id (durable functions), or an id
  the answer comes back with as an Update (Temporal) or through `argo node set` (Argo). With
  `aws sqs:sendMessage`, the token travels in the message.
- A task that says `event` calls nothing: the workflow waits for a value sent to it by its id and
  the task's name, as an approval tool or a carrier's webhook would send it, knowing only the order
  it is about. The workflow takes an event only while it waits for it, and refuses any other, so the
  sender learns so and sends it again later. Only Temporal can be sent a value by name, so the other
  platforms refuse the task (E050).
- On Argo Workflows every task runs in a container. The calls dandori writes go into an image built
  from the generated `caller/`; a task with `image` runs your image, which reads the call from
  `DANDORI_CALL`, writes the answer to `/tmp/dandori/answer.json`, and for a declared error writes
  `{"error", "message"}` to `/tmp/dandori/error.json` and exits with 3.

## API descriptions

When the API a task calls is described, the `.flow` reads the description and the checker holds
the task to it (E016):

```flow
use openapi stripe from "../specs/stripe.json"
use smithy sns from "../specs/sns.json"
use proto warehouse from "../specs/warehouse.proto"
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

- An OpenAPI 3 document (JSON) gives an `http` task its operation: the URL is the document's server
  and the path, and the document says whether the body is URL-encoded.
- An AWS API's Smithy model (the JSON AST AWS publishes) is used by the service's name, so
  `aws sns:publish` is held to the model of SNS's `Publish`.
- A `.proto` (proto3) gives `connect` its method, called by the Connect protocol with JSON: a POST to
  `<url>/<package>.<Service>/<Method>` with `Connect-Protocol-Version: 1`. A Connect error is declared
  by its code (`resource_exhausted`) and told apart by the code's HTTP status.

The task passes no parameter the operation does not take and every one it requires, of a type and
in a range it takes. It reads the answer into a type the answer fits, with `T?` for a field the answer
may leave out or send as null, and an enum with every value the answer may have. Its errors are ones
the operation answers with, and an AWS API's `key <parameter>` must be the operation's idempotency
token.

In a draft of calls to Stripe and to a stock service
([tests/fixtures/api_calls.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/api_calls.flow)):

```text
error[E016]: tests/fixtures/api_calls.flow:23:1: what `stripe` POST /v1/payment_intents answers is not `PaymentIntent`: in `created`, an integer is not `timestamp`
    23 | task create_intent(amount: int, currency: string, capture_method: CaptureMethod) -> PaymentIntent
error[E016]: tests/fixtures/api_calls.flow:26:1: what `warehouse` StockService/Reserve answers is not `Reservation`: in `count`, a 64-bit integer comes as a string in protobuf's JSON; declare it `string`
    26 | task reserve_stock(sku: string, quantity: int) -> Reservation
```

protobuf's JSON leaves out a field without presence when it holds its zero value (an empty string,
0, false, an enum's first value, an empty list or map), and whoever reads it with protobuf reads the
zero value there. A `connect` task's answer is read the same way on every platform: the code dandori
writes fills the zero values in from the `.proto`, in the answer, in its messages and in the messages
of its lists, before the answer's type is checked. The checker reads a description only as far as the
task's types go, so Stripe's 8 MB document costs what is looked at; the examples keep cut-down copies.

## Child flows

A task whose child workflow is written in dandori too says where its `.flow` is:
`flow "arrange_delivery.flow"`. The checker checks the child on its own and holds the task to it
(E015): the task passes every input the child needs, of a type and in a range the input takes; its
answer is a record whose fields are the child's outputs; and every error it declares is one the child
fails with. The two files declare their records and enums each on their own, so the types are
compared by shape: a record by its fields, an enum by its values, a range by what it holds, in the
direction the values go.

In a draft of the fulfillment whose delivery task does not fit its child
([tests/fixtures/fulfillment_child.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/fulfillment_child.flow)):

```text
error[E015]: tests/fixtures/fulfillment_child.flow:18:1: the parameter `carrier` is not what `arrange_delivery` takes as its input `carrier`: `drone` of `Carrier` is not a value of `carrier`
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
error[E015]: tests/fixtures/fulfillment_child.flow:18:1: `arrange_delivery` answers `tracking_number` with what the field `tracking_number` of `Delivery` does not take: `string` is not `int`
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
```

The child's names come from its `.flow`: on Temporal the task starts the child's workflow type
(`arrange_delivery_v1`) on its task queue (the same name, where the child's generated worker polls),
and on Argo a Workflow of the child's WorkflowTemplate. Step Functions and Lambda durable functions
still need the ARN the child is deployed as (`state machine`, `durable function`). A flow that runs
itself, directly or through others, is refused: there is no recursion.
