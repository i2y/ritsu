#!/usr/bin/env python3
"""The fifth diagram: where each layer of checking sits, and the one joint nobody watches.

The assurance page lists nine layers in a table, in the order they run. A table is the
wrong shape for the thing that actually matters about them, which is **what each one is a
claim about**. Drawn out, they fall into place:

    現実 ──?── 写した文書 ──9── 書かれた表 ──4·5·6── 生成コード
                                    │
                                  1·2·3

The spine is four things, each one a copy of the one before it. Every layer of checking
lives at a joint — it holds two neighbours to each other — except 1, 2 and 3, which look
at the table on its own. The leftmost joint is the picture's point: **there is no layer
there**. A person read the world and wrote the document, and nothing in this tool can
check that reading. Everything rulec says begins one box to the right.

7 and 8 are not at a joint either: they are aimed at the tool that produces the rest, so
they hang under the right-hand half with no arrow into the spine.

Run it after editing the words below. The SVGs are committed because building the site
must not need Python.

    python3 tools/make_assurance.py
    python3 tools/make_assurance.py --verify

`--verify` holds the picture to the page: every layer number drawn here has to be a row of
the table in assurance.md, with the same name, in both languages. A picture that drifts
from the page it illustrates is worse than no picture.
"""

import pathlib
import re
import sys

from diagram import (DARK, LIGHT, FONT, width, fit, esc, text, card, sheet,
                     arrow, marker)

HERE = pathlib.Path(__file__).resolve().parent

# --- geometry ---------------------------------------------------------------
#
# Tall and narrow, because the column this is read in is about 590 CSS pixels across. The
# wide diagrams on the front page are wrapped in a scrolling box; a page of prose should not
# ask for that, so this one is drawn to fit at its own size — 12.5px text stays 12.5px.
#
# The spine of four things runs down the left; the checking is a column of cards on the
# right, each with an arrow back to what it watches. An arrow landing between two boxes
# means the layer holds those two to each other; one landing on a box means it looks at the
# box alone.

W = 576
SHEET_X, SHEET_W, SHEET_H = 20, 190, 64
CARD_X, CARD_W = 238, 318
PITCH = 160                                  # sheet to sheet, down the spine
TOP = 20

SPINE_Y = [TOP + i * PITCH for i in range(4)]
JOINTS = [(SPINE_Y[i] + SHEET_H, SPINE_Y[i + 1]) for i in range(3)]

FOOT_Y = SPINE_Y[3] + SHEET_H + 26           # the card that watches the tool, full width
FOOT_H = 118
H = FOOT_Y + FOOT_H + 42


def gap_mid(i):
    a, b = JOINTS[i]
    return (a + b) / 2


# --- words ------------------------------------------------------------------
#
#   spine    the four things, left to right, as sheets: (title, lines, tag)
#   above    cards over the spine: (x, width, name, lines, where the arrow lands —
#            an x for a box it looks into, ("joint", i) for a seam it holds together)
#   below    cards under it: (x, width, name, lines, joint index or None, colour)
#   note     the dim line at the foot

JA = dict(
    alt="九つの層が、どの継ぎ目を見張っているか。上から順に 現実の業務 → 写した文書 → 書かれた表 → 生成コード の四つが並び、"
        "隣どうしは写しの関係にある。写した文書と書かれた表のあいだは 9 出典（E116・W120）が、書かれた表と生成コードの"
        "あいだは 4 ベクタ・5 網羅・6 モデル検査器が見張る。書かれた表そのものは 1 五つの証明・2 二つの宣言・3 例 が見る。"
        "いちばん上、現実の業務と写した文書のあいだには層が一つも無く、そこは人が読んで決めるところ。7 証明書と二つの"
        "再検査器、8 リポジトリのテストは継ぎ目ではなく、道具そのものに向いている。",
    spine=[
        ("現実の業務", ["決めたいことそのもの"], None),
        ("写した文書", ["規約、料金表、法令"], None),
        ("書かれた表", ["行と列、宣言した範囲"], ".rule"),
        ("生成コード", ["12 言語と、その呼び方"], "gen"),
    ],
    cards=[
        ("層がありません", ["人が読んで書いたところ。rulec が", "見るのは、ここから下だけ"], ("joint", 0), "human"),
        ("9 — 出典を見る", ["写し間違いを捕まえる。", "@法 別表第一 → E116・W120"], ("joint", 1), "accent"),
        ("1・2・3 — 表を見る", ["五つの証明（抜け、重なり、当たらない", "行、単位、int64）。丸めの宣言と、例"], ("box", 2), "accent"),
        ("4・5・6 — コードを見る", ["12 言語が一バイトずつ一致。網羅の五基準。", "モデル検査器は Rust を全入力で読む"], ("joint", 2), "accent"),
    ],
    foot=("7・8 — 道具そのものを見る",
          ["継ぎ目ではなく、上の層を出している道具そのものへ。渡せる証拠を、",
           "コードを共有しない二つが読み直す。わざと壊した 83 本が診断を確かめる"]),
    note="出典を引いていない規則には、上から二つめの箱がありません。届かない範囲が表まで広がります。",
)

