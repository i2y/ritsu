#!/usr/bin/env bash
# Pull the repository's canonical documents into the site.
#
# Five documents (AGENTS.md and the four references under docs/) are
# written once, in the repository, and tested there: tests/docs.rs
# checks their links and the commands they name, tests/codes.rs holds
# docs/codes*.md to `rulec explain --all`, and tests/api.rs holds
# docs/generated-code.md to the tool's own output. Copying them in is
# what keeps the site from becoming a second, drifting copy — so the
# copies are generated here and not committed.
#
# The site's own pages (index, install, tour, checks, generate, compare)
# are authored in docs/ and docs-ja/ and are NOT touched by this script.
set -eu
cd "$(dirname "$0")"
root=..

# ---- English: copy, and make the repository's relative links site-local.
# In AGENTS.md a link reads `docs/reference.md`, because AGENTS.md sits at
# the repository root; on the site every page is a sibling.
sed 's|](docs/|](|g' "$root/AGENTS.md"            > docs/agents.md
cp "$root/docs/reference.md"                        docs/reference.md
cp "$root/docs/formats.md"                          docs/formats.md
cp "$root/docs/generated-code.md"                   docs/generated-code.md
# The worked example links to a rule in the repository; on the site that path does not
# exist, so it points at the file on GitHub instead.
sed 's|](../tests/corpus/ec261.rule)|](https://github.com/i2y/rulec/blob/main/tests/corpus/ec261.rule)|g' \
    "$root/docs/backends.md"                      > docs/backends.md
# Already the output of `rulec explain --all --format markdown`, and a
# test in the repository holds it to that, so it is copied rather than
# regenerated (the site build needs no Rust toolchain).
cp "$root/docs/codes.md"                            docs/codes.md

# ---- Japanese: the ledger has a Japanese rendering of its own; the
# other five are English by the project's own rule (the first reader of
# a reference is an agent), so each copy says so at the top rather than
# leaving a reader wondering whether a translation was lost.
mkdir -p docs-ja/stylesheets docs-ja/images
cp "$root/docs/codes.ja.md"                         docs-ja/codes.md

banner() {
  cat <<'MD'
!!! note "この資料は英語です"

    リファレンス（文法・形式・生成物・エージェント向けの手順）は、第一の読み手がエージェントなので英語で書いています。日本語で読めるのは、ホーム・インストール・表を書く・何を証明するか・生成して呼ぶ・突き合わせと再生・例で見る・自分のルールが入るか、そして**診断コードの台帳**です。

MD
}
for f in agents reference formats generated-code backends; do
  { banner; cat "docs/$f.md"; } > "docs-ja/$f.md"
done

# One stylesheet and one set of images (the diagrams and the screenshots), two docs
# trees: zensical does not follow a symlinked directory, so they are real copies.
cp docs/stylesheets/extra.css docs-ja/stylesheets/extra.css
cp docs/images/*.svg docs/images/*.png docs-ja/images/
echo "synced: agents, reference, formats, generated-code, backends, codes (en + ja)"
