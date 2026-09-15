# Install

rulec is one binary with no runtime and no external dependencies. Today
it is built from source; there are no published releases yet.

## From source

```console
$ git clone https://github.com/i2y/rulec
$ cd rulec
$ cargo install --path .
$ rulec --version
rulec 0.1.0
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

- **`rulec test`** runs the generated Python, TypeScript, JavaScript, Rust, Ruby, Go and
  Swift and compares them with the reference evaluator. It needs `python3`,
  `node`, `rustc`, `ruby`, `go` and `swiftc` on the path; without one it says which side it skipped and
  does not fail.
- **`rulec verify`** starts your adapter as a child process, so it needs
  whatever that adapter is written in.

Two more, and only if you want to **type-check** the output: the generated Python passes
`mypy --strict`, and the generated Ruby ships an `.rbs` that `steep` reads. Neither is
needed to use what comes out — it runs as it stands.

## The agent skill

The first user of this tool is an agent, and `skills/rulec/` is the
skill that drives it: the procedure, the grammar, thirteen worked
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

```yaml
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/
```

Those five are the gate. Replaying past records belongs in a separate
job, one that has the records — and there the output language is set to
the one the people reading the pull request use, because that is what it
is pasted in front of:

```yaml
- run: rulec diff 送料@v3 送料@v4 --fixtures "$FIXTURES" --format markdown > diff.md
  env:
    RULEC_LANG: ja        # the people approving this one read Japanese
- run: gh pr comment "$PR" --body-file diff.md
```

## Where to go next

<div class="grid cards" markdown>

-   __[Write a table (.rule)](tour.md)__

    The language, from the first line to a rule that checks clean.

-   __[What it proves](checks.md)__

    The seven checks, and how to read what they report.

-   __[For agents](agents.md)__

    The whole loop, and the four things not to do.

</div>
