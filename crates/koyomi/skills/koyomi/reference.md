# The koyomi reference

The whole language, the commands, and the formats koyomi reads and writes. DESIGN.md
(in Japanese) says why each part is the way it is and what was set aside; the diagnostics are
in [codes.md](codes.md), and the generated code in [generated-code.md](generated-code.md).

## Files

A `.cal` file is one of two kinds, told apart by its first line.

- **calendar**: which days are closed (days of the week, days every year, a table of holidays,
  given days), the days that are open anyway, and a UTC offset.
- **dates**: dates computed from one input date (and integers), the claims they keep, and
  worked examples. It reads a calendar with `use calendar`.

A calendar from the examples, whole:

```cal
calendar england_and_wales v1
description "Saturdays, Sundays and the bank holidays of England and Wales, as GOV.UK lists them. No offset: England and Wales has daylight saving time, so this calendar gives dates only"

source bank_holidays = file "data/bank-holidays.json" url "https://www.gov.uk/bank-holidays.json" sha256:538b3482c28b85ec
  format govuk "england-and-wales"
  covers listed years

closed weekly sat, sun
closed bank_holidays
```

And a dates file that reads it:

```cal
dates close_20th_pay_10th v1
description "Closes on the 20th; pays on the 10th of the next month, or on the business day before when that day is closed in England and Wales. The claim within_60_days_of_receipt is this example's own, written as it reads; it does not say how any law is read"
use calendar "calendars/england_and_wales.cal"

inputs
  received : date  range >=2026-01-01 <=2028-11-20

date closing = received
  close day 20          # "closes on the 20th"

date payment = closing
  day 10 of month +1    # "pays on the 10th of the next month"
  roll preceding        # "on the business day before when that day is closed"

claims
  paid_on_a_business_day      : payment is open
  within_60_days_of_receipt   : payment <= received + 60 days
  later_receipt_later_payment : payment is monotonic

examples
| received   | -> closing | -> payment |
| 2026-04-01 | 2026-04-20 | 2026-05-08 |
| 2026-12-21 | 2027-01-20 | 2027-02-10 |
```

### The order of a file

The sections come in a fixed order, and a file that breaks it gets E004.

| Kind | Order |
|---|---|
| calendar | the heading, `description`, `offset`, `use calendar`, `source` (tables and laws), `closed` and `open` (mixed as you like) |
| dates | the heading, `description`, `use calendar`, `source` (laws only; tables belong to calendars), `inputs`, `date` (any number), `claims`, `examples` |

`description`, `offset`, `use calendar`, `inputs`, `claims` and `examples` come at most once. The
heading is `calendar` or `dates`, a name, and a version (`v1`); the version is for people, and
koyomi only copies it into the generated code and `koyomi api`.

Indentation, in spaces, marks what is under a line: the lines of a section, the operations of a
date, the lines under a source. Lines at one level are indented alike (E005); a tab is refused.
`#` starts a comment, to the end of the line.

### Names and aliases

A name is letters, digits and `_`, in any script, and does not start with a digit, unless what
follows the digits is not ASCII (`40営業日以内` is a name; `30days` is E001, a missing space). The
names of a file, its inputs and its dates become identifiers in five languages, so each needs an
ASCII alias in parentheses, `[a-z][a-z0-9_]*`, unless the name is already of that form:

```cal
dates net30 v1
  received : date  range >=2026-01-01 <=2028-11-20
dates 支払条件(payment_terms) v1
  受領日(received) : date  range >=2026-01-01 <=2027-11-20
date 支払日(payment) = 締め日
```

The last three lines are from the Japanese version of an example. A name in English that is not of
that form takes an alias too (`InvoiceDate(invoice_date)`).

Claims and sources are not in the generated code and need no alias. A name or an alias cannot be
a keyword, a reserved word of a target language, or a name the generated code uses itself
(`is_open`, `Date`, `KoyomiError`, `<alias>_at`, …): E009.

### Keywords

The keywords are English, one spelling each. `day` and `days`, `month` and `months`, `year` and
`years` are the same word in the grammar's two numbers, not two spellings of one meaning.

