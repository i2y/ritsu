# 締め日と支払日を受け取る v1

締め日、支払の月、支払の日を整数で受け取る。締め日を規則から受け取る形。支払日が休みなら前の営業日。条件は、この例のために文字どおりに書いたもの

- File: `closing_and_payment_days_as_inputs.ja.cal` (dates 締め日と支払日を受け取る v1, sha256:b9ecb83d731da674)
- Calendar: `calendars/東京の営業日.cal` (calendar 東京の営業日 v1, sha256:d7b6134e23a8cb9f)
- Table: 祝日 = `calendars/data/syukujitsu.csv` (sha256:cec37a743c96995c, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, covers 1955-01-01..2027-12-31)
- koyomi: 0.1.0

koyomi 0.1.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 3 claims hold on all 871,596 combinations of 受領日 (2026-01-01..2027-10-01), 締め日 (1..31), 支払の月 (1..2) and 支払の日 (10..31).

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### 締め (closing)

From 受領日, in this order:

1. The closing day of its period, closing on the day 締め日; the end of that month when it has no such day `close day 締め日 else end_of_month  # 締め日で締める。その月に無い日（31 日など）なら月末で締める`

### 支払 (payment)

From 締め, in this order:

1. The day 支払の日 of the month 支払の月 months later; the end of that month when it has no such day `day 支払の日 of month +支払の月 else end_of_month  # 支払の月だけ後の月の、支払の日。無い日なら月末`
2. If it is closed, the business day before `roll preceding  # 支払日が休みなら前の営業日`

   The day is closed, and moves, on 298,780 of the 871,596 input combinations; by 5 days at most.

## What was checked

Every date was computed, and every claim checked, on all 871,596 combinations of 受領日 (2026-01-01..2027-10-01), 締め日 (1..31), 支払の月 (1..2) and 支払の日 (10..31).

| Claim | As written | Result |
|---|---|---|
| 営業日に払う | `支払 is open` | holds on all 871,596 input combinations |
| 締めのあとに払う | `支払 > 締め` | holds on all 871,596 input combinations. The least room is at 受領日 2026-03-31, 締め日 30, 支払の月 1, 支払の日 10, where 支払 2026-05-08 is 8 days after 締め (the claim allows at least 1 day after) |
| 遅い受領は遅い支払 | `支払 is monotonic` | holds on all 870,232 pairs of adjacent days |

## Days a month does not have

Adding months, taking a day of a month some months away, and closing on a day of the month can land on a day the month does not have, such as 30 February. The .cal says what to do then. For every input of the range, koyomi counted how often that is used, and how often another way would change the result.

- `close day 締め日 else end_of_month` (締め)

  It lands on a day the month does not have on 16,896 of the 871,596 input combinations, and `else end_of_month` is used. The first is 受領日 2026-01-30, 締め日 29, 支払の月 1, 支払の日 10: 2026-02-29 does not exist, and 2026-02-28 is taken.

  With `else start_of_next_month` instead, 締め would differ on 17,468 of the 871,596 input combinations. With `else reject` instead, 16,896 of the 871,596 input combinations would be refused.

- `day 支払の日 of month +支払の月 else end_of_month` (支払)

  It lands on a day the month does not have on 21,705 of the 871,596 input combinations, and `else end_of_month` is used. The first is 受領日 2026-01-01, 締め日 1, 支払の月 1, 支払の日 29: 2026-02-29 does not exist, and 2026-02-28 is taken.

  With `else start_of_next_month` instead, 支払 would differ on 18,388 of the 871,596 input combinations. With `else reject` instead, 21,705 of the 871,596 input combinations would be refused.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| 受領日 | 締め日 | 支払の月 | 支払の日 | 締め | 支払 | Why |
