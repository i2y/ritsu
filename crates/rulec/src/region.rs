//! The region IR and box algebra (§6).
//!
//! Each input column becomes an independent axis, and its coordinates are compressed to the
//! boundary values that appear in the column. Because §3 restricts a cell to a unary test on
//! its own column, a row is "the product of one subset per axis", that is, a box, and
//! completeness, unreachable rows, and overlaps all reduce to finite set operations.

use crate::ast::*;
use crate::diag::{Diag, FixKind, Span};
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
    /// The absent value of an optional number or date: the first coordinate of its axis, as
    /// `none` is the first value of an optional enum's (§15.201). No comparison and no literal
    /// takes it; `none` takes it alone, and `-` and `not:` take it with the rest.
    Absent,
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
    ///
    /// `days` is the set a date input takes when its range is a date of a koyomi file
    /// (`range from koyomi …`, §15.174): a coordinate with no day of it in is one no input
    /// reaches, and a witness on the axis is always one of its days.
    ///
    /// `grid` is the step the column's values take (§15.190), which `step` divides: they are
    /// the same but where the values are coarser than the whole units a cell is written in
    /// (`amount × 30%` takes 0.3, and the axis is cut at 0.1). A witness inside an interval
    /// coordinate is put on it where it can be, so that it is a value the column can hold.
    Num { unit: String, date: bool, coords: Vec<Coord>, shown: i128, wire: i128, step: Rat, grid: Rat, days: Option<std::sync::Arc<Vec<i64>>> },
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

/// The first day of a sorted set strictly between two boundaries, either of which may run on.
fn day_inside(days: &[i64], a: Option<Rat>, b: Option<Rat>) -> Option<i64> {
    days.iter().copied().find(|d| {
        let v = Rat::int(*d as i128);
        a.is_none_or(|a| a.cmp_to(v) == std::cmp::Ordering::Less) && b.is_none_or(|b| v.cmp_to(b) == std::cmp::Ordering::Less)
    })
}

fn inside(a: Option<Rat>, b: Option<Rat>, step: Rat) -> Rat {
    match (a, b) {
        (Some(a), _) => a.add(step),
        (None, Some(b)) => b.sub(step),
        (None, None) => Rat::zero(),
    }
}

/// A value strictly inside `(a, b)` on `grid`, the step a column's values take (§15.190): the
/// first after `a`, or with `a` open the last before `b`. Where `grid` has no value in there, or
/// is the axis's own step, the axis's step past the end, as [`inside`] gives it.
fn inside_on(a: Option<Rat>, b: Option<Rat>, step: Rat, grid: Rat) -> Rat {
    let at = inside(a, b, step);
    if grid.cmp_to(step) == std::cmp::Ordering::Equal || grid.num <= 0 {
        return at;
    }
    let lt = |x: Rat, y: Rat| x.cmp_to(y) == std::cmp::Ordering::Less;
    let on = (|| -> Option<Rat> {
        let k = |v: Rat| -> Option<i128> {
            let q = v.checked_div(grid)?;
            Some(q.num.div_euclid(q.den))
        };
        match (a, b) {
            (Some(a), _) => grid.checked_mul(Rat::int(k(a)?.checked_add(1)?)),
            (None, Some(b)) => {
                let q = b.checked_div(grid)?;
                let below = if q.is_int() { q.num.checked_sub(1)? } else { q.num.div_euclid(q.den) };
                grid.checked_mul(Rat::int(below))
            }
            (None, None) => Some(Rat::zero()),
        }
    })();
    match on {
        Some(v) if a.is_none_or(|a| lt(a, v)) && b.is_none_or(|b| lt(v, b)) => v,
        _ => at,
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
            Axis::Num { coords, step, grid, days, .. } => match coords.get(i) {
                Some(Coord::Absent) => None,
                Some(Coord::Point(v)) => Some(*v),
                // On an axis of koyomi's days, the first day inside the interval (§15.174).
                Some(Coord::Open(a, b)) => match days {
                    Some(ds) => day_inside(ds, *a, *b).map(|d| Rat::int(d as i128)).or(Some(inside(*a, *b, *step))),
                    None => Some(inside_on(*a, *b, *step, *grid)),
                },
                None => None,
            },
        }
    }

    /// Whether coordinate `i` holds a day of the axis's set, on an axis of koyomi's days; true
    /// on every other axis, where nothing is ruled out this way (§15.174).
    fn holds_a_day(&self, i: usize) -> bool {
        match self {
            Axis::Num { coords, days: Some(ds), .. } => match coords.get(i) {
                // The absent value is no day, and it is a value the input takes (§15.201).
                Some(Coord::Absent) => true,
                Some(Coord::Point(v)) => v.den == 1 && ds.binary_search(&(v.num as i64)).is_ok(),
                Some(Coord::Open(a, b)) => day_inside(ds, *a, *b).is_some(),
                None => true,
            },
            _ => true,
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
            // The absent value of an optional number or date is written as the cell that tests
            // it (§15.201).
            Axis::Num { coords, .. } if matches!(coords.get(i), Some(Coord::Absent)) => crate::kw::NONE.into(),
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

    /// [`Axis::witness_at`] as a sentence or a cell says it: a number with its digits grouped as
    /// the languages group them (`30_001JPY`, §15.333). The certificate keeps the labels
    /// `witness_at` writes, which a re-checker matches one against another.
    fn witness_said_at(&self, i: usize, chosen: Option<Rat>) -> String {
        match self {
            Axis::Num { coords, date: false, unit, shown, .. } if !matches!(coords.get(i), Some(Coord::Absent)) => match self.value_at(i, chosen) {
                Some(v) => format!("{}{unit}", crate::types::written_num(v.mul(Rat::int(*shown)))),
                None => String::new(),
            },
            _ => self.witness_at(i, chosen),
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
            // An optional truth value's axis is the enum of `none`, `true` and `false` (§15.201);
            // its two values are still truth values. No enum has a value called either.
            Axis::Enum { values } if matches!(values.get(i).map(String::as_str), Some(crate::kw::TRUE | crate::kw::FALSE)) => {
                WVal::Bool(values[i] == crate::kw::TRUE)
            }
            Axis::Enum { .. } => WVal::Str(self.witness_at(i, chosen)),
            Axis::Prefix { patterns } => WVal::Str(match patterns.get(i) {
                Some(p) => p.clone(),
                None => outside_all(patterns),
            }),
            Axis::Bool => WVal::Bool(i == 0),
            // The absent value is written as the word the cell tests it with, as an optional
            // enum's is (§15.201).
            Axis::Num { coords, .. } if matches!(coords.get(i), Some(Coord::Absent)) => WVal::Str(crate::kw::NONE.into()),
            // Empty unit means "a date"; the display form is already `YYYY-MM-DD`.
            Axis::Num { date: true, .. } => WVal::Str(self.witness_at(i, chosen)),
            Axis::Num { wire, .. } => {
                let v = self.value_at(i, chosen).unwrap_or(Rat::zero());
                // A value the rule computes can fall between the integers of the wire — a derive
                // of tenths of a pound is 1.1 (§15.190). It travels on no wire, so it is written
                // as the number it is rather than cut to an integer it is not.
                match v.checked_mul(Rat::int(*wire)) {
                    Some(s) if s.is_int() => WVal::Int(s.num),
                    _ => WVal::Str(v.to_string()),
                }
            }
        }
    }

    /// Whether `v` can be written in a cell of this axis's column: a whole number of the unit
    /// for money, a quantity and `number`, which is all their cells take (§15.190).
    fn writable(&self, v: Rat) -> bool {
        match self {
            Axis::Num { date: false, unit, shown, .. } if *shown == 1 && unit != "%" => v.is_int(),
            _ => true,
        }
    }
}

/// One `never` line as one table reads it (§15.282): each axis it tests, with the coordinates its
/// cell takes there. The axes are cut at the line's own values, so a coordinate is wholly inside
/// or wholly outside.
#[derive(Debug, Clone)]
pub struct NeverCut {
    /// The line's place among the rule's `never` lines.
    pub line: usize,
    pub on: Vec<(usize, Vec<bool>)>,
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
    /// Axis → the numbers the table's own cells write in the column, each with the text it is
    /// written in (`2kg` for 2000). A row the completeness check asks for writes a boundary the
    /// way the table does, where the table writes it.
    written: Vec<Vec<(Rat, String)>>,
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
    /// If the axis is a `define` that computes a number, the values it can come to: its
    /// expression over the intervals of what it reads (`Checked::ranges`) — the inputs' ranges,
    /// the derives' and the other defines' reach, the values a table above writes — which holds
    /// every value it takes (§15.195). The completeness check and the third form of E102 read it
    /// as they read a derive's; nothing else here does, so an overlap is decided as before.
    define_reach: Vec<Option<Ival>>,
    /// If the axis is a boolean `define` of one comparison that takes one truth value over the
    /// ranges it is read with, that value ([`truth_of_expr`]); the other coordinate is a point no
    /// input reaches (§15.196). Read by the whole sieve, `feasible` included, so the completeness
    /// check, the overlap checks and ritsu's port see the same points.
    define_truth: Vec<Option<bool>>,
    /// Every `derive` in the rule, by name. The interval a derived column can actually reach is
    /// its expression worked out over the intervals of what it reads (§5) — which is what the
    /// per-axis sieve does not look at.
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
    /// The rule's `never` lines this table has a column for every name of (§15.282). A box they
    /// take is a combination that does not happen.
    nevers: Vec<NeverCut>,
    /// The rule's linear model around this table's numeric axes (§15.141): the defining
    /// equation of every linear `derive` the axes reach, the declared ranges, and every
    /// `constraint`, closed over the names they tie together — one system for each type of
    /// number. The sieve reads a constraint on its own and a derived axis against its own
    /// range; the model reads them together, which is what a chain of constraints through
    /// an input no table has a column for needs. Empty when it would say nothing the ranges
    /// do not.
    model: Vec<crate::fourier::Ground>,
    /// Axis → for a numeric column a table above decides where every row of it writes a number
    /// literal or `none`, whether some row writes into each coordinate (§15.322). A coordinate no
    /// row writes into is a point no input reaches, read by the whole sieve; each is a `never`
    /// fact of the certificate's.
    written_above: Vec<Option<Vec<bool>>>,
    /// What a `define` comes to with the axes it reads held to one coordinate each, where that
    /// leaves coordinates of its own axis out (§15.322), for every `define` the linear model has
    /// no equation for. Read by the whole sieve; each is a fact the certificate states and both
    /// re-checkers work out again.
    narrowed: Vec<NarrowCut>,
    /// The derives and `define`s whose values the rule works out, by name: every one but the open
    /// values (§15.325). A column a table above cuts that is one of them is worked out again over a
    /// box of this table's, to tell the rows of that table that cannot fire there.
    computed: BTreeMap<String, Expr>,
    /// The step each input is stored at, by name ([`input_steps`]): the grid a value read with some
    /// names held follows an input it reads onto ([`held_grid`]).
    steps: BTreeMap<String, Rat>,
}

/// A combination of coordinates, one on each of some numeric axes a `define` reads, and the
/// coordinates of the define's own axis no input reaches together with them: what the define's
/// expression comes to with those axes held to those coordinates leaves them out (§15.322). A
/// table over `fee = down(amount * 3%, 1JPY)` and `amount` reads `fee > 900JPY` as out of reach
/// beside `amount <= 30_000JPY`.
#[derive(Debug, Clone)]
pub struct NarrowCut {
    /// The axis of the `define`.
    pub axis: usize,
    /// The axes held, each at one coordinate.
    pub by: Vec<(usize, usize)>,
    /// Coordinate of `axis` → whether no input reaches it beside `by`.
    pub out: Vec<bool>,
    /// One of the axes of `by` — or an input the table has no axis for, over the whole of its range
    /// (§15.325) — and the values it is cut at: the define is worked out again on each piece between
    /// two cuts (the ends shared), and leaves the coordinates of `out` out on every one (§15.322).
    /// `None` where the whole coordinate does it.
    pub cuts: Option<(CutOn, Vec<Rat>)>,
}

/// What a fact of `narrowed` cuts into pieces (§15.322, §15.325).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CutOn {
    /// An axis of `by`, by its place among the table's axes: the coordinate it is held at.
    Axis(usize),
    /// An input the table has no axis for, by its name: the whole of the interval the int64 claim
    /// reads it in. `fee = down(amount * 1%, 1JPY) - up(amount * 2%, 1JPY)` read alone comes to
    /// −3 yen to 0 yen over the whole of `amount`, and to −1 yen or −2 yen over each piece of it.
    Input(String),
}

/// How many combinations of coordinates of the axes a `define` reads are each worked out on their
/// own; past it, one axis at a time is (§15.322).
const NARROW_COMBOS: usize = 256;

/// How many of a row's boxes an input is built in before the row is left undecided (W128, §15.322).
const REACHING_BOXES: usize = 8;

/// How many pieces one coordinate of a held axis is cut into for one coordinate of a `define`'s
/// axis, and how many times a table works a `define` out on a piece in all (§15.322). Past either,
/// the coordinate is asked for as before.
const NARROW_PIECES: usize = 64;
/// How many pieces one reason a row of a table above fires nowhere in a box tries, over all the
/// axes it holds (§15.325): the reason is asked for each box and each row, so it tries fewer than a
/// fact of `narrowed`, which is worked out once for the table.
const REACH_SPLITS: usize = 256;
const NARROW_SPLITS: usize = 4096;

/// The grid the values of `e` sit on (§15.322): `(origin, step)`, every value being `origin + k ·
/// step` for some whole `k`, and a step of zero the one value `origin`. A rounding puts its value on
/// the multiples of its step, whatever it rounds; a sum or a difference sits on the finest grid both
/// sides sit on; a multiple or a quotient by a constant on that multiple or quotient of the grid;
/// `min` and `max` on one grid both arguments sit on; a derive or a `define` on the grid of its
/// expression. `None` where the expression says nothing of one: a name the rule does not work out,
/// a product of two values. Both re-checkers work it out the same way from the expressions.
fn grid_of(chk: &Checked, e: &Expr, ty: &Ty, computed: &BTreeMap<String, Expr>, depth: usize) -> Option<(Rat, Rat)> {
    if depth > 16 {
        return None;
    }
    let abs = |x: Rat| if x.num < 0 { Rat::zero().sub(x) } else { x };
    let gcd = |a: Rat, b: Rat| -> Option<Rat> {
        match (a.num == 0, b.num == 0) {
            (true, _) => Some(abs(b)),
            (_, true) => Some(abs(a)),
            _ => crate::types::common_step(abs(a), abs(b)),
        }
    };
    // A literal stands for the value the certificate writes beside it.
    let lit = |x: &Expr| -> Option<Rat> {
        match x {
            Expr::Lit(Lit::Num(n), _) => crate::types::lit_value_in_pub(n, ty).or_else(|| crate::types::lit_value_in_pub(n, &crate::types::lit_ty_pub(n))),
            _ => None,
        }
    };
    match e {
        Expr::Lit(Lit::Num(_), _) => lit(e).map(|v| (v, Rat::zero())),
        Expr::Name(n, _) => {
            let x = computed.get(n)?;
            grid_of(chk, x, chk.ty_of(n)?.present(), computed, depth + 1)
        }
        Expr::Bin(l, op, r, _) => match op {
            BinOp::Add | BinOp::Sub => {
                let (a, s) = grid_of(chk, l, ty, computed, depth + 1)?;
                let (b, t) = grid_of(chk, r, ty, computed, depth + 1)?;
                let o = if *op == BinOp::Add { a.checked_add(b)? } else { a.checked_sub(b)? };
                Some((o, gcd(s, t)?))
            }
            BinOp::Mul => {
                let (k, (o, s)) = match lit(r) {
                    Some(k) => (k, grid_of(chk, l, ty, computed, depth + 1)?),
                    None => (lit(l)?, grid_of(chk, r, ty, computed, depth + 1)?),
                };
                Some((o.checked_mul(k)?, s.checked_mul(abs(k))?))
            }
            BinOp::Div => {
                let k = lit(r)?;
                if k.num == 0 {
                    return None;
                }
                let (o, s) = grid_of(chk, l, ty, computed, depth + 1)?;
                Some((o.checked_div(k)?, s.checked_div(abs(k))?))
            }
            _ => None,
        },
        Expr::Call(name, args, _) => match (name.as_str(), args.as_slice()) {
            (m, [_, g]) if crate::num::RoundMode::parse(m).is_some() => {
                let step = lit(g)?;
                (step.num > 0).then_some((Rat::zero(), step))
            }
            (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                let (a, s) = grid_of(chk, x, ty, computed, depth + 1)?;
                let (b, t) = grid_of(chk, y, ty, computed, depth + 1)?;
                Some((a, gcd(gcd(s, t)?, a.checked_sub(b)?)?))
            }
            _ => None,
        },
        _ => None,
    }
}

/// The grid `e`'s values sit on with the names of `over` held to their intervals (§15.325): as
/// [`grid_of`] works it out, where a part of `e` the held names pin to one value ([`narrow_iv`]) sits
/// on that value alone; a value of `inline` on its own expression's grid read the same way; another
/// value the rule works out on [`grid_of`]'s; an input on the step it is stored at (`steps`, one over
/// its scale, §7.1); and a product on a multiple of one side's grid where the other side is one value.
/// `amount * qty` with `qty` held at 3 sits on the multiples of 3 yen, and
/// `down(amount * 1%, 1JPY) + qty * 37JPY` with `amount` held under 100 yen on those of 37 yen. A
/// re-checker reads the same (Python's `held_grid`; Lean's `gridOf` with the point `heldInterval`
/// comes to, `eval_onGridAt_gridOf`).
#[allow(clippy::too_many_arguments)]
fn held_grid(
    chk: &Checked,
    e: &Expr,
    ty: &Ty,
    over: &[(String, (Rat, Rat))],
    inline: &BTreeMap<String, Expr>,
    computed: &BTreeMap<String, Expr>,
    steps: &BTreeMap<String, Rat>,
    depth: usize,
) -> Option<(Rat, Rat)> {
    if depth > 16 {
        return None;
    }
    if let Some((lo, hi)) = narrow_iv(chk, e, ty, over, inline, 0) {
        if lo.cmp_to(hi) == std::cmp::Ordering::Equal {
            return Some((lo, Rat::zero()));
        }
    }
    let abs = |x: Rat| if x.num < 0 { Rat::zero().sub(x) } else { x };
    let gcd = |a: Rat, b: Rat| -> Option<Rat> {
        match (a.num == 0, b.num == 0) {
            (true, _) => Some(abs(b)),
            (_, true) => Some(abs(a)),
            _ => crate::types::common_step(abs(a), abs(b)),
        }
    };
    let lit = |x: &Expr| -> Option<Rat> {
        match x {
            Expr::Lit(Lit::Num(n), _) => crate::types::lit_value_in_pub(n, ty).or_else(|| crate::types::lit_value_in_pub(n, &crate::types::lit_ty_pub(n))),
            _ => None,
        }
    };
    let sub = |x: &Expr| held_grid(chk, x, ty, over, inline, computed, steps, depth + 1);
    match e {
        Expr::Lit(Lit::Num(_), _) => lit(e).map(|v| (v, Rat::zero())),
        Expr::Name(n, _) => {
            if let Some(x) = inline.get(n) {
                return held_grid(chk, x, chk.ty_of(n)?.present(), over, inline, computed, steps, depth + 1);
            }
            match computed.get(n) {
                Some(x) => grid_of(chk, x, chk.ty_of(n)?.present(), computed, depth + 1),
                None => steps.get(n).map(|st| (Rat::zero(), *st)),
            }
        }
        Expr::Bin(l, op, r, _) => match op {
            BinOp::Add | BinOp::Sub => {
                let (a, s) = sub(l)?;
                let (b, t) = sub(r)?;
                let o = if *op == BinOp::Add { a.checked_add(b)? } else { a.checked_sub(b)? };
                Some((o, gcd(s, t)?))
            }
            BinOp::Mul => {
                let (a, s) = sub(l)?;
                let (b, t) = sub(r)?;
                let o = a.checked_mul(b)?;
                if t.num == 0 {
                    Some((o, abs(s.checked_mul(b)?)))
                } else if s.num == 0 {
                    Some((o, abs(t.checked_mul(a)?)))
                } else {
                    None
                }
            }
            BinOp::Div => {
                let k = lit(r)?;
                if k.num == 0 {
                    return None;
                }
                let (o, s) = sub(l)?;
                Some((o.checked_div(k)?, s.checked_div(abs(k))?))
            }
            _ => None,
        },
        Expr::Call(name, args, _) => match (name.as_str(), args.as_slice()) {
            (m, [_, g]) if crate::num::RoundMode::parse(m).is_some() => {
                let step = lit(g)?;
                (step.num > 0).then_some((Rat::zero(), step))
            }
            (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                let (a, s) = sub(x)?;
                let (b, t) = sub(y)?;
                Some((a, gcd(gcd(s, t)?, a.checked_sub(b)?)?))
            }
            _ => None,
        },
        _ => None,
    }
}

/// What a `define` comes to, met with the range the rule accepts for it (§15.313): a call whose
/// value falls outside is turned away, so no call the rule answers has it there. The two ends cross
/// where no such call is left.
fn within_accepts((lo, hi): (Rat, Rat), acc: Option<&(Option<Rat>, Option<Rat>)>) -> (Rat, Rat) {
    match acc {
        Some((l, h)) => (l.map_or(lo, |l| crate::grid::max_rat(lo, l)), h.map_or(hi, |h| crate::grid::min_rat(hi, h))),
        None => (lo, hi),
    }
}

/// The step each input is stored at, by name: one over its scale (§7.1). Every value of the input
/// is a whole number of it.
fn input_steps(c: &Checked, f: &RuleFile) -> BTreeMap<String, Rat> {
    f.inputs
        .iter()
        .filter_map(|i| {
            let k = *c.scales.get(&i.name.text)?;
            (k > 0).then(|| (i.name.text.clone(), Rat::new(1, k)))
        })
        .collect()
}

/// Whether no value of `grid` lies both on coordinate `cd` and inside `reach` (§15.322): a point
/// off the grid or outside the reach, or an interval coordinate with no grid value strictly between
/// its ends that the reach takes.
fn grid_misses(cd: &Coord, (o, s): (Rat, Rat), (rl, rh): (Rat, Rat)) -> bool {
    use std::cmp::Ordering::*;
    // What the coordinate takes inside the reach: two ends, each left out or taken.
    let (l, l_out, r, r_out) = match cd {
        Coord::Absent => return false,
        Coord::Point(v) => (*v, false, *v, false),
        Coord::Open(a, b) => {
            let (l, l_out) = match a {
                Some(a) if a.cmp_to(rl) != Less => (*a, true),
                _ => (rl, false),
            };
            let (r, r_out) = match b {
                Some(b) if b.cmp_to(rh) != Greater => (*b, true),
                _ => (rh, false),
            };
            (l, l_out, r, r_out)
        }
    };
    if l.cmp_to(rl) == Less || r.cmp_to(rh) == Greater {
        return true;
    }
    if s.num == 0 {
        let above = if l_out { o.cmp_to(l) == Greater } else { o.cmp_to(l) != Less };
        let below = if r_out { o.cmp_to(r) == Less } else { o.cmp_to(r) != Greater };
        return !(above && below);
    }
    let (Some(ql), Some(qr)) = (l.checked_sub(o).and_then(|x| x.checked_div(s)), r.checked_sub(o).and_then(|x| x.checked_div(s))) else {
        return false;
    };
    let floor = |q: Rat| q.num.div_euclid(q.den);
    let ceil = |q: Rat| -(-q.num).div_euclid(q.den);
    let lo = if l_out { floor(ql) + 1 } else { ceil(ql) };
    let hi = if r_out { ceil(qr) - 1 } else { floor(qr) };
    lo > hi
}

/// Whether a value that comes to the interval `(rl, rh)`, on the grid `grid` where there is one,
/// never stands on coordinate `cd` (§15.322): the coordinate lies outside the interval, or holds no
/// value of the grid inside it. The absent value is never out this way.
fn coord_out(cd: &Coord, (rl, rh): (Rat, Rat), grid: Option<(Rat, Rat)>) -> bool {
    use std::cmp::Ordering::*;
    let outside = match cd {
        Coord::Point(v) => v.cmp_to(rl) == Less || v.cmp_to(rh) == Greater,
        Coord::Open(a, b) => b.is_some_and(|b| b.cmp_to(rl) != Greater) || a.is_some_and(|a| a.cmp_to(rh) != Less),
        Coord::Absent => false,
    };
    outside || grid.is_some_and(|g| grid_misses(cd, g, (rl, rh)))
}

/// Whether every piece of `[lo, hi]`, values the whole multiples of `step`, leaves a coordinate out,
/// cutting a piece in two wherever it does not: `[a, m]` and `[m + step, b]`, `m` the middle on the
/// grid, down to pieces of one value (§15.322). The cuts made — where each piece but the first starts — or `None`
/// past [`NARROW_PIECES`] pieces or the table's `left`, or at a value that does not leave it out.
fn split_out(lo: Rat, hi: Rat, step: Rat, left: &mut usize, out: &dyn Fn(Rat, Rat) -> bool) -> Option<Vec<Rat>> {
    use std::cmp::Ordering::*;
    let mut stack = vec![(lo, hi)];
    let mut cuts: Vec<Rat> = Vec::new();
    while let Some((a, b)) = stack.pop() {
        *left = left.checked_sub(1)?;
        if out(a, b) {
            continue;
        }
        if a.cmp_to(b) != Less || cuts.len() + 2 > NARROW_PIECES {
            return None;
        }
        let n = b.checked_sub(a)?.checked_div(step)?;
        let m = a.checked_add(Rat::int(n.num.div_euclid(n.den) / 2).checked_mul(step)?)?;
        let next = m.checked_add(step)?;
        // A cut is a whole multiple of the step, so that no value lies between two pieces.
        if m.cmp_to(a) == Less || next.cmp_to(b) == Greater || next.checked_div(step)?.den != 1 {
            return None;
        }
        cuts.push(next);
        stack.push((next, b));
        stack.push((a, m));
    }
    cuts.sort_by(|x, y| x.cmp_to(*y));
    Some(cuts)
}

/// The names a value reads, through the derives and `define`s it reads.
fn reads_through(name: &str, exprs: &BTreeMap<String, Expr>, defines: &BTreeMap<String, Expr>, out: &mut BTreeSet<String>) {
    let Some(e) = defines.get(name).or_else(|| exprs.get(name)) else { return };
    let mut ns = Vec::new();
    names_in(e, &mut ns);
    for n in ns {
        if out.insert(n.clone()) {
            reads_through(&n, exprs, defines, out);
        }
    }
}

/// The columns of a table a value reads (§15.322): the names its expression reads that are
/// columns, and through any other derive or `define` it reads, theirs. A column stops the walk: what
/// it reads in turn comes to it through its own value, which the box holds.
fn reads_to_columns(name: &str, exprs: &BTreeMap<String, Expr>, defines: &BTreeMap<String, Expr>, columns: &[String], out: &mut BTreeSet<String>, seen: &mut BTreeSet<String>) {
    let Some(e) = defines.get(name).or_else(|| exprs.get(name)) else { return };
    let mut ns = Vec::new();
    names_in(e, &mut ns);
    for n in ns {
        if columns.contains(&n) {
            out.insert(n);
        } else if seen.insert(n.clone()) {
            reads_to_columns(&n, exprs, defines, columns, out, seen);
        }
    }
}

/// What `e` comes to with the names of `over` held to their intervals, and every derive or
/// `define` in `inline` worked out again from its own expression the same way (§15.322); any other
/// name is read as the int64 claim reads it ([`Checked::int64_interval`]). The interval arithmetic
/// alone: [`narrow_iv`] meets it with what the linear form says.
fn narrow_plain(chk: &Checked, e: &Expr, ty: &Ty, over: &[(String, (Rat, Rat))], inline: &BTreeMap<String, Expr>, depth: usize) -> Option<(Rat, Rat)> {
    if depth > 16 {
        return None;
    }
    let look = |n: &str| -> Option<Option<(Rat, Rat)>> {
        if let Some((_, iv)) = over.iter().find(|(m, _)| m == n) {
            return Some(Some(*iv));
        }
        let x = inline.get(n)?;
        let t = chk.ty_of(n)?;
        Some(narrow_plain(chk, x, t.present(), over, inline, depth + 1))
    };
    chk.interval_by(e, ty, true, Some(&look))
}

/// A value read as a linear form over the names held (§15.325): `Σ coeffs[i] · over[i] + r`, the
/// coefficients in the order of `over`, and `r` somewhere in `[lo, hi]`. Interval arithmetic reads
/// `amount - amount` over 115 to 337 yen as anything from −222 to 222 yen; the form reads it as 0.
#[derive(Debug, Clone)]
struct HeldForm {
    coeffs: Vec<Rat>,
    lo: Rat,
    hi: Rat,
}

impl HeldForm {
    fn constant(n: usize, (lo, hi): (Rat, Rat)) -> HeldForm {
        HeldForm { coeffs: vec![Rat::zero(); n], lo, hi }
    }

    /// `self + k · o`.
    fn plus(&self, o: &HeldForm, k: Rat) -> Option<HeldForm> {
        let o = o.times(k)?;
        let coeffs = self.coeffs.iter().zip(&o.coeffs).map(|(a, b)| a.checked_add(*b)).collect::<Option<Vec<Rat>>>()?;
        Some(HeldForm { coeffs, lo: self.lo.checked_add(o.lo)?, hi: self.hi.checked_add(o.hi)? })
    }

    fn times(&self, k: Rat) -> Option<HeldForm> {
        let coeffs = self.coeffs.iter().map(|c| c.checked_mul(k)).collect::<Option<Vec<Rat>>>()?;
        let (a, b) = (self.lo.checked_mul(k)?, self.hi.checked_mul(k)?);
        let (lo, hi) = if a.cmp_to(b) == std::cmp::Ordering::Greater { (b, a) } else { (a, b) };
        Some(HeldForm { coeffs, lo, hi })
    }

    /// The one value it stands for, where it reads no held name and its remainder is a point.
    fn point(&self) -> Option<Rat> {
        (self.coeffs.iter().all(|c| c.num == 0) && self.lo.cmp_to(self.hi) == std::cmp::Ordering::Equal).then_some(self.lo)
    }

    /// Everything it can come to with each held name anywhere in its interval.
    fn range(&self, over: &[(String, (Rat, Rat))]) -> Option<(Rat, Rat)> {
        let (mut lo, mut hi) = (self.lo, self.hi);
        for (c, (_, (l, h))) in self.coeffs.iter().zip(over) {
            let (a, b) = (c.checked_mul(*l)?, c.checked_mul(*h)?);
            let (a, b) = if a.cmp_to(b) == std::cmp::Ordering::Greater { (b, a) } else { (a, b) };
            lo = lo.checked_add(a)?;
            hi = hi.checked_add(b)?;
        }
        Some((lo, hi))
    }
}

/// `e` as a linear form over the names of `over` (§15.325): a held name is itself; a derive or a
/// `define` in `inline` is read through its own expression; a sum, a difference, a multiple of a
/// constant and a quotient by one keep the form; `min` and `max` are the argument the other never
/// passes, where the difference of the two forms keeps one sign over the held intervals; a
/// rounding to a grid `g` is its argument with `[-g, g]` added to the remainder, since every mode
/// lands within one step of what it rounds. Anything else is a constant: the interval arithmetic's
/// interval, as the remainder. Both re-checkers read a value the same way.
fn narrow_form(chk: &Checked, e: &Expr, ty: &Ty, over: &[(String, (Rat, Rat))], inline: &BTreeMap<String, Expr>, depth: usize) -> Option<HeldForm> {
    if depth > 16 {
        return None;
    }
    let n = over.len();
    let fall = || narrow_plain(chk, e, ty, over, inline, depth).map(|iv| HeldForm::constant(n, iv));
    let form = |x: &Expr| narrow_form(chk, x, ty, over, inline, depth);
    let one = Rat::int(1);
    match e {
        Expr::Name(m, _) => {
            if let Some(i) = over.iter().position(|(x, _)| x == m) {
                let mut f = HeldForm::constant(n, (Rat::zero(), Rat::zero()));
                f.coeffs[i] = one;
                return Some(f);
            }
            match (inline.get(m), chk.ty_of(m)) {
                (Some(x), Some(t)) => narrow_form(chk, x, t.present(), over, inline, depth + 1).or_else(fall),
                _ => fall(),
            }
        }
        Expr::Bin(l, op, r, _) if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) => {
            let (Some(a), Some(b)) = (form(l), form(r)) else { return fall() };
            let got = match op {
                BinOp::Add => a.plus(&b, one),
                BinOp::Sub => a.plus(&b, Rat::int(-1)),
                BinOp::Mul => match (b.point(), a.point()) {
                    (Some(k), _) => a.times(k),
                    (None, Some(k)) => b.times(k),
                    _ => None,
                },
                _ => match b.point() {
                    Some(k) if k.num != 0 => one.checked_div(k).and_then(|q| a.times(q)),
                    _ => None,
                },
            };
            got.or_else(fall)
        }
        Expr::Call(name, args, _) => match (name.as_str(), args.as_slice()) {
            (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                let (Some(a), Some(b)) = (form(x), form(y)) else { return fall() };
                let Some((dl, dh)) = a.plus(&b, Rat::int(-1)).and_then(|d| d.range(over)) else { return fall() };
                let max = name == crate::kw::MAX;
                if dl.cmp_to(Rat::zero()) != std::cmp::Ordering::Less {
                    Some(if max { a } else { b })
                } else if dh.cmp_to(Rat::zero()) != std::cmp::Ordering::Greater {
                    Some(if max { b } else { a })
                } else {
                    fall()
                }
            }
            (m, [x, g]) if crate::num::RoundMode::parse(m).is_some() => {
                let Some(step) = narrow_plain(chk, g, ty, over, inline, depth).filter(|(a, b)| a.cmp_to(*b) == std::cmp::Ordering::Equal).map(|(a, _)| a) else {
                    return fall();
                };
                if step.num <= 0 {
                    return fall();
                }
                match form(x) {
                    Some(f) => match (f.lo.checked_sub(step), f.hi.checked_add(step)) {
                        (Some(lo), Some(hi)) => Some(HeldForm { coeffs: f.coeffs, lo, hi }),
                        _ => fall(),
                    },
                    None => fall(),
                }
            }
            _ => fall(),
        },
        _ => fall(),
    }
}

/// What `e` comes to with the names of `over` held to their intervals (§15.322): the interval
/// arithmetic of [`narrow_plain`], met with the range of the linear form [`narrow_form`] over the
/// same intervals (§15.325). `max(amount, 35JPY) - amount` with `amount` from 115 to 337 yen comes
/// to 0 yen; the interval arithmetic alone says −222 to 222 yen.
fn narrow_iv(chk: &Checked, e: &Expr, ty: &Ty, over: &[(String, (Rat, Rat))], inline: &BTreeMap<String, Expr>, depth: usize) -> Option<(Rat, Rat)> {
    let (lo, hi) = narrow_plain(chk, e, ty, over, inline, depth)?;
    let Some((fl, fh)) = narrow_form(chk, e, ty, over, inline, depth).and_then(|f| f.range(over)) else {
        return Some((lo, hi));
    };
    Some((crate::grid::max_rat(lo, fl), crate::grid::min_rat(hi, fh)))
}

/// The truth value a boolean `define` of one comparison always takes with the names of `over`
/// held to their intervals, as [`truth_of_expr`] reads it over the ranges (§15.322).
fn narrow_truth(chk: &Checked, e: &Expr, over: &[(String, (Rat, Rat))], inline: &BTreeMap<String, Expr>) -> Option<bool> {
    let Expr::Bin(l, op, r, _) = e else { return None };
    let ty = |e: &Expr| match e {
        Expr::Name(n, _) => chk.ty_of(n),
        _ => None,
    };
    let want = ty(l).or_else(|| ty(r))?;
    let side = |e: &Expr| -> Option<(Rat, Rat)> {
        match e {
            Expr::Lit(Lit::Date(y, m, d), _) => {
                let v = crate::types::date_ord(*y, *m, *d);
                Some((v, v))
            }
            _ => narrow_iv(chk, e, want.present(), over, inline, 0),
        }
    };
    let ((al, ah), (bl, bh)) = (side(l)?, side(r)?);
    use std::cmp::Ordering::*;
    let le = |x: Rat, y: Rat| x.cmp_to(y) != Greater;
    let lt = |x: Rat, y: Rat| x.cmp_to(y) == Less;
    match op {
        BinOp::Le if le(ah, bl) => Some(true),
        BinOp::Le if lt(bh, al) => Some(false),
        BinOp::Lt if lt(ah, bl) => Some(true),
        BinOp::Lt if le(bh, al) => Some(false),
        BinOp::Ge if le(bh, al) => Some(true),
        BinOp::Ge if lt(ah, bl) => Some(false),
        BinOp::Gt if lt(bh, al) => Some(true),
        BinOp::Gt if le(ah, bl) => Some(false),
        _ => None,
    }
}

/// Why a `never` line rules out nothing (W134, §15.286).
enum NeverIdle {
    /// No value the name can take meets this condition of the line, by its place in the line.
    NoValue(usize),
    /// Everything it takes is out already: by these other lines and these `constraint`s, by their
    /// places among the rule's.
    TakenAlready { lines: BTreeSet<usize>, constraints: BTreeSet<usize> },
    /// It has no door, and no table has all of its names as columns.
    Unread,
}

/// More combinations than this in one line are not walked (W134): the line stays unsaid.
const NEVER_IDLE_POINTS: usize = 4096;

/// What a line takes is read as a table of one row reads it: the line's names as the columns, its
/// cells as the row. The axes then hold what the table's check would hold, the ranges and the other
/// lines' values among them, so the line's combinations are those coordinates and none of them cut.
fn never_idle(place: usize, n: &crate::ast::NeverLine, f: &RuleFile, c: &Checked, told: &BTreeSet<usize>) -> Option<NeverIdle> {
    let row = Row {
        cells: n.conds.iter().map(|k| k.cell.clone()).collect(),
        cell_spans: n.conds.iter().map(|k| k.name.span.clone()).collect(),
        outs: Vec::new(),
        out_spans: Vec::new(),
        span: n.span.clone(),
        index: 1,
        cite: None,
        label: None,
        origin: None,
    };
    let table = Table {
        name: None,
        policy: Policy::Unique,
        inputs: n.conds.iter().map(|k| (k.name.text.clone(), k.name.span.clone())).collect(),
        outputs: Vec::new(),
        rows: vec![row],
        span: n.span.clone(),
        overrides: Vec::new(),
        clause: false,
        cite: None,
        applied: None,
        open: None,
        contributed: None,
    };
    let reg = TableRegion::build(&table, c, f)?;
    if reg.unanalyzable.is_some() {
        return None;
    }
    let picks: Vec<Vec<usize>> = reg.masks[0]
        .iter()
        .enumerate()
        .map(|(ai, m)| (0..m.len()).filter(|ci| m[*ci] && reg.axes[ai].holds_a_day(*ci)).collect())
        .collect();
    if let Some(ai) = picks.iter().position(Vec::is_empty) {
        return n.conds.iter().position(|k| k.name.text == reg.col_names[ai]).map(NeverIdle::NoValue);
    }
    if picks.iter().try_fold(1usize, |all, p| all.checked_mul(p.len())).is_some_and(|all| all <= NEVER_IDLE_POINTS) {
        let (mut lines, mut constraints) = (BTreeSet::new(), BTreeSet::new());
        let mut at = vec![0usize; picks.len()];
        let taken = loop {
            let path: Vec<usize> = at.iter().zip(&picks).map(|(k, p)| p[*k]).collect();
            if let Some(k) = reg.constraints.iter().position(|k| reg.constraint_impossible(k, &path)) {
                constraints.insert(k);
            } else if let Some(other) = reg.nevers.iter().find(|m| m.line != place && !told.contains(&m.line) && m.on.iter().all(|(ai, mask)| mask[path[*ai]])) {
                lines.insert(other.line);
            } else {
                break false;
            }
            // The next combination, the last axis moving fastest.
            let mut ai = picks.len();
            loop {
                if ai == 0 {
                    break;
                }
                ai -= 1;
                at[ai] += 1;
                if at[ai] < picks[ai].len() {
                    ai = usize::MAX;
                    break;
                }
                at[ai] = 0;
            }
            if ai != usize::MAX {
                break true;
            }
        };
        if taken {
            return Some(NeverIdle::TakenAlready { lines, constraints });
        }
    }
    let door = f.door_nevers().any(|d| std::ptr::eq(d, n));
    let read = c.sets.iter().any(|set| n.names().all(|m| set.table.inputs.iter().any(|(col, _)| col == m)));
    (!door && !read).then_some(NeverIdle::Unread)
}

/// W134 (§15.286): the `never` lines that rule out nothing. A line that takes nothing is a line the
/// person who approves the page reads as saying something: a condition written against a range that
/// has since moved, a value the enum no longer has, or a line another one holds whole. Two lines
/// that take the same are told from the last, so that the first stays with nothing said of it.
pub fn idle_nevers(f: &RuleFile, c: &Checked, rule_path: &str) -> Vec<Diag> {
    let mut told = BTreeSet::new();
    let mut found = Vec::new();
    for (place, n) in f.nevers.iter().enumerate().rev() {
        if let Some(why) = never_idle(place, n, f, c, &told) {
            told.insert(place);
            found.push((n, why));
        }
    }
    found.reverse();
    found.into_iter().map(|(n, why)| idle_never_diag(n, &why, f, c, rule_path)).collect()
}

fn idle_never_diag(n: &crate::ast::NeverLine, why: &NeverIdle, f: &RuleFile, c: &Checked, rule_path: &str) -> Diag {
    let said = n.said();
    let at = tr!("{rule_path}:{} 起こらない組み合わせ", "{rule_path}:{} never", n.span.line);
    let d = |title: String, label: String| Diag::warning("W134", title).at(at.clone()).key(format!("W134\u{1}{said}")).fix_kind(FixKind::None).mark(n.span.clone(), label);
    match why {
        NeverIdle::NoValue(k) => {
            let cond = n.conds[*k].said();
            let name = &n.conds[*k].name.text;
            let mut d = d(
                tr!("`{said}` は何も外しません。`{cond}` に当たる値がありません", "`{said}` rules out nothing: no value meets `{cond}`"),
                tr!("`{name}` の値は、この条件に当たりません", "no value of `{name}` meets this condition"),
            );
            if let Some(whole) = name_takes(name, c) {
                d = d.note(tr!("`{name}` が取るのは {whole} です。", "`{name}` takes {whole}."));
            }
            d.note(tr!(
                "ヒント: 範囲か列挙の値が動いたのなら、行をいまの値に合わせてください。要らないなら、この行を消してください。",
                "hint: if the range or the values of the enum have moved, write the line for what they are now. If it is not needed, delete it."
            ))
        }
        NeverIdle::TakenAlready { lines, constraints } => {
            let mut those: Vec<String> = lines.iter().filter_map(|k| f.nevers.get(*k)).map(|m| format!("`{}`", m.said())).collect();
            those.extend(constraints.iter().filter_map(|k| f.constraints.get(*k)).map(|k| format!("`{} {}`", crate::kw::CONSTRAINT, k.said())));
            let ls = those.join(tr!("、", ", ").as_str());
            d(
                tr!("`{said}` は何も外しません。ほかの行がすでに外しています", "`{said}` rules out nothing: other lines take all of it"),
                tr!("この組み合わせは {ls} がすでに外しています", "already taken out by {ls}"),
            )
            .note(tr!(
                "ヒント: この行を消してください。消しても、外れるものは変わりません。",
                "hint: delete this line. What is taken out stays the same without it."
            ))
        }
        NeverIdle::Unread => d(
            tr!("`{said}` は何も外しません。読む表がありません", "`{said}` rules out nothing: no table reads it"),
            tr!("名前を全部列に持つ表がありません", "no table has all of these names as columns"),
        )
        .note(tr!(
            "集計だけの行には入口がなく、効くのは名前が全部列にある表だけです。",
            "A line of summaries alone has no door. It acts only on a table with all of its names as columns."
        ))
        .note(tr!(
            "ヒント: 表がこの組み合わせを読むなら、名前を全部その表の列にしてください。読まないなら、この行を消してください。",
            "hint: if a table is to read this combination, make every name a column of it. If none is, delete this line."
        )),
    }
}

/// What a name can be, in words, for a line that asks something else of it. `None` when there is
/// nothing to say that the line does not say: a truth value, a name with no ends.
fn name_takes(name: &str, c: &Checked) -> Option<String> {
    match c.ty_of(name)?.present() {
        Ty::Enum(e) => {
            let all = c.enum_ranges.get(name).cloned().or_else(|| c.enums.get(e).cloned())?;
            Some(all.join(", "))
        }
        _ => {
            let whole = crate::doc::range_text(c, name);
            (!whole.is_empty()).then_some(whole)
        }
    }
}

fn lit_rat(l: &Lit, want: &Ty) -> Option<Rat> {
    match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, want),
        Lit::Date(y, m, d) if *want == Ty::Date => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

/// The range a walk's answer takes where a table reads it (§15.219, §15.220, §15.236): a summary's
/// (`count`, `sum`, `max`, `min`) and a fold's named answer, as the type check worked it out — the
/// declared range of a `count` and a `sum`, the hull of the column's range and the `empty` value
/// of a `max` and a `min`, the hull of the values a fold answers with. A table that reads one is
/// checked over that range, as it is over an input's declared one, and the certificate states it
/// for the re-checkers to hold the axis to (`walks`, `extremes`, `fold_answers`).
fn walk_answer_range(f: &RuleFile, c: &Checked, name: &str) -> Option<(Option<Rat>, Option<Rat>)> {
    let answers = f.items.iter().any(|it| match it {
        Item::Agg(d) => d.name.text == name,
        Item::Fold(d) => d.answer_name(f) == Some(name),
        _ => false,
    });
    if answers { c.ranges.get(name).copied() } else { None }
}

/// Takes the ends of a range among the boundaries of an axis, and as its ends. A boundary a cell
/// names beyond them is not a coordinate, as one beyond an input's declared range is not
/// (`num_bounds`): the range is the universe (§15.314). Left in, a row for a count of 0 under a
/// range that starts at 1 looked reachable, a cell written `>0` had the table asked for a row at
/// 0, and the certificate stated an axis wider than the range it stated beside it, which the
/// re-checkers turn away.
fn bound_by(b: &mut Vec<Rat>, lo: &mut Option<Rat>, hi: &mut Option<Rat>, (rlo, rhi): (Option<Rat>, Option<Rat>)) {
    for v in [rlo, rhi].into_iter().flatten() {
        b.push(v);
    }
    *lo = rlo.or(*lo);
    *hi = rhi.or(*hi);
    let (l, h) = (*lo, *hi);
    b.retain(|x| l.is_none_or(|l| x.cmp_to(l) != std::cmp::Ordering::Less) && h.is_none_or(|h| x.cmp_to(h) != std::cmp::Ordering::Greater));
    b.sort_by(|x, y| x.cmp_to(*y));
    b.dedup_by(|x, y| x.cmp_to(*y) == std::cmp::Ordering::Equal);
}

/// The ends of the range an input's line writes, as the type check read them (`Checked::declared`),
/// or read the same way here for a file the check did not see this input in.
fn declared_ends(c: &Checked, name: &str, r: &Range, ty: &Ty) -> Option<(Option<Rat>, Option<Rat>)> {
    c.declared.get(name).copied().or_else(|| {
        let step = Rat::new(1, c.scales.get(name).copied().unwrap_or(1).max(1));
        crate::types::range_ends(r, ty, Some(step))
    })
}

/// Collect the boundaries of a numeric axis. Both ends of the declared range count as
/// boundaries too, which closes the universe into a finite one. The ends are the ones the type
/// check read off the range (`types::bounds_of`, §15.314): each included, on the step the value is
/// held at, so the axis ends where the door does. They were read here a second time, off the
/// range as written, with `>` taken as `>=`.
fn num_bounds(rows: &[Row], ci: usize, want: &Ty, declared: Option<(Option<Rat>, Option<Rat>)>) -> (Vec<Rat>, Option<Rat>, Option<Rat>) {
    let mut set: BTreeSet<(i128, i128)> = BTreeSet::new();
    let push = |r: Rat, s: &mut BTreeSet<(i128, i128)>| {
        s.insert((r.num, r.den));
    };
    let (lo, hi) = declared.unwrap_or((None, None));
    for v in [lo, hi].into_iter().flatten() {
        push(v, &mut set);
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
        // Each member of a set is a point of its own, as a single value is.
        if let Some(Cell::Set(ls) | Cell::Not(ls)) = row.cells.get(ci) {
            for l in ls {
                if let Some(v) = lit_rat(l, want) {
                    push(v, &mut set);
                }
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

/// The numbers the cells of column `ci` write, each with the text of the first cell that writes
/// it: the form a row the check asks for gives the same boundary in.
fn written_numbers(rows: &[Row], ci: usize, want: &Ty) -> Vec<(Rat, String)> {
    let mut out: Vec<(Rat, String)> = Vec::new();
    for row in rows {
        let lits: Vec<&Lit> = match row.cells.get(ci) {
            Some(Cell::Cmp(cs)) => cs.iter().map(|(_, l)| l).collect(),
            Some(Cell::Lit(l)) => vec![l],
            Some(Cell::Set(ls) | Cell::Not(ls)) => ls.iter().collect(),
            _ => Vec::new(),
        };
        for l in lits {
            let Lit::Num(n) = l else { continue };
            let Some(v) = crate::types::lit_value_in_pub(n, want) else { continue };
            if !out.iter().any(|(w, _)| w.cmp_to(v) == std::cmp::Ordering::Equal) {
                let text = if n.neg && !n.raw.starts_with('-') { format!("-{}", n.raw) } else { n.raw.clone() };
                out.push((v, text));
            }
        }
    }
    out
}

/// The values a `never` line tests a column at, cut into its coordinates as a cell's are
/// (§15.282): a line takes whole coordinates, or it could not take a box out of the table.
fn never_bounds(b: &mut Vec<Rat>, (lo, hi): (Option<Rat>, Option<Rat>), f: &RuleFile, name: &str, want: &Ty) {
    let inside = |x: &Rat| lo.is_none_or(|l| x.cmp_to(l) != std::cmp::Ordering::Less) && hi.is_none_or(|h| x.cmp_to(h) != std::cmp::Ordering::Greater);
    for cell in f.never_cells(name) {
        let lits: Vec<&Lit> = match cell {
            Cell::Cmp(cs) => cs.iter().map(|(_, l)| l).collect(),
            Cell::Lit(l) => vec![l],
            Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
            _ => Vec::new(),
        };
        for v in lits.into_iter().filter_map(|l| lit_rat(l, want)).filter(inside) {
            if !b.iter().any(|x| x.cmp_to(v) == std::cmp::Ordering::Equal) {
                b.push(v);
            }
        }
    }
    b.sort_by(|a, b| a.cmp_to(*b));
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
        // A comparison holds only of a value that is there (§2.1, §15.201).
        Coord::Absent => false,
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

/// The table a column's value comes from, when one decides it: that table — the merged table of a
/// set — the column's place among its outputs, and the roundings on the way (§15.322). A `define`
/// that only names another value is that value: an `apply` brings an output in so
/// (`define 受ける名前 = 準用名:出力`), and a number rounded the way the callee declares
/// (`define 手数料 = down(準用名:手数料, 1円)`), which the rows' numbers go through in turn.
pub(crate) struct Source<'a> {
    pub table: &'a Table,
    pub oi: usize,
    /// The roundings, in the order they are made: the innermost definition's first.
    pub rounds: Vec<(crate::num::RoundMode, Rat)>,
}

/// How many definitions that only name another value are followed to the table that decides it.
const SOURCE_STEPS: usize = 16;

/// The table a column's value comes from (§15.322), through the definitions that only name another
/// value. `None` for an input, a derive, any other definition, and a column nothing decides.
pub(crate) fn source_of<'a>(f: &'a RuleFile, c: &'a Checked, t: &Table, k: &str) -> Option<Source<'a>> {
    let mut name = k.to_string();
    let mut rounds: Vec<(crate::num::RoundMode, Rat)> = Vec::new();
    for _ in 0..SOURCE_STEPS {
        if let Some(u) = producing_table(f, c, t, &name) {
            let oi = u.outputs.iter().position(|o| o.name.text == name)?;
            rounds.reverse();
            return Some(Source { table: u, oi, rounds });
        }
        if f.open_value(&name).is_some() {
            return None;
        }
        let d = f.items.iter().find_map(|it| match it {
            Item::Define(d) if d.name.text == name => Some(d),
            _ => None,
        })?;
        match &d.expr {
            Expr::Name(n, _) => name = n.clone(),
            Expr::Call(m, args, _) => {
                let mode = crate::num::RoundMode::parse(m)?;
                let [Expr::Name(n, _), Expr::Lit(Lit::Num(g), _)] = args.as_slice() else { return None };
                let ty = c.ty_of(&d.name.text)?;
                let grid = crate::types::lit_value_in_pub(g, ty.present())?;
                if grid.num <= 0 {
                    return None;
                }
                rounds.push((mode, grid));
                name = n.clone();
            }
            _ => return None,
        }
    }
    None
}

/// What a row of the table a column comes from puts into it, read as the column holds it here
/// (§15.322).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Put {
    /// The value word the row writes (`none` included, an optional column's absent value).
    Word(String),
    /// The number the row writes, put through the roundings on the way.
    Num(Rat),
    /// The absent value of an optional number or date.
    Absent,
    /// Something these readings do not follow: a name, an expression. It may stand anywhere.
    Any,
}

/// What row `ri` of `t` puts into its output `oi`, put through `rounds`: a word the row writes as
/// such (`written_value`), a number literal, a date literal as its day number (§15.325), or `none`.
/// A name the row passes on is read as anything, as both re-checkers read it (§15.322).
pub(crate) fn put_of(t: &Table, ri: usize, oi: usize, rounds: &[(crate::num::RoundMode, Rat)], c: &Checked) -> Put {
    use crate::num::RoundTo;
    let (Some(row), Some(col)) = (t.rows.get(ri), t.outputs.get(oi)) else { return Put::Any };
    let Some(ty) = c.ty_of(&col.name.text) else { return Put::Any };
    match ty.present() {
        Ty::Enum(_) | Ty::Bool => written_value(row.outs.get(oi), c).map(|w| Put::Word(w.to_string())).unwrap_or(Put::Any),
        p @ (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number) => match row.outs.get(oi) {
            Some(OutCell::Lit(Lit::Num(n))) => match crate::types::lit_value_in_pub(n, p) {
                Some(v) => Put::Num(rounds.iter().fold(v, |v, (m, g)| v.round_to(*m, *g))),
                None => Put::Any,
            },
            Some(OutCell::Nothing) => Put::Absent,
            _ => Put::Any,
        },
        // A date has no rounding; one put through a rounding is not followed.
        Ty::Date => match row.outs.get(oi) {
            Some(OutCell::Lit(Lit::Date(y, m, d))) if rounds.is_empty() => Put::Num(crate::types::date_ord(*y, *m, *d)),
            Some(OutCell::Nothing) => Put::Absent,
            _ => Put::Any,
        },
        _ => Put::Any,
    }
}

/// Whether what a row puts can stand on coordinate `ci` of `axis`. Something the readings do not
/// follow can stand anywhere.
fn coord_takes(axis: &Axis, ci: usize, put: &Put) -> bool {
    use std::cmp::Ordering::*;
    match (axis, put) {
        (_, Put::Any) => true,
        (Axis::Enum { values }, Put::Word(w)) => values.get(ci) == Some(w),
        (Axis::Bool, Put::Word(w)) => w == if ci == 0 { crate::kw::TRUE } else { crate::kw::FALSE },
        (Axis::Num { coords, .. }, Put::Num(v)) => match coords.get(ci) {
            Some(Coord::Point(p)) => p.cmp_to(*v) == Equal,
            Some(Coord::Open(a, b)) => a.is_none_or(|a| a.cmp_to(*v) == Less) && b.is_none_or(|b| v.cmp_to(b) == Less),
            _ => false,
        },
        (Axis::Num { coords, .. }, Put::Absent) => matches!(coords.get(ci), Some(Coord::Absent)),
        (Axis::Prefix { .. }, _) => true,
        _ => false,
    }
}

/// A table above the one that decides a column, which decides a column that table reads and the
/// table below does not (§15.322): the rows of the first are joined with the rows of this one that
/// write what they read there.
struct Chained<'a> {
    /// The column it decides.
    name: String,
    table: &'a Table,
    reg: TableRegion,
    oi: usize,
    rounds: Vec<(crate::num::RoundMode, Rat)>,
}

/// How many tables above the deciding one are followed, and how many ways a value can arrive along
/// them, before the reading stops and rests on nothing about those rows (§15.322).
const CHAIN_TABLES: usize = 8;
const CHAIN_PATHS: usize = 64;

/// One way a value decided above arrives (§15.322): a row of the table that decides it, joined
/// with a row of each table above that decides a column it reads and the table below does not.
/// Each step is the table — 0 the deciding one, `k + 1` the chain's `k`-th — and the row.
pub(crate) type Path = Vec<(usize, usize)>;

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
    /// The roundings between what the table writes and the column's value (§15.322).
    rounds: Vec<(crate::num::RoundMode, Rat)>,
    /// The tables above the deciding one this reading follows (§15.322).
    chain: Vec<Chained<'a>>,
    /// Columns read above that a table decides and the chain does not hold — past
    /// [`CHAIN_TABLES`], or a table with no region — so no way through one is followed.
    unfollowed: Vec<String>,
    /// The tables that decide a column the deciding table reads and the table below reads too, the
    /// column the box fixes (§15.325). A way is joined through them only where the ways that are not
    /// leave a row with no reason ([`Upstream::paths_through`]); their steps come after the chain's.
    through: Vec<Chained<'a>>,
    /// [`span_of_value`] for a value and one of the table's own columns, worked out once: it does
    /// not hang on the box that asks.
    spans: std::cell::RefCell<BTreeMap<(String, String), Option<CertSpan>>>,
}

impl Upstream<'_> {
    /// The region of a step's table: 0 the deciding one, `k` the chain's `k − 1`-th, and past the
    /// chain the tables a way is joined through (§15.325).
    fn reg_of(&self, k: usize) -> &TableRegion {
        match k {
            0 => &self.reg,
            k if k <= self.chain.len() => &self.chain[k - 1].reg,
            k => &self.through[k - 1 - self.chain.len()].reg,
        }
    }

    /// The table of a step, numbered as [`Upstream::reg_of`] numbers it.
    fn table_of(&self, k: usize) -> &Table {
        match k {
            0 => self.table,
            k if k <= self.chain.len() => self.chain[k - 1].table,
            k => self.through[k - 1 - self.chain.len()].table,
        }
    }

    /// How many steps' tables there are: the deciding one, the chain's, and those a way is joined
    /// through.
    fn steps(&self) -> usize {
        1 + self.chain.len() + self.through.len()
    }

    /// The name of a step's table, as the certificate names it.
    fn table_name(&self, k: usize) -> String {
        self.table_of(k).name.as_ref().map(|n| n.text.clone()).unwrap_or_default()
    }

    /// [`span_of_value`] for what a coordinate of the reading table's axis on this column holds,
    /// kept for the next box that asks. On a column of words the rows are those that write the word;
    /// on one of numbers, those whose number, put through the roundings on the way, lands in the
    /// coordinate (§15.322).
    fn span_of(&self, c: &Checked, held: &Held, col: &str) -> Option<CertSpan> {
        let key = (held.label.clone(), col.to_string());
        if let Some(sp) = self.spans.borrow().get(&key) {
            return sp.clone();
        }
        let sp = match &held.num {
            None => span_of_value(self, c, &|ri| Some(written_value(self.table.rows[ri].outs.get(self.oi), c)? == held.label), col),
            Some((axis, ci)) => span_of_value(
                self,
                c,
                &|ri| match self.put(ri, c) {
                    Put::Any => None,
                    p => Some(coord_takes(axis, *ci, &p)),
                },
                col,
            ),
        };
        self.spans.borrow_mut().insert(key, sp.clone());
        sp
    }

    /// What row `ri` of the deciding table puts into the column.
    fn put(&self, ri: usize, c: &Checked) -> Put {
        put_of(self.table, ri, self.oi, &self.rounds, c)
    }

    /// Every way row `ri` of the deciding table can fire, joined with the rows above that write
    /// what it reads on a column `here` does not have (§15.322), each table's rows in their order
    /// and the columns in the order of the table that reads them. `None` past [`CHAIN_PATHS`], or
    /// where a way runs through a column the chain does not follow.
    fn paths(&self, ri: usize, here: &TableRegion, c: &Checked) -> Option<Vec<Path>> {
        self.paths_from(0, ri, here, c, 0)
    }

    /// The ways of [`Upstream::paths`], each joined besides through the columns the deciding table
    /// reads that `here` reads too and a table decides (§15.325): with a row of that table that
    /// writes a value row `ri` takes there, one step and no further. The box fixes such a column, but
    /// the row that writes it carries what it takes of the other columns, which the box does not: a
    /// table above that reads `band` and `amount` writes `t3` only for `amount` up to 1, and the rows
    /// that write the `band` it takes fire only past 9. The steps of one column come where the column
    /// comes among the deciding table's, as the chain's do. `None` past [`CHAIN_PATHS`], or where a
    /// way runs through a column the chain does not follow.
    fn paths_through(&self, ri: usize, here: &TableRegion, c: &Checked) -> Option<Vec<Path>> {
        let reg = &self.reg;
        let mut acc: Vec<Path> = vec![vec![(0, ri)]];
        for (ua, d) in reg.col_names.iter().enumerate() {
            let alts: Vec<Path> = if here.col_names.contains(d) {
                let Some(j) = self.through.iter().position(|th| th.name == *d) else { continue };
                let th = &self.through[j];
                let mask = reg.masks.get(ri)?.get(ua)?;
                (0..th.table.rows.len())
                    .filter(|&rj| {
                        let put = put_of(th.table, rj, th.oi, &th.rounds, c);
                        (0..reg.axes[ua].len()).any(|x| mask[x] && coord_takes(&reg.axes[ua], x, &put))
                    })
                    .map(|rj| vec![(1 + self.chain.len() + j, rj)])
                    .collect()
            } else {
                if self.unfollowed.contains(d) {
                    return None;
                }
                let Some(j) = self.chain.iter().position(|ch| ch.name == *d) else { continue };
                let ch = &self.chain[j];
                let mask = reg.masks.get(ri)?.get(ua)?;
                let mut alts: Vec<Path> = Vec::new();
                for rj in 0..ch.table.rows.len() {
                    let put = put_of(ch.table, rj, ch.oi, &ch.rounds, c);
                    if !(0..reg.axes[ua].len()).any(|x| mask[x] && coord_takes(&reg.axes[ua], x, &put)) {
                        continue;
                    }
                    alts.extend(self.paths_from(j + 1, rj, here, c, 1)?);
                    if alts.len() > CHAIN_PATHS {
                        return None;
                    }
                }
                alts
            };
            let mut next: Vec<Path> = Vec::with_capacity(acc.len() * alts.len());
            for p in &acc {
                for a in &alts {
                    let mut q = p.clone();
                    q.extend(a.iter().copied());
                    next.push(q);
                }
            }
            if next.len() > CHAIN_PATHS {
                return None;
            }
            acc = next;
        }
        Some(acc)
    }

    fn paths_from(&self, k: usize, ri: usize, here: &TableRegion, c: &Checked, depth: usize) -> Option<Vec<Path>> {
        if depth > CHAIN_TABLES {
            return None;
        }
        let reg = self.reg_of(k);
        let mut acc: Vec<Path> = vec![vec![(k, ri)]];
        for (ua, d) in reg.col_names.iter().enumerate() {
            if here.col_names.contains(d) {
                continue;
            }
            if self.unfollowed.contains(d) {
                return None;
            }
            let Some(j) = self.chain.iter().position(|ch| ch.name == *d) else { continue };
            let ch = &self.chain[j];
            let mask = reg.masks.get(ri)?.get(ua)?;
            let mut alts: Vec<Path> = Vec::new();
            for rj in 0..ch.table.rows.len() {
                let put = put_of(ch.table, rj, ch.oi, &ch.rounds, c);
                if !(0..reg.axes[ua].len()).any(|x| mask[x] && coord_takes(&reg.axes[ua], x, &put)) {
                    continue;
                }
                alts.extend(self.paths_from(j + 1, rj, here, c, depth + 1)?);
                if alts.len() > CHAIN_PATHS {
                    return None;
                }
            }
            let mut next: Vec<Path> = Vec::with_capacity(acc.len() * alts.len());
            for p in &acc {
                for a in &alts {
                    let mut q = p.clone();
                    q.extend(a.iter().copied());
                    next.push(q);
                }
            }
            if next.len() > CHAIN_PATHS {
                return None;
            }
            acc = next;
        }
        Some(acc)
    }
}

/// The one value an output cell writes, when it writes one: a value written as such, or a word
/// no declared name is spelled like (§3.2). A name stands for what it holds when the row fires,
/// which these readings do not follow (`Checked::row_writes` does).
///
/// `none` writes the absent value, the word an optional column's axis has for it (§15.249).
fn written_value<'o>(o: Option<&'o OutCell>, c: &Checked) -> Option<&'o str> {
    match o? {
        OutCell::Lit(Lit::Word(w)) => Some(w),
        OutCell::Name(w) if !c.syms.contains_key(w) => Some(w),
        OutCell::Nothing => Some(crate::kw::NONE),
        _ => None,
    }
}

/// The column of its own table a row passes on into the output column `oi`, when its answer cell
/// names one: the row writes what that column holds where it fires (`Checked::row_writes`), which a
/// re-checker reads off the row's own box on the column's axis (§15.261). `None` for a word, a
/// number, and a name of anything else.
pub(crate) fn passed_column<'t>(t: &'t Table, ri: usize, oi: usize, c: &Checked) -> Option<&'t str> {
    let OutCell::Name(w) = t.rows.get(ri)?.outs.get(oi)? else { return None };
    if !c.syms.contains_key(w) || !matches!(c.ty_of(&t.outputs.get(oi)?.name.text).as_ref().map(Ty::present), Some(Ty::Enum(_) | Ty::Bool)) {
        return None;
    }
    t.inputs.iter().find(|(n, _)| n == w).map(|(n, _)| n.as_str())
}

/// The values a row of the table that decides a column writes into it, said so that a re-checker
/// can settle them: the word its answer cell writes, or, where it passes on one of the table's own
/// columns, the values its cell on that column takes (§15.261). `None` where a re-checker could not
/// work them out from the certificate: a name of anything else writes whatever that name holds.
fn settled_writes(t: &Table, ri: usize, oi: usize, c: &Checked) -> Option<Vec<String>> {
    if passed_column(t, ri, oi, c).is_some() {
        return c.row_writes(t, ri, oi);
    }
    written_value(t.rows.get(ri)?.outs.get(oi), c).map(|w| vec![w.to_string()])
}

/// Whether no row of a table writes this value into its output column `oi`, said so that a
/// re-checker can settle it: every row writes a value word other than this one, or passes on one of
/// the table's own columns where its cell takes no such value. A row that passes on any other name
/// writes whatever the name holds, so a table with one says nothing here (§15.115, §15.253,
/// §15.261).
fn never_writes(t: &Table, oi: usize, v: &str, c: &Checked) -> bool {
    (0..t.rows.len()).all(|ri| settled_writes(t, ri, oi, c).is_some_and(|ws| !ws.iter().any(|w| w == v)))
}

/// Whether no row of the table that decides a column writes this value into it ([`never_writes`]).
fn never_written(up: &Upstream, v: &str, c: &Checked) -> bool {
    never_writes(up.table, up.oi, v, c)
}

/// Every column of `t` that a table above it decides, through the definitions that only name
/// another value (§15.322), each with the tables above that one this reading follows.
fn upstreams<'a>(t: &'a Table, c: &'a Checked, f: &'a RuleFile) -> Vec<Upstream<'a>> {
    let mut out = Vec::new();
    let mine: Vec<&str> = t.inputs.iter().map(|(n, _)| n.as_str()).collect();
    for (ci, (k, _)) in t.inputs.iter().enumerate() {
        let Some(src) = source_of(f, c, t, k) else { continue };
        let Some(reg) = TableRegion::build(src.table, c, f) else { continue };
        let Some(ty) = c.ty_of(k) else { continue };
        // The tables above: each decides a column a table already here reads and `t` does not.
        // One past the cap, or one with no region, is a column no way is followed through.
        let mut chain: Vec<Chained> = Vec::new();
        let mut unfollowed: Vec<String> = Vec::new();
        let mut queue: Vec<&Table> = vec![src.table];
        let mut seen: Vec<*const Table> = vec![src.table as *const Table];
        while let Some(u) = queue.pop() {
            for (d, _) in &u.inputs {
                if mine.contains(&d.as_str()) || chain.iter().any(|ch| ch.name == *d) || unfollowed.contains(d) {
                    continue;
                }
                let Some(s) = source_of(f, c, u, d) else { continue };
                let r = if std::ptr::eq(s.table, t) || chain.len() >= CHAIN_TABLES { None } else { TableRegion::build(s.table, c, f) };
                let Some(r) = r else {
                    unfollowed.push(d.clone());
                    continue;
                };
                if !seen.contains(&(s.table as *const Table)) {
                    seen.push(s.table as *const Table);
                    queue.push(s.table);
                }
                chain.push(Chained { name: d.clone(), table: s.table, reg: r, oi: s.oi, rounds: s.rounds });
            }
        }
        // The tables a way may be joined through: each decides a column the deciding table reads
        // and `t` reads too (§15.325).
        let mut through: Vec<Chained> = Vec::new();
        for (d, _) in &src.table.inputs {
            if !mine.contains(&d.as_str()) || d == k || through.iter().any(|th| th.name == *d) {
                continue;
            }
            let Some(s) = source_of(f, c, src.table, d) else { continue };
            if std::ptr::eq(s.table, t) || std::ptr::eq(s.table, src.table) {
                continue;
            }
            // Only a table a certificate states, so that a re-checker counts the same ways.
            let Some(r) = TableRegion::build(s.table, c, f) else { continue };
            if r.unanalyzable.is_some() || r.axes.is_empty() || s.table.rows.is_empty() {
                continue;
            }
            through.push(Chained { name: d.clone(), table: s.table, reg: r, oi: s.oi, rounds: s.rounds });
        }
        out.push(Upstream { name: k.clone(), ci, table: src.table, reg, oi: src.oi, ty, rounds: src.rounds, chain, unfollowed, through, spans: Default::default() });
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
/// Every reading here keeps at least what an input can come to, so "no producer" really means
/// none, and E102 is never handed a row that is alive (§15.322). This row's cells are read on the
/// axes of the table above with [`cell_meets`]: that table cuts the column at its own boundaries,
/// and a coordinate of it is kept when a value of the cell lies in it, so a coordinate partly
/// inside the cell is kept. A column that table reads and a table further up decides, which this
/// row's table does not read, keeps the values that table writes while this row's cells hold
/// ([`row_on`]). What this row pins the other columns above to narrows what is left
/// ([`narrow_exactly`]). Each axis of the table above is cut at its own rows' cells, so a row of it
/// fires somewhere in what is left exactly when its box meets it — and, in a `policy first` table,
/// when the rows above it taken together do not take all of what they meet ([`fires_in`]). A row
/// that reads a column another table above decides fires only where that table writes the column a
/// value the row takes ([`fed`]).
///
/// A gap's box is asked about one point at a time by [`TableRegion::upstream_dead_at`], which reads
/// only what a certificate can state of a leaf the tables above rule out: one column of words at a
/// time, the rows above one at a time. Every point that reading rules out lies inside a row this one
/// rules out, but not the other way round: a row this one rules out can have points that reading
/// leaves standing, and those are points it cannot rule out, not points an input reaches.
fn upstream_dead(row: &Row, ups: &[Upstream], c: &Checked, t: &Table) -> bool {
    let reads: Vec<RowRead> = ups
        .iter()
        .map(|up| {
            let kcell = row.cells.get(up.ci).filter(|k| !matches!(k, Cell::DontCare)).cloned();
            let mut chain = vec![None; up.chain.len()];
            let on = row_on(&up.reg, row, t, &up.chain, c, &mut chain, 0);
            RowRead { kcell, on, chain }
        })
        .collect();
    ups.iter().enumerate().any(|(k, up)| {
        let Some(kcell) = &reads[k].kcell else { return false };
        let mut allowed = reads[k].on.clone();
        narrow_exactly(&mut allowed, k, ups, &reads, c);
        let d = up.decider();
        !(0..up.table.rows.len()).any(|ri| writes_into(&d, ri, Some(kcell), c) != Some(false) && fires_in(&d, ri, &allowed, true) && fed(ups, k, ri, &allowed, &reads, c))
    })
}

/// A row's cells, read for one column a table above decides (§15.322).
struct RowRead {
    /// The cell the row pins the column to; `None` for any value.
    kcell: Option<Cell>,
    /// The row's cells on the axes of the table that decides the column ([`row_on`]).
    on: Vec<Vec<bool>>,
    /// The same on the axes of each table of the chain above that one, where it was worked out.
    chain: Vec<Option<Vec<Vec<bool>>>>,
}

/// This row's own cells, read on the axes of `reg`, a table above (§15.322): the coordinates
/// [`cell_meets`] keeps for a column the row's table reads, and for a column a table further up
/// `chain` decides, the coordinates holding the values that table writes while these cells hold
/// ([`values_here`]), as far as [`CHAIN_TABLES`] tables up. Every coordinate a point of the row
/// comes to is kept. `memo` holds the reading on each table of the chain once it is worked out.
fn row_on(reg: &TableRegion, row: &Row, t: &Table, chain: &[Chained], c: &Checked, memo: &mut Vec<Option<Vec<Vec<bool>>>>, depth: usize) -> Vec<Vec<bool>> {
    (0..reg.axes.len())
        .map(|ai| {
            let n = &reg.col_names[ai];
            let all = || vec![true; reg.axes[ai].len()];
            if let Some(i) = t.inputs.iter().position(|(m, _)| m == n) {
                return match c.ty_of(n) {
                    Some(ty) => cell_meets(&reg.axes[ai], row.cells.get(i), ty.present(), c),
                    None => all(),
                };
            }
            let Some(j) = chain.iter().position(|ch| ch.name == *n) else { return all() };
            let Some(ty) = c.ty_of(n) else { return all() };
            if memo[j].is_none() {
                if depth >= CHAIN_TABLES {
                    return all();
                }
                let below = row_on(&chain[j].reg, row, t, chain, c, memo, depth + 1);
                memo[j] = Some(below);
            }
            let ch = &chain[j];
            let d = Decider { table: ch.table, reg: &ch.reg, oi: ch.oi, rounds: &ch.rounds, ty };
            values_here(&d, None, memo[j].as_deref().unwrap_or_default(), &reg.axes[ai], c).unwrap_or_else(all)
        })
        .collect()
}

/// Narrow what is left of the axes of the table that decides `ups[k]`'s column by what the row
/// pins the **other** columns above to (§15.114, §15.322).
///
/// Reading one column at a time cannot see that two of them are decided by the same input: each
/// half looks possible on its own while the pair never arrives together. A row naming `small` and
/// `heavy`, where one table calls anything over 10kg large and the other calls anything over 20kg
/// heavy, is dead — and neither column alone says so. Where the table above reads the column the
/// other one decides, its axis for that column keeps only the values the other table writes while
/// the row's cells hold ([`values_here`]): a table that reads a band another table works out from
/// the amount sees, for an amount the row holds to, only the bands that amount comes to. And where
/// the row pins the other column, the columns the two tables share keep only where the other table
/// writes a value the row takes ([`fires_where`]), a number as much as a word.
fn narrow_exactly(allowed: &mut [Vec<bool>], k: usize, ups: &[Upstream], reads: &[RowRead], c: &Checked) {
    let up = &ups[k];
    for (o, other) in ups.iter().enumerate() {
        if o == k {
            continue;
        }
        let d = other.decider();
        let (ocell, oallowed) = (reads[o].kcell.as_ref(), &reads[o].on);
        if let Some(ua) = up.reg.col_names.iter().position(|n| *n == other.name)
            && let Some(here) = values_here(&d, ocell, oallowed, &up.reg.axes[ua], c)
        {
            for (bit, keep) in allowed[ua].iter_mut().zip(here) {
                *bit &= keep;
            }
        }
        let Some(ocell) = ocell else { continue };
        let owhere = fires_where(&d, ocell, oallowed, c);
        for (ua, n) in up.reg.col_names.iter().enumerate() {
            let Some(oa) = other.reg.col_names.iter().position(|m| m == n) else { continue };
            for (bit, keep) in allowed[ua].iter_mut().zip(carry(&other.reg.axes[oa], &owhere[oa], &up.reg.axes[ua])) {
                *bit &= keep;
            }
        }
    }
}

/// Whether row `ri` of the table that decides `ups[k]`'s column can fire in `allowed` with the
/// columns it reads that another table above decides holding values that table writes there
/// (§15.322). For each such column, some row of the table that decides it has to write a value
/// the row's box takes on the column — and the row of this table pins it to — and fire where both
/// rows' boxes meet on the columns the two tables share and the row's cells hold.
fn fed(ups: &[Upstream], k: usize, ri: usize, allowed: &[Vec<bool>], reads: &[RowRead], c: &Checked) -> bool {
    let up = &ups[k];
    let reg = &up.reg;
    let live: Vec<Vec<bool>> = (0..reg.axes.len()).map(|a| (0..reg.axes[a].len()).map(|x| reg.masks[ri][a][x] && allowed[a][x]).collect()).collect();
    for (ua, n) in reg.col_names.iter().enumerate() {
        let (d, ocell, oallowed): (Decider, Option<&Cell>, &[Vec<bool>]) = if let Some(o) = ups.iter().position(|o| o.name == *n) {
            if o == k {
                continue;
            }
            (ups[o].decider(), reads[o].kcell.as_ref(), &reads[o].on)
        } else if let Some(j) = up.chain.iter().position(|ch| ch.name == *n)
            && let Some(on) = reads[k].chain[j].as_deref()
            && let Some(ty) = c.ty_of(n)
        {
            let ch = &up.chain[j];
            (Decider { table: ch.table, reg: &ch.reg, oi: ch.oi, rounds: &ch.rounds, ty }, None, on)
        } else {
            continue;
        };
        if oallowed.len() != d.reg.axes.len() {
            continue;
        }
        // Where the row's cells hold on the deciding table's axes, inside this row's box on the
        // columns the two tables share.
        let mut oa: Vec<Vec<bool>> = oallowed.to_vec();
        for (da, m) in d.reg.col_names.iter().enumerate() {
            if let Some(ub) = reg.col_names.iter().position(|x| x == m) {
                for (bit, keep) in oa[da].iter_mut().zip(carry(&reg.axes[ub], &live[ub], &d.reg.axes[da])) {
                    *bit &= keep;
                }
            }
        }
        let some = (0..d.table.rows.len()).any(|oj| {
            writes_into(&d, oj, ocell, c) != Some(false)
                && lands(&d, oj, ocell, &reg.axes[ua], c).is_none_or(|m| m.iter().zip(&live[ua]).any(|(x, y)| *x && *y))
                && fires_in(&d, oj, &oa, true)
        });
        if !some {
            return false;
        }
    }
    true
}

/// The coordinates of `axis` — an axis, in another table, for the column `d` decides — the values
/// row `oj` of `d`'s table writes there lie in, each value `ocell` takes (any value, without one).
/// `None` where it writes something these readings do not follow, or a value the axis cannot place.
fn lands(d: &Decider, oj: usize, ocell: Option<&Cell>, axis: &Axis, c: &Checked) -> Option<Vec<bool>> {
    use crate::eval::{Val, cell_matches};
    let mut out = vec![false; axis.len()];
    match (c.row_writes(d.table, oj, d.oi), d.put(oj, c)) {
        (Some(vals), _) => {
            for v in vals {
                if !ocell.is_none_or(|cell| cell_matches(c, cell, &Val::Enum(v.clone()), &d.ty)) {
                    continue;
                }
                match axis {
                    Axis::Enum { values } => values.iter().zip(out.iter_mut()).filter(|(w, _)| **w == v).for_each(|(_, o)| *o = true),
                    Axis::Bool if v == crate::kw::TRUE => out[0] = true,
                    Axis::Bool if v == crate::kw::FALSE => out[1] = true,
                    _ => return None,
                }
            }
        }
        (None, put @ (Put::Num(_) | Put::Absent)) => {
            if !matches!(axis, Axis::Num { .. }) {
                return None;
            }
            for (x, o) in out.iter_mut().enumerate() {
                *o = coord_takes(axis, x, &put);
            }
        }
        _ => return None,
    }
    Some(out)
}

/// A table that decides a column, as the readings of the rows above take it: the deciding table of
/// an [`Upstream`], or one of the tables above that one its chain follows.
struct Decider<'r> {
    table: &'r Table,
    reg: &'r TableRegion,
    oi: usize,
    rounds: &'r [(crate::num::RoundMode, Rat)],
    ty: Ty,
}

impl Decider<'_> {
    /// What row `ri` puts into the column.
    fn put(&self, ri: usize, c: &Checked) -> Put {
        put_of(self.table, ri, self.oi, self.rounds, c)
    }
}

impl Upstream<'_> {
    /// The deciding table, as [`Decider`].
    fn decider(&self) -> Decider<'_> {
        Decider { table: self.table, reg: &self.reg, oi: self.oi, rounds: &self.rounds, ty: self.ty.clone() }
    }
}

/// The shared half of [`TableRegion::upstream_dead_at`]: given, per upstream column, what the
/// caller fixes, decide whether **no** row of the table above can produce a value the caller
/// allows. This is what a certificate can state of a leaf the tables above rule out, and the
/// re-checkers read it the same way.
///
/// `restrict` returns `None` for a column the caller leaves free, and otherwise the cell it pins
/// the column to together with that same restriction read on the upstream table's axes. Before the
/// rows are walked, that restriction is narrowed by what the caller pins the **other** columns
/// above to, which is the only way a pair of them decided by one input can be seen to exclude each
/// other (§15.114). A row of the table above that writes something these readings do not follow is
/// assumed to produce the value, and one that is not the first in a `policy first` table to take
/// what is left is held back only by a single row above it.
fn upstream_blocked(ups: &[Upstream], c: &Checked, restrict: &dyn Fn(&Upstream) -> Option<(Option<Cell>, Vec<Vec<bool>>)>) -> bool {
    for up in ups {
        let Some((Some(kcell), mut allowed)) = restrict(up) else { continue };
        narrow_by_siblings(&mut allowed, up, ups, c, restrict);
        let d = up.decider();
        let producible = (0..up.table.rows.len()).any(|ri| match writes_into(&d, ri, Some(&kcell), c) {
            Some(false) => false,
            Some(true) => fires_in(&d, ri, &allowed, false),
            None => true,
        });
        if !producible {
            return true;
        }
    }
    false
}

/// Whether row `ri` of the table that decides a column writes there a value `kcell` takes (any
/// value, without one): a value it writes, or what a name it writes holds — the values its own
/// cell lets through when the name is one of the table's columns, every value of its type
/// otherwise. The type check reads the cell the same way, so the values E102 says nothing writes
/// are the values this says no row produces (`Checked::row_writes`, §15.208). A number is what
/// the row writes, put through the roundings on the way (§15.322). `None` where it writes
/// something these readings do not follow, which may be any value.
fn writes_into(d: &Decider, ri: usize, kcell: Option<&Cell>, c: &Checked) -> Option<bool> {
    use crate::eval::{Val, cell_matches};
    let takes = |v: &Val| kcell.is_none_or(|cell| cell_matches(c, cell, v, &d.ty));
    match (c.row_writes(d.table, ri, d.oi), d.put(ri, c)) {
        (Some(vals), _) => Some(vals.into_iter().any(|v| takes(&Val::Enum(v)))),
        // A date is put as its day number (§15.325), and a cell takes it as the date it is: a
        // cell of one date compares dates, not a number with a date.
        (None, Put::Num(v)) if *d.ty.present() == Ty::Date => {
            let (y, m, day) = crate::types::ord_to_date(v);
            Some(takes(&Val::Date(y, m, day)))
        }
        (None, Put::Num(v)) => Some(takes(&Val::Num(v))),
        (None, Put::Absent) => Some(takes(&Val::Enum(crate::kw::NONE.to_string()))),
        (None, Put::Word(_) | Put::Any) => None,
    }
}

/// How many steps the cover of what is left of a row's box by the rows above it in a `policy
/// first` table may take (§15.322). Past them the row is taken to fire, which is the safe side.
const FIRST_COVER_STEPS: i64 = 4096;

/// Whether row `ri` of a table that decides a column fires somewhere in `allowed`, a box on that
/// table's axes (§15.322). What is left of the row's box once `allowed` is imposed has to be not
/// empty, and in a `policy first` table the rows above it must not take all of what is left: with
/// `exact`, the rows above taken together ([`masks_cover`]), as far as [`FIRST_COVER_STEPS`] lets
/// the cover be worked out; otherwise any one of them alone, which is what a certificate can state
/// of a leaf the tables above rule out ([`span_of_value`] reads the same).
fn fires_in(d: &Decider, ri: usize, allowed: &[Vec<bool>], exact: bool) -> bool {
    let reg = d.reg;
    let live: Vec<Vec<bool>> = (0..reg.axes.len()).map(|a| (0..reg.axes[a].len()).map(|x| reg.masks[ri][a][x] && allowed[a][x]).collect()).collect();
    if live.iter().any(|m| !m.iter().any(|x| *x)) {
        return false;
    }
    if d.table.policy != Policy::TopDown {
        return true;
    }
    let one_takes_all = (0..ri).any(|e| (0..reg.axes.len()).all(|a| (0..reg.axes[a].len()).all(|x| !live[a][x] || reg.masks[e][a][x])));
    if one_takes_all || !exact {
        return !one_takes_all;
    }
    // A column the region cannot read has boxes that say nothing of it.
    if reg.unanalyzable.is_some() {
        return true;
    }
    let over: Vec<&Vec<Vec<bool>>> = (0..ri).map(|e| &reg.masks[e]).collect();
    let mut left = FIRST_COVER_STEPS;
    masks_cover(&live, &over, &mut left) != Some(true)
}

/// Whether the box `part` — a set of coordinates per axis — lies inside the boxes `over` taken
/// together: what lies outside the first of them has to lie inside the rest. `None` past `budget`
/// steps.
fn masks_cover(part: &[Vec<bool>], over: &[&Vec<Vec<bool>>], budget: &mut i64) -> Option<bool> {
    *budget -= 1;
    if *budget < 0 {
        return None;
    }
    if part.iter().any(|m| !m.iter().any(|x| *x)) {
        return Some(true);
    }
    let Some((b, rest)) = over.split_first() else { return Some(false) };
    if part.iter().zip(b.iter()).any(|(p, q)| !p.iter().zip(q).any(|(x, y)| *x && *y)) {
        return masks_cover(part, rest, budget);
    }
    let mut inside: Vec<Vec<bool>> = part.to_vec();
    for k in 0..part.len() {
        let out: Vec<bool> = inside[k].iter().zip(&b[k]).map(|(x, y)| *x && !*y).collect();
        if out.iter().any(|x| *x) {
            let mut piece = inside.clone();
            piece[k] = out;
            if !masks_cover(&piece, rest, budget)? {
                return Some(false);
            }
        }
        inside[k] = inside[k].iter().zip(&b[k]).map(|(x, y)| *x && *y).collect();
    }
    Some(true)
}

/// The coordinates of `axis` — an axis, in another table, for the column `d` decides — that the
/// column can hold here (§15.322): the values the rows of `d`'s table write ([`lands`]), each row
/// that writes a value `ocell` takes and fires somewhere in `oallowed` ([`fires_in`], read exactly).
/// `None` where such a row writes something these readings do not follow, or a value the axis
/// cannot place.
fn values_here(d: &Decider, ocell: Option<&Cell>, oallowed: &[Vec<bool>], axis: &Axis, c: &Checked) -> Option<Vec<bool>> {
    if oallowed.len() != d.reg.axes.len() {
        return None;
    }
    let mut out = vec![false; axis.len()];
    for ri in 0..d.table.rows.len() {
        if writes_into(d, ri, ocell, c) == Some(false) || !fires_in(d, ri, oallowed, true) {
            continue;
        }
        for (o, x) in out.iter_mut().zip(lands(d, ri, ocell, axis, c)?) {
            *o |= x;
        }
    }
    Some(out)
}

/// The closed ends of a coordinate: a point is itself, an open interval is pulled in by one
/// step on each side it has.
fn coord_ends(cd: &Coord, step: Rat) -> Ival {
    match cd {
        // No number at all: `carry` meets it with the other axis's absent value only.
        Coord::Absent => (None, None),
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
                // The absent value meets the absent value and nothing else (§15.201).
                let absent = |c: &Coord| matches!(c, Coord::Absent);
                if absent(t) {
                    return fc.iter().zip(mask).any(|(f, &m)| m && absent(f));
                }
                let te = coord_ends(t, *ts);
                fc.iter().zip(mask).any(|(f, &m)| m && !absent(f) && ivals_meet(coord_ends(f, *fs), te))
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
        let v = written_value(up.table.rows[ri].outs.get(up.oi), c)?.to_string();
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

/// Where on its own axes the table `d` stands for decides a value `ocell` takes while `oallowed`
/// holds (§15.322): what is left of the box of each row that may write such a value and fires there
/// ([`fires_in`], read exactly), taken together axis by axis. Where the column holds such a value,
/// the columns of that table lie inside it; a number the row writes counts as a value as much as a
/// word, and a row that writes something these readings do not follow counts wherever it fires.
fn fires_where(d: &Decider, ocell: &Cell, oallowed: &[Vec<bool>], c: &Checked) -> Vec<Vec<bool>> {
    let reg = d.reg;
    let mut acc: Vec<Vec<bool>> = reg.axes.iter().map(|a| vec![false; a.len()]).collect();
    for ri in 0..d.table.rows.len() {
        if writes_into(d, ri, Some(ocell), c) == Some(false) || !fires_in(d, ri, oallowed, true) {
            continue;
        }
        for (a, m) in acc.iter_mut().enumerate() {
            for (x, bit) in m.iter_mut().enumerate() {
                *bit |= reg.masks[ri][a][x] && oallowed[a][x];
            }
        }
    }
    acc
}

/// Narrow one column's restriction by what the **other** columns above are pinned to, as
/// [`upstream_blocked`] reads it.
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

/// What a point holds on a column a table above decides, as [`TableRegion::held_above`] reads it:
/// the word, or the label of a coordinate of numbers with the axis it is on (§15.322).
struct Held {
    label: String,
    num: Option<(Axis, usize)>,
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
/// `writes` says whether row `ri` writes the value, `None` where the row writes something these
/// readings cannot follow, and then there is no span.
fn span_of_value(up: &Upstream, _c: &Checked, writes: &dyn Fn(usize) -> Option<bool>, col: &str) -> Option<CertSpan> {
    let ai = up.reg.col_names.iter().position(|n| n == col)?;
    let mut picked: Vec<usize> = Vec::new();
    for ri in 0..up.table.rows.len() {
        if writes(ri)? {
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

/// Which coordinates of an axis one literal selects: the enum value or the group it names, the
/// truth value, the point a number or a date is.
fn lit_mask(axis: &Axis, l: &Lit, ty: &Ty, c: &Checked) -> Vec<bool> {
    let mut v = vec![false; axis.len()];
    match (axis, l) {
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
    }
    v
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
            // `none` matches only the first coordinate of an optional axis: the value `none` of
            // an enum's, the absent value of a number's or a date's (§15.201).
            Some(Cell::Nothing) => {
                if matches!(axis, Axis::Enum { values } if values.first().map(|s| s.as_str()) == Some(crate::kw::NONE))
                    || matches!(axis, Axis::Num { coords, .. } if matches!(coords.first(), Some(Coord::Absent)))
                {
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
            Some(Cell::Lit(l)) => v = lit_mask(axis, l, ty, c),
            // A set is its members taken together, on any axis — a column of numbers as much as
            // an enum: `100, 200` used to take no coordinate at all there, so the row was
            // unreachable (E102) and the table short of it.
            Some(Cell::Set(ls)) | Some(Cell::Not(ls)) => {
                for l in ls {
                    for (x, m) in v.iter_mut().zip(lit_mask(axis, l, ty, c)) {
                        *x |= m;
                    }
                }
                if matches!(cell, Some(Cell::Not(_))) {
                    v.iter_mut().for_each(|x| *x = !*x);
                }
            }
            // The values a comparison on an ordered enum covers (§15.210). `none`, the absent
            // value of an optional column, is no value of the enum and passes no comparison.
            Some(Cell::Ranked { values, .. }) => {
                if let Axis::Enum { values: on } = axis {
                    for (i, val) in on.iter().enumerate() {
                        v[i] = values.contains(val);
                    }
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

/// The first multiple of `step` past `x`: above it with `up`, below it otherwise. `None` where
/// the arithmetic overflows.
fn step_past(x: Rat, step: Rat, up: bool) -> Option<Rat> {
    let q = x.checked_div(step)?;
    let k = if up { q.num.div_euclid(q.den).checked_add(1)? } else { (-((-q.num).div_euclid(q.den))).checked_sub(1)? };
    step.checked_mul(Rat::int(k))
}

/// Which coordinates of an axis **another table** cut for the same column a cell meets: those
/// with some value of the cell in them (§15.322).
///
/// A table's own axes are cut at its own cells, so there a coordinate lies wholly inside a cell
/// or wholly outside it, and [`cell_mask`] reads it exactly. Another table cuts the column at its
/// own boundaries, and one of its coordinates can lie partly inside the cell: `>50JPY <=80JPY`
/// against the coordinate between 50JPY and 300JPY of a table cut at 50JPY. [`cell_mask`] drops
/// such a coordinate, since not all of it is inside the cell; this keeps it, since a value of the
/// cell lies in it. It is the reading for saying what cannot happen, which may be said only of a
/// coordinate no value of the cell lies in. On a coordinate of numbers the values are those on the
/// axis's step, which every value of the column is a multiple of; an end of the cell between two
/// of them is read as it falls. An enum's and a truth value's coordinates are values, which a
/// cell takes or not, so there the reading is [`cell_mask`]'s.
fn cell_meets(axis: &Axis, cell: Option<&Cell>, ty: &Ty, c: &Checked) -> Vec<bool> {
    use std::cmp::Ordering::*;
    let n = axis.len();
    let all = vec![true; n];
    let cell = match cell {
        None | Some(Cell::DontCare) => return all,
        Some(x) => x,
    };
    match axis {
        Axis::Enum { .. } | Axis::Bool => cell_mask(axis, Some(cell), ty, c),
        // A class holds the strings whose longest pattern of the axis is its own (the last: no
        // pattern at all). `starts_with p` meets a class whose pattern extends `p` — the pattern
        // itself is such a string — and the class of the longest pattern `p` extends, which `p`
        // itself is in; the last class when no pattern is a start of `p`.
        Axis::Prefix { patterns } => {
            let Cell::Prefix(ps) = cell else { return all };
            let mut v = vec![false; n];
            for p in ps {
                let under = patterns.iter().enumerate().filter(|(_, q)| p.starts_with(q.as_str())).max_by_key(|(_, q)| q.len()).map(|(i, _)| i);
                for (i, q) in patterns.iter().enumerate() {
                    if q.starts_with(p.as_str()) || under == Some(i) {
                        v[i] = true;
                    }
                }
                if under.is_none() {
                    v[patterns.len()] = true;
                }
            }
            v
        }
        Axis::Num { coords, step, .. } => {
            // A coordinate's values as a closed interval on the step: a point is itself, an open
            // interval runs from the first multiple of the step past its lower end to the last
            // one short of its upper end.
            let step = *step;
            let ends = |cd: &Coord| -> Option<Ival> {
                match cd {
                    Coord::Absent => None,
                    Coord::Point(p) => Some((Some(*p), Some(*p))),
                    Coord::Open(a, b) => Some((a.and_then(|a| step_past(a, step, true)), b.and_then(|b| step_past(b, step, false)))),
                }
            };
            // Whether a value of the coordinate equals `x`.
            let holds_value = |cd: &Coord, x: Rat| match cd {
                Coord::Absent => false,
                Coord::Point(p) => p.cmp_to(x) == Equal,
                Coord::Open(a, b) => a.is_none_or(|a| a.cmp_to(x) == Less) && b.is_none_or(|b| x.cmp_to(b) == Less),
            };
            let lit_meets = |l: &Lit| -> Option<Vec<bool>> {
                let x = lit_rat(l, ty)?;
                Some(coords.iter().map(|cd| holds_value(cd, x)).collect())
            };
            match cell {
                Cell::Nothing => coords.iter().map(|cd| matches!(cd, Coord::Absent)).collect(),
                // A literal that is not a number of the column's type takes nothing on its own
                // axis either; every coordinate is kept rather than one dropped wrongly.
                Cell::Lit(l) => lit_meets(l).unwrap_or(all),
                Cell::Set(ls) => {
                    let mut v = vec![false; n];
                    for l in ls {
                        let Some(m) = lit_meets(l) else { return all };
                        for (x, y) in v.iter_mut().zip(m) {
                            *x |= y;
                        }
                    }
                    v
                }
                // Some value of the coordinate is none of the set's: the absent value is none of
                // them, a point is itself, and an interval is unless every value on the step in
                // it is a member.
                Cell::Not(ls) => {
                    let Some(xs) = ls.iter().map(|l| lit_rat(l, ty)).collect::<Option<Vec<Rat>>>() else { return all };
                    let member = |v: Rat| xs.iter().any(|x| x.cmp_to(v) == Equal);
                    coords
                        .iter()
                        .map(|cd| match (cd, ends(cd)) {
                            (Coord::Absent, _) => true,
                            (Coord::Point(p), _) => !member(*p),
                            (_, Some((Some(lo), Some(hi)))) => {
                                let mut v = lo;
                                let mut seen = 0;
                                while v.cmp_to(hi) != Greater {
                                    // More values than members: one of them is none of the set's.
                                    if !member(v) || seen >= xs.len() {
                                        return true;
                                    }
                                    v = v.add(step);
                                    seen += 1;
                                }
                                false
                            }
                            _ => true,
                        })
                        .collect()
                }
                // The values of the comparisons form one interval. A coordinate meets it when its
                // first value is under every upper end and its last value over every lower end:
                // with both ends on the step, a value of both lies between the two.
                Cell::Cmp(cs) => {
                    let Some(bs) = cs.iter().map(|(op, l)| Some((*op, lit_rat(l, ty)?))).collect::<Option<Vec<(CmpOp, Rat)>>>() else { return all };
                    coords
                        .iter()
                        .map(|cd| match ends(cd) {
                            None => false,
                            Some((lo, hi)) => bs.iter().all(|&(op, b)| match op {
                                CmpOp::Le | CmpOp::Lt => lo.is_none_or(|v| cmp_holds(op, v, b)),
                                CmpOp::Ge | CmpOp::Gt => hi.is_none_or(|v| cmp_holds(op, v, b)),
                            }),
                        })
                        .collect()
                }
                Cell::Ranked { .. } | Cell::Prefix(_) | Cell::DontCare => all,
            }
        }
    }
}

/// The truth value a `define` of one comparison always takes over the values its two sides come
/// to, when it always takes one (§15.196): `made <= 2027-03-31` with `made` declared up to the end
/// of 2026 is always true, and `a_due <= b_due` with both dates held to one day each is true or
/// false whatever else the input is. The sides are read as `Checked::interval` reads any value —
/// the inputs' ranges, what the derives and other defines come to, the values a table above
/// writes — and a date literal as its day number. An end a range does not write is read as the end
/// of the type, as E108 reads it (§15.299): `a >= 0` over `a` written `>=0` alone is always true.
/// `None` when the comparison can go either way, or a side has no interval.
pub(crate) fn truth_of_expr(e: &Expr, chk: &Checked) -> Option<bool> {
    let Expr::Bin(l, op, r, _) = e else { return None };
    let ty = |e: &Expr| match e {
        Expr::Name(n, _) => chk.ty_of(n),
        _ => None,
    };
    let want = ty(l).or_else(|| ty(r))?;
    let side = |e: &Expr| -> Option<(Rat, Rat)> {
        match e {
            Expr::Lit(Lit::Date(y, m, d), _) => {
                let v = crate::types::date_ord(*y, *m, *d);
                Some((v, v))
            }
            _ => chk.reach(e, &want),
        }
    };
    let ((al, ah), (bl, bh)) = (side(l)?, side(r)?);
    use std::cmp::Ordering::*;
    let le = |x: Rat, y: Rat| x.cmp_to(y) != Greater;
    let lt = |x: Rat, y: Rat| x.cmp_to(y) == Less;
    match op {
        BinOp::Le if le(ah, bl) => Some(true),
        BinOp::Le if lt(bh, al) => Some(false),
        BinOp::Lt if lt(ah, bl) => Some(true),
        BinOp::Lt if le(bh, al) => Some(false),
        BinOp::Ge if le(bh, al) => Some(true),
        BinOp::Ge if lt(ah, bl) => Some(false),
        BinOp::Gt if lt(bh, al) => Some(true),
        BinOp::Gt if le(ah, bl) => Some(false),
        // `=` is left to the comparison's own values: the two re-checkers read the four
        // orderings only, and a truth value no one can check is not one to rest on.
        _ => None,
    }
}

/// A number as the value of input `name`: a date's ordinal as the date, any other number as it is.
fn as_input_val(c: &Checked, name: &str, v: Rat) -> crate::eval::Val {
    if c.ty_of(name).is_some_and(|t| *t.present() == Ty::Date) {
        let (y, m, d) = crate::types::ord_to_date(v);
        crate::eval::Val::Date(y, m, d)
    } else {
        crate::eval::Val::Num(v)
    }
}

/// The closed interval of coordinate `ci` of a numeric axis: a point is itself, an open interval
/// is pulled in by one step on each side it has, and a side it does not have is the end of what
/// the name is read in otherwise ([`Checked::wide_range`]).
fn closed_coord(axis: &Axis, ci: usize, name: &str, c: &Checked) -> Option<(Rat, Rat)> {
    let Axis::Num { coords, step, .. } = axis else { return None };
    let (lo, hi) = match coords.get(ci)? {
        Coord::Absent => return None,
        cd => coord_ends(cd, *step),
    };
    let wide = || c.wide_range(name);
    Some((match lo {
        Some(l) => l,
        None => wide()?.0,
    }, match hi {
        Some(h) => h,
        None => wide()?.1,
    }))
}

/// Every [`NarrowCut`] of a table (§15.322): for each axis of a `define` — a number, or a truth
/// value of one comparison — the numeric axes of what it reads, held to each combination of their
/// coordinates while there are at most [`NARROW_COMBOS`], and otherwise one at a time; a
/// combination is kept where it leaves out a coordinate the define's reach over the whole ranges
/// does not. A coordinate is left out where the define's interval does not reach it, or where no
/// value of the grid its values sit on ([`grid_of`]) lies in it inside that interval — with nothing
/// held, too, where the table reads none of what the define reads. One the interval over the whole
/// of the held coordinates leaves in is asked again over pieces of one held coordinate
/// ([`split_out`]): an expression that reads an input twice comes to a narrower interval over a
/// narrower piece.
#[allow(clippy::too_many_arguments)]
fn narrow_cuts(
    axes: &[Axis],
    col_names: &[String],
    exprs: &BTreeMap<String, Expr>,
    defines: &BTreeMap<String, Expr>,
    define_reach: &[Option<Ival>],
    define_truth: &[Option<bool>],
    c: &Checked,
    f: &RuleFile,
) -> Vec<NarrowCut> {
    let mut out = Vec::new();
    let mut left = NARROW_SPLITS;
    // The values the rule works out, which a grid follows through by name.
    let computed: BTreeMap<String, Expr> =
        defines.iter().chain(exprs.iter()).filter(|(n, _)| f.open_value(n).is_none()).map(|(n, x)| (n.clone(), x.clone())).collect();
    let steps = input_steps(c, f);
    // The values worked out again from their own expressions while the names of `held` are held:
    // every one, other than `name` and the held names, that reads a held name through what it reads.
    let inline_of = |name: &str, held: &BTreeSet<&str>| -> BTreeMap<String, Expr> {
        defines
            .iter()
            .chain(exprs.iter())
            .filter(|(n, _)| n.as_str() != name && !held.contains(n.as_str()) && f.open_value(n).is_none())
            .filter(|(n, _)| {
                let mut r = BTreeSet::new();
                reads_through(n, exprs, defines, &mut r);
                r.iter().any(|m| held.contains(m.as_str()))
            })
            .map(|(n, x)| (n.clone(), x.clone()))
            .collect()
    };
    for i in 0..axes.len() {
        let name = &col_names[i];
        let Some(e) = defines.get(name) else { continue };
        if f.open_value(name).is_some() {
            continue;
        }
        let numeric = matches!(axes[i], Axis::Num { date: false, .. });
        let truth = matches!(axes[i], Axis::Bool) && matches!(e, Expr::Bin(_, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt, _, _));
        if !numeric && !truth {
            continue;
        }
        let Some(ty) = c.ty_of(name) else { continue };
        let mut reads = BTreeSet::new();
        reads_to_columns(name, exprs, defines, col_names, &mut reads, &mut BTreeSet::new());
        let by_axes: Vec<usize> = (0..axes.len())
            .filter(|&j| j != i && reads.contains(&col_names[j]) && matches!(axes[j], Axis::Num { date: false, .. }))
            .collect();
        // What the define comes to and the grid it sits on, by the intervals held. Every coordinate
        // asks the same pieces of an input on its way down, so each is worked out once. What is
        // inlined follows from the names held, so the intervals alone say what was asked.
        type Worked = (Option<(Rat, Rat)>, Option<Option<(Rat, Rat)>>);
        let worked: std::cell::RefCell<std::collections::HashMap<Vec<(String, (Rat, Rat))>, Worked>> = Default::default();
        // What leaves coordinate `cd` out with the names of `over` held: it lies outside the interval
        // the define comes to, or holds no value of the grid it sits on there inside it (§15.325).
        // A call the rule turns away at the define's range has no value it reads (§15.313).
        let reach_cut = |cd: &Coord, over: &[(String, (Rat, Rat))], inline: &BTreeMap<String, Expr>| -> bool {
            let known = worked.borrow().get(over).map(|w| w.0);
            let iv = known.unwrap_or_else(|| {
                let iv = narrow_iv(c, e, ty.present(), over, inline, 0);
                worked.borrow_mut().insert(over.to_vec(), (iv, None));
                iv
            });
            iv.is_some_and(|iv| {
                let iv = within_accepts(iv, c.accepts.get(name));
                iv.0.cmp_to(iv.1) == std::cmp::Ordering::Greater || {
                    let known = worked.borrow().get(over).and_then(|w| w.1);
                    let grid = known.unwrap_or_else(|| {
                        let g = held_grid(c, e, ty.present(), over, inline, &computed, &steps, 0);
                        if let Some(w) = worked.borrow_mut().get_mut(over) {
                            w.1 = Some(g);
                        }
                        g
                    });
                    coord_out(cd, iv, grid)
                }
            })
        };
        // The inputs the define reads that the table has no axis for, each with the interval the
        // int64 claim reads it in and the step it is stored at: a fact can cut one of them into
        // pieces over the whole of that interval (§15.325).
        let free_inputs: Vec<(String, (Rat, Rat), Rat)> = {
            let mut r = BTreeSet::new();
            reads_through(name, exprs, defines, &mut r);
            r.into_iter()
                .filter(|n| !col_names.contains(n) && !computed.contains_key(n))
                .filter_map(|n| {
                    let st = *steps.get(&n)?;
                    let span = c.wide_range(&n)?;
                    Some((n, span, st))
                })
                .collect()
        };
        // Coordinate `x` cut out over pieces of one of `free_inputs`, the axes of `over` held as they
        // are: the input and where its pieces after the first start.
        let input_pieces = |over: &[(String, (Rat, Rat))], x: usize, left: &mut usize| -> Option<(String, Vec<Rat>)> {
            let Axis::Num { coords, .. } = &axes[i] else { return None };
            for (n, (lo, hi), st) in &free_inputs {
                let mut held: BTreeSet<&str> = over.iter().map(|(m, _)| m.as_str()).collect();
                held.insert(n.as_str());
                let inline = inline_of(name, &held);
                let at = |a: Rat, b: Rat| -> bool {
                    let mut o = over.to_vec();
                    o.push((n.clone(), (a, b)));
                    reach_cut(&coords[x], &o, &inline)
                };
                if let Some(cs) = split_out(*lo, *hi, *st, left, &at) {
                    return Some((n.clone(), cs));
                }
            }
            None
        };
        let before = |x: usize| -> bool {
            let Axis::Num { coords, .. } = &axes[i] else { return false };
            let Some(cd) = coords.get(x) else { return false };
            define_reach.get(i).copied().flatten().is_some_and(|(rl, rh)| match cd {
                Coord::Point(v) => rl.is_some_and(|l| v.cmp_to(l) == std::cmp::Ordering::Less) || rh.is_some_and(|h| v.cmp_to(h) == std::cmp::Ordering::Greater),
                Coord::Open(a, b) => {
                    matches!((b, rl), (Some(b), Some(l)) if b.cmp_to(l) != std::cmp::Ordering::Greater)
                        || matches!((a, rh), (Some(a), Some(h)) if a.cmp_to(h) != std::cmp::Ordering::Less)
                }
                Coord::Absent => false,
            })
        };
        if by_axes.is_empty() {
            // Nothing the table reads holds the define: the coordinates inside its reach no value on
            // its grid takes, and those it leaves out over pieces of an input it reads (§15.325).
            let (true, Axis::Num { coords, .. }) = (numeric, &axes[i]) else { continue };
            let none = BTreeMap::new();
            let cut: Vec<bool> = coords.iter().enumerate().map(|(x, cd)| !before(x) && reach_cut(cd, &[], &none)).collect();
            if cut.iter().any(|x| *x) {
                out.push(NarrowCut { axis: i, by: Vec::new(), out: cut.clone(), cuts: None });
            }
            let mut pieces: Vec<(String, Vec<bool>, Vec<Rat>)> = Vec::new();
            for x in (0..coords.len()).filter(|&x| !cut[x] && !before(x) && !matches!(coords[x], Coord::Absent)) {
                if left == 0 {
                    break;
                }
                let Some((n, cs)) = input_pieces(&[], x, &mut left) else { continue };
                match pieces.iter_mut().find(|p| p.0 == n) {
                    Some(p) => {
                        p.1[x] = true;
                        for v in cs {
                            if !p.2.iter().any(|w| w.cmp_to(v) == std::cmp::Ordering::Equal) {
                                p.2.push(v);
                            }
                        }
                        p.2.sort_by(|a, b| a.cmp_to(*b));
                    }
                    None => {
                        let mut m = vec![false; coords.len()];
                        m[x] = true;
                        pieces.push((n, m, cs));
                    }
                }
            }
            for (n, m, cs) in pieces {
                out.push(NarrowCut { axis: i, by: Vec::new(), out: m, cuts: Some((CutOn::Input(n), cs)) });
            }
            continue;
        }
        let coords_of = |j: usize| -> Vec<usize> {
            let Axis::Num { coords, .. } = &axes[j] else { return Vec::new() };
            (0..coords.len()).filter(|&x| !matches!(coords[x], Coord::Absent)).collect()
        };
        let total = by_axes.iter().try_fold(1usize, |acc, &j| acc.checked_mul(coords_of(j).len()).filter(|n| *n <= NARROW_COMBOS));
        let combos: Vec<Vec<(usize, usize)>> = match total {
            Some(_) => by_axes.iter().fold(vec![Vec::new()], |acc, &j| {
                let mut next = Vec::new();
                for p in &acc {
                    for x in coords_of(j) {
                        let mut q = p.clone();
                        q.push((j, x));
                        next.push(q);
                    }
                }
                next
            }),
            None => by_axes.iter().flat_map(|&j| coords_of(j).into_iter().map(move |x| vec![(j, x)])).collect(),
        };
        for by in combos {
            let Some(over) = by.iter().map(|&(j, x)| Some((col_names[j].clone(), closed_coord(&axes[j], x, &col_names[j], c)?))).collect::<Option<Vec<_>>>() else {
                continue;
            };
            // The values that read a held axis are worked out again from their expressions.
            let held: BTreeSet<&str> = by.iter().map(|&(j, _)| col_names[j].as_str()).collect();
            let inline = inline_of(name, &held);
            // What leaves coordinate `x` out with the held axes at `over`: the interval and the grid for a
            // number, the truth value the comparison never takes for a truth value.
            let leaves = |over: &[(String, (Rat, Rat))], x: usize| -> bool {
                if numeric {
                    let Axis::Num { coords, .. } = &axes[i] else { return false };
                    reach_cut(&coords[x], over, &inline)
                } else {
                    // `true` is the first coordinate of a boolean axis.
                    narrow_truth(c, e, over, &inline).is_some_and(|t| x == if t { 1 } else { 0 })
                }
            };
            if !numeric && define_truth.get(i).copied().flatten().is_some() {
                continue;
            }
            let open = |x: usize| match &axes[i] {
                Axis::Num { coords, .. } => !matches!(coords[x], Coord::Absent) && !before(x),
                _ => true,
            };
            let cut: Vec<bool> = (0..axes[i].len()).map(|x| open(x) && leaves(&over, x)).collect();
            // What the whole of the held coordinates leaves in, asked again over pieces of one of them,
            // and of a number, over pieces of an input the table has no axis for (§15.325).
            let mut pieces: Vec<(CutOn, Vec<bool>, Vec<Rat>)> = Vec::new();
            for x in (0..axes[i].len()).filter(|&x| open(x) && !cut[x]) {
                if left == 0 {
                    break;
                }
                let found = by.iter().enumerate().find_map(|(k, &(j, _))| {
                    let Axis::Num { step, .. } = &axes[j] else { return None };
                    let (lo, hi) = over[k].1;
                    let at = |a: Rat, b: Rat| -> bool {
                        let mut o = over.clone();
                        o[k].1 = (a, b);
                        leaves(&o, x)
                    };
                    split_out(lo, hi, *step, &mut left, &at).map(|cs| (CutOn::Axis(j), cs))
                });
                let found = match found {
                    Some(f) => Some(f),
                    None if numeric => input_pieces(&over, x, &mut left).map(|(n, cs)| (CutOn::Input(n), cs)),
                    None => None,
                };
                if let Some((k, cs)) = found {
                    match pieces.iter_mut().find(|p| p.0 == k) {
                        Some(p) => {
                            p.1[x] = true;
                            for v in cs {
                                if !p.2.iter().any(|w| w.cmp_to(v) == std::cmp::Ordering::Equal) {
                                    p.2.push(v);
                                }
                            }
                            p.2.sort_by(|a, b| a.cmp_to(*b));
                        }
                        None => {
                            let mut m = vec![false; axes[i].len()];
                            m[x] = true;
                            pieces.push((k, m, cs));
                        }
                    }
                }
            }
            if cut.iter().any(|x| *x) {
                out.push(NarrowCut { axis: i, by: by.clone(), out: cut, cuts: None });
            }
            for (k, m, cs) in pieces {
                out.push(NarrowCut { axis: i, by: by.clone(), out: m, cuts: Some((k, cs)) });
            }
        }
    }
    out
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
            // An optional column has one value more than the type it wraps: `none`, the absent
            // value. Since §2.1 rules that it "can only be consumed by a `none` cell and never
            // appears in an expression", the axis merely gains one coordinate. It used to gain
            // it on an enum only: a number's or a date's axis had no coordinate for the absent
            // value, and its cells were read in `T?`, where no number is a value — so `<5` took
            // every coordinate and `none` took none, and a table with a gap passed (§15.201).
            let (ty, opt) = match &ty {
                Ty::Opt(inner) => ((**inner).clone(), true),
                other => (other.clone(), false),
            };
            let axis = match &ty {
                Ty::Enum(en) => {
                    let mut vs = c.enums.get(en)?.clone();
                    // An input of an ordered enum takes the values its `range` lets in and no
                    // other (§15.210), as a number's axis runs over its declared range: the
                    // generated code refuses the rest at its door.
                    if let Some(allowed) = c.enum_ranges.get(name).filter(|_| inputs.iter().any(|i| i.name.text == *name)) {
                        vs.retain(|v| allowed.contains(v));
                    }
                    if opt {
                        vs.insert(0, crate::kw::NONE.into());
                    }
                    Axis::Enum { values: vs }
                }
                // An optional truth value is the enum of `none`, `true` and `false`: three values,
                // and a cell of `true` or `false` names one of them (§15.201).
                Ty::Bool if opt => Axis::Enum {
                    values: vec![crate::kw::NONE.into(), crate::kw::TRUE.into(), crate::kw::FALSE.into()],
                },
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
                    for cell in f.never_cells(name) {
                        if let Cell::Prefix(ps) = cell {
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
                    let declared = inputs.iter().find(|i| i.name.text == *name).and_then(|i| i.range.as_ref()).and_then(|r| declared_ends(c, name, r, &ty));
                    let (mut b, mut lo, mut hi) = num_bounds(&t.rows, ci, &ty, declared);
                    // The days of a koyomi date (§15.174): the axis runs over their hull, and
                    // every coordinate with none of them in it is one no input reaches.
                    let days = c.day_sets.get(name).map(|d| std::sync::Arc::new(d.days.clone()));
                    if let Some(r) = days.as_ref().and_then(|_| c.ranges.get(name).copied()) {
                        bound_by(&mut b, &mut lo, &mut hi, r);
                    }
                    // The latest or earliest date a walk found, and a fold's answer that is a
                    // date, run over the range the check worked out for them, as a number does
                    // (§15.236).
                    if let Some(r) = walk_answer_range(f, c, name) {
                        bound_by(&mut b, &mut lo, &mut hi, r);
                    }
                    // `outputs_over` (ritsu's port): the input takes only the days given.
                    if let Some(h) = crate::over::held(name) {
                        (lo, hi) = crate::over::narrow(&mut b, (lo, hi), h);
                    }
                    never_bounds(&mut b, (lo, hi), f, name, &ty);
                    // A date's step is one day. Dates are serial day numbers, so adjacent days
                    // differ by 1.
                    Axis::Num {
                        unit: String::new(),
                        date: true,
                        coords: num_coords(&b, lo, hi, Rat::int(1)),
                        shown: 1,
                        wire: 1,
                        step: Rat::int(1),
                        grid: Rat::int(1),
                        days,
                    }
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
                    // An open value's axis runs over its declared range, as an input's does
                    // (§15.227): the contributions may bring it anywhere inside.
                    let declared = match inputs.iter().find(|i| i.name.text == *name) {
                        Some(i) => i.range.as_ref().and_then(|r| declared_ends(c, name, r, &ty)),
                        None => f.open_value(name).and_then(|o| o.range.as_ref()).and_then(|r| c.opens.get(name).copied().or_else(|| declared_ends(c, name, r, &ty))),
                    };
                    let (mut b, mut lo, mut hi) = num_bounds(&t.rows, ci, &ty, declared);
                    // A count is bounded by construction — never negative, never longer than
                    // the sequence the guard caps — and it declares that bound (§15.58). The
                    // axis takes it, or the completeness check would ask for a row covering a
                    // count of −1. So does every other answer of a walk: a sum's declared range,
                    // the range a `max` or a `min` was worked out to, and the hull of the values
                    // a fold answers with (§15.236).
                    if let Some(r) = walk_answer_range(f, c, name) {
                        bound_by(&mut b, &mut lo, &mut hi, r);
                    }
                    // `outputs_over` (ritsu's port): the input takes only the values given.
                    if let Some(h) = crate::over::held(name) {
                        (lo, hi) = crate::over::narrow(&mut b, (lo, hi), h);
                    }
                    never_bounds(&mut b, (lo, hi), f, name, &ty);
                    let unit = match &ty {
                        Ty::Money { cur, .. } => cur.clone(),
                        Ty::Qty { unit, .. } => unit.clone(),
                        Ty::Rate => "%".into(),
                        // `number` counts something the rule never names, so there is nothing to
                        // write after the digits (§2.1).
                        _ => String::new(),
                    };
                    // The step every value of the column and every cell of it is a multiple of:
                    // 1 for money and quantities that come in, the declared step for rates, and
                    // for a value the rule computes, the step its values take (§15.190). The
                    // axis was cut at whole units whatever the column was, and a derive of
                    // tenths lost the open coordinate between two whole pounds a step apart:
                    // `<=1GBP` and `>=2GBP` passed as complete while 1.1 to 1.9 matched nothing.
                    let q = crate::coverage::quantum(c, name, &ty);
                    Axis::Num {
                        unit,
                        date: false,
                        coords: num_coords(&b, lo, hi, q),
                        // A rate is written in percent whatever its step is.
                        shown: if matches!(ty, Ty::Rate) { 100 } else { 1 },
                        wire: c.wire_scale(name),
                        step: q,
                        // The values' own step, which `q` divides; only where the two agree
                        // on a whole multiple is it used.
                        grid: {
                            let g = c.value_step(name);
                            if g.checked_div(q).is_some_and(|k| k.is_int()) { g } else { q }
                        },
                        days: None,
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
            // The absent value of an optional number or date goes first on its axis, as `none`
            // goes first among an optional enum's values (§15.201). An input ritsu's port holds
            // to values or days it is given takes none but those, as an enum held to its values
            // does not take `none`.
            let held_here = crate::over::held(name).is_some() || crate::days::held_over(name).is_some();
            let axis = match axis {
                Axis::Num { unit, date, mut coords, shown, wire, step, grid, days } if opt && !held_here => {
                    coords.insert(0, Coord::Absent);
                    Axis::Num { unit, date, coords, shown, wire, step, grid, days }
                }
                a => a,
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
        let written: Vec<Vec<(Rat, String)>> = (0..axes.len())
            .map(|ai| match (&axes[ai], c.ty_of(&col_names[ai])) {
                (Axis::Num { date: false, .. }, Some(ty)) => written_numbers(&t.rows, cell_of[ai], match &ty {
                    Ty::Opt(inner) => inner,
                    ty => ty,
                }),
                _ => Vec::new(),
            })
            .collect();

        // Build these **after** reordering the axes. Built before, their indices would not
        // line up with the masks.
        let is_define: Vec<bool> = col_names.iter().map(|n| c.define_deps.contains_key(n)).collect();
        // What a derived value reaches, an end an input's range does not write read as the end of
        // its type, as E108 and E112 read it (§15.299): `min(a, 100)` over `a` written `>=0` alone
        // reaches 0 to 100, and 0.50.0 asked a table over it for a row at 101.
        let derived: Vec<Option<((Option<Rat>, Option<Rat>), Vec<String>)>> = col_names
            .iter()
            .map(|n| {
                c.derived_deps.get(n).map(|deps| {
                    (c.reach_range(n).unwrap_or((None, None)), deps.clone())
                })
            })
            .collect();
        let define_reach: Vec<Option<Ival>> = col_names
            .iter()
            .zip(&axes)
            .map(|(n, a)| {
                let numeric = matches!(a, Axis::Num { date: false, .. });
                // An open value is read over its declared range, not over what its terms reach.
                let a_define = f.items.iter().any(|it| matches!(it, Item::Define(d) if d.name.text == *n)) && f.open_value(n).is_none();
                if !numeric || !a_define {
                    return None;
                }
                // An end an input's range does not write is read as the end of its type, as E108
                // reads it (§15.299): `min(a, 100)` over `a` written `>=0` alone reaches 0 to 100.
                c.ranges
                    .get(n)
                    .copied()
                    .filter(|(lo, hi)| lo.is_some() && hi.is_some())
                    .or_else(|| c.reach_of(n).map(|(lo, hi)| (Some(lo), Some(hi))))
            })
            .collect();
        let define_truth: Vec<Option<bool>> = col_names
            .iter()
            .zip(&axes)
            .map(|(n, a)| if matches!(a, Axis::Bool) { defines.get(n).and_then(|e| truth_of_expr(e, c)) } else { None })
            .collect();


        // The coordinates of the values an upstream table can produce. Only enum axes have them.
        // `outputs_over` (ritsu's port) holds an enum or a bool input to some of its values the
        // same way: the coordinates of the others are values no input takes.
        let mut written_above: Vec<Option<Vec<bool>>> = Vec::with_capacity(axes.len());
        let reachable: Vec<Option<Vec<bool>>> = (0..axes.len())
            .map(|ai| {
                let name = &col_names[ai];
                let held = f.inputs.iter().any(|i| i.name.text == *name).then(|| crate::over::held_values(name)).flatten();
                // What the table above writes into the column: every value of the axis but those no
                // row of it writes, each of which the certificate states as a fact (§15.253). Only a
                // value it can state is left out, so no box is narrowed by a fact the certificate
                // could not say (§15.261); a column with no table above to read is left whole.
                // The table is found through the definitions that only name another value, and a
                // number is a coordinate no row writes into, where every row writes one these
                // readings follow (§15.322).
                let src = source_of(f, c, t, name);
                let above: Option<Vec<String>> = src.as_ref().and_then(|s| {
                    let all: Vec<String> = match &axes[ai] {
                        Axis::Enum { values } => values.clone(),
                        Axis::Bool => vec![crate::kw::TRUE.to_string(), crate::kw::FALSE.to_string()],
                        _ => return None,
                    };
                    Some(all.into_iter().filter(|v| !never_writes(s.table, s.oi, v, c)).collect())
                });
                let numbers_above: Option<Vec<bool>> = src.as_ref().and_then(|s| {
                    let Axis::Num { .. } = &axes[ai] else { return None };
                    let puts: Vec<Put> = (0..s.table.rows.len()).map(|ri| put_of(s.table, ri, s.oi, &s.rounds, c)).collect();
                    if puts.iter().any(|p| *p == Put::Any) {
                        return None;
                    }
                    Some((0..axes[ai].len()).map(|ci| puts.iter().any(|p| coord_takes(&axes[ai], ci, p))).collect())
                });
                let mut out: Option<Vec<bool>> = numbers_above.clone();
                written_above.push(numbers_above);
                for vs in [above.as_ref(), held.as_ref()].into_iter().flatten() {
                    let these: Vec<bool> = match &axes[ai] {
                        Axis::Enum { values } => values.iter().map(|v| vs.contains(v)).collect(),
                        // A boolean can be an upstream output too. Coordinate 0 is true, 1 is false.
                        Axis::Bool => vec![vs.iter().any(|v| v == crate::kw::TRUE), vs.iter().any(|v| v == crate::kw::FALSE)],
                        _ => continue,
                    };
                    out = Some(match out {
                        Some(was) => was.iter().zip(&these).map(|(a, b)| *a && *b).collect(),
                        None => these,
                    });
                }
                out
            })
            .collect();

        let nevers: Vec<NeverCut> = f
            .nevers
            .iter()
            .enumerate()
            .filter_map(|(line, n)| {
                let on = n
                    .conds
                    .iter()
                    .map(|k| {
                        let ai = col_names.iter().position(|m| *m == k.name.text)?;
                        let ty = c.ty_of(&k.name.text)?;
                        Some((ai, cell_mask(&axes[ai], Some(&k.cell), ty.present(), c)))
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(NeverCut { line, on })
            })
            .collect();

        let mut unreachable_row = Vec::new();
        let mut masks = Vec::new();
        for row in &t.rows {
            let mut m = Vec::new();
            for (ai, axis) in axes.iter().enumerate() {
                let ty = c.ty_of(&col_names[ai])?;
                // A cell of an optional column is read in the type it wraps (§15.201).
                let v = cell_mask(axis, row.cells.get(cell_of[ai]), ty.present(), c);
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
        let mut seed: Vec<String> =
            col_names.iter().zip(&axes).filter(|(_, a)| matches!(a, Axis::Num { .. })).map(|(n, _)| n.clone()).collect();
        // The derives and `define`s the tables above read — the one that decides a column of this
        // table and the ones above it — where this table does not (§15.322): the model ties them to
        // what this table reads, so the ends of those tables' rows on them can meet a box's.
        let mut stack: Vec<&Table> = col_names.iter().filter_map(|n| source_of(f, c, t, n).map(|s| s.table)).collect();
        let mut seen: Vec<*const Table> = Vec::new();
        while let Some(u) = stack.pop() {
            if seen.contains(&(u as *const Table)) || seen.len() > CHAIN_TABLES {
                continue;
            }
            seen.push(u as *const Table);
            for (d, _) in &u.inputs {
                if col_names.contains(d) {
                    continue;
                }
                let computed = defines.contains_key(d) || exprs.contains_key(d);
                let number = c.ty_of(d).is_some_and(|ty| matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number));
                if computed && number && f.open_value(d).is_none() && !seed.contains(d) {
                    seed.push(d.clone());
                }
                if let Some(s) = source_of(f, c, u, d) {
                    stack.push(s.table);
                }
            }
        }
        let model: Vec<crate::fourier::Ground> = crate::fourier::grounds(&seed, f, c)
            .into_iter()
            .filter(|g| g.sys.iter().any(|q| matches!(q.origin, crate::fourier::Origin::Derive { .. } | crate::fourier::Origin::Define { .. } | crate::fourier::Origin::Constraint(_))))
            .collect();
        // A `define` the linear model has the equation of is read by the model, exactly over the
        // rationals (§15.209). Its cut keeps only the coordinates the model leaves values in with the
        // fact's axes held — those only the whole numbers rule out (§15.325), below — so no fact says
        // what the model says there. A smaller box under a fact that the model rules out as well, with
        // more axes fixed, may rest on either: the cover asks the facts first, as for any `define`.
        let modeled: BTreeSet<String> = col_names
            .iter()
            .filter(|name| model.iter().any(|g| g.sys.iter().any(|q| matches!(&q.origin, crate::fourier::Origin::Define { name: n, .. } if n == *name))))
            .cloned()
            .collect();
        let narrowed: Vec<NarrowCut> = narrow_cuts(&axes, &col_names, &exprs, &defines, &define_reach, &define_truth, c, f);
        let computed: BTreeMap<String, Expr> =
            defines.iter().chain(exprs.iter()).filter(|(n, _)| f.open_value(n).is_none()).map(|(n, x)| (n.clone(), x.clone())).collect();
        let mut region = TableRegion {
            axes,
            col_names,
            written,
            unreachable_row,
            display_of: cell_of,
            is_define,
            derived,
            define_reach,
            define_truth,
            exprs,
            defines,
            spans,
            masks,
            unanalyzable,
            constraints: f.constraints.clone(),
            nevers,
            model,
            written_above,
            narrowed: Vec::new(),
            computed,
            steps: input_steps(c, f),
        };
        // Of a `define` the model reads, a coordinate stays in the cut only where the model, over
        // the rationals, leaves values in it with the axes held: ends a step apart on the whole
        // numbers (`added < 2`, `excess > 0`, `excess = added - limit`, `limit >= 1`), or a whole
        // number of an input it multiplies (`fee = amount - qty * 32JPY`), rule out what the model
        // cannot (§15.325).
        region.narrowed = narrowed
            .into_iter()
            .filter_map(|mut n| {
                if !modeled.contains(&region.col_names[n.axis]) {
                    return Some(n);
                }
                for x in 0..n.out.len() {
                    if !n.out[x] {
                        continue;
                    }
                    let mut bx: Vec<Vec<usize>> = region.axes.iter().map(|a| (0..a.len()).collect()).collect();
                    for &(aj, cj) in &n.by {
                        bx[aj] = vec![cj];
                    }
                    bx[n.axis] = vec![x];
                    if region.refute_box(&bx).is_some() {
                        n.out[x] = false;
                    }
                }
                n.out.iter().any(|o| *o).then_some(n)
            })
            .collect();
        Some(region)
    }

    /// The `never` line that takes every point below `path`, by its place in this table's list
    /// (§15.282). An axis the path has not fixed is one the line takes whole, or the line takes
    /// only part of the box.
    pub(crate) fn never_rules_out(&self, path: &[usize]) -> Option<usize> {
        self.nevers.iter().position(|n| {
            n.on.iter().all(|(ai, m)| match path.get(*ai) {
                Some(&ci) => m[ci],
                None => m.iter().all(|x| *x),
            })
        })
    }

    /// The `never` line that takes every point of a row, by its place among the rule's lines.
    pub(crate) fn never_takes_row(&self, i: usize) -> Option<usize> {
        self.nevers.iter().find(|n| n.on.iter().all(|(ai, m)| (0..m.len()).all(|ci| !self.masks[i][*ai][ci] || m[ci]))).map(|n| n.line)
    }

    /// The coordinates a path allows on each axis: the one it takes where it has fixed one,
    /// every one where it has not.
    pub(crate) fn path_box(&self, path: &[usize]) -> Vec<Vec<usize>> {
        (0..self.axes.len())
            .map(|ai| match path.get(ai) {
                Some(&c) => vec![c],
                None => (0..self.axes[ai].len()).collect(),
            })
            .collect()
    }

    /// The coordinates two rows both take on each axis.
    pub(crate) fn pair_box(&self, i: usize, j: usize) -> Vec<Vec<usize>> {
        (0..self.axes.len())
            .map(|ai| (0..self.axes[ai].len()).filter(|&c| self.masks[i][ai][c] && self.masks[j][ai][c]).collect())
            .collect()
    }

    /// The two ends a set of coordinates allows on a numeric axis, each with whether it is
    /// left out: the lowest of their low ends, left out only when every coordinate that
    /// reaches it leaves it out, and the same at the top. `None` inside for an end that is
    /// not there.
    #[allow(clippy::type_complexity)]
    fn ends_of(&self, ai: usize, cs: &[usize]) -> Option<(Option<(Rat, bool)>, Option<(Rat, bool)>)> {
        use std::cmp::Ordering::*;
        let Axis::Num { coords, .. } = &self.axes[ai] else { return None };
        let mut ends: Option<(Option<(Rat, bool)>, Option<(Rat, bool)>)> = None;
        for &c in cs {
            let (a, b) = match coords.get(c)? {
                // The absent value has no number to put an end on (§15.201).
                Coord::Absent => continue,
                Coord::Point(v) => (Some((*v, false)), Some((*v, false))),
                Coord::Open(a, b) => (a.map(|x| (x, true)), b.map(|x| (x, true))),
            };
            ends = Some(match ends {
                None => (a, b),
                Some((lo, hi)) => (
                    match (lo, a) {
                        (Some((x, xs)), Some((y, ys))) => Some(match x.cmp_to(y) {
                            Less => (x, xs),
                            Greater => (y, ys),
                            Equal => (x, xs && ys),
                        }),
                        _ => None,
                    },
                    match (hi, b) {
                        (Some((x, xs)), Some((y, ys))) => Some(match x.cmp_to(y) {
                            Greater => (x, xs),
                            Less => (y, ys),
                            Equal => (x, xs && ys),
                        }),
                        _ => None,
                    },
                ),
            });
        }
        ends
    }

    /// A refutation of a box on the linear model (§15.141): the model's facts, and the two
    /// ends of the coordinates the box allows on each numeric axis. `None` when the model is
    /// empty or leaves the box possible.
    pub(crate) fn refute_box(&self, bx: &[Vec<usize>]) -> Option<crate::fourier::Refutation> {
        self.refute_in(&self.model, bx)
    }

    /// The same, over `model` rather than the table's own: the third form of E102 reads the
    /// relaxation [`Reach`] keeps (§15.189).
    fn refute_in(&self, model: &[crate::fourier::Ground], bx: &[Vec<usize>]) -> Option<crate::fourier::Refutation> {
        use crate::fourier::{Lin, Origin, Refutation};
        for g in model {
            let mut sys = g.sys.clone();
            for (ai, name) in self.col_names.iter().enumerate() {
                if !g.vars.contains(name) {
                    continue;
                }
                let Some((lo, hi)) = bx.get(ai).and_then(|cs| self.ends_of(ai, cs)) else { continue };
                let off = |x: Rat| Lin::var(name).plus(&Lin::con(x.mul(Rat::int(-1))));
                if let Some((x, strict)) = lo {
                    sys.push(off(x).ge(strict).tag(Origin::Coord { axis: ai, hi: false }));
                }
                if let Some((x, strict)) = hi {
                    sys.push(off(x).le(strict).tag(Origin::Coord { axis: ai, hi: true }));
                }
            }
            if let Some(r) = Refutation::of(sys) {
                return Some(r);
            }
        }
        None
    }

    /// Whether the linear model rules out the whole of what two rows both take.
    fn refutes_pair(&self, i: usize, j: usize) -> bool {
        !self.model.is_empty() && self.refute_box(&self.pair_box(i, j)).is_some()
    }

    /// Whether the linear model rules out the whole of the box a path fixes.
    fn refutes_path(&self, path: &[usize]) -> bool {
        !self.model.is_empty() && self.refute_box(&self.path_box(path)).is_some()
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
        self.hole_rec(rows, 0, budget, &mut Vec::new(), ups, c, &|_| false)
    }

    #[allow(clippy::too_many_arguments)]
    fn hole_rec(
        &self,
        rows: &[usize],
        ai: usize,
        budget: &mut i64,
        path: &mut Vec<usize>,
        ups: &[Upstream],
        chk: &Checked,
        skip: &dyn Fn(&[usize]) -> bool,
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
            return self.first_reachable(path, ups, chk, budget, true).filter(|h| !skip(h));
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
            if let Some(h) = self.hole_rec(&sub, ai + 1, budget, path, ups, chk, skip) {
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
    ///
    /// With `linear`, a point the sieve lets through is also held to the rule's linear model
    /// (§15.141), and to what a table above writes a value under together with it (§15.195), and
    /// one either rules out is no gap either. The cover's point-by-point leaf walks without them,
    /// because that leaf is re-checked by walking the sieve alone. The reach of a `define` is part
    /// of the sieve (§15.195).
    fn first_reachable(
        &self,
        path: &[usize],
        ups: &[Upstream],
        chk: &Checked,
        budget: &mut i64,
        linear: bool,
    ) -> Option<Vec<usize>> {
        if self.feasible(path) == Feasible::No
            || self.defines_rule_out(path).is_some()
            || (linear && (self.refutes_path(path) || self.above_rows_out(path, ups, chk).is_some()))
        {
            return None;
        }
        let mut p = path.to_vec();
        self.first_reachable_rec(&mut p, ups, chk, budget, linear)
    }

    fn first_reachable_rec(
        &self,
        p: &mut Vec<usize>,
        ups: &[Upstream],
        chk: &Checked,
        budget: &mut i64,
        linear: bool,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if p.len() == self.axes.len() {
            if self.feasible(p) == Feasible::No
                || self.defines_rule_out(p).is_some()
                || self.upstream_dead_at(p, ups, chk)
                || (linear && (self.refutes_path(p) || self.above_rows_out(p, ups, chk).is_some()))
            {
                return None;
            }
            return Some(p.clone());
        }
        let ai = p.len();
        for c in 0..self.axes[ai].len() {
            p.push(c);
            let keep = self.feasible(p) != Feasible::No && !self.define_out_of_reach(ai, p);
            let got = if keep { self.first_reachable_rec(p, ups, chk, budget, linear) } else { None };
            p.pop();
            if got.is_some() {
                return got;
            }
        }
        None
    }

    /// Whether every point of the box is ruled out by what a re-checker asks of a point: the sieve,
    /// or a fact about the tables above a certificate states — a value they never write, or two
    /// spans apart. The facts the points rest on come back, each with its point's text; `None`
    /// where a point rests on anything else (§15.325), or past the budget.
    fn points_stated(&self, path: &[usize], ups: &[Upstream], chk: &Checked, budget: &mut i64) -> Option<Vec<(String, Apart)>> {
        let mut out = Vec::new();
        let mut p = path.to_vec();
        self.points_stated_rec(&mut p, ups, chk, budget, &mut out).then_some(out)
    }

    fn points_stated_rec(&self, p: &mut Vec<usize>, ups: &[Upstream], chk: &Checked, budget: &mut i64, out: &mut Vec<(String, Apart)>) -> bool {
        *budget -= 1;
        if *budget < 0 {
            return false;
        }
        if p.len() == self.axes.len() {
            if self.feasible(p) == Feasible::No || self.defines_rule_out(p).is_some() {
                return true;
            }
            if !self.upstream_dead_at(p, ups, chk) {
                return false;
            }
            let Some(a) = self.never_at(p, ups, chk).or_else(|| self.apart_at(p, ups, chk)) else { return false };
            out.push((self.witness_text(p), a));
            return true;
        }
        let ai = p.len();
        for c in 0..self.axes[ai].len() {
            p.push(c);
            let keep = self.feasible(p) != Feasible::No && !self.define_out_of_reach(ai, p);
            let ok = !keep || self.points_stated_rec(p, ups, chk, budget, out);
            p.pop();
            if !ok {
                return false;
            }
        }
        true
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
            // The same sieve the completeness check reads (§15.196): what a table above writes a
            // value under, together with the linear model, as well.
            if self.feasible(p) == Feasible::No
                || self.upstream_dead_at(p, ups, chk)
                || self.refutes_path(p)
                || self.above_rows_out(p, ups, chk).is_some()
            {
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

    /// The points of this box the sieve leaves standing, in the order
    /// [`TableRegion::first_reachable`] meets them with `linear`, at most `limit` of them
    /// (§15.322): where an input is looked for behind a gap, the first point may be one no input
    /// gives while a later one of the same box is.
    fn reachable_points(&self, path: &[usize], ups: &[Upstream], chk: &Checked, budget: &mut i64, limit: usize) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        if self.feasible(path) == Feasible::No || self.defines_rule_out(path).is_some() || self.refutes_path(path) || self.above_rows_out(path, ups, chk).is_some() {
            return out;
        }
        let mut p = path.to_vec();
        self.points_rec(&mut p, None, ups, chk, budget, limit, &mut out);
        out
    }

    /// The points where rows `i` and `j` meet that the sieve leaves standing, in the order
    /// [`TableRegion::first_reachable_pair`] meets them, at most `limit` of them (§15.322).
    fn pair_points(&self, i: usize, j: usize, ups: &[Upstream], chk: &Checked, budget: &mut i64, limit: usize) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        self.points_rec(&mut Vec::new(), Some((i, j)), ups, chk, budget, limit, &mut out);
        out
    }

    /// The walk of [`TableRegion::reachable_points`] and [`TableRegion::pair_points`]: each
    /// coordinate in turn, the sieve on every prefix and at the end, and only coordinates both
    /// rows take where there is a pair.
    #[allow(clippy::too_many_arguments)]
    fn points_rec(&self, p: &mut Vec<usize>, pair: Option<(usize, usize)>, ups: &[Upstream], chk: &Checked, budget: &mut i64, limit: usize, out: &mut Vec<Vec<usize>>) {
        *budget -= 1;
        if *budget < 0 || out.len() >= limit {
            return;
        }
        if p.len() == self.axes.len() {
            if !(self.feasible(p) == Feasible::No
                || (pair.is_none() && self.defines_rule_out(p).is_some())
                || self.upstream_dead_at(p, ups, chk)
                || self.refutes_path(p)
                || self.above_rows_out(p, ups, chk).is_some())
            {
                out.push(p.clone());
            }
            return;
        }
        let ai = p.len();
        for x in 0..self.axes[ai].len() {
            if pair.is_some_and(|(i, j)| !(self.masks[i][ai][x] && self.masks[j][ai][x])) {
                continue;
            }
            p.push(x);
            if self.feasible(p) != Feasible::No && (pair.is_some() || !self.define_out_of_reach(ai, p)) {
                self.points_rec(p, pair, ups, chk, budget, limit, out);
            }
            p.pop();
            if *budget < 0 || out.len() >= limit {
                return;
            }
        }
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
    /// axes first) is not shown to the reader. This is the certificate's text of a point, in the
    /// labels of its axes ([`Axis::witness_at`]); a sentence says it with [`Self::witness_said`].
    fn witness_text(&self, path: &[usize]) -> String {
        self.point_text(path, self.witness_values(path), false)
    }

    /// The witness as a sentence says it, its numbers grouped as the languages group them
    /// ([`Axis::witness_said_at`], §15.333).
    fn witness_said(&self, path: &[usize]) -> String {
        self.point_text(path, self.witness_values(path), true)
    }

    /// The same, with the values behind the point already chosen.
    fn witness_said_with(&self, path: &[usize], vals: Option<Vec<Option<Rat>>>) -> String {
        self.point_text(path, vals, true)
    }

    fn point_text(&self, path: &[usize], vals: Option<Vec<Option<Rat>>>, said: bool) -> String {
        let mut items: Vec<(usize, String)> = (0..self.axes.len())
            .map(|ai| {
                let v = vals.as_ref().and_then(|w| w.get(ai).copied().flatten());
                let at = path.get(ai).copied().unwrap_or(0);
                let value = if said { self.axes[ai].witness_said_at(at, v) } else { self.axes[ai].witness_at(at, v) };
                (self.display_of[ai], format!("{} = {value}", self.col_names[ai]))
            })
            .collect();
        items.sort_by_key(|(d, _)| *d);
        items.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(", ")
    }

    /// The same witness, as `(column, value)` pairs in the table's visible column order.
    fn witness_pairs(&self, path: &[usize]) -> Vec<(String, crate::diag::WVal)> {
        self.witness_pairs_with(path, self.witness_values(path))
    }

    /// The same, with the values behind the point already chosen: a certificate hands over
    /// the ones it solved, and the pairs a reader can paste have to be that same point.
    fn witness_pairs_with(&self, path: &[usize], vals: Option<Vec<Option<Rat>>>) -> Vec<(String, crate::diag::WVal)> {
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

    /// What a gap asks of the columns the reads of a collection stand behind, and of what is read
    /// beside them: the coordinates of the gap, or with `point` the one value of each.
    fn asks_of_gap(&self, path: &[usize], point: Option<&[Option<Rat>]>, walks: &crate::projection::walks::Walks) -> (Vec<(String, crate::projection::walks::Ask)>, Vec<crate::fourier::Ineq>) {
        use crate::fourier::Lin;
        use crate::projection::walks::Ask;
        let mut asks = Vec::new();
        let mut ineqs = Vec::new();
        for (ai, axis) in self.axes.iter().enumerate() {
            let name = &self.col_names[ai];
            if !walks.knows(name) {
                continue;
            }
            // A gap that stops short leaves the rest of its axes open, and the example at the
            // first coordinate of each.
            let ci = match (path.get(ai), point) {
                (Some(i), _) => *i,
                (None, Some(_)) => 0,
                (None, None) => continue,
            };
            let exactly = |ineqs: &mut Vec<crate::fourier::Ineq>, k: Rat| {
                let d = Lin::var(name).plus(&Lin::con(k.mul(Rat::int(-1))));
                ineqs.push(d.clone().le(false));
                ineqs.push(d.ge(false));
            };
            match axis {
                Axis::Bool => asks.push((name.clone(), Ask::Truth(ci == 0))),
                Axis::Enum { values } => match values.get(ci).map(String::as_str) {
                    Some(crate::kw::TRUE) => asks.push((name.clone(), Ask::Truth(true))),
                    Some(crate::kw::FALSE) => asks.push((name.clone(), Ask::Truth(false))),
                    Some(crate::kw::NONE) | None => {}
                    Some(v) => asks.push((name.clone(), Ask::Words(vec![v.to_string()]))),
                },
                Axis::Num { coords, wire, .. } => {
                    let w = Rat::int(*wire);
                    let chosen = point.and_then(|p| p.get(ai).copied().flatten());
                    match (chosen, coords.get(ci)) {
                        (_, Some(Coord::Absent) | None) => {}
                        (Some(x), _) => exactly(&mut ineqs, x.mul(w)),
                        (None, Some(Coord::Point(x))) => exactly(&mut ineqs, x.mul(w)),
                        (None, Some(Coord::Open(a, b))) => {
                            if let Some(a) = a {
                                ineqs.push(Lin::var(name).plus(&Lin::con(a.mul(w).mul(Rat::int(-1)))).ge(true));
                            }
                            if let Some(b) = b {
                                ineqs.push(Lin::var(name).plus(&Lin::con(b.mul(w).mul(Rat::int(-1)))).le(true));
                            }
                        }
                    }
                }
                Axis::Prefix { .. } => {}
            }
        }
        (asks, ineqs)
    }

    /// The gap E101 shows: the first one, unless no definition of a collection's reads reaches it
    /// and another one is reached. A gap that occurs is the one to fix first, and saying a gap
    /// never occurs while one beside it does would be read as saying it of the table. Only so many
    /// gaps are tried, since each is a question of its own; past them the first one stands.
    #[allow(clippy::too_many_arguments)]
    fn gap_to_show(&self, first: Vec<usize>, rows: &[usize], budget: i64, ups: &[Upstream], c: &Checked, f: &RuleFile, rule_path: &str) -> Vec<usize> {
        use crate::projection::walks::Walks;
        if !reads_a_collection(f) {
            return first;
        }
        let walks = Walks::of(f, c, rule_path);
        if !self.col_names.iter().any(|n| walks.walks_for(n)) {
            return first;
        }
        let held = walks.constraints();
        if !self.never_reached(&first, &walks, &held) {
            return first;
        }
        let mut left = budget;
        let tried = std::cell::Cell::new(0);
        let skip = |h: &[usize]| {
            tried.set(tried.get() + 1);
            tried.get() > GAPS_TRIED || self.never_reached(h, &walks, &held)
        };
        self.hole_rec(rows, 0, &mut left, &mut Vec::new(), ups, c, &skip).unwrap_or(first)
    }

    /// Whether no definition reaches a gap.
    fn never_reached(&self, path: &[usize], walks: &crate::projection::walks::Walks, held: &[crate::fourier::Ineq]) -> bool {
        let (asks, mut ineqs) = self.asks_of_gap(path, None, walks);
        ineqs.extend(held.iter().cloned());
        matches!(walks.ask(&asks, &ineqs), crate::projection::walks::Possible::No(_))
    }

    /// What the definitions of a collection's reads say of a gap. The table is held to every
    /// combination whatever they say, because a target that does not read `from` is handed what
    /// the caller worked out (§15.55), so E101 stays; only its words change. A gap no definition
    /// reaches gets notes that say so. In a gap some part of which is reached, the example is put
    /// on a combination that occurs, when the one it had does not. The last of the three says
    /// whether the example is one a collection built and evaluated gives, every column the table
    /// reads that is no input being a read of one (§15.322).
    fn behind_the_gap(&self, path: &[usize], boxes: &[Vec<usize>], vals: Option<Vec<Option<Rat>>>, c: &Checked, f: &RuleFile, rule_path: &str) -> (Option<Vec<Option<Rat>>>, Vec<String>, bool) {
        use crate::projection::walks::{Possible, Walks};
        if !reads_a_collection(f) {
            return (vals, Vec::new(), false);
        }
        let walks = Walks::of(f, c, rule_path);
        let read: Vec<String> = self.col_names.iter().filter(|n| walks.walks_for(n)).cloned().collect();
        if read.is_empty() {
            return (vals, Vec::new(), false);
        }
        // every column the table reads that is no input, a read of a collection
        let all_read = self.col_names.iter().all(|n| walks.walks_for(n) || f.inputs.iter().any(|i| i.name.text == *n));
        let held = walks.constraints();
        let (asks, mut ineqs) = self.asks_of_gap(path, None, &walks);
        ineqs.extend(held.iter().cloned());
        match walks.ask(&asks, &ineqs) {
            Possible::No(_) => {
                let mut notes = walks.never_notes(&read);
                // The lines that take out every part no definition reaches, not the one of the
                // example alone: a line for each run would name the next part each time
                // (§15.317). A box the definitions rule out as a whole is one line; the example's
                // own line stands where its box is ruled out only in part.
                let mut lines: Vec<String> = Vec::new();
                let mut whole = false;
                for b in boxes.iter().take(GAPS_TRIED) {
                    if self.never_reached(b, &walks, &held)
                        && let Some(line) = self.never_for_gap(b, &walks)
                    {
                        whole |= path.starts_with(b);
                        if !lines.contains(&line) {
                            lines.push(line);
                        }
                    }
                }
                if !whole
                    && let Some(line) = self.never_for_gap(path, &walks)
                    && !lines.contains(&line)
                {
                    lines.insert(0, line);
                }
                match lines.as_slice() {
                    [] => {}
                    [line] => notes.push(tr!(
                        "ヒント: 起こらない組み合わせは、行を足さずに次の一行で表から外せます: `{line}`",
                        "hint: a combination that does not happen comes out of the tables with this line, and no row: `{line}`"
                    )),
                    _ => {
                        notes.push(tr!(
                            "ヒント: 起こらない組み合わせは、行を足さずに次の行で表から外せます:",
                            "hint: the combinations that do not happen come out of the tables with these lines, and no rows:"
                        ));
                        notes.extend(lines.iter().map(|l| format!("  {l}")));
                    }
                }
                (vals, notes, false)
            }
            Possible::Unknown => (vals, Vec::new(), false),
            Possible::Yes(ex) => {
                let Some(point) = vals.as_deref() else { return (vals, Vec::new(), false) };
                let (asks, mut ineqs) = self.asks_of_gap(path, Some(point), &walks);
                ineqs.extend(held);
                match walks.ask(&asks, &ineqs) {
                    Possible::No(_) => {}
                    Possible::Yes(_) => return (vals, Vec::new(), all_read),
                    Possible::Unknown => return (vals, Vec::new(), false),
                }
                let moved = vals.map(|w| {
                    w.into_iter()
                        .enumerate()
                        .map(|(ai, v)| match (&self.axes[ai], ex.values.get(&self.col_names[ai])) {
                            (Axis::Num { wire, .. }, Some(x)) if walks.knows(&self.col_names[ai]) => Some(x.div(Rat::int(*wire))),
                            _ => v,
                        })
                        .collect()
                });
                (moved, Vec::new(), all_read)
            }
        }
    }

    /// The `never` line that takes a gap out (§15.282), when the definitions show it does not
    /// occur: each axis the gap fixes and the definitions know of, at the coordinate it takes.
    fn never_for_gap(&self, path: &[usize], walks: &crate::projection::walks::Walks) -> Option<String> {
        use crate::ast::{Lit, Name, NeverCond};
        let word = |w: &str| Cell::Lit(Lit::Word(w.to_string()));
        let span = crate::diag::Span::new(1, 1, 1);
        let mut conds = Vec::new();
        for (ai, &ci) in path.iter().enumerate() {
            let name = &self.col_names[ai];
            if !walks.knows(name) {
                continue;
            }
            let cell = match &self.axes[ai] {
                Axis::Bool => word(if ci == 0 { crate::kw::TRUE } else { crate::kw::FALSE }),
                Axis::Enum { values } => match values.get(ci).map(String::as_str) {
                    None | Some(crate::kw::NONE) => return None,
                    Some(v) => word(v),
                },
                Axis::Num { coords, date, unit, shown, .. } => {
                    let lit = |v: Rat| -> Option<Lit> {
                        if *date {
                            let (y, m, d) = crate::types::ord_to_date(v);
                            return Some(Lit::Date(y, m, d));
                        }
                        let text = format!("{}{unit}", v.mul(Rat::int(*shown)));
                        let toks = crate::lex::lex_line_soft(1, &text).ok()?.0;
                        match toks.as_slice() {
                            [crate::lex::Token { kind: crate::lex::Kind::Num(n), .. }] => Some(Lit::Num(n.clone())),
                            _ => None,
                        }
                    };
                    match coords.get(ci)? {
                        Coord::Absent => return None,
                        Coord::Point(v) => Cell::Lit(lit(*v)?),
                        Coord::Open(a, b) => {
                            let mut cs = Vec::new();
                            if let Some(a) = a {
                                cs.push((CmpOp::Gt, lit(*a)?));
                            }
                            if let Some(b) = b {
                                cs.push((CmpOp::Lt, lit(*b)?));
                            }
                            Cell::Cmp(cs)
                        }
                    }
                }
                Axis::Prefix { .. } => return None,
            };
            conds.push(NeverCond { name: Name { text: name.clone(), ascii: None, span: span.clone() }, cell });
        }
        walks.line_for(conds, &span)
    }

    /// The input behind a gap that lies on a column the rule computes (§15.190). The witness
    /// says what the derived columns come to, and no caller sends those, so the input is worked
    /// out: the linear model solved with every numeric column held at the value the witness
    /// shows, the inputs the table reads at their own witness values, and the rest at the first
    /// value they can take. Where that input does not make the witness — the model ties no `min`,
    /// rounding, `define` or table above to the inputs, so nothing pins them — and the inputs the
    /// computed columns are worked out from are few, every combination of those is walked
    /// instead, the inputs the table reads still at their witness values, and the first that
    /// makes it is taken (§15.194, `grid::exhaust_input`). Either way it is handed over only when
    /// the reference evaluator gives every column the table reads the very value the witness
    /// shows and no row takes them, which is the input the generated code stops on. `None` for a
    /// gap on inputs alone, where the witness is the input already, and when nothing could be
    /// built or there were too many inputs to walk, which proves nothing either way.
    fn input_behind(&self, path: &[usize], vals: Option<Vec<Option<Rat>>>, t: &Table, c: &Checked, f: &RuleFile) -> Option<BTreeMap<String, crate::eval::Val>> {
        use crate::eval::Val;
        // A column the rule computes is any column no caller sends: a derive, a define of any type
        // (`is_define` marks the boolean ones only) and what a table above gives (§15.194).
        if self.col_names.iter().all(|n| f.inputs.iter().any(|i| i.name.text == *n)) {
            return None;
        }
        let vals = vals?;
        let coord = |ai: usize| path.get(ai).copied().unwrap_or(0);
        let at_axis = |ai: usize| self.axes[ai].witness_num(coord(ai), vals.get(ai).copied().flatten());
        let as_val = |name: &str, v: Rat| -> Val {
            if c.ty_of(name).is_some_and(|t| *t.present() == Ty::Date) {
                let (y, m, d) = crate::types::ord_to_date(v);
                Val::Date(y, m, d)
            } else {
                Val::Num(v)
            }
        };
        let mut a = crate::vectors::first_input(f, c)?;
        for (ai, name) in self.col_names.iter().enumerate() {
            if !a.contains_key(name) {
                continue;
            }
            let v = match &self.axes[ai] {
                // The absent value of an optional input, and an optional truth value's two
                // values, as the evaluator reads them (§15.201).
                Axis::Num { coords, .. } if matches!(coords.get(coord(ai)), Some(Coord::Absent)) => Val::Enum(crate::kw::NONE.into()),
                Axis::Num { .. } => as_val(name, at_axis(ai)?),
                Axis::Enum { values } if matches!(values.get(coord(ai)).map(String::as_str), Some(crate::kw::TRUE | crate::kw::FALSE)) => {
                    Val::Bool(values[coord(ai)] == crate::kw::TRUE)
                }
                Axis::Enum { values } => Val::Enum(values.get(coord(ai))?.clone()),
                Axis::Bool => Val::Bool(coord(ai) == 0),
                Axis::Prefix { .. } => match self.axes[ai].witness_val(coord(ai), None) {
                    crate::diag::WVal::Str(s) => Val::Str(s),
                    _ => return None,
                },
            };
            a.insert(name.clone(), v);
        }
        // Every column at the witness's own value: what the note says the input makes is what
        // it makes, not merely some point of the same gap.
        let makes = |b: &BTreeMap<String, Val>| -> bool {
            let Some(got) = crate::vectors::no_row_takes(f, c, t, b) else { return false };
            self.col_names.iter().enumerate().all(|(ai, name)| match (&self.axes[ai], got.get(name)) {
                (Axis::Num { coords, .. }, Some(Val::Enum(w))) if matches!(coords.get(coord(ai)), Some(Coord::Absent)) => w == crate::kw::NONE,
                (Axis::Enum { values }, Some(Val::Bool(b))) => values.get(coord(ai)).map(String::as_str) == Some(if *b { crate::kw::TRUE } else { crate::kw::FALSE }),
                (Axis::Num { .. }, Some(Val::Num(v))) => at_axis(ai).is_some_and(|w| w.cmp_to(*v) == std::cmp::Ordering::Equal),
                (Axis::Num { .. }, Some(Val::Date(y, m, d))) => {
                    at_axis(ai).is_some_and(|w| w.cmp_to(crate::types::date_ord(*y, *m, *d)) == std::cmp::Ordering::Equal)
                }
                (Axis::Enum { values }, Some(Val::Enum(v))) => values.get(coord(ai)) == Some(v),
                (Axis::Bool, Some(Val::Bool(b))) => *b == (coord(ai) == 0),
                (Axis::Prefix { .. }, Some(Val::Str(s))) => {
                    matches!(self.axes[ai].witness_val(coord(ai), None), crate::diag::WVal::Str(w) if w == *s)
                }
                _ => false,
            })
        };
        if let Some(b) = self.solved_from(&a, &at_axis, c).filter(|b| makes(b)) {
            return Some(b);
        }
        // What the model could not tie to the inputs (§15.194): the inputs behind the computed
        // columns, walked when they are few, with the inputs the table reads kept at the witness.
        let held: BTreeSet<String> = self.col_names.iter().filter(|n| a.contains_key(*n)).cloned().collect();
        crate::grid::exhaust_input(f, c, &self.col_names, &a, &held, makes).flatten()
    }

    /// The linear model solved with every numeric column it ties held at `at` of its axis: `a`, with
    /// each input the model solves for at the value it comes to, whole where it can be
    /// (`fourier::solve`). `None` where the model has no such point within its own bounds, which
    /// proves nothing either way (§15.190). The input behind a gap, and the one behind a pair of
    /// rows that meet, are worked out this way before anything is searched for (§15.322).
    fn solved_from(&self, a: &BTreeMap<String, crate::eval::Val>, at: &dyn Fn(usize) -> Option<Rat>, c: &Checked) -> Option<BTreeMap<String, crate::eval::Val>> {
        use crate::fourier::{Lin, Origin};
        let mut b = a.clone();
        for g in &self.model {
            let mut sys = g.sys.clone();
            for (ai, name) in self.col_names.iter().enumerate() {
                if !g.vars.contains(name) {
                    continue;
                }
                let Some(v) = at(ai) else { continue };
                let d = Lin::var(name).plus(&Lin::con(v.mul(Rat::int(-1))));
                sys.push(d.clone().le(false).tag(Origin::Coord { axis: ai, hi: true }));
                sys.push(d.ge(false).tag(Origin::Coord { axis: ai, hi: false }));
            }
            let got = crate::fourier::solve(sys.clone())?;
            if !sys.iter().all(|q| q.holds_at(&got) == Some(true)) {
                return None;
            }
            for (n, v) in got {
                if b.contains_key(&n) {
                    b.insert(n.clone(), as_input_val(c, &n, v));
                }
            }
        }
        Some(b)
    }

    /// Whether a value the reference evaluator gives a column lies in coordinate `ci` of its axis
    /// `ai`: the one value of a point, strictly between the ends of an interval, the word of a
    /// value, the absent value of an optional input (§15.322).
    fn lands(&self, ai: usize, ci: usize, v: &crate::eval::Val) -> bool {
        use crate::eval::Val;
        use std::cmp::Ordering::*;
        match (&self.axes[ai], v) {
            (Axis::Num { coords, .. }, Val::Enum(w)) => matches!(coords.get(ci), Some(Coord::Absent)) && w == crate::kw::NONE,
            (Axis::Num { coords, .. }, Val::Num(_) | Val::Date(..)) => {
                let x = match v {
                    Val::Num(r) => *r,
                    Val::Date(y, m, d) => crate::types::date_ord(*y, *m, *d),
                    _ => return false,
                };
                match coords.get(ci) {
                    Some(Coord::Point(w)) => w.cmp_to(x) == Equal,
                    Some(Coord::Open(a, b)) => a.is_none_or(|a| a.cmp_to(x) == Less) && b.is_none_or(|b| x.cmp_to(b) == Less),
                    _ => false,
                }
            }
            (Axis::Enum { values }, Val::Bool(b)) => values.get(ci).map(String::as_str) == Some(if *b { crate::kw::TRUE } else { crate::kw::FALSE }),
            (Axis::Enum { values }, Val::Enum(w)) => values.get(ci) == Some(w),
            (Axis::Bool, Val::Bool(b)) => *b == (ci == 0),
            _ => false,
        }
    }

    /// Where the value a column comes to stands to coordinate `ci` of its numeric axis `ai`: short
    /// of it (-1), in it (0) or past it (1). `None` for a value that is no number.
    fn side_of(&self, ai: usize, ci: usize, v: &crate::eval::Val) -> Option<i8> {
        use crate::eval::Val;
        use std::cmp::Ordering::*;
        let x = match v {
            Val::Num(r) => *r,
            Val::Date(y, m, d) => crate::types::date_ord(*y, *m, *d),
            _ => return None,
        };
        let Axis::Num { coords, .. } = &self.axes[ai] else { return None };
        match coords.get(ci)? {
            Coord::Point(w) => Some(match x.cmp_to(*w) {
                Less => -1,
                Equal => 0,
                Greater => 1,
            }),
            Coord::Open(a, b) => Some(if a.is_some_and(|a| x.cmp_to(a) != Greater) {
                -1
            } else if b.is_some_and(|b| x.cmp_to(b) != Less) {
                1
            } else {
                0
            }),
            Coord::Absent => None,
        }
    }

    /// An input with every input the table reads at the value point `p` shows for it — the value
    /// `vals` gives a numeric axis, or else the first of its coordinate — and every other input at
    /// the first value it can take.
    fn inputs_at(&self, p: &[usize], vals: Option<&[Option<Rat>]>, c: &Checked, f: &RuleFile) -> Option<BTreeMap<String, crate::eval::Val>> {
        use crate::eval::Val;
        let mut a = crate::vectors::first_input(f, c)?;
        for (ai, name) in self.col_names.iter().enumerate() {
            if !a.contains_key(name) {
                continue;
            }
            let v = match &self.axes[ai] {
                Axis::Num { coords, .. } if matches!(coords.get(*p.get(ai)?), Some(Coord::Absent)) => Val::Enum(crate::kw::NONE.into()),
                Axis::Num { .. } => as_input_val(c, name, self.axes[ai].witness_num(p[ai], vals.and_then(|v| v.get(ai).copied().flatten()))?),
                Axis::Enum { values } if matches!(values.get(p[ai]).map(String::as_str), Some(crate::kw::TRUE | crate::kw::FALSE)) => {
                    Val::Bool(values[p[ai]] == crate::kw::TRUE)
                }
                Axis::Enum { values } => Val::Enum(values.get(p[ai])?.clone()),
                Axis::Bool => Val::Bool(p[ai] == 0),
                Axis::Prefix { .. } => return None,
            };
            a.insert(name.clone(), v);
        }
        Some(a)
    }

    /// The input that makes the example point `p` shows as it reads, where there is one: the input
    /// with every input the table reads at the value the example gives it, or else the one the
    /// linear model solves for with every column it ties at that value ([`TableRegion::solved_from`]),
    /// which makes every column the very value the example gives it, and `holds` takes the values
    /// (§15.322).
    fn made_as_shown(&self, p: &[usize], t: &Table, c: &Checked, f: &RuleFile, holds: &dyn Fn(&BTreeMap<String, crate::eval::Val>) -> bool) -> Option<BTreeMap<String, crate::eval::Val>> {
        use crate::eval::Val;
        let vals = self.witness_values(p);
        let shown = |ai: usize| self.axes[ai].witness_num(p[ai], vals.as_ref().and_then(|v| v.get(ai).copied().flatten()));
        let makes = |a: &BTreeMap<String, Val>| -> bool {
            let Some(cols) = crate::vectors::columns_at(f, c, t, a) else { return false };
            holds(&cols)
                && self.col_names.iter().enumerate().all(|(ai, n)| match (&self.axes[ai], cols.get(n)) {
                    (Axis::Num { coords, .. }, Some(v)) if !matches!(coords.get(p[ai]), Some(Coord::Absent)) => {
                        let x = match v {
                            Val::Num(r) => *r,
                            Val::Date(y, m, d) => crate::types::date_ord(*y, *m, *d),
                            _ => return false,
                        };
                        shown(ai).is_some_and(|w| w.cmp_to(x) == std::cmp::Ordering::Equal)
                    }
                    (_, Some(v)) => self.lands(ai, p[ai], v),
                    (_, None) => false,
                })
        };
        let a = self.inputs_at(p, vals.as_deref(), c, f)?;
        if makes(&a) {
            return Some(a);
        }
        self.solved_from(&a, &shown, c).filter(|b| makes(b))
    }

    /// Where the values the reference evaluator gives the columns for an input lie (§15.322): the
    /// coordinate of each axis its column's value lands in, and the values as a witness numbers
    /// them. An example written from these is the input's own. `None` where a column has no value
    /// or one in no coordinate.
    fn landing(&self, cols: &BTreeMap<String, crate::eval::Val>) -> Option<(Vec<usize>, Vec<Option<Rat>>)> {
        let p = self
            .col_names
            .iter()
            .enumerate()
            .map(|(ai, n)| {
                let v = cols.get(n)?;
                (0..self.axes[ai].len()).find(|&ci| self.lands(ai, ci, v))
            })
            .collect::<Option<Vec<usize>>>()?;
        Some((p, self.vals_of(cols)))
    }

    /// An input behind a part of a gap (§15.322): each point of each part in `boxes` the sieve
    /// leaves standing, in turn, with the input [`TableRegion::input_behind`] works out for it or
    /// one [`TableRegion::input_inside`] builds in it, as far as [`GAP_POINTS_TRIED`] points of the
    /// first [`GAPS_TRIED`] parts. The point the input's values land on, those values, and the
    /// input; `None` when none gives one, which proves nothing.
    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    fn gap_with_input(
        &self,
        boxes: &[Vec<usize>],
        t: &Table,
        ups: &[Upstream],
        c: &Checked,
        f: &RuleFile,
        budget: i64,
    ) -> Option<(Vec<usize>, Vec<Option<Rat>>, BTreeMap<String, crate::eval::Val>)> {
        let none_takes = |cols: &BTreeMap<String, crate::eval::Val>| t.rows.iter().all(|r| !crate::vectors::row_holds_at(c, t, r, cols));
        let mut left = budget;
        let mut tried = 0;
        for b in boxes.iter().take(GAPS_TRIED) {
            for p in self.reachable_points(b, ups, c, &mut left, GAP_POINTS_TRIED - tried) {
                tried += 1;
                let vals = self.reach_values(&p);
                if let Some(a) = self.input_behind(&p, vals.clone(), t, c, f)
                    && let Some(cols) = crate::vectors::no_row_takes(f, c, t, &a)
                    && let Some((p, v)) = self.landing(&cols)
                {
                    return Some((p, v, a));
                }
                if let Some((a, cols)) = self.input_inside(&p, t, ups, c, f, &none_takes)
                    && let Some((p, v)) = self.landing(&cols)
                {
                    return Some((p, v, a));
                }
            }
            if tried >= GAP_POINTS_TRIED || left < 0 {
                break;
            }
        }
        None
    }

    /// Every input the table's columns are worked out from, each run through the reference
    /// evaluator, where they are few enough to run them all (`grid::exhaust_input`, §15.322). Run
    /// once for a table, and only where an example is wanted that nothing else gave. `None` where
    /// they are too many.
    fn walk_inputs(&self, t: &Table, c: &Checked, f: &RuleFile) -> Option<Walked> {
        let a0 = crate::vectors::first_input(f, c)?;
        let mut w = Walked::default();
        let n = t.rows.len();
        crate::grid::exhaust_input(f, c, &self.col_names, &a0, &BTreeSet::new(), |a| {
            let Some(cols) = crate::vectors::columns_at(f, c, t, a) else { return false };
            let hold: Vec<usize> = (0..n).filter(|&r| crate::vectors::row_holds_at(c, t, &t.rows[r], &cols)).collect();
            let at = self.landing(&cols);
            // the rows the input reaches: each row it matches, or under `policy first` the first
            let reached = if t.policy == crate::ast::Policy::Unique { &hold[..] } else { &hold[..hold.len().min(1)] };
            for &r in reached {
                w.reach.entry(r).or_insert_with(|| a.clone());
            }
            if hold.is_empty() && w.gap.is_none() {
                match &at {
                    Some((p, v)) => w.gap = Some((p.clone(), v.clone(), a.clone())),
                    None => w.gap_unplaced = true,
                }
            }
            for x in 0..hold.len() {
                for y in (x + 1)..hold.len() {
                    let k = (hold[x], hold[y]);
                    if w.both.contains_key(&k) {
                        continue;
                    }
                    match &at {
                        Some((p, v)) => {
                            w.both.insert(k, (p.clone(), v.clone(), a.clone()));
                        }
                        None => {
                            w.both_unplaced.insert(k);
                        }
                    }
                }
            }
            false
        })?;
        Some(w)
    }

    /// The values the columns come to, as a witness numbers them: a number for each numeric axis,
    /// nothing for the others.
    fn vals_of(&self, cols: &BTreeMap<String, crate::eval::Val>) -> Vec<Option<Rat>> {
        use crate::eval::Val;
        self.col_names
            .iter()
            .enumerate()
            .map(|(ai, n)| match (&self.axes[ai], cols.get(n)) {
                (Axis::Num { .. }, Some(Val::Num(r))) => Some(*r),
                (Axis::Num { .. }, Some(Val::Date(y, m, d))) => Some(crate::types::date_ord(*y, *m, *d)),
                _ => None,
            })
            .collect()
    }

    /// An input a caller can send inside the box point `p` spans, with the values the reference
    /// evaluator gives the table's columns for it (§15.322): each column's value lies in the
    /// point's coordinate on its axis, and `holds` takes the values — no row takes them for a gap,
    /// both rows do for a pair. Only for a table that reads a column no caller sends. What is
    /// tried, in turn: every input the table reads at the first value of its coordinate; the
    /// values one of the rows of a table above fires on, where the point takes a value one decides
    /// ([`TableRegion::above_values`]); the inputs a table above reads and this one does not, at the
    /// first value of the box of a row of it that writes the point's value (as far as
    /// [`ABOVE_ROWS_TRIED`] rows); and, from each of those, each numeric input the table reads moved
    /// through its coordinate — halving between where a column computed from it falls short of its
    /// coordinate and where it no longer does, which finds the first value of the input that brings
    /// a column one input moves one way into it, and then the coordinate's last value — and from the
    /// first, each other numeric input moved the same way through its range; last, from the first,
    /// the same moves one after another, each input left where its move brought the column nearest
    /// its coordinate, which finds a column several inputs add up to. Every input handed over is one
    /// the evaluator ran; `None` proves nothing.
    #[allow(clippy::type_complexity)]
    fn input_inside(
        &self,
        p: &[usize],
        t: &Table,
        ups: &[Upstream],
        c: &Checked,
        f: &RuleFile,
        holds: &dyn Fn(&BTreeMap<String, crate::eval::Val>) -> bool,
    ) -> Option<(BTreeMap<String, crate::eval::Val>, BTreeMap<String, crate::eval::Val>)> {
        use crate::eval::Val;
        let is_input = |n: &str| f.inputs.iter().any(|i| i.name.text == n);
        if p.len() != self.axes.len() || self.col_names.iter().all(|n| is_input(n)) || self.axes.iter().any(|a| matches!(a, Axis::Prefix { .. })) {
            return None;
        }
        let as_val = |name: &str, v: Rat| as_input_val(c, name, v);
        let base = self.inputs_at(p, None, c, f)?;
        let take = |a: &BTreeMap<String, Val>| -> Option<BTreeMap<String, Val>> {
            let cols = crate::vectors::columns_at(f, c, t, a)?;
            let land = self.col_names.iter().enumerate().all(|(ai, n)| cols.get(n).is_some_and(|v| self.lands(ai, p[ai], v)));
            (land && holds(&cols)).then_some(cols)
        };
        if let Some(cols) = take(&base) {
            return Some((base, cols));
        }
        // the values one of the rows of a table above fires on
        let keys: Vec<AboveKey> = ups
            .iter()
            .filter_map(|u| self.col_names.iter().position(|n| *n == u.name))
            .map(|ai| AboveKey { axis: ai, coord: p[ai], through: false })
            .collect();
        if !keys.is_empty()
            && let Some((nums, _)) = self.above_values(p, &[], ups, c, &keys)
        {
            let mut a = base.clone();
            for (ai, name) in self.col_names.iter().enumerate() {
                if let (true, Axis::Num { .. }, Some(Some(v))) = (a.contains_key(name), &self.axes[ai], nums.get(ai)) {
                    a.insert(name.clone(), as_val(name, *v));
                }
            }
            if let Some(cols) = take(&a) {
                return Some((a, cols));
            }
        }
        // the inputs a table above reads and this one does not, at what a row of it that writes
        // the point's value there takes on them: the first value of the row's box on each
        let mut bases = vec![base.clone()];
        for up in ups {
            let Some(ai) = self.col_names.iter().position(|n| *n == up.name) else { continue };
            let d = up.decider();
            for ri in (0..up.table.rows.len()).take(ABOVE_ROWS_TRIED) {
                if !lands(&d, ri, None, &self.axes[ai], c).is_none_or(|m| m.get(p[ai]).copied().unwrap_or(false)) {
                    continue;
                }
                let mut a = base.clone();
                for (ua, m) in up.reg.col_names.iter().enumerate() {
                    if self.col_names.contains(m) || !a.contains_key(m) {
                        continue;
                    }
                    let Some(x) = (0..up.reg.axes[ua].len()).find(|&x| up.reg.masks[ri][ua][x]) else { continue };
                    let v = match &up.reg.axes[ua] {
                        Axis::Num { coords, .. } if matches!(coords.get(x), Some(Coord::Absent)) => Val::Enum(crate::kw::NONE.into()),
                        Axis::Num { .. } => {
                            let Some((lo, _)) = closed_coord(&up.reg.axes[ua], x, m, c) else { continue };
                            as_val(m, lo)
                        }
                        Axis::Enum { values } if matches!(values[x].as_str(), crate::kw::TRUE | crate::kw::FALSE) => Val::Bool(values[x] == crate::kw::TRUE),
                        Axis::Enum { values } => Val::Enum(values[x].clone()),
                        Axis::Bool => Val::Bool(x == 0),
                        Axis::Prefix { .. } => continue,
                    };
                    a.insert(m.clone(), v);
                }
                if !bases.contains(&a) {
                    if let Some(cols) = take(&a) {
                        return Some((a, cols));
                    }
                    bases.push(a);
                }
            }
        }
        // each numeric input the table reads, through its coordinate, from each of those; and from
        // the first, each other numeric input, through its range
        let computed: Vec<usize> = (0..self.axes.len()).filter(|&ai| !is_input(&self.col_names[ai]) && matches!(self.axes[ai], Axis::Num { .. })).collect();
        let numeric = |n: &str| c.ty_of(n).is_some_and(|ty| matches!(ty.present(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date));
        let others: Vec<&String> = f.inputs.iter().map(|i| &i.name.text).filter(|n| !self.col_names.contains(*n) && numeric(n)).collect();
        let moves = |base: &BTreeMap<String, Val>, first: bool| -> Vec<(String, Option<usize>)> {
            self.col_names
                .iter()
                .enumerate()
                .filter(|(aj, name)| base.contains_key(*name) && matches!(self.axes[*aj], Axis::Num { .. }))
                .map(|(aj, name)| (name.clone(), Some(aj)))
                .chain(others.iter().filter(|_| first).map(|name| ((*name).clone(), None)))
                .collect()
        };
        // One input moved through its values from `base`: an input that brings every column into
        // the point and that `holds` takes, where the move finds one; and the value the move leaves
        // the input at, for moves after it to start from — where the column it follows comes into
        // its coordinate, or else the end of the input that brings the column nearer to it.
        #[allow(clippy::type_complexity)]
        let move_one = |base: &BTreeMap<String, Val>, name: &String, aj: Option<usize>| -> (Option<(BTreeMap<String, Val>, BTreeMap<String, Val>)>, Option<Rat>) {
            if !base.contains_key(name) {
                return (None, None);
            }
            let ends = match aj {
                Some(aj) => closed_coord(&self.axes[aj], p[aj], name, c),
                None => c.wide_range(name),
            };
            let Some((lo, hi)) = ends else { return (None, None) };
            if lo.cmp_to(hi) != std::cmp::Ordering::Less {
                return (None, None);
            }
            let Some(ty) = c.ty_of(name) else { return (None, None) };
            let step = crate::coverage::quantum(c, name, ty.present());
            if step.num <= 0 {
                return (None, None);
            }
            let at = |x: Rat| {
                let mut a = base.clone();
                a.insert(name.clone(), as_val(name, x));
                a
            };
            let value = |x: Rat, k: usize| -> Option<Rat> {
                match crate::vectors::columns_at(f, c, t, &at(x))?.get(&self.col_names[k])? {
                    Val::Num(r) => Some(*r),
                    Val::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                    _ => None,
                }
            };
            // The computed column the walk follows: the first that the coordinate's first or last
            // value leaves outside its own coordinate.
            let side = |x: Rat, k: usize| -> Option<i8> {
                let cols = crate::vectors::columns_at(f, c, t, &at(x))?;
                self.side_of(k, p[k], cols.get(&self.col_names[k])?)
            };
            let follow = computed.iter().copied().find(|&k| side(lo, k).is_some_and(|s| s != 0) || side(hi, k).is_some_and(|s| s != 0));
            let mut keep = None;
            if let Some(k) = follow
                && let (Some(sl), Some(sh)) = (side(lo, k), side(hi, k))
            {
                if sl == 0 {
                    keep = Some(lo);
                } else if sl != sh {
                    // the first value of the input on which the column is no longer on the side it
                    // starts on
                    let (mut l, mut h) = (lo, hi);
                    for _ in 0..128 {
                        let n = h.sub(l).div(step);
                        if n.cmp_to(Rat::int(1)) != std::cmp::Ordering::Greater {
                            break;
                        }
                        let m = l.add(step.mul(Rat::int((n.num / n.den) / 2)));
                        if side(m, k) == Some(sl) {
                            l = m;
                        } else {
                            h = m;
                        }
                    }
                    if let Some(cols) = take(&at(h)) {
                        return (Some((at(h), cols)), Some(h));
                    }
                    keep = Some(h);
                } else if let (Some(vl), Some(vh)) = (value(lo, k), value(hi, k)) {
                    // both ends short of the coordinate (or both past it): the end whose value lies
                    // nearer to it
                    let rises = vh.cmp_to(vl) == std::cmp::Ordering::Greater;
                    keep = Some(if (sl < 0) == rises { hi } else { lo });
                }
            }
            if let Some(cols) = take(&at(hi)) {
                return (Some((at(hi), cols)), Some(hi));
            }
            (None, keep)
        };
        for (bi, base) in bases.iter().enumerate() {
            for (name, aj) in moves(base, bi == 0) {
                if let (Some(found), _) = move_one(base, &name, aj) {
                    return Some(found);
                }
            }
        }
        // Last, from the first, the same moves one after another, each input left where its own
        // move left it: a column several inputs add up to comes into a coordinate that none of them
        // brings it into alone.
        let first = moves(&bases[0], true);
        if first.len() >= 2 {
            let mut a = bases[0].clone();
            for (name, aj) in &first {
                let (found, keep) = move_one(&a, name, *aj);
                if let Some(found) = found {
                    return Some(found);
                }
                if let Some(x) = keep {
                    a.insert(name.clone(), as_val(name, x));
                }
            }
        }
        None
    }

    /// An input that reaches row `i`, built inside a box of the row the sieve leaves standing
    /// ([`TableRegion::input_inside`]); under `policy first`, one no row above it takes. Looked for
    /// where neither the vectors nor the gap behind the row gave one (W128, §15.322).
    fn input_reaching(&self, t: &Table, c: &Checked, f: &RuleFile, ups: &[Upstream], i: usize, budget: i64) -> Option<BTreeMap<String, crate::eval::Val>> {
        let unique = t.policy == crate::ast::Policy::Unique;
        let holds = |cols: &BTreeMap<String, crate::eval::Val>| {
            crate::vectors::row_holds_at(c, t, &t.rows[i], cols) && (unique || (0..i).all(|k| !crate::vectors::row_holds_at(c, t, &t.rows[k], cols)))
        };
        // a few of the row's boxes the tables above leave standing, each tried once
        let tried: std::cell::RefCell<Vec<Vec<usize>>> = std::cell::RefCell::new(Vec::new());
        let mut left = budget;
        for _ in 0..REACHING_BOXES {
            let fresh = |p: &[usize]| !tried.borrow().iter().any(|q| q.as_slice() == p) && self.above_rows_out(p, ups, c).is_none();
            let p = self.reach_point(i, unique, &mut left, &fresh)?;
            if let Some((a, _)) = self.input_inside(&p, t, ups, c, f, &holds) {
                return Some(a);
            }
            tried.borrow_mut().push(p);
        }
        None
    }

    /// An input that reaches row `i` of the set, found from the gap the check finds with the row
    /// taken out: no other row takes the input behind that gap, so this one does — which the
    /// reference evaluator is asked to confirm. `None` where no gap turns up within the budget, or
    /// no input makes the gap's values (W128 stays then).
    fn input_by_gap(
        &self,
        set: &crate::defset::DefSet,
        c: &Checked,
        f: &RuleFile,
        ups: &[Upstream],
        i: usize,
        budget: i64,
    ) -> Option<BTreeMap<String, crate::eval::Val>> {
        let t = &set.table;
        let others: Vec<usize> = (0..t.rows.len()).filter(|r| *r != i).collect();
        let mut left = budget;
        let hole = self.find_hole(&others, &mut left, ups, c)?;
        let vals = self.reach_values(&hole);
        let mut without = t.clone();
        without.rows.remove(i);
        let a = self.input_behind(&hole, vals, &without, c, f)?;
        let (_, fired, _) = crate::eval::run_bindings(f, c, a.clone().into_iter().collect());
        fired.contains(&crate::eval::row_tag(set.row_table(i), t.rows[i].index)).then_some(a)
    }
}

/// How many gaps E101 tries, looking for one the definitions of a collection's reads reach.
const GAPS_TRIED: usize = 64;

/// How many points of those parts are tried in all (§15.322).
const GAP_POINTS_TRIED: usize = 64;

/// How many rows of a table above an input is built from, where it reads inputs the table does not
/// (§15.322).
const ABOVE_ROWS_TRIED: usize = 32;

/// An input, the point of a table its columns' values land on, and those values.
type Placed = (Vec<usize>, Vec<Option<Rat>>, BTreeMap<String, crate::eval::Val>);

/// What running every input a table's columns are worked out from shows ([`TableRegion::walk_inputs`]):
/// the first input no row takes, for each pair of rows the first input both take, and for each row
/// the first input that reaches it (one it matches, and under `policy first` no row above it does).
/// An input whose values land in no coordinate is not shown; `gap_unplaced` and `both_unplaced`
/// hold where one did what is looked for, so that nothing is said there of what the run found.
#[derive(Default)]
struct Walked {
    reach: BTreeMap<usize, BTreeMap<String, crate::eval::Val>>,
    gap: Option<Placed>,
    gap_unplaced: bool,
    both: BTreeMap<(usize, usize), Placed>,
    both_unplaced: BTreeSet<(usize, usize)>,
}

impl Walked {
    /// Whether the run found no input no row takes, and none that it could not place.
    fn no_gap(&self) -> bool {
        self.gap.is_none() && !self.gap_unplaced
    }

    /// Whether the run found no input rows `i` and `j` both take, and none that it could not place.
    fn apart(&self, i: usize, j: usize) -> bool {
        !self.both.contains_key(&(i, j)) && !self.both_unplaced.contains(&(i, j))
    }
}

/// How many of the parts no row takes E101 writes out, each with its row; the rest are counted.
const GAP_ROWS_SHOWN: usize = 8;

/// How many boxes no row takes E101 gathers before it stops and says there are more.
const GAP_BOXES: usize = 2048;

/// One part of the input no row takes, as E101 shows it (§15.317).
struct GapRow {
    /// The row that takes exactly this part, its output cells left as `?`: None where no cell
    /// names the part of some column.
    row: Option<String>,
    /// Where there is no such row: the row with `-` in the columns no cell names the part of,
    /// and the coordinates it spans, column by column. Written after every other row of a
    /// `policy first` table it takes the part, the rows above having taken the rest of it.
    wide: Option<(String, Vec<BTreeSet<usize>>)>,
    /// The coordinates of the part, column by column.
    own: Vec<BTreeSet<usize>>,
    /// The part in words, column by column, in the language of the run.
    words: String,
    /// Whether the part is one value of every column: one input, as far as the table reads it.
    point: bool,
    /// Whether a cell names a value a table above writes inside a cut, not a boundary the table
    /// writes or an end of a range (§15.331).
    held: bool,
}

/// One column's share of a part.
struct Piece {
    /// The cell that takes exactly it: None where no cell does.
    cell: Option<String>,
    /// The same in words: None where it is every value of the column.
    words: Option<String>,
    /// Its coordinates.
    own: BTreeSet<usize>,
    /// Where no cell takes exactly it: the narrowest cell that takes it and more, with the
    /// coordinates that cell spans. None where that cell is `-`.
    wide: Option<(String, BTreeSet<usize>)>,
    /// Whether it is one value.
    point: bool,
}

impl Piece {
    /// A piece some cell takes exactly, which is one value where `point` says so.
    fn exact(cell: String, words: Option<String>, own: BTreeSet<usize>, point: bool) -> Piece {
        Piece { cell: Some(cell), words, own, wide: None, point }
    }
}

/// Whether the box `part` lies inside the boxes `over` taken together, each a set of coordinates
/// per column: what lies outside the first of them has to lie inside the rest.
fn box_covered(part: &[BTreeSet<usize>], over: &[&[BTreeSet<usize>]]) -> bool {
    let Some((b, rest)) = over.split_first() else { return false };
    if part.iter().zip(b.iter()).any(|(p, q)| p.is_disjoint(q)) {
        return box_covered(part, rest);
    }
    let mut inside: Vec<BTreeSet<usize>> = part.to_vec();
    for k in 0..part.len() {
        let out: BTreeSet<usize> = inside[k].difference(&b[k]).copied().collect();
        if !out.is_empty() {
            let mut piece = inside.clone();
            piece[k] = out;
            if !box_covered(&piece, rest) {
                return false;
            }
        }
        inside[k] = inside[k].intersection(&b[k]).copied().collect();
    }
    true
}

impl TableRegion {
    /// Every box no row takes that some input reaches, in the order the search of §6.3 meets
    /// them: each the path to it, which spans every coordinate of the axes past its end. And
    /// whether these are all of them — false when the budget or [`GAP_BOXES`] ran out first.
    /// The boxes are those `find_hole` would return a point of, one after another: the same
    /// walk, the same sieve.
    fn gap_boxes(&self, rows: &[usize], budget: i64, ups: &[Upstream], c: &Checked) -> (Vec<Vec<usize>>, bool) {
        let mut left = budget;
        let mut out = Vec::new();
        let all = self.gap_boxes_rec(rows, 0, &mut left, &mut Vec::new(), ups, c, &mut out);
        (out, all && left >= 0)
    }

    #[allow(clippy::too_many_arguments)]
    fn gap_boxes_rec(&self, rows: &[usize], ai: usize, budget: &mut i64, path: &mut Vec<usize>, ups: &[Upstream], chk: &Checked, out: &mut Vec<Vec<usize>>) -> bool {
        *budget -= 1;
        if *budget < 0 {
            return false;
        }
        if rows.is_empty() {
            if self.first_reachable(path, ups, chk, budget, true).is_some() {
                if out.len() == GAP_BOXES {
                    return false;
                }
                out.push(path.clone());
            }
            return *budget >= 0;
        }
        if ai == self.axes.len() || rows.iter().any(|&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x))) {
            return true;
        }
        for c in 0..self.axes[ai].len() {
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            let all = self.gap_boxes_rec(&sub, ai + 1, budget, path, ups, chk, out);
            path.pop();
            if !all {
                return false;
            }
        }
        true
    }

    /// The coordinates, axis by axis, of the box `bx` that some input reaches (§15.325): on each
    /// axis, those a point of the box the sieve lets in has there — walked as
    /// [`Self::first_reachable`] walks, with the linear model and the tables above. A part no row
    /// takes is said by the values that occur in it: a value the tables above never give with the
    /// rest of the part, or one a `constraint` rules out, is left out of the words and the rows, and
    /// the rows that take what is left close the gap as the whole part would, since no input reaches
    /// the rest. The box as it is where the budget runs out.
    fn gap_reached(&self, bx: &[BTreeSet<usize>], ups: &[Upstream], chk: &Checked, budget: &mut i64) -> Vec<BTreeSet<usize>> {
        // A part some input reaches has a point, so no axis is left empty; as it is otherwise.
        self.reached_at(bx, &[], ups, chk, budget).unwrap_or_else(|| bx.to_vec())
    }

    /// [`Self::gap_reached`] with the columns of some axes held to one value each, `vals`: a value
    /// a table above writes inside a cut, which a point of the box takes there only where a row
    /// that writes it may fire (§15.331). None where no point of the box is reached so, and the
    /// box as it is where the budget runs out.
    fn reached_at(&self, bx: &[BTreeSet<usize>], vals: &[(usize, Rat)], ups: &[Upstream], chk: &Checked, budget: &mut i64) -> Option<Vec<BTreeSet<usize>>> {
        let mut sets: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); bx.len()];
        for j in 0..bx.len() {
            for &x in &bx[j] {
                if sets[j].contains(&x) {
                    continue;
                }
                match self.reachable_with(&mut Vec::new(), bx, (j, x), vals, ups, chk, budget) {
                    Some(pt) => {
                        for (k, &y) in pt.iter().enumerate() {
                            sets[k].insert(y);
                        }
                    }
                    None if *budget < 0 => return Some(bx.to_vec()),
                    None => {}
                }
            }
        }
        (!sets.iter().any(|s| s.is_empty())).then_some(sets)
    }

    /// The first point of the box `bx` the sieve lets in whose coordinate on the axis `at.0` is
    /// `at.1`, held to what [`Self::first_reachable`] holds a point to with `linear`, and to the
    /// values `vals` (§15.331).
    #[allow(clippy::too_many_arguments)]
    fn reachable_with(&self, p: &mut Vec<usize>, bx: &[BTreeSet<usize>], at: (usize, usize), vals: &[(usize, Rat)], ups: &[Upstream], chk: &Checked, budget: &mut i64) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if p.len() == self.axes.len() {
            if self.feasible(p) == Feasible::No
                || self.defines_rule_out(p).is_some()
                || self.upstream_dead_at(p, ups, chk)
                || self.refutes_path(p)
                || self.above_rows_out(p, ups, chk).is_some()
                || self.values_out(p, vals, ups, chk)
            {
                return None;
            }
            return Some(p.clone());
        }
        let ai = p.len();
        for &c in &bx[ai] {
            if ai == at.0 && c != at.1 {
                continue;
            }
            p.push(c);
            let keep = self.feasible(p) != Feasible::No && !self.define_out_of_reach(ai, p);
            let got = if keep { self.reachable_with(p, bx, at, vals, ups, chk, budget) } else { None };
            p.pop();
            if got.is_some() {
                return got;
            }
        }
        None
    }

    /// Whether at the point `p` one of the values `vals` holds its column to is one no row above
    /// lets come: every row of the deciding table that writes it fires nowhere at the point, by
    /// every way it arrives (§15.331).
    fn values_out(&self, p: &[usize], vals: &[(usize, Rat)], ups: &[Upstream], chk: &Checked) -> bool {
        if vals.is_empty() {
            return false;
        }
        let bx = self.path_box(p);
        vals.iter().any(|&(ai, v)| {
            let Some(up) = ups.iter().find(|u| u.name == self.col_names[ai]) else { return false };
            let Some(&ci) = p.get(ai) else { return false };
            let writes = |put: &Put| matches!(put, Put::Num(w) if w.cmp_to(v) == std::cmp::Ordering::Equal);
            // where the point lets the table above read: a row that writes the value fires nowhere
            // there, a row of a `policy first` table counted only where the rows before it leave
            // some of it
            let d = up.decider();
            let allowed: Vec<Vec<bool>> = up.reg.col_names.iter().enumerate().map(|(ua, n)| self.project(n, p, &up.reg.axes[ua])).collect();
            let none_fires = (0..up.table.rows.len()).filter(|&ri| writes(&up.put(ri, chk))).all(|ri| !fires_in(&d, ri, &allowed, true));
            none_fires || self.above_rows_where(&bx, up, ai, ci, &writes, chk).is_some()
        })
    }

    /// The values a table above writes inside the coordinate `ci` of the axis `ai` of numbers or
    /// dates, in order (§15.331): what the column takes in that cut, since it takes nothing but
    /// what the rows of the table that decides it write. None where that is not known — a row
    /// writes what it reads, a name or an expression, which may stand anywhere — and where the
    /// coordinate is one value already, the column is decided by no table above, or a value is
    /// one a cell cannot hold.
    fn written_in(&self, ai: usize, ci: usize, ups: &[Upstream], chk: &Checked) -> Option<Vec<Rat>> {
        let Axis::Num { coords, .. } = &self.axes[ai] else { return None };
        if !matches!(coords.get(ci), Some(Coord::Open(..))) {
            return None;
        }
        let up = ups.iter().find(|u| u.name == self.col_names[ai])?;
        let mut vals: Vec<Rat> = Vec::new();
        for ri in 0..up.table.rows.len() {
            let put = up.put(ri, chk);
            match put {
                Put::Any => return None,
                Put::Num(v) if coord_takes(&self.axes[ai], ci, &put) => {
                    if !vals.iter().any(|w| w.cmp_to(v) == std::cmp::Ordering::Equal) {
                        vals.push(v);
                    }
                }
                _ => {}
            }
        }
        if vals.is_empty() || !vals.iter().all(|v| self.axes[ai].writable(*v)) {
            return None;
        }
        vals.sort_by(|a, b| a.cmp_to(*b));
        Some(vals)
    }

    /// A part as its rows write it (§15.331): on each column a table above decides, a cut of
    /// numbers or dates in which that table writes only some values is those values, one part for
    /// each, which is then held to what some input reaches with that value. A value no input
    /// reaches with the rest of the part is not said, and since the column takes no other value in
    /// the cut, the rows of the values close the part as the cut would. Beside each part, the value
    /// its column is held to, axis by axis. The part as it is where the budget runs out.
    fn at_written(&self, part: Vec<BTreeSet<usize>>, ups: &[Upstream], chk: &Checked, budget: &mut i64) -> Vec<(Vec<BTreeSet<usize>>, Vec<Option<Rat>>)> {
        let n = self.axes.len();
        let whole = vec![(part.clone(), vec![None; n])];
        let mut out = whole.clone();
        for ai in 0..n {
            let mut next = Vec::new();
            for (p, vals) in out {
                let mut rest: BTreeSet<usize> = BTreeSet::new();
                let mut points: Vec<(usize, Rat)> = Vec::new();
                for &ci in &p[ai] {
                    match self.written_in(ai, ci, ups, chk) {
                        Some(vs) => points.extend(vs.into_iter().map(|v| (ci, v))),
                        None => {
                            rest.insert(ci);
                        }
                    }
                }
                if points.is_empty() {
                    next.push((p, vals));
                    continue;
                }
                let held: Vec<(usize, Rat)> = vals.iter().enumerate().filter_map(|(k, v)| v.map(|v| (k, v))).collect();
                let before = next.len();
                for (ci, v) in points {
                    let mut q = p.clone();
                    q[ai] = BTreeSet::from([ci]);
                    let mut with = held.clone();
                    with.push((ai, v));
                    if let Some(q) = self.reached_at(&q, &with, ups, chk, budget) {
                        let mut vs = vals.clone();
                        vs[ai] = Some(v);
                        next.push((q, vs));
                    }
                }
                if !rest.is_empty() {
                    let mut q = p.clone();
                    q[ai] = rest;
                    if let Some(q) = self.reached_at(&q, &held, ups, chk, budget) {
                        next.push((q, vals.clone()));
                    }
                }
                // a part is reached, so some of it is said; as it was otherwise
                if next.len() == before {
                    next.push((p, vals));
                }
            }
            if *budget < 0 {
                return whole;
            }
            out = next;
        }
        out
    }

    /// Parts no row takes put together where they differ on one axis only, that axis holding the
    /// coordinates of both, until none merges: the parts stay disjoint, and together they are what
    /// they were (§15.317).
    fn merge_parts(&self, mut parts: Vec<Vec<BTreeSet<usize>>>) -> Vec<Vec<BTreeSet<usize>>> {
        let n = self.axes.len();
        loop {
            let before = parts.len();
            for k in 0..n {
                let mut order: Vec<Vec<BTreeSet<usize>>> = Vec::new();
                let mut groups: BTreeMap<Vec<BTreeSet<usize>>, BTreeSet<usize>> = BTreeMap::new();
                for p in parts {
                    let mut rest = p;
                    let mine = rest.remove(k);
                    if !groups.contains_key(&rest) {
                        order.push(rest.clone());
                    }
                    groups.entry(rest).or_default().extend(mine);
                }
                parts = order
                    .into_iter()
                    .map(|mut rest| {
                        let set = groups.remove(&rest).unwrap_or_default();
                        rest.insert(k, set);
                        rest
                    })
                    .collect();
            }
            if parts.len() == before {
                return parts;
            }
        }
    }

    /// [`Self::merge_parts`] for parts held to values (§15.331): those that hold the same values
    /// are put together as parts are, and no others.
    fn merge_held(&self, parts: Vec<(Vec<BTreeSet<usize>>, Vec<Option<Rat>>)>) -> Vec<(Vec<BTreeSet<usize>>, Vec<Option<Rat>>)> {
        let same = |a: &[Option<Rat>], b: &[Option<Rat>]| {
            a.iter().zip(b).all(|(x, y)| match (x, y) {
                (Some(x), Some(y)) => x.cmp_to(*y) == std::cmp::Ordering::Equal,
                (x, y) => x.is_none() && y.is_none(),
            })
        };
        let mut groups: Vec<(Vec<Option<Rat>>, Vec<Vec<BTreeSet<usize>>>)> = Vec::new();
        for (p, vals) in parts {
            match groups.iter_mut().find(|(v, _)| same(v, &vals)) {
                Some((_, ps)) => ps.push(p),
                None => groups.push((vals, vec![p])),
            }
        }
        groups.into_iter().flat_map(|(vals, ps)| self.merge_parts(ps).into_iter().map(move |p| (p, vals.clone()))).collect()
    }

    /// The parts no row takes as the rows that take them, and in words (§15.317): the boxes put
    /// together ([`Self::merge_parts`]), each part then held to the coordinates some input reaches
    /// in it ([`Self::gap_reached`], §15.325) and put together again. Boxes that
    /// differ on one axis only are put together, that axis holding the coordinates of both,
    /// until none merges: the parts stay disjoint, and together they are the boxes, no more.
    /// A part is then written as one row where each column's coordinates are one cell — a run
    /// of a number's or a date's coordinates as the comparison between its ends, an enum's
    /// values as their set, every coordinate as `-` — and as several where they are not (the
    /// absent value beside others, two runs). The cells use the coordinates the axes already
    /// have, which are the boundaries the table writes and the ends of the ranges, so adding
    /// the rows cuts no axis anew, and the search finds no gap where they are.
    fn gap_rows(&self, boxes: &[Vec<usize>], outputs: usize, ups: &[Upstream], chk: &Checked, budget: i64) -> Vec<GapRow> {
        let n = self.axes.len();
        let parts = self.merge_parts(
            boxes
                .iter()
                .map(|p| {
                    (0..n)
                        .map(|ai| match p.get(ai) {
                            Some(c) => BTreeSet::from([*c]),
                            None => (0..self.axes[ai].len()).collect(),
                        })
                        .collect()
                })
                .collect(),
        );
        let mut left = budget;
        let parts = self.merge_parts(parts.iter().map(|p| self.gap_reached(p, ups, chk, &mut left)).collect());
        // A cut of a column a table above decides, in which that table writes a few values only, as
        // those values (§15.331).
        let mut held: Vec<(Vec<BTreeSet<usize>>, Vec<Option<Rat>>)> = Vec::new();
        for p in parts {
            held.extend(self.at_written(p, ups, chk, &mut left));
        }
        let mut parts = self.merge_held(held);
        // in the order of the table's columns, by the coordinates of each and the value held
        let shown: Vec<usize> = {
            let mut axes: Vec<usize> = (0..n).collect();
            axes.sort_by_key(|&ai| self.display_of[ai]);
            axes
        };
        parts.sort_by(|(p, pv), (q, qv)| {
            shown
                .iter()
                .map(|&ai| {
                    p[ai].iter().cmp(q[ai].iter()).then_with(|| match (pv[ai], qv[ai]) {
                        (Some(a), Some(b)) => a.cmp_to(b),
                        (a, b) => a.is_some().cmp(&b.is_some()),
                    })
                })
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let ja = crate::i18n::ja();
        let mut out = Vec::new();
        for (p, vals) in &parts {
            // each column's pieces, and every way of taking one of each
            let pieces: Vec<Vec<Piece>> = shown
                .iter()
                .map(|&ai| match (vals[ai], p[ai].first()) {
                    (Some(v), Some(&ci)) => vec![self.value_piece(ai, ci, v)],
                    _ => self.pieces(ai, &p[ai].iter().copied().collect::<Vec<_>>()),
                })
                .collect();
            let mut at = vec![0usize; pieces.len()];
            'rows: loop {
                let picked: Vec<&Piece> = pieces.iter().zip(&at).map(|(ps, i)| &ps[*i]).collect();
                let written = |mut cells: Vec<String>| {
                    cells.extend(std::iter::repeat_n("?".to_string(), outputs));
                    format!("| {} |", cells.join(" | "))
                };
                let cells: Option<Vec<String>> = picked.iter().map(|p| p.cell.clone()).collect();
                let row = cells.map(written);
                let wide = row.is_none().then(|| {
                    let (cells, span) = picked
                        .iter()
                        .zip(&shown)
                        .map(|(p, &ai)| match (&p.cell, &p.wide) {
                            (Some(cell), _) => (cell.clone(), p.own.clone()),
                            (None, Some((cell, span))) => (cell.clone(), span.clone()),
                            (None, None) => ("-".to_string(), (0..self.axes[ai].len()).collect()),
                        })
                        .unzip();
                    (written(cells), span)
                });
                let own = picked.iter().map(|p| p.own.clone()).collect();
                let said: Vec<String> = picked.iter().filter_map(|p| p.words.clone()).collect();
                let words = if said.is_empty() { tr!("すべての入力", "every input") } else { said.join(if ja { "、" } else { ", " }) };
                let point = picked.iter().all(|p| p.point);
                out.push(GapRow { row, wide, own, words, point, held: vals.iter().any(Option::is_some) });
                for k in (0..at.len()).rev() {
                    at[k] += 1;
                    if at[k] < pieces[k].len() {
                        continue 'rows;
                    }
                    at[k] = 0;
                }
                break;
            }
        }
        out
    }

    /// All of what the cells of rows `i` and `j` share, as a note beside the one example of it
    /// (§15.317): that the two take the same inputs, that one takes everything the other does,
    /// or the inputs both take, in words, column by column. `ni` and `nj` are the rows as the
    /// finding names them. None where the two share one input, which the example says already.
    fn both_take(&self, i: usize, j: usize, ni: &str, nj: &str) -> Option<String> {
        let ja = crate::i18n::ja();
        let within = |a: usize, b: usize| (0..self.axes.len()).all(|ai| (0..self.axes[ai].len()).all(|c| !self.masks[a][ai][c] || self.masks[b][ai][c]));
        match (within(i, j), within(j, i)) {
            (true, true) => return Some(tr!("二つの行のセルは、同じ入力を取ります。", "The cells of the two rows take the same inputs.")),
            (true, false) => return Some(tr!("{ni} のセルが取る入力は、どれも {nj} のセルも取ります。", "Every input the cells of {ni} take, those of {nj} take too.")),
            (false, true) => return Some(tr!("{nj} のセルが取る入力は、どれも {ni} のセルも取ります。", "Every input the cells of {nj} take, those of {ni} take too.")),
            (false, false) => {}
        }
        let mut shown: Vec<usize> = (0..self.axes.len()).collect();
        shown.sort_by_key(|&ai| self.display_of[ai]);
        let mut said: Vec<String> = Vec::new();
        let mut point = true;
        for ai in shown {
            let set: Vec<usize> = (0..self.axes[ai].len()).filter(|&c| self.masks[i][ai][c] && self.masks[j][ai][c]).collect();
            if set.is_empty() {
                return None;
            }
            let pieces = self.pieces(ai, &set);
            point &= pieces.len() == 1 && pieces[0].point;
            let words: Vec<String> = pieces.into_iter().filter_map(|p| p.words).collect();
            if !words.is_empty() {
                said.push(words.join(if ja { "、または " } else { " or " }));
            }
        }
        (!point && !said.is_empty()).then(|| tr!("両方の行のセルが取る入力: {}", "The cells of both rows take: {}", said.join(if ja { "、" } else { ", " })))
    }

    /// The coordinates `set` of axis `ai` (in order) as the cells that take them, with the same
    /// in words: one piece where one cell does, more where it takes several.
    fn pieces(&self, ai: usize, set: &[usize]) -> Vec<Piece> {
        let name = &self.col_names[ai];
        let ja = crate::i18n::ja();
        let whole = set.len() == self.axes[ai].len();
        if whole {
            return vec![Piece::exact("-".to_string(), None, set.iter().copied().collect(), false)];
        }
        let list = |vs: &[String]| vs.join(if ja { "、" } else { ", " });
        // In Japanese a column's share is `名前が 値`: the name as the subject, spaced from the
        // particle where it ends in a Latin letter, and the value after a space.
        let ga = ritsu_base::text::ja_spacing(&format!("{name}が"));
        let either = |vs: &[String]| match vs {
            [one] => tr!("{ga} {one}", "{name} = {one}"),
            _ if ja => format!("{ga} {} のどれか", vs.join(", ")),
            _ => format!("{name} one of {}", list(vs)),
        };
        let absent = |i: usize| Piece::exact(crate::kw::NONE.to_string(), Some(tr!("{ga}無い", "{name} left out")), BTreeSet::from([i]), true);
        match &self.axes[ai] {
            Axis::Bool => set.iter().map(|&i| (i, self.axes[ai].witness(i))).map(|(i, v)| Piece::exact(v.clone(), Some(either(&[v])), BTreeSet::from([i]), true)).collect(),
            Axis::Enum { values } => {
                let mut out = Vec::new();
                let mut present: Vec<(usize, String)> = Vec::new();
                for &i in set {
                    match values.get(i).map(String::as_str) {
                        Some(crate::kw::NONE) => out.push(absent(i)),
                        Some(v) => present.push((i, v.to_string())),
                        None => {}
                    }
                }
                // the two truth values of a column that may be absent are a cell each: together
                // they are every value that is there, which no one cell says
                if present.iter().any(|(_, v)| v == crate::kw::TRUE || v == crate::kw::FALSE) {
                    out.extend(present.iter().map(|(i, v)| Piece::exact(v.clone(), Some(either(std::slice::from_ref(v))), BTreeSet::from([*i]), true)));
                } else if !present.is_empty() {
                    let names: Vec<String> = present.iter().map(|(_, v)| v.clone()).collect();
                    out.push(Piece::exact(names.join(", "), Some(either(&names)), present.iter().map(|(i, _)| *i).collect(), names.len() == 1));
                }
                out
            }
            Axis::Prefix { patterns } => {
                let quoted = |i: usize| format!("\"{}\"", patterns[i]);
                // The classes are "under this prefix and under no longer one the table writes"
                // and "under none". `starts_with` takes a prefix's class and those of the longer
                // ones, so it is the cell of a class only where the table writes no longer one;
                // and no cell takes the strings under none.
                let longer = |i: usize| -> Vec<usize> {
                    let over: Vec<usize> = (0..patterns.len()).filter(|&j| j != i && patterns[j].starts_with(patterns[i].as_str())).collect();
                    // the shortest of them say it: a string under a longer one is under these
                    over.iter().copied().filter(|&j| !over.iter().any(|&k| k != j && patterns[j].starts_with(patterns[k].as_str()))).collect()
                };
                let named: Vec<usize> = set.iter().copied().filter(|&i| i < patterns.len()).collect();
                let none_of = set.iter().any(|&i| i >= patterns.len());
                // the prefixes named, each with the longer ones its class leaves out
                let plain = named.iter().all(|&i| longer(i).is_empty());
                let each: Vec<String> = named
                    .iter()
                    .map(|&i| {
                        let but: Vec<String> = longer(i).into_iter().map(quoted).collect();
                        match (but.is_empty(), ja) {
                            (_, true) if plain => quoted(i),
                            (true, true) => format!("{} で始まる", quoted(i)),
                            (false, true) => format!("{} で始まる（{} で始まるものを除く）", quoted(i), but.join(", ")),
                            (true, false) => quoted(i),
                            (false, false) => format!("{} (but not {})", quoted(i), but.join(", ")),
                        }
                    })
                    .collect();
                let under = match (ja, plain, each.len()) {
                    (_, _, 0) => None,
                    (true, true, 1) => Some(format!("{} で始まる", each[0])),
                    (true, true, _) => Some(format!("{} のどれかで始まる", each.join(", "))),
                    (true, false, _) => Some(each.join("か、")),
                    (false, ..) => Some(each.join(" or ")),
                };
                let words = match (under, none_of, ja) {
                    (Some(u), false, true) => format!("{ga} {u}"),
                    (Some(u), false, false) => format!("{name} starting with {u}"),
                    (Some(u), true, true) => format!("{ga} {u}か、表の書くどの前置きでも始まらない"),
                    (Some(u), true, false) => format!("{name} starting with {u} or with none of the prefixes the table writes"),
                    (None, _, true) => format!("{ga}、表の書くどの前置きでも始まらない"),
                    (None, _, false) => format!("{name} starting with none of the prefixes the table writes"),
                };
                let own: BTreeSet<usize> = set.iter().copied().collect();
                let cell = format!("{} {}", crate::kw::STARTS_WITH, named.iter().map(|&i| quoted(i)).collect::<Vec<_>>().join(", "));
                // what `starts_with` of the prefixes named takes: their classes and the longer ones'
                let span: BTreeSet<usize> = (0..patterns.len()).filter(|&j| named.iter().any(|&i| patterns[j].starts_with(patterns[i].as_str()))).collect();
                vec![if none_of {
                    Piece { cell: None, words: Some(words), own, wide: None, point: false }
                } else if span == own {
                    Piece::exact(cell, Some(words), own, false)
                } else {
                    Piece { cell: None, words: Some(words), own, wide: Some((cell, span)), point: false }
                }]
            }
            Axis::Num { coords, .. } => {
                let mut out = Vec::new();
                // the runs of coordinates next to one another, the absent value apart
                let mut runs: Vec<(usize, usize)> = Vec::new();
                for &i in set {
                    if matches!(coords.get(i), Some(Coord::Absent)) {
                        out.push(absent(i));
                        continue;
                    }
                    match runs.last_mut() {
                        Some((_, hi)) if *hi + 1 == i => *hi = i,
                        _ => runs.push((i, i)),
                    }
                }
                out.extend(runs.into_iter().map(|(lo, hi)| self.run_piece(ai, lo, hi)));
                out
            }
        }
    }

    /// The coordinates `lo..=hi` of the numeric axis `ai`, none of them the absent value, as one
    /// cell and in words. An end at the end of the axis is left unwritten in the cell (the range
    /// ends there), unless the column may be absent and the run is every value that is there:
    /// a cell with no comparison would be `-`, which takes the absent value too.
    /// The value `v` inside the coordinate `ci` of the numeric axis `ai`, one a table above writes,
    /// as the cell that takes it and in words (§15.331).
    fn value_piece(&self, ai: usize, ci: usize, v: Rat) -> Piece {
        let name = &self.col_names[ai];
        let axis = &self.axes[ai];
        let lit = axis.witness_said_at(usize::MAX, Some(v));
        let ga = ritsu_base::text::ja_spacing(&format!("{name}が"));
        let words = tr!("{ga} {}", "{name} = {}", lit);
        Piece { cell: axis.writable(v).then_some(lit), words: Some(words), own: BTreeSet::from([ci]), wide: None, point: true }
    }

    fn run_piece(&self, ai: usize, lo: usize, hi: usize) -> Piece {
        let Axis::Num { coords, step, date, .. } = &self.axes[ai] else { return Piece { cell: None, words: None, own: BTreeSet::new(), wide: None, point: false } };
        let name = &self.col_names[ai];
        let axis = &self.axes[ai];
        let lit = |v: Rat| axis.witness_said_at(usize::MAX, Some(v));
        // in a cell, a boundary the table writes is written the way the table writes it
        let in_cell = |v: Rat| self.written[ai].iter().find(|(w, _)| w.cmp_to(v) == std::cmp::Ordering::Equal).map(|(_, text)| text.clone()).unwrap_or_else(|| lit(v));
        let first = coords.iter().position(|c| !matches!(c, Coord::Absent)).unwrap_or(0);
        let last = coords.len() - 1;
        // each end as a cell compares it, and whether the value is the run's own (`>=`, `<=`)
        let low: Option<(bool, Rat)> = match &coords[lo] {
            Coord::Point(a) => Some((true, *a)),
            Coord::Open(Some(a), _) => Some((false, *a)),
            _ => None,
        };
        let high: Option<(bool, Rat)> = match &coords[hi] {
            Coord::Point(b) => Some((true, *b)),
            Coord::Open(_, Some(b)) => Some((false, *b)),
            _ => None,
        };
        let held = |v: Rat| axis.writable(v);
        // In words, an end that is not the run's own is said by the value one step in, where the
        // step says which value that is: `>500JPY` is "from 501JPY".
        let said = |end: Option<(bool, Rat)>, up: bool| -> Option<(bool, Rat)> {
            let (own, v) = end?;
            if own {
                return Some((true, v));
            }
            let next = if up { v.checked_add(*step) } else { v.checked_sub(*step) };
            match next {
                Some(n) if held(v) && held(n) => Some((true, n)),
                _ => Some((false, v)),
            }
        };
        let (from, to) = (said(low, true), said(high, false));
        let ja = crate::i18n::ja();
        let ga = ritsu_base::text::ja_spacing(&format!("{name}が"));
        let ni = ritsu_base::text::ja_spacing(&format!("{name}に"));
        let words = match (from, to) {
            (Some((true, a)), Some((true, b))) if a.cmp_to(b) == std::cmp::Ordering::Equal => tr!("{ga} {}", "{name} = {}", lit(a)),
            (Some((true, a)), Some((true, b))) => tr!("{ga} {} から {}", "{name} from {} to {}", lit(a), lit(b)),
            (None, None) => tr!("{ni}値がある", "every value of {name}"),
            (from, to) => {
                let lower = from.map(|(own, a)| match (own, *date, ja) {
                    (true, false, true) => format!("{} 以上", lit(a)),
                    (true, false, false) => format!("{} or more", lit(a)),
                    (false, false, true) => format!("{} より大きい", lit(a)),
                    (false, false, false) => format!("above {}", lit(a)),
                    (true, true, true) => format!("{} 以降", lit(a)),
                    (true, true, false) => format!("on or after {}", lit(a)),
                    (false, true, true) => format!("{} より後", lit(a)),
                    (false, true, false) => format!("after {}", lit(a)),
                });
                let upper = to.map(|(own, b)| match (own, *date, ja) {
                    (true, false, true) => format!("{} 以下", lit(b)),
                    (true, false, false) => format!("{} or less", lit(b)),
                    (false, false, true) => format!("{} より小さい", lit(b)),
                    (false, false, false) => format!("below {}", lit(b)),
                    (true, true, true) => format!("{} 以前", lit(b)),
                    (true, true, false) => format!("on or before {}", lit(b)),
                    (false, true, true) => format!("{} より前", lit(b)),
                    (false, true, false) => format!("before {}", lit(b)),
                });
                // the low end as the Japanese joins it to the high end
                let joined = from.map(|(own, a)| match (own, *date) {
                    (true, false) => format!("{} 以上で ", lit(a)),
                    (false, false) => format!("{} より大きく ", lit(a)),
                    (true, true) => format!("{} 以降で ", lit(a)),
                    (false, true) => format!("{} より後で ", lit(a)),
                });
                match (lower, upper, ja) {
                    (Some(_), Some(u), true) => format!("{ga} {}{u}", joined.unwrap_or_default()),
                    (Some(l), Some(u), false) => format!("{name} {l} and {u}"),
                    (Some(x), None, true) | (None, Some(x), true) => format!("{ga} {x}"),
                    (Some(x), None, false) | (None, Some(x), false) => format!("{name} {x}"),
                    (None, None, _) => tr!("{ni}値がある", "every value of {name}"),
                }
            }
        };
        // the cell: one value as itself, a run as the comparison between its ends
        let end = |e: Option<(bool, Rat)>, up: bool| -> Option<String> {
            let (own, v) = e?;
            let op = match (up, own) {
                (true, true) => ">=",
                (true, false) => ">",
                (false, true) => "<=",
                (false, false) => "<",
            };
            held(v).then(|| format!("{op}{}", in_cell(v)))
        };
        let cell = (|| -> Option<String> {
            // one value: the value itself, whether the run is a point or the one value the
            // step leaves between two boundaries
            if let (Some((true, a)), Some((true, b))) = (from, to)
                && a.cmp_to(b) == std::cmp::Ordering::Equal
            {
                return held(a).then(|| in_cell(a));
            }
            match (lo == first, hi == last) {
                // every value that is there, of a column that may be absent (the whole of a
                // column that may not is `-`): one comparison says so, at an end the range has
                (true, true) => end(low, true).or_else(|| end(high, false)),
                (true, false) => end(high, false),
                (false, true) => end(low, true),
                (false, false) => Some(format!("{} {}", end(low, true)?, end(high, false)?)),
            }
        })();
        let point = matches!((from, to), (Some((true, a)), Some((true, b))) if a.cmp_to(b) == std::cmp::Ordering::Equal);
        Piece { cell, words: Some(words), own: (lo..=hi).collect(), wide: None, point }
    }
}

/// Whether a rule reads a collection at all: what the definitions say is not looked for otherwise.
fn reads_a_collection(f: &RuleFile) -> bool {
    f.inputs.iter().any(|i| i.from.as_ref().is_some_and(|p| !matches!(p.kind, crate::ast::ProjKind::Field))) || f.aggs().next().is_some()
}

/// `(column, value)` pairs as the input half of a witness.
fn pairs_to_witness(ps: Vec<(String, crate::diag::WVal)>) -> crate::diag::Witness {
    crate::diag::Witness { inputs: ps, ..Default::default() }
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
    /// reaches beyond what it takes precedence over. The page for people tells them apart.
    pub edge_pairs: Vec<(usize, usize, bool)>,
    /// The rows that got E102 (0-based). They never match, so §9.2's row coverage leaves them
    /// out.
    pub dead: Vec<usize>,
    /// The rows that got W128 (0-based): no input was found for them, and none was shown not to
    /// reach them (§15.209). §9.2's row coverage says so of them.
    pub undecided: Vec<usize>,
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
        Cell::Ranked { ops, .. } => crate::ast::ranked_text(ops),
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
        Cell::Ranked { ops, .. } => ops
            .iter()
            .map(|(o, v)| format!("{} {v}", o.word()))
            .collect::<Vec<_>>()
            .join(if crate::i18n::ja() { " かつ " } else { " and " }),
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
        // Two rows that both answer that there is no value answer the same (§15.249).
        (OutCell::Nothing, OutCell::Nothing) => true,
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
/// The inputs behind the gaps of rows no search of the vectors reached: for each row of each
/// definition set the inputs reach before any walk, with a cell on a computed column, that
/// `covered` says no vector fires, the input `input_by_gap` finds (§15.209, refined). The vectors
/// take them in, so the generated code is run on the row too.
pub fn gap_inputs(f: &RuleFile, c: &Checked, covered: &dyn Fn(&str, usize) -> bool) -> Vec<(String, usize, BTreeMap<String, crate::eval::Val>)> {
    let mut out = Vec::new();
    if f.walks() || !f.elements.is_empty() {
        return out;
    }
    let computed = |col: &str| f.open_value(col).is_none() && f.items.iter().any(|it| matches!(it, Item::Derived(d) if d.name.text == col) || matches!(it, Item::Define(d) if d.name.text == col));
    for set in &c.sets {
        let t = &set.table;
        let asks: Vec<usize> = (0..t.rows.len())
            .filter(|&i| set.applied[set.member_of[i]].is_none())
            .filter(|&i| !covered(set.row_table(i), t.rows[i].index))
            .filter(|&i| t.inputs.iter().enumerate().any(|(ci, (col, _))| !matches!(t.rows[i].cells.get(ci), None | Some(Cell::DontCare)) && computed(col)))
            .collect();
        if asks.is_empty() {
            continue;
        }
        let Some(reg) = TableRegion::build(t, c, f) else { continue };
        if reg.axes.is_empty() || reg.unanalyzable.is_some() {
            continue;
        }
        let ups = upstreams(t, c, f);
        for i in asks {
            if let Some(a) = reg.input_by_gap(set, c, f, &ups, i, DEFAULT_BUDGET) {
                out.push((set.row_table(i).to_string(), t.rows[i].index, a));
            }
        }
    }
    out
}

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
    let mut undecided: Vec<usize> = Vec::new();
    // The rows the sieve leaves out with no derived or `define` value taking part: a `constraint`,
    // a `never`, koyomi's days or the tables above alone (§15.189). They pass, and are not
    // undecided either (§15.308).
    let mut out_unsaid: Vec<usize> = Vec::new();
    let empty = TableCheck {
        diags: Vec::new(),
        w114: Vec::new(),
        quiet: Vec::new(),
        shadow,
        nodes: 0,
        overlaps: Vec::new(),
        edge_pairs: Vec::new(),
        dead: Vec::new(),
        undecided: Vec::new(),
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
                    .note(tr!("この表の完全性も重なりも検査できていません。黙って通さずに、ここで止めます。", "Neither the completeness nor the overlaps of this table have been checked. Stopping is better than passing it silently."))
                    .note(tr!("列の型を、列挙・真偽・数量・金額・率・日付・それらの optional のいずれかにしてください。", "Give the column one of these types: an enum, boolean, quantity, money, rate, date, or an optional of one of those.")),
            ],
            quiet: Vec::new(),
            shadow: Shadow::default(),
            nodes: 0,
            overlaps: Vec::new(),
            edge_pairs: Vec::new(),
            dead: Vec::new(),
            undecided: Vec::new(),
        };
    }
    let ups = upstreams(t, c, f);
    let merged = set.merged();
    // Every input the columns are worked out from, run once where an example is wanted that
    // nothing else gave (§15.322).
    let walk_cell: std::cell::OnceCell<Option<Walked>> = std::cell::OnceCell::new();
    let walked = || walk_cell.get_or_init(|| reg.walk_inputs(t, c, f)).as_ref();
    // What is said where running every input found none, and why the finding stays (§15.322).
    let kept_anyway = || {
        tr!(
            "検査は、証明書に載せて再検査で確かめられることだけで決めます。入力を全部流した結果はそこに入らないので、この指摘は残ります。",
            "The check decides only by what its certificate carries for a re-checker to confirm, and a run of every input is not among it, so the finding stays."
        )
    };
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
            // The rule's linear model rules the whole of what both rows take out: no input
            // satisfying its derives and constraints reaches both (§15.141).
            if reg.refutes_pair(i, j) {
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
            let mut wpath = match found {
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
            let mut w = reg.witness_said(&wpath);
            // The values the example shows, where an input was run for them (§15.322).
            let mut wvals: Option<Vec<Option<Rat>>> = None;
            // Whether the example is an input: a point of a table that reads only what callers
            // send is one, and on a column no caller sends it is one only where an input was
            // built and the reference evaluator ran it.
            let mut an_input = reg.col_names.iter().all(|n| f.inputs.iter().any(|x| x.name.text == *n));
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
            let mut built: Option<BTreeMap<String, crate::eval::Val>> = None;
            let mut ruled_out = false;
            // Whether running every input found none both rows take.
            let mut ran_none = false;
            if free_axis && feas != Feasible::No {
                nodes += (reg.axes.len() * 8) as i64;
                match crate::vectors::pair_witness(f, c, t, i, j) {
                    Some(a) => {
                        feas = Feasible::Yes;
                        // The example is where the input lands, with the values it gives.
                        if let Some(cols) = crate::vectors::columns_at(f, c, t, &a)
                            && let Some((p, v)) = reg.landing(&cols)
                        {
                            wpath = p;
                            w = reg.witness_said_with(&wpath, Some(v.clone()));
                            wvals = Some(v);
                            an_input = true;
                        }
                        built = Some(a);
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
            // A pair on a column no caller sends, which nothing above has built an input for: the
            // example the point's own values make is kept where an input makes it as it reads, and
            // otherwise an input is built inside what both rows take, and the example shows the
            // values it gives (§15.322). One that cannot be built leaves the example a combination
            // of the columns, and it is not called an input.
            if !an_input && feas != Feasible::No && !(!set.same_member(i, j) && set.comparable(i, j)) {
                let both = |cols: &BTreeMap<String, crate::eval::Val>| crate::vectors::row_holds_at(c, t, &t.rows[i], cols) && crate::vectors::row_holds_at(c, t, &t.rows[j], cols);
                if let Some(a) = reg.made_as_shown(&wpath, t, c, f, &both) {
                    an_input = true;
                    built = Some(a);
                } else {
                    // The point the walk met first, and then the others both rows take, in turn.
                    let mut left = budget;
                    let mut points = vec![wpath.clone()];
                    points.extend(reg.pair_points(i, j, &ups, c, &mut left, GAP_POINTS_TRIED).into_iter().filter(|p| *p != wpath));
                    for p in points {
                        if let Some((a, cols)) = reg.input_inside(&p, t, &ups, c, f, &both) {
                            nodes += (reg.axes.len() * 8) as i64;
                            feas = Feasible::Yes;
                            let v = reg.vals_of(&cols);
                            wpath = p;
                            w = reg.witness_said_with(&wpath, Some(v.clone()));
                            wvals = Some(v);
                            an_input = true;
                            built = Some(a);
                            break;
                        }
                    }
                    // Every input the columns are worked out from, when they are few.
                    if !an_input && let Some(wk) = walked() {
                        match wk.both.get(&(i, j)) {
                            Some((p, v, a)) => {
                                feas = Feasible::Yes;
                                wpath = p.clone();
                                w = reg.witness_said_with(&wpath, Some(v.clone()));
                                wvals = Some(v.clone());
                                an_input = true;
                                built = Some(a.clone());
                            }
                            None => ran_none = wk.apart(i, j),
                        }
                    }
                }
            }
            // The point both rows take, as W114 hands it over with the condition it names.
            let corner = match &wvals {
                Some(v) => pairs_to_witness(reg.witness_pairs_with(&wpath, Some(v.clone()))),
                None => pairs_to_witness(reg.witness_pairs(&wpath)),
            };
            // What E105 and W105 hand over: an input, or nothing where none was built.
            let wit = if an_input { corner.clone() } else { crate::diag::Witness::default() };
            // The example in words: what both rows take, and the input behind it where the input
            // says more than the example does — an input the table does not read (§15.322).
            let example = if an_input {
                vec![tr!("両方に当てはまる例: {w}", "Both rows match: {w}")]
            } else if ran_none {
                vec![
                    tr!("両方の行が取る組み合わせ: {w}", "A combination both rows take: {w}"),
                    tr!(
                        "入力を全部、参照の評価器で流しても、両方の行に当てはまる入力はありませんでした。",
                        "Running every input through the reference evaluator found none that matches both rows."
                    ),
                ]
            } else {
                vec![
                    tr!("両方の行が取る組み合わせ: {w}", "A combination both rows take: {w}"),
                    tr!(
                        "この組み合わせになる入力は見つからず、無いことも示せませんでした。",
                        "No input that gives it was found, and it could not be shown that none does."
                    ),
                ]
            };
            // The first line of E105 and W105: an input both rows match, where there is one to
            // show, and otherwise what is known, the combination both rows take.
            let overlap_head = || {
                if an_input {
                    tr!("行の重なり: 同じ入力が {} と {} の両方に当てはまります", "Overlapping rows: the same input matches {} and {}", rn(i), rn(j))
                } else {
                    tr!("行の重なり: {} と {} の両方に当てはまる組み合わせがあります", "Overlapping rows: {} and {} take a common combination", rn(i), rn(j))
                }
            };
            let input_line = built.as_ref().filter(|a| an_input && a.keys().any(|n| !reg.col_names.contains(n))).map(|a| {
                let b = a.iter().map(|(n, v)| format!("{n} = {}", crate::vectors::show_named(c, n, v))).collect::<Vec<_>>().join(", ");
                tr!("この例を作る入力: {b}", "An input producing this example: {b}")
            });
            // What orders the pair: the policy of their table when they share one, the
            // declared precedence when they do not. Rows come in evaluation order, so an
            // ordered pair `i < j` always has `i` winning.
            let same = set.same_member(i, j);
            let ordered = set.comparable(i, j);
            let tn_j = set.row_table(j).to_string();
            if !same && ordered {
                // A declared precedence between two tables. The overlap is what the
                // `overrides` line is for, so nothing is reported; the pair is kept for the
                // coverage obligation and for the page for people, which says whether the
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
                                OutCell::Nothing => crate::kw::NONE.into(),
                                _ => String::new(),
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    out.push(
                        Diag::warning(
                            "W114",
                            if ran_none {
                                tr!("未確認の重なり: {} と {} の両方に当てはまる組み合わせがあります", "Unconfirmed overlap: {} and {} take a common combination", rn(i), rn(j))
                            } else {
                                tr!("未確認の重なり: {} と {} の両方に当てはまる入力が有り得ます", "Unconfirmed overlap: an input may match both {} and {}", rn(i), rn(j))
                            },
                        )
                        .at(at(t.rows[j].span.line, set.member_of[j]))
                        .table(tn_j.clone())
                        .rowref(set.row_table(i).to_string(), t.rows[i].index)
                        .rowref(tn_j.clone(), t.rows[j].index)
                        .wit(corner)
                        .mark(t.rows[i].span.clone(), rn(i))
                        .mark(t.rows[j].span.clone(), rn(j))
                        .note(tr!("重なる条件: {}", "Overlapping condition: {}", reg.overlap_text(&t.rows[i], &t.rows[j])))
                        .note(if ran_none {
                            // what running every input found, and the guard the check keeps
                            tr!(
                                "入力を全部、参照の評価器で流しても、両方の行に当てはまる入力はありませんでした。検査は、証明書に載せて再検査で確かめられることだけで決めるので、この重なりは確かめられないまま残ります。",
                                "Running every input through the reference evaluator found none that matches both rows. The check decides only by what its certificate carries for a re-checker to confirm, so the overlap stays unconfirmed."
                            )
                        } else if touches_define {
                            tr!("定義の中身まで含めて、この条件を同時に満たす入力を構成できませんでした。存在しないことの証明ではありません。", "No input satisfying this condition, definitions included, could be constructed. This is not a proof that none exists.")
                        } else {
                            tr!(
                                "導出（{}）が入力を共有しているため、この条件を同時に満たす入力が存在するかどうかを、検査は判定できませんでした。",
                                "Because the derived values ({}) share inputs, the check could not decide whether an order satisfying this condition exists.",
                                dnames.join(if crate::i18n::ja() { "、" } else { ", " })
                            )
                        })
                        .note(if ran_none {
                            String::new()
                        } else {
                            tr!(
                                "存在するなら: 行を直してください。出力が異なる（{} と {}）ので、当てはまれば矛盾です。",
                                "If one exists: fix the rows. The outputs differ ({} vs {}), so a match would be a contradiction.",
                                outs(&t.rows[i]),
                                outs(&t.rows[j])
                            )
                        })
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
                        Diag::error("E105", overlap_head())
                            .at(at(t.rows[j].span.line, set.member_of[j]))
                            .table(tn_j.clone())
                            .rowref(set.row_table(i).to_string(), t.rows[i].index)
                            .rowref(tn_j.clone(), t.rows[j].index)
                            .wit(wit)
                            .mark(t.rows[i].span.clone(), rn(i))
                            .mark(t.rows[j].span.clone(), rn(j))
                            // all of what the two share, before the one input of it
                            .maybe_note(reg.both_take(i, j, &rn(i), &rn(j)))
                            .notes_from(example.clone())
                            .maybe_note(ran_none.then(kept_anyway))
                            // §6.2: a witness that involves definitions is shown only after an
                            // input has been constructed and confirmed by the evaluator. Present it
                            // in a form that can be copied.
                            .maybe_note(input_line.clone())
                            .note(if !same {
                                // A clause has no `policy` line: its `overrides` goes under it, as
                                // a line of its own (§15.208).
                                let (ci, cj) = (set.is_clause_row(i), set.is_clause_row(j));
                                let (ki, kj) = (set.kind_word(set.member_of[i]), set.kind_word(set.member_of[j]));
                                let (ti, tj) = (set.row_table(i), set.row_table(j));
                                let (p, o) = (crate::kw::POLICY, crate::kw::OVERRIDES);
                                match (ci, cj) {
                                    (false, false) => tr!(
                                        "表 {ti} と 表 {tj} のどちらが優先するか書かれていません。優先する表の `{p}` の次に `{o} <相手>` を書いてください。順序に意味が無いなら、どちらかの行を直してください。",
                                        "It is not written whether table {ti} or table {tj} takes precedence. Put `{o} <the other>` after `{p}` on the table that wins. If no order is meant, fix one of the rows."
                                    ),
                                    (true, true) => tr!(
                                        "節 {ti} と 節 {tj} のどちらが優先するか書かれていません。優先する節の下に `{o} <相手>` の行を書いてください。順序に意味が無いなら、どちらかの条件を直してください。",
                                        "It is not written whether clause {ti} or clause {tj} takes precedence. Add an `{o} <the other>` line under the clause that wins. If no order is meant, fix the condition of one of them."
                                    ),
                                    _ => tr!(
                                        "{ki} {ti} と {kj} {tj} のどちらが優先するか書かれていません。表が優先するなら表の `{p}` の次に、節が優先するなら節の下に、`{o} <相手>` を書いてください。順序に意味が無いなら、どちらかを直してください。",
                                        "It is not written whether {ki} {ti} or {kj} {tj} takes precedence. Put `{o} <the other>` after `{p}` if the table wins, or as a line under the clause if the clause wins. If no order is meant, fix one of the two."
                                    ),
                                }
                            } else if same_len {
                                tr!("`{p} {u}` では重なりは許されません。どちらが正しいか決めるか、順序に意味を持たせるなら `{p} {f}` を宣言してください。", "`{p} {u}` does not allow overlapping rows. Decide which row is right, or declare `{p} {f}` if the order is meant to matter.", p = crate::kw::POLICY, u = crate::kw::UNIQUE, f = crate::kw::FIRST)
                            } else {
                                tr!("`{} {}` では重なりは許されません。", "`{} {}` does not allow overlapping rows.", crate::kw::POLICY, crate::kw::UNIQUE)
                            })
                            .note(if !same && outs_equal(&t.rows[i], &t.rows[j]) {
                                // The same answer today is no reason to let the two overlap: an edit
                                // tomorrow may change one of them and not the other (§4).
                                tr!(
                                    "二つは同じ答えを返します。どちらが決めてもよいなら、一つの `{p} {f}` の表の行にするか、`{o}` で順を書いてください。",
                                    "The two answer the same. If either may decide, make them rows of one `{p} {f}` table, or write the order with `{o}`.",
                                    p = crate::kw::POLICY,
                                    f = crate::kw::FIRST,
                                    o = crate::kw::OVERRIDES
                                )
                            } else {
                                String::new()
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
                    let d = Diag::warning("W105", overlap_head())
                    .at(at(t.rows[j].span.line, set.member_of[j]))
                    .table(tn_j.clone())
                    .rowref(tn_j.clone(), t.rows[i].index)
                    .rowref(tn_j.clone(), t.rows[j].index)
                    .mark(t.rows[i].span.clone(), rn(i))
                    .mark(t.rows[j].span.clone(), rn(j));
                    let d = if confirmed {
                        d.wit(wit)
                            // a row inside the later one is told so below, as a staircase
                            .maybe_note(if contained { None } else { reg.both_take(i, j, &rn(i), &rn(j)) })
                            .notes_from(example)
                            .maybe_note(input_line)
                    } else if ran_none {
                        d.notes_from(example)
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
                    "`{}` は、両方に当てはまる入力があるときに、どちらが勝つかを決めます。交わる行が一つも無いので、この行は何も決めていません。ただし書が本文の一部を切り出す形になっていないときは、規則を転記するときに誤ったことが多いです。",
                    "`{}` decides which wins when an input matches both. No rows meet, so the line decides nothing. That is the usual sign of a proviso transcribed so that it no longer carves out part of the main rule.",
                    crate::kw::OVERRIDES
                )),
        );
    }

    // --- Unreachable rows
    // The third form (§15.189) reads the reach of the derived and `define` columns, the truth
    // value a boolean `define` always takes (§15.196), what a `define` comes to with the axes it
    // reads held (§15.322), and what a table above writes a value under together with them
    // (§15.195). Every axis has a
    // coordinate for each value its column takes since the axes are cut on the step the
    // values take (§15.190), so it reads every table that has such a column or a column a
    // table above decides. Its walk has a budget of its own for the whole table; past it, a row
    // passes as it did before.
    let reach = Some(reg.reach_of()).filter(|r| {
        r.axes.iter().any(|x| *x) || reg.define_truth.iter().any(|t| t.is_some()) || !reg.narrowed.is_empty() || ups.iter().any(|u| reg.above_can(u))
    });
    let mut reach_left = budget;
    // The walks asked again with the derived values first (below) have a budget of their own, so
    // that they take nothing from the walks of the rows after them.
    let mut again_left = budget;
    for i in 0..t.rows.len() {
        // A row every point of which the rows that take precedence and the tables above leave out,
        // with no derived or `define` value, `constraint`, `never` line or day of koyomi's taking
        // part: what the rule declares does not come into it, so no input reaches it (§15.322).
        let mut above_alone: Option<OutOfReach> = None;
        let up_dead = upstream_dead(&t.rows[i], &ups, c, t);
        let winners = &set.beats[i];
        // A row that takes no day of koyomi's on an axis of them never matches (§15.174).
        let no_day = reg.misses_the_days(i).map(|ai| reg.col_names[ai].clone());
        let dead = if reg.empty(i) || up_dead || no_day.is_some() {
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
        // What the first two forms let through, the reach of a derived column may still leave
        // out: every point of the row is taken by the rows that take precedence, or reached by
        // no input (§15.189).
        let beyond = match &reach {
            Some(rs) if !dead => {
                let meet: Vec<usize> = winners.iter().copied().filter(|&e| reg.intersects(e, i)).collect();
                let before = reach_left;
                let why = reg.out_of_reach(&meet, i, &ups, c, rs, &mut reach_left);
                // The walk writes down, for each point it leaves out, the first test that holds,
                // and it takes the axes in the order of their sizes (§6.3): a `constraint` on two
                // input axes can leave out every point before the walk comes to the axis of a
                // derive that leaves the row out on its own. Whether a derived or `define` value
                // takes part is asked again with those values first, so that it does not hang on
                // the order (§15.308). A row the second walk cannot finish within its budget is
                // left out all the same, by the first.
                let why = match why {
                    Some(w) if !w.by_a_derive() => {
                        let first = Reach { axes: rs.axes.clone(), derives_first: true };
                        let again_before = again_left;
                        let again = reg.out_of_reach(&meet, i, &ups, c, &first, &mut again_left);
                        nodes += (again_before - again_left).max(0);
                        // One a `constraint` line or a `never` line takes part in passes, as the author
                        // declared it (§15.309); `rulec coverage` names it.
                        let plain = |x: &OutOfReach| {
                            x.upstream && x.constraints.is_empty() && x.model_constraints.is_empty() && x.nevers.is_empty() && x.days.is_empty()
                        };
                        match again {
                            Some(again) if again.by_a_derive() => Some(again),
                            again => {
                                if plain(&w) {
                                    above_alone = Some(w);
                                } else if let Some(a) = again.filter(|a| plain(a)) {
                                    above_alone = Some(a);
                                } else {
                                    out_unsaid.push(i);
                                }
                                None
                            }
                        }
                    }
                    why => why,
                };
                nodes += (before - reach_left).max(0);
                why.filter(|w| w.by_a_derive())
            }
            _ => None,
        };
        if dead || beyond.is_some() || above_alone.is_some() {
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
            let d = Diag::error("E102", tr!("{} はどの入力にも当てはまりません", "Unreachable row: {} never matches", rn(i)))
                .at(at(t.rows[i].span.line, set.member_of[i]))
                .table(tn.clone())
                .row(t.rows[i].index)
                .rowref(tn.clone(), t.rows[i].index)
                .fix_kind(crate::diag::FixKind::RemoveRow)
                .mark(t.rows[i].span.clone(), tr!("{}: ここに到達する入力はありません", "{}: no input reaches here", rn(i)));
            if let Some(w) = &beyond {
                out.push(third_form(d, w, &reg, i, c, f, by_position, &winner_tables));
                continue;
            }
            if let Some(w) = &above_alone {
                let meet: Vec<usize> = winners.iter().copied().filter(|&e| reg.intersects(e, i)).collect();
                if let Some(why) = unsaid_text(set, c, f, &reg, i, &meet, w) {
                    out.push(d.note(why));
                    continue;
                }
            }
            out.push(
                d
                    .note(if let Some(col) = &no_day {
                        let from = c.day_sets.get(col).and_then(|d| d.from.as_ref()).map(|fr| format!("koyomi \"{}\" date {}", fr.file, fr.date)).unwrap_or_default();
                        tr!("{col} は {from} がとる日だけで、この行の日付はそのどれでもありません。", "{col} takes only the days {from} comes to, and this row's dates are none of them.")
                    } else if reg.unreachable_row[i] {
                        tr!("この行が書いている値を、上流の表は決して出しません。", "The upstream table never produces the values this row names.")
                    } else if up_dead {
                        tr!(
                            "ほかの列がこの行の条件どおりのとき、上流の表はこの行が書いている値を出しません。どちらか一方だけなら起こりますが、両方が同時には起こりません。",
                            "The upstream table does not produce the value this row names while the other columns are what this row says. Either half happens; the two together do not."
                        )
                    } else if by_position {
                        tr!("`{} {}` なので、この行の範囲は、上にある行がすべて先に取ります。", "Because of `{} {}`, the earlier rows take all of this row's range first.", crate::kw::POLICY, crate::kw::FIRST)
                    } else if !winner_tables.is_empty() {
                        tr!("この行の範囲は、優先する {} の行がすべて先に取ります。", "The rows of {}, which take precedence, take all of this row's range first.", winner_tables.join(if crate::i18n::ja() { "、" } else { ", " }))
                    } else {
                        tr!("この行の条件を同時に満たす入力がありません。", "No input satisfies all of this row's conditions at once.")
                    })
                    .note(if no_day.is_some() {
                        tr!("ヒント: koyomi のファイルがこの日をとるようにするか、この行を削除してください。", "hint: make the koyomi file come to such a day, or delete this row.")
                    } else if reg.unreachable_row[i] {
                        tr!("ヒント: 上流の表がこの値を出すようにするか、この行を削除してください。", "hint: make the upstream table produce this value, or delete this row.")
                    } else {
                        tr!("ヒント: 新しい仕様なら上へ移してください。不要なら削除してください。", "hint: if this is a new specification, move it up; if it is not needed, delete it.")
                    }),
            );
        }
    }

    // --- Rows nothing decided (§15.209, W128)
    // A row with a cell on a derived or `define` column: its point on the axes is not an input,
    // and the columns are computed from the inputs by relations the axes do not carry. Neither
    // E102 nor E101 says anything of a row when the sieve leaves it standing, and `ok` then reads as
    // "every row is reached". Where the search the vectors run for a row finds no input either, the
    // row is said to be undecided. A row of inputs and enums alone needs no search: its point is an
    // input. Only a table the inputs reach: one of an element, or one that reads what a walk
    // answers, is reached through a sequence, which this search does not build.
    let stage_of_set = f
        .items
        .iter()
        .position(|it| matches!(it, Item::Table(x) if x.name.as_ref().is_some_and(|n| n.text == set.eval_at)))
        .map(|k| crate::types::stages(f).at(k));
    if stage_of_set.is_none_or(|s| s == crate::types::Stage::BEFORE) {
        // A column a table of this rule decides is computed too: its value is not an input, and the
        // relation to the inputs is that table's rows (§15.322).
        let computed = |col: &str| {
            f.open_value(col).is_none()
                && (f.items.iter().any(|it| matches!(it, Item::Derived(d) if d.name.text == col) || matches!(it, Item::Define(d) if d.name.text == col))
                    || producing_table(f, c, t, col).is_some())
        };
        // A row the sieve leaves out is decided, though nothing reports it: W128 would say that no
        // input was shown not to reach it (§15.308).
        let asks: Vec<(usize, Vec<String>)> = (0..t.rows.len())
            .filter(|i| !dead_rows.contains(i) && !out_unsaid.contains(i) && set.applied[set.member_of[*i]].is_none())
            .filter_map(|i| {
                let cols: Vec<String> = t
                    .inputs
                    .iter()
                    .enumerate()
                    .filter(|(ci, (col, _))| !matches!(t.rows[i].cells.get(*ci), None | Some(Cell::DontCare)) && computed(col))
                    .map(|(_, (col, _))| col.clone())
                    .collect();
                (!cols.is_empty()).then_some((i, cols))
            })
            .collect();
        let rows: Vec<usize> = asks.iter().map(|(i, _)| *i).collect();
        let found_none = crate::vectors::rows_without_input(f, c, set, &rows);
        // A row completeness asks for: with the row taken out, the check finds a gap, and an
        // input that makes the gap's values — no other row takes it, so this one does — is an
        // input the row is reached by. It is looked for the way E101 looks for the input behind a
        // gap (§15.194) and run through the reference evaluator, which has to fire the row. A gap
        // no input makes (two truth values no number gives at once, say) decides nothing, and the
        // row stays undecided; so does a search that runs out of its budget, which is the check's
        // own for each row. Computed columns no linear model ties to the inputs — a remainder, a
        // whole square root, a rounded quotient — are where the search of the vectors misses the
        // most (§15.209's honest limits, the lane of §15.213–§15.215). Last, an input is built
        // inside a box of the row: a column one input moves one way is followed through that
        // input's values to where it comes into the row's cell (§15.322).
        let found_none: Vec<usize> = found_none
            .into_iter()
            .filter(|&i| reg.input_by_gap(set, c, f, &ups, i, budget).is_none() && reg.input_reaching(t, c, f, &ups, i, budget).is_none())
            .collect();
        // Last, every input the columns are worked out from, when they are few: a row one of them
        // reaches is decided, and of one none reaches, that is what is said (§15.322).
        let wk = if found_none.is_empty() { None } else { walked() };
        let found_none: Vec<usize> = found_none.into_iter().filter(|i| !wk.is_some_and(|w| w.reach.contains_key(i))).collect();
        undecided = found_none.clone();
        for (i, cols) in asks.iter().filter(|(i, _)| found_none.contains(i)) {
            let i = *i;
            let tn = set.row_table(i).to_string();
            let cols = cols.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join(if crate::i18n::ja() { "、" } else { ", " });
            let d = if wk.is_some() {
                Diag::warning("W128", tr!("{} に当たる入力はありませんが、検査はそれを示せません", "No input reaches {}, but the check cannot show it", rn(i)))
            } else {
                Diag::warning(
                    "W128",
                    tr!(
                        "{} に当たる入力を見つけられず、当たらないことも示せませんでした",
                        "No input was found that reaches {}, and none was shown not to",
                        rn(i)
                    ),
                )
            };
            let d = d
                .at(at(t.rows[i].span.line, set.member_of[i]))
                .table(tn.clone())
                .row(t.rows[i].index)
                .rowref(tn, t.rows[i].index)
                .mark(t.rows[i].span.clone(), tr!("{}: 決められませんでした", "{}: undecided", rn(i)));
            out.push(if wk.is_some() {
                d.note(tr!(
                    "この行は、計算した列 {cols} の値を求めています。入力を全部、参照の評価器で流しても、この行に当たる入力はありませんでした。検査は、証明書に載せて再検査で確かめられることだけで決めるので、この行は決められないまま残ります。",
                    "This row asks for values of {cols}, which are computed. Running every input through the reference evaluator found none that reaches it. The check decides only by what its certificate carries for a re-checker to confirm, so the row stays undecided."
                ))
                .note(tr!(
                    "この行は削除してかまいません。削除して完全性の欠落（E101）が出るなら、その組み合わせが起きないことを検査は示せないので、行は残してかまいません。",
                    "The row can be deleted. If deleting it brings a completeness gap (E101), the check cannot show that the combination never comes, and the row can stay."
                ))
            } else {
                d.note(tr!(
                    "この行は、計算した列 {cols} の値を求めています。その値になる入力を探しましたが見つからず、どの入力も当たらないことも示せませんでした。",
                    "This row asks for values of {cols}, which are computed. An input that gives them was looked for and not found, and it could not be shown that no input does."
                ))
                .note(tr!(
                    "この行に当たる入力が本当にあるか、計算の式と行の条件を見比べてください。あるなら、その入力を例に書くと、この警告は消えます。無いなら、この行を削除してください。削除して完全性の欠落（E101）が出るなら、その組み合わせが起きないことも rulec は示せていないので、行は残してかまいません。",
                    "Look at the expressions and the row's conditions together to see whether some input reaches it. If one does, write it as an example and this warning goes; if none does, delete the row. If deleting it brings a completeness gap (E101), rulec cannot show that the combination never comes either, and the row can stay."
                ))
            });
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
    // Whether E101 asks a `policy first` table for a last row that overlaps the rows above it.
    let mut asked_to_overlap = false;
    if let Some(hole) = hole {
        let hole = reg.gap_to_show(hole, &all, budget, &ups, c, f, path);
        out.push(
            {
                // Every part of the input no row takes (§15.317).
                let (boxes, counted) = reg.gap_boxes(&all, budget, &ups, c);
                // The values behind the gap, each computed column's inside what it comes to
                // (§15.195): the example is one that occurs.
                let vals = reg.reach_values(&hole);
                // What the reads of a collection say of the gap (§15.281): the example is moved
                // to a combination that occurs, or there is none and the notes say so.
                let (vals, never, a_collection_gives) = reg.behind_the_gap(&hole, &boxes, vals, c, f, path);
                // The input behind the gap: the one the example's own values make, or else one
                // built inside the gap's box, whose values the example then shows (§15.322).
                let (vals, behind) = match reg.input_behind(&hole, vals.clone(), t, c, f) {
                    Some(a) => (vals, Some(a)),
                    None => {
                        let none_takes = |cols: &BTreeMap<String, crate::eval::Val>| t.rows.iter().all(|r| !crate::vectors::row_holds_at(c, t, r, cols));
                        match reg.input_inside(&hole, t, &ups, c, f, &none_takes) {
                            Some((a, cols)) => (Some(reg.vals_of(&cols)), Some(a)),
                            None => (vals, None),
                        }
                    }
                };
                // On a column no caller sends, the example is an input only where one was built
                // and the reference evaluator ran it, or a collection built and evaluated gives the
                // reads of it (§15.322). Where the gap the walk met first gave none, the other parts
                // are tried in turn; with none from any of them, the example is the combination it
                // is, and it is not called an input.
                let reads_computed = reg.col_names.iter().any(|n| !f.inputs.iter().any(|i| i.name.text == *n)) && !a_collection_gives;
                let (hole, vals, behind) = match behind {
                    None if reads_computed => match reg.gap_with_input(&boxes, t, &ups, c, f, budget) {
                        Some((p, v, a)) => (p, Some(v), Some(a)),
                        // every input the columns are worked out from, when they are few
                        None => match walked().and_then(|w| w.gap.as_ref()) {
                            Some((p, v, a)) => (p.clone(), Some(v.clone()), Some(a.clone())),
                            None => (hole, vals, None),
                        },
                    },
                    b => (hole, vals, b),
                };
                let an_input = !reads_computed || behind.is_some();
                // Whether running every input found none that no row takes.
                let ran_none = !an_input && walked().is_some_and(|w| w.no_gap());
                // The example, and the input behind it where the input says more than the example
                // does: an input the table does not read.
                let example = reg.witness_said_with(&hole, vals.clone());
                let example = if an_input {
                    vec![tr!("当てはまらない例: {example}", "An input that matches no row: {example}")]
                } else if !never.is_empty() {
                    // the definitions of the reads of a collection say it never comes, below
                    vec![tr!("当てはまらない組み合わせの例: {example}", "A combination no row takes: {example}")]
                } else if ran_none {
                    vec![
                        tr!("当てはまらない組み合わせの例: {example}", "A combination no row takes: {example}"),
                        tr!(
                            "入力を全部、参照の評価器で流しても、どの行にも当てはまらない入力はありませんでした。",
                            "Running every input through the reference evaluator found none that matches no row."
                        ),
                        kept_anyway(),
                    ]
                } else {
                    vec![
                        tr!("当てはまらない組み合わせの例: {example}", "A combination no row takes: {example}"),
                        tr!(
                            "この組み合わせになる入力は見つからず、無いことも示せませんでした。",
                            "No input that gives it was found, and it could not be shown that none does."
                        ),
                    ]
                };
                let input_line = behind.as_ref().filter(|a| a.keys().any(|n| !reg.col_names.contains(n))).map(|a| {
                    let b = a.iter().map(|(n, v)| format!("{n} = {}", crate::vectors::show_named(c, n, v))).collect::<Vec<_>>().join(", ");
                    tr!("この例を作る入力: {b}", "An input producing this example: {b}")
                });
                // The parts, each as a row cut at the boundaries the table already writes, the
                // ends of the ranges and the values the tables above produce inside a cut, where
                // the column takes those alone (§15.331): the rows close the gap as they are, and
                // no boundary is chosen for whoever writes the table.
                let parts = reg.gap_rows(&boxes, t.outputs.len(), &ups, c, budget);
                let shown = &parts[..parts.len().min(GAP_ROWS_SHOWN)];
                let more = parts.len() - shown.len();
                let rest = match (more, counted) {
                    (0, true) => None,
                    (m, true) => Some(tr!("  ほかに {m} か所", "  and {m} more")),
                    (0, false) => Some(tr!("  ほかにもあります（数え切れませんでした）", "  and more, which were not all counted")),
                    (m, false) => Some(tr!("  ほかに {m} か所より多く（数え切れませんでした）", "  and more than {m} more, which were not all counted")),
                };
                // the parts in words, before the example of one input
                // Where no input was found, what is known is said: combinations of the columns.
                let words: Vec<String> = match shown {
                    [] => Vec::new(),
                    // one part that is one input: the example below says it, and that is enough
                    [one] if rest.is_none() && one.point => Vec::new(),
                    [one] if rest.is_none() && an_input => vec![tr!("どの行にも当てはまらない入力: {}", "No row matches: {}", one.words)],
                    [one] if rest.is_none() => vec![tr!("どの行にも当てはまらない組み合わせ: {}", "No row matches: {}", one.words)],
                    _ => {
                        let n = parts.len();
                        let mut w = vec![match (counted, an_input) {
                            (true, true) => tr!("どの行にも当てはまらない入力が {n} か所あります:", "No row matches these {n} parts of the input:"),
                            (true, false) => tr!("どの行にも当てはまらない組み合わせが {n} か所あります:", "No row matches these {n} parts:"),
                            (false, true) => tr!("どの行にも当てはまらない入力が、次のほかにもあります:", "No row matches these parts of the input, and more:"),
                            (false, false) => tr!("どの行にも当てはまらない組み合わせが、次のほかにもあります:", "No row matches these parts, and more:"),
                        }];
                        w.extend(shown.iter().map(|p| format!("  {}", p.words)));
                        w.extend(rest.clone());
                        w
                    }
                };
                let head = if an_input {
                    tr!("完全性の欠落: どの行にも当てはまらない入力があります", "Completeness gap: some input matches no row")
                } else {
                    tr!("完全性の欠落: どの行にも当てはまらない組み合わせがあります", "Completeness gap: some combination matches no row")
                };
                let d = Diag::error("E101", head)
                    .at(at(head_span.line, 0))
                    .table(anchor.clone())
                    .wit(if an_input { pairs_to_witness(reg.witness_pairs_with(&hole, vals.clone())) } else { crate::diag::Witness::default() })
                    .mark(
                        head_span.clone(),
                        if an_input {
                            tr!("起こりうる入力を網羅していません", "the input space is not fully covered")
                        } else {
                            tr!("列の値の組み合わせを網羅していません", "the combinations of the columns are not all covered")
                        },
                    )
                    .notes_from(words)
                    .notes_from(example)
                    // A gap on a computed column names values no caller sends; the input behind
                    // them is what the generated code stops on (§15.190).
                    .maybe_note(input_line)
                    .notes_from(never);
                // A gap that turns on an input another rule's output feeds (§15.322): the line says
                // where the value comes from, not what that rule was given, so the check reads it
                // free of the other inputs. `apply` writes both, and the check reads the two together.
                let fed: Vec<String> = (0..reg.axes.len())
                    .filter(|&ai| {
                        let col = &reg.col_names[ai];
                        let fed_by_a_rule = f.inputs.iter().any(|i| i.name.text == *col && i.fed.as_ref().is_some_and(|fb| fb.name.tool == ritsu_base::naming::Tool::Rulec));
                        // the parts are written column by column, in the table's order
                        let k = (0..reg.axes.len()).filter(|&a| reg.display_of[a] < reg.display_of[ai]).count();
                        fed_by_a_rule && parts.iter().any(|p| p.own.get(k).is_some_and(|s| s.len() < reg.axes[ai].len()))
                    })
                    .map(|ai| format!("`{}`", reg.col_names[ai]))
                    .collect();
                let d = if fed.is_empty() {
                    d
                } else {
                    // `a`, `a` と `b`, `a`、`b`、`c` — and in English `a`, `a` and `b`, `a`, `b` and `c`
                    let who = match fed.as_slice() {
                        [a] => a.clone(),
                        [a, b] => tr!("{a} と {b}", "{a} and {b}"),
                        _ if crate::i18n::ja() => fed.join("、"),
                        _ => format!("{} and {}", fed[..fed.len() - 1].join(", "), fed[fed.len() - 1]),
                    };
                    d.note(if fed.len() == 1 {
                        tr!(
                            "{who} は、ほかの規則の出力を受け取る入力です（`fed by`）。`fed by` は値の出どころを書くだけで、その規則に何を渡したかは書きません。そのため、この検査は {who} を、ほかの入力とは関係の無い値として読みます。出どころの規則がほかの入力から決める値として読ませるには、その規則を `{apply}` で取り込んでください。",
                            "{who} is an input another rule's output feeds (`fed by`). `fed by` says where the value comes from, not what that rule was given, so this check reads {who} as free of the other inputs. To have it read as what that rule decides from them, bring the rule in with `{apply}`.",
                            apply = crate::kw::APPLY
                        )
                    } else {
                        tr!(
                            "{who} は、ほかの規則の出力を受け取る入力です（`fed by`）。`fed by` は値の出どころを書くだけで、出どころの規則に何を渡したかは書きません。そのため、この検査は {who} を、どれもほかの入力とは関係の無い値として読みます。出どころの規則がほかの入力から決める値として読ませるには、それぞれの規則を `{apply}` で取り込んでください。",
                            "{who} are inputs other rules' outputs feed (`fed by`). `fed by` says where a value comes from, not what that rule was given, so this check reads each of them as free of the other inputs. To have them read as what those rules decide from them, bring the rules in with `{apply}`.",
                            apply = crate::kw::APPLY
                        )
                    })
                };
                let add_rows = || {
                    if an_input {
                        tr!("ヒント: この入力に当てはまる行を足してください。", "hint: add rows that match these inputs.")
                    } else {
                        tr!("ヒント: この組み合わせに当てはまる行を足してください。", "hint: add rows that match these combinations.")
                    }
                };
                let d = if merged {
                    d.note(add_rows()).note(tr!(
                        "{} を合わせても網羅できていません。行はどの表に足してもよく、優先の順序はそのまま効きます。",
                        "{} together do not cover it. The row may go in any of them; the declared precedence still applies.",
                        members_text()
                    ))
                } else {
                    d
                };
                // The rewritten form (§11 principle 3) as data: the rows, their output cells
                // left as `?`. **What a row answers has to come from the written rule**, and a
                // `?` does not pass the check, so a row pasted and left as it is stops there
                // rather than answer with a value nobody chose. A merged set has no one table
                // the rows belong to, so it gets no rows and no `fix.text`.
                let exact: Vec<&str> = shown.iter().filter_map(|p| p.row.as_deref()).collect();
                // The parts no cell names exactly, as the rows that take them at the end of a
                // `policy first` table: in the order of the parts, but a row above every row that
                // spans all of it, and none that the ones before it already take the part of, so
                // that each row written is one some input reaches.
                let mut wide: Vec<&GapRow> = Vec::new();
                for p in shown.iter().filter(|p| p.wide.is_some()) {
                    let inside = |q: &&GapRow| match (&p.wide, &q.wide) {
                        (Some((_, a)), Some((_, b))) => a != b && a.iter().zip(b.iter()).all(|(x, y)| x.is_subset(y)),
                        _ => false,
                    };
                    let at = wide.iter().position(inside).unwrap_or(wide.len());
                    wide.insert(at, p);
                }
                let mut last: Vec<(&str, &[BTreeSet<usize>])> = Vec::new();
                for p in &wide {
                    let Some((row, span)) = &p.wide else { continue };
                    if !box_covered(&p.own, &last.iter().map(|(_, s)| *s).collect::<Vec<_>>()) {
                        last.push((row.as_str(), span.as_slice()));
                    }
                }
                let unwritten = wide.len();
                let no_cell = tr!(
                    "文字列の列に書けるのは `starts_with` と `-` だけで、どの前置きにも当たらない文字列や、表がもっと長い前置きを書いている前置きを、一つのセルでは指せません。数の列は、単位の整数でない境をセルに書けません",
                    "a string column holds `starts_with` and `-` only, and no one cell names the strings under none of the prefixes, or under a prefix the table writes a longer one of; a number column holds no boundary that is not a whole unit"
                );
                let listed = |d: Diag, one: String, many: String, rows: &[&str]| match rows {
                    [row] => d.note(format!("{one} `{row}`")),
                    _ => {
                        let mut d = d.note(many);
                        for row in rows {
                            d = d.note(format!("  {row}"));
                        }
                        d
                    }
                };
                let top_down = set.policies.first() == Some(&Policy::TopDown);
                asked_to_overlap = !merged && top_down && !last.is_empty();
                if merged {
                    d
                } else {
                    // under `policy first` the rows with `-` go with the others, at the end
                    let rows: Vec<&str> = exact.iter().copied().chain(last.iter().filter(|_| top_down).map(|(r, _)| *r)).collect();
                    let d = if rows.is_empty() {
                        // no part was gathered (the search ran out before the first), or none has
                        // a row here: the example stands for the gap
                        d.maybe_note((unwritten == 0).then(add_rows))
                    } else {
                        listed(
                            d,
                            tr!("ヒント: 次の行を足し、`?` に答えを書いてください:", "hint: add this row, with what it answers written in place of `?`:"),
                            tr!("ヒント: 次の行を足し、`?` に答えを書いてください:", "hint: add these rows, with what each answers written in place of `?`:"),
                            &rows,
                        )
                    };
                    let d = if unwritten == 0 {
                        d
                    } else if top_down {
                        d.note(match (rows.len(), last.len()) {
                            (1, _) => tr!(
                                "この行は、表の今の行の後ろに足してください。`policy first` の表では、上の行に当てはまらなかった入力を下の行が取ります。ちょうど取るセルが書けない列には `-` を書いています（{no_cell}）。",
                                "Add it after the rows the table has: under `policy first` a row takes what the rows above it leave. It writes `-` in a column where no cell takes exactly the part ({no_cell})."
                            ),
                            (_, 1) => tr!(
                                "行は、表の今の行の後ろに、この順で足してください。`policy first` の表では、上の行に当てはまらなかった入力を下の行が取ります。終わりの行は、ちょうど取るセルが書けない列に `-` を書いています（{no_cell}）。",
                                "Add them after the rows the table has, in this order: under `policy first` a row takes what the rows above it leave. The last one writes `-` in a column where no cell takes exactly the part ({no_cell})."
                            ),
                            (_, n) => tr!(
                                "行は、表の今の行の後ろに、この順で足してください。`policy first` の表では、上の行に当てはまらなかった入力を下の行が取ります。終わりの {n} 行は、ちょうど取るセルが書けない列に `-` を書いています（{no_cell}）。",
                                "Add them after the rows the table has, in this order: under `policy first` a row takes what the rows above it leave. The last {n} write `-` in a column where no cell takes exactly the part ({no_cell})."
                            ),
                        })
                    } else {
                        let rows: Vec<&str> = last.iter().map(|(r, _)| *r).collect();
                        let (one, many) = if exact.is_empty() {
                            (
                                tr!(
                                    "ヒント: この入力をちょうど取るセルは書けません（{no_cell}）。`policy first` の表なら、ほかのどの行よりも後ろに次の行を書けば取れます:",
                                    "hint: no cell takes exactly these inputs ({no_cell}). A table under `policy first` takes them with this row, written after every other row:"
                                ),
                                tr!(
                                    "ヒント: この入力をちょうど取るセルは書けません（{no_cell}）。`policy first` の表なら、ほかのどの行よりも後ろに次の行をこの順で書けば取れます:",
                                    "hint: no cell takes exactly these inputs ({no_cell}). A table under `policy first` takes them with these rows, written after every other row in this order:"
                                ),
                            )
                        } else {
                            let which = match unwritten {
                                1 => tr!("行を書いていない 1 か所は、ちょうど取るセルが書けません", "The one without a row is one no cell takes exactly"),
                                n => tr!("行を書いていない {n} か所は、ちょうど取るセルが書けません", "The {n} without a row are ones no cell takes exactly"),
                            };
                            (
                                tr!(
                                    "{which}（{no_cell}）。`policy first` の表なら、ほかのどの行よりも後ろに次の行を書けば取れます:",
                                    "{which} ({no_cell}). A table under `policy first` takes them with this row, written after every other row:"
                                ),
                                tr!(
                                    "{which}（{no_cell}）。`policy first` の表なら、ほかのどの行よりも後ろに次の行をこの順で書けば取れます:",
                                    "{which} ({no_cell}). A table under `policy first` takes them with these rows, written after every other row in this order:"
                                ),
                            )
                        };
                        listed(d, one, many, &rows)
                    };
                    let whole = rest.is_none() && (unwritten == 0 || top_down);
                    // a row that names a value the tables above produce inside a cut (§15.331)
                    let at_values = shown.iter().any(|p| p.held);
                    let d = d.note(match (rows.is_empty(), whole) {
                        (false, true) if at_values => tr!(
                            "行のセルは、表がすでに書いている境と範囲の端、それに上流の表が出す値で切ってあります。その列がそこで取るのは上流の表が出す値だけなので、このまま足せば穴は全部埋まります。行の答えは道具が決めるものではありません。規約か Excel か、いま動いている実装か、どれが出どころかを決めて、そこから書いてください。途中で答えが変わる行は、そこで分けてください。",
                            "The cells are cut at the boundaries the table already writes, at the ends of the ranges, and at the values the tables above produce, the only values those columns take there: the rows close the whole gap as they are. What a row answers is not the tool's to decide. Settle which of the written rule, the spreadsheet and the legacy implementation is the source, and take it from there; split a row where the answer changes inside it."
                        ),
                        (false, false) if at_values => tr!(
                            "行のセルは、表がすでに書いている境と範囲の端、それに上流の表が出す値で切ってあります。その列がそこで取るのは上流の表が出す値だけです。ここに挙げた行で埋まるのは挙げた所だけで、残りは次の実行で出ます。行の答えは道具が決めるものではありません。規約か Excel か、いま動いている実装か、どれが出どころかを決めて、そこから書いてください。途中で答えが変わる行は、そこで分けてください。",
                            "The cells are cut at the boundaries the table already writes, at the ends of the ranges, and at the values the tables above produce, the only values those columns take there. The rows listed close the parts listed, and the next run names what is left. What a row answers is not the tool's to decide. Settle which of the written rule, the spreadsheet and the legacy implementation is the source, and take it from there; split a row where the answer changes inside it."
                        ),
                        (true, _) => tr!(
                            "行の答えは道具が決めるものではありません。規約か Excel か、いま動いている実装か、どれが出どころかを決めて、そこから書いてください。",
                            "What a row answers is not the tool's to decide. Settle which of the written rule, the spreadsheet and the legacy implementation is the source, and take it from there."
                        ),
                        (false, true) => tr!(
                            "行のセルは、表がすでに書いている境と、範囲の端だけで切ってあります。新しい境は決めていないので、このまま足せば穴は全部埋まります。行の答えは道具が決めるものではありません。規約か Excel か、いま動いている実装か、どれが出どころかを決めて、そこから書いてください。途中で答えが変わる行は、そこで分けてください。",
                            "The cells are cut at the boundaries the table already writes and at the ends of the ranges: no new boundary is chosen, and the rows close the whole gap as they are. What a row answers is not the tool's to decide. Settle which of the written rule, the spreadsheet and the legacy implementation is the source, and take it from there; split a row where the answer changes inside it."
                        ),
                        (false, false) => tr!(
                            "行のセルは、表がすでに書いている境と、範囲の端だけで切ってあります。新しい境は決めていません。ここに挙げた行で埋まるのは挙げた所だけで、残りは次の実行で出ます。行の答えは道具が決めるものではありません。規約か Excel か、いま動いている実装か、どれが出どころかを決めて、そこから書いてください。途中で答えが変わる行は、そこで分けてください。",
                            "The cells are cut at the boundaries the table already writes and at the ends of the ranges: no new boundary is chosen. The rows listed close the parts listed, and the next run names what is left. What a row answers is not the tool's to decide. Settle which of the written rule, the spreadsheet and the legacy implementation is the source, and take it from there; split a row where the answer changes inside it."
                        ),
                    });
                    if rows.is_empty() { d } else { d.fix(crate::diag::FixKind::AddRow, rows.join("\n")) }
                }
            },
        );
    } else if left < 0 {
        out.push(
            Diag::error("E109", tr!("検査の予算を超えたので、完全性を証明できませんでした", "The check exceeded its budget, so completeness could not be proven"))
                .at(at(head_span.line, 0))
                .table(anchor.clone())
                .mark(head_span.clone(), "")
                .note(tr!("組み合わせを大きくしているのは {} です。", "The dominant columns are {}.", reg.dominant_axes()))
                .note(tr!("列をグループでまとめるか、表を分けてください。rulec は近似では通しません。", "Combine columns into groups or split the table. No approximation is accepted in its place.")),
        );
    }

    // --- W110: a `policy first` table with no overlaps
    for (mi, pol) in set.policies.iter().enumerate() {
        if *pol != Policy::TopDown {
            continue;
        }
        let rows_of: Vec<usize> = (0..t.rows.len()).filter(|&i| set.member_of[i] == mi).collect();
        // A table E101 has just asked for a last row that takes what the others leave is not
        // told beside it that it has no overlaps and could be `policy unique`: with that row it
        // has them, and under `policy unique` the row could not be written (§15.317).
        if rows_of.len() > 1 && !rows_of.iter().any(|&i| shadowed[i]) && !asked_to_overlap {
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
    TableCheck { diags: out, w114, quiet, shadow, nodes, overlaps, edge_pairs, dead: dead_rows, undecided }
}

/// The third form of E102 (§15.189, §15.195): the derived and `define` columns whose reach left the
/// row out, each with the values it can come to — marked on its line and said in the note, over
/// the ranges of the inputs and the `constraint` lines that narrow it — what a table above writes
/// a value under, where that left the rest out together with a derived or `define` value, and what
/// took the rest of the row, the rows that take precedence first. A row whose cells let in values
/// those columns come to is left out by something else, and is told that instead (§15.307).
#[allow(clippy::too_many_arguments)]
fn third_form(d: Diag, w: &OutOfReach, reg: &TableRegion, row: usize, c: &Checked, f: &RuleFile, by_position: bool, winner_tables: &[String]) -> Diag {
    let ja = crate::i18n::ja();
    // The derived and `define` values that took part, in the order the rule declares them, each
    // with its line, whether it is a derive (a `define` has no `range` to widen), and the
    // `constraint` lines that narrow what it comes to.
    struct Took<'a> {
        name: &'a str,
        span: &'a Span,
        derive: bool,
        iv: Ival,
        /// A `define` with a range the rule accepts (§15.313): what it comes to is read inside that
        /// range, and its line is marked at the range.
        accepted: bool,
    }
    let mut names: Vec<Took> = Vec::new();
    let mut ks: BTreeSet<usize> = BTreeSet::new();
    let by_reach = |n: &str| w.reach.iter().any(|&ai| reg.col_names[ai] == n) || w.derives.contains(n);
    for it in &f.items {
        let (n, span, derive, accepted) = match it {
            Item::Derived(dv) => (&dv.name.text, dv.expr.span(), true, false),
            Item::Define(dv) => match &dv.range {
                Some(rg) if c.accepts.contains_key(&dv.name.text) => (&dv.name.text, &rg.span, false, true),
                _ => (&dv.name.text, dv.expr.span(), false, false),
            },
            _ => continue,
        };
        if !(by_reach(n) || w.above_by.contains(n)) {
            continue;
        }
        let Some((iv, by)) = reg.reach_interval(n, c) else { continue };
        if by_reach(n) {
            ks.extend(by);
        }
        names.push(Took { name: n, span, derive, iv, accepted });
    }
    // The boolean `define`s at the truth value they never take (§15.196): each one's line,
    // marked with the value it always takes, and a note that says so.
    let truth_word = |t: bool| if t { crate::kw::TRUE } else { crate::kw::FALSE };
    let mut d = d;
    let mut truths: Vec<String> = Vec::new();
    for &ai in &w.truth {
        let name = &reg.col_names[ai];
        let Some(t) = reg.define_truth.get(ai).copied().flatten() else { continue };
        if let Some(e) = f.items.iter().find_map(|it| match it {
            Item::Define(dv) if dv.name.text == *name => Some(&dv.expr),
            _ => None,
        }) && !name.contains(':')
        {
            d = d.mark(e.span().clone(), tr!("入力の範囲では、いつも {} です", "always {} over the ranges of the inputs", truth_word(t)));
        }
        d = d.note(tr!(
            "入力の範囲では、`{name}` はいつも {} です。この行は {} を求めています。",
            "Over the ranges of the inputs, `{name}` is always {}, and this row asks for it to be {}.",
            truth_word(t),
            truth_word(!t)
        ));
        truths.push(format!("`{name}`"));
    }
    // Who takes the row's values first, where some do: the earlier rows under `policy first`, or the
    // rows that take precedence.
    let who_ja = if by_position {
        "上にある行".to_string()
    } else if !winner_tables.is_empty() {
        format!("優先する {} の行", winner_tables.join("、"))
    } else {
        "優先する行".to_string()
    };
    let who_en = if by_position {
        "the earlier rows".to_string()
    } else if !winner_tables.is_empty() {
        format!("the rows of {}, which take precedence,", winner_tables.join(", "))
    } else {
        "the rows that take precedence".to_string()
    };
    let lead = if by_position { tr!("`{} {}` なので、", "Because of `{} {}`, ", crate::kw::POLICY, crate::kw::FIRST) } else { String::new() };
    let who_en_cap = if by_position { who_en.clone() } else { capital(&who_en) };
    // The `define`s that, with the values they read held to what the row takes, do not come to what
    // the row asks for (§15.322): each one's line, marked with what it comes to there, and a note. A
    // note that leaves part of the row to the rows that take precedence is followed by one that says
    // they take the rest.
    let mut narrowed_said = false;
    let mut rest_to_say = false;
    for &ai in &w.narrowed {
        let Some((note, mark, whole)) = reg.narrowed_note(ai, row, c, f, w.narrowed_alone()) else { continue };
        if let Some((span, text)) = mark {
            d = d.mark(span, text);
        }
        d = d.note(note);
        narrowed_said = true;
        rest_to_say |= !whole;
    }
    if rest_to_say && w.covered {
        d = d.note(tr!("{lead}この行の残りの値は、{who_ja}が先に取ります。", "{lead}{who_en_cap} take the rest of this row's values first."));
    }
    if names.is_empty() && truths.is_empty() && narrowed_said {
        return d.note(tr!(
            "ヒント: 計算の式とこの行の条件を見比べてください。どの入力もこの行に当たらないので、不要なら削除してください。",
            "hint: compare the expression with this row's conditions. No input reaches this row, so if it is not needed, delete it."
        ));
    }
    // A value the form reads always has an interval; were none to be had, the row is still
    // reached by no input, and is said so plainly.
    if names.is_empty() && !truths.is_empty() {
        let who = truths.join(if ja { "、" } else { ", " });
        return d.note(tr!(
            "ヒント: この行の {who} の条件を、それがいつもとる値に書き直すか、この行を削除してください。",
            "hint: rewrite the condition on {who} to the value it always takes, or delete this row."
        ));
    }
    if names.is_empty() {
        return d
            .note(tr!("この行の条件を同時に満たす入力がありません。", "No input satisfies all of this row's conditions at once."))
            .note(tr!("ヒント: 新しい仕様なら上へ移してください。不要なら削除してください。", "hint: if this is a new specification, move it up; if it is not needed, delete it."));
    }
    // The other `constraint` lines that left a point out.
    let others: BTreeSet<usize> = w.constraints.iter().chain(&w.model_constraints).copied().filter(|k| !ks.contains(k)).collect();
    let ends = |(lo, hi): Ival, ty: &Ty| -> String {
        let mut s: Vec<String> = Vec::new();
        if let Some(l) = lo {
            s.push(format!(">={}", crate::types::fmt_val(l, ty)));
        }
        if let Some(h) = hi {
            s.push(format!("<={}", crate::types::fmt_val(h, ty)));
        }
        s.join(" ")
    };
    let ty_of = |n: &str| c.ty_of(n).unwrap_or(Ty::Unknown);
    // A list of names, values or lines: `a`, `a` と `b`, `a`、`b`、`c` — and in English `a`,
    // `a` and `b`, `a`, `b` and `c`.
    let list = |xs: &[String]| -> String {
        match xs {
            [] => String::new(),
            [a] => a.clone(),
            [a, b] => tr!("{a} と {b}", "{a} and {b}"),
            _ if ja => xs.join("、"),
            _ => format!("{} and {}", xs[..xs.len() - 1].join(", "), xs[xs.len() - 1]),
        }
    };
    // Where the reach comes from: the inputs' ranges, the values a table above writes where an
    // expression reads one, and the `constraint` lines that narrow it.
    let line_of = |k: &usize| -> Option<String> {
        let con = f.constraints.get(*k)?;
        Some(format!("`{} {}`", crate::kw::CONSTRAINT, con.said()))
    };
    let lines: Vec<String> = ks.iter().filter_map(line_of).collect();
    let other_lines: Vec<String> = others.iter().filter_map(line_of).collect();
    let reached: Vec<&Took> = names.iter().filter(|t| by_reach(t.name)).collect();
    let quoted: Vec<String> = reached.iter().map(|t| format!("`{}`", t.name)).collect();
    let one = reached.len() == 1;
    let from_above = reached.iter().any(|t| reads_a_table(t.name, f, c, 0));
    // Whether the reach is what the row asks past (§15.307): one value, where the row's cell lets in
    // none of what it comes to; several, unless each cell lets in all of it (two values may still
    // never come together). A row whose cells let in what the values come to — a `-` among them —
    // is left out by something else: the values past the reach it lets in too are ones no input
    // brings, and the reach left them out only because it is asked first.
    let against: Vec<Option<(bool, bool)>> = reached.iter().map(|t| reg.col_names.iter().position(|n| n == t.name).and_then(|ai| reg.cell_against_reach(row, ai, t.iv))).collect();
    let past_the_reach = reached.is_empty() || if one { !matches!(against.first(), Some(Some((true, _)))) } else { !against.iter().all(|a| matches!(a, Some((true, true)))) };
    // The equations that took part (§15.209, §15.307): those of the `define`s, and those of the
    // derives where nothing else is said of a row that lets in what they come to — no rows take
    // part of it first, no table above. The intervals meet the row's cells one column at a time,
    // and it is the equations that leave no values for all of them.
    let derive_equations_said = !past_the_reach && !w.covered && w.above.is_empty();
    let equations: Vec<String> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Define(dv) if w.define_equations.contains(&dv.name.text) => Some(format!("`{} = {}`", dv.name.text, expr_text(&dv.expr))),
            Item::Derived(dv) if derive_equations_said && w.derive_equations.contains(&dv.name.text) => Some(format!("`{} = {}`", dv.name.text, expr_text(&dv.expr))),
            _ => None,
        })
        .collect();
    // Where nothing else is said of the values either — no rows take part of them first, no table
    // above, no equation — the row is told only what does leave it out.
    let plain = derive_equations_said && equations.is_empty();
    // Each value's line, with what it can come to. One a rule applied is written in the other
    // file, so its line is not marked here.
    for t in &names {
        if t.name.contains(':') || plain {
            continue;
        }
        let iv = ends(t.iv, &ty_of(t.name));
        d = d.mark(
            t.span.clone(),
            if t.accepted {
                tr!("受け付ける範囲の中で取りうる値は {iv} です", "inside the range it accepts, it can come to {iv}")
            } else {
                tr!("実際に取りうる値は {iv} です", "the reachable interval is {iv}")
            },
        );
    }
    // What becomes of the row's values among them: the rows that take precedence take them, or
    // none of them meets the row. Where a table above leaves the rest out, that is said below.
    if plain {
        d = d.note(tr!("この行の条件を同時に満たす入力がありません。", "No input satisfies all of this row's conditions at once."));
    } else if !reached.is_empty() {
        // What the values come to.
        let reach = if ja {
            let ivs: Vec<String> = reached.iter().map(|t| ends(t.iv, &ty_of(t.name))).collect();
            let inputs = if from_above { "入力の範囲と、上流の表が出す値" } else { "入力の範囲" };
            // A `define` with a range the rule accepts is read inside it (§15.313).
            let inputs = if reached.iter().any(|t| t.accepted) { format!("{inputs}と、定義が受け付ける範囲") } else { inputs.to_string() };
            let basis = if lines.is_empty() { format!("{inputs}から") } else { format!("{inputs}と、{} から", list(&lines)) };
            if one {
                format!("{} が取りうる値は、{basis}計算すると {} です。", quoted[0], ivs[0])
            } else {
                format!("{} が取りうる値は、{basis}計算すると、それぞれ {} です。", list(&quoted), list(&ivs))
            }
        } else {
            let mut basis = vec![if from_above { "the ranges of the inputs and the values the tables above produce".to_string() } else { "the ranges of the inputs".to_string() }];
            // A `define` with a range the rule accepts is read inside it (§15.313).
            for t in reached.iter().filter(|t| t.accepted) {
                basis.push(format!("the range `{}` accepts", t.name));
            }
            basis.extend(lines.iter().cloned());
            let basis = list(&basis);
            let mut parts: Vec<String> = Vec::new();
            for (k, t) in reached.iter().enumerate() {
                parts.push(if k == 0 { format!("`{}` can only come to {}", t.name, ends(t.iv, &ty_of(t.name))) } else { format!("`{}` to {}", t.name, ends(t.iv, &ty_of(t.name))) });
            }
            format!("Over {basis}, {}.", if parts.len() > 1 { format!("{}, and {}", parts[..parts.len() - 1].join(", "), parts[parts.len() - 1]) } else { parts.join("") })
        };
        let rest = if !w.above.is_empty() {
            String::new()
        } else if w.covered {
            if one {
                tr!(
                    "{lead}その中でこの行の条件に当てはまる値は、{who_ja}がすべて先に取ります。",
                    "{lead}{who_en_cap} take first every one of those values that meets this row's conditions."
                )
            } else {
                tr!(
                    "{lead}その中でこの行の条件をすべて満たす値の組は、{who_ja}がすべて先に取ります。",
                    "{lead}{who_en_cap} take first every combination of those values that meets all of this row's conditions."
                )
            }
        } else if !equations.is_empty() {
            // The equations are what leave the row out, and the note below says so.
            String::new()
        } else if one {
            tr!("この行の条件に当てはまる値は、その中にありません。", "None of those values meets this row's conditions.")
        } else {
            tr!("この行の条件をすべて満たす値の組は、その中にありません。", "No combination of those values meets all of this row's conditions.")
        };
        d = d.note(if rest.is_empty() { reach } else { format!("{reach}{}{rest}", if ja { "" } else { " " }) });
    }
    // The equations that took part.
    if !equations.is_empty() {
        let many = equations.len() > 1;
        let eqs = list(&equations);
        d = d.note(if many {
            tr!(
                "{eqs} は、どの入力でも成り立つ式です。この式も合わせると、この行の条件をすべて満たす値はありません。",
                "{eqs} hold for every input, and with them no values meet all of this row's conditions."
            )
        } else {
            tr!(
                "{eqs} は、どの入力でも成り立つ式です。この式も合わせると、この行の条件をすべて満たす値はありません。",
                "{eqs} holds for every input, and with it no values meet all of this row's conditions."
            )
        });
    }
    // What a table above writes a value under (§15.195): the rows that write it, and what the
    // derived and `define` values come to where they fire.
    for a in &w.above {
        d = d.note(above_note(a, reg, c, f, &ends, &list));
    }
    if !w.above.is_empty() && w.covered {
        d = d.note(tr!(
            "{lead}この行の残りの値は、{who_ja}が先に取ります。",
            "{lead}{who_en_cap} take the rest of this row's values first."
        ));
    }
    // What else left a point out: a `constraint` that does not bear on the values, the tables
    // above, koyomi's days. A reason after another is said with "either"; the first one said, where
    // the row is told only what leaves it out, without — and every `constraint` line that left a
    // point out is said there, the ones that narrow the values too.
    let mut also = !plain;
    let other_lines: Vec<String> = if plain { w.constraints.iter().chain(&w.model_constraints).copied().collect::<BTreeSet<usize>>().iter().filter_map(line_of).collect() } else { other_lines };
    if !other_lines.is_empty() {
        let ls = list(&other_lines);
        let s = if other_lines.len() == 1 { "s" } else { "" };
        d = d.note(if also {
            tr!("{ls} が許さない入力の組み合わせも来ません。", "The combinations of inputs that {ls} rule{s} out never come either.")
        } else {
            tr!("{ls} が許さない入力の組み合わせは来ません。", "The combinations of inputs that {ls} rule{s} out never come.")
        });
        also = true;
    }
    if !w.nevers.is_empty() {
        let lines: Vec<String> = w.nevers.iter().filter_map(|k| f.nevers.get(*k)).map(|n| format!("`{}`", n.said())).collect();
        let ls = list(&lines);
        d = d.note(if also { tr!("{ls} が外した組み合わせも来ません。", "The combinations {ls} take out never come either.") } else { tr!("{ls} が外した組み合わせは来ません。", "The combinations {ls} take out never come.") });
        also = true;
    }
    if w.upstream {
        d = d.note(if also {
            tr!("上流の表が同時には出さない値の組み合わせも来ません。", "The combinations of values the tables above never produce together never come either.")
        } else {
            tr!("上流の表が同時には出さない値の組み合わせは来ません。", "The combinations of values the tables above never produce together never come.")
        });
    }
    for &ai in &w.days {
        let col = &reg.col_names[ai];
        let from = c.day_sets.get(col).and_then(|x| x.from.as_ref()).map(|fr| format!("koyomi \"{}\" date {}", fr.file, fr.date)).unwrap_or_default();
        d = d.note(tr!("{col} は {from} がとる日だけです。", "{col} takes only the days {from} comes to."));
    }
    let all: Vec<String> = names.iter().map(|t| format!("`{}`", t.name)).collect();
    let who = list(&all);
    if !equations.is_empty() {
        // Every column may come to the values the row asks of it one at a time: it is the equations
        // that leave the row no values, so rewriting a condition inside an interval does not help.
        d.note(tr!(
            "ヒント: この行は削除してください。残すなら、条件を、式どうしの関係が許す値の組に書き直してください。",
            "hint: delete this row. To keep it, rewrite its conditions to values the equations allow together."
        ))
    } else if !past_the_reach && w.covered {
        // The row asks for values the columns come to, and the rows that take precedence take them:
        // rewriting it inside what they come to does not help (§15.307).
        d.note(tr!("ヒント: 新しい仕様なら上へ移してください。不要なら削除してください。", "hint: if this is a new specification, move it up; if it is not needed, delete it."))
    } else if !past_the_reach {
        d.note(tr!(
            "ヒント: 条件を書き直して、すべてを満たす入力があるようにするか、この行を削除してください。",
            "hint: rewrite this row's conditions so that some input meets them all, or delete this row."
        ))
    } else if names.iter().any(|t| t.derive) {
        d.note(tr!(
            "ヒント: 条件を {who} の取りうる値の中に書き直すか、この行を削除してください。導出の `range` を広げても、取りうる値は変わりません。",
            "hint: rewrite the condition within the values {who} can come to, or delete this row. Widening a derive's `range` does not change the values it can come to."
        ))
    } else {
        d.note(tr!(
            "ヒント: 条件を {who} の取りうる値の中に書き直すか、この行を削除してください。",
            "hint: rewrite the condition within the values {who} can come to, or delete this row."
        ))
    }
}

/// An expression as the rule writes it, for a message: names, literals as written, the four
/// operations and the comparisons, and calls.
/// The bounds of a system of `constraint` lines, some of them with a sum on a side, pushed until
/// they stop moving, and then each name fixed at its low end in turn with the bounds pushed again
/// (§15.216). `a + b <= c` lowers the top of `a` to the top of `c` less the bottom of `b`, and
/// raises the bottom of `c` to the two bottoms added up; a strict comparison keeps one step apart.
/// What comes out is a point every constraint is checked against afterwards, so a bound pushed
/// too far costs a witness and never names an input that breaks one. `false` when a name is left
/// with no value.
fn settle_sums(b: &mut BTreeMap<String, Ival>, ks: &[&crate::ast::Constraint], step_of: &dyn Fn(&str) -> Rat) -> bool {
    use std::cmp::Ordering::*;
    let push = |b: &mut BTreeMap<String, Ival>| -> bool {
        // Each round moves a bound by at least one step or stops; ranges are finite, and the
        // number of rounds is held to something a person would wait for all the same.
        for _ in 0..(64 + 8 * b.len()) {
            let mut moved = false;
            for k in ks {
                // `small <= big`, the smaller side first.
                let (small, big, strict) = match k.op {
                    CmpOp::Le => (&k.left, &k.right, false),
                    CmpOp::Lt => (&k.left, &k.right, true),
                    CmpOp::Ge => (&k.right, &k.left, false),
                    CmpOp::Gt => (&k.right, &k.left, true),
                };
                let lows = |side: &[String], b: &BTreeMap<String, Ival>| side.iter().try_fold(Rat::zero(), |acc, n| b.get(n)?.0.map(|v| acc.add(v)));
                let highs = |side: &[String], b: &BTreeMap<String, Ival>| side.iter().try_fold(Rat::zero(), |acc, n| b.get(n)?.1.map(|v| acc.add(v)));
                let gap = |n: &str| if strict { step_of(n) } else { Rat::zero() };
                // Each name on the smaller side is at most the larger side's top less the others'
                // bottoms.
                if let (Some(top), Some(bottoms)) = (highs(big, b), lows(small, b)) {
                    for n in small.iter() {
                        let Some(&(lo, hi)) = b.get(n) else { continue };
                        let Some(own) = lo else { continue };
                        let cap = top.sub(bottoms.sub(own)).sub(gap(n));
                        if hi.is_none_or(|h| cap.cmp_to(h) == Less) {
                            b.insert(n.clone(), (lo, Some(cap)));
                            moved = true;
                        }
                    }
                }
                // Each name on the larger side is at least the smaller side's bottom less the
                // others' tops.
                if let (Some(bottom), Some(tops)) = (lows(small, b), highs(big, b)) {
                    for n in big.iter() {
                        let Some(&(lo, hi)) = b.get(n) else { continue };
                        let Some(own) = hi else { continue };
                        let floor = bottom.sub(tops.sub(own)).add(gap(n));
                        if lo.is_none_or(|l| floor.cmp_to(l) == Greater) {
                            b.insert(n.clone(), (Some(floor), hi));
                            moved = true;
                        }
                    }
                }
            }
            if b.values().any(|(lo, hi)| matches!((lo, hi), (Some(x), Some(y)) if x.cmp_to(*y) == Greater)) {
                return false;
            }
            if !moved {
                return true;
            }
        }
        true
    };
    if !push(b) {
        return false;
    }
    let names: Vec<String> = b.keys().cloned().collect();
    for n in names {
        let Some(&(lo, hi)) = b.get(&n) else { continue };
        let Some(v) = lo.or(hi) else { continue };
        b.insert(n, (Some(v), Some(v)));
        if !push(b) {
            return false;
        }
    }
    true
}

pub(crate) fn expr_text(e: &Expr) -> String {
    match e {
        Expr::Name(n, _) => n.clone(),
        Expr::Lit(Lit::Num(n), _) => n.raw.clone(),
        Expr::Lit(Lit::Word(w), _) => w.clone(),
        Expr::Lit(Lit::Str(x), _) => format!("\"{x}\""),
        Expr::Lit(Lit::Date(y, m, d), _) => format!("{y:04}-{m:02}-{d:02}"),
        Expr::Bin(l, op, r, _) => {
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Le => "<=",
                BinOp::Lt => "<",
                BinOp::Ge => ">=",
                BinOp::Gt => ">",
                BinOp::Eq => "=",
            };
            let side = |x: &Expr| match x {
                Expr::Bin(..) => format!("({})", expr_text(x)),
                _ => expr_text(x),
            };
            format!("{} {o} {}", side(l), side(r))
        }
        Expr::Call(name, args, _) => format!("{name}({})", args.iter().map(expr_text).collect::<Vec<_>>().join(", ")),
    }
}

/// Whether a computed value reads, through the derives and `define`s it is computed from, a value a
/// table writes.
fn reads_a_table(name: &str, f: &RuleFile, c: &Checked, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    let expr = f.items.iter().find_map(|it| match it {
        Item::Derived(d) if d.name.text == name => Some(&d.expr),
        Item::Define(d) if d.name.text == name => Some(&d.expr),
        _ => None,
    });
    let Some(e) = expr else { return false };
    let mut ns = Vec::new();
    names_in(e, &mut ns);
    ns.iter().any(|n| matches!(c.syms.get(n).map(|s| &s.kind), Some(crate::types::SymKind::TableOut)) || reads_a_table(n, f, c, depth + 1))
}

/// The note of E102's third form for a value decided above (§15.195): which rows of the table
/// above write it, and — where one row does — what the derived and `define` values come to while
/// it fires.
fn above_note(a: &AboveRows, reg: &TableRegion, c: &Checked, f: &RuleFile, ends: &dyn Fn(Ival, &Ty) -> String, list: &dyn Fn(&[String]) -> String) -> String {
    let ja = crate::i18n::ja();
    // The table that decides the column, found by its name: a column an `apply` brings in is
    // decided by the table it is brought from (§15.322).
    let Some(set) = c.sets.iter().find(|s| s.table.name.as_ref().is_some_and(|n| n.text == a.table) || s.table.outputs.iter().any(|o| o.name.text == a.column)) else {
        return tr!("上流の表が同時には出さない値の組み合わせも来ません。", "The combinations of values the tables above never produce together never come either.");
    };
    let t = &set.table;
    let table = |ri: usize| t.rows.get(ri).and_then(|r| r.origin.clone()).unwrap_or_else(|| a.table.clone());
    let index = |ri: usize| t.rows.get(ri).map(|r| r.index).unwrap_or(ri + 1);
    let label = |ri: usize| -> String { tr!("表 {t} の 行{n}", "row {n} of table {t}", t = table(ri), n = index(ri)) };
    let conds = |ri: usize| -> String {
        let Some(r) = t.rows.get(ri) else { return String::new() };
        let parts: Vec<String> = t
            .inputs
            .iter()
            .zip(&r.cells)
            .filter(|(_, cell)| !matches!(cell, Cell::DontCare))
            .map(|((col, _), cell)| format!("{col} {}", cell_text(cell)))
            .collect();
        parts.join(if ja { " かつ " } else { " and " })
    };
    let col = &a.column;
    // the value as a sentence says it; the certificate keeps the axis's label (§15.333)
    let val = &reg.axes.get(a.axis).map_or_else(|| a.value.clone(), |ax| ax.witness_said_at(a.coord, None));
    if let [(ri, _, RowOut::Farkas(_))] = a.rows.as_slice() {
        // One row writes the value: what the values come to while it fires.
        let up = TableRegion::build(t, c, f);
        let mut said: Vec<(String, String)> = Vec::new();
        for it in &f.items {
            let n = match it {
                Item::Derived(d) => &d.name.text,
                Item::Define(d) => &d.name.text,
                _ => continue,
            };
            if !reg.col_names.iter().any(|m| m == n) || reg.reach_at(reg.col_names.iter().position(|m| m == n).unwrap_or(0)).is_none() {
                continue;
            }
            let Some(iv) = up.as_ref().and_then(|u| reg.reach_under(n, u, *ri, c)) else { continue };
            said.push((format!("`{n}`"), ends(iv, &c.ty_of(n).unwrap_or(Ty::Unknown))));
        }
        let cs = conds(*ri);
        let at = if cs.is_empty() { label(*ri) } else { tr!("{}（{cs}）", "{} ({cs})", label(*ri)) };
        if said.is_empty() {
            return tr!(
                "`{col}` が {val} になるのは、{at}が当たるときだけです。そのとき、この行の条件をすべて満たす値の組はありません。",
                "`{col}` is {val} only where {at} fires, and there no combination of values meets all of this row's conditions."
            );
        }
        let who: Vec<String> = said.iter().map(|(n, _)| n.clone()).collect();
        let ivs: Vec<String> = said.iter().map(|(_, v)| v.clone()).collect();
        return if said.len() == 1 {
            tr!(
                "`{col}` が {val} になるのは、{at}が当たるときだけです。そのとき {} が取りうる値は {} で、この行の条件に当てはまる値はその中にありません。",
                "`{col}` is {val} only where {at} fires. There {} can only come to {}, and none of those values meets this row's conditions.",
                who[0],
                ivs[0]
            )
        } else {
            tr!(
                "`{col}` が {val} になるのは、{at}が当たるときだけです。そのとき {} が取りうる値は、それぞれ {} で、この行の条件をすべて満たす値の組はその中にありません。",
                "`{col}` is {val} only where {at} fires. There {} can only come to {} respectively, and no combination of those values meets all of this row's conditions.",
                list(&who),
                list(&ivs)
            )
        };
    }
    // Several rows write the value: grouped by the table each is written in, `表 k の 行1 か 行3`.
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    // A row joined with rows of the tables above it in more than one way is named once (§15.322).
    let mut writers: Vec<usize> = Vec::new();
    for (ri, _, _) in &a.rows {
        if !writers.contains(ri) {
            writers.push(*ri);
        }
    }
    for ri in &writers {
        match groups.iter_mut().find(|(tn, _)| *tn == table(*ri)) {
            Some((_, ns)) => ns.push(index(*ri)),
            None => groups.push((table(*ri), vec![index(*ri)])),
        }
    }
    let two = writers.len() == 2;
    let either = |ns: &[usize]| -> String {
        let ws: Vec<String> = ns.iter().map(|n| tr!("行{n}", "{n}")).collect();
        match ws.as_slice() {
            [w] => tr!("{w}", "row {w}"),
            [a, b] => tr!("{a} か {b}", "row {a} or row {b}"),
            _ => tr!("{} のどれか", "one of rows {}", if ja { ws.join("、") } else { format!("{} and {}", ws[..ws.len() - 1].join(", "), ws[ws.len() - 1]) }),
        }
    };
    let rows: Vec<String> = groups.iter().map(|(tn, ns)| tr!("表 {tn} の {}", "{} of table {tn}", either(ns))).collect();
    let rows = if ja { rows.join(" か ") } else { rows.join(" or ") };
    // A row's label is followed by a space, as the messages write `行1 と 行2`; a word is not.
    let rows = if ja && !rows.ends_with("どれか") { format!("{rows} ") } else { rows };
    if two {
        tr!(
            "`{col}` が {val} になるのは、{rows}が当たるときだけです。どちらが当たるときも、この行の条件をすべて満たす値の組はありません。",
            "`{col}` is {val} only where {rows} fires, and where either does, no combination of values meets all of this row's conditions."
        )
    } else {
        tr!(
            "`{col}` が {val} になるのは、{rows}が当たるときだけです。どれが当たるときも、この行の条件をすべて満たす値の組はありません。",
            "`{col}` is {val} only where {rows} fires, and wherever one does, no combination of values meets all of this row's conditions."
        )
    }
}

/// The first letter in upper case, for a phrase that starts a sentence.
fn capital(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}

/// Which rows of a definition set some input may reach (`outputs_over`, ritsu's port), on the
/// merged table: a row is live when some point of its region that no row taking precedence over
/// it covers is left standing by the sieve — koyomi's days, the `constraint`s, a derived value's
/// reach, the tables above (with what a table above writes a value under, together with the
/// linear model, §15.195), and the rule's linear model — and the reach of every numeric column,
/// a `define`'s and a table output's included, with the open ends of a coordinate left out, and
/// the truth value a boolean `define` of one comparison always takes over the held ranges. E102
/// reads the same sieve only where a derived or `define` column takes part (§15.189, §15.195),
/// so a row it lets through can still be dead here: one only a `constraint` rules out, or one
/// that asks a boolean `define` for the truth value it never takes.
///
/// A dead row is never reached. A live row may still be out of reach where the sieve cannot
/// tell (two derived values over the same inputs), so a caller that needs an exact answer finds
/// an input for each live row's value. None when the region cannot be analyzed or the walk runs
/// past `budget`.
pub fn live_rows(set: &crate::defset::DefSet, c: &Checked, f: &RuleFile, budget: i64) -> Option<Vec<bool>> {
    let t = &set.table;
    let reg = TableRegion::build(t, c, f)?;
    if reg.unanalyzable.is_some() {
        return None;
    }
    let ups = upstreams(t, c, f);
    let mut left = budget;
    let mut out = Vec::with_capacity(t.rows.len());
    for i in 0..t.rows.len() {
        if reg.empty(i) || upstream_dead(&t.rows[i], &ups, c, t) || reg.misses_the_days(i).is_some() {
            out.push(false);
            continue;
        }
        let winners: Vec<usize> = set.beats[i].iter().copied().filter(|&e| reg.intersects(e, i)).collect();
        let live = reg.live_rec(&winners, i, &mut Vec::new(), &ups, c, &mut left).is_some();
        if left < 0 {
            return None;
        }
        out.push(live);
    }
    Some(out)
}

/// What `rulec coverage` says of a row that `check` passes and no input reaches (§15.282,
/// §15.309): the walk of E102's third form over the row's points writes down what leaves each
/// one out — the rows that take precedence, the `constraint` and `never` lines, koyomi's days,
/// the tables above — and the message names them, as the third form names them where a derived
/// value takes part. `None` for a row a derived or `define` value takes part in, which is E102's
/// to report; for one some point of which is left standing; and for one the walk does not finish
/// within its budget. Of those the vectors did not find an input for, as far as can be told.
pub fn no_input_reaches(set: &crate::defset::DefSet, c: &Checked, f: &RuleFile, ri: usize) -> Option<String> {
    let t = &set.table;
    let reg = TableRegion::build(t, c, f)?;
    if reg.unanalyzable.is_some() || reg.axes.is_empty() || ri >= t.rows.len() {
        return None;
    }
    let ups = upstreams(t, c, f);
    let winners: Vec<usize> = set.beats[ri].iter().copied().filter(|&e| reg.intersects(e, ri)).collect();
    let mut left = DEFAULT_BUDGET;
    let w = reg.out_of_reach(&winners, ri, &ups, c, &reg.reach_of(), &mut left)?;
    if w.by_a_derive() {
        return None;
    }
    unsaid_text(set, c, f, &reg, ri, &winners, &w)
}

/// What leaves out every point of a row the walk of E102's third form leaves out with no derived or
/// `define` value taking part, said as `rulec coverage` says it of a row `check` passes and as
/// E102 says it of one only the rows that take precedence and the tables above leave out
/// (§15.309, §15.322). `None` where nothing but the rows that take precedence does.
fn unsaid_text(set: &crate::defset::DefSet, c: &Checked, f: &RuleFile, reg: &TableRegion, ri: usize, winners: &[usize], w: &OutOfReach) -> Option<String> {
    let ja = crate::i18n::ja();
    // A list of lines or names: `a`, `a` と `b`, `a`、`b`、`c` — and in English `a`, `a` and `b`,
    // `a`, `b` and `c`.
    let list = |xs: &[String]| -> String {
        match xs {
            [] => String::new(),
            [a] => a.clone(),
            [a, b] => tr!("{a} と {b}", "{a} and {b}"),
            _ if ja => xs.join("、"),
            _ => format!("{} and {}", xs[..xs.len() - 1].join(", "), xs[xs.len() - 1]),
        }
    };
    // What leaves the points out, and what to correct where such an input can come after all.
    let ks: BTreeSet<usize> = w.constraints.iter().chain(&w.model_constraints).copied().collect();
    let mut what: Vec<String> = ks.iter().filter_map(|k| f.constraints.get(*k)).map(|k| format!("`{} {}`", crate::kw::CONSTRAINT, k.said())).collect();
    what.extend(w.nevers.iter().filter_map(|k| f.nevers.get(*k)).map(|n| format!("`{}`", n.said())));
    let mut fix: Vec<String> = Vec::new();
    if !ks.is_empty() {
        fix.push(if ks.len() == 1 { tr!("`constraint` の行", "the `constraint` line") } else { tr!("`constraint` の行", "the `constraint` lines") });
    }
    if !w.nevers.is_empty() {
        fix.push(if w.nevers.len() == 1 { tr!("`never` の行", "the `never` line") } else { tr!("`never` の行", "the `never` lines") });
    }
    // The days and the tables above are said in the plural, a line in the singular.
    let plural = !w.days.is_empty() || w.upstream;
    for &ai in &w.days {
        let col = &reg.col_names[ai];
        let from = c.day_sets.get(col).and_then(|x| x.from.as_ref()).map(|fr| format!("koyomi \"{}\" date {}", fr.file, fr.date)).unwrap_or_default();
        what.push(tr!("`{col}` の日（{from}）", "the days of `{col}` ({from})"));
    }
    if !w.days.is_empty() {
        fix.push(tr!("koyomi のファイル", "the koyomi file"));
    }
    if w.upstream {
        what.push(tr!("上流の表", "the tables above"));
        fix.push(tr!("上流の表", "the tables above"));
    }
    if what.is_empty() {
        return None;
    }
    let s = if what.len() == 1 && !plural { "s" } else { "" };
    let what = list(&what);
    let fix = list(&fix);
    // In Japanese a line in backquotes is set off from the words beside it, a word is not.
    let sp = if what.ends_with('`') { " " } else { "" };
    let fix_sp = if fix.starts_with('`') { " " } else { "" };
    let rest = if w.covered {
        // Some of the row's points the rows that take precedence take first, named as the third
        // form names them.
        let tn = set.row_table(ri).to_string();
        let by_position = set.policy_of(ri) == Policy::TopDown && winners.iter().all(|&e| set.same_member(e, ri));
        let mut tables: Vec<String> = Vec::new();
        for &e in winners {
            let mi = set.member_of[e];
            let n = format!("{} {}", set.kind_word(mi), set.members[mi]);
            if set.members[mi] != tn && !tables.contains(&n) {
                tables.push(n);
            }
        }
        let (who_ja, who_en) = if by_position {
            ("上にある行".to_string(), "the earlier rows".to_string())
        } else if !tables.is_empty() {
            (format!("優先する {} の行", tables.join("、")), format!("the rows of {}, which take precedence,", tables.join(", ")))
        } else {
            ("優先する行".to_string(), "the rows that take precedence".to_string())
        };
        tr!(
            "この行が求める入力のうち、{who_ja}が先に取らないものは、{what}{sp}がすべて外しています。",
            "{what} leave{s} out every input it asks for that {who_en} do not take first."
        )
    } else {
        tr!("この行が求める入力は、{what}{sp}がすべて外しています。", "{what} leave{s} out every input it asks for.")
    };
    Some(tr!(
        "この行に当たる入力はありません。{rest}この行を削除するか、そういう入力が来るなら{fix_sp}{fix}を直してください。",
        "No input reaches this row: {rest} Delete this row, or, if such an input can come, correct {fix}."
    ))
}

impl TableRegion {
    /// A point of row `target`'s region that `rows` do not cover and the sieve leaves standing,
    /// as [`live_rows`] asks for it.
    fn live_rec(&self, rows: &[usize], target: usize, path: &mut Vec<usize>, ups: &[Upstream], chk: &Checked, budget: &mut i64) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if path.len() == self.axes.len() {
            if !rows.is_empty()
                || self.feasible(path) == Feasible::No
                || self.beyond_reach(path, chk)
                || self.upstream_dead_at(path, ups, chk)
                || self.refutes_path(path)
                || self.above_rows_out(path, ups, chk).is_some()
                || self.truth_rules_out(path).is_some()
            {
                return None;
            }
            return Some(path.clone());
        }
        let ai = path.len();
        if rows.iter().any(|&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x))) {
            return None;
        }
        for c in 0..self.axes[ai].len() {
            if !self.masks[target][ai][c] {
                continue;
            }
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            let got = if self.feasible(path) == Feasible::No { None } else { self.live_rec(&sub, target, path, ups, chk, budget) };
            path.pop();
            if got.is_some() {
                return got;
            }
        }
        None
    }

    /// Whether a numeric axis of the path takes a coordinate wholly outside the values its column
    /// can come to (`Checked::ranges`: an input's range, a derive's or a define's reach by the
    /// arithmetic of intervals, the values an upstream table writes), read as the sieve reads a
    /// derive's and a `define`'s reach ([`TableRegion::outside_reach`]): an open coordinate leaves
    /// its ends out, so `> 0` against a reach that ends at 0 is outside it.
    fn beyond_reach(&self, path: &[usize], chk: &Checked) -> bool {
        (0..path.len()).any(|ai| chk.reach_range(&self.col_names[ai]).is_some_and(|reach| self.outside_reach(ai, path, reach)))
    }
}

/// What the third form of E102 may read a row's reach from (§15.189, §15.195).
pub(crate) struct Reach {
    /// Axis → whether it is a derived or `define` column with a reach: its expression over the
    /// intervals of what it reads, which holds every value it takes — a product, a rounding, a
    /// `min` or a value a table above writes as much as a sum.
    axes: Vec<bool>,
    /// Whether the walk asks the derived and `define` values first: the tests that read none of
    /// them — koyomi's days, the `constraint` and `never` lines, the tables above — wait for the
    /// whole point, after the linear model and the rows above (§15.308).
    derives_first: bool,
}

/// Why a row the first two forms of E102 let through is reached by no input all the same
/// (§15.189), as the walk over its points wrote it down: each point it left out, it left out for
/// the first of these that holds.
#[derive(Default)]
pub(crate) struct OutOfReach {
    /// Some points are taken by the rows that take precedence.
    covered: bool,
    /// The axes of derived and `define` columns whose reach leaves a point out.
    reach: BTreeSet<usize>,
    /// The axes of boolean `define`s at the truth value they never take (§15.196).
    truth: BTreeSet<usize>,
    /// The axes of `define`s that, with the axes they read held to a box's coordinates, do not
    /// come to the box's coordinate on theirs (§15.322).
    narrowed: BTreeSet<usize>,
    /// The `constraint` lines (by index) that leave a point out, read one at a time.
    constraints: BTreeSet<usize>,
    /// The `never` lines (by index) that leave a point out (§15.282).
    nevers: BTreeSet<usize>,
    /// The axes of koyomi's days that leave a point out.
    days: BTreeSet<usize>,
    /// The tables above leave a point out, with no derived or `define` value taking part.
    upstream: bool,
    /// The derives and `define`s whose equations or reach a refutation used, and the `constraint`
    /// lines it used.
    derives: BTreeSet<String>,
    model_constraints: BTreeSet<usize>,
    /// The `define`s whose equations a refutation used (§15.209): what the message names, since
    /// the intervals alone do not show why the row is out.
    define_equations: BTreeSet<String>,
    /// The derives whose equations a refutation used: what the message names where the row asks for
    /// values its derives come to, and their reach is not what leaves it out (§15.307).
    derive_equations: BTreeSet<String>,
    /// The values decided above whose rows, together with a derived or `define` value, leave a
    /// point out (§15.195), the first time each is met, and the derived and `define` values
    /// that took part.
    above: Vec<AboveRows>,
    above_by: BTreeSet<String>,
}

impl OutOfReach {
    /// Whether a derived or `define` value took part, which is what the third form is about. A
    /// row left out only by `constraint` lines, the tables above or koyomi's days is not reported
    /// by it.
    fn by_a_derive(&self) -> bool {
        !self.reach.is_empty() || !self.truth.is_empty() || !self.narrowed.is_empty() || !self.derives.is_empty() || !self.above_by.is_empty()
    }

    /// Whether what one `define` comes to with the axes it reads held left out every point, and
    /// nothing else took part (§15.322): its note may then say that the row asks for nothing it comes
    /// to.
    fn narrowed_alone(&self) -> bool {
        self.narrowed.len() == 1
            && !self.covered
            && self.reach.is_empty()
            && self.truth.is_empty()
            && self.derives.is_empty()
            && self.above_by.is_empty()
            && self.above.is_empty()
            && self.constraints.is_empty()
            && self.model_constraints.is_empty()
            && self.nevers.is_empty()
            && self.days.is_empty()
            && !self.upstream
    }
}

impl TableRegion {
    /// What the third form of E102 may read here (§15.189, §15.195).
    fn reach_of(&self) -> Reach {
        Reach { axes: (0..self.axes.len()).map(|ai| self.reach_at(ai).is_some()).collect(), derives_first: false }
    }

    /// Whether the name is computed by a `derive` or by a `define` of a number.
    fn computed(&self, name: &str) -> bool {
        self.exprs.contains_key(name) || (self.defines.contains_key(name) && !self.a_truth_value(name))
    }

    /// Whether the name is a column of truth values here, as a boolean `define` is.
    fn a_truth_value(&self, name: &str) -> bool {
        self.col_names.iter().zip(&self.axes).any(|(n, a)| n == name && matches!(a, Axis::Bool))
    }

    /// The derived and `define` values a refutation leans on — an equation, a reach, the end of a
    /// coordinate on such a column — written into `by`, and the `constraint` lines into `ks`.
    fn leaned_on(&self, r: &crate::fourier::Refutation, by: &mut BTreeSet<String>, ks: &mut BTreeSet<usize>) {
        use crate::fourier::Origin;
        for (q, _) in r.used() {
            match &q.origin {
                Origin::Derive { name, .. } | Origin::Define { name, .. } => {
                    by.insert(name.clone());
                }
                Origin::Range { name, .. } if self.computed(name) => {
                    by.insert(name.clone());
                }
                Origin::Coord { axis, .. } if self.reach_at(*axis).is_some() => {
                    by.insert(self.col_names[*axis].clone());
                }
                Origin::Constraint(k) => {
                    ks.insert(*k);
                }
                _ => {}
            }
        }
    }

    /// Whether row `target` is reached by no input although the first two forms of E102 let it
    /// through (§15.189): every point of its region is taken by `rows` (the rows that take
    /// precedence over it and meet it) or left out by the sieve — koyomi's days, the `constraint`
    /// lines, the reach of the derived and `define` columns, the tables above, the linear model,
    /// and what a table above writes a value under together with it (§15.195). Each test is one
    /// E101 makes as well, so a box this leaves out is a box E101 never asks a row for: deleting
    /// the row leaves no gap behind.
    ///
    /// `None` when some point is left standing, or when the walk runs past `budget`: the row
    /// is then reached, as far as this can tell, and passes as it did before.
    fn out_of_reach(&self, rows: &[usize], target: usize, ups: &[Upstream], chk: &Checked, rs: &Reach, budget: &mut i64) -> Option<OutOfReach> {
        let mut why = OutOfReach::default();
        let mut path = Vec::new();
        let live = self.out_rec(rows, target, &mut path, ups, chk, rs, budget, &mut why);
        (live.is_none() && *budget >= 0).then_some(why)
    }

    #[allow(clippy::too_many_arguments)]
    fn out_rec(
        &self,
        rows: &[usize],
        target: usize,
        path: &mut Vec<usize>,
        ups: &[Upstream],
        chk: &Checked,
        rs: &Reach,
        budget: &mut i64,
        why: &mut OutOfReach,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if path.len() == self.axes.len() {
            if !rows.is_empty() {
                why.covered = true;
                return None;
            }
            if self.left_out(path, ups, chk, rs, true, why) {
                return None;
            }
            return Some(path.clone());
        }
        let ai = path.len();
        if rows.iter().any(|&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x))) {
            why.covered = true;
            return None;
        }
        for c in 0..self.axes[ai].len() {
            if !self.masks[target][ai][c] {
                continue;
            }
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            let got = if self.left_out(path, ups, chk, rs, false, why) { None } else { self.out_rec(&sub, target, path, ups, chk, rs, budget, why) };
            path.pop();
            if got.is_some() {
                return got;
            }
        }
        None
    }

    /// Whether the sieve of the third form leaves out every point below `path`, written into
    /// `why`. On a prefix only the tests that read the axes one at a time are asked; at a whole
    /// point (`leaf`), the tables above, the linear model and the rows above together with it
    /// too. The reach of a derived or `define` column is asked first, since it is what the form
    /// is about, and read as `feasible` reads a derive's. With `rs.derives_first`, the tests that
    /// read no derived or `define` value wait for the whole point, and come after the linear model
    /// and the rows above (§15.308).
    fn left_out(&self, path: &[usize], ups: &[Upstream], chk: &Checked, rs: &Reach, leaf: bool, why: &mut OutOfReach) -> bool {
        // A `define` that does not come to the box's coordinate with the axes it reads held to
        // theirs (§15.322).
        if let Some(k) = self.narrowed_out(path) {
            why.narrowed.insert(self.narrowed[k].axis);
            return true;
        }
        for (ai, &ci) in path.iter().enumerate() {
            if self.truth_never(ai, ci) {
                why.truth.insert(ai);
                return true;
            }
            if !rs.axes[ai] {
                continue;
            }
            if self.define_out_of_reach(ai, path) {
                why.reach.insert(ai);
                return true;
            }
            if self.derived_out_of_reach(ai, path) {
                why.reach.insert(ai);
                return true;
            }
        }
        if !rs.derives_first && self.left_out_by_the_rest(path, ups, chk, leaf, why) {
            return true;
        }
        if !leaf {
            return false;
        }
        if !self.model.is_empty()
            && let Some(r) = self.refute_box(&self.path_box(path))
        {
            let mut by = BTreeSet::new();
            self.leaned_on(&r, &mut by, &mut why.model_constraints);
            // the coordinates of a derived column are no derive's equation or reach
            by.retain(|n| r.used().iter().any(|(q, _)| matches!(&q.origin, crate::fourier::Origin::Derive { name, .. } | crate::fourier::Origin::Define { name, .. } | crate::fourier::Origin::Range { name, .. } if name == n)));
            why.derives.extend(by);
            for (q, _) in r.used() {
                match &q.origin {
                    crate::fourier::Origin::Define { name, .. } => {
                        why.define_equations.insert(name.clone());
                    }
                    crate::fourier::Origin::Derive { name, .. } => {
                        why.derive_equations.insert(name.clone());
                    }
                    _ => {}
                }
            }
            return true;
        }
        if let Some(a) = self.above_rows_out(path, ups, chk) {
            let mut by = BTreeSet::new();
            for (_, _, o) in &a.rows {
                if let RowOut::Farkas(r) = o {
                    self.leaned_on(r, &mut by, &mut why.model_constraints);
                }
            }
            if by.is_empty() {
                why.upstream = true;
            } else {
                why.above_by.extend(by);
                if !why.above.iter().any(|b| b.axis == a.axis && b.coord == a.coord) {
                    why.above.push(a);
                }
            }
            return true;
        }
        rs.derives_first && self.left_out_by_the_rest(path, ups, chk, leaf, why)
    }

    /// The tests of the sieve that read no derived or `define` value, written into `why`: koyomi's
    /// days, the `constraint` and `never` lines, and at a whole point (`leaf`) the tables above.
    fn left_out_by_the_rest(&self, path: &[usize], ups: &[Upstream], chk: &Checked, leaf: bool, why: &mut OutOfReach) -> bool {
        if let Some(ai) = self.days_rule_out(path) {
            why.days.insert(ai);
            return true;
        }
        for (k, con) in self.constraints.iter().enumerate() {
            if self.constraint_impossible(con, path) {
                why.constraints.insert(k);
                return true;
            }
        }
        if let Some(n) = self.never_rules_out(path) {
            why.nevers.insert(self.nevers[n].line);
            return true;
        }
        if leaf && self.upstream_dead_at(path, ups, chk) {
            why.upstream = true;
            return true;
        }
        false
    }

    /// What a derived or `define` column comes to while row `ri` of a table above fires (its region
    /// `up`), for the message of E102's third form (§15.195): the bounds the linear model puts on
    /// it with the ends of the row's own box lent, pulled in to the step its values sit on.
    fn reach_under(&self, name: &str, up: &TableRegion, ri: usize, c: &Checked) -> Option<Ival> {
        use crate::fourier::Lin;
        let ty = c.ty_of(name)?;
        let g = self.model.iter().find(|g| g.want.same(&ty) && g.vars.iter().any(|v| v == name))?;
        let mut sys = g.sys.clone();
        let mut lent = false;
        for (ua, n) in up.col_names.iter().enumerate() {
            if !g.vars.contains(n) || !matches!(up.axes[ua], Axis::Num { .. }) {
                continue;
            }
            let cs: Vec<usize> = (0..up.axes[ua].len()).filter(|&x| up.masks[ri][ua][x]).collect();
            let Some((lo, hi)) = up.ends_of(ua, &cs) else { continue };
            let off = |x: Rat| Lin::var(n).plus(&Lin::con(x.mul(Rat::int(-1))));
            if let Some((x, strict)) = lo {
                sys.push(off(x).ge(strict));
                lent = true;
            }
            if let Some((x, strict)) = hi {
                sys.push(off(x).le(strict));
                lent = true;
            }
        }
        if !lent {
            return None;
        }
        let grid = Rat::new(1, c.scales.get(name).copied().unwrap_or(1).max(1));
        let (lo, hi) = crate::fourier::bounds(sys, name)??;
        let up_to = |x: Rat, strict: bool| {
            let k = x.div(grid);
            let ce = Rat::int(-((-k.num).div_euclid(k.den)));
            let ce = if strict && ce.cmp_to(k) == std::cmp::Ordering::Equal { ce.add(Rat::int(1)) } else { ce };
            ce.mul(grid)
        };
        let down_to = |x: Rat, strict: bool| {
            let k = x.div(grid);
            let fl = Rat::int(k.num.div_euclid(k.den));
            let fl = if strict && fl.cmp_to(k) == std::cmp::Ordering::Equal { fl.sub(Rat::int(1)) } else { fl };
            fl.mul(grid)
        };
        Some((lo.map(|(x, s)| up_to(x, s)), hi.map(|(x, s)| down_to(x, s))))
    }

    /// The interval a derived column comes to (§15.189), for the message: the bounds the linear
    /// model puts on it once every other name is eliminated, pulled in to the grid
    /// its values sit on, or the interval its expression reaches when the model says nothing.
    /// With the `constraint` lines (by index) that narrow it: those of its system, when the
    /// bounds without them are wider.
    #[allow(clippy::type_complexity)]
    fn reach_interval(&self, name: &str, c: &Checked) -> Option<(Ival, Vec<usize>)> {
        use crate::fourier::Origin;
        let plain = c.reach_range(name)?;
        let Some(g) = self.model.iter().find(|g| g.vars.iter().any(|v| v == name)) else { return Some((plain, Vec::new())) };
        let grid = Rat::new(1, c.scales.get(name).copied().unwrap_or(1).max(1));
        let inward = |b: Option<(Option<(Rat, bool)>, Option<(Rat, bool)>)>| -> Option<Ival> {
            let (lo, hi) = b?;
            let up = |x: Rat, strict: bool| {
                let k = x.div(grid);
                let ce = Rat::int(-((-k.num).div_euclid(k.den)));
                let ce = if strict && ce.cmp_to(k) == std::cmp::Ordering::Equal { ce.add(Rat::int(1)) } else { ce };
                ce.mul(grid)
            };
            let down = |x: Rat, strict: bool| {
                let k = x.div(grid);
                let fl = Rat::int(k.num.div_euclid(k.den));
                let fl = if strict && fl.cmp_to(k) == std::cmp::Ordering::Equal { fl.sub(Rat::int(1)) } else { fl };
                fl.mul(grid)
            };
            Some((lo.map(|(x, s)| up(x, s)), hi.map(|(x, s)| down(x, s))))
        };
        let with = crate::fourier::bounds(g.sys.clone(), name).and_then(inward);
        let without = crate::fourier::bounds(g.sys.iter().filter(|q| !matches!(q.origin, Origin::Constraint(_))).cloned().collect(), name).and_then(inward);
        let Some(iv) = with else { return Some((plain, Vec::new())) };
        let moved = |a: Option<Rat>, b: Option<Rat>| match (a, b) {
            (Some(a), Some(b)) => a.cmp_to(b) != std::cmp::Ordering::Equal,
            (a, b) => a.is_some() != b.is_some(),
        };
        let by: Vec<usize> = match without {
            Some(w) if !moved(w.0, iv.0) && !moved(w.1, iv.1) => Vec::new(),
            _ => g.sys.iter().filter_map(|q| if let Origin::Constraint(k) = q.origin { Some(k) } else { None }).collect::<BTreeSet<usize>>().into_iter().collect(),
        };
        Some((iv, by))
    }
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
                Coord::Absent => None,
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
            Coord::Absent => None,
            Coord::Point(v) => Some((Some(*v), Some(*v))),
            Coord::Open(a, b) => Some((a.map(|v| v.add(*step)), b.map(|v| v.sub(*step)))),
        }
    }

    /// The first and the last value each coordinate of an axis stands for, written as the
    /// coordinates are (`0.1%` and `0.4%` for the rates above 0% and below 0.5%, on a step of
    /// 0.1%), on the grid the column's values take: what ritsu's port of rules gives a file that
    /// holds an input to one coordinate with a value of its own (dandori's `held`). None for a
    /// coordinate that is no interval of numbers or holds no value of the column, and None inside
    /// for an end the declared range leaves open.
    pub fn coord_ends(&self, ai: usize) -> Vec<Option<(Option<String>, Option<String>)>> {
        use std::cmp::Ordering::Less;
        let Axis::Num { unit, date, shown, step, grid, coords, .. } = &self.axes[ai] else {
            return vec![None; self.axes[ai].len()];
        };
        let write = |v: Rat| {
            if *date {
                let (y, m, d) = crate::types::ord_to_date(v);
                format!("{y:04}-{m:02}-{d:02}")
            } else {
                format!("{}{unit}", v.mul(Rat::int(*shown)))
            }
        };
        // a column whose values are coarser than the axis's step takes only the grid's values
        let coarser = grid.num > 0 && grid.cmp_to(*step) != std::cmp::Ordering::Equal;
        (0..coords.len())
            .map(|ci| {
                let (lo, hi) = self.coord_closed(ai, ci)?;
                let (lo, hi) = if coarser { (lo.map(|v| crate::grid::ceil_to(v, *grid)), hi.map(|v| crate::grid::floor_to(v, *grid))) } else { (lo, hi) };
                if let (Some(l), Some(h)) = (lo, hi)
                    && h.cmp_to(l) == Less
                {
                    return None;
                }
                Some((lo.map(write), hi.map(write)))
            })
            .collect()
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
            (Axis::Num { coords: own, .. }, Axis::Num { coords, step, .. }) => {
                // The absent value stands for the absent value there (§15.201).
                if matches!(own.get(ci), Some(Coord::Absent)) {
                    return coords.iter().map(|cd| matches!(cd, Coord::Absent)).collect();
                }
                let Some((lo, hi)) = self.coord_closed(ai, ci) else { return all };
                coords
                    .iter()
                    .map(|cd| {
                        let (a, b) = match cd {
                            Coord::Absent => return false,
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

    /// Whether the tables above rule **this point** out, the question `upstream_dead` asks of a
    /// row, read as a certificate can state it ([`upstream_blocked`]): the columns of words the
    /// point fixes, its other coordinates carried onto the axes of the tables above with
    /// [`TableRegion::project`], which keeps a coordinate there whenever it meets the point's. A gap
    /// the tables above cannot produce is not a gap.
    ///
    /// Under `policy first` a row of the table above fires only where no earlier row does, and
    /// `upstream_blocked` takes an earlier row away only where it takes the whole of what is left.
    /// So a table above under `first` is read again on its own axes, coordinate by coordinate
    /// ([`span_of_value`]): where the value it holds here is written, against where this point
    /// holds the column (§15.322). On a table above of one axis that reading is exact, and a
    /// certificate states it as it is (`apart_at`), each re-checker working the span out again.
    fn upstream_dead_at(&self, path: &[usize], ups: &[Upstream], c: &Checked) -> bool {
        if self.upstream_blocked_at(path, ups, c) {
            return true;
        }
        ups.iter().any(|u| u.table.policy == Policy::TopDown) && self.spans_apart(path, ups, c).is_some()
    }

    /// [`upstream_blocked`] at a point, the reading `upstream_dead_at` starts from.
    fn upstream_blocked_at(&self, path: &[usize], ups: &[Upstream], c: &Checked) -> bool {
        upstream_blocked(
            ups,
            c,
            &|up: &Upstream| {
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
            },
        )
    }

    /// The evidence behind an upstream leaf, where it can be written small (§15.115).
    ///
    /// Two spans of one input column that do not meet. One always comes from a column above;
    /// the other comes from a second column above, or from what this box already fixes that
    /// input to. `None` when no such pair settles it — the leaf then goes out bare and is
    /// counted among the things the certificate states rather than proves.
    fn apart_at(&self, path: &[usize], ups: &[Upstream], c: &Checked) -> Option<Apart> {
        // The smallest reason first: the table that decides the column never writes the
        // value, whatever the rest of the box says.
        if let Some(never) = self.never_at(path, ups, c) {
            return Some(never);
        }
        self.spans_apart(path, ups, c)
    }

    /// Two spans of one input column that do not meet, as [`TableRegion::apart_at`] writes them:
    /// where two values held above put it, or where one does and this point holds it.
    fn spans_apart(&self, path: &[usize], ups: &[Upstream], c: &Checked) -> Option<Apart> {
        let held = self.held_above(path, ups);
        for (i, (up, v)) in held.iter().enumerate() {
            for col in &up.reg.col_names {
                let Some(mine) = up.span_of(c, v, col) else { continue };
                for (up2, v2) in held.iter().skip(i + 1) {
                    let Some(theirs) = up2.span_of(c, v2, col) else { continue };
                    if !mine.meets(&theirs) {
                        return Some(Apart::Spans {
                            input: col.clone(),
                            spans: vec![
                                (Some(up.name.clone()), Some(v.label.clone()), mine),
                                (Some(up2.name.clone()), Some(v2.label.clone()), theirs),
                            ],
                        });
                    }
                }
                if let Some(here) = self.span_here(col, path) {
                    if !mine.meets(&here) {
                        return Some(Apart::Spans {
                            input: col.clone(),
                            spans: vec![(Some(up.name.clone()), Some(v.label.clone()), mine), (None, None, here)],
                        });
                    }
                }
            }
        }
        None
    }

    /// The value each column decided above holds where this path fixes its axis, in the order
    /// of `ups`. A column the path has not fixed yet is not among them. A coordinate of numbers is
    /// named by its label, as the certificate's axis names it, and carries the axis it is on
    /// (§15.322); the absent value of an optional number is not among them.
    fn held_above<'u, 'a>(&self, path: &[usize], ups: &'u [Upstream<'a>]) -> Vec<(&'u Upstream<'a>, Held)> {
        ups.iter()
            .filter_map(|up| {
                let ai = self.col_names.iter().position(|x| *x == up.name)?;
                let &ci = path.get(ai)?;
                if let Axis::Num { coords, date: false, .. } = &self.axes[ai] {
                    if matches!(coords.get(ci), None | Some(Coord::Absent)) {
                        return None;
                    }
                    return Some((up, Held { label: self.axes[ai].witness(ci), num: Some((self.axes[ai].clone(), ci)) }));
                }
                Some((up, Held { label: self.value_at(ai, ci)?, num: None }))
            })
            .collect()
    }

    /// The value a coordinate of an enum's or a truth value's axis stands for.
    fn value_at(&self, ai: usize, ci: usize) -> Option<String> {
        match &self.axes[ai] {
            Axis::Enum { values } => values.get(ci).cloned(),
            Axis::Bool => Some(if ci == 0 { crate::kw::TRUE } else { crate::kw::FALSE }.to_string()),
            _ => None,
        }
    }

    /// Whether no row of the table that decides the column of `up` puts anything on coordinate
    /// `ci` of its axis `ai` here: a value word none writes (§15.253), or a number none writes,
    /// where every row writes one these readings follow — a number literal, put through the
    /// roundings on the way, or `none` (§15.322).
    fn never_holds(&self, up: &Upstream, ai: usize, ci: usize, c: &Checked) -> bool {
        match &self.axes[ai] {
            Axis::Enum { .. } | Axis::Bool => self.value_at(ai, ci).is_some_and(|v| never_written(up, &v, c)),
            Axis::Num { .. } => (0..up.table.rows.len()).all(|ri| {
                let put = up.put(ri, c);
                put != Put::Any && !coord_takes(&self.axes[ai], ci, &put)
            }),
            Axis::Prefix { .. } => false,
        }
    }

    /// The first column decided above that this path fixes at a value no row of the deciding
    /// table writes: one fact that rules out every point under the path, however many axes it
    /// has left open (§15.253).
    fn never_at(&self, path: &[usize], ups: &[Upstream], c: &Checked) -> Option<Apart> {
        ups.iter().find_map(|up| {
            let ai = self.col_names.iter().position(|x| *x == up.name)?;
            let &ci = path.get(ai)?;
            self.never_holds(up, ai, ci, c).then(|| Apart::Never { column: up.name.clone(), value: self.axes[ai].witness(ci) })
        })
    }

    /// Every coordinate of a column decided above whose value no row of the deciding table
    /// writes, as the certificate states it (§15.253, §15.322). The boxes leave each of them out
    /// (`reachable`), and a re-checker takes a box back from its cells only with the coordinates
    /// it is told of left out, so each one is stated whether or not a leaf of the cover rests on
    /// it.
    fn never_facts(&self, ups: &[Upstream], c: &Checked) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for up in ups {
            let Some(ai) = self.col_names.iter().position(|x| *x == up.name) else { continue };
            for ci in 0..self.axes[ai].len() {
                if self.never_holds(up, ai, ci, c) {
                    out.push((ai, ci));
                }
            }
        }
        out
    }

    /// What a box of the cover fixes, as a leaf names it: the columns of the axes it has fixed,
    /// each at the value it holds there, in the table's own column order. At a whole point it is
    /// the point's text.
    fn box_text(&self, path: &[usize]) -> String {
        if path.len() >= self.axes.len() {
            return self.witness_text(path);
        }
        let mut items: Vec<(usize, String)> = path
            .iter()
            .enumerate()
            .map(|(ai, &ci)| (self.display_of[ai], format!("{} = {}", self.col_names[ai], self.axes[ai].witness_at(ci, None))))
            .collect();
        items.sort_by_key(|(d, _)| *d);
        items.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(", ")
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
                // A coordinate of numbers is named by its label, as the certificate's axis names it
                // (§15.322).
                Axis::Num { .. } => (0..self.axes[ai].len()).find(|&c| self.axes[ai].witness(c) == val)?,
                _ => return None,
            };
            Some((ai, ci))
        };
        match cv {
            Cover::Split(kids) => kids.iter().for_each(|k| self.gather_above(k, out)),
            // The facts the points of a box asked one at a time rest on, each read as a leaf's.
            Cover::ByPoints(facts) => {
                for (what, a) in facts {
                    self.gather_above(&Cover::ByUpstream(what.clone(), Some(a.clone())), out);
                }
            }
            Cover::ByUpstream(_, Some(Apart::Never { column, value })) => {
                if let Some((axis, coord)) = idx(column, value) {
                    if !out.iter().any(|f| matches!(f, AboveFact::Never { axis: x, coord: y } if *x == axis && *y == coord)) {
                        out.push(AboveFact::Never { axis, coord });
                    }
                }
            }
            Cover::ByUpstream(what, Some(Apart::Spans { input, spans })) => {
                // A span with no column of its own is the box's own coordinate on the
                // shared input. On an enum the leaf's own text names which one it is; on a
                // number it is the coordinate whose span it is, which both re-checkers work out
                // again from the axis (§15.293).
                let pin = |col: &Option<String>, val: &Option<String>, span: &CertSpan| -> Option<(usize, usize)> {
                    match (col, val) {
                        (Some(c), Some(v)) => idx(c, v),
                        _ => {
                            let ai = self.col_names.iter().position(|n| n == input)?;
                            let said = || what.split(", ").find_map(|part| part.strip_prefix(&format!("{input} = ")));
                            let ci = match &self.axes[ai] {
                                Axis::Enum { values } => said().and_then(|v| values.iter().position(|x| x == v))?,
                                // `true` is a truth value's first coordinate (§15.322).
                                Axis::Bool => said().and_then(|v| [crate::kw::TRUE, crate::kw::FALSE].iter().position(|x| *x == v))?,
                                Axis::Num { .. } => (0..self.axes[ai].len()).find(|&c| self.coord_closed(ai, c).map(|(a, b)| CertSpan::Num(a, b)).as_ref() == Some(span))?,
                                _ => return None,
                            };
                            Some((ai, ci))
                        }
                    }
                };
                if let (Some(a), Some(b)) = (pin(&spans[0].0, &spans[0].1, &spans[0].2), pin(&spans[1].0, &spans[1].1, &spans[1].2)) {
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
            let Some(ivs) = k.names().map(|n| self.closed_of_name(n, path).map(|iv| (n.clone(), iv))).collect::<Option<Vec<_>>>() else {
                continue;
            };
            for (n, iv) in ivs {
                b.entry(n).or_insert(iv);
            }
            ks.push(k);
        }
        // A side that adds names up is not a difference constraint, and Bellman-Ford does not
        // settle it: the bounds of a sum are pushed onto each name it adds, and the names are then
        // fixed one at a time, each at its low end, with the bounds pushed again after each
        // (§15.216). Without a sum the system is the difference constraints it always was, and
        // the chain below is left as it is.
        if ks.iter().any(|k| k.is_sum()) {
            if !settle_sums(&mut b, &ks, &step_of) {
                return None;
            }
        }
        // Bellman-Ford over the difference constraints: one pass per name is enough for
        // the bounds to travel the longest chain, and a cycle settles on equality.
        for _ in 0..=b.len() {
            let mut moved = false;
            for k in &ks {
                let Some((kl, kr)) = k.pair() else { continue };
                let (Some(&(ll, lh)), Some(&(rl, rh))) = (b.get(kl), b.get(kr)) else { continue };
                let gap = |n: &str, strict: bool| if strict { step_of(n) } else { Rat::zero() };
                let (nlh, nrl, nll, nrh) = match k.op {
                    // left <= right: the left cannot pass the right's top, and the right
                    // cannot sit below the left's bottom.
                    CmpOp::Le | CmpOp::Lt => {
                        let d = gap(kl, k.op == CmpOp::Lt);
                        (drop_to(lh, rh.map(|v| v.sub(d))), raise(rl, ll.map(|v| v.add(gap(kr, k.op == CmpOp::Lt)))), ll, rh)
                    }
                    CmpOp::Ge | CmpOp::Gt => {
                        let d = gap(kr, k.op == CmpOp::Gt);
                        (lh, rl, raise(ll, rl.map(|v| v.add(gap(kl, k.op == CmpOp::Gt)))), drop_to(rh, lh.map(|v| v.sub(d))))
                    }
                };
                let changed = |a: Option<Rat>, c: Option<Rat>| !matches!((a, c), (None, None)) && a.map(|x| x.num) != c.map(|x| x.num);
                if changed(lh, nlh) || changed(rl, nrl) || changed(ll, nll) || changed(rh, nrh) {
                    moved = true;
                }
                b.insert(kl.clone(), (nll, nlh));
                b.insert(kr.clone(), (nrl, nrh));
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
            let total = |side: &[String]| side.iter().try_fold(Rat::zero(), |acc, n| b.get(n).and_then(pick).map(|v| acc.add(v)));
            let (Some(x), Some(y)) = (total(&k.left), total(&k.right)) else {
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
        let vals: Vec<Option<Rat>> = (0..self.axes.len())
            .map(|ai| {
                let by_constraint = b.get(&self.col_names[ai]).and_then(pick);
                self.axes[ai].witness_num(path.get(ai).copied().unwrap_or(0), by_constraint)
            })
            .collect();
        // On an axis of koyomi's days a value that is not one of them names an input that never
        // arrives (§15.174); no point is named rather than that one.
        for (ai, v) in vals.iter().enumerate() {
            if let (Some(ds), Some(v)) = (self.days_of_axis(ai), v)
                && !(v.den == 1 && ds.binary_search(&(v.num as i64)).is_ok())
            {
                return None;
            }
        }
        Some(vals)
    }

    /// The same values, with the value of each derived or `define` column moved inside what the
    /// column can come to where its coordinate reaches past it (§15.195): a gap of E101 and a
    /// point that reaches a row are examples of values that occur, and the first value of an
    /// interval coordinate is not one where the reach starts later. A `constraint` names inputs
    /// only, so moving these breaks none.
    fn reach_values(&self, path: &[usize]) -> Option<Vec<Option<Rat>>> {
        let mut vals = self.witness_values(path)?;
        for (ai, v) in vals.iter_mut().enumerate() {
            if let (Some(x), Some(&ci)) = (*v, path.get(ai)) {
                *v = Some(self.inside_reach(ai, ci, x));
            }
        }
        Some(vals)
    }

    /// The reach of a derived or `define` column, if it has one.
    fn reach_at(&self, ai: usize) -> Option<Ival> {
        self.define_reach.get(ai).copied().flatten().or_else(|| self.derived.get(ai).and_then(|d| d.as_ref().map(|(r, _)| *r)))
    }

    /// `v`, or where it lies outside the reach of its column, the value nearest the reach that
    /// lies inside both the reach and the interval coordinate `ci`, on the step the column's
    /// values take. `v` again where there is none, which the sieve does not let happen.
    fn inside_reach(&self, ai: usize, ci: usize, v: Rat) -> Rat {
        use std::cmp::Ordering::*;
        let Some((rl, rh)) = self.reach_at(ai) else { return v };
        let Axis::Num { coords, grid, step, date: false, .. } = &self.axes[ai] else { return v };
        let Some(Coord::Open(a, b)) = coords.get(ci) else { return v };
        let g = if grid.num > 0 { *grid } else { *step };
        let inside = |x: Rat| {
            a.is_none_or(|a| a.cmp_to(x) == Less)
                && b.is_none_or(|b| x.cmp_to(b) == Less)
                && rl.is_none_or(|l| l.cmp_to(x) != Greater)
                && rh.is_none_or(|h| x.cmp_to(h) != Greater)
        };
        let k = |x: Rat| x.checked_div(g);
        if let Some(l) = rl
            && v.cmp_to(l) == Less
        {
            // the first value on the step at or above the reach's low end
            let up = k(l).map(|q| Rat::int(-((-q.num).div_euclid(q.den))));
            if let Some(x) = up.and_then(|n| n.checked_mul(g))
                && inside(x)
            {
                return x;
            }
        }
        if let Some(h) = rh
            && v.cmp_to(h) == Greater
        {
            let down = k(h).map(|q| Rat::int(q.num.div_euclid(q.den)));
            if let Some(x) = down.and_then(|n| n.checked_mul(g))
                && inside(x)
            {
                return x;
            }
        }
        v
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
        // The ends a side can reach: a name's own, or for a sum the ends of the names it adds,
        // added up — an end left out when any of the ends it adds is (§15.216).
        type Ends = ((Option<Rat>, bool), (Option<Rat>, bool));
        let side = |names: &[String]| -> Option<Ends> {
            let (mut lo, mut hi): ((Option<Rat>, bool), (Option<Rat>, bool)) = ((Some(Rat::zero()), false), (Some(Rat::zero()), false));
            for n in names {
                let ((a, ao), (b, bo)) = self.ends_of_name(n, path)?;
                lo = (lo.0.zip(a).map(|(x, y)| x.add(y)), lo.1 || ao);
                hi = (hi.0.zip(b).map(|(x, y)| x.add(y)), hi.1 || bo);
            }
            Some((lo, hi))
        };
        let (Some(((la, lo_l), (lb, hi_l))), Some(((ra, lo_r), (rb, hi_r)))) = (side(&k.left), side(&k.right)) else {
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
                    Some(Coord::Absent) | None => {}
                }
            }
        }
        self.spans.get(name).map(|(a, b)| ((*a, false), (*b, false)))
    }

    /// The sieve of §6.2. For each derived axis, check whether the reachable interval and the
    /// coordinate's interval intersect, an interval coordinate's ends left out (§15.304). If they
    /// do not, the point is infeasible. When two or more constrained derived values share an
    /// input, their dependency cannot be examined.
    pub fn feasible(&self, path: &[usize]) -> Feasible {
        if self.days_rule_out(path).is_some() || self.unwritten_at(path).is_some() || self.narrowed_out(path).is_some() {
            return Feasible::No;
        }
        // A `define` whose coordinate lies outside what it can come to, and a boolean one at the
        // truth value it never takes, are points no input reaches: the overlap checks read them
        // as the completeness check does (§15.196).
        if self.defines_rule_out(path).is_some() || self.truth_rules_out(path).is_some() {
            return Feasible::No;
        }
        for k in &self.constraints {
            if self.constraint_impossible(k, path) {
                return Feasible::No;
            }
        }
        if self.never_rules_out(path).is_some() {
            return Feasible::No;
        }
        let mut constrained: Vec<(usize, Vec<String>)> = Vec::new();
        for (ai, d) in self.derived.iter().enumerate() {
            let Some((_, deps)) = d else { continue };
            let Some(&ci) = path.get(ai) else { continue };
            let Some((cl, ch)) = self.coord_span(ai, ci) else { continue };
            if self.derived_out_of_reach(ai, path) {
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

/// Why a row of a table above, one that writes the value a column of this table holds, fires
/// nowhere in a box (§15.195).
#[derive(Debug, Clone)]
pub enum RowOut {
    /// On this axis of the table below, the box takes no coordinate the row lets in: the two
    /// tables cut a column of words or truth values they share, and they do not agree.
    Clash(usize),
    /// The linear model of the table below, the ends of the box, and the ends of what the row
    /// takes on the numeric columns it cuts leave no values: the multipliers say so.
    Farkas(crate::fourier::Refutation),
    /// A row of the way is in a `policy first` table, and the rows above it there take all of what
    /// its box leaves in the box (§15.325): `step` is the row's place in the way (0 the row that
    /// writes the value), `rows` the rows above it whose boxes cover that together, in the table's
    /// order (0-based).
    Earlier { step: usize, rows: Vec<usize> },
    /// A row of the way takes nothing of the box: on a column both tables cut, read on each axis's
    /// grid, no coordinate it takes meets one the box takes (§15.325). `step` is the row's place in
    /// the way. Two rows cut at different boundaries can leave room between them only off the grid,
    /// where no value is: `< 31` and `> 30 < 32` of whole yen.
    Misses { step: usize },
    /// A row of the way cuts a value the rule works out — a `define` or a derive, not an open one —
    /// on its axis `axis`, and none of the coordinates it takes there is one the value comes to in
    /// the box (§15.325): worked out with this table's axes `by` held to the span of what the box
    /// takes on each — met with what the row takes of the same column, where its table cuts it — and
    /// the axes `row_by` of the row's own table, of columns this table does not cut, held to the span
    /// of what the row takes on each, as a fact of `narrowed` is; met with what the box takes of the
    /// value itself, where this table cuts it, and with the range the rule accepts for it (§15.313).
    /// A truth value of one comparison is read the same way, and the row takes only the other one.
    /// Where that leaves a coordinate in, `cuts` names one axis of `by` and where each piece of its
    /// span after the first starts, and every piece leaves every coordinate out. `step` is the row's
    /// place in the way. The table above decides a tier from `fee`, `fee` is worked out from
    /// `amount`, and the box asks for an amount whose fee no row writing that tier takes.
    Reach { step: usize, axis: usize, by: Vec<usize>, row_by: Vec<usize>, cuts: Option<(usize, Vec<Rat>)> },
    /// Two rows of the way take no value of a column together (§15.325): the tables of the steps
    /// `steps` both cut the column, on the axes `axes`, and no coordinate the later row takes there —
    /// of those the box lets a value stand on — meets one the earlier row takes, read as [`carry`]
    /// reads it. The table above decides `fee` from `amount` and `kind`, the table that decides the
    /// tier from `fee` and `kind` writes `t1` for `kind` r, and in the box only `kind` q writes that
    /// fee.
    Disjoint { steps: (usize, usize), axes: (usize, usize) },
    /// A row of the way, in a table that decides a column this table cuts too (one the way is joined
    /// through), writes a value none of the coordinates the box takes on that column holds
    /// (§15.325): `axis` is this table's axis of the column. The box asks for a fee of 100 yen or
    /// less, and the row that decides the fee writes 260 yen.
    Writes { step: usize, axis: usize },
}

/// A box no input reaches because the value a column decided above holds there is written only by
/// rows of that table, and none of them can fire in the box (§15.195): the table above writes
/// `low` only for an amount up to 50 pounds, and the box asks for an excess over the limit past
/// 50 pounds, which no amount up to 50 pounds makes.
#[derive(Debug, Clone)]
pub struct AboveRows {
    /// The axis of the column a table above decides, and the coordinate (the value) the box takes.
    pub axis: usize,
    pub coord: usize,
    /// That column, the value, and the table that decides it.
    pub column: String,
    pub value: String,
    pub table: String,
    /// Each way the value arrives — a row of that table that writes it (0-based, in that table's
    /// order), with the rows of the tables above it that write what it reads on a column this
    /// table does not (§15.322), each named by its table and its number there — and why the way
    /// fires nowhere here.
    pub rows: Vec<(usize, Vec<(String, usize)>, RowOut)>,
    /// Whether the ways are joined besides through the columns the deciding table reads that this
    /// table reads too ([`Upstream::paths_through`], §15.325).
    pub through: bool,
}

impl TableRegion {
    /// Whether the coordinate this path takes on the axis of a `define` lies outside the values the
    /// define can come to (§15.195), read as [`TableRegion::outside_reach`] reads it.
    fn define_out_of_reach(&self, ai: usize, path: &[usize]) -> bool {
        let Some(reach) = self.define_reach.get(ai).copied().flatten() else { return false };
        self.outside_reach(ai, path, reach)
    }

    /// Whether the coordinate this path takes on axis `ai` lies outside `reach`, the values a
    /// derived or `define` column can come to. An interval coordinate leaves its ends out, so
    /// `>100%` lies outside a reach that ends at 100% (§15.195), and `>100` lies outside what
    /// `min(a, 100)` comes to (§15.304).
    fn outside_reach(&self, ai: usize, path: &[usize], reach: Ival) -> bool {
        let Some(&ci) = path.get(ai) else { return false };
        self.coord_outside(ai, ci, reach)
    }

    /// Whether coordinate `ci` of axis `ai` lies outside `reach`, as [`TableRegion::outside_reach`]
    /// reads the coordinate a path takes.
    fn coord_outside(&self, ai: usize, ci: usize, (rl, rh): Ival) -> bool {
        use std::cmp::Ordering::*;
        let Axis::Num { coords, .. } = &self.axes[ai] else { return false };
        match coords.get(ci) {
            Some(Coord::Point(v)) => rl.is_some_and(|l| v.cmp_to(l) == Less) || rh.is_some_and(|h| v.cmp_to(h) == Greater),
            Some(Coord::Open(a, b)) => {
                matches!((b, rl), (Some(b), Some(l)) if b.cmp_to(l) != Greater) || matches!((a, rh), (Some(a), Some(h)) if a.cmp_to(h) != Less)
            }
            // The absent value is one the input takes, outside any range (§15.201).
            Some(Coord::Absent) | None => false,
        }
    }

    /// What E102's third form says of a `define` whose [`NarrowCut`]s left out the row's points
    /// (§15.322): the note, a mark for the define's line where it is written in this file, and
    /// whether the note says the define leaves out every value the row asks for. What the define
    /// comes to is worked out over the hull of what the row takes on each axis it reads; where that
    /// still meets what the row asks for, the note names the grid its values sit on if no value of it
    /// lies in what the row asks for there, and otherwise says that no value the row takes brings
    /// it there — where the define left out every point alone (`alone`) — or that some of them
    /// leave out some of what the row asks for. Of a define the table reads none of the values of,
    /// the note names the grid over the whole of what it comes to.
    fn narrowed_note(&self, ai: usize, row: usize, c: &Checked, f: &RuleFile, alone: bool) -> Option<(String, Option<(Span, String)>, bool)> {
        let name = &self.col_names[ai];
        let mut by: Vec<usize> = Vec::new();
        for n in self.narrowed.iter().filter(|n| n.axis == ai) {
            for &(j, _) in &n.by {
                if !by.contains(&j) {
                    by.push(j);
                }
            }
        }
        by.sort_by_key(|&j| self.display_of[j]);
        let ja = crate::i18n::ja();
        let names: Vec<String> = by.iter().map(|&j| format!("`{}`", self.col_names[j])).collect();
        let who = match names.as_slice() {
            [] => String::new(),
            [a] => a.clone(),
            [a, b] => tr!("{a} と {b}", "{a} and {b}"),
            _ if ja => names.join("、"),
            _ => format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1]),
        };
        // The hull of what the row takes on each axis the define reads.
        let over: Option<Vec<(String, (Rat, Rat))>> = by
            .iter()
            .map(|&j| {
                let spans: Vec<(Rat, Rat)> = (0..self.axes[j].len()).filter(|&x| self.masks[row][j][x]).filter_map(|x| closed_coord(&self.axes[j], x, &self.col_names[j], c)).collect();
                let lo = spans.iter().map(|s| s.0).min_by(|a, b| a.cmp_to(*b))?;
                let hi = spans.iter().map(|s| s.1).max_by(|a, b| a.cmp_to(*b))?;
                Some((self.col_names[j].clone(), (lo, hi)))
            })
            .collect();
        let held: BTreeSet<&str> = by.iter().map(|&j| self.col_names[j].as_str()).collect();
        let inline: BTreeMap<String, Expr> = self
            .defines
            .iter()
            .chain(self.exprs.iter())
            .filter(|(n, _)| *n != name && !held.contains(n.as_str()))
            .filter(|(n, _)| {
                let mut r = BTreeSet::new();
                reads_through(n, &self.exprs, &self.defines, &mut r);
                r.iter().any(|m| held.contains(m.as_str()))
            })
            .map(|(n, x)| (n.clone(), x.clone()))
            .collect();
        let e = self.defines.get(name)?;
        let line = f.items.iter().find_map(|it| match it {
            Item::Define(dv) if dv.name.text == *name && !name.contains(':') => Some(dv.expr.span().clone()),
            _ => None,
        });
        let generic = if alone {
            tr!(
                "`{name}` は {who} から計算する値です。この行がとる {who} のどの値でも、`{name}` はこの行が求める値になりません。",
                "`{name}` is computed from {who}. For no value of {who} this row takes does `{name}` come to what this row asks for."
            )
        } else {
            // What is left out is held coordinate by coordinate, so the note says no more than that
            // some of the values the row takes leave out some of what it asks for (§15.325): a value
            // the row asks for may still come at the others.
            tr!(
                "`{name}` は {who} から計算する値です。この行がとる {who} の値の一部では、この行が求める値の一部に `{name}` がなりません。",
                "`{name}` is computed from {who}. At some of the values of {who} this row takes, `{name}` does not come to some of what this row asks for."
            )
        };
        if matches!(self.axes[ai], Axis::Bool) {
            let truth_word = |t: bool| if t { crate::kw::TRUE } else { crate::kw::FALSE };
            return match over.as_ref().and_then(|o| narrow_truth(c, e, o, &inline)) {
                Some(t) => {
                    let mark = line.map(|sp| (sp, tr!("この行の {who} の値では、いつも {} です", "always {} over the values of {who} in this row", truth_word(t))));
                    // `true` is the first coordinate of a boolean axis. A row that lets in the value
                    // the define takes there is left that much to the rows that take precedence.
                    if self.masks[row][ai].get(if t { 0 } else { 1 }).copied().unwrap_or(false) {
                        Some((
                            tr!(
                                "`{name}` は {who} から決まる真偽値です。この行がとる {who} の値では、`{name}` はいつも {} です。",
                                "`{name}` follows from {who}. Over the values of {who} this row takes, `{name}` is always {}.",
                                truth_word(t)
                            ),
                            mark,
                            false,
                        ))
                    } else {
                        Some((
                            tr!(
                                "`{name}` は {who} から決まる真偽値です。この行がとる {who} の値では、`{name}` はいつも {} です。この行は {} を求めています。",
                                "`{name}` follows from {who}. Over the values of {who} this row takes, `{name}` is always {}, and this row asks for it to be {}.",
                                truth_word(t),
                                truth_word(!t)
                            ),
                            mark,
                            true,
                        ))
                    }
                }
                None => Some((generic, None, alone)),
            };
        }
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        // The grid the define's values sit on, said where no value of it lies in what the row asks
        // for inside `reach`: the multiples of the step, or a value and the multiples of the step
        // added to it.
        let computed: BTreeMap<String, Expr> =
            self.defines.iter().chain(self.exprs.iter()).filter(|(n, _)| f.open_value(n).is_none()).map(|(n, x)| (n.clone(), x.clone())).collect();
        let grid_words = |reach: (Rat, Rat), over: &[(String, (Rat, Rat))], inline: &BTreeMap<String, Expr>| -> Option<String> {
            let Axis::Num { coords, .. } = &self.axes[ai] else { return None };
            let (o, st) = held_grid(c, e, ty.present(), over, inline, &computed, &self.steps, 0)?;
            if st.num <= 0 {
                return None;
            }
            let asked: Vec<usize> = (0..coords.len()).filter(|&x| self.masks[row][ai][x] && !matches!(coords[x], Coord::Absent)).collect();
            if asked.is_empty() || !asked.iter().all(|&x| grid_misses(&coords[x], (o, st), reach)) {
                return None;
            }
            let q = o.checked_div(st)?;
            let first = o.checked_sub(Rat::int(q.num.div_euclid(q.den)).checked_mul(st)?)?;
            let step = crate::types::fmt_val(st, ty.present());
            Some(if first.num == 0 {
                tr!("{step} の倍数", "multiples of {step}")
            } else {
                let first = crate::types::fmt_val(first, ty.present());
                tr!("{first} に {step} の倍数を足した値", "{first} plus multiples of {step}")
            })
        };
        if by.is_empty() {
            // The table reads nothing the define reads: what leaves the row out is the grid over the
            // whole of what the define comes to.
            let whole = narrow_iv(c, e, ty.present(), &[], &BTreeMap::new(), 0).map(|r| within_accepts(r, c.accepts.get(name)));
            return Some(match whole.and_then(|r| grid_words(r, &[], &BTreeMap::new())) {
                Some(g) => (
                    tr!("`{name}` は {g}しかとらず、この行が求める値になりません。", "`{name}` only comes to {g}, and this row asks for none of them."),
                    line.map(|sp| (sp, tr!("{g}しかとりません", "only {g}"))),
                    true,
                ),
                None if alone => (tr!("`{name}` は、この行が求める値になりません。", "`{name}` never comes to what this row asks for."), None, true),
                None => (tr!("この行が求める値のうち、`{name}` がとらないものがあります。", "`{name}` does not come to some of what this row asks for."), None, false),
            });
        }
        let iv = over.as_ref().and_then(|o| narrow_iv(c, e, ty.present(), o, &inline, 0));
        // A call whose value falls outside the range the rule accepts for it is turned away
        // (§15.313): where all of what the row takes is, the row has no call to answer.
        if let (Some(raw), Some(acc)) = (iv, c.accepts.get(name)) {
            if within_accepts(raw, Some(acc)).0.cmp_to(within_accepts(raw, Some(acc)).1) == std::cmp::Ordering::Greater {
                let fmt = |x: Rat| crate::types::fmt_val(x, ty.present());
                let ends = format!(">={} <={}", fmt(raw.0), fmt(raw.1));
                let range = [acc.0.map(|l| format!(">={}", fmt(l))), acc.1.map(|h| format!("<={}", fmt(h)))]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ");
                return Some((
                    tr!(
                        "`{name}` は {who} から計算する値です。この行がとる {who} の値では、`{name}` は {ends} の値しかとらず、受け付ける範囲（{range}）に入りません。規則はそうした呼び出しをどれも受け付けません。",
                        "`{name}` is computed from {who}. Over the values of {who} this row takes, `{name}` only comes to {ends}, outside the range the rule accepts for it ({range}), so every such call is turned away."
                    ),
                    line.map(|sp| (sp, tr!("この行の {who} の値では {ends} で、受け付ける範囲の外です", "{ends} over the values of {who} in this row, outside the range it accepts"))),
                    true,
                ));
            }
        }
        let iv = iv.map(|r| within_accepts(r, c.accepts.get(name)));
        let on_grid = iv.and_then(|r| grid_words(r, over.as_deref().unwrap_or(&[]), &inline));
        match iv {
            Some((lo, hi)) if on_grid.is_some() && !matches!(self.cell_against_reach(row, ai, (Some(lo), Some(hi))), Some((false, _))) => {
                let g = on_grid.unwrap_or_default();
                let ends = format!(">={} <={}", crate::types::fmt_val(lo, ty.present()), crate::types::fmt_val(hi, ty.present()));
                Some((
                    tr!(
                        "`{name}` は {who} から計算する値です。この行がとる {who} の値では、`{name}` は {ends} の {g}しかとらず、この行が求める値になりません。",
                        "`{name}` is computed from {who}. Over the values of {who} this row takes, `{name}` only comes to {g} within {ends}, none of which this row asks for."
                    ),
                    line.map(|sp| (sp, tr!("この行の {who} の値では {ends} の {g}です", "{g} within {ends} over the values of {who} in this row"))),
                    true,
                ))
            }
            Some((lo, hi)) if matches!(self.cell_against_reach(row, ai, (Some(lo), Some(hi))), Some((false, _))) => {
                let ends = format!(">={} <={}", crate::types::fmt_val(lo, ty.present()), crate::types::fmt_val(hi, ty.present()));
                Some((
                    tr!(
                        "`{name}` は {who} から計算する値です。この行がとる {who} の値では、`{name}` は {ends} の値しかとらず、この行が求める値になりません。",
                        "`{name}` is computed from {who}. Over the values of {who} this row takes, `{name}` only comes to {ends}, which is not what this row asks for."
                    ),
                    line.map(|sp| (sp, tr!("この行の {who} の値では {ends} です", "{ends} over the values of {who} in this row"))),
                    true,
                ))
            }
            _ => Some((generic, None, alone)),
        }
    }

    /// How row `row`'s cell on the axis of a derived or `define` column stands to `reach`, the values
    /// the column comes to (§15.307): whether it lets in some coordinate inside the reach, and
    /// whether it lets in every one. `None` when the axis is no axis of numbers.
    fn cell_against_reach(&self, row: usize, ai: usize, reach: Ival) -> Option<(bool, bool)> {
        let Axis::Num { coords, .. } = &self.axes[ai] else { return None };
        let (mut meets, mut covers) = (false, true);
        for (ci, co) in coords.iter().enumerate() {
            if matches!(co, Coord::Absent) || self.coord_outside(ai, ci, reach) {
                continue;
            }
            if self.masks[row][ai][ci] {
                meets = true;
            } else {
                covers = false;
            }
        }
        Some((meets, covers))
    }

    /// The first axis of a numeric column decided above on which this path takes a coordinate no
    /// row of the deciding table writes into (§15.322).
    pub(crate) fn unwritten_at(&self, path: &[usize]) -> Option<usize> {
        path.iter().enumerate().find(|(ai, ci)| self.written_above.get(*ai).and_then(|w| w.as_ref()).is_some_and(|w| !w.get(**ci).copied().unwrap_or(true))).map(|(ai, _)| ai)
    }

    /// The first [`NarrowCut`] that takes this path: it fixes every axis the cut holds at the cut's
    /// coordinate, and the define's axis at one the cut leaves out (§15.322).
    pub(crate) fn narrowed_out(&self, path: &[usize]) -> Option<usize> {
        self.narrowed.iter().position(|n| {
            path.get(n.axis).is_some_and(|&ci| n.out.get(ci).copied().unwrap_or(false)) && n.by.iter().all(|&(j, x)| path.get(j) == Some(&x))
        })
    }

    /// The first axis of a `define` on which this path takes a coordinate the define never comes to.
    pub(crate) fn defines_rule_out(&self, path: &[usize]) -> Option<usize> {
        (0..path.len()).find(|&ai| self.define_out_of_reach(ai, path))
    }

    /// Whether a column a table above decides holds, in this box, a value whose every row in that
    /// table fails to fire here (§15.195). A row fails where the box takes none of what the row
    /// lets in on a column of words both tables cut, or where the table's linear model, the ends
    /// of the box and the ends of the row's own box on the numeric columns it cuts have no
    /// solution — the multipliers of the refutation are kept, so a certificate can hand them on.
    ///
    /// A row's box holds every input on which it fires, also under `policy first`, where an
    /// earlier row takes part of it: what is read is wider than the truth, which is the safe side.
    /// A table above that writes a name rather than a value may write the value anywhere, and is
    /// not read. `None` when nothing is ruled out this way.
    fn above_rows_out(&self, path: &[usize], ups: &[Upstream], chk: &Checked) -> Option<AboveRows> {
        let bx = self.path_box(path);
        for up in ups {
            let Some(ai) = self.col_names.iter().position(|n| *n == up.name) else { continue };
            let Some(&ci) = path.get(ai) else { continue };
            if let Some(a) = self.above_rows_in(&bx, up, ai, ci, chk) {
                return Some(a);
            }
        }
        None
    }

    /// The same, for any box that takes coordinate `ci` alone on the axis `ai` of the column `up`
    /// decides: every way the value arrives, with why it fires nowhere in the box. A row of the
    /// deciding table writes the value when the word it writes is the coordinate's, or the number
    /// it writes, put through the roundings on the way, lies in the coordinate (§15.322); a way
    /// joins it with the rows above that write what it reads on a column this table does not.
    fn above_rows_in(&self, bx: &[Vec<usize>], up: &Upstream, ai: usize, ci: usize, chk: &Checked) -> Option<AboveRows> {
        self.above_rows_where(bx, up, ai, ci, &|put| coord_takes(&self.axes[ai], ci, put), chk)
    }

    /// [`Self::above_rows_in`] for the rows `writes` picks of those that write a value of the
    /// coordinate: a number among the numbers of a cut is one of them (§15.331).
    fn above_rows_where(&self, bx: &[Vec<usize>], up: &Upstream, ai: usize, ci: usize, writes: &dyn Fn(&Put) -> bool, chk: &Checked) -> Option<AboveRows> {
        if !matches!(self.axes[ai], Axis::Enum { .. } | Axis::Bool | Axis::Num { .. }) {
            return None;
        }
        if !self.above_can(up) {
            return None;
        }
        let mut writers: Vec<usize> = Vec::new();
        for ri in 0..up.table.rows.len() {
            let put = up.put(ri, chk);
            if put == Put::Any {
                return None;
            }
            if writes(&put) {
                writers.push(ri);
            }
        }
        // A value no row writes is the second form's business, and the masks have it already.
        if writers.is_empty() {
            return None;
        }
        // Each way with its reason; where one has none, every way joined besides through the
        // columns this table reads too, which carry what their own rows take (§15.325).
        let reasons = |through: bool| -> Option<Vec<(usize, Vec<(String, usize)>, RowOut)>> {
            let mut rows = Vec::new();
            for &ri in &writers {
                let ways = if through { up.paths_through(ri, self, chk)? } else { up.paths(ri, self, chk)? };
                for way in ways {
                    let why = self.way_out(bx, up, &way, chk)?;
                    let via = way[1..].iter().map(|&(k, r)| (up.table_name(k), r + 1)).collect();
                    rows.push((ri, via, why));
                }
            }
            Some(rows)
        };
        let (rows, through) = match reasons(false) {
            Some(rows) => (rows, false),
            None if !up.through.is_empty() => (reasons(true)?, true),
            None => return None,
        };
        Some(AboveRows {
            axis: ai,
            coord: ci,
            column: up.name.clone(),
            value: self.axes[ai].witness(ci),
            table: up.table_name(0),
            rows,
            through,
        })
    }

    /// Two rows that meet only where the rows of a table above leave no values (§15.196): on the
    /// axis of a column that table decides, for every value both rows take there, every row that
    /// writes it fires nowhere in what both rows take. The pair is apart on the points the rule is
    /// asked about, and each value comes with its reasons, as a cover's leaf does.
    fn above_apart(&self, i: usize, j: usize, ups: &[Upstream], chk: &Checked) -> Option<(usize, Vec<AboveRows>)> {
        let pb = self.pair_box(i, j);
        for up in ups {
            let Some(ai) = self.col_names.iter().position(|n| *n == up.name) else { continue };
            if pb[ai].is_empty() {
                continue;
            }
            let mut all = Vec::new();
            for &c in &pb[ai] {
                let mut sub = pb.clone();
                sub[ai] = vec![c];
                match self.above_rows_in(&sub, up, ai, c, chk) {
                    Some(a) => all.push(a),
                    None => break,
                }
            }
            if all.len() == pb[ai].len() {
                return Some((ai, all));
            }
        }
        None
    }

    /// Whether a table above can rule anything out here this way: it — or a table above it whose
    /// rows a way joins (§15.322) — cuts a numeric column this table numbers a value for (an axis
    /// of its, or a name of its linear model), a column of words both cut, or a value of numbers or
    /// a truth value the rule works out from a column of numbers this table cuts (§15.325). Asked
    /// first, since the rest is a system to eliminate per row.
    fn above_can(&self, up: &Upstream) -> bool {
        (0..up.steps()).any(|k| {
            let reg = up.reg_of(k);
            reg.col_names.iter().enumerate().any(|(ua, n)| match &reg.axes[ua] {
                Axis::Num { date, .. } => self.numbered(n) || (!date && self.reads_an_axis(n)),
                Axis::Enum { .. } => self.col_names.iter().zip(&self.axes).any(|(m, a)| m == n && matches!(a, Axis::Enum { .. } | Axis::Bool)),
                // A truth value the rule works out from a column this table cuts is read over the
                // box as a value of numbers is (§15.325).
                Axis::Bool => self.col_names.iter().zip(&self.axes).any(|(m, a)| m == n && matches!(a, Axis::Enum { .. } | Axis::Bool)) || self.reads_an_axis(n),
                Axis::Prefix { .. } => false,
            })
        })
    }

    /// Whether `n` is a value the rule works out that reads, through what it reads, a column this
    /// table has an axis of numbers for (§15.325): a box of this table then holds what it comes to.
    fn reads_an_axis(&self, n: &str) -> bool {
        if !self.computed.contains_key(n) {
            return false;
        }
        let mut reads = BTreeSet::new();
        reads_to_columns(n, &self.exprs, &self.defines, &self.col_names, &mut reads, &mut BTreeSet::new());
        self.col_names.iter().zip(&self.axes).any(|(m, a)| m != n && reads.contains(m) && matches!(a, Axis::Num { date: false, .. }))
    }

    /// Whether this table numbers a value for the name: a numeric axis of its, or a name of its
    /// linear model. A certificate numbers them the same way (the axes, then `linear.extra`).
    fn numbered(&self, n: &str) -> bool {
        self.col_names.iter().zip(&self.axes).any(|(m, a)| m == n && matches!(a, Axis::Num { .. })) || self.model.iter().any(|g| g.vars.iter().any(|v| v == n))
    }

    /// Why a way a value arrives (§15.322) — a row of the table above, joined with the rows of the
    /// tables above it — fires nowhere in the box `bx`, or nothing when it may. Every row of the way
    /// fires where the value arrives, so one that cannot is enough, and the ends of all of them
    /// hold together.
    fn way_out(&self, bx: &[Vec<usize>], up: &Upstream, way: &[(usize, usize)], chk: &Checked) -> Option<RowOut> {
        use crate::fourier::{Lin, Origin, Refutation};
        // A column of words or truth values a table of the way and this one both cut, on which the
        // box takes nothing the row lets in.
        for &(k, ri) in way {
            let reg = up.reg_of(k);
            for (ua, n) in reg.col_names.iter().enumerate() {
                let Some(aj) = self.col_names.iter().position(|m| m == n) else { continue };
                let same = matches!((&self.axes[aj], &reg.axes[ua]), (Axis::Enum { .. }, Axis::Enum { .. }) | (Axis::Bool, Axis::Bool));
                if !same {
                    continue;
                }
                let mut mine = vec![false; self.axes[aj].len()];
                for &c in &bx[aj] {
                    mine[c] = true;
                }
                let lets = carry(&self.axes[aj], &mine, &reg.axes[ua]);
                if !(0..reg.axes[ua].len()).any(|x| reg.masks[ri][ua][x] && lets[x]) {
                    return Some(RowOut::Clash(aj));
                }
            }
        }
        // The systems, one per type of number: this table's linear model where it has one for
        // the type, and otherwise its own numeric axes of that type with nothing else.
        let mut systems: Vec<(Ty, Vec<String>, Vec<crate::fourier::Ineq>)> =
            self.model.iter().map(|g| (g.want.clone(), g.vars.clone(), g.sys.clone())).collect();
        for (aj, n) in self.col_names.iter().enumerate() {
            if !matches!(self.axes[aj], Axis::Num { .. }) {
                continue;
            }
            let Some(ty) = chk.ty_of(n) else { continue };
            // one system for each unit, however it is spelled (§15.332)
            match systems.iter_mut().find(|s| s.0.same(&ty)) {
                Some(s) => {
                    if !s.1.contains(n) {
                        s.1.push(n.clone());
                    }
                }
                None => systems.push((ty, vec![n.clone()], Vec::new())),
            }
        }
        for (_, vars, facts) in systems {
            let mut sys = facts;
            let mut lent = false;
            for &(k, ri) in way {
                let reg = up.reg_of(k);
                for (ua, n) in reg.col_names.iter().enumerate() {
                    if !vars.contains(n) || !matches!(reg.axes[ua], Axis::Num { .. }) {
                        continue;
                    }
                    let cs: Vec<usize> = (0..reg.axes[ua].len()).filter(|&x| reg.masks[ri][ua][x]).collect();
                    let Some((lo, hi)) = reg.ends_of(ua, &cs) else { continue };
                    let off = |x: Rat| Lin::var(n).plus(&Lin::con(x.mul(Rat::int(-1))));
                    if let Some((x, strict)) = lo {
                        sys.push(off(x).ge(strict).tag(Origin::Above { name: n.clone(), hi: false }));
                        lent = true;
                    }
                    if let Some((x, strict)) = hi {
                        sys.push(off(x).le(strict).tag(Origin::Above { name: n.clone(), hi: true }));
                        lent = true;
                    }
                }
            }
            // A way that bounds none of these names adds nothing the model does not say alone.
            if !lent {
                continue;
            }
            for (aj, n) in self.col_names.iter().enumerate() {
                if !vars.contains(n) {
                    continue;
                }
                let Some((lo, hi)) = bx.get(aj).and_then(|cs| self.ends_of(aj, cs)) else { continue };
                let off = |x: Rat| Lin::var(n).plus(&Lin::con(x.mul(Rat::int(-1))));
                if let Some((x, strict)) = lo {
                    sys.push(off(x).ge(strict).tag(Origin::Coord { axis: aj, hi: false }));
                }
                if let Some((x, strict)) = hi {
                    sys.push(off(x).le(strict).tag(Origin::Coord { axis: aj, hi: true }));
                }
            }
            if let Some(r) = Refutation::of(sys) {
                return Some(RowOut::Farkas(r));
            }
        }
        // A row of the way in a `policy first` table fires only where no row above it there does:
        // the rows above it may take all of what its box leaves in the box (§15.325).
        way.iter().enumerate().find_map(|(s, &(k, ri))| self.step_out(bx, up, s, k, ri))
            // Two rows of the way that take no value of a column together, and a row of the way that
            // cuts a value the rule works out where the value does not come in the box (§15.325).
            .or_else(|| self.disjoint_out(bx, up, way))
            .or_else(|| way.iter().enumerate().find_map(|(s, &(k, ri))| self.writes_out(bx, up, s, k, ri, chk)))
            .or_else(|| way.iter().enumerate().find_map(|(s, &(k, ri))| self.reach_out(bx, up, s, k, ri, chk)))
    }

    /// Why row `ri` of the table of step `k` fires nowhere in the box `bx` by what it takes of a value
    /// the rule works out (§15.325): on an axis of that table whose column is a `define` or a derive
    /// the rule works out ([`TableRegion::computed`]), no coordinate the row takes there — of those
    /// the box lets a value stand on ([`TableRegion::part_in`]) — is one the value comes to with this
    /// table's axes held ([`TableRegion::held_spans`]), each span met with what the row takes of the
    /// same column where its table cuts it, and the value met with what the box takes of it where this
    /// table cuts it: each lies outside the interval, or holds no value of the grid the value sits on
    /// inside it ([`held_grid`]), as for a fact of `narrowed`. Where the whole spans leave one in, the
    /// span of one held axis is cut into pieces on its step ([`split_out`], within
    /// [`REACH_SPLITS`] tries), and every piece has to leave every coordinate out. `step` is the
    /// row's place in the way.
    fn reach_out(&self, bx: &[Vec<usize>], up: &Upstream, s: usize, k: usize, ri: usize, chk: &Checked) -> Option<RowOut> {
        use std::cmp::Ordering::*;
        let reg = up.reg_of(k);
        if reg.unanalyzable.is_some() || ri >= reg.masks.len() {
            return None;
        }
        let part = self.part_in(bx, reg, ri);
        // The hull of the closed spans of some coordinates of an axis; `None` where one is the absent
        // value or has an end nothing reads.
        let hull = |axis: &Axis, xs: &[usize], name: &str| -> Option<(Rat, Rat)> {
            let spans = xs.iter().map(|&x| closed_coord(axis, x, name, chk)).collect::<Option<Vec<_>>>()?;
            let lo = spans.iter().map(|s| s.0).min_by(|a, b| a.cmp_to(*b))?;
            let hi = spans.iter().map(|s| s.1).max_by(|a, b| a.cmp_to(*b))?;
            Some((lo, hi))
        };
        for (ua, n) in reg.col_names.iter().enumerate() {
            // A value of numbers, or a truth value of one comparison (§15.322's `narrowed`).
            let num = match &reg.axes[ua] {
                Axis::Num { coords, date: false, .. } => Some(coords),
                Axis::Bool => None,
                _ => continue,
            };
            let Some(e) = self.computed.get(n) else { continue };
            if num.is_none() && !matches!(e, Expr::Bin(_, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt, _, _)) {
                continue;
            }
            let Some(ty) = chk.ty_of(n) else { continue };
            let xs: Vec<usize> = (0..reg.axes[ua].len()).filter(|&x| part[ua].get(x) == Some(&true)).collect();
            if xs.is_empty() {
                continue;
            }
            let (by, mut over) = self.held_spans(n, bx, chk);
            // What the row itself takes of a held column, where its table cuts that column too.
            let mut empty = false;
            for (m, span) in over.iter_mut() {
                let Some(uj) = reg.col_names.iter().position(|c| c == m) else { continue };
                if !matches!(reg.axes[uj], Axis::Num { date: false, .. }) {
                    continue;
                }
                let ys: Vec<usize> = (0..reg.axes[uj].len()).filter(|&y| part[uj].get(y) == Some(&true)).collect();
                let Some((l, h)) = hull(&reg.axes[uj], &ys, m) else { continue };
                *span = (crate::grid::max_rat(span.0, l), crate::grid::min_rat(span.1, h));
                empty |= span.0.cmp_to(span.1) == Greater;
            }
            if empty {
                continue;
            }
            // The columns of the row's own table the value reads that this table does not cut, each
            // held to the span of what the row takes there: where the way fires, the row does.
            let mut row_by = Vec::new();
            {
                let mut reads = BTreeSet::new();
                reads_through(n, &self.exprs, &self.defines, &mut reads);
                for (uj, m) in reg.col_names.iter().enumerate() {
                    if m == n || !reads.contains(m) || self.col_names.contains(m) || over.iter().any(|(o, _)| o == m) {
                        continue;
                    }
                    if !matches!(reg.axes[uj], Axis::Num { date: false, .. }) {
                        continue;
                    }
                    let ys: Vec<usize> = (0..reg.axes[uj].len()).filter(|&y| part[uj].get(y) == Some(&true)).collect();
                    let Some(span) = hull(&reg.axes[uj], &ys, m) else { continue };
                    row_by.push(uj);
                    over.push((m.clone(), span));
                }
            }
            // What the box takes of the value itself, where this table cuts it.
            let mine = self.col_names.iter().position(|c| c == n).and_then(|av| {
                let cs = bx.get(av).filter(|cs| !cs.is_empty())?;
                hull(&self.axes[av], cs, n)
            });
            let inline = self.inline_for(n, &over);
            let outs = |over: &[(String, (Rat, Rat))]| -> bool {
                let Some(coords) = num else {
                    // The truth value the comparison takes there, and the row takes only the other one;
                    // `true` is the first coordinate of a boolean axis.
                    return narrow_truth(chk, e, over, &inline).is_some_and(|t| !xs.contains(&(if t { 0 } else { 1 })));
                };
                let Some(iv) = narrow_iv(chk, e, ty.present(), over, &inline, 0) else { return false };
                let iv = within_accepts(iv, chk.accepts.get(n));
                let iv = match mine {
                    Some((l, h)) => (crate::grid::max_rat(iv.0, l), crate::grid::min_rat(iv.1, h)),
                    None => iv,
                };
                // Nothing the value comes to in the box is left.
                if iv.0.cmp_to(iv.1) == Greater {
                    return true;
                }
                let grid = held_grid(chk, e, ty.present(), over, &inline, &self.computed, &self.steps, 0);
                xs.iter().all(|&x| coord_out(&coords[x], iv, grid))
            };
            if outs(&over) {
                return Some(RowOut::Reach { step: s, axis: ua, by, row_by, cuts: None });
            }
            let mut left = REACH_SPLITS;
            for (kk, &aj) in by.iter().enumerate() {
                let Axis::Num { step, .. } = &self.axes[aj] else { continue };
                let (lo, hi) = over[kk].1;
                let at = |a: Rat, b: Rat| -> bool {
                    let mut o = over.clone();
                    o[kk].1 = (a, b);
                    outs(&o)
                };
                if let Some(cs) = split_out(lo, hi, *step, &mut left, &at) {
                    return Some(RowOut::Reach { step: s, axis: ua, by: by.clone(), row_by: row_by.clone(), cuts: Some((aj, cs)) });
                }
            }
        }
        None
    }

    /// Why row `ri` of the table of step `k` fires nowhere in the box `bx` by what it writes
    /// ([`RowOut::Writes`], §15.325): the table is one a way is joined through, and decides a column
    /// this table cuts; what the row puts there, through the roundings on the way, stands on no
    /// coordinate the box takes on it.
    fn writes_out(&self, bx: &[Vec<usize>], up: &Upstream, s: usize, k: usize, ri: usize, chk: &Checked) -> Option<RowOut> {
        let th = up.through.get(k.checked_sub(1 + up.chain.len())?)?;
        let aj = self.col_names.iter().position(|m| *m == th.name)?;
        let put = put_of(th.table, ri, th.oi, &th.rounds, chk);
        if matches!(put, Put::Any) {
            return None;
        }
        let cs = bx.get(aj)?;
        (!cs.iter().any(|&x| coord_takes(&self.axes[aj], x, &put))).then_some(RowOut::Writes { step: s, axis: aj })
    }

    /// Why the way fires nowhere in the box `bx` because two of its rows take no value of a column
    /// together ([`RowOut::Disjoint`], §15.325): for each two steps whose tables both cut a column,
    /// what the earlier row takes there — of what the box lets in — carried onto the later table's
    /// axis ([`carry`]), against what the later row takes there.
    fn disjoint_out(&self, bx: &[Vec<usize>], up: &Upstream, way: &[(usize, usize)]) -> Option<RowOut> {
        let parts: Vec<Option<Vec<Vec<bool>>>> = way
            .iter()
            .map(|&(k, ri)| {
                let reg = up.reg_of(k);
                (reg.unanalyzable.is_none() && ri < reg.masks.len()).then(|| self.part_in(bx, reg, ri))
            })
            .collect();
        for (s1, &(k1, _)) in way.iter().enumerate() {
            let Some(p1) = &parts[s1] else { continue };
            let reg1 = up.reg_of(k1);
            for (s2, &(k2, _)) in way.iter().enumerate().skip(s1 + 1) {
                let Some(p2) = &parts[s2] else { continue };
                let reg2 = up.reg_of(k2);
                for (u1, n) in reg1.col_names.iter().enumerate() {
                    let Some(u2) = reg2.col_names.iter().position(|m| m == n) else { continue };
                    let lets = carry(&reg1.axes[u1], &p1[u1], &reg2.axes[u2]);
                    if !(0..reg2.axes[u2].len()).any(|y| p2[u2].get(y) == Some(&true) && lets.get(y) == Some(&true)) {
                        return Some(RowOut::Disjoint { steps: (s1, s2), axes: (u1, u2) });
                    }
                }
            }
        }
        None
    }

    /// The axes of this table a value the rule works out is read with held (§15.325): the axes of
    /// numbers — not dates — whose columns the value reads, through the derives and `define`s it
    /// reads ([`reads_to_columns`]), each held to the span of the coordinates the box `bx` takes on
    /// it, an end no range writes read as the int64 claim reads it. An axis where the box takes the
    /// absent value, or one with no span, is not held. With the axes, by their places, the spans by
    /// the columns' names.
    #[allow(clippy::type_complexity)]
    fn held_spans(&self, n: &str, bx: &[Vec<usize>], chk: &Checked) -> (Vec<usize>, Vec<(String, (Rat, Rat))>) {
        let mut reads = BTreeSet::new();
        reads_to_columns(n, &self.exprs, &self.defines, &self.col_names, &mut reads, &mut BTreeSet::new());
        let mut by = Vec::new();
        let mut over = Vec::new();
        for (aj, m) in self.col_names.iter().enumerate() {
            if m == n || !reads.contains(m) || !matches!(self.axes[aj], Axis::Num { date: false, .. }) {
                continue;
            }
            let Some(cs) = bx.get(aj).filter(|cs| !cs.is_empty()) else { continue };
            let Some(spans) = cs.iter().map(|&x| closed_coord(&self.axes[aj], x, m, chk)).collect::<Option<Vec<_>>>() else { continue };
            let lo = spans.iter().map(|s| s.0).min_by(|a, b| a.cmp_to(*b));
            let hi = spans.iter().map(|s| s.1).max_by(|a, b| a.cmp_to(*b));
            let (Some(lo), Some(hi)) = (lo, hi) else { continue };
            by.push(aj);
            over.push((m.clone(), (lo, hi)));
        }
        (by, over)
    }

    /// The values worked out again from their own expressions while the names of `over` are held
    /// (§15.322): every derive and `define` the rule works out, other than `n` and the held names,
    /// that reads a held name through what it reads.
    fn inline_for(&self, n: &str, over: &[(String, (Rat, Rat))]) -> BTreeMap<String, Expr> {
        self.computed
            .iter()
            .filter(|(m, _)| m.as_str() != n && !over.iter().any(|(h, _)| h == *m))
            .filter(|(m, _)| {
                let mut r = BTreeSet::new();
                reads_through(m, &self.exprs, &self.defines, &mut r);
                r.iter().any(|x| over.iter().any(|(h, _)| h == x))
            })
            .map(|(m, x)| (m.clone(), x.clone()))
            .collect()
    }

    /// What row `ri` of the table of step `k` can still take in the box `bx` of this table
    /// (§15.325): on each axis of that table, the coordinates the row takes that meet what the box
    /// takes on the same column, read as [`carry`] reads it — a word or a truth value the same, a
    /// number where the closed ends meet — and every coordinate the row takes on a column this table
    /// has no axis for. A re-checker works the same out from the certificate's axes.
    fn part_in(&self, bx: &[Vec<usize>], reg: &TableRegion, ri: usize) -> Vec<Vec<bool>> {
        (0..reg.axes.len())
            .map(|ua| {
                let lets = match self.col_names.iter().position(|m| *m == reg.col_names[ua]) {
                    Some(aj) => {
                        let mut mine = vec![false; self.axes[aj].len()];
                        for &c in bx.get(aj).map(|v| v.as_slice()).unwrap_or(&[]) {
                            if let Some(m) = mine.get_mut(c) {
                                *m = true;
                            }
                        }
                        carry(&self.axes[aj], &mine, &reg.axes[ua])
                    }
                    None => vec![true; reg.axes[ua].len()],
                };
                (0..reg.axes[ua].len()).map(|x| reg.masks[ri][ua][x] && lets[x]).collect()
            })
            .collect()
    }

    /// The rows above row `ri` in the `policy first` table of step `k` that take all of what the
    /// row's box leaves in the box `bx` together (§15.325): those whose boxes meet what is left, in
    /// the table's order, when [`masks_cover`] shows within [`FIRST_COVER_STEPS`] that they cover it.
    /// Where nothing is left, no row is needed. `None` for a table under another policy, one with a
    /// column its region cannot read, or rows that do not cover it.
    fn step_out(&self, bx: &[Vec<usize>], up: &Upstream, s: usize, k: usize, ri: usize) -> Option<RowOut> {
        let reg = up.reg_of(k);
        if reg.unanalyzable.is_some() || ri >= reg.masks.len() {
            return None;
        }
        let part = self.part_in(bx, reg, ri);
        if part.iter().any(|a| !a.iter().any(|&x| x)) {
            return Some(RowOut::Misses { step: s });
        }
        if up.table_of(k).policy != Policy::TopDown {
            return None;
        }
        let rows: Vec<usize> = (0..ri)
            .filter(|&e| (0..reg.axes.len()).all(|a| (0..reg.axes[a].len()).any(|x| part[a][x] && reg.masks[e][a][x])))
            .collect();
        let over: Vec<&Vec<Vec<bool>>> = rows.iter().map(|&e| &reg.masks[e]).collect();
        let mut left = FIRST_COVER_STEPS;
        (masks_cover(&part, &over, &mut left) == Some(true)).then_some(RowOut::Earlier { step: s, rows })
    }

    /// Whether coordinate `ci` of axis `ai` is the truth value a boolean `define` on it never
    /// takes ([`truth_of_expr`]): `true` is the first coordinate of a boolean axis.
    pub(crate) fn truth_never(&self, ai: usize, ci: usize) -> bool {
        self.define_truth.get(ai).copied().flatten().is_some_and(|t| t != (ci == 0))
    }

    /// The first axis of a boolean `define` on which the path takes the truth value the define
    /// never takes (§15.196).
    pub(crate) fn truth_rules_out(&self, path: &[usize]) -> Option<usize> {
        path.iter().enumerate().find(|(ai, ci)| self.truth_never(*ai, **ci)).map(|(ai, _)| ai)
    }
}

impl TableRegion {
    /// The first axis of koyomi's days on which this path takes a coordinate with no day in it
    /// (§15.174): no input reaches anything below the path.
    pub fn days_rule_out(&self, path: &[usize]) -> Option<usize> {
        path.iter().enumerate().find(|(ai, ci)| !self.axes[*ai].holds_a_day(**ci)).map(|(ai, _)| ai)
    }

    /// Whether row `ri` takes no day of koyomi's on some axis of koyomi's days: every
    /// coordinate it takes there holds none (§15.174), so it never matches.
    pub fn misses_the_days(&self, ri: usize) -> Option<usize> {
        (0..self.axes.len()).find(|&ai| {
            matches!(&self.axes[ai], Axis::Num { days: Some(_), .. })
                && (0..self.axes[ai].len()).all(|ci| !self.masks[ri][ai][ci] || !self.axes[ai].holds_a_day(ci))
        })
    }

    /// The days of an axis of koyomi's, as the certificate's checker reads them.
    pub fn days_of_axis(&self, ai: usize) -> Option<&[i64]> {
        match &self.axes[ai] {
            Axis::Num { days: Some(ds), .. } => Some(ds.as_slice()),
            _ => None,
        }
    }

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
    /// The first and the last value each coordinate stands for, written as `coords` are
    /// ([`TableRegion::coord_ends`]). Not in the certificate: ritsu's port of rules carries it.
    pub ends: Vec<Option<(Option<String>, Option<String>)>>,
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
    /// For each of the table's own output columns that holds a word, the axis of the column of
    /// this table the row passes on into it, where its answer cell names one: the row writes what
    /// that column holds where it fires, so a value outside the row's box on that axis is one it
    /// never writes. `None` everywhere else (§15.261).
    pub passes: Vec<Option<usize>>,
    /// Where the row writes each word of `produces`, or the name of the column `passes` names, as
    /// `(line, byte column, byte length)`, in the row's own file: the answer cell of that column, on
    /// the row's line or, for a `clause`, on its `then` line. `None` where the row writes neither,
    /// and everywhere in a row an `apply` brought in.
    /// The certificate writes these for the rows of a table written in more than one place — a set of
    /// tables and clauses that decide one column, a contribution's rows among them — whose answer
    /// cells are not in the order the table decides its columns (§15.243).
    pub produced: Vec<Option<(usize, usize, usize)>>,
    /// What the row writes into each of the table's own output columns that holds a number or a
    /// date, in the order `CertTable::decides` names them — a literal or a name, the only two
    /// things an answer cell can be (§3.2) — with where the cell stands, and `None` for a column
    /// that holds neither. A re-checker works out from these the values the column can take, and
    /// so the range a table below reads it with, rather than taking the certificate's word for
    /// it (§15.196); and the dates a table below is asked for rows at (§15.325).
    #[allow(clippy::type_complexity)]
    pub writes: Vec<Option<(Expr, crate::types::Ty, Option<(usize, usize, usize)>)>>,
    /// For each write that names one of the table's own columns, the values the row's cell on
    /// that column lets through, where they narrow the column's range and that changes the
    /// range of the column written (`Checked::narrowed`, §15.213). `None` everywhere else.
    pub inside: Vec<Option<(crate::num::Rat, crate::num::Rat)>>,
    /// The member table the row was written in. Rows that share one were written in one
    /// table, so they have a cell in the same columns and in no others.
    pub origin: String,
    /// The line the row itself is written on, or 0 for a row an `apply` brought in. Every
    /// cell of the row is on this line; a `clause` whose `when` names no column has no
    /// cell at all, and this is the only thing that says where it is (§15.99).
    pub line: usize,
    /// The file the row is written in, when it is not the rule's own: a row a contribution adds
    /// (§15.231). Its spans are in that file, and the certificate pins it by its SHA-256.
    pub file: Option<std::sync::Arc<crate::diag::SpanFile>>,
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
    /// `100, 200` on a column of numbers: the values, each a point of the axis.
    In(Vec<Option<Rat>>),
    /// `not: 100, 200`.
    NotIn(Vec<Option<Rat>>),
    /// `>=medieval <industrial` on a column of an ordered enum (§15.210): each comparison and
    /// the value it is against. The re-checker rebuilds the values it takes from the order the
    /// certificate's `ordered` gives the enum.
    Rank(Vec<(&'static str, String)>),
}

/// What the certificate says about one table.
pub struct CertTable {
    /// The `never` lines this table reads (§15.282): the line's place among the rule's, and each
    /// axis's cell. A column the line does not test holds `Any`.
    pub nevers: Vec<(usize, Vec<CertCell>)>,
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
    ///
    /// Where the table has a linear model, the values come from solving it, and the values of
    /// the model's other names (`model_extra`, in that order) come with them: the point has to
    /// satisfy the model as well as the coordinates (§15.141).
    #[allow(clippy::type_complexity)]
    pub reach: Vec<(usize, Vec<usize>, Vec<(String, crate::diag::WVal)>, Vec<Option<Rat>>, Vec<Option<Rat>>)>,
    /// Rows a `apply` brought in that this rule's own bindings leave unused (§15.69). They
    /// are outside the reachability claim, and the certificate says which they are rather
    /// than passing over them.
    pub unused: Vec<usize>,
    /// Rows the sieve rules out entirely: every point of the row's box is one no input
    /// reaches. E102 reads the sieve only where a derive's reach takes part (§15.189), so
    /// `check` passes the rest — a row only a `constraint` rules out, for one — and the
    /// certificate names them rather than leaving a row with no point and no reason.
    pub unreachable: Vec<usize>,
    /// The completeness cover: the walk of §6.3, written down. `None` when it ran past the
    /// budget — a certificate says what it does not have.
    pub cover: Option<Cover>,
    /// The `constraint` lines a cover leaf can point at.
    /// Each side the names it adds: one for a side that is not a sum (§15.216).
    pub constraints: Vec<(Vec<String>, &'static str, Vec<String>)>,
    /// What the tables above rule out, written on this table's own axes. A leaf that rests
    /// on them is settled by a coordinate test; the facts themselves are earned back from
    /// the rows of the tables that decide the columns (§15.115).
    pub above: Vec<AboveFact>,
    /// The table's linear model (§15.141): its facts, by where each came from — a `derive`'s
    /// equation, a declared range, a `constraint` — in the order a refutation counts them.
    /// A re-checker builds each one again from the rule rather than reading it here.
    pub model: Vec<crate::fourier::Origin>,
    /// The names the model's facts use that are not columns of this table, in the order a
    /// point's values for them are given.
    pub model_extra: Vec<String>,
    /// The pairs the axes do not part and the model does: nothing that satisfies it reaches
    /// both rows, and the multipliers say why (§15.141).
    pub refuted: Vec<(usize, usize, crate::fourier::Refutation)>,
    /// The pairs the axes do not part and koyomi's days do (§15.174): on this axis of days, every
    /// coordinate both rows take holds none of them, so no input reaches both.
    pub days_apart: Vec<(usize, usize, usize)>,
    /// The pairs the rows of a table above part (§15.196): on the axis of a column that table
    /// decides, every value both rows take is written only by rows that fire nowhere in what
    /// both rows take, each with why.
    pub above_apart: Vec<(usize, usize, usize, Vec<AboveRows>)>,
    /// What a `define` comes to with the axes it reads held to one coordinate each, where that
    /// leaves coordinates of its own axis out (§15.322): each one the sieve reads, so a leaf that
    /// rests on one, and a point the sieve rules out by one, can be settled.
    pub narrowed: Vec<NarrowCut>,
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
    let walked = crate::cert::walked_names(f, c);
    Some(reg.certificate(t, &w114, &walked, &applied, cover, c, f))
}

/// One cell, resolved only as far as the units: the re-checker does the geometry.
pub(crate) fn cert_cell(cell: Option<&Cell>, ty: Option<Ty>) -> CertCell {
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
        // A set of numbers or dates is its values; one of words is its words.
        Some(Cell::Set(ls)) if ls.iter().all(|l| matches!(l, Lit::Num(_) | Lit::Date(..))) => {
            CertCell::In(ls.iter().map(|l| ty.as_ref().and_then(|t| lit_rat(l, t))).collect())
        }
        Some(Cell::Not(ls)) if ls.iter().all(|l| matches!(l, Lit::Num(_) | Lit::Date(..))) => {
            CertCell::NotIn(ls.iter().map(|l| ty.as_ref().and_then(|t| lit_rat(l, t))).collect())
        }
        Some(Cell::Set(ls)) => CertCell::Is(ls.iter().map(word).collect()),
        Some(Cell::Not(ls)) => CertCell::Not(ls.iter().map(word).collect()),
        Some(Cell::Prefix(ps)) => CertCell::Prefix(ps.clone()),
        Some(Cell::Ranked { ops, .. }) => CertCell::Rank(ops.iter().map(|(o, v)| (o.word(), v.clone())).collect()),
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
    #[allow(clippy::too_many_arguments)]
    pub fn certificate(&self, t: &Table, w114: &[(usize, usize)], walked: &[String], applied: &[bool], cover: Option<Cover>, chk: &Checked, f: &RuleFile) -> CertTable {
        let inputs = &f.inputs;
        let unique = t.policy == crate::ast::Policy::Unique;
        let axes: Vec<CertAxis> = (0..self.axes.len())
            .map(|ai| CertAxis {
                name: self.col_names[ai].clone(),
                kind: if f.open_value(&self.col_names[ai]).is_some() {
                    // An open value (§15.227): its range is declared, as an input's is, and what the
                    // contributions bring stays inside it (E131, `opens` of the certificate).
                    "open"
                } else if self.derived[ai].is_some() {
                    "derived"
                } else if self.is_define[ai] || f.items.iter().any(|it| matches!(it, Item::Define(d) if d.name.text == self.col_names[ai])) {
                    // A `define` of any type: a truth value and a number alike (§15.196). A
                    // number's column was called `upstream` here before, which said the wrong
                    // thing about where its points come from.
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
                ends: self.coord_ends(ai),
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
                    // A cell of an optional column is read in the type it wraps (§15.201).
                    .map(|ai| cert_cell(row.cells.get(self.display_of[ai]), chk.ty_of(&self.col_names[ai]).map(|t| t.present().clone())))
                    .collect(),
                accepts: (0..self.axes.len())
                    .map(|ai| (0..self.axes[ai].len()).filter(|&c| self.masks[ri][ai][c]).collect())
                    .collect(),
                produces: (0..t.outputs.len())
                    .map(|oi| written_value(row.outs.get(oi), chk).map(str::to_string))
                    .collect(),
                passes: (0..t.outputs.len())
                    .map(|oi| passed_column(t, ri, oi, chk).and_then(|n| self.col_names.iter().position(|x| x == n)))
                    .collect(),
                produced: (0..t.outputs.len())
                    .map(|oi| {
                        if written_value(row.outs.get(oi), chk).is_none() && passed_column(t, ri, oi, chk).is_none() {
                            return None;
                        }
                        if applied.get(ri).copied().unwrap_or(false) {
                            return None;
                        }
                        let span = row.out_spans.get(oi)?;
                        Some((span.line, span.col, span.len))
                    })
                    .collect(),
                writes: (0..t.outputs.len())
                    .map(|oi| {
                        let ty = chk.ty_of(&t.outputs[oi].name.text)?;
                        let ty = match ty {
                            crate::types::Ty::Opt(t) => *t,
                            t => t,
                        };
                        // A date too, as its day number (§15.325): a table below that reads it is
                        // asked for rows only at the dates the rows here write.
                        if !matches!(ty, crate::types::Ty::Money { .. } | crate::types::Ty::Qty { .. } | crate::types::Ty::Rate | crate::types::Ty::Number | crate::types::Ty::Date) {
                            return None;
                        }
                        let span = row.out_spans.get(oi).cloned().unwrap_or_else(|| row.span.clone());
                        let e = match row.outs.get(oi)? {
                            OutCell::Lit(Lit::Num(n)) => Expr::Lit(Lit::Num(n.clone()), span.clone()),
                            OutCell::Lit(Lit::Date(y, m, d)) => Expr::Lit(Lit::Date(*y, *m, *d), span.clone()),
                            OutCell::Name(w) => Expr::Name(w.clone(), span.clone()),
                            _ => return None,
                        };
                        let at = (!applied.get(ri).copied().unwrap_or(false)).then_some((span.line, span.col, span.len));
                        Some((e, ty, at))
                    })
                    .collect(),
                inside: (0..t.outputs.len())
                    .map(|oi| {
                        if chk.narrowed.contains(&t.outputs[oi].name.text) {
                            chk.row_written(t, row, oi)
                        } else {
                            None
                        }
                    })
                    .collect(),
                origin: row.origin.clone().unwrap_or_else(|| t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()),
                file: if applied.get(ri).copied().unwrap_or(false) { None } else { row.span.file.clone() },
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
        // Unless the model rules out what both rows take, and then the multipliers that
        // say so are the proof (§15.141).
        let mut refuted = Vec::new();
        if !self.model.is_empty() {
            undecided.retain(|&(a, b)| match self.refute_box(&self.pair_box(a - 1, b - 1)) {
                Some(r) => {
                    refuted.push((a, b, r));
                    false
                }
                None => true,
            });
        }
        // Or koyomi's days part them: every coordinate both take on an axis of days holds none
        // of the days (§15.174).
        let mut days_apart = Vec::new();
        undecided.retain(|&(a, b)| {
            let on = (0..self.axes.len()).find(|&ai| {
                self.days_of_axis(ai).is_some() && (0..self.axes[ai].len()).all(|c| !(self.masks[a - 1][ai][c] && self.masks[b - 1][ai][c]) || !self.axes[ai].holds_a_day(c))
            });
            match on {
                Some(ai) => {
                    days_apart.push((a, b, ai));
                    false
                }
                None => true,
            }
        });
        // Or the rows of a table above do: every value both take on a column that table decides is
        // written only by rows that fire nowhere in what both take (§15.196).
        let ups = upstreams(t, chk, f);
        let mut above_apart = Vec::new();
        undecided.retain(|&(a, b)| match self.above_apart(a - 1, b - 1, &ups, chk) {
            Some((ai, at)) => {
                above_apart.push((a, b, ai, at));
                false
            }
            None => true,
        });
        let model: Vec<crate::fourier::Origin> = self.model.iter().flat_map(|g| g.sys.iter().map(|q| q.origin.clone())).collect();
        let mut model_extra: Vec<String> = self
            .model
            .iter()
            .flat_map(|g| g.vars.iter().cloned())
            .filter(|n| !self.col_names.iter().zip(&self.axes).any(|(c, a)| c == n && matches!(a, Axis::Num { .. })))
            .collect();
        model_extra.sort();
        model_extra.dedup();

        let (mut reach, mut unused, mut unreachable) = (Vec::new(), Vec::new(), Vec::new());
        let mut left = crate::region::DEFAULT_BUDGET;
        // The values a column decided above holds where the cover rests on the rows of that table
        // (§15.195): a point that reaches a row with one of them has values one of those rows
        // fires on, and the re-checkers hold it to that.
        // And where a pair rests on them: a re-checker reads those rows into its sieve as it reads a
        // leaf's, so a point with one of those values has to have values one of the rows fires on.
        // Each comes with how its ways are joined (§15.325), which the re-checkers hold the point
        // to as well.
        let keys = self.above_keys(cover.as_ref(), &above_apart);
        let held = |p: &[usize]| -> bool {
            self.above_rows_out(p, &ups, chk).is_none()
                && (!keys.iter().any(|k| p.get(k.axis) == Some(&k.coord)) || self.above_values(p, &model_extra, &ups, chk, &keys).is_some())
        };
        for ri in 0..t.rows.len() {
            match self.reach_point(ri, unique, &mut left, &held) {
                Some(p) => {
                    // The values behind the point, solved against the constraints — or,
                    // where there is a linear model, against the model. Where no assignment
                    // was found the certificate states none, and a re-checker falls back to
                    // the weaker reading and says which one it used (§15.99).
                    let solved = self.above_values(&p, &model_extra, &ups, chk, &keys).or_else(|| self.model_values(&p, &model_extra));
                    let (nums, extra, input) = match solved {
                        Some((nums, extra)) => {
                            let input = self.witness_pairs_with(&p, Some(nums.clone()));
                            (nums, extra, input)
                        }
                        None if self.model.is_empty() => {
                            let vals = self.reach_values(&p);
                            (vals.clone().unwrap_or_else(|| vec![None; self.axes.len()]), Vec::new(), self.witness_pairs_with(&p, vals))
                        }
                        None => (vec![None; self.axes.len()], vec![None; model_extra.len()], self.witness_pairs(&p)),
                    };
                    reach.push((ri + 1, p, input, nums, extra));
                }
                None if applied.get(ri).copied().unwrap_or(false) => unused.push(ri + 1),
                // The sieve rules out every point of the row. E102 reads the sieve only where a
                // derive's reach takes part (§15.189), so `check` is silent about the rest;
                // dropping the row here made the certificate look as though it had simply
                // forgotten one (§15.99).
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
                // The rows' boxes leave out every value no table above writes (`reachable`), not
                // only the ones a leaf of the cover rests on, and a re-checker takes a box back from
                // its cells with the coordinates of the stated facts left out (§15.253).
                for (axis, coord) in self.never_facts(&ups, chk) {
                    if !v.iter().any(|f| matches!(f, AboveFact::Never { axis: x, coord: y } if *x == axis && *y == coord)) {
                        v.push(AboveFact::Never { axis, coord });
                    }
                }
                v
            },
            cover,
            nevers: self.never_cert(f, chk),
            constraints: self.constraint_list(),
            model,
            model_extra,
            refuted,
            days_apart,
            above_apart,
            narrowed: self.narrowed.clone(),
        }
    }

    /// The values behind a point that satisfy the linear model (§15.141): each of its systems
    /// solved with the point's coordinates held, the axes' values in axis order and the
    /// others' in the order `extra` names them. `None` when there is no model, or a system
    /// could not be solved.
    #[allow(clippy::type_complexity)]
    fn model_values(&self, p: &[usize], extra: &[String]) -> Option<(Vec<Option<Rat>>, Vec<Option<Rat>>)> {
        use crate::fourier::{Lin, Origin};
        if self.model.is_empty() {
            return None;
        }
        let bx = self.path_box(p);
        let mut at: BTreeMap<String, Rat> = BTreeMap::new();
        for g in &self.model {
            let mut sys = g.sys.clone();
            for (ai, name) in self.col_names.iter().enumerate() {
                if !g.vars.contains(name) {
                    continue;
                }
                let Some((lo, hi)) = self.ends_of(ai, &bx[ai]) else { continue };
                let off = |x: Rat| Lin::var(name).plus(&Lin::con(x.mul(Rat::int(-1))));
                if let Some((x, strict)) = lo {
                    sys.push(off(x).ge(strict).tag(Origin::Coord { axis: ai, hi: false }));
                }
                if let Some((x, strict)) = hi {
                    sys.push(off(x).le(strict).tag(Origin::Coord { axis: ai, hi: true }));
                }
            }
            let got = crate::fourier::solve(sys.clone())?;
            // Hold the answer to every inequality before handing it on: a point that misses
            // one is a witness to nothing.
            if !sys.iter().all(|q| q.holds_at(&got) == Some(true)) {
                return None;
            }
            at.extend(got);
        }
        let axes = (0..self.axes.len())
            .map(|ai| match &self.axes[ai] {
                Axis::Num { .. } => at.get(&self.col_names[ai]).copied().or_else(|| self.axes[ai].witness_num(p[ai], None).map(|x| self.inside_reach(ai, p[ai], x))),
                _ => None,
            })
            .collect();
        Some((axes, extra.iter().map(|n| at.get(n).copied()).collect()))
    }

    /// The values behind a point that takes one of `keys` — a column decided above, at a value
    /// the cover rests on the rows of that table for (§15.195) — chosen so that one of the rows
    /// that write the value fires on them: the linear model (or, for a type it says nothing
    /// about, the numeric axes alone, each inside its reach) solved with the point's coordinates
    /// held and that row's ends lent, and the answer held to the row's own coordinates — not
    /// merely to their hull — and to its words. The rows are tried in their table's order, and
    /// the first that holds is taken. `None` when the point takes none of the keys, or no row
    /// holds; the caller then picks another point.
    #[allow(clippy::type_complexity)]
    fn above_values(&self, p: &[usize], extra: &[String], ups: &[Upstream], chk: &Checked, keys: &[AboveKey]) -> Option<(Vec<Option<Rat>>, Vec<Option<Rat>>)> {
        // For each key the point takes, every way its value arrives: a row that writes it, joined
        // with the rows of the tables above it (§15.322), and through the columns both tables cut
        // where the key's are (§15.325). A re-checker leaves out a way a box holding the point says
        // never decides there (`earlier`, `misses`); no point of such a box comes here, since
        // `above_rows_out` finds the same reasons at each point of it and the search turns the
        // point down first.
        let mut choices: Vec<Vec<(&Upstream, Path)>> = Vec::new();
        for key in keys {
            let (axis, coord) = (key.axis, key.coord);
            if p.get(axis) != Some(&coord) {
                continue;
            }
            let up = ups.iter().find(|u| self.col_names.get(axis) == Some(&u.name))?;
            if !matches!(self.axes[axis], Axis::Enum { .. } | Axis::Bool | Axis::Num { .. }) {
                return None;
            }
            let mut writers = Vec::new();
            for ri in 0..up.table.rows.len() {
                let put = up.put(ri, chk);
                if put == Put::Any {
                    return None;
                }
                if coord_takes(&self.axes[axis], coord, &put) {
                    let ways = if key.through { up.paths_through(ri, self, chk)? } else { up.paths(ri, self, chk)? };
                    writers.extend(ways.into_iter().map(|w| (up, w)));
                }
            }
            choices.push(writers);
        }
        if choices.is_empty() {
            return None;
        }
        // One row per key, every combination in turn: a point takes few keys, and the first row
        // that holds is the usual answer.
        let mut at_k = vec![0usize; choices.len()];
        for _ in 0..256 {
            let combo: Vec<(&Upstream, Path)> = choices.iter().zip(&at_k).filter_map(|(c, &k)| c.get(k).cloned()).collect();
            if combo.len() == choices.len()
                && let Some(v) = self.values_under(p, extra, chk, &combo)
            {
                return Some(v);
            }
            let mut k = 0;
            loop {
                if k == at_k.len() {
                    return None;
                }
                at_k[k] += 1;
                if at_k[k] < choices[k].len() {
                    break;
                }
                at_k[k] = 0;
                k += 1;
            }
        }
        None
    }

    /// The values behind point `p` with the rows `combo` of the tables above firing (see
    /// [`TableRegion::above_values`]).
    #[allow(clippy::type_complexity)]
    fn values_under(&self, p: &[usize], extra: &[String], chk: &Checked, combo: &[(&Upstream, Path)]) -> Option<(Vec<Option<Rat>>, Vec<Option<Rat>>)> {
        use crate::fourier::{Lin, Origin};
        // Every row of every way, each with the table it is a row of: they all fire together.
        let steps: Vec<(&TableRegion, usize)> = combo.iter().flat_map(|(up, way)| way.iter().map(move |&(k, ri)| (up.reg_of(k), ri))).collect();
        // the words each row asks of a column both tables cut, held to the point exactly
        for &(reg, ri) in &steps {
            for (ua, n) in reg.col_names.iter().enumerate() {
                let Some(aj) = self.col_names.iter().position(|m| m == n) else { continue };
                if !matches!((&self.axes[aj], &reg.axes[ua]), (Axis::Enum { .. }, Axis::Enum { .. }) | (Axis::Bool, Axis::Bool)) {
                    continue;
                }
                let mut mine = vec![false; self.axes[aj].len()];
                mine[p[aj]] = true;
                let lets = carry(&self.axes[aj], &mine, &reg.axes[ua]);
                if !(0..reg.axes[ua].len()).any(|x| reg.masks[ri][ua][x] && lets[x]) {
                    return None;
                }
            }
        }
        let bx = self.path_box(p);
        let mut systems: Vec<(Ty, Vec<String>, Vec<crate::fourier::Ineq>)> =
            self.model.iter().map(|g| (g.want.clone(), g.vars.clone(), g.sys.clone())).collect();
        for (aj, n) in self.col_names.iter().enumerate() {
            if !matches!(self.axes[aj], Axis::Num { .. }) {
                continue;
            }
            let Some(ty) = chk.ty_of(n) else { continue };
            // the reach of a derived or `define` column, which a re-checker holds the value to
            let mut bound = Vec::new();
            if let Some((lo, hi)) = self.reach_at(aj) {
                let off = |x: Rat| Lin::var(n).plus(&Lin::con(x.mul(Rat::int(-1))));
                if let Some(x) = lo {
                    bound.push(off(x).ge(false));
                }
                if let Some(x) = hi {
                    bound.push(off(x).le(false));
                }
            }
            // one system for each unit, however it is spelled (§15.332)
            match systems.iter_mut().find(|s| s.0.same(&ty)) {
                Some(s) => {
                    if !s.1.contains(n) {
                        s.1.push(n.clone());
                    }
                    s.2.extend(bound);
                }
                None => systems.push((ty, vec![n.clone()], bound)),
            }
        }
        let mut at: BTreeMap<String, Rat> = BTreeMap::new();
        for (_, vars, facts) in systems {
            let mut sys = facts;
            for (aj, n) in self.col_names.iter().enumerate() {
                if !vars.contains(n) {
                    continue;
                }
                let Some((lo, hi)) = self.ends_of(aj, &bx[aj]) else { continue };
                let off = |x: Rat| Lin::var(n).plus(&Lin::con(x.mul(Rat::int(-1))));
                if let Some((x, strict)) = lo {
                    sys.push(off(x).ge(strict).tag(Origin::Coord { axis: aj, hi: false }));
                }
                if let Some((x, strict)) = hi {
                    sys.push(off(x).le(strict).tag(Origin::Coord { axis: aj, hi: true }));
                }
            }
            for &(reg, ri) in &steps {
                for (ua, n) in reg.col_names.iter().enumerate() {
                    if !vars.contains(n) || !matches!(reg.axes[ua], Axis::Num { .. }) {
                        continue;
                    }
                    let cs: Vec<usize> = (0..reg.axes[ua].len()).filter(|&x| reg.masks[ri][ua][x]).collect();
                    let Some((lo, hi)) = reg.ends_of(ua, &cs) else { continue };
                    let off = |x: Rat| Lin::var(n).plus(&Lin::con(x.mul(Rat::int(-1))));
                    if let Some((x, strict)) = lo {
                        sys.push(off(x).ge(strict));
                    }
                    if let Some((x, strict)) = hi {
                        sys.push(off(x).le(strict));
                    }
                }
            }
            let got = crate::fourier::solve(sys.clone())?;
            if !sys.iter().all(|q| q.holds_at(&got) == Some(true)) {
                return None;
            }
            at.extend(got);
        }
        // Each row's own coordinates, exactly: a value inside the hull of a row that takes two
        // coordinates apart is not a value the row takes.
        for &(reg, ri) in &steps {
            for (ua, n) in reg.col_names.iter().enumerate() {
                let Axis::Num { coords, .. } = &reg.axes[ua] else { continue };
                let Some(v) = at.get(n) else { continue };
                let takes = coords.iter().enumerate().any(|(x, cd)| {
                    reg.masks[ri][ua][x]
                        && match cd {
                            Coord::Absent => false,
                            Coord::Point(w) => w.cmp_to(*v) == std::cmp::Ordering::Equal,
                            Coord::Open(a, b) => {
                                a.is_none_or(|a| a.cmp_to(*v) == std::cmp::Ordering::Less) && b.is_none_or(|b| v.cmp_to(b) == std::cmp::Ordering::Less)
                            }
                        }
                });
                if !takes {
                    return None;
                }
            }
        }
        let axes = (0..self.axes.len())
            .map(|ai| match &self.axes[ai] {
                Axis::Num { .. } => at.get(&self.col_names[ai]).copied().or_else(|| self.axes[ai].witness_num(p[ai], None).map(|x| self.inside_reach(ai, p[ai], x))),
                _ => None,
            })
            .collect();
        Some((axes, extra.iter().map(|n| at.get(n).copied()).collect()))
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
    fn reach_point(&self, ri: usize, unique: bool, budget: &mut i64, held: &dyn Fn(&[usize]) -> bool) -> Option<Vec<usize>> {
        let mut p: Vec<usize> = Vec::new();
        self.reach_rec(ri, unique, &mut p, budget, &[], held)
    }

    /// A point that reaches a row **with one axis held at one coordinate** (§15.148): a call
    /// from one given state that the row answers, which is what a machine's certificate needs
    /// to show a final state can still be reached from that state. The values behind it are
    /// worked out as a reach point's are.
    #[allow(clippy::type_complexity)]
    pub fn point_at(
        &self,
        ri: usize,
        unique: bool,
        axis: usize,
        coord: usize,
    ) -> Option<(Vec<usize>, Vec<(String, crate::diag::WVal)>, Vec<Option<Rat>>, Vec<Option<Rat>>)> {
        self.point_at_all(ri, unique, &[(axis, coord)])
    }

    /// `point_at`, with a coordinate fixed on each of several axes: the state's, and those of
    /// the inputs a case keeps (§15.149).
    pub fn point_at_all(
        &self,
        ri: usize,
        unique: bool,
        fix: &[(usize, usize)],
    ) -> Option<(Vec<usize>, Vec<(String, crate::diag::WVal)>, Vec<Option<Rat>>, Vec<Option<Rat>>)> {
        let mut left = DEFAULT_BUDGET;
        let mut path = Vec::new();
        let p = self.reach_rec(ri, unique, &mut path, &mut left, fix, &|_| true)?;
        let model_extra = self.model_extra_names();
        let (nums, extra, input) = match self.model_values(&p, &model_extra) {
            Some((nums, extra)) => {
                let input = self.witness_pairs_with(&p, Some(nums.clone()));
                (nums, extra, input)
            }
            None if self.model.is_empty() => {
                let vals = self.reach_values(&p);
                (vals.clone().unwrap_or_else(|| vec![None; self.axes.len()]), Vec::new(), self.witness_pairs_with(&p, vals))
            }
            None => (vec![None; self.axes.len()], vec![None; model_extra.len()], self.witness_pairs(&p)),
        };
        Some((p, input, nums, extra))
    }

    /// The names the linear model reads that are not a numeric axis of the table: the
    /// `extra_values` of a reach point, in this order.
    fn model_extra_names(&self) -> Vec<String> {
        let mut model_extra: Vec<String> = self
            .model
            .iter()
            .flat_map(|g| g.vars.iter().cloned())
            .filter(|n| !self.col_names.iter().zip(&self.axes).any(|(c, a)| c == n && matches!(a, Axis::Num { .. })))
            .collect();
        model_extra.sort();
        model_extra.dedup();
        model_extra
    }

    /// `held` is asked of a whole point last: what the caller holds a point to besides the sieve.
    #[allow(clippy::too_many_arguments)]
    fn reach_rec(&self, ri: usize, unique: bool, p: &mut Vec<usize>, budget: &mut i64, fix: &[(usize, usize)], held: &dyn Fn(&[usize]) -> bool) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if p.len() == self.axes.len() {
            if self.feasible(p) == Feasible::No || self.defines_rule_out(p).is_some() || self.refutes_path(p) {
                return None;
            }
            if !unique && (0..ri).any(|k| (0..self.axes.len()).all(|a| self.masks[k][a][p[a]])) {
                return None;
            }
            if !held(p) {
                return None;
            }
            return Some(p.clone());
        }
        let ai = p.len();
        for c in 0..self.axes[ai].len() {
            if !self.masks[ri][ai][c] || fix.iter().any(|&(fa, fc)| fa == ai && fc != c) {
                continue;
            }
            p.push(c);
            let keep = self.feasible(p) != Feasible::No && !self.define_out_of_reach(ai, p);
            let got = if keep { self.reach_rec(ri, unique, p, budget, fix, held) } else { None };
            p.pop();
            if got.is_some() {
                return got;
            }
        }
        None
    }
}

/// The (axis, coordinate) pairs a cover rests on the rows of a table above for (§15.195), each once.
/// A value decided above that the cover, or a pair of rows, rests on the rows of (§15.195): its
/// axis and coordinate, and whether its ways are joined through the columns both tables cut
/// (§15.325). A re-checker reads the same out of the certificate.
pub(crate) struct AboveKey {
    axis: usize,
    coord: usize,
    through: bool,
}

/// One leaf or pair, read into the keys.
fn note_above_key(out: &mut Vec<AboveKey>, a: &AboveRows) {
    if !out.iter().any(|k| k.axis == a.axis && k.coord == a.coord && k.through == a.through) {
        out.push(AboveKey { axis: a.axis, coord: a.coord, through: a.through });
    }
}

impl TableRegion {
    /// Every value the cover and the pairs rest on the rows of a table above for ([`AboveKey`]).
    fn above_keys(&self, cover: Option<&Cover>, pairs: &[(usize, usize, usize, Vec<AboveRows>)]) -> Vec<AboveKey> {
        let mut out = Vec::new();
        if let Some(cv) = cover {
            self.above_keys_in(cv, &mut Vec::new(), &mut out);
        }
        for (_, _, _, at) in pairs {
            for x in at {
                note_above_key(&mut out, x);
            }
        }
        out
    }

    fn above_keys_in(&self, cv: &Cover, path: &mut Vec<usize>, out: &mut Vec<AboveKey>) {
        match cv {
            Cover::Split(kids) => {
                for (c, k) in kids.iter().enumerate() {
                    path.push(c);
                    self.above_keys_in(k, path, out);
                    path.pop();
                }
            }
            Cover::ByAboveRows(a) => note_above_key(out, a),
            _ => {}
        }
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
    /// A `never` line takes the whole box, by its place in the table's list (§15.282).
    ByNever(usize),
    ByDerived(usize),
    /// Every point of the box was asked about one at a time, and the sieve ruled each one
    /// out. The re-checker redoes exactly that (§15.98), with the facts about the tables above the
    /// points rest on, each with the point's text, which the certificate states (§15.325).
    ByPoints(Vec<(String, Apart)>),
    /// The coordinate the path takes on this axis of koyomi's days holds none of them
    /// (§15.174).
    ByDays(usize),
    /// The tables above cannot produce this combination. When the reason can be written
    /// small — two spans on one input that do not meet — it comes with it and is re-checked
    /// like any other leaf; otherwise the leaf is bare and is **stated, not proved**
    /// (§15.115).
    ByUpstream(String, Option<Apart>),
    /// The rule's linear model has no solution anywhere in the box, and the multipliers that
    /// say so (§15.141).
    ByFarkas(crate::fourier::Refutation),
    /// The coordinate the path takes on the axis of a `define` lies outside the values the
    /// define can come to (§15.195), as `ByDerived` says it of a derive.
    ByDefine(usize),
    /// The coordinate the path takes on the axis of a boolean `define` is the truth value the
    /// define never takes over the ranges of the inputs (§15.196).
    ByTruth(usize),
    /// The value a column decided above holds here is written only by rows of that table none of
    /// which fires anywhere in the box, each with why (§15.195).
    ByAboveRows(AboveRows),
    /// A `define`, with the axes it reads held to the coordinates this box takes, does not come to
    /// the coordinate the box takes on its axis: the table's [`NarrowCut`] by its place (§15.322).
    ByNarrowed(usize),
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
            return self.empty_cover(path, ups, chk, budget);
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

    /// The cover of a box no row takes (§15.98, §15.141). The reason has to hold for the
    /// **whole box**, not for one point of it: an axis the path has not fixed keeps its
    /// declared range, which is what `ends_of_name` does when the path is short. The reasons
    /// are tried in order — a `constraint` that cannot hold anywhere in the box, a derived
    /// axis outside its own reach, the tables above, the rule's linear model, and the sieve
    /// point by point — and where none holds for the whole box but the model may rule its
    /// parts out, the box is split on its next axis and each part covered the same way. A
    /// point no reason covers is a gap: `check` reports it as E101 and the rule does not
    /// pass, so there is no certificate to write.
    fn empty_cover(&self, path: &mut Vec<usize>, ups: &[Upstream], chk: &Checked, budget: &mut i64) -> Option<Cover> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if let Some(ai) = self.days_rule_out(path) {
            return Some(Cover::ByDays(ai));
        }
        for (k, con) in self.constraints.iter().enumerate() {
            if self.constraint_impossible(con, path) {
                return Some(Cover::ByConstraint(k));
            }
        }
        if let Some(k) = self.never_rules_out(path) {
            return Some(Cover::ByNever(k));
        }
        for ai2 in 0..path.len() {
            if self.derived[ai2].is_some() && self.derived_out_of_reach(ai2, path) {
                return Some(Cover::ByDerived(ai2));
            }
        }
        for ai2 in 0..path.len() {
            if self.define_out_of_reach(ai2, path) {
                return Some(Cover::ByDefine(ai2));
            }
        }
        if let Some(ai2) = self.truth_rules_out(path) {
            return Some(Cover::ByTruth(ai2));
        }
        if let Some(k) = self.narrowed_out(path) {
            return Some(Cover::ByNarrowed(k));
        }
        // A box that fixes a column decided above at a value no row of the deciding table writes
        // is out of reach whatever the axes it leaves open hold, for one fact a re-checker reads
        // off those rows (§15.253). Asked of each point under it instead, the re-checkers would need
        // the fact all the same, and before it was stated the leaf could rest on nothing written.
        // At a whole point the test below finds the same reason first.
        if path.len() < self.axes.len()
            && let Some(never) = self.never_at(path, ups, chk)
        {
            return Some(Cover::ByUpstream(self.box_text(path), Some(never)));
        }
        if path.len() == self.axes.len() && self.upstream_dead_at(path, ups, chk) {
            let apart = self.apart_at(path, ups, chk);
            // With no reason small enough to write, the rows that write the value are one, each
            // with why it does not fire here (§15.195); a bare leaf is stated rather than proved.
            if apart.is_none()
                && let Some(a) = self.above_rows_out(path, ups, chk)
            {
                return Some(Cover::ByAboveRows(a));
            }
            return Some(Cover::ByUpstream(self.witness_text(path), apart));
        }
        if !self.model.is_empty() {
            if let Some(r) = self.refute_box(&self.path_box(path)) {
                return Some(Cover::ByFarkas(r));
            }
        }
        if let Some(a) = self.above_rows_out(path, ups, chk) {
            return Some(Cover::ByAboveRows(a));
        }
        // Point by point, then: the box is only impossible when every point in it is. A re-checker
        // asks each point what the sieve and the stated facts say, so a point that rests on a
        // reason only a leaf of its own carries — the rows of a table above, each with why — is
        // split down to (§15.325).
        let mut down = false;
        if self.first_reachable(path, ups, chk, budget, false).is_none() {
            if let Some(facts) = self.points_stated(path, ups, chk, budget) {
                return Some(Cover::ByPoints(facts));
            }
            down = true;
        }
        // What the model or a table above rules out may hold only of parts of the box.
        let parts = down || !self.model.is_empty() || !self.narrowed.is_empty() || ups.iter().any(|u| self.above_can(u));
        if path.len() == self.axes.len() || !parts {
            return None;
        }
        let ai = path.len();
        let mut kids = Vec::with_capacity(self.axes[ai].len());
        for c in 0..self.axes[ai].len() {
            path.push(c);
            let k = self.empty_cover(path, ups, chk, budget);
            path.pop();
            kids.push(k?);
        }
        Some(Cover::Split(kids))
    }

    /// Whether the coordinate this path takes on a derived axis lies outside the interval
    /// the derive can reach — the half of the sieve that looks at one derived value alone. An
    /// interval coordinate leaves its ends out, as a `define`'s does (§15.304). The linear model
    /// holds a derive's reach only beside an equation it reads or a `constraint`, so a table over
    /// `min(a, 100)` with rows up to 100 was asked for a row at 101.
    fn derived_out_of_reach(&self, ai: usize, path: &[usize]) -> bool {
        let Some(Some((reach, _))) = self.derived.get(ai) else { return false };
        self.outside_reach(ai, path, *reach)
    }

    /// The coordinates of an axis as closed intervals of true values, for the axes where
    /// that means anything. `None` at an end is unbounded.
    pub fn coord_bounds(&self, ai: usize) -> Vec<Option<(Option<Rat>, Option<Rat>)>> {
        (0..self.axes[ai].len()).map(|ci| self.coord_span(ai, ci)).collect()
    }

    /// The `never` lines of this table, as the re-checker reads them: each axis's cell.
    fn never_cert(&self, f: &RuleFile, chk: &Checked) -> Vec<(usize, Vec<CertCell>)> {
        self.nevers
            .iter()
            .map(|n| {
                let line = &f.nevers[n.line];
                let cells = self
                    .col_names
                    .iter()
                    .map(|name| cert_cell(line.conds.iter().find(|k| k.name.text == *name).map(|k| &k.cell), chk.ty_of(name).map(|t| t.present().clone())))
                    .collect();
                (n.line, cells)
            })
            .collect()
    }

    /// The rule's `constraint` lines, as the certificate names them.
    pub fn constraint_list(&self) -> Vec<(Vec<String>, &'static str, Vec<String>)> {
        self.constraints.iter().map(|k| (k.left.clone(), k.op.word(), k.right.clone())).collect()
    }
}



/// The region of one definition set, for a certificate section that needs points of its own
/// (§15.148).
pub fn region_of(set: &crate::defset::DefSet, c: &Checked, f: &RuleFile) -> Option<TableRegion> {
    let reg = TableRegion::build(&set.table, c, f)?;
    if reg.axes.is_empty() || set.table.rows.is_empty() || reg.unanalyzable.is_some() {
        return None;
    }
    Some(reg)
}
