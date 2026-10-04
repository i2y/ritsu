# period_of_months_two_readings v1

Breaks its claims on purpose. It writes moving an end on a closed day to the day after in two ways, as the very next day and as the next business day, and lists the inputs they part on; and it holds the day before the corresponding day against adding the months and rounding down to the end of the month. It does not choose a reading

- File: `period_of_months_two_readings.cal` (dates period_of_months_two_readings v1, sha256:4fd356c038abcbd2)
- Calendar: `calendars/england_and_wales.cal` (calendar england_and_wales v1, sha256:00a0d8a87344f4f4)
- Table: bank_holidays = `calendars/data/bank-holidays.json` (sha256:538b3482c28b85ec, a copy of https://www.gov.uk/bank-holidays.json, covers listed years = 2019-01-01..2028-12-31)
- koyomi: 0.1.0

koyomi 0.1.0 made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date.

> [!WARNING]
> The claim the_two_readings_agree fails for 709 of the 4,380 combinations of origin and month_count.
>
> The claim rounding_to_the_month_end fails for 39 of the 4,380 combinations of origin and month_count.
>
> The input days it fails on are in bold in the month tables below.

## How the dates are computed

Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it (`#` starts a comment).

### first_day

From origin, in this order:

1. The day after `+ 1 day  # the day itself is not counted`

### last_day

From first_day, in this order:

1. The same day month_count months later; the first of the following month when it has no such day `+ month_count months else start_of_next_month  # the corresponding day of the last month; with none, the 1st of the month after`
2. The day before `- 1 day  # the day before it: the last day of the last month when it had no corresponding day`

### next_day

From last_day, in this order:

1. If it is closed, the day after (once, whatever that day is) `if closed + 1 day  # the first reading: the very next day`

   The day is closed, and the line acts, on 1,356 of the 4,380 input combinations.

### next_business_day

From last_day, in this order:

1. If it is closed, the next business day `roll following  # the second reading: on to the next business day`

   The day is closed, and moves, on 1,356 of the 4,380 input combinations; by 4 days at most.

### months_added

From origin, in this order:

1. The same day month_count months later; the end of that month when it has no such day `+ month_count months else end_of_month  # another way to write it: add the months to the day, rounding down to the end of the month`

## What was checked

Every date was computed, and every claim checked, on all 4,380 combinations of origin (2026-01-01..2026-12-31) and month_count (1..12).

| Claim | As written | Result |
|---|---|---|
| the_two_readings_agree | `next_day = next_business_day` | **fails** on 709 of the 4,380 input combinations. The farthest is at origin 2026-03-03, month_count 1, where next_day 2026-04-04 is 3 days before next_business_day |
| rounding_to_the_month_end | `last_day = months_added` | **fails** on 39 of the 4,380 input combinations. The farthest is at origin 2026-02-28, month_count 1, where last_day 2026-03-31 is 3 days after months_added |

### Where the_two_readings_agree fails

The inputs it fails on, by the values of the integer inputs:

- month_count 1, 61 inputs: 2026-01-07, 2026-01-14, 2026-01-21, 2026-01-28..2026-01-31 (4 days), 2026-02-07, 2026-02-14, 2026-02-21, 2026-03-03..2026-03-05 (3 days), 2026-03-11, 2026-03-18, 2026-03-25, 2026-04-02..2026-04-03 (2 days), 2026-04-09, 2026-04-16, 2026-04-23..2026-04-24 (2 days), 2026-05-06, 2026-05-13, 2026-05-20, 2026-05-27, 2026-06-04, 2026-06-11, 2026-06-18, 2026-06-25, 2026-07-01, 2026-07-08, 2026-07-15, 2026-07-22, 2026-07-29..2026-07-30 (2 days), 2026-08-05, 2026-08-12, 2026-08-19, 2026-08-26, 2026-09-03, 2026-09-10, 2026-09-17, 2026-09-24, 2026-09-30, 2026-10-07, 2026-10-14, 2026-10-21, 2026-10-28, 2026-11-05, 2026-11-12, 2026-11-19, 2026-11-25..2026-11-27 (3 days), 2026-12-01..2026-12-02 (2 days), 2026-12-09, 2026-12-16, 2026-12-23, 2026-12-30
- month_count 2, 58 inputs: 2026-01-07, 2026-01-14, 2026-01-21, 2026-01-28, 2026-02-03..2026-02-05 (3 days), 2026-02-11, 2026-02-18, 2026-02-25, 2026-03-02..2026-03-03 (2 days), 2026-03-09, 2026-03-16, 2026-03-23..2026-03-24 (2 days), 2026-03-30, 2026-04-06, 2026-04-13, 2026-04-20, 2026-04-27, 2026-05-04, 2026-05-11, 2026-05-18, 2026-05-25, 2026-06-01, 2026-06-08, 2026-06-15, 2026-06-22, 2026-06-29, 2026-07-05, 2026-07-12, 2026-07-19, 2026-07-26, 2026-08-03, 2026-08-10, 2026-08-17, 2026-08-24, 2026-08-31, 2026-09-07, 2026-09-14, 2026-09-21, 2026-09-28, 2026-10-05, 2026-10-12, 2026-10-19, 2026-10-25..2026-10-27 (3 days), 2026-11-01..2026-11-02 (2 days), 2026-11-09, 2026-11-16, 2026-11-23, 2026-12-06, 2026-12-13, 2026-12-20, 2026-12-27
- month_count 3, 61 inputs: 2026-01-03..2026-01-05 (3 days), 2026-01-11, 2026-01-18, 2026-01-25, 2026-02-02..2026-02-03 (2 days), 2026-02-09, 2026-02-16, 2026-02-23..2026-02-24 (2 days), 2026-03-06, 2026-03-13, 2026-03-20, 2026-03-27, 2026-04-04, 2026-04-11, 2026-04-18, 2026-04-25, 2026-05-01, 2026-05-08, 2026-05-15, 2026-05-22, 2026-05-29..2026-05-30 (2 days), 2026-06-05, 2026-06-12, 2026-06-19, 2026-06-26, 2026-07-03, 2026-07-10, 2026-07-17, 2026-07-24, 2026-07-31, 2026-08-07, 2026-08-14, 2026-08-21, 2026-08-28, 2026-09-05, 2026-09-12, 2026-09-19, 2026-09-25..2026-09-27 (3 days), 2026-10-01..2026-10-02 (2 days), 2026-10-09, 2026-10-16, 2026-10-23, 2026-10-30, 2026-11-06, 2026-11-13, 2026-11-20, 2026-11-27, 2026-12-06, 2026-12-13, 2026-12-20, 2026-12-26..2026-12-28 (3 days)
- month_count 4, 58 inputs: 2026-01-02..2026-01-03 (2 days), 2026-01-09, 2026-01-16, 2026-01-23..2026-01-24 (2 days), 2026-01-30, 2026-02-06, 2026-02-13, 2026-02-20, 2026-02-27, 2026-03-04, 2026-03-11, 2026-03-18, 2026-03-25, 2026-04-01, 2026-04-08, 2026-04-15, 2026-04-22, 2026-04-29, 2026-05-05, 2026-05-12, 2026-05-19, 2026-05-26, 2026-06-03, 2026-06-10, 2026-06-17, 2026-06-24, 2026-06-30, 2026-07-07, 2026-07-14, 2026-07-21, 2026-07-28, 2026-08-05, 2026-08-12, 2026-08-19, 2026-08-25..2026-08-27 (3 days), 2026-09-01..2026-09-02 (2 days), 2026-09-09, 2026-09-16, 2026-09-23, 2026-10-06, 2026-10-13, 2026-10-20, 2026-10-27, 2026-11-06, 2026-11-13, 2026-11-20, 2026-11-26..2026-11-28 (3 days), 2026-12-03, 2026-12-10, 2026-12-17, 2026-12-24
- month_count 5, 60 inputs: 2026-01-06, 2026-01-13, 2026-01-20, 2026-01-27, 2026-02-04, 2026-02-11, 2026-02-18, 2026-02-25, 2026-03-01, 2026-03-08, 2026-03-15, 2026-03-22, 2026-03-29..2026-03-30 (2 days), 2026-04-05, 2026-04-12, 2026-04-19, 2026-04-26, 2026-05-03, 2026-05-10, 2026-05-17, 2026-05-24, 2026-05-31, 2026-06-07, 2026-06-14, 2026-06-21, 2026-06-28, 2026-07-05, 2026-07-12, 2026-07-19, 2026-07-25..2026-07-27 (3 days), 2026-08-01..2026-08-02 (2 days), 2026-08-09, 2026-08-16, 2026-08-23, 2026-08-30, 2026-09-06, 2026-09-13, 2026-09-20, 2026-09-27, 2026-10-06, 2026-10-13, 2026-10-20, 2026-10-26..2026-10-28 (3 days), 2026-11-03, 2026-11-10, 2026-11-17, 2026-11-24, 2026-12-01..2026-12-02 (2 days), 2026-12-08, 2026-12-15, 2026-12-22, 2026-12-29..2026-12-30 (2 days)
- month_count 6, 57 inputs: 2026-01-04, 2026-01-11, 2026-01-18, 2026-01-25, 2026-02-01, 2026-02-08, 2026-02-15, 2026-02-22, 2026-03-05, 2026-03-12, 2026-03-19, 2026-03-26, 2026-04-03, 2026-04-10, 2026-04-17, 2026-04-24, 2026-04-30, 2026-05-07, 2026-05-14, 2026-05-21, 2026-05-28, 2026-06-05, 2026-06-12, 2026-06-19, 2026-06-25..2026-06-27 (3 days), 2026-07-01..2026-07-02 (2 days), 2026-07-09, 2026-07-16, 2026-07-23, 2026-07-30, 2026-08-06, 2026-08-13, 2026-08-20, 2026-08-27, 2026-09-06, 2026-09-13, 2026-09-20, 2026-09-26..2026-09-28 (3 days), 2026-10-03, 2026-10-10, 2026-10-17, 2026-10-24, 2026-11-01..2026-11-02 (2 days), 2026-11-08, 2026-11-15, 2026-11-22, 2026-11-29, 2026-12-05, 2026-12-12, 2026-12-19, 2026-12-26
- month_count 7, 60 inputs: 2026-01-01, 2026-01-08, 2026-01-15, 2026-01-22, 2026-01-29..2026-01-30 (2 days), 2026-02-05, 2026-02-12, 2026-02-19, 2026-02-26, 2026-03-03, 2026-03-10, 2026-03-17, 2026-03-24, 2026-03-31, 2026-04-07, 2026-04-14, 2026-04-21, 2026-04-28, 2026-05-05, 2026-05-12, 2026-05-19, 2026-05-25..2026-05-27 (3 days), 2026-06-01..2026-06-02 (2 days), 2026-06-09, 2026-06-16, 2026-06-23, 2026-07-06, 2026-07-13, 2026-07-20, 2026-07-27, 2026-08-06, 2026-08-13, 2026-08-20, 2026-08-26..2026-08-28 (3 days), 2026-09-03, 2026-09-10, 2026-09-17, 2026-09-24, 2026-10-01..2026-10-02 (2 days), 2026-10-08, 2026-10-15, 2026-10-22, 2026-10-29..2026-10-30 (2 days), 2026-11-05, 2026-11-12, 2026-11-19, 2026-11-26, 2026-12-03, 2026-12-10, 2026-12-17, 2026-12-24, 2026-12-31
- month_count 8, 59 inputs: 2026-01-05, 2026-01-12, 2026-01-19, 2026-01-26, 2026-02-03, 2026-02-10, 2026-02-17, 2026-02-24, 2026-02-28, 2026-03-07, 2026-03-14, 2026-03-21, 2026-03-28, 2026-04-05, 2026-04-12, 2026-04-19, 2026-04-25..2026-04-27 (3 days), 2026-05-01..2026-05-02 (2 days), 2026-05-09, 2026-05-16, 2026-05-23, 2026-05-30, 2026-06-06, 2026-06-13, 2026-06-20, 2026-06-27, 2026-07-06, 2026-07-13, 2026-07-20, 2026-07-26..2026-07-28 (3 days), 2026-08-03, 2026-08-10, 2026-08-17, 2026-08-24, 2026-09-01..2026-09-02 (2 days), 2026-09-08, 2026-09-15, 2026-09-22, 2026-09-29, 2026-10-05, 2026-10-12, 2026-10-19, 2026-10-26, 2026-11-03, 2026-11-10, 2026-11-17, 2026-11-24, 2026-11-30, 2026-12-07, 2026-12-14, 2026-12-21, 2026-12-28..2026-12-29 (2 days)
- month_count 9, 58 inputs: 2026-01-03, 2026-01-10, 2026-01-17, 2026-01-24, 2026-01-31, 2026-02-07, 2026-02-14, 2026-02-21, 2026-03-05, 2026-03-12, 2026-03-19, 2026-03-25..2026-03-27 (3 days), 2026-04-01..2026-04-02 (2 days), 2026-04-09, 2026-04-16, 2026-04-23, 2026-05-06, 2026-05-13, 2026-05-20, 2026-05-27, 2026-06-06, 2026-06-13, 2026-06-20, 2026-06-26..2026-06-28 (3 days), 2026-07-03, 2026-07-10, 2026-07-17, 2026-07-24, 2026-08-01..2026-08-02 (2 days), 2026-08-08, 2026-08-15, 2026-08-22, 2026-08-29..2026-08-30 (2 days), 2026-09-05, 2026-09-12, 2026-09-19, 2026-09-26, 2026-10-03, 2026-10-10, 2026-10-17, 2026-10-24, 2026-10-31, 2026-11-07, 2026-11-14, 2026-11-21, 2026-11-28..2026-11-29 (2 days), 2026-12-04, 2026-12-11, 2026-12-18, 2026-12-25
- month_count 10, 60 inputs: 2026-01-07, 2026-01-14, 2026-01-21, 2026-01-28, 2026-02-05, 2026-02-12, 2026-02-19, 2026-02-25..2026-02-27 (3 days), 2026-03-01..2026-03-02 (2 days), 2026-03-09, 2026-03-16, 2026-03-23, 2026-03-30, 2026-04-06, 2026-04-13, 2026-04-20, 2026-04-27, 2026-05-06, 2026-05-13, 2026-05-20, 2026-05-26..2026-05-28 (3 days), 2026-06-03, 2026-06-10, 2026-06-17, 2026-06-24, 2026-07-01..2026-07-02 (2 days), 2026-07-08, 2026-07-15, 2026-07-22, 2026-07-29..2026-07-30 (2 days), 2026-08-05, 2026-08-12, 2026-08-19, 2026-08-26, 2026-09-03, 2026-09-10, 2026-09-17, 2026-09-24, 2026-09-30, 2026-10-07, 2026-10-14, 2026-10-21, 2026-10-28..2026-10-29 (2 days), 2026-11-04, 2026-11-11, 2026-11-18, 2026-11-25, 2026-12-02, 2026-12-09, 2026-12-16, 2026-12-23, 2026-12-30
- month_count 11, 57 inputs: 2026-01-05, 2026-01-12, 2026-01-19, 2026-01-25..2026-01-27 (3 days), 2026-02-01..2026-02-02 (2 days), 2026-02-09, 2026-02-16, 2026-02-23, 2026-03-06, 2026-03-13, 2026-03-20, 2026-03-27, 2026-04-06, 2026-04-13, 2026-04-20, 2026-04-26..2026-04-28 (3 days), 2026-05-03, 2026-05-10, 2026-05-17, 2026-05-24, 2026-06-01..2026-06-02 (2 days), 2026-06-08, 2026-06-15, 2026-06-22, 2026-06-29, 2026-07-05, 2026-07-12, 2026-07-19, 2026-07-26, 2026-08-03, 2026-08-10, 2026-08-17, 2026-08-24, 2026-08-31, 2026-09-07, 2026-09-14, 2026-09-21, 2026-09-28..2026-09-29 (2 days), 2026-10-04, 2026-10-11, 2026-10-18, 2026-10-25, 2026-11-02, 2026-11-09, 2026-11-16, 2026-11-23, 2026-12-06, 2026-12-13, 2026-12-20, 2026-12-27
- month_count 12, 60 inputs: 2026-01-01..2026-01-02 (2 days), 2026-01-09, 2026-01-16, 2026-01-23, 2026-01-30, 2026-02-06, 2026-02-13, 2026-02-20, 2026-02-27, 2026-03-06, 2026-03-13, 2026-03-20, 2026-03-26..2026-03-28 (3 days), 2026-04-03, 2026-04-10, 2026-04-17, 2026-04-24, 2026-05-01..2026-05-02 (2 days), 2026-05-08, 2026-05-15, 2026-05-22, 2026-05-29..2026-05-30 (2 days), 2026-06-05, 2026-06-12, 2026-06-19, 2026-06-26, 2026-07-03, 2026-07-10, 2026-07-17, 2026-07-24, 2026-07-31, 2026-08-07, 2026-08-14, 2026-08-21, 2026-08-28..2026-08-29 (2 days), 2026-09-04, 2026-09-11, 2026-09-18, 2026-09-25, 2026-10-02, 2026-10-09, 2026-10-16, 2026-10-23, 2026-10-30, 2026-11-06, 2026-11-13, 2026-11-20, 2026-11-27, 2026-12-04, 2026-12-11, 2026-12-18, 2026-12-25..2026-12-27 (3 days)

The first input it fails on:

```text
origin             2026-01-07 Wed
                   month_count = 1
first_day          2026-01-08 Thu  + 1 day
last_day           2026-02-08 Sun  + month_count months else start_of_next_month (month_count = 1)
                   2026-02-07 Sat  - 1 day
next_day           2026-02-08 Sun  if closed + 1 day: 2026-02-07 (Saturday) is closed
next_business_day  2026-02-09 Mon  roll following: 2026-02-07 (Saturday) and 2026-02-08 (Sunday) are closed
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
| 2026-01-01 Thu | 1 | 2026-01-02 Fri | 2026-02-01 Sun | 2026-02-02 Mon | 2026-02-02 Mon | 2026-02-01 Sun | next_day falls on a Sunday, and `if closed + 1 day` acts; next_business_day falls on a Sunday and moves to the next business day; origin is New Year’s Day, a closed day |
| 2026-01-02 Fri | 1 | 2026-01-03 Sat | 2026-02-02 Mon | 2026-02-02 Mon | 2026-02-02 Mon | 2026-02-02 Mon | origin is the day after New Year’s Day |
| 2026-01-07 Wed | 1 | 2026-01-08 Thu | 2026-02-07 Sat | 2026-02-08 Sun | 2026-02-09 Mon | 2026-02-07 Sat | the claim the_two_readings_agree fails; next_business_day falls on a Saturday and moves to the next business day |
| 2026-01-28 Wed | 1 | 2026-01-29 Thu | 2026-02-28 Sat | 2026-03-01 Sun | 2026-03-02 Mon | 2026-02-28 Sat | computing last_day lands on 2026-02-29, which does not exist, and uses `else start_of_next_month` |
| 2026-01-29 Thu | 1 | 2026-01-30 Fri | 2026-02-28 Sat | 2026-03-01 Sun | 2026-03-02 Mon | 2026-02-28 Sat | computing months_added lands on 2026-02-29, which does not exist, and uses `else end_of_month` |
| 2026-01-31 Sat | 1 | 2026-02-01 Sun | 2026-02-28 Sat | 2026-03-01 Sun | 2026-03-02 Mon | 2026-02-28 Sat | the fewest days from origin to months_added (28); origin is the 31st, the end of its month |
| 2026-02-02 Mon | 1 | 2026-02-03 Tue | 2026-03-02 Mon | 2026-03-02 Mon | 2026-03-02 Mon | 2026-03-02 Mon | the fewest days from origin to next_day and next_business_day (28) |
| 2026-02-28 Sat | 1 | 2026-03-01 Sun | 2026-03-31 Tue | 2026-03-31 Tue | 2026-03-31 Tue | 2026-03-28 Sat | the claim rounding_to_the_month_end fails; origin is 28 February, the end of the month in a year that is not a leap year |
| 2026-03-03 Tue | 1 | 2026-03-04 Wed | 2026-04-03 Fri | 2026-04-04 Sat | 2026-04-07 Tue | 2026-04-03 Fri | next_business_day falls on Good Friday and moves to the next business day; next_business_day moves the most (4 days) |
| 2026-03-06 Fri | 1 | 2026-03-07 Sat | 2026-04-06 Mon | 2026-04-07 Tue | 2026-04-07 Tue | 2026-04-06 Mon | next_business_day falls on Easter Monday and moves to the next business day |
| 2026-04-02 Thu | 1 | 2026-04-03 Fri | 2026-05-02 Sat | 2026-05-03 Sun | 2026-05-05 Tue | 2026-05-02 Sat | origin is the day before Good Friday |
| 2026-04-04 Sat | 1 | 2026-04-05 Sun | 2026-05-04 Mon | 2026-05-05 Tue | 2026-05-05 Tue | 2026-05-04 Mon | next_business_day falls on Early May bank holiday and moves to the next business day |
| 2026-04-25 Sat | 1 | 2026-04-26 Sun | 2026-05-25 Mon | 2026-05-26 Tue | 2026-05-26 Tue | 2026-05-25 Mon | next_business_day falls on Spring bank holiday and moves to the next business day |
| 2026-04-30 Thu | 1 | 2026-05-01 Fri | 2026-05-31 Sun | 2026-06-01 Mon | 2026-06-01 Mon | 2026-05-30 Sat | origin is the 30th, the end of its month |
| 2026-01-01 Thu | 12 | 2026-01-02 Fri | 2027-01-01 Fri | 2027-01-02 Sat | 2027-01-04 Mon | 2027-01-01 Fri | the most days from origin to next_day (366); the most days from origin to months_added (365) |
| 2026-03-26 Thu | 12 | 2026-03-27 Fri | 2027-03-26 Fri | 2027-03-27 Sat | 2027-03-30 Tue | 2027-03-26 Fri | the most days from origin to next_business_day (369) |

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

In 2026-01-01..2027-12-31, the days of origin and of the dates computed from it, the longest run of closed days is 2026-04-03..2026-04-06, 4 days.

### Month by month

In the tables, a day in parentheses is closed; a day in bold is an input a claim fails on; ◆ marks the input of an edge case.

**January 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  | **(1)**◆ | **2**◆ | **(3)** | **(4)** |
| **5** | **6** | **7**◆ | **8** | **9** | **(10)** | **(11)** |
| **12** | **13** | **14** | **15** | **16** | **(17)** | **(18)** |
| **19** | **20** | **21** | **22** | **23** | **(24)** | **(25)** |
| **26** | **27** | **28**◆ | **29**◆ | **30** | **(31)**◆ |  |

Named closed days: 1 New Year’s Day

**February 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2**◆ | **3** | **4** | **5** | **6** | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26** | **27** | **(28)**◆ |  |

**March 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  |  | **(1)** |
| **2** | **3**◆ | **4** | **5** | **6**◆ | **(7)** | **(8)** |
| **9** | **10** | **11** | **12** | **13** | **(14)** | **(15)** |
| **16** | **17** | **18** | **19** | **20** | **(21)** | **(22)** |
| **23** | **24** | **25** | **26**◆ | **27** | **(28)** | **(29)** |
| **30** | **31** |  |  |  |  |  |

**April 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  | **1** | **2**◆ | **(3)** | **(4)**◆ | **(5)** |
| **(6)** | **7** | **8** | **9** | **10** | **(11)** | **(12)** |
| **13** | **14** | **15** | **16** | **17** | **(18)** | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)**◆ | **(26)** |
| **27** | **28** | **29** | **30**◆ |  |  |  |

Named closed days: 3 Good Friday, 6 Easter Monday

**May 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  | **1** | **(2)** | **(3)** |
| **(4)** | **5** | **6** | **7** | **8** | **(9)** | **(10)** |
| **11** | **12** | **13** | **14** | **15** | **(16)** | **(17)** |
| **18** | **19** | **20** | **21** | **22** | **(23)** | **(24)** |
| **(25)** | **26** | **27** | **28** | **29** | **(30)** | **(31)** |

Named closed days: 4 Early May bank holiday, 25 Spring bank holiday

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
|  |  | **1** | **2** | **3** | (4) | **(5)** |
| **6** | **7** | **8** | **9** | **10** | (11) | **(12)** |
| **13** | **14** | **15** | **16** | **17** | (18) | **(19)** |
| **20** | **21** | **22** | **23** | **24** | **(25)** | **(26)** |
| **27** | **28** | **29** | **30** | **31** |  |  |

**August 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  |  |  |  |  | **(1)** | **(2)** |
| **3** | 4 | **5** | **6** | **7** | **(8)** | **(9)** |
| **10** | 11 | **12** | **13** | **14** | **(15)** | **(16)** |
| **17** | 18 | **19** | **20** | **21** | **(22)** | **(23)** |
| **24** | **25** | **26** | **27** | **28** | **(29)** | **(30)** |
| **(31)** |  |  |  |  |  |  |

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
| **30** |  |  |  |  |  |  |

**December 2026**

| Mon | Tue | Wed | Thu | Fri | Sat | Sun |
|---:|---:|---:|---:|---:|---:|---:|
|  | **1** | **2** | **3** | **4** | **(5)** | **(6)** |
| **7** | **8** | **9** | **10** | **11** | **(12)** | **(13)** |
| **14** | **15** | **16** | **17** | **18** | **(19)** | **(20)** |
| **21** | **22** | **23** | **24** | **(25)** | **(26)** | **(27)** |
| **(28)** | **29** | **30** | **31** |  |  |  |

Named closed days: 25 Christmas Day, 28 Boxing Day (Substitute day)

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
