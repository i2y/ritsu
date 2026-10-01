# How it is checked

The reference interpreter defines what a `.flow` means. The tests generate the scenarios of every
example and run each of them eight ways: in the reference interpreter, and in what each platform
runs. Every way must make the same calls, with the same arguments and idempotency keys, and end the
same way.

| Way | Where it runs |
|---|---|
| the reference interpreter | `dandori run` |
| the ASL, under JSONata 2.0.6 | `tools/asl-run.mjs` |
| the ASL, on LocalStack's Step Functions | `tools/localstack/run.mjs` |
| the Temporal workflow in TypeScript, on a Temporal server | `tools/temporal/run.mjs` (the Temporal CLI's dev server) |
| the Temporal workflow in Python, on a Temporal server | `tools/temporal-python/run.py` |
| the durable function, in the SDK's local test runner | `tools/durable/run.mjs` |
| the WorkflowTemplate, on Argo Workflows v4.1.4 in a local kind cluster | `tools/argo/run.mjs` |
| the graph, with pydantic-graph 2.51.0 | `tools/pydantic-graph/run.py` |

## The scenarios

A scenario is an input and the answers the calls get, in order. The scenarios take every arm, every
handler, every way a case can move, and lists that are empty, short, and longer than their loop
takes. No two values in a scenario are alike, so a platform that mixes up two answers, or the rounds
of a loop, shows it (a number with a range is picked inside it, where two may meet). The scenarios also
give answers every platform must refuse: of the wrong shape, with a state the machine does not lead
to, and with a number outside its range. For a workflow that implements a service, they also give an
input, and values of events and answers of callbacks that the service sends, with every field at its
zero value left out, as protobuf's JSON writes them; a platform that does not fill them in reads
another input, or another answer, from the reference's. And when the workflow reads a field that
says whether it is set with an input that is not `T?`, one input goes without it, and every
platform must end that run at once, with `Dandori.BadInput`.

On Temporal, durable functions and pydantic-graph, the tasks dandori writes run with a stand-in
`Transport` that records what they would send, so what they send is compared with what Step Functions
sends. The tasks you write, the rules and the child workflows are stand-ins that answer from the
scenario. The rounds of a parallel loop run one at a time in the runners, so the calls come in the
reference's order.

## Temporal

Every run of a flow goes at once, each as a workflow of its own id, which the idempotency keys carry.

- The runners make their workers and start their runs with the generated `worker` and `client`,
  answer callbacks with its Update (and see that a second answer is refused), and check what the query
  and the search attribute say of the cases at the end.
- Every run's history is replayed with the same code. The histories kept in `tests/histories` are
  replayed with the code dandori writes now, so a change of the generator that would break a running
  workflow shows.
- Two builds that differ in one text run as versions of one deployment under Worker Deployment
  Versioning, and a run that started on the first ends on it, the round it goes on to in a new run
  too.
- The server keeps real time, so the copy of the code the runners run waits at most 10 ms on a timer
  and gives an activity 5 seconds; a call the scenario times out is kept busy until the server times it
  out. The copy also counts every history as long, so a loop at the top of the flow goes on in a new
  run at every round but the first, and the runners follow each run to the next to replay them all.
- Every flow also runs with its workflow in one language and its activities in the other, both ways.
- In a flow with `on cancel`, the scenarios also cancel the workflow during a call.
- A workflow that runs another `.flow` as its child also runs with the child's generated workflow in
  place of a stand-in, in one language and in the two crossed.

## Step Functions

The ASL runs under JSONata 2.0.6 in `tools/asl-run.mjs`, and is validated with asl-validator.

It also runs on LocalStack's Step Functions, whose JSONata is the Java implementation: the community
image of 4.14.0 in Docker, the last that starts without an account. Every call's answer is mocked
through LocalStack's mocked service integrations, each run is a test case, and the calls, the waits
and the end are read from the execution's history. The rounds of a Map run one at a time, a Wait goes
on at once and keeps what it would have waited for in a variable of its own, and a retry really waits,
timed from the history. A run whose retry waits alone came out long, as they do on a loaded machine,
is played once more.

That version of LocalStack knows neither the HTTP Task nor its errors, and its mock file cannot throw
Step Functions' own errors. So the runner gives an HTTP Task the resource of an AWS SDK integration
whose answers LocalStack hands on as they are, and throws Step Functions' errors under another name,
which the Retry and the Catch use too: a timeout, an HTTP Task's error, and the
`States.QueryEvaluationError` that an agent's refusal fails its Task with (that version would fail the
whole execution with `States.Runtime` instead).

