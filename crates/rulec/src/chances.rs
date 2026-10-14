//! How likely each answer is, when the `random` inputs are drawn (§15.224, §15.225).
//!
//! An input marked `random` is drawn by the caller uniformly from what its declaration lets in —
//! the whole numbers of its range in steps of its type, an enum's values, a truth value's two —
//! independently of the other inputs, within what the `constraint` lines leave. Every chance is then
//! a count of points over a count of points, and so an exact rational.
//!
//! The rows of a table are boxes, so the points need not be looked at one by one where the rule
//! decides alike over a box. The inputs that are not random are cut into classes at the rule's own
//! boundaries — the cells `diff` and the machine walk (§15.122, §15.148) — and so are the random
//! inputs' axes. When nothing computed reads a random input together with one that is not, and no
//! `constraint` ties the two, each cell of that walk is a class of the inputs that are not random
//! crossed with a box of the random ones, and the number of points of the box is the product of the
//! lengths of its intervals. An answer that is one value over a cell counts all of them at once,
//! whatever the ranges. Only where an answer moves inside a cell, or where the two kinds of input are
//! read together, are the points run one by one, up to a budget; past it, the claim is W129 and not a
//! green it could not show.
//!
//! A pick by weight (§15.226) is counted from its weights: the drawn value that takes an element runs
//! from the total of the weights before it up to the total after it, so its count is its weight.

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::eval::{NoAnswer, Val};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use crate::vdiff::{Axis, Coord, Settled, Walker};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// How many region nodes of `--budget` one point evaluated by itself counts as: the cost of a run of
/// the rule, as for the machine's cells (§15.217). The default budget of the check lets a claim run a
/// million points.
pub const NODES_PER_POINT: usize = crate::machine::NODES_PER_CELL;

/// How many points a claim may run one by one, under `budget` (`--budget`'s nodes).
pub fn point_budget(budget: usize) -> u128 {
    (budget / NODES_PER_POINT).max(1) as u128
}

// ---------------------------------------------------------------------------------------------
// The points of a random input
// ---------------------------------------------------------------------------------------------

/// The ends of a number's points, as whole counts of its step, and the step: `range >=0 <=13` in
/// steps of 1 is (0, 13, 1).
fn number_span(c: &Checked, name: &str) -> Option<(i128, i128, i128)> {
    let (Some(lo), Some(hi)) = c.ranges.get(name).copied()? else { return None };
    let scale = *c.scales.get(name).unwrap_or(&1);
    let s = Rat::int(scale);
    let a = lo.mul(s);
    let b = hi.mul(s);
    // The first whole count at or above the low end, the last at or below the high end.
    let ceil = |x: Rat| if x.num.rem_euclid(x.den) == 0 { x.num / x.den } else { x.num.div_euclid(x.den) + 1 };
    let floor = |x: Rat| x.num.div_euclid(x.den);
    Some((ceil(a), floor(b), scale))
}

/// How many values a random input is drawn from (§15.224): the whole steps of a number's range, an
/// enum's values (those its range lets in), a truth value's two. `None` for one that cannot be drawn.
pub fn points_of(c: &Checked, name: &str) -> Option<u128> {
    match c.ty_of(name)? {
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
            let (a, b, _) = number_span(c, name)?;
            Some(if b < a { 0 } else { (b - a + 1) as u128 })
        }
        Ty::Enum(en) => Some(match c.enum_ranges.get(name) {
            Some(vs) => vs.len() as u128,
            None => c.enums.get(&en)?.len() as u128,
        }),
        Ty::Bool => Some(2),
        _ => None,
    }
}

/// The values a random input is drawn from, lowest first (an enum's in its order, `false` before
/// `true`): the order the function that counts the answers walks them in, in every language.
pub fn values_of(c: &Checked, name: &str) -> Option<Vec<Val>> {
    match c.ty_of(name)? {
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
            let (a, b, scale) = number_span(c, name)?;
            Some((a..=b).map(|k| Val::Num(Rat::new(k, scale))).collect())
        }
        Ty::Enum(en) => Some(match c.enum_ranges.get(name) {
            Some(vs) => vs.iter().map(|v| Val::Enum(v.clone())).collect(),
            None => c.enums.get(&en)?.iter().map(|v| Val::Enum(v.clone())).collect(),
        }),
        Ty::Bool => Some(vec![Val::Bool(false), Val::Bool(true)]),
        _ => None,
    }
}

/// The points of every random input together, saturating; `None` when one cannot be counted.
pub fn space(f: &RuleFile, c: &Checked) -> Option<u128> {
    f.random_inputs().try_fold(1u128, |acc, i| points_of(c, &i.name.text).map(|n| acc.saturating_mul(n)))
}

/// How the answers are counted for one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// From the weights of the rule's pick by weight (§15.226): the one random input is the value it
    /// draws, and nothing else reads it. The fold is at `item` among the rule's items.
    Weighted { item: usize, draw: String },
    /// By running the rule on every point of the random inputs.
    Points,
}

/// How the answers of `f` are counted.
pub fn method(f: &RuleFile) -> Method {
    let randoms: Vec<&str> = f.random_inputs().map(|i| i.name.text.as_str()).collect();
    let Some((item, fold)) = f.items.iter().enumerate().find_map(|(i, it)| match it {
        Item::Fold(d) if d.weighted() => Some((i, d)),
        _ => None,
    }) else {
        return Method::Points;
    };
    let Some(draw) = fold.draw.as_ref().map(|d| d.text.clone()) else { return Method::Points };
    if randoms != [draw.as_str()] {
        return Method::Points;
    }
    let elsewhere = f.items.iter().enumerate().any(|(i, it)| {
        let mut reads = crate::types::item_reads(it);
        if i == item {
            if let Some(p) = reads.iter().position(|n| *n == draw) {
                reads.remove(p);
            }
        }
        reads.contains(&draw)
    }) || f.constraints.iter().any(|k| k.names().any(|n| *n == draw))
        || f.result.as_ref().is_some_and(|r| {
            let mut ns = Vec::new();
            crate::types::collect_names(&r.expr, &mut ns);
            ns.contains(&draw)
        });
    if elsewhere { Method::Points } else { Method::Weighted { item, draw } }
}

// ---------------------------------------------------------------------------------------------
// One call's distribution, by the reference evaluator
// ---------------------------------------------------------------------------------------------

/// The answers over the points of the random inputs, for one combination of the inputs that are not
/// random (§15.225): how many points there are, and each answer — every output, in the order declared
/// — with how many points give it, in the order first met. The points are walked with the first random
/// input outermost, each from its lowest value, which is the order the generated functions walk them
/// in; a pick by weight meets its answers in the order of the elements.
#[derive(Debug, Clone, PartialEq)]
pub struct Dist {
    pub total: u128,
    pub ways: Vec<(Vec<(String, Option<Val>)>, u128)>,
    /// How many runs of the rule it took: every point of the random inputs, or for a pick by weight
    /// one run per element weighed and one for the weights.
    pub cost: u128,
}

impl Dist {
    fn add(&mut self, outs: Vec<(String, Option<Val>)>, n: u128) {
        self.total += n;
        match self.ways.iter_mut().find(|(o, _)| *o == outs) {
            Some((_, k)) => *k += n,
            None => self.ways.push((outs, n)),
        }
    }

    /// The chance that the outputs pass the cells of `row`, as `hits / total`.
    pub fn chance(&self, f: &RuleFile, c: &Checked, ch: &ChancesDecl, row: &ChanceRow) -> Option<Rat> {
        if self.total == 0 {
            return None;
        }
        let hits: u128 = self.ways.iter().filter(|(o, _)| passes(f, c, ch, row, o)).map(|(_, n)| *n).sum();
        Some(Rat::new(hits as i128, self.total as i128))
    }
}

