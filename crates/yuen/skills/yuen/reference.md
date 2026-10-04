# The yuen reference

The whole language of `.req` files, how they name what other tools hold, the hashes and the
records that stop the check when something moves, the check itself, the commands, their exit codes
and their JSON. The design and the reasons behind each choice are in DESIGN.md
(in Japanese); every diagnostic code is in [codes.md](codes.md).

yuen is one of the languages of ritsu. Run it as `ritsu yuen <command>` (or as `yuen`, the link
ritsu installs): then the rules, calendars, books, specs, workflows and maps a project names are
read by their own languages in the same process.

## Files

A `.req` file holds requirements and where they come from. Its sections come in this order (E004
otherwise): the heading, `description`, `role`, `source`, `scope`, `requirement`.

```req
requirements payment_terms v1
description "Where the dates of koyomi's example payment_20th_close_next_10th.cal come from. The closed days come from the table of national holidays the calendar file pins, borrowed; the terms of payment were decided for this example"

role accounting "Decides the terms of payment"
role development "Writes and fixes koyomi's files"

source holidays = koyomi "calendars/tokyo_business_days.cal" source national_holidays

scope koyomi "payment_20th_close_next_10th.cal" date

requirement business_days
  text "A payment is made on a business day: when the payment day is a Saturday, a Sunday or a national holiday, it is made on the business day before"
  in force 2026-10-01..
  owner accounting
  from @holidays
    reviewed 2026-10-04 by accounting sha256:cec37a743c96995c -> sha256:d53e37ddddbb71fa
  satisfied by koyomi "payment_20th_close_next_10th.cal" date payment
    reviewed 2026-10-04 by development sha256:d53e37ddddbb71fa -> sha256:77727589d8318079
  verified by koyomi "payment_20th_close_next_10th.cal" claim paid_on_a_business_day
    reviewed 2026-10-04 by development sha256:d53e37ddddbb71fa -> sha256:a30c094c89e283df
```

- The heading is `requirements <name> v<n>`; the version is for people, and `api` and the exports
  copy it. It cannot be left out (E002).
- `description` comes at most once; `role`, `source`, `scope` and `requirement` any number of
  times.
- Indentation (spaces; a tab is E005) marks the lines of a requirement, the pins of a source and
  the record under a link or a waiver.
- `#` starts a comment to the end of the line. A string is one line, in `"…"`, and its only escapes
  are `\"` and `\\`.
- What is given to a command is one project: the files given, and for a directory every `.req`
  under it in path order (not under a directory whose name starts with `.`, nor `target` or
  `node_modules`). The names of requirements and roles are one in the project (E007); the names of
  sources are a file's own.

### Keywords

Every keyword has one English spelling. Names (the heading, requirements, roles, sources) can be
written in any language.

| Where | Words |
|---|---|
| line | `requirements` `description` `role` `source` `scope` `requirement` |
| source | `law` `file` `url` `asof` `sha256:` `egov` `ecfr` `source` |
| requirement | `text` `in force` `owner` `replaces` `from` `decided` `by` `satisfied by` `verified by` `not satisfied` `not verified` |
| record | `reviewed` `approved` `by` `->` |
| symbol | `@` `..` `,` `->` `=` `#` |

A name cannot be a word that starts a line or a word of the lines of a requirement, the records
included (E002). The tool words and kind words of a naming are keywords only where a naming is
written: a requirement may be called `output`.

### Names and aliases

A name is made of Unicode letters, digits and `_`; one that is ASCII and starts with a digit
(`30days`) is E001. A requirement whose name is not of the form `[a-z][a-z0-9_]*` carries an ASCII
alias in parentheses, `満了日_142条(last_day_142)` (E010 without it). The alias is one in the
project (E007), and every version of a requirement writes the same one. Inside a `.req` a
requirement is always named by its name; the alias is what `--requirement`, ReqIF, PROV and the
anchors of the `doc` page use.

## Roles

```req
role safety "Decides how the regulation reads"
```

The owner of a requirement, whoever decided, whoever looked at a link and whoever approved a waiver
is named by a role declared somewhere in the project (E008 otherwise). A role may be a person or a
team; its description may be left out.

## Sources

A source is written one of three ways.

```req
source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269
source 法 = law "342AC0000000023" asof 2026-04-01
  別表第一 sha256:0ba69792e960021e
source 民法 = koyomi "civil_code_period_end.ja.cal" source 民法
```

