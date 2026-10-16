# Install

chobo is one of the languages of [ritsu](https://github.com/i2y/ritsu), and comes with it: a release
of ritsu holds one binary, `ritsu`, with a link to it named for each of its languages, and called as
`chobo` it is chobo. `ritsu chobo <command>` is the same command. chobo reads no other language, so
it can also be built alone.

To try it before installing anything, open [Try it in the browser](../playground/#project=chobo/refunds)
on ritsu's site: ritsu runs in the page, and each example book opens there to be checked, built and
drawn as you edit it.

## A release of ritsu

Every release publishes `ritsu` for macOS (arm64, x64) and Linux (x64, arm64) on ritsu's
[releases page](https://github.com/i2y/ritsu/releases), with the SHA-256 of each file in
`SHA256SUMS`. The archive holds `ritsu` and its links, `chobo` among them, so unpacking it into a
directory on the path is the whole install. With Homebrew it is `brew install i2y/tap/ritsu`, on
Debian and Fedora the release's `.deb` and `.rpm` install `/usr/bin/ritsu` with the same links, and
in GitHub Actions `uses: i2y/ritsu@v<version>` puts the release on the runner's path.
[rulec's install page](../rulec/install/) goes through each of them; the commands are the same for
every language of ritsu.

A release also carries the npm package `@i2y/ritsu`, ritsu built for WebAssembly, which Node runs
with no native binary. Its `chobo` command prints what the binary prints, all but
`check --diff-base`, which runs git: [ritsu in Node](../node/).

## From source

With a recent stable Rust, from ritsu's repository:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

That installs `ritsu` and every language of it; `ritsu chobo <command>` is every command on this
site, and a link to `ritsu` named `chobo` does the same. To have chobo alone:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked chobo
```

chobo's one dependency is serde_json.

## Check that it works

The examples are in the repository, under `crates/chobo/examples`. In a clone of it
(`git clone https://github.com/i2y/ritsu`), in `crates/chobo`:

```console
$ chobo check examples/inventory/inventory.book
examples/inventory/inventory.book: ok
…
$ chobo build examples/inventory/inventory.book --target postgres --out db
```

`--lang ja` (or `CHOBO_LANG=ja`, or `RITSU_LANG=ja`) prints the messages in Japanese; the keys of
the JSON stay English.

## The agent skill

`skills/chobo` at the root of ritsu's repository is an [Agent Skill](https://agentskills.io) for AI
coding agents such as Claude Code: the loop from a first draft to a build, the language on one page,
what to ask a person, and the fix for each diagnostic. The four pages of this site's reference come
with it. The `ritsu` binary carries it, with the skills of ritsu and of the other languages:

```console
$ ritsu skills install chobo                                # one project: its .claude/skills/
$ ritsu skills install chobo --user                         # every project on this machine
$ ritsu skills install chobo --dir path/to/skills           # where another agent reads skills
```

In Claude Code, the plugin `ritsu` holds all nine: `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json`,
then `/plugin install ritsu@ritsu`. [For agents](agents.md) says what is in the skill.

## What the output needs

The code a build writes is called by your program, with a client of the database it gives it:

| Target | Runs with |
|---|---|
| `postgres` | PostgreSQL: the SQL makes a schema, its tables and a function for each operation, and can run any number of times |
| `postgres-typescript` | a `pg` `Client`, `Pool` or `PoolClient` |
| `postgres-python` | a DB-API connection of psycopg 3 or psycopg2 |
| `postgres-go` | pgx's `*pgx.Conn`, `*pgxpool.Pool` or `pgx.Tx` |
| `tigerbeetle-typescript` | a client of `tigerbeetle-node` 0.17.9 |
| `tigerbeetle-python` | a `ClientSync` of `tigerbeetle` 0.17.9 |
| `tigerbeetle-go` | a client of `tigerbeetle-go` v0.17.9, which uses cgo |

[Build for a database](databases.md) says what each target writes, and
[Targets](targets.md) how to call it from each language.