/// Why a distribution was not worked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoDist {
    /// More points than the budget lets run.
    TooMany(u128),
    /// No point is one the door takes: the inputs that are not random are no call anybody can make.
    NoPoint,
    /// The rule contradicts itself on a point (`take_unique`).
    Contradiction,
}

/// The distribution of the answers for `fixed` — every input that is not random, and the sequences —
/// by the reference evaluator, running at most `cap` points.
pub fn distribution(f: &RuleFile, c: &Checked, fixed: &BTreeMap<String, Val>, cap: u128) -> Result<Dist, NoDist> {
    let mut d = Dist { total: 0, ways: Vec::new(), cost: 0 };
    let run = |extra: &[(String, Val)]| -> Result<Vec<(String, Option<Val>)>, NoAnswer> {
        let mut m: HashMap<String, Val> = fixed.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, v) in extra {
            m.insert(k.clone(), v.clone());
        }
        crate::eval::answer(f, c, m)
    };
    if let Method::Weighted { draw, .. } = method(f) {
        let Some((a, b, _)) = number_span(c, &draw) else { return Err(NoDist::NoPoint) };
        let at = |k: i128| vec![(draw.clone(), Val::Num(Rat::int(k)))];
        let mut base: HashMap<String, Val> = fixed.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        base.insert(draw.clone(), Val::Num(Rat::int(a)));
        let Some(cuts) = crate::eval::weighted_cuts(f, c, base) else { return Err(NoDist::NoPoint) };
        d.cost = cuts.len() as u128 + 1;
        let total = cuts.last().copied().unwrap_or_else(Rat::zero);
        if total.num == 0 {
            // The weights add up to 0: every drawn value answers `exhausted` alike.
            match run(&at(a)) {
                Ok(o) => d.add(o, (b - a + 1) as u128),
                Err(NoAnswer::Contradiction) => return Err(NoDist::Contradiction),
                Err(NoAnswer::Refused) => return Err(NoDist::NoPoint),
            }
            return Ok(d);
        }
        let mut from: i128 = 0;
        for cut in cuts {
            let to = cut.num / cut.den;
            if to > from && from <= b && from >= a {
                let n = to.min(b + 1) - from;
                match run(&at(from)) {
                    Ok(o) => d.add(o, n as u128),
                    Err(NoAnswer::Contradiction) => return Err(NoDist::Contradiction),
                    Err(NoAnswer::Refused) => {}
                }
            }
            from = from.max(to);
        }
        return if d.total == 0 { Err(NoDist::NoPoint) } else { Ok(d) };
    }
    let names: Vec<String> = f.random_inputs().map(|i| i.name.text.clone()).collect();
    let lists: Vec<Vec<Val>> = match names.iter().map(|n| values_of(c, n)).collect::<Option<Vec<_>>>() {
        Some(l) => l,
        None => return Err(NoDist::NoPoint),
    };
    let n = lists.iter().fold(1u128, |acc, l| acc.saturating_mul(l.len() as u128));
    if n > cap {
        return Err(NoDist::TooMany(n));
    }
    if lists.iter().any(|l| l.is_empty()) {
        return Err(NoDist::NoPoint);
    }
    d.cost = n;
    let mut idx = vec![0usize; lists.len()];
    loop {
        let extra: Vec<(String, Val)> = names.iter().zip(&idx).zip(&lists).map(|((n, &i), l)| (n.clone(), l[i].clone())).collect();
        match run(&extra) {
            Ok(o) => d.add(o, 1),
            Err(NoAnswer::Contradiction) => return Err(NoDist::Contradiction),
            Err(NoAnswer::Refused) => {}
        }
        // The last random input moves fastest: the first is the outermost loop.
        let mut k = idx.len();
        loop {
            if k == 0 {
                return if d.total == 0 { Err(NoDist::NoPoint) } else { Ok(d) };
            }
            k -= 1;
            idx[k] += 1;
            if idx[k] < lists[k].len() {
                break;
            }
            idx[k] = 0;
        }
    }
}

/// Whether the outputs of one answer pass the output cells of a claim's row.
pub fn passes(f: &RuleFile, c: &Checked, ch: &ChancesDecl, row: &ChanceRow, outs: &[(String, Option<Val>)]) -> bool {
    let _ = f;
    ch.outputs.iter().zip(&row.outs).all(|((name, _), cell)| {
        if matches!(cell, Cell::DontCare) {
            return true;
        }
        let Some(ty) = c.ty_of(name) else { return false };
        match outs.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.as_ref()) {
            Some(v) => crate::eval::cell_matches(c, cell, v, &ty),
            None => false,
        }
    })
}

// ---------------------------------------------------------------------------------------------
// The claims
// ---------------------------------------------------------------------------------------------

/// How one claim came out.
#[derive(Debug, Clone)]
pub enum Verdict {
    Holds,
    /// Broken at this combination of the inputs that are not random, where the chance is this.
    Broken { at: BTreeMap<String, Val>, got: Rat },
    /// Not decided within the budget, and why.
    Undecided(String),
}

/// A random box of one class, and what its points answer (§15.224): the coordinates of the random
/// axes, how many points it holds (the product of the lengths), and whether its answer passes the
/// claim.
#[derive(Debug, Clone)]
pub struct Box {
    pub coords: Vec<usize>,
    pub count: u128,
    pub outs: Vec<(String, Option<Val>)>,
    pub hit: bool,
}

/// One class of the inputs that are not random, inside a claim's row: its coordinates on the axes
/// that are not random, and how it was counted.
#[derive(Debug, Clone)]
pub struct Class {
    pub coords: Vec<usize>,
    pub how: Counted,
}

#[derive(Debug, Clone)]
pub enum Counted {
    /// Every random box answers one value over it, and holds the product of its lengths: the boxes,
    /// which tile the random axes, and the chance they come to.
    Boxes { boxes: Vec<Box>, total: u128, hits: u128 },
    /// The points were run one by one (how many, over how many combinations of the inputs that are not
    /// random): stated, not shown box by box. The chance when every combination run came to the same
    /// one, and the lowest and the highest it came to.
    Points { points: u128, chance: Option<Rat>, low: Option<Rat>, high: Option<Rat> },
    /// No input is in it: the walk showed its cells empty.
    Empty,
}

/// One claim, checked.
#[derive(Debug, Clone)]
pub struct Claim {
    /// The `chances` section, and the row (1-based) within it.
    pub table: usize,
    pub row: usize,
    pub verdict: Verdict,
    /// The chance, when it is one value over the whole row.
    pub chance: Option<Rat>,
    pub classes: Vec<Class>,
    /// How many points were run one by one for it.
    pub points: u128,
}

/// What the claims of a rule came to, with the axes they were counted on.
#[derive(Debug, Clone, Default)]
pub struct Analysis {
    /// The axes the classes and the boxes are coordinates on, when the walk was made (empty when the
    /// points were run one by one throughout).
    pub axes: Vec<Axis>,
    /// Which of `axes` are random (their columns read only the random inputs).
    pub random: Vec<usize>,
    pub claims: Vec<Claim>,
}

