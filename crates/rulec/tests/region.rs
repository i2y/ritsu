//! The box algebra's treatment of a **derived** column (§6.2, DESIGN §15.27 and §15.29).
//!
//! §6 makes every column an independent axis, which is what leaves completeness and overlap
//! exactly decidable. A `derive` is not independent — it is a linear combination of inputs — so
//! the product of the axes contains points no input can reach, and an overlap read off the boxes
//! alone can be one that cannot happen.
//!
//! Two properties matter here and they pull against each other. An overlap that cannot happen
//! must not be reported, or a correct rule is rejected. An overlap that **can** happen must
//! never be dropped, because that is the direction a tool claiming to prove things must not
//! fail in. The pair of rules below differ in one character and land on opposite sides.

use ritsu_testkit::TempDir;

/// The same rule twice, with the lower bound of `b` as the only difference.
fn rule(b_low: &str) -> String {
    format!(
        "\
rule t(t) v1

inputs
  a : number range >=0 <=10
  b : number range >={b_low} <=10

outputs
  ok(ok) : bool

derive d : number = b - a  range >=-10 <=10

table x(x)
policy first
| a | d   | -> ok(ok) : bool |
| 1 | -   | true             |
| - | <0  | false            |
| - | -   | false            |
"
    )
}

fn pairs(src: &str, code: &str) -> Vec<(usize, usize)> {
    rulec::check_source(src, "region.rule")
        .iter()
        .filter(|d| d.code == code && d.rows.len() == 2)
        .map(|d| (d.rows[0].row, d.rows[1].row))
        .collect()
}

/// With `b >= 1`, row 1 pins `a = 1` and `d = b - a` is then at least 0, so row 2's `d < 0`
/// cannot hold at the same time. The pair is impossible and gets no diagnostic of any kind —
/// not W105, and not the W114 that would otherwise put a runtime guard in six languages for a
/// case that can never arrive.
#[test]
fn 起こり得ない重なりは報告されない() {
    let src = rule("1");
    assert!(
        !pairs(&src, "W105").contains(&(1, 2)),
        "起こり得ない対に W105 が出ている: {:?}",
        pairs(&src, "W105")
    );
    assert!(
        !pairs(&src, "W114").contains(&(1, 2)),
        "起こり得ない対に W114 が出ている: {:?}",
        pairs(&src, "W114")
    );
    // And the rule is accepted, which is the whole point: it is correct, and `policy unique`
    // would have turned that impossible pair into an E105 that rejects it.
    assert!(
        !rulec::has_error(&rulec::check_source(&src, "region.rule")),
        "正しい規則が弾かれている: {:?}",
        rulec::check_source(&src, "region.rule")
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>()
    );
}

/// One character apart: with `b >= 0`, `a = 1` and `b = 0` give `d = -1`, so rows 1 and 2 do
/// meet. **It has to be reported.** Dropping a real overlap is the failure this tool cannot
/// afford, and the tightening that silences the case above must not reach this one.
#[test]
fn 本物の重なりは導出ごしでも報告される() {
    let src = rule("0");
    assert!(
        pairs(&src, "W105").contains(&(1, 2)),
        "起こり得る対が報告されていない: {:?}",
        pairs(&src, "W105")
    );
}

/// And the audit does not ask for a point that cannot exist. `coverage` demands a witness for
/// every shadow pair, so an impossible one left the rule permanently red with nothing an author
/// could do about it.
#[test]
fn 起こり得ない隠れ対はカバー義務にならない() {
    let src = rule("1");
    let (f, c) = rulec::prepare(&src, "region.rule").expect("検査を通る");
    let vs = rulec::vectors::generate(&f, &c);
    let a = rulec::coverage::audit(&f, &c, "region.rule", &vs, &[]);
    assert!(a.ok(), "{}", rulec::coverage::render(&a, &vs, &[]));
}

/// A rule whose lower table names an upstream answer together with the column that decides it.
fn stacked(mark: &str, kind: &str) -> String {
    format!(
        "\
rule t(t) v1

enum 種(kind) = 甲(a) | 乙(b)
enum 札(mark) = 黒(k) | 白(w)

inputs
  種(kind) : 種

outputs
  結果(r) : bool

table 上(up)
policy first
| 種 | -> 札(mark) : 札 |
| 甲 | 黒               |
| -  | 白               |

table 下(down)
policy first
| 札   | 種   | -> 結果(r) : bool |
| {mark} | {kind} | true              |
| -    | -    | false             |
"
    )
}

fn dead_rows(src: &str) -> Vec<usize> {
    rulec::check_source(src, "region.rule")
        .iter()
        .filter(|d| d.code == "E102")
        .filter_map(|d| d.row)
        .collect()
}

/// `黒` comes only from `種 = 甲`, so a row asking for `黒` beside `種 = 乙` is dead — however
/// live each half is on its own. Read a column at a time, both are ordinary values the table
/// above really does produce, which is why this used to pass `check` and then sit in `coverage`
/// as a hole the author could not close.
#[test]
fn 上流が出せない組み合わせの行は指摘される() {
    assert_eq!(dead_rows(&stacked("黒", "乙")), vec![1], "死んだ行を E102 が指摘していない");
}

/// One value apart: `白` is exactly what `種 = 乙` produces, so the row is alive. **It must not
/// be touched.** E102 is an error, and this is the direction where being too clever rejects a
/// correct rule — the one thing worse than the hole this check closes.
#[test]
fn 上流が出せる組み合わせの行は生きたまま() {
    assert!(dead_rows(&stacked("白", "乙")).is_empty(), "生きている行を殺している");
    assert!(dead_rows(&stacked("黒", "甲")).is_empty(), "生きている行を殺している");
}

/// The same when the producing row exists but an earlier row of its own table takes the case
/// first. `黒` is row 2 of the table above, and row 1 covers it whenever `旗` is false, so
/// asking for `黒` with `旗 = false` below is asking for something that never arrives.
#[test]
fn 先行行に取られる上流の値も指摘される() {
    let src = "\
rule t(t) v1

enum 種(kind) = 甲(a) | 乙(b)
enum 札(mark) = 黒(k) | 白(w)

inputs
  種(kind) : 種
  旗(flag) : bool

outputs
  結果(r) : bool

table 上(up)
policy first
| 種 | 旗    | -> 札(mark) : 札 |
| 甲 | false | 白               |
| 甲 | -     | 黒               |
| -  | -     | 白               |

table 下(down)
policy first
| 札 | 種 | 旗    | -> 結果(r) : bool |
| 黒 | 甲 | false | true              |
| -  | -  | -     | false             |
";
    assert_eq!(dead_rows(src), vec![1], "先行行に取られる組み合わせが指摘されていない");
}

/// And the completeness side must agree with it.
///
/// E102 naming a row dead and E101 demanding a row for the same combination is the tool telling
/// an author to do two opposite things: delete this row, then add it back. The gap search reads
/// the tables above for the same reason the row check does.
#[test]
fn 上流が出せない組み合わせは穴として要求されない() {
    // The row that covered `黒 × 乙` is gone. That combination cannot happen, so its absence is
    // not a gap.
    let src = "\
rule t(t) v1

enum 種(kind) = 甲(a) | 乙(b)
enum 札(mark) = 黒(k) | 白(w)

inputs
  種(kind) : 種

outputs
  結果(r) : bool

table 上(up)
policy first
| 種 | -> 札(mark) : 札 |
| 甲 | 黒               |
| -  | 白               |

table 下(down)
policy unique
| 札 | 種 | -> 結果(r) : bool |
| 黒 | 甲 | true              |
| 白 | -  | false             |
| 黒 | 乙 | false             |
";
    // With the row present the table is complete; take it away and the gap it leaves is one the
    // table above cannot produce.
    let without = src.replace("| 黒 | 乙 | false             |\n", "");
    assert!(without != src, "消す行が見つからない");
    let codes: Vec<&str> =
        rulec::check_source(&without, "region.rule").iter().map(|d| d.code).collect();
    assert!(!codes.contains(&"E101"), "起こり得ない組み合わせが穴として要求されている: {codes:?}");
}

