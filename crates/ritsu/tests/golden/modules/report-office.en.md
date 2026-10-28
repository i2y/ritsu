## ritsu mod report: `one` → the working tree

| Check | Errors | Warnings |
|---|---|---|
| With the lock of `one` | 0 | 1 |
| With the lock of the working tree | 0 | 1 |

### `example.com/acme/desk` (`@desk`) 1.0.0 → 1.1.0: upgraded

- Files: 1 added, 1 removed, 4 changed, 1 the same
- require: no change

| Change | File |
|---|---|
| added | `calendars/data/days_off_v2.csv` |
| removed | `calendars/data/days_off.csv` |
| changed | `calendars/office.cal` |
| changed | `gates/band.rule` |
| changed | `gates/limits.gate` |
| changed | `ritsu.mod` |

#### Behavior

<details><summary><code>calendars/office.cal</code> (koyomi): the answers differ</summary>

**koyomi diff: calendar `office` v1 → v2**

- Old: `@desk@1.0.0/calendars/office.cal`
- New: `@desk@1.1.0/calendars/office.cal`
- Compared: 2026-01-01..2026-12-31 (365 days)
- The days known: 2026-01-01..2026-12-31 → 2026-01-01..2027-12-31
- Offset: none → +01:00

**Open or closed changed**

| Days | Old | New |
|---|---|---|
| 2026-06-10 | open | closed (Founding day) |

**Closed in both, for other reasons**

| Days | Old reasons | New reasons |
|---|---|---|
| 2026-05-04 | Early May bank holiday | May Day |

Every other day of 2026-01-01..2026-12-31 gets the same answer.

</details>

<details><summary><code>calendars/payment.cal</code> (koyomi): the answers differ</summary>

**koyomi diff: dates `payment` v1 → v1**

- Old: `@desk@1.0.0/calendars/payment.cal`
- New: `@desk@1.1.0/calendars/payment.cal`
- Inputs compared: received 2026-01-01..2026-10-31 (304 combinations)
- Dates compared: closing, payment

**`payment`**

| Inputs | Combinations | Old | New | Shift |
|---|---|---|---|---|
| received 2026-04-21..2026-05-20 | 30 | 2026-06-10 | 2026-06-09 | 1 day earlier |

For every other combination, every date gets the same value.

</details>

<details><summary><code>gates/band.rule</code> (rulec): the answers differ</summary>

**Version diff (over the whole input space)**

`@desk@1.0.0/gates/band.rule` → `@desk@1.1.0/gates/band.rule`

| where they differ | answer | example | cells |
|---|---|---|---|
| amount >=6 <=8 | band: large → small | amount=6 | 3 |

Outside this region the two versions answer alike.

</details>

<details><summary><code>gates/limits.gate</code> (sekisho): the answers differ</summary>

**sekisho diff: gate `limits` v1 → v2**

- Old: `@desk@1.0.0/gates/limits.gate`
- New: `@desk@1.1.0/gates/limits.gate`

**`refund_order`**

20 combinations walked over both versions.

| Change | Combinations | Old deciding policies | New deciding policies | Example |
|---|---|---|---|---|
| allow gained (deny → allow) | 1 | none | clerks_refund_small_amounts | User holding clerk, Order, amount: 1 to 15, size: small (in the old version: size: large) |
| the same answer, decided by other policies | 1 | managers_refund | clerks_refund_small_amounts, managers_refund | User holding clerk and manager, Order, amount: 1 to 15, size: small (in the old version: size: large) |

For every other combination, every action is decided the same.

</details>

#### Interface

- `calendars/office.cal` (koyomi, api): 7 changes, 7 of them may break a user
  - The version at the head of the file: `1` → `2`

| Change | Where | Old → new |
|---|---|---|
| may break: changed | `/calendar/offset` | `null → "+01:00"` |
| may break: changed | `/calendar/sources/days_off/file` | `"data/days_off.csv" → "data/days_off_v2.csv"` |
| may break: changed | `/calendar/sources/days_off/sha256` | `"db3a1d57457a1dd9" → "a045e62eccc16e1f"` |
| may break: changed | `/calendar/sources/days_off/covers/to` | `"2026-12-31" → "2027-12-31"` |
| may break: changed | `/calendar/sources/days_off/rows` | `3 → 7` |
| may break: changed | `/calendar/data_range/to` | `"2026-12-31" → "2027-12-31"` |
| may break: changed | `/wasm/package` | `"koyomi:office@1.0.0" → "koyomi:office@2.0.0"` |

- `gates/band.rule` (rulec, api): 4 changes, 4 of them may break a user
  - The version at the head of the file: `1` → `2`

| Change | Where | Old → new |
|---|---|---|
| may break: changed | `/connect/proto` | `"proto/rulec/band/v1/band.proto" → "proto/rulec/band/v2/band.proto"` |
| may break: changed | `/connect/package` | `"rulec.band.v1" → "rulec.band.v2"` |
| may break: changed | `/connect/path` | `"/rulec.band.v1.BandService/Decide" → "/rulec.band.v2.BandService/Decide"` |
| may break: changed | `/wasm/package` | `"rulec:band@1.0.0" → "rulec:band@2.0.0"` |

- `gates/limits.gate` (sekisho, api): 1 change, 0 of them may break a user
  - The version at the head of the file: `v1` → `v2`

| Change | Where | Old → new |
|---|---|---|
| changed | `/description` | `"…y the band of a rule: a clerk refunds a small amou…" → "…y the band of a rule, read from its second version…"` |

A change that may break a user is a candidate: whether it does is for that user's check to say.

A difference is shown in behavior or in an interface.

#### Your answers that change

<details><summary><code>invoice.cal</code> (koyomi): the answers differ</summary>

**koyomi diff: dates `invoice` v1 → v1**

- Old: `invoice.cal`
- New: `invoice.cal`
- Inputs compared: received 2026-01-01..2026-10-31 (304 combinations)
- Dates compared: closing, payment

**`payment`**

| Inputs | Combinations | Old | New | Shift |
|---|---|---|---|---|
| received 2026-04-21..2026-05-20 | 30 | 2026-06-10 | 2026-06-09 | 1 day earlier |

For every other combination, every date gets the same value.

</details>

<details><summary><code>refunds.gate</code> (sekisho): the answers differ</summary>

**sekisho diff: gate `refunds` v1 → v1**

- Old: `refunds.gate`
- New: `refunds.gate`

**`refund_order`**

20 combinations walked over both versions.

| Change | Combinations | Old deciding policies | New deciding policies | Example |
|---|---|---|---|---|
| allow gained (deny → allow) | 1 | none | clerks_refund_small_amounts | User holding clerk, Order, amount: 1 to 15, size: small (in the old version: size: large) |
| the same answer, decided by other policies | 1 | managers_refund | clerks_refund_small_amounts, managers_refund | User holding clerk and manager, Order, amount: 1 to 15, size: small (in the old version: size: large) |

For every other combination, every action is decided the same.

</details>

The check with the lock of the working tree has no error; a difference is shown in behavior.