/// The inputs a column of the walk is worked out from.
fn roots(f: &RuleFile, name: &str, inputs: &BTreeSet<String>, seen: &mut BTreeSet<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if inputs.contains(name) {
        out.insert(name.to_string());
        return out;
    }
    if !seen.insert(name.to_string()) {
        return out;
    }
    for it in &f.items {
        let names: Vec<String> = match it {
            Item::Derived(d) if d.name.text == name => {
                let mut ns = Vec::new();
                crate::types::collect_names(&d.expr, &mut ns);
                ns
            }
            Item::Define(d) if d.name.text == name => {
                let mut ns = Vec::new();
                crate::types::collect_names(&d.expr, &mut ns);
                ns
            }
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == name) => crate::types::item_reads(it),
            _ => continue,
        };
        for n in names {
            out.extend(roots(f, &n, inputs, seen));
        }
    }
    out
}

/// Whether the rule's columns fall apart into the random and the rest (§15.224): no fold or summary,
/// no column of the walk worked out from both kinds of input, no `constraint` naming both. Then a cell
/// of the walk is a class of the inputs that are not random crossed with a box of the random ones.
fn separable(f: &RuleFile, axes: &[Axis]) -> Option<Vec<bool>> {
    if f.walks() {
        return None;
    }
    let inputs: BTreeSet<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let random: BTreeSet<String> = f.random_inputs().map(|i| i.name.text.clone()).collect();
    for k in &f.constraints {
        let r = k.names().filter(|n| random.contains(*n)).count();
        if r > 0 && r < k.names().count() {
            return None;
        }
    }
    let mut out = Vec::new();
    for ax in axes {
        let rs = roots(f, &ax.col, &inputs, &mut BTreeSet::new());
        let r = rs.iter().filter(|n| random.contains(*n)).count();
        if r > 0 && r < rs.len() {
            return None;
        }
        out.push(r > 0);
    }
    Some(out)
}

/// How many points of the axis's grid a coordinate holds.
fn coord_count(ax: &Axis, co: &Coord) -> u128 {
    match co {
        Coord::Word(_) | Coord::OtherStr => 1,
        co => {
            let (lo, hi) = crate::vdiff::ival(co, ax.step);
            let (Some(lo), Some(hi)) = (lo, hi) else { return 0 };
            let a = lo.div(ax.step);
            let b = hi.div(ax.step);
            let ceil = |x: Rat| if x.num.rem_euclid(x.den) == 0 { x.num / x.den } else { x.num.div_euclid(x.den) + 1 };
            let floor = |x: Rat| x.num.div_euclid(x.den);
            let (a, b) = (ceil(a), floor(b));
            if b < a { 0 } else { (b - a + 1) as u128 }
        }
    }
}

/// The boundaries the claims put on the inputs that are not random, so that every class is wholly
/// inside or wholly outside each row.
fn claim_cuts(f: &RuleFile, c: &Checked) -> Vec<(String, Rat)> {
    let mut out = Vec::new();
    for ch in &f.chances {
        for row in &ch.rows {
            for ((n, _), cell) in ch.inputs.iter().zip(&row.cells) {
                let Some(ty) = c.ty_of(n) else { continue };
                let lits: Vec<&Lit> = match cell {
                    Cell::Lit(l) => vec![l],
                    Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
                    Cell::Cmp(cs) => cs.iter().map(|(_, l)| l).collect(),
                    _ => Vec::new(),
                };
                for l in lits {
                    let v = match l {
                        Lit::Num(n) => crate::types::lit_value_in_pub(n, ty.present()),
                        Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                        _ => None,
                    };
                    if let Some(v) = v {
                        out.push((n.clone(), v));
                    }
                }
            }
        }
    }
    out
}

/// Whether a coordinate of an input's axis lies inside a cell of a claim (wholly, as the cuts make
/// every coordinate), and is a value the input takes at all (an enum's range).
fn coord_in(c: &Checked, ax: &Axis, ci: usize, cell: &Cell) -> bool {
    let Some(v) = crate::vdiff::coord_val(ax, ci) else { return false };
    if let (Val::Enum(w), Some(allowed)) = (&v, c.enum_ranges.get(&ax.col)) {
        if !allowed.contains(w) {
            return false;
        }
    }
    matches!(cell, Cell::DontCare) || crate::eval::cell_matches(c, cell, &v, &ax.ty)
}

/// The claims of `f`, checked within `budget` (§15.224).
pub fn analyze(f: &RuleFile, c: &Checked, budget: usize) -> Analysis {
    let mut a = Analysis::default();
    if f.chances.is_empty() || !f.has_random() {
        return a;
    }
    let cap = point_budget(budget);
    // The walk, when the columns fall apart into the random and the rest.
    let walk = if f.walks() {
        None
    } else {
        Walker::new(f, c, &claim_cuts(f, c), &BTreeSet::new()).ok().and_then(|w| {
            let kinds = separable(f, &w.axes)?;
            (w.size() <= cap).then_some((w, kinds))
        })
    };
    match walk {
        Some((mut w, kinds)) => {
            a.axes = w.axes.clone();
            a.random = kinds.iter().enumerate().filter(|(_, r)| **r).map(|(i, _)| i).collect();
            by_classes(f, c, &mut w, &kinds, cap, &mut a);
        }
        None => {
            for (ti, ch) in f.chances.iter().enumerate() {
                for row in &ch.rows {
                    a.claims.push(by_points(f, c, ti, ch, row, cap));
                }
            }
        }
    }
    a
}

/// One settled random box of one class.
struct Settle {
    coords: Vec<usize>,
    count: u128,
    /// The input realizing it and what it answers, or `None` when the walk showed it empty.
    found: Option<(HashMap<String, Val>, crate::vdiff::Answer)>,
    unknown: bool,
}

