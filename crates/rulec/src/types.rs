//! Types, names, and rounding (§2, §7). Produces diagnostics E011, E103, E104, E106, W111.

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::num::{Rat, RoundMode, RoundTo};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// A declared or imported enumeration.
    Enum(String),
    /// §2.1: currency and tax are a double brand. `tax: None` is a bare literal,
    /// which unifies with either brand.
    Money { cur: String, tax: Option<String> },
    /// `mass[g]` / `length[cm]` — a dimension plus the declared unit.
    Qty { dim: String, unit: String },
    Rate,
    /// §2.1: a whole number that carries no unit — a count, a number of days, a score. It is
    /// dimensionless like a rate, but its values are whole and it has no step to declare.
    Number,
    Bool,
    Str,
    Date,
    /// `T?`. Consumed only by `none` in a cell; it never appears in an expression (§2.1).
    Opt(Box<Ty>),
    Unknown,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Enum(n) => write!(f, "{n}"),
            Ty::Money { cur, tax: Some(t) } => write!(f, "{}[{cur}, {t}]", crate::kw::MONEY),
            Ty::Money { cur, tax: None } => write!(f, "{}[{cur}]", crate::kw::MONEY),
            Ty::Qty { dim, unit } => write!(f, "{dim}[{unit}]"),
            Ty::Rate => write!(f, "{}", crate::kw::RATE),
            Ty::Number => write!(f, "{}", crate::kw::NUMBER),
            Ty::Bool => write!(f, "{}", crate::kw::BOOL),
            Ty::Str => write!(f, "{}", crate::kw::STRING),
            Ty::Date => write!(f, "{}", crate::kw::DATE),
            Ty::Opt(t) => write!(f, "{t}?"),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

impl Ty {
    /// The type as ritsu's unit (ritsu's DESIGN 5.2), with the step a rate's integer counts —
    /// the one `Checked::wire_scale` gives, as 1/scale. The spelling stays as written (`JPY`
    /// stays `JPY`). None for a type that is not a number, and for a spelling the table of units
    /// does not have: a unit no literal can be written in, or a tax word other than `incl_tax`
    /// and `excl_tax`, which the type keeps as written.
    pub fn unit(&self, step: Option<Rat>) -> Option<ritsu_units::Unit> {
        match self {
            Ty::Money { cur, tax } => {
                let tax = match tax {
                    Some(t) => Some(ritsu_units::Tax::parse(t)?),
                    None => None,
                };
                ritsu_units::Unit::money(cur, tax)
            }
            Ty::Qty { dim, unit } => ritsu_units::Unit::quantity(dim, unit),
            Ty::Rate => Some(ritsu_units::Unit::rate(step)),
            Ty::Number => Some(ritsu_units::Unit::number()),
            _ => None,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number)
    }

    /// The type of a value that is there: the type itself, or the one an optional wraps. A
    /// cell, a range and a step of an optional column are read in it, and only `none` tests the
    /// absent value (§2.1, §15.201).
    pub fn present(&self) -> &Ty {
        match self {
            Ty::Opt(t) => t.as_ref(),
            t => t,
        }
    }
    /// Do two types describe the same quantity? Money literals carry no tax brand,
    /// so they unify with either.
    pub fn unifies(&self, o: &Ty) -> bool {
        match (self, o) {
            (Ty::Money { cur: a, tax: ta }, Ty::Money { cur: b, tax: tb }) => {
                a == b && (ta.is_none() || tb.is_none() || ta == tb)
            }
            (Ty::Unknown, _) | (_, Ty::Unknown) => true,
            _ => self == o,
        }
    }
}

/// The dimension a type belongs to, or `None` for one that carries no unit. A currency is a
/// dimension of its own (§15.18), so two of them are as unrelated as grams and centimetres.
fn dim_of(t: &Ty) -> Option<String> {
    match t {
        Ty::Money { cur, .. } => Some(format!("{}/{cur}", crate::kw::MONEY)),
        Ty::Qty { dim, .. } => Some(dim.clone()),
        _ => None,
    }
}

/// The currencies a literal may be written in, as ISO 4217 codes: ritsu's table of units
/// (`ritsu_units::table`, ritsu's DESIGN 5.1), which began as this file's. A closed set on
/// purpose: with any identifier allowed, `100lbs` would pass as an amount in a currency called
/// "lbs". Each also has a hundredth, spelled by appending `c` — `USD` and `USDc` — which is the
/// relation `円` already had to `銭`, and `mass[kg]` to `mass[g]`.
pub use ritsu_units::table::CURRENCIES;

/// (dimension, factor to that dimension's base unit), from ritsu's table of units. A currency is
/// its own dimension, so the name is `money/<currency>`; everything else is the dimension it
/// belongs to.
///
/// **Two currencies never convert into one another.** There is no exchange rate in this tool
/// and there must not be one, so the currency is the dimension rather than a brand inside a
/// single "money" dimension. Mixing them is E103, exactly like adding grams to yen.
///
/// The imperial factors are exact rationals (a pound is 453.59237 g on the nose), so they
/// cost nothing in precision. What they do cost is scale: `1lb` in a `mass[g]` column is
/// not a whole number of grams and is refused, which is right — a pound is not writable in
/// a column that counts grams.
fn unit_info(u: &str) -> Option<(String, Rat)> {
    ritsu_units::table::unit(u).map(|(dim, f)| (dim.key(), f))
}

/// What a unit adds after its factor, to reach its dimension's base unit: `base = v × f + o`.
/// Every unit but ℉ has a zero where its dimension has one, so this is 0 for all of them and
/// the arithmetic below reduces to the scaling it always was.
///
/// ℉ is the one entry that is not a scaling: 41℉ is exactly 5℃, and −40 is the same in both.
/// It is exact — 5/9 and −160/9 are rationals — and it is safe here **because a temperature
/// has no arithmetic**: the offset would be wrong on a difference (a rise of 9℉ is a rise of
/// 5℃, not of −27.2℃), and E048 is what guarantees no difference is ever formed.
fn unit_offset(u: &str) -> Rat {
    ritsu_units::table::offset(u)
}

/// Dimensions that are ordered but not arithmetic (§15.84). `date` is the same shape and was
/// always described this way in §2.1 — "comparison and range only, no arithmetic" — but
/// nothing enforced it, so `甲 - 乙` between two dates typed clean.
pub fn compares_only(t: &Ty) -> bool {
    match t {
        Ty::Date => true,
        Ty::Qty { dim, .. } => ritsu_units::Dim::of_word(dim).is_some_and(|d| d.compares_only()),
        _ => false,
    }
}

/// Every unit a literal may carry, for the message that names them when one is not known.
/// The currencies are given as the rule rather than as thirty-one codes: a list that long
/// in a diagnostic is skipped, and the rule is what the reader has to know anyway.
pub fn units() -> String {
    tr!(
        "質量 mg g kg t oz lb · 長さ mm cm m km in ft yd mi · \
         面積 mm2 cm2 m2 a ha km2 坪 in2 ft2 yd2 mi2 ac · 体積 mm3 cm3 mL L m3 kL · \
         時間 ms s min h d w · 温度 ℃ ℉ · 音量 dB · 率 % · 金額 円 銭、\
         または ISO 4217 のコード（その 1/100 はコードに c を付ける。USD と USDc）",
        "mass mg g kg t oz lb · length mm cm m km in ft yd mi · \
         area mm2 cm2 m2 a ha km2 坪 (tsubo) in2 ft2 yd2 mi2 ac · volume mm3 cm3 mL L m3 kL · \
         duration ms s min h d w · temperature ℃ ℉ · sound dB · rate % · money 円 (yen, the same as JPY) 銭 (a hundredth of 円), \
         or an ISO 4217 code (its hundredth is the code plus c: USD and USDc)"
    )
}

/// The note a numeric literal gets when its suffix is not a unit at all. Without it, E103
/// says only that the value does not fit the column, which for a typo in the unit is the
/// one thing the reader already knows.
fn unit_note(n: &crate::lex::Num) -> Option<String> {
    let u = n.unit.as_deref()?;
    unit_info(u).is_none().then(|| {
        tr!(
            "`{u}` は単位ではありません。書けるのは {} です。",
            "`{u}` is not a unit. These are all of them: {}.",
            units()
        )
    })
}

/// A date is held as the **day count since the epoch** (proleptic Gregorian calendar,
/// 1970-01-01 = 0).
///
/// `y*10000 + m*100 + d` would preserve the ordering too, but it leaves gaps of integers
/// that are not real dates at every month end (20260332..20260400). A table tiled with
/// `<=2026-03-31` and `>=2026-04-01` is actually complete, yet it would get a **false
/// E101** because the phantom integers are not covered. Tiling with inclusive bounds is
/// the natural way to write business rules (§3.1), so this would be hit every time.
///
/// With day counts, adjacency is exactly +1, so the ±1 at boundaries, the emptiness test,
/// the merging of neighbors, and the write-back of witnesses all stay correct with the
/// existing integer machinery, unchanged.
pub fn date_ord(y: i32, m: u32, d: u32) -> Rat {
    Rat::int(days_from_civil(y, m as i64, d as i64))
}

pub fn ord_to_date(v: Rat) -> (i32, u32, u32) {
    let (y, m, d) = civil_from_days(v.num / v.den);
    (y as i32, m as u32, d as u32)
}

/// Howard Hinnant's days_from_civil. Proleptic Gregorian calendar.
fn days_from_civil(y: i32, m: i64, d: i64) -> i128 {
    let y = y as i64 - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe - 719468) as i128
}

fn civil_from_days(z: i128) -> (i64, i64, i64) {
    let z = z as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod date_tests {
    use super::*;

    #[test]
    fn 通算日は往復して隣接が一になる() {
        for (y, m, d) in [(2026, 4, 1), (2026, 3, 31), (2024, 2, 29), (1970, 1, 1), (1999, 12, 31)] {
            let o = date_ord(y, m, d);
            assert_eq!(ord_to_date(o), (y, m, d), "{y}-{m}-{d} round trip");
        }
        // The last day of a month and the first day of the next are adjacent. This is why
        // no phantom integer gap appears.
        let a = date_ord(2026, 3, 31);
        let b = date_ord(2026, 4, 1);
        assert_eq!(b.sub(a), Rat::int(1), "3/31 and 4/1 are adjacent");
        // Leap day
        let c = date_ord(2024, 2, 28);
        assert_eq!(date_ord(2024, 2, 29).sub(c), Rat::int(1));
        assert_eq!(date_ord(2024, 3, 1).sub(date_ord(2024, 2, 29)), Rat::int(1));
        // In a common year the day after 2/28 is 3/1
        assert_eq!(date_ord(2025, 3, 1).sub(date_ord(2025, 2, 28)), Rat::int(1));
    }
}

/// The number a literal writes, before its unit: `100万` is 1000000 and `1.5` is 3/2. Exact,
/// because §2.1 asks for `0.5%` to be the rational 1/200 rather than a rounded decimal.
fn magnitude(n: &crate::lex::Num) -> Option<Rat> {
    let whole: i128 = n.digits.parse().ok()?;
    let den = 10i128.checked_pow(u32::try_from(n.frac.len()).ok()?)?;
    let frac: i128 = if n.frac.is_empty() { 0 } else { n.frac.parse().ok()? };
    let v = Rat::new(whole.checked_mul(den)?.checked_add(frac)?, den).mul(Rat::int(n.mult as i128));
    Some(if n.neg { Rat::zero().sub(v) } else { v })
}

/// A literal as a value that can be held against what a document says (§15.82): the magnitude
/// in its dimension's base unit, and the dimension when it has one. A bare number has none and
/// compares equal to any dimension's value of the same magnitude — a copy whose unit sits in
/// the header says `990` where the rule writes `990円`, and they are the same amount.
pub fn comparable(n: &crate::lex::Num) -> Option<(Option<String>, Rat)> {
    let v = magnitude(n)?;
    match n.unit.as_deref() {
        None => Some((None, v)),
        Some(u) => {
            let (dim, f) = unit_info(u)?;
            Some((Some(dim), v.mul(f).add(unit_offset(u))))
        }
    }
}

/// Whether two such values are the same amount. A value with no dimension is the same as one
/// with a dimension when the magnitudes agree: the document left the unit in its header.
pub fn same_value(a: &(Option<String>, Rat), b: &(Option<String>, Rat)) -> bool {
    a.1 == b.1 && (a.0.is_none() || b.0.is_none() || a.0 == b.0)
}

/// Value of a numeric literal, expressed in `want`'s declared unit.
/// `None` means the literal's unit does not belong to `want`'s dimension, or that a decimal
/// was written where the declared unit only has whole values.
fn lit_value_in(n: &crate::lex::Num, want: &Ty) -> Option<Rat> {
    let v = magnitude(n)?;
    // A number carries no unit, and that is the whole point of it: `3` is three of whatever
    // the column counts. Nothing else accepts a bare literal (§3).
    if matches!(want, Ty::Number) {
        return if n.unit.is_none() && v.is_int() { Some(v) } else { None };
    }
    let unit = n.unit.as_deref()?;
    let (dim, f) = unit_info(unit)?;
    // §2.1: money and quantities are integers in their declared unit, and only a rate is an
    // exact rational. `1.5kg` is fine in a `mass[g]` column because it lands on 1500g; `0.5円`
    // is not a value of `money[円]` at all.
    let whole = |v: Rat| if v.is_int() { Some(v) } else { None };
    match want {
        // Money goes through the declared unit's factor exactly as a quantity does. It did
        // not use to, which made `money[銭]` read `5円` as 5 rather than 500.
        Ty::Money { cur, .. } => {
            let (d, fd) = unit_info(cur)?;
            (dim == d).then(|| whole(v.mul(f).div(fd)))?
        }
        Ty::Qty { dim: d, unit: du } if dim == *d => {
            let (_, fd) = unit_info(du)?;
            // `base = v × f + o`, then back out of the declared unit the same way. For every
            // dimension but temperature both offsets are 0 and this is the old scaling.
            let base = v.mul(f).add(unit_offset(unit));
            whole(base.sub(unit_offset(du)).div(fd))
        }
        Ty::Rate if dim == crate::kw::RATE => Some(v.mul(f)),
        _ => None,
    }
}

/// Type of a numeric literal read on its own, before any expectation.
fn lit_ty(n: &crate::lex::Num) -> Ty {
    match n.unit.as_deref().and_then(unit_info) {
        Some((d, _)) if d.starts_with(crate::kw::MONEY) => {
            Ty::Money { cur: n.unit.clone().unwrap(), tax: None }
        }
        Some((d, _)) if d == crate::kw::RATE => Ty::Rate,
        Some((d, _)) => Ty::Qty { dim: d, unit: n.unit.clone().unwrap() },
        // A literal with no unit is a plain number. It used to be "unknown", which unified
        // with anything and let `3` stand where a yen amount was meant.
        None if n.unit.is_none() => Ty::Number,
        None => Ty::Unknown,
    }
}

#[derive(Debug, Clone)]
pub struct Sym {
    pub ty: Ty,
    pub span: Span,
    pub kind: SymKind,
    pub contract_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymKind {
    Input,
    /// A field of one element of the sequence a `fold` walks (§15.56). It is an input, but
    /// one the caller passes once per element rather than once per call.
    Element,
    Derived,
    Define,
    TableOut,
    Output,
}

pub struct Checked {
    /// The built-in namespaces the rule imports (§15.182), for the hints of E012.
    pub imported: Vec<crate::prelude::Import>,
    pub syms: HashMap<String, Sym>,
    /// Output name → rounding (mode and grid). E106 checks that literals sit on this grid.
    pub roundings: HashMap<String, (RoundMode, Rat)>,
    /// Input name → declared range. E112 uses it to check that the reachable interval is
    /// contained.
    pub ranges: HashMap<String, (Option<Rat>, Option<Rat>)>,
    /// Table output name → the values that table can actually produce. A downstream row that
    /// names a value the upstream table never produces is unreachable (a variant of §11 E102).
    pub out_values: HashMap<String, Vec<String>>,
    /// Boolean definition name → the names its expression mentions. Used to decide whether
    /// a witness can be constructed.
    pub define_deps: HashMap<String, Vec<String>>,
    /// Derived name → the inputs it depends on. Used to see whether two derived values share
    /// an input: when they do, a sieve of independent intervals per derived value cannot see
    /// the dependency (§6.2).
    pub derived_deps: HashMap<String, Vec<String>>,
    /// `a <= b` pairs the caller guarantees, from `constraint` lines. `allocate` needs one
    /// to know that a running total stays within the whole it is a share of (§15.102).
    pub at_most: HashSet<(String, String)>,
    /// Name → reciprocal of the step: the value is a multiple of 1/k. Under §7.1's
    /// "a single int64 plus a static rational scale", the stored integer is value×k.
    pub scales: HashMap<String, i128>,
    /// A computed value → the step its values sit on, worked out from its expression
    /// (§15.190): `amount × 10%` takes tenths of a pound. The storage step above can be finer
    /// (`10%` brings a hundredth along), and the axes of §6 and the boundary vectors of §9 need
    /// the step the values really take. [`Checked::value_step`] reads it, falling back to the
    /// storage step.
    pub grids: HashMap<String, Rat>,
    /// A rate output → the reciprocal of the step it travels at: the step it declares, or
    /// with none declared its rounding grid, which every answer sits on (§15.144). `scales`
    /// holds the value inside the code, which may be finer; this is what a caller counts.
    pub out_scales: HashMap<String, i128>,
    /// Enum name -> values, in declaration order (§6.3 picks the first as a witness).
    pub enums: HashMap<String, Vec<String>>,
    /// The enums in the order they were declared, the imported ones first. Two enums may
    /// share a value's name (§15.150); where nothing says which one is meant, the first wins,
    /// the same one every run.
    pub enum_order: Vec<String>,
    /// Group name -> the enum it belongs to and its members.
    pub groups: HashMap<String, (String, Vec<String>)>,
    /// The enums whose values are ordered as written (§15.210). The order itself is `enums`.
    pub ordered: HashSet<String>,
    /// Enum name -> the display name of each value, in the enum's order, for an enum that gives
    /// any (§15.211). A value without one is shown by its name.
    pub labels: HashMap<String, Vec<Option<String>>>,
    /// Input name -> the values its `range` lets in, in order, for an input of an ordered enum
    /// that declares one (§15.210): the universe the checks quantify over, and what the
    /// generated code takes at its door.
    pub enum_ranges: HashMap<String, Vec<String>>,
    /// Enum name -> the `enums` file the rule reads it from (§15.212).
    pub owners: HashMap<String, crate::ast::EnumOwner>,
    pub used: HashSet<String>,
    /// Enum values named in a cell or expression, as `(enum, value)`: `automatic` named in a
    /// column of one enum leaves another enum's `automatic` unnamed. W111 uses it to report
    /// "values that appear in no row".
    pub used_values: HashSet<(String, String)>,
    pub diags: Vec<Diag>,
    /// The definition sets: one per table, or one per output that several tables define
    /// (§15.66). Everything downstream — the region checks, the evaluator, the
    /// generators, the vectors, the page for people — works on these.
    pub sets: Vec<crate::defset::DefSet>,
    /// Table name → its set, and whether the set is evaluated at that table's position.
    pub set_of: HashMap<String, (usize, bool)>,
    /// Date input name → the days it takes, when its range is a date of a koyomi file
    /// (`range from koyomi …`, §15.174) or `checked_over` holds it to a set. The input's range
    /// is the hull of the days; the region sieves out every other day, and the generated code
    /// refuses it.
    pub day_sets: HashMap<String, Days>,
    /// What E132 and E139 were said about: the divisor of a rounding whose range reaches zero,
    /// and the argument of an `isqrt` whose range goes below it, each with the span its
    /// diagnostic marks (§15.213, §15.215). Once the check is done, the input that makes one so
    /// is looked for and named in the diagnostic (`lib::enrich_arith`).
    pub arith_witness: Vec<Witnessed>,
    /// The columns of numbers whose range is narrower for what the rows' own cells let through:
    /// a row that writes one of its table's columns writes only the values its cell there lets
    /// through (§15.213). The certificate says so on each such row (`inside`).
    pub narrowed: HashSet<String>,
}

/// One expression E132 or E139 was said about (see [`Checked::arith_witness`]).
#[derive(Clone, Debug)]
pub struct Witnessed {
    pub code: &'static str,
    pub span: Span,
    pub expr: Expr,
}

/// The two halves of a rounding whose argument divides by a value that is not a constant:
/// `half_even(attack * 1000 / (attack + defense), 1)` (§15.213). The division is the rounding's
/// own — the outermost operation of its first argument — and anywhere else `÷` keeps its constant
/// divisor (E115). A divisor written as a literal is the constant division it always was.
pub fn var_division<'e>(name: &str, args: &'e [Expr]) -> Option<(&'e Expr, &'e Expr)> {
    RoundMode::parse(name)?;
    match args {
        [Expr::Bin(n, BinOp::Div, d, _), _] if !matches!(**d, Expr::Lit(..)) => Some((n, d)),
        _ => None,
    }
}

/// The largest whole number whose square does not pass `v`, for `v` at least zero: the answer
/// `isqrt` gives (§15.215). `None` below zero.
pub fn isqrt_rat(v: Rat) -> Option<Rat> {
    if v.num < 0 {
        return None;
    }
    let x = v.num.div_euclid(v.den);
    // The square of anything up to 2^63 fits in an i128, and no value the generated code holds is
    // larger than that (E108).
    let (mut lo, mut hi) = (0i128, x.min(1 << 63));
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        if mid * mid <= x {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(Rat::int(lo))
}

/// The remainder of `v` divided by the positive whole `n`, from 0 up to but not including `n`,
/// whatever the sign of `v`: what `mod` gives (§15.214).
pub fn mod_rat(v: Rat, n: i128) -> Rat {
    let q = v.div(Rat::int(n));
    let fl = q.num.div_euclid(q.den);
    v.sub(Rat::int(n).mul(Rat::int(fl)))
}

/// The days a date input takes (§15.174).
#[derive(Clone, Debug)]
pub struct Days {
    /// Where they come from, as the rule writes it; None for the days `checked_over` was given.
    pub from: Option<crate::ast::DaysFrom>,
    /// The koyomi file as rulec reached it: the rule's directory joined with what is written.
    pub file: String,
    /// The SHA-256 of the koyomi file, in hex; empty for the days `checked_over` was given.
    pub sha256: String,
    /// In order, as rulec's ordinals of dates (days since 1970-01-01).
    pub days: Vec<i64>,
}

impl Days {
    /// Whether some day lies between `lo` and `hi`, both ends included where they are given.
    pub fn any_within(&self, lo: Option<Rat>, hi: Option<Rat>) -> bool {
        self.first_within(lo, hi).is_some()
    }

    /// The first day between `lo` and `hi`, both ends included where they are given.
    pub fn first_within(&self, lo: Option<Rat>, hi: Option<Rat>) -> Option<i64> {
        self.days.iter().copied().find(|d| {
            let v = Rat::int(*d as i128);
            lo.is_none_or(|l| l.cmp_to(v) != std::cmp::Ordering::Greater) && hi.is_none_or(|h| v.cmp_to(h) != std::cmp::Ordering::Greater)
        })
    }

    /// The days written as dates.
    pub fn shown(&self) -> Vec<String> {
        self.days.iter().map(|d| {
            let (y, m, dd) = ord_to_date(Rat::int(*d as i128));
            format!("{y:04}-{m:02}-{dd:02}")
        }).collect()
    }
}

impl Checked {
    /// The set a table belongs to.
    pub fn set_of_table(&self, t: &Table) -> Option<&crate::defset::DefSet> {
        let name = t.name.as_ref().map(|n| n.text.as_str()).unwrap_or("");
        self.set_of.get(name).map(|(i, _)| &self.sets[*i])
    }

    /// The label of a row, by the table it was written in and its position there.
    pub fn label_of(&self, table: &str, row: usize) -> Option<&str> {
        let (i, _) = self.set_of.get(table)?;
        let set = &self.sets[*i];
        set.table
            .rows
            .iter()
            .find(|r| r.index == row && r.origin.as_deref().map(|o| o == table).unwrap_or(true))
            .and_then(|r| r.label.as_ref().map(|l| l.text.as_str()))
    }

    /// What to evaluate, check or generate at the position of `t`: the merged table of its
    /// set when `t` is the member the set is evaluated at, `t`'s own copy when it stands
    /// alone, and nothing when the set is evaluated at a later member.
    pub fn table_at(&self, t: &Table) -> Option<&Table> {
        let name = t.name.as_ref().map(|n| n.text.as_str()).unwrap_or("");
        match self.set_of.get(name) {
            Some((i, true)) => Some(&self.sets[*i].table),
            Some((_, false)) => None,
            // A table outside every set (examples, or a rule whose sets were not built).
            None => None,
        }
    }
}

/// The names that only have a value **inside a walk**: a field of one element, and everything
/// computed from one (§15.58), of any sequence the rule walks (§15.219).
pub fn element_scoped(f: &RuleFile) -> HashSet<String> {
    stages(f).each.keys().cloned().collect()
}

/// What a rule that walks sequences answers with once a walk is over (§15.208, §15.219): the name
/// each `fold` binds its answer to — the one after its `->`, else the first output — and the name of
/// every `count`, `sum`, `max` and `min`. Nothing, for a rule that walks no sequence.
pub fn walk_answers(f: &RuleFile) -> Vec<String> {
    let mut out = Vec::new();
    for it in &f.items {
        match it {
            Item::Fold(d) => out.extend(d.answer_name(f).map(str::to_string)),
            Item::Agg(d) => out.push(d.name.text.clone()),
            _ => {}
        }
    }
    out
}

/// Where an item of a rule that walks sequences runs (§15.208, §15.219).
///
/// The walks are numbered from 1. A walk over a sequence reads what the walks before it answered:
/// an item of one element that reads what walk 1 answered runs in walk 2 — the second walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Once per call, after the walks numbered up to this one have run: 0 is before every walk. It
    /// reads no element, and the walks whose answers it reads, through what it reads, end at this one.
    Once(usize),
    /// Once per element of the sequence `seq` (its place among the rule's `elements`), in walk
    /// `walk` over it — the first walk the values it reads are all there in. A summary (`count`,
    /// `sum`, `max`, `min`, `fold`) is the walk itself, and is marked with the walk it reduces in.
    Each { seq: usize, walk: usize },
}

impl Stage {
    /// Before every walk: reads no element and nothing a walk answers.
    pub const BEFORE: Stage = Stage::Once(0);
}

/// The stages of a rule's items, and the names on each side of every walk (§15.208, §15.219).
pub struct Stages {
    /// One per item of the rule, in the order written. A rule that walks nothing has every item
    /// `Once(0)`.
    pub of: Vec<Stage>,
    /// The names that have a value only inside a walk — the fields of an element and everything
    /// computed from one — with the sequence they belong to.
    pub each: HashMap<String, usize>,
    /// Each name's walk: for a name of one element, the first walk it is there in; for a name with
    /// one value per call, the last walk whose answer it reads (0 for one that reads none). An input
    /// is not listed, and is 0.
    pub level: HashMap<String, usize>,
    /// The last walk.
    pub depth: usize,
    /// The items that read the elements of two sequences, by index, with a name of each: one walk
    /// inside another (E031).
    pub two: Vec<(usize, String, String)>,
    /// The folds whose answer comes back into what they read — the rule's first output read by
    /// the table that decides its verdict — by index, with the name through which it comes back
    /// (E021). No walk can run before itself.
    pub cycle: Vec<(usize, String)>,
    /// What each item reads, the tables of one output together (`item_reads`).
    reads: Vec<Vec<String>>,
    /// What each item binds (`item_binds`).
    binds: Vec<Vec<String>>,
    /// Whether each item is a summary: a `count`, `sum`, `max`, `min` or `fold`.
    summary: Vec<bool>,
}

impl Stages {
    /// The stage of the item at `i`.
    pub fn at(&self, i: usize) -> Stage {
        self.of.get(i).copied().unwrap_or(Stage::BEFORE)
    }

    /// What the item at `i` reads, the tables of one output together.
    pub fn reads_of(&self, i: usize) -> &[String] {
        self.reads.get(i).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The walk a name is there after (a name of one call) or in (a name of one element).
    pub fn level_of(&self, name: &str) -> usize {
        self.level.get(name).copied().unwrap_or(0)
    }

    /// The names worked out from what some walk answers: one value per call, after a walk.
    pub fn after(&self) -> HashSet<String> {
        self.level.iter().filter(|(n, l)| **l > 0 && !self.each.contains_key(*n)).map(|(n, _)| n.clone()).collect()
    }

    /// The walks the rule runs, in the order they run: by number, and within one number in the
    /// order the sequences are declared. A walk is there when a summary reduces in it, or an item
    /// of one element that nothing reads is in it; an item of one element that something reads is
    /// worked out where that is (`body`).
    pub fn walks(&self) -> Vec<(usize, usize)> {
        let read: HashSet<&String> = self.reads.iter().flatten().collect();
        let mut out: Vec<(usize, usize)> = self
            .of
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Stage::Each { seq, walk } if self.summary[i] || !self.binds[i].iter().any(|b| read.contains(b)) => Some((*walk, *seq)),
                _ => None,
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The summaries that reduce in walk `walk` over sequence `seq`, in the order written.
    pub fn summaries_in(&self, f: &RuleFile, seq: usize, walk: usize) -> Vec<usize> {
        (0..f.items.len())
            .filter(|&i| matches!(f.items[i], Item::Agg(_) | Item::Fold(_)) && self.at(i) == Stage::Each { seq, walk })
            .collect()
    }

    /// The items worked out on each element in walk `walk` over sequence `seq`, in the order
    /// written: what the walk's summaries read, through every item of one element in between —
    /// including those of an earlier walk over the same sequence, worked out again from the same
    /// element, since a value of one element is gone once its walk is over (§15.219) — and an item
    /// of this walk that nothing reads at all, which still runs where it always did.
    pub fn body(&self, f: &RuleFile, seq: usize, walk: usize) -> Vec<usize> {
        let n = f.items.len();
        let read: HashSet<&String> = self.reads.iter().flatten().collect();
        let mut take = vec![false; n];
        let mut todo: Vec<usize> = Vec::new();
        for (i, it) in f.items.iter().enumerate() {
            if self.at(i) != (Stage::Each { seq, walk }) {
                continue;
            }
            match it {
                Item::Agg(_) | Item::Fold(_) => todo.push(i),
                _ if !item_binds(f, it).iter().any(|b| read.contains(b)) => {
                    take[i] = true;
                    todo.push(i);
                }
                _ => {}
            }
        }
        while let Some(i) = todo.pop() {
            for r in &self.reads[i] {
                for (j, it) in f.items.iter().enumerate() {
                    if take[j] || matches!(it, Item::Agg(_) | Item::Fold(_)) {
                        continue;
                    }
                    if let Stage::Each { seq: s, walk: w } = self.at(j) {
                        if s == seq && w <= walk && item_binds(f, it).iter().any(|x| x == r) {
                            take[j] = true;
                            todo.push(j);
                        }
                    }
                }
            }
        }
        (0..n).filter(|&i| take[i]).collect()
    }

    /// The items that run once, after walk `level` (0: before every walk), in the order written.
    pub fn once(&self, level: usize) -> Vec<usize> {
        (0..self.of.len()).filter(|&i| self.of[i] == Stage::Once(level)).collect()
    }
}

/// The names an item binds: a `derive`'s or a `define`'s name, a table's output columns, a
/// summary's name. A `fold` binds its answer, to the name after its `->` — the rule's first output
/// when there is none — so it needs the rule to say which that is (`item_binds`).
pub fn item_names(it: &Item) -> Vec<String> {
    match it {
        Item::Derived(d) => vec![d.name.text.clone()],
        Item::Define(d) => vec![d.name.text.clone()],
        Item::Table(t) => t.outputs.iter().map(|o| o.name.text.clone()).collect(),
        Item::Agg(d) => vec![d.name.text.clone()],
        Item::Fold(d) => d.answer.iter().map(|a| a.name.text.clone()).collect(),
    }
}

/// The names an item binds in its rule: [`item_names`], with the first output for a fold that
/// names no answer.
pub fn item_binds(f: &RuleFile, it: &Item) -> Vec<String> {
    match it {
        Item::Fold(d) => d.answer_name(f).map(|n| vec![n.to_string()]).unwrap_or_default(),
        _ => item_names(it),
    }
}

/// The names an item reads: a `derive`'s or a `define`'s expression, a table's columns and the
/// names its output cells write (an output cell that names an input passes it on, so the table
/// reads it as surely as a column), a summary's column and the column its `where` tests, and a
/// fold's verdict, the values its arms take and the values of `empty` and `exhausted` (`held`, the
/// fold's own, is not a name of the rule).
pub fn item_reads(it: &Item) -> Vec<String> {
    let mut out = Vec::new();
    match it {
        Item::Derived(d) => collect_names(&d.expr, &mut out),
        Item::Define(d) => collect_names(&d.expr, &mut out),
        Item::Table(t) => {
            out.extend(t.inputs.iter().map(|(n, _)| n.clone()));
            for row in &t.rows {
                for o in &row.outs {
                    if let OutCell::Name(w) = o {
                        out.push(w.clone());
                    }
                }
            }
        }
        Item::Agg(d) => {
            out.push(d.column.text.clone());
            if let Some((w, _)) = &d.filter {
                out.push(w.text.clone());
            }
        }
        Item::Fold(d) => {
            out.push(d.verdict.clone());
            for e in d.empty.iter().chain(d.exhausted.iter()) {
                collect_names(e, &mut out);
            }
            for (_, arm, _) in &d.arms {
                match arm {
                    Arm::Stop(Some(x)) => collect_names(x, &mut out),
                    Arm::Take { expr, .. } => collect_names(expr, &mut out),
                    Arm::KeepMax { expr, key } => {
                        collect_names(expr, &mut out);
                        collect_names(key, &mut out);
                    }
                    _ => {}
                }
            }
            out.retain(|n| n != crate::kw::HELD);
        }
    }
    out
}

/// The stages of the items of a rule that walks sequences (§15.208, §15.219).
///
/// An item runs **once** when it reads no element's field, and **once per element** of a sequence
/// when it reads a field of that sequence or something that does (§15.58). Which walk it runs in,
/// or after, is decided by what the walks answer: a summary reduces in the first walk the values it
/// reads are there in; an item of one call that reads what walk k answered runs after walk k; an
/// item of one element that reads it runs in walk k + 1 — the second walk, for k = 1. The split is
/// derived rather than declared (§15.58), and the loop repeats until nothing changes, so that it does
/// not depend on the order the items are written in. A walk is only ever made to wait for the walks
/// its summaries read, so no rule has more walks than summaries: a fold whose answer comes back into
/// its own verdict would wait for itself, and is stopped there (`cycle`). An item that reads the
/// elements of two sequences belongs to neither, and is listed in `two`.
pub fn stages(f: &RuleFile) -> Stages {
    let n = f.items.len();
    let seq_index = |name: &str| f.elements.iter().position(|e| e.name.text == name);
    let mut each: HashMap<String, usize> = HashMap::new();
    let mut level: HashMap<String, usize> = HashMap::new();
    for (s, el) in f.elements.iter().enumerate() {
        for fd in &el.fields {
            each.insert(fd.name.text.clone(), s);
            level.insert(fd.name.text.clone(), 1);
        }
    }
    // The tables that define one output are evaluated together, as one merged table at the last
    // of them (§15.66), so each of them reads what any of them reads and they share a stage.
    let mut by_output: HashMap<String, Vec<String>> = HashMap::new();
    for it in &f.items {
        if let Item::Table(t) = it {
            for oc in &t.outputs {
                by_output.entry(oc.name.text.clone()).or_default().extend(item_reads(it));
            }
        }
    }
    let reads: Vec<Vec<String>> = f
        .items
        .iter()
        .map(|it| match it {
            Item::Table(t) => t.outputs.iter().flat_map(|oc| by_output.get(&oc.name.text).cloned().unwrap_or_default()).collect(),
            _ => item_reads(it),
        })
        .collect();
    let binds: Vec<Vec<String>> = f.items.iter().map(|it| item_binds(f, it)).collect();
    // A name two items decide (a fold's answer and a `sum` of the same name: E021) takes its
    // stage from the first of them. Each writing its own walk into it in turn, the two never
    // agreed, and `check` did not come back.
    let mut owner: HashMap<&str, usize> = HashMap::new();
    for (i, bs) in binds.iter().enumerate() {
        for b in bs {
            owner.entry(b.as_str()).or_insert(i);
        }
    }
    let summaries = f.items.iter().filter(|it| matches!(it, Item::Agg(_) | Item::Fold(_))).count();
    let mut of = vec![Stage::BEFORE; n];
    let mut two: Vec<(usize, String, String)> = Vec::new();
    let mut cycle: Vec<(usize, String)> = Vec::new();
    let mut frozen = vec![false; n];
    // Every walk is at most one past the summaries, and a name joins the values of one element at
    // most once, so the stages settle well inside this many rounds; the bound is there so that a
    // shape nobody foresaw still lets the check end (the core of rulec: every check halts).
    let rounds = (n + 1) * (summaries + 2) * 2;
    for _ in 0..rounds {
        let mut changed = false;
        for (i, it) in f.items.iter().enumerate() {
            let rs = &reads[i];
            let summary = matches!(it, Item::Agg(_) | Item::Fold(_));
            let over = match it {
                Item::Agg(d) => seq_index(&d.over),
                Item::Fold(d) => seq_index(&d.over),
                _ => None,
            };
            let stage = if summary && frozen[i] {
                of[i]
            } else if summary {
                // A summary of a sequence that is not declared is E021/E028's; it is taken as
                // the first, so that what reads it still has a stage.
                let s = over.unwrap_or(0);
                let mut w = 1;
                for r in rs {
                    match each.get(r) {
                        Some(&rs_) if rs_ == s => w = w.max(level.get(r).copied().unwrap_or(1)),
                        // A value of another sequence is E029 or E031, said where it is read.
                        Some(_) => {}
                        None => w = w.max(level.get(r).copied().unwrap_or(0) + 1),
                    }
                }
                if w > summaries.max(1) {
                    // Waiting for itself. Said once, and held at the last walk there can be.
                    frozen[i] = true;
                    w = summaries.max(1);
                    let own: HashSet<String> = binds[i].iter().cloned().collect();
                    let back = rs.iter().find(|r| own.contains(*r) || depends_on(f, &reads, r, &own)).cloned().unwrap_or_default();
                    cycle.push((i, back));
                }
                Stage::Each { seq: s, walk: w }
            } else {
                let mut seqs: Vec<(usize, &String)> = Vec::new();
                for r in rs {
                    if let Some(&s) = each.get(r) {
                        if !seqs.iter().any(|(x, _)| *x == s) {
                            seqs.push((s, r));
                        }
                    }
                }
                match seqs.first() {
                    None => Stage::Once(rs.iter().map(|r| level.get(r).copied().unwrap_or(0)).max().unwrap_or(0)),
                    Some(&(s, _)) => {
                        let mut w = 1;
                        for r in rs {
                            match each.get(r) {
                                Some(&rs_) if rs_ == s => w = w.max(level.get(r).copied().unwrap_or(1)),
                                Some(_) => {}
                                None => w = w.max(level.get(r).copied().unwrap_or(0) + 1),
                            }
                        }
                        Stage::Each { seq: s, walk: w.min(summaries + 1) }
                    }
                }
            };
            if stage != of[i] {
                of[i] = stage;
                changed = true;
            }
            for b in &binds[i] {
                if owner.get(b.as_str()) != Some(&i) {
                    continue;
                }
                let (lv, sq) = match stage {
                    // What a summary answers is one value per call, there once its walk is over.
                    Stage::Each { walk, .. } if summary => (walk, None),
                    Stage::Each { seq, walk } => (walk, Some(seq)),
                    Stage::Once(l) => (l, None),
                };
                if level.get(b) != Some(&lv) {
                    level.insert(b.clone(), lv);
                    changed = true;
                }
                let was = each.get(b).copied();
                if was != sq {
                    match sq {
                        Some(s) => {
                            each.insert(b.clone(), s);
                        }
                        None => {
                            each.remove(b);
                        }
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (i, it) in f.items.iter().enumerate() {
        let mut seen: Vec<(usize, &String)> = Vec::new();
        for r in &reads[i] {
            if let Some(&s) = each.get(r) {
                if !seen.iter().any(|(x, _)| *x == s) {
                    seen.push((s, r));
                }
            }
        }
        // A fold reads its own sequence; a value of another one is a walk inside its walk. A
        // summary's column of another sequence is E029, said there.
        if let Item::Fold(d) = it {
            // A fold of a sequence that is not declared is E021's, said there.
            let Some(own) = seq_index(&d.over) else { continue };
            if let Some(&(_, other)) = seen.iter().find(|(s, _)| *s != own) {
                let first = seen.iter().find(|(s, _)| *s == own).map(|(_, r)| (*r).clone()).unwrap_or_else(|| d.verdict.clone());
                two.push((i, first, other.clone()));
            }
            continue;
        }
        if matches!(it, Item::Agg(_)) {
            continue;
        }
        if seen.len() > 1 {
            two.push((i, seen[0].1.clone(), seen[1].1.clone()));
        }
    }
    let depth = of
        .iter()
        .map(|s| match s {
            Stage::Each { walk, .. } => *walk,
            Stage::Once(l) => *l,
        })
        .max()
        .unwrap_or(0);
    let summary: Vec<bool> = f.items.iter().map(|it| matches!(it, Item::Agg(_) | Item::Fold(_))).collect();
    Stages { of, each, level, depth, two, cycle, reads, binds, summary }
}

/// The sequence a `sequence` block is a list of (§15.219): the one whose fields its header
/// writes, by its place among the rule's `elements` — all of its columns when one has them, else
/// the one with the most, so that what is wrong is said against the list it was meant for. `None`
/// for a rule that walks nothing.
pub fn sequence_list(f: &RuleFile, sq: &SeqDecl) -> Option<usize> {
    let hits = |el: &ElementsDecl| sq.cols.iter().filter(|(n, _)| el.fields.iter().any(|fd| fd.name.text == *n)).count();
    if let Some(k) = f.elements.iter().position(|el| !sq.cols.is_empty() && hits(el) == sq.cols.len()) {
        return Some(k);
    }
    let best = (0..f.elements.len()).max_by_key(|&k| (hits(&f.elements[k]), std::cmp::Reverse(k)))?;
    Some(best)
}

/// Whether `name` is worked out, through the items, from one of `from`.
fn depends_on(f: &RuleFile, reads: &[Vec<String>], name: &str, from: &HashSet<String>) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut todo = vec![name.to_string()];
    while let Some(x) = todo.pop() {
        if !seen.insert(x.clone()) {
            continue;
        }
        for (i, it) in f.items.iter().enumerate() {
            if item_binds(f, it).iter().any(|b| *b == x) {
                for r in &reads[i] {
                    if from.contains(r) {
                        return true;
                    }
                    todo.push(r.clone());
                }
            }
        }
    }
    false
}

/// The enum a declared type names, or `None` for a type of numbers, truth values, strings or
/// dates. Every other word a type is written with is the name of an enum (`resolve`).
fn enum_named(t: &TypeRef) -> Option<String> {
    use crate::kw;
    match t.base.as_str() {
        kw::MONEY | kw::MASS | kw::LENGTH | kw::AREA | kw::VOLUME | kw::DURATION | kw::TEMPERATURE | kw::SOUND
        | kw::RATE | kw::NUMBER | kw::BOOL | kw::STRING | kw::DATE => None,
        other => Some(other.to_string()),
    }
}

/// Reads a bare word where the place it is written in says what it is (§15.208): an output cell
/// of a table, and an answer of a `fold`.
///
/// The parser cannot tell an enum value from a name, so it keeps both as a name, and everything
/// downstream settled the two its own way. The type check looked the name up first, so `gold`, a
/// value of the column's enum, was read as the number output spelled the same and refused (E103);
/// the evaluator looked a fold's answer up as a name, so `empty -> none_color` gave no value at
/// all, and the generated code wrote the bare word. The type of the place decides now, once,
/// and what it decides is written back into the rule, so that the checks, the evaluator and every
/// generator read the word the same way:
///
/// - a word that is a value of the column's enum, in a column of that enum, is that value. A
///   name spelled the same whose type is another (`gold : number`) is not what the cell means,
///   and the word becomes the literal value. A name of that same enum spelled the same leaves
///   both readings open, and the author has to rename one (E075).
/// - in an answer of a `fold`, the same for the type the fold answers with — the rule's first
///   output — and `true` and `false` are the truth values in an answer of `bool`.
///
/// A word that is a value and no name stays as the parser wrote it: everything downstream reads
/// that as the value already.
pub fn read_words(f: &mut RuleFile, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let mut enums: HashMap<String, Vec<String>> = HashMap::new();
    for (p, _) in &f.imports {
        if let Some(im) = crate::prelude::lookup(p) {
            enums.insert(im.ty.to_string(), im.values());
        }
    }
    for e in &f.enums {
        enums.insert(e.name.text.clone(), e.values.iter().map(|v| v.text.clone()).collect());
    }
    // Every name the rule declares, with the enum its type names and what kind of name it is.
    let mut declared: HashMap<String, (Option<String>, String)> = HashMap::new();
    for i in &f.inputs {
        declared.insert(i.name.text.clone(), (enum_named(&i.ty), tr!("入力", "an input")));
    }
    for i in f.elements.iter().flat_map(|e| &e.fields) {
        declared.insert(i.name.text.clone(), (enum_named(&i.ty), tr!("要素のフィールド", "an element field")));
    }
    for o in &f.outputs {
        declared.insert(o.name.text.clone(), (enum_named(&o.ty), tr!("出力", "an output")));
    }
    // The type of each table's output columns, in the order written: a column with no type of its
    // own takes the output's, or what a table above gave the same name.
    let mut col_enum: Vec<Vec<Option<String>>> = Vec::new();
    for it in &f.items {
        match it {
            Item::Derived(d) => {
                declared.insert(d.name.text.clone(), (enum_named(&d.ty), tr!("導出", "a derived value")));
                col_enum.push(Vec::new());
            }
            Item::Define(d) => {
                declared.insert(d.name.text.clone(), (enum_named(&d.ty), tr!("定義", "a definition")));
                col_enum.push(Vec::new());
            }
            Item::Agg(d) => {
                declared.entry(d.name.text.clone()).or_insert((None, tr!("まとめ", "a summary")));
                col_enum.push(Vec::new());
            }
            // A fold's answer under a name of its own is declared there, with the type written
            // (§15.219).
            Item::Fold(d) => {
                if let Some(FoldAnswer { name, ty: Some(ty) }) = &d.answer {
                    declared.entry(name.text.clone()).or_insert((enum_named(ty), tr!("畳み込みの答え", "a fold's answer")));
                }
                col_enum.push(Vec::new());
            }
            Item::Table(t) => {
                let mut cols = Vec::new();
                for oc in &t.outputs {
                    let e = match &oc.ty {
                        Some(ty) => enum_named(ty),
                        None => declared.get(&oc.name.text).and_then(|(e, _)| e.clone()),
                    };
                    declared.entry(oc.name.text.clone()).or_insert((e.clone(), tr!("表の出力", "a table output")));
                    cols.push(e);
                }
                col_enum.push(cols);
            }
        }
    }
    let ambiguous = |w: &str, en: &str, kind: &str, sp: &Span, at: String| -> Diag {
        Diag::error("E075", tr!("`{w}` は、列挙 {en} の値でも、{kind}の名前でもあります", "`{w}` is both a value of enum {en} and the name of {kind}"))
            .at(at)
            .mark(sp.clone(), tr!("どちらとしても読めます", "it reads either way"))
            .note(tr!(
                "値と名前のどちらの意味か決まらないので、rulec はこのセルを読めません。どちらかの名前を変えてください。",
                "rulec cannot tell which of the two is meant here. Rename one of them."
            ))
    };
    // How the place reads a word: `Some(true)` for the value, `Some(false)` for both readings (an
    // error, already reported), `None` for a name or a word the type check will look at.
    let read = |w: &str, en: Option<&String>, boolean: bool| -> Option<bool> {
        if boolean && (w == crate::kw::TRUE || w == crate::kw::FALSE) {
            return Some(true);
        }
        let en = en?;
        if !enums.get(en).is_some_and(|vs| vs.iter().any(|v| v == w)) {
            return None;
        }
        match declared.get(w) {
            Some((Some(e), _)) if e == en => Some(false),
            _ => Some(true),
        }
    };
    // A comparison on a column of an ordered enum takes the values it covers (§15.210). The
    // cell keeps how it was written; the values are what every check reads, as it reads a set.
    // A comparison whose bound is no value of the enum, or one on an enum without an order, is
    // left as the parser wrote it, for the type check to say what is wrong with it.
    let ordered: HashMap<String, Vec<String>> =
        f.enums.iter().filter(|e| e.ordered).map(|e| (e.name.text.clone(), e.values.iter().map(|v| v.text.clone()).collect())).collect();
    let rank = |cell: &mut Cell, en: Option<&String>| {
        let Cell::Cmp(cs) = cell else { return };
        let Some(order) = en.and_then(|e| ordered.get(e)) else { return };
        let ops: Option<Vec<(CmpOp, String)>> = cs
            .iter()
            .map(|(op, l)| match l {
                Lit::Word(w) if order.contains(w) => Some((*op, w.clone())),
                _ => None,
            })
            .collect();
        let Some(ops) = ops else { return };
        let values = order.iter().filter(|v| crate::ast::ranked_holds(order, &ops, v)).cloned().collect();
        *cell = Cell::Ranked { ops, values };
    };
    for it in f.items.iter_mut() {
        let Item::Table(t) = it else { continue };
        let cols: Vec<Option<String>> = t.inputs.iter().map(|(n, _)| declared.get(n).and_then(|(e, _)| e.clone())).collect();
        for row in &mut t.rows {
            for (ci, cell) in row.cells.iter_mut().enumerate() {
                rank(cell, cols.get(ci).and_then(|e| e.as_ref()));
            }
        }
    }
    if let Some(m) = f.machine.as_mut() {
        for o in &mut m.onces {
            let en = declared.get(&o.output.text).and_then(|(e, _)| e.clone());
            rank(&mut o.cell, en.as_ref());
        }
    }
    for (k, it) in f.items.iter_mut().enumerate() {
        let Item::Table(t) = it else { continue };
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let clause = t.clause;
        for row in &mut t.rows {
            for (oi, o) in row.outs.iter_mut().enumerate() {
                let OutCell::Name(w) = o else { continue };
                let Some(en) = col_enum.get(k).and_then(|cs| cs.get(oi)).cloned().flatten() else { continue };
                match read(w, Some(&en), false) {
                    // A value no name shares stays as the parser wrote it.
                    Some(true) if declared.contains_key(w.as_str()) => {
                        *o = OutCell::Lit(Lit::Word(w.clone()));
                    }
                    Some(false) => {
                        let kind = declared.get(w.as_str()).map(|(_, k)| k.clone()).unwrap_or_default();
                        let sp = row.out_spans.get(oi).cloned().unwrap_or_else(|| row.span.clone());
                        let at = if clause {
                            tr!("{path}:{} 節 {tname}", "{path}:{} clause {tname}", sp.line)
                        } else {
                            tr!("{path}:{} 表 {tname}", "{path}:{} table {tname}", sp.line)
                        };
                        out.push(ambiguous(w, &en, &kind, &sp, at));
                    }
                    _ => {}
                }
            }
        }
    }
    // The answers of a fold, read in the type it answers with: the type written after its `->`,
    // the output it names, or the rule's first output (§15.219).
    let outputs: Vec<(String, TypeRef)> = f.outputs.iter().map(|o| (o.name.text.clone(), o.ty.clone())).collect();
    for it in f.items.iter_mut() {
        let Item::Fold(fold) = it else { continue };
        let ty: Option<TypeRef> = match &fold.answer {
            Some(FoldAnswer { ty: Some(t), .. }) => Some(t.clone()),
            Some(FoldAnswer { name, ty: None }) => outputs.iter().find(|(n, _)| *n == name.text).map(|(_, t)| t.clone()),
            None => outputs.first().map(|(_, t)| t.clone()),
        };
        let Some(ty) = ty else { continue };
        let (en, boolean) = (enum_named(&ty), ty.base == crate::kw::BOOL);
        let line = fold.span.line;
        let mut values: Vec<&mut Expr> = Vec::new();
        values.extend(fold.empty.as_mut());
        values.extend(fold.exhausted.as_mut());
        for (_, arm, _) in fold.arms.iter_mut() {
            match arm {
                Arm::Stop(Some(x)) => values.push(x),
                Arm::Take { expr, .. } => values.push(expr),
                Arm::KeepMax { expr, .. } => values.push(expr),
                _ => {}
            }
        }
        for e in values {
            let Expr::Name(w, sp) = e else { continue };
            // A truth value is one whatever the answer's type: written for an answer that is no
            // truth value, it is the wrong type (E103), not an unknown name.
            if w == crate::kw::TRUE || w == crate::kw::FALSE {
                *e = Expr::Lit(Lit::Word(w.clone()), sp.clone());
                continue;
            }
            match read(w, en.as_ref(), boolean) {
                Some(true) => *e = Expr::Lit(Lit::Word(w.clone()), sp.clone()),
                Some(false) => {
                    let kind = declared.get(w.as_str()).map(|(_, k)| k.clone()).unwrap_or_default();
                    let at = tr!("{path}:{line} 畳み込み", "{path}:{line} fold");
                    out.push(ambiguous(w, en.as_deref().unwrap_or_default(), &kind, sp, at));
                }
                None => {}
            }
        }
    }
    out
}

pub fn check(f: &RuleFile, path: &str) -> Checked {
    let mut c = Checked {
        imported: crate::prelude::imports(f),
        syms: HashMap::new(),
        roundings: HashMap::new(),
        ranges: HashMap::new(),
        derived_deps: HashMap::new(),
        define_deps: HashMap::new(),
        out_values: HashMap::new(),
        at_most: HashSet::new(),
        scales: HashMap::new(),
        grids: HashMap::new(),
        out_scales: HashMap::new(),
        enums: HashMap::new(),
        groups: HashMap::new(),
        ordered: HashSet::new(),
        labels: HashMap::new(),
        enum_ranges: HashMap::new(),
        owners: HashMap::new(),
        used: HashSet::new(),
        used_values: HashSet::new(),
        enum_order: Vec::new(),
        diags: Vec::new(),
        sets: Vec::new(),
        set_of: HashMap::new(),
        day_sets: HashMap::new(),
        arith_witness: Vec::new(),
        narrowed: HashSet::new(),
    };
    // Read the guarantees before anything is typed: `allocate` asks about them, and the
    // order of declarations in the file should not change what is accepted. Whether both
    // sides are really inputs is E018's business, below. A constraint with a sum on a side
    // (§15.216) says nothing of one name against another on its own, and is left out.
    for k in &f.constraints {
        let Some((l, r)) = k.pair() else { continue };
        let (l, r) = (l.clone(), r.clone());
        match k.op {
            CmpOp::Le | CmpOp::Lt => c.at_most.insert((l, r)),
            CmpOp::Ge | CmpOp::Gt => c.at_most.insert((r, l)),
        };
    }

    // Chains count: two lines saying a running total never passes the one after it, and
    // that one never passes the whole, together say the first never passes the whole.
    loop {
        let before = c.at_most.len();
        let step: Vec<(String, String)> = c
            .at_most
            .iter()
            .flat_map(|(a, b)| {
                c.at_most
                    .iter()
                    .filter(move |(x, _)| x == b)
                    .map(move |(_, y)| (a.clone(), y.clone()))
            })
            .collect();
        c.at_most.extend(step);
        if c.at_most.len() == before {
            break;
        }
    }

    let at = |line: usize| format!("{path}:{line}");

    // The built-in namespace: the first-level divisions of thirteen countries, their values
    // from the prelude (§2.2, §15.182). Every other spelling of a division was already written
    // as its value (`prelude::normalize`).
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for (p, sp) in &f.imports {
        match crate::prelude::lookup(p) {
            Some(im) => {
                // `std/都道府県` and `std/jp/prefectures` are the same 47 prefectures, spelled two
                // ways: both in one rule would be two enums of one generated type.
                if let Some((q, _)) = seen.iter().find(|(_, c)| *c == im.ns.country) {
                    c.diags.push(
                        Diag::error("E013", tr!("`{p}` は `{q}` と同じ区分です", "`{p}` holds the same divisions as `{q}`"))
                            .at(at(sp.line))
                            .mark(sp.clone(), "")
                            .note(tr!(
                                "一つの国の区分は、一つの名前で取り込んでください。`std/都道府県` の値は日本語の名前、`std/jp/prefectures` の値は英語の名前です。どちらから取り込んでも、両方の綴りを書けます。",
                                "Import one country's divisions under one name. `std/都道府県` spells the values in Japanese and `std/jp/prefectures` in English; either import accepts both spellings."
                            )),
                    );
                    continue;
                }
                seen.push((p.as_str(), im.ns.country));
                c.enum_order.push(im.ty.to_string());
                c.enums.insert(im.ty.to_string(), im.values());
            }
            None => c.diags.push(
                Diag::error("E013", tr!("`{p}` という取込先はありません", "There is no import named `{p}`"))
                    .at(at(sp.line))
                    .mark(sp.clone(), "")
                    .maybe_note(crate::prelude::nearest_path(p).map(|q| tr!("`{q}` のことですか。", "Did you mean `{q}`?")))
                    .note(tr!(
                        "組み込みの名前空間は、十三か国の一段目の区分です: {}。",
                        "The built-in namespaces are the first-level divisions of thirteen countries: {}.",
                        crate::prelude::paths().join(", ")
                    )),
            ),
        }
    }

    for e in &f.enums {
        c.enum_order.push(e.name.text.clone());
        c.enums.insert(
            e.name.text.clone(),
            e.values.iter().map(|v| v.text.clone()).collect(),
        );
        if e.ordered {
            c.ordered.insert(e.name.text.clone());
        }
        if e.has_labels() {
            c.labels.insert(e.name.text.clone(), (0..e.values.len()).map(|i| e.label(i).map(|s| s.to_string())).collect());
        }
        if let Some(o) = &e.from {
            c.owners.insert(e.name.text.clone(), o.clone());
        }
    }
    for g in &f.groups {
        // A member has to be a value of some enum. One that is not used to be dropped in
        // silence, so a group of 47 prefectures with one misspelling was a group of 46 and
        // the value it meant to hold fell through to whatever row caught the rest (§15.86).
        for m in &g.members {
            if !c.enums.values().any(|vs| vs.iter().any(|v| v == &m.text)) {
                c.diags.push(
                    Diag::error("E012", tr!("`{}` はどの列挙の値でもありません", "`{}` is not a value of any enum", m.text))
                        .at(at(m.span.line))
                        .mark(m.span.clone(), tr!("グループの値として書かれています", "written as a member of this group"))
                        .maybe_note(crate::prelude::hint(&m.text, &c.imported))
                        .note(tr!(
                            "グループは列挙の値の部分集合です。綴りを直すか、その値を列挙に足してください。読めない値は黙って外され、グループが一つ小さくなります。",
                            "A group is a subset of an enum's values. Correct the spelling, or add the value to the enum. A member that names nothing was dropped in silence, leaving the group one value smaller."
                        )),
                );
            }
        }
        // The group's enum is one that holds every member. Two enums may share values
        // (§15.150): the group then takes the first one declared until a column of the other
        // is the first to use it (`cell`). Members of no single enum are a mistake, and were
        // read as the first member's enum in silence.
        let holders: Vec<String> = c
            .enum_order
            .iter()
            .filter(|e| g.members.iter().all(|m| c.enums.get(*e).is_some_and(|vs| vs.contains(&m.text))))
            .cloned()
            .collect();
        let known = g.members.iter().all(|m| c.enums.values().any(|vs| vs.contains(&m.text)));
        if holders.is_empty() && known && !g.members.is_empty() {
            let of = |m: &Name| c.enum_order.iter().find(|e| c.enums.get(*e).is_some_and(|vs| vs.contains(&m.text))).cloned().unwrap_or_default();
            let first = of(&g.members[0]);
            if let Some(other) = g.members.iter().find(|m| !c.enums.get(&first).is_some_and(|vs| vs.contains(&m.text))) {
                c.diags.push(
                    Diag::error("E103", tr!("グループ {} に、列挙 {} の値と列挙 {} の値が混ざっています", "Group {} mixes values of enum {} and enum {}", g.name.text, first, of(other)))
                        .at(at(other.span.line))
                        .mark(other.span.clone(), tr!("{} の値です", "a value of {}", of(other)))
                        .note(tr!(
                            "グループは一つの列挙の値の部分集合です。列には一つの列挙の値しか来ないので、ほかの列挙の値は当たりません。",
                            "A group is a subset of one enum's values. A column holds values of one enum, so the other enum's value never matches."
                        )),
                );
            }
        }
        let owner = holders
            .first()
            .cloned()
            .or_else(|| g.members.first().and_then(|m| c.enum_order.iter().find(|e| c.enums.get(*e).is_some_and(|vs| vs.contains(&m.text))).cloned()))
            .unwrap_or_else(|| "都道府県".to_string());
        c.groups.insert(
            g.name.text.clone(),
            (owner, g.members.iter().map(|m| m.text.clone()).collect()),
        );
    }

    for i in &f.inputs {
        let ty = c.resolve(&i.ty);
        if let (Ty::Enum(en), Some(r)) = (ty.present(), &i.range) {
            let en = en.clone();
            enum_range(&mut c, i, &en, r, &at);
        } else if let Some(from) = i.range.as_ref().and_then(|r| r.days.clone()) {
            koyomi_days(&mut c, i, &ty, &from, path, &at);
        } else if let Some(r) = &i.range {
            // An optional input's range is the range its values take when there is one: the
            // bounds are read in the type it wraps. They were read in `T?` itself, where no
            // number is a value, so `number?` said E103 and `date?` dropped its bounds without a
            // word, leaving no range to prove completeness over or to guard (§15.201).
            let (b, bad) = bounds_of(r, ty.present());
            for n in bad {
                c.diags.push(
                    Diag::error("E103", tr!("範囲の `{}` は {ty} の値ではありません", "The bound `{}` is not a value of {ty}", n.raw))
                        .at(at(i.span.line))
                        .mark(i.name.span.clone(), tr!("この宣言の範囲", "the range on this declaration"))
                        .maybe_note(unit_note(&n))
                        .note(tr!(
                            "範囲は、完全性の証明が対象にする値の全体で、生成コードの入口のガードでもあります。読めない境界を黙って落とすと、片側の無い範囲のまま「完全」と判定してしまいます。",
                            "The range is the universe the completeness proof quantifies over, and the entry guard of the generated code. Dropping a bound it cannot read would answer \"complete\" for a range with one side missing."
                        )),
                );
            }
            c.ranges.insert(i.name.text.clone(), b);
        } else if matches!(ty.present(), Ty::Rate) {
            // A rate input that declares no range is 0%..100%. The overflow proof always
            // assumed as much; recording it here is what makes the entry guard, the
            // inventory and the vectors say the same thing (§15.34). A rate that can exceed
            // 100% declares its range like any other number.
            c.ranges.insert(i.name.text.clone(), (Some(Rat::zero()), Some(Rat::int(1))));
        }
        if *ty.present() == Ty::Date
            && let Some(given) = crate::days::held_over(&i.name.text)
        {
            // `checked_over` (ritsu's port): the input takes only the days given, inside what the
            // rule declares for it.
            let (lo, hi) = c.ranges.get(&i.name.text).copied().unwrap_or((None, None));
            let held = Days { from: None, file: String::new(), sha256: String::new(), days: given };
            let days: Vec<i64> = held.days.iter().copied().filter(|d| held_one(*d, lo, hi)).collect();
            let held = Days { days, ..held };
            if let (Some(first), Some(last)) = (held.days.first(), held.days.last()) {
                c.ranges.insert(i.name.text.clone(), (Some(Rat::int(*first as i128)), Some(Rat::int(*last as i128))));
            }
            c.day_sets.insert(i.name.text.clone(), held);
        }
        if let Some(given) = crate::over::held(&i.name.text) {
            // `outputs_over` (ritsu's port): the input takes only the values given, inside what
            // the rule declares for it; a date of koyomi's keeps the days among them.
            let declared = c.ranges.get(&i.name.text).copied().unwrap_or((None, None));
            let (lo, hi) = crate::over::meet(declared, given);
            c.ranges.insert(i.name.text.clone(), (lo, hi));
            if let Some(d) = c.day_sets.get_mut(&i.name.text) {
                d.days.retain(|x| held_one(*x, lo, hi));
            }
        }
        if let Some(n) = unreadable_step(&i.ty, ty.present()) {
            c.diags.push(
                Diag::error("E103", tr!("刻み `{}` は {ty} の値ではありません", "The step `{}` is not a value of {ty}", n.raw))
                    .at(at(i.name.span.line))
                    .mark(i.ty.span.clone(), tr!("この型が宣言している刻みです", "this is the step the type declares"))
                    .maybe_note(unit_note(&n))
                    .note(tr!(
                        "刻みは型と同じ単位で書いてください（`{}` なら `rate[step 0.1%]`）。読めない刻みは 1 として扱われ、実行時の値が単位の整数になり、宣言した範囲も生成コードの入口ガードもその目盛りで読まれます。",
                        "Write the step in the unit of the type (`{}` takes `rate[step 0.1%]`). A step that cannot be read was taken as 1, so the runtime value counted whole units and both the declared range and the generated guard were read on that scale.",
                        ty
                    )),
            );
        }
        if let Some(d) = missing_step(&i.name, &i.ty, ty.present(), i.name.span.line, path) {
            c.diags.push(d);
        }
        c.scales.insert(i.name.text.clone(), scale_of_type(&i.ty, ty.present()));
        c.syms.insert(
            i.name.text.clone(),
            Sym { ty, span: i.name.span.clone(), kind: SymKind::Input, contract_only: i.contract_only },
        );
    }
    // `range from koyomi` is a range an input takes; anywhere else it says nothing a check could
    // read (§15.174).
    let misplaced = f
        .elements
        .iter()
        .flat_map(|el| el.fields.iter())
        .filter_map(|o| o.range.as_ref().and_then(|r| r.days.as_ref().map(|d| (o.name.text.clone(), d.span.clone()))))
        .chain(f.items.iter().filter_map(|it| match it {
            Item::Derived(d) => d.range.as_ref().and_then(|r| r.days.as_ref().map(|x| (d.name.text.clone(), x.span.clone()))),
            _ => None,
        }))
        .collect::<Vec<_>>();
    for (name, span) in misplaced {
        c.diags.push(
            Diag::error("E065", tr!("`range from koyomi` を書けるのは入力だけです", "Only an input takes `range from koyomi`"))
                .at(at(span.line))
                .mark(span, tr!("{name} は入力ではありません", "{name} is not an input"))
                .note(tr!(
                    "koyomi の日付がとりうる日は、呼び出し側が渡す日付の範囲です。出力、導出、並びの要素の範囲は、規則が自分で確かめる主張なので、別のファイルから借りられません。",
                    "The days a koyomi date comes to are the range of a date the caller passes. The range of an output, a derive or an element's field is a claim the rule checks itself, and cannot be borrowed from another file."
                )),
        );
    }
    // The fields of one element (§15.56). They are declared like inputs and a table may use
    // them as columns; what differs is that the caller passes them once per element, so the
    // completeness proof quantifies over one element exactly as it quantifies over one call.
    // A rule may walk more than one sequence (§15.219); each field is one sequence's.
    for el in &f.elements {
        for i in &el.fields {
            let ty = c.resolve(&i.ty);
            if let (Ty::Enum(en), Some(r)) = (ty.present(), &i.range) {
                // The range of an enum narrows what an input takes at the door (§15.210); an
                // element passes no door of its own that could hold it, and its bounds used to be
                // dropped in silence.
                c.diags.push(
                    Diag::error("E069", tr!("要素のフィールド {} の列挙に範囲は書けません", "Element field {} cannot hold an enum to a range", i.name.text))
                        .at(at(r.span.line))
                        .mark(r.span.clone(), tr!("{en} の範囲", "a range of {en}"))
                        .note(tr!(
                            "列挙の範囲を書けるのは入力だけです。要素の値を絞るなら、表の行で `-` 以外の値を書いて、残りを E101 に挙げさせてください。",
                            "Only an input holds an enum to a range. To narrow what an element takes, write its values in the rows and let E101 name the rest."
                        )),
                );
            } else if let Some(r) = &i.range {
                // The element's fields are declared like inputs, and the bound that cannot be
                // read is the same error there — but the diagnostics were dropped on the
                // floor here, so `額 : money[円] range >=0円 <=1000g` passed with no upper
                // bound at all, over which completeness was then proved (§15.86).
                let (b, bad) = bounds_of(r, ty.present());
                for n in bad {
                    c.diags.push(
                        Diag::error("E103", tr!("範囲の `{}` は {ty} の値ではありません", "The bound `{}` is not a value of {ty}", n.raw))
                            .at(at(i.span.line))
                            .mark(i.name.span.clone(), tr!("この宣言の範囲", "the range on this declaration"))
                            .maybe_note(unit_note(&n))
                            .note(tr!(
                                "範囲は、完全性の証明が対象にする値の全体で、生成コードの入口のガードでもあります。読めない境界を黙って落とすと、片側の無い範囲のまま「完全」と判定してしまいます。",
                                "The range is the universe the completeness proof quantifies over, and the entry guard of the generated code. Dropping a bound it cannot read would answer \"complete\" for a range with one side missing."
                            )),
                    );
                }
                c.ranges.insert(i.name.text.clone(), b);
            } else if matches!(ty.present(), Ty::Rate) {
                c.ranges.insert(i.name.text.clone(), (Some(Rat::zero()), Some(Rat::int(1))));
            }
            if let Some(n) = unreadable_step(&i.ty, ty.present()) {
                c.diags.push(
                    Diag::error("E103", tr!("刻み `{}` は {ty} の値ではありません", "The step `{}` is not a value of {ty}", n.raw))
                        .at(at(i.name.span.line))
                        .mark(i.ty.span.clone(), tr!("この型が宣言している刻みです", "this is the step the type declares"))
                        .maybe_note(unit_note(&n))
                        .note(tr!(
                            "刻みは型と同じ単位で書いてください（`{}` なら `rate[step 0.1%]`）。読めない刻みは 1 として扱われ、実行時の値が単位の整数になり、宣言した範囲も生成コードの入口ガードもその目盛りで読まれます。",
                            "Write the step in the unit of the type (`{}` takes `rate[step 0.1%]`). A step that cannot be read was taken as 1, so the runtime value counted whole units and both the declared range and the generated guard were read on that scale.",
                            ty
                        )),
                );
            }
            if let Some(d) = missing_step(&i.name, &i.ty, ty.present(), i.name.span.line, path) {
                c.diags.push(d);
            }
            c.scales.insert(i.name.text.clone(), scale_of_type(&i.ty, ty.present()));
            c.syms.insert(
                i.name.text.clone(),
                Sym { ty, span: i.name.span.clone(), kind: SymKind::Element, contract_only: i.contract_only },
            );
        }
    }
    // A sequence is there to be walked (§15.219): the caller passes every element of it, and a
    // sequence no `count`, `sum`, `max`, `min` or `fold` walks is one nothing reads — its fields
    // would not even be held to their ranges, since an element is held to them as it is walked.
    // A summary whose `over` names no sequence is said where it is (E021, E028): the sequence it
    // meant is then not said to be walked by nothing as well.
    let misnamed = f.items.iter().any(|it| match it {
        Item::Agg(d) => f.sequence_named(&d.over).is_none(),
        Item::Fold(d) => f.sequence_named(&d.over).is_none(),
        _ => false,
    });
    for el in &f.elements {
        let walked = f.items.iter().any(|it| match it {
            Item::Agg(d) => d.over == el.name.text,
            Item::Fold(d) => d.over == el.name.text,
            _ => false,
        });
        if !walked && !misnamed && f.kind == FileKind::Rule {
            c.diags.push(
                Diag::error("E020", tr!("並び {} をたどるものがありません", "Nothing walks the sequence {}", el.name.text))
                    .at(at(el.span.line))
                    .mark(el.name.span.clone(), tr!("`over {}` と書いた行がありません", "no line says `over {}`", el.name.text))
                    .note(tr!(
                        "並びは、`count`・`sum`・`max`・`min`・`fold` のどれかでたどって、はじめて規則の値になります。たどる行を足すか、この `elements` を消してください。",
                        "A sequence becomes a value of the rule only when a `count`, a `sum`, a `max`, a `min` or a `fold` walks it. Add the line that walks it, or take this `elements` out."
                    )),
            );
        }
    }
    // A sequence and each field of an element are one name each, beside the inputs: a field
    // spelled as an input, or as a field of another sequence, would be read as whichever was
    // declared last, and a walk would read the wrong value in silence (§15.219).
    {
        let mut seen: HashMap<&str, &Span> = HashMap::new();
        for i in &f.inputs {
            seen.insert(i.name.text.as_str(), &i.name.span);
        }
        let names = f.elements.iter().flat_map(|el| std::iter::once(&el.name).chain(el.fields.iter().map(|fd| &fd.name)));
        for n in names {
            match seen.get(n.text.as_str()) {
                Some(first) => c.diags.push(
                    Diag::error("E034", tr!("名前 `{}` が二度宣言されています", "The name `{}` is declared twice", n.text))
                        .at(at(n.span.line))
                        .mark((*first).clone(), tr!("最初の宣言", "the first declaration"))
                        .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                        .note(tr!(
                            "入力、たどる並び、要素のフィールドは、規則の中でそれぞれ一つの値を指します。同じ名前が二つあると、表や式がどちらを読むかが決まりません。どちらかの名前を変えてください。",
                            "An input, a sequence and a field of an element each name one value in the rule. With two of one name, a table or an expression cannot say which it reads. Change one of the two."
                        )),
                ),
                None => {
                    seen.insert(n.text.as_str(), &n.span);
                }
            }
        }
    }

    // A file of enums declares no function, so it has nothing to return (§15.212).
    if f.outputs.is_empty() && f.kind == FileKind::Rule {
        c.diags.push(
            Diag::error("E009", tr!("出力がありません", "No outputs are declared"))
                .at(at(f.name.span.line))
                .mark(f.name.span.clone(), "")
                .note(tr!("規則は値をひとつ以上返します。`{}` の節を書いてください。", "A rule returns at least one value. Write an `{}` section.", crate::kw::OUTPUTS)),
        );
    }
    for o in &f.outputs {
        let ty = c.resolve(&o.ty);
        // §7.2: a numeric output must declare its rounding.
        if ty.is_numeric() && o.rounding.is_none() {
            c.diags.push(
                Diag::error("E104", tr!("出力に丸めの宣言がありません", "The output declares no rounding"))
                    .at(at(o.span.line))
                    .mark(o.ty.span.clone(), tr!("丸めの宣言がありません", "no rounding is declared"))
                    .fix(
                        crate::diag::FixKind::AddRounding,
                        format!("{} {}({})", crate::kw::ROUND, crate::kw::DOWN, unit_one(&ty)),
                    )
                    .note(tr!("出力 {} は {} です。", "Output {} has type {}.", o.name.text, ty)),
            );
        }
        if let Some(rd) = &o.rounding {
            // The grid is a value of the column it rounds. It was read with `lit_value_in`
            // and, when that said no, simply not recorded — so `round up(10銭)` and
            // `round up(10)` on a `money[円]` output both fell back on a grid of one yen, ten
            // times away from either of the two things the author could have meant, with
            // nothing said (§15.86).
            match (RoundMode::parse(&rd.mode), lit_value_in(&rd.grid, &ty)) {
                (Some(m), Some(g)) => {
                    c.roundings.insert(o.name.text.clone(), (m, g));
                }
                (Some(_), None) if ty != Ty::Unknown => c.diags.push(
                    Diag::error("E103", tr!("丸めの刻み `{}` は {ty} の値ではありません", "The rounding grid `{}` is not a value of {ty}", rd.grid.raw))
                        .at(at(rd.span.line))
                        .mark(rd.span.clone(), tr!("この列を丸める刻みです", "this is the grid this column is rounded to"))
                        .maybe_note(unit_note(&rd.grid))
                        .note({
                            // The English example is written in the column's own unit (`1JPY`, `1g`),
                            // as the fix of E104 is, so it is not a yen amount on a column of grams.
                            let one = unit_one(&ty);
                            tr!(
                                "刻みは丸める列と同じ単位で書いてください（`{}` なら `round down(1円)`）。",
                                "Write the grid in the unit of the column it rounds (`{}` takes `round down({one})`).",
                                ty
                            )
                        }),
                ),
                _ => {}
            }
        }
        c.syms.insert(
            o.name.text.clone(),
            Sym { ty, span: o.name.span.clone(), kind: SymKind::Output, contract_only: false },
        );
    }

    // E060: a step or a rounding grid of zero or less. `round up(0円)` passed the check and the
    // generated code divided by it at run time — Python stopped with ZeroDivisionError — and
    // `rate[step 0%]` was read as a step of one (§15.156). Every type a declaration writes is
    // looked at, the table headers' too.
    {
        let mut typed: Vec<&TypeRef> = Vec::new();
        typed.extend(f.inputs.iter().map(|i| &i.ty));
        typed.extend(f.outputs.iter().map(|o| &o.ty));
        for el in &f.elements {
            typed.extend(el.fields.iter().map(|i| &i.ty));
        }
        for it in &f.items {
            match it {
                Item::Derived(d) => typed.push(&d.ty),
                Item::Define(d) => typed.push(&d.ty),
                Item::Table(t) => typed.extend(t.outputs.iter().filter_map(|oc| oc.ty.as_ref())),
                Item::Agg(_) => {}
                Item::Fold(d) => typed.extend(d.answer.iter().filter_map(|a| a.ty.as_ref())),
            }
        }
        for t in typed {
            let ty = c.resolve(t);
            // E103: a money type whose tax is neither `incl_tax` nor `excl_tax`. It passed in
            // silence, with the word taken as a tax of its own (§15.169).
            if let Some(w) = unreadable_tax(t) {
                c.diags.push(
                    Diag::error("E103", tr!("税区分 `{w}` は incl_tax でも excl_tax でもありません", "The tax `{w}` is neither incl_tax nor excl_tax"))
                        .at(at(t.span.line))
                        .mark(t.span.clone(), tr!("この型が宣言している税区分です", "this is the tax the type declares"))
                        .note(tr!(
                            "お金の型の二つ目には、税込なら `incl_tax`、税抜なら `excl_tax` を書いてください。ほかの語では、税込か税抜かが決まりません。",
                            "The second word of a money type is `incl_tax` for an amount with tax and `excl_tax` for one without. Any other word leaves it unsaid which the amount is."
                        )),
                );
            }
            for a in &t.args {
                if let TypeArg::Scaled(w, n) = a
                    && w == crate::kw::STEP
                    && lit_value_in(n, ty.present()).is_some_and(|v| v.num <= 0)
                {
                    c.diags.push(nonpositive(n, t.span.clone(), at(t.span.line), tr!("型の刻み", "the step of a type")));
                }
            }
        }
        for o in &f.outputs {
            if let Some(rd) = &o.rounding {
                let ty = c.resolve(&o.ty);
                if lit_value_in(&rd.grid, &ty).is_some_and(|v| v.num <= 0) {
                    c.diags.push(nonpositive(&rd.grid, rd.span.clone(), at(rd.span.line), tr!("丸めの刻み", "the rounding grid")));
                }
            }
        }
        // E061: a range with no value in it. The completeness proof answered "complete" over
        // the empty set, and the generated code's entry guard refused every call (§15.156).
        let mut ranged: Vec<(&crate::ast::Range, Ty)> = Vec::new();
        for i in f.inputs.iter().chain(f.elements.iter().flat_map(|el| el.fields.iter())) {
            if let Some(r) = &i.range {
                // An optional input's range is read in the type it wraps (§15.201).
                ranged.push((r, c.resolve(&i.ty).present().clone()));
            }
        }
        for it in &f.items {
            match it {
                Item::Derived(d) => {
                    if let Some(r) = &d.range {
                        ranged.push((r, c.resolve(&d.ty)));
                    }
                }
                Item::Agg(d) => {
                    if let Some(r) = &d.range {
                        ranged.push((r, Ty::Unknown));
                    }
                }
                _ => {}
            }
        }
        for (r, ty) in ranged {
            if let Some(d) = empty_range(r, &ty, at(r.span.line)) {
                c.diags.push(d);
            }
        }
    }

    // §1.1 fixes each keyword to a single spelling. When a name collides with a keyword,
    // the line-oriented parser reads the declaration as the start of a section and silently
    // drops it. Silent dropping is the worst outcome, so it is the name that gets rejected.
    const KEYWORDS: &[&str] = crate::kw::RESERVED;
    let mut named: Vec<(&str, &Span)> = Vec::new();
    named.push((f.name.text.as_str(), &f.name.span));
    for s in &f.sources {
        named.push((s.name.text.as_str(), &s.name.span));
    }
    for i in &f.inputs {
        named.push((i.name.text.as_str(), &i.name.span));
    }
    for o in &f.outputs {
        named.push((o.name.text.as_str(), &o.name.span));
    }
    // A value is held to fewer words than a name: it never starts a line (§15.150).
    let mut values: Vec<(&str, &Span)> = Vec::new();
    for e in &f.enums {
        named.push((e.name.text.as_str(), &e.name.span));
        for v in &e.values {
            values.push((v.text.as_str(), &v.span));
        }
    }
    for g in &f.groups {
        named.push((g.name.text.as_str(), &g.name.span));
    }
    for it in &f.items {
        match it {
            Item::Derived(d) => named.push((d.name.text.as_str(), &d.name.span)),
            Item::Define(d) => named.push((d.name.text.as_str(), &d.name.span)),
            Item::Agg(d) => named.push((d.name.text.as_str(), &d.name.span)),
            Item::Fold(d) => {
                if let Some(a) = &d.answer {
                    named.push((a.name.text.as_str(), &a.name.span));
                }
            }
            Item::Table(t) => {
                if let Some(n) = &t.name {
                    named.push((n.text.as_str(), &n.span));
                }
                for oc in &t.outputs {
                    named.push((oc.name.text.as_str(), &oc.name.span));
                }
                for row in &t.rows {
                    if let Some(l) = &row.label {
                        named.push((l.text.as_str(), &l.span));
                    }
                }
            }
        }
    }
    for (n, sp) in named {
        if KEYWORDS.contains(&n) {
            c.diags.push(
                Diag::error("E009", tr!("`{n}` はキーワードなので、名前にできません", "`{n}` is a keyword and cannot be used as a name"))
                    .at(at(sp.line))
                    .mark(sp.clone(), "")
                    .note(tr!("rulec は規則を行ごとに読むので、キーワードと同じ名前を付けると、その宣言が黙って捨てられてしまいます。", "The syntax is line-oriented, so a declaration named like a keyword is silently dropped."))
                    .note(tr!("別の名前を付けてください。", "Choose a different name.")),
            );
        }
    }
    for (n, sp) in values {
        if crate::kw::VALUE_RESERVED.contains(&n) {
            c.diags.push(
                Diag::error("E009", tr!("`{n}` はキーワードなので、列挙の値にできません", "`{n}` is a keyword and cannot be an enum value"))
                    .at(at(sp.line))
                    .mark(sp.clone(), "")
                    .note(tr!(
                        "値はセル・`enum` の行・`machine` と `fold` の行に書かれ、この語はそこで別の意味に読まれます。値にできないキーワードは {} の {} 語だけです。",
                        "A value is written in cells, in its `enum` line and in the lines under `machine` and `fold`, where this word is read as something else. These {1} are the only keywords a value cannot be: {0}.",
                        crate::kw::VALUE_RESERVED.join(" "),
                        crate::kw::VALUE_RESERVED.len()
                    ))
                    .note(tr!("別の名前を付けてください。", "Choose a different name.")),
            );
        }
    }

    // §1.3: the public face needs ASCII aliases; the inside does not. A name that is
    // already ASCII needs none either — the reason for the alias is that a kanji has no
    // uppercase and so cannot begin an exported Go identifier, which is not a problem
    // `shipping_fee` has. Demanding `rule shipping_fee(shipping_fee)` was pure ceremony.
    let mut missing: Vec<String> = Vec::new();
    if needs_alias(&f.name) {
        missing.push(tr!("規則 {}", "rule {}", f.name.text));
    }
    for i in &f.inputs {
        if needs_alias(&i.name) {
            missing.push(tr!("入力 {}", "input {}", i.name.text));
        }
    }
    for o in &f.outputs {
        if needs_alias(&o.name) {
            missing.push(tr!("出力 {}", "output {}", o.name.text));
        }
    }
    if !missing.is_empty() {
        c.diags.push(
            Diag::error("E011", tr!("公開される名前に ASCII の別名がありません", "A public name has no ASCII alias"))
                .at(at(f.name.span.line))
                .fix_kind(crate::diag::FixKind::AddAlias)
                .mark(f.name.span.clone(), "")
                .note(tr!("不足: {}", "Missing: {}", missing.join(" / ")))
                .note(tr!("Go の公開識別子は先頭が大文字でなければならず、漢字とかなには大文字がありません。名前がもとから ASCII なら、別名は要りません。", "An exported Go identifier must start with an uppercase letter, and kanji and kana have no uppercase. A name that is already ASCII needs no alias."))
                .note(tr!("宣言の位置に丸括弧で書いてください。例: 届け先(dest)", "Write it in parentheses at the declaration, e.g. 届け先(dest)")),
        );
    }

    // W121: an alias that the target language will not take, or will take only by hiding
    // something of its own (§15.103). Every ASCII alias becomes an identifier somewhere —
    // a function, a parameter, a local, a type, a member — so the words each backend
    // refuses are the words an alias cannot be. Measured: a rule with an input aliased
    // `type` generated Rust that does not compile, and one aliased `sum` generated Python
    // whose module shadows the builtin without saying so.
    {
        // The comparison ignores case, because an alias reaches some targets in PascalCase
        // and others in UPPER_CASE. `top` is true for the two aliases that become an
        // identifier at the outermost level of the generated file: the rule's, which names
        // the function, and an enum's, which names a type. Everything else lands as a
        // parameter, a local or a member, where a name of its own shadows nothing outside
        // the body it is in.
        // (the alias, where, what it names, whether it is at the outermost level, whether the
        // generator writes it with `_` after it where a language keeps it as a keyword)
        let mut aliases: Vec<(&str, &Span, String, bool, bool)> = Vec::new();
        fn take<'n>(
            n: &'n Name,
            what: String,
            top: bool,
            renamed: bool,
            v: &mut Vec<(&'n str, &'n Span, String, bool, bool)>,
        ) {
            if let Some(a) = n.ascii.as_deref() {
                // An alias is the author's identifier, and stays as written (§15.210): W121 says
                // where a language does not take it.
                v.push((a, &n.span, what, top, false));
            } else if renamed && n.text.is_ascii() && !crate::codegen::keyword_in(&n.text).is_empty() {
                // A value's name with no alias is its own identifier — a parameter, a local, a
                // field — and one a language keeps as a keyword is written with `_` after it
                // there (§15.210, `codegen::escape_keywords`): said as an alias would be.
                v.push((n.text.as_str(), &n.span, what, top, renamed));
            }
        }
        // The rule's name and an enum's are types and functions, not renamed; an enum's values
        // become members in each language's own case, and a table's name is a string in the
        // trace, so neither is renamed either.
        take(&f.name, tr!("規則", "rule"), true, false, &mut aliases);
        for i in &f.inputs {
            take(&i.name, tr!("入力", "input"), false, true, &mut aliases);
        }
        for o in &f.outputs {
            take(&o.name, tr!("出力", "output"), false, true, &mut aliases);
        }
        for e in &f.enums {
            take(&e.name, tr!("列挙", "enum"), true, false, &mut aliases);
            for v in &e.values {
                take(v, tr!("列挙の値", "enum value"), false, false, &mut aliases);
            }
        }
        for el in &f.elements {
            take(&el.name, tr!("並び", "sequence"), false, true, &mut aliases);
            for fd in &el.fields {
                take(&fd.name, tr!("要素のフィールド", "element field"), false, true, &mut aliases);
            }
        }
        for it in &f.items {
            match it {
                Item::Derived(d) => take(&d.name, tr!("導出", "derived value"), false, true, &mut aliases),
                Item::Define(d) => take(&d.name, tr!("定義", "definition"), false, true, &mut aliases),
                Item::Agg(d) => take(&d.name, tr!("まとめ", "summary"), false, true, &mut aliases),
                // An answer that is no output's is a local, like a derived value (§15.219).
                Item::Fold(d) => {
                    if let Some(a) = d.answer.as_ref().filter(|a| !f.outputs.iter().any(|o| o.name.text == a.name.text)) {
                        take(&a.name, tr!("畳み込みの答え", "fold's answer"), false, true, &mut aliases);
                    }
                }
                Item::Table(t) => {
                    if let Some(n) = &t.name {
                        take(n, tr!("表", "table"), false, false, &mut aliases);
                    }
                    for oc in &t.outputs {
                        take(&oc.name, tr!("表の出力", "table output"), false, true, &mut aliases);
                    }
                }
            }
        }
        let rule_alias = f.name.ascii.as_deref();
        for (alias, sp, what, top, renames) in aliases {
            let mut kw: Vec<&str> = Vec::new();
            let mut hides: Vec<&str> = Vec::new();
            // The rule's alias also names a module or a package, which the standard library
            // may already have (§15.149): the generated `time.py` is what `import time` finds.
            let module = top && std::ptr::eq(sp, &f.name.span) && rule_alias == Some(alias);
            let mut collides: Vec<&str> = Vec::new();
            for b in crate::backend::ALL {
                if b.reserved.iter().any(|w| w.eq_ignore_ascii_case(alias)) {
                    kw.push(b.name);
                } else if top && b.globals.iter().any(|w| w.eq_ignore_ascii_case(alias)) {
                    hides.push(b.name);
                } else if module && b.modules.iter().any(|w| w.eq_ignore_ascii_case(&(b.module_of)(alias))) {
                    collides.push(b.name);
                }
            }
            if kw.is_empty() && hides.is_empty() && collides.is_empty() {
                continue;
            }
            // A parameter, a local or a field is written with `_` after it where a language that
            // writes names as they are keeps it (§15.210): it compiles, under another name.
            let renamed = if renames { crate::codegen::keyword_in(alias) } else { Vec::new() };
            if !renamed.is_empty() {
                c.diags.push(
                    Diag::warning(
                        "W121",
                        tr!("`{alias}` は生成先の予約語なので、生成コードでは `{alias}_` と書きます", "`{alias}` is a keyword in a target language, so the generated code writes it as `{alias}_`"),
                    )
                    .at(at(sp.line))
                    .mark(sp.clone(), tr!("{what}の名前", "the {what}'s name"))
                    .note(tr!(
                        "{}: そこでは予約語です。生成コードは、どの言語でもこの名前を `{alias}_` と書き、`rulec api` もその名前を挙げます。受け渡しする値の名前（入力の JSON のキーや記録）は、宣言した名前のままです。",
                        "{}: it is a keyword there. Every language's generated code writes this name as `{alias}_`, and `rulec api` names it so; the name a value travels under (the keys of the input JSON, the records) stays the one declared.",
                        renamed.join(", ")
                    ))
                    .note(tr!(
                        "生成コードでの名前を自分で決めるなら、別名を付けてください（`{alias}(<別名>)`）。",
                        "To choose the name the generated code uses yourself, give it an alias (`{alias}(<alias>)`)."
                    )),
                );
                continue;
            }
            let mut d = Diag::warning(
                "W121",
                tr!("別名 `{alias}` は生成先の言葉とぶつかります", "The alias `{alias}` collides with a word in a target language"),
            )
            .at(at(sp.line))
            .mark(sp.clone(), tr!("{what}の別名", "the {what}'s alias"));
            if !kw.is_empty() {
                d = d.note(tr!(
                    "{}: そこでは予約語です。生成コードはその名前をそのまま書くので、コンパイルできません。",
                    "{}: it is a keyword there, so generated code that writes the name as it is does not compile.",
                    kw.join(", ")
                ));
            }
            if !hides.is_empty() {
                d = d.note(tr!(
                    "{}: その名前はすでに使われています。この別名はファイルの一番外側に出るので、それを隠してしまいます。",
                    "{}: the name is already taken there, and this alias, which reaches the top level of the file, hides it.",
                    hides.join(", ")
                ));
            }
            if !collides.is_empty() {
                d = d.note(tr!(
                    "{}: 言語の側に同じ名前のモジュールかクラスがあり、この規則のために生成するモジュールとぶつかります。生成したものがそちらの代わりに読まれるか、二つを同時には定義できないか、`import` がどちらを指すか決まらなくなります。",
                    "{}: the language already has a module or a class of that name, and the one generated for this rule collides with it: the generated one is read in its place, the two cannot both be defined, or an import cannot tell which is meant.",
                    collides.join(", ")
                ));
            }
            c.diags.push(d.note(tr!(
                "使わない生成先なら、このままで構いません。使うなら別名を変えてください（公開名はそのままで構いません）。",
                "For a target you do not generate, leave it. For one you do, change the alias; the public name can stay as it is."
            )));
        }
    }

    // The `.proto` of the rule as a Connect service takes four names of its own, and they are
    // not in any language's keyword list (§15.112). An output aliased `trace` would be a
    // second field of that name in the answer, and an enum aliased `fired` would be a second
    // message called `Fired` in the package; protoc refuses either, which is late and about a
    // file the writer of the rule never asked for.
    {
        let taken = |a: &str, names: &[&str]| names.iter().any(|w| w.eq_ignore_ascii_case(a));
        let mut hit: Vec<(&Name, String)> = Vec::new();
        for o in &f.outputs {
            if o.name.ascii.as_deref().is_some_and(|a| taken(a, &["trace"])) {
                hit.push((&o.name, tr!("答えには、出力と並んで `trace` が入ります", "the answer carries `trace` beside the outputs")));
            }
        }
        // A rule that walks more than one sequence names each element's message after its
        // sequence (`<sequence>_element`, §15.219).
        let elements: Vec<String> = if f.elements.len() > 1 {
            f.elements.iter().map(|el| format!("{}_element", el.name.ascii.clone().unwrap_or_else(|| el.name.text.clone()))).collect()
        } else {
            Vec::new()
        };
        for e in &f.enums {
            let mut names: Vec<&str> = vec!["fired", "element", "decide_request", "decide_response"];
            names.extend(elements.iter().map(|s| s.as_str()));
            if e.name.ascii.as_deref().is_some_and(|a| taken(a, &names)) {
                hit.push((&e.name, tr!("同じ名前のメッセージが既にあります", "a message of that name is already there")));
            }
        }
        for (n, why) in hit {
            c.diags.push(
                Diag::warning(
                    "W121",
                    tr!(
                        "別名 `{}` は生成先の言葉とぶつかります",
                        "The alias `{}` collides with a word in a target language",
                        n.ascii.as_deref().unwrap_or("")
                    ),
                )
                .at(at(n.span.line))
                .mark(n.span.clone(), tr!("別名", "the alias"))
                .note(tr!(
                    "Connect: {why}。`.proto` がその名前を二度書くので、protoc がエラーにします。",
                    "Connect: {why}, so the `.proto` would write that name twice and protoc refuses it."
                ))
                .note(tr!(
                    "使わない生成先なら、このままで構いません。使うなら別名を変えてください（公開名はそのままで構いません）。",
                    "For a target you do not generate, leave it. For one you do, change the alias; the public name can stay as it is."
                )),
            );
        }
    }

    // A table's or a clause's name is what the trace, an `overrides` line and a later version
    // refer to, so two cannot share one (E034, the same rule as for row labels).
    {
        let mut seen: HashMap<&str, &Span> = HashMap::new();
        for it in &f.items {
            let Item::Table(t) = it else { continue };
            let Some(n) = &t.name else { continue };
            match seen.get(n.text.as_str()) {
                Some(first) => c.diags.push(
                    Diag::error("E034", tr!("表か節の名前 `{}` が二度あります", "The table or clause name `{}` appears twice", n.text))
                        .at(at(n.span.line))
                        .mark((*first).clone(), tr!("最初の宣言", "the first declaration"))
                        .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                        .note(tr!(
                            "表と節は同じ名前空間に入ります。記録の trace と `{}` の行は、その名前で表や節を指します。どちらかの名前を変えてください。",
                            "Tables and clauses share one namespace: the trace and `{}` lines refer to them by name. Change one of the two.",
                            crate::kw::OVERRIDES
                        )),
                ),
                None => {
                    seen.insert(&n.text, &n.span);
                }
            }
        }
    }

    // Two sources cannot share a name either: a citation names one of them (E034).
    {
        let mut seen: HashMap<&str, &Span> = HashMap::new();
        for s in &f.sources {
            match seen.get(s.name.text.as_str()) {
                Some(first) => c.diags.push(
                    Diag::error("E034", tr!("出典の名前 `{}` が二度あります", "The source name `{}` appears twice", s.name.text))
                        .at(at(s.name.span.line))
                        .mark((*first).clone(), tr!("最初の宣言", "the first declaration"))
                        .mark(s.name.span.clone(), tr!("同じ名前", "the same name"))
                        .note(tr!("引用 `@名前` は、この名前で出典を指します。どちらかの名前を変えてください。", "A citation `@name` refers to the source by this name. Change one of the two.")),
                ),
                None => {
                    seen.insert(&s.name.text, &s.name.span);
                }
            }
        }
    }

    // §5.1: items are a define-before-use pipeline, so a single forward pass is enough.
    for it in &f.items {
        match it {
            Item::Derived(d) => {
                let ty = c.resolve(&d.ty);
                let got = c.expr_ty(&d.expr, path);
                c.literal_values(&d.expr, path);
                c.check_same(&ty, &got, &d.span, path, &tr!("導出", "derived value"));
                c.derived_range(d, &ty, path);
                c.overflow(&d.expr, &ty, &d.span, path, &d.name.text);
                c.record_value(&d.name.text, &d.expr, &ty);
                let mut deps = Vec::new();
                collect_names(&d.expr, &mut deps);
                c.derived_deps.insert(d.name.text.clone(), deps);
                c.syms.insert(
                    d.name.text.clone(),
                    Sym { ty, span: d.name.span.clone(), kind: SymKind::Derived, contract_only: false },
                );
            }
            Item::Define(d) => {
                let ty = c.resolve(&d.ty);
                let got = c.expr_ty(&d.expr, path);
                c.literal_values(&d.expr, path);
                c.check_same(&ty, &got, &d.span, path, &tr!("定義", "definition"));
                c.overflow(&d.expr, &ty, &d.span, path, &d.name.text);
                c.record_value(&d.name.text, &d.expr, &ty);
                c.syms.insert(
                    d.name.text.clone(),
                    Sym { ty, span: d.name.span.clone(), kind: SymKind::Define, contract_only: false },
                );
            }
            Item::Table(t) => c.table(t, path),
            Item::Agg(d) => c.agg_decl(d, f, path),
            // A fold is checked where it is written, so that what it reads is declared above
            // it and its answer is declared from there on (§15.219).
            Item::Fold(d) => c.fold_decl(d, f, path),
        }
    }

    // After the items, so that a column naming a table's output or a `define` resolves.
    for ex in &f.examples {
        c.example_cells(ex, f, path);
    }
    for sc in &f.scenarios {
        let name = sc.name.text.clone();
        let at = move |line: usize| tr!("{path}:{line} 手順の例 {}", "{path}:{line} scenario {}", name);
        c.value_cells(&sc.table, f, &at);
    }

    // §5.3 E113: the atoms of a boolean definition are limited to unary tests on an input or
    // a derived value. Relaxing this lets half-spaces like `x − y >= c` into the columns, and
    // the box algebra of §6.2 falls apart.
    for it in &f.items {
        if let Item::Define(d) = it {
            if c.resolve(&d.ty) == Ty::Bool {
                c.atoms(&d.expr, &d.name.text, path);
                let mut deps = Vec::new();
                collect_names(&d.expr, &mut deps);
                c.define_deps.insert(d.name.text.clone(), deps);
            }
        }
    }

    // An example of a walk has to say which sequences it walks. A row of cells has no room for
    // one, so the cell names a `sequence` block instead (§15.56). Every sequence a summary or a
    // fold walks needs its column (§15.219).
    {
        let walked: Vec<&ElementsDecl> = f
            .elements
            .iter()
            .filter(|el| {
                f.items.iter().any(|it| match it {
                    Item::Agg(d) => d.over == el.name.text,
                    Item::Fold(d) => d.over == el.name.text,
                    _ => false,
                })
            })
            .collect();
        for ex in &f.examples {
            for el in &walked {
                if ex.inputs.iter().any(|(n, _)| n == &el.name.text) {
                    continue;
                }
                let sp = ex.rows.first().map(|r| r.span.clone()).unwrap_or_else(|| ex.span.clone());
                c.diags.push(
                    Diag::error("E025", tr!("例に並びの列がありません", "The examples have no column for the sequence"))
                        .at(tr!("{path}:{} 例", "{path}:{} examples", sp.line))
                        .mark(sp, tr!("{} の列がありません", "no column for {}", el.name.text))
                        .note(tr!(
                            "この規則は並びをたどるので、例にはどの並びをたどるのかも書く必要があります。`sequence <名前>` で並びを書き、見出しに `{}` の列を足して、その名前をセルに書いてください。",
                            "This rule walks a sequence, so an example is only a case once it says which sequence. Write the list with `sequence <name>`, add a `{}` column to the header, and name it in the cell.",
                            el.name.text
                        )),
                );
            }
        }
    }

    // The three stages of a walk (§15.208): what each item, each output and each answer of the
    // fold may read, given when it runs.
    if !f.elements.is_empty() && f.walks() {
        c.walk_stages(f, path);
    }

    // `sequence` (§15.56): one named list of elements, which an example names in a cell.
    // Everything in it is a value — this is not a table, so nothing here narrows a range.
    {
        let mut seen: Vec<String> = Vec::new();
        for sq in &f.sequences {
            let at_seq = tr!("{path}:{} 並び {}", "{path}:{} sequence {}", sq.span.line, sq.name.text);
            // A block is a list of the sequence whose fields it writes (§15.219).
            let Some(el) = sequence_list(f, sq).map(|k| &f.elements[k]) else {
                c.diags.push(
                    Diag::error("E026", tr!("たどる並びのない規則に `sequence` は書けません", "A `sequence` needs a rule that walks one"))
                        .at(at_seq.clone())
                        .mark(sq.span.clone(), tr!("`elements` がありません", "there is no `elements`"))
                        .note(tr!(
                            "`sequence` は `elements` で宣言したフィールドの並びです。たどる並びが無い規則では、書いても使われません。",
                            "A `sequence` is a list of the fields `elements` declares. With no sequence to walk there is nothing for it to be a list of."
                        )),
                );
                continue;
            };
            if seen.contains(&sq.name.text) {
                c.diags.push(
                    Diag::error("E026", tr!("`{}` という名前の `sequence` が二つあります", "There are two sequences called `{}`", sq.name.text))
                        .at(at_seq.clone())
                        .mark(sq.name.span.clone(), tr!("二本目です", "this is a second one"))
                        .note(tr!("例はこの名前で並びを指すので、同じ名前が二つあるとどちらか決まりません。", "An example names a sequence by this name, so two of them leave it undecided.")),
                );
                continue;
            }
            seen.push(sq.name.text.clone());
            let fields: Vec<&str> = el.fields.iter().map(|fd| fd.name.text.as_str()).collect();
            for (n, sp) in &sq.cols {
                if !fields.contains(&n.as_str()) {
                    c.diags.push(
                        Diag::error("E026", tr!("`{n}` は要素のフィールドではありません", "`{n}` is not a field of an element"))
                            .at(at_seq.clone())
                            .mark(sp.clone(), "")
                            .note(tr!(
                                "書けるのは {} のフィールドだけです: {}。",
                                "Only the fields of {} can be written here: {}.",
                                el.name.text,
                                fields.join(" / ")
                            )),
                    );
                }
            }
            for fd in &el.fields {
                if !sq.cols.iter().any(|(n, _)| n == &fd.name.text) {
                    c.diags.push(
                        Diag::error("E026", tr!("要素のフィールド `{}` が書かれていません", "The element's field `{}` is not written", fd.name.text))
                            .at(at_seq.clone())
                            .mark(sq.span.clone(), tr!("`{}` の列がありません", "no column for `{}`", fd.name.text))
                            .note(tr!(
                                "要素には、フィールドを全部書いてください。フィールドを落とすと、その値を誰も決めていないことになります。",
                                "One element is one element only when all of its fields are there. A field left out is a value nobody decided."
                            )),
                    );
                }
            }
            // Every cell is a literal of its field's type: a range or a `-` would be a pattern,
            // and this is a value.
            for row in &sq.rows {
                for (ci, cell) in row.cells.iter().enumerate() {
                    let Some((col, _)) = sq.cols.get(ci) else { continue };
                    let Some(sym) = c.syms.get(col).cloned() else { continue };
                    let sp = row.cell_spans.get(ci).unwrap_or(&row.span).clone();
                    if !matches!(cell, Cell::Lit(_) | Cell::Nothing) {
                        c.diags.push(
                            Diag::error("E026", tr!("`sequence` のセルは値だけです", "A cell of a sequence is a value and nothing else"))
                                .at(at_seq.clone())
                                .mark(sp.clone(), tr!("`{col}` に値が書かれていません", "`{col}` does not hold a value"))
                                .note(tr!(
                                    "範囲や `-` は表のセルの書き方です。ここには実際に渡す一件を書くので、値を書いてください。",
                                    "A range or a `-` is how a table's cell is written. This is one element as it would really be passed, so write the value."
                                )),
                        );
                        continue;
                    }
                    let sc = *c.scales.get(col).unwrap_or(&1);
                    c.cell(cell, &sym.ty, sc, &sp, &at_seq, false);
                }
            }
        }
    }

    // The cells that name one. A name with no block behind it is the one mistake this shape
    // makes possible, and it is caught here rather than at the first run.
    {
        let mut named: Vec<String> = Vec::new();
        let columns = f.examples.iter().flat_map(|ex| {
            f.elements.iter().enumerate().filter_map(move |(k, el)| ex.inputs.iter().position(|(n, _)| n == &el.name.text).map(|ci| (ex, k, ci)))
        });
        for (ex, k, ci) in columns {
            for row in &ex.rows {
                let sp = row.cell_spans.get(ci).unwrap_or(&row.span).clone();
                let at_ex = tr!("{path}:{} 例", "{path}:{} examples", row.span.line);
                match row.cells.get(ci) {
                    Some(Cell::Lit(Lit::Word(w))) => {
                        let found = f.sequences.iter().find(|sq| &sq.name.text == w);
                        let list = found.and_then(|sq| sequence_list(f, sq));
                        if let (Some(_), Some(l)) = (found, list.filter(|l| *l != k)) {
                            // A list of another sequence's elements (§15.219).
                            c.diags.push(
                                Diag::error("E027", tr!("`{w}` は {} の並びです", "`{w}` is a list of {}", f.elements[l].name.text))
                                    .at(at_ex)
                                    .mark(sp, tr!("この列は {} です", "this column is {}", f.elements[k].name.text))
                                    .note(tr!(
                                        "この列には、{} のフィールドを見出しにした `sequence` の名前を書いてください。",
                                        "Name a `sequence` whose header is the fields of {} here.",
                                        f.elements[k].name.text
                                    )),
                            );
                            named.push(w.clone());
                        } else if found.is_some() {
                            named.push(w.clone());
                        } else {
                            let mut d = Diag::error("E027", tr!("`{w}` という `sequence` はありません", "There is no sequence called `{w}`"))
                                .at(at_ex)
                                .mark(sp, "");
                            if !f.sequences.is_empty() {
                                d = d.note(tr!(
                                    "書いてあるのは {} です。",
                                    "The ones that are written are {}.",
                                    f.sequences.iter().map(|sq| sq.name.text.clone()).collect::<Vec<_>>().join(" / ")
                                ));
                            }
                            c.diags.push(d.note(tr!(
                                "`sequence {w}` で並びを書いてください。行がゼロ本なら、要素ゼロ件の例になります。",
                                "Write the list with `sequence {w}`. With no rows it is the example for a sequence with nothing in it."
                            )));
                        }
                    }
                    _ => c.diags.push(
                        Diag::error("E027", tr!("この列には `sequence` の名前を書いてください", "This column holds the name of a sequence"))
                            .at(at_ex)
                            .mark(sp, tr!("名前ではありません", "this is not a name"))
                            .note(tr!(
                                "例がたどる並びは `sequence <名前>` で書き、ここにはその名前だけを書いてください。",
                                "The sequence a case walks is written with `sequence <name>`, and this cell holds that name and nothing else."
                            )),
                    ),
                }
            }
        }
        for sq in &f.sequences {
            if !named.contains(&sq.name.text) {
                c.diags.push(
                    Diag::warning("W116", tr!("どの例も使っていない `sequence` です: {}", "No example uses this sequence: {}", sq.name.text))
                        .at(tr!("{path}:{} 並び {}", "{path}:{} sequence {}", sq.span.line, sq.name.text))
                        .mark(sq.name.span.clone(), tr!("使っている例がありません", "no example names it"))
                        .note(tr!(
                            "並びは、例が使ったときにだけ走ります。例を足すか、この並びを消してください。",
                            "A sequence runs only when an example names it. Add the example, or drop the sequence."
                        )),
                );
            }
        }
    }

    // `constraint` (§15.55). It computes nothing: it says which combinations of inputs exist,
    // so the region checks do not demand rows for the ones that do not, and the entry guard
    // refuses them. Both sides have to be inputs of the same comparable type — a relation
    // between a value and something the caller does not pass cannot be checked at the door.
    for k in &f.constraints {
        let mut syms = Vec::new();
        for name in k.names() {
            match c.syms.get(name).cloned() {
                Some(sym) if sym.kind == SymKind::Input => syms.push(Some(sym)),
                found => {
                    let what = match found {
                        None => tr!("{name} という入力はありません", "there is no input called {name}"),
                        Some(_) => tr!("{name} は入力ではありません", "{name} is not an input"),
                    };
                    c.diags.push(
                        Diag::error("E018", tr!("`constraint` は入力どうしの関係です", "A `constraint` relates two inputs"))
                            .at(tr!("{path}:{} 制約", "{path}:{} constraint", k.span.line))
                            .mark(k.span.clone(), what)
                            .note(tr!(
                                "両側とも `inputs` に宣言した名前にしてください。導出や定義は入力から計算されるので、関係は、その元になった入力どうしで書いてください。",
                                "Name something declared in `inputs` on both sides. A derived or defined value is computed from the inputs, so write the relation between those inputs instead."
                            )),
                    );
                    syms.push(None);
                }
            }
        }
        let Some(syms) = syms.into_iter().collect::<Option<Vec<Sym>>>() else {
            continue;
        };
        // Numbers and dates are what the region checker can narrow by arithmetic. An enum or a
        // boolean has no order, so `<=` between two of them would be a shape with no meaning.
        let ordered = |t: &Ty| matches!(t, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date);
        if syms.iter().any(|s| !ordered(&s.ty)) {
            let (l, r) = (Constraint::side_text(&k.left), Constraint::side_text(&k.right));
            c.diags.push(
                Diag::error("E018", tr!("`constraint` で比べられるのは、大小のある型どうしだけです", "A `constraint` compares two ordered types"))
                    .at(tr!("{path}:{} 制約", "{path}:{} constraint", k.span.line))
                    .mark(k.span.clone(), tr!("{l} と {r} は比べられません", "{l} and {r} cannot be compared"))
                    .note(tr!(
                        "比べられるのは、金額・数量・率・number・日付です。列挙や真偽に大小はありません。",
                        "Money, a quantity, a rate, a number and a date can be compared. An enum and a boolean have no order."
                    )),
            );
            continue;
        }
        // A side that adds inputs up adds their values (§15.216), so each has to have arithmetic:
        // a date, a temperature and a sound level are ordered and are not added (E048).
        let added = [&k.left, &k.right]
            .into_iter()
            .filter(|side| side.len() > 1)
            .flat_map(|side| side.iter())
            .filter_map(|n| c.syms.get(n).map(|s| s.ty.clone()))
            .find(|t| *t == Ty::Date || compares_only(t));
        if let Some(ty) = added {
            c.no_arithmetic(&ty, &k.span, path);
            continue;
        }
        let first = syms[0].ty.clone();
        for s in &syms[1..] {
            c.check_same(&first, &s.ty, &k.span, path, &tr!("制約", "constraint"));
        }
        // The door compares the stored integers, and those count steps: a rate in 1% steps and
        // one in 0.1% steps hold 50% as 50 and as 500. The guard brings both to the step they
        // share before comparing (§15.139), which is a multiplication, so its product has to
        // fit in int64 like any other value the generated code computes (E108). A sum is added
        // up on that step too, so the sum, not only each of its parts, has to fit.
        let scale_of = |n: &String| c.scales.get(n).copied().unwrap_or(1).max(1);
        let common = k.names().map(scale_of).fold(1, lcm_i128);
        let abs = |r: Rat| if r.num < 0 { Rat::zero().sub(r) } else { r };
        let mag_of = |n: &String| -> Option<Rat> {
            let (Some(lo), Some(hi)) = c.ranges.get(n).cloned()? else { return None };
            Some(if abs(lo).cmp_to(abs(hi)).is_gt() { abs(lo) } else { abs(hi) })
        };
        let differ = k.names().any(|n| scale_of(n) != common);
        for side in [&k.left, &k.right] {
            if side.len() == 1 && !differ {
                continue;
            }
            let Some(mag) = side.iter().map(mag_of).try_fold(Rat::zero(), |acc, m| m.map(|m| acc.add(m))) else { continue };
            let stored = mag.mul(Rat::int(common));
            if stored.num / stored.den > i64::MAX as i128 {
                let said = k.said();
                let shown = Constraint::side_text(side);
                let (mark, note) = if side.len() > 1 {
                    (
                        tr!("和は、共通の刻み 1/{common} で数えて足します", "the sum is added up on the common step of 1/{common}"),
                        tr!(
                            "{shown} は最大で {} になり、刻み 1/{common} で数えると int64 を超えます。足す入力の範囲を狭めてください。",
                            "{shown} reaches {}, which counted on the step of 1/{common} exceeds int64. Narrow the ranges of the inputs it adds.",
                            fmt_val(mag, &first)
                        ),
                    )
                } else {
                    (
                        tr!("刻みの違う二つは、共通の刻み 1/{common} にそろえて比べます", "the two sides are compared on their common step of 1/{common}"),
                        tr!(
                            "{shown} は最大で {} になり、共通の刻みで数えると int64 を超えます。範囲を狭めるか、二つの刻みをそろえてください。",
                            "{shown} reaches {}, which on the common step exceeds int64. Narrow the range, or give the two the same step.",
                            fmt_val(mag, &first)
                        ),
                    )
                };
                c.diags.push(
                    Diag::error("E108", tr!("制約 `{said}` の比較が int64 に収まることを証明できません", "Cannot prove that the comparison in the constraint `{said}` fits in int64"))
                        .at(tr!("{path}:{} 制約", "{path}:{} constraint", k.span.line))
                        .mark(k.span.clone(), mark)
                        .note(note),
                );
            }
        }
        // An input that only ever appears in a constraint is still doing work: it narrows the
        // space the completeness proof walks. W111 is about a declaration nothing reads.
        for n in k.names() {
            c.used.insert(n.clone());
        }
    }

    if let Some(r) = &f.result {
        c.used.insert(r.name.clone());
        let got = c.expr_ty(&r.expr, path);
        c.literal_values(&r.expr, path);
        if let Some(s) = c.syms.get(&r.name).cloned() {
            c.check_same(&s.ty, &got, &r.span, path, &tr!("結果", "result"));
            // E108 holds for `result` too. The proof used to be stated over the two `Item`s
            // alone, `define` and `derive`, and `result` is neither — so a product assembled
            // straight into the first output was never held to int64. The corpus never
            // stepped on it: its widest proven interval is seven orders of magnitude below
            // i64::MAX, so nothing there can overflow whether it is proved or not (§15.9).
            c.overflow(&r.expr, &s.ty, &r.span, path, &r.name);
        }
        // E015: `result` is sugar for the first output and nothing else — the evaluator and
        // the generated code both apply it there. Naming a later output used to type-check
        // against that output while the value went to the first one, so a `number` landed in
        // a `money` slot without a word (§15.16).
        if let Some(first) = f.outputs.first() {
            if first.name.text != r.name {
                c.diags.push(
                    Diag::error(
                        "E015",
                        tr!(
                            "`result` が書けるのは最初の出力 {} だけです",
                            "`result` can only assemble the first output, {}",
                            first.name.text
                        ),
                    )
                    .at(tr!("{path}:{} 結果", "{path}:{} result", r.span.line))
                    .mark(r.span.clone(), tr!("{} は最初の出力ではありません", "{} is not the first output", r.name))
                    .note(tr!(
                        "二つ目以降の出力は、同じ名前の `define` で書いてください: `define {}(…) : … = …`。",
                        "The second and later outputs are taken from a `define` of the same name: `define {}(…) : … = …`.",
                        r.name
                    ))
                    .note(tr!(
                        "この出力を `result` で組み立てたいなら、`outputs` の先頭へ移してください。",
                        "To assemble this output with `result`, move it to the top of `outputs`."
                    )),
                );
            }
        }
    }

    // `machine` and `scenario` (§15.148): do the names fit what they are said to be.
    c.machine_decl(f, path);

    // §11 W111: declarations nothing names. `contract_only` silences a range-guard-only input.
    for i in &f.inputs {
        if !c.used.contains(&i.name.text) && !i.contract_only {
            c.diags.push(
                Diag::warning("W111", tr!("入力 {} はどの表でも使われていません", "Input {} is not used by any table", i.name.text))
                    .at(at(i.span.line))
                    .fix(crate::diag::FixKind::MarkContractOnly, crate::kw::CONTRACT_ONLY)
                    .mark(i.name.span.clone(), tr!("どの列にも現れません", "appears in no column"))
                    .note(tr!("本来使うべき列の書き忘れかもしれません。", "A column that should use it may have been left out."))
                    .note(tr!("範囲の入口検査としてだけ効かせるつもりなら、宣言に `{}` を付けてください（W111）。", "If it is meant only as an entry check on its range, add `{}` to the declaration (W111).", crate::kw::CONTRACT_ONLY)),
            );
        }
    }
    // The fields of an element are declarations too, and an unused one costs more than an
    // unused input: the caller fills it once per element (§15.56).
    for i in f.elements.iter().flat_map(|e| &e.fields) {
        if !c.used.contains(&i.name.text) && !i.contract_only {
            c.diags.push(
                Diag::warning("W111", tr!("要素のフィールド {} はどの表でも使われていません", "Element field {} is not used by any table", i.name.text))
                    .at(at(i.span.line))
                    .fix(crate::diag::FixKind::MarkContractOnly, crate::kw::CONTRACT_ONLY)
                    .mark(i.name.span.clone(), tr!("どの列にも現れません", "appears in no column"))
                    .note(tr!(
                        "呼び出し側は要素ごとにこのフィールドを埋めることになります。使わないなら消してください。",
                        "The caller fills this field for every element. If nothing uses it, take it out."
                    )),
            );
        }
    }
    // A declaration named like an output is read: the output answers with its value (§15.208).
    let answers = |n: &str| f.outputs.iter().any(|o| o.name.text == n);
    for it in &f.items {
        if let Item::Agg(d) = it {
            if !c.used.contains(&d.name.text) && !answers(&d.name.text) {
                let (title, note) = match d.kind {
                    AggKind::Count => (
                        tr!("数え上げ {} はどこでも使われていません", "Count {} is never used", d.name.text),
                        tr!(
                            "数えた結果を使わないなら、並びをたどる意味がありません。表の列に置くか、消してください。",
                            "A count nothing reads is a walk for nothing. Put it in a column, or remove it."
                        ),
                    ),
                    AggKind::Sum => (
                        tr!("合計 {} はどこでも使われていません", "Sum {} is never used", d.name.text),
                        tr!(
                            "合計を使わないなら、並びをたどる意味がありません。表の列か式に置くか、消してください。",
                            "A sum nothing reads is a walk for nothing. Put it in a column or an expression, or remove it."
                        ),
                    ),
                    AggKind::Max | AggKind::Min => (
                        if d.kind == AggKind::Max {
                            tr!("最大 {} はどこでも使われていません", "Max {} is never used", d.name.text)
                        } else {
                            tr!("最小 {} はどこでも使われていません", "Min {} is never used", d.name.text)
                        },
                        tr!(
                            "求めた値を使わないなら、並びをたどる意味がありません。表の列か式に置くか、消してください。",
                            "A value nothing reads is a walk for nothing. Put it in a column or an expression, or remove it."
                        ),
                    ),
                };
                c.diags.push(
                    Diag::warning("W111", title)
                        .at(at(d.span.line))
                        .mark(d.name.span.clone(), tr!("どの列にも式にも現れません", "appears in no column and no expression"))
                        .note(note),
                );
            }
        }
        // A fold's answer under a name of its own, which nothing reads (§15.219).
        if let Item::Fold(d) = it {
            if let Some(a) = d.answer.as_ref().filter(|a| !answers(&a.name.text) && !c.used.contains(&a.name.text)) {
                c.diags.push(
                    Diag::warning("W111", tr!("畳み込みの答え {} はどこでも使われていません", "The fold's answer {} is never used", a.name.text))
                        .at(at(a.name.span.line))
                        .mark(a.name.span.clone(), tr!("どの列にも式にも現れません", "appears in no column and no expression"))
                        .note(tr!(
                            "答えを使わないなら、並びをたどる意味がありません。表の列か式に置くか、出力の名前を付けるか、消してください。",
                            "A fold whose answer nothing reads is a walk for nothing. Put it in a column or an expression, give it an output's name, or remove it."
                        )),
                );
            }
        }
        if let Item::Derived(d) = it {
            if !c.used.contains(&d.name.text) && !answers(&d.name.text) {
                c.diags.push(
                    Diag::warning("W111", tr!("導出 {} はどこでも使われていません", "Derived value {} is never used", d.name.text))
                        .at(at(d.span.line))
                        .mark(d.name.span.clone(), tr!("どの列にも式にも現れません", "appears in no column and no expression"))
                        .note(tr!("使わない導出は、検査の軸を一本増やすだけです。消すか、使ってください。", "An unused derived value only adds one more axis to the checks. Remove it, or use it.")),
                );
            }
        }
    }
    // Only types declared in this file are checked. For an imported type (the 47 prefectures)
    // it is normal that a cell like `not: 沖縄` leaves 45 values unnamed; demanding a mark on
    // each of them would wreck the whole warning channel.
    for e in &f.enums {
        let unnamed: Vec<&Name> = e
            .values
            .iter()
            .enumerate()
            .filter(|(i, v)| {
                !c.used_values.contains(&(e.name.text.clone(), v.text.clone())) && !e.default_marks.get(*i).copied().unwrap_or(false)
            })
            .map(|(_, v)| v)
            .collect();
        // A value of an ordered enum that the `range` of every input of the enum keeps out never
        // arrives: when every name of the enum's type is an input whose range leaves the value out,
        // no call brings it, and no row has to name it (§15.210, §15.233). One name of the type
        // without such a range — an input with none, a field of an element, an output, a value the
        // rule computes — brings every value back.
        let uses: Vec<&String> = c
            .syms
            .iter()
            .filter(|(_, s)| match &s.ty {
                Ty::Enum(n) => *n == e.name.text,
                Ty::Opt(t) => matches!(t.as_ref(), Ty::Enum(n) if *n == e.name.text),
                _ => false,
            })
            .map(|(n, _)| n)
            .collect();
        let kept_out = |v: &str| {
            !uses.is_empty()
                && uses.iter().all(|n| c.syms[*n].kind == SymKind::Input && c.enum_ranges.get(*n).is_some_and(|vs| !vs.iter().any(|x| x == v)))
        };
        let prereq_only = unnamed.len() == e.values.len() && f.prerequisites.iter().any(|p| p.over_name() == e.name.text);
        let unused: Vec<&Name> = unnamed.into_iter().filter(|v| !kept_out(&v.text)).collect();
        // An enum bound to a `.proto` is reported by the pass that read the file (E033): the
        // question there is not "did you forget a line" but "has anyone read the change the
        // contract shipped", and it is only a fair question once the file resolved (§15.59).
        let bound = f.enum_imports.iter().any(|p| p.target.text == e.name.text);
        // An enum read from an `enums` file is the vocabulary several rules share, and a rule
        // names the values it decides on, as one names the prefectures it decides on (§15.212).
        // Nor does a file of enums name any: it has no rows (§15.212).
        // An enum no row names, which a `prerequisites` block is over, is that block's: every value
        // of it is named there (§15.222). (`prereq_only` is worked out above, before the values
        // the ranges keep out are taken away.)
        if !unused.is_empty() && !bound && e.from.is_none() && f.kind == FileKind::Rule && !prereq_only {
            let names: Vec<String> = unused.iter().map(|v| v.text.clone()).collect();
            c.diags.push(
                Diag::warning("W111", tr!("型 {} の値がどの行にも現れません", "Values of type {} appear in no row", e.name.text))
                    .at(at(e.span.line))
                    .fix(crate::diag::FixKind::MarkDefault, crate::kw::DEFAULT)
                    .mark(e.name.span.clone(), "")
                    .note(tr!("現れない値: {}", "Values that never appear: {}", names.join(" / ")))
                    .note(tr!("完全性の検査は通っていても、その値は `-` の行に当たっているだけかもしれません。", "Even though the completeness check passes, the rows for those values may simply be absorbed by a `-`.")),
            );
        }
    }

    // A table's output column may introduce a name of its own. Nothing downstream reading it
    // means the column computes a value that never leaves the table — and Go will not compile
    // a local that is never read, so this has to be said here rather than discovered there.
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        // A table an `ordered by` reads leaves through that line (§15.222).
        let ordering = t.name.as_ref().is_some_and(|tn| f.prerequisites.iter().any(|p| p.ordered_by.as_ref().is_some_and(|(n, _)| n.text == tn.text)));
        for oc in &t.outputs {
            let n = &oc.name.text;
            if ordering || c.used.contains(n) || f.outputs.iter().any(|o| o.name.text == *n) {
                continue;
            }
            c.diags.push(
                Diag::warning("W111", tr!("表の列 {n} はどこでも使われていません", "Table column {n} is never used"))
                    .at(at(oc.name.span.line))
                    .mark(oc.name.span.clone(), tr!("この列の値は表から出ていきません", "this column's value never leaves the table"))
                    .note(tr!("規則の出力にするか、後ろの表か `result` で使うか、消してください。", "Make it an output of the rule, use it in a later table or in `result`, or remove it.")),
            );
        }
    }
    for g in &f.groups {
        // A group that came with its enum from an `enums` file is the file's to declare, and is
        // used by whichever rule needs it (§15.212).
        if !c.used.contains(&g.name.text) && g.from.is_none() && f.kind == FileKind::Rule {
            c.diags.push(
                Diag::warning("W111", tr!("グループ {} はどのセルでも使われていません", "Group {} is not used in any cell", g.name.text))
                    .at(at(g.span.line))
                    .mark(g.name.span.clone(), ""),
            );
        }
    }
    // The definition sets, once every table is typed (§15.66).
    let (sets, set_of, ds) = crate::defset::build(f, path);
    c.sets = sets;
    c.set_of = set_of;
    c.diags.extend(ds);
    c.output_steps(f, path);
    c
}

impl Checked {
    /// The step each rate output travels at (§15.144): the one its type declares, or with none
    /// declared its rounding grid. It used to be taken from what the rows write, so a row
    /// that names a finer value changed what every other row's answer counts in, and a
    /// `result` fell back on whole percents and cut 12.3% to 12. The value inside the code
    /// is held at a scale this divides, and brought down to it after the rounding.
    fn output_steps(&mut self, f: &RuleFile, path: &str) {
        for o in &f.outputs {
            if !matches!(self.syms.get(&o.name.text).map(|s| &s.ty), Some(Ty::Rate)) {
                continue;
            }
            let grid = self.roundings.get(&o.name.text).map(|(_, g)| *g);
            let wire = if has_step(&o.ty) {
                scale_of_type(&o.ty, &Ty::Rate)
            } else if let Some(g) = grid {
                g.den
            } else {
                continue;
            };
            // The answer sits on the rounding grid; the caller counts the declared step. A grid
            // that is not a whole number of steps leaves answers the step cannot write.
            if let (true, Some(g), Some(rd)) = (has_step(&o.ty), grid, &o.rounding) {
                if !g.mul(Rat::int(wire)).is_int() {
                    let step = fmt_val(Rat::new(1, wire), &Ty::Rate);
                    self.diags.push(
                        Diag::error(
                            "E114",
                            tr!(
                                "丸めの刻み `{}` が、出力の刻み {step} に載っていません",
                                "The rounding grid `{}` does not sit on the output's step of {step}",
                                rd.grid.raw
                            ),
                        )
                        .at(tr!("{path}:{} 出力 {}", "{path}:{} output {}", rd.span.line, o.name.text))
                        .mark(rd.span.clone(), tr!("刻みは {step} です", "the step is {step}"))
                        .note(tr!(
                            "答えは丸めの刻みに載りますが、呼び出し側へは宣言した刻みの整数で渡すので、表せない答えが出ます。",
                            "Every answer sits on the rounding grid, and it is handed to the caller as a whole number of the declared step, so some answers could not be written."
                        ))
                        .note(tr!(
                            "刻みを丸めに合わせるか（`rate[step {}]`）、丸めの刻みを {step} の整数倍にしてください。",
                            "Declare the step the rounding needs (`rate[step {}]`), or round to a whole number of {step}.",
                            rd.grid.raw
                        )),
                    );
                }
            }
            self.out_scales.insert(o.name.text.clone(), wire);
            let sc = self.scales.get(&o.name.text).copied().unwrap_or(1);
            self.scales.insert(o.name.text.clone(), lcm_i128(sc, wire));
        }
    }
}

impl Checked {
    fn resolve(&mut self, t: &TypeRef) -> Ty {
        let base = match t.base.as_str() {
            crate::kw::MONEY => {
                let mut it = t.args.iter().filter_map(|a| match a {
                    TypeArg::Word(w) => Some(w.clone()),
                    _ => None,
                });
                Ty::Money { cur: it.next().unwrap_or_else(|| "円".into()), tax: it.next() }
            }
            crate::kw::MASS
            | crate::kw::LENGTH
            | crate::kw::AREA
            | crate::kw::VOLUME
            | crate::kw::DURATION
            | crate::kw::TEMPERATURE
            | crate::kw::SOUND => {
                let unit = t
                    .args
                    .iter()
                    .find_map(|a| match a {
                        TypeArg::Word(w) => Some(w.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                Ty::Qty { dim: t.base.clone(), unit }
            }
            crate::kw::RATE => Ty::Rate,
            crate::kw::NUMBER => Ty::Number,
            crate::kw::BOOL => Ty::Bool,
            crate::kw::STRING => Ty::Str,
            crate::kw::DATE => Ty::Date,
            other => Ty::Enum(other.to_string()),
        };
        if t.optional { Ty::Opt(Box::new(base)) } else { base }
    }

    fn check_same(&mut self, want: &Ty, got: &Ty, span: &Span, path: &str, what: &str) {
        // §2.1: a rate and a number are both dimensionless and share one runtime form — an
        // integer with a scale. Which of the two a ratio is called is for the declaration to
        // say: `達成率 : rate = 合計点 ÷ 満点` writes its thresholds as percentages, while
        // `基本点 : number = 税込金額 ÷ 100円` writes them as counts. Only a declaration gets
        // this latitude; `unifies` stays strict, so a rate still cannot be added to a count.
        let both_dimensionless =
            matches!((want, got), (Ty::Rate, Ty::Number) | (Ty::Number, Ty::Rate));
        if !want.unifies(got) && !both_dimensionless && *got != Ty::Unknown {
            self.diags.push(
                Diag::error("E103", tr!("型が合いません: {want} に {got} を入れています", "Type mismatch: {want} is given {got}"))
                    .at(format!("{path}:{} {what}", span.line))
                    .mark(span.clone(), ""),
            );
        }
    }

    /// The number literals of an expression, each held to the unit it is written in (§15.191).
    /// A literal in an expression is read in its own unit, as the reference evaluator and every
    /// generator read it, and one with no value there came out as no answer from the evaluator
    /// and as 0 in the generated code, with nothing said: a bare decimal (`0.4` is a `number`,
    /// and a `number` is whole) and a fraction of a unit (`0.5GBP`). The language writes a
    /// rate with `%` and an amount with its unit, so a bare decimal is answered with the rate it
    /// would be. A divisor is E115's: it has to be a whole constant anyway.
    fn literal_values(&mut self, e: &Expr, path: &str) {
        match e {
            Expr::Lit(Lit::Num(n), sp) => {
                let ty = lit_ty(n);
                if ty == Ty::Unknown || lit_value_in(n, &ty).is_some() {
                    return;
                }
                let raw = &n.raw;
                let d = if n.unit.is_none() {
                    let pct = magnitude(n).map(|v| format!("{}%", v.mul(Rat::int(100))));
                    let d = Diag::error("E103", tr!("単位の無い小数 `{raw}` は書けません", "A bare decimal `{raw}` cannot be written"))
                        .at(format!("{path}:{}", sp.line))
                        .mark(sp.clone(), pct.as_ref().map(|p| tr!("率なら `{p}`", "as a rate, `{p}`")).unwrap_or_default())
                        .note(tr!(
                            "単位の無い数は `number` で、`number` は整数です。`{raw}` には値が無く、この式は答えを出せません。",
                            "A number with no unit is a `number`, and a `number` is whole: `{raw}` has no value, and the expression has no answer."
                        ))
                        .note(match &pct {
                            Some(p) => tr!(
                                "率を掛けるなら `%` で書いてください（`{raw}` は `{p}`）。金額や量なら、単位を付けてください。",
                                "To multiply by a rate, write it with `%` (`{raw}` is `{p}`). An amount or a quantity takes its unit."
                            ),
                            None => tr!("率なら `%` で、金額や量なら単位を付けて書いてください。", "Write a rate with `%`, and an amount or a quantity with its unit."),
                        });
                    match pct {
                        Some(p) => d.fix(crate::diag::FixKind::RewriteLiteral, p),
                        None => d,
                    }
                } else {
                    Diag::error("E103", tr!("`{raw}` は {ty} の値ではありません", "`{raw}` is not a value of {ty}"))
                        .at(format!("{path}:{}", sp.line))
                        .mark(sp.clone(), "")
                        .note(tr!(
                            "金額と量は、その単位の整数です。細かい値が要るなら、細かい単位の型で書いてください（`mass[g]` の `1500g` など）。",
                            "Money and quantities are whole numbers of their unit. A finer value needs a finer unit in the types (`1500g` in `mass[g]`)."
                        ))
                };
                self.diags.push(d);
            }
            Expr::Bin(l, op, r, _) => {
                self.literal_values(l, path);
                // A divisor written as a literal is E115's; one that is an expression (inside a
                // rounding, §15.213) holds its literals to their units like any other.
                if *op != BinOp::Div || !matches!(**r, Expr::Lit(..)) {
                    self.literal_values(r, path);
                }
            }
            Expr::Call(_, args, _) => {
                for a in args {
                    self.literal_values(a, path);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn expr_ty(&mut self, e: &Expr, path: &str) -> Ty {
        match e {
            Expr::Name(n, sp) => {
                self.used.insert(n.clone());
                match self.syms.get(n) {
                    Some(s) => s.ty.clone(),
                    None => {
                        self.diags.push(
                            Diag::error("E012", tr!("`{n}` という名前は宣言されていません", "The name `{n}` is not declared"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(tr!("上の行までに 入力 / 導出 / 定義 / 表の出力 として宣言されている必要があります。", "It must be declared on an earlier line as an input, a derived value, a definition, or a table output.")),
                        );
                        Ty::Unknown
                    }
                }
            }
            Expr::Lit(l, _) => match l {
                Lit::Num(n) => lit_ty(n),
                Lit::Str(_) => Ty::Str,
                Lit::Date(..) => Ty::Date,
                Lit::Word(w) => {
                    if w == crate::kw::TRUE || w == crate::kw::FALSE {
                        Ty::Bool
                    } else {
                        self.enum_of_value(w).map(Ty::Enum).unwrap_or(Ty::Unknown)
                    }
                }
            },
            Expr::Call(name, args, sp) => {
                // A rounding of a quotient by a value (§15.213) is typed as one: the division is
                // the rounding's, so it is not the `÷` E115 refuses.
                if let Some((n, d)) = var_division(name, args) {
                    return self.divided_rounding(name, n, d, &args[0], &args[1], sp, path);
                }
                let ats: Vec<Ty> = args.iter().map(|a| self.expr_ty(a, path)).collect();
                // The set of calls is closed and each one takes a fixed number of
                // arguments. Until §15.102 neither was checked: `foo(a, b)` and
                // `min(a, b, c)` both passed, and the generator had no case for either —
                // one silently generated 0 (§15.99's class of defect), the other dropped
                // the third argument on the floor.
                let arity = match name.as_str() {
                    crate::kw::DOWN | crate::kw::UP | crate::kw::HALF_UP | crate::kw::HALF_EVEN
                    | crate::kw::HALF_DOWN | crate::kw::MIN | crate::kw::MAX | crate::kw::MOD => Some(2),
                    crate::kw::ALLOCATE => Some(3),
                    crate::kw::ISQRT => Some(1),
                    _ => None,
                };
                match arity {
                    None => {
                        self.diags.push(
                            Diag::error("E118", tr!("`{name}` という関数はありません", "There is no function called `{name}`"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(tr!(
                                    "書けるのは `min` `max` `allocate` `mod` `isqrt` と丸めの五つ（`down` `up` `half_up` `half_down` `half_even`）だけです。",
                                    "The calls are `min`, `max`, `allocate`, `mod`, `isqrt` and the five rounding modes (`down`, `up`, `half_up`, `half_down`, `half_even`)."
                                )),
                        );
                        return Ty::Unknown;
                    }
                    Some(n) if args.len() != n => {
                        self.diags.push(
                            Diag::error("E118", tr!("`{name}` の引数は {n} 個です", "`{name}` takes {n} arguments"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), tr!("ここでは {} 個です", "{} here", args.len()))
                                .note(tr!(
                                    "多い引数は黙って捨てられ、少なければ答えが決まりません。",
                                    "A spare argument would be dropped on the floor, and a missing one leaves no answer."
                                )),
                        );
                        return Ty::Unknown;
                    }
                    Some(_) => {}
                }
                match name.as_str() {
                    crate::kw::DOWN | crate::kw::UP | crate::kw::HALF_UP | crate::kw::HALF_EVEN | crate::kw::HALF_DOWN => {
                        let t = ats.first().cloned().unwrap_or(Ty::Unknown);
                        // The grid of a rounding call, held to what an output's `round` is
                        // held to: a constant (a name passed the check and the generated code
                        // divided by it, zero included, at run time), and a positive one
                        // (E060) (§15.156).
                        match args.get(1) {
                            Some(Expr::Lit(Lit::Num(n), gsp)) => {
                                if lit_value_in(n, &t).or_else(|| lit_value_in(n, &lit_ty(n))).is_some_and(|v| v.num <= 0) {
                                    self.diags.push(nonpositive(n, gsp.clone(), format!("{path}:{}", gsp.line), tr!("丸めの刻み", "the rounding grid")));
                                }
                            }
                            _ => self.diags.push(
                                Diag::error("E118", tr!("`{name}` の刻みは定数で書いてください", "The grid of `{name}` is a constant"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(sp.clone(), tr!("二つめの引数が定数ではありません", "the second argument is not a constant"))
                                    .note(tr!(
                                        "`{name}(<値>, 10円)` のように、丸める刻みを数か金額で書いてください。値によって変わる刻みは、0 になった呼び出しで生成コードが 0 で割ります。",
                                        "Write the grid as a number or an amount, as in `{name}(<value>, 10JPY)`. A grid that moves with the input makes the generated code divide by zero on the call where it is zero."
                                    )),
                            ),
                        }
                        t
                    }
                    // `allocate(T, C, S)` is `floor(T × C ÷ S)` — one operation, so the
                    // scale stays decidable where a bare `÷ S` would not (§2.3, E115).
                    // It is the largest-remainder allocation written as one step: the
                    // caller takes the difference of two of these and the parts add up to
                    // the whole exactly (§15.102).
                    crate::kw::ALLOCATE => {
                        // The two that divide have to be the same kind of thing, or the
                        // ratio is not a ratio; and the whole has to be positive, or the
                        // division has no answer.
                        if !ats[1].unifies(&ats[2]) {
                            self.diags.push(
                                Diag::error("E103", tr!("`allocate` の比の二つの型が違います: {} と {}", "The two sides of the ratio in `allocate` have different types: {} and {}", ats[1], ats[2]))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(sp.clone(), ""),
                            );
                        }
                        // All three are names with declared ranges: the answer is read
                        // through those ranges, so anything without one cannot be checked.
                        let mut named = Vec::new();
                        for (i, what) in [
                            (0usize, tr!("配る額", "the amount")),
                            (1, tr!("累計", "the running total")),
                            (2, tr!("全体", "the whole")),
                        ] {
                            match &args[i] {
                                Expr::Name(n, _) if self.ranges.contains_key(n) => named.push(Some(n.clone())),
                                _ => {
                                    named.push(None);
                                    self.diags.push(
                                        Diag::error("E117", tr!("`allocate` の{what}は範囲のある名前で書いてください", "In `allocate`, {what} has to be a name with a declared range"))
                                            .at(format!("{path}:{}", sp.line))
                                            .mark(sp.clone(), tr!("宣言した範囲がありません", "no declared range"))
                                            .note(tr!(
                                                "三つとも宣言した範囲を見て配るので、入力か導出の名前だけを書けます。",
                                                "All three are read through their declared ranges, so each has to be the name of an input or a derived value."
                                            )),
                                    );
                                }
                            }
                        }
                        // Every part non-negative: the answer is then a truncation, which
                        // every target does the same way — Python floors toward −∞ and Go
                        // toward zero, and they only differ below zero (§7.1).
                        for (i, what) in [(0usize, tr!("配る額", "the amount")), (1, tr!("累計", "the running total"))] {
                            if let Some(n) = &named[i] {
                                let ok = matches!(
                                    self.ranges.get(n),
                                    Some((Some(lo), _)) if lo.cmp_to(Rat::zero()) != std::cmp::Ordering::Less
                                );
                                if !ok {
                                    self.diags.push(
                                        Diag::error("E117", tr!("`allocate` の{what}は負になれません", "In `allocate`, {what} cannot be negative"))
                                            .at(format!("{path}:{}", sp.line))
                                            .mark(sp.clone(), tr!("{n} の範囲が負を含みます", "the range of {n} includes negative values"))
                                            .note(tr!(
                                                "`range >=0円 …` のように書いてください。負の値が混じると、下に丸めるか 0 の方向へ丸めるかで、言語ごとに答えが分かれます。",
                                                "Declare it with `range >=0JPY …`. Below zero the targets disagree about which way to round."
                                            )),
                                    );
                                }
                            }
                        }
                        if let Some(n) = &named[2] {
                            let positive = matches!(
                                self.ranges.get(n),
                                Some((Some(lo), _)) if lo.cmp_to(Rat::zero()) == std::cmp::Ordering::Greater
                            );
                            if !positive {
                                self.diags.push(
                                    Diag::error("E117", tr!("`allocate` の全体は正でなければなりません", "The whole in `allocate` has to be positive"))
                                        .at(format!("{path}:{}", sp.line))
                                        .mark(sp.clone(), tr!("{n} の範囲が 0 を含みます", "the range of {n} includes 0"))
                                        .note(tr!(
                                            "`range >=1円 …` のように、下限を正にしてください。0 で割った答えは決まっていないので、rulec は実行時に落ちる前に、検査の時点でエラーにします。",
                                            "Declare it with a positive lower bound, as in `range >=1JPY …`. Dividing by zero has no answer, so it is refused here rather than at run time."
                                        )),
                                );
                            }
                        }
                        // The running total never passes the whole. Without that the share
                        // is not a share: it could exceed the amount being handed out, and
                        // the reachable interval would be the product of two ranges rather
                        // than the amount itself (§15.102).
                        if let (Some(cn), Some(sn)) = (&named[1], &named[2]) {
                            if !self.at_most.contains(&(cn.clone(), sn.clone())) && cn != sn {
                                self.diags.push(
                                    Diag::error("E117", tr!("`allocate` の累計が全体を超えないと書いてありません", "Nothing says the running total in `allocate` stays within the whole"))
                                        .at(format!("{path}:{}", sp.line))
                                        .mark(sp.clone(), tr!("{cn} <= {sn} の保証がありません", "nothing guarantees {cn} <= {sn}"))
                                        .note(tr!(
                                            "`constraint {cn} <= {sn}` と書いてください。これがないと配る分が配る額を超えることがあり、配分になりません。",
                                            "Write `constraint {cn} <= {sn}`. Without it a share can exceed the amount being handed out, which is not a share."
                                        )),
                                );
                            }
                        }
                        // The answer is at most the amount, but the product on the way
                        // there is not: `T × C` is computed before the division in every
                        // target, and int64 is what it is computed in (§7.1). E108 asks
                        // about the value; this asks about the step on the way.
                        let big = |e: &Expr, t: &Ty| -> Option<i128> {
                            let (_, hi) = self.interval(e, t)?;
                            let sc = self.scale(e)?;
                            let stored = hi.mul(Rat::int(sc));
                            Some(stored.num / stored.den)
                        };
                        if let (Some(t), Some(c)) = (big(&args[0], &ats[0]), big(&args[1], &ats[1])) {
                            let sw = self.scale(&args[2]).unwrap_or(1);
                            let product = t.checked_mul(c).and_then(|p| p.checked_mul(sw));
                            if product.is_none_or(|p| p > i64::MAX as i128) {
                                self.diags.push(
                                    Diag::error("E108", tr!("`allocate` の途中の積が int64 に収まることを証明できません", "Cannot prove that the product inside `allocate` fits in int64"))
                                        .at(format!("{path}:{}", sp.line))
                                        .mark(sp.clone(), "")
                                        .note(tr!(
                                            "配る額は最大 {t}、累計は最大 {c} まで格納され、割る前にこの二つを掛けます。",
                                            "The amount is stored up to {t} and the running total up to {c}, and the two are multiplied before the division."
                                        ))
                                        .note(tr!("ヒント: どちらかの範囲を狭めるか、先に丸めて刻みを粗くしてください。", "Hint: narrow one of the two ranges, or round first so the step is coarser.")),
                                );
                            }
                        }
                        ats.first().cloned().unwrap_or(Ty::Unknown)
                    }
                    crate::kw::MIN | crate::kw::MAX => {
                        if ats.len() == 2 && !ats[0].unifies(&ats[1]) {
                            self.diags.push(
                                Diag::error(
                                    "E103",
                                    tr!("{name} の二つの引数の型が違います: {} と {}", "The two arguments of {name} have different types: {} and {}", ats[0], ats[1]),
                                )
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), ""),
                            );
                        } else if let Some(Ty::Enum(en)) = ats.first() {
                            // The larger of two values of an ordered enum is the later one in its
                            // order, and an enum without one has no larger value (§15.210).
                            if !self.ordered.contains(en) {
                                let at = format!("{path}:{}", sp.line);
                                self.diags.push(unordered_comparison(en, &at, sp, format!("`{name}`")));
                            }
                        }
                        ats.first().cloned().unwrap_or(Ty::Unknown)
                    }
                    // `mod(x, n)`: the remainder of a number by a positive whole constant, from 0
                    // up to n − 1 (§15.214). The divisor is a constant for the reason E115 gives
                    // for any divisor, and the number is a `number`: the remainder of an amount
                    // or of a rate by a count is not a thing a rule asks for.
                    crate::kw::MOD => {
                        let t = ats[0].clone();
                        if !matches!(t, Ty::Number | Ty::Unknown) {
                            self.diags.push(
                                Diag::error("E103", tr!("`mod` が取るのは number だけです: {t} が渡されています", "`mod` takes a number only, and is given {t}"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(args[0].span().clone(), tr!("{t} です", "this is {t}"))
                                    .note(tr!(
                                        "余りを取るのは、通し番号や回数のような単位の無い整数です。金額や率の余りが要るなら、先に `number` の値にしてください。",
                                        "A remainder is taken of a count with no unit: a running number, a number of turns. Where an amount or a rate is meant, make it a `number` first."
                                    )),
                            );
                        } else if t == Ty::Number && self.lattice(&args[0]).is_none_or(|l| !l.is_int()) {
                            // A number divided by a constant is a fraction, and the remainder of a
                            // fraction is not what a turn counter or a running number asks for.
                            self.diags.push(
                                Diag::error("E103", tr!("`mod` が取るのは整数だけです", "`mod` takes a whole number only"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(args[0].span().clone(), tr!("整数にならない値です", "this is not always a whole number"))
                                    .note(tr!(
                                        "割り算で端数の出る値は、先に `down(x / 2, 1)` のように丸めてから渡してください。",
                                        "Round a value that division leaves a fraction in first, as in `down(x / 2, 1)`."
                                    )),
                            );
                        }
                        let k = match &args[1] {
                            Expr::Lit(Lit::Num(n), _) if lit_ty(n) == Ty::Number => const_value(&args[1]),
                            _ => None,
                        };
                        if k.is_none() {
                            let mark = if matches!(args[1], Expr::Lit(..)) {
                                tr!("割る数が正の整数ではありません", "the divisor is not a positive whole number")
                            } else {
                                tr!("割る数が定数ではありません", "the divisor is not a constant")
                            };
                            self.diags.push(
                                Diag::error("E115", tr!("割る数が正の定数ではありません", "The divisor is not a positive constant"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(args[1].span().clone(), mark)
                                    .note(tr!(
                                        "`mod` の割る数は、`mod(x, 7)` のように正の整数の定数で書いてください。答えはいつも 0 から割る数 − 1 までで、範囲も刻みも静的に決まります。",
                                        "The divisor of `mod` is a positive whole constant, as in `mod(x, 7)`. The answer is then always from 0 to one less than it, and its range and step are known statically."
                                    ))
                                    .note(tr!(
                                        "割る数が変わる余りは、言語ごとに負の値の扱いが違い、0 で割る呼び出しも起こりえます。",
                                        "A remainder by a value that varies is where languages disagree below zero, and where a call can divide by zero."
                                    )),
                            );
                        }
                        // The remainder is taken of the stored integer by the divisor at the same
                        // scale; both have to fit int64.
                        if let (Some(k), Some((lo, hi)), Some(sc)) = (k, self.interval(&args[0], &t), self.scale(&args[0])) {
                            let mag = magnitude_of(lo, hi).mul(Rat::int(sc));
                            let stored = mag.num / mag.den;
                            let modulus = k.checked_mul(sc);
                            if stored > i64::MAX as i128 || modulus.is_none_or(|m| m > i64::MAX as i128) {
                                self.diags.push(
                                    Diag::error("E108", tr!("`mod` の引数が int64 に収まることを証明できません", "Cannot prove that the argument of `mod` fits in int64"))
                                        .at(format!("{path}:{}", sp.line))
                                        .mark(args[0].span().clone(), "")
                                        .note(tr!(
                                            "とりうる値は {} から {} で、刻みが 1/{sc} なので、格納される整数は最大 {stored} になります。",
                                            "It ranges from {} to {} and the step is 1/{sc}, so the stored integer reaches {stored}.",
                                            fmt_val(lo, &t),
                                            fmt_val(hi, &t)
                                        ))
                                        .note(tr!("ヒント: 入力の範囲を狭めてください。", "Hint: narrow the input ranges.")),
                                );
                            }
                        }
                        Ty::Number
                    }
                    // `isqrt(x)`: the largest whole number whose square does not pass x
                    // (§15.215). A power is written as a product, so `isqrt(n * n * n)` is
                    // `floor(n^1.5)` exactly. Below zero there is no root: E139.
                    crate::kw::ISQRT => {
                        let t = ats[0].clone();
                        if !matches!(t, Ty::Number | Ty::Unknown) {
                            self.diags.push(
                                Diag::error("E103", tr!("`isqrt` が取るのは number だけです: {t} が渡されています", "`isqrt` takes a number only, and is given {t}"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(args[0].span().clone(), tr!("{t} です", "this is {t}"))
                                    .note(tr!(
                                        "平方根は単位の無い数について取ります。金額や量の平方根は、その単位の値ではありません。",
                                        "A square root is taken of a number with no unit: the root of an amount or of a quantity is not a value of that unit."
                                    )),
                            );
                            return Ty::Number;
                        }
                        let arg = &args[0];
                        let text = crate::region::expr_text(arg);
                        let found = self.interval(arg, &t);
                        let below = match found {
                            Some((lo, _)) => lo.num < 0,
                            None => true,
                        };
                        if below && t != Ty::Unknown {
                            let mark = match found {
                                Some((lo, hi)) => tr!("とりうる値は {} から {} で、負の値を含みます", "it ranges from {} to {}, below zero included", fmt_val(lo, &t), fmt_val(hi, &t)),
                                None => tr!("範囲の決まらない値を読んでいます", "it reads a value with no range"),
                            };
                            self.diags.push(
                                Diag::error("E139", tr!("`isqrt` の引数 {text} が負になりえます", "The argument {text} of `isqrt` can be below zero"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(arg.span().clone(), mark)
                                    .note(tr!(
                                        "負の数に平方根はありません。生成コードは、その呼び出しで答えを持ちません。rulec は、引数が 0 以上だと範囲から示せた `isqrt` だけを通します。",
                                        "A number below zero has no square root, and the generated code would have no answer on that call. rulec passes an `isqrt` only where the ranges show its argument is zero or more."
                                    ))
                                    .note(tr!(
                                        "ヒント: 引数の範囲を 0 以上に狭めるか、`max({text}, 0)` のように 0 以上にしてから渡してください。",
                                        "Hint: narrow the ranges so that the argument stays at zero or more, or pass `max({text}, 0)`."
                                    )),
                            );
                            self.arith_witness.push(Witnessed { code: "E139", span: arg.span().clone(), expr: arg.clone() });
                        }
                        if let (Some((lo, hi)), Some(sc)) = (found, self.scale(arg)) {
                            let mag = magnitude_of(lo, hi).mul(Rat::int(sc));
                            let stored = mag.num / mag.den;
                            if stored > i64::MAX as i128 {
                                self.diags.push(
                                    Diag::error("E108", tr!("`isqrt` の引数が int64 に収まることを証明できません", "Cannot prove that the argument of `isqrt` fits in int64"))
                                        .at(format!("{path}:{}", sp.line))
                                        .mark(arg.span().clone(), "")
                                        .note(tr!(
                                            "とりうる値は {} から {} で、刻みが 1/{sc} なので、格納される整数は最大 {stored} になります。冪は掛け算で書くので、`n * n * n` のような積は速く大きくなります。",
                                            "It ranges from {} to {} and the step is 1/{sc}, so the stored integer reaches {stored}. A power is written as a product, and a product such as `n * n * n` grows fast.",
                                            fmt_val(lo, &t),
                                            fmt_val(hi, &t)
                                        ))
                                        .note(tr!("ヒント: 入力の範囲を狭めてください。", "Hint: narrow the input ranges.")),
                                );
                            }
                        }
                        Ty::Number
                    }
                    _ => Ty::Unknown,
                }
            }
            Expr::Bin(l, op, r, sp) => {
                let lt = self.expr_ty(l, path);
                let rt = self.expr_ty(r, path);
                use BinOp::*;
                // Ordered, not arithmetic (§15.84). The comparisons below are exactly what
                // these dimensions are for, so the guard sits in front of the four that are
                // not comparisons.
                if matches!(op, Add | Sub | Mul | Div) {
                    let bad = [&lt, &rt].into_iter().find(|t| compares_only(t));
                    if let Some(t) = bad {
                        self.no_arithmetic(t, sp, path);
                        return Ty::Unknown;
                    }
                }
                match op {
                    Le | Ge | Lt | Gt | Eq => {
                        if !lt.unifies(&rt) {
                            self.mix(&lt, &rt, sp, path);
                        } else if *op != Eq {
                            // Two values of an enum are put in order by the order it declares, and
                            // one that declares none has nothing to put them in (§15.210). The
                            // evaluator answered nothing for such a comparison, and the generated
                            // code compared whatever each language holds an enum as.
                            if let Ty::Enum(en) = &lt {
                                if !self.ordered.contains(en) {
                                    let at = format!("{path}:{}", sp.line);
                                    self.diags.push(unordered_comparison(en, &at, sp, tr!("大小の比較", "a comparison of order")));
                                }
                            }
                        }
                        Ty::Bool
                    }
                    Add | Sub => {
                        if !lt.unifies(&rt) {
                            self.mix(&lt, &rt, sp, path);
                            return Ty::Unknown;
                        }
                        if matches!(lt, Ty::Money { tax: None, .. }) { rt } else { lt }
                    }
                    Mul => match (&lt, &rt) {
                        // §2.3: money × rate is unrounded money. money × money is forbidden.
                        // A number is dimensionless too, so it multiplies the same way — that
                        // is what makes `送料 × 個数` say what it means.
                        (Ty::Money { .. }, Ty::Rate | Ty::Number) => lt.clone(),
                        (Ty::Rate | Ty::Number, Ty::Money { .. }) => rt.clone(),
                        (Ty::Qty { .. }, Ty::Rate | Ty::Number)
                        | (Ty::Rate | Ty::Number, Ty::Qty { .. }) => {
                            if matches!(lt, Ty::Rate | Ty::Number) { rt } else { lt }
                        }
                        // A rate of a count is a rate, and a count of counts is a count.
                        (Ty::Number, Ty::Rate) | (Ty::Rate, Ty::Number) => Ty::Rate,
                        // Two values that both carry a unit. Their product is a compound
                        // dimension — 円×円, g×cm, cm×cm — and §2.1 stops it as a modeling
                        // error rather than inventing a dimension to hold it.
                        (
                            Ty::Money { .. } | Ty::Qty { .. },
                            Ty::Money { .. } | Ty::Qty { .. },
                        ) => {
                            self.compound(&lt, &rt, sp, path);
                            Ty::Unknown
                        }
                        (Ty::Rate, Ty::Rate) => Ty::Rate,
                        _ => lt.clone(),
                    },
                    Div => {
                        // §2.3 and §7.4: the divisor is a constant, always. A variable
                        // divisor would leave the scale undecidable, and the generator would
                        // have to fall back on a language's own division — Python rounds
                        // toward -inf and Go toward zero, so the two would disagree.
                        if const_value(r).is_none() {
                            // A literal that is not a positive whole number (`÷ 0`, `÷ -2`,
                            // `÷ 0.5`) is not a variable, and saying so sent the reader looking
                            // for one.
                            let mark = if matches!(r.as_ref(), Expr::Lit(..)) {
                                tr!("割る数が正の整数ではありません", "the divisor is not a positive whole number")
                            } else {
                                tr!("割る数が定数ではありません", "the divisor is not a constant")
                            };
                            self.diags.push(
                                Diag::error("E115", tr!("割る数が正の定数ではありません", "The divisor is not a positive constant"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(sp.clone(), mark)
                                    .note(tr!(
                                        "割る数は正の整数の定数か、同じ単位の金額・数量の定数だけです。",
                                        "A divisor is a positive whole constant, or a constant amount or quantity in the same unit."
                                    ))
                                    .note(tr!(
                                        "割る数が業務のデータなら、それは率か、定数を引く表として書けるはずです。割合そのものを渡したいなら `rate` の入力にしてください。",
                                        "If the divisor is business data, it can be written as a rate, or as a table that looks the constant up. To pass a ratio in, make it a `rate` input."
                                    ))
                                    .note(tr!(
                                        "刻みが静的に決まらないと、生成コードは言語の除算に頼ることになり、Python は −∞ 方向、Go は 0 方向に丸めて答えが食い違います。",
                                        "Without a statically known step the generated code would fall back on the language's own division, and Python rounding toward -inf and Go toward zero would give different answers."
                                    ))
                                    // A value that varies can still be divided by, inside a
                                    // rounding (§15.213); this division is not that one.
                                    .maybe_note((!matches!(r.as_ref(), Expr::Lit(..))).then(|| {
                                        tr!(
                                            "ヒント: 値で割れるのは、丸めの引数のいちばん外側の割り算だけです（`down(a * b / c, 1円)`）。答えは正確な商を刻みに乗せたものになります。",
                                            "Hint: a value can be divided by only as the outermost operation of a rounding's argument (`down(a * b / c, 1JPY)`). The answer is then the exact quotient put on the grid."
                                        )
                                    })),
                            );
                            return Ty::Unknown;
                        }
                        // Dividing by a constant of the same unit cancels it and leaves a
                        // plain number — this is the "one point per 100 yen" case. A
                        // dimensionless divisor leaves the left side as it was.
                        let dimensioned = |t: &Ty| matches!(t, Ty::Money { .. } | Ty::Qty { .. });
                        if lt == Ty::Unknown || !dimensioned(&rt) {
                            return lt.clone();
                        }
                        if dimensioned(&lt) && lt.unifies(&rt) {
                            return Ty::Number;
                        }
                        // The divisor is read at its own unit, so `重さ(mass[g]) ÷ 2kg`
                        // divided by 2 and stayed a mass, and `重さ ÷ 2m` did the same. Both
                        // looked right and were not.
                        if dim_of(&lt) == dim_of(&rt) {
                            self.divisor_unit(&lt, &rt, sp, path);
                        } else {
                            self.compound(&lt, &rt, sp, path);
                        }
                        Ty::Unknown
                    }
                }
            }
        }
    }

    /// `<mode>(n ÷ d, <grid>)` with a divisor that is not a constant (§15.213).
    ///
    /// The quotient is typed by §2.3's rule for a division: a value over another of the same unit
    /// is a ratio with no unit, and a value over a `number` or a rate keeps its type. The answer
    /// is the quotient put on the grid, so the grid is written in the quotient's own unit — a
    /// ratio's grid is a number or a rate, and which of the two it is says which the answer is
    /// (E103). The divisor's range has to keep it off zero (E132), and the two integers the
    /// generated code divides have to fit int64 (E108).
    #[allow(clippy::too_many_arguments)]
    fn divided_rounding(&mut self, name: &str, n: &Expr, d: &Expr, arg: &Expr, grid: &Expr, sp: &Span, path: &str) -> Ty {
        let nt = self.expr_ty(n, path);
        let dt = self.expr_ty(d, path);
        let asp = arg.span().clone();
        if let Some(t) = [&nt, &dt].into_iter().find(|t| compares_only(t)).cloned() {
            self.no_arithmetic(&t, &asp, path);
            return Ty::Unknown;
        }
        let dimensioned = |t: &Ty| matches!(t, Ty::Money { .. } | Ty::Qty { .. });
        let q = if nt == Ty::Unknown || dt == Ty::Unknown {
            Ty::Unknown
        } else if !dimensioned(&dt) {
            nt.clone()
        } else if dimensioned(&nt) && nt.unifies(&dt) {
            // A ratio. Whether it is a rate or a count is the grid's to say, below.
            Ty::Number
        } else if dim_of(&nt) == dim_of(&dt) {
            self.divisor_unit(&nt, &dt, &asp, path);
            return Ty::Unknown;
        } else {
            self.compound(&nt, &dt, &asp, path);
            return Ty::Unknown;
        };
        let Expr::Lit(Lit::Num(g), gsp) = grid else {
            self.diags.push(
                Diag::error("E118", tr!("`{name}` の刻みは定数で書いてください", "The grid of `{name}` is a constant"))
                    .at(format!("{path}:{}", sp.line))
                    .mark(sp.clone(), tr!("二つめの引数が定数ではありません", "the second argument is not a constant"))
                    .note(tr!(
                        "`{name}(<値>, 10円)` のように、丸める刻みを数か金額で書いてください。値によって変わる刻みは、0 になった呼び出しで生成コードが 0 で割ります。",
                        "Write the grid as a number or an amount, as in `{name}(<value>, 10JPY)`. A grid that moves with the input makes the generated code divide by zero on the call where it is zero."
                    )),
            );
            return q;
        };
        let gt = lit_ty(g);
        let answer = match &q {
            Ty::Unknown => Ty::Unknown,
            Ty::Money { .. } | Ty::Qty { .. } if gt.unifies(&q) => q.clone(),
            Ty::Number | Ty::Rate if matches!(gt, Ty::Number | Ty::Rate) => gt.clone(),
            _ => {
                let want = if dimensioned(&q) { tr!("{q} の単位", "the unit of {q}") } else { tr!("数か率", "a number or a rate") };
                self.diags.push(
                    Diag::error("E103", tr!("丸めの刻み `{}` は {q} の値ではありません", "The rounding grid `{}` is not a value of {q}", g.raw))
                        .at(format!("{path}:{}", gsp.line))
                        .mark(gsp.clone(), tr!("割った値は {q} です", "the quotient is {q}"))
                        .note(tr!(
                            "刻みは、割った値の単位で書いてください（ここでは{want}）。同じ単位の値どうしを割った値は単位の無い比なので、刻みは `1` や `0.1%` です。",
                            "Write the grid in the unit of the quotient (here, {want}). A value divided by another of the same unit is a ratio with no unit, whose grid is `1` or `0.1%`."
                        )),
                );
                return Ty::Unknown;
            }
        };
        if lit_value_in(g, &gt).is_some_and(|v| v.num <= 0) {
            self.diags.push(nonpositive(g, gsp.clone(), format!("{path}:{}", gsp.line), tr!("丸めの刻み", "the rounding grid")));
            return answer;
        }
        if dt == Ty::Unknown {
            return answer;
        }
        // E132: the divisor stays off zero over every value its ranges allow.
        let text = crate::region::expr_text(d);
        let reach = self.interval(d, &dt);
        let mark = match reach {
            Some((lo, hi)) if (lo.num > 0 && hi.num > 0) || (lo.num < 0 && hi.num < 0) => None,
            Some((lo, hi)) => Some(tr!("とりうる値は {} から {} で、0 を含みます", "it ranges from {} to {}, zero included", fmt_val(lo, &dt), fmt_val(hi, &dt))),
            None => Some(tr!("範囲の決まらない値を読んでいます", "it reads a value with no range")),
        };
        if let Some(mark) = mark {
            self.diags.push(
                Diag::error("E132", tr!("割る数 {text} が 0 になりえます", "The divisor {text} can be zero"))
                    .at(format!("{path}:{}", sp.line))
                    .mark(d.span().clone(), mark)
                    .note(tr!(
                        "0 で割った答えは決まっていません。生成コードはその呼び出しで止まります。rulec は、割る数が 0 にならないと範囲から示せた割り算だけを通します。",
                        "Dividing by zero has no answer, and the generated code would stop on that call. rulec passes a division only where the ranges show its divisor is never zero."
                    ))
                    .note(tr!(
                        "ヒント: 割る数の範囲を 0 を含まないように狭めるか、`max({text}, 1)` のように 1 以上にしてから割るか、0 のときに別の値を選ぶ表を置いて、その表の出力で割ってください。",
                        "Hint: narrow the ranges so that the divisor cannot be zero, divide by `max({text}, 1)`, or put a table that picks another value when it is zero and divide by that table's output."
                    )),
            );
            self.arith_witness.push(Witnessed { code: "E132", span: d.span().clone(), expr: d.clone() });
            return answer;
        }
        // E108: the generated code divides one integer by another, each the value at its own
        // scale brought to the grid's (`division_factors`); both have to fit.
        let parts = (self.interval(n, &nt), self.scale(n), reach, self.scale(d), grid_int(g));
        if let (Some((nl, nh)), Some(sn), Some((dl, dh)), Some(sd), Some((gi, sg))) = parts {
            let (ka, kb) = division_factors(sn, sd, gi, sg);
            for (what, lo, hi, s, k, e, t) in [
                (tr!("割られる数", "the dividend"), nl, nh, sn, ka, n, &nt),
                (tr!("割る数", "the divisor"), dl, dh, sd, kb, d, &dt),
            ] {
                let stored = magnitude_of(lo, hi).mul(Rat::int(s)).mul(Rat::int(k));
                let stored = stored.num / stored.den;
                if stored > i64::MAX as i128 {
                    self.diags.push(
                        Diag::error("E108", tr!("`{name}` の{what}が int64 に収まることを証明できません", "Cannot prove that {what} in `{name}` fits in int64"))
                            .at(format!("{path}:{}", sp.line))
                            .mark(e.span().clone(), "")
                            .note(tr!(
                                "とりうる値は {} から {} で、刻みをそろえて割るので、格納される整数は最大 {stored} になります。",
                                "It ranges from {} to {}, and the two are brought to one step before the division, so the stored integer reaches {stored}.",
                                fmt_val(lo, t),
                                fmt_val(hi, t)
                            ))
                            .note(tr!("ヒント: 入力の範囲を狭めるか、丸めの刻みを粗くしてください。", "Hint: narrow the input ranges, or round to a coarser grid.")),
                    );
                }
            }
        }
        answer
    }

    /// Two values that both carry a unit, multiplied or divided into one another. §2.1 does
    /// not do dimensional analysis: there is no dimension for 円×円 or for kg÷m, and
    /// inventing one silently is worse than stopping. Until this was checked, `縦 × 横` in a
    /// `length[m]` column came back as a length and compared against a length threshold.
    fn compound(&mut self, a: &Ty, b: &Ty, sp: &Span, path: &str) {
        self.diags.push(
            Diag::error("E103", tr!("単位どうしを掛け合わせています: {a} と {b}", "Compound dimension: {a} and {b}"))
                .at(format!("{path}:{}", sp.line))
                .mark(sp.clone(), "")
                .note(tr!(
                    "rulec には、円×円 や g×cm のような、単位を掛け合わせた単位がありません。次元解析はしないので、書き方の誤りとして止めます。",
                    "There is no compound dimension for JPY×JPY or g×cm. This tool does not do dimensional analysis, so it is stopped as a modeling error."
                ))
                .note(tr!(
                    "掛ける相手が業務のデータなら、それは率か個数（`rate`、`number`）のはずです。面積のように積そのものが答えなら、入力として受け取ってください。",
                    "If the other operand is business data, it is a rate or a count (`rate`, `number`). Where the product itself is the answer, as an area is, take it as an input."
                )),
        );
    }

    /// A divisor of the right dimension but the wrong unit. The divisor is read at its own
    /// unit and never converted, so this used to divide by the bare number written.
    fn divisor_unit(&mut self, a: &Ty, b: &Ty, sp: &Span, path: &str) {
        self.diags.push(
            Diag::error("E103", tr!("割る数の単位が合いません: {a} を {b} で割っています", "The divisor's unit does not match: {a} divided by {b}"))
                .at(format!("{path}:{}", sp.line))
                .mark(sp.clone(), "")
                .note(tr!(
                    "同じ次元でも、割る数は左辺と同じ単位で書いてください。割る数は書いてある単位のまま読まれるので、`mass[g]` の値を `2kg` で割ると 2000 ではなく 2 で割られます。",
                    "Even within one dimension, write the divisor in the same unit as the left side. A divisor is read at the unit it is written in, so dividing a `mass[g]` by `2kg` divides by 2, not by 2000."
                )),
        );
    }

    /// A dimension that is ordered but has no arithmetic, used in an arithmetic expression.
    fn no_arithmetic(&mut self, t: &Ty, sp: &Span, path: &str) {
        let why = match t {
            Ty::Date => tr!(
                "日付は暦日で、足し引きの結果を入れる型がありません。二つの日付のあいだの日数が要るなら、それは呼び出し側で数えて `number` か `duration` として渡してください。",
                "A date is a calendar day, and there is no type to hold the result of adding or subtracting one. Where the days between two dates are needed, count them on the calling side and pass them in as a `number` or a `duration`."
            ),
            _ => tr!(
                "温度と音量は、比べるためだけの目盛りです。℃ は 0 が「無い」を意味しないので `気温 × 2` に意味が無く、dB は対数なので、二つ足しても音が二つ分になるわけではありません。",
                "A temperature and a sound level are scales to compare against, nothing more. A ℃ has a displaced zero, so `temp × 2` means nothing, and a decibel is a logarithm, so adding two of them is not two sounds' worth."
            ),
        };
        self.diags.push(
            Diag::error("E048", tr!("{t} には足し算も掛け算もありません", "{t} has no arithmetic"))
                .at(format!("{path}:{}", sp.line))
                .mark(sp.clone(), tr!("この型が使えるのは比較と範囲だけです", "this type is for comparison and range only"))
                .note(why)
                .note(tr!(
                    "閾値として比べるか、`{}` に書いてください。差や倍率そのものが業務ルールなら、計算した結果を入力として受け取るか、表で引いてください。",
                    "Compare it against a threshold, or write it in a `{}`. Where a difference or a multiple is itself the rule, take the computed value as an input, or look it up in a table.",
                    crate::kw::RANGE
                )),
        );
    }

    fn mix(&mut self, a: &Ty, b: &Ty, sp: &Span, path: &str) {
        let note = match (a, b) {
            (Ty::Money { tax: Some(x), .. }, Ty::Money { tax: Some(y), .. }) if x != y => {
                tr!("税の変換は変換式ではなく表として書いてください。", "Write a tax conversion as a table, not as a conversion formula.")
            }
            _ => tr!("単位の違う値は足せません。数量に応じた加算料金なら、それは表で書いてください。", "Values of different dimensions cannot be added. A surcharge that depends on a quantity is written as a table."),
        };
        self.diags.push(
            Diag::error("E103", tr!("単位の混同: {a} に {b} を足しています", "Mixed units: adding {b} to {a}"))
                .at(format!("{path}:{}", sp.line))
                .mark(sp.clone(), "")
                .note(tr!("ヒント: {note}", "Hint: {note}")),
        );
    }

    /// The first enum, in declaration order, that has a value of this name. Where the place
    /// a value is written says which enum it is, `enum_of_value_in` reads it there instead.
    fn enum_of_value(&self, v: &str) -> Option<String> {
        self.enum_order.iter().find(|e| self.enum_has(e, v)).cloned()
    }

    fn enum_has(&self, e: &str, v: &str) -> bool {
        self.enums.get(e).is_some_and(|vs| vs.iter().any(|x| x == v))
    }

    /// An output cell that names a value of another enum than its column's. It passed in
    /// silence, and the generated code returned a value of the wrong type (§15.150).
    fn value_of_other_enum(&mut self, w: &str, e: &str, want: &Ty, at: &str, sp: &Span) {
        let col = match want {
            Ty::Opt(t) => t.as_ref(),
            t => t,
        };
        if matches!(col, Ty::Unknown) || matches!(col, Ty::Enum(c) if c == e) {
            return;
        }
        self.diags.push(
            Diag::error("E103", tr!("この列は {want} ですが `{w}`（{e} の値）が書かれています", "This column is {want}, but `{w}` (a value of {e}) is written here"))
                .at(at.to_string())
                .mark(sp.clone(), ""),
        );
    }

    /// The enum a value is read in: the enum of the column it is written under when that one
    /// has it, since two enums may both have `automatic` (§15.150); else the first that has it.
    fn enum_of_value_in(&self, v: &str, want: &Ty) -> Option<String> {
        let want = match want {
            Ty::Opt(t) => t.as_ref(),
            t => t,
        };
        match want {
            Ty::Enum(e) if self.enum_has(e, v) => Some(e.clone()),
            _ => self.enum_of_value(v),
        }
    }

    /// Whether a name's declared range starts at zero or above. A summed column needs
    /// this so the running total only ever rises (§15.100).
    fn non_negative(&self, name: &str) -> bool {
        matches!(self.ranges.get(name), Some((Some(lo), _)) if lo.cmp_to(Rat::zero()) != std::cmp::Ordering::Less)
    }

    /// `count 一致数(hits) over 納入先 where 判定 = 一致  range >=0 <=100` (§15.58), `sum` (§15.100),
    /// and `max` and `min` (§15.219).
    ///
    /// Three things have to hold for a count to be a number the rest of the rule can treat like
    /// any other: the test reads a column of **one element** of the sequence it runs over, that
    /// column's values are a closed set, and the range is declared. The range is not decoration
    /// — it is the universe the completeness check quantifies over when the count becomes a
    /// column, and it is the cap the generated code holds the sequence to.
    ///
    /// `max` and `min` read a column of numbers, dates or an ordered enum, and answer `empty` when
    /// no element is looked at. They need no range: the answer is one of the column's values or the
    /// `empty` value, so the range is worked out from the column's and that value. One that is
    /// written has to hold it (E112).
    fn agg_decl(&mut self, d: &AggDecl, f: &RuleFile, path: &str) {
        let summing = d.kind == AggKind::Sum;
        let extreme = d.kind.extreme();
        let at = match d.kind {
            AggKind::Sum => tr!("{path}:{} 合計", "{path}:{} sum", d.span.line),
            AggKind::Count => tr!("{path}:{} 数え上げ", "{path}:{} count", d.span.line),
            AggKind::Max => tr!("{path}:{} 最大", "{path}:{} max", d.span.line),
            AggKind::Min => tr!("{path}:{} 最小", "{path}:{} min", d.span.line),
        };
        let col = d.column.text.clone();

        let seq = f.elements.iter().position(|e| e.name.text == d.over);
        if seq.is_none() {
            self.diags.push(
                Diag::error("E028", tr!("`over` が指す並びがありません", "`over` names a sequence that is not declared"))
                    .at(at.clone())
                    .mark(d.span.clone(), tr!("{} は宣言されていません", "{} is not declared", d.over))
                    .note(tr!(
                        "数えられるのは `elements <名前>(<別名>)` で宣言した並びだけです。",
                        "Only the sequence declared by `elements <name>(<alias>)` can be counted."
                    )),
            );
        }

        // The values of one element of the sequence this runs over, worked out over the items
        // above (§15.219): a value of another sequence's elements is not one it sees.
        let st = stages(f);
        let of_this = |n: &str| st.each.get(n).copied() == seq || seq.is_none() && st.each.contains_key(n);
        let other = |n: &str| st.each.get(n).filter(|s| Some(**s) != seq).map(|s| f.elements[*s].name.text.clone());
        let sym = self.syms.get(&col).cloned();
        // A `max` and a `min` count nothing: their title says what they do with the column.
        let title = if extreme { tr!("この列はまとめられません", "This column cannot be summarised") } else { tr!("この列は数えられません", "This column cannot be counted") };
        let bad = |c: &mut Self, sp: &Span, what: String, note: String| {
            c.diags.push(Diag::error("E029", title.clone()).at(at.clone()).mark(sp.clone(), what).note(note));
        };
        match &sym {
            None => self.diags.push(
                Diag::error("E012", tr!("`{}` という列はありません", "There is no column called `{}`", col))
                    .at(at.clone())
                    .mark(d.column.span.clone(), tr!("宣言されていません", "not declared")),
            ),
            Some(_) if other(&col).is_some() => bad(
                self,
                &d.column.span,
                tr!("{col} は {} の要素の値です", "{col} is a value of one element of {}", other(&col).unwrap_or_default()),
                tr!(
                    "まとめられるのは、`over` が指す並びの要素の値だけです。ほかの並びの値は、その並びを `over` に書いた行でまとめてください。",
                    "Only a value of one element of the sequence `over` names can be summarised. Summarise another sequence's value on a line whose `over` names that sequence."
                ),
            ),
            Some(_) if !of_this(&col) => bad(
                self,
                &d.column.span,
                tr!("{col} は要素ごとの値ではありません", "{col} is not a value of one element"),
                tr!(
                    "まとめられるのは、要素のフィールドか、要素ごとの表が出した列だけです。一回の呼び出しに一つしかない値は、並びとしてまとめられません。",
                    "Only a field of an element, or a column a per-element table produces, can be summarised. A value there is one of per call says nothing about the sequence."
                ),
            ),
            // A sum reads a number; a count reads a value out of a closed set.
            Some(sym) if summing && !matches!(sym.ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Number | Ty::Rate) => bad(
                self,
                &d.column.span,
                tr!("{col} は {} です", "{col} is {}", sym.ty),
                tr!(
                    "合計できるのは数の列（金額・数量・`number`・`rate`）だけです。真偽や列挙を合計しても意味が決まりません。",
                    "Only a column of numbers — an amount, a quantity, `number` or `rate` — can be summed. A bool or an enum has no total."
                ),
            ),
            // The running total has to move one way, so the guard can refuse a sequence the
            // moment it leaves the declared range and the accumulator stays inside int64
            // (§15.100). A column that can go negative has no such guard.
            Some(_) if summing && !self.non_negative(&col) => bad(
                self,
                &d.column.span,
                tr!("{col} は負になりえます", "{col} can be negative"),
                tr!(
                    "合計する列には `range >=0…` が要ります。負の値が混じると途中の合計が上下するので、生成コードは、合計が宣言した範囲を出た時点でエラーにすることができません。差を取りたいなら、正の列を二つ合計して引いてください。",
                    "A summed column needs `range >=0…`. With negative values the running total moves both ways, and the guard cannot refuse the moment it leaves the declared range. To take a difference, sum two non-negative columns and subtract."
                ),
            ),
            // The largest or the smallest of a column needs an order: a number, a date, or an
            // enum whose values are ordered (§15.210).
            Some(sym)
                if extreme
                    && !matches!(sym.ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Number | Ty::Rate | Ty::Date)
                    && self.ordered_enum(&sym.ty).is_none() =>
            {
                bad(
                    self,
                    &d.column.span,
                    tr!("{col} は {} です", "{col} is {}", sym.ty),
                    tr!(
                        "最大と最小を取れるのは、順のある列（金額・数量・`number`・`rate`・日付、`<` で並べた列挙）だけです。値が無いことのある列（`?`）と、順の無い列挙には、大小がありません。",
                        "Only a column with an order has a largest and a smallest value: an amount, a quantity, `number`, `rate`, a date, or an enum whose values are ordered with `<`. A column that may have no value (`?`) and an enum without an order have neither."
                    ),
                );
            }
            Some(sym) if !summing && !extreme && !matches!(sym.ty, Ty::Bool | Ty::Enum(_)) => bad(
                self,
                &d.column.span,
                tr!("{col} は {} です", "{col} is {}", sym.ty),
                tr!(
                    "数える条件は「その列がこの値であること」なので、値が有限の集合である列（真偽か列挙）にだけ書けます。",
                    "The test is \"this column has this value\", so the column's values have to be a closed set: a bool or an enum."
                ),
            ),
            Some(_) if summing || extreme => {}
            Some(sym) => {
                let ty = sym.ty.clone();
                self.test_value(&col, &d.column.span, &d.value, &ty, &bad);
            }
        }
        self.used.insert(col);
        // The elements `max` and `min` look at: a test on a column of one element, read the way a
        // count reads its own (§15.219).
        if let Some((w, want)) = &d.filter {
            match self.syms.get(&w.text).cloned() {
                None => self.diags.push(
                    Diag::error("E012", tr!("`{}` という列はありません", "There is no column called `{}`", w.text))
                        .at(at.clone())
                        .mark(w.span.clone(), tr!("宣言されていません", "not declared")),
                ),
                Some(_) if other(&w.text).is_some() => bad(
                    self,
                    &w.span,
                    tr!("{} は {} の要素の値です", "{} is a value of one element of {}", w.text, other(&w.text).unwrap_or_default()),
                    tr!(
                        "`where` が読めるのは、`over` が指す並びの要素の値だけです。",
                        "`where` can only read a value of one element of the sequence `over` names."
                    ),
                ),
                Some(_) if !of_this(&w.text) => bad(
                    self,
                    &w.span,
                    tr!("{} は要素ごとの値ではありません", "{} is not a value of one element", w.text),
                    tr!(
                        "`where` は、要素ごとに、その要素を見るかどうかを決めます。一回の呼び出しに一つしかない値では、要素ごとに決まりません。",
                        "`where` decides, element by element, whether the element is looked at. A value there is one of per call decides nothing element by element."
                    ),
                ),
                Some(sym) if !matches!(sym.ty, Ty::Bool | Ty::Enum(_)) => bad(
                    self,
                    &w.span,
                    tr!("{} は {} です", "{} is {}", w.text, sym.ty),
                    tr!(
                        "`where` の条件は「その列がこの値であること」なので、値が有限の集合である列（真偽か列挙）にだけ書けます。",
                        "The test of `where` is \"this column has this value\", so the column's values have to be a closed set: a bool or an enum."
                    ),
                ),
                Some(sym) => {
                    let ty = sym.ty.clone();
                    self.test_value(&w.text, &w.span, want, &ty, &bad);
                }
            }
            self.used.insert(w.text.clone());
        }

        if extreme {
            self.extreme_decl(d, path, &at);
            return;
        }

        // The range, which is required and is what the sequence is held to. A count is a
        // plain number; a sum keeps the unit of the column it adds up.
        let ty = if summing {
            self.syms.get(&d.column.text).map(|s| s.ty.clone()).unwrap_or(Ty::Number)
        } else {
            Ty::Number
        };
        let bounds = d.range.as_ref().map(|r| bounds_of(r, &ty).0);
        match bounds {
            Some((Some(lo), Some(hi))) if lo.cmp_to(Rat::zero()) != std::cmp::Ordering::Less => {
                self.ranges.insert(d.name.text.clone(), (Some(lo), Some(hi)));
            }
            other => {
                let what = match other {
                    None => tr!("範囲がありません", "there is no range"),
                    Some((_, None)) => tr!("上限がありません", "there is no upper bound"),
                    Some((None, _)) => tr!("下限がありません", "there is no lower bound"),
                    Some(_) => tr!("下限が負です", "the lower bound is negative"),
                };
                self.diags.push(
                    Diag::error("E030", tr!("`{}` に範囲が要ります", "A `{}` needs a range", if summing { crate::kw::SUM } else { crate::kw::COUNT }))
                        .at(at.clone())
                        .mark(d.span.clone(), what)
                        .note(tr!(
                            "`range >=0 <=100` の形で書いてください。この範囲には二つの役割があります。数えた結果を列に使ったときに完全性の検査が見る値の全体であり、並びの長さの上限でもあります。生成コードは、これより長い並びを入口で受け付けません。",
                            "Write it as `range >=0 <=100`. The range means two things: the universe the completeness check quantifies over once the count is a column, and **the cap on the sequence** — the generated code refuses a longer one at the door."
                        )),
                );
                self.ranges.insert(d.name.text.clone(), (Some(Rat::zero()), None));
            }
        }
        self.scales.insert(d.name.text.clone(), self.scales.get(&d.column.text).copied().filter(|_| summing).unwrap_or(1));
        self.syms.insert(
            d.name.text.clone(),
            Sym { ty, span: d.name.span.clone(), kind: SymKind::Derived, contract_only: false },
        );
    }

    /// The value a test on a column of one element looks for (`count … where <column> = <value>`,
    /// the `where` of `max` and `min`): a bool needs none, which is what makes `where 会社名一致`
    /// read the way it is meant to; an enum's has to be one of its values.
    fn test_value(&mut self, col: &str, sp: &Span, value: &Option<Name>, ty: &Ty, bad: &dyn Fn(&mut Self, &Span, String, String)) {
        match (value, ty) {
            (None, Ty::Bool) => {}
            (None, _) => bad(
                self,
                sp,
                tr!("どの値を数えるのかが書かれていません", "the value to count is missing"),
                tr!(
                    "`where {col} = <値>` と書いてください。値を書かなくていいのは真偽の列だけです。",
                    "Write `where {col} = <value>`. Only a bool column may leave it out."
                ),
            ),
            (Some(v), Ty::Bool) => {
                if v.text != crate::kw::TRUE && v.text != crate::kw::FALSE {
                    bad(
                        self,
                        sp,
                        tr!("{} は真偽の値ではありません", "{} is not a boolean", v.text),
                        tr!("書けるのは `true` か `false` です。", "Write `true` or `false`."),
                    );
                }
            }
            (Some(v), Ty::Enum(en)) => {
                let ok = self.enums.get(en).is_some_and(|vs| vs.contains(&v.text));
                if !ok {
                    bad(
                        self,
                        sp,
                        tr!("{} は列挙 {en} の値ではありません", "{} is not a value of the enum {en}", v.text),
                        tr!("その列挙の値を一つ書いてください。", "Name one of that enum's values."),
                    );
                } else {
                    self.used_values.insert((en.clone(), v.text.clone()));
                }
            }
            _ => {}
        }
    }

    /// What `max` and `min` answer (§15.219): a value of the column, or the `empty` value, which is
    /// required (E022) and has to be a value of the column's type. The answer's range is the
    /// column's with that value added — the column's values are held to its range by the door, or
    /// worked out from it — and a range written on the line has to hold it (E112). An ordered enum
    /// has no range of numbers: its answer is one of its values.
    fn extreme_decl(&mut self, d: &AggDecl, path: &str, at: &str) {
        let word = d.kind.word();
        let ty = self.syms.get(&d.column.text).map(|s| s.ty.clone()).unwrap_or(Ty::Unknown);
        let scale = self.scales.get(&d.column.text).copied().unwrap_or(1);
        let empty: Option<Rat> = match &d.empty {
            None => {
                self.diags.push(
                    Diag::error("E022", tr!("要素がゼロ件のときの答えが宣言されていません", "The answer for a sequence with no elements is not declared"))
                        .at(at.to_string())
                        .mark(d.span.clone(), tr!("`empty <値>` がありません", "there is no `empty <value>`"))
                        .note(tr!(
                            "`{word}` は、見る要素が一つも無いときに何を答えるかを書いてください。並びが空のときと、`where` に当てはまる要素が無いときです。何を答えるかは業務の判断で、ツールが決められることではありません。",
                            "Say what `{word}` answers when no element is looked at: when the sequence is empty, and when no element passes the `where`. What to answer is a business decision, and not one the tool can make."
                        )),
                );
                None
            }
            Some((l, sp)) => {
                let before = self.diags.len();
                self.cell(&Cell::Lit(l.clone()), &ty, scale, sp, at, false);
                if self.diags.len() > before {
                    None
                } else {
                    match (l, ty.present()) {
                        (Lit::Num(n), t) => lit_value_in(n, t),
                        (Lit::Date(y, m, dd), Ty::Date) => Some(date_ord(*y, *m, *dd)),
                        (Lit::Word(w), Ty::Enum(en)) => {
                            self.used_values.insert((en.clone(), w.clone()));
                            None
                        }
                        _ => None,
                    }
                }
            }
        };
        if ty.is_numeric() || ty == Ty::Date {
            // The column's range, and the `empty` value beside it.
            let (cl, ch) = self.ranges.get(&d.column.text).copied().unwrap_or((None, None));
            let lo = match (cl, empty) {
                (Some(a), Some(e)) => Some(if e.cmp_to(a) == std::cmp::Ordering::Less { e } else { a }),
                (a, _) => a,
            };
            let hi = match (ch, empty) {
                (Some(a), Some(e)) => Some(if e.cmp_to(a) == std::cmp::Ordering::Greater { e } else { a }),
                (a, _) => a,
            };
            let mut range = (lo, hi);
            if let Some(rg) = &d.range {
                let ((dl, dh), bad) = bounds_of(rg, &ty);
                for n in bad {
                    self.diags.push(
                        Diag::error("E103", tr!("範囲の `{}` は {ty} の値ではありません", "The bound `{}` is not a value of {ty}", n.raw))
                            .at(at.to_string())
                            .mark(d.name.span.clone(), tr!("この宣言の範囲", "the range on this declaration"))
                            .maybe_note(unit_note(&n)),
                    );
                }
                let too_low = matches!((dl, lo), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Less) || (dl.is_some() && lo.is_none());
                let too_high = matches!((dh, hi), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Greater) || (dh.is_some() && hi.is_none());
                if too_low || too_high {
                    let shown = |v: Option<Rat>| v.map(|x| fmt_val(x, &ty)).unwrap_or_else(|| "…".to_string());
                    let mut e = Diag::error(
                        "E112",
                        tr!("`{word}` の範囲が、実際に取りうる値を含んでいません", "The range of a `{word}` does not contain the values it can actually take"),
                    )
                    .at(tr!("{path}:{} {word} {}", "{path}:{} {word} {}", rg.span.line, d.name.text))
                    .mark(rg.span.clone(), tr!("実際に取りうる値は >={} <={} です", "the reachable interval is >={} <={}", shown(lo), shown(hi)))
                    .note(tr!(
                        "`{word}` の答えは、{} のどれかの値か、`empty` の値です。範囲が狭いと、完全性の検査は、実際に起きる値を見ないまま「完全」と判定します。",
                        "What a `{word}` answers is a value of {}, or the `empty` value. With a range that is too narrow, the completeness check answers \"complete\" without ever seeing the values that actually occur.",
                        d.column.text
                    ));
                    if let (Some(l), Some(h)) = (lo, hi) {
                        e = e.fix(crate::diag::FixKind::WidenRange, format!("{} >={} <={}", crate::kw::RANGE, fmt_val_fixed(l, &ty), fmt_val_fixed(h, &ty)));
                    }
                    self.diags.push(e);
                } else {
                    // A written range that holds the answer is the universe, as any declared one is.
                    range = (dl.or(lo), dh.or(hi));
                }
            }
            if range.0.is_some() || range.1.is_some() {
                self.ranges.insert(d.name.text.clone(), range);
            }
        } else if let Some(rg) = &d.range {
            self.diags.push(
                Diag::error("E069", tr!("`{word}` {} の範囲は書けません", "A `{word}` {} takes no range", d.name.text))
                    .at(at.to_string())
                    .mark(rg.span.clone(), tr!("{} は {ty} です", "{} is {ty}", d.column.text))
                    .note(tr!(
                        "列挙の `{word}` は、その列挙の値のどれかを答えます。範囲は、数と日付の列にだけ書けます。",
                        "A `{word}` of an enum answers one of the enum's values. Only a column of numbers or dates takes a range."
                    )),
            );
        }
        self.scales.insert(d.name.text.clone(), scale);
        if let Some(g) = self.grids.get(&d.column.text).copied() {
            self.grids.insert(d.name.text.clone(), g);
        }
        self.syms.insert(d.name.text.clone(), Sym { ty, span: d.name.span.clone(), kind: SymKind::Derived, contract_only: false });
    }

    /// The type a fold answers with (§15.219): the name after its `->` — an output's, or one it
    /// declares with a type — or the rule's first output.
    pub(crate) fn fold_ty(&self, fold: &FoldDecl, f: &RuleFile) -> Ty {
        match fold.answer_name(f) {
            Some(n) => self.syms.get(n).map(|s| s.ty.clone()).unwrap_or(Ty::Unknown),
            None => Ty::Unknown,
        }
    }

    /// `fold <verdict> over <sequence> [-> <name> [: <type>]]` (§15.56, §15.219), where it is
    /// written.
    ///
    /// The table decides one element at a time and its verdict column is a finite enum, so the
    /// walk is a reduction of a string over a finite alphabet. What has to be checked is that the
    /// automaton has no holes: an arm for every verdict the table can produce, and the two
    /// answers that belong to no element at all. The answer goes to the name after `->`: an output
    /// of the rule, or a value of its own with the type written there, which the items below can
    /// read like any other (E021 for a name that is neither, or one something else decides).
    fn fold_decl(&mut self, fold: &FoldDecl, f: &RuleFile, path: &str) {
        let at_fold = tr!("{path}:{} 畳み込み", "{path}:{} fold", fold.span.line);
        let fold_bad = |s: &mut Self, sp: &Span, title: String, mark: String, note: String| {
            s.diags.push(Diag::error("E021", title).at(at_fold.clone()).mark(sp.clone(), mark).note(note));
        };
        // Where the answer goes.
        if let Some(FoldAnswer { name, ty }) = &fold.answer {
            let output = f.outputs.iter().any(|o| o.name.text == name.text);
            match (self.syms.get(&name.text).cloned(), ty) {
                (Some(sym), Some(t)) if output => {
                    let want = self.resolve(t);
                    if want != sym.ty && sym.ty != Ty::Unknown && want != Ty::Unknown {
                        fold_bad(
                            self,
                            &t.span,
                            tr!("出力 {} は {} です", "Output {} is {}", name.text, sym.ty),
                            tr!("{want} と書かれています", "{want} is written here"),
                            tr!(
                                "出力の名前に答えるなら、型は出力の宣言のとおりです。`: <型>` を消すか、出力の型にそろえてください。",
                                "An answer that goes to an output takes the output's type. Drop the `: <type>`, or make it the output's."
                            ),
                        );
                    }
                }
                (Some(_), _) if output => {}
                (Some(sym), _) => {
                    let what = match sym.kind {
                        SymKind::Input => tr!("入力", "an input"),
                        SymKind::Element => tr!("要素のフィールド", "a field of an element"),
                        SymKind::Derived | SymKind::Define => tr!("上で宣言した値", "a value declared above"),
                        SymKind::TableOut => tr!("表の出力", "a table's output"),
                        SymKind::Output => tr!("出力", "an output"),
                    };
                    fold_bad(
                        self,
                        &name.span,
                        tr!("`{}` はすでに{what}です", "`{}` is already {what}", name.text),
                        tr!("この畳み込みの答えの名前です", "this is the name of the fold's answer"),
                        tr!(
                            "畳み込みの答えには、出力の名前か、まだ使っていない名前を付けてください。新しい名前なら `-> <名前>(<別名>) : <型>` と型も書きます。",
                            "Give a fold's answer the name of an output, or a name not yet used; a new one takes its type too: `-> <name>(<alias>) : <type>`."
                        ),
                    );
                }
                (None, None) => fold_bad(
                    self,
                    &name.span,
                    tr!("`{}` の型が書かれていません", "`{}` is not given a type", name.text),
                    tr!("出力の名前ではありません", "this is no output's name"),
                    tr!(
                        "出力でない名前に答えるなら、型も書いてください: `-> {}(<別名>) : <型>`。",
                        "An answer that goes to a name that is no output takes a type: `-> {}(<alias>) : <type>`.",
                        name.text
                    ),
                ),
                (None, Some(t)) => {
                    let ty = self.resolve(t);
                    if let Some(d) = missing_step(name, t, ty.present(), name.span.line, path) {
                        self.diags.push(d);
                    }
                    self.scales.insert(name.text.clone(), scale_of_type(t, ty.present()));
                    self.syms.insert(name.text.clone(), Sym { ty, span: name.span.clone(), kind: SymKind::Derived, contract_only: false });
                }
            }
        }
        let out_ty = self.fold_ty(fold, f);
        if f.sequence_named(&fold.over).is_none() {
            self.diags.push(
                Diag::error("E021", tr!("`over` が指す列がありません", "`over` names a sequence that is not declared"))
                    .at(at_fold.clone())
                    .mark(fold.span.clone(), tr!("{} は宣言されていません", "{} is not declared", fold.over))
                    .note(tr!(
                        "たどる並びは `elements <名前>(<別名>)` で宣言し、その下に一要素ぶんのフィールドを書いてください。",
                        "Declare the sequence with `elements <name>(<alias>)`, and the fields of one element inside it."
                    )),
            );
        }
        // The verdicts the table can actually produce. A fold reads a column some table
        // writes, and that column's type is what makes the walk finite.
        let produced: Vec<String> = self.out_values.get(&fold.verdict).cloned().unwrap_or_default();
        let known = self.syms.get(&fold.verdict).cloned();
        match &known {
            Some(sym) if matches!(sym.ty, Ty::Enum(_)) => {}
            Some(sym) => self.diags.push(
                Diag::error("E021", tr!("畳めるのは列挙の列だけです", "Only a column of an enum can be folded"))
                    .at(at_fold.clone())
                    .mark(fold.span.clone(), tr!("{} は {} です", "{} is {}", fold.verdict, sym.ty))
                    .note(tr!(
                        "畳み込みが検査できるのは、判定が有限の値のどれかに落ちるからです。数や金額の列は畳めません。",
                        "A fold is checkable because each element lands on one of finitely many verdicts. A column of numbers or money is not one of them."
                    )),
            ),
            None => self.diags.push(
                Diag::error("E012", tr!("`{}` という列はありません", "There is no column called `{}`", fold.verdict))
                    .at(at_fold.clone())
                    .mark(fold.span.clone(), tr!("どの表もこの列を出していません", "no table produces this column")),
            ),
        }
        self.used.insert(fold.verdict.clone());
        // Everything the arms and the two answers read is read (§11 W111). An element's field
        // that only ever appears in `take_unique 行運賃` is used by the rule, and saying it is
        // not would send the author to delete the value the walk answers with.
        {
            let mut names = Vec::new();
            for e in fold.empty.iter().chain(fold.exhausted.iter()) {
                collect_names(e, &mut names);
            }
            for (_, arm, _) in &fold.arms {
                match arm {
                    Arm::Stop(Some(x)) => collect_names(x, &mut names),
                    Arm::Take { expr, .. } => collect_names(expr, &mut names),
                    Arm::KeepMax { expr, key } => {
                        collect_names(expr, &mut names);
                        collect_names(key, &mut names);
                    }
                    _ => {}
                }
            }
            for n in names {
                self.used.insert(n);
            }
        }

        // A literal answer is a value of the output the walk produces. It was never read
        // against that type, so `empty -> 999銭` under a `money[円]` output passed `check`
        // and generated 99900 — nine hundred and ninety-nine yen written as 9.99, kept as a
        // count of 銭, and answered as 99900円 (§15.86). The same literal in a table cell of
        // that column is E103, and has always been.
        if out_ty != Ty::Unknown {
            let mut answers: Vec<(&Expr, &Span)> = Vec::new();
            for e in fold.empty.iter().chain(fold.exhausted.iter()) {
                answers.push((e, &fold.span));
            }
            for (_, arm, sp) in &fold.arms {
                match arm {
                    Arm::Stop(Some(x)) => answers.push((x, sp)),
                    Arm::Take { expr, .. } => answers.push((expr, sp)),
                    Arm::KeepMax { expr, .. } => answers.push((expr, sp)),
                    _ => {}
                }
            }
            for (e, sp) in answers {
                if let Expr::Lit(l, lsp) = e {
                    let at_l = at_fold.clone();
                    let where_ = if lsp.len > 0 { lsp.clone() } else { sp.clone() };
                    self.out_lit(l, &out_ty, &where_, &at_l);
                }
                let Expr::Lit(Lit::Num(n), lsp) = e else { continue };
                if lit_value_in(n, &out_ty).is_none() {
                    let note = match &fold.answer {
                        None => tr!(
                            "畳み込みの答えは、この規則の出力そのものです。表のセルと同じ単位で書いてください。",
                            "What a fold answers is the rule's output itself, and is written in the same unit its table cells are."
                        ),
                        Some(a) => tr!(
                            "畳み込みの答えは {} の値です。その型の単位で書いてください。",
                            "What a fold answers is the value of {}, and is written in the unit of its type.",
                            a.name.text
                        ),
                    };
                    self.diags.push(
                        Diag::error("E103", tr!("この畳み込みの答えは {out_ty} ですが `{}` が書かれています", "This fold answers with {out_ty}, but `{}` is written here", n.raw))
                            .at(at_fold.clone())
                            .mark(if lsp.len > 0 { lsp.clone() } else { sp.clone() }, "")
                            .maybe_note(unit_note(n))
                            .note(note),
                    );
                }
            }
        }

        // (1) and (2): the two answers that belong to no element. Declaring them is not
        // optional, because a walk over nothing and a walk that ran out are exactly the two
        // cases a hand-written loop forgets.
        if fold.empty.is_none() {
            self.diags.push(
                Diag::error("E022", tr!("要素がゼロ件のときの答えが宣言されていません", "The answer for a sequence with no elements is not declared"))
                    .at(at_fold.clone())
                    .mark(fold.span.clone(), tr!("`empty -> <値>` がありません", "there is no `empty -> <value>`"))
                    .note(tr!(
                        "空の並びは必ず来ます。来たときに何を返すかは業務の判断で、ツールが決められることではありません。",
                        "An empty sequence will arrive. What to answer then is a business decision, and not one the tool can make."
                    )),
            );
        }
        if fold.exhausted.is_none() {
            self.diags.push(
                Diag::error("E023", tr!("最後まで見終えたときの答えが宣言されていません", "The answer for a walk that reached the end is not declared"))
                    .at(at_fold.clone())
                    .mark(fold.span.clone(), tr!("`exhausted -> <値>` がありません", "there is no `exhausted -> <value>`"))
                    .note(tr!(
                        "どの要素でも打ち切らずに最後まで見たときの答えです。保持している値を返すなら `exhausted -> {}` と書いてください。",
                        "It is the answer when no element ended the walk. To answer with what is held, write `exhausted -> {}`.",
                        crate::kw::HELD
                    )),
            );
        }

        // (3): every verdict the table can produce has an arm, and no arm waits for a verdict
        // the table cannot produce. This is the fold's own completeness, and it is decided the
        // same way the table's is: by comparing two finite sets.
        let armed: Vec<String> = fold.arms.iter().map(|(n, _, _)| n.text.clone()).collect();
        let missing: Vec<&String> = produced.iter().filter(|v| !armed.contains(v)).collect();
        if !missing.is_empty() && !produced.is_empty() {
            let names = missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
            self.diags.push(
                Diag::error("E024", tr!("行き先の無い判定があります: {names}", "These verdicts have no arm: {names}"))
                    .at(at_fold.clone())
                    .mark(fold.span.clone(), tr!("表はこの判定を出しますが、畳み込みに扱いがありません", "the table produces them and the fold has nothing to do with them"))
                    .note(tr!(
                        "その判定の要素が来たとき、次にどうするかが決まっていません。行き先を足すか、表がその値を出さないようにしてください。",
                        "When an element lands on one of them the walk has no move. Add an arm, or stop the table producing the value."
                    )),
            );
        }
        for (n, _, sp) in &fold.arms {
            if !produced.is_empty() && !produced.contains(&n.text) {
                self.diags.push(
                    Diag::warning("W115", tr!("どの要素もこの判定にはなりません: {}", "No element can land on this verdict: {}", n.text))
                        .at(at_fold.clone())
                        .mark(sp.clone(), tr!("表がこの値を出しません", "the table does not produce this value"))
                        .note(tr!(
                            "行き先は書かれていますが、届きません。表の行を見直すか、この行き先を消してください。",
                            "The arm is written but unreachable. Look again at the table's rows, or drop the arm."
                        )),
                );
            }
        }

        // What the answer can be, for the items below that read it (§15.219): one of the values
        // the fold answers with, so the hull of theirs, as a declared range bounds a `sum`.
        // Without it a `define` over the answer had no interval, and its int64 claim was never
        // made. Only an answer something reads gets one, so a rule whose fold answers its first
        // output and nothing more keeps the certificate it had.
        if let Some(name) = fold.answer_name(f) {
            let read = f.items.iter().any(|it| item_reads(it).iter().any(|n| n == name))
                || f.result.as_ref().is_some_and(|r| {
                    let mut ns = Vec::new();
                    collect_names(&r.expr, &mut ns);
                    ns.iter().any(|n| n == name)
                });
            let ty = self.syms.get(name).map(|s| s.ty.present().clone()).unwrap_or(Ty::Unknown);
            if read && (ty.is_numeric() || ty == Ty::Date) {
                let mut hull: Option<(Rat, Rat)> = None;
                let mut known = true;
                for e in fold.answer_values() {
                    let iv = match e {
                        Expr::Lit(Lit::Date(y, m, d), _) => Some((date_ord(*y, *m, *d), date_ord(*y, *m, *d))),
                        _ => self.interval(e, &ty),
                    };
                    match iv {
                        Some((lo, hi)) => {
                            hull = Some(match hull {
                                None => (lo, hi),
                                Some((a, b)) => (crate::grid::min_rat(a, lo), crate::grid::max_rat(b, hi)),
                            });
                        }
                        None => known = false,
                    }
                }
                if let (true, Some((lo, hi))) = (known, hull) {
                    self.ranges.insert(name.to_string(), (Some(lo), Some(hi)));
                }
            }
        }
    }

    fn table(&mut self, t: &Table, path: &str) {
        let at = |line: usize| match &t.name {
            Some(n) if t.clause => tr!("{path}:{line} 節 {}", "{path}:{line} clause {}", n.text),
            Some(n) => tr!("{path}:{line} 表 {}", "{path}:{line} table {}", n.text),
            None => tr!("{path}:{line} 例", "{path}:{line} examples"),
        };

        // A label names a row for `overrides` lines, traces and later versions, so two rows
        // of one table cannot share one (E034).
        let mut seen: HashMap<&str, &Span> = HashMap::new();
        for row in &t.rows {
            let Some(l) = &row.label else { continue };
            match seen.get(l.text.as_str()) {
                Some(first) => self.diags.push(
                    Diag::error("E034", tr!("行ラベル `{}` が二度あります", "The row label `{}` appears twice", l.text))
                        .at(at(row.span.line))
                        .mark((*first).clone(), tr!("最初の行", "the first row"))
                        .mark(l.span.clone(), tr!("同じラベル", "the same label"))
                        .note(tr!(
                            "ラベルは、一つの表の中で重ならないようにしてください。`{}` の行と記録は、ラベルで行を指します。どちらかのラベルを変えてください。",
                            "A label is unique within its table: `{}` lines and records refer to the row by it. Change one of the two.",
                            crate::kw::OVERRIDES
                        )),
                ),
                None => {
                    seen.insert(&l.text, &l.span);
                }
            }
        }

        // Column headers must name something already in scope (§5.1).
        let mut col_ty: Vec<Ty> = Vec::new();
        for (name, sp) in &t.inputs {
            self.used.insert(name.clone());
            match self.syms.get(name) {
                Some(s) => col_ty.push(s.ty.clone()),
                None => {
                    if self.groups.contains_key(name) || self.enums.contains_key(name) {
                        col_ty.push(Ty::Unknown);
                    } else {
                        self.diags.push(
                            Diag::error("E012", tr!("列の名前 `{name}` は宣言されていません", "Column `{name}` is not a declared name"))
                                .at(at(sp.line))
                                .mark(sp.clone(), "")
                                .note(tr!("列に書けるのは 入力・導出・真偽/列挙の中間値です。", "A column can only be an input, a derived value, or a boolean/enum intermediate value.")),
                        );
                        col_ty.push(Ty::Unknown);
                    }
                }
            }
        }

        // Output columns enter scope for later tables and for `result`.
        let mut out_ty: Vec<Ty> = Vec::new();
        for (oi, oc) in t.outputs.iter().enumerate() {
            let ty = match &oc.ty {
                Some(tr) => self.resolve(tr),
                None => self.syms.get(&oc.name.text).map(|s| s.ty.clone()).unwrap_or(Ty::Unknown),
            };
            out_ty.push(ty.clone());
            if t.name.is_some() {
                // A column's storage scale, which is fixed by what its cells hold: the
                // coarsest grid every literal sits on, and the scale of anything a cell
                // names. Without this a later `define` that multiplies by a rate column
                // read that rate as if it were whole units, and the generated code came
                // back a factor of the rate's step too large (§15.16).
                if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number) {
                    let mut sc: i128 = 1;
                    for row in &t.rows {
                        match row.outs.get(oi) {
                            Some(OutCell::Lit(Lit::Num(n))) => {
                                if let Some(v) = lit_value_in_pub(n, &ty) {
                                    sc = lcm_i128(sc, v.den);
                                }
                            }
                            Some(OutCell::Name(w)) => {
                                sc = lcm_i128(sc, *self.scales.get(w).unwrap_or(&1));
                            }
                            _ => {}
                        }
                    }
                    // Another table may define this output too (a definition set); the
                    // column's scale is the coarsest grid every one of them sits on.
                    let sc = lcm_i128(sc, *self.scales.get(&oc.name.text).unwrap_or(&1));
                    // And the declared rounding decides it too, because the grid has to be a
                    // whole number of storage units. `rate[step 1%] round down(1%)` held
                    // 0%, 20%, 40% and 45%, whose reduced denominators are twentieths, so
                    // one percent came out as a grid of **zero** and the generated code
                    // divided by it — the first rule with a rate output found it (§15.9
                    // again: what nothing exercises is not checked).
                    let sc = match self.roundings.get(&oc.name.text) {
                        Some((_, g)) => lcm_i128(sc, g.den),
                        None => sc,
                    };
                    self.scales.insert(oc.name.text.clone(), sc);

                    // The column's range, likewise fixed by its cells: the smallest and the
                    // largest literal, widened by the range of anything a cell names. A cell
                    // that names something without a range leaves the column without one —
                    // no range rather than a guessed one. A rate column used to be assumed to
                    // lie within 0..100%, and a column holding 150% put a wrong bound into the
                    // overflow proof and into `rulec api` (§15.34).
                    //
                    // A cell that names one of the table's own columns writes only what the row's
                    // cell on that column lets through: `| >=1 | units |` writes `units` at one
                    // or more. The column is narrowed to that, and remembered as narrowed, only
                    // where it changes the column's range; the certificate then says so on each
                    // such row (§15.213).
                    let mut bounds: Vec<(Rat, Rat)> = Vec::new();
                    let mut plain: Vec<(Rat, Rat)> = Vec::new();
                    let mut known = true;
                    for row in &t.rows {
                        match row.outs.get(oi) {
                            Some(OutCell::Lit(Lit::Num(n))) => {
                                match lit_value_in(n, &ty).or_else(|| lit_value_in(n, &lit_ty(n))) {
                                    Some(v) => {
                                        bounds.push((v, v));
                                        plain.push((v, v));
                                    }
                                    None => known = false,
                                }
                            }
                            Some(OutCell::Name(w)) => match self.ranges.get(w) {
                                Some((Some(a), Some(b))) => {
                                    bounds.push(self.row_written(t, row, oi).unwrap_or((*a, *b)));
                                    plain.push((*a, *b));
                                }
                                _ => known = false,
                            },
                            _ => known = false,
                        }
                    }
                    if known && !bounds.is_empty() {
                        let hull = |bs: &[(Rat, Rat)]| {
                            let lo = bs.iter().map(|b| b.0).min_by(|a, b| a.cmp_to(*b)).unwrap();
                            let hi = bs.iter().map(|b| b.1).max_by(|a, b| a.cmp_to(*b)).unwrap();
                            (lo, hi)
                        };
                        let (mut lo, mut hi) = hull(&bounds);
                        let (pl, ph) = hull(&plain);
                        if lo.cmp_to(pl) != std::cmp::Ordering::Equal || hi.cmp_to(ph) != std::cmp::Ordering::Equal {
                            self.narrowed.insert(oc.name.text.clone());
                        }
                        // Widened by what another table of the same output already put there.
                        if let Some((Some(a), Some(b))) = self.ranges.get(&oc.name.text) {
                            if a.cmp_to(lo) == std::cmp::Ordering::Less {
                                lo = *a;
                            }
                            if b.cmp_to(hi) == std::cmp::Ordering::Greater {
                                hi = *b;
                            }
                        }
                        self.ranges.insert(oc.name.text.clone(), (Some(lo), Some(hi)));
                    } else if !known {
                        self.ranges.remove(&oc.name.text);
                    }
                }
                self.syms.insert(
                    oc.name.text.clone(),
                    Sym { ty, span: oc.name.span.clone(), kind: SymKind::TableOut, contract_only: false },
                );
            }
        }

        // Collect the values the upstream table can produce. Only enums and booleans reach a
        // downstream column (§5.3). A cell that names one of the table's own columns passes on
        // what that column holds in the row, and a name of anything else whatever its type
        // holds (`row_writes`, §15.208).
        if t.name.is_some() {
            for (oi, oc) in t.outputs.iter().enumerate() {
                if !matches!(out_ty.get(oi), Some(Ty::Enum(_)) | Some(Ty::Bool)) {
                    continue;
                }
                let mut vs: Vec<String> = Vec::new();
                for ri in 0..t.rows.len() {
                    for w in self.row_writes(t, ri, oi).unwrap_or_default() {
                        if !vs.contains(&w) {
                            vs.push(w);
                        }
                    }
                }
                if !vs.is_empty() {
                    // Joined with the values another table of the same output produces.
                    let e = self.out_values.entry(oc.name.text.clone()).or_default();
                    for v in vs {
                        if !e.contains(&v) {
                            e.push(v);
                        }
                    }
                }
            }
        }

        for row in &t.rows {
            for (ci, cell) in row.cells.iter().enumerate() {
                let Some(want) = col_ty.get(ci) else { continue };
                let sp = row.cell_spans.get(ci).unwrap_or(&row.span).clone();
                let sc = t
                    .inputs
                    .get(ci)
                    .map(|(n, _)| *self.scales.get(n).unwrap_or(&1))
                    .unwrap_or(1);
                self.cell(cell, want, sc, &sp, &at(row.span.line), !t.name.is_none());
            }
            for (oi, oc) in row.outs.iter().enumerate() {
                let Some(want) = out_ty.get(oi) else { continue };
                let osp = row.out_spans.get(oi).unwrap_or(&row.span).clone();
                let ocol = t.outputs.get(oi).map(|o| o.name.text.clone()).unwrap_or_default();
                if let OutCell::Lit(l) = oc {
                    self.out_lit(l, want, &osp, &at(row.span.line));
                }
                match oc {
                    OutCell::Lit(Lit::Num(n)) => match lit_value_in(n, want) {
                        None => self.diags.push(
                            Diag::error("E103", tr!("この列は {want} ですが `{}` が書かれています", "This column is {want}, but `{}` is written here", n.raw))
                                .at(at(row.span.line))
                                .mark(osp.clone(), "")
                                .maybe_note(unit_note(n)),
                        ),
                        Some(v) => {
                            // §2.4: a tool that silently snaps to the declared rounding defeats
                            // itself as a tool for reviewing diffs. If the value is off the grid,
                            // let the author decide.
                            if let Some((m, g)) = self.roundings.get(&ocol).copied() {
                                if !v.on_grid(g) {
                                    let near = v.round_to(m, g);
                                    self.diags.push(
                                        Diag::error("E106", tr!("`{}` は丸めの刻みに載っていません", "`{}` is not on the rounding grid", n.raw))
                                            .at(at(row.span.line))
                                            .mark(osp.clone(), "")
                                            .note(tr!("出力 {ocol} の丸めは {}({}) です。", "The rounding of output {ocol} is {}({}).", m.name(), fmt_val(g, want)))
                                            .note(tr!("ヒント: {} と書くか、丸めの宣言のほうを直してください。", "Hint: write {} instead, or fix the rounding declaration.", fmt_val(near, want)))
                                            .note(tr!("rulec は黙って値を寄せません。どちらが正しいかは業務の判断です。", "Nothing is snapped silently. Which one is right is a business decision.")),
                                    );
                                }
                            }
                        }
                    },
                    // A word the type of the column reads as a value (`read_words`, §15.208): a
                    // value of the column's enum that a name of another type is spelled like.
                    OutCell::Lit(Lit::Word(w)) => {
                        if w == crate::kw::TRUE || w == crate::kw::FALSE {
                            if !matches!(want.present(), Ty::Bool | Ty::Unknown) {
                                self.diags.push(
                                    Diag::error("E103", tr!("この列は {want} ですが 真偽 が書かれています", "This column is {want}, but a boolean is written here"))
                                        .at(at(row.span.line))
                                        .mark(osp.clone(), ""),
                                );
                            }
                        } else if let Some(e) = self.enum_of_value_in(w, want) {
                            self.used_values.insert((e.clone(), w.clone()));
                            self.value_of_other_enum(w, &e, want, &at(row.span.line), &osp);
                        }
                    }
                    OutCell::Name(w) => {
                        // An enum value, or (§3.2) the name of an input / derived / define.
                        if let Some(s) = self.syms.get(w).cloned() {
                            self.used.insert(w.clone());
                            // A name still here that is also a value of the column's enum is the
                            // cell E075 stops on (`read_words`): it names the value as much as
                            // the input, so W111 does not add that the value appears nowhere.
                            if let Some(e) = self.enum_of_value_in(w, want) {
                                self.used_values.insert((e.clone(), w.clone()));
                            }
                            if !s.ty.unifies(want) {
                                self.diags.push(
                                    Diag::error("E103", tr!("この列は {want} ですが `{w}` は {} です", "This column is {want}, but `{w}` is {}", s.ty))
                                        .at(at(row.span.line))
                                        .mark(osp.clone(), ""),
                                );
                            }
                        } else if let Some(e) = self.enum_of_value_in(w, want) {
                            self.used_values.insert((e.clone(), w.clone()));
                            self.value_of_other_enum(w, &e, want, &at(row.span.line), &osp);
                        } else if w != crate::kw::TRUE && w != crate::kw::FALSE {
                            self.diags.push(
                                Diag::error("E012", tr!("`{w}` は値の名前としても、宣言された名前としても見つかりません", "`{w}` is found neither as a value nor as a declared name"))
                                    .at(at(row.span.line))
                                    .mark(osp.clone(), "")
                                    .maybe_note(crate::prelude::hint(w, &self.imported))
                                    .note(tr!("出力セルに書けるのは リテラルか名前（入力・導出・定義）だけです。式は書けません。", "An output cell holds only a literal or a name (an input, derived value, or definition). Expressions are not allowed.")),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The cells of `examples`, held to the types of the columns they sit under.
    ///
    /// `examples` is shaped like a table but is not an `Item`, so it never went through
    /// `table` and **nothing ever looked at its cells**. What that let through:
    ///
    /// - `1lb` under a `mass[g]` column, or a bare `500` where a unit is required, produced
    ///   no value at all. The only thing said was E107 "no value came out", which names the
    ///   output — not the cell that could not be read.
    /// - An enum value that is not one landed on a `-` row, and the example passed while
    ///   proving nothing.
    /// - An expected value in the wrong unit was worse still: `800kg` under a `money[円]`
    ///   output compared equal to 800円. §1.2 calls `examples` an executable specification,
    ///   and this is the one check meant to catch what the evaluator and every generated
    ///   language get wrong together (AGENTS §1); it was answering yes to a unit it had
    ///   never read.
    /// - A column heading that names nothing at all was simply ignored.
    ///
    /// The sequence column of a walk (§15.56) names a `sequence` rather than an input, and
    /// is checked as one by E025 and E027, so it is stepped over here.
    fn example_cells(&mut self, t: &Table, f: &RuleFile, path: &str) {
        let at = |line: usize| tr!("{path}:{line} 例", "{path}:{line} examples");
        self.value_cells(t, f, &at);
    }

    /// The cells of a table of values — `examples`, or a `scenario` (§15.148) — held to the
    /// types of the columns they sit under, with `at` naming where they are.
    fn value_cells(&mut self, t: &Table, f: &RuleFile, at: &dyn Fn(usize) -> String) {
        let seqs: Vec<String> = f.elements.iter().map(|el| el.name.text.clone()).collect();

        // W111 asks whether a **table** uses a declaration, and an example is not a table:
        // an enum value that only an example names still has no row of its own, which is
        // exactly what the warning is for. `cell` marks what it reads as used, so the two
        // sets are put back afterwards.
        let used = self.used.clone();
        let used_values = self.used_values.clone();

        let mut col_ty: Vec<Option<Ty>> = Vec::new();
        for (name, sp) in &t.inputs {
            if seqs.contains(name) {
                col_ty.push(None);
                continue;
            }
            match self.syms.get(name) {
                Some(s) => col_ty.push(Some(s.ty.clone())),
                // A group or an enum used as a column is the table's own shorthand; its
                // cells are values, and `table` treats it the same way.
                None if self.groups.contains_key(name) || self.enums.contains_key(name) => {
                    col_ty.push(None)
                }
                None => {
                    self.diags.push(
                        Diag::error("E012", tr!("列の名前 `{name}` は宣言されていません", "Column `{name}` is not a declared name"))
                            .at(at(sp.line))
                            .mark(sp.clone(), "")
                            .note(tr!("例の列は、入力・導出・表の出力のどれかの名前です。", "A column of the examples names an input, a derived value, or a table's output.")),
                    );
                    col_ty.push(None);
                }
            }
        }

        let mut out_ty: Vec<Option<Ty>> = Vec::new();
        for oc in &t.outputs {
            match self.syms.get(&oc.name.text) {
                Some(s) => out_ty.push(Some(s.ty.clone())),
                None => {
                    self.diags.push(
                        Diag::error("E012", tr!("列 `{}` という名前は宣言されていません", "Column `{}` is not a declared name", oc.name.text))
                            .at(at(oc.name.span.line))
                            .mark(oc.name.span.clone(), "")
                            .note(tr!("`->` の右に書けるのは、この規則の出力の名前です。", "What stands to the right of `->` is the name of one of this rule's outputs.")),
                    );
                    out_ty.push(None);
                }
            }
        }

        for row in &t.rows {
            for (ci, cell) in row.cells.iter().enumerate() {
                let Some(Some(want)) = col_ty.get(ci) else { continue };
                let want = want.clone();
                let sp = row.cell_spans.get(ci).unwrap_or(&row.span).clone();
                let sc = t
                    .inputs
                    .get(ci)
                    .map(|(n, _)| *self.scales.get(n).unwrap_or(&1))
                    .unwrap_or(1);
                self.cell(cell, &want, sc, &sp, &at(row.span.line), !t.name.is_none());
            }
            for (oi, oc) in row.outs.iter().enumerate() {
                let Some(Some(want)) = out_ty.get(oi) else { continue };
                let want = want.clone();
                let osp = row.out_spans.get(oi).unwrap_or(&row.span).clone();
                if let OutCell::Lit(l) = oc {
                    self.out_lit(l, &want, &osp, &at(row.span.line));
                }
                match oc {
                    OutCell::Lit(Lit::Num(n)) if lit_value_in(n, &want).is_none() => {
                        self.diags.push(
                            Diag::error("E103", tr!("この列は {want} ですが `{}` が書かれています", "This column is {want}, but `{}` is written here", n.raw))
                                .at(at(row.span.line))
                                .mark(osp, "")
                                .maybe_note(unit_note(n))
                                .note(tr!(
                                    "期待する値が読めないと、例は規則と突き合わせられません。",
                                    "An expected value that cannot be read leaves nothing to hold the rule to."
                                )),
                        );
                    }
                    // An expected value is a value: the evaluator reads the cell as one, so a value
                    // of the column's enum is that value even where a name of another type is
                    // spelled the same (§15.208).
                    OutCell::Name(w) => {
                        if let Some(e) = self.enum_of_value_in(w, &want) {
                            self.value_of_other_enum(w, &e, &want, &at(row.span.line), &osp);
                        } else if let Some(s) = self.syms.get(w).cloned() {
                            if !s.ty.unifies(&want) {
                                self.diags.push(
                                    Diag::error("E103", tr!("この列は {want} ですが `{w}` は {} です", "This column is {want}, but `{w}` is {}", s.ty))
                                        .at(at(row.span.line))
                                        .mark(osp, ""),
                                );
                            }
                        } else if w != crate::kw::TRUE && w != crate::kw::FALSE {
                            self.diags.push(
                                Diag::error("E012", tr!("`{w}` は値の名前としても、宣言された名前としても見つかりません", "`{w}` is found neither as a value nor as a declared name"))
                                    .at(at(row.span.line))
                                    .mark(osp, "")
                                    .maybe_note(crate::prelude::hint(w, &self.imported)),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        self.used = used;
        self.used_values = used_values;
    }

    /// What the items, the outputs and the answers of the folds read, given the stage each runs
    /// in (§15.208, §15.219).
    ///
    /// An item that reads the elements of two sequences would walk one inside the walk of the
    /// other (E031); so would an output that is a value of one element, which is not one value
    /// once the walk is over. An item that reads a value of one element and what a walk answered
    /// runs in a second walk, after the first, and is not said anything of. A fold's answers are
    /// decided where some values are not there: `empty` and `exhausted` see no element, `held` is
    /// only there once the walk is over, and nothing a fold reads can be worked out from its own
    /// answer (E021). Each answer is read in the type the fold answers with (E012, E103).
    fn walk_stages(&mut self, f: &RuleFile, path: &str) {
        let st = stages(f);
        let at = |line: usize| format!("{path}:{line}");
        let seq_of = |n: &str| st.each.get(n).map(|s| f.elements[*s].name.text.clone()).unwrap_or_default();
        for (i, a, b) in &st.two {
            let (what, sp) = match &f.items[*i] {
                Item::Derived(d) => (tr!("導出 {}", "derived value {}", d.name.text), d.name.span.clone()),
                Item::Define(d) => (tr!("定義 {}", "definition {}", d.name.text), d.name.span.clone()),
                Item::Table(t) => {
                    let n = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                    let sp = t.name.as_ref().map(|n| n.span.clone()).unwrap_or_else(|| t.span.clone());
                    (if t.clause { tr!("節 {n}", "clause {n}") } else { tr!("表 {n}", "table {n}") }, sp)
                }
                Item::Fold(d) => (tr!("畳み込み {}", "fold {}", d.verdict), d.span.clone()),
                Item::Agg(_) => continue,
            };
            let (sa, sb) = (seq_of(a), seq_of(b));
            self.diags.push(
                Diag::error("E031", tr!("一つの項目が、二つの並びの要素を読んでいます", "One item reads the elements of two sequences"))
                    .at(at(sp.line))
                    .mark(sp, tr!("{what} は、{sa} の要素の値 {a} と、{sb} の要素の値 {b} を読んでいます", "{what} reads {a}, a value of one element of {sa}, and {b}, a value of one element of {sb}"))
                    .note(tr!(
                        "要素の値は、その並びをたどっているあいだにしかありません。二つの並びの要素を一緒に読むには、片方の並びの要素ごとに、もう片方を最初からたどることになります（並びの中で並びをたどる）。そうすると歩きの数が並びの長さにつれて増え、検査が終わりません。",
                        "A value of one element exists only while its sequence is walked. Reading the elements of two sequences together would walk one of them again from the start at every element of the other — a walk inside a walk. The number of walks would then grow with the length of a sequence, and the checks could not finish."
                    ))
                    .note(tr!(
                        "片方の並びを `count`・`sum`・`max`・`min`・`fold` で一つの値にしてから、もう片方の項目で読んでください。その項目は、その値が決まったあとの二回目の歩きで計算されます。",
                        "Make one of the sequences one value first — a `count`, a `sum`, a `max`, a `min` or a `fold` — and read that value in the item of the other. The item is then worked out on a second walk, once the value is known."
                    )),
            );
        }
        // The outputs are read once every walk is over. A fold answers its own, and a `result`
        // line is read below.
        let answered: Vec<String> = f.folds().filter_map(|d| d.answer_name(f).map(str::to_string)).collect();
        let per_element = |s: &mut Self, name: &str, sp: &Span, what: String| {
            s.diags.push(
                Diag::error("E031", tr!("出力 {name} は、要素ごとの値です", "Output {name} is a value of one element"))
                    .at(at(sp.line))
                    .mark(sp.clone(), what)
                    .note(tr!(
                        "要素ごとの値は、その要素を見ているあいだにしかありません。並び全体で一つの値にしてから、出力にしてください。",
                        "A value of one element exists only while the walk is on that element. Make it one value for the whole sequence first, and make that the output."
                    )),
            );
        };
        for (oi, od) in f.outputs.iter().enumerate() {
            if answered.contains(&od.name.text) || (oi == 0 && f.result.is_some()) {
                continue;
            }
            if st.each.contains_key(&od.name.text) {
                per_element(self, &od.name.text, &od.name.span, tr!("並びをたどり終えたあとには、一つの値に決まりません", "once the walk is over, it is not one value"));
            }
        }
        if let (Some(r), Some(od)) = (&f.result, f.outputs.first()) {
            let mut names = Vec::new();
            collect_names(&r.expr, &mut names);
            if let Some(n) = names.iter().find(|n| st.each.contains_key(*n)) {
                per_element(self, &od.name.text, &r.span, tr!("`result` が、要素の値 {n} を読んでいます", "`result` reads {n}, a value of one element"));
            }
        }

        // One answer per name: two folds, or a fold and another item, deciding one name leave the
        // answer to whichever runs last (§15.219).
        {
            let mut by: HashMap<String, (usize, Span)> = HashMap::new();
            for (i, it) in f.items.iter().enumerate() {
                for b in item_binds(f, it) {
                    let sp = match it {
                        Item::Fold(d) => d.answer.as_ref().map(|a| a.name.span.clone()).unwrap_or_else(|| d.span.clone()),
                        Item::Table(t) => t.outputs.iter().find(|o| o.name.text == b).map(|o| o.name.span.clone()).unwrap_or_else(|| t.span.clone()),
                        Item::Derived(d) => d.name.span.clone(),
                        Item::Define(d) => d.name.span.clone(),
                        Item::Agg(d) => d.name.span.clone(),
                    };
                    match by.get(&b) {
                        // Tables that define one output are one definition set (§15.66).
                        Some((j, _)) if matches!((&f.items[*j], it), (Item::Table(_), Item::Table(_))) => {}
                        Some((j, first)) if matches!(it, Item::Fold(_)) || matches!(f.items[*j], Item::Fold(_)) => {
                            let fsp = if matches!(it, Item::Fold(_)) { sp.clone() } else { first.clone() };
                            self.diags.push(
                                Diag::error("E021", tr!("`{b}` を決めるところが二つあります", "Two places decide `{b}`"))
                                    .at(at(fsp.line))
                                    .mark(first.clone(), tr!("一つめ", "the first"))
                                    .mark(sp.clone(), tr!("二つめ", "the second"))
                                    .note(tr!(
                                        "畳み込みの答えは、その名前の値そのものです。ほかの項目もその名前を決めると、どちらが答えになるかが決まりません。答えに別の名前を付けてください（`fold … -> <名前>`）。",
                                        "A fold's answer is the value of its name. With another item deciding the same name, which of the two is the answer is not decided. Give the answer a name of its own (`fold … -> <name>`)."
                                    )),
                            );
                        }
                        Some(_) => {}
                        None => {
                            by.insert(b, (i, sp));
                        }
                    }
                }
            }
        }

        for (fi, it) in f.items.iter().enumerate() {
            let Item::Fold(fold) = it else { continue };
            self.fold_reads(f, &st, fi, fold, path);
        }
    }

    /// What one fold's answers read, and the type each is read in (§15.208, §15.219).
    fn fold_reads(&mut self, f: &RuleFile, st: &Stages, fi: usize, fold: &FoldDecl, path: &str) {
        let out_ty = self.fold_ty(fold, f);
        let answer = fold.answer_name(f).unwrap_or_default().to_string();
        let at_fold = tr!("{path}:{} 畳み込み", "{path}:{} fold", fold.span.line);
        let fold_bad = |s: &mut Self, sp: &Span, title: String, note: String| {
            s.diags.push(Diag::error("E021", title).at(at_fold.clone()).mark(sp.clone(), "").note(note));
        };
        // A walk that waits for its own answer (§15.219).
        if let Some((_, back)) = st.cycle.iter().find(|(i, _)| *i == fi) {
            self.diags.push(
                Diag::error("E021", tr!("畳み込みが、自分の答えから計算する {back} を読んでいます", "The fold reads {back}, which is worked out from its own answer"))
                    .at(at_fold.clone())
                    .mark(fold.span.clone(), tr!("答えは {answer} です", "its answer is {answer}"))
                    .note(tr!(
                        "畳み込みの答えは、並びを最後まで見ないと決まりません。その答えから計算する値で要素を判定すると、答えを出す前に答えが要ることになります。",
                        "A fold's answer is known only once the walk is over. Judging an element by a value worked out from that answer would need the answer before there is one."
                    ))
                    .note(tr!(
                        "判定に要るのが別の値なら、その値を読むようにしてください。答えを使う判定は、別の畳み込みにして、二回目の歩きで読んでください（`fold … -> <名前>`）。",
                        "Where the verdict needs another value, read that one. A verdict that does need the answer goes in another fold, which reads it on a second walk (`fold … -> <name>`)."
                    )),
            );
            return;
        }
        // Where an answer is decided: before any element (`empty`), after the last (`exhausted`),
        // or on one element (an arm, and the key of `keep_max`, which is no answer).
        #[derive(PartialEq, Clone, Copy)]
        enum Place {
            Empty,
            Exhausted,
            Arm,
            Key,
        }
        let mut values: Vec<(Place, &Expr)> = Vec::new();
        if let Some(e) = &fold.empty {
            values.push((Place::Empty, e));
        }
        if let Some(e) = &fold.exhausted {
            values.push((Place::Exhausted, e));
        }
        for (_, arm, _) in &fold.arms {
            match arm {
                Arm::Stop(Some(x)) => values.push((Place::Arm, x)),
                Arm::Take { expr, .. } => values.push((Place::Arm, expr)),
                Arm::KeepMax { expr, key } => {
                    values.push((Place::Arm, expr));
                    values.push((Place::Key, key));
                }
                _ => {}
            }
        }
        for (place, e) in values {
            // A word E075 stopped on (`read_words`): a value of the answer's enum and a name of
            // that enum at once. Nothing more is said of it, and its value is not then said to
            // appear nowhere (W111).
            if let (Expr::Name(w, _), Ty::Enum(en), false) = (e, out_ty.present(), place == Place::Key) {
                if self.enum_has(en, w) && self.syms.get(w).is_some_and(|s| matches!(s.ty.present(), Ty::Enum(x) if x == en)) {
                    self.used_values.insert((en.clone(), w.clone()));
                    continue;
                }
            }
            let mut names = Vec::new();
            collect_names(e, &mut names);
            let sp = e.span().clone();
            if place != Place::Exhausted && names.iter().any(|n| n == crate::kw::HELD) {
                fold_bad(
                    self,
                    &sp,
                    tr!("`{h}` は `{x}` の値にだけ書けます", "`{h}` can only be written in the `{x}` answer", h = crate::kw::HELD, x = crate::kw::EXHAUSTED),
                    tr!(
                        "`{}` は、最後まで見終えたときに保持している値です。ほかの値を決める時点では、まだ決まっていません。",
                        "`{}` is what the walk holds once it has seen every element. Where any other answer is decided, it is not decided yet.",
                        crate::kw::HELD
                    ),
                );
                continue;
            }
            let none_seen = match place {
                Place::Empty => Some(tr!("要素ゼロ件のときの答え", "The answer for no elements")),
                Place::Exhausted => Some(tr!("最後まで見終えたときの答え", "The answer for a walk that reached the end")),
                _ => None,
            };
            if let (Some(which), Some(n)) = (&none_seen, names.iter().find(|n| st.each.contains_key(*n))) {
                fold_bad(
                    self,
                    &sp,
                    tr!("{which}は、要素の値 {n} を読めません", "{which} cannot read {n}, a value of one element"),
                    tr!(
                        "この答えは、どの要素も見ていないところで決まります。要素の値を答えにするなら、`take_unique`・`take_first`・`keep_max` で採ってから、`exhausted -> {}` で返してください。",
                        "It is decided where no element is being looked at. To answer with a value of an element, take it with `take_unique`, `take_first` or `keep_max`, and answer `exhausted -> {}`.",
                        crate::kw::HELD
                    ),
                );
                continue;
            }
            if place == Place::Key {
                continue;
            }
            let into = match &fold.answer {
                None => tr!("畳み込みの答えは、この規則の最初の出力に入ります。", "What a fold answers becomes the rule's first output."),
                Some(a) => tr!("畳み込みの答えは {} に入ります。", "What the fold answers becomes {}.", a.name.text),
            };
            // The type the fold answers with. A literal number is read above (E103, §15.86); a
            // word was settled by `read_words` into a value where it is one of this type.
            match e {
                Expr::Lit(Lit::Num(_), _) => {}
                Expr::Lit(Lit::Word(v), lsp) => {
                    let fits = match out_ty.present() {
                        Ty::Bool => v == crate::kw::TRUE || v == crate::kw::FALSE,
                        Ty::Enum(en) => self.enum_has(en, v),
                        Ty::Unknown => true,
                        _ => false,
                    };
                    if let (true, Ty::Enum(en)) = (fits, out_ty.present()) {
                        self.used_values.insert((en.clone(), v.clone()));
                    }
                    if !fits {
                        let note = match &fold.answer {
                            None => tr!(
                                "畳み込みの答えは、この規則の最初の出力そのものです。その型の値を書いてください。",
                                "What a fold answers is the rule's first output itself. Write a value of its type."
                            ),
                            Some(a) => tr!("畳み込みの答えは {} の値です。その型の値を書いてください。", "What a fold answers is the value of {}. Write a value of its type.", a.name.text),
                        };
                        self.diags.push(
                            Diag::error("E103", tr!("この畳み込みの答えは {out_ty} ですが `{v}` が書かれています", "This fold answers with {out_ty}, but `{v}` is written here"))
                                .at(at_fold.clone())
                                .mark(lsp.clone(), "")
                                .note(note),
                        );
                    }
                }
                Expr::Name(w, wsp) if w != crate::kw::HELD && !self.syms.contains_key(w) => {
                    self.diags.push(
                        Diag::error("E012", tr!("`{w}` は、答えの型 {out_ty} の値でも、宣言された名前でもありません", "`{w}` is neither a value of {out_ty}, the type the fold answers with, nor a declared name"))
                            .at(at_fold.clone())
                            .mark(wsp.clone(), "")
                            .maybe_note(crate::prelude::hint(w, &self.imported))
                            .note(tr!("{into}その型の値か、上で宣言した名前を書いてください。", "{into} Write a value of its type, or a name declared above.")),
                    );
                }
                _ => {
                    // `held` is what the walk holds: a value of the type the fold answers with.
                    let held = (place == Place::Exhausted).then(|| {
                        self.syms.insert(
                            crate::kw::HELD.to_string(),
                            Sym { ty: out_ty.clone(), span: fold.span.clone(), kind: SymKind::Derived, contract_only: false },
                        )
                    });
                    let got = self.expr_ty(e, path);
                    if let Some(was) = held {
                        match was {
                            Some(s) => {
                                self.syms.insert(crate::kw::HELD.to_string(), s);
                            }
                            None => {
                                self.syms.remove(crate::kw::HELD);
                            }
                        }
                    }
                    if out_ty != Ty::Unknown {
                        let before = self.diags.len();
                        self.check_same(&out_ty, &got, &sp, path, &tr!("畳み込みの答え", "fold answer"));
                        if self.diags.len() > before {
                            if let Some(d) = self.diags.last_mut() {
                                d.where_ = at_fold.clone();
                            }
                        }
                    }
                }
            }
        }
    }

    /// `machine` and `scenario` (§15.148): the names they use exist and fit.
    ///
    /// The shape of the lines is the parser's (E050). Here: the carried pair is an input and
    /// an output of one enum, and the table it is `over` decides that output (E051); every
    /// state named is a value of that enum, and `never` does not name one state on both sides
    /// (E052); `once` names an output other than the carried one, with a cell that fits it
    /// (E053); the rule does not fold a sequence (E054); and a `scenario` has a machine to
    /// start, names every input but the carried one, and has a name of its own (E055).
    fn machine_decl(&mut self, f: &RuleFile, path: &str) {
        let Some(m) = &f.machine else {
            for sc in &f.scenarios {
                self.diags.push(
                    Diag::error("E055", tr!("`machine` の無い規則に `scenario` は書けません", "A `scenario` needs a `machine`"))
                        .at(tr!("{path}:{} 手順の例 {}", "{path}:{} scenario {}", sc.span.line, sc.name.text))
                        .mark(sc.span.clone(), tr!("`machine` がありません", "there is no `machine`"))
                        .note(tr!(
                            "手順の例は、`initial` の状態から呼び出しを順に重ねたものです。持ち越す状態が無ければ、一行ずつの `examples` で書けます。",
                            "A scenario is a sequence of calls from the `initial` state. With no state carried from one call to the next, each row is an `examples` row."
                        )),
                );
            }
            return;
        };
        let at_m = |line: usize| tr!("{path}:{line} ステートマシン {}", "{path}:{line} machine {}", m.name.text);
        if let Some(fd) = f.folds().next() {
            self.diags.push(
                Diag::error("E054", tr!("並びを畳む規則は、ステートマシンの一歩になれません", "A rule that folds a sequence cannot be the step of a machine"))
                    .at(at_m(m.span.line))
                    .mark(m.span.clone(), "")
                    .note(tr!(
                        "`fold` のある規則の答えは並び全体で決まるので、入力を決まった数の列で区切れません（{} 行目）。ステートマシンの検査は、その区切りごとに、一歩ずつの行き先を数えます。",
                        "The answer of a rule with a `fold` depends on the whole sequence, which does not cut into finitely many columns (line {}). The machine's checks count the transitions over exactly that cut."
                    , fd.span.line))
                    .note(tr!(
                        "並びを一件ずつ読むのは呼び出す側に任せ、その結果（件数や合計）を入力として渡すか、`count`・`sum` で数えてください。",
                        "Leave the walk to the caller and pass what it found (a count, a total) as an input, or count it with `count` or `sum`."
                    )),
            );
        }
        // The carried pair.
        let mut state_enum: Option<String> = None;
        if let Some((i, o, sp)) = &m.carry {
            let ity = self.syms.get(&i.text).filter(|s| s.kind == SymKind::Input).map(|s| s.ty.clone());
            let declared_out = f.outputs.iter().any(|d| d.name.text == o.text);
            let oty = if declared_out { self.syms.get(&o.text).map(|s| s.ty.clone()) } else { None };
            let mut bad = |what: String, span: &Span, note: String| {
                self.diags.push(
                    Diag::error("E051", tr!("`carry` の行が宣言と噛み合いません: {}", "The `carry` line does not fit the declarations: {}", what))
                        .at(at_m(sp.line))
                        .mark(span.clone(), "")
                        .note(note),
                );
            };
            match (&ity, &oty) {
                (None, _) => bad(
                    tr!("{} は入力ではありません", "{} is not an input", i.text),
                    &i.span,
                    tr!("左は、呼び出しのたびに渡される入力の名前です。", "The left side is the name of an input, passed on every call."),
                ),
                (_, None) => bad(
                    tr!("{} は出力ではありません", "{} is not an output", o.text),
                    &o.span,
                    tr!("右は、呼び出しが返す出力の名前です。次の呼び出しでは、その値が左の入力になります。", "The right side is the name of an output the call returns; on the next call its value is the input on the left."),
                ),
                (Some(Ty::Enum(a)), Some(Ty::Enum(b))) if a == b => state_enum = Some(a.clone()),
                (Some(a), Some(b)) => bad(
                    tr!("{} は {a}、{} は {b} です", "{} is {a} and {} is {b}", i.text, o.text),
                    sp,
                    tr!(
                        "持ち越す状態は、同じ列挙を型に持つ入力と出力の組です。状態の数が有限なので、呼び出しの並びについての主張を確かめられます。",
                        "The carried state is an input and an output of the same enum. That the states are finitely many is what makes the claims about sequences of calls decidable."
                    ),
                ),
            }
            if let Some(ov) = &m.over {
                let table = f.items.iter().find_map(|it| match it {
                    Item::Table(t) if !t.clause && t.name.as_ref().is_some_and(|n| n.text == ov.text) => Some(t),
                    _ => None,
                });
                match table {
                    None => self.diags.push(
                        Diag::error("E051", tr!("`{}` という表はありません", "There is no table called `{}`", ov.text))
                            .at(at_m(m.span.line))
                            .mark(ov.span.clone(), "")
                            .note(tr!("`over` の右は、持ち越す出力を決める表の名前です。", "What follows `over` is the name of the table that decides the carried output.")),
                    ),
                    Some(t) if !t.outputs.iter().any(|oc| oc.name.text == o.text) => self.diags.push(
                        Diag::error("E051", tr!("表 {} は {} を決めていません", "Table {} does not decide {}", ov.text, o.text))
                            .at(at_m(m.span.line))
                            .mark(ov.span.clone(), "")
                            .note(tr!(
                                "`over` の表の行が、状態の行き先そのものです。`over` には、{} を出力の列に持つ表の名前を書いてください。",
                                "The rows of the `over` table are the transitions. Name a table that has {} as an output column.",
                                o.text
                            )),
                    ),
                    Some(_) => {}
                }
            }
        }
        // The further carried values (§15.217), where they start, and the `always` lines.
        self.machine_more(f, m, path);
        // The states named.
        if let Some(en) = &state_enum {
            let values = self.enums.get(en).cloned().unwrap_or_default();
            let mut named: Vec<(&Name, String)> = Vec::new();
            if let Some((v, _)) = &m.initial {
                named.push((v, crate::kw::INITIAL.to_string()));
            }
            for v in &m.finals {
                named.push((v, crate::kw::FINAL.to_string()));
            }
            for al in &m.alwayses {
                for v in &al.when {
                    named.push((v, crate::kw::ALWAYS.to_string()));
                }
            }
            for nv in &m.nevers {
                for v in nv.states.iter().chain(nv.after.iter()) {
                    named.push((v, crate::kw::NEVER.to_string()));
                }
            }
            for (v, line_kw) in named {
                if !values.contains(&v.text) {
                    self.diags.push(
                        Diag::error("E052", tr!("`{}` は {en} の値ではありません", "`{}` is not a value of {en}", v.text))
                            .at(at_m(v.span.line))
                            .mark(v.span.clone(), tr!("`{line_kw}` の行", "on the `{line_kw}` line"))
                            .note(tr!("{en} の値は {} です。", "The values of {en} are {}.", values.join(" / "))),
                    );
                }
            }
            for nv in &m.nevers {
                for v in &nv.states {
                    if nv.after.iter().any(|a| a.text == v.text) {
                        self.diags.push(
                            Diag::error("E052", tr!("`never` の両側に同じ状態 {} があります", "The same state {} is on both sides of `never`", v.text))
                                .at(at_m(nv.span.line))
                                .mark(v.span.clone(), "")
                                .note(tr!(
                                    "`never A after B` は、B を通ったあとに A に着く手順が無いという主張です。同じ状態が両側にあると、そこに留まる一歩で破れてしまい、意図した主張になりません。",
                                    "`never A after B` says no sequence of calls reaches A once it has been in B. A state on both sides is broken by the first call that stays in it, which is not what anyone means."
                                )),
                        );
                    }
                }
            }
        }
        // `held`: inputs a case holds from its first call to its last (§15.149).
        {
            let carried_in = m.carried().map(|(i, _)| i.to_string());
            let mut seen: Vec<&str> = Vec::new();
            for n in &m.held {
                let bad = |what: String, note: String| {
                    Diag::error("E056", tr!("`held` の行が宣言と噛み合いません: {}", "The `held` line does not fit the declarations: {}", what))
                        .at(at_m(n.span.line))
                        .mark(n.span.clone(), "")
                        .note(note)
                };
                if seen.contains(&n.text.as_str()) {
                    self.diags.push(bad(
                        tr!("{} が二度あります", "{} is named twice", n.text),
                        tr!("一度書けば足ります。", "Once is enough."),
                    ));
                    continue;
                }
                seen.push(n.text.as_str());
                let is_input = self.syms.get(&n.text).is_some_and(|s| s.kind == SymKind::Input);
                if !is_input {
                    self.diags.push(bad(
                        tr!("{} は入力ではありません", "{} is not an input", n.text),
                        tr!(
                            "`held` に書くのは、一つの案件の最初の呼び出しから最後の呼び出しまで同じ値で渡される入力です（注文の金額、申し込んだ人の区分）。",
                            "`held` names the inputs one case passes with the same value on every call, from its first to its last: the amount of an order, the class of the person who applied."
                        ),
                    ));
                    continue;
                }
                if carried_in.as_deref() == Some(n.text.as_str()) || m.more().iter().any(|(i, _)| *i == n.text) {
                    self.diags.push(bad(
                        if carried_in.as_deref() == Some(n.text.as_str()) {
                            tr!("{} は持ち越す状態です", "{} is the carried state", n.text)
                        } else {
                            tr!("{} は持ち越す値です", "{} is a carried value", n.text)
                        },
                        tr!(
                            "持ち越す入力は、呼び出しのたびに一つ前の答えで入れ替わります。変わらない入力とは両立しません。",
                            "The carried input is replaced by the answer of the call before on every call, so it cannot be one that never changes."
                        ),
                    ));
                }
            }
        }
        // `once`. What follows reads the carried pair, and says nothing more once the `carry`
        // line itself has been refused: a second error about the same line is noise.
        let carried_out = m.carried().map(|(_, o)| o.to_string()).filter(|_| state_enum.is_some());
        for on in &m.onces {
            let declared = f.outputs.iter().any(|d| d.name.text == on.output.text);
            let bad = |what: String, note: String| {
                Diag::error("E053", tr!("`once` の行が宣言と噛み合いません: {}", "The `once` line does not fit the declarations: {}", what))
                    .at(at_m(on.span.line))
                    .mark(on.output.span.clone(), "")
                    .note(note)
            };
            if !declared {
                self.diags.push(bad(
                    tr!("{} は出力ではありません", "{} is not an output", on.output.text),
                    tr!("`once` が数えるのは、一回の呼び出しが返す出力の値です。", "What `once` counts is a value one call returns: an output."),
                ));
                continue;
            }
            if carried_out.as_deref() == Some(on.output.text.as_str()) || m.more().iter().any(|(_, o)| *o == on.output.text) {
                self.diags.push(bad(
                    if carried_out.as_deref() == Some(on.output.text.as_str()) {
                        tr!("{} は持ち越す状態です", "{} is the carried state", on.output.text)
                    } else {
                        tr!("{} は持ち越す値です", "{} is a carried value", on.output.text)
                    },
                    tr!(
                        "状態は、留まる呼び出しのたびに同じ値を返すので、`once` で数えると留まるだけで破れます。状態についての主張なら、`never … after …` を使ってください。",
                        "A state is answered again on every call that stays in it, so counting it with `once` breaks on the first stay. Say it about states with `never … after …`."
                    ),
                ));
                continue;
            }
            if matches!(on.cell, Cell::DontCare) {
                self.diags.push(bad(
                    tr!("`-` はどの値も数えます", "`-` counts every value"),
                    tr!("数える値をセルで書いてください（`>0円`、`true` など）。", "Write the values to count as a cell (`>0JPY`, `true`, …)."),
                ));
                continue;
            }
            if let Some(ty) = self.syms.get(&on.output.text).map(|s| s.ty.clone()) {
                let sc = *self.scales.get(&on.output.text).unwrap_or(&1);
                // The cell is a test on the output, read exactly as a table's cell on a column of
                // that type would be; what it names is not a row, so W111 is left as it was.
                let (used, used_values) = (self.used.clone(), self.used_values.clone());
                self.cell(&on.cell, &ty, sc, &on.cell_span, &at_m(on.span.line), true);
                self.used = used;
                self.used_values = used_values;
            }
        }
        // `scenario`.
        let mut names: Vec<&str> = Vec::new();
        let carried_in = m.carried().map(|(i, _)| i.to_string());
        let seq_col: Vec<String> = f.elements.iter().map(|el| el.name.text.clone()).collect();
        for sc in &f.scenarios {
            let at_s = tr!("{path}:{} 手順の例 {}", "{path}:{} scenario {}", sc.span.line, sc.name.text);
            if names.contains(&sc.name.text.as_str()) {
                self.diags.push(
                    Diag::error("E055", tr!("`{}` という `scenario` が二つあります", "There are two scenarios called `{}`", sc.name.text))
                        .at(at_s.clone())
                        .mark(sc.name.span.clone(), tr!("二つ目です", "this is a second one")),
                );
            }
            names.push(sc.name.text.as_str());
            if state_enum.is_none() {
                continue;
            }
            for (n, sp) in &sc.table.inputs {
                if carried_in.as_deref() == Some(n.as_str()) || m.more().iter().any(|(i, _)| i == n) {
                    self.diags.push(
                        Diag::error("E055", tr!("手順の例に、持ち越す入力 {n} の列があります", "The scenario has a column for the carried input {n}"))
                            .at(at_s.clone())
                            .mark(sp.clone(), "")
                            .note(tr!(
                                "一行目は `initial` の状態から、二行目からは一つ前の呼び出しが返した状態から始まります。列に書くと、その二つのどちらを信じるかが決まりません。",
                                "The first row starts from the `initial` state and every later row from the state the call before it answered. A column for it would leave two answers to that question."
                            )),
                    );
                }
            }
            let missing: Vec<String> = f
                .inputs
                .iter()
                .map(|i| i.name.text.clone())
                .chain(seq_col.clone())
                .filter(|n| carried_in.as_deref() != Some(n.as_str()) && !m.more().iter().any(|(i, _)| i == n))
                .filter(|n| !sc.table.inputs.iter().any(|(c, _)| c == n))
                .collect();
            if !missing.is_empty() {
                self.diags.push(
                    Diag::error("E055", tr!("手順の例に入力 {} の列がありません", "The scenario has no column for input {}", missing.join(", ")))
                        .at(at_s.clone())
                        .mark(sc.span.clone(), "")
                        .note(tr!(
                            "呼び出し一回に渡す入力を、全部書いてください。持ち越す入力のほかは、どの入力も一行ごとに値が要ります。",
                            "Write everything one call is passed: every input but the carried one takes a value in every row."
                        )),
                );
            }
        }
    }

    /// The `carry` lines after the first, the `initial <name> <value>` lines and the `always`
    /// lines (§15.217). A further `carry` line is held to the declarations as the first one is
    /// (E051), except that its pair need not be an enum; what it carries must be an enum, a
    /// boolean or a number whose range has both ends, every one of them starts somewhere, and
    /// an `always` line names them and holds each to a cell of its type (E072). Returns the
    /// pairs that fit, with their type.
    fn machine_more(&mut self, f: &RuleFile, m: &MachineDecl, path: &str) -> Vec<(String, String, Ty)> {
        let at_m = |line: usize| tr!("{path}:{line} ステートマシン {}", "{path}:{line} machine {}", m.name.text);
        let state_in = m.carried().map(|(i, _)| i.to_string());
        let mut ok: Vec<(String, String, Ty)> = Vec::new();
        let mut seen_in: Vec<String> = m.carried().map(|(i, _)| i.to_string()).into_iter().collect();
        let mut seen_out: Vec<String> = m.carried().map(|(_, o)| o.to_string()).into_iter().collect();
        let e072 = |title: String, at: String, span: &Span, note: String| Diag::error("E072", title).at(at).mark(span.clone(), "").note(note);
        for (i, o, sp) in &m.carries {
            let ity = self.syms.get(&i.text).filter(|s| s.kind == SymKind::Input).map(|s| s.ty.clone());
            let declared_out = f.outputs.iter().any(|d| d.name.text == o.text);
            let oty = if declared_out { self.syms.get(&o.text).map(|s| s.ty.clone()) } else { None };
            let e051 = |what: String, span: &Span, note: String| {
                Diag::error("E051", tr!("`carry` の行が宣言と噛み合いません: {}", "The `carry` line does not fit the declarations: {}", what))
                    .at(at_m(sp.line))
                    .mark(span.clone(), "")
                    .note(note)
            };
            let ty = match (&ity, &oty) {
                (None, _) => {
                    self.diags.push(e051(
                        tr!("{} は入力ではありません", "{} is not an input", i.text),
                        &i.span,
                        tr!("左は、呼び出しのたびに渡される入力の名前です。", "The left side is the name of an input, passed on every call."),
                    ));
                    continue;
                }
                (_, None) => {
                    self.diags.push(e051(
                        tr!("{} は出力ではありません", "{} is not an output", o.text),
                        &o.span,
                        tr!("右は、呼び出しが返す出力の名前です。次の呼び出しでは、その値が左の入力になります。", "The right side is the name of an output the call returns; on the next call its value is the input on the left."),
                    ));
                    continue;
                }
                (Some(a), Some(b)) if a != b => {
                    self.diags.push(e051(
                        tr!("{} は {a}、{} は {b} です", "{} is {a} and {} is {b}", i.text, o.text),
                        sp,
                        tr!(
                            "持ち越す値は、同じ型の入力と出力の組です。次の呼び出しでは、出力の値をそのまま入力として渡します。",
                            "A carried value is an input and an output of one type: the output's value is passed as the input on the next call, as it is."
                        ),
                    ));
                    continue;
                }
                (Some(a), Some(_)) => a.clone(),
            };
            let carry_title = |what: String| tr!("`carry` の行で持ち越せない値です: {}", "A `carry` line carries what cannot be carried: {}", what);
            if seen_in.contains(&i.text) || seen_out.contains(&o.text) {
                let twice = if seen_in.contains(&i.text) { &i.text } else { &o.text };
                self.diags.push(e072(
                    carry_title(tr!("{} を二度持ち越しています", "{} is carried twice", twice)),
                    at_m(sp.line),
                    sp,
                    tr!("一つの値は、`carry` の一行で持ち越します。", "One value is carried on one `carry` line."),
                ));
                continue;
            }
            seen_in.push(i.text.clone());
            seen_out.push(o.text.clone());
            match &ty {
                Ty::Enum(_) | Ty::Bool => {}
                Ty::Number | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    if !matches!(self.ranges.get(&i.text), Some((Some(_), Some(_)))) {
                        self.diags.push(e072(
                            carry_title(tr!("{} の範囲に両端がありません", "{} has no range with both ends", i.text)),
                            at_m(sp.line),
                            &i.span,
                            tr!(
                                "持ち越す数は、呼び出しのたびに変わります。範囲に両端があれば値は有限個になり、呼び出しの並びについての主張を確かめられます。入力に `range >=0 <=16` のように両端を書いてください。",
                                "A carried number changes from one call to the next. With both ends of a range its values are finitely many, which is what makes the claims about sequences of calls decidable. Give the input both ends, as in `range >=0 <=16`."
                            ),
                        ));
                        continue;
                    }
                }
                other => {
                    self.diags.push(e072(
                        carry_title(tr!("{} は {other} です", "{} is {other}", i.text)),
                        at_m(sp.line),
                        sp,
                        tr!(
                            "持ち越せるのは、列挙と真偽と、範囲のある数（数、金額、量、率）です。",
                            "What can be carried is an enum, a boolean, or a number with a range: a count, an amount, a quantity or a rate."
                        ),
                    ));
                    continue;
                }
            }
            ok.push((i.text.clone(), o.text.clone(), ty));
        }
        // Where each starts.
        let init_title = |what: String| tr!("`initial` の行が宣言と噛み合いません: {}", "The `initial` line does not fit the declarations: {}", what);
        let mut started: Vec<String> = Vec::new();
        for (n, v, sp) in &m.initials {
            if state_in.as_deref() == Some(n.text.as_str()) {
                self.diags.push(e072(
                    init_title(tr!("{} は状態です", "{} is the state", n.text)),
                    at_m(sp.line),
                    &n.span,
                    tr!("状態の始まりは、`initial <状態>` と一語で書きます。", "Where the state starts is written in one word: `initial <state>`."),
                ));
                continue;
            }
            if !m.carries.iter().any(|c| c.0.text == n.text) {
                self.diags.push(e072(
                    init_title(tr!("{} は持ち越す値ではありません", "{} is not a carried value", n.text)),
                    at_m(sp.line),
                    &n.span,
                    tr!(
                        "`initial <名前> <値>` の名前は、二行目からの `carry` の左にある入力です。",
                        "The name in `initial <name> <value>` is the input on the left of a `carry` line after the first."
                    ),
                ));
                continue;
            }
            if started.contains(&n.text) {
                self.diags.push(e072(
                    init_title(tr!("{} の行が二つあります", "there are two lines for {}", n.text)),
                    at_m(sp.line),
                    sp,
                    tr!("一つの値の始まりは一つです。", "A value starts in one place."),
                ));
                continue;
            }
            started.push(n.text.clone());
            let Some((_, _, ty)) = ok.iter().find(|x| x.0 == n.text) else { continue };
            let fits = match (ty, crate::eval::lit_to_val(v, ty)) {
                (Ty::Enum(en), Some(crate::eval::Val::Enum(w))) => self.enums.get(en).is_some_and(|vs| vs.contains(&w)),
                (Ty::Bool, Some(crate::eval::Val::Bool(_))) => true,
                (Ty::Number | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate, Some(crate::eval::Val::Num(r))) => {
                    let (lo, hi) = self.ranges.get(&n.text).cloned().unwrap_or((None, None));
                    let inside = lo.is_none_or(|l| r.cmp_to(l) != std::cmp::Ordering::Less) && hi.is_none_or(|h| r.cmp_to(h) != std::cmp::Ordering::Greater);
                    let scaled = r.mul(Rat::int(self.wire_scale(&n.text)));
                    inside && scaled.den == 1 && matches!(v, Lit::Num(_))
                }
                _ => false,
            };
            if !fits {
                let range = match self.ranges.get(&n.text) {
                    Some((Some(l), Some(h))) => tr!("、範囲は >={} <={}", ", with the range >={} <={}", fmt_val(*l, ty), fmt_val(*h, ty)),
                    _ => String::new(),
                };
                self.diags.push(e072(
                    init_title(tr!("{} は {} の値になれません", "{} cannot be a value of {}", crate::ast::lit_text(v), n.text)),
                    at_m(sp.line),
                    sp,
                    tr!("{} は {ty}{range} です。", "{} is {ty}{range}.", n.text),
                ));
            }
        }
        for (i, _, sp) in &m.carries {
            if ok.iter().any(|x| x.0 == i.text) && !started.contains(&i.text) {
                self.diags.push(e072(
                    init_title(tr!("{} の始まりの値がありません", "{} has no value to start at", i.text)),
                    at_m(sp.line),
                    sp,
                    tr!(
                        "持ち越す値には、どれも始まりの値を書いてください（`initial {} <値>`）。案件の最初の呼び出しに渡す値です。",
                        "Give every carried value the value it starts at (`initial {} <value>`): what the first call of a case is passed.",
                        i.text
                    ),
                ));
            }
        }
        // The `always` lines.
        let always_title = |what: String| tr!("`always` の行が宣言と噛み合いません: {}", "The `always` line does not fit the declarations: {}", what);
        for al in &m.alwayses {
            for (n, cell, csp) in &al.tests {
                if state_in.as_deref() == Some(n.text.as_str()) {
                    self.diags.push(e072(
                        always_title(tr!("{} は状態です", "{} is the state", n.text)),
                        at_m(al.span.line),
                        &n.span,
                        tr!(
                            "状態について言うのは `never … after …` と `final` です。`always` の行が成り立つ状態は、`when` の後に書きます。",
                            "What is said of the state is said with `never … after …` and `final`; the states an `always` line holds in follow its `when`."
                        ),
                    ));
                    continue;
                }
                let Some((_, _, ty)) = ok.iter().find(|x| x.0 == n.text) else {
                    if !m.carries.iter().any(|c| c.0.text == n.text) {
                        self.diags.push(e072(
                            always_title(tr!("{} は持ち越す値ではありません", "{} is not a carried value", n.text)),
                            at_m(al.span.line),
                            &n.span,
                            tr!(
                                "`always` に書くのは、二行目からの `carry` の左にある入力です。案件がどの状態にあるときも、その値がセルに当てはまることを確かめます。",
                                "An `always` line names the input on the left of a `carry` line after the first, and the check is that its value fits the cell in every state a case can be in."
                            ),
                        ));
                    }
                    continue;
                };
                let ty = ty.clone();
                let sc = *self.scales.get(&n.text).unwrap_or(&1);
                // The cell is a test on the carried input, read as a table's cell on its column
                // would be; what it names is not a row, so W111 is left as it was.
                let (used, used_values) = (self.used.clone(), self.used_values.clone());
                self.cell(cell, &ty, sc, csp, &at_m(al.span.line), true);
                self.used = used;
                self.used_values = used_values;
            }
        }
        ok
    }

    /// A literal written as an output value, held to the type of the thing it is written
    /// into. Only `Lit::Num` was ever read: a date in a `money[円]` column generated the day
    /// number as an amount — `2026-04-01` came out as **20544円** — and a string came out as
    /// 0, both with nothing said (§15.88).
    fn out_lit(&mut self, l: &Lit, want: &Ty, sp: &Span, at: &str) {
        let want = match want {
            Ty::Opt(inner) => inner.as_ref(),
            other => other,
        };
        if *want == Ty::Unknown {
            return;
        }
        let what = match l {
            Lit::Date(..) if *want != Ty::Date => tr!("日付", "a date"),
            Lit::Str(_) if *want != Ty::Str => tr!("文字列", "a string"),
            _ => return,
        };
        self.diags.push(
            Diag::error("E103", tr!("この列は {want} ですが {what} が書かれています", "This column is {want}, but {what} is written here"))
                .at(at.to_string())
                .mark(sp.clone(), ""),
        );
    }

    fn cell(&mut self, cell: &Cell, want: &Ty, scale: i128, span: &Span, at: &str, pattern: bool) {
        // Writing a present-side value in an optional column is correct. `none` arrives as
        // Cell::Nothing.
        let want = match want {
            Ty::Opt(inner) => inner.as_ref(),
            other => other,
        };
        let check_lit = |s: &mut Self, l: &Lit| match l {
            Lit::Num(n) => {
                let Some(v) = lit_value_in(n, want) else {
                    s.diags.push(
                        Diag::error("E103", tr!("この列は {want} ですが `{}` が書かれています", "This column is {want}, but `{}` is written here", n.raw))
                            .at(at.to_string())
                            .mark(span.clone(), "")
                            .note(if n.unit.is_none() {
                                tr!("単位が要ります。裸の数は書けません。", "A unit is required. A bare number cannot be written.")
                            } else {
                                tr!("`{}` は {want} の単位ではありません。", "`{}` is not a unit of {want}.", n.raw)
                            }),
                    );
                    return;
                };
                // The runtime value is an integer count of the declared step (§2.1), so a
                // value between two steps has no representation. Rounding it silently would
                // move a boundary the reader can read on the page.
                if scale > 1 && !v.mul(Rat::int(scale)).is_int() {
                    let step = fmt_val(Rat::new(1, scale), want);
                    // The nearest value that is on the step, rounded half away from zero.
                    let k = v.mul(Rat::int(scale));
                    let half = if k.num < 0 { -k.den } else { k.den };
                    let near = fmt_val(Rat::new((k.num * 2 + half) / (k.den * 2), scale), want);
                    s.diags.push(
                        Diag::error("E114", tr!("`{}` はこの列の刻みに載っていません", "`{}` does not sit on this column's step", n.raw))
                            .at(at.to_string())
                            .mark(span.clone(), tr!("刻みは {step} です", "the step is {step}"))
                            .note(tr!(
                                "この列の型は刻みを {step} と宣言しているので、実行時の値はその整数倍だけです。",
                                "The column's type declares a step of {step}, so at runtime the value is a whole number of them."
                            ))
                            .note(tr!(
                                "`{near}` のように刻みに載る値に直すか、型の刻みを細かくしてください。黙って寄せると、読める境界と動く境界が食い違います。",
                                "Write a value on the step, such as `{near}`, or declare a finer step. Rounding it quietly would make the boundary on the page differ from the boundary in the code."
                            )),
                    );
                }
            }
            Lit::Date(..) => {
                if *want != Ty::Date && *want != Ty::Unknown {
                    s.diags.push(
                        Diag::error("E103", tr!("この列は {want} ですが 日付 が書かれています", "This column is {want}, but a date is written here"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    );
                }
            }
            // A string in a column that is not one. It fell off the end of this match, and
            // the row simply never matched anything — reported as E102, which sends the
            // reader to look at the other rows (§15.88).
            Lit::Str(_) => {
                if *want != Ty::Str && *want != Ty::Unknown {
                    s.diags.push(
                        Diag::error("E103", tr!("この列は {want} ですが 文字列が書かれています", "This column is {want}, but a string is written here"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    );
                }
            }
            Lit::Word(w) => {
                if w == crate::kw::TRUE || w == crate::kw::FALSE {
                    if *want != Ty::Bool && *want != Ty::Unknown {
                        s.diags.push(
                            Diag::error("E103", tr!("この列は {want} ですが 真偽 が書かれています", "This column is {want}, but a boolean is written here"))
                                .at(at.to_string())
                                .mark(span.clone(), ""),
                        );
                    }
                    return;
                }
                let col = match want {
                    Ty::Opt(t) => t.as_ref(),
                    t => t,
                };
                if let Some((mut owner, members)) = s.groups.get(w).cloned() {
                    if let Ty::Enum(ce) = col {
                        if *ce != owner {
                            if members.iter().all(|m| s.enum_has(ce, m)) && !s.used.contains(w) {
                                // A group of values two enums share belongs to the enum of the
                                // first column it is written in (§15.150).
                                if let Some(g) = s.groups.get_mut(w) {
                                    g.0 = ce.clone();
                                }
                                owner = ce.clone();
                            } else {
                                s.diags.push(
                                    Diag::error("E103", tr!("この列は {want} ですが、グループ `{w}`（{owner} の値）が書かれています", "This column is {want}, but group `{w}` (values of {owner}) is written here"))
                                        .at(at.to_string())
                                        .mark(span.clone(), ""),
                                );
                            }
                        }
                    }
                    s.used.insert(w.clone());
                    for m in members {
                        s.used_values.insert((owner.clone(), m));
                    }
                    return;
                }
                let of = s.enum_of_value_in(w, want);
                if let Some(e) = &of {
                    s.used_values.insert((e.clone(), w.clone()));
                }
                match (&of, col) {
                    (Some(e), Ty::Enum(w2)) if e == w2 => {}
                    (_, Ty::Unknown) => {}
                    (None, _) => s.diags.push(
                        Diag::error("E012", tr!("`{w}` は値としてもグループとしても見つかりません", "`{w}` is found neither as a value nor as a group"))
                            .at(at.to_string())
                            .mark(span.clone(), "")
                            .maybe_note(crate::prelude::hint(w, &s.imported)),
                    ),
                    (Some(e), _) => s.diags.push(
                        Diag::error("E103", tr!("この列は {want} ですが `{w}`（{e} の値）が書かれています", "This column is {want}, but `{w}` (a value of {e}) is written here"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    ),
                }
            }
        };
        // A `string` column takes a prefix and nothing else, and a prefix goes nowhere
        // else (§15.101). Without this both mistakes came out as E102 "no input reaches
        // here", which is a symptom and not the fault.
        // Only in a table: a cell of `examples` is a value the caller sends, and a string
        // there is a string.
        let text = pattern && matches!(want, Ty::Str);
        match (text || (pattern && matches!(cell, Cell::Prefix(_))), cell) {
            (true, Cell::Prefix(_)) if !matches!(want, Ty::Str) => self.diags.push(
                Diag::error("E103", tr!("この列は {want} ですが、前方一致が書かれています", "This column is {want}, and a prefix is written here"))
                    .at(at.to_string())
                    .mark(span.clone(), tr!("`starts_with` は文字列の列にだけ書けます", "`starts_with` belongs to a column of strings"))
                    .note(tr!(
                        "前方一致で切れるのは文字列です。数や列挙なら、比較や値の名前で書いてください。",
                        "A prefix cuts strings. On a number or an enum, write a comparison or the value's name."
                    )),
            ),
            (true, Cell::Lit(_) | Cell::Set(_) | Cell::Not(_) | Cell::Cmp(_)) if matches!(want, Ty::Str) => self.diags.push(
                Diag::error("E110", tr!("文字列の列に書けるのは前方一致だけです", "A column of strings takes a prefix and nothing else"))
                    .at(at.to_string())
                    .mark(span.clone(), tr!("前方一致ではありません", "not a prefix"))
                    .note(tr!(
                        "`starts_with \"ABC\"` と書いてください。文字列を一つずつ数え上げることはできないので、等号や集合は書けません。値が数えられるものなら、`enum` にしてください。",
                        "Write `starts_with \"ABC\"`. Strings cannot be enumerated, so equality and sets are not available — make it an `enum` if the values can be listed."
                    )),
            ),
            _ => {}
        }
        match cell {
            Cell::DontCare | Cell::Nothing => {}
            Cell::Prefix(_) => {}
            Cell::Lit(l) => check_lit(self, l),
            Cell::Set(ls) | Cell::Not(ls) => {
                for l in ls {
                    check_lit(self, l);
                }
            }
            Cell::Cmp(cs) => {
                // A comparison left as the parser wrote it on a column of an enum: the enum has no
                // order to compare in, or a bound is none of its values, which `check_lit` says.
                if let Ty::Enum(en) = want {
                    if !self.ordered.contains(en) {
                        self.diags.push(unordered_comparison(en, at, span, tr!("比べるセル", "a comparison in a cell")));
                        // The cell meant to name some of the values; which ones has no answer
                        // without an order, so W111 does not call any of them unused on top.
                        for v in self.enums.get(en).cloned().unwrap_or_default() {
                            self.used_values.insert((en.clone(), v));
                        }
                        return;
                    }
                }
                for (_, l) in cs {
                    check_lit(self, l);
                }
            }
            // The values a comparison on an ordered enum covers are named by it (§15.210), so W111
            // does not call them unused.
            Cell::Ranked { values, .. } => {
                if let Ty::Enum(en) = want {
                    for v in values {
                        self.used_values.insert((en.clone(), v.clone()));
                    }
                }
            }
        }
    }
}

/// E069: an enum whose values are separated by `|`, and so have no order, met by something
/// that compares two of them (§15.210).
fn unordered_comparison(en: &str, at: &str, span: &Span, what: String) -> Diag {
    Diag::error("E069", tr!("列挙 {en} には順序がないので、{what}は書けません", "Enum {en} has no order, so {what} cannot be written"))
        .at(at.to_string())
        .mark(span.clone(), "")
        .note(tr!(
            "比べられるのは、値を `<` で区切って宣言した列挙だけです（`enum {en} = a < b < c`）。宣言の順が意味を持つなら `<` で区切ってください。順序に意味が無いなら、値か集合で書いてください。",
            "Only an enum whose values are declared with `<` between them is compared (`enum {en} = a < b < c`). If the order it is declared in means something, separate the values with `<`; if not, write the values or a set."
        ))
}

impl Checked {
    pub fn ty_of(&self, name: &str) -> Option<Ty> {
        self.syms.get(name).map(|s| s.ty.clone())
    }

    /// The enum a type is, when it is an ordered one, optional or not (§15.210).
    pub fn ordered_enum<'t>(&self, ty: &'t Ty) -> Option<&'t str> {
        match ty.present() {
            Ty::Enum(e) if self.ordered.contains(e) => Some(e.as_str()),
            _ => None,
        }
    }

    /// The place of value `v` in the order of the ordered enum `en`, counting from 0.
    pub fn rank(&self, en: &str, v: &str) -> Option<usize> {
        if !self.ordered.contains(en) {
            return None;
        }
        self.enums.get(en)?.iter().position(|x| x == v)
    }

    /// What value `v` of enum `en` is shown as (§15.211): its display name, or its name when it
    /// has none.
    pub fn shown_value(&self, en: &str, v: &str) -> String {
        let i = self.enums.get(en).and_then(|vs| vs.iter().position(|x| x == v));
        i.and_then(|i| self.labels.get(en).and_then(|ls| ls.get(i).cloned().flatten())).unwrap_or_else(|| v.to_string())
    }

    /// Every value of a type of words: an enum's values, or the two truth values. `None` for any
    /// other type.
    pub fn values_of(&self, ty: &Ty) -> Option<Vec<String>> {
        match ty.present() {
            Ty::Enum(e) => self.enums.get(e).cloned(),
            Ty::Bool => Some(vec![crate::kw::TRUE.to_string(), crate::kw::FALSE.to_string()]),
            _ => None,
        }
    }

    /// The values row `ri` of table `t` can write into its output column `oi`, for a column of
    /// an enum or of truth values (§15.208).
    ///
    /// A value written in the cell is that value. A name stands for what it holds when the row
    /// fires: when it is one of the table's own columns, the values this row's cell for it lets
    /// through — `| c | x |` writes `c`, the only value of `x` the row takes — and otherwise every
    /// value of its type. That is the safe side for both of the checks that read it: E102 never
    /// takes a row below for dead when a name above can reach it, and E101 asks no row for a value
    /// nothing above can write. The two used to read the cell differently — the values the cells
    /// write, with the name taken for a value of its own, against "a name can be anything" — so the
    /// same rule was told to delete a row (E102) and to add it (E101) at once. `None` for a
    /// column of another type, or a name whose type is not one of words.
    pub fn row_writes(&self, t: &Table, ri: usize, oi: usize) -> Option<Vec<String>> {
        let row = t.rows.get(ri)?;
        let col = self.ty_of(&t.outputs.get(oi)?.name.text)?;
        self.values_of(&col)?;
        match row.outs.get(oi)? {
            OutCell::Lit(Lit::Word(w)) => Some(vec![w.clone()]),
            OutCell::Lit(_) => None,
            OutCell::Name(w) => {
                let Some(ty) = self.ty_of(w) else {
                    // No name of that spelling where the cell is: the word is a value.
                    return Some(vec![w.clone()]);
                };
                let all = self.values_of(&ty)?;
                let Some(ci) = t.inputs.iter().position(|(n, _)| n == w) else { return Some(all) };
                let Some(cell) = row.cells.get(ci) else { return Some(all) };
                Some(
                    all.into_iter()
                        .filter(|v| {
                            let val = match ty.present() {
                                Ty::Bool => crate::eval::Val::Bool(v == crate::kw::TRUE),
                                _ => crate::eval::Val::Enum(v.clone()),
                            };
                            crate::eval::cell_matches(self, cell, &val, &ty)
                        })
                        .collect(),
                )
            }
        }
    }

    /// The factor a named value is multiplied by to become the integer it travels as.
    ///
    /// One for everything with a unit of its own — yen, grams, centimetres are already
    /// integers in the unit they declare. A **rate has no unit**, so it travels as the
    /// number of steps its declaration names: `10%` under `rate[step 1%]` is 10 (§2.1,
    /// §10.2). A rate with no step declared falls back to hundredths, which is what the
    /// generated code assumes for a column whose literals fixed no finer grid.
    ///
    /// Everything that turns a value into an integer — the vectors, the fixtures, a
    /// report, a witness, the entry guard of the generated code — asks here. Two answers
    /// for one value is how a rate came to mean 1 on one side of the wire and 100 on the
    /// other.
    pub fn wire_scale(&self, name: &str) -> i128 {
        if let Some(s) = self.out_scales.get(name) {
            return *s;
        }
        // An optional rate travels as the same count of steps when it is there (§15.201).
        match self.ty_of(name).as_ref().map(Ty::present) {
            Some(Ty::Rate) => *self.scales.get(name).unwrap_or(&100),
            _ => 1,
        }
    }

    /// What `check` keeps of a computed value: its range where the arithmetic bounds it, the
    /// scale it is stored at, and the step its values land on (§15.190). The scale does not wait
    /// for the range. The generated code holds the value at its expression's scale either way,
    /// and a name recorded at none was read back as whole units: `amount × 10%` over an amount
    /// with no range was held in pennies and compared with whole pounds.
    fn record_value(&mut self, name: &str, e: &Expr, ty: &Ty) {
        let sc = self.scale(e);
        if let (Some(iv), Some(_)) = (self.interval(e, ty), sc) {
            self.ranges.insert(name.to_string(), (Some(iv.0), Some(iv.1)));
        }
        if let Some(sc) = sc {
            self.scales.insert(name.to_string(), sc);
        }
        if let Some(g) = self.lattice(e).filter(|g| g.num != 0) {
            self.grids.insert(name.to_string(), g);
        }
    }

    /// The step every value of `name` is a multiple of (§15.190): for a computed value, the step
    /// its expression lands on ([`Checked::grids`]); for anything else, the step it is stored
    /// at, which holds every value it takes and perhaps more. An input of money, a quantity or
    /// `number` is whole; a rate input is on its declared step.
    pub fn value_step(&self, name: &str) -> Rat {
        self.grids.get(name).copied().unwrap_or_else(|| Rat::new(1, self.scales.get(name).copied().unwrap_or(1).max(1)))
    }

    /// The step every value of an expression is a multiple of, from its leaves up: a name is on
    /// its own step and a literal on its value; a sum or a difference on the step both sides
    /// share, a product on the product of the two, a quotient by a whole constant on its left
    /// side's step over the constant; `min` and `max` give one of their sides, so the step both
    /// share; a rounding lands on its grid, `allocate` on whole units. `amount × 10%` is on
    /// tenths whatever `10%` is stored at, `12570GBP − above × 50%` on halves.
    ///
    /// Zero, for an expression that is always zero, divides nothing and is what a sum leaves
    /// out. `None` for a shape this does not read; the storage step stands in for it.
    fn lattice(&self, e: &Expr) -> Option<Rat> {
        let abs = |r: Rat| -> Option<Rat> { if r.num < 0 { Some(Rat { num: r.num.checked_neg()?, den: r.den }) } else { Some(r) } };
        let share = |a: Rat, b: Rat| -> Option<Rat> {
            if a.num == 0 {
                Some(b)
            } else if b.num == 0 {
                Some(a)
            } else {
                common_step(a, b)
            }
        };
        match e {
            Expr::Name(n, _) => Some(self.value_step(n)),
            Expr::Lit(Lit::Num(n), _) => abs(lit_value_in(n, &lit_ty(n))?),
            Expr::Lit(..) => None,
            Expr::Bin(l, op, r, _) => match op {
                BinOp::Add | BinOp::Sub => share(self.lattice(l)?, self.lattice(r)?),
                BinOp::Mul => self.lattice(l)?.checked_mul(self.lattice(r)?),
                // §2.3: the divisor is a whole constant, as `scale` reads it.
                BinOp::Div => self.lattice(l)?.checked_div(Rat::int(const_value(r)?)),
                _ => None,
            },
            Expr::Call(name, args, _) => {
                if name == crate::kw::MIN || name == crate::kw::MAX {
                    let [x, y] = args.as_slice() else { return None };
                    return share(self.lattice(x)?, self.lattice(y)?);
                }
                if crate::num::RoundMode::parse(name).is_some() {
                    return match args.get(1)? {
                        Expr::Lit(Lit::Num(g), _) => abs(lit_value_in(g, &lit_ty(g))?),
                        _ => None,
                    };
                }
                // A remainder of a whole number by n moves it by multiples of n, so it keeps the
                // step the two share (§15.214); a whole square root is on whole units.
                if let (crate::kw::MOD, [x, k]) = (name.as_str(), args.as_slice()) {
                    return common_step(self.lattice(x)?, Rat::int(const_value(k)?));
                }
                (name == crate::kw::ALLOCATE || name == crate::kw::ISQRT).then_some(Rat::int(1))
            }
        }
    }
}

/// The largest step two positive steps are both whole multiples of: a sixth and a fourth share a
/// twelfth, a tenth and a whole unit a tenth (§15.190). `None` where the arithmetic does not fit.
pub fn common_step(a: Rat, b: Rat) -> Option<Rat> {
    fn gcd(mut x: i128, mut y: i128) -> i128 {
        while y != 0 {
            let t = x % y;
            x = y;
            y = t;
        }
        x
    }
    let n = gcd(a.num.checked_mul(b.den)?.checked_abs()?, b.num.checked_mul(a.den)?.checked_abs()?);
    if n == 0 {
        return None;
    }
    Rat::checked_new(n, a.den.checked_mul(b.den)?)
}

/// The larger of the two magnitudes at the ends of an interval: how far from zero a value in it
/// can be.
pub fn magnitude_of(lo: Rat, hi: Rat) -> Rat {
    let abs = |r: Rat| if r.num < 0 { Rat::zero().sub(r) } else { r };
    if abs(lo).cmp_to(abs(hi)) == std::cmp::Ordering::Greater { abs(lo) } else { abs(hi) }
}

/// A rounding grid as the generated code holds it: the integer and the scale it is held at,
/// read in the literal's own unit (`10円` is 10 at 1, `0.1%` is 1 at 1000). The same reading
/// `codegen` gives a literal, so the factors below are the ones the generated code multiplies by.
pub fn grid_int(g: &crate::lex::Num) -> Option<(i128, i128)> {
    let v = lit_value_in(g, &lit_ty(g))?;
    let den = v.den.max(1);
    let s = lit_scale(g).filter(|s| s % den == 0).unwrap_or(den);
    Some((v.num.checked_mul(s / den)?, s))
}

/// What the two sides of a rounded division are multiplied by before the one integer division
/// the generated code does (§15.213): the dividend `N` at scale `sn`, the divisor `D` at scale
/// `sd`, and the grid `G` at scale `sg`. The number of grid steps is `N·sd·sg ÷ (D·sn·G)`, so the
/// dividend is multiplied by `sd·sg` and the divisor by `sn·G`, each over what the two share.
pub fn division_factors(sn: i128, sd: i128, gi: i128, sg: i128) -> (i128, i128) {
    fn gcd(mut a: i128, mut b: i128) -> i128 {
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a.abs().max(1)
    }
    let (a, b) = (sd * sg, sn * gi);
    let g = gcd(a, b);
    (a / g, b / g)
}

/// A value as the integer it travels as, at the scale `Checked::wire_scale` gives.
pub fn wire_int(v: Rat, scale: i128) -> i128 {
    let s = v.mul(Rat::int(scale));
    s.num / s.den
}

/// The same journey back: an integer off the wire into the value it stands for.
pub fn from_wire(n: i128, scale: i128) -> Rat {
    Rat::new(n, scale)
}

pub fn lit_ty_pub(n: &crate::lex::Num) -> Ty {
    lit_ty(n)
}

/// Entry point so that `region` uses the same conversion.
/// Least common multiple, on the scales of §7.1 (all positive).
pub fn lcm_i128(a: i128, b: i128) -> i128 {
    fn gcd(mut a: i128, mut b: i128) -> i128 {
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a.max(1)
    }
    a / gcd(a, b) * b
}

pub fn lit_value_in_pub(n: &crate::lex::Num, want: &Ty) -> Option<Rat> {
    lit_value_in(n, want)
}

/// The scale at which a numeric literal is a whole number (§7.1).
///
/// A decimal moves it by a power of ten: `0.5%` is 1/200, so 100 would not clear it. A unit
/// finer than its dimension's base carries its own factor: `%` and `銭` are hundredths,
/// `USDc` too, `mm` a tenth.
///
/// Deliberately *not* the reduced denominator of the value. `3.6%` is 9/250, but a name
/// defined as `× 3.6%` is recorded at 1000 — so a generator that stored it at 250 would
/// write at one scale and read back at another. One definition, used by both sides.
pub fn lit_scale(n: &crate::lex::Num) -> Option<i128> {
    let d = 10i128.checked_pow(u32::try_from(n.frac.len()).ok()?)?;
    let u = n.unit.as_deref().and_then(unit_info).map_or(1, |(_, f)| f.den);
    d.checked_mul(u)
}

/// The §1.6 rendering uses the same write-back. Showing a reader `1000000円` forces a
/// mental conversion before it can be matched against the `100万円` in the rule source.
pub fn fmt_big_pub(v: Rat) -> String {
    fmt_big(v, crate::i18n::ja())
}

/// Large amounts are written back with 万 and 億. Answering `1000000円` to an author who
/// wrote `100万円` forces a mental conversion before they can fix anything.
///
/// In English the same author writes `1_000_000JPY`, the way the language groups digits (a `,`
/// is E049) and the way the corpus writes them, so that is what comes back: a whole number of
/// five digits or more, in groups of three joined by `_`. Japanese is as it was.
fn fmt_big(v: Rat, ja: bool) -> String {
    if !v.is_int() {
        return format!("{v}");
    }
    let n = v.num;
    if !ja {
        return group_digits(n);
    }
    if n != 0 && n % 1_000_000_000_000 == 0 {
        return format!("{}兆", n / 1_000_000_000_000);
    }
    if n != 0 && n % 100_000_000 == 0 {
        return format!("{}億", n / 100_000_000);
    }
    if n != 0 && n % 10_000 == 0 {
        return format!("{}万", n / 10_000);
    }
    format!("{n}")
}

/// `1000000` is `1_000_000`, and `-12345` is `-12_345`; four digits or fewer are left alone, as
/// the corpus writes `5000JPY` and `10_000JPY`.
fn group_digits(n: i128) -> String {
    let digits = n.unsigned_abs().to_string();
    if digits.len() < 5 {
        return n.to_string();
    }
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('_');
        }
        out.push(c);
    }
    out
}

/// One unit of the type, as it is written: `1円`, `1g`, `1%`. It is the finest grid a
/// rounding declaration can name, so `round down(1円)` is the edit that removes E104 while
/// changing the answer the least. **Which direction and which grid are right is a business
/// decision**, which the notes say and `fix.text` cannot.
fn unit_one(ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, .. } => format!("1{cur}"),
        Ty::Qty { unit, .. } => format!("1{unit}"),
        Ty::Rate => "1%".into(),
        Ty::Number => "1".into(),
        _ => "1".into(),
    }
}

/// A value the way the rule writes one of its type: `1000円`, `2kg`, `1%`. Amounts of many digits
/// are written back as the language of the output writes them (see `fmt_big`).
pub fn fmt_val(v: Rat, ty: &Ty) -> String {
    fmt_val_in(v, ty, crate::i18n::ja())
}

/// The same for what a fix writes into the rule (`fix.text`). A fix is data, the same in both
/// languages (the JSON tests hold it), so its amounts are written back as they always were.
pub fn fmt_val_fixed(v: Rat, ty: &Ty) -> String {
    fmt_val_in(v, ty, true)
}

fn fmt_val_in(v: Rat, ty: &Ty, ja: bool) -> String {
    match ty {
        Ty::Money { cur, .. } => format!("{}{cur}", fmt_big(v, ja)),
        Ty::Qty { unit, .. } => format!("{v}{unit}"),
        Ty::Rate => format!("{}%", v.mul(Rat::int(100))),
        _ => format!("{v}"),
    }
}

/// Lower `range >=a <=b` to an interval. Open bounds are treated as closed (overestimating
/// is the safe side).
/// Also returns the bounds it could not read. A bound that does not fit its declared type —
/// `<=1lb` in a column of grams, `<=0.5円` in one of yen — used to be skipped, and the range
/// came out with one side missing. That side is the universe the completeness proof
/// quantifies over and the entry guard of the generated code, so losing it in silence is
/// the worst shape a mistake can take here.
/// Whether the day `d` lies inside a declared range, both ends included where they are given.
fn held_one(d: i64, lo: Option<Rat>, hi: Option<Rat>) -> bool {
    let v = Rat::int(d as i128);
    lo.is_none_or(|l| l.cmp_to(v) != std::cmp::Ordering::Greater) && hi.is_none_or(|h| v.cmp_to(h) != std::cmp::Ordering::Greater)
}

/// The days of `range from koyomi "<file>" date <name>` on the input `i` (§15.174): read through
/// ritsu's port of dates, recorded as the input's set, with its hull as the input's range. A
/// set that cannot be had stops the check: the tables are not checked over the whole of the
/// days in between instead (E129, E130).
fn koyomi_days(c: &mut Checked, i: &VarDecl, ty: &Ty, from: &crate::ast::DaysFrom, path: &str, at: &dyn Fn(usize) -> String) {
    let name = &i.name.text;
    if !matches!(ty, Ty::Date) {
        c.diags.push(
            Diag::error("E065", tr!("`range from koyomi` を書けるのは日付の入力だけです", "Only a date input takes `range from koyomi`"))
                .at(at(from.span.line))
                .mark(from.span.clone(), tr!("{name} は {ty} です", "{name} is a {ty}"))
                .note(tr!(
                    "koyomi の日付は暦日です。`date` の入力にだけ、その日の集合を範囲にできます。",
                    "A koyomi date is a calendar day, so only a `date` input can take its days as its range."
                )),
        );
        return;
    }
    let dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let file = dir.join(&from.file);
    let shown = file.to_string_lossy().to_string();
    let naming = format!("koyomi \"{}\" date {}", from.file, from.date);
    let stop = |c: &mut Checked, title: String, mark: String, notes: Vec<String>| {
        let mut d = Diag::error("E130", title).at(at(from.span.line)).mark(from.span.clone(), mark);
        for n in notes {
            d = d.note(n);
        }
        d = d.note(tr!(
            "日付の集合を読めないときは、そのあいだのすべての日で確かめ直さずに、ここで止めます。その集合の日だけを考えて書いた表は、すべての日で見ると完全でないことが多いからです。",
            "When the set cannot be read, the check stops here rather than checking every day in between instead: a table written over the set is seldom complete over every day."
        ));
        c.diags.push(d);
    };
    match crate::days::read(&file, &from.date) {
        crate::days::Read::Days { sha256, days } if !days.is_empty() => {
            let (first, last) = (days[0], days[days.len() - 1]);
            c.ranges.insert(name.clone(), (Some(Rat::int(first as i128)), Some(Rat::int(last as i128))));
            c.day_sets.insert(name.clone(), Days { from: Some(from.clone()), file: shown, sha256, days });
        }
        crate::days::Read::Days { .. } => stop(
            c,
            tr!("{naming} がとる日が一つもありません", "{naming} comes to no day at all"),
            tr!("{name} の範囲が空になります", "the range of {name} is empty"),
            vec![],
        ),
        crate::days::Read::NotJoined => c.diags.push(
            Diag::error("E129", tr!("この rulec は koyomi のファイルを読めません", "This rulec reads no koyomi file"))
                .at(at(from.span.line))
                .mark(from.span.clone(), tr!("{name} の範囲は {naming} がとる日です", "the range of {name} is the days {naming} comes to"))
                .note(tr!(
                    "rulec 単独のバイナリには、koyomi が入っていません。`ritsu rulec check …` か `ritsu check …` で走らせると、ritsu を通して koyomi から日の集合を受け取って確かめます。",
                    "The binary of rulec's own crate has no koyomi joined. Run `ritsu rulec check …` or `ritsu check …`, which hand the set of days over through koyomi's port."
                )),
        ),
        crate::days::Read::Refused(said) => {
            let notes = said
                .iter()
                .map(|s| {
                    let code = if s.code.is_empty() { String::new() } else { format!("[{}] ", s.code) };
                    let line = s.line.map(|l| format!(":{l}")).unwrap_or_default();
                    let msg = if crate::i18n::ja() { s.message.ja.clone() } else { s.message.en.clone() };
                    tr!("koyomi の診断: {code}{}{line}: {msg}", "What koyomi says: {code}{}{line}: {msg}", s.file)
                })
                .collect();
            stop(c, tr!("{naming} の日を、koyomi から得られません", "koyomi does not answer for the days of {naming}"), tr!("{shown} を読めないか、koyomi の検査を通りません", "{shown} cannot be read, or does not pass koyomi's check"), notes)
        }
        crate::days::Read::Undecided(why) => {
            let why = if crate::i18n::ja() { why.ja } else { why.en };
            stop(c, tr!("{naming} がとる日を koyomi が数えられません", "koyomi does not count the days {naming} comes to"), tr!("{why}", "{why}"), vec![])
        }
    }
}

/// `range >=classical <=renaissance` on an input of an enum (§15.210): the values it lets in,
/// in the enum's order, become the input's universe and what the generated code takes at its
/// door. An enum with no order has no range to hold a value to (E069), a bound that is none of
/// its values is said (E012, E103), and a range that lets no value in is E069.
fn enum_range(c: &mut Checked, i: &VarDecl, en: &str, r: &Range, at: &dyn Fn(usize) -> String) {
    let place = at(r.span.line);
    if !c.ordered.contains(en) {
        c.diags.push(unordered_comparison(en, &place, &r.span, tr!("範囲", "a range")));
        return;
    }
    let order = c.enums.get(en).cloned().unwrap_or_default();
    let mut ops: Vec<(CmpOp, String)> = Vec::new();
    for (op, l) in &r.bounds {
        match l {
            Lit::Word(w) if order.contains(w) => ops.push((*op, w.clone())),
            Lit::Word(w) => {
                let d = match c.enum_of_value(w) {
                    Some(other) => Diag::error("E103", tr!("範囲の `{w}` は {en} の値ではありません（{other} の値です）", "The bound `{w}` is not a value of {en} (it is one of {other})")),
                    None => Diag::error("E012", tr!("範囲の `{w}` は {en} の値ではありません", "The bound `{w}` is not a value of {en}")),
                };
                c.diags.push(d.at(place.clone()).mark(r.span.clone(), "").note(tr!("{en} の値: {}", "The values of {en}: {}", order.join(" < "))));
                return;
            }
            _ => {
                c.diags.push(
                    Diag::error("E103", tr!("範囲の境界が {en} の値ではありません", "A bound of the range is not a value of {en}"))
                        .at(place.clone())
                        .mark(r.span.clone(), "")
                        .note(tr!("{en} の値: {}", "The values of {en}: {}", order.join(" < "))),
                );
                return;
            }
        }
    }
    let values: Vec<String> = order.iter().filter(|v| crate::ast::ranked_holds(&order, &ops, v)).cloned().collect();
    if values.is_empty() {
        c.diags.push(
            Diag::error("E069", tr!("入力 {} の範囲に入る値がありません", "The range of input {} lets no value in", i.name.text))
                .at(place)
                .mark(r.span.clone(), "")
                .note(tr!("{en} の値は {} の順です。", "The values of {en} are, in order, {}.", order.join(" < "))),
        );
        return;
    }
    c.enum_ranges.insert(i.name.text.clone(), values);
}

fn bounds_of(r: &Range, ty: &Ty) -> ((Option<Rat>, Option<Rat>), Vec<crate::lex::Num>) {
    let (mut lo, mut hi) = (None, None);
    let mut bad = Vec::new();
    for (op, l) in &r.bounds {
        let v = match l {
            Lit::Num(n) => match lit_value_in(n, ty) {
                Some(v) => v,
                None => {
                    bad.push(n.clone());
                    continue;
                }
            },
            Lit::Date(y, m, d) if *ty == Ty::Date => date_ord(*y, *m, *d),
            _ => continue,
        };
        match op {
            CmpOp::Ge | CmpOp::Gt => lo = Some(v),
            CmpOp::Le | CmpOp::Lt => hi = Some(v),
        }
    }
    ((lo, hi), bad)
}

impl Checked {
    /// E112: the declared range must contain the reachable interval obtained by interval
    /// arithmetic over the input ranges. Otherwise the completeness check answers "complete"
    /// without ever seeing the values that actually occur.
    fn derived_range(&mut self, d: &DerivedDecl, ty: &Ty, path: &str) {
        let Some((rl, rh)) = self.interval(&d.expr, ty) else { return };
        let Some(rg) = &d.range else { return };
        let ((dl, dh), bad) = bounds_of(rg, ty);
        for n in bad {
            self.diags.push(
                Diag::error("E103", tr!("範囲の `{}` は {ty} の値ではありません", "The bound `{}` is not a value of {ty}", n.raw))
                    .at(format!("{path}:{}", d.span.line))
                    .mark(d.name.span.clone(), tr!("この宣言の範囲", "the range on this declaration"))
                    .maybe_note(unit_note(&n)),
            );
        }
        let too_low = matches!((dl, Some(rl)), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Less);
        let too_high = matches!((dh, Some(rh)), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Greater);
        if too_low || too_high {
            self.diags.push(
                Diag::error("E112", tr!("導出の範囲が、実際に到達しうる値を含んでいません", "The range of the derived value does not contain the values it can actually reach"))
                    .at(tr!("{path}:{} 導出 {}", "{path}:{} derived value {}", rg.span.line, d.name.text))
                    .fix(
                        crate::diag::FixKind::WidenRange,
                        format!("{} >={} <={}", crate::kw::RANGE, fmt_val_fixed(rl, ty), fmt_val_fixed(rh, ty)),
                    )
                    .mark(rg.span.clone(), tr!("実際に取りうる値は >={} <={} です", "the reachable interval is >={} <={}", fmt_val(rl, ty), fmt_val(rh, ty)))
                    .note(tr!("範囲が狭いと、完全性の検査は、実際に起きる値を見ないまま「完全」と判定します。", "With a range that is too narrow, the completeness check answers \"complete\" without ever seeing the values that actually occur."))
                    .note(tr!(
                        "ヒント: 範囲 >={} <={} に広げてください。起こりえない値まで広げても、検査はその分を除いて調べるので、害はありません。",
                        "Hint: widen the range to >={} <={}. Widening it past what is reachable does no harm; the checks sieve that part out as an infeasible region.",
                        fmt_val(rl, ty),
                        fmt_val(rh, ty)
                    )),
            );
        }
    }

    /// What a row writes into an output column of numbers, as an interval, when its cell there
    /// names one of the table's own columns of numbers and the row's cell on that column lets
    /// through less than the column's range: the range narrowed to what the cell lets through
    /// (§15.213). `| >=1 | units |` writes `units` only at one or more, so a table that picks
    /// another divisor where `units` is 0 leaves a column that is never 0. `None` where the cell
    /// narrows nothing, or lets nothing through (a row that never fires).
    pub(crate) fn row_written(&self, t: &Table, row: &Row, oi: usize) -> Option<(Rat, Rat)> {
        let Some(OutCell::Name(w)) = row.outs.get(oi) else { return None };
        let ci = t.inputs.iter().position(|(c, _)| c == w)?;
        let (Some(lo), Some(hi)) = self.ranges.get(w).copied()? else { return None };
        let ty = self.ty_of(w)?;
        if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number) {
            return None;
        }
        let step = Rat::new(1, (*self.scales.get(w).unwrap_or(&1)).max(1));
        let (cl, ch) = cell_hull(row.cells.get(ci)?, &ty, step)?;
        let l = cl.map_or(lo, |c| crate::grid::max_rat(c, lo));
        let h = ch.map_or(hi, |c| crate::grid::min_rat(c, hi));
        let same = l.cmp_to(lo) == std::cmp::Ordering::Equal && h.cmp_to(hi) == std::cmp::Ordering::Equal;
        if same || l.cmp_to(h) == std::cmp::Ordering::Greater {
            return None;
        }
        Some((l, h))
    }

    /// Interval arithmetic from the declared input ranges. The right-hand side of a derived
    /// value is a linear combination of inputs only, so addition, subtraction, and constant
    /// multiples are all that is needed (§5.2).
    pub(crate) fn interval(&self, e: &Expr, ty: &Ty) -> Option<(Rat, Rat)> {
        match e {
            // A name without a range bounds nothing. A rate used to be assumed to lie within
            // 0..100% here, which a table column holding 150% quietly contradicted (§15.34);
            // a column's range now comes from its cells, and a name that still has none
            // leaves the expression unbounded rather than bounded wrongly.
            Expr::Name(n, _) => {
                let (lo, hi) = self.ranges.get(n)?;
                Some(((*lo)?, (*hi)?))
            }
            Expr::Lit(Lit::Num(n), _) => {
                // The expression's own type is only a hint: `商品合計 × 10%` and
                // `税込金額 ÷ 100円` both mix dimensions on purpose (§2.3), so a literal that
                // does not belong to `ty` is read in its own unit instead of giving up.
                let v = lit_value_in(n, ty).or_else(|| lit_value_in(n, &lit_ty(n)))?;
                Some((v, v))
            }
            Expr::Bin(l, op, r, _) => {
                let (al, ah) = self.interval(l, ty)?;
                let (bl, bh) = self.interval(r, ty)?;
                match op {
                    BinOp::Add => Some((al.add(bl), ah.add(bh))),
                    BinOp::Sub => Some((al.sub(bh), ah.sub(bl))),
                    BinOp::Mul | BinOp::Div => {
                        // A divisor that can be zero has no interval, and `Rat::div` asserts
                        // rather than returning one — so `税込金額 ÷ 税込金額` **panicked**
                        // the whole of `rulec check` before E115 could say that a divisor has
                        // to be a constant (DESIGN §15.88). Nothing here may outrun a
                        // diagnostic; the interval is simply not known.
                        // Both ends on one side of zero, too: a range written upside down
                        // (`>=28 <=0`) has a positive low end and a zero high one, and a division
                        // by a value took the zero end into `Rat::div` (§15.213).
                        let above = |v: Rat| v.cmp_to(Rat::zero()) == std::cmp::Ordering::Greater;
                        let below = |v: Rat| v.cmp_to(Rat::zero()) == std::cmp::Ordering::Less;
                        if *op == BinOp::Div && !((above(bl) && above(bh)) || (below(bl) && below(bh))) {
                            return None;
                        }
                        // Minimum and maximum over the endpoint combinations. A rate is not
                        // necessarily non-negative, so all four are examined.
                        let f = |x: Rat, y: Rat| if *op == BinOp::Mul { x.mul(y) } else { x.div(y) };
                        let mut vs = [f(al, bl), f(al, bh), f(ah, bl), f(ah, bh)];
                        vs.sort_by(|a, b| a.cmp_to(*b));
                        Some((vs[0], vs[3]))
                    }
                    _ => None,
                }
            }
            Expr::Call(name, args, _) => {
                use crate::num::RoundMode;
                match (name.as_str(), args.as_slice()) {
                    // A rounded value stays within its argument's bounds pushed out to the
                    // grid, whichever way the mode rounds: down toward zero on the side
                    // nearer zero, up away from it on the other. Without this a callee's
                    // rounded output had no range, and a definition over it in the applying
                    // rule had no range and no scale — and came out at the wrong scale in
                    // every generated language (§15.72).
                    (m, [x, g]) if RoundMode::parse(m).is_some() => {
                        let (lo, hi) = self.interval(x, ty)?;
                        let Expr::Lit(Lit::Num(n), _) = g else { return None };
                        let grid = lit_value_in(n, ty).or_else(|| lit_value_in(n, &lit_ty(n)))?;
                        let out = |v: Rat| v.round_to(if v.num < 0 { RoundMode::Up } else { RoundMode::Down }, grid);
                        let inn = |v: Rat| v.round_to(if v.num < 0 { RoundMode::Down } else { RoundMode::Up }, grid);
                        Some((out(lo), inn(hi)))
                    }
                    // `floor(T × C ÷ S)` with all three non-negative and S positive. The
                    // smallest answer is the smallest amount over the largest whole. The
                    // largest is the amount itself: the typing has already made sure a
                    // `constraint` says C never passes S, so the ratio never passes 1
                    // (§15.102). Reading the ranges alone would give T×C÷S, which is the
                    // product of two ranges and says nothing useful.
                    (crate::kw::ALLOCATE, [t, cc, ss]) => {
                        let (tl, th) = self.interval(t, ty)?;
                        let (cl, _) = self.interval(cc, ty)?;
                        let (sl, sh) = self.interval(ss, ty)?;
                        // Both ends: a range written upside down (`>=1円 <=0円`) has a
                        // positive lower end and a zero upper one, and the division below
                        // panicked on it before the range itself was reported (§15.156).
                        if sl.cmp_to(Rat::zero()) != std::cmp::Ordering::Greater
                            || sh.cmp_to(Rat::zero()) != std::cmp::Ordering::Greater
                        {
                            return None;
                        }
                        // Without the guarantee there is no bound worth stating: E117 has
                        // already refused the rule, and saying nothing here keeps this
                        // arithmetic and the two re-checkers' the same.
                        match (cc, ss) {
                            (Expr::Name(a, _), Expr::Name(b, _))
                                if a == b || self.at_most.contains(&(a.clone(), b.clone())) => {}
                            _ => return None,
                        }
                        let down = |v: Rat| v.round_to(RoundMode::Down, Rat::int(1));
                        Some((down(tl.mul(cl).div(sh)), down(th)))
                    }
                    (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                        let (al, ah) = self.interval(x, ty)?;
                        let (bl, bh) = self.interval(y, ty)?;
                        let pick = |a: Rat, b: Rat| {
                            let less = a.cmp_to(b) == std::cmp::Ordering::Less;
                            if (name == crate::kw::MIN) == less { a } else { b }
                        };
                        Some((pick(al, bl), pick(ah, bh)))
                    }
                    // A remainder: inside one period the argument's interval moved down by the
                    // multiples of the divisor below it, otherwise the whole of 0 to n − 1
                    // (§15.214). The argument is whole (E103), and the ends are read as whole too,
                    // which is what the re-checkers read.
                    (crate::kw::MOD, [x, k]) => {
                        let n = const_value(k)?;
                        let (lo, hi) = self.interval(x, ty)?;
                        let (a, b) = (lo.num.div_euclid(lo.den), hi.num.div_euclid(hi.den));
                        if a.div_euclid(n) == b.div_euclid(n) {
                            let base = a.div_euclid(n) * n;
                            Some((Rat::int(a - base), Rat::int(b - base)))
                        } else {
                            Some((Rat::zero(), Rat::int(n - 1)))
                        }
                    }
                    // A whole square root rises with its argument, so its ends are the roots of the
                    // argument's ends (§15.215). Below zero there is none, and E139 has said so.
                    (crate::kw::ISQRT, [x]) => {
                        let (lo, hi) = self.interval(x, ty)?;
                        Some((isqrt_rat(lo)?, isqrt_rat(hi)?))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// The values a cell on a column of numbers lets through, as the two ends of one interval
/// (§15.213): a literal its value, a comparison what it leaves, a set of values their hull. A
/// strict end is moved inward to the next point of `step`, the finest grid the column's values sit
/// on (its storage step), so `>0` on whole numbers starts at 1. `None` for a cell that says nothing
/// an interval can (`-`, `not:`, `none`).
fn cell_hull(cell: &Cell, ty: &Ty, step: Rat) -> Option<(Option<Rat>, Option<Rat>)> {
    let val = |l: &Lit| match l {
        Lit::Num(n) => lit_value_in(n, ty),
        _ => None,
    };
    let floor = |v: Rat| {
        let q = v.div(step);
        Rat::int(q.num.div_euclid(q.den)).mul(step)
    };
    match cell {
        Cell::Lit(l) => {
            let v = val(l)?;
            Some((Some(v), Some(v)))
        }
        Cell::Set(ls) if !ls.is_empty() => {
            let vs = ls.iter().map(val).collect::<Option<Vec<Rat>>>()?;
            let lo = vs.iter().copied().min_by(|a, b| a.cmp_to(*b))?;
            let hi = vs.iter().copied().max_by(|a, b| a.cmp_to(*b))?;
            Some((Some(lo), Some(hi)))
        }
        Cell::Cmp(atoms) => {
            let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = (None, None);
            for (op, l) in atoms {
                let v = val(l)?;
                let (end, upper) = match op {
                    CmpOp::Ge => (v, false),
                    // The next point of the grid above `v`.
                    CmpOp::Gt => (floor(v).add(step), false),
                    CmpOp::Le => (v, true),
                    // The point of the grid below `v`.
                    CmpOp::Lt => (Rat::zero().sub(floor(Rat::zero().sub(v))).sub(step), true),
                };
                if upper {
                    hi = Some(hi.map_or(end, |h| crate::grid::min_rat(h, end)));
                } else {
                    lo = Some(lo.map_or(end, |l| crate::grid::max_rat(l, end)));
                }
            }
            Some((lo, hi))
        }
        _ => None,
    }
}

/// Whether a public name still has to be given an ASCII alias. One written in ASCII already
/// serves as its own identifier in every target language (§1.3).
fn needs_alias(n: &Name) -> bool {
    if n.ascii.is_some() {
        return false;
    }
    let mut cs = n.text.chars();
    let first_ok = cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    !(first_ok && cs.all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// The value of a constant divisor, which §2.3 requires to be a positive whole number of its
/// own unit: `100円` is 100 and `3` is 3.
fn const_value(e: &Expr) -> Option<i128> {
    let Expr::Lit(Lit::Num(n), _) = e else { return None };
    let v = lit_value_in(n, &lit_ty(n))?;
    if v.den == 1 && v.num > 0 { Some(v.num) } else { None }
}

/// The reciprocal of the step from the declaration: 1000 for `rate[step 0.1%]`.
fn scale_of_type(tr: &TypeRef, ty: &Ty) -> i128 {
    for a in &tr.args {
        if let TypeArg::Scaled(w, n) = a {
            if w == crate::kw::STEP {
                if let Some(v) = lit_value_in(n, ty) {
                    if v.num != 0 {
                        return v.den * (1 / v.num.max(1)).max(1);
                    }
                }
            }
        }
    }
    1
}

/// Whether the type declares a step at all (`rate[step 1%]`), readable or not.
fn has_step(tr: &TypeRef) -> bool {
    tr.args.iter().any(|a| matches!(a, TypeArg::Scaled(w, _) if w == crate::kw::STEP))
}

/// E103 for a rate that is passed in with no step (§15.139). What the caller hands over is one
/// integer count of the step, so a rate input without one counted whole units: `10%` in its
/// range became `1` in the generated guard, and 1000 passed as "at most 10%". The reference
/// said the step would come from the literals, but for an input it cannot — the argument's
/// meaning would then change whenever a row with a finer value was added.
fn missing_step(name: &crate::ast::Name, tr: &TypeRef, ty: &Ty, line: usize, path: &str) -> Option<Diag> {
    if !matches!(ty, Ty::Rate) || has_step(tr) {
        return None;
    }
    Some(
        Diag::error("E103", tr!("率の入力 `{}` に刻みがありません", "The rate input `{}` declares no step", name.text))
            .at(format!("{path}:{line}"))
            .mark(tr.span.clone(), tr!("`rate[step 1%]` のように刻みを書いてください", "write the step, as in `rate[step 1%]`"))
            .note(tr!(
                "生成したコードが受け取るのは、刻みを単位にした整数です（`rate[step 1%]` なら 12% は 12）。刻みが無いと、その整数は 100% を単位に数えることになり、範囲の `10%` も表の境界も表せません。",
                "The generated code takes one integer count of the step (with `rate[step 1%]`, 12% is 12). With no step that integer counted whole units of 100%, so neither `10%` in the range nor any boundary in the table could be written in it."
            ))
            .note(tr!(
                "rulec は、刻みを表のリテラルから決めません。行を一つ足すだけで、呼び出し側が渡す整数の意味が変わってしまうからです。",
                "The step is not taken from the literals in the table: adding a single row would then change what the integer the caller passes means."
            )),
    )
}

/// E061 when no value satisfies the range: the lower end above the upper, or the two equal
/// with one of them strict. A bound that cannot be read is E103's, and is not looked at here.
fn empty_range(r: &crate::ast::Range, ty: &Ty, at: String) -> Option<Diag> {
    let (mut lo, mut hi) = (None, None);
    for (op, l) in &r.bounds {
        let v = match l {
            Lit::Num(n) => lit_value_in(n, ty).or_else(|| lit_value_in(n, &lit_ty(n)))?,
            Lit::Date(y, m, d) => date_ord(*y, *m, *d),
            _ => continue,
        };
        match op {
            CmpOp::Ge => lo = Some((v, true)),
            CmpOp::Gt => lo = Some((v, false)),
            CmpOp::Le => hi = Some((v, true)),
            CmpOp::Lt => hi = Some((v, false)),
        }
    }
    let ((l, l_in), (h, h_in)) = (lo?, hi?);
    let empty = match l.cmp_to(h) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => !(l_in && h_in),
        std::cmp::Ordering::Less => false,
    };
    empty.then(|| {
        Diag::error("E061", tr!("範囲が空です", "The range is empty"))
            .at(at)
            .mark(r.span.clone(), tr!("この範囲に入る値は一つもありません", "no value lies in this range"))
            .note(tr!(
                "下限が上限を超えています。取りうる値が無いと、完全性の証明は空の集合について「漏れなし」と判定し、生成コードの入口はどの呼び出しも受け付けません。",
                "The lower end is past the upper one. With no value to take, the completeness proof answers \"complete\" over nothing, and the generated code's entry refuses every call."
            ))
    })
}

/// E060 for a step or a rounding grid of zero or less.
fn nonpositive(n: &crate::lex::Num, span: Span, at: String, what: String) -> Diag {
    Diag::error("E060", tr!("刻み `{}` が正の値ではありません", "The step `{}` is not positive", n.raw))
        .at(at)
        .mark(span, tr!("{what}は 0 より大きい値です", "{what} has to be greater than zero"))
        .note(tr!(
            "刻みは、実行時の整数が何を 1 と数えるか、どこへ丸めるかを決めます。刻み 0 の丸めは生成コードの中で 0 で割り、実行時に止まっていました。刻み 0 の型は、黙って刻み 1 として読まれていました。",
            "A step decides what one unit of the runtime integer counts, or where a value is rounded to. A rounding grid of zero made the generated code divide by zero at run time, and a step of zero was quietly read as a step of one."
        ))
}

/// The tax of a money type, when it is written and is neither `incl_tax` nor `excl_tax`
/// (`money[円, foo]`). The currency is the first word, the tax the second.
fn unreadable_tax(tr: &TypeRef) -> Option<String> {
    if tr.base != crate::kw::MONEY {
        return None;
    }
    let words: Vec<&String> = tr.args.iter().filter_map(|a| match a {
        TypeArg::Word(w) => Some(w),
        _ => None,
    }).collect();
    words.get(1).filter(|w| ritsu_units::Tax::parse(w).is_none()).map(|w| w.to_string())
}

/// The step, when it is written and cannot be read as a value of the type it steps. It fell
/// back on a scale of 1 above, which for `rate[step 1g]` made the runtime value count whole
/// units and turned the declared `0%..100%` into `0..1` in the generated guard (§15.86).
fn unreadable_step(tr: &TypeRef, ty: &Ty) -> Option<crate::lex::Num> {
    tr.args.iter().find_map(|a| match a {
        TypeArg::Scaled(w, n) if w == crate::kw::STEP && lit_value_in(n, ty).is_none() => {
            Some(n.clone())
        }
        _ => None,
    })
}

impl Checked {
    /// The scale of an expression: the lcm for addition and subtraction, the product for
    /// multiplication, and the divisor's numerator times for division by a constant.
    pub(crate) fn scale(&self, e: &Expr) -> Option<i128> {
        fn lcm(a: i128, b: i128) -> i128 {
            fn g(mut a: i128, mut b: i128) -> i128 {
                while b != 0 {
                    let t = a % b;
                    a = b;
                    b = t;
                }
                a.max(1)
            }
            a / g(a, b) * b
        }
        match e {
            Expr::Name(n, _) => Some(*self.scales.get(n).unwrap_or(&1)),
            Expr::Lit(Lit::Num(n), _) => lit_scale(n),
            Expr::Bin(l, op, r, _) => {
                let a = self.scale(l)?;
                let b = self.scale(r)?;
                Some(match op {
                    BinOp::Add | BinOp::Sub => lcm(a, b),
                    BinOp::Mul => a * b,
                    // Dividing by the constant k does not shrink the stored integer: it
                    // multiplies the scale by k, so the division is exact and no language's
                    // rounding convention gets a say (§7.1). The divisor is a constant —
                    // §2.3 forbids dividing by a variable, and E115 says so.
                    BinOp::Div => a.checked_mul(const_value(r)?)?,
                    _ => 1,
                })
            }
            Expr::Call(name, args, _) => {
                if crate::num::RoundMode::parse(name).is_some() {
                    // After rounding, the scale is back to the grid's step.
                    return args.get(1).and_then(|g| self.scale(g));
                }
                // A share is a whole number of the unit, so its step is 1 whatever the
                // steps of the three it is computed from (§15.102). A remainder and a whole
                // square root are whole numbers too (§15.214, §15.215).
                if name == crate::kw::ALLOCATE || name == crate::kw::MOD || name == crate::kw::ISQRT {
                    return Some(1);
                }
                // `min` and `max` give one of their sides, so the scale both sides sit on, as
                // the generator holds them. The first side's alone read `min(amount, amount ×
                // 10%)` as whole pounds while the code held it in pennies (§15.190).
                if let (crate::kw::MIN | crate::kw::MAX, [x, y]) = (name.as_str(), args.as_slice()) {
                    return Some(lcm(self.scale(x)?, self.scale(y)?));
                }
                args.first().and_then(|a| self.scale(a))
            }
            _ => Some(1),
        }
    }

    /// E108: prove from the declared ranges and scales that an intermediate value fits in
    /// int64. If it cannot be proved, stop: rather than overflowing silently, ask the business
    /// where to round.
    fn overflow(&mut self, e: &Expr, ty: &Ty, span: &Span, path: &str, name: &str) {
        let (Some((lo, hi)), Some(sc)) = (self.interval(e, ty), self.scale(e)) else { return };
        // The stored integer is value × reciprocal of the step. Use whichever endpoint of the
        // interval has the larger absolute value.
        let abs = |r: Rat| if r.num < 0 { Rat::zero().sub(r) } else { r };
        let mag = if abs(lo).cmp_to(abs(hi)) == std::cmp::Ordering::Greater { abs(lo) } else { abs(hi) };
        let stored_r = mag.mul(Rat::int(sc));
        let stored = stored_r.num / stored_r.den;
        if stored > i64::MAX as i128 {
            self.diags.push(
                Diag::error("E108", tr!("{name} が int64 に収まることを証明できません", "Cannot prove that {name} fits in int64"))
                    .at(format!("{path}:{}", span.line))
                    .mark(span.clone(), "")
                    .note(tr!(
                        "実際に取りうる値は {} から {} で、刻みが 1/{sc} なので、格納される整数は最大 {stored} になります。",
                        "The reachable interval is {} to {} and the step is 1/{sc}, so the stored integer reaches {stored}.",
                        fmt_val(lo, ty),
                        fmt_val(hi, ty)
                    ))
                    .note(tr!("ヒント: 入力の範囲を狭めるか、途中で丸めを一つ入れてください。", "Hint: narrow the input ranges, or insert one rounding step along the way."))
                    .note(tr!("どこで丸めるかで答えが変わり、それは業務の判断なので、rulec は勝手に決めません。", "Where to round is a business decision that moves yen, so the tool does not decide it on its own.")),
            );
        }
    }
}

impl Checked {
    /// Walk the atoms of a boolean definition. Conjunction, disjunction, and negation are
    /// allowed; a comparison must have the name of an input or derived value on one side and
    /// a constant on the other.
    fn atoms(&mut self, e: &Expr, owner: &str, path: &str) {
        match e {
            Expr::Bin(l, op, r, sp) => {
                use BinOp::*;
                if matches!(op, Le | Ge | Lt | Gt | Eq) {
                    let axis_of = |x: &Expr, s: &Checked| -> Option<Ty> {
                        let Expr::Name(n, _) = x else { return None };
                        match s.syms.get(n) {
                            Some(v) if matches!(v.kind, SymKind::Input | SymKind::Derived) => {
                                Some(v.ty.clone())
                            }
                            _ => None,
                        }
                    };
                    // First form: a unary test on an axis (a name and a literal).
                    let ok = |x: &Expr, y: &Expr, s: &Checked| -> bool {
                        axis_of(x, s).is_some() && matches!(y, Expr::Lit(..))
                    };
                    // Second form: a comparison between two axes of the same type. Numeric pairs
                    // stay E113, though: what could be analyzed exactly as a derived value must
                    // not be left opaque out of laziness (a discipline of precision, not a matter
                    // of soundness; §5.3).
                    let pair = match (axis_of(l, self), axis_of(r, self)) {
                        (Some(a), Some(b)) if a.unifies(&b) => Some(a),
                        _ => None,
                    };
                    if let Some(t) = &pair {
                        if !t.is_numeric() {
                            return;
                        }
                        self.diags.push(
                            Diag::error("E113", tr!("定義 {owner} が {t} どうしを直接比べています", "Definition {owner} compares two {t} values directly"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(tr!("数値どうしの比較は、差を `{}` として宣言してから定数と比べてください。そのほうが厳密に解析できます。", "To compare two numbers, declare their difference with `{}` and compare that with a constant. It can then be analyzed exactly.", crate::kw::DERIVE))
                                .note(tr!("導出にできない型（日付、列挙）どうしなら、そのまま比べられます。", "Two values of a type that cannot be derived (a date, an enum) may be compared directly.")),
                        );
                        return;
                    }
                    if !ok(l, r, self) && !ok(r, l, self) {
                        let hint = if matches!(**l, Expr::Name(..)) && matches!(**r, Expr::Name(..)) {
                            tr!("入力の線形結合なら `{}` として宣言してから比べてください。表の出力と比べたいなら、その表に真偽の出力列を足すのが正しい書き方です。", "A linear combination of inputs must be declared with `{}` before it is compared. To compare with a table output, the right way is to add a boolean output column to that table.", crate::kw::DERIVE)
                        } else {
                            tr!("比較の片側は入力か導出の名前、もう片側は定数である必要があります。", "One side of the comparison must be the name of an input or derived value, and the other side a constant.")
                        };
                        self.diags.push(
                            Diag::error("E113", tr!("定義 {owner} の条件が、値ひとつと定数の比較になっていません", "The condition of definition {owner} is not one value compared with a constant"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(hint.to_string())
                                .note(tr!("この制限が、完全性と重なりの検査が有限で終わることの土台です。", "This restriction is what makes the completeness and overlap checks finite.")),
                        );
                    }
                    return;
                }
                self.atoms(l, owner, path);
                self.atoms(r, owner, path);
            }
            Expr::Call(_, args, _) => {
                for a in args {
                    self.atoms(a, owner, path);
                }
            }
            _ => {}
        }
    }
}

/// The names an expression reads, in order, with repeats.
pub fn collect_names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Name(n, _) => out.push(n.clone()),
        Expr::Bin(l, _, r, _) => {
            collect_names(l, out);
            collect_names(r, out);
        }
        Expr::Call(_, args, _) => {
            for a in args {
                collect_names(a, out);
            }
        }
        _ => {}
    }
}
