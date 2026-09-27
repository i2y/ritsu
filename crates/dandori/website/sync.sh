#!/bin/sh
# The Japanese site shows the same pictures and uses the same stylesheet as the English one,
# so they are copied over rather than kept twice (docs-ja/images and docs-ja/stylesheets are
# not committed).
set -eu
cd "$(dirname "$0")"
rm -rf docs-ja/images docs-ja/stylesheets
cp -R docs/images docs-ja/images
cp -R docs/stylesheets docs-ja/stylesheets
