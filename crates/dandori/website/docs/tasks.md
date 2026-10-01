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
| `jev …` ([Jev](jev.md)) | HTTP Task to TypeSafe's API | an activity dandori writes, with `fetch` | a step dandori writes, with `fetch` | the same, with `fetch` | the same, with urllib |
| `state machine "<arn>"` | nested execution (`startExecution.sync:2`) | | | | |
| `flow "<path>"` ([Child flows](#child-flows)) | nested execution, with `state machine` | child workflow `<name>_v<n>` on the child's task queue | invoke, with `durable function` | a Workflow from the child's WorkflowTemplate | a function you write |
| `workflow "<type>"` | | child workflow | | | |
| `durable function "<arn>"` | | | invoke of another durable function | | |
| `workflow template "<name>"` | | | | a Workflow from that WorkflowTemplate | |
| `image "<image>"` | | | | a container of your image | |
| `event` | cannot build (E050) | nothing is called: the workflow waits for a value sent to it by name | cannot build (E050) | cannot build (E050) | cannot build (E050) |
| none of these | cannot build (E050) | an activity you write (`OwnTasks`) | a step running code you write (`OwnTasks`) | cannot build (E050) | a function you write (`OwnTasks`) |

The calls dandori writes go through a `Transport` (`io.ts`, `io.py`), whose credentials and clients
are yours to set: the headers of an HTTP API, the AWS SDK's clients, an agent's API key, TypeSafe's
key for Jev. In tests it is where a stand-in goes.

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

<div class="dd-term" markdown>

```text
error[E016]: tests/fixtures/api_calls.flow:23:1: what `stripe` POST /v1/payment_intents answers is not `PaymentIntent`: in `created`, an integer is not `timestamp`
    23 | task create_intent(amount: int, currency: string, capture_method: CaptureMethod) -> PaymentIntent
error[E016]: tests/fixtures/api_calls.flow:26:1: what `warehouse` StockService/Reserve answers is not `Reservation`: in `count`, a 64-bit integer comes as a string in protobuf's JSON; declare it `string`
    26 | task reserve_stock(sku: string, quantity: int) -> Reservation
```

</div>

protobuf's JSON leaves out a field without presence when it holds its zero value (an empty string,
0, false, an enum's first value, an empty list or map), and whoever reads it with protobuf reads the
zero value there. A `connect` task's answer is read the same way on every platform: the code dandori
writes fills the zero values in from the `.proto`, in the answer, in its messages and in the messages
of its lists, before the answer's type is checked. A field that is `json` and is left out (protobuf's
JSON leaves out a `Value` that is not set) is read as null. The checker reads a description only as
far as the task's types go, so Stripe's 8 MB document costs what is looked at; the examples keep
cut-down copies.

### Types from a .proto

A message or an enum of a `.proto` that `use proto` read is a type, written after the name `use`
gives it: `warehouse.ReserveResponse`. It goes wherever a type does (a record's field, a task's
parameter or answer, `inputs` and `outputs`, `let x: T`, inside `list[…]` and `T?`, a case's record),
so a record that only repeats what the description says need not be written by hand. The warehouse
of the fulfillment example says:

```proto
enum Stock {
  unspecified = 0;
  secured = 1;
  short = 2;
}

message ReserveResponse {
  string sku = 1;
  Stock stock = 2;
  optional string id = 3;
}
```

and the flow uses what it says:

```flow
record PackingRequest
  order_id     : string
  reservations : list[warehouse.ReserveResponse]

task reserve_stock(sku: string, quantity: int) -> warehouse.ReserveResponse
  connect warehouse "StockService/Reserve"
  errors busy = resource_exhausted
```

`warehouse.ReserveResponse` is a record of `sku : string`, `stock : warehouse.Stock` and `id : string?`,
and `warehouse.Stock` an enum of `secured | short`. The task needs nothing more to fit its
description: the table that makes a type is the one the task is held to.

| A `.proto` has | A flow has |
|---|---|
| `string`, `bytes` | `string` |
| `bool` | `bool` |
| a 32-bit integer (`int32`, `uint32`, `sint32`, `fixed32`, `sfixed32`) | `int` |
| a 64-bit integer | `string`, as protobuf's JSON writes it |
| `float`, `double`, `map` | `json` |
| an enum | an enum, with its values named as the `.proto` names them |
| a message | a record, with its fields named as protobuf's JSON names them (`json_name`, else lowerCamelCase) |
| `repeated T` | `list[T]` |
| a field that says whether it is set (`optional`, a message, a member of a `oneof`) | `T?` |
| `google.protobuf.Timestamp` | `timestamp` |
| `Struct`, `Value`, `ListValue`, `Any` | `json` |
| `Empty` | a record with no fields |
| `Duration`, `FieldMask` | `string` |
| the wrappers (`Int32Value`, …) | the type inside: `Int64Value` is `string`, `DoubleValue` is `json` |

- A nested message is written `warehouse.Order.Line`. A type of another package that the file imports
  is written from its package: `warehouse.common.v1.Money`. Only the types a flow names, and what
  they reach, are made. A message that holds itself, directly or through others (a tree, with
  `repeated Tree children`), cannot be a record (E003); write the record yourself, with `json` where
  it holds itself. A record written in a `.flow` cannot hold itself either, for the same reason: the
  scenarios and the code dandori writes follow a type to its end, and there is none.
- A name the `.proto` does not have is E002, with the names it has. A service's name is E002 too, and
  so is a name after an OpenAPI document or a Smithy model, which make no types.

<div class="dd-term" markdown>

```text
error[E002]: tests/fixtures/proto_types.flow:27:11: there is no type `types.Stok`
    27 |   stock : types.Stok
  = the messages and enums of `types` are Box, Everything, Everything.Nested, GetRequest, Line, Mode, Status, Ticket
```

</div>

- A number's range is what Protovalidate says of it: `gte`, `gt`, `lte`, `lt` and `const` under
  `(buf.validate.field).int32`, written in one option or one by one, and `repeated.items` for the
  numbers of a list. A field ignored at its zero value (`IGNORE_IF_ZERO_VALUE`) lets 0 through, and a
  low end above the high end means what lies outside them, which is no range. The range is the field's
  in the record, so what [Ranges](tour.md#ranges) says holds: a value that may fall outside it is E014,
  one whose range nothing says is W104, and a task that sends the field without a range of its own is
  E016. A field that says whether it is set is `T`, not `T?`, when it is `required`. The other rules
  (the length of a string, the count of a list, CEL) are read as not there.

<div class="dd-term" markdown>

```text
error[E014]: tests/fixtures/proto_ranges.flow:35:1: `line.quantity` can be outside `>=1 <=5`, the range of the parameter `n` of `few`: it is `>=1 <=99`
    35 |   few(n: line.quantity)
error[E014]: tests/fixtures/proto_ranges.flow:41:1: `many` can be outside `>=1 <=10`, the range of the field `count` of `types.Box`: it is `>=1 <=200`
    41 |   let too_many: types.Box = {count: many}
```

</div>

- An enum's zero value is left out when its name says nothing was set: with the enum's own name in
  capitals in front (`STOCK_`) taken off, it is `unspecified`, in any case. That is protobuf's mark of a
  field that was not set, not a value to `match` on, and an answer that comes with it fails with
  `Dandori.BadResponse`. A zero value with another name (`ACTIVE = 0`) is a value, and stays.
- A `Value` that is not set is left out of protobuf's JSON, so a field that is `json` may not be
  there. It reads as null, as a `T?` does, and a task that is given the field is passed `null`; a
  record passed on whole goes on without it. A `json` input of the workflow reads the same way when
  the start leaves it out.
- The code dandori writes names a type with its dots turned to underscores (`warehouse_ReserveResponse`),
  and E006 says so when another type of the flow comes to the same name. `dandori doc` shows the name as
  the `.flow` writes it.
- `url` is asked for where a task calls the service (`connect`). A `.proto` read only for its types needs
  none.
- A `.proto` may import files that are not on the disk. `google/api/annotations.proto` is imported for
  options, and nothing in it is a type a flow uses, so the `.proto` is read all the same and the file is
  passed over. A type that only such a file has is not known: a message with a field of one cannot be
  made (E002), a method that takes or answers one cannot be held to a task (E016), and the diagnostic
  names the files that were not read.

<div class="dd-term" markdown>

```text
error[E002]: tests/fixtures/proto_unread.flow:11:12: `catalog.Priced` cannot be made: its field `price` is of the type `google.type.Money`, which is not known; write the record yourself, with `json` for `price`
    11 |   priced : catalog.Priced
  = `google/api/annotations.proto`, `google/type/money.proto` and `shop/v2/cancel.proto` could not be read, so the types in them cannot be used
```

</div>

## A rule as a service

By default a rule's code goes with the workflow: a Lambda function around the Python rulec generates on
Step Functions, an activity around its TypeScript or Python on Temporal. A rule can instead be called
at the Connect service that `rulec gen` writes for it, from every platform, so that the rule is in one
place and a change of its table reaches every workflow that calls it:

```flow
use rule urgency from "../rules/urgency.rule"
  connect "https://rules.example.com"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/rules/5e6f7a"
```

`connect` says where the service is. `connection` is the EventBridge connection Step Functions' HTTP
Task calls it through, and it goes only with `connect`. A rule is called by `lambda` or by `connect`,
not by both (E007); `local` still makes it a local activity on Temporal.

<div class="dd-term" markdown>

```text
error[E007]: tests/fixtures/rule_connect.flow:6:3: the rule is already called another way (line 5); a rule is called by `lambda` or by `connect`
     6 |   connect "https://rules.example.com"
```

</div>

dandori reads the service as it reads the rest of the rule, from `rulec api`, and not from the `.proto`
that `rulec gen` writes: the path of the method, the fields of its request and its response, and what
the service calls the values of each enum. A call is a POST of the URL and the path, with the header
`Connect-Protocol-Version: 1` and a JSON body written as protobuf's JSON writes it. Field names are in
lowerCamelCase, a number is a decimal string (rulec's numbers are 64-bit integers), and an enum's value
is the `.proto`'s name for it, as `rulec api` gives it (`CARRIER_NEXTDAY` for `next_day`). Every input
is written out, at its zero value too (`false`, `"0"`): the service tells an input left out from one set
to zero. This is what the service `rulec gen` writes for the urgency rule is sent, and what it answers:

```text
POST /rulec.urgency.v1.UrgencyService/Decide
Connect-Protocol-Version: 1
Content-Type: application/json

{"member":false,"amount":"5000"}

200 OK
rulec-source-sha256: 5ff6efc93a9b39d25225c10aa67460104da8e393a052635b225d497c51b7078f

{"carrier":"CARRIER_STANDARD","trace":[{"table":"decide","row":3}]}
```

- The service leaves a field out of its answer when it is at its zero value, as protobuf's JSON does:
  `urgent` is false here. The answer is read as protobuf reads it, with what was left out put back
  (false, 0, an empty string, an enum's value 0), and then as the rule's own record: a number from its
  decimal string, an enum's value by the rule's name for it. Value 0 of the carrier is
  `CARRIER_UNSPECIFIED`, which is no value of the rule's. Whatever does not fit is left as it is, and the
  check every rule's answer goes through, its types and the range rulec gives each output, ends the call
  with `Dandori.BadResponse`: an enum's value 0 that is no value of the rule's, a name no value has, a
  number that is not a decimal, or is more than 2^53 − 1, a body that is not an object. `trace`, the rows
  that matched, is not read.
- A status other than 200, and a request that does not go through, fail the call. A rule has no errors
  of its own, so `failure` is the only one, and it is retried as a rule's call always is: twice, a second
  and two seconds later.
- The version of the table that answered (`rulec-source-sha256`) is not looked at. A service that has
  another table than the one checked does not fail the call: the point of one service is to change the
  table without publishing the workflows again. What holds is the answer's type and range, checked each
  time, and the `v1` in the path, which is the version of the contract and which `buf breaking` guards.

| Platform | A rule called at its service is |
|---|---|
| Step Functions | an HTTP Task through `connection` (E050 without one, and for a URL that is not HTTPS); no Lambda function is written |
| Temporal | an activity dandori writes, still named `rule_<rule>`, which sends through the `Transport`; the workflow and the histories it leaves are the same as with the rule's code |
| Lambda durable functions | a step that sends through the `Transport` |
| Argo Workflows | the caller image, as for any rule, sending through the `Transport` |
| pydantic-graph | a function of `Deps.tasks`, sending through the `Transport` |

### An enum from a contract

A rule can take an enum from a `.proto` (`import proto` in the rule). Its values are named as the
contract names them, which need not follow buf's convention, and its value 0 may be a value of its own:

```proto
enum Status {
  ACTIVE = 0;
  CLOSED = 1;
}
```

So dandori reads these names from `rulec api` rather than building them from the enum's name. The service
leaves out of its answer an enum at value 0, as it leaves out any zero value, and when value 0 is a value
of the rule's, dandori reads the answer without it as that value. Sent, `ACTIVE` is written out like any
other value. The rule of `tests/fixtures/rules/account_fee.rule` takes this enum, and its service answers
a state that stays `ACTIVE`, and no fee, with nothing but the rows that matched:

```text
POST /rulec.account_fee.v1.AccountFeeService/Decide
Connect-Protocol-Version: 1
Content-Type: application/json

{"state":"ACTIVE","balance":"20000"}

200 OK
rulec-source-sha256: bdb2f090c3a1a0c471d0b94dd4f23db7aa7a4d9bd8c23bdd786effd3b0db16ba

{"trace":[{"table":"手数料表","row":1}]}
```

`rulec api` says these names (`connect.enums`) from rulec 0.22.0; 0.21.2 and before do not. With those,
a rule whose enum is a contract's cannot be called at its service, and `check` says so; a rule's own
enums are still called, by the names those rulecs give them:

<div class="dd-term" markdown>

```text
error[E005]: tests/flows/connect_rules_contract.flow:4:10: could not read the rule `../fixtures/rules/account_fee.rule`
     4 | use rule 手数料 from "../fixtures/rules/account_fee.rule"
  = the enum `口座の状態` of the rule is a contract's (`Status`), and this rulec's `rulec api` does not say what the rule's service calls its values; `connect` needs rulec 0.22.0 or later, whose `rulec api` says them (`connect.enums`)
```

</div>

### What the service refuses

A service written by rulec 0.22.0 or later refuses, with `invalid_argument` (400), an input
outside the rule's range, a name its enum does not have, a field the request does not have, and a request
that leaves an input out. dandori sends none of them to a service that runs the rule it checked. A
service that runs another version of the rule, with an input renamed or an enum's value changed, refuses
the call rather than deciding on a zero value, and the call fails as it does for any status other than
200. The services rulec 0.21.2 writes pass over a field they do not know, read an input
left out as its zero value, and read a name their enum does not have as its value 0, which they refuse
only when value 0 says that nothing was set.

A rule that walks a list of elements (`elements` in the rule) is not called at all, at its service or by
its code: dandori does not pass a rule a list yet (E005).

The order example for AWS calls its urgency rule this way
([order.flow](https://github.com/i2y/dandori/blob/main/examples/order/aws/order.flow)), and the tests
send the services `rulec gen` writes what dandori sends them ([How it is checked](assurance.md)).

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

<div class="dd-term" markdown>

```text
error[E015]: tests/fixtures/fulfillment_child.flow:18:1: the parameter `carrier` is not what `arrange_delivery` takes as its input `carrier`: `drone` of `Carrier` is not a value of `carrier`
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
error[E015]: tests/fixtures/fulfillment_child.flow:18:1: `arrange_delivery` answers `tracking_number` with what the field `tracking_number` of `Delivery` does not take: `string` is not `int`
    18 | task arrange_delivery(order_id: string, carrier: Carrier, recipient: string?, extra: json) -> Delivery
```

</div>

The child's names come from its `.flow`: on Temporal the task starts the child's workflow type
(`arrange_delivery_v1`) on its task queue (the same name, where the child's generated worker polls),
and on Argo a Workflow of the child's WorkflowTemplate. Step Functions and Lambda durable functions
still need the ARN the child is deployed as (`state machine`, `durable function`). A flow that runs
itself, directly or through others, is refused: there is no recursion.
