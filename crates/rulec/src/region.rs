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
use std::collections::{BTreeMap, BTreeSet};

/// A closed interval with open ends, as the axes and the ranges both carry it.
type Ival = (Option<Rat>, Option<Rat>);

/// Both ends or nothing: an unbounded end makes the result unbounded.
fn both(a: Option<Rat>, b: Option<Rat>, f: impl Fn(Rat, Rat) -> Rat) -> Option<Rat> {
    match (a, b) {
        (Some(x), Some(y)) => Some(f(x, y)),
        _ => None,
    }
}

/// The value of an interval that is a single point, which is what §5 allows on one side of `×`.
fn point_of(v: Ival) -> Option<Rat> {
    match v {
        (Some(a), Some(b)) if a.cmp_to(b) == std::cmp::Ordering::Equal => Some(a),
        _ => None,
    }
}

/// A negative constant swaps the ends.
fn scale(v: Ival, k: Rat) -> Ival {
    let (lo, hi) = (v.0.map(|x| x.mul(k)), v.1.map(|x| x.mul(k)));
    if k.num < 0 { (hi, lo) } else { (lo, hi) }
}

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
    /// machinery applies unchanged; `date` is what says to write it back as `YYYY-MM-DD`. It
    /// used to be "the unit is empty", which left `number` — the one numeric type with no unit
    /// at all — nowhere to sit, so it was given `%` and every witness on a count read as a
    /// percentage.
    ///
    /// Coordinates are true values. A rate is written in percent and travels as a count of
    /// steps (§10.2), and those two factors differ once the step is finer than 1%, so both
    /// are carried: `shown` to write the value back into a cell, `wire` to put it in a
    /// witness. Everything else has 1 for both. `step` is the axis's own grid, which is what
    /// keeps a witness on a value the type can actually hold.
    Num { unit: String, date: bool, coords: Vec<Coord>, shown: i128, wire: i128, step: Rat },
    Bool,
}

/// A value strictly inside an open coordinate, on the axis's own grid. The midpoint is not
/// on it — `(10%, 30%)` has midpoint 20%, but `(10%, 15%)` has 12.5%, which `rate[step 1%]`
/// cannot hold — and a witness that cannot be written back into a cell is no use to whoever
/// has to close the gap. One grid unit in from the closed side is always representable, and
/// it is the value most likely to show an off-by-one (§11 principle 2). Dates always used
/// this; it is the same rule.
fn inside(a: Option<Rat>, b: Option<Rat>, step: Rat) -> Rat {
    match (a, b) {
        (Some(a), _) => a.add(step),
        (None, Some(b)) => b.sub(step),
        (None, None) => Rat::zero(),
    }
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
            Axis::Num { date: true, coords, step, .. } => match coords.get(i) {
                // For dates, prefer a boundary (an actual calendar day). The midpoint of an
                // interval need not be one.
                Some(Coord::Point(v)) => {
                    let (y, m, d) = crate::types::ord_to_date(*v);
                    format!("{y:04}-{m:02}-{d:02}")
                }
                Some(Coord::Open(a, b)) => {
                    // Dates are serial day numbers, so an open interval always contains a real
                    // calendar day. Print that day instead of hedging with "around".
                    let (y, m, d) = crate::types::ord_to_date(inside(*a, *b, *step));
                    format!("{y:04}-{m:02}-{d:02}")
                }
                None => String::new(),
            },
            Axis::Num { unit, coords, shown, step, .. } => {
                let w = |v: Rat| format!("{}{unit}", v.mul(Rat::int(*shown)));
                match coords.get(i) {
                    Some(Coord::Point(v)) => w(*v),
                    Some(Coord::Open(a, b)) => w(inside(*a, *b, *step)),
                    None => String::new(),
                }
            }
        }
    }

    /// The witness value as a plain number on the axis's own scale: the coordinate a
    /// point names stands for this value, and the bounds in the certificate are written on
    /// the same scale. `None` where the coordinate stands for no number at all — an enum,
    /// a flag — which is also every coordinate the sieve has nothing to say about.
    ///
    /// `witness_val` below is the reader's form: a rate comes out as its wire count and a
    /// date as `YYYY-MM-DD`, neither of which can be compared with a bound. The re-checker
    /// needs the number, so the certificate carries both (§15.97).
    pub(crate) fn witness_num(&self, i: usize) -> Option<Rat> {
        match self {
            Axis::Enum { .. } | Axis::Bool => None,
            Axis::Num { coords, step, .. } => match coords.get(i) {
                Some(Coord::Point(v)) => Some(*v),
                Some(Coord::Open(a, b)) => Some(inside(*a, *b, *step)),
                None => None,
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
            Axis::Num { date: true, .. } => WVal::Str(self.witness(i)),
            Axis::Num { coords, wire, step, .. } => {
                let v = match coords.get(i) {
                    Some(Coord::Point(v)) => *v,
                    Some(Coord::Open(a, b)) => inside(*a, *b, *step),
                    None => Rat::zero(),
                };
                WVal::Int(crate::types::wire_int(v, *wire))
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
    /// Every `derive` in the rule, by name. A derived column is a **linear combination of
    /// inputs** (§5), so the interval it can actually reach is decidable by arithmetic on the
    /// intervals of those inputs — which is what the per-axis sieve does not look at.
    exprs: BTreeMap<String, Expr>,
    /// The declared range of every name, for the ones this table has no axis for.
    spans: BTreeMap<String, Ival>,
    /// Row → axis → whether each coordinate is selected.
    masks: Vec<Vec<Vec<bool>>>,
    /// The rule's `constraint` lines (§15.55). A box that no input satisfying them can reach
    /// is not a gap and not an overlap: it is a combination the caller says does not happen.
    constraints: Vec<crate::ast::Constraint>,
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
    let push = |r: Rat, s: &mut BTreeSet<(i128, i128)>| {
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
    // A boundary a cell names beyond the declared range is not a coordinate: the range is
    // the universe, and what lies outside it no input reaches. Left in, an interval between
    // the range's end and such a boundary would be demanded by the completeness check and
    // a row lying wholly beyond the range would look reachable.
    let inside = |x: &Rat| {
        lo.is_none_or(|l| x.cmp_to(l) != std::cmp::Ordering::Less) && hi.is_none_or(|h| x.cmp_to(h) != std::cmp::Ordering::Greater)
    };
    let mut v: Vec<Rat> = set.into_iter().map(|(n, d)| Rat { num: n, den: d }).filter(inside).collect();
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

/// The table that decides a column, when one does: the merged table of the set that defines
/// it, which is the table itself when it stands alone.
fn producing_table<'a>(f: &'a RuleFile, c: &'a Checked, t: &Table, k: &str) -> Option<&'a Table> {
    let mine = t.name.as_ref().map(|n| n.text.as_str());
    if let Some(s) = c.sets.iter().find(|s| s.table.outputs.iter().any(|o| o.name.text == k)) {
        let its = s.table.name.as_ref().map(|n| n.text.as_str());
        if its != mine && !s.members.iter().any(|m| Some(m.as_str()) == mine) {
            return Some(&s.table);
        }
        return None;
    }
    f.items.iter().find_map(|it| match it {
        Item::Table(u)
            if u.name.as_ref().map(|n| &n.text) != t.name.as_ref().map(|n| &n.text)
                && u.outputs.iter().any(|o| o.name.text == k) =>
        {
            Some(u)
        }
        _ => None,
    })
}

/// A column an upstream table decides, prepared once for the whole table below it.
struct Upstream<'a> {
    /// The column's name.
    name: String,
    /// Position of the column among this table's own columns.
    ci: usize,
    table: &'a Table,
    reg: TableRegion,
    /// Position of the column among the upstream table's outputs.
    oi: usize,
    ty: Ty,
}

/// Every column of `t` that a table above it decides.
fn upstreams<'a>(t: &'a Table, c: &'a Checked, f: &'a RuleFile) -> Vec<Upstream<'a>> {
    let mut out = Vec::new();
    for (ci, (k, _)) in t.inputs.iter().enumerate() {
        let Some(u) = producing_table(f, c, t, k) else { continue };
        let Some(oi) = u.outputs.iter().position(|o| o.name.text == *k) else { continue };
        let Some(reg) = TableRegion::build(u, c, f) else { continue };
        let Some(ty) = c.ty_of(k) else { continue };
        out.push(Upstream { name: k.clone(), ci, table: u, reg, oi, ty });
    }
    out
}

/// Whether the tables **above** this row rule its combination out.
///
/// `reachable` asks, per column, which values an upstream table can produce **at all**. It
/// cannot see that a value and another column exclude each other: one value of 加算種別 sends
/// the table above to a different answer, so that answer and that value never arrive together,
/// and a row naming both is dead however live each half looks on its own.
///
/// The reading is loose in the safe direction. A producing row counts as possible unless this
/// row's own cells contradict it outright, or a **single** earlier row of that table covers
/// whatever is left of it — a union of earlier rows is not subtracted, and the answer is not
/// chased further upstream than one table. So "no producer" really means none, and E102 is
/// never handed a row that is alive.
fn upstream_dead(row: &Row, ups: &[Upstream], c: &Checked, t: &Table) -> bool {
    upstream_blocked(ups, c, &|up: &Upstream| {
        let kcell = row.cells.get(up.ci);
        if matches!(kcell, None | Some(Cell::DontCare)) {
            return None;
        }
        // This row's own cells, read on the axes of the table above.
        let allowed: Vec<Vec<bool>> = up
            .reg
            .col_names
            .iter()
            .enumerate()
            .map(|(ai, n)| {
                let dc = t.inputs.iter().position(|(m, _)| m == n).and_then(|i| row.cells.get(i));
                match c.ty_of(n) {
                    Some(ty) => cell_mask(&up.reg.axes[ai], dc, &ty, c),
                    None => vec![true; up.reg.axes[ai].len()],
                }
            })
            .collect();
        Some((kcell.cloned(), allowed))
    })
}

/// The shared half: given, per upstream column, what the caller fixes, decide whether **no**
/// row of the table above can produce a value the caller allows.
///
/// `restrict` returns `None` for a column the caller leaves free, and otherwise the cell it
/// pins the column to together with that same restriction read on the upstream table's axes.
fn upstream_blocked(
    ups: &[Upstream],
    c: &Checked,
    restrict: &dyn Fn(&Upstream) -> Option<(Option<Cell>, Vec<Vec<bool>>)>,
) -> bool {
    for up in ups {
        let Some((kcell, allowed)) = restrict(up) else { continue };
        let kcell = kcell.as_ref();
        let mut producible = false;
        for ri in 0..up.table.rows.len() {
            let v = match up.table.rows[ri].outs.get(up.oi) {
                // A bare word in an output cell is either an enum value or the name of
                // something declared (§3.2); the parser cannot tell them apart and writes
                // both as `Name`. Only the first reading names a value this check can
                // compare. A declared name stands for whatever it holds at run time, so the
                // value that row produces is not known here, and the row below must be
                // assumed reachable. The evaluator resolves the same two readings the same
                // way round, by looking the binding up first (`eval.rs`).
                Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w))
                    if !c.syms.contains_key(w) =>
                {
                    w.clone()
                }
                _ => {
                    // An output this reading cannot name is assumed to reach the row.
                    producible = true;
                    break;
                }
            };
            let val = crate::eval::Val::Enum(v);
            if !kcell.is_none_or(|cell| crate::eval::cell_matches(c, cell, &val, &up.ty)) {
                continue;
            }
            // What is left of that row once this one's cells are imposed.
            let live: Vec<Vec<bool>> = (0..up.reg.axes.len())
                .map(|a| {
                    (0..up.reg.axes[a].len())
                        .map(|x| up.reg.masks[ri][a][x] && allowed[a][x])
                        .collect()
                })
                .collect();
            if live.iter().any(|m| !m.iter().any(|x| *x)) {
                continue;
            }
            let blocked = up.table.policy == Policy::TopDown
                && (0..ri).any(|e| {
                    (0..up.reg.axes.len()).all(|a| {
                        (0..up.reg.axes[a].len()).all(|x| !live[a][x] || up.reg.masks[e][a][x])
                    })
                });
            if !blocked {
                producible = true;
                break;
            }
        }
        if !producible {
            return true;
        }
    }
    false
}

