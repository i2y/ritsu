# Install

dandori is one binary. It needs rulec only for a workflow that uses rules (`use rule`); a workflow
without them is checked, built and played without rulec, and its output has nothing of rulec's.

## dandori

Build it from a clone with a recent stable Rust. Its one dependency is serde_json.

```console
$ git clone https://github.com/i2y/dandori
$ cd dandori
$ cargo install --path .
```

## rulec

For a workflow that uses rules, dandori runs rulec to read them, and `rulec gen` writes the code of
each rule, which the generated code calls. dandori finds rulec through `DANDORI_RULEC`, else on the
PATH. With Homebrew, on macOS or Linux:

```console
$ brew install i2y/tap/rulec
```

Or take a binary from rulec's [releases](https://github.com/i2y/rulec/releases). dandori is tested
with rulec 0.20.0 and 0.21.1.

Without rules, a workflow gives up two things. Its branches can only match what its tasks answer (an
enum, a bool, a value that may be absent), since a `.flow` neither compares nor computes: comparing
an amount or a date is left to a task, such as an API, an agent or your own code, and nothing proves
the decision has no gaps. And it has
no cases, since a case follows a rule's state machine, so the checks of cases (E013, E020 to E022,
E030, W101 to W103) have nothing to look at. The review example (`examples/review`) is written without
rules: the task that scores an application answers `approve`, `reject` or `hold`, and the flow
matches that.

## Check that it works

Check one of the examples, then build it:

```console
$ dandori check examples/hotel/temporal/hotel.flow
examples/hotel/temporal/hotel.flow: ok
$ dandori build examples/hotel/temporal/hotel.flow --target temporal --out out/hotel
```

The hotel booking uses rules. Without rulec, check the review example instead:

```console
$ dandori check examples/review/temporal/review.flow
examples/review/temporal/review.flow: ok
```

`--lang ja` (or `DANDORI_LANG=ja`) prints the checker's messages in Japanese.

## The agent skill

`skills/dandori` in the repository is an [Agent Skill](https://agentskills.io) for AI coding agents
such as Claude Code: when to use dandori, the loop from a first draft to a build, the language on
one page, what to ask a person, and the fix for each diagnostic. The pages of this site that it
refers to come with it.

```console
$ cp -r skills/dandori ~/.claude/skills/                  # every project on this machine
$ cp -r skills/dandori <your-project>/.claude/skills/     # one project, committed with it
```

The skill runs `dandori` from the PATH, and `rulec` for a workflow that uses rules.

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
