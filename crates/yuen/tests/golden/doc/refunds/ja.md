# refunds — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.25.0 が作った。

- `refunds.req`（refunds v1、`sha256:e9f310e314736b47`）

検査の結果：`examples/refunds/refunds.req: ok — 要件 1 件のリンク 4 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があり、範囲の transfer 2 個は、どれも要件に辿れます。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `within_sales` | 2026-10-01.. | 2026-10-04 に accounting が決めた | accounting | `chobo "refunds.book" account refundable`、`chobo "refunds.book" transfer sale`、`chobo "refunds.book" transfer refund` | `chobo "refunds.book"` | 確かめたまま |

## 出典

出典の宣言は無い。どの要件も、決めたことか、ほかの要件から来ている。

## 要件ごとの「なぜ」

### within_sales

**A refund does not exceed the sale of its order**

- 期間：2026-10-01..
- 持ち主：accounting
- ファイル：`examples/refunds/refunds.req:9`
- 要件のハッシュ：`sha256:29fcc8e8216d5dac`

2026-10-04 に accounting が決めた：What is refunded on an order never exceeds what was sold on it. Decided for this example

満たすもの `chobo "refunds.book" account refundable` — development が 2026-10-04 に確かめた（`sha256:29fcc8e8216d5dac` → `sha256:49d9b3dd2d4bf83b`）。状態：確かめたまま

満たすもの `chobo "refunds.book" transfer sale` — development が 2026-10-04 に確かめた（`sha256:29fcc8e8216d5dac` → `sha256:699a4feaa65a1dfe`）。状態：確かめたまま

満たすもの `chobo "refunds.book" transfer refund` — development が 2026-10-04 に確かめた（`sha256:29fcc8e8216d5dac` → `sha256:d4eb1e0e94a3b78f`）。状態：確かめたまま

確かめるもの `chobo "refunds.book"` — development が 2026-10-04 に確かめた（`sha256:29fcc8e8216d5dac` → `sha256:3b8bf7fc7a202c38`）。状態：確かめたまま

## 範囲

`scope chobo "refunds.book" transfer` — 成果物は 2 個で、どれも要件に辿れる。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book" account refundable` | `sha256:29fcc8e8216d5dac -> sha256:49d9b3dd2d4bf83b` |
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book" transfer sale` | `sha256:29fcc8e8216d5dac -> sha256:699a4feaa65a1dfe` |
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book" transfer refund` | `sha256:29fcc8e8216d5dac -> sha256:d4eb1e0e94a3b78f` |
| 2026-10-04 | development | `within_sales` → `chobo "refunds.book"` | `sha256:29fcc8e8216d5dac -> sha256:3b8bf7fc7a202c38` |
