# 支払条件 — 要件の出どころ

このページは yuen 0.23.0 が、下の .req のファイルと、出典の写しと、ほかの言語が成果物について渡すものから作った。

- `payment_terms.ja.req`（支払条件 v1、`sha256:efb13e38decc0120`）

検査の結果：`examples/payment_terms/payment_terms.ja.req: ok — 要件 2 件のリンク 6 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（か、その見送り）があり、範囲の date 2 個は、どれも要件に辿れます。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `支払日 (payment_day)` | 2026-10-01.. | 2026-10-04 に 経理 が決めた | 経理 | `koyomi "payment_20th_close_next_10th.ja.cal" date 締め日`; `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `koyomi "payment_20th_close_next_10th.ja.cal" claim 受領から60日以内` | 確かめたまま |
| `営業日 (business_days)` | 2026-10-01.. | `@祝日` | 経理 | `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `koyomi "payment_20th_close_next_10th.ja.cal" claim 営業日に払う` | 確かめたまま |

## 出典

### 祝日

ファイル `calendars/data/syukujitsu.csv`（元は `https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv`）、固定は `sha256:cec37a743c96995c`。 借りた先：`koyomi "calendars/東京の営業日.cal" source 祝日`（写しと固定は、そのファイルのもの。その言語の検査が確かめる）。

引く要件：`営業日 (business_days)`

## 要件ごとの「なぜ」

### 支払日 (payment_day)

**20 日締め翌月 10 日払い。受領から 60 日以内に払う**

- 期間：2026-10-01..
- 持ち主：経理
- ファイル：`examples/payment_terms/payment_terms.ja.req:11`
- 要件の端：`sha256:5d97c87d3ce26490`

2026-10-04 に 経理 が決めた：20 日に締めて翌月 10 日に払えば、どの支払も受領から 60 日以内に収まる。この例のために決めたもの

満たすもの `koyomi "payment_20th_close_next_10th.ja.cal" date 締め日` — 開発 が 2026-10-04 に確かめた（`sha256:5d97c87d3ce26490` → `sha256:74f19beadf2f4043`）。状態：確かめたまま

満たすもの `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` — 開発 が 2026-10-04 に確かめた（`sha256:5d97c87d3ce26490` → `sha256:feb9535fb9350ecc`）。状態：確かめたまま

確かめるもの `koyomi "payment_20th_close_next_10th.ja.cal" claim 受領から60日以内` — 開発 が 2026-10-04 に確かめた（`sha256:5d97c87d3ce26490` → `sha256:57f704e64ea8e3c8`）。状態：確かめたまま

### 営業日 (business_days)

**支払は営業日にする。支払日が土曜、日曜、祝日なら、その前の営業日に払う**

- 期間：2026-10-01..
- 持ち主：経理
- ファイル：`examples/payment_terms/payment_terms.ja.req:23`
- 要件の端：`sha256:cc3109016d422ecd`

出どころ `@祝日` — 経理 が 2026-10-04 に確かめた（`sha256:cec37a743c96995c` → `sha256:cc3109016d422ecd`）。状態：確かめたまま

満たすもの `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` — 開発 が 2026-10-04 に確かめた（`sha256:cc3109016d422ecd` → `sha256:feb9535fb9350ecc`）。状態：確かめたまま

確かめるもの `koyomi "payment_20th_close_next_10th.ja.cal" claim 営業日に払う` — 開発 が 2026-10-04 に確かめた（`sha256:cc3109016d422ecd` → `sha256:3cf8fc3fb2dda9d9`）。状態：確かめたまま

## 範囲

`scope koyomi "payment_20th_close_next_10th.ja.cal" date` — 成果物 2 個。どれも要件に辿れる。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-04 | 開発 | `支払日 (payment_day)` → `koyomi "payment_20th_close_next_10th.ja.cal" date 締め日` | `sha256:5d97c87d3ce26490 -> sha256:74f19beadf2f4043` |
| 2026-10-04 | 開発 | `支払日 (payment_day)` → `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `sha256:5d97c87d3ce26490 -> sha256:feb9535fb9350ecc` |
| 2026-10-04 | 開発 | `支払日 (payment_day)` → `koyomi "payment_20th_close_next_10th.ja.cal" claim 受領から60日以内` | `sha256:5d97c87d3ce26490 -> sha256:57f704e64ea8e3c8` |
| 2026-10-04 | 経理 | `@祝日` → `営業日 (business_days)` | `sha256:cec37a743c96995c -> sha256:cc3109016d422ecd` |
| 2026-10-04 | 開発 | `営業日 (business_days)` → `koyomi "payment_20th_close_next_10th.ja.cal" date 支払日` | `sha256:cc3109016d422ecd -> sha256:feb9535fb9350ecc` |
| 2026-10-04 | 開発 | `営業日 (business_days)` → `koyomi "payment_20th_close_next_10th.ja.cal" claim 営業日に払う` | `sha256:cc3109016d422ecd -> sha256:3cf8fc3fb2dda9d9` |
