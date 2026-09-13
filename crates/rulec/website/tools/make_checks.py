#!/usr/bin/env python3
"""The second diagram: one machinery, three different defects.

The opening diagram shows a gap, and shows it well enough that a reader can
come away thinking finding gaps is the whole of it. It is not. The same
arithmetic on the same rectangles decides three things, and this picture puts
them side by side so that the sameness is the message: identical plane,
identical bands, identical ticks, one thing changed in each.

    gap (E101)        a stretch of the plane no row covers
    overlap (E105)    a stretch two rows both cover, under policy unique
    unreachable (E102) a row whose whole stretch an earlier row took first

Run it after editing the text below. The SVGs are committed because building
the site must not need Python.

    python3 tools/make_checks.py
    python3 tools/make_checks.py --verify [path/to/rulec]

Nothing here is made up either. The three tables are the one table in
tools/overview.rule and tools/overview-ja.rule with a single documented edit
each (see VARIANTS), and every line of words under a panel is a line rulec
check really prints for that edit. `--verify` makes the three edits, runs the
tool and holds the words to its output.
"""

import pathlib
import sys

from diagram import (DARK, LIGHT, FONT, PAD, LINE_W, width, fit, esc, text,
                     sheet_title)

HERE = pathlib.Path(__file__).resolve().parent

# --- words ------------------------------------------------------------------
#
# Each panel is (short name, code, policy, the lines rulec prints). A line is
# (text, monospace): the tool sets an input in monospace and prose in the page
# font, and the panel keeps that distinction.

JA = dict(
    alt="同じ検査が三つの欠陥を決める。抜け(E101)は、どの行も覆っていない範囲があること。"
        "重なり(E105)は、policy unique の表で二つの行が同じ範囲を覆っていること。"
        "当てはまらない行(E102)は、policy first の表で先の行がその行の範囲を先に全部取ること。"
        "どれも同じ区画の計算で決まり、抜けと重なりはそれを起こす入力そのものが返る。",
    bands=("近畿圏", "遠隔地"),
    panels=[
        ("抜け", "E101", "policy unique",
         [("当てはまらない例:", False), ("あて先 = 遠隔地, 重量 = 2001g", True)]),
        ("重なり", "E105", "policy unique",
         [("両方に当てはまる例:", False), ("あて先 = 遠隔地, 重量 = 2001g", True)]),
        ("当てはまらない行", "E102", "policy first",
         [("行7 はどの入力にも当てはまりません", False),
          ("この行の範囲は先行する行がすべて先に取ります", False)]),
    ],
    dead="行7",
)

EN = dict(
    alt="One check decides three defects. A gap (E101) is a stretch of the input space no "
        "row covers. An overlap (E105) is a stretch two rows both cover, under policy "
        "unique. An unreachable row (E102) is a row whose whole stretch the earlier rows "
        "take first, under policy first. All three are decided by the same arithmetic on "
        "the same rectangles, and the first two hand back the input that causes them.",
    bands=("Kinki", "Remote"),
    panels=[
        ("Gap", "E101", "policy unique",
         [("An input that matches no row:", False),
          ("Destination = Remote, Weight = 2001g", True)]),
        ("Overlap", "E105", "policy unique",
         [("Both rows match:", False), ("Destination = Remote, Weight = 2001g", True)]),
        ("Unreachable row", "E102", "policy first",
         [("row 7 never matches", False),
          ("the earlier rows take all of this row's range first", False)]),
    ],
    dead="row 7",
)

FILES = [("checks", EN, "en"), ("checks-ja", JA, "ja")]      # (name, words, --lang)

# The single edit each panel makes to the shared table, as (find, replace) on
# the weight cell of one row, plus what it appends. Keeping them here rather
# than in three more .rule files means the table can only be changed in one
# place; --verify applies them to the real file and runs the tool.
VARIANTS = (
    "drop the last row",                  # E101
    (">5kg", ">2kg"),                     # E105: the remote >5kg row swallows >2kg <=5kg
    ("policy unique", "policy first", "| {remote} | >2500g <=4500g | 1600円 |"),   # E102
)

