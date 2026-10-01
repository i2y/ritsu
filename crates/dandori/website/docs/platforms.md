# Build for a platform

```console
$ dandori build <file.flow> --target temporal|temporal-python|asl|durable|argo|pydantic-graph [--out <dir>]
```

A build writes the code for one platform, and refuses what that platform cannot do (E050) and a
workflow whose one run can outgrow it (E040). Temporal is dandori's main platform.

## Temporal (TypeScript)

`--target temporal` writes `workflow.ts`, `types.ts`, `activities.ts`
(`makeActivities(own, transport)`: the tasks dandori writes, with the ones you write), `io.ts` (the
`Transport`), `rules.ts` (the rules as activities around rulec's TypeScript), `runtime.ts`,
`worker.ts` (`makeWorker(own)`) and `client.ts`. A rule that says `connect` under `use rule` has no
code in `rules.ts`: its activity is in `activities.ts`, and sends to the rule's service
([A rule as a service](tasks.md#a-rule-as-a-service)).

- **Versions.** The workflow's type and task queue carry the `.flow`'s version (`hotel_stay_v1`), so a
  new version runs beside the old one. `makeWorker(own, { deployment })` also puts the worker on
  Worker Deployment Versioning, with a hash of the code as its build id, and pins each run to the
  build it started on. To change the code of a version without it, give the histories of the runs
  that are going on (`client.ts`'s `histories`) to `worker.ts`'s `replay` first.
- **The client.** `client.ts` has `start` (which never reuses a workflow id, since the idempotency
  keys are made from it), `answer` (a callback's answer, as an Update the workflow refuses for a
  callback it does not wait for, or a second time), `status` (the query `dandori.status`: the line the
  workflow waits at, each case's state, and the events it waits for) and `send` for the events.
  Started with `{ searchAttributes: true }`, the workflow also keeps the search attribute
  `DandoriCases` (`"pi=requires_capture"`, …) up to date, so runs can be found by their cases' states.
  For a workflow that implements a service, it also has a function for each of the service's
  methods (`fulfill`, `answerPacking`), and a type for each message by its name in the `.proto`
  (`FulfillRequest`), as [Implement a service](services.md) says.
- **Long runs.** A `repeat`, or a `for` that is not parallel, at the top of the flow goes on in a new
  run (Continue-As-New) at the start of a round once the history is long: 10,000 events, or sooner
  when the server suggests it (the dev server did past 4,096). It hands on the variables, the round, and the loop's list and what it
  has yielded. The workflow id stays, and so do the idempotency keys and the ids of callbacks and
  child workflows; under Worker Deployment Versioning the new run stays on its build.
- **Timeouts.** A task without `timeout` gets as long as the other platforms would give it: 60 seconds
  for `http`, `agent` and `jev`, as an HTTP Task has, 900 for `lambda`, and no limit of its own for the rest.
  Every activity the workflow's worker serves heartbeats, so that a worker that went away is noticed
  within 30 seconds. A rule's activity gets 10 seconds, and is retried when it runs out.

## Temporal (Python)

`--target temporal-python` writes the same for Temporal's Python SDK, as a package named after the
workflow: `workflow.py` (the workflow, and `workflows` to give the worker), `types.py`,
`activities.py` (`make_activities(own, transport)`), `io.py`, `rules.py` (around rulec's Python),
`runtime.py`, `worker.py` (`make_worker`) and `client.py` (`start`, `answer`, `status`, and for a
workflow that implements a service, a function for each method, in snake case: `answer_packing`).
The workflow type, the activities, the callback's Update and signal, the query and the ids are named
as in the TypeScript, so a worker in one language can serve the other.

## AWS Step Functions

`--target asl` writes the state machine, in ASL with JSONata, and for every rule it calls by Lambda, a
Lambda handler around the Python rulec generates. A rule that says `connect` is called at its service
by an HTTP Task, which needs a `connection` and a URL that is HTTPS, and has no Lambda function. The tasks call Lambda functions, HTTP APIs through
EventBridge connections, and AWS services through the SDK integrations; a callback hands on a task
token. Code you write has no place in a state machine, so a task that is neither of these is refused
(E050), and so are the features that only Temporal can keep (`on cancel`, `event`, and a method of
the service the workflow implements that asks where a run is).

## AWS Lambda durable functions

`--target durable` writes `workflow.ts` with `makeHandler(own, transport)`, `types.ts`, `tasks.ts`,
`io.ts` and `runtime.ts`, and for every rule it calls by Lambda, the same Lambda handler as `asl`, which
the durable function invokes; a rule that says `connect` is a step that sends to its service. A task of
your own is a step that runs your code.

## Argo Workflows

`--target argo` writes `<workflow>.argo.yaml`, a WorkflowTemplate that takes the input as the
parameter `input` and leaves the outputs in the global parameter `dd_output`, and `caller/`: the
program that runs the `lambda`, `http`, `aws`, `agent` and `jev` tasks and the rules in the workflow's
containers, with its `package.json` and `Dockerfile`. A task of your own is a container of your image
(`image`). The WorkflowTemplate keeps the flow's variables in global output parameters, and the YAML
starts with a comment that names each variable's parameter.

A task's `timeout` becomes the pod's `activeDeadlineSeconds`, which Kubernetes counts from the moment
it takes the pod up, the start of its containers included. A busy cluster can take several seconds to
start a pod, so give a task a `timeout` that leaves room for that. A pod that ends just as its deadline
passes is marked as past it, and Argo then cannot see the name of the error the task ended with: a
`retry` on that error does not happen.

## pydantic-graph

`--target pydantic-graph` writes a package with `graph.py` (`graph`, and its `State` and `Deps`),
`types.py`, `tasks.py` (`make_tasks(own, transport)`), `io.py`, `rules.py` and `runtime.py`. Every
statement is a node, and the return type of each node names where the flow can go, so
`graph.render()` draws the flow. The run lives in the process that runs it: the waits sleep by
`Deps.clock`, a callback's answer comes to `Deps.callbacks`, and pydantic-graph 2.x keeps nothing
anywhere else, so a run the process loses is lost. It suits a prototype, or a short flow inside an
agent.

Lambda durable functions, Argo Workflows and pydantic-graph refuse `event` tasks and a service's
method that asks where a run is too (E050). A workflow that implements a service fills the zero
values protobuf's JSON leaves out of its input, and of a callback's answer that a method of the
service sends, on every platform.

## Scenarios and the reference interpreter

```console
$ dandori scenarios <file.flow> [--out <dir>]
$ dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|durable|argo|pydantic-graph]
```

`scenarios` writes inputs and scripted answers that together take every arm, every handler, every way
a case can move, and lists that are empty, short, and longer than their loop takes. `run` plays one
through the reference interpreter and prints the calls as the target would make them.
[How it is checked](assurance.md) says how the builds are held to it.
