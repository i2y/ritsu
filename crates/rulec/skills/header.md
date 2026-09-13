---
name: rulec
description: Turn a table-shaped business rule into proved, dependency-free Python and Go with rulec. Use when a shipping tariff, fee schedule, discount or coupon policy, eligibility test, period classification, or any rule that is already written as a table has to become code; when writing, editing or reviewing a `.rule` file; when a rulec diagnostic (E001-E014, E101-E114, W105, W110, W111, W114) has to be fixed; or when a change to such a rule has to be shown to a person before it ships.
compatibility: Requires the `rulec` binary on PATH (https://github.com/i2y/rulec).
license: MIT
---

## When this applies

The source of truth is **already a table, or would be one if someone wrote it down**: a
tariff, a rate card, a fee schedule, the conditions under which a discount applies, which
period a date falls in, whether a return is accepted. It lives in a spreadsheet, a published
policy, a wiki page, or a legacy implementation, and the job is to turn it into code someone
can approve.

It does **not** apply to workflows with several steps and state, to judgements about a
collection ("any line is refrigerated", "three or more items"), to pattern matching on
strings, or to scoring and optimisation. Flatten collection facts at the boundary and pass
the scalar in; keep iteration in the caller.

Bundled with this skill, read on demand — do not read them all up front:

| | |
|---|---|
| [reference.md](reference.md) | the complete grammar |
| [examples.md](examples.md) | ten complete rules, smallest first, each with what it demonstrates |
| [formats.md](formats.md) | every machine-readable format: `--format json`, vectors, fixtures, the manifest, the adapter protocol |
| [generated-code.md](generated-code.md) | the shape and guarantees of the generated Python and Go, and how to call it |

Every diagnostic is `rulec explain <CODE>`, which is always current, so there is no bundled
copy of the ledger.

---

