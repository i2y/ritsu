# civil_code_period_end v1

The last day of a period, computed by writing Articles 140, 141 and 143 of the Civil Code as they read, with the day after of Article 142 written as it reads too. It does not settle how the articles are read. The English version of civil_code_period_end.ja.cal

- File: `civil_code_period_end.cal` (dates civil_code_period_end v1, sha256:88e7b68f48f76570)
- Calendar: `calendars/civil_code_142_days.cal` (calendar civil_code_142_days v1, sha256:dd534a7343411409)
- Table: national_holidays = `calendars/data/syukujitsu.csv` (sha256:cec37a743c96995c, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, covers 1955-01-01..2027-12-31)
- Law: civil_code = law 129AC0000000089 on e-Gov as of 2026-10-01 (revision 129AC0000000089_20260624_508AC0000000045): 第140条 sha256:e880059021fbb67d, 第141条 sha256:0575c131b9f08063, 第142条 sha256:fc8c35a0769d3b35, 第143条 sha256:6950bdfb988439b6
- koyomi: 0.23.0

koyomi 0.23.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!NOTE]
> 4 claims hold on all 4,380 combinations of origin (2026-01-01..2026-12-31) and month_count (1..12).

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### first_day

From origin, in this order:

1. The day after `+ 1 day  # the first day is not counted`

> **civil_code 第140条 (e-Gov, as of 2026-10-01, revision 129AC0000000089_20260624_508AC0000000045)**
>
> 第百四十条　日、週、月又は年によって期間を定めたときは、期間の初日は、算入しない。ただし、その期間が午前零時から始まるときは、この限りでない。

### last_day

From first_day, in this order:

1. The same day month_count months later; the first of the following month when it has no such day `+ month_count months else start_of_next_month  # the corresponding day of the last month; with none, the 1st of the month after`
2. The day before `- 1 day  # the day before it: the last day of the last month when it had no corresponding day`

> **civil_code 第141条 (e-Gov, as of 2026-10-01, revision 129AC0000000089_20260624_508AC0000000045)**
>
> （期間の満了）
>
> 第百四十一条　前条の場合には、期間は、その末日の終了をもって満了する。

> **civil_code 第143条 (e-Gov, as of 2026-10-01, revision 129AC0000000089_20260624_508AC0000000045)**
>
> （暦による期間の計算）
>
> 第百四十三条　週、月又は年によって期間を定めたときは、その期間は、暦に従って計算する。
>
> ２　週、月又は年の初めから期間を起算しないときは、その期間は、最後の週、月又は年においてその起算日に応当する日の前日に満了する。ただし、月又は年によって期間を定めた場合において、最後の月に応当する日がないときは、その月の末日に満了する。

### last_day_142

From last_day, in this order:

1. If it is closed, the day after (once, whatever that day is) `if closed + 1 day  # when the last day is a day off, the day after`

   The day is closed, and the line acts, on 843 of the 4,380 input combinations.

> **civil_code 第142条 (e-Gov, as of 2026-10-01, revision 129AC0000000089_20260624_508AC0000000045)**
>
> 第百四十二条　期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。

## What was checked

Every date was computed, and every claim checked, on all 4,380 combinations of origin (2026-01-01..2026-12-31) and month_count (1..12).

| Claim | As written | Result |
|---|---|---|
| last_day_is_monotonic | `last_day is monotonic` | holds on all 4,368 pairs of adjacent days |
| last_day_142_is_monotonic | `last_day_142 is monotonic` | holds on all 4,368 pairs of adjacent days |
| last_day_after_origin | `last_day > origin` | holds on all 4,380 input combinations. The least room is at origin 2026-01-31, month_count 1, where last_day 2026-02-28 is 28 days after origin (the claim allows at least 1 day after) |
| last_day_142_on_or_after_last | `last_day_142 >= last_day` | holds on all 4,380 input combinations. The least room is at origin 2026-01-02, month_count 1, where last_day_142 2026-02-02 is the same day as last_day (the claim allows at least the same day) |

