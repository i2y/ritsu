# closing_and_payment_days_as_inputs v1

Takes the closing day, the month of payment and the day of payment as integers, as when the closing day comes from a rule. A payment day that is closed moves to the business day before. The claims are this example's own, written as they read. The English version of closing_and_payment_days_as_inputs.ja.cal

- File: `closing_and_payment_days_as_inputs.cal` (dates closing_and_payment_days_as_inputs v1, sha256:e4aef610c514fd26)
- Calendar: `calendars/tokyo_business_days.cal` (calendar tokyo_business_days v1, sha256:37af228cf6ba7b95)
- Table: national_holidays = `calendars/data/syukujitsu.csv` (sha256:cec37a743c96995c, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, covers 1955-01-01..2027-12-31)
- koyomi: 0.25.0

koyomi 0.25.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 3 claims hold on all 871,596 combinations of received (2026-01-01..2027-10-01), closing_day (1..31), payment_month (1..2) and payment_day (10..31).

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### closing

From received, in this order:

1. The closing day of its period, closing on the day closing_day; the end of that month when it has no such day `close day closing_day else end_of_month  # closes on closing_day; in a month without that day (the 31st and the like), at the end of the month`

### payment

From closing, in this order:

1. The day payment_day of the month payment_month months later; the end of that month when it has no such day `day payment_day of month +payment_month else end_of_month  # payment_day of the month payment_month later; at the end of the month when it has no such day`
2. If it is closed, the business day before `roll preceding  # on the business day before when that day is closed`

   The day is closed, and moves, on 298,780 of the 871,596 input combinations; by 5 days at most.

## What was checked

Every date was computed, and every claim checked, on all 871,596 combinations of received (2026-01-01..2027-10-01), closing_day (1..31), payment_month (1..2) and payment_day (10..31).

| Claim | As written | Result |
|---|---|---|
| paid_on_a_business_day | `payment is open` | holds on all 871,596 input combinations |
| paid_after_closing | `payment > closing` | holds on all 871,596 input combinations. The least room is at received 2026-03-31, closing_day 30, payment_month 1, payment_day 10, where payment 2026-05-08 is 8 days after closing (the claim allows at least 1 day after) |
| later_receipt_later_payment | `payment is monotonic` | holds on all 870,232 pairs of adjacent days |

## Days a month does not have

Adding months, taking a day of a month some months away, and closing on a day of the month can land on a day the month does not have, such as 30 February. The .cal says what to do then. For every input of the range, koyomi counted how often that is used, and how often another way would change the result.

- `close day closing_day else end_of_month` (closing)

  It lands on a day the month does not have on 16,896 of the 871,596 input combinations, and `else end_of_month` is used. The first is received 2026-01-30, closing_day 29, payment_month 1, payment_day 10: 2026-02-29 does not exist, and 2026-02-28 is taken.

  With `else start_of_next_month` instead, closing would differ on 17,468 of the 871,596 input combinations. With `else reject` instead, 16,896 of the 871,596 input combinations would be refused.

- `day payment_day of month +payment_month else end_of_month` (payment)

  It lands on a day the month does not have on 21,705 of the 871,596 input combinations, and `else end_of_month` is used. The first is received 2026-01-01, closing_day 1, payment_month 1, payment_day 29: 2026-02-29 does not exist, and 2026-02-28 is taken.

  With `else start_of_next_month` instead, payment would differ on 18,388 of the 871,596 input combinations. With `else reject` instead, 21,705 of the 871,596 input combinations would be refused.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| received | closing_day | payment_month | payment_day | closing | payment | Why |
|---|---|---|---|---|---|---|
| 2026-01-01 Thu | 1 | 1 | 10 | 2026-01-01 Thu | 2026-02-10 Tue | received is 元日, a closed day; received falls in New Year holidays |
| 2026-01-13 Tue | 1 | 1 | 10 | 2026-02-01 Sun | 2026-03-10 Tue | received is the day after 成人の日 |
| 2026-01-31 Sat | 1 | 1 | 10 | 2026-02-01 Sun | 2026-03-10 Tue | received is the 31st, the end of its month |
| 2026-02-10 Tue | 1 | 1 | 10 | 2026-03-01 Sun | 2026-04-10 Fri | received is the day before 建国記念の日 |
| 2026-03-02 Mon | 1 | 1 | 10 | 2026-04-01 Wed | 2026-05-08 Fri | payment falls on a Sunday and moves to the business day before |
| 2026-08-02 Sun | 1 | 1 | 10 | 2026-09-01 Tue | 2026-10-09 Fri | payment falls on a Saturday and moves to the business day before |
| 2026-01-01 Thu | 1 | 1 | 11 | 2026-01-01 Thu | 2026-02-10 Tue | payment falls on 建国記念の日 and moves to the business day before |
| 2026-06-02 Tue | 1 | 1 | 11 | 2026-07-01 Wed | 2026-08-10 Mon | payment falls on 山の日 and moves to the business day before |
| 2026-11-02 Mon | 1 | 1 | 11 | 2026-12-01 Tue | 2027-01-08 Fri | payment falls on 成人の日 and moves to the business day before |
| 2027-08-02 Mon | 1 | 1 | 11 | 2027-09-01 Wed | 2027-10-08 Fri | payment falls on スポーツの日 and moves to the business day before |
| 2026-07-02 Thu | 1 | 1 | 23 | 2026-08-01 Sat | 2026-09-18 Fri | payment moves the most (5 days) |
| 2026-01-01 Thu | 1 | 1 | 29 | 2026-01-01 Thu | 2026-02-27 Fri | computing payment lands on 2026-02-29, which does not exist, and uses `else end_of_month` |
| 2026-05-02 Sat | 1 | 2 | 31 | 2026-06-01 Mon | 2026-08-31 Mon | the most days from received to payment (121) |
| 2026-01-30 Fri | 29 | 1 | 10 | 2026-02-28 Sat | 2026-03-10 Tue | computing closing lands on 2026-02-29, which does not exist, and uses `else end_of_month` |
| 2026-03-31 Tue | 30 | 1 | 10 | 2026-04-30 Thu | 2026-05-08 Fri | the least room for the claim paid_after_closing (payment is 8 days after closing; the claim allows at least 1 day after) |
| 2026-04-30 Thu | 30 | 1 | 10 | 2026-04-30 Thu | 2026-05-08 Fri | the fewest days from received to payment (8) |

