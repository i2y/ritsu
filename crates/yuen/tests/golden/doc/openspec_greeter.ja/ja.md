# 挨拶 — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.24.0 が作った。

- `greeter.ja.req`（挨拶 v1、`sha256:23706b2d5df397b0`）

検査の結果：`examples/openspec_greeter/greeter.ja.req: ok — 要件 3 件のリンク 10 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があります。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `名前で挨拶する (greeting_by_name)` | — | `@挨拶 名前で挨拶する` | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 名前で挨拶する`、`geas "greeter.ja.geas" claim 空の名前は受け付けない` | 確かめたまま |
| `足した数の合計 (running_total)` | — | `@挨拶 足した数の合計` | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 足した数が積み上がる` | 確かめたまま |
| `知らないパス (unknown_paths)` | — | `@挨拶 知らないパス` | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` | 確かめたまま |

## 出典

### 挨拶

OpenSpec の仕様 `ja/openspec/specs/greeting/spec.md`。要件ごとに、そのブロック（見出しからシナリオまで）を固定する。

| 要件 | 固定 | 引く要件 |
|---|---|---|
| 名前で挨拶する | `sha256:584f410eda34b49e` | `名前で挨拶する (greeting_by_name)` |
| 足した数の合計 | `sha256:16b2a7d63d90dcf5` | `足した数の合計 (running_total)` |
| 知らないパス | `sha256:d0b84e0286e74b3a` | `知らないパス (unknown_paths)` |

## 要件ごとの「なぜ」

### 名前で挨拶する (greeting_by_name)

**挨拶は 200 と、Hello と頼まれた名前を返す。空の名前には 400 を返す**

- 期間：—
- 持ち主：窓口
- ファイル：`examples/openspec_greeter/greeter.ja.req:12`
- 要件のハッシュ：`sha256:0fe7e5be49136195`

出どころ `@挨拶 名前で挨拶する` — 窓口 が 2026-10-05 に確かめた（`sha256:584f410eda34b49e` → `sha256:0fe7e5be49136195`）。状態：確かめたまま

> **挨拶 名前で挨拶する（OpenSpec の仕様 examples/openspec_greeter/ja/openspec/specs/greeting/spec.md）**
>
> ### Requirement: 名前で挨拶する
>
> サービスは `GET /greet?name=<名前>` に、ステータス 200 と、`message` が `Hello, <名前>` の JSON で答える（SHALL）。空の名前には、ステータス 400 で答える（SHALL）。
>
> #### Scenario: 名前で挨拶する
>
> - **WHEN** クライアントが `/greet?name=Alice` を求める
>
> - **THEN** ステータスは 200
>
> - **AND** `message` は `Hello, Alice`
>
> #### Scenario: 空の名前は受け付けない
>
> - **WHEN** クライアントが `/greet?name=` を求める
>
> - **THEN** ステータスは 400

満たすもの `file "server.py"` — 開発 が 2026-10-05 に確かめた（`sha256:0fe7e5be49136195` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim 名前で挨拶する` — 開発 が 2026-10-05 に確かめた（`sha256:0fe7e5be49136195` → `sha256:23771933ab7071aa`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim 空の名前は受け付けない` — 開発 が 2026-10-05 に確かめた（`sha256:0fe7e5be49136195` → `sha256:cfde1ffad5721517`）。状態：確かめたまま

### 足した数の合計 (running_total)

**合計はリクエストをまたいで覚えておく。POST /reset で 0 にし、POST /add で足し、GET /total で返す**

- 期間：—
- 持ち主：窓口
- ファイル：`examples/openspec_greeter/greeter.ja.req:24`
- 要件のハッシュ：`sha256:2c9bc244d3620e0c`

出どころ `@挨拶 足した数の合計` — 窓口 が 2026-10-05 に確かめた（`sha256:16b2a7d63d90dcf5` → `sha256:2c9bc244d3620e0c`）。状態：確かめたまま

