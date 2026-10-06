# 挨拶 — where the requirements come from

yuen 0.24.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `greeter.ja.req` (挨拶 v1, `sha256:23706b2d5df397b0`)

The check: `examples/openspec_greeter/greeter.ja.req: ok — 3 requirements, whose 10 links are as they were looked at; every requirement is met and checked, or waived`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `名前で挨拶する (greeting_by_name)` | — | `@挨拶 名前で挨拶する` | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 名前で挨拶する`; `geas "greeter.ja.geas" claim 空の名前は受け付けない` | as looked at |
| `足した数の合計 (running_total)` | — | `@挨拶 足した数の合計` | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 足した数が積み上がる` | as looked at |
| `知らないパス (unknown_paths)` | — | `@挨拶 知らないパス` | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` | as looked at |

## Sources

### 挨拶

The OpenSpec spec `ja/openspec/specs/greeting/spec.md`, each requirement pinned by its block (from its header to its scenarios).

| Requirement | Pin | Cited by |
|---|---|---|
| 名前で挨拶する | `sha256:584f410eda34b49e` | `名前で挨拶する (greeting_by_name)` |
| 足した数の合計 | `sha256:16b2a7d63d90dcf5` | `足した数の合計 (running_total)` |
| 知らないパス | `sha256:d0b84e0286e74b3a` | `知らないパス (unknown_paths)` |

## Why each requirement is so

### 名前で挨拶する (greeting_by_name)

**挨拶は 200 と、Hello と頼まれた名前を返す。空の名前には 400 を返す**

- In force: —
- Owner: 窓口
- File: `examples/openspec_greeter/greeter.ja.req:12`
- The requirement's end: `sha256:0fe7e5be49136195`

Comes from `@挨拶 名前で挨拶する` — looked at by 窓口 on 2026-10-05 (`sha256:584f410eda34b49e` → `sha256:0fe7e5be49136195`). State: as looked at

> **挨拶 名前で挨拶する (the OpenSpec spec examples/openspec_greeter/ja/openspec/specs/greeting/spec.md)**
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

Met by `file "server.py"` — looked at by 開発 on 2026-10-05 (`sha256:0fe7e5be49136195` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.ja.geas" claim 名前で挨拶する` — looked at by 開発 on 2026-10-05 (`sha256:0fe7e5be49136195` → `sha256:23771933ab7071aa`). State: as looked at

Checked by `geas "greeter.ja.geas" claim 空の名前は受け付けない` — looked at by 開発 on 2026-10-05 (`sha256:0fe7e5be49136195` → `sha256:cfde1ffad5721517`). State: as looked at

### 足した数の合計 (running_total)

**合計はリクエストをまたいで覚えておく。POST /reset で 0 にし、POST /add で足し、GET /total で返す**

- In force: —
- Owner: 窓口
- File: `examples/openspec_greeter/greeter.ja.req:24`
- The requirement's end: `sha256:2c9bc244d3620e0c`

Comes from `@挨拶 足した数の合計` — looked at by 窓口 on 2026-10-05 (`sha256:16b2a7d63d90dcf5` → `sha256:2c9bc244d3620e0c`). State: as looked at

> **挨拶 足した数の合計 (the OpenSpec spec examples/openspec_greeter/ja/openspec/specs/greeting/spec.md)**
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

Met by `file "server.py"` — looked at by 開発 on 2026-10-05 (`sha256:2c9bc244d3620e0c` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.ja.geas" claim 足した数が積み上がる` — looked at by 開発 on 2026-10-05 (`sha256:2c9bc244d3620e0c` → `sha256:851003f8a61e8af4`). State: as looked at

### 知らないパス (unknown_paths)

**知らないパスには 404 を返す**

- In force: —
- Owner: 窓口
- File: `examples/openspec_greeter/greeter.ja.req:34`
- The requirement's end: `sha256:eb33424df22e79c6`

Comes from `@挨拶 知らないパス` — looked at by 窓口 on 2026-10-05 (`sha256:d0b84e0286e74b3a` → `sha256:eb33424df22e79c6`). State: as looked at

> **挨拶 知らないパス (the OpenSpec spec examples/openspec_greeter/ja/openspec/specs/greeting/spec.md)**
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

Met by `file "server.py"` — looked at by 開発 on 2026-10-05 (`sha256:eb33424df22e79c6` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` — looked at by 開発 on 2026-10-05 (`sha256:eb33424df22e79c6` → `sha256:8788b539c21c495d`). State: as looked at

## Scope

No scope is declared.

## Records

| Date | By | What | Hashes |
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
