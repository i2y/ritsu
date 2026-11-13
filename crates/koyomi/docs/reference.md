# The koyomi reference

The whole language, the commands, and the formats koyomi reads and writes. [DESIGN.md](../DESIGN.md)
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
a keyword, a reserved word of a target language, or a name that breaks the code of every target
(`<alias>_at`, and `is_open` beside a calendar when the function takes the date alone): E009.

Go and PostgreSQL spell an alias their own way, rustc warns of some aliases as they are, and Python
keeps one module per name. Go names a date's function in PascalCase and the package by the file's
alias without its `_`; PostgreSQL keeps the first 63 bytes of a name; rustc's lint `non_snake_case`
warns of a name with two `_` in a row, and the Rust and Wasm code is written to build with
`-D warnings`; the Python module is named by the file's alias. So `net_30` and `net30` are both
`Net30` in Go, an alias can be in Go a name the generated code declares (`date_` is `Date`), a
file's alias can make a package Go cannot take (`go_to_` is `goto`, `main_` is `main`), two long
aliases that share their first 63 bytes are one function, or one parameter, in PostgreSQL,
`ship__by` is a function rustc warns of (so is `cutoff__at`, the function of the time of a date
`cutoff_`), and a file's alias can be the name of a module of Python's standard library (`abc`),
so that whichever of the two is imported first is what every `import abc` gives. A name can be
one that only some targets declare or read: `koyomi_error` and `parse_date` in Go, `err` in Go
beside an integer input, `str`, `tuple` and `isinstance` in Python, `is_open` beside a calendar a
function also gives integers, and `datetime`, `sys` and `typing` as a file's alias in Python. A
file's alias can be a schema PostgreSQL keeps for itself (`pg_terms`, `information_schema`), and an
input `__proto__` is the prototype of the object a JavaScript host builds for the Wasm module. And
files given together can be one in a target: two aliases that are one Go package (`pay_days`,
`paydays`), one WIT package (`pay__days`, `pay_days`, of one version), or one PostgreSQL schema
(the first 63 bytes). `check` warns about each (W001) and names the targets, and the file passes;
for files given together, the warning is on the later file and names the earlier. `gen` writes
nothing when it is to write a target the warning names (E016), and no name is changed: give the
name the note points to another alias, or, for a target you do not use, generate only the ones you
use with `--target`.

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

`fed by` at the end of an input's line ([below](#fed-by--where-an-inputs-value-comes-from)) is cut
off before the line's words are read, so neither word is reserved: an input can be called `fed`.

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
`第143条第2項第1号`, `第20条の2`, and branch numbers on an article or an item as deep as the law has
them, `第41条の16の2`, `第2条第1項第12号の2`; in kanji numerals too); supplementary provisions and appended
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

### `fed by` — where an input's value comes from

Hand-written code often carries the input from somewhere else. `fed by` at the end of the input's
line says where:

```cal
inputs
  due_on : date  range >=2026-02-02 <=2028-12-29  fed by koyomi "net30.cal" date due
```

The rest of the line is a naming, in the form the whole toolchain names things; the path goes from
the directory of the file it is written in. It stands only on a line of `inputs`; anywhere else, or
when the naming does not read or names something that holds no value, it is E017. `fed` and `by`
are not reserved: they are the form only when the two stand together at the end of a line.

| The value comes from | Named as |
|---|---|
| an output of a rule | `rulec "<file>" output <name>` |
| an account of a book | `chobo "<file>" account <name>` |
| a date of a dates file | `koyomi "<file>" date <name>` |
| a field of a `.proto` message | `proto "<file>" message <name> field <name>` |
| a property of an OpenAPI schema | `openapi "<file>" schema <name> property <name>` |
| what an operation of OpenAPI writes in place (a request body, a parameter) | `openapi "<file>" pointer "<pointer>"` |
| a property of a JSON Schema | `jsonschema "<file>" def <name> property <name>`, or `jsonschema "<file>" pointer <pointer>` |
| an output of a workflow, or a field of its record | `dandori "<file>" output <name>`, `dandori "<file>" record <name> field <name>` |
| a field of a kaname record | `kaname "<file>" record <name> field <name>` |
| a place that took the value first | `rulec "<file>" input <name>`, `koyomi "<file>" input <name>`, `dandori "<file>" input <name>`, `sekisho "<file>" resource <name> attribute <name>` (or `principal`), `sekisho "<file>" action <name> input <name>` |

