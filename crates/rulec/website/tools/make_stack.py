#!/usr/bin/env python3
"""The third diagram: how a complicated rule is written when a cell holds so little.

A cell tests its own column and nothing else, which reads like a rule that can only say
small things. The way out is not a bigger cell — it is another table. What one table
produces is written as a column of the next, and the chain carries as far as it needs to.

This draws one real rule, tests/corpus/会員特典.rule, as its three table headers in a row:

    table 重さ判定   | 重量 | -> 区分 |
    table 帯判定     | 区分 | 会員区分 | -> 帯 |
    table 送料表     | 帯 | 支払額 | -> 送料 | -> 倍率 |

The point is the repeated word. `区分` leaves the first table and arrives as a column of
the second; `帯` leaves the second and arrives as a column of the third. A filled column
is a value an earlier step produced; an outlined one is an input of the rule.

Run it after editing the text below. The SVGs are committed because building the site
must not need Python.

    python3 tools/make_stack.py
    python3 tools/make_stack.py --verify [path/to/rulec]

Nothing here is invented. Every column name, every formula and the rule's outputs are read
back out of the .rule file by `--verify`, which also holds the file to `rulec check`. Both
languages draw the same geometry and the rule's own Japanese names; only the words around
the picture change, the way the examples page keeps the sources as they are.
"""

import pathlib
import re
import sys

from diagram import (DARK, LIGHT, FONT, PAD, LINE_W, width, fit, esc, text,
                     sheet_frame, sheet_title, arrow, marker)

HERE = pathlib.Path(__file__).resolve().parent
RULE = HERE.parent.parent / "tests" / "corpus" / "会員特典.rule"

# --- words ------------------------------------------------------------------
#
# A table is (name, input columns, output columns); a column is (name, from) where `from`
# is "in" for an input of the rule, "step" for something an earlier step produced.

TABLES = [
    ("重さ判定", [("重量", "in")], ["区分"]),
    ("帯判定", [("区分", "step"), ("会員区分", "in")], ["帯"]),
    ("送料表", [("帯", "step"), ("支払額", "step")], ["送料", "倍率"]),
]
DERIVE = "derive 支払額 = 商品合計 - 値引"
ASSEMBLE = ["result 請求額 = max(支払額, 0円) + 送料", "define 付与点 = 基本点 × 倍率"]
OUTPUTS = ["請求額", "付与点"]

JA = dict(
    alt="表は何段でも重ねられる。表 重さ判定 が 重量 から 区分 を出し、その 区分 が 表 帯判定 の列になり、"
        "出てきた 帯 が 表 送料表 の列になる。導出した 支払額 も列として入り、最後の表が 送料 と 倍率 を"
        "同時に出して、請求額 と 付与点 になる。色のついた欄は前の段が作った値。",
    lede="会員特典.rule — 表を三段重ねて、出力を二つ返す規則",
    tables="表", makes="が作る", key="色のついた列は、前の段が作った値です",
    derive_tag="導出", out_tag="出力",
)

EN = dict(
    alt="Tables stack as deep as needed. 重さ判定 turns 重量 into 区分, which is a column of "
        "帯判定; the 帯 it produces is a column of 送料表. The derived 支払額 comes in as a "
        "column too, and the last table produces 送料 and 倍率 at once, which become 請求額 "
        "and 付与点. A filled column is a value an earlier step produced.",
    lede="会員特典.rule - one rule: three tables stacked, two outputs returned",
    tables="table", makes="produces", key="A filled column is a value an earlier step produced",
    derive_tag="derived", out_tag="outputs",
)

FILES = [("stack", EN), ("stack-ja", JA)]

# --- geometry ---------------------------------------------------------------
#
# One left-to-right line, the way the opening diagram reads: the three tables in the order
# the file declares them, then the two lines that assemble the outputs. Everything that
# enters a table from outside the chain hangs above it, so the only long lines in the
# picture are the short ones between neighbours.

MARGIN = 20
Y_LEDE = 30                          # names the rule being drawn, so that the picture says
                                     # what it is when it is looked at away from the page
Y_ABOVE = 62                         # baseline of what hangs above a card
CARD_Y, CARD_H = 84, 78              # the row of table cards
Y_CHIP = CARD_Y + 52                 # the middle of the chips, and of the arrows between
GAP = 54                             # room for an arrow and the name it carries
GAP_OUT = 78                         # the last hop carries two names, so it needs more
CHIP_H, CHIP_PAD, CHIP_GAP = 22, 9, 7
ARROW_W = 15                         # the `->` inside a card
FS_CHIP, FS_TITLE, FS_SMALL = 11, 11.5, 10.5


