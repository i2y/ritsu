//! Completeness audit of the vector suite (§9.2).
//!
//! The seven coverage criteria are derived **from the rule first**, not from the generated
//! vector set. The list of obligations is built independently of the generator, so that an
//! obligation the candidate population failed to reach cannot be written off as "never
//! needed in the first place". The point of the separation is the one-way relation: when the
//! generator changes, the auditor turns red.

use crate::ast::*;
use crate::eval::{self, Val};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use crate::vectors::{self, Vector};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const ROW: &str = "行カバー";
pub const BOUND: &str = "境界の両側カバー";
pub const SHADOW: &str = "隠れ対カバー";
/// A row that returns a computed value — a name in its output cell, not a literal — and an
/// output a `define` or a `result` line computes (§15.151). What has to be seen is the value
/// being computed there: two vectors that both land on the row, differ in exactly one input,
/// and get different values in that column. A cell replaced by a constant, or by a value that
/// ignores that input, gives one of the two a wrong answer. Row coverage alone is met by one
/// point, and the point the generator reaches a row with is the low end of every range — a
/// refund of the amount paid, tried only at 0 yen.
pub const VALUE: &str = "計算値の対カバー";
pub const TIE: &str = "丸めの同着カバー";
/// The transitions of a `fold` (§15.56): nothing, one element on each verdict, and every
/// ordered pair of verdicts. The length of a sequence is not what has to be covered — the
/// walk is an automaton, and what it can do is decided by which verdict follows which.
pub const FOLD: &str = "畳み込みの遷移カバー";
/// The transitions of a `machine` (§15.148): every transition a case can make, and every two
/// that can follow one another, played from the initial state. The pair is the obligation
/// because what a trace adds to the single calls is the hand-over of the state from one call
/// to the next, in the language under test.
pub const MACHINE: &str = "ステートマシンの遷移カバー";

/// Every criterion, in the order the report lists them.
pub const CRITERIA: [&str; 7] = [ROW, BOUND, SHADOW, VALUE, TIE, FOLD, MACHINE];

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
            VALUE => "value-pair coverage",
            TIE => "rounding-tie coverage",
            FOLD => "fold-transition coverage",
            MACHINE => "machine-transition coverage",
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

