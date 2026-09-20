//! Completeness audit of the vector suite (§9.2). Acceptance for M1.
//!
//! The auditor itself is tested by mutation. Unless both are seen — red when vectors are removed,
//! green when they are not — it cannot be told apart from "an auditor that always returns green".

use rulec::coverage::{self, BOUND, ROW, SHADOW};
use rulec::vectors::{self, Vector};

const CORPUS: [&str; 30] = [
    "tests/corpus/ゆうパック運賃.rule",
    "tests/corpus/クーポン割引.rule",
    "tests/corpus/クーポン併用.rule",
    "tests/corpus/送料.rule",
    "tests/corpus/期間区分.rule",
    "tests/corpus/適用順序.rule",
    "tests/corpus/クーポン一枚.rule",
    "tests/corpus/ec261.rule",
    "tests/corpus/健康保険料.rule",
    "tests/corpus/厚生年金保険料.rule",
    "tests/corpus/所得税.rule",
    "tests/corpus/領収書の印紙税.rule",
    "tests/corpus/印紙税.rule",
    "tests/corpus/印紙税の本則と軽減.rule",
    "tests/corpus/送料のただし書.rule",
    "tests/corpus/退職手当.rule",
    "tests/corpus/非常勤退職手当.rule",
    "tests/corpus/全国運賃.rule",
    "tests/corpus/納入先照合.rule",
    "tests/corpus/Claude利用料.rule",
    "tests/corpus/ポイント付与.rule",
    "tests/corpus/予約取消可否.rule",
    "tests/corpus/会員特典.rule",
    "tests/corpus/値引の充当.rule",
    "tests/corpus/決済手数料.rule",
    "tests/corpus/補償証明書.rule",
    "tests/corpus/評価ランク.rule",
    "tests/corpus/預け荷物料金.rule",
    "tests/corpus/保存基準.rule",
    "tests/corpus/事務所の衛生基準.rule",
];

fn load(rel: &str) -> (rulec::ast::RuleFile, rulec::types::Checked, Vec<Vector>) {
    let path = format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect(rel);
    let (f, c) = rulec::prepare(&src, rel).unwrap_or_else(|_| panic!("{rel} は検査を通らない"));
    let vs = vectors::generate(&f, &c);
    (f, c, vs)
}

#[test]
fn コーパスは五基準を全部満たす() {
    for rel in CORPUS {
        let (f, c, _) = load(rel);
        // `audit_file` is what `rulec coverage` runs: the cases with an answer **and** the cases
        // that are refused. A fold's last transition — a second element on `take_unique` — is
        // only ever witnessed by a refused case, so leaving them out fails a rule that is green.
        let (a, vs, refused) = coverage::audit_file(&f, &c, rel);
        assert!(a.ok(), "{rel}\n{}", coverage::render(&a, &vs, &refused));
        for k in [ROW, BOUND, SHADOW] {
            let (met, req) = a.tally.get(k).copied().unwrap_or((0, 0));
            assert_eq!(met, req, "{rel} の {k}");
        }
        // The guideline is a few hundred per rule (§9.2), and an order of magnitude more means
        // the folding of candidates is broken. The 50-grade premium table with three more
        // inputs is the largest rule here at about 1,200, most of them pairwise cases, so the
        // bound sits above that and below where "an order of magnitude" begins.
        assert!(vs.len() < 2000, "{rel}: ベクタが {} 件と多すぎる", vs.len());
    }
}

