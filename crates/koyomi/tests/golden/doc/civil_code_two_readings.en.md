# civil_code_two_readings v1

Breaks its claims on purpose. It writes the day after of Article 142 of the Civil Code in two ways, as the very next day and as the day the days off end, and lists the inputs they part on; and it holds Article 143 against adding the months and rounding down to the end of the month. It does not choose a reading. The English version of civil_code_two_readings.ja.cal

- File: `civil_code_two_readings.cal` (dates civil_code_two_readings v1, sha256:51e1c96cb2705bf6)
- Calendar: `calendars/civil_code_142_days.cal` (calendar civil_code_142_days v1, sha256:dd534a7343411409)
- Table: national_holidays = `calendars/data/syukujitsu.csv` (sha256:cec37a743c96995c, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, covers 1955-01-01..2027-12-31)
- Law: civil_code = law 129AC0000000089 on e-Gov as of 2026-10-01 (revision 129AC0000000089_20260624_508AC0000000045): 第140条 sha256:e880059021fbb67d, 第141条 sha256:0575c131b9f08063, 第142条 sha256:fc8c35a0769d3b35, 第143条 sha256:6950bdfb988439b6
- koyomi: 0.24.0

koyomi 0.24.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!WARNING]
> The claim the_two_readings_agree fails for 121 of the 4,380 combinations of origin and month_count.
>
> The claim rounding_to_the_month_end fails for 39 of the 4,380 combinations of origin and month_count.
>
> The input days it fails on are in bold in the month tables below.

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

### next_day

From last_day, in this order:

1. If it is closed, the day after (once, whatever that day is) `if closed + 1 day  # the first reading: the day after is the very next day`

   The day is closed, and the line acts, on 843 of the 4,380 input combinations.

> **civil_code 第142条 (e-Gov, as of 2026-10-01, revision 129AC0000000089_20260624_508AC0000000045)**
>
> 第百四十二条　期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。

### next_business_day

From last_day, in this order:

1. If it is closed, the next business day `roll following  # the second reading: on to the day the days off end`

   The day is closed, and moves, on 843 of the 4,380 input combinations; by 4 days at most.

> **civil_code 第142条 (e-Gov, as of 2026-10-01, revision 129AC0000000089_20260624_508AC0000000045)**
>
> 第百四十二条　期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。

### months_added

From origin, in this order:

1. The same day month_count months later; the end of that month when it has no such day `+ month_count months else end_of_month  # another way to write it than Article 143: add the months to the origin, rounding down to the end of the month`

## What was checked

Every date was computed, and every claim checked, on all 4,380 combinations of origin (2026-01-01..2026-12-31) and month_count (1..12).

| Claim | As written | Result |
|---|---|---|
| the_two_readings_agree | `next_day = next_business_day` | **fails** on 121 of the 4,380 input combinations. The farthest is at origin 2026-04-03, month_count 1, where next_day 2026-05-04 is 3 days before next_business_day |
| rounding_to_the_month_end | `last_day = months_added` | **fails** on 39 of the 4,380 input combinations. The farthest is at origin 2026-02-28, month_count 1, where last_day 2026-03-31 is 3 days after months_added |

### Where the_two_readings_agree fails

The inputs it fails on, by the values of the integer inputs:

- month_count 1, 11 inputs: 2026-01-22, 2026-04-03..2026-04-05 (3 days), 2026-06-19, 2026-08-20..2026-08-22 (3 days), 2026-09-11, 2026-10-22, 2026-12-10
- month_count 2, 10 inputs: 2026-03-03..2026-03-05 (3 days), 2026-05-19, 2026-07-20..2026-07-22 (3 days), 2026-08-11, 2026-09-22, 2026-11-10
- month_count 3, 11 inputs: 2026-02-03..2026-02-05 (3 days), 2026-04-19, 2026-06-20..2026-06-22 (3 days), 2026-07-11, 2026-08-22, 2026-10-10, 2026-12-21
- month_count 4, 11 inputs: 2026-01-03..2026-01-05 (3 days), 2026-03-19, 2026-05-20..2026-05-22 (3 days), 2026-06-11, 2026-07-22, 2026-09-10, 2026-11-21
- month_count 5, 11 inputs: 2026-02-19, 2026-04-20..2026-04-22 (3 days), 2026-05-11, 2026-06-22, 2026-08-10, 2026-10-21, 2026-12-02..2026-12-04 (3 days)
- month_count 6, 11 inputs: 2026-01-19, 2026-03-20..2026-03-22 (3 days), 2026-04-11, 2026-05-22, 2026-07-10, 2026-09-21, 2026-11-02..2026-11-04 (3 days)
- month_count 7, 11 inputs: 2026-02-20..2026-02-22 (3 days), 2026-03-11, 2026-04-22, 2026-06-10, 2026-08-21, 2026-10-02..2026-10-04 (3 days), 2026-12-18
- month_count 8, 11 inputs: 2026-01-20..2026-01-22 (3 days), 2026-02-11, 2026-03-22, 2026-05-10, 2026-07-21, 2026-09-02..2026-09-04 (3 days), 2026-11-18
- month_count 9, 9 inputs: 2026-01-11, 2026-02-22, 2026-04-10, 2026-06-21, 2026-08-02..2026-08-04 (3 days), 2026-10-18, 2026-12-19
- month_count 10, 9 inputs: 2026-01-22, 2026-03-10, 2026-05-21, 2026-07-02..2026-07-04 (3 days), 2026-09-18, 2026-11-19, 2026-12-10
- month_count 11, 8 inputs: 2026-02-10, 2026-04-21, 2026-06-02..2026-06-04 (3 days), 2026-08-18, 2026-10-19, 2026-11-10
- month_count 12, 8 inputs: 2026-01-10, 2026-03-21, 2026-05-02..2026-05-04 (3 days), 2026-07-18, 2026-09-19, 2026-10-10