## Days a month does not have

Adding months, taking a day of a month some months away, and closing on a day of the month can land on a day the month does not have, such as 30 February. The .cal says what to do then. For every input of the range, koyomi counted how often that is used, and how often another way would change the result.

- `+ month_count months else start_of_next_month` (last_day)

  It lands on a day the month does not have on 57 of the 4,380 input combinations, and `else start_of_next_month` is used. The first is origin 2026-01-28, month_count 1: 2026-02-29 does not exist, and 2026-03-01 is taken.

  With `else end_of_month` instead, last_day would differ on 57 of the 4,380 input combinations. With `else reject` instead, 57 of the 4,380 input combinations would be refused.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| origin | month_count | first_day | last_day | last_day_142 | Why |
|---|---|---|---|---|---|
| 2026-01-01 Thu | 1 | 2026-01-02 Fri | 2026-02-01 Sun | 2026-02-02 Mon | last_day_142 falls on a Sunday, and `if closed + 1 day` acts; origin is 元日, a closed day |
| 2026-01-02 Fri | 1 | 2026-01-03 Sat | 2026-02-02 Mon | 2026-02-02 Mon | the least room for the claim last_day_142_on_or_after_last (last_day_142 is the same day as last_day; the claim allows at least the same day); origin is the day after 元日 |
| 2026-01-28 Wed | 1 | 2026-01-29 Thu | 2026-02-28 Sat | 2026-02-28 Sat | computing last_day lands on 2026-02-29, which does not exist, and uses `else start_of_next_month` |
| 2026-01-31 Sat | 1 | 2026-02-01 Sun | 2026-02-28 Sat | 2026-02-28 Sat | the least room for the claim last_day_after_origin (last_day is 28 days after origin; the claim allows at least 1 day after); the fewest days from origin to last_day_142 (28); origin is the 31st, the end of its month |
| 2026-02-10 Tue | 1 | 2026-02-11 Wed | 2026-03-10 Tue | 2026-03-10 Tue | origin is the day before 建国記念の日 |
| 2026-02-28 Sat | 1 | 2026-03-01 Sun | 2026-03-31 Tue | 2026-03-31 Tue | origin is 28 February, the end of the month in a year that is not a leap year |
| 2026-04-30 Thu | 1 | 2026-05-01 Fri | 2026-05-31 Sun | 2026-06-01 Mon | origin is the 30th, the end of its month |
| 2026-01-01 Thu | 12 | 2026-01-02 Fri | 2027-01-01 Fri | 2027-01-02 Sat | the most days from origin to last_day_142 (366) |

## Calendar

The file reads the calendar civil_code_142_days v1 (`calendars/civil_code_142_days.cal`).

Its description: Only the days Article 142 of the Civil Code names are closed: Sundays, and the days off under the Act on National Holidays. This example does not decide what the other days off of the article are. The English version of 民法142条の休日.cal

### Closed days

These days are closed.

- every Sunday `closed weekly sun`
- the days the table national_holidays lists `closed national_holidays`

### Sources and the days the calendar knows

- national_holidays: 1,067 rows, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, pinned at sha256:cec37a743c96995c; it lists every closed day from 1955-01-01 to 2027-12-31.

The calendar knows 1955-01-01..2027-12-31. A computation that asks whether a day outside it is a business day stops, in the check and in the generated code alike.

In 2026-01-01..2027-12-31, the days of origin and of the dates computed from it, the longest run of closed days is 2026-05-03..2026-05-06, 4 days.

### Month by month

In the tables, a day in parentheses is closed; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1)◆ | 2◆ | 3 | (4) |
| 5 | 6 | 7 | 8 | 9 | 10 | (11) |
| (12) | 13 | 14 | 15 | 16 | 17 | (18) |
| 19 | 20 | 21 | 22 | 23 | 24 | (25) |
| 26 | 27 | 28◆ | 29 | 30 | 31◆ |  |