def chip_w(s):
    return width(s, FS_CHIP) + 2 * CHIP_PAD


def card_w(t):
    _, ins, outs = t
    cells = [chip_w(n) for n, _ in ins] + [chip_w(n) for n in outs]
    return PAD * 2 + sum(cells) + CHIP_GAP * (len(cells) - 1) + ARROW_W + CHIP_GAP


CARD_W = [card_w(t) for t in TABLES]
CARD_X = []
x = MARGIN
for w in CARD_W:
    CARD_X.append(x)
    x += w + GAP

SHEET = (CARD_X[-1] + CARD_W[-1] + GAP_OUT, CARD_Y,
         max(width(l, FS_SMALL, mono=True) for l in ASSEMBLE) + 2 * PAD, CARD_H)
W = SHEET[0] + SHEET[2] + MARGIN
Y_OUT = CARD_Y + CARD_H + 34         # the output chips, under the sheet
H = Y_OUT + CHIP_H + 34


# --- primitives of this picture only ----------------------------------------

def chip(x, y, s, kind, c):
    """One column of a table header. Filled when the value came from an earlier step of
    the same file, outlined when it is an input of the rule - which is the whole reading
    of the picture, so it is carried by fill rather than by a legend alone."""
    w = chip_w(s)
    fill = f'fill="{c["accent"]}" fill-opacity="0.18"' if kind != "in" else 'fill="none"'
    edge = c["accent"] if kind != "in" else c["neutral"]
    o = [f'<rect x="{x}" y="{y}" width="{w}" height="{CHIP_H}" rx="5" {fill} '
         f'stroke="{edge}" stroke-opacity="0.8"/>',
         text(x + w / 2, y + CHIP_H / 2 + 4, s, FS_CHIP,
              c["ink"] if kind != "in" else c["dim"])]
    return o, w


def card(i, t, c):
    """A table, shown as the one line of it that matters here: its header."""
    name, ins, outs = t
    x, w = CARD_X[i], CARD_W[i]
    o = [f'<rect x="{x}" y="{CARD_Y}" width="{w}" height="{CARD_H}" rx="10" '
         f'fill="{c["actor"]}" stroke="{c["actor_edge"]}"/>',
         text(x + PAD, CARD_Y + 24, f'table {name}', FS_TITLE, c["ink"], 600,
              anchor="start", mono=True)]
    cx = x + PAD
    for n, kind in ins:
        part, cw = chip(cx, Y_CHIP - CHIP_H / 2, n, kind, c)
        o += part
        cx += cw + CHIP_GAP
    o.append(text(cx + ARROW_W / 2, Y_CHIP + 4, "->", FS_CHIP, c["dim"], mono=True))
    cx += ARROW_W + CHIP_GAP
    for n in outs:
        part, cw = chip(cx, Y_CHIP - CHIP_H / 2, n, "out", c)
        o += part
        cx += cw + CHIP_GAP
    return o


def hangs(x, y, label, tag, c):
    """What enters a table from outside the chain: an input of the rule, or a derived
    value with the formula that made it. Set above the card it feeds, so that no line in
    the picture has to travel."""
    o = []
    if tag:
        o.append(text(x, y - 13, tag, 9.5, c["dim"], anchor="start", mono=True))
    o.append(text(x, y, label, FS_SMALL, c["dim"], anchor="start", mono=True))
    return o


# --- the picture ------------------------------------------------------------

