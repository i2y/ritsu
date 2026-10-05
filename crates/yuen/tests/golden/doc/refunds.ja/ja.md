# 返金 — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.23.0 が作った。

- `refunds.ja.req`（返金 v1、`sha256:4c3fc1d4ffaf75be`）

検査の結果：`examples/refunds/refunds.ja.req: ok — 要件 1 件のリンク 4 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があり、範囲の transfer 2 個は、どれも要件に辿れます。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `売上を超えない (within_sales)` | 2026-10-01.. | 2026-10-04 に 経理 が決めた | 経理 | `chobo "refunds.ja.book" account 返金できる残り`、`chobo "refunds.ja.book" transfer 売上計上`、`chobo "refunds.ja.book" transfer 返金` | `chobo "refunds.ja.book"` | 確かめたまま |

## 出典

出典の宣言は無い。どの要件も、決めたことか、ほかの要件から来ている。

## 要件ごとの「なぜ」

### 売上を超えない (within_sales)

**返金は、その注文の売上を超えない**

- 期間：2026-10-01..
- 持ち主：経理
- ファイル：`examples/refunds/refunds.ja.req:9`
- 要件のハッシュ：`sha256:5c5082d4aa498fe8`

2026-10-04 に 経理 が決めた：注文ごとに、返金の額は売上の額を超えない。この例のために決めたもの

満たすもの `chobo "refunds.ja.book" account 返金できる残り` — 開発 が 2026-10-04 に確かめた（`sha256:5c5082d4aa498fe8` → `sha256:9f9b0d74872f62a4`）。状態：確かめたまま

満たすもの `chobo "refunds.ja.book" transfer 売上計上` — 開発 が 2026-10-04 に確かめた（`sha256:5c5082d4aa498fe8` → `sha256:851ab806078168fe`）。状態：確かめたまま

満たすもの `chobo "refunds.ja.book" transfer 返金` — 開発 が 2026-10-04 に確かめた（`sha256:5c5082d4aa498fe8` → `sha256:84e9ce254075c697`）。状態：確かめたまま

確かめるもの `chobo "refunds.ja.book"` — 開発 が 2026-10-04 に確かめた（`sha256:5c5082d4aa498fe8` → `sha256:b2daf81c87ff979e`）。状態：確かめたまま

## 範囲

`scope chobo "refunds.ja.book" transfer` — 成果物は 2 個で、どれも要件に辿れる。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book" account 返金できる残り` | `sha256:5c5082d4aa498fe8 -> sha256:9f9b0d74872f62a4` |
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book" transfer 売上計上` | `sha256:5c5082d4aa498fe8 -> sha256:851ab806078168fe` |
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book" transfer 返金` | `sha256:5c5082d4aa498fe8 -> sha256:84e9ce254075c697` |
| 2026-10-04 | 開発 | `売上を超えない (within_sales)` → `chobo "refunds.ja.book"` | `sha256:5c5082d4aa498fe8 -> sha256:b2daf81c87ff979e` |
