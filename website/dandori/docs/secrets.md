# Secrets

A workflow carries values from one call to the next, and every platform but pydantic-graph keeps
those values in the history of the run, where anyone who may read the executions reads them too.
`dandori check` says when a value the contracts call secret ends up there, when a task sends one to a
party outside the project, when a key is written into the `.flow`, and when a call goes over plain
HTTP. It does so in the `.flow` alone, so `dandori check` says it on its own, and `ritsu check`
prints it as dandori says it.

| Code | What it says | How to say it is meant |
|---|---|---|
| W901 | a key in the shape a provider gives it is written in the `.flow` | `# ritsu: test secret` on the same line |
| W902 | a task calls a URL over plain HTTP, on another machine | `plaintext "<why>"` where the URL is written |
| W904 | a secret is kept in the history of the workflow | `history encrypted` under `workflow` |
| E906 | a task sends a secret outside the project | `discloses <parameter> "<why>"` under the task |

## What is secret

A value is secret when it is read from a place marked secret. There are three ways to mark one:

- **`secret` in the `.flow`**, after the type of an input, an output, a field, a parameter or a
  task's answer (after its `range`, when it has one):

  ```flow
  record Contact
    name  : string
    phone : string  secret

  inputs
    customer_id  : string
    api_token    : string  secret

  task issue_token(user: string) -> string  secret
  ```

- **`debug_redact` in a `.proto`**, written on the field or on the value of an enum that a custom
  option of the field gives it, as protobuf's own debug formats read it. It marks the fields of a
  record made from the message (`bank.Account`), and the parameters and answer of a `connect` task.

  ```proto
  message Account {
    string id = 1;
    string number = 2 [debug_redact = true];
    string holder = 3 [(acme.v1.sensitivity) = PERSONAL];
  }

  enum Sensitivity {
    SENSITIVITY_UNSPECIFIED = 0;
    PUBLIC = 1;
    PERSONAL = 2 [debug_redact = true];
  }
  ```

- **An OpenAPI schema**: a property with `x-data-classification` at `confidential` or `restricted`
  (or with no sensitivity, which is `confidential`), `x-sensitive-data`, or `format: password`. It
  marks the parameters and the answer of a task held to the operation. `writeOnly` alone is not a
  mark, and neither is `internal`.

