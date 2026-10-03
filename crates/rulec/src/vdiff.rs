//! The difference between two versions of one rule, over the whole input space (§15.122).
//!
//! `rulec diff` with `--fixtures` answers "how many of these records move". This answers the
//! question that needs no records: **which inputs get a different answer, and is there
//! anything outside them**. Two versions are both total functions over a space the checks
//! already cut into boxes, so the difference is decidable — which the general "did this code
//! change behaviour" question is not.
//!
//! The space is the rule's **columns**, not its inputs. A derived column can cut the input
//! space diagonally (`余裕 = 床面積 - 占有面積` tested at `<10m2` is a slab, not a box), and a
//! walk's summary is not an input at all, so a region stated over inputs alone cannot be
//! written down — and a tool that tried would answer "no difference" where there is one.
//! Stated over columns it is a box again, and it reads the way the rule reads.
//!
//! Each cell of the common refinement of the two versions' axes is settled three ways
//! (§15.99's rule: what has not been proven is not presented as proven).
//!
//! - `Same` — proved. Either the two versions run the *same* computation on the cell (the
//!   same rows fire and their output cells and the expressions above them are identical), or
//!   both answers are constant on the cell and equal.
//! - `Differs` — proved, with the input that shows it.
//! - `Unknown` — the cell could not be settled, or no input realizing it could be built.
//!   Counted and reported, never folded into either of the other two.

use crate::ast::*;
use crate::eval::Val;
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// How many cells the walk will visit before it gives up and says so.
///
/// The bound is on the space, but what it is really for is the clock: the corpus's widest
/// rule is 437,400 cells and takes about seven seconds, so a million is roughly the point
/// where a pull-request gate stops feeling like one. `--budget` moves it either way.
pub const DEFAULT_BUDGET: usize = 1_000_000;

/// One coordinate of one axis: a class of values that every cell of every table treats alike.
#[derive(Debug, Clone, PartialEq)]
pub enum Coord {
    /// An enum value, a boolean, or a string class named by its prefix.
    Word(String),
    /// A string under none of the prefixes any cell names.
    OtherStr,
    Point(Rat),
    /// Strictly between two boundaries; `None` runs on to the declared end.
    Open(Option<Rat>, Option<Rat>),
}

/// What a column is, which decides how a point on it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A declared input: a point here is an input a caller can send.
    Input,
    /// `derive` or `define`: a value computed from the columns above it.
    Derived,
    /// `sum` or `count` over the sequence the rule walks: not an input, and not computable
    /// from one either.
    Walk,
}

#[derive(Debug, Clone)]
pub struct Axis {
    pub col: String,
    pub ty: Ty,
    pub kind: Kind,
    pub coords: Vec<Coord>,
    /// The grid the column's values sit on: 1 for money and quantities, the declared step
    /// for a rate, one day for a date.
    pub step: Rat,
    /// How to write a value of this column back into a cell (100 for a rate, 1 otherwise).
    pub shown: i128,
    pub unit: String,
    pub date: bool,
}

impl Axis {
    pub(crate) fn len(&self) -> usize {
        self.coords.len()
    }
}

/// A box: the coordinates each axis accepts. An axis that accepts all of them is a `-`.
pub type Region = Vec<BTreeSet<usize>>;

