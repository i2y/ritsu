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
    add_accepted_edges(f, c, &mut out);
    out
}

/// The values at which each `define` with a range the rule accepts lands on an end of that range
/// (§15.313), added to the candidates of what it is worked out from: the inputs of the call and,
/// for a value of one element, that element's fields. The ends of the range are where the door
/// starts to refuse, and a row over the value near an end is reached only there: a fee of 3%
/// accepted up to 20,000 yen comes to its top only at an amount near 666,667 yen, which no range or
/// cell writes. A rule with no such range has the candidates it always had.
fn add_accepted_edges(f: &RuleFile, c: &Checked, out: &mut BTreeMap<String, Vec<Val>>) {
    if c.accepts.is_empty() {
        return;
    }
    // Every name at its first candidate, then each other candidate of one name at a time: a value
    // that reaches an end only beside one value of an enum (a base of 500 yen for one zone) is
    // solved from there.
    let first: BTreeMap<String, Val> = out.iter().filter_map(|(k, vs)| Some((k.clone(), vs.first()?.clone()))).collect();
    let mut seeds = vec![first.clone()];
    for (k, vs) in out.iter() {
        for v in vs.iter().skip(1) {
            if seeds.len() >= EDGE_SEEDS {
                break;
            }
            let mut s = first.clone();
            s.insert(k.clone(), v.clone());
            seeds.push(s);
        }
    }
    for e in accepted_edges(f, c, &seeds) {
        if !e.inside {
            continue;
        }
        for (k, v) in e.values {
            if let Some(vs) = out.get_mut(&k) {
                if !vs.contains(&v) {
                    vs.push(v);
                }
            }
        }
    }
}

/// Where a `define` with a range the rule accepts sits at one end of the range (§15.313).
pub(crate) struct Edge {
    /// The `define`.
    pub define: String,
    /// The sequence whose elements it is worked out on, by its place among the rule's `elements`;
    /// `None` for a value of the call.
    pub seq: Option<usize>,
    /// The top end, or the bottom.
    pub hi: bool,
    /// On the end, which the door takes, or one step past it, which it refuses.
    pub inside: bool,
    /// The seed it was solved from, by its place among those given.
    pub from: usize,
    /// What it is worked out from at that point: the inputs of the call and, for a value of one
    /// element, that element's fields, side by side.
    pub values: BTreeMap<String, Val>,
}

/// How many points the ends of an accepted range are solved from at most: past these, an end not
/// reached is left without a vector rather than searched for at length.
const EDGE_SEEDS: usize = 16;

/// For each end each `define` with a range the rule accepts writes, the values it is worked out
/// from at which it lands on the end, and at which it lands past it (§15.313), each solved from
/// the first of `seeds` it can be solved from, as `place` solves a column, and confirmed by the
/// evaluator. Only a value worked out before every walk, or on one element in a walk, is looked at.
/// An end the value cannot pass, by what it reaches, has no point past it, and one it cannot come
/// to has none on it either.
pub(crate) fn accepted_edges(f: &RuleFile, c: &Checked, seeds: &[BTreeMap<String, Val>]) -> Vec<Edge> {
    use std::cmp::Ordering::*;
    let st = crate::types::stages(f);
    let mut out = Vec::new();
    for (i, it) in f.items.iter().enumerate() {
        let Item::Define(d) = it else { continue };
        let name = &d.name.text;
        let Some(&(lo, hi)) = c.accepts.get(name) else { continue };
        let Some(ty) = c.ty_of(name) else { continue };
        let ty = ty.present().clone();
        let seq = match st.at(i) {
            crate::types::Stage::Each { seq, .. } => Some(seq),
            crate::types::Stage::Once(0) => None,
            _ => continue,
        };
        let (rl, rh) = c.reach(&d.expr, &ty).unwrap_or((Rat::int(i64::MIN as i128), Rat::int(i64::MAX as i128)));
        let mut movable: Vec<String> = f.inputs.iter().map(|x| x.name.text.clone()).collect();
        if let Some(s) = seq {
            movable.extend(f.elements[s].fields.iter().map(|fd| fd.name.text.clone()));
        }
        let step = Rat::new(1, c.scales.get(name).copied().unwrap_or(1).max(1));
        for (end, top) in [(lo, false), (hi, true)] {
            let Some(end) = end else { continue };
            // How far what it reaches goes past the end: on it (Equal) or beyond it (Greater).
            let past = if top { rh.cmp_to(end) } else { end.cmp_to(rl) };
            for inside in [true, false] {
                if past == Less || (!inside && past != Greater) {
                    continue;
                }
                let target = match (inside, top) {
                    (true, _) => end,
                    (false, true) => end.add(step),
                    (false, false) => end.sub(step),
                };
                let found = seeds.iter().enumerate().find_map(|(si, seed)| {
                    [!top, top].into_iter().find_map(|up| {
                        let a = solve_flat(f, c, seed, &movable, name, target, up)?;
                        let v = flat_value(f, c, &a, name)?;
                        let ok = if inside {
                            !eval::outside_accepted(c, name, &to_val(v, &ty))
                        } else if top {
                            v.cmp_to(end) == Greater
                        } else {
                            v.cmp_to(end) == Less
                        };
                        ok.then_some((si, a))
                    })
                });
                if let Some((from, a)) = found {
                    let values = a.into_iter().filter(|(k, _)| movable.contains(k)).collect();
                    out.push(Edge { define: name.clone(), seq, hi: top, inside, from, values });
                }
            }
        }
    }
    out
}

/// The value of `col` on a flat assignment: the inputs of a call and the fields of one element side
/// by side, the items run without walking (`eval::run_tables`).
fn flat_value(f: &RuleFile, c: &Checked, a: &BTreeMap<String, Val>, col: &str) -> Option<Rat> {
    let (_, _, _, b) = eval::run_tables(f, c, a.clone().into_iter().collect());
    b.get(col).and_then(as_rat)
}