/// An output cell holding a **name** is not a value this check can compare against a cell below.
/// The parser writes both readings of a bare word as `Name` (§3.2), and the one that matters
/// here is the second: `長い` in the cell of a numeric column stands for the input of that name,
/// not for an enum value spelled that way. Read as a value it matches no numeric cell, so every
/// row below is judged to have no producer — and a substitution table, which is how 準用 is
/// written, comes back rejected with E102 in full.
#[test]
fn 名前を出す上流の列は下流の行を殺さない() {
    let src = "\
rule t(t) v1

enum 区分(kind) = 甲(a) | 乙(b)

inputs
  区分(kind)  : 区分
  長い(long)  : number range >=0 <=40
  短い(short) : number range >=0 <=3

outputs
  額(amount) : money[円] round down(1円)

table 読替(sub)
policy unique
| 区分 | -> 期間(span) : number |
| 甲   | 長い                   |
| 乙   | 短い                   |

table 下(down)
policy unique
| 期間 | -> 額(amount) : money[円] |
| <10  | 1000円                    |
| >=10 | 5000円                    |
";
    assert!(dead_rows(src).is_empty(), "名前を出す上流のせいで下流の行が殺されている");
    let codes: Vec<&str> = rulec::check_source(src, "region.rule").iter().map(|d| d.code).collect();
    assert!(!codes.contains(&"E101"), "名前の出す値域が下流に届いていない: {codes:?}");
}


/// A set on a column of numbers is its values, each a point of the axis, as it is on an enum
/// (§15.143). `100, 200` used to take no coordinate at all: the row was unreachable (E102),
/// and the boundary vectors stood on none of its values.
#[test]
fn 数の集合は値ごとの点になる() {
    let src = "rule 個数の割引(pieces) v1\n\ninputs\n  個数(n) : number  range >=1 <=500\n\noutputs\n  割引(off) : money[円]  round down(1円)\n\ntable 割引表(t)\npolicy first\n| 個数         | -> 割引 |\n| 100, 200     | 500円   |\n| not: 300, 400 | 100円   |\n| -            | 0円     |\n";
    let ds = rulec::check_source(src, "pieces.rule");
    assert!(!rulec::has_error(&ds), "{:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
    let tmp = TempDir::new("region-set");
    let dir = tmp.path().to_path_buf();
    let p = dir.join("pieces.rule");
    std::fs::write(&p, src).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").args(["vectors", p.to_str().unwrap()]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let got = |n: i128| {
        text.lines()
            .filter_map(|l| rulec::json::parse(l).ok())
            .filter(|v| v.get("in").and_then(|i| i.get("個数")).and_then(|x| x.as_int()) == Some(n))
            .map(|v| v.get("out").and_then(|o| o.get("割引")).and_then(|x| x.as_int()) == Some(500))
            .collect::<Vec<_>>()
    };
    for n in [100, 200] {
        assert_eq!(got(n).first(), Some(&true), "{n} は 500円の行: {text}");
    }
    for n in [99, 101, 199, 201] {
        assert_eq!(got(n).first(), Some(&false), "{n} が境目のベクタに無いか、行を取り違えている: {text}");
    }
}