def draw(t, c):
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" '
         f'aria-label="{esc(t["alt"])}" font-family=\'{FONT}\'>',
         "<defs>", marker("accent", c), marker("neutral", c), "</defs>"]

    fit(t["lede"], 12.5, W - 2 * MARGIN)
    o.append(text(MARGIN, Y_LEDE, t["lede"], 12.5, c["dim"], anchor="start"))

    for i, tb in enumerate(TABLES):
        o += card(i, tb, c)

    # What each table takes that the chain does not hand it, hung over that card.
    o += hangs(CARD_X[0] + PAD, Y_ABOVE, "重量", None, c)
    o += hangs(CARD_X[1] + PAD, Y_ABOVE, "会員区分", None, c)
    o += hangs(CARD_X[2] + PAD, Y_ABOVE, DERIVE, t["derive_tag"], c)
    for i in (0, 1, 2):
        o.append(arrow(CARD_X[i] + PAD + 6, Y_ABOVE + 7, CARD_X[i] + PAD + 6, CARD_Y - 4,
                       "neutral", c))

    # The chain: the name one table produces, carried to the column of the next.
    for i in range(len(TABLES) - 1):
        x1 = CARD_X[i] + CARD_W[i]
        x2 = CARD_X[i + 1]
        o.append(arrow(x1 + 4, Y_CHIP, x2 - 4, Y_CHIP, "accent", c))
        carried = TABLES[i][2][0]
        fit(carried, FS_SMALL, x2 - x1 - 16)
        o.append(text((x1 + x2) / 2, Y_CHIP - 10, carried, FS_SMALL, c["accent"]))

    # Out of the last table and into the two lines that assemble the outputs.
    x1 = CARD_X[-1] + CARD_W[-1]
    o.append(arrow(x1 + 4, Y_CHIP, SHEET[0] - 4, Y_CHIP, "accent", c))
    carried = " ".join(TABLES[-1][2])
    fit(carried, FS_SMALL, SHEET[0] - x1 - 16)
    o.append(text((x1 + SHEET[0]) / 2, Y_CHIP - 10, carried, FS_SMALL, c["accent"]))

    o += sheet_frame(SHEET, c)
    for i, line in enumerate(ASSEMBLE):
        fit(line, FS_SMALL, SHEET[2] - 2 * PAD, mono=True)
        o.append(text(SHEET[0] + PAD, SHEET[1] + 32 + i * 20, line, FS_SMALL, c["ink"],
                      anchor="start", mono=True))

    # The two outputs, under the sheet that assembles them.
    cx = SHEET[0] + PAD
    o.append(text(cx, Y_OUT - 10, t["out_tag"], 9.5, c["dim"], anchor="start", mono=True))
    for n in OUTPUTS:
        part, cw = chip(cx, Y_OUT, n, "out", c)
        o += part
        cx += cw + CHIP_GAP
    o.append(arrow(SHEET[0] + PAD + 6, SHEET[1] + SHEET[3] + 2, SHEET[0] + PAD + 6,
                   Y_OUT - 24, "accent", c))

    fit(t["key"], FS_SMALL, W - 2 * MARGIN)
    o.append(text(MARGIN, H - 14, t["key"], FS_SMALL, c["dim"], anchor="start"))

    o.append("</svg>")
    return "\n".join(o) + "\n"


# --- holding the words to the rule ------------------------------------------

def headers(src):
    """Each table in the file, as (name, input column names, output column names)."""
    out, name = [], None
    for line in src.splitlines():
        if line.startswith("table "):
            name = re.match(r"table\s+([^\s(]+)", line).group(1)
        elif line.startswith("|") and "->" in line and name:
            cells = [c.strip() for c in line.strip("|").split("|")]
            ins = [c for c in cells if not c.startswith("->")]
            outs = [re.match(r"->\s*([^\s(:]+)", c).group(1) for c in cells if c.startswith("->")]
            out.append((name, ins, outs))
            name = None
    return out


def verify(rulec):
    import subprocess
    src = RULE.read_text(encoding="utf-8")
    real = headers(src)
    drawn = [(n, [c for c, _ in ins], outs) for n, ins, outs in TABLES]
    if real != drawn:
        raise SystemExit(f"the tables drawn are not the tables in {RULE.name}:\n"
                         f"  file:  {real}\n  drawn: {drawn}")
    for line in [DERIVE, *ASSEMBLE]:
        # The file writes a derive with its type and range; the picture keeps the halves
        # that say what it is made of.
        head, _, tail = line.partition(" = ")
        if not any(l.startswith(head.split()[0]) and head.split()[1] in l and tail in l
                   for l in src.splitlines()):
            raise SystemExit(f"{RULE.name} does not say {line!r}")
    for n in OUTPUTS:
        if not any(l.strip().startswith(n) for l in src.splitlines()):
            raise SystemExit(f"{RULE.name} has no output {n}")
    r = subprocess.run([rulec, "check", str(RULE), "--lang", "en"],
                       capture_output=True, text=True)
    if r.returncode:
        raise SystemExit(r.stdout + r.stderr)
    print("verified against", rulec)


def main(argv):
    # --verify draws as well, so that a test can tell a stale SVG from a fresh one by
    # whether the bytes on disk moved.
    if argv[:1] == ["--verify"]:
        verify(argv[1] if len(argv) > 1 else "rulec")
    images = HERE.parent / "docs" / "images"
    for name, t in FILES:
        for suffix, c in [("", DARK), ("-light", LIGHT)]:
            p = images / f"{name}{suffix}.svg"
            p.write_text(draw(t, c), encoding="utf-8")
            print("wrote", p.relative_to(HERE.parent.parent))


if __name__ == "__main__":
    main(sys.argv[1:])
