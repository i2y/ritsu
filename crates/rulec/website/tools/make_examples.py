#!/usr/bin/env python3
"""Build the examples page from the corpus, so the sources on the page cannot drift.

Every rule shown here is one of the files the test suite runs: `rulec check` passes on it,
its examples execute, and the reference evaluator and every generated language agree on it
byte for byte. Run this after editing a corpus rule or the prose below:

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
    (
        "会員特典.rule",
        "表を三段重ねて、出力を二つ返す",
        "重量から重さ区分、重さ区分と会員区分から帯、帯と支払額から送料と倍率。前の表が出した値が、そのまま次の表の列になります。返るのは請求額とポイントの二つです。",
        [
            "**前の表の出力は、後の表の列にそのまま書けます。** 段数に上限はありません（検査の予算を超えたら E109 で止まります）。`rulec check` が落ちたときの「発火した行」も、`表 重さ判定 行2 / 表 帯判定 行4 / 表 送料表 行4` と段の数だけ出ます。",
            "**一つの表が出力列を複数持てます。** `送料表` は `送料` と `倍率` を同時に出し、下の `define` がその率を使います。率の刻み（`step 10%`）は列に入っても保たれるので、`基本点 × 倍率` は最後に一度だけ丸められます。",
            "**`derive` が列になります。** `支払額 = 商品合計 − 値引` を宣言してあるので、「値引き後の金額で判定する」が一本の式ではなく `送料表` の一つの列になります。",
            "**`result` が組み立てるのは最初の出力だけです**（E015）。二つ目からは、その出力と同じ名前の `define` から取ります — ここでは `define 付与点`。`result` を二本書くと E016 で止まります。",
        ],
        "Three tables stacked, two outputs returned",
        "Weight gives a weight class, the class and the membership give a tier, and the tier with the amount payable gives shipping and a multiplier. What one table produces is a column of the next. Two things come back: the amount charged and the points.",
        [
            "**A table's output column is a column of any later table.** There is no limit on the depth (past the check's budget it stops at E109). When `rulec check` fails it names the row that fired in each of them: `table 重さ判定 row 2 / table 帯判定 row 4 / table 送料表 row 4`.",
            "**One table may produce several output columns.** `送料表` produces `送料` and `倍率` at once, and the `define` below it uses that rate. A rate keeps its step (`step 10%`) through the column, so `基本点 × 倍率` is rounded exactly once, at the end.",
            "**A `derive` can be a column.** Declaring `支払額 = 商品合計 - 値引` turns judging on the amount after the discount into one column of `送料表` rather than one bare line of arithmetic.",
            "**`result` assembles the first output and nothing else** (E015). The second and later ones are taken from a `define` of the same name - `define 付与点` here. A second `result` line stops at E016.",
        ],
    ),
    (
        "ec261.rule",
        "英語で書いた規則 — EU 旅客権利規則",
        "名前もセルも英語なので、ASCII 別名が一つも出てきません。金額は EUR、距離は km です。公開されている法令（(EC) No 261/2004 第 7 条）の転記で、条文そのものが決定表の形をしています。",
        [
            "**ASCII の名前には別名が要りません。** 漢字は Go の公開識別子になれないので `運賃(fee)` のような別名が要りますが、`distance` にはその必要がありません。英語で書けば、カッコはどこにも出てきません。",
            "**通貨どうしは換算されません。** `money[EUR]` の列に `100円` を書くと E103 で止まります。為替レートはこの道具の中に無く、あってはいけないものだからです（[単位の一覧](reference.md)）。",
            "**条文が決めていないことを、表が決めさせます。** 第 7 条 1 項の (b) は「between 1500 and 3500 kilometres」で、両端を含むかが読めません。(a) が「1500km 以下」なので下端は開く、とここで決めています。決めなければ抜けか重なりで止まるので、**あいまいなまま先へは進めません**。",
            "**同じ条件を二度書くほうが正しいこともあります。** 5 割引きの閾値（2 / 3 / 4 時間）は帯で引けば二列で済みますが、条文の 2 項は距離の条件を丸ごと書き直しています。ここでも距離で引いたのは、そのほうが原文と行が一対一で並ぶからです。",
        ],
        "A rule written in English — EU air passenger rights",
        "Names and cells are English, so not one ASCII alias appears. The money is EUR and the distance is km. It is a transcription of published law — Article 7 of Regulation (EC) No 261/2004 — whose text is already shaped like a decision table.",
        [
            "**An ASCII name needs no alias.** A kanji cannot begin an exported Go identifier, which is why `運賃(fee)` carries one; `distance` does not. Write the rule in English and there are no parentheses anywhere.",
            "**Two currencies never convert.** `100円` in a `money[EUR]` column stops at E103. There is no exchange rate in this tool and there must not be one ([the units](reference.md)).",
            "**What the text leaves open, the table makes you decide.** Article 7(1)(b) says \"between 1500 and 3500 kilometres\" and does not say whether either end is included. Since (a) is \"1500 kilometres or less\", the lower end is open here — and that is a decision, made in the open. Leave it undecided and the checker stops with a gap or an overlap.",
            "**Sometimes writing the same condition twice is the faithful thing.** The 50% reduction thresholds could be keyed on the band in two columns, but Article 7(2) restates the distance conditions in full. Keying them on distance keeps the rows one-for-one with the text.",
        ],
    ),
    (
        "領収書の印紙税.rule",
        "領収書の印紙税",
        "国税庁タックスアンサー No.7141 の第17号文書（売上代金に係る金銭又は有価証券の受取書）の税額表です。受取金額のほかに、金額の記載があるか、営業に関するものかで決まります。",
        [
            "**非課税は 0 円の行です。** 5 万円未満と、営業に関しないものは非課税で、表はそれを `0円` の行として持ちます。「課税されない」も規則の答えのひとつです。",
            "**金額の記載が無いものは金額を見ません。** `金額の記載あり` が false の行は受取金額の列が `-` で、いくらでも 200 円です。三つの入力のどの組み合わせもちょうど一行に当たることを、検査が証明しています。",
        ],
        "Stamp duty on a receipt",
        "The table for document type 17 (a receipt for the proceeds of a sale) from NTA tax answer No.7141. Besides the amount received, whether an amount is stated and whether the receipt is in the course of business decide it.",
        [
            "**Exempt is a row of 0 yen.** Under 50,000 yen, and receipts not in the course of business, are exempt, and the table holds that as `0円` rows: \"not taxed\" is an answer of the rule too.",
            "**A receipt with no amount stated does not look at the amount.** The row with `金額の記載あり` false has `-` in the amount column and is 200 yen whatever the amount. The checker proves that every combination of the three inputs hits exactly one row.",
        ],
    ),
    (
        "所得税.rule",
        "所得税の速算表",
        "国税庁タックスアンサー No.2260 の速算表です。課税される所得金額の段ごとに税率と控除額があり、`課税所得 × 税率 − 控除額` で所得税が出ます。復興特別所得税はその 2.1% です。",
        [
            "**一つの表が率と金額を同時に出します。** 税率の列は `rate[step 1%]`、控除額の列は `money[円]` で、`define` がその二つを掛けて引きます。ページの計算例（7,000,000円 × 0.23 − 636,000円 = 974,000円）が、そのまま `examples` の行です。",
            "**段の境目は「次の段の始まり未満」で書いています。** ページは「1,000円 から 1,949,000円まで」「1,950,000円 から」と書きますが、課税所得は 1,000円 単位なので同じことです。整数の全体で完全性を証明するには、隙間の無い書き方のほうが要ります。",
            "**書いていないことは仮置きと明記します。** 復興特別所得税に 1 円未満の端数が出たときの扱いは、このページにはありません。`round down` に仮置きして、宣言の横のコメントに出典が無いことを残しています。承認する人は `rulec doc` でそれを読みます。",
        ],
        "The income-tax bracket table",
        "The quick-calculation table of NTA tax answer No.2260. Each bracket of taxable income carries a rate and a deduction, and `taxable × rate − deduction` is the tax; the reconstruction surtax is 2.1% of it.",
        [
            "**One table produces a rate and an amount at once.** The rate column is `rate[step 1%]`, the deduction column `money[円]`, and a `define` multiplies and subtracts. The page's own worked example (7,000,000 × 0.23 − 636,000 = 974,000 yen) is an `examples` row as it stands.",
            "**Bracket edges are written as \"below the start of the next bracket\".** The page says \"from 1,000 to 1,949,000 yen\" and \"from 1,950,000 yen\"; since taxable income is in units of 1,000 yen those are the same thing, and completeness over all the integers needs the form with no gap.",
            "**What the page does not say is marked as a placeholder.** How a fraction of a yen in the surtax is settled is not on this page. The rule says `round down` and keeps, in the comment beside the declaration, that the source is silent — which is what `rulec doc` shows the approver.",
        ],
    ),
    (
        "印紙税.rule",
        "契約書の印紙税と、期限つきの軽減税率",
        "不動産の譲渡に関する契約書（第1号文書）の印紙税額です。本則（No.7140）と、令和9年3月31日までに作成された契約書の軽減税率（No.7108）を一つの表に持ちます。作成日が入力です。",
        [
            "**期限のある特例は、日付の定義と一つの列になります。** `define 軽減期間 = 作成日 <= 2027-03-31` を列に置き、軽減の行は `true`、本則の行は `false`、期間によらない行は `-` です。`policy unique` なので、どの契約金額もどの作成日も、ちょうど一行に当たることが証明されています。",
            "**軽減の対象外は本則の行が受けます。** 軽減は契約金額が 10 万円を超えるものだけなので、1 万円未満（非課税）と 10 万円以下の行は、期間の列が `-` です。",
            "**この規則が生成器の欠陥を一つ見つけました。** 定義の中の日付リテラルが、六言語すべてで 0 として生成されていました。参照評価器は正しく読んでいたので、`rulec test` の突き合わせで食い違いとして出ました。",
        ],
        "Stamp duty on a contract, with a reduced rate that expires",
        "The stamp duty on a contract for the transfer of real estate (document type 1). The standard amounts (No.7140) and the reduced amounts for contracts made up to 31 March 2027 (No.7108) sit in one table, with the date of the contract as an input.",
        [
            "**A time-limited exception is a date definition and one column.** `define 軽減期間 = 作成日 <= 2027-03-31` goes into a column: `true` on the reduced rows, `false` on the standard ones, `-` where the period does not matter. Under `policy unique` every amount on every date is proved to hit exactly one row.",
            "**What the reduction does not cover, the standard rows take.** The reduction applies only above 100,000 yen, so the exempt row (under 10,000 yen) and the row up to 100,000 yen have `-` in the period column.",
            "**This rule found a defect in the generator.** A date literal inside a definition was generated as 0 in all six languages. The reference evaluator read the date, so the disagreement showed up in `rulec test`.",
        ],
    ),
    (
        "厚生年金保険料.rule",
        "厚生年金保険料の等級表",
        "日本年金機構の厚生年金保険料額表（令和8年度版）です。32 等級で、料率は一般の被保険者なら 18.3% ですが、厚生年金基金の加入員は基金ごとに違うので入力にしてあります。",
        [
            "**健康保険料と同じ形です。** 表が 50 等級から 32 等級になり、上限が 650,000 円になるだけで、折半と二つの端数処理はそのままです。同じ形の規則は、同じ形に写せます。",
            "**この表では二つの端数処理が同じ答えになります。** 18.3% × 標準報酬月額は必ず偶数の円なので、折半額に端数が出ません。規則には二つの丸め方が書いてありますが、この料率では表に現れない、ということまで `examples` が示しています。",
            "**印刷された 32 等級すべてと突き合わせてあります。** 表の折半額を写した記録（`tests/oracle/`）に `rulec replay` を当て、全件一致することをテストが確かめます。",
        ],
        "The employees' pension grade table",
        "The premium table for employees' pension from 日本年金機構 (fiscal 2026 edition): 32 grades, and a rate that is 18.3% for ordinary insured people but varies by fund for members of a pension fund, so the rate is an input.",
        [
            "**The same shape as the health-insurance rule.** Fifty grades become thirty-two and the ceiling is 650,000 yen; the halving and the two ways of settling the sen are unchanged. Rules of one shape transcribe into rules of one shape.",
            "**Here the two ways agree.** 18.3% of a standard remuneration is always an even number of yen, so the half has no fraction. The rule states both roundings; the `examples` show that at this rate the difference never appears.",
            "**All 32 printed grades are held to the rule.** The printed halves are transcribed into records (`tests/oracle/`), and a test replays the rule over them and requires every one to agree.",
        ],
    ),
    (
        "健康保険料.rule",
        "保険料額表を写して、二つの端数処理を出す",
        "協会けんぽの保険料額表（東京支部、令和8年3月分から）です。報酬月額から 50 等級の標準報酬月額を引き、料率を掛けて折半します。円未満の端数は、給与から控除するときと現金で納めるときで丸め方が違います。この表を写すために `half_down` が言語に入りました。",
        [
            "**同じ折半額から、丸め方の違う二つの出力を返します。** 表の注記は、給与から控除するなら「50銭以下は切り捨て、50銭を超える場合は切り上げ」、現金で納めるなら「50銭未満は切り捨て、50銭以上は切り上げ」と書いています。後者は `half_up`、前者は `half_down` です。折半額が 6,599.5 円の等級で、二つの出力は 1 円違います。",
            "**料率は入力です。** 都道府県ごと、年度ごとに変わるものを規則に焼き込むと、改定のたびに表を書き換えることになります。`derive` で健康保険料率と介護保険料率を足し、介護保険第2号被保険者かどうかで、表が使う率を選びます。",
            "**表の全等級と突き合わせてあります。** 印刷された折半額を写した記録（`tests/oracle/`）に `rulec replay` を当て、100 件すべてで一致することをテストが確かめます。",
        ],
        "A premium table, with two ways to settle the sen",
        "The 協会けんぽ premium table (Tokyo branch, from March 2026). Monthly pay picks one of 50 grades of standard remuneration, the rate is applied and the amount halved. Fractions of a yen are settled two different ways — one when the premium is deducted from salary, another when it is paid in cash — and transcribing this table is what put `half_down` into the language.",
        [
            "**Two outputs from one halved amount, rounded two ways.** The table's notes say: deducted from salary, half a yen or less is dropped and more than half is carried up; paid in cash, less than half is dropped and half or more is carried up. The second is `half_up`; the first is `half_down`. At the grade whose half is 6,599.5 yen the two outputs differ by one yen.",
            "**The rates are inputs.** They change by prefecture and by year; baking them into the rule would mean rewriting the table at every revision. A `derive` adds the care-insurance rate to the health-insurance rate, and a table picks which applies by whether the person is a category-2 care insured.",
            "**Every grade is held to the printed table.** The printed halves are transcribed into records (`tests/oracle/`), and a test replays the rule over all 100 of them and requires every one to agree.",
        ],
    ),
]

JA_HEAD = """# 例で見る

ここにあるのは全部、**このリポジトリのテストが毎回走らせている規則**です。`rulec check` を通り、書いてある例が実行され、参照評価器と生成したどの言語も同じ答えを返すことまで確かめられています。そのままコピーして動かせます。

小さいものから順に並べてあります。

"""

EN_HEAD = """# Examples

Every rule on this page is **one the repository's tests run on every commit**: it passes
`rulec check`, its own examples execute, and the reference evaluator and every generated
language are held to the same answers byte for byte. Copy any of them and it works.

They are ordered smallest first.

"""

JA_TAIL = """---

[表(.rule)を書く](tour.md){ .md-button .md-button--primary }
[文法](reference.md){ .md-button }
"""

EN_TAIL = """---

[Write a table (.rule)](tour.md){ .md-button .md-button--primary }
[Grammar](reference.md){ .md-button }
"""


def page(head, tail, title_i, lede_i, points_i, shows):
    out = [head]
    for e in EXAMPLES:
        src = (CORPUS / e[0]).read_text(encoding="utf-8").rstrip("\n")
        out.append(f"## {e[title_i]}\n\n{e[lede_i]}\n\n```rule\n{src}\n```\n\n**{shows}**\n\n")
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
