#!/usr/bin/env bash
# The npm package @i2y/ritsu (ritsu's DESIGN 8.8): `ritsu` built for WASI (wasm32-wasip1), which
# Node runs with node:wasi, as i2y-ritsu-<version>.tgz in the directory given.
#
#   $ packaging/npm/build.sh <out-dir>
#
# It builds the binary for wasm32-wasip1 in the workspace's build directory, which it asks cargo
# for (`cargo metadata`), as website/tools/make_wasm.sh does; with cargo-auditable when it is
# there, as the release builds every binary (DESIGN 3.6), so that the module carries the list of
# the crates it is made of. A panic's message names the file it is in, and a dependency's file is
# where Cargo keeps it on this machine; --remap-path-prefix writes that as /cargo, so the module
# names no one's home (the workspace's own files are named from its root, crates/…). Then it puts
# the package together in <out-dir>/package — package.json, bin/, lib/, ritsu.wasm, README.md, the
# two licenses and THIRD_PARTY_NOTICES — and packs it with `npm pack`. The version of package.json
# is the workspace's; the build stops when the two differ.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="${1:?usage: packaging/npm/build.sh <out-dir>}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
target=wasm32-wasip1

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
declared="$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).version' "$here/package.json")"
if [ "$version" != "$declared" ]; then
  echo "build.sh: packaging/npm/package.json says $declared, and the workspace is $version" >&2
  exit 1
fi

rustup target list --installed | grep -qx "$target" || rustup target add "$target"
built="$(cd "$root" && cargo metadata --format-version 1 --no-deps | sed -e 's/.*"target_directory":"\([^"]*\)".*/\1/')"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
if cargo auditable --version >/dev/null 2>&1; then
  build="auditable build"
else
  build="build"
  echo "build.sh: cargo-auditable is not here; the module will not carry the list of its crates" >&2
fi
# Optimized for size, in one unit, with the whole program linked as one (LTO): of the ways measured
# (DESIGN 8.8), the quickest first run of a command, which is what a run of the command line is,
# and a module a quarter smaller than one optimized for speed. wasm-opt made it smaller still but
# no quicker, and the .tgz hardly smaller. The 8 MiB stack is crates/ritsu/build.rs's.
(cd "$root" && CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_LTO=fat \
  RUSTFLAGS="--remap-path-prefix=$cargo_home=/cargo" cargo $build --release --locked -p ritsu --bin ritsu --target "$target")

pkg="$out/package"
rm -rf "$pkg"
mkdir -p "$pkg/bin" "$pkg/lib"
cp "$here/package.json" "$here/README.md" "$pkg/"
cp "$here"/bin/*.js "$pkg/bin/"
cp "$here"/lib/*.js "$here"/lib/*.d.ts "$pkg/lib/"
cp "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$root/THIRD_PARTY_NOTICES" "$pkg/"
cp "$built/$target/release/ritsu.wasm" "$pkg/ritsu.wasm"
chmod 755 "$pkg"/bin/*.js
chmod 644 "$pkg/ritsu.wasm"

rm -f "$out/i2y-ritsu-$version.tgz"
(cd "$pkg" && npm pack --ignore-scripts --pack-destination "$out" --silent >/dev/null)
ls -l "$pkg/ritsu.wasm" "$out/i2y-ritsu-$version.tgz"
