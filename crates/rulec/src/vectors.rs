//! Test-case generation from boundaries (§9).
//!
//! Candidate values are chosen per column, the candidate population built from them is run
//! through the reference evaluator, and a subset satisfying row coverage, boundary-pair
//! coverage, shadow-pair coverage and rounding-tie coverage is selected deterministically.
//! The evaluator attaches
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
    // An element's fields are table columns like any other, so their candidates come out of
    // the same equivalence classes the cells induce (§15.56).
    for i in f.inputs.iter().chain(f.elements.iter().flat_map(|e| &e.fields)) {
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
            // §9.1 on a `string` column: one representative per class the prefixes cut it
            // into — the prefix itself for each one, and one string under none of them
            // (§15.101).
            Ty::Str => {
                let mut ps: Vec<String> = Vec::new();
                for cell in column_cells(f, name) {
                    if let Cell::Prefix(xs) = cell {
                        for x in xs {
                            if !ps.contains(&x) {
                                ps.push(x.clone());
                            }
                        }
                    }
                }
                for p in &ps {
                    vs.push(Val::Str(p.clone()));
                    vs.push(Val::Str(format!("{p}0001")));
                }
                vs.push(Val::Str(outside_prefixes(&ps)));
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
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
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
        // values take part in the comparison across implementations (§8.5).
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
        Ty::Str => Val::Str(String::new()),
        _ => Val::Num(Rat::zero()),
    }
}

/// A string none of the prefixes is a prefix of.
fn outside_prefixes(ps: &[String]) -> String {
    for c in "zxqjk".chars() {
        let s = c.to_string();
        if !ps.iter().any(|p| s.starts_with(p.as_str()) || p.is_empty()) {
            return s;
        }
    }
    let n = ps.iter().map(|p| p.chars().count()).max().unwrap_or(0);
    "z".repeat(n + 1)
}

