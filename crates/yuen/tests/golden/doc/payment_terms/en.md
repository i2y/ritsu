# payment_terms — where the requirements come from

yuen 0.23.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `payment_terms.req` (payment_terms v1, `sha256:7601e66a63843869`)

The check: `examples/payment_terms/payment_terms.req: ok — 2 requirements, whose 6 links are as they were looked at; every requirement is met and checked, or waived; the 2 dates in scope all trace to a requirement`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `payment_day` | 2026-10-01.. | decided by accounting on 2026-10-04 | accounting | `koyomi "payment_20th_close_next_10th.cal" date closing`; `koyomi "payment_20th_close_next_10th.cal" date payment` | `koyomi "payment_20th_close_next_10th.cal" claim within_60_days_of_receipt` | as looked at |
| `business_days` | 2026-10-01.. | `@holidays` | accounting | `koyomi "payment_20th_close_next_10th.cal" date payment` | `koyomi "payment_20th_close_next_10th.cal" claim paid_on_a_business_day` | as looked at |

## Sources

### holidays

The file `calendars/data/syukujitsu.csv` (from `https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv`), pinned at `sha256:cec37a743c96995c`. Borrowed from `koyomi "calendars/tokyo_business_days.cal" source national_holidays` (the copies and the pins are that file's, and its language's check holds them).

Cited by: `business_days`

## Why each requirement is so

### payment_day

**Invoices close on the 20th and are paid on the 10th of the next month, within 60 days of receipt**

- In force: 2026-10-01..
- Owner: accounting
- File: `examples/payment_terms/payment_terms.req:11`
- The requirement's end: `sha256:ce2048eb0cb0cf01`

Decided by accounting on 2026-10-04: Closing on the 20th and paying on the 10th of the next month keeps every payment within 60 days of receipt. Decided for this example

Met by `koyomi "payment_20th_close_next_10th.cal" date closing` — looked at by development on 2026-10-04 (`sha256:ce2048eb0cb0cf01` → `sha256:64153f3ffeeabb6b`). State: as looked at

Met by `koyomi "payment_20th_close_next_10th.cal" date payment` — looked at by development on 2026-10-04 (`sha256:ce2048eb0cb0cf01` → `sha256:77727589d8318079`). State: as looked at

Checked by `koyomi "payment_20th_close_next_10th.cal" claim within_60_days_of_receipt` — looked at by development on 2026-10-04 (`sha256:ce2048eb0cb0cf01` → `sha256:39afffc0504e0754`). State: as looked at

### business_days

**A payment is made on a business day: when the payment day is a Saturday, a Sunday or a national holiday, it is made on the business day before**

- In force: 2026-10-01..
- Owner: accounting
- File: `examples/payment_terms/payment_terms.req:23`
- The requirement's end: `sha256:d53e37ddddbb71fa`

Comes from `@holidays` — looked at by accounting on 2026-10-04 (`sha256:cec37a743c96995c` → `sha256:d53e37ddddbb71fa`). State: as looked at

Met by `koyomi "payment_20th_close_next_10th.cal" date payment` — looked at by development on 2026-10-04 (`sha256:d53e37ddddbb71fa` → `sha256:77727589d8318079`). State: as looked at

Checked by `koyomi "payment_20th_close_next_10th.cal" claim paid_on_a_business_day` — looked at by development on 2026-10-04 (`sha256:d53e37ddddbb71fa` → `sha256:a30c094c89e283df`). State: as looked at

## Scope

`scope koyomi "payment_20th_close_next_10th.cal" date` — 2 artifacts in it, every one tracing to a requirement.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-04 | development | `payment_day` → `koyomi "payment_20th_close_next_10th.cal" date closing` | `sha256:ce2048eb0cb0cf01 -> sha256:64153f3ffeeabb6b` |
| 2026-10-04 | development | `payment_day` → `koyomi "payment_20th_close_next_10th.cal" date payment` | `sha256:ce2048eb0cb0cf01 -> sha256:77727589d8318079` |
| 2026-10-04 | development | `payment_day` → `koyomi "payment_20th_close_next_10th.cal" claim within_60_days_of_receipt` | `sha256:ce2048eb0cb0cf01 -> sha256:39afffc0504e0754` |
| 2026-10-04 | accounting | `@holidays` → `business_days` | `sha256:cec37a743c96995c -> sha256:d53e37ddddbb71fa` |
| 2026-10-04 | development | `business_days` → `koyomi "payment_20th_close_next_10th.cal" date payment` | `sha256:d53e37ddddbb71fa -> sha256:77727589d8318079` |
| 2026-10-04 | development | `business_days` → `koyomi "payment_20th_close_next_10th.cal" claim paid_on_a_business_day` | `sha256:d53e37ddddbb71fa -> sha256:a30c094c89e283df` |
