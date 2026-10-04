#!/usr/bin/env bash
# Build ritsu's site into build/: ritsu's own pages in both languages (build/ and build/ja), then
# the site of each language below, in both of its languages, each built in its own directory by its
# own build.sh and copied to build/<language> (and build/<language>/ja).
#
# Always the whole of it, in this order: ritsu's English build cleans build/, so a language's site
# put there before it would be lost, and the Japanese build then writes build/ja.
set -eu
cd "$(dirname "$0")"
# The sites of the languages, each a directory here with its own zensical.toml and build.sh, built
# into <directory>/build and published at /ritsu/<directory>/.
sites=(dandori)
./sync.sh
# The page cache is keyed on the Markdown, not on what renders it.
rm -rf .cache
.venv/bin/zensical build
.venv/bin/zensical build -f zensical.ja.toml
for site in "${sites[@]}"; do
  "$site/build.sh"
  rm -rf "build/$site"
  cp -R "$site/build" "build/$site"
done
echo "site: build/ (en) + build/ja (ja)"
for site in "${sites[@]}"; do echo "  $site: build/$site (en) + build/$site/ja (ja)"; done