/// `place` over a flat assignment (`flat_value`), moving the names in `movable`: each is taken as
/// far toward `target` as its own range allows, a step brought onto its grid upward with `up` and
/// downward without. The slope is probed with a step ten times larger each time the value does not
/// move, so that a rounded value (`down(amount × 3%, 1JPY)`, which one yen does not move) is still
/// solved for. What comes back is the closest point reached; the caller holds it to what it needs.
fn solve_flat(f: &RuleFile, c: &Checked, seed: &BTreeMap<String, Val>, movable: &[String], col: &str, target: Rat, up: bool) -> Option<BTreeMap<String, Val>> {
    let mut a = seed.clone();
    for _ in 0..=movable.len() {
        let e0 = flat_value(f, c, &a, col)?;
        if e0.cmp_to(target) == std::cmp::Ordering::Equal {
            return Some(a);
        }
        let mut moved = false;
        for x in movable {
            let Some(xty) = c.ty_of(x).map(|t| t.present().clone()) else { continue };
            if !matches!(xty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let Some(x0) = a.get(x).and_then(as_rat) else { continue };
            let Some(e0) = flat_value(f, c, &a, col) else { continue };
            let q = if xty == Ty::Date { Rat::int(1) } else { crate::coverage::quantum(c, x, &xty) };
            let mut k = q;
            let mut slope = None;
            // A probe stays inside what the name is held to — its range, an end it does not write
            // being the end of its type (`wide_range`): E108 holds every step of the arithmetic to
            // int64 there and nowhere else, and a value a hundred sextillion past the range,
            // squared, is past what the evaluator's own numbers hold. Where a step up leaves the
            // range, the same step down is tried, and where both do there is no slope to find.
            let held = c.wide_range(x);
            for _ in 0..24 {
                let inside = |v: Rat| held.is_none_or(|(lo, hi)| v.cmp_to(lo) != std::cmp::Ordering::Less && v.cmp_to(hi) != std::cmp::Ordering::Greater);
                let step = if inside(x0.add(k)) {
                    k
                } else if inside(x0.sub(k)) {
                    Rat::zero().sub(k)
                } else {
                    break;
                };
                let mut probe = a.clone();
                probe.insert(x.clone(), to_val(x0.add(step), &xty));
                if let Some(e1) = flat_value(f, c, &probe, col) {
                    if e1.cmp_to(e0) != std::cmp::Ordering::Equal {
                        slope = Some(e1.sub(e0).div(step));
                        break;
                    }
                }
                k = k.mul(Rat::int(10));
            }
            let Some(slope) = slope else { continue };
            let want = x0.add(target.sub(e0).div(slope));
            let xn = snap_toward(c, x, want, q, up);
            if xn.cmp_to(x0) == std::cmp::Ordering::Equal {
                continue;
            }
            let mut next = a.clone();
            next.insert(x.clone(), to_val(xn, &xty));
            let Some(e2) = flat_value(f, c, &next, col) else { continue };
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
            break;
        }
    }
    Some(a)
}

/// A value brought inside the declared range of `name` and onto its grid, upward with `up`.
fn snap_toward(c: &Checked, name: &str, v: Rat, q: Rat, up: bool) -> Rat {
    use std::cmp::Ordering::*;
    let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
    let mut v = if q.num == 0 {
        v
    } else if up {
        crate::grid::ceil_to(v, q)
    } else {
        crate::grid::floor_to(v, q)
    };
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
    v
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
    out.extend(f.never_cells(col).cloned());
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
            // A `count` takes the elements whose column is one value, and a `max` or a `min` looks
            // at those its `where` names: that value is a class of its own, beside the values it
            // leaves out. Without it, an enum no table reads had one candidate, the first value —
            // every element was counted, or none was, and a count read by a table never stood on
            // the far side of its boundary (§15.323).
            Item::Agg(d) => {
                let named = match (&d.kind, &d.filter) {
                    (crate::ast::AggKind::Count, _) => (d.column.text == col).then_some(d.value.as_ref()).flatten(),
                    (_, Some((c, Some(v)))) => (c.text == col).then_some(v),
                    _ => None,
                };
                if let Some(v) = named {
                    out.push(Cell::Lit(Lit::Word(v.text.clone())));
                }
            }
            Item::Fold(_) => {}
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
    out.extend(f.never_cells(col).cloned());
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
    // The boundaries of a `never` line are boundaries of what the door takes (§15.282).
    for cell in f.never_cells(col) {
        let lits: Vec<&Lit> = match cell {
            Cell::Cmp(cs) => cs.iter().map(|(_, l)| l).collect(),
            Cell::Lit(l) => vec![l],
            Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
            _ => Vec::new(),
        };
        for v in lits.into_iter().filter_map(lit) {
            push(v, &mut set);
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
fn baseline(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>) -> BTreeMap<String, Val> {
    let mut base: BTreeMap<String, Val> = f.inputs
        .iter()
        .map(|i| i.name.text.clone())
        .map(|n| (n.clone(), cands[&n].first().cloned().unwrap()))
        .collect();
    // The points around a row and the sweeps of one column hold every other input at the baseline,
    // so a baseline under a `never` line (§15.282) would lose all of them: a truth value's first
    // candidate is `true`, and `never a = true, b = true` is what a rule writes first. A line moves
    // one of its own names, the last it can, to a candidate that clears every line, or failing that
    // this one.
    let under = |b: &BTreeMap<String, Val>| f.door_nevers().filter(|n| !never_clear(n, c, &|m| b.get(m))).count();
    for _ in 0..=f.nevers.len() {
        let Some(line) = f.door_nevers().find(|n| !never_clear(n, c, &|m| base.get(m))) else { break };
        let mut moves: Vec<BTreeMap<String, Val>> = Vec::new();
        for k in line.conds.iter().rev() {
            for v in cands.get(&k.name.text).into_iter().flatten() {
                let mut b = base.clone();
                b.insert(k.name.text.clone(), v.clone());
                if never_clear(line, c, &|m| b.get(m)) {
                    moves.push(b);
                }
            }
        }
        let Some(best) = moves.into_iter().min_by_key(|b| under(b)) else { break };
        base = best;
    }
    base
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
        allowed(f, c, a) && bind(f, c, a).get(name).and_then(as_rat).is_some_and(|v| is_tie(v, g))
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
                if allowed(f, c, &a) {
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
        if !allowed(f, c, a) {
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
    let scalars: BTreeMap<String, Val> = baseline(f, c, cands)
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
    for (a, why) in pool_scalars(f, c, cands, &fields, &BTreeMap::new()) {
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
    held: &BTreeMap<String, Val>,
) -> Vec<(BTreeMap<String, Val>, String)> {
    pool_inner_at(f, c, cands, held)
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

/// The most elements at the largest weight `heavy_case` sets side by side to take the total of the
/// weights past int64.
const HEAVY_MOST: usize = 8;

/// The case that holds the generated code to refusing weights whose total passes int64 (§15.226):
/// the sequence of the pick by weight with the fewest copies of one element that take the total
/// past int64, and every other input as the answered vector it came from has it. The element is the
/// one of largest weight among the answered vectors' elements, each weighed alone with the drawn
/// value at the bottom of its range; a field marked `unique` takes a value of its own in each copy
/// (§15.290). Two copies never pass int64, which E108 holds the rule to, and no case is made where
/// it takes more than eight: the weights are then small enough that the case would say nothing the
/// other vectors do not. Nor where the door refuses the copies for something else first — a `count`
/// over the sequence that holds it to fewer elements, a `sum` past its range.
fn heavy_case(f: &RuleFile, c: &Checked, answered: &[Vector]) -> Option<(BTreeMap<String, Val>, String)> {
    let fold = f.folds().find(|d| d.weighted())?;
    let draw = fold.draw.as_ref()?.text.clone();
    let lo = c.ranges.get(&draw).and_then(|(lo, _)| *lo)?;
    let seq = fold.over.clone();
    let el = f.elements.iter().find(|el| el.name.text == seq)?;
    // The element of largest weight, weighed alone with the drawn value at the bottom of its range.
    let mut best: Option<(Rat, &BTreeMap<String, Val>, &BTreeMap<String, Val>)> = None;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for v in answered {
        let Some(Val::Seq(xs)) = v.input.get(&seq) else { continue };
        for e in xs {
            if !seen.insert(e.iter().map(|(k, x)| format!("{k}={}", show(x))).collect::<Vec<_>>().join(",")) {
                continue;
            }
            let mut a = v.input.clone();
            a.insert(seq.clone(), Val::Seq(vec![e.clone()]));
            a.insert(draw.clone(), Val::Num(lo));
            let Some(cuts) = eval::weighted_cuts(f, c, a.into_iter().collect()) else { continue };
            let [w] = cuts.as_slice() else { continue };
            if w.num > 0 && best.is_none_or(|(b, _, _)| w.cmp_to(b) == std::cmp::Ordering::Greater) {
                best = Some((*w, e, &v.input));
            }
        }
    }
    let (_, e, base) = best?;
    let said = crate::door::weights_past(&seq);
    for k in 2..=HEAVY_MOST {
        let mut a = base.clone();
        a.insert(seq.clone(), Val::Seq(apart(c, el, e, k)?));
        if eval::weights_past(f, c, a.clone().into_iter().collect()).is_none() {
            continue;
        }
        // The door says the weights, and nothing before them.
        let v = Vector { input: a, outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() };
        let j = crate::json::parse(&in_object(f, c, &v)).ok()?;
        if crate::door::refusal(f, c, &j).is_none_or(|r| r.error != said) {
            return None;
        }
        let why = tr!(
            "{seq} の、重みがいちばん大きい要素 {k} 件（重みの合計が int64 を越える）",
            "{k} elements of {seq} at the largest weight (the total of the weights passes int64)"
        );
        return Some((v.input, why));
    }
    None
}

/// `k` copies of the element `e` of the sequence `el`, each with a value of its own in every field
/// marked `unique` (§15.290), the element's own first, as `spread` gives them; `None` where a field
/// does not have that many.
fn apart(c: &Checked, el: &ElementsDecl, e: &BTreeMap<String, Val>, k: usize) -> Option<Vec<BTreeMap<String, Val>>> {
    let mut xs = vec![e.clone(); k];
    for fd in el.fields.iter().filter(|fd| fd.unique.is_some()) {
        let name = &fd.name.text;
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        let at = e.get(name).and_then(|v| crate::json::parse(&wire_value(c, name, v)).ok());
        for (x, j) in xs.iter_mut().zip(spread(c, name, &ty, at.as_ref(), k)?) {
            x.insert(name.clone(), crate::door::value(c, name, &j)?);
        }
    }
    Some(xs)
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
    let mut base: BTreeMap<String, Val> = baseline(f, c, &cands).into_iter().filter(|(k, _)| !fields.contains(k)).collect();
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
    out.retain(|m| allowed(f, c, m) && days_ok(c, m));
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
    let scalars: BTreeMap<String, Val> = baseline(f, c, cands).into_iter().filter(|(k, _)| !fields.contains(k)).collect();
    let combos: Vec<Vec<BTreeMap<String, Val>>> = f.elements.iter().map(|el| element_combos(el, cands)).collect();
    // Each sequence at rest: one element, its first candidate, so that a walk over it has one; or,
    // where the walk refuses that one element for a `count` or a `sum` under its bottom, a
    // sequence it answers, made from it (`at_rest`, §15.323).
    let firsts: Vec<Val> = combos.iter().map(|cs| Val::Seq(cs.first().cloned().into_iter().collect())).collect();
    let fitted = {
        let mut m = scalars.clone();
        for (el, v) in f.elements.iter().zip(&firsts) {
            m.insert(el.name.text.clone(), v.clone());
        }
        at_rest(f, c, cands, &m)
    };
    let rest: Vec<Val> = f.elements.iter().zip(firsts).map(|(el, v)| fitted.get(&el.name.text).cloned().unwrap_or(v)).collect();
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
                    // What a sequence brings the summary to. One a walk refuses because another
                    // summary over it comes outside its range still brings this one somewhere: the
                    // case is aimed all the same, and brought inside the other's range when the
                    // audit is left owed it (`fit_summaries`, §15.323). Without it, a sequence of
                    // one element under a count's bottom of two aimed at nothing.
                    let answer = |xs: Vec<BTreeMap<String, Val>>| -> Option<Val> {
                        let m = with(&scalars, s, seq(xs));
                        binds_of(&m).get(&d.name.text).cloned().or_else(|| {
                            let env: std::collections::HashMap<String, Val> = m.into_iter().collect();
                            eval::summary_outside(f, c, env.clone())?;
                            eval::summaries_past(f, c, env).get(&d.name.text).cloned()
                        })
                    };
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
                                    let Some((_, want, _)) = crate::coverage::pairs_on(c, other, oc, &oty).into_iter().next() else { continue };
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
                        for (b, inside, outside) in crate::coverage::pairs_on(c, col, cell, &ty) {
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
                for (b, inside, outside) in crate::coverage::pairs_on(c, col, cell, &ty) {
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
    // still exercised. They are looked for with no sequence, as they always were, unless a walk
    // refuses an empty one; then with the sequences at rest (`held_at_rest`, §15.323).
    let held = held_at_rest(f, c, &scalars, &f.elements.iter().map(|el| el.name.text.clone()).zip(rest.iter().cloned()).collect());
    for (a, why) in pool_scalars(f, c, cands, &fields, &held) {
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
        baseline(f, c, cands).into_iter().filter(|(k, _)| !fields.contains(k)).collect();

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
                for (b, inside, outside) in crate::coverage::pairs_on(c, col, cell, &ty) {
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
    // still exercised. Where the walk refuses that one element — a `count` or a `sum` over the
    // sequence with a bottom it does not reach — they are carried by a sequence it answers, made
    // from it (`at_rest`, §15.323).
    if let Some(e) = combos.first() {
        let mut first = scalars.clone();
        first.insert(over.clone(), seq(vec![e]));
        let carry = at_rest(f, c, cands, &first).get(&over).cloned().unwrap_or_else(|| seq(vec![e]));
        let held = held_at_rest(f, c, &scalars, &BTreeMap::from([(over.clone(), carry.clone())]));
        for (a, why) in pool_scalars(f, c, cands, &fields, &held) {
            let mut m = a.clone();
            m.insert(over.clone(), carry.clone());
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
                let pairs = crate::coverage::pairs_on(c, col, cell, &ty);
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
    pool_inner_at(f, c, cands, &BTreeMap::new())
}

/// The sequences the values of one call are looked for with, in `pool_inner`: none, as they always
/// were — a walk over a sequence that is not there walks no element — unless the walks refuse that,
/// a `count` or a `sum` with a bottom above zero (§15.314); then `rest`, the sequences at rest. With
/// no sequence, every case the rows and the boundaries of a table over the values of the call are
/// looked for with was refused, and a row such a case had found was left with none (§15.323).
fn held_at_rest(f: &RuleFile, c: &Checked, scalars: &BTreeMap<String, Val>, rest: &BTreeMap<String, Val>) -> BTreeMap<String, Val> {
    if eval::summary_outside(f, c, scalars.clone().into_iter().collect()).is_some() { rest.clone() } else { BTreeMap::new() }
}

/// `pool_inner`, with `held` — the sequences at rest, or nothing — in every case it makes.
fn pool_inner_at(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>, held: &BTreeMap<String, Val>) -> Vec<(BTreeMap<String, Val>, String)> {
    let names: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let mut base = baseline(f, c, cands);
    base.extend(held.iter().map(|(k, v)| (k.clone(), v.clone())));
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
                for (b, inside, outside) in crate::coverage::pairs_on(c, col, cell, &ty) {
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

/// The same, for a value that is about to be read by a person: as the rule writes a value of its
/// type (`types::fmt_val`), in its unit and with its digits grouped as the languages group them,
/// `30_000JPY`, `1200g`, `10%` (a bare `0.1` reads as either a tenth or ten percent), so that a
/// number said is one the file can take as it is (§15.333). A value of a name the rule does not
/// type is its digits alone. The elements of a sequence say their fields the same way, each by its
/// own name (`lines=[price=499JPY,wrapped=true]`).
pub fn show_named(c: &Checked, name: &str, v: &Val) -> String {
    match (v, c.ty_of(name)) {
        // An optional value that is there is a value of the type it wraps (§15.249).
        (Val::Num(r), Some(t)) => crate::types::fmt_val(*r, &t),
        (Val::Num(r), None) => crate::types::written_num(*r),
        (Val::Seq(xs), _) => {
            let one = |e: &BTreeMap<String, Val>| e.iter().map(|(k, v)| format!("{k}={}", show_named(c, k, v))).collect::<Vec<_>>().join(",");
            format!("[{}]", xs.iter().map(one).collect::<Vec<_>>().join(";"))
        }
        _ => show(v),
    }
}

/// A value as data a page lays out, not as a sentence says it: a rate in percent (`10%`), and any
/// other value as [`show`] writes it, with no unit and no grouping (`1500`). The table of what two
/// versions answer (`vdiff::Map`) hands its cells over in this form: the page that draws it groups
/// the digits its own way and says the unit once, above the column.
pub fn show_plain(c: &Checked, name: &str, v: &Val) -> String {
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
pub fn example_inputs(f: &RuleFile, c: &Checked) -> Vec<BTreeMap<String, Val>> {
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

/// One case run through the reference evaluator as the suite runs it, or `None` where the suite
/// would not keep it as a case with an answer: a combination the door refuses (a `constraint`, a
/// day koyomi does not give, a `define` outside the range the rule accepts for it), two elements
/// both taking under `take_unique`, a `sum` or a `count` outside its range once the walk is over,
/// a drawn value not below the total of the weights, weights whose total passes int64.
/// The customer page asks it for the other side of a threshold (§15.302).
pub fn answer(f: &RuleFile, c: &Checked, a: &BTreeMap<String, Val>, why: &str) -> Option<Vector> {
    if !allowed(f, c, a) || !days_ok(c, a) {
        return None;
    }
    let env = || a.clone().into_iter().collect();
    if (f.walks() && eval::contradiction(f, c, env()).is_some())
        || eval::summary_outside(f, c, env()).is_some()
        || eval::drawn_past(f, c, env()).is_some()
        || eval::weights_past(f, c, env()).is_some()
    {
        return None;
    }
    let (outputs, trace, fired, _) = eval::run_all_traced(f, c, env());
    Some(Vector { input: a.clone(), outputs, trace, fired, why: why.to_string() })
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
/// what keeps the suite and the proof talking about the same set of inputs. An input on which a
/// `define` falls outside the range the rule accepts for it is refused too, where the value is
/// worked out (§15.313), and is not a case either.
pub fn allowed(f: &RuleFile, c: &Checked, a: &BTreeMap<String, Val>) -> bool {
    promised(f, c, a) && (c.accepts.is_empty() || eval::define_outside(f, c, a.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).is_none())
}

/// Whether an input is one the caller's promises let through: every `constraint` holds, no `never`
/// line of the door takes it, and no sequence repeats a value of a field marked `unique`. What a
/// `define` accepts is not asked.
fn promised(f: &RuleFile, c: &Checked, a: &BTreeMap<String, Val>) -> bool {
    f.constraints.iter().all(|k| constraint_holds(k, &|n| a.get(n))) && f.door_nevers().all(|n| never_clear(n, c, &|m| a.get(m))) && distinct(f, a)
}

/// Whether no sequence has two elements with the same value of a field marked `unique` (§15.290):
/// the generated code refuses one that does at its door, as it refuses what a `constraint` rules
/// out.
pub fn distinct(f: &RuleFile, a: &BTreeMap<String, Val>) -> bool {
    f.elements.iter().all(|el| match a.get(&el.name.text) {
        Some(Val::Seq(xs)) => crate::door::repeated(el, xs).is_none(),
        _ => true,
    })
}

/// Whether these values stay clear of a `never` line (§15.282): not every condition holds, each
/// read as a table's cell is. A line with a name that has no value is not held to it, the way the
/// door compares only what it was passed.
pub fn never_clear<'v>(n: &crate::ast::NeverLine, c: &Checked, value: &dyn Fn(&str) -> Option<&'v Val>) -> bool {
    !n.conds.iter().all(|k| {
        let name = k.name.text.as_str();
        value(name).is_some_and(|v| eval::cell_matches(c, &k.cell, v, &c.ty_of(name).unwrap_or(Ty::Unknown)))
    })
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

/// The inputs the generated vectors draw from, each one a call the door takes: where a question about
/// one call looks when the rule's space does not cut into cells (§15.287).
pub(crate) fn plain_inputs(f: &RuleFile, c: &Checked) -> Vec<BTreeMap<String, Val>> {
    let cands = candidates(f, c);
    pool(f, c, &cands).into_iter().map(|(a, _)| a).filter(|a| allowed(f, c, a) && days_ok(c, a)).collect()
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
    /// The calls on an end of a range the rule writes — the end of an input or of a field of an
    /// element, as many elements as a count lets a sequence have, or a sequence with none — where
    /// none of the cases tried on the end is answered (§15.314): one call for each such end, which
    /// `vectors/<alias>.refused.jsonl` holds with what the reference evaluator refuses it for. In a
    /// suite with no answered case at all, one call for each `define` whose range refuses a case
    /// of the pool as well. They are no part of what the audit of the fold transitions reads.
    pub on_ends: Vec<Vector>,
}

pub fn generate(f: &RuleFile, c: &Checked) -> Vec<Vector> {
    suite(f, c).vectors
}

/// Evaluate the candidate population and deterministically select a subset that satisfies
/// coverage.
pub fn suite(f: &RuleFile, c: &Checked) -> Suite {
    let cands = candidates(f, c);
    // The cases of the pool that a `define` refuses, by the range the rule accepts for it, are
    // kept aside, the first few. Such a range may leave out every case the pool has: the pool aims
    // at nothing on a value worked out once a walk is over, whose ends are then solved from these
    // (`walked_edges`); and where no case at all is answered in the end, they are what the suite
    // has to hold the generated code to.
    let mut outside: Vec<Vector> = Vec::new();
    let raw: Vec<_> = pool(f, c, &cands)
        .into_iter()
        .filter(|(a, _)| {
            if !days_ok(c, a) {
                return false;
            }
            if allowed(f, c, a) {
                return true;
            }
            if outside.len() < EDGE_SEEDS && promised(f, c, a) {
                outside.push(Vector { input: a.clone(), outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() });
            }
            false
        })
        .collect();

    let key_of = |a: &BTreeMap<String, Val>| -> String {
        a.iter().map(|(k, v)| format!("{k}={}", show(v))).collect::<Vec<_>>().join(",")
    };
    let mut evaluated: Vec<Vector> = Vec::new();
    let mut refused: Vec<Vector> = Vec::new();
    let mut past_sums: Vec<String> = Vec::new();
    let mut off_range: Vec<(BTreeMap<String, Val>, String)> = Vec::new();
    // The inputs seen: an answered case by where it is in `evaluated`, a refused one by `None`.
    let mut seen_in: BTreeMap<String, Option<usize>> = BTreeMap::new();
    for (a, why) in raw {
        let key = key_of(&a);
        if seen_in.contains_key(&key) {
            continue;
        }
        seen_in.insert(key.clone(), None);
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        // A case the reference evaluator refuses is not a case with an answer, and it is not
        // nothing either. For a rule that walks a sequence, no answer means exactly one thing:
        // two elements both took under `take_unique` (§15.56). It goes to the other list,
        // where `rulec test` holds the generated code to refusing it too.
        if f.walks() && crate::eval::contradiction(f, c, a.clone().into_iter().collect()).is_some() {
            refused.push(Vector { input: a, outputs: outs, trace, fired, why });
            continue;
        }
        // Weights whose total passes int64 are refused the moment it does (§15.226): one such case
        // holds the generated code to refusing it.
        if let Some(n) = crate::eval::weights_past(f, c, a.clone().into_iter().collect()) {
            let key = format!("weights:{n}");
            if !past_sums.contains(&key) {
                past_sums.push(key);
                refused.push(Vector { input: a, outputs: outs, trace, fired, why });
            }
            continue;
        }
        // A sum past the top of its range is refused where the walk ends (§15.100), and so is a
        // sum or a count under the bottom of its own (§15.314): one such case per summary and end
        // holds the generated code to refusing it, and the rest of them say nothing more. The
        // first few are kept aside as well, to be brought inside the range where the audit is
        // left owed something (`fit_summaries`, §15.323).
        if let Some(p) = crate::eval::summary_outside(f, c, a.clone().into_iter().collect()) {
            if off_range.len() < FIT_POOL {
                off_range.push((a.clone(), why.clone()));
            }
            let key = if p.below { format!("under:{}", p.name) } else { p.name };
            if !past_sums.contains(&key) {
                past_sums.push(key);
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
        seen_in.insert(key, Some(evaluated.len()));
        evaluated.push(Vector { input: a, outputs: outs, trace, fired, why });
    }
    // A case of the pool seldom takes the weights of a pick past int64: two elements never do
    // (E108), and few cases have more of them at the top. Where none did, the case is made.
    if !past_sums.iter().any(|k| k.starts_with("weights:")) {
        if let Some((a, why)) = heavy_case(f, c, &evaluated) {
            let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
            refused.push(Vector { input: a, outputs: outs, trace, fired, why });
        }
    }

    // The examples take part in the audit like any other vector, and are never dropped from the
    // suite afterwards — they are the one thing in it that a person chose.
    let mut forced: BTreeSet<usize> = BTreeSet::new();
    for (n, a) in example_inputs(f, c).into_iter().enumerate() {
        let key = key_of(&a);
        if let Some(&i) = seen_in.get(&key) {
            forced.extend(i);
            continue;
        }
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        seen_in.insert(key, Some(evaluated.len()));
        forced.insert(evaluated.len());
        evaluated.push(Vector {
            input: a,
            outputs: outs,
            trace,
            fired,
            why: tr!("例 {}行目", "example row {}", n + 1),
        });
    }

    // Each end of the range the rule accepts for a `define`, on the end (§15.313): the call the
    // door takes nearest to where it starts refusing, which the line one step past it in the
    // refused cases answers.
    let ends: Vec<(Edge, Vector)> = edge_inputs(f, c, &evaluated, true);
    for (e, v) in ends {
        let a = v.input;
        let key = key_of(&a);
        if let Some(&i) = seen_in.get(&key) {
            forced.extend(i);
            continue;
        }
        if !allowed(f, c, &a) || !days_ok(c, &a) {
            continue;
        }
        let why = if e.hi {
            tr!("定義 {} を、受け付ける範囲の上端に", "the definition {} at the top of the range it accepts", e.define)
        } else {
            tr!("定義 {} を、受け付ける範囲の下端に", "the definition {} at the bottom of the range it accepts", e.define)
        };
        let Some(vec) = answer(f, c, &a, &why) else { continue };
        seen_in.insert(key, Some(evaluated.len()));
        forced.insert(evaluated.len());
        evaluated.push(vec);
    }

    // Each end a walk holds a `sum` or a `count` to once it is over, on the end (§15.100, §15.314):
    // the call the walk answers nearest to where it starts refusing, which the line one step past
    // it in the refused cases answers.
    let on_ends: Vec<(SummaryEdge, BTreeMap<String, Val>)> = summary_edges(f, c, &evaluated).into_iter().filter(|(e, _)| e.inside).collect();
    for (e, a) in on_ends {
        let key = key_of(&a);
        if let Some(&i) = seen_in.get(&key) {
            forced.extend(i);
            continue;
        }
        if !allowed(f, c, &a) || !days_ok(c, &a) {
            continue;
        }
        let Some(vec) = answer(f, c, &a, &e.why()) else { continue };
        seen_in.insert(key, Some(evaluated.len()));
        forced.insert(evaluated.len());
        evaluated.push(vec);
    }

    // Each end of the range the rule accepts for a `define` it works out once a walk is over, on
    // the end (§15.313, §15.314): a value read off what a walk answers, come to with a sequence
    // that brings the summary where the value needs it. The cases so far are where it is solved
    // from, and after them the cases of the pool a `define` refuses.
    let walked_ranges = walked_defines(f).iter().any(|d| c.accepts.contains_key(&d.name.text));
    let from: Vec<Vector> = if walked_ranges && !outside.is_empty() { evaluated.iter().chain(outside.iter()).cloned().collect() } else { Vec::new() };
    for (e, a) in walked_edges(f, c, if from.is_empty() { &evaluated } else { &from }).into_iter().filter(|(e, _)| e.inside) {
        let key = key_of(&a);
        if let Some(&i) = seen_in.get(&key) {
            forced.extend(i);
            continue;
        }
        let Some(vec) = answer(f, c, &a, &e.why()) else { continue };
        seen_in.insert(key, Some(evaluated.len()));
        forced.insert(evaluated.len());
        evaluated.push(vec);
    }

    // The call that shows a `possible` line outside a `machine` (§15.287) stays as well: the
    // generated code is held to answering what the line says happens.
    for (said, a) in crate::possible::one_call_inputs(f, c) {
        let key = key_of(&a);
        if let Some(&i) = seen_in.get(&key) {
            forced.extend(i);
            continue;
        }
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        seen_in.insert(key, Some(evaluated.len()));
        forced.insert(evaluated.len());
        evaluated.push(Vector { input: a, outputs: outs, trace, fired, why: said });
    }

    // A row with a cell on a computed column that no vector reached may still be one the check
    // found an input for: the input behind the gap that taking the row out leaves (§15.209,
    // refined). It goes in like any other, so the generated code is run on that row too.
    let fired_rows: BTreeSet<(String, usize)> = evaluated.iter().flat_map(|v| v.fired.iter().cloned()).collect();
    for (table, row, a) in crate::region::gap_inputs(f, c, &|tn, ri| fired_rows.contains(&(tn.to_string(), ri))) {
        let key = key_of(&a);
        if seen_in.contains_key(&key) || !allowed(f, c, &a) || !days_ok(c, &a) {
            continue;
        }
        let (outs, trace, fired, _) = eval::run_all_traced(f, c, a.clone().into_iter().collect());
        seen_in.insert(key, Some(evaluated.len()));
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
    // Where the audit is left owed something, the boundaries the tables write on a value worked
    // out once a walk is over are aimed at (`walked_cases`), and the audit is asked again: what it
    // takes of them stays, beside everything it took before, and the rest are cases like the
    // pool's that no criterion took. A rule the audit was satisfied with has the vectors it had.
    if !audit.missing.is_empty() {
        let before = evaluated.len();
        for (a, why) in walked_cases(f, c, &evaluated) {
            let key = key_of(&a);
            if seen_in.contains_key(&key) {
                continue;
            }
            let Some(vec) = answer(f, c, &a, &why) else { continue };
            seen_in.insert(key, Some(evaluated.len()));
            evaluated.push(vec);
        }
        // The pool's cases a walk refused for a `sum` or a `count` outside its range, brought
        // inside it with what they were aimed at kept (§15.323): a count on a boundary of a table,
        // made of elements whose total comes in under the total's bottom.
        for (a, hold) in places(f, c, &off_range, FIT_SEEDS) {
            let Some((b, why)) = fit_summaries(f, c, a, &cands, &hold) else { continue };
            let key = key_of(&b);
            if seen_in.contains_key(&key) {
                continue;
            }
            let Some(vec) = answer(f, c, &b, &why) else { continue };
            seen_in.insert(key, Some(evaluated.len()));
            evaluated.push(vec);
        }
        if evaluated.len() > before {
            keep.extend(crate::coverage::audit(f, c, "", &evaluated, &refused).witness);
        }
    }
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

    // Each end a `range` writes for an input or for a field of an element, on the end (§15.314):
    // the value itself for an end written with `>=` or `<=`, and the first value inside one
    // written with `>` or `<`, which is read one step in from what is written. The line the
    // refused cases have one step past the end answers this one. Where no case kept so far stands
    // there, the first case the pool has there is kept; else the first case tried that is
    // answered — a case of the suite with the value moved there, for a field of an element in its
    // first element or in that element alone. Where none is, the first of them goes to the
    // refused cases, with what it is refused for.
    let mut no_answer: Vec<Vector> = Vec::new();
    for end in written_ends(f, c) {
        if keep.iter().any(|&i| end.on(&evaluated[i].input)) {
            continue;
        }
        if let Some(i) = evaluated.iter().position(|x| end.on(&x.input)) {
            keep.insert(i);
            continue;
        }
        let tries = end.tries(f, c, &evaluated);
        let why = end.why();
        match tries.iter().find_map(|a| answer(f, c, a, &why)) {
            Some(vec) => {
                seen_in.insert(key_of(&vec.input), Some(evaluated.len()));
                keep.insert(evaluated.len());
                evaluated.push(vec);
            }
            None => no_answer.extend(tries.into_iter().next().map(|input| Vector { input, outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: unanswered(&why) })),
        }
    }

    // As many elements as a count lets a sequence have (§15.314): the longest sequence the door
    // takes, which the line of the refused cases with one element more answers. Where no case
    // kept so far has that many, the first case the pool has of that length is kept; else the
    // first sequence tried that is answered — that many copies of an element the suite has, each
    // with a value of its own in a field marked `unique`. Where none is, the first of them goes to
    // the refused cases.
    for el in &f.elements {
        let seq = &el.name.text;
        let Some(cap) = crate::door::count_cap(f, c, seq).filter(|cap| (1..=EDGE_ELEMENTS).contains(cap)) else { continue };
        let full = |a: &BTreeMap<String, Val>| matches!(a.get(seq), Some(Val::Seq(xs)) if xs.len() as i128 == cap);
        if keep.iter().any(|&i| full(&evaluated[i].input)) {
            continue;
        }
        if let Some(i) = evaluated.iter().position(|x| full(&x.input)) {
            keep.insert(i);
            continue;
        }
        let mut known: Vec<&BTreeMap<String, Val>> = Vec::new();
        for v in &evaluated {
            let Some(Val::Seq(xs)) = v.input.get(seq) else { continue };
            for e in xs {
                if known.len() < 32 && !known.contains(&e) {
                    known.push(e);
                }
            }
        }
        let bases = evaluated.iter().filter(|v| matches!(v.input.get(seq), Some(Val::Seq(xs)) if !xs.is_empty())).take(END_BASES);
        let mut tries: Vec<BTreeMap<String, Val>> = Vec::new();
        for base in bases {
            for e in &known {
                let Some(xs) = apart(c, el, e, cap as usize) else { continue };
                let mut a = base.input.clone();
                a.insert(seq.clone(), Val::Seq(xs));
                tries.push(a);
            }
        }
        let why = full_why(seq);
        match tries.iter().find_map(|a| answer(f, c, a, &why)) {
            Some(vec) => {
                seen_in.insert(key_of(&vec.input), Some(evaluated.len()));
                keep.insert(evaluated.len());
                evaluated.push(vec);
            }
            None => no_answer.extend(tries.into_iter().next().map(|input| Vector { input, outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: unanswered(&why) })),
        }
    }

    // No elements (§15.314): the shortest sequence there is, the other end of how long one can be.
    // Where no case kept so far gives the sequence empty and no walk the pool found refused does,
    // the first case the pool has of it is kept; else the first case of the suite that is answered
    // with the sequence emptied. Where none is, the first of them goes to the refused cases.
    for el in &f.elements {
        let seq = &el.name.text;
        if keep.iter().any(|&i| no_elements(&evaluated[i].input, seq)) || refused.iter().any(|v| no_elements(&v.input, seq)) {
            continue;
        }
        if let Some(i) = evaluated.iter().position(|x| no_elements(&x.input, seq)) {
            keep.insert(i);
            continue;
        }
        let tries: Vec<BTreeMap<String, Val>> = evaluated
            .iter()
            .take(END_TRIES)
            .map(|v| {
                let mut a = v.input.clone();
                a.insert(seq.clone(), Val::Seq(Vec::new()));
                a
            })
            .collect();
        let why = empty_why(seq);
        match tries.iter().find_map(|a| answer(f, c, a, &why)) {
            Some(vec) => {
                seen_in.insert(key_of(&vec.input), Some(evaluated.len()));
                keep.insert(evaluated.len());
                evaluated.push(vec);
            }
            None => no_answer.extend(tries.into_iter().next().map(|input| Vector { input, outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: unanswered(&why) })),
        }
    }

    // Where no case is answered — the range of a `define` leaves out every input found, as one
    // that no value on the grid of what it is worked out from comes into does — the suite is not
    // left with nothing: for each `define` that refuses a case of the pool, the first such case
    // goes to the refused cases, and the generated code is held to refusing it for that value.
    if keep.is_empty() {
        let mut named: BTreeSet<String> = BTreeSet::new();
        for mut v in outside {
            let Some(name) = eval::define_outside(f, c, v.input.clone().into_iter().collect()) else { continue };
            if named.insert(name.clone()) {
                v.why = tr!("定義 {name} が受け付けない入力（答えのある入力が見つからない）", "an input the definition {name} refuses, where no case found is answered");
                no_answer.push(v);
            }
        }
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
        on_ends: no_answer,
    }
}

/// One refused case, as a line of `vectors/<alias>.refused.jsonl`: the inputs in the same
/// wire form the vectors file uses, what the reference evaluator refuses them for and the
/// sentence it says, and why the case is in the suite. The generated runners read `in` and
/// nothing else, so the same line drives them (§15.56); `rulec test` holds what they say to
/// `refused` and `error` (§15.204).
pub fn refused_json(f: &RuleFile, c: &Checked, v: &Vector) -> String {
    let inp = in_object(f, c, v);
    // The door says every refusal of a rule that passes check, the rule's own contradiction
    // with its two elements among them (§15.324); what is left is said as the rule's first fold
    // contradicting itself on its first two elements.
    let r = crate::json::parse(&inp).ok().and_then(|j| crate::door::refusal(f, c, &j)).unwrap_or_else(|| {
        let (verdict, over) = f.folds().next().map(|d| (d.verdict.as_str(), d.over.as_str())).unwrap_or_default();
        crate::door::Refusal { kind: "contradiction", error: crate::door::fold_contradiction(verdict, over, 0, 1) }
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

/// Each end of the ranges the rule accepts for its `define`s (§15.313), on the end (`inside`) or
/// one step past it, as a whole input: a vector of the suite with what the value is worked out from
/// moved there — for a value of one element, its first element, the other elements as they were.
/// The vectors are tried in order, a vector with no element taking the first element of the first
/// vector that has one.
pub(crate) fn edge_inputs(f: &RuleFile, c: &Checked, vectors: &[Vector], inside: bool) -> Vec<(Edge, Vector)> {
    if c.accepts.is_empty() || vectors.is_empty() {
        return Vec::new();
    }
    let firsts: Vec<Option<&BTreeMap<String, Val>>> = f
        .elements
        .iter()
        .map(|el| {
            vectors.iter().find_map(|v| match v.input.get(&el.name.text) {
                Some(Val::Seq(xs)) => xs.first(),
                _ => None,
            })
        })
        .collect();
    // A seed per vector: its inputs, and the fields of the first element of each sequence.
    let mut seeds: Vec<BTreeMap<String, Val>> = Vec::new();
    let mut bases: Vec<Vector> = Vec::new();
    for v in vectors.iter().take(EDGE_SEEDS) {
        let mut seed: BTreeMap<String, Val> = v.input.iter().filter(|(k, _)| f.inputs.iter().any(|i| i.name.text == **k)).map(|(k, x)| (k.clone(), x.clone())).collect();
        let mut base = v.clone();
        for (el, first) in f.elements.iter().zip(&firsts) {
            let Some(first) = first else { continue };
            let one = match base.input.get_mut(&el.name.text) {
                Some(Val::Seq(xs)) if !xs.is_empty() => xs[0].clone(),
                Some(Val::Seq(xs)) => {
                    xs.push((*first).clone());
                    (*first).clone()
                }
                _ => continue,
            };
            for (k, x) in one {
                seed.entry(k).or_insert(x);
            }
        }
        seeds.push(seed);
        bases.push(base);
    }
    let mut out = Vec::new();
    for e in accepted_edges(f, c, &seeds) {
        if e.inside != inside {
            continue;
        }
        let base = &bases[e.from];
        let mut input = base.input.clone();
        for i in &f.inputs {
            if let Some(v) = e.values.get(&i.name.text) {
                input.insert(i.name.text.clone(), v.clone());
            }
        }
        if let Some(s) = e.seq {
            let seq = &f.elements[s].name.text;
            if let Some(Val::Seq(xs)) = input.get_mut(seq) {
                for fd in &f.elements[s].fields {
                    if let (Some(v), Some(x)) = (e.values.get(&fd.name.text), xs.first_mut()) {
                        x.insert(fd.name.text.clone(), v.clone());
                    }
                }
            }
        }
        let v = Vector { input, outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() };
        out.push((e, v));
    }
    out
}

/// How many cases of the suite an end of a range is tried on, the value moved there, before the
/// call on the end is taken to have no answer.
const END_TRIES: usize = 64;

/// How many cases of the suite a sequence of as many elements as a count lets it have is tried in.
const END_BASES: usize = 4;

/// One end the `range` of an input or of a field of an element writes (§15.314).
struct WrittenEnd {
    name: String,
    /// The sequence, for a field of an element.
    seq: Option<String>,
    top: bool,
    /// Written with `>` or `<`: the end is read one step in from what is written.
    strict: bool,
    /// The end as it is read: the value written with `>=` or `<=`, the first value inside one
    /// written with `>` or `<`, and for an ordered enum the first or the last value its range
    /// lets in.
    value: Val,
}

impl WrittenEnd {
    /// Whether a case has the value on the end: the input, or the field in any element.
    fn on(&self, a: &BTreeMap<String, Val>) -> bool {
        let same = |v: Option<&Val>| v.is_some_and(|v| show(v) == show(&self.value));
        match &self.seq {
            None => same(a.get(&self.name)),
            Some(seq) => matches!(a.get(seq), Some(Val::Seq(xs)) if xs.iter().any(|x| same(x.get(&self.name)))),
        }
    }

    /// Why the case is in the suite.
    fn why(&self) -> String {
        let name = &self.name;
        match (&self.seq, self.strict, self.top) {
            (None, true, false) => tr!("{name} を、`>` と書いた端の一つ内側に", "{name} one step in from the end its range writes with `>`"),
            (None, true, true) => tr!("{name} を、`<` と書いた端の一つ内側に", "{name} one step in from the end its range writes with `<`"),
            (Some(seq), true, false) => tr!("{seq} の要素の {name} を、`>` と書いた端の一つ内側に", "{name} one step in from the end its range writes with `>`, in an element of {seq}"),
            (Some(seq), true, true) => tr!("{seq} の要素の {name} を、`<` と書いた端の一つ内側に", "{name} one step in from the end its range writes with `<`, in an element of {seq}"),
            (None, false, false) => tr!("{name} を範囲の下端に", "{name} at the bottom of its range"),
            (None, false, true) => tr!("{name} を範囲の上端に", "{name} at the top of its range"),
            (Some(seq), false, false) => tr!("{seq} の要素の {name} を範囲の下端に", "{name} at the bottom of its range, in an element of {seq}"),
            (Some(seq), false, true) => tr!("{seq} の要素の {name} を範囲の上端に", "{name} at the top of its range, in an element of {seq}"),
        }
    }

    /// The cases the end is tried on, in order: each of the first cases of the suite with the
    /// value moved there — for a field of an element, in the first element of a case that has
    /// one, then in that element alone — and then the first of them with every other input moved
    /// to the top of its range, and to the bottom, since a `constraint` or the range a `define`
    /// accepts may leave the end an answer only beside the ends of the others.
    fn tries(&self, f: &RuleFile, c: &Checked, vectors: &[Vector]) -> Vec<BTreeMap<String, Val>> {
        let mut out: Vec<BTreeMap<String, Val>> = Vec::new();
        match &self.seq {
            None => {
                for v in vectors.iter().take(END_TRIES) {
                    let mut a = v.input.clone();
                    a.insert(self.name.clone(), self.value.clone());
                    out.push(a);
                }
            }
            Some(seq) => {
                for v in vectors.iter().filter(|v| matches!(v.input.get(seq), Some(Val::Seq(xs)) if !xs.is_empty())).take(END_TRIES) {
                    let mut a = v.input.clone();
                    let Some(Val::Seq(xs)) = a.get_mut(seq) else { continue };
                    xs[0].insert(self.name.clone(), self.value.clone());
                    let alone = vec![xs[0].clone()];
                    out.push(a.clone());
                    a.insert(seq.clone(), Val::Seq(alone));
                    out.push(a);
                }
            }
        }
        if let Some(first) = out.first().cloned() {
            for top in [true, false] {
                let mut a = first.clone();
                for i in f.inputs.iter().filter(|i| i.name.text != self.name) {
                    let name = &i.name.text;
                    let (Some(ty), Some(&(Some(lo), Some(hi)))) = (c.ty_of(name), c.ranges.get(name)) else { continue };
                    if matches!(ty.present(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) && matches!(a.get(name), Some(Val::Num(_) | Val::Date(..))) {
                        a.insert(name.clone(), to_val(if top { hi } else { lo }, ty.present()));
                    }
                }
                out.push(a);
            }
        }
        out
    }
}

/// Why a call on an end is among the refused cases: it stands where a case with an answer would,
/// and none of the cases tried there has one.
fn unanswered(why: &str) -> String {
    tr!("{why}（試した組み合わせのどれにも答えが無い）", "{why}, where no case tried is answered")
}

/// The ends the `range` of each input and of each field of an element writes (§15.314), the
/// bottom before the top.
fn written_ends(f: &RuleFile, c: &Checked) -> Vec<WrittenEnd> {
    let mut out = Vec::new();
    let vars = f.inputs.iter().map(|i| (i, None)).chain(f.elements.iter().flat_map(|el| el.fields.iter().map(move |fd| (fd, Some(el.name.text.clone())))));
    for (v, seq) in vars {
        let name = &v.name.text;
        let (Some(r), Some(ty)) = (&v.range, c.ty_of(name)) else { continue };
        let ty = ty.present().clone();
        let written = |ops: [CmpOp; 2]| r.bounds.iter().map(|(o, _)| *o).find(|o| ops.contains(o));
        let sides = [([CmpOp::Ge, CmpOp::Gt], false), ([CmpOp::Le, CmpOp::Lt], true)];
        let ends: [Option<Val>; 2] = match (&ty, c.enum_ranges.get(name), c.ranges.get(name)) {
            (Ty::Enum(_), Some(vs), _) => [vs.first().map(|v| Val::Enum(v.clone())), vs.last().map(|v| Val::Enum(v.clone()))],
            (_, _, Some(&(lo, hi))) => [lo.map(|e| to_val(e, &ty)), hi.map(|e| to_val(e, &ty))],
            _ => continue,
        };
        for ((ops, top), value) in sides.into_iter().zip(ends) {
            if let (Some(op), Some(value)) = (written(ops), value) {
                out.push(WrittenEnd { name: name.clone(), seq: seq.clone(), top, strict: matches!(op, CmpOp::Gt | CmpOp::Lt), value });
            }
        }
    }
    out
}

/// Each end the suite is held to stand on (§15.314) — the ends the ranges of the inputs and of the
/// fields of the elements write, as many elements as a count lets a sequence have, and a sequence
/// with no elements — by the reason a case made for it carries, with whether an answered case
/// stands on the end and whether a refused one does. Every end has one or the other, which the
/// tests hold the corpus to.
pub fn ends_stood_on(f: &RuleFile, c: &Checked, suite: &Suite) -> Vec<(String, bool, bool)> {
    let refused: Vec<Json> = refused_lines(f, c, suite).iter().filter_map(|l| crate::json::parse(l).ok()).filter_map(|j| j.get("in").cloned()).collect();
    let mut out = Vec::new();
    for end in written_ends(f, c) {
        let wire = crate::json::parse(&wire_value(c, &end.name, &end.value)).ok();
        let on = |a: &Json| match &end.seq {
            None => a.get(&end.name) == wire.as_ref(),
            Some(seq) => a.get(seq).and_then(|s| s.as_arr()).is_some_and(|xs| xs.iter().any(|x| x.get(&end.name) == wire.as_ref())),
        };
        out.push((end.why(), suite.vectors.iter().any(|v| end.on(&v.input)), wire.is_some() && refused.iter().any(on)));
    }
    for el in &f.elements {
        let seq = &el.name.text;
        let Some(cap) = crate::door::count_cap(f, c, seq).filter(|cap| (1..=EDGE_ELEMENTS).contains(cap)) else { continue };
        let answered = suite.vectors.iter().any(|v| matches!(v.input.get(seq), Some(Val::Seq(xs)) if xs.len() as i128 == cap));
        let turned = refused.iter().any(|a| a.get(seq).and_then(|s| s.as_arr()).is_some_and(|xs| xs.len() as i128 == cap));
        out.push((full_why(seq), answered, turned));
    }
    for el in &f.elements {
        let seq = &el.name.text;
        let answered = suite.vectors.iter().any(|v| no_elements(&v.input, seq));
        let turned = refused.iter().any(|a| a.get(seq).and_then(|s| s.as_arr()).is_some_and(|xs| xs.is_empty()));
        out.push((empty_why(seq), answered, turned));
    }
    out
}

/// Why a sequence of as many elements as a count lets it have is in the suite.
fn full_why(seq: &str) -> String {
    tr!("{seq} の要素を、数え上げの上限ちょうどに", "{seq} with as many elements as a count over it can be")
}

/// Why a sequence with no elements is in the suite.
fn empty_why(seq: &str) -> String {
    tr!("{seq} の要素ゼロ件", "no elements of {seq}")
}

/// Whether a case gives the sequence `seq` with no elements.
fn no_elements(a: &BTreeMap<String, Val>, seq: &str) -> bool {
    matches!(a.get(seq), Some(Val::Seq(xs)) if xs.is_empty())
}

/// One end of the range a `sum` or a `count` is held to once its walk is over (§15.100, §15.314).
pub(crate) struct SummaryEdge {
    /// The summary, and the sequence it walks.
    pub name: String,
    pub seq: String,
    /// A `count`, and not a `sum`.
    pub count: bool,
    /// The top of the range, and not the bottom.
    pub hi: bool,
    /// On the end, where the call is answered; otherwise past it, where it is refused.
    pub inside: bool,
}

impl SummaryEdge {
    /// Why the case is in the suite.
    pub(crate) fn why(&self) -> String {
        let (seq, name) = (&self.seq, &self.name);
        match (self.count, self.hi, self.inside) {
            (false, true, true) => tr!("{seq} の合計 {name} を範囲の上端に", "the sum {name} of {seq} at the top of its range"),
            (false, true, false) => tr!("{seq} の合計 {name} を範囲の上端の先に", "the sum {name} of {seq} past the top of its range"),
            (false, false, true) => tr!("{seq} の合計 {name} を範囲の下端に", "the sum {name} of {seq} at the bottom of its range"),
            (false, false, false) => tr!("{seq} の合計 {name} を範囲の下端の先に", "the sum {name} of {seq} past the bottom of its range"),
            (true, true, true) => tr!("{seq} を数えた {name} を範囲の上端に", "the count {name} of {seq} at the top of its range"),
            (true, true, false) => tr!("{seq} を数えた {name} を範囲の上端の先に", "the count {name} of {seq} past the top of its range"),
            (true, false, true) => tr!("{seq} を数えた {name} を範囲の下端に", "the count {name} of {seq} at the bottom of its range"),
            (true, false, false) => tr!("{seq} を数えた {name} を範囲の下端の先に", "the count {name} of {seq} past the bottom of its range"),
        }
    }
}

/// How many elements a sequence built to bring a summary to an end of its range has at most: past
/// these, the end is left without a case.
const EDGE_ELEMENTS: i128 = 50;

/// How many of the elements the vectors have such a sequence is made from.
const EDGE_TEMPLATES: usize = 6;

/// For each end a walk holds a `sum` or a `count` to once it is over — the top of a `sum`'s range,
/// and the bottom of either where a walk can come in under it (§15.100, §15.314) — a whole input
/// that brings the summary onto the end, and one that takes it one step past: a vector of the suite
/// with the sequence replaced. A sum over a field takes as few elements as carry the total, each
/// within the field's range; any other summary takes copies of one element the vectors have, or one
/// or two of those elements as they are, and where nothing lands one step past the top, the fewest
/// copies that pass it. A field marked
/// `unique` takes a value of its own in each element (§15.290). Each case is confirmed by the
/// evaluator: on the end the call is answered with the summary there, and past it this summary is
/// what the walk refuses. An end no such sequence reaches has no case.
pub(crate) fn summary_edges(f: &RuleFile, c: &Checked, vectors: &[Vector]) -> Vec<(SummaryEdge, BTreeMap<String, Val>)> {
    use crate::ast::AggKind;
    use std::cmp::Ordering::*;
    let mut out = Vec::new();
    let Some(any) = vectors.first() else { return out };
    for el in &f.elements {
        let seq = &el.name.text;
        let base = vectors.iter().find(|v| matches!(v.input.get(seq), Some(Val::Seq(xs)) if !xs.is_empty())).unwrap_or(any);
        let known = known_elements(vectors, seq);
        let cap = crate::door::count_cap(f, c, seq);
        let fits = |n: i128| n <= EDGE_ELEMENTS && cap.is_none_or(|cap| n <= cap);
        let with = |xs: Vec<BTreeMap<String, Val>>| -> BTreeMap<String, Val> {
            let mut a = base.input.clone();
            a.insert(seq.clone(), Val::Seq(xs));
            a
        };
        for d in f.aggs().filter(|d| d.over == *seq && !d.kind.extreme()) {
            let name = &d.name.text;
            let Some(&(_, hi)) = c.ranges.get(name) else { continue };
            let count = d.kind == AggKind::Count;
            let step = Rat::new(1, c.scales.get(name).copied().unwrap_or(1).max(1));
            let value = |xs: &[BTreeMap<String, Val>]| eval::summary_value(f, c, with(xs.to_vec()).into_iter().collect(), name);
            // What one element alone adds, for each element the vectors have that adds something.
            let adds: Vec<(Rat, &BTreeMap<String, Val>)> = known.iter().filter_map(|e| value(std::slice::from_ref(e)).filter(|m| m.num > 0).map(|m| (m, e))).collect();
            let reach = |t: Rat| reaching(f, c, el, d, &known, &adds, t);
            let floor = c.walk_floor(d);
            let ends = [(if count { None } else { hi }, true), (floor, false)];
            for (end, top) in ends {
                let Some(end) = end else { continue };
                // On the end: answered, the summary there.
                let on = reach(end).into_iter().map(&with).find(|a| {
                    eval::summary_outside(f, c, a.clone().into_iter().collect()).is_none()
                        && eval::summary_value(f, c, a.clone().into_iter().collect(), name).is_some_and(|v| v.cmp_to(end) == Equal)
                });
                if let Some(a) = on {
                    out.push((SummaryEdge { name: name.clone(), seq: seq.clone(), count, hi: top, inside: true }, a));
                }
                // One step past it; for the top, failing that, the fewest copies of the element
                // that adds most which pass it; for the bottom, no element at all.
                let mut tries = reach(if top { end.add(step) } else { end.sub(step) });
                if top {
                    if let Some((m, e)) = adds.iter().max_by(|a, b| a.0.cmp_to(b.0)) {
                        let q = end.div(*m);
                        let n = q.num.div_euclid(q.den) + 1;
                        if fits(n) {
                            tries.extend(apart(c, el, e, n as usize));
                        }
                    }
                } else {
                    tries.push(Vec::new());
                }
                let past = tries.into_iter().map(&with).find(|a| {
                    eval::summary_outside(f, c, a.clone().into_iter().collect()).is_some_and(|p| p.name == *name && p.below != top)
                });
                if let Some(a) = past {
                    out.push((SummaryEdge { name: name.clone(), seq: seq.clone(), count, hi: top, inside: false }, a));
                }
            }
        }
    }
    out
}

/// The elements the vectors have of the sequence `seq`, each once, the first 32 of them.
fn known_elements(vectors: &[Vector], seq: &str) -> Vec<BTreeMap<String, Val>> {
    let mut known: Vec<BTreeMap<String, Val>> = Vec::new();
    for v in vectors {
        let Some(Val::Seq(xs)) = v.input.get(seq) else { continue };
        for e in xs {
            if known.len() < 32 && !known.contains(e) {
                known.push(e.clone());
            }
        }
    }
    known
}

/// The sequences of `el` that may bring the `sum` or the `count` `d` to `t`, the likeliest first,
/// made from the elements `known`; `adds` is what one of them alone adds to it, for each that adds
/// something.
fn reaching(
    f: &RuleFile,
    c: &Checked,
    el: &ElementsDecl,
    d: &crate::ast::AggDecl,
    known: &[BTreeMap<String, Val>],
    adds: &[(Rat, &BTreeMap<String, Val>)],
    t: Rat,
) -> Vec<Vec<BTreeMap<String, Val>>> {
    use std::cmp::Ordering::*;
    let count = d.kind == crate::ast::AggKind::Count;
    let cap = crate::door::count_cap(f, c, &el.name.text);
    let fits = |n: i128| n <= EDGE_ELEMENTS && cap.is_none_or(|cap| n <= cap);
    let mut tries: Vec<Vec<BTreeMap<String, Val>>> = Vec::new();
    if t.num < 0 {
        return tries;
    }
    if t.num == 0 {
        tries.push(Vec::new());
    }
    // A sum over a field: the fewest elements that carry the total, each from the bottom of the
    // field's range up, the first ones to its top; made from each of the first elements the vectors
    // have, since what else an element holds decides what the other summaries of the walk come to.
    let field = el.fields.iter().find(|fd| fd.name.text == d.column.text && fd.unique.is_none());
    if let (false, Some(_), Some(&(Some(flo), Some(fhi)))) = (count, field, c.ranges.get(&d.column.text)) {
        let q = if fhi.num > 0 { t.div(fhi) } else { Rat::zero() };
        let n = (q.num + q.den - 1).div_euclid(q.den).max(1);
        if fhi.num > 0 && fits(n) && flo.mul(Rat::int(n)).cmp_to(t) != Greater {
            for first in known.iter().take(EDGE_TEMPLATES) {
                let Some(mut xs) = apart(c, el, first, n as usize) else { continue };
                let room = fhi.sub(flo);
                let mut left = t.sub(flo.mul(Rat::int(n)));
                for x in xs.iter_mut() {
                    let add = if left.cmp_to(room) == Greater { room } else { left };
                    x.insert(d.column.text.clone(), Val::Num(flo.add(add)));
                    left = left.sub(add);
                }
                tries.push(xs);
            }
        }
    }
    // Copies of one element the vectors have.
    for (m, e) in adds {
        let q = t.div(*m);
        if q.den == 1 && q.num >= 1 && fits(q.num) {
            tries.extend(apart(c, el, e, q.num as usize));
        }
    }
    // One element the vectors have, or two of them: where another summary of the walk has a bottom
    // of its own, the sequence has to bring that one inside its range.
    for a in known.iter().take(EDGE_TEMPLATES) {
        tries.push(vec![a.clone()]);
    }
    if fits(2) {
        for a in known.iter().take(EDGE_TEMPLATES) {
            for b in known.iter().take(EDGE_TEMPLATES) {
                if a == b {
                    tries.extend(apart(c, el, a, 2));
                } else {
                    tries.push(vec![a.clone(), b.clone()]);
                }
            }
        }
    }
    tries
}

/// One end of the range the rule accepts for a `define` it works out once a walk is over
/// (§15.313, §15.314): a value read off what a walk answers — a `sum`, a `count`, a `max`, a `min`.
pub(crate) struct WalkedEdge {
    /// The `define`.
    pub define: String,
    /// The top end, or the bottom.
    pub hi: bool,
    /// On the end, where the call is answered; otherwise past it, where it is refused.
    pub inside: bool,
}

impl WalkedEdge {
    /// Why the case is in the suite: what a case on an end of a value worked out before the walks
    /// says, or one step past it.
    pub(crate) fn why(&self) -> String {
        let define = &self.define;
        match (self.hi, self.inside) {
            (true, true) => tr!("定義 {define} を、受け付ける範囲の上端に", "the definition {define} at the top of the range it accepts"),
            (false, true) => tr!("定義 {define} を、受け付ける範囲の下端に", "the definition {define} at the bottom of the range it accepts"),
            (true, false) => tr!("定義 {define} を、受け付ける範囲の上端の先に", "the definition {define} past the top of the range it accepts"),
            (false, false) => tr!("定義 {define} を、受け付ける範囲の下端の先に", "the definition {define} past the bottom of the range it accepts"),
        }
    }
}

/// The values a rule works out once a walk is over — each `define` that reads what a walk answers
/// — by name, in the order written.
fn walked_defines(f: &RuleFile) -> Vec<&crate::ast::DefineDecl> {
    let st = crate::types::stages(f);
    f.items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| match it {
            Item::Define(d) if matches!(st.at(i), crate::types::Stage::Once(k) if k > 0) => Some(d),
            _ => None,
        })
        .collect()
}

/// The inputs of a whole input, and what each summary of the rule comes to on it, side by side:
/// what `run_tables` works a value out from once the walks are over, without walking.
fn flat_of(f: &RuleFile, c: &Checked, a: &BTreeMap<String, Val>) -> BTreeMap<String, Val> {
    let mut s: BTreeMap<String, Val> = a.iter().filter(|(k, _)| f.inputs.iter().any(|i| i.name.text == **k)).map(|(k, x)| (k.clone(), x.clone())).collect();
    for d in f.aggs() {
        if let Some(v) = eval::summary_value(f, c, a.clone().into_iter().collect(), &d.name.text) {
            s.insert(d.name.text.clone(), Val::Num(v));
        }
    }
    s
}

/// What a value worked out once a walk is over is solved over: the inputs of the call, and what
/// the summaries of the walks come to — a `sum`, a `count`, a `max`, a `min`.
fn walked_movable(f: &RuleFile) -> Vec<String> {
    let mut movable: Vec<String> = f.inputs.iter().map(|x| x.name.text.clone()).collect();
    movable.extend(f.aggs().map(|d| d.name.text.clone()));
    movable
}

/// For each end of the range the rule accepts for a `define` it works out once a walk is over — a
/// value read off what a walk answers, which `accepted_edges` does not look at — a whole input
/// that brings the value onto the end, and one that takes it one step past (§15.313, §15.314). The
/// value is solved as `accepted_edges` solves one, over the inputs of the call and what the
/// summaries of the walks come to, from each of the first vectors in turn; a summary that moved is
/// then given a sequence that comes to it (`with_summaries`), the rest of the vector as it was.
/// Each case is confirmed by the evaluator on the whole input: the value is where it was solved
/// to, on the end the call is answered, and past it this value is what the door refuses the call
/// for. An end no such input reaches has no case.
pub(crate) fn walked_edges(f: &RuleFile, c: &Checked, vectors: &[Vector]) -> Vec<(WalkedEdge, BTreeMap<String, Val>)> {
    use std::cmp::Ordering::*;
    let mut out = Vec::new();
    if c.accepts.is_empty() || !f.walks() || vectors.is_empty() {
        return out;
    }
    let movable = walked_movable(f);
    for d in walked_defines(f) {
        let name = &d.name.text;
        let Some(&(lo, hi)) = c.accepts.get(name) else { continue };
        let Some(ty) = c.ty_of(name).map(|t| t.present().clone()) else { continue };
        let (rl, rh) = c.reach(&d.expr, &ty).unwrap_or((Rat::int(i64::MIN as i128), Rat::int(i64::MAX as i128)));
        let step = Rat::new(1, c.scales.get(name).copied().unwrap_or(1).max(1));
        for (end, top) in [(lo, false), (hi, true)] {
            let Some(end) = end else { continue };
            // How far what it reaches goes past the end: on it (Equal) or beyond it (Greater).
            let past = if top { rh.cmp_to(end) } else { end.cmp_to(rl) };
            for inside in [true, false] {
                if past == Less || (!inside && past != Greater) {
                    continue;
                }
                let target = match (inside, top) {
                    (true, _) => end,
                    (false, true) => end.add(step),
                    (false, false) => end.sub(step),
                };
                // A point solved from a seed, either way, where it is on the side of the end the
                // case is for, with what the value comes to there.
                let solve = |seed: &BTreeMap<String, Val>, up: bool| -> Option<(BTreeMap<String, Val>, Rat)> {
                    let a = solve_flat(f, c, seed, &movable, name, target, up)?;
                    let v = flat_value(f, c, &a, name)?;
                    let ok = if inside {
                        !eval::outside_accepted(c, name, &to_val(v, &ty))
                    } else if top {
                        v.cmp_to(end) == Greater
                    } else {
                        v.cmp_to(end) == Less
                    };
                    ok.then_some((a, v))
                };
                // The whole input the point is, where the value comes to `v` on it and the rule
                // answers it, or, past the end, refuses it for the value.
                let whole = |base: &Vector, seed: &BTreeMap<String, Val>, a: &BTreeMap<String, Val>, v: Rat| -> Option<BTreeMap<String, Val>> {
                    with_summaries(f, c, vectors, &base.input, seed, a, name).into_iter().find(|whole| {
                        if flat_value(f, c, &flat_of(f, c, whole), name).is_none_or(|w| w.cmp_to(v) != Equal) {
                            return false;
                        }
                        if inside {
                            return answer(f, c, whole, "").is_some();
                        }
                        let case = Vector { input: whole.clone(), outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() };
                        crate::json::parse(&in_object(f, c, &case)).ok().and_then(|obj| crate::door::refusal(f, c, &obj)).is_some_and(|r| r.error == crate::door::out_of_range(name))
                    })
                };
                // The case is one the value comes to the end on, or one step past it, from any
                // seed; only where none comes there, the one nearest it, which an end the value
                // takes no point on has (§15.323). The first point on the side of the end is not
                // the case: `points = gifts * 10 + boxes` solved to 35 for a top of 40 from one
                // seed, and to 40 from the other way.
                let mut near: Vec<(Rat, &Vector, BTreeMap<String, Val>, BTreeMap<String, Val>, Rat)> = Vec::new();
                let mut found = None;
                'seeds: for base in vectors.iter().take(EDGE_SEEDS) {
                    let seed = flat_of(f, c, &base.input);
                    for up in [!top, top] {
                        let Some((a, v)) = solve(&seed, up) else { continue };
                        if v.cmp_to(target) != Equal {
                            near.push((dist(v, target), base, seed.clone(), a, v));
                            continue;
                        }
                        if let Some(w) = whole(base, &seed, &a, v) {
                            found = Some(w);
                            break 'seeds;
                        }
                    }
                }
                if found.is_none() {
                    // stable: among the points as near, the seeds' order
                    near.sort_by(|x, y| x.0.cmp_to(y.0));
                    found = near.into_iter().find_map(|(_, base, seed, a, v)| whole(base, &seed, &a, v));
                }
                if let Some(a) = found {
                    out.push((WalkedEdge { define: name.clone(), hi: top, inside }, a));
                }
            }
        }
    }
    out
}

/// The tries of one sequence for its summaries: the sequence, its elements, the elements the
/// vectors have, and how many elements it may have.
struct Walk<'a> {
    f: &'a RuleFile,
    c: &'a Checked,
    el: &'a ElementsDecl,
    known: Vec<BTreeMap<String, Val>>,
    room: i128,
}

impl Walk<'_> {
    fn seq(&self) -> &String {
        &self.el.name.text
    }

    fn at(&self, input: &BTreeMap<String, Val>, d: &AggDecl) -> Option<Rat> {
        eval::summary_value(self.f, self.c, input.clone().into_iter().collect(), &d.name.text)
    }

    fn hits(&self, b: &BTreeMap<String, Val>, ds: &[(&AggDecl, Rat)]) -> bool {
        ds.iter().all(|(d, t)| self.at(b, d).is_some_and(|v| v.cmp_to(*t) == std::cmp::Ordering::Equal))
    }

    /// The tries for the summary `d` to come to `t`, from the whole input `input`. Without `wide`,
    /// the sequence as it is with the value of an element moved, the sequence with elements put
    /// after it, a sequence `reaching` makes, and the sequence with one element put after it that
    /// the summary looks at, its value moved to where the summary has to be — two moves, for a
    /// summary none of whose elements is there yet: the least price of the wrapped lines, where no
    /// line is wrapped. With `wide`, the one that moves what an element is looked at by (§15.323):
    /// an element of the sequence taken into what a `max` or a `min` looks at, at where the summary
    /// has to be — a line made a wrapped one, at the price the dearest wrapped line has to have.
    fn tries(&self, input: &BTreeMap<String, Val>, d: &AggDecl, t: Rat, wide: bool) -> Vec<Vec<BTreeMap<String, Val>>> {
        use std::cmp::Ordering::*;
        let (f, c, el, seq, known, room) = (self.f, self.c, self.el, self.seq(), &self.known, self.room);
        let with = |xs: Vec<BTreeMap<String, Val>>| -> BTreeMap<String, Val> {
            let mut b = input.clone();
            b.insert(seq.clone(), Val::Seq(xs));
            b
        };
        let mut tries: Vec<Vec<BTreeMap<String, Val>>> = Vec::new();
        let cur: &[BTreeMap<String, Val>] = match input.get(seq) {
            Some(Val::Seq(xs)) => xs,
            _ => &[],
        };
        let Some(now) = self.at(input, d) else { return tries };
        let col = &d.column.text;
        let range = c.ranges.get(col).copied();
        let fits = |v: Rat| range.is_some_and(|(flo, fhi)| !flo.is_some_and(|lo| v.cmp_to(lo) == Less) && !fhi.is_some_and(|hi| v.cmp_to(hi) == Greater));
        let plain = el.fields.iter().any(|fd| fd.name.text == *col && fd.unique.is_none());
        if wide {
            // One element the summary does not look at, taken in at `t`.
            if let (AggKind::Max | AggKind::Min, Some(view)) = (&d.kind, view_of(c, el, d))
                && plain
                && fits(t)
            {
                for (k, e) in cur.iter().enumerate().take(EDGE_TEMPLATES) {
                    if in_view(&view, e) {
                        continue;
                    }
                    let Some(mut x) = into_view(c, el, d, e) else { continue };
                    x.insert(col.clone(), Val::Num(t));
                    let mut xs = cur.to_vec();
                    xs[k] = x;
                    tries.push(xs);
                }
            }
            return tries;
        }
        // The sequence as it is with one element's value moved: what a `sum` reads, by what the
        // total is away from there; what a `max` or a `min` reads, to where it has to be. The
        // rest of the sequence stays, and with it what the other summaries of the walk come to.
        // These come first, and the ones that put elements after it.
        if range.is_some() {
            for (k, e) in cur.iter().enumerate().take(EDGE_TEMPLATES) {
                let Some(old) = e.get(col).and_then(as_rat) else { continue };
                let v = match d.kind {
                    AggKind::Sum => old.add(t.sub(now)),
                    AggKind::Max | AggKind::Min => t,
                    _ => continue,
                };
                if !fits(v) {
                    continue;
                }
                let mut xs = cur.to_vec();
                xs[k].insert(col.clone(), Val::Num(v));
                tries.push(xs);
            }
        }
        let adds: Vec<(Rat, &BTreeMap<String, Val>)> = known.iter().filter_map(|e| self.at(&with(vec![e.clone()]), d).filter(|m| m.num > 0).map(|m| (m, e))).collect();
        // The sequence as it is, and after it as many of one element the vectors have as bring
        // the summary up to there.
        if t.cmp_to(now) == Greater {
            for (m, e) in &adds {
                let q = t.sub(now).div(*m);
                let n = cur.len() as i128 + q.num;
                if q.den != 1 || q.num < 1 || n > room {
                    continue;
                }
                if let Some(more) = apart(c, el, e, q.num as usize) {
                    tries.push(cur.iter().cloned().chain(more).collect());
                }
            }
        }
        // A sequence made to bring it there, of the elements the vectors have.
        tries.extend(reaching(f, c, el, d, known, &adds, t));
        // An element the vectors have put after the sequence, made one the summary looks at,
        // and its value moved: the column a `max` or a `min` reads at where the summary has to
        // be, the one a `sum` adds up at what the total still needs; for a `count`, as many of
        // it as it still needs.
        if (cur.len() as i128) < room {
            for e in known.iter().take(EDGE_TEMPLATES) {
                let Some(mut x) = into_view(c, el, d, e) else { continue };
                let v = match d.kind {
                    AggKind::Max | AggKind::Min => Some(t),
                    AggKind::Sum => Some(t.sub(now)),
                    _ => None,
                };
                match v {
                    Some(v) => {
                        if !plain || !fits(v) {
                            continue;
                        }
                        x.insert(col.clone(), Val::Num(v));
                        tries.push(cur.iter().cloned().chain([x]).collect());
                    }
                    None => {
                        let q = t.sub(now);
                        if q.den != 1 || q.num < 1 || cur.len() as i128 + q.num > room {
                            continue;
                        }
                        if let Some(more) = apart(c, el, &x, q.num as usize) {
                            tries.push(cur.iter().cloned().chain(more).collect());
                        }
                    }
                }
            }
        }
        tries
    }

    /// One step: from each of `stage`, the tries that bring `d` to `t` with every summary of
    /// `held` (`d` at `t` among them) where it has it; one that has them there already, as it is.
    /// The tries that do not move what an element is looked at by come first, over every input of
    /// `stage`, and then the ones that do. The first `WHOLES` of them.
    fn step(&self, stage: &[BTreeMap<String, Val>], d: &AggDecl, t: Rat, held: &[(&AggDecl, Rat)]) -> Vec<BTreeMap<String, Val>> {
        let mut after: Vec<BTreeMap<String, Val>> = Vec::new();
        for wide in [false, true] {
            for input in stage {
                if after.len() >= WHOLES {
                    return after;
                }
                // Brought there already, by the moves before it.
                if self.hits(input, held) {
                    if !after.contains(input) {
                        after.push(input.clone());
                    }
                    continue;
                }
                for xs in self.tries(input, d, t, wide) {
                    let mut b = input.clone();
                    b.insert(self.seq().clone(), Val::Seq(xs));
                    if !after.contains(&b) && self.hits(&b, held) {
                        after.push(b);
                        if after.len() >= WHOLES {
                            return after;
                        }
                    }
                }
            }
        }
        after
    }
}

/// The whole inputs a solved point may be: `base` with the inputs as `a` gives them, and each
/// sequence over which `a` moved a summary from what `seed` has — what it comes to on `base` —
/// replaced by one that brings every summary that moved to where the point has it. The summaries are
/// brought there one after another (§15.323): the tries for each (`Walk::tries`) are made from the
/// sequences that brought the ones before it, and a try is kept where those are still where they
/// were brought. In the order written, and for more than one, the other way round as well: a
/// wrapped line put in for the least wrapped price adds to the total, so the total is brought
/// there after it. A sequence no summary of which moved stays as it is. At each step, the first
/// `WHOLES` of them, those that keep the rest of the sequence first; none where no such sequence is
/// found.
///
/// A try for one summary can take another along that the point did not move, and then the value
/// `name` is not where the point has it. Where none of the sequences found has it there, the
/// summaries a sequence has away from the point are brought back, one after another, each kept where
/// it is brought and the ones the point has kept where they are, and those come after: a line put
/// in for the total that is wrapped moves the dearest wrapped price, and letting it go unwrapped
/// brings that back. Before, one try had to bring every summary that moved at once, and a point
/// that moved two of them — the largest price and the least of the wrapped lines — had no sequence.
fn with_summaries(f: &RuleFile, c: &Checked, vectors: &[Vector], base: &BTreeMap<String, Val>, seed: &BTreeMap<String, Val>, a: &BTreeMap<String, Val>, name: &str) -> Vec<BTreeMap<String, Val>> {
    use std::cmp::Ordering::*;
    let mut input = base.clone();
    for i in &f.inputs {
        if let Some(v) = a.get(&i.name.text) {
            input.insert(i.name.text.clone(), v.clone());
        }
    }
    let mut wholes = vec![input];
    let mut walks: Vec<Walk> = Vec::new();
    for el in &f.elements {
        let seq = &el.name.text;
        // The summaries of the sequence that the point moved, with where it has them.
        let moved: Vec<(&AggDecl, Rat)> = f
            .aggs()
            .filter(|d| d.over == *seq)
            .filter_map(|d| {
                let t = a.get(&d.name.text).and_then(as_rat)?;
                (seed.get(&d.name.text).and_then(as_rat).is_none_or(|s| s.cmp_to(t) != Equal)).then_some((d, t))
            })
            .collect();
        if moved.is_empty() {
            continue;
        }
        let w = Walk { f, c, el, known: known_elements(vectors, seq), room: crate::door::count_cap(f, c, seq).unwrap_or(EDGE_ELEMENTS).min(EDGE_ELEMENTS) };
        let n = moved.len();
        let orders: Vec<Vec<usize>> = if n > 1 { vec![(0..n).collect(), (0..n).rev().collect()] } else { vec![vec![0]] };
        let mut next: Vec<BTreeMap<String, Val>> = Vec::new();
        for order in &orders {
            if next.len() >= WHOLES {
                break;
            }
            let mut stage: Vec<BTreeMap<String, Val>> = wholes.iter().take(WHOLES).cloned().collect();
            for (j, &k) in order.iter().enumerate() {
                let (d, t) = moved[k];
                let held: Vec<(&AggDecl, Rat)> = order[..=j].iter().map(|&i| moved[i]).collect();
                stage = w.step(&stage, d, t, &held);
                if stage.is_empty() {
                    break;
                }
            }
            for b in stage {
                if next.len() < WHOLES && !next.contains(&b) && w.hits(&b, &moved) {
                    next.push(b);
                }
            }
        }
        wholes = next;
        walks.push(w);
    }
    let Some(want) = flat_value(f, c, a, name) else { return wholes };
    let right = |b: &BTreeMap<String, Val>| flat_value(f, c, &flat_of(f, c, b), name).is_some_and(|v| v.cmp_to(want) == Equal);
    if walks.is_empty() || wholes.iter().any(right) {
        return wholes;
    }
    // The summaries the value reads, by whether moving one a step, or to an end of its range,
    // moves the value: one it does not read is left wherever a try takes it (the dearest price of
    // all the lines, beside a value of the total and the dearest wrapped one).
    let reads = |d: &AggDecl| -> bool {
        let n = &d.name.text;
        let (Some(x0), Some(ty)) = (a.get(n).and_then(as_rat), c.ty_of(n).map(|t| t.present().clone())) else { return false };
        let q = crate::coverage::quantum(c, n, &ty);
        let (lo, hi) = c.ranges.get(n).copied().unwrap_or((None, None));
        [Some(x0.add(q)), Some(x0.sub(q)), lo, hi].into_iter().flatten().any(|v| {
            let mut b = a.clone();
            b.insert(n.clone(), to_val(v, &ty));
            flat_value(f, c, &b, name).is_some_and(|y| y.cmp_to(want) != Equal)
        })
    };
    let mut back: Vec<BTreeMap<String, Val>> = Vec::new();
    for whole in &wholes {
        let mut stage = vec![whole.clone()];
        for w in &walks {
            let all: Vec<(&AggDecl, Rat)> = f.aggs().filter(|d| d.over == *w.seq() && reads(d)).filter_map(|d| Some((d, a.get(&d.name.text).and_then(as_rat)?))).collect();
            let mut held: Vec<(&AggDecl, Rat)> = all.iter().copied().filter(|&(d, t)| w.at(whole, d).is_some_and(|v| v.cmp_to(t) == Equal)).collect();
            for &(d, t) in &all {
                if held.iter().any(|(h, _)| h.name.text == d.name.text) {
                    continue;
                }
                held.push((d, t));
                stage = w.step(&stage, d, t, &held);
                if stage.is_empty() {
                    break;
                }
            }
        }
        for b in stage {
            if back.len() < WHOLES && right(&b) && !back.contains(&b) && !wholes.contains(&b) {
                back.push(b);
            }
        }
        if back.len() >= WHOLES {
            break;
        }
    }
    wholes.extend(back);
    wholes
}

/// The field the summary `d` looks at the elements by, at the value it asks for: the one a
/// `count` counts, or the one a `max`'s or a `min`'s `where` reads. `None` for a summary that looks
/// at every element, and where what it reads is not a field of the element of a truth value or an
/// enum.
fn view_of<'a>(c: &Checked, el: &ElementsDecl, d: &'a AggDecl) -> Option<(&'a str, Val)> {
    let (col, val) = match (&d.kind, &d.filter) {
        (AggKind::Count, _) => (&d.column, d.value.as_ref()),
        (AggKind::Max | AggKind::Min, Some((col, v))) => (col, v.as_ref()),
        _ => return None,
    };
    el.fields.iter().find(|fd| fd.name.text == col.text)?;
    let v = match c.ty_of(&col.text)? {
        Ty::Bool => Val::Bool(val.is_none_or(|v| v.text == "true")),
        Ty::Enum(_) => Val::Enum(val?.text.clone()),
        _ => return None,
    };
    Some((col.text.as_str(), v))
}

/// Whether the summary whose view is `view` looks at `e`.
fn in_view((col, v): &(&str, Val), e: &BTreeMap<String, Val>) -> bool {
    e.get(*col) == Some(v)
}

/// `e` made an element the summary `d` looks at: the field a `max`'s or a `min`'s `where` reads, or
/// the one a `count` counts, at the value it asks for. `e` as it is for a summary that looks at
/// every element; `None` where what it reads is not a field of the element of a truth value or an
/// enum.
fn into_view(c: &Checked, el: &ElementsDecl, d: &AggDecl, e: &BTreeMap<String, Val>) -> Option<BTreeMap<String, Val>> {
    match (&d.kind, &d.filter) {
        (AggKind::Count, _) | (AggKind::Max | AggKind::Min, Some(_)) => {
            let (col, v) = view_of(c, el, d)?;
            let mut x = e.clone();
            x.insert(col.to_string(), v);
            Some(x)
        }
        _ => Some(e.clone()),
    }
}

/// How many whole inputs a solved point of a value worked out once a walk is over is tried as.
const WHOLES: usize = 12;

/// How many of the pool's cases that a walk refuses for a summary outside its range are kept aside,
/// and from how many places, at most, they are brought inside it (`places`, `fit_summaries`). Past
/// it, what the audit is still owed is said missing, as before.
const FIT_POOL: usize = 4096;
const FIT_SEEDS: usize = 64;

/// How many elements of the candidates' cross product `fit_summaries` tries to put after a
/// sequence, beside the elements the sequence has.
const FIT_TEMPLATES: usize = 64;

/// The cases a walk refused for a summary outside its range, one for each place they stand — what
/// each summary inside its range came to, which summaries are outside, and the inputs that are not a
/// sequence: a second case standing where the first does is brought to the same place. At most
/// `take` of them, each in turn the one that brings the most the places before it did not: a summary
/// inside its range on a point of a boundary pair a table writes on it (`coverage::pairs_on`). Among
/// those that bring as much, the pool's order stands, and the places that bring nothing new come
/// after, in it. The pool's order alone puts first the places an element's candidates make, which a
/// rule with many fields has more of than `FIT_SEEDS` lets in, and the places that stand on a
/// boundary, after them. With each, the summaries it stands on such a point with, which
/// `fit_summaries` holds: none, for a place that stands on no point, holds every summary inside.
fn places<'a>(f: &RuleFile, c: &Checked, cases: &'a [(BTreeMap<String, Val>, String)], take: usize) -> Vec<(&'a BTreeMap<String, Val>, BTreeSet<String>)> {
    use std::cmp::Ordering::*;
    let summaries: BTreeSet<&str> = f.aggs().map(|d| d.name.text.as_str()).collect();
    let mut targets: BTreeSet<(String, (i128, i128))> = BTreeSet::new();
    for set in &c.sets {
        let t = &set.table;
        for row in &t.rows {
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let (true, Some(cell), Some(ty)) = (summaries.contains(col.as_str()), row.cells.get(ci), c.ty_of(col).map(|t| t.present().clone())) else { continue };
                for (_, inside, outside) in crate::coverage::pairs_on(c, col, cell, &ty) {
                    targets.insert((col.clone(), (inside.num, inside.den)));
                    targets.insert((col.clone(), (outside.num, outside.den)));
                }
            }
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut all: Vec<(&BTreeMap<String, Val>, BTreeSet<(String, (i128, i128))>)> = Vec::new();
    for (a, _) in cases {
        let got = eval::summaries_past(f, c, a.clone().into_iter().collect());
        let mut place: Vec<String> = a.iter().filter(|(_, v)| !matches!(v, Val::Seq(_))).map(|(k, v)| format!("{k}={}", show(v))).collect();
        let mut on: BTreeSet<(String, (i128, i128))> = BTreeSet::new();
        for d in f.aggs() {
            let name = &d.name.text;
            let v = got.get(name);
            let r = v.and_then(as_rat);
            let under = r.zip(c.walk_floor(d)).is_some_and(|(x, lo)| x.cmp_to(lo) == Less);
            let over = d.kind == crate::ast::AggKind::Sum && r.zip(c.ranges.get(name).and_then(|(_, hi)| *hi)).is_some_and(|(x, hi)| x.cmp_to(hi) == Greater);
            if under || over {
                place.push(format!("{name} outside"));
                continue;
            }
            place.push(format!("{name}={}", v.map(show).unwrap_or_default()));
            if let Some(x) = r.map(|x| (name.clone(), (x.num, x.den))).filter(|k| targets.contains(k)) {
                on.insert(x);
            }
        }
        if seen.insert(place.join(",")) {
            all.push((a, on));
        }
    }
    let mut hit: BTreeSet<(String, (i128, i128))> = BTreeSet::new();
    let mut used = vec![false; all.len()];
    let mut order: Vec<usize> = Vec::new();
    while order.len() < take {
        let best = (0..all.len())
            .filter(|&i| !used[i])
            .map(|i| (all[i].1.difference(&hit).count(), i))
            .filter(|(n, _)| *n > 0)
            .max_by(|x, y| x.0.cmp(&y.0).then(y.1.cmp(&x.1)));
        let Some((_, i)) = best else { break };
        used[i] = true;
        hit.extend(all[i].1.iter().cloned());
        order.push(i);
    }
    let rest: Vec<usize> = (0..all.len()).filter(|i| !used[*i]).collect();
    order.extend(rest.into_iter().take(take.saturating_sub(order.len())));
    order.into_iter().map(|i| (all[i].0, all[i].1.iter().map(|(n, _)| n.clone()).collect())).collect()
}

/// The sequences that carry the values of one call: `base` — the values, with each sequence at rest
/// — as it is where every walk ends inside the ranges of its summaries; else with each sequence
/// brought inside them by `fit_summaries`. The pool moves the values of one call with the sequences
/// held; a `count` held to two elements, or a `sum` held to 200 yen, refused every one of those
/// cases, and a table over the values of the call was left with no case at all (§15.323). An empty
/// map where neither is answered, or `base` has no sequence: the pools then keep what they had.
fn at_rest(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>, base: &BTreeMap<String, Val>) -> BTreeMap<String, Val> {
    let seqs = |m: &BTreeMap<String, Val>| -> BTreeMap<String, Val> { m.iter().filter(|(_, v)| matches!(v, Val::Seq(_))).map(|(k, v)| (k.clone(), v.clone())).collect() };
    if eval::summary_outside(f, c, base.clone().into_iter().collect()).is_none() {
        return seqs(base);
    }
    fit_summaries(f, c, base, cands, &BTreeSet::new()).map(|(m, _)| seqs(&m)).unwrap_or_default()
}

/// A case a walk refuses because a `sum` or a `count` comes outside its range (§15.314), with each
/// sequence brought back inside the ranges of its summaries while the summaries in `hold` keep what
/// they came to: the count, the total or the largest value the case was built to stand on
/// (§15.323). An empty `hold` holds every summary that was inside; the others may move, inside their
/// ranges — a count of units held at two would leave no room for the line that brings a total to its
/// bottom, though no table asks for two units. The pool aims a summary at the boundaries of the tables that read it with copies of
/// one element, and a total with as few elements as carry it; where another summary of the same
/// walk has a bottom — a total held to 200 yen, a count held to two elements — such a sequence comes
/// in under it, the door refuses it, and the boundary is left with no case. For each summary
/// outside, in turn:
/// - a total under its bottom: the field it adds up moved up in the elements already there, and
///   then elements put after the sequence, its field at what the total still needs;
/// - a total past its top: the field moved down, and then elements taken out that leave the rest
///   where it was;
/// - a count under its bottom: elements it counts put after the sequence.
///
/// An element put after the sequence has each other field a summary reads at the bottom of where it
/// can be, and what it adds to a total that was inside is taken back off the elements before it: a
/// line of one more unit after a line of nine leaves that one with eight. It is one no count that
/// was inside takes, where the candidates have one.
///
/// A field is moved inside its range, and not past what a `max` or a `min` that was inside came to:
/// an element over the largest value would take its place. A summary over a column worked out per
/// element is moved by whole elements only. The sequence stays inside the length the counts over it
/// let through, and within `EDGE_ELEMENTS`. What comes out is confirmed by the evaluator: each
/// summary held comes to what it did, none is outside, and the call is answered. With
/// it, why it is in the suite: what it stands on, and what was brought inside. `None` where nothing
/// does it: no element in reach adds what is missing without moving what the case stands on.
fn fit_summaries(
    f: &RuleFile,
    c: &Checked,
    a: &BTreeMap<String, Val>,
    cands: &BTreeMap<String, Vec<Val>>,
    hold: &BTreeSet<String>,
) -> Option<(BTreeMap<String, Val>, String)> {
    use crate::ast::AggKind;
    use std::cmp::Ordering::*;
    let mut whole = a.clone();
    let mut stands: Vec<String> = Vec::new();
    let mut brought: Vec<String> = Vec::new();
    let least = |x: Rat, y: Rat| if x.cmp_to(y) == Greater { y } else { x };
    let most = |x: Rat, y: Rat| if x.cmp_to(y) == Less { y } else { x };
    for el in &f.elements {
        let seq = el.name.text.clone();
        let Some(Val::Seq(start)) = whole.get(&seq).cloned() else { continue };
        let aggs: Vec<&AggDecl> = f.aggs().filter(|d| d.over == seq).collect();
        if aggs.is_empty() {
            continue;
        }
        let base = whole.clone();
        // What every summary comes to on the sequence, one walk for all of them.
        let got = |xs: &[BTreeMap<String, Val>]| -> std::collections::HashMap<String, Val> {
            let mut m = base.clone();
            m.insert(seq.clone(), Val::Seq(xs.to_vec()));
            eval::summaries_past(f, c, m.into_iter().collect())
        };
        let value = |xs: &[BTreeMap<String, Val>], d: &AggDecl| got(xs).get(&d.name.text).and_then(as_rat);
        let top = |d: &AggDecl| if d.kind == AggKind::Sum { c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi) } else { None };
        // Where a summary is outside: under its bottom (`Less`) or past its top (`Greater`).
        let off = |xs: &[BTreeMap<String, Val>], d: &AggDecl| -> Option<std::cmp::Ordering> {
            let v = value(xs, d)?;
            if c.walk_floor(d).is_some_and(|lo| v.cmp_to(lo) == Less) {
                Some(Less)
            } else if top(d).is_some_and(|hi| v.cmp_to(hi) == Greater) {
                Some(Greater)
            } else {
                None
            }
        };
        let outside: Vec<&AggDecl> = aggs.iter().filter(|d| off(&start, d).is_some()).copied().collect();
        if outside.is_empty() {
            continue;
        }
        let first = got(&start);
        let kept: Vec<(&AggDecl, Option<Val>)> = aggs
            .iter()
            .filter(|d| !outside.iter().any(|o| o.name.text == d.name.text) && (hold.is_empty() || hold.contains(&d.name.text)))
            .map(|d| (*d, first.get(&d.name.text).cloned()))
            .collect();
        let same = |x: Option<&Val>, y: Option<&Val>| match (x, y) {
            (Some(Val::Num(x)), Some(Val::Num(y))) => x.cmp_to(*y) == Equal,
            (x, y) => x == y,
        };
        let holds = |xs: &[BTreeMap<String, Val>]| {
            let now = got(xs);
            kept.iter().all(|(d, v)| same(now.get(&d.name.text), v.as_ref()))
        };
        for (d, v) in &kept {
            if let Some(v) = v {
                stands.push(format!("{} = {}", d.name.text, show(v)));
            }
        }
        brought.extend(outside.iter().map(|d| d.name.text.clone()));
        let cap = crate::door::count_cap(f, c, &seq).unwrap_or(EDGE_ELEMENTS).min(EDGE_ELEMENTS);
        // A field a summary reads as it is, with how far it can be moved: its range, kept under
        // what a `max` that was inside came to and over what a `min` did.
        let field = |col: &str| -> Option<(Rat, Rat)> {
            el.fields.iter().find(|fd| fd.name.text == col && fd.unique.is_none())?;
            let Some(&(Some(mut lo), Some(mut hi))) = c.ranges.get(col) else { return None };
            for (d, v) in &kept {
                let Some(v) = v.as_ref().and_then(as_rat) else { continue };
                if d.column.text == col {
                    match d.kind {
                        AggKind::Max => hi = least(hi, v),
                        AggKind::Min => lo = most(lo, v),
                        _ => {}
                    }
                }
            }
            (lo.cmp_to(hi) != Greater).then_some((lo, hi))
        };
        let num = |e: &BTreeMap<String, Val>, col: &str| e.get(col).and_then(as_rat);
        let mut templates: Vec<BTreeMap<String, Val>> = Vec::new();
        for e in start.iter().cloned().chain(element_combos(el, cands).into_iter().take(FIT_TEMPLATES)) {
            if !templates.contains(&e) {
                templates.push(e);
            }
        }
        // The columns the summaries over the sequence read, each once.
        let mut read: Vec<String> = Vec::new();
        for d in &aggs {
            if d.kind != AggKind::Count && !read.contains(&d.column.text) {
                read.push(d.column.text.clone());
            }
        }
        // The element `t` put after the sequence: each column a summary reads at the bottom of where
        // it can be, but `raise`, the column a total under its bottom adds up, at the value given;
        // and what it adds to each total that was inside taken back off the elements before it, as
        // far as their fields go down. `None` where that leaves a summary that was inside moved.
        let put = |xs: &[BTreeMap<String, Val>], t: &BTreeMap<String, Val>, raise: Option<(&str, Rat)>| -> Option<Vec<BTreeMap<String, Val>>> {
            let mut e = t.clone();
            for col in &read {
                if let Some((flo, _)) = field(col) {
                    e.insert(col.clone(), Val::Num(flo));
                }
            }
            if let Some((col, v)) = raise {
                e.insert(col.to_string(), Val::Num(v));
            }
            let mut ys = xs.to_vec();
            ys.push(e);
            for (s, want) in &kept {
                if s.kind != AggKind::Sum {
                    continue;
                }
                let scol = &s.column.text;
                let (Some(want), Some(is), Some((flo, _))) = (want.as_ref().and_then(as_rat), value(&ys, s), field(scol)) else { continue };
                let mut over = is.sub(want);
                for k in 0..xs.len() {
                    if over.num <= 0 {
                        break;
                    }
                    let Some(old) = num(&ys[k], scol) else { continue };
                    let take = least(old.sub(flo), over);
                    if take.num <= 0 {
                        continue;
                    }
                    ys[k].insert(scol.clone(), Val::Num(old.sub(take)));
                    over = over.sub(take);
                }
            }
            holds(&ys).then_some(ys)
        };
        let mut xs = start.clone();
        for _ in 0..aggs.len() * 2 {
            let Some((d, side)) = aggs.iter().find_map(|d| off(&xs, d).map(|o| (*d, o))) else { break };
            let Some(now) = value(&xs, d) else { break };
            let col = d.column.text.clone();
            let moved = match (d.kind, side) {
                // A total under its bottom: the field up in place, then elements put after it.
                (AggKind::Sum, Less) => {
                    let lo = c.walk_floor(d)?;
                    let need = |xs: &[BTreeMap<String, Val>]| value(xs, d).map(|v| lo.sub(v));
                    if let Some((_, fhi)) = field(&col) {
                        for k in 0..xs.len() {
                            let Some(left) = need(&xs).filter(|n| n.num > 0) else { break };
                            let Some(old) = num(&xs[k], &col) else { continue };
                            let new = least(old.add(left), fhi);
                            if new.cmp_to(old) != Greater {
                                continue;
                            }
                            let mut ys = xs.clone();
                            ys[k].insert(col.clone(), Val::Num(new));
                            if holds(&ys) {
                                xs = ys;
                            }
                        }
                    }
                    for t in &templates {
                        while let Some(left) = need(&xs).filter(|n| n.num > 0) {
                            if xs.len() as i128 >= cap {
                                break;
                            }
                            let raise = field(&col).map(|(flo, fhi)| (col.as_str(), most(least(left, fhi), flo)));
                            match put(&xs, t, raise) {
                                Some(ys) if value(&ys, d).zip(value(&xs, d)).is_some_and(|(is, was)| is.cmp_to(was) == Greater) => xs = ys,
                                _ => break,
                            }
                        }
                    }
                    need(&xs).is_some_and(|n| n.num <= 0)
                }
                // A total past its top: the field down in place, then elements taken out.
                (AggKind::Sum, Greater) => {
                    let hi = top(d)?;
                    let mut need = now.sub(hi);
                    if let Some((flo, _)) = field(&col) {
                        for k in 0..xs.len() {
                            if need.num <= 0 {
                                break;
                            }
                            let Some(old) = num(&xs[k], &col) else { continue };
                            let new = most(old.sub(need), flo);
                            if new.cmp_to(old) != Less {
                                continue;
                            }
                            let mut ys = xs.clone();
                            ys[k].insert(col.clone(), Val::Num(new));
                            if holds(&ys) {
                                need = need.sub(old.sub(new));
                                xs = ys;
                            }
                        }
                    }
                    let mut k = xs.len();
                    while need.num > 0 && k > 0 {
                        k -= 1;
                        let mut ys = xs.clone();
                        ys.remove(k);
                        let (Some(was), Some(is)) = (value(&xs, d), value(&ys, d)) else { continue };
                        let drop = was.sub(is);
                        if drop.num > 0 && holds(&ys) {
                            need = need.sub(drop);
                            xs = ys;
                        }
                    }
                    need.num <= 0
                }
                // A count under its bottom: elements it counts put after it.
                (AggKind::Count, Less) => {
                    let lo = c.walk_floor(d)?;
                    let short = |xs: &[BTreeMap<String, Val>]| value(xs, d).is_some_and(|v| v.cmp_to(lo) == Less);
                    let mut done = false;
                    for t in &templates {
                        let mut ys = xs.clone();
                        while short(&ys) && (ys.len() as i128) < cap {
                            match put(&ys, t, None) {
                                Some(zs) if value(&zs, d).zip(value(&ys, d)).is_some_and(|(is, was)| is.cmp_to(was) == Greater) => ys = zs,
                                _ => break,
                            }
                        }
                        if !short(&ys) {
                            xs = ys;
                            done = true;
                            break;
                        }
                    }
                    done
                }
                _ => false,
            };
            if !moved {
                return None;
            }
        }
        if aggs.iter().any(|d| off(&xs, d).is_some()) || !holds(&xs) {
            return None;
        }
        whole.insert(seq, Val::Seq(xs));
    }
    let sep = if crate::i18n::ja() { "、" } else { ", " };
    let (stands, moved) = (stands.join(sep), brought.join(sep));
    let why = if stands.is_empty() {
        tr!("{moved} を範囲の中に戻した", "{moved} brought inside its range")
    } else {
        tr!("{stands} はそのままで、{moved} を範囲の中に戻した", "{stands} kept, {moved} brought inside its range")
    };
    answer(f, c, &whole, "").map(|v| (v.input, why))
}

/// How many of the vectors a boundary on a value worked out once a walk is over is aimed from.
const WALKED_BASES: usize = 4;

/// The cases a table that reads a value worked out once a walk is over is owed for each boundary a
/// cell writes on that value (§9.2): a whole input that brings the value to the inside of the
/// boundary and, from that one, the input that takes it to the outside — the two apart in one
/// sequence, or in one input, alone, which is what makes them a pair. The pool aims a summary at
/// the boundaries the tables write on it; a value worked out from one has its boundaries
/// somewhere the pool does not look, and is aimed here, solved as `walked_edges` solves an end.
/// The vectors the row won on are tried first as the place to start from, since the other cells of
/// the row have to hold on the pair, and then the first vectors of all.
fn walked_cases(f: &RuleFile, c: &Checked, vectors: &[Vector]) -> Vec<(BTreeMap<String, Val>, String)> {
    let mut out: Vec<(BTreeMap<String, Val>, String)> = Vec::new();
    if !f.walks() || vectors.is_empty() {
        return out;
    }
    let walked: Vec<&str> = walked_defines(f).into_iter().map(|d| d.name.text.as_str()).collect();
    if walked.is_empty() {
        return out;
    }
    for set in &c.sets {
        let t = &set.table;
        for (ri, row) in t.rows.iter().enumerate() {
            let tag = eval::row_tag(set.row_table(ri), row.index);
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if !walked.contains(&col.as_str()) {
                    continue;
                }
                let Some(ty) = c.ty_of(col).map(|t| t.present().clone()) else { continue };
                if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                for (_, inside, outside) in crate::coverage::pairs_on(c, col, cell, &ty) {
                    let won = vectors.iter().filter(|v| v.trace.contains(&tag)).take(WALKED_BASES);
                    for base in won.chain(vectors.iter().take(WALKED_BASES)) {
                        let Some(a) = walked_at(f, c, vectors, &base.input, col, inside) else { continue };
                        let Some(z) = walked_at(f, c, vectors, &a, col, outside) else { continue };
                        // A pair is two inputs apart in one thing.
                        if a.len() != z.len() || a.iter().filter(|(k, v)| z.get(*k) != Some(*v)).count() != 1 {
                            continue;
                        }
                        let said = |v: Rat| tr!("{col} が {} になる入力", "{col} brought to {}", crate::coverage::show_rat(v, &ty));
                        out.push((a, said(inside)));
                        out.push((z, said(outside)));
                    }
                }
            }
        }
    }
    out
}

/// A whole input the rule answers on which the value `name`, worked out once a walk is over, is
/// `target`: `base` itself where it is one, else `base` with an input, or the sequence a summary
/// walks, moved to where the value is solved to be there.
fn walked_at(f: &RuleFile, c: &Checked, vectors: &[Vector], base: &BTreeMap<String, Val>, name: &str, target: Rat) -> Option<BTreeMap<String, Val>> {
    use std::cmp::Ordering::*;
    let seed = flat_of(f, c, base);
    let there = |whole: &BTreeMap<String, Val>| flat_value(f, c, &flat_of(f, c, whole), name).is_some_and(|w| w.cmp_to(target) == Equal) && answer(f, c, whole, "").is_some();
    if there(base) {
        return Some(base.clone());
    }
    let movable = walked_movable(f);
    [true, false].into_iter().find_map(|up| {
        let a = solve_flat(f, c, &seed, &movable, name, target, up)?;
        if flat_value(f, c, &a, name)?.cmp_to(target) != Equal {
            return None;
        }
        with_summaries(f, c, vectors, base, &seed, &a, name).into_iter().find(|whole| there(whole))
    })
}

/// The inputs the door of every generated language refuses, as lines of the same file
/// (§15.204): for each input, the input left out and a value of another kind; for a number, one
/// that is not whole (off its step, for a rate) and one past each end of its range; for a date,
/// a day the calendar does not have, one past each end and a day its koyomi date does not come
/// to; for an enum, a word it does not have; for a truth value, the string "false"; for each
/// `constraint`, a combination it rules out; and for the sequence a walk reads, the same of the
/// sequence and of one element's fields, one element past what a count over it can be, and a `sum`
/// or a `count` one step past each end its walk holds it to (`summary_edges`). Each
/// is a vector of the suite with one thing changed, so the door has one thing to say, and the
/// line carries what the reference evaluator refuses it for. A case the reference evaluator does
/// not refuse — an end the type already closes — is not written. After them come the inputs with
/// two things wrong, of two reasons, for each pair of places the door looks at (`two_faults`): the
/// door says the first, in the one order every way looks in (§15.328).
pub fn door_cases(f: &RuleFile, c: &Checked, vectors: &[Vector]) -> Vec<String> {
    door_lines(f, c, vectors).into_iter().map(|(_, line)| line).collect()
}

/// Every line of `vectors/<alias>.refused.jsonl` (§15.56, §15.204): the walks the suite found
/// refused, the door cases made from its vectors, and the calls on an end of a range that none of
/// the cases tried there answers (§15.314). An input is written once: a walk the suite found that
/// a door case gives as well — no elements, under a `count` that starts above zero — is the door
/// case, which names the end it is past. Two inputs refused for the same end of the same `sum` or
/// `count` both stay — the walk the suite found, which is often the one farthest from the end (no
/// elements, a total of zero), and the door case one step past it: a door that holds the end only
/// for some of the calls under it is stopped by one of the two and not by the other.
pub fn refused_lines(f: &RuleFile, c: &Checked, suite: &Suite) -> Vec<String> {
    let door = door_lines(f, c, &suite.vectors);
    let mut said: BTreeSet<String> = door.iter().map(|(text, _)| text.clone()).collect();
    let mut out: Vec<String> = Vec::new();
    for v in &suite.refused {
        let text = crate::json::parse(&in_object(f, c, v)).map(|j| j.compact()).unwrap_or_default();
        if said.insert(text) {
            out.push(refused_json(f, c, v));
        }
    }
    out.extend(door.iter().map(|(_, line)| line.clone()));
    for v in &suite.on_ends {
        let text = crate::json::parse(&in_object(f, c, v)).map(|j| j.compact()).unwrap_or_default();
        if said.insert(text) {
            out.push(refused_json(f, c, v));
        }
    }
    out
}

/// The door cases, each with its inputs as the line writes them.
fn door_lines(f: &RuleFile, c: &Checked, vectors: &[Vector]) -> Vec<(String, String)> {
    let parsed = |v: &Vector| crate::json::parse(&in_object(f, c, v)).ok();
    let Some(base) = vectors.first().and_then(parsed) else { return Vec::new() };
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut push = |obj: Json, why: String| -> bool {
        let text = obj.compact();
        if !seen.insert(text.clone()) {
            return false;
        }
        if let Some(r) = crate::door::refusal(f, c, &obj) {
            let line = refused_line(&text, &r, &why);
            out.push((text, line));
            return true;
        }
        false
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
    for n in f.door_nevers() {
        if let Some(obj) = inside_never(f, c, n, &base) {
            push(obj, tr!("`{}` の組み合わせ", "the combination of `{}`", n.said()));
        }
    }
    // A `define` one step past each end of the range the rule accepts for it (§15.313): the vector
    // with what the value is worked out from moved there — for a value of one element, an element
    // after a good one where the sequence can have two, so that the door says it at its index
    // (§15.324).
    // The first element of each sequence in the first vector that has one: the good element a
    // case puts before a bad one.
    let goods: Vec<Option<Json>> = f
        .elements
        .iter()
        .map(|el| vectors.iter().filter_map(parsed).find_map(|j| j.get(&el.name.text).and_then(|v| v.as_arr()).and_then(|xs| xs.first().cloned())))
        .collect();
    for e in edge_inputs(f, c, vectors, false) {
        let Ok(obj) = crate::json::parse(&in_object(f, c, &e.1)) else { continue };
        let obj = match e.0.seq {
            Some(s) => {
                let seq = &f.elements[s].name.text;
                let from = parsed(&vectors[e.0.from]).and_then(|j| j.get(seq).and_then(|v| v.as_arr()).and_then(|xs| xs.first().cloned()));
                past_on_second(f, c, &obj, s, &e.0.define, from.as_ref().or(goods[s].as_ref()))
            }
            None => crate::door::refusal(f, c, &obj).is_some_and(|r| r.error == crate::door::out_of_range(&e.0.define)).then_some(obj),
        };
        if let Some(obj) = obj {
            let why = if e.0.hi {
                tr!("定義 {} を、受け付ける範囲の上端の先に", "the definition {} past the top of the range it accepts", e.0.define)
            } else {
                tr!("定義 {} を、受け付ける範囲の下端の先に", "the definition {} past the bottom of the range it accepts", e.0.define)
            };
            push(obj, why);
        }
    }
    // The same of a `define` worked out once a walk is over (§15.314): a sequence that takes the
    // summary it reads to where the value is one step past the end.
    for (e, a) in walked_edges(f, c, vectors) {
        if e.inside {
            continue;
        }
        let v = Vector { input: a, outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() };
        let Ok(obj) = crate::json::parse(&in_object(f, c, &v)) else { continue };
        if crate::door::refusal(f, c, &obj).is_some_and(|r| r.error == crate::door::out_of_range(&e.define)) {
            push(obj, e.why());
        }
    }
    // A `sum` or a `count` one step past each end of the range its walk holds it to (§15.100,
    // §15.314): past the top of a sum, and under the bottom of either where a walk can come in
    // under it. The door says that summary, and nothing before it.
    for (e, a) in summary_edges(f, c, vectors) {
        if e.inside {
            continue;
        }
        let v = Vector { input: a, outputs: Vec::new(), trace: Vec::new(), fired: Vec::new(), why: String::new() };
        let Ok(obj) = crate::json::parse(&in_object(f, c, &v)) else { continue };
        if crate::door::refusal(f, c, &obj).is_some_and(|r| r.error == crate::door::out_of_range(&e.name)) {
            push(obj, e.why());
        }
    }
    for el in &f.elements {
        let seq = &el.name.text;
        let with_one = vectors
            .iter()
            .find(|v| matches!(v.input.get(seq), Some(Val::Seq(xs)) if !xs.is_empty()))
            .and_then(parsed);
        let top = with_one.clone().unwrap_or_else(|| base.clone());
        let cap = crate::door::count_cap(f, c, seq);
        let first = with_one.as_ref().and_then(|t| t.get(seq)).and_then(|s| s.as_arr()).and_then(|xs| xs.first()).cloned();
        push(without(&top, seq), tr!("{seq} を渡さない", "{seq} left out"));
        push(with(&top, seq, Json::Int(0)), tr!("{seq} に並びでない値", "{seq} not a sequence"));
        // An element that is not an object after a good one, and another after it: the door says
        // the first of the two by its index (§15.324). Alone, where the sequence has no good element
        // to put first or can have only one.
        let odd = match &first {
            Some(good) if cap.is_none_or(|cap| cap >= 3) => vec![good.clone(), Json::Int(0), Json::Int(0)],
            Some(good) if cap.is_none_or(|cap| cap >= 2) => vec![good.clone(), Json::Int(0)],
            _ => vec![Json::Int(0)],
        };
        let odd = with(&top, seq, Json::Arr(odd));
        let odd = if crate::door::refusal(f, c, &odd).is_some_and(|r| r.error == crate::door::not_object(seq, 1)) { odd } else { with(&top, seq, Json::Arr(vec![Json::Int(0)])) };
        push(odd, tr!("{seq} にオブジェクトでない要素", "an element of {seq} that is not an object"));
        if let Some(first) = first {
            if let Some(cap) = cap.filter(|cap| *cap <= 50) {
                let many = Json::Arr(vec![first.clone(); cap as usize + 1]);
                push(with(&top, seq, many), tr!("{seq} の要素を、数え上げの上限より一つ多く", "{seq} one element past what a count over it can be"));
            }
            // The fields marked `unique` (§15.290), and the values each takes in elements built side
            // by side: the vector's own first, then others its door takes.
            let uniques: Vec<&crate::ast::VarDecl> = el.fields.iter().filter(|fd| fd.unique.is_some()).collect();
            let spread_of = |n: usize| -> Option<Vec<Json>> {
                let mut xs = vec![first.clone(); n];
                for fd in &uniques {
                    let ty = c.ty_of(&fd.name.text).unwrap_or(Ty::Unknown);
                    let vs = spread(c, &fd.name.text, &ty, first.get(&fd.name.text), n)?;
                    for (x, v) in xs.iter_mut().zip(vs) {
                        *x = with(x, &fd.name.text, v);
                    }
                }
                Some(xs)
            };
            // Two elements with the same value of a `unique` field, one case for each such field:
            // the two differ in every other `unique` field, so the door has that one to say. The
            // first and the third, where the sequence can have three and the walk takes the first
            // two, so that the door says both by their indices (§15.324); else the first two.
            if cap.is_none_or(|cap| cap >= 2) {
                for fd in &uniques {
                    let field = &fd.name.text;
                    let why = tr!("{seq} の要素を、{} が同じ二つに", "two elements of {seq} with the same {}", field);
                    let three = spread_of(3).filter(|_| cap.is_none_or(|cap| cap >= 3)).and_then(|mut xs| {
                        // The first two walked over with no refusal of an element's own.
                        let walked = with(&top, seq, Json::Arr(xs[..2].to_vec()));
                        if crate::door::refusal(f, c, &walked).is_some_and(|r| r.error.contains(&format!("{seq}["))) {
                            return None;
                        }
                        let same = xs[0].get(field).cloned().unwrap_or(Json::Null);
                        xs[2] = with(&xs[2], field, same);
                        let obj = with(&top, seq, Json::Arr(xs));
                        crate::door::refusal(f, c, &obj).is_some_and(|r| r.error == crate::door::duplicate(seq, field, 0, 2)).then_some(obj)
                    });
                    if let Some(obj) = three {
                        push(obj, why);
                        continue;
                    }
                    let Some(mut two) = spread_of(2) else { continue };
                    let same = two[0].get(field).cloned().unwrap_or(Json::Null);
                    two[1] = with(&two[1], field, same);
                    let obj = with(&top, seq, Json::Arr(two));
                    if crate::door::refusal(f, c, &obj).is_some_and(|r| r.error == crate::door::duplicate(seq, field, 0, 1)) {
                        push(obj, why);
                    }
                }
            }
            for fd in &el.fields {
                let name = &fd.name.text;
                let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
                let mut cases = vec![(None, tr!("{seq} の要素の {name} を渡さない", "{name} left out of an element of {seq}"))];
                for (value, why) in wrong_values(c, name, &ty, first.get(name)) {
                    cases.push((Some(value), tr!("{seq} の要素の {why}", "{why}, in an element of {seq}")));
                }
                // The value on the elements after a good one, so that the door says the first of them
                // by its index (§15.324): two of them where the sequence can have three. Else the
                // first element of the vector's own sequence.
                let said = format!("{} ", crate::door::element(seq, 1, name));
                for (value, why) in cases {
                    let bad = |x: &Json| match &value {
                        None => without(x, name),
                        Some(v) => with(x, name, v.clone()),
                    };
                    let after = [3, 2].into_iter().filter(|n| cap.is_none_or(|cap| cap >= *n as i128)).find_map(|n| {
                        let mut xs = spread_of(n)?;
                        for x in xs.iter_mut().skip(1) {
                            *x = bad(x);
                        }
                        let obj = with(&top, seq, Json::Arr(xs));
                        crate::door::refusal(f, c, &obj).is_some_and(|r| r.error.starts_with(&said)).then_some(obj)
                    });
                    if let Some(obj) = after {
                        push(obj, why);
                        continue;
                    }
                    let mut xs = top.get(seq).and_then(|s| s.as_arr()).map(|a| a.to_vec()).unwrap_or_default();
                    xs[0] = bad(&first);
                    push(with(&top, seq, Json::Arr(xs)), why);
                }
            }
        }
    }
    for (obj, why) in two_faults(f, c, vectors, &base) {
        push(obj, why);
    }
    out
}

/// Inputs with two things wrong with them, of two different reasons, that the door says in the
/// order every way looks (§15.328): first the reading look — each argument in the order the function
/// takes them, there and of its kind, and a number one int64 holds — then each input inside its
/// range, the constraints, the cap of a sequence, and then the walk, where an element's fields are
/// held to their ranges and its `unique` fields to the elements before it, and where a `stop` ends
/// the work and not the door. One case for each pair of places, each kept only where the reference
/// evaluator says the one the order puts first: a door that looks in another order says the other.
fn two_faults(f: &RuleFile, c: &Checked, vectors: &[Vector], base: &Json) -> Vec<(Json, String)> {
    let parsed = |v: &Vector| crate::json::parse(&in_object(f, c, v)).ok();
    let said = |obj: &Json| crate::door::refusal(f, c, obj).map(|r| r.error);
    let mut out: Vec<(Json, String)> = Vec::new();
    // `obj`, kept where the reference evaluator says of it what it says of `alone`, the input with
    // only the thing wrong the order puts first, and says another sentence of `other`, the input with
    // only the other: both are refused, and the order is what decides which is said.
    let keep = |out: &mut Vec<(Json, String)>, obj: Json, alone: &Json, other: &Json, why: String| -> bool {
        let want = said(alone);
        let theirs = said(other);
        if want.is_some() && theirs.is_some() && theirs != want && said(&obj) == want {
            out.push((obj, why));
            return true;
        }
        false
    };
    let ty = |n: &str| c.ty_of(n).unwrap_or(Ty::Unknown);
    let ins: Vec<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
    // An input outside its range, and one after it of another kind: the kind.
    'pair: for (k, a) in ins.iter().enumerate() {
        let Some(ra) = range_fault(c, a, &ty(a), base.get(a)) else { continue };
        for b in &ins[k + 1..] {
            let Some(kb) = kind_fault(c, b, &ty(b), base.get(b)) else { continue };
            let alone = with(base, b, kb);
            let why = tr!("{a} を範囲の外に、あとの {b} を別の種類の値に。種類のほうを言う", "{a} outside its range and {b} after it of another kind: the kind is said");
            if keep(&mut out, with(&alone, a, ra.clone()), &alone, &with(base, a, ra.clone()), why) {
                break 'pair;
            }
        }
    }
    // A number past what int64 holds, and an input after it of another kind: the number, read
    // first.
    'pair: for (k, a) in ins.iter().enumerate() {
        let Some(pa) = past_fault(&ty(a)) else { continue };
        let alone = with(base, a, pa);
        for b in &ins[k + 1..] {
            let Some(kb) = kind_fault(c, b, &ty(b), base.get(b)) else { continue };
            let why = tr!("{a} を int64 で持てない数に、あとの {b} を別の種類の値に。先に読む {a} を言う", "{a} past what int64 holds and {b} after it of another kind: {a}, read first, is said");
            if keep(&mut out, with(&alone, b, kb.clone()), &alone, &with(base, b, kb), why) {
                break 'pair;
            }
        }
    }
    // Each sequence by the first vector that has an element in it, and that element.
    let tops: Vec<Option<(Json, Json)>> = f
        .elements
        .iter()
        .map(|el| {
            let seq = &el.name.text;
            let top = vectors.iter().find(|v| matches!(v.input.get(seq), Some(Val::Seq(xs)) if !xs.is_empty())).and_then(parsed)?;
            let first = top.get(seq)?.as_arr()?.first()?.clone();
            Some((top, first))
        })
        .collect();
    // A field of an element with a value of another kind, and one outside its range, each the first
    // the element has.
    let field_fault = |s: usize, first: &Json, range: bool| -> Option<(String, Json)> {
        f.elements[s].fields.iter().find_map(|fd| {
            let n = &fd.name.text;
            let v = if range { range_fault(c, n, &ty(n), first.get(n)) } else { kind_fault(c, n, &ty(n), first.get(n)) }?;
            Some((n.clone(), v))
        })
    };
    // Elements that differ in every `unique` field, the first of them `first`.
    let apart = |s: usize, first: &Json, n: usize| -> Vec<Json> {
        let mut xs = vec![first.clone(); n];
        for fd in f.elements[s].fields.iter().filter(|fd| fd.unique.is_some()) {
            if let Some(vs) = spread(c, &fd.name.text, &ty(&fd.name.text), first.get(&fd.name.text), n) {
                for (x, v) in xs.iter_mut().zip(vs) {
                    *x = with(x, &fd.name.text, v);
                }
            }
        }
        xs
    };
    for (s, el) in f.elements.iter().enumerate() {
        let Some((top, first)) = &tops[s] else { continue };
        let seq = &el.name.text;
        let cap = crate::door::count_cap(f, c, seq);
        let one = |x: Json| with(top, seq, Json::Arr(vec![x]));
        if let Some((fk, kv)) = field_fault(s, first, false) {
            let alone = one(with(first, &fk, kv.clone()));
            // An input outside its range, and a field of an element of another kind: the field,
            // read with the arguments before any range is looked at.
            for a in &ins {
                let Some(ra) = range_fault(c, a, &ty(a), top.get(a)) else { continue };
                let why = tr!("{a} を範囲の外に、{seq} の要素の {fk} を別の種類の値に。種類のほうを言う", "{a} outside its range and {fk} of an element of {seq} of another kind: the kind is said");
                if keep(&mut out, with(&alone, a, ra.clone()), &alone, &with(&one(first.clone()), a, ra), why) {
                    break;
                }
            }
            // An input of another kind, and a field of an element of another kind: the input, an
            // argument before the sequence.
            for a in &ins {
                let Some(ka) = kind_fault(c, a, &ty(a), top.get(a)) else { continue };
                let input_alone = with(top, a, ka.clone());
                let why = tr!("{a} を別の種類の値に、{seq} の要素の {fk} も別の種類の値に。先の引数の {a} を言う", "{a} of another kind and {fk} of an element of {seq} of another kind too: {a}, the argument before, is said");
                if keep(&mut out, with(&alone, a, ka), &input_alone, &alone, why) {
                    break;
                }
            }
        }
        if cap.is_none_or(|cap| cap >= 2) {
            // A field of the first element outside its range, and a field of the second of another
            // kind: the kind, read for every element before the walk holds one to a range.
            if let (Some((fr, rv)), Some((fk, kv))) = (field_fault(s, first, true), field_fault(s, first, false)) {
                let xs = apart(s, first, 2);
                let alone = with(top, seq, Json::Arr(vec![xs[0].clone(), with(&xs[1], &fk, kv.clone())]));
                let other = with(top, seq, Json::Arr(vec![with(&xs[0], &fr, rv.clone()), xs[1].clone()]));
                let obj = with(top, seq, Json::Arr(vec![with(&xs[0], &fr, rv), with(&xs[1], &fk, kv)]));
                keep(&mut out, obj, &alone, &other, tr!("{seq} の一つ目の要素の {fr} を範囲の外に、二つ目の {fk} を別の種類の値に。種類のほうを言う", "{fr} of the first element of {seq} outside its range and {fk} of the second of another kind: the kind is said"));
            }
            // The second element with the `unique` value of the first, and a field of it outside its
            // range: the range, which the walk looks at on an element before its `unique` fields.
            let unique = el.fields.iter().find(|fd| fd.unique.is_some()).map(|fd| fd.name.text.clone());
            let ranged = el.fields.iter().filter(|fd| Some(&fd.name.text) != unique.as_ref()).find_map(|fd| {
                let n = &fd.name.text;
                Some((n.clone(), range_fault(c, n, &ty(n), first.get(n))?))
            });
            if let (Some(u), Some((fr, rv))) = (unique, ranged) {
                let xs = apart(s, first, 2);
                let second = with(&xs[1], &fr, rv.clone());
                let alone = with(top, seq, Json::Arr(vec![xs[0].clone(), second.clone()]));
                let first_u = xs[0].get(&u).cloned().unwrap_or(Json::Null);
                let other = with(top, seq, Json::Arr(vec![xs[0].clone(), with(&xs[1], &u, first_u.clone())]));
                let obj = with(top, seq, Json::Arr(vec![xs[0].clone(), with(&second, &u, first_u.clone())]));
                keep(&mut out, obj, &alone, &other, tr!("{seq} の二つ目の要素の {u} を一つ目と同じに、その {fr} を範囲の外に。範囲のほうを言う", "{u} of the second element of {seq} the same as the first's, and its {fr} outside its range: the range is said"));
                // The second element with the `unique` value of the first, and the third outside its
                // range: the two the same, which the walk meets first.
                let xs = apart(s, first, 3);
                if cap.is_none_or(|cap| cap >= 3) {
                    let same = with(&xs[1], &u, first_u);
                    let third = with(&xs[2], &fr, rv);
                    let alone = with(top, seq, Json::Arr(vec![xs[0].clone(), same.clone(), xs[2].clone()]));
                    let other = with(top, seq, Json::Arr(vec![xs[0].clone(), xs[1].clone(), third.clone()]));
                    let obj = with(top, seq, Json::Arr(vec![xs[0].clone(), same, third]));
                    keep(&mut out, obj, &alone, &other, tr!("{seq} の二つ目の要素の {u} を一つ目と同じに、三つ目の {fr} を範囲の外に。歩きが先に会う重なりのほうを言う", "{u} of the second element of {seq} the same as the first's, and {fr} of the third outside its range: the two the same, which the walk meets first, are said"));
                }
            }
        }
        // One element past the cap of the sequence, and a combination a constraint rules out: the
        // constraint, which the door looks at before the cap.
        if let Some(cap) = cap.filter(|cap| *cap <= 50) {
            for k in &f.constraints {
                let Some(broken) = breaking(f, c, k, top) else { continue };
                let many = Json::Arr(apart(s, first, cap as usize + 1));
                let obj = with(&broken, seq, many.clone());
                if keep(&mut out, obj, &broken, &with(top, seq, many), tr!("{seq} の要素を、数え上げの上限より一つ多くし、制約 {} も破る。制約のほうを言う", "{seq} one element past its cap and the constraint {} broken: the constraint is said", k.said())) {
                    break;
                }
            }
        }
        // A `define` worked out before the walk past the range the rule accepts for it, and a field
        // of an element outside its range: the `define`, worked out before the walk reads an element.
        if let Some((fr, rv)) = field_fault(s, first, true) {
            for e in edge_inputs(f, c, vectors, false) {
                if e.0.seq.is_some() {
                    continue;
                }
                let Some(alone) = parsed(&e.1) else { continue };
                if said(&alone) != Some(crate::door::out_of_range(&e.0.define)) {
                    continue;
                }
                let bad = Json::Arr(vec![with(first, &fr, rv.clone())]);
                let obj = with(&alone, seq, bad.clone());
                let why = tr!("定義 {} を、受け付ける範囲の外に、{seq} の要素の {fr} も範囲の外に。歩きの前に求める定義のほうを言う", "the definition {} outside the range it accepts and {fr} of an element of {seq} outside its range: the definition, worked out before the walk, is said", e.0.define);
                if keep(&mut out, obj, &alone, &with(top, seq, bad), why) {
                    break;
                }
            }
        }
        // An element the fold stops at, and after it one with a field outside its range: the
        // range, which the door looks at past the `stop` on the first walk over the sequence. And two
        // elements taking under a `take_unique`, and a third outside its range: the two, which the
        // walk meets first.
        let Some((fr, rv)) = field_fault(s, first, true) else { continue };
        let (mut stopped, mut taken) = (false, false);
        for (i, it) in f.items.iter().enumerate() {
            let Item::Fold(fold) = it else { continue };
            if fold.over != *seq {
                continue;
            }
            let arms = |pick: fn(&Arm) -> bool| -> Vec<&str> { fold.arms.iter().filter(|(_, a, _)| pick(a)).map(|(n, _, _)| n.text.as_str()).collect() };
            // The first vector with an element whose verdict is one of `names`, the sequence it has,
            // and where that element is in it.
            let landing = |names: &[&str]| {
                vectors.iter().find_map(|v| {
                    let verdicts = eval::fold_verdicts(f, c, v.input.clone().into_iter().collect());
                    let at = verdicts.get(&i)?.iter().position(|x| x.as_deref().is_some_and(|x| names.contains(&x)))?;
                    let obj = parsed(v)?;
                    let xs = obj.get(seq)?.as_arr()?.to_vec();
                    Some((obj, xs, at))
                })
            };
            let stops = arms(|a| matches!(a, Arm::Stop(_)));
            if let Some((obj, xs, at)) = (!stopped && !stops.is_empty()).then(|| landing(&stops)).flatten().filter(|(_, _, at)| cap.is_none_or(|cap| cap >= *at as i128 + 2)) {
                let mut walked = xs[..=at].to_vec();
                let good = apart(s, &xs[at], 2).pop().unwrap_or_else(|| xs[at].clone());
                walked.push(with(&good, &fr, rv.clone()));
                let row = with(&obj, seq, Json::Arr(walked));
                let want = crate::door::out_of_range(&crate::door::element(seq, at + 1, &fr));
                if said(&row) == Some(want) {
                    out.push((row, tr!("{seq} の要素で fold が止まり、その次の要素の {fr} を範囲の外に。止まったあとの要素も入口で確かめる", "the fold stops on an element of {seq}, and {fr} of the element after it outside its range: the door looks at the elements past the stop")));
                    stopped = true;
                }
            }
            let takes = arms(|a| matches!(a, Arm::Take { unique: true, .. }));
            if let Some((obj, xs, at)) = (!taken && !takes.is_empty() && cap.is_none_or(|cap| cap >= 3)).then(|| landing(&takes)).flatten() {
                let ts = apart(s, &xs[at], 3);
                let bad = with(&ts[2], &fr, rv.clone());
                let alone = with(&obj, seq, Json::Arr(ts.clone()));
                let other = with(&obj, seq, Json::Arr(vec![ts[0].clone(), bad.clone()]));
                let row = with(&obj, seq, Json::Arr(vec![ts[0].clone(), ts[1].clone(), bad]));
                taken = keep(&mut out, row, &alone, &other, tr!("{seq} の二つの要素が take_unique で当たり、三つ目の {fr} を範囲の外に。歩きが先に会う二件のほうを言う", "two elements of {seq} taking under a take_unique, and {fr} of the third outside its range: the two, which the walk meets first, are said"));
            }
        }
    }
    // Two sequences: a field of an element of the first outside its range, and a field of an
    // element of the second of another kind: the kind, read for every sequence before the walks.
    for s1 in 0..f.elements.len() {
        for s2 in s1 + 1..f.elements.len() {
            let (Some((_, first1)), Some((_, first2))) = (&tops[s1], &tops[s2]) else { continue };
            let (Some((fr, rv)), Some((fk, kv))) = (field_fault(s1, first1, true), field_fault(s2, first2, false)) else { continue };
            let (q1, q2) = (&f.elements[s1].name.text, &f.elements[s2].name.text);
            let both = with(&with(base, q1, Json::Arr(vec![first1.clone()])), q2, Json::Arr(vec![first2.clone()]));
            let alone = with(&both, q2, Json::Arr(vec![with(first2, &fk, kv)]));
            let other = with(&both, q1, Json::Arr(vec![with(first1, &fr, rv.clone())]));
            let obj = with(&alone, q1, Json::Arr(vec![with(first1, &fr, rv)]));
            keep(&mut out, obj, &alone, &other, tr!("{q1} の要素の {fr} を範囲の外に、{q2} の要素の {fk} を別の種類の値に。種類のほうを言う", "{fr} of an element of {q1} outside its range and {fk} of an element of {q2} of another kind: the kind is said"));
        }
    }
    out
}

/// The input `obj`, whose sequence `s` has a value worked out on each element moved past an end of
/// the range the rule accepts for it on its first element (`edge_inputs`), with that element put
/// after a good one (`good`: the first element of the vector the edge was solved from, or of the
/// first vector that has one) and again after it where the sequence can have three: the door then says the value at index 1, the first of the
/// two (§15.324). The unique fields of the two are spread apart from the good one's, where the value
/// is not worked out from them. `obj` as it is when the door says the value at index 0 of it
/// instead, and `None` when it says neither.
fn past_on_second(f: &RuleFile, c: &Checked, obj: &Json, s: usize, define: &str, good: Option<&Json>) -> Option<Json> {
    let seq = &f.elements[s].name.text;
    let xs = obj.get(seq).and_then(|v| v.as_arr()).unwrap_or_default();
    let said = |k: usize| crate::door::out_of_range(&crate::door::element(seq, k, define));
    let says = |o: &Json, k: usize| crate::door::refusal(f, c, o).is_some_and(|r| r.error == said(k));
    let cap = crate::door::count_cap(f, c, seq);
    if let (Some(past), Some(good)) = (xs.first(), good) {
        let uniques: Vec<&crate::ast::VarDecl> = f.elements[s].fields.iter().filter(|fd| fd.unique.is_some()).collect();
        for n in [3usize, 2] {
            if cap.is_some_and(|cap| cap < n as i128) {
                continue;
            }
            let mut els = vec![good.clone()];
            els.extend(std::iter::repeat_n(past.clone(), n - 1));
            let plain = with(obj, seq, Json::Arr(els.clone()));
            if says(&plain, 1) {
                return Some(plain);
            }
            // The unique fields set apart, each to values its door takes other than the good one's.
            let mut apart = true;
            for fd in &uniques {
                let ty = c.ty_of(&fd.name.text).unwrap_or(Ty::Unknown);
                let Some(vs) = spread(c, &fd.name.text, &ty, good.get(&fd.name.text), n) else {
                    apart = false;
                    break;
                };
                for (x, v) in els.iter_mut().zip(vs).skip(1) {
                    *x = with(x, &fd.name.text, v);
                }
            }
            let spread_out = with(obj, seq, Json::Arr(els));
            if apart && says(&spread_out, 1) {
                return Some(spread_out);
            }
        }
    }
    says(obj, 0).then(|| obj.clone())
}

/// `n` values of a field its door takes, each other than the rest, the value it has (`at`) first:
/// what elements side by side take in a field marked `unique` (§15.290). `None` when the field
/// does not have that many — an enum with fewer values, a truth value past two, a range too
/// narrow — or is of a type with no next value to take.
fn spread(c: &Checked, name: &str, ty: &Ty, at: Option<&Json>, n: usize) -> Option<Vec<Json>> {
    let mut out: Vec<Json> = at.into_iter().cloned().collect();
    let add = |out: &mut Vec<Json>, v: Json| {
        if out.len() < n && !out.contains(&v) {
            out.push(v);
        }
    };
    match ty.present() {
        Ty::Enum(en) => {
            let vs = c.enum_ranges.get(name).or_else(|| c.enums.get(en))?;
            for v in vs {
                add(&mut out, Json::Str(v.clone()));
            }
        }
        Ty::Bool => {
            add(&mut out, Json::Bool(false));
            add(&mut out, Json::Bool(true));
        }
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
            let sc = c.wire_scale(name);
            let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
            let (lo, hi) = (lo.map(|x| crate::types::wire_int(x, sc)), hi.map(|x| crate::types::wire_int(x, sc)));
            let n0 = at.and_then(|j| j.as_int()).or(lo).unwrap_or(0);
            let inside = |v: i128| lo.is_none_or(|l| v >= l) && hi.is_none_or(|h| v <= h);
            for k in 0..(4 * n as i128 + 2) {
                for v in [n0 + k, n0 - k] {
                    if inside(v) {
                        add(&mut out, Json::Int(v));
                    }
                }
            }
        }
        Ty::Str => {
            let s = at.and_then(|j| j.as_str()).unwrap_or("").to_string();
            for k in 1..=n {
                add(&mut out, Json::Str(format!("{s}{k}")));
            }
        }
        Ty::Date => {
            let days: Vec<i64> = match c.day_sets.get(name) {
                Some(set) => set.days.clone(),
                None => {
                    let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
                    let d0 = at.and_then(|j| j.as_str()).and_then(crate::door::civil).map(|(y, m, d)| crate::types::date_ord(y, m, d).num as i64)?;
                    (0..(2 * n as i64 + 2))
                        .flat_map(|k| [d0 + k, d0 - k])
                        .filter(|d| lo.is_none_or(|l| *d as i128 * l.den >= l.num) && hi.is_none_or(|h| *d as i128 * h.den <= h.num))
                        .collect()
                }
            };
            for d in days {
                add(&mut out, Json::Str(date_text(d as i128)));
            }
        }
        _ => {}
    }
    (out.len() == n).then_some(out)
}

/// A value of `name` the door refuses for its kind, which every way looks at as it reads the
/// argument, before any range (§15.328): the first of `wrong_values`, a value of another kind.
fn kind_fault(c: &Checked, name: &str, ty: &Ty, at: Option<&Json>) -> Option<Json> {
    wrong_values(c, name, ty, at).into_iter().next().map(|(v, _)| v)
}

/// A value of `name` of its kind, one int64 holds, that the door refuses all the same — past the
/// range it declares, or a day its koyomi date does not come to — which every way looks at once
/// every argument has been read (§15.328). `None` for a type with no such value.
fn range_fault(c: &Checked, name: &str, ty: &Ty, at: Option<&Json>) -> Option<Json> {
    let max = crate::door::WIRE_MAX;
    let vs = match ty.present() {
        Ty::Enum(en) => c.enums.get(en).cloned().unwrap_or_default(),
        _ => Vec::new(),
    };
    wrong_values(c, name, ty, at).into_iter().map(|(v, _)| v).find(|v| match (ty.present(), v) {
        // −2^63 is a value int64 holds, and outside the range ±(2^63−1) the door holds a number to.
        (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number, Json::Int(n)) => (-max - 1..=max).contains(n),
        // A day the calendar has: the one it does not have is of another kind.
        (Ty::Date, Json::Str(s)) => s != "2026-02-30",
        (Ty::Enum(_), Json::Str(s)) => vs.contains(s),
        _ => false,
    })
}

/// A whole number past what int64 holds, for a number: refused as out of range by every way as it
/// reads it, before the arguments after it (§15.298, §15.328).
fn past_fault(ty: &Ty) -> Option<Json> {
    matches!(ty.present(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number).then_some(Json::Int(crate::door::WIRE_MAX + 1))
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
            // Nearer zero than 2^52, where a float64 still holds the half: past it, a language that
            // reads a JSON number as a float64 reads -9223372036854775807.5 as the whole number
            // beside it, and the reference evaluator, which reads the digits, does not (§15.295).
            let near = (1i128 << 52) - 1;
            let half = format!("{}.5", n.clamp(-near, near));
            let why = if *present == Ty::Rate { tr!("{name} を刻みに載らない値に", "{name} off its step") } else { tr!("{name} を整数でない数に", "{name} not a whole number") };
            out.push((Json::Frac(half), why));
            // The door holds the value to the end its range writes, and to the end of its type,
            // ±(2^63−1), where the range writes none or one past it (§15.295). The one past each
            // end is a case: the least int64 below, and 2^63 above, which no int64 holds and every
            // reader refuses as out of range (§15.298).
            let max = crate::door::WIRE_MAX;
            match lo_w {
                Some(l) if l > -max => out.push((Json::Int(l - 1), tr!("{name} を範囲の下端の一つ下に", "{name} one below its range"))),
                _ => out.push((Json::Int(-max - 1), tr!("{name} を型の持てる値の下端の一つ下に", "{name} one below what its type holds"))),
            }
            match hi_w {
                Some(h) if h < max => out.push((Json::Int(h + 1), tr!("{name} を範囲の上端の一つ上に", "{name} one above its range"))),
                _ => out.push((Json::Int(max + 1), tr!("{name} を型の持てる値の上端の一つ上に", "{name} one above what its type holds"))),
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

/// A combination inside a `never` line (§15.282): each name at a value its cell takes, the first
/// combination the door refuses for this line and nothing before it. The values tried are the
/// candidates the vectors take for the name, which stand on each side of every boundary its cells
/// name.
fn inside_never(f: &RuleFile, c: &Checked, n: &crate::ast::NeverLine, base: &Json) -> Option<Json> {
    let said = crate::door::never(&n.said());
    let cands = candidates(f, c);
    let names: Vec<&String> = n.names().collect();
    let lists: Vec<Vec<Json>> = n
        .conds
        .iter()
        .map(|k| {
            let name = &k.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            cands
                .get(name)
                .into_iter()
                .flatten()
                .filter(|v| eval::cell_matches(c, &k.cell, v, &ty))
                .filter_map(|v| crate::json::parse(&wire_value(c, name, v)).ok())
                .collect()
        })
        .collect();
    if lists.iter().any(|l| l.is_empty()) {
        return None;
    }
    let mut at = vec![0usize; names.len()];
    loop {
        let mut obj = base.clone();
        for (i, name) in names.iter().enumerate() {
            obj = with(&obj, name, lists[i][at[i]].clone());
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
/// One value, in the wire form of §10.2.
pub fn wire_value(c: &Checked, name: &str, val: &Val) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
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
}

pub fn in_object(f: &RuleFile, c: &Checked, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    // A sequence is an array of objects, and each element's fields are written the same way
    // (§15.56) — the wire gains a shape, not a second kind of number.
    let scalar = |name: &str, val: &Val| -> String { wire_value(c, name, val) };
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

pub(crate) fn to_val(r: Rat, ty: &Ty) -> Val {
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
        for (_, inside, _) in crate::coverage::pairs_on(c, col, cell, &ty) {
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
    let cols = columns_at(f, c, t, a)?;
    t.rows.iter().all(|row| !row_holds_in(c, t, row, &cols)).then_some(cols)
}

/// The values the reference evaluator gives the columns of `t` for an input a caller can send:
/// every input on its step and inside its range, the `constraint`s and koyomi's days kept, and
/// every column the table reads given a value (§15.322). `None` for any other input.
pub(crate) fn columns_at(f: &RuleFile, c: &Checked, t: &Table, a: &BTreeMap<String, Val>) -> Option<BTreeMap<String, Val>> {
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
    if !sendable || !allowed(f, c, a) || !days_ok(c, a) {
        return None;
    }
    let binds = bind(f, c, a);
    t.inputs.iter().map(|(col, _)| Some((col.clone(), binds.get(col)?.clone()))).collect()
}

/// Whether the cells of `row` hold of the values of the table's columns.
pub(crate) fn row_holds_at(c: &Checked, t: &Table, row: &Row, cols: &BTreeMap<String, Val>) -> bool {
    row_holds_in(c, t, row, cols)
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
        for (_, _, outside) in crate::coverage::pairs_on(c, col, cell, &ty) {
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
        if !allowed(f, c, &a) || !days_ok(c, &a) {
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
        .filter(|&ri| !win_row(f, c, &cands, &seed, set, ri).is_some_and(|a| allowed(f, c, &a) && days_ok(c, &a)))
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
        if !allowed(f, c, &a) || !days_ok(c, &a) {
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
        allowed(f, c, a) && row_holds(f, c, t, &t.rows[i], a) && row_holds(f, c, t, &t.rows[j], a)
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
    // The calls that show a `possible` line (§15.287): every language's runner plays them. One that
    // begins where a case starts has no call to play.
    for cl in &a.possible_claims {
        if let crate::possible::Possibly::Shown(t) = &cl.verdict {
            if !t.is_empty() {
                out.traces.push(Trace { why: cl.said.clone(), steps: t.iter().map(&step_of).collect() });
            }
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