The first input it fails on:

```text
origin             2026-01-22 Thu
                   month_count = 1
first_day          2026-01-23 Fri  + 1 day
last_day           2026-02-23 Mon  + month_count months else start_of_next_month (month_count = 1)
                   2026-02-22 Sun  - 1 day
next_day           2026-02-23 Mon  if closed + 1 day: 2026-02-22 (Sunday) is closed
next_business_day  2026-02-24 Tue  roll following: 2026-02-22 (Sunday) and 2026-02-23 (天皇誕生日) are closed
                   next_day is 1 day before next_business_day
```

### Where rounding_to_the_month_end fails

The inputs it fails on, by the values of the integer inputs:

- month_count 1, 5 inputs: 2026-02-28, 2026-04-30, 2026-06-30, 2026-09-30, 2026-11-30
- month_count 2, 3 inputs: 2026-02-28, 2026-06-30, 2026-11-30
- month_count 3, 3 inputs: 2026-02-28, 2026-04-30, 2026-09-30
- month_count 4, 5 inputs: 2026-02-28, 2026-04-30, 2026-06-30, 2026-09-30, 2026-11-30
- month_count 5, 1 input: 2026-02-28
- month_count 6, 5 inputs: 2026-02-28, 2026-04-30, 2026-06-30, 2026-09-30, 2026-11-30
- month_count 7, 2 inputs: 2026-02-28, 2026-06-30
- month_count 8, 4 inputs: 2026-02-28, 2026-04-30, 2026-09-30, 2026-11-30
- month_count 9, 4 inputs: 2026-02-28, 2026-04-30, 2026-06-30, 2026-11-30
- month_count 10, 2 inputs: 2026-02-28, 2026-09-30
- month_count 11, 5 inputs: 2026-02-28, 2026-04-30, 2026-06-30, 2026-09-30, 2026-11-30

The first input it fails on:

```text
origin        2026-02-28 Sat
              month_count = 1
first_day     2026-03-01 Sun  + 1 day
last_day      2026-04-01 Wed  + month_count months else start_of_next_month (month_count = 1)
              2026-03-31 Tue  - 1 day
months_added  2026-03-28 Sat  + month_count months else end_of_month (month_count = 1)
              last_day is 3 days after months_added
```

## Days a month does not have

Adding months, taking a day of a month some months away, and closing on a day of the month can land on a day the month does not have, such as 30 February. The .cal says what to do then. For every input of the range, koyomi counted how often that is used, and how often another way would change the result.

- `+ month_count months else start_of_next_month` (last_day)

  It lands on a day the month does not have on 57 of the 4,380 input combinations, and `else start_of_next_month` is used. The first is origin 2026-01-28, month_count 1: 2026-02-29 does not exist, and 2026-03-01 is taken.

  With `else end_of_month` instead, last_day would differ on 57 of the 4,380 input combinations. With `else reject` instead, 57 of the 4,380 input combinations would be refused.

- `+ month_count months else end_of_month` (months_added)

  It lands on a day the month does not have on 57 of the 4,380 input combinations, and `else end_of_month` is used. The first is origin 2026-01-29, month_count 1: 2026-02-29 does not exist, and 2026-02-28 is taken.

  With `else start_of_next_month` instead, months_added would differ on 57 of the 4,380 input combinations. With `else reject` instead, 57 of the 4,380 input combinations would be refused.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| origin | month_count | first_day | last_day | next_day | next_business_day | months_added | Why |
