# The Agent Skills of ritsu

This folder holds nine [Agent Skills](https://agentskills.io): one for ritsu, and one for each of
its eight languages. An agent that reads Agent Skills (Claude Code is one) reads a skill's
`SKILL.md` when a task matches the skill's description, and the other pages of the folder only when
it needs them. The skills are about *using* ritsu and its languages in a project, not about working
on this repository.

| Skill | Use it for |
|---|---|
| [ritsu](ritsu/SKILL.md) | a project with files of more than one language: `ritsu check` across them, `ritsu run`, `ritsu gen`, and the diagnostics ritsu has of its own |
| [rulec](rulec/SKILL.md) | business rules (`.rule`): tariffs, fees, discounts, eligibility, turned into proved code |
| [dandori](dandori/SKILL.md) | workflows (`.flow`), checked before they run and built for Temporal, Step Functions, durable functions, Argo and pydantic-graph |
| [koyomi](koyomi/SKILL.md) | closing days, payment days and business days (`.cal`) |
| [chobo](chobo/SKILL.md) | books of stock, money, points and booking slots (`.book`), kept on PostgreSQL or TigerBeetle |
| [geas](geas/SKILL.md) | claims a person has read, held over the code an agent wrote (`.geas`) |
| [yuen](yuen/SKILL.md) | where requirements come from, and what meets and checks them (`.req`) |
| [sakai](sakai/SKILL.md) | maps of bounded contexts, held to the rules, workflows and code they name (`.ctx`) |
| [sekisho](sekisho/SKILL.md) | who may do what (`.gate`), checked on every combination and compiled to Cedar, with the code that asks it |

Each skill runs ritsu, so `ritsu` has to be on PATH; the [README](../README.md#install) says how to
install it. Under the name of a language (a link named `rulec`, say) ritsu is that language, and
`ritsu <language> …` runs it too.

## Install

There are four ways, and each installs the same files.

### Claude Code: the plugin

ritsu's site publishes a Claude Code plugin marketplace, `ritsu`, with one plugin, also `ritsu`,
which holds the nine skills:

```text
/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json
/plugin install ritsu@ritsu
```

The marketplace is that one file, and the plugin is the folder `skills/` of this repository, which
Claude Code checks out alone (about 1 MB) rather than the whole repository. Claude Code names a
plugin's skills after the plugin: `ritsu:ritsu`, `ritsu:rulec`,
`ritsu:dandori`, and so on. The plugin takes the version of ritsu, so a new release of ritsu is a
new version of the plugin.

### Any agent: `ritsu skills install`

The binary carries the skills of its own version, so `ritsu` on PATH is enough:

```console
$ ritsu skills list
ritsu    a project with files of more than one language (`ritsu check`, `ritsu run`, `ritsu gen`, the diagnostics across the languages)
rulec    business rules (.rule)
dandori  workflows (.flow)
koyomi   closing days, payment days and business days (.cal)
chobo    books of stock, money, points and booking slots (.book)
geas     claims a person has read, held over the code (.geas)
yuen     where requirements come from (.req)
sakai    maps of bounded contexts (.ctx)
sekisho  who may do what, compiled to Cedar (.gate)
$ ritsu skills install
$ ritsu skills install --user
$ ritsu skills install rulec dandori
$ ritsu skills install --dir path/to/skills
```

`ritsu skills install` writes each skill as `<dir>/<name>/`. The directory is the project's
`.claude/skills/` in the directory you run it in, `~/.claude/skills/` with `--user` (every project
on the machine), or any directory with `--dir`, for an agent that reads skills from somewhere else.
Names after `install` write only those skills. A file that is already there and the same is left as
it is; a file that differs from the one ritsu carries (changed by hand, or written by another
version of ritsu) stops the install before anything is written, and `--force` writes over it.
ritsu never removes a file it did not write.

### By hand: copy the folders

From a clone of the repository, copy the folders you need:

```console
$ cp -r skills/rulec skills/dandori ~/.claude/skills/                  # every project on this machine
$ cp -r skills/rulec skills/dandori <your-project>/.claude/skills/     # one project, committed with it
```

### From a release

Every release has `ritsu-skills-v<version>.zip`, listed in its `SHA256SUMS`. It holds the nine
folders and the two licenses, so unzipping it where an agent reads skills is the install:

```console
$ unzip ritsu-skills-v0.24.0.zip -d ~/.claude/skills -x 'LICENSE-*'
```

## Letting a skill run its commands

To let the skills run ritsu and the languages without asking each time, allow the commands in the
project's settings (`.claude/settings.json` for Claude Code):

```json
{ "permissions": { "allow": ["Bash(ritsu check:*)", "Bash(ritsu explain:*)", "Bash(rulec:*)", "Bash(dandori:*)", "Bash(koyomi:*)", "Bash(chobo:*)", "Bash(geas:*)", "Bash(ritsu sekisho:*)"] } }
```

Leave out what a person should be asked about each time: `ritsu yuen review` records that a person
looked at a link, so it should always ask, and `ritsu skills install` writes files.

## How they are kept

`ritsu/SKILL.md` is written by hand. The skill of a language is kept with the language:
`crates/<language>/skills/sync.sh` writes the pages that are copies of the language's documents, and
`crates/<language>/skills/README.md` says which pages those are. The tests of each language hold its
skill to what the tool does. `crates/ritsu/tests/skill.rs` holds the nine together: each folder's
`SKILL.md` names the folder and the repository's license, the binary carries every file of the
nine folders and `ritsu skills install` writes them as they are, the release zip holds the same
files, and the marketplace on the site says the version of the workspace and hands out `skills/`
alone.
