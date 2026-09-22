#!/usr/bin/env bash
# The screenshots of the page an approver can try a case on (`rulec doc --format html`),
# for the site. They are pictures of real output: the page is rendered from a corpus rule
# by the rulec at hand, opened on one of its own examples (`?example=N`) and on one of its
# own deciders (`#t-<table>`), and photographed by headless Chrome. Re-run after anything
# that changes the page.
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
shot() {  # lang query out
  page "$1"
  # The board fills the window, so one window is one picture — there is nothing below the
  # fold to scroll to, and nothing to crop out of a tall render.
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
    --window-size=1440,680 --virtual-time-budget=4000 --screenshot="$3" "file://$tmp/$1.html$2" >/dev/null 2>&1
}
# Two pictures per language. The board on a case: the form on the left, one card per
# decider, the row that fired lit inside the card it belongs to. Then the same board with
# 基本送料 selected, which is what the address `#t-基本送料` opens on — the card in colour
# and the dock below it holding what `rulec check` verified about that table.
for lang in ja en; do
  shot "$lang" "?example=2" "docs/images/try-$lang.png"
  shot "$lang" "?example=2#t-基本送料" "docs/images/try-dock-$lang.png"
done
ls -la docs/images/try-*.png
