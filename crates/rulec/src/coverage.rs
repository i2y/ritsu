//! Completeness audit of the vector suite (§9.2).
//!
//! The three coverage criteria are derived **from the rule first**, not from the generated
//! vector set. The list of obligations is built independently of the generator, so that an
//! obligation the candidate population failed to reach cannot be written off as "never
//! needed in the first place". The point of the separation is the one-way relation: when the
//! generator changes, the auditor turns red.

use crate::ast::*;
use crate::eval::{self, Val};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use crate::vectors::{self, Vector};
use std::collections::{BTreeMap, BTreeSet};

pub const ROW: &str = "行被覆";
pub const BOUND: &str = "境界両側被覆";
pub const SHADOW: &str = "遮蔽対被覆";

/// The name of a criterion in the output language. The constants above stay Japanese: they
/// are the keys of `Audit::tally` and `Missing::kind`, and the tests compare against them.
fn label(k: &'static str) -> &'static str {
    if crate::i18n::ja() {
        k
    } else {
        match k {
            ROW => "row coverage",
            BOUND => "boundary-pair coverage",
            SHADOW => "shadow-pair coverage",
            other => other,
        }
    }
}

pub struct Missing {
    pub kind: &'static str,
    pub what: String,
    pub hint: String,
}

pub struct Audit {
    /// Per criterion: (satisfied, required).
    pub tally: BTreeMap<&'static str, (usize, usize)>,
    pub missing: Vec<Missing>,
    /// Indices of the vectors that actually discharged an obligation. Their union is the
    /// subset that satisfies §9.2; a pair obligation admits both of its vectors together, which
    /// greedy scoring would drop.
    pub witness: BTreeSet<usize>,
    /// Boundary obligations dropped as unrealizable (§9.1, "the side whose solution falls
    /// outside the input range"). One of the §9.2 safety nets checks that adding this back
    /// matches the count of a naive collector; an enumerator that overlooks a new kind of
    /// column shows up as the difference.
    pub pruned_bounds: usize,
}

impl Audit {
    pub fn ok(&self) -> bool {
        self.missing.is_empty()
    }
}

/// The smallest step of a number or date. Only a rate takes it from the column's storage scale.
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
    matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date)
}

/// Whether every cell of the row except the `skip` column holds under these bindings.
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

/// Whether two assignments differ in exactly one input (the "vector pair" of §9.2).
fn differ_in_one(a: &BTreeMap<String, Val>, b: &BTreeMap<String, Val>, col: &str) -> bool {
    let mut diff = 0;
    let derived = !a.contains_key(col);
    for (k, v) in a {
        if b.get(k) != Some(v) {
            diff += 1;
            // For an input column, the column that moves must be that column itself. A derived
            // column is not an input, so any single moving input will do (the mapping of §9.1).
            if !derived && k != col {
                return false;
            }
        }
    }
    diff == 1 && a.len() == b.len()
}

/// Expand the boundaries of one cell into (threshold, inside, outside) triples.
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
        // A point literal is two boundaries as well; a mistyped literal dies on this pair.
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

pub fn show_rat(v: Rat, ty: &Ty) -> String {
    match ty {
        Ty::Date => {
            let (y, m, d) = crate::types::ord_to_date(v);
            format!("{y:04}-{m:02}-{d:02}")
        }
        // A rate is stored as a fraction and written as a percentage, and the two look
        // nothing alike: the boundary `0.5%` would otherwise be named `0.005`.
        Ty::Rate => format!("{}%", v.mul(Rat::int(100))),
        _ => format!("{v}"),
    }
}

