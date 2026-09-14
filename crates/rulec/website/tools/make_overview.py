#!/usr/bin/env python3
"""The front page's opening diagram: what the tool does to a table.

Four files come from one layout - two languages times two colour schemes - so
the geometry cannot drift between them; only the words change. Run it after
editing the text below. The SVGs are committed because building the site
must not need Python.

    python3 tools/make_overview.py
    python3 tools/make_overview.py --verify [path/to/rulec]

The shapes and colours are the ones make_flow.py draws with (diagram.py): a
sheet with a folded corner is a thing that gets handed over, a card with a
coloured bar is whoever makes it or takes it, indigo is the path between the
table and rulec, amber is anything that needs a person, grey is what leaves
the system.

    table --check--> rulec --gen, once proved--> Python, TypeScript, Rust, Ruby, Go, Swift
      ^                |
      |                v  the input that falls through the gap
      +-- add the row, run again -- witness (E101)

Where the flow diagram says who does what, this one shows the single idea
the prose has the hardest time with: a row is a box. Every cell narrows its
own column and nothing else, so a row is one interval per axis - a
rectangle - and rulec lays the rectangles over the declared input space and
asks whether they tile it. A gap is a hole, an overlap is a double cover,
and both are decided by arithmetic on the bounds rather than by anyone's
eye. So the picture is a five-row tariff that looks complete as a table and
is not: one box is missing, a witness sits in the hole, and it comes back as
the row to add, with the one thing the tool never invents - the amount -
left blank for a person.

Nothing on the sheets is made up. The table is tools/overview.rule (English
names) and tools/overview-ja.rule (Japanese names). The plane is computed
from its rows the way the checker reads them (box_of), so words and geometry
cannot disagree. The witness is what `rulec check` says about the rule
without its last row - the row the picture shows as missing - and every
line of code is a line `rulec gen` writes for the whole rule, copied
verbatim, with `...` standing for what is left out. `--verify` runs the tool
and holds all three to its output, the way the repository's tests hold the
prose to it; run it after changing the table.
"""

import pathlib
import sys

from diagram import (DARK, LIGHT, FONT, PAD, LINE_W, width, fit, esc, text, card,
                     sheet_frame, sheet_title, arrow, marker)

HERE = pathlib.Path(__file__).resolve().parent

# --- words ------------------------------------------------------------------
#
# The table is (title, subtitle, tag); its rows are (destination, weight
# condition, fee) in the rule's own syntax, and the ghost row is the one that
# is missing. Everything on the plane - the bands, the boxes, the hole - is
# derived from those rows. The witness and the code are the tool's own
# output for the .rule file of the same name, so change the table there and
# here together, and let --verify say whether they still agree.

JA = dict(
    alt="表を書く。rulec は一行を入力の組み合わせの一区画にして並べ、隙間も重なりも無いことを"
        "計算で証明する。抜けがあれば、それを起こす入力（あて先 = 遠隔地, 重量 = 2001g）が"
        "返ってきて、行を足してもう一度。運賃がいくらかだけは人が決める。証明できた表からだけ、"
        "依存ゼロの Python・TypeScript・Rust・Ruby・Go・Swift が出る。",
    table=("表", "業務の人が読んで、承認する", ".rule"),
    head=("あて先", "重量", "→ 運賃"),
    rows=[("近畿圏", "<=2kg", "800円"),
          ("近畿圏", ">2kg <=5kg", "1000円"),
          ("近畿圏", ">5kg", "1300円"),
          ("遠隔地", "<=2kg", "1200円"),
          ("遠隔地", ">5kg", "2000円")],
    ghost=("遠隔地", ">2kg <=5kg", "?円"),
    bands=("近畿圏", "遠隔地"),
    rulec=("rulec", ["一行が、入力の組み合わせの一区画になる",
                     "隙間も重なりも、目ではなく計算で見つける"]),
    witness=("当てはまらない例", "あて先 = 遠隔地, 重量 = 2001g",
             "運賃はいくら？ それだけは人が決める", "E101"),
    code=("Python · TypeScript · Rust · Ruby · Go · Swift",
          ["def fee(dest: Zone, weight: Gram) -> YenInclTax:",
           "    ...",
           "        fee = 800",
           "    ...",
           "        fee = 1000",
           "    ...",
           "        fee = 1300",
           "    ...",
           "    return YenInclTax(_round_up(fee, 10))"],
          ["依存ゼロ・エンジンなし", "どれも同じ答え"], ".py"),
    check="rulec check", gen="rulec gen", proved="証明できたら",
    falls="それを起こす入力", again="行を足して、もう一度",
)

