# The koyomi agent skill

`koyomi/` is an [Agent Skill](https://agentskills.io) for **using koyomi**: writing a `.cal`,
getting it past `koyomi check`, showing it to the people who read it, generating its code,
and knowing what to ask a person. It is not about working on koyomi itself.

## Install

The eight skills of ritsu, this one among them, install together or one by one, in any of the four
ways [skills/README.md](../../../skills/README.md) at the root of the repository gives: the Claude
Code plugin `ritsu`, `ritsu skills install koyomi`, a copy of `skills/koyomi/`, or the zip of a
release. It needs `koyomi` on PATH, or `ritsu`, which runs it as `ritsu koyomi`. To let the skill run
koyomi without a prompt each time, add this to the project's settings:

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
