//! Completeness audit of the vector suite (§9.2). Acceptance for M1.
//!
//! The auditor itself is tested by mutation. Unless both are seen — red when vectors are removed,
//! green when they are not — it cannot be told apart from "an auditor that always returns green".

use rulec::coverage::{self, BOUND, ROW, SHADOW};
use rulec::vectors::{self, Vector};

const CORPUS: [&str; 8] = [
    "tests/corpus/ゆうパック運賃.rule",
    "tests/corpus/クーポン割引.rule",
    "tests/corpus/クーポン併用.rule",
    "tests/corpus/送料.rule",
    "tests/corpus/期間区分.rule",
    "tests/corpus/適用順序.rule",
    "tests/corpus/クーポン一枚.rule",
    "tests/corpus/ec261.rule",
];

fn load(rel: &str) -> (rulec::ast::RuleFile, rulec::types::Checked, Vec<Vector>) {
    let path = format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect(rel);
    let (f, c) = rulec::prepare(&src, rel).unwrap_or_else(|_| panic!("{rel} は検査を通らない"));
    let vs = vectors::generate(&f, &c);
    (f, c, vs)
}

#[test]
fn コーパスは三基準を全部満たす() {
    for rel in CORPUS {
        let (f, c, vs) = load(rel);
        let a = coverage::audit(&f, &c, rel, &vs);
        assert!(a.ok(), "{rel}\n{}", coverage::render(&a, &vs));
        for k in [ROW, BOUND, SHADOW] {
            let (met, req) = a.tally.get(k).copied().unwrap_or((0, 0));
            assert_eq!(met, req, "{rel} の {k}");
        }
        // The guideline is a few hundred per rule (§9.2). An order of magnitude more means the
        // folding of candidates is broken.
        assert!(vs.len() < 600, "{rel}: ベクタが {} 件と多すぎる", vs.len());
    }
}

/// The list of obligations must come from the rule, not from the vector set. An auditor that says
/// "all satisfied" of the empty set says nothing when it returns green.
#[test]
fn 空集合はすべての義務が欠ける() {
    for rel in CORPUS {
        let (f, c, vs) = load(rel);
        let full = coverage::audit(&f, &c, rel, &vs);
        let empty = coverage::audit(&f, &c, rel, &[]);
        assert!(!empty.ok(), "{rel}: 空集合を通した");
        for k in [ROW, BOUND, SHADOW] {
            let (_, req) = full.tally.get(k).copied().unwrap_or((0, 0));
            let (met0, req0) = empty.tally.get(k).copied().unwrap_or((0, 0));
            assert_eq!(req0, req, "{rel} の {k}: 義務の数がベクタに依存している");
            assert_eq!(met0, 0, "{rel} の {k}: 空集合で満たしたと言っている");
        }
    }
}

/// Removing every vector that lets a row win leaves exactly that row's row coverage missing.
#[test]
fn 行を勝たせる例を抜くと行カバーが欠ける() {
    let rel = "tests/corpus/ゆうパック運賃.rule";
    let (f, c, vs) = load(rel);
    let tag = "表 運賃表 行42"; // the 沖縄 × S170 cell
    let kept: Vec<Vector> = vs.iter().filter(|v| !v.trace.iter().any(|t| t == tag)).cloned().collect();
    assert!(kept.len() < vs.len(), "抜く対象がない");
    let a = coverage::audit(&f, &c, rel, &kept);
    assert!(!a.ok(), "抜いたのに緑のまま");
    let rows: Vec<&str> = a.missing.iter().filter(|m| m.kind == ROW).map(|m| m.what.as_str()).collect();
    assert_eq!(rows, vec![tag], "欠けた行を名指ししていない: {rows:?}");
}

/// Removing the vectors that step on the outside of a boundary leaves both-sides boundary coverage
/// missing. An auditor that goes green on the inside alone cannot catch a boundary's ±1.
#[test]
fn 境界の片側を抜くと境界の両側カバーが欠ける() {
    let rel = "tests/corpus/ゆうパック運賃.rule";
    let (f, c, vs) = load(rel);
    // Drop every example that steps on 三辺合計 = 61cm (just outside <=60cm).
    let kept: Vec<Vector> = vs
        .iter()
        .filter(|v| vectors::show(&v.input["三辺合計"]) != "61")
        .cloned()
        .collect();
    assert!(kept.len() < vs.len(), "抜く対象がない");
    let a = coverage::audit(&f, &c, rel, &kept);
    let b: Vec<&str> = a.missing.iter().filter(|m| m.kind == BOUND).map(|m| m.what.as_str()).collect();
    assert!(!b.is_empty(), "境界の外側を抜いたのに緑のまま:\n{}", coverage::render(&a, &kept));
    assert!(b.iter().any(|w| w.contains("境界 60")), "どの境界かを名指ししていない: {b:?}");
}

