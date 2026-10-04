# close_eom_pay_two_months_on v1

Closes at the end of the month; pays at the end of the month two months later, or on the business day before when that day is closed in England and Wales. It breaks the claim within_60_days_of_receipt on purpose: the claim is this example's own, written as it reads, and does not say how any law is read

- File: `close_eom_pay_two_months_on.cal` (dates close_eom_pay_two_months_on v1, sha256:d79271982e9e1200)
- Calendar: `calendars/england_and_wales.cal` (calendar england_and_wales v1, sha256:00a0d8a87344f4f4)
- Table: bank_holidays = `calendars/data/bank-holidays.json` (sha256:538b3482c28b85ec, a copy of https://www.gov.uk/bank-holidays.json, covers listed years = 2019-01-01..2028-12-31)
- koyomi: 0.1.0

koyomi 0.1.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!WARNING]
> The claim within_60_days_of_receipt fails for 1,008 of the 1,035 days of received.
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

   The day is closed, and moves, on 337 of the 1,035 days of received; by 3 days at most.

## What was checked

Every date was computed, and every claim checked, on all 1,035 days of received (2026-01-01..2028-10-31).

| Claim | As written | Result |
|---|---|---|
| within_60_days_of_receipt | `payment <= received + 60 days` | **fails** on 1,008 of the 1,035 days of received. The farthest is at received 2026-05-01, where payment 2026-07-31 is 91 days after received (the claim allows at most 60 days after) |

### Where within_60_days_of_receipt fails

The days it fails on: 2026-01-01..2026-01-29 (29 days), 2026-02-01..2026-03-29 (57 days), 2026-04-01..2026-06-28 (89 days), 2026-07-01..2026-08-30 (61 days), 2026-09-01..2026-11-29 (90 days), 2026-12-01..2026-12-27 (27 days), 2027-01-01..2027-01-29 (29 days), 2027-02-01..2027-03-28 (56 days), 2027-04-01..2027-05-30 (60 days), 2027-06-01..2027-08-29 (90 days), 2027-09-01..2027-12-30 (121 days), 2028-01-01..2028-01-30 (30 days), 2028-02-01..2028-02-27 (27 days), 2028-03-01..2028-07-30 (152 days), 2028-08-01..2028-10-29 (90 days)

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
| 2026-01-01 Thu | 2026-01-31 Sat | 2026-03-31 Tue | the claim within_60_days_of_receipt fails; received is New Year’s Day, a closed day |
| 2026-01-02 Fri | 2026-01-31 Sat | 2026-03-31 Tue | received is the day after New Year’s Day |
| 2026-01-31 Sat | 2026-01-31 Sat | 2026-03-31 Tue | received is the 31st, the end of its month |
| 2026-02-28 Sat | 2026-02-28 Sat | 2026-04-30 Thu | received is 28 February, the end of the month in a year that is not a leap year |
| 2026-03-01 Sun | 2026-03-31 Tue | 2026-05-29 Fri | payment falls on a Sunday and moves to the business day before |
| 2026-04-02 Thu | 2026-04-30 Thu | 2026-06-30 Tue | received is the day before Good Friday |
| 2026-04-30 Thu | 2026-04-30 Thu | 2026-06-30 Tue | received is the 30th, the end of its month |
| 2026-05-01 Fri | 2026-05-31 Sun | 2026-07-31 Fri | the most days from received to payment (91) |
| 2026-06-01 Mon | 2026-06-30 Tue | 2026-08-28 Fri | payment falls on Summer bank holiday and moves to the business day before; payment moves the most (3 days) |
| 2026-08-01 Sat | 2026-08-31 Mon | 2026-10-30 Fri | payment falls on a Saturday and moves to the business day before |
| 2026-12-31 Thu | 2026-12-31 Thu | 2027-02-26 Fri | the fewest days from received to payment (57) |
| 2027-03-01 Mon | 2027-03-31 Wed | 2027-05-28 Fri | payment falls on Spring bank holiday and moves to the business day before |
| 2028-02-29 Tue | 2028-02-29 Tue | 2028-04-28 Fri | received is 29 February |

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

In 2026-01-01..2028-12-29, the days of received and of the dates computed from it, the longest run of closed days is 2026-04-03..2026-04-06, 4 days.

### Month by month

The inputs and the dates computed from them fall in the 36 months of 2026-01..2028-12. Only the 34 months that hold an input a claim fails on, or an edge case's input or dates, are shown; 2 are left out.

In the tables, a day in parentheses is closed; a day in bold is an input a claim fails on; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **(1)**◆ | **2**◆ | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **12** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **29** | 30 | (31)◆ |  |

Named closed days: 1 New Year’s Day

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)**◆ |  |

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)**◆ |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)** | **(29)** |
| 30 | 31 |  |  |  |  |  |

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2**◆ | **(3)** | **(4)** | **(5)** |
| **(6)** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **27** | **28** | **29** | **30**◆ |  |  |  |

Named closed days: 3 Good Friday, 6 Easter Monday

**May 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **1**◆ | **(2)** | **(3)** |
| **(4)** | **5** | **6** | **7** | **8** | **(9)** | **(10)** |
| **11** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **(25)** | **26** | **27** | **28** | **29** | **(30)** | **(31)** |

Named closed days: 4 Early May bank holiday, 25 Spring bank holiday

**June 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **1**◆ | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **11** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **22** | **23** | **24** | **25** | **26** | **(27)** | **(28)** |
| 29 | 30 |  |  |  |  |  |

