#!/bin/sh
# Build the distributable rulec skill from this repository's own documents.
#
# The skill is for **people and agents who use rulec**, not for working on rulec itself, so
# it has to stand on its own inside someone else's project: no link may leave the skill
# directory. SKILL.md is therefore assembled — header, the body of AGENTS.md with its
# repository links rewritten to the bundled copies, footer — rather than written twice.
# tests/skill.rs repeats the same assembly and fails if the committed files have drifted.
#
#   $ skills/sync.sh
set -eu
cd "$(dirname "$0")"

# The body: everything in AGENTS.md above its own "where to look" table.
body=$(sed -n '1,/^## 7\. Where to look$/p' ../AGENTS.md | sed '$d')

{
  cat header.md
  # `$(...)` strips trailing newlines; the blank line before the footer is put back.
  printf '%s\n\n' "$body" | sed \
    -e 's|(docs/reference\.md)|(reference.md)|g' \
    -e 's|(docs/formats\.md|(formats.md|g' \
    -e 's|(docs/generated-code\.md)|(generated-code.md)|g' \
    -e 's| (\[docs/codes\.md\](docs/codes\.md))||g' \
    -e 's|\[docs/reference\.md\]|[reference.md]|g' \
    -e 's|\[docs/formats\.md\]|[formats.md]|g' \
    -e 's|\[docs/generated-code\.md\]|[generated-code.md]|g'
  cat footer.md
} > rulec/SKILL.md

# The ledger is not bundled: `rulec explain` prints it and is always current.
sed -e 's|\[codes\.md\](codes\.md)|`rulec explain --all`|g' \
    ../docs/reference.md > rulec/reference.md
cp ../docs/formats.md        rulec/formats.md
cp ../docs/generated-code.md rulec/generated-code.md
# The examples page ends in the site's own navigation buttons, which lead nowhere here.
sed -e '/^\[Write a table\](tour\.md)/,$d' ../website/docs/examples.md \
  | awk '{ a[n++] = $0 } END { while (n > 0 && (a[n-1] == "" || a[n-1] == "---")) n--
           for (i = 0; i < n; i++) print a[i] }' > rulec/examples.md

echo "built skills/rulec/ ($(wc -l < rulec/SKILL.md | tr -d ' ') lines of SKILL.md)"
