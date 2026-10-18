//! Test-case generation from boundaries (§9).
//!
//! Candidate values are chosen per column, the candidate population built from them is run
//! through the reference evaluator, and a subset satisfying row coverage, boundary-pair
//! coverage, shadow-pair coverage, value-pair coverage and rounding-tie coverage is selected
//! deterministically.
//! The evaluator attaches
//! the expected values and the fired rows.

use crate::ast::*;
use crate::eval::{self, Val};
use crate::num::{Rat, RoundTo};
use crate::json::Json;
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
        // `outputs_over` (ritsu's port) may hold an enum or a bool input to some of its values:
        // then those are its only candidates, and `none` is not among them
        let held = crate::over::held_values(name);
        if matches!(ty, Ty::Opt(_)) && held.is_none() {
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
                let mut all = c.enums.get(en).cloned().unwrap_or_default();
                if let Some(h) = &held {
                    all.retain(|v| h.contains(v));
                }
                // An input of an ordered enum takes the values its range lets in (§15.210).
                if let Some(r) = c.enum_ranges.get(name) {
                    all.retain(|v| r.contains(v));
                }
                let cells = enum_cells(f, name);
                // A value that is compared with another value in an expression (`a <= b`) has no
                // class of its own to stand for it: which side is the larger depends on the
                // other input, so every value is a candidate (§15.210).
                let compared = c.ordered.contains(en) && compared_in_expr(f, name);
                let mut seen: BTreeSet<Vec<bool>> = BTreeSet::new();
                for v in &all {
                    let val = Val::Enum(v.clone());
                    let sig: Vec<bool> =
                        cells.iter().map(|cell| eval::cell_matches(c, cell, &val, &inner)).collect();
                    if seen.insert(sig) || compared {
                        vs.push(val);
                    }
                }
            }
            Ty::Bool => {
                for b in [true, false] {
                    if held.as_ref().is_none_or(|h| h.contains(&b.to_string())) {
                        vs.push(Val::Bool(b));
                    }
                }
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
            // The days of a koyomi date (§15.174): around each boundary, the nearest day of the
            // set on each side, since no other day arrives.
            Ty::Date if c.day_sets.contains_key(name) => {
                let days = &c.day_sets[name].days;
                let mut picked: BTreeSet<i64> = BTreeSet::new();
                for b in numeric_bounds(f, name, &inner, c) {
                    let at = (b.num / b.den) as i64;
                    if let Some(d) = days.iter().copied().filter(|d| *d <= at).last() {
                        picked.insert(d);
                    }
                    if let Some(d) = days.iter().copied().find(|d| *d >= at) {
                        picked.insert(d);
                    }
                }
                for d in picked {
                    let (y, m, dd) = crate::types::ord_to_date(Rat::int(d as i128));
                    vs.push(Val::Date(y, m, dd));
                }
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

/// Whether a name stands on one side of an order comparison, or in `min` or `max`, in some
/// expression of the rule (§15.210).
fn compared_in_expr(f: &RuleFile, name: &str) -> bool {
    fn walk(e: &Expr, name: &str) -> bool {
        match e {
            Expr::Bin(l, op, r, _) => {
                let here = matches!(op, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt)
                    && (matches!(&**l, Expr::Name(n, _) if n == name) || matches!(&**r, Expr::Name(n, _) if n == name));
                here || walk(l, name) || walk(r, name)
            }
            Expr::Call(fname, args, _) => {
                ((fname == crate::kw::MIN || fname == crate::kw::MAX) && args.iter().any(|a| matches!(a, Expr::Name(n, _) if n == name)))
                    || args.iter().any(|a| walk(a, name))
            }
            _ => false,
        }
    }
    f.items.iter().any(|it| match it {
        Item::Define(d) => walk(&d.expr, name),
        Item::Derived(d) => walk(&d.expr, name),
        _ => false,
    }) || f.result.as_ref().is_some_and(|r| walk(&r.expr, name))
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
            // A summary declares no cells; what it reads is tested inside the walk.
            Item::Agg(_) | Item::Fold(_) => {}
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
            Item::Agg(_) | Item::Fold(_) => {}
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
/// family computing zero. That is what `tie_search` is for: it does not sweep from the baseline.
fn baseline(f: &RuleFile, cands: &BTreeMap<String, Vec<Val>>) -> BTreeMap<String, Val> {
    f.inputs
        .iter()
        .map(|i| i.name.text.clone())
        .map(|n| (n.clone(), cands[&n].first().cloned().unwrap()))
        .collect()
}

/// How far out from its starting point a tie is looked for before giving up.
const TIE_REACH: i128 = 64;

/// The same, from a point the search made up itself rather than one a row or the baseline gave.
const TIE_REACH_NEAR: i128 = 8;

/// An input that lands output `name` on its rounding tie: the value exactly half a step off
/// the grid, which is the single point where `half_up`, `half_down` and `half_even` disagree,
/// and where `up` and `down` disagree too. `round` is mandatory (E104) yet none of the three
/// §9.2 criteria aims at it, so a rule may carry two different modes on two outputs and no
/// vector ever tell them apart — the 50銭 of the social insurance tables is exactly that case.
///
/// Which outputs owe a tie is the auditor's question, answered from the rule
/// (`coverage::tie_duties`); this only looks, and it looks harder than it used to (§15.151):
///
/// 1. along the slopes from the baseline, which is where every tie was found before;
/// 2. by solving for it, where the output is linear in its inputs: `amount × 3.49% + 49` hits a
///    half cent at one amount in ten thousand, and no walk along a slope lands on that;
/// 3. along the slopes from each row's own point — the baseline lands on one row, and where that
///    row writes a constant nothing near it leaves the grid;
/// 4. from each of those points with one number moved off the low end: at `商品合計 × 割引率`
///    with both at zero, neither slope moves the value at all.
fn tie_search(f: &RuleFile, c: &Checked, base: &BTreeMap<String, Val>, seeds: &[&BTreeMap<String, Val>], name: &str, g: Rat) -> Option<BTreeMap<String, Val>> {
    if let Some(a) = walk_to_tie(f, c, base, name, g, TIE_REACH) {
        return Some(a);
    }
    let starts: Vec<&BTreeMap<String, Val>> = std::iter::once(base).chain(seeds.iter().copied()).collect();
    let lands = |a: &BTreeMap<String, Val>| {
        allowed(f, a) && bind(f, c, a).get(name).and_then(as_rat).is_some_and(|v| is_tie(v, g))
    };
    if let Some(forms) = crate::grid::Grid::new(f, c).forms_of(name) {
        for s in &starts {
            for form in &forms {
                for x in form.terms.keys().filter(|x| f.inputs.iter().any(|i| &i.name.text == *x)) {
                    let Some(v) = solve_tie(c, form, s, x, g) else { continue };
                    let mut a = (*s).clone();
                    a.insert(x.clone(), v);
                    if lands(&a) {
                        return Some(a);
                    }
                }
            }
        }
        // The point the audit's own argument walks to, where every name it sets is an input:
        // several inputs moved at once, inside the ranges the rows allow (§15.152).
        for form in &forms {
            let Some(Some(point)) = form.tie_witness(g) else { continue };
            let Some(vals) = point
                .iter()
                .map(|(x, v)| Some((x.clone(), to_val(*v, &c.ty_of(x)?))))
                .collect::<Option<Vec<(String, Val)>>>()
            else {
                continue;
            };
            if !vals.iter().all(|(x, _)| f.inputs.iter().any(|i| &i.name.text == x)) {
                continue;
            }
            for s in &starts {
                let mut a = (*s).clone();
                a.extend(vals.iter().cloned());
                if lands(&a) {
                    return Some(a);
                }
            }
        }
    }
    for s in seeds {
        if let Some(a) = walk_to_tie(f, c, s, name, g, TIE_REACH) {
            return Some(a);
        }
    }
    for s in &starts {
        for i in &f.inputs {
            let x = &i.name.text;
            let Some(ty) = c.ty_of(x) else { continue };
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number) {
                continue;
            }
            let q = crate::coverage::quantum(c, x, &ty);
            let (lo, hi) = c.ranges.get(x).copied().unwrap_or((None, None));
            let low = lo.map(|l| if l.cmp_to(Rat::zero()) == std::cmp::Ordering::Less { Rat::zero() } else { l }).unwrap_or(Rat::zero()).add(q);
            for v in [Some(low), hi].into_iter().flatten() {
                if !within(c, x, v) {
                    continue;
                }
                let mut s2 = (*s).clone();
                s2.insert(x.clone(), Val::Num(v));
                if let Some(a) = walk_to_tie(f, c, &s2, name, g, TIE_REACH_NEAR) {
                    return Some(a);
                }
            }
        }
    }
    // Last, where few enough inputs are involved: every one of them (§15.153).
    crate::grid::exhaust_tie(f, c, name, g).flatten()
}

/// Walk out from `start` along the slopes, tie by tie, until one is placed.
fn walk_to_tie(f: &RuleFile, c: &Checked, start: &BTreeMap<String, Val>, name: &str, g: Rat, reach: i128) -> Option<BTreeMap<String, Val>> {
    let half = g.div(Rat::int(2));
    let v0 = bind(f, c, start).get(name).and_then(as_rat)?;
    // Start at the tie nearest the value this point already produces and walk outward.
    // Every candidate is confirmed by `place`, which re-evaluates the rule, so a non-linear
    // expression fails to place rather than placing wrongly.
    let floor0 = v0.round_to(crate::num::RoundMode::Down, g);
    for step in 0..reach {
        for dir in [1i128, -1] {
            if step == 0 && dir == -1 {
                continue;
            }
            let t = floor0.add(g.mul(Rat::int(dir * step))).add(half);
            if let Some(a) = place(f, c, start, name, t, &BTreeSet::new()) {
                if allowed(f, &a) {
                    return Some(a);
                }
            }
        }
    }
    None
}

/// A value of input `x` that puts `form` exactly half a step off `grid`, the form's other names
/// held where `a` has them: `aₓ·stepₓ·m + rest ≡ grid ÷ 2 (mod grid)`, a linear congruence in
/// the whole number `m`, solved and taken nearest to where `x` stands, inside its range. Whether
/// the form is the one the rule takes at that point is the evaluator's to say, afterwards.
fn solve_tie(c: &Checked, form: &crate::grid::GridForm, a: &BTreeMap<String, Val>, x: &str, grid: Rat) -> Option<Val> {
    let ax = *form.terms.get(x)?;
    if ax.num == 0 {
        return None;
    }
    let q = *form.steps.get(x)?;
    let mut rest = form.k;
    for (n, an) in &form.terms {
        if n == x || an.num == 0 {
            continue;
        }
        rest = rest.checked_add(an.checked_mul(as_rat(a.get(n)?)?)?)?;
    }
    let lhs = ax.checked_mul(q)?;
    let rhs = grid.checked_div(Rat::int(2))?.checked_sub(rest)?;
    let (m0, period) = crate::grid::solve_congruence(lhs, rhs, grid)??;
    // Every solution is m0 + t·period. Take the one nearest to where x stands, in range.
    let here = as_rat(a.get(x)?)?.checked_div(q)?;
    let here = here.num.div_euclid(here.den);
    let (lo, hi) = c.ranges.get(x).copied().unwrap_or((None, None));
    let lo_m = lo.and_then(|l| l.checked_div(q)).map(|r| -((-r.num).div_euclid(r.den)));
    let hi_m = hi.and_then(|h| h.checked_div(q)).map(|r| r.num.div_euclid(r.den));
    let below = m0.checked_add((here.checked_sub(m0)?).div_euclid(period).checked_mul(period)?)?;
    let first = lo_m.and_then(|l| m0.checked_add((l.checked_sub(m0)?.checked_add(period - 1)?).div_euclid(period).checked_mul(period)?));
    let ty = c.ty_of(x)?;
    [Some(below), below.checked_add(period), first]
        .into_iter()
        .flatten()
        .find(|m| lo_m.is_none_or(|l| *m >= l) && hi_m.is_none_or(|h| *m <= h))
        .and_then(|m| Some(to_val(Rat::int(m).checked_mul(q)?, &ty)))
}

/// The points that make a value pair (§15.151): two points one input apart, on the row tagged
/// `tag` (anywhere, for an output no row computes), with `col` moved. Returned are the points
/// the pool does not have yet.
///
/// Each start in turn is tried with one move. Where no single move from any of them does it,
/// the first start is tried with two: `商品合計 × 割引率` is 0 there with both at the low end,
/// and only after one of them has moved does the other move the value; a row that returns
/// `額面` under `残余 > 0` leaves the row as soon as 額面 rises, until 商品合計 has risen
/// first. The first point of the two is then the pair's other half, and it is on the row too.
fn value_pair(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    names: &[String],
    starts: &[(&BTreeMap<String, Val>, bool)],
    tag: Option<&str>,
    col: &str,
) -> Vec<BTreeMap<String, Val>> {
    // What the auditor compares: the column's value and the rule's outputs (`value_moved`).
    type Seen = (Option<Val>, Vec<(String, Option<Val>)>);
    let on_row = |a: &BTreeMap<String, Val>| -> Option<Seen> {
        if !allowed(f, a) {
            return None;
        }
        let (outs, fired, b) = eval::run_all(f, c, a.clone().into_iter().collect());
        tag.is_none_or(|tag| fired.iter().any(|t| t == tag)).then(|| (b.get(col).cloned(), outs))
    };
    let moved = |x: &Seen, y: &Seen| crate::coverage::value_moved(f, col, (x.0.as_ref(), &x.1), (y.0.as_ref(), &y.1));
    let moves = |from: &BTreeMap<String, Val>| -> Vec<BTreeMap<String, Val>> {
        let mut out = Vec::new();
        for x in names {
            for v in spread_cands(c, cands, x) {
                if from.get(x) != Some(&v) {
                    let mut a = from.clone();
                    a.insert(x.clone(), v);
                    out.push(a);
                }
            }
        }
        out
    };
    let on: Vec<(&BTreeMap<String, Val>, bool, Seen)> =
        starts.iter().filter_map(|(s, pooled)| on_row(s).map(|v| (*s, *pooled, v))).collect();
    for (s, pooled, v0) in &on {
        for a in moves(s) {
            // A start already in the pool — the row's own point, the baseline, a point made for
            // another obligation — needs only its partner.
            if on_row(&a).is_some_and(|v| moved(&v, v0)) {
                return if *pooled { vec![a] } else { vec![(*s).clone(), a] };
            }
        }
    }
    let Some((s, _, _)) = on.first() else { return Vec::new() };
    for p in moves(s) {
        let Some(vp) = on_row(&p) else { continue };
        for q in moves(&p) {
            if on_row(&q).is_some_and(|v| moved(&v, &vp)) {
                return vec![p, q];
            }
        }
    }
    Vec::new()
}

/// `a` with every number that has a range moved to the top of it, or to its middle.
fn lift(c: &Checked, a: &BTreeMap<String, Val>, top: bool) -> BTreeMap<String, Val> {
    let mut out = a.clone();
    for (name, v) in a {
        let Some(ty) = c.ty_of(name) else { continue };
        if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) || !matches!(v, Val::Num(_) | Val::Date(..)) {
            continue;
        }
        let Some((Some(lo), Some(hi))) = c.ranges.get(name) else { continue };
        let want = if top { *hi } else { lo.add(*hi).div(Rat::int(2)) };
        let q = crate::coverage::quantum(c, name, &ty);
        out.insert(name.clone(), to_val(snap(c, name, want, q), &ty));
    }
    out
}

/// The values a value pair is looked for with (§15.151): every value an enum has — the column's
/// candidates keep one per class the cells cut, and a row that hands the value itself back
/// tells apart two values of one class — and for a number the ends of its range first, so
/// that the value the pair moves is as far from the low end as the input allows.
fn spread_cands(c: &Checked, cands: &BTreeMap<String, Vec<Val>>, name: &str) -> Vec<Val> {
    let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
    let inner = match &ty {
        Ty::Opt(t) => (**t).clone(),
        other => other.clone(),
    };
    let mut vs: Vec<Val> = Vec::new();
    if matches!(ty, Ty::Opt(_)) {
        vs.push(Val::Enum(crate::kw::NONE.into()));
    }
    match &inner {
        Ty::Enum(en) => vs.extend(
            c.enums
                .get(en)
                .into_iter()
                .flatten()
                .filter(|v| c.enum_ranges.get(name).is_none_or(|r| r.contains(v)))
                .map(|v| Val::Enum(v.clone())),
        ),
        Ty::Bool => vs.extend([Val::Bool(true), Val::Bool(false)]),
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
            if let Some((lo, hi)) = c.ranges.get(name) {
                vs.extend(hi.iter().chain(lo.iter()).map(|r| to_val(*r, &inner)));
            }
            vs.extend(cands.get(name).into_iter().flatten().rev().cloned());
        }
        _ => vs.extend(cands.get(name).into_iter().flatten().cloned()),
    }
    let mut out: Vec<Val> = Vec::new();
    for v in vs {
        if !out.contains(&v) {
            out.push(v);
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
    // The rule's one fold over its one sequence: `pool` sends any other walk to `walk_pool`.
    let fold = f.folds().next().expect("a rule with elements has a fold");
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
    let mut out = match one_walk(f) {
        Some(true) => fold_pool(f, c, cands),
        Some(false) => count_pool(f, c, cands),
        None if f.walks() => walk_pool(f, c, cands),
        None => pool_inner(f, c, cands),
    };
    // A pick by weight (§15.226): the drawn values at the edges of each element's share.
    let more = weighted_cases(f, c, &out);
    out.extend(more);
    out
}

/// What the reason of a case at an edge of the weights of a pick says (§15.226).
fn weight_edge() -> String {
    tr!("重みの区切り", "an edge of the weights")
}

/// The cases a pick by weight is looked at with (§15.226): for each case of the pool whose sequence
/// has an element weighed, the drawn value at the first and the last value that take each element,
/// and at the total of the weights, which the door refuses. A drawn value chosen from the range alone
/// takes the first element at 0 and is refused at the top, so the elements after the first were
/// never picked by a vector.
fn weighted_cases(f: &RuleFile, c: &Checked, pool: &[(BTreeMap<String, Val>, String)]) -> Vec<(BTreeMap<String, Val>, String)> {
    let Some(draw) = f.folds().find(|d| d.weighted()).and_then(|d| d.draw.as_ref()).map(|d| d.text.clone()) else { return Vec::new() };
    let Some((Some(lo), Some(hi))) = c.ranges.get(&draw).copied() else { return Vec::new() };
    let mut out = Vec::new();
    let mut seen: BTreeSet<Vec<(i128, i128)>> = BTreeSet::new();
    for (a, _) in pool.iter().take(64) {
        let mut base = a.clone();
        base.insert(draw.clone(), Val::Num(lo));
        let Some(cuts) = crate::eval::weighted_cuts(f, c, base.clone().into_iter().collect()) else { continue };
        if cuts.is_empty() || !seen.insert(cuts.iter().map(|r| (r.num, r.den)).collect()) {
            continue;
        }
        let mut at: BTreeSet<(i128, i128)> = BTreeSet::new();
        let mut from = Rat::zero();
        for cut in &cuts {
            if cut.cmp_to(from) == std::cmp::Ordering::Greater {
                at.insert((from.num, from.den));
                let last = cut.sub(Rat::int(1));
                at.insert((last.num, last.den));
            }
            from = *cut;
        }
        at.insert((from.num, from.den));
        for (n, d) in at {
            let v = Rat { num: n, den: d };
            if v.cmp_to(lo) == std::cmp::Ordering::Less || v.cmp_to(hi) == std::cmp::Ordering::Greater {
                continue;
            }
            let mut m = a.clone();
            m.insert(draw.clone(), Val::Num(v));
            out.push((m, format!("{draw} = {v} ({})", weight_edge())));
        }
    }
    out
}

/// Whether the rule walks the way a rule walked before it could walk more (§15.219): one
/// sequence, walked once, by one fold alone (`Some(true)`) or by counts and sums alone
/// (`Some(false)`). Those keep the pools made for them, and so the vectors they always had; any
/// other walk takes `walk_pool`.
fn one_walk(f: &RuleFile) -> Option<bool> {
    if f.elements.len() != 1 || crate::types::stages(f).depth > 1 {
        return None;
    }
    let aggs: Vec<&crate::ast::AggDecl> = f.aggs().collect();
    match f.folds().count() {
        1 if aggs.is_empty() => Some(true),
        0 if !aggs.is_empty() && aggs.iter().all(|d| !d.kind.extreme()) => Some(false),
        _ => None,
    }
}

/// The cases a second walk over sequence `s` is looked at with (§15.219): the values of one call
/// at their first candidates, every other sequence at one element, and for `s` each element of a
/// small set on its own and the ordered pairs it makes — a few of the candidate elements spread over
/// them, every ordered pair of those, and each of those beside itself with one of its numbers moved a
/// step or two either way. What the first walk finds then depends on both elements, and each meets it
/// from the other's side: the pairs a boundary of a second walk's table is reached with, and the
/// orders of verdicts a second walk's fold can take. Read off the rule alone, so that the generator
/// builds them and the coverage audit asks what they reach without either asking the other.
pub fn second_walk_cases(f: &RuleFile, c: &Checked, s: usize) -> Vec<BTreeMap<String, Val>> {
    let cands = candidates(f, c);
    let Some(el) = f.elements.get(s) else { return Vec::new() };
    let fields: Vec<String> = f.element_fields().map(|i| i.name.text.clone()).collect();
    let mut base: BTreeMap<String, Val> = baseline(f, &cands).into_iter().filter(|(k, _)| !fields.contains(k)).collect();
    for (t, other) in f.elements.iter().enumerate() {
        if t != s {
            base.insert(other.name.text.clone(), Val::Seq(element_combos(other, &cands).into_iter().take(1).collect()));
        }
    }
    let combos = element_combos(el, &cands);
    let step = (combos.len() / 8).max(1);
    let spread: Vec<BTreeMap<String, Val>> = combos.iter().step_by(step).take(8).cloned().collect();
    // Each one beside itself with one number moved: a step or two either way, inside its range.
    let mut near: Vec<(BTreeMap<String, Val>, BTreeMap<String, Val>)> = Vec::new();
    for x in &spread {
        for fd in &el.fields {
            let ty = c.ty_of(&fd.name.text).unwrap_or(Ty::Unknown);
            let Some(Val::Num(v)) = x.get(&fd.name.text) else { continue };
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number) {
                continue;
            }
            let q = crate::coverage::quantum(c, &fd.name.text, &ty);
            for k in [-2, -1, 1, 2] {
                let w = v.add(q.mul(Rat::int(k)));
                if !crate::coverage::in_range(c, &fd.name.text, w) {
                    continue;
                }
                let mut y = x.clone();
                y.insert(fd.name.text.clone(), Val::Num(w));
                near.push((x.clone(), y));
            }
        }
    }
    let case = |xs: Vec<BTreeMap<String, Val>>| -> BTreeMap<String, Val> {
        let mut m = base.clone();
        m.insert(el.name.text.clone(), Val::Seq(xs));
        m
    };
    let mut out = Vec::new();
    for x in spread.iter().chain(near.iter().map(|(_, y)| y)) {
        out.push(case(vec![x.clone()]));
    }
    for a in &spread {
        for b in &spread {
            out.push(case(vec![a.clone(), b.clone()]));
        }
    }
    for (x, y) in &near {
        out.push(case(vec![x.clone(), y.clone()]));
        out.push(case(vec![y.clone(), x.clone()]));
    }
    out.retain(|m| allowed(f, m) && days_ok(c, m));
    out
}

/// The elements the candidates of one sequence's fields make: their cross product, as the pools
/// of one sequence build it (up to some four thousand).
fn element_combos(el: &crate::ast::ElementsDecl, cands: &BTreeMap<String, Vec<Val>>) -> Vec<BTreeMap<String, Val>> {
    let mut combos: Vec<BTreeMap<String, Val>> = vec![BTreeMap::new()];
    for fd in &el.fields {
        let Some(vs) = cands.get(&fd.name.text) else { continue };
        let mut next = Vec::new();
        for base in &combos {
            for v in vs {
                let mut m = base.clone();
                m.insert(fd.name.text.clone(), v.clone());
                next.push(m);
            }
            if next.len() > 4096 {
                break;
            }
        }
        combos = next;
    }
    combos
}

/// The cases of a rule that walks more than one sequence, or one sequence more than once, or with
/// a `max`, a `min` or more than one kind of summary on it (§15.219).
///
/// Each sequence is moved while the others stay at one element each: nothing, each of its
/// candidate elements on its own, and what each summary over it needs — an element on each verdict
/// of a fold and every ordered pair of them; for a count, as many counted elements as each boundary
/// of the tables that read it asks; for a sum, a total on each boundary; for a `max` or a `min`, an
/// element at each boundary with one beside it that does not take its place, and an element its
/// `where` leaves out. Where something runs in a second walk, every ordered pair of a few elements
/// is a case too: what the first walk found then depends on both, and each element meets it from
/// the other's side. The values of one call move as they do in every pool, and a table that reads
/// what a walk answers has its boundary pairs built with the row's other columns held, as the
/// count's pool builds them.
fn walk_pool(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>) -> Vec<(BTreeMap<String, Val>, String)> {
    use crate::ast::AggKind;
    use crate::types::Stage;
    let st = crate::types::stages(f);
    let fields: Vec<String> = f.element_fields().map(|i| i.name.text.clone()).collect();
    let scalars: BTreeMap<String, Val> = baseline(f, cands).into_iter().filter(|(k, _)| !fields.contains(k)).collect();
    let combos: Vec<Vec<BTreeMap<String, Val>>> = f.elements.iter().map(|el| element_combos(el, cands)).collect();
    // Each sequence at rest: one element, its first candidate, so that a walk over it has one.
    let rest: Vec<Val> = combos.iter().map(|cs| Val::Seq(cs.first().cloned().into_iter().collect())).collect();
    let with = |base: &BTreeMap<String, Val>, s: usize, v: Val| -> BTreeMap<String, Val> {
        let mut m = base.clone();
        for (t, el) in f.elements.iter().enumerate() {
            m.insert(el.name.text.clone(), if t == s { v.clone() } else { rest[t].clone() });
        }
        m
    };
    let seq = |xs: Vec<BTreeMap<String, Val>>| Val::Seq(xs);
    // What the rule binds on a case: every summary's answer is among them.
    let binds_of = |m: &BTreeMap<String, Val>| -> std::collections::HashMap<String, Val> { eval::run_bindings(f, c, m.clone().into_iter().collect()).2 };
    let mut out: Vec<(BTreeMap<String, Val>, String)> = Vec::new();
    // Per summary that a table reads: how to reach a given value of it, for the sweep below.
    let mut reach: Vec<(String, usize, Vec<(Rat, Val)>)> = Vec::new();
    for (s, el) in f.elements.iter().enumerate() {
        let name = &el.name.text;
        let mut keep: Vec<BTreeMap<String, Val>> = Vec::new();
        out.push((with(&scalars, s, seq(Vec::new())), tr!("{name} の要素ゼロ件", "no elements of {name}")));
        for e in &combos[s] {
            out.push((with(&scalars, s, seq(vec![e.clone()])), tr!("{name} の要素の候補", "a candidate element of {name}")));
        }
        for (i, it) in f.items.iter().enumerate() {
            let Stage::Each { seq: over, .. } = st.at(i) else { continue };
            if over != s {
                continue;
            }
            match it {
                // An element on each verdict, the one whose numbers are largest (as `fold_pool`
                // keeps it), and every ordered pair of them.
                Item::Fold(fold) => {
                    let weight = |e: &BTreeMap<String, Val>| -> Rat {
                        e.values().fold(Rat::zero(), |acc, x| match x {
                            Val::Num(r) => acc.add(*r),
                            _ => acc,
                        })
                    };
                    let mut per: BTreeMap<String, BTreeMap<String, Val>> = BTreeMap::new();
                    for e in &combos[s] {
                        let m = with(&scalars, s, seq(vec![e.clone()]));
                        let vs = eval::fold_verdicts(f, c, m.into_iter().collect());
                        if let Some(Some(v)) = vs.get(&i).and_then(|xs| xs.first()) {
                            match per.get(v) {
                                Some(old) if weight(old).cmp_to(weight(e)) != std::cmp::Ordering::Less => {}
                                _ => {
                                    per.insert(v.clone(), e.clone());
                                }
                            }
                        }
                    }
                    for (v, e) in &per {
                        out.push((with(&scalars, s, seq(vec![e.clone()])), tr!("畳み込み {} の判定 {v} 一件", "fold {}: one element on {v}", fold.verdict)));
                        keep.push(e.clone());
                    }
                    for (a, ea) in &per {
                        for (b, eb) in &per {
                            out.push((with(&scalars, s, seq(vec![ea.clone(), eb.clone()])), tr!("畳み込み {} の判定 {a} のあとに {b}", "fold {}: {a} then {b}", fold.verdict)));
                        }
                    }
                }
                Item::Agg(d) => {
                    let ty = c.ty_of(&d.name.text).unwrap_or(Ty::Number);
                    let answer = |xs: Vec<BTreeMap<String, Val>>| -> Option<Val> { binds_of(&with(&scalars, s, seq(xs))).get(&d.name.text).cloned() };
                    let targets: Vec<Rat> = numeric_bounds(f, &d.name.text, &ty, c)
                        .into_iter()
                        .flat_map(|b| [b.sub(Rat::int(1)), b, b.add(Rat::int(1))])
                        .filter(|v| crate::coverage::in_range(c, &d.name.text, *v))
                        .collect();
                    let mut reached: Vec<(Rat, Val)> = Vec::new();
                    match d.kind {
                        // As many counted elements as each boundary asks, and one counted beside
                        // one that is not (`count_pool`).
                        AggKind::Count => {
                            let hit = combos[s].iter().find(|e| answer(vec![(*e).clone()]) == Some(Val::Num(Rat::int(1)))).cloned();
                            let miss = combos[s].iter().find(|e| answer(vec![(*e).clone()]) == Some(Val::Num(Rat::zero()))).cloned();
                            let Some(hit) = hit else { continue };
                            let top = targets.iter().map(|r| r.num / r.den).max().unwrap_or(1).max(1);
                            let cap = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi).map(|h| h.num / h.den).unwrap_or(top);
                            for n in 1..=top.min(cap) {
                                let xs: Vec<BTreeMap<String, Val>> = (0..n).map(|_| hit.clone()).collect();
                                out.push((with(&scalars, s, seq(xs.clone())), tr!("{} が {n} 件", "{} of them: {n}", d.name.text)));
                                reached.push((Rat::int(n), seq(xs)));
                            }
                            keep.push(hit.clone());
                            if let Some(miss) = miss {
                                out.push((with(&scalars, s, seq(vec![hit.clone(), miss.clone()])), tr!("{} に入るものと入らないもの", "one counted and one not, for {}", d.name.text)));
                                keep.push(miss);
                            }
                        }
                        // A total on each boundary, one element carrying as much of it as the
                        // field's range allows (`count_pool`'s `total_seq`), where the column is a
                        // field of the element.
                        AggKind::Sum => {
                            let Some(base) = combos[s].first().cloned() else { continue };
                            let Some((Some(lo), Some(hi))) = c.ranges.get(&d.column.text).copied() else { continue };
                            if !el.fields.iter().any(|fd| fd.name.text == d.column.text) || hi.cmp_to(Rat::zero()) != std::cmp::Ordering::Greater {
                                continue;
                            }
                            for want in targets {
                                let mut left = want;
                                let mut xs: Vec<BTreeMap<String, Val>> = Vec::new();
                                while left.cmp_to(Rat::zero()) == std::cmp::Ordering::Greater && xs.len() < 64 {
                                    let take = if left.cmp_to(hi) == std::cmp::Ordering::Greater { hi } else { left };
                                    if take.cmp_to(lo) == std::cmp::Ordering::Less {
                                        break;
                                    }
                                    let mut e = base.clone();
                                    e.insert(d.column.text.clone(), Val::Num(take));
                                    xs.push(e);
                                    left = left.sub(take);
                                }
                                if left.cmp_to(Rat::zero()) != std::cmp::Ordering::Equal || answer(xs.clone()) != Some(Val::Num(want)) {
                                    continue;
                                }
                                out.push((with(&scalars, s, seq(xs.clone())), tr!("{} が {} の並び", "a sequence whose {} is {}", d.name.text, show(&Val::Num(want)))));
                                reached.push((want, seq(xs)));
                            }
                        }
                        // The answer on each boundary: an element at it, and one beside it that
                        // does not take its place; and an element the `where` leaves out, for the
                        // `empty` value (§15.219).
                        AggKind::Max | AggKind::Min => {
                            let ord_of = |v: &Val| -> Option<Rat> {
                                match v {
                                    Val::Num(r) => Some(*r),
                                    Val::Date(y, m, dd) => Some(crate::types::date_ord(*y, *m, *dd)),
                                    _ => None,
                                }
                            };
                            let single = |e: &BTreeMap<String, Val>| answer(vec![e.clone()]);
                            // Elements by the value they give the answer on their own.
                            let mut by: Vec<(BTreeMap<String, Val>, Val)> = combos[s].iter().filter_map(|e| single(e).map(|v| (e.clone(), v))).collect();
                            // A field read as it is can be set to the value wanted directly.
                            if let Some(fd) = el.fields.iter().find(|fd| fd.name.text == d.column.text) {
                                if let Some(base) = by.first().map(|(e, _)| e.clone()) {
                                    for t in &targets {
                                        let fty = c.ty_of(&fd.name.text).unwrap_or(Ty::Number);
                                        if !crate::coverage::in_range(c, &fd.name.text, *t) {
                                            continue;
                                        }
                                        let mut e = base.clone();
                                        e.insert(fd.name.text.clone(), if fty == Ty::Date { let (y, m, dd) = crate::types::ord_to_date(*t); Val::Date(y, m, dd) } else { Val::Num(*t) });
                                        if let Some(v) = single(&e) {
                                            by.push((e, v));
                                        }
                                    }
                                }
                            }
                            let empty = d.empty.as_ref().and_then(|(l, _)| eval::lit_to_val(l, c.ty_of(&d.column.text).unwrap_or(Ty::Unknown).present()));
                            for t in &targets {
                                let Some((e, v)) = by.iter().find(|(_, v)| ord_of(v).is_some_and(|x| x.cmp_to(*t) == std::cmp::Ordering::Equal)) else { continue };
                                out.push((with(&scalars, s, seq(vec![e.clone()])), tr!("{} が {} の要素", "an element that makes {} {}", d.name.text, show(v))));
                                reached.push((*t, seq(vec![e.clone()])));
                                keep.push(e.clone());
                                // One that does not take its place: smaller for a max, larger for a min.
                                let beside = by.iter().filter(|(_, w)| Some(w) != empty.as_ref() || empty.is_none()).find(|(_, w)| match (ord_of(w), ord_of(v)) {
                                    (Some(x), Some(y)) => (d.kind == AggKind::Max && x.cmp_to(y) == std::cmp::Ordering::Less) || (d.kind == AggKind::Min && x.cmp_to(y) == std::cmp::Ordering::Greater),
                                    _ => false,
                                });
                                if let Some((b, _)) = beside {
                                    for xs in [vec![e.clone(), b.clone()], vec![b.clone(), e.clone()]] {
                                        if answer(xs.clone()).as_ref() == Some(v) {
                                            out.push((with(&scalars, s, seq(xs)), tr!("{} が {} で、そこに届かない要素も並ぶ", "{} at {}, beside an element that does not take its place", d.name.text, show(v))));
                                        }
                                    }
                                }
                            }
                            // The values of an ordered enum, each by an element on it.
                            for (e, v) in &by {
                                if matches!(v, Val::Enum(_)) {
                                    out.push((with(&scalars, s, seq(vec![e.clone()])), tr!("{} が {} の要素", "an element that makes {} {}", d.name.text, show(v))));
                                    keep.push(e.clone());
                                }
                            }
                            // An element the `where` leaves out: the answer is the `empty` value.
                            if d.filter.is_some() {
                                if let Some(e) = combos[s].iter().find(|e| single(e) == empty && empty.is_some()) {
                                    out.push((with(&scalars, s, seq(vec![e.clone()])), tr!("{} の `where` が外す要素だけ", "only an element the `where` of {} leaves out", d.name.text)));
                                    keep.push(e.clone());
                                }
                            }
                        }
                    }
                    if !reached.is_empty() {
                        reach.push((d.name.text.clone(), s, reached));
                    }
                }
                _ => {}
            }
        }
        // A second walk: the cases it is looked at with (`second_walk_cases`), and every ordered
        // pair of the elements found above, so that what the first walk finds depends on both and
        // each element meets it from the other's side.
        let second = (0..f.items.len()).any(|i| matches!(st.at(i), Stage::Each { seq: x, walk } if x == s && walk > 1));
        if second {
            let probes = second_walk_cases(f, c, s);
            // A table of the second walk that reads a value of one call worked out from what the
            // first walk found (`light = hour_light + most_add + most_sub`) has its boundaries
            // reached by moving an input with the sequence held: the first walk then finds the
            // same, and the element's case moves in that input alone (§9.1, §15.219).
            for set in c.sets.iter().filter(|set| {
                f.items.iter().enumerate().any(|(i, it)| matches!(it, Item::Table(t) if t.name.as_ref().is_some_and(|n| n.text == set.eval_at)) && matches!(st.at(i), Stage::Each { seq: x, walk } if x == s && walk > 1))
            }) {
                let t = &set.table;
                for (ri, row) in t.rows.iter().enumerate() {
                    for (ci, (col, _)) in t.inputs.iter().enumerate() {
                        // A column of one element moves between two elements of one sequence:
                        // each pair the probes make, with the row's columns of one call brought
                        // inside its cells, so that both elements meet the row's other cells.
                        if st.each.contains_key(col) {
                            for m in probes.iter().filter(|m| matches!(m.get(&f.elements[s].name.text), Some(Val::Seq(xs)) if xs.len() == 2)).take(64) {
                                let mut a = m.clone();
                                for (cj, (other, _)) in t.inputs.iter().enumerate() {
                                    if cj == ci || st.each.contains_key(other) || f.inputs.iter().any(|i| &i.name.text == other) {
                                        continue;
                                    }
                                    let (Some(oc), Some(oty)) = (row.cells.get(cj), c.ty_of(other)) else { continue };
                                    let now = bind(f, c, &a).get(other).cloned();
                                    if now.as_ref().is_some_and(|v| eval::cell_matches(c, oc, v, &oty)) {
                                        continue;
                                    }
                                    let q = crate::coverage::quantum(c, other, &oty);
                                    let Some((_, want, _)) = crate::coverage::thresholds_pub(oc, &oty, q).into_iter().next() else { continue };
                                    match place(f, c, &a, other, want, &BTreeSet::new()) {
                                        Some(next) => a = next,
                                        None => break,
                                    }
                                }
                                if &a != m {
                                    out.push((a, tr!("表 {} 行{} の列 {col} を、二件で（二回目の歩き）", "table {} row {}, column {col}, over two elements (a second walk)", set.row_table(ri), row.index)));
                                }
                            }
                            continue;
                        }
                        if f.inputs.iter().any(|i| &i.name.text == col) {
                            continue;
                        }
                        let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
                        if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                            continue;
                        }
                        let Some(cell) = row.cells.get(ci) else { continue };
                        let q = crate::coverage::quantum(c, col, &ty);
                        for (b, inside, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
                            let why = tr!(
                                "境界両側: 表 {} 行{} {col} {} の",
                                "boundary pair: table {} row {} {col} {}",
                                set.row_table(ri),
                                row.index,
                                crate::coverage::show_rat(b, &ty)
                            );
                            for seed in probes.iter().take(48) {
                                let Some(a1) = place(f, c, seed, col, inside, &BTreeSet::new()) else { continue };
                                let Some(a2) = place(f, c, &a1, col, outside, &BTreeSet::new()) else { continue };
                                out.push((a1, tr!("{why}内側", "{why} inside")));
                                out.push((a2, tr!("{why}外側", "{why} outside")));
                            }
                        }
                    }
                }
            }
            for m in probes {
                out.push((m, tr!("{name} の一件か二件（二回目の歩き）", "one or two elements of {name} (a second walk)")));
            }
            let mut few: Vec<BTreeMap<String, Val>> = Vec::new();
            for e in keep {
                if !few.contains(&e) {
                    few.push(e);
                }
            }
            few.truncate(8);
            for a in &few {
                for b in &few {
                    out.push((with(&scalars, s, seq(vec![a.clone(), b.clone()])), tr!("{name} の二件（二回目の歩き）", "two elements of {name} (a second walk)")));
                }
            }
        }
    }

    // A table downstream of a summary is an ordinary table, so its boundary pairs are walked with
    // the row's other columns held (§9.2), out of the row's own assignment for them and a sequence
    // that brings the summary to each side (`count_pool`).
    for set in &c.sets {
        let t = &set.table;
        for (ri, row) in t.rows.iter().enumerate() {
            let tname = set.row_table(ri).to_string();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let Some((_, s, reached)) = reach.iter().find(|(n, _, _)| n == col) else { continue };
                let Some(cell) = row.cells.get(ci) else { continue };
                let ty = c.ty_of(col).unwrap_or(Ty::Number);
                let mut a = scalars.clone();
                for (cj, (other, _)) in t.inputs.iter().enumerate() {
                    if cj == ci || fields.contains(other) || reach.iter().any(|(n, _, _)| n == other) {
                        continue;
                    }
                    let (Some(oc), Some(ovs)) = (row.cells.get(cj), cands.get(other)) else { continue };
                    let oty = c.ty_of(other).unwrap_or(Ty::Unknown);
                    if let Some(v) = satisfying(oc, ovs, &oty, c) {
                        a.insert(other.clone(), v);
                    }
                }
                let q = crate::coverage::quantum(c, col, &ty);
                for (b, inside, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
                    let b = crate::coverage::show_rat(b, &ty);
                    let why = tr!("境界両側: 表 {tname} 行{} {col} {b} の", "boundary pair: table {tname} row {} {col} {b}", row.index);
                    for (n, side) in [(inside, tr!("{why}内側", "{why} inside")), (outside, tr!("{why}外側", "{why} outside"))] {
                        if let Some((_, xs)) = reached.iter().find(|(r, _)| r.cmp_to(n) == std::cmp::Ordering::Equal) {
                            out.push((with(&a, *s, xs.clone()), side));
                        }
                    }
                }
            }
        }
    }

    // The values of one call move too, each sequence at rest, so a value that decides a cell is
    // still exercised.
    for (a, why) in pool_scalars(f, c, cands, &fields) {
        let m = with(&a, usize::MAX, Val::Seq(Vec::new()));
        out.push((m, why));
    }

    // A row that holds what a walk answers and a value of the call together: the value's pairs
    // again, with the sequence that brings the answer to the row's cell (`held_pairs`).
    let held = |t: &Table, row: &Row| -> Option<BTreeMap<String, Val>> {
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let Some(cell) = row.cells.get(ci) else { continue };
            if matches!(cell, Cell::DontCare) {
                continue;
            }
            let Some((_, s, reached)) = reach.iter().find(|(n, _, _)| n == col) else { continue };
            let ty = c.ty_of(col).unwrap_or(Ty::Number);
            let vals: Vec<Val> = reached.iter().map(|(r, _)| to_val(*r, &ty)).collect();
            let Some(v) = satisfying(cell, &vals, &ty, c) else { continue };
            let i = vals.iter().position(|x| *x == v)?;
            return Some(with(&BTreeMap::new(), *s, reached[i].1.clone()));
        }
        None
    };
    let summaries: Vec<String> = f.aggs().map(|d| d.name.text.clone()).collect();
    let skip = |n: &str| fields.iter().any(|x| x == n) || summaries.iter().any(|x| x == n);
    out.extend(held_pairs(f, c, cands, &scalars, &skip, &held));
    out
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
    // The rule's one sequence: `pool` sends any other walk to `walk_pool`.
    let el = f.elements.first().expect("a count has elements");
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

    // A row that holds a count and a value of the call together: the value's pairs again, with
    // as many counted elements as the row's cell asks, or a total that meets it (`held_pairs`).
    let lengths = |d: &crate::ast::AggDecl| -> Vec<Val> {
        let top = numeric_bounds(f, &d.name.text, &Ty::Number, c).iter().map(|r| r.num / r.den).max().unwrap_or(1);
        let cap = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi).map(|h| h.num / h.den).unwrap_or(top + 1);
        (0..=(top + 1).min(cap)).map(|n| Val::Num(Rat::int(n))).collect()
    };
    let totals = |d: &crate::ast::AggDecl, ty: &Ty| -> Vec<Val> {
        std::iter::once(Rat::zero())
            .chain(numeric_bounds(f, &d.name.text, ty, c).into_iter().flat_map(|b| [b, b.sub(Rat::int(1)), b.add(Rat::int(1))]))
            .filter(|v| crate::coverage::in_range(c, &d.name.text, *v))
            .map(|v| to_val(v, ty))
            .collect()
    };
    let held = |t: &Table, row: &Row| -> Option<BTreeMap<String, Val>> {
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let Some(cell) = row.cells.get(ci) else { continue };
            if matches!(cell, Cell::DontCare) {
                continue;
            }
            if let Some(&(d, hit)) = counted.iter().find(|(d, _)| d.name.text == *col) {
                let Some(Val::Num(n)) = satisfying(cell, &lengths(d), &Ty::Number, c) else { continue };
                return Some(BTreeMap::from([(over.clone(), seq((0..n.num).map(|_| hit).collect()))]));
            }
            if let Some(&d) = summed.iter().find(|d| d.name.text == *col) {
                let ty = c.ty_of(col).unwrap_or(Ty::Number);
                let Some(want) = satisfying(cell, &totals(d, &ty), &ty, c).as_ref().and_then(as_rat) else { continue };
                if let Some(xs) = total_seq(d, want) {
                    return Some(BTreeMap::from([(over.clone(), xs)]));
                }
            }
        }
        None
    };
    let skip = |n: &str| fields.iter().any(|x| x == n) || counts.iter().any(|d| d.name.text == n);
    out.extend(held_pairs(f, c, cands, &scalars, &skip, &held));
    out
}

