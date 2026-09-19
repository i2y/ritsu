#!/usr/bin/env python3
"""The three pictures of the scenarios page: one per reader, drawn with the front page's
vocabulary (cards for actors, sheets for artifacts, three colours for three kinds of path).

    python3 tools/make_scenarios.py

Each picture is one left-to-right sentence — what goes in, who does what, what comes out —
with at most one loop under it (the thing that comes back) and one detour to a person.
Twelve files come from three layouts: two languages times two colour schemes each, so the
geometry cannot drift between them; only the words change. The SVGs are committed because
building the site must not need Python.
"""

import pathlib
import sys

from diagram import DARK, LIGHT, FONT, fit, esc, text, card, sheet, arrow, marker

TOP = 20
Y_IN, Y_BACK = 70, 172
CARD_H = 196
SHEET_H = 100
GAP = 96                              # room for a command label between two shapes


# --- words ------------------------------------------------------------------
#
# A figure is: alt text; nodes left to right as (kind, spec, width) where kind is "sheet",
# "actor" (indigo card) or "person" (amber card); labels for the gaps as (mono, plain);
# an optional loop (from node, to node, sheet spec, caption) drawn under the row; and an
# optional detour (node, person card spec, label) drawn below that node.

FIGS = {}

FIGS["existing"] = dict(
    ja=dict(
        alt="法令や公開された規約を、エージェントが表（.rule）に写し、条文を @ で引用する。rulec が写しと照合し、抜けと重なりを証明して、十一言語に生成する。承認する人は rulec doc の資料で条文と表を見比べる。改正が出れば rulec source outdated が知らせ、表を読み直す。",
        nodes=[
            ("sheet", ("法令・規約", ["e-Gov の条文", "規約の PDF", "料金表"]), 118),
            ("actor", ("エージェント", ["条文を読む", "表に写す", "出典を @ で引く"]), 150),
            ("sheet", ("表", ["@法 第91条", "写しに固定"], ".rule"), 120),
            ("actor", ("rulec", ["写しと照合する", "抜けと重なり", "を証明する", "十一言語に生成"]), 150),
            ("sheet", ("生成コード", ["十一言語、依存ゼロ", "ヘッダに出典と", "時点が刻まれる"]), 150),
        ],
        labels=[("", "読む"), ("", "写す"), ("rulec check", "fetch と pin も"), ("rulec gen", "通ったら")],
        loop=(3, 1, ("改正", ["いつから何が", "変わるか"]), "rulec source outdated"),
        detour=(3, ("承認する人", ["条文と表を", "見比べる", "承認する"]), "rulec doc"),
    ),
    en=dict(
        alt="An agent transcribes a statute or a published policy into a table (.rule), citing the article with @. rulec holds the table to the copy, proves it has no gap and no overlap, and generates eleven languages. The approver compares the article and the table on the page rulec doc renders. When an amendment comes, rulec source outdated says so and the table is reread.",
        nodes=[
            ("sheet", ("Statute, policy", ["an article on e-Gov", "a policy PDF", "a tariff sheet"]), 130),
            ("actor", ("Agent", ["reads the article", "writes the table", "cites it with @"]), 150),
            ("sheet", ("Table", ["@法 第91条", "pinned to a copy"], ".rule"), 120),
            ("actor", ("rulec", ["holds it to the copy", "proves no gap,", "no overlap", "emits eleven languages"]), 160),
            ("sheet", ("Generated code", ["eleven languages, no runtime", "header names the source", "and its date"]), 176),
        ],
        labels=[("", "reads"), ("", "writes"), ("rulec check", "fetch, pin too"), ("rulec gen", "once it passes")],
        loop=(3, 1, ("Amendment", ["what changes,", "from when"]), "rulec source outdated"),
        detour=(3, ("Approver", ["compares the article", "with the table", "and signs off"]), "rulec doc"),
    ),
)