| Where | Words |
|---|---|
| at the start of a line | `calendar` `dates` `description` `offset` `source` `closed` `open` `use` `inputs` `date` `claims` `examples` |
| a source line | `file` `url` `law` `asof` `sha256:` |
| under a source | `format` `covers` `csv` `govuk` `utf8` `shift_jis` `listed` `years` |
| a calendar | `weekly` `every` `mon` `tue` `wed` `thu` `fri` `sat` `sun` |
| a type and a range | `date` `int` `range` |
| an operation | `+` `-` `day` `days` `business` `month` `months` `year` `years` `of` `start` `end` `close` `roll` `following` `preceding` `modified` `if` `closed` `else` `at` |
| a day the month does not have | `end_of_month` `start_of_next_month` `reject` |
| a claim | `is` `open` `monotonic` `=` `<` `<=` `>` `>=` |

The symbols are `->` (an output column of the examples), `..` (a span of days, both ends
included), `@` (a citation of a law) and `#` (a comment).

## Calendars

| Line | What it says |
|---|---|
| `closed weekly sat, sun` | those days of every week are closed |
| `closed every 12-29..01-03 "New Year break"` | those days of every year are closed; the span may cross the new year, and may be one day (`closed every 05-01 "Founding day"`) |
| `closed 2000-08-14..2000-08-16 "Summer closure"` | those days are closed (one day: `closed 2099-12-30 "The day before the last business day"`) |
| `closed bank_holidays` | the days of the table `bank_holidays` are closed |
| `open 2000-01-03 "Extra business day"` | the day is open, whatever closes it |
| `offset +09:00` | the UTC offset the times are given at |
| `use calendar "<file>"` | another calendar, which this one adds to |

A day is a business day when an `open` line names it, or when no `closed` line does; `open`
always wins, and the order of the lines means nothing. `use calendar` reads another calendar and
merges its lines with this one's, closings and openings alike (a company's days off on top of
the national holidays). A calendar has one offset, and one that differs from the calendar it
reads is E015. The offset is `±HH:MM`: a time zone's name (`Asia/Tokyo`) is refused (E107),
because a zone with daylight saving time has two offsets a year; for such a place, give dates
and no times.

**The data range.** A table of holidays knows a span of days (its `covers`). The calendar knows
the days every table it reads knows, and a calendar without a table knows every day,
0001-01-01 to 9999-12-31. Whether a day outside the data range is open is not known, and a
computation that asks stops: the check says where (E203), and the generated code raises an
error of kind `data`. A calendar with no business day at all is E108.

## Sources

### Tables of holidays

```cal
source bank_holidays = file "data/bank-holidays.json" url "https://www.gov.uk/bank-holidays.json" sha256:538b3482c28b85ec
  format govuk "england-and-wales"
  covers listed years
source national_holidays = file "data/syukujitsu.csv" url "https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv" sha256:cec37a743c96995c
  format csv shift_jis
  covers 1955-01-01..2027-12-31
```

The second is the Cabinet Office's table of Japan's holidays, in Shift_JIS.

- `file`: the copy, from the `.cal`'s directory. `url`: where `koyomi source fetch` and
  `koyomi source outdated` take it from. `sha256:`: the first 16 hex digits of the SHA-256 of the
  copy's bytes, which pin it.
- `format csv [utf8 | shift_jis]`: a line is `<date>,<name>`; the date is `2026-01-01` or
  `2026/1/1`; a first line whose first value is not a date is a heading and is skipped; the name
  may be left out, and a value with `,` in it is quoted. UTF-8 unless `shift_jis` is written.
  Shift_JIS is read with the WHATWG Encoding Standard's table, and a byte it cannot read is an
  error that says where, never a replacement character.
- `format govuk "<division>"`: GOV.UK's `bank-holidays.json`, one of `england-and-wales`,
  `scotland` and `northern-ireland`. A holiday with `notes` gets them in parentheses
  (`Boxing Day (Substitute day)`).
- `covers <date>..<date>`: the span the table lists every closed day of. `covers listed years`:
  from 1 January of the first year with a row to 31 December of the last, refused when a year in
  between has no row (E106). The span is written rather than guessed from the rows, because a
  company's calendar is often made for a fiscal year.

The copy is kept as the bytes the source serves, never converted, so that anyone can check a pin
with `curl -s <url> | shasum -a 256`. A missing copy is E101, a missing pin E102 (it says the pin
to write, and `koyomi source pin` writes it), a copy that differs from its pin E103, a copy that
cannot be read E104, and a row outside `covers` E105. `koyomi check` never reads the network.

