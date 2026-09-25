# dandori

A small typed language for workflows that call business rules. A `.flow` is checked
before it runs — types, every arm of every match, every state a case can be left in,
retries that could repeat a change, how long the execution history can grow — and
compiled to **AWS Step Functions** (ASL with JSONata), to **Temporal** (TypeScript), to
**AWS Lambda durable functions** (TypeScript) and to **Argo Workflows** (a WorkflowTemplate).

The decisions themselves are written in [rulec](https://github.com/i2y/rulec): tables
that rulec proves complete and free of overlaps. dandori reads them through rulec's
command line, and uses a rule's state machine as the type of the thing a workflow
drives — a Stripe PaymentIntent, an order in a warehouse.

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
  cases when a task fails and nothing handled it.

### What a task calls

| The task says | Step Functions | Temporal | Lambda durable functions | Argo Workflows |
|---|---|---|---|---|
| `lambda "<function>"` | Lambda Task | an activity dandori writes, invoking the function | a step dandori writes, invoking the function | a container running the code dandori writes |
| `http POST "<url>"` | HTTP Task | an activity dandori writes, with `fetch` | a step dandori writes, with `fetch` | the same, with `fetch` |
| `aws sns:publish` | AWS SDK integration | an activity dandori writes, with the AWS SDK | a step dandori writes, with the AWS SDK | the same, with the AWS SDK |
| `state machine "<arn>"` | nested execution (`startExecution.sync:2`) | | | |
| `workflow "<type>"` | | child workflow | | |
| `durable function "<arn>"` | | | invoke of another durable function | |
| `workflow template "<name>"` | | | | a Workflow from that WorkflowTemplate |
| `image "<image>"` | | | | a container of your image |
| none of these | cannot build (E050) | an activity you write (`OwnTasks`) | a step running code you write (`OwnTasks`) | cannot build (E050) |

The code dandori writes for Temporal, durable functions and Argo sends what Step Functions
sends, through a `Transport` (`io.ts`) whose credentials and clients are yours to set.
`queue "<name>"` sends a Temporal activity or child workflow to that task queue. A
`callback` task hands on a token (Step Functions), a callback id (durable functions), or
an id the answer comes back with as a signal (Temporal) or through `argo node set` (Argo);
with `aws sqs:sendMessage` the token travels in the message.

On Argo Workflows every task runs in a container. The code dandori writes for `lambda`,
`http` and `aws` goes into an image built from `caller/`; a task with `image` runs your
image, which reads the call from `DANDORI_CALL`, writes the answer to
`/tmp/dandori/answer.json`, and for a declared error writes `{"error", "message"}` to
`/tmp/dandori/error.json` and exits with 3. The WorkflowTemplate keeps the flow's variables
in global output parameters, and the YAML starts with a comment that names each
variable's parameter.

The complete examples are in [examples/hotel](examples/hotel/hotel.flow),
[examples/order](examples/order/order.flow), [examples/fulfillment](examples/fulfillment/fulfillment.flow),
and [examples/review](examples/review/review.flow), which calls only tasks the user writes
and so is for Temporal, durable functions and Argo (with `image`). The design, in Japanese, is in
[DESIGN.md](DESIGN.md).

## Commands

```
dandori check <file.flow>...
dandori build <file.flow> --target asl|temporal|durable|argo [--out <dir>]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target asl|temporal|durable|argo]
```

`--lang ja` prints the messages in Japanese. rulec is found through `DANDORI_RULEC`, else
on the PATH.

- `build --target asl` writes the state machine and, for every rule it calls, a Lambda
  handler around the Python rulec generates.
- `build --target temporal` writes `workflow.ts`, `types.ts`, `activities.ts`
  (`makeActivities(own, transport)`: the tasks dandori writes, with the ones you write),
  `io.ts` (the `Transport`), `rules.ts` (the rules as activities around rulec's
  TypeScript) and `runtime.ts`.
- `build --target durable` writes `workflow.ts` with `makeHandler(own, transport)`,
  `types.ts`, `tasks.ts`, `io.ts` and `runtime.ts`, and for every rule it calls, the same
  Lambda handler as `asl`, which the durable function invokes.
- `build --target argo` writes `<workflow>.argo.yaml`, a WorkflowTemplate that takes the
  input as the parameter `input` and leaves the outputs in the global parameter
  `dd_output`, and `caller/`: the program that runs the `lambda`, `http` and `aws` tasks and
  the rules in the workflow's containers, with its `package.json` and `Dockerfile`.
- `scenarios` writes inputs and scripted answers that together take every arm, every
  handler, every way a case can move, and lists that are empty, short, and longer than
  their loop takes; `run` plays one through the reference interpreter.

## How the output is checked

The reference interpreter defines what a `.flow` means. The tests generate the scenarios
of every example and run each of them five ways — the reference interpreter, the
generated ASL under JSONata 2.0.6 (`tools/asl-run.mjs`), the generated Temporal workflow
in Temporal's time-skipping test environment (`tools/temporal/run.mjs`), the generated
durable function in the SDK's local test runner (`tools/durable/run.mjs`), and the
generated WorkflowTemplate on Argo Workflows v4.1.4 in a local kind cluster
(`tools/argo/run.mjs`) — and require the same calls, with the same arguments and
idempotency keys, and the same end.
On Temporal and durable functions, the tasks dandori writes run with a stand-in
`Transport` that records what they would send, so the comparison is with what Step
Functions sends; the tasks the user writes, the rules and the child workflows are
stand-ins that answer from the scenario. On Argo, the caller runs in the pods with such a
`Transport`, and the other tasks, the rules and the workflows a task starts are stand-in
containers that ask a mock in the cluster for the scenario's answer. The rounds of a
parallel loop run one at a time in the runners, so the calls come in the reference's order.

The durable functions test runner cannot time a call out on cue, so the scenarios with a
timeout are left out there. Temporal's test environment and Argo cannot time out a task
either, but a callback's timeout can be played: on Temporal by not answering it, on Argo
by answering the wait with what its running out gives. The ASL is also validated with
asl-validator, and the code between each platform and a rule answers every vector
`rulec vectors` produces as rulec says.

```
npm install --prefix tools
npm install --prefix tools/temporal
npm install --prefix tools/durable
sh tools/argo/setup.sh        # a kind cluster with Argo Workflows (docker, kind, kubectl)
DANDORI_RULEC=/path/to/rulec cargo test
```

A test that cannot find rulec, Node, the tools, the cluster or the `argo` command prints a
`SKIP:` line. The runs on Argo take about half an hour.

## Status

Early. Not yet: Parallel with different branches, types of AWS API calls read from the
published Smithy models (the parameters and answers are declared by hand, as for HTTP),
cases the workflow holds itself, a rule's preconditions checked at the task that produced
the value, runs on AWS and on a Temporal server, and the caller image run against real
Lambda, HTTP and AWS endpoints from Argo. The design, the decisions and what is
left are in [DESIGN.md](DESIGN.md).
