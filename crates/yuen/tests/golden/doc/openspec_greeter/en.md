# openspec_greeter — where the requirements come from

yuen 0.26.0 made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts.

- `greeter.req` (openspec_greeter v1, `sha256:54e1d167deb692f2`)

The check: `examples/openspec_greeter/greeter.req: ok — 3 requirements, whose 10 links are as they were looked at; every requirement is met and checked, or waived`

## Traceability

| Requirement | In force | Comes from | Owner | Met by | Checked by | State |
|---|---|---|---|---|---|---|
| `greeting_by_name` | — | `@greeting "Greeting by name"` | api | `file "server.py"` | `geas "greeter.geas" claim "greets by name"`; `geas "greeter.geas" claim "rejects an empty name"` | as looked at |
| `running_total` | — | `@greeting "Running total"` | api | `file "server.py"` | `geas "greeter.geas" claim "totals accumulate across requests"` | as looked at |
| `unknown_paths` | — | `@greeting "Unknown paths"` | api | `file "server.py"` | `geas "greeter.geas" claim "unknown paths are 404"` | as looked at |

## Sources

### greeting

The OpenSpec spec `openspec/specs/greeting/spec.md`, each requirement pinned by its block (from its header to its scenarios).

| Requirement | Pin | Cited by |
|---|---|---|
| Greeting by name | `sha256:1c3d865f4a521275` | `greeting_by_name` |
| Running total | `sha256:f94d564d4cf48f1f` | `running_total` |
| Unknown paths | `sha256:58e467e6888e0230` | `unknown_paths` |

## Why each requirement is so

### greeting_by_name

**A greeting answers 200 with Hello and the name asked for, and an empty name answers 400**

- In force: —
- Owner: api
- File: `examples/openspec_greeter/greeter.req:12`
- The requirement's end: `sha256:093faf9edde29919`

Comes from `@greeting "Greeting by name"` — looked at by api on 2026-10-05 (`sha256:1c3d865f4a521275` → `sha256:093faf9edde29919`). State: as looked at

> **greeting "Greeting by name" (the OpenSpec spec examples/openspec_greeter/openspec/specs/greeting/spec.md)**
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

Met by `file "server.py"` — looked at by development on 2026-10-05 (`sha256:093faf9edde29919` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.geas" claim "greets by name"` — looked at by development on 2026-10-05 (`sha256:093faf9edde29919` → `sha256:2b4e11c2cc191431`). State: as looked at

Checked by `geas "greeter.geas" claim "rejects an empty name"` — looked at by development on 2026-10-05 (`sha256:093faf9edde29919` → `sha256:3f7c8d1b2bbcc309`). State: as looked at

### running_total

**A total is kept across requests: POST /reset sets it to 0, POST /add adds to it, GET /total answers it**

- In force: —
- Owner: api
- File: `examples/openspec_greeter/greeter.req:24`
- The requirement's end: `sha256:a2019ef11a58ef98`

Comes from `@greeting "Running total"` — looked at by api on 2026-10-05 (`sha256:f94d564d4cf48f1f` → `sha256:a2019ef11a58ef98`). State: as looked at

> **greeting "Running total" (the OpenSpec spec examples/openspec_greeter/openspec/specs/greeting/spec.md)**
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

Met by `file "server.py"` — looked at by development on 2026-10-05 (`sha256:a2019ef11a58ef98` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.geas" claim "totals accumulate across requests"` — looked at by development on 2026-10-05 (`sha256:a2019ef11a58ef98` → `sha256:d653be1c06f5c8b1`). State: as looked at

### unknown_paths

**Any path the service does not know answers 404**

- In force: —
- Owner: api
- File: `examples/openspec_greeter/greeter.req:34`
- The requirement's end: `sha256:fa91c8ff4af0068e`

Comes from `@greeting "Unknown paths"` — looked at by api on 2026-10-05 (`sha256:58e467e6888e0230` → `sha256:fa91c8ff4af0068e`). State: as looked at

> **greeting "Unknown paths" (the OpenSpec spec examples/openspec_greeter/openspec/specs/greeting/spec.md)**
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

Met by `file "server.py"` — looked at by development on 2026-10-05 (`sha256:fa91c8ff4af0068e` → `sha256:331e02e26d128e8c`). State: as looked at

Checked by `geas "greeter.geas" claim "unknown paths are 404"` — looked at by development on 2026-10-05 (`sha256:fa91c8ff4af0068e` → `sha256:1950201e8644bc57`). State: as looked at

## Scope

No scope is declared.

## Records

| Date | By | What | Hashes |
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
