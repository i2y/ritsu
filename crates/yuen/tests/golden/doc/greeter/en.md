# greeter — where the requirements come from

yuen 0.23.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `greeter.req` (greeter v1, `sha256:83c7eff218129a2e`)

The check: `examples/greeter/greeter.req: ok — 4 requirements, whose 8 links are as they were looked at; every requirement is met and checked, or waived; the file in scope traces to a requirement`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `greets_by_name` | — | decided by api on 2026-10-04 | api | `file "server.py"` | `geas "greeter.geas" claim "greets by name"` | as looked at |
| `rejects_an_empty_name` | — | decided by api on 2026-10-04 | api | `file "server.py"` | `geas "greeter.geas" claim "rejects an empty name"` | as looked at |
| `totals_accumulate` | — | decided by api on 2026-10-04 | api | `file "server.py"` | `geas "greeter.geas" claim "totals accumulate across requests"` | as looked at |
| `unknown_paths_are_404` | — | decided by api on 2026-10-04 | api | `file "server.py"` | `geas "greeter.geas" claim "unknown paths are 404"` | as looked at |

## Sources

No source is declared: every requirement comes from a decision or from another requirement.

## Why each requirement is so

### greets_by_name

**GET /greet?name=<name> answers 200 with the message Hello, <name>**

- In force: —
- Owner: api
- File: `examples/greeter/greeter.req:9`
- The requirement's end: `sha256:59d408ab78823210`

Decided by api on 2026-10-04: A greeting names whoever asked for it. Decided for this example

Met by `file "server.py"` — looked at by development on 2026-10-04 (`sha256:59d408ab78823210` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.geas" claim "greets by name"` — looked at by development on 2026-10-04 (`sha256:59d408ab78823210` → `sha256:2b4e11c2cc191431`). State: as looked at

### rejects_an_empty_name

**GET /greet with an empty name answers 400**

- In force: —
- Owner: api
- File: `examples/greeter/greeter.req:18`
- The requirement's end: `sha256:611723760e60eba7`

Decided by api on 2026-10-04: An empty name is a mistake of the client, not a greeting. Decided for this example

Met by `file "server.py"` — looked at by development on 2026-10-04 (`sha256:611723760e60eba7` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.geas" claim "rejects an empty name"` — looked at by development on 2026-10-04 (`sha256:611723760e60eba7` → `sha256:3f7c8d1b2bbcc309`). State: as looked at

### totals_accumulate

**POST /add adds its body to a total that GET /total answers, and POST /reset sets it to 0**

- In force: —
- Owner: api
- File: `examples/greeter/greeter.req:27`
- The requirement's end: `sha256:fd0003d1f89cfc8c`

Decided by api on 2026-10-04: The total is kept across requests until it is reset. Decided for this example

Met by `file "server.py"` — looked at by development on 2026-10-04 (`sha256:fd0003d1f89cfc8c` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.geas" claim "totals accumulate across requests"` — looked at by development on 2026-10-04 (`sha256:fd0003d1f89cfc8c` → `sha256:d653be1c06f5c8b1`). State: as looked at

### unknown_paths_are_404

**Any other path answers 404**

- In force: —
- Owner: api
- File: `examples/greeter/greeter.req:36`
- The requirement's end: `sha256:9e53cc1bcdc81167`

Decided by api on 2026-10-04: The service answers only the paths it knows. Decided for this example

Met by `file "server.py"` — looked at by development on 2026-10-04 (`sha256:9e53cc1bcdc81167` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.geas" claim "unknown paths are 404"` — looked at by development on 2026-10-04 (`sha256:9e53cc1bcdc81167` → `sha256:1950201e8644bc57`). State: as looked at

## Scope

`scope file "server.py"` — 1 artifact in it, which traces to a requirement.

## Records

| Date | By | What | Hashes |
|---|---|---|---|
| 2026-10-04 | development | `greets_by_name` → `file "server.py"` | `sha256:59d408ab78823210 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `greets_by_name` → `geas "greeter.geas" claim "greets by name"` | `sha256:59d408ab78823210 -> sha256:2b4e11c2cc191431` |
| 2026-10-04 | development | `rejects_an_empty_name` → `file "server.py"` | `sha256:611723760e60eba7 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `rejects_an_empty_name` → `geas "greeter.geas" claim "rejects an empty name"` | `sha256:611723760e60eba7 -> sha256:3f7c8d1b2bbcc309` |
| 2026-10-04 | development | `totals_accumulate` → `file "server.py"` | `sha256:fd0003d1f89cfc8c -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `totals_accumulate` → `geas "greeter.geas" claim "totals accumulate across requests"` | `sha256:fd0003d1f89cfc8c -> sha256:d653be1c06f5c8b1` |
| 2026-10-04 | development | `unknown_paths_are_404` → `file "server.py"` | `sha256:9e53cc1bcdc81167 -> sha256:331e02e26d128e8c` |
| 2026-10-04 | development | `unknown_paths_are_404` → `geas "greeter.geas" claim "unknown paths are 404"` | `sha256:9e53cc1bcdc81167 -> sha256:1950201e8644bc57` |