EN = dict(
    alt="Which joint each of the nine layers watches. Down the left run four things — the business as it is, "
        "the document it was copied from, the table as written, and the generated code — each a copy of the one "
        "above it. Between the document and the table stands layer 9, the source check (E116, W120); between the "
        "table and the code stand layers 4, 5 and 6, the vectors, the coverage criteria and the model checker. "
        "The table itself is watched by layers 1, 2 and 3, the five proofs, the two declarations and the examples. "
        "At the topmost joint, between the business and the document, there is no layer at all: that is where a "
        "person read and decided. Layers 7 and 8, the certificate with its two re-checkers and the repository's own "
        "tests, are aimed at the tool rather than at a joint.",
    spine=[
        ("The business", ["what you actually mean"], None),
        ("The document", ["terms, a tariff, a statute"], None),
        ("The table", ["rows, columns, ranges"], ".rule"),
        ("The generated code", ["twelve languages"], "gen"),
    ],
    cards=[
        ("No layer here", ["A person read, and wrote it down.", "rulec sees only from here down"], ("joint", 0), "human"),
        ("9 — the source", ["Catches a mistyped amount.", "@法 別表第一 → E116, W120"], ("joint", 1), "accent"),
        ("1, 2, 3 — the table", ["Five proofs: gaps, overlaps, dead rows,", "units, int64. Rounding. Examples."], ("box", 2), "accent"),
        ("4, 5, 6 — the code", ["Twelve languages, byte for byte. Five", "coverage criteria. Rust over every input"], ("joint", 2), "accent"),
    ],
    foot=("7, 8 — the tool itself",
          ["Not a joint: the side that produces the rest. Evidence read back by two",
           "programs sharing no code with it; 83 rules broken on purpose, each held to its code"]),
    note="A rule that cites no document has no second box, and what nothing reaches widens to the table itself.",
)

FILES = [("assurance-ja", JA), ("assurance", EN)]


def draw(fig, c):
    o = [None, "<defs>", marker("neutral", c), marker("accent", c), marker("human", c), "</defs>"]

    # The spine: four sheets down the left, with a grey arrow through each joint. The arrow
    # is the relation — a copy of the box above it — and a card reaches back to it.
    for y, spec in zip(SPINE_Y, fig["spine"]):
        o += sheet((SHEET_X, y, SHEET_W, SHEET_H), spec, c)
    mid_x = SHEET_X + SHEET_W / 2
    for a, b in JOINTS:
        o.append(arrow(mid_x, a + 6, mid_x, b - 2, "neutral", c))

    card_h = 110
    for i, (name, lines, target, kind) in enumerate(fig["cards"]):
        y = TOP + 16 + i * (card_h + 18)
        o += card((CARD_X, y, CARD_W, card_h), (name, lines), c[kind], c, f"clip-c{i}")
        what, n = target
        to_y = gap_mid(n) if what == "joint" else SPINE_Y[n] + SHEET_H / 2
        to_x = mid_x if what == "joint" else SHEET_X + SHEET_W + 4
        o.append(arrow(CARD_X - 6, y + card_h / 2, to_x, to_y, kind, c))

    o += card((SHEET_X, FOOT_Y, W - 2 * SHEET_X, FOOT_H), fig["foot"], c["accent"], c, "clip-foot")

    fit(fig["note"], 11, W - 40)
    o.append(text(W / 2, H - 16, fig["note"], 11, c["dim"]))

    o[0] = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" '
            f'height="{H}" role="img" aria-label="{esc(fig["alt"])}" font-family=\'{FONT}\'>')
    o.append("</svg>")
    return "\n".join(o) + "\n"


def verify():
    """Every layer the picture numbers has to be a row of the page's own table. The page is
    the text; this is only its picture, and a picture that drifts is worse than none."""
    pages = {"assurance-ja": HERE.parent / "docs-ja" / "assurance.md",
             "assurance": HERE.parent / "docs" / "assurance.md"}
    for name, fig in FILES:
        rows = dict(re.findall(r"^\| \*\*(\d+)\. ([^*]+)\*\* \|",
                               pages[name].read_text(encoding="utf-8"), re.M))
        if len(rows) != 9:
            raise SystemExit(f"{pages[name].name}: 表の行が 9 でなく {len(rows)}")
        heads = [n for n, _, _, _ in fig["cards"]] + [fig["foot"][0]]
        drawn = {n for head in heads for n in re.findall(r"\b([1-9])\b", head)}
        if drawn != set(rows):
            raise SystemExit(f"図が描いている層 {sorted(drawn)} と、表の行 {sorted(rows)} が違う")
    print("verified against the pages")


def main(argv):
    # --verify draws as well, so a test can tell a stale SVG from a fresh one by whether
    # the bytes on disk moved.
    if argv[:1] == ["--verify"]:
        verify()
    images = HERE.parent / "docs" / "images"
    for name, fig in FILES:
        for suffix, c in [("", DARK), ("-light", LIGHT)]:
            p = images / f"{name}{suffix}.svg"
            p.write_text(draw(fig, c), encoding="utf-8")
            print("wrote", p.relative_to(HERE.parent.parent))


if __name__ == "__main__":
    main(sys.argv[1:])
