#!/usr/bin/env python3
"""Draw the one diagram the front page needs: who hands what to whom.

Four files come from one layout — two languages times two colour schemes — so the
geometry cannot drift between them and only the words change. Run it after editing
the text below; the SVGs are committed, because building the site must not need
Python.

    python3 tools/make_flow.py

Every actor box states what it takes in and what it gives out, and every arrow is
labelled with **the thing that moves**, not with a verb — a verb on an arrow leaves
the reader to guess what was actually passed. The forward arrows also carry the
command that causes them, because rulec does not act on its own: the agent runs it.

No line crosses another:

    documents -> agent -> rulec -> generated code
                   ^        |
                   |        +-- (2) the diagnosis ---------+
                   |        |
                   |        +-- (3) what it cannot decide -> person
                   +--------------- (4) the answer ---------+
"""

import pathlib

W, H = 1000, 600

# --- palettes ---------------------------------------------------------------

DARK = dict(
    box="#171b24", edge="#2b3242", muted_box="#12151c", muted_edge="#242a37",
    ink="#e8ebf4", dim="#98a1b8", accent="#8e9cff", line="#4a5468",
    rule="#2b3242", chip="#c3cae0",
)
LIGHT = dict(
    box="#ffffff", edge="#dfe3ee", muted_box="#f4f6fb", muted_edge="#e2e6f2",
    ink="#10131a", dim="#5b6478", accent="#3d55d4", line="#9aa3ba",
    rule="#e6e9f3", chip="#404a60",
)

FONT = ('-apple-system, BlinkMacSystemFont, "Hiragino Sans", "Noto Sans JP", '
        '"Yu Gothic", Meiryo, "Segoe UI", Roboto, sans-serif')
MONO = 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace'

# --- words ------------------------------------------------------------------
#
# Each actor is (title, what it does, what it takes in, what it gives out).

JA = dict(
    alt="エージェントは規約や Excel を読んで表（.rule）を書き、rulec check にかける。"
        "rulec は診断（どこが・どう直すか・それを起こす入力）を返し、道具では決められないことは"
        "具体例つきの質問として人に渡る。人は金額と丸めの向きだけを決める。"
        "表が通ったら rulec gen が依存ゼロの Python と Go を出す。",
    in_="入", out_="出",
    source=["規約の文書", "Excel", "旧実装"],
    agent=("エージェント", "読んで書く・直す", "規約・診断・人の答え", "表（.rule）"),
    rulec=("rulec", "7 つを証明する", "表（.rule）", "診断・生成物"),
    person=("人", "業務の判断だけをする", "具体例つきの質問", "答え・承認"),
    out_box=("Python と Go", ["依存ゼロの関数", "両言語で同じ答え"]),
    a_read="読む",
    a_check=("rulec check", "① 表（.rule）"),
    a_gen=("rulec gen", "⑤ 生成物"),
    r_fix=["② 診断 ＝ どこが・どう直すか・", "それを起こす入力 → 直して ① へ"],
    r_ask=["③ 道具では決められないこと", "＝ 具体例つきの質問"],
    r_answer=["④ 答え（金額・丸めの向き）", "→ 表に書いて ① へ"],
    foot_label="このループが終わったとき、手に入るもの",
    foot=[("抜けも矛盾もない表", "人が承認できる形で"),
          ("証明済みの Python と Go", "依存ゼロ・両言語で同じ答え"),
          ("入れる前に分かる影響", "何件が、いくら動くか")],
)