/// Which coordinates of an axis a cell selects.
///
/// Pulled out of the region build so the same reading can be applied to a cell from **another**
/// table: deciding whether an upstream table can produce a value takes holding its rows against
/// the constraints of the row downstream, and those are cells on the same columns.
fn cell_mask(axis: &Axis, cell: Option<&Cell>, ty: &Ty, c: &Checked) -> Vec<bool> {
    let n = axis.len();
    let mut v = vec![false; n];
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
    v
}

impl TableRegion {
    pub fn build(t: &Table, c: &Checked, f: &RuleFile) -> Option<TableRegion> {
        let inputs = &f.inputs;
        let mut exprs: BTreeMap<String, Expr> = BTreeMap::new();
        for it in &f.items {
            if let Item::Derived(d) = it {
                exprs.insert(d.name.text.clone(), d.expr.clone());
            }
        }
        let spans: BTreeMap<String, Ival> =
            c.ranges.iter().map(|(k, v)| (k.clone(), *v)).collect();
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
                    Axis::Num {
                        unit: String::new(),
                        date: true,
                        coords: num_coords(&b, lo, hi, Rat::int(1)),
                        shown: 1,
                        wire: 1,
                        step: Rat::int(1),
                    }
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
                    let range = inputs.iter().find(|i| i.name.text == *name).and_then(|i| i.range.clone());
                    let (mut b, mut lo, mut hi) = num_bounds(&t.rows, ci, &ty, &range);
                    // A count is bounded by construction — never negative, never longer than
                    // the sequence the guard caps — and it declares that bound (§15.58). The
                    // axis takes it, or the completeness check would ask for a row covering a
                    // count of −1.
                    let counted = f.items.iter().find_map(|it| match it {
                        Item::Count(d) if d.name.text == *name => c.ranges.get(name).copied(),
                        _ => None,
                    });
                    if let Some((clo, chi)) = counted {
                        for v in [clo, chi].into_iter().flatten() {
                            b.push(v);
                        }
                        b.sort_by(|x, y| x.cmp_to(*y));
                        b.dedup_by(|x, y| x.cmp_to(*y) == std::cmp::Ordering::Equal);
                        lo = clo.or(lo);
                        hi = chi.or(hi);
                    }
                    let unit = match &ty {
                        Ty::Money { cur, .. } => cur.clone(),
                        Ty::Qty { unit, .. } => unit.clone(),
                        Ty::Rate => "%".into(),
                        // `number` counts something the rule never names, so there is nothing to
                        // write after the digits (§2.1).
                        _ => String::new(),
                    };
                    // The runtime representation is a single integer in the declared unit
                    // (§7.1), so the step is 1 for money and quantities and the declared step
                    // for rates.
                    let q = match &ty {
                        Ty::Rate => Rat::new(1, *c.scales.get(name).unwrap_or(&100)),
                        _ => Rat::int(1),
                    };
                    Axis::Num {
                        unit,
                        date: false,
                        coords: num_coords(&b, lo, hi, q),
                        // A rate is written in percent whatever its step is.
                        shown: if matches!(ty, Ty::Rate) { 100 } else { 1 },
                        wire: c.wire_scale(name),
                        step: q,
                    }
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
                let ty = c.ty_of(&col_names[ai])?;
                let v = cell_mask(axis, row.cells.get(cell_of[ai]), &ty, c);
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
        Some(TableRegion {
            axes,
            col_names,
            unreachable_row,
            display_of: cell_of,
            is_define,
            derived,
            exprs,
            spans,
            masks,
            unanalyzable,
            constraints: f.constraints.clone(),
        })
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
    fn find_hole(&self, rows: &[usize], budget: &mut i64, ups: &[Upstream], c: &Checked) -> Option<Vec<usize>> {
        self.hole_rec(rows, 0, budget, &mut Vec::new(), ups, c)
    }

    fn hole_rec(
        &self,
        rows: &[usize],
        ai: usize,
        budget: &mut i64,
        path: &mut Vec<usize>,
        ups: &[Upstream],
        chk: &Checked,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if rows.is_empty() {
            // A gap proven infeasible is not reported. An undecidable gap is kept (we only ever
            // drop in the direction of over-reporting; §6.1). The tables above count here too:
            // demanding a row for a combination they cannot produce would contradict the E102
            // that names such a row dead.
            //
            // **Every point of the box is asked about, not one corner of it** (§15.98). Padding
            // the path with zeros and sieving that single point dismissed a whole subtree
            // whenever the corner happened to be impossible — a `constraint` that forbids
            // (甲=1, 乙=0) hid the gap at (甲=1, 乙=1), and `check` said ok while the generated
            // code hit its own `unreachable!`.
            return self.first_reachable(path, ups, chk, budget);
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
            if let Some(h) = self.hole_rec(&sub, ai + 1, budget, path, ups, chk) {
                path.pop();
                return Some(h);
            }
            path.pop();
        }
        None
    }

    /// The first point of this box the sieve does not rule out, or nothing when it rules
    /// out every one of them (§15.98). Called only where no row takes the box, so the point
    /// it finds is a gap and the emptiness it reports is a box no input reaches.
    ///
    /// The box is rejected as a whole first, which is one interval test and settles the
    /// common case; the walk below only runs where that is not enough, and it is charged to
    /// the same budget as the rest of the search, so an overrun becomes E109 rather than a
    /// silent pass.
    fn first_reachable(
        &self,
        path: &[usize],
        ups: &[Upstream],
        chk: &Checked,
        budget: &mut i64,
    ) -> Option<Vec<usize>> {
        if self.feasible(path) == Feasible::No {
            return None;
        }
        let mut p = path.to_vec();
        self.first_reachable_rec(&mut p, ups, chk, budget)
    }

    fn first_reachable_rec(
        &self,
        p: &mut Vec<usize>,
        ups: &[Upstream],
        chk: &Checked,
        budget: &mut i64,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if p.len() == self.axes.len() {
            if self.feasible(p) == Feasible::No || self.upstream_dead_at(p, ups, chk) {
                return None;
            }
            return Some(p.clone());
        }
        let ai = p.len();
        for c in 0..self.axes[ai].len() {
            p.push(c);
            let keep = self.feasible(p) != Feasible::No;
            let got = if keep { self.first_reachable_rec(p, ups, chk, budget) } else { None };
            p.pop();
            if got.is_some() {
                return got;
            }
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
    /// The pairs a declared precedence orders across two tables (0-based, the winning row
    /// first), with whether the winner lies inside the loser: an exception, or a rule that
    /// reaches beyond what it takes precedence over. The approver's page tells them apart.
    pub edge_pairs: Vec<(usize, usize, bool)>,
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
/// The checks of one table on its own. The unit of checking is the definition set
/// (§15.66); this is the set of one table, kept for callers that hold a table.
pub fn check_table(t: &Table, c: &Checked, f: &RuleFile, path: &str, budget: i64) -> TableCheck {
    match c.set_of_table(t) {
        Some(s) if s.members.len() == 1 => check_set(s, c, f, path, budget),
        _ => check_set(&crate::defset::single(t), c, f, path, budget),
    }
}

/// The checks of one definition set: completeness of the union, decisiveness of every
/// overlap, reachability of every row, on the merged table (§15.66).
///
/// For a set of one table this is exactly the check the table always had: the rows come in
/// their own order, and the precedence relation is the policy's.
pub fn check_set(set: &crate::defset::DefSet, c: &Checked, f: &RuleFile, path: &str, budget: i64) -> TableCheck {
    let t = &set.table;
    let inputs = &f.inputs;
    let mut out = Vec::new();
    let mut quiet = Vec::new();
    let mut shadow = Shadow::default();
    let mut nodes = 0i64;
    let mut w114: Vec<(usize, usize)> = Vec::new();
    let mut overlaps: Vec<(usize, usize)> = Vec::new();
    let mut edge_pairs: Vec<(usize, usize, bool)> = Vec::new();
    let mut dead_rows: Vec<usize> = Vec::new();
    let empty = TableCheck {
        diags: Vec::new(),
        w114: Vec::new(),
        quiet: Vec::new(),
        shadow,
        nodes: 0,
        overlaps: Vec::new(),
        edge_pairs: Vec::new(),
        dead: Vec::new(),
    };
    let _ = inputs;
    let Some(reg) = TableRegion::build(t, c, f) else { return empty };
    if reg.axes.is_empty() || t.rows.is_empty() {
        return empty;
    }
    let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    if let Some((col, ty)) = &reg.unanalyzable {
        return TableCheck {
            w114: Vec::new(),
            diags: vec![
                Diag::error("E110", tr!("列 {col} の型 {ty} は、まだ検査できません", "Column {col} has type {ty}, which cannot be checked yet"))
                    .at(tr!("{path}:{} 表 {tname}", "{path}:{} table {tname}", t.span.line))
                    .table(tname.clone())
                    .mark(t.span.clone(), "")
                    .note(tr!("この表の完全性も重なりも検査していません。黙って通すより止めます。", "Neither the completeness nor the overlaps of this table have been checked. Stopping is better than passing it silently."))
                    .note(tr!("列の型を、列挙・真偽・数量・金額・率・日付・それらの optional のいずれかにしてください。", "Give the column one of these types: an enum, boolean, quantity, money, rate, date, or an optional of one of those.")),
            ],
            quiet: Vec::new(),
            shadow: Shadow::default(),
            nodes: 0,
            overlaps: Vec::new(),
            edge_pairs: Vec::new(),
            dead: Vec::new(),
        };
    }
    let merged = set.merged();
    // Where a diagnostic points: the file, the line, and the table (or clause) the row was
    // written in.
    let at = |line: usize, mi: usize| tr!("{path}:{line} {} {}", "{path}:{line} {} {}", set.kind_word(mi), set.members[mi]);
    // How a row is named. A row of a set of one table is `行3`; in a merged set the table is
    // named too, and a labelled row shows its label beside the number. A clause is named as
    // a clause: it has one row, and the row is the clause.
    let rn = |i: usize| -> String {
        let r = &t.rows[i];
        if set.is_clause_row(i) {
            return tr!("節 {}", "clause {}", set.row_table(i));
        }
        let base = if merged {
            tr!("表 {} 行{}", "table {} row {}", set.row_table(i), r.index)
        } else {
            tr!("行{}", "row {}", r.index)
        };
        match &r.label {
            Some(l) => tr!("{base}（{}）", "{base} ({})", l.text),
            None => base,
        }
    };
    let head_span: Span = t.name.as_ref().map(|n| n.span.clone()).unwrap_or(t.span.clone());
    let members_text = || (0..set.members.len()).map(|mi| format!("{} {}", set.kind_word(mi), set.members[mi])).collect::<Vec<_>>().join(if crate::i18n::ja() { "、" } else { ", " });

    // --- Overlaps and shadowing
    let mut shadowed = vec![false; t.rows.len()];
    let mut effective = vec![false; set.edges.len()];
    for i in 0..t.rows.len() {
        for j in (i + 1)..t.rows.len() {
            if !reg.intersects(i, j) {
                continue;
            }
            let mut wpath = Vec::new();
            for ai in 0..reg.axes.len() {
                let c0 = (0..reg.axes[ai].len())
                    .find(|&c| reg.masks[i][ai][c] && reg.masks[j][ai][c])
                    .unwrap_or(0);
                wpath.push(c0);
            }
            let w = reg.witness_text(&wpath);
            let feas = reg.feasible(&wpath);
            // An overlap proven infeasible is not reported (§6.2). The point-wise sieve is
            // joined by the arithmetic of the derived columns over the whole intersection.
            if feas == Feasible::No || reg.derived_conflict(i, j) {
                continue;
            }
            // §6.2 "witnesses on definition axes": the intersection box merely places its
            // coordinate on a definition axis freely, without checking that some input
            // actually produces that truth value. So we construct a real input, let the
            // evaluator compute the definitions too, and treat **only the overlaps we could
            // construct** as real contradictions. Failing to construct one is not a proof of
            // non-existence, so instead of asserting anything we demote to W114.
            // A box places its coordinate freely on any axis that is not a free input — a
            // definition's truth value, and a derived value too. The sieve only holds a derived
            // axis against *its own* declared range, so `a = 1` next to `d < 0` survives it even
            // though `d = b - a` and `b >= 1` leave `d >= 0` there. Both kinds
            // therefore go through the same door: construct a real input, let the evaluator
            // compute the definitions and the derived values, and treat only what could be
            // constructed as real.
            let free_axis = (0..reg.axes.len()).any(|ai| {
                (reg.is_define[ai] || reg.derived[ai].is_some())
                    && (0..reg.axes[ai].len()).any(|cc| reg.masks[i][ai][cc] && reg.masks[j][ai][cc])
            });
            let touches_define = (0..reg.axes.len()).any(|ai| {
                reg.is_define[ai]
                    && (0..reg.axes[ai].len()).any(|cc| reg.masks[i][ai][cc] && reg.masks[j][ai][cc])
            });
            let mut feas = feas;
            let mut built: Option<String> = None;
            if free_axis && feas != Feasible::No {
                nodes += (reg.axes.len() * 8) as i64;
                match crate::vectors::pair_witness(f, c, t, i, j) {
                    Some(a) => {
                        feas = Feasible::Yes;
                        built = Some(
                            a.iter()
                                .map(|(n, v)| format!("{n} = {}", crate::vectors::show_named(c, n, v)))
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                    }
                    None => feas = Feasible::Unknown,
                }
            }
            // What orders the pair: the policy of their table when they share one, the
            // declared precedence when they do not. Rows come in evaluation order, so an
            // ordered pair `i < j` always has `i` winning.
            let same = set.same_member(i, j);
            let ordered = set.comparable(i, j);
            let tn_j = set.row_table(j).to_string();
            if !same && ordered {
                // A declared precedence between two tables. The overlap is what the
                // `overrides` line is for, so nothing is reported; the pair is kept for the
                // coverage obligation and for the approver's page, which says whether the
                // winner lies inside the loser (an exception) or reaches beyond it.
                if feas != Feasible::Unknown {
                    overlaps.push((i, j));
                    let contained = reg.contains(j, i);
                    nodes += (reg.axes.len() * 4) as i64;
                    edge_pairs.push((i, j, contained));
                    for (k, e) in set.edges.iter().enumerate() {
                        if set.member_of[i] == e.winner
                            && set.member_of[j] == e.loser
                            && e.loser_row.is_none_or(|r| r == j)
                        {
                            effective[k] = true;
                        }
                    }
                }
                continue;
            }
            let policy = if same { set.policy_of(j) } else { Policy::Unique };
            match policy {
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
                            tr!("未確認の重なり: {} と {} の両方に当てはまる入力が有り得ます", "Unconfirmed overlap: an input may match both {} and {}", rn(i), rn(j)),
                        )
                        .at(at(t.rows[j].span.line, set.member_of[j]))
                        .table(tn_j.clone())
                        .rowref(set.row_table(i).to_string(), t.rows[i].index)
                        .rowref(tn_j.clone(), t.rows[j].index)
                        .wit(pairs_to_witness(reg.witness_pairs(&wpath)))
                        .mark(t.rows[i].span.clone(), rn(i))
                        .mark(t.rows[j].span.clone(), rn(j))
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
                            "存在するなら: 行を直してください。出力が異なる（{} と {}）ので、当てはまれば矛盾です。",
                            "If one exists: fix the rows. The outputs differ ({} vs {}), so a match would be a contradiction.",
                            outs(&t.rows[i]),
                            outs(&t.rows[j])
                        ))
                        .note(if same {
                            tr!(
                                "存在しないなら: このままで構いません。生成コードには、万一この条件に当てはまる入力が来たとき黙って {} を選ばずエラーを返すガードが入ります。",
                                "If none exists: leave it as is. The generated code gets a guard that, should an input ever match this condition, returns an error instead of silently picking {}.",
                                rn(i)
                            )
                        } else {
                            tr!(
                                "存在しないなら: このままで構いません。存在するなら、どちらが優先するかを、優先する表の `{o} {}` の行に書いてください。それまでは生成コードにガードが入ります。",
                                "If none exists: leave it as is. If one does, say which takes precedence with an `{o} {}` line on the table that wins. Until then the generated code carries a guard.",
                                set.row_table(i),
                                o = crate::kw::OVERRIDES
                            )
                        })
                        .note(tr!("この警告は check --diff-base では新規分だけ表示されます。", "Under check --diff-base, only new instances of this warning are shown."))
                        .key(pair_key(&t.rows[i], &t.rows[j])),
                    );
                }
                Policy::Unique => {
                    let same_len = t.rows[i].outs.len() == t.rows[j].outs.len();
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
                        Diag::error("E105", tr!("行の重なり: 同じ入力が {} と {} の両方に当てはまります", "Overlapping rows: the same input matches {} and {}", rn(i), rn(j)))
                            .at(at(t.rows[j].span.line, set.member_of[j]))
                            .table(tn_j.clone())
                            .rowref(set.row_table(i).to_string(), t.rows[i].index)
                            .rowref(tn_j.clone(), t.rows[j].index)
                            .wit(pairs_to_witness(reg.witness_pairs(&wpath)))
                            .mark(t.rows[i].span.clone(), rn(i))
                            .mark(t.rows[j].span.clone(), rn(j))
                            .note(tr!("両方に当てはまる例: {w}", "Both rows match: {w}"))
                            .note(match &built {
                                // §6.2: a witness that involves definitions is shown only after
                                // an input has been constructed and confirmed by the evaluator.
                                // Present it in a form that can be copied.
                                Some(b) => tr!("この例を作る入力: {b}", "An input producing this example: {b}"),
                                None => String::new(),
                            })
                            .note(if !same {
                                tr!(
                                    "表 {} と 表 {} のどちらが優先するか書かれていません。優先する表の `{p}` の次に `{o} <相手>` を書いてください。順序に意味が無いなら、どちらかの行を直してください。",
                                    "It is not written whether table {} or table {} takes precedence. Put `{o} <the other>` after `{p}` on the table that wins. If no order is meant, fix one of the rows.",
                                    set.row_table(i),
                                    set.row_table(j),
                                    p = crate::kw::POLICY,
                                    o = crate::kw::OVERRIDES
                                )
                            } else if same_len {
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
                    // A pair whose point could not be constructed is not presented as a shadow:
                    // no witness, and no coverage obligation to reach a point that may not
                    // exist. Under `first` the earlier row wins either way, so there is nothing
                    // to guard — the demotion costs the reader nothing here.
                    let confirmed = feas != Feasible::Unknown;
                    if confirmed {
                        shadowed[j] = true;
                        overlaps.push((i, j));
                    }
                    // The three kinds of §4. Containment is decided on regions (whether row i's
                    // region minus row j's region is empty, not per-axis projections). The sieve
                    // is not applied.
                    let contained = reg.contains(j, i);
                    nodes += (reg.axes.len() * 4) as i64;
                    let same_out = outs_equal(&t.rows[i], &t.rows[j]);
                    let d = Diag::warning(
                        "W105",
                        tr!("行の重なり: 同じ入力が {} と {} の両方に当てはまります", "Overlapping rows: the same input matches {} and {}", rn(i), rn(j)),
                    )
                    .at(at(t.rows[j].span.line, set.member_of[j]))
                    .table(tn_j.clone())
                    .rowref(tn_j.clone(), t.rows[i].index)
                    .rowref(tn_j.clone(), t.rows[j].index)
                    .mark(t.rows[i].span.clone(), rn(i))
                    .mark(t.rows[j].span.clone(), rn(j));
                    let d = if confirmed {
                        d.wit(pairs_to_witness(reg.witness_pairs(&wpath)))
                            .note(tr!("両方に当てはまる例: {w}", "Both rows match: {w}"))
                            .note(match &built {
                                Some(b) => tr!("この例を作る入力: {b}", "An input producing this example: {b}"),
                                None => String::new(),
                            })
                    } else {
                        d.note(tr!(
                            "両方に当てはまる入力は構成できませんでした。存在しないことの証明ではありません。",
                            "No input matching both could be constructed. This is not a proof that none exists."
                        ))
                    };
                    let d = d
                    .note(tr!("`{} {}` のため {} が勝ちます。意図通りですか。", "Because of `{} {}`, {} wins. Is this intended?", crate::kw::POLICY, crate::kw::FIRST, rn(i)))
                    .key(pair_key(&t.rows[i], &t.rows[j]));
                    if contained {
                        shadow.structural += 1;
                        quiet.push(d.note(tr!("この行の範囲が、後の行にまるごと含まれています（階段としてよくある形です）。", "The row's region is entirely contained in the later row (the normal shape of a staircase).")));
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

    // --- W117: a declared precedence that no pair of rows exercises
    for (k, e) in set.edges.iter().enumerate() {
        if effective[k] {
            continue;
        }
        let winner = format!("{} {}", set.kind_word(e.winner), set.members[e.winner]);
        let loser = match e.loser_row {
            Some(r) => rn(r),
            None => format!("{} {}", set.kind_word(e.loser), set.members[e.loser]),
        };
        out.push(
            Diag::warning("W117", tr!("効かない例外: {winner} の行は {loser} の行と交わりません", "An exception with no effect: no row of {winner} meets a row of {loser}"))
                .at(at(e.span.line, e.winner))
                .table(set.members[e.winner].clone())
                .mark(e.span.clone(), "")
                .note(tr!(
                    "`{}` は、両方に当てはまる入力があるときに、どちらが勝つかを決めます。交わる行が一つも無いので、この行は何も決めていません。ただし書が本文の一部を切り出す形になっていない、という転記の誤りの徴候です。",
                    "`{}` decides which wins when an input matches both. No rows meet, so the line decides nothing. That is the usual sign of a proviso transcribed so that it no longer carves out part of the main rule.",
                    crate::kw::OVERRIDES
                )),
        );
    }

    // --- Unreachable rows
    let ups = upstreams(t, c, f);
    for i in 0..t.rows.len() {
        let up_dead = upstream_dead(&t.rows[i], &ups, c, t);
        let winners = &set.beats[i];
        let dead = if reg.empty(i) || up_dead {
            true
        } else if !winners.is_empty() {
            // First check containment in a single winning row. Every dead row of a staircase
            // is caught here, and the test is cheap, being a per-axis subset test. Containment
            // in a union is needed only when several rows cover the row just together.
            if winners.iter().any(|&e| reg.contains(e, i)) {
                nodes += (winners.len() * reg.axes.len() * 4) as i64;
                true
            } else {
                // Winning rows that do not intersect contribute nothing to the union, so drop
                // them.
                let earlier: Vec<usize> = winners.iter().copied().filter(|&e| reg.intersects(e, i)).collect();
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
            let tn = set.row_table(i).to_string();
            let by_position = set.policy_of(i) == Policy::TopDown && winners.iter().all(|&e| set.same_member(e, i));
            let winner_tables: Vec<String> = {
                let mut v: Vec<String> = Vec::new();
                for &e in winners {
                    let mi = set.member_of[e];
                    let n = format!("{} {}", set.kind_word(mi), set.members[mi]);
                    if set.members[mi] != tn && !v.contains(&n) {
                        v.push(n);
                    }
                }
                v
            };
            dead_rows.push(i);
            // A row brought in by an `apply` is not this rule's to delete: it is the callee's,
            // and dead here only because of what this rule binds or defines. It stays out of
            // the coverage demand; W118 below says when a whole applied table is dead.
            if set.applied[set.member_of[i]].is_some() {
                continue;
            }
            out.push(
                Diag::error("E102", tr!("{} はどの入力にも当てはまりません", "Unreachable row: {} never matches", rn(i)))
                    .at(at(t.rows[i].span.line, set.member_of[i]))
                    .table(tn.clone())
                    .row(t.rows[i].index)
                    .rowref(tn.clone(), t.rows[i].index)
                    .fix_kind(crate::diag::FixKind::RemoveRow)
                    .mark(t.rows[i].span.clone(), tr!("{}: ここに到達する入力はありません", "{}: no input reaches here", rn(i)))
                    .note(if reg.unreachable_row[i] {
                        tr!("この行が名指ししている値を、上流の表は決して出しません。", "The upstream table never produces the values this row names.")
                    } else if up_dead {
                        tr!(
                            "上流の表は、この行が名指しする値を、ほかの列がこの行の言うとおりであるときには出しません。片方ずつなら起こりますが、同時には起こりません。",
                            "The upstream table does not produce the value this row names while the other columns are what this row says. Either half happens; the two together do not."
                        )
                    } else if by_position {
                        tr!("`{} {}` のため、この行の範囲は先行する行がすべて先に取ります。", "Because of `{} {}`, the earlier rows take all of this row's range first.", crate::kw::POLICY, crate::kw::FIRST)
                    } else if !winner_tables.is_empty() {
                        tr!("この行の範囲は、優先する {} の行がすべて先に取ります。", "The rows of {}, which take precedence, take all of this row's range first.", winner_tables.join(if crate::i18n::ja() { "、" } else { ", " }))
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

    // An applied table none of whose rows can be reached from what this rule binds (§15.69
    // W118). Its rows are the callee's and stay silent one by one; as a whole, the table is
    // doing nothing in this rule, which is worth a word.
    for (mi, ap) in set.applied.iter().enumerate() {
        let Some(an) = ap else { continue };
        let mine: Vec<usize> = (0..t.rows.len()).filter(|&i| set.member_of[i] == mi).collect();
        if mine.is_empty() || !mine.iter().all(|i| dead_rows.contains(i)) {
            continue;
        }
        let sp = f.applies.iter().find(|a| a.name.text == *an).map(|a| a.span.clone()).unwrap_or_else(|| t.rows[mine[0]].span.clone());
        let tn = set.members[mi].clone();
        out.push(
            Diag::warning("W118", tr!("{} {} の行は、この準用ではどれも当たりません", "No row of {} {} is reached in this apply", set.kind_word(mi), tn))
                .at(at(sp.line, mi))
                .table(tn.clone())
                .mark(sp, "")
                .note(tr!(
                    "読み替えた値がこの表の条件に届かないか、この規則のほかの定義が先に取っています。元の規則が自分の入力の範囲で完全なことは変わりません。",
                    "What is bound never reaches this table's conditions, or other definitions of this rule take precedence first. The callee is still complete over its own inputs."
                ))
                .note(tr!("この準用に要らない表なら、`{} {}` で外せます。", "If this apply does not need the table, `{} {}` leaves it out.", crate::kw::EXCEPT, tn.rsplit(':').next().unwrap_or(&tn))),
        );
    }

    // --- Completeness
    let mut left = budget;
    let all: Vec<usize> = (0..t.rows.len()).collect();
    let hole = reg.find_hole(&all, &mut left, &ups, c);
    nodes += budget - left;
    let anchor = set.members[0].clone();
    if let Some(hole) = hole {
        out.push(
            {
                let d = Diag::error("E101", tr!("完全性の欠落: どの行にも当てはまらない入力があります", "Completeness gap: some input matches no row"))
                    .at(at(head_span.line, 0))
                    .table(anchor.clone())
                    .wit(pairs_to_witness(reg.witness_pairs(&hole)))
                    .mark(head_span.clone(), tr!("起こりうる入力を覆いきっていません", "the input space is not fully covered"))
                    .note(tr!("当てはまらない例: {}", "An input that matches no row: {}", reg.witness_text(&hole)))
                    .note(tr!("ヒント: この入力に当てはまる行を足してください。", "hint: add a row that matches this input."));
                let d = if merged {
                    d.note(tr!(
                        "{} を合わせても覆えていません。行はどの表に足してもよく、優先の順序はそのまま効きます。",
                        "{} together do not cover it. The row may go in any of them; the declared precedence still applies.",
                        members_text()
                    ))
                } else {
                    d
                };
                // The rewritten form (§11 principle 3) as data: the row's input cells are the
                // witness, and the output cells are copied from the first row purely to give
                // a shape that parses. **The amount has to come from the written rule**, which
                // the note says and `fix.text` — being prose-free and language independent —
                // cannot. A merged set has no one table the row belongs to, so no `fix.text`.
                match reg.row_text(&hole, t) {
                    Some(row) if !merged => d
                        .note(tr!(
                            "足す行の形: `{row}`。出力の値は表の一行目から写した「形」で、正しい額ではありません。規約か Excel か、いま動いている実装か、どれが出どころかを決めて、そこから書いてください。この一行が閉じるのは、いま出た入力の穴だけです。ほかにも抜けがあれば、次の入力が出ます。",
                            "The shape of the row to add: `{row}`. Its output values are copied from the first row to give a shape that parses; they are not the right amounts. Decide whether the written rule, the spreadsheet or the legacy implementation is the source, and take them from there. One row closes the gap this witness names; if more is left, the next run names the next one."
                        ))
                        .fix(crate::diag::FixKind::AddRow, row),
                    _ => d,
                }
            },
        );
    } else if left < 0 {
        out.push(
            Diag::error("E109", tr!("検査の予算を超えたので、完全性を証明できませんでした", "The check exceeded its budget, so completeness could not be proven"))
                .at(at(head_span.line, 0))
                .table(anchor.clone())
                .mark(head_span.clone(), "")
                .note(tr!("支配的なのは {}。", "The dominant columns are {}.", reg.dominant_axes()))
                .note(tr!("列をグループでまとめるか、表を分けてください（§6.3）。近似では通しません。", "Combine columns into groups or split the table (§6.3). No approximation is accepted in its place.")),
        );
    }

    // --- W110: a `policy first` table with no overlaps
    for (mi, pol) in set.policies.iter().enumerate() {
        if *pol != Policy::TopDown {
            continue;
        }
        let rows_of: Vec<usize> = (0..t.rows.len()).filter(|&i| set.member_of[i] == mi).collect();
        if rows_of.len() > 1 && !rows_of.iter().any(|&i| shadowed[i]) {
            let tn = set.members[mi].clone();
            let (line, span) = if merged {
                let first = &t.rows[rows_of[0]];
                (first.span.line, first.span.clone())
            } else {
                (head_span.line, head_span.clone())
            };
            out.push(
                Diag::warning("W110", tr!("この表には重なりがありません", "This table has no overlapping rows"))
                    .at(at(line, mi))
                    .table(tn)
                    .fix(crate::diag::FixKind::ChangePolicy, format!("{} {}", crate::kw::POLICY, crate::kw::UNIQUE))
                    .mark(span, "")
                    .note(tr!("`{} {}` にすると、行の並べ替えが意味を変えないことを検査が保証します。", "With `{} {}`, the checker guarantees that reordering the rows does not change the meaning.", crate::kw::POLICY, crate::kw::UNIQUE)),
            );
        }
    }
    TableCheck { diags: out, w114, quiet, shadow, nodes, overlaps, edge_pairs, dead: dead_rows }
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

    /// A coordinate as a **closed** interval on the axis's own grid.
    ///
    /// `Coord::Open` excludes its ends, and the values on the axis sit on a grid, so `< 0` on a
    /// whole-number axis is `<= -1`. Reading the end as if it were included costs exactly the
    /// one step that decides whether two intervals touch, which is the whole question here.
    fn coord_closed(&self, ai: usize, ci: usize) -> Option<Ival> {
        let Axis::Num { coords, step, .. } = &self.axes[ai] else { return None };
        match coords.get(ci)? {
            Coord::Point(v) => Some((Some(*v), Some(*v))),
            Coord::Open(a, b) => Some((a.map(|v| v.add(*step)), b.map(|v| v.sub(*step)))),
        }
    }

    /// The interval an axis is allowed over the **intersection of two rows** — the hull of the
    /// coordinates both of them select. A hull is wider than the set it covers, which is the
    /// safe direction: every point of the intersection lies inside it.
    fn hull(&self, ai: usize, i: usize, j: usize) -> Option<Ival> {
        let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = (None, None);
        let (mut any, mut open_lo, mut open_hi) = (false, false, false);
        for ci in 0..self.axes[ai].len() {
            if !(self.masks[i][ai][ci] && self.masks[j][ai][ci]) {
                continue;
            }
            let (l, h) = self.coord_closed(ai, ci)?;
            any = true;
            match l {
                None => open_lo = true,
                Some(v) => lo = Some(lo.map_or(v, |c| if v.cmp_to(c) == std::cmp::Ordering::Less { v } else { c })),
            }
            match h {
                None => open_hi = true,
                Some(v) => hi = Some(hi.map_or(v, |c| if v.cmp_to(c) == std::cmp::Ordering::Greater { v } else { c })),
            }
        }
        any.then(|| (if open_lo { None } else { lo }, if open_hi { None } else { hi }))
    }

    /// The interval a name is allowed over that same intersection: what the rows pin it to when
    /// this table has an axis for it, the derived value unfolded when they do not, and the
    /// declared range otherwise.
    fn name_ival(&self, n: &str, i: usize, j: usize, depth: usize) -> Option<Ival> {
        if let Some(ai) = self.col_names.iter().position(|x| x == n) {
            if let Some(s) = self.hull(ai, i, j) {
                return Some(s);
            }
        }
        if let Some(e) = self.exprs.get(n) {
            if let Some(s) = self.expr_ival(e, i, j, depth + 1) {
                return Some(s);
            }
        }
        self.spans.get(n).copied()
    }

    /// A `derive`'s expression evaluated on intervals. §5 allows inputs, `+`, `-` and a constant
    /// multiple, so this is exact for everything a rule can write; anything else gives up, which
    /// reports rather than hides.
    fn expr_ival(&self, e: &Expr, i: usize, j: usize, depth: usize) -> Option<Ival> {
        if depth > 8 {
            return None;
        }
        match e {
            Expr::Name(n, _) => self.name_ival(n, i, j, depth),
            Expr::Lit(Lit::Num(n), _) => {
                let v = crate::types::lit_value_in_pub(n, &crate::types::lit_ty_pub(n))?;
                Some((Some(v), Some(v)))
            }
            Expr::Bin(l, op, r, _) => {
                let a = self.expr_ival(l, i, j, depth + 1)?;
                let b = self.expr_ival(r, i, j, depth + 1)?;
                match op {
                    BinOp::Add => Some((both(a.0, b.0, |x, y| x.add(y)), both(a.1, b.1, |x, y| x.add(y)))),
                    BinOp::Sub => Some((both(a.0, b.1, |x, y| x.sub(y)), both(a.1, b.0, |x, y| x.sub(y)))),
                    BinOp::Mul => point_of(b).map(|k| scale(a, k)).or_else(|| point_of(a).map(|k| scale(b, k))),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Whether the arithmetic of the derived columns rules the intersection of two rows out.
    ///
    /// The per-axis sieve holds a derived coordinate against that column's **own** declared
    /// range and no further, so `a = 1` beside `d < 0` survives it even though `d = b - a` with
    /// `b >= 1` leaves `d >= 0` there. What decides it is the expression over the intervals the
    /// intersection allows its inputs. Both sides are hulls, so `true` means the intersection
    /// really is empty — the direction §6.1 requires.
    pub fn derived_conflict(&self, i: usize, j: usize) -> bool {
        for ai in 0..self.axes.len() {
            let Some(expr) = self.exprs.get(&self.col_names[ai]) else { continue };
            let Some((cl, ch)) = self.hull(ai, i, j) else { continue };
            let Some((el, eh)) = self.expr_ival(expr, i, j, 0) else { continue };
            let apart = matches!((ch, el), (Some(a), Some(b)) if a.cmp_to(b) == std::cmp::Ordering::Less)
                || matches!((cl, eh), (Some(a), Some(b)) if a.cmp_to(b) == std::cmp::Ordering::Greater);
            if apart {
                return true;
            }
        }
        false
    }

    /// This point's constraint on a column, read on **another table's** axis for that column.
    ///
    /// The two tables compress their coordinates independently, so an index cannot be carried
    /// across: an enum goes by its value, and a number by the interval the coordinate stands
    /// for. A column this point does not fix leaves the other axis open.
    fn project(&self, name: &str, path: &[usize], uaxis: &Axis) -> Vec<bool> {
        let all = vec![true; uaxis.len()];
        let Some(ai) = self.col_names.iter().position(|x| x == name) else { return all };
        let Some(&ci) = path.get(ai) else { return all };
        match (&self.axes[ai], uaxis) {
            (Axis::Enum { values }, Axis::Enum { values: uv }) => match values.get(ci) {
                Some(v) => uv.iter().map(|x| x == v).collect(),
                None => all,
            },
            (Axis::Bool, Axis::Bool) => (0..uaxis.len()).map(|x| x == ci).collect(),
            (Axis::Num { .. }, Axis::Num { coords, step, .. }) => {
                let Some((lo, hi)) = self.coord_closed(ai, ci) else { return all };
                coords
                    .iter()
                    .map(|cd| {
                        let (a, b) = match cd {
                            Coord::Point(v) => (Some(*v), Some(*v)),
                            Coord::Open(x, y) => (x.map(|v| v.add(*step)), y.map(|v| v.sub(*step))),
                        };
                        !(matches!((b, lo), (Some(p), Some(q)) if p.cmp_to(q) == std::cmp::Ordering::Less)
                            || matches!((a, hi), (Some(p), Some(q)) if p.cmp_to(q) == std::cmp::Ordering::Greater))
                    })
                    .collect()
            }
            _ => all,
        }
    }

    /// Whether the tables above rule **this point** out, the same question `upstream_dead` asks
    /// of a row. A gap the tables above cannot produce is not a gap.
    fn upstream_dead_at(&self, path: &[usize], ups: &[Upstream], c: &Checked) -> bool {
        upstream_blocked(ups, c, &|up: &Upstream| {
            let ai = self.col_names.iter().position(|x| *x == up.name)?;
            let &ci = path.get(ai)?;
            let v = match &self.axes[ai] {
                Axis::Enum { values } => values.get(ci)?.clone(),
                Axis::Bool => {
                    if ci == 0 { crate::kw::TRUE.to_string() } else { crate::kw::FALSE.to_string() }
                }
                _ => return None,
            };
            let allowed = up
                .reg
                .col_names
                .iter()
                .enumerate()
                .map(|(ua, n)| self.project(n, path, &up.reg.axes[ua]))
                .collect();
            Some((Some(Cell::Lit(Lit::Word(v))), allowed))
        })
    }

    /// The sieve of §6.2. For each derived axis, check whether the reachable interval and the
    /// coordinate's interval intersect. If they do not, the point is infeasible. When two or
    /// more constrained derived values share an input, their dependency cannot be examined.
    /// The interval a name can hold inside this box: the coordinate's own span when the name
    /// is one of this table's axes, and its declared range otherwise.
    fn span_of_name(&self, name: &str, path: &[usize]) -> Option<Ival> {
        if let Some(ai) = self.col_names.iter().position(|n| n == name) {
            if let Some(&ci) = path.get(ai) {
                if let Some(sp) = self.coord_span(ai, ci) {
                    return Some(sp);
                }
            }
        }
        self.spans.get(name).copied()
    }

    /// Whether a `constraint` can hold anywhere in this box. Interval arithmetic, the same
    /// shape the derived sieve uses: only a relation that is impossible for **every** pair of
    /// values the box allows takes the box out (§15.55).
    fn constraint_impossible(&self, k: &crate::ast::Constraint, path: &[usize]) -> bool {
        let (Some((la, lb)), Some((ra, rb))) =
            (self.span_of_name(&k.left, path), self.span_of_name(&k.right, path))
        else {
            return false;
        };
        use std::cmp::Ordering::*;
        match k.op {
            // left <= right is out of reach when the smallest left is already past the
            // largest right.
            CmpOp::Le => matches!((la, rb), (Some(x), Some(y)) if x.cmp_to(y) == Greater),
            CmpOp::Lt => matches!((la, rb), (Some(x), Some(y)) if x.cmp_to(y) != Less),
            CmpOp::Ge => matches!((lb, ra), (Some(x), Some(y)) if x.cmp_to(y) == Less),
            CmpOp::Gt => matches!((lb, ra), (Some(x), Some(y)) if x.cmp_to(y) != Greater),
        }
    }

    pub fn feasible(&self, path: &[usize]) -> Feasible {
        for k in &self.constraints {
            if self.constraint_impossible(k, path) {
                return Feasible::No;
            }
        }
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

// --- The certificate (§15.96) -------------------------------------------------------

/// One axis, as the certificate states it: the column it stands for and a label per
/// coordinate. The labels are for a person reading beside the rule; the checker works on
/// the indices.
pub struct CertAxis {
    pub name: String,
    /// `input`, `derived`, `define`, or `upstream` — what the column is. A point on an axis
    /// of inputs is an input a caller can send; on any other axis it is a point the sieve
    /// could not rule out, which is weaker and says so.
    pub kind: &'static str,
    pub coords: Vec<String>,
    /// Each coordinate as a closed interval of true values, where the axis is numeric.
    /// `None` for an enum or a boolean; an unbounded end is `None` inside the pair.
    pub bounds: Vec<Option<(Option<Rat>, Option<Rat>)>>,
}

/// One row as a box: the coordinates it accepts on each axis, and the cells it was written
/// with, so the box can be held against the rule's own text.
pub struct CertRow {
    pub row: usize,
    pub label: String,
    pub cells: Vec<String>,
    /// The same cells, resolved far enough that the box can be **recomputed** rather than
    /// believed: a comparison keeps its operator and the value its unit stands for, a word
    /// keeps the word (the certificate carries the groups), and a don't-care is empty.
    pub tests: Vec<CertCell>,
    pub accepts: Vec<Vec<usize>>,
    /// Where each cell stands in the file, as `(line, byte column, byte length)`, in the
    /// same order as `tests`. A re-checker that has the file can read the cell back out of
    /// it and hold the parsed form beside the text it came from (§15.97).
    ///
    /// Empty for a row an `apply` brought in: that row is written in **another** file, and
    /// a span into this one would point at the `apply` line. `None` where there is no cell
    /// to point at — a column the `when` line of a `clause` does not mention, or one a
    /// merged member table does not have (§15.31). The certificate says so rather than
    /// quoting the wrong text.
    pub spans: Vec<Option<(usize, usize, usize)>>,
    /// The member table the row was written in. Rows that share one were written in one
    /// table, so they have a cell in the same columns and in no others.
    pub origin: String,
}

/// One cell, as the re-checker reads it.
pub enum CertCell {
    Any,
    Nothing,
    Is(Vec<String>),
    Not(Vec<String>),
    Cmp(Vec<(&'static str, Option<Rat>)>),
}

/// What the certificate says about one table.
pub struct CertTable {
    pub name: String,
    pub policy: &'static str,
    pub axes: Vec<CertAxis>,
    pub rows: Vec<CertRow>,
    /// `unique` only: for each pair of rows, an axis on which their coordinates do not meet.
    /// Checking one entry is one set intersection; a pair that is missing from this list and
    /// not in `undecided` is a certificate that does not hold.
    pub disjoint: Vec<(usize, usize, usize)>,
    /// The pairs the check could not settle either way (W114). They are stated, not proved.
    pub undecided: Vec<(usize, usize)>,
    /// For each row, a point that reaches it: the coordinate on every axis, the input that
    /// point stands for, and the same input as plain numbers on the axes' own scale. The
    /// last of these is what a re-checker can compare with the bounds (§15.97); the one
    /// before it is what a reader can put in a fixture.
    pub reach: Vec<(usize, Vec<usize>, Vec<(String, crate::diag::WVal)>, Vec<Option<Rat>>)>,
    /// Rows a `apply` brought in that this rule's own bindings leave unused (§15.69). They
    /// are outside the reachability claim, and the certificate says which they are rather
    /// than passing over them.
    pub unused: Vec<usize>,
    /// The completeness cover: the walk of §6.3, written down. `None` when it ran past the
    /// budget — a certificate says what it does not have.
    pub cover: Option<Cover>,
    /// The `constraint` lines a cover leaf can point at.
    pub constraints: Vec<(String, &'static str, String)>,
}

/// The certificate of one definition set (§15.96), or nothing when the set has no region to
/// state — a table the checker could not analyze states nothing rather than stating less.
pub fn certificate_of(set: &crate::defset::DefSet, c: &Checked, f: &RuleFile) -> Option<CertTable> {
    let t = &set.table;
    let reg = TableRegion::build(t, c, f)?;
    if reg.axes.is_empty() || t.rows.is_empty() || reg.unanalyzable.is_some() {
        return None;
    }
    let w114 = check_set(set, c, f, "", DEFAULT_BUDGET).w114;
    // A row an `apply` brought in and this rule does not use is not unreachable: it is the
    // callee's row, and §15.69 keeps it out of the coverage demand. The certificate says so.
    let applied: Vec<bool> = (0..t.rows.len()).map(|i| set.applied[set.member_of[i]].is_some()).collect();
    let cover = reg.cover(t, c, f, DEFAULT_BUDGET);
    Some(reg.certificate(t, &w114, &f.inputs, &applied, cover, c))
}

/// One cell, resolved only as far as the units: the re-checker does the geometry.
fn cert_cell(cell: Option<&Cell>, ty: Option<Ty>) -> CertCell {
    let word = |l: &Lit| match l {
        Lit::Word(w) => w.clone(),
        Lit::Str(s) => s.clone(),
        other => format!("{other:?}"),
    };
    match cell {
        None | Some(Cell::DontCare) => CertCell::Any,
        Some(Cell::Nothing) => CertCell::Nothing,
        Some(Cell::Lit(Lit::Word(w))) => CertCell::Is(vec![w.clone()]),
        Some(Cell::Lit(l)) => CertCell::Cmp(vec![("=", ty.as_ref().and_then(|t| lit_rat(l, t)))]),
        Some(Cell::Set(ls)) => CertCell::Is(ls.iter().map(word).collect()),
        Some(Cell::Not(ls)) => CertCell::Not(ls.iter().map(word).collect()),
        Some(Cell::Cmp(cs)) => CertCell::Cmp(
            cs.iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Lt => "<",
                        CmpOp::Ge => ">=",
                        CmpOp::Gt => ">",
                    };
                    (op, ty.as_ref().and_then(|t| lit_rat(l, t)))
                })
                .collect(),
        ),
    }
}

impl TableRegion {
    /// The parts of the certificate this table can state (§15.96): that no two rows of a
    /// `unique` table meet, and that every row is reached by some input.
    ///
    /// Both are **positive** claims with small evidence. Two rows that do not meet do not
    /// meet on some one axis, and naming it turns the check into one set intersection; a row
    /// that is reached is reached by some point, and naming the point turns the check into
    /// one lookup. Neither asks the reader to search, which is the whole difference between
    /// evidence and a second run of the same program.
    pub fn certificate(&self, t: &Table, w114: &[(usize, usize)], inputs: &[crate::ast::VarDecl], applied: &[bool], cover: Option<Cover>, chk: &Checked) -> CertTable {
        let unique = t.policy == crate::ast::Policy::Unique;
        let axes: Vec<CertAxis> = (0..self.axes.len())
            .map(|ai| CertAxis {
                name: self.col_names[ai].clone(),
                kind: if self.derived[ai].is_some() {
                    "derived"
                } else if self.is_define[ai] {
                    "define"
                } else if inputs.iter().any(|i| i.name.text == self.col_names[ai]) {
                    "input"
                } else {
                    "upstream"
                },
                coords: (0..self.axes[ai].len()).map(|c| self.axes[ai].witness(c)).collect(),
                bounds: self.coord_bounds(ai),
            })
            .collect();
        let rows: Vec<CertRow> = t
            .rows
            .iter()
            .enumerate()
            .map(|(ri, row)| CertRow {
                row: ri + 1,
                label: row.label.as_ref().map(|l| l.text.clone()).unwrap_or_default(),
                cells: row.cells.iter().map(cell_text).collect(),
                tests: (0..self.axes.len())
                    .map(|ai| cert_cell(row.cells.get(self.display_of[ai]), chk.ty_of(&self.col_names[ai])))
                    .collect(),
                accepts: (0..self.axes.len())
                    .map(|ai| (0..self.axes[ai].len()).filter(|&c| self.masks[ri][ai][c]).collect())
                    .collect(),
                origin: row.origin.clone().unwrap_or_else(|| t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()),
                spans: if applied.get(ri).copied().unwrap_or(false) {
                    Vec::new()
                } else {
                    (0..self.axes.len())
                        .map(|ai| {
                            row.cell_spans.get(self.display_of[ai]).map(|s| (s.line, s.col, s.len))
                        })
                        .collect()
                },
            })
            .collect();

        let mut disjoint = Vec::new();
        let mut undecided = Vec::new();
        if unique {
            for i in 0..t.rows.len() {
                for j in i + 1..t.rows.len() {
                    match (0..self.axes.len())
                        .find(|&ai| !(0..self.axes[ai].len()).any(|c| self.masks[i][ai][c] && self.masks[j][ai][c]))
                    {
                        Some(ai) => disjoint.push((i + 1, j + 1, ai)),
                        None => undecided.push((i + 1, j + 1)),
                    }
                }
            }
        }
        // A pair W114 named is undecided by definition; a pair that overlaps in the axis
        // space and was not named is one the sieve ruled out, and the certificate says so
        // the same way — it is not proved disjoint here.
        for (a, b) in w114 {
            if !undecided.contains(&(a + 1, b + 1)) {
                undecided.push((a + 1, b + 1));
            }
        }
        undecided.sort_unstable();
        undecided.dedup();

        let (mut reach, mut unused) = (Vec::new(), Vec::new());
        for ri in 0..t.rows.len() {
            match self.reach_point(ri, unique) {
                Some(p) => {
                    let input = self.witness_pairs(&p);
                    let nums = (0..self.axes.len())
                        .map(|ai| self.axes[ai].witness_num(p.get(ai).copied().unwrap_or(0)))
                        .collect();
                    reach.push((ri + 1, p, input, nums));
                }
                None if applied.get(ri).copied().unwrap_or(false) => unused.push(ri + 1),
                None => {}
            }
        }
        CertTable {
            name: t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
            policy: if unique { "unique" } else { "first" },
            axes,
            rows,
            disjoint,
            undecided,
            reach,
            unused,
            cover,
            constraints: self.constraint_list(),
        }
    }

    /// A coordinate point that reaches a row: inside its box, feasible, and — under
    /// `policy first` — outside every row above it, since a point an earlier row also takes
    /// is decided by that row and reaches nothing.
    fn reach_point(&self, ri: usize, unique: bool) -> Option<Vec<usize>> {
        let mut path = vec![0usize; self.axes.len()];
        self.reach_rec(ri, unique, 0, &mut path)
    }

    fn reach_rec(&self, ri: usize, unique: bool, ai: usize, path: &mut Vec<usize>) -> Option<Vec<usize>> {
        if ai == self.axes.len() {
            if self.feasible(path) == Feasible::No {
                return None;
            }
            if !unique && (0..ri).any(|k| (0..self.axes.len()).all(|a| self.masks[k][a][path[a]])) {
                return None;
            }
            return Some(path.clone());
        }
        for c in 0..self.axes[ai].len() {
            if !self.masks[ri][ai][c] {
                continue;
            }
            path[ai] = c;
            if let Some(p) = self.reach_rec(ri, unique, ai + 1, path) {
                return Some(p);
            }
        }
        None
    }
}

/// A node of the cover (§15.96): the same walk `hole_rec` makes, written down. An internal
/// node has one child per coordinate of the axis at its depth, which is what makes "the
/// children tile the axis" true by shape rather than by a claim.
pub enum Cover {
    Split(Vec<Cover>),
    /// This row takes every point below here.
    Row(usize),
    /// No input reaches here: the sieve ruled the box out (§6.2) — by a `constraint`, or by
    /// a derived value whose coordinate lies outside what the derive can produce.
    ByConstraint(usize),
    ByDerived(usize),
    /// Every point of the box was asked about one at a time, and the sieve ruled each one
    /// out. The re-checker redoes exactly that (§15.98).
    ByPoints,
    /// The upstream table cannot produce this value. **Stated, not proved**: re-checking it
    /// needs the upstream table's own region, which this certificate does not carry.
    ByUpstream(String),
}

impl TableRegion {
    /// The cover, or nothing when the walk runs past the budget. A rule that passes `check`
    /// has no hole, so every leaf is a row or an impossibility.
    pub fn cover(&self, t: &Table, c: &Checked, f: &RuleFile, budget: i64) -> Option<Cover> {
        let ups = upstreams(t, c, f);
        let all: Vec<usize> = (0..self.masks.len()).collect();
        let mut path = Vec::new();
        let mut left = budget;
        self.cover_rec(&all, 0, &mut path, &ups, c, &mut left)
    }

    fn cover_rec(
        &self,
        rows: &[usize],
        ai: usize,
        path: &mut Vec<usize>,
        ups: &[Upstream],
        chk: &Checked,
        budget: &mut i64,
    ) -> Option<Cover> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if rows.is_empty() {
            // The reason has to hold for the **whole box**, not for one point of it
            // (§15.98): an axis the path has not fixed keeps its declared range, which is
            // what `span_of_name` does when the path is short.
            for (k, con) in self.constraints.iter().enumerate() {
                if self.constraint_impossible(con, path) {
                    return Some(Cover::ByConstraint(k));
                }
            }
            for ai2 in 0..path.len() {
                if self.derived[ai2].is_some() && self.derived_out_of_reach(ai2, path) {
                    return Some(Cover::ByDerived(ai2));
                }
            }
            if path.len() == self.axes.len() && self.upstream_dead_at(path, ups, chk) {
                return Some(Cover::ByUpstream(self.witness_text(path)));
            }
            // Point by point, then: the box is only impossible when every point in it is.
            if self.first_reachable(path, ups, chk, budget).is_none() {
                return Some(Cover::ByPoints);
            }
            // A hole. `check` reports it as E101 and the rule does not pass, so there is no
            // certificate to write.
            return None;
        }
        if ai == self.axes.len() {
            return Some(Cover::Row(rows[0] + 1));
        }
        if let Some(&r) = rows.iter().find(|&&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x))) {
            return Some(Cover::Row(r + 1));
        }
        let mut kids = Vec::with_capacity(self.axes[ai].len());
        for c in 0..self.axes[ai].len() {
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            let k = self.cover_rec(&sub, ai + 1, path, ups, chk, budget);
            path.pop();
            kids.push(k?);
        }
        Some(Cover::Split(kids))
    }

    /// Whether the coordinate this path takes on a derived axis lies outside the interval
    /// the derive can reach — the half of the sieve that looks at one derived value alone.
    fn derived_out_of_reach(&self, ai: usize, path: &[usize]) -> bool {
        let Some(((rl, rh), _)) = &self.derived[ai] else { return false };
        let Some(&ci) = path.get(ai) else { return false };
        let Some((cl, ch)) = self.coord_span(ai, ci) else { return false };
        matches!((ch, rl), (Some(a), Some(b)) if a.cmp_to(*b) == std::cmp::Ordering::Less)
            || matches!((cl, rh), (Some(a), Some(b)) if a.cmp_to(*b) == std::cmp::Ordering::Greater)
    }

    /// The coordinates of an axis as closed intervals of true values, for the axes where
    /// that means anything. `None` at an end is unbounded.
    pub fn coord_bounds(&self, ai: usize) -> Vec<Option<(Option<Rat>, Option<Rat>)>> {
        (0..self.axes[ai].len()).map(|ci| self.coord_span(ai, ci)).collect()
    }

    /// The rule's `constraint` lines, as the certificate names them.
    pub fn constraint_list(&self) -> Vec<(String, &'static str, String)> {
        self.constraints.iter().map(|k| (k.left.clone(), k.op.word(), k.right.clone())).collect()
    }
}

