# 支払条件 — where the requirements come from

yuen 0.24.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `payment_terms.ja.req` (支払条件 v1, `sha256:efb13e38decc0120`)

The check: `examples/payment_terms/payment_terms.ja.req: ok — 2 requirements, whose 6 links are as they were looked at; every requirement is met and checked, or waived; the 2 dates in scope all trace to a requirement`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `支払日 (payment_day)` | 2026-10-01.. | decided by 経理 on 2026-10-04 | 経理 | `koyomi "payment_20th_close_next_10th.ja.cal" date 締め日`; `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `koyomi "payment_20th_close_next_10th.ja.cal" claim 受領から60日以内` | as looked at |
| `営業日 (business_days)` | 2026-10-01.. | `@祝日` | 経理 | `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `koyomi "payment_20th_close_next_10th.ja.cal" claim 営業日に払う` | as looked at |

## Sources

### 祝日

The file `calendars/data/syukujitsu.csv` (from `https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv`), pinned at `sha256:cec37a743c96995c`. Borrowed from `koyomi "calendars/東京の営業日.cal" source 祝日` (the copies and the pins are that file's, and its language's check holds them).

Cited by: `営業日 (business_days)`

## Why each requirement is so

### 支払日 (payment_day)

**20 日締め翌月 10 日払い。受領から 60 日以内に払う**

- In force: 2026-10-01..
- Owner: 経理
- File: `examples/payment_terms/payment_terms.ja.req:11`
- The requirement's end: `sha256:5d97c87d3ce26490`

Decided by 経理 on 2026-10-04: 20 日に締めて翌月 10 日に払えば、どの支払も受領から 60 日以内に収まる。この例のために決めたもの

Met by `koyomi "payment_20th_close_next_10th.ja.cal" date 締め日` — looked at by 開発 on 2026-10-04 (`sha256:5d97c87d3ce26490` → `sha256:74f19beadf2f4043`). State: as looked at

Met by `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` — looked at by 開発 on 2026-10-04 (`sha256:5d97c87d3ce26490` → `sha256:feb9535fb9350ecc`). State: as looked at

Checked by `koyomi "payment_20th_close_next_10th.ja.cal" claim 受領から60日以内` — looked at by 開発 on 2026-10-04 (`sha256:5d97c87d3ce26490` → `sha256:57f704e64ea8e3c8`). State: as looked at

### 営業日 (business_days)

**支払は営業日にする。支払日が土曜、日曜、祝日なら、その前の営業日に払う**

- In force: 2026-10-01..
- Owner: 経理
- File: `examples/payment_terms/payment_terms.ja.req:23`
- The requirement's end: `sha256:cc3109016d422ecd`

Comes from `@祝日` — looked at by 経理 on 2026-10-04 (`sha256:cec37a743c96995c` → `sha256:cc3109016d422ecd`). State: as looked at

Met by `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` — looked at by 開発 on 2026-10-04 (`sha256:cc3109016d422ecd` → `sha256:feb9535fb9350ecc`). State: as looked at

Checked by `koyomi "payment_20th_close_next_10th.ja.cal" claim 営業日に払う` — looked at by 開発 on 2026-10-04 (`sha256:cc3109016d422ecd` → `sha256:3cf8fc3fb2dda9d9`). State: as looked at

## Scope

`scope koyomi "payment_20th_close_next_10th.ja.cal" date` — 2 artifacts in it, every one tracing to a requirement.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-04 | 開発 | `支払日 (payment_day)` → `koyomi "payment_20th_close_next_10th.ja.cal" date 締め日` | `sha256:5d97c87d3ce26490 -> sha256:74f19beadf2f4043` |
| 2026-10-04 | 開発 | `支払日 (payment_day)` → `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `sha256:5d97c87d3ce26490 -> sha256:feb9535fb9350ecc` |
| 2026-10-04 | 開発 | `支払日 (payment_day)` → `koyomi "payment_20th_close_next_10th.ja.cal" claim 受領から60日以内` | `sha256:5d97c87d3ce26490 -> sha256:57f704e64ea8e3c8` |
| 2026-10-04 | 経理 | `@祝日` → `営業日 (business_days)` | `sha256:cec37a743c96995c -> sha256:cc3109016d422ecd` |
| 2026-10-04 | 開発 | `営業日 (business_days)` → `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `sha256:cc3109016d422ecd -> sha256:feb9535fb9350ecc` |
| 2026-10-04 | 開発 | `営業日 (business_days)` → `koyomi "payment_20th_close_next_10th.ja.cal" claim 営業日に払う` | `sha256:cc3109016d422ecd -> sha256:3cf8fc3fb2dda9d9` |