- **`law`** reads a law an article (a section) at a time, as rulec and koyomi do. Without a
  database word it is e-Gov (Japan); `ecfr` is the US eCFR. Each article cited is pinned on a line
  below the source. The copies sit beside the `.req`, at
  `sources/law/<ID>@<date>/<element>.xml` for e-Gov and
  `sources/law/<title>-CFR-<part>@<date>/<section>.xml` for the eCFR, with e-Gov's version in
  `revision.txt`; an article's name maps to its element as in rulec (`第143条第2項` →
  `MainProvision-Article_143-Paragraph_2`, `別表第一` → `AppdxTable_1`).
- **`file`** pins a whole document: `source <name> = file "<path>" [url "<url>"] sha256:<16>`.
  The path is from the `.req`, and the file is the copy; `url` is where `source fetch` and
  `source outdated` go.
- **A borrowed source**, `<tool> "<path>" source <name>`, is a source a rule (`rulec`) or a
  calendar (`koyomi`) declares and pins. Its copies and pins are that file's, held by its
  language's check, and are not written twice.

A requirement cites a source with `from @<source> <article>[, <article>…]`; a name that is not a
word is quoted (`@osha "§1910.157"`), and a `file` source is cited whole (`@holidays`).

The check never reads the network. A copy missing is E101 (`yuen source fetch`), an article cited
without a pin E102 (`yuen source pin`), a copy that differs from its pin E103, a copy that does
not read as one E104, a citation that cannot be read or names no declared source E105, a borrowed
source its file does not declare or pin E106, a pin no requirement cites W101. When a requirement
copies an article a rule or a calendar that meets it also pins, the two copies must say the same
(E107).

## Requirements

```req
requirement 契約書の印紙税額(contract_stamp_tax) v2
  text "不動産の譲渡に関する契約書の印紙税額は、記載された契約金額に応じて、別表第一の第1号文書の欄の税率による。契約金額が1万円未満のものは課税しない"
  in force 2027-04-01..
  owner 税務
  from @法 別表第一
```

A requirement starts with `requirement <name>[(<alias>)] [v<n>]`, and its lines come in this order
(E004 otherwise):

| Line | What it says | How many |
|---|---|---|
| `text "<sentence>"` | what is required | one (E010 without it) |
| `in force <date>..<date>` | the period it is in force; either end may be left out | at most one |
| `owner <role>` | who approves a change to it | one (E010 without it) |
| `replaces <requirement> [v<n>]` | a requirement of another name this one replaces | any |
| `from @<source> <article>…`, `from <requirement> [v<n>]` | where it comes from: articles, or another requirement | any |
| `decided <date> by <role> "<why>"` | a decision: who, when, why | any |
| `satisfied by <naming>`, `not satisfied "<why>"` | what meets it, or a waiver | any |
| `verified by <naming>`, `not verified "<why>"` | what checks it, or a waiver | any |

- A requirement comes from somewhere: at least one `from` or `decided` (E010).
- **Versions.** Requirements of one name written `v1`, `v2` are versions of one requirement. With
  two or more, each has `in force`, and the periods follow one another with no gap and no overlap
  (E406–E409). Versions split by when a requirement is in force; rewording a sentence stays in its
  version and the hashes catch it.
- `from <requirement>` reads one requirement from another; a requirement with several versions
  is named with its version (E009). A cycle is E405.
- `replaces` is for a requirement of another name (split, merged, renamed): the one replaced ends,
  and this one starts the next day.

## Links and waivers

```req
  satisfied by rulec "osha_extinguisher.rule" table distance
    reviewed 2026-10-04 by development sha256:849dc53f1d112c23 -> sha256:5681500476af8a56
  verified by rulec "osha_extinguisher.rule"
    reviewed 2026-10-04 by development sha256:849dc53f1d112c23 -> sha256:a52e955b88af88d6
```

`satisfied by` names what meets the requirement, `verified by` what checks it, one to a line. The
checking side takes only what a tool checks and can fail (E403 otherwise):

| Checked by | What checks it |
|---|---|
| `geas "<spec>" claim "<name>"` | a claim geas runs |
| `koyomi "<file>" claim <name>` | a claim koyomi holds on every day of its range |
| `rulec "<file>"`, `koyomi "<file>"`, `chobo "<file>"`, `dandori "<file>"`, `geas "<spec>"` | that tool's check of the file (for rulec: completeness, overlaps, examples) |
| `file "<path>"` | a test file; yuen does not know what runs it |

