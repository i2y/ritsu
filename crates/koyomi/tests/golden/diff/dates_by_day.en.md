### koyomi diff: dates `payment` v1 → v1

- Old: `payment_by_day.cal`
- New: `payment_by_day_later.cal`
- Inputs compared: received 2026-01-01..2026-10-31, pay_day 9..11 (912 combinations)
- Dates compared: closing, payment

#### `payment`

| Inputs | Combinations | Old | New | Shift |
|---|---|---|---|---|
| received 2026-04-21..2026-05-20, pay_day 10 | 30 | 2026-06-10 | 2026-06-09 | 1 day earlier |

For every other combination, every date gets the same value.
