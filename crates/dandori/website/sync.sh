#!/bin/sh
# The Japanese site shows the same pictures, uses the same stylesheet and runs the same
# playground as the English one, so they are copied over rather than kept twice (docs-ja/images,
# docs-ja/stylesheets and docs-ja/playground are not committed).
set -eu
cd "$(dirname "$0")"
rm -rf docs-ja/images docs-ja/stylesheets docs-ja/playground
cp -R docs/images docs-ja/images
cp -R docs/stylesheets docs-ja/stylesheets
# The playground is four files: the module, the bundle of the examples, the script and the
# stylesheet. The page in each language loads the same ones.
mkdir -p docs-ja/playground
cp docs/playground/dandori.wasm docs/playground/presets.json docs/playground/playground.js docs/playground/playground.css docs-ja/playground/