`not satisfied "<why>"` and `not verified "<why>"` waive a side, with the reason (it cannot be left
out). Every version of every requirement has at least one of `satisfied by` or `not satisfied`
(E401) and at least one of `verified by` or `not verified` (E402); a waiver beside a link on the
same side is W401.

## Scope

```req
scope chobo "refunds.book" transfer
```

`scope <naming> [<kind>]` declares the artifacts that must each trace to a requirement (E404).
With a kind at the end, every artifact of that kind in what is named; without one, what is named
itself (a directory stands for every file of the tool under it, a file at a time). A naming's
pairs and the kind are told apart by the number of words after the path: an odd number ends with
the kind. `source` and `yuen` cannot be written in a scope (E012).

## Naming what other tools hold

```
<tool> "<path>" [<kind> <name>]...
```

With no pair a naming is the file itself; with one pair, one thing in the file. Two pairs are for
what nests: `proto` `service S [method M]`, `message M [field f]`, `enum E [value V]`, and `rulec`
`enum E [value V]`; a child kind comes only right after its parent's pair (E012).

| Tool | Files | Kinds |
|---|---|---|
| `rulec` | `.rule` | `input` `output` `enum` (`value` below) `table` `clause` `define` `derive` `machine` `source` |
| `koyomi` | `.cal` | `input` `date` `claim` `source` |
| `chobo` | `.book` | `unit` `account` `transfer` |
| `geas` | `.geas` | `claim` |
| `dandori` | `.flow` | `task` `case` `record` (`field` below) `enum` (`value` below) `input` `output` |
| `proto` | `.proto` | `service` (`method` below) `message` (`field` below) `enum` (`value` below) |
| `file` | anything | none |
| `yuen` | `.req` | `requirement` `source` |
| `sakai` | `.ctx` | `context` `term` |

- **Paths** are written in quotes, from the directory of the file the naming is written in, with
  `/` between parts; `.` and `..` fold away. An empty path, an absolute one and one that folds out
  of the root are E013. The **root** is the nearest directory above the first path given that
  holds `.git` (else the directory given), or `--root <dir>`.
- **Names** are each language's own (not their aliases: E202 points at the name). A name is a word
  (no space, `"` or `#`) or a string in `"…"`; names are compared as written, case and all. A
  `proto` name is the name from the file's package, nested names joined with `.`.
- **The same, and inside**: two namings are the same when the tool, the path from the root and the
  pairs are the same. A file holds everything in it, and a parent pair its children.
- A tool unknown is E011, a kind or a pair the tool does not have E012, a thing the file does not
  hold E202, a file its language does not pass (or cannot read) E203, a file that is not there
  E201, and a language the running yuen does not hold E206 (the binary of yuen's own crate holds
  none: run it as `ritsu yuen`).

In JSON a naming is `{"text": …, "tool": …, "path": …, "items": [[kind, name], …]}`, in that order,
the path from the root; `text` is the naming written back, the path from the root and a name in
quotes only when it is not a word.

## Ends, hashes and records

Every link has two ends, and every end has its bytes and a hash: the first 16 hex digits of their
SHA-256.

| End | Its bytes |
|---|---|
| an article of a law | the copy, as the database served it (the same hash the pins have) |
| a `file` source | the file |
| a file named by any tool | the file |
| one thing in a file | the definition the language hands over for it: a rule's lines as `rulec fmt` writes them, a date's or claim's lines in koyomi, chobo's JSON of the unit, account or transfer, a claim's block in geas, a declaration's lines in dandori, a context's or term's lines in sakai, the fixed text of a `.proto` item |
| a requirement | the lines below |

A requirement's end is its sentence, then a `from` line for each article cited (`from law <db>
<ID> <article> sha256:<copy>`), file cited (`from file <path from the root> sha256:<file>`) and
requirement read from (`from requirement <name> v<n> sha256:<its end>`), and its period (`in force
<period>`), the lines after the first in byte order, each ending in LF. The owner, the decisions,
the links, the waivers, the name and the version are not in it: renaming a requirement does not ask
anyone to look at its artifacts again. Because the upper ends' hashes are in it, a change to an
article reaches every link below the requirements that cite it.

Under each link, `yuen review` writes who looked, when, and the hashes of both ends; under each
waiver, who approved it, when, and the requirement's hash:

```req
  from @民法 第141条, 第143条
    reviewed 2026-10-04 by 法務 sha256:0575c131b9f08063, sha256:6950bdfb988439b6 -> sha256:465b83ed8c251406
  not verified "初日を算入しないことは、koyomi doc のページで、条文と言い直しを見比べて確かめる。この例に、それを確かめる条件は無い"
    approved 2026-10-04 by 法務 sha256:a9ebc73907faddc8
