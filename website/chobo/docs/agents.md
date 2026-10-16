# For agents

chobo is made to be written by an AI coding agent and read by a person. What the agent hands back
is a book, which the people who keep the accounts can read and check, and not the code around a
database that keeps a balance within its bounds: that code is what `chobo build` writes. Two things
stay with people: the numbers and the names of the business (the bounds, the reasons a refusal
gives, what makes a call once only, how long a hold lasts), and the code around the book (where the
amounts come from, who posts and voids a hold, the connection to the database).

## The agent skill

[skills/chobo](https://github.com/i2y/ritsu/tree/main/skills/chobo) is an
[Agent Skill](https://agentskills.io) for using chobo, written for an agent working in someone's
project. Its `SKILL.md` has:

- **the loop**, from finding the quantities and the accounts, to turning every condition into the
  bound of one account, writing the book, checking it until it is clean, watching it run, showing
  it to a person and building it;
- **the language on one page**, with the inventory example;
- **a table that turns a condition into a bound**: a refund that never exceeds the sale becomes
  `refundable(order)` `at least 0`, at most 120 seats a day becomes `free_seats(day)` `at least 0`,
  and *the fee is 10% of the price* becomes no bound at all, since the caller works out the fee;
- **what to ask a person** rather than guess, and how: with the scenario the checker or
  `chobo run` gives, in the reader's terms;
- **the fix for each diagnostic**, code by code;
- **what a machine reads**: `chobo check --format json`, `chobo api`, and the exit codes;
- **the targets**, and what each needs.

The four pages of this site's reference come with it, as copies made from the same documents, so
that no link in the skill leaves its folder. The skill runs chobo as `ritsu chobo` from the path.

## Install it

The `ritsu` binary carries it, with the skills of ritsu and of the other languages:

```console
$ ritsu skills install chobo                                # one project: its .claude/skills/
$ ritsu skills install chobo --user                         # every project on this machine
$ ritsu skills install chobo --dir path/to/skills           # where another agent reads skills
```

In Claude Code, the plugin `ritsu` holds all nine: `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json`,
then `/plugin install ritsu@ritsu`. From a clone of the repository, copying `skills/chobo` into
`~/.claude/skills/` or a project's `.claude/skills/` does the same, and every release has the nine
in `ritsu-skills-v<version>.zip`. Asked for something it covers ("keep the stock from going below
0", "fix this W103"), the agent usually takes it up by itself; to be certain, name it: "use the
chobo skill".

## What a machine reads

Everything is on the command line, and what an agent acts on is data:

- `chobo check --format json <file.book>…` prints `{"v": 2, "files": [...]}`: each file's
  diagnostics, with the operations that get there, and its report, each operation's reasons with
  an example ([Formats](formats.md#diagnostics-and-the-report)).
- `chobo api <file.book>` prints how to call the book, the reasons of each operation, the life of a
  hold as a state machine, how the IDs are made, and what each target names things.
- The exit code is 0 when there is no error (warnings may be), 1 when there is, and 2 for a mistake
  in how chobo is called or a file it cannot read. A flag chobo does not know stops it with 2, so
  that an agent never believes a request went through that was ignored.
- `--lang ja` gives the messages in Japanese, for a person who reads them; the keys of the JSON stay
  English.

In a project of more than one language, the skill of ritsu goes with it: `ritsu check` checks the
book where a workflow calls it ([With the other languages](together.md)).
