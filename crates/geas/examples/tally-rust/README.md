# tally-rust

A tally on the command line, written in Rust ([src/main.rs](src/main.rs)): `tally add 5 7` prints
`12`, and anything that is not a whole number is refused with exit 2. [tally.geas](tally.geas)
holds four claims on what it prints and how it exits.

`geas map` needs the program built with `-C instrument-coverage`, and the target's command has to
start that binary itself (`run "./tally"`), since `llvm-cov` turns a profile into lines only with
the program that wrote it. Build it in its directory (the binary is ignored by git):

```console
$ cd examples/tally-rust
$ rustc --edition 2024 -C instrument-coverage -o tally src/main.rs
$ cd ../..
$ geas check examples/tally-rust/tally.geas
$ geas map examples/tally-rust/tally.geas --root examples/tally-rust
```

`map` sets `LLVM_PROFILE_FILE` for each run, merges the profiles with `llvm-profdata` and reads them
with `llvm-cov`, both from rustup's `llvm-tools` component (`rustup component add llvm-tools`), and
prints `map: 1 file · 28 lines of code, 21 run by some claim`. Run outside `map`, the instrumented
binary writes `default_*.profraw` files into its directory.

[tests/changes/tally-rust](../../tests/changes/tally-rust) holds an agent's change, and
[tests/golden/en/languages/tally-rust](../../tests/golden/en/languages/tally-rust) what the tests
expect of `map` and `geas affected` on it: the `min` command it adds, line 25, is code no claim runs.
