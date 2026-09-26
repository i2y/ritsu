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
    95 |   succeed 結果 = 宿泊済
  the run that gets there:
      81  見積 = 与信(…)
      84  match 見積.扱い: 自動
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_capture
      90  match pi.status: requires_capture
      90  wait until 予約.チェックアウト
      93  capture_intent: pi requires_capture → processing
          `settle` happens on the other side: pi processing → requires_payment_method
      95  succeed
```

The transitions come from `payment_intent.rule`, a transcription of Stripe's documents
into a rulec state machine. The workflow says which events happen on their own
(`external authenticate, settle, expire`), and the checker follows them too: waiting
until check-out, the authorization can expire, and then the capture is refused.

## A workflow

```
case pi : PaymentIntent follows payment_intent.payment
  held capture_method = manual
  held confirmation_method = automatic
  external authenticate, settle, expire
  refused when refused = true

flow
  let 見積 = 与信(客室: 予約.客室, 泊数: 予約.泊数)
  match 見積.扱い
    確認 => succeed 結果 = 確認待ち
    自動 => pi <- create_intent(amount: 見積.与信額, currency: "jpy", payment_method: 予約.カード, capture_method: manual)
  pi <- confirm_intent(id: pi.id)
    on card_declined => pi <- get_intent(id: pi.id)
  …
```

Lists, values that may be absent, and anything a task can call
([examples/fulfillment](examples/fulfillment/fulfillment.flow)):

```
task 梱包を待つ(QueueUrl: string, MessageBody: 梱包の依頼) -> 梱包
  aws sqs:sendMessage
  callback
  timeout 2 days

flow
  let 結果 = for 明細 in 注文.明細 at most 50 in parallel, 10 at a time
    let r = 在庫を引き当てる(sku: 明細.sku, 数: 明細.数)
    yield r
  …
  match 注文.贈り物
    some 贈り物 => let 宛名 = 贈り物.宛名
    none => pass
  let 箱 = 梱包を待つ(QueueUrl: "https://sqs.…/packing", MessageBody: {注文ID: 注文.id, 引当: 結果})
    on timeout => fail PackingLate "二日たっても梱包の知らせがありません"
  知らせる(TopicArn: "arn:aws:sns:…:orders", Message: "注文 {注文.id} を{判定.便}で出しました")
```

- Conditions and arithmetic live in rulec rules. A `.flow` builds values — records,
  lists, strings with values put in — but has no comparison or arithmetic, and branches
  only by matching an enum, a bool, or a value that may be absent (`none` / `some x`).
- Types: `int`, units such as `money[円, incl_tax]`, `string`, `bool`, `timestamp`, enums,
  records, `list[T]`, `T?` for a value that may be absent, and `json` for a value that is
  passed along without being looked into.
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

### What a task calls

| The task says | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| `lambda "<function>"` | Lambda Task | an activity dandori writes, invoking the function | a step dandori writes, invoking the function | a container running the code dandori writes | a function dandori writes, invoking the function |
| `http POST "<url>"` | HTTP Task | an activity dandori writes, with `fetch` | a step dandori writes, with `fetch` | the same, with `fetch` | the same, with urllib |
| `aws sns:publish` | AWS SDK integration | an activity dandori writes, with the AWS SDK | a step dandori writes, with the AWS SDK | the same, with the AWS SDK | the same, with boto3 |
| `agent "<instructions>"` and `model "<model>"` | HTTP Task to OpenAI's Responses API | an activity dandori writes, with OpenAI's Agents SDK | a step dandori writes, with the Agents SDK | the same, with the Agents SDK | the same, with the Agents SDK for Python |
| `agent claude "<instructions>"` and `model "<model>"` | HTTP Task to Claude's Messages API | an activity dandori writes, with Anthropic's SDK | a step dandori writes, with Anthropic's SDK | the same, with Anthropic's SDK | the same, with Anthropic's SDK for Python |
| `state machine "<arn>"` | nested execution (`startExecution.sync:2`) | | | | |
| `workflow "<type>"` | | child workflow | | | |
| `durable function "<arn>"` | | | invoke of another durable function | | |
| `workflow template "<name>"` | | | | a Workflow from that WorkflowTemplate | |
| `image "<image>"` | | | | a container of your image | |
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
an id the answer comes back with as a signal (Temporal) or through `argo node set` (Argo);
with `aws sqs:sendMessage` the token travels in the message.

On Argo Workflows every task runs in a container. The code dandori writes for `lambda`,
`http`, `aws` and `agent` goes into an image built from `caller/`; a task with `image` runs your
image, which reads the call from `DANDORI_CALL`, writes the answer to
`/tmp/dandori/answer.json`, and for a declared error writes `{"error", "message"}` to
`/tmp/dandori/error.json` and exits with 3. The WorkflowTemplate keeps the flow's variables
in global output parameters, and the YAML starts with a comment that names each
variable's parameter.

### Agents

An `agent` task gives a model its arguments and takes back a value of the task's type. The
model reads and writes; the rules decide. [examples/inquiry](examples/inquiry/inquiry.flow)
reads a customer's message with an OpenAI agent, routes it with a rulec rule, and drafts the
reply with a Claude one:

```
task 読み取る(本文: string) -> 読み取り
  agent "お客さまからの問い合わせの本文を読み、種類を一つ選び、注文番号が書かれていれば取り出し、…"
  model "gpt-5.4-mini"
  connection "arn:aws:events:…:connection/openai/…"
  timeout 60 seconds
  retry 2 times every 10 seconds

