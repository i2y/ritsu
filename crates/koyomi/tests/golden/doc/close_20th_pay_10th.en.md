# close_20th_pay_10th v1

Closes on the 20th; pays on the 10th of the next month, or on the business day before when that day is closed in England and Wales. The claim within_60_days_of_receipt is this example's own, written as it reads; it does not say how any law is read

- File: `close_20th_pay_10th.cal` (dates close_20th_pay_10th v1, sha256:38bfa1df75d69ac1)
- Calendar: `calendars/england_and_wales.cal` (calendar england_and_wales v1, sha256:00a0d8a87344f4f4)
- Table: bank_holidays = `calendars/data/bank-holidays.json` (sha256:538b3482c28b85ec, a copy of https://www.gov.uk/bank-holidays.json, covers listed years = 2019-01-01..2028-12-31)
- koyomi: 0.23.0

koyomi 0.23.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 3 claims hold on all 1,055 days of received (2026-01-01..2028-11-20); 2 examples match.

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### closing

From received, in this order:

1. The closing day of its period, closing on the 20th (the 21st to the 20th) `close day 20  # "closes on the 20th"`

### payment

From closing, in this order:

1. The 10th of the next month `day 10 of month +1  # "pays on the 10th of the next month"`
2. If it is closed, the business day before `roll preceding  # "on the business day before when that day is closed"`

   The day is closed, and moves, on 274 of the 1,055 days of received; by 2 days at most.

## What was checked

Every date was computed, and every claim checked, on all 1,055 days of received (2026-01-01..2028-11-20).

| Claim | As written | Result |
|---|---|---|
| paid_on_a_business_day | `payment is open` | holds on all 1,055 days of received |
| within_60_days_of_receipt | `payment <= received + 60 days` | holds on all 1,055 days of received. The least room is at received 2026-07-21, where payment 2026-09-10 is 51 days after received (the claim allows at most 60 days after) |
| later_receipt_later_payment | `payment is monotonic` | holds on all 1,054 pairs of adjacent days |

## Days a month does not have

No operation of this file can land on a day its month does not have, such as 30 February.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| received | closing | payment | Why |
|---|---|---|---|
| 2026-01-01 Thu | 2026-01-20 Tue | 2026-02-10 Tue | received is New Year’s Day, a closed day |
| 2026-01-02 Fri | 2026-01-20 Tue | 2026-02-10 Tue | received is the day after New Year’s Day |
| 2026-01-31 Sat | 2026-02-20 Fri | 2026-03-10 Tue | received is the 31st, the end of its month |
| 2026-02-20 Fri | 2026-02-20 Fri | 2026-03-10 Tue | the fewest days from received to payment (18) |
| 2026-02-28 Sat | 2026-03-20 Fri | 2026-04-10 Fri | received is 28 February, the end of the month in a year that is not a leap year |
| 2026-03-21 Sat | 2026-04-20 Mon | 2026-05-08 Fri | payment falls on a Sunday and moves to the business day before; payment moves the most (2 days) |
| 2026-04-02 Thu | 2026-04-20 Mon | 2026-05-08 Fri | received is the day before Good Friday |
| 2026-04-30 Thu | 2026-05-20 Wed | 2026-06-10 Wed | received is the 30th, the end of its month |
| 2026-07-21 Tue | 2026-08-20 Thu | 2026-09-10 Thu | the least room for the claim within_60_days_of_receipt (payment is 51 days after received; the claim allows at most 60 days after); the most days from received to payment (51) |
| 2026-08-21 Fri | 2026-09-20 Sun | 2026-10-09 Fri | payment falls on a Saturday and moves to the business day before |
| 2028-02-29 Tue | 2028-03-20 Mon | 2028-04-10 Mon | received is 29 February |

## Examples

The rows the .cal writes under examples, and what the computation gives.

| received | → closing | → payment | Result |
|---|---|---|---|
| 2026-04-01 Wed | 2026-04-20 Mon | 2026-05-08 Fri | matches |
| 2026-12-21 Mon | 2027-01-20 Wed | 2027-02-10 Wed | matches |

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

In 2026-01-01..2028-12-08, the days of received and of the dates computed from it, the longest run of closed days is 2026-04-03..2026-04-06, 4 days.

### Month by month

The inputs and the dates computed from them fall in the 36 months of 2026-01..2028-12. Only the 13 months that hold an input a claim fails on, or an edge case's input or dates, are shown; 23 are left out.

In the tables, a day in parentheses is closed; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1)◆ | 2◆ | (3) | (4) |
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
| 16 | 17 | 18 | 19 | 20◆ | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28)◆ |  |

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21)◆ | (22) |
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
| 20 | 21◆ | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| 3 | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | 11 | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21◆ | (22) | (23) |
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

**April 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| 3 | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | 11 | 12 | 13 | (14) | (15) | (16) |
| (17) | 18 | 19 | 20 | 21 | (22) | (23) |
| 24 | 25 | 26 | 27 | 28 | (29) | (30) |

Named closed days: 14 Good Friday, 17 Easter Monday
