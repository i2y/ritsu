//! Test-case generation from boundaries (§9).
//!
//! Candidate values are chosen per column, the candidate population built from them is run
//! through the reference evaluator, and a subset satisfying row coverage, boundary-pair
//! coverage and shadow-pair coverage is selected deterministically. The evaluator attaches
//! the expected values and the fired rows.

use crate::ast::*;
use crate::eval::{self, Val};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct Vector {
    pub input: BTreeMap<String, Val>,
    /// All outputs in declaration order (the multiple outputs of §8.5). The wire format and the
    /// golden files follow this order too.
    pub outputs: Vec<(String, Option<Val>)>,
    pub trace: Vec<String>,
    /// The same rows as `(table, row)`. `verify` clusters on these; `trace` is the prose.
    pub fired: Vec<(String, usize)>,
    pub why: String,
}

/// Candidate values per column (§9.1).
fn candidates(f: &RuleFile, c: &Checked) -> BTreeMap<String, Vec<Val>> {
    let mut out: BTreeMap<String, Vec<Val>> = BTreeMap::new();
    for i in &f.inputs {
        let name = &i.name.text;
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        let inner = match &ty {
            Ty::Opt(t) => (**t).clone(),
            other => other.clone(),
        };
        let mut vs: Vec<Val> = Vec::new();
        if matches!(ty, Ty::Opt(_)) {
            // An optional column gets `none` plus the candidates of the present side (§9.1).
            vs.push(Val::Enum(crate::kw::NONE.into()));
        }
        match &inner {
            Ty::Enum(en) => {
                // §9.1: one representative per equivalence class induced by the cells. Rather
                // than sweeping all 47 prefectures, only the partitions the table distinguishes
                // are visited. A class is determined by the sequence of "which cells match", so
                // overlapping groups and `not:` fold correctly on their own. Values no cell names
                // fall into one class, which becomes the representative of the `not:` side.
                let all = c.enums.get(en).cloned().unwrap_or_default();
                let cells = enum_cells(f, name);
                let mut seen: BTreeSet<Vec<bool>> = BTreeSet::new();
                for v in &all {
                    let val = Val::Enum(v.clone());
                    let sig: Vec<bool> =
                        cells.iter().map(|cell| eval::cell_matches(c, cell, &val, &inner)).collect();
                    if seen.insert(sig) {
                        vs.push(val);
                    }
                }
            }
            Ty::Bool => {
                vs.push(Val::Bool(true));
                vs.push(Val::Bool(false));
            }
            Ty::Date => {
                let q = Rat::int(1);
                for b in numeric_bounds(f, name, &inner, c) {
                    for d in [-1i128, 0, 1] {
                        let v = b.add(q.mul(Rat::int(d)));
                        let (y, m, dd) = crate::types::ord_to_date(v);
                        vs.push(Val::Date(y, m, dd));
                    }
                }
            }
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                let q = match &inner {
                    Ty::Rate => Rat::new(1, *c.scales.get(name).unwrap_or(&100)),
                    _ => Rat::int(1),
                };
                for b in numeric_bounds(f, name, &inner, c) {
                    for d in [-1i128, 0, 1] {
                        vs.push(Val::Num(b.add(q.mul(Rat::int(d)))));
                    }
                }
            }
            _ => {}
        }
        // Drop values outside the declared range. The entry guard rejects them, so only in-range
        // values take part in the three-way comparison (§8.5).
        if let Some((lo, hi)) = c.ranges.get(name) {
            vs.retain(|v| match v {
                Val::Num(x) => {
                    lo.is_none_or(|l| x.cmp_to(l) != std::cmp::Ordering::Less)
                        && hi.is_none_or(|h| x.cmp_to(h) != std::cmp::Ordering::Greater)
                }
                Val::Date(y, m, d) => {
                    let x = crate::types::date_ord(*y, *m, *d);
                    lo.is_none_or(|l| x.cmp_to(l) != std::cmp::Ordering::Less)
                        && hi.is_none_or(|h| x.cmp_to(h) != std::cmp::Ordering::Greater)
                }
                _ => true,
            });
        }
        vs.dedup_by(|a, b| a == b);
        if vs.is_empty() {
            vs.push(default_val(&inner, c));
        }
        out.insert(name.clone(), vs);
    }
    out
}

