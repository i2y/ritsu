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
    let a = rulec::coverage::audit(&f, &c, "region.rule", &vs);
    assert!(a.ok(), "{}", rulec::coverage::render(&a, &vs));
}