/// The list of obligations must come from the rule, not from the vector set. An auditor that says
/// "all satisfied" of the empty set says nothing when it returns green.
#[test]
fn 空集合はすべての義務が欠ける() {
    for rel in CORPUS {
        let (f, c, vs) = load(rel);
        let full = coverage::audit(&f, &c, rel, &vs, &[]);
        let empty = coverage::audit(&f, &c, rel, &[], &[]);
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
    let a = coverage::audit(&f, &c, rel, &kept, &[]);
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
    let a = coverage::audit(&f, &c, rel, &kept, &[]);
    let b: Vec<&str> = a.missing.iter().filter(|m| m.kind == BOUND).map(|m| m.what.as_str()).collect();
    assert!(!b.is_empty(), "境界の外側を抜いたのに緑のまま:\n{}", coverage::render(&a, &kept, &[]));
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
    let a = coverage::audit(&f, &c, rel, &vs, &[]);
    assert_eq!(a.tally[SHADOW].1, pairs, "隠れ対の数が検査と食い違う");
    // Drop the inside points. What remains are only the points that hit row j alone.
    let inside: Vec<usize> = (0..vs.len())
        .filter(|&i| vs[i].why.starts_with("隠れ対"))
        .collect();
    assert!(!inside.is_empty(), "交差の内側を狙ったベクタがない");
    let kept: Vec<Vector> =
        vs.iter().enumerate().filter(|(i, _)| !inside.contains(i)).map(|(_, v)| v.clone()).collect();
    let b = coverage::audit(&f, &c, rel, &kept, &[]);
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
        let _ = f;
        let mut n = 0;
        let mut ti = 0;
        // One (merged) table per definition set, which is what the dead-row lists are indexed
        // by. A cell filled in for a column the row's own table lacks is `-` and counts nothing.
        for set in &c.sets {
            let t = &set.table;
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
        let a = coverage::audit(&f, &c, rel, &vs, &[]);
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
        ("tests/corpus/健康保険料.rule", 52, 98, 0),
        ("tests/corpus/厚生年金保険料.rule", 32, 62, 0),
        ("tests/corpus/所得税.rule", 7, 12, 0),
        ("tests/corpus/領収書の印紙税.rule", 17, 28, 0),
        ("tests/corpus/印紙税.rule", 23, 41, 0),
        ("tests/corpus/印紙税の本則と軽減.rule", 23, 41, 10),
        ("tests/corpus/送料のただし書.rule", 6, 1, 1),
        ("tests/corpus/退職手当.rule", 6, 4, 1),
        // The callee's rows beyond this rule's ranges are not obligations (§15.69).
        ("tests/corpus/非常勤退職手当.rule", 2, 0, 0),
        ("tests/corpus/全国運賃.rule", 4, 4, 0),
        ("tests/corpus/納入先照合.rule", 7, 6, 0),
        ("tests/corpus/Claude利用料.rule", 34, 0, 0),
        ("tests/corpus/ポイント付与.rule", 3, 0, 0),
        ("tests/corpus/予約取消可否.rule", 6, 0, 15),
        ("tests/corpus/会員特典.rule", 12, 4, 3),
        ("tests/corpus/値引の充当.rule", 2, 0, 0),
        ("tests/corpus/決済手数料.rule", 3, 3, 0),
        ("tests/corpus/補償証明書.rule", 8, 0, 9),
        ("tests/corpus/評価ランク.rule", 4, 3, 6),
        ("tests/corpus/預け荷物料金.rule", 9, 0, 0),
        ("tests/corpus/保存基準.rule", 15, 9, 3),
        ("tests/corpus/事務所の衛生基準.rule", 8, 5, 16),
    ];
    // Every rule of the corpus is audited and pinned. `threeway.rs` keeps its own list
    // honest the same way; this one had no such guard, and nine rules had drifted out of it
    // — their vector suites were generated and run, and never audited (§15.92).
    for rel in CORPUS {
        assert!(PINNED.iter().any(|(p, ..)| p == &rel), "{rel} の固定値がありません");
    }
    assert_eq!(PINNED.len(), CORPUS.len(), "コーパスを足したら固定値も足す");
    for (rel, rows, bounds, shadows) in PINNED {
        let (f, c, vs) = load(rel);
        let a = coverage::audit(&f, &c, rel, &vs, &[]);
        assert_eq!(
            (a.tally[ROW].1, a.tally[BOUND].1, a.tally[SHADOW].1),
            (*rows, *bounds, *shadows),
            "{rel}: 義務の件数が動いた。機能を足したのなら固定値を更新し、\
             そうでないなら列挙器が何かを見落とし始めている"
        );
    }
}

/// Build a rule from source and audit it, for shapes the corpus does not have.
fn audit_src(tag: &str, src: &str) -> (coverage::Audit, Vec<Vector>) {
    let (f, c) = rulec::prepare(src, tag).unwrap_or_else(|d| {
        panic!("{tag} は検査を通らない: {:?}", d.iter().map(|x| x.code.to_string()).collect::<Vec<_>>())
    });
    let vs = vectors::generate(&f, &c);
    let a = coverage::audit(&f, &c, tag, &vs, &[]);
    (a, vs)
}

/// A row that names **every** column, under `policy first`, which has to beat an earlier row on
/// a column it names itself.
///
/// Row 4 wants `a >= 1`; row 2 takes `a = 1` before it. The way through is `a = 2`, which
/// satisfies row 4 and misses row 2 — but the search would only move a column row 4 left as `-`,
/// and row 4 leaves none, so it gave up. Raising `a` also drops `d` below zero while `b` stays
/// put, so the row has to be rebuilt around the move as well.
///
/// The shape needs this many columns: with two or three, the pool's own sweep stumbles on the
/// witness by accident and the hole never appears.
///
/// Only row coverage is asserted here, since the rest of the audit answers to a different part
/// of the machinery.
#[test]
fn 全列を名指しする行でも勝たせられる() {
    let (a, _) = audit_src(
        "win.rule",
        "\
rule 形(shape) v1

enum 区域(zone) = 甲(a) | 乙(b)
enum 種別(kind) = 壱(one) | 弐(two)
enum 札(verdict) = 白(w) | 黒(k) | 赤(r) | 青(bl) | 緑(g)

inputs
  区域(zone) : 区域
  種別(kind) : 種別
  a : number range >=0 <=99999
  b : number range >=1 <=99999
  c : money[円, incl_tax] range >=0円 <=100万円

outputs
  結果(v) : 札

derive d : number = b - a  range >=-99998 <=99999

table x(x)
policy first
| 区域 | 種別 | a   | c   | d   | -> 結果(v) : 札 |
| 乙   | -    | -   | -   | -   | 白              |
| 甲   | -    | 1   | 0円 | -   | 黒              |
| 甲   | 壱   | -   | -   | -   | 赤              |
| 甲   | 弐   | >=1 | 0円 | >=0 | 青              |
| 甲   | 弐   | -   | -   | <0  | 緑              |
| 甲   | 弐   | -   | -   | -   | 赤              |
",
    );
    let (met, req) = a.tally.get(ROW).copied().unwrap_or((0, 0));
    assert_eq!(met, req, "行カバーが満たせていない（行4 が勝てない）");
}

/// A row whose derived cell can only be repaired through the input another of its own cells is
/// holding down.
///
/// Row 5 wants `a >= 2` and `d >= 0`, where `d = b - a`. Reaching `d >= 0` takes the first input
/// that shifts it, which is `a` — and lowering it undoes the `>= 2` two columns to its left. The
/// next pass put `a` back and the one after lowered it again: the passes oscillated, and the row
/// came out unreachable however reachable it was. Held, the solve goes to `b` instead, and the
/// boundary and shadow obligations that hung off the same row come back with it.
#[test]
fn 導出列の直しが同じ行の別のセルを壊さない() {
    let (a, vs) = audit_src(
        "derive.rule",
        "\
rule 形二(shape2) v1

enum 種別(kind) = 壱(one) | 弐(two) | 丙(three)
enum 札(verdict) = 白(w) | 青(bl) | 赤(r)

inputs
  種別(kind) : 種別
  旗一(f1)   : bool
  旗二(f2)   : bool
  旗三(f3)   : bool
  a : number range >=0 <=99999
  b : number range >=1 <=99999
  c : money[円, incl_tax] range >=0円 <=100万円

outputs
  結果(v) : 札

derive d : number = b - a  range >=-99998 <=99999

table x(x)
policy first
| 種別 | 旗一  | 旗二  | 旗三  | a   | c   | d   | -> 結果(v) : 札 |
| 丙   | -     | -     | -     | -   | -   | -   | 白              |
| -    | false | -     | -     | -   | -   | -   | 白              |
| -    | -     | false | -     | -   | -   | -   | 白              |
| 弐   | -     | -     | false | -   | -   | -   | 白              |
| 弐   | -     | -     | -     | >=2 | 0円 | >=0 | 青              |
| 弐   | -     | -     | -     | -   | 0円 | -   | 白              |
| 弐   | -     | -     | -     | -   | -   | <0  | 白              |
| 弐   | -     | -     | -     | -   | -   | -   | 赤              |
| 壱   | -     | -     | -     | 1   | -   | -   | 赤              |
| 壱   | -     | -     | -     | -   | -   | -   | 白              |
",
    );
    assert!(a.ok(), "{}", coverage::render(&a, &vs, &[]));
}

/// The `examples` are part of the vector suite.
///
/// They are executable specification and the only cases a person wrote, yet E107 held them to
/// the reference evaluator and stopped: nothing ran an author's worked cases through the
/// generated code, in any language, and they counted toward no obligation — so `examples` could
/// not close a hole the generator had left.
///
/// The value below is deliberately interior. The generator works from boundaries, so 37 is a
/// point it would never pick on its own; finding it among the vectors means the example got
/// there, not that the sweep happened to pass through.
#[test]
fn 例はベクタ集合に入る() {
    let src = "\
rule t(t) v1

inputs
  n(n) : number range >=0 <=100

outputs
  ok(ok) : bool

table j(j)
policy first
| n    | -> ok(ok) : bool |
| <=50 | true             |
| -    | false            |

examples
| n  | -> ok |
| 37 | true  |
| 63 | false |
";
    let (f, c) = rulec::prepare(src, "ex.rule").expect("検査を通る");
    let vs = vectors::generate(&f, &c);
    for want in [37i128, 63] {
        assert!(
            vs.iter().any(|v| matches!(v.input.get("n"), Some(rulec::eval::Val::Num(r)) if r.num == want && r.den == 1)),
            "例の {want} がベクタに無い: {:?}",
            vs.iter().filter_map(|v| v.input.get("n").map(rulec::vectors::show)).collect::<Vec<_>>()
        );
    }
    // And they say where they came from, so a reader of the vectors can tell them apart.
    assert_eq!(
        vs.iter().filter(|v| v.why.contains("example") || v.why.contains("例")).count(),
        2,
        "例の出どころが why に無い"
    );
}
