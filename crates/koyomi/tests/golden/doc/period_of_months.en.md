# period_of_months v1

The end of a period of months counted from a day, written as this example reads it: the day itself is not counted, the period ends on the day before the corresponding day of the last month, or on the last day of that month when it has no such day, and an end that falls on a day closed in England and Wales moves to the day after. It says nothing of how any law is read

- File: `period_of_months.cal` (dates period_of_months v1, sha256:0f1a06d9b71a39f6)
- Calendar: `calendars/england_and_wales.cal` (calendar england_and_wales v1, sha256:00a0d8a87344f4f4)
- Table: bank_holidays = `calendars/data/bank-holidays.json` (sha256:538b3482c28b85ec, a copy of https://www.gov.uk/bank-holidays.json, covers listed years = 2019-01-01..2028-12-31)
- koyomi: 0.24.0

koyomi 0.24.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 4 claims hold on all 4,380 combinations of origin (2026-01-01..2026-12-31) and month_count (1..12).

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### first_day

From origin, in this order:

1. The day after `+ 1 day  # the day itself is not counted`

### last_day

From first_day, in this order:

1. The same day month_count months later; the first of the following month when it has no such day `+ month_count months else start_of_next_month  # the corresponding day of the last month; with none, the 1st of the month after`
2. The day before `- 1 day  # the day before it: the last day of the last month when it had no corresponding day`

### last_day_moved

From last_day, in this order:

1. If it is closed, the day after (once, whatever that day is) `if closed + 1 day  # an end on a closed day moves to the day after`

   The day is closed, and the line acts, on 1,356 of the 4,380 input combinations.

## What was checked

Every date was computed, and every claim checked, on all 4,380 combinations of origin (2026-01-01..2026-12-31) and month_count (1..12).

| Claim | As written | Result |
|---|---|---|
| last_day_is_monotonic | `last_day is monotonic` | holds on all 4,368 pairs of adjacent days |
| moved_last_day_is_monotonic | `last_day_moved is monotonic` | holds on all 4,368 pairs of adjacent days |
| last_day_after_origin | `last_day > origin` | holds on all 4,380 input combinations. The least room is at origin 2026-01-31, month_count 1, where last_day 2026-02-28 is 28 days after origin (the claim allows at least 1 day after) |
| moved_on_or_after_last_day | `last_day_moved >= last_day` | holds on all 4,380 input combinations. The least room is at origin 2026-01-02, month_count 1, where last_day_moved 2026-02-02 is the same day as last_day (the claim allows at least the same day) |

## Days a month does not have

Adding months, taking a day of a month some months away, and closing on a day of the month can land on a day the month does not have, such as 30 February. The .cal says what to do then. For every input of the range, koyomi counted how often that is used, and how often another way would change the result.

- `+ month_count months else start_of_next_month` (last_day)

  It lands on a day the month does not have on 57 of the 4,380 input combinations, and `else start_of_next_month` is used. The first is origin 2026-01-28, month_count 1: 2026-02-29 does not exist, and 2026-03-01 is taken.

  With `else end_of_month` instead, last_day would differ on 57 of the 4,380 input combinations. With `else reject` instead, 57 of the 4,380 input combinations would be refused.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| origin | month_count | first_day | last_day | last_day_moved | Why |
|---|---|---|---|---|---|
| 2026-01-01 Thu | 1 | 2026-01-02 Fri | 2026-02-01 Sun | 2026-02-02 Mon | last_day_moved falls on a Sunday, and `if closed + 1 day` acts; origin is New Year’s Day, a closed day |
| 2026-01-02 Fri | 1 | 2026-01-03 Sat | 2026-02-02 Mon | 2026-02-02 Mon | the least room for the claim moved_on_or_after_last_day (last_day_moved is the same day as last_day; the claim allows at least the same day); origin is the day after New Year’s Day |
| 2026-01-28 Wed | 1 | 2026-01-29 Thu | 2026-02-28 Sat | 2026-03-01 Sun | computing last_day lands on 2026-02-29, which does not exist, and uses `else start_of_next_month` |
| 2026-01-31 Sat | 1 | 2026-02-01 Sun | 2026-02-28 Sat | 2026-03-01 Sun | the least room for the claim last_day_after_origin (last_day is 28 days after origin; the claim allows at least 1 day after); origin is the 31st, the end of its month |
| 2026-02-02 Mon | 1 | 2026-02-03 Tue | 2026-03-02 Mon | 2026-03-02 Mon | the fewest days from origin to last_day_moved (28) |
| 2026-02-28 Sat | 1 | 2026-03-01 Sun | 2026-03-31 Tue | 2026-03-31 Tue | origin is 28 February, the end of the month in a year that is not a leap year |
| 2026-04-02 Thu | 1 | 2026-04-03 Fri | 2026-05-02 Sat | 2026-05-03 Sun | origin is the day before Good Friday |
| 2026-04-30 Thu | 1 | 2026-05-01 Fri | 2026-05-31 Sun | 2026-06-01 Mon | origin is the 30th, the end of its month |
| 2026-01-01 Thu | 12 | 2026-01-02 Fri | 2027-01-01 Fri | 2027-01-02 Sat | the most days from origin to last_day_moved (366) |

## Calendar

The file reads the calendar england_and_wales v1 (`calendars/england_and_wales.cal`).

Its description: Saturdays, Sundays and the bank holidays of England and Wales, as GOV.UK lists them. No offset: England and Wales has daylight saving time, so this calendar gives dates only

### Closed days

These days are closed.

- every Saturday and Sunday `closed weekly sat, sun`
- the days the table bank_holidays lists `closed bank_holidays`

### Sources and the days the calendar knows

- bank_holidays: 83 rows, a copy of https://www.gov.uk/bank-holidays.json, pinned at sha256:538b3482c28b85ec; it lists every closed day from 2019-01-01 to 2028-12-31 (from the first year with a row to the last).

The calendar knows 2019-01-01..2028-12-31. A computation that asks whether a day outside it is a business day stops, in the check and in the generated code alike.

In 2026-01-01..2027-12-31, the days of origin and of the dates computed from it, the longest run of closed days is 2026-04-03..2026-04-06, 4 days.

### Month by month

In the tables, a day in parentheses is closed; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1)◆ | 2◆ | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| 12 | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28◆ | 29 | 30 | (31)◆ |  |

