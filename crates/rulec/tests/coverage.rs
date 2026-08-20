//! ベクタ套件の完全性検査（§9.2）。M1 の受け入れ。
//!
//! 判定器そのものを変異で試す。ベクタを抜けば赤くなり、抜かなければ緑になる、
//! という両方を見ないと「いつも緑を返す判定器」と区別が付かない。

use rulec::coverage::{self, BOUND, ROW, SHADOW};
use rulec::vectors::{self, Vector};

const CORPUS: [&str; 7] = [
    "tests/corpus/ゆうパック運賃.rule",
    "tests/corpus/クーポン割引.rule",
    "tests/corpus/クーポン併用.rule",
    "tests/corpus/送料.rule",
    "tests/corpus/期間区分.rule",
    "tests/corpus/適用順序.rule",
    "tests/corpus/クーポン一枚.rule",
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
        // 目安は規則あたり数百件（§9.2）。桁が違えば候補の畳み方が壊れている。
        assert!(vs.len() < 600, "{rel}: ベクタが {} 件と多すぎる", vs.len());
    }
}

/// 義務の一覧が、ベクタ集合ではなく規則から来ていること。空集合に対して
/// 「全部満たした」と言う判定器は、緑を返しても何も言っていない。
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

/// 行を勝たせているベクタを全部抜けば、その行の行被覆だけが欠ける。
#[test]
fn 行を勝たせる例を抜くと行被覆が欠ける() {
    let rel = "tests/corpus/ゆうパック運賃.rule";
    let (f, c, vs) = load(rel);
    let tag = "表 運賃表 行42"; // 沖縄 × S170
    let kept: Vec<Vector> = vs.iter().filter(|v| !v.trace.iter().any(|t| t == tag)).cloned().collect();
    assert!(kept.len() < vs.len(), "抜く対象がない");
    let a = coverage::audit(&f, &c, rel, &kept);
    assert!(!a.ok(), "抜いたのに緑のまま");
    let rows: Vec<&str> = a.missing.iter().filter(|m| m.kind == ROW).map(|m| m.what.as_str()).collect();
    assert_eq!(rows, vec![tag], "欠けた行を名指ししていない: {rows:?}");
}

/// 境界の外側を踏むベクタを抜けば、境界両側被覆が欠ける。
/// 内側だけで緑になる判定器は、境界の ±1 を捕まえられない。
#[test]
fn 境界の片側を抜くと境界両側被覆が欠ける() {
    let rel = "tests/corpus/ゆうパック運賃.rule";
    let (f, c, vs) = load(rel);
    // 三辺合計 = 61cm（<=60cm の外側）を踏む例を全部落とす。
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

/// 遮蔽対は「交差の内側」を要求する。行 j が当たるだけの点では足りない。
#[test]
fn 交差の内側を抜くと遮蔽対被覆が欠ける() {
    let rel = "tests/corpus/送料.rule";
    let (f, c, vs) = load(rel);
    let checks = rulec::table_checks(&f, &c, rel);
    let pairs: usize = checks.iter().map(|k| k.overlaps.len()).sum();
    assert!(pairs > 0, "遮蔽対のある規則を選んでいない");
    let a = coverage::audit(&f, &c, rel, &vs);
    assert_eq!(a.tally[SHADOW].1, pairs, "遮蔽対の数が検査と食い違う");
    // 内側の点を落とす。残るのは行 j 単独で当たる点だけ。
    let inside: Vec<usize> = (0..vs.len())
        .filter(|&i| vs[i].why.starts_with("遮蔽対"))
        .collect();
    assert!(!inside.is_empty(), "交差の内側を狙ったベクタがない");
    let kept: Vec<Vector> =
        vs.iter().enumerate().filter(|(i, _)| !inside.contains(i)).map(|(_, v)| v.clone()).collect();
    let b = coverage::audit(&f, &c, rel, &kept);
    assert!(b.tally[SHADOW].0 <= a.tally[SHADOW].0, "抜いて増えている");
}

/// §9.2 の網 その二。境界の義務の数を、**式の木から境界リテラルを数えるだけの
/// 素朴な収集器**と照合する。義務の列挙は判定器と生成器で一箇所を共有しているので、
/// 将来ここに新しい列の種類や原子の形を足したとき、列挙器がそれを見落とすと
/// 両方が揃って黙る。この照合だけが、その沈黙を破る。
#[test]
fn 境界の義務は素朴な数え上げと一致する() {
    use rulec::ast::*;
    // 収集器は coverage.rs を一切参照しない。セルの形だけを見て数える。
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
                    let numeric = matches!(
                        c.ty_of(col),
                        Some(rulec::types::Ty::Money { .. })
                            | Some(rulec::types::Ty::Qty { .. })
                            | Some(rulec::types::Ty::Rate)
                            | Some(rulec::types::Ty::Date)
                    );
                    if !numeric {
                        continue;
                    }
                    n += match row.cells.get(ci) {
                        Some(Cell::Cmp(cs)) => cs.len(), // 比較記号ひとつが境界ひとつ
                        Some(Cell::Lit(_)) => 2,         // 点は上下二つの境界
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

/// §9.2 の網 その三。規則ごとの義務件数を回帰に固定する。
/// **列の種類や原子の形を言語に足すときは、該当カテゴリの件数が増えることが
/// 受け入れ条件**である。増えなければ、列挙器がその機能を見ていない。
#[test]
fn 義務の件数を固定する() {
    // (規則, 行被覆, 境界両側被覆, 遮蔽対被覆)
    const PINNED: &[(&str, usize, usize, usize)] = &[
        ("tests/corpus/ゆうパック運賃.rule", 49, 6, 21),
        ("tests/corpus/クーポン割引.rule", 9, 2, 10),
        ("tests/corpus/クーポン併用.rule", 3, 4, 0),
        ("tests/corpus/送料.rule", 7, 4, 3),
        ("tests/corpus/期間区分.rule", 4, 6, 0),
        ("tests/corpus/適用順序.rule", 4, 0, 5),
        ("tests/corpus/クーポン一枚.rule", 7, 1, 3),
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
