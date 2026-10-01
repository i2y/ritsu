# Implement a service

A workflow's entry can be written as a standard proto3 service: what a run starts with and ends
with, the names it can fail with, and what it takes on the way. The `.flow` says it implements the
service, and the checker holds it to the service as it holds a task to the API the task calls. The
`.proto` stays a plain `.proto`: protoc and buf read it, and the clients in other languages get
their types from it. dandori puts its marks on the service with options of its own.

## dandori's options

The options are in [proto/dandori/v1/options.proto](https://github.com/i2y/dandori/blob/main/proto/dandori/v1/options.proto)
of the repository. Copy that file into the root of your protos as `dandori/v1/options.proto`, and
import it from the file that has the service. dandori knows the file without a copy, as it knows
Google's well-known types; protoc and buf need the copy. It passes buf's standard lint.

The service names the workflow that implements it, and each method says how it reaches a run, with
one of four options:

| Option | The method |
|---|---|
| `(dandori.v1.start)` | starts a run: its request is the workflow's inputs, its response its outputs, and `fails` lists the names of the `.flow`'s `fail`s |
| `(dandori.v1.event)` | sends a run the value an `event` task waits for: its request is the value |
| `(dandori.v1.answer)` | answers a `callback` task: its request is the answer |
| `(dandori.v1.status)` | asks a run where it is: its response is `dandori.v1.Status`, or a message with the same fields |

The fulfillment example implements this service
([fulfillment.proto](https://github.com/i2y/dandori/blob/main/examples/fulfillment/specs/fulfillment.proto)):

```proto
import "dandori/v1/options.proto";

service FulfillmentService {
  option (dandori.v1.workflow) = {name: "fulfillment", version: 1};

  rpc Fulfill(FulfillRequest) returns (FulfillResponse) {
    option (dandori.v1.start) = {fails: ["OutOfStock", "DeliveryFailed", "PackingLate"]};
  }

  rpc AnswerPacking(AnswerPackingRequest) returns (AnswerPackingResponse) {
    option (dandori.v1.answer) = {task: "wait_for_packing"};
  }
}
```

and each of its versions says so on its first line:

```flow
workflow fulfillment v1 implements shop.FulfillmentService
use proto shop from "../specs/fulfillment.proto"
```

`shop` is the name `use proto` gives the `.proto`, and `FulfillmentService` the service in it. A
workflow implements one service. The `.proto` needs no `url`: no task calls it.

The Japanese versions implement the service of another description, `fulfillment.ja.proto`, whose
names in protobuf's JSON (`json_name`) are Japanese, so that they are those versions' names of the
workflow, its inputs and their fields.

## What the checker holds

The checker holds the workflow to the service (E017), one difference at a time:

- The service's `(dandori.v1.workflow)` names the workflow and its version, as its `workflow` line
  does.
- Every method has one of the four options, and takes and answers one message (no streams). One
  method starts a run.
- The fields of the start's request are the inputs, by their names in protobuf's JSON (`json_name`,
  else lowerCamelCase), and an input reads what its field holds, by the table a task is held to an
  API by: a 64-bit integer is a `string`, an enum has every value the field can have.
- The fields of its response are the outputs, and each output is one its field takes. An output
  that may be absent (`T?`) goes to a field that says whether it is set.
- `fails` lists every name the `.flow` fails with, and no other: a client learns them from the
  `.proto`, which has no place for errors of its own.
- A method with `(dandori.v1.event)` names an `event` task, and one with `(dandori.v1.answer)` a
  `callback` task; the task's answer reads the method's request, and the method answers nothing.
  One method a task.
- A method with `(dandori.v1.status)` takes nothing, and answers what `dandori.v1.Status` has.
- The file imports `dandori/v1/options.proto`, which protoc and buf want.

In a draft whose service and workflow have drifted apart
([tests/fixtures/service.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/service.flow)):

<div class="dd-term" markdown>

```text
error[E017]: tests/fixtures/service.flow:1:36: the workflow fails with `PackingLate`, which `fails` of `Fulfill` does not list
     1 | workflow fulfillment v1 implements shop.FulfillmentService
error[E017]: tests/fixtures/service.flow:1:36: `fails` of `Fulfill` lists `Lost`, and the workflow never fails with it
     1 | workflow fulfillment v1 implements shop.FulfillmentService
```

</div>

The types are compared by shape, not by name. An input can be a message of the `.proto`
(`shop.Order`), and then it fits as it is; or a record of the `.flow`, as the fulfillment's order
is, whose amount is a `money[JPY, incl_tax]` with a range, where the `.proto` has an `int32`, which
knows no unit.

A service need not have every event and callback the workflow waits for: only the ones its clients
send. An event that carries a state of a rule in Japanese, as a test flow's does, cannot be a
`.proto`'s enum, whose values are ASCII names. What a client gets back (the outputs, the names of
the failures, where a run is) is always all there.

## What comes in, and what goes out

Every value goes to and from a run as JSON, in the shape protobuf's JSON mapping gives the message.
That JSON leaves out a field without presence when it holds its zero value (an empty string, 0,
false, an enum's first value, an empty list or map). So on every platform, the code dandori writes
fills those zero values in, from the `.proto`, before it checks what comes in: the input of a run,
and the value of an event or the answer of a callback that a method of the service sends. It does
so in the messages inside them, and in the messages of their lists.

Whether a value is there is the workflow's to say. A field that says whether it is set (a message,
`optional`, a member of a `oneof`) can be read by an input that is not `T?`; a request without it
fails the run at once, with `Dandori.BadInput`. A `json` input is the exception: protobuf's JSON
leaves out a `google.protobuf.Value` that is not set, and the run reads it as null.

What goes out is written in full, the zero values too, which protobuf's JSON reads as well. A run
of a workflow that implements a service ends with an object even when it has no outputs: `{}`,
since protobuf does not read a message from `null`.

## Clients in other languages

The values are JSON: on Temporal, a `json/plain` payload. A client in another language makes the
message's protobuf JSON and hands it on as a JSON value: in Go,
`json.RawMessage(protojson.Marshal(m))`; in Python, `json_format.MessageToDict(m)`. Handed on as a
string, it reaches the workflow as a string, and the run fails with `Dandori.BadInput`. A payload of
protobuf's own (`json/protobuf`, which Temporal's Go SDK makes of a message given as it is) is not
read.

## On Temporal

The methods are what the client already does. The workflow's type is `<name>_v<version>`
(`fulfillment_v1`); an event is the Update `dandori.event`, a callback's answer the Update
`dandori.answer`, and where a run is the query `dandori.status`. The generated `client.ts` and
`client.py` also have:

- `SERVICE`, the service's full name;
- a type for each message, by its name in the `.proto`: `FulfillRequest` is the workflow's input,
  `FulfillResponse` its output, and the request of an event or a callback is the task's answer;
- a function for each method, by its name (`fulfill`, `answerPacking`; in Python `answer_packing`),
  which calls `start`, `send`, `answer` or `status`. A method whose function the client has already
  (`Start` is `start`) adds none.

## On the other platforms

A run is started, and a callback answered, the same way everywhere: the input and the answer are
filled in at the entry on Step Functions, Lambda durable functions, Argo Workflows and pydantic-graph
too. An event is Temporal's alone, as it always is. So is a method that asks a run where it is: the
other platforms have no query a run answers, and refuse a service with one (E050). A service for
them leaves the method out.

## buf's lint

buf's standard lint wants each method's request and response named after it and kept to it
(`GetStatusRequest`, `GetStatusResponse`), across the whole module. That is why the checker compares
a method's messages by their fields, and not by their names. To pass the lint, copy the three fields
of `dandori.v1.Status` into a response of the method's own:

```proto
message GetStatusResponse {
  optional int32 at = 1;
  map<string, google.protobuf.Value> cases = 2;
  repeated string events = 3;
}
```

Without the lint, `dandori.v1.Status` itself is the response.