A contract often leaves a bound to the code, which checks it by hand and then gives the value to a gate and to a workflow. The second can name the first: what comes from a place is what its own line writes. Two places that name each other say they take the same values, so whichever of the two moves, one of them stops.

`ritsu check` asks the language on each side, and comes to one of three answers:

- **It holds.** The place takes everything the source can come to. A place wider than its source is fine.
- **E207.** The source can come to a value the place refuses. The message gives the value, and the input that gives it when the source can say.
- **W207.** It cannot be decided, and the message says why: rulec counts an end of a rule's output that none of the rule's vectors reaches, or a contract carries a date as a string and lists no days.

E208 says the source is not there: its file, or the name in it.

Numbers are compared in one unit and one step, and another unit does not hold. The values of two enums are the same when they share a name, as a file writes it or as its public name, since the code that carries one turns one spelling into the other; a date by its days, a bool or a string by its type alone. An end the place does not write is not compared. A contract with no upper or no lower bound lets through a value beyond what the place takes, as with a rule's `from` (E122), and does not hold. So does an account of a book with no `at most`, which goes as far as chobo's limit (2⁶³ − 1).

A contract carries a date as a string, so one that lists its days (`enum`, `const`) is compared by
those days, and one that does not is W207.

**What is not checked.** Nothing checks that the hand-written code really carries that value. The
page of `koyomi doc` says so: Comes from `…` (`ritsu check` holds the range of its values; the hand-written code that carries the value is not checked). Generated code does not change, and `koyomi api` gives the
naming as the input's `fed_by`.

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

### The periods of the Civil Code

Unless a law or a contract says otherwise (Article 138), a period in Japan is counted by Articles 140
to 143 of the Civil Code. Each article is a line or two of the operations above, and no operation is
named after one: `examples/civil_code_period_end.cal` writes them with the articles cited.

```cal
date first_day = origin                                       @civil_code 第140条
  + 1 day                                        # the first day is not counted
date last_day = first_day                                     @civil_code 第141条, 第143条
  + month_count months else start_of_next_month  # the corresponding day of the last month; with none, the 1st of the month after
  - 1 day                                        # the day before it: the last day of the last month when it had no corresponding day
```

| Article | What it says | The lines |
|---|---|---|
| 140 | The day the period starts on is not counted, unless the period starts at midnight. | `+ 1 day` from the day of the event: `first_day`, the day the counting starts from. |
| 143, paragraph 2 | A period of weeks, months or years ends on the day before the day of the last week, month or year that corresponds to the day the counting starts from. | `+ n months else start_of_next_month` from the first day, then `- 1 day`. |
| 143, the proviso | When the last month has no such day, the period ends on that month's last day. | The same two lines: the 1st of the month after, and the day before it. |
| 141 | The period ends when its last day ends. | Nothing more: a date is a whole day. |
| 142 | When the last day is a Sunday, a national holiday or another day off on which it is customary not to trade, the period ends on the day after. | `if closed + 1 day` or `roll following`, over a calendar of the days off: two readings, which `examples/civil_code_two_readings.cal` sets side by side. |
| Act on Calculation of Age | An age is counted from the day of birth, the first day counted, and by Article 143. | No `+ 1 day`: `+ n years else start_of_next_month` from the day of birth, then `- 1 day`. |

