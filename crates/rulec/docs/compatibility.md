# Compatibility

From 1.0.0, rulec's version numbers follow [Semantic Versioning](https://semver.org/). This
page says what "compatible" means for each thing rulec reads or writes, so that a 1.x
release is a promise you can hold it to.

The short form: **a rule that passes `rulec check` under 1.0 passes under every 1.x, means
the same thing, and its generated code is called the same way and answers the same.**

## What every 1.x keeps

1. **The `.rule` language.** Every construct 1.0 accepts keeps its syntax and its meaning —
   the state machines (`machine`, `held`, `never`, `once`, `scenario`) included. New
   constructs may be added.
2. **The names you may use.** The reserved words — the 65 words E009 refuses as a name,
   listed in `src/kw.rs` — are fixed at 1.0, and a test holds the list. A later release may
   add words to the language, but never to that list: a new word is read only where no name
   can stand, or gives way to a name the rule declares. A name that is legal under 1.0 stays
   legal.
3. **What the generated code answers.** For a rule that passes, the code generated in every
   language returns the same outputs for the same inputs under every 1.x, and so does the
   reference evaluator: a record replayed under a later 1.x gets the answer it got before.
4. **How the generated code is called.** The names of modules, functions, types, fields and
   errors, the integers on the wire (§10.2), the record a call writes, the `_traced`,
   `_record` and `_from` functions — everything [`rulec api`](formats.md#api) describes. New
   functions may appear beside them.
5. **The command line.** Commands, flags, their values, and the exit codes: 0 notes only, 1
   findings, 2 bad arguments or a file that cannot be read. New commands and flags may be
   added.
6. **Diagnostic codes.** A code keeps its meaning. A code that is retired stays in the ledger,
   marked as retired, and its number is never given to anything else.
7. **Machine-readable formats.** Every `--format json` output, the vectors, the fixtures and
   the replay manifest, the `adapter/1` and `extract/1` protocols, the certificate, the MCP
   tools with their arguments and resources, the GitHub Action's inputs, and the names of
   the release archives keep every field they have at 1.0, with the same meaning. A 1.x may
   add a field, and may add a value to a set of values the documentation lists — a new
   diagnostic code, a new `what` in the `domain` of `diff`. **A reader should skip a key it
   does not know, and treat a value it does not know as it treats an unknown key.** Where a
   format carries `v`, that is the version of its shape: `check` is at 2 and the certificate
   at 1, and an output without `v` is at 1. A re-checker of certificates refuses a `v` it was
   not written for, as `tools/recheck.py` and the Lean program in `proofs/` do.

## What a 1.x may change

- **Prose.** Diagnostic titles, notes and hints, `--help`, the text reports, what `rulec doc`
  writes, and this documentation. Read codes and JSON keys, not sentences (§11 principle 5).
- **The text of the generated code.** Its layout, comments and local names, and its header,
  which names the version of rulec that wrote it. Upgrading rulec therefore means running
  `rulec gen` again and committing what changed; `rulec gen --check` fails until you do. CI
  installs one exact release (`uses: i2y/rulec@v1.0.0`), and there is no moving tag such as
  `@v1`: the generated files would go stale under it without a change of yours.
- **A pass that should have been a failure.** When a 1.x finds that an earlier version let
  through something this page or [what it proves](https://i2y.github.io/rulec/checks/) says
  it catches — a gap or an overlap it missed, a line it read into nothing — the fix makes that
  rule fail. A false green is not kept for the sake of compatibility. Each such change is
  named in the release notes, with the kind of rule it can affect.
- **An answer that disagreed with the reference evaluator.** If the code generated in some
  language answered differently from the reference evaluator, the fix makes it agree, and
  the release notes name it.
- **How long a check takes,** as long as a rule that finished within the default budget
  still does.

## What is not covered

- **The Rust library** (`src/lib.rs`). Its modules are public so that the tests, the
  playground and `rulec mcp` can reach them, not as an interface; they change with the tool.
  Use the command line, `rulec mcp`, or the generated code.
- **The playground and the site.**
- **Everything under `experiments/`.**
