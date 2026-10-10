# greeter — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.26.0 が作った。

- `greeter.req`（greeter v1、`sha256:83c7eff218129a2e`）

検査の結果：`examples/greeter/greeter.req: ok — 要件 4 件のリンク 8 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があり、範囲のファイル 1 個は、要件に辿れます。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `greets_by_name` | — | 2026-10-04 に api が決めた | api | `file "server.py"` | `geas "greeter.geas" claim "greets by name"` | 確かめたまま |
| `rejects_an_empty_name` | — | 2026-10-04 に api が決めた | api | `file "server.py"` | `geas "greeter.geas" claim "rejects an empty name"` | 確かめたまま |
| `totals_accumulate` | — | 2026-10-04 に api が決めた | api | `file "server.py"` | `geas "greeter.geas" claim "totals accumulate across requests"` | 確かめたまま |
| `unknown_paths_are_404` | — | 2026-10-04 に api が決めた | api | `file "server.py"` | `geas "greeter.geas" claim "unknown paths are 404"` | 確かめたまま |

## 出典

出典の宣言は無い。どの要件も、決めたことか、ほかの要件から来ている。

## 要件ごとの「なぜ」

### greets_by_name

**GET /greet?name=<name> answers 200 with the message Hello, <name>**

- 期間：—
- 持ち主：api
- ファイル：`examples/greeter/greeter.req:9`
- 要件のハッシュ：`sha256:59d408ab78823210`

2026-10-04 に api が決めた：A greeting names whoever asked for it. Decided for this example

満たすもの `file "server.py"` — development が 2026-10-04 に確かめた（`sha256:59d408ab78823210` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "greets by name"` — development が 2026-10-04 に確かめた（`sha256:59d408ab78823210` → `sha256:2b4e11c2cc191431`）。状態：確かめたまま

### rejects_an_empty_name

**GET /greet with an empty name answers 400**

- 期間：—
- 持ち主：api
- ファイル：`examples/greeter/greeter.req:18`
- 要件のハッシュ：`sha256:611723760e60eba7`

2026-10-04 に api が決めた：An empty name is a mistake of the client, not a greeting. Decided for this example

満たすもの `file "server.py"` — development が 2026-10-04 に確かめた（`sha256:611723760e60eba7` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "rejects an empty name"` — development が 2026-10-04 に確かめた（`sha256:611723760e60eba7` → `sha256:3f7c8d1b2bbcc309`）。状態：確かめたまま

### totals_accumulate

**POST /add adds its body to a total that GET /total answers, and POST /reset sets it to 0**

- 期間：—
- 持ち主：api
- ファイル：`examples/greeter/greeter.req:27`
- 要件のハッシュ：`sha256:fd0003d1f89cfc8c`

2026-10-04 に api が決めた：The total is kept across requests until it is reset. Decided for this example

満たすもの `file "server.py"` — development が 2026-10-04 に確かめた（`sha256:fd0003d1f89cfc8c` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "totals accumulate across requests"` — development が 2026-10-04 に確かめた（`sha256:fd0003d1f89cfc8c` → `sha256:d653be1c06f5c8b1`）。状態：確かめたまま

### unknown_paths_are_404

**Any other path answers 404**

- 期間：—
- 持ち主：api
- ファイル：`examples/greeter/greeter.req:36`
- 要件のハッシュ：`sha256:9e53cc1bcdc81167`

2026-10-04 に api が決めた：The service answers only the paths it knows. Decided for this example

満たすもの `file "server.py"` — development が 2026-10-04 に確かめた（`sha256:9e53cc1bcdc81167` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "unknown paths are 404"` — development が 2026-10-04 に確かめた（`sha256:9e53cc1bcdc81167` → `sha256:1950201e8644bc57`）。状態：確かめたまま

## 範囲

`scope file "server.py"` — 成果物は 1 個で、要件に辿れる。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-04 | development | `greets_by_name` → `file "server.py"` | `sha256:59d408ab78823210 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `greets_by_name` → `geas "greeter.geas" claim "greets by name"` | `sha256:59d408ab78823210 -> sha256:2b4e11c2cc191431` |
| 2026-10-04 | development | `rejects_an_empty_name` → `file "server.py"` | `sha256:611723760e60eba7 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `rejects_an_empty_name` → `geas "greeter.geas" claim "rejects an empty name"` | `sha256:611723760e60eba7 -> sha256:3f7c8d1b2bbcc309` |
| 2026-10-04 | development | `totals_accumulate` → `file "server.py"` | `sha256:fd0003d1f89cfc8c -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `totals_accumulate` → `geas "greeter.geas" claim "totals accumulate across requests"` | `sha256:fd0003d1f89cfc8c -> sha256:d653be1c06f5c8b1` |
| 2026-10-04 | development | `unknown_paths_are_404` → `file "server.py"` | `sha256:9e53cc1bcdc81167 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `unknown_paths_are_404` → `geas "greeter.geas" claim "unknown paths are 404"` | `sha256:9e53cc1bcdc81167 -> sha256:1950201e8644bc57` |