## Calendar

The file reads the calendar tokyo_business_days v1 (`calendars/tokyo_business_days.cal`).

Its description: Saturdays, Sundays, Japan's national holidays and other days off, and 29 December to 3 January are closed. The English version of 東京の営業日.cal

### Closed days

These days are closed.

- every Saturday and Sunday `closed weekly sat, sun`
- the days the table national_holidays lists `closed national_holidays`
- every year, from 29 December to 3 January (New Year holidays) `closed every 12-29..01-03 "New Year holidays"`

### Sources and the days the calendar knows

- national_holidays: 1,067 rows, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, pinned at sha256:cec37a743c96995c; it lists every closed day from 1955-01-01 to 2027-12-31.

The calendar knows 1955-01-01..2027-12-31. A computation that asks whether a day outside it is a business day stops, in the check and in the generated code alike.

In 2026-01-01..2027-12-28, the days of received and of the dates computed from it, the longest run of closed days is 2026-12-29..2027-01-03, 6 days.

### Month by month

In the tables, a day in parentheses is closed; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1)◆ | (2) | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| (12) | 13◆ | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30◆ | (31)◆ |  |

Named closed days: 1 元日 and New Year holidays, 2–3 New Year holidays, 12 成人の日

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10◆ | (11) | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| (23) | 24 | 25 | 26 | 27 | (28) |  |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2◆ | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | (20) | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 | 31◆ |  |  |  |  |  |

Named closed days: 20 春分の日

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | (29) | 30◆ |  |  |  |

Named closed days: 29 昭和の日

**May 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2)◆ | (3) |
| (4) | (5) | (6) | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日, 6 休日

**June 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2◆ | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | 26 | (27) | (28) |
| 29 | 30 |  |  |  |  |  |

**July 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2◆ | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| (20) | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |

Named closed days: 20 海の日

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2)◆ |
| 3 | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | (11) | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21 | (22) | (23) |
| 24 | 25 | 26 | 27 | 28 | (29) | (30) |
| 31 |  |  |  |  |  |  |

Named closed days: 11 山の日

**September 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| (21) | (22) | (23) | 24 | 25 | (26) | (27) |
| 28 | 29 | 30 |  |  |  |  |

Named closed days: 21 敬老の日, 22 休日, 23 秋分の日

**October 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| (12) | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 | (31) |  |

Named closed days: 12 スポーツの日

**November 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2◆ | (3) | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| (23) | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 |  |  |  |  |  |  |

Named closed days: 3 文化の日, 23 勤労感謝の日

**December 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | (5) | (6) |
| 7 | 8 | 9 | 10 | 11 | (12) | (13) |
| 14 | 15 | 16 | 17 | 18 | (19) | (20) |
| 21 | 22 | 23 | 24 | 25 | (26) | (27) |
| 28 | (29) | (30) | (31) |  |  |  |

Named closed days: 29–31 New Year holidays

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | (1) | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| (11) | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 1 元日 and New Year holidays, 2–3 New Year holidays, 11 成人の日

**February 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | (11) | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | (23) | 24 | 25 | 26 | (27) | (28) |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| (22) | 23 | 24 | 25 | 26 | (27) | (28) |
| 29 | 30 | 31 |  |  |  |  |

Named closed days: 21 春分の日, 22 休日

**April 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| 12 | 13 | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | (29) | 30 |  |  |

Named closed days: 29 昭和の日

**May 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| (3) | (4) | (5) | 6 | 7 | (8) | (9) |
| 10 | 11 | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21 | (22) | (23) |
| 24 | 25 | 26 | 27 | 28 | (29) | (30) |
| 31 |  |  |  |  |  |  |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日

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
| (19) | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 | (31) |  |

Named closed days: 19 海の日

**August 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2◆ | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | (11) | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 | 31 |  |  |  |  |  |

Named closed days: 11 山の日

**September 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| (20) | 21 | 22 | (23) | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 |  |  |  |

Named closed days: 20 敬老の日, 23 秋分の日

**October 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| (11) | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 11 スポーツの日

**November 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | (3) | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | (23) | 24 | 25 | 26 | (27) | (28) |
| 29 | 30 |  |  |  |  |  |

Named closed days: 3 文化の日, 23 勤労感謝の日

**December 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | (29) | (30) | (31) |  |  |

Named closed days: 29–31 New Year holidays
