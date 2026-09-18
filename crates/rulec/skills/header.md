---
name: rulec
description: Turn a table-shaped business rule into proved, dependency-free Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL with rulec. Use when a shipping tariff, fee schedule, discount or coupon policy, eligibility test, period classification, or any rule that is already written as a table has to become code; when writing, editing or reviewing a `.rule` file; when a rulec diagnostic (E001-E033, E101-E115, W105, W110, W111, W114, W115, W116) has to be fixed; or when a change to such a rule has to be shown to a person before it ships.
compatibility: Requires the `rulec` binary on PATH (https://github.com/i2y/rulec).
license: MIT
---

## When this applies

The rule **is a table, or could be written as one** — a table plus, where it needs it, a
little arithmetic over the inputs. Some are already tables: a tariff, a rate card, a fee
schedule. Others are prose or folklore and become a table once written down: when a discount
applies, whether a return is accepted, which period a date falls in, what rank a set of scores
earns. The source of truth may be a spreadsheet, a published policy, a wiki page, a legacy
implementation, or a person, and the job is to turn it into code someone can approve.

It does **not** apply to workflows with several steps and state, to judgements about a
collection ("any line is refrigerated", "three or more items"), to pattern matching on
strings, or to scoring and optimisation. Flatten collection facts at the boundary and pass
the scalar in; keep iteration in the caller.

`rulec gen` writes Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL today; Java and Kotlin are planned.
A language only goes in once its output can be held against the reference evaluator byte for
byte, so whatever `rulec gen` writes is covered by `rulec test`.

Bundled with this skill, read on demand — do not read them all up front:

| | |
|---|---|
| [reference.md](reference.md) | the complete grammar |
| [examples.md](examples.md) | every worked rule in full, smallest first, each with what it demonstrates |
| [formats.md](formats.md) | every machine-readable format: `--format json`, vectors, fixtures, the manifest, the adapter protocol |
| [generated-code.md](generated-code.md) | the shape and guarantees of the generated code in each language, and how to call it |

Every diagnostic is `rulec explain <CODE>`, which is always current, so none of them are
bundled here.

---

