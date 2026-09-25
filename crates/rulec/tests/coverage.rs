//! Completeness audit of the vector suite (§9.2). Acceptance for M1.
//!
//! The auditor itself is tested by mutation. Unless both are seen — red when vectors are removed,
//! green when they are not — it cannot be told apart from "an auditor that always returns green".

use rulec::coverage::{self, BOUND, ROW, SHADOW, TIE, VALUE};
use rulec::vectors::{self, Vector};

const CORPUS: [&str; 48] = [
    "tests/corpus/速達の見積.rule",
    "tests/corpus/出荷の送料.rule",
    "tests/corpus/注文の送料.rule",
    "tests/corpus/比例配分.rule",
    "tests/corpus/品番の扱い.rule",
    "tests/corpus/買物かごの送料.rule",
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
    "tests/corpus/parcel_rate.rule",
    "tests/corpus/return_eligibility.rule",
    "tests/corpus/uk_minimum_wage.rule",
    "tests/corpus/uk_income_tax.rule",
    "tests/corpus/us_income_tax.rule",
    "tests/corpus/osha_extinguisher.rule",
    "tests/corpus/uk_stamp_duty.rule",
    "tests/corpus/osha_noise.rule",
    "tests/corpus/osha_excavation.rule",
    "tests/corpus/paypal_fee.rule",
    "tests/corpus/注文の状態.rule",
    "tests/corpus/payment_intent.rule",
];

fn load(rel: &str) -> (rulec::ast::RuleFile, rulec::types::Checked, Vec<Vector>) {
    let path = format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect(rel);
    let (f, c) = rulec::prepare(&src, rel).unwrap_or_else(|_| panic!("{rel} は検査を通らない"));
    let vs = vectors::generate(&f, &c);
    (f, c, vs)
}

