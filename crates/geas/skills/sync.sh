#!/bin/sh
# Writes skills/geas/codes.md from `geas explain --all`, so the codes the skill
# carries are the ones the binary explains. The other pages of the skill are
# written by hand. tests/skill.rs runs this into a scratch directory and fails
# when the committed codes.md is not what it writes.
#
#   $ skills/sync.sh [<directory>]      (the root skills/geas unless given)
#
# The geas it runs is $GEAS when that is set, else target/debug/geas after a
# `cargo build`, else the geas on PATH.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$here/../../../skills/geas}
geas=${GEAS:-}
if [ -z "$geas" ]; then
  if [ -x "$here/../target/debug/geas" ]; then
    geas=$here/../target/debug/geas
  else
    geas=geas
  fi
fi
mkdir -p "$out"
{
  echo '# Every diagnostic'
  echo
  echo 'What `geas explain --all` prints: for each code, when it appears, what usually fixes it,'
  echo 'and the smallest run that gives it. `geas explain <code>` prints one, and `--lang ja`'
  echo 'prints them in Japanese.'
  echo
  echo '```text'
  "$geas" explain --all
  echo '```'
} > "$out/codes.md"
echo "wrote $out/codes.md"