fn default_val(ty: &Ty, c: &Checked) -> Val {
    match ty {
        Ty::Enum(en) => Val::Enum(c.enums.get(en).and_then(|v| v.first()).cloned().unwrap_or_default()),
        Ty::Bool => Val::Bool(true),
        Ty::Date => Val::Date(2026, 1, 1),
        _ => Val::Num(Rat::zero()),
    }
}

/// The enum values named by the cells of a column (groups are expanded).
/// Collects every cell that appears in column `col`; the equivalence-class signature is
/// determined by this sequence. A comparison against an enum value inside a definition or a
/// derived value counts as a cell too (an atom such as `種別 = 率引き` never appears in a
/// table, yet it distinguishes values).
fn enum_cells(f: &RuleFile, col: &str) -> Vec<Cell> {
    fn atoms(e: &Expr, col: &str, out: &mut Vec<Cell>) {
        match e {
            Expr::Bin(l, BinOp::Eq, r, _) => {
                for (a, b) in [(&**l, &**r), (&**r, &**l)] {
                    if let (Expr::Name(n, _), Expr::Lit(li, _)) = (a, b) {
                        if n == col {
                            out.push(Cell::Lit(li.clone()));
                        }
                    }
                }
            }
            Expr::Bin(l, _, r, _) => {
                atoms(l, col, out);
                atoms(r, col, out);
            }
            Expr::Call(_, args, _) => args.iter().for_each(|a| atoms(a, col, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for it in &f.items {
        match it {
            Item::Table(t) => {
                let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
                for row in &t.rows {
                    if let Some(cell) = row.cells.get(ci) {
                        out.push(cell.clone());
                    }
                }
            }
            Item::Define(d) => atoms(&d.expr, col, &mut out),
            Item::Derived(d) => atoms(&d.expr, col, &mut out),
        }
    }
    out
}

#[allow(dead_code)]
fn named_values(f: &RuleFile, col: &str, c: &Checked) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
        for row in &t.rows {
            let Some(cell) = row.cells.get(ci) else { continue };
            let ls: Vec<&Lit> = match cell {
                Cell::Lit(l) => vec![l],
                Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
                _ => vec![],
            };
            for l in ls {
                if let Lit::Word(w) = l {
                    match c.groups.get(w) {
                        Some((_, ms)) => out.extend(ms.iter().cloned()),
                        None => {
                            out.insert(w.clone());
                        }
                    }
                }
            }
        }
    }
    out
}

/// The boundary values that appear in the column's cells and in its declared range.
fn numeric_bounds(f: &RuleFile, col: &str, ty: &Ty, c: &Checked) -> Vec<Rat> {
    let mut set: BTreeSet<(i128, i128)> = BTreeSet::new();
    let push = |r: Rat, s: &mut BTreeSet<(i128, i128)>| {
        s.insert((r.num, r.den));
    };
    if let Some((lo, hi)) = c.ranges.get(col) {
        for b in [lo, hi].into_iter().flatten() {
            push(*b, &mut set);
        }
    }
    let lit = |l: &Lit| -> Option<Rat> {
        match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty),
            Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
            _ => None,
        }
    };
    for it in &f.items {
        match it {
            Item::Table(t) => {
                if let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) {
                    for row in &t.rows {
                        match row.cells.get(ci) {
                            Some(Cell::Cmp(cs)) => {
                                for (_, l) in cs {
                                    if let Some(v) = lit(l) {
                                        push(v, &mut set);
                                    }
                                }
                            }
                            Some(Cell::Lit(l)) => {
                                if let Some(v) = lit(l) {
                                    push(v, &mut set);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            // §9.1: the analysis treats boolean definitions as free axes, so the boundaries of
            // their atoms are picked up here. A boundary the analysis cannot see is exactly the
            // one nobody steps on unless the vectors do.
            Item::Define(d) => atom_bounds(&d.expr, col, ty, &mut set),
            Item::Derived(_) => {}
        }
    }
    let mut v: Vec<Rat> = set.into_iter().map(|(n, d)| Rat { num: n, den: d }).collect();
    v.sort_by(|a, b| a.cmp_to(*b));
    v
}

fn atom_bounds(e: &Expr, col: &str, ty: &Ty, set: &mut BTreeSet<(i128, i128)>) {
    match e {
        Expr::Bin(l, op, r, _) if matches!(op, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt | BinOp::Eq) => {
            let mut take = |name: &Expr, lit: &Expr| {
                if let (Expr::Name(n, _), Expr::Lit(li, _)) = (name, lit) {
                    if n == col {
                        let v = match li {
                            Lit::Num(x) => crate::types::lit_value_in_pub(x, ty),
                            Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                            _ => None,
                        };
                        if let Some(v) = v {
                            set.insert((v.num, v.den));
                        }
                    }
                }
            };
            take(l, r);
            take(r, l);
        }
        Expr::Bin(l, _, r, _) => {
            atom_bounds(l, col, ty, set);
            atom_bounds(r, col, ty, set);
        }
        _ => {}
    }
}

/// The first candidate value that satisfies the cell. Row coverage (§9.2) targets a row with it.
fn satisfying(cell: &Cell, cands: &[Val], ty: &Ty, c: &Checked) -> Option<Val> {
    let env = eval::Env::new(c, BTreeMap::new().into_iter().collect());
    cands.iter().find(|v| env.matches_pub(cell, v, ty)).cloned()
}

/// Build the candidate population: row targeting, then one column at a time, then pairs of
/// columns, added in that order.
fn pool(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>) -> Vec<(BTreeMap<String, Val>, String)> {
    let names: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let base: BTreeMap<String, Val> = names
        .iter()
        .map(|n| (n.clone(), cands[n].first().cloned().unwrap()))
        .collect();
    let mut out: Vec<(BTreeMap<String, Val>, String)> = vec![(base.clone(), tr!("基準", "baseline"))];

    // Row coverage: build an assignment that makes each row win. Columns holding derived or
    // intermediate values are mapped back onto the inputs (§9.1). Naively "putting a value that
    // satisfies the input cell" never touches a column a derived value decides, and the row
    // stays beaten by an earlier row under `first`. That showed up in the coverage auditor as
    // missing row coverage (クーポン併用 row 3, 適用順序 row 4, 素の割引 row 3).
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for (ri, row) in t.rows.iter().enumerate() {
            let mut a = base.clone();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if !cands.contains_key(col) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
                if let Some(v) = satisfying(cell, &cands[col], &ty, c) {
                    a.insert(col.clone(), v);
                }
            }
            let seed = win_row(f, c, cands, &a, t, ri).unwrap_or(a);
            out.push((seed.clone(), tr!("行狙い: 表 {tname} 行{}", "row target: table {tname} row {}", ri + 1)));

            // Boundary-pair coverage: build a **pair** that steps on both sides of a boundary
            // with the row's other columns held fixed. `place` picks inputs in declaration order
            // so that the same input moves for the inside and the outside point.
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let Some(ty) = c.ty_of(col) else { continue };
                if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let q = crate::coverage::quantum(c, col, &ty);
                for (b, inside, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
                    let (Some(ain), Some(aout)) =
                        (place(f, c, &seed, col, inside), place(f, c, &seed, col, outside))
                    else {
                        continue;
                    };
                    let why = tr!(
                        "境界両側: 表 {tname} 行{} {col} {b} の",
                        "boundary pair: table {tname} row {} {col} {b}",
                        ri + 1
                    );
                    out.push((ain, tr!("{why}内側", "{why} inside")));
                    out.push((aout, tr!("{why}外側", "{why} outside")));
                }
            }
        }

        // Shadow-pair coverage: the inside of the intersection (a point where row i wins while
        // row j's conditions hold too).
        if t.policy == Policy::TopDown {
            for j in 1..t.rows.len() {
                for i in 0..j {
                    let Some(a) = reach_row(f, c, cands, &base, t, &t.rows[j]) else { continue };
                    let Some(a) = win_row(f, c, cands, &a, t, i) else { continue };
                    if row_holds(f, c, t, &t.rows[j], &a) {
                        out.push((
                            a,
                            tr!("遮蔽対: 表 {tname} 行{}∩行{}", "shadow pair: table {tname} row {} ∩ row {}", i + 1, j + 1),
                        ));
                    }
                }
            }
        }
    }

    // §9.1: a comparison between two names (`A期限 <= B期限`) steps directly on the tie and on
    // both sides of it. It has no literal boundary, so it never shows up among the candidates.
    for (x, y) in name_pairs(f) {
        let (Some(xty), Some(_)) = (c.ty_of(&x), c.ty_of(&y)) else { continue };
        let q = crate::coverage::quantum(c, &x, &xty);
        for d in [-1i128, 0, 1] {
            let Some(xv) = base.get(&x).and_then(as_rat) else { continue };
            let t = xv.add(q.mul(Rat::int(d)));
            if let Some(a) = place(f, c, &base, &y, t) {
                out.push((a, tr!("同着: {x} と {y} の {d:+}", "tie: {x} and {y}, offset {d:+}")));
            }
        }
    }

    // Both sides of a boundary: sweep one column at a time over all its candidates, which were
    // built at ±one step around each boundary.
    for n in &names {
        for v in &cands[n] {
            let mut a = base.clone();
            a.insert(n.clone(), v.clone());
            out.push((a, tr!("境界: {n}", "boundary: {n}")));
        }
    }

    // Pairwise: greedily add combinations of two columns (the safety net of §9.2).
    let mut seen: BTreeSet<(String, String, String, String)> = BTreeSet::new();
    for (a, _) in &out {
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                seen.insert(key2(&names[i], &a[&names[i]], &names[j], &a[&names[j]]));
            }
        }
    }
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            for vi in &cands[&names[i]] {
                for vj in &cands[&names[j]] {
                    let k = key2(&names[i], vi, &names[j], vj);
                    if seen.contains(&k) {
                        continue;
                    }
                    let mut a = base.clone();
                    a.insert(names[i].clone(), vi.clone());
                    a.insert(names[j].clone(), vj.clone());
                    for x in 0..names.len() {
                        for y in (x + 1)..names.len() {
                            seen.insert(key2(&names[x], &a[&names[x]], &names[y], &a[&names[y]]));
                        }
                    }
                    out.push((a, tr!("ペアワイズ: {} × {}", "pairwise: {} × {}", names[i], names[j])));
                }
            }
        }
    }
    out
}

