# payment_20th_close_next_10th v1

Closes on the 20th; pays on the 10th of the next month, or on the business day before when that day is closed. The English version of payment_20th_close_next_10th.ja.cal. The claim within_60_days_of_receipt is this example's own, written as it reads; it does not say how any law is read

- File: `payment_20th_close_next_10th.cal` (dates payment_20th_close_next_10th v1, sha256:9973542043dbee17)
- Calendar: `calendars/tokyo_business_days.cal` (calendar tokyo_business_days v1, sha256:37af228cf6ba7b95)
- Table: national_holidays = `calendars/data/syukujitsu.csv` (sha256:cec37a743c96995c, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, covers 1955-01-01..2027-12-31)
- koyomi: 0.24.0

koyomi 0.24.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 3 claims hold on all 689 days of received (2026-01-01..2027-11-20); 2 examples match.

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### closing

From received, in this order:

1. The closing day of its period, closing on the 20th (the 21st to the 20th) `close day 20  # "closes on the 20th"`

### payment

From closing, in this order:

1. The 10th of the next month `day 10 of month +1  # "pays on the 10th of the next month"`
2. If it is closed, the business day before `roll preceding  # "on the business day before when that day is closed"`

   The day is closed, and moves, on 182 of the 689 days of received; by 2 days at most.

3. At 09:00 (+09:00) on that day `at 09:00`

## What was checked

Every date was computed, and every claim checked, on all 689 days of received (2026-01-01..2027-11-20).

| Claim | As written | Result |
|---|---|---|
| paid_on_a_business_day | `payment is open` | holds on all 689 days of received |
| within_60_days_of_receipt | `payment <= received + 60 days` | holds on all 689 days of received. The least room is at received 2026-07-21, where payment 2026-09-10 is 51 days after received (the claim allows at most 60 days after) |
| later_receipt_later_payment | `payment is monotonic` | holds on all 688 pairs of adjacent days |

## Days a month does not have

No operation of this file can land on a day its month does not have, such as 30 February.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| received | closing | payment | Why |
|---|---|---|---|
| 2026-01-01 Thu | 2026-01-20 Tue | 2026-02-10 Tue | received is 元日, a closed day; received falls in New Year holidays |
| 2026-01-13 Tue | 2026-01-20 Tue | 2026-02-10 Tue | received is the day after 成人の日 |
| 2026-01-31 Sat | 2026-02-20 Fri | 2026-03-10 Tue | received is the 31st, the end of its month |
| 2026-02-10 Tue | 2026-02-20 Fri | 2026-03-10 Tue | received is the day before 建国記念の日 |
| 2026-02-20 Fri | 2026-02-20 Fri | 2026-03-10 Tue | the fewest days from received to payment (18) |
| 2026-02-28 Sat | 2026-03-20 Fri | 2026-04-10 Fri | received is 28 February, the end of the month in a year that is not a leap year |
| 2026-03-21 Sat | 2026-04-20 Mon | 2026-05-08 Fri | payment falls on a Sunday and moves to the business day before; payment moves the most (2 days) |
| 2026-04-30 Thu | 2026-05-20 Wed | 2026-06-10 Wed | received is the 30th, the end of its month |
| 2026-07-21 Tue | 2026-08-20 Thu | 2026-09-10 Thu | the least room for the claim within_60_days_of_receipt (payment is 51 days after received; the claim allows at most 60 days after); the most days from received to payment (51) |
| 2026-08-21 Fri | 2026-09-20 Sun | 2026-10-09 Fri | payment falls on a Saturday and moves to the business day before |

## Examples

The rows the .cal writes under examples, and what the computation gives.

| received | → closing | → payment | Result |
|---|---|---|---|
| 2026-04-01 Wed | 2026-04-20 Mon | 2026-05-08 Fri | matches |
| 2026-12-21 Mon | 2027-01-20 Wed | 2027-02-10 Wed | matches |

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

In 2026-01-01..2027-12-10, the days of received and of the dates computed from it, the longest run of closed days is 2026-12-29..2027-01-03, 6 days.

### Month by month

In the tables, a day in parentheses is closed; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1)◆ | (2) | (3) | (4) |
| 5 | 6 | 7 | 8 | 9 | (10) | (11) |
| (12) | 13◆ | 14 | 15 | 16 | (17) | (18) |
| 19 | 20 | 21 | 22 | 23 | (24) | (25) |
| 26 | 27 | 28 | 29 | 30 | (31)◆ |  |

Named closed days: 1 元日 and New Year holidays, 2–3 New Year holidays, 12 成人の日

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10◆ | (11) | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20◆ | (21) | (22) |
| (23) | 24 | 25 | 26 | 27 | (28)◆ |  |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | (20) | (21)◆ | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| 30 | 31 |  |  |  |  |  |

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
|  |  |  |  | 1 | (2) | (3) |
| (4) | (5) | (6) | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日, 6 休日

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
| (20) | 21◆ | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |

Named closed days: 20 海の日

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | (1) | (2) |
| 3 | 4 | 5 | 6 | 7 | (8) | (9) |
| 10 | (11) | 12 | 13 | 14 | (15) | (16) |
| 17 | 18 | 19 | 20 | 21◆ | (22) | (23) |
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
| 2 | (3) | 4 | 5 | 6 | (7) | (8) |
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
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
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
