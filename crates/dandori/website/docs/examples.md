# Examples

Five examples, each written for Temporal, for AWS and for pydantic-graph: the same flow, with its
tasks called and its news brought in the way the platform does. Temporal is dandori's main platform,
and its version is the one to read first. The version for Temporal is also drawn by `dandori doc`, on
a page where each scenario lights up the way its run goes ([Draw a workflow](diagrams.md)).

Every version has a Japanese twin beside it, `<name>.ja.flow` (fulfillment's child too,
`arrange_delivery.ja.flow`), which names everything in Japanese but what an API description fixes:
Stripe's fields and states, the warehouse's `.proto`, the SNS and SQS APIs. The Japanese versions of
fulfillment implement a service of their own, `fulfillment.ja.proto`, whose names in JSON are
Japanese. The Japanese rules sit beside the English ones in each `rules/`, and the Japanese site
draws the Japanese versions.

| Example | For Temporal | For AWS (Step Functions, Lambda durable functions) | For pydantic-graph | Drawn |
|---|---|---|---|---|
| a hotel booking that holds a card and captures at check-out, held to Stripe's OpenAPI document | [temporal](https://github.com/i2y/dandori/blob/main/examples/hotel/temporal/hotel.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/hotel/aws/hotel.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/hotel/pydantic-graph/hotel.flow) | [page](doc/hotel.html) |
| an order in a warehouse's system, reminded, shipped, delivered | [temporal](https://github.com/i2y/dandori/blob/main/examples/order/temporal/order.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/order/aws/order.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/order/pydantic-graph/order.flow) | [page](doc/order.html) |
| reserving the lines of an order side by side, packing, delivery: a workflow that implements a service of a `.proto` ([Implement a service](services.md)), the warehouse called by Connect, with the types of its answers made from its `.proto`, and the delivery a child flow, [arrange_delivery](https://github.com/i2y/dandori/blob/main/examples/fulfillment/arrange_delivery.flow), written once for every platform | [temporal](https://github.com/i2y/dandori/blob/main/examples/fulfillment/temporal/fulfillment.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/fulfillment/aws/fulfillment.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/fulfillment/pydantic-graph/fulfillment.flow) | [page](doc/fulfillment.html) |
| an inquiry sorted by [Jev](jev.md), with an agent's reading when Jev is not sure, a rule that routes it, and an agent that drafts the reply | [temporal](https://github.com/i2y/dandori/blob/main/examples/inquiry/temporal/inquiry.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/inquiry/aws/inquiry.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/inquiry/pydantic-graph/inquiry.flow) | [page](doc/inquiry.html) |
| an application scored by [Jev](jev.md), and a rule that weighs how sure the score is and sends the rest to a person's approval; also [for Argo Workflows](https://github.com/i2y/dandori/blob/main/examples/review/argo/review.flow) | [temporal](https://github.com/i2y/dandori/blob/main/examples/review/temporal/review.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/review/aws/review.flow) (Lambda durable functions) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/review/pydantic-graph/review.flow) | [page](doc/review.html) |

## How the versions differ

**For Temporal**, a call to an HTTP API is an activity dandori writes (the `Transport` adds the
credentials, so there is no `connection`), and the rest are activities you write. News from outside
the workflow comes as an `event`, sent to the workflow by its id (Stripe's webhook in hotel, the
carrier in order); a request that is answered later stays a `callback`, answered by the Update the
generated client sends. Hotel and order release what they hold when the workflow is cancelled
(`on cancel`); fulfillment and review send work to other task queues, and fulfillment falls back to
the standard carrier when its child flow finds no next-day van (the child's `fail NoVan`, which the
task declares). Inquiry calls its rule as a local activity, and order's reminder loop goes on in a
new run as its history grows. Review asks Jev from the workers of its own task queue, and inquiry
from the workflow's.

**For AWS**, the tasks call Lambda functions, HTTP APIs through EventBridge connections, and SNS and
SQS, as Step Functions does; a callback hands on a task token. Order calls its urgency rule at its
Connect service, through a connection too, where the other versions keep the rule's code with the
workflow ([A rule as a service](tasks.md#a-rule-as-a-service)). Lambda durable functions runs the same
versions, and the code dandori writes for the other platforms makes the same calls, so these build
for all five. Jev is called by an HTTP Task, with TypeSafe's key in the connection. Review's is the
exception: asking for the approval and the notice are your own code, which Step Functions cannot
run, so on AWS it is for Lambda durable functions alone.

**For Argo Workflows**, review's approval and notice are containers of your images (`image`), and
its scoring and rule run in the caller image dandori builds. The other examples run on Argo as they
are written for AWS, with their calls made by that caller image.

**For pydantic-graph**, the graph runs in the Python process that takes the input: a call to an HTTP
API, an agent or Jev is a function dandori writes, the rules run in the process, the rest are functions
you write, and a callback is answered in the same process (`Deps.callbacks`). Waits hold the process,
and a run the process loses is lost.

Only a flow that runs as it is on every platform sits beside the versions: fulfillment's child, whose
calls are HTTP ones dandori writes for each. The versions share the rules (`rules/`) and the API
descriptions (`specs/`).

## The tests' flows

The flows under
[tests/flows](https://github.com/i2y/dandori/tree/main/tests/flows) exercise the corners of the
language: lists put into `json`, a parallel loop inside a parallel loop, agents' answers with every
kind of type, Jev's questions of every kind and how sure its answers are, calls that time out, local
rules, Connect's zero values, events, and services implemented, whose requests come with their zero
values left out. They are written with
Japanese names on purpose, to see that names outside ASCII come through all five platforms as
identifiers, keys and URL paths.
