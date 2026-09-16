#!/usr/bin/env bash
# The checker as one file the site can load: `docs/playground/rulec.wasm` (§15.48).
#
# It is committed, like the diagrams and the screenshots, so that building the site needs
# no Rust toolchain — and like them it is a build product, so re-run this after anything
# that changes what `check`, `gen` or `doc` answer. `tests/wasm.rs` drives the committed
# file through node and holds its answers to this repository's own, so a stale one is a
# failing test rather than a page that quietly answers an old way.
#
#   $ website/tools/make_wasm.sh
#
# opt-level=z because this file travels over the network to a reader who has not decided
# yet whether to install anything: 900 KB (315 KB gzipped) against 1.6 MB at the default,
# for a check that takes milliseconds either way on a table a person typed.
set -eu
cd "$(dirname "$0")/.."
target=wasm32-unknown-unknown
rustup target list --installed | grep -qx "$target" || rustup target add "$target"
RUSTFLAGS="-C opt-level=z -C codegen-units=1 -C strip=symbols" \
  cargo rustc --manifest-path ../Cargo.toml --lib --release --target "$target" --crate-type cdylib
cp "../target/$target/release/rulec.wasm" docs/playground/rulec.wasm
ls -l docs/playground/rulec.wasm
