## ritsu mod report: `two` → `three`

| Check | Errors | Warnings |
|---|---|---|
| With the lock of `two` | 0 | 0 |
| With the lock of `three` | 1 | 0 |

### New diagnostics (1)

| Severity | Code | File | Line | Message | Module |
|---|---|---|---|---|---|
| error | `dandori E002` | `orders/place_order.flow` | 49 | `shipping.outputs` has no field `fee` (shipping_fee) | `example.com/acme/pricing` |

### `example.com/acme/pricing` (`@pricing`) 1.1.0 → 2.0.0: upgraded

- New diagnostics: 1
- Files: 2 changed, 2 the same
- require: no change

| Change | File |
|---|---|
| changed | `ritsu.mod` |
| changed | `rules/shipping_fee.rule` |

#### Behavior

- `rules/order_total.rule` (rulec): the same answers

<details><summary><code>rules/shipping_fee.rule</code> (rulec): the answers differ</summary>

**Version diff (over the whole input space)**

`@pricing@1.1.0/rules/shipping_fee.rule` → `@pricing@2.0.0/rules/shipping_fee.rule`

- `fee` — an output was removed
- `shipping_fee` — an output was added

| where they differ | answer | example | cells |
|---|---|---|---|
| region = domestic  and  weight >=1g <=2500g | fee: 395GBPc → - | region=domestic, weight=1 | 3 |
| region = world  and  weight >=1g <=2500g | fee: 1850GBPc → - | region=world, weight=1 | 3 |
| region = eu  and  weight >=1g <=2500g | fee: 1250GBPc → - | region=eu, weight=1 | 3 |
| region = domestic  and  weight >=2501g <=30000g | fee: 695GBPc → - | region=domestic, weight=2501 | 2 |
| region = world  and  weight >=2501g <=30000g | fee: 2950GBPc → - | region=world, weight=2501 | 2 |
| region = eu  and  weight >=2501g <=30000g | fee: 1950GBPc → - | region=eu, weight=2501 | 2 |

Of the inputs both versions accept, those outside this region get the same answer.

</details>

#### Interface

- `rules/shipping_fee.rule` (rulec, api): 25 changes, 13 of them may break a user

| Change | Where | Old → new |
|---|---|---|
| may break: removed | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/fee` |  |
| may break: removed | `/go/output_fields/fee` |  |
| may break: removed | `/connect/response_fields/fee` |  |
| may break: changed | `/sql/function/signature` | `"…xt) RETURNS TABLE (\"fee\" bigint, \"fee_by_weight_ro…" → "…xt) RETURNS TABLE (\"shipping_fee\" bigint, \"fee_by_…"` |
| added | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/shipping_fee` |  |
| added | `/go/output_fields/shipping_fee` |  |
| added | `/connect/response_fields/shipping_fee` |  |

A change that may break a user is a candidate: whether it does is for that user's check to say.

A difference is shown in behavior or in an interface.

#### Your lines that read answers that change

- `orders/place_order.flow:7` (dandori, `use rule`): `@pricing/rules/shipping_fee.rule`

dandori and kaname have no diff of behavior: whether the answers of these files change is not compared.

The check with the lock of `three` has 1 error.