/// Whether no input reaches `col = v` with every other numeric cell of the row holding, the
/// declared ranges and the rule's `constraint`s kept (§15.140). Under `申告額 <= 補償額`, the
/// row `<=5万円 | <=5万円` has no input at 申告額 = 50001: the outside point of that boundary is
/// one the generated code refuses at the door, and no vector can stand on it. Proved by
/// elimination over the rationals, so a side some input does reach is never dropped. A rule
/// with no `constraint` has nothing here the ranges have not already said.
fn unreachable_side(f: &RuleFile, c: &Checked, t: &Table, row: &Row, col: &str, v: Rat) -> bool {
    if f.constraints.is_empty() {
        return false;
    }
    let mut seed = vec![col.to_string()];
    seed.extend(t.inputs.iter().map(|(n, _)| n.clone()).filter(|n| n != col && c.ty_of(n).is_some_and(|ty| is_numeric(&ty))));
    for g in crate::fourier::grounds(&seed, f, c) {
        if !g.vars.iter().any(|n| n == col) {
            continue;
        }
        let mut sys = g.sys.clone();
        let at = crate::fourier::Lin::var(col).plus(&crate::fourier::Lin::con(v.mul(Rat::int(-1))));
        sys.push(at.clone().le(false));
        sys.push(at.ge(false));
        for (ci, (other, _)) in t.inputs.iter().enumerate() {
            if other == col || !g.vars.contains(other) {
                continue;
            }
            let x = crate::fourier::Lin::var(other);
            let lit = |l: &Lit| crate::fourier::lit_of(l, &g.want);
            match row.cells.get(ci) {
                Some(Cell::Lit(l)) => {
                    if let Some(k) = lit(l) {
                        let d = x.clone().plus(&crate::fourier::Lin::con(k.mul(Rat::int(-1))));
                        sys.push(d.clone().le(false));
                        sys.push(d.ge(false));
                    }
                }
                Some(Cell::Cmp(ops)) => {
                    for (op, l) in ops {
                        if let Some(k) = lit(l) {
                            sys.push(crate::fourier::cmp(x.clone().plus(&crate::fourier::Lin::con(k.mul(Rat::int(-1)))), *op, false));
                        }
                    }
                }
                _ => {}
            }
        }
        if crate::fourier::unsat(sys) {
            return true;
        }
    }
    false
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
        // A set is its members, each a point of its own; its complement is the same points
        // with inside and outside the other way round.
        Cell::Set(ls) => {
            for b in ls.iter().filter_map(lit) {
                out.push((b, b, b.sub(q)));
                out.push((b, b, b.add(q)));
            }
        }
        Cell::Not(ls) => {
            for b in ls.iter().filter_map(lit) {
                out.push((b, b.sub(q), b));
                out.push((b, b.add(q), b));
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

/// Judge whether a vector set satisfies the seven criteria of §9.2.
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

    // All seven criteria are always reported. If a criterion with no obligations were left
    // out, the tallying side could not tell that apart from "the criterion was not checked".
    let mut tally: BTreeMap<&'static str, (usize, usize)> = CRITERIA.into_iter().map(|k| (k, (0, 0))).collect();
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

    // One (merged) table per definition set, parallel to `checks` (§15.66).
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

    // --- the machine's transitions (§15.148). The obligations are read off the rule's own
    // walk; whether one is met is read off the suite's traces, replayed call by call through
    // the reference evaluator — not off the list the generator worked from.
    if f.machine.is_some() {
        machine_obligations(f, c, &mut tally, &mut missing, &bump);
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
                    // §9.1: an unrealizable side is not an obligation. Nor is a side no input
                    // satisfying the rule's `constraint`s reaches with the row's other cells held.
                    if !in_range(c, col, inside)
                        || !in_range(c, col, outside)
                        || unreachable_side(f, c, t, row, col, inside)
                        || unreachable_side(f, c, t, row, col, outside)
                    {
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

    // --- value-pair coverage (§15.151). The obligations are the rule's own; a pair is two
    // vectors on the row, one input apart, with the column's value moved.
    for d in value_duties(f, c) {
        if let Some((si, ri)) = d.at {
            if checks.get(si).is_some_and(|k| k.dead.contains(&ri)) {
                continue; // a row E102 already reports as never matching
            }
        }
        let tag = d.at.map(|(si, ri)| eval::row_tag(c.sets[si].row_table(ri), c.sets[si].table.rows[ri].index));
        let at: Vec<usize> = (0..vs.len())
            .filter(|&k| tag.as_ref().is_none_or(|t| fired[k].contains(t)) && binds[k].contains_key(&d.col))
            .collect();
        let pair = at.iter().enumerate().find_map(|(i, &a)| {
            at[i + 1..]
                .iter()
                .find(|&&z| {
                    value_moved(f, &d.col, (binds[a].get(&d.col), &vs[a].outputs), (binds[z].get(&d.col), &vs[z].outputs))
                        && one_apart(&vs[a].input, &vs[z].input)
                })
                .map(|&z| (a, z))
        });
        if let Some((a, z)) = pair {
            witness.insert(a);
            witness.insert(z);
        }
        bump(VALUE, pair.is_some(), &mut tally);
        if pair.is_none() {
            missing.push(Missing {
                kind: VALUE,
                what: match d.at {
                    Some((si, ri)) => tr!(
                        "表 {} 行{} 列 {}（{} を返す）",
                        "table {} row {} column {} (returns {})",
                        c.sets[si].row_table(ri),
                        c.sets[si].table.rows[ri].index,
                        d.col,
                        d.name
                    ),
                    None => tr!("出力 {}", "output {}", d.col),
                },
                hint: match d.at {
                    Some(_) => tr!(
                        "この行に当たり、入力が一つだけ違い、この列の値も違う二つのベクタがありません。この値を定数に置き換えた実装でも、どのベクタの期待値も変わりません。",
                        "No two vectors land on this row, differ in one input and get different values in this column. An implementation that returned a constant here would still match every vector."
                    ),
                    None => tr!(
                        "入力が一つだけ違い、この出力の値も違う二つのベクタがありません。この出力を定数にした実装でも、どのベクタの期待値も変わりません。",
                        "No two vectors differ in one input and get different values of this output. An implementation that returned a constant would still match every vector."
                    ),
                },
            });
        }
    }

    // The rounding tie (§9.2). `round` is mandatory on a numeric output (E104), and this is the
    // only criterion that looks at whether the declaration was ever exercised: at a tie the five
    // modes give different answers, anywhere else `half_up` and `half_down` agree. The
    // obligations come from the rule, like every other criterion's: an output raises none only
    // where the rule shows that no input takes it half a step off the grid (`tie_duties`),
    // never because the generator did not get there (§15.151).
    for (name, grid) in tie_duties(f, c) {
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
                    "同着に載る入力が無いと、`half_up` と `half_down` を入れ替えても期待値が変わりません。載る入力が見つからず、載らないことも示せませんでした。載る入力を知っていれば、examples に一行足すと監査に加わります。",
                    "Without an input that lands on the tie, swapping `half_up` for `half_down` changes no expected value. No such input was found, and none was shown impossible. If you know one, a row in `examples` takes part in the audit."
                ),
            }),
        }
    }
    Audit { tally, missing, witness, pruned_bounds }
}

/// Whether two cases show the value of `col` moving, as an implementation is held to it: an
/// output compared after rounding, the way `test` and `verify` compare it — 3.49% of one cent
/// and of two are different numbers and the same 0 cents — and a column on the way to the
/// outputs moved together with at least one output, or the move is not seen.
pub fn value_moved(
    f: &RuleFile,
    col: &str,
    a: (Option<&Val>, &[(String, Option<Val>)]),
    b: (Option<&Val>, &[(String, Option<Val>)]),
) -> bool {
    let out = |outs: &[(String, Option<Val>)]| outs.iter().find(|(n, _)| n == col).map(|(_, v)| v.clone());
    if f.outputs.iter().any(|o| o.name.text == col) {
        out(a.1) != out(b.1)
    } else {
        a.0 != b.0 && a.1 != b.1
    }
}

/// Whether two inputs differ in exactly one name.
fn one_apart(a: &BTreeMap<String, Val>, b: &BTreeMap<String, Val>) -> bool {
    a.len() == b.len() && a.iter().filter(|(k, v)| b.get(*k) != Some(*v)).count() == 1
}

/// One obligation of `VALUE`: a row that writes the name `name` into output column `col`, or
/// an output the rule computes outside any table.
pub struct ValueDuty {
    /// The row: an index into `Checked::sets` and one into the rows of that set's merged
    /// table. `None` for an output a `define` or a `result` line computes, which is not on a
    /// row; the pair may then be anywhere.
    pub at: Option<(usize, usize)>,
    pub col: String,
    pub name: String,
}

