//! The region IR and box algebra (§6).
//!
//! Each input column becomes an independent axis, and its coordinates are compressed to the
//! boundary values that appear in the column. Because §3 restricts a cell to a unary test on
//! its own column, a row is "the product of one subset per axis", that is, a box, and
//! completeness, unreachable rows, and overlaps all reduce to finite set operations.

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
enum Coord {
    /// Exactly this value.
    Point(Rat),
    /// Strictly between two boundaries (neither end included). A `None` end means the interval
    /// runs on past the last boundary, where the declared range has no limit.
    Open(Option<Rat>, Option<Rat>),
}

#[derive(Debug, Clone)]
enum Axis {
    /// `values` may start with `none` (the axis of an optional column).
    Enum { values: Vec<String> },
    /// Numbers and dates. A date is held as an ordinal (its serial day number), so the interval
    /// machinery applies unchanged. An empty `unit` means "write it back as a date".
    Num { unit: String, coords: Vec<Coord> },
    Bool,
}

impl Axis {
    fn len(&self) -> usize {
        match self {
            Axis::Enum { values } => values.len(),
            Axis::Num { coords, .. } => coords.len(),
            Axis::Bool => 2,
        }
    }
    /// The concrete value used as a witness. §11 principle 2 prefers declaration order for
    /// enums and boundary values for numbers.
    fn witness(&self, i: usize) -> String {
        match self {
            Axis::Enum { values } => values.get(i).cloned().unwrap_or_default(),
            Axis::Bool => if i == 0 { crate::kw::TRUE.into() } else { crate::kw::FALSE.into() },
            Axis::Num { unit, coords } if unit.is_empty() => match coords.get(i) {
                // For dates, prefer a boundary (an actual calendar day). The midpoint of an
                // interval need not be one.
                Some(Coord::Point(v)) => {
                    let (y, m, d) = crate::types::ord_to_date(*v);
                    format!("{y:04}-{m:02}-{d:02}")
                }
                Some(Coord::Open(a, b)) => {
                    // Dates are serial day numbers, so an open interval always contains a real
                    // calendar day. Print that day instead of hedging with "around".
                    let v = match (a, b) {
                        (Some(a), _) => a.add(Rat::int(1)),
                        (None, Some(b)) => b.sub(Rat::int(1)),
                        (None, None) => Rat::zero(),
                    };
                    let (y, m, d) = crate::types::ord_to_date(v);
                    format!("{y:04}-{m:02}-{d:02}")
                }
                None => String::new(),
            },
            Axis::Num { unit, coords } => match coords.get(i) {
                Some(Coord::Point(v)) => format!("{v}{unit}"),
                Some(Coord::Open(a, b)) => {
                    let v = match (a, b) {
                        (Some(a), Some(b)) => a.add(*b).div(Rat::int(2)),
                        (Some(a), None) => a.add(Rat::int(1)),
                        (None, Some(b)) => b.sub(Rat::int(1)),
                        (None, None) => Rat::zero(),
                    };
                    format!("{v}{unit}")
                }
                None => String::new(),
            },
        }
    }

    /// The same witness value, typed, for the structured half of a diagnostic. Numbers come
    /// out as integers in the canonical unit and dates as `YYYY-MM-DD`, which is the wire
    /// shape of §10.2 — a caller can hand a witness straight to a vector or a fixture.
    fn witness_val(&self, i: usize) -> crate::diag::WVal {
        use crate::diag::WVal;
        match self {
            Axis::Enum { .. } => WVal::Str(self.witness(i)),
            Axis::Bool => WVal::Bool(i == 0),
            // Empty unit means "a date"; the display form is already `YYYY-MM-DD`.
            Axis::Num { unit, .. } if unit.is_empty() => WVal::Str(self.witness(i)),
            Axis::Num { coords, .. } => {
                let v = match coords.get(i) {
                    Some(Coord::Point(v)) => *v,
                    Some(Coord::Open(a, b)) => match (a, b) {
                        (Some(a), Some(b)) => a.add(*b).div(Rat::int(2)),
                        (Some(a), None) => a.add(Rat::int(1)),
                        (None, Some(b)) => b.sub(Rat::int(1)),
                        (None, None) => Rat::zero(),
                    },
                    None => Rat::zero(),
                };
                WVal::Int(v.num / v.den)
            }
        }
    }
}

/// Feasibility per §6.2. Because derived values get independent axes, the space contains points
/// that cannot actually occur. The sieve drops those at reporting time, but once dependencies
/// are involved it cannot always decide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Feasible {
    /// Proven infeasible. Not reported.
    No,
    /// A witness could be constructed. Reported as an error.
    Yes,
    /// Undecided. What has not been proven must not be presented as proven (W114).
    Unknown,
}

pub struct TableRegion {
    axes: Vec<Axis>,
    col_names: Vec<String>,
    /// Axis → the coordinates of the values an upstream table can produce. `None` means
    /// unrestricted (an input or derived axis).
    reachable: Vec<Option<Vec<bool>>>,
    /// The name and type of a column whose type cannot be analyzed, if there is one. Rather
    /// than skipping the check, we stop with E110.
    unanalyzable: Option<(String, Ty)>,
    /// Row → whether it names only values the upstream table can never reach (a variant of
    /// E102).
    unreachable_row: Vec<bool>,
    /// Axis → its original column position in the table. The search walks the narrow axes
    /// first, but witnesses are printed in the table's visible order.
    display_of: Vec<usize>,
    /// Whether the axis is a boolean definition. The sieve does not look at definition axes
    /// (M1 adds that, verification first), so a witness for an overlap that involves a
    /// definition column says that "consistency with the definition's body is unconfirmed".
    is_define: Vec<bool>,
    /// If the axis is a derived value, its reachable interval and the names of the inputs it
    /// depends on.
    derived: Vec<Option<((Option<Rat>, Option<Rat>), Vec<String>)>>,
    /// Row → axis → whether each coordinate is selected.
    masks: Vec<Vec<Vec<bool>>>,
}

fn lit_rat(l: &Lit, want: &Ty) -> Option<Rat> {
    match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, want),
        Lit::Date(y, m, d) if *want == Ty::Date => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

