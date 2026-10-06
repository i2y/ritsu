---
name: koyomi
description: Write, check and compile koyomi files (`.cal`), the rules of due dates — closing days, payment days, business days and month arithmetic, such as closing on the 20th and paying on the 10th of the next month or on the business day before when that is a holiday — with calendars of closed days read from published tables of holidays, claims checked on every day of a declared range, and code generated for TypeScript, Python, Go, Rust and PostgreSQL. Use when payment terms, a deadline or a calendar of business days has to be written or changed as a `.cal`; when a koyomi diagnostic (E001-E305, W101-W202, W901) has to be fixed; when such a rule has to be shown to the people who read it; or when its generated code has to be called.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run koyomi as `ritsu koyomi <command>`, or as `koyomi <command>` through a link to ritsu named for it. `ritsu koyomi source fetch` and `source outdated` also need `curl`.
license: MIT OR Apache-2.0
---

## When this applies

The job is a **rule of dates**: a date computed from another by closing, adding days, business
days or months, taking a day of a month, and moving off closed days. Payment terms ("closes on the
20th, pays on the 10th of the next month, on the business day before when that day is closed"),
Net 30, a deadline counted in business days, the end of a period under a statute. koyomi writes
it once as a `.cal`, checks it on every input of a range, shows it to the people who read it,
and generates the same function for five languages.

It does not apply to decisions made from tables of conditions (a fee schedule, an eligibility
test): those are rulec rules. Nor to times of day in a zone with daylight saving time: koyomi
gives dates, and times only at a fixed UTC offset.

The files bundled with this skill are listed in §7. Read them when you need them, not all up
front.

---

# Working with koyomi

Your part is to write the `.cal`, get it past `ritsu koyomi check`, show it to a person, and generate the
code. Two things stay with people:

- **what the terms and the calendar are**: which days are closed, which table of holidays, what a
  contract means by a day the month does not have, how a law is read. §3 says when to ask.
- **the code around the generated functions**: where they are called, what an input outside the
  range means for the caller.

Run every command here as `ritsu koyomi <command>` (through a link to ritsu named koyomi,
`koyomi <command>` is the same). Everything is reachable from the command line:
`ritsu koyomi --help` lists the commands, `ritsu koyomi explain <CODE>` explains a diagnostic, and
`ritsu koyomi check --format json` gives the diagnostics as data (§5). There is no step where you
have to read koyomi's source.

## 1. The loop

1. **Find the rule.** What is the input date (the invoice, the receipt, the start of a period),
   what dates come from it, which days are closed, and what must hold (a business day, no later
   than so many days, never earlier for a later input). Settle the range of the input with a
   person: every input in it will be computed.
2. **Write the calendar**, or use one (`use calendar "calendars/<file>.cal"`): closed days of the
   week, days every year, a table of holidays pinned by its digest, given days, `open` days.
   `ritsu koyomi source fetch` takes a table from its `url`, and `ritsu koyomi source pin` writes its digest.
3. **Write the dates file** (§2): `inputs`, the dates, one operation a line, `claims`, `examples`.
4. **Check it:** `ritsu koyomi check <file.cal>`. A diagnostic comes with the input that gets there and
   its computation, step by step; fix it (§4; `ritsu koyomi explain <CODE>` explains one code) and
   check again, until it prints `<file.cal>: ok — …`. A warning does not stop anything, but it says
   something true.
5. **Look at one input:** `ritsu koyomi eval <file.cal> <input>=<date>` prints every date of that input,
   a step a line, and what each claim says of it.
6. **Show it to a person:** `ritsu koyomi doc <file.cal> > <file>.md` (or `--format html`) writes the page
   for people, for those who read the terms: every operation in words beside its line, the claims and their
   least room, the edge cases, the months with the holidays named and the failing input days
   marked. A file whose claims fail still gets its page; the failing days are the point of it.
7. **Generate the code:** `ritsu koyomi gen <file.cal> --out generated` writes TypeScript, Python, Go,
   Rust and SQL, each with a runner. `ritsu koyomi gen --check` in CI says when the code is stale.
   `ritsu koyomi api <file.cal>` gives the functions, their inputs and ranges, and the signatures in each
   language, without reading the code. [generated-code.md](generated-code.md) says how to call it.
8. **In a project with rules or workflows**, `ritsu check` checks the `.cal` files with the rest,
   and across them: a rule input can take a date's days as its range (`range from koyomi`), and
   where a workflow calls a date or gives its day to a rule, the days are held to the ranges
   (ritsu's E202 and E205; the ritsu skill has them).

## 2. The language on one page

A whole dates file, from the examples:

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

And the calendar it reads:

```cal
calendar england_and_wales v1
description "Saturdays, Sundays and the bank holidays of England and Wales, as GOV.UK lists them. No offset: England and Wales has daylight saving time, so this calendar gives dates only"

source bank_holidays = file "data/bank-holidays.json" url "https://www.gov.uk/bank-holidays.json" sha256:538b3482c28b85ec
  format govuk "england-and-wales"
  covers listed years

closed weekly sat, sun
closed bank_holidays
```

A calendar for a place that changes its clocks has no offset, so dates on it have no time. The
examples written for Japan read Tokyo's business days, which keep +09:00 and so let a date say
`at 09:00`; each of them is there in English and in Japanese (`<name>.cal` and `<name>.ja.cal`,
the same terms with the same results), since a name of a file, an input or a date can be written in
any script, with an ASCII alias for the generated code (`受領日(received)`).

`else` says what happens on a day the month does not have: `else end_of_month` (the last day of
that month), `else start_of_next_month` (the first of the next), or `else reject` (the check
makes sure no input of the range gets there, E202). There is no default (E201).

### Claims

```text
X is open                  X is a business day
A <= B + 60 days           =, <, <=, >, >=; either side may add or take days or business days
X is monotonic             a later input never gives an earlier X
```

[reference.md](reference.md) has the whole language: calendars, tables of holidays, laws and
their citations (`@民法 第143条`), the meaning of every operation, the commands and the formats.

## 3. What stays with a person

Ask instead of guessing:

- **Which days are closed.** Which calendar applies (a company's, a bank's, the days a statute
  names), and which table of holidays, from where. A table knows a span of years
  (`covers`); past it, nothing can be computed (E203). Say so, with the date the check gives,
  rather than widening the table.
- **What a day the month does not have means.** The 31st in a month of 30, the 29th of February:
  the end of the month, the first of the next, or never. It is in the contract, or it is not, and
  then a person decides. A law can decide it too (the Japanese Civil Code, Article 143).
- **How a law is read.** koyomi writes what a person reads the law as, and cites the article; it
  does not read the law. When an article can be read two ways (the "next day" of Article 142: the
  day after, or the next business day), write both as dates, claim they are equal, and show the
  person the days they part on.
- **The range and the numbers**: the inputs' ranges, the days a claim allows, the time of day.
- **What a failing claim means.** A claim that fails on 648 of 669 days may mean the terms are
  wrong, or the claim is; that is a business decision, not a fix.

Ask with the input the check gives, in the reader's terms: "Received on 2026-05-01, the payment
falls on 2026-07-31, 91 days later. The claim allows 60. Are the terms or the claim wrong?" The
page `ritsu koyomi doc` writes shows the same days on a calendar, when a person would rather see them.

## 4. From a diagnostic to a fix

A diagnostic names the code, the place, what is wrong, how to fix it, and the input that gets
there:

```text
error[E201]: tests/mutants/E201_no_way_for_a_missing_day.cal:7:3: `+ 1 month` can land on a day the month does not have, and the line does not say what to do then
     7 |   + 1 month
  = For received 2026-01-29 it would be 2026-02-29
  = To fix it, write one of: `+ 1 month else end_of_month` (giving 2026-02-28), `+ 1 month else start_of_next_month` (giving 2026-03-01), `+ 1 month else reject` (the check makes sure it never happens in the range)
  the input that gets there:
      received     2026-01-29 Thu
      month_later                  + 1 month: 2026-02-29 does not exist
```

| Code | What it finds | The usual fix |
|---|---|---|
| E001 to E006 | words, lines, the order of sections, indentation, dates that do not exist | what the message says |
| E007 to E014 | names twice or undeclared, aliases, types, the one date input, ranges, dates that start from each other | what the message says |
| E015 | a `use calendar` that cannot be used | the path is from the `.cal`'s directory |
| E101 to E106 | a table's copy missing, unpinned, changed, unreadable, or outside its `covers` | `ritsu koyomi source fetch`, read what changed, `ritsu koyomi source pin` |
| E107 | a time zone's name as the offset | write `+09:00`; for a zone with daylight saving time, give dates only |
| E108 to E111 | no business day; an operation needs a calendar; `at` needs an offset; a citation | what the message says |
| E201, W201 | a missing day's handling not said, or said where it cannot happen | ask (§3), then write `else …`; or delete it |
| E202 | `else reject` that happens in the range | another `else`, or a narrower range |
| E203 | a day past the table asked about | the range the note gives, or a newer table |
| E204 | a date outside 0001-01-01..9999-12-31 | narrow the range |
| E301, E302 | a claim that fails; a date that is not monotonic | ask (§3); do not loosen the claim on your own |
| E303, E304 | an example that does not compute as written; a column missing | ask whether the example or the rule is wrong |
| E305 | more combinations than the budget | narrow a range, split the file, or `--budget` |
| W101, W102 | an `open` day that is open anyway; a pinned article nothing cites | delete the line |
| W202 | a longer way of writing a shorter line | the line the warning gives |
| W901 | a key written in the file, in a string or a comment (the kind, its prefix and its length; never the key) | take it out and read it from where the code runs; revoke it first if it is real; `# ritsu: test secret` on the line of a value for tests |

[codes.md](codes.md) has every code, with a reproduction of each.

## 5. For a machine

- `ritsu koyomi check --format json <file.cal>…` prints one JSON object a file:
  `{"file", "ok", "summary", "diagnostics": [{"code", "severity", "file", "line", "col",
  "message", "notes", "inputs", "steps", "fails", "fix"}]}`. `inputs` is the example's input as
  `ritsu koyomi eval` takes it, `fails` every input it fails on (as runs of days), `fix` the line to
  paste, when there is one.
- `ritsu koyomi eval --format json` and `ritsu koyomi api` give the computation and the functions as JSON;
  `ritsu koyomi vectors` gives every input's result as JSON Lines.
- The exit code is 0 with no error (warnings may be), 1 with one, and 2 for bad arguments or a file
  that cannot be read. An unknown flag is refused with 2, never ignored.
- `--lang ja`, or `KOYOMI_LANG=ja`, gives the messages in Japanese. `check` never reads the
  network; `source fetch` and `source outdated` do.

## 6. The generated code

| Target | What `gen` writes | A date is |
|---|---|---|
| `typescript` | `typescript/<alias>.ts`, which Node runs by stripping the types | a `"YYYY-MM-DD"` string |
| `python` | `python/<alias>.py`, `mypy --strict` clean | a `datetime.date` |
| `go` | `go/<package>/<package>.go`, one package, no go.mod | the package's `Date` |
| `rust` | `rust/<alias>.rs`, a module for the 2021 and the 2024 editions | the module's `Date` |
| `sql` | `sql/<alias>.sql`, a schema of PL/pgSQL functions, PostgreSQL 14 and later | a `date` |

Each date is a function named by its alias, taking only the inputs it uses; a date with `at` has
`<alias>_at`, giving its time in RFC 3339 UTC; a file with a calendar has `is_open`. An error has a
kind: `range` (an input outside its range), `data` (a day the calendar does not know), `reject`,
`date`. The runner beside each file reads `ritsu koyomi vectors` and prints what the functions give, so
the code can be held to the reference interpreter wherever it is copied.

## 7. The files bundled with this skill

| File | What is in it |
|---|---|
| [reference.md](reference.md) | the whole language, the commands, the exit codes and the formats |
| [codes.md](codes.md) | every diagnostic: when it appears, how to fix it, a reproduction |
| [generated-code.md](generated-code.md) | what each target writes, its functions and errors, and the runners |

They are copies of the pages under `crates/koyomi/docs/` in the ritsu repository, https://github.com/i2y/ritsu.