The age, in `tests/fixtures/age.cal`: born on 29 February 2008, a person's 18 years end on 28 February
2026; born on 1 April 2008, on 31 March 2026, a day before those of one born on 2 April 2008.

```cal
date first_day = birth                          # counted from the day of birth: the first day counts
date last_day = first_day
  + year_count years else start_of_next_month   # the corresponding day; with none, the 1st of the month after
  - 1 day                                       # the day before it: the age goes up when this day ends
```

Two other ways of writing Article 143 look as right, and are not. `else end_of_month` reads like the
proviso, but the end of the month and then the day before it end the period a day early whenever
the last month has no corresponding day:

```text
error[E301]: tests/mutants/E301_a_day_early.cal:21:3: The claim rounding_down_first fails for 57 of the 4,380 combinations of origin and month_count
    21 |   rounding_down_first     : last_day = rounded_down
  = It fails on month_count 1: 2026-01-28..2026-01-30 (3 days), 2026-03-30, 2026-05-30, 2026-08-30, 2026-10-30; month_count 2: 2026-07-30, and 33 more runs; --format json lists them all
  = By input: month_count 1 → 7, month_count 2 → 4, month_count 3 → 5, month_count 4 → 6, month_count 5 → 3, month_count 6 → 7, month_count 7 → 3, month_count 8 → 6, month_count 9 → 5, month_count 10 → 4, month_count 11 → 7
  = The farthest is origin 2026-01-28, month_count 1, where last_day 2026-02-28 is 1 day after rounded_down
  the first input it fails on:
      origin        2026-01-28 Wed
                    month_count = 1
      first_day     2026-01-29 Thu  + 1 day
      last_day      2026-03-01 Sun  + month_count months else start_of_next_month (month_count = 1): 2026-02-29 does not exist; the first of the next month
                    2026-02-28 Sat  - 1 day
      rounded_down  2026-02-28 Sat  + month_count months else end_of_month (month_count = 1): 2026-02-29 does not exist; the end of that month
                    2026-02-27 Fri  - 1 day
                    last_day is 1 day after rounded_down
```

And counting the months from the day of the event instead of from `first_day` ends almost every
period early, by four days at most (from 28 February, one month ends on 27 March instead of 31 March):

```text
error[E301]: tests/mutants/E301_a_day_early.cal:22:3: The claim counting_from_the_event fails for 4,323 of the 4,380 combinations of origin and month_count
    22 |   counting_from_the_event : last_day = from_the_event
  = It fails on month_count 1: 2026-01-01..2026-01-28 (28 days), 2026-02-01..2026-03-30 (58 days), 2026-04-01..2026-05-30 (60 days), 2026-06-01..2026-08-30 (91 days), 2026-09-01..2026-10-30 (60 days), 2026-11-01..2026-12-31 (61 days), and 40 more runs; --format json lists them all
  = By input: month_count 1 → 358, month_count 2 → 361, month_count 3 → 360, month_count 4 → 359, month_count 5 → 362, month_count 6 → 358, month_count 7 → 362, month_count 8 → 359, month_count 9 → 360, month_count 10 → 361, month_count 11 → 358, month_count 12 → 365
  = The farthest is origin 2026-02-28, month_count 1, where last_day 2026-03-31 is 4 days after from_the_event
  the first input it fails on:
      origin          2026-01-01 Thu
                      month_count = 1
      first_day       2026-01-02 Fri  + 1 day
      last_day        2026-02-02 Mon  + month_count months else start_of_next_month (month_count = 1)
                      2026-02-01 Sun  - 1 day
      from_the_event  2026-02-01 Sun  + month_count months else start_of_next_month (month_count = 1)
                      2026-01-31 Sat  - 1 day
                      last_day is 1 day after from_the_event
```

Neither is an error of its own, since a contract may mean either. Where a law decides, write the
lines above, and a claim that another writing agrees finds every day it does not.

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
lines (E001–E006); names and types, and the calendar the file reads (E007–E015, E304, W001); the
calendar and the sources (E101–E111, W101, W102); what follows from what is written (E201, W201,
W202); and every input of the range (E202–E204, E301–E303, E305).

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
koyomi 0.55.1

