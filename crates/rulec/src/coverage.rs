//! ベクタ套件の完全性検査（§9.2）。
//!
//! 三つの被覆基準を、生成されたベクタ集合からではなく**規則から先に**導く。
//! 母集団が踏めなかった義務を「そもそも必要なかった」ことにしないためで、
//! 義務の一覧は生成器と独立に作る。生成器が変われば判定器が赤くなる、という
//! 一方向の関係を保つのがこの分離の目的。

use crate::ast::*;
use crate::eval::{self, Val};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use crate::vectors::{self, Vector};
use std::collections::{BTreeMap, BTreeSet};

pub const ROW: &str = "行被覆";
pub const BOUND: &str = "境界両側被覆";
pub const SHADOW: &str = "遮蔽対被覆";

pub struct Missing {
    pub kind: &'static str,
    pub what: String,
    pub hint: String,
}

pub struct Audit {
    /// 基準ごとの (満たした, 要求された)。
    pub tally: BTreeMap<&'static str, (usize, usize)>,
    pub missing: Vec<Missing>,
    /// 義務を実際に片づけたベクタの番号。§9.2 を満たす部分集合はこの和集合で、
    /// 対の義務は二本まとめて入る。貪欲な点数付けでは対が落ちる。
    pub witness: BTreeSet<usize>,
    /// 実現不能として境界の義務から外した数（§9.1 の「解が入力範囲の外になる側」）。
    /// §9.2 の網の一つは、これを足し戻した数が素朴な収集器の数と一致することを
    /// 見る。列挙器が新しい列の種類を見落としたら、その差として出る。
    pub pruned_bounds: usize,
}

impl Audit {
    pub fn ok(&self) -> bool {
        self.missing.is_empty()
    }
}

/// 数値・日付の最小刻み。率だけは列の格納尺度で決まる。
pub fn quantum(c: &Checked, col: &str, ty: &Ty) -> Rat {
    match ty {
        Ty::Rate => Rat::new(1, *c.scales.get(col).unwrap_or(&100)),
        _ => Rat::int(1),
    }
}

fn ord(v: &Val) -> Option<Rat> {
    match v {
        Val::Num(r) => Some(*r),
        Val::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

fn is_numeric(ty: &Ty) -> bool {
    matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date)
}

/// 行 r のうち `skip` 列以外のセルが、この束縛で全部成り立つか。
fn others_hold(
    t: &Table,
    row: &Row,
    skip: Option<&str>,
    binds: &BTreeMap<String, Val>,
    c: &Checked,
) -> bool {
    t.inputs.iter().enumerate().all(|(ci, (col, _))| {
        if Some(col.as_str()) == skip {
            return true;
        }
        let Some(cell) = row.cells.get(ci) else { return true };
        if matches!(cell, Cell::DontCare) {
            return true;
        }
        let (Some(v), Some(ty)) = (binds.get(col), c.ty_of(col)) else { return false };
        eval::cell_matches(c, cell, v, &ty)
    })
}

/// 二つの割り当てが、ちょうど一つの入力でだけ違うか（§9.2 の「ベクタ対」）。
fn differ_in_one(a: &BTreeMap<String, Val>, b: &BTreeMap<String, Val>, col: &str) -> bool {
    let mut diff = 0;
    let derived = !a.contains_key(col);
    for (k, v) in a {
        if b.get(k) != Some(v) {
            diff += 1;
            // 入力の列なら動くのはその列自身。導出の列は入力ではないので、
            // どれか一つの入力が動いていればよい（§9.1 の写像）。
            if !derived && k != col {
                return false;
            }
        }
    }
    diff == 1 && a.len() == b.len()
}

/// 一つのセルが持つ境界を (敷居, 内側, 外側) に展開する。
pub fn thresholds_pub(cell: &Cell, ty: &Ty, q: Rat) -> Vec<(Rat, Rat, Rat)> {
    thresholds(cell, ty, q)
}

fn thresholds(cell: &Cell, ty: &Ty, q: Rat) -> Vec<(Rat, Rat, Rat)> {
    let lit = |l: &Lit| -> Option<Rat> {
        match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty),
            Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
            _ => None,
        }
    };
    let mut out = Vec::new();
    match cell {
        Cell::Cmp(cs) => {
            for (op, l) in cs {
                let Some(b) = lit(l) else { continue };
                let (inside, outside) = match op {
                    CmpOp::Le => (b, b.add(q)),
                    CmpOp::Lt => (b.sub(q), b),
                    CmpOp::Ge => (b, b.sub(q)),
                    CmpOp::Gt => (b.add(q), b),
                };
                out.push((b, inside, outside));
            }
        }
        // 点の指定も境界二つ。リテラルの打ち間違いはこの対で死ぬ。
        Cell::Lit(l) => {
            if let Some(b) = lit(l) {
                out.push((b, b, b.sub(q)));
                out.push((b, b, b.add(q)));
            }
        }
        _ => {}
    }
    out
}

