"""Draws the overview on the home page: a .flow and the rules it calls, what the checker looks
at, the reference interpreter with its scenarios, and the five platforms, each held to it.

Four pictures come out, for English and Japanese on the dark and the light scheme:
docs/images/overview{,-light,-ja,-ja-light}.svg. They are committed, so building the site needs
nothing but Zensical; run this again after changing a label.

    $ .venv/bin/python tools/make_overview.py
"""

from html import escape
from pathlib import Path

W, H = 1200, 470

SCHEMES = {
    "dark": {"text": "#e6ecee", "muted": "#9fb0b6", "box": "#141b1f", "line": "#2a363c",
             "accent": "#36c4aa", "code": "#cfd8dc", "arrow": "#5f7078"},
    "light": {"text": "#10161a", "muted": "#56646a", "box": "#ffffff", "line": "#d3dfdc",
              "accent": "#0d8a77", "code": "#22313a", "arrow": "#8a9aa0"},
}

CODE = [
    "task confirm_intent(intent: string)",
    '  http POST stripe "…/confirm"',
    "  sends payment_intent.confirm",
    "flow",
    "  let quote = hold(nights: …)",
    "  match quote.handling",
    "    auto => pi <- create_intent(…)",
]

TEXT = {
    "en": {
        "flow": ".flow",
        "rules": "rules · rulec",
        "rule_files": ["hold_amount.rule", "payment_intent.rule"],
        "rules_note": "decisions and state machines rulec proves",
        "check": "check",
        "checks": [
            "types, and every arm of every match",
            "every state a case can be left in",
            "retries that could repeat a change",
            "the history's size on each platform",
            "ranges, child .flows, API descriptions",
            "what each platform can and cannot do",
        ],
        "ref": "reference interpreter · scenarios",
        "ref_note": ["one meaning, and scenarios that take", "every arm and every error"],
        "build": "build",
        "main": "main",
        "platforms": [
            ("Temporal", "workflow, worker and client, in TypeScript or Python"),
            ("AWS Step Functions", "a state machine in ASL with JSONata"),
            ("Lambda durable functions", "a handler in TypeScript"),
            ("Argo Workflows", "a WorkflowTemplate and its caller image"),
            ("pydantic-graph", "a graph that runs in your own process"),
        ],
        "held": "each run on every scenario, and held to the reference",
    },
    "ja": {
        "flow": ".flow",
        "rules": "規則 · rulec",
        "rule_files": ["hold_amount.rule", "payment_intent.rule"],
        "rules_note": "抜けと重なりが無いことを rulec が証明",
        "check": "検査",
        "checks": [
            "型と、match のすべての分岐",
            "案件が最後に残りうるすべての状態",
            "外部のデータを変える呼び出しのリトライ",
            "プラットフォームごとの実行履歴の大きさ",
            "範囲、子の .flow、API の記述",
            "プラットフォームにできること、できないこと",
        ],
        "ref": "参照インタプリタ · シナリオ",
        "ref_note": ["意味を一つに決める。どの分岐も", "どのエラーも通るシナリオを作る"],
        "build": "ビルド",
        "main": "主",
        "platforms": [
            ("Temporal", "ワークフロー一式（TypeScript か Python）"),
            ("AWS Step Functions", "JSONata で書いた ASL のステートマシン"),
            ("Lambda durable functions", "TypeScript のハンドラー"),
            ("Argo Workflows", "WorkflowTemplate と、呼び出し用のイメージ"),
            ("pydantic-graph", "Python のプロセスの中で動くグラフ"),
        ],
        "held": "どれもシナリオごとに走らせ、参照と突き合わせる",
    },
}

SANS = "Inter, 'Hiragino Sans', 'Noto Sans JP', 'Segoe UI', system-ui, sans-serif"
MONO = "'M PLUS 1 Code', Menlo, Consolas, monospace"


