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

- **`rulec test`** runs the generated Python, TypeScript and Go and
  compares them with the reference evaluator. It needs `python3`, `node`
  and `go` on the path; without one it says which side it skipped and
  does not fail.
- **`rulec verify`** starts your adapter as a child process, so it needs
  whatever that adapter is written in.

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
job, one that has the records — and there the language is turned to
Japanese, because what it produces is pasted in front of a person:

```yaml
- run: rulec diff 送料@v3 送料@v4 --fixtures "$FIXTURES" --format markdown > diff.md
  env:
    RULEC_LANG: ja
- run: gh pr comment "$PR" --body-file diff.md
```

## Where to go next

<div class="grid cards" markdown>

-   __[Write a table](tour.md)__

    The language, from the first line to a rule that checks clean.

-   __[What it proves](checks.md)__

    The seven checks, and how to read what they report.

-   __[For agents](agents.md)__

    The whole loop, and the four things not to do.

</div>
