#!/bin/sh
# Build the references bundled with the chobo skill from the pages of docs/.
#
# The skill is for people and agents who use chobo in their own projects, so no link in it may
# leave the skill directory: the pages are copied flat, and a link to a file of the repository
# that is not bundled goes to it on GitHub. SKILL.md is written by hand and is not touched here.
# tests/skill.rs runs this into a scratch directory and fails if the committed copies have
# drifted from the pages.
#
#   $ skills/sync.sh [<directory>]      (skills/chobo unless given)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$here/chobo}
docs=$here/../docs
repo=https://github.com/i2y/ritsu/blob/main/crates/chobo
mkdir -p "$out"

page() {
  sed -e "s|](\.\./|]($repo/|g" "$1" > "$2"
}

for p in reference formats targets codes; do
  page "$docs/$p.md" "$out/$p.md"
done

echo "built the references in $out"