EN = dict(
    alt="The agent reads documents and writes the table, then runs rulec check. rulec "
        "returns the diagnosis — where, how to fix it, and an input that shows the problem. "
        "What the tool cannot decide goes to a person as a question with a concrete case; "
        "the person decides only amounts and rounding. Once the table passes, rulec gen "
        "emits Python and Go with no dependencies.",
    in_="in", out_="out",
    source=["Policy documents", "Spreadsheets", "Legacy code"],
    agent=("Agent", "reads, writes, fixes", "docs, diagnosis, answers", "the table (.rule)"),
    rulec=("rulec", "proves the seven", "the table (.rule)", "diagnosis, code"),
    person=("Person", "only business decisions", "a concrete question", "the answer, approval"),
    out_box=("Python & Go", ["plain functions", "zero dependencies", "same answer in both"]),
    a_read="read",
    a_check=("rulec check", "① the table"),
    a_gen=("rulec gen", "⑤ the code"),
    r_fix=["② the diagnosis — where, how to fix,", "an input that shows it → fix, then ①"],
    r_ask=["③ what the tool cannot decide", "= a question with a real case"],
    r_answer=["④ the answer (amount, rounding)", "→ into the table, then ①"],
    foot_label="What you have when the loop ends",
    foot=[("A table with no gaps", "in a shape a person can approve"),
          ("Proved Python and Go", "zero deps, same answer in both"),
          ("The impact, before you deploy", "how many change, and by how much")],
)

# --- geometry ---------------------------------------------------------------

ROW_Y, ROW_H = 64, 132
S = (24, ROW_Y, 126, ROW_H)          # the documents that start it
A = (238, ROW_Y, 210, ROW_H)         # agent
R = (536, ROW_Y, 210, ROW_H)         # rulec
G = (834, ROW_Y, 142, ROW_H)         # what comes out
P = (550, 300, 240, ROW_H)           # person
MID = ROW_Y + ROW_H // 2             # 130 — where the row-1 arrows run
BOT = ROW_Y + ROW_H                  # 196 — the underside of row 1

FIX_X, FIX_Y, FIX_IN = 580, 242, 340   # rulec -> agent: the diagnosis
ASK_X = 680                            # rulec -> person: what it cannot decide
ANS_Y, ANS_IN = 366, 300               # person -> agent: the answer

FOOT_Y, FOOT_H = 482, 88


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def box(x, y, w, h, c, muted=False):
    f, e = (c["muted_box"], c["muted_edge"]) if muted else (c["box"], c["edge"])
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" '
            f'fill="{f}" stroke="{e}"/>')


def text(x, y, s, c, size=13, fill=None, weight=400, anchor="middle", font=None):
    f = f' font-family=\'{font}\'' if font else ""
    return (f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}" fill="{fill or c["ink"]}"{f}>{esc(s)}</text>')


def actor(rect, spec, c, t):
    """An actor: what it is, what it does, and — the point of the picture — what it
    takes in and what it gives out."""
    x, y, w, h = rect
    title, action, takes, gives = spec
    cx = x + w / 2
    o = [box(x, y, w, h, c),
         text(cx, y + 27, title, c, size=16.5, weight=700),
         text(cx, y + 47, action, c, size=11, fill=c["dim"]),
         f'<line x1="{x + 14}" y1="{y + 60}" x2="{x + w - 14}" y2="{y + 60}" '
         f'stroke="{c["rule"]}" stroke-width="1"/>']
    for i, (label, value) in enumerate([(t["in_"], takes), (t["out_"], gives)]):
        yy = y + 80 + i * 21
        o.append(f'<text x="{x + 14}" y="{yy}" font-size="10.5" text-anchor="start" '
                 f'fill="{c["accent"]}">{esc(label)}'
                 f'<tspan dx="7" font-size="11" fill="{c["dim"]}">{esc(value)}</tspan></text>')
    return o


def arrow(x1, y1, x2, y2, c):
    return (f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c["line"]}" '
            f'stroke-width="1.8" marker-end="url(#head)"/>')


def path(d, c):
    """A feedback edge, in the accent so the loop reads as a loop rather than as
    one more step to the right."""
    return (f'<path d="{d}" fill="none" stroke="{c["accent"]}" stroke-width="1.6" '
            f'marker-end="url(#headA)"/>')


