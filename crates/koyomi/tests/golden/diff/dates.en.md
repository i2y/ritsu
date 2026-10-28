### koyomi diff: dates `payment` v1 → v2

- Old: `payment.cal`
- New: `payment_v2.cal`
- Inputs compared: received 2026-01-01..2026-09-30 (273 combinations)
- Dates compared: closing, payment
- Dates only in the new version: reminder

#### The inputs differ

- received: 2026-01-01..2026-10-31 → 2026-01-01..2026-09-30

#### `payment`

| Inputs | Combinations | Old | New | Shift |
|---|---|---|---|---|
| received 2026-04-21..2026-05-20 | 30 | 2026-06-10 | 2026-06-09 | 1 day earlier |

For every other combination, every date gets the same value.
