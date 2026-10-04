# The yuen agent skill

`yuen/` is an [Agent Skill](https://agentskills.io) for **using yuen**: writing a `.req`, getting
it past `yuen check`, reading what a mark says changed, showing the page to the people who have to
understand and check what the code is meant to do, and knowing what to ask a person. It is not
about working on yuen itself.

## Install

```console
$ cp -r skills/yuen ~/.claude/skills/                  # every project on this machine
$ cp -r skills/yuen <your-project>/.claude/skills/     # one project, committed with it
```

It needs `ritsu` on PATH (yuen comes with it: `cargo install --git https://github.com/i2y/ritsu
--locked ritsu`). To let the skill run yuen without a prompt each time, add this to the project's
settings, and leave `review` out of it: a record says a person looked, so `ritsu yuen review` should
always ask.

```json
{ "permissions": { "allow": ["Bash(ritsu yuen check:*)", "Bash(ritsu yuen trace:*)", "Bash(ritsu yuen doc:*)", "Bash(ritsu yuen affected:*)", "Bash(ritsu yuen explain:*)"] } }
```

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, the loop, the language on one page, what to ask a person, from a diagnostic to a fix, why an agent does not run `review` on its own |
| `reference.md` | the whole language, the commands, the exit codes and the JSON |
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
diagnostic in it is what `yuen check` prints, and every line of `.req` is a line of a real file.
