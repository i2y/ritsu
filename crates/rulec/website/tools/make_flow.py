#!/usr/bin/env python3
"""The front page's one diagram: who makes what, and who takes it.

Four files come from one layout - two languages times two colour schemes - so
the geometry cannot drift between them; only the words change. Run it after
editing the text below. The SVGs are committed because building the site must
not need Python.

    python3 tools/make_flow.py

The picture has two kinds of shape and nothing else:

  * actors - the agent, rulec, a person - are cards: a filled surface with a
    coloured bar along the top and a bold name;
  * artifacts - the things that move between them - are sheets: a thin outline
    with a folded corner, the silhouette of a document.

Every arrow runs from an actor to a sheet or from a sheet to an actor, so
"this actor produces this thing, which that actor consumes" is the only
sentence an arrow can say. There are three colours for the three paths: grey
for what enters and leaves the whole system, indigo for the loop between the
agent and rulec, amber for the detour through a person.

    sources -> Agent -> table -> rulec -> Python and Go
                 ^                |
                 +-- diagnosis <--+          round and round, until it passes
                 ^                |
             answer <- Person <- question     only what the tool cannot decide

No line crosses another: the loop lives between the two cards, the question
drops out of the bottom of rulec, the answer climbs into the bottom of the
agent, and the person sits between those two verticals.
"""

import pathlib

W = 1000

# --- palettes ---------------------------------------------------------------
#
# "actor" is the card surface, "sheet" the paper of an artifact. On the light
# page the sheets are a shade greyer than the cards, on the dark page a shade
# lighter: either way the two kinds of shape differ in fill as well as in
# outline, so the distinction survives a small rendering. The amber is the
# second accent: everything on the path through a person carries it.

DARK = dict(
    ink="#e8ebf4", dim="#98a1b8",
    actor="#171b24", actor_edge="#2b3242",
    sheet="#1e2331", sheet_edge="#3a4257", fold="#2b3242",
    accent="#8e9cff", human="#e0b45c", neutral="#6b7590",
)
LIGHT = dict(
    ink="#10131a", dim="#5b6478",
    actor="#ffffff", actor_edge="#dfe3ee",
    sheet="#f5f6fa", sheet_edge="#cdd3e2", fold="#e3e7f1",
    accent="#4457d8", human="#9c6508", neutral="#8b94ab",
)

FONT = ('-apple-system, BlinkMacSystemFont, "Hiragino Sans", "Noto Sans JP", '
        '"Yu Gothic", Meiryo, "Segoe UI", Roboto, sans-serif')
MONO = 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace'

# --- words ------------------------------------------------------------------
#
# Cards are (name, lines); sheets are (title, lines) with an optional tag - the
# file format, set in monospace at the top right. The line breaks are chosen by
# hand for each language because SVG cannot wrap; fit() below refuses to write
# a file where a line would leave its shape.

JA = dict(
    alt="エージェントが資料を読んで表（.rule）を書き、rulec check にかける。"
        "rulec は診断（どこが・どう直すか・それを起こす入力）を返し、エージェントが直して、"
        "表が通るまで繰り返す。道具では決められないことだけが具体例つきの質問として人に渡り、"
        "人は金額と丸めの向きを答える。表が通ると rulec gen が証明済みの Python と Go を出す。",
    agent=("エージェント", ["資料を読む", "表を書く", "診断のとおりに直す", "決められないことは人へ"]),
    rulec=("rulec", ["7 つを証明する：", "完全性・重なり", "当たらない行・単位", "丸め・オーバーフロー", "例が通ること"]),
    person=("人", ["金額を決める", "丸めの向きを決める", "表を承認する", "コードは書かない"]),
    sources=("元の資料", ["規約の文書", "Excel", "旧実装"]),
    table=("表", ["1 規則 = 1 表"], ".rule"),
    diagnosis=("診断", ["どこが", "どう直すか", "それを起こす入力"], "JSON"),
    question=("具体例つきの質問", ["「山梨県あての S60 の", "運賃はいくらですか」"]),
    answer=("答え", ["金額", "丸めの向き"]),
    code=("Python と Go", ["証明済み", "依存ゼロ", "両言語で同じ答え"]),
    check="rulec check", gen="rulec gen",
    loop="通るまで繰り返す", passed="通ったら",
    only="決められないことだけ", back="ループへ戻る",
)

