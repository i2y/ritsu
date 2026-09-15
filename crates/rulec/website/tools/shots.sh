#!/usr/bin/env bash
# The screenshots of the page an approver can try a case on (`rulec doc --format html`),
# for the site. They are pictures of real output: the page is rendered from a corpus rule
# by the rulec at hand, opened on one of its own examples (`?example=N`), and photographed
# by headless Chrome. Re-run after anything that changes the page.
#
#   $ website/tools/shots.sh [path/to/rulec]
set -eu
cd "$(dirname "$0")/.."
rulec="${1:-../target/debug/rulec}"
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
command -v google-chrome >/dev/null 2>&1 && chrome=google-chrome
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
page() {  # lang
  [ -f "$tmp/$1.html" ] || "$rulec" doc --lang "$1" --format html ../tests/corpus/送料.rule > "$tmp/$1.html"
}
shot() {  # lang query width height out
  page "$1"
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size="$3,$4" --screenshot="$5" "file://$tmp/$1.html$2" >/dev/null 2>&1
}
# Two pictures per language: the form with its answer at the top of the page, and the two
# tables further down with the rows that matched lit up. Headless Chrome photographs only
# the top of the window, and a scrolled window comes out blank, so the tables are cut out
# of one tall picture; the offset is where the first table's heading sits on this page.
shot ja "?example=2" 1000 600 docs/images/try-ja.png
shot en "?example=2" 1000 600 docs/images/try-en.png
rows() {  # lang offset out
  shot "$1" "?example=2" 1000 2600 "$tmp/tall-$1.png"
  cp "$tmp/tall-$1.png" "docs/images/$3"
  sips -c 900 1000 --cropOffset "$2" 0 "docs/images/$3" >/dev/null 2>&1
}
rows ja 1560 try-rows-ja.png
rows en 1590 try-rows-en.png
ls docs/images/try-*.png
