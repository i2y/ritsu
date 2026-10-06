# Diagnostic codes

Written by `yuen explain --all --format markdown`; do not edit.

<a id="e001"></a>

## E001 — Something cannot be read as a word of the language

**When**: A character the language does not have, a string not closed, an escape other than `\"` and `\\`, a date, hash, version or alias of the wrong shape, an ASCII name that starts with a digit, or a full-width space.

**Fix**: Correct it where it points: close the string with `"`; write a date as `2026-10-03`, a hash as `sha256:` and 16 lowercase hex digits, an alias as `(payment_day)`.

**Example**:

```req
requirements example v1
description "not closed
```

See also: [E002](#e002)

<a id="e002"></a>

## E002 — A word is written where it does not belong

**When**: A line that does not exist, a word the syntax does not take there, a word missing, or a word of the language (such as `text`) used as a name.

**Fix**: Use one of the forms the note gives.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  because "y"
```

See also: [E004](#e004)

<a id="e003"></a>

## E003 — The file does not start with a `requirements` line

**When**: The first line that is not a comment or blank is not `requirements …`, or the file is empty.

**Fix**: Start with the name and version of the set of requirements, like `requirements payment_terms v1`.

**Example**:

```req
role legal
```

See also: [E004](#e004)

<a id="e004"></a>

## E004 — A section or a line is out of order, or there are too many

**When**: The sections are not in the order `requirements`, `description`, `role`, `source`, `scope`, `requirement`; the lines of a requirement are not in the order `text`, `in force`, `owner`, `replaces`, `from`, `decided`, `satisfied by` and `not satisfied`, `verified by` and `not verified`; or a line that comes once (`description`, `text`, `in force`, `owner`, a record) comes twice.

**Fix**: Put them in their order, and delete the second one.

**Example**:

```req
requirements example v1
role legal
description "after the role"
```

See also: [E002](#e002)

<a id="e005"></a>

## E005 — The indentation does not line up

**When**: The indentation has a tab, the lines of a block are not indented alike, an indented line follows nothing that takes indented lines, or a record is not right under its link or waiver, indented deeper.

**Fix**: Indent with spaces, every line of a block alike, and a record deeper than its link.

**Example**:

```req
requirements example v1
role legal

requirement r1
	text "x"
```

<a id="e006"></a>

## E006 — A date that does not exist, or a period that ends before it starts

**When**: A date no calendar has, like `2026-02-30`, or a period whose end is before its start. Dates run from 0001-01-01 to 9999-12-31.

**Fix**: Write a date that exists, and a period start first.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-02-30 by legal "example"
```

See also: [E001](#e001)

<a id="e007"></a>

## E007 — A name is declared twice

**When**: A requirement (twice, without versions), an alias, a role, a source of the same file or the heading of a file comes twice, or a requirement has two aliases.

**Fix**: Rename one of them; if they are versions of one requirement, write `v1` and `v2`.

**Example**:

```req
requirements example v1
role legal
role legal
```

See also: [E009](#e009)

<a id="e008"></a>

## E008 — A name that is not declared

**When**: The role of an `owner`, a `decided … by`, the `by` of a record or `review --by` is not declared, or the requirement of a `from` or `replaces` is not in the project.

**Fix**: Declare the role with `role legal "…"`, or correct the misspelling.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal_dept
  decided 2026-10-03 by legal "example"
```

See also: [E007](#e007)

<a id="e009"></a>

## E009 — A version is wrong

**When**: A `v0`; the same version twice; a version of a requirement with more than one that does not write its version; pointing at a requirement with more than one version without saying which; or pointing at a version that does not exist.

**Fix**: Versions count from `v1`; when there are two or more, every version writes its own, and so does whatever points at one.

**Example**:

```req
requirements example v1
role legal

requirement r1 v0
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
```

See also: [E007](#e007), [E408](#e408)

<a id="e010"></a>

## E010 — A requirement lacks something it needs

**When**: A requirement has no `text` or no `owner`, no origin (neither `from` nor `decided`), or no alias although its name is not lowercase ASCII, digits and `_`.

**Fix**: Write the line missing; an alias goes right after the name, as in `PaymentDay(payment_day)`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  owner legal
  decided 2026-10-03 by legal "example"
```

<a id="e011"></a>

## E011 — A tool that does not exist

**When**: The first word of a naming is none of rulec, dandori, koyomi, chobo, geas, proto, file, yuen and sakai (`dir` is not a word of a naming either).

**Fix**: Write one of the nine; name any other file with `file "…"`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by excel "a.xlsx"
```

See also: [E012](#e012), [E013](#e013)

<a id="e012"></a>

## E012 — A kind or pair that cannot be written there

**When**: A kind the tool does not have, a child kind (`method`, `field`, `value`) not right after its parent, two child pairs, two pairs for a tool without nesting, a kind for file, or a kind without its name; also `source` or a `yuen` naming in a link or a scope, and a borrowed source that is not the `source` of a rulec or koyomi file.

**Fix**: Write one of the kinds the message gives; file names a whole file (`file "src/app.py"`).

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by dandori "order.flow" table reserve
```

See also: [E011](#e011), [E013](#e013)

<a id="e013"></a>

## E013 — A path that cannot be read

**When**: A path without quotes, empty, absolute, or outside the root once collapsed.

**Fix**: Write it in quotes, from the directory of the file the naming is in.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "/etc/hosts"
```

See also: [E011](#e011), [E012](#e012)

<a id="e101"></a>

## E101 — The copy of a source is not there

**When**: The copy of a pinned article (`sources/law/<id>@<date>/<element>.xml`), the file of a `file` source, or the spec of an `openspec` source, is not there. check never reads the network.

**Fix**: Bring the copy with `yuen source fetch`, or correct the path; an OpenSpec spec is a file of the project, so correct its path.

**Example**:

```req
requirements example v1
role legal

source cfr = law ecfr "37 CFR 1" asof 2026-01-01
  "§1.7" sha256:01de176ebe4740d7

requirement r1
  text "x"
  owner legal
  from @cfr "§1.7"
  not satisfied "left out in this example"
  not verified "left out in this example"
```

See also: [E102](#e102), [E103](#e103)

<a id="e102"></a>

## E102 — A source is not pinned

**When**: An article or an OpenSpec requirement cited has no pin line, or a pin line or a `file` source has no `sha256:`.

**Fix**: Write the first 16 digits of the SHA-256 of the copy, or of an OpenSpec requirement's block (the fixed line is shown; `yuen source pin` writes it too).

**Example**:

```req
requirements example v1
role legal

source cfr = law ecfr "37 CFR 1" asof 2026-01-01
  "§1.7"

requirement r1
  text "x"
  owner legal
  from @cfr "§1.7"
  not satisfied "left out in this example"
  not verified "left out in this example"
```

See also: [E101](#e101), [E103](#e103)

<a id="e103"></a>

## E103 — A copy does not match its pin

**When**: The hash of the copy differs from the `sha256:` of its pin: the copy changed after it was pinned. For an OpenSpec requirement, its block changed (an archived change, or an edit); the diff is shown when the block looked at is in reviewed/.

**Fix**: Read what changed (`yuen source outdated`), then pin it again.

**Example**:

```req
requirements example v1
role legal

source cfr = law ecfr "37 CFR 1" asof 2026-01-01
  "§1.7" sha256:0000000000000000

requirement r1
  text "x"
  owner legal
  from @cfr "§1.7"
  not satisfied "left out in this example"
  not verified "left out in this example"
```

See also: [E102](#e102), [E302](#e302)

<a id="e104"></a>

## E104 — A copy cannot be read

**When**: The copy of a law is not UTF-8 XML, or not what e-Gov or the eCFR serves (an article starts with `<Article>`, an eCFR section with `<DIV8>`); also an OpenSpec spec that is not UTF-8, has no `## Requirements` section (a change's delta spec among them), or holds two requirements of one name.

**Fix**: Fetch it again with `yuen source fetch` rather than editing it. For OpenSpec, name `openspec/specs/<capability>/spec.md`, and fix its form with `openspec validate --specs`.

**Example**:

```req
requirements example v1
role legal

source cfr = law ecfr "37 CFR 1" asof 2026-01-01
  "§1.7" sha256:6210aedce8fd1601

requirement r1
  text "x"
  owner legal
  from @cfr "§1.7"
  not satisfied "left out in this example"
  not verified "left out in this example"
```

`sources/law/37-CFR-1@2026-01-01/1.7.xml`:

```
not xml
```

See also: [E101](#e101)

<a id="e105"></a>

## E105 — A citation cannot be used

**When**: The article of a citation is not in a form read, the source is not declared in the same file, a law is cited without an article or an OpenSpec spec without a requirement, or a `file` source is cited with one.

**Fix**: Declare the source in the same file; cite a law by article or section (`@cfr "§1.7"`) and a `file` source whole (`@terms`).

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  from @usc "§1.1"
  not satisfied "left out in this example"
  not verified "left out in this example"
```

See also: [E102](#e102)

<a id="e106"></a>

## E106 — A borrowed source cannot be used

**When**: The file named declares no such source, or does not pin the article cited; also when the file is not there, or a copy cannot be read or does not match its pin.

**Fix**: Write a source and an article the file declares and pins; to cite another article, add its pin to that file.

**Example**:

```req
requirements example v1
role legal

source cfr = koyomi "a.cal" source cfr

requirement r1
  text "x"
  owner legal
  from @cfr "§1.7"
  not satisfied "left out in this example"
  not verified "left out in this example"
```

`a.cal`:

```
dates example v1

inputs
  origin : date  range >=2026-01-01 <=2026-12-31

date next_day = origin
  + 1 day
```

See also: [E105](#e105), [E203](#e203)

<a id="e107"></a>

## E107 — A requirement and what meets it read different texts of one article

**When**: A rule or a calendar that meets a requirement pins an article the requirement cites, and none of its copies has the text of the requirement's copy: one of them is old.

**Fix**: Read the diff of the texts, then fetch and pin the older copy again.

**Example**:

```req
requirements fire_extinguishers v1
description "A requirement and the rule that meets it read one section, each from a copy of its own, and the copies say different things. A test material"

role safety "decides how the regulation reads"
role dev "writes and corrects the rule"

source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:cacff282db4d47b4

requirement travel_distance
  text "Portable fire extinguishers for Class A hazards are placed so that the travel distance to any extinguisher is 75 feet or less, and for Class B hazards 50 feet or less"
  in force 2026-01-01..
  owner safety
  from @osha "§1910.157"
    reviewed 2026-10-04 by safety sha256:cacff282db4d47b4 -> sha256:8511f994d7d9e7a9
  satisfied by rulec "rules/osha_extinguisher.rule" table distance
    reviewed 2026-10-04 by safety sha256:8511f994d7d9e7a9 -> sha256:5681500476af8a56
  not verified "rulec checks that the table covers every hazard; this material names no claim"
    approved 2026-10-04 by safety sha256:8511f994d7d9e7a9
```

See also: [E103](#e103)

<a id="e108"></a>

## E108 — An OpenSpec spec has no requirement of that name

**When**: A pin line or a citation of an `openspec` source names a requirement the spec does not have. OpenSpec compares names as written, so a name that differs only in case or spaces gives it too (the near name is noted); so does a requirement an archived change renamed (RENAMED) or removed (REMOVED).

**Fix**: Write the name after the spec's `### Requirement:` as it is. If an archive changed it, look again at the requirements that read it, then correct the pin and the citations.

**Example**:

```req
requirements example v1
role api

source greeting = openspec "openspec/specs/greeting/spec.md"
  "Greeting by Name" sha256:0000000000000000

requirement r1
  text "x"
  owner api
  from @greeting "Greeting by Name"
  not satisfied "left out in this example"
  not verified "left out in this example"
```

See also: [E101](#e101), [E104](#e104), [W102](#w102)

<a id="w101"></a>

## W101 — A pinned article is cited by no requirement

**When**: A pin line under a source, and no `from` of a requirement of the same file cites the article.

**Fix**: If it is what is left after a citation was removed, delete the pin line.

**Example**:

```req
requirements example v1
role legal

source cfr = law ecfr "37 CFR 1" asof 2026-01-01
  "§1.7" sha256:01de176ebe4740d7

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

See also: [E102](#e102)

<a id="w102"></a>

## W102 — A requirement of an OpenSpec spec is pinned by no source

**When**: A spec the project reads as an `openspec` source holds a requirement no source of the project pins: no requirement of the project reads it. Said once for a spec, at the first source that names it.

**Fix**: Pin it and read it with a requirement. To leave one out, read it with a requirement all the same and write `not satisfied` and `not verified` with the reasons: leaving it out is then on record, with its approval.

**Example**:

```req
requirements example v1
role api

source greeting = openspec "openspec/specs/greeting/spec.md"

requirement r1
  text "x"
  owner api
  decided 2026-10-05 by api "example"
  not satisfied "left out in this example"
    approved 2026-10-05 by api sha256:fbdfb71af500ce5f
  not verified "left out in this example"
    approved 2026-10-05 by api sha256:fbdfb71af500ce5f
```

See also: [W101](#w101), [E108](#e108)

<a id="e201"></a>

## E201 — The file of an artifact is not there

**When**: The file a link names, or the path of a scope, is not there; also a directory written in a link.

**Fix**: Correct the path; if the file was renamed, correct the link.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "missing.txt"
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

See also: [E013](#e013)

<a id="e202"></a>

## E202 — The name of an artifact is not in its file

**When**: The name is not in the file (among the things its language gives, or in the `.proto`): written by its alias, or renamed (the candidates are given).

**Fix**: Write the name the language gives (not its alias); if it was renamed, correct the link.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by proto "a.proto" message Orders
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.proto`:

```
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
}
```

See also: [E201](#e201)

<a id="e203"></a>

## E203 — The language of what is named cannot answer for its file

**When**: The file named does not pass its language's check, or does not read; no end is made from a file that does not pass. The notes give what the language says.

**Fix**: Make the file pass its language's check.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by rulec "a.rule" table fees
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.rule`:

```
rule fee v1

inputs
  amount : money[JPY]  range >=0JPY <=10000JPY

outputs
  fee : money[JPY]  round down(1JPY)

table fees
| amount       | -> fee  |
| <5000JPY     | 500JPY  |
| >5000JPY     | 0JPY    |
```

See also: [E106](#e106), [E202](#e202)

<a id="e204"></a>

## E204 — The tool's JSON is not of a known shape

**When**: Never: it was for a tool's JSON lacking a key yuen reads.

**Fix**: Nothing to fix.

**Example**: Retired in ritsu 0.23.0: it was set aside for reading the other languages from the JSON of child processes, and yuen reads them through ritsu's ports, as types, and reads no JSON.

See also: [E203](#e203)

<a id="e205"></a>

## E205 — A .proto cannot be read

**When**: The `.proto` cannot be read by ritsu's reader of `.proto` files (not proto3, or an import that does not read).

**Fix**: Correct the `.proto`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by proto "a.proto" message Order
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.proto`:

```
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
```

See also: [E201](#e201)

<a id="e206"></a>

## E206 — The language of what a line names is not joined

**When**: The binary of yuen's own crate (`yuen`), given a project that names a thing of another language (a rule, a calendar, a book, a claim, a workflow, a context), which it holds none of and cannot read. It is said once for each language, where the first thing of it is named, and the check stops there. The exit code is 2: it is how the command is run, not what the project says.

**Fix**: Run the same command as `ritsu yuen` (`ritsu yuen check .`), which joins every language and reads them in the same process.

**Example**:

```req
requirements shipping v1
role legal

requirement r1
  text "a fee for every amount"
  owner legal
  decided 2026-10-04 by legal "an example"
  satisfied by rulec "fee.rule" table fees
```

`fee.rule`:

```
rule fee v1

inputs
  amount : money[USD]  range >=0USD <=10000USD

outputs
  fee : money[USD]  round down(1USD)

table fees
| amount    | -> fee |
| <5000USD  | 5USD   |
| >=5000USD | 0USD   |
```

See also: [E203](#e203)

<a id="w201"></a>

## W201 — No geas record, so whether the claim exists is not checked

**When**: Never: it was for a spec with no `geas map` record, where only the file could be checked.

**Fix**: Nothing to fix.

**Example**: Retired in ritsu 0.23.0: geas's port reads the claims from the spec, so whether a claim exists is checked with no record; a file of a scope that cannot be traced for want of a record is said in a note of E404.

See also: [E202](#e202), [E404](#e404)

<a id="e301"></a>

## E301 — A link no one has looked at yet

**When**: The link has no record (`reviewed …`) under it.

**Fix**: Once a person has read both ends, write the record with `yuen review … --by <role>`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "a.txt"
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

See also: [E302](#e302), [E303](#e303)

<a id="e302"></a>

## E302 — The upper end changed after the link was looked at

**When**: The record's hash of the upper end differs from its hash now. The upper end of a `from` is the article or the requirement it reads from, and of `satisfied by` and `verified by` the requirement; a requirement's end holds its sources' hashes, so a changed article marks the links below it too.

**Fix**: Read the diff; once a person has made sure the requirement still reads right and what meets it still does, write the record again with `yuen review`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by legal sha256:f1e653e8ce72c16f -> sha256:87428fc522803d31
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

`reviewed/f1e653e8ce72c16f`:

```
text the old x
```

See also: [E303](#e303), [E304](#e304), [W301](#w301)

<a id="e303"></a>

## E303 — The lower end changed after the link was looked at

**When**: The record's hash of the lower end differs from its hash now: the requirement, for a `from`, and the artifact, for `satisfied by` and `verified by`.

**Fix**: Read the diff; once a person has made sure it still meets the requirement, write the record again with `yuen review`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

`reviewed/0263829989b6fd95`:

```
b
```

See also: [E302](#e302), [W301](#w301)

<a id="e304"></a>

## E304 — A waiver is not approved, or the requirement changed after it was

**When**: A `not satisfied` or `not verified` has no approval (`approved …`) under it, or the approval's hash of the requirement differs from its hash now.

**Fix**: Once the owner has read the reason and approves it, write the approval with `yuen review … --by <role>`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

See also: [E301](#e301)

<a id="e305"></a>

## E305 — A record is not written right

**When**: A record is not of its form (no `->` or `by`, a `reviewed` under a waiver), or holds a number of hashes other than the link's upper ends.

**Fix**: A record is what `yuen review` writes: look again, and let it write the record anew.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

See also: [E301](#e301)

<a id="w301"></a>

## W301 — What was looked at is not in reviewed/, so no diff can be shown

**When**: The content a marked link was looked at with (`reviewed/<hash>`) is not there; the mark is still made by the hashes.

**Fix**: Keep `reviewed/` in git; a lost content can often be brought back from the history.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

See also: [E302](#e302), [E303](#e303)

<a id="e401"></a>

## E401 — Nothing meets the requirement, and no waiver says so

**When**: A version of a requirement has neither `satisfied by` nor `not satisfied`.

**Fix**: Write `satisfied by <artifact>`, or `not satisfied "<why>"` and have it approved.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

See also: [E402](#e402)

<a id="e402"></a>

## E402 — Nothing checks the requirement, and no waiver says so

**When**: A version of a requirement has neither `verified by` nor `not verified`.

**Fix**: Write `verified by <claim>`, or `not verified "<why>"` and have it approved.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

See also: [E401](#e401)

<a id="e403"></a>

## E403 — Something that checks nothing is on the side that verifies

**When**: `verified by` names something other than a geas or koyomi claim, the whole file of a tool that checks it, or a test file (a rulec output, a chobo transfer, a proto).

**Fix**: If it meets the requirement, write it after `satisfied by`; the side that verifies takes what can fail.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  verified by koyomi "payment_terms.cal" date payment_day
```

See also: [E012](#e012)

<a id="e404"></a>

## E404 — An artifact in scope traces to no requirement

**When**: No link names an artifact a `scope` gathers, nor anything containing it or in it.

**Fix**: Add a requirement whose `satisfied by` or `verified by` names it, or narrow the scope.

**Example**:

```req
requirements example v1
role legal

scope file "b.txt"

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`b.txt`:

```
b
```

See also: [E401](#e401)

<a id="e405"></a>

## E405 — The requirements make a cycle

**When**: Following `from <requirement>` and `replaces <requirement>` comes back to where it started.

**Fix**: Decide which one comes first, and delete the other's `from` or `replaces`.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  from r2
  not satisfied "left out in this example"
  not verified "left out in this example"

requirement r2
  text "y"
  owner legal
  from r1
  not satisfied "left out in this example"
  not verified "left out in this example"
```

See also: [E409](#e409)

<a id="e406"></a>

## E406 — The periods of the versions leave a gap

**When**: A version does not start on the day after the one before ends; the days in no version are given.

**Fix**: Start the next version on the day after the one before ends (the fixed line is shown).

**Example**:

```req
requirements example v1
role legal

requirement x v1
  text "x"
  in force 2026-01-01..2026-12-31
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:5ca1701c0312a54b
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:5ca1701c0312a54b

requirement x v2
  text "x"
  in force 2027-01-02..
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:4e7392102a031a5b
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:4e7392102a031a5b
```

See also: [E407](#e407), [E408](#e408)

<a id="e407"></a>

## E407 — The periods of the versions overlap

**When**: A version starts on or before the day the one before ends; the days and the two versions are given.

**Fix**: One version holds on a day: correct the end of the one or the start of the other.

**Example**:

```req
requirements example v1
role legal

requirement x v1
  text "x"
  in force 2026-01-01..2026-12-31
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:5ca1701c0312a54b
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:5ca1701c0312a54b

requirement x v2
  text "x"
  in force 2026-12-30..
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:7523d312c91d81c0
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:7523d312c91d81c0
```

See also: [E406](#e406), [E408](#e408)

<a id="e408"></a>

## E408 — The periods of the versions are not all written

**When**: A requirement with more than one version has a version without a period, a version that leaves its end open is not the last, one that leaves its start open is not the first, or the versions do not number in the order of their periods.

**Fix**: Give every version an `in force`, and number them in the order of their periods.

**Example**:

```req
requirements example v1
role legal

requirement x v1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f

requirement x v2
  text "x"
  in force 2027-01-01..
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:fa2942b05a851b79
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fa2942b05a851b79
```

See also: [E406](#e406), [E407](#e407)

<a id="e409"></a>

## E409 — What replaces a requirement does not start the day after it ends

**When**: What `replaces` names has no end, or the version that writes `replaces` does not start on the day after it.

**Fix**: End what is replaced, and start what replaces it on the day after (the fixed line is shown).

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  in force 2026-01-01..2026-12-31
  owner legal
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:5ca1701c0312a54b
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:5ca1701c0312a54b

requirement r2
  text "y"
  in force 2027-01-02..
  owner legal
  replaces r1
  decided 2026-10-03 by legal "example"
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:cb98f1b81b3a40fd
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:cb98f1b81b3a40fd
```

See also: [E406](#e406), [E405](#e405)

<a id="w401"></a>

## W401 — A waiver and a link on the same side

**When**: A version has both a `satisfied by` and a `not satisfied` (or a `verified by` and a `not verified`): the waiver is not needed.

**Fix**: Now that there is a link, delete the waiver.

**Example**:

```req
requirements example v1
role legal

requirement r1
  text "x"
  owner legal
  decided 2026-10-03 by legal "example"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f -> sha256:87428fc522803d31
  not satisfied "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
  not verified "left out in this example"
    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

See also: [E401](#e401)

<a id="w402"></a>

## W402 — A scenario of an OpenSpec requirement has no claim of its name

**When**: A requirement reads a requirement of an OpenSpec spec and is checked by claims of geas, and a scenario of the spec's requirement has no claim of its name among the claims that check it (`verified by geas …`; every claim of a spec named whole). It often comes after an archived change added a scenario.

**Fix**: Write a claim of that name with the person who reads the claims, and link it with `verified by` (`geas scenarios --draft` gives a frame); if a claim of another name runs the scenario, ask which of the two names is to change.

**Example**:

```req
requirements example v1
role api

source greeting = openspec "openspec/specs/greeting/spec.md"
  "Unknown paths" sha256:f140ceede48af8ff

requirement r1
  text "x"
  owner api
  from @greeting "Unknown paths"
  not satisfied "left out in this example"
  verified by geas "greeter.geas" claim "greets by name"
```

`greeter.geas`:

```
target api {
  serve "python3 server.py {port}"
  port auto
}

claim "greets by name" {
  when api.get("/greet?name=Alice")
  then status is 200
}
```

See also: [W102](#w102), [E402](#e402)

<a id="w901"></a>

## W901 — A key is written in the file

**When**: Somewhere in a `.req`, in a string or a comment alike, there is a value in the shape of a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, or a PEM private key, each a shape its provider fixes. Every language of ritsu looks for them the same way. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line.

**Fix**: Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line.

**Example**:

```req
requirements payment v1
# the Google Calendar API key the payment service reads holidays with: AIzaSyD-ritsu-fake-key-for-tests-000000

role accounting

requirement payment_day
  text "Closes on the 20th, pays on the 10th of the next month"
  owner accounting
  decided 2026-10-03 by accounting "decided for the example"
  not satisfied "left out in this example"
    approved 2026-10-03 by accounting sha256:54009bc976fdc31c
  not verified "left out in this example"
    approved 2026-10-03 by accounting sha256:54009bc976fdc31c
```
