# koyomi

`.cal` — dates: closing and payment days, business days, legal periods.

**koyomi** writes a date as a contract or a law says it: thirty days after the invoice, moved to the next business day. `koyomi check` computes it on every day of the declared range and checks each claim on every one of them; only a file that passes compiles, into TypeScript, Python, Go, Rust and SQL, and into a Wasm module that any host calls with one JSON object.

koyomi's documents are in the repository: [README](https://github.com/i2y/ritsu/blob/main/crates/koyomi/README.md), [the reference](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/reference.md), [the generated code](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/generated-code.md), [the codes of its diagnostics](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/codes.md).
