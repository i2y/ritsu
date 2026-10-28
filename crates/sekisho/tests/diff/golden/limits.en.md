### sekisho diff: gate `limits` v1 → v2

- Old: `tests/diff/limits.gate`
- New: `tests/diff/limits_v2.gate`

#### `refund_order`

20 combinations walked over both versions.

| Change | Combinations | Old deciding policies | New deciding policies | Example |
|---|---|---|---|---|
| allow gained (deny → allow) | 1 | none | clerks_refund_small_amounts | User holding clerk, Order, amount: 1 to 15, size: small (in the old version: size: large) |
| the same answer, decided by other policies | 1 | managers_refund | clerks_refund_small_amounts, managers_refund | User holding clerk and manager, Order, amount: 1 to 15, size: small (in the old version: size: large) |

For every other combination, every action is decided the same.
