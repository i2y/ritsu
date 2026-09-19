//! Completeness audit of the vector suite (§9.2).
//!
//! The five coverage criteria are derived **from the rule first**, not from the generated
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

pub const ROW: &str = "行カバー";
pub const BOUND: &str = "境界の両側カバー";
pub const SHADOW: &str = "隠れ対カバー";
pub const TIE: &str = "丸めの同着カバー";
/// The transitions of a `fold` (§15.56): nothing, one element on each verdict, and every
/// ordered pair of verdicts. The length of a sequence is not what has to be covered — the
/// walk is an automaton, and what it can do is decided by which verdict follows which.
pub const FOLD: &str = "畳み込みの遷移カバー";

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
            TIE => "rounding-tie coverage",
            FOLD => "fold-transition coverage",
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

pub fn in_range(c: &Checked, col: &str, v: Rat) -> bool {
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

/// Judge whether a vector set satisfies the five criteria of §9.2.
/// `refused` are the cases the reference evaluator has no answer for (`vectors::Suite`). They
/// are part of the suite — `rulec test` holds the generated code to refusing them — so an
/// obligation only such a case can reach is met, not missing (§15.56).
pub fn audit(f: &RuleFile, c: &Checked, path: &str, vs: &[Vector], refused: &[Vector]) -> Audit {
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

    // All five criteria are always reported. If a criterion with no obligations were left
    // out, the tallying side could not tell that apart from "the criterion was not checked".
    let mut tally: BTreeMap<&'static str, (usize, usize)> =
        [ROW, BOUND, SHADOW, TIE, FOLD].into_iter().map(|k| (k, (0, 0))).collect();
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

    // One (merged) table per definition set, parallel to `checks` (DESIGN-draft §2.4).
    let tables: Vec<&Table> = c.sets.iter().map(|s| &s.table).collect();

    // --- the fold's transitions. What each element lands on is what the table wrote, so the
    // obligations are read off the verdict's own enum and the witnesses off the vectors.
    if let Some(fold) = &f.fold {
        let verdict_of = |v: &Vector| -> Vec<String> {
            let Some(Val::Seq(xs)) = v.input.get(&fold.over) else { return Vec::new() };
            xs.iter()
                .filter_map(|e| {
                    let mut m: std::collections::HashMap<String, Val> = v
                        .input
                        .iter()
                        .filter(|(k, _)| *k != &fold.over)
                        .map(|(k, x)| (k.clone(), x.clone()))
                        .collect();
                    for (k, x) in e {
                        m.insert(k.clone(), x.clone());
                    }
                    match eval::run_tables(f, c, m).3.get(&fold.verdict) {
                        Some(Val::Enum(s)) => Some(s.clone()),
                        _ => None,
                    }
                })
                .collect()
        };
        let seqs: Vec<Vec<String>> = vs.iter().map(verdict_of).collect();
        let has_seq: Vec<bool> = vs.iter().map(|v| matches!(v.input.get(&fold.over), Some(Val::Seq(_)))).collect();
        // The same reading of the refused cases. They carry no witness index: nothing prunes
        // them, so nothing has to be told to keep them.
        let refused_seqs: Vec<Vec<String>> = refused.iter().map(verdict_of).collect();
        let values: Vec<String> = c.out_values.get(&fold.verdict).cloned().unwrap_or_default();

        let mut want: Vec<(String, Vec<String>)> = vec![(tr!("要素ゼロ件", "no elements"), Vec::new())];
        for v in &values {
            want.push((tr!("判定 {v}", "verdict {v}"), vec![v.clone()]));
        }
        for a in &values {
            for b in &values {
                want.push((tr!("{a} のあと {b}", "{a} then {b}"), vec![a.clone(), b.clone()]));
            }
        }
        for (what, pattern) in want {
            let met = seqs
                .iter()
                .zip(&has_seq)
                .position(|(sq, ok)| *ok && sq == &pattern);
            match met {
                Some(k) => {
                    witness.insert(k);
                    bump(FOLD, true, &mut tally);
                }
                None if refused_seqs.iter().any(|sq| sq == &pattern) => {
                    bump(FOLD, true, &mut tally);
                }
                None => {
                    bump(FOLD, false, &mut tally);
                    missing.push(Missing {
                        kind: FOLD,
                        what,
                        hint: tr!(
                            "この並びを作るベクタがありません。畳み込みの振る舞いは、どの判定のあとにどの判定が来るかで決まります。",
                            "No vector produces this sequence. What the walk does is decided by which verdict follows which."
                        ),
                    });
                }
            }
        }
    }

    for (ti, t) in tables.iter().enumerate() {
        let set = &c.sets[ti];
        let chk = checks.get(ti);
        let dead: BTreeSet<usize> = chk.map(|k| k.dead.iter().copied().collect()).unwrap_or_default();

        // --- row coverage
        for (ri, _) in t.rows.iter().enumerate() {
            if dead.contains(&ri) {
                continue; // a row E102 already reports as never matching
            }
            let tag = eval::row_tag(set.row_table(ri), t.rows[ri].index);
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
                    // A row nothing can reach is named by E102 — including one only the
                    // tables above rule out — and E102 rows are left out of this tally, so what
                    // is left here is usually the generator not getting there. Usually, not
                    // always: reading the tables above is an under-approximation, so a dead row
                    // it could not prove dead still lands in this bucket.
                    hint: tr!(
                        "この行が勝つ入力をベクタが一つも作れていません。到達できない行は E102 が名指しするので、多くは生成器が届いていない側です。",
                        "No vector produces an input on which this row wins. A row nothing can reach is named by E102 instead, so this is usually the generator not getting there."
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
                                "表 {} 行{} 列 {col} の境界 {}（{} / {} を踏む対）",
                                "table {} row {} column {col} boundary {} (pair at {} / {})",
                                set.row_table(ri),
                                t.rows[ri].index,
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
            let tag = eval::row_tag(set.row_table(i), t.rows[i].index);
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
                    what: {
                        let (ti, tj) = (set.row_table(i), set.row_table(j));
                        let (ni, nj) = (t.rows[i].index, t.rows[j].index);
                        if ti == tj {
                            tr!("表 {ti} 行{ni} ∩ 行{nj}（行{ni} が勝つ点）", "table {ti} row {ni} ∩ row {nj} (a point where row {ni} wins)")
                        } else {
                            tr!("表 {ti} 行{ni} ∩ 表 {tj} 行{nj}（表 {ti} 行{ni} が勝つ点）", "table {ti} row {ni} ∩ table {tj} row {nj} (a point where table {ti} row {ni} wins)")
                        }
                    },
                    hint: tr!(
                        "この点が無いと、隣接行を入れ替えても期待値が変わりません。",
                        "Without this point, swapping the adjacent rows changes no expected value."
                    ),
                });
            }
        }
    }

    // The rounding tie (§9.2). `round` is mandatory on a numeric output (E104), and this is the
    // only criterion that looks at whether the declaration was ever exercised: at a tie the five
    // modes give different answers, anywhere else `half_up` and `half_down` agree. An output the
    // rule can never take off the grid raises no obligation, the way an unrealizable boundary
    // does not — `tie_plan` answers that, and the generator aims at the same targets.
    for (name, grid, _) in vectors::tie_plan(f, c) {
        let hit = binds
            .iter()
            .position(|b| b.get(&name).and_then(ord).is_some_and(|v| vectors::is_tie(v, grid)));
        bump(TIE, hit.is_some(), &mut tally);
        match hit {
            Some(i) => {
                witness.insert(i);
            }
            None => missing.push(Missing {
                kind: TIE,
                what: tr!("出力 {name} の丸めの同着", "the rounding tie of output {name}"),
                hint: tr!(
                    "同着に載る入力が無いと、`half_up` と `half_down` を入れ替えても期待値が変わりません。",
                    "Without an input that lands on the tie, swapping `half_up` for `half_down` changes no expected value."
                ),
            }),
        }
    }
    Audit { tally, missing, witness, pruned_bounds }
}

