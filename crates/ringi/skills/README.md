# The ringi agent skill

`ringi/` is an [Agent Skill](https://agentskills.io) for **using ringi**: writing a `.ringi` file for
a kind of request any department makes (its form, its route and who approves each stage, its
deadlines, the rules of its ticket), with the organization and the masters it reads; getting it
past `ringi check` and the checks `ritsu check` adds across the languages; trying requesters and
scenarios; showing the page to the people who approve and manage the requests; generating what a
server and its screens read with `ritsu gen`; and knowing what to ask a person. It is not about
working on ringi itself.

## Install

The skills of ritsu, this one among them, install together or one by one, in any of the four ways
[skills/README.md](../../../skills/README.md) at the root of the repository gives: the Claude Code
plugin `ritsu`, `ritsu skills install ringi`, a copy of `skills/ringi/`, or the zip of a release.
It needs `ritsu` on PATH (ringi comes with it: `cargo install --git
https://github.com/i2y/ritsu --locked ritsu`). To let the skill run ringi without a prompt each
time, add this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(ritsu ringi:*)", "Bash(ringi:*)"] } }
```

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, the loop from a request to its checks, what to ask a person, and from a diagnostic to a fix |
| `reference.md` | the whole language, what `check` says, the ticket machine, the organization and the masters, what `gen` writes, the commands, the exit codes and the JSON |
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
line of `.ringi` is a line of a real file, every command listed is one of ringi's, and ritsu's
`tests/ringi.rs` runs the commands shown with what they print.
