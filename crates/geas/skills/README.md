# The geas agent skill

`geas/` is an [Agent Skill](https://agentskills.io) for **using geas**: writing code that a claims
file holds, running the gate on a change, fixing a failing claim or a diagnostic, and proposing to a
person the claims that are missing. It is not about working on geas itself.

## Install

The eight skills of ritsu, this one among them, install together or one by one, in any of the four
ways [skills/README.md](../../../skills/README.md) at the root of the repository gives: the Claude
Code plugin `ritsu`, `ritsu skills install geas`, a copy of `skills/geas/`, or the zip of a
release. geas's own binary carries this skill too, so a project that has only `geas` on PATH can
install it with geas alone:

```console
$ geas skill --install .claude/skills                # one project, committed with it
$ geas skill --install ~/.claude/skills              # every project on this machine
```

`geas skill --install <dir>` writes the folder as `<dir>/geas/`, and writes over one that is there
only with `--force` (it then leaves any other file in the folder as it is). From a clone of this
repository, copying the folder does the same:

```console
$ cp -r skills/geas ~/.claude/skills/
```

To let the skill run geas without a prompt each time, add this to the project's settings:

```json
{ "permissions": { "allow": ["Bash(geas:*)"] } }
```

An agent that does not read Agent Skills can be pointed at the guide from its own instructions
file (`AGENTS.md`, a rules file): a line such as "Before changing code that a `.geas` file holds,
run `geas skill` and follow what it prints" is enough, since `geas skill` prints `SKILL.md` and the
other pages are a `geas skill --install` away. geas does not write into other tools' configuration
itself: those files are the person's.

## What is in it

| File | What is in it |
|---|---|
| `SKILL.md` | when the skill applies, who writes what, the loop, the language on one page, reading the answers, from a diagnostic to a fix, what `map` needs from each language, GUI targets, pins and `--jobs`, what to ask the person |
| `language.md` | every form of the language |
| `commands.md` | the commands, options, exit codes, environment variables, and the files and JSON geas writes |
| `codes.md` | every diagnostic, as `geas explain --all` prints it |
| `gui.md` | the screen, the actions, pixie, Chrome, and the driver protocol |
| `map.md` | `geas map` and `geas affected` in full, and CI recipes |
| `examples.md` | the examples and what each shows |

## Rebuilding it

Every page is written by hand except `codes.md`, which `skills/sync.sh` writes from `geas explain
--all`:

```console
$ cargo build
$ skills/sync.sh
```

The binary takes the folder in when it is built (`include_str!`), so build again after changing a
page. `tests/skill.rs` holds the skill to the tool: what `geas skill --install` writes is the
folder, `codes.md` is what `sync.sh` writes, no link leaves the folder, the frontmatter is what the
format asks for, and the driver in `gui.md` runs. `tests/docs.rs` holds the skill as it holds the
READMEs: every claims file in it parses, every output in it is what geas printed in a test, and
every command, option and code it names exists.
