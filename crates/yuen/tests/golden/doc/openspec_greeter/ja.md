# openspec_greeter — 要件の出どころ

このページは、下の .req のファイルと、出典のコピーと、ほかの言語から読んだ成果物の定義をもとに、yuen 0.25.0 が作った。

- `greeter.req`（openspec_greeter v1、`sha256:54e1d167deb692f2`）

検査の結果：`examples/openspec_greeter/greeter.req: ok — 要件 3 件のリンク 10 本が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があります。`

## トレーサビリティ

| 要件 | 期間 | 出どころ | 持ち主 | 満たすもの | 確かめるもの | 状態 |
|---|---|---|---|---|---|---|
| `greeting_by_name` | — | `@greeting "Greeting by name"` | api | `file "server.py"` | `geas "greeter.geas" claim "greets by name"`、`geas "greeter.geas" claim "rejects an empty name"` | 確かめたまま |
| `running_total` | — | `@greeting "Running total"` | api | `file "server.py"` | `geas "greeter.geas" claim "totals accumulate across requests"` | 確かめたまま |
| `unknown_paths` | — | `@greeting "Unknown paths"` | api | `file "server.py"` | `geas "greeter.geas" claim "unknown paths are 404"` | 確かめたまま |

## 出典

### greeting

OpenSpec の仕様 `openspec/specs/greeting/spec.md`。要件ごとに、そのブロック（見出しからシナリオまで）を固定する。

| 要件 | 固定 | 引く要件 |
|---|---|---|
| Greeting by name | `sha256:1c3d865f4a521275` | `greeting_by_name` |
| Running total | `sha256:f94d564d4cf48f1f` | `running_total` |
| Unknown paths | `sha256:58e467e6888e0230` | `unknown_paths` |

## 要件ごとの「なぜ」

### greeting_by_name

**A greeting answers 200 with Hello and the name asked for, and an empty name answers 400**

- 期間：—
- 持ち主：api
- ファイル：`examples/openspec_greeter/greeter.req:12`
- 要件のハッシュ：`sha256:093faf9edde29919`

出どころ `@greeting "Greeting by name"` — api が 2026-10-05 に確かめた（`sha256:1c3d865f4a521275` → `sha256:093faf9edde29919`）。状態：確かめたまま

> **greeting "Greeting by name"（OpenSpec の仕様 examples/openspec_greeter/openspec/specs/greeting/spec.md）**
>
> ### Requirement: Greeting by name
>
> The service SHALL answer `GET /greet?name=<name>` with status 200 and a JSON body whose `message` is `Hello, <name>`, and SHALL refuse an empty name with status 400.
>
> #### Scenario: greets by name
>
> - **WHEN** a client asks for `/greet?name=Alice`
>
> - **THEN** the status is 200
>
> - **AND** `message` is `Hello, Alice`
>
> #### Scenario: rejects an empty name
>
> - **WHEN** a client asks for `/greet?name=`
>
> - **THEN** the status is 400

満たすもの `file "server.py"` — development が 2026-10-05 に確かめた（`sha256:093faf9edde29919` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "greets by name"` — development が 2026-10-05 に確かめた（`sha256:093faf9edde29919` → `sha256:2b4e11c2cc191431`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "rejects an empty name"` — development が 2026-10-05 に確かめた（`sha256:093faf9edde29919` → `sha256:3f7c8d1b2bbcc309`）。状態：確かめたまま

### running_total

**A total is kept across requests: POST /reset sets it to 0, POST /add adds to it, GET /total answers it**

- 期間：—
- 持ち主：api
- ファイル：`examples/openspec_greeter/greeter.req:24`
- 要件のハッシュ：`sha256:a2019ef11a58ef98`

出どころ `@greeting "Running total"` — api が 2026-10-05 に確かめた（`sha256:f94d564d4cf48f1f` → `sha256:a2019ef11a58ef98`）。状態：確かめたまま