fn key2(a: &str, va: &Val, b: &str, vb: &Val) -> (String, String, String, String) {
    (a.into(), show(va), b.into(), show(vb))
}

pub fn show(v: &Val) -> String {
    match v {
        Val::Enum(s) | Val::Str(s) => s.clone(),
        Val::Bool(b) => if *b { crate::kw::TRUE } else { crate::kw::FALSE }.into(),
        Val::Num(r) => format!("{}", r.num / r.den),
        Val::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

/// Evaluate the candidate population and deterministically select a subset that satisfies
/// coverage.
pub fn generate(f: &RuleFile, c: &Checked) -> Vec<Vector> {
    let cands = candidates(f, c);
    let raw = pool(f, c, &cands);

    let mut evaluated: Vec<Vector> = Vec::new();
    let mut seen_in: BTreeSet<String> = BTreeSet::new();
    for (a, why) in raw {
        let key = a.iter().map(|(k, v)| format!("{k}={}", show(v))).collect::<Vec<_>>().join(",");
        if !seen_in.insert(key) {
            continue;
        }
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        evaluated.push(Vector { input: a, outputs: outs, trace, fired, why });
    }

    // Keep the vectors that actually discharged one of the three §9.2 criteria. The key is not
    // what the targeting side claims, but only what the auditor accepted as "this discharges
    // the obligation". A pair obligation admits both vectors together, which is where greedy
    // scoring would drop one.
    let audit = crate::coverage::audit(f, c, "", &evaluated);
    let mut keep: BTreeSet<usize> = audit.witness.clone();

    // The pairwise safety net (§9.2): with the three criteria satisfied, greedily add the
    // two-column combinations that have not appeared yet.
    let mut seen2: BTreeSet<(String, String, String, String)> = BTreeSet::new();
    let names: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let pairs_of = |v: &Vector, out: &mut BTreeSet<(String, String, String, String)>| {
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                let (Some(a), Some(b)) = (v.input.get(&names[i]), v.input.get(&names[j])) else {
                    continue;
                };
                out.insert(key2(&names[i], a, &names[j], b));
            }
        }
    };
    for &k in &keep {
        pairs_of(&evaluated[k], &mut seen2);
    }
    loop {
        let mut best: Option<(usize, usize)> = None;
        for (i, v) in evaluated.iter().enumerate() {
            if keep.contains(&i) {
                continue;
            }
            let mut mine = BTreeSet::new();
            pairs_of(v, &mut mine);
            let gain = mine.difference(&seen2).count();
            if gain > 0 && best.map(|(g, _)| gain > g).unwrap_or(true) {
                best = Some((gain, i));
            }
        }
        let Some((_, i)) = best else { break };
        keep.insert(i);
        pairs_of(&evaluated[i], &mut seen2);
    }

    evaluated
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, v)| v)
        .collect()
}