FIGS["internal"] = dict(
    ja=dict(
        alt="社内の規程や自社サービスの規約、Excel、動いているコードといった手元のものを、エージェントが表（.rule）に写す。Excel からは rulec import で下書きを起こし、文書は @ で引用してファイルごとハッシュを固定する。rulec が抜けと重なりを証明し、旧実装や過去の記録と突き合わせて、食い違いをどの行で何件いくらかで返す。承認する人は rulec doc の資料で文書と表を見比べる。通った表から十一言語に生成する。",
        nodes=[
            ("sheet", ("手元のもの", ["社内の規程、規約", "Excel、料金表", "動いているコード"]), 130),
            ("actor", ("エージェント", ["読んで表に写す", "Excel は import で", "出典を @ で引く"]), 150),
            ("sheet", ("表", ["@規約 別紙1", "ファイルに固定"], ".rule"), 120),
            ("actor", ("rulec", ["抜けと重なり", "を証明する", "旧実装や記録と", "突き合わせる"]), 150),
            ("sheet", ("生成コード", ["十一言語、依存ゼロ", "旧実装と同じ答え"]), 150),
        ],
        labels=[("", "読む"), ("", "写す"), ("rulec check", "pin も"), ("rulec gen", "通ったら")],
        loop=(3, 1, ("食い違い", ["どの行で何件", "いくら違うか"]), "rulec verify / replay"),
        detour=(3, ("承認する人", ["文書と表を", "見比べる", "承認する"]), "rulec doc"),
    ),
    en=dict(
        alt="An agent transcribes what is at hand - an internal policy, the terms of your own service, a spreadsheet, code that runs today - into a table (.rule): a spreadsheet becomes a draft through rulec import, a document is cited with @ and pinned whole by its digest. rulec proves no gap and no overlap, holds the table to the legacy implementation and to past records, and returns every mismatch by row, count and amount. The approver compares the document and the table on the page rulec doc renders. From a passed table come eleven languages.",
        nodes=[
            ("sheet", ("What you have", ["an internal policy", "a spreadsheet, a tariff", "code that runs today"]), 140),
            ("actor", ("Agent", ["reads and transcribes", "imports the spreadsheet", "cites the file with @"]), 160),
            ("sheet", ("Table", ["@規約 別紙1", "pinned to the file"], ".rule"), 130),
            ("actor", ("rulec", ["proves no gap,", "no overlap", "holds it to the code", "and to the records"]), 160),
            ("sheet", ("Generated code", ["eleven languages, no runtime", "the same answers as before"]), 176),
        ],
        labels=[("", "reads"), ("", "writes"), ("rulec check", "pin too"), ("rulec gen", "once it passes")],
        loop=(3, 1, ("Mismatches", ["which rows, how many,", "by how much"]), "rulec verify / replay"),
        detour=(3, ("Approver", ["compares the document", "with the table", "and signs off"]), "rulec doc"),
    ),
)

