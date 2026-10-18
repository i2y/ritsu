//! The analysis of ranges (DESIGN 5): what the values of every expression of a function can be, as
//! an estimate from above, worked out from what the parameters declare. It reads a module that
//! passes its check of types, and each function once, from its parameters' types and ranges.
//!
//! - The abstract values ([`Abs`], DESIGN 5.1): for a number an interval of exact rationals in the
//!   unit of its type, with a bound on the denominator its values have in lowest terms ([`Iv`]); for
//!   a date an interval of day numbers; for an enum or a bool the values it can be; for a value
//!   that may be none whether it can be, and what it is when it is not; for a list the interval of
//!   its count and what every element can be; a record field by field.
//! - The operations follow the reference interpreter's: a value is in the unit of its expression's
//!   type, read in another unit only where the interpreter reads it so (`interp::coerce`).
//! - An `if` and a `match` narrow, in each branch, the names the condition is about (DESIGN 5.3): a
//!   local and the fields under it. A comparison of two such names is kept as a relation, and a
//!   list `running_totals` answers keeps `before <= upto <= total` on its elements when its weights
//!   are 0 or more.
//! - A function is analysed once; a call of it takes what it answers, cut to the range its result
//!   declares. A function in a ring of calls answers what its result declares (DESIGN 5.4).
//! - Each place where the generated code can fail is kept with the value it is given ([`Place`]):
//!   an input of a rule or a date, a parameter of a function, a function's answer, a field of a
//!   record, `isqrt`'s argument, an operation that can leave int64, a divisor (DESIGN 4.2). Whether
//!   the value keeps what the place takes is shown here; where it is not, `witness.rs` looks for
//!   an input of the function that breaks it (DESIGN 5.7). The places the checks across the
//!   languages decide (a rule's preconditions, a koyomi day given to a rule, an amount of a
//!   transfer, a koyomi date's input) take ritsu-cross's verdicts (`ritsu_ports::Verdicts`, DESIGN
//!   14.5), and wait for them where the run hands over none.
//! - Each call into another language is kept with what it is given ([`CallSite`]): the values,
//!   where each can come from ([`Src`]: a koyomi date's day or a rule's output carried unchanged,
//!   or what kaname computes), and the relations between a rule's inputs; what `Programs` hands
//!   ritsu-cross (DESIGN 14.2).
//! - Where a value is a sum of parameters times constants ([`Lin`]), the analysis says so: the
//!   vectors take the inputs that reach each boundary of a comparison of such values (DESIGN 9), and
//!   the search for an example solves for an input that takes a place's value just outside.

use crate::ast::*;
use crate::check::Checked;
use crate::interp::{common, has_unit};
use crate::names::{column_slot, column_values, date_input_slot};
use crate::num::{self, Rat, Rounding};
use crate::suite::Suite;
use crate::ty::*;
use crate::types::Ref;
use crate::value::{Program, Value};
use crate::vectors::{Path as Atom, Step};
use ritsu_base::text::Text;
use ritsu_ports::{ColumnType, DateKind, Found, Precondition};
use ritsu_units::Dim;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

// ── Intervals ────────────────────────────────────────────────────────────────

/// An end of an interval: a value, or none where the interval goes on without end.
pub type End = Option<Rat>;

/// The interval a number or a date can be in, both ends in, and the largest denominator its values
/// have in lowest terms (1 for whole numbers; None where it is not known).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Iv {
    pub lo: End,
    pub hi: End,
    pub den: Option<i128>,
}

/// The magnitude past which an end is no end: far past anything int64 holds, so an interval that
/// reaches it fails the rule of int64 all the same, and the arithmetic of the ends stays in i128.
const CAP: i128 = 1 << 70;
/// The finest denominator an end keeps: a finer one is moved outward onto this grid, which keeps
/// the interval an estimate from above and the products of ends in i128.
const FINE: i128 = 1 << 24;

const I64_MAX: i128 = i64::MAX as i128;
const I64_MIN: i128 = i64::MIN as i128;

/// The order of two values, exact whatever their size.
pub fn cmp(a: Rat, b: Rat) -> Ordering {
    if let Some(o) = a.checked_cmp(b) {
        return o;
    }
    let (qa, qb) = (a.num.div_euclid(a.den), b.num.div_euclid(b.den));
    if qa != qb {
        return qa.cmp(&qb);
    }
    let (ra, rb) = (Rat::new(a.num.rem_euclid(a.den), a.den), Rat::new(b.num.rem_euclid(b.den), b.den));
    match (ra.num == 0, rb.num == 0) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        // two fractions of [0, 1): the larger has the smaller reciprocal
        _ => cmp(Rat::new(rb.den, rb.num), Rat::new(ra.den, ra.num)),
    }
}

/// The largest value on the grid of 1/`den` at or below `v`.
fn floor_to(v: Rat, den: i128) -> Rat {
    let (q, r) = (v.num.div_euclid(v.den), v.num.rem_euclid(v.den));
    let k = match r.checked_mul(den) {
        Some(x) => x / v.den,
        None => r / (v.den / den + 1),
    };
    Rat::checked_new(q.saturating_mul(den).saturating_add(k), den).unwrap_or(Rat::int(q))
}

/// The smallest value on the grid of 1/`den` at or above `v`.
fn ceil_to(v: Rat, den: i128) -> Rat {
    let (q, r) = (v.num.div_euclid(v.den), v.num.rem_euclid(v.den));
    if r == 0 {
        return Rat::int(q);
    }
    let k = match r.checked_mul(den) {
        Some(x) => (x + v.den - 1) / v.den,
        None => r / (v.den / den).max(1) + 1,
    };
    Rat::checked_new(q.saturating_mul(den).saturating_add(k), den).unwrap_or(Rat::int(q.saturating_add(1)))
}

fn too_big(v: Rat) -> bool {
    (v.num / v.den).checked_abs().is_none_or(|m| m > CAP)
}

/// A lower end kept within the arithmetic: past [`CAP`] it is no end, and a finer denominator than
/// [`FINE`] moves down onto that grid.
fn norm_lo(v: End) -> End {
    let v = v?;
    if too_big(v) {
        return None;
    }
    Some(if v.den > FINE { floor_to(v, FINE) } else { v })
}

fn norm_hi(v: End) -> End {
    let v = v?;
    if too_big(v) {
        return None;
    }
    Some(if v.den > FINE { ceil_to(v, FINE) } else { v })
}

fn min_lo(a: End, b: End) -> End {
    match (a, b) {
        (Some(x), Some(y)) => Some(if cmp(x, y).is_le() { x } else { y }),
        _ => None,
    }
}

fn max_hi(a: End, b: End) -> End {
    match (a, b) {
        (Some(x), Some(y)) => Some(if cmp(x, y).is_ge() { x } else { y }),
        _ => None,
    }
}

/// The larger of two lower ends (a narrowing): an end that is none is the other.
fn max_lo(a: End, b: End) -> End {
    match (a, b) {
        (Some(x), Some(y)) => Some(if cmp(x, y).is_ge() { x } else { y }),
        (x, None) | (None, x) => x,
    }
}

fn min_hi(a: End, b: End) -> End {
    match (a, b) {
        (Some(x), Some(y)) => Some(if cmp(x, y).is_le() { x } else { y }),
        (x, None) | (None, x) => x,
    }
}

/// An end of an interval in the arithmetic: below every value, a value, above every value.
#[derive(Clone, Copy, Debug, PartialEq)]
enum X {
    Neg,
    F(Rat),
    Pos,
}

impl X {
    fn sign(self) -> i32 {
        match self {
            X::Neg => -1,
            X::Pos => 1,
            X::F(r) => r.num.signum() as i32,
        }
    }

    fn of_sign(s: i32) -> X {
        if s < 0 {
            X::Neg
        } else if s > 0 {
            X::Pos
        } else {
            X::F(Rat::zero())
        }
    }

    fn mul(self, o: X) -> X {
        match (self, o) {
            (X::F(a), X::F(b)) => match a.checked_mul(b) {
                Some(p) => X::F(p),
                None => X::of_sign(self.sign() * o.sign()),
            },
            _ => X::of_sign(self.sign() * o.sign()),
        }
    }

    fn le(self, o: X) -> bool {
        match (self, o) {
            (X::Neg, _) | (_, X::Pos) => true,
            (X::Pos, _) | (_, X::Neg) => false,
            (X::F(a), X::F(b)) => cmp(a, b).is_le(),
        }
    }
}

impl Iv {
    pub const TOP: Iv = Iv { lo: None, hi: None, den: None };

    pub fn new(lo: End, hi: End, den: Option<i128>) -> Iv {
        Iv { lo: norm_lo(lo), hi: norm_hi(hi), den: den.filter(|d| *d >= 1) }
    }

    pub fn exact(v: Rat) -> Iv {
        Iv::new(Some(v), Some(v), Some(v.den))
    }

    /// Whole numbers from `lo` to `hi`.
    pub fn ints(lo: i128, hi: i128) -> Iv {
        Iv::new(Some(Rat::int(lo)), Some(Rat::int(hi)), Some(1))
    }

    /// The same interval, its values known to have no larger denominator than `d` too.
    pub fn with_den(self, d: Option<i128>) -> Iv {
        let den = match (self.den, d) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        Iv { den, ..self }
    }

    pub fn hull(self, o: Iv) -> Iv {
        let den = match (self.den, o.den) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
        Iv { lo: min_lo(self.lo, o.lo), hi: max_hi(self.hi, o.hi), den }
    }

    /// The values in both; None when there are none.
    pub fn meet(self, o: Iv) -> Option<Iv> {
        let (lo, hi) = (max_lo(self.lo, o.lo), min_hi(self.hi, o.hi));
        if let (Some(a), Some(b)) = (lo, hi)
            && cmp(a, b).is_gt()
        {
            return None;
        }
        Some(Iv { lo, hi, den: Iv::with_den(self, o.den).den })
    }

    /// The interval cut to a declared range (both ends in, None at an open end).
    pub fn cut(self, r: &Range) -> Iv {
        self.meet(Iv { lo: r.min, hi: r.max, den: None }).unwrap_or(self)
    }

    pub fn is_exact(self) -> Option<Rat> {
        match (self.lo, self.hi) {
            (Some(a), Some(b)) if a == b => Some(a),
            _ => None,
        }
    }

    pub fn has_zero(self) -> bool {
        self.lo.is_none_or(|l| l.num <= 0) && self.hi.is_none_or(|h| h.num >= 0)
    }

    /// Whether a value is in the interval (and keeps its bound on the denominator).
    pub fn has(self, v: Rat) -> bool {
        self.lo.is_none_or(|l| cmp(l, v).is_le()) && self.hi.is_none_or(|h| cmp(v, h).is_le()) && self.den.is_none_or(|d| v.den <= d)
    }

    /// Whether every value is within a declared range.
    pub fn within(self, r: &Range) -> bool {
        r.min.is_none_or(|m| self.lo.is_some_and(|l| cmp(l, m).is_ge())) && r.max.is_none_or(|m| self.hi.is_some_and(|h| cmp(h, m).is_le()))
    }

    /// Whether every value keeps the rule of int64 (DESIGN 5.6): in lowest terms, a numerator from
    /// −2⁶³ to 2⁶³ − 1 and a denominator up to 2⁶³ − 1. A value `n/d` with `d` at most the bound
    /// `D` has `n = v·d`, at most `hi·D` above zero and at least `lo·D` below it.
    pub fn fits(self) -> bool {
        let (Some(lo), Some(hi), Some(d)) = (self.lo, self.hi, self.den) else { return false };
        if d > I64_MAX {
            return false;
        }
        let dd = Rat::int(d);
        let low_ok = lo.num >= 0 || lo.checked_mul(dd).is_some_and(|x| cmp(x, Rat::int(I64_MIN)).is_ge());
        let high_ok = hi.num <= 0 || hi.checked_mul(dd).is_some_and(|x| cmp(x, Rat::int(I64_MAX)).is_le());
        low_ok && high_ok
    }

    fn x_lo(self) -> X {
        self.lo.map(X::F).unwrap_or(X::Neg)
    }

    fn x_hi(self) -> X {
        self.hi.map(X::F).unwrap_or(X::Pos)
    }

    fn of_x(lo: X, hi: X, den: Option<i128>) -> Iv {
        let end = |x: X| match x {
            X::F(r) => Some(r),
            _ => None,
        };
        Iv::new(end(lo), end(hi), den)
    }

    fn den_mul(a: Option<i128>, b: Option<i128>) -> Option<i128> {
        a?.checked_mul(b?)
    }

    pub fn plus(self, o: Iv) -> Iv {
        let lo = self.lo.zip(o.lo).and_then(|(a, b)| a.checked_add(b));
        let hi = self.hi.zip(o.hi).and_then(|(a, b)| a.checked_add(b));
        Iv::new(lo, hi, Iv::den_mul(self.den, o.den))
    }

    pub fn negated(self) -> Iv {
        let n = |v: End| v.and_then(|r| r.num.checked_neg().map(|x| Rat { num: x, den: r.den }));
        Iv::new(n(self.hi), n(self.lo), self.den)
    }

    pub fn minus(self, o: Iv) -> Iv {
        self.plus(o.negated())
    }

    pub fn times(self, o: Iv) -> Iv {
        let cands = [self.x_lo().mul(o.x_lo()), self.x_lo().mul(o.x_hi()), self.x_hi().mul(o.x_lo()), self.x_hi().mul(o.x_hi())];
        let mut lo = cands[0];
        let mut hi = cands[0];
        for c in cands {
            if c.le(lo) {
                lo = c;
            }
            if hi.le(c) {
                hi = c;
            }
        }
        Iv::of_x(lo, hi, Iv::den_mul(self.den, o.den))
    }

    /// The reciprocals of an interval that holds no 0.
    fn recip(self) -> Iv {
        let r = |v: X| match v {
            X::F(x) if x.num != 0 => X::F(Rat::new(x.den, x.num)),
            _ => X::F(Rat::zero()),
        };
        Iv::of_x(r(self.x_hi()), r(self.x_lo()), None)
    }

    /// The largest magnitude a value has, None when there is none.
    fn magnitude(self) -> Option<Rat> {
        let (lo, hi) = (self.lo?, self.hi?);
        let a = |r: Rat| Rat { num: r.num.abs(), den: r.den };
        Some(if cmp(a(lo), a(hi)).is_ge() { a(lo) } else { a(hi) })
    }

    /// `self / o` over the values of `o` that are not 0: a value of `o` that is not 0 is at least
    /// 1/D away from it, D its bound on the denominator.
    pub fn over(self, o: Iv) -> Iv {
        // the denominator of a quotient: the dividend's times the divisor's numerator
        let den = (|| {
            let m = o.magnitude()?;
            let d = o.den?;
            let numer = m.checked_mul(Rat::int(d))?;
            let numer = numer.num.div_euclid(numer.den) + 1;
            self.den?.checked_mul(numer)
        })();
        let positive = o.lo.is_some_and(|l| l.num > 0);
        let negative = o.hi.is_some_and(|h| h.num < 0);
        if positive || negative {
            return Iv { den, ..self.times(o.recip()) };
        }
        let Some(d) = o.den else { return Iv::TOP };
        let near = Rat::new(1, d);
        let mut out: Option<Iv> = None;
        if o.lo.is_none_or(|l| l.num < 0) {
            let part = Iv { lo: o.lo, hi: Some(Rat::zero().sub(near)), den: o.den };
            let q = self.times(part.recip());
            out = Some(out.map_or(q, |x| x.hull(q)));
        }
        if o.hi.is_none_or(|h| h.num > 0) {
            let part = Iv { lo: Some(near), hi: o.hi, den: o.den };
            let q = self.times(part.recip());
            out = Some(out.map_or(q, |x| x.hull(q)));
        }
        match out {
            Some(q) => Iv { den, ..q },
            None => Iv { lo: None, hi: None, den },
        }
    }

    /// The interval scaled by a constant.
    pub fn scale(self, f: Rat) -> Iv {
        self.times(Iv::exact(f))
    }

    /// Rounded to the step `g` one of the five ways: each way is monotone, so the ends rounded are
    /// the ends of the values rounded. An end whose rounding leaves int64 is rounded again from the
    /// whole number beside it, outward (below a lower end, above an upper one), and is none where that
    /// leaves int64 too: a whole number inside it would round to an end that values of the interval
    /// can pass (rounding toward 0 to hundredths, `-92929292929292929.6` fits and is above what
    /// `-92929292929292930` rounds to).
    pub fn round(self, way: Rounding, g: Rat) -> Iv {
        let r = |v: End, upper: bool| {
            v.and_then(|x| {
                num::round(x, way, g).ok().or_else(|| {
                    let whole = if upper { -(-x.num).div_euclid(x.den) } else { x.num.div_euclid(x.den) };
                    num::round(Rat::int(whole), way, g).ok()
                })
            })
        };
        let (lo, hi) = (r(self.lo, false), r(self.hi, true));
        Iv::new(if self.lo.is_some() { lo } else { None }, if self.hi.is_some() { hi } else { None }, Some(g.den))
    }

    /// Whether the two intervals have no value in common, and which is below.
    fn below(self, o: Iv) -> bool {
        self.hi.zip(o.lo).is_some_and(|(h, l)| cmp(h, l).is_lt())
    }
}

/// A range as kaname writes it: `>=1 <=1500`, `>=-759 <=1430`, with the ends a person reads (a
/// date as `YYYY-MM-DD`); `any value` where neither end is known.
pub fn iv_text(iv: Iv, t: &Ty) -> Text {
    if iv.lo.is_none() && iv.hi.is_none() {
        return tr!("どの値にもなりうる", "any value");
    }
    Text::same(crate::value::range_text(&Range { min: iv.lo, max: iv.hi }, t))
}

// ── Abstract values ─────────────────────────────────────────────────────────

/// How two fields of a record are known to compare: `before <= upto` for every value of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rel {
    Lt,
    Le,
    Eq,
}