/// The boundary pairs of a row's values of one call, walked with the row's other columns held
/// (§9.2) and the sequences set to what the row's cells on the walks' answers ask for. A pool
/// carries a value's pairs with a sequence at rest, which lands on the near side of a cell like
/// `>=2` on a count: a row `| >=2 | >=100USD |` had no vector with two counted elements and 99
/// dollars, so the outside point of the subtotal was never run. `held` gives the sequences for a
/// row, or `None` when its cells on the walks' answers are all `-`, where the pool's own pairs are
/// already on the row. `skip` names the columns that are not values of the call.
fn held_pairs(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    scalars: &BTreeMap<String, Val>,
    skip: &dyn Fn(&str) -> bool,
    held: &dyn Fn(&Table, &Row) -> Option<BTreeMap<String, Val>>,
) -> Vec<(BTreeMap<String, Val>, String)> {
    let mut out = Vec::new();
    for set in &c.sets {
        let t = &set.table;
        for (ri, row) in t.rows.iter().enumerate() {
            let Some(seqs) = held(t, row) else { continue };
            let tname = set.row_table(ri).to_string();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if skip(col) {
                    continue;
                }
                let Some(ty) = c.ty_of(col) else { continue };
                if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let q = crate::coverage::quantum(c, col, &ty);
                let pairs = crate::coverage::thresholds_pub(cell, &ty, q);
                if pairs.is_empty() {
                    continue;
                }
                let mut a = scalars.clone();
                for (cj, (other, _)) in t.inputs.iter().enumerate() {
                    if cj == ci || skip(other) {
                        continue;
                    }
                    let (Some(oc), Some(ovs)) = (row.cells.get(cj), cands.get(other)) else { continue };
                    let oty = c.ty_of(other).unwrap_or(Ty::Unknown);
                    if let Some(v) = satisfying(oc, ovs, &oty, c) {
                        a.insert(other.clone(), v);
                    }
                }
                a.extend(seqs.clone());
                for (b, inside, outside) in pairs {
                    let (Some(ain), Some(aout)) =
                        (place(f, c, &a, col, inside, &BTreeSet::new()), place(f, c, &a, col, outside, &BTreeSet::new()))
                    else {
                        continue;
                    };
                    let b = crate::coverage::show_rat(b, &ty);
                    let why = tr!("境界両側: 表 {tname} 行{} {col} {b} の", "boundary pair: table {tname} row {} {col} {b}", row.index);
                    out.push((ain, tr!("{why}内側", "{why} inside")));
                    out.push((aout, tr!("{why}外側", "{why} outside")));
                }
            }
        }
    }
    out
}