/// Canonical JSON. Three-way agreement is judged on these bytes (§8.5).
pub fn to_json(f: &RuleFile, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut ins: Vec<String> = Vec::new();
    for i in &f.inputs {
        let Some(val) = v.input.get(&i.name.text) else { continue };
        let body = match val {
            Val::Num(r) => format!("{}", r.num / r.den),
            Val::Bool(b) => format!("{b}"),
            other => format!("\"{}\"", esc(&show(other))),
        };
        ins.push(format!("\"{}\":{body}", esc(&i.name.text)));
    }
    format!(
        "{{\"in\":{{{}}},\"out\":{},\"trace\":[{}],\"why\":\"{}\"}}",
        ins.join(","),
        out_object(v),
        v.trace.iter().map(|t| format!("\"{}\"", esc(t))).collect::<Vec<_>>().join(","),
        esc(&v.why)
    )
}

/// The JSON object of the outputs, in declaration order (not the name order of a BTreeMap),
/// so a reader can match it by eye against the `outputs` lines of the rule source.
fn out_object(v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let one = |o: &Option<Val>| match o {
        Some(Val::Num(r)) => format!("{}", r.num / r.den),
        Some(Val::Bool(b)) => format!("{b}"),
        Some(other) => format!("\"{}\"", esc(&show(other))),
        None => "null".into(),
    };
    let body: Vec<String> =
        v.outputs.iter().map(|(n, val)| format!("\"{}\":{}", esc(n), one(val))).collect();
    format!("{{{}}}", body.join(","))
}