|---|---|---|---|---|---|---|
| 2026-01-01 Thu | 1 | 1 | 10 | 2026-01-01 Thu | 2026-02-10 Tue | 受領日 is 元日, a closed day; 受領日 falls in 年末年始 |
| 2026-01-13 Tue | 1 | 1 | 10 | 2026-02-01 Sun | 2026-03-10 Tue | 受領日 is the day after 成人の日 |
| 2026-01-31 Sat | 1 | 1 | 10 | 2026-02-01 Sun | 2026-03-10 Tue | 受領日 is the 31st, the end of its month |
| 2026-02-10 Tue | 1 | 1 | 10 | 2026-03-01 Sun | 2026-04-10 Fri | 受領日 is the day before 建国記念の日 |
| 2026-03-02 Mon | 1 | 1 | 10 | 2026-04-01 Wed | 2026-05-08 Fri | 支払 falls on a Sunday and moves to the business day before |
| 2026-08-02 Sun | 1 | 1 | 10 | 2026-09-01 Tue | 2026-10-09 Fri | 支払 falls on a Saturday and moves to the business day before |
| 2026-01-01 Thu | 1 | 1 | 11 | 2026-01-01 Thu | 2026-02-10 Tue | 支払 falls on 建国記念の日 and moves to the business day before |
| 2026-06-02 Tue | 1 | 1 | 11 | 2026-07-01 Wed | 2026-08-10 Mon | 支払 falls on 山の日 and moves to the business day before |
| 2026-11-02 Mon | 1 | 1 | 11 | 2026-12-01 Tue | 2027-01-08 Fri | 支払 falls on 成人の日 and moves to the business day before |
| 2027-08-02 Mon | 1 | 1 | 11 | 2027-09-01 Wed | 2027-10-08 Fri | 支払 falls on スポーツの日 and moves to the business day before |
| 2026-07-02 Thu | 1 | 1 | 23 | 2026-08-01 Sat | 2026-09-18 Fri | 支払 moves the most (5 days) |
| 2026-01-01 Thu | 1 | 1 | 29 | 2026-01-01 Thu | 2026-02-27 Fri | computing 支払 lands on 2026-02-29, which does not exist, and uses `else end_of_month` |
| 2026-05-02 Sat | 1 | 2 | 31 | 2026-06-01 Mon | 2026-08-31 Mon | the most days from 受領日 to 支払 (121) |
| 2026-01-30 Fri | 29 | 1 | 10 | 2026-02-28 Sat | 2026-03-10 Tue | computing 締め lands on 2026-02-29, which does not exist, and uses `else end_of_month` |
| 2026-03-31 Tue | 30 | 1 | 10 | 2026-04-30 Thu | 2026-05-08 Fri | the least room for the claim 締めのあとに払う (支払 is 8 days after 締め; the claim allows at least 1 day after) |
| 2026-04-30 Thu | 30 | 1 | 10 | 2026-04-30 Thu | 2026-05-08 Fri | the fewest days from 受領日 to 支払 (8) |

## Calendar

The file reads the calendar 東京の営業日 v1 (`calendars/東京の営業日.cal`).

Its description: 土日、国民の祝日と休日、12 月 29 日から 1 月 3 日までを休む

### Closed days

These days are closed.

- every Saturday and Sunday `closed weekly sat, sun`
- the days the table 祝日 lists `closed 祝日`
- every year, from 29 December to 3 January (年末年始) `closed every 12-29..01-03 "年末年始"`

### Sources and the days the calendar knows

- 祝日: 1,067 rows, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, pinned at sha256:cec37a743c96995c; it lists every closed day from 1955-01-01 to 2027-12-31.

The calendar knows 1955-01-01..2027-12-31. A computation that asks whether a day outside it is a business day stops, in the check and in the generated code alike.

In 2026-01-01..2027-12-28, the days of 受領日 and of the dates computed from it, the longest run of closed days is 2026-12-29..2027-01-03, 6 days.

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

Named closed days: 1 元日 and 年末年始, 2–3 年末年始, 12 成人の日

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

Named closed days: 29–31 年末年始

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | (1) | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| (11) | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 1 元日 and 年末年始, 2–3 年末年始, 11 成人の日

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

Named closed days: 29–31 年末年始
