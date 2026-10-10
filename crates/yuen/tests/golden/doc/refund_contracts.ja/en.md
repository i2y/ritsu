# 返金の契約 — where the requirements come from

yuen 0.26.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `refund_contracts.ja.req` (返金の契約 v1, `sha256:4738e5072c85cd5d`)

The check: `examples/refund_contracts/refund_contracts.ja.req: ok — 4 requirements, whose 5 links and 4 waivers are as they were looked at; every requirement is met and checked, or waived`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `注文を返金する (refund_an_order)` | — | decided by 決済 on 2026-10-06 | 決済 | `openapi "api/注文.yaml" operation 返金する`; `cedar "policies/店.cedarschema" action 返金` | waived | as looked at (1 waived) |
| `返金は円で (refund_in_yen)` | — | decided by 決済 on 2026-10-06 | 決済 | `openapi "api/注文.yaml" schema 返金 property 金額` | waived | as looked at (1 waived) |
| `係には上限がある (clerks_have_a_limit)` | — | decided by 決済 on 2026-10-06 | 決済 | `cedar "policies/返金.cedar" policy 係は自分の上限まで返金できる` | waived | as looked at (1 waived) |
| `返金をほかに知らせる (others_hear_of_a_refund)` | — | decided by 決済 on 2026-10-06 | 決済 | `asyncapi "events/注文.yaml" operation 返金を知らせる` | waived | as looked at (1 waived) |

## Sources

No source is declared: every requirement comes from a decision or from another requirement.

## Why each requirement is so

### 注文を返金する (refund_an_order)

**店の人が、注文の一部か全部を、API で返金する**

- In force: —
- Owner: 決済
- File: `examples/refund_contracts/refund_contracts.ja.req:7`
- The requirement's end: `sha256:718c7d1fa3f18e6a`

Decided by 決済 on 2026-10-06: 店は自分の API で返金する

Met by `openapi "api/注文.yaml" operation 返金する` — looked at by 開発 on 2026-10-06 (`sha256:718c7d1fa3f18e6a` → `sha256:ce072e77d7b9291b`). State: as looked at

Met by `cedar "policies/店.cedarschema" action 返金` — looked at by 開発 on 2026-10-06 (`sha256:718c7d1fa3f18e6a` → `sha256:7af4e4d13df4b756`). State: as looked at

Nothing checks it (waived): この例には API を呼ぶ主張が無い — approved by 開発 on 2026-10-06 (`sha256:718c7d1fa3f18e6a`). State: as looked at

### 返金は円で (refund_in_yen)

**返金は 1 円以上 10 万円以下の、円の整数である**

- In force: —
- Owner: 決済
- File: `examples/refund_contracts/refund_contracts.ja.req:18`
- The requirement's end: `sha256:5fe0ddfe3e950118`

Decided by 決済 on 2026-10-06: 10 万円を超える返金は銀行を通す

Met by `openapi "api/注文.yaml" schema 返金 property 金額` — looked at by 開発 on 2026-10-06 (`sha256:5fe0ddfe3e950118` → `sha256:e98e692466066f55`). State: as looked at

Nothing checks it (waived): この例には API を呼ぶ主張が無い — approved by 開発 on 2026-10-06 (`sha256:5fe0ddfe3e950118`). State: as looked at

### 係には上限がある (clerks_have_a_limit)

**係は、自分の上限を超えて返金しない**

- In force: —
- Owner: 決済
- File: `examples/refund_contracts/refund_contracts.ja.req:27`
- The requirement's end: `sha256:c436ea13e448749c`

Decided by 決済 on 2026-10-06: 係ごとに上限があり、店長が決める

Met by `cedar "policies/返金.cedar" policy 係は自分の上限まで返金できる` — looked at by 開発 on 2026-10-06 (`sha256:c436ea13e448749c` → `sha256:4d2087493a79024f`). State: as looked at

Nothing checks it (waived): この例には Cedar に尋ねる主張が無い — approved by 開発 on 2026-10-06 (`sha256:c436ea13e448749c`). State: as looked at

### 返金をほかに知らせる (others_hear_of_a_refund)

**ほかのサービスは、返金のたびに知らせを受ける**

- In force: —
- Owner: 決済
- File: `examples/refund_contracts/refund_contracts.ja.req:36`
- The requirement's end: `sha256:0f56f17aa132ee68`

Decided by 決済 on 2026-10-06: 出荷と経理が返金に応じて動く

Met by `asyncapi "events/注文.yaml" operation 返金を知らせる` — looked at by 開発 on 2026-10-06 (`sha256:0f56f17aa132ee68` → `sha256:2ec3c65482725512`). State: as looked at

Nothing checks it (waived): この例にはイベントを読む主張が無い — approved by 開発 on 2026-10-06 (`sha256:0f56f17aa132ee68`). State: as looked at

## Scope

No scope is declared.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-06 | 開発 | `注文を返金する (refund_an_order)` → `openapi "api/注文.yaml" operation 返金する` | `sha256:718c7d1fa3f18e6a -> sha256:ce072e77d7b9291b` |
| 2026-10-06 | 開発 | `注文を返金する (refund_an_order)` → `cedar "policies/店.cedarschema" action 返金` | `sha256:718c7d1fa3f18e6a -> sha256:7af4e4d13df4b756` |
| 2026-10-06 | 開発 | `注文を返金する (refund_an_order)`: nothing checks it, approved | `sha256:718c7d1fa3f18e6a` |
| 2026-10-06 | 開発 | `返金は円で (refund_in_yen)` → `openapi "api/注文.yaml" schema 返金 property 金額` | `sha256:5fe0ddfe3e950118 -> sha256:e98e692466066f55` |
| 2026-10-06 | 開発 | `返金は円で (refund_in_yen)`: nothing checks it, approved | `sha256:5fe0ddfe3e950118` |
| 2026-10-06 | 開発 | `係には上限がある (clerks_have_a_limit)` → `cedar "policies/返金.cedar" policy 係は自分の上限まで返金できる` | `sha256:c436ea13e448749c -> sha256:4d2087493a79024f` |
| 2026-10-06 | 開発 | `係には上限がある (clerks_have_a_limit)`: nothing checks it, approved | `sha256:c436ea13e448749c` |
| 2026-10-06 | 開発 | `返金をほかに知らせる (others_hear_of_a_refund)` → `asyncapi "events/注文.yaml" operation 返金を知らせる` | `sha256:0f56f17aa132ee68 -> sha256:2ec3c65482725512` |
| 2026-10-06 | 開発 | `返金をほかに知らせる (others_hear_of_a_refund)`: nothing checks it, approved | `sha256:0f56f17aa132ee68` |