/// Only the expected values, in the same shape the runner emits. Three-way agreement is judged
/// on these bytes.
pub fn expected_json(_f: &RuleFile, v: &Vector) -> String {
    out_object(v)
}

// ── The mapping of §9.1 ─────────────────────────────────────────────────────
//
// A column decided by a derived value, a definition or an upstream table is not an input, so
// its value cannot be set directly. The design says "solve the linear expression for one input
// and map it onto the input vector". Here nothing is solved symbolically: the slope is taken by
// numeric differentiation, the solution is computed from it, and then **always re-evaluated to
// confirm**. If the expression is not linear the solution will not match, and it is discarded
// then. A symbolic solver would only work for linear expressions, whereas in this form the
// evaluator answers "did it take effect" even with an upstream table or definition in between,
// so this one mechanism suffices.

/// Run an assignment and get the bindings, including derived values, definitions and
/// intermediate outputs.
fn bind(f: &RuleFile, c: &Checked, a: &BTreeMap<String, Val>) -> BTreeMap<String, Val> {
    let (_, _, b) = eval::run_bindings(f, c, a.clone().into_iter().collect());
    b.into_iter().collect()
}

fn as_rat(v: &Val) -> Option<Rat> {
    match v {
        Val::Num(r) => Some(*r),
        Val::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

fn to_val(r: Rat, ty: &Ty) -> Val {
    if matches!(ty, Ty::Date) {
        let (y, m, d) = crate::types::ord_to_date(r);
        Val::Date(y, m, d)
    } else {
        Val::Num(r)
    }
}

fn within(c: &Checked, col: &str, v: Rat) -> bool {
    let Some((lo, hi)) = c.ranges.get(col) else { return true };
    lo.is_none_or(|l| v.cmp_to(l) != std::cmp::Ordering::Less)
        && hi.is_none_or(|h| v.cmp_to(h) != std::cmp::Ordering::Greater)
}

/// Build from `seed` an assignment that sets column `col` to `target`. An input is set directly;
/// a derived value is solved through one chosen input. **Exactly one input changes**, and the
/// choice is fixed to the declaration order of the inputs, so the same input moves for the
/// inside and the outside point. The "boundary-pair coverage" of §9.2 demands a vector pair,
/// which is why this is needed.
fn place(
    f: &RuleFile,
    c: &Checked,
    seed: &BTreeMap<String, Val>,
    col: &str,
    target: Rat,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    if seed.contains_key(col) {
        if !within(c, col, target) {
            return None;
        }
        let mut a = seed.clone();
        a.insert(col.into(), to_val(target, &ty));
        return Some(a);
    }
    let e0 = as_rat(bind(f, c, seed).get(col)?)?;
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        let xty = c.ty_of(&x)?;
        let Some(x0) = seed.get(&x).and_then(as_rat) else { continue };
        let q = crate::coverage::quantum(c, &x, &xty);
        let mut probe = seed.clone();
        probe.insert(x.clone(), to_val(x0.add(q), &xty));
        let Some(e1) = bind(f, c, &probe).get(col).and_then(as_rat) else { continue };
        let slope = e1.sub(e0).div(q);
        if slope.num == 0 {
            continue;
        }
        let xn = x0.add(target.sub(e0).div(slope));
        // Discard a solution that is off the input's grid (an input in whole yen never gets
        // 0.5 yen).
        if !xn.div(q).is_int() || !within(c, &x, xn) {
            continue;
        }
        let mut a = seed.clone();
        a.insert(x.clone(), to_val(xn, &xty));
        // A non-linear expression fails here: confirmed by evaluation, not by guessing.
        if bind(f, c, &a).get(col).and_then(as_rat).is_some_and(|v| v.cmp_to(target) == std::cmp::Ordering::Equal) {
            return Some(a);
        }
    }
    None
}

/// Move one step toward satisfying a single cell. Numbers are solved for and hit exactly;
/// anything else sweeps the inputs.
fn satisfy_cell(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    col: &str,
    cell: &Cell,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    let binds = bind(f, c, seed);
    if let Some(v) = binds.get(col) {
        if eval::cell_matches(c, cell, v, &ty) {
            return Some(seed.clone());
        }
    }
    // Number or date: solve for a point inside the cell.
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
        let q = crate::coverage::quantum(c, col, &ty);
        for (_, inside, _) in crate::coverage::thresholds_pub(cell, &ty, q) {
            if let Some(a) = place(f, c, seed, col, inside) {
                return Some(a);
            }
        }
        return None;
    }
    // Enum or boolean: sweep the inputs one at a time and take the first that matches (in a
    // deterministic order).
    if let Some(vs) = cands.get(col) {
        for v in vs {
            if eval::cell_matches(c, cell, v, &ty) {
                let mut a = seed.clone();
                a.insert(col.into(), v.clone());
                return Some(a);
            }
        }
        return None;
    }
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        for v in cands.get(&x).into_iter().flatten() {
            let mut a = seed.clone();
            a.insert(x.clone(), v.clone());
            if bind(f, c, &a).get(col).is_some_and(|got| eval::cell_matches(c, cell, got, &ty)) {
                return Some(a);
            }
        }
    }
    None
}

