# Diagnostic codes

Written by `koyomi explain --all --format markdown`; do not edit.

<a id="e001"></a>

## E001 — Something cannot be read as a word of the language

**When**: A character the language does not have, a string not closed, a date or pin of the wrong shape (a pin is 16 lowercase hex digits), or an ASCII name that starts with a digit.

**Fix**: Correct it where it points: close the string with `"`, write a date as `2026-01-01`.

**Example**:

```cal
dates t v1
description "not closed
```

See also: [E006](#e006)

<a id="e002"></a>

## E002 — A word is written where it does not belong

**When**: The syntax does not take the word there: an operation that does not exist, an unknown way after `else`, an unknown convention after `roll`, and the like.

**Fix**: Use one of the forms the note lists.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  next month
```

See also: [E004](#e004)

<a id="e003"></a>

## E003 — The file does not start with a `calendar` or a `dates` line

**When**: The first line that is not a comment or blank is neither `calendar …` nor `dates …`.

**Fix**: Start a calendar like `calendar tokyo v1` and date functions like `dates payment_terms v1`.

**Example**:

```cal
inputs
  d : date  range >=2026-01-01 <=2026-01-31
```

See also: [E004](#e004)

<a id="e004"></a>

## E004 — A section is out of order, repeated, or in the wrong kind of file

**When**: Sections are out of their order, a section that comes once comes twice, a section belongs to the other kind of file (`inputs` in a calendar), or a table has no `format` or `covers` line.

**Fix**: A calendar goes: heading, `description`, `offset`, `use calendar`, `source`, `closed` and `open`. A dates file goes: heading, `description`, `use calendar`, `source`, `inputs`, `date`, `claims`, `examples`.

**Example**:

```cal
calendar t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31
```

See also: [E002](#e002), [E003](#e003)

<a id="e005"></a>

## E005 — The indentation does not line up

**When**: The indentation has a tab, the lines of one block are not indented alike, or an indented line follows nothing that takes indented lines.

**Fix**: Indent with spaces, every line of a block by the same amount.

**Example**:

```cal
dates t v1

inputs
	d : date  range >=2026-01-01 <=2026-01-31
```

<a id="e006"></a>

## E006 — A date, a month and day, or a time that does not exist

**When**: A value no calendar or clock has, like `2026-02-30`, `13-01` or `25:00`. Dates run from 0001-01-01 to 9999-12-31.

**Fix**: Write one that exists.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-02-01 <=2026-02-30
```

See also: [E001](#e001)

<a id="e007"></a>

## E007 — A name is declared twice

**When**: An input or date name, an alias, a claim name, a source name or an example column comes twice. An input and a date cannot have the same name.

**Fix**: Rename one of them.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  + 1 day
date x = d
  + 2 days
```

See also: [E008](#e008)

<a id="e008"></a>

## E008 — A name is not declared

**When**: The start of a date, a number in an operation, a claim, an example column or a `closed` table names something not declared.

**Fix**: Correct the spelling, or declare it.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = e
  + 1 day
```

See also: [E007](#e007), [E011](#e011)

<a id="e009"></a>

## E009 — A name or an alias is a reserved word

**When**: A name or an alias is a keyword of koyomi, an alias is a reserved word of a target (TypeScript, Python, Go, Rust, PostgreSQL), or it collides with a name the generated code defines (`is_open`, `date`, `<alias>_at`, …).

**Fix**: Choose another name or alias.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date select = d
  + 1 day
```

See also: [E010](#e010)

<a id="e010"></a>

## E010 — A public name has no ASCII alias

**When**: The name of the heading, an input or a date is not of the form `[a-z][a-z0-9_]*` and has no alias in parentheses, or its alias is not of that form.

**Fix**: Add the alias the generated code will use, like `Received(received)`.

**Example**:

```cal
dates t v1

inputs
  Received : date  range >=2026-01-01 <=2026-01-31
```

See also: [E009](#e009)

<a id="e011"></a>

## E011 — A type does not fit

**When**: An integer input where a date goes, a date where a number goes, a claim that compares integers, or an example value of the wrong type.

**Fix**: Take a date from the date input or another date, and a number from a number or an integer input.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31
  n : int   range >=1 <=3

date x = n
  + 1 day
```

See also: [E008](#e008)

<a id="e012"></a>

## E012 — There is not exactly one `date` input

**When**: The `inputs` of a dates file have no date or more than one. One date keeps the check exhaustive.

**Fix**: Keep one date; for a computation that needs a second one, take the days between as an integer input.

**Example**:

```cal
dates t v1

inputs
  a : date  range >=2026-01-01 <=2026-01-31
  b : date  range >=2026-01-01 <=2026-01-31
```

<a id="e013"></a>

## E013 — A range is wrong

**When**: An input's range has an end missing, is empty, or is written with `>` or `<`; an integer input added can be negative; a day of the month can fall outside 1..31.

**Fix**: Write both ends with `>=` and `<=`, like `range >=2026-01-01 <=2026-12-31`.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01
```

<a id="e014"></a>

## E014 — Dates start from each other in a circle

**When**: Following where each date starts from comes back to it.

**Fix**: Start one of them from the date input.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = y
  + 1 day
date y = x
  + 1 day
```

<a id="e015"></a>

## E015 — The calendar of `use calendar` cannot be used

**When**: It cannot be read, it is not a calendar file, reading it leads back to the file, or its offset differs.

**Fix**: Correct the path (from this .cal's directory) or the file it names.

**Example**:

```cal
dates t v1
use calendar "nowhere.cal"

inputs
  d : date  range >=2026-01-01 <=2026-01-31
```

<a id="e101"></a>

## E101 — The copy of a source is not there

**When**: The `file` of a table, or the copy of a pinned article of a law (under `sources/law/<law id>@<date>/`), is missing. check never reads the network.

**Fix**: Take the copy with `koyomi source fetch`, or correct the path.

**Example**:

```cal
calendar t v1

source holidays = file "holidays.csv" sha256:899aee90fcd554a9
  format csv
  covers 2026-01-01..2026-12-31

closed holidays
```

See also: [E102](#e102), [E103](#e103)

<a id="e102"></a>

## E102 — A source is not pinned

**When**: A table's line, or the pin line of a law's article, has no `sha256:`.

**Fix**: Pin it with the first 16 digits of the copy's SHA-256; the fix gives the line with the copy's own.

**Example**:

```cal
calendar t v1

source holidays = file "holidays.csv"
  format csv
  covers 2026-01-01..2026-12-31

closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2026-05-04,Greenery Day
```

See also: [E101](#e101), [E103](#e103)

<a id="e103"></a>

## E103 — A copy does not match its pin

**When**: The first 16 digits of the copy's SHA-256 differ from the pin: the copy changed after it was pinned.

**Fix**: Read what changed (`koyomi source outdated`), then pin it again.

**Example**:

```cal
calendar t v1

source holidays = file "holidays.csv" sha256:0123456789abcdef
  format csv
  covers 2026-01-01..2026-12-31

closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2026-05-04,Greenery Day
```

See also: [E102](#e102)

<a id="e104"></a>

## E104 — A copy cannot be read

**When**: The copy is not in its encoding (bytes Shift_JIS or UTF-8 does not have), a line does not start with a date, has too many values or repeats a date, the JSON cannot be read or lacks the division, or there are no rows.

**Fix**: Correct the line it names, or the encoding under `format`.

**Example**:

```cal
calendar t v1

source holidays = file "holidays.csv" sha256:6467b916662fb97a
  format csv
  covers 2026-01-01..2026-12-31

closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2026-13-01,Day off
```

<a id="e105"></a>

## E105 — A row of a table is outside `covers`

**When**: `covers` is the span the table lists every closed day of, and a row lies outside it.

**Fix**: When the table grew, widen `covers` too; the fix gives a span that holds the rows.

**Example**:

```cal
calendar t v1

source holidays = file "holidays.csv" sha256:899aee90fcd554a9
  format csv
  covers 2026-01-01..2026-03-31

closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2026-05-04,Greenery Day
```

See also: [E106](#e106)

<a id="e106"></a>

## E106 — `covers listed years`, and a year has no rows

**When**: A year between the first and the last with rows has none: it cannot be told whether it had no closed days or fell out of the table.

**Fix**: Write the span as dates, like `covers 2025-01-01..2027-12-31`.

**Example**:

```cal
calendar t v1

source holidays = file "holidays.csv" sha256:cf519bbe72d28ea7
  format csv
  covers listed years

closed holidays
```

`holidays.csv`:

```
2025-01-01,New Year's Day
2027-01-01,New Year's Day
```

See also: [E105](#e105)

<a id="e107"></a>

## E107 — The offset is not of the form `±HH:MM`

**When**: `offset` names a time zone, like `Asia/Tokyo`, or is a word like `UTC`. A zone with daylight saving time is an hour off a fixed offset for half the year, so it is not taken.

**Fix**: Write the number, like `offset +09:00`. For a place with daylight saving time, give dates only.

**Example**:

```cal
calendar t v1
offset Asia/Tokyo

closed weekly sat, sun
```

See also: [E110](#e110)

<a id="e108"></a>

## E108 — The calendar has no business day at all

**When**: Every day of the data range is closed, or the tables' `covers` do not overlap. Looking for a business day would never end.

**Fix**: Look at the closing lines again.

**Example**:

```cal
calendar t v1

closed weekly mon, tue, wed, thu, fri, sat, sun
```

<a id="e109"></a>

## E109 — A line asks which days are closed, and there is no calendar

**When**: There is `business days`, `roll`, `if closed`, or a claim with `is open` or `business days`, and no `use calendar`.

**Fix**: Write `use calendar "<file>"` after the heading.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  roll following
```

See also: [E110](#e110)

<a id="e110"></a>

## E110 — `at` is written, and the calendar has no offset

**When**: A date has `at 09:00` or `at end of day`, and its calendar (with the ones it reads) has no `offset`.

**Fix**: Give the calendar an offset, like `offset +09:00`, or delete the `at` line.

**Example**:

```cal
dates t v1
use calendar "weekends.cal"

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  roll following
  at 09:00
```

`weekends.cal`:

```
calendar weekends v1

closed weekly sat, sun
```

See also: [E107](#e107)

<a id="e111"></a>

## E111 — A citation of a law cannot be used

**When**: A citation names a source not declared, a source that is not a law, no article, a fragment of a form that cannot be cited (only articles, paragraphs and items of the main provisions), or one that is not pinned.

**Fix**: Declare `source 民法 = law "<law id>" asof <date>` and pin every article cited under it, like `第143条 sha256:…`.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d    @民法 第143条
  + 1 day
```

See also: [W102](#w102), [E101](#e101)

<a id="w101"></a>

## W101 — A day under `open` is a business day anyway

**When**: No `closed` line names the day written under `open`.

**Fix**: Unless the date is a slip, delete the line.

**Example**:

```cal
calendar t v1

closed weekly sat, sun
open 2026-12-28 "Special opening"
```

<a id="w102"></a>

## W102 — A pinned article is cited nowhere

**When**: No line cites, with `@`, an article pinned under a law: what is left after a citation was removed.

**Fix**: Delete the pin line, or cite it on the line it belongs to.

**Example**:

```cal
dates t v1
source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条 sha256:fc8c35a0769d3b35

inputs
  d : date  range >=2026-01-01 <=2026-01-31
```

See also: [E111](#e111)

<a id="e201"></a>

## E201 — An operation that can land on a missing day does not say what to do there

**When**: Adding months, or `day N of month` and `close day N` with N that can be 29 or more, has no `else`. It is decided from what is written, so it comes even when the range never lands on one.

**Fix**: Write `else end_of_month` (the end of that month), `else start_of_next_month` (the first of the next) or `else reject` (the check makes sure it never happens).

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-12-31

date x = d
  + 1 month
```

See also: [E202](#e202), [W201](#w201)

<a id="e202"></a>

## E202 — An `else reject` operation lands on a missing day in the range

**When**: `else reject` says it never happens in the range, and an input makes it happen.

**Fix**: Say what to do instead, or narrow the range.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-12-31

date x = d
  + 1 month else reject
```

See also: [E201](#e201)

<a id="e203"></a>

## E203 — A computation asks whether a day outside the data range is a business day

**When**: For an input in the range, the computation goes past what the calendar's tables know and asks whether that day is a business day.

**Fix**: Narrow the input's range to the one the fix gives, or take the copy again when a newer table is out.

**Example**:

```cal
dates t v1
use calendar "closed_days.cal"

inputs
  d : date  range >=2026-12-01 <=2026-12-31

date x = d
  + 5 business days
```

`closed_days.cal`:

```
calendar closed_days v1

source holidays = file "holidays.csv" sha256:899aee90fcd554a9
  format csv
  covers 2026-01-01..2026-12-31

closed weekly sat, sun
closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2026-05-04,Greenery Day
```

See also: [E108](#e108)

<a id="e204"></a>

## E204 — A computed date falls outside 0001-01-01..9999-12-31

**When**: For an input in the range, the computation goes outside the dates there are.

**Fix**: Narrow the range.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=9999-12-01 <=9999-12-31

date x = d
  + 1 day
```

<a id="w201"></a>

## W201 — An operation that never lands on a missing day has an `else`

**When**: N of `day N of month` or `close day N` is 28 or less, or the operation takes no `else` at all.

**Fix**: Delete the `else …`.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  day 10 of month +1 else end_of_month
```

See also: [E201](#e201)

<a id="w202"></a>

## W202 — There is a shorter way to write the same thing

**When**: `day 31 of month ±k else end_of_month` (the same as `end of month ±k`), `day 1 of month ±k` (the same as `start of month ±k`), or `close day 31 else end_of_month` (the same as `close end of month`).

**Fix**: Write the short form the fix gives.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  close day 31 else end_of_month
```

<a id="e301"></a>

## E301 — A claim fails on some inputs

**When**: Computed on every input of the range, the claim fails on at least one.

**Fix**: Compute an input it fails on with `koyomi eval`, then correct the rule, the claim or the range; which of them is wrong is for a person to decide.

**Example**:

```cal
dates t v1
use calendar "weekends.cal"

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  + 1 day

claims
  business_day : x is open
```

`weekends.cal`:

```
calendar weekends v1
offset +09:00

closed weekly sat, sun
```

See also: [E302](#e302)

<a id="e302"></a>

## E302 — A date is not monotonic

**When**: On a pair of adjacent days, the later day gives an earlier date. It happens with `if closed`.

**Fix**: See whether `roll` says it; if the date need not be monotonic, delete the claim.

**Example**:

```cal
dates t v1
use calendar "weekends.cal"

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  if closed + 3 days

claims
  later_is_later : x is monotonic
```

`weekends.cal`:

```
calendar weekends v1
offset +09:00

closed weekly sat, sun
```

See also: [E301](#e301)

<a id="e303"></a>

## E303 — An example has a different value

**When**: A date in an example row differs from what the reference interpreter computes, or the row's input is outside its range.

**Fix**: Read the computation, then correct the example or the rule; the fix gives the row as computed.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  + 1 day

examples
| d          | -> x       |
| 2026-01-01 | 2026-01-03 |
```

See also: [E304](#e304)

<a id="e304"></a>

## E304 — The examples lack a column for an input or a date

**When**: The `examples` table has no column for an input or a date. Only values a person wrote can catch a mistake the reference interpreter and the generated code share.

**Fix**: Add the columns missing (a date's is `-> <name>`).

**Example**:

```cal
dates t v1

inputs
  d : date  range >=2026-01-01 <=2026-01-31

date x = d
  + 1 day

examples
| d          |
| 2026-01-01 |
```

See also: [E303](#e303)

<a id="e305"></a>

## E305 — The check is over its budget (nothing was checked)

**When**: The input combinations (the days of the date's range times the size of every integer input's range) are more than the budget. It never tries some and passes.

**Fix**: Narrow a range, split the file, or raise the budget with `--budget`.

**Example**:

```cal
dates t v1

inputs
  d : date  range >=0001-01-01 <=9999-12-31
  n : int   range >=1 <=100

date x = d
  + n days
```
