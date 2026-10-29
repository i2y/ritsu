### sekisho diff: gate `held` v1 → v2

- Old: `tests/diff/held.gate`
- New: `tests/diff/held_v2.gate`

#### `refund_order`

8 combinations walked over both versions.

| Change | Combinations | Old deciding policies | New deciding policies | Example |
|---|---|---|---|---|
| allow lost (allow → deny) | 1 | clerks_refund_small_amounts | nothing_from_4_to_10 | User holding clerk, Order, amount: 4 to 10, size: small |
| the same answer, decided by other policies | 3 | none | nothing_from_4_to_10 | User holding no role, Order, amount: 4 to 10, size: small |

For every other combination, every action is decided the same.
