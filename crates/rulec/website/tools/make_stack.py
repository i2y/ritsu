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
CORPUS = HERE.parent.parent / "tests" / "corpus"

# --- words ------------------------------------------------------------------
#
# A picture is described per language, because the two draw different rules: the Japanese
# one a three-deep chain, the English one a chain of two beside a table that takes the
# rule's inputs directly. Everything below is read out of the .rule file by --verify.
#
#   tables    (name, [(column, "in" | "step")], [output columns])
#   above     what hangs over a card: (card, tag or None, text)
#   chain     an arrow from one card to the next: (from, to, the name it carries)
#   to_sheet  an arrow into the assembling sheet: (card, name, "straight" | "under")

JA = dict(
    rule="会員特典.rule",
    alt="表は何段でも重ねられる。表 重さ判定 が 重量 から 区分 を出し、その 区分 が 表 帯判定 の列になり、"
        "出てきた 帯 が 表 送料表 の列になる。導出した 支払額 も列として入り、最後の表が 送料 と 倍率 を"
        "同時に出して、請求額 と 付与点 になる。色のついた欄は前の段が作った値。",
    lede="会員特典.rule — 表を三段重ねて、出力を二つ返す規則",
    key="色のついた列は、前の段が作った値です",
    out_tag="出力",
    tables=[
        ("重さ判定", [("重量", "in")], ["区分"]),
        ("帯判定", [("区分", "step"), ("会員区分", "in")], ["帯"]),
        ("送料表", [("帯", "step"), ("支払額", "step")], ["送料", "倍率"]),
    ],
    above=[(0, None, "重量"), (1, None, "会員区分"),
           (2, "導出", "derive 支払額 = 商品合計 - 値引")],
    chain=[(0, 1, "区分"), (1, 2, "帯")],
    to_sheet=[(2, "送料 倍率", "straight")],
    assemble=["result 請求額 = max(支払額, 0円) + 送料", "define 付与点 = 基本点 × 倍率"],
    outputs=["請求額", "付与点"],
)

EN = dict(
    rule="ec261.rule",
    alt="What one table produces is a column of the next: band_of turns the distance and "
        "whether the flight is intra-EU into a band, and amount turns that band into the "
        "compensation. Not every table is in the chain — reduction reads the rule's inputs "
        "directly — and result puts the two together. A filled column is a value an earlier "
        "step produced.",
    lede="ec261.rule - Article 7 of Regulation (EC) 261/2004, as three tables",
    key="A filled column is a value an earlier step produced",
    out_tag="output",
    tables=[
        ("band_of", [("distance", "in"), ("intra_eu", "in")], ["band"]),
        ("amount", [("band", "step")], ["base"]),
        ("reduction", [("distance", "in"), ("intra_eu", "in"), ("delay", "in")], ["factor"]),
    ],
    above=[(0, None, "distance  intra_eu"), (2, None, "distance  intra_eu  delay")],
    chain=[(0, 1, "band")],
    to_sheet=[(1, "base", "under"), (2, "factor", "straight")],
    assemble=["result compensation = base × factor"],
    outputs=["compensation"],
)

FILES = [("stack", EN), ("stack-ja", JA)]

# --- geometry ---------------------------------------------------------------
#
# One left-to-right line, the way the opening diagram reads: the tables in the order the
# file declares them, then the lines that assemble the outputs. What a table takes from
# outside the chain hangs above it, so the only long line in the picture is the one that
# carries a middle table's output past its neighbour, and it runs under the row.

MARGIN = 20
Y_LEDE = 30                          # names the rule being drawn, so that the picture says
                                     # what it is when it is looked at away from the page
Y_ABOVE = 62                         # baseline of what hangs above a card
CARD_Y, CARD_H = 84, 78              # the row of table cards
Y_CHIP = CARD_Y + 52                 # the middle of the chips, and of the arrows between
Y_UNDER = CARD_Y + CARD_H + 20       # the lane a non-adjacent hop runs along
GAP = 54                             # room for an arrow and the name it carries
GAP_OUT = 78                         # the last hop may carry two names, so it needs more
CHIP_H, CHIP_PAD, CHIP_GAP = 22, 9, 7
ARROW_W = 15                         # the `->` inside a card
FS_CHIP, FS_TITLE, FS_SMALL = 11, 11.5, 10.5


def chip_w(s):
    return width(s, FS_CHIP) + 2 * CHIP_PAD


def card_w(tb):
    _, ins, outs = tb
    cells = [chip_w(n) for n, _ in ins] + [chip_w(n) for n in outs]
    return PAD * 2 + sum(cells) + CHIP_GAP * (len(cells) - 1) + ARROW_W + CHIP_GAP


def layout(t):
    """Where everything sits, for this language's rule. The width falls out of the tables:
    a picture of two cards is narrower than one of three, and each file keeps its own."""
    cw = [card_w(tb) for tb in t["tables"]]
    cx, x = [], MARGIN
    for w in cw:
        cx.append(x)
        x += w + GAP
    sheet_w = max(width(l, FS_SMALL, mono=True) for l in t["assemble"]) + 2 * PAD
    sx = cx[-1] + cw[-1] + GAP_OUT
    y_out = CARD_Y + CARD_H + 54
    return dict(cw=cw, cx=cx, sheet=(sx, CARD_Y, sheet_w, CARD_H),
                w=sx + sheet_w + MARGIN, y_out=y_out, h=y_out + CHIP_H + 34)


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


