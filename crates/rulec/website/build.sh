#!/usr/bin/env bash
# Build BOTH languages, always in this order: the English build cleans
# build/, the Japanese build then writes build/ja. Building only one of
# them silently loses the other — use this script.
set -eu
cd "$(dirname "$0")"
./sync.sh
# tools/rulelexer.py colours the .rule fences, and the only way it gets imported is by
# being named as a Markdown extension, so it has to be importable.
export PYTHONPATH="$PWD/tools${PYTHONPATH:+:$PYTHONPATH}"
.venv/bin/zensical build
.venv/bin/zensical build -f zensical.ja.toml
echo "site: build/ (en) + build/ja (ja)"