task 下書きする(種類: 振り分け.種類, 要点: string, 注文ID: string?, 期限: duration[h]) -> string
  agent claude "問い合わせへの最初の返事を、丁寧な日本語で三文以内に下書きしてください。…"
  model "claude-sonnet-5"
  connection "arn:aws:events:…:connection/claude/…"
  timeout 60 seconds

flow
  let 読 = 読み取る(本文: 問い合わせ.本文)
    on failure => …
  let 判定 = 振り分け(種類: 読.種類, 会員: 問い合わせ.会員)
```

- The answer's type becomes a JSON Schema in the strict form of OpenAI's Structured Outputs —
  every field of a record required, `T?` a choice with null, an enum its values — around
  `{"answer": …}`, since the top must be an object. Claude's structured outputs take the same
  schema. The answer is then checked against the type like any other.
- Every target asks the model the same thing: the instructions, the arguments as the same
  JSON text, and the schema, and no model settings — dandori adds none, and keeps the Agents
  SDK from adding its defaults. Step Functions sends it to the Responses API from an HTTP
  Task, with the API key in the EventBridge connection. The code dandori writes for the other targets runs it with
  OpenAI's Agents SDK through the `Transport`, which reads `OPENAI_API_KEY`, or takes a run
  configuration of your own (another model provider, for one).
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
  that may be absent. Step Functions refuses an agent without `connection`, and an HTTP Task
  whose `timeout` is over the 60 seconds it gives a request (E050).

The complete examples are in [examples/hotel](examples/hotel/hotel.flow),
[examples/order](examples/order/order.flow), [examples/fulfillment](examples/fulfillment/fulfillment.flow),
[examples/inquiry](examples/inquiry/inquiry.flow), and [examples/review](examples/review/review.flow),
which calls only tasks the user writes and so is for Temporal, durable functions and Argo
(with `image`). The design, in Japanese, is in [DESIGN.md](DESIGN.md).

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
  query `dandori.status`: the line the workflow waits at, and each case's state). Started with
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

## How the output is checked

The reference interpreter defines what a `.flow` means. The tests generate the scenarios
of every example and run each of them seven ways — the reference interpreter, the
generated ASL under JSONata 2.0.6 (`tools/asl-run.mjs`), the generated Temporal workflow
on a Temporal server (the Temporal CLI's dev server), in TypeScript (`tools/temporal/run.mjs`)
and in Python (`tools/temporal-python/run.py`), the generated durable function in the
SDK's local test runner (`tools/durable/run.mjs`), the generated WorkflowTemplate on Argo
Workflows v4.1.4 in a local kind cluster (`tools/argo/run.mjs`), and the generated graph
with pydantic-graph 2.51.0 (`tools/pydantic-graph/run.py`) — and require the same calls,
with the same arguments and idempotency keys, and the same end. No two values in a
scenario are alike, so a target that mixes up two answers or the rounds of a loop shows it.
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
its activity until the server cancels that too.

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
without a retry. Nothing goes to OpenAI or Anthropic.

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
sh tools/argo/setup.sh        # a kind cluster with Argo Workflows (docker, kind 0.33+, kubectl)
DANDORI_RULEC=/path/to/rulec cargo test
```

A test that cannot find rulec, Node, the tools, the cluster or the `argo` command prints a
`SKIP:` line. The whole `cargo test` takes a minute and a half to a little over two minutes; `tools/argo/setup.sh`
sets Argo's controller up for it (it looks at a workflow again a second after a change, not
ten) on the node image of kind 0.33.0. When the platform could not run one of a real run's
pods (it ended in Error, or Unknown with exit code 255, as containerd in the node image of
kind 0.29.0 made them now and then under load), the Argo runner plays the run again, at most
twice, and the test says so. `DANDORI_FLOW=<part of a path>`
runs only the flows whose path has it.

## Status

Early. Not yet: Parallel with different branches, types of AWS API calls read from the
published Smithy models (the parameters and answers are declared by hand, as for HTTP),
cases the workflow holds itself, a rule's preconditions checked at the task that produced
the value, runs on AWS and on a Temporal server, the caller image run against real
Lambda, HTTP and AWS endpoints from Argo, and agents run against OpenAI and Anthropic themselves. The design, the decisions and what is
left are in [DESIGN.md](DESIGN.md).