|---|---|---|---|---|---|---|---|
| 2026-01-01 Thu | 1 | 2026-01-02 Fri | 2026-02-01 Sun | 2026-02-02 Mon | 2026-02-02 Mon | 2026-02-01 Sun | next_day falls on a Sunday, and `if closed + 1 day` acts; next_business_day falls on a Sunday and moves to the next business day; origin is 元日, a closed day |
| 2026-01-02 Fri | 1 | 2026-01-03 Sat | 2026-02-02 Mon | 2026-02-02 Mon | 2026-02-02 Mon | 2026-02-02 Mon | origin is the day after 元日 |
| 2026-01-11 Sun | 1 | 2026-01-12 Mon | 2026-02-11 Wed | 2026-02-12 Thu | 2026-02-12 Thu | 2026-02-11 Wed | next_business_day falls on 建国記念の日 and moves to the next business day |
| 2026-01-22 Thu | 1 | 2026-01-23 Fri | 2026-02-22 Sun | 2026-02-23 Mon | 2026-02-24 Tue | 2026-02-22 Sun | the claim the_two_readings_agree fails |
| 2026-01-23 Fri | 1 | 2026-01-24 Sat | 2026-02-23 Mon | 2026-02-24 Tue | 2026-02-24 Tue | 2026-02-23 Mon | next_business_day falls on 天皇誕生日 and moves to the next business day |
| 2026-01-28 Wed | 1 | 2026-01-29 Thu | 2026-02-28 Sat | 2026-02-28 Sat | 2026-02-28 Sat | 2026-02-28 Sat | computing last_day lands on 2026-02-29, which does not exist, and uses `else start_of_next_month` |
| 2026-01-29 Thu | 1 | 2026-01-30 Fri | 2026-02-28 Sat | 2026-02-28 Sat | 2026-02-28 Sat | 2026-02-28 Sat | computing months_added lands on 2026-02-29, which does not exist, and uses `else end_of_month` |
| 2026-01-31 Sat | 1 | 2026-02-01 Sun | 2026-02-28 Sat | 2026-02-28 Sat | 2026-02-28 Sat | 2026-02-28 Sat | the fewest days from origin to next_day, next_business_day and months_added (28); origin is the 31st, the end of its month |
| 2026-02-10 Tue | 1 | 2026-02-11 Wed | 2026-03-10 Tue | 2026-03-10 Tue | 2026-03-10 Tue | 2026-03-10 Tue | origin is the day before 建国記念の日 |
| 2026-02-20 Fri | 1 | 2026-02-21 Sat | 2026-03-20 Fri | 2026-03-21 Sat | 2026-03-21 Sat | 2026-03-20 Fri | next_business_day falls on 春分の日 and moves to the next business day |
| 2026-02-28 Sat | 1 | 2026-03-01 Sun | 2026-03-31 Tue | 2026-03-31 Tue | 2026-03-31 Tue | 2026-03-28 Sat | the claim rounding_to_the_month_end fails; origin is 28 February, the end of the month in a year that is not a leap year |
| 2026-03-29 Sun | 1 | 2026-03-30 Mon | 2026-04-29 Wed | 2026-04-30 Thu | 2026-04-30 Thu | 2026-04-29 Wed | next_business_day falls on 昭和の日 and moves to the next business day |
| 2026-04-03 Fri | 1 | 2026-04-04 Sat | 2026-05-03 Sun | 2026-05-04 Mon | 2026-05-07 Thu | 2026-05-03 Sun | next_business_day moves the most (4 days) |
| 2026-04-04 Sat | 1 | 2026-04-05 Sun | 2026-05-04 Mon | 2026-05-05 Tue | 2026-05-07 Thu | 2026-05-04 Mon | next_business_day falls on みどりの日 and moves to the next business day |
| 2026-01-01 Thu | 12 | 2026-01-02 Fri | 2027-01-01 Fri | 2027-01-02 Sat | 2027-01-02 Sat | 2027-01-01 Fri | the most days from origin to next_day (366); the most days from origin to months_added (365) |
| 2026-05-02 Sat | 12 | 2026-05-03 Sun | 2027-05-02 Sun | 2027-05-03 Mon | 2027-05-06 Thu | 2027-05-02 Sun | the most days from origin to next_business_day (369) |

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

