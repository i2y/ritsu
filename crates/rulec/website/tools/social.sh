#!/usr/bin/env bash
# The repository's social preview — the picture GitHub, Slack and the rest show when a link
# to it is posted. GitHub takes it in Settings → General → Social preview; there is no API
# for it, so this script only makes the file.
#
#   $ website/tools/social.sh
#
# It is the site's own hero, drawn once at 1280×640 (GitHub's size) and photographed by the
# same headless Chrome that takes the screenshots of the approver's page: the same ink, the
# same mark, the same ruling of a table behind it, and the same faces: Inter for the text and
# the site's code face, M PLUS 1 Code, for the wordmark (§15.134). Kept as one HTML document
# rather than an SVG because the ruling and the gradient are the stylesheet's, and a second
# set of colours is a second thing to keep in step. The faces come from Google Fonts, so
# taking the picture needs the network.
set -eu
cd "$(dirname "$0")/.."
out="docs/images/social.png"
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
command -v google-chrome >/dev/null 2>&1 && chrome=google-chrome
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/social.html" <<'ENDHTML'
<!doctype html>
<meta charset="utf-8">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&family=M+PLUS+1+Code:wght@700&display=block">
<style>
  :root { --ink: #10131a; --ink-2: #080a0f; --mark-light: #8e9cff; }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body { width: 1280px; height: 640px; }
  body {
    position: relative; overflow: hidden; color: #fff;
    background: radial-gradient(120% 140% at 14% 0%, #1e2331 0%, var(--ink) 46%, var(--ink-2) 100%);
    font-family: "Inter", -apple-system, "Helvetica Neue", Arial, sans-serif;
  }
  /* The faint ruling of a table, as the site draws it. */
  body::after {
    content: ""; position: absolute; inset: 0; pointer-events: none;
    background-image:
      linear-gradient(rgba(255,255,255,.05) 1px, transparent 1px),
      linear-gradient(90deg, rgba(255,255,255,.05) 1px, transparent 1px);
    background-size: 48px 48px;
    -webkit-mask-image: radial-gradient(110% 100% at 50% 0%, #000 25%, transparent 78%);
  }
  .wrap { position: relative; z-index: 1; height: 100%; padding: 74px 80px; display: flex; flex-direction: column; }
  .top { display: flex; align-items: center; gap: 26px; }
  .name { font-family: "M PLUS 1 Code", SFMono-Regular, Menlo, Consolas, monospace; font-size: 76px; font-weight: 700; letter-spacing: -.045em; }
  .name .c { color: var(--mark-light); }
  .tag { margin-top: 30px; font-size: 52px; font-weight: 600; letter-spacing: -.015em; color: var(--mark-light); }
  .lede { margin-top: 24px; font-size: 27px; line-height: 1.52; color: rgba(255,255,255,.84); max-width: 980px; }
  .lede b { color: #fff; font-weight: 600; }
  .foot { margin-top: auto; padding-top: 26px; border-top: 1px solid rgba(255,255,255,.12);
          display: flex; align-items: baseline; justify-content: space-between; gap: 24px;
          font-size: 19px; white-space: nowrap; color: rgba(255,255,255,.6); }
  .foot .langs { color: rgba(255,255,255,.8); letter-spacing: .01em; }
</style>
<div class="wrap">
  <div class="top">
    <svg width="104" height="104" viewBox="0 0 48 48" role="img" aria-label="rulec">
      <rect x="2" y="2" width="44" height="44" rx="11" fill="#151a25"/>
      <g fill="#8e99b3">
        <rect x="8" y="11" width="16" height="4.4" rx="2.2"/>
        <rect x="28" y="11" width="9" height="4.4" rx="2.2"/>
        <rect x="8" y="21.8" width="16" height="4.4" rx="2.2"/>
        <rect x="28" y="21.8" width="9" height="4.4" rx="2.2"/>
        <rect x="8" y="32.6" width="16" height="4.4" rx="2.2"/>
      </g>
      <circle cx="33.5" cy="34.8" r="9.2" fill="#7a8cff"/>
      <path d="M29.3 35.1l3 3 5.8-6.6" fill="none" stroke="#10131a" stroke-width="2.7"
            stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
    <div class="name">rule<span class="c">c</span></div>
  </div>
  <div class="tag">Write the table. Ship the proof.</div>
  <div class="lede">
    A small language for table-shaped business rules, and a harness for the agent that
    turns them into code.
    <b>The proof happens before the code exists</b>: no gap, no contradiction, no dead row,
    and no value the API contract lets through that the rule would refuse.
  </div>
  <div class="foot">
    <span class="langs">Python · NumPy · TypeScript · JavaScript · Rust · Ruby · PHP · Go · Swift · Java · SQL · Wasm</span>
    <span>no runtime, no dependencies</span>
  </div>
</div>
ENDHTML

# The time budget lets the faces arrive before the picture is taken.
"$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
  --window-size=1280,640 --virtual-time-budget=10000 --screenshot="$out" "file://$tmp/social.html" >/dev/null 2>&1
echo "wrote $out ($(du -h "$out" | cut -f1), $(file -b "$out" | sed 's/,.*//'))"