Named closed days: 1 New Year’s Day

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2◆ | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28)◆ |  |

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 | 31 |  |  |  |  |  |

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2◆ | (3) | (4) | (5) |
| (6) | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30◆ |  |  |  |

Named closed days: 3 Good Friday, 6 Easter Monday

**May 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2) | (3) |
| (4) | 5 | 6 | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| (25) | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 4 Early May bank holiday, 25 Spring bank holiday

**June 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | 26 | (27) | (28) |
| 29 | 30 |  |  |  |  |  |

**July 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| 3 | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | 11 | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21 | (22) | (23) |
| 24 | 25 | 26 | 27 | 28 | (29) | (30) |
| (31) |  |  |  |  |  |  |

Named closed days: 31 Summer bank holiday

**September 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| 21 | 22 | 23 | 24 | 25 | (26) | (27) |
| 28 | 29 | 30 |  |  |  |  |

**October 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| 12 | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 | (31) |  |

**November 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 |  |  |  |  |  |  |

**December 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| 21 | 22 | 23 | 24 | (25) | (26) | (27) |
| (28) | 29 | 30 | 31 |  |  |  |

Named closed days: 25 Christmas Day, 28 Boxing Day (Substitute day)

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | (1) | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 1 New Year’s Day

**February 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | 26 | (27) | (28) |

**March 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | (26) | (27) | (28) |
| (29) | 30 | 31 |  |  |  |  |

Named closed days: 26 Good Friday, 29 Easter Monday

**April 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| 12 | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 |  |  |

**May 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| (3) | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | 11 | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21 | (22) | (23) |
| 24 | 25 | 26 | 27 | 28 | (29) | (30) |
| (31) |  |  |  |  |  |  |

Named closed days: 3 Early May bank holiday, 31 Spring bank holiday

**June 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| 21 | 22 | 23 | 24 | 25 | (26) | (27) |
| 28 | 29 | 30 |  |  |  |  |

**July 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| 12 | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 | (31) |  |

**August 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| (30) | 31 |  |  |  |  |  |

Named closed days: 30 Summer bank holiday

**September 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 |  |  |  |

**October 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

**November 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | 26 | (27) | (28) |
| 29 | 30 |  |  |  |  |  |

**December 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| (27) | (28) | 29 | 30 | 31 |  |  |

Named closed days: 27 Christmas Day (Substitute day), 28 Boxing Day (Substitute day)