In the tables, a day in parentheses is closed; a day in bold is an input a claim fails on; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | (1)◆ | 2◆ | **3** | **(4)** |
| **5** | 6 | 7 | 8 | 9 | **10** | **(11)**◆ |
| (12) | 13 | 14 | 15 | 16 | 17 | (18) |
| **19** | **20** | **21** | **22**◆ | 23◆ | 24 | (25) |
| 26 | 27 | 28◆ | 29◆ | 30 | 31◆ |  |

Named closed days: 1 元日, 12 成人の日

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | **3** | **4** | **5** | 6 | 7 | (8) |
| 9 | **10**◆ | **(11)** | 12 | 13 | 14 | (15) |
| 16 | 17 | 18 | **19** | **20**◆ | **21** | **(22)** |
| (23) | 24 | 25 | 26 | 27 | **28**◆ |  |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| 2 | **3** | **4** | **5** | 6 | 7 | (8) |
| 9 | **10** | **11** | 12 | 13 | 14 | (15) |
| 16 | 17 | 18 | **19** | **(20)** | **21** | **(22)** |
| 23 | 24 | 25 | 26 | 27 | 28 | (29)◆ |
| 30 | 31 |  |  |  |  |  |

Named closed days: 20 春分の日

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | **3**◆ | **4**◆ | **(5)** |
| 6 | 7 | 8 | 9 | **10** | **11** | (12) |
| 13 | 14 | 15 | 16 | 17 | 18 | **(19)** |
| **20** | **21** | **22** | 23 | 24 | 25 | (26) |
| 27 | 28 | (29) | **30** |  |  |  |

Named closed days: 29 昭和の日

**May 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | **2**◆ | **(3)** |
| **(4)** | (5) | (6) | 7 | 8 | 9 | **(10)** |
| **11** | 12 | 13 | 14 | 15 | 16 | (17) |
| 18 | **19** | **20** | **21** | **22** | 23 | (24) |
| 25 | 26 | 27 | 28 | 29 | 30 | (31) |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日, 6 休日

**June 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | **2** | **3** | **4** | 5 | 6 | (7) |
| 8 | 9 | **10** | **11** | 12 | 13 | (14) |
| 15 | 16 | 17 | 18 | **19** | **20** | **(21)** |
| **22** | 23 | 24 | 25 | 26 | 27 | (28) |
| 29 | **30** |  |  |  |  |  |

**July 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | **2** | **3** | **4** | (5) |
| 6 | 7 | 8 | 9 | **10** | **11** | (12) |
| 13 | 14 | 15 | 16 | 17 | **18** | (19) |
| **(20)** | **21** | **22** | 23 | 24 | 25 | (26) |
| 27 | 28 | 29 | 30 | 31 |  |  |

Named closed days: 20 海の日

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | 1 | **(2)** |
| **3** | **4** | 5 | 6 | 7 | 8 | (9) |
| **10** | **(11)** | 12 | 13 | 14 | 15 | (16) |
| 17 | **18** | 19 | **20** | **21** | **22** | (23) |
| 24 | 25 | 26 | 27 | 28 | 29 | (30) |
| 31 |  |  |  |  |  |  |

Named closed days: 11 山の日

**September 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | **2** | **3** | **4** | 5 | (6) |
| 7 | 8 | 9 | **10** | **11** | 12 | (13) |
| 14 | 15 | 16 | 17 | **18** | **19** | (20) |
| **(21)** | **(22)** | (23) | 24 | 25 | 26 | (27) |
| 28 | 29 | **30** |  |  |  |  |

Named closed days: 21 敬老の日, 22 休日, 23 秋分の日

**October 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | 1 | **2** | **3** | **(4)** |
| 5 | 6 | 7 | 8 | 9 | **10** | (11) |
| (12) | 13 | 14 | 15 | 16 | 17 | **(18)** |
| **19** | 20 | **21** | **22** | 23 | 24 | (25) |
| 26 | 27 | 28 | 29 | 30 | 31 |  |

Named closed days: 12 スポーツの日

**November 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | (1) |
| **2** | **(3)** | **4** | 5 | 6 | 7 | (8) |
| 9 | **10** | 11 | 12 | 13 | 14 | (15) |
| 16 | 17 | **18** | **19** | 20 | **21** | (22) |
| (23) | 24 | 25 | 26 | 27 | 28 | (29) |
| **30** |  |  |  |  |  |  |

Named closed days: 3 文化の日, 23 勤労感謝の日

**December 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | 1 | **2** | **3** | **4** | 5 | (6) |
| 7 | 8 | 9 | **10** | 11 | 12 | (13) |
| 14 | 15 | 16 | 17 | **18** | **19** | (20) |
| **21** | 22 | 23 | 24 | 25 | 26 | (27) |
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