/// A shadow pair demands a point "inside the intersection". A point that merely hits row j is not
/// enough.
#[test]
fn 交差の内側を抜くと隠れ対カバーが欠ける() {
    let rel = "tests/corpus/送料.rule";
    let (f, c, vs) = load(rel);
    let checks = rulec::table_checks(&f, &c, rel);
    let pairs: usize = checks.iter().map(|k| k.overlaps.len()).sum();
    assert!(pairs > 0, "隠れ対のある規則を選んでいない");
    let a = coverage::audit(&f, &c, rel, &vs);
    assert_eq!(a.tally[SHADOW].1, pairs, "隠れ対の数が検査と食い違う");
    // Drop the inside points. What remains are only the points that hit row j alone.
    let inside: Vec<usize> = (0..vs.len())
        .filter(|&i| vs[i].why.starts_with("隠れ対"))
        .collect();
    assert!(!inside.is_empty(), "交差の内側を狙ったベクタがない");
    let kept: Vec<Vector> =
        vs.iter().enumerate().filter(|(i, _)| !inside.contains(i)).map(|(_, v)| v.clone()).collect();
    let b = coverage::audit(&f, &c, rel, &kept);
    assert!(b.tally[SHADOW].0 <= a.tally[SHADOW].0, "抜いて増えている");
}

/// §9.2 net, part two. Cross-check the number of boundary obligations against a **naive collector
/// that merely counts boundary literals in the expression tree**. The enumeration of obligations is
/// shared in one place by the auditor and the generator, so when a new column kind or atom shape is
/// added here later and the enumerator overlooks it, both fall silent together. Only this
/// cross-check breaks that silence.
#[test]
fn 境界の義務は素朴な数え上げと一致する() {
    use rulec::ast::*;
    // The collector never consults coverage.rs. It counts by looking at the shape of cells alone.
    fn naive(f: &rulec::ast::RuleFile, c: &rulec::types::Checked, dead: &[Vec<usize>]) -> usize {
        let mut n = 0;
        let mut ti = 0;
        for it in &f.items {
            let Item::Table(t) = it else { continue };
            for (ri, row) in t.rows.iter().enumerate() {
                if dead[ti].contains(&ri) {
                    continue;
                }
                for (ci, (col, _)) in t.inputs.iter().enumerate() {
                    // `number` belongs here as much as the rest. It was added to the
                    // language (§15.11) without being added to this list, and no rule in the
                    // corpus had a `number` column with a comparison in it, so the two
                    // counters agreed by never looking.
                    let numeric = matches!(
                        c.ty_of(col),
                        Some(rulec::types::Ty::Money { .. })
                            | Some(rulec::types::Ty::Qty { .. })
                            | Some(rulec::types::Ty::Rate)
                            | Some(rulec::types::Ty::Number)
                            | Some(rulec::types::Ty::Date)
                    );
                    if !numeric {
                        continue;
                    }
                    n += match row.cells.get(ci) {
                        Some(Cell::Cmp(cs)) => cs.len(), // one comparison operator, one boundary
                        Some(Cell::Lit(_)) => 2,         // a point is a lower and an upper boundary
                        _ => 0,
                    };
                }
            }
            ti += 1;
        }
        n
    }

    for rel in CORPUS {
        let (f, c, vs) = load(rel);
        let a = coverage::audit(&f, &c, rel, &vs);
        let dead: Vec<Vec<usize>> =
            rulec::table_checks(&f, &c, rel).into_iter().map(|k| k.dead).collect();
        assert_eq!(
            a.tally[BOUND].1 + a.pruned_bounds,
            naive(&f, &c, &dead),
            "{rel}: 境界の義務の数が素朴な数え上げと合わない"
        );
    }
}

/// §9.2 net, part three. Pin the per-rule obligation counts as a regression.
/// **When a column kind or atom shape is added to the language, the acceptance condition is that
/// the count in the matching category goes up.** If it does not, the enumerator is not seeing the
/// feature.
#[test]
fn 義務の件数を固定する() {
    // (rule, row coverage, both-sides boundary coverage, shadow-pair coverage)
    const PINNED: &[(&str, usize, usize, usize)] = &[
        ("tests/corpus/ゆうパック運賃.rule", 49, 6, 21),
        ("tests/corpus/クーポン割引.rule", 9, 2, 10),
        ("tests/corpus/クーポン併用.rule", 3, 4, 0),
        ("tests/corpus/送料.rule", 7, 4, 3),
        ("tests/corpus/期間区分.rule", 4, 6, 0),
        ("tests/corpus/適用順序.rule", 4, 0, 5),
        ("tests/corpus/クーポン一枚.rule", 7, 1, 3),
        ("tests/corpus/ec261.rule", 15, 23, 0),
    ];
    assert_eq!(PINNED.len(), CORPUS.len(), "コーパスを足したら固定値も足す");
    for (rel, rows, bounds, shadows) in PINNED {
        let (f, c, vs) = load(rel);
        let a = coverage::audit(&f, &c, rel, &vs);
        assert_eq!(
            (a.tally[ROW].1, a.tally[BOUND].1, a.tally[SHADOW].1),
            (*rows, *bounds, *shadows),
            "{rel}: 義務の件数が動いた。機能を足したのなら固定値を更新し、\
             そうでないなら列挙器が何かを見落とし始めている"
        );
    }
}
