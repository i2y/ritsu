# The chobo agent skill

`chobo/` is an [Agent Skill](https://agentskills.io) for **using chobo**: writing a `.book`,
getting it past `chobo check`, building it for PostgreSQL or TigerBeetle, and knowing what to
ask a person. It is not about working on chobo itself.

## Install

The nine skills of ritsu, this one among them, install together or one by one, in any of the four
ways [skills/README.md](../../../skills/README.md) at the root of the repository gives: the Claude
Code plugin `ritsu`, `ritsu skills install chobo`, a copy of `skills/chobo/`, or the zip of a
release. It needs `chobo` on PATH, or `ritsu`, which runs it as `ritsu chobo`. To let the skill run
chobo without a prompt each time, add this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(chobo:*)"] } }
```

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, the loop, the language on one page, turning a condition into a bound, what to ask a person, from a diagnostic to a fix, the targets |
| `reference.md` | the whole language |
| `formats.md` | the JSON chobo reads and writes |
| `targets.md` | what `chobo build` writes, and how to call it |
| `codes.md` | every diagnostic code |

## Rebuilding it

`SKILL.md` is written by hand. Everything else is a copy of a page under `docs/`, made by
`skills/sync.sh`, with the links to the rest of the repository sent to GitHub, so that none
leaves the skill directory: a copied skill sits in someone else's project, where a link back
into this repository leads nowhere.

```console
$ skills/sync.sh
```

`tests/skill.rs` runs the same script into a scratch directory and fails if the committed copies
have drifted, checks that no link in the skill leaves it, and checks the frontmatter of
`SKILL.md`. `tests/docs.rs` holds the skill to the tool as it holds the README and the pages:
every diagnostic in it is in a golden file, and every line of `.book` is a line of a real book.