EN = dict(
    alt="The agent reads the sources and writes the table (.rule), then runs rulec check. "
        "rulec returns a diagnosis - where, how to fix it, and an input that shows the "
        "problem - and the agent fixes the table until it passes. Only what the tool cannot "
        "decide reaches a person, as a question with a concrete case; the person answers "
        "with an amount or a rounding direction. Once the table passes, rulec gen emits "
        "proved Python and Go.",
    agent=("Agent", ["reads the sources", "writes the table", "fixes what rulec finds", "asks a person the rest"]),
    rulec=("rulec", ["proves seven things:", "completeness, overlap,", "dead rows, units,", "rounding, overflow,", "the worked examples"]),
    person=("Person", ["decides amounts", "and which way to round", "approves the table", "never writes code"]),
    sources=("Sources", ["policy documents", "spreadsheets", "legacy code"]),
    table=("Table", ["1 rule = 1 table"], ".rule"),
    diagnosis=("Diagnosis", ["where", "how to fix it", "an input that shows it"], "JSON"),
    question=("A concrete question", ["“What is the fee to 山梨県", "at size S60?”"]),
    answer=("Answer", ["an amount,", "which way to round"]),
    code=("Python & Go", ["already proved", "zero dependencies", "the same answer", "in both languages"]),
    check="rulec check", gen="rulec gen",
    loop="until it passes", passed="once it passes",
    only="only what it cannot decide", back="back into the loop",
)

# --- geometry ---------------------------------------------------------------
#
# One horizontal line carries the whole pipeline - sources, agent, table,
# rulec, code - at Y_IN, so the eye reads it left to right as one sentence.
# The loop is the pair of opposite arrows between the two cards: the table
# goes right along Y_IN, the diagnosis comes back left along Y_BACK. The
# person's band is below, read right to left like every return path.

TOP = 20
Y_IN, Y_BACK = 70, 162               # the two horizontal lines of the loop
CARD_H = 196                         # tall enough to receive both lines
AGENT = (166, TOP, 160, CARD_H)
RULEC = (606, TOP, 160, CARD_H)
SOURCES = (20, Y_IN - 50, 116, 100)  # what goes in, level with the pipeline
CODE = (860, Y_IN - 50, 120, 100)    # what comes out, same
TABLE = (410, Y_IN - 25, 112, 50)
DIAG = (391, Y_BACK - 40, 150, 80)
LOOP_Y = 113                         # the caption between the two lines

Y_BAND = 306                         # centre line of the person's band
X_ASK = 686                          # the question drops from the middle of rulec
X_ANS = 246                          # the answer climbs into the middle of the agent
QUESTION = (X_ASK - 85, Y_BAND - 35, 170, 70)
ANSWER = (X_ANS - 70, Y_BAND - 35, 140, 70)
PERSON = (375, Y_BAND - 74, 170, 148)
H = PERSON[1] + PERSON[3] + TOP

CARD_RX, BAR = 10, 5                 # card corners; the coloured bar on top
FOLD, PAD = 12, 9                    # the sheet's dog-ear; text inset in both shapes
LINE_W = 1.8


# --- measuring text ---------------------------------------------------------

def width(s, size, weight=400, mono=False):
    """A rough advance width - enough to catch a line that will not fit.

    Ideographs and kana are one em square; Latin averages about half an em,
    more for capitals, less for punctuation; bold runs a little wider. The
    renders are still looked at, this only turns an overflow into an error
    instead of a picture with text sticking out of a box."""
    if mono:
        return len(s) * 0.6 * size
    bold = 1.08 if weight >= 600 else 1.0
    w = 0.0
    for ch in s:
        o = ord(ch)
        if o > 0x2E7F:                   # CJK, kana, full-width forms, 「」
            w += 1.0
        elif ch == " ":
            w += 0.27
        elif ch in ",.;:'!|“”":
            w += 0.28
        elif ch in "()[]-=&":
            w += 0.36 * bold
        elif ch.isupper():
            w += 0.66 * bold
        elif ch.isdigit():
            w += 0.56 * bold
        else:
            w += 0.5 * bold
    return w * size


