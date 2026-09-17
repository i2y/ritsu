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
fn 上流が出せない組み合わせの行は名指しされる() {
    assert_eq!(dead_rows(&stacked("黒", "乙")), vec![1], "死んだ行が E102 で名指しされていない");
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
fn 先行行に取られる上流の値も名指しされる() {
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
    assert_eq!(dead_rows(src), vec![1], "先行行に取られる組み合わせが名指しされていない");
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