FIGS["designing"] = dict(
    ja=dict(
        alt="決めたいこと（送料、クーポン、返品の可否、社内の判定基準）を、検討する人が表（.rule）に書く。rulec check が抜けと重なりを、それを起こす入力つきで返し、通るまで直す。通った表からは承認者向けの資料、お客向けの案内、改定の影響が出る。",
        nodes=[
            ("sheet", ("決めたいこと", ["送料、クーポン", "返品の可否", "社内の判定基準"]), 130),
            ("person", ("検討する人", ["条件を表に書く", "Excel からでも", "金額は自分で決める"]), 150),
            ("sheet", ("表", ["1 規則 = 1 表", "例も書く"], ".rule"), 120),
            ("actor", ("rulec", ["抜けと重なりを", "それを起こす", "入力つきで返す"]), 150),
            ("sheet", ("資料", ["承認する人向け", "お客向けの案内", "改定の影響"]), 150),
        ],
        labels=[("", "整理する"), ("", "書く"), ("rulec check", "通るまで"), ("rulec doc", "rulec diff")],
        loop=(3, 1, ("診断", ["どこが", "それを起こす入力", "足す行の形"]), "通るまで繰り返す"),
        detour=None,
    ),
    en=dict(
        alt="Someone designing a rule (shipping, coupons, returns, an internal criterion) writes it as a table (.rule). rulec check returns every gap and overlap with an input that shows it, until the table passes. From a passed table come the approver's page, the customer article and the impact of a revision.",
        nodes=[
            ("sheet", ("What to decide", ["shipping, coupons", "returns", "an internal criterion"]), 140),
            ("person", ("Designer", ["writes the conditions", "as a table, or from", "Excel; decides amounts"]), 160),
            ("sheet", ("Table", ["1 rule = 1 table", "with worked examples"], ".rule"), 140),
            ("actor", ("rulec", ["returns every gap", "and overlap with an", "input that shows it"]), 150),
            ("sheet", ("Pages", ["the approver's page", "the customer article", "the impact of a change"]), 160),
        ],
        labels=[("", "sorts out"), ("", "writes"), ("rulec check", "until it passes"), ("rulec doc", "rulec diff")],
        loop=(3, 1, ("Diagnosis", ["where", "an input that shows it", "the row to add"]), "until it passes"),
        detour=None,
    ),
)

FIGS["implementing"] = dict(
    ja=dict(
        alt="検査を通った表（.rule）から rulec gen が十一言語のコードを出し、rulec test が参照評価器と突き合わせる。実装する人は rulec api で呼び方を読み、関数・SQL・Wasm・MCP サーバのどれかを自分のアプリやバッチやエージェントに組み込む。生成物は編集せず、表が変われば CI の gen --check が止める。",
        nodes=[
            ("sheet", ("表", ["検査を通った規則"], ".rule"), 118),
            ("actor", ("rulec", ["十一言語に生成", "参照評価器と", "突き合わせる", "呼び方の一覧"]), 150),
            ("sheet", ("生成コード", ["関数（七言語）", "SQL の問い合わせ", "Wasm のモジュール", "MCP サーバ"]), 150),
            ("person", ("実装する人", ["呼び出し側を書く", "生成物は編集しない", "CI に置く"]), 160),
            ("sheet", ("動くもの", ["アプリ、バッチ", "エージェントのツール", "ブラウザ"]), 150),
        ],
        labels=[("rulec gen", ""), ("rulec test", "どれも同じ答え"), ("rulec api", "呼び方を読む"), ("", "組み込む")],
        loop=None,
        detour=None,
    ),
    en=dict(
        alt="From a table that passed check, rulec gen writes code in eleven languages and rulec test holds each to the reference evaluator. The implementer reads how to call it from rulec api and puts the function, the SQL query, the Wasm module or the MCP server into an app, a batch or an agent. Generated code is never edited; when the table changes, gen --check in CI stops the build.",
        nodes=[
            ("sheet", ("Table", ["a rule that passed check"], ".rule"), 150),
            ("actor", ("rulec", ["emits eleven languages", "holds each to the", "reference evaluator", "lists how to call it"]), 160),
            ("sheet", ("Generated code", ["functions (seven languages)", "one SQL query", "one Wasm module", "an MCP server"]), 176),
            ("person", ("Implementer", ["writes the caller", "never edits the output", "puts it in CI"]), 150),
            ("sheet", ("What runs", ["an app, a batch", "an agent's tool", "a browser"]), 130),
        ],
        labels=[("rulec gen", ""), ("rulec test", "one answer, nine"), ("rulec api", "how to call it"), ("", "wires it in")],
        loop=None,
        detour=None,
    ),
)


# --- layout -----------------------------------------------------------------

def layout(fig):
    """x-positions left to right; the row is centred on Y_IN, cards hang from TOP."""
    rects = []
    x = TOP
    for kind, spec, w in fig["nodes"]:
        if kind == "sheet":
            rects.append((x, Y_IN - SHEET_H / 2, w, SHEET_H))
        else:
            rects.append((x, TOP, w, CARD_H))
        x += w + GAP
    width = x - GAP + TOP
    return rects, width