> **挨拶 足した数の合計（OpenSpec の仕様 examples/openspec_greeter/ja/openspec/specs/greeting/spec.md）**
>
> ### Requirement: 足した数の合計
>
> サービスは、リクエストをまたいで合計を覚えておく（SHALL）。`POST /reset` で 0 にし、`POST /add` で本文の数を足し、`GET /total` で合計を返す。
>
> #### Scenario: 足した数が積み上がる
>
> - **GIVEN** 合計を 0 にした
>
> - **WHEN** クライアントが 5 と 7 を足す
>
> - **THEN** `GET /total` は 12 を返す

満たすもの `file "server.py"` — 開発 が 2026-10-05 に確かめた（`sha256:2c9bc244d3620e0c` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim 足した数が積み上がる` — 開発 が 2026-10-05 に確かめた（`sha256:2c9bc244d3620e0c` → `sha256:851003f8a61e8af4`）。状態：確かめたまま

### 知らないパス (unknown_paths)

**知らないパスには 404 を返す**

- 期間：—
- 持ち主：窓口
- ファイル：`examples/openspec_greeter/greeter.ja.req:34`
- 要件のハッシュ：`sha256:eb33424df22e79c6`

出どころ `@挨拶 知らないパス` — 窓口 が 2026-10-05 に確かめた（`sha256:d0b84e0286e74b3a` → `sha256:eb33424df22e79c6`）。状態：確かめたまま

> **挨拶 知らないパス（OpenSpec の仕様 examples/openspec_greeter/ja/openspec/specs/greeting/spec.md）**
>
> ### Requirement: 知らないパス
>
> サービスは、知らないパスにステータス 404 で答える（MUST）。
>
> #### Scenario: 知らないパスには 404 を返す
>
> - **WHEN** クライアントが `/nope` を求める
>
> - **THEN** ステータスは 404

満たすもの `file "server.py"` — 開発 が 2026-10-05 に確かめた（`sha256:eb33424df22e79c6` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` — 開発 が 2026-10-05 に確かめた（`sha256:eb33424df22e79c6` → `sha256:8788b539c21c495d`）。状態：確かめたまま

## 範囲

範囲の宣言は無い。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-05 | 窓口 | `@挨拶 名前で挨拶する` → `名前で挨拶する (greeting_by_name)` | `sha256:584f410eda34b49e -> sha256:0fe7e5be49136195` |
| 2026-10-05 | 開発 | `名前で挨拶する (greeting_by_name)` → `file "server.py"` | `sha256:0fe7e5be49136195 -> sha256:331e02e26d128e8c` |
| 2026-10-05 | 開発 | `名前で挨拶する (greeting_by_name)` → `geas "greeter.ja.geas" claim 名前で挨拶する` | `sha256:0fe7e5be49136195 -> sha256:23771933ab7071aa` |
| 2026-10-05 | 開発 | `名前で挨拶する (greeting_by_name)` → `geas "greeter.ja.geas" claim 空の名前は受け付けない` | `sha256:0fe7e5be49136195 -> sha256:cfde1ffad5721517` |
| 2026-10-05 | 窓口 | `@挨拶 足した数の合計` → `足した数の合計 (running_total)` | `sha256:16b2a7d63d90dcf5 -> sha256:2c9bc244d3620e0c` |
| 2026-10-05 | 開発 | `足した数の合計 (running_total)` → `file "server.py"` | `sha256:2c9bc244d3620e0c -> sha256:331e02e26d128e8c` |
| 2026-10-05 | 開発 | `足した数の合計 (running_total)` → `geas "greeter.ja.geas" claim 足した数が積み上がる` | `sha256:2c9bc244d3620e0c -> sha256:851003f8a61e8af4` |
| 2026-10-05 | 窓口 | `@挨拶 知らないパス` → `知らないパス (unknown_paths)` | `sha256:d0b84e0286e74b3a -> sha256:eb33424df22e79c6` |
| 2026-10-05 | 開発 | `知らないパス (unknown_paths)` → `file "server.py"` | `sha256:eb33424df22e79c6 -> sha256:331e02e26d128e8c` |
| 2026-10-05 | 開発 | `知らないパス (unknown_paths)` → `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` | `sha256:eb33424df22e79c6 -> sha256:8788b539c21c495d` |
