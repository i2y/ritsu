#!/usr/bin/env bash
# ritsu as one file the playground loads: website/docs/playground/ritsu.wasm (DESIGN 8.7).
#
# It is committed, as rulec's and dandori's were, so that building the site needs no Rust
# toolchain; and like them it is made from the source, so run this again after a change to what
# `ritsu check`, a language's check, `gen` or `doc` answers. crates/ritsu/tests/playground.rs drives
# the committed file through node and holds what it answers to what the library answers, and its
# version to the crate's, so a stale file fails the tests.
#
#   $ website/tools/make_wasm.sh
#
# It builds crates/ritsu-wasm for wasm32-unknown-unknown in the workspace's build directory, which
# it asks cargo for (`cargo metadata`): the workspace's target/, or CARGO_TARGET_DIR, or what a
# cargo config says. opt-level=z, because the file travels over the network to a reader who has
# not decided yet whether to install anything, and a check of a small project takes milliseconds
# either way. A panic's message names the file it is in, and a dependency's file is where Cargo
# keeps it on this machine; --remap-path-prefix writes that as /cargo, so the file names no one's
# home (the workspace's own files are named from its root, crates/…).
set -eu
here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/.." && pwd)"
target=wasm32-unknown-unknown
rustup target list --installed | grep -qx "$target" || rustup target add "$target"
built="$(cd "$root" && cargo metadata --format-version 1 --no-deps | sed -e 's/.*"target_directory":"\([^"]*\)".*/\1/')"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
(cd "$root" && RUSTFLAGS="-C opt-level=z -C codegen-units=1 -C strip=symbols --remap-path-prefix=$cargo_home=/cargo" \
  cargo rustc --package ritsu-wasm --lib --release --target "$target" --crate-type cdylib)
mkdir -p "$here/docs/playground"
cp "$built/$target/release/ritsu_wasm.wasm" "$here/docs/playground/ritsu.wasm"
chmod 644 "$here/docs/playground/ritsu.wasm"
ls -l "$here/docs/playground/ritsu.wasm"
