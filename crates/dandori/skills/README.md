# The dandori agent skill

`dandori/` is an [Agent Skill](https://agentskills.io) for **using dandori**: writing a `.flow`,
getting it past `dandori check`, building it for a platform, and knowing what to ask a person. It
is not about working on dandori itself.

## Install

```console
$ cp -r skills/dandori ~/.claude/skills/                  # every project on this machine
$ cp -r skills/dandori <your-project>/.claude/skills/     # one project, committed with it
```

It needs the `dandori` binary on PATH, and `rulec` too when a workflow uses rules. To let the
skill run dandori without a prompt each time, add this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(dandori:*)"] } }
```

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, the loop, the language on one page, what to ask a person, from a diagnostic to a fix, the platforms |
| `tour.md` | the language, through the hotel booking |
| `tasks.md` | every way a task can call, API descriptions, child flows |
| `agents.md` | agent tasks |
| `checks.md`, `codes.md` | what the checker looks at, and every diagnostic code |
| `commands.md` | the commands, flags and exit codes |
| `platforms.md` | what each target writes |
| `examples.md` | the five examples |
| `design.md` | the six principles |

## Rebuilding it

`SKILL.md` is written by hand. Everything else is a copy of an English page of the site
(`website/docs`), made by `skills/sync.sh`, with the links rewritten so that none leaves the skill
directory: a copied skill sits in someone else's project, where a link back into this repository
leads nowhere.

```console
$ skills/sync.sh
```

`tests/skill.rs` runs the same script into a scratch directory and fails if the committed copies
have drifted, checks that no link in the skill leaves it, and checks the frontmatter of
`SKILL.md`. `tests/docs.rs` holds the skill to the tool as it holds the site: every diagnostic in
it is in a golden file, and every line of `.flow` is a line of a real flow.