> **greeting "Running total"（OpenSpec の仕様 examples/openspec_greeter/openspec/specs/greeting/spec.md）**
>
> ### Requirement: Running total
>
> The service SHALL keep a total across requests: `POST /reset` sets it to 0, `POST /add` adds the number in the body, and `GET /total` answers it.
>
> #### Scenario: totals accumulate across requests
>
> - **GIVEN** the total was reset
>
> - **WHEN** a client adds 5 and then 7
>
> - **THEN** `GET /total` answers 12

満たすもの `file "server.py"` — development が 2026-10-05 に確かめた（`sha256:a2019ef11a58ef98` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "totals accumulate across requests"` — development が 2026-10-05 に確かめた（`sha256:a2019ef11a58ef98` → `sha256:d653be1c06f5c8b1`）。状態：確かめたまま

### unknown_paths

**Any path the service does not know answers 404**

- 期間：—
- 持ち主：api
- ファイル：`examples/openspec_greeter/greeter.req:34`
- 要件のハッシュ：`sha256:fa91c8ff4af0068e`

出どころ `@greeting "Unknown paths"` — api が 2026-10-05 に確かめた（`sha256:58e467e6888e0230` → `sha256:fa91c8ff4af0068e`）。状態：確かめたまま

> **greeting "Unknown paths"（OpenSpec の仕様 examples/openspec_greeter/openspec/specs/greeting/spec.md）**
>
> ### Requirement: Unknown paths
>
> The service MUST answer any path it does not know with status 404.
>
> #### Scenario: unknown paths are 404
>
> - **WHEN** a client asks for `/nope`
>
> - **THEN** the status is 404

満たすもの `file "server.py"` — development が 2026-10-05 に確かめた（`sha256:fa91c8ff4af0068e` → `sha256:331e02e26d128e8c`）。状態：確かめたまま

確かめるもの `geas "greeter.geas" claim "unknown paths are 404"` — development が 2026-10-05 に確かめた（`sha256:fa91c8ff4af0068e` → `sha256:1950201e8644bc57`）。状態：確かめたまま

## 範囲

範囲の宣言は無い。

## 確かめた記録

| 日付 | 誰が | 何を | ハッシュ |
|---|---|---|---|
| 2026-10-05 | api | `@greeting "Greeting by name"` → `greeting_by_name` | `sha256:1c3d865f4a521275 -> sha256:093faf9edde29919` |
| 2026-10-05 | development | `greeting_by_name` → `file "server.py"` | `sha256:093faf9edde29919 -> sha256:331e02e26d128e8c` |
| 2026-10-05 | development | `greeting_by_name` → `geas "greeter.geas" claim "greets by name"` | `sha256:093faf9edde29919 -> sha256:2b4e11c2cc191431` |
| 2026-10-05 | development | `greeting_by_name` → `geas "greeter.geas" claim "rejects an empty name"` | `sha256:093faf9edde29919 -> sha256:3f7c8d1b2bbcc309` |
| 2026-10-05 | api | `@greeting "Running total"` → `running_total` | `sha256:f94d564d4cf48f1f -> sha256:a2019ef11a58ef98` |
| 2026-10-05 | development | `running_total` → `file "server.py"` | `sha256:a2019ef11a58ef98 -> sha256:331e02e26d128e8c` |
| 2026-10-05 | development | `running_total` → `geas "greeter.geas" claim "totals accumulate across requests"` | `sha256:a2019ef11a58ef98 -> sha256:d653be1c06f5c8b1` |
| 2026-10-05 | api | `@greeting "Unknown paths"` → `unknown_paths` | `sha256:58e467e6888e0230 -> sha256:fa91c8ff4af0068e` |
| 2026-10-05 | development | `unknown_paths` → `file "server.py"` | `sha256:fa91c8ff4af0068e -> sha256:331e02e26d128e8c` |
| 2026-10-05 | development | `unknown_paths` → `geas "greeter.geas" claim "unknown paths are 404"` | `sha256:fa91c8ff4af0068e -> sha256:1950201e8644bc57` |