/// An assignment that satisfies every cell of the row. Columns already satisfied are left alone.
fn reach_row(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    t: &Table,
    row: &Row,
) -> Option<BTreeMap<String, Val>> {
    let mut a = seed.clone();
    // Two passes: fixing an earlier column can break a later one, so confirm that one pass was
    // enough before returning.
    for _ in 0..2 {
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let Some(cell) = row.cells.get(ci) else { continue };
            if matches!(cell, Cell::DontCare) {
                continue;
            }
            a = satisfy_cell(f, c, cands, &a, col, cell)?;
        }
        if row_holds(f, c, t, row, &a) {
            return Some(a);
        }
    }
    None
}

fn row_holds(f: &RuleFile, c: &Checked, t: &Table, row: &Row, a: &BTreeMap<String, Val>) -> bool {
    let binds = bind(f, c, a);
    t.inputs.iter().enumerate().all(|(ci, (col, _))| {
        let Some(cell) = row.cells.get(ci) else { return true };
        if matches!(cell, Cell::DontCare) {
            return true;
        }
        let (Some(v), Some(ty)) = (binds.get(col), c.ty_of(col)) else { return false };
        eval::cell_matches(c, cell, v, &ty)
    })
}

/// An assignment that makes row `ri` **win**. Under `first` satisfying its cells is not enough:
/// the earlier rows must be knocked out, and only through columns this row leaves as `-` while
/// the earlier row names them (moving a column row `ri` names would break `ri` itself).
fn win_row(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    t: &Table,
    ri: usize,
) -> Option<BTreeMap<String, Val>> {
    let tag = eval::row_tag(&t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(), ri + 1);
    let mut a = reach_row(f, c, cands, seed, t, &t.rows[ri])?;
    for _ in 0..t.rows.len() + 1 {
        let (_, fired, _) = eval::run_bindings(f, c, a.clone().into_iter().collect());
        if fired.contains(&tag) {
            return Some(a);
        }
        // Knock out one row that currently wins ahead of it.
        let mut moved = false;
        for e in 0..ri {
            if !row_holds(f, c, t, &t.rows[e], &a) {
                continue;
            }
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if matches!(t.rows[ri].cells.get(ci), Some(Cell::DontCare) | None) {
                    let Some(ecell) = t.rows[e].cells.get(ci) else { continue };
                    if matches!(ecell, Cell::DontCare) {
                        continue;
                    }
                    if let Some(b) = violate_cell(f, c, cands, &a, col, ecell) {
                        if row_holds(f, c, t, &t.rows[ri], &b) {
                            a = b;
                            moved = true;
                            break;
                        }
                    }
                }
            }
            if moved {
                break;
            }
        }
        if !moved {
            return None;
        }
    }
    None
}

