#!/usr/bin/env python3
"""Build the examples page from the corpus, so the sources on the page cannot drift.

Every rule shown here is one of the files the test suite runs: `rulec check` passes on it,
its examples execute, and the reference evaluator, the generated Python and the generated Go
agree on it byte for byte. Run this after editing a corpus rule or the prose below:

    python3 tools/make_examples.py

The output is committed, so building the site needs no Python.
"""
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
CORPUS = ROOT / "tests" / "corpus"

# (file, ja title, ja lede, ja points, en title, en lede, en points)
EXAMPLES = [
    (
        "期間区分.rule",
        "日付で区分を返す",
        "いちばん小さい例です。入力は注文日ひとつ、出力は期間の区分ひとつ。行は隣り合う日付の範囲で、隙間なく敷き詰めてあります。",
        [
            "日付は**比較と範囲だけ**です。足し算も引き算もありません。",
            "`<=2026-03-31` と `>=2026-04-01` が隣り合っていることを検査が知っています。内部で日付を通算日として持っているからで、`20260331` のような数で持つと月末と翌月初のあいだに実在しない隙間ができ、偽のエラーが出ます。",
            "出力が金額ではないので、`round` は要りません。",
        ],
        "A date decides which period it is",
        "The smallest example there is: one input, one output. The rows are adjacent date ranges laid end to end with nothing between them.",
        [
            "A date has **comparison and range, and nothing else** — no addition, no subtraction.",
            "The checker knows `<=2026-03-31` and `>=2026-04-01` are adjacent, because a date is held internally as a day number. Held as `20260331` instead, there would be a phantom gap between the end of one month and the start of the next, and the completeness check would report a hole that is not there.",
            "The output is not a number, so no `round` is required.",
        ],
    ),
    (
        "送料.rule",
        "条件が三つ絡む送料",
        "設計のスケッチをそのまま規則にしたものです。真偽の定義を表の列に置く形、率の出力、`default` の印、`policy first` の隠れが一度に出てきます。",
        [
            "`define … : bool` は**表の列に置ける真偽の名前**です。条件に名前が付くので、表のほうが読みやすくなります。",
            "`default` は「この値に専用の行は要らない」という宣言です。付けないと「どの行にも現れません」と警告されます。",
            "`policy first` なので、上の行が下の行を隠します。それが階段として自然な形かどうかを、検査が三つに分けて数えます。",
        ],
        "A shipping fee with three conditions crossing",
        "The design sketch written out as a rule. A boolean definition used as a column, a rate output, the `default` mark and `policy first` shadowing all appear at once.",
        [
            "`define … : bool` is **a named boolean you can put in a column**. Naming the condition is what makes the table readable.",
            "`default` declares that a value needs no row of its own. Without it you get \"appears in no row\".",
            "Under `policy first` an earlier row hides a later one. The checker sorts those into three kinds and counts them, so the staircase does not drown the one pair that matters.",
        ],
    ),
    (
        "ゆうパック運賃.rule",
        "実在の運賃表を写す",
        "日本郵便の基本運賃表（東京発）です。47 都道府県 × 7 サイズ = 329 通りを、6 つのグループで 42 行に畳んでいます。**この道具の最初の実用価値がここにあります** — 県をひとつ書き落とすと、走らせる前に、その県を名指しして止まります。",
        [
            "`group` は列挙の一部に名前を付けたものです。検査のときは必ずもとの値に展開されるので、**グループの分け方が 47 県の過不足ない分割になっているか**まで検査されます。",
            "`import std/都道府県` で 47 値が入ります（ASCII の別名つき）。",
            "行が 42 本あっても、`policy unique` なので**並べ替えても意味が変わらない**ことが証明されています。",
        ],
        "Transcribing a real published tariff",
        "Japan Post's base tariff, shipping from Tokyo. 47 prefectures × 7 sizes = 329 combinations, folded into 42 rows by six groups. **This is where the tool first pays for itself**: drop one prefecture and it stops before anything runs, naming that prefecture.",
        [
            "A `group` names part of an enum. It is always expanded back to the values for checking, so **whether the grouping is an exact partition of the 47** is checked too.",
            "`import std/都道府県` brings the 47 values in, each with an ASCII alias.",
            "42 rows, and `policy unique` still proves **reordering them cannot change the answer**.",
        ],
    ),
    (
        "クーポン一枚.rule",
        "出力が二つある",
        "クーポン 1 枚について、使えるかどうか（可否）と、いくら引くか（素割引）を同時に返します。重ね掛けの順序と反復は呼び出し側の仕事です。",
        [
            "**出力は二つ以上書けます。** 二つの表が一つずつ埋め、どちらにも `round` が別々に効きます。",
            "出力のセルには**値か名前を一つ**だけ書けます。計算は `define` に出します — 表には分岐だけを残すためです。",
            "一つ前の表の出力（`可否`）を、次の表の列に使っています。上から下への一方通行なので、依存はいつでも目で追えます。",
        ],
        "Two outputs at once",
        "For one coupon: whether it applies, and how much it takes off. Stacking several coupons — the order and the loop — stays with the caller.",
        [
            "**There can be two or more outputs.** Two tables fill one each, and each carries its own `round`.",
            "An output cell holds **one value or one name**. The arithmetic moves to a `define`, so the table keeps the branching and nothing else.",
            "The first table's output (`可否`) is a column of the second. Items run one way, top to bottom, so the dependencies are always readable off the page.",
        ],
    ),
    (
        "クーポン割引.rule",
        "可否と金額を一度に",
        "同じ題材を別の形で書いたものです。導出（`derive`）で残額を出し、それを表の列に置いています。",
        [
            "`derive` は**入力どうしの足し算・引き算だけ**でできた、名前の付いた金額です。数量のまま表の列に置ける唯一の途中の値で、`range` の宣言が要ります。",
            "宣言した範囲が、実際に起こりうる値を含んでいないと E112 で止まります。範囲は検査が使う「全体集合」だからです。",
        ],
        "Eligibility and amount together",
        "The same subject in another shape: a `derive` computes what is left, and that becomes a column.",
        [
            "A `derive` is a named amount built from **additions and subtractions of inputs only**. It is the one intermediate value that can sit in a column as a quantity, and it must declare a `range`.",
            "If the declared range does not contain the values that can actually occur, E112 stops it — the range is the universe the checks reason over.",
        ],
    ),
    (
        "クーポン併用.rule",
        "検査が決められなかったとき",
        "二つの導出が同じ入力を共有しているため、二つの行が同時に当てはまりうるかを検査が決められません。黙って通すことも、嘘のエラーを出すこともしません。**警告を出し、生成コードに実行時のガードを入れます。**",
        [
            "W114 は「証明できなかった」という警告です。重なりが**ある**とも**ない**とも言っていません。",
            "生成コードには、万一その条件に当てはまる入力が来たとき、黙って先の行を選ばずにエラーを返すガードが入ります。**このガードが動いたなら、重なりは本当にあったということです。**",
            "静的に決められないことを実行時に持ち越す唯一の場所です。",
        ],
        "When the checker could not decide",
        "Two derived values share an input, so whether two rows can fire together is not decidable here. The tool neither waves it through nor invents an error: **it warns, and puts a runtime guard in the generated code.**",
        [
            "W114 says \"not proved\". It does not say the overlap exists, and it does not say it doesn't.",
            "The generated code refuses to silently pick the earlier row if such an input ever arrives; it raises instead. **If that guard ever fires, the overlap was real.**",
            "This is the only place where something undecided statically is carried into runtime.",
        ],
    ),
    (
        "適用順序.rule",
        "答えが順序",
        "クーポン二枚のどちらを先に適用するかを返します。並べ替えそのものは呼び出し側がしますが、**比較の基準は規則の中にあります** — そこが承認の対象だからです。",
        [
            "返せる答えは金額・可否・区分・順序の四種で、これはその四つめです。",
            "入力は「二枚ぶんの属性」で、出力は `先` か `後`。一度の判定に収まる形にしてあります。",
            "日付どうしの比較が出てきます。",
        ],
        "The answer is an order",
        "Which of two coupons applies first. The sorting itself is the caller's loop, but **the comparison lives in the rule**, because that is the part someone has to approve.",
        [
            "Of the four kinds of answer — amount, yes/no, class, order — this is the fourth.",
            "The inputs are the attributes of both coupons and the output is \"first\" or \"second\", which keeps it to one decision.",
            "Two dates get compared here.",
        ],
    ),
    (
        "値引の充当.rule",
        "按分を、一件ずつに割る",
        "一括の値引きを明細に配り切ります。**1 明細 = 1 回の判定**にしてあり、反復と残額の持ち回りは呼び出し側です。呼ぶ側は明細を順に回して残りを引くだけで、**合計は構成上ぴったり合います**（2,000 通りの明細で確かめました）。",
        [
            "「配り方」を決めるのが規則、「配って回る」のが呼び出し側、という切り分けです。",
            "`min` が使えます。上限で頭打ちにする形は、これで一行に収まります。",
            "定価の比で割り付ける按分（`明細 ÷ 合計`）は**書けません**。割り算は定数でしか書けないからです。",
        ],
        "Apportioning, one line at a time",
        "Spreading one discount across the lines of an order. **One line is one decision**; the loop and the running remainder stay with the caller, who walks the lines subtracting as it goes. **The total comes out exact by construction** (checked over 2,000 generated sets of lines).",
        [
            "The rule decides how to allocate; the caller does the walking.",
            "`min` is available, so \"up to this line's own value\" fits on one line.",
            "Splitting by ratio (`line ÷ total`) **cannot be written**: division is only allowed by a constant.",
        ],
    ),
    (
        "決済手数料.rule",
        "1% より細かい率",
        "契約ごとに決まる手数料率から、取引一件の手数料を出します。率の刻みが 0.1% なので、`0.5%` のような値を書けます。",
        [
            "`rate[step 0.1%]` は「実行時の値は 0.1% の整数倍」という宣言です。0.5% は 5 刻み、10% は 100 刻みとして運ばれます。",
            "**刻みの間の値は書けません。** `rate[step 0.1%]` の列に `0.05%` と書くと E114 で止まります。黙って近い刻みに寄せると、表で読める境界と生成コードの境界が食い違うからです。",
            "率のまま計算して、最後に一度だけ丸めます。途中で丸めないのは、丸め方が業務の判断だからです。",
        ],
        "A rate finer than one percent",
        "A per-contract fee rate turned into the fee on one transaction. The rate moves in tenths of a percent, so `0.5%` is a value you can write.",
        [
            "`rate[step 0.1%]` declares that at runtime the value is a whole number of tenths of a percent: 0.5% travels as 5, 10% as 100.",
            "**A value between two steps cannot be written.** `0.05%` in that column stops at E114 — quietly moving it to the nearest step would put a different boundary in the code than the one on the page.",
            "The arithmetic stays a rate until the end, where it is rounded exactly once, because where to round is a business decision.",
        ],
    ),
    (
        "評価ランク.rule",
        "点数を合算して、達成率でランクを付ける",
        "お金がまったく出てこない例です。四つの評価項目を重み付きで足し、満点に対する達成率でランクを決めます。「表で書かれてはいないが、書き出せば表と少しの式で足りる」ルールの典型です。",
        [
            "`derive` は**入力どうしの足し算・引き算と整数倍**でできた名前です。重み付きの合計はこれで書けます。",
            "`define 達成率 : rate = 合計点 ÷ 50` と宣言すると、表のセルに `>=90%` と**割合のまま**書けます。無次元の値を率と呼ぶか数と呼ぶかは、宣言する側が決めます。",
            "**実行時に割り算は起きません。** `>=90%` は生成コードでは `合計点 >= 45` になります。満点が定数なので、境界が定数に畳めるからです。逆に**満点が入力だと書けません** — 変数で割ることになり、E115 で止まります（割合そのものを `rate` の入力で受け取ってください）。",
        ],
        "Scores added up, then ranked by how much of the total they reach",
        "No money anywhere in this one. Four scored criteria are added with weights, and the rank comes from how much of the maximum the total reaches. This is the shape of rule that is *not* written as a table today but becomes one the moment somebody writes it down.",
        [
            "A `derive` is a name for **additions, subtractions and integer multiples of the inputs**, which is exactly what a weighted total is.",
            "Declaring `define 達成率 : rate = 合計点 ÷ 50` lets the cells say `>=90%` — **the threshold stays a proportion on the page**. Whether a dimensionless value is called a rate or a number is the declaration's to say.",
            "**No division happens at runtime.** `>=90%` compiles to `合計点 >= 45`, because a constant maximum folds the boundary into a constant. A maximum that is an *input* cannot be written: that is division by a variable, and E115 stops it — take the proportion itself as a `rate` input instead.",
        ],
    ),
    (
        "ポイント付与.rule",
        "単位のない数を返す",
        "100 円につき 1 点。金額を金額で割ると単位が消えて、`number`（単位のない整数）になります。",
        [
            "`number` は件数・日数・点数のような、**単位を持たない整数**のための型です。",
            "`税込金額 ÷ 100円` の割り算は**整数を割りません**。内部の刻みを 100 倍するだけなので、1050 円は 10.5 点のまま最後まで運ばれ、`round down(1)` が一度だけ 10 点に落とします。Python の `//` や Go の `/` の切り捨ての向きの違いが入り込む余地がありません。",
            "割り算は**定数でだけ**書けます。割る数が業務データなら、それは率か定数表として表に現れるはずです。",
        ],
        "Returning a number with no unit",
        "One point per 100 yen. Dividing money by money cancels the unit and leaves a `number` — a whole number carrying none.",
        [
            "`number` is the type for **counts, days and scores**: whole numbers with no unit.",
            "`税込金額 ÷ 100円` **does not divide the integer**. It multiplies the internal step by 100, so 1050 yen stays 10.5 points all the way to the end, where `round down(1)` makes it 10 exactly once. Python's `//` and Go's `/` truncate in different directions; neither gets a say here.",
            "Division is allowed **by a constant only**. If the divisor is business data, it belongs in the table as a rate or a constant column.",
        ],
    ),
]

