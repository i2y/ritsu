# The koyomi agent skill

`koyomi/` is an [Agent Skill](https://agentskills.io) for **using koyomi**: writing a `.cal`,
getting it past `koyomi check`, showing it to the person who approves it, generating its code,
and knowing what to ask a person. It is not about working on koyomi itself.

## Install

```console
$ cp -r skills/koyomi ~/.claude/skills/                  # every project on this machine
$ cp -r skills/koyomi <your-project>/.claude/skills/     # one project, committed with it
```

It needs the `koyomi` binary on PATH. To let the skill run koyomi without a prompt each time, add
this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(koyomi:*)"] } }
```

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, the loop, the language on one page, what to ask a person, from a diagnostic to a fix, the generated code |
| `reference.md` | the whole language, the commands, the exit codes and the formats |
| `codes.md` | every diagnostic code |
| `generated-code.md` | what each target writes, and how to call it |

## Rebuilding it

`SKILL.md` is written by hand. Everything else is a copy of a page under `docs/`, made by
`skills/sync.sh`, with the links rewritten so that none leaves the skill directory: a copied skill
sits in someone else's project, where a link back into this repository leads nowhere.

```console
$ skills/sync.sh
```

`tests/skill.rs` runs the same script into a scratch directory and fails if the committed copies
have drifted, checks that no link in the skill leaves it, and checks the frontmatter of
`SKILL.md`. `tests/docs.rs` holds the skill to the tool as it holds the READMEs and `docs/`: every
diagnostic in it is what `koyomi check` prints, and every line of `.cal` is a line of a real file.