Named closed days: 1 元日, 12 成人の日

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | 7 | (8) |
| 9 | 10◆ | (11) | 12 | 13 | 14 | (15) |
| 16 | 17 | 18 | 19 | 20 | 21 | (22) |
| (23) | 24 | 25 | 26 | 27 | 28◆ |  |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | 7 | (8) |
| 9 | 10 | 11 | 12 | 13 | 14 | (15) |
| 16 | 17 | 18 | 19 | (20) | 21 | (22) |
| 23 | 24 | 25 | 26 | 27 | 28 | (29) |
| 30 | 31 |  |  |  |  |  |

Named closed days: 20 春分の日

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | 4 | (5) |
| 6 | 7 | 8 | 9 | 10 | 11 | (12) |
| 13 | 14 | 15 | 16 | 17 | 18 | (19) |
| 20 | 21 | 22 | 23 | 24 | 25 | (26) |
| 27 | 28 | (29) | 30◆ |  |  |  |

Named closed days: 29 昭和の日

**May 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | 2 | (3) |
| (4) | (5) | (6) | 7 | 8 | 9 | (10) |
| 11 | 12 | 13 | 14 | 15 | 16 | (17) |
| 18 | 19 | 20 | 21 | 22 | 23 | (24) |
| 25 | 26 | 27 | 28 | 29 | 30 | (31) |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日, 6 休日

**June 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | 6 | (7) |
| 8 | 9 | 10 | 11 | 12 | 13 | (14) |
| 15 | 16 | 17 | 18 | 19 | 20 | (21) |
| 22 | 23 | 24 | 25 | 26 | 27 | (28) |
| 29 | 30 |  |  |  |  |  |

**July 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | 4 | (5) |
| 6 | 7 | 8 | 9 | 10 | 11 | (12) |
| 13 | 14 | 15 | 16 | 17 | 18 | (19) |
| (20) | 21 | 22 | 23 | 24 | 25 | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |

Named closed days: 20 海の日

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | 1 | (2) |
| 3 | 4 | 5 | 6 | 7 | 8 | (9) |
| 10 | (11) | 12 | 13 | 14 | 15 | (16) |
| 17 | 18 | 19 | 20 | 21 | 22 | (23) |
| 24 | 25 | 26 | 27 | 28 | 29 | (30) |
| 31 |  |  |  |  |  |  |

Named closed days: 11 山の日

**September 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | 5 | (6) |
| 7 | 8 | 9 | 10 | 11 | 12 | (13) |
| 14 | 15 | 16 | 17 | 18 | 19 | (20) |
| (21) | (22) | (23) | 24 | 25 | 26 | (27) |
| 28 | 29 | 30 |  |  |  |  |

Named closed days: 21 敬老の日, 22 休日, 23 秋分の日

**October 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | 3 | (4) |
| 5 | 6 | 7 | 8 | 9 | 10 | (11) |
| (12) | 13 | 14 | 15 | 16 | 17 | (18) |
| 19 | 20 | 21 | 22 | 23 | 24 | (25) |
| 26 | 27 | 28 | 29 | 30 | 31 |  |

Named closed days: 12 スポーツの日

**November 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | (3) | 4 | 5 | 6 | 7 | (8) |
| 9 | 10 | 11 | 12 | 13 | 14 | (15) |
| 16 | 17 | 18 | 19 | 20 | 21 | (22) |
| (23) | 24 | 25 | 26 | 27 | 28 | (29) |
| 30 |  |  |  |  |  |  |

Named closed days: 3 文化の日, 23 勤労感謝の日

**December 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | 5 | (6) |
| 7 | 8 | 9 | 10 | 11 | 12 | (13) |
| 14 | 15 | 16 | 17 | 18 | 19 | (20) |
| 21 | 22 | 23 | 24 | 25 | 26 | (27) |
| 28 | 29 | 30 | 31 |  |  |  |

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | (1) | 2 | (3) |
| 4 | 5 | 6 | 7 | 8 | 9 | (10) |
| (11) | 12 | 13 | 14 | 15 | 16 | (17) |
| 18 | 19 | 20 | 21 | 22 | 23 | (24) |
| 25 | 26 | 27 | 28 | 29 | 30 | (31) |