/// The claims, class by class (§15.224): each class of the inputs that are not random, crossed with
/// every box of the random axes.
fn by_classes(f: &RuleFile, c: &Checked, w: &mut Walker, kinds: &[bool], cap: u128, a: &mut Analysis) {
    let axes = w.axes.clone();
    let rand_ix: Vec<usize> = (0..axes.len()).filter(|&i| kinds[i]).collect();
    let fixed_ix: Vec<usize> = (0..axes.len()).filter(|&i| !kinds[i]).collect();
    // Counting a box by the product of its lengths asks that every random axis be an input's and that
    // no `constraint` name a random input.
    let random: BTreeSet<String> = f.random_inputs().map(|i| i.name.text.clone()).collect();
    let pure = rand_ix.iter().all(|&i| random.contains(&axes[i].col)) && !f.constraints.iter().any(|k| k.names().any(|n| random.contains(n)));
    let combos = |ix: &[usize]| -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = vec![Vec::new()];
        for &i in ix {
            let mut next = Vec::new();
            for base in &out {
                for ci in 0..axes[i].len() {
                    let mut b = base.clone();
                    b.push(ci);
                    next.push(b);
                }
            }
            out = next;
        }
        out
    };
    let classes = combos(&fixed_ix);
    let boxes = combos(&rand_ix);
    let mut spent: u128 = 0;
    // Each class, settled once for all the claims.
    let mut settled: Vec<Vec<Settle>> = Vec::new();
    for cl in &classes {
        let mut row = Vec::new();
        for bx in &boxes {
            let mut cell = vec![0usize; axes.len()];
            for (k, &i) in fixed_ix.iter().enumerate() {
                cell[i] = cl[k];
            }
            for (k, &i) in rand_ix.iter().enumerate() {
                cell[i] = bx[k];
            }
            // An enum value outside an input's range is not drawn, nor sent.
            let outside = (0..axes.len()).any(|i| match (&axes[i].coords[cell[i]], c.enum_ranges.get(&axes[i].col)) {
                (Coord::Word(wd), Some(allowed)) => !allowed.contains(wd),
                _ => false,
            });
            let count: u128 = rand_ix.iter().map(|&i| coord_count(&axes[i], &axes[i].coords[cell[i]])).product();
            if outside {
                row.push(Settle { coords: bx.clone(), count: 0, found: None, unknown: false });
                continue;
            }
            match w.settle(&cell) {
                Settled::Realized(inp, ans) => row.push(Settle { coords: bx.clone(), count, found: Some((inp, ans)), unknown: false }),
                Settled::Empty => row.push(Settle { coords: bx.clone(), count: 0, found: None, unknown: false }),
                Settled::Unknown => row.push(Settle { coords: bx.clone(), count, found: None, unknown: true }),
            }
        }
        settled.push(row);
    }
    for (ti, ch) in f.chances.iter().enumerate() {
        let claimed: Vec<String> = ch.outputs.iter().map(|(n, _)| n.clone()).collect();
        for row in &ch.rows {
            let mut claim = Claim { table: ti, row: row.index, verdict: Verdict::Holds, chance: None, classes: Vec::new(), points: 0 };
            let mut seen_chances: Vec<Rat> = Vec::new();
            let Some(test) = row.chance.clone() else {
                claim.verdict = Verdict::Undecided(String::new());
                a.claims.push(claim);
                continue;
            };
            for (k, cl) in classes.iter().enumerate() {
                // Inside the row: every input column's coordinate in its cell.
                let inside = ch.inputs.iter().zip(&row.cells).all(|((n, _), cell)| {
                    match fixed_ix.iter().position(|&i| axes[i].col == *n) {
                        Some(p) => coord_in(c, &axes[fixed_ix[p]], cl[p], cell),
                        None => true,
                    }
                });
                if !inside {
                    continue;
                }
                let subs = &settled[k];
                if subs.iter().all(|s| s.found.is_none() && !s.unknown) {
                    claim.classes.push(Class { coords: cl.clone(), how: Counted::Empty });
                    continue;
                }
                if subs.iter().any(|s| s.unknown) {
                    claim.verdict = Verdict::Undecided(tr!(
                        "乱数でない入力の区分 {} で、入力を作れず、無いことも示せなかったところがあります",
                        "in the class {} of the inputs that are not random, a cell had no input built for it and was not shown empty",
                        class_text(c, &axes, &fixed_ix, cl)
                    ));
                    break;
                }
                // Counted box by box where every answer is one value over its box.
                let open_fixed: BTreeSet<&str> = fixed_ix.iter().zip(cl).filter(|(i, ci)| matches!(axes[**i].coords[**ci], Coord::Open(_, _))).map(|(i, _)| axes[*i].col.as_str()).collect();
                let mut counted = pure;
                let mut moves_with_fixed = false;
                for s in subs {
                    let Some((_, ans)) = &s.found else { continue };
                    let mut cell = vec![0usize; axes.len()];
                    for (q, &i) in fixed_ix.iter().enumerate() {
                        cell[i] = cl[q];
                    }
                    for (q, &i) in rand_ix.iter().enumerate() {
                        cell[i] = s.coords[q];
                    }
                    if !crate::vdiff::names_constant(f, c, &axes, &cell, &ans.rows, &claimed) {
                        counted = false;
                        if !crate::vdiff::names_constant_over(f, c, &open_fixed, &ans.rows, &claimed) {
                            moves_with_fixed = true;
                        }
                    }
                }
                let witness: BTreeMap<String, Val> = subs
                    .iter()
                    .find_map(|s| s.found.as_ref())
                    .map(|(inp, _)| inp.iter().filter(|(n, _)| !random.contains(*n)).map(|(n, v)| (n.clone(), v.clone())).collect())
                    .unwrap_or_default();
                if counted {
                    let mut bxs = Vec::new();
                    let (mut total, mut hits) = (0u128, 0u128);
                    for s in subs {
                        let Some((_, ans)) = &s.found else { continue };
                        let hit = passes(f, c, ch, row, &ans.outs);
                        total += s.count;
                        if hit {
                            hits += s.count;
                        }
                        bxs.push(Box { coords: s.coords.clone(), count: s.count, outs: ans.outs.clone(), hit });
                    }
                    claim.classes.push(Class { coords: cl.clone(), how: Counted::Boxes { boxes: bxs, total, hits } });
                    if total == 0 {
                        continue;
                    }
                    let p = Rat::new(hits as i128, total as i128);
                    if !seen_chances.contains(&p) {
                        seen_chances.push(p);
                    }
                    if !test.holds(p) {
                        claim.verdict = Verdict::Broken { at: witness, got: p };
                        break;
                    }
                    continue;
                }
                // Run the points of the class one by one: once, with one combination of the inputs that
                // are not random, when what moves inside the class moves with the random inputs alone.
                let reps: Vec<BTreeMap<String, Val>> = if moves_with_fixed || !pure_counts(f, &random) {
                    match class_points(f, c, &axes, &fixed_ix, cl, cap.saturating_sub(spent)) {
                        Some(xs) => xs,
                        None => {
                            claim.verdict = Verdict::Undecided(tr!(
                                "乱数でない入力の区分 {} の点が予算（{cap} 点）に収まりません",
                                "the points of the class {} of the inputs that are not random do not fit the budget ({cap} points)",
                                class_text(c, &axes, &fixed_ix, cl)
                            ));
                            break;
                        }
                    }
                } else {
                    vec![witness.clone()]
                };
                let mut run = 0u128;
                let mut here: Vec<Rat> = Vec::new();
                let mut stop = false;
                for x in reps {
                    match distribution(f, c, &x, cap.saturating_sub(spent)) {
                        Ok(d) => {
                            spent += d.cost.max(1);
                            run += d.cost;
                            let Some(p) = d.chance(f, c, ch, row) else { continue };
                            if !seen_chances.contains(&p) {
                                seen_chances.push(p);
                            }
                            if !here.contains(&p) {
                                here.push(p);
                            }
                            if !test.holds(p) {
                                claim.verdict = Verdict::Broken { at: x.clone(), got: p };
                                stop = true;
                                break;
                            }
                        }
                        Err(NoDist::NoPoint) => {}
                        Err(NoDist::TooMany(n)) => {
                            claim.verdict = Verdict::Undecided(tr!(
                                "乱数でない入力の区分 {} で、乱数の入力の点 {n} が予算（{cap} 点）に収まりません",
                                "in the class {} of the inputs that are not random, the {n} points of the random inputs do not fit the budget ({cap} points)",
                                class_text(c, &axes, &fixed_ix, cl)
                            ));
                            stop = true;
                            break;
                        }
                        Err(NoDist::Contradiction) => {
                            claim.verdict = Verdict::Undecided(tr!("規則が矛盾する点があります", "the rule contradicts itself on a point"));
                            stop = true;
                            break;
                        }
                    }
                    if spent > cap {
                        claim.verdict = Verdict::Undecided(tr!("点が予算（{cap} 点）を越えました", "the points went past the budget ({cap} points)"));
                        stop = true;
                        break;
                    }
                }
                claim.points += run;
                claim.classes.push(Class { coords: cl.clone(), how: points_counted(run, &here) });
                if stop {
                    break;
                }
            }
            if matches!(claim.verdict, Verdict::Holds) && seen_chances.len() == 1 {
                claim.chance = seen_chances.first().copied();
            }
            a.claims.push(claim);
        }
    }
}

/// Whether the number of points a combination of the inputs that are not random leaves is the same
/// for all of them: no `constraint` names a random input.
fn pure_counts(f: &RuleFile, random: &BTreeSet<String>) -> bool {
    !f.constraints.iter().any(|k| k.names().any(|n| random.contains(n)))
}