/// The obligations of `VALUE`, read off the rule alone (§15.151).
///
/// A row whose output cell holds a literal returns a constant and raises none. Nor does a row
/// that itself pins down everything the name is computed from: `| 受付 | 出荷 | 状態 |` hands
/// the carried state back, and the same row says the state is 受付 there. Every other name is
/// an obligation, whether or not the generator reaches it (§9.2).
///
/// So is an output that a `define` or a `result` line computes from the inputs, with no table
/// in between: it is not on any row, and a rule that is nothing but `手数料 = 金額 × 3.49%`
/// raised no obligation at all — its suite came out empty, and every implementation matched it.
pub fn value_duties(f: &RuleFile, c: &Checked) -> Vec<ValueDuty> {
    let reads = reads_of(f);
    let mut out = Vec::new();
    // A value that is one value wherever it can be looked at raises none either (§15.152): an
    // output whose every possible value rounds to the same amount — 3.49% of at most ten cents
    // is 0 cents, whatever comes in — and a column that is one number wherever its row holds.
    // Shown by the value's range, row by row; what that cannot show stays an obligation.
    let rounding_of = |col: &str| -> Option<(crate::num::RoundMode, Rat)> {
        let od = f.outputs.iter().find(|o| o.name.text == col)?;
        let rd = od.rounding.as_ref()?;
        Some((crate::num::RoundMode::parse(&rd.mode)?, crate::types::lit_value_in_pub(&rd.grid, &c.ty_of(col)?)?))
    };
    let fixed = |forms: &[GridForm], rounding: Option<(crate::num::RoundMode, Rat)>| -> bool {
        let mut span: Option<(Rat, Rat)> = None;
        for g in forms {
            match g.interval() {
                None => return false,
                Some(None) => {}
                Some(Some((l, h))) => {
                    span = Some(match span {
                        Some((a, b)) => (min_rat(a, l), max_rat(b, h)),
                        None => (l, h),
                    })
                }
            }
        }
        let Some((l, h)) = span else { return true };
        match rounding {
            Some((m, g)) => l.round_to(m, g) == h.round_to(m, g),
            None => l == h,
        }
    };
    let output_fixed = |n: &str| grid_forms_of(f, c, n).is_some_and(|forms| fixed(&forms, rounding_of(n)));
    // Nothing moves an answer when every output is fixed.
    if !f.outputs.is_empty() && f.outputs.iter().all(|o| output_fixed(&o.name.text)) {
        return out;
    }
    for (si, set) in c.sets.iter().enumerate() {
        let t = &set.table;
        for (ri, row) in t.rows.iter().enumerate() {
            let pins: BTreeSet<&str> = t
                .inputs
                .iter()
                .zip(&row.cells)
                .filter(|(_, cell)| match cell {
                    Cell::Lit(_) | Cell::Nothing => true,
                    Cell::Set(xs) => xs.len() == 1,
                    _ => false,
                })
                .map(|((col, _), _)| col.as_str())
                .collect();
            for (oi, oc) in t.outputs.iter().enumerate() {
                let Some(OutCell::Name(n)) = row.outs.get(oi) else { continue };
                // A word that names nothing is a value of the column's enum, or true or false.
                if !bound(f, n) {
                    continue;
                }
                if pins.contains(n.as_str()) || leaves(&reads, n).iter().all(|l| pins.contains(l.as_str())) {
                    continue;
                }
                let bx = row_box(f, c, t, row);
                let at_row = grid_forms(f, c, &Expr::Name(n.clone(), row.span.clone()), 0, &mut 0).map(|mut forms| {
                    forms.iter_mut().for_each(|g| g.narrow(&bx));
                    forms
                });
                if at_row.is_some_and(|forms| fixed(&forms, rounding_of(&oc.name.text))) {
                    continue;
                }
                out.push(ValueDuty { at: Some((si, ri)), col: oc.name.text.clone(), name: n.clone() });
            }
        }
    }
    // A walk's outputs are what the fold answers, not a computation of this kind.
    if f.fold.is_some() {
        return out;
    }
    for (oi, od) in f.outputs.iter().enumerate() {
        let n = &od.name.text;
        let by_result = oi == 0 && f.result.is_some();
        let by_define = f.items.iter().any(|it| matches!(it, Item::Define(d) if &d.name.text == n));
        let on_table = f.items.iter().any(|it| matches!(it, Item::Table(t) if t.outputs.iter().any(|o| &o.name.text == n)));
        if !(by_result || (by_define && !on_table)) {
            continue;
        }
        let from: BTreeSet<String> = match (&f.result, by_result) {
            (Some(r), true) => {
                let mut ns = BTreeSet::new();
                expr_names(&r.expr, &mut ns);
                ns.iter().flat_map(|m| leaves(&reads, m)).collect()
            }
            _ => leaves(&reads, n),
        };
        if !from.is_empty() && !output_fixed(n) {
            out.push(ValueDuty { at: None, col: n.clone(), name: n.clone() });
        }
    }
    out
}

/// Every name an expression reads.
fn expr_names(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Name(n, _) => {
            out.insert(n.clone());
        }
        Expr::Lit(..) => {}
        Expr::Bin(l, _, r, _) => {
            expr_names(l, out);
            expr_names(r, out);
        }
        Expr::Call(_, args, _) => args.iter().for_each(|a| expr_names(a, out)),
    }
}

/// Whether a word in an output cell names a value the rule binds — an input, an element's
/// field, a count, a `derive`, a `define`, a table's output column — rather than being a value
/// of the column's own enum. The evaluator reads a cell the same way: the binding when there is
/// one, the word itself otherwise.
fn bound(f: &RuleFile, n: &str) -> bool {
    f.inputs.iter().chain(f.elements.iter().flat_map(|e| &e.fields)).any(|i| i.name.text == n)
        || f.items.iter().any(|it| match it {
            Item::Derived(d) => d.name.text == n,
            Item::Define(d) => d.name.text == n,
            Item::Table(t) => t.outputs.iter().any(|o| o.name.text == n),
            Item::Agg(a) => a.name.text == n,
        })
}

