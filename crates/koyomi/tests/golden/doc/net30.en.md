# net30 v1

Net 30: due 30 days after the invoice date, moved to the next business day in England and Wales when that day is closed. The claims are this example's own, written as they read

- File: `net30.cal` (dates net30 v1, sha256:979afe6711f909dd)
- Calendar: `calendars/england_and_wales.cal` (calendar england_and_wales v1, sha256:00a0d8a87344f4f4)
- Table: bank_holidays = `calendars/data/bank-holidays.json` (sha256:538b3482c28b85ec, a copy of https://www.gov.uk/bank-holidays.json, covers listed years = 2019-01-01..2028-12-31)
- koyomi: 0.26.0

koyomi 0.26.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 3 claims hold on all 1,064 days of invoice_date (2026-01-01..2028-11-29).

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### due

From invoice_date, in this order:

1. 30 days later `+ 30 days  # "Net 30": due 30 days after the invoice date`
2. If it is closed, the next business day `roll following  # on the next business day when that day is closed`

   The day is closed, and moves, on 327 of the 1,064 days of invoice_date; by 4 days at most.

## What was checked

Every date was computed, and every claim checked, on all 1,064 days of invoice_date (2026-01-01..2028-11-29).

| Claim | As written | Result |
|---|---|---|
| due_on_a_business_day | `due is open` | holds on all 1,064 days of invoice_date |
| at_least_30_days | `due >= invoice_date + 30 days` | holds on all 1,064 days of invoice_date. The least room is at invoice_date 2026-01-03, where due 2026-02-02 is 30 days after invoice_date (the claim allows at least 30 days after) |
| later_invoice_later_due | `due is monotonic` | holds on all 1,063 pairs of adjacent days |

## Days a month does not have

No operation of this file can land on a day its month does not have, such as 30 February.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| invoice_date | due | Why |
|---|---|---|
| 2026-01-01 Thu | 2026-02-02 Mon | due falls on a Saturday and moves to the next business day; invoice_date is New Year’s Day, a closed day |
| 2026-01-02 Fri | 2026-02-02 Mon | due falls on a Sunday and moves to the next business day; invoice_date is the day after New Year’s Day |
| 2026-01-03 Sat | 2026-02-02 Mon | the least room for the claim at_least_30_days (due is 30 days after invoice_date; the claim allows at least 30 days after); the fewest days from invoice_date to due (30) |
| 2026-01-31 Sat | 2026-03-02 Mon | invoice_date is the 31st, the end of its month |
| 2026-02-28 Sat | 2026-03-30 Mon | invoice_date is 28 February, the end of the month in a year that is not a leap year |
| 2026-03-04 Wed | 2026-04-07 Tue | the most days from invoice_date to due (34); due falls on Good Friday and moves to the next business day; due moves the most (4 days) |
| 2026-03-07 Sat | 2026-04-07 Tue | due falls on Easter Monday and moves to the next business day |
| 2026-04-02 Thu | 2026-05-05 Tue | invoice_date is the day before Good Friday |
| 2026-04-04 Sat | 2026-05-05 Tue | due falls on Early May bank holiday and moves to the next business day |
| 2026-04-25 Sat | 2026-05-26 Tue | due falls on Spring bank holiday and moves to the next business day |
| 2026-04-30 Thu | 2026-06-01 Mon | invoice_date is the 30th, the end of its month |
| 2028-02-29 Tue | 2028-03-30 Thu | invoice_date is 29 February |

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

In 2026-01-01..2028-12-29, the days of invoice_date and of the dates computed from it, the longest run of closed days is 2026-04-03..2026-04-06, 4 days.

### Month by month

The inputs and the dates computed from them fall in the 36 months of 2026-01..2028-12. Only the 8 months that hold an input a claim fails on, or an edge case's input or dates, are shown; 28 are left out.

In the tables, a day in parentheses is closed; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1)◆ | 2◆ | (3)◆ | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| 12 | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 | (31)◆ |  |

Named closed days: 1 New Year’s Day

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28)◆ |  |

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4◆ | 5 | 6 | (7)◆ | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 | 31 |  |  |  |  |  |

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2◆ | (3) | (4)◆ | (5) |
| (6) | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25)◆ | (26) |
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

**February 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| 21 | 22 | 23 | 24 | 25 | (26) | (27) |
| 28 | 29◆ |  |  |  |  |  |

**March 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |
