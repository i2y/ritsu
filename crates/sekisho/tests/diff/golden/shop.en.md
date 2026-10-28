### sekisho diff: gate `shop` v1 → v2

- Old: `tests/diff/shop.gate`
- New: `tests/diff/shop_v2.gate`
- Only in the old version: action cancel_order; attribute User.seniority
- Only in the new version: action approve_refund; role auditor; attribute User.region

#### `refund_order`

64 combinations walked over both versions.

| Change | Combinations | Old deciding policies | New deciding policies | Example |
|---|---|---|---|---|
| allow gained (deny → allow) | 2 | none | clerks_refund_small_amounts | User holding clerk (seniority: 0 to 1, suspended: no), Order, amount: 6 to 8 |
| allow gained (deny → allow) | 2 | none | clerks_refund_small_amounts, managers_refund_up_to_15 | User holding manager (seniority: 0 to 1, suspended: no), Order, amount: 6 to 8 |
| allow gained (deny → allow) | 2 | none | managers_refund_up_to_15 | User holding manager (seniority: 0 to 1, suspended: no), Order, amount: 9 to 15 |
| allow lost (allow → deny) | 4 | clerks_refund_small_amounts | suspended_members_refund_nothing | User holding clerk (seniority: 0 to 1, suspended: yes), Order, amount: 1 to 5 |
| allow lost (allow → deny) | 2 | clerks_refund_small_amounts, managers_refund | suspended_members_refund_nothing | User holding manager (seniority: 2 to 10, suspended: yes), Order, amount: 1 to 5 |
| allow lost (allow → deny) | 4 | managers_refund | suspended_members_refund_nothing | User holding manager (seniority: 2 to 10, suspended: yes), Order, amount: 6 to 8 |
| the same answer, decided by other policies | 22 | none | suspended_members_refund_nothing | User holding no role (seniority: 0 to 1, suspended: yes), Order, amount: 1 to 5 |
| the same answer, decided by other policies | 2 | clerks_refund_small_amounts | clerks_refund_small_amounts, managers_refund_up_to_15 | User holding manager (seniority: 0 to 1, suspended: no), Order, amount: 1 to 5 |
| the same answer, decided by other policies | 2 | clerks_refund_small_amounts, managers_refund | clerks_refund_small_amounts, managers_refund_up_to_15 | User holding manager (seniority: 2 to 10, suspended: no), Order, amount: 1 to 5 |
| the same answer, decided by other policies | 2 | managers_refund | clerks_refund_small_amounts, managers_refund_up_to_15 | User holding manager (seniority: 2 to 10, suspended: no), Order, amount: 6 to 8 |
| the same answer, decided by other policies | 2 | managers_refund | managers_refund_up_to_15 | User holding manager (seniority: 2 to 10, suspended: no), Order, amount: 9 to 15 |

For every other combination, every action is decided the same.