**July 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2** | **3** | **(4)** | **(5)** |
| **6** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **27** | **28** | **29** | **30** | **31** |  |  |

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)**◆ | **(2)** |
| **3** | **4** | **5** | **6** | **7** | **(8)** | **(9)** |
| **10** | **11** | **12** | **13** | **14** | **(15)** | **(16)** |
| **17** | **18** | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |
| (31) |  |  |  |  |  |  |

Named closed days: 31 Summer bank holiday

**September 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **21** | **22** | **23** | **24** | **25** | **(26)** | **(27)** |
| **28** | **29** | **30** |  |  |  |  |

**October 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **1** | **2** | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **12** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **29** | **30** | **(31)** |  |

**November 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)** | **(29)** |
| 30 |  |  |  |  |  |  |

**December 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **21** | **22** | **23** | **24** | **(25)** | **(26)** | **(27)** |
| (28) | 29 | 30 | 31◆ |  |  |  |

Named closed days: 25 Christmas Day, 28 Boxing Day (Substitute day)

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **(1)** | **(2)** | **(3)** |
| **4** | **5** | **6** | **7** | **8** | **(9)** | **(10)** |
| **11** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **25** | **26** | **27** | **28** | **29** | (30) | (31) |

Named closed days: 1 New Year’s Day

**February 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **1** | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **11** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **22** | **23** | **24** | **25** | **26** | **(27)** | **(28)** |

**March 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **1**◆ | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **11** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **22** | **23** | **24** | **25** | **(26)** | **(27)** | **(28)** |
| (29) | 30 | 31 |  |  |  |  |

Named closed days: 26 Good Friday, 29 Easter Monday

**April 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **1** | **2** | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **12** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **29** | **30** |  |  |

**May 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)** | **(2)** |
| **(3)** | **4** | **5** | **6** | **7** | **(8)** | **(9)** |
| **10** | **11** | **12** | **13** | **14** | **(15)** | **(16)** |
| **17** | **18** | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |
| (31) |  |  |  |  |  |  |

Named closed days: 3 Early May bank holiday, 31 Spring bank holiday

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
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **29** | **30** | **(31)** |  |

**August 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)** | **(29)** |
| (30) | 31 |  |  |  |  |  |

Named closed days: 30 Summer bank holiday

**September 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2** | **3** | **(4)** | **(5)** |
| **6** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **27** | **28** | **29** | **30** |  |  |  |

**October 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **1** | **(2)** | **(3)** |
| **4** | **5** | **6** | **7** | **8** | **(9)** | **(10)** |
| **11** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **25** | **26** | **27** | **28** | **29** | **(30)** | **(31)** |

**November 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **1** | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **11** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **22** | **23** | **24** | **25** | **26** | **(27)** | **(28)** |
| **29** | **30** |  |  |  |  |  |

**December 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2** | **3** | **(4)** | **(5)** |
| **6** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **(27)** | **(28)** | **29** | **30** | 31 |  |  |

Named closed days: 27 Christmas Day (Substitute day), 28 Boxing Day (Substitute day)

**January 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)** | **(2)** |
| **(3)** | **4** | **5** | **6** | **7** | **(8)** | **(9)** |
| **10** | **11** | **12** | **13** | **14** | **(15)** | **(16)** |
| **17** | **18** | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |
| 31 |  |  |  |  |  |  |

Named closed days: 3 New Year’s Day (Substitute day)

**February 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **21** | **22** | **23** | **24** | **25** | **(26)** | **(27)** |
| 28 | 29◆ |  |  |  |  |  |

**March 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2** | **3** | **(4)** | **(5)** |
| **6** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **27** | **28** | **29** | **30** | **31** |  |  |

**April 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)** | **(2)** |
| **3** | **4** | **5** | **6** | **7** | **(8)** | **(9)** |
| **10** | **11** | **12** | **13** | **(14)** | **(15)** | **(16)** |
| **(17)** | **18** | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |

Named closed days: 14 Good Friday, 17 Easter Monday

**May 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| **(1)** | **2** | **3** | **4** | **5** | **(6)** | **(7)** |
| **8** | **9** | **10** | **11** | **12** | **(13)** | **(14)** |
| **15** | **16** | **17** | **18** | **19** | **(20)** | **(21)** |
| **22** | **23** | **24** | **25** | **26** | **(27)** | **(28)** |
| **(29)** | **30** | **31** |  |  |  |  |

Named closed days: 1 Early May bank holiday, 29 Spring bank holiday

**June 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **1** | **2** | **(3)** | **(4)** |
| **5** | **6** | **7** | **8** | **9** | **(10)** | **(11)** |
| **12** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28** | **29** | **30** |  |  |

**July 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)** | **(2)** |
| **3** | **4** | **5** | **6** | **7** | **(8)** | **(9)** |
| **10** | **11** | **12** | **13** | **14** | **(15)** | **(16)** |
| **17** | **18** | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |
| 31 |  |  |  |  |  |  |

**August 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **21** | **22** | **23** | **24** | **25** | **(26)** | **(27)** |
| **(28)** | **29** | **30** | **31** |  |  |  |

Named closed days: 28 Summer bank holiday

**September 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **1** | **(2)** | **(3)** |
| **4** | **5** | **6** | **7** | **8** | **(9)** | **(10)** |
| **11** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **25** | **26** | **27** | **28** | **29** | **(30)** |  |

**October 2028**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)** | **(29)** |
| 30 | 31 |  |  |  |  |  |