def lines(x, y, ls, c, size=11, fill=None, anchor="middle"):
    return [text(x, y + i * 15, l, c, size=size, fill=fill, anchor=anchor)
            for i, l in enumerate(ls)]


def draw(t, c):
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" '
         f'height="{H}" role="img" aria-label="{esc(t["alt"])}" font-family=\'{FONT}\'>',
         f'<defs><marker id="head" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" '
         f'markerHeight="6" orient="auto-start-reverse">'
         f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{c["line"]}"/></marker>'
         f'<marker id="headA" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" '
         f'markerHeight="6" orient="auto-start-reverse">'
         f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{c["accent"]}"/></marker></defs>']

    # --- where it starts: not an actor, just what the agent is handed
    x, y, w, h = S
    o.append(box(x, y, w, h, c, muted=True))
    o += lines(x + w / 2, y + 52, t["source"], c, size=12, fill=c["dim"])

    o += actor(A, t["agent"], c, t)
    o += actor(R, t["rulec"], c, t)
    o += actor(P, t["person"], c, t)

    # --- what comes out is an artifact, not an actor, so it has no in/out rows
    x, y, w, h = G
    o.append(box(x, y, w, h, c))
    o.append(text(x + w / 2, y + 48, t["out_box"][0], c, size=15, weight=700))
    o += lines(x + w / 2, y + 72, t["out_box"][1], c, size=10.5, fill=c["dim"])

    # --- the forward arrows: the artifact, and the command that moves it
    o.append(arrow(S[0] + S[2] + 6, MID, A[0] - 6, MID, c))
    o.append(text((S[0] + S[2] + A[0]) / 2, MID - 10, t["a_read"], c, size=10.5, fill=c["dim"]))
    for (a, b, spec) in [(A[0] + A[2], R[0], t["a_check"]), (R[0] + R[2], G[0], t["a_gen"])]:
        cmd, what = spec
        o.append(arrow(a + 6, MID, b - 6, MID, c))
        o.append(text((a + b) / 2, MID - 26, cmd, c, size=10, fill=c["chip"], font=MONO))
        o.append(text((a + b) / 2, MID - 10, what, c, size=10.5, fill=c["dim"]))

    # --- (2) rulec -> agent: the diagnosis
    o.append(path(f"M {FIX_X} {BOT} V {FIX_Y} H {FIX_IN} V {BOT + 6}", c))
    o += lines((FIX_IN + FIX_X) / 2, FIX_Y - 24, t["r_fix"], c, size=11, fill=c["accent"])

    # --- (3) rulec -> person, and (4) person -> agent
    o.append(path(f"M {ASK_X} {BOT} V {P[1] - 6}", c))
    o += lines(ASK_X + 14, BOT + 34, t["r_ask"], c, size=11, fill=c["accent"], anchor="start")
    o.append(path(f"M {P[0]} {ANS_Y} H {ANS_IN} V {BOT + 6}", c))
    o += lines((ANS_IN + P[0]) / 2, ANS_Y - 24, t["r_answer"], c, size=11, fill=c["accent"])

    # --- what all of that leaves you with
    o.append(text(24, FOOT_Y - 12, t["foot_label"], c, size=12, fill=c["dim"], anchor="start"))
    o.append(box(24, FOOT_Y, W - 48, FOOT_H, c, muted=True))
    cell = (W - 48) / 3
    for i, (head, sub) in enumerate(t["foot"]):
        cx = 24 + cell * (i + 0.5)
        o.append(text(cx, FOOT_Y + 36, head, c, size=14, weight=700))
        o.append(text(cx, FOOT_Y + 58, sub, c, size=11.5, fill=c["dim"]))
        if i:
            gx = 24 + cell * i
            o.append(f'<line x1="{gx}" y1="{FOOT_Y + 18}" x2="{gx}" y2="{FOOT_Y + FOOT_H - 18}" '
                     f'stroke="{c["muted_edge"]}" stroke-width="1"/>')

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