Named closed days: 1 元日, 11 成人の日

**February 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | 6 | (7) |
| 8 | 9 | 10 | (11) | 12 | 13 | (14) |
| 15 | 16 | 17 | 18 | 19 | 20 | (21) |
| 22 | (23) | 24 | 25 | 26 | 27 | (28) |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | 6 | (7) |
| 8 | 9 | 10 | 11 | 12 | 13 | (14) |
| 15 | 16 | 17 | 18 | 19 | 20 | (21) |
| (22) | 23 | 24 | 25 | 26 | 27 | (28) |
| 29 | 30 | 31 |  |  |  |  |

Named closed days: 21 春分の日, 22 休日

**April 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | 3 | (4) |
| 5 | 6 | 7 | 8 | 9 | 10 | (11) |
| 12 | 13 | 14 | 15 | 16 | 17 | (18) |
| 19 | 20 | 21 | 22 | 23 | 24 | (25) |
| 26 | 27 | 28 | (29) | 30 |  |  |

Named closed days: 29 昭和の日

**May 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | 1 | (2) |
| (3) | (4) | (5) | 6 | 7 | 8 | (9) |
| 10 | 11 | 12 | 13 | 14 | 15 | (16) |
| 17 | 18 | 19 | 20 | 21 | 22 | (23) |
| 24 | 25 | 26 | 27 | 28 | 29 | (30) |
| 31 |  |  |  |  |  |  |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日

**June 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | 2 | 3 | 4 | 5 | (6) |
| 7 | 8 | 9 | 10 | 11 | 12 | (13) |
| 14 | 15 | 16 | 17 | 18 | 19 | (20) |
| 21 | 22 | 23 | 24 | 25 | 26 | (27) |
| 28 | 29 | 30 |  |  |  |  |

**July 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | 2 | 3 | (4) |
| 5 | 6 | 7 | 8 | 9 | 10 | (11) |
| 12 | 13 | 14 | 15 | 16 | 17 | (18) |
| (19) | 20 | 21 | 22 | 23 | 24 | (25) |
| 26 | 27 | 28 | 29 | 30 | 31 |  |

Named closed days: 19 海の日

**August 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | 7 | (8) |
| 9 | 10 | (11) | 12 | 13 | 14 | (15) |
| 16 | 17 | 18 | 19 | 20 | 21 | (22) |
| 23 | 24 | 25 | 26 | 27 | 28 | (29) |
| 30 | 31 |  |  |  |  |  |

Named closed days: 11 山の日

**September 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | 4 | (5) |
| 6 | 7 | 8 | 9 | 10 | 11 | (12) |
| 13 | 14 | 15 | 16 | 17 | 18 | (19) |
| (20) | 21 | 22 | (23) | 24 | 25 | (26) |
| 27 | 28 | 29 | 30 |  |  |  |

Named closed days: 20 敬老の日, 23 秋分の日

**October 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | 2 | (3) |
| 4 | 5 | 6 | 7 | 8 | 9 | (10) |
| (11) | 12 | 13 | 14 | 15 | 16 | (17) |
| 18 | 19 | 20 | 21 | 22 | 23 | (24) |
| 25 | 26 | 27 | 28 | 29 | 30 | (31) |

Named closed days: 11 スポーツの日

**November 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | (3) | 4 | 5 | 6 | (7) |
| 8 | 9 | 10 | 11 | 12 | 13 | (14) |
| 15 | 16 | 17 | 18 | 19 | 20 | (21) |
| 22 | (23) | 24 | 25 | 26 | 27 | (28) |
| 29 | 30 |  |  |  |  |  |

Named closed days: 3 文化の日, 23 勤労感謝の日

**December 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | 4 | (5) |
| 6 | 7 | 8 | 9 | 10 | 11 | (12) |
| 13 | 14 | 15 | 16 | 17 | 18 | (19) |
| 20 | 21 | 22 | 23 | 24 | 25 | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |
