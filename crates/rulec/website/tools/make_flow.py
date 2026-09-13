#!/usr/bin/env python3
"""Draw the one diagram the front page needs: who does what, and what comes out.

Four files come from one layout — two languages times two colour schemes — so the
geometry cannot drift between them and only the words change. Run it after editing
the text below; the SVGs are committed, because building the site must not need
Python.

    python3 tools/make_flow.py

The layout is a single left-to-right story with two return paths underneath, and
no line crosses another:

    documents -> agent -> rulec -> generated code
                   ^        |
                   |        +-- what to fix ------------+  (short return)
                   |        |
                   |        +-- what it cannot decide -> person
                   +----------------- the answer --------+  (long return)
"""

import pathlib

W, H = 1000, 500

# --- palettes ---------------------------------------------------------------

DARK = dict(
    box="#171b24", edge="#2b3242", muted_box="#12151c", muted_edge="#242a37",
    ink="#e8ebf4", dim="#98a1b8", accent="#8e9cff", line="#4a5468",
    foot="#141821", foot_edge="#2b3242",
)
LIGHT = dict(
    box="#ffffff", edge="#dfe3ee", muted_box="#f4f6fb", muted_edge="#e2e6f2",
    ink="#10131a", dim="#5b6478", accent="#3d55d4", line="#9aa3ba",
    foot="#f7f8fc", foot_edge="#e2e6f2",
)

FONT = ('-apple-system, BlinkMacSystemFont, "Hiragino Sans", "Noto Sans JP", '
        '"Yu Gothic", Meiryo, "Segoe UI", Roboto, sans-serif')

# --- words ------------------------------------------------------------------

JA = dict(
    alt="人とエージェントと rulec の役割分担: エージェントが表を書き、rulec が証明して直し方を返し、"
        "決められないことだけを人が決める。出てくるのは証明済みの Python と Go と、入れる前に分かる影響。",
    source=["規約の文書", "Excel", "旧実装"],
    agent=("エージェント", ["読んで、表（.rule）に書く", "指摘のとおりに直す"]),
    rulec=("rulec", ["抜け・重なり・使われない行", "単位・丸め・桁あふれ・例", "を証明する"]),
    out=("Python と Go", ["依存ゼロの", "普通の関数"]),
    person=("人", ["金額と丸めの向きを決める", "表を承認する"]),
    a_check="① 検査", a_gen="⑤ 生成", a_read="読む",
    r_fix="② 直し方と、それを起こす入力",
    r_ask="③ 道具が決められないこと",
    r_answer="④ 答え（金額・丸めの向き）",
    foot_label="こうして手に入るもの",
    foot=[("抜けも矛盾もない表", "承認できる形で"),
          ("証明済みの Python と Go", "依存ゼロ・両言語で同じ答え"),
          ("入れる前に分かる影響", "何件が、いくら動くか")],
)

EN = dict(
    alt="Who does what: the agent writes the table, rulec proves it and hands back what to "
        "fix with a witness, and only what cannot be decided goes to a person. Out come "
        "proved Python and Go, and the impact known before you deploy.",
    source=["Policy documents", "Spreadsheets", "Legacy code"],
    agent=("Agent", ["reads them, writes the table", "fixes what is reported"]),
    rulec=("rulec", ["gaps, overlaps, dead rows,", "units, rounding, overflow,", "examples — proves them"]),
    out=("Python & Go", ["plain functions,", "zero dependencies"]),
    person=("Person", ["decides amounts and rounding", "approves the table"]),
    a_check="① check", a_gen="⑤ generate", a_read="read",
    r_fix="② what to fix, with an example",
    r_ask="③ what the tool cannot decide",
    r_answer="④ the answer (amount, rounding)",
    foot_label="What you end up with",
    foot=[("A table with no gaps", "and no contradictions"),
          ("Proved Python and Go", "zero deps, same answer in both"),
          ("The impact, before you deploy", "how many change, and by how much")],
)

# --- geometry ---------------------------------------------------------------

ROW_Y, ROW_H = 72, 92
S = (24, ROW_Y, 146, ROW_H)
A = (256, ROW_Y, 200, ROW_H)
R = (542, ROW_Y, 200, ROW_H)
G = (828, ROW_Y, 148, ROW_H)
P = (560, 268, 240, 84)          # the person
MID = ROW_Y + ROW_H // 2         # 118 — where the row-1 arrows run
BOT = ROW_Y + ROW_H              # 164 — the underside of row 1

FIX_Y = 214                      # the short return
FIX_X = 592                      # rulec -> agent, going down
ASK_X = 692                      # rulec -> person, going down
ANS_Y = 310                      # person -> agent, going left
FIX_IN, ANS_IN = 352, 308        # where the two returns re-enter the agent