```

People do not write these lines. `review` writes them, and keeps the bytes of both ends in
`reviewed/<hash>` beside the `.req` (not over 1 MiB, and only UTF-8), so that a later check can show
what changed. `reviewed/` goes into git with the `.req`.

A link with no record is E301; one whose upper end changed E302; one whose lower end changed E303,
with the diff from what was looked at; a waiver not approved, or approved before the requirement
changed, E304; a record that does not read E305; and W301 when the bytes looked at are not in
`reviewed/` to show a diff.

## The check

`yuen check` goes through seven stages, in order:

1. words and lines (E001–E006);
2. names (E007–E013, E403);
3. sources (E101–E106, W101): the copies against their pins, and what a borrowed source's language
   says;
4. artifacts (E201–E203, E205, E206, E107): what each named thing is, through its language;
5. cycles and periods (E405–E409);
6. links and hashes (E301–E305, W301);
7. coverage and scope (E401, E402, E404, W401).

An error in the first two stops the check there: until the names are known, nothing can be
compared. Errors in stages 3 and 4 let it go on, leaving out the links whose ends could not be
read. When the check passes, it says what it checked in one line:

```console
$ ritsu yuen check examples/osha/osha.req --root examples/osha
examples/osha/osha.req: ok — 1 requirement, whose 3 links are as they were looked at; every requirement is met and checked, or waived; the table in scope traces to a requirement
```

## Commands

```console
$ ritsu yuen --help
yuen 0.23.0

Write where each requirement comes from. Link what meets it. Stop when anything moves.

Usage:
  yuen check <path>...                      check the names, the copies of the sources against their pins, every record against the hashes now, the periods, the coverage and the scope
  yuen review <path>...                     record what a person has looked at: under each chosen link and waiver that is marked, write who looked, when, and the hashes of its ends, and keep what was looked at in reviewed/
  yuen trace <path>...                      show why something is the way it is, from one requirement, artifact or article back to the sources
  yuen affected <path>...                   answer which requirements a diff touches, with their owners, where they come from and the last decision on them; code is followed through geas's records to the claims, and on to the requirements
  yuen doc <path>...                        print the page of the requirements and where they come from (Markdown, or one HTML file), for whoever has to understand and check what the code is meant to do
  yuen api <path>...                        print the whole graph of the project as JSON, for other tools to read from the command's output alone
  yuen export reqif|prov <path>...          write the project out as ReqIF (for requirements tools) or W3C PROV (provenance); marks and gaps go in as states
  yuen source fetch|pin|outdated <path>...  fetch the copies of the sources, pin their hashes, or ask whether the originals moved on (only fetch and outdated read the network)
  yuen explain <CODE>                       look a diagnostic code up: when it comes, how to fix it, the smallest reproduction

For one command in detail: `yuen <cmd> --help` (`yuen help <cmd>` is the same page).
Every command takes --lang ja|en (default en; the YUEN_LANG or RITSU_LANG environment variable works too) and --root <dir>.
Exit codes: 0 no errors / 1 errors / 2 bad arguments or a file that cannot be read
```

- `check <path>... [--format json]`: the seven stages above.
- `review <path>... (--at <file.req>:<line>)... (--requirement '<name>[ v<n>]')... [--all] --by <role> [--date <YYYY-MM-DD>]`:
  writes a record under each chosen link and waiver that is marked, and says, a line each, what had
  changed and what it wrote. What is not marked is left alone (a date is not renewed without
  anyone looking again). `--at` takes the place a diagnostic gives. Run it only for the person who
  looked, with their role.
- `trace <path>... (--requirement '<name>[ v<n>]' | --artifact '<naming>' | --source '@<source> <article>') [--format json]`:
  why something is the way it is, back to the articles, quoted from the copies.
- `affected <path>... --diff <file|-> [--map <spec.geas>=<record>]... [--format json]`: the
  requirements a unified diff touches, with their owners, where they come from and the last
  decision on them. Code is followed through geas's records (before and after the change) to the
  claims, and on to the requirements those claims check; exit 1 when a change reaches no
  requirement.
- `doc <path>... [--format markdown|html] [--out <dir>]`: the page of the project for whoever has
  to understand and check what the code is meant to do: the files and the check, a traceability
  table, the sources and the articles each requirement cites (quoted from the copies), why each
  requirement is so (its decisions, links, records and marks, with their diffs), the scope, and
  every record by date. No page while stages 1 to 4 have errors (exit 1, the diagnostics on
  standard error); marks, gaps and what the scope misses are on the page (exit 0). The HTML page is
  one file that reads nothing from anywhere else, with a light and a dark palette.
- `api <path>...`: the whole graph as JSON (below).
- `export reqif|prov <path>... [--format provn|json] [--time <RFC 3339>] [--out <file>]`: ReqIF
  1.2 for requirements tools, or W3C PROV (PROV-N, or PROV-JSON with `--format json`). The words
  yuen adds to PROV (`yuen:Requirement`, `yuen:sha256`, …) are in the namespace
  `https://i2y.github.io/ritsu/ns/yuen#`: opening a word's IRI shows what it says, on the page
  <https://i2y.github.io/ritsu/ns/yuen/>.
