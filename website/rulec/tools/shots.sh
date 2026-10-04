#!/usr/bin/env bash
# The screenshots of the page an approver can try a case on (`rulec doc --format html`),
# for the site. They are pictures of real output: the page is rendered from a corpus rule
# by the rulec at hand, opened on one of its own examples (`?example=N`) and on one of its
# own deciders (`#t-<table>`), and photographed by headless Chrome. Re-run after anything
# that changes the page.
#
# Each language is photographed from the rule it shows. The Japanese pages show the shipping fee
# (`送料.rule`); the English pages show `parcel_rate.rule`, a tariff written in English (pounds,
# inches, USD). (They were taken when the shipping fee's English twin still showed a prefecture in
# Japanese; the twin spells it in English now, from `std/jp/prefectures`, DESIGN §15.182.)
#
#   $ website/rulec/tools/shots.sh [path/to/rulec [languages]]      e.g.  shots.sh ../../target/debug/rulec en
#
# The site is ritsu's website/rulec; the rules are in rulec's crate, crates/rulec, and the rulec at
# hand is the workspace's debug build (CARGO_TARGET_DIR, or the target/ at the workspace's root).
set -eu
cd "$(dirname "$0")/.."
rulec="${1:-${CARGO_TARGET_DIR:-../../target}/debug/rulec}"
langs="${2:-ja en}"
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
command -v google-chrome >/dev/null 2>&1 && chrome=google-chrome
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
rule_of() {  # lang -> the corpus rule that language's pages show
  case "$1" in
    ja) echo ../../crates/rulec/tests/corpus/送料.rule ;;
    en) echo ../../crates/rulec/tests/corpus/parcel_rate.rule ;;
  esac
}
table_of() {  # lang -> the table the second picture selects: the tariff of each rule
  case "$1" in
    ja) echo 基本送料 ;;
    en) echo base_rate ;;
  esac
}
page() {  # lang
  [ -f "$tmp/$1.html" ] || "$rulec" doc --lang "$1" --format html "$(rule_of "$1")" > "$tmp/$1.html"
}
shot() {  # lang query out [width,height]
  page "$1"
  # The board fills the window, so one window is one picture — there is nothing below the
  # fold to scroll to, and nothing to crop out of a tall render.
  #
  # Twice: the page follows the reader's light or dark setting, and so does the site that
  # shows these, so each picture has a twin ending in `-dark`. The scheme is asked for
  # outright — left alone, headless Chrome takes the one this machine is set to, and the
  # light pictures would come out dark on a dark desktop.
  for scheme in light dark; do
    out="$3"; pref=1
    [ "$scheme" = dark ] && { out="${3%.png}-dark.png"; pref=0; }
    "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
      --blink-settings=preferredColorScheme=$pref \
      --window-size="${4:-1440,680}" --virtual-time-budget=4000 --screenshot="$out" "file://$tmp/$1.html$2" >/dev/null 2>&1
  done
}
# Two pictures per language. The board on a case: the form on the left, one card per
# decider, the row that fired lit inside the card it belongs to. Then the same board with
# the tariff selected (`基本送料`, or `base_rate` in parcel_rate), which is what the address
# `#t-<table>` opens on — the card in colour and the dock below it holding what `rulec check`
# verified about that table.
# The front page holds the first one in half a row, where a whole board would be too small to
# read. A narrower window keeps the form, the lit rows and the answer at a size that reads there;
# the result card on the right is what falls off, and the form states the answer anyway.
for lang in $langs; do
  shot "$lang" "?example=2" "docs/images/try-$lang.png"
  shot "$lang" "?example=2#t-$(table_of "$lang")" "docs/images/try-dock-$lang.png"
  shot "$lang" "?example=2" "docs/images/try-top-$lang.png" 1100,620
done
ls -la docs/images/try-*.png
