### ritsu mod diff: `example.com/acme/stock` 1.0.0 → 1.1.0

- Old: `stock-1.0.0`
- New: `stock-1.1.0`
- Files: 2 changed
- require: no change

| Change | File |
|---|---|
| changed | `books/stock.book` |
| changed | `ritsu.mod` |

#### Interface

- `books/stock.book` (chobo, api): 15 changes, 6 of them may break a user
  - The version at the head of the file: `1` → `2`
  - Breaks the data: **E050** (line 4): the account `shelf` differs from stock-1.0.0 in its lower bound (0 → 3): the accounts that already exist keep the old definition, so chobo cannot make this change
  - Breaks the data: **E051** (line 9): the transfer `receive` differs from stock-1.0.0 in its key: a retry in flight would be refused with key_conflict

| Change | Where | Old → new |
|---|---|---|
| may break: changed | `/accounts/shelf/lower/value` | `0 → 3` |
| may break: removed | `/transfers/receive/key/1` |  |
| may break: changed | `/transfers/receive/definition` | `"181da693d81130042643070747c5bb1c" → "b0574e00b7d68262080316101403e8c7"` |
| may break: removed | `/transfers/receive/operations/do/key/params/1` |  |
| may break: changed | `/transfers/ship/definition` | `"0369dd8e4cec74d2c1e5f68bf98ee348" → "2e82fe24dacb05136689593c7a23ff0f"` |
| may break: changed | `/transfers/ship/operations/do/refusals/out_of_stock/bound` | `"at least 0" → "at least 3"` |
| added | `/transfers/take_back` |  |
| added | `/targets/postgres/functions/take_back_do` |  |
| added | `/targets/{typescript,python,go,rust}/transfers/2` |  |
| added | `/targets/tigerbeetle/chains/1/transfers/1` |  |
| added | `/targets/tigerbeetle/chains/1/transfers/2` |  |
| added | `/targets/tigerbeetle/chains/2` |  |

A change that may break a user is a candidate: whether it does is for that user's check to say.

A difference is shown in behavior or in an interface.