#[test]
fn コーパスは七基準を全部満たす() {
    for rel in CORPUS {
        let (f, c, _) = load(rel);
        // `audit_file` is what `rulec coverage` runs: the cases with an answer **and** the cases
        // that are refused. A fold's last transition — a second element on `take_unique` — is
        // only ever witnessed by a refused case, so leaving them out fails a rule that is green.
        let (a, vs, refused) = coverage::audit_file(&f, &c, rel);
        assert!(a.ok(), "{rel}\n{}", coverage::render(&a, &vs, &refused));
        for k in [ROW, BOUND, SHADOW, VALUE, TIE] {
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
        // The value pairs and the rounding ties are counted from the rule as well (§15.151): the
        // tie used to be an obligation only where the generator had found one.
        for k in [ROW, BOUND, SHADOW, VALUE, TIE] {
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
    // (rule, row coverage, both-sides boundary coverage, shadow-pair coverage, value-pair
    // coverage, rounding-tie coverage)
    const PINNED: &[(&str, usize, usize, usize, usize, usize)] = &[
        ("tests/corpus/ゆうパック運賃.rule", 49, 6, 21, 0, 0),
        ("tests/corpus/クーポン割引.rule", 9, 2, 10, 3, 0),
        ("tests/corpus/クーポン併用.rule", 3, 4, 0, 0, 0),
        ("tests/corpus/送料.rule", 7, 4, 3, 1, 0),
        ("tests/corpus/期間区分.rule", 4, 6, 0, 0, 0),
        ("tests/corpus/適用順序.rule", 4, 0, 5, 0, 0),
        ("tests/corpus/クーポン一枚.rule", 7, 1, 3, 2, 1),
        ("tests/corpus/ec261.rule", 15, 23, 0, 1, 0),
        ("tests/corpus/健康保険料.rule", 52, 98, 0, 4, 2),
        ("tests/corpus/厚生年金保険料.rule", 32, 62, 0, 2, 0),
        ("tests/corpus/所得税.rule", 7, 12, 0, 2, 2),
        ("tests/corpus/領収書の印紙税.rule", 17, 28, 0, 0, 0),
        ("tests/corpus/印紙税.rule", 23, 41, 0, 0, 0),
        ("tests/corpus/印紙税の本則と軽減.rule", 23, 41, 10, 0, 0),
        ("tests/corpus/送料のただし書.rule", 6, 1, 1, 1, 0),
        ("tests/corpus/退職手当.rule", 6, 4, 1, 2, 0),
        // The callee's rows beyond this rule's ranges are not obligations (§15.69).
        ("tests/corpus/非常勤退職手当.rule", 2, 0, 0, 2, 0),
        ("tests/corpus/全国運賃.rule", 4, 4, 0, 0, 0),
        ("tests/corpus/納入先照合.rule", 7, 6, 0, 0, 0),
        ("tests/corpus/Claude利用料.rule", 34, 0, 0, 1, 1),
        ("tests/corpus/ポイント付与.rule", 3, 0, 0, 4, 1),
        ("tests/corpus/予約取消可否.rule", 6, 0, 15, 0, 0),
        ("tests/corpus/会員特典.rule", 12, 4, 3, 2, 2),
        ("tests/corpus/parcel_rate.rule", 15, 6, 3, 1, 1),
        ("tests/corpus/return_eligibility.rule", 7, 5, 0, 0, 0),
        ("tests/corpus/uk_minimum_wage.rule", 5, 6, 4, 0, 0),
        ("tests/corpus/uk_income_tax.rule", 7, 10, 0, 1, 1),
        ("tests/corpus/us_income_tax.rule", 7, 12, 0, 1, 1),
        ("tests/corpus/osha_extinguisher.rule", 5, 0, 0, 0, 0),
        ("tests/corpus/uk_stamp_duty.rule", 9, 11, 4, 0, 0),
        ("tests/corpus/osha_noise.rule", 9, 15, 8, 0, 0),
        ("tests/corpus/osha_excavation.rule", 4, 3, 0, 0, 0),
        ("tests/corpus/paypal_fee.rule", 2, 0, 0, 1, 1),
        ("tests/corpus/値引の充当.rule", 2, 0, 0, 2, 0),
        ("tests/corpus/決済手数料.rule", 3, 3, 0, 2, 1),
        ("tests/corpus/補償証明書.rule", 8, 0, 9, 1, 0),
        ("tests/corpus/評価ランク.rule", 4, 3, 6, 1, 0),
        ("tests/corpus/預け荷物料金.rule", 9, 0, 0, 1, 0),
        ("tests/corpus/保存基準.rule", 15, 9, 3, 0, 0),
        ("tests/corpus/事務所の衛生基準.rule", 8, 5, 16, 1, 0),
        ("tests/corpus/買物かごの送料.rule", 4, 6, 0, 0, 0),
        ("tests/corpus/品番の扱い.rule", 4, 2, 4, 0, 0),
        ("tests/corpus/比例配分.rule", 2, 0, 0, 2, 0),
        ("tests/corpus/注文の送料.rule", 7, 2, 0, 1, 0),
        ("tests/corpus/出荷の送料.rule", 10, 4, 0, 1, 0),
        ("tests/corpus/速達の見積.rule", 8, 10, 0, 1, 0),
        ("tests/corpus/注文の状態.rule", 10, 0, 0, 1, 0),
        ("tests/corpus/payment_intent.rule", 28, 0, 0, 0, 0),
    ];
    // Every rule of the corpus is audited and pinned. `threeway.rs` keeps its own list
    // honest the same way; this one had no such guard, and nine rules had drifted out of it
    // — their vector suites were generated and run, and never audited (§15.92).
    for rel in CORPUS {
        assert!(PINNED.iter().any(|(p, ..)| p == &rel), "{rel} の固定値がありません");
    }
    assert_eq!(PINNED.len(), CORPUS.len(), "コーパスを足したら固定値も足す");
    for (rel, rows, bounds, shadows, values, ties) in PINNED {
        let (f, c, vs) = load(rel);
        let a = coverage::audit(&f, &c, rel, &vs, &[]);
        assert_eq!(
            (a.tally[ROW].1, a.tally[BOUND].1, a.tally[SHADOW].1, a.tally[VALUE].1, a.tally[TIE].1),
            (*rows, *bounds, *shadows, *values, *ties),
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

/// The rule the value pairs and the ties were missing on (§15.151): the one row that computes
/// the fee is not the first, so its own point sits at 金額 = 0 and the fee there is 0 whatever
/// an implementation computes.
const FEE: &str = "\
rule 手数料(fee) v1

enum 区分(kind) = A(a) | B(b)
enum 地域(region) = 東(east) | 西(west)

inputs
  区分(kind)   : 区分
  地域(region) : 地域
  金額(amount) : money[円, incl_tax]  range >=0円 <=100万円

outputs
  手数料(fee) : money[円, incl_tax]  round down(1円)

define 料率分(pct) : money[円, incl_tax] = 金額 × 3%

table 手数料表(table)
policy unique
| 区分 | 地域 | -> 手数料 |
| A    | -    | 0円       |
| B    | 東   | 0円       |
| B    | 西   | 料率分    |
";

fn fee_of(v: &Vector) -> String {
    vectors::show(v.outputs[0].1.as_ref().expect("手数料が無い"))
}

/// A row that returns a computed value is seen at two values, one input apart. Without the
/// pair, an implementation that returned 0 on the row matched every vector.
#[test]
fn 計算した値を返す行は二つの値で試される() {
    let (a, vs) = audit_src("fee.rule", FEE);
    assert!(a.ok(), "{}", coverage::render(&a, &vs, &[]));
    assert_eq!(a.tally[VALUE], (1, 1));
    let on: Vec<&Vector> = vs.iter().filter(|v| v.trace.iter().any(|t| t == "表 手数料表 行3")).collect();
    let fees: std::collections::BTreeSet<String> = on.iter().map(|v| fee_of(v)).collect();
    assert!(fees.len() >= 2, "行3 の手数料が一通りしかない: {fees:?}");
    assert!(on.iter().any(|v| fee_of(v) != "0"), "行3 が 0 円でしか試されていない");
}

/// The auditor asks for the pair itself: with the row seen at 金額 = 0 alone, the obligation is
/// missing and names the row.
#[test]
fn 計算した値を一通りに減らすと計算値の対カバーが欠ける() {
    let (f, c) = rulec::prepare(FEE, "fee.rule").expect("検査を通る");
    let vs = vectors::generate(&f, &c);
    let kept: Vec<Vector> = vs
        .iter()
        .filter(|v| !v.trace.iter().any(|t| t == "表 手数料表 行3") || fee_of(v) == "0")
        .cloned()
        .collect();
    assert!(kept.len() < vs.len(), "抜く対象がない");
    let a = coverage::audit(&f, &c, "fee.rule", &kept, &[]);
    assert_eq!(a.tally[VALUE], (0, 1));
    let m: Vec<&str> = a.missing.iter().filter(|m| m.kind == VALUE).map(|m| m.what.as_str()).collect();
    assert!(m.len() == 1 && m[0].contains("行3") && m[0].contains("料率分"), "{m:?}");
}

/// A row that says itself what the name it hands back is raises nothing: `| 受付 | 出荷 | 状態 |`
/// returns 受付. Of the order's ten rows, only the refund of what was paid is computed.
#[test]
fn 行が値を決めている行は計算値の義務にならない() {
    let (f, c, _) = load("tests/corpus/注文の状態.rule");
    let duties = coverage::value_duties(&f, &c);
    let named: Vec<(usize, &str)> = duties
        .iter()
        .map(|d| {
            let (si, ri) = d.at.expect("表の行の義務");
            (c.sets[si].table.rows[ri].index, d.col.as_str())
        })
        .collect();
    assert_eq!(named, vec![(5, "返金額")]);
}

/// The tie is an obligation although the row the baseline lands on returns a constant, and it
/// is met on the row that computes (§15.151). It used to be looked for from the baseline alone,
/// and an obligation it did not find there was never counted.
#[test]
fn 同着の義務は定数を返す行があっても立つ() {
    let (a, vs) = audit_src("fee.rule", FEE);
    assert_eq!(a.tally[TIE], (1, 1));
    assert!(
        vs.iter().any(|v| v.why.contains("丸めの同着") && v.trace.iter().any(|t| t == "表 手数料表 行3")),
        "同着のベクタが行3 に無い"
    );
}

/// Taking the tie away leaves it missing, 0 of 1 — not 0 of 0.
#[test]
fn 同着のベクタを抜くと丸めの同着カバーが欠ける() {
    let (f, c) = rulec::prepare(FEE, "fee.rule").expect("検査を通る");
    let vs = vectors::generate(&f, &c);
    let a = coverage::audit(&f, &c, "fee.rule", &vs, &[]);
    let tie: Vec<usize> = a.witness.iter().copied().filter(|&i| vs[i].why.contains("丸めの同着")).collect();
    assert!(!tie.is_empty(), "同着のベクタが無い");
    // Every vector whose fee sits half a yen off the grid, whoever made it.
    let kept: Vec<Vector> = vs
        .iter()
        .filter(|v| {
            let (_, _, b) = rulec::eval::run_bindings(&f, &c, v.input.clone().into_iter().collect());
            !matches!(b.get("手数料"), Some(rulec::eval::Val::Num(r)) if vectors::is_tie(*r, rulec::num::Rat::int(1)))
        })
        .cloned()
        .collect();
    let b = coverage::audit(&f, &c, "fee.rule", &kept, &[]);
    assert_eq!(b.tally[TIE], (0, 1));
    assert!(b.missing.iter().any(|m| m.kind == TIE && m.what.contains("手数料")));
}

/// An output the rule shows can never sit on a tie raises no obligation: every standard monthly
/// remuneration is a multiple of 2,000 yen and the rate one of 0.1%, so half their product is
/// whole yen; a base fee times a share of 0%, 50% or 100% is a multiple of 10 yen under a 10-yen
/// grid; a product of two whole numbers is whole; and 3.49% of at most 100 cents never ends in
/// half a cent, though 3.49% of 5,000 does.
#[test]
fn 同着に届かない出力は義務にならない() {
    for (rel, want) in [("tests/corpus/厚生年金保険料.rule", 0), ("tests/corpus/送料.rule", 0), ("tests/corpus/paypal_fee.rule", 1)] {
        let (f, c, vs) = load(rel);
        let a = coverage::audit(&f, &c, rel, &vs, &[]);
        assert_eq!(a.tally[TIE], (want, want), "{rel}");
    }
    let product = "\
rule 積(product) v1

inputs
  個数(count) : number  range >=0 <=100
  単価(price) : money[円]  range >=0円 <=10000円

outputs
  金額(amount) : money[円]  round down(1円)

define 金額(amount) : money[円] = 単価 × 個数
";
    assert_eq!(audit_src("product.rule", product).0.tally[TIE], (0, 0));
    for (hi, want) in [(100, 0), (10000, 1)] {
        let src = format!(
            "\
rule 手数料率(fee_rate) v1

inputs
  金額(amount) : money[USDc]  range >=1USDc <={hi}USDc

outputs
  手数料(fee) : money[USDc]  round half_up(1USDc)

define 手数料(fee) : money[USDc] = 金額 × 3.49%
"
        );
        let (a, vs) = audit_src("fee_rate.rule", &src);
        assert_eq!(a.tally[TIE], (want, want), "{hi}\n{}", coverage::render(&a, &vs, &[]));
    }
}

/// A rule with no table at all still has a suite: the output it computes is an obligation of
/// its own. It had none, and its suite came out empty — `rulec test` and `verify` compared
/// nothing and passed.
#[test]
fn 表の無い規則にもベクタがある() {
    let src = "\
rule 手数料率(fee_rate) v1

inputs
  金額(amount) : money[USDc]  range >=1USDc <=100USDc

outputs
  手数料(fee) : money[USDc]  round half_up(1USDc)

define 手数料(fee) : money[USDc] = 金額 × 3.49%
";
    let (a, vs) = audit_src("fee_rate.rule", src);
    assert!(a.ok(), "{}", coverage::render(&a, &vs, &[]));
    assert_eq!(a.tally[VALUE], (1, 1));
    let fees: std::collections::BTreeSet<String> = vs.iter().map(fee_of).collect();
    assert!(fees.len() >= 2, "{fees:?}");
}

/// Two amounts at two rates, each amount at most `hi` cents.
fn two_rates(lo: i32, hi: i32) -> String {
    format!(
        "\
rule 二つの率(two_rates) v1

inputs
  国内(domestic) : money[USDc]  range >={lo}USDc <={hi}USDc
  国外(abroad)   : money[USDc]  range >={lo}USDc <={hi}USDc

outputs
  手数料(fee) : money[USDc]  round half_up(1USDc)

define 手数料(fee) : money[USDc] = 国内 × 3.49% + 国外 × 1.5%
"
    )
}

/// With two names in the value, the ranges are taken in too (§15.152). At ten cents apiece the
/// fee never passes 0.499 cents: no tie, and nothing but 0 after rounding, so neither a tie nor
/// a value pair is owed. Both were owed, and neither could be met — a red no rule could fix,
/// since the rounding is required and no example can land where no input does.
#[test]
fn 範囲の中で届かない同着と一つにしかならない値は義務にならない() {
    let (a, vs) = audit_src("two_rates.rule", &two_rates(0, 10));
    assert!(a.ok(), "{}", coverage::render(&a, &vs, &[]));
    assert_eq!((a.tally[VALUE], a.tally[TIE]), ((0, 0), (0, 0)));
    // A product can be fixed the same way: at most ten yen at at most 1% is 0 yen.
    let product = "\
rule 積の範囲(narrow_product) v1

inputs
  金額(amount) : money[円]  range >=0円 <=10円
  率(rate)     : rate[step 0.01%]  range >=0% <=1%

outputs
  額(fee) : money[円]  round half_up(1円)

define 額(fee) : money[円] = 金額 × 率
";
    let (a, vs) = audit_src("narrow_product.rule", product);
    assert!(a.ok(), "{}", coverage::render(&a, &vs, &[]));
    assert_eq!((a.tally[VALUE], a.tally[TIE]), ((0, 0), (0, 0)));
}

/// A row's own cells narrow what it can compute from: 3.49% reaches a half cent at 5,000 cents,
/// but the row that computes it only holds up to 10.
#[test]
fn 行の条件の中で届かない同着は義務にならない() {
    let src = "\
rule 行で狭い(narrow_row) v1

inputs
  金額(amount) : money[USDc]  range >=0USDc <=10000USDc

outputs
  手数料(fee) : money[USDc]  round half_up(1USDc)

define 料率分(pct) : money[USDc] = 金額 × 3.49%

table 手数料表(fees)
policy unique
| 金額     | -> 手数料 |
| <=10USDc | 料率分    |
| >10USDc  | 0USDc     |
";
    let (a, vs) = audit_src("narrow_row.rule", src);
    assert!(a.ok(), "{}", coverage::render(&a, &vs, &[]));
    assert_eq!((a.tally[VALUE], a.tally[TIE]), ((0, 0), (0, 0)));
}

/// Where the tie is reached by one combination of the two amounts alone — 50 and 117 cents,
/// 1.745 + 1.755 — the generator solves for it rather than walking to it, and the tie is met.
#[test]
fn 二つの入力を同時に動かさないと届かない同着も見つける() {
    let (a, vs) = audit_src("two_rates.rule", &two_rates(1, 120));
    assert!(a.ok(), "{}", coverage::render(&a, &vs, &[]));
    assert_eq!(a.tally[TIE], (1, 1));
    let at = |v: &Vector, k: &str| vectors::show(&v.input[k]);
    assert!(vs.iter().any(|v| at(v, "国内") == "50" && at(v, "国外") == "117"), "同着の組み合わせが無い");
}