def card(g, i, tb, c):
    """A table, shown as the one line of it that matters here: its header."""
    name, ins, outs = tb
    x, w = g["cx"][i], g["cw"][i]
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
    value with the formula that made it. Set above the card it feeds, so that no short
    line in the picture has to travel."""
    o = []
    if tag:
        o.append(text(x, y - 13, tag, 9.5, c["dim"], anchor="start", mono=True))
    o.append(text(x, y, label, FS_SMALL, c["dim"], anchor="start", mono=True))
    return o


# --- the picture ------------------------------------------------------------

def draw(t, c):
    g = layout(t)
    W, H = g["w"], g["h"]
    sx, sy, sw, sh = g["sheet"]
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" '
         f'aria-label="{esc(t["alt"])}" font-family=\'{FONT}\'>',
         "<defs>", marker("accent", c), marker("neutral", c), "</defs>"]

    fit(t["lede"], 12.5, W - 2 * MARGIN)
    o.append(text(MARGIN, Y_LEDE, t["lede"], 12.5, c["dim"], anchor="start"))

    for i, tb in enumerate(t["tables"]):
        o += card(g, i, tb, c)

    for i, tag, label in t["above"]:
        x = g["cx"][i] + PAD
        fit(label, FS_SMALL, g["cw"][i] + GAP, mono=True)
        o += hangs(x, Y_ABOVE, label, tag, c)
        o.append(arrow(x + 6, Y_ABOVE + 7, x + 6, CARD_Y - 4, "neutral", c))

    for a, b, carried in t["chain"]:
        x1 = g["cx"][a] + g["cw"][a]
        x2 = g["cx"][b]
        o.append(arrow(x1 + 4, Y_CHIP, x2 - 4, Y_CHIP, "accent", c))
        fit(carried, FS_SMALL, x2 - x1 - 16)
        o.append(text((x1 + x2) / 2, Y_CHIP - 10, carried, FS_SMALL, c["accent"]))

    # Into the lines that assemble the outputs. A card beside the sheet reaches it in a
    # straight line; one further left goes under the row, because a line drawn through a
    # card would read as passing into it.
    for i, carried, route in t["to_sheet"]:
        x1 = g["cx"][i] + g["cw"][i]
        if route == "straight":
            o.append(arrow(x1 + 4, Y_CHIP, sx - 4, Y_CHIP, "accent", c))
            fit(carried, FS_SMALL, sx - x1 - 16)
            o.append(text((x1 + sx) / 2, Y_CHIP - 10, carried, FS_SMALL, c["accent"]))
        else:
            mx = g["cx"][i] + g["cw"][i] / 2
            y2, r, turn = Y_CHIP + 22, 10, sx - 26
            o.append(
                f'<path d="M{mx},{CARD_Y + CARD_H} V{Y_UNDER - r} '
                f'A{r},{r} 0 0 0 {mx + r},{Y_UNDER} H{turn - r} '
                f'A{r},{r} 0 0 0 {turn},{Y_UNDER - r} V{y2 + r} '
                f'A{r},{r} 0 0 1 {turn + r},{y2} H{sx - 4}" fill="none" '
                f'stroke="{c["accent"]}" stroke-width="{LINE_W}" '
                f'marker-end="url(#head-accent)"/>')
            fit(carried, FS_SMALL, turn - mx - 40)
            o.append(text(mx + 16, Y_UNDER - 6, carried, FS_SMALL, c["accent"],
                          anchor="start"))

    o += sheet_frame(g["sheet"], c)
    for k, line in enumerate(t["assemble"]):
        fit(line, FS_SMALL, sw - 2 * PAD, mono=True)
        o.append(text(sx + PAD, sy + 32 + k * 20, line, FS_SMALL, c["ink"],
                      anchor="start", mono=True))

    cx = sx + PAD
    o.append(text(cx, g["y_out"] - 10, t["out_tag"], 9.5, c["dim"], anchor="start", mono=True))
    for n in t["outputs"]:
        part, cw = chip(cx, g["y_out"], n, "out", c)
        o += part
        cx += cw + CHIP_GAP
    o.append(arrow(sx + PAD + 6, sy + sh + 2, sx + PAD + 6, g["y_out"] - 24, "accent", c))

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
    """Each language against its own rule: the tables drawn, the lines quoted under them,
    the outputs, and that the file passes `check`."""
    import subprocess
    for _, t in FILES:
        rule = CORPUS / t["rule"]
        src = rule.read_text(encoding="utf-8")
        real = headers(src)
        drawn = [(n, [col for col, _ in ins], outs) for n, ins, outs in t["tables"]]
        if real != drawn:
            raise SystemExit(f"the tables drawn are not the tables in {rule.name}:\n"
                             f"  file:  {real}\n  drawn: {drawn}")
        quoted = [lbl for _, tag, lbl in t["above"] if tag] + list(t["assemble"])
        for line in quoted:
            # The file writes a derive with its type and range; the picture keeps the
            # halves that say what it is made of.
            head, _, tail = line.partition(" = ")
            if not any(l.startswith(head.split()[0]) and head.split()[1] in l and tail in l
                       for l in src.splitlines()):
                raise SystemExit(f"{rule.name} does not say {line!r}")
        for n in t["outputs"]:
            if not any(l.strip().startswith(n) for l in src.splitlines()):
                raise SystemExit(f"{rule.name} has no output {n}")
        r = subprocess.run([rulec, "check", str(rule), "--lang", "en"],
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