/// What each computed name reads directly: a `derive` or `define` its expression's names, a
/// table's output column the table's columns and the names its rows write into it. A name
/// with no entry is an input, an element's field, or a count — a value that comes in.
fn reads_of(f: &RuleFile) -> HashMap<String, BTreeSet<String>> {
    let mut reads: HashMap<String, BTreeSet<String>> = HashMap::new();
    for it in &f.items {
        match it {
            Item::Derived(d) => expr_names(&d.expr, reads.entry(d.name.text.clone()).or_default()),
            Item::Define(d) => expr_names(&d.expr, reads.entry(d.name.text.clone()).or_default()),
            Item::Table(t) => {
                for (ci, oc) in t.outputs.iter().enumerate() {
                    let r = reads.entry(oc.name.text.clone()).or_default();
                    r.extend(t.inputs.iter().map(|(n, _)| n.clone()));
                    for row in &t.rows {
                        if let Some(OutCell::Name(n)) = row.outs.get(ci) {
                            r.insert(n.clone());
                        }
                    }
                }
            }
            Item::Agg(_) => {}
        }
    }
    reads
}

/// The values that come in and that `name` is computed from, followed through every
/// definition and table in between.
fn leaves(reads: &HashMap<String, BTreeSet<String>>, name: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut out = BTreeSet::new();
    let mut stack = vec![name.to_string()];
    while let Some(n) = stack.pop() {
        if !seen.insert(n.clone()) {
            continue;
        }
        match reads.get(&n) {
            Some(r) => stack.extend(r.iter().cloned()),
            None => {
                out.insert(n);
            }
        }
    }
    out
}

/// The outputs whose rounding tie is an obligation, with the grid of each (§15.151).
///
/// Every output that declares a rounding raises one, unless the rule shows that no input
/// brings it half a step off the grid. What shows it is arithmetic on grids: the value, taken
/// row by row and definition by definition, is `Σ aᵥ·v + k` with each `v` a multiple of its
/// own step (a yen, one step of a rate), so every value it can take is `k` plus a multiple of
/// the greatest common divisor `d` of the grid and the `aᵥ·stepᵥ`. The tie is `grid ÷ 2` off
/// the grid; when `grid ÷ 2 − k` is no multiple of `d` in any branch, nothing reaches it.
/// 厚生年金保険料 is the case in point: every standard monthly remuneration is a multiple of
/// 2,000 yen, the rate a multiple of 0.1%, and half of their product is always whole yen.
///
/// The ranges are left out, and a product of two names is taken to reach every multiple of the
/// product of their steps; both only make more values reachable, so what this rules out is
/// ruled out. What the arithmetic does not cover — a division by an input — proves nothing, and
/// the obligation stands. So does a tie that some form reaches and no input was found for: the
/// audit then says so, rather than the obligation going away (§9.2).
pub fn tie_duties(f: &RuleFile, c: &Checked) -> Vec<(String, Rat)> {
    let mut out = Vec::new();
    for od in &f.outputs {
        let Some(rd) = &od.rounding else { continue };
        let name = od.name.text.clone();
        let Some(ty) = c.ty_of(&name) else { continue };
        let Some(g) = crate::types::lit_value_in_pub(&rd.grid, &ty) else { continue };
        if g.num == 0 {
            continue;
        }
        let mut fresh = 0usize;
        let unreachable = grid_forms(f, c, &Expr::Name(name.clone(), od.name.span.clone()), 0, &mut fresh)
            .is_some_and(|forms| forms.iter().all(|l| l.reaches_tie(g) == Some(false)));
        if !unreachable {
            out.push((name, g));
        }
    }
    out
}

/// A value as the grid argument reads it: `Σ aᵥ·v + k`, each `v` ranging over the multiples
/// of `steps[v]`, inside `ranges[v]` where that is known. A name that starts with `#` stands for
/// a value no input sets directly: one the rule rounded on its way (`allocate`, a rounding
/// function), or the product of two names.
#[derive(Clone)]
pub struct GridForm {
    pub terms: BTreeMap<String, Rat>,
    pub steps: BTreeMap<String, Rat>,
    pub ranges: BTreeMap<String, (Rat, Rat)>,
    pub k: Rat,
}

/// The forms output `name` can take, one per way through the tables and the sides of a `min`
/// or `max`; `None` when the arithmetic does not cover it. The vector generator solves them for
/// a tie, where moving along a slope cannot land on one (§15.151).
pub fn grid_forms_of(f: &RuleFile, c: &Checked, name: &str) -> Option<Vec<GridForm>> {
    let span = f.outputs.iter().find(|o| o.name.text == name).map(|o| o.name.span.clone())?;
    grid_forms(f, c, &Expr::Name(name.to_string(), span), 0, &mut 0)
}

impl GridForm {
    fn con(k: Rat) -> GridForm {
        GridForm { terms: BTreeMap::new(), steps: BTreeMap::new(), ranges: BTreeMap::new(), k }
    }

    fn var(n: &str, step: Rat, range: Option<(Rat, Rat)>) -> GridForm {
        GridForm {
            terms: BTreeMap::from([(n.to_string(), Rat::int(1))]),
            steps: BTreeMap::from([(n.to_string(), step)]),
            ranges: range.map(|r| (n.to_string(), r)).into_iter().collect(),
            k: Rat::zero(),
        }
    }

    fn constant(&self) -> Option<Rat> {
        self.terms.values().all(|a| a.num == 0).then_some(self.k)
    }

