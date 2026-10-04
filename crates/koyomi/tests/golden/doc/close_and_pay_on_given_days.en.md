# close_and_pay_on_given_days v1

Takes the closing day, the month of payment and the day of payment as integers, as when the closing day comes from a rule. A payment day that is closed in England and Wales moves to the business day before. The claims are this example's own, written as they read

- File: `close_and_pay_on_given_days.cal` (dates close_and_pay_on_given_days v1, sha256:f5323722663873b0)
- Calendar: `calendars/england_and_wales.cal` (calendar england_and_wales v1, sha256:00a0d8a87344f4f4)
- Table: bank_holidays = `calendars/data/bank-holidays.json` (sha256:538b3482c28b85ec, a copy of https://www.gov.uk/bank-holidays.json, covers listed years = 2019-01-01..2028-12-31)
- koyomi: 0.1.0

koyomi 0.1.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 3 claims hold on all 872,960 combinations of received (2027-01-01..2028-10-01), closing_day (1..31), payment_month (1..2) and payment_day (10..31).

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### closing

From received, in this order:

1. The closing day of its period, closing on the day closing_day; the end of that month when it has no such day `close day closing_day else end_of_month  # closes on closing_day; in a month without that day (the 31st and the like), at the end of the month`

### payment

From closing, in this order:

1. The day payment_day of the month payment_month months later; the end of that month when it has no such day `day payment_day of month +payment_month else end_of_month  # payment_day of the month payment_month later; at the end of the month when it has no such day`
2. If it is closed, the business day before `roll preceding  # on the business day before when that day is closed`

   The day is closed, and moves, on 266,412 of the 872,960 input combinations; by 4 days at most.

## What was checked

Every date was computed, and every claim checked, on all 872,960 combinations of received (2027-01-01..2028-10-01), closing_day (1..31), payment_month (1..2) and payment_day (10..31).

| Claim | As written | Result |
|---|---|---|
| paid_on_a_business_day | `payment is open` | holds on all 872,960 input combinations |
| paid_after_closing | `payment > closing` | holds on all 872,960 input combinations. The least room is at received 2027-08-31, closing_day 30, payment_month 1, payment_day 10, where payment 2027-10-08 is 8 days after closing (the claim allows at least 1 day after) |
| later_receipt_later_payment | `payment is monotonic` | holds on all 871,596 pairs of adjacent days |

## Days a month does not have

Adding months, taking a day of a month some months away, and closing on a day of the month can land on a day the month does not have, such as 30 February. The .cal says what to do then. For every input of the range, koyomi counted how often that is used, and how often another way would change the result.

- `close day closing_day else end_of_month` (closing)

  It lands on a day the month does not have on 15,664 of the 872,960 input combinations, and `else end_of_month` is used. The first is received 2027-01-30, closing_day 29, payment_month 1, payment_day 10: 2027-02-29 does not exist, and 2027-02-28 is taken.

  With `else start_of_next_month` instead, closing would differ on 16,192 of the 872,960 input combinations. With `else reject` instead, 15,664 of the 872,960 input combinations would be refused.

- `day payment_day of month +payment_month else end_of_month` (payment)

  It lands on a day the month does not have on 19,844 of the 872,960 input combinations, and `else end_of_month` is used. The first is received 2027-01-01, closing_day 1, payment_month 1, payment_day 29: 2027-02-29 does not exist, and 2027-02-28 is taken.

  With `else start_of_next_month` instead, payment would differ on 12,372 of the 872,960 input combinations. With `else reject` instead, 19,844 of the 872,960 input combinations would be refused.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| received | closing_day | payment_month | payment_day | closing | payment | Why |
|---|---|---|---|---|---|---|
| 2027-01-01 Fri | 1 | 1 | 10 | 2027-01-01 Fri | 2027-02-10 Wed | received is New Year’s Day, a closed day |
| 2027-01-31 Sun | 1 | 1 | 10 | 2027-02-01 Mon | 2027-03-10 Wed | received is the 31st, the end of its month |
| 2027-02-02 Tue | 1 | 1 | 10 | 2027-03-01 Mon | 2027-04-09 Fri | payment falls on a Saturday and moves to the business day before |
| 2027-02-28 Sun | 1 | 1 | 10 | 2027-03-01 Mon | 2027-04-09 Fri | received is 28 February, the end of the month in a year that is not a leap year |
| 2027-03-25 Thu | 1 | 1 | 10 | 2027-04-01 Thu | 2027-05-10 Mon | received is the day before Good Friday |
| 2027-03-30 Tue | 1 | 1 | 10 | 2027-04-01 Thu | 2027-05-10 Mon | received is the day after Easter Monday |
| 2027-08-02 Mon | 1 | 1 | 10 | 2027-09-01 Wed | 2027-10-08 Fri | payment falls on a Sunday and moves to the business day before |
| 2028-02-02 Wed | 1 | 1 | 14 | 2028-03-01 Wed | 2028-04-13 Thu | payment falls on Good Friday and moves to the business day before |
| 2028-02-02 Wed | 1 | 1 | 17 | 2028-03-01 Wed | 2028-04-13 Thu | payment falls on Easter Monday and moves to the business day before; payment moves the most (4 days) |
| 2027-10-02 Sat | 1 | 1 | 27 | 2027-11-01 Mon | 2027-12-24 Fri | payment falls on Christmas Day (Substitute day) and moves to the business day before |
| 2027-10-02 Sat | 1 | 1 | 28 | 2027-11-01 Mon | 2027-12-24 Fri | payment falls on Boxing Day (Substitute day) and moves to the business day before |
| 2027-01-01 Fri | 1 | 1 | 29 | 2027-01-01 Fri | 2027-02-26 Fri | computing payment lands on 2027-02-29, which does not exist, and uses `else end_of_month` |
| 2027-05-02 Sun | 1 | 2 | 31 | 2027-06-01 Tue | 2027-08-31 Tue | the most days from received to payment (121) |
| 2027-01-30 Sat | 29 | 1 | 10 | 2027-02-28 Sun | 2027-03-10 Wed | computing closing lands on 2027-02-29, which does not exist, and uses `else end_of_month` |
| 2027-08-31 Tue | 30 | 1 | 10 | 2027-09-30 Thu | 2027-10-08 Fri | the least room for the claim paid_after_closing (payment is 8 days after closing; the claim allows at least 1 day after) |
| 2027-09-30 Thu | 30 | 1 | 10 | 2027-09-30 Thu | 2027-10-08 Fri | the fewest days from received to payment (8) |

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

In 2027-01-01..2028-12-29, the days of received and of the dates computed from it, the longest run of closed days is 2027-03-26..2027-03-29, 4 days.

### Month by month

In the tables, a day in parentheses is closed; ◆ marks the input of an edge case.

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | (1)◆ | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30)◆ | (31)◆ |

