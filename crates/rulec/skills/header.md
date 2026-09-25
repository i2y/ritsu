---
name: rulec
description: Turn a table-shaped business rule into proved, dependency-free Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm with rulec. Use when a shipping tariff, fee schedule, discount or coupon policy, eligibility test, period classification, or any rule that is already written as a table has to become code; when writing, editing or reviewing a `.rule` file; when a rulec diagnostic (E001-E057, E101-E128, W105, W110, W111, W114-W127) has to be fixed; or when a change to such a rule has to be shown to a person before it ships.
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

A process with state applies one call at a time: the table decides a step, and the caller
keeps the state. It does **not** apply to running a workflow, to judgements about a collection
("any line is refrigerated", "three or more items"), to pattern matching on strings, or to
scoring and optimisation. Flatten collection facts at the boundary and pass the scalar in;
keep iteration in the caller.

`rulec gen` writes Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm.
A language only goes in once its output can be held against the reference evaluator byte for
byte, so whatever `rulec gen` writes is covered by `rulec test`.

The files bundled with this skill are listed under §7 at the end: read them on demand, not
all up front. Every diagnostic is `rulec explain <CODE>`, which is always current, so none of
them are bundled here.

---

