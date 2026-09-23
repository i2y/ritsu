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
    /// A `string` column, cut by the prefixes its own cells name (§15.101).
    ///
    /// Coordinate `i` is "starts with `patterns[i]`, and with no longer pattern"; the last
    /// coordinate is "starts with none of them". A finite set of prefixes cuts the strings
    /// into finitely many classes, which is the only thing §6.2's compression asks of a
    /// column — so completeness and overlap work here exactly as they do on an enum.
    Prefix { patterns: Vec<String> },
}

/// A value strictly inside an open coordinate, on the axis's own grid. The midpoint is not
/// on it — `(10%, 30%)` has midpoint 20%, but `(10%, 15%)` has 12.5%, which `rate[step 1%]`
/// cannot hold — and a witness that cannot be written back into a cell is no use to whoever
/// has to close the gap. One grid unit in from the closed side is always representable, and
/// it is the value most likely to show an off-by-one (§11 principle 2). Dates always used
/// this; it is the same rule.
/// A string no pattern of the axis is a prefix of. Every pattern rules out one first
/// character at most, so one of the first few printable letters is always free.
fn outside_all(patterns: &[String]) -> String {
    for c in "zxqjk".chars() {
        let s = c.to_string();
        if !patterns.iter().any(|p| s.starts_with(p.as_str()) || p.is_empty()) {
            return s;
        }
    }
    // Every short letter is taken, so go longer than the longest pattern.
    let n = patterns.iter().map(|p| p.chars().count()).max().unwrap_or(0);
    let mut s = String::new();
    for _ in 0..=n {
        s.push('z');
    }
    if patterns.iter().any(|p| s.starts_with(p.as_str())) { format!("{s}!") } else { s }
}

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
            // One class per prefix, and one for the strings under none of them.
            Axis::Prefix { patterns } => patterns.len() + 1,
        }
    }
    /// The value a coordinate stands for, or the one chosen for it.
    ///
    /// A coordinate confines its value to an interval, and a `constraint` can tie two
    /// axes together, so the value cannot always be read off one axis alone. Where the
    /// caller has solved for it, `chosen` carries it; where it has not, the coordinate's
    /// own reading is used (§15.99).
    fn value_at(&self, i: usize, chosen: Option<Rat>) -> Option<Rat> {
        if chosen.is_some() {
            return chosen;
        }
        match self {
            Axis::Enum { .. } | Axis::Bool | Axis::Prefix { .. } => None,
            Axis::Num { coords, step, .. } => match coords.get(i) {
                Some(Coord::Point(v)) => Some(*v),
                Some(Coord::Open(a, b)) => Some(inside(*a, *b, *step)),
                None => None,
            },
        }
    }

    /// The concrete value used as a witness. §11 principle 2 prefers declaration order for
    /// enums and boundary values for numbers.
    fn witness_at(&self, i: usize, chosen: Option<Rat>) -> String {
        match self {
            Axis::Enum { values } => values.get(i).cloned().unwrap_or_default(),
            Axis::Bool => if i == 0 { crate::kw::TRUE.into() } else { crate::kw::FALSE.into() },
            // A witness for "starts with this prefix" is the prefix itself: no longer
            // pattern has it as a prefix unless that pattern is itself listed, and then
            // the string sits in that pattern's own class. For "starts with none of them"
            // it is a character no pattern begins with (§15.101).
            Axis::Prefix { patterns } => match patterns.get(i) {
                Some(p) => format!("\"{p}\""),
                None => format!("\"{}\"", outside_all(patterns)),
            },
            // Dates are serial day numbers, so every value on the axis — the ends of a
            // coordinate included — is a real calendar day. Print that day.
            Axis::Num { date: true, .. } => match self.value_at(i, chosen) {
                Some(v) => {
                    let (y, m, d) = crate::types::ord_to_date(v);
                    format!("{y:04}-{m:02}-{d:02}")
                }
                None => String::new(),
            },
            Axis::Num { unit, shown, .. } => match self.value_at(i, chosen) {
                Some(v) => format!("{}{unit}", v.mul(Rat::int(*shown))),
                None => String::new(),
            },
        }
    }

    fn witness(&self, i: usize) -> String {
        self.witness_at(i, None)
    }

    /// The witness value as a plain number on the axis's own scale: the coordinate a
    /// point names stands for this value, and the bounds in the certificate are written on
    /// the same scale. `None` where the coordinate stands for no number at all — an enum,
    /// a flag — which is also every coordinate the sieve has nothing to say about.
    ///
    /// `witness_val` below is the reader's form: a rate comes out as its wire count and a
    /// date as `YYYY-MM-DD`, neither of which can be compared with a bound. The re-checker
    /// needs the number, so the certificate carries both (§15.97).
    pub(crate) fn witness_num(&self, i: usize, chosen: Option<Rat>) -> Option<Rat> {
        self.value_at(i, chosen)
    }

    /// The same witness value, typed, for the structured half of a diagnostic. Numbers come
    /// out as integers in the canonical unit and dates as `YYYY-MM-DD`, which is the wire
    /// shape of §10.2 — a caller can hand a witness straight to a vector or a fixture.
    fn witness_val(&self, i: usize, chosen: Option<Rat>) -> crate::diag::WVal {
        use crate::diag::WVal;
        match self {
            Axis::Enum { .. } => WVal::Str(self.witness_at(i, chosen)),
            Axis::Prefix { patterns } => WVal::Str(match patterns.get(i) {
                Some(p) => p.clone(),
                None => outside_all(patterns),
            }),
            Axis::Bool => WVal::Bool(i == 0),
            // Empty unit means "a date"; the display form is already `YYYY-MM-DD`.
            Axis::Num { date: true, .. } => WVal::Str(self.witness_at(i, chosen)),
            Axis::Num { wire, .. } => {
                WVal::Int(crate::types::wire_int(self.value_at(i, chosen).unwrap_or(Rat::zero()), *wire))
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
    /// Every `define` in the rule, by name. A boolean one is a **single comparison** (§6.2),
    /// and where an overlap pins it to one truth value that comparison is a linear condition
    /// like any other — which is how the thresholds inside a definition become visible to
    /// the arithmetic that decides the pair (§15.127).
    defines: BTreeMap<String, Expr>,
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
/// Before the rows are walked, that restriction is narrowed by what the caller pins the
/// **other** columns above to, which is the only way a pair of them decided by one input can
/// be seen to exclude each other (§15.114).
fn upstream_blocked(
    ups: &[Upstream],
    c: &Checked,
    restrict: &dyn Fn(&Upstream) -> Option<(Option<Cell>, Vec<Vec<bool>>)>,
) -> bool {
    for up in ups {
        let Some((kcell, mut allowed)) = restrict(up) else { continue };
        narrow_by_siblings(&mut allowed, up, ups, c, restrict);
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

/// The closed ends of a coordinate: a point is itself, an open interval is pulled in by one
/// step on each side it has.
fn coord_ends(cd: &Coord, step: Rat) -> Ival {
    match cd {
        Coord::Point(v) => (Some(*v), Some(*v)),
        Coord::Open(a, b) => (a.map(|v| v.add(step)), b.map(|v| v.sub(step))),
    }
}

/// Whether two closed intervals meet.
fn ivals_meet(x: Ival, y: Ival) -> bool {
    use std::cmp::Ordering::{Greater, Less};
    !(matches!((x.1, y.0), (Some(p), Some(q)) if p.cmp_to(q) == Less)
        || matches!((x.0, y.1), (Some(p), Some(q)) if p.cmp_to(q) == Greater))
}

/// Carry a restriction on one table's axis over to another table's axis for the same column.
///
/// Two tables cut the same column at their own boundaries, so a mask on one says nothing
/// coordinate for coordinate about the other. The reading is `project`'s, widened from a
/// point to a set: a coordinate of `to` is kept when it meets **any** kept coordinate of
/// `from`. That over-approximates, which is the safe direction here — what the carry drops
/// is only what the mask certainly excludes.
fn carry(from: &Axis, mask: &[bool], to: &Axis) -> Vec<bool> {
    let all = vec![true; to.len()];
    match (from, to) {
        (Axis::Enum { values }, Axis::Enum { values: tv }) => tv
            .iter()
            .map(|x| values.iter().zip(mask).any(|(v, &m)| m && v == x))
            .collect(),
        (Axis::Bool, Axis::Bool) => (0..to.len()).map(|i| mask.get(i).copied().unwrap_or(true)).collect(),
        (Axis::Num { coords: fc, step: fs, .. }, Axis::Num { coords: tc, step: ts, .. }) => tc
            .iter()
            .map(|t| {
                let te = coord_ends(t, *ts);
                fc.iter().zip(mask).any(|(f, &m)| m && ivals_meet(coord_ends(f, *fs), te))
            })
            .collect(),
        // Prefix classes are cut by the patterns each table happens to name, and two tables
        // need not name the same ones. Nothing is carried rather than something wrong.
        _ => all,
    }
}

/// Where a table above can produce a value, read as a mask on each of its own axes.
///
/// The union of the boxes of every row that produces the value, taken axis by axis. Both
/// widenings go the safe way: a row's box contains the region where the row actually fires
/// (an earlier row may take part of it), and a per-axis union contains the union of the
/// boxes. So the answer is a **superset** of the inputs on which the column really holds
/// that value, and intersecting it into a sibling column's restriction can only drop inputs
/// where this column certainly does not hold it.
///
/// `None` where a row's output cannot be read as a value: such a row might produce the value
/// anywhere in its box, and leaving it out would narrow the answer past the truth.
fn produces_where(up: &Upstream, c: &Checked, cell: &Cell) -> Option<Vec<Vec<bool>>> {
    let mut acc: Vec<Vec<bool>> = up.reg.axes.iter().map(|a| vec![false; a.len()]).collect();
    for ri in 0..up.table.rows.len() {
        let v = match up.table.rows[ri].outs.get(up.oi) {
            Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w)) if !c.syms.contains_key(w) => w.clone(),
            _ => return None,
        };
        if !crate::eval::cell_matches(c, cell, &crate::eval::Val::Enum(v), &up.ty) {
            continue;
        }
        for (a, m) in acc.iter_mut().enumerate() {
            for (x, bit) in m.iter_mut().enumerate() {
                *bit |= up.reg.masks[ri][a][x];
            }
        }
    }
    Some(acc)
}

/// Narrow one column's restriction by what the **other** columns above are pinned to.
///
/// Reading one column at a time cannot see that two of them are decided by the same input:
/// each half looks possible on its own while the pair never arrives together. A row naming
/// `small` and `heavy`, where one table calls anything over 10kg large and the other calls
/// anything over 20kg heavy, is dead — and neither column alone says so (§15.114).
fn narrow_by_siblings(
    allowed: &mut [Vec<bool>],
    up: &Upstream,
    ups: &[Upstream],
    c: &Checked,
    restrict: &dyn Fn(&Upstream) -> Option<(Option<Cell>, Vec<Vec<bool>>)>,
) {
    for other in ups {
        if other.name == up.name {
            continue;
        }
        let Some((Some(ocell), oallowed)) = restrict(other) else { continue };
        let Some(mut owhere) = produces_where(other, c, &ocell) else { continue };
        for (a, m) in owhere.iter_mut().enumerate() {
            for (x, bit) in m.iter_mut().enumerate() {
                *bit &= oallowed[a][x];
            }
        }
        for (ua, n) in up.reg.col_names.iter().enumerate() {
            let Some(oa) = other.reg.col_names.iter().position(|m| m == n) else { continue };
            let carried = carry(&other.reg.axes[oa], &owhere[oa], &up.reg.axes[ua]);
            for (bit, keep) in allowed[ua].iter_mut().zip(carried) {
                *bit &= keep;
            }
        }
    }
}

/// A span of one input column: an interval where the column is a number, the values it is
/// allowed to take where it is not.
#[derive(Debug, Clone, PartialEq)]
pub enum CertSpan {
    Num(Option<Rat>, Option<Rat>),
    Words(Vec<String>),
}

impl CertSpan {
    /// Whether two spans of the same column have anything in common.
    fn meets(&self, other: &CertSpan) -> bool {
        match (self, other) {
            (CertSpan::Num(a, b), CertSpan::Num(c, d)) => ivals_meet((*a, *b), (*c, *d)),
            (CertSpan::Words(a), CertSpan::Words(b)) => a.iter().any(|x| b.contains(x)),
            // Two spans of one column are read off two axes for that column, so they are
            // the same kind. A mix means the certificate is about to say something it
            // cannot support, and saying nothing is the safe half.
            _ => true,
        }
    }
}

/// Why a box no row takes is a box no input reaches: one input column, and two spans of it
/// that do not meet.
///
/// Each span comes with what forces it. A span named by a column above says "while that
/// column holds that value, this input lies in here", and a re-checker earns it back from
/// that column's own table: the rows of it that write the value, and the boxes those rows
/// take. A span with no column is what the box itself already fixes the input to. Two
/// spans, one intersection, and the leaf is settled the way every other leaf is — by
/// redoing it, not by believing it (§15.115).
/// A fact a table gets from the ones above it, written on its own axes so that what rests
/// on it is a coordinate test and nothing more (§15.115).
#[derive(Debug, Clone)]
pub enum AboveFact {
    /// No row of the table that decides this axis's column writes this coordinate's value.
    Never { axis: usize, coord: usize },
    /// These two coordinates cannot stand together: on the input they share, the spans the
    /// tables above leave them do not meet.
    Apart {
        a: (usize, usize),
        b: (usize, usize),
        input: String,
        spans: (CertSpan, CertSpan),
    },
}

#[derive(Debug, Clone)]
pub enum Apart {
    /// No row of the table that decides this column writes this value at all. The smallest
    /// reason there is, and a re-checker settles it by reading that table's rows.
    Never { column: String, value: String },
    /// One input column, and two spans of it that do not meet.
    Spans {
        input: String,
        /// The two spans, as `(the column above that forces it, the value it holds, the
        /// span)`. `None` in the first field is the box's own coordinate on that column.
        spans: Vec<(Option<String>, Option<String>, CertSpan)>,
    },
}

/// The span an upstream column's value puts one of that table's own input columns into.
///
/// The union of the boxes of the rows that write the value, read on the axis for `col`.
/// A row's box contains the region where it actually fires, and the per-axis union contains
/// the union of the boxes, so what comes back is a **superset** of the inputs on which the
/// column really holds the value — which is the direction that keeps "these two do not
/// meet" honest. `None` where a row's output cannot be read as a value, or where the axis
/// is one no span can be written for.
fn span_of_value(up: &Upstream, c: &Checked, val: &str, col: &str) -> Option<CertSpan> {
    let ai = up.reg.col_names.iter().position(|n| n == col)?;
    let mut picked: Vec<usize> = Vec::new();
    for ri in 0..up.table.rows.len() {
        let w = match up.table.rows[ri].outs.get(up.oi) {
            Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w)) if !c.syms.contains_key(w) => w.clone(),
            _ => return None,
        };
        if w == val {
            picked.push(ri);
        }
    }
    if picked.is_empty() {
        return None;
    }
    // A coordinate is in the span when some row that writes the value can still fire
    // somewhere above it. Under `first` a row's box is not where it fires: an earlier row
    // may take all of it, and the catch-all at the bottom of a table has the whole axis for
    // a box while firing only on what is left. The reading is the sieve's (§15.30): a row
    // counts as blocked only when a **single** earlier row takes the whole of its box at
    // this coordinate, which is loose in the safe direction — what drops out is only what
    // certainly cannot fire.
    let live = |x: usize| {
        picked.iter().any(|&ri| {
            if !up.reg.masks[ri][ai][x] {
                return false;
            }
            if up.table.policy != Policy::TopDown {
                return true;
            }
            !(0..ri).any(|e| {
                (0..up.reg.axes.len()).all(|a| {
                    (0..up.reg.axes[a].len()).all(|y| {
                        let inside =
                            up.reg.masks[ri][a][y] && (a != ai || y == x);
                        !inside || up.reg.masks[e][a][y]
                    })
                })
            })
        })
    };
    match &up.reg.axes[ai] {
        Axis::Num { .. } => {
            let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = (None, None);
            let (mut any, mut open_lo, mut open_hi) = (false, false, false);
            for x in 0..up.reg.axes[ai].len() {
                if !live(x) {
                    continue;
                }
                let (a, b) = up.reg.coord_closed(ai, x)?;
                any = true;
                match (a, lo) {
                    (None, _) => open_lo = true,
                    (Some(v), Some(l)) if v.cmp_to(l) == std::cmp::Ordering::Less => lo = Some(v),
                    (Some(v), None) => lo = Some(v),
                    _ => {}
                }
                match (b, hi) {
                    (None, _) => open_hi = true,
                    (Some(v), Some(h)) if v.cmp_to(h) == std::cmp::Ordering::Greater => hi = Some(v),
                    (Some(v), None) => hi = Some(v),
                    _ => {}
                }
            }
            if !any {
                return None;
            }
            Some(CertSpan::Num(if open_lo { None } else { lo }, if open_hi { None } else { hi }))
        }
        Axis::Enum { values } => Some(CertSpan::Words(
            values.iter().enumerate().filter(|(x, _)| live(*x)).map(|(_, v)| v.clone()).collect(),
        )),
        Axis::Bool => Some(CertSpan::Words(
            (0..2)
                .filter(|&x| live(x))
                .map(|x| if x == 0 { crate::kw::TRUE.to_string() } else { crate::kw::FALSE.to_string() })
                .collect(),
        )),
        // A prefix axis cuts the strings by the patterns one table happens to name, and two
        // tables need not name the same ones. No span is written rather than a wrong one.
        Axis::Prefix { .. } => None,
    }
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
            // `starts_with "ABC"` takes every class whose own prefix extends `ABC`. A
            // string in such a class starts with that class's pattern, hence with `ABC`;
            // and a string in any other class cannot, because `ABC` is itself a pattern
            // and the string would sit in its class instead (§15.101).
            Some(Cell::Prefix(ps)) => {
                if let Axis::Prefix { patterns } = axis {
                    for (i, pat) in patterns.iter().enumerate() {
                        if ps.iter().any(|p| pat.starts_with(p.as_str())) {
                            v[i] = true;
                        }
                    }
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
        let mut defines: BTreeMap<String, Expr> = BTreeMap::new();
        for it in &f.items {
            match it {
                Item::Derived(d) => {
                    exprs.insert(d.name.text.clone(), d.expr.clone());
                }
                Item::Define(d) => {
                    defines.insert(d.name.text.clone(), d.expr.clone());
                }
                _ => {}
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
                // §6.2 on a `string` column: the prefixes its own cells name cut the
                // strings into finitely many classes, and that is all the compression
                // asks for (§15.101). A column no cell tests has one class, "anything",
                // which is what a don't-care column is anyway.
                // An optional string would need the absent value as one more class, and
                // `none` beside a prefix has no reading yet — so it stays unanalyzable
                // rather than being cut wrongly (§15.101).
                Ty::Str if !opt => {
                    let mut patterns: Vec<String> = Vec::new();
                    for row in &t.rows {
                        if let Some(Cell::Prefix(ps)) = row.cells.get(ci) {
                            for p in ps {
                                if !patterns.contains(p) {
                                    patterns.push(p.clone());
                                }
                            }
                        }
                    }
                    // Longest first, so that a class is "under this and under nothing
                    // longer" by position as well as by construction.
                    patterns.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
                    Axis::Prefix { patterns }
                }
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
                        Item::Agg(d) if d.name.text == *name => c.ranges.get(name).copied(),
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
            defines,
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

    /// The first point of the box where two rows meet that neither the sieve nor the tables
    /// above rule out, or nothing when every one of its points is ruled out.
    ///
    /// The same walk as `first_reachable`, restricted to the coordinates both rows select.
    /// It is charged to the same budget, so an overrun is visible to the caller rather than
    /// silently becoming "no overlap".
    fn first_reachable_pair(
        &self,
        i: usize,
        j: usize,
        ups: &[Upstream],
        chk: &Checked,
        budget: &mut i64,
    ) -> Option<Vec<usize>> {
        self.pair_rec(i, j, &mut Vec::new(), ups, chk, budget)
    }

    fn pair_rec(
        &self,
        i: usize,
        j: usize,
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
        for x in 0..self.axes[ai].len() {
            if !(self.masks[i][ai][x] && self.masks[j][ai][x]) {
                continue;
            }
            p.push(x);
            let keep = self.feasible(p) != Feasible::No;
            let got = if keep { self.pair_rec(i, j, p, ups, chk, budget) } else { None };
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
        let vals = self.witness_values(path);
        let mut items: Vec<(usize, String)> = (0..self.axes.len())
            .map(|ai| {
                let v = vals.as_ref().and_then(|w| w.get(ai).copied().flatten());
                (
                    self.display_of[ai],
                    format!("{} = {}", self.col_names[ai], self.axes[ai].witness_at(path.get(ai).copied().unwrap_or(0), v)),
                )
            })
            .collect();
        items.sort_by_key(|(d, _)| *d);
        items.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(", ")
    }

    /// The same witness, as `(column, value)` pairs in the table's visible column order.
    fn witness_pairs(&self, path: &[usize]) -> Vec<(String, crate::diag::WVal)> {
        let vals = self.witness_values(path);
        let mut items: Vec<(usize, (String, crate::diag::WVal))> = (0..self.axes.len())
            .map(|ai| {
                let v = vals.as_ref().and_then(|w| w.get(ai).copied().flatten());
                (
                    self.display_of[ai],
                    (
                        self.col_names[ai].clone(),
                        self.axes[ai].witness_val(path.get(ai).copied().unwrap_or(0), v),
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
        let vals = self.witness_values(path);
        let mut cells: Vec<(usize, String)> = (0..self.axes.len())
            .map(|ai| {
                let v = vals.as_ref().and_then(|w| w.get(ai).copied().flatten());
                (self.display_of[ai], self.axes[ai].witness_at(path.get(ai).copied().unwrap_or(0), v))
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
        Cell::Prefix(ps) => format!("{} {}", crate::kw::STARTS_WITH, ps.iter().map(|p| format!("\"{p}\"")).collect::<Vec<_>>().join(",")),
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
        Cell::Prefix(ps) => format!("{} {}", crate::kw::STARTS_WITH, ps.iter().map(|p| format!("\"{p}\"")).collect::<Vec<_>>().join(", ")),
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

pub(crate) fn row_key(r: &Row) -> String {
    r.cells.iter().map(cell_key).collect::<Vec<_>>().join("|")
}

fn pair_key(a: &Row, b: &Row) -> String {
    format!("{}\u{1}{}", row_key(a), row_key(b))
}

/// Whether the output cells are syntactically identical. Used for the "equivalent" verdict of
/// §4.
impl TableRegion {
    /// Whether linear arithmetic can prove that no input matches both rows (§15.126).
    ///
    /// The per-axis sieve gives every derived value an axis of its own, so two derives that
    /// share an input move independently there and a pair that can only meet where no input
    /// reaches still looks like an overlap. Here the same question is asked as one system of
    /// linear inequalities — the cells of both rows, every `derive`'s defining equation,
    /// every declared range, every `constraint` — and eliminated variable by variable.
    ///
    /// **A refutation means proved impossible; `None` means nothing.** Anything this cannot
    /// read is dropped rather than guessed at, which is safe in exactly one direction: a
    /// relaxed system that is still unsatisfiable proves the original one is, and one that
    /// is satisfiable over the rationals proves nothing about the integers. So `None` leaves
    /// W114 standing exactly as it stood before. The refutation itself — the multipliers and
    /// where each inequality came from — is what a certificate hands on (§15.139).
    fn linearly_impossible(
        &self,
        t: &Table,
        i: usize,
        j: usize,
        f: &RuleFile,
        c: &Checked,
    ) -> Option<crate::fourier::Refutation> {
        use crate::fourier::{grounds, pinned, Lin, Origin, Refutation};
        // A boolean `define` the overlap pins to one truth value (§15.127). Its body is one
        // comparison, so there it is a linear condition — and the thresholds inside it, which
        // the per-axis sieve never looks at, come into the system with it.
        let pins: Vec<(&str, &Expr, bool)> = (0..self.axes.len())
            .filter(|&ai| self.is_define[ai] && matches!(self.axes[ai], Axis::Bool))
            .filter_map(|ai| {
                let both: Vec<usize> =
                    (0..self.axes[ai].len()).filter(|&k| self.masks[i][ai][k] && self.masks[j][ai][k]).collect();
                // Pinned only when the overlap leaves it one value. Where both survive, the
                // pair does not depend on the definition and there is nothing to add.
                let [k] = both[..] else { return None };
                let name = self.col_names[ai].as_str();
                Some((name, self.defines.get(name)?, k == 0))
            })
            .collect();
        let mut seed: Vec<String> = self.col_names.clone();
        for (_, e, _) in &pins {
            names_in(e, &mut seed);
        }
        // One system for each type of number among the columns; any of them that has no
        // solution settles the pair, because each is the question with some conditions left
        // out.
        for g in grounds(&seed, f, c) {
            if pins.is_empty() && !g.vars.iter().any(|n| self.exprs.contains_key(n)) {
                // Nothing correlated: the sieve already sees everything this would.
                continue;
            }
            let mut sys = g.sys;
            // The cells of both rows. A cell this cannot read is dropped, which only relaxes.
            for row in [i, j] {
                for (ai, name) in self.col_names.iter().enumerate() {
                    if !g.vars.contains(name) {
                        continue;
                    }
                    let Some(cell) = t.rows[row].cells.get(self.display_of[ai]) else { continue };
                    let v = Lin::var(name);
                    let at = |part: usize| Origin::Cell { row, col: name.clone(), part };
                    match cell {
                        Cell::Lit(l) => {
                            let Some(r) = crate::fourier::lit_of(l, &g.want) else { continue };
                            let d = v.plus(&Lin::con(r.mul(crate::num::Rat::int(-1))));
                            sys.push(d.clone().le(false).tag(at(0)));
                            sys.push(d.ge(false).tag(at(1)));
                        }
                        Cell::Cmp(ops) => {
                            for (part, (op, l)) in ops.iter().enumerate() {
                                let Some(r) = crate::fourier::lit_of(l, &g.want) else { continue };
                                let d = v.clone().plus(&Lin::con(r.mul(crate::num::Rat::int(-1))));
                                sys.push(crate::fourier::cmp(d, *op, false).tag(at(part)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            // And the definitions the overlap pinned, as the comparisons they are.
            for (name, e, yes) in &pins {
                for (part, q) in pinned(e, *yes, &g.want, c).into_iter().enumerate() {
                    sys.push(q.tag(Origin::Pin { name: name.to_string(), yes: *yes, part }));
                }
            }
            if let Some(r) = Refutation::of(sys) {
                return Some(r);
            }
        }
        None
    }
}

/// Every name an expression mentions, appended.
fn names_in(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Name(n, _) => out.push(n.clone()),
        Expr::Lit(..) => {}
        Expr::Bin(a, _, b, _) => {
            names_in(a, out);
            names_in(b, out);
        }
        Expr::Call(_, args, _) => args.iter().for_each(|a| names_in(a, out)),
    }
}

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
    let ups = upstreams(t, c, f);
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
            // **A point of the intersection nothing rules out, not its first corner.** The
            // corner is where the axes happen to start, and a corner the tables above cannot
            // produce made E105 report an overlap at an input that does not exist — while
            // E102, in the same run, called the very row that named it dead. §15.98 fixed
            // this for the hole; this is the overlap (§15.114).
            let mut left = budget;
            let found = reg.first_reachable_pair(i, j, &ups, c, &mut left);
            nodes += budget - left;
            let wpath = match found {
                Some(p) => p,
                // Every point ruled out: no input matches both rows, so there is nothing to
                // report. Unless the walk ran out of budget before it could say so, and then
                // the corner is taken and the pair reported — over-reporting is the
                // direction an exhausted proof falls in (§6.1).
                None if left >= 0 => continue,
                None => (0..reg.axes.len())
                    .map(|ai| {
                        (0..reg.axes[ai].len())
                            .find(|&x| reg.masks[i][ai][x] && reg.masks[j][ai][x])
                            .unwrap_or(0)
                    })
                    .collect(),
            };
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
            let mut ruled_out = false;
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
                    None => {
                        // No input could be constructed, which is not a proof that none
                        // exists — so before demoting to W114, ask linear arithmetic
                        // whether one can exist at all (§15.126). Dropping every condition
                        // it cannot read only relaxes the system, so a `true` here really
                        // does settle the pair, and a `false` leaves W114 where it was.
                        nodes += (reg.axes.len() * 32) as i64;
                        ruled_out = reg.linearly_impossible(t, i, j, f, c).is_some();
                        feas = Feasible::Unknown;
                    }
                }
            }
            if ruled_out {
                continue;
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

    /// The evidence behind an upstream leaf, where it can be written small (§15.115).
    ///
    /// Two spans of one input column that do not meet. One always comes from a column above;
    /// the other comes from a second column above, or from what this box already fixes that
    /// input to. `None` when no such pair settles it — the leaf then goes out bare and is
    /// counted among the things the certificate states rather than proves.
    fn apart_at(&self, path: &[usize], ups: &[Upstream], c: &Checked) -> Option<Apart> {
        let held: Vec<(&Upstream, String)> = ups
            .iter()
            .filter_map(|up| {
                let ai = self.col_names.iter().position(|x| *x == up.name)?;
                let &ci = path.get(ai)?;
                let v = match &self.axes[ai] {
                    Axis::Enum { values } => values.get(ci)?.clone(),
                    Axis::Bool => {
                        if ci == 0 { crate::kw::TRUE.to_string() } else { crate::kw::FALSE.to_string() }
                    }
                    _ => return None,
                };
                Some((up, v))
            })
            .collect();
        // The smallest reason first: the table that decides the column never writes the
        // value, whatever the rest of the box says.
        for (up, v) in &held {
            if !up.table.rows.iter().any(|r| {
                matches!(r.outs.get(up.oi),
                    Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w)) if !c.syms.contains_key(w) && w == v)
            }) && up.table.rows.iter().all(|r| {
                matches!(r.outs.get(up.oi),
                    Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w)) if !c.syms.contains_key(w))
            }) {
                return Some(Apart::Never { column: up.name.clone(), value: v.clone() });
            }
        }
        for (i, (up, v)) in held.iter().enumerate() {
            for col in &up.reg.col_names {
                let Some(mine) = span_of_value(up, c, v, col) else { continue };
                for (up2, v2) in held.iter().skip(i + 1) {
                    let Some(theirs) = span_of_value(up2, c, v2, col) else { continue };
                    if !mine.meets(&theirs) {
                        return Some(Apart::Spans {
                            input: col.clone(),
                            spans: vec![
                                (Some(up.name.clone()), Some(v.clone()), mine),
                                (Some(up2.name.clone()), Some(v2.clone()), theirs),
                            ],
                        });
                    }
                }
                if let Some(here) = self.span_here(col, path) {
                    if !mine.meets(&here) {
                        return Some(Apart::Spans {
                            input: col.clone(),
                            spans: vec![(Some(up.name.clone()), Some(v.clone()), mine), (None, None, here)],
                        });
                    }
                }
            }
        }
        None
    }

    /// Every fact the leaves of a cover rest on, written on this table's own axes and
    /// listed once. What the leaf itself then needs is a coordinate test.
    fn gather_above(&self, cv: &Cover, out: &mut Vec<AboveFact>) {
        let idx = |col: &str, val: &str| -> Option<(usize, usize)> {
            let ai = self.col_names.iter().position(|n| n == col)?;
            let ci = match &self.axes[ai] {
                Axis::Enum { values } => values.iter().position(|v| v == val)?,
                Axis::Bool => {
                    if val == crate::kw::TRUE { 0 } else if val == crate::kw::FALSE { 1 } else { return None }
                }
                _ => return None,
            };
            Some((ai, ci))
        };
        match cv {
            Cover::Split(kids) => kids.iter().for_each(|k| self.gather_above(k, out)),
            Cover::ByUpstream(_, Some(Apart::Never { column, value })) => {
                if let Some((axis, coord)) = idx(column, value) {
                    if !out.iter().any(|f| matches!(f, AboveFact::Never { axis: x, coord: y } if *x == axis && *y == coord)) {
                        out.push(AboveFact::Never { axis, coord });
                    }
                }
            }
            Cover::ByUpstream(what, Some(Apart::Spans { input, spans })) => {
                // A span with no column of its own is the box's own coordinate on the
                // shared input, and the leaf's own text names which one it is.
                let pin = |col: &Option<String>, val: &Option<String>| -> Option<(usize, usize)> {
                    match (col, val) {
                        (Some(c), Some(v)) => idx(c, v),
                        _ => {
                            let ai = self.col_names.iter().position(|n| n == input)?;
                            let ci = what
                                .split(", ")
                                .find_map(|part| part.strip_prefix(&format!("{input} = ")))
                                .and_then(|v| match &self.axes[ai] {
                                    Axis::Enum { values } => values.iter().position(|x| x == v),
                                    _ => None,
                                })?;
                            Some((ai, ci))
                        }
                    }
                };
                if let (Some(a), Some(b)) = (pin(&spans[0].0, &spans[0].1), pin(&spans[1].0, &spans[1].1)) {
                    let f = AboveFact::Apart {
                        a,
                        b,
                        input: input.clone(),
                        spans: (spans[0].2.clone(), spans[1].2.clone()),
                    };
                    if !out.iter().any(|g| matches!((g, &f), (AboveFact::Apart { a: x, b: y, .. }, AboveFact::Apart { a: p, b: q, .. }) if x == p && y == q)) {
                        out.push(f);
                    }
                }
            }
            _ => {}
        }
    }

    /// The span this box puts one of its **own** columns into.
    fn span_here(&self, col: &str, path: &[usize]) -> Option<CertSpan> {
        let ai = self.col_names.iter().position(|n| n == col)?;
        let &ci = path.get(ai)?;
        match &self.axes[ai] {
            Axis::Num { .. } => self.coord_closed(ai, ci).map(|(a, b)| CertSpan::Num(a, b)),
            Axis::Enum { values } => values.get(ci).map(|v| CertSpan::Words(vec![v.clone()])),
            Axis::Bool => Some(CertSpan::Words(vec![if ci == 0 {
                crate::kw::TRUE.to_string()
            } else {
                crate::kw::FALSE.to_string()
            }])),
            Axis::Prefix { .. } => None,
        }
    }

    /// A coordinate as a closed interval on the grid, for a name this table has an axis
    /// for; the declared range for one it does not. The witness needs the grid reading:
    /// an open end is not a value anything can take.
    fn closed_of_name(&self, name: &str, path: &[usize]) -> Option<Ival> {
        if let Some(ai) = self.col_names.iter().position(|n| n == name) {
            if let Some(&ci) = path.get(ai) {
                if let Some(sp) = self.coord_closed(ai, ci) {
                    return Some(sp);
                }
            }
        }
        self.spans.get(name).copied()
    }

    /// **The values behind a point, chosen so that every `constraint` holds.**
    ///
    /// Each coordinate confines its value to an interval, and a `constraint` ties two
    /// names together. Taking the low end of each interval on its own can break one — and
    /// a witness that breaks a constraint is a witness to nothing (§11 principle 2): the
    /// input it names cannot arrive, and a reader who pastes it gets a row that is dead.
    /// So the bounds are pushed along the constraints until they stop moving, the low end
    /// of each is taken, and the result is **checked against every constraint** before it
    /// is handed back. `None` means no assignment was found: the box may still be
    /// reachable — the sieve reads each constraint on its own — but this program will not
    /// name a point it cannot stand behind (§15.99).
    fn witness_values(&self, path: &[usize]) -> Option<Vec<Option<Rat>>> {
        use std::cmp::Ordering::*;
        let step_of = |name: &str| -> Rat {
            match self.col_names.iter().position(|n| n == name).map(|ai| &self.axes[ai]) {
                Some(Axis::Num { step, .. }) => *step,
                _ => Rat::int(1),
            }
        };
        // A lower bound only ever rises and an upper bound only ever falls; `None` is the
        // end that is not there, which is minus or plus infinity depending on which end
        // it is, so it never wins.
        let raise = |a: Option<Rat>, b: Option<Rat>| -> Option<Rat> {
            match (a, b) {
                (Some(x), Some(y)) => Some(if x.cmp_to(y) == Less { y } else { x }),
                (x, None) => x,
                (None, y) => y,
            }
        };
        let drop_to = |a: Option<Rat>, b: Option<Rat>| -> Option<Rat> {
            match (a, b) {
                (Some(x), Some(y)) => Some(if x.cmp_to(y) == Greater { y } else { x }),
                (x, None) => x,
                (None, y) => y,
            }
        };
        let mut b: BTreeMap<String, Ival> = BTreeMap::new();
        let mut ks: Vec<&crate::ast::Constraint> = Vec::new();
        for k in &self.constraints {
            let (Some(l), Some(r)) = (self.closed_of_name(&k.left, path), self.closed_of_name(&k.right, path))
            else {
                continue;
            };
            b.entry(k.left.clone()).or_insert(l);
            b.entry(k.right.clone()).or_insert(r);
            ks.push(k);
        }
        // Bellman-Ford over the difference constraints: one pass per name is enough for
        // the bounds to travel the longest chain, and a cycle settles on equality.
        for _ in 0..=b.len() {
            let mut moved = false;
            for k in &ks {
                let (Some(&(ll, lh)), Some(&(rl, rh))) = (b.get(&k.left), b.get(&k.right)) else { continue };
                let gap = |n: &str, strict: bool| if strict { step_of(n) } else { Rat::zero() };
                let (nlh, nrl, nll, nrh) = match k.op {
                    // left <= right: the left cannot pass the right's top, and the right
                    // cannot sit below the left's bottom.
                    CmpOp::Le | CmpOp::Lt => {
                        let d = gap(&k.left, k.op == CmpOp::Lt);
                        (drop_to(lh, rh.map(|v| v.sub(d))), raise(rl, ll.map(|v| v.add(gap(&k.right, k.op == CmpOp::Lt)))), ll, rh)
                    }
                    CmpOp::Ge | CmpOp::Gt => {
                        let d = gap(&k.right, k.op == CmpOp::Gt);
                        (lh, rl, raise(ll, rl.map(|v| v.add(gap(&k.left, k.op == CmpOp::Gt)))), drop_to(rh, lh.map(|v| v.sub(d))))
                    }
                };
                let changed = |a: Option<Rat>, c: Option<Rat>| !matches!((a, c), (None, None)) && a.map(|x| x.num) != c.map(|x| x.num);
                if changed(lh, nlh) || changed(rl, nrl) || changed(ll, nll) || changed(rh, nrh) {
                    moved = true;
                }
                b.insert(k.left.clone(), (nll, nlh));
                b.insert(k.right.clone(), (nrl, nrh));
            }
            if !moved {
                break;
            }
        }
        let pick = |iv: &Ival| -> Option<Rat> { iv.0.or(iv.1) };
        for (_, iv) in b.iter() {
            if matches!((iv.0, iv.1), (Some(x), Some(y)) if x.cmp_to(y) == Greater) {
                return None;
            }
        }
        // Hand nothing back that does not satisfy what the rule declares.
        for k in &ks {
            let (Some(x), Some(y)) = (b.get(&k.left).and_then(pick), b.get(&k.right).and_then(pick)) else {
                continue;
            };
            let ok = match k.op {
                CmpOp::Le => x.cmp_to(y) != Greater,
                CmpOp::Lt => x.cmp_to(y) == Less,
                CmpOp::Ge => x.cmp_to(y) != Less,
                CmpOp::Gt => x.cmp_to(y) == Greater,
            };
            if !ok {
                return None;
            }
        }
        Some(
            (0..self.axes.len())
                .map(|ai| {
                    let by_constraint = b.get(&self.col_names[ai]).and_then(pick);
                    self.axes[ai].witness_num(path.get(ai).copied().unwrap_or(0), by_constraint)
                })
                .collect(),
        )
    }

    /// Whether a `constraint` can hold anywhere in this box. Interval arithmetic, the same
    /// shape the derived sieve uses: only a relation that is impossible for **every** pair of
    /// values the box allows takes the box out (§15.55).
    ///
    /// An interval coordinate leaves its ends out, and that decides the case where the two
    /// ends meet: `>5万円` against `<=5万円` under `申告額 <= 補償額` has no pair, since the one
    /// value both could share is one the left side never takes. Read with its ends included,
    /// the box looked reachable and was reported as a gap (§15.140). The reading stays over
    /// the rationals, which is what the certificate's checker holds it to.
    fn constraint_impossible(&self, k: &crate::ast::Constraint, path: &[usize]) -> bool {
        let (Some(((la, lo_l), (lb, hi_l))), Some(((ra, lo_r), (rb, hi_r)))) =
            (self.ends_of_name(&k.left, path), self.ends_of_name(&k.right, path))
        else {
            return false;
        };
        use std::cmp::Ordering::*;
        match k.op {
            // left <= right is out of reach when the smallest left is already past the
            // largest right, or equal to it with one of the two left out.
            CmpOp::Le => matches!((la, rb), (Some(x), Some(y)) if x.cmp_to(y) == Greater || (x.cmp_to(y) == Equal && (lo_l || hi_r))),
            CmpOp::Lt => matches!((la, rb), (Some(x), Some(y)) if x.cmp_to(y) != Less),
            CmpOp::Ge => matches!((lb, ra), (Some(x), Some(y)) if x.cmp_to(y) == Less || (x.cmp_to(y) == Equal && (hi_l || lo_r))),
            CmpOp::Gt => matches!((lb, ra), (Some(x), Some(y)) if x.cmp_to(y) != Greater),
        }
    }

    /// The two ends a name can reach inside this box, each with whether it is left out: an
    /// interval coordinate leaves out both of its ends, a single value and a declared range
    /// neither.
    #[allow(clippy::type_complexity)]
    fn ends_of_name(&self, name: &str, path: &[usize]) -> Option<((Option<Rat>, bool), (Option<Rat>, bool))> {
        if let Some(ai) = self.col_names.iter().position(|n| n == name) {
            if let (Some(&ci), Axis::Num { coords, .. }) = (path.get(ai), &self.axes[ai]) {
                match coords.get(ci) {
                    Some(Coord::Point(v)) => return Some(((Some(*v), false), (Some(*v), false))),
                    Some(Coord::Open(a, b)) => return Some(((*a, true), (*b, true))),
                    None => {}
                }
            }
        }
        self.spans.get(name).map(|(a, b)| ((*a, false), (*b, false)))
    }

    /// The sieve of §6.2. For each derived axis, check whether the reachable interval and the
    /// coordinate's interval intersect. If they do not, the point is infeasible. When two or
    /// more constrained derived values share an input, their dependency cannot be examined.
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
    /// For a `string` axis: the prefix each coordinate stands for, and `None` for the one
    /// coordinate that stands for "under none of them" (§15.101). `None` for the whole
    /// list on any other axis.
    pub prefixes: Option<Vec<Option<String>>>,
    /// The grid the axis's values sit on, where it is numeric. Two coordinates that touch
    /// share an end; two that are one step apart have nothing between them, and without
    /// the step a re-checker cannot tell that from a coordinate quietly removed (§15.99).
    pub step: Option<Rat>,
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
    /// The value this row writes into each of the table's own output columns, in the order
    /// `CertTable::decides` names them, and `None` where the cell is not a plain value word
    /// — an amount, an expression, a name that stands for something else. A table below
    /// reads these to learn which rows of this one can put a column at a value, which is
    /// what turns an upstream leaf from a claim into a check (§15.115).
    pub produces: Vec<Option<String>>,
    /// The member table the row was written in. Rows that share one were written in one
    /// table, so they have a cell in the same columns and in no others.
    pub origin: String,
    /// The line the row itself is written on, or 0 for a row an `apply` brought in. Every
    /// cell of the row is on this line; a `clause` whose `when` names no column has no
    /// cell at all, and this is the only thing that says where it is (§15.99).
    pub line: usize,
}

/// One cell, as the re-checker reads it.
pub enum CertCell {
    Any,
    Nothing,
    Is(Vec<String>),
    Not(Vec<String>),
    /// `starts_with "ABC"` — the prefixes the cell names (§15.101).
    Prefix(Vec<String>),
    Cmp(Vec<(&'static str, Option<Rat>)>),
}

/// What the certificate says about one table.
pub struct CertTable {
    pub name: String,
    pub policy: &'static str,
    /// How many columns the table writes to the right of `->`. A re-checker splits a row's
    /// line on `|` and needs this to know where the cells stop and the answers begin: the
    /// cells the certificate names have to be **every** field before them (§15.99).
    pub outputs: usize,
    pub axes: Vec<CertAxis>,
    /// The columns this table writes, in the order `CertRow::produces` lists their values.
    /// A table below names one of these as an axis of kind `upstream`, and that is the link
    /// a re-checker follows to find the rows that can put it at a value (§15.115).
    pub decides: Vec<String>,
    pub rows: Vec<CertRow>,
    /// `unique` only: for each pair of rows, an axis on which their coordinates do not meet.
    /// Checking one entry is one set intersection; a pair that is missing from this list and
    /// not in `undecided` is a certificate that does not hold.
    pub disjoint: Vec<(usize, usize, usize)>,
    /// The pairs the axes do not part. Stated, not proved — a W114 the check could not
    /// settle lands here, and so does a pair a declared precedence orders and a pair the
    /// sieve ruled out, none of which the certificate has a way to claim (§15.114).
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
    /// Rows the sieve rules out entirely: every point of the row's box is one no input
    /// reaches. E102 does not look at the sieve, so `check` passes them; the certificate
    /// names them rather than leaving a row with no point and no reason.
    pub unreachable: Vec<usize>,
    /// The completeness cover: the walk of §6.3, written down. `None` when it ran past the
    /// budget — a certificate says what it does not have.
    pub cover: Option<Cover>,
    /// The `constraint` lines a cover leaf can point at.
    pub constraints: Vec<(String, &'static str, String)>,
    /// What the tables above rule out, written on this table's own axes. A leaf that rests
    /// on them is settled by a coordinate test; the facts themselves are earned back from
    /// the rows of the tables that decide the columns (§15.115).
    pub above: Vec<AboveFact>,
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
    let walked: Vec<String> = f
        .items
        .iter()
        .filter_map(|it| if let crate::ast::Item::Agg(d) = it { Some(d.name.text.clone()) } else { None })
        .collect();
    Some(reg.certificate(t, &w114, &f.inputs, &walked, &applied, cover, c))
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
        Some(Cell::Prefix(ps)) => CertCell::Prefix(ps.clone()),
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
    pub fn certificate(&self, t: &Table, w114: &[(usize, usize)], inputs: &[crate::ast::VarDecl], walked: &[String], applied: &[bool], cover: Option<Cover>, chk: &Checked) -> CertTable {
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
                } else if walked.contains(&self.col_names[ai]) {
                    // What the walk left behind. It is neither an input a caller sends nor
                    // a value a table above decided, and calling it upstream said the
                    // wrong thing about where its points come from (§15.100).
                    "walk"
                } else {
                    "upstream"
                },
                coords: (0..self.axes[ai].len()).map(|c| self.axes[ai].witness(c)).collect(),
                step: match &self.axes[ai] {
                    Axis::Num { step, .. } => Some(*step),
                    _ => None,
                },
                prefixes: match &self.axes[ai] {
                    Axis::Prefix { patterns } => {
                        Some(patterns.iter().map(|p| Some(p.clone())).chain([None]).collect())
                    }
                    _ => None,
                },
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
                produces: (0..t.outputs.len())
                    .map(|oi| match row.outs.get(oi) {
                        Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w)) if !chk.syms.contains_key(w) => {
                            Some(w.clone())
                        }
                        _ => None,
                    })
                    .collect(),
                origin: row.origin.clone().unwrap_or_else(|| t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()),
                line: if applied.get(ri).copied().unwrap_or(false) { 0 } else { row.span.line },
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

        let (mut reach, mut unused, mut unreachable) = (Vec::new(), Vec::new(), Vec::new());
        let mut left = crate::region::DEFAULT_BUDGET;
        for ri in 0..t.rows.len() {
            match self.reach_point(ri, unique, &mut left) {
                Some(p) => {
                    let input = self.witness_pairs(&p);
                    // The values behind the point, solved against the constraints. Where no
                    // assignment was found the certificate states none, and a re-checker
                    // falls back to the weaker reading and says which one it used (§15.99).
                    let nums = self
                        .witness_values(&p)
                        .unwrap_or_else(|| vec![None; self.axes.len()]);
                    reach.push((ri + 1, p, input, nums));
                }
                None if applied.get(ri).copied().unwrap_or(false) => unused.push(ri + 1),
                // The sieve rules out every point of the row. E102 does not sieve, so
                // `check` is silent about it; dropping the row here made the certificate
                // look as though it had simply forgotten one (§15.99).
                None => unreachable.push(ri + 1),
            }
        }
        CertTable {
            name: t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
            policy: if unique { "unique" } else { "first" },
            outputs: t.outputs.len(),
            axes,
            decides: t.outputs.iter().map(|o| o.name.text.clone()).collect(),
            rows,
            disjoint,
            undecided,
            reach,
            unused,
            unreachable,
            above: {
                let mut v = Vec::new();
                if let Some(cv) = cover.as_ref() {
                    self.gather_above(cv, &mut v);
                }
                v
            },
            cover,
            constraints: self.constraint_list(),
        }
    }

    /// A coordinate point that reaches a row: inside its box, feasible, and — under
    /// `policy first` — outside every row above it, since a point an earlier row also takes
    /// is decided by that row and reaches nothing.
    /// A point that reaches a row, or nothing.
    ///
    /// The walk prunes on the prefix and is charged to a budget, both for the same reason
    /// as §6.3: a wide table's full-depth points are a product, and asking the sieve only
    /// at the bottom walks all of them. Twelve columns of eleven coordinates is 11¹¹
    /// points, and the tool sat there (§15.99).
    fn reach_point(&self, ri: usize, unique: bool, budget: &mut i64) -> Option<Vec<usize>> {
        let mut p: Vec<usize> = Vec::new();
        self.reach_rec(ri, unique, &mut p, budget)
    }

    fn reach_rec(&self, ri: usize, unique: bool, p: &mut Vec<usize>, budget: &mut i64) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if p.len() == self.axes.len() {
            if self.feasible(p) == Feasible::No {
                return None;
            }
            if !unique && (0..ri).any(|k| (0..self.axes.len()).all(|a| self.masks[k][a][p[a]])) {
                return None;
            }
            return Some(p.clone());
        }
        let ai = p.len();
        for c in 0..self.axes[ai].len() {
            if !self.masks[ri][ai][c] {
                continue;
            }
            p.push(c);
            let keep = self.feasible(p) != Feasible::No;
            let got = if keep { self.reach_rec(ri, unique, p, budget) } else { None };
            p.pop();
            if got.is_some() {
                return got;
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
    /// The tables above cannot produce this combination. When the reason can be written
    /// small — two spans on one input that do not meet — it comes with it and is re-checked
    /// like any other leaf; otherwise the leaf is bare and is **stated, not proved**
    /// (§15.115).
    ByUpstream(String, Option<Apart>),
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
                return Some(Cover::ByUpstream(self.witness_text(path), self.apart_at(path, ups, chk)));
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

