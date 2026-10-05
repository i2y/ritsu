# Install

dandori is one of the languages of [ritsu](https://github.com/i2y/ritsu), and is built from its
repository. A workflow that uses rules (`use rule`) runs as `ritsu dandori`, which reads the rules
with rulec in the same process, and its dates files and books (`use dates`, `use book`) with koyomi
and chobo; a workflow without them also runs with the `dandori` command alone, and its output has
nothing of rulec's.

To try it before installing anything, open [Try it in the browser](../playground/#flow=tests/fixtures/hotel_naive.flow)
on ritsu's site: dandori runs in the page, with the rules, the dates files and the books a flow reads.

## ritsu

With a recent stable Rust. dandori's one outside dependency is serde_json.

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu dandori <command>` is dandori's command, with the rules read by rulec: `ritsu dandori check`,
`ritsu dandori build`, and the rest. The package `dandori` in place of `ritsu` installs the `dandori`
command alone, for workflows without rules, dates files or books; given one that uses them, it says
to run it with `ritsu dandori`.

## rulec

`rulec gen` writes the code of each rule, which the generated code calls, and a Connect service for
it, which a workflow can call instead. ritsu runs it as `ritsu rulec gen`; the package `rulec`
installs rulec alone:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked rulec
```

Without rules, a workflow gives up two things. Its branches can only match what its tasks answer (an
enum, a bool, a value that may be absent), since a `.flow` neither compares nor computes: comparing
an amount or a date is left to a task, such as an API, an agent or your own code, and nothing proves
the decision has no gaps. And it has
no cases, since a case follows a rule's state machine, so the checks of cases (E013, E020 to E022,
E030, W101 to W103) have nothing to look at. The child flow of the fulfillment example
(`examples/fulfillment/arrange_delivery.flow`) is written without rules: it branches on the carrier
it is given, an enum among its inputs.

## Check that it works

The examples are in the repository, under `crates/dandori/examples`. In a clone of it
(`git clone https://github.com/i2y/ritsu`), in `crates/dandori`, check one of them, then build it:

```console
$ ritsu dandori check examples/hotel/temporal/hotel.flow
examples/hotel/temporal/hotel.flow: ok
$ ritsu dandori build examples/hotel/temporal/hotel.flow --target temporal --out out/hotel
```

The hotel booking uses rules. With `dandori` alone, check a flow without rules instead:

```console
$ dandori check examples/fulfillment/arrange_delivery.flow
examples/fulfillment/arrange_delivery.flow: ok
```

`--lang ja` (or `DANDORI_LANG=ja`, or `RITSU_LANG=ja`) prints the checker's messages in Japanese.

## The agent skill

`skills/dandori` at the root of ritsu's repository is an [Agent Skill](https://agentskills.io) for AI coding agents
such as Claude Code: when to use dandori, the loop from a first draft to a build, the language on
one page, what to ask a person, and the fix for each diagnostic. The pages of this site that it
refers to come with it.

The `ritsu` binary carries it, with the skills of ritsu and of the other languages:

```console
$ ritsu skills install dandori                                # one project: its .claude/skills/
$ ritsu skills install dandori --user                         # every project on this machine
$ ritsu skills install dandori --dir path/to/skills           # where another agent reads skills
```

In Claude Code, the plugin `ritsu` holds all eight: `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json`, then
`/plugin install ritsu@ritsu`. From a clone of the repository, copying `skills/dandori` into
`~/.claude/skills/` or a project's `.claude/skills/` does the same, and every release has the eight
in `ritsu-skills-v<version>.zip`.

The skill runs `ritsu dandori` from the PATH for a workflow that uses rules, and `dandori` alone for one without.

## What the output needs

The code a build writes imports each platform's own SDK, and nothing of dandori's:

| Target | Runs with |
|---|---|
| `temporal` | Temporal's TypeScript SDK (`@temporalio/*`); the AWS SDK, OpenAI's Agents SDK or Anthropic's SDK only if a task calls through them |
| `temporal-python` | Temporal's Python SDK (`temporalio`); boto3 and the agents' SDKs the same way |
| `temporal-go` | Temporal's Go SDK (`go.temporal.io/sdk`, which asks for Go 1.26); the AWS SDK for Go v2, OpenAI's Go client or Anthropic's Go SDK only if a task calls through them |
| `asl` | AWS Step Functions; a rule is a Lambda function around the Python rulec generates, or an HTTP Task to its service |
| `durable` | AWS Lambda durable functions (`@aws/durable-execution-sdk-js`) |
| `argo` | Argo Workflows, and the image built from the generated `caller/` |
| `pydantic-graph` | pydantic-graph 2.x, in your own Python process |

[Build for a platform](platforms.md) says what each target writes.