def fit(s, size, avail, weight=400, mono=False):
    if width(s, size, weight, mono) > avail:
        raise ValueError(f"{s!r} needs {width(s, size, weight, mono):.0f}, has {avail}")


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


# --- primitives -------------------------------------------------------------

def text(x, y, s, size, fill, weight=400, anchor="middle", mono=False):
    fam = f' font-family="{MONO}"' if mono else ""
    return (f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}" fill="{fill}"{fam}>{esc(s)}</text>')


def card(rect, spec, bar, c, cid):
    """An actor. The bar along the top is the colour of the path it belongs
    to, which is the only decoration a card gets; the bold name does the rest.
    A clipPath rounds the bar's corners with the card's."""
    x, y, w, h = rect
    name, lines = spec
    cx = x + w / 2
    fit(name, 17, w - 2 * PAD, 700)
    o = [f'<clipPath id="{cid}"><rect x="{x}" y="{y}" width="{w}" height="{h}" '
         f'rx="{CARD_RX}"/></clipPath>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{CARD_RX}" '
         f'fill="{c["actor"]}" stroke="{c["actor_edge"]}"/>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{BAR}" fill="{bar}" '
         f'clip-path="url(#{cid})"/>',
         text(cx, y + 43, name, 17, c["ink"], 700)]
    for i, line in enumerate(lines):
        fit(line, 12.5, w - 2 * PAD)
        o.append(text(cx, y + 72 + i * 19, line, 12.5, c["dim"]))
    if 72 + (len(lines) - 1) * 19 + 8 > h:
        raise ValueError(f"{name}: {len(lines)} lines do not fit in {h}")
    return o


def sheet(rect, spec, c):
    """An artifact: a document silhouette, thin outline, folded corner. The
    fold is what says "a thing that is handed over" at any size, and it is
    the one feature no card has."""
    x, y, w, h = rect
    title, lines = spec[0], spec[1]
    tag = spec[2] if len(spec) > 2 else None
    f = FOLD
    inner = w - 2 * PAD
    o = [f'<path d="M{x},{y} H{x + w - f} L{x + w},{y + f} V{y + h} H{x} Z" '
         f'fill="{c["sheet"]}" stroke="{c["sheet_edge"]}" stroke-linejoin="round"/>',
         f'<path d="M{x + w - f},{y} V{y + f} H{x + w} Z" fill="{c["fold"]}" '
         f'stroke="{c["sheet_edge"]}" stroke-linejoin="round"/>']
    # A title with a tag shares its line with it; leave the tag's width free.
    fit(title, 13, inner - (width(tag, 9.5, mono=True) + 8 if tag else 0), 600)
    o.append(text(x + PAD, y + 22, title, 13, c["ink"], 600, "start"))
    if tag:
        o.append(text(x + w - PAD, y + 22, tag, 9.5, c["dim"], anchor="end", mono=True))
    for i, line in enumerate(lines):
        fit(line, 11.5, inner)
        o.append(text(x + PAD, y + 40 + i * 15, line, 11.5, c["dim"], anchor="start"))
    if 40 + (len(lines) - 1) * 15 + 6 > h:
        raise ValueError(f"{title}: {len(lines)} lines do not fit in {h}")
    return o


def arrow(x1, y1, x2, y2, kind, c):
    """kind names the path - "neutral", "accent" or "human" - which picks both
    the stroke and the matching arrowhead."""
    return (f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c[kind]}" '
            f'stroke-width="{LINE_W}" marker-end="url(#head-{kind})"/>')


def marker(kind, c):
    # userSpaceOnUse keeps the head the same size whatever the stroke width.
    return (f'<marker id="head-{kind}" viewBox="0 0 10 10" refX="9" refY="5" '
            f'markerWidth="9" markerHeight="9" markerUnits="userSpaceOnUse" '
            f'orient="auto"><path d="M0,1 L9,5 L0,9 Z" fill="{c[kind]}"/></marker>')