/// Judge whether a vector set satisfies the three criteria of §9.2.
pub fn audit(f: &RuleFile, c: &Checked, path: &str, vs: &[Vector]) -> Audit {
    let checks = crate::table_checks(f, c, path);
    // Re-run every vector to get bindings that include the derived values and definitions.
    let binds: Vec<BTreeMap<String, Val>> = vs
        .iter()
        .map(|v| {
            let (_, _, b) = eval::run_bindings(f, c, v.input.clone().into_iter().collect());
            b.into_iter().collect()
        })
        .collect();
    let fired: Vec<BTreeSet<String>> = vs.iter().map(|v| v.trace.iter().cloned().collect()).collect();

    // All three criteria are always reported. If a criterion with no obligations were left
    // out, the tallying side could not tell that apart from "the criterion was not checked".
    let mut tally: BTreeMap<&'static str, (usize, usize)> =
        [ROW, BOUND, SHADOW].into_iter().map(|k| (k, (0, 0))).collect();
    let mut missing: Vec<Missing> = Vec::new();
    let mut witness: BTreeSet<usize> = BTreeSet::new();
    let mut pruned_bounds = 0usize;
    let bump = |k: &'static str, met: bool, t: &mut BTreeMap<&'static str, (usize, usize)>| {
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

        // --- row coverage
        for (ri, _) in t.rows.iter().enumerate() {
            if dead.contains(&ri) {
                continue; // a row E102 already reports as never matching
            }
            let tag = eval::row_tag(&tname, ri + 1);
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
                    hint: tr!(
                        "この行が勝つ入力をベクタが一つも作れていません。",
                        "No vector produces an input on which this row wins."
                    ),
                });
            }
        }

        // --- boundary-pair coverage
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
                    // §9.1: an unrealizable side is not an obligation.
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
                    // It must be a pair: unless the two points cross the boundary with the other
                    // columns held fixed, they cannot show that ±1 at the boundary changes the
                    // expected value.
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
                            tr!("内側", "the inside point")
                        } else if outs.is_empty() {
                            tr!("外側", "the outside point")
                        } else {
                            tr!(
                                "対（他列が揃っていない）",
                                "the pair (the other columns are not held equal)"
                            )
                        };
                        missing.push(Missing {
                            kind: BOUND,
                            what: tr!(
                                "表 {tname} 行{} 列 {col} の境界 {}（{} / {} を踏む対）",
                                "table {tname} row {} column {col} boundary {} (pair at {} / {})",
                                ri + 1,
                                show_rat(b, &ty),
                                show_rat(inside, &ty),
                                show_rat(outside, &ty)
                            ),
                            hint: tr!("欠けているのは {side} です。", "Missing: {side}."),
                        });
                    }
                }
            }
        }

        // --- shadow-pair coverage
        for &(i, j) in chk.map(|k| k.overlaps.as_slice()).unwrap_or(&[]) {
            let tag = eval::row_tag(&tname, i + 1);
            // The inside of the intersection: a point where row j's conditions hold too, yet row
            // i wins.
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
                    what: tr!(
                        "表 {tname} 行{} ∩ 行{}（行{} が勝つ点）",
                        "table {tname} row {} ∩ row {} (a point where row {} wins)",
                        i + 1,
                        j + 1,
                        i + 1
                    ),
                    hint: tr!(
                        "この点が無いと、隣接行を入れ替えても期待値が変わりません。",
                        "Without this point, swapping the adjacent rows changes no expected value."
                    ),
                });
            }
        }
    }
    Audit { tally, missing, witness, pruned_bounds }
}

pub fn render(a: &Audit, vs: &[Vector]) -> String {
    let mut o = tr!("ベクタ {} 件\n", "{} vectors\n", vs.len());
    // The Japanese column is 12 characters wide (the tests pin that output); the English
    // labels are longer, so the column widens to the longest of them.
    let w = if crate::i18n::ja() { 12 } else { 22 };
    for k in [ROW, BOUND, SHADOW] {
        let (met, req) = a.tally.get(k).copied().unwrap_or((0, 0));
        let mark = if met == req { tr!("満たす", "satisfied") } else { tr!("欠け", "missing") };
        let k = label(k);
        o.push_str(&format!("  {k:<w$} {met:>4} / {req:<4}  {mark}\n"));
    }
    if a.missing.is_empty() {
        return o;
    }
    o.push_str(&tr!(
        "\n満たせなかった義務 {} 件:\n",
        "\nunsatisfied obligations: {}\n",
        a.missing.len()
    ));
    for m in &a.missing {
        o.push_str(&format!("  [{}] {}\n    {}\n", label(m.kind), m.what, m.hint));
    }
    o
}

/// The body of `rulec coverage`: audit the generated vectors as they are.
pub fn audit_file(f: &RuleFile, c: &Checked, path: &str) -> (Audit, Vec<Vector>) {
    let vs = vectors::generate(f, c);
    let a = audit(f, c, path, &vs);
    (a, vs)
}

/// `--format json` (docs/formats.md). One object per rule file.
pub fn render_json(a: &Audit, vs: &[Vector], path: &str) -> String {
    let criteria: Vec<String> = [ROW, BOUND, SHADOW]
        .iter()
        .map(|k| {
            let (met, req) = a.tally.get(k).copied().unwrap_or((0, 0));
            let missing: Vec<String> = a
                .missing
                .iter()
                .filter(|m| m.kind == *k)
                .map(|m| crate::json::Obj::new().str("what", &m.what).str("hint", &m.hint).finish())
                .collect();
            crate::json::Obj::new()
                .str("name", json_name(k))
                .int("satisfied", met as i128)
                .int("total", req as i128)
                .raw("missing", crate::json::arr(&missing))
                .finish()
        })
        .collect();
    crate::json::Obj::new()
        .str("file", path)
        .int("vectors", vs.len() as i128)
        .raw("criteria", crate::json::arr(&criteria))
        .finish()
}

/// The criterion's name in JSON. Language independent, unlike `label`.
fn json_name(k: &str) -> &'static str {
    match k {
        ROW => "row",
        BOUND => "boundary_pair",
        _ => "shadow_pair",
    }
}