EN = dict(
    alt="Write the table. rulec turns each row into a box in the input space and proves "
        "by computation that the boxes leave no gap and no overlap. If there is a gap, "
        "back comes the input that falls through it (Destination = Overseas, Weight = 2001g): "
        "add the row and run again - only what the fee is takes a person. Only a proved "
        "table generates, and what comes out is dependency-free Python, TypeScript, Rust, Ruby, Go and Swift.",
    table=("Table", "a domain expert reads and approves it", ".rule"),
    head=("Destination", "Weight", "→ Fee"),
    rows=[("Domestic", "<=2kg", "8USD"),
          ("Domestic", ">2kg <=5kg", "10USD"),
          ("Domestic", ">5kg", "13USD"),
          ("Overseas", "<=2kg", "12USD"),
          ("Overseas", ">5kg", "20USD")],
    ghost=("Overseas", ">2kg <=5kg", "?USD"),
    bands=("Domestic", "Overseas"),
    rulec=("rulec", ["each row becomes a box in the input space",
                     "gaps and overlaps are computed, not eyeballed"]),
    witness=("An input that matches no row", "Destination = Overseas, Weight = 2001g",
             "what is the fee? only a person can say", "E101"),
    code=("Python · TypeScript · Rust · Ruby · Go · Swift",
          ["def fee(dest: Zone, weight: Gram) -> USDInclTax:",
           "    ...",
           "        fee = 8",
           "    ...",
           "        fee = 10",
           "    ...",
           "        fee = 13",
           "    ...",
           "    return USDInclTax(_round_up(fee, 1))"],
          ["zero dependencies, no engine,", "the same answer from every one"], ".py"),
    check="rulec check", gen="rulec gen", proved="once proved",
    falls="the input that causes it", again="add the row, run again",
)

FILES = [("overview", EN, "en"), ("overview-ja", JA, "ja")]   # (name, words, --lang)

# --- geometry ---------------------------------------------------------------
#
# One horizontal line carries the pipeline - table, rulec, code - at Y_IN, so
# it reads left to right as one sentence. The witness drops out of the hole,
# through the bottom of the card, onto a sheet; from there the amber path
# turns left and climbs back into the table it came from.
#
# The page scales the picture to its column, so the canvas width sets the
# size of every letter on screen: 1260 units read too small there, 900 too
# big, and this layout lands near 1120. That is why the code sheet shows
# only the short lines of the generated function - a branch with its row
# comment is 460 units wide on its own and would push the canvas back past
# 1150 - and lets the three fees, straight from the file, carry the
# correspondence with the table and the plane.

TOP = 20
Y_IN = 72                            # the pipeline's one horizontal line

TABLE = (20, TOP, 236, 204)
CARD = (TABLE[0] + TABLE[2] + 82, TOP, 314, 236)   # 82: room for "rulec check"
CODE = (CARD[0] + CARD[2] + 76, TOP, 352, 200)     # 76: room for "once proved"
STACK = 8                            # the two sheets behind the code, offset
Y_NOTES = 179                        # where the quiet line under the code sits
W = CODE[0] + CODE[2] + 2 * STACK + 20

COLS = (TABLE[0] + PAD, TABLE[0] + PAD + 72, TABLE[0] + TABLE[2] - PAD)
Y_HEAD, Y_ROW, ROW_H, Y_GHOST = 62, 84, 19, 184   # baselines, from the sheet's top

PLANE = (CARD[0] + 62, CARD[1] + 108, 240, 100)   # the declared input space
RANGE = 10                           # 重量 is declared  range >=1g <=10kg
BAND = PLANE[3] / 2                  # two destinations, two bands
TICKS = (2, 5, 10)

WIT_W = 270
X_BACK = TABLE[0] + TABLE[2] / 2     # the return path climbs into the table here


def kg(v):
    return PLANE[0] + v * PLANE[2] / RANGE


def band_y(b):
    return PLANE[1] + b * BAND