### Laws

A law is cited from e-Gov, an article, a paragraph or an item at a time. The articles are named as
e-Gov names them, whatever the language of the rest of the file:

```cal
source civil_code = law "129AC0000000089" asof 2026-10-01
  第140条 sha256:e880059021fbb67d
  第141条 sha256:0575c131b9f08063
  第142条 sha256:fc8c35a0769d3b35
  第143条 sha256:6950bdfb988439b6
date first_day = origin                                       @civil_code 第140条
date last_day = first_day                                     @civil_code 第141条, 第143条
```

The copies are `sources/law/<law id>@<asof>/<element>.xml` beside the `.cal` (what e-Gov law API
v2's `law_data` returns for the element), with the revision e-Gov served in `revision.txt`. A
citation, `@<source> <article>[, <article>…]`, goes at the end of a `date` line, an operation, an
`at` line, a claim, or a calendar's `closed` and `open` lines, before the comment. Articles,
paragraphs and items of the main provisions can be cited (`第143条`, `第143条第2項`,
`第143条第2項第1号`, `第20条の2`, in kanji numerals too); supplementary provisions and appended
tables cannot yet. Every article cited is pinned under its source (E111), and a pin nothing cites
is W102.

koyomi does not read a law to decide a computation. The person who writes the `.cal` writes the
computation the article is read as, and the citation records which text it was written against:
the page for people quotes it beside the computation, and `koyomi source outdated` says when a
later revision changes it.

## Dates files

### Inputs

```cal
inputs
  received      : date  range >=2026-01-01 <=2027-10-01
  closing_day   : int   range >=1 <=31
  payment_month : int   range >=1 <=2
  payment_day   : int   range >=10 <=31
```

The types are `date` and `int`. A dates file has exactly one `date` input (E012) and any number
of `int` inputs, and every input has both ends of its `range` (E013). The range is what the check
computes every value of, what the generated code refuses outside of, and what decides whether a
computation can leave the data range. One date keeps the check exhaustive: a date over a century
is 36,525 values, two of them over a billion.

### Dates and operations

A date starts from an input or another date, and the operations under it apply one after another,
top to bottom, a line each:

```cal
date payment = closing
  day payment_day of month +payment_month else end_of_month  # payment_day of the month payment_month later; at the end of the month when it has no such day
  roll preceding                                             # on the business day before when that day is closed
```

| Operation | What it gives | Needs a calendar | Needs `else` |
|---|---|---|---|
| `+ 30 days`, `- 1 day` | so many days later or earlier | no | no |
| `+ 5 business days`, `- 2 business days` | so many business days later or earlier | yes | no |
| `+ 1 month else …`, `- 3 months else …` | the same day so many months later or earlier | no | always |
| `+ 1 year else …`, `- 2 years else …` | the same as 12 months a year | no | always |
| `day 10 of month +1` | the 10th of the month so many months away (`+0` may be left out; `-1` is the month before) | no | when the day can be 29 or more |
| `start of month +1`, `end of month +2` | the first or the last day of the month so many months away | no | no |
| `close day 20` | closing on the 20th: the closing day of the period the day falls in | no | when the day can be 29 or more |
| `close end of month` | closing at the end of the month: the last day of the day's month | no | no |
| `roll following`, `roll preceding`, `roll modified following`, `roll modified preceding` | a closed day moved to a business day | yes | no |
| `if closed <operation>` | the operation, on a closed day only | yes | as the operation |

A number in an operation can be the name of an integer input (`close day closing_day else end_of_month`,
`+ day_count business days`); whether the operation needs `else` is then decided by the input's range.
Numbers are 0 or more, and `+` and `-` give the direction.

Exactly what each operation does, for a day `z` with year, month and day `(y, m, d)`:

- `± n months else P`: the day `d` of the month `n` months away, or what `P` says when that month
  has no day `d`. `± n years` is `± 12n months`, rounded once.
- `day N of month ±k else P`: the day `N` of the month `k` months away, or `P`.
- `close day N else P`: of the closing days of the months (the day `N` of each, or `P`), the
  earliest on or after `z`.
- `roll following` and `roll preceding`: `z` when it is open, else the first business day after
  or before it. The `modified` ones do the same, unless the day they find is in another month;
  then they go the other way.
- `+ n business days`: counting starts the day after `z` (the day before, for `-`), whether `z` is
  open or not, and the `n`-th business day is the result. Saturday's first business day after is
  Monday, as Friday's is. `+ 0 business days` is `z` when open and the next business day when not
  (`- 0`, the business day before), so a count of business days always ends on one.
- `if closed <operation>`: on a closed day, the operation once; on an open day, nothing.
  `if closed + 1 day` is the day after, whatever that day is.

### Days a month does not have

Adding months, the day `N` of a month, and closing on the day `N` can land on a day the month does
not have (30 February). Such an operation says what to do then, or the check stops (E201):

| `else` | What it gives | `2026-01-31` `+ 1 month` |
|---|---|---|
| `else end_of_month` | the last day of that month | `2026-02-28` |
| `else start_of_next_month` | the first day of the month after | `2026-03-01` |
| `else reject` | nothing: the check finds every input in the range that gets there (E202), and the generated code raises an error of kind `reject` | E202 |

There is no default, on purpose: libraries disagree here (the one-month-after of 2023-01-31 is
2023-02-28 in most and 2023-03-03 in Go and in JavaScript's `Date`), and which one a contract or a
law means is for people to say. `else` on an operation that cannot land on a missing day is W201;
`day 31 of month +k else end_of_month`, `day 1 of month +k` and `close day 31 else end_of_month`
have shorter forms (W202).

Some things that look true are not. Adding a month twice is not adding two months at once
(2023-03-31 gives 2023-05-30 and 2023-05-31), adding a day and then a month is not adding a month
and then a day, and adding a month and taking it away does not always come back. When a rewrite
should give the same dates, say so with a claim, and the check finds every day it does not.

### Times

`at 09:00` or `at end of day`, after a date's operations, gives that date's time too: 09:00 of
that day at the calendar's offset, or the end of the day (00:00 of the next). The calendar needs
an offset (E110). A time is written in UTC, ending in `Z` (`2026-05-08T00:00:00Z`), the form
dandori's `timestamp` takes, so it can be handed to a workflow's `wait until`. A place that changes
its clocks has no one offset to give, so the examples on England and Wales give dates only; the
examples on Tokyo's business days, which keep +09:00, pay at 09:00.

### Claims

```cal
claims
  paid_on_a_business_day      : payment is open
  within_60_days_of_receipt   : payment <= received + 60 days
  later_receipt_later_payment : payment is monotonic
```

| Claim | Holds when |
|---|---|
| `X is open` | `X` is a business day |
| `A <= B`, `A < B`, `A = B`, `A > B`, `A >= B` | the comparison holds; either side can add or take away days or business days (`received + 60 days`, `received + 40 business days`) |
| `X is monotonic` | a later input date never gives an earlier `X`, for each value of the integer inputs |

Every claim is checked on every input of the range. A claim that fails on any is E301, a date that
is not monotonic E302, and both list every input it fails on, the first one's computation step by
step, and, for a comparison, the input farthest from holding. A monotonic claim is checked on
every pair of adjacent days, which covers every pair of days, since `<=` chains.

### Examples

```cal
examples
| received   | -> closing | -> payment |
| 2026-04-01 | 2026-04-20 | 2026-05-08 |
| 2026-12-21 | 2027-01-20 | 2027-02-10 |
```

A row gives every input, and every date after `->`. A value the computation does not give is
E303; a missing column, E304. Written values are what can catch a mistake the reference
interpreter and the generated code would share.

## Checking

`koyomi check` goes through five stages, each only when the one before found no error: words and
lines (E001–E006); names and types, and the calendar the file reads (E007–E015, E304); the calendar
and the sources (E101–E111, W101, W102); what follows from what is written (E201, W201, W202); and
every input of the range (E202–E204, E301–E303, E305).

The last stage computes every date of every input combination: the days of the date's range times
the values of each integer input. Over the budget, 10⁸ combinations unless `--budget` says
otherwise, it checks nothing and says so (E305); it never tries some inputs and passes. A computation
that stops on any input (a missing day under `else reject`, a day outside the data range, a date
outside 0001–9999) is reported, and then the claims are not checked, since they could not hold on
every input. When nothing fails:

```console
$ koyomi check examples/close_20th_pay_10th.cal
examples/close_20th_pay_10th.cal: ok — 3 claims hold on all 1,055 days of received (2026-01-01..2028-11-20); 2 examples match
```

What the check shows is that the file, as written, keeps its claims on every input of its range.
It does not show that the file says what the contract, the terms or the law say, nor that the
table of holidays matches the world (only that the copy is the file the source served), nor
anything outside the range.

## Commands

```console
$ koyomi --help
koyomi 0.23.0

A small language for closing days, payment days, business days and month arithmetic, checked on every day of its range.

Usage:
  koyomi check <file.cal>...                   compute on every input of the range, and hold the claims and the examples to it
  koyomi eval <file.cal> <name>=<value>...     compute every date on one input and show each step; for a calendar, whether a day is open or closed
  koyomi gen <file.cal>...                     generate TypeScript, Python, Go, Rust and SQL, each with a runner; nothing is generated from a file that does not pass check
  koyomi vectors <file.cal>                    print the reference interpreter's result for every input of the range as JSON Lines, with the inputs just outside it
  koyomi doc <file.cal>                        print the page for people (accounting, legal, whoever keeps the calendar, the developers reviewing the code): Markdown, or one HTML file
  koyomi api <file.cal>                        print the functions, their inputs, the calendar, the data range and the sources' digests as JSON, for other tools
  koyomi source fetch|pin|outdated <file.cal>  handle the copies of the sources: fetch brings them beside the .cal, pin writes their digests into it, outdated asks whether the originals moved on
  koyomi explain <CODE>                        look a diagnostic code up: when it comes, how to fix it, the smallest reproduction

For one command in detail: `koyomi <cmd> --help` (`koyomi help <cmd>` is the same page).
Every command takes --lang ja|en (default en; the KOYOMI_LANG or RITSU_LANG environment variable works too).
Exit codes: 0 no errors / 1 errors / 2 bad arguments or a file that cannot be read
```

| Command | Flags |
|---|---|
| `koyomi check <file.cal>...` | `--format json`, `--budget <n>`; a directory stands for every `.cal` under it |
| `koyomi eval <file.cal> <name>=<value>...` | `--format json`; a name or its alias; for a calendar, one date (`koyomi eval examples/calendars/england_and_wales.cal 2026-04-03`) |
| `koyomi gen <file.cal>...` | `--target typescript\|python\|go\|rust\|sql` (all five without it), `--out <dir>` (`generated`), `--check` |
| `koyomi vectors <file.cal>` | |
| `koyomi doc <file.cal>` | `--format markdown\|html`, `--months <YYYY-MM>..<YYYY-MM>` |
| `koyomi api <file.cal>` | |
| `koyomi source fetch\|pin\|outdated <file.cal>` | |
| `koyomi explain <CODE>` | `--all`, `--format markdown` |

Every command takes `--lang ja|en`, else the `KOYOMI_LANG` environment variable, else `RITSU_LANG`
(the variable every language of ritsu reads), else English; the system's locale is never read. An unknown flag, a value outside a flag's set, a flag without its
value and a flag given twice are refused with exit code 2, never ignored.

**Exit codes.** 0 when there is no error (warnings may be), 1 when there is one, 2 for bad
arguments, a file that cannot be read, or a bug in koyomi. `doc` prints a page, and exits 0, for a
file whose claims or examples fail; any other error leaves it without a page, with exit 1.
`source outdated` exits 1 when a source has moved on.

**The network.** Only `source fetch` and `source outdated` read it, through `curl`, trying an HTTP
URL three times. Every other command, `check` among them, reads only the files.

`koyomi eval` shows one input's computation, line by line, and what each claim says of it:

```console
$ koyomi eval examples/close_20th_pay_10th.cal received=2026-04-01
received  2026-04-01 Wed
closing   2026-04-20 Mon  close day 20: closes the period 2026-03-21..2026-04-20
payment   2026-05-10 Sun  day 10 of month +1
          2026-05-08 Fri  roll preceding: 2026-05-10 (Sunday) and 2026-05-09 (Saturday) are closed

claim paid_on_a_business_day: holds
claim within_60_days_of_receipt: holds (payment is 37 days after received, and the claim allows at most 60 days after)
claim later_receipt_later_payment: holds (for the day before, 2026-03-31, payment is 2026-05-08)
```

A date with `at` has a line more, its time at the calendar's offset and in UTC; here the same terms
on Tokyo's business days:

```console
$ koyomi eval examples/payment_20th_close_next_10th.cal received=2026-04-01
received  2026-04-01 Wed
closing   2026-04-20 Mon  close day 20: closes the period 2026-03-21..2026-04-20
payment   2026-05-10 Sun  day 10 of month +1
          2026-05-08 Fri  roll preceding: 2026-05-10 (Sunday) and 2026-05-09 (Saturday) are closed
          time 2026-05-08T09:00:00+09:00 (2026-05-08T00:00:00Z in UTC)

claim paid_on_a_business_day: holds
claim within_60_days_of_receipt: holds (payment is 37 days after received, and the claim allows at most 60 days after)
claim later_receipt_later_payment: holds (for the day before, 2026-03-31, payment is 2026-05-08)
```

```console
$ koyomi eval examples/calendars/england_and_wales.cal 2026-04-03
2026-04-03 Fri  closed: Good Friday
```

## Formats

### `check --format json`

One object a file, a line each: `{"file", "ok", "summary", "diagnostics"}`. A diagnostic is
`{"code", "severity", "file", "line", "col", "message", "notes", "inputs", "steps", "fails", "fix"}`:
`inputs` is the input of its example, as `koyomi eval` takes it; `steps` the computation, a step a
line (`{"line", "name", "date", "label", "note", "detail"}`, or `{"line", "text"}` for a sentence
under it); `fails` every input it fails on, as runs of days (`{"from", "to", "params"}`, `params`
holding the integer inputs), up to a million runs; `fix` the line to paste into the `.cal`, when
there is one. The keys stay in English whatever `--lang` says.

### `eval --format json`

`{"inputs", "dates", "times", "steps", "claims", "error"}`: the dates by name, the times as
`{"utc", "local"}`, the claims as `{"name", "holds", "note"}` (`holds` is null when one input
cannot decide it, such as `is monotonic` on the first day of the range), and `error` when the
computation stopped. For a calendar: `{"date", "open", "reasons", "opened_by"}`.

### `vectors`

A line of JSON for every input of the range, in the order the check computes them (the integer
inputs outside, the last declared turning fastest, the date inside), then the inputs just outside
the range, where the generated code must refuse:

```jsonl
{"in":{"invoice_date":"2026-01-01"},"out":{"due":"2026-02-02"}}
{"in":{"invoice_date":"2026-01-02"},"out":{"due":"2026-02-02"}}
{"in":{"invoice_date":"2025-12-31"},"error":"range"}
{"in":{"invoice_date":"2028-11-30"},"error":"range"}
```

`out` holds every date by name, in the order the file declares them, each followed by its time
(`<name>.at`, in UTC) when it has `at`. A calendar's vectors are every day of its data range,
`{"in":{"date":"…"},"out":{"open":true}}`, then the day before and the day after it with
`"error":"data"`; a calendar without a table gives 1900–2100.

### `api`

What calling the generated code takes, without reading it: the functions, their inputs with types
and ranges, the times, the calendar (its files, lines, sources with their pins and its data
range), the laws, and for each target the file, the module and the signatures. The shape follows
rulec's `api`, so a tool that reads one can read the other. `wire.errors` lists the kinds of error
the generated code raises: `range` (an input outside its range), `data` (a day the calendar does
not know), `reject` (a missing day under `else reject`) and `date` (outside 0001–9999).

## Environment

| Variable | What it does |
|---|---|
| `KOYOMI_LANG` | `ja` or `en`, when `--lang` is not given |
| `RITSU_LANG` | the same, when neither `--lang` nor `KOYOMI_LANG` is given (every language of ritsu reads it) |
| `KOYOMI_EGOV` | where e-Gov law API v2 is, for `source fetch` and `source outdated` (the tests point it at a server of their own) |

The tests read more, through ritsu-testkit: `KOYOMI_BLESS=1` (or `RITSU_BLESS=1`) writes the golden
files again; `KOYOMI_TSC`, `KOYOMI_MYPY`, `KOYOMI_PG_BIN`, `KOYOMI_PG_SOCKET_DIR` and `KOYOMI_CHROME`
(or the same names starting `RITSU_`) say where the tools are; `KOYOMI_NET=1` lets them ask the real
Cabinet Office, GOV.UK and e-Gov.
