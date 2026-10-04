#!/usr/bin/env bash
# dandori as one file the playground loads: docs/playground/dandori.wasm.
#
# It is committed, like the pictures, so that building the site needs no Rust toolchain; and like
# them it is made from the source, so run this again after a change to what `check`, `build` or
# `doc` answers. crates/dandori/tests/playground.rs drives the committed file through node and holds
# what it answers to what the binary answers, so a stale file fails the tests.
#
#   $ website/dandori/tools/make_wasm.sh
#
# opt-level=z, because the file travels over the network to a reader who has not decided yet
# whether to install anything, and a check of one flow takes milliseconds either way.
#
# A panic's message names the file it is in, and a dependency's file is where Cargo keeps it on
# this machine; --remap-path-prefix writes that as /cargo, so the file names no one's home. The crates
# of ritsu's workspace are named from the workspace's root (crates/dandori/src/…). The site is in
# website/dandori of the workspace, two directories under its root, and the directory Cargo builds
# in is the one it says (`cargo metadata`): the workspace's target/, or CARGO_TARGET_DIR, or what a
# cargo config says.
set -eu
here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/../.." && pwd)"
target=wasm32-unknown-unknown
rustup target list --installed | grep -qx "$target" || rustup target add "$target"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
built="$(cd "$root" && cargo metadata --format-version 1 --no-deps | sed -e 's/.*"target_directory":"\([^"]*\)".*/\1/')"
(cd "$root" && RUSTFLAGS="-C opt-level=z -C codegen-units=1 -C strip=symbols --remap-path-prefix=$cargo_home=/cargo" \
  cargo rustc --package dandori --lib --release --target "$target" --crate-type cdylib)
mkdir -p "$here/docs/playground"
cp "$built/$target/release/dandori.wasm" "$here/docs/playground/dandori.wasm"
chmod 644 "$here/docs/playground/dandori.wasm"
ls -l "$here/docs/playground/dandori.wasm"
