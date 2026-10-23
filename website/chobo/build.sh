#!/usr/bin/env bash
# Build chobo's site in both languages into build/, always in this order: the English build
# cleans build/, and the Japanese build then writes build/ja. Building only one of them silently
# loses the other.
#
# This site is one of those inside ritsu's: ../build.sh runs this and copies build/ to
# ../build/chobo, where it is published, so go through that to see the whole site (../serve.sh).
# Zensical is the one ritsu's site installs, in ../.venv (../README.md).
set -eu
cd "$(dirname "$0")"
./sync.sh
# The page cache is keyed on the Markdown, not on what renders it, so a change to a lexer would
# leave every cached page as it was.
rm -rf .cache
# tools/booklexer.py colours the ```book fences, and the lexer of ritsu's pages,
# ../tools/gatelexer.py, the ```gate fences; each is imported only by being named as a Markdown
# extension, so each has to be importable.
export PYTHONPATH="$PWD/tools:$PWD/../tools${PYTHONPATH:+:$PYTHONPATH}"
../.venv/bin/zensical build
../.venv/bin/zensical build -f zensical.ja.toml
echo "chobo's site: website/chobo/build/ (en) + build/ja (ja)"