# --- geometry ---------------------------------------------------------------
#
# Three panels of identical size in one row. Everything inside a panel is
# placed from its own origin, so the three planes land on the same pixels and
# the eye can hold them against each other.

MARGIN, GAP = 20, 26
PANEL_W, PANEL_H = 330, 222
LABEL_W = 54                         # room for the band name, right of the panel's pad

PLANE = (PAD + LABEL_W, 56, PANEL_W - PAD * 2 - LABEL_W, 96)   # within a panel
RANGE = 10                           # 重量 is declared  range >=1g <=10kg
BAND = PLANE[3] / 2                  # two destinations, two bands
TICKS = (2, 5, 10)
Y_SAID = 190                         # the first of the two lines rulec prints
DEAD_INSET = 6                       # see dead_box

W = MARGIN * 2 + 3 * PANEL_W + 2 * GAP
H = MARGIN * 2 + PANEL_H

# The rows of the shared table, as (band index, from kg, to kg, fee). Only the
# remote band differs between panels, so the kinki band is written once.
KINKI = [(0, 0, 2, "800円"), (0, 2, 5, "1000円"), (0, 5, 10, "1300円")]
REMOTE = [(1, 0, 2, "1200円"), (1, 5, 10, "2000円"), (1, 2, 5, "1500円")]


def kg(v):
    return PLANE[0] + v * PLANE[2] / RANGE


def band_y(b):
    return PLANE[1] + b * BAND


# --- primitives of this picture only ----------------------------------------

def hatch(cid, colour):
    """A region two rows both cover. Diagonal ruling rather than a solid fill,
    because what is underneath - the two boxes themselves - has to stay
    visible through it."""
    return (f'<pattern id="{cid}" width="6" height="6" patternUnits="userSpaceOnUse" '
            f'patternTransform="rotate(45)"><line x1="0" y1="0" x2="0" y2="6" '
            f'stroke="{colour}" stroke-width="2"/></pattern>')


def box(lo, hi, b, fee, c, opacity="0.16"):
    """One row: the band it names crossed with the weights it accepts."""
    x1, x2 = kg(lo), kg(hi)
    o = [f'<rect x="{x1}" y="{band_y(b)}" width="{x2 - x1}" height="{BAND}" '
         f'fill="{c["accent"]}" fill-opacity="{opacity}" stroke="{c["accent"]}" '
         f'stroke-opacity="0.7"/>']
    if fee:
        fit(fee, 10.5, x2 - x1 - 6)
        o.append(text((x1 + x2) / 2, band_y(b) + BAND / 2 + 4, fee, 10.5, c["dim"]))
    return o


def dashed_box(lo, hi, b, c, colour):
    """The outline of something the table does not have: the gap no row covers."""
    return (f'<rect x="{kg(lo) + 2}" y="{band_y(b) + 2}" width="{kg(hi) - kg(lo) - 4}" '
            f'height="{BAND - 4}" rx="3" fill="none" stroke="{colour}" '
            f'stroke-width="1" stroke-dasharray="3 2.5"/>')


def dead_box(lo, hi, b, label, c):
    """A row nothing reaches. It covers the whole band, exactly like the row
    that swallowed it, so it is drawn inset by a few pixels - otherwise the two
    outlines fall on the same pixels and the nesting cannot be seen."""
    x1, x2, y = kg(lo), kg(hi), band_y(b) + DEAD_INSET
    h = BAND - 2 * DEAD_INSET
    o = [f'<rect x="{x1}" y="{y}" width="{x2 - x1}" height="{h}" rx="3" '
         f'fill="url(#dead)" fill-opacity="0.5" stroke="{c["neutral"]}" '
         f'stroke-width="1" stroke-dasharray="3 2.5"/>']
    fit(label, 10, x2 - x1 - 6)
    o.append(text((x1 + x2) / 2, y + h / 2 + 3.5, label, 10, c["dim"]))
    return o


