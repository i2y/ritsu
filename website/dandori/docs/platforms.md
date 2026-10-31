# Build for a platform

```console
$ dandori build <file.flow> --target temporal|temporal-python|temporal-go|asl|durable|argo|pydantic-graph [--out <dir>]
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
  within 30 seconds. A rule's or a date's activity gets 10 seconds from the moment a worker takes it
  (its start-to-close timeout; the time it waits in the task queue is not counted), and is retried
  twice, 1 and 2 seconds apart, when it runs out or fails. The activity that hands a callback's id on
  gets a minute, whatever the callback's `timeout`, which is the wait for the answer. The wait of an
  `event` or a `callback` task with `timeout until` is a timer of the workflow, set by the workflow's
  clock when the wait starts ([How long a call may take](tasks.md#how-long-a-call-may-take)).

## Temporal (Python)

`--target temporal-python` writes the same for Temporal's Python SDK, as a package named after the
workflow: `workflow.py` (the workflow, and `workflows` to give the worker), `types.py`,
`activities.py` (`make_activities(own, transport)`), `io.py`, `rules.py` (around rulec's Python),
`runtime.py`, `worker.py` (`make_worker`) and `client.py` (`start`, `answer`, `status`, and for a
workflow that implements a service, a function for each method, in snake case: `answer_packing`).
The workflow type, the activities, the callback's Update and signal, the query and the ids are named
as in the TypeScript, so a worker in one language can serve another.

## Temporal (Go)

`--target temporal-go` writes the same for Temporal's Go SDK, as one Go package named after the
workflow, with no `go.mod` of its own: put it in your module and `go get` the modules its `doc.go`
names. Go's import paths are ASCII, so when the workflow's name is not, the package is named after the
`.flow` file (`hotel.ja.flow` writes `hotel_ja`), or else `workflow`. `workflow.go` has `Workflow`,
`activities.go` the interface `OwnTasks` of the tasks you write and `Activities(own, transport)`,
`io.go` the `Transport`, `rules.go` the rules as activities around rulec's Go, `worker.go`
`NewWorker(client, own, transport, deployment)` and `Replay`, and `client.go` `Start`, `Answer`,
`Send`, `Status` and `Histories`, and for a workflow that implements a service, a function for each
method (`Fulfill`, `AnswerPacking`). Everything on the wire is named as in the TypeScript.

- **JSON values.** The workflow carries its values as JSON values (`map[string]any`, `[]any`,
  `float64`, `string`, `bool`, `nil`), as the Python build does: a struct would make a field that is
  not there the same as a null, drop the fields it does not know, and fail on an answer of another
  shape, which the workflow has to see to refuse it. `types.go` still has a struct for each record and a
  string type for each enum, and `Decode` reads a task's arguments into them; each of your tasks gets
  its arguments as `map[string]any` and answers any value the SDK can write as JSON.
- **The rules.** `rulec gen <rule> --out <the package's directory>/rulec` writes each rule's Go as a
  module of its own (`rulec/go/holdamount`), which your `go.mod` requires and replaces with that
  directory, as `rules.go` says at its top.
- **The default `Transport`** uses `net/http`, the AWS SDK for Go v2 for Lambda and the AWS APIs, OpenAI's
  Go client for OpenAI's agents, and Anthropic's Go SDK for Claude's, and a package imports only the
  SDKs its flow calls through. OpenAI has no Agents SDK for Go; the Go client sends the request Step
  Functions sends to the Responses API.
- **Go 1.26.** The Temporal Go SDK the code is written against, 1.49.0, asks for Go 1.26, which the
  `go` command fetches by itself when yours is older.

## AWS Step Functions

`--target asl` writes the state machine, in ASL with JSONata, and for every rule it calls by Lambda, a
Lambda handler around the Python rulec generates. A rule that says `connect` is called at its service
by an HTTP Task, which needs a `connection` and a URL that is HTTPS, and has no Lambda function. The tasks call Lambda functions, HTTP APIs through
EventBridge connections, and AWS services through the SDK integrations; a callback hands on a task
token. Code you write has no place in a state machine, so a task that is neither of these is refused
(E050), and so are the features that only Temporal can keep (`on cancel`, `event`, and a method of
the service the workflow implements that asks where a run is).

A rule's or a date's Task, by Lambda or by HTTP, has `TimeoutSeconds` 10, and its `Retry` tries it
twice more, 1 and 2 seconds apart, when it times out (`States.Timeout`) or fails. A callback with
`timeout until` gives its `.waitForTaskToken` Task a `TimeoutSeconds` that JSONata works out from the
time and the moment the state was entered, a second at the least; the Task's clock starts before the
token is handed on, so the time the handing on takes is in it. So it is with a callback's `timeout`:
the Task has no limit of its own on handing the token on, which the other platforms give a minute.

## AWS Lambda durable functions

`--target durable` writes `workflow.ts` with `makeHandler(own, transport)`, `types.ts`, `tasks.ts`,
`io.ts` and `runtime.ts`, and for every rule it calls by Lambda, the same Lambda handler as `asl`. A
rule's or a date's call is a step that calls its Lambda function through the `Transport`, as a `lambda`
task does, or sends to the rule's service when the rule says `connect`, and the step gives the call 10
seconds: the SDK's `context.invoke` takes no timeout. A task of your own is a step that runs your
code. A callback with `timeout until` reads the clock in a step of its own, and gives
`createCallback` the seconds left to the time, a second at the least. The step that hands a callback's
id on gives the call a minute.

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