def right(a, b):
    return a[0] + a[2], b[0]


def mid(a, b):
    x1, x2 = right(a, b)
    return (x1 + x2) / 2


def draw(fig, c):
    rects, W = layout(fig)
    H = TOP + CARD_H + TOP
    if fig["detour"]:
        H = TOP + CARD_H + 74 + 148 + TOP
    o = [None, "<defs>", marker("neutral", c), marker("accent", c), marker("human", c), "</defs>"]

    # Shapes first, then arrows, then the words beside arrows.
    for i, (kind, spec, _) in enumerate(fig["nodes"]):
        if kind == "sheet":
            o += sheet(rects[i], spec, c)
        else:
            o += card(rects[i], spec, c["accent" if kind == "actor" else "human"], c, f"clip-{i}")

    kinds = [k for k, _, _ in fig["nodes"]]
    for i, (mono, plain) in enumerate(fig["labels"]):
        a, b = rects[i], rects[i + 1]
        x1, x2 = right(a, b)
        # The pipeline is grey where it enters and leaves, indigo between the actors.
        path = "accent" if "actor" in (kinds[i], kinds[i + 1]) else "neutral"
        o.append(arrow(x1, Y_IN, x2, Y_IN, path, c))
        avail = x2 - x1 - 8
        if mono:
            fit(mono, 10.5, avail, mono=True)
            o.append(text(mid(a, b), Y_IN - 9, mono, 10.5, c["dim"], mono=True))
        if plain:
            fit(plain, 10.5, avail)
            o.append(text(mid(a, b), Y_IN + 18 if mono else Y_IN - 9, plain, 10.5, c["dim"]))

    if fig["loop"]:
        # The loop runs right to left under the row, between two cards: what comes back from
        # the card on the right to the card on the left, with the sheet that carries it in
        # the middle and its caption underneath.
        src, dst, spec, caption = fig["loop"]
        a, b = rects[dst], rects[src]
        x1, x2 = right(a, b)
        w = min(176, x2 - x1 - 48)
        box = ((x1 + x2) / 2 - w / 2, Y_BACK - 40, w, 80)
        o += sheet(box, spec, c)
        o.append(arrow(x2, Y_BACK, box[0] + box[2], Y_BACK, "accent", c))
        o.append(arrow(box[0], Y_BACK, x1, Y_BACK, "accent", c))
        mono = caption.startswith("rulec ")
        fit(caption, 11, x2 - x1, mono=mono)
        o.append(text((x1 + x2) / 2, Y_BACK + 40 + 15, caption, 11, c["accent"], mono=mono))

    if fig["detour"]:
        at, spec, label = fig["detour"]
        r = rects[at]
        cx = r[0] + r[2] / 2
        bottom = TOP + CARD_H
        box = (cx - 85, bottom + 74, 170, 148)
        o += card(box, spec, c["human"], c, "clip-person")
        o.append(arrow(cx, bottom, cx, box[1], "human", c))
        fit(label, 10.5, 200, mono=True)
        o.append(text(cx + 8, (bottom + box[1]) / 2 + 4, label, 10.5, c["human"], anchor="start", mono=True))

    o[0] = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" '
            f'height="{H}" role="img" aria-label="{esc(fig["alt"])}" font-family=\'{FONT}\'>')
    o.append("</svg>")
    return "\n".join(o) + "\n"


def main(argv):
    here = pathlib.Path(__file__).resolve().parent.parent / "docs" / "images"
    for name, fig in FIGS.items():
        for lang, suffix_l in [("en", ""), ("ja", "-ja")]:
            for suffix, c in [("", DARK), ("-light", LIGHT)]:
                p = here / f"scenario-{name}{suffix_l}{suffix}.svg"
                p.write_text(draw(fig[lang], c), encoding="utf-8")
                print("wrote", p.relative_to(here.parent.parent))


if __name__ == "__main__":
    main(sys.argv[1:])