impl Rel {
    pub fn shown(self) -> &'static str {
        match self {
            Rel::Lt => "<",
            Rel::Le => "<=",
            Rel::Eq => "=",
        }
    }
}

/// A record's fields, and what is known of how they compare.
#[derive(Clone, Debug, PartialEq)]
pub struct Rec {
    pub fields: Vec<(String, Abs)>,
    pub rels: Vec<(String, Rel, String)>,
}

/// What an expression's values can be (DESIGN 5.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Abs {
    /// No value: what is never reached.
    Never,
    /// Anything: what the analysis does not follow.
    #[default]
    Any,
    /// A number, in the unit of its expression's type.
    Num(Iv),
    /// A date, as its day number.
    Date(Iv),
    /// The values an enum or a bool can be (`true`, `false`); None for any.
    Set(Option<BTreeSet<String>>),
    Str,
    /// A value that may be none: whether it can be none, and what it is when it is there.
    Opt(bool, Box<Abs>),
    /// A list: the interval of its count, and what each element can be.
    List(Iv, Box<Abs>),
    Record(Rec),
    /// A transfer built: its arguments.
    Transfer(Vec<(String, Abs)>),
}

fn set_of(vs: &[&str]) -> Abs {
    Abs::Set(Some(vs.iter().map(|v| v.to_string()).collect()))
}

impl Abs {
    pub fn bools() -> Abs {
        set_of(&["false", "true"])
    }

    pub fn none() -> Abs {
        Abs::Opt(true, Box::new(Abs::Never))
    }

    pub fn iv(&self) -> Option<Iv> {
        match self {
            Abs::Num(i) | Abs::Date(i) => Some(*i),
            Abs::Opt(_, x) => x.iv(),
            _ => None,
        }
    }

    /// The interval of a number or a date, through optionals and lists: what a value read in another
    /// unit is held to int64 in.
    fn iv_deep(&self) -> Option<Iv> {
        match self {
            Abs::Num(i) | Abs::Date(i) => Some(*i),
            Abs::Opt(_, x) | Abs::List(_, x) => x.iv_deep(),
            _ => None,
        }
    }

    /// The numbers of a value, through lists (an optional kept): what a place for int64 is given
    /// where a value is read in another unit.
    fn num_part(&self) -> Abs {
        match self {
            Abs::List(_, x) => x.num_part(),
            Abs::Opt(n, x) => Abs::Opt(*n, Box::new(x.num_part())),
            x => x.clone(),
        }
    }

    /// Whether a value can be anything where it holds a number (through optionals and lists).
    fn has_any(&self) -> bool {
        match self {
            Abs::Any => true,
            Abs::Opt(_, x) | Abs::List(_, x) => x.has_any(),
            _ => false,
        }
    }

    /// The value when one is there: itself, or what an optional holds.
    pub fn present(&self) -> &Abs {
        match self {
            Abs::Opt(_, x) => x,
            x => x,
        }
    }

    pub fn may_be_none(&self) -> bool {
        matches!(self, Abs::Opt(true, _))
    }

    pub fn field(&self, name: &str) -> Abs {
        match self {
            Abs::Record(r) => r.fields.iter().find(|(n, _)| n == name).map(|(_, a)| a.clone()).unwrap_or(Abs::Any),
            Abs::Opt(_, x) => x.field(name),
            Abs::Never => Abs::Never,
            _ => Abs::Any,
        }
    }

    /// Both: every value of either.
    pub fn join(&self, o: &Abs) -> Abs {
        match (self, o) {
            (Abs::Never, x) | (x, Abs::Never) => x.clone(),
            (Abs::Any, _) | (_, Abs::Any) => Abs::Any,
            (Abs::Num(a), Abs::Num(b)) => Abs::Num(a.hull(*b)),
            (Abs::Date(a), Abs::Date(b)) => Abs::Date(a.hull(*b)),
            (Abs::Set(a), Abs::Set(b)) => Abs::Set(match (a, b) {
                (Some(x), Some(y)) => Some(x.union(y).cloned().collect()),
                _ => None,
            }),
            (Abs::Str, Abs::Str) => Abs::Str,
            (Abs::Opt(n, x), Abs::Opt(m, y)) => Abs::Opt(*n || *m, Box::new(x.join(y))),
            (Abs::Opt(n, x), y) | (y, Abs::Opt(n, x)) => Abs::Opt(*n, Box::new(x.join(y))),
            (Abs::List(c, x), Abs::List(d, y)) => Abs::List(c.hull(*d), Box::new(x.join(y))),
            (Abs::Record(a), Abs::Record(b)) => {
                let fields = a.fields.iter().map(|(n, x)| (n.clone(), b.fields.iter().find(|(m, _)| m == n).map(|(_, y)| x.join(y)).unwrap_or(Abs::Any))).collect();
                let rels = a.rels.iter().filter(|r| b.rels.contains(r)).cloned().collect();
                Abs::Record(Rec { fields, rels })
            }
            (Abs::Transfer(a), Abs::Transfer(b)) => Abs::Transfer(a.iter().map(|(n, x)| (n.clone(), b.iter().find(|(m, _)| m == n).map(|(_, y)| x.join(y)).unwrap_or(Abs::Any))).collect()),
            _ => Abs::Any,
        }
    }

    /// Whether a value of the reference interpreter is among these.
    pub fn has(&self, v: &Value) -> bool {
        match (self, v) {
            (Abs::Never, _) => false,
            (Abs::Any, _) => true,
            (Abs::Opt(n, _), Value::None) => *n,
            (Abs::Opt(_, x), v) => x.has(v),
            (_, Value::None) => false,
            (Abs::Num(i), Value::Num(r)) => i.has(*r),
            (Abs::Date(i), Value::Date(d)) => i.has(Rat::int(*d as i128)),
            (Abs::Set(None), Value::Enum(_) | Value::Bool(_)) => true,
            (Abs::Set(Some(s)), Value::Enum(x)) => s.contains(x),
            (Abs::Set(Some(s)), Value::Bool(b)) => s.contains(&b.to_string()),
            (Abs::Str, Value::Str(_)) => true,
            (Abs::List(c, e), Value::List(xs)) => c.has(Rat::int(xs.len() as i128)) && xs.iter().all(|x| e.has(x)),
            (Abs::Record(r), Value::Record(fs)) => fs.iter().all(|(n, x)| r.fields.iter().find(|(m, _)| m == n).is_none_or(|(_, a)| a.has(x))),
            (Abs::Transfer(args), Value::Transfer(t)) => t.args.iter().all(|(n, x)| args.iter().find(|(m, _)| m == n).is_none_or(|(_, a)| a.has(x))),
            _ => false,
        }
    }

    /// The same values, the numbers read in another unit: times `f`, plus `shift` (the zeros of ℉
    /// and ℃).
    fn scaled(&self, (f, shift): (Rat, Rat)) -> Abs {
        match self {
            Abs::Num(i) => {
                let x = i.scale(f);
                let x = if shift.num == 0 { x } else { x.plus(Iv::exact(shift)) };
                Abs::Num(Iv { den: i.den.and_then(|d| d.checked_mul(f.den)).and_then(|d| d.checked_mul(shift.den)), ..x })
            }
            Abs::Opt(n, x) => Abs::Opt(*n, Box::new(x.scaled((f, shift)))),
            Abs::List(c, x) => Abs::List(*c, Box::new(x.scaled((f, shift)))),
            x => x.clone(),
        }
    }
}

// ── Sums of parameters ───────────────────────────────────────────────────────

/// A value as a constant and a sum of parts of the function's parameters times constants, in the
/// unit of its expression's type (a date as its day number): `paid_on - due` is `paid_on − invoiced
/// − 30` when the date `due` is always 30 days after its input.
#[derive(Clone, Debug, PartialEq)]
pub struct Lin {
    pub k: Rat,
    pub terms: Vec<(Atom, Rat)>,
}

impl Lin {
    pub fn konst(k: Rat) -> Lin {
        Lin { k, terms: Vec::new() }
    }

    pub fn atom(a: Atom) -> Lin {
        Lin { k: Rat::zero(), terms: vec![(a, Rat::int(1))] }
    }

    pub fn add(&self, o: &Lin) -> Option<Lin> {
        let mut terms = self.terms.clone();
        for (a, c) in &o.terms {
            match terms.iter_mut().find(|(b, _)| b == a) {
                Some((_, d)) => *d = d.checked_add(*c)?,
                None => terms.push((a.clone(), *c)),
            }
        }
        terms.retain(|(_, c)| c.num != 0);
        Some(Lin { k: self.k.checked_add(o.k)?, terms })
    }

    pub fn scale(&self, f: Rat) -> Option<Lin> {
        let terms = self.terms.iter().map(|(a, c)| Some((a.clone(), c.checked_mul(f)?))).collect::<Option<Vec<_>>>()?.into_iter().filter(|(_, c)| c.num != 0).collect();
        Some(Lin { k: self.k.checked_mul(f)?, terms })
    }

    pub fn sub(&self, o: &Lin) -> Option<Lin> {
        self.add(&o.scale(Rat::int(-1))?)
    }

    pub fn is_const(&self) -> bool {
        self.terms.is_empty()
    }
}

// ── What the analysis keeps ─────────────────────────────────────────────────

/// What a place is (DESIGN 4.2): where a value is given, and what takes it.
#[derive(Clone, Debug, PartialEq)]
pub enum What {
    /// A numeric input of a rule, with its range.
    RuleInput { rule: String, input: String, range: Range, ty: Ty },
    /// An input of a rule of an ordered enum held to a range over its values (`range >=basic
    /// <=gold`), with the values the range lets in, in the enum's order (the port's
    /// `ColumnType::Enum`).
    RuleEnum { rule: String, input: String, values: Vec<String>, ty: Ty },
    /// A numeric field of the elements of a list a rule walks.
    RuleField { rule: String, list: String, field: String, range: Range, ty: Ty },
    /// A whole-number input of a date of koyomi.
    DateInt { date: String, input: String, range: Range },
    /// A parameter of a function of kaname, as a call gives it.
    Param { function: String, param: String, slot: Slot },
    /// What a function answers, held to the range its result declares.
    Answer { function: String, slot: Slot },
    /// A field of a record of a module, as the record is built.
    Field { record: String, field: String, slot: Slot },
    /// `isqrt`'s argument, 0 or more.
    Isqrt,
    /// An operation whose value may leave int64.
    Overflow,
    /// A divisor (`allocate`'s third argument too).
    Zero,
    /// A precondition of a rule (ritsu-cross's X2).
    Precondition { rule: String, said: String },
    /// A date given to a date input of a rule that has a range (X3 (a)).
    RuleDate { rule: String, input: String },
    /// An amount of a transfer (X4).
    Amount { transfer: String, param: String },
    /// The date given to a date of koyomi (X6).
    DateDay { date: String, input: String },
}

impl What {
    /// The check across the languages that decides it (ritsu's X2, X3, X4, X6), for the places
    /// that wait for ritsu-cross.
    pub fn across(&self) -> Option<&'static str> {
        match self {
            What::Precondition { .. } => Some("X2"),
            What::RuleDate { .. } => Some("X3"),
            What::Amount { .. } => Some("X4"),
            What::DateDay { .. } => Some("X6"),
            _ => None,
        }
    }
}

/// An input of the function that breaks a place, and the value it gives the place there (none for
/// an operation that fails).
#[derive(Clone, Debug, PartialEq)]
pub struct Witness {
    pub input: Vec<Value>,
    pub value: Option<Value>,
}

/// What a place comes to (DESIGN 5.7, P5).
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The value keeps what the place takes, for every input: shown.
    Shown,
    /// An input of the function breaks it.
    Example(Box<Witness>),
    /// Neither: the generated code checks it when it runs.
    Undecided,
    /// A check across the languages that ritsu-cross decides, whose answer this run has not had.
    Pending,
}

/// A place where the generated code can fail (DESIGN 4.2), with the value it is given.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    /// The expression the value is, or the operation that computes it.
    pub expr: usize,
    pub span: Span,
    pub kind: FailKind,
    pub what: What,
    /// The values given, in the unit of the place.
    pub value: Abs,
    /// The value as a sum of parameters, when it is one.
    pub lin: Option<Lin>,
    /// The type of the value given, in the unit of the place.
    pub ty: Ty,
    pub outcome: Outcome,
    /// For a place the checks across the languages decide and do not find shown: the code of ritsu's
    /// ledger it is said with (`W203`), and the headline, as ritsu-cross said them (DESIGN 14.4).
    pub across: Option<(String, Text)>,
}

impl Place {
    pub fn failing(&self) -> bool {
        !matches!(self.outcome, Outcome::Shown)
    }
}

/// A comparison of values that are sums of parameters (DESIGN 9): the vectors take the inputs that
/// reach its boundary, a step below it and a step above it.
#[derive(Clone, Debug, PartialEq)]
pub struct Boundary {
    pub expr: usize,
    /// The comparison as written.
    pub text: String,
    /// The left side less the right, in the unit they are compared in.
    pub lin: Lin,
    /// The step the values of `lin` land on.
    pub step: Rat,
}

/// The relations that hold between the inputs a call of a rule is given (DESIGN 5.3): the line, the
/// rule as the module names it, and each `(input, relation, input)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub line: usize,
    pub rule: String,
    pub rels: Vec<(String, Rel, String)>,
}

/// Where a value is carried from unchanged, as the checks across the languages read it (DESIGN
/// 14.2): a koyomi date's day, a rule's numeric output, the outputs of a call of a rule as one record
/// (a field of it is an output), or what kaname computes or the host gives.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Src {
    /// The day a koyomi date answers: the dates file on the disk, and the date.
    Day(PathBuf, String),
    /// A numeric output of a rule: the rule on the disk, and the output.
    Output(PathBuf, String),
    /// The outputs of a call of a rule: the rule on the disk.
    Outputs(PathBuf),
    /// What kaname computes, or the host gives.
    Kaname,
}

/// What a call into another language calls.
#[derive(Clone, Debug, PartialEq)]
pub enum SiteKind {
    /// A rule, on the disk, as the module writes it (`charges`).
    Rule { canon: PathBuf, name: String },
    /// A date of a dates file, on the disk, as the module writes it (`terms.due`), and the date.
    Date { canon: PathBuf, name: String, date: String },
    /// An operation of a transfer of a book, as the module writes it (`ledger.capture.do`).
    Transfer { id: TransferId, name: String },
}

/// One value a call into another language is given.
#[derive(Clone, Debug, PartialEq)]
pub struct Gave {
    /// What takes it: the rule's input (or the list it walks), the date's input, the transfer's
    /// parameter, by the other language's name of it.
    pub to: String,
    pub expr: usize,
    pub span: Span,
    /// The value as written.
    pub shown: String,
    /// The values given, in the unit of what takes them, and its type.
    pub value: Abs,
    pub ty: Ty,
    /// Where it can come from, as an `if` or a `match` joins them; empty for what kaname computes.
    pub from: Vec<Src>,
}

/// A call into another language (DESIGN 14.2): the call, what it calls, what it is given, and for a
/// rule the relations that hold between its inputs there (DESIGN 5.3).
#[derive(Clone, Debug, PartialEq)]
pub struct CallSite {
    pub expr: usize,
    pub span: Span,
    pub kind: SiteKind,
    pub given: Vec<Gave>,
    pub relations: Vec<(String, Rel, String)>,
}

/// The reasons an operation of a transfer (a `do` or a `hold`) can be refused with at the amounts a
/// call gives it, as ritsu-cross finds them with chobo's search (X4, DESIGN 6.3): None until the
/// verdict comes, and where it cannot be decided.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub span: Span,
    /// The transfer and the operation, as the book names them (`processing_fee.do`), and the book on
    /// the disk.
    pub op: String,
    pub book: PathBuf,
    pub reasons: Option<Vec<String>>,
}

/// A call of a function of kaname: what the effects follow (DESIGN 4.3).
#[derive(Clone, Debug, PartialEq)]
pub struct FnCall {
    pub line: usize,
    pub module: PathBuf,
    pub function: String,
}

/// What the analysis finds of one function.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FnFacts {
    /// What it answers, cut to the range its result declares.
    pub result: Abs,
    pub places: Vec<Place>,
    pub calls: Vec<FnCall>,
    pub boundaries: Vec<Boundary>,
    /// The relations kept between the inputs of each call of a rule (DESIGN 5.3): what ritsu-cross's
    /// X2 reads.
    pub relations: Vec<Kept>,
    /// Every call into another language, with what it is given (DESIGN 14.2).
    pub sites: Vec<CallSite>,
    /// The reasons each `do` and `hold` it builds can be refused with (DESIGN 6.3).
    pub refusals: Vec<Refusal>,
    pub effect: crate::effects::Effect,
}

/// What the analysis finds of a module.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    pub fns: Vec<FnFacts>,
    /// Every expression's values, by its id: None for one the analysis never reaches.
    pub at: Vec<Option<Abs>>,
}

// ── The analysis of a function ──────────────────────────────────────────────

/// What an expression comes to: its values, the part of a parameter it is (as the vectors name
/// one), the sum of parameters it is, and where it is carried from unchanged (empty for what kaname
/// computes).
#[derive(Clone, Debug)]
struct V {
    abs: Abs,
    path: Option<Atom>,
    lin: Option<Lin>,
    from: Vec<Src>,
}

impl V {
    fn of(abs: Abs) -> V {
        V { abs, path: None, lin: None, from: Vec::new() }
    }

    /// The sum of parameters it is: a number or a date that is a part of one is that part.
    fn sum(&self) -> Option<Lin> {
        match (&self.lin, &self.path, &self.abs) {
            (Some(l), _, _) => Some(l.clone()),
            (None, Some(p), Abs::Num(_) | Abs::Date(_)) => Some(Lin::atom(p.clone())),
            _ => None,
        }
    }
}