    fn plus(&self, o: &GridForm, sign: i128) -> Option<GridForm> {
        let mut r = self.clone();
        for (n, a) in &o.terms {
            let e = r.terms.entry(n.clone()).or_insert(Rat::zero());
            *e = e.checked_add(a.checked_mul(Rat::int(sign))?)?;
        }
        r.steps.extend(o.steps.iter().map(|(n, s)| (n.clone(), *s)));
        // A name both sides carry is bounded by both: the rows the two came from each narrowed
        // it, and a pair of rows that cannot hold together leaves it no values at all.
        for (n, b) in &o.ranges {
            let v = match r.ranges.get(n) {
                Some(a) => (max_rat(a.0, b.0), min_rat(a.1, b.1)),
                None => *b,
            };
            r.ranges.insert(n.clone(), v);
        }
        r.k = r.k.checked_add(o.k.checked_mul(Rat::int(sign))?)?;
        Some(r)
    }

    /// Narrow the names this form reads to what a row's own cells allow of them.
    fn narrow(&mut self, bx: &BTreeMap<String, (Rat, Rat)>) {
        for (n, b) in bx {
            if !self.terms.contains_key(n) {
                continue;
            }
            let v = match self.ranges.get(n) {
                Some(a) => (max_rat(a.0, b.0), min_rat(a.1, b.1)),
                None => *b,
            };
            self.ranges.insert(n.clone(), v);
        }
    }

    /// The values the form takes over the ranges of its names: `Some(None)` when some name has
    /// no value left (the rows it came from cannot hold together), `None` when a name has no
    /// known range.
    fn interval(&self) -> Option<Option<(Rat, Rat)>> {
        let (mut lo, mut hi) = (self.k, self.k);
        for (n, a) in &self.terms {
            if a.num == 0 {
                continue;
            }
            let (l, h) = *self.ranges.get(n)?;
            if l.cmp_to(h) == std::cmp::Ordering::Greater {
                return Some(None);
            }
            let (x, y) = (a.checked_mul(l)?, a.checked_mul(h)?);
            let (x, y) = if x.cmp_to(y) == std::cmp::Ordering::Greater { (y, x) } else { (x, y) };
            lo = lo.checked_add(x)?;
            hi = hi.checked_add(y)?;
        }
        Some(Some((lo, hi)))
    }

    /// A point that puts this form exactly half a step off `grid`, each name inside its range:
    /// `Some(Some(point))`, or `Some(None)` when there is none. Every name but one is walked —
    /// only as far as its multiples still land on new remainders of the grid — and the last is
    /// solved for (`solve_congruence`). `None` when a name has no known range, the walk would be
    /// too long, or the numbers do not fit; the caller then falls back on the divisor alone.
    pub fn tie_witness(&self, grid: Rat) -> Option<Option<BTreeMap<String, Rat>>> {
        let off = grid.checked_div(Rat::int(2))?.checked_sub(self.k)?;
        // (name, coefficient × step, step, first multiple, last multiple)
        let mut vars: Vec<(&String, Rat, Rat, i128, i128)> = Vec::new();
        for (n, a) in &self.terms {
            if a.num == 0 {
                continue;
            }
            let q = *self.steps.get(n)?;
            let (lo, hi) = *self.ranges.get(n)?;
            let (lo_m, hi_m) = (lo.checked_div(q)?, hi.checked_div(q)?);
            let lo_m = -((-lo_m.num).div_euclid(lo_m.den));
            let hi_m = hi_m.num.div_euclid(hi_m.den);
            if hi_m < lo_m {
                return Some(None);
            }
            vars.push((n, a.checked_mul(q)?, q, lo_m, hi_m));
        }
        // The widest name is the one solved for, so that the walk is over the narrow ones.
        vars.sort_by_key(|v| v.4 - v.3);
        let Some((solved, others)) = vars.split_last_mut().map(|(l, rest)| (*l, rest)) else {
            // A constant: on the tie or not, whatever comes in.
            return Some(off.checked_div(grid)?.is_int().then(BTreeMap::new));
        };
        // Walk the others. Past `grid ÷ gcd(coefficient × step, grid)` multiples a name repeats
        // the remainders it already gave, so that many are all it can say.
        let mut spans: Vec<i128> = Vec::new();
        let mut total: i128 = 1;
        for (_, a, _, lo_m, hi_m) in others.iter() {
            let period = grid.checked_div(rat_gcd(abs(*a), abs(grid))?)?;
            if !period.is_int() {
                return None;
            }
            let span = (hi_m - lo_m + 1).min(period.num);
            spans.push(span);
            total = total.checked_mul(span)?;
            if total > TIE_WALK {
                return None;
            }
        }
        let (sn, sa, sq, slo, shi) = solved;
        let mut at: Vec<i128> = vec![0; others.len()];
        loop {
            let mut rest = off;
            for (i, (_, a, _, lo_m, _)) in others.iter().enumerate() {
                rest = rest.checked_sub(a.checked_mul(Rat::int(lo_m + at[i]))?)?;
            }
            if let Some((m0, period)) = solve_congruence(sa, rest, grid)? {
                let first = m0.checked_add(slo.checked_sub(m0)?.checked_add(period - 1)?.div_euclid(period).checked_mul(period)?)?;
                if first <= shi {
                    let mut point = BTreeMap::new();
                    point.insert(sn.clone(), Rat::int(first).checked_mul(sq)?);
                    for (i, (n, _, q, lo_m, _)) in others.iter().enumerate() {
                        point.insert((*n).clone(), Rat::int(lo_m + at[i]).checked_mul(*q)?);
                    }
                    return Some(Some(point));
                }
            }
            // The next combination, odometer fashion.
            let mut i = 0;
            loop {
                if i == at.len() {
                    return Some(None);
                }
                at[i] += 1;
                if at[i] < spans[i] {
                    break;
                }
                at[i] = 0;
                i += 1;
            }
        }
    }

    fn scale(&self, f: Rat) -> Option<GridForm> {
        let mut r = self.clone();
        for a in r.terms.values_mut() {
            *a = a.checked_mul(f)?;
        }
        r.k = r.k.checked_mul(f)?;
        Some(r)
    }

