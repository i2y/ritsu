#!/bin/sh
# Build the references bundled with the dandori skill from the site's English pages.
#
# The skill is for people and agents who use dandori in their own projects, so no link in it may
# leave the skill directory: the pages are copied flat, their links to one another are rewritten
# to the copies, and a link to a page that is not bundled goes to the published site. SKILL.md is
# written by hand and is not touched here. tests/skill.rs runs this into a scratch directory and
# fails if the committed copies have drifted from the pages.
#
#   $ skills/sync.sh [<directory>]      (skills/dandori unless given)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$here/dandori}
docs=$here/../website/docs
site=https://i2y.github.io/dandori
mkdir -p "$out"

# One page: the site's wrappers around the diagnostics dropped, the links rewritten, and the
# blank lines the wrappers leave behind squeezed to one.
page() {
  sed -e '/^<div class="dd-term" markdown>$/d' \
      -e '/^<\/div>$/d' \
      -e 's|](reference/codes\.md)|](codes.md)|g' \
      -e 's|](\.\./checks\.md)|](checks.md)|g' \
      -e 's|](\.\./platforms\.md)|](platforms.md)|g' \
      -e 's|](\.\./diagrams\.md)|](diagrams.md)|g' \
      -e "s|](assurance\.md)|]($site/assurance/)|g" \
      -e "s|](install\.md)|]($site/install/)|g" \
      -e "s|](doc/|]($site/doc/|g" \
      "$1" | cat -s > "$2"
}

for p in tour tasks agents jev checks diagrams platforms examples design; do
  page "$docs/$p.md" "$out/$p.md"
done
page "$docs/reference/commands.md" "$out/commands.md"
page "$docs/reference/codes.md" "$out/codes.md"

echo "built the references in $out"
