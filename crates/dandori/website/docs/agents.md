# Agents

An `agent` task gives a model its arguments and takes back a value of the task's type, which the
flow can match like any other answer, or give to a rule. The inquiry example leaves the reading and
the writing to models and the routing to a rule:
[examples/inquiry](https://github.com/i2y/dandori/blob/main/examples/inquiry/temporal/inquiry.flow)
reads a customer's message with a model behind the company's own Open Responses endpoint (Ollama, in
the example), routes it with a rulec rule, and drafts the reply with Claude. The kind it routes by is
[Jev](jev.md)'s, and the model's only when Jev is not sure of its own:

```flow
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, take out the order number if one is written, …"
  model "gpt-oss:20b"
  effort low
  url "http://ollama.internal:11434/v1"
  timeout 60 seconds
  retry 2 times every 10 seconds

task draft_reply(kind: routing.kind, point: string, order_id: string?, within: duration[h]) -> string
  agent claude "Draft the first reply to the inquiry, politely, in three sentences at most. …"
  model "claude-sonnet-5"
  effort medium
  timeout 60 seconds

flow
  let reading = read_inquiry(text: inquiry.text)
    on failure => …
  let kind = pick_kind(text: inquiry.text)
    on unsure, failure => let kind = reading.kind
  let decision = routing(kind: kind, member: inquiry.member)
```

## The answer is typed

The answer's type becomes a JSON Schema in the strict form of OpenAI's Structured Outputs: every
field of a record required, `T?` a choice with null, an enum its values, around `{"answer": …}`,
since the top must be an object. Claude's structured outputs take the same schema. The answer is
then checked against the type like any other answer, and one that does not fit fails the call.

Every platform asks the model the same thing: the instructions, the arguments as the same JSON text,
the schema, and the effort when the task gives one (below). dandori adds no other model settings, and
keeps the Agents SDK from adding its defaults.

## How hard the model reasons

`effort <level>` asks the model to reason less or more before it answers. Without it, the model does
what it does by default. The level goes where each API takes it, and every platform sends it the same
way:

| Agent | Where the level goes | Levels |
|---|---|---|
| OpenAI, and an Open Responses endpoint (`url`) | `reasoning.effort` of the Responses API | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` |
| Claude | `output_config.effort` of the Messages API | `low`, `medium`, `high`, `xhigh`, `max` |

A level the provider does not take is refused by the checker (E007). Whether a model reasons at all
is the model's to say, and a model that does not fails the call when it is sent an effort. Ollama, for
one, refuses it for such a model ("does not support thinking"), and the call fails as any failed call
does.

## Who is called

- **OpenAI**, without `url`. Step Functions sends the request to the Responses API from an HTTP Task,
  with the API key in the EventBridge connection the task names (`connection`). The code dandori
  writes for the other platforms runs it with OpenAI's Agents SDK through the `Transport`, which reads
  `OPENAI_API_KEY`, or takes a run configuration of your own. OpenAI has no Agents SDK for Go: the Go
  build sends the request Step Functions sends to the Responses API, with OpenAI's Go client.
- **Any Open Responses endpoint**, with `url "<base>"`. Open Responses is the open specification of
  OpenAI's Responses API, which OpenAI, Hugging Face, OpenRouter, Ollama, vLLM, LM Studio and Vercel
  took up in January 2026. The call goes to `<base>/responses` at that endpoint as the same request
  Step Functions sends, which the code dandori writes sends over HTTP with no SDK, since an SDK may
  send what the specification does not have. The server's credentials come from the `Transport`'s
  headers. Its limits on a schema are its own, so the checker holds the answer to OpenAI's limits only
  when the call goes to OpenAI.
- **Claude**, with `agent claude "…"`. It gets the instructions as the system prompt, the arguments'
  JSON text as the user's message, and the schema as `output_config.format`, with `max_tokens` 16000.
  Step Functions sends it from an HTTP Task, with the key in the connection as `x-api-key`; the other
  platforms use Anthropic's SDK, which reads `ANTHROPIC_API_KEY`. Claude may give an enum's value in
  another case, so a Claude agent's enum values are taken without regard to case, and an enum whose
  values differ only in case cannot be in such an answer (E007).

## Failures and retries

- An agent changes nothing on the other side, so it takes no `key`, and retrying it is always safe.
  It declares no errors: a refusal, or a call that fails, is `failure`.
- Neither SDK's client retries by itself in the default `Transport`. The workflow retries, as the
  task's `retry` says, as Step Functions does.
- The checker refuses an answer the schema cannot say (`json`, a record that contains itself through
  others) or one larger than the provider takes (E007): for Claude, more than 16 values that may be
  absent.
- Step Functions refuses an agent without `connection`, an HTTP Task whose `timeout` is over the 60
  seconds it gives a request, and one that is not sent over HTTPS (E050). The HTTP Task calls only a
  server under a public name with a publicly trusted certificate, even for a private API. A connection always holds
  a key, so give it one even for a server that wants none.
