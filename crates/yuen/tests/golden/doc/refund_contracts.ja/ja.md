# 返金の契約 — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.26.0 が作った。

- `refund_contracts.ja.req`（返金の契約 v1、`sha256:4738e5072c85cd5d`）

検査の結果：`examples/refund_contracts/refund_contracts.ja.req: ok — 要件 4 件のリンク 5 本と見送り 4 件が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があります。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `注文を返金する (refund_an_order)` | — | 2026-10-06 に 決済 が決めた | 決済 | `openapi "api/注文.yaml" operation 返金する`、`cedar "policies/店.cedarschema" action 返金` | 見送り | 確かめたまま（見送り 1 件） |
| `返金は円で (refund_in_yen)` | — | 2026-10-06 に 決済 が決めた | 決済 | `openapi "api/注文.yaml" schema 返金 property 金額` | 見送り | 確かめたまま（見送り 1 件） |
| `係には上限がある (clerks_have_a_limit)` | — | 2026-10-06 に 決済 が決めた | 決済 | `cedar "policies/返金.cedar" policy 係は自分の上限まで返金できる` | 見送り | 確かめたまま（見送り 1 件） |
| `返金をほかに知らせる (others_hear_of_a_refund)` | — | 2026-10-06 に 決済 が決めた | 決済 | `asyncapi "events/注文.yaml" operation 返金を知らせる` | 見送り | 確かめたまま（見送り 1 件） |

## 出典

出典の宣言は無い。どの要件も、決めたことか、ほかの要件から来ている。

## 要件ごとの「なぜ」

### 注文を返金する (refund_an_order)

**店の人が、注文の一部か全部を、API で返金する**

- 期間：—
- 持ち主：決済
- ファイル：`examples/refund_contracts/refund_contracts.ja.req:7`
- 要件のハッシュ：`sha256:718c7d1fa3f18e6a`

2026-10-06 に 決済 が決めた：店は自分の API で返金する

満たすもの `openapi "api/注文.yaml" operation 返金する` — 開発 が 2026-10-06 に確かめた（`sha256:718c7d1fa3f18e6a` → `sha256:ce072e77d7b9291b`）。状態：確かめたまま

満たすもの `cedar "policies/店.cedarschema" action 返金` — 開発 が 2026-10-06 に確かめた（`sha256:718c7d1fa3f18e6a` → `sha256:7af4e4d13df4b756`）。状態：確かめたまま

確かめるものを置かない（見送り）：この例には API を呼ぶ主張が無い — 開発 が 2026-10-06 に承認した（`sha256:718c7d1fa3f18e6a`）。状態：確かめたまま

### 返金は円で (refund_in_yen)

**返金は 1 円以上 10 万円以下の、円の整数である**

- 期間：—
- 持ち主：決済
- ファイル：`examples/refund_contracts/refund_contracts.ja.req:18`
- 要件のハッシュ：`sha256:5fe0ddfe3e950118`

2026-10-06 に 決済 が決めた：10 万円を超える返金は銀行を通す

満たすもの `openapi "api/注文.yaml" schema 返金 property 金額` — 開発 が 2026-10-06 に確かめた（`sha256:5fe0ddfe3e950118` → `sha256:e98e692466066f55`）。状態：確かめたまま

確かめるものを置かない（見送り）：この例には API を呼ぶ主張が無い — 開発 が 2026-10-06 に承認した（`sha256:5fe0ddfe3e950118`）。状態：確かめたまま

### 係には上限がある (clerks_have_a_limit)

**係は、自分の上限を超えて返金しない**

- 期間：—
- 持ち主：決済
- ファイル：`examples/refund_contracts/refund_contracts.ja.req:27`
- 要件のハッシュ：`sha256:c436ea13e448749c`

2026-10-06 に 決済 が決めた：係ごとに上限があり、店長が決める

満たすもの `cedar "policies/返金.cedar" policy 係は自分の上限まで返金できる` — 開発 が 2026-10-06 に確かめた（`sha256:c436ea13e448749c` → `sha256:4d2087493a79024f`）。状態：確かめたまま

確かめるものを置かない（見送り）：この例には Cedar に尋ねる主張が無い — 開発 が 2026-10-06 に承認した（`sha256:c436ea13e448749c`）。状態：確かめたまま

### 返金をほかに知らせる (others_hear_of_a_refund)

**ほかのサービスは、返金のたびに知らせを受ける**

- 期間：—
- 持ち主：決済
- ファイル：`examples/refund_contracts/refund_contracts.ja.req:36`
- 要件のハッシュ：`sha256:0f56f17aa132ee68`

2026-10-06 に 決済 が決めた：出荷と経理が返金に応じて動く

満たすもの `asyncapi "events/注文.yaml" operation 返金を知らせる` — 開発 が 2026-10-06 に確かめた（`sha256:0f56f17aa132ee68` → `sha256:2ec3c65482725512`）。状態：確かめたまま

確かめるものを置かない（見送り）：この例にはイベントを読む主張が無い — 開発 が 2026-10-06 に承認した（`sha256:0f56f17aa132ee68`）。状態：確かめたまま

## 範囲

範囲の宣言は無い。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-06 | 開発 | `注文を返金する (refund_an_order)` → `openapi "api/注文.yaml" operation 返金する` | `sha256:718c7d1fa3f18e6a -> sha256:ce072e77d7b9291b` |
| 2026-10-06 | 開発 | `注文を返金する (refund_an_order)` → `cedar "policies/店.cedarschema" action 返金` | `sha256:718c7d1fa3f18e6a -> sha256:7af4e4d13df4b756` |
| 2026-10-06 | 開発 | `注文を返金する (refund_an_order)` の見送り（確かめるもの）を承認 | `sha256:718c7d1fa3f18e6a` |
| 2026-10-06 | 開発 | `返金は円で (refund_in_yen)` → `openapi "api/注文.yaml" schema 返金 property 金額` | `sha256:5fe0ddfe3e950118 -> sha256:e98e692466066f55` |
| 2026-10-06 | 開発 | `返金は円で (refund_in_yen)` の見送り（確かめるもの）を承認 | `sha256:5fe0ddfe3e950118` |
| 2026-10-06 | 開発 | `係には上限がある (clerks_have_a_limit)` → `cedar "policies/返金.cedar" policy 係は自分の上限まで返金できる` | `sha256:c436ea13e448749c -> sha256:4d2087493a79024f` |
| 2026-10-06 | 開発 | `係には上限がある (clerks_have_a_limit)` の見送り（確かめるもの）を承認 | `sha256:c436ea13e448749c` |
| 2026-10-06 | 開発 | `返金をほかに知らせる (others_hear_of_a_refund)` → `asyncapi "events/注文.yaml" operation 返金を知らせる` | `sha256:0f56f17aa132ee68 -> sha256:2ec3c65482725512` |
| 2026-10-06 | 開発 | `返金をほかに知らせる (others_hear_of_a_refund)` の見送り（確かめるもの）を承認 | `sha256:0f56f17aa132ee68` |