/// Collect the boundaries of a numeric axis. Both ends of the declared range count as
/// boundaries too, which closes the universe into a finite one.
fn num_bounds(rows: &[Row], ci: usize, want: &Ty, range: &Option<Range>) -> (Vec<Rat>, Option<Rat>, Option<Rat>) {
    let mut set: BTreeSet<(i128, i128)> = BTreeSet::new();
    let mut push = |r: Rat, s: &mut BTreeSet<(i128, i128)>| {
        s.insert((r.num, r.den));
    };
    let (mut lo, mut hi) = (None, None);
    if let Some(rg) = range {
        for (op, l) in &rg.bounds {
            if let Some(v) = lit_rat(l, want) {
                match op {
                    CmpOp::Ge | CmpOp::Gt => lo = Some(v),
                    CmpOp::Le | CmpOp::Lt => hi = Some(v),
                }
                push(v, &mut set);
            }
        }
    }
    for row in rows {
        if let Some(Cell::Cmp(cs)) = row.cells.get(ci) {
            for (_, l) in cs {
                if let Some(v) = lit_rat(l, want) {
                    push(v, &mut set);
                }
            }
        }
        if let Some(Cell::Lit(l)) = row.cells.get(ci) {
            if let Some(v) = lit_rat(l, want) {
                push(v, &mut set);
            }
        }
    }
    let mut v: Vec<Rat> = set.into_iter().map(|(n, d)| Rat { num: n, den: d }).collect();
    v.sort_by(|a, b| a.cmp_to(*b));
    (v, lo, hi)
}

/// Build the coordinates. `quantum` is the smallest step a value can take (1 for `money[円]`,
/// one day for a date, 1/1000 for `rate[step 0.1%]`).
///
/// When two adjacent boundaries differ by exactly one step, the open interval between them
/// holds **no value at all**. Creating an empty coordinate there would make a table tiled with
/// `<=2026-03-31` and `>=2026-04-01` look incompletely covered and raise a false E101. Tiling
/// with inclusive ends is the natural way to write business rules, so this test is essential.
fn num_coords(bounds: &[Rat], lo: Option<Rat>, hi: Option<Rat>, quantum: Rat) -> Vec<Coord> {
    let mut out = Vec::new();
    if bounds.is_empty() {
        return vec![Coord::Open(lo, hi)];
    }
    // Below the declared lower bound is outside the universe. Only when there is no lower
    // bound does an open interval go first.
    if lo.is_none() {
        out.push(Coord::Open(None, Some(bounds[0])));
    }
    for (i, b) in bounds.iter().enumerate() {
        out.push(Coord::Point(*b));
        if i + 1 < bounds.len() {
            let gap = bounds[i + 1].sub(*b);
            if gap.cmp_to(quantum) == std::cmp::Ordering::Greater {
                out.push(Coord::Open(Some(*b), Some(bounds[i + 1])));
            }
        }
    }
    if hi.is_none() {
        out.push(Coord::Open(Some(*bounds.last().unwrap()), None));
    }
    out
}

fn cmp_holds(op: CmpOp, v: Rat, bound: Rat) -> bool {
    use std::cmp::Ordering::*;
    match (op, v.cmp_to(bound)) {
        (CmpOp::Le, Less | Equal) => true,
        (CmpOp::Lt, Less) => true,
        (CmpOp::Ge, Greater | Equal) => true,
        (CmpOp::Gt, Greater) => true,
        _ => false,
    }
}

/// Whether every value of this coordinate satisfies the comparison. Since every boundary is a
/// coordinate, an open interval always falls entirely on one side (that is what the compressed
/// coordinates buy us).
fn coord_satisfies(c: &Coord, op: CmpOp, bound: Rat) -> bool {
    match c {
        Coord::Point(v) => cmp_holds(op, *v, bound),
        Coord::Open(a, b) => match op {
            CmpOp::Le | CmpOp::Lt => b.is_some_and(|b| cmp_holds(CmpOp::Le, b, bound)),
            CmpOp::Ge | CmpOp::Gt => a.is_some_and(|a| cmp_holds(CmpOp::Ge, a, bound)),
        },
    }
}

