#!/usr/bin/env bash
# Build dandori's site in both languages into build/, always in this order: the English build
# cleans build/, and the Japanese build then writes build/ja. Building only one of them silently
# loses the other.
#
# This site is one of those inside ritsu's: ../build.sh runs this and copies build/ to
# ../build/dandori, where it is published, so go through that to see the whole site (../serve.sh).
# Zensical is the one ritsu's site installs, in ../.venv (../README.md).
set -eu
cd "$(dirname "$0")"
./sync.sh
# The page cache is keyed on the Markdown, not on what renders it, so a change to
# tools/flowlexer.py would leave every cached page as it was.
rm -rf .cache
# tools/flowlexer.py colours the ```flow fences, and it is imported only by being named as
# a Markdown extension, so it has to be importable.
export PYTHONPATH="$PWD/tools${PYTHONPATH:+:$PYTHONPATH}"
../.venv/bin/zensical build
../.venv/bin/zensical build -f zensical.ja.toml
echo "dandori's site: website/dandori/build/ (en) + build/ja (ja)"
