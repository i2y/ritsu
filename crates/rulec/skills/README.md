# The rulec agent skill

`rulec/` is an [Agent Skill](https://agentskills.io) for **using rulec** — writing a `.rule`,
getting it past `rulec check`, generating the nine languages, and showing a person what
changed. It is not about working on rulec itself.

## Install

```console
$ cp -r skills/rulec ~/.claude/skills/            # every project on this machine
$ cp -r skills/rulec <your-project>/.claude/skills/   # one project, committed with it
```

It needs the `rulec` binary on PATH. Nothing else: the skill reads rulec through its own
`--help`, `--format json` and `rulec explain`, so it never has to read rulec's source.

To let it run rulec without a prompt each time, add this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(rulec:*)"] } }
```

## What is in it

| | |
|---|---|
| `SKILL.md` | when the skill applies, the loop, what not to decide alone, how to turn a diagnostic into a question for a person |
| `reference.md` | the complete grammar |
| `examples.md` | every rule in the corpus in full, smallest first, each with what it shows |
| `formats.md` | every machine-readable format |
| `generated-code.md` | the shape of the generated code in each language, and how to call it |
| `backends.md` | targeting a language rulec does not generate, without losing the comparison |

The diagnostics are deliberately **not** bundled: `rulec explain <CODE>` prints them from the
binary, so they can never be out of date with the binary in front of you.

## Rebuilding it

Everything except the frontmatter and the two short framing sections is copied from this
repository's own documents, so the skill cannot describe a tool that has moved on.

```console
$ skills/sync.sh
```

`tests/skill.rs` repeats the same assembly and fails if the committed files have drifted, and
also checks that no link in the skill points outside the skill directory — which would be a
dead link the moment someone copies it into their own project.