fn in_range(c: &Checked, col: &str, v: Rat) -> bool {
    let Some((lo, hi)) = c.ranges.get(col) else { return true };
    lo.is_none_or(|l| v.cmp_to(l) != std::cmp::Ordering::Less)
        && hi.is_none_or(|h| v.cmp_to(h) != std::cmp::Ordering::Greater)
}

fn show_rat(v: Rat, ty: &Ty) -> String {
    if matches!(ty, Ty::Date) {
        let (y, m, d) = crate::types::ord_to_date(v);
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{v}")
    }
}

/// ベクタ集合が §9.2 の三基準を満たしているかを判定する。
pub fn audit(f: &RuleFile, c: &Checked, path: &str, vs: &[Vector]) -> Audit {
    let checks = crate::table_checks(f, c, path);
    // 各ベクタを走らせ直して、導出や定義まで含めた束縛を持つ。
    let binds: Vec<BTreeMap<String, Val>> = vs
        .iter()
        .map(|v| {
            let (_, _, b) = eval::run_bindings(f, c, v.input.clone().into_iter().collect());
            b.into_iter().collect()
        })
        .collect();
    let fired: Vec<BTreeSet<String>> = vs.iter().map(|v| v.trace.iter().cloned().collect()).collect();

    // 三基準は常に報告する。義務が一つも無い基準を「表に出さない」と、
    // 数え上げ側からは「その基準を見ていない」と区別が付かない。
    let mut tally: BTreeMap<&'static str, (usize, usize)> =
        [ROW, BOUND, SHADOW].into_iter().map(|k| (k, (0, 0))).collect();
    let mut missing: Vec<Missing> = Vec::new();
    let mut witness: BTreeSet<usize> = BTreeSet::new();
    let mut pruned_bounds = 0usize;
    let mut bump = |k: &'static str, met: bool, t: &mut BTreeMap<&'static str, (usize, usize)>| {
        let e = t.entry(k).or_insert((0, 0));
        e.1 += 1;
        if met {
            e.0 += 1;
        }
    };

    let tables: Vec<&Table> = f
        .items
        .iter()
        .filter_map(|i| if let Item::Table(t) = i { Some(t) } else { None })
        .collect();

    for (ti, t) in tables.iter().enumerate() {
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let chk = checks.get(ti);
        let dead: BTreeSet<usize> = chk.map(|k| k.dead.iter().copied().collect()).unwrap_or_default();

        // --- 行被覆
        for (ri, _) in t.rows.iter().enumerate() {
            if dead.contains(&ri) {
                continue; // E102 が既に「決して当たらない」と言っている行
            }
            let tag = format!("表 {tname} 行{}", ri + 1);
            let met = match fired.iter().position(|s| s.contains(&tag)) {
                Some(k) => {
                    witness.insert(k);
                    true
                }
                None => false,
            };
            bump(ROW, met, &mut tally);
            if !met {
                missing.push(Missing {
                    kind: ROW,
                    what: tag.clone(),
                    hint: "この行が勝つ入力をベクタが一つも作れていません。".into(),
                });
            }
        }

        // --- 境界両側被覆
        for (ri, row) in t.rows.iter().enumerate() {
            if dead.contains(&ri) {
                continue;
            }
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let Some(ty) = c.ty_of(col) else { continue };
                if !is_numeric(&ty) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let q = quantum(c, col, &ty);
                for (b, inside, outside) in thresholds(cell, &ty, q) {
                    // §9.1: 実現不能な側は義務にしない。
                    if !in_range(c, col, inside) || !in_range(c, col, outside) {
                        pruned_bounds += 1;
                        continue;
                    }
                    let hit = |target: Rat| -> Vec<usize> {
                        (0..vs.len())
                            .filter(|&k| {
                                binds[k].get(col).and_then(ord).is_some_and(|x| x.cmp_to(target) == std::cmp::Ordering::Equal)
                                    && others_hold(t, row, Some(col), &binds[k], c)
                            })
                            .collect()
                    };
                    let ins = hit(inside);
                    let outs = hit(outside);
                    // 対であること。他列を固定したまま境界をまたぐ二点でなければ、
                    // 境界の ±1 が期待値を変えることを示せない。
                    let pair = ins.iter().find_map(|&a| {
                        outs.iter().find(|&&z| differ_in_one(&vs[a].input, &vs[z].input, col)).map(|&z| (a, z))
                    });
                    if let Some((a, z)) = pair {
                        witness.insert(a);
                        witness.insert(z);
                    }
                    let met = pair.is_some();
                    bump(BOUND, met, &mut tally);
                    if !met {
                        let side = if ins.is_empty() {
                            "内側"
                        } else if outs.is_empty() {
                            "外側"
                        } else {
                            "対（他列が揃っていない）"
                        };
                        missing.push(Missing {
                            kind: BOUND,
                            what: format!(
                                "表 {tname} 行{} 列 {col} の境界 {}（{} / {} を踏む対）",
                                ri + 1,
                                show_rat(b, &ty),
                                show_rat(inside, &ty),
                                show_rat(outside, &ty)
                            ),
                            hint: format!("欠けているのは {side} です。"),
                        });
                    }
                }
            }
        }

        // --- 遮蔽対被覆
        for &(i, j) in chk.map(|k| k.overlaps.as_slice()).unwrap_or(&[]) {
            let tag = format!("表 {tname} 行{}", i + 1);
            // 交差の内側: 行 j の条件も成り立つのに、行 i が勝つ点。
            let met = match (0..vs.len())
                .find(|&k| fired[k].contains(&tag) && others_hold(t, &t.rows[j], None, &binds[k], c))
            {
                Some(k) => {
                    witness.insert(k);
                    true
                }
                None => false,
            };
            bump(SHADOW, met, &mut tally);
            if !met {
                missing.push(Missing {
                    kind: SHADOW,
                    what: format!("表 {tname} 行{} ∩ 行{}（行{} が勝つ点）", i + 1, j + 1, i + 1),
                    hint: "この点が無いと、隣接行を入れ替えても期待値が変わりません。".into(),
                });
            }
        }
    }
    Audit { tally, missing, witness, pruned_bounds }
}

pub fn render(a: &Audit, vs: &[Vector]) -> String {
    let mut o = format!("ベクタ {} 件\n", vs.len());
    for k in [ROW, BOUND, SHADOW] {
        let (met, req) = a.tally.get(k).copied().unwrap_or((0, 0));
        let mark = if met == req { "満たす" } else { "欠け" };
        o.push_str(&format!("  {k:<12} {met:>4} / {req:<4}  {mark}\n"));
    }
    if a.missing.is_empty() {
        return o;
    }
    o.push_str(&format!("\n満たせなかった義務 {} 件:\n", a.missing.len()));
    for m in &a.missing {
        o.push_str(&format!("  [{}] {}\n    {}\n", m.kind, m.what, m.hint));
    }
    o
}

/// `rulec coverage` の本体。生成したベクタをそのまま判定する。
pub fn audit_file(f: &RuleFile, c: &Checked, path: &str) -> (Audit, Vec<Vector>) {
    let vs = vectors::generate(f, c);
    let a = audit(f, c, path, &vs);
    (a, vs)
}