Named closed days: 1 New Year’s Day

**February 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2◆ | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | 26 | (27) | (28)◆ |

**March 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25◆ | (26) | (27) | (28) |
| (29) | 30◆ | 31 |  |  |  |  |

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
|  |  |  |  |  | (1) | (2)◆ |
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
| 2◆ | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| (30) | 31◆ |  |  |  |  |  |

Named closed days: 30 Summer bank holiday

**September 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30◆ |  |  |  |

**October 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2)◆ | (3) |
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

**January 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| (3) | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | 11 | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21 | (22) | (23) |
| 24 | 25 | 26 | 27 | 28 | (29) | (30) |
| 31 |  |  |  |  |  |  |

Named closed days: 3 New Year’s Day (Substitute day)

**February 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2◆ | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| 21 | 22 | 23 | 24 | 25 | (26) | (27) |
| 28 | 29 |  |  |  |  |  |

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

**May 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| (1) | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | 26 | (27) | (28) |
| (29) | 30 | 31 |  |  |  |  |

Named closed days: 1 Early May bank holiday, 29 Spring bank holiday

**June 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| 12 | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 |  |  |

**July 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| 3 | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | 11 | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21 | (22) | (23) |
| 24 | 25 | 26 | 27 | 28 | (29) | (30) |
| 31 |  |  |  |  |  |  |

**August 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| 21 | 22 | 23 | 24 | 25 | (26) | (27) |
| (28) | 29 | 30 | 31 |  |  |  |

Named closed days: 28 Summer bank holiday

**September 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) |  |

**October 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 | 31 |  |  |  |  |  |

**November 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 |  |  |  |

**December 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| (25) | (26) | 27 | 28 | 29 | (30) | (31) |

Named closed days: 25 Christmas Day, 26 Boxing Day