/// Move one step toward **violating** a cell: solve for the outside of a boundary, or put in a
/// candidate that does not match.
fn violate_cell(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    col: &str,
    cell: &Cell,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
        let q = crate::coverage::quantum(c, col, &ty);
        for (_, _, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
            if let Some(a) = place(f, c, seed, col, outside) {
                if !eval::cell_matches(c, cell, &to_val(outside, &ty), &ty) {
                    return Some(a);
                }
            }
        }
        return None;
    }
    if let Some(vs) = cands.get(col) {
        for v in vs {
            if !eval::cell_matches(c, cell, v, &ty) {
                let mut a = seed.clone();
                a.insert(col.into(), v.clone());
                return Some(a);
            }
        }
        return None;
    }
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        for v in cands.get(&x).into_iter().flatten() {
            let mut a = seed.clone();
            a.insert(x.clone(), v.clone());
            if bind(f, c, &a).get(col).is_some_and(|got| !eval::cell_matches(c, cell, got, &ty)) {
                return Some(a);
            }
        }
    }
    None
}

/// The "for a comparison between names, the tie A = B and both sides of it" of §9.1.
/// An atom such as `A期限 <= B期限` inside a definition has no literal boundary, so nothing of
/// it reaches `numeric_bounds`; here it is stepped on directly as a pair of inputs.
fn name_pairs(f: &RuleFile) -> Vec<(String, String)> {
    fn walk(e: &Expr, out: &mut Vec<(String, String)>) {
        match e {
            Expr::Bin(l, op, r, _)
                if matches!(op, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt | BinOp::Eq) =>
            {
                if let (Expr::Name(a, _), Expr::Name(b, _)) = (&**l, &**r) {
                    out.push((a.clone(), b.clone()));
                }
                walk(l, out);
                walk(r, out);
            }
            Expr::Bin(l, _, r, _) => {
                walk(l, out);
                walk(r, out);
            }
            Expr::Call(_, args, _) => args.iter().for_each(|a| walk(a, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for it in &f.items {
        match it {
            Item::Define(d) => walk(&d.expr, &mut out),
            Item::Derived(d) => walk(&d.expr, &mut out),
            _ => {}
        }
    }
    out
}

// ── §6.2 "witness on a definition axis" ─────────────────────────────────────
//
// The region analysis treats boolean definitions as free axes, so a coordinate of an
// intersection box only says "the definition is true" without checking that an input making
// it true exists. Here an input is actually constructed, and the evaluator computes the
// definitions to confirm it. Only an overlap that could be constructed is a confirmed, real
// contradiction (E105 under `unique`); one that could not is demoted to W114 and the runtime
// guard. Mechanically dropping to Unknown on shared dependencies alone is not done (§8.5).

/// The lower and upper bound of the values a column may take; `None` means unbounded. A cell
/// that cannot be analysed returns `None`.
fn cell_span(cell: &Cell, ty: &Ty, q: Rat) -> Option<(Option<Rat>, Option<Rat>)> {
    let lit = |l: &Lit| -> Option<Rat> {
        match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty),
            Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
            _ => None,
        }
    };
    match cell {
        Cell::DontCare => Some((None, None)),
        Cell::Lit(l) => lit(l).map(|b| (Some(b), Some(b))),
        Cell::Cmp(cs) => {
            let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = (None, None);
            for (op, l) in cs {
                let b = lit(l)?;
                let (nl, nh) = match op {
                    CmpOp::Ge => (Some(b), None),
                    CmpOp::Gt => (Some(b.add(q)), None),
                    CmpOp::Le => (None, Some(b)),
                    CmpOp::Lt => (None, Some(b.sub(q))),
                };
                if let Some(x) = nl {
                    lo = Some(lo.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Greater { x } else { c }));
                }
                if let Some(x) = nh {
                    hi = Some(hi.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Less { x } else { c }));
                }
            }
            Some((lo, hi))
        }
        _ => None,
    }
}

