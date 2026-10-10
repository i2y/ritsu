# 挨拶 — where the requirements come from

yuen 0.26.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `greeter.ja.req` (挨拶 v1, `sha256:56bec55280e19241`)

The check: `examples/greeter/greeter.ja.req: ok — 4 requirements, whose 8 links are as they were looked at; every requirement is met and checked, or waived; the file in scope traces to a requirement`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `名前で挨拶する (greets_by_name)` | — | decided by 窓口 on 2026-10-04 | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 名前で挨拶する` | as looked at |
| `空の名前は受け付けない (rejects_an_empty_name)` | — | decided by 窓口 on 2026-10-04 | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 空の名前は受け付けない` | as looked at |
| `足した数が積み上がる (totals_accumulate)` | — | decided by 窓口 on 2026-10-04 | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim 足した数が積み上がる` | as looked at |
| `知らないパスは404 (unknown_paths_are_404)` | — | decided by 窓口 on 2026-10-04 | 窓口 | `file "server.py"` | `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` | as looked at |

## Sources

No source is declared: every requirement comes from a decision or from another requirement.

## Why each requirement is so

### 名前で挨拶する (greets_by_name)

**GET /greet?name=<名前> は 200 と、メッセージ Hello, <名前> を返す**

- In force: —
- Owner: 窓口
- File: `examples/greeter/greeter.ja.req:9`
- The requirement's end: `sha256:e87a6fee4807cbad`

Decided by 窓口 on 2026-10-04: 挨拶には、頼んだ人の名前を入れる。この例のために決めたもの

Met by `file "server.py"` — looked at by 開発 on 2026-10-04 (`sha256:e87a6fee4807cbad` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.ja.geas" claim 名前で挨拶する` — looked at by 開発 on 2026-10-04 (`sha256:e87a6fee4807cbad` → `sha256:23771933ab7071aa`). State: as looked at

### 空の名前は受け付けない (rejects_an_empty_name)

**名前が空の GET /greet は 400 を返す**

- In force: —
- Owner: 窓口
- File: `examples/greeter/greeter.ja.req:18`
- The requirement's end: `sha256:2590c87a9ffa99f1`

Decided by 窓口 on 2026-10-04: 空の名前は呼び出す側の誤りで、挨拶ではない。この例のために決めたもの

Met by `file "server.py"` — looked at by 開発 on 2026-10-04 (`sha256:2590c87a9ffa99f1` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.ja.geas" claim 空の名前は受け付けない` — looked at by 開発 on 2026-10-04 (`sha256:2590c87a9ffa99f1` → `sha256:cfde1ffad5721517`). State: as looked at

### 足した数が積み上がる (totals_accumulate)

**POST /add は本文の数を合計に足し、GET /total が合計を返し、POST /reset が合計を 0 に戻す**

- In force: —
- Owner: 窓口
- File: `examples/greeter/greeter.ja.req:27`
- The requirement's end: `sha256:301f01ea03399fc9`

Decided by 窓口 on 2026-10-04: 合計は、戻すまでリクエストをまたいで持つ。この例のために決めたもの

Met by `file "server.py"` — looked at by 開発 on 2026-10-04 (`sha256:301f01ea03399fc9` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.ja.geas" claim 足した数が積み上がる` — looked at by 開発 on 2026-10-04 (`sha256:301f01ea03399fc9` → `sha256:851003f8a61e8af4`). State: as looked at

### 知らないパスは404 (unknown_paths_are_404)

**ほかのパスは 404 を返す**

- In force: —
- Owner: 窓口
- File: `examples/greeter/greeter.ja.req:36`
- The requirement's end: `sha256:38b694c45841408b`

Decided by 窓口 on 2026-10-04: サービスは、知っているパスにだけ答える。この例のために決めたもの

Met by `file "server.py"` — looked at by 開発 on 2026-10-04 (`sha256:38b694c45841408b` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` — looked at by 開発 on 2026-10-04 (`sha256:38b694c45841408b` → `sha256:8788b539c21c495d`). State: as looked at

## Scope

`scope file "server.py"` — 1 artifact in it, which traces to a requirement.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-04 | 開発 | `名前で挨拶する (greets_by_name)` → `file "server.py"` | `sha256:e87a6fee4807cbad -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `名前で挨拶する (greets_by_name)` → `geas "greeter.ja.geas" claim 名前で挨拶する` | `sha256:e87a6fee4807cbad -> sha256:23771933ab7071aa` |
| 2026-10-04 | 開発 | `空の名前は受け付けない (rejects_an_empty_name)` → `file "server.py"` | `sha256:2590c87a9ffa99f1 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `空の名前は受け付けない (rejects_an_empty_name)` → `geas "greeter.ja.geas" claim 空の名前は受け付けない` | `sha256:2590c87a9ffa99f1 -> sha256:cfde1ffad5721517` |
| 2026-10-04 | 開発 | `足した数が積み上がる (totals_accumulate)` → `file "server.py"` | `sha256:301f01ea03399fc9 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `足した数が積み上がる (totals_accumulate)` → `geas "greeter.ja.geas" claim 足した数が積み上がる` | `sha256:301f01ea03399fc9 -> sha256:851003f8a61e8af4` |
| 2026-10-04 | 開発 | `知らないパスは404 (unknown_paths_are_404)` → `file "server.py"` | `sha256:38b694c45841408b -> sha256:331e02e26d128e8c` |
| 2026-10-04 | 開発 | `知らないパスは404 (unknown_paths_are_404)` → `geas "greeter.ja.geas" claim "知らないパスには 404 を返す"` | `sha256:38b694c45841408b -> sha256:8788b539c21c495d` |