def box_of(row, bands):
    """The rectangle a row is: the destination picks the band, the weight
    condition the interval. Reading the cells this way is the whole point of
    the picture, so the plane is derived from the table, not drawn beside it."""
    band = bands.index(row[0])
    lo, hi = 0, RANGE
    for cond in row[1].split():
        if cond.startswith("<="):
            hi = float(cond[2:-2])
        elif cond.startswith(">"):
            lo = float(cond[1:-2])
    return band, lo, hi


# The hole is the ghost row's box. Both languages must agree on it, since the
# geometry is shared; and the witness - 2001g, the first gram past the row
# that stops at 2kg - sits just inside its left edge.
HOLE = box_of(JA["ghost"], JA["bands"])
assert HOLE == box_of(EN["ghost"], EN["bands"]), "the two languages draw different holes"
DOT = (kg(HOLE[1]) + 10.5, band_y(HOLE[0]) + BAND / 2)
assert kg(HOLE[1]) + 6.5 <= DOT[0] <= kg(HOLE[2]) - 6.5, "the witness is not in the hole"
WIT = (DOT[0] - WIT_W / 2, CARD[1] + CARD[3] + 30, WIT_W, 62)
H = WIT[1] + WIT[3] + TOP
assert CODE[0] + CODE[2] + 2 * STACK + TOP <= W, "the code sheets run off the right edge"


# --- primitives of this picture only ----------------------------------------

def dashed(x, y, w, h, c):
    """The outline of something that is not there yet: the missing box on
    the plane, the missing row in the table. Amber, because filling it in
    is what needs a person."""
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="3" fill="none" '
            f'stroke="{c["human"]}" stroke-width="1" stroke-dasharray="3 2.5"/>')


def cells(y, row, size, fills, mono):
    """One row of the little table: two cells set flush left, the fee flush
    right. Each cell is checked against the room its column really has."""
    a, b, d = row
    c1, c2, c3 = COLS
    fit(a, size, c2 - c1 - 8, mono=mono)
    fit(b, size, c3 - width(d, size, mono=mono) - c2 - 8, mono=mono)
    return [text(c1, y, a, size, fills[0], anchor="start", mono=mono),
            text(c2, y, b, size, fills[1], anchor="start", mono=mono),
            text(c3, y, d, size, fills[2], anchor="end", mono=mono, weight=fills[3])]


def table_sheet(t, c):
    x, y, w, h = TABLE
    title, sub, tag = t["table"]
    o = sheet_frame(TABLE, c) + sheet_title(TABLE, title, tag, c)
    fit(sub, 11.5, w - 2 * PAD)
    o.append(text(x + PAD, y + 40, sub, 11.5, c["dim"], anchor="start"))
    o += cells(y + Y_HEAD, t["head"], 11, (c["dim"], c["dim"], c["dim"], 400), False)
    o.append(f'<line x1="{x + PAD}" y1="{y + Y_HEAD + 6}" x2="{x + w - PAD}" '
             f'y2="{y + Y_HEAD + 6}" stroke="{c["sheet_edge"]}"/>')
    for i, row in enumerate(t["rows"]):
        o += cells(y + Y_ROW + i * ROW_H, row, 12, (c["ink"],) * 3 + (400,), True)
    gy = y + Y_GHOST
    o.append(dashed(x + PAD - 5, gy - 13, w - 2 * PAD + 10, 18, c))
    o += cells(gy, t["ghost"], 12, (c["dim"], c["dim"], c["human"], 600), True)
    if Y_GHOST + 5 + 12 > h:
        raise ValueError(f"the table does not fit in {h}")
    return o