/// Where the values of two answers that go to one place come from (an `if`'s two, a `match`'s
/// arms): every place of either, what kaname computes named once. An answer that is no value at all
/// (`none`, or one never reached) comes from nowhere.
fn join_from(a: &[Src], b: &V) -> Vec<Src> {
    if matches!(b.abs, Abs::Never) || b.abs == Abs::none() {
        return a.to_vec();
    }
    let theirs: Vec<Src> = if b.from.is_empty() { vec![Src::Kaname] } else { b.from.clone() };
    let mut out = a.to_vec();
    for s in theirs {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// A name in scope: its values, the part of a parameter it holds, the sum it is.
#[derive(Clone, Debug)]
struct Local {
    name: String,
    v: V,
}

/// A name and the fields under it, that a branch narrows: `invoice.amount`.
type Sym = (String, Vec<String>);

/// The analysis of the functions of one module.
pub struct Analyzer<'a> {
    m: &'a Checked,
    p: &'a Program,
    suite: &'a Suite,
    /// What each function of the module answers, once it is analysed.
    pub results: Vec<Option<Abs>>,
    pub at: Vec<Option<Abs>>,
    /// The functions of the ring the function analysed now is in.
    ring: BTreeSet<usize>,
    env: Vec<Local>,
    /// What the conditions in force say of how two names compare.
    facts: Vec<(Sym, Rel, Sym)>,
    /// The value an expression came to last, where a condition reads it to narrow.
    last: BTreeMap<usize, V>,
    places: BTreeMap<(usize, String), Place>,
    calls: Vec<FnCall>,
    boundaries: Vec<Boundary>,
    relations: Vec<Kept>,
    sites: Vec<CallSite>,
    outputs: BTreeMap<(PathBuf, String), Iv>,
}

fn ty<'m>(m: &'m Checked, e: &Expr) -> &'m Ty {
    m.typed.types.get(e.id).unwrap_or(&Ty::Error)
}

fn rf<'m>(m: &'m Checked, e: &Expr) -> &'m Ref {
    m.typed.refs.get(e.id).unwrap_or(&Ref::None)
}

/// How a value of `from` is read in `to`, where the interpreter reads it so: times a factor, plus
/// the shift between two zeros (℉ and ℃ alone have one). None where it keeps its value (the same
/// unit, a number and a rate, no unit at all).
fn factor_of(from: &Ty, to: &Ty) -> Option<(Rat, Rat)> {
    let (Ty::Num(a), Ty::Num(b)) = (strip(from), strip(to)) else { return None };
    let dimless = |n: &NumTy| matches!(n.unit.dim, Dim::Number | Dim::Rate);
    if dimless(a) && (dimless(b) || a.konst.is_some_and(|k| k.num == 0)) {
        return None;
    }
    let shift = if a.unit.compares_only() && b.unit.compares_only() { convert(Rat::zero(), &a.unit, &b.unit).unwrap_or(Rat::zero()) } else { Rat::zero() };
    match factor(&a.unit, &b.unit) {
        Some(f) if f != Rat::int(1) || shift.num != 0 => Some((f, shift)),
        _ => None,
    }
}

/// A type with its optionals and lists taken off: what its numbers are.
fn strip(t: &Ty) -> &Ty {
    match t {
        Ty::Opt(x) | Ty::List(x) => strip(x),
        x => x,
    }
}

/// The denominator every value of a numeric type has at most: its step's.
fn step_den(t: &Ty) -> Option<i128> {
    match strip(t) {
        Ty::Num(n) => match (n.konst, n.step) {
            (Some(k), _) => Some(k.den),
            (None, Some(s)) if s.num != 0 => Some(s.den),
            (None, Some(_)) => Some(1),
            _ => None,
        },
        Ty::Date => Some(1),
        _ => None,
    }
}

/// The step the values of a numeric type land on, in its unit: a constant's value, the type's step.
fn step_of(t: &Ty) -> Option<Rat> {
    match strip(t) {
        Ty::Num(n) => match n.konst {
            Some(k) => Some(Rat { num: k.num.abs(), den: k.den }),
            None => n.step,
        },
        Ty::Date => Some(Rat::int(1)),
        _ => None,
    }
}

impl<'a> Analyzer<'a> {
    pub fn new(m: &'a Checked, p: &'a Program, suite: &'a Suite) -> Analyzer<'a> {
        Analyzer {
            m,
            p,
            suite,
            results: vec![None; m.decls.fns.len()],
            at: vec![None; m.module.exprs],
            ring: BTreeSet::new(),
            env: Vec::new(),
            facts: Vec::new(),
            last: BTreeMap::new(),
            places: BTreeMap::new(),
            calls: Vec::new(),
            boundaries: Vec::new(),
            relations: Vec::new(),
            sites: Vec::new(),
            outputs: BTreeMap::new(),
        }
    }

    // ── Types to values ─────────────────────────────────────────────────────

    /// Every value of a type: what a value nothing more is known of can be. A record of a module
    /// keeps the ranges its fields declare, which the interpreter holds every one to as it is built
    /// or comes in.
    pub fn top(&self, t: &Ty) -> Abs {
        self.top_in(t, 0)
    }

    fn top_in(&self, t: &Ty, depth: usize) -> Abs {
        if depth > 8 {
            return Abs::Any;
        }
        match t {
            Ty::Num(n) => match n.konst {
                Some(k) => Abs::Num(Iv::exact(k)),
                None => Abs::Num(Iv { den: step_den(t), ..Iv::TOP }),
            },
            Ty::Date => Abs::Date(Iv { den: Some(1), ..Iv::TOP }),
            Ty::Bool => Abs::bools(),
            Ty::Enum(id) => Abs::Set(Some(self.p.values(id).0.iter().map(|v| v.text.clone()).collect())),
            Ty::Str => Abs::Str,
            Ty::Opt(x) => Abs::Opt(true, Box::new(self.top_in(x, depth))),
            Ty::List(x) => Abs::List(Iv::new(Some(Rat::zero()), None, Some(1)), Box::new(self.top_in(x, depth))),
            Ty::Record(id) => {
                let module = matches!(id, RecId::Module { .. });
                let fields = self
                    .p
                    .fields(id)
                    .iter()
                    .map(|(f, s)| (f.text.clone(), if module { self.slot_in(s, false, depth + 1) } else { self.top_in(&s.ty, depth + 1) }))
                    .collect();
                Abs::Record(Rec { fields, rels: Vec::new() })
            }
            Ty::Transfer(id) => Abs::Transfer(self.p.transfer(id).map(|(_, ps)| ps.iter().map(|(n, t)| (n.clone(), self.top_in(t, depth))).collect()).unwrap_or_default()),
            Ty::None => Abs::none(),
            Ty::Empty => Abs::List(Iv::ints(0, 0), Box::new(Abs::Never)),
            Ty::Error => Abs::Any,
        }
    }

    /// The values a place declares (a parameter, a field, a result): its type, cut to its range and
    /// its count. `reached`: a parameter's, where a rate with a step and no range is held to 0% to
    /// 100% and the fields of the records it reaches to their ranges (DESIGN 3.9, 4.4).
    pub fn slot(&self, s: &Slot, reached: bool) -> Abs {
        self.slot_in(s, reached, 0)
    }

    fn slot_in(&self, s: &Slot, reached: bool, depth: usize) -> Abs {
        if depth > 8 {
            return Abs::Any;
        }
        match &s.ty {
            Ty::Opt(x) => Abs::Opt(true, Box::new(self.slot_in(&Slot { ty: (**x).clone(), ..s.clone() }, reached, depth))),
            Ty::List(x) => {
                let count = match &s.count {
                    Some(c) => Iv::new(c.min.or(Some(Rat::zero())), c.max, Some(1)),
                    None => Iv::new(Some(Rat::zero()), None, Some(1)),
                };
                let elem = self.slot_in(&Slot { ty: (**x).clone(), range: s.range.clone(), count: None }, reached, depth);
                Abs::List(count, Box::new(elem))
            }
            Ty::Num(n) => {
                let range = match (&s.range, reached && n.is_rate() && n.step.is_some()) {
                    (Some(r), _) => Some(r.clone()),
                    (None, true) => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
                    _ => None,
                };
                let iv = Iv { den: step_den(&s.ty), ..Iv::TOP };
                Abs::Num(match range {
                    Some(r) => iv.cut(&r),
                    None => iv,
                })
            }
            Ty::Date => {
                let iv = Iv { den: Some(1), ..Iv::TOP };
                Abs::Date(match &s.range {
                    Some(r) => iv.cut(r),
                    None => iv,
                })
            }
            Ty::Record(id @ RecId::Module { .. }) => {
                let fields = self.p.fields(id).iter().map(|(f, fs)| (f.text.clone(), self.slot_in(fs, reached, depth + 1))).collect();
                Abs::Record(Rec { fields, rels: Vec::new() })
            }
            t => self.top_in(t, depth),
        }
    }

    /// What a function answers when what it answers is not followed (a function in a ring of
    /// calls): what its result declares.
    fn declared(&self, info: &crate::names::FnInfo) -> Abs {
        self.slot(&info.ret, false)
    }

    // ── Names in scope ──────────────────────────────────────────────────────

    fn lookup(&self, name: &str) -> Option<&V> {
        self.env.iter().rev().find(|l| l.name == name).map(|l| &l.v)
    }

    fn bind(&mut self, name: &str, v: V) {
        self.env.push(Local { name: name.to_string(), v });
    }

    /// The name and the fields an expression is, where a branch can narrow it.
    fn sym(&self, e: &Expr) -> Option<Sym> {
        match (&e.kind, rf(self.m, e)) {
            (ExprKind::Var(w), Ref::Local) => Some((w.clone(), Vec::new())),
            (ExprKind::Field(b, _, _), Ref::Field(name)) => {
                let (n, mut fs) = self.sym(b)?;
                fs.push(name.clone());
                Some((n, fs))
            }
            _ => None,
        }
    }

    /// The list a call of `count` with no condition counts, when it is a name.
    fn counted(&self, e: &Expr) -> Option<Sym> {
        let ExprKind::Call(callee, args) = &e.kind else { return None };
        if !matches!(rf(self.m, callee), Ref::Builtin("count")) {
            return None;
        }
        match args.as_slice() {
            [a] => self.sym(a.expr()),
            _ => None,
        }
    }

    /// The values a name and its fields hold now.
    fn sym_abs(&self, s: &Sym) -> Option<Abs> {
        let mut a = self.lookup(&s.0)?.abs.clone();
        for f in &s.1 {
            a = a.field(f);
        }
        Some(a)
    }

    /// Narrow a name and its fields to `abs` from here to the end of the branch.
    fn narrow_to(&mut self, s: &Sym, abs: Abs) {
        let Some(v) = self.lookup(&s.0).cloned() else { return };
        fn put(a: &Abs, path: &[String], to: Abs) -> Abs {
            match path.split_first() {
                None => to,
                Some((f, rest)) => match a {
                    Abs::Record(r) => {
                        let fields = r.fields.iter().map(|(n, x)| (n.clone(), if n == f { put(x, rest, to.clone()) } else { x.clone() })).collect();
                        Abs::Record(Rec { fields, rels: r.rels.clone() })
                    }
                    Abs::Opt(n, x) => Abs::Opt(*n, Box::new(put(x, path, to))),
                    x => x.clone(),
                },
            }
        }
        let abs = put(&v.abs, &s.1, abs);
        self.bind(&s.0, V { abs, ..v });
    }

    // ── Places ──────────────────────────────────────────────────────────────

    fn src(&self, sp: Span) -> String {
        let line = self.m.src.lines().nth(sp.line.saturating_sub(1)).unwrap_or("");
        let cs: Vec<char> = line.chars().collect();
        let (a, b) = (sp.col.saturating_sub(1).min(cs.len()), sp.end.saturating_sub(1).min(cs.len()));
        cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
    }

    /// A place, kept by its expression and what it is: a place reached more than once (a function
    /// given to `reduce`) holds the values of every time.
    #[allow(clippy::too_many_arguments)]
    fn place(&mut self, e: &Expr, key: String, kind: FailKind, what: What, value: Abs, lin: Option<Lin>, t: &Ty) {
        let k = (e.id, key);
        match self.places.get_mut(&k) {
            Some(p) => {
                p.value = p.value.join(&value);
                if p.lin != lin {
                    p.lin = None;
                }
            }
            None => {
                let outcome = if what.across().is_some() { Outcome::Pending } else { Outcome::Shown };
                self.places.insert(k, Place { expr: e.id, span: e.span, kind, what, value, lin, ty: t.clone(), outcome, across: None });
            }
        }
    }

    /// An operation whose value may leave int64, at `e`.
    fn overflow(&mut self, e: &Expr, abs: &Abs, t: &Ty) {
        self.place(e, "overflow".into(), FailKind::Overflow, What::Overflow, abs.clone(), None, t);
    }

    /// An operation held to int64 at `e`: the place, and the value from here on, which the
    /// interpreter lets through only within int64 (a numerator in int64 and a denominator of 1 or
    /// more put the value between −2⁶³ and 2⁶³ − 1).
    fn held(&mut self, e: &Expr, abs: Abs, t: &Ty) -> Abs {
        self.overflow(e, &abs, t);
        match abs {
            Abs::Num(i) => Abs::Num(i.meet(Iv { lo: Some(Rat::int(I64_MIN)), hi: Some(Rat::int(I64_MAX)), den: None }).map(|x| Iv { den: i.den, ..x }).unwrap_or(i)),
            x => x,
        }
    }

    /// What holds once a place has taken a value and the evaluation goes on (DESIGN 5.3): the name
    /// the value is (a local, or a field under one) keeps what the place takes, from here to the end
    /// of the block. A call of a rule that answers had its inputs in their ranges; a record built
    /// had its fields in theirs.
    fn assume(&mut self, x: &Expr, range: &Range, place_ty: &Ty) {
        let Some(s) = self.sym(x) else { return };
        if factor_of(ty(self.m, x), place_ty).is_some() {
            return;
        }
        let Some(now) = self.sym_abs(&s) else { return };
        let narrowed = match now {
            Abs::Num(i) => i.meet(Iv { lo: range.min, hi: range.max, den: None }).map(Abs::Num),
            Abs::Date(i) => i.meet(Iv { lo: range.min, hi: range.max, den: None }).map(Abs::Date),
            _ => None,
        };
        if let Some(n) = narrowed {
            self.narrow_to(&s, n);
        }
    }

    // ── Units ───────────────────────────────────────────────────────────────

    /// A value of `from` read in `to`, as the interpreter reads it there (at `at`, where a value that
    /// leaves int64 fails).
    fn conv(&mut self, v: V, from: &Ty, to: &Ty, at: &Expr) -> V {
        match factor_of(from, to) {
            None => v,
            Some(f) => {
                let abs = v.abs.scaled(f);
                // each number read is held to int64 there: inside a list and an optional too, and
                // where the value can be anything (DESIGN 5.6)
                let past = match abs.iv_deep() {
                    Some(iv) => !iv.fits(),
                    None => abs.has_any(),
                };
                if past {
                    self.overflow(at, &abs.num_part(), to);
                }
                V { abs, lin: v.sum().and_then(|l| l.scale(f.0)).and_then(|l| l.add(&Lin::konst(f.1))), path: None, from: Vec::new() }
            }
        }
    }

    // ── Functions ───────────────────────────────────────────────────────────

    /// The analysis of the function `f`, in the ring `ring` (the functions that call each other
    /// with it; itself alone when none do).
    pub fn function(&mut self, f: usize, ring: &BTreeSet<usize>) -> FnFacts {
        self.ring = ring.clone();
        self.env.clear();
        self.facts.clear();
        self.last.clear();
        self.places.clear();
        self.calls.clear();
        self.boundaries.clear();
        self.relations.clear();
        self.sites.clear();
        let m = self.m;
        let info = &m.decls.fns[f];
        let decl = &m.module.fns[f];
        for (i, prm) in info.params.iter().enumerate() {
            let abs = self.slot(&prm.slot, true);
            self.bind(&prm.name.text, V { abs, path: Some((i, Vec::new())), lin: None, from: Vec::new() });
        }
        let out = self.block(&decl.body);
        let out = self.conv(out, ty(m, &decl.body.answer), &info.ret.ty, &decl.body.answer);
        if info.ret.range.is_some() || info.ret.count.is_some() {
            let what = What::Answer { function: info.name.text.clone(), slot: info.ret.clone() };
            let ans = &decl.body.answer;
            // an `if` or a `match` that answers: each branch's answer is where its value comes from
            self.place(ans, "answer".into(), FailKind::Range, what, out.abs.clone(), out.sum(), &info.ret.ty);
        }
        let declared = self.declared(info);
        let result = cut(&out.abs, &declared);
        let mut places: Vec<Place> = std::mem::take(&mut self.places).into_values().collect();
        for p in &mut places {
            if p.what.across().is_none() {
                p.outcome = if self.shown(p) { Outcome::Shown } else { Outcome::Undecided };
            }
        }
        places.sort_by_key(|p| (p.span.line, p.span.col, p.expr));
        FnFacts {
            result,
            places,
            calls: std::mem::take(&mut self.calls),
            boundaries: std::mem::take(&mut self.boundaries),
            relations: std::mem::take(&mut self.relations),
            refusals: refusals_of(&self.sites),
            sites: std::mem::take(&mut self.sites),
            effect: crate::effects::Effect::default(),
        }
    }

    /// Whether the values a place is given keep what it takes, for every input.
    pub fn shown(&self, p: &Place) -> bool {
        match &p.what {
            What::RuleInput { range, .. } | What::RuleField { range, .. } | What::DateInt { range, .. } => p.value.present().iv().is_some_and(|i| i.within(range)) || matches!(p.value.present(), Abs::Never),
            // every value of the enum it can be is one the range lets in (and none, for an
            // optional input, the rule takes)
            What::RuleEnum { values, .. } => match p.value.present() {
                Abs::Never => true,
                Abs::Set(Some(s)) => s.iter().all(|x| values.contains(x)),
                _ => false,
            },
            What::Param { slot, .. } => self.keeps(&p.value, slot, true),
            What::Answer { slot, .. } | What::Field { slot, .. } => self.keeps(&p.value, slot, false),
            What::Isqrt => matches!(p.value, Abs::Never) || p.value.iv().is_some_and(|i| i.lo.is_some_and(|l| l.num >= 0)),
            What::Overflow => ok_num(&p.value),
            What::Zero => matches!(p.value, Abs::Never) || p.value.iv().is_some_and(|i| !i.has_zero()),
            _ => false,
        }
    }

    /// Whether every value keeps what a place declares, as the interpreter holds a value to it
    /// (`interp::outside`): the range of its numbers and dates, the count of a list, and with
    /// `reached` the fields of the records of a module it reaches.
    pub fn keeps(&self, a: &Abs, s: &Slot, reached: bool) -> bool {
        self.keeps_in(a, s, reached, 0)
    }

    fn keeps_in(&self, a: &Abs, s: &Slot, reached: bool, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let a = match a {
            Abs::Opt(_, x) => &**x,
            x => x,
        };
        if matches!(a, Abs::Never) {
            return true;
        }
        let mut t = &s.ty;
        while let Ty::Opt(x) = t {
            t = x;
        }
        let (elem, elem_ty) = match (t, a) {
            (Ty::List(x), Abs::List(c, e)) => {
                if let Some(r) = &s.count
                    && !c.within(r)
                {
                    return false;
                }
                let mut et = &**x;
                while let Ty::Opt(y) = et {
                    et = y;
                }
                (match &**e {
                    Abs::Opt(_, v) => (**v).clone(),
                    v => v.clone(),
                }, et)
            }
            (Ty::List(_), _) => return false,
            _ => (a.clone(), t),
        };
        if matches!(elem, Abs::Never) {
            return true;
        }
        let range = match (&s.range, elem_ty) {
            (Some(r), _) => Some(r.clone()),
            (None, Ty::Num(n)) if reached && n.is_rate() && n.step.is_some() => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
            _ => None,
        };
        if let Some(r) = &range
            && !elem.iv().is_some_and(|i| i.within(r))
        {
            return false;
        }
        if reached && let Ty::Record(id @ RecId::Module { .. }) = elem_ty {
            for (f, fs) in self.p.fields(id) {
                if !self.keeps_in(&elem.field(&f.text), &fs, true, depth + 1) {
                    return false;
                }
            }
        }
        true
    }

    // ── Blocks and expressions ──────────────────────────────────────────────

    fn block(&mut self, b: &Block) -> V {
        let (mark, fmark) = (self.env.len(), self.facts.len());
        for l in &b.lets {
            let v = self.ev(&l.value);
            // a value that never comes ends the evaluation there: what follows is never reached
            if matches!(v.abs, Abs::Never) {
                self.env.truncate(mark);
                self.facts.truncate(fmark);
                return V::of(Abs::Never);
            }
            let from = ty(self.m, &l.value).clone();
            let bound = self.m.typed.bound.get(&l.value.id).cloned().unwrap_or_else(|| from.clone());
            let v = self.conv(v, &from, &bound, &l.value);
            self.bind(&l.name, v);
        }
        let out = self.ev(&b.answer);
        self.env.truncate(mark);
        self.facts.truncate(fmark);
        out
    }

    /// A block that answers for `e`: its answer read in `e`'s type.
    fn branch(&mut self, e: &Expr, b: &Block) -> V {
        let v = self.block(b);
        let from = ty(self.m, &b.answer).clone();
        let to = ty(self.m, e).clone();
        self.conv(v, &from, &to, &b.answer)
    }

    fn ev(&mut self, e: &Expr) -> V {
        let v = self.ev_inner(e);
        let slot = &mut self.at[e.id];
        *slot = Some(match slot.take() {
            None => v.abs.clone(),
            Some(a) => a.join(&v.abs),
        });
        self.last.insert(e.id, v.clone());
        v
    }

    fn ev_inner(&mut self, e: &Expr) -> V {
        let m = self.m;
        match &e.kind {
            ExprKind::Num(n) => {
                let v = if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value };
                let abs = Abs::Num(Iv::exact(v));
                if !num::fits(v) {
                    self.overflow(e, &abs, ty(m, e));
                }
                V { abs, path: None, lin: Some(Lin::konst(v)), from: Vec::new() }
            }
            ExprKind::Date(d) => V { abs: Abs::Date(Iv::ints(*d as i128, *d as i128)), path: None, lin: Some(Lin::konst(Rat::int(*d as i128))), from: Vec::new() },
            ExprKind::Str(_) => V::of(Abs::Str),
            ExprKind::Bool(b) => V::of(set_of(&[&b.to_string()])),
            ExprKind::None => V::of(Abs::none()),
            ExprKind::Var(w) => match rf(m, e) {
                Ref::EnumValue(_, v) => V::of(set_of(&[v])),
                Ref::Local => self.lookup(w).cloned().unwrap_or(V::of(Abs::Any)),
                _ => V::of(Abs::Any),
            },
            ExprKind::Field(base, _, _) => match rf(m, e) {
                Ref::EnumValue(_, v) => V::of(set_of(&[v])),
                Ref::Field(name) => {
                    let b = self.ev(base);
                    let abs = b.abs.field(name);
                    let path = b.path.map(|(i, mut steps)| {
                        steps.push(Step::Field(name.clone()));
                        (i, steps)
                    });
                    // a field of the outputs of a call of a rule is that output, carried unchanged
                    let from = b.from.iter().filter_map(|s| match s {
                        Src::Outputs(rule) => Some(Src::Output(rule.clone(), name.clone())),
                        _ => None,
                    });
                    V { abs, path, lin: None, from: from.collect() }
                }
                _ => V::of(Abs::Any),
            },
            ExprKind::Call(callee, args) => self.call(e, callee, args),
            ExprKind::Record(_, inits) => self.record(e, inits),
            ExprKind::List(xs) => {
                let elem_ty = match ty(m, e) {
                    Ty::List(t) => (**t).clone(),
                    _ => Ty::Error,
                };
                let mut elem = Abs::Never;
                for x in xs {
                    let v = self.ev(x);
                    if matches!(v.abs, Abs::Never) {
                        return V::of(Abs::Never);
                    }
                    let v = self.conv(v, &ty(m, x).clone(), &elem_ty, x);
                    elem = elem.join(&v.abs);
                }
                V::of(Abs::List(Iv::ints(xs.len() as i128, xs.len() as i128), Box::new(elem)))
            }
            ExprKind::Neg(x) => {
                let v = self.ev(x);
                match v.abs.iv() {
                    Some(i) if matches!(v.abs, Abs::Num(_)) => {
                        let te = ty(m, e).clone();
                        let abs = self.held(e, Abs::Num(i.negated()), &te);
                        V { abs, path: None, lin: v.sum().and_then(|l| l.scale(Rat::int(-1))), from: Vec::new() }
                    }
                    _ => V::of(v.abs),
                }
            }
            ExprKind::Not(x) => {
                let v = self.ev(x);
                V::of(match v.abs {
                    Abs::Set(Some(s)) => Abs::Set(Some(s.iter().map(|b| if b == "true" { "false".to_string() } else { "true".to_string() }).collect())),
                    _ => Abs::bools(),
                })
            }
            ExprKind::In(x, _, _) => {
                let v = self.ev(x);
                let (from, to) = (ty(m, x).clone(), ty(m, e).clone());
                let v = self.conv(v, &from, &to, e);
                // a number held to int64 (`interp`'s `x in u`); a value that may be a number the
                // analysis does not know (anything, or an optional) keeps the place too
                if matches!(v.abs, Abs::Num(_) | Abs::Any | Abs::Opt(..)) {
                    let abs = self.held(e, v.abs.clone(), &to);
                    return V { abs, ..v };
                }
                v
            }
            ExprKind::Bin(a, op, b) => self.binary(e, a, *op, b),
            ExprKind::If(c, t, f) => {
                let cv = self.ev(c);
                if matches!(cv.abs, Abs::Never) {
                    return V::of(Abs::Never);
                }
                let (can_true, can_false) = truths(&cv.abs);
                let mut out = Abs::Never;
                let mut from = Vec::new();
                for (go, b) in [(true, t), (false, f)] {
                    if (go && !can_true) || (!go && !can_false) {
                        continue;
                    }
                    let (mark, fmark) = (self.env.len(), self.facts.len());
                    self.narrow(c, go);
                    let v = self.branch(e, b);
                    out = out.join(&v.abs);
                    from = join_from(&from, &v);
                    self.env.truncate(mark);
                    self.facts.truncate(fmark);
                }
                V { from: if from == [Src::Kaname] { Vec::new() } else { from }, ..V::of(out) }
            }
            ExprKind::Match(s, arms) => self.matching(e, s, arms),
            ExprKind::Lambda(..) => V::of(Abs::Any),
            ExprKind::Block(b) => self.branch(e, b),
        }
    }

