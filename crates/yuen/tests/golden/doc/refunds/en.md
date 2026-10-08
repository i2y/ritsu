# refunds — where the requirements come from

yuen 0.25.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `refunds.req` (refunds v1, `sha256:e9f310e314736b47`)

The check: `examples/refunds/refunds.req: ok — 1 requirement, whose 4 links are as they were looked at; every requirement is met and checked, or waived; the 2 transfers in scope all trace to a requirement`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `within_sales` | 2026-10-01.. | decided by accounting on 2026-10-04 | accounting | `chobo "refunds.book" account refundable`; `chobo "refunds.book" transfer sale`; `chobo "refunds.book" transfer refund` | `chobo "refunds.book"` | as looked at |

## Sources

No source is declared: every requirement comes from a decision or from another requirement.

## Why each requirement is so

### within_sales

**A refund does not exceed the sale of its order**

- In force: 2026-10-01..
- Owner: accounting
- File: `examples/refunds/refunds.req:9`
- The requirement's end: `sha256:29fcc8e8216d5dac`

Decided by accounting on 2026-10-04: What is refunded on an order never exceeds what was sold on it. Decided for this example

Met by `chobo "refunds.book" account refundable` — looked at by development on 2026-10-04 (`sha256:29fcc8e8216d5dac` → `sha256:49d9b3dd2d4bf83b`). State: as looked at

Met by `chobo "refunds.book" transfer sale` — looked at by development on 2026-10-04 (`sha256:29fcc8e8216d5dac` → `sha256:699a4feaa65a1dfe`). State: as looked at

Met by `chobo "refunds.book" transfer refund` — looked at by development on 2026-10-04 (`sha256:29fcc8e8216d5dac` → `sha256:d4eb1e0e94a3b78f`). State: as looked at

Checked by `chobo "refunds.book"` — looked at by development on 2026-10-04 (`sha256:29fcc8e8216d5dac` → `sha256:3b8bf7fc7a202c38`). State: as looked at

## Scope

`scope chobo "refunds.book" transfer` — 2 artifacts in it, every one tracing to a requirement.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book" account refundable` | `sha256:29fcc8e8216d5dac -> sha256:49d9b3dd2d4bf83b` |
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book" transfer sale` | `sha256:29fcc8e8216d5dac -> sha256:699a4feaa65a1dfe` |
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book" transfer refund` | `sha256:29fcc8e8216d5dac -> sha256:d4eb1e0e94a3b78f` |
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book"` | `sha256:29fcc8e8216d5dac -> sha256:3b8bf7fc7a202c38` |
