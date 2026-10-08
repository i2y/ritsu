# civil_code_142_days v1

Only the days Article 142 of the Civil Code names are closed: Sundays, and the days off under the Act on National Holidays. This example does not decide what the other days off of the article are. The English version of 民法142条の休日.cal

- File: `civil_code_142_days.cal` (calendar civil_code_142_days v1, sha256:dd534a7343411409)
- Table: national_holidays = `data/syukujitsu.csv` (sha256:cec37a743c96995c, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, covers 1955-01-01..2027-12-31)
- koyomi: 0.25.0

koyomi 0.25.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

## Closed days

These days are closed.

- every Sunday `closed weekly sun`
- the days the table national_holidays lists `closed national_holidays`

## Sources and the days the calendar knows

- national_holidays: 1,067 rows, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, pinned at sha256:cec37a743c96995c; it lists every closed day from 1955-01-01 to 2027-12-31.

The calendar knows 1955-01-01..2027-12-31. A computation that asks whether a day outside it is a business day stops, in the check and in the generated code alike.

## Business days

| Year | Business days | Closed days |
|---|---|---|
| 2026 | 296 | 69 |
| 2027 | 297 | 68 |

The longest run of closed days in 2026-01..2027-12 is 2026-05-03..2026-05-06, 4 days.

## Month by month

In the tables, a day in parentheses is closed.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1) | 2 | 3 | (4) |
| 5 | 6 | 7 | 8 | 9 | 10 | (11) |
| (12) | 13 | 14 | 15 | 16 | 17 | (18) |
| 19 | 20 | 21 | 22 | 23 | 24 | (25) |
| 26 | 27 | 28 | 29 | 30 | 31 |  |

Named closed days: 1 元日, 12 成人の日

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | 3 | 4 | 5 | 6 | 7 | (8) |
| 9 | 10 | (11) | 12 | 13 | 14 | (15) |
| 16 | 17 | 18 | 19 | 20 | 21 | (22) |
| (23) | 24 | 25 | 26 | 27 | 28 |  |

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
| 27 | 28 | (29) | 30 |  |  |  |

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