## Argo Workflows

The controller runs the generated WorkflowTemplate, but the runner plays the pods: each pod waits for
a scheduler the cluster does not have, and the runner does what its container and Argo's executor
would do. It runs the generated caller (with the stand-in `Transport`) or a stand-in for the other
tasks, reports the outputs the template declares, and ends the pod with the exit code and the
termination message the container would leave. No container starts, so every run of every flow goes
at once. One run of each flow is played again with real pods, the caller and the stand-ins in
node:24-alpine answered by a mock in the cluster, so that the containers' side is run too. When the
platform could not run one of a real run's pods (it ended in Error, or Unknown with exit code 255, as
containerd in the node image of kind 0.29.0 made them now and then under load), the runner plays the
run again, at most twice, and the test says so.

## Agents, Jev and the default Transport

- An agent's call is recorded as the `Transport` gets it, and the stand-in answers `{"answer": …}` as
  the model would. The ASL runner answers an HTTP Task to the Responses API or the Messages API with a
  response of the API's shape, and plays a failure as the model refusing, so the state machine's own
  reading of the answer is what fails the Task. Claude agents get answers whose enum values are in
  another case, and every platform must take them as the values.
- The agent call of the default `Transport`, in TypeScript and in Python, runs with OpenAI's Agents
  SDK and a scripted model in place of OpenAI's, and with Anthropic's SDK against a stand-in of the
  Messages API on this machine: the model must be asked what Step Functions asks for the same call,
  and an error status must fail the call without a retry. An agent on another server of Open
  Responses goes to a stand-in of that server, which must get the very request Step Functions sends.
  Nothing goes to OpenAI or Anthropic. When Ollama runs on this machine, one call of each such agent
  also goes to it for real, without its effort (a model that does not reason is refused one), and each
  answer must fit the task's type (`DANDORI_OLLAMA` names where it runs, `DANDORI_OLLAMA_MODEL` the
  model; else the smallest one it has).
- The rest of the default `Transport` (`fetch` and the AWS SDK in TypeScript, the standard library and
  boto3 in Python) sends every HTTP, Lambda and AWS call of the scenarios to stand-ins on this machine:
  a server that answers HTTP and Lambda's Invoke, and moto for SNS and SQS. What arrives must be the
  call, in the same text from both languages, and an AWS error must come back by the name the task
  declares (`NotFoundException`).
- A rule called at its service (`connect` under `use rule`) is an HTTP request on every platform, and the
  scenarios answer it as the service writes: the record they chose as protobuf's JSON, with the zero
  values left out and the numbers as strings. They also answer with the false and the 0 left out (and an
  enum at value 0, when value 0 is one of the rule's values), with an enum left out whose value 0 is
  none of them, with a number outside its range, with a body of the wrong shape, and with a failure.
  Every platform must read the answer back as the rule's record, or end the call as the reference does.
- What dandori reads from `rulec api` for a rule's service (the path, the fields of the request and the
  response by their JSON names and kinds, each enum's values and its value 0, and the zero values the
  response leaves out) is compared with the `.proto` that `rulec gen` writes for the same rule, read by
  dandori's own reader, for every rule of the examples, and for two whose enum is a `.proto`'s
  (`import proto`): one whose values carry the contract's prefix, and one whose values have none and
  whose value 0 is a value of its own (`ACTIVE = 0`).
- The service `rulec gen` writes for each of these rules runs as it is (`--http`, the standard library's
  server, with the stubs that buf writes with the plugins of `tools/connect/.venv`), and every vector
  `rulec vectors` writes is sent to it as dandori writes a request, every input written out. What
  dandori reads of the answer must be the vector's output, and the header `rulec-source-sha256` of an
  answer must be the rule's `source_sha256`. An input above a range, a name no value of the enum has, a
  field the request does not have and a request without one of its inputs must each be refused with
  `invalid_argument` and the status 400. The 350 vectors of the 13 rules were sent, 114 of them with an
  input at its zero value, and the services refused 9 inputs above a range, 11 names, 13 fields and 13
  requests without an input. With rulec 0.21.2 and before, whose `rulec api` does not say what a service
  calls the values of its enums, the two rules of `.proto` enums are skipped, and so are the field and
  the input left out, which the services of those rulecs do not refuse.
- What a service answers is read as the rule's record in four places: the reference interpreter, the
  TypeScript and the Python that dandori writes, and the JSONata of the state machine. 200 answers of
  four rules must be read alike in all four, among them the ones a service would not write: a number
  that is not a decimal or is more than 2^53 − 1, a name that no enum has (`constructor`, `__proto__`),
  a field of another kind, a null, a body that is not an object. None of them may raise.
- A Jev task's call is an HTTP request, and the stand-in answers it with Jev's response as TypeSafe's
  API reference shows it: for each question the choice, the score or the probability of yes, with how
  sure Jev is. The scenarios answer each Jev call exactly as sure as the task's `confidence` asks, then
  just under it, and with an answer that does not fit the type, so every platform's reading of the
  answer and the edge of the confidence are compared with the reference's. A call whose answer is less
  sure than asked leaves the variable as it was, and the next call sends it, so a platform that kept the
  answer would show. The default `Transport`'s request to Jev goes to the stand-in server too, where
  TypeSafe's key from `TYPESAFE_API_KEY` must arrive as `Authorization: Bearer <key>`.
- When `TYPESAFE_API_KEY` is set, the first call of each Jev task of the examples and the test flows
  also goes to TypeSafe for real, from the default `Transport` of TypeScript and of Python. The
  response must answer every question, and each answer must read into the task's type, or fail the
  call with the task's error when Jev is not sure enough. Without the key, nothing goes to TypeSafe.

## The services

What a workflow that implements a service takes and answers is held to protobuf itself. Each
`.proto` of a service is built by protoc into descriptors, with the repository's
`proto/dandori/v1/options.proto` as the file of dandori's options, and protobuf's Python
(`json_format`, which refuses a field it does not know) reads with them every input the scenarios
give, as the request of the method that starts a run; every output the reference interpreter ends a
run with, as its response; every value of an event and every answer of a callback a method sends, as
its request; and what the query `dandori.status` answers, as `dandori.v1.Status`. An input that
protobuf writes back, its zero values left out, fills in to the input again. dandori's options pass
buf's standard lint, and every `.proto` of a service in the examples and the tests builds with
protoc.

## The pictures

What `dandori doc` writes for the examples, the flows of tests/flows and a first draft with errors
is held word for word to golden files, and the pages of the examples on this site to what it writes
now; the rules they call are in them as `rulec doc` renders them. On every scenario, the way a run
lights up must hold together: every step it lit is reached by an edge it lit, and every arm and
handler it took, and every way round and out of a loop, lights up an edge. Every Mermaid chart is
drawn by Mermaid 11 and 12 in headless Chrome, rulec's charts of its state machines among them, and
in Chrome a page lights up what its data says and opens the page `rulec doc` renders for each of its
rules.

## The playground

[Try it in the browser](playground.md) runs dandori compiled to wasm32, and reads what the examples
read from `presets.json`: their files, and what rulec printed for their rules, `rulec doc` among it.
Both are committed, and both are held to the repository.

