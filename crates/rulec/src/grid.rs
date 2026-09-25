//! The arithmetic of grids (§15.151–§15.153): which values an output can take, on which grid,
//! and whether half a step off that grid — the rounding tie — is ever reached.
//!
//! The coverage audit asks two questions of the rule itself, before any vector exists: does an
//! output owe a rounding tie, and can a computed value be seen at two values. Answering "no"
//! takes an argument, and the argument here is arithmetic: a value, taken row by row, is
//! `Σ aᵥ·v + k` with each `v` a multiple of its own step inside the range the row allows. Where
//! the arithmetic cannot settle it and the inputs involved are few, every one of them is
//! evaluated instead. Where neither settles it, the answer is "yes, owed", never "no".

use crate::ast::*;
use crate::coverage::{bound, leaves, reads_of, value_moved};
use crate::eval::{self, Val};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// A value as the grid argument reads it: `Σ aᵥ·v + k`, each `v` ranging over the multiples
/// of `steps[v]`, inside `ranges[v]` where that is known. A name that starts with `#` stands for
/// a value no input sets directly: one the rule rounded on its way (`allocate`, a rounding
/// function), or the product of two names. A derived column a table tests is one name too, on
/// the grid its expression lands on (§15.153).
#[derive(Clone)]
pub struct GridForm {
    pub terms: BTreeMap<String, Rat>,
    pub steps: BTreeMap<String, Rat>,
    pub ranges: BTreeMap<String, (Rat, Rat)>,
    pub k: Rat,
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