A record holds the secrets of its fields, a list those of its items, a string with values put in is
secret when one of them is, and a `json` value made from a secret is secret as a whole. What a rule
answers is a decision, and holds none; neither does the answer of a task with no mark. What a
variable holds is gathered from every value put in it anywhere in the flow, as its
[range](tour.md#ranges) is.

## Kept in the history (W904)

Temporal, Step Functions, Lambda durable functions and Argo Workflows keep the inputs and the
outputs of the workflow and of every call. So W904 comes where a secret is an input or an output,
the answer of a task the flow calls, an argument of a call, or the reason of a `fail`. Here the bank's
contract marks the account's number, and the flow writes it into the payout's reference
([tests/fixtures/security/W904_payout_number.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/tests/fixtures/security/W904_payout_number.flow)):

<div class="dd-term" markdown>

```text
warning[W904]: tests/fixtures/security/W904_payout_number.flow:39:1: the secret `account.number` is kept in the history of the workflow, as the argument `reference` of `pay`
    39 |   let paid = pay(account_id: account_id, amount: amount, reference: "Sales payout to {account.number}")
  = The mark is `debug_redact = true` at ../../../examples/payout/specs/payout.proto:34.
  = Temporal, Step Functions, Lambda durable functions and Argo Workflows keep the inputs and outputs of the workflow and of every call in the history, which whoever may read the executions can read.
  = Pass a reference instead (an ID, the name of a secret) and fetch the value inside the task. If the history is encrypted with a key you hold (Temporal: a payload codec; Step Functions and Lambda durable functions: a customer managed KMS key), write `history encrypted` under `workflow`.
```

</div>

The way out is to carry a reference. The payout example
([examples/payout](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/payout/payout.flow))
gives the bank the account's id, and nothing says anything. When the history is encrypted with a key
only those who may read the secrets hold, say so under `workflow`:

```flow
workflow payout v1
  history encrypted
```

| Platform | What `history encrypted` does |
|---|---|
| Temporal (TypeScript, Python, Go) | the worker and the client take a payload codec, by their types: `makeWorker(own, { codec })` and `encryptedClient(connection, codec)`; `connect(target, codec)` in Python; `Dial(options, codec)` in Go. A client made without the codec does not type-check where the generated client and worker take one. The messages and stack traces of failures go through the codec too |
| Step Functions, Lambda durable functions | nothing changes in what dandori writes: the key is a customer managed KMS key set on the state machine (`EncryptionConfiguration`) or on the function (`DurableConfig.KMSKeyArn`), which dandori cannot see; reading the history then takes `kms:Decrypt` |
| Argo Workflows | E050: a Workflow keeps its parameters as they are, and has no way to encrypt them |
| pydantic-graph | nothing: it keeps no history |

## Sent outside the project (E906)

A model's provider (OpenAI, Anthropic, or an Open Responses server that is not this machine), the API
of a decision model (TypeSafe's Jev, OpenAI's Decisions API, or a server of either API that is not
this machine), a host an `http` task names by its URL alone, and an AWS service are outside the
project. A task that
sends one a secret is an error. When it is meant, the task says so, and the history still keeps the
value (W904):

```flow
task draft_notice(amount: money[JPY, incl_tax], payout_id: string, holder: string) -> string
  agent "Draft a short notice to the seller that their payout was sent, …"
  model "gpt-5.4-mini"
  discloses holder "The notice greets the seller by the name on the account; the provider keeps no data under the agreement with it"
```

A secret sent to another file of the project (an OpenAPI document, a `.proto`, a rule's Connect
service, a child `.flow`, a book, a dates file) is for `ritsu check` to hold to the map of contexts:
dandori tells it which calls do so.

## Plain HTTP (W902)

The URLs a task calls are an `http` task's URL, an agent's `url`, `use rule … connect`, and the URL
under `use openapi` and `use proto` (or an OpenAPI document's first server). One that is `http://`,
to a host that is not this machine (`localhost`, `127.0.0.1`, `::1`), is a warning:

<div class="dd-term" markdown>

```text
warning[W902]: tests/fixtures/security/W902_agent_url.flow:7:3: the agent `read` sends its requests to ollama.internal over plain HTTP
     7 |   url "http://ollama.internal:11434/v1"
  = Whoever is on the network between can read and change the requests, the answers, and any key in the headers.
  = Use https://. If the connection is protected another way (a service mesh, a private link), say so under the task with `plaintext "<why>"`.
```

</div>

The inquiry example's version for Temporal reaches its Open Responses endpoint (Ollama, in the
example) inside the cluster, and says so:

```flow
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, take out the order number if one is written, …"
  model "gpt-oss:20b"
  effort low
  url "http://ollama.internal:11434/v1"
  plaintext "The model server is reached only inside the cluster network, which the service mesh encrypts"
```

`plaintext` goes where the URL is written: under the task, under `use rule … connect`, or under the
`use` of the API. Step Functions calls HTTPS alone, so a build for it still refuses `http://` (E050).

## Keys in the file (W901)

A value in the shape of an AWS access key ID, a GitHub, Slack, Stripe, OpenAI, Anthropic or Google
key or token, a Slack incoming webhook URL, or a PEM private key, written anywhere in the `.flow`, in
a string or in a comment, is a warning. The message and the line it shows carry the key's prefix and
length, never the key, so the log of a build does not keep it either:

<div class="dd-term" markdown>

```text
warning[W901]: tests/fixtures/security/W901_key_in_a_string.flow:5:50: a Google API key is written here (AIza…, 39 characters)
     5 |   http GET "https://maps.example.com/v1/find?key=AIza…"
  = A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there.
  = If this key is real, revoke it with Google first: taking it out of the file leaves it in the history of the repository.
  = If it is a value for tests, write `ritsu: test secret` in a comment on the same line.
```

</div>

Every language of ritsu looks for the same shapes, the same way.