def rulec_card(t, c):
    o = card(CARD, t["rulec"], c["accent"], c, "clip-rulec")
    px, py, pw, ph = PLANE
    o.append(f'<rect x="{px}" y="{py}" width="{pw}" height="{ph}" fill="{c["sheet"]}" '
             f'stroke="{c["sheet_edge"]}"/>')
    for row in t["rows"]:
        b, lo, hi = box_of(row, t["bands"])
        x1, x2 = kg(lo), kg(hi)
        o.append(f'<rect x="{x1}" y="{band_y(b)}" width="{x2 - x1}" height="{BAND}" '
                 f'fill="{c["accent"]}" fill-opacity="0.16" stroke="{c["accent"]}" '
                 f'stroke-opacity="0.7"/>')
        fit(row[2], 10.5, x2 - x1 - 6)
        o.append(text((x1 + x2) / 2, band_y(b) + BAND / 2 + 4, row[2], 10.5, c["dim"]))
    b, lo, hi = HOLE
    o.append(dashed(kg(lo) + 2, band_y(b) + 2, kg(hi) - kg(lo) - 4, BAND - 4, c))
    for b, name in enumerate(t["bands"]):
        fit(name, 10.5, px - 8 - (CARD[0] + PAD))
        o.append(text(px - 8, band_y(b) + BAND / 2 + 4, name, 10.5, c["dim"], anchor="end"))
    # Each tick label ends at its tick, under the box it closes - which is
    # what a cell like <=2kg says - and keeps clear of the witness's drop.
    for v in TICKS:
        o.append(text(kg(v) - 3, py + ph + 14, f"{v}kg", 10.5, c["dim"], anchor="end"))
    # The witness: the one input the picture is about, drawn last so that it
    # sits on top of the hole's outline.
    o.append(f'<circle cx="{DOT[0]}" cy="{DOT[1]}" r="4.5" fill="{c["human"]}" '
             f'stroke="{c["actor"]}" stroke-width="1.5"/>')
    return o


def code_sheets(t, c):
    """Three sheets in a stack - one per language - with the front one open."""
    title, code, notes, tag = t["code"]
    x, y, w, h = CODE
    o = []
    for k in (2, 1):
        o += sheet_frame((x + k * STACK, y + k * STACK, w, h), c)
    o += sheet_frame(CODE, c) + sheet_title(CODE, title, tag, c)
    inner = w - 2 * PAD
    keep = ' xml:space="preserve" style="white-space:pre"'   # indentation is the point
    for i, line in enumerate(code):
        fit(line, 10, inner, mono=True)
        o.append(text(x + PAD, y + 40 + i * 15, line, 10, c["ink"], anchor="start",
                      mono=True, extra=keep))
    if 40 + len(code) * 15 > Y_NOTES:
        raise ValueError(f"{title}: {len(code)} lines of code run into the notes")
    for i, line in enumerate(notes):
        fit(line, 11.5, inner)
        o.append(text(x + PAD, y + Y_NOTES + i * 15, line, 11.5, c["dim"], anchor="start"))
    if Y_NOTES + (len(notes) - 1) * 15 + 6 > h:
        raise ValueError(f"{title}: the code and notes do not fit in {h}")
    return o


def witness_sheet(t, c):
    title, case, then, tag = t["witness"]
    x, y, w, h = WIT
    inner = w - 2 * PAD
    o = sheet_frame(WIT, c) + sheet_title(WIT, title, tag, c)
    fit(case, 11, inner, mono=True)
    o.append(text(x + PAD, y + 40, case, 11, c["ink"], anchor="start", mono=True))
    fit(then, 11.5, inner)
    o.append(text(x + PAD, y + 55, then, 11.5, c["dim"], anchor="start"))
    return o


# --- the picture ------------------------------------------------------------