JA_HEAD = """# 例で見る

ここにあるのは全部、**このリポジトリのテストが毎回走らせている規則**です。`rulec check` を通り、書いてある例が実行され、参照評価器・生成した Python・生成した Go の三つが同じ答えを返すことまで確かめられています。そのままコピーして動かせます。

小さいものから順に並べてあります。

"""

EN_HEAD = """# Examples

Every rule on this page is **one the repository's tests run on every commit**: it passes
`rulec check`, its own examples execute, and the reference evaluator, the generated Python
and the generated Go are held to the same answers byte for byte. Copy any of them and it
works.

They are ordered smallest first.

"""

JA_TAIL = """---

[表を書く](tour.md){ .md-button .md-button--primary }
[文法](reference.md){ .md-button }
"""

EN_TAIL = """---

[Write a table](tour.md){ .md-button .md-button--primary }
[Grammar](reference.md){ .md-button }
"""


def page(head, tail, title_i, lede_i, points_i, shows):
    out = [head]
    for e in EXAMPLES:
        src = (CORPUS / e[0]).read_text(encoding="utf-8").rstrip("\n")
        out.append(f"## {e[title_i]}\n\n{e[lede_i]}\n\n```\n{src}\n```\n\n**{shows}**\n\n")
        out.append("".join(f"- {p}\n" for p in e[points_i]))
        out.append("\n")
    out.append(tail)
    return "".join(out)


def main():
    (ROOT / "website" / "docs-ja" / "examples.md").write_text(
        page(JA_HEAD, JA_TAIL, 1, 2, 3, "この例が見せていること"), encoding="utf-8"
    )
    (ROOT / "website" / "docs" / "examples.md").write_text(
        page(EN_HEAD, EN_TAIL, 4, 5, 6, "What this one shows"), encoding="utf-8"
    )
    print(f"wrote examples.md (en + ja): {len(EXAMPLES)} rules")


if __name__ == "__main__":
    main()
