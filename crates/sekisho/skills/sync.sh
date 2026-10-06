#!/bin/sh
# Build the references bundled with the sekisho skill from the pages under docs/.
#
# The skill is for people and agents who use sekisho in their own projects, so no link in it may
# leave the skill directory: the pages are copied flat, and a link to a file of this repository
# that is not bundled becomes plain text. SKILL.md is written by hand and is not touched here.
# tests/skill.rs runs this into a scratch directory and fails if the committed copies have drifted
# from the pages.
#
#   $ skills/sync.sh [<directory>]      (the root skills/sekisho unless given)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$here/../../../skills/sekisho}
docs=$here/../docs
mkdir -p "$out"

# One page, with the links that leave the skill rewritten.
page() {
  sed -e 's|\[DESIGN\.md\](\.\./DESIGN\.md)|DESIGN.md|g' \
      "$1" > "$2"
}

for p in reference codes; do
  page "$docs/$p.md" "$out/$p.md"
done

echo "built the references in $out"
