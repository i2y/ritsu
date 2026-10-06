# eom_close_two_months_later v1

Closes at the end of the month; pays at the end of the month two months later, or on the business day before when that day is closed. The English version of eom_close_two_months_later.ja.cal. It breaks the claim within_60_days_of_receipt on purpose: the claim is this example's own, written as it reads, and does not say how any law is read

- File: `eom_close_two_months_later.cal` (dates eom_close_two_months_later v1, sha256:0e16bdf8d3db5f22)
- Calendar: `calendars/tokyo_business_days.cal` (calendar tokyo_business_days v1, sha256:37af228cf6ba7b95)
- Table: national_holidays = `calendars/data/syukujitsu.csv` (sha256:cec37a743c96995c, a copy of https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv, covers 1955-01-01..2027-12-31)
- koyomi: 0.24.0

koyomi 0.24.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!WARNING]
> The claim within_60_days_of_receipt fails for 648 of the 669 days of received.
>
> The input days it fails on are in bold in the month tables below.

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### closing

From received, in this order:

1. The end of its month `close end of month  # "closes at the end of the month"`

### payment

From closing, in this order:

1. The end of the month 2 months later `end of month +2  # "pays at the end of the month two months later"`
2. If it is closed, the business day before `roll preceding  # "on the business day before when that day is closed"`

   The day is closed, and moves, on 247 of the 669 days of received; by 3 days at most.

## What was checked

Every date was computed, and every claim checked, on all 669 days of received (2026-01-01..2027-10-31).

| Claim | As written | Result |
|---|---|---|
| within_60_days_of_receipt | `payment <= received + 60 days` | **fails** on 648 of the 669 days of received. The farthest is at received 2026-05-01, where payment 2026-07-31 is 91 days after received (the claim allows at most 60 days after) |

### Where within_60_days_of_receipt fails

The days it fails on: 2026-01-01..2026-01-29 (29 days), 2026-02-01..2026-03-29 (57 days), 2026-04-01..2026-08-30 (152 days), 2026-09-01..2026-10-28 (58 days), 2026-11-01..2026-11-29 (29 days), 2026-12-01..2026-12-27 (27 days), 2027-01-01..2027-01-29 (29 days), 2027-02-01..2027-05-30 (119 days), 2027-06-01..2027-08-29 (90 days), 2027-09-01..2027-10-28 (58 days)

The first input it fails on:

```text
received  2026-01-01 Thu
closing   2026-01-31 Sat  close end of month
payment   2026-03-31 Tue  end of month +2
          2026-03-31 Tue  roll preceding: a business day, stays
          payment is 89 days after received, and the claim allows at most 60 days after
```

## Days a month does not have

No operation of this file can land on a day its month does not have, such as 30 February.

## Edge cases

Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says.

| received | closing | payment | Why |
|---|---|---|---|
| 2026-01-01 Thu | 2026-01-31 Sat | 2026-03-31 Tue | the claim within_60_days_of_receipt fails; received is 元日, a closed day; received falls in New Year holidays |
| 2026-01-13 Tue | 2026-01-31 Sat | 2026-03-31 Tue | received is the day after 成人の日 |
| 2026-01-31 Sat | 2026-01-31 Sat | 2026-03-31 Tue | received is the 31st, the end of its month |
| 2026-02-10 Tue | 2026-02-28 Sat | 2026-04-30 Thu | received is the day before 建国記念の日 |
| 2026-02-28 Sat | 2026-02-28 Sat | 2026-04-30 Thu | received is 28 February, the end of the month in a year that is not a leap year |
| 2026-03-01 Sun | 2026-03-31 Tue | 2026-05-29 Fri | payment falls on a Sunday and moves to the business day before |
| 2026-04-30 Thu | 2026-04-30 Thu | 2026-06-30 Tue | received is the 30th, the end of its month |
| 2026-05-01 Fri | 2026-05-31 Sun | 2026-07-31 Fri | the most days from received to payment (91) |
| 2026-08-01 Sat | 2026-08-31 Mon | 2026-10-30 Fri | payment falls on a Saturday and moves to the business day before |
| 2026-10-01 Thu | 2026-10-31 Sat | 2026-12-28 Mon | payment falls on New Year holidays and moves to the business day before; payment moves the most (3 days) |
| 2026-12-31 Thu | 2026-12-31 Thu | 2027-02-26 Fri | the fewest days from received to payment (57) |

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

In the tables, a day in parentheses is closed; a day in bold is an input a claim fails on; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **(1)**◆ | **(2)** | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **(12)** | **13**◆ | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **29** | 30 | (31)◆ |  |

