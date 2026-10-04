# 挨拶 — 要件の出どころ

このページは、下の .req のファイルと、出典の写しと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.23.0 が作った。

- `greeter.ja.req`（挨拶 v1、`sha256:cf339bc81c8e313d`）

検査の結果：`examples/greeter/greeter.ja.req: ok — 要件 4 件のリンク 8 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があり、範囲のファイル 1 個は、要件に辿れます。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `名前で挨拶する (greets_by_name)` | — | 2026-10-04 に 窓口 が決めた | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 名前で挨拶する` | 確かめたまま |
| `空の名前は断る (rejects_an_empty_name)` | — | 2026-10-04 に 窓口 が決めた | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 空の名前は断る` | 確かめたまま |
| `足した数が積み上がる (totals_accumulate)` | — | 2026-10-04 に 窓口 が決めた | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 足した数が積み上がる` | 確かめたまま |
| `知らないパスは404 (unknown_paths_are_404)` | — | 2026-10-04 に 窓口 が決めた | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` | 確かめたまま |

## 出典

出典の宣言は無い。どの要件も、決めたことか、ほかの要件から来ている。

## 要件ごとの「なぜ」

### 名前で挨拶する (greets_by_name)

**GET /greet?name=<名前> は 200 と、メッセージ Hello, <名前> を返す**

- 期間：—
- 持ち主：窓口
- ファイル：`examples/greeter/greeter.ja.req:9`
- 要件のハッシュ：`sha256:e87a6fee4807cbad`

2026-10-04 に 窓口 が決めた：挨拶には、頼んだ人の名前を入れる。この例のために決めたもの

満たすもの `file "server.py"` — 開発 が 2026-10-04 に確かめた（`sha256:e87a6fee4807cbad` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim 名前で挨拶する` — 開発 が 2026-10-04 に確かめた（`sha256:e87a6fee4807cbad` → `sha256:23771933ab7071aa`）。状態：確かめたまま

### 空の名前は断る (rejects_an_empty_name)

**名前が空の GET /greet は 400 を返す**

- 期間：—
- 持ち主：窓口
- ファイル：`examples/greeter/greeter.ja.req:18`
- 要件のハッシュ：`sha256:2590c87a9ffa99f1`

2026-10-04 に 窓口 が決めた：空の名前は呼び出す側の誤りで、挨拶ではない。この例のために決めたもの

満たすもの `file "server.py"` — 開発 が 2026-10-04 に確かめた（`sha256:2590c87a9ffa99f1` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim 空の名前は断る` — 開発 が 2026-10-04 に確かめた（`sha256:2590c87a9ffa99f1` → `sha256:e973e18f878e1f9e`）。状態：確かめたまま

### 足した数が積み上がる (totals_accumulate)

**POST /add は本文の数を合計に足し、GET /total が合計を返し、POST /reset が合計を 0 に戻す**

- 期間：—
- 持ち主：窓口
- ファイル：`examples/greeter/greeter.ja.req:27`
- 要件のハッシュ：`sha256:301f01ea03399fc9`

2026-10-04 に 窓口 が決めた：合計は、戻すまでリクエストをまたいで持つ。この例のために決めたもの

満たすもの `file "server.py"` — 開発 が 2026-10-04 に確かめた（`sha256:301f01ea03399fc9` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim 足した数が積み上がる` — 開発 が 2026-10-04 に確かめた（`sha256:301f01ea03399fc9` → `sha256:851003f8a61e8af4`）。状態：確かめたまま

### 知らないパスは404 (unknown_paths_are_404)

**ほかのパスは 404 を返す**

- 期間：—
- 持ち主：窓口
- ファイル：`examples/greeter/greeter.ja.req:36`
- 要件のハッシュ：`sha256:38b694c45841408b`

2026-10-04 に 窓口 が決めた：サービスは、知っているパスにだけ答える。この例のために決めたもの

満たすもの `file "server.py"` — 開発 が 2026-10-04 に確かめた（`sha256:38b694c45841408b` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` — 開発 が 2026-10-04 に確かめた（`sha256:38b694c45841408b` → `sha256:8788b539c21c495d`）。状態：確かめたまま

## 範囲

`scope file "server.py"` — 成果物は 1 個で、要件に辿れる。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-04 | 開発 | `名前で挨拶する (greets_by_name)` → `file "server.py"` | `sha256:e87a6fee4807cbad -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `名前で挨拶する (greets_by_name)` → `geas "greeter.ja.geas" claim 名前で挨拶する` | `sha256:e87a6fee4807cbad -> sha256:23771933ab7071aa` |
| 2026-10-04 | 開発 | `空の名前は断る (rejects_an_empty_name)` → `file "server.py"` | `sha256:2590c87a9ffa99f1 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `空の名前は断る (rejects_an_empty_name)` → `geas "greeter.ja.geas" claim 空の名前は断る` | `sha256:2590c87a9ffa99f1 -> sha256:e973e18f878e1f9e` |
| 2026-10-04 | 開発 | `足した数が積み上がる (totals_accumulate)` → `file "server.py"` | `sha256:301f01ea03399fc9 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `足した数が積み上がる (totals_accumulate)` → `geas "greeter.ja.geas" claim 足した数が積み上がる` | `sha256:301f01ea03399fc9 -> sha256:851003f8a61e8af4` |
| 2026-10-04 | 開発 | `知らないパスは404 (unknown_paths_are_404)` → `file "server.py"` | `sha256:38b694c45841408b -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `知らないパスは404 (unknown_paths_are_404)` → `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` | `sha256:38b694c45841408b -> sha256:8788b539c21c495d` |