    /// The product of two forms. `x·y` with `x = stepₓ·m` and `y = step_y·n` is
    /// `stepₓ·step_y·(m·n)`, a multiple of `stepₓ·step_y`, so each pair of names becomes one
    /// name on that grid, between the products of their ends. Which multiples in there it
    /// reaches is left open — every one, as far as this argument knows — so the product is read
    /// wider than it is, never narrower.
    fn times(&self, o: &GridForm) -> Option<GridForm> {
        let mut r = GridForm::con(self.k.checked_mul(o.k)?);
        let linear = |g: &GridForm| GridForm { k: Rat::zero(), ..g.clone() };
        r = r.plus(&linear(self).scale(o.k)?, 1)?;
        r = r.plus(&linear(o).scale(self.k)?, 1)?;
        for (x, a) in &self.terms {
            for (y, b) in &o.terms {
                let name = if x <= y { format!("#{x}*{y}") } else { format!("#{y}*{x}") };
                let step = self.steps.get(x)?.checked_mul(*o.steps.get(y)?)?;
                let range = match (self.ranges.get(x), o.ranges.get(y)) {
                    (Some((xl, xh)), Some((yl, yh))) => {
                        let ends = [xl.checked_mul(*yl)?, xl.checked_mul(*yh)?, xh.checked_mul(*yl)?, xh.checked_mul(*yh)?];
                        let lo = ends.iter().copied().min_by(|p, q| p.cmp_to(*q))?;
                        let hi = ends.iter().copied().max_by(|p, q| p.cmp_to(*q))?;
                        Some((lo, hi))
                    }
                    _ => None,
                };
                r = r.plus(&GridForm::var(&name, step, range).scale(a.checked_mul(*b)?)?, 1)?;
            }
        }
        Some(r)
    }

    /// `Some(false)` when no value of this form sits half a step off `grid`, `Some(true)` when
    /// one may, `None` when the numbers do not fit.
    ///
    /// Settled exactly, ranges included, wherever `tie_witness` can walk it: `amount × 3.49%`
    /// reaches a half cent at one amount in ten thousand, and whether that amount lies in the
    /// declared range is a question with an answer; so is whether two small amounts at two
    /// rates ever add up to one. Past that, the common divisor alone decides, ranges left out.
    fn reaches_tie(&self, grid: Rat) -> Option<bool> {
        if let Some(w) = self.tie_witness(grid) {
            return Some(w.is_some());
        }
        let off = grid.checked_div(Rat::int(2))?.checked_sub(self.k)?;
        let mut d = abs(grid);
        for (n, a) in self.terms.iter().filter(|(_, a)| a.num != 0) {
            d = rat_gcd(d, abs(a.checked_mul(*self.steps.get(n)?)?))?;
        }
        Some(off.checked_div(d)?.is_int())
    }
}

/// The whole numbers `m` with `a·m ≡ b (mod g)`, as `m0 + t·period`: `Some(None)` when there is
/// none, `None` when the numbers do not fit in 128 bits.
pub fn solve_congruence(a: Rat, b: Rat, g: Rat) -> Option<Option<(i128, i128)>> {
    let d = lcm(lcm(a.den, b.den)?, g.den)?;
    let big_a = a.num.checked_mul(d / a.den)?;
    let big_b = b.num.checked_mul(d / b.den)?;
    let big_g = g.num.checked_mul(d / g.den)?.checked_abs()?;
    if big_g == 0 {
        return None;
    }
    let (g0, inv, _) = egcd(big_a.rem_euclid(big_g), big_g);
    if big_b.rem_euclid(g0) != 0 {
        return Some(None);
    }
    let period = big_g / g0;
    let m0 = (big_b / g0).rem_euclid(period).checked_mul(inv.rem_euclid(period))?.rem_euclid(period);
    Some(Some((m0, period)))
}

/// `(g, x, y)` with `a·x + b·y = g`, the greatest common divisor of `a` and `b`.
fn egcd(a: i128, b: i128) -> (i128, i128, i128) {
    if b == 0 {
        (a, 1, 0)
    } else {
        let (g, x, y) = egcd(b, a.rem_euclid(b));
        (g, y, x - a.div_euclid(b) * y)
    }
}

fn lcm(a: i128, b: i128) -> Option<i128> {
    let (mut x, mut y) = (a.abs(), b.abs());
    while y != 0 {
        let t = x % y;
        x = y;
        y = t;
    }
    if x == 0 { Some(0) } else { (a.abs() / x).checked_mul(b.abs()) }
}

fn abs(r: Rat) -> Rat {
    Rat { num: r.num.abs(), den: r.den }
}

fn max_rat(a: Rat, b: Rat) -> Rat {
    if a.cmp_to(b) == std::cmp::Ordering::Less { b } else { a }
}

fn min_rat(a: Rat, b: Rat) -> Rat {
    if a.cmp_to(b) == std::cmp::Ordering::Greater { b } else { a }
}

/// How many combinations `tie_witness` walks before it leaves the question to the divisor.
const TIE_WALK: i128 = 1 << 20;

