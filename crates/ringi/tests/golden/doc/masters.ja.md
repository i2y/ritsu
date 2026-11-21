<!-- ringi 0.55.2 が shared/masters.ringi（sha256:a69d7e3d8e1df8c8）から生成したページです。読むためのもので、もとになるのは .ringi のほうです。ここを編集しても .ringi には戻せません。 -->
# company_masters

The lists the requests choose from or look up approvers in (fictional): account codes, vendors and the company's systems with their owners. Each row says from when and until when it may be chosen

ファイル：`shared/masters.ringi`（マスタ `company_masters` の版 1、sha256:a69d7e3d8e1df8c8）

組織は、置いてある版のそれぞれ（2026-04-01、2027-04-01 から）で読んでいます。

## `accounts`

行を見分けるキーは `code` です。行を選んだ申請は、その値を持ちます。

| フィールド | 型 | 受け付ける値 |
|---|---|---|
| `code` | 一行の文 | 10字まで |
| `label` | 一行の文 | 60字まで |

`../masters/accounts.csv` から 4 行を読んでいます。行は、始まりの日から終わりの日まで選べます。

| `code` | `label` | から | まで |
|---|---|---|---|
| 5101 | Travel | 2026-01-01（木） | 終わり無し |
| 5102 | Meetings | 2026-01-01（木） | 終わり無し |
| 5103 | Supplies | 2026-01-01（木） | 2026-12-31（木） |
| 5110 | Supplies and small equipment | 2027-01-01（金） | 終わり無し |

## `vendors`

行を見分けるキーは `id` です。行を選んだ申請は、その値を持ちます。

| フィールド | 型 | 受け付ける値 |
|---|---|---|
| `id` | 一行の文 | 12字まで |
| `name` | 一行の文 | 100字まで |

`../masters/vendors.csv` から 3 行を読んでいます。行は、始まりの日から終わりの日まで選べます。

| `id` | `name` | から | まで |
|---|---|---|---|
| V-0012 | Office supplies vendor (fictional) | 2026-01-01（木） | 終わり無し |
| V-0031 | Machine maker (fictional) | 2026-01-01（木） | 終わり無し |
| V-0044 | Software reseller (fictional) | 2026-04-01（水） | 2027-03-31（水） |

## `systems`

行を見分けるキーは `id` です。行を選んだ申請は、その値を持ちます。

| フィールド | 型 | 受け付ける値 |
|---|---|---|
| `id` | 一行の文 | 30字まで |
| `name` | 一行の文 | 100字まで |
| `owner` | 組織の人 | — |

`../masters/systems.csv` から 3 行を読んでいます。行は、始まりの日から終わりの日まで選べます。

| `id` | `name` | `owner` | から | まで |
|---|---|---|---|---|
| erp | Accounting system | e1001（経理部長） | 2026-01-01（木） | 終わり無し |
| crm | Customer system | e3101（営業一部長） | 2026-01-01（木） | 終わり無し |
| wiki | Company wiki | e7001（情報システム部長） | 2026-01-01（木） | 終わり無し |
