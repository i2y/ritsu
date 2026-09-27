# Install

dandori is one binary. It reads the rules through rulec, so it needs rulec beside it.

## dandori

Build it from a clone with a recent stable Rust. Its one dependency is serde_json.

```console
$ git clone https://github.com/i2y/dandori
$ cd dandori
$ cargo install --path .
```

## rulec

dandori runs rulec to read the rules a workflow calls. It finds rulec through `DANDORI_RULEC`,
else on the PATH. With Homebrew, on macOS or Linux:

```console
$ brew install i2y/tap/rulec
```

Or take a binary from rulec's [releases](https://github.com/i2y/rulec/releases). dandori is tested
with rulec 0.20.0 and 0.21.1.

## Check that it works

Check one of the examples, then build it:

```console
$ dandori check examples/hotel/temporal/hotel.flow
examples/hotel/temporal/hotel.flow: ok
$ dandori build examples/hotel/temporal/hotel.flow --target temporal --out out/hotel
```

`--lang ja` (or `DANDORI_LANG=ja`) prints the checker's messages in Japanese.

## What the output needs

The code a build writes imports each platform's own SDK, and nothing of dandori's:

| Target | Runs with |
|---|---|
| `temporal` | Temporal's TypeScript SDK (`@temporalio/*`); the AWS SDK, OpenAI's Agents SDK or Anthropic's SDK only if a task calls through them |
| `temporal-python` | Temporal's Python SDK (`temporalio`); boto3 and the agents' SDKs the same way |
| `asl` | AWS Step Functions; a rule is a Lambda function around the Python rulec generates |
| `durable` | AWS Lambda durable functions (`@aws/durable-execution-sdk-js`) |
| `argo` | Argo Workflows, and the image built from the generated `caller/` |
| `pydantic-graph` | pydantic-graph 2.x, in your own Python process |

[Build for a platform](platforms.md) says what each target writes.