- `source fetch|pin|outdated <path>...`: fetch the copies of the sources a project copies itself,
  pin their hashes in the `.req`, or ask whether the originals moved on. Only `fetch` and
  `outdated` read the network (e-Gov's API v2, the eCFR's versioner API, and a `file` source's
  `url`, through `curl`).
- `explain <CODE>`, `explain --all [--format markdown]`: a code, when it comes, how to fix it,
  and the smallest reproduction.

Every command takes `--lang ja|en` (else `YUEN_LANG`, then `RITSU_LANG`, else English) and
`--root <dir>`.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | no errors (there may be warnings); `doc` made its page |
| 1 | errors (a marked link is one); for `affected`, a change no requirement reaches; for `source outdated`, an original that moved on |
| 2 | bad arguments, a file that cannot be read, or an artifact of a language not joined (E206) |

## JSON

`check --format json` prints one object for the project; the diagnostics, with their diffs, are in
it:

```console
$ ritsu yuen check examples/refunds/refunds.req --root examples/refunds --format json
{"root":"examples/refunds","ok":true,"summary":"examples/refunds/refunds.req: ok — 1 requirement, whose 4 links are as they were looked at; every requirement is met and checked, or waived; the 2 transfers in scope all trace to a requirement","diagnostics":[]}
```

`api` prints the whole graph, its keys in this order: `yuen` (the version), `root`, `files`
(`path`, `name`, `version`, `sha256`), `roles`, `sources` (a `law` with `db`, `id`, `asof`,
`revision` and its `pins`; a `file` with `path`, `url`, `sha256`; `borrowed`, the naming it is
borrowed from, or null), `requirements` (`name`, `alias`, `version`, `file`, `line`, `text`,
`in_force`, `owner`, `replaces`, `sha256`, `from`, `decided`, `links`, `waivers`), `artifacts`
(the naming's keys, then `sha256`, `end` (`file` or `item`) and the articles its file pins),
`scopes` (`file`, `line`, `text`, `artifacts`, `untraced`) and `check` (`ok`, `summary`,
`diagnostics`). The `status` of a link or a waiver is one of `ok`, `unreviewed`, `up_changed`,
`down_changed`, `unapproved`, `bad_record` and `unreadable`. Paths are from the root, and `root` is
the root seen from where yuen runs.

## Environment

| Variable | What it does |
|---|---|
| `YUEN_LANG`, `RITSU_LANG` | the language of the text when `--lang` is not given (`en` or `ja`) |
| `YUEN_BLESS`, `RITSU_BLESS` | the tests write their golden files instead of comparing |
| `YUEN_PYTHON` | a Python with `prov==3.2.2` and `reqif==0.1.0`, for the tests of the exports (else `tools/.venv`) |
| `YUEN_XMLLINT` | xmllint, for the tests of ReqIF (else `xmllint` on PATH) |
| `YUEN_REQIF_XSD` | where `tools/reqif/fetch.sh` put ReqIF's schemas (else `tools/reqif/xsd`) |
| `YUEN_EGOV`, `YUEN_ECFR` | another address for e-Gov's or the eCFR's API (the tests' own server) |
| `YUEN_NET` | `1` lets the tests ask the real e-Gov and eCFR |
| `YUEN_CHROME`, `RITSU_CHROME` | the Chrome the tests draw the `doc` pages with |
