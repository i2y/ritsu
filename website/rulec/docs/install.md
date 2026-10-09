# Install

rulec is one binary with no runtime and no external dependencies, handed out
as part of [ritsu](https://github.com/i2y/ritsu): a release of ritsu holds one
binary, `ritsu`, with a link to it named for each of its languages, and called
as `rulec` it is rulec. Every release publishes it for macOS (arm64,
x64) and Linux (x64, arm64), with the SHA-256 of each beside it, on ritsu's
[releases page](https://github.com/i2y/ritsu/releases). The Linux ones are
statically linked; the macOS ones link only the system library every Mac has.
The same binaries come through Homebrew and as `.deb` and `.rpm` packages, so
rulec can arrive the way everything else on the machine did.

ritsu's first release, 0.23.0, continues rulec's numbering: rulec's own
releases end at 0.22.1, and [From rulec's own releases](#from-rulecs-own-releases)
says how to move from one of them. From source, rulec is built from ritsu's
repository ([From source](#from-source)).

## Homebrew

On macOS and on Linux:

```console
$ brew install i2y/tap/ritsu
$ rulec --version
rulec 0.25.0
```

That puts `ritsu` on the path with the eight links beside it, so `rulec` is a
command just as before. The old name, `i2y/tap/rulec`, leads to the same
formula, but Homebrew 7 takes it only once that formula is trusted: by itself,
`brew install i2y/tap/rulec` stops with
`Refusing to load formula i2y/tap/ritsu from untrusted tap i2y/tap`, and after
`brew trust --formula i2y/tap/ritsu` the old name installs ritsu.

The formula installs the release archive for your platform, held to its line
in `SHA256SUMS`. Each release rewrites it, and only after brew has installed it
and run its test on macOS and on Linux, which calls all nine names;
`brew upgrade ritsu` takes the next one.

## Debian, Ubuntu, Fedora, RHEL

Every release carries a `.deb` and an `.rpm` for x64 and arm64. They hold the
same static binary as the archives, so they depend on nothing:

```console
$ v=0.25.0; a=amd64                  # arm64 on ARM
$ curl -fsSLO "https://github.com/i2y/ritsu/releases/download/v$v/ritsu_$v-1_$a.deb"
$ curl -fsSL "https://github.com/i2y/ritsu/releases/download/v$v/SHA256SUMS" | grep "ritsu_$v-1_$a.deb" | sha256sum -c
ritsu_0.25.0-1_amd64.deb: OK
$ sudo apt install "./ritsu_$v-1_$a.deb"
```

```console
$ v=0.25.0; a=x86_64                 # aarch64 on ARM
$ curl -fsSLO "https://github.com/i2y/ritsu/releases/download/v$v/ritsu-$v-1.$a.rpm"
$ curl -fsSL "https://github.com/i2y/ritsu/releases/download/v$v/SHA256SUMS" | grep "ritsu-$v-1.$a.rpm" | sha256sum -c
ritsu-0.25.0-1.x86_64.rpm: OK
$ sudo dnf install "./ritsu-$v-1.$a.rpm"
```

The package installs `/usr/bin/ritsu` with the eight links beside it
(`/usr/bin/rulec -> ritsu`, and so on), and replaces the package `rulec` of
rulec's own releases.

The packages are not signed: as with the archives, their line in `SHA256SUMS`
is the check. No package repository stands behind them, so `apt upgrade` and
`dnf upgrade` do not see a new release — install the next one the same way.

<!-- crates.io is on hold (DESIGN §15.158, and ritsu's DESIGN 13.2). Once the crate is published,
uncomment this section and say "and through Cargo" in the first paragraph again.

## Cargo

```console
$ cargo install rulec          # builds it; there is nothing else to fetch
```

`cargo install` needs a recent stable Rust and nothing more, because rulec has
no dependencies.
-->

## The release binary

```console
$ v=v0.25.0; t=aarch64-apple-darwin
$ curl -fsSLO "https://github.com/i2y/ritsu/releases/download/$v/ritsu-$v-$t.tar.gz"
$ curl -fsSL "https://github.com/i2y/ritsu/releases/download/$v/SHA256SUMS" | grep "$t" | shasum -a 256 -c
ritsu-v0.25.0-aarch64-apple-darwin.tar.gz: OK
$ tar -xzf "ritsu-$v-$t.tar.gz" -C ~/.local/bin --exclude 'LICENSE-*' --exclude THIRD_PARTY_NOTICES
$ rulec --version
rulec 0.25.0
```

The archive holds `ritsu`, a link to it for each language (`rulec`, `dandori`,
`koyomi`, `chobo`, `geas`, `yuen`, `sakai`, `sekisho`), the two licenses and, from
0.24.0 on, THIRD_PARTY_NOTICES, the notices and licenses of what the binary holds
from others, side by side. The links are relative, so unpacking it into a
directory on the path is the whole install;
`--exclude 'LICENSE-*' --exclude THIRD_PARTY_NOTICES` leaves the licenses and the
notices in the archive.
`t` is one of `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`; the Linux
two are linked statically and run on any distribution. On Linux the check
is `sha256sum -c`. Holding the archive to `SHA256SUMS` before running it
is the whole of the verification, so that line is not the one to skip.

## From rulec's own releases

Up to 0.22.1, rulec had releases of its own. ritsu's releases carry `rulec`
as a link, so moving changes the name of what is installed, not the command.

With Homebrew, the tap renamed the formula `rulec` to `ritsu`, which lets brew
move an installed rulec over to ritsu. Homebrew 7 reads a formula from a tap
other than its own only once that tap is trusted, and naming the formula with
its tap trusts it. `brew install` then says that rulec is installed but not
migrated; `brew migrate ritsu` moves it over, and `brew upgrade ritsu` takes it
to 0.25.0:

```console
$ brew install i2y/tap/ritsu
$ brew migrate ritsu
$ brew upgrade ritsu
$ rulec --version
rulec 0.25.0
```

`brew trust --formula i2y/tap/ritsu` followed by `brew upgrade` does the same:
the upgrade migrates rulec and takes it to 0.25.0 in one go.

Installing ritsu's `.deb` or `.rpm` as above removes the package `rulec`, and
`/usr/bin/rulec` becomes a link to `ritsu`. ritsu's archive, unpacked into the
directory that holds the old `rulec`, puts the link in place of the old binary. In CI, the line `uses: i2y/rulec@v0.22.1` becomes
`uses: i2y/ritsu@v0.25.0` ([In CI](#in-ci)).

## From source

With a recent stable Rust, from ritsu's repository:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked rulec
```

That builds rulec alone, which needs no other language and fetches nothing,
because rulec has **zero dependencies** outside the standard library. With the
package `ritsu` in its place, you have `ritsu` and every language of it, and
`ritsu rulec <command>` is every command on this site; a rule over the days of
a koyomi date (`range from koyomi`) is checked by `ritsu rulec check`.

To run it out of the build directory instead:

```console
$ git clone https://github.com/i2y/ritsu
$ cd ritsu
$ cargo build --release -p rulec
$ ./target/release/rulec --help
```

## What else you might need

Nothing, for the checks. `rulec check`, `fmt`, `gen`, `vectors`,
`coverage`, `doc`, `api`, `explain`, `schema`, `adapter` and
`fixtures lint` are self-contained.

Two steps reach outside:

- **`rulec test`** runs the generated Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go,
  Swift, Java, NumPy, SQL and Wasm and compares them with the reference evaluator. It needs
  `python3`, `node`, `rustc`, `ruby`, `php`, `go`, `swiftc` and a JDK on the path (the SQL runs
  on the `sqlite3` inside that `python3`); without one it says which side it skipped and does
  not fail.
- **`rulec verify`** starts your adapter as a child process, so it needs
  whatever that adapter is written in.

Three more, and only if you want to **type-check** the output: the generated Python passes
`mypy --strict`, the generated TypeScript passes `tsc --strict`, and the generated Ruby ships
an `.rbs` that `steep` reads. None of them is needed to use what comes out — it runs as it
stands.

## The agent skill

The first user of this tool is an agent, and `skills/rulec/` (at the root of
ritsu's repository) is the skill that drives it: the procedure, the grammar, eighteen worked
rules, the data formats, and how to target a language rulec does not
generate. It does not copy the tool's own details down: it asks, through
`--help`, `--format json` and `rulec explain`, so it does not go stale
against the binary on the path.

The `ritsu` binary carries it, with the skills of ritsu and of the other languages:

```console
$ ritsu skills install rulec
```

That gives `.claude/skills/rulec/SKILL.md` with six files beside it.
The folder is what makes the skill findable, so keep it whole. To have
it in every project rather than one, add `--user`, which puts it in
`~/.claude/skills/` instead; `--dir <dir>` puts it where another agent reads skills.
In Claude Code, the plugin `ritsu` holds all nine skills: `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json`,
then `/plugin install ritsu@ritsu`. From a clone of the repository, copying `skills/rulec` does
the same, and every release has the nine in `ritsu-skills-v<version>.zip`.

The only thing it needs is `rulec` on the path, whichever way above put it there.

Ask for something it covers ("write a `.rule` for this tariff", "fix this
E101") and it usually applies on its own. **To be certain, name it: "use
the rulec skill".**

## The MCP server

Where the agent has no shell — a chat client, an IDE assistant that speaks MCP — the same
commands are there as tools:

```console
$ claude mcp add rulec -- rulec mcp
```

or, for any client, a stdio server whose command is `rulec mcp`:

```json
{ "mcpServers": { "rulec": { "command": "rulec", "args": ["mcp"] } } }
```

One tool per command (`rulec_check`, `rulec_gen`, `rulec_doc`, …), one argument per flag,
and the exit code at the end of every result. The procedure and the references are served
as resources, so an agent that cannot read this repository still reads `rulec://docs/agents.md`
first. The shape is in [Formats](formats.md#mcp).

**This one speaks stdio and nothing else.** It hands an agent the commands that read and
write your files, so it belongs on the same machine as the shell it stands in for; there is
no `--http`, and that is a decision rather than a gap.

**The other MCP server is a different thing.** That one is the tool for the agent that
*writes* a rule; for the agent that *calls* one, `gen` writes the rule itself as a server
beside the module, and that server speaks stdio **and MCP's Streamable HTTP**
(`--http 8000`) for the hosts that only take a URL. Where the host renders **MCP Apps**, it
also offers the page for people as the tool's own view:
[the rule as a tool for an agent](generate.md#the-rule-as-a-tool-for-an-agent).

## Language

Output is **English by default**. One setting brings back Japanese —
every surface, including the prose inside generated code:

```console
$ rulec check rules/shipping_fee.rule --lang ja
$ RULEC_LANG=ja rulec check rules/shipping_fee.rule
```

`--lang` beats `RULEC_LANG`, which beats `RITSU_LANG`, which beats the default. The system locale
is deliberately ignored: generated files are checked with `gen --check`
and CI logs are diffed, so the output must not change with the machine
it runs on.

## In CI

`uses: i2y/ritsu@v0.25.0` puts that release of ritsu on the runner's `PATH`,
`rulec` and the other links with it, verified against the checksums
published with it. The ref the action is referenced with is the release, so
by default the two cannot drift apart (`with: { version: … }` is how you ask
for another one on purpose). The check against `SHA256SUMS` **always runs** —
a missing line for the archive is itself a failure. To pin the archive's hash
in the workflow as well, add `with: { sha256: … }`: one more check, not a
different one. A workflow that still says `uses: i2y/rulec@v0.22.1`, rulec's
own last release, keeps working for as long as rulec's repository is there.

**That one line is the whole install**, but it needs `actions/checkout`
before it: what rulec reads is the `rules/` in your repository. As a job:

```yaml
check:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v7
      with:
        fetch-depth: 0                   # --diff-base reads origin/main
    - uses: i2y/ritsu@v0.25.0
    - run: rulec fmt --check rules/
    - run: rulec check rules/ --diff-base origin/main
    - run: rulec gen rules/ --out generated/ --check
    - run: rulec coverage rules/
    - run: rulec test generated/
```

It runs on the Linux (x86_64, aarch64) and macOS (x86_64, arm64) runners.
Those are the four releases there are, so any other runner stops with
`no ritsu release is built for …`.

Those five `run:` lines are the gate. Replaying past records belongs in a separate
job, one that has the records, and it is the job that makes a change
visible: the pull request gets a comment saying how many records move and
by how much. Four things about it are deliberate.

- The old version is `rules/shipping_fee.rule@origin/main`, the file as it is on
  the base branch, so the checkout fetches that branch.
- `diff` exits 1 when there is an impact. Here that is information, not a
  failure, so the step goes on after 1 and stops only on 2.
- `--terse` leaves the witness column out. A comment is read by everyone
  with access to the repository, and the values of a production record
  are not for it.
- The output language is the one the people reading the pull request use,
  because that is what it is pasted in front of.

```yaml
replay:
  if: github.event_name == 'pull_request'
  runs-on: ubuntu-latest
  permissions:
    contents: read
    pull-requests: write
  steps:
    - uses: actions/checkout@v7
      with:
        fetch-depth: 0                   # origin/main is where the old version is read from
    - uses: i2y/ritsu@v0.25.0
    # a step of your own puts the records at $FIXTURES: an artifact, or protected storage
    - run: rulec diff rules/shipping_fee.rule@origin/main rules/shipping_fee.rule --fixtures "$FIXTURES" --format markdown --terse > diff.md || [ $? -eq 1 ]
      env:
        RULEC_LANG: ja                   # the people who read this one read Japanese
    - run: gh pr comment "$PR" --body-file diff.md
      env:
        GH_TOKEN: ${{ github.token }}
        PR: ${{ github.event.pull_request.number }}
```

The job above needs records. The one below needs nothing but the two versions, so it runs
on every pull request from the first day — and the two comments read side by side: what
*can* move, and how much of what you have does.

```yaml
- run: rulec diff rules/shipping_fee.rule@origin/main rules/shipping_fee.rule --format markdown > region.md || [ $? -eq 1 ]
  env:
    RULEC_LANG: ja
- run: gh pr comment "$PR" --body-file region.md
```


With several rules, `git diff --name-only --diff-filter=M origin/main...HEAD -- 'rules/*.rule'`
lists the ones the pull request changed, and the same two lines run once
per rule.

## Where to go next

<div class="grid cards" markdown>

-   __[Write a rule (.rule)](tour.md)__

    The language, from the first line to a rule that checks clean.

-   __[What it proves](checks.md)__

    The seven checks, and how to read what they report.

-   __[For agents](agents.md)__

    The whole loop, and the four things not to do.

</div>