impl TableRegion {
    pub fn build(t: &Table, c: &Checked, inputs: &[VarDecl]) -> Option<TableRegion> {
        let mut axes = Vec::new();
        let mut col_names = Vec::new();
        let mut unanalyzable: Option<(String, Ty)> = None;
        for (ci, (name, _)) in t.inputs.iter().enumerate() {
            let ty = c.ty_of(name)?;
            col_names.push(name.clone());
            // An optional column is treated as an enum with one extra value, `none`. Since
            // §2.1 rules that it "can only be consumed by a `none` cell and never appears in
            // an expression", the axis merely gains one value.
            let (ty, opt) = match &ty {
                Ty::Opt(inner) => ((**inner).clone(), true),
                other => (other.clone(), false),
            };
            let axis = match &ty {
                Ty::Enum(en) => {
                    let mut vs = c.enums.get(en)?.clone();
                    if opt {
                        vs.insert(0, crate::kw::NONE.into());
                    }
                    Axis::Enum { values: vs }
                }
                Ty::Bool => Axis::Bool,
                Ty::Date => {
                    let range = inputs.iter().find(|i| i.name.text == *name).and_then(|i| i.range.clone());
                    let (b, lo, hi) = num_bounds(&t.rows, ci, &ty, &range);
                    // A date's step is one day. Dates are serial day numbers, so adjacent days
                    // differ by 1.
                    Axis::Num { unit: String::new(), coords: num_coords(&b, lo, hi, Rat::int(1)) }
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    let range = inputs.iter().find(|i| i.name.text == *name).and_then(|i| i.range.clone());
                    let (b, lo, hi) = num_bounds(&t.rows, ci, &ty, &range);
                    let unit = match &ty {
                        Ty::Money { cur, .. } => cur.clone(),
                        Ty::Qty { unit, .. } => unit.clone(),
                        _ => "%".into(),
                    };
                    // The runtime representation is a single integer in the declared unit
                    // (§7.1), so the step is 1 for money and quantities and the declared step
                    // for rates.
                    let q = match &ty {
                        Ty::Rate => Rat::new(1, *c.scales.get(name).unwrap_or(&100)),
                        _ => Rat::int(1),
                    };
                    Axis::Num { unit, coords: num_coords(&b, lo, hi, q) }
                }
                _ => {
                    // Silently skipping a type we cannot analyze would report ok without ever
                    // checking the table's completeness or overlaps. We stepped on that twice,
                    // with dates and with optional.
                    unanalyzable = Some((name.clone(), ty.clone()));
                    Axis::Bool
                }
            };
            axes.push(axis);
        }

        // §6.3: order the axes by ascending number of compressed coordinates. Splitting on the
        // narrow axes first makes pruning kick in early.
        let mut order: Vec<usize> = (0..axes.len()).collect();
        order.sort_by_key(|&i| axes[i].len());
        let axes: Vec<Axis> = order.iter().map(|&i| axes[i].clone()).collect();
        let col_names: Vec<String> = order.iter().map(|i| col_names[*i].clone()).collect();
        let cell_of: Vec<usize> = order.clone();

        // Build these **after** reordering the axes. Built before, their indices would not
        // line up with the masks.
        let is_define: Vec<bool> = col_names.iter().map(|n| c.define_deps.contains_key(n)).collect();
        let derived: Vec<Option<((Option<Rat>, Option<Rat>), Vec<String>)>> = col_names
            .iter()
            .map(|n| c.derived_deps.get(n).map(|deps| (c.ranges.get(n).copied().unwrap_or((None, None)), deps.clone())))
            .collect();


        // The coordinates of the values an upstream table can produce. Only enum axes have them.
        let reachable: Vec<Option<Vec<bool>>> = (0..axes.len())
            .map(|ai| match (&axes[ai], c.out_values.get(&col_names[ai])) {
                (Axis::Enum { values }, Some(vs)) => {
                    Some(values.iter().map(|v| vs.contains(v)).collect())
                }
                // A boolean can be an upstream output too. Coordinate 0 is true, 1 is false.
                (Axis::Bool, Some(vs)) => Some(vec![
                    vs.iter().any(|v| v == crate::kw::TRUE),
                    vs.iter().any(|v| v == crate::kw::FALSE),
                ]),
                _ => None,
            })
            .collect();

        let mut unreachable_row = Vec::new();
        let mut masks = Vec::new();
        for row in &t.rows {
            let mut m = Vec::new();
            for (ai, axis) in axes.iter().enumerate() {
                let n = axis.len();
                let cell = row.cells.get(cell_of[ai]);
                let mut v = vec![false; n];
                let ty = c.ty_of(&col_names[ai])?;
                match cell {
                    None | Some(Cell::DontCare) => v.iter_mut().for_each(|x| *x = true),
                    // `none` matches only the first coordinate of an optional axis.
                    Some(Cell::Nothing) => {
                        if matches!(axis, Axis::Enum { values } if values.first().map(|s| s.as_str()) == Some(crate::kw::NONE)) {
                            v[0] = true;
                        }
                    }
                    Some(Cell::Lit(l)) => match (axis, l) {
                        (Axis::Enum { values }, Lit::Word(w)) => {
                            for (i, val) in values.iter().enumerate() {
                                if val == w {
                                    v[i] = true;
                                }
                            }
                            if let Some((_, members)) = c.groups.get(w) {
                                for (i, val) in values.iter().enumerate() {
                                    if members.contains(val) {
                                        v[i] = true;
                                    }
                                }
                            }
                        }
                        (Axis::Bool, Lit::Word(w)) => v[if w == crate::kw::TRUE { 0 } else { 1 }] = true,
                        (Axis::Num { coords, .. }, l) => {
                            if let Some(x) = lit_rat(l, &ty) {
                                for (i, cd) in coords.iter().enumerate() {
                                    if matches!(cd, Coord::Point(p) if p.cmp_to(x) == std::cmp::Ordering::Equal) {
                                        v[i] = true;
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                    Some(Cell::Set(ls)) | Some(Cell::Not(ls)) => {
                        if let Axis::Enum { values } = axis {
                            for l in ls {
                                if let Lit::Word(w) = l {
                                    for (i, val) in values.iter().enumerate() {
                                        if val == w {
                                            v[i] = true;
                                        }
                                    }
                                    if let Some((_, members)) = c.groups.get(w) {
                                        for (i, val) in values.iter().enumerate() {
                                            if members.contains(val) {
                                                v[i] = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if matches!(cell, Some(Cell::Not(_))) {
                            v.iter_mut().for_each(|x| *x = !*x);
                        }
                    }
                    Some(Cell::Cmp(cs)) => {
                        if let Axis::Num { coords, .. } = axis {
                            v.iter_mut().for_each(|x| *x = true);
                            for (op, l) in cs {
                                if let Some(b) = lit_rat(l, &ty) {
                                    for (i, cd) in coords.iter().enumerate() {
                                        if !coord_satisfies(cd, *op, b) {
                                            v[i] = false;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                m.push(v);
            }
            // A row that names only values the upstream table never produces is dead
            // downstream.
            let mut only_unreachable = false;
            for (ai, r) in reachable.iter().enumerate() {
                let Some(r) = r else { continue };
                let had = m[ai].iter().any(|x| *x);
                for (k, ok) in r.iter().enumerate() {
                    if !ok {
                        m[ai][k] = false;
                    }
                }
                if had && !m[ai].iter().any(|x| *x) {
                    only_unreachable = true;
                }
            }
            unreachable_row.push(only_unreachable);
            masks.push(m);
        }
        Some(TableRegion { axes, col_names, reachable, unreachable_row, display_of: cell_of, is_define, derived, masks, unanalyzable })
    }

    fn intersects(&self, i: usize, j: usize) -> bool {
        self.masks[i]
            .iter()
            .zip(&self.masks[j])
            .all(|(a, b)| a.iter().zip(b).any(|(x, y)| *x && *y))
    }

    /// Whether the region of row `inner` lies entirely within row `outer`.
    ///
    /// A row is "the product of one subset per axis", a single box, so containment is
    /// **exactly** the per-axis subset test (not an approximation by projection). Falling back
    /// to region subtraction would only be needed if a row were a union of boxes, which the
    /// current representation never produces. Subtraction is exponential in the number of
    /// axes, so we rely on the product structure here.
    fn contains(&self, outer: usize, inner: usize) -> bool {
        self.masks[inner]
            .iter()
            .zip(&self.masks[outer])
            .all(|(a, b)| a.iter().zip(b).all(|(x, y)| !*x || *y))
    }

    fn empty(&self, i: usize) -> bool {
        self.masks[i].iter().any(|m| m.iter().all(|x| !*x))
    }

    /// Return one uncovered coordinate. In the recursive split of §6.3, a branch in which some
    /// surviving row is don't-care on all remaining axes is pruned as certainly covered.
    fn find_hole(&self, rows: &[usize], budget: &mut i64) -> Option<Vec<usize>> {
        self.hole_rec(rows, 0, budget, &mut Vec::new())
    }

    fn hole_rec(
        &self,
        rows: &[usize],
        ai: usize,
        budget: &mut i64,
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if rows.is_empty() {
            let mut p = path.clone();
            while p.len() < self.axes.len() {
                p.push(0);
            }
            // A gap proven infeasible is not reported. An undecidable gap is kept (we only ever
            // drop in the direction of over-reporting; §6.1).
            if self.feasible(&p) == Feasible::No {
                return None;
            }
            return Some(p);
        }
        if ai == self.axes.len() {
            return None;
        }
        // If some row spans the whole of every remaining axis, this subtree is covered.
        if rows
            .iter()
            .any(|&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x)))
        {
            return None;
        }
        for c in 0..self.axes[ai].len() {
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            if let Some(h) = self.hole_rec(&sub, ai + 1, budget, path) {
                path.pop();
                return Some(h);
            }
            path.pop();
        }
        None
    }

    /// Describe the region where two rows overlap as a conjunction in business terms.
    ///
    /// W114 must not print a point. A single point on a derived axis is a coordinate that is
    /// not realized, and if it wore the face of a concrete value the reader would go looking
    /// for "that order". This is where the principle of not presenting the unverified as a
    /// witness applies (§11 principle 2).
    fn overlap_text(&self, a: &Row, b: &Row) -> String {
        let mut items: Vec<(usize, String)> = Vec::new();
        for ai in 0..self.axes.len() {
            let ci = self.display_of[ai];
            let (ca, cb) = (a.cells.get(ci), b.cells.get(ci));
            let mut parts: Vec<String> = Vec::new();
            for c in [ca, cb].into_iter().flatten() {
                if matches!(c, Cell::DontCare) {
                    continue;
                }
                let t = cell_text(c);
                if !parts.contains(&t) {
                    parts.push(t);
                }
            }
            if parts.is_empty() {
                continue;
            }
            items.push((ci, format!("{} {}", self.col_names[ai], parts.join(if crate::i18n::ja() { " かつ " } else { " and " }))));
        }
        items.sort_by_key(|(d, _)| *d);
        if items.is_empty() {
            return tr!("（どの列も制約していません）", "(no column is constrained)");
        }
        items.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(if crate::i18n::ja() { " かつ " } else { " and " })
    }

    /// Witnesses are written in the table's column order. The search's convenience (narrow
    /// axes first) is not shown to the reader.
    fn witness_text(&self, path: &[usize]) -> String {
        let mut items: Vec<(usize, String)> = (0..self.axes.len())
            .map(|ai| {
                (
                    self.display_of[ai],
                    format!("{} = {}", self.col_names[ai], self.axes[ai].witness(path.get(ai).copied().unwrap_or(0))),
                )
            })
            .collect();
        items.sort_by_key(|(d, _)| *d);
        items.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(", ")
    }

    /// The same witness, as `(column, value)` pairs in the table's visible column order.
    fn witness_pairs(&self, path: &[usize]) -> Vec<(String, crate::diag::WVal)> {
        let mut items: Vec<(usize, (String, crate::diag::WVal))> = (0..self.axes.len())
            .map(|ai| {
                (
                    self.display_of[ai],
                    (
                        self.col_names[ai].clone(),
                        self.axes[ai].witness_val(path.get(ai).copied().unwrap_or(0)),
                    ),
                )
            })
            .collect();
        items.sort_by_key(|(d, _)| *d);
        items.into_iter().map(|(_, p)| p).collect()
    }

    /// A row that matches the witness, written out so it can be pasted into the table. The
    /// output cells are copied from the table's first row: **the tool does not know the
    /// amount**, only the shape, and the notes say so. This is E101's `fix.text`.
    fn row_text(&self, path: &[usize], t: &Table) -> Option<String> {
        let mut cells: Vec<(usize, String)> = (0..self.axes.len())
            .map(|ai| {
                (self.display_of[ai], self.axes[ai].witness(path.get(ai).copied().unwrap_or(0)))
            })
            .collect();
        cells.sort_by_key(|(d, _)| *d);
        let mut out: Vec<String> = cells.into_iter().map(|(_, c)| c).collect();
        for o in &t.rows.first()?.outs {
            out.push(out_cell_text(o));
        }
        Some(format!("| {} |", out.join(" | ")))
    }
}

/// `(column, value)` pairs as the input half of a witness.
fn pairs_to_witness(ps: Vec<(String, crate::diag::WVal)>) -> crate::diag::Witness {
    crate::diag::Witness { inputs: ps, ..Default::default() }
}

/// An output cell as it is written in the source.
fn out_cell_text(o: &OutCell) -> String {
    match o {
        OutCell::Name(n) => n.clone(),
        OutCell::Lit(Lit::Num(n)) => n.raw.clone(),
        OutCell::Lit(Lit::Word(n)) => n.clone(),
        OutCell::Lit(Lit::Date(y, m, d)) => format!("{y:04}-{m:02}-{d:02}"),
        OutCell::Lit(Lit::Str(v)) => format!("\"{v}\""),
    }
}

/// The three kinds of shadowing (§4). Structural and equivalent ones are only counted; only
/// those that need review are listed.
#[derive(Debug, Default, Clone, Copy)]
pub struct Shadow {
    /// A later row's region contains the earlier row's. The normal shape of a staircase.
    pub structural: usize,
    /// A partial intersection, not containment, but with the same output. Whichever row wins,
    /// the value does not change.
    pub equivalent: usize,
    /// A partial intersection, not containment, with different outputs. Only this kind deserves
    /// an "is this intended?".
    pub confirm: usize,
}

impl Shadow {
    pub fn total(&self) -> usize {
        self.structural + self.equivalent + self.confirm
    }
}

/// The visited-node budget of §6.3. When it is exceeded we stop with "not proven"; there is no
/// quiet degradation to an approximate check.
///
/// The default was set from measurements of the synthetic benchmark in §15-2
/// (`tests/budget.rs`).
///
/// | columns (enum+num) | rows | nodes | ms (release) |
/// |---|---|---|---|
/// | 8+4 (the 12-column design target) | 500 | 10,042,885 | 164 |
/// | 6+6 | 500 | 10,821,337 | 197 |
/// | 6+6 | 1000 | 41,530,393 | 680 |
///
/// The speed is about 60,000 nodes/ms (release) and grows quadratically with the row count.
/// The design target (12 columns, 500 rows) fits in 10M, so it was the provisional 10⁶ that was
/// off. Fifty million gives the target a 5x margin while still stopping in just under a second
/// when fully spent. The corpus of real tables sits at 522 / 267 / 7 nodes, five orders of
/// magnitude below.
pub const DEFAULT_BUDGET: i64 = 50_000_000;

pub struct TableCheck {
    pub diags: Vec<Diag>,
    /// The row pairs of W114 (0-based). The guards in generated code go on exactly these pairs
    /// (§8.1).
    pub w114: Vec<(usize, usize)>,
    /// The number of nodes visited while checking this table. A measurement for setting the
    /// budget.
    pub nodes: i64,
    /// The structural and equivalent shadowings, listed only under `--show-shadow`.
    pub quiet: Vec<Diag>,
    pub shadow: Shadow,
    /// The shadowing pairs under `policy first` (0-based, the winning row i first). All of them,
    /// whichever of the three kinds they are. The shadow-pair coverage of §9.2 demands a point
    /// "inside the intersection" for each of these pairs.
    pub overlaps: Vec<(usize, usize)>,
    /// The rows that got E102 (0-based). They never match, so §9.2's row coverage leaves them
    /// out.
    pub dead: Vec<usize>,
}

/// The canonical form of a cell. It is the key for `--diff-base`, so it depends on neither
/// formatting nor line numbers.
fn cell_key(c: &Cell) -> String {
    fn lit(l: &Lit) -> String {
        match l {
            Lit::Num(n) => n.raw.replace(' ', ""),
            Lit::Word(w) => w.clone(),
            Lit::Str(s) => format!("\"{s}\""),
            Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        }
    }
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => crate::kw::NONE.into(),
        Cell::Lit(l) => lit(l),
        Cell::Set(ls) => ls.iter().map(lit).collect::<Vec<_>>().join(","),
        Cell::Not(ls) => format!("{}:{}", crate::kw::NOT, ls.iter().map(lit).collect::<Vec<_>>().join(",")),
        Cell::Cmp(cs) => cs
            .iter()
            .map(|(o, l)| {
                let op = match o {
                    CmpOp::Le => "<=",
                    CmpOp::Ge => ">=",
                    CmpOp::Lt => "<",
                    CmpOp::Gt => ">",
                };
                format!("{op}{}", lit(l))
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Write a cell in business terms. W114 reports a region rather than a point, so the text of
/// the condition is needed.
fn cell_text(c: &Cell) -> String {
    fn lit(l: &Lit) -> String {
        match l {
            Lit::Num(n) => n.raw.clone(),
            Lit::Word(w) => w.clone(),
            Lit::Str(s) => format!("\"{s}\""),
            Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        }
    }
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => crate::kw::NONE.into(),
        Cell::Lit(l) => format!("= {}", lit(l)),
        Cell::Set(ls) => format!("∈ {{{}}}", ls.iter().map(lit).collect::<Vec<_>>().join(", ")),
        Cell::Not(ls) => format!("∉ {{{}}}", ls.iter().map(lit).collect::<Vec<_>>().join(", ")),
        Cell::Cmp(cs) => cs
            .iter()
            .map(|(o, l)| {
                let op = match o {
                    CmpOp::Le => "<=",
                    CmpOp::Ge => ">=",
                    CmpOp::Lt => "<",
                    CmpOp::Gt => ">",
                };
                format!("{op} {}", lit(l))
            })
            .collect::<Vec<_>>()
            .join(if crate::i18n::ja() { " かつ " } else { " and " }),
    }
}

fn row_key(r: &Row) -> String {
    r.cells.iter().map(cell_key).collect::<Vec<_>>().join("|")
}

fn pair_key(a: &Row, b: &Row) -> String {
    format!("{}\u{1}{}", row_key(a), row_key(b))
}

/// Whether the output cells are syntactically identical. Used for the "equivalent" verdict of
/// §4.
fn outs_equal(a: &Row, b: &Row) -> bool {
    if a.outs.len() != b.outs.len() {
        return false;
    }
    a.outs.iter().zip(&b.outs).all(|(x, y)| match (x, y) {
        (OutCell::Name(p), OutCell::Name(q)) => p == q,
        (OutCell::Lit(Lit::Num(p)), OutCell::Lit(Lit::Num(q))) => p == q,
        (OutCell::Lit(Lit::Word(p)), OutCell::Lit(Lit::Word(q))) => p == q,
        _ => false,
    })
}

/// Emits E101 / E102 / E105 / W105 / W110.
pub fn check_table(t: &Table, c: &Checked, f: &RuleFile, path: &str, budget: i64) -> TableCheck {
    let inputs = &f.inputs;
    let mut out = Vec::new();
    let mut quiet = Vec::new();
    let mut shadow = Shadow::default();
    let mut nodes = 0i64;
    let mut w114: Vec<(usize, usize)> = Vec::new();
    let mut overlaps: Vec<(usize, usize)> = Vec::new();
    let mut dead_rows: Vec<usize> = Vec::new();
    let empty = TableCheck {
        diags: Vec::new(),
        w114: Vec::new(),
        quiet: Vec::new(),
        shadow,
        nodes: 0,
        overlaps: Vec::new(),
        dead: Vec::new(),
    };
    let Some(reg) = TableRegion::build(t, c, inputs) else { return empty };
    if reg.axes.is_empty() || t.rows.is_empty() {
        return empty;
    }
    if let Some((col, ty)) = &reg.unanalyzable {
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        return TableCheck {
            w114: Vec::new(),
            diags: vec![
                Diag::error("E110", tr!("列 {col} の型 {ty} は、まだ検査できません", "Column {col} has type {ty}, which cannot be checked yet"))
                    .at(tr!("{path}:{} 表 {tname}", "{path}:{} table {tname}", t.span.line))
                    .table(tname.clone())
                    .mark(t.span.clone(), "")
                    .note(tr!("この表の完全性も重複も検査していません。黙って通すより止めます。", "Neither the completeness nor the overlaps of this table have been checked. Stopping is better than passing it silently."))
                    .note(tr!("列の型を、列挙・真偽・数量・金額・率・日付・それらの optional のいずれかにしてください。", "Give the column one of these types: an enum, boolean, quantity, money, rate, date, or an optional of one of those.")),
            ],
            quiet: Vec::new(),
            shadow: Shadow::default(),
            nodes: 0,
            overlaps: Vec::new(),
            dead: Vec::new(),
        };
    }
    let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    let at = |line: usize| tr!("{path}:{line} 表 {tname}", "{path}:{line} table {tname}");
    let head_span: Span = t.name.as_ref().map(|n| n.span.clone()).unwrap_or(t.span.clone());

    // --- Overlaps and shadowing
    let mut shadowed = vec![false; t.rows.len()];
    for i in 0..t.rows.len() {
        for j in (i + 1)..t.rows.len() {
            if !reg.intersects(i, j) {
                continue;
            }
            let mut sub = vec![vec![false; 0]; 0];
            let _ = &mut sub;
            let mut wpath = Vec::new();
            for ai in 0..reg.axes.len() {
                let c0 = (0..reg.axes[ai].len())
                    .find(|&c| reg.masks[i][ai][c] && reg.masks[j][ai][c])
                    .unwrap_or(0);
                wpath.push(c0);
            }
            let w = reg.witness_text(&wpath);
            let feas = reg.feasible(&wpath);
            // An overlap proven infeasible is not reported (§6.2).
            if feas == Feasible::No {
                continue;
            }
            // §6.2 "witnesses on definition axes": the intersection box merely places its
            // coordinate on a definition axis freely, without checking that some input
            // actually produces that truth value. So we construct a real input, let the
            // evaluator compute the definitions too, and treat **only the overlaps we could
            // construct** as real contradictions. Failing to construct one is not a proof of
            // non-existence, so instead of asserting anything we demote to W114.
            let touches_define = (0..reg.axes.len()).any(|ai| {
                reg.is_define[ai]
                    && (0..reg.axes[ai].len()).any(|cc| reg.masks[i][ai][cc] && reg.masks[j][ai][cc])
            });
            let mut feas = feas;
            let mut built: Option<String> = None;
            if touches_define && feas != Feasible::No {
                nodes += (reg.axes.len() * 8) as i64;
                match crate::vectors::pair_witness(f, c, t, i, j) {
                    Some(a) => {
                        feas = Feasible::Yes;
                        built = Some(
                            a.iter()
                                .map(|(n, v)| format!("{n} = {}", crate::vectors::show(v)))
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                    }
                    None => feas = Feasible::Unknown,
                }
            }
            match t.policy {
                Policy::Unique if feas == Feasible::Unknown => {
                    // §8.1: a pair whose outputs are syntactically identical yields the same
                    // value whichever row wins, so it gets no guard.
                    if !outs_equal(&t.rows[i], &t.rows[j]) {
                        w114.push((i, j));
                    }
                    // An overlap for which no witness could be constructed. What has not been
                    // proven must not be presented as proven (§6.2). The guard in generated
                    // code is its counterpart.
                    let dnames: Vec<String> = reg.derived_names();
                    let outs = |r: &Row| -> String {
                        r.outs
                            .iter()
                            .map(|o| match o {
                                OutCell::Name(n) => n.clone(),
                                OutCell::Lit(Lit::Num(n)) => n.raw.clone(),
                                OutCell::Lit(Lit::Word(n)) => n.clone(),
                                _ => String::new(),
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    out.push(
                        Diag::warning(
                            "W114",
                            tr!("未確認の重なり: 行{} と 行{} の両方に当たる入力が有り得ます", "Unconfirmed overlap: an input may match both row {} and row {}", i + 1, j + 1),
                        )
                        .at(at(t.rows[j].span.line))
                        .table(tname.clone())
                        .rowref(tname.clone(), i + 1)
                        .rowref(tname.clone(), j + 1)
                        .wit(pairs_to_witness(reg.witness_pairs(&wpath)))
                        .mark(t.rows[i].span.clone(), tr!("行{}", "row {}", i + 1))
                        .mark(t.rows[j].span.clone(), tr!("行{}", "row {}", j + 1))
                        .note(tr!("重なる条件: {}", "Overlapping condition: {}", reg.overlap_text(&t.rows[i], &t.rows[j])))
                        .note(if touches_define {
                            tr!("定義の中身まで含めて、この条件を同時に満たす入力を構成できませんでした。存在しないことの証明ではありません。", "No input satisfying this condition, definitions included, could be constructed. This is not a proof that none exists.")
                        } else {
                            tr!(
                                "導出（{}）が入力を共有しているため、この条件を同時に満たす注文が存在するかどうかを、検査は判定できませんでした。",
                                "Because the derived values ({}) share inputs, the check could not decide whether an order satisfying this condition exists.",
                                dnames.join(if crate::i18n::ja() { "、" } else { ", " })
                            )
                        })
                        .note(tr!(
                            "存在するなら: 行を直してください。出力が異なる（{} と {}）ので、当たれば矛盾です。",
                            "If one exists: fix the rows. The outputs differ ({} vs {}), so a match would be a contradiction.",
                            outs(&t.rows[i]),
                            outs(&t.rows[j])
                        ))
                        .note(tr!(
                            "存在しないなら: このままで構いません。生成コードには、万一この条件に当たる入力が来たとき黙って 行{} を選ばずエラーを返す番人が入ります。",
                            "If none exists: leave it as is. The generated code gets a guard that, should an input ever match this condition, returns an error instead of silently picking row {}.",
                            i + 1
                        ))
                        .note(tr!("この警告は check --diff-base では新規分だけ表示されます。", "Under check --diff-base, only new instances of this warning are shown."))
                        .key(pair_key(&t.rows[i], &t.rows[j])),
                    );
                }
                Policy::Unique => {
                    let same = t.rows[i].outs.len() == t.rows[j].outs.len();
                    // If a definition column takes part in the intersection, the witness has
                    // not been checked against the definition's body. The sieve will look at
                    // definition axes in M1 (§8.5). Until then, say so rather than stay silent.
                    let unverified: Vec<String> = (0..reg.axes.len())
                        .filter(|&ai| {
                            reg.is_define[ai]
                                && (0..reg.axes[ai].len())
                                    .any(|c| reg.masks[i][ai][c] && reg.masks[j][ai][c])
                                && !(0..reg.axes[ai].len()).all(|c| reg.masks[i][ai][c] && reg.masks[j][ai][c])
                        })
                        .map(|ai| reg.col_names[ai].clone())
                        .collect();
                    out.push(
                        Diag::error("E105", tr!("行の重なり: 同じ入力が 行{} と 行{} の両方に当たります", "Overlapping rows: the same input matches row {} and row {}", i + 1, j + 1))
                            .at(at(t.rows[j].span.line))
                            .table(tname.clone())
                            .rowref(tname.clone(), i + 1)
                            .rowref(tname.clone(), j + 1)
                            .wit(pairs_to_witness(reg.witness_pairs(&wpath)))
                            .mark(t.rows[i].span.clone(), tr!("行{}", "row {}", i + 1))
                            .mark(t.rows[j].span.clone(), tr!("行{}", "row {}", j + 1))
                            .note(tr!("両方に当たる例: {w}", "Both rows match: {w}"))
                            .note(match &built {
                                // §6.2: a witness that involves definitions is shown only after
                                // an input has been constructed and confirmed by the evaluator.
                                // Present it in a form that can be copied.
                                Some(b) => tr!("この例を作る入力: {b}", "An input producing this example: {b}"),
                                None => String::new(),
                            })
                            .note(if same {
                                tr!("`{p} {u}` では重なりは許されません。どちらが正しいか決めるか、順序に意味を持たせるなら `{p} {f}` を宣言してください。", "`{p} {u}` does not allow overlapping rows. Decide which row is right, or declare `{p} {f}` if the order is meant to matter.", p = crate::kw::POLICY, u = crate::kw::UNIQUE, f = crate::kw::FIRST)
                            } else {
                                tr!("`{} {}` では重なりは許されません。", "`{} {}` does not allow overlapping rows.", crate::kw::POLICY, crate::kw::UNIQUE)
                            })
                            .note(if unverified.is_empty() || touches_define {
                                String::new()
                            } else {
                                tr!(
                                    "なお、この例は定義（{}）の中身との整合を確認していません。",
                                    "Note that this example has not been checked for consistency with the body of the definitions ({}).",
                                    unverified.join(if crate::i18n::ja() { "、" } else { ", " })
                                )
                            }),
                    );
                }
                Policy::TopDown => {
                    shadowed[j] = true;
                    overlaps.push((i, j));
                    // The three kinds of §4. Containment is decided on regions (whether row i's
                    // region minus row j's region is empty, not per-axis projections). The sieve
                    // is not applied.
                    let contained = reg.contains(j, i);
                    nodes += (reg.axes.len() * 4) as i64;
                    let same_out = outs_equal(&t.rows[i], &t.rows[j]);
                    let d = Diag::warning(
                        "W105",
                        tr!("行の重なり: 同じ入力が 行{} と 行{} の両方に当たります", "Overlapping rows: the same input matches row {} and row {}", i + 1, j + 1),
                    )
                    .at(at(t.rows[j].span.line))
                    .table(tname.clone())
                    .rowref(tname.clone(), i + 1)
                    .rowref(tname.clone(), j + 1)
                    .wit(pairs_to_witness(reg.witness_pairs(&wpath)))
                    .mark(t.rows[i].span.clone(), tr!("行{}", "row {}", i + 1))
                    .mark(t.rows[j].span.clone(), tr!("行{}", "row {}", j + 1))
                    .note(tr!("両方に当たる例: {w}", "Both rows match: {w}"))
                    .note(match &built {
                        Some(b) => tr!("この例を作る入力: {b}", "An input producing this example: {b}"),
                        None => String::new(),
                    })
                    .note(tr!("`{} {}` のため 行{} が勝ちます。意図通りですか。", "Because of `{} {}`, row {} wins. Is this intended?", crate::kw::POLICY, crate::kw::FIRST, i + 1))
                    .key(pair_key(&t.rows[i], &t.rows[j]));
                    if contained {
                        shadow.structural += 1;
                        quiet.push(d.note(tr!("行の領域が後の行に丸ごと含まれています（階段の通常の姿）。", "The row's region is entirely contained in the later row (the normal shape of a staircase).")));
                    } else if same_out {
                        shadow.equivalent += 1;
                        quiet.push(d.note(tr!("出力が同じなので、どちらが勝っても値は変わりません。", "The outputs are the same, so the value does not change whichever row wins.")));
                    } else {
                        shadow.confirm += 1;
                        out.push(d.note(
                            tr!("意図通りならこのままで構いません。CI の check --diff-base は新たに生じた分だけを報告します。", "If this is intended, leave it as is. In CI, check --diff-base reports only newly introduced instances."),
                        ));
                    }
                }
            }
        }
    }

    // --- Unreachable rows
    for i in 0..t.rows.len() {
        let dead = if reg.empty(i) {
            true
        } else if t.policy == Policy::TopDown && i > 0 {
            // First check containment in a single earlier row. Every dead row of a staircase
            // is caught here, and the test is cheap, being a per-axis subset test. Containment
            // in a union is needed only when several rows cover the row just together.
            if (0..i).any(|e| reg.contains(e, i)) {
                nodes += (i * reg.axes.len() * 4) as i64;
                true
            } else {
                // Earlier rows that do not intersect contribute nothing to the union, so drop
                // them.
                let earlier: Vec<usize> = (0..i).filter(|&e| reg.intersects(e, i)).collect();
                if earlier.is_empty() {
                    false
                } else {
                    let mut b = budget;
                    let r = reg.hole_within(&earlier, i, &mut b).is_none() && b >= 0;
                    nodes += budget - b;
                    r
                }
            }
        } else {
            false
        };
        if dead {
            dead_rows.push(i);
            out.push(
                Diag::error("E102", tr!("冗長な行: 行{} は決して当たりません", "Unreachable row: row {} never matches", i + 1))
                    .at(at(t.rows[i].span.line))
                    .table(tname.clone())
                    .row(i + 1)
                    .rowref(tname.clone(), i + 1)
                    .fix_kind(crate::diag::FixKind::RemoveRow)
                    .mark(t.rows[i].span.clone(), tr!("行{}: ここに到達する入力はありません", "row {}: no input reaches here", i + 1))
                    .note(if reg.unreachable_row[i] {
                        tr!("この行が名指ししている値を、上流の表は決して出しません。", "The upstream table never produces the values this row names.")
                    } else if t.policy == Policy::TopDown {
                        tr!("`{} {}` のため、この行の範囲は先行する行がすべて先に取ります。", "Because of `{} {}`, the earlier rows take all of this row's range first.", crate::kw::POLICY, crate::kw::FIRST)
                    } else {
                        tr!("この行の条件を同時に満たす入力がありません。", "No input satisfies all of this row's conditions at once.")
                    })
                    .note(if reg.unreachable_row[i] {
                        tr!("ヒント: 上流の表がこの値を出すようにするか、この行を削除してください。", "hint: make the upstream table produce this value, or delete this row.")
                    } else {
                        tr!("ヒント: 新しい仕様なら上へ移してください。不要なら削除してください。", "hint: if this is a new specification, move it up; if it is not needed, delete it.")
                    }),
            );
        }
    }

    // --- Completeness
    let mut left = budget;
    let all: Vec<usize> = (0..t.rows.len()).collect();
    let hole = reg.find_hole(&all, &mut left);
    nodes += budget - left;
    if let Some(hole) = hole {
        out.push(
            {
                let d = Diag::error("E101", tr!("完全性の欠落: どの行にも当たらない入力があります", "Completeness gap: some input matches no row"))
                    .at(at(head_span.line))
                    .table(tname.clone())
                    .wit(pairs_to_witness(reg.witness_pairs(&hole)))
                    .mark(head_span.clone(), tr!("入力空間を覆いきっていません", "the input space is not fully covered"))
                    .note(tr!("当たらない例: {}", "An input that matches no row: {}", reg.witness_text(&hole)))
                    .note(tr!("ヒント: この入力に当たる行を足してください。", "hint: add a row that matches this input."));
                // The rewritten form (§11 principle 3) as data: the row's input cells are the
                // witness, and the output cells are copied from the first row purely to give
                // a shape that parses. **The amount has to come from the written rule**, which
                // the note says and `fix.text` — being prose-free and language independent —
                // cannot.
                match reg.row_text(&hole, t) {
                    Some(row) => d
                        .note(tr!(
                            "足す行の形: `{row}`。出力の値は表の一行目から写した「形」で、正しい額ではありません。規約か Excel か旧実装のどれが出どころかを決めて、そこから書いてください。この一行が閉じるのはこの証人の穴だけで、まだ残っていれば次の証人が出ます。",
                            "The shape of the row to add: `{row}`. Its output values are copied from the first row to give a shape that parses; they are not the right amounts. Decide whether the written rule, the spreadsheet or the legacy implementation is the source, and take them from there. One row closes the gap this witness names; if more is left, the next run names the next one."
                        ))
                        .fix(crate::diag::FixKind::AddRow, row),
                    None => d,
                }
            },
        );
    } else if left < 0 {
        out.push(
            Diag::error("E109", tr!("検査の予算を超えたので、完全性を証明できませんでした", "The check exceeded its budget, so completeness could not be proven"))
                .at(at(head_span.line))
                .table(tname.clone())
                .mark(head_span.clone(), "")
                .note(tr!("支配的なのは {}。", "The dominant columns are {}.", reg.dominant_axes()))
                .note(tr!("列を群でまとめるか、表を分けてください（§6.3）。近似では通しません。", "Combine columns into groups or split the table (§6.3). No approximation is accepted in its place.")),
        );
    }

    // --- W110: a `policy first` table with no overlaps
    if t.policy == Policy::TopDown && !shadowed.iter().any(|x| *x) && t.rows.len() > 1 {
        out.push(
            Diag::warning("W110", tr!("この表には重なりがありません", "This table has no overlapping rows"))
                .at(at(head_span.line))
                .table(tname.clone())
                .fix(crate::diag::FixKind::ChangePolicy, format!("{} {}", crate::kw::POLICY, crate::kw::UNIQUE))
                .mark(head_span.clone(), "")
                .note(tr!("`{} {}` にすると、行の並べ替えが意味を変えないことを検査が保証します。", "With `{} {}`, the checker guarantees that reordering the rows does not change the meaning.", crate::kw::POLICY, crate::kw::UNIQUE)),
        );
    }
    TableCheck { diags: out, w114, quiet, shadow, nodes, overlaps, dead: dead_rows }
}

impl TableRegion {
    /// Find a coordinate in the region of row `target` that `rows` do not cover.
    fn hole_within(&self, rows: &[usize], target: usize, budget: &mut i64) -> Option<Vec<usize>> {
        let mut path = Vec::new();
        self.within_rec(rows, target, 0, budget, &mut path)
    }

    fn within_rec(
        &self,
        rows: &[usize],
        target: usize,
        ai: usize,
        budget: &mut i64,
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if rows.is_empty() {
            let mut p = path.clone();
            while p.len() < self.axes.len() {
                p.push(0);
            }
            return Some(p);
        }
        if ai == self.axes.len() {
            return None;
        }
        if rows
            .iter()
            .any(|&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x)))
        {
            return None;
        }
        for c in 0..self.axes[ai].len() {
            if !self.masks[target][ai][c] {
                continue;
            }
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            if let Some(h) = self.within_rec(&sub, target, ai + 1, budget, path) {
                path.pop();
                return Some(h);
            }
            path.pop();
        }
        None
    }
}

impl TableRegion {
    fn coord_span(&self, ai: usize, ci: usize) -> Option<(Option<Rat>, Option<Rat>)> {
        match &self.axes[ai] {
            Axis::Num { coords, .. } => match coords.get(ci)? {
                Coord::Point(v) => Some((Some(*v), Some(*v))),
                Coord::Open(a, b) => Some((*a, *b)),
            },
            _ => None,
        }
    }

    /// The sieve of §6.2. For each derived axis, check whether the reachable interval and the
    /// coordinate's interval intersect. If they do not, the point is infeasible. When two or
    /// more constrained derived values share an input, their dependency cannot be examined.
    pub fn feasible(&self, path: &[usize]) -> Feasible {
        let mut constrained: Vec<(usize, Vec<String>)> = Vec::new();
        for (ai, d) in self.derived.iter().enumerate() {
            let Some(((rl, rh), deps)) = d else { continue };
            let Some(&ci) = path.get(ai) else { continue };
            let Some((cl, ch)) = self.coord_span(ai, ci) else { continue };
            let disjoint = matches!((ch, rl), (Some(a), Some(b)) if a.cmp_to(*b) == std::cmp::Ordering::Less)
                || matches!((cl, rh), (Some(a), Some(b)) if a.cmp_to(*b) == std::cmp::Ordering::Greater);
            if disjoint {
                return Feasible::No;
            }
            if !(cl.is_none() && ch.is_none()) {
                constrained.push((ai, deps.clone()));
            }
        }
        for i in 0..constrained.len() {
            for j in (i + 1)..constrained.len() {
                if constrained[i].1.iter().any(|x| constrained[j].1.contains(x)) {
                    return Feasible::Unknown;
                }
            }
        }
        Feasible::Yes
    }
}

impl TableRegion {
    pub fn derived_names(&self) -> Vec<String> {
        self.col_names
            .iter()
            .enumerate()
            .filter(|(i, _)| self.derived[*i].is_some())
            .map(|(_, n)| n.clone())
            .collect()
    }
}

impl TableRegion {
    /// §6.3: when the budget runs out, name the columns whose boundary counts dominate. This
    /// decides whether to aim for "combine into groups" or "split the table".
    pub fn dominant_axes(&self) -> String {
        let mut v: Vec<(usize, usize)> = (0..self.axes.len()).map(|i| (self.axes[i].len(), i)).collect();
        v.sort_by(|a, b| b.0.cmp(&a.0));
        v.iter()
            .take(3)
            .map(|(n, i)| tr!("{}（{n} 区分）", "{} ({n} segments)", self.col_names[*i]))
            .collect::<Vec<_>>()
            .join(if crate::i18n::ja() { "、" } else { ", " })
    }
}