/// Move column `col` one step toward a value that satisfies **all** of the given cells.
fn satisfy_all(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    col: &str,
    cells: &[&Cell],
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    let hit = |v: &Val| cells.iter().all(|cell| eval::cell_matches(c, cell, v, &ty));
    if bind(f, c, seed).get(col).is_some_and(hit) {
        return Some(seed.clone());
    }
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
        // Intersect the intervals. If it is empty, the pair cannot hold at once on this column.
        let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = match c.ranges.get(col) {
            Some((l, h)) => (*l, *h),
            None => (None, None),
        };
        let q = crate::coverage::quantum(c, col, &ty);
        for cell in cells {
            let (l, h) = cell_span(cell, &ty, q)?;
            if let Some(x) = l {
                lo = Some(lo.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Greater { x } else { c }));
            }
            if let Some(x) = h {
                hi = Some(hi.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Less { x } else { c }));
            }
        }
        if let (Some(l), Some(h)) = (lo, hi) {
            if l.cmp_to(h) == std::cmp::Ordering::Greater {
                return None;
            }
        }
        for target in [lo, hi].into_iter().flatten() {
            if let Some(a) = place(f, c, seed, col, target) {
                if bind(f, c, &a).get(col).is_some_and(hit) {
                    return Some(a);
                }
            }
        }
        return None;
    }
    if let Some(vs) = cands.get(col) {
        let v = vs.iter().find(|v| hit(v))?;
        let mut a = seed.clone();
        a.insert(col.into(), v.clone());
        return Some(a);
    }
    // A column decided by an upstream table or definition: sweep the inputs one at a time and
    // take the first that gets there (in a deterministic order).
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        for v in cands.get(&x).into_iter().flatten() {
            let mut a = seed.clone();
            a.insert(x.clone(), v.clone());
            if bind(f, c, &a).get(col).is_some_and(hit) {
                return Some(a);
            }
        }
    }
    None
}

/// Actually construct an input that matches **both** row i and row j (§6.2, "witness on a
/// definition axis"). If one is found the overlap is real, and E105 under `unique`. If none is
/// found it stays unconfirmed and is demoted to W114 and the runtime guard. **Not finding one
/// is no proof of non-existence**, so the caller does not assert that.
pub fn pair_witness(
    f: &RuleFile,
    c: &Checked,
    t: &Table,
    i: usize,
    j: usize,
) -> Option<BTreeMap<String, Val>> {
    let cands = candidates(f, c);
    let mut a: BTreeMap<String, Val> = f
        .inputs
        .iter()
        .map(|x| Some((x.name.text.clone(), cands.get(&x.name.text)?.first()?.clone())))
        .collect::<Option<BTreeMap<_, _>>>()?;
    let hold = |a: &BTreeMap<String, Val>| {
        row_holds(f, c, t, &t.rows[i], a) && row_holds(f, c, t, &t.rows[j], a)
    };
    // Bring the columns in one by one. Fixing one can break an earlier one, so loop until
    // nothing breaks any more.
    for _ in 0..3 {
        if hold(&a) {
            return Some(a);
        }
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let cells: Vec<&Cell> = [i, j]
                .iter()
                .filter_map(|&r| t.rows[r].cells.get(ci))
                .filter(|cell| !matches!(cell, Cell::DontCare))
                .collect();
            if cells.is_empty() {
                continue;
            }
            a = satisfy_all(f, c, &cands, &a, col, &cells)?;
        }
    }
    hold(&a).then_some(a)
}