def draw(t, c):
    # No width or height: the picture takes the width it is given and keeps
    # its shape, which is what the top of a page read on a phone needs.
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" '
         f'aria-label="{esc(t["alt"])}" font-family=\'{FONT}\'>',
         "<defs>", marker("neutral", c), marker("accent", c), marker("human", c), "</defs>"]

    # Shapes first, then arrows, then the words that sit beside arrows, so
    # that a label is never painted under a shape.
    o += table_sheet(t, c)
    o += rulec_card(t, c)
    o += code_sheets(t, c)
    o += witness_sheet(t, c)

    # -- in: the table goes to rulec
    x1, x2 = TABLE[0] + TABLE[2], CARD[0]
    o.append(arrow(x1, Y_IN, x2, Y_IN, "accent", c))
    fit(t["check"], 10.5, x2 - x1 - 12, mono=True)
    o.append(text((x1 + x2) / 2, Y_IN - 9, t["check"], 10.5, c["dim"], mono=True))

    # -- out: once proved, the code
    x1, x2 = CARD[0] + CARD[2], CODE[0]
    o.append(arrow(x1, Y_IN, x2, Y_IN, "neutral", c))
    fit(t["gen"], 10.5, x2 - x1 - 12, mono=True)
    fit(t["proved"], 10.5, x2 - x1 - 12)
    o.append(text((x1 + x2) / 2, Y_IN - 9, t["gen"], 10.5, c["dim"], mono=True))
    o.append(text((x1 + x2) / 2, Y_IN + 18, t["proved"], 10.5, c["dim"]))

    # -- down: the witness falls through the hole, out of the card, onto a sheet
    bottom = CARD[1] + CARD[3]
    o.append(arrow(DOT[0], DOT[1] + 7, DOT[0], WIT[1], "human", c))
    fit(t["falls"], 11, CARD[0] + CARD[2] - (DOT[0] + 8))
    o.append(text(DOT[0] + 8, (bottom + WIT[1]) / 2 + 4, t["falls"], 11, c["human"],
                  anchor="start"))

    # -- back: left along the band, then up into the table, one rounded corner
    yb = WIT[1] + WIT[3] / 2
    top = TABLE[1] + TABLE[3]
    r = 12
    o.append(f'<path d="M{WIT[0]},{yb} H{X_BACK + r} A{r},{r} 0 0 1 {X_BACK},{yb - r} '
             f'V{top}" fill="none" stroke="{c["human"]}" stroke-width="{LINE_W}" '
             f'marker-end="url(#head-human)"/>')
    fit(t["again"], 11, WIT[0] - (X_BACK + 8) - 8)
    o.append(text(X_BACK + 8, (top + yb) / 2 + 4, t["again"], 11, c["human"], anchor="start"))

    o.append("</svg>")
    return "\n".join(o) + "\n"


# --- holding the words to the tool ------------------------------------------

def table_of(rule):
    """The rows of the one table in a .rule file, as tuples of cell text."""
    rows = []
    for line in rule.read_text(encoding="utf-8").splitlines():
        if line.startswith("|") and "->" not in line:
            rows.append(tuple(cell.strip() for cell in line.strip("|").split("|")))
    return rows


def verify(rulec):
    """Nothing on the sheets may be made up. The table on the sheet is the
    table in the .rule file; every line of code is one rulec gen writes for
    it; and the witness is what rulec check says once the last row - the
    ghost - is taken away."""
    import subprocess
    import tempfile
    for name, t, lang in FILES:
        rule = HERE / f"{name}.rule"
        rows = table_of(rule)
        if rows[:-1] != t["rows"] or rows[-1][:2] != t["ghost"][:2]:
            raise SystemExit(f"{name}: the table on the sheet is not the table in {rule.name}")
        with tempfile.TemporaryDirectory() as tmp:
            def run(*args):
                return subprocess.run([rulec, *args, "--lang", lang],
                                      capture_output=True, text=True)
            r = run("gen", str(rule), "--out", tmp)
            if r.returncode:
                raise SystemExit(r.stdout + r.stderr)
            written = (pathlib.Path(tmp) / "python" / "fee.py").read_text(encoding="utf-8")
            for line in t["code"][1]:
                if line.strip() != "..." and line not in written.splitlines():
                    raise SystemExit(f"{name}: rulec gen does not write {line!r}")
            gapped = pathlib.Path(tmp) / rule.name
            lines = rule.read_text(encoding="utf-8").splitlines(keepends=True)
            gapped.write_text("".join(lines[:-1]), encoding="utf-8")
            r = run("check", str(gapped))
            said = f"{t['witness'][0]}: {t['witness'][1]}"
            if said not in r.stdout + r.stderr:
                raise SystemExit(f"{name}: rulec check does not say {said!r}\n{r.stdout}{r.stderr}")
    print("verified against", rulec)


def main(argv):
    if argv[:1] == ["--verify"]:
        verify(argv[1] if len(argv) > 1 else "rulec")
    images = HERE.parent / "docs" / "images"
    for name, t, _ in FILES:
        for suffix, c in [("", DARK), ("-light", LIGHT)]:
            p = images / f"{name}{suffix}.svg"
            p.write_text(draw(t, c), encoding="utf-8")
            print("wrote", p.relative_to(HERE.parent.parent))


if __name__ == "__main__":
    main(sys.argv[1:])