def dot(lo, hi, b, c):
    """The input the tool hands back. It is 2001g - one gram past the row that
    stops at 2kg - which on this scale is the left edge itself, so the mark is
    set a few pixels in. The assertion keeps it inside the stretch it names.
    It sits at the middle of the band: in the overlap panel that is the line
    between the two lanes, which is where one input being in both rows shows."""
    x = kg(lo) + 10.5
    assert kg(lo) + 6.5 <= x <= kg(hi) - 6.5, "the input is drawn outside its own stretch"
    return (f'<circle cx="{x}" cy="{band_y(b) + BAND / 2}" r="4.5" '
            f'fill="{c["human"]}" stroke="{c["sheet"]}" stroke-width="1.5"/>')


def lane(lo, hi, b, half, fee, c):
    """One of two rows drawn in the same band. Two rows that both name 遠隔地
    occupy one band, so drawn at full height the wider one disappears under the
    narrower and the picture reads as a correct table. Each gets half the
    band's height instead - a way of showing them, not a second destination -
    so that both stretches can be seen at once, which is the defect."""
    x1, x2, y = kg(lo), kg(hi), band_y(b) + half * BAND / 2
    o = [f'<rect x="{x1}" y="{y}" width="{x2 - x1}" height="{BAND / 2}" '
         f'fill="{c["accent"]}" fill-opacity="0.16" stroke="{c["accent"]}" '
         f'stroke-opacity="0.7"/>']
    fit(fee, 10.5, x2 - x1 - 6)
    o.append(text((x1 + x2) / 2, y + BAND / 4 + 4, fee, 10.5, c["dim"]))
    return o


def plane(t, c, which):
    """The declared input space, with the boxes the panel's table draws on it."""
    px, py, pw, ph = PLANE
    o = [f'<rect x="{px}" y="{py}" width="{pw}" height="{ph}" fill="{c["sheet"]}" '
         f'stroke="{c["sheet_edge"]}"/>']
    for b, lo, hi, fee in KINKI:
        o += box(lo, hi, b, fee, c)

    if which == 0:                                    # E101: the row is not there
        for b, lo, hi, fee in REMOTE[:2]:
            o += box(lo, hi, b, fee, c)
        o.append(dashed_box(2, 5, 1, c, c["human"]))
        o.append(dot(2, 5, 1, c))
    elif which == 1:                                  # E105: two rows, one stretch
        o += box(0, 2, 1, "1200円", c)
        o += lane(2, 10, 1, 0, "2000円", c)            # the widened row
        o += lane(2, 5, 1, 1, "1500円", c)
        o.append(f'<rect x="{kg(2)}" y="{band_y(1)}" width="{kg(5) - kg(2)}" '
                 f'height="{BAND}" fill="url(#both)" fill-opacity="0.42" '
                 f'stroke="{c["human"]}" stroke-width="1"/>')
        o.append(dot(2, 5, 1, c))
    else:                                             # E102: a row nothing reaches
        for b, lo, hi, fee in REMOTE:
            # The row that swallowed it gives up its fee label: the ghost sits
            # on top of exactly that stretch, and two labels in one place is
            # worse than one label missing.
            o += box(lo, hi, b, "" if (lo, hi) == (2, 5) else fee, c)
        o += dead_box(2.5, 4.5, 1, t["dead"], c)

    for b, name in enumerate(t["bands"]):
        fit(name, 10.5, LABEL_W - 8)
        o.append(text(px - 8, band_y(b) + BAND / 2 + 4, name, 10.5, c["dim"], anchor="end"))
    # Each tick label ends at its tick, under the box it closes - which is
    # what a cell like <=2kg says.
    for v in TICKS:
        o.append(text(kg(v) - 3, py + ph + 14, f"{v}kg", 10.5, c["dim"], anchor="end"))
    return o