def right(a, b):
    """The x-extent between two rects: from the right edge of a to the left of b."""
    return a[0] + a[2], b[0]


def mid(a, b):
    x1, x2 = right(a, b)
    return (x1 + x2) / 2


# --- the picture ------------------------------------------------------------

def draw(t, c):
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" '
         f'height="{H}" role="img" aria-label="{esc(t["alt"])}" font-family=\'{FONT}\'>',
         "<defs>", marker("neutral", c), marker("accent", c), marker("human", c), "</defs>"]

    # Shapes first, then arrows, then the words that sit beside arrows, so
    # that a label is never painted under a shape.
    o += card(AGENT, t["agent"], c["accent"], c, "clip-agent")
    o += card(RULEC, t["rulec"], c["accent"], c, "clip-rulec")
    o += card(PERSON, t["person"], c["human"], c, "clip-person")
    for rect, key in [(SOURCES, "sources"), (TABLE, "table"), (DIAG, "diagnosis"),
                      (CODE, "code"), (QUESTION, "question"), (ANSWER, "answer")]:
        o += sheet(rect, t[key], c)

    # -- in: the sources, read by the agent
    x1, x2 = right(SOURCES, AGENT)
    o.append(arrow(x1, Y_IN, x2, Y_IN, "neutral", c))

    # -- the loop, clockwise: table to the right, diagnosis back to the left
    x1, x2 = right(AGENT, TABLE)
    o.append(arrow(x1, Y_IN, x2, Y_IN, "accent", c))
    x1, x2 = right(TABLE, RULEC)
    o.append(arrow(x1, Y_IN, x2, Y_IN, "accent", c))
    o.append(text(mid(TABLE, RULEC), Y_IN - 9, t["check"], 10.5, c["dim"], mono=True))
    x1, x2 = right(DIAG, RULEC)
    o.append(arrow(x2, Y_BACK, x1, Y_BACK, "accent", c))
    x1, x2 = right(AGENT, DIAG)
    o.append(arrow(x2, Y_BACK, x1, Y_BACK, "accent", c))
    fit(t["loop"], 11, DIAG[2])
    o.append(text(TABLE[0] + TABLE[2] / 2, LOOP_Y, t["loop"], 11, c["accent"]))

    # -- out: once the table passes, the code
    x1, x2 = right(RULEC, CODE)
    o.append(arrow(x1, Y_IN, x2, Y_IN, "neutral", c))
    fit(t["gen"], 10.5, x2 - x1 - 12, mono=True)
    fit(t["passed"], 10.5, x2 - x1 - 12)
    o.append(text(mid(RULEC, CODE), Y_IN - 9, t["gen"], 10.5, c["dim"], mono=True))
    o.append(text(mid(RULEC, CODE), Y_IN + 18, t["passed"], 10.5, c["dim"]))

    # -- the detour: what rulec cannot decide goes down to a person, and the
    #    answer comes back up into the agent — that is, into the loop
    bottom = TOP + CARD_H
    o.append(arrow(X_ASK, bottom, X_ASK, QUESTION[1], "human", c))
    fit(t["only"], 11, W - (X_ASK + 8) - 20)
    o.append(text(X_ASK + 8, (bottom + QUESTION[1]) / 2 + 4, t["only"], 11, c["human"],
                  anchor="start"))
    x1, x2 = right(PERSON, QUESTION)
    o.append(arrow(x2, Y_BAND, x1, Y_BAND, "human", c))
    x1, x2 = right(ANSWER, PERSON)
    o.append(arrow(x2, Y_BAND, x1, Y_BAND, "human", c))
    o.append(arrow(X_ANS, ANSWER[1], X_ANS, bottom, "human", c))
    fit(t["back"], 11, PERSON[0] - (X_ANS + 8) - 8)
    o.append(text(X_ANS + 8, (bottom + ANSWER[1]) / 2 + 4, t["back"], 11, c["human"],
                  anchor="start"))

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