/// A class written for a message: each axis that is not random, and its coordinate.
fn class_text(c: &Checked, axes: &[Axis], fixed_ix: &[usize], cl: &[usize]) -> String {
    let parts: Vec<String> = fixed_ix
        .iter()
        .zip(cl)
        .filter_map(|(&i, &ci)| {
            let ax = &axes[i];
            let shown = crate::machine::coord_text(ax, ci);
            (ax.len() > 1).then(|| format!("{} {}", ax.col, shown))
        })
        .collect();
    let _ = c;
    if parts.is_empty() { "-".into() } else { parts.join(", ") }
}

/// Every combination of the inputs that are not random inside one class, when there are at most `cap`
/// of them: the whole steps of each input axis's coordinate. A class with a column worked out from
/// the inputs is held to it by the reference evaluator.
fn class_points(f: &RuleFile, c: &Checked, axes: &[Axis], fixed_ix: &[usize], cl: &[usize], cap: u128) -> Option<Vec<BTreeMap<String, Val>>> {
    let inputs: BTreeSet<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let mut lists: Vec<(String, Vec<Val>)> = Vec::new();
    let mut n: u128 = 1;
    for (&i, &ci) in fixed_ix.iter().zip(cl) {
        let ax = &axes[i];
        if !inputs.contains(&ax.col) {
            continue;
        }
        let vals: Vec<Val> = match &ax.coords[ci] {
            Coord::Word(_) | Coord::OtherStr => vec![crate::vdiff::coord_val(ax, ci)?],
            co => {
                let k = coord_count(ax, co);
                if k > cap {
                    return None;
                }
                let (lo, _) = crate::vdiff::ival(co, ax.step);
                let lo = lo?;
                let start = {
                    let x = lo.div(ax.step);
                    if x.num.rem_euclid(x.den) == 0 { x.num / x.den } else { x.num.div_euclid(x.den) + 1 }
                };
                (0..k as i128).map(|j| crate::vdiff::val_of(ax, ax.step.mul(Rat::int(start + j)))).collect()
            }
        };
        n = n.saturating_mul(vals.len() as u128);
        if n > cap {
            return None;
        }
        lists.push((ax.col.clone(), vals));
    }
    let mut out: Vec<BTreeMap<String, Val>> = vec![BTreeMap::new()];
    for (name, vals) in lists {
        let mut next = Vec::new();
        for base in &out {
            for v in &vals {
                let mut m = base.clone();
                m.insert(name.clone(), v.clone());
                next.push(m);
            }
        }
        out = next;
    }
    // The derived columns of the class hold the point to it.
    let derived: Vec<usize> = fixed_ix.iter().copied().filter(|&i| !inputs.contains(&axes[i].col)).collect();
    if !derived.is_empty() {
        out.retain(|x| {
            let (_, _, _, binds) = crate::eval::run_all_traced(f, c, x.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
            derived.iter().all(|&i| {
                let p = fixed_ix.iter().position(|&j| j == i).unwrap_or(0);
                binds.get(&axes[i].col).is_some_and(|v| crate::vdiff::holds(&axes[i], cl[p], v))
            })
        });
    }
    Some(out)
}

/// The values an input that is not random takes inside a cell of a claim, when they are few enough:
/// an enum's values and a truth value's two that the cell lets in, a number's whole steps between the
/// ends of its range and of the cell.
fn input_values(c: &Checked, name: &str, cell: &Cell, cap: u128) -> Result<Vec<Val>, String> {
    let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
    let keep = |v: &Val| matches!(cell, Cell::DontCare) || crate::eval::cell_matches(c, cell, v, &ty);
    let mut vals: Vec<Val> = Vec::new();
    if let Ty::Opt(_) = ty {
        let none = Val::Enum(crate::kw::NONE.into());
        if keep(&none) {
            vals.push(none);
        }
    }
    match ty.present() {
        Ty::Enum(en) => {
            let all = match c.enum_ranges.get(name) {
                Some(vs) => vs.clone(),
                None => c.enums.get(en).cloned().unwrap_or_default(),
            };
            vals.extend(all.into_iter().map(Val::Enum).filter(|v| keep(v)));
        }
        Ty::Bool => vals.extend([Val::Bool(false), Val::Bool(true)].into_iter().filter(|v| keep(v))),
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
            let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
            // The cell's own ends narrow it.
            let (mut lo, mut hi) = (lo, hi);
            if let Cell::Cmp(cs) = cell {
                for (op, l) in cs {
                    let v = match l {
                        Lit::Num(n) => crate::types::lit_value_in_pub(n, ty.present()),
                        Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                        _ => None,
                    };
                    let Some(v) = v else { continue };
                    match op {
                        CmpOp::Ge | CmpOp::Gt => lo = Some(lo.map_or(v, |x| if v.cmp_to(x) == Ordering::Greater { v } else { x })),
                        CmpOp::Le | CmpOp::Lt => hi = Some(hi.map_or(v, |x| if v.cmp_to(x) == Ordering::Less { v } else { x })),
                    }
                }
            }
            if let Cell::Lit(Lit::Num(n)) = cell {
                if let Some(v) = crate::types::lit_value_in_pub(n, ty.present()) {
                    lo = Some(v);
                    hi = Some(v);
                }
            }
            if let Cell::Set(ls) = cell {
                for l in ls {
                    let v = match l {
                        Lit::Num(n) => crate::types::lit_value_in_pub(n, ty.present()).map(Val::Num),
                        Lit::Date(y, m, d) => Some(Val::Date(*y, *m, *d)),
                        _ => None,
                    };
                    if let Some(v) = v {
                        vals.push(v);
                    }
                }
                return Ok(vals);
            }
            let (Some(lo), Some(hi)) = (lo, hi) else {
                return Err(tr!("{name} に範囲の端が無いので、値を一つずつ挙げられません", "{name} has no end to its range, so its values cannot be listed"));
            };
            let step = if matches!(ty.present(), Ty::Rate) { Rat::new(1, *c.scales.get(name).unwrap_or(&100)) } else { Rat::int(1) };
            let a = lo.div(step);
            let b = hi.div(step);
            let a = if a.num.rem_euclid(a.den) == 0 { a.num / a.den } else { a.num.div_euclid(a.den) + 1 };
            let b = b.num.div_euclid(b.den);
            if b >= a && (b - a + 1) as u128 > cap {
                return Err(tr!("{name} の値が予算（{cap} 点）を越えます", "the values of {name} go past the budget ({cap} points)"));
            }
            for k in a..=b {
                let r = step.mul(Rat::int(k));
                let v = if matches!(ty.present(), Ty::Date) {
                    let (y, m, d) = crate::types::ord_to_date(r);
                    Val::Date(y, m, d)
                } else {
                    Val::Num(r)
                };
                if keep(&v) {
                    vals.push(v);
                }
            }
        }
        _ => return Err(tr!("{name} の値は一つずつ挙げられません", "the values of {name} cannot be listed one by one")),
    }
    Ok(vals)
}