A small language for closing days, payment days, business days and month arithmetic, checked on every day of its range.

Usage:
  koyomi check <file.cal>...                   compute on every input of the range, and hold the claims and the examples to it
  koyomi eval <file.cal> <name>=<value>...     compute every date on one input and show each step; for a calendar, whether a day is open or closed
  koyomi diff <old> <new>                      compare the answers of two versions: a calendar on every day both know, a dates file on every input both take
  koyomi gen <file.cal>...                     generate the code of each target given (TypeScript, Python, Go, Rust, SQL, Wasm), and the runners if asked; nothing is generated from a file that does not pass check
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
| `koyomi diff <old> <new>` | `--format markdown\|json`, `--budget <n>`; two calendars or two dates files, each a path, `<path>@<git revision>`, `@<name>/<path>` or `@<name>@<version>/<path>` |
| `koyomi gen <file.cal>...` | `--target typescript\|python\|go\|rust\|sql\|wasm`, once or more (without it, exit 2 and the command that writes what 0.40.0 wrote), `--extra runner`, `--out <dir>` (`generated`), `--check`; a target a W001 names stops the run (E016) |
| `koyomi vectors <file.cal>` | |
| `koyomi doc <file.cal>` | `--format markdown\|html`, `--months <YYYY-MM>..<YYYY-MM>` |
| `koyomi api <file.cal>` | |
| `koyomi source fetch\|pin\|outdated <file.cal>` | |
| `koyomi explain <CODE>` | `--all`, `--format markdown` |

Every command takes `--lang ja|en`, else the `KOYOMI_LANG` environment variable, else `RITSU_LANG`
(the variable every language of ritsu reads), else English; the system's locale is never read. An unknown flag, a value outside a flag's set, a flag without its
value and a flag given twice (for `gen --target`, which may be given more than once, a target given
twice) are refused with exit code 2, never ignored.

**Exit codes.** 0 when there is no error (warnings may be), 1 when there is one, 2 for bad
arguments (`gen` with no `--target` among them), a file that cannot be read, or a bug in koyomi. `doc` prints a page, and exits 0, for a
file whose claims or examples fail; any other error leaves it without a page, with exit 1.
`source outdated` exits 1 when a source has moved on. `diff` exits 1 when the inputs or the answers
of the two versions differ, and 2 when they cannot be compared.

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
and ranges (and `fed_by`, the naming of an input's `fed by`), the times, the calendar (its files, lines, sources with their pins and its data
range), the laws, and for each target the file, the module and the signatures. The shape follows
rulec's `api`, so a tool that reads one can read the other. `wire.errors` lists the kinds of error
the generated code raises: `range` (an input outside its range), `data` (a day the calendar does
not know), `reject` (a missing day under `else reject`) and `date` (outside 0001–9999).

The Wasm entry has the shape of rulec's: `build` (the line that builds the module, run in the
directory of `file`), `wit`, `package` and `world` (as the `.wit` writes them), `call`,
`call_signature`, `post_return`, `realloc` and `memory` (the exports a host needs), `runner` (the
file `gen --extra runner` writes), and
`component` (the two lines that make a component of the module and the `.wit`); with `file`,
`module` (the `.wasm` a host instantiates) and `date_type`. In the place of the signatures,
`functions` gives each date the key of `out` it comes under (`key`, and `at_key` for its time) with
its inputs, and `is_open` the question a calendar takes, `{"in": "date", "out": "open"}`. `answers`
gives the two shapes `call` answers with: the record, `{"in", "out"}`, the line `koyomi vectors`
writes for that input, and the refusal, `{"refused", "error"}`, `refused` being `input` (the object
does not carry the inputs) or one of `wire.errors`. It has no `errors`: the module raises nothing.

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