/// What a row's own cells say of the numbers that come in: an interval for each input column
/// the row tests with a number. A column computed from several inputs says something too, but
/// not about one of them alone, and is left out — which only leaves more values possible.
fn row_box(f: &RuleFile, c: &Checked, t: &Table, row: &Row) -> BTreeMap<String, (Rat, Rat)> {
    let mut out = BTreeMap::new();
    for ((col, _), cell) in t.inputs.iter().zip(&row.cells) {
        if !f.inputs.iter().chain(f.elements.iter().flat_map(|e| &e.fields)).any(|i| &i.name.text == col) {
            continue;
        }
        let Some(ty) = c.ty_of(col) else { continue };
        if !is_numeric(&ty) {
            continue;
        }
        let q = quantum(c, col, &ty);
        let (mut lo, mut hi) = match c.ranges.get(col.as_str()) {
            Some((Some(l), Some(h))) => (*l, *h),
            _ => continue,
        };
        let val = |l: &Lit| -> Option<Rat> {
            match l {
                Lit::Num(n) => crate::types::lit_value_in_pub(n, &ty),
                Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                _ => None,
            }
        };
        match cell {
            Cell::Lit(l) => {
                let Some(v) = val(l) else { continue };
                (lo, hi) = (max_rat(lo, v), min_rat(hi, v));
            }
            Cell::Cmp(atoms) => {
                for (op, l) in atoms {
                    let Some(v) = val(l) else { continue };
                    match op {
                        CmpOp::Le => hi = min_rat(hi, v),
                        CmpOp::Lt => hi = min_rat(hi, v.sub(q)),
                        CmpOp::Ge => lo = max_rat(lo, v),
                        CmpOp::Gt => lo = max_rat(lo, v.add(q)),
                    }
                }
            }
            _ => continue,
        }
        out.insert(col.clone(), (lo, hi));
    }
    out
}

/// The greatest common divisor of two non-negative rationals: the largest `d` of which both
/// are whole multiples.
fn rat_gcd(a: Rat, b: Rat) -> Option<Rat> {
    if a.num == 0 {
        return Some(b);
    }
    if b.num == 0 {
        return Some(a);
    }
    let (mut x, mut y) = (a.num.checked_mul(b.den)?, b.num.checked_mul(a.den)?);
    while y != 0 {
        let t = x % y;
        x = y;
        y = t;
    }
    Rat::checked_new(x, a.den.checked_mul(b.den)?)
}

/// How many branches the grid argument follows before it gives up and leaves the obligation
/// standing.
const GRID_BRANCHES: usize = 4096;

/// The forms an expression can take: one per way through the tables it reads — a table's
/// column is each of its rows' cells in turn — and per side of a `min` or `max`. `None` for
/// anything the arithmetic does not cover.
fn grid_forms(f: &RuleFile, c: &Checked, e: &Expr, depth: usize, fresh: &mut usize) -> Option<Vec<GridForm>> {
    if depth > 64 {
        return None;
    }
    let cross = |a: Vec<GridForm>, b: Vec<GridForm>, op: &dyn Fn(&GridForm, &GridForm) -> Option<GridForm>| {
        if a.len().saturating_mul(b.len()) > GRID_BRANCHES {
            return None;
        }
        let mut out = Vec::new();
        for x in &a {
            for y in &b {
                out.push(op(x, y)?);
            }
        }
        Some(out)
    };
    match e {
        Expr::Lit(l @ Lit::Num(n), _) => match eval::lit_to_val(l, &crate::types::lit_ty_pub(n))? {
            Val::Num(r) => Some(vec![GridForm::con(r)]),
            _ => None,
        },
        Expr::Lit(..) => None,
        Expr::Name(n, _) => {
            let expr_of = f.items.iter().find_map(|it| match it {
                Item::Derived(d) if d.name.text == *n => Some(&d.expr),
                Item::Define(d) if d.name.text == *n => Some(&d.expr),
                _ => None,
            });
            if let Some(x) = expr_of {
                return grid_forms(f, c, x, depth + 1, fresh);
            }
            // The first output, when a `result` line gives it.
            if let (Some(r), Some(od)) = (&f.result, f.outputs.first()) {
                if od.name.text == *n {
                    return grid_forms(f, c, &r.expr, depth + 1, fresh);
                }
            }
            let ty = c.ty_of(n)?;
            let mut out = Vec::new();
            let mut column = false;
            for it in &f.items {
                let Item::Table(t) = it else { continue };
                let Some(ci) = t.outputs.iter().position(|o| o.name.text == *n) else { continue };
                column = true;
                for row in &t.rows {
                    match row.outs.get(ci)? {
                        OutCell::Lit(Lit::Num(x)) => out.push(GridForm::con(crate::types::lit_value_in_pub(x, &ty)?)),
                        OutCell::Lit(_) => return None,
                        // A word that names nothing is `none`, or a value of an enum: no number,
                        // so nothing that could sit on a tie.
                        OutCell::Name(m) if !bound(f, m) => {}
                        OutCell::Name(m) => {
                            // The row's value only where the row holds: its cells narrow what
                            // comes in (§15.152).
                            let bx = row_box(f, c, t, row);
                            for mut g in grid_forms(f, c, &Expr::Name(m.clone(), row.span.clone()), depth + 1, fresh)? {
                                g.narrow(&bx);
                                out.push(g);
                            }
                        }
                    }
                    if out.len() > GRID_BRANCHES {
                        return None;
                    }
                }
            }
            if column {
                return Some(out);
            }
            // A value that comes in: a multiple of its own step.
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                return None;
            }
            let range = match c.ranges.get(n.as_str()) {
                Some((Some(lo), Some(hi))) => Some((*lo, *hi)),
                _ => None,
            };
            Some(vec![GridForm::var(n, quantum(c, n, &ty), range)])
        }
        Expr::Bin(l, op, r, _) => {
            let (a, b) = (grid_forms(f, c, l, depth + 1, fresh)?, grid_forms(f, c, r, depth + 1, fresh)?);
            match op {
                BinOp::Add => cross(a, b, &|x, y| x.plus(y, 1)),
                BinOp::Sub => cross(a, b, &|x, y| x.plus(y, -1)),
                BinOp::Mul => cross(a, b, &|x, y| match (x.constant(), y.constant()) {
                    (Some(k), _) => y.scale(k),
                    (_, Some(k)) => x.scale(k),
                    _ => x.times(y),
                }),
                BinOp::Div => cross(a, b, &|x, y| {
                    let k = y.constant()?;
                    if k.num == 0 { None } else { x.scale(Rat::int(1).checked_div(k)?) }
                }),
                _ => None,
            }
        }
        Expr::Call(name, args, _) => {
            // `min` and `max` are one of their two sides.
            if name == crate::kw::MIN || name == crate::kw::MAX {
                let mut out = Vec::new();
                for x in args {
                    out.extend(grid_forms(f, c, x, depth + 1, fresh)?);
                }
                return (out.len() <= GRID_BRANCHES).then_some(out);
            }
            // `allocate` is rounded down to a whole unit, a rounding to its grid: each lands on
            // a grid of its own, whatever it was computed from.
            let step = if name == crate::kw::ALLOCATE {
                Rat::int(1)
            } else if crate::num::RoundMode::parse(name).is_some() {
                let g = grid_forms(f, c, args.get(1)?, depth + 1, fresh)?;
                let [one] = g.as_slice() else { return None };
                let g = one.constant()?;
                if g.num == 0 {
                    return None;
                }
                abs(g)
            } else {
                return None;
            };
            *fresh += 1;
            Some(vec![GridForm::var(&format!("#{fresh}"), step, None)])
        }
    }
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
    let w: usize = CRITERIA.iter().map(|k| crate::diag::width(label(k))).max().unwrap_or(0);
    for k in CRITERIA {
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
    let criteria: Vec<String> = CRITERIA
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
        VALUE => "value_pair",
        TIE => "rounding_tie",
        FOLD => "fold_transition",
        MACHINE => "machine_transition",
        _ => "shadow_pair",
    }
}