/// One claim with the points run one by one (§15.224): every combination of the inputs that are not
/// random its cells let in — and the sequence it names, for a rule that walks — with the distribution of
/// each, up to `cap` points in all.
fn by_points(f: &RuleFile, c: &Checked, ti: usize, ch: &ChancesDecl, row: &ChanceRow, cap: u128) -> Claim {
    let mut claim = Claim { table: ti, row: row.index, verdict: Verdict::Holds, chance: None, classes: Vec::new(), points: 0 };
    let Some(test) = row.chance.clone() else {
        claim.verdict = Verdict::Undecided(String::new());
        return claim;
    };
    let seqs: Vec<&str> = f.elements.iter().map(|e| e.name.text.as_str()).collect();
    let mut lists: Vec<(String, Vec<Val>)> = Vec::new();
    let mut n: u128 = 1;
    for ((name, _), cell) in ch.inputs.iter().zip(&row.cells) {
        if seqs.contains(&name.as_str()) {
            let v = match cell {
                Cell::Lit(Lit::Word(w)) => crate::eval::seq_value(f, c, w),
                _ => None,
            };
            match v {
                Some(v) => lists.push((name.clone(), vec![v])),
                None => {
                    claim.verdict = Verdict::Undecided(tr!("並び {name} が読めません", "the sequence {name} cannot be read"));
                    return claim;
                }
            }
            continue;
        }
        match input_values(c, name, cell, cap) {
            Ok(vs) => {
                n = n.saturating_mul(vs.len() as u128);
                lists.push((name.clone(), vs));
            }
            Err(why) => {
                claim.verdict = Verdict::Undecided(why);
                return claim;
            }
        }
    }
    // Every combination of the inputs that are not random, each with every point of the random ones
    // (or, for a pick by weight, a run per element): known before a point is run.
    let per = match method(f) {
        Method::Weighted { .. } => 1,
        Method::Points => space(f, c).unwrap_or(u128::MAX),
    };
    if n.saturating_mul(per) > cap {
        claim.verdict = Verdict::Undecided(if per > 1 {
            tr!(
                "乱数でない入力の組み合わせ {n} 通りに、乱数の入力の点 {per} をかけると、予算（{cap} 点）を越えます",
                "the {n} combinations of the inputs that are not random, times the {per} points of the random inputs, go past the budget ({cap} points)"
            )
        } else {
            tr!(
                "乱数でない入力の組み合わせ {n} 通りが予算（{cap} 点）を越えます",
                "the {n} combinations of the inputs that are not random go past the budget ({cap} points)"
            )
        });
        return claim;
    }
    let mut xs: Vec<BTreeMap<String, Val>> = vec![BTreeMap::new()];
    for (name, vals) in lists {
        let mut next = Vec::new();
        for base in &xs {
            for v in &vals {
                let mut m = base.clone();
                m.insert(name.clone(), v.clone());
                next.push(m);
            }
        }
        xs = next;
    }
    let mut spent: u128 = 0;
    let mut seen: Vec<Rat> = Vec::new();
    for x in xs {
        match distribution(f, c, &x, cap.saturating_sub(spent)) {
            Ok(d) => {
                spent += d.cost.max(1);
                claim.points += d.cost;
                let Some(p) = d.chance(f, c, ch, row) else { continue };
                if !seen.contains(&p) {
                    seen.push(p);
                }
                if !test.holds(p) {
                    claim.verdict = Verdict::Broken { at: x, got: p };
                    return claim;
                }
            }
            Err(NoDist::NoPoint) => {}
            Err(NoDist::TooMany(m)) => {
                claim.verdict = Verdict::Undecided(tr!(
                    "乱数の入力の点 {m} が、残りの予算（{} 点）に収まりません",
                    "the {m} points of the random inputs do not fit what is left of the budget ({} points)",
                    cap.saturating_sub(spent)
                ));
                return claim;
            }
            Err(NoDist::Contradiction) => {
                claim.verdict = Verdict::Undecided(tr!("規則が矛盾する点があります", "the rule contradicts itself on a point"));
                return claim;
            }
        }
        if spent > cap {
            claim.verdict = Verdict::Undecided(tr!("点が予算（{cap} 点）を越えました", "the points went past the budget ({cap} points)"));
            return claim;
        }
    }
    claim.classes.push(Class { coords: Vec::new(), how: points_counted(claim.points, &seen) });
    if seen.len() == 1 {
        claim.chance = seen.first().copied();
    }
    claim
}

/// How the points run one by one came out: `run` of them, with the chances `seen` (each once).
fn points_counted(run: u128, seen: &[Rat]) -> Counted {
    let mut s = seen.to_vec();
    s.sort_by(|a, b| a.cmp_to(*b));
    Counted::Points { points: run, chance: if s.len() == 1 { s.first().copied() } else { None }, low: s.first().copied(), high: s.last().copied() }
}

/// A chance as it is read: `5/14`, `0`, `1`.
pub fn chance_text(p: Rat) -> String {
    if p.den == 1 { format!("{}", p.num) } else { format!("{}/{}", p.num, p.den) }
}

/// The chance as a percentage with one decimal, for the page and the messages: `35.7%`.
pub fn percent_text(p: Rat) -> String {
    let tenths = Rat::new(p.num * 1000, p.den);
    let q = tenths.num.div_euclid(tenths.den);
    let r = tenths.num.rem_euclid(tenths.den);
    let q = if 2 * r >= tenths.den { q + 1 } else { q };
    format!("{}.{}%", q / 10, q % 10)
}