A rule's or a date's call gets its 10 seconds in the caller, not on the pod: past them the caller
exits 143, as a container Argo stops at its deadline does, and the workflow reads it as a timeout. The
time Kubernetes takes to place and start the pod is not counted, as a Temporal activity's time in its
task queue is not. The caller gives a callback's id a minute to be handed on in the same way. A
callback that runs your own image hands its id on in your image, where dandori cannot keep the time,
so the pod that hands it on gets `activeDeadlineSeconds: 60`, which counts the pod's start too.
The template's `retryStrategy` tries a call twice more, with a backoff of 1 and 2
seconds, but Argo starts the next try when its controller looks at the workflow again, 10 seconds after
a change by default. A callback with `timeout until` has a step after the one that hands the id on,
which works out the seconds left to the time from `now()`, a second at the least, and the wait's
`suspend` lasts them.

## pydantic-graph

`--target pydantic-graph` writes a package with `graph.py` (`graph`, and its `State` and `Deps`),
`types.py`, `tasks.py` (`make_tasks(own, transport)`), `io.py`, `rules.py` and `runtime.py`. Every
statement is a node, and the return type of each node names where the flow can go, so
`graph.render()` draws the flow. The run lives in the process that runs it: the waits sleep by
`Deps.clock`, a callback's answer comes to `Deps.callbacks`, and pydantic-graph 2.x keeps nothing
anywhere else, so a run the process loses is lost. It suits a prototype, or a short flow inside an
agent.

A rule that says `connect` gets 10 seconds for each try at its service. A rule or a date computed in
the process, by rulec's or koyomi's Python, gets no limit: it reads and writes nothing, always ends,
and asyncio could not stop it in the middle anyway. A callback's id gets a minute to be handed on. A
callback with `timeout until` waits the seconds from `Deps.clock`'s time to the one it is given, a
second at the least.

A dates file's dates and a book's operations build for every platform: a date is a call like a rule's,
and an operation of a book is a Lambda function on Step Functions and a call through the `Transport`
elsewhere, on the chobo client you give it. Step Functions and Lambda durable functions need the
`lambda` of a date that is called, and Step Functions the `lambda` of a book (E050).
[Dates and books](dates-and-books.md) has what each platform writes for them, and what to put beside
it.

Lambda durable functions, Argo Workflows and pydantic-graph refuse `event` tasks and a service's
method that asks where a run is too (E050). A workflow that implements a service fills the zero
values protobuf's JSON leaves out of its input, and of a callback's answer that a method of the
service sends, on every platform.

## The clock and the period

Every platform reads the clock once for a statement that reads `now`, so the statement reads one
moment however many times it says `now`, and every try of a call that is tried again gives the
moment of the first. A workflow that says the period it runs in (`now range …`;
[The period a workflow runs in](dates-and-books.md#the-period-a-workflow-runs-in)) holds that moment
to the period before the statement, and the moment a run starts at before anything else.

| | how `now` is read | the check of the period | what E040 counts for it |
|---|---|---|---|
| Temporal | the workflow's clock, read before the call into a variable (`dd_now_<site>`) | an `if` in the workflow's code: `dd.fail` at the start, a `TaskError` before a statement (in Go, `ddFail` and `ddTaskError`) | nothing: the code makes no command |
| Step Functions | the time the state was entered | a Pass that keeps that time in `dd_now_<site>`, and a Choice; outside, a Fail at the start, else a Pass that sets `dd_error` and goes to `on failure` | 6 events for each check |
| Lambda durable functions | a step that reads the clock (`<line> clock`), its answer checkpointed | the same `if` as on Temporal, on the time the step read | 1 operation for each step that reads the clock: one before each statement that reads `now`, and with a period, one at the start too |
| Argo Workflows | a template of its own before the statement (`s<site>-now`), whose output the statement reads | that template (`dd-period-start` at the start), and one that holds its time to the period (`s<site>-period`) and sets `dd_ctl` to `fail` at the start, or to `task` before a statement | 4 nodes for each template that reads the clock, and 4 for each check |
| pydantic-graph | `Deps.clock`, read once at the start of the node | a node (`S<site>_period`): `dd.Failure` at the start, `dd.TaskError` before a statement | nothing (no limit) |

Outside the period at the start, a run calls nothing, as with an input outside its range; before a
statement, it goes to `on failure` as after a task's error nothing handles, and no `on` of a task and
no `retry` takes it. On Argo, the expressions of a template that computes are evaluated each on its
own, and each reads `now()` again: so a statement that reads `now` reads it from the template before
it, and the seconds of `wait until` and of `timeout until` are worked out from one reading, in a
`let`.

## Scenarios and the reference interpreter

```console
$ dandori scenarios <file.flow> [--out <dir>]
$ dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph]
```

`scenarios` writes inputs and scripted answers that together take every arm, every handler, every way
a case can move, and lists that are empty, short, and longer than their loop takes. `run` plays one
through the reference interpreter and prints the calls as the target would make them.
[How it is checked](assurance.md) says how the builds are held to it.

Run as `ritsu dandori scenarios`, with rulec beside it, a call of a rule of the flow's own (not one
called at its Connect service) is not a place to choose an answer: rulec computes it from the
values the call gives, as `ritsu run` does, and the scenario says so with `"rules": "rulec"` and
holds no answer for it. To change what a rule answers, a scenario changes the answers of the
tasks its inputs come from. `ritsu dandori run` computes those calls the same way, so it runs
such a scenario as `ritsu run` does. A scenario written before, which answers the calls of rules
itself, stops `ritsu run` at the first of them, saying so; `ritsu dandori run` still runs it.