pub fn render(a: &Audit, vs: &[Vector], refused: &[Vector]) -> String {
    let mut o = tr!("ベクタ {} 件\n", "{} vectors\n", vs.len());
    if !refused.is_empty() {
        o.push_str(&tr!(
            "  うち断る入力 {} 件（答えではなく、断ることが期待値）\n",
            "  plus {} refused inputs (the expected answer is a refusal)\n",
            refused.len()
        ));
    }
    // Padded to display width, not character count: a Japanese label is drawn twice as wide
    // as an ASCII one, so counting characters leaves the column ragged on a terminal.
    let w: usize = if crate::i18n::ja() { 22 } else { 24 };
    for k in [ROW, BOUND, SHADOW, TIE, FOLD] {
        let (met, req) = a.tally.get(k).copied().unwrap_or((0, 0));
        let mark = if met == req { tr!("満たす", "satisfied") } else { tr!("欠け", "missing") };
        let k = label(k);
        let pad = " ".repeat(w.saturating_sub(crate::diag::width(k)));
        o.push_str(&format!("  {k}{pad} {met:>4} / {req:<4}  {mark}\n"));
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

/// The body of `rulec coverage`: audit the generated suite as it is — the cases with an
/// answer and the cases that are refused.
pub fn audit_file(f: &RuleFile, c: &Checked, path: &str) -> (Audit, Vec<Vector>, Vec<Vector>) {
    let s = vectors::suite(f, c);
    let a = audit(f, c, path, &s.vectors, &s.refused);
    (a, s.vectors, s.refused)
}

/// `--format json` (docs/formats.md). One object per rule file.
pub fn render_json(a: &Audit, vs: &[Vector], refused: &[Vector], path: &str) -> String {
    let criteria: Vec<String> = [ROW, BOUND, SHADOW, TIE, FOLD]
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
        .int("refused", refused.len() as i128)
        .raw("criteria", crate::json::arr(&criteria))
        .finish()
}

/// The criterion's name in JSON. Language independent, unlike `label`.
fn json_name(k: &str) -> &'static str {
    match k {
        ROW => "row",
        BOUND => "boundary_pair",
        TIE => "rounding_tie",
        FOLD => "fold_transition",
        _ => "shadow_pair",
    }
}
