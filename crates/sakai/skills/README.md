# The sakai agent skill

`sakai/` is an [Agent Skill](https://agentskills.io) for **using sakai**: writing a context map as `.ctx` files, getting it past `sakai check`, writing the settings of the import linters from it, showing its page to the people who check what the code is to do, and knowing what to ask a person. It is not about working on sakai itself.

## Install

The nine skills of ritsu, this one among them, install together or one by one, in any of the four
ways [skills/README.md](../../../skills/README.md) at the root of the repository gives: the Claude
Code plugin `ritsu`, `ritsu skills install sakai`, a copy of `skills/sakai/`, or the zip of a
release. It needs ritsu on PATH (sakai comes with it). To let the skill run sakai without a prompt
each time, add this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(ritsu sakai:*)", "Bash(sakai:*)"] } }
```

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, the loop, the language on one page, what to ask a person, from a diagnostic to a fix, the linters and the page |
| `reference.md` | the whole language and the commands |
| `targets.md` | the settings of the four import linters, what each catches, Rust, and CML |
| `codes.md` | every diagnostic code |

## Rebuilding it

`SKILL.md` is written by hand. Everything else is a copy of a page under `docs/`, made by `skills/sync.sh`, with the links rewritten so that none leaves the skill directory: a copied skill sits in someone else's project, where a link back into this repository leads nowhere.

```console
$ skills/sync.sh
```

`tests/skill.rs` runs the same script into a scratch directory and fails if the committed copies have drifted, checks that no link in the skill leaves it, and checks the frontmatter of `SKILL.md`. `tests/docs.rs` holds the skill to the tool as it holds the READMEs and `docs/`: every command shown is one, and every line of `.ctx` is a line of a real file.