/// The values of the inputs that are not random, for a message, in the order the rule declares them:
/// `plans_left = true, escort_fits = false`.
fn at_text(f: &RuleFile, c: &Checked, at: &BTreeMap<String, Val>, ch: &ChancesDecl, row: &ChanceRow) -> String {
    let order: Vec<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).chain(f.elements.iter().map(|e| e.name.text.as_str())).collect();
    let mut keys: Vec<&String> = at.keys().collect();
    keys.sort_by_key(|k| order.iter().position(|o| o == k).unwrap_or(usize::MAX));
    if keys.is_empty() {
        return "-".into();
    }
    keys.into_iter()
        .map(|k| match &at[k] {
            // A sequence is said by the name of the `sequence` the row names.
            v @ Val::Seq(_) => match ch.inputs.iter().position(|(n, _)| n == k).and_then(|i| row.cells.get(i)) {
                Some(Cell::Lit(Lit::Word(w))) => format!("{k} = {w}"),
                _ => format!("{k} = {}", v.show(&Ty::Unknown)),
            },
            v => format!("{k} = {}", v.show(&c.ty_of(k).unwrap_or(Ty::Unknown))),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The output cells of a claim's row, for a message: `find = coins`.
fn outs_text(ch: &ChancesDecl, row: &ChanceRow) -> String {
    let parts: Vec<String> = ch
        .outputs
        .iter()
        .zip(&row.outs)
        .filter(|(_, cell)| !matches!(cell, Cell::DontCare))
        .map(|((n, _), cell)| match cell {
            Cell::Lit(l) => format!("{n} = {}", crate::ast::lit_text(l)),
            other => format!("{n} {}", crate::ast::cell_text(other)),
        })
        .collect();
    if parts.is_empty() { tr!("どの答えでも", "any answer") } else { parts.join(", ") }
}

/// E133 and W129 (§15.224): the claims that are broken, and the ones the budget did not decide.
pub fn check(f: &RuleFile, c: &Checked, path: &str, budget: usize) -> Vec<Diag> {
    let a = analyze(f, c, budget);
    let mut out = Vec::new();
    for cl in &a.claims {
        let ch = &f.chances[cl.table];
        let Some(row) = ch.rows.iter().find(|r| r.index == cl.row) else { continue };
        let at = tr!("{path}:{} 確率の主張", "{path}:{} chances", row.span.line);
        match &cl.verdict {
            Verdict::Holds => {}
            Verdict::Broken { at: x, got } => {
                let mut d = Diag::error("E133", tr!("確率の主張が成り立ちません（行 {}）", "A claim of `chances` does not hold (row {})", row.index))
                    .at(at)
                    .mark(row.chance_span.clone(), tr!("{} と書かれています", "{} is written here", row.chance_text))
                    .note(if x.is_empty() {
                        // No input that is not random: the chance is the rule's alone.
                        tr!("{} の確率は {}（{}）です。", "The chance of {} is {} ({}).", outs_text(ch, row), chance_text(*got), percent_text(*got))
                    } else {
                        tr!(
                            "{} のとき、{} の確率は {}（{}）です。",
                            "With {}, the chance of {} is {} ({}).",
                            at_text(f, c, x, ch, row),
                            outs_text(ch, row),
                            chance_text(*got),
                            percent_text(*got)
                        )
                    })
                    .note(tr!(
                        "主張は、行の入力のセルに当てはまるどの組み合わせでも、乱数の入力を一様に引いたときの確率です。表が正しいなら主張を直してください。主張のほうが正しいなら、表の行を直してください。",
                        "A claim speaks of every combination its cells let in, with the random inputs drawn uniformly. If the table is right, correct the claim; if the claim is the business truth, correct the table's rows."
                    ));
                for (k, v) in x {
                    if !matches!(v, Val::Seq(_)) {
                        d = d.win(k.clone(), crate::eval::wval(c, k, v));
                    }
                }
                out.push(d);
            }
            Verdict::Undecided(why) if why.is_empty() => {}
            Verdict::Undecided(why) => out.push(
                Diag::warning("W129", tr!("確率の主張を予算の中で決められませんでした（行 {}）", "A claim of `chances` could not be decided within the budget (row {})", row.index))
                    .at(at)
                    .mark(row.span.clone(), why.clone())
                    .note(tr!(
                        "答えが乱数の入力の式で区分の中で変わるところと、乱数の入力と乱数でない入力を一緒に読むところは、点を一つずつ評価します。主張一つに使える点は予算（`--budget` の {} 分の 1）までです。",
                        "Where an answer moves inside a cell with the random inputs, and where the random inputs are read together with the others, the points are run one by one, up to the budget for one claim (one {}th of `--budget`'s nodes).",
                        NODES_PER_POINT
                    ))
                    .note(tr!(
                        "行の乱数でない入力のセルを狭めるか、`--budget` を上げてください。決められない主張は、成り立つとも成り立たないとも言いません。",
                        "Narrow the cells of the inputs that are not random, or raise `--budget`. A claim that is not decided is said to be neither holding nor broken."
                    )),
            ),
        }
    }
    out
}

/// The span of the first random input's mark, for the messages that point at what is drawn.
pub fn random_span(f: &RuleFile) -> Option<Span> {
    f.random_inputs().next().and_then(|i| i.random.clone())
}

// ---------------------------------------------------------------------------------------------
// What the generated functions are held to
// ---------------------------------------------------------------------------------------------

/// How many lines of inputs the functions that count the answers are asked about, at most.
pub const VECTOR_LINES: usize = 24;

/// How many points the reference may run, over every line, to write what the functions have to
/// answer.
pub const VECTOR_POINTS: u128 = 2_000_000;

/// What each language's function that counts the answers has to give (§15.225): the lines of
/// `vectors/<alias>.chances.jsonl`, one per combination of the inputs that are not random met in
/// the vectors (the random inputs at the values the vector has, which the function does not read),
/// and of `vectors/<alias>.chances.expected`, where each line's answer is a line `{"total":N}` and
/// then a line per answer, the count, a tab, and the outputs as a record's `observed` writes them.
/// `None` for a rule with no random input, or when no line could be worked out within the budget.
pub fn vector_files(f: &RuleFile, c: &Checked, vs: &[crate::vectors::Vector]) -> Option<(String, String)> {
    if !f.has_random() {
        return None;
    }
    let random: BTreeSet<String> = f.random_inputs().map(|i| i.name.text.clone()).collect();
    let mut seen: Vec<BTreeMap<String, Val>> = Vec::new();
    let (mut input, mut expected) = (String::new(), String::new());
    let mut spent: u128 = 0;
    for v in vs {
        if seen.len() >= VECTOR_LINES {
            break;
        }
        let fixed: BTreeMap<String, Val> = v.input.iter().filter(|(k, _)| !random.contains(*k)).map(|(k, v)| (k.clone(), v.clone())).collect();
        if seen.contains(&fixed) {
            continue;
        }
        let Ok(d) = distribution(f, c, &fixed, VECTOR_POINTS.saturating_sub(spent)) else { continue };
        spent += d.cost;
        seen.push(fixed);
        input.push_str(&crate::vectors::to_json(f, c, v));
        input.push('\n');
        expected.push_str(&format!("{{\"total\":{}}}\n", d.total));
        for (outs, n) in &d.ways {
            expected.push_str(&format!("{n}\t{}\n", crate::vectors::observed_json(c, outs)));
        }
    }
    (!seen.is_empty()).then_some((input, expected))
}

// ---------------------------------------------------------------------------------------------
// The certificate
// ---------------------------------------------------------------------------------------------

/// A rational as the certificate writes it: `"7/2"`, or `"3"` when it is whole.
fn rat_text(r: &Rat) -> String {
    if r.den == 1 { r.num.to_string() } else { format!("{}/{}", r.num, r.den) }
}

/// A value as the certificate writes it: a number in its column's unit as a rational, a date as its
/// day number, a word, a truth value as `"true"` or `"false"`.
fn value_json(v: &Option<Val>) -> String {
    match v {
        None => "null".into(),
        Some(Val::Num(r)) => crate::json::quote(&rat_text(r)),
        Some(Val::Date(y, m, d)) => crate::json::quote(&rat_text(&crate::types::date_ord(*y, *m, *d))),
        Some(Val::Bool(b)) => crate::json::quote(if *b { crate::kw::TRUE } else { crate::kw::FALSE }),
        Some(Val::Enum(s)) | Some(Val::Str(s)) => crate::json::quote(s),
        Some(Val::Seq(xs)) => crate::json::quote(&format!("{} elements", xs.len())),
    }
}

/// One cell of a claim, for its column's type, as the certificate writes it: `{"any":true}`,
/// `{"nothing":true}`, `{"words":[…]}`, `{"not_words":[…]}`, `{"nums":[…]}`, `{"not_nums":[…]}`,
/// `{"cmp":[[op, value], …]}`, `{"strs":[…]}`, `{"prefix":[…]}` — numbers in the column's unit.
fn cell_json(cell: &Cell, ty: &Ty) -> String {
    use crate::json::{quote, strs, Obj};
    let num = |l: &Lit| -> Option<String> {
        match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty.present()).map(|r| rat_text(&r)),
            Lit::Date(y, m, d) => Some(rat_text(&crate::types::date_ord(*y, *m, *d))),
            _ => None,
        }
    };
    let word = |l: &Lit| -> Option<String> {
        match l {
            Lit::Word(w) => Some(w.clone()),
            _ => None,
        }
    };
    let set = |ls: &[Lit], words: &str, nums: &str| -> String {
        if let Some(ws) = ls.iter().map(word).collect::<Option<Vec<_>>>() {
            return Obj::new().raw(words, strs(&ws)).finish();
        }
        if let Some(ns) = ls.iter().map(num).collect::<Option<Vec<_>>>() {
            return Obj::new().raw(nums, strs(&ns)).finish();
        }
        let ss: Vec<String> = ls.iter().filter_map(|l| if let Lit::Str(s) = l { Some(s.clone()) } else { None }).collect();
        Obj::new().raw("strs", strs(&ss)).finish()
    };
    match cell {
        Cell::DontCare => "{\"any\":true}".into(),
        Cell::Nothing => "{\"nothing\":true}".into(),
        Cell::Lit(l) => set(std::slice::from_ref(l), "words", "nums"),
        Cell::Set(ls) => set(ls, "words", "nums"),
        Cell::Not(ls) => set(ls, "not_words", "not_nums"),
        Cell::Ranked { values, .. } => Obj::new().raw("words", strs(values)).finish(),
        Cell::Cmp(cs) => {
            let parts: Vec<String> = cs.iter().filter_map(|(op, l)| num(l).map(|v| format!("[{},{}]", quote(op.word()), quote(&v)))).collect();
            format!("{{\"cmp\":[{}]}}", parts.join(","))
        }
        Cell::Prefix(ps) => Obj::new().raw("prefix", strs(ps)).finish(),
    }
}