- `presets.json` must be what checking the examples reads now, and what rulec prints for their rules
  now.
- For every flow the page opens, the command, reading the disk and running rulec, must print and
  write what the page answers from `presets.json`: `check`, `build` for all six targets, and `doc` in
  both formats.
- The rules tab has no command to be held to: what it answers from `presets.json` must be what it
  answers reading the disk and running rulec, and every rule must have its page.
- The module must answer every request as the library does: the flows as they are, and edits that
  reach what they do not (a flow that does not parse, a rule and a child flow the page does not have,
  a flow that runs itself).
- In Chrome, the page in each language must start, show what `check` prints for the draft, follow a
  link to a flow, a tab and a platform, and show each rule's text with a link to its page.

## What is left out

The durable functions test runner cannot time a call out on cue, so the scenarios with a timeout are
left out there. Argo and the graph's runner cannot time out a task either, but a callback's timeout can
be played: on pydantic-graph by not answering it, on Argo by answering the wait with what its running
out gives. On Temporal, both can. The TypeScript for Temporal, durable functions and Argo's caller also
passes `tsc --strict`, and the code between each platform and a rule answers every vector
`rulec vectors` produces as rulec says.

Nothing here runs on AWS itself, on a production Temporal cluster or Temporal Cloud, or against real
OpenAI and Anthropic APIs.

## Running the tests

```console
$ npm install --prefix tools
$ npm install --prefix tools/temporal
$ npm install --prefix tools/durable
$ uv venv --python 3.13 tools/temporal-python/.venv
$ uv pip install --python tools/temporal-python/.venv/bin/python -r tools/temporal-python/requirements.txt
$ uv venv --python 3.13 tools/pydantic-graph/.venv
$ uv pip install --python tools/pydantic-graph/.venv/bin/python -r tools/pydantic-graph/requirements.txt
$ npm install --prefix tools/agents
$ uv venv --python 3.13 tools/agents/.venv
$ uv pip install --python tools/agents/.venv/bin/python -r tools/agents/requirements.txt
$ npm install --prefix tools/wire
$ uv venv --python 3.13 tools/wire/.venv
$ uv pip install --python tools/wire/.venv/bin/python -r tools/wire/requirements.txt
$ uv venv --python 3.13 tools/connect/.venv
$ uv pip install --python tools/connect/.venv/bin/python -r tools/connect/requirements.txt
$ npm install --prefix tools/mermaid
$ sh tools/argo/setup.sh        # a kind cluster with Argo Workflows (docker, kind 0.33+, kubectl)
$ docker pull localstack/localstack:4.14.0
$ DANDORI_RULEC=/path/to/rulec cargo test
```

Run the tests with rulec 0.22.0 or later. The golden files of `dandori doc`, the pages of the site's
examples and the playground's `presets.json` hold what rulec prints for the rules, `rulec doc` among it,
and its version number is in that. With rulec 0.21.2 or before, three tests fail on that alone: the two
that compare `dandori doc` with those files, and the one that compares the playground's answers with
the command's. The tests that need `connect.enums`, which 0.22.0 is the first to print, print a `SKIP:`
line.

A test that cannot find rulec, Node, the tools, buf, protoc, the cluster, the `argo` command, the image of
LocalStack or Chrome prints a `SKIP:` line and passes, so read the output with `-- --nocapture`. The whole
`cargo test` takes two to four minutes; `tools/argo/setup.sh` sets Argo's controller up for it, to
look at a workflow again a second after a change rather than ten, on the node image of kind 0.33.0.
`DANDORI_FLOW=<part of a path>` runs only the flows whose path
has it, and `DANDORI_BLESS=1` rewrites the golden files and the site's pages of the examples, and records the kept histories and the playground's `presets.json` anew.
After a change to what `check`, `build` or `doc` answers, `website/tools/make_wasm.sh` builds the
playground's module again (it needs the `wasm32-unknown-unknown` target of rustup).
