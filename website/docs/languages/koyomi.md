# koyomi

`.cal` — dates: closing and payment days, business days, legal periods.

**koyomi** writes a date as a contract or a law says it: thirty days after the invoice, moved to the next business day. `koyomi check` computes it on every day of the declared range and checks each claim on every one of them; only a file that passes compiles, into TypeScript, Python, Go, Rust and SQL, and into a Wasm module that any host calls with one JSON object.

koyomi's documents are in the repository: [README](https://github.com/i2y/ritsu/blob/main/crates/koyomi/README.md), [the reference](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/reference.md), [the generated code](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/generated-code.md), [the codes of its diagnostics](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/codes.md).

## Comparing two versions

`koyomi diff` compares the answers of two versions: of a calendar on every day both know, of a dates file on every input both take. Here an office's calendar closes on its founding day, renames a bank holiday, knows 2027 too and takes an offset:

```console
$ cd crates/koyomi/tests/diff
$ koyomi diff office.cal office_v2.cal
koyomi diff: calendar office v1 → v2
  old: office.cal
  new: office_v2.cal

compared: 2026-01-01..2026-12-31 (365 days, the days both versions know)

open or closed changed (1 interval, 1 day):
  2026-06-10: open → closed (Founding day)

closed in both, for other reasons (1 interval, 1 day):
  2026-05-04: Early May bank holiday → May Day

the days known: 2026-01-01..2026-12-31 → 2026-01-01..2027-12-31
offset: none → +01:00

every other day of 2026-01-01..2026-12-31 gets the same answer.
```

Each side is a path, a path at a git revision (`calendars/office.cal@origin/main`, the file as it is on that branch), a file of a dependency at the version the lock pins (`@calendars/office.cal`), or a file of another version of that dependency in the store (`@calendars@1.0.0/office.cal`). The exit code is 0 when nothing differs, 1 when something does, and 2 when the two cannot be compared: a calendar beside a dates file, or a side that does not pass `koyomi check`. `--format markdown` writes the same in Markdown, and `--format json` gives every interval.
