# refund_contracts — where the requirements come from

yuen 0.23.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `refund_contracts.req` (refund_contracts v1, `sha256:3769711dc4e8f14e`)

The check: `examples/refund_contracts/refund_contracts.req: ok — 4 requirements, whose 5 links and 4 waivers are as they were looked at; every requirement is met and checked, or waived`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `refund_an_order` | — | decided by payments on 2026-10-06 | payments | `openapi "api/orders.yaml" operation refundOrder`; `cedar "policies/shop.cedarschema" action refund_order` | waived | as looked at (1 waived) |
| `refund_in_pence` | — | decided by payments on 2026-10-06 | payments | `openapi "api/orders.yaml" schema Refund property amount` | waived | as looked at (1 waived) |
| `clerks_have_a_limit` | — | decided by payments on 2026-10-06 | payments | `cedar "policies/refunds.cedar" policy clerks_refund_within_their_limit` | waived | as looked at (1 waived) |
| `others_hear_of_a_refund` | — | decided by payments on 2026-10-06 | payments | `asyncapi "events/orders.yaml" operation sendOrderRefunded` | waived | as looked at (1 waived) |

## Sources

No source is declared: every requirement comes from a decision or from another requirement.

## Why each requirement is so

### refund_an_order

**A member of the staff refunds an order, in part or in whole, through the API**

- In force: —
- Owner: payments
- File: `examples/refund_contracts/refund_contracts.req:7`
- The requirement's end: `sha256:34ed3dfa002d8b12`

Decided by payments on 2026-10-06: The shop refunds through its own API

Met by `openapi "api/orders.yaml" operation refundOrder` — looked at by development on 2026-10-06 (`sha256:34ed3dfa002d8b12` → `sha256:3ce78f035097e095`). State: as looked at

Met by `cedar "policies/shop.cedarschema" action refund_order` — looked at by development on 2026-10-06 (`sha256:34ed3dfa002d8b12` → `sha256:76a958d088eb5d8d`). State: as looked at

Nothing checks it (waived): This example has no claim that calls the API — approved by development on 2026-10-06 (`sha256:34ed3dfa002d8b12`). State: as looked at

### refund_in_pence

**A refund is a whole number of pence, from one penny to one hundred pounds**

- In force: —
- Owner: payments
- File: `examples/refund_contracts/refund_contracts.req:18`
- The requirement's end: `sha256:4dbb88795db91fae`

Decided by payments on 2026-10-06: A refund over one hundred pounds goes through the bank

Met by `openapi "api/orders.yaml" schema Refund property amount` — looked at by development on 2026-10-06 (`sha256:4dbb88795db91fae` → `sha256:55d566e37d48e6f7`). State: as looked at

Nothing checks it (waived): This example has no claim that calls the API — approved by development on 2026-10-06 (`sha256:4dbb88795db91fae`). State: as looked at

### clerks_have_a_limit

**A clerk refunds no more than the clerk's own limit**

- In force: —
- Owner: payments
- File: `examples/refund_contracts/refund_contracts.req:27`
- The requirement's end: `sha256:ac26306142b38a02`

Decided by payments on 2026-10-06: Each clerk has a limit, set by a manager

Met by `cedar "policies/refunds.cedar" policy clerks_refund_within_their_limit` — looked at by development on 2026-10-06 (`sha256:ac26306142b38a02` → `sha256:0c7317f0e764cd08`). State: as looked at

Nothing checks it (waived): This example has no claim that asks Cedar — approved by development on 2026-10-06 (`sha256:ac26306142b38a02`). State: as looked at

### others_hear_of_a_refund

**The other services hear of each refund**

- In force: —
- Owner: payments
- File: `examples/refund_contracts/refund_contracts.req:36`
- The requirement's end: `sha256:c57be8713de0d6db`

Decided by payments on 2026-10-06: Shipping and the accounts act on refunds

Met by `asyncapi "events/orders.yaml" operation sendOrderRefunded` — looked at by development on 2026-10-06 (`sha256:c57be8713de0d6db` → `sha256:80f62fd6c2d2bb47`). State: as looked at

Nothing checks it (waived): This example has no claim that reads the events — approved by development on 2026-10-06 (`sha256:c57be8713de0d6db`). State: as looked at

## Scope

No scope is declared.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-06 | development | `refund_an_order` → `openapi "api/orders.yaml" operation refundOrder` | `sha256:34ed3dfa002d8b12 -> sha256:3ce78f035097e095` |
| 2026-10-06 | development | `refund_an_order` → `cedar "policies/shop.cedarschema" action refund_order` | `sha256:34ed3dfa002d8b12 -> sha256:76a958d088eb5d8d` |
| 2026-10-06 | development | `refund_an_order`: nothing checks it, approved | `sha256:34ed3dfa002d8b12` |
| 2026-10-06 | development | `refund_in_pence` → `openapi "api/orders.yaml" schema Refund property amount` | `sha256:4dbb88795db91fae -> sha256:55d566e37d48e6f7` |
| 2026-10-06 | development | `refund_in_pence`: nothing checks it, approved | `sha256:4dbb88795db91fae` |
| 2026-10-06 | development | `clerks_have_a_limit` → `cedar "policies/refunds.cedar" policy clerks_refund_within_their_limit` | `sha256:ac26306142b38a02 -> sha256:0c7317f0e764cd08` |
| 2026-10-06 | development | `clerks_have_a_limit`: nothing checks it, approved | `sha256:ac26306142b38a02` |
| 2026-10-06 | development | `others_hear_of_a_refund` → `asyncapi "events/orders.yaml" operation sendOrderRefunded` | `sha256:c57be8713de0d6db -> sha256:80f62fd6c2d2bb47` |
| 2026-10-06 | development | `others_hear_of_a_refund`: nothing checks it, approved | `sha256:c57be8713de0d6db` |
