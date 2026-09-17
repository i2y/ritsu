#!/usr/bin/env bash
# Build BOTH languages, always in this order: the English build cleans
# build/, the Japanese build then writes build/ja. Building only one of
# them silently loses the other — use this script.
set -eu
cd "$(dirname "$0")"
./sync.sh
# The page cache is keyed on the Markdown, not on what renders it, so a change to
# tools/rulelexer.py leaves every cached page exactly as it was: `half_down` went
# uncoloured on the built site for a release after the lexer had been fixed. Dropping the
# cache costs a third of a second on a site this size.
rm -rf .cache
# tools/rulelexer.py colours the .rule fences, and the only way it gets imported is by
# being named as a Markdown extension, so it has to be importable.
export PYTHONPATH="$PWD/tools${PYTHONPATH:+:$PYTHONPATH}"
.venv/bin/zensical build
.venv/bin/zensical build -f zensical.ja.toml
echo "site: build/ (en) + build/ja (ja)"
