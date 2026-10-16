#!/bin/sh
# Pull chobo's reference into the site, and what the Japanese site shares with the English one.
#
# The four pages of crates/chobo/docs (the language, the diagnostics, the JSON chobo reads and
# writes, and what `chobo build` writes) are written once, in chobo's crate, and tested there:
# tests/docs.rs holds their books, commands and links, docs/codes*.md to `chobo explain --all`,
# and docs/reference.md's keywords to src/syntax.rs. Copying them in is what keeps the site from
# becoming a second, drifting copy, so the copies are made here and not committed. A link of
# theirs to a file of the crate (`../examples/…`) goes to it on GitHub, as the skill's copies do.
#
# The site's own pages (index, tour, checks, …) are written in docs/ and docs-ja/ and are NOT
# touched by this script.
set -eu
cd "$(dirname "$0")"
# chobo's crate, where the documents are; this site is ritsu's website/chobo.
docs=../../crates/chobo/docs
repo=https://github.com/i2y/ritsu/blob/main/crates/chobo

page() {
  sed -e "s|](\.\./|]($repo/|g" "$1"
}

# ---- English: the four pages, side by side in docs/.
for p in reference formats targets codes; do
  page "$docs/$p.md" > "docs/$p.md"
done

# ---- Japanese: the diagnostics have a Japanese page of their own; the other three are written in
# English alone, so each copy says so at the top rather than leaving a reader wondering whether a
# translation was lost.
page "$docs/codes.ja.md" > docs-ja/codes.md

banner() {
  cat <<'MD'
!!! note "この資料は英語です"

    chobo のリファレンスのうち、言語・形式・出力先の三つは英語だけで書いています。日本語で読めるのは、このサイトの日本語のページと、診断コードの一覧です。

MD
}
for p in reference formats targets; do
  { banner; page "$docs/$p.md"; } > "docs-ja/$p.md"
done

# One stylesheet and one set of images, two docs trees: zensical does not follow a symlinked
# directory, so they are real copies (docs-ja/images and docs-ja/stylesheets are not committed).
rm -rf docs-ja/images docs-ja/stylesheets
cp -R docs/images docs-ja/images
cp -R docs/stylesheets docs-ja/stylesheets

echo "synced: reference, formats, targets, codes (en + ja), the images and the stylesheet"