def panel(t, c, i):
    """One check: its name, the code it reports under, the policy it needs,
    the plane, and the words the tool prints."""
    name, code, policy, lines = t["panels"][i]
    o = [f'<rect x="0" y="0" width="{PANEL_W}" height="{PANEL_H}" rx="10" '
         f'fill="{c["actor"]}" stroke="{c["actor_edge"]}"/>']
    o += sheet_title((0, 0, PANEL_W, PANEL_H), name, code, c)
    fit(policy, 10, PANEL_W - 2 * PAD, mono=True)
    o.append(text(PAD, 40, policy, 10, c["dim"], anchor="start", mono=True))
    o += plane(t, c, i)
    for k, (line, mono) in enumerate(lines):
        fit(line, 11 if mono else 11.5, PANEL_W - 2 * PAD, mono=mono)
        o.append(text(PAD, Y_SAID + k * 17, line, 11 if mono else 11.5,
                      c["ink"] if mono else c["dim"], anchor="start", mono=mono))
    if Y_SAID + (len(lines) - 1) * 17 + 6 > PANEL_H:
        raise ValueError(f"{name}: the lines do not fit in {PANEL_H}")
    return o


# --- the picture ------------------------------------------------------------

def draw(t, c):
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" '
         f'aria-label="{esc(t["alt"])}" font-family=\'{FONT}\'>',
         "<defs>", hatch("both", c["human"]), hatch("dead", c["neutral"]), "</defs>"]
    for i in range(3):
        x = MARGIN + i * (PANEL_W + GAP)
        o.append(f'<g transform="translate({x},{MARGIN})">')
        o += panel(t, c, i)
        o.append("</g>")
    o.append("</svg>")
    return "\n".join(o) + "\n"


# --- holding the words to the tool ------------------------------------------

def variants(rule):
    """The three tables, as text, each one documented edit away from the one
    table the opening diagram already draws."""
    lines = rule.read_text(encoding="utf-8").splitlines(keepends=True)
    rows = [i for i, l in enumerate(lines) if l.startswith("|") and "->" not in l]
    remote = lines[rows[-1]].strip("|\n").split("|")[0].strip()

    gap = "".join(lines[:rows[-1]] + lines[rows[-1] + 1:])

    over = list(lines)
    for i in rows:                       # the remote row that reaches the top of the range
        cells = over[i].strip("|\n").split("|")
        if cells[0].strip() == remote and cells[1].strip() == ">5kg":
            over[i] = over[i].replace(">5kg", ">2kg", 1)
    over = "".join(over)

    dead = "".join(lines).replace("policy unique", "policy first", 1)
    dead += VARIANTS[2][2].format(remote=remote) + "\n"
    return gap, over, dead


def verify(rulec):
    import subprocess
    import tempfile
    for name, t, lang in FILES:
        rule = HERE / ("overview.rule" if lang == "en" else "overview-ja.rule")
        with tempfile.TemporaryDirectory() as tmp:
            for i, body in enumerate(variants(rule)):
                f = pathlib.Path(tmp) / f"{i}.rule"
                f.write_text(body, encoding="utf-8")
                r = subprocess.run([rulec, "check", str(f), "--lang", lang],
                                   capture_output=True, text=True)
                out = r.stdout + r.stderr
                _, code, policy, lines = t["panels"][i]
                # The policy is a property of the table, not something every
                # diagnostic repeats, so it is held to the file; everything
                # else the panel shows is held to what the tool printed.
                if policy not in body:
                    raise SystemExit(f"{name} panel {i}: the table is not {policy!r}")
                for want in [f"error[{code}]"] + [l for l, _ in lines]:
                    if want not in out:
                        raise SystemExit(
                            f"{name} panel {i}: rulec check does not say {want!r}\n{out}")
    print("verified against", rulec)


def main(argv):
    # --verify draws as well, so that a test can tell a stale SVG from a fresh
    # one by whether the bytes on disk moved.
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
