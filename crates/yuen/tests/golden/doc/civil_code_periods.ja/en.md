# 民法の期間 — where the requirements come from

yuen 0.23.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `civil_code_periods.ja.req` (民法の期間 v1, `sha256:810f491b0587c912`)

The check: `examples/civil_code_periods/civil_code_periods.ja.req: ok — 3 requirements, whose 7 links and 2 waivers are as they were looked at; every requirement is met and checked, or waived; the 3 dates in scope all trace to a requirement`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `起算日 (first_day)` | 2026-10-01.. | `@民法 第140条` | 法務 | `koyomi "civil_code_period_end.ja.cal" date 起算日` | waived | as looked at (1 waived) |
| `満了日 (last_day)` | 2026-10-01.. | `@民法 第141条, 第143条` | 法務 | `koyomi "civil_code_period_end.ja.cal" date 満了日` | waived | as looked at (1 waived) |
| `満了日_142条 (last_day_142)` | 2026-10-01.. | `@民法 第142条`; decided by 法務 on 2026-10-03 | 法務 | `koyomi "civil_code_period_end.ja.cal" date 満了日_142条` | `koyomi "civil_code_period_end.ja.cal" claim 142条の満了日は満了日以後` | as looked at |

## Sources

### 民法

The law `129AC0000000089` in e-Gov, as of 2026-10-01, version `129AC0000000089_20260624_508AC0000000045` (revision.txt beside the copies). Borrowed from `koyomi "civil_code_period_end.ja.cal" source 民法` (the copies and the pins are that file's, and its language's check holds them).

| Article | Pin | Cited by | Pinned by the artifacts |
|---|---|---|---|
| `第140条` | `sha256:e880059021fbb67d` | `起算日 (first_day)` | `koyomi "civil_code_period_end.ja.cal"` |
| `第141条` | `sha256:0575c131b9f08063` | `満了日 (last_day)` | `koyomi "civil_code_period_end.ja.cal"` |
| `第142条` | `sha256:fc8c35a0769d3b35` | `満了日_142条 (last_day_142)` | `koyomi "civil_code_period_end.ja.cal"` |
| `第143条` | `sha256:6950bdfb988439b6` | `満了日 (last_day)` | `koyomi "civil_code_period_end.ja.cal"` |

## Why each requirement is so

### 起算日 (first_day)

**日、週、月又は年によって期間を定めたときは、期間の初日は、算入しない。ただし、その期間が午前零時から始まるときは、この限りでない**

- In force: 2026-10-01..
- Owner: 法務
- File: `examples/civil_code_periods/civil_code_periods.ja.req:11`
- The requirement's end: `sha256:a9ebc73907faddc8`

Comes from `@民法 第140条` — looked at by 法務 on 2026-10-04 (`sha256:e880059021fbb67d` → `sha256:a9ebc73907faddc8`). State: as looked at

> **民法 第140条 (e-Gov 129AC0000000089, the copy as of 2026-10-01)**
>
> 第百四十条　日、週、月又は年によって期間を定めたときは、期間の初日は、算入しない。ただし、その期間が午前零時から始まるときは、この限りでない。

Met by `koyomi "civil_code_period_end.ja.cal" date 起算日` — looked at by 開発 on 2026-10-04 (`sha256:a9ebc73907faddc8` → `sha256:15aa6c91d6aaae80`). State: as looked at

Nothing checks it (waived): 初日を算入しないことは、koyomi doc のページで、条文と言い直しを見比べて確かめる。この例に、それを確かめる条件は無い — approved by 法務 on 2026-10-04 (`sha256:a9ebc73907faddc8`). State: as looked at

### 満了日 (last_day)

**月によって期間を定めたときは、最後の月においてその起算日に応当する日の前日の終わりに満了する。応当する日がないときは、その月の末日の終わりに満了する**

- In force: 2026-10-01..
- Owner: 法務
- File: `examples/civil_code_periods/civil_code_periods.ja.req:22`
- The requirement's end: `sha256:465b83ed8c251406`

Comes from `@民法 第141条, 第143条` — looked at by 法務 on 2026-10-04 (`sha256:0575c131b9f08063, sha256:6950bdfb988439b6` → `sha256:465b83ed8c251406`). State: as looked at

> **民法 第141条 (e-Gov 129AC0000000089, the copy as of 2026-10-01)**
>
> （期間の満了）
>
> 第百四十一条　前条の場合には、期間は、その末日の終了をもって満了する。

