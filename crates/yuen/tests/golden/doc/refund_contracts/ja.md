# refund_contracts — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.26.0 が作った。

- `refund_contracts.req`（refund_contracts v1、`sha256:3769711dc4e8f14e`）

検査の結果：`examples/refund_contracts/refund_contracts.req: ok — 要件 4 件のリンク 5 本と見送り 4 件が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があります。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `refund_an_order` | — | 2026-10-06 に payments が決めた | payments | `openapi "api/orders.yaml" operation refundOrder`、`cedar "policies/shop.cedarschema" action refund_order` | 見送り | 確かめたまま（見送り 1 件） |
| `refund_in_pence` | — | 2026-10-06 に payments が決めた | payments | `openapi "api/orders.yaml" schema Refund property amount` | 見送り | 確かめたまま（見送り 1 件） |
| `clerks_have_a_limit` | — | 2026-10-06 に payments が決めた | payments | `cedar "policies/refunds.cedar" policy clerks_refund_within_their_limit` | 見送り | 確かめたまま（見送り 1 件） |
| `others_hear_of_a_refund` | — | 2026-10-06 に payments が決めた | payments | `asyncapi "events/orders.yaml" operation sendOrderRefunded` | 見送り | 確かめたまま（見送り 1 件） |

## 出典

出典の宣言は無い。どの要件も、決めたことか、ほかの要件から来ている。

## 要件ごとの「なぜ」

### refund_an_order

**A member of the staff refunds an order, in part or in whole, through the API**

- 期間：—
- 持ち主：payments
- ファイル：`examples/refund_contracts/refund_contracts.req:7`
- 要件のハッシュ：`sha256:34ed3dfa002d8b12`

2026-10-06 に payments が決めた：The shop refunds through its own API

満たすもの `openapi "api/orders.yaml" operation refundOrder` — development が 2026-10-06 に確かめた（`sha256:34ed3dfa002d8b12` → `sha256:3ce78f035097e095`）。状態：確かめたまま

満たすもの `cedar "policies/shop.cedarschema" action refund_order` — development が 2026-10-06 に確かめた（`sha256:34ed3dfa002d8b12` → `sha256:76a958d088eb5d8d`）。状態：確かめたまま

確かめるものを置かない（見送り）：This example has no claim that calls the API — development が 2026-10-06 に承認した（`sha256:34ed3dfa002d8b12`）。状態：確かめたまま

### refund_in_pence

**A refund is a whole number of pence, from one penny to one hundred pounds**

- 期間：—
- 持ち主：payments
- ファイル：`examples/refund_contracts/refund_contracts.req:18`
- 要件のハッシュ：`sha256:4dbb88795db91fae`

2026-10-06 に payments が決めた：A refund over one hundred pounds goes through the bank

満たすもの `openapi "api/orders.yaml" schema Refund property amount` — development が 2026-10-06 に確かめた（`sha256:4dbb88795db91fae` → `sha256:55d566e37d48e6f7`）。状態：確かめたまま

確かめるものを置かない（見送り）：This example has no claim that calls the API — development が 2026-10-06 に承認した（`sha256:4dbb88795db91fae`）。状態：確かめたまま

### clerks_have_a_limit

**A clerk refunds no more than the clerk's own limit**

- 期間：—
- 持ち主：payments
- ファイル：`examples/refund_contracts/refund_contracts.req:27`
- 要件のハッシュ：`sha256:ac26306142b38a02`

2026-10-06 に payments が決めた：Each clerk has a limit, set by a manager

満たすもの `cedar "policies/refunds.cedar" policy clerks_refund_within_their_limit` — development が 2026-10-06 に確かめた（`sha256:ac26306142b38a02` → `sha256:0c7317f0e764cd08`）。状態：確かめたまま

確かめるものを置かない（見送り）：This example has no claim that asks Cedar — development が 2026-10-06 に承認した（`sha256:ac26306142b38a02`）。状態：確かめたまま

### others_hear_of_a_refund

**The other services hear of each refund**

- 期間：—
- 持ち主：payments
- ファイル：`examples/refund_contracts/refund_contracts.req:36`
- 要件のハッシュ：`sha256:c57be8713de0d6db`

2026-10-06 に payments が決めた：Shipping and the accounts act on refunds

満たすもの `asyncapi "events/orders.yaml" operation sendOrderRefunded` — development が 2026-10-06 に確かめた（`sha256:c57be8713de0d6db` → `sha256:80f62fd6c2d2bb47`）。状態：確かめたまま

確かめるものを置かない（見送り）：This example has no claim that reads the events — development が 2026-10-06 に承認した（`sha256:c57be8713de0d6db`）。状態：確かめたまま

## 範囲

範囲の宣言は無い。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-06 | development | `refund_an_order` → `openapi "api/orders.yaml" operation refundOrder` | `sha256:34ed3dfa002d8b12 -> sha256:3ce78f035097e095` |
| 2026-10-06 | development | `refund_an_order` → `cedar "policies/shop.cedarschema" action refund_order` | `sha256:34ed3dfa002d8b12 -> sha256:76a958d088eb5d8d` |
| 2026-10-06 | development | `refund_an_order` の見送り（確かめるもの）を承認 | `sha256:34ed3dfa002d8b12` |
| 2026-10-06 | development | `refund_in_pence` → `openapi "api/orders.yaml" schema Refund property amount` | `sha256:4dbb88795db91fae -> sha256:55d566e37d48e6f7` |
| 2026-10-06 | development | `refund_in_pence` の見送り（確かめるもの）を承認 | `sha256:4dbb88795db91fae` |
| 2026-10-06 | development | `clerks_have_a_limit` → `cedar "policies/refunds.cedar" policy clerks_refund_within_their_limit` | `sha256:ac26306142b38a02 -> sha256:0c7317f0e764cd08` |
| 2026-10-06 | development | `clerks_have_a_limit` の見送り（確かめるもの）を承認 | `sha256:ac26306142b38a02` |
| 2026-10-06 | development | `others_hear_of_a_refund` → `asyncapi "events/orders.yaml" operation sendOrderRefunded` | `sha256:c57be8713de0d6db -> sha256:80f62fd6c2d2bb47` |
| 2026-10-06 | development | `others_hear_of_a_refund` の見送り（確かめるもの）を承認 | `sha256:c57be8713de0d6db` |