/// The machine's obligations and which of them the suite's traces meet (§15.148).
fn machine_obligations(
    f: &RuleFile,
    c: &Checked,
    tally: &mut BTreeMap<&'static str, (usize, usize)>,
    missing: &mut Vec<Missing>,
    bump: &dyn Fn(&'static str, bool, &mut BTreeMap<&'static str, (usize, usize)>),
) {
    let Some(m) = &f.machine else { return };
    let Some((cin, _)) = m.carried() else { return };
    let Some(a) = crate::machine::analyze(f, c, crate::region::DEFAULT_BUDGET as usize) else { return };
    if a.blocked.is_some() || a.over_budget {
        bump(MACHINE, false, tally);
        missing.push(Missing {
            kind: MACHINE,
            what: tr!("ステートマシンの遷移", "the machine's transitions"),
            hint: tr!(
                "入力の区画を歩ききれなかったので、遷移の一覧を立てられませんでした（E128）。",
                "The inputs' cells could not all be walked, so the transitions could not be listed (E128)."
            ),
        });
        return;
    }
    // The transitions a case can make, and the pairs one case can make in a row: both calls
    // in the same world, since a case holds its `held` inputs (§15.149).
    let key = |e: &crate::machine::Edge| -> vectors::TransitionKey { (e.from.clone(), e.row.clone(), e.to.clone()) };
    let mut ts: Vec<vectors::TransitionKey> = Vec::new();
    let mut pairs: Vec<(vectors::TransitionKey, vectors::TransitionKey)> = Vec::new();
    for (i, e1) in a.edges.iter().enumerate() {
        if !a.reaches(&e1.from, &e1.world) || a.mixed.contains(&i) {
            continue;
        }
        if !ts.contains(&key(e1)) {
            ts.push(key(e1));
        }
        for (j, e2) in a.edges.iter().enumerate() {
            let p = (key(e1), key(e2));
            if e2.world == e1.world && e2.from == e1.to && !a.mixed.contains(&j) && !pairs.contains(&p) {
                pairs.push(p);
            }
        }
    }
    let name = |k: &vectors::TransitionKey| -> String {
        match &k.1 {
            Some((t, r)) => format!("{} -[{}]-> {}", k.0, eval::row_tag(t, *r), k.2),
            None => format!("{} -> {}", k.0, k.2),
        }
    };
    // Replay the traces.
    let mut seen: BTreeSet<vectors::TransitionKey> = BTreeSet::new();
    let mut seen_pairs: BTreeSet<(vectors::TransitionKey, vectors::TransitionKey)> = BTreeSet::new();
    if let Some(traces) = vectors::machine_traces(f, c) {
        for tr in &traces.traces {
            let mut state = Val::Enum(traces.initial.clone());
            let mut prev: Option<vectors::TransitionKey> = None;
            for s in &tr.steps {
                let mut input = s.clone();
                input.insert(cin.to_string(), state.clone());
                let Some((k, _)) = vectors::transition_of(f, c, &input) else { break };
                seen.insert(k.clone());
                if let Some(p) = prev.take() {
                    seen_pairs.insert((p, k.clone()));
                }
                state = Val::Enum(k.2.clone());
                prev = Some(k);
            }
        }
    }
    for t in &ts {
        let met = seen.contains(t);
        bump(MACHINE, met, tally);
        if !met {
            missing.push(Missing {
                kind: MACHINE,
                what: tr!("遷移 {}", "transition {}", name(t)),
                hint: tr!("この遷移を通る手順がありません。", "No trace makes this transition."),
            });
        }
    }
    for (t1, t2) in &pairs {
        {
            let met = seen_pairs.contains(&(t1.clone(), t2.clone()));
            bump(MACHINE, met, tally);
            if !met {
                missing.push(Missing {
                    kind: MACHINE,
                    what: tr!("{} のあと {}", "{} then {}", name(t1), name(t2)),
                    hint: tr!(
                        "この二つを続けて通る手順がありません。手順が確かめるのは、一回の答えを次の呼び出しに渡すところです。",
                        "No trace makes these two in a row. What a trace holds is the answer of one call handed to the next."
                    ),
                });
            }
        }
    }
}