FOOT_Y, FOOT_H = 392, 84


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def box(x, y, w, h, c, muted=False):
    f, e = (c["muted_box"], c["muted_edge"]) if muted else (c["box"], c["edge"])
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" '
            f'fill="{f}" stroke="{e}"/>')


def text(x, y, s, c, size=13, fill=None, weight=400, anchor="middle"):
    return (f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}" fill="{fill or c["ink"]}">{esc(s)}</text>')


def titled(rect, title, lines, c):
    """A box with a heading and a couple of quieter lines under it."""
    x, y, w, h = rect
    cx = x + w / 2
    out = [box(x, y, w, h, c)]
    out.append(text(cx, y + 30, title, c, size=17, weight=700))
    for i, l in enumerate(lines):
        out.append(text(cx, y + 52 + i * 17, l, c, size=12, fill=c["dim"]))
    return out


def arrow(x1, y1, x2, y2, c):
    return (f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c["line"]}" '
            f'stroke-width="1.8" marker-end="url(#head)"/>')


def path(d, c):
    """A feedback edge. Drawn in the accent so that the loop reads as a loop and
    not as one more step to the right."""
    return (f'<path d="{d}" fill="none" stroke="{c["accent"]}" stroke-width="1.6" '
            f'marker-end="url(#headA)"/>')


def draw(t, c):
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" '
         f'height="{H}" role="img" aria-label="{esc(t["alt"])}" font-family=\'{FONT}\'>',
         f'<defs><marker id="head" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" '
         f'markerHeight="6" orient="auto-start-reverse">'
         f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{c["line"]}"/></marker>'
         f'<marker id="headA" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" '
         f'markerHeight="6" orient="auto-start-reverse">'
         f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{c["accent"]}"/></marker></defs>']

    # --- row 1
    x, y, w, h = S
    o.append(box(x, y, w, h, c, muted=True))
    for i, l in enumerate(t["source"]):
        o.append(text(x + w / 2, y + 33 + i * 19, l, c, size=12.5, fill=c["dim"]))

    o += titled(A, *t["agent"], c)
    o += titled(R, *t["rulec"], c)
    o += titled(G, *t["out"], c)
    o += titled(P, *t["person"], c)

    for (a, b, label) in [
        (S[0] + S[2], A[0], t["a_read"]),
        (A[0] + A[2], R[0], t["a_check"]),
        (R[0] + R[2], G[0], t["a_gen"]),
    ]:
        o.append(arrow(a + 6, MID, b - 6, MID, c))
        o.append(text((a + b) / 2, MID - 12, label, c, size=11, fill=c["dim"]))

    # --- the short return: rulec -> agent, "what to fix"
    o.append(path(f"M {FIX_X} {BOT} V {FIX_Y} H {FIX_IN} V {BOT + 6}", c))
    o.append(text((FIX_IN + FIX_X) / 2, FIX_Y - 9, t["r_fix"], c, size=11.5, fill=c["accent"]))

    # --- the long return: rulec -> person -> agent
    o.append(path(f"M {ASK_X} {BOT} V {P[1] - 6}", c))
    o.append(text(ASK_X + 14, (BOT + P[1]) / 2 + 4, t["r_ask"], c, size=11.5,
                  fill=c["accent"], anchor="start"))
    o.append(path(f"M {P[0]} {ANS_Y} H {ANS_IN} V {BOT + 6}", c))
    o.append(text((ANS_IN + P[0]) / 2, ANS_Y - 9, t["r_answer"], c, size=11.5, fill=c["accent"]))

    # --- what comes out of all that
    o.append(text(24, FOOT_Y - 12, t["foot_label"], c, size=12, fill=c["dim"], anchor="start"))
    o.append(box(24, FOOT_Y, W - 48, FOOT_H, c, muted=True))
    cell = (W - 48) / 3
    for i, (head, sub) in enumerate(t["foot"]):
        cx = 24 + cell * (i + 0.5)
        o.append(text(cx, FOOT_Y + 34, head, c, size=14, weight=700))
        o.append(text(cx, FOOT_Y + 56, sub, c, size=12, fill=c["dim"]))
        if i:
            gx = 24 + cell * i
            o.append(f'<line x1="{gx}" y1="{FOOT_Y + 16}" x2="{gx}" y2="{FOOT_Y + FOOT_H - 16}" '
                     f'stroke="{c["foot_edge"]}" stroke-width="1"/>')

    o.append("</svg>")
    return "\n".join(o) + "\n"


def main():
    here = pathlib.Path(__file__).resolve().parent.parent / "docs" / "images"
    for name, t in [("flow", EN), ("flow-ja", JA)]:
        for suffix, c in [("", DARK), ("-light", LIGHT)]:
            p = here / f"{name}{suffix}.svg"
            p.write_text(draw(t, c), encoding="utf-8")
            print("wrote", p.relative_to(here.parent.parent))


if __name__ == "__main__":
    main()