    /// Narrow the names this form reads to what a box allows of them.
    fn narrow(&mut self, bx: &Box) {
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
    pub fn interval(&self) -> Option<Option<(Rat, Rat)>> {
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

    /// `Some(false)` when no value of this form sits half a step off `grid`, `Some(true)` when
    /// one may, `None` when the numbers do not fit.
    ///
    /// Settled exactly, ranges included, wherever `tie_witness` can walk it: `amount × 3.49%`
    /// reaches a half cent at one amount in ten thousand, and whether that amount lies in the
    /// declared range is a question with an answer; so is whether two small amounts at two
    /// rates ever add up to one. Past that, the common divisor alone decides, ranges left out.
    pub fn reaches_tie(&self, grid: Rat) -> Option<bool> {
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

/// An interval for each numeric name — an input, or a derived column read as one name — a row
/// allows. A name it does not carry is anywhere in its declared range.
pub type Box = BTreeMap<String, (Rat, Rat)>;

/// A derived column read as one name (§15.153): one a table tests, whose expression is linear in
/// what comes in. Its values lie on the grid its terms land on, inside the range its expression
/// reaches, and a row's cell on the column narrows that range the way a cell on an input does.
struct Derived {
    /// The expression, in the names that come in.
    form: GridForm,
    step: Rat,
    range: (Rat, Rat),
}

/// What the grid argument knows of one rule, for every question asked of it.
pub struct Grid<'a> {
    f: &'a RuleFile,
    c: &'a Checked,
    derived: BTreeMap<String, Derived>,
    boxes: RefCell<HashMap<(usize, usize), Vec<Box>>>,
}

/// How many branches the grid argument follows before it gives up and leaves the obligation
/// standing.
const GRID_BRANCHES: usize = 4096;

/// How many combinations `tie_witness` walks before it leaves the question to the divisor.
const TIE_WALK: i128 = 1 << 20;

/// How many pieces `policy first` may cut one row into before the rest of its earlier rows are
/// left out — which only leaves more values possible.
const ROW_PIECES: usize = 64;

/// How many inputs an exhaustive look evaluates at most (§15.153).
const EXHAUST: usize = 1 << 14;

impl<'a> Grid<'a> {
    pub fn new(f: &'a RuleFile, c: &'a Checked) -> Grid<'a> {
        let mut g = Grid { f, c, derived: BTreeMap::new(), boxes: RefCell::new(HashMap::new()) };
        let tested: BTreeSet<&str> =
            c.sets.iter().flat_map(|s| s.table.inputs.iter().map(|(n, _)| n.as_str())).collect();
        let inputs: BTreeSet<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
        let mut derived = BTreeMap::new();
        for it in &f.items {
            let Item::Derived(d) = it else { continue };
            if !tested.contains(d.name.text.as_str()) {
                continue;
            }
            let Some(forms) = g.forms(&d.expr, 0, &mut 0) else { continue };
            let [form] = forms.as_slice() else { continue };
            if !form.terms.keys().all(|n| inputs.contains(n.as_str())) {
                continue;
            }
            // The grid: every term's coefficient times its step, and the constant.
            let mut step = abs(form.k);
            let mut ok = true;
            for (n, a) in &form.terms {
                match form.steps.get(n).and_then(|q| a.checked_mul(*q)).and_then(|t| rat_gcd(step, abs(t))) {
                    Some(s) => step = s,
                    None => ok = false,
                }
            }
            let Some((Some(lo), Some(hi))) = c.ranges.get(d.name.text.as_str()).copied() else { continue };
            if !ok || step.num == 0 {
                continue;
            }
            derived.insert(d.name.text.clone(), Derived { form: form.clone(), step, range: (lo, hi) });
        }
        g.derived = derived;
        // Boxes made while the derived columns were being read do not know them.
        g.boxes.borrow_mut().clear();
        g
    }

    /// The forms output `name` can take, one per way through the tables and the sides of a
    /// `min` or `max`, each inside what the rule's `constraint`s allow; `None` when the
    /// arithmetic does not cover it.
    pub fn forms_of(&self, name: &str) -> Option<Vec<GridForm>> {
        let span = self.f.outputs.iter().find(|o| o.name.text == name).map(|o| o.name.span.clone())?;
        let everywhere = self.settle(Box::new())?;
        let mut forms = self.forms(&Expr::Name(name.to_string(), span), 0, &mut 0)?;
        forms.iter_mut().for_each(|g| g.narrow(&everywhere));
        Some(forms)
    }

    /// The forms `name` takes where row `ri` of definition set `si` wins: one per piece of the
    /// row's region (`row_boxes`). Empty where the row never wins.
    pub fn forms_at_row(&self, si: usize, ri: usize, name: &str) -> Option<Vec<GridForm>> {
        let row = &self.c.sets[si].table.rows[ri];
        let forms = self.forms(&Expr::Name(name.to_string(), row.span.clone()), 0, &mut 0)?;
        let mut out = Vec::new();
        for bx in self.row_boxes(si, ri) {
            for g in &forms {
                let mut g = g.clone();
                g.narrow(&bx);
                out.push(g);
            }
        }
        Some(out)
    }

    /// The forms an expression can take: one per way through the tables it reads — a table's
    /// column is each of its rows' cells in turn, where the row wins — and per side of a `min`
    /// or `max`. `None` for anything the arithmetic does not cover.
    fn forms(&self, e: &Expr, depth: usize, fresh: &mut usize) -> Option<Vec<GridForm>> {
        let (f, c) = (self.f, self.c);
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
                if let Some(d) = self.derived.get(n) {
                    return Some(vec![GridForm::var(n, d.step, Some(d.range))]);
                }
                let expr_of = f.items.iter().find_map(|it| match it {
                    Item::Derived(d) if d.name.text == *n => Some(&d.expr),
                    Item::Define(d) if d.name.text == *n => Some(&d.expr),
                    _ => None,
                });
                if let Some(x) = expr_of {
                    return self.forms(x, depth + 1, fresh);
                }
                // The first output, when a `result` line gives it.
                if let (Some(r), Some(od)) = (&f.result, f.outputs.first()) {
                    if od.name.text == *n {
                        return self.forms(&r.expr, depth + 1, fresh);
                    }
                }
                let ty = c.ty_of(n)?;
                let mut out = Vec::new();
                let mut column = false;
                for (si, set) in c.sets.iter().enumerate() {
                    let t = &set.table;
                    let Some(ci) = t.outputs.iter().position(|o| o.name.text == *n) else { continue };
                    column = true;
                    for (ri, row) in t.rows.iter().enumerate() {
                        // A row counts only where it wins: its own cells, the rows before it
                        // under `policy first`, the derived columns and the `constraint`s.
                        let boxes = self.row_boxes(si, ri);
                        if boxes.is_empty() {
                            continue;
                        }
                        match row.outs.get(ci)? {
                            OutCell::Lit(Lit::Num(x)) => out.push(GridForm::con(crate::types::lit_value_in_pub(x, &ty)?)),
                            OutCell::Lit(_) => return None,
                            // A word that names nothing is `none`, or a value of an enum: no
                            // number, so nothing that could sit on a tie.
                            OutCell::Name(m) if !bound(f, m) => {}
                            OutCell::Name(m) => {
                                for g in self.forms(&Expr::Name(m.clone(), row.span.clone()), depth + 1, fresh)? {
                                    for bx in &boxes {
                                        let mut g = g.clone();
                                        g.narrow(bx);
                                        out.push(g);
                                    }
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
                if !numeric(&ty) {
                    return None;
                }
                let range = match c.ranges.get(n.as_str()) {
                    Some((Some(lo), Some(hi))) => Some((*lo, *hi)),
                    _ => None,
                };
                Some(vec![GridForm::var(n, crate::coverage::quantum(c, n, &ty), range)])
            }
            Expr::Bin(l, op, r, _) => {
                let (a, b) = (self.forms(l, depth + 1, fresh)?, self.forms(r, depth + 1, fresh)?);
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
                        out.extend(self.forms(x, depth + 1, fresh)?);
                    }
                    return (out.len() <= GRID_BRANCHES).then_some(out);
                }
                // `allocate` is rounded down to a whole unit, a rounding to its grid: each
                // lands on a grid of its own, whatever it was computed from.
                let step = if name == crate::kw::ALLOCATE {
                    Rat::int(1)
                } else if crate::num::RoundMode::parse(name).is_some() {
                    let g = self.forms(args.get(1)?, depth + 1, fresh)?;
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

    /// Where row `ri` of definition set `si` wins, as boxes: its own cells on numbers that come
    /// in and on derived columns, less what the rows before it take under `policy first`, inside
    /// what the `constraint`s allow. Empty when the row never wins.
    ///
    /// A row before it takes a piece away only where one of its cells is left that this row does
    /// not already satisfy, and that cell is a number: then this row wins outside that cell's
    /// interval, which is one or two boxes. Where more than one cell is left, the row before it
    /// is passed over — which only leaves more values possible.
    pub fn row_boxes(&self, si: usize, ri: usize) -> Vec<Box> {
        if let Some(b) = self.boxes.borrow().get(&(si, ri)) {
            return b.clone();
        }
        let out = self.compute_row_boxes(si, ri);
        self.boxes.borrow_mut().insert((si, ri), out.clone());
        out
    }

    fn compute_row_boxes(&self, si: usize, ri: usize) -> Vec<Box> {
        let set = &self.c.sets[si];
        let t = &set.table;
        let row = &t.rows[ri];
        let mut own = Box::new();
        for ((col, _), cell) in t.inputs.iter().zip(&row.cells) {
            if let Some(iv) = self.cell_interval(col, cell) {
                own.insert(col.clone(), iv);
            }
        }
        let Some(own) = self.settle(own) else { return Vec::new() };
        let mut boxes = vec![own];
        for &e in &set.beats[ri] {
            let erow = &t.rows[e];
            let mut next = Vec::new();
            for b in boxes {
                // The cells of the earlier row this piece does not already satisfy.
                let mut open: Vec<(&String, (Option<Rat>, Option<Rat>))> = Vec::new();
                let mut other = false;
                for (ci, (col, _)) in t.inputs.iter().enumerate() {
                    let (Some(ecell), Some(rcell)) = (erow.cells.get(ci), row.cells.get(ci)) else { continue };
                    if matches!(ecell, Cell::DontCare) {
                        continue;
                    }
                    match self.lattice(col) {
                        Some(q) => {
                            let Some(ebounds) = self.cell_bounds(col, ecell, q) else {
                                other = true;
                                continue;
                            };
                            let (bl, bh) = self.span_in(&b, col);
                            let inside = ebounds.0.is_none_or(|l| bl.cmp_to(l) != std::cmp::Ordering::Less)
                                && ebounds.1.is_none_or(|h| bh.cmp_to(h) != std::cmp::Ordering::Greater);
                            if !inside {
                                open.push((col, ebounds));
                            }
                        }
                        None => {
                            if !self.implies(col, rcell, ecell) {
                                other = true;
                            }
                        }
                    }
                }
                if open.is_empty() && !other {
                    continue; // the earlier row holds all over this piece: it never wins here
                }
                if other || open.len() > 1 {
                    next.push(b);
                    continue;
                }
                let (col, (el, eh)) = open[0];
                let q = self.lattice(col).unwrap_or(Rat::int(1));
                let (bl, bh) = self.span_in(&b, col);
                // Below the earlier row's interval, and above it.
                if let Some(el) = el {
                    let top = min_rat(bh, below(el, q));
                    if bl.cmp_to(top) != std::cmp::Ordering::Greater {
                        let mut p = b.clone();
                        p.insert(col.clone(), (bl, top));
                        if let Some(p) = self.settle(p) {
                            next.push(p);
                        }
                    }
                }
                if let Some(eh) = eh {
                    let bottom = max_rat(bl, above(eh, q));
                    if bottom.cmp_to(bh) != std::cmp::Ordering::Greater {
                        let mut p = b.clone();
                        p.insert(col.clone(), (bottom, bh));
                        if let Some(p) = self.settle(p) {
                            next.push(p);
                        }
                    }
                }
            }
            boxes = next;
            if boxes.is_empty() || boxes.len() > ROW_PIECES {
                break;
            }
        }
        boxes
    }

    /// Carry what a box says through the derived columns and the `constraint`s, both ways, until
    /// nothing moves (twice is enough for the shapes the language has). `None` when some name is
    /// left with no value: the box is empty.
    ///
    /// A derived column's interval narrows each of its terms (`合計 <= 60円` keeps `商品A` at or
    /// under 60 yen, less what `商品B` must at least be), and the terms' intervals narrow the
    /// column. `a <= b` keeps `a` under `b`'s top and `b` over `a`'s bottom. This is the one-step
    /// form of eliminating the other names (§15.126): enough to bound each name alone, which is
    /// all a box holds.
    fn settle(&self, mut b: Box) -> Option<Box> {
        // A narrowing whose numbers do not fit is skipped, never taken for an empty box: an empty
        // box drops the row, and a row dropped on overflow is a value the argument missed.
        let term = |a: Rat, (l, h): (Rat, Rat)| -> Option<(Rat, Rat)> {
            let (p, r) = (a.checked_mul(l)?, a.checked_mul(h)?);
            Some(if p.cmp_to(r) == std::cmp::Ordering::Greater { (r, p) } else { (p, r) })
        };
        for _ in 0..2 {
            for (name, d) in &self.derived {
                // The column from its terms.
                let from_terms = d.form.terms.iter().filter(|(_, a)| a.num != 0).try_fold((d.form.k, d.form.k), |(lo, hi), (x, a)| {
                    let (p, r) = term(*a, self.span_in(&b, x))?;
                    Some((lo.checked_add(p)?, hi.checked_add(r)?))
                });
                let (mut dl, mut dh) = self.span_in(&b, name);
                if let Some((lo, hi)) = from_terms {
                    (dl, dh) = (max_rat(dl, up(lo, d.step)), min_rat(dh, down(hi, d.step)));
                    if dl.cmp_to(dh) == std::cmp::Ordering::Greater {
                        return None;
                    }
                    b.insert(name.clone(), (dl, dh));
                }
                // Each term from the column and the others: a·x ∈ [dl − rest_hi, dh − rest_lo].
                for (x, a) in d.form.terms.iter().filter(|(_, a)| a.num != 0) {
                    let rest = d.form.terms.iter().filter(|(y, ay)| *y != x && ay.num != 0).try_fold((d.form.k, d.form.k), |(lo, hi), (y, ay)| {
                        let (p, r) = term(*ay, self.span_in(&b, y))?;
                        Some((lo.checked_add(p)?, hi.checked_add(r)?))
                    });
                    let Some((rest_lo, rest_hi)) = rest else { continue };
                    let bounds = (|| {
                        let (p, r) = (dl.checked_sub(rest_hi)?.checked_div(*a)?, dh.checked_sub(rest_lo)?.checked_div(*a)?);
                        Some(if p.cmp_to(r) == std::cmp::Ordering::Greater { (r, p) } else { (p, r) })
                    })();
                    let (Some((p, r)), Some(q)) = (bounds, self.lattice(x)) else { continue };
                    let (xl, xh) = self.span_in(&b, x);
                    let (xl, xh) = (max_rat(xl, up(p, q)), min_rat(xh, down(r, q)));
                    if xl.cmp_to(xh) == std::cmp::Ordering::Greater {
                        return None;
                    }
                    b.insert(x.clone(), (xl, xh));
                }
            }
            for k in &self.f.constraints {
                let (Some(_), Some(_)) = (self.lattice(&k.left), self.lattice(&k.right)) else { continue };
                let (ll, lh) = self.span_in(&b, &k.left);
                let (rl, rh) = self.span_in(&b, &k.right);
                // `<` and `>` are read as `<=` and `>=`: a step wider, never narrower.
                let ((ll, lh), (rl, rh)) = match k.op {
                    CmpOp::Le | CmpOp::Lt => ((ll, min_rat(lh, rh)), (max_rat(rl, ll), rh)),
                    CmpOp::Ge | CmpOp::Gt => ((max_rat(ll, rl), lh), (rl, min_rat(rh, lh))),
                };
                if ll.cmp_to(lh) == std::cmp::Ordering::Greater || rl.cmp_to(rh) == std::cmp::Ordering::Greater {
                    return None;
                }
                b.insert(k.left.clone(), (ll, lh));
                b.insert(k.right.clone(), (rl, rh));
            }
        }
        Some(b)
    }

    /// The step a numeric name that comes in, or a derived column read as one name, lands on.
    fn lattice(&self, col: &str) -> Option<Rat> {
        if let Some(d) = self.derived.get(col) {
            return Some(d.step);
        }
        let comes_in = self.f.inputs.iter().chain(self.f.elements.iter().flat_map(|e| &e.fields)).any(|i| i.name.text == col);
        let ty = self.c.ty_of(col)?;
        (comes_in && numeric(&ty)).then(|| crate::coverage::quantum(self.c, col, &ty))
    }

    /// The declared range of a name, or its derived column's.
    fn declared(&self, col: &str) -> Option<(Rat, Rat)> {
        if let Some(d) = self.derived.get(col) {
            return Some(d.range);
        }
        match self.c.ranges.get(col) {
            Some((Some(l), Some(h))) => Some((*l, *h)),
            _ => None,
        }
    }

    /// The interval a box gives a name: its own entry, or else the declared range.
    fn span_in(&self, b: &Box, col: &str) -> (Rat, Rat) {
        b.get(col).copied().or_else(|| self.declared(col)).unwrap_or((Rat::int(i64::MIN as i128), Rat::int(i64::MAX as i128)))
    }

    /// A cell on a number, as the bounds it puts on the column's grid; `None` for a cell that is
    /// not one.
    fn cell_bounds(&self, col: &str, cell: &Cell, q: Rat) -> Option<(Option<Rat>, Option<Rat>)> {
        let ty = self.c.ty_of(col)?;
        let val = |l: &Lit| -> Option<Rat> {
            match l {
                Lit::Num(n) => crate::types::lit_value_in_pub(n, &ty),
                Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                _ => None,
            }
        };
        match cell {
            Cell::Lit(l) => {
                let v = val(l)?;
                Some((Some(v), Some(v)))
            }
            Cell::Cmp(atoms) => {
                let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = (None, None);
                for (op, l) in atoms {
                    let v = val(l)?;
                    match op {
                        CmpOp::Le => hi = Some(hi.map_or(down(v, q), |h| min_rat(h, down(v, q)))),
                        CmpOp::Lt => hi = Some(hi.map_or(below(v, q), |h| min_rat(h, below(v, q)))),
                        CmpOp::Ge => lo = Some(lo.map_or(up(v, q), |l| max_rat(l, up(v, q)))),
                        CmpOp::Gt => lo = Some(lo.map_or(above(v, q), |l| max_rat(l, above(v, q)))),
                    }
                }
                Some((lo, hi))
            }
            _ => None,
        }
    }

    /// A row's cell on a number, as an interval inside the column's declared range.
    fn cell_interval(&self, col: &str, cell: &Cell) -> Option<(Rat, Rat)> {
        let q = self.lattice(col)?;
        let (lo, hi) = self.cell_bounds(col, cell, q)?;
        let (dl, dh) = self.declared(col)?;
        Some((lo.map_or(dl, |l| max_rat(l, dl)), hi.map_or(dh, |h| min_rat(h, dh))))
    }

    /// Whether every value `rcell` lets through on a column that is not a number `ecell` lets
    /// through too. An enum or a truth value is a finite set, and the sets are compared.
    fn implies(&self, col: &str, rcell: &Cell, ecell: &Cell) -> bool {
        if matches!(ecell, Cell::DontCare) {
            return true;
        }
        let Some(ty) = self.c.ty_of(col) else { return false };
        let inner = match &ty {
            Ty::Opt(t) => (**t).clone(),
            other => other.clone(),
        };
        let mut values: Vec<Val> = match &inner {
            Ty::Enum(en) => self.c.enums.get(en).into_iter().flatten().map(|v| Val::Enum(v.clone())).collect(),
            Ty::Bool => vec![Val::Bool(true), Val::Bool(false)],
            _ => return false,
        };
        if matches!(ty, Ty::Opt(_)) {
            values.push(Val::Enum(crate::kw::NONE.into()));
        }
        values
            .iter()
            .filter(|v| matches!(rcell, Cell::DontCare) || eval::cell_matches(self.c, rcell, v, &ty))
            .all(|v| eval::cell_matches(self.c, ecell, v, &ty))
    }
}

/// Whether the values of a type are numbers on a grid.
fn numeric(ty: &Ty) -> bool {
    matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date)
}

/// The largest multiple of `q` at or under `v`, and the smallest at or over it.
fn down(v: Rat, q: Rat) -> Rat {
    let m = v.div(q);
    Rat::int(m.num.div_euclid(m.den)).mul(q)
}

fn up(v: Rat, q: Rat) -> Rat {
    let m = v.div(q);
    Rat::int(-((-m.num).div_euclid(m.den))).mul(q)
}

/// The largest multiple of `q` under `v`, and the smallest over it.
fn below(v: Rat, q: Rat) -> Rat {
    let d = down(v, q);
    if d == v { d.sub(q) } else { d }
}

fn above(v: Rat, q: Rat) -> Rat {
    let u = up(v, q);
    if u == v { u.add(q) } else { u }
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

pub fn max_rat(a: Rat, b: Rat) -> Rat {
    if a.cmp_to(b) == std::cmp::Ordering::Less { b } else { a }
}

pub fn min_rat(a: Rat, b: Rat) -> Rat {
    if a.cmp_to(b) == std::cmp::Ordering::Greater { b } else { a }
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

/// Every input a question can turn on, each with every value it can take, when there are few
/// enough of them to evaluate all (§15.153). The rest of the inputs stay at one value each.
struct Domain {
    names: Vec<String>,
    values: Vec<Vec<Val>>,
    rest: BTreeMap<String, Val>,
}

impl Domain {
    /// The inputs `names` reads, and those a `constraint` ties to them; `None` when one is not
    /// an input (a field of an element, a count), cannot be listed (a string), or there are
    /// more combinations than `EXHAUST`.
    fn of(f: &RuleFile, c: &Checked, reads: &BTreeSet<String>) -> Option<Domain> {
        if f.fold.is_some() || f.elements.is_some() {
            return None;
        }
        let inputs: BTreeSet<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
        let mut names: BTreeSet<String> = BTreeSet::new();
        for n in reads {
            if !inputs.contains(n.as_str()) {
                return None;
            }
            names.insert(n.clone());
        }
        loop {
            let before = names.len();
            for k in &f.constraints {
                if names.contains(&k.left) || names.contains(&k.right) {
                    names.insert(k.left.clone());
                    names.insert(k.right.clone());
                }
            }
            if names.len() == before {
                break;
            }
        }
        let mut d = Domain { names: Vec::new(), values: Vec::new(), rest: BTreeMap::new() };
        let mut total: usize = 1;
        for i in &f.inputs {
            let n = &i.name.text;
            let ty = c.ty_of(n)?;
            if names.contains(n) {
                let vs = listed(c, n, &ty, EXHAUST)?;
                total = total.checked_mul(vs.len())?;
                if total > EXHAUST {
                    return None;
                }
                d.names.push(n.clone());
                d.values.push(vs);
            } else {
                d.rest.insert(n.clone(), one_value(c, n, &ty));
            }
        }
        Some(d)
    }

    /// Every combination, each as a full assignment, with the index of each name's value.
    fn each(&self, mut visit: impl FnMut(&[usize], BTreeMap<String, Val>) -> bool) {
        let mut at = vec![0usize; self.names.len()];
        loop {
            let mut a = self.rest.clone();
            for (i, n) in self.names.iter().enumerate() {
                a.insert(n.clone(), self.values[i][at[i]].clone());
            }
            if !visit(&at, a) {
                return;
            }
            let mut i = 0;
            loop {
                if i == at.len() {
                    return;
                }
                at[i] += 1;
                if at[i] < self.values[i].len() {
                    break;
                }
                at[i] = 0;
                i += 1;
            }
        }
    }
}

/// Every value an input takes, when there are at most `cap`: each multiple of its step in its
/// range, each value of its enum.
fn listed(c: &Checked, n: &str, ty: &Ty, cap: usize) -> Option<Vec<Val>> {
    let inner = match ty {
        Ty::Opt(t) => (**t).clone(),
        other => other.clone(),
    };
    let mut out: Vec<Val> = Vec::new();
    if matches!(ty, Ty::Opt(_)) {
        out.push(Val::Enum(crate::kw::NONE.into()));
    }
    match &inner {
        Ty::Enum(en) => out.extend(c.enums.get(en).into_iter().flatten().map(|v| Val::Enum(v.clone()))),
        Ty::Bool => out.extend([Val::Bool(true), Val::Bool(false)]),
        t if numeric(t) => {
            let (Some(lo), Some(hi)) = c.ranges.get(n).copied()? else { return None };
            let q = crate::coverage::quantum(c, n, t);
            let (lo, hi) = (up(lo, q), down(hi, q));
            let count = hi.sub(lo).div(q);
            if !count.is_int() || count.num < 0 || count.num as usize >= cap {
                return None;
            }
            for i in 0..=count.num {
                let v = lo.add(q.mul(Rat::int(i)));
                out.push(if matches!(t, Ty::Date) {
                    let (y, m, d) = crate::types::ord_to_date(v);
                    Val::Date(y, m, d)
                } else {
                    Val::Num(v)
                });
            }
        }
        _ => return None,
    }
    Some(out)
}

/// Some value an input takes, for an input no question turns on.
fn one_value(c: &Checked, n: &str, ty: &Ty) -> Val {
    match ty {
        Ty::Opt(_) => Val::Enum(crate::kw::NONE.into()),
        Ty::Enum(en) => Val::Enum(c.enums.get(en).and_then(|v| v.first()).cloned().unwrap_or_default()),
        Ty::Bool => Val::Bool(true),
        Ty::Str => Val::Str(String::new()),
        t => {
            let lo = c.ranges.get(n).and_then(|r| r.0).unwrap_or(Rat::zero());
            if matches!(t, Ty::Date) {
                let (y, m, d) = crate::types::ord_to_date(lo);
                Val::Date(y, m, d)
            } else {
                Val::Num(lo)
            }
        }
    }
}

/// Every input that output `name` can turn on, evaluated, for one that lands it on its tie:
/// `Some(Some(input))`, `Some(None)` when no input does, `None` when there are too many to try
/// (§15.153). What the arithmetic leaves open, a small enough domain settles.
pub fn exhaust_tie(f: &RuleFile, c: &Checked, name: &str, grid: Rat) -> Option<Option<BTreeMap<String, Val>>> {
    let reads = reads_of(f);
    let dom = Domain::of(f, c, &leaves_of(f, &reads, name))?;
    let mut found = None;
    dom.each(|_, a| {
        if crate::vectors::allowed(f, &a) {
            let (_, _, b) = eval::run_bindings(f, c, a.clone().into_iter().collect());
            if let Some(Val::Num(v)) = b.get(name) {
                if crate::vectors::is_tie(*v, grid) {
                    found = Some(a);
                    return false;
                }
            }
        }
        true
    });
    Some(found)
}

/// Every input a value pair can turn on, evaluated, for two that make the pair: on the row
/// tagged `tag` (anywhere, without one), one input apart, with `col` moved the way the audit
/// compares it (`coverage::value_moved`). `Some(None)` when no two inputs do — the obligation
/// cannot be met — and `None` when there are too many to try (§15.153).
pub fn exhaust_pair(
    f: &RuleFile,
    c: &Checked,
    tag: Option<&str>,
    col: &str,
) -> Option<Option<(BTreeMap<String, Val>, BTreeMap<String, Val>)>> {
    let reads = reads_of(f);
    // A column on the way to the outputs moves only together with one of them, so every input
    // any output turns on is in the question.
    let mut on = leaves_of(f, &reads, col);
    if !f.outputs.iter().any(|o| o.name.text == col) {
        for o in &f.outputs {
            on.extend(leaves_of(f, &reads, &o.name.text));
        }
    }
    let dom = Domain::of(f, c, &on)?;
    // Each input met so far, grouped once per name by every other name's value: two in one
    // group are one input apart. A pair is looked for as each input comes, and the walk stops
    // at the first.
    type Seen = (BTreeMap<String, Val>, Option<Val>, Vec<(String, Option<Val>)>);
    let mut seen: Vec<Seen> = Vec::new();
    let mut groups: Vec<HashMap<Vec<usize>, Vec<usize>>> = vec![HashMap::new(); dom.names.len()];
    let mut found = None;
    dom.each(|at, a| {
        if !crate::vectors::allowed(f, &a) {
            return true;
        }
        let (outs, fired, b) = eval::run_all(f, c, a.clone().into_iter().collect());
        if !tag.is_none_or(|t| fired.iter().any(|x| x == t)) {
            return true;
        }
        let here = seen.len();
        seen.push((a, b.get(col).cloned(), outs));
        for (x, g) in groups.iter_mut().enumerate() {
            let mut key = at.to_vec();
            key[x] = usize::MAX;
            let members = g.entry(key).or_default();
            for &m in members.iter() {
                let (p, q) = (&seen[m], &seen[here]);
                if value_moved(f, col, (p.1.as_ref(), &p.2), (q.1.as_ref(), &q.2)) {
                    found = Some((p.0.clone(), q.0.clone()));
                    return false;
                }
            }
            members.push(here);
        }
        true
    });
    Some(found)
}

/// The inputs a name turns on. The first output a `result` line gives reads the names of that
/// line's expression.
fn leaves_of(f: &RuleFile, reads: &HashMap<String, BTreeSet<String>>, name: &str) -> BTreeSet<String> {
    if let (Some(r), Some(od)) = (&f.result, f.outputs.first()) {
        if od.name.text == name && !reads.contains_key(name) {
            let mut ns = BTreeSet::new();
            crate::coverage::expr_names(&r.expr, &mut ns);
            return ns.iter().flat_map(|m| leaves(reads, m)).collect();
        }
    }
    leaves(reads, name)
}