    fn binary(&mut self, e: &Expr, a: &Expr, op: BinOp, b: &Expr) -> V {
        let m = self.m;
        if matches!(op, BinOp::And | BinOp::Or) {
            let l = self.ev(a);
            let (lt, lf) = truths(&l.abs);
            // the right is evaluated only when the left does not decide (DESIGN 8)
            let runs = if op == BinOp::And { lt } else { lf };
            let (rt, rf_) = if runs {
                let (mark, fmark) = (self.env.len(), self.facts.len());
                self.narrow(a, op == BinOp::And);
                let r = self.ev(b);
                self.env.truncate(mark);
                self.facts.truncate(fmark);
                truths(&r.abs)
            } else {
                (false, false)
            };
            let (t, f) = if op == BinOp::And { (lt && rt, lf || rf_) } else { (lt || rt, lf && rf_) };
            return V::of(bools(t, f));
        }
        let (ta, tb) = (ty(m, a).clone(), ty(m, b).clone());
        let va = self.ev(a);
        let vb = self.ev(b);
        if op.is_comparison() {
            return self.comparison(e, a, op, b, va, vb, &ta, &tb);
        }
        if matches!(va.abs, Abs::Never) || matches!(vb.abs, Abs::Never) {
            return V::of(Abs::Never);
        }
        match (&va.abs, &vb.abs) {
            (Abs::Date(x), Abs::Date(y)) => {
                let abs = Abs::Num(x.minus(*y).with_den(Some(1)));
                let lin = va.sum().zip(vb.sum()).and_then(|(p, q)| p.sub(&q));
                V { abs, path: None, lin, from: Vec::new() }
            }
            (Abs::Num(_), Abs::Num(_)) => {
                let te = ty(m, e).clone();
                let out = match op {
                    BinOp::Add | BinOp::Sub => {
                        let pa = self.conv(va, &ta, &te, a);
                        let pb = self.conv(vb, &tb, &te, b);
                        let (Some(x), Some(y)) = (pa.abs.iv(), pb.abs.iv()) else { return V::of(Abs::Any) };
                        let mut iv = if op == BinOp::Add { x.plus(y) } else { x.minus(y) };
                        // the difference of two names whose order a condition or a record keeps: `m - n`
                        // where `n < m` is above 0 (DESIGN 5.3); the names as they are, neither read in
                        // another unit
                        if op == BinOp::Sub && factor_of(&ta, &te).is_none() && factor_of(&tb, &te).is_none() {
                            let step = step_of(&te).filter(|s| s.num > 0);
                            match self.relation(b, a) {
                                Some(Rel::Eq) => iv = Iv::exact(Rat::zero()),
                                Some(Rel::Le) => iv = Iv { lo: max_lo(iv.lo, Some(Rat::zero())), ..iv },
                                Some(Rel::Lt) => iv = Iv { lo: max_lo(iv.lo, Some(step.unwrap_or(Rat::zero()))), ..iv },
                                None => {}
                            }
                            match self.relation(a, b) {
                                Some(Rel::Le) => iv = Iv { hi: min_hi(iv.hi, Some(Rat::zero())), ..iv },
                                Some(Rel::Lt) => iv = Iv { hi: min_hi(iv.hi, Some(step.map(|s| Rat::zero().sub(s)).unwrap_or(Rat::zero()))), ..iv },
                                _ => {}
                            }
                        }
                        let lin = pa.sum().zip(pb.sum()).and_then(|(p, q)| if op == BinOp::Add { p.add(&q) } else { p.sub(&q) });
                        V { abs: Abs::Num(iv.with_den(step_den(&te))), path: None, lin, from: Vec::new() }
                    }
                    BinOp::Mul => {
                        let (Some(x), Some(y)) = (va.abs.iv(), vb.abs.iv()) else { return V::of(Abs::Any) };
                        let lin = match (x.is_exact(), y.is_exact()) {
                            (Some(k), _) => vb.sum().and_then(|l| l.scale(k)),
                            (_, Some(k)) => va.sum().and_then(|l| l.scale(k)),
                            _ => None,
                        };
                        V { abs: Abs::Num(x.times(y).with_den(step_den(&te))), path: None, lin, from: Vec::new() }
                    }
                    _ => {
                        // two amounts are read in one unit before one is divided by the other
                        let (pa, pb) = if has_unit(&ta) && has_unit(&tb) {
                            let t = common(&ta, &tb).clone();
                            (self.conv(va, &ta, &t, a), self.conv(vb, &tb, &t, b))
                        } else {
                            (va, vb)
                        };
                        let (Some(x), Some(y)) = (pa.abs.iv(), pb.abs.iv()) else { return V::of(Abs::Any) };
                        self.place(e, "zero".into(), FailKind::Zero, What::Zero, pb.abs.clone(), pb.sum(), &tb);
                        // a divisor that is always 0 leaves the evaluation here
                        if y.is_exact().is_some_and(|k| k.num == 0) {
                            return V::of(Abs::Never);
                        }
                        let lin = y.is_exact().filter(|k| k.num != 0).and_then(|k| pa.sum().and_then(|l| l.scale(Rat::new(k.den, k.num))));
                        V { abs: Abs::Num(x.over(y).with_den(step_den(&te))), path: None, lin, from: Vec::new() }
                    }
                };
                let abs = self.held(e, out.abs, &te);
                V { abs, ..out }
            }
            _ => V::of(Abs::Any),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn comparison(&mut self, e: &Expr, a: &Expr, op: BinOp, b: &Expr, va: V, vb: V, ta: &Ty, tb: &Ty) -> V {
        let t = common(ta, tb).clone();
        let pa = self.conv(va, ta, &t, a);
        let pb = self.conv(vb, tb, &t, b);
        let decided = match (&pa.abs, &pb.abs) {
            (Abs::Num(x), Abs::Num(y)) | (Abs::Date(x), Abs::Date(y)) => decide(*x, op, *y),
            (Abs::Set(Some(x)), Abs::Set(Some(y))) if matches!(op, BinOp::Eq | BinOp::Ne) && x.len() == 1 && y.len() == 1 => Some((x == y) == (op == BinOp::Eq)),
            (Abs::Set(Some(x)), Abs::Set(Some(y))) if matches!(op, BinOp::Eq | BinOp::Ne) && x.is_disjoint(y) => Some(op == BinOp::Ne),
            _ => None,
        };
        // a comparison of sums of parameters is a boundary the vectors reach (DESIGN 9); a part of
        // a parameter compared with a literal is one the vectors take already
        if let (Some(la), Some(lb)) = (pa.sum(), pb.sum())
            && let Some(l) = la.sub(&lb)
            && !l.is_const()
            && !(direct(a, b, &pa, &pb) || direct(b, a, &pb, &pa))
            && let Some(step) = crate::ty::share(step_of_conv(ta, &t), step_of_conv(tb, &t))
            && step.num != 0
        {
            let text = self.src(e.span);
            if !self.boundaries.iter().any(|x| x.expr == e.id) {
                self.boundaries.push(Boundary { expr: e.id, text, lin: l, step });
            }
        }
        V::of(match decided {
            Some(v) => bools(v, !v),
            None => Abs::bools(),
        })
    }

    // ── Narrowing ───────────────────────────────────────────────────────────

    /// What the branch where `c` is `truth` knows (DESIGN 5.3).
    fn narrow(&mut self, c: &Expr, truth: bool) {
        match &c.kind {
            ExprKind::Not(x) => self.narrow(x, !truth),
            ExprKind::Bin(a, BinOp::And, b) if truth => {
                self.narrow(a, true);
                self.narrow(b, true);
            }
            ExprKind::Bin(a, BinOp::Or, b) if !truth => {
                self.narrow(a, false);
                self.narrow(b, false);
            }
            ExprKind::Bin(a, op, b) if op.is_comparison() => self.narrow_cmp(a, *op, b, truth),
            ExprKind::Var(_) | ExprKind::Field(..) if matches!(ty(self.m, c), Ty::Bool) => {
                if let Some(s) = self.sym(c) {
                    self.narrow_to(&s, set_of(&[&truth.to_string()]));
                }
            }
            _ => {}
        }
    }

    fn narrow_cmp(&mut self, a: &Expr, op: BinOp, b: &Expr, truth: bool) {
        let op = if truth { op } else { negate(op) };
        let m = self.m;
        let (ta, tb) = (ty(m, a), ty(m, b));
        let t = common(ta, tb).clone();
        // a comparison that reads one side in another unit (a constant read in the unit of the other)
        // keeps no relation and narrows no count: what it says is of the value read, not of the name
        let own = factor_of(ta, &t).is_none() && factor_of(tb, &t).is_none();
        // `count(xs) = 0` narrows the count of the list `xs`: the guard of an empty list
        if own {
            for (x, y, o) in [(a, b, op), (b, a, flip(op))] {
                if let Some(list) = self.counted(x)
                    && let Some(Abs::List(c, el)) = self.sym_abs(&list)
                    && let Some(Abs::Num(other)) = self.last.get(&y.id).map(|v| v.abs.clone())
                    && let Some(n) = narrow_iv(c, o, other, Some(Rat::int(1)))
                {
                    self.narrow_to(&list, Abs::List(n, el));
                }
            }
        }
        let (sa, sb) = (self.sym(a), self.sym(b));
        if sa.is_none() && sb.is_none() {
            return;
        }
        let read = |me: &Self, x: &Expr, tx: &Ty| -> Option<Abs> {
            let v = me.last.get(&x.id)?;
            Some(match factor_of(tx, &t) {
                Some(f) => v.abs.scaled(f),
                None => v.abs.clone(),
            })
        };
        let (Some(xa), Some(xb)) = (read(self, a, ta), read(self, b, tb)) else { return };
        // a relation is kept between two numbers or two dates (DESIGN 5.3)
        let ordered = matches!((&xa, &xb), (Abs::Num(_), Abs::Num(_)) | (Abs::Date(_), Abs::Date(_)));
        if let (Some(p), Some(q), true) = (&sa, &sb, own && ordered) {
            match op {
                BinOp::Lt => self.facts.push((p.clone(), Rel::Lt, q.clone())),
                BinOp::Le => self.facts.push((p.clone(), Rel::Le, q.clone())),
                BinOp::Gt => self.facts.push((q.clone(), Rel::Lt, p.clone())),
                BinOp::Ge => self.facts.push((q.clone(), Rel::Le, p.clone())),
                BinOp::Eq => self.facts.push((p.clone(), Rel::Eq, q.clone())),
                _ => {}
            }
        }
        // each side, as far as the other bounds it; a side that is no name is not narrowed
        let sides = [(sa, xa.clone(), xb.clone(), op, ta), (sb, xb, xa, flip(op), tb)];
        for (s, me, other, op, tme) in sides {
            let Some(s) = s else { continue };
            // a value read in another unit to be compared (a constant) is not a name: only names are
            // narrowed, and a name is in the unit compared
            if factor_of(tme, &t).is_some() {
                continue;
            }
            let narrowed = match (&me, &other) {
                (Abs::Num(x), Abs::Num(y)) => narrow_iv(*x, op, *y, step_of(tme)).map(Abs::Num),
                (Abs::Date(x), Abs::Date(y)) => narrow_iv(*x, op, *y, Some(Rat::int(1))).map(Abs::Date),
                (Abs::Set(x), Abs::Set(Some(y))) if y.len() == 1 => {
                    let v = y.iter().next().cloned().unwrap_or_default();
                    match op {
                        BinOp::Eq => Some(Abs::Set(Some(x.clone().map(|s| s.into_iter().filter(|w| *w == v).collect()).unwrap_or_else(|| [v].into())))),
                        BinOp::Ne => x.clone().map(|s| Abs::Set(Some(s.into_iter().filter(|w| *w != v).collect()))),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(n) = narrowed {
                self.narrow_to(&s, n);
            }
        }
    }

    // ── match ───────────────────────────────────────────────────────────────

    fn matching(&mut self, e: &Expr, s: &Expr, arms: &[Arm]) -> V {
        let m = self.m;
        let sv = self.ev(s);
        if matches!(sv.abs, Abs::Never) {
            return V::of(Abs::Never);
        }
        let st = ty(m, s).clone();
        let sym = self.sym(s);
        let mut out = Abs::Never;
        let mut from: Vec<Src> = Vec::new();
        for arm in arms {
            let (mark, fmark) = (self.env.len(), self.facts.len());
            let mut reached = false;
            for p in &arm.patterns {
                match p {
                    Pattern::None(_) => {
                        if sv.abs.may_be_none() || matches!(sv.abs, Abs::Any) {
                            reached = true;
                            if let Some(sy) = &sym {
                                self.narrow_to(sy, Abs::none());
                            }
                        }
                    }
                    Pattern::Some(x, _) => {
                        let inner = match &sv.abs {
                            Abs::Opt(_, v) => (**v).clone(),
                            Abs::Any => Abs::Any,
                            v => v.clone(),
                        };
                        if !matches!(inner, Abs::Never) {
                            reached = true;
                            self.bind(x, V { abs: inner, path: sv.path.clone(), lin: None, from: sv.from.clone() });
                        }
                    }
                    _ => {}
                }
            }
            let named: Vec<String> = arm
                .patterns
                .iter()
                .filter_map(|p| match p {
                    Pattern::Bool(b, _) => Some(b.to_string()),
                    Pattern::Value(path, _) => {
                        let word = &path[path.len() - 1].0;
                        match st.present() {
                            Ty::Enum(id) => self.p.values(id).0.iter().find(|n| n.is(word)).map(|n| n.text.clone()),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect();
            if !named.is_empty() {
                let can: Vec<String> = match &sv.abs {
                    Abs::Set(Some(vs)) => named.iter().filter(|n| vs.contains(*n)).cloned().collect(),
                    _ => named.clone(),
                };
                if !can.is_empty() {
                    reached = true;
                    if let Some(sy) = &sym {
                        self.narrow_to(sy, Abs::Set(Some(can.into_iter().collect())));
                    }
                }
            }
            if reached {
                let v = self.branch(e, &arm.body);
                out = out.join(&v.abs);
                from = join_from(&from, &v);
            }
            self.env.truncate(mark);
            self.facts.truncate(fmark);
        }
        V { from: if from == [Src::Kaname] { Vec::new() } else { from }, ..V::of(out) }
    }

    // ── Records ─────────────────────────────────────────────────────────────

    fn record(&mut self, e: &Expr, inits: &[FieldInit]) -> V {
        let m = self.m;
        let Ref::Record(id) = rf(m, e) else { return V::of(Abs::Any) };
        let fields = self.p.fields(id);
        let mut given: Vec<Option<(Abs, Option<Lin>, &Expr)>> = vec![None; fields.len()];
        let mut vals = Vec::new();
        for fi in inits {
            let v = self.ev(&fi.value);
            if matches!(v.abs, Abs::Never) {
                return V::of(Abs::Never);
            }
            vals.push((fi, v));
        }
        for (fi, v) in vals {
            let Some(k) = fields.iter().position(|(n, _)| n.is(&fi.name)) else { continue };
            let v = self.conv(v, &ty(m, &fi.value).clone(), &fields[k].1.ty, &fi.value);
            let lin = v.sum();
            given[k] = Some((v.abs, lin, &fi.value));
        }
        let module = matches!(id, RecId::Module { .. });
        let shown_rec = self.p.show_ty(m, &Ty::Record(id.clone()));
        let mut out = Vec::new();
        for (k, (f, slot)) in fields.iter().enumerate() {
            let abs = match &given[k] {
                Some((a, lin, x)) => {
                    if module && (slot.range.is_some() || slot.count.is_some()) {
                        let what = What::Field { record: shown_rec.clone(), field: f.text.clone(), slot: slot.clone() };
                        self.place(x, format!("field:{}", f.text), FailKind::Range, what, a.clone(), lin.clone(), &slot.ty);
                        if let (Some(r), Ty::Num(_) | Ty::Date) = (&slot.range, &slot.ty) {
                            let (r, t) = (r.clone(), slot.ty.clone());
                            self.assume(x, &r, &t);
                        }
                    }
                    match (a, &slot.range, module) {
                        (Abs::Num(i), Some(r), true) => Abs::Num(i.cut(r)),
                        (Abs::Date(i), Some(r), true) => Abs::Date(i.cut(r)),
                        (a, _, _) => a.clone(),
                    }
                }
                None => Abs::none(),
            };
            out.push((f.text.clone(), abs));
        }
        V::of(Abs::Record(Rec { fields: out, rels: Vec::new() }))
    }

    // ── Calls ───────────────────────────────────────────────────────────────

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> V {
        match rf(self.m, callee) {
            Ref::Fn(canon, name) => self.call_fn(e, canon, name, args),
            Ref::Rule(canon) => self.call_rule(e, callee, canon, args),
            Ref::Date(canon, date) => self.call_date(e, callee, canon, date, args),
            Ref::Transfer(id) => self.call_transfer(e, id, args),
            Ref::Builtin(name) => self.builtin(e, name, args),
            _ => match self.args(args) {
                Some(_) => V::of(Abs::Any),
                None => V::of(Abs::Never),
            },
        }
    }

    /// The arguments of a call, from the left: each its name (as written) and its expression. None
    /// when one never comes: the call is never made, and the arguments after it are never reached.
    fn args<'x>(&mut self, args: &'x [Arg]) -> Option<Vec<(Option<&'x str>, &'x Expr, V)>> {
        let mut out = Vec::new();
        for a in args {
            let v = self.ev(a.expr());
            if matches!(v.abs, Abs::Never) {
                return None;
            }
            out.push(match a {
                Arg::Named(n, _, x) => (Some(n.as_str()), x, v),
                Arg::Pos(x) => (None, x, v),
            });
        }
        Some(out)
    }

    fn call_fn(&mut self, e: &Expr, canon: &std::path::Path, name: &str, args: &[Arg]) -> V {
        let m = self.m;
        let Some(vals) = self.args(args) else { return V::of(Abs::Never) };
        let callee: &Checked = self.p.module(canon);
        let Some(g) = callee.decls.fns.iter().position(|f| f.name.text == name) else { return V::of(Abs::Any) };
        let info = callee.decls.fns[g].clone();
        for (n, x, v) in vals {
            let Some(n) = n else { continue };
            let Some(prm) = info.params.iter().find(|p| p.name.is(n)) else { continue };
            let v = self.conv(v, &ty(m, x).clone(), &prm.slot.ty, x);
            let what = What::Param { function: info.name.text.clone(), param: prm.name.text.clone(), slot: prm.slot.clone() };
            let lin = v.sum();
            self.place(x, format!("param:{}", prm.name.text), FailKind::Range, what, v.abs, lin, &prm.slot.ty);
            if let (Some(r), Ty::Num(_) | Ty::Date) = (&prm.slot.range, &prm.slot.ty) {
                let (r, t) = (r.clone(), prm.slot.ty.clone());
                self.assume(x, &r, &t);
            }
        }
        self.calls.push(FnCall { line: e.span.line, module: canon.to_path_buf(), function: info.name.text.clone() });
        let same = canon == m.canon.as_path();
        let result = if same {
            if self.ring.contains(&g) { None } else { self.results[g].clone() }
        } else {
            callee.analysis.get().and_then(|a| a.fns.get(g)).map(|f| f.result.clone())
        };
        V::of(result.unwrap_or_else(|| self.declared(&info)))
    }

    /// The values a numeric output of a rule comes to, over every input it takes (DESIGN 5.1).
    fn output_iv(&mut self, canon: &std::path::Path, c: &ritsu_ports::Column) -> Iv {
        let key = (canon.to_path_buf(), c.name.clone());
        if let Some(iv) = self.outputs.get(&key) {
            return *iv;
        }
        let t = column_slot(c, canon).ty;
        // on the wire, a rate is a count of its step and anything else a whole number of its unit
        let step = match strip(&t) {
            Ty::Num(n) if n.is_rate() => n.step.unwrap_or(Rat::int(1)),
            _ => Rat::int(1),
        };
        let den = step_den(&t);
        let iv = match self.suite.rules.as_ref().map(|r| r.output_values(&self.p.file(canon), &c.name)) {
            Some(Ok(Found::Value(ov))) => {
                let end = |v: Option<i128>| v.map(|i| Rat::int(i).mul(step));
                Iv::new(end(ov.min), end(ov.max), den)
            }
            _ => Iv { den, ..Iv::TOP },
        };
        self.outputs.insert(key, iv);
        iv
    }

    fn call_rule(&mut self, e: &Expr, callee: &Expr, canon: &std::path::Path, args: &[Arg]) -> V {
        let m = self.m;
        let Some(vals) = self.args(args) else { return V::of(Abs::Never) };
        let Some(facts) = self.p.rules.get(canon).cloned() else { return V::of(Abs::Any) };
        let rule = self.src(callee.span);
        let file = self.p.file(canon);
        let mut given: BTreeMap<String, &Expr> = BTreeMap::new();
        let mut gave = Vec::new();
        for (n, x, v) in vals {
            let Some(n) = n else { continue };
            if let Some(c) = facts.inputs.iter().find(|c| c.name == n || c.alias == n) {
                let slot = column_slot(c, canon);
                let v = self.conv(v, &ty(m, x).clone(), &slot.ty, x);
                given.insert(c.name.clone(), x);
                gave.push(Gave { to: c.name.clone(), expr: x.id, span: x.span, shown: self.src(x.span), value: v.abs.clone(), ty: slot.ty.clone(), from: v.from.clone() });
                let lin = v.sum();
                // a date input whose range is a koyomi date takes the date's days as the rule's
                // precondition, which X2 decides (ritsu's DESIGN 7.5)
                let days = facts.preconditions.iter().any(|p| matches!(p, Precondition::Days { input, .. } if *input == c.name));
                // an input of an ordered enum held to a range over its values (ritsu's DESIGN 3.2)
                let values = column_values(c);
                match (&slot.range, strip(&slot.ty), values) {
                    (Some(r), Ty::Num(_), _) => {
                        let what = What::RuleInput { rule: rule.clone(), input: c.name.clone(), range: r.clone(), ty: slot.ty.clone() };
                        self.place(x, format!("input:{}", c.name), FailKind::Rule, what, v.abs, lin, &slot.ty);
                        if !matches!(slot.ty, Ty::Opt(_)) {
                            self.assume(x, r, &slot.ty);
                        }
                    }
                    (_, Ty::Date, _) if !days => {
                        let ranged = self.suite.rules.as_ref().and_then(|r| r.date_range(&file, &c.name).ok()).is_some_and(|(lo, hi)| lo.is_some() || hi.is_some());
                        if ranged {
                            let what = What::RuleDate { rule: rule.clone(), input: c.name.clone() };
                            self.place(x, format!("rdate:{}", c.name), FailKind::Rule, what, v.abs, lin, &slot.ty);
                        }
                    }
                    // the name is not narrowed after the call: what an enum is narrowed to, only a
                    // `match` or a comparison with one value says (DESIGN 5.4)
                    (None, Ty::Enum(_), Some(values)) => {
                        let what = What::RuleEnum { rule: rule.clone(), input: c.name.clone(), values, ty: slot.ty.clone() };
                        self.place(x, format!("input:{}", c.name), FailKind::Rule, what, v.abs, lin, &slot.ty);
                    }
                    _ => {}
                }
            } else if let Some((seq, _, cols)) = facts.elements.iter().find(|(s, a, _)| s == n || a == n) {
                let elem_ty = Ty::List(Box::new(Ty::Record(RecId::Element(canon.to_path_buf(), seq.clone()))));
                let v = self.conv(v, &ty(m, x).clone(), &elem_ty, x);
                given.insert(seq.clone(), x);
                gave.push(Gave { to: seq.clone(), expr: x.id, span: x.span, shown: self.src(x.span), value: v.abs.clone(), ty: elem_ty.clone(), from: Vec::new() });
                let elem = match &v.abs {
                    Abs::List(_, el) => (**el).clone(),
                    _ => Abs::Any,
                };
                for c in cols {
                    let slot = column_slot(c, canon);
                    if let (Some(r), Ty::Num(_)) = (&slot.range, strip(&slot.ty)) {
                        let what = What::RuleField { rule: rule.clone(), list: seq.clone(), field: c.name.clone(), range: r.clone(), ty: slot.ty.clone() };
                        let fv = elem.field(&c.name);
                        self.place(x, format!("elem:{seq}.{}", c.name), FailKind::Rule, what, fv, None, &slot.ty);
                    }
                }
            }
        }
        // the preconditions are ritsu-cross's to decide (X2); the relations the call keeps between
        // the inputs are what it reads (DESIGN 5.3, 14.3)
        for (i, pre) in facts.preconditions.iter().enumerate() {
            let what = What::Precondition { rule: rule.clone(), said: said(pre) };
            self.place(e, format!("pre:{i}"), FailKind::Rule, what, Abs::Any, None, &Ty::Error);
        }
        let mut rels = Vec::new();
        let names: Vec<(&String, &&Expr)> = given.iter().collect();
        for (i, (na, xa)) in names.iter().enumerate() {
            for (nb, xb) in &names[i + 1..] {
                if let Some(r) = self.relation(xa, xb) {
                    rels.push(((*na).clone(), r, (*nb).clone()));
                } else if let Some(r) = self.relation(xb, xa) {
                    rels.push(((*nb).clone(), r, (*na).clone()));
                }
            }
        }
        if !rels.is_empty() {
            self.relations.push(Kept { line: e.span.line, rule: rule.clone(), rels: rels.clone() });
        }
        self.site(CallSite { expr: e.id, span: e.span, kind: SiteKind::Rule { canon: canon.to_path_buf(), name: rule.clone() }, given: gave, relations: rels });
        let mut fields = Vec::new();
        for c in &facts.outputs {
            let t = column_slot(c, canon).ty;
            let abs = self.column_abs(canon, c, &t);
            fields.push((c.name.clone(), abs));
        }
        V { from: vec![Src::Outputs(canon.to_path_buf())], ..V::of(Abs::Record(Rec { fields, rels: Vec::new() })) }
    }

    /// A call into another language, kept once: a call reached more than once (in a function given
    /// to `reduce`) holds the values of every time, where each comes from, and the relations that
    /// hold every time.
    fn site(&mut self, site: CallSite) {
        match self.sites.iter_mut().find(|x| x.expr == site.expr) {
            None => self.sites.push(site),
            Some(old) => {
                for g in site.given {
                    match old.given.iter_mut().find(|x| x.to == g.to && x.expr == g.expr) {
                        Some(h) => {
                            h.value = h.value.join(&g.value);
                            let other = V { abs: g.value.clone(), path: None, lin: None, from: g.from.clone() };
                            let from = join_from(&if h.from.is_empty() { vec![Src::Kaname] } else { h.from.clone() }, &other);
                            h.from = if from == [Src::Kaname] { Vec::new() } else { from };
                        }
                        None => old.given.push(g),
                    }
                }
                old.relations.retain(|r| site.relations.contains(r));
            }
        }
    }

    /// The values of an output of a rule, by its type.
    fn column_abs(&mut self, canon: &std::path::Path, c: &ritsu_ports::Column, t: &Ty) -> Abs {
        let inner = match &c.ty {
            ColumnType::Opt(_) => match t {
                Ty::Opt(x) => (**x).clone(),
                x => x.clone(),
            },
            _ => t.clone(),
        };
        let v = match &inner {
            Ty::Num(_) => Abs::Num(self.output_iv(canon, c)),
            other => self.top(other),
        };
        if matches!(c.ty, ColumnType::Opt(_)) { Abs::Opt(true, Box::new(v)) } else { v }
    }

    /// How the values of two expressions are known to compare where a call gives them: what a
    /// condition in force says of the names they are, or the fields of one record (the elements of
    /// `running_totals`), followed through each other.
    fn relation(&self, a: &Expr, b: &Expr) -> Option<Rel> {
        let (sa, sb) = (self.sym(a)?, self.sym(b)?);
        let mut edges: Vec<(Sym, Rel, Sym)> = self.facts.clone();
        for s in [&sa, &sb] {
            if s.1.is_empty() {
                continue;
            }
            let base: Sym = (s.0.clone(), s.1[..s.1.len() - 1].to_vec());
            if let Some(Abs::Record(r)) = self.sym_abs(&base) {
                for (x, rel, y) in &r.rels {
                    let mut px = base.clone();
                    px.1.push(x.clone());
                    let mut py = base.clone();
                    py.1.push(y.clone());
                    edges.push((px, *rel, py));
                }
            }
        }
        // the strongest relation from `sa` to `sb` along the edges
        let mut best: BTreeMap<Sym, Rel> = BTreeMap::new();
        best.insert(sa.clone(), Rel::Eq);
        for _ in 0..edges.len() + 1 {
            let mut changed = false;
            for (x, r, y) in &edges {
                for (from, to, rel) in [(x, y, *r), (y, x, *r)] {
                    // an `=` goes both ways; `<` and `<=` only forward
                    if from == y && rel != Rel::Eq {
                        continue;
                    }
                    let Some(have) = best.get(from).copied() else { continue };
                    let next = match (have, rel) {
                        (Rel::Eq, r) | (r, Rel::Eq) => r,
                        (Rel::Lt, _) | (_, Rel::Lt) => Rel::Lt,
                        _ => Rel::Le,
                    };
                    let better = match best.get(to) {
                        None => true,
                        Some(old) => strength(next) > strength(*old),
                    };
                    if better {
                        best.insert(to.clone(), next);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        if sa == sb {
            return Some(Rel::Eq);
        }
        best.get(&sb).copied()
    }

    fn call_date(&mut self, e: &Expr, callee: &Expr, canon: &std::path::Path, date: &str, args: &[Arg]) -> V {
        let m = self.m;
        let Some(vals) = self.args(args) else { return V::of(Abs::Never) };
        let Some(facts) = self.p.dates.get(canon).cloned() else { return V::of(Abs::Any) };
        let shown = self.src(callee.span);
        let file = self.p.file(canon);
        let reads: Vec<String> = facts.functions.iter().find(|f| f.name == date).map(|f| f.params.clone()).unwrap_or_default();
        let mut day_given: Option<V> = None;
        let mut gave = Vec::new();
        for (n, x, v) in vals {
            let Some(n) = n else { continue };
            let Some(input) = facts.inputs.iter().find(|i| i.name == n || i.alias == n) else { continue };
            let Some(slot) = date_input_slot(&facts, &input.name) else { continue };
            let v = self.conv(v, &ty(m, x).clone(), &slot.ty, x);
            gave.push(Gave { to: input.name.clone(), expr: x.id, span: x.span, shown: self.src(x.span), value: v.abs.clone(), ty: slot.ty.clone(), from: v.from.clone() });
            let lin = v.sum();
            match input.kind {
                DateKind::Date => {
                    let what = What::DateDay { date: shown.clone(), input: input.name.clone() };
                    self.place(x, format!("day:{}", input.name), FailKind::Date, what, v.abs.clone(), lin, &slot.ty);
                    if reads.contains(&input.name) {
                        day_given = Some(v);
                    }
                    // a date that answers had its date in the range of its input
                    if let Some(r) = &slot.range {
                        self.assume(x, r, &slot.ty);
                    }
                }
                DateKind::Int => {
                    let range = slot.range.clone().unwrap_or_default();
                    let what = What::DateInt { date: shown.clone(), input: input.name.clone(), range: range.clone() };
                    self.place(x, format!("int:{}", input.name), FailKind::Date, what, v.abs, lin, &slot.ty);
                    self.assume(x, &range, &slot.ty);
                }
            }
        }
        let dates = self.suite.dates.as_ref();
        let hull = match dates.map(|d| d.values(&file, date)) {
            Some(Ok(Found::Value(set))) if !set.is_empty() => {
                let (lo, hi) = (*set.iter().next().unwrap_or(&0), *set.iter().next_back().unwrap_or(&0));
                Iv::ints(lo as i128, hi as i128)
            }
            _ => Iv { den: Some(1), ..Iv::TOP },
        };
        let mut iv = hull;
        let mut lin = None;
        // the days from the date given to the date, over every input koyomi takes (DESIGN 6.2): a
        // date that is always its input and a fixed number of days is that sum
        if let Some(d) = &day_given
            && let Some(Ok(Found::Value((fewest, most)))) = dates.map(|x| x.days(&file, date))
        {
            if let Some(given) = d.abs.iv() {
                let moved = given.plus(Iv::ints(fewest as i128, most as i128));
                iv = iv.meet(moved).unwrap_or(iv);
            }
            if fewest == most {
                lin = d.sum().and_then(|l| l.add(&Lin::konst(Rat::int(fewest as i128))));
            }
        }
        self.site(CallSite { expr: e.id, span: e.span, kind: SiteKind::Date { canon: canon.to_path_buf(), name: shown, date: date.to_string() }, given: gave, relations: Vec::new() });
        V { abs: Abs::Date(iv), path: None, lin, from: vec![Src::Day(canon.to_path_buf(), date.to_string())] }
    }

    fn call_transfer(&mut self, e: &Expr, id: &TransferId, args: &[Arg]) -> V {
        let m = self.m;
        let Some(vals) = self.args(args) else { return V::of(Abs::Never) };
        let params = self.p.transfer(id).map(|(_, ps)| ps).unwrap_or_default();
        let shown = format!("{}.{}", id.transfer, id.op);
        let mut out = Vec::new();
        let mut gave = Vec::new();
        for (n, x, v) in vals {
            let Some(n) = n else { continue };
            let Some((pn, pt)) = params.iter().find(|(p, _)| p == n) else { continue };
            let v = self.conv(v, &ty(m, x).clone(), pt, x);
            if matches!(pt, Ty::Num(_)) {
                let what = What::Amount { transfer: shown.clone(), param: pn.clone() };
                let lin = v.sum();
                self.place(x, format!("amount:{pn}"), FailKind::Range, what, v.abs.clone(), lin, pt);
                gave.push(Gave { to: pn.clone(), expr: x.id, span: x.span, shown: self.src(x.span), value: v.abs.clone(), ty: pt.clone(), from: v.from.clone() });
            }
            out.push((pn.clone(), v.abs));
        }
        let name = self.src(match &e.kind {
            ExprKind::Call(callee, _) => callee.span,
            _ => e.span,
        });
        self.site(CallSite { expr: e.id, span: e.span, kind: SiteKind::Transfer { id: id.clone(), name }, given: gave, relations: Vec::new() });
        V::of(Abs::Transfer(out))
    }

    // ── Builtins ────────────────────────────────────────────────────────────

    /// A function given to a builtin, analysed with its parameters bound to `params`: its answer,
    /// read in `to` where the builtin reads it so.
    fn lambda(&mut self, lam: &Expr, params: Vec<V>, to: Option<&Ty>) -> V {
        let ExprKind::Lambda(ps, body) = &lam.kind else {
            return self.ev(lam);
        };
        let (mark, fmark) = (self.env.len(), self.facts.len());
        for ((p, _), v) in ps.iter().zip(params) {
            self.bind(p, v);
        }
        let v = self.ev(body);
        let v = match to {
            Some(t) => {
                let from = ty(self.m, lam).clone();
                self.conv(v, &from, t, lam)
            }
            None => v,
        };
        self.env.truncate(mark);
        self.facts.truncate(fmark);
        v
    }

    /// A condition given to a builtin (`filter`, `first`), analysed on an element: whether it can
    /// hold and fail, and the element where it holds.
    fn condition(&mut self, lam: &Expr, elem: V) -> (bool, bool, Abs) {
        let ExprKind::Lambda(ps, body) = &lam.kind else { return (true, true, elem.abs) };
        let (mark, fmark) = (self.env.len(), self.facts.len());
        let Some((p, _)) = ps.first() else { return (true, true, elem.abs) };
        self.bind(p, elem.clone());
        let v = self.ev(body);
        let (t, f) = truths(&v.abs);
        self.narrow(body, true);
        let held = self.lookup(p).map(|x| x.abs.clone()).unwrap_or(elem.abs);
        self.env.truncate(mark);
        self.facts.truncate(fmark);
        (t, f, held)
    }

    /// The list a builtin walks: its count, and an element, as the part of a parameter it is (a
    /// count below 0 for a list that never comes).
    fn walked(&mut self, x: &Expr) -> (Iv, V) {
        let v = self.ev(x);
        let (count, elem) = match &v.abs {
            Abs::Never => (Iv::ints(-1, -1), Abs::Never),
            Abs::List(c, el) => (*c, (**el).clone()),
            Abs::Opt(_, inner) => match &**inner {
                Abs::List(c, el) => (*c, (**el).clone()),
                _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any),
            },
            _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any),
        };
        let path = v.path.map(|(i, mut steps)| {
            steps.push(Step::Elem);
            (i, steps)
        });
        (count, V { abs: elem, path, lin: None, from: Vec::new() })
    }

    fn builtin(&mut self, e: &Expr, name: &str, args: &[Arg]) -> V {
        let m = self.m;
        let me = ty(m, e).clone();
        let xs: Vec<&Expr> = args.iter().map(Arg::expr).collect();
        match name {
            "count" | "sum" | "any" | "all" | "map" | "filter" | "first" | "max_of" | "min_of" | "running_totals" if xs.len() == 2 => self.walk(e, name, xs[0], xs[1], &me),
            "reduce" if xs.len() == 3 => self.reduce(e, xs[0], xs[1], xs[2]),
            _ => {
                let mut vals: Vec<V> = Vec::new();
                for x in &xs {
                    let v = self.ev(x);
                    if matches!(v.abs, Abs::Never) {
                        return V::of(Abs::Never);
                    }
                    vals.push(v);
                }
                self.scalar(e, name, &xs, vals, &me)
            }
        }
    }

    fn walk(&mut self, e: &Expr, name: &str, list: &Expr, lam: &Expr, me: &Ty) -> V {
        let (count, elem) = self.walked(list);
        if count.hi.is_some_and(|h| h.num < 0) {
            return V::of(Abs::Never);
        }
        let none_at_all = count.hi.is_some_and(|h| h.num <= 0) || matches!(elem.abs, Abs::Never);
        match name {
            "count" => {
                if none_at_all {
                    let _ = self.condition(lam, elem);
                    return V::of(Abs::Num(Iv::ints(0, 0)));
                }
                let (t, f, _) = self.condition(lam, elem);
                let iv = match (t, f) {
                    (true, false) => count,
                    (false, _) => Iv::ints(0, 0),
                    _ => Iv::new(Some(Rat::zero()), count.hi, Some(1)),
                };
                V::of(Abs::Num(iv))
            }
            "any" | "all" => {
                let (t, f, _) = self.condition(lam, elem);
                // a list with no elements: `any` is false, `all` true
                let some = !none_at_all;
                let empty = none_at_all || count.lo.is_none_or(|l| l.num <= 0);
                let (can_true, can_false) = if name == "any" { (some && t, empty || (some && f)) } else { (empty || (some && t), some && f) };
                V::of(bools(can_true, can_false))
            }
            "filter" => {
                let (_, _, held) = self.condition(lam, elem);
                V::of(Abs::List(Iv::new(Some(Rat::zero()), count.hi, Some(1)), Box::new(held)))
            }
            "first" => {
                let (_, _, held) = self.condition(lam, elem);
                // an element that is none and no element at all are one none (DESIGN 3.6)
                let held = match held {
                    Abs::Opt(_, x) => *x,
                    x => x,
                };
                V::of(Abs::Opt(true, Box::new(held)))
            }
            "map" => {
                let to = match me {
                    Ty::List(t) => Some((**t).clone()),
                    _ => None,
                };
                let v = self.lambda(lam, vec![elem], to.as_ref());
                V::of(Abs::List(count, Box::new(if none_at_all { Abs::Never } else { v.abs })))
            }
            "max_of" | "min_of" => {
                let to = me.present().clone();
                let v = self.lambda(lam, vec![elem], Some(&to));
                let can_be_empty = count.lo.is_none_or(|l| l.num <= 0);
                V::of(Abs::Opt(can_be_empty, Box::new(if none_at_all { Abs::Never } else { v.abs })))
            }
            "sum" => {
                let v = self.lambda(lam, vec![elem], Some(me));
                let w = v.abs.iv().unwrap_or(Iv::TOP);
                // every partial sum is held to int64, as the whole is
                let every = sums(Iv::new(Some(Rat::zero()), count.hi, Some(1)), w, step_den(me));
                self.overflow(e, &Abs::Num(every), me);
                let total = self.held(e, Abs::Num(sums(count, w, step_den(me))), me);
                V::of(if none_at_all { Abs::Num(Iv::ints(0, 0)) } else { total })
            }
            "running_totals" => {
                let w_ty = match me {
                    Ty::List(t) => match &**t {
                        Ty::Record(RecId::Running(_, w)) => (**w).clone(),
                        _ => Ty::Error,
                    },
                    _ => Ty::Error,
                };
                let item = elem.abs.clone();
                let v = self.lambda(lam, vec![elem], Some(&w_ty));
                let w = v.abs.iv().unwrap_or(Iv::TOP);
                let den = step_den(&w_ty);
                let total = sums(count, w, den);
                let before = sums(Iv::new(Some(Rat::zero()), count.hi.map(|h| h.sub(Rat::int(1))).map(|h| if h.num < 0 { Rat::zero() } else { h }), Some(1)), w, den);
                let upto = sums(Iv::new(Some(Rat::int(1)), count.hi, Some(1)), w, den);
                // every partial sum and the whole is held to int64
                let every = Abs::Num(sums(Iv::new(Some(Rat::zero()), count.hi, Some(1)), w, den));
                self.overflow(e, &every, &w_ty);
                let rels = if w.lo.is_some_and(|l| l.num >= 0) { vec![("before".to_string(), Rel::Le, "upto".to_string()), ("upto".to_string(), Rel::Le, "total".to_string())] } else { Vec::new() };
                let rec = Rec { fields: vec![("item".into(), item), ("before".into(), Abs::Num(before)), ("upto".into(), Abs::Num(upto)), ("total".into(), Abs::Num(total))], rels };
                V::of(Abs::List(count, Box::new(if none_at_all { Abs::Never } else { Abs::Record(rec) })))
            }
            _ => V::of(Abs::Any),
        }
    }

    /// `reduce(xs, first, (acc, x) => …)` (DESIGN 5.5): the running value from the first, the
    /// function laid over it as many times as the list can have elements; where that is more than
    /// [`EXACT`] or not known, three times, and an end that still moves is let go.
    fn reduce(&mut self, e: &Expr, list: &Expr, first: &Expr, lam: &Expr) -> V {
        let m = self.m;
        let (count, elem) = self.walked(list);
        if count.hi.is_some_and(|h| h.num < 0) {
            return V::of(Abs::Never);
        }
        let v0 = self.ev(first);
        if matches!(v0.abs, Abs::Never) {
            return V::of(Abs::Never);
        }
        let ft = ty(m, first).clone();
        let mut acc = v0.abs.clone();
        let exact = count.hi.filter(|h| h.num <= EXACT as i128).map(|h| h.num.max(0) as usize);
        let rounds = exact.unwrap_or(3);
        let mut settled = false;
        for _ in 0..rounds {
            let out = self.lambda(lam, vec![V::of(acc.clone()), elem.clone()], Some(&ft));
            let next = acc.join(&out.abs);
            if next == acc {
                settled = true;
                break;
            }
            acc = next;
        }
        if !settled && exact.is_none() {
            acc = widen(&v0.abs, &acc);
            // once more with the ends let go, so that what the function is given is what it can be
            let out = self.lambda(lam, vec![V::of(acc.clone()), elem], Some(&ft));
            acc = widen(&v0.abs, &acc.join(&out.abs));
        }
        let te = ty(m, e).clone();
        self.conv(V::of(acc), &ft, &te, first)
    }

    fn scalar(&mut self, e: &Expr, name: &str, xs: &[&Expr], vals: Vec<V>, me: &Ty) -> V {
        let m = self.m;
        if vals.iter().any(|v| matches!(v.abs, Abs::Never)) {
            return V::of(Abs::Never);
        }
        let iv = |v: &V| v.abs.iv().unwrap_or(Iv::TOP);
        match name {
            "min" | "max" if vals.len() == 2 => {
                let mut it = vals.into_iter();
                let (a, b) = (it.next().unwrap_or(V::of(Abs::Any)), it.next().unwrap_or(V::of(Abs::Any)));
                let a = self.conv(a, &ty(m, xs[0]).clone(), me, xs[0]);
                let b = self.conv(b, &ty(m, xs[1]).clone(), me, xs[1]);
                match (&a.abs, &b.abs) {
                    (Abs::Num(x), Abs::Num(y)) | (Abs::Date(x), Abs::Date(y)) => {
                        let (lo, hi) = if name == "min" { (min_lo(x.lo, y.lo), min_hi(x.hi, y.hi)) } else { (max_lo(x.lo, y.lo), max_hi(x.hi, y.hi)) };
                        let den = x.hull(*y).den;
                        let iv = Iv::new(lo, hi, den);
                        V::of(if matches!(a.abs, Abs::Num(_)) { Abs::Num(iv) } else { Abs::Date(iv) })
                    }
                    _ => V::of(a.abs.join(&b.abs)),
                }
            }
            "mod" if vals.len() == 2 => {
                let n = iv(&vals[1]).is_exact().filter(|k| k.is_int() && k.num > 0).map(|k| k.num);
                let x = iv(&vals[0]);
                let out = match n {
                    Some(n) => {
                        let hi = match (x.lo, x.hi) {
                            (Some(l), Some(h)) if l.num >= 0 && cmp(h, Rat::int(n)).is_lt() => h.num.div_euclid(h.den),
                            _ => n - 1,
                        };
                        Iv::ints(0, hi)
                    }
                    None => Iv { den: Some(1), ..Iv::TOP },
                };
                V::of(Abs::Num(out))
            }
            "isqrt" if vals.len() == 1 => {
                let v = &vals[0];
                let x = iv(v);
                self.place(xs[0], "isqrt".into(), FailKind::Range, What::Isqrt, v.abs.clone(), v.sum(), ty(m, xs[0]));
                let t0 = ty(m, xs[0]).clone();
                self.assume(xs[0], &Range { min: Some(Rat::zero()), max: None }, &t0);
                let lo = x.lo.map(|l| if l.num < 0 { Rat::zero() } else { l }).and_then(num::isqrt).unwrap_or(Rat::zero());
                let hi = x.hi.and_then(num::isqrt);
                V::of(Abs::Num(Iv::new(Some(lo), hi, Some(1))))
            }
            "allocate" if vals.len() == 3 => {
                let (tc, ts) = (ty(m, xs[1]).clone(), ty(m, xs[2]).clone());
                let both = common(&tc, &ts).clone();
                let mut it = vals.into_iter();
                let (t, c, s) = (it.next().unwrap_or(V::of(Abs::Any)), it.next().unwrap_or(V::of(Abs::Any)), it.next().unwrap_or(V::of(Abs::Any)));
                let c = self.conv(c, &tc, &both, xs[1]);
                let s = self.conv(s, &ts, &both, xs[2]);
                self.place(e, "zero".into(), FailKind::Zero, What::Zero, s.abs.clone(), s.sum(), &both);
                let (ti, ci, si) = (iv(&t), iv(&c), iv(&s));
                let product = ti.times(ci);
                let quotient = product.over(si);
                let rounded = quotient.round(Rounding::Down, Rat::int(1));
                // `t × c`, the quotient and its rounding are each held to int64
                let worst = [product, quotient].into_iter().find(|i| !i.fits()).unwrap_or(rounded);
                let te = ty(m, xs[0]).clone();
                self.overflow(e, &Abs::Num(worst), &te);
                if si.is_exact().is_some_and(|k| k.num == 0) {
                    return V::of(Abs::Never);
                }
                let rounded = self.held(e, Abs::Num(rounded), &te);
                self.conv(V::of(rounded), &te, me, e)
            }
            "up" | "down" | "half_up" | "half_down" | "half_even" if vals.len() == 2 => {
                let tx = ty(m, xs[0]).clone();
                let g = self.conv(vals[1].clone(), &ty(m, xs[1]).clone(), &tx, xs[1]);
                let way = Rounding::parse(name).unwrap_or(Rounding::Down);
                let Some(grid) = iv(&g).is_exact().filter(|g| g.num > 0) else { return V::of(self.top(me)) };
                let out = iv(&vals[0]).round(way, grid);
                let abs = self.held(e, Abs::Num(out), &tx);
                self.conv(V::of(abs), &tx, me, e)
            }
            "count" => V::of(match vals.first().map(|v| &v.abs) {
                Some(Abs::List(c, _)) => Abs::Num(*c),
                _ => Abs::Num(Iv::new(Some(Rat::zero()), None, Some(1))),
            }),
            "starts_with" => V::of(Abs::bools()),
            "concat" | "text" => V::of(Abs::Str),
            _ => V::of(self.top(me)),
        }
    }
}

// ── A module ────────────────────────────────────────────────────────────────

/// The analysis of a module whose names and types pass (DESIGN 4, 5, 7): each function's values and
/// places, ring of calls by ring, the functions a ring calls first; the examples looked for where a
/// place is not shown; the effects; and the diagnostics of all of it (E301–W303, E401, W501).
pub fn analyze(c: &std::rc::Rc<Checked>, suite: &Suite) -> (Analysis, Vec<crate::diag::Diag>) {
    let p = Program::of(c);
    let rings = crate::effects::rings(c);
    let mut an = Analyzer::new(c, &p, suite);
    let mut fns: Vec<FnFacts> = vec![FnFacts::default(); c.decls.fns.len()];
    for (ring, _) in &rings {
        let set: BTreeSet<usize> = ring.iter().copied().collect();
        for &f in ring {
            let mut facts = an.function(f, &set);
            an.results[f] = Some(facts.result.clone());
            crate::witness::search(&p, suite, c, f, &mut facts.places, &facts.boundaries);
            fns[f] = facts;
        }
    }
    // what the checks across the languages came to, where the run hands it over (DESIGN 14.5)
    if let Some(v) = &suite.verdicts {
        let verdicts = v.verdicts(std::path::Path::new(&c.path));
        across(c, &p, suite, &mut fns, &verdicts);
    }
    crate::effects::infer(c, &mut fns, &rings);
    let mut diags = Vec::new();
    let mut said: BTreeMap<(usize, usize), (Text, String)> = BTreeMap::new();
    for (f, facts) in fns.iter().enumerate() {
        for (i, pl) in facts.places.iter().enumerate() {
            if let Some((d, head)) = place_diag(c, &p, f, pl) {
                said.insert((f, i), (head, d.code.to_string()));
                diags.push(d);
            } else if let Some((code, head)) = &pl.across {
                // ritsu-cross says it, in ritsu's codes: `ritsu W203`
                said.insert((f, i), (head.clone(), format!("ritsu {code}")));
            }
        }
    }
    diags.extend(crate::effects::written_holds(c, &fns, &rings, &said));
    diags.extend(crate::hatch::check(c));
    (Analysis { fns, at: std::mem::take(&mut an.at) }, diags)
}

/// The check across the languages a place is decided by, and what at the place it is about, as the
/// verdicts name them (DESIGN 14.5).
pub fn crossing_of(what: &What) -> Option<(ritsu_ports::Crossing, &str)> {
    use ritsu_ports::Crossing;
    Some(match what {
        What::Precondition { said, .. } => (Crossing::Precondition, said.as_str()),
        What::RuleDate { input, .. } => (Crossing::RuleDay, input.as_str()),
        What::Amount { param, .. } => (Crossing::Amount, param.as_str()),
        What::DateDay { input, .. } => (Crossing::DateDay, input.as_str()),
        _ => return None,
    })
}

/// The reasons each `do` and `hold` of a function can be refused with: none known until the
/// verdicts come.
fn refusals_of(sites: &[CallSite]) -> Vec<Refusal> {
    sites
        .iter()
        .filter_map(|s| match &s.kind {
            SiteKind::Transfer { id, .. } if id.op == "do" || id.op == "hold" => Some(Refusal { span: s.span, op: format!("{}.{}", id.transfer, id.op), book: id.book.clone(), reasons: None }),
            _ => None,
        })
        .collect()
}

/// The verdicts of the checks across the languages, taken by the places they decide (DESIGN 4.3):
/// shown, an example (the run of the function's vectors that breaks it), or undecided; and the
/// reasons each `do` and `hold` can be refused with. A place no verdict names waits.
fn across(c: &Checked, p: &Program, suite: &Suite, fns: &mut [FnFacts], verdicts: &[ritsu_ports::CallVerdict]) {
    use ritsu_ports::{Answer, At, Crossing};
    for (f, facts) in fns.iter_mut().enumerate() {
        for pl in facts.places.iter_mut() {
            let Some((check, about)) = crossing_of(&pl.what) else { continue };
            let at = At { line: pl.span.line, col: pl.span.col };
            let Some(v) = verdicts.iter().find(|v| v.at == at && v.check == check && v.about == about) else { continue };
            match &v.outcome {
                Answer::Holds => pl.outcome = Outcome::Shown,
                Answer::Fails(seen) => {
                    pl.outcome = Outcome::Example(Box::new(seen_witness(c, p, suite, f, seen, &pl.what)));
                    pl.across = v.said.clone();
                }
                Answer::Undecided(_) => {
                    pl.outcome = Outcome::Undecided;
                    pl.across = v.said.clone();
                }
            }
        }
        for r in facts.refusals.iter_mut() {
            let at = At { line: r.span.line, col: r.span.col };
            if let Some(v) = verdicts.iter().find(|v| v.at == at && v.check == Crossing::Refusals && v.about == r.op) {
                r.reasons = v.refusals.clone();
            }
        }
    }
}

/// The example of a check across the languages, as kaname keeps one (DESIGN 5.7): the function's
/// input of the run that breaks it, and the value it gave the place (none for a precondition, which
/// is about the call).
fn seen_witness(c: &Checked, p: &Program, suite: &Suite, f: usize, seen: &ritsu_ports::Seen, what: &What) -> Witness {
    let input = ritsu_base::json::parse(&seen.input).ok().and_then(|j| crate::interp::Interp::new(p, suite).enter_json(c, f, &j).ok()).unwrap_or_default();
    let to = match what {
        What::RuleDate { input, .. } | What::DateDay { input, .. } => Some(input.as_str()),
        What::Amount { param, .. } => Some(param.as_str()),
        _ => None,
    };
    let value = to.and_then(|to| seen.given.iter().find(|(n, _)| n == to)).and_then(|(_, v)| match v {
        ritsu_ports::Value::Int(i) => Some(Value::Num(Rat::int(*i))),
        ritsu_ports::Value::Date(d) => crate::date::parse(d).map(Value::Date),
        _ => None,
    });
    Witness { input, value }
}

/// The code a place that is not shown is said with: E301–E303 for an example, W301–W303 for an
/// undecided one; none for one shown, and for one the checks across the languages decide (ritsu's
/// codes, which ritsu-cross says: [`Place::across`]).
pub fn code_of(pl: &Place) -> Option<&'static str> {
    if pl.what.across().is_some() {
        return None;
    }
    let example = match pl.outcome {
        Outcome::Example(_) => true,
        Outcome::Undecided => false,
        _ => return None,
    };
    Some(match (&pl.what, example) {
        (What::Overflow, true) => "E302",
        (What::Overflow, false) => "W302",
        (What::Zero, true) => "E303",
        (What::Zero, false) => "W303",
        (_, true) => "E301",
        (_, false) => "W301",
    })
}

/// The range of an input of an enum as the rule writes it: `>=basic <=gold`, the first value the
/// range lets in and the last (the range of an ordered enum has no gap in the enum's order).
fn enum_range_text(values: &[String]) -> String {
    match (values.first(), values.last()) {
        (Some(lo), Some(hi)) => format!(">={lo} <={hi}"),
        _ => String::new(),
    }
}

/// What a place declares, as a person reads it: `range >=0 <=5`, `count <=30`.
fn decl_text(s: &Slot) -> String {
    let mut t = &s.ty;
    while let Ty::Opt(x) | Ty::List(x) = t {
        t = x;
    }
    let mut parts = Vec::new();
    if let Some(r) = &s.range {
        parts.push(format!("range {}", crate::value::range_text(r, t)));
    }
    if let Some(r) = &s.count {
        parts.push(format!("count {}", crate::value::range_text(r, &Ty::Num(NumTy::number()))));
    }
    parts.join(" ")
}

/// The diagnostic of a place that is not shown, and its headline (what E401 notes again).
fn place_diag(c: &Checked, p: &Program, f: usize, pl: &Place) -> Option<(crate::diag::Diag, Text)> {
    let code = code_of(pl)?;
    let example = code.starts_with('E');
    let line = c.src.lines().nth(pl.span.line.saturating_sub(1)).unwrap_or("");
    let cs: Vec<char> = line.chars().collect();
    let (a, b) = (pl.span.col.saturating_sub(1).min(cs.len()), pl.span.end.saturating_sub(1).min(cs.len()));
    let src: String = cs[a..b.max(a)].iter().collect::<String>().trim().to_string();
    let rt = |r: &Range, t: &Ty| crate::value::range_text(r, strip(t));
    // what takes the value, and how it reads
    let head = match &pl.what {
        What::RuleInput { rule, input, range, ty } => {
            let r = rt(range, ty);
            if example {
                tr!("`{src}` が、規則 `{rule}` の入力 `{input}` の範囲 {r} の外になることがあります", "`{src}` can be outside {r}, the range of the input `{input}` of the rule `{rule}`")
            } else {
                tr!("`{src}` が、規則 `{rule}` の入力 `{input}` の範囲 {r} に収まるかを決められません", "Whether `{src}` stays in {r}, the range of the input `{input}` of the rule `{rule}`, cannot be decided")
            }
        }
        What::RuleEnum { rule, input, values, .. } => {
            let r = enum_range_text(values);
            if example {
                tr!("`{src}` が、規則 `{rule}` の入力 `{input}` の範囲 {r} の外になることがあります", "`{src}` can be outside {r}, the range of the input `{input}` of the rule `{rule}`")
            } else {
                tr!("`{src}` が、規則 `{rule}` の入力 `{input}` の範囲 {r} に収まるかを決められません", "Whether `{src}` stays in {r}, the range of the input `{input}` of the rule `{rule}`, cannot be decided")
            }
        }
        What::RuleField { rule, list, field, range, ty } => {
            let r = rt(range, ty);
            if example {
                tr!("`{src}` の要素の `{field}` が、規則 `{rule}` がたどる並び `{list}` のフィールド `{field}` の範囲 {r} の外になることがあります", "The `{field}` of an element of `{src}` can be outside {r}, the range of the field `{field}` of the list `{list}` the rule `{rule}` walks")
            } else {
                tr!("`{src}` の要素の `{field}` が、規則 `{rule}` がたどる並び `{list}` のフィールド `{field}` の範囲 {r} に収まるかを決められません", "Whether the `{field}` of each element of `{src}` stays in {r}, the range of the field `{field}` of the list `{list}` the rule `{rule}` walks, cannot be decided")
            }
        }
        What::DateInt { date, input, range } => {
            let r = rt(range, &Ty::Num(NumTy::number()));
            if example {
                tr!("`{src}` が、日付 `{date}` の入力 `{input}` の範囲 {r} の外になることがあります", "`{src}` can be outside {r}, the range of the input `{input}` of the date `{date}`")
            } else {
                tr!("`{src}` が、日付 `{date}` の入力 `{input}` の範囲 {r} に収まるかを決められません", "Whether `{src}` stays in {r}, the range of the input `{input}` of the date `{date}`, cannot be decided")
            }
        }
        What::Param { function, param, slot } => {
            let d = decl_text(slot);
            let decl = if d.is_empty() { tr!("関数 `{function}` の引数 `{param}` の宣言", "what the parameter `{param}` of `{function}` declares") } else { tr!("関数 `{function}` の引数 `{param}` の宣言（{d}）", "what the parameter `{param}` of `{function}` declares ({d})") };
            let ja = crate::diag::ja_before(&decl.ja);
            if example {
                tr!("`{src}` が、{ja}の外になることがあります", "`{src}` can be outside {}", ; decl.en)
            } else {
                tr!("`{src}` が、{ja}に収まるかを決められません", "Whether `{src}` stays in {} cannot be decided", ; decl.en)
            }
        }
        What::Answer { function, slot } => {
            let d = decl_text(slot);
            if example {
                tr!("関数 `{function}` の答えが、宣言（{d}）の外になることがあります", "The answer of `{function}` can be outside what it declares ({d})")
            } else {
                tr!("関数 `{function}` の答えが、宣言（{d}）に収まるかを決められません", "Whether the answer of `{function}` stays in what it declares ({d}) cannot be decided")
            }
        }
        What::Field { record, field, slot } => {
            let d = decl_text(slot);
            if example {
                tr!("`{src}` が、{record} のフィールド `{field}` の宣言（{d}）の外になることがあります", "`{src}` can be outside what the field `{field}` of {record} declares ({d})")
            } else {
                tr!("`{src}` が、{record} のフィールド `{field}` の宣言（{d}）に収まるかを決められません", "Whether `{src}` stays in what the field `{field}` of {record} declares ({d}) cannot be decided")
            }
        }
        What::Isqrt => {
            if example {
                tr!("`isqrt` に渡す `{src}` が、負になることがあります", "`{src}`, given to `isqrt`, can be below 0")
            } else {
                tr!("`isqrt` に渡す `{src}` が、負にならないかを決められません", "Whether `{src}`, given to `isqrt`, stays at 0 or above cannot be decided")
            }
        }
        What::Overflow => {
            if example {
                tr!("`{src}` の値が int64 を超えることがあります", "The value of `{src}` can leave int64")
            } else {
                tr!("`{src}` の値が int64 に収まるかを決められません", "Whether the value of `{src}` stays in int64 cannot be decided")
            }
        }
        What::Zero => {
            if example {
                tr!("`{src}` の割る数が 0 になることがあります", "The divisor of `{src}` can be 0")
            } else {
                tr!("`{src}` の割る数が 0 にならないかを決められません", "Whether the divisor of `{src}` stays away from 0 cannot be decided")
            }
        }
        _ => return None,
    };
    let mut d = crate::diag::Diag::at(code, &c.path, pl.span.line, pl.span.col, head.clone()).source(&c.src);
    match &pl.outcome {
        Outcome::Example(w) => {
            let info = &c.decls.fns[f];
            let args: Vec<String> = info.params.iter().zip(&w.input).map(|(prm, v)| format!("{}: {}", prm.name.text, p.show(c, v, &prm.slot.ty))).collect();
            let call = format!("{}({})", info.name.text, args.join(", "));
            d = d.note(match (&pl.what, &w.value) {
                (What::Overflow, _) => tr!("たとえば `{call}` のとき、int64 を超えます。", "For example, `{call}` takes it past int64."),
                (What::Zero, _) => tr!("たとえば `{call}` のとき、0 で割ります。", "For example, `{call}` divides by 0."),
                (_, Some(v)) => {
                    let shown = p.show(c, v, &pl.ty);
                    tr!("たとえば `{call}` のとき、{shown} になります。", "For example, `{call}` gives it {shown}.")
                }
                (_, None) => tr!("たとえば `{call}` のときです。", "For example, `{call}`."),
            });
            d = d.note(match &pl.what {
                What::Overflow => tr!("入力の範囲を狭めるか、先に丸めてください。", "Narrow the ranges of the inputs, or round sooner."),
                What::Zero => tr!("割る前に `if` で 0 を除くか、割る数の範囲から 0 を外してください。", "Leave 0 out with an `if` before the division, or out of the divisor's range."),
                What::Isqrt => tr!("`isqrt` に渡す前に、`if` で 0 以上に狭めてください。", "Narrow it to 0 and above with an `if` before it goes to `isqrt`."),
                What::Answer { .. } => tr!("答えを `if` で宣言の範囲に狭めるか、宣言を広げてください。", "Narrow the answer to what it declares with an `if`, or widen what it declares."),
                What::RuleEnum { .. } => tr!(
                    "渡す前に `match` か `if` で範囲に入る値に狭めるか、規則の範囲を広げてください。",
                    "Narrow it to the values the range lets in with a `match` or an `if` before it goes there, or widen the rule's range."
                ),
                _ => tr!("渡す前に `if` でその範囲に狭めるか、渡す先の範囲を広げてください。", "Narrow it to that range with an `if` before it goes there, or widen the range it goes to."),
            });
        }
        _ => {
            let shown_ty = match &pl.what {
                What::Zero | What::Overflow => pl.ty.clone(),
                _ => strip(&pl.ty).clone(),
            };
            let what = match &pl.what {
                What::Zero => tr!("割る数", "the divisor"),
                _ => tr!("値", "the value"),
            };
            match (&pl.what, pl.value.present(), pl.value.present().iv()) {
                // the values of the enum the analysis finds it can be, in the enum's order
                (What::RuleEnum { .. }, Abs::Set(Some(s)), _) => {
                    let order: Vec<Text> = match strip(&pl.ty) {
                        Ty::Enum(id) => p.values(id).0.iter().filter(|n| s.contains(&n.text)).map(|n| Text::same(n.text.clone())).collect(),
                        _ => s.iter().map(|x| Text::same(x.clone())).collect(),
                    };
                    let vs = Text::list(&order);
                    d = d.note(tr!(
                        "範囲の解析では、値は {} のどれかです。関数のベクタにも、外れる入力はありませんでした。",
                        "The analysis of ranges finds it one of {}, and no input among the function's vectors takes it out.",
                        vs.ja;
                        vs.en
                    ));
                }
                (What::RuleEnum { .. }, _, _) => {
                    d = d.note(tr!(
                        "範囲の解析では、値が列挙のどの値になるかが決まりません。関数のベクタにも、外れる入力はありませんでした。",
                        "The analysis of ranges does not find which values of the enum it can be, and no input among the function's vectors takes it out."
                    ));
                }
                (_, _, Some(i)) if i.lo.is_some() || i.hi.is_some() => {
                    let at = iv_text(i, strip(&shown_ty));
                    d = d.note(tr!(
                        "範囲の解析では、{}は {} です。関数のベクタにも、外れる入力はありませんでした。",
                        "The analysis of ranges finds {} in {}, and no input among the function's vectors takes it out.",
                        what.ja, at.ja;
                        what.en, at.en
                    ));
                }
                _ => {
                    d = d.note(tr!(
                        "範囲の解析では、{}の範囲が決まりません。関数のベクタにも、外れる入力はありませんでした。",
                        "The analysis of ranges finds no bound on {}, and no input among the function's vectors takes it out.",
                        what.ja;
                        what.en
                    ));
                }
            }
            d = d.note(tr!("生成したコードが、走らせたときにこの場所で確かめます。", "The generated code checks it there when it runs."));
            d = d.note(match &pl.what {
                What::Overflow => tr!("入力の範囲を狭めるか、先に丸めると、示せるようになります。", "Narrow the ranges of the inputs, or round sooner, and it is shown."),
                What::Zero => tr!("割る前に `if` で 0 を除くと、範囲の解析がそれを読んで示せるようになります。", "Leave 0 out with an `if` before the division, which the analysis reads, and it is shown."),
                What::RuleEnum { .. } => tr!(
                    "`match` か `if` で範囲に入る値に狭めると、範囲の解析がそれを読んで示せるようになります。",
                    "Narrow it to the values the range lets in with a `match` or an `if`, which the analysis reads, and it is shown."
                ),
                _ => tr!("`if` で範囲に狭めると、範囲の解析がそれを読んで示せるようになります。", "Narrow it with an `if`, which the analysis reads, and it is shown."),
            });
        }
    }
    Some((d, head))
}

/// How many functions of a list are laid over each other exactly in `reduce`, at most.
pub const EXACT: usize = 16;

fn strength(r: Rel) -> u8 {
    match r {
        Rel::Le => 1,
        Rel::Lt => 2,
        Rel::Eq => 3,
    }
}

/// The values a step of `t` lands on, read in `to` (a constant in its value read there).
fn step_of_conv(t: &Ty, to: &Ty) -> Option<Rat> {
    let s = step_of(t)?;
    if let (Ty::Num(n), Some((f, shift))) = (strip(t), factor_of(t, to))
        && let Some(k) = n.konst
    {
        // a constant read in `to` is its own step there
        let x = k.mul(f).add(shift);
        return Some(Rat { num: x.num.abs(), den: x.den });
    }
    Some(match factor_of(t, to) {
        Some((f, _)) => {
            let x = s.mul(f);
            Rat { num: x.num.abs(), den: x.den }
        }
        None => s,
    })
}

/// Whether `a` is a part of a parameter as it is and `b` a literal: a comparison the vectors take
/// the values of already (DESIGN 9).
fn direct(a: &Expr, b: &Expr, va: &V, vb: &V) -> bool {
    va.path.is_some() && va.lin.is_none() && matches!(&b.kind, ExprKind::Num(_) | ExprKind::Date(_) | ExprKind::Neg(_)) && vb.sum().is_some_and(|l| l.is_const()) && !matches!(a.kind, ExprKind::Bin(..))
}

/// Whether every number of a value keeps the rule of int64 (a place for `overflow` shown): a number or
/// a date by its interval, an optional by what it holds, a value that can be anything never.
fn ok_num(a: &Abs) -> bool {
    match a {
        Abs::Num(i) | Abs::Date(i) => i.fits(),
        Abs::Opt(_, x) => ok_num(x),
        Abs::Any => false,
        _ => true,
    }
}

/// The sums of up to `count` values each in `w` (and of none): what `sum` answers, and every partial
/// sum on the way. The bound on their denominators is the one the type's step says, and none where it
/// says none: a sum of values each of a denominator of at most `d` can have a larger one (1/2 + 1/3
/// is 5/6), so the elements' bound is no bound on the sum's (DESIGN 5.5).
fn sums(count: Iv, w: Iv, den: Option<i128>) -> Iv {
    let c_lo = count.lo.unwrap_or(Rat::zero());
    let lo = match w.lo {
        Some(l) if l.num < 0 => count.hi.and_then(|h| h.checked_mul(l)),
        Some(l) => c_lo.checked_mul(l),
        None => None,
    };
    let hi = match w.hi {
        Some(h) if h.num > 0 => count.hi.and_then(|c| c.checked_mul(h)),
        Some(h) => c_lo.checked_mul(h),
        None => None,
    };
    let lo = match (lo, count.lo.is_none_or(|l| l.num <= 0)) {
        (Some(l), true) if l.num > 0 => Some(Rat::zero()),
        (x, _) => x,
    };
    let hi = match (hi, count.lo.is_none_or(|l| l.num <= 0)) {
        (Some(h), true) if h.num < 0 => Some(Rat::zero()),
        (x, _) => x,
    };
    Iv::new(lo, hi, den)
}

/// The ends of `acc` that moved away from `start`, let go.
fn widen(start: &Abs, acc: &Abs) -> Abs {
    match (start, acc) {
        (Abs::Num(s), Abs::Num(a)) | (Abs::Date(s), Abs::Date(a)) => {
            let lo = if a.lo == s.lo { a.lo } else { None };
            let hi = if a.hi == s.hi { a.hi } else { None };
            let iv = Iv { lo, hi, den: a.den };
            if matches!(acc, Abs::Num(_)) { Abs::Num(iv) } else { Abs::Date(iv) }
        }
        (Abs::Opt(_, s), Abs::Opt(n, a)) => Abs::Opt(*n, Box::new(widen(s, a))),
        (Abs::List(sc, se), Abs::List(ac, ae)) => {
            let hi = if ac.hi == sc.hi { ac.hi } else { None };
            Abs::List(Iv { hi, ..*ac }, Box::new(widen(se, ae)))
        }
        (Abs::Record(s), Abs::Record(a)) => Abs::Record(Rec {
            fields: a.fields.iter().map(|(n, x)| (n.clone(), s.fields.iter().find(|(m, _)| m == n).map(|(_, y)| widen(y, x)).unwrap_or(Abs::Any))).collect(),
            rels: a.rels.clone(),
        }),
        (_, a) => a.clone(),
    }
}

/// A value cut to what is declared of it: the intervals met, field by field.
fn cut(a: &Abs, d: &Abs) -> Abs {
    match (a, d) {
        (Abs::Num(x), Abs::Num(y)) => Abs::Num(x.meet(*y).map(|i| i.with_den(x.den)).unwrap_or(*x)),
        (Abs::Date(x), Abs::Date(y)) => Abs::Date(x.meet(*y).unwrap_or(*x)),
        (Abs::Opt(n, x), Abs::Opt(_, y)) => Abs::Opt(*n, Box::new(cut(x, y))),
        (Abs::List(c, x), Abs::List(e, y)) => Abs::List(c.meet(*e).unwrap_or(*c), Box::new(cut(x, y))),
        (x, _) => x.clone(),
    }
}

/// Whether a condition can hold, and whether it can fail.
fn truths(a: &Abs) -> (bool, bool) {
    match a {
        Abs::Set(Some(s)) => (s.contains("true"), s.contains("false")),
        Abs::Never => (false, false),
        _ => (true, true),
    }
}

fn bools(t: bool, f: bool) -> Abs {
    let mut s = BTreeSet::new();
    if t {
        s.insert("true".to_string());
    }
    if f {
        s.insert("false".to_string());
    }
    Abs::Set(Some(s))
}

/// What a comparison of two intervals comes to, when it is decided.
fn decide(x: Iv, op: BinOp, y: Iv) -> Option<bool> {
    let lt = x.below(y);
    let gt = y.below(x);
    let le = x.hi.zip(y.lo).is_some_and(|(h, l)| cmp(h, l).is_le());
    let ge = y.hi.zip(x.lo).is_some_and(|(h, l)| cmp(h, l).is_le());
    let same = x.is_exact().zip(y.is_exact()).is_some_and(|(a, b)| a == b);
    match op {
        BinOp::Lt if lt => Some(true),
        BinOp::Lt if ge => Some(false),
        BinOp::Le if le => Some(true),
        BinOp::Le if gt => Some(false),
        BinOp::Gt if gt => Some(true),
        BinOp::Gt if le => Some(false),
        BinOp::Ge if ge => Some(true),
        BinOp::Ge if lt => Some(false),
        BinOp::Eq if same => Some(true),
        BinOp::Eq if lt || gt => Some(false),
        BinOp::Ne if same => Some(false),
        BinOp::Ne if lt || gt => Some(true),
        _ => None,
    }
}

/// The comparison that holds where `op` does not.
fn negate(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::Ge,
        BinOp::Le => BinOp::Gt,
        BinOp::Gt => BinOp::Le,
        BinOp::Ge => BinOp::Lt,
        BinOp::Eq => BinOp::Ne,
        BinOp::Ne => BinOp::Eq,
        x => x,
    }
}

/// The comparison with its sides swapped.
fn flip(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::Gt,
        BinOp::Le => BinOp::Ge,
        BinOp::Gt => BinOp::Lt,
        BinOp::Ge => BinOp::Le,
        x => x,
    }
}

/// `x` where `x op y` holds, its values on the step `step`: `x < 5` for whole numbers is `x <= 4`.
fn narrow_iv(x: Iv, op: BinOp, y: Iv, step: Option<Rat>) -> Option<Iv> {
    let step = step.filter(|s| s.num > 0);
    // the largest value on the step below `v`, and the smallest above it
    let below = |v: Rat| -> Rat {
        match step {
            Some(s) => {
                let k = v.checked_div(s).map(|q| {
                    let f = q.num.div_euclid(q.den);
                    if Rat::int(f) == q { f - 1 } else { f }
                });
                k.and_then(|k| Rat::int(k).checked_mul(s)).unwrap_or(v)
            }
            None => v,
        }
    };
    let above = |v: Rat| -> Rat {
        match step {
            Some(s) => {
                let k = v.checked_div(s).map(|q| q.num.div_euclid(q.den) + 1);
                k.and_then(|k| Rat::int(k).checked_mul(s)).unwrap_or(v)
            }
            None => v,
        }
    };
    let out = match op {
        BinOp::Lt => Iv { hi: min_hi(x.hi, y.hi.map(below)), ..x },
        BinOp::Le => Iv { hi: min_hi(x.hi, y.hi), ..x },
        BinOp::Gt => Iv { lo: max_lo(x.lo, y.lo.map(above)), ..x },
        BinOp::Ge => Iv { lo: max_lo(x.lo, y.lo), ..x },
        BinOp::Eq => x.meet(y)?,
        BinOp::Ne => match y.is_exact() {
            Some(v) if x.lo == Some(v) && step.is_some() => Iv { lo: Some(above(v)), ..x },
            Some(v) if x.hi == Some(v) && step.is_some() => Iv { hi: Some(below(v)), ..x },
            _ => x,
        },
        _ => x,
    };
    if let (Some(a), Some(b)) = (out.lo, out.hi)
        && cmp(a, b).is_gt()
    {
        return None;
    }
    Some(out)
}

/// How a precondition reads in a message (as ritsu-cross writes it).
pub fn said(p: &Precondition) -> String {
    match p {
        Precondition::Relation { left, op, right } => format!("{left} {op} {right}"),
        Precondition::SumRelation { left, op, right } => format!("{} {op} {}", left.join(" + "), right.join(" + ")),
        Precondition::Sum { name, max, .. } => format!("{name} <= {max}"),
        Precondition::Length { sequence, max } => format!("len({sequence}) <= {max}"),
        Precondition::Days { input, file, date, .. } => format!("{input} in koyomi \"{file}\" date {date}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iv(lo: i128, hi: i128) -> Iv {
        Iv::ints(lo, hi)
    }

    #[test]
    fn the_arithmetic_of_intervals() {
        assert_eq!(iv(1, 3).plus(iv(10, 20)), iv(11, 23));
        assert_eq!(iv(1, 3).minus(iv(10, 20)), iv(-19, -7));
        assert_eq!(iv(-2, 3).times(iv(4, 5)), iv(-10, 15));
        let q = iv(10, 20).over(iv(2, 5));
        assert_eq!((q.lo, q.hi), (Some(Rat::int(2)), Some(Rat::int(10))));
        // a divisor that can be 0: its other values are a whole step away from it
        let q = iv(1, 5).over(iv(0, 4));
        assert_eq!((q.lo, q.hi), (Some(Rat::new(1, 4)), Some(Rat::int(5))));
        assert!(iv(0, i64::MAX as i128).fits());
        assert!(!iv(0, i64::MAX as i128 + 1).fits());
        assert!(iv(i64::MIN as i128, 0).fits());
        assert!(!Iv::TOP.fits());
        // rounding is monotone: the ends rounded are the ends of the values rounded
        let r = Iv::new(Some(Rat::new(1, 3)), Some(Rat::new(7, 2)), None).round(Rounding::HalfUp, Rat::int(1));
        assert_eq!(r, iv(0, 4));
    }

    #[test]
    fn narrowing_on_the_step() {
        assert_eq!(narrow_iv(iv(-759, 1430), BinOp::Ge, iv(1, 1), Some(Rat::int(1))), Some(iv(1, 1430)));
        assert_eq!(narrow_iv(iv(0, 9), BinOp::Lt, iv(5, 5), Some(Rat::int(1))), Some(iv(0, 4)));
        assert_eq!(narrow_iv(iv(0, 9), BinOp::Ne, iv(0, 0), Some(Rat::int(1))), Some(iv(1, 9)));
        assert_eq!(narrow_iv(iv(0, 9), BinOp::Gt, iv(9, 9), Some(Rat::int(1))), None);
    }

    #[test]
    fn exact_order_of_large_values() {
        let a = Rat::new(i128::MAX / 3, 7);
        let b = Rat::new(i128::MAX / 3 - 1, 7);
        assert_eq!(cmp(a, b), Ordering::Greater);
        assert_eq!(cmp(Rat::new(1, 3), Rat::new(1, 3)), Ordering::Equal);
    }
}
