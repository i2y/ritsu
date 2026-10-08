# 返金 — where the requirements come from

yuen 0.25.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `refunds.ja.req` (返金 v1, `sha256:4c3fc1d4ffaf75be`)

The check: `examples/refunds/refunds.ja.req: ok — 1 requirement, whose 4 links are as they were looked at; every requirement is met and checked, or waived; the 2 transfers in scope all trace to a requirement`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `売上を超えない (within_sales)` | 2026-10-01.. | decided by 経理 on 2026-10-04 | 経理 | `chobo "refunds.ja.book" account 返金できる残り`; `chobo "refunds.ja.book" transfer 売上計上`; `chobo "refunds.ja.book" transfer 返金` | `chobo "refunds.ja.book"` | as looked at |

## Sources

No source is declared: every requirement comes from a decision or from another requirement.

## Why each requirement is so

### 売上を超えない (within_sales)

**返金は、その注文の売上を超えない**

- In force: 2026-10-01..
- Owner: 経理
- File: `examples/refunds/refunds.ja.req:9`
- The requirement's end: `sha256:5c5082d4aa498fe8`

Decided by 経理 on 2026-10-04: 注文ごとに、返金の額は売上の額を超えない。この例のために決めたもの

Met by `chobo "refunds.ja.book" account 返金できる残り` — looked at by 開発 on 2026-10-04 (`sha256:5c5082d4aa498fe8` → `sha256:9f9b0d74872f62a4`). State: as looked at

Met by `chobo "refunds.ja.book" transfer 売上計上` — looked at by 開発 on 2026-10-04 (`sha256:5c5082d4aa498fe8` → `sha256:851ab806078168fe`). State: as looked at

Met by `chobo "refunds.ja.book" transfer 返金` — looked at by 開発 on 2026-10-04 (`sha256:5c5082d4aa498fe8` → `sha256:84e9ce254075c697`). State: as looked at

Checked by `chobo "refunds.ja.book"` — looked at by 開発 on 2026-10-04 (`sha256:5c5082d4aa498fe8` → `sha256:b2daf81c87ff979e`). State: as looked at

## Scope

`scope chobo "refunds.ja.book" transfer` — 2 artifacts in it, every one tracing to a requirement.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book" account 返金できる残り` | `sha256:5c5082d4aa498fe8 -> sha256:9f9b0d74872f62a4` |
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book" transfer 売上計上` | `sha256:5c5082d4aa498fe8 -> sha256:851ab806078168fe` |
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book" transfer 返金` | `sha256:5c5082d4aa498fe8 -> sha256:84e9ce254075c697` |
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book"` | `sha256:5c5082d4aa498fe8 -> sha256:b2daf81c87ff979e` |
