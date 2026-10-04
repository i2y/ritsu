# england_and_wales v1

Saturdays, Sundays and the bank holidays of England and Wales, as GOV.UK lists them. No offset: England and Wales has daylight saving time, so this calendar gives dates only

- File: `england_and_wales.cal` (calendar england_and_wales v1, sha256:00a0d8a87344f4f4)
- Table: bank_holidays = `data/bank-holidays.json` (sha256:538b3482c28b85ec, a copy of https://www.gov.uk/bank-holidays.json, covers listed years = 2019-01-01..2028-12-31)
- koyomi: 0.23.0

koyomi 0.23.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

## Closed days

These days are closed.

- every Saturday and Sunday `closed weekly sat, sun`
- the days the table bank_holidays lists `closed bank_holidays`

## Sources and the days the calendar knows

- bank_holidays: 83 rows, a copy of https://www.gov.uk/bank-holidays.json, pinned at sha256:538b3482c28b85ec; it lists every closed day from 2019-01-01 to 2028-12-31 (from the first year with a row to the last).

The calendar knows 2019-01-01..2028-12-31. A computation that asks whether a day outside it is a business day stops, in the check and in the generated code alike.

## Business days

| Year | Business days | Closed days |
|---|---|---|
| 2027 | 253 | 112 |
| 2028 | 252 | 114 |

The longest run of closed days in 2027-01..2028-12 is 2027-03-26..2027-03-29, 4 days.

## Month by month

In the tables, a day in parentheses is closed.

**January 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | (1) | (2) | (3) |
| 4 | 5 | 6 | 7 | 8 | (9) | (10) |
| 11 | 12 | 13 | 14 | 15 | (16) | (17) |
| 18 | 19 | 20 | 21 | 22 | (23) | (24) |
| 25 | 26 | 27 | 28 | 29 | (30) | (31) |

Named closed days: 1 New Year’s Day

**February 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | 26 | (27) | (28) |

**March 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 3 | 4 | 5 | (6) | (7) |
| 8 | 9 | 10 | 11 | 12 | (13) | (14) |
| 15 | 16 | 17 | 18 | 19 | (20) | (21) |
| 22 | 23 | 24 | 25 | (26) | (27) | (28) |
| (29) | 30 | 31 |  |  |  |  |

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
|  |  |  |  |  | (1) | (2) |
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
| 2 | 3 | 4 | 5 | 6 | (7) | (8) |
| 9 | 10 | 11 | 12 | 13 | (14) | (15) |
| 16 | 17 | 18 | 19 | 20 | (21) | (22) |
| 23 | 24 | 25 | 26 | 27 | (28) | (29) |
| (30) | 31 |  |  |  |  |  |

Named closed days: 30 Summer bank holiday

**September 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | 1 | 2 | 3 | (4) | (5) |
| 6 | 7 | 8 | 9 | 10 | (11) | (12) |
| 13 | 14 | 15 | 16 | 17 | (18) | (19) |
| 20 | 21 | 22 | 23 | 24 | (25) | (26) |
| 27 | 28 | 29 | 30 |  |  |  |

**October 2027**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | 1 | (2) | (3) |
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
|  | 1 | 2 | 3 | 4 | (5) | (6) |
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
