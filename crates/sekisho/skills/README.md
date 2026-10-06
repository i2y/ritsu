# The sekisho agent skill

`sekisho/` is an [Agent Skill](https://agentskills.io) for **using sekisho**: writing a `.gate`,
getting it past `sekisho check` and the checks `ritsu check` adds across the languages, generating
the Cedar and the code that asks it, showing the page to the people who decide who may do what, and
knowing what to ask a person. It is not about working on sekisho itself.

## Install

The nine skills of ritsu, this one among them, install together or one by one, in any of the four
ways [skills/README.md](../../../skills/README.md) at the root of the repository gives: the Claude
Code plugin `ritsu`, `ritsu skills install sekisho`, a copy of `skills/sekisho/`, or the zip of a
release. It needs `ritsu` on PATH (sekisho comes with it: `cargo install --git
https://github.com/i2y/ritsu --locked ritsu`). To let the skill run sekisho without a prompt each
time, add this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(ritsu sekisho:*)", "Bash(sekisho:*)"] } }
```

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, the loop, the language on one page, what to ask a person, from a diagnostic to a fix (sekisho's and ritsu's across the languages), what it generates and the page |
| `reference.md` | the whole language, what `check` says, what `gen` writes, the commands, the exit codes and the JSON |
| `codes.md` | every diagnostic code |

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
command shown prints what it shows, every diagnostic is what a mutant gives, every line of `.gate`
is a line of a real file, and every line of Cedar is one `gen` writes.
