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

from diagram import (DARK, LIGHT, FONT, LINE_W, width, fit, esc, text, card, sheet,
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
SHEET_X, SHEET_W, SHEET_H = 44, 190, 64
CARD_X, CARD_W = 252, 304
PITCH = 150                                  # box to box, down the spine
TOP = 20
CHANNEL = 22                                 # the lane the branch runs down, left of the spine

SPINE_Y = [TOP + i * PITCH for i in range(5)]
JOINTS = [(SPINE_Y[i] + SHEET_H, SPINE_Y[i + 1]) for i in range(3)]

FOOT_Y = SPINE_Y[4] + SHEET_H + 26
FOOT_H = 118
H = FOOT_Y + FOOT_H + 40


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
    alt="九つの層が、何と何のあいだを見張っているか。上から順に 現実の業務 → 写した文書 → 書かれた表 → 生成コード が並び、"
        "隣どうしは写しの関係にある。書かれた表からはもう一つ、証明書（cert.json、五つの証明の中身）が出ていて、左を回る線で"
        "表とつながっている。写した文書と書かれた表のあいだは 9 出典（E116・W120）が、書かれた表と生成コードのあいだは "
        "4 ベクタ・5 網羅・6 モデル検査器が見張る。書かれた表そのものは 1 五つの証明・2 二つの宣言・3 例 が見て、証明書は "
        "7 二つの再検査器が読み直す。いちばん上、現実の業務と写した文書のあいだには層が一つも無く、そこは人が読んで決める"
        "ところ。8 リポジトリのテストだけは継ぎ目ではなく、道具そのものに向いている。",
    spine=[
        ("現実の業務", ["決めたいことそのもの"], None),
        ("写した文書", ["規約、料金表、法令"], None),
        ("書かれた表", ["行と列、宣言した範囲"], ".rule"),
        ("生成コード", ["12 言語と、その呼び方"], "gen"),
        ("証明書", ["五つの証明の中身"], "cert.json"),
    ],
    cards=[
        ("層がありません", ["人が読んで書いたところ。rulec が", "見るのは、ここから下だけ"], ("joint", 0), "human"),
        ("9 — 出典を見る", ["写し間違いを捕まえる。", "@法 別表第一 → E116・W120"], ("joint", 1), "accent"),
        ("1・2・3 — 表を見る", ["五つの証明（抜け、重なり、当たらない", "行、単位、int64）。丸めの宣言と、例"], ("box", 2), "accent"),
        ("4・5・6 — コードを見る", ["12 言語が一バイトずつ一致。網羅の五基準。", "モデル検査器は Rust を全入力で読む"], ("joint", 2), "accent"),
        ("7 — 証拠を読み直す", ["rulec とコードを共有しない二つが読む。", "依存ゼロの一ファイルと、Lean の証明つき"], ("box", 4), "accent"),
    ],
    foot=("8 — 道具そのものを試す",
          ["ここまでの層を出しているのは rulec の実装で、その正しさは証明していません。",
           "わざと壊した 83 本が狙った診断を出し、33 本の規則が毎コミット全言語で走る"]),
    note="出典を引いていない規則には、上から二つめの箱がありません。届かない範囲が表まで広がります。",
)

EN = dict(
    alt="What each of the nine layers stands between. Down the left run the business as it is, the document it "
        "was copied from, the table as written, and the generated code, each a copy of the one above it. The table "
        "produces one more thing, the certificate (cert.json, what the five proofs rest on), joined to it by a line "
        "that runs down the left margin. Between the document and the table stands layer 9, the source check "
        "(E116, W120); between the table and the code stand layers 4, 5 and 6, the vectors, the coverage criteria "
        "and the model checker. The table itself is watched by layers 1, 2 and 3; the certificate is read back by "
        "layer 7, two programs that share no code with rulec. At the topmost joint, between the business and the "
        "document, there is no layer at all: that is where a person read and decided. Only layer 8, the "
        "repository's own tests, is aimed at the tool rather than at anything on the spine.",
    spine=[
        ("The business", ["what you actually mean"], None),
        ("The document", ["terms, a tariff, a statute"], None),
        ("The table", ["rows, columns, ranges"], ".rule"),
        ("The generated code", ["twelve languages"], "gen"),
        ("The certificate", ["what the five proofs rest on"], "cert.json"),
    ],
    cards=[
        ("No layer here", ["A person read, and wrote it down.", "rulec sees only from here down"], ("joint", 0), "human"),
        ("9 — the source", ["Catches a mistyped amount.", "@法 別表第一 → E116, W120"], ("joint", 1), "accent"),
        ("1, 2, 3 — the table", ["Five proofs: gaps, overlaps, dead rows,", "units, int64. Rounding. Examples."], ("box", 2), "accent"),
        ("4, 5, 6 — the code", ["Twelve languages, byte for byte. Five", "coverage criteria. Rust over every input"], ("joint", 2), "accent"),
        ("7 — the evidence, re-read", ["Two programs share no code with rulec:", "one file with no imports, and Lean's proofs"], ("box", 4), "accent"),
    ],
    foot=("8 — the tool itself, tried",
          ["The layers above come out of rulec's implementation, which is not proved correct.",
           "83 rules broken on purpose hold it to its diagnostics; 33 run in every language"]),
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
    # The certificate is the table's other product, so it hangs off the table and not off
    # the code above it: a lane down the left says where it came from.
    y1 = SPINE_Y[2] + SHEET_H / 2
    y2 = SPINE_Y[4] + SHEET_H / 2
    o.append(f'<path d="M{SHEET_X - 2},{y1} H{CHANNEL} V{y2} H{SHEET_X - 8}" fill="none" '
             f'stroke="{c["neutral"]}" stroke-width="{LINE_W}" stroke-linejoin="round" '
             f'marker-end="url(#head-neutral)"/>')

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