> **民法 第143条 (e-Gov 129AC0000000089, the copy as of 2026-10-01)**
>
> （暦による期間の計算）
>
> 第百四十三条　週、月又は年によって期間を定めたときは、その期間は、暦に従って計算する。
>
> ２　週、月又は年の初めから期間を起算しないときは、その期間は、最後の週、月又は年においてその起算日に応当する日の前日に満了する。ただし、月又は年によって期間を定めた場合において、最後の月に応当する日がないときは、その月の末日に満了する。

Met by `koyomi "civil_code_period_end.ja.cal" date 満了日` — looked at by 開発 on 2026-10-04 (`sha256:465b83ed8c251406` → `sha256:23441fd408f07548`). State: as looked at

Nothing checks it (waived): 条文の場合分けをそのまま書いた関数と、起点 2000〜2027 年のすべての日と月数 1〜12 で一致することを、koyomi の設計の段階の試作が確かめた。この例の中に、それを確かめる条件は無い — approved by 法務 on 2026-10-04 (`sha256:465b83ed8c251406`). State: as looked at

### 満了日_142条 (last_day_142)

**期間の末日が日曜日、国民の祝日に関する法律に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する**

- In force: 2026-10-01..
- Owner: 法務
- File: `examples/civil_code_periods/civil_code_periods.ja.req:33`
- The requirement's end: `sha256:d4f2d2a67322df17`

Comes from `@民法 第142条` — looked at by 法務 on 2026-10-04 (`sha256:fc8c35a0769d3b35` → `sha256:d4f2d2a67322df17`). State: as looked at

> **民法 第142条 (e-Gov 129AC0000000089, the copy as of 2026-10-01)**
>
> 第百四十二条　期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。

Decided by 法務 on 2026-10-03: 「その翌日」は文字どおり末日の翌日と読み、翌日も休みでもそれ以上は動かさない。休みが明けるまで動かす読み方とは、起点 2026 年と月数 1〜12 の 4,380 通りのうち 121 通りで分かれる。この例のために決めたもので、法令の読み方を示すものではない

Met by `koyomi "civil_code_period_end.ja.cal" date 満了日_142条` — looked at by 開発 on 2026-10-04 (`sha256:d4f2d2a67322df17` → `sha256:bba4761410179e6e`). State: as looked at

Checked by `koyomi "civil_code_period_end.ja.cal" claim 142条の満了日は満了日以後` — looked at by 開発 on 2026-10-04 (`sha256:d4f2d2a67322df17` → `sha256:7c616f5dcc9503a8`). State: as looked at

## Scope

`scope koyomi "civil_code_period_end.ja.cal" date` — 3 artifacts in it, every one tracing to a requirement.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-04 | 法務 | `@民法 第140条` → `起算日 (first_day)` | `sha256:e880059021fbb67d -> sha256:a9ebc73907faddc8` |
| 2026-10-04 | 開発 | `起算日 (first_day)` → `koyomi "civil_code_period_end.ja.cal" date 起算日` | `sha256:a9ebc73907faddc8 -> sha256:15aa6c91d6aaae80` |
| 2026-10-04 | 法務 | `起算日 (first_day)`: nothing checks it, approved | `sha256:a9ebc73907faddc8` |
| 2026-10-04 | 法務 | `@民法 第141条, 第143条` → `満了日 (last_day)` | `sha256:0575c131b9f08063, sha256:6950bdfb988439b6 -> sha256:465b83ed8c251406` |
| 2026-10-04 | 開発 | `満了日 (last_day)` → `koyomi "civil_code_period_end.ja.cal" date 満了日` | `sha256:465b83ed8c251406 -> sha256:23441fd408f07548` |
| 2026-10-04 | 法務 | `満了日 (last_day)`: nothing checks it, approved | `sha256:465b83ed8c251406` |
| 2026-10-04 | 法務 | `@民法 第142条` → `満了日_142条 (last_day_142)` | `sha256:fc8c35a0769d3b35 -> sha256:d4f2d2a67322df17` |
| 2026-10-04 | 開発 | `満了日_142条 (last_day_142)` → `koyomi "civil_code_period_end.ja.cal" date 満了日_142条` | `sha256:d4f2d2a67322df17 -> sha256:bba4761410179e6e` |
| 2026-10-04 | 開発 | `満了日_142条 (last_day_142)` → `koyomi "civil_code_period_end.ja.cal" claim 142条の満了日は満了日以後` | `sha256:d4f2d2a67322df17 -> sha256:7c616f5dcc9503a8` |
