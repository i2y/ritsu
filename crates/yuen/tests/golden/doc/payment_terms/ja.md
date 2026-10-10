# payment_terms — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.26.0 が作った。

- `payment_terms.req`（payment_terms v1、`sha256:7601e66a63843869`）

検査の結果：`examples/payment_terms/payment_terms.req: ok — 要件 2 件のリンク 6 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があり、範囲の date 2 個は、どれも要件に辿れます。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `payment_day` | 2026-10-01.. | 2026-10-04 に accounting が決めた | accounting | `koyomi "payment_20th_close_next_10th.cal" date closing`、`koyomi "payment_20th_close_next_10th.cal" date payment` | `koyomi "payment_20th_close_next_10th.cal" claim within_60_days_of_receipt` | 確かめたまま |
| `business_days` | 2026-10-01.. | `@holidays` | accounting | `koyomi "payment_20th_close_next_10th.cal" date payment` | `koyomi "payment_20th_close_next_10th.cal" claim paid_on_a_business_day` | 確かめたまま |

## 出典

### holidays

ファイル `calendars/data/syukujitsu.csv`（元は `https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv`）、固定は `sha256:cec37a743c96995c`。借りた先：`koyomi "calendars/tokyo_business_days.cal" source national_holidays`（コピーと固定はそのファイルのもので、その言語の検査が確かめる）。

引く要件：`business_days`

## 要件ごとの「なぜ」

### payment_day

**Invoices close on the 20th and are paid on the 10th of the next month, within 60 days of receipt**

- 期間：2026-10-01..
- 持ち主：accounting
- ファイル：`examples/payment_terms/payment_terms.req:11`
- 要件のハッシュ：`sha256:ce2048eb0cb0cf01`

2026-10-04 に accounting が決めた：Closing on the 20th and paying on the 10th of the next month keeps every payment within 60 days of receipt. Decided for this example

満たすもの `koyomi "payment_20th_close_next_10th.cal" date closing` — development が 2026-10-04 に確かめた（`sha256:ce2048eb0cb0cf01` → `sha256:64153f3ffeeabb6b`）。状態：確かめたまま

満たすもの `koyomi "payment_20th_close_next_10th.cal" date payment` — development が 2026-10-04 に確かめた（`sha256:ce2048eb0cb0cf01` → `sha256:77727589d8318079`）。状態：確かめたまま

確かめるもの `koyomi "payment_20th_close_next_10th.cal" claim within_60_days_of_receipt` — development が 2026-10-04 に確かめた（`sha256:ce2048eb0cb0cf01` → `sha256:39afffc0504e0754`）。状態：確かめたまま

### business_days

**A payment is made on a business day: when the payment day is a Saturday, a Sunday or a national holiday, it is made on the business day before**

- 期間：2026-10-01..
- 持ち主：accounting
- ファイル：`examples/payment_terms/payment_terms.req:23`
- 要件のハッシュ：`sha256:d53e37ddddbb71fa`

出どころ `@holidays` — accounting が 2026-10-04 に確かめた（`sha256:cec37a743c96995c` → `sha256:d53e37ddddbb71fa`）。状態：確かめたまま

満たすもの `koyomi "payment_20th_close_next_10th.cal" date payment` — development が 2026-10-04 に確かめた（`sha256:d53e37ddddbb71fa` → `sha256:77727589d8318079`）。状態：確かめたまま

確かめるもの `koyomi "payment_20th_close_next_10th.cal" claim paid_on_a_business_day` — development が 2026-10-04 に確かめた（`sha256:d53e37ddddbb71fa` → `sha256:a30c094c89e283df`）。状態：確かめたまま

## 範囲

`scope koyomi "payment_20th_close_next_10th.cal" date` — 成果物は 2 個で、どれも要件に辿れる。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-04 | development | `payment_day` → `koyomi "payment_20th_close_next_10th.cal" date closing` | `sha256:ce2048eb0cb0cf01 -> sha256:64153f3ffeeabb6b` |
| 2026-10-04 | development | `payment_day` → `koyomi "payment_20th_close_next_10th.cal" date payment` | `sha256:ce2048eb0cb0cf01 -> sha256:77727589d8318079` |
| 2026-10-04 | development | `payment_day` → `koyomi "payment_20th_close_next_10th.cal" claim within_60_days_of_receipt` | `sha256:ce2048eb0cb0cf01 -> sha256:39afffc0504e0754` |
| 2026-10-04 | accounting | `@holidays` → `business_days` | `sha256:cec37a743c96995c -> sha256:d53e37ddddbb71fa` |
| 2026-10-04 | development | `business_days` → `koyomi "payment_20th_close_next_10th.cal" date payment` | `sha256:d53e37ddddbb71fa -> sha256:77727589d8318079` |
| 2026-10-04 | development | `business_days` → `koyomi "payment_20th_close_next_10th.cal" claim paid_on_a_business_day` | `sha256:d53e37ddddbb71fa -> sha256:a30c094c89e283df` |
