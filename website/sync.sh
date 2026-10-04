#!/bin/sh
# The Japanese page in the browser runs the same playground as the English one, so its files are
# copied over rather than kept twice (docs-ja/playground is not committed). The sites of the
# languages copy what they share in their own sync.sh, which their build.sh runs.
set -eu
cd "$(dirname "$0")"
rm -rf docs-ja/playground
mkdir -p docs-ja/playground
cp docs/playground/ritsu.wasm docs/playground/projects.json docs/playground/playground.js docs/playground/playground.css docs-ja/playground/