fn pool_inner(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>) -> Vec<(BTreeMap<String, Val>, String)> {
    let names: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let base = baseline(f, cands);
    let mut out: Vec<(BTreeMap<String, Val>, String)> = vec![(base.clone(), tr!("基準", "baseline"))];
    // Each row's own point, by (definition set, row). The value pairs and the rounding ties
    // start from here as well as from the baseline (§15.151).
    let mut seeds: BTreeMap<(usize, usize), BTreeMap<String, Val>> = BTreeMap::new();

    // Row coverage: build an assignment that makes each row win. Columns holding derived or
    // intermediate values are mapped back onto the inputs (§9.1). Naively "putting a value that
    // satisfies the input cell" never touches a column a derived value decides, and the row
    // stays beaten by an earlier row under `first`. That showed up in the coverage auditor as
    // missing row coverage (クーポン併用 row 3, 適用順序 row 4, 素の割引 row 3).
    for (si, set) in c.sets.iter().enumerate() {
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
            seeds.insert((si, ri), seed.clone());
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

    // Value pairs (§15.151): for each row that returns a computed value, a second point one
    // input away from the row's own, still on the row, with the value moved. The row's point
    // is the low end of every range, so a refund of the amount paid is 0 yen there; the input
    // moved is taken to the far end first, where a wrong factor shows as well as a constant.
    for d in crate::coverage::value_duties(f, c) {
        let col = &d.col;
        // Where the pair may start: the row's own point, then every point already made — at
        // `min(素割引, 上限額)` the baseline has both at zero, and a point that made 素割引
        // large for another obligation is one move away from the pair.
        let (tag, first, why) = match d.at {
            Some((si, ri)) => {
                let (set, row) = (&c.sets[si], &c.sets[si].table.rows[ri]);
                let (tname, rn) = (set.row_table(ri), row.index);
                let Some(seed) = seeds.get(&(si, ri)) else { continue };
                (
                    Some(eval::row_tag(tname, rn)),
                    seed.clone(),
                    tr!("計算値の対: 表 {tname} 行{rn} {col}", "value pair: table {tname} row {rn} {col}"),
                )
            }
            None => (None, base.clone(), tr!("計算値の対: 出力 {col}", "value pair: output {col}")),
        };
        // The same point with every number at the top of its range, and in the middle: the
        // row's own point has them at the bottom, where a product is 0 and a `min` with a cap
        // of 0 is 0 whatever else moves. These are not in the pool yet.
        let lifted: Vec<BTreeMap<String, Val>> = [true, false].iter().map(|top| lift(c, &first, *top)).collect();
        let starts: Vec<(&BTreeMap<String, Val>, bool)> = std::iter::once((&first, true))
            .chain(lifted.iter().map(|a| (a, false)))
            .chain(out.iter().map(|(a, _)| (a, true)))
            .collect();
        let mut found = value_pair(f, c, cands, &names, &starts, tag.as_deref(), col);
        // Last, where few enough inputs are involved: every one of them (§15.153).
        if found.is_empty() {
            if let Some(Some((p, q))) = crate::grid::exhaust_pair(f, c, tag.as_deref(), col) {
                found = vec![p, q];
            }
        }
        for a in found {
            out.push((a, why.clone()));
        }
    }

    // The rounding tie (§9.2). Unlike the sweep above, this does not hold the other columns at
    // the baseline: `place` moves whatever it needs to land the value half a step off the grid.
    // It starts from the baseline, then from each row's own point (§15.151).
    let starts: Vec<&BTreeMap<String, Val>> = seeds.values().collect();
    for (name, grid) in crate::coverage::tie_duties(f, c) {
        if let Some(a) = tie_search(f, c, &base, &starts, &name, grid) {
            out.push((a, tr!("丸めの同着: {name}", "rounding tie: {name}")));
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
    // An optional rate that is there is a rate (§15.249).
    match (v, c.ty_of(name).as_ref().map(crate::types::Ty::present)) {
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
        let seq_ok = f.elements.iter().all(|el| a.contains_key(&el.name.text));
        if seq_ok && f.inputs.iter().all(|i| a.contains_key(&i.name.text)) {
            out.push(a);
        }
    }
    out
}

/// Whether every date input of koyomi's days takes one of them (§15.174): the generated code
/// refuses any other day at its door, so no vector names one.
pub fn days_ok(c: &Checked, a: &BTreeMap<String, Val>) -> bool {
    c.day_sets.iter().all(|(name, d)| match a.get(name) {
        Some(Val::Date(y, m, dd)) => {
            let v = crate::types::date_ord(*y, *m, *dd);
            d.days.binary_search(&(v.num as i64)).is_ok()
        }
        _ => true,
    })
}

/// Whether an input satisfies every `constraint` the rule declares (§15.55).
///
/// A combination the caller says does not happen is not a test case: the generated code
/// refuses it at the door, and the checker never demanded a row for it. Filtering here is
/// what keeps the suite and the proof talking about the same set of inputs.
pub fn allowed(f: &RuleFile, a: &BTreeMap<String, Val>) -> bool {
    f.constraints.iter().all(|k| constraint_holds(k, &|n| a.get(n)))
}

/// Whether one `constraint` holds of the values its two sides take, each side's names read with
/// `value`. The two sides are inputs of one type with an order (E018): two numbers compare as
/// numbers, and two dates by their day numbers (§2.1), the way the generated code's door compares
/// them. Reading only numbers here let every pair of dates through, so `check_in <= check_out`
/// filtered nothing (§15.198). A side that adds names up compares their sum (§15.216). A side
/// with a name that has no value is not held to it, the way the door compares only what it was
/// passed.
pub fn constraint_holds<'v>(k: &Constraint, value: &dyn Fn(&str) -> Option<&'v Val>) -> bool {
    let ord = |v: Option<&Val>| match v {
        Some(Val::Num(x)) => Some(*x),
        Some(Val::Date(y, m, d)) => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    };
    let side = |names: &[String]| names.iter().try_fold(Rat::zero(), |acc, n| ord(value(n)).map(|v| acc.add(v)));
    let (Some(x), Some(y)) = (side(&k.left), side(&k.right)) else { return true };
    let o = x.cmp_to(y);
    match k.op {
        CmpOp::Le => o != std::cmp::Ordering::Greater,
        CmpOp::Lt => o == std::cmp::Ordering::Less,
        CmpOp::Ge => o != std::cmp::Ordering::Less,
        CmpOp::Gt => o == std::cmp::Ordering::Greater,
    }
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

/// Evaluate the candidate population and deterministically select a subset that satisfies
/// coverage.
pub fn suite(f: &RuleFile, c: &Checked) -> Suite {
    let cands = candidates(f, c);
    let raw: Vec<_> = pool(f, c, &cands).into_iter().filter(|(a, _)| allowed(f, a) && days_ok(c, a)).collect();

    let key_of = |a: &BTreeMap<String, Val>| -> String {
        a.iter().map(|(k, v)| format!("{k}={}", show(v))).collect::<Vec<_>>().join(",")
    };
    let mut evaluated: Vec<Vector> = Vec::new();
    let mut refused: Vec<Vector> = Vec::new();
    let mut past_sums: Vec<String> = Vec::new();
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
        if f.walks() && crate::eval::contradiction(f, c, a.clone().into_iter().collect()).is_some() {
            refused.push(Vector { input: a, outputs: outs, trace, fired, why });
            continue;
        }
        // A sum past the top of its range is refused where the walk ends (§15.100): one such case
        // per sum holds the generated code to refusing it, and the rest of them say nothing more.
        if let Some(n) = crate::eval::sum_past_range(f, c, a.clone().into_iter().collect()) {
            if !past_sums.contains(&n) {
                past_sums.push(n);
                refused.push(Vector { input: a, outputs: outs, trace, fired, why });
            }
            continue;
        }
        // A drawn value not below the total of the weights is refused once the walk is over
        // (§15.226): one such case holds the generated code to refusing it.
        if let Some(n) = crate::eval::drawn_past(f, c, a.clone().into_iter().collect()) {
            let key = format!("draw:{n}");
            if !past_sums.contains(&key) {
                past_sums.push(key);
                refused.push(Vector { input: a, outputs: outs, trace, fired, why });
            }
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

    // A row with a cell on a computed column that no vector reached may still be one the check
    // found an input for: the input behind the gap that taking the row out leaves (§15.209,
    // refined). It goes in like any other, so the generated code is run on that row too.
    let fired_rows: BTreeSet<(String, usize)> = evaluated.iter().flat_map(|v| v.fired.iter().cloned()).collect();
    for (table, row, a) in crate::region::gap_inputs(f, c, &|tn, ri| fired_rows.contains(&(tn.to_string(), ri))) {
        let key = key_of(&a);
        if seen_in.contains_key(&key) || !allowed(f, &a) || !days_ok(c, &a) {
            continue;
        }
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        seen_in.insert(key, evaluated.len());
        evaluated.push(Vector {
            input: a,
            outputs: outs,
            trace,
            fired,
            why: tr!("表 {table} 行{row} を除いたときの穴を作る入力", "the input behind the gap table {table} row {row} closes"),
        });
    }

    // Keep the vectors that actually discharged one of the three §9.2 criteria. The key is not
    // what the targeting side claims, but only what the auditor accepted as "this discharges
    // the obligation". A pair obligation admits both vectors together, which is where greedy
    // scoring would drop one.
    let audit = crate::coverage::audit(f, c, "", &evaluated, &refused);
    let mut keep: BTreeSet<usize> = audit.witness.clone();
    keep.extend(forced.iter().copied());
    // The drawn values at the edges of each element's share of a pick by weight (§15.226) stay: no
    // criterion of the audit asks which element a pick takes, and without them a pick past the
    // first element would never be run.
    let edge = weight_edge();
    keep.extend(evaluated.iter().enumerate().filter(|(_, v)| v.why.contains(&edge)).map(|(i, _)| i).take(32));

    // The pairwise safety net (§9.2): with the criteria satisfied, greedily add the
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
/// wire form the vectors file uses, what the reference evaluator refuses them for and the
/// sentence it says, and why the case is in the suite. The generated runners read `in` and
/// nothing else, so the same line drives them (§15.56); `rulec test` holds what they say to
/// `refused` and `error` (§15.204).
pub fn refused_json(f: &RuleFile, c: &Checked, v: &Vector) -> String {
    let inp = in_object(f, c, v);
    let r = crate::json::parse(&inp)
        .ok()
        .and_then(|j| crate::door::refusal(f, c, &j))
        .unwrap_or(crate::door::Refusal {
            kind: "contradiction",
            error: crate::door::fold_contradiction(f.folds().next().map(|d| d.verdict.as_str()).unwrap_or_default()),
        });
    refused_line(&inp, &r, &v.why)
}

fn refused_line(inp: &str, r: &crate::door::Refusal, why: &str) -> String {
    format!(
        "{{\"in\":{inp},\"refused\":{},\"error\":{},\"why\":{}}}",
        crate::json::quote(r.kind),
        crate::json::quote(&r.error),
        crate::json::quote(why)
    )
}

/// The inputs the door of every generated language refuses, as lines of the same file
/// (§15.204): for each input, the input left out and a value of another kind; for a number, one
/// that is not whole (off its step, for a rate) and one past each end of its range; for a date,
/// a day the calendar does not have, one past each end and a day its koyomi date does not come
/// to; for an enum, a word it does not have; for a truth value, the string "false"; for each
/// `constraint`, a combination it rules out; and for the sequence a walk reads, the same of the
/// sequence and of one element's fields, and one element past what a count over it can be. Each
/// is a vector of the suite with one thing changed, so the door has one thing to say, and the
/// line carries what the reference evaluator refuses it for. A case the reference evaluator does
/// not refuse — an end the type already closes — is not written.
pub fn door_cases(f: &RuleFile, c: &Checked, vectors: &[Vector]) -> Vec<String> {
    let parsed = |v: &Vector| crate::json::parse(&in_object(f, c, v)).ok();
    let Some(base) = vectors.first().and_then(parsed) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut push = |obj: Json, why: String| {
        let text = obj.compact();
        if !seen.insert(text.clone()) {
            return;
        }
        if let Some(r) = crate::door::refusal(f, c, &obj) {
            out.push(refused_line(&text, &r, &why));
        }
    };
    for i in &f.inputs {
        let name = &i.name.text;
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        push(without(&base, name), tr!("{name} を渡さない", "{name} left out"));
        for (value, why) in wrong_values(c, name, &ty, base.get(name)) {
            push(with(&base, name, value), why);
        }
    }
    for k in &f.constraints {
        if let Some(obj) = breaking(f, c, k, &base) {
            push(obj, tr!("制約 {} を破る組み合わせ", "a combination the constraint {} rules out", k.said()));
        }
    }
    for el in &f.elements {
        let seq = &el.name.text;
        let with_one = vectors
            .iter()
            .find(|v| matches!(v.input.get(seq), Some(Val::Seq(xs)) if !xs.is_empty()))
            .and_then(parsed);
        let top = with_one.clone().unwrap_or_else(|| base.clone());
        push(without(&top, seq), tr!("{seq} を渡さない", "{seq} left out"));
        push(with(&top, seq, Json::Int(0)), tr!("{seq} に並びでない値", "{seq} not a sequence"));
        push(with(&top, seq, Json::Arr(vec![Json::Int(0)])), tr!("{seq} にオブジェクトでない要素", "an element of {seq} that is not an object"));
        if let Some(first) = with_one.as_ref().and_then(|t| t.get(seq)).and_then(|s| s.as_arr()).and_then(|xs| xs.first()).cloned() {
            let cap = crate::door::count_cap(f, c, seq);
            if let Some(cap) = cap.filter(|cap| *cap <= 50) {
                let many = Json::Arr(vec![first.clone(); cap as usize + 1]);
                push(with(&top, seq, many), tr!("{seq} の要素を、数え上げの上限より一つ多く", "{seq} one element past what a count over it can be"));
            }
            // A sum over a field past the top of its range: as many elements at the field's own
            // top as it takes, where that is a few dozen.
            for d in crate::door::sums(f).into_iter().filter(|d| d.over == *seq) {
                let (Some(hi), Some((_, Some(fhi)))) = (c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi), c.ranges.get(&d.column.text).copied()) else {
                    continue;
                };
                if !el.fields.iter().any(|fd| fd.name.text == d.column.text) || fhi.num <= 0 {
                    continue;
                }
                let q = hi.div(fhi);
                let n = q.num.div_euclid(q.den) + 1;
                if n > 50 || cap.is_some_and(|cap| n > cap) {
                    continue;
                }
                let top_value = with(&first, &d.column.text, Json::Int(crate::types::wire_int(fhi, c.wire_scale(&d.column.text))));
                let many = Json::Arr(vec![top_value; n as usize]);
                push(with(&top, seq, many), tr!("{} の合計 {} を範囲の上端の先に", "the sum {1} of {0} past the top of its range", seq, d.name.text));
            }
            for fd in &el.fields {
                let name = &fd.name.text;
                let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
                let mut cases = vec![(None, tr!("{seq} の要素の {name} を渡さない", "{name} left out of an element of {seq}"))];
                for (value, why) in wrong_values(c, name, &ty, first.get(name)) {
                    cases.push((Some(value), tr!("{seq} の要素の {why}", "{why}, in an element of {seq}")));
                }
                for (value, why) in cases {
                    let el1 = match value {
                        None => without(&first, name),
                        Some(v) => with(&first, name, v),
                    };
                    let mut xs = top.get(seq).and_then(|s| s.as_arr()).map(|a| a.to_vec()).unwrap_or_default();
                    xs[0] = el1;
                    push(with(&top, seq, Json::Arr(xs)), why);
                }
            }
        }
    }
    out
}

/// The values of another kind, or outside what the input takes, that its door refuses, each
/// with why it is in the file: the reasons the door has for an input of this type.
fn wrong_values(c: &Checked, name: &str, ty: &Ty, at: Option<&Json>) -> Vec<(Json, String)> {
    let mut out = Vec::new();
    let present = ty.present();
    let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
    match present {
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
            let sc = c.wire_scale(name);
            let (lo_w, hi_w) = (lo.map(|x| crate::types::wire_int(x, sc)), hi.map(|x| crate::types::wire_int(x, sc)));
            let n = at.and_then(|j| j.as_int()).or(lo_w).or(hi_w).unwrap_or(0);
            out.push((Json::Str(n.to_string()), tr!("{name} に数でなく文字列", "{name} as a string, not a number")));
            // Half a step past the vector's own value: the door looks at the kind before the range.
            let half = format!("{n}.5");
            let why = if *present == Ty::Rate { tr!("{name} を刻みに載らない値に", "{name} off its step") } else { tr!("{name} を整数でない数に", "{name} not a whole number") };
            out.push((Json::Frac(half), why));
            if let Some(l) = lo_w {
                out.push((Json::Int(l - 1), tr!("{name} を範囲の下端の一つ下に", "{name} one below its range")));
            }
            if let Some(h) = hi_w {
                out.push((Json::Int(h + 1), tr!("{name} を範囲の上端の一つ上に", "{name} one above its range")));
            }
        }
        Ty::Date => {
            let ord = at.and_then(|j| j.as_str()).and_then(crate::door::civil).map(|(y, m, d)| crate::types::date_ord(y, m, d).num);
            let ord = ord.or(lo.map(|l| l.num / l.den)).unwrap_or(20454);
            out.push((Json::Int(ord), tr!("{name} に日付でなく数", "{name} as a number, not a date")));
            out.push((Json::Str("2026-02-30".into()), tr!("{name} に暦に無い日", "{name} a day the calendar does not have")));
            if let Some(l) = lo {
                out.push((Json::Str(date_text(l.num / l.den - 1)), tr!("{name} を範囲の初日の前の日に", "{name} the day before its range")));
            }
            if let Some(h) = hi {
                out.push((Json::Str(date_text(h.num / h.den + 1)), tr!("{name} を範囲の終わりの次の日に", "{name} the day after its range")));
            }
            if let Some(set) = c.day_sets.get(name) {
                let (first, last) = (lo.map(|l| (l.num / l.den) as i64), hi.map(|h| (h.num / h.den) as i64));
                let from = set.days.first().copied().unwrap_or(0).max(first.unwrap_or(i64::MIN));
                let to = set.days.last().copied().unwrap_or(0).min(last.unwrap_or(i64::MAX));
                if let Some(off) = (from..=to).find(|d| set.days.binary_search(d).is_err()) {
                    let what = set.from.as_ref().map(|fr| format!("koyomi \"{}\" date {}", fr.file, fr.date)).unwrap_or_default();
                    out.push((Json::Str(date_text(off as i128)), tr!("{name} を {what} がとらない日に", "{name} a day {what} does not come to")));
                }
            }
        }
        Ty::Enum(en) => {
            let vs = c.enums.get(en).cloned().unwrap_or_default();
            out.push((Json::Int(0), tr!("{name} に列挙の値でなく数", "{name} as a number, not a value of its enum")));
            let mut word = format!("{}?", vs.first().cloned().unwrap_or_default());
            while vs.contains(&word) {
                word.push('?');
            }
            out.push((Json::Str(word), tr!("{name} に列挙 {en} に無い値", "{name} a value enum {en} does not have")));
            // The values of an ordered enum on either side of the range the input declares
            // (§15.210): the nearest one below it and the nearest one above it.
            if let Some(allowed) = c.enum_ranges.get(name) {
                let first = allowed.first().and_then(|a| vs.iter().position(|v| v == a));
                let last = allowed.last().and_then(|a| vs.iter().position(|v| v == a));
                if let Some(below) = first.and_then(|i| i.checked_sub(1)).and_then(|i| vs.get(i)) {
                    out.push((Json::Str(below.clone()), tr!("{name} を範囲の下端の一つ前の値に", "{name} the value just before its range")));
                }
                if let Some(above) = last.and_then(|i| vs.get(i + 1)) {
                    out.push((Json::Str(above.clone()), tr!("{name} を範囲の上端の一つ後の値に", "{name} the value just after its range")));
                }
            }
        }
        Ty::Bool => {
            out.push((Json::Str("false".into()), tr!("{name} に真偽でなく文字列の \"false\"", "{name} as the string \"false\", not a truth value")));
            out.push((Json::Int(1), tr!("{name} に真偽でなく数の 1", "{name} as the number 1, not a truth value")));
        }
        Ty::Str => {
            out.push((Json::Int(0), tr!("{name} に文字列でなく数", "{name} as a number, not a string")));
        }
        _ => {}
    }
    out
}

/// A combination `k` rules out, both sides inside what each takes: the ends of each side's range
/// (or of its days), and the value the vector gives it, tried in turn.
fn breaking(f: &RuleFile, c: &Checked, k: &Constraint, base: &Json) -> Option<Json> {
    let said = crate::door::constraint(&k.said());
    let candidates = |name: &str| -> Vec<Json> {
        let mut out = Vec::new();
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
        match ty.present() {
            Ty::Date => {
                let mut ords: Vec<i128> = Vec::new();
                match c.day_sets.get(name) {
                    Some(set) => ords.extend(set.days.first().iter().chain(set.days.last().iter()).map(|d| **d as i128)),
                    None => ords.extend(lo.iter().chain(hi.iter()).map(|x| x.num / x.den)),
                }
                out.extend(ords.into_iter().map(|o| Json::Str(date_text(o))));
            }
            _ => {
                let sc = c.wire_scale(name);
                out.extend(lo.iter().chain(hi.iter()).map(|x| Json::Int(crate::types::wire_int(*x, sc))));
            }
        }
        if let Some(v) = base.get(name) {
            out.push(v.clone());
        }
        out
    };
    // Every name's candidates, combined: the first combination the door refuses for this
    // constraint and nothing before it. A side that adds names up has a candidate list per name
    // (§15.216).
    let names: Vec<&String> = k.names().collect();
    let lists: Vec<Vec<Json>> = names.iter().map(|n| candidates(n)).collect();
    let mut at = vec![0usize; names.len()];
    if lists.iter().any(|l| l.is_empty()) {
        return None;
    }
    loop {
        let mut obj = base.clone();
        for (i, n) in names.iter().enumerate() {
            obj = with(&obj, n, lists[i][at[i]].clone());
        }
        if crate::door::refusal(f, c, &obj).is_some_and(|x| x.error == said) {
            return Some(obj);
        }
        let mut i = 0;
        loop {
            if i == at.len() {
                return None;
            }
            at[i] += 1;
            if at[i] < lists[i].len() {
                break;
            }
            at[i] = 0;
            i += 1;
        }
    }
}

fn date_text(ord: i128) -> String {
    let (y, m, d) = crate::types::ord_to_date(Rat::int(ord));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The object with `key` left out.
fn without(obj: &Json, key: &str) -> Json {
    match obj {
        Json::Obj(ps) => Json::Obj(ps.iter().filter(|(k, _)| k != key).cloned().collect()),
        other => other.clone(),
    }
}

/// The object with `key` set to `v`, in the place it had.
fn with(obj: &Json, key: &str, v: Json) -> Json {
    match obj {
        Json::Obj(ps) => {
            let mut ps = ps.clone();
            match ps.iter_mut().find(|(k, _)| k == key) {
                Some(p) => p.1 = v,
                None => ps.push((key.to_string(), v)),
            }
            Json::Obj(ps)
        }
        other => other.clone(),
    }
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
pub fn in_object(f: &RuleFile, c: &Checked, v: &Vector) -> String {
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
    // A sequence is named by `elements`, whether a fold reads it or a count does (§15.58); each
    // comes before the inputs, in the order the rule declares them (§15.219).
    for el in &f.elements {
        let Some(Val::Seq(xs)) = v.input.get(&el.name.text) else { continue };
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
    observed_json(c, &v.outputs)
}

/// The outputs of one answer as the `observed` object of a record writes them: each output by its
/// declared name, in the order declared, its value on the wire (§10.2). What the runners print for
/// each answer the function that counts the answers gives (§15.225), too.
pub fn observed_json(c: &Checked, outputs: &[(String, Option<Val>)]) -> String {
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
        outputs.iter().map(|(n, val)| format!("\"{}\":{}", esc(n), one(n, val))).collect();
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
    row_holds_in(c, t, row, &bind(f, c, a))
}

/// The values the reference evaluator gives the columns of `t` for an input a caller can send and
/// no row of `t` takes (§15.190): every input on its step and inside its range, the `constraint`s
/// and koyomi's days kept, every column the table reads given a value — a column a table above
/// leaves unset says nothing about this one — and no row's cells holding of them. The generated
/// code stops on such an input, so it is what E101 can hand over. `None` for any other input.
pub fn no_row_takes(f: &RuleFile, c: &Checked, t: &Table, a: &BTreeMap<String, Val>) -> Option<BTreeMap<String, Val>> {
    let sendable = f.inputs.iter().all(|i| {
        let Some(ty) = c.ty_of(&i.name.text) else { return false };
        match a.get(&i.name.text) {
            Some(Val::Num(v)) => {
                within(c, &i.name.text, *v)
                    && v.checked_div(crate::coverage::quantum(c, &i.name.text, &ty)).is_some_and(|k| k.is_int())
            }
            Some(_) => true,
            None => false,
        }
    });
    if !sendable || !allowed(f, a) || !days_ok(c, a) {
        return None;
    }
    let binds = bind(f, c, a);
    let none = t.inputs.iter().all(|(col, _)| binds.contains_key(col)) && t.rows.iter().all(|row| !row_holds_in(c, t, row, &binds));
    none.then(|| t.inputs.iter().filter_map(|(col, _)| Some((col.clone(), binds.get(col)?.clone()))).collect())
}

fn row_holds_in(c: &Checked, t: &Table, row: &Row, binds: &BTreeMap<String, Val>) -> bool {
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

/// The rows of a set among `rows` for which the search the vector generator runs for a row finds
/// no input a caller can send that makes the row win: W128 asks it of the rows with a cell on a
/// derived or `define` column, whose point on the axes is not an input (§15.209). It starts where
/// the generator does (every input at its first candidate) and holds what it finds to the ranges,
/// the `constraint` lines and koyomi's days.
pub fn rows_without_input(f: &RuleFile, c: &Checked, set: &crate::defset::DefSet, rows: &[usize]) -> Vec<usize> {
    if rows.is_empty() {
        return Vec::new();
    }
    // The rows an example of the rule reaches: an author who knows of an input the search misses
    // writes it as an example, and the row is decided.
    let t = &set.table;
    let mut by_example: BTreeSet<usize> = BTreeSet::new();
    for a in example_inputs(f, c) {
        if !allowed(f, &a) || !days_ok(c, &a) {
            continue;
        }
        let (_, fired, _) = eval::run_bindings(f, c, a.into_iter().collect());
        for &ri in rows {
            if fired.contains(&eval::row_tag(set.row_table(ri), t.rows[ri].index)) {
                by_example.insert(ri);
            }
        }
    }
    let cands = candidates(f, c);
    let Some(seed) = f.inputs.iter().map(|x| Some((x.name.text.clone(), cands.get(&x.name.text)?.first()?.clone()))).collect::<Option<BTreeMap<String, Val>>>() else {
        return rows.iter().copied().filter(|ri| !by_example.contains(ri)).collect();
    };
    let left: Vec<usize> = rows
        .iter()
        .copied()
        .filter(|ri| !by_example.contains(ri))
        .filter(|&ri| !win_row(f, c, &cands, &seed, set, ri).is_some_and(|a| allowed(f, &a) && days_ok(c, &a)))
        .collect();
    if left.is_empty() {
        return left;
    }
    // What the row-by-row search misses — a row whose columns pull one input two ways, say — the
    // candidates taken together may still reach: every combination of them, up to a bound, each
    // run through the reference evaluator.
    let mut combos: Vec<BTreeMap<String, Val>> = vec![BTreeMap::new()];
    for i in &f.inputs {
        let Some(vs) = cands.get(&i.name.text) else { return left };
        let mut next = Vec::new();
        for base in &combos {
            for v in vs {
                let mut m = base.clone();
                m.insert(i.name.text.clone(), v.clone());
                next.push(m);
            }
        }
        if next.len() > 4096 {
            return left;
        }
        combos = next;
    }
    let mut reached: BTreeSet<usize> = BTreeSet::new();
    for a in combos {
        if !allowed(f, &a) || !days_ok(c, &a) {
            continue;
        }
        let (_, fired, _) = eval::run_bindings(f, c, a.into_iter().collect());
        for &ri in &left {
            if fired.contains(&eval::row_tag(set.row_table(ri), t.rows[ri].index)) {
                reached.insert(ri);
            }
        }
        if reached.len() == left.len() {
            break;
        }
    }
    left.into_iter().filter(|ri| !reached.contains(ri)).collect()
}

/// An input to start from: the first candidate value of every input. `pair_witness` builds on it,
/// and so does the input E101 gives behind a gap on derived columns (§15.190).
pub fn first_input(f: &RuleFile, c: &Checked) -> Option<BTreeMap<String, Val>> {
    let cands = candidates(f, c);
    f.inputs.iter().map(|x| Some((x.name.text.clone(), cands.get(&x.name.text)?.first()?.clone()))).collect()
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
    let mut a: BTreeMap<String, Val> = first_input(f, c)?;
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
    /// The values carried along with the state (§15.217): input, output, and where a case
    /// starts. A trace's first call is passed these, and every later one what the call before
    /// answered for them.
    pub more: Vec<(String, String, Val)>,
}

/// What a transition is called in the suite: from, the row that decided it, to. Two calls
/// that go the same way by the same row are the same transition.
pub type TransitionKey = (String, Option<(String, usize)>, String);

/// The transition one call makes, read off the answer, with the answer.
#[allow(clippy::type_complexity)]
pub fn transition_of(f: &RuleFile, c: &Checked, input: &BTreeMap<String, Val>) -> Option<(TransitionKey, Vec<(String, Option<Val>)>)> {
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
    Some(((from, row, to), outs))
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
    let more: Vec<(String, String, Val)> = a.more.iter().map(|it| (it.input.clone(), it.output.clone(), it.initial.clone())).collect();
    let mut out = Traces { initial: a.initial.clone(), finals, traces: Vec::new(), more };
    // A step is passed everything but what it carries: the state, and the further carried values.
    let carried: Vec<String> = std::iter::once(cin.to_string()).chain(a.more.iter().map(|it| it.input.clone())).collect();
    let step_of = |e: &crate::machine::Edge| -> BTreeMap<String, Val> {
        e.inputs.iter().filter(|(k, _)| !carried.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect()
    };
    if a.blocked.is_none() && !a.over_budget && !a.more.is_empty() {
        out.traces = real_traces(&a, &step_of);
    } else if a.blocked.is_none() && !a.over_budget {
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
                let Some(mut p) = a.trace_to_node(&(k1.0.clone(), w, Vec::new())) else { continue };
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
            let Some(mut p) = a.trace_to_node(&(k.0.clone(), a.edges[*e].world.clone(), Vec::new())) else { continue };
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
            let s: BTreeMap<String, Val> = env.into_iter().filter(|(k, _)| !carried.contains(k)).collect();
            if f.inputs.iter().filter(|i| !carried.contains(&i.name.text)).any(|i| !s.contains_key(&i.name.text)) {
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

/// The traces of a machine that carries more than its state (§15.217), made of the calls that
/// really happen: the same obligations — every transition a case makes, and every two that
/// follow one another — met by the shortest sequence among those calls that ends with them.
/// Each call of a trace is passed what the one before answered, so the values a case holds are
/// the ones the trace's own calls were made with.
fn real_traces(a: &crate::machine::Analysis, step_of: &dyn Fn(&crate::machine::Edge) -> BTreeMap<String, Val>) -> Vec<Trace> {
    let real = &a.real;
    let (order, parent) = real.reach();
    let at: std::collections::HashMap<usize, usize> = order.iter().enumerate().map(|(k, &n)| (n, k)).collect();
    let key = |e: &crate::machine::Edge| -> TransitionKey { (e.from.clone(), e.row.clone(), e.to.clone()) };
    let name = |k: &TransitionKey| -> String {
        match &k.1 {
            Some((t, r)) => format!("{} -[{}]-> {}", k.0, eval::row_tag(t, *r), k.2),
            None => format!("{} -> {}", k.0, k.2),
        }
    };
    // The calls a case reaches, nearest first.
    let mut calls: Vec<usize> = (0..real.calls.len()).filter(|&i| real.calls[i].2.is_some() && at.contains_key(&real.calls[i].0)).collect();
    calls.sort_by_key(|&i| at[&real.calls[i].0]);
    let mut out_of: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for &i in &calls {
        out_of.entry(real.calls[i].0).or_default().push(i);
    }
    let mut paths: Vec<(Vec<crate::machine::Edge>, String)> = Vec::new();
    let mut paired: BTreeSet<TransitionKey> = BTreeSet::new();
    let mut pairs: BTreeSet<(TransitionKey, TransitionKey)> = BTreeSet::new();
    const CAP: usize = 4000;
    for &i1 in &calls {
        let (n1, e1, t1) = &real.calls[i1];
        let Some(t1) = t1 else { continue };
        for &i2 in out_of.get(t1).map(|v| v.as_slice()).unwrap_or(&[]) {
            if paths.len() >= CAP {
                break;
            }
            let e2 = &real.calls[i2].1;
            let p = (key(e1), key(e2));
            if !pairs.insert(p.clone()) {
                continue;
            }
            let mut t = real.path(&parent, *n1);
            t.push(e1.clone());
            t.push(e2.clone());
            paired.insert(p.0.clone());
            paired.insert(p.1.clone());
            paths.push((t, tr!("遷移の対: {} → {}", "transition pair: {} then {}", name(&p.0), name(&p.1))));
        }
    }
    let mut single: BTreeSet<TransitionKey> = BTreeSet::new();
    for &i in &calls {
        let (n, e, _) = &real.calls[i];
        let k = key(e);
        if paired.contains(&k) || !single.insert(k.clone()) {
            continue;
        }
        let mut t = real.path(&parent, *n);
        t.push(e.clone());
        paths.push((t, tr!("遷移: {}", "transition: {}", name(&k))));
    }
    // A trace that begins another one is played by the longer one.
    let steps: Vec<Vec<BTreeMap<String, Val>>> = paths.iter().map(|(p, _)| p.iter().map(step_of).collect()).collect();
    let mut kept: Vec<Trace> = Vec::new();
    for (i, (_, why)) in paths.iter().enumerate() {
        let p = &steps[i];
        let prefix = steps.iter().enumerate().any(|(j, q)| j != i && q.len() > p.len() && q.starts_with(p));
        let same_before = steps[..i].iter().any(|q| q == p);
        if !prefix && !same_before {
            kept.push(Trace { why: why.clone(), steps: p.clone() });
        }
    }
    kept
}

/// The traces file: the line that asks a runner for its constants, then one line per call. A
/// machine that carries more than its state names those values on the first line (`more`),
/// and every trace's first call says where they start (`carried`), in the wire's form (§15.217).
pub fn traces_json(f: &RuleFile, c: &Checked, t: &Traces) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let (cin, cout) = f.machine.as_ref().and_then(|m| m.carried()).unwrap_or(("", ""));
    let more_meta = if t.more.is_empty() {
        String::new()
    } else {
        format!(
            ",\"more\":[{}]",
            t.more.iter().map(|(i, o, _)| format!("{{\"in\":\"{}\",\"out\":\"{}\"}}", esc(i), esc(o))).collect::<Vec<_>>().join(",")
        )
    };
    let carried = if t.more.is_empty() {
        String::new()
    } else {
        format!(
            ",\"carried\":{{{}}}",
            t.more.iter().map(|(i, _, v)| format!("\"{}\":{}", esc(i), eval::wval(c, i, v).json())).collect::<Vec<_>>().join(",")
        )
    };
    let mut lines = vec![format!("{{\"machine\":\"meta\",\"carry\":{{\"in\":\"{}\",\"out\":\"{}\"}}{more_meta}}}", esc(cin), esc(cout))];
    for tr in &t.traces {
        for (k, s) in tr.steps.iter().enumerate() {
            let v = Vector { input: s.clone(), outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() };
            if k == 0 {
                lines.push(format!(
                    "{{\"in\":{},\"step\":\"start\",\"state\":\"{}\"{carried},\"why\":\"{}\"}}",
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
    // The values a further carried input starts at, as each language's constants hold them
    // (§15.217).
    let carried = if t.more.is_empty() {
        String::new()
    } else {
        format!(
            ",\"carried\":{{{}}}",
            t.more.iter().map(|(i, _, v)| format!("\"{}\":{}", esc(i), eval::wval(c, i, v).json())).collect::<Vec<_>>().join(",")
        )
    };
    let mut lines = vec![format!(
        "{{\"initial\":\"{}\",\"final\":[{}]{carried}}}",
        esc(&t.initial),
        t.finals.iter().map(|s| format!("\"{}\"", esc(s))).collect::<Vec<_>>().join(",")
    )];
    for tr in &t.traces {
        let mut state = Val::Enum(t.initial.clone());
        let mut held: Vec<Val> = t.more.iter().map(|(_, _, v)| v.clone()).collect();
        for s in &tr.steps {
            let mut input = s.clone();
            input.insert(cin.to_string(), state.clone());
            for ((i, _, _), v) in t.more.iter().zip(&held) {
                input.insert(i.clone(), v.clone());
            }
            let env: std::collections::HashMap<String, Val> = input.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let (outs, _, rows, _) = eval::run_all_traced(f, c, env);
            let next = outs.iter().find(|(k, _)| k == cout).and_then(|(_, v)| v.clone());
            let next_more: Option<Vec<Val>> = t.more.iter().map(|(_, o, _)| outs.iter().find(|(k, _)| k == o).and_then(|(_, v)| v.clone())).collect();
            let v = Vector { input, outputs: outs, trace: Vec::new(), fired: rows, why: String::new() };
            lines.push(expected_json(f, c, &v));
            match (next, next_more) {
                (Some(n), Some(nm)) => {
                    state = n;
                    held = nm;
                }
                _ => break,
            }
        }
    }
    lines.join("\n") + "\n"
}