/// A coordinate of an axis as the certificate writes it: `{"word":w}`, `{"other":true}`,
/// `{"point":v}`, or `{"open":[lo, hi]}` with the ends left out (exclusive) and `null` for none.
fn coord_json(co: &Coord) -> String {
    match co {
        Coord::Word(w) => format!("{{\"word\":{}}}", crate::json::quote(w)),
        Coord::OtherStr => "{\"other\":true}".into(),
        Coord::Point(r) => format!("{{\"point\":{}}}", crate::json::quote(&rat_text(r))),
        Coord::Open(lo, hi) => {
            let e = |x: &Option<Rat>| x.map(|r| crate::json::quote(&rat_text(&r))).unwrap_or_else(|| "null".into());
            format!("{{\"open\":[{},{}]}}", e(lo), e(hi))
        }
    }
}

/// The claims of `chances` and how each was counted (§15.224), for the certificate: the random
/// inputs and the points each is drawn from; the axes of the walk, when the classes were counted on
/// it; and per claim its cells and chance, and per class of the inputs that are not random inside it,
/// the random boxes that tile the random axes, each with its number of points, what its points
/// answer and whether that passes the claim — or that the class holds no input, or that its points
/// were run one by one (counted, and stated). `None` for a rule with no `chances`, so no other
/// certificate changes.
pub fn certificate_json(f: &RuleFile, c: &Checked) -> Option<String> {
    use crate::json::{arr, quote, Obj};
    if f.chances.is_empty() || !f.has_random() {
        return None;
    }
    let a = analyze(f, c, crate::region::DEFAULT_BUDGET.max(0) as usize);
    let random: Vec<String> = f
        .random_inputs()
        .map(|i| {
            let o = Obj::new().str("name", &i.name.text).raw("points", quote(&points_of(c, &i.name.text).unwrap_or(0).to_string()));
            match c.ty_of(&i.name.text) {
                Some(Ty::Enum(_)) | Some(Ty::Bool) => {
                    let vs: Vec<String> = values_of(c, &i.name.text).unwrap_or_default().iter().map(|v| match v {
                        Val::Bool(b) => if *b { crate::kw::TRUE.to_string() } else { crate::kw::FALSE.to_string() },
                        Val::Enum(s) => s.clone(),
                        _ => String::new(),
                    }).collect();
                    o.raw("values", crate::json::strs(&vs)).finish()
                }
                _ => {
                    let (lo, hi) = c.ranges.get(&i.name.text).copied().unwrap_or((None, None));
                    let e = |x: Option<Rat>| x.map(|r| quote(&rat_text(&r))).unwrap_or_else(|| "null".into());
                    o.raw("range", format!("[{},{}]", e(lo), e(hi))).int("scale", *c.scales.get(&i.name.text).unwrap_or(&1)).finish()
                }
            }
        })
        .collect();
    let axes: Vec<String> = a
        .axes
        .iter()
        .enumerate()
        .map(|(k, ax)| {
            Obj::new()
                .str("column", &ax.col)
                .bool("random", a.random.contains(&k))
                .str("type", &ax.ty.to_string())
                .str("step", &rat_text(&ax.step))
                .raw("coords", arr(&ax.coords.iter().map(coord_json).collect::<Vec<_>>()))
                .finish()
        })
        .collect();
    let fixed_ix: Vec<usize> = (0..a.axes.len()).filter(|k| !a.random.contains(k)).collect();
    let at_json = |ix: &[usize], coords: &[usize]| -> String { arr(&ix.iter().zip(coords).map(|(i, ci)| format!("[{i},{ci}]")).collect::<Vec<_>>()) };
    let mut claims = Vec::new();
    for cl in &a.claims {
        let ch = &f.chances[cl.table];
        let Some(row) = ch.rows.iter().find(|r| r.index == cl.row) else { continue };
        let cells = |cols: &[(String, crate::diag::Span)], cs: &[Cell]| -> String {
            arr(&cols
                .iter()
                .zip(cs)
                .map(|((n, _), cell)| {
                    let ty = c.ty_of(n).unwrap_or(Ty::Unknown);
                    let seq = f.elements.iter().any(|e| e.name.text == *n);
                    let body = match (seq, cell) {
                        (true, Cell::Lit(Lit::Word(w))) => format!("{{\"sequence\":{}}}", quote(w)),
                        _ => cell_json(cell, &ty),
                    };
                    Obj::new().str("column", n).raw("cell", body).finish()
                })
                .collect::<Vec<_>>())
        };
        let chance = match &row.chance {
            Some(ChanceTest::Is(v)) => format!("{{\"is\":{}}}", quote(&rat_text(v))),
            Some(ChanceTest::Cmp(cs)) => format!("{{\"cmp\":[{}]}}", cs.iter().map(|(op, v)| format!("[{},{}]", quote(op.word()), quote(&rat_text(v)))).collect::<Vec<_>>().join(",")),
            None => "null".into(),
        };
        let random_ix: Vec<usize> = a.random.clone();
        let classes: Vec<String> = cl
            .classes
            .iter()
            .map(|k| {
                let o = Obj::new().raw("at", at_json(&fixed_ix, &k.coords));
                match &k.how {
                    Counted::Empty => o.bool("empty", true).finish(),
                    Counted::Points { points, chance, low, high } => o
                        .raw("run", quote(&points.to_string()))
                        .raw("chance", chance.map(|p| quote(&rat_text(&p))).unwrap_or_else(|| "null".into()))
                        .raw("low", low.map(|p| quote(&rat_text(&p))).unwrap_or_else(|| "null".into()))
                        .raw("high", high.map(|p| quote(&rat_text(&p))).unwrap_or_else(|| "null".into()))
                        .finish(),
                    Counted::Boxes { boxes, total, hits } => {
                        let bs: Vec<String> = boxes
                            .iter()
                            .map(|b| {
                                let mut outs = Obj::new();
                                for (n, v) in &b.outs {
                                    outs = outs.raw(n, value_json(v));
                                }
                                Obj::new()
                                    .raw("at", at_json(&random_ix, &b.coords))
                                    .raw("points", quote(&b.count.to_string()))
                                    .raw("outputs", outs.finish())
                                    .bool("hit", b.hit)
                                    .finish()
                            })
                            .collect();
                        o.raw("boxes", arr(&bs)).raw("points", quote(&total.to_string())).raw("hits", quote(&hits.to_string())).finish()
                    }
                }
            })
            .collect();
        let verdict = match &cl.verdict {
            Verdict::Holds => "holds",
            Verdict::Broken { .. } => "broken",
            Verdict::Undecided(_) => "undecided",
        };
        claims.push(
            Obj::new()
                .int("chances", cl.table as i128)
                .int("row", cl.row as i128)
                .raw("inputs", cells(&ch.inputs, &row.cells))
                .raw("outputs", cells(&ch.outputs, &row.outs))
                .raw("chance", chance)
                .str("verdict", verdict)
                .raw("classes", arr(&classes))
                .finish(),
        );
    }
    Some(Obj::new().raw("random", arr(&random)).raw("axes", arr(&axes)).raw("claims", arr(&claims)).finish())
}
