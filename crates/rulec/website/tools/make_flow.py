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

    sources -> Agent -> table -> rulec -> Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift, SQL
                 ^                |
                 +-- diagnosis <--+          round and round, until it passes
                 ^                |
             answer <- Person <- question     only what the tool cannot decide

No line crosses another: the loop lives between the two cards, the question
drops out of the bottom of rulec, the answer climbs into the bottom of the
agent, and the person sits between those two verticals.
"""

import pathlib
import sys

# The palettes, the fonts, the text metrics and the two shapes are shared
# with make_overview.py, so that the two pictures cannot drift apart.
from diagram import DARK, LIGHT, FONT, fit, esc, text, card, sheet, arrow, marker


# --- words ------------------------------------------------------------------
#
# Cards are (name, lines); sheets are (title, lines) with an optional tag - the
# file format, set in monospace at the top right. The line breaks are chosen by
# hand for each language because SVG cannot wrap; fit() below refuses to write
# a file where a line would leave its shape.

# The languages `rulec gen` writes, as (the name the diagram shows, the directory it
# writes into). --verify holds both halves to the tool: a backend added without touching
# this diagram fails, and so does a name dropped from it. This drawing said "Python と Go"
# for two releases after there were four, which is what the check is for.
LANGS = [("Python", "python"), ("TypeScript", "typescript"), ("JavaScript", "javascript"),
         ("Rust", "rust"), ("Ruby", "ruby"), ("PHP", "php"), ("Go", "go"), ("Swift", "swift"),
         ("Java", "java"), ("SQL", "sql"), ("Wasm", "wasm"), ("NumPy", "numpy")]

JA = dict(
    alt="エージェントが資料を読んで表（.rule）を書き、rulec check にかける。"
        "rulec は診断（どこが・どう直すか・それを起こす入力）を返し、エージェントが直して、"
        "表が通るまで繰り返す。道具では決められないことだけが具体例つきの質問として人に渡り、"
        "人は金額と丸めの向きを答える。表が通ると、証明済みの表から rulec gen が Python・TypeScript・JavaScript・Rust・Ruby・PHP・Go・Swift・Java・SQL・Wasm を出す。",
    agent=("エージェント", ["資料を読む", "表を書く", "診断のとおりに直す", "決められないことは人へ"]),
    rulec=("rulec", ["7 つを証明する：", "完全性・重なり", "当てはまらない行", "単位・丸め", "オーバーフロー・例"]),
    person=("人", ["金額を決める", "丸めの向きを決める", "表を承認する", "コードは書かない"]),
    sources=("元の資料", ["規約の文書", "Excel", "いまのコード"]),
    table=("表", ["1 規則 = 1 表"], ".rule"),
    diagnosis=("診断", ["どこが", "どう直すか", "それを起こす入力"], "JSON"),
    question=("具体例つきの質問", ["「山梨県あての S60 の", "運賃はいくらですか」"]),
    answer=("答え", ["金額", "丸めの向き"]),
    code=("生成コード", ["Python・TypeScript・JavaScript", "Rust・Ruby・PHP・Go", "Swift・Java・SQL・Wasm・NumPy", "依存ゼロ・どれも同じ答え"]),
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
        "Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm from the proved table.",
    agent=("Agent", ["reads the sources", "writes the table", "fixes what rulec finds", "asks a person the rest"]),
    rulec=("rulec", ["proves seven things:", "completeness, overlap,", "dead rows, units,", "rounding, overflow,", "the worked examples"]),
    person=("Person", ["decides amounts", "and which way to round", "approves the table", "never writes code"]),
    sources=("Sources", ["policy documents", "spreadsheets", "legacy code"]),
    table=("Table", ["1 rule = 1 table"], ".rule"),
    diagnosis=("Diagnosis", ["where", "how to fix it", "an input that shows it"], "JSON"),
    question=("A concrete question", ["“What is the fee to 山梨県", "at size S60?”"]),
    answer=("Answer", ["an amount,", "which way to round"]),
    code=("Generated code", ["Python, TypeScript, JavaScript,", "Rust, Ruby, PHP, Go, Swift, Java,", "SQL, Wasm, NumPy — from a proved", "table, no runtime, one answer"]),
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
CODE = (860, Y_IN - 50, 200, 100)    # what comes out, same
W = CODE[0] + CODE[2] + TOP
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


def verify(rulec):
    """The one fact in this picture that the tool can settle: which languages come out."""
    import subprocess
    import tempfile
    here = pathlib.Path(__file__).resolve().parent
    rule = here / "overview.rule"
    with tempfile.TemporaryDirectory() as tmp:
        r = subprocess.run([rulec, "gen", str(rule), "--out", tmp], capture_output=True, text=True)
        if r.returncode:
            raise SystemExit(r.stdout + r.stderr)
        # `gen` also writes the unit vectors, which are not a language.
        wrote = {p.name for p in pathlib.Path(tmp).iterdir() if p.is_dir()} - {"vectors"}
    want = {d for _, d in LANGS}
    if wrote != want:
        raise SystemExit(f"rulec gen writes {sorted(wrote)}, the diagram is drawn for {sorted(want)}")
    for t in (JA, EN):
        said = " ".join([t["code"][0], *t["code"][1], t["alt"]])
        for name, _ in LANGS:
            if name not in said:
                raise SystemExit(f"the diagram does not name {name}")
    print("verified against", rulec)


def main(argv):
    # --verify draws as well, so that a test can tell a stale SVG from a fresh one by
    # whether the bytes on disk moved.
    if argv[:1] == ["--verify"]:
        verify(argv[1] if len(argv) > 1 else "rulec")
    here = pathlib.Path(__file__).resolve().parent.parent / "docs" / "images"
    for name, t in [("flow", EN), ("flow-ja", JA)]:
        for suffix, c in [("", DARK), ("-light", LIGHT)]:
            p = here / f"{name}{suffix}.svg"
            p.write_text(draw(t, c), encoding="utf-8")
            print("wrote", p.relative_to(here.parent.parent))


if __name__ == "__main__":
    main(sys.argv[1:])
