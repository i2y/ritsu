#!/bin/sh
# The Japanese pages in the browser run the same playground as the English ones, so its files are
# copied over rather than kept twice (docs-ja/playground is not committed): the component
# (playground.js, playground.css), the worker that runs ritsu.wasm, the libraries the editor and the
# Markdown are made of, and the projects and the words it reads. The page that is nothing but the
# playground (docs-ja/play/index.html) is committed, in Japanese, and loads them from there. The
# sites of the languages copy what they share in their own sync.sh, which their build.sh runs.
set -eu
cd "$(dirname "$0")"
rm -rf docs-ja/playground
mkdir -p docs-ja/playground
cp docs/playground/ritsu.wasm docs/playground/worker.js docs/playground/libraries.js \
  docs/playground/projects.json docs/playground/words.json \
  docs/playground/playground.js docs/playground/playground.css docs-ja/playground/