Named closed days: 1 元日 and New Year holidays, 2–3 New Year holidays, 12 成人の日

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10**◆ | **(11)** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **(23)** | **24** | **25** | **26** | **27** | **(28)**◆ |  |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)**◆ |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **(20)** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)** | **(29)** |
| 30 | 31 |  |  |  |  |  |

Named closed days: 20 春分の日

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2** | **3** | **(4)** | **(5)** |
| **6** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **27** | **28** | **(29)** | **30**◆ |  |  |  |

Named closed days: 29 昭和の日

**May 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **1**◆ | **(2)** | **(3)** |
| **(4)** | **(5)** | **(6)** | **7** | **8** | **(9)** | **(10)** |
| **11** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **25** | **26** | **27** | **28** | **29** | **(30)** | **(31)** |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日, 6 休日

**June 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **1** | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **11** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **22** | **23** | **24** | **25** | **26** | **(27)** | **(28)** |
| **29** | **30** |  |  |  |  |  |

**July 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2** | **3** | **(4)** | **(5)** |
| **6** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **(20)** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **27** | **28** | **29** | **30** | **31** |  |  |

Named closed days: 20 海の日

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)**◆ | **(2)** |
| **3** | **4** | **5** | **6** | **7** | **(8)** | **(9)** |
| **10** | **(11)** | **12** | **13** | **14** | **(15)** | **(16)** |
| **17** | **18** | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |
| 31 |  |  |  |  |  |  |

Named closed days: 11 山の日

**September 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **(21)** | **(22)** | **(23)** | **24** | **25** | **(26)** | **(27)** |
| **28** | **29** | **30** |  |  |  |  |

Named closed days: 21 敬老の日, 22 休日, 23 秋分の日

**October 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **1**◆ | **2** | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **(12)** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | 29 | 30 | (31) |  |

Named closed days: 12 スポーツの日

**November 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **(3)** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **(23)** | **24** | **25** | **26** | **27** | **(28)** | **(29)** |
| 30 |  |  |  |  |  |  |

Named closed days: 3 文化の日, 23 勤労感謝の日

**December 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **21** | **22** | **23** | **24** | **25** | **(26)** | **(27)** |
| 28 | (29) | (30) | (31)◆ |  |  |  |

Named closed days: 29–31 New Year holidays

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **(1)** | **(2)** | **(3)** |
| **4** | **5** | **6** | **7** | **8** | **(9)** | **(10)** |
| **(11)** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **25** | **26** | **27** | **28** | **29** | (30) | (31) |

Named closed days: 1 元日 and New Year holidays, 2–3 New Year holidays, 11 成人の日

**February 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **1** | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **(11)** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **22** | **(23)** | **24** | **25** | **26** | **(27)** | **(28)** |

Named closed days: 11 建国記念の日, 23 天皇誕生日

**March 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **1** | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **11** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **(22)** | **23** | **24** | **25** | **26** | **(27)** | **(28)** |
| **29** | **30** | **31** |  |  |  |  |

Named closed days: 21 春分の日, 22 休日

**April 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **1** | **2** | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **12** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **(29)** | **30** |  |  |

Named closed days: 29 昭和の日

**May 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)** | **(2)** |
| **(3)** | **(4)** | **(5)** | **6** | **7** | **(8)** | **(9)** |
| **10** | **11** | **12** | **13** | **14** | **(15)** | **(16)** |
| **17** | **18** | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |
| 31 |  |  |  |  |  |  |

Named closed days: 3 憲法記念日, 4 みどりの日, 5 こどもの日

**June 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **21** | **22** | **23** | **24** | **25** | **(26)** | **(27)** |
| **28** | **29** | **30** |  |  |  |  |

**July 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **1** | **2** | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **12** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **(19)** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **29** | **30** | **(31)** |  |

Named closed days: 19 海の日

**August 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **(11)** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)** | **(29)** |
| 30 | 31 |  |  |  |  |  |

Named closed days: 11 山の日

**September 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2** | **3** | **(4)** | **(5)** |
| **6** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **(20)** | **21** | **22** | **(23)** | **24** | **(25)** | **(26)** |
| **27** | **28** | **29** | **30** |  |  |  |

Named closed days: 20 敬老の日, 23 秋分の日

**October 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **1** | **(2)** | **(3)** |
| **4** | **5** | **6** | **7** | **8** | **(9)** | **(10)** |
| **(11)** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **25** | **26** | **27** | **28** | 29 | (30) | (31) |

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
