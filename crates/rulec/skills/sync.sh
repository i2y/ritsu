#!/bin/sh
# Build the distributable rulec skill from this repository's own documents.
#
# The skill is for **people and agents who use rulec**, not for working on rulec itself, so
# it has to stand on its own inside someone else's project: no link may leave the skill
# directory. SKILL.md is therefore assembled — header, the body of AGENTS.md with its
# repository links rewritten to the bundled copies, footer — rather than written twice, and so
# is more-of-the-loop.md, from the steps of the loop that SKILL.md leaves out.
# tests/skill.rs repeats the same assembly and fails if the committed files have drifted.
#
#   $ skills/sync.sh
set -eu
cd "$(dirname "$0")"
# The skill itself sits with the other nine at the root of the workspace.
out=../../../skills/rulec

# The repository's links, rewritten to the bundled copies.
links() {
  sed -e 's|(docs/reference\.md)|(reference.md)|g' \
      -e 's|(docs/formats\.md|(formats.md|g' \
      -e 's|(docs/generated-code\.md)|(generated-code.md)|g' \
      -e 's|(docs/backends\.md)|(backends.md)|g' \
      -e 's| (\[docs/codes\.md\](docs/codes\.md))||g' \
      -e 's|\[docs/reference\.md\]|[reference.md]|g' \
      -e 's|\[docs/formats\.md\]|[formats.md]|g' \
      -e 's|\[docs/generated-code\.md\]|[generated-code.md]|g' \
      -e 's|\[docs/backends\.md\]|[backends.md]|g'
}

# AGENTS.md above its own "where to look" table, in three parts: 1 is above `### In CI`; 2, the
# steps of the loop for one occasion each, runs from there to the rule (`---`) above
# `## 2. Language`; 3 is the rest. SKILL.md is read whole whenever the skill is used, and
# tests/skill.rs holds it under 500 lines, so part 2 goes to more-of-the-loop.md and a heading
# in its place points there.
part() {
  awk -v want="$1" '
    NR == 1 { p = 1 }
    /^## 7\. Where to look$/ { exit }
    p == 1 && /^### In CI$/ { p = 2 }
    p == 2 && /^---$/ { p = 3 }
    p == want { print }
  ' ../AGENTS.md
}

body=$({ part 1; cat more-pointer.md; part 3; })
{
  cat header.md
  # `$(...)` strips trailing newlines; the blank line before the footer is put back.
  printf '%s\n\n' "$body" | links
  cat footer.md
} > "$out/SKILL.md"

# One level up, as the file's own sections, and without the blank lines before the rule.
{
  cat more-header.md
  part 2 | sed 's/^### /## /' | links \
    | awk '{ a[n++] = $0 } END { while (n > 0 && a[n-1] == "") n--
             for (i = 0; i < n; i++) print a[i] }'
} > "$out/more-of-the-loop.md"

# The ledger is not bundled: `rulec explain` prints it and is always current.
sed -e 's|\[codes\.md\](codes\.md)|`rulec explain --all`|g' \
    ../docs/reference.md > "$out/reference.md"
cp ../docs/formats.md        "$out/formats.md"
cp ../docs/generated-code.md "$out/generated-code.md"
cp ../docs/compatibility.md  "$out/compatibility.md"
# The worked example points at a rule in this repository; inside someone else's project
# that path leads nowhere, so the link becomes the plain name of the rule.
sed -e 's|\[`tests/corpus/ec261.rule`\](../tests/corpus/ec261.rule)|`ec261.rule`|g' \
    -e 's|\[codes\.md\](codes\.md)|`rulec explain --all`|g' \
    ../docs/backends.md > "$out/backends.md"
# The examples page ends in the site's own navigation buttons, which lead nowhere here.
# Matched by where they point, not by what they are labelled: the label has been renamed
# once already, and the cut failed silently when it was. The page is rulec's site's, which
# ritsu's site holds at website/rulec, at the root of the workspace.
sed -e '/^\[.*\](tour\.md)/,$d' ../../../website/rulec/docs/examples.md \
  | awk '{ a[n++] = $0 } END { while (n > 0 && (a[n-1] == "" || a[n-1] == "---")) n--
           for (i = 0; i < n; i++) print a[i] }' > "$out/examples.md"

echo "built skills/rulec/ ($(wc -l < "$out/SKILL.md" | tr -d ' ') lines of SKILL.md)"