/// A cell of the table above that passes an input of the column's enum through (`| review,
/// approved | false | current |`) writes only what that row's own cell on the input allows, here
/// `review` and `approved`; the row below that asks for `draft` is the one no input reaches. The
/// checks that say which values come down (E102) and which a row is owed (E101) read the cell the
/// same way (§15.208): with the row, E102 alone; without it, nothing. They used to read it two
/// ways, and said E102 and E101 of the same value at once. The English rule, and its Japanese
/// version.
#[test]
fn 入力をそのまま渡すセルは行が許す値だけを書く() {
    let en = "rule review_route v1\ndescription \"Where a document goes next, and whether to notify. A draft goes to review; a document already in review or approved stays where it is unless it is urgent, and the stage it stays in is passed through as it is\"\n\nenum stage = draft | review | approved\n\ninputs\n  current : stage\n  urgent  : bool\n\noutputs\n  notify : bool\n\ntable route\npolicy unique\n| current          | urgent | -> next_stage : stage |\n| draft            | -      | review                |\n| review, approved | true   | review                |\n| review, approved | false  | current               |\n\ntable notice\npolicy unique\n| next_stage | -> notify : bool |\n| draft      | false            |\n| review     | true             |\n| approved   | false            |\n";
    let ja = "rule 回付先(review_route) v1\ndescription \"文書の次の段階と、知らせるかどうか。下書きは確認へ回る。確認中か承認済みの文書は、急ぎでなければその段階のままで、その段階をそのまま渡す\"\n\nenum 段階(stage) = 下書き(draft) | 確認中(review) | 承認済み(approved)\n\ninputs\n  現在(current) : 段階\n  急ぎ(urgent)  : bool\n\noutputs\n  通知(notify) : bool\n\ntable 回付(route)\npolicy unique\n| 現在             | 急ぎ  | -> 次の段階(next_stage) : 段階 |\n| 下書き           | -     | 確認中                         |\n| 確認中, 承認済み | true  | 確認中                         |\n| 確認中, 承認済み | false | 現在                           |\n\ntable 知らせ(notice)\npolicy unique\n| 次の段階 | -> 通知 : bool |\n| 下書き   | false          |\n| 確認中   | true           |\n| 承認済み | false          |\n";
    for (src, draft_row) in [(en, "| draft      | false            |\n"), (ja, "| 下書き   | false          |\n")] {
        let codes = |s: &str| -> Vec<(&'static str, Option<usize>)> { rulec::check_source(s, "region.rule").iter().map(|d| (d.code, d.row)).collect() };
        assert_eq!(codes(src), vec![("E102", Some(1))], "{src}");
        let without = src.replace(draft_row, "");
        assert_ne!(without, src);
        assert!(codes(&without).is_empty(), "{:?}\n{without}", codes(&without));
    }
}

/// A row that asks a computed column for a value is reached by some input or by none, and where
/// neither was found the check says so (W128, §15.209) rather than passing the row as reached: the
/// product of two numbers is 997 only as one box of 997 or 997 boxes of one, and no candidate the
/// search walks is either. An example that reaches the row decides it, and the warning goes. A row
/// of inputs alone is not searched: its point is an input. The English rule, and its Japanese
/// version.
#[test]
fn 決められない行はw128で言い_例で決まる() {
    use rulec::i18n::{self, Lang};
    let en = "rule lucky_batch v1\ndescription \"A batch of exactly 997 units gets a prize. 997 is a prime, so only one box of 997 units or 997 boxes of one unit make it\"\n\ninputs\n  boxes   : number  range >=0 <=1000\n  per_box : number  range >=0 <=1000\n\noutputs\n  prize : bool\n\ndefine units : number = boxes * per_box\n\ntable prize_of\npolicy first\n| units | -> prize : bool |\n| 997   | true            |\n| -     | false           |\n";
    let ja = "rule 当たりの束(lucky_batch) v1\ndescription \"ちょうど 997 個の束に賞が出る。997 は素数なので、997 個入りの箱が一つか、1 個入りの箱が 997 個のときだけ\"\n\ninputs\n  箱の数(boxes)      : number  range >=0 <=1000\n  一箱の数(per_box)  : number  range >=0 <=1000\n\noutputs\n  賞(prize) : bool\n\ndefine 個数(units) : number = 箱の数 * 一箱の数\n\ntable 賞の表(prize_of)\npolicy first\n| 個数 | -> 賞 : bool |\n| 997  | true         |\n| -    | false        |\n";
    for (src, lang, title, example) in [
        (en, Lang::En, "No input was found that reaches row 1, and none was shown not to", "\nexamples\n| boxes | per_box | -> prize |\n| 1     | 997     | true     |\n"),
        (ja, Lang::Ja, "行1 に当たる入力を見つけられず、当たらないことも示せませんでした", "\nexamples\n| 箱の数 | 一箱の数 | -> 賞 |\n| 1      | 997      | true  |\n"),
    ] {
        let ds = i18n::with(lang, || rulec::check_source(src, "region.rule"));
        let found: Vec<_> = ds.iter().map(|d| (d.code, d.title.as_str(), d.row)).collect();
        assert_eq!(found, vec![("W128", title, Some(1))], "{src}");
        let marked = ds[0].notes.join("\n");
        assert!(marked.contains(if lang == Lang::En { "`units`" } else { "`個数`" }), "{marked}");
        let with = format!("{src}{example}");
        let ds = i18n::with(lang, || rulec::check_source(&with, "region.rule"));
        assert!(ds.is_empty(), "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
    }
    // the same row on the input itself is no question: its point is an input
    let direct = en.replace("| units | -> prize : bool |\n| 997   |", "| boxes | -> prize : bool |\n| 997   |").replace("| -     | false           |", "| -     | false           |");
    let ds = rulec::check_source(&direct, "region.rule");
    assert!(!ds.iter().any(|d| d.code == "W128"), "{:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
}