/// Every cell that appears in one column of any table of the rule.
fn column_cells(f: &RuleFile, col: &str) -> Vec<Cell> {
    let mut out = Vec::new();
    for t in f.items.iter().filter_map(|it| if let Item::Table(t) = it { Some(t) } else { None }) {
        let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
        for row in &t.rows {
            if let Some(cell) = row.cells.get(ci) {
                out.push(cell.clone());
            }
        }
    }
    out
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
            // A count declares no cells; what it counts is tested inside the walk.
            Item::Agg(_) => {}
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
            Item::Agg(_) => {}
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
                            // Each member of a set is a boundary of its own.
                            Some(Cell::Set(ls) | Cell::Not(ls)) => {
                                for l in ls {
                                    if let Some(v) = lit(l) {
                                        push(v, &mut set);
                                    }
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
/// Every input at its first candidate. One column at a time is moved off it (§9.1), so a value
/// here that annihilates the arithmetic downstream — a rate of 0% — leaves the whole boundary
/// family computing zero. That is what `tie_plan` is for: it does not sweep from the baseline.
fn baseline(f: &RuleFile, cands: &BTreeMap<String, Vec<Val>>) -> BTreeMap<String, Val> {
    f.inputs
        .iter()
        .map(|i| i.name.text.clone())
        .map(|n| (n.clone(), cands[&n].first().cloned().unwrap()))
        .collect()
}

/// How far out from the baseline a tie is looked for before giving up.
const TIE_REACH: i128 = 64;

/// The rounding tie of each output that declares one: the value exactly half a step off the
/// grid, which is the single point where `half_up`, `half_down` and `half_even` disagree, and
/// where `up` and `down` disagree too. `round` is mandatory (E104) yet none of the three §9.2
/// criteria aims at it, so a rule may carry two different modes on two outputs and no vector
/// ever tell them apart — the 50銭 of the social insurance tables is exactly that case.
///
/// Returns the assignment that reaches the tie, or nothing for an output the rule can never
/// take off the grid (18.3% of a standard monthly remuneration is always an even number of yen,
/// so the halved amount has no fraction and there is no tie to reach). The auditor asks the
/// same question, so the two sides agree on which obligations exist.
pub fn tie_plan(f: &RuleFile, c: &Checked) -> Vec<(String, Rat, BTreeMap<String, Val>)> {
    let cands = candidates(f, c);
    let base = baseline(f, &cands);
    let mut out = Vec::new();
    for od in &f.outputs {
        let Some(rd) = &od.rounding else { continue };
        let name = od.name.text.clone();
        let Some(ty) = c.ty_of(&name) else { continue };
        let Some(g) = crate::types::lit_value_in_pub(&rd.grid, &ty) else { continue };
        if g.cmp_to(Rat::zero()) == std::cmp::Ordering::Equal {
            continue;
        }
        let half = g.div(Rat::int(2));
        let Some(v0) = bind(f, c, &base).get(&name).and_then(as_rat) else { continue };
        // Start at the tie nearest the value the baseline already produces and walk outward.
        // Every candidate is confirmed by `place`, which re-evaluates the rule, so a non-linear
        // expression fails to place rather than placing wrongly.
        let floor0 = v0.round_to(crate::num::RoundMode::Down, g);
        let mut found = None;
        'search: for step in 0..TIE_REACH {
            for dir in [1i128, -1] {
                if step == 0 && dir == -1 {
                    continue;
                }
                let t = floor0.add(g.mul(Rat::int(dir * step))).add(half);
                if let Some(a) = place(f, c, &base, &name, t, &BTreeSet::new()) {
                    found = Some((t, a));
                    break 'search;
                }
            }
        }
        if let Some((_, a)) = found {
            out.push((name, g, a));
        }
    }
    out
}

/// Is `v` exactly half a step off `grid`? The auditor's question, and the one place the five
/// rounding modes are told apart.
pub fn is_tie(v: Rat, grid: Rat) -> bool {
    if grid.cmp_to(Rat::zero()) == std::cmp::Ordering::Equal {
        return false;
    }
    let below = v.round_to(crate::num::RoundMode::Down, grid);
    v.sub(below).cmp_to(grid.div(Rat::int(2))) == std::cmp::Ordering::Equal
}

/// The cases a walk has, as opposed to the cases one element has (§15.56).
///
/// The fold is an automaton over the verdicts, so what has to be covered is its transitions,
/// not the length of the sequence: nothing, one element on each verdict, and every **ordered
/// pair** of verdicts. That is what tells `take_first` from `take_unique`, shows `keep_max`
/// replacing and not replacing, and puts an element after a `stop` to prove it is not read.
fn fold_pool(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
) -> Vec<(BTreeMap<String, Val>, String)> {
    let fold = f.fold.as_ref().expect("a rule with elements has a fold");
    let fields: Vec<String> =
        f.elements.iter().flat_map(|e| &e.fields).map(|i| i.name.text.clone()).collect();
    let scalars: BTreeMap<String, Val> = baseline(f, cands)
        .into_iter()
        .filter(|(k, _)| !fields.contains(k))
        .collect();

    // One element per verdict, built from the fields' own candidates: walk the cross product
    // and keep the first that lands on each verdict.
    let mut per_verdict: BTreeMap<String, BTreeMap<String, Val>> = BTreeMap::new();
    let mut combos: Vec<BTreeMap<String, Val>> = vec![BTreeMap::new()];
    for name in &fields {
        let Some(vs) = cands.get(name) else { continue };
        let mut next = Vec::new();
        for base in &combos {
            for v in vs {
                let mut m = base.clone();
                m.insert(name.clone(), v.clone());
                next.push(m);
            }
            if next.len() > 4096 {
                break;
            }
        }
        combos = next;
    }
    for e in &combos {
        let mut m: std::collections::HashMap<String, Val> = scalars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, v) in e {
            m.insert(k.clone(), v.clone());
        }
        // One element, run through the tables the way any case is run: the verdict is what
        // the per-element table wrote, read out of the evaluator's own environment.
        let (_, _, _, vals) = crate::eval::run_tables(f, c, m);
        if let Some(Val::Enum(v)) = vals.get(&fold.verdict) {
            // Keep the element whose numbers are largest, not the first one found. The first
            // is every field at the low end of its range, so `take` and `keep_max` would all
            // answer with the same zero and an emitter that answered with the wrong value
            // would still match (§15.9: an obligation nothing distinguishes is not one).
            let weight = |e: &BTreeMap<String, Val>| -> Rat {
                e.values().fold(Rat::zero(), |acc, x| match x {
                    Val::Num(r) => acc.add(*r),
                    _ => acc,
                })
            };
            match per_verdict.get(v) {
                Some(old) if weight(old).cmp_to(weight(e)) != std::cmp::Ordering::Less => {}
                _ => {
                    per_verdict.insert(v.clone(), e.clone());
                }
            }
        }
    }

    let seq = |xs: Vec<&BTreeMap<String, Val>>| -> Val {
        Val::Seq(xs.into_iter().cloned().collect())
    };

    let case = |elements: Val, why: String| -> (BTreeMap<String, Val>, String) {
        let mut m = scalars.clone();
        m.insert(fold.over.clone(), elements);
        (m, why)
    };

    let mut out = vec![case(Val::Seq(Vec::new()), tr!("要素ゼロ件", "no elements"))];
    // Every candidate element on its own. The boundaries of an element's fields are covered
    // exactly the way an input's are — one element is one case — and the audit keeps the ones
    // that discharge an obligation.
    for e in &combos {
        out.push(case(seq(vec![e]), tr!("要素の候補", "an element's candidates")));
    }
    for (v, e) in &per_verdict {
        out.push(case(seq(vec![e]), tr!("判定 {v} 一件", "one element on {v}")));
    }
    for (a, ea) in &per_verdict {
        for (b, eb) in &per_verdict {
            out.push(case(seq(vec![ea, eb]), tr!("判定 {a} のあとに {b}", "{a} then {b}")));
        }
    }
    // The scalars move too: each of their boundary cases, carried by one element of each
    // verdict, so a scalar that decides a cell is still exercised.
    for (a, why) in pool_scalars(f, c, cands, &fields) {
        for (v, e) in &per_verdict {
            let mut m = a.clone();
            m.insert(fold.over.clone(), seq(vec![e]));
            out.push((m, tr!("{why}（判定 {v}）", "{why} (verdict {v})")));
        }
    }
    out
}

/// The pool as it is for a rule with no sequence, with the element fields left out.
fn pool_scalars(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    fields: &[String],
) -> Vec<(BTreeMap<String, Val>, String)> {
    pool_inner(f, c, cands)
        .into_iter()
        .map(|(mut a, why)| {
            a.retain(|k, _| !fields.contains(k));
            (a, why)
        })
        .collect()
}

fn pool(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>) -> Vec<(BTreeMap<String, Val>, String)> {
    if f.fold.is_some() {
        return fold_pool(f, c, cands);
    }
    if f.items.iter().any(|it| matches!(it, Item::Agg(_))) {
        return count_pool(f, c, cands);
    }
    pool_inner(f, c, cands)
}

/// The cases a rule that counts needs (§15.58).
///
/// What the count feeds is an ordinary table, so what has to be covered is the values that
/// table's cells care about — and the only way to reach a count of *n* is to pass *n*
/// elements the test accepts. So the pool carries, for each count, a sequence of every
/// length up to one past the largest number any cell names, made of elements that count.
/// The elements themselves are covered the way a fold covers them: each candidate on its
/// own, so the per-element table's rows and boundaries are exercised.
fn count_pool(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
) -> Vec<(BTreeMap<String, Val>, String)> {
    let counts: Vec<&crate::ast::AggDecl> = f
        .items
        .iter()
        .filter_map(|it| if let Item::Agg(d) = it { Some(d) } else { None })
        .collect();
    let el = f.elements.as_ref().expect("a count has elements");
    let over = el.name.text.clone();
    let fields: Vec<String> = el.fields.iter().map(|i| i.name.text.clone()).collect();
    let scalars: BTreeMap<String, Val> =
        baseline(f, cands).into_iter().filter(|(k, _)| !fields.contains(k)).collect();

    // Every element the fields' own candidates can make, as the fold pool builds them.
    let mut combos: Vec<BTreeMap<String, Val>> = vec![BTreeMap::new()];
    for name in &fields {
        let Some(vs) = cands.get(name) else { continue };
        let mut next = Vec::new();
        for base in &combos {
            for v in vs {
                let mut m = base.clone();
                m.insert(name.clone(), v.clone());
                next.push(m);
            }
            if next.len() > 4096 {
                break;
            }
        }
        combos = next;
    }

    let case = |elements: Val, why: String| -> (BTreeMap<String, Val>, String) {
        let mut m = scalars.clone();
        m.insert(over.clone(), elements);
        (m, why)
    };
    let seq = |xs: Vec<&BTreeMap<String, Val>>| -> Val {
        Val::Seq(xs.into_iter().cloned().collect())
    };

    // A sum is reached by its **total**, not by a number of elements: one element carrying
    // the whole amount where the field's range allows it, and as many as it takes where it
    // does not (§15.100). `None` when the total cannot be built at all — the field has no
    // range, or the amount is negative.
    let total_seq = |d: &crate::ast::AggDecl, want: Rat| -> Option<Val> {
        let base = combos.first()?.clone();
        let (lo, hi) = c.ranges.get(&d.column.text).copied()?;
        let (lo, hi) = (lo?, hi?);
        if want.cmp_to(Rat::zero()) == std::cmp::Ordering::Less || hi.cmp_to(Rat::zero()) != std::cmp::Ordering::Greater {
            return None;
        }
        let mut left = want;
        let mut xs: Vec<BTreeMap<String, Val>> = Vec::new();
        while left.cmp_to(Rat::zero()) == std::cmp::Ordering::Greater && xs.len() < 64 {
            let take = if left.cmp_to(hi) == std::cmp::Ordering::Greater { hi } else { left };
            if take.cmp_to(lo) == std::cmp::Ordering::Less {
                // The remainder is below what one element may carry; spread it instead.
                return None;
            }
            let mut e = base.clone();
            e.insert(d.column.text.clone(), Val::Num(take));
            xs.push(e);
            left = left.sub(take);
        }
        if left.cmp_to(Rat::zero()) != std::cmp::Ordering::Equal {
            return None;
        }
        Some(Val::Seq(xs))
    };

    let mut out = vec![case(Val::Seq(Vec::new()), tr!("要素ゼロ件", "no elements"))];
    for e in &combos {
        out.push(case(seq(vec![e]), tr!("要素の候補", "an element's candidates")));
    }

    // The counted element and the cap of each count, kept for the row-wise sweep below.
    let mut counted: Vec<(&crate::ast::AggDecl, &BTreeMap<String, Val>)> = Vec::new();
    let mut summed: Vec<&crate::ast::AggDecl> = Vec::new();

    for d in &counts {
        // An element this count accepts, and one it does not.
        let mut hit: Option<&BTreeMap<String, Val>> = None;
        let mut miss: Option<&BTreeMap<String, Val>> = None;
        for e in &combos {
            let mut m: std::collections::HashMap<String, Val> =
                scalars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            for (k, v) in e {
                m.insert(k.clone(), v.clone());
            }
            let (_, _, _, vals) = crate::eval::run_tables(f, c, m);
            let v = vals.get(&d.column.text);
            let counted = match (v, &d.value) {
                (Some(Val::Bool(b)), None) => *b,
                (Some(Val::Bool(b)), Some(w)) => *b == (w.text == crate::kw::TRUE),
                (Some(Val::Enum(x)), Some(w)) => *x == w.text,
                _ => false,
            };
            if counted {
                hit.get_or_insert(e);
            } else {
                miss.get_or_insert(e);
            }
        }
        // A sum reads a number off every element, so there is no "an element it accepts";
        // what it needs is a total, built above.
        if d.kind == crate::ast::AggKind::Sum {
            summed.push(d);
            for b in numeric_bounds(f, &d.name.text, &c.ty_of(&d.name.text).unwrap_or(Ty::Number), c) {
                for want in [b, b.sub(Rat::int(1)), b.add(Rat::int(1))] {
                    if !crate::coverage::in_range(c, &d.name.text, want) {
                        continue;
                    }
                    if let Some(xs) = total_seq(d, want) {
                        out.push(case(xs, tr!("{} が {} の並び", "a sequence whose {} is {}", d.name.text, want.num / want.den)));
                    }
                }
            }
            continue;
        }
        let Some(hit) = hit else { continue };
        counted.push((d, hit));
        // One past the largest number the cells name, held to what the range allows: that is
        // the first count on the far side of every boundary the tables draw.
        let top = numeric_bounds(f, &d.name.text, &Ty::Number, c)
            .iter()
            .map(|r| r.num / r.den)
            .max()
            .unwrap_or(1);
        let cap = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi).map(|h| h.num / h.den).unwrap_or(top + 1);
        let top = (top + 1).min(cap).max(1);
        for n in 1..=top {
            let xs: Vec<&BTreeMap<String, Val>> = (0..n).map(|_| hit).collect();
            out.push(case(seq(xs), tr!("{} が {n} 件", "{} of them: {n}", d.name.text)));
        }
        // One that counts and one that does not, so the walk is shown skipping an element
        // rather than counting everything it sees.
        if let Some(miss) = miss {
            out.push(case(seq(vec![hit, miss]), tr!("{} に入るものと入らないもの", "one counted and one not, for {}", d.name.text)));
        }
    }

    // A table downstream of a count is an ordinary table, so its boundary pairs have to be
    // walked with the row's other columns held (§9.2). The sweep above moves the count with the
    // baseline scalars only, which never lands on the far side of a boundary drawn in a row the
    // baseline does not satisfy — `納入先照合` 行3 (`| 1 | false |`) had no vector at all with
    // `自動確定可 = false` and a count of 0 or 2. `place` cannot help: a count is not an input,
    // so there is nothing to place a number on. The pair is built here instead, out of the row's
    // own assignment for the other columns and a sequence of exactly that many counted elements.
    for set in &c.sets {
        let t = &set.table;
        for (ri, row) in t.rows.iter().enumerate() {
            let tname = set.row_table(ri).to_string();
            let rn = row.index;
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let found = counted.iter().find(|(d, _)| d.name.text == *col).map(|&(d, h)| (d, Some(h)));
                let found = found.or_else(|| summed.iter().find(|d| d.name.text == *col).map(|&d| (d, None)));
                let Some((d, hit)) = found else { continue };
                let Some(cell) = row.cells.get(ci) else { continue };
                let ty = c.ty_of(col).unwrap_or(Ty::Number);
                // The row's other columns, as the row-target seed builds them. A column another
                // count decides is left alone: there is no one length that sets two counts.
                let mut a = scalars.clone();
                for (cj, (other, _)) in t.inputs.iter().enumerate() {
                    if cj == ci
                        || fields.contains(other)
                        || counted.iter().any(|(e, _)| e.name.text == *other)
                        || summed.iter().any(|e| e.name.text == *other)
                    {
                        continue;
                    }
                    let (Some(oc), Some(ovs)) = (row.cells.get(cj), cands.get(other)) else {
                        continue;
                    };
                    let oty = c.ty_of(other).unwrap_or(Ty::Unknown);
                    if let Some(v) = satisfying(oc, ovs, &oty, c) {
                        a.insert(other.clone(), v);
                    }
                }
                let q = crate::coverage::quantum(c, col, &ty);
                for (b, inside, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
                    let b = crate::coverage::show_rat(b, &ty);
                    let why = tr!(
                        "境界両側: 表 {tname} 行{} {col} {b} の",
                        "boundary pair: table {tname} row {} {col} {b}",
                        rn
                    );
                    for (n, side) in [
                        (inside, tr!("{why}内側", "{why} inside")),
                        (outside, tr!("{why}外側", "{why} outside")),
                    ] {
                        // A count is a whole number of elements, inside the range it declared —
                        // which is also the cap on how long a sequence may be.
                        if !crate::coverage::in_range(c, &d.name.text, n) {
                            continue;
                        }
                        let mut m = a.clone();
                        match hit {
                            // A count is a whole number of elements.
                            Some(hit) => {
                                if n.den != 1 || n.num < 0 {
                                    continue;
                                }
                                m.insert(over.clone(), seq((0..n.num).map(|_| hit).collect()));
                            }
                            // A sum is a total, which one element can carry.
                            None => match total_seq(d, n) {
                                Some(xs) => {
                                    m.insert(over.clone(), xs);
                                }
                                None => continue,
                            },
                        }
                        out.push((m, side));
                    }
                }
            }
        }
    }

    // The scalars move too, carried by one element each, so a scalar that decides a cell is
    // still exercised.
    for (a, why) in pool_scalars(f, c, cands, &fields) {
        if let Some(e) = combos.first() {
            let mut m = a.clone();
            m.insert(over.clone(), seq(vec![e]));
            out.push((m, why));
        }
    }
    out
}