/// What changed about what the rule accepts, as opposed to what it answers.
#[derive(Debug, Clone)]
pub struct DomainChange {
    pub what: &'static str,
    pub name: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// One region where the two versions answer differently, with what they answer.
#[derive(Debug, Clone)]
pub struct Change {
    pub region: Region,
    /// Output name → (old, new) **at the witness**, as the wire writes them.
    pub outs: Vec<(String, String, String)>,
    /// Whether every cell of the box moves the same way. False when the amounts come out
    /// of an expression rather than off the row, and then `outs` is the witness's own
    /// transition and not the box's.
    pub uniform: bool,
    /// The rows that fired, old and new, as `(table, row number)` — the shape a fixture's
    /// `trace` uses, so the two answers name a row the same way.
    pub old_rows: Vec<(String, usize)>,
    pub new_rows: Vec<(String, usize)>,
    /// One input that shows it.
    pub witness: BTreeMap<String, Val>,
    /// How many cells of the refinement the box holds.
    pub cells: usize,
}

/// The whole answer.
#[derive(Debug, Clone)]
pub struct VDiff {
    pub rule: String,
    pub old_version: String,
    pub new_version: String,
    pub axes: Vec<Axis>,
    pub domain: Vec<DomainChange>,
    pub changes: Vec<Change>,
    /// Regions the walk could not settle, with why.
    pub unknown: Vec<(Region, String)>,
    pub cells: usize,
    pub feasible: usize,
    pub same: usize,
    pub differing: usize,
    pub unsettled: usize,
    /// Cells no input could be built for. A cell that was never realized was never
    /// compared, so it is not one the "outside this region they answer alike" claim covers.
    pub unrealized: usize,
    /// True when the space is larger than the budget: the answer is then about nothing.
    pub over_budget: bool,
    /// Set when the two versions are not comparable cell by cell at all.
    pub blocked: Option<String>,
    /// When both versions are one step of a state machine (§15.148): the calls, not the
    /// single call, that the change reaches.
    pub machine: Option<MachineDiff>,
}

/// What a change to a machine does to sequences of calls (§15.148).
#[derive(Debug, Clone)]
pub struct MachineDiff {
    /// The carried input and output.
    pub carry: (String, String),
    /// The initial state, old and new.
    pub initial: (String, String),
    /// The shortest sequence of calls the two versions answer differently. Every call but the
    /// last is answered alike by both — the shortest one cannot have parted earlier — and the
    /// last is where they part. Empty when the change touches no state a case reaches.
    pub shortest: Vec<MStep>,
    /// How many of the regions where the answers differ lie wholly in states no case reaches
    /// from the initial state under the old version: no new case will meet them.
    pub unreached: usize,
    /// What becomes of a case in progress: a state the old version can reach, and what the
    /// new version does to a case sitting in it.
    pub migration: Vec<(String, &'static str)>,
}

/// One call of a trace across two versions.
#[derive(Debug, Clone)]
pub struct MStep {
    pub inputs: BTreeMap<String, Val>,
    pub old: Vec<(String, Option<Val>)>,
    pub new: Vec<(String, Option<Val>)>,
}

impl VDiff {
    /// Whether anything moved. The exit code is this: a region where the answers differ, or
    /// a change in what the rule accepts. A region that could not be settled is **not** one
    /// — the same line `check` draws around W114, where what could not be decided is
    /// reported and does not fail the run. Whoever wants the stricter gate reads `total`.
    pub fn any(&self) -> bool {
        !self.changes.is_empty() || !self.domain.is_empty()
    }
    /// Whether "outside the reported region the two answer alike" is a claim this run
    /// earned. A cell that could not be realized was never compared, so it takes the claim
    /// away just as an unsettled one does.
    pub fn total(&self) -> bool {
        !self.over_budget && self.blocked.is_none() && self.unknown.is_empty() && self.unrealized == 0
    }
}

// ---------------------------------------------------------------------------------------
// Axes
// ---------------------------------------------------------------------------------------

fn lit_rat(l: &Lit, want: &Ty) -> Option<Rat> {
    match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, want),
        Lit::Date(y, m, d) if *want == Ty::Date => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

fn tables_of(f: &RuleFile) -> Vec<&Table> {
    f.items
        .iter()
        .filter_map(|it| if let Item::Table(t) = it { Some(t) } else { None })
        .collect()
}

/// Every comparison `<name> <op> <literal>` an expression makes, so that a threshold hidden
/// in a `define` becomes a boundary on the column it tests. Without this, `大口 = 注文金額 >=
/// 3万円` would leave 注文金額 uncut and the region would be stated over `大口` where it could
/// have been stated over the amount the caller actually sends.
fn expr_cuts(e: &Expr, c: &Checked, out: &mut Vec<(String, Rat)>) {
    if let Expr::Bin(l, op, r, _) = e {
        if matches!(op, BinOp::Le | BinOp::Ge | BinOp::Lt | BinOp::Gt | BinOp::Eq) {
            if let (Expr::Name(n, _), Expr::Lit(lit, _)) = (&**l, &**r) {
                if let Some(ty) = c.ty_of(n) {
                    if let Some(v) = lit_rat(lit, &ty) {
                        out.push((n.clone(), v));
                    }
                }
            }
            if let (Expr::Lit(lit, _), Expr::Name(n, _)) = (&**l, &**r) {
                if let Some(ty) = c.ty_of(n) {
                    if let Some(v) = lit_rat(lit, &ty) {
                        out.push((n.clone(), v));
                    }
                }
            }
        }
        expr_cuts(l, c, out);
        expr_cuts(r, c, out);
    }
    if let Expr::Call(_, args, _) = e {
        for a in args {
            expr_cuts(a, c, out);
        }
    }
}

/// The columns that get an axis, in dependency order: the inputs, then every `derive`,
/// `define` and walk summary as it is declared. A table's own output is computed, not
/// quantified over — the table says what it is.
/// The columns that get an axis, in dependency order.
///
/// The inputs always. A `derive`, a `define` or a walk's summary **only when some table
/// tests it**: that is the one case where its own boundaries cut the space and a point on
/// it has to be quantified over. One that is merely read by an expression is computed from
/// the columns above it, and giving it an axis would multiply the space by a factor whose
/// every value but one is an input no caller can send. A declared output never: it is the
/// answer, not a coordinate.
fn columns(f: &RuleFile, also: &RuleFile) -> Vec<(String, Kind)> {
    columns_with(f, also, &BTreeSet::new())
}

/// `columns`, with `more` counted as tested: the names a question about the rule tests that no
/// table does (§15.148).
pub(crate) fn columns_with(f: &RuleFile, also: &RuleFile, more: &BTreeSet<String>) -> Vec<(String, Kind)> {
    let outs: BTreeSet<&str> = f.outputs.iter().map(|o| o.name.text.as_str()).collect();
    let mut tested: BTreeSet<&str> = more.iter().map(|s| s.as_str()).collect();
    for g in [f, also] {
        for it in &g.items {
            if let Item::Table(t) = it {
                for (n, _) in &t.inputs {
                    tested.insert(n.as_str());
                }
            }
        }
    }
    let mut out: Vec<(String, Kind)> = f.inputs.iter().map(|i| (i.name.text.clone(), Kind::Input)).collect();
    for it in &f.items {
        let (name, kind) = match it {
            Item::Derived(d) => (d.name.text.as_str(), Kind::Derived),
            Item::Define(d) => (d.name.text.as_str(), Kind::Derived),
            Item::Agg(a) => (a.name.text.as_str(), Kind::Walk),
            Item::Table(_) => continue,
        };
        if tested.contains(name) && !outs.contains(name) {
            out.push((name.to_string(), kind));
        }
    }
    out
}

/// Build one axis over the union of what both versions cut the column at.
fn axis_for(col: &str, kind: Kind, ty: &Ty, versions: &[(&RuleFile, &Checked)]) -> Option<Axis> {
    axis_with(col, kind, ty, versions, &[])
}

/// `axis_for`, cut also at `extra`: boundaries that no cell of the rule names but that a
/// question about the rule does — a `once` line's cell, tested on the value an output takes
/// from this column (§15.148).
pub(crate) fn axis_with(
    col: &str,
    kind: Kind,
    ty: &Ty,
    versions: &[(&RuleFile, &Checked)],
    extra: &[(String, Rat)],
) -> Option<Axis> {
    let c0 = versions[0].1;
    match ty {
        Ty::Opt(inner) => {
            // §6.2: an optional column has one coordinate more than the enum it wraps —
            // the absent value, which `none` in a cell is the test for.
            let mut ax = axis_with(col, kind, inner, versions, extra)?;
            ax.coords.insert(0, Coord::Word(crate::kw::NONE.into()));
            ax.ty = ty.clone();
            Some(ax)
        }
        Ty::Enum(en) => {
            // The values **both** versions have. One that only the new version takes is not
            // an input the old one can be asked about; that it was added is reported as a
            // change to the door, not as an answer that moved.
            let mut values: Vec<String> = Vec::new();
            for v in versions[0].1.enums.get(en).into_iter().flatten() {
                if versions.iter().all(|(_, c)| c.enums.get(en).is_some_and(|vs| vs.contains(v))) && !values.contains(v) {
                    values.push(v.clone());
                }
            }
            Some(Axis {
                col: col.into(),
                ty: ty.clone(),
                kind,
                coords: values.into_iter().map(Coord::Word).collect(),
                step: Rat::int(1),
                shown: 1,
                unit: String::new(),
                date: false,
            })
        }
        Ty::Bool => Some(Axis {
            col: col.into(),
            ty: ty.clone(),
            kind,
            coords: vec![Coord::Word(crate::kw::FALSE.into()), Coord::Word(crate::kw::TRUE.into())],
            step: Rat::int(1),
            shown: 1,
            unit: String::new(),
            date: false,
        }),
        Ty::Str => {
            let mut patterns: Vec<String> = Vec::new();
            for (f, _) in versions {
                for t in tables_of(f) {
                    let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
                    for row in &t.rows {
                        if let Some(Cell::Prefix(ps)) = row.cells.get(ci) {
                            for p in ps {
                                if !patterns.contains(p) {
                                    patterns.push(p.clone());
                                }
                            }
                        }
                    }
                }
            }
            patterns.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
            let mut coords: Vec<Coord> = patterns.into_iter().map(Coord::Word).collect();
            coords.push(Coord::OtherStr);
            Some(Axis {
                col: col.into(),
                ty: ty.clone(),
                kind,
                coords,
                step: Rat::int(1),
                shown: 1,
                unit: String::new(),
                date: false,
            })
        }
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
            let mut bounds: BTreeSet<(i128, i128)> = BTreeSet::new();
            let (mut lo, mut hi) = (None, None);
            for (f, c) in versions {
                // The **narrower** end on each side: the values both versions accept. A
                // range that only one of them takes is a change to the door (`domain`), not
                // an answer that moved, and quantifying over it would compare a version
                // against an input it refuses.
                if let Some((l, h)) = c.ranges.get(col) {
                    if let Some(l) = l {
                        lo = Some(match lo {
                            None => *l,
                            Some(p) => if l.cmp_to(p) == Ordering::Greater { *l } else { p },
                        });
                    }
                    if let Some(h) = h {
                        hi = Some(match hi {
                            None => *h,
                            Some(p) => if h.cmp_to(p) == Ordering::Less { *h } else { p },
                        });
                    }
                }
                for t in tables_of(f) {
                    let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
                    for row in &t.rows {
                        match row.cells.get(ci) {
                            Some(Cell::Cmp(cs)) => {
                                for (_, l) in cs {
                                    if let Some(v) = lit_rat(l, ty) {
                                        bounds.insert((v.num, v.den));
                                    }
                                }
                            }
                            Some(Cell::Lit(l)) => {
                                if let Some(v) = lit_rat(l, ty) {
                                    bounds.insert((v.num, v.den));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                let mut cuts = Vec::new();
                for it in &f.items {
                    match it {
                        Item::Derived(d) => expr_cuts(&d.expr, c, &mut cuts),
                        Item::Define(d) => expr_cuts(&d.expr, c, &mut cuts),
                        _ => {}
                    }
                }
                if let Some(r) = &f.result {
                    expr_cuts(&r.expr, c, &mut cuts);
                }
                for (n, v) in cuts {
                    if n == col {
                        bounds.insert((v.num, v.den));
                    }
                }
            }
            for (n, v) in extra {
                if n == col {
                    bounds.insert((v.num, v.den));
                }
            }
            if let Some(l) = lo {
                bounds.insert((l.num, l.den));
            }
            if let Some(h) = hi {
                bounds.insert((h.num, h.den));
            }
            let inside = |x: &Rat| {
                lo.is_none_or(|l| x.cmp_to(l) != Ordering::Less) && hi.is_none_or(|h| x.cmp_to(h) != Ordering::Greater)
            };
            let mut b: Vec<Rat> = bounds.into_iter().map(|(n, d)| Rat { num: n, den: d }).filter(inside).collect();
            b.sort_by(|a, x| a.cmp_to(*x));
            let step = match ty {
                Ty::Rate => Rat::new(1, *c0.scales.get(col).unwrap_or(&100)),
                _ => Rat::int(1),
            };
            let mut coords = Vec::new();
            if b.is_empty() {
                coords.push(Coord::Open(lo, hi));
            } else {
                if lo.is_none() {
                    coords.push(Coord::Open(None, Some(b[0])));
                }
                for (i, x) in b.iter().enumerate() {
                    coords.push(Coord::Point(*x));
                    if i + 1 < b.len() && b[i + 1].sub(*x).cmp_to(step) == Ordering::Greater {
                        coords.push(Coord::Open(Some(*x), Some(b[i + 1])));
                    }
                }
                if hi.is_none() {
                    coords.push(Coord::Open(Some(*b.last().unwrap()), None));
                }
            }
            let unit = match ty {
                Ty::Money { cur, .. } => cur.clone(),
                Ty::Qty { unit, .. } => unit.clone(),
                Ty::Rate => "%".into(),
                _ => String::new(),
            };
            Some(Axis {
                col: col.into(),
                ty: ty.clone(),
                kind,
                coords,
                step,
                shown: if matches!(ty, Ty::Rate) { 100 } else { 1 },
                unit,
                date: matches!(ty, Ty::Date),
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------
// Coordinates as values
// ---------------------------------------------------------------------------------------

/// The closed interval a numeric coordinate stands for, with the open ends already moved one
/// grid step in. A coordinate that holds no value at all cannot occur: `num_coords` only
/// makes an open interval where two boundaries are more than one step apart.
pub(crate) fn ival(co: &Coord, step: Rat) -> (Option<Rat>, Option<Rat>) {
    match co {
        Coord::Point(v) => (Some(*v), Some(*v)),
        Coord::Open(lo, hi) => (lo.map(|x| x.add(step)), hi.map(|x| x.sub(step))),
        _ => (None, None),
    }
}

/// The value to try first for a coordinate: the point itself, or one grid step inside the
/// closed end. §11 principle 2 — the value most likely to show an off-by-one.
pub(crate) fn primary(co: &Coord, step: Rat) -> Option<Rat> {
    match co {
        Coord::Point(v) => Some(*v),
        Coord::Open(Some(lo), _) => Some(lo.add(step)),
        Coord::Open(None, Some(hi)) => Some(hi.sub(step)),
        Coord::Open(None, None) => Some(Rat::zero()),
        _ => None,
    }
}

pub(crate) fn val_of(ax: &Axis, r: Rat) -> Val {
    if ax.date {
        let (y, m, d) = crate::types::ord_to_date(r);
        Val::Date(y, m, d)
    } else {
        Val::Num(r)
    }
}

/// Whether the coordinate holds the value. Coordinates are cut at every boundary either
/// version names, so a coordinate is wholly inside or wholly outside every test — which is
/// what lets one point stand for the whole of it.
pub(crate) fn holds(ax: &Axis, ci: usize, v: &Val) -> bool {
    match (&ax.coords[ci], v) {
        (Coord::Word(w), Val::Enum(s) | Val::Str(s)) => w == s,
        (Coord::Word(w), Val::Bool(b)) => (w == crate::kw::TRUE) == *b,
        (Coord::OtherStr, Val::Str(s)) => !ax
            .coords
            .iter()
            .any(|c| matches!(c, Coord::Word(p) if s.starts_with(p.as_str()))),
        (co, Val::Num(r)) => {
            let (lo, hi) = ival(co, ax.step);
            lo.is_none_or(|l| r.cmp_to(l) != Ordering::Less) && hi.is_none_or(|h| r.cmp_to(h) != Ordering::Greater)
        }
        (co, Val::Date(y, m, d)) => {
            let r = crate::types::date_ord(*y, *m, *d);
            let (lo, hi) = ival(co, ax.step);
            lo.is_none_or(|l| r.cmp_to(l) != Ordering::Less) && hi.is_none_or(|h| r.cmp_to(h) != Ordering::Greater)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------
// Realizing a cell: an input the cell actually stands for
// ---------------------------------------------------------------------------------------

/// The element values worth trying, when the rule walks a sequence. One per combination of
/// the element's own fields at their first candidate — enough to reach every verdict the
/// per-element table can give, because a field is cut at every boundary its cells name.
fn element_menu(f: &RuleFile, c: &Checked, cap: usize) -> Vec<BTreeMap<String, Val>> {
    let Some(el) = &f.elements else { return Vec::new() };
    let mut menu: Vec<BTreeMap<String, Val>> = vec![BTreeMap::new()];
    for fld in &el.fields {
        let Some(ty) = c.ty_of(&fld.name.text) else { continue };
        let Some(ax) = axis_for(&fld.name.text, Kind::Input, &ty, &[(f, c)]) else { continue };
        let mut next = Vec::new();
        for base in &menu {
            for ci in 0..ax.coords.len() {
                let Some(v) = coord_val(&ax, ci) else { continue };
                let mut m = base.clone();
                m.insert(fld.name.text.clone(), v);
                next.push(m);
                if next.len() >= cap {
                    break;
                }
            }
            if next.len() >= cap {
                break;
            }
        }
        menu = next;
    }
    menu
}

/// The value that stands for a coordinate.
pub(crate) fn coord_val(ax: &Axis, ci: usize) -> Option<Val> {
    match &ax.coords[ci] {
        Coord::Word(w) => Some(match ax.ty {
            Ty::Bool => Val::Bool(w == crate::kw::TRUE),
            Ty::Str => Val::Str(w.clone()),
            _ => Val::Enum(w.clone()),
        }),
        Coord::OtherStr => Some(Val::Str(other_string(ax))),
        co => primary(co, ax.step).map(|r| val_of(ax, r)),
    }
}

/// A string under none of the prefixes the axis names.
fn other_string(ax: &Axis) -> String {
    for ch in "zxqjk".chars() {
        let s = ch.to_string();
        if !ax.coords.iter().any(|c| matches!(c, Coord::Word(p) if s.starts_with(p.as_str()) || p.is_empty())) {
            return s;
        }
    }
    let n = ax
        .coords
        .iter()
        .filter_map(|c| if let Coord::Word(p) = c { Some(p.chars().count()) } else { None })
        .max()
        .unwrap_or(0);
    "z".repeat(n + 1)
}

/// Everything one run of a version says about one input.
pub(crate) struct Answer {
    pub(crate) outs: Vec<(String, Option<Val>)>,
    pub(crate) rows: Vec<(String, usize)>,
    pub(crate) binds: HashMap<String, Val>,
}

pub(crate) fn run(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> Answer {
    let (outs, _, rows, binds) = crate::eval::run_all_traced(f, c, inputs);
    Answer { outs, rows, binds }
}

/// Whether the `constraint` lines hold of an assignment. They are a promise the caller
/// makes, not something the evaluator enforces, so a cell realized by inputs that break one
/// is a cell no caller reaches (§15.55).
fn constraints_hold(f: &RuleFile, m: &HashMap<String, Val>) -> bool {
    f.constraints.iter().all(|k| match (m.get(&k.left), m.get(&k.right)) {
        (Some(Val::Num(a)), Some(Val::Num(b))) => match k.op {
            CmpOp::Le => a.cmp_to(*b) != Ordering::Greater,
            CmpOp::Lt => a.cmp_to(*b) == Ordering::Less,
            CmpOp::Ge => a.cmp_to(*b) != Ordering::Less,
            CmpOp::Gt => a.cmp_to(*b) == Ordering::Greater,
        },
        _ => true,
    })
}

/// A value on the axis's own grid, inside the coordinate.
fn clamp_to(ax: &Axis, ci: usize, v: Rat) -> Rat {
    let (lo, hi) = ival(&ax.coords[ci], ax.step);
    // Land on the grid the column's values sit on, then back inside the ends.
    let k = v.div(ax.step);
    let mut r = ax.step.mul(Rat::int(k.num.div_euclid(k.den)));
    if let Some(l) = lo {
        if r.cmp_to(l) == Ordering::Less {
            r = l;
        }
    }
    if let Some(h) = hi {
        if r.cmp_to(h) == Ordering::Greater {
            r = h;
        }
    }
    r
}

/// The `derive` and `define` declarations that read nothing but inputs. Those are the ones
/// that cut the input space diagonally, and the only ones a cell can be solved for without
/// running the tables.
fn scalar_derives(f: &RuleFile, inputs: &BTreeSet<String>) -> Vec<(String, Expr)> {
    let all: Vec<(String, Expr)> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Derived(d) => Some((d.name.text.clone(), d.expr.clone())),
            Item::Define(d) => Some((d.name.text.clone(), d.expr.clone())),
            _ => None,
        })
        .collect();
    // `達成率 = 合計点 ÷ 50` reads no input directly, yet it is a function of four of them
    // through `合計点`. The chain is what matters, so the set grows to a fixed point.
    //
    // The names come out of the expression rather than out of `Checked`: `derived_deps` is
    // filled for `derive` and `define_deps` only for the boolean `define`s, so a `define`
    // of any other type is in neither, and reading them would quietly leave it out.
    let mut scalar: BTreeSet<String> = inputs.clone();
    loop {
        let mut grew = false;
        for (n, e) in &all {
            if scalar.contains(n) {
                continue;
            }
            let ds = expr_names(e);
            if !ds.is_empty() && ds.iter().all(|d| scalar.contains(d)) {
                scalar.insert(n.clone());
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    all.into_iter().filter(|(n, _)| scalar.contains(n)).collect()
}

/// Every name an expression reads.
fn expr_names(e: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    fn go(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Name(n, _) => {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
            Expr::Lit(_, _) => {}
            Expr::Bin(l, _, r, _) => {
                go(l, out);
                go(r, out);
            }
            Expr::Call(_, args, _) => {
                for a in args {
                    go(a, out);
                }
            }
        }
    }
    go(e, &mut out);
    out
}

/// The inputs a column is a function of, followed through the `derive`s between them.
fn root_inputs(name: &str, scalars: &[(String, Expr)], inputs: &BTreeSet<String>) -> Vec<String> {
    let by: HashMap<&str, &Expr> = scalars.iter().map(|(n, e)| (n.as_str(), e)).collect();
    let mut out: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack = vec![name.to_string()];
    while let Some(n) = stack.pop() {
        if !seen.insert(n.clone()) {
            continue;
        }
        if inputs.contains(&n) {
            if !out.contains(&n) {
                out.push(n);
            }
            continue;
        }
        if let Some(e) = by.get(n.as_str()) {
            stack.extend(expr_names(e));
        }
    }
    out.sort();
    out
}

/// The inputs, plus every scalar `derive` above them already worked out. `達成率 = 合計点 ÷
/// 50` cannot be read off the inputs alone: the name in the middle has to be bound first.
fn chained(base: &HashMap<String, Val>, scalars: &[(String, Expr)], c: &Checked) -> HashMap<String, Val> {
    let mut m = base.clone();
    for (n, e) in scalars {
        if let Some(v) = crate::eval::Env::new(c, m.clone()).expr(e) {
            m.insert(n.clone(), v);
        }
    }
    m
}

/// Build one input the cell stands for, or nothing when none could be built.
///
/// The inputs come off their own coordinates. The columns that are computed — a `derive`, a
/// `define`, a walk's summary — are then held to the cell's coordinate: a cell that asks for
/// `余裕` between 10m2 and 15m2 is only realized by inputs whose difference actually lands
/// there, and no choice of coordinates on `床面積` and `占有面積` alone puts it there. So the
/// slab is **solved for**: an expression is linear on a cell (§5), so one probe per input
/// gives its coefficient and one division gives the value to move it to.
/// Other points of the same cell. The first input built for a cell is one grid step inside
/// the closed end (§11 principle 2), which is where an off-by-one shows; it is not where a
/// **rate** shows. `10USD` and `17USD` per million tokens both come to nothing at one
/// token, so a cell whose token count is an open span agrees at that point and differs
/// everywhere else in it. So the other ends are tried too, and the cell is only left
/// unsettled when none of them parts the two versions.
fn other_points(axes: &[Axis], cell: &[usize], base: &HashMap<String, Val>, cap: usize) -> Vec<HashMap<String, Val>> {
    let open: Vec<usize> = (0..axes.len())
        .filter(|&i| axes[i].kind == Kind::Input && matches!(axes[i].coords[cell[i]], Coord::Open(_, _)))
        .collect();
    if open.is_empty() {
        return Vec::new();
    }
    let far = |i: usize| -> Option<Val> {
        let (lo, hi) = ival(&axes[i].coords[cell[i]], axes[i].step);
        // The end away from the one the first point took.
        let v = match (lo, hi) {
            (Some(_), Some(h)) => h,
            (None, Some(h)) => h,
            (Some(l), None) => l,
            (None, None) => return None,
        };
        Some(val_of(&axes[i], v))
    };
    let mut out = Vec::new();
    // Every open axis at its far end at once, then one at a time.
    let mut all = base.clone();
    for &i in &open {
        if let Some(v) = far(i) {
            all.insert(axes[i].col.clone(), v);
        }
    }
    out.push(all);
    for &i in open.iter().take(cap.saturating_sub(1)) {
        if let Some(v) = far(i) {
            let mut m = base.clone();
            m.insert(axes[i].col.clone(), v);
            out.push(m);
        }
    }
    out
}

/// The axes the scalar solve and the feasibility test actually look at: every derived
/// column that has an axis, every input it is a function of, and every column a
/// `constraint` relates. Whether a cell can be realized depends on **these coordinates
/// alone** — the rest are carried along — so the answer is worth remembering: a rule with
/// ten inputs asks the same small question hundreds of thousands of times.
fn cone_of(axes: &[Axis], f: &RuleFile, c: &Checked, scalars: &[(String, Expr)]) -> Vec<usize> {
    let ax_of: HashMap<&str, usize> = axes.iter().enumerate().map(|(i, a)| (a.col.as_str(), i)).collect();
    let inputs: BTreeSet<String> = axes.iter().filter(|a| a.kind == Kind::Input).map(|a| a.col.clone()).collect();
    let _ = c;
    let mut set: BTreeSet<usize> = BTreeSet::new();
    for (name, _) in scalars {
        let Some(&ai) = ax_of.get(name.as_str()) else { continue };
        set.insert(ai);
        for d in root_inputs(name, scalars, &inputs) {
            if let Some(&di) = ax_of.get(d.as_str()) {
                set.insert(di);
            }
        }
    }
    if !set.is_empty() {
        for k in &f.constraints {
            for n in [&k.left, &k.right] {
                if let Some(&i) = ax_of.get(n.as_str()) {
                    set.insert(i);
                }
            }
        }
    }
    set.into_iter().collect()
}

/// What a cell's cone settled to: the values its inputs take, or nothing when no input
/// realizes those coordinates.
type Solved = Option<HashMap<String, Val>>;

#[allow(clippy::too_many_arguments)]
fn realize(
    axes: &[Axis],
    cell: &[usize],
    f: &RuleFile,
    c: &Checked,
    menu: &[BTreeMap<String, Val>],
    scalars: &[(String, Expr)],
    cone: &[usize],
    memo: &mut HashMap<Vec<usize>, Solved>,
    seed: Option<&HashMap<String, Val>>,
) -> Option<(HashMap<String, Val>, Answer)> {
    let ax_of: HashMap<&str, usize> = axes.iter().enumerate().map(|(i, a)| (a.col.as_str(), i)).collect();
    let computed: Vec<usize> = (0..axes.len()).filter(|&i| axes[i].kind != Kind::Input).collect();
    let mut base: HashMap<String, Val> = axes
        .iter()
        .enumerate()
        .filter(|(_, a)| a.kind == Kind::Input)
        .filter_map(|(i, a)| coord_val(a, cell[i]).map(|v| (a.col.clone(), v)))
        .collect();
    // A seeded run asks for a *different* point of the same cell, so it neither reads the
    // memo nor writes to it: the memo answers "can this cell be realized at all".
    if let Some(sd) = seed {
        for (k, v) in sd {
            base.insert(k.clone(), v.clone());
        }
    }
    let key: Vec<usize> = cone.iter().map(|&i| cell[i]).collect();
    if seed.is_none() && !cone.is_empty() {
        if let Some(hit) = memo.get(&key) {
            match hit {
                None => return None,
                Some(vals) => {
                    for (k, v) in vals {
                        base.insert(k.clone(), v.clone());
                    }
                    return finish(axes, cell, f, c, menu, base, &computed);
                }
            }
        }
    }

    // Solve the scalar derives. One move of one input rarely lands the target — `達成率`
    // is a quarter of each of four scores, so reaching 90% means moving all four — so each
    // input is moved as far towards the target as its own coordinate allows, and the next
    // one takes up the rest. The expression is linear on a cell (§5), so one probe gives a
    // coefficient and the walk converges or stops making progress.
    //
    // Two derives can also read the same input (`最低差` and `支払差` both read `現在残額`),
    // so settling one can unsettle another: the whole pass repeats until they hold at once.
    let input_names: BTreeSet<String> = axes.iter().filter(|a| a.kind == Kind::Input).map(|a| a.col.clone()).collect();
    let climbed: Option<HashMap<String, Val>> = 'climb: {
    for _pass in 0..4 {
        let mut moved = false;
        for (name, expr) in scalars {
            let Some(&ai) = ax_of.get(name.as_str()) else { continue };
            let value = |m: &HashMap<String, Val>| -> Option<Val> {
                crate::eval::Env::new(c, chained(m, scalars, c)).expr(expr)
            };
            let num = |m: &HashMap<String, Val>| -> Option<Rat> {
                match value(m) {
                    Some(Val::Num(r)) => Some(r),
                    Some(Val::Bool(b)) => Some(Rat::int(i128::from(b))),
                    Some(Val::Date(y, mo, d)) => Some(crate::types::date_ord(y, mo, d)),
                    _ => None,
                }
            };
            let ok = |m: &HashMap<String, Val>| -> bool {
                match value(m) {
                    Some(v) => holds(&axes[ai], cell[ai], &v),
                    None => false,
                }
            };
            if ok(&base) {
                continue;
            }
            let (lo, hi) = ival(&axes[ai].coords[cell[ai]], axes[ai].step);
            let Some(mut now) = num(&base) else { return None };
            // Aim at the middle of the target, so that landing on the grid stays inside.
            let target = match (lo, hi) {
                (Some(l), Some(h)) => l.add(h).div(Rat::int(2)),
                (Some(l), None) => l,
                (None, Some(h)) => h,
                (None, None) => now,
            };
            let gap = |v: Rat| {
                let d = v.sub(target);
                if d.num < 0 { Rat { num: -d.num, den: d.den } } else { d }
            };
            // Move the inputs no other axis reads first. `最低差` and `支払差` both read
            // `現在残額`, and settling one through the shared input is what unsettles the
            // other; each has an input of its own that costs nothing to move.
            let mut deps = root_inputs(name, scalars, &input_names);
            let shared = |x: &String| -> usize {
                scalars
                    .iter()
                    .filter(|(m, _)| m != name && ax_of.contains_key(m.as_str()))
                    .filter(|(m, _)| root_inputs(m, scalars, &input_names).contains(x))
                    .count()
            };
            deps.sort_by_key(|x| (shared(x), x.clone()));
            let mut walked = false;
            'walk: for _step in 0..deps.len().max(1) * 2 {
                let mut progress = false;
                for d in &deps {
                    if ok(&base) {
                        break 'walk;
                    }
                    let Some(&di) = ax_of.get(d.as_str()) else { continue };
                    if axes[di].kind != Kind::Input {
                        continue;
                    }
                    let Some(Val::Num(x0)) = base.get(d).cloned() else { continue };
                    let mut probe = base.clone();
                    probe.insert(d.clone(), Val::Num(x0.add(axes[di].step)));
                    let Some(bumped) = num(&probe) else { continue };
                    let coef = bumped.sub(now);
                    if coef.cmp_to(Rat::zero()) == Ordering::Equal {
                        continue;
                    }
                    let steps = target.sub(now).div(coef.div(axes[di].step));
                    let want = clamp_to(&axes[di], cell[di], x0.add(steps));
                    if want.cmp_to(x0) == Ordering::Equal {
                        continue;
                    }
                    let mut try_ = base.clone();
                    try_.insert(d.clone(), Val::Num(want));
                    let Some(after) = num(&try_) else { continue };
                    // Take the move when it gets closer and keeps the caller's promises.
                    if gap(after).cmp_to(gap(now)) == Ordering::Less && constraints_hold(f, &try_) {
                        base = try_;
                        now = after;
                        progress = true;
                        walked = true;
                    }
                }
                if !progress {
                    break;
                }
            }
            if !ok(&base) {
                break 'climb None;
            }
            moved |= walked;
        }
        if !moved {
            break;
        }
    }
    let mut settled = constraints_hold(f, &base);
    // Every scalar derive has to hold at the same time, not one after another.
    if settled {
        for (name, expr) in scalars {
            let Some(&ai) = ax_of.get(name.as_str()) else { continue };
            match crate::eval::Env::new(c, chained(&base, scalars, c)).expr(expr) {
                Some(v) if holds(&axes[ai], cell[ai], &v) => {}
                _ => {
                    settled = false;
                    break;
                }
            }
        }
    }
    if settled { Some(base) } else { None }
    };
    // The walk moves one input at a time towards the middle of each target, which cannot
    // reach a point that only the **intersection** of two targets holds — `残高A > 1000円`
    // and `残高B < 3980円` off one 合計 is such a cell. The elimination solves the whole
    // cell at once and hands back a point (§15.129). What it hands back is then held to the
    // coordinates exactly as the walk's own answer is, so nothing is trusted here on the
    // strength of the arithmetic: a point that does not verify is a cell not realized, as
    // before.
    let base = match climbed.or_else(|| solved_point(axes, cell, f, c, scalars, &input_names)) {
        Some(b) => b,
        None => {
            if seed.is_none() && !cone.is_empty() {
                memo.insert(key, None);
            }
            return None;
        }
    };
    if seed.is_none() && !cone.is_empty() {
        memo.insert(
            key,
            Some(cone.iter().filter(|&&i| axes[i].kind == Kind::Input).filter_map(|&i| base.get(&axes[i].col).map(|v| (axes[i].col.clone(), v.clone()))).collect()),
        );
    }
    finish(axes, cell, f, c, menu, base, &computed)
}

/// The part of realizing a cell that the cone cannot be asked about: build the sequence a
/// walk needs, run the rule, and hold every computed column to its coordinate.
#[allow(clippy::too_many_arguments)]
fn finish(
    axes: &[Axis],
    cell: &[usize],
    f: &RuleFile,
    c: &Checked,
    menu: &[BTreeMap<String, Val>],
    base: HashMap<String, Val>,
    computed: &[usize],
) -> Option<(HashMap<String, Val>, Answer)> {
    let walks: Vec<usize> = (0..axes.len()).filter(|&i| axes[i].kind == Kind::Walk).collect();
    let inputs = if walks.is_empty() {
        base
    } else {
        build_seq(axes, cell, f, c, &base, menu, &walks)?
    };
    let a = run(f, c, inputs.clone());
    if computed.iter().all(|&i| match a.binds.get(&axes[i].col) {
        Some(v) => holds(&axes[i], cell[i], v),
        None => false,
    }) {
        Some((inputs, a))
    } else {
        None
    }
}

/// A sequence whose summaries land on the cell's coordinates. Each element of the menu is
/// weighed once — what one of it does to each summary — and then taken as many times as the
/// target needs. `sum` adds, `count` counts, and both are monotone in the number of copies,
/// so this reaches every reachable total.
fn build_seq(
    axes: &[Axis],
    cell: &[usize],
    f: &RuleFile,
    c: &Checked,
    scalars: &HashMap<String, Val>,
    menu: &[BTreeMap<String, Val>],
    walks: &[usize],
) -> Option<HashMap<String, Val>> {
    let over = f.items.iter().find_map(|it| if let Item::Agg(a) = it { Some(a.over.clone()) } else { None })?;
    let want: Vec<(usize, Option<Rat>, Option<Rat>)> =
        walks.iter().map(|&i| (i, ival(&axes[i].coords[cell[i]], axes[i].step).0, ival(&axes[i].coords[cell[i]], axes[i].step).1)).collect();
    let weigh = |e: &BTreeMap<String, Val>| -> Vec<Rat> {
        let mut m = scalars.clone();
        m.insert(over.clone(), Val::Seq(vec![e.clone()]));
        let a = run(f, c, m);
        walks
            .iter()
            .map(|&i| match a.binds.get(&axes[i].col) {
                Some(Val::Num(r)) => *r,
                _ => Rat::zero(),
            })
            .collect()
    };
    let mut empty = scalars.clone();
    empty.insert(over.clone(), Val::Seq(Vec::new()));
    let zero = run(f, c, empty.clone());
    let at = |a: &Answer, i: usize| -> Rat {
        match a.binds.get(&axes[i].col) {
            Some(Val::Num(r)) => *r,
            _ => Rat::zero(),
        }
    };
    let fits = |a: &Answer| {
        want.iter().all(|(i, lo, hi)| {
            let v = at(a, *i);
            lo.is_none_or(|l| v.cmp_to(l) != Ordering::Less) && hi.is_none_or(|h| v.cmp_to(h) != Ordering::Greater)
        })
    };
    if fits(&zero) {
        return Some(empty);
    }
    // A `sum` is built, not searched for: the total is split over as few elements as the
    // field's own range allows. Reaching `合計 = 3000円` by adding 1円 three thousand times
    // is not a search, it is a way of not finding it.
    let sums: Vec<(usize, String)> = walks
        .iter()
        .filter_map(|&i| {
            f.items.iter().find_map(|it| match it {
                Item::Agg(a) if a.name.text == axes[i].col && a.kind == AggKind::Sum => Some((i, a.column.text.clone())),
                _ => None,
            })
        })
        .collect();
    if sums.len() == walks.len() && !sums.is_empty() {
        let mut built: Vec<BTreeMap<String, Val>> = Vec::new();
        let mut room = 0usize;
        for (i, field) in &sums {
            let (lo, _) = ival(&axes[*i].coords[cell[*i]], axes[*i].step);
            let (_, hi) = ival(&axes[*i].coords[cell[*i]], axes[*i].step);
            let target = match (lo, hi) {
                (Some(l), Some(h)) => l.add(h).div(Rat::int(2)),
                (Some(l), None) => l,
                (None, Some(h)) => h,
                (None, None) => Rat::zero(),
            };
            let target = Rat::int(target.num.div_euclid(target.den));
            let cap_each = c.ranges.get(field).and_then(|(_, h)| *h).unwrap_or(Rat::int(1));
            if cap_each.cmp_to(Rat::zero()) != Ordering::Greater {
                built.clear();
                break;
            }
            let n = {
                let q = target.div(cap_each);
                let k = q.num.div_euclid(q.den) + i128::from(q.num.rem_euclid(q.den) != 0);
                k.max(if target.cmp_to(Rat::zero()) == Ordering::Greater { 1 } else { 0 })
            };
            if n > 64 {
                built.clear();
                break;
            }
            let mut left = target;
            for k in 0..n {
                let take = if k + 1 == n { left } else { cap_each };
                left = left.sub(take);
                let mut e = BTreeMap::new();
                e.insert(field.clone(), Val::Num(take));
                if room + (k as usize) < built.len() {
                    built[room + k as usize].insert(field.clone(), Val::Num(take));
                } else {
                    built.push(e);
                }
            }
            room = built.len();
        }
        if !built.is_empty() {
            // Fields the sums did not name still have to be there.
            if let Some(el) = &f.elements {
                for e in &mut built {
                    for fld in &el.fields {
                        if !e.contains_key(&fld.name.text) {
                            if let Some(v) = menu.first().and_then(|m| m.get(&fld.name.text)).cloned() {
                                e.insert(fld.name.text.clone(), v);
                            }
                        }
                    }
                }
            }
            let mut m = scalars.clone();
            m.insert(over.clone(), Val::Seq(built));
            if fits(&run(f, c, m.clone())) {
                return Some(m);
            }
        }
    }
    // How many copies of one element to try before giving up on it. A `count` is small by
    // construction and a `sum` is built outright above, so this only bounds the fallback.
    let cap = 64;
    for e in menu {
        let w = weigh(e);
        if w.iter().all(|r| r.cmp_to(Rat::zero()) == Ordering::Equal) {
            continue;
        }
        let mut seq: Vec<BTreeMap<String, Val>> = Vec::new();
        for _ in 0..cap {
            seq.push(e.clone());
            let mut m = scalars.clone();
            m.insert(over.clone(), Val::Seq(seq.clone()));
            let a = run(f, c, m.clone());
            if fits(&a) {
                return Some(m);
            }
            // Past every target and still climbing: this element cannot land it.
            if want.iter().all(|(i, _, hi)| hi.is_some_and(|h| at(&a, *i).cmp_to(h) == Ordering::Greater)) {
                break;
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------------------
// Settling one cell
// ---------------------------------------------------------------------------------------

/// A literal as its **value**, not as its spelling. `1800円` and `1_800円` are the same
/// amount written two ways, and a check that called them different would report a region
/// where a formatting change had been made and nothing else.
fn lit_key(l: &Lit) -> String {
    match l {
        Lit::Num(n) => {
            let digits = n.digits.trim_start_matches('0');
            let frac = n.frac.trim_end_matches('0');
            format!(
                "n:{}{}.{}*{}{}",
                if n.neg { "-" } else { "" },
                if digits.is_empty() { "0" } else { digits },
                frac,
                n.mult,
                n.unit.clone().unwrap_or_default()
            )
        }
        Lit::Word(w) => format!("w:{w}"),
        Lit::Str(x) => format!("s:{x}"),
        Lit::Date(y, m, d) => format!("d:{y:04}-{m:02}-{d:02}"),
    }
}

fn same_expr(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Name(x, _), Expr::Name(y, _)) => x == y,
        (Expr::Lit(x, _), Expr::Lit(y, _)) => lit_key(x) == lit_key(y),
        (Expr::Bin(l1, o1, r1, _), Expr::Bin(l2, o2, r2, _)) => o1 == o2 && same_expr(l1, l2) && same_expr(r1, r2),
        (Expr::Call(n1, a1, _), Expr::Call(n2, a2, _)) => {
            n1 == n2 && a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| same_expr(x, y))
        }
        _ => false,
    }
}

/// Whether everything the answer is computed *with* — as opposed to the rows it is computed
/// *from* — is written the same way in both versions. The rounding counts: it is the last
/// thing that touches an output (§7.2).
fn same_machinery(o: (&RuleFile, &Checked), n: (&RuleFile, &Checked)) -> bool {
    let exprs = |f: &RuleFile| -> Vec<(String, Expr)> {
        let mut v: Vec<(String, Expr)> = f
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Derived(d) => Some((d.name.text.clone(), d.expr.clone())),
                Item::Define(d) => Some((d.name.text.clone(), d.expr.clone())),
                _ => None,
            })
            .collect();
        if let Some(r) = &f.result {
            v.push((r.name.clone(), r.expr.clone()));
        }
        v
    };
    let (a, b) = (exprs(o.0), exprs(n.0));
    if a.len() != b.len() || !a.iter().zip(&b).all(|((x, ex), (y, ey))| x == y && same_expr(ex, ey)) {
        return false;
    }
    if o.0.outputs.len() != n.0.outputs.len() {
        return false;
    }
    o.0.outputs.iter().zip(&n.0.outputs).all(|(x, y)| {
        x.name.text == y.name.text
            && o.1.roundings.get(&x.name.text).map(|(m, g)| (m.name(), g.num, g.den))
                == n.1.roundings.get(&y.name.text).map(|(m, g)| (m.name(), g.num, g.den))
    })
}

/// Every row of every definition set, by the name the trace calls it: `(table, row number)`.
/// The answers put the same names in `rows`, so the two versions' rows meet here.
/// A row as the two versions can be compared on it: the columns it writes and what it
/// writes into each, keyed by the name a trace calls the row.
type RowMap = HashMap<(String, usize), (Vec<String>, Vec<String>)>;

/// One cell as what it tests, not as how it is spelled. A set and a conjunction of
/// comparisons are unordered, so they are sorted: swapping `>=1kg <=5kg` for `<=5kg >=1kg`
/// tests the same thing.
fn cell_key(cell: &Cell) -> String {
    let many = |ls: &[Lit], tag: &str| {
        let mut v: Vec<String> = ls.iter().map(lit_key).collect();
        v.sort();
        format!("{tag}:{}", v.join("|"))
    };
    match cell {
        Cell::DontCare => "-".into(),
        Cell::Nothing => "none".into(),
        Cell::Lit(l) => format!("={}", lit_key(l)),
        Cell::Set(ls) => many(ls, "in"),
        Cell::Not(ls) => many(ls, "not"),
        Cell::Prefix(ps) => {
            let mut v = ps.to_vec();
            v.sort();
            format!("pre:{}", v.join("|"))
        }
        Cell::Cmp(cs) => {
            let mut v: Vec<String> = cs.iter().map(|(op, l)| format!("{}{}", op.word(), lit_key(l))).collect();
            v.sort();
            format!("cmp:{}", v.join("|"))
        }
    }
}

fn row_map(c: &Checked) -> RowMap {
    let mut m = HashMap::new();
    for set in &c.sets {
        let cols: Vec<String> = set.table.outputs.iter().map(|o| o.name.text.clone()).collect();
        for (i, r) in set.table.rows.iter().enumerate() {
            let outs: Vec<String> = r
                .outs
                .iter()
                .map(|o| match o {
                    OutCell::Name(n) => format!("={n}"),
                    OutCell::Lit(l) => lit_key(l),
                })
                .collect();
            // **What the row tests, not only what it answers.** Moving a threshold leaves
            // every output cell where it was, so a fingerprint made of answers alone says
            // "nothing changed" about a rule that now charges a different amount.
            let mut tests: Vec<String> = set
                .table
                .inputs
                .iter()
                .enumerate()
                .map(|(ci, (col, _))| format!("{col}{}", r.cells.get(ci).map(cell_key).unwrap_or_default()))
                .collect();
            tests.push(format!("policy:{}", if matches!(set.policy_of(i), Policy::TopDown) { "first" } else { "unique" }));
            let mut beats: Vec<String> = set.beats[i].iter().map(|b| b.to_string()).collect();
            beats.sort();
            tests.push(format!("beats:{}", beats.join(",")));
            m.insert((set.row_table(i).to_string(), r.index), (cols.clone(), [outs, tests].concat()));
        }
    }
    m
}

/// Everything outside the rows that decides an answer: which values a group stands for, and
/// which combinations the caller promises never to send. Neither is written in a row, and
/// both change what the rows mean.
fn around_rows(f: &RuleFile, c: &Checked) -> Vec<String> {
    let mut out: Vec<String> = c
        .groups
        .iter()
        .map(|(g, (en, ms))| {
            let mut v = ms.clone();
            v.sort();
            format!("group {g}:{en}={}", v.join("|"))
        })
        .collect();
    out.extend(
        f.constraints
            .iter()
            .map(|k| format!("constraint {} {} {}", k.left, k.op.word(), k.right)),
    );
    out.sort();
    out
}

/// Whether the two versions ran the identical computation here: the same rows, writing the
/// same answers, over the same expressions. Then they agree on the **whole** cell and not
/// only at the point they were compared on — the coordinates are cut at every boundary
/// either version names, so no row selection changes inside a cell.
fn identical_run(ao: &Answer, an: &Answer, om: &RowMap, nm: &RowMap, machinery: bool) -> bool {
    machinery
        && ao.rows == an.rows
        && ao.rows.iter().all(|k| {
            let key = (k.0.clone(), k.1);
            match (om.get(&key), nm.get(&key)) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            }
        })
}

/// Whether every output is one value over the whole cell. A column with an open coordinate
/// takes more than one, so an answer that reads it is not settled by comparing one point;
/// one that does not is.
pub(crate) fn constant_here(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], ans: &Answer) -> bool {
    let names: Vec<String> = f.outputs.iter().map(|o| o.name.text.clone()).collect();
    names_constant(f, c, axes, cell, &ans.rows, &names)
}

/// Whether each of `names` is one value over the whole cell, given the rows that fired there
/// (§15.148 asks this of the carried output and of the outputs a `once` line counts).
pub(crate) fn names_constant(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], rows: &[(String, usize)], names: &[String]) -> bool {
    let open: BTreeSet<&str> = axes
        .iter()
        .enumerate()
        .filter(|(i, a)| matches!(a.coords[cell[*i]], Coord::Open(_, _)))
        .map(|(_, a)| a.col.as_str())
        .collect();
    let fired: HashMap<&str, usize> = rows.iter().map(|(t, r)| (t.as_str(), *r)).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    fn walk_expr(e: &Expr, f: &RuleFile, c: &Checked, open: &BTreeSet<&str>, fired: &HashMap<&str, usize>, seen: &mut BTreeSet<String>) -> bool {
        match e {
            Expr::Lit(_, _) => true,
            Expr::Name(n, _) => walk_name(n, f, c, open, fired, seen),
            Expr::Bin(l, _, r, _) => {
                walk_expr(l, f, c, open, fired, seen) && walk_expr(r, f, c, open, fired, seen)
            }
            Expr::Call(_, args, _) => args.iter().all(|a| walk_expr(a, f, c, open, fired, seen)),
        }
    }
    fn walk_name(n: &str, f: &RuleFile, c: &Checked, open: &BTreeSet<&str>, fired: &HashMap<&str, usize>, seen: &mut BTreeSet<String>) -> bool {
        if !seen.insert(n.to_string()) {
            return true;
        }
        if open.contains(n) {
            return false;
        }
        for it in &f.items {
            match it {
                Item::Derived(d) if d.name.text == n => return walk_expr(&d.expr, f, c, open, fired, seen),
                Item::Define(d) if d.name.text == n => return walk_expr(&d.expr, f, c, open, fired, seen),
                _ => {}
            }
        }
        // A column a table decides: constant when the row that fired writes a literal.
        for set in &c.sets {
            let Some(ci) = set.table.outputs.iter().position(|o| o.name.text == n) else { continue };
            for (i, r) in set.table.rows.iter().enumerate() {
                let t = set.row_table(i);
                if fired.get(t) == Some(&r.index) {
                    return match r.outs.get(ci) {
                        Some(OutCell::Lit(_)) => true,
                        Some(OutCell::Name(m)) => walk_name(m, f, c, open, fired, seen),
                        None => false,
                    };
                }
            }
        }
        // An input with a point coordinate reached here only if it is not open.
        true
    }
    names.iter().all(|n| {
        let mut s = seen.clone();
        let r = match &f.result {
            Some(rd) if rd.name == *n => walk_expr(&rd.expr, f, c, &open, &fired, &mut s),
            _ => walk_name(n, f, c, &open, &fired, &mut s),
        };
        seen.extend(s);
        r
    })
}

// ---------------------------------------------------------------------------------------
// What the rule accepts, as opposed to what it answers
// ---------------------------------------------------------------------------------------

fn show_range(c: &Checked, n: &str) -> String {
    match c.ranges.get(n) {
        Some((lo, hi)) => format!(
            "{}..{}",
            lo.map(|x| x.to_string()).unwrap_or_default(),
            hi.map(|x| x.to_string()).unwrap_or_default()
        ),
        None => String::new(),
    }
}

fn domain_diff(o: (&RuleFile, &Checked), n: (&RuleFile, &Checked)) -> Vec<DomainChange> {
    let mut out = Vec::new();
    let oi: Vec<&VarDecl> = o.0.inputs.iter().collect();
    let ni: Vec<&VarDecl> = n.0.inputs.iter().collect();
    for a in &oi {
        if !ni.iter().any(|b| b.name.text == a.name.text) {
            out.push(DomainChange { what: "input_removed", name: a.name.text.clone(), old: None, new: None });
        }
    }
    for b in &ni {
        match oi.iter().find(|a| a.name.text == b.name.text) {
            None => out.push(DomainChange { what: "input_added", name: b.name.text.clone(), old: None, new: None }),
            Some(a) => {
                let (ta, tb) = (o.1.ty_of(&a.name.text), n.1.ty_of(&b.name.text));
                if ta.as_ref().map(|x| x.to_string()) != tb.as_ref().map(|x| x.to_string()) {
                    out.push(DomainChange {
                        what: "input_type",
                        name: b.name.text.clone(),
                        old: ta.map(|x| x.to_string()),
                        new: tb.map(|x| x.to_string()),
                    });
                }
                let (ra, rb) = (show_range(o.1, &a.name.text), show_range(n.1, &b.name.text));
                if ra != rb {
                    out.push(DomainChange { what: "input_range", name: b.name.text.clone(), old: Some(ra), new: Some(rb) });
                }
            }
        }
    }
    let mut names: Vec<&String> = o.1.enums.keys().chain(n.1.enums.keys()).collect();
    names.sort();
    names.dedup();
    for en in names {
        let (a, b) = (o.1.enums.get(en), n.1.enums.get(en));
        match (a, b) {
            (Some(a), Some(b)) => {
                for v in a {
                    if !b.contains(v) {
                        out.push(DomainChange { what: "enum_value_removed", name: format!("{en}.{v}"), old: None, new: None });
                    }
                }
                for v in b {
                    if !a.contains(v) {
                        out.push(DomainChange { what: "enum_value_added", name: format!("{en}.{v}"), old: None, new: None });
                    }
                }
            }
            (Some(_), None) => out.push(DomainChange { what: "enum_removed", name: en.clone(), old: None, new: None }),
            (None, Some(_)) => out.push(DomainChange { what: "enum_added", name: en.clone(), old: None, new: None }),
            (None, None) => {}
        }
    }
    for a in &o.0.outputs {
        if !n.0.outputs.iter().any(|b| b.name.text == a.name.text) {
            out.push(DomainChange { what: "output_removed", name: a.name.text.clone(), old: None, new: None });
        }
    }
    for b in &n.0.outputs {
        match o.0.outputs.iter().find(|a| a.name.text == b.name.text) {
            None => out.push(DomainChange { what: "output_added", name: b.name.text.clone(), old: None, new: None }),
            Some(_) => {
                let r = |c: &Checked| c.roundings.get(&b.name.text).map(|(m, g)| format!("{} {g}", m.name()));
                let (ra, rb) = (r(o.1), r(n.1));
                if ra != rb {
                    out.push(DomainChange { what: "output_rounding", name: b.name.text.clone(), old: ra, new: rb });
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------------------

fn wire_of(c: &Checked, name: &str, v: &Option<Val>) -> String {
    crate::report::wire(c, name, v.as_ref()).unwrap_or_else(|| "-".into())
}

fn rows_text(rows: &[(String, usize)]) -> String {
    rows.iter().map(|(t, r)| tr!("表 {} 行{}", "table {} row {}", t, r)).collect::<Vec<_>>().join(", ")
}

/// The whole answer: what the two versions do differently, over the whole input space, and,
/// when both are the step of a state machine, over sequences of calls (§15.148).
pub fn diff(o: (&RuleFile, &Checked), n: (&RuleFile, &Checked), budget: usize) -> VDiff {
    let mut d = diff_cells(o, n, budget);
    d.machine = machine_diff(o, n, &d, budget);
    d
}

/// The machine's half of a diff (§15.148). `None` unless both versions carry the same input
/// back as the same output.
/// The values of the `held` inputs one world's calls are made with (§15.149).
fn world_values(a: &crate::machine::Analysis, w: &[usize]) -> Option<BTreeMap<String, Val>> {
    let (_, e) = a.edges.iter().enumerate().find(|(i, e)| e.world.as_slice() == w && !a.mixed.contains(i))?;
    Some(a.held.iter().filter_map(|n| e.inputs.get(n).map(|v| (n.clone(), v.clone()))).collect())
}

fn machine_diff(o: (&RuleFile, &Checked), n: (&RuleFile, &Checked), d: &VDiff, budget: usize) -> Option<MachineDiff> {
    let (om, nm) = (o.0.machine.as_ref()?, n.0.machine.as_ref()?);
    let (ocarry, ncarry) = (om.carried()?, nm.carried()?);
    if ocarry != ncarry {
        return None;
    }
    let (cin, cout) = ocarry;
    let nodes = budget.saturating_mul(crate::machine::NODES_PER_CELL);
    let oa = crate::machine::analyze(o.0, o.1, nodes)?;
    let na = crate::machine::analyze(n.0, n.1, nodes)?;
    let mut md = MachineDiff {
        carry: (cin.to_string(), cout.to_string()),
        initial: (oa.initial.clone(), na.initial.clone()),
        shortest: Vec::new(),
        unreached: 0,
        migration: Vec::new(),
    };
    // The calls that part the two, shortest first. Before the first difference both run the
    // same calls to the same states, so a path the old version takes to a state is one the new
    // version takes too — up to the call where they part.
    if oa.blocked.is_none() && !oa.over_budget && oa.initial == na.initial {
        let carry_ax = d.axes.iter().position(|a| a.col == cin);
        let mut best: Option<Vec<MStep>> = None;
        for ch in &d.changes {
            let states: Vec<String> = match carry_ax {
                Some(ci) => ch.region[ci]
                    .iter()
                    .filter_map(|&k| match &d.axes[ci].coords[k] {
                        Coord::Word(w) => Some(w.clone()),
                        _ => None,
                    })
                    .collect(),
                None => Vec::new(),
            };
            let reached: Vec<&String> = states.iter().filter(|st| oa.reachable.contains(st)).collect();
            if reached.is_empty() {
                md.unreached += 1;
                continue;
            }
            // One case holds its `held` inputs (§15.149), so the path runs in one world and
            // the call where the two part is made with that world's values.
            for node in oa.nodes.iter().filter(|nd| reached.contains(&&nd.0)) {
                let st = &node.0;
                let Some(path) = oa.trace_to_node(node) else { continue };
                if path.iter().any(|e| oa.mixed.contains(e)) {
                    continue;
                }
                if best.as_ref().is_some_and(|b| b.len() <= path.len() + 1) {
                    continue;
                }
                let mut last = ch.witness.clone();
                last.insert(cin.to_string(), Val::Enum(st.clone()));
                if let Some(vals) = world_values(&oa, &node.1) {
                    last.extend(vals);
                }
                let env: HashMap<String, Val> = last.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                let (ao, an) = (run(o.0, o.1, env.clone()), run(n.0, n.1, env));
                let wire = |f: &Checked, outs: &[(String, Option<Val>)]| -> Vec<String> { outs.iter().map(|(k, v)| wire_of(f, k, v)).collect() };
                if wire(o.1, &ao.outs) == wire(n.1, &an.outs) {
                    continue;
                }
                let mut steps: Vec<MStep> = path
                    .iter()
                    .map(|&e| {
                        let edge = &oa.edges[e];
                        MStep { inputs: edge.inputs.clone(), old: edge.outputs.clone(), new: edge.outputs.clone() }
                    })
                    .collect();
                steps.push(MStep { inputs: last, old: ao.outs, new: an.outs });
                best = Some(steps);
            }
        }
        md.shortest = best.unwrap_or_default();
    }
    // Cases in progress: where the old version can take a case, and what the new one does
    // with a case that is already there — in the world the case is in, which the new version
    // reads off the case's `held` inputs (§15.149).
    let can_finish = na.can_finish();
    let new_worlds: BTreeSet<Vec<usize>> = na.nodes.iter().map(|nd| nd.1.clone()).chain(na.edges.iter().map(|e| e.world.clone())).collect();
    let stranded = |st: &String| -> bool {
        oa.nodes.iter().filter(|nd| &nd.0 == st).any(|nd| {
            let vals = world_values(&oa, &nd.1).unwrap_or_default();
            match na.world_of(|n| vals.get(n)) {
                Some(w) => !can_finish.contains(&(st.clone(), w)),
                None => new_worlds.iter().any(|w| !can_finish.contains(&(st.clone(), w.clone()))),
            }
        })
    };
    for st in &oa.reachable {
        let kind = if !na.states.contains(st) {
            "removed"
        } else if !na.finals.is_empty() && stranded(st) && na.blocked.is_none() && !na.over_budget && !na.unknown.contains_key(st) {
            "stranded"
        } else if oa.finals.contains(st) && !na.finals.contains(st) {
            "no_longer_final"
        } else if !oa.finals.contains(st) && na.finals.contains(st) {
            "now_final"
        } else {
            continue;
        };
        md.migration.push((st.clone(), kind));
    }
    let _ = cout;
    Some(md)
}

/// What a migration kind says, in prose.
fn migration_text(state: &str, kind: &str) -> String {
    match kind {
        "removed" => tr!(
            "{state}: 新しい版にこの状態はありません。この状態にいる案件は、新しい版の入口で断られます",
            "{state}: the new version has no such state; a case in it is refused at the door"
        ),
        "stranded" => tr!(
            "{state}: 新しい版では、この状態から終わりの状態に着けません。この状態にいる案件は終われなくなります",
            "{state}: under the new version no final state can be reached from here; a case in it can no longer finish"
        ),
        "no_longer_final" => tr!(
            "{state}: 古い版では終わりの状態でしたが、新しい版ではそうではありません。終わっていた案件がまた動きえます",
            "{state}: final under the old version and not under the new; a case that had ended can move again"
        ),
        "now_final" => tr!(
            "{state}: 新しい版では終わりの状態です。この状態にいる案件は、そこで終わります",
            "{state}: final under the new version; a case in it ends there"
        ),
        other => format!("{state}: {other}"),
    }
}

/// One call of a machine diff's trace, as a line.
fn mstep_text(c: &Checked, st: &MStep, cin: &str, cout: &str, k: usize) -> String {
    let from = st.inputs.get(cin).map(crate::vectors::show).unwrap_or_default();
    let human = |n: &str, v: &Val| v.show(&c.ty_of(n).unwrap_or(Ty::Unknown));
    let args: Vec<String> = st
        .inputs
        .iter()
        .filter(|(n, _)| n.as_str() != cin)
        .map(|(n, v)| format!("{n} = {}", human(n, v)))
        .collect();
    let show = |outs: &[(String, Option<Val>)]| -> String {
        outs.iter()
            .map(|(n, v)| format!("{n} = {}", v.as_ref().map(|v| human(n, v)).unwrap_or_else(|| "-".into())))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let (o, n) = (show(&st.old), show(&st.new));
    let _ = cout;
    if o == n {
        tr!("  {}. {} のとき {} → {}", "  {}. at {}, {} → {}", k, from, args.join(", "), o)
    } else {
        tr!("  {}. {} のとき {} → 古い版 {} / 新しい版 {}", "  {}. at {}, {} → old: {} / new: {}", k, from, args.join(", "), o, n)
    }
}

fn diff_cells(o: (&RuleFile, &Checked), n: (&RuleFile, &Checked), budget: usize) -> VDiff {
    let mut out = VDiff {
        rule: n.0.name.text.clone(),
        old_version: o.0.version.clone(),
        new_version: n.0.version.clone(),
        axes: Vec::new(),
        domain: domain_diff(o, n),
        changes: Vec::new(),
        unknown: Vec::new(),
        cells: 0,
        feasible: 0,
        same: 0,
        differing: 0,
        unsettled: 0,
        unrealized: 0,
        over_budget: false,
        blocked: None,
        machine: None,
    };
    // A rule that folds a sequence is not a function of finitely many columns: the answer
    // depends on the whole sequence, and a region over the summaries would not say what it
    // sounds like it says. What can be settled is whether anything inside the walk changed.
    if o.0.fold.is_some() || n.0.fold.is_some() {
        if decides_alike(o, n) {
            out.same = 1;
            out.cells = 1;
            out.feasible = 1;
        } else {
            out.blocked = Some(tr!(
                "この規則は並び全体を見て答えを出すので、入力を決まった数の項目の組み合わせに分けられません。変わったのは並びを見ていく側なので、どの入力で答えが変わるかはここでは言えません",
                "this rule folds a sequence: the answer depends on the whole of it, so the space of columns does not cut into cells. What changed is inside the walk, and which inputs move cannot be stated here"
            ));
        }
        return out;
    }

    // Nothing that decides an answer changed: the same rows, writing the same values, over
    // the same expressions and the same rounding. Then the two are the same function on
    // every input there is, and walking the space would only spend time confirming it.
    // This is the same argument `identical_run` makes about one cell, made about all of
    // them at once, and it is what keeps `--diff-base`-shaped use cheap.
    if decides_alike(o, n) && out.domain.is_empty() {
        out.cells = 1;
        out.feasible = 1;
        out.same = 1;
        return out;
    }
    // Two versions that do not take the same inputs cannot be compared point by point:
    // there is no input to give them both. What changed is in `domain`, which is the honest
    // answer, and no region is invented on top of it.
    let (oi, ni): (BTreeSet<&str>, BTreeSet<&str>) =
        (o.0.inputs.iter().map(|i| i.name.text.as_str()).collect(), n.0.inputs.iter().map(|i| i.name.text.as_str()).collect());
    if oi != ni {
        out.blocked = Some(tr!(
            "二つの版が取る入力が違うので、同じ入力を両方に渡せません。何が変わったかは上のとおりです",
            "the two versions do not take the same inputs, so there is no input to give them both; what changed is listed above"
        ));
        return out;
    }
    let cols = columns(n.0, o.0);
    let mut axes: Vec<Axis> = Vec::new();
    for (col, kind) in &cols {
        let Some(ty) = n.1.ty_of(col).or_else(|| o.1.ty_of(col)) else {
            // Skipping the column would shrink the space the claim is about without
            // saying so, which is the one thing this command must not do.
            out.blocked = Some(tr!("列 {} の型が分かりません", "the type of column {} is not known", col));
            return out;
        };
        let Some(ax) = axis_for(col, *kind, &ty, &[(o.0, o.1), (n.0, n.1)]) else {
            out.blocked = Some(tr!(
                "列 {} の型 {} では、値を区切って比べられません",
                "column {} has type {}, which does not cut into coordinates",
                col,
                ty
            ));
            return out;
        };
        if ax.coords.is_empty() {
            // A column with no coordinate would be a column quietly left out of the walk,
            // and the claim would then be about a smaller space than it says (§15.99).
            out.blocked = Some(tr!("列 {} の値を区切れません", "column {} does not cut into coordinates", col));
            return out;
        }
        axes.push(ax);
    }
    // Walk the wide axes last: a box grows over them, and the merge is cheaper that way.
    axes.sort_by_key(|a| a.len());
    let total: u128 = axes.iter().map(|a| a.len() as u128).product::<u128>().max(1);
    out.cells = total.min(u128::from(u64::MAX)) as usize;
    if total > budget as u128 {
        out.over_budget = true;
        out.axes = axes;
        return out;
    }
    out.axes = axes.clone();

    let (om, nm) = (row_map(o.1), row_map(n.1));
    let machinery = same_machinery(o, n);
    let menu = element_menu(n.0, n.1, 24);
    let input_names: BTreeSet<String> = o.0.inputs.iter().map(|i| i.name.text.clone()).collect();
    let scalars = scalar_derives(o.0, &input_names);
    let cone = cone_of(&axes, o.0, o.1, &scalars);
    let mut memo: HashMap<Vec<usize>, Solved> = HashMap::new();
    let mut shape_memo: HashMap<Vec<usize>, bool> = HashMap::new();

    let n_cells = total as usize;
    let mut strides = vec![1usize; axes.len()];
    for a in (0..axes.len().saturating_sub(1)).rev() {
        strides[a] = strides[a + 1] * axes[a + 1].len();
    }
    let mut reached = vec![false; n_cells];
    let mut considered = vec![false; n_cells];
    let mut diffs: Vec<Diffed> = Vec::new();
    let mut unsure: Vec<usize> = Vec::new();
    let mut unbuilt: Vec<usize> = Vec::new();
    let mut cell = vec![0usize; axes.len()];
    let mut at = 0usize;
    loop {
        if let Some((inputs, ao)) = realize(&axes, &cell, o.0, o.1, &menu, &scalars, &cone, &mut memo, None) {
            out.feasible += 1;
            reached[at] = true;
            considered[at] = true;
            let an = run(n.0, n.1, inputs.clone());
            let ow: Vec<(String, String, String)> = ao
                .outs
                .iter()
                .map(|(k, v)| {
                    let nv = an.outs.iter().find(|(k2, _)| k2 == k).and_then(|(_, v)| v.clone());
                    (k.clone(), wire_of(o.1, k, v), wire_of(n.1, k, &nv))
                })
                .collect();
            let moved = ow.iter().any(|(_, a, b)| a != b) || ao.outs.len() != an.outs.len();
            if moved {
                out.differing += 1;
                diffs.push((
                    at,
                    ao.rows.clone(),
                    an.rows.clone(),
                    ow,
                    inputs.into_iter().collect(),
                ));
            } else if identical_run(&ao, &an, &om, &nm, machinery)
                || (constant_here(o.0, o.1, &axes, &cell, &ao) && constant_here(n.0, n.1, &axes, &cell, &an))
            {
                out.same += 1;
            } else {
                // The two agreed where they were asked. Ask again, elsewhere in the same
                // cell — but only at points the cell actually holds, so every computed
                // column is held to its coordinate again before the answers are compared.
                let mut parted = None;
                for seed in other_points(&axes, &cell, &inputs, 6) {
                    // Moving one input can take a derived column out of the cell (raising
                    // `商品合計` moves `支払額` with it), so the rest is solved again around
                    // the seed rather than left where it was.
                    let Some((alt, ao2)) =
                        realize(&axes, &cell, o.0, o.1, &menu, &scalars, &cone, &mut memo, Some(&seed))
                    else {
                        continue;
                    };
                    let an2 = run(n.0, n.1, alt.clone());
                    let ow2: Vec<(String, String, String)> = ao2
                        .outs
                        .iter()
                        .map(|(k, v)| {
                            let nv = an2.outs.iter().find(|(k2, _)| k2 == k).and_then(|(_, v)| v.clone());
                            (k.clone(), wire_of(o.1, k, v), wire_of(n.1, k, &nv))
                        })
                        .collect();
                    if ow2.iter().any(|(_, a, b)| a != b) {
                        parted = Some((alt, ao2, an2, ow2));
                        break;
                    }
                    let _ = &ao2;
                }
                match parted {
                    Some((alt, ao2, an2, ow2)) => {
                        out.differing += 1;
                        diffs.push((at, ao2.rows.clone(), an2.rows.clone(), ow2, alt.into_iter().collect()));
                    }
                    None => {
                        out.unsettled += 1;
                        unsure.push(at);
                    }
                }
            }
        } else if {
            let k: Vec<usize> = cone.iter().map(|&i| cell[i]).collect();
            match shape_memo.get(&k) {
                Some(v) => *v,
                None => {
                    let v = feasible_shape(&axes, &cell, o.0, o.1, &scalars);
                    shape_memo.insert(k, v);
                    v
                }
            }
        } {
            // The coordinates do not contradict each other, yet no input was built for them.
            // Before calling that unknown, ask linear arithmetic whether the cell holds any
            // input at all — it is the same question W114 asks about a pair of rows, over
            // the same system, and the columns of a cell are the conditions (§15.129). A
            // cell that neither version can reach is nobody's case, so it takes nothing
            // away from "elsewhere the two answer alike".
            // Proved empty: the cell is not a case anybody can send, so it is passed over
            // exactly as one the shape ruled out is — counted nowhere and claimed nothing
            // about.
            if !(linearly_empty(&axes, &cell, o) && linearly_empty(&axes, &cell, n)) {
                out.unrealized += 1;
                considered[at] = true;
                unbuilt.push(at);
            }
        }
        // odometer
        let mut i = axes.len();
        loop {
            if i == 0 {
                out.changes = cluster(&diffs, &reached, &axes, &strides);
                let all = vec![true; n_cells];
                out.unknown = merge(&unsure, &all, &axes, &strides)
                    .into_iter()
                    .map(|r| {
                        (r, tr!("試した入力では同じ答えでしたが、この範囲のどの入力でも同じだとまでは示せませんでした", "the answers agreed, but not provably over the whole cell"))
                    })
                    .chain(merge(&unbuilt, &all, &axes, &strides).into_iter().map(|r| {
                        (
                            r,
                            tr!(
                                "この組み合わせに当てはまる入力を作れませんでした。起こらないとも示せていません（導出どうしが入力を共有していると、ここが残ります）",
                                "no input was built for these coordinates, and they were not shown to be impossible either (the blind spot of the sieve in §6.2)"
                            ),
                        )
                    }))
                    .collect();
                return out;
            }
            i -= 1;
            cell[i] += 1;
            at += strides[i];
            if cell[i] < axes[i].len() {
                break;
            }
            at -= strides[i] * axes[i].len();
            cell[i] = 0;
        }
    }
}

/// Whether two arms of a walk do the same thing. The name of the verdict is not enough:
/// `打ち切り -> stop with 0円` and `stop with 100円` are the same arm with a different
/// answer in it.
fn same_arm(a: &Arm, b: &Arm) -> bool {
    match (a, b) {
        (Arm::Next, Arm::Next) => true,
        (Arm::Stop(x), Arm::Stop(y)) => match (x, y) {
            (Some(p), Some(q)) => same_expr(p, q),
            (None, None) => true,
            _ => false,
        },
        (Arm::Take { expr: p, unique: u }, Arm::Take { expr: q, unique: v }) => u == v && same_expr(p, q),
        (Arm::KeepMax { expr: p, key: k1 }, Arm::KeepMax { expr: q, key: k2 }) => same_expr(p, q) && same_expr(k1, k2),
        _ => false,
    }
}

/// Whether **nothing that decides an answer** differs: the rows (what each tests and what
/// it answers, in what order and under which policy), the expressions and rounding above
/// them, the walk, the groups and the constraints. Then the two versions are the same
/// function on every input, and the space need not be walked at all.
///
/// Getting this list wrong is the one way this command can lie outright, so it is written
/// as one place rather than spelled out at each call.
fn decides_alike(o: (&RuleFile, &Checked), n: (&RuleFile, &Checked)) -> bool {
    same_machinery(o, n)
        && fold_same(o.0, n.0)
        && row_map(o.1) == row_map(n.1)
        && around_rows(o.0, o.1) == around_rows(n.0, n.1)
}

/// Whether the two walks reduce the same way.
fn fold_same(a: &RuleFile, b: &RuleFile) -> bool {
    match (&a.fold, &b.fold) {
        (Some(x), Some(y)) => {
            x.verdict == y.verdict
                && x.over == y.over
                && x.arms.len() == y.arms.len()
                && x.arms.iter().zip(&y.arms).all(|((n1, a1, _), (n2, a2, _))| n1.text == n2.text && same_arm(a1, a2))
                && match (&x.empty, &y.empty) {
                    (Some(p), Some(q)) => same_expr(p, q),
                    (None, None) => true,
                    _ => false,
                }
                && match (&x.exhausted, &y.exhausted) {
                    (Some(p), Some(q)) => same_expr(p, q),
                    (None, None) => true,
                    _ => false,
                }
        }
        (None, None) => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------
// From cells back to boxes
// ---------------------------------------------------------------------------------------

/// Cover a set of cells with boxes. Greedy: take an uncovered cell, widen it on one axis at
/// a time for as long as the whole product stays inside the set, and emit it.
///
/// A cell **no input reaches** widens a box for free. The claim is about inputs a caller can
/// send, so a coordinate that only holds unreachable cells is a don't-care — and saying so
/// is the difference between `余裕 >=10m2 <15m2` and the same thing with two columns of
/// noise beside it, where the noise is only the shape of what cannot happen.
fn merge(cells: &[usize], reached: &[bool], axes: &[Axis], strides: &[usize]) -> Vec<Region> {
    if cells.is_empty() {
        return Vec::new();
    }
    let mut inside = vec![false; reached.len()];
    for &i in cells {
        inside[i] = true;
    }
    let ok = |i: usize| inside[i] || !reached[i];
    let mut left: BTreeSet<usize> = cells.iter().copied().collect();
    let mut out = Vec::new();
    while let Some(seed) = left.iter().next().copied() {
        let coords: Vec<usize> = (0..axes.len()).map(|a| (seed / strides[a]) % axes[a].len()).collect();
        let mut box_: Region = coords.iter().map(|&x| BTreeSet::from([x])).collect();
        let mut grown = true;
        while grown {
            grown = false;
            for ai in 0..axes.len() {
                for cand in 0..axes[ai].len() {
                    if box_[ai].contains(&cand) {
                        continue;
                    }
                    let lists: Vec<Vec<usize>> = box_
                        .iter()
                        .enumerate()
                        .map(|(i, s)| if i == ai { vec![cand] } else { s.iter().copied().collect() })
                        .collect();
                    if product(&lists, strides).all(ok) {
                        box_[ai].insert(cand);
                        grown = true;
                    }
                }
            }
        }
        // Growing over the cells no input reaches makes the box as wide as the claim allows;
        // it can also carry a coordinate that holds nothing but such cells. Those say
        // nothing and read as noise (`余裕 = -1999m2 または …`), so they come back off. Every
        // cell that actually differs is still inside, which is what the box is for.
        for ai in 0..axes.len() {
            let keep: BTreeSet<usize> = box_[ai]
                .iter()
                .copied()
                .filter(|&cand| {
                    let lists: Vec<Vec<usize>> = box_
                        .iter()
                        .enumerate()
                        .map(|(i, sset)| if i == ai { vec![cand] } else { sset.iter().copied().collect() })
                        .collect();
                    product(&lists, strides).any(|i| inside[i])
                })
                .collect();
            if !keep.is_empty() {
                box_[ai] = keep;
            }
        }
        // Trimming can leave a column looking like a condition when it is not one: dropping
        // the one coordinate of `床面積` that no input reaches turns a don't-care into
        // `>=2m2`. So every axis is offered the whole of itself back, and takes it when the
        // box stays sound. What is left saying something is what actually decides.
        let mut again = true;
        while again {
            again = false;
            for ai in 0..axes.len() {
                if box_[ai].len() == axes[ai].len() {
                    continue;
                }
                let lists: Vec<Vec<usize>> = box_
                    .iter()
                    .enumerate()
                    .map(|(i, sset)| if i == ai { (0..axes[ai].len()).collect() } else { sset.iter().copied().collect() })
                    .collect();
                if product(&lists, strides).all(ok) {
                    box_[ai] = (0..axes[ai].len()).collect();
                    again = true;
                }
            }
        }
        let lists: Vec<Vec<usize>> = box_.iter().map(|s| s.iter().copied().collect()).collect();
        for i in product(&lists, strides) {
            left.remove(&i);
        }
        out.push(box_);
    }
    out
}

/// Every linear cell index the box holds.
fn product(lists: &[Vec<usize>], strides: &[usize]) -> impl Iterator<Item = usize> + use<> {
    let lists = lists.to_vec();
    let strides = strides.to_vec();
    let total: usize = lists.iter().map(|l| l.len()).product();
    (0..total).map(move |mut k| {
        let mut idx = 0usize;
        for a in (0..lists.len()).rev() {
            let n = lists[a].len();
            idx += lists[a][k % n] * strides[a];
            k /= n;
        }
        idx
    })
}

type Diffed = (usize, Vec<(String, usize)>, Vec<(String, usize)>, Vec<(String, String, String)>, BTreeMap<String, Val>);

/// Group the cells that differ by the rows that fired, then cover each group with boxes.
/// The grouping is the one `rulec diff` already uses on records (§10.4), so the two answers
/// read alike.
fn cluster(diffs: &[Diffed], reached: &[bool], axes: &[Axis], strides: &[usize]) -> Vec<Change> {
    type Key = (Vec<(String, usize)>, Vec<(String, usize)>);
    let mut groups: Vec<(Key, Vec<&Diffed>)> = Vec::new();
    for d in diffs {
        let key = (d.1.clone(), d.2.clone());
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(d),
            None => groups.push((key, vec![d])),
        }
    }
    let mut out = Vec::new();
    for ((orows, nrows), items) in groups {
        let cells: Vec<usize> = items.iter().map(|d| d.0).collect();
        for region in merge(&cells, reached, axes, strides) {
            let lists: Vec<Vec<usize>> = region.iter().map(|s| s.iter().copied().collect()).collect();
            let held: BTreeSet<usize> = product(&lists, strides).collect();
            let mine: Vec<&&Diffed> = items.iter().filter(|d| held.contains(&d.0)).collect();
            let Some(pick) = mine.first() else { continue };
            // The transition is the one the example shows; where every cell of the box shows
            // the same one it is the box's own, and where it does not the amounts come out of
            // an expression rather than off this row.
            let uniform = mine.iter().all(|d| d.3 == pick.3);
            out.push(Change {
                region,
                outs: pick.3.clone(),
                uniform,
                old_rows: orows.clone(),
                new_rows: nrows.clone(),
                witness: pick.4.clone(),
                cells: mine.len(),
            });
        }
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.cells));
    out
}

// ---------------------------------------------------------------------------------------
// Writing it down
// ---------------------------------------------------------------------------------------

fn num_text(ax: &Axis, r: Rat) -> String {
    if ax.date {
        let (y, m, d) = crate::types::ord_to_date(r);
        return format!("{y:04}-{m:02}-{d:02}");
    }
    let v = if ax.shown == 100 { r.mul(Rat::int(100)) } else { r };
    format!("{}{}", v, ax.unit)
}

/// One axis of a box, written the way a cell is written. An axis that takes every
/// coordinate is a don't-care and says nothing.
fn show_axis(ax: &Axis, sel: &BTreeSet<usize>, c: &Checked) -> Option<String> {
    if sel.len() == ax.len() {
        return None;
    }
    let words: Vec<String> = sel
        .iter()
        .filter_map(|&i| match &ax.coords[i] {
            Coord::Word(w) => Some(w.clone()),
            Coord::OtherStr => Some(tr!("それ以外", "anything else")),
            _ => None,
        })
        .collect();
    if words.len() == sel.len() {
        // A set that is exactly a declared group reads better as the group's name.
        for (g, (en, members)) in &c.groups {
            if matches!(&ax.ty, Ty::Enum(e) if e == en) && members.len() == words.len() && members.iter().all(|m| words.contains(m)) {
                return Some(format!("{} = {}", ax.col, g));
            }
        }
        // More than half the axis: say what it is not.
        if words.len() * 2 > ax.len() {
            let rest: Vec<String> = (0..ax.len())
                .filter(|i| !sel.contains(i))
                .filter_map(|i| if let Coord::Word(w) = &ax.coords[i] { Some(w.clone()) } else { None })
                .collect();
            return Some(format!("{} {} {}", ax.col, crate::kw::NOT, rest.join(", ")));
        }
        return Some(format!("{} = {}", ax.col, words.join(" | ")));  // one column, one test
    }
    // Numeric: the selected coordinates as intervals, contiguous runs joined.
    let idx: Vec<usize> = sel.iter().copied().collect();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &i in &idx {
        match runs.last_mut() {
            Some((_, e)) if *e + 1 == i => *e = i,
            _ => runs.push((i, i)),
        }
    }
    let one = |a: usize, b: usize| -> String {
        let (lo, _) = ival(&ax.coords[a], ax.step);
        let (_, hi) = ival(&ax.coords[b], ax.step);
        match (lo, hi) {
            (Some(l), Some(h)) if l.cmp_to(h) == Ordering::Equal => format!("{} = {}", ax.col, num_text(ax, l)),
            (Some(l), Some(h)) => format!("{} >={} <={}", ax.col, num_text(ax, l), num_text(ax, h)),
            (Some(l), None) => format!("{} >={}", ax.col, num_text(ax, l)),
            (None, Some(h)) => format!("{} <={}", ax.col, num_text(ax, h)),
            (None, None) => format!("{} = -", ax.col),
        }
    };
    let text = runs.iter().map(|&(a, b)| one(a, b)).collect::<Vec<_>>().join(tr!(" または ", " or ").as_str());
    Some(if runs.len() > 1 { format!("({text})") } else { text })
}

pub fn show_region(r: &Region, axes: &[Axis], c: &Checked) -> String {
    let parts: Vec<String> = axes.iter().zip(r).filter_map(|(ax, sel)| show_axis(ax, sel, c)).collect();
    if parts.is_empty() {
        tr!("どんな入力でも", "the whole input space")
    } else {
        parts.join(tr!("  かつ  ", "  and  ").as_str())
    }
}

fn witness_text(c: &Checked, w: &BTreeMap<String, Val>) -> String {
    let mut p: Vec<String> = w.iter().map(|(k, v)| format!("{k}={}", crate::vectors::show_named(c, k, v))).collect();
    p.sort();
    p.join(", ")
}

/// What the rule accepts, where that changed. Printed before anything else, and printed
/// even when the two cannot be compared at all — it is often the whole answer.
/// One change to what the rule accepts, in words. The key stays English in the JSON; this
/// is the half a person reads.
fn domain_word(what: &str) -> String {
    match what {
        "input_added" => tr!("入力が増えた", "an input was added"),
        "input_removed" => tr!("入力が消えた", "an input was removed"),
        "input_type" => tr!("入力の型が変わった", "an input changed type"),
        "input_range" => tr!("入力の範囲が変わった", "an input changed range"),
        "enum_value_added" => tr!("列挙に値が増えた", "an enum gained a value"),
        "enum_value_removed" => tr!("列挙から値が消えた", "an enum lost a value"),
        "enum_added" => tr!("列挙が増えた", "an enum was added"),
        "enum_removed" => tr!("列挙が消えた", "an enum was removed"),
        "output_added" => tr!("出力が増えた", "an output was added"),
        "output_removed" => tr!("出力が消えた", "an output was removed"),
        "output_rounding" => tr!("出力の丸めが変わった", "an output changed rounding"),
        other => other.to_string(),
    }
}

/// What the rule accepts, where that changed. Printed before anything else, and printed
/// even when the two cannot be compared at all — it is often the whole answer.
fn domain_text(d: &VDiff) -> String {
    let mut s = String::new();
    for x in &d.domain {
        let extra = match (&x.old, &x.new) {
            (Some(a), Some(b)) => format!("  {a} → {b}"),
            _ => String::new(),
        };
        s.push_str(&format!("  {}: {}{extra}\n", domain_word(x.what), x.name));
    }
    s
}

/// The machine's half of a diff as text (§15.148).
fn machine_block(d: &VDiff, m: &MachineDiff, c: &Checked) -> String {
    let _ = d;
    let mut s = String::new();
    s.push_str(&format!("\n{}\n", tr!("ステートマシンとして:", "as a state machine:")));
    if m.initial.0 != m.initial.1 {
        s.push_str(&tr!(
            "  始まりの状態が {} から {} に変わりました。どの案件も、最初の呼び出しから違います。\n",
            "  the initial state moved from {} to {}: every case differs from its first call.\n",
            m.initial.0,
            m.initial.1
        ));
    } else if m.shortest.is_empty() {
        s.push_str(&tr!(
            "  始まりの状態から着ける呼び出しの並びで、答えが変わるものはありません。\n",
            "  no sequence of calls from the initial state gets a different answer.\n"
        ));
    } else {
        let (cin, cout) = (&m.carry.0, &m.carry.1);
        s.push_str(&tr!(
            "  答えが変わる呼び出しの並びのうち、いちばん短いもの（{} 回）:\n",
            "  the shortest sequence of calls the two answer differently ({} calls):\n",
            m.shortest.len()
        ));
        for (k, st) in m.shortest.iter().enumerate() {
            s.push_str(&mstep_text(c, st, cin, cout, k + 1));
            s.push('\n');
        }
    }
    if m.unreached > 0 {
        s.push_str(&tr!(
            "  上の範囲のうち {} 件は、古い版ではどの案件も着かない状態のものです。\n",
            "  {} of the regions above lie in states no case reaches under the old version.\n",
            m.unreached
        ));
    }
    if !m.migration.is_empty() {
        s.push_str(&tr!("  処理中の案件:\n", "  cases in progress:\n"));
        for (st, kind) in &m.migration {
            s.push_str(&format!("    - {}\n", migration_text(st, kind)));
        }
    }
    s
}

pub fn render(d: &VDiff, c: &Checked, terse: bool) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "{}\n",
        tr!("規則 {} v{} → v{}", "rule {} v{} → v{}", d.rule, d.old_version, d.new_version)
    ));
    s.push_str(&domain_text(d));
    if let Some(b) = &d.blocked {
        s.push_str(&format!("{}\n", tr!("比べられません: {}", "cannot be compared: {}", b)));
        return s;
    }
    if d.over_budget {
        s.push_str(&format!(
            "{}\n",
            tr!(
                "入力の組み合わせが {} 通りあり、`--budget` の上限を超えました。どこで違うかは出していません。",
                "the space of columns holds {} cells, over the budget; no region was worked out",
                d.cells
            )
        ));
        return s;
    }
    // Nothing that decides an answer changed, so the space was never walked. Printing a
    // count of one cell would be answering a question that was not asked.
    if d.axes.is_empty() {
        s.push_str(&format!(
            "{}\n",
            if d.domain.is_empty() {
                tr!(
                    "答えを決めるものは何も変わっていません。どの入力でも同じ答えです。",
                    "nothing that decides an answer changed: every input gets the same answer."
                )
            } else {
                tr!(
                    "答えを決めるものは何も変わっていません。両方の版が受け付ける入力では、どれも同じ答えです。",
                    "nothing that decides an answer changed: every input both versions accept gets the same answer."
                )
            }
        ));
        return s;
    }
    s.push_str(&format!(
        "{}\n",
        tr!(
            "入力の組み合わせ {} 通り。うち起きうるのは {} 通りで、同じ {} / 違う {} / 決められず {} / 入力を作れず {}",
            "{} cells, of which {} are inputs that can occur: {} same, {} differ, {} unsettled, {} unrealized",
            d.cells,
            d.feasible,
            d.same,
            d.differing,
            d.unsettled,
            d.unrealized
        )
    ));
    if d.changes.is_empty() && d.unknown.is_empty() && d.unrealized == 0 {
        s.push_str(&format!(
            "{}\n",
            if d.domain.is_empty() {
                tr!("どの入力でも答えは同じです。", "every input gets the same answer.")
            } else {
                tr!(
                    "両方の版が受け付ける入力では、どれも答えは同じです。",
                    "every input both versions accept gets the same answer."
                )
            }
        ));
        return s;
    }

    for ch in &d.changes {
        s.push_str(&format!("\n  {}\n", show_region(&ch.region, &d.axes, c)));
        for (k, a, b) in &ch.outs {
            let note = if ch.uniform { String::new() } else { tr!("（この例での差。同じ範囲でも入力によって差は変わります）", " (at the example; not uniform over the box)") };
            s.push_str(&format!("    {k}: {a} → {b}{note}\n"));
        }
        if ch.old_rows != ch.new_rows {
            s.push_str(&format!("    {}: {} → {}\n", tr!("当たる行", "rows"), rows_text(&ch.old_rows), rows_text(&ch.new_rows)));
        } else {
            s.push_str(&format!("    {}: {}\n", tr!("当たる行", "rows"), rows_text(&ch.old_rows)));
        }
        if !terse {
            s.push_str(&format!("    {}: {}\n", tr!("例", "example"), witness_text(c, &ch.witness)));
        }
    }
    for (r, why) in &d.unknown {
        s.push_str(&format!("\n  ? {}\n    {why}\n", show_region(r, &d.axes, c)));
    }
    if d.total() {
        s.push_str(&format!(
            "\n{}\n",
            if d.domain.is_empty() {
                tr!(
                    "ここに挙げた入力のほかでは、二つの版は同じ答えを返します。",
                    "outside this region the two versions answer alike."
                )
            } else {
                tr!(
                    "両方の版が受け付ける入力のうち、ここに挙げたほかは同じ答えを返します。受け付ける入力そのものの違いは上のとおりです。",
                    "of the inputs both versions accept, those outside this region get the same answer; what they accept differs as listed above."
                )
            }
        ));
    } else {
        s.push_str(&format!(
            "\n{}\n",
            tr!(
                "ここに挙げたほかの入力について、同じだとは言えていません。",
                "nothing is claimed about what lies outside the region."
            )
        ));
    }
    if let Some(m) = &d.machine {
        s.push_str(&machine_block(d, m, c));
    }
    s
}

/// The machine-facing shape (docs/formats.md). The keys are fixed in English whatever
/// language the prose is in, like every other `--format json`.
fn rows_json(rows: &[(String, usize)]) -> String {
    let v: Vec<String> = rows
        .iter()
        .map(|(t, r)| crate::json::Obj::new().str("table", t).int("row", *r as i128).finish())
        .collect();
    format!("[{}]", v.join(","))
}

pub fn render_json(d: &VDiff, c: &Checked, old: &str, new: &str) -> String {
    use crate::json::Obj;
    let region_json = |r: &Region| -> String {
        let parts: Vec<String> = d
            .axes
            .iter()
            .zip(r)
            .filter(|(ax, sel)| sel.len() != ax.len())
            .map(|(ax, sel)| {
                let coords: Vec<String> = sel
                    .iter()
                    .map(|&i| match &ax.coords[i] {
                        Coord::Word(w) => crate::json::quote(w),
                        Coord::OtherStr => crate::json::quote("*"),
                        co => {
                            let (lo, hi) = ival(co, ax.step);
                            Obj::new()
                                .opt_str("from", lo.map(|x| x.to_string()))
                                .opt_str("to", hi.map(|x| x.to_string()))
                                .finish()
                        }
                    })
                    .collect();
                Obj::new()
                    .str("column", &ax.col)
                    .str("kind", match ax.kind {
                        Kind::Input => "input",
                        Kind::Derived => "derived",
                        Kind::Walk => "walk",
                    })
                    .raw("accepts", format!("[{}]", coords.join(",")))
                    .str("text", show_axis(ax, sel, c).unwrap_or_default())
                    .finish()
            })
            .collect();
        format!("[{}]", parts.join(","))
    };
    let changes: Vec<String> = d
        .changes
        .iter()
        .map(|ch| {
            let outs: Vec<String> = ch
                .outs
                .iter()
                .map(|(k, a, b)| Obj::new().str("output", k).str("old", a).str("new", b).finish())
                .collect();
            let w: Vec<String> = ch
                .witness
                .iter()
                .map(|(k, v)| Obj::new().str("input", k).str("value", crate::vectors::show(v)).finish())
                .collect();
            Obj::new()
                .raw("region", region_json(&ch.region))
                .str("text", show_region(&ch.region, &d.axes, c))
                .raw("outputs", format!("[{}]", outs.join(",")))
                .bool("uniform", ch.uniform)
                .raw("old_rows", rows_json(&ch.old_rows))
                .raw("new_rows", rows_json(&ch.new_rows))
                .raw("witness", format!("[{}]", w.join(",")))
                .int("cells", ch.cells as i128)
                .finish()
        })
        .collect();
    let unknown: Vec<String> = d
        .unknown
        .iter()
        .map(|(r, why)| {
            Obj::new().raw("region", region_json(r)).str("text", show_region(r, &d.axes, c)).str("why", why).finish()
        })
        .collect();
    let domain: Vec<String> = d
        .domain
        .iter()
        .map(|x| Obj::new().str("what", x.what).str("name", &x.name).opt_str("old", x.old.as_ref()).opt_str("new", x.new.as_ref()).finish())
        .collect();
    Obj::new()
        .str("rule", &d.rule)
        .str("old", old)
        .str("new", new)
        .str("old_version", &d.old_version)
        .str("new_version", &d.new_version)
        .opt_str("blocked", d.blocked.as_ref())
        .bool("over_budget", d.over_budget)
        .bool("total", d.total())
        .int("cells", d.cells as i128)
        .int("feasible", d.feasible as i128)
        .int("same", d.same as i128)
        .int("differing", d.differing as i128)
        .int("unsettled", d.unsettled as i128)
        .int("unrealized", d.unrealized as i128)
        .raw("domain", format!("[{}]", domain.join(",")))
        .raw("changes", format!("[{}]", changes.join(",")))
        .raw("unknown", format!("[{}]", unknown.join(",")))
        .raw("machine", match &d.machine {
            None => "null".into(),
            Some(m) => {
                let outs = |x: &[(String, Option<Val>)]| -> String {
                    let mut o = Obj::new();
                    for (k, v) in x {
                        o = o.raw(k, match v {
                            Some(v) => crate::diag::WVal::json(&crate::eval::wval(c, k, v)),
                            None => "null".into(),
                        });
                    }
                    o.finish()
                };
                let steps: Vec<String> = m
                    .shortest
                    .iter()
                    .map(|st| {
                        let mut ins = Obj::new();
                        for (k, v) in &st.inputs {
                            ins = ins.raw(k, crate::diag::WVal::json(&crate::eval::wval(c, k, v)));
                        }
                        Obj::new().raw("inputs", ins.finish()).raw("old", outs(&st.old)).raw("new", outs(&st.new)).finish()
                    })
                    .collect();
                let mig: Vec<String> = m.migration.iter().map(|(st, k)| Obj::new().str("state", st).str("kind", k).finish()).collect();
                Obj::new()
                    .raw("carry", Obj::new().str("input", &m.carry.0).str("output", &m.carry.1).finish())
                    .raw("initial", Obj::new().str("old", &m.initial.0).str("new", &m.initial.1).finish())
                    .raw("shortest", format!("[{}]", steps.join(",")))
                    .int("unreached", m.unreached as i128)
                    .raw("migration", format!("[{}]", mig.join(",")))
                    .finish()
            }
        })
        .finish()
}

/// The same answer as markdown, to paste on a pull request.
pub fn markdown(d: &VDiff, c: &Checked, old: &str, new: &str, terse: bool) -> String {
    let mut s = String::new();
    s.push_str(&format!("### {}\n\n", tr!("版の差分（入力の全体で比べたもの）", "Version diff (over the whole input space)")));
    s.push_str(&format!("`{old}` → `{new}`\n\n"));
    for x in &d.domain {
        let extra = match (&x.old, &x.new) {
            (Some(a), Some(b)) => format!(" (`{a}` → `{b}`)"),
            _ => String::new(),
        };
        s.push_str(&format!("- `{}` — {}{extra}\n", x.name, domain_word(x.what)));
    }
    if !d.domain.is_empty() {
        s.push('\n');
    }
    if let Some(b) = &d.blocked {
        s.push_str(&format!("{}\n", tr!("比べられません: {}", "cannot be compared: {}", b)));
        return s;
    }
    if d.over_budget {
        s.push_str(&format!(
            "{}\n",
            tr!("入力の組み合わせが {} 通りあり、`--budget` の上限を超えました。", "the space of columns holds {} cells, over the budget.", d.cells)
        ));
        return s;
    }
    if d.changes.is_empty() && d.unknown.is_empty() {
        s.push_str(&format!(
            "{}\n",
            if d.axes.is_empty() {
                tr!(
                    "答えを決めるものは何も変わっていません。どの入力でも同じ答えです。",
                    "Nothing that decides an answer changed: every input gets the same answer."
                )
            } else {
                tr!("どの入力でも答えは同じです。", "Every input gets the same answer.")
            }
        ));
        return s;
    }
    let head = if terse {
        format!("| {} | {} | {} |\n|---|---|---|\n", tr!("違いが出る入力", "where they differ"), tr!("答え", "answer"), tr!("通り", "cells"))
    } else {
        format!(
            "| {} | {} | {} | {} |\n|---|---|---|---|\n",
            tr!("違いが出る入力", "where they differ"),
            tr!("答え", "answer"),
            tr!("例", "example"),
            tr!("通り", "cells")
        )
    };
    s.push_str(&head);
    for ch in &d.changes {
        let mark = if ch.uniform { "" } else { "*" };
        let outs: Vec<String> = ch.outs.iter().map(|(k, a, b)| format!("{k}: {a} → {b}{mark}")).collect();
        let region = show_region(&ch.region, &d.axes, c);
        if terse {
            s.push_str(&format!("| {region} | {} | {} |\n", outs.join("<br>"), ch.cells));
        } else {
            s.push_str(&format!("| {region} | {} | {} | {} |\n", outs.join("<br>"), witness_text(c, &ch.witness), ch.cells));
        }
    }
    for (r, _) in &d.unknown {
        let region = show_region(r, &d.axes, c);
        s.push_str(&if terse { format!("| {region} | ? | |\n") } else { format!("| {region} | ? | | |\n") });
    }
    s.push('\n');
    if d.changes.iter().any(|c| !c.uniform) {
        s.push_str(&format!(
            "{}\n\n",
            tr!("\\* その行はこの例での差です。同じ範囲でも入力によって差は変わります。", "\\* at the example; not uniform over that box.")
        ));
    }
    s.push_str(&format!(
        "{}\n",
        if d.total() && d.domain.is_empty() {
            tr!("ここに挙げた入力のほかでは、二つの版は同じ答えを返します。", "Outside this region the two versions answer alike.")
        } else if d.total() {
            tr!("両方の版が受け付ける入力のうち、ここに挙げたほかは同じ答えを返します。", "Of the inputs both versions accept, those outside this region get the same answer.")
        } else {
            tr!("ここに挙げたほかの入力について、同じだとは言えていません。", "Nothing is claimed about what lies outside the region.")
        }
    ));
    if let Some(m) = &d.machine {
        s.push_str(&format!("\n#### {}\n\n", tr!("ステートマシンとして", "As a state machine")));
        for line in machine_block(d, m, c).lines().skip(2) {
            let t = line.trim_start();
            if t.is_empty() {
                continue;
            }
            // The lines of the text form, as a list: the calls numbered, the rest bulleted.
            if t.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
                s.push_str(&format!("   {t}\n"));
            } else if let Some(x) = t.strip_prefix("- ") {
                s.push_str(&format!("   - {x}\n"));
            } else {
                s.push_str(&format!("- {t}\n"));
            }
        }
    }
    s
}

/// Whether the cell's coordinates are consistent with each other on their face: every
/// scalar `derive` can reach its coordinate from somewhere inside the inputs' boxes. A cell
/// that fails this holds no input at all and is not counted against the claim; one that
/// passes it and still could not be realized is.
/// The inputs of a point inside the cell, solved rather than walked towards (§15.129).
///
/// `None` when the arithmetic cannot read the cell, when there is no point, or when the
/// point it found is not whole. Nothing here is trusted: the caller holds what comes back to
/// every coordinate, the same way it holds the walk's own answer.
fn solved_point(
    axes: &[Axis],
    cell: &[usize],
    f: &RuleFile,
    c: &Checked,
    scalars: &[(String, Expr)],
    inputs: &BTreeSet<String>,
) -> Option<HashMap<String, Val>> {
    use crate::fourier::{grounds, Lin};
    // Only a cell whose computed columns are correlated is worth solving; where they are
    // not, the walk already reached whatever there was.
    if !scalars.iter().any(|(n, _)| axes.iter().any(|a| a.col == *n)) {
        return None;
    }
    let seed: Vec<String> = axes.iter().map(|a| a.col.clone()).collect();
    // One system per type of number. They share no name, so their solutions put together
    // are a solution of the whole — and every value is held to the cell again below anyway.
    let gs = grounds(&seed, f, c);
    if gs.is_empty() {
        return None;
    }
    let mut at: BTreeMap<String, Rat> = BTreeMap::new();
    for g in gs {
        let mut sys = g.sys;
        for (ai, a) in axes.iter().enumerate() {
            if !g.vars.contains(&a.col) {
                continue;
            }
            let Some(co) = a.coords.get(cell[ai]) else { continue };
            let x = Lin::var(&a.col);
            match co {
                Coord::Point(p) => {
                    let d = x.plus(&Lin::con(p.mul(Rat::int(-1))));
                    sys.push(d.clone().le(false));
                    sys.push(d.ge(false));
                }
                Coord::Open(lo, hi) => {
                    if let Some(lo) = lo {
                        sys.push(x.clone().plus(&Lin::con(lo.mul(Rat::int(-1)))).ge(true));
                    }
                    if let Some(hi) = hi {
                        sys.push(x.plus(&Lin::con(hi.mul(Rat::int(-1)))).le(true));
                    }
                }
                Coord::Word(_) | Coord::OtherStr => {}
            }
        }
        at.extend(crate::fourier::solve(sys)?);
    }
    // Only the free inputs are taken from the solution; every computed column is recomputed
    // from them and checked, which is what makes an unsound point harmless.
    let mut out: HashMap<String, Val> = HashMap::new();
    for a in axes {
        if a.kind != Kind::Input {
            continue;
        }
        match at.get(&a.col) {
            Some(v) if v.is_int() => {
                out.insert(a.col.clone(), Val::Num(*v));
            }
            // A free input the system said nothing about keeps the coordinate the cell gave
            // it; one the system placed off the grid is not a value a caller can send.
            Some(_) => return None,
            None => {
                let i = axes.iter().position(|x| x.col == a.col)?;
                out.insert(a.col.clone(), coord_val(a, cell[i])?);
            }
        }
    }
    let _ = inputs;
    (!out.is_empty()).then_some(out)
}

/// Whether no input of this version reaches the cell, as far as linear arithmetic can tell
/// (§15.129).
///
/// The per-axis walk gives every computed column an axis of its own, so two columns off one
/// input move independently there and a cell that no caller can reach still looks like one
/// the walk owes an answer for. The columns of the cell are conditions on a system the rule
/// already fixes — the defining equation of each `derive`, each declared range, each
/// `constraint` — and eliminating variables decides it.
///
/// **`true` means proved empty; `false` means nothing.** A coordinate this cannot read is
/// dropped, which only relaxes the system, so a `true` really does settle the cell and a
/// `false` leaves it exactly where it was.
fn linearly_empty(axes: &[Axis], cell: &[usize], v: (&RuleFile, &Checked)) -> bool {
    use crate::fourier::{grounds, pinned, Lin};
    let (f, c) = v;
    // A boolean `define` the cell fixes: its body is a comparison that has to hold there.
    let body_of = |n: &str| -> Option<&Expr> {
        f.items.iter().find_map(|it| match it {
            Item::Define(d) if d.name.text == n => Some(&d.expr),
            _ => None,
        })
    };
    let mut pins: Vec<(&Expr, bool)> = Vec::new();
    let mut seed: Vec<String> = Vec::new();
    for (ai, a) in axes.iter().enumerate() {
        match (&a.ty, a.coords.get(cell[ai])) {
            (Ty::Bool, Some(Coord::Word(w))) => {
                let Some(e) = body_of(&a.col) else { continue };
                let mut names = Vec::new();
                names_of(e, &mut names);
                seed.extend(names);
                pins.push((e, w == crate::kw::TRUE));
            }
            _ => seed.push(a.col.clone()),
        }
    }
    // Nothing correlated: the walk already sees everything this would.
    let computed = |n: &str| f.items.iter().any(|it| matches!(it, Item::Derived(d) if d.name.text == n));
    // One system per type of number; any of them with no solution empties the cell, because
    // each is the cell with some of its conditions left out.
    grounds(&seed, f, c).into_iter().any(|g| {
        if pins.is_empty() && !g.vars.iter().any(|n| computed(n)) {
            return false;
        }
        let mut sys = g.sys;
        for (ai, a) in axes.iter().enumerate() {
            if !g.vars.contains(&a.col) {
                continue;
            }
            let Some(co) = a.coords.get(cell[ai]) else { continue };
            let x = Lin::var(&a.col);
            match co {
                Coord::Point(p) => {
                    let d = x.plus(&Lin::con(p.mul(Rat::int(-1))));
                    sys.push(d.clone().le(false));
                    sys.push(d.ge(false));
                }
                Coord::Open(lo, hi) => {
                    if let Some(lo) = lo {
                        sys.push(x.clone().plus(&Lin::con(lo.mul(Rat::int(-1)))).ge(true));
                    }
                    if let Some(hi) = hi {
                        sys.push(x.plus(&Lin::con(hi.mul(Rat::int(-1)))).le(true));
                    }
                }
                Coord::Word(_) | Coord::OtherStr => {}
            }
        }
        for (e, yes) in &pins {
            sys.extend(pinned(e, *yes, &g.want, c));
        }
        crate::fourier::unsat(sys)
    })
}

/// Every name an expression mentions, appended.
fn names_of(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Name(n, _) => out.push(n.clone()),
        Expr::Lit(..) => {}
        Expr::Bin(a, _, b, _) => {
            names_of(a, out);
            names_of(b, out);
        }
        Expr::Call(_, args, _) => args.iter().for_each(|a| names_of(a, out)),
    }
}

fn feasible_shape(axes: &[Axis], cell: &[usize], f: &RuleFile, c: &Checked, scalars: &[(String, Expr)]) -> bool {
    let ax_of: HashMap<&str, usize> = axes.iter().enumerate().map(|(i, a)| (a.col.as_str(), i)).collect();
    for (name, expr) in scalars {
        let Some(&ai) = ax_of.get(name.as_str()) else { continue };
        let inputs: BTreeSet<String> = axes.iter().filter(|a| a.kind == Kind::Input).map(|a| a.col.clone()).collect();
        let deps: Vec<String> = root_inputs(name, scalars, &inputs);
        // The exact range of a linear expression over a box is reached at its corners, so
        // walking them decides the question rather than approximating it.
        let movable: Vec<&String> = deps.iter().filter(|d| ax_of.contains_key(d.as_str())).collect();
        if movable.len() > 8 {
            return true;
        }
        let mut worlds: Vec<HashMap<String, Val>> = vec![axes
            .iter()
            .enumerate()
            .filter(|(_, a)| a.kind == Kind::Input)
            .filter_map(|(i, a)| coord_val(a, cell[i]).map(|v| (a.col.clone(), v)))
            .collect()];
        for d in &movable {
            let Some(&di) = ax_of.get(d.as_str()) else { continue };
            if axes[di].kind != Kind::Input {
                continue;
            }
            let (lo, hi) = ival(&axes[di].coords[cell[di]], axes[di].step);
            let ends: Vec<Rat> = [lo, hi].into_iter().flatten().collect();
            if ends.is_empty() {
                continue;
            }
            let mut next = Vec::new();
            for w in &worlds {
                for e in &ends {
                    let mut m = w.clone();
                    m.insert((*d).clone(), val_of(&axes[di], *e));
                    next.push(m);
                }
            }
            worlds = next;
            if worlds.len() > 512 {
                return true;
            }
        }
        let reachable = worlds.iter().filter(|w| constraints_hold(f, w)).any(|w| {
            match crate::eval::Env::new(c, chained(w, scalars, c)).expr(expr) {
                Some(Val::Num(r)) => {
                    let (lo, hi) = ival(&axes[ai].coords[cell[ai]], axes[ai].step);
                    // A corner on either side of the target means the target is crossed.
                    let _ = (lo, hi);
                    true_if_crosses(&axes[ai], cell[ai], r, &worlds, c, expr, f, scalars)
                }
                Some(v) => holds(&axes[ai], cell[ai], &v),
                None => false,
            }
        });
        if !reachable {
            return false;
        }
    }
    true
}

/// Whether the target interval lies between the smallest and the largest value the corners
/// give. A linear expression takes every value in between, so this is exact.
fn true_if_crosses(
    ax: &Axis,
    ci: usize,
    _at: Rat,
    worlds: &[HashMap<String, Val>],
    c: &Checked,
    expr: &Expr,
    f: &RuleFile,
    scalars: &[(String, Expr)],
) -> bool {
    let mut lo: Option<Rat> = None;
    let mut hi: Option<Rat> = None;
    for w in worlds.iter().filter(|w| constraints_hold(f, w)) {
        if let Some(Val::Num(r)) = crate::eval::Env::new(c, chained(w, scalars, c)).expr(expr) {
            lo = Some(match lo {
                None => r,
                Some(p) => if r.cmp_to(p) == Ordering::Less { r } else { p },
            });
            hi = Some(match hi {
                None => r,
                Some(p) => if r.cmp_to(p) == Ordering::Greater { r } else { p },
            });
        }
    }
    let (Some(lo), Some(hi)) = (lo, hi) else { return false };
    let (tlo, thi) = ival(&ax.coords[ci], ax.step);
    tlo.is_none_or(|t| t.cmp_to(hi) != Ordering::Greater) && thi.is_none_or(|t| t.cmp_to(lo) != Ordering::Less)
}


// ---------------------------------------------------------------------------------------
// One rule's space, a cell at a time (§15.148)
// ---------------------------------------------------------------------------------------

/// What one cell of one rule's space turned out to be.
pub(crate) enum Settled {
    /// An input the cell stands for, and what the rule answers for it.
    Realized(HashMap<String, Val>, Answer),
    /// No input realizes the cell: its coordinates contradict each other, or linear
    /// arithmetic proves the system empty. It is nobody's case.
    Empty,
    /// No input was built, and none was shown impossible either.
    Unknown,
}

/// The walk `diff` makes over two versions, made over one: the axes of the rule's columns,
/// and a cell settled on request. What a state machine's checks need is exactly this — every
/// class of input the rule tells apart, each with an input that shows what the rule does with
/// it — and the three-valued answer is the same one: what could not be settled is said to be
/// unsettled, never folded into either side (§15.99).
pub(crate) struct Walker<'a> {
    pub(crate) axes: Vec<Axis>,
    f: &'a RuleFile,
    c: &'a Checked,
    menu: Vec<BTreeMap<String, Val>>,
    scalars: Vec<(String, Expr)>,
    cone: Vec<usize>,
    memo: HashMap<Vec<usize>, Solved>,
    shape_memo: HashMap<Vec<usize>, bool>,
}

impl<'a> Walker<'a> {
    /// The axes of `f`, with `extra` boundaries and `more` columns (see `axis_with` and
    /// `columns_with`). `Err` says why the space does not cut into cells.
    pub(crate) fn new(
        f: &'a RuleFile,
        c: &'a Checked,
        extra: &[(String, Rat)],
        more: &BTreeSet<String>,
    ) -> Result<Self, String> {
        if f.fold.is_some() {
            return Err(tr!(
                "この規則は並び全体を見て答えを出すので、入力を決まった数の項目の組み合わせに分けられません",
                "this rule folds a sequence: the answer depends on the whole of it, so the space of columns does not cut into cells"
            ));
        }
        let mut axes = Vec::new();
        for (col, kind) in columns_with(f, f, more) {
            let Some(ty) = c.ty_of(&col) else {
                return Err(tr!("列 {} の型が分かりません", "the type of column {} is not known", col));
            };
            let Some(ax) = axis_with(&col, kind, &ty, &[(f, c)], extra) else {
                return Err(tr!("列 {} の型 {} では、値を区切れません", "column {} has type {}, which does not cut into coordinates", col, ty));
            };
            if ax.coords.is_empty() {
                return Err(tr!("列 {} の値を区切れません", "column {} does not cut into coordinates", col));
            }
            axes.push(ax);
        }
        let input_names: BTreeSet<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
        let scalars = scalar_derives(f, &input_names);
        let cone = cone_of(&axes, f, c, &scalars);
        Ok(Walker {
            menu: element_menu(f, c, 24),
            axes,
            f,
            c,
            scalars,
            cone,
            memo: HashMap::new(),
            shape_memo: HashMap::new(),
        })
    }

    /// How many cells the space has.
    pub(crate) fn size(&self) -> u128 {
        self.axes.iter().map(|a| a.len() as u128).product::<u128>().max(1)
    }

    /// Settle one cell.
    pub(crate) fn settle(&mut self, cell: &[usize]) -> Settled {
        if let Some((inputs, a)) = realize(&self.axes, cell, self.f, self.c, &self.menu, &self.scalars, &self.cone, &mut self.memo, None) {
            return Settled::Realized(inputs, a);
        }
        let k: Vec<usize> = self.cone.iter().map(|&i| cell[i]).collect();
        let shaped = match self.shape_memo.get(&k) {
            Some(v) => *v,
            None => {
                let v = feasible_shape(&self.axes, cell, self.f, self.c, &self.scalars);
                self.shape_memo.insert(k, v);
                v
            }
        };
        if !shaped || linearly_empty(&self.axes, cell, (self.f, self.c)) {
            Settled::Empty
        } else {
            Settled::Unknown
        }
    }

    /// Other inputs of the same cell, each held to the cell's coordinates again, with what
    /// the rule answers for them. The first input of a cell is one step inside its closed end;
    /// these are its far ends, which is where an answer that reads an open column moves.
    pub(crate) fn others(&mut self, cell: &[usize], first: &HashMap<String, Val>) -> Vec<(HashMap<String, Val>, Answer)> {
        let mut out = Vec::new();
        for seed in other_points(&self.axes, cell, first, 6) {
            if let Some(got) = realize(&self.axes, cell, self.f, self.c, &self.menu, &self.scalars, &self.cone, &mut self.memo, Some(&seed)) {
                out.push(got);
            }
        }
        out
    }
}
