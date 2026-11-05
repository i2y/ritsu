# ritsu-fmla

The Family and Medical Leave Act of the United States as its regulations, 29 CFR Part 825, state it, in
five of the languages of ritsu. Each file answers one part of the same leave, and `ritsu check` holds them
together: the flow calls the rules, the dates and the book with the types they declare.

| File | Language | What it holds | Sections |
|---|---|---|---|
| `fmla_eligibility.rule` | rulec | Whether an employee is eligible, and the first reason why not | 825.110(a) |
| `fmla_entitlement.rule` | rulec | How many workweeks a reason gives, and how many minutes they are of the employee's own workweek | 825.200, 825.205(b) |
| `business_days.cal`, `fmla_deadlines.cal` | koyomi | The last day for each notice and for a certification, on the federal holidays of the module `holidays` | 825.300(b), (d), 825.305(b) |
| `fmla_leave.book` | chobo | The leave an employee has left in a 12-month period, never below zero, with a designation held until the leave is taken | 825.200, 825.205 |
| `fmla_request.flow` | dandori | From a request to the end of the leave: the notices an employer owes, the certification, the designation | 825.300, 825.305 |
| `fmla_records.gate` | sekisho | Who may learn what from the medical records, and who may contact the health care provider | 825.500(g), 825.307(a) |

The files transcribe the sections as of 2026-01-01, to be read against them. They are not legal advice.

## What is a reading made here

A comment in each file says where a section leaves something open and how it was decided:

- A business day is Monday to Friday less the federal holidays. Part 825 does not define it, and an
  employer's own may differ.
- The deadlines are days and not moments. The sections name no hour, and an employer may be in any of the
  country's time zones. They are the days when no "extenuating circumstances" apply.
- The flow asks for a certification on the day the notice is given, the first of the days 825.305(b) allows.
- 825.500(g) does not say who keeps the file. In the gate, the leave administrator and the human resources
  professional read it.

## What it leaves out

- Whether an employer is covered (825.104) and whether a condition is a serious health condition (825.113).
- Airline flight crew employees (825.801 to 825.803), employees who work outside the United States
  (825.105(b)), and the special rules for employees of schools (825.600 to 825.604).
- Spouses who work for the same employer and share a limit (825.120(a)(3), 825.127(f)).
- The combined total of 825.200(g). The book holds one limit for a period, and an employer names the single
  12-month period of 825.200(f), for a covered servicemember's care, as a period of its own.
- The rolling 12-month period of 825.200(b)(4), which moves every day and needs another shape of book.
- The substitution of paid leave (825.207), and the leave laws of the states.
- What an employee may see of the employee's own file, which these paragraphs do not say.

## Use

Add the module, add the line that checks the signatures of this module and of `holidays` to your
`ritsu.mod`, and fetch them:

```console
$ ritsu mod add fmla ghcr.io/i2y/ritsu-fmla@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-fmla" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
signature "ghcr.io/i2y/ritsu-holidays" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read its files from your own:

```
use rule eligibility from "@fmla/fmla_eligibility.rule"
use dates deadlines from "@fmla/fmla_deadlines.cal"
use book leave from "@fmla/fmla_leave.book"
```

The flow's tasks that go through a queue run on Temporal, and `ritsu gen --target temporal-typescript`,
`temporal-python` or `temporal-go` writes it for one of them. The gate's policies are written as Cedar by
`ritsu gen --target cedar`.

## Sources

`sources/law/` holds the sections as the eCFR served them. The rules pin the three they read by their
SHA-256, and `rulec source outdated <file>` asks the eCFR whether a later amendment touched them. A file of
koyomi, chobo, dandori or sekisho has no line that pins a section of the eCFR: its comments name the
sections, and the copies beside it are what they were read from. `THIRD_PARTY_NOTICES.md` lists them.

The files are under the MIT License or the Apache License, version 2.0, at your option, as ritsu is.

## Versions

A version that follows a change in the law raises the minor version. A version that renames a file of the
languages or removes one, or changes what a file takes or gives, raises the major version.