fn pool_inner(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>) -> Vec<(BTreeMap<String, Val>, String)> {
    let names: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let base = baseline(f, cands);
    let mut out: Vec<(BTreeMap<String, Val>, String)> = vec![(base.clone(), tr!("基準", "baseline"))];

    // Row coverage: build an assignment that makes each row win. Columns holding derived or
    // intermediate values are mapped back onto the inputs (§9.1). Naively "putting a value that
    // satisfies the input cell" never touches a column a derived value decides, and the row
    // stays beaten by an earlier row under `first`. That showed up in the coverage auditor as
    // missing row coverage (クーポン併用 row 3, 適用順序 row 4, 素の割引 row 3).
    for set in &c.sets {
        let t = &set.table;
        for (ri, row) in t.rows.iter().enumerate() {
            let tname = set.row_table(ri).to_string();
            let rn = row.index;
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
            let seed = win_row(f, c, cands, &a, set, ri).unwrap_or(a);
            out.push((seed.clone(), tr!("行を当てる: 表 {tname} 行{}", "row target: table {tname} row {}", rn)));

            // Boundary-pair coverage: build a **pair** that steps on both sides of a boundary
            // with the row's other columns held fixed. `place` picks inputs in declaration order
            // so that the same input moves for the inside and the outside point.
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let Some(ty) = c.ty_of(col) else { continue };
                if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let q = crate::coverage::quantum(c, col, &ty);
                for (b, inside, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
                    let (Some(ain), Some(aout)) =
                        (
                            place(f, c, &seed, col, inside, &BTreeSet::new()),
                            place(f, c, &seed, col, outside, &BTreeSet::new()),
                        )
                    else {
                        continue;
                    };
                    let b = crate::coverage::show_rat(b, &ty);
                    let why = tr!(
                        "境界両側: 表 {tname} 行{} {col} {b} の",
                        "boundary pair: table {tname} row {} {col} {b}",
                        rn
                    );
                    out.push((ain, tr!("{why}内側", "{why} inside")));
                    out.push((aout, tr!("{why}外側", "{why} outside")));
                }
            }
        }

        // Shadow-pair coverage: the inside of the intersection (a point where row i wins while
        // row j's conditions hold too). The pairs are the ones the precedence relation orders
        // — the earlier rows of a `first` table, and the rows of a table this one takes
        // precedence over (§15.66).
        for j in 1..t.rows.len() {
            for &i in &set.beats[j] {
                let Some(a) = reach_row(f, c, cands, &base, t, &t.rows[j], &BTreeSet::new())
                else {
                    continue;
                };
                let Some(a) = win_row(f, c, cands, &a, set, i) else { continue };
                if row_holds(f, c, t, &t.rows[j], &a) {
                    let (ti, tj) = (set.row_table(i), set.row_table(j));
                    let (ni, nj) = (t.rows[i].index, t.rows[j].index);
                    out.push((
                        a,
                        if ti == tj {
                            tr!("隠れ対: 表 {ti} 行{ni}∩行{nj}", "shadow pair: table {ti} row {ni} ∩ row {nj}")
                        } else {
                            tr!("隠れ対: 表 {ti} 行{ni}∩表 {tj} 行{nj}", "shadow pair: table {ti} row {ni} ∩ table {tj} row {nj}")
                        },
                    ));
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
            if let Some(a) = place(f, c, &base, &y, t, &BTreeSet::new()) {
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

    // The rounding tie (§9.2). Unlike the sweep above, this does not hold the other columns at
    // the baseline: `place` moves whatever it needs to land the value half a step off the grid.
    for (name, _, a) in tie_plan(f, c) {
        out.push((a, tr!("丸めの同着: {name}", "rounding tie: {name}")));
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

/// A value as one short string. It is also used as a deduplication key, so it has to tell
/// two different values apart: a rate is a fraction, and truncating it to an integer made
/// every rate under 100% look like 0, which silently collapsed a whole axis to one point.
pub fn show(v: &Val) -> String {
    match v {
        Val::Enum(s) | Val::Str(s) => s.clone(),
        Val::Bool(b) => if *b { crate::kw::TRUE } else { crate::kw::FALSE }.into(),
        Val::Num(r) => format!("{r}"),
        Val::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        // Two sequences differ when any element does, so the key is the elements' own keys.
        Val::Seq(xs) => {
            let one = |e: &std::collections::BTreeMap<String, Val>| {
                e.iter().map(|(k, v)| format!("{k}={}", show(v))).collect::<Vec<_>>().join(",")
            };
            format!("[{}]", xs.iter().map(one).collect::<Vec<_>>().join(";"))
        }
    }
}

/// The same, for a value that is about to be read by a person. Money and quantities are
/// already whole numbers of their own unit, so only a rate needs its `%` back: `0.1` on its
/// own reads as either a tenth or ten percent.
pub fn show_named(c: &Checked, name: &str, v: &Val) -> String {
    match (v, c.ty_of(name)) {
        (Val::Num(r), Some(crate::types::Ty::Rate)) => format!("{}%", r.mul(Rat::int(100))),
        _ => show(v),
    }
}

/// The `examples` as assignments.
///
/// They are executable specification (§1.2) and the only cases a **person** wrote, yet nothing
/// ran them: E107 holds them to the reference evaluator and stops there. An author's worked
/// cases — the ones taken from the published terms — never reached the generated code in any
/// language, and counted toward no coverage obligation either, so `examples` could not close a
/// hole the generator had left.
///
/// A row becomes a vector only when it names **every** input. The section is not required to
/// (E111 demands the outputs, not the inputs), and a vector missing one would hand the runner a
/// key that is not there.
fn example_inputs(f: &RuleFile, c: &Checked) -> Vec<BTreeMap<String, Val>> {
    let mut out = Vec::new();
    for (ex, row) in f.examples.iter().flat_map(|ex| ex.rows.iter().map(move |r| (ex, r))) {
        let a: BTreeMap<String, Val> = eval::example_env(f, c, ex, row).into_iter().collect();
        // The sequence counts as an input of the case: a walk with none is not the example
        // that was written (§15.56).
        let seq_ok = match &f.elements {
            Some(el) => a.contains_key(&el.name.text),
            None => true,
        };
        if seq_ok && f.inputs.iter().all(|i| a.contains_key(&i.name.text)) {
            out.push(a);
        }
    }
    out
}

/// Evaluate the candidate population and deterministically select a subset that satisfies
/// coverage.
/// Whether an input satisfies every `constraint` the rule declares (§15.55).
///
/// A combination the caller says does not happen is not a test case: the generated code
/// refuses it at the door, and the checker never demanded a row for it. Filtering here is
/// what keeps the suite and the proof talking about the same set of inputs.
pub fn allowed(f: &RuleFile, a: &BTreeMap<String, Val>) -> bool {
    f.constraints.iter().all(|k| match (a.get(&k.left), a.get(&k.right)) {
        (Some(Val::Num(x)), Some(Val::Num(y))) => {
            let o = x.cmp_to(*y);
            match k.op {
                crate::ast::CmpOp::Le => o != std::cmp::Ordering::Greater,
                crate::ast::CmpOp::Lt => o == std::cmp::Ordering::Less,
                crate::ast::CmpOp::Ge => o != std::cmp::Ordering::Less,
                crate::ast::CmpOp::Gt => o == std::cmp::Ordering::Greater,
            }
        }
        _ => true,
    })
}

/// The whole suite: the cases the reference evaluator answers, and the cases it **refuses**.
///
/// A refusal is an answer of its own kind. For a rule that walks a sequence it is where two
/// elements both take under `take_unique`: the evaluator has no answer and the generated code
/// raises, and holding the generated code to that is what makes the transition covered rather
/// than merely named (§15.56).
pub struct Suite {
    pub vectors: Vec<Vector>,
    pub refused: Vec<Vector>,
}

pub fn generate(f: &RuleFile, c: &Checked) -> Vec<Vector> {
    suite(f, c).vectors
}

pub fn suite(f: &RuleFile, c: &Checked) -> Suite {
    let cands = candidates(f, c);
    let raw: Vec<_> = pool(f, c, &cands).into_iter().filter(|(a, _)| allowed(f, a)).collect();

    let key_of = |a: &BTreeMap<String, Val>| -> String {
        a.iter().map(|(k, v)| format!("{k}={}", show(v))).collect::<Vec<_>>().join(",")
    };
    let mut evaluated: Vec<Vector> = Vec::new();
    let mut refused: Vec<Vector> = Vec::new();
    let mut seen_in: BTreeMap<String, usize> = BTreeMap::new();
    for (a, why) in raw {
        let key = key_of(&a);
        if seen_in.contains_key(&key) {
            continue;
        }
        seen_in.insert(key, evaluated.len());
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        // A case the reference evaluator refuses is not a case with an answer, and it is not
        // nothing either. For a rule that walks a sequence, no answer means exactly one thing:
        // two elements both took under `take_unique` (§15.56). It goes to the other list,
        // where `rulec test` holds the generated code to refusing it too.
        if f.fold.is_some() && outs.first().is_some_and(|(_, v)| v.is_none()) {
            refused.push(Vector { input: a, outputs: outs, trace, fired, why });
            continue;
        }
        evaluated.push(Vector { input: a, outputs: outs, trace, fired, why });
    }

    // The examples take part in the audit like any other vector, and are never dropped from the
    // suite afterwards — they are the one thing in it that a person chose.
    let mut forced: BTreeSet<usize> = BTreeSet::new();
    for (n, a) in example_inputs(f, c).into_iter().enumerate() {
        let key = key_of(&a);
        if let Some(&i) = seen_in.get(&key) {
            forced.insert(i);
            continue;
        }
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        seen_in.insert(key, evaluated.len());
        forced.insert(evaluated.len());
        evaluated.push(Vector {
            input: a,
            outputs: outs,
            trace,
            fired,
            why: tr!("例 {}行目", "example row {}", n + 1),
        });
    }

    // Keep the vectors that actually discharged one of the three §9.2 criteria. The key is not
    // what the targeting side claims, but only what the auditor accepted as "this discharges
    // the obligation". A pair obligation admits both vectors together, which is where greedy
    // scoring would drop one.
    let audit = crate::coverage::audit(f, c, "", &evaluated, &refused);
    let mut keep: BTreeSet<usize> = audit.witness.clone();
    keep.extend(forced.iter().copied());

    // The pairwise safety net (§9.2): with the five criteria satisfied, greedily add the
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

    // Every refused case is kept: there are as many of them as there are transitions no
    // answer can reach, and dropping one would take a check away rather than a duplicate.
    Suite {
        vectors: evaluated
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, v)| v)
            .collect(),
        refused,
    }
}

/// One refused case, as a line of `vectors/<alias>.refused.jsonl`: the inputs in the same
/// wire form the vectors file uses, and why the case is in the suite. The generated runners
/// read `in` and nothing else, so the same line drives them (§15.56).
pub fn refused_json(f: &RuleFile, c: &Checked, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\"in\":{},\"refused\":\"contradiction\",\"why\":\"{}\"}}",
        in_object(f, c, v),
        esc(&v.why)
    )
}

/// Canonical JSON. Three-way agreement is judged on these bytes (§8.5).
pub fn to_json(f: &RuleFile, c: &Checked, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\"in\":{},\"out\":{},\"trace\":[{}],\"why\":\"{}\"}}",
        in_object(f, c, v),
        out_object(c, v),
        v.trace.iter().map(|t| format!("\"{}\"", esc(t))).collect::<Vec<_>>().join(","),
        esc(&v.why)
    )
}

/// The JSON object of the inputs, in declaration order, in the wire form of §10.2.
fn in_object(f: &RuleFile, c: &Checked, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    // One value, in the wire form of §10.2. A sequence is an array of objects, and each
    // element's fields are written the same way (§15.56) — the wire gains a shape, not a
    // second kind of number.
    let scalar = |name: &str, val: &Val| -> String {
        // The absent value of an optional column is JSON `null` on the wire — which is what
        // `rulec schema` declares and what the generated `_record` writes. The vectors wrote
        // the word `none` instead, so even a runner that parsed it correctly would have
        // disagreed with the record it produced (§15.88).
        if matches!(val, Val::Enum(e) if e == crate::kw::NONE)
            && matches!(c.ty_of(name), Some(Ty::Opt(_)))
        {
            return "null".into();
        }
        match val {
            Val::Num(r) => format!("{}", crate::types::wire_int(*r, c.wire_scale(name))),
            Val::Bool(b) => format!("{b}"),
            other => format!("\"{}\"", esc(&show(other))),
        }
    };
    let mut ins: Vec<String> = Vec::new();
    // The sequence is named by `elements`, whether a fold reads it or a count does (§15.58).
    if let (Some(el), Some(Val::Seq(xs))) =
        (&f.elements, f.elements.as_ref().and_then(|el| v.input.get(&el.name.text)))
    {
        let one = |e: &std::collections::BTreeMap<String, Val>| -> String {
            let fields: Vec<String> = el
                .fields
                .iter()
                .filter_map(|fd| {
                    let val = e.get(&fd.name.text)?;
                    Some(format!("\"{}\":{}", esc(&fd.name.text), scalar(&fd.name.text, val)))
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        };
        ins.push(format!(
            "\"{}\":[{}]",
            esc(&el.name.text),
            xs.iter().map(one).collect::<Vec<_>>().join(",")
        ));
    }
    for i in &f.inputs {
        let Some(val) = v.input.get(&i.name.text) else { continue };
        // The same `scalar` the element fields go through. It used to be written out a
        // second time here, and the second copy is the one that ran — so a fix to the first
        // (the absent value of an optional) changed nothing at all (§15.88).
        ins.push(format!("\"{}\":{}", esc(&i.name.text), scalar(&i.name.text, val)));
    }
    format!("{{{}}}", ins.join(","))
}

/// The JSON object of the outputs, in declaration order (not the name order of a BTreeMap),
/// so a reader can match it by eye against the `outputs` lines of the rule source.
fn out_object(c: &Checked, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let one = |name: &str, o: &Option<Val>| match o {
        Some(Val::Enum(e))
            if e == crate::kw::NONE && matches!(c.ty_of(name), Some(Ty::Opt(_))) =>
        {
            "null".into()
        }
        Some(Val::Num(r)) => {
            format!("{}", crate::types::wire_int(*r, c.wire_scale(name)))
        }
        Some(Val::Bool(b)) => format!("{b}"),
        Some(other) => format!("\"{}\"", esc(&show(other))),
        None => "null".into(),
    };
    let body: Vec<String> =
        v.outputs.iter().map(|(n, val)| format!("\"{}\":{}", esc(n), one(n, val))).collect();
    format!("{{{}}}", body.join(","))
}

/// One vector as a record in the fixtures format (docs/formats.md): the inputs, the expected
/// values as `observed`, and the rows that matched. It is what the generated runner prints
/// through the module's own record function, so agreement is judged on the whole record —
/// row-level, and over the wire form of every input, dates included (§15.33, §15.35). The
/// file is also a valid fixtures file, which `rulec fixtures lint` and `replay` accept.
pub fn expected_json(f: &RuleFile, c: &Checked, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    // A labelled row carries its label, as the generated record functions write it.
    let fired: Vec<String> = v
        .fired
        .iter()
        .map(|(t, r)| match c.label_of(t, *r) {
            Some(l) => format!("{{\"table\":\"{}\",\"row\":{r},\"label\":\"{}\"}}", esc(t), esc(l)),
            None => format!("{{\"table\":\"{}\",\"row\":{r}}}", esc(t)),
        })
        .collect();
    format!("{{\"in\":{},\"observed\":{},\"trace\":[{}]}}", in_object(f, c, v), out_object(c, v), fired.join(","))
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
/// a derived value is solved through one chosen input. The choice is fixed to the declaration
/// order of the inputs, so the same input moves for the inside and the outside point. The
/// "boundary-pair coverage" of §9.2 demands a vector pair, which is why this is needed.
///
/// `frozen` names inputs that must not move. Solving a derived value takes the first input in
/// declaration order that shifts it, which is the wrong one when another constraint is already
/// holding that input down: `d = b - a` is reached by lowering `a`, undoing the very move that
/// was made to miss an earlier row. Freezing `a` sends the solver to `b` instead.
fn place(
    f: &RuleFile,
    c: &Checked,
    seed: &BTreeMap<String, Val>,
    col: &str,
    target: Rat,
    frozen: &BTreeSet<String>,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    if frozen.contains(col) {
        return None;
    }
    if seed.contains_key(col) {
        if !within(c, col, target) {
            return None;
        }
        let mut a = seed.clone();
        a.insert(col.into(), to_val(target, &ty));
        return Some(a);
    }
    // One input often cannot span the target on its own: `品質 × 3 + 納期 × 2 + 価格 × 2 + 対応`
    // with each score capped at 10 needs several of them moved before the total reaches 72.
    // So each input is taken as far as its own range allows and the rest is left to the next
    // one. Every step is confirmed by evaluating the rule, never by trusting the arithmetic,
    // so a non-linear expression simply fails to place rather than placing wrongly.
    let mut a = seed.clone();
    for _ in 0..=f.inputs.len() {
        let e0 = as_rat(bind(f, c, &a).get(col)?)?;
        if e0.cmp_to(target) == std::cmp::Ordering::Equal {
            return Some(a);
        }
        let mut moved = false;
        for x in f.inputs.iter().map(|i| i.name.text.clone()) {
            if frozen.contains(&x) {
                continue;
            }
            let xty = c.ty_of(&x)?;
            let Some(x0) = a.get(&x).and_then(as_rat) else { continue };
            let e0 = as_rat(bind(f, c, &a).get(col)?)?;
            let q = crate::coverage::quantum(c, &x, &xty);
            let mut probe = a.clone();
            probe.insert(x.clone(), to_val(x0.add(q), &xty));
            let Some(e1) = bind(f, c, &probe).get(col).and_then(as_rat) else { continue };
            let slope = e1.sub(e0).div(q);
            if slope.num == 0 {
                continue;
            }
            let want = x0.add(target.sub(e0).div(slope));
            // Bring it inside the declared range and onto the input's own grid: an input in
            // whole yen never gets 0.5 yen, and a score capped at 10 never gets 24.
            let xn = snap(c, &x, want, q);
            if xn.cmp_to(x0) == std::cmp::Ordering::Equal {
                continue;
            }
            let mut next = a.clone();
            next.insert(x.clone(), to_val(xn, &xty));
            let Some(e2) = bind(f, c, &next).get(col).and_then(as_rat) else { continue };
            // Only keep a step that actually moves the value toward the target. A non-linear
            // expression that the slope mispredicts is dropped here.
            if dist(e2, target).cmp_to(dist(e0, target)) != std::cmp::Ordering::Less {
                continue;
            }
            a = next;
            moved = true;
            if e2.cmp_to(target) == std::cmp::Ordering::Equal {
                return Some(a);
            }
        }
        if !moved {
            return None;
        }
    }
    None
}

fn dist(a: Rat, b: Rat) -> Rat {
    let d = a.sub(b);
    if d.num < 0 { Rat::zero().sub(d) } else { d }
}

/// A value brought inside the declared range of `name` and down onto its grid.
fn snap(c: &Checked, name: &str, v: Rat, q: Rat) -> Rat {
    use std::cmp::Ordering::*;
    let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
    let mut v = v;
    if let Some(h) = hi {
        if v.cmp_to(h) == Greater {
            v = h;
        }
    }
    if let Some(l) = lo {
        if v.cmp_to(l) == Less {
            v = l;
        }
    }
    if q.num == 0 {
        return v;
    }
    let k = v.div(q);
    if k.is_int() {
        return v;
    }
    let floored = Rat::int(k.num / k.den).mul(q);
    if lo.is_some_and(|l| floored.cmp_to(l) == Less) { floored.add(q) } else { floored }
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
    frozen: &BTreeSet<String>,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    let binds = bind(f, c, seed);
    if let Some(v) = binds.get(col) {
        if eval::cell_matches(c, cell, v, &ty) {
            return Some(seed.clone());
        }
    }
    // Number or date: solve for a point inside the cell.
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
        let q = crate::coverage::quantum(c, col, &ty);
        for (_, inside, _) in crate::coverage::thresholds_pub(cell, &ty, q) {
            if let Some(a) = place(f, c, seed, col, inside, frozen) {
                return Some(a);
            }
        }
        return None;
    }
    // Enum or boolean: sweep the inputs one at a time and take the first that matches (in a
    // deterministic order).
    if let Some(vs) = cands.get(col) {
        for v in vs {
            if eval::cell_matches(c, cell, v, &ty) && !frozen.contains(col) {
                let mut a = seed.clone();
                a.insert(col.into(), v.clone());
                return Some(a);
            }
        }
        return None;
    }
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        if frozen.contains(&x) {
            continue;
        }
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
    frozen: &BTreeSet<String>,
) -> Option<BTreeMap<String, Val>> {
    let mut a = seed.clone();
    // Two passes: fixing an earlier column can break a later one, so confirm that one pass was
    // enough before returning.
    for _ in 0..2 {
        // An input this row has already pinned is held while the rest is solved. With
        // `d = b - a`, reaching `d >= 0` by lowering `a` undoes the `a >= 2` two columns to its
        // left, and the second pass only puts it back: the passes oscillate instead of
        // converging, and the row comes out unreachable however reachable it is. Held, the
        // solver goes to `b`. The unfrozen solve is still tried when holding leaves no way
        // through.
        let mut held: BTreeSet<String> = frozen.clone();
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let Some(cell) = row.cells.get(ci) else { continue };
            if matches!(cell, Cell::DontCare) {
                continue;
            }
            a = satisfy_cell(f, c, cands, &a, col, cell, &held)
                .or_else(|| satisfy_cell(f, c, cands, &a, col, cell, frozen))?;
            // An input is in the assignment; a derived column is not. Only the former can be
            // held, and only the former is what a later solve would reach for.
            if a.contains_key(col) {
                held.insert(col.clone());
            }
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
/// the earlier rows must be knocked out too.
///
/// A column this row names is fair game for that. It used to be skipped, on the reasoning that
/// moving a column row `ri` names would break `ri` itself — which holds only when the cell is a
/// single point. `>=1` against an earlier `1` has room: 2 satisfies this row and misses that
/// one. A row that names *every* column therefore could never be won at all, however reachable
/// it was, and `coverage` reported a hole no author could close. What decides now is whether
/// the row still holds afterwards, which is checked either way.
fn win_row(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    set: &crate::defset::DefSet,
    ri: usize,
) -> Option<BTreeMap<String, Val>> {
    let t = &set.table;
    let tag = eval::row_tag(set.row_table(ri), t.rows[ri].index);
    let mut a = reach_row(f, c, cands, seed, t, &t.rows[ri], &BTreeSet::new())?;
    for _ in 0..t.rows.len() + 1 {
        let (_, fired, _) = eval::run_bindings(f, c, a.clone().into_iter().collect());
        if fired.contains(&tag) {
            return Some(a);
        }
        // Knock out one row that currently wins ahead of it: one the precedence relation
        // puts before it (the earlier rows under `first`, or the rows of a table that takes
        // precedence over this one).
        let mut moved = false;
        for &e in &set.beats[ri] {
            if !row_holds(f, c, t, &t.rows[e], &a) {
                continue;
            }
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let Some(ecell) = t.rows[e].cells.get(ci) else { continue };
                if matches!(ecell, Cell::DontCare) {
                    continue;
                }
                // Moving one input can break another of this row's own cells: raising the one
                // that misses the earlier row can push a derived value out of its own cell while
                // the other input stays put. So a candidate is judged *after* the row has been
                // rebuilt around it, and it is the rebuilt assignment that is taken — provided
                // the rebuild did not put the earlier row back.
                let held: BTreeSet<String> = std::iter::once(col.clone()).collect();
                let repair = |b: &BTreeMap<String, Val>| -> Option<BTreeMap<String, Val>> {
                    let r = reach_row(f, c, cands, b, t, &t.rows[ri], &held)?;
                    (!row_holds(f, c, t, &t.rows[e], &r)).then_some(r)
                };
                let keep = |b: &BTreeMap<String, Val>| repair(b).is_some();
                if let Some(b) = violate_cell(f, c, cands, &a, col, ecell, &keep).and_then(|b| repair(&b)) {
                    a = b;
                    moved = true;
                    break;
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
///
/// `keep` says which of those the caller can use. Every way out of the cell is tried until one
/// of them passes it, rather than the first being taken and the caller left to reject it.
fn violate_cell(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    col: &str,
    cell: &Cell,
    keep: &dyn Fn(&BTreeMap<String, Val>) -> bool,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
        let q = crate::coverage::quantum(c, col, &ty);
        for (_, _, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
            if let Some(a) = place(f, c, seed, col, outside, &BTreeSet::new()) {
                if !eval::cell_matches(c, cell, &to_val(outside, &ty), &ty) && keep(&a) {
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
                if keep(&a) {
                    return Some(a);
                }
            }
        }
        return None;
    }
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        for v in cands.get(&x).into_iter().flatten() {
            let mut a = seed.clone();
            a.insert(x.clone(), v.clone());
            if bind(f, c, &a).get(col).is_some_and(|got| !eval::cell_matches(c, cell, got, &ty))
                && keep(&a)
            {
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
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
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
            if let Some(a) = place(f, c, seed, col, target, &BTreeSet::new()) {
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
    // A witness has to be a case somebody could really send (§6.1). An assignment that
    // breaks a `constraint` is not one, and calling such a pair an overlap would name an
    // input the caller has already guaranteed cannot arrive. Where the columns are derived
    // values the region's own sieve cannot apply the constraints — they are stated over
    // inputs, and those inputs are not columns — so this is the place that can (§15.126).
    let hold = |a: &BTreeMap<String, Val>| {
        allowed(f, a) && row_holds(f, c, t, &t.rows[i], a) && row_holds(f, c, t, &t.rows[j], a)
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

// ── The machine's traces (§15.148) ─────────────────────────────────────────

/// One sequence of calls from the machine's initial state: what each call is passed, the
/// carried input left out — it is what the call before answered, and the runner that plays
/// the trace takes it from the answer its own language gave.
#[derive(Clone)]
pub struct Trace {
    pub why: String,
    pub steps: Vec<BTreeMap<String, Val>>,
}

/// The traces of a machine and what the reference evaluator answers to them.
pub struct Traces {
    pub initial: String,
    pub finals: Vec<String>,
    pub traces: Vec<Trace>,
}

/// What a transition is called in the suite: from, the row that decided it, to. Two calls
/// that go the same way by the same row are the same transition.
pub type TransitionKey = (String, Option<(String, usize)>, String);

/// The transition one call makes, read off the answer.
pub fn transition_of(f: &RuleFile, c: &Checked, input: &BTreeMap<String, Val>) -> Option<(TransitionKey, BTreeMap<String, Val>)> {
    let m = f.machine.as_ref()?;
    let (cin, cout) = m.carried()?;
    let from = match input.get(cin) {
        Some(Val::Enum(s)) => s.clone(),
        _ => return None,
    };
    let env: std::collections::HashMap<String, Val> = input.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let (outs, _, rows, _) = eval::run_all_traced(f, c, env);
    let to = match outs.iter().find(|(k, _)| k == cout).and_then(|(_, v)| v.clone()) {
        Some(Val::Enum(s)) => s,
        _ => return None,
    };
    let deciders: BTreeSet<String> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == cout) => t.name.as_ref().map(|n| n.text.clone()),
            _ => None,
        })
        .collect();
    let row = rows.iter().find(|(t, _)| deciders.contains(t)).cloned();
    let _ = c;
    Some(((from, row, to), input.clone()))
}

/// The traces the suite plays (§15.148): for every transition a case can make, and for every
/// two transitions that can follow one another, the shortest sequence of calls from the
/// initial state that ends with them; and every `scenario`, as written. A trace that is the
/// beginning of another is left out — the longer one plays it.
///
/// The obligation is on **pairs** because what a trace adds to the single calls is the
/// hand-over: the state one call answers is the state the next one is passed, in the
/// language under test, as that language holds it. A pair is the smallest place that shows.
pub fn machine_traces(f: &RuleFile, c: &Checked) -> Option<Traces> {
    let m = f.machine.as_ref()?;
    let (cin, _) = m.carried()?;
    let a = crate::machine::analyze(f, c, crate::region::DEFAULT_BUDGET as usize)?;
    // The final states in the enum's order, as every language lists them: the order of the
    // `final` line is how the rule happens to be written, not a fact about it.
    let finals: Vec<String> = a.states.iter().filter(|s| a.finals.contains(s)).cloned().collect();
    let mut out = Traces { initial: a.initial.clone(), finals, traces: Vec::new() };
    let step_of = |e: &crate::machine::Edge| -> BTreeMap<String, Val> {
        e.inputs.iter().filter(|(k, _)| k.as_str() != cin).map(|(k, v)| (k.clone(), v.clone())).collect()
    };
    if a.blocked.is_none() && !a.over_budget {
        // One edge per transition, from the states a case reaches in the world the edge is in
        // (§15.149): a trace is one case, so it holds its `held` inputs.
        let mut rep: Vec<(TransitionKey, usize)> = Vec::new();
        for (i, e) in a.edges.iter().enumerate() {
            if !a.reaches(&e.from, &e.world) || a.mixed.contains(&i) {
                continue;
            }
            let k = (e.from.clone(), e.row.clone(), e.to.clone());
            if !rep.iter().any(|(x, _)| *x == k) {
                rep.push((k, i));
            }
        }
        let name = |k: &TransitionKey| -> String {
            match &k.1 {
                Some((t, r)) => format!("{} -[{}]-> {}", k.0, eval::row_tag(t, *r), k.2),
                None => format!("{} -> {}", k.0, k.2),
            }
        };
        // Every world a transition is made in, with its first edge there. A pair is one case's
        // two calls, so both are made in one world — which need not be the world the first
        // transition was found in first: a decline happens in every world, and the hold that
        // may follow it only in a `manual` one.
        let mut in_world: BTreeMap<(TransitionKey, Vec<usize>), usize> = BTreeMap::new();
        for (i, e) in a.edges.iter().enumerate() {
            if !a.mixed.contains(&i) {
                in_world.entry(((e.from.clone(), e.row.clone(), e.to.clone()), e.world.clone())).or_insert(i);
            }
        }
        let mut paths: Vec<(Vec<usize>, String)> = Vec::new();
        let mut paired: BTreeSet<TransitionKey> = BTreeSet::new();
        const CAP: usize = 4000;
        for (k1, e1) in &rep {
            let first = a.edges[*e1].world.clone();
            let worlds = std::iter::once(first.clone())
                .chain(in_world.keys().filter(|(k, w)| k == k1 && *w != first).map(|(_, w)| w.clone()));
            let worlds: Vec<Vec<usize>> = worlds.collect();
            for (k2, _) in &rep {
                if k1.2 != k2.0 || paths.len() >= CAP {
                    continue;
                }
                // The first world, the first transition's own before the others, in which a
                // case reaches the first call and can go on with the second.
                let Some((i1, i2, w)) = worlds.iter().find_map(|w| {
                    let i1 = *in_world.get(&(k1.clone(), w.clone()))?;
                    let i2 = *in_world.get(&(k2.clone(), w.clone()))?;
                    a.reaches(&k1.0, w).then(|| (i1, i2, w.clone()))
                }) else {
                    continue;
                };
                let Some(mut p) = a.trace_to_node(&(k1.0.clone(), w)) else { continue };
                p.push(i1);
                p.push(i2);
                paired.insert(k1.clone());
                paired.insert(k2.clone());
                paths.push((p, tr!("遷移の対: {} → {}", "transition pair: {} then {}", name(k1), name(k2))));
            }
        }
        for (k, e) in &rep {
            if paired.contains(k) {
                continue;
            }
            let Some(mut p) = a.trace_to_node(&(k.0.clone(), a.edges[*e].world.clone())) else { continue };
            p.push(*e);
            paths.push((p, tr!("遷移: {}", "transition: {}", name(k))));
        }
        // A trace that begins another one is played by the longer one.
        let all: Vec<Vec<usize>> = paths.iter().map(|(p, _)| p.clone()).collect();
        let mut kept: Vec<(Vec<usize>, String)> = Vec::new();
        for (i, (p, why)) in paths.into_iter().enumerate() {
            let prefix = all.iter().enumerate().any(|(j, q)| j != i && q.len() > p.len() && q.starts_with(&p));
            let same_before = all[..i].iter().any(|q| *q == p);
            if !prefix && !same_before {
                kept.push((p, why));
            }
        }
        for (p, why) in kept {
            out.traces.push(Trace { why, steps: p.iter().map(|&e| step_of(&a.edges[e])).collect() });
        }
    }
    for sc in &f.scenarios {
        let mut steps = Vec::new();
        for row in &sc.table.rows {
            let env = eval::example_env(f, c, &sc.table, row);
            let s: BTreeMap<String, Val> = env.into_iter().filter(|(k, _)| k != cin).collect();
            if f.inputs.iter().filter(|i| i.name.text != cin).any(|i| !s.contains_key(&i.name.text)) {
                steps.clear();
                break;
            }
            steps.push(s);
        }
        if !steps.is_empty() {
            out.traces.push(Trace { why: tr!("手順の例 {}", "scenario {}", sc.name.text), steps });
        }
    }
    Some(out)
}

/// The traces file: the line that asks a runner for its constants, then one line per call.
pub fn traces_json(f: &RuleFile, c: &Checked, t: &Traces) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let (cin, cout) = f.machine.as_ref().and_then(|m| m.carried()).unwrap_or(("", ""));
    let mut lines = vec![format!("{{\"machine\":\"meta\",\"carry\":{{\"in\":\"{}\",\"out\":\"{}\"}}}}", esc(cin), esc(cout))];
    for tr in &t.traces {
        for (k, s) in tr.steps.iter().enumerate() {
            let v = Vector { input: s.clone(), outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() };
            if k == 0 {
                lines.push(format!(
                    "{{\"in\":{},\"step\":\"start\",\"state\":\"{}\",\"why\":\"{}\"}}",
                    in_object(f, c, &v),
                    esc(&t.initial),
                    esc(&tr.why)
                ));
            } else {
                lines.push(format!("{{\"in\":{},\"step\":\"next\"}}", in_object(f, c, &v)));
            }
        }
    }
    lines.join("\n") + "\n"
}

/// What the runner has to print for the traces file: its constants, then the record of every
/// call, each call passed the state the reference evaluator answered to the call before.
pub fn traces_expected_json(f: &RuleFile, c: &Checked, t: &Traces) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let (cin, cout) = f.machine.as_ref().and_then(|m| m.carried()).unwrap_or(("", ""));
    let mut lines = vec![format!(
        "{{\"initial\":\"{}\",\"final\":[{}]}}",
        esc(&t.initial),
        t.finals.iter().map(|s| format!("\"{}\"", esc(s))).collect::<Vec<_>>().join(",")
    )];
    for tr in &t.traces {
        let mut state = Val::Enum(t.initial.clone());
        for s in &tr.steps {
            let mut input = s.clone();
            input.insert(cin.to_string(), state.clone());
            let env: std::collections::HashMap<String, Val> = input.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let (outs, _, rows, _) = eval::run_all_traced(f, c, env);
            let next = outs.iter().find(|(k, _)| k == cout).and_then(|(_, v)| v.clone());
            let v = Vector { input, outputs: outs, trace: Vec::new(), fired: rows, why: String::new() };
            lines.push(expected_json(f, c, &v));
            match next {
                Some(n) => state = n,
                None => break,
            }
        }
    }
    lines.join("\n") + "\n"
}
