# Install

rulec is one binary with no runtime and no external dependencies. Every
release publishes a static binary for macOS (arm64, x64) and Linux (x64,
arm64), with the SHA-256 of each beside it, on the
[releases page](https://github.com/i2y/rulec/releases).

## The release binary

```console
$ v=v0.18.0; t=aarch64-apple-darwin
$ curl -fsSLO "https://github.com/i2y/rulec/releases/download/$v/rulec-$v-$t.tar.gz"
$ curl -fsSL "https://github.com/i2y/rulec/releases/download/$v/SHA256SUMS" | grep "$t" | shasum -a 256 -c
rulec-v0.18.0-aarch64-apple-darwin.tar.gz: OK
$ tar -xzf "rulec-$v-$t.tar.gz" && install -m 755 rulec ~/.local/bin/
$ rulec --version
rulec 0.18.0
```

`t` is one of `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`; the Linux
two are linked statically and run on any distribution. On Linux the check
is `sha256sum -c`. Holding the archive to `SHA256SUMS` before running it
is the whole of the verification, so that line is not the one to skip.

## From source

```console
$ git clone https://github.com/i2y/rulec
$ cd rulec
$ cargo install --path .
$ rulec --version
rulec 0.18.0
```

A recent stable Rust is all that is needed — rulec has **zero
dependencies** outside the standard library, so `cargo install` fetches
nothing.

To run it out of the build directory instead:

```console
$ cargo build --release
$ ./target/release/rulec --help
```

## What else you might need

Nothing, for the checks. `rulec check`, `fmt`, `gen`, `vectors`,
`coverage`, `doc`, `api`, `explain`, `schema`, `adapter` and
`fixtures lint` are self-contained.

Two steps reach outside:

- **`rulec test`** runs the generated Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go,
  Swift, Java, NumPy, SQL and Wasm and compares them with the reference evaluator. It needs
  `python3`, `node`, `rustc`, `ruby`, `php`, `go`, `swiftc` and a JDK on the path (the SQL runs
  on the `sqlite3` inside that `python3`); without one it says which side it skipped and does
  not fail.
- **`rulec verify`** starts your adapter as a child process, so it needs
  whatever that adapter is written in.

Two more, and only if you want to **type-check** the output: the generated Python passes
`mypy --strict`, and the generated Ruby ships an `.rbs` that `steep` reads. Neither is
needed to use what comes out — it runs as it stands.

## The agent skill

The first user of this tool is an agent, and `skills/rulec/` is the
skill that drives it: the procedure, the grammar, eighteen worked
rules, the data formats, and how to target a language rulec does not
generate. It does not copy the tool's own details down: it asks, through
`--help`, `--format json` and `rulec explain`, so it does not go stale
against the binary on the path.

Copy the directory in, keeping its name:

```console
$ git clone https://github.com/i2y/rulec /tmp/rulec
$ mkdir -p .claude/skills
$ cp -r /tmp/rulec/skills/rulec .claude/skills/
```

That gives `.claude/skills/rulec/SKILL.md` with five files beside it.
The folder is what makes the skill findable, so keep it whole. To have
it in every project rather than one, put it in `~/.claude/skills/`
instead.

The only thing it needs is `rulec` on the path — the section above.

Ask for something it covers ("write a `.rule` for this tariff", "fix this
E101") and it usually applies on its own. **To be certain, name it: "use
the rulec skill".**

## The MCP server

Where the agent has no shell — a chat client, an IDE assistant that speaks MCP — the same
commands are there as tools:

```console
$ claude mcp add rulec -- rulec mcp
```

or, for any client, a stdio server whose command is `rulec mcp`:

```json
{ "mcpServers": { "rulec": { "command": "rulec", "args": ["mcp"] } } }
```

One tool per command (`rulec_check`, `rulec_gen`, `rulec_doc`, …), one argument per flag,
and the exit code at the end of every result. The procedure and the references are served
as resources, so an agent that cannot read this repository still reads `rulec://docs/agents.md`
first. The shape is in [Formats](formats.md#mcp).

**This one speaks stdio and nothing else.** It hands an agent the commands that read and
write your files, so it belongs on the same machine as the shell it stands in for; there is
no `--http`, and that is a decision rather than a gap.

**The other MCP server is a different thing.** That one is the tool for the agent that
*writes* a rule; for the agent that *calls* one, `gen` writes the rule itself as a server
beside the module, and that server speaks stdio **and MCP's Streamable HTTP**
(`--http 8000`) for the hosts that only take a URL. Where the host renders **MCP Apps**, it
also offers the approver's page as the tool's own view:
[the rule as a tool for an agent](generate.md#the-rule-as-a-tool-for-an-agent).

## Language

Output is **English by default**. One setting brings back Japanese —
every surface, including the prose inside generated code:

```console
$ rulec check rules/送料.rule --lang ja
$ RULEC_LANG=ja rulec check rules/送料.rule
```

`--lang` beats `RULEC_LANG`, which beats the default. The system locale
is deliberately ignored: generated files are checked with `gen --check`
and CI logs are diffed, so the output must not change with the machine
it runs on.

## In CI

`uses: i2y/rulec@v0.18.0` puts that release on the runner's `PATH`,
verified against the checksums published with it. The ref the action is
referenced with is the release, so by default the two cannot drift apart
(`with: { version: v0.4.0 }` is how you ask for another one on purpose).
The check against `SHA256SUMS` **always runs** — a missing line for the
archive is itself a failure. To pin the archive's hash in the workflow as
well, add `with: { sha256: … }`: one more check, not a different one.

**That one line is the whole install**, but it needs `actions/checkout`
before it: what rulec reads is the `rules/` in your repository. As a job:

```yaml
check:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v7
      with:
        fetch-depth: 0                   # --diff-base reads origin/main
    - uses: i2y/rulec@v0.18.0
    - run: rulec fmt --check rules/
    - run: rulec check rules/ --diff-base origin/main
    - run: rulec gen rules/ --out generated/ --check
    - run: rulec coverage rules/
    - run: rulec test generated/
```

It runs on the Linux (x86_64, aarch64) and macOS (x86_64, arm64) runners.
Those are the four releases there are, so any other runner stops with
`no rulec release is built for …`.

Those five `run:` lines are the gate. Replaying past records belongs in a separate
job, one that has the records, and it is the job that makes a change
visible: the pull request gets a comment saying how many records move and
by how much. Four things about it are deliberate.

- The old version is `rules/送料.rule@origin/main`, the file as it is on
  the base branch, so the checkout fetches that branch.
- `diff` exits 1 when there is an impact. Here that is information, not a
  failure, so the step goes on after 1 and stops only on 2.
- `--terse` leaves the witness column out. A comment is read by everyone
  with access to the repository, and the values of a production record
  are not for it.
- The output language is the one the people reading the pull request use,
  because that is what it is pasted in front of.

```yaml
replay:
  if: github.event_name == 'pull_request'
  runs-on: ubuntu-latest
  permissions:
    contents: read
    pull-requests: write
  steps:
    - uses: actions/checkout@v7
      with:
        fetch-depth: 0                   # origin/main is where the old version is read from
    - uses: i2y/rulec@v0.18.0
    # a step of your own puts the records at $FIXTURES: an artifact, or protected storage
    - run: rulec diff rules/送料.rule@origin/main rules/送料.rule --fixtures "$FIXTURES" --format markdown --terse > diff.md || [ $? -eq 1 ]
      env:
        RULEC_LANG: ja                   # the people approving this one read Japanese
    - run: gh pr comment "$PR" --body-file diff.md
      env:
        GH_TOKEN: ${{ github.token }}
        PR: ${{ github.event.pull_request.number }}
```

The job above needs records. The one below needs nothing but the two versions, so it runs
on every pull request from the first day — and the two comments read side by side: what
*can* move, and how much of what you have does.

```yaml
- run: rulec diff rules/送料.rule@origin/main rules/送料.rule --format markdown > region.md || [ $? -eq 1 ]
  env:
    RULEC_LANG: ja
- run: gh pr comment "$PR" --body-file region.md
```


With several rules, `git diff --name-only --diff-filter=M origin/main...HEAD -- 'rules/*.rule'`
lists the ones the pull request changed, and the same two lines run once
per rule.

## Where to go next

<div class="grid cards" markdown>

-   __[Write a table (.rule)](tour.md)__

    The language, from the first line to a rule that checks clean.

-   __[What it proves](checks.md)__

    The seven checks, and how to read what they report.

-   __[For agents](agents.md)__

    The whole loop, and the four things not to do.

</div>