def text(x, y, s, size, fill, weight=400, family=SANS, anchor="start"):
    return (f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" font-weight="{weight}" '
            f'fill="{fill}" text-anchor="{anchor}" xml:space="preserve">{escape(s)}</text>')


def box(x, y, w, h, c, stroke=None):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" fill="{c["box"]}" stroke="{stroke or c["line"]}" stroke-width="1.5"/>'


def arrow(x1, y1, x2, y2, c, dashed=False):
    dash = ' stroke-dasharray="6 6"' if dashed else ""
    return (f'<path d="M{x1} {y1} C{(x1 + x2) / 2} {y1}, {(x1 + x2) / 2} {y2}, {x2} {y2}" fill="none" '
            f'stroke="{c["arrow"]}" stroke-width="2"{dash} marker-end="url(#head)"/>')


def draw(lang, scheme):
    c = SCHEMES[scheme]
    t = TEXT[lang]
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img">',
           f'<defs><marker id="head" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">'
           f'<path d="M0 0L10 5L0 10z" fill="{c["arrow"]}"/></marker></defs>']

    # left: the .flow and the rules
    out.append(box(20, 30, 330, 250, c))
    out.append(text(40, 62, t["flow"], 18, c["accent"], 700, MONO))
    for i, line in enumerate(CODE):
        out.append(text(40, 96 + i * 25, line, 13, c["code"], 400, MONO))
    out.append(box(20, 300, 330, 140, c))
    out.append(text(40, 332, t["rules"], 18, c["accent"], 700))
    for i, f in enumerate(t["rule_files"]):
        out.append(text(40, 362 + i * 24, f, 13, c["code"], 400, MONO))
    out.append(text(40, 422, t["rules_note"], 13, c["muted"]))

    # middle: the checks, and the reference interpreter
    out.append(box(405, 30, 360, 250, c, c["accent"]))
    out.append(text(425, 62, t["check"], 18, c["accent"], 700))
    for i, s in enumerate(t["checks"]):
        out.append(f'<circle cx="431" cy="{93 + i * 31}" r="3" fill="{c["accent"]}"/>')
        out.append(text(443, 98 + i * 31, s, 14, c["text"]))
    out.append(box(405, 300, 360, 140, c))
    out.append(text(425, 332, t["ref"], 17, c["accent"], 700))
    for i, s in enumerate(t["ref_note"]):
        out.append(text(425, 364 + i * 24, s, 14, c["text"]))
    out.append(arrow(350, 155, 405, 155, c))
    out.append(arrow(350, 370, 405, 200, c))
    out.append(arrow(585, 280, 585, 300, c))

    # right: the five platforms
    out.append(text(815, 22, t["build"], 14, c["muted"], 600))
    for i, (name, note) in enumerate(t["platforms"]):
        y = 30 + i * 82
        out.append(box(815, y, 365, 70, c, c["accent"] if i == 0 else None))
        out.append(text(833, y + 29, name, 16, c["text"], 700))
        if i == 0:
            out.append(f'<rect x="{1180 - 62}" y="{y + 12}" width="46" height="22" rx="11" fill="{c["accent"]}"/>')
            out.append(text(1180 - 39, y + 28, t["main"], 12, c["box"], 700, anchor="middle"))
        out.append(text(833, y + 53, note, 13, c["muted"]))
    for y in (65, 147, 229, 311, 393):
        out.append(arrow(765, 155, 815, y, c))
    out.append(arrow(765, 400, 805, 440, c, dashed=True))
    out.append(text(997, 462, t["held"], 13, c["muted"], anchor="middle"))
    out.append("</svg>")
    return "\n".join(out) + "\n"


def main():
    images = Path(__file__).resolve().parent.parent / "docs" / "images"
    for lang in ("en", "ja"):
        for scheme in ("dark", "light"):
            name = "overview" + ("-ja" if lang == "ja" else "") + ("-light" if scheme == "light" else "") + ".svg"
            (images / name).write_text(draw(lang, scheme), encoding="utf-8")
            print(images / name)


if __name__ == "__main__":
    main()
