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
//!   declares. A function in a ring of calls answers what its result declares (DESIGN 5.4). A generic
//!   function and one that takes a function are read again where they are called, with the values the
//!   call gives (DESIGN 26.5, 27.4).
//! - A function value is the lambdas it can be (DESIGN 27.4): calling it reads each lambda's body with
//!   what it caught and the arguments, and the places of that body are the places of the function
//!   that wrote it, wherever it is read.
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
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;

// ── Intervals ────────────────────────────────────────────────────────────────

/// An end of an interval: a value, or none where the interval goes on without end.
pub type End = Option<Rat>;

/// The interval a number or a date can be in, both ends in, and the largest denominator its values
/// have in lowest terms (1 for whole numbers; None where it is not known).
///
/// Each end, and the bound on the denominator, says whether it comes from the end of a type (DESIGN
/// 22.2): a value whose range is not written takes the whole of its type, and an end an operation
/// makes from such an end, or a bound on a denominator a quotient makes from it, is marked too. The
/// marks change what is said of a place for int64 alone (`Outcome::Unbounded`), never whether a place
/// is shown nor what a value can be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Iv {
    pub lo: End,
    pub hi: End,
    pub den: Option<i128>,
    /// Whether the lower end comes from the end of a type.
    pub open_lo: bool,
    /// Whether the upper end comes from the end of a type.
    pub open_hi: bool,
    /// Whether the bound on the denominator comes from the end of a type.
    pub open_den: bool,
}

/// An end with its mark: whether it comes from the end of a type.
type Marked = (End, bool);

/// The lower of two lower ends (none is below every value), with its mark; on a tie, marked when
/// either is (DESIGN 22.2: two values joined keep the mark of the end chosen).
fn lower(a: Marked, b: Marked) -> Marked {
    match (a.0, b.0) {
        (None, None) => (None, a.1 || b.1),
        (None, _) => a,
        (_, None) => b,
        (Some(x), Some(y)) => match cmp(x, y) {
            Ordering::Less => a,
            Ordering::Greater => b,
            Ordering::Equal => (a.0, a.1 || b.1),
        },
    }
}

/// The higher of two upper ends (none is above every value), with its mark.
fn higher(a: Marked, b: Marked) -> Marked {
    match (a.0, b.0) {
        (None, None) => (None, a.1 || b.1),
        (None, _) => a,
        (_, None) => b,
        (Some(x), Some(y)) => match cmp(x, y) {
            Ordering::Greater => a,
            Ordering::Less => b,
            Ordering::Equal => (a.0, a.1 || b.1),
        },
    }
}

/// The lower of two upper ends where none bounds nothing (`min`'s upper end), with its mark.
fn least(a: Marked, b: Marked) -> Marked {
    match (a.0, b.0) {
        (None, None) => (None, a.1 || b.1),
        (None, _) => b,
        (_, None) => a,
        (Some(x), Some(y)) => match cmp(x, y) {
            Ordering::Less => a,
            Ordering::Greater => b,
            Ordering::Equal => (a.0, a.1 || b.1),
        },
    }
}

/// The higher of two lower ends where none bounds nothing (`max`'s lower end), with its mark.
fn most(a: Marked, b: Marked) -> Marked {
    match (a.0, b.0) {
        (None, None) => (None, a.1 || b.1),
        (None, _) => b,
        (_, None) => a,
        (Some(x), Some(y)) => match cmp(x, y) {
            Ordering::Greater => a,
            Ordering::Less => b,
            Ordering::Equal => (a.0, a.1 || b.1),
        },
    }
}

/// A lower end narrowed by `by`: the end `by` puts it at and the mark of `by` where it raises it, the
/// end as it was otherwise (DESIGN 22.2: an end a branch narrows takes the mark of what it is
/// compared with).
fn raise(own: Marked, by: Marked) -> Marked {
    match (own.0, by.0) {
        (_, None) => own,
        (None, Some(_)) => by,
        (Some(x), Some(y)) => {
            if cmp(y, x).is_gt() {
                by
            } else {
                own
            }
        }
    }
}

/// An upper end narrowed by `by`.
fn sink(own: Marked, by: Marked) -> Marked {
    match (own.0, by.0) {
        (_, None) => own,
        (None, Some(_)) => by,
        (Some(x), Some(y)) => {
            if cmp(y, x).is_lt() {
                by
            } else {
                own
            }
        }
    }
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

/// An end of an interval in the arithmetic: below every value, a value, above every value.
#[derive(Clone, Copy, Debug, PartialEq)]
enum X {
    Neg,
    F(Rat),
    Pos,
}

impl X {
    fn zero(self) -> bool {
        matches!(self, X::F(r) if r.num == 0)
    }

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
    pub const TOP: Iv = Iv { lo: None, hi: None, den: None, open_lo: false, open_hi: false, open_den: false };

    pub fn new(lo: End, hi: End, den: Option<i128>) -> Iv {
        Iv { lo: norm_lo(lo), hi: norm_hi(hi), den: den.filter(|d| *d >= 1), ..Iv::TOP }
    }

    /// The same, the ends kept within the arithmetic and their marks as given.
    fn marked(lo: Marked, hi: Marked, den: Option<i128>, open_den: bool) -> Iv {
        Iv { open_lo: lo.1, open_hi: hi.1, open_den, ..Iv::new(lo.0, hi.0, den) }
    }

    pub fn exact(v: Rat) -> Iv {
        Iv::new(Some(v), Some(v), Some(v.den))
    }

    /// Whole numbers from `lo` to `hi`.
    pub fn ints(lo: i128, hi: i128) -> Iv {
        Iv::new(Some(Rat::int(lo)), Some(Rat::int(hi)), Some(1))
    }

    /// The whole of a type: from `lo` to `hi`, both ends marked as the type's (DESIGN 22.1).
    pub fn whole(lo: i128, hi: i128, den: Option<i128>) -> Iv {
        Iv { open_lo: true, open_hi: true, ..Iv::new(Some(Rat::int(lo)), Some(Rat::int(hi)), den) }
    }

    fn lo_m(self) -> Marked {
        (self.lo, self.open_lo)
    }

    fn hi_m(self) -> Marked {
        (self.hi, self.open_hi)
    }

    /// The lower end raised to `v` where `v` is above it, with the mark `m` there.
    pub fn lift_lo(self, v: End, m: bool) -> Iv {
        let (lo, open_lo) = raise(self.lo_m(), (v, m));
        Iv { lo, open_lo, ..self }
    }

    /// The upper end lowered to `v` where `v` is below it, with the mark `m` there.
    pub fn drop_hi(self, v: End, m: bool) -> Iv {
        let (hi, open_hi) = sink(self.hi_m(), (v, m));
        Iv { hi, open_hi, ..self }
    }

    /// The same interval, its values known to have no larger denominator than `d` too.
    pub fn with_den(self, d: Option<i128>) -> Iv {
        let (den, open_den) = match (self.den, d) {
            (Some(a), Some(b)) if b < a => (Some(b), false),
            (Some(a), _) => (Some(a), self.open_den),
            (None, Some(b)) => (Some(b), false),
            (None, None) => (None, self.open_den),
        };
        Iv { den, open_den, ..self }
    }

    pub fn hull(self, o: Iv) -> Iv {
        let den = match (self.den, o.den) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
        let (lo, open_lo) = lower(self.lo_m(), o.lo_m());
        let (hi, open_hi) = higher(self.hi_m(), o.hi_m());
        Iv { lo, hi, den, open_lo, open_hi, open_den: self.open_den || o.open_den }
    }

    /// The values in both, as `self` narrowed by `o`: an end `o` moves takes `o`'s mark; None when
    /// there are none.
    pub fn meet(self, o: Iv) -> Option<Iv> {
        let (lo, open_lo) = raise(self.lo_m(), o.lo_m());
        let (hi, open_hi) = sink(self.hi_m(), o.hi_m());
        if let (Some(a), Some(b)) = (lo, hi)
            && cmp(a, b).is_gt()
        {
            return None;
        }
        let d = Iv { den: o.den, open_den: o.open_den, ..Iv::TOP };
        let (den, open_den) = match (self.den, d.den) {
            (Some(a), Some(b)) if b < a => (Some(b), d.open_den),
            (Some(a), _) => (Some(a), self.open_den),
            (None, Some(b)) => (Some(b), d.open_den),
            (None, None) => (None, self.open_den),
        };
        Some(Iv { lo, hi, den, open_lo, open_hi, open_den })
    }

    /// The interval cut to a declared range (both ends in, None at an open end).
    pub fn cut(self, r: &Range) -> Iv {
        self.meet(Iv { lo: r.min, hi: r.max, ..Iv::TOP }).unwrap_or(self)
    }

    /// The interval held to int64, as the interpreter lets a value through only there (DESIGN 5.2):
    /// an end it moves keeps its mark, for what it holds still comes from where it came.
    pub fn held64(self) -> Iv {
        let int = Iv { lo: Some(Rat::int(I64_MIN)), hi: Some(Rat::int(I64_MAX)), ..Iv::TOP };
        match self.meet(int) {
            Some(x) => Iv { open_lo: self.open_lo, open_hi: self.open_hi, den: self.den, open_den: self.open_den, ..x },
            None => self,
        }
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

    /// Whether every way the interval leaves int64 comes from the end of a type (DESIGN 22.2): an end
    /// with no value, or past int64 with the bound on the denominator, is marked, or the bound is;
    /// a bound on the denominator past int64, or none, is marked. False for an interval that fits.
    pub fn leaves_by_type(self) -> bool {
        if self.fits() {
            return false;
        }
        let den_ok = match self.den {
            Some(d) => d <= I64_MAX || self.open_den,
            None => self.open_den,
        };
        let dd = self.den.map(Rat::int);
        let lo_ok = match self.lo {
            None => self.open_lo,
            Some(l) => l.num >= 0 || dd.is_some_and(|d| l.checked_mul(d).is_some_and(|x| cmp(x, Rat::int(I64_MIN)).is_ge())) || self.open_lo || self.open_den,
        };
        let hi_ok = match self.hi {
            None => self.open_hi,
            Some(h) => h.num <= 0 || dd.is_some_and(|d| h.checked_mul(d).is_some_and(|x| cmp(x, Rat::int(I64_MAX)).is_le())) || self.open_hi || self.open_den,
        };
        den_ok && lo_ok && hi_ok
    }

    fn x_lo(self) -> X {
        self.lo.map(X::F).unwrap_or(X::Neg)
    }

    fn x_hi(self) -> X {
        self.hi.map(X::F).unwrap_or(X::Pos)
    }

    fn of_x(lo: (X, bool), hi: (X, bool), den: Option<i128>, open_den: bool) -> Iv {
        let end = |x: X| match x {
            X::F(r) => Some(r),
            _ => None,
        };
        Iv::marked((end(lo.0), lo.1), (end(hi.0), hi.1), den, open_den)
    }

    fn den_mul(a: Option<i128>, b: Option<i128>) -> Option<i128> {
        a?.checked_mul(b?)
    }

    pub fn plus(self, o: Iv) -> Iv {
        let lo = self.lo.zip(o.lo).and_then(|(a, b)| a.checked_add(b));
        let hi = self.hi.zip(o.hi).and_then(|(a, b)| a.checked_add(b));
        Iv::marked((lo, self.open_lo || o.open_lo), (hi, self.open_hi || o.open_hi), Iv::den_mul(self.den, o.den), self.open_den || o.open_den)
    }

    pub fn negated(self) -> Iv {
        let n = |v: End| v.and_then(|r| r.num.checked_neg().map(|x| Rat { num: x, den: r.den }));
        Iv::marked((n(self.hi), self.open_hi), (n(self.lo), self.open_lo), self.den, self.open_den)
    }

    pub fn minus(self, o: Iv) -> Iv {
        self.plus(o.negated())
    }

    /// The products of the ends, the lowest and the highest: an end made of two ends is marked where
    /// one of them is and the other is not 0 (DESIGN 22.2: a product takes the marks of the pair of
    /// ends chosen); on a tie, where either pair is.
    pub fn times(self, o: Iv) -> Iv {
        let pair = |a: (X, bool), b: (X, bool)| (a.0.mul(b.0), (a.1 && !b.0.zero()) || (b.1 && !a.0.zero()));
        let (l, h) = ((self.x_lo(), self.open_lo), (self.x_hi(), self.open_hi));
        let (ol, oh) = ((o.x_lo(), o.open_lo), (o.x_hi(), o.open_hi));
        let cands = [pair(l, ol), pair(l, oh), pair(h, ol), pair(h, oh)];
        let mut lo = cands[0];
        let mut hi = cands[0];
        for c in cands {
            if c.0 == lo.0 {
                lo.1 |= c.1;
            } else if c.0.le(lo.0) {
                lo = c;
            }
            if c.0 == hi.0 {
                hi.1 |= c.1;
            } else if hi.0.le(c.0) {
                hi = c;
            }
        }
        Iv::of_x(lo, hi, Iv::den_mul(self.den, o.den), self.open_den || o.open_den)
    }

    /// The reciprocals of an interval that holds no 0.
    fn recip(self) -> Iv {
        let r = |v: X| match v {
            X::F(x) if x.num != 0 => X::F(Rat::new(x.den, x.num)),
            _ => X::F(Rat::zero()),
        };
        Iv::of_x((r(self.x_hi()), self.open_hi), (r(self.x_lo()), self.open_lo), None, false)
    }

    /// The largest magnitude a value has, None when there is none; with whether it comes from the end
    /// of a type.
    fn magnitude(self) -> Option<(Rat, bool)> {
        let (lo, hi) = (self.lo?, self.hi?);
        let a = |r: Rat| Rat { num: r.num.abs(), den: r.den };
        Some(match cmp(a(lo), a(hi)) {
            Ordering::Greater => (a(lo), self.open_lo),
            Ordering::Less => (a(hi), self.open_hi),
            Ordering::Equal => (a(lo), self.open_lo || self.open_hi),
        })
    }

    /// `self / o` over the values of `o` that are not 0: a value of `o` that is not 0 is at least
    /// 1/D away from it, D its bound on the denominator.
    pub fn over(self, o: Iv) -> Iv {
        // the denominator of a quotient: the dividend's times the divisor's numerator, marked where
        // the divisor's magnitude comes from the end of a type
        let (den, open_den) = {
            let made = (|| {
                let (m, mark) = o.magnitude()?;
                let d = o.den?;
                let numer = m.checked_mul(Rat::int(d))?;
                let numer = numer.num.div_euclid(numer.den) + 1;
                Some((self.den?.checked_mul(numer), mark))
            })();
            match made {
                Some((den, mark)) => (den, self.open_den || o.open_den || mark),
                None => (None, self.open_den || o.open_den || o.open_lo || o.open_hi),
            }
        };
        let positive = o.lo.is_some_and(|l| l.num > 0);
        let negative = o.hi.is_some_and(|h| h.num < 0);
        if positive || negative {
            return Iv { den, open_den, ..self.times(o.recip()) };
        }
        let Some(d) = o.den else { return Iv { open_den, ..Iv::TOP } };
        let near = Rat::new(1, d);
        let mut out: Option<Iv> = None;
        if o.lo.is_none_or(|l| l.num < 0) {
            let part = Iv { lo: o.lo, hi: Some(Rat::zero().sub(near)), den: o.den, open_lo: o.open_lo, ..Iv::TOP };
            let q = self.times(part.recip());
            out = Some(out.map_or(q, |x| x.hull(q)));
        }
        if o.hi.is_none_or(|h| h.num > 0) {
            let part = Iv { lo: Some(near), hi: o.hi, den: o.den, open_hi: o.open_hi, ..Iv::TOP };
            let q = self.times(part.recip());
            out = Some(out.map_or(q, |x| x.hull(q)));
        }
        match out {
            Some(q) => Iv { den, open_den, ..q },
            None => Iv { den, open_den, ..Iv::TOP },
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
    /// `-92929292929292930` rounds to). An end rounded keeps its mark.
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
        Iv::marked((if self.lo.is_some() { lo } else { None }, self.open_lo), (if self.hi.is_some() { hi } else { None }, self.open_hi), Some(g.den), false)
    }

    /// The least of two intervals' values (`min`) and the greatest (`max`), both ends marked as the
    /// end chosen.
    pub fn min_of(self, o: Iv) -> Iv {
        let (lo, hi) = (lower(self.lo_m(), o.lo_m()), least(self.hi_m(), o.hi_m()));
        Iv::marked(lo, hi, self.hull(o).den, self.open_den || o.open_den)
    }

    pub fn max_of(self, o: Iv) -> Iv {
        let (lo, hi) = (most(self.lo_m(), o.lo_m()), higher(self.hi_m(), o.hi_m()));
        Iv::marked(lo, hi, self.hull(o).den, self.open_den || o.open_den)
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
    /// A string: the interval of its count of code points (DESIGN 25.6). What it holds is not
    /// followed.
    Str(Iv),
    /// A value that may be none: whether it can be none, and what it is when it is there.
    Opt(bool, Box<Abs>),
    /// A list: the interval of its count, and what each element can be.
    List(Iv, Box<Abs>),
    /// A dict (DESIGN 24.5): the interval of the count of its pairs, what each key can be, and what
    /// each value can be.
    Dict(Iv, Box<Abs>, Box<Abs>),
    /// A set: the interval of the count of its members, and what each member can be.
    Members(Iv, Box<Abs>),
    Record(Rec),
    /// A value of an enum that has fields (DESIGN 23.7): each value it can be, by its name, with what
    /// its fields can be (no fields for a value with none).
    Variants(BTreeMap<String, Rec>),
    /// A transfer built: its arguments.
    Transfer(Vec<(String, Abs)>),
    /// A function value (DESIGN 27.4): the lambdas it can be, and whether it can be one the analysis
    /// does not see (a parameter of a function type, in the analysis of its function itself), with the
    /// effect its type writes, where it writes one.
    Fns(Vec<Lam>, Option<Option<FnEffect>>),
}

/// A lambda made a value (DESIGN 27.4): the module it is written in and its expression, the function
/// of the module it is written in, the analysis of a function it was made in (its frame: where its
/// body's places go), and what the names in scope were then.
#[derive(Clone, Debug, PartialEq)]
pub struct Lam {
    pub module: PathBuf,
    pub expr: usize,
    pub owner: usize,
    pub frame: usize,
    pub env: Rc<Vec<(String, Abs)>>,
}

/// Two effects of functions the analysis does not see joined: each kind of either, where both write
/// one; none written where either writes none.
fn join_unseen(a: &Option<Option<FnEffect>>, b: &Option<Option<FnEffect>>) -> Option<Option<FnEffect>> {
    match (a, b) {
        (None, x) | (x, None) => x.clone(),
        (Some(Some(x)), Some(Some(y))) => {
            let mut fails = x.fails.clone();
            for k in &y.fails {
                if !fails.contains(k) {
                    fails.push(*k);
                }
            }
            fails.sort();
            Some(Some(FnEffect { fails, diverges: x.diverges || y.diverges }))
        }
        _ => Some(None),
    }
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
            // the field of each value it can be that has it: a value's type has one value (DESIGN 23.7)
            Abs::Variants(vs) => {
                let mut out = Abs::Never;
                for r in vs.values() {
                    if let Some((_, a)) = r.fields.iter().find(|(n, _)| n == name) {
                        out = out.join(a);
                    }
                }
                out
            }
            _ => Abs::Any,
        }
    }

    /// One value of a sum with fields: its name, and what its fields can be.
    pub fn variant(name: &str, fields: Vec<(String, Abs)>) -> Abs {
        Abs::Variants([(name.to_string(), Rec { fields, rels: Vec::new() })].into())
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
            (Abs::Str(a), Abs::Str(b)) => Abs::Str(a.hull(*b)),
            (Abs::Opt(n, x), Abs::Opt(m, y)) => Abs::Opt(*n || *m, Box::new(x.join(y))),
            (Abs::Opt(n, x), y) | (y, Abs::Opt(n, x)) => Abs::Opt(*n, Box::new(x.join(y))),
            (Abs::List(c, x), Abs::List(d, y)) => Abs::List(c.hull(*d), Box::new(x.join(y))),
            (Abs::Dict(c, k, x), Abs::Dict(d, l, y)) => Abs::Dict(c.hull(*d), Box::new(k.join(l)), Box::new(x.join(y))),
            (Abs::Members(c, x), Abs::Members(d, y)) => Abs::Members(c.hull(*d), Box::new(x.join(y))),
            (Abs::Record(a), Abs::Record(b)) => Abs::Record(join_rec(a, b)),
            // every value either can be, the fields of a value both can be joined
            (Abs::Variants(a), Abs::Variants(b)) => {
                let mut out = a.clone();
                for (v, r) in b {
                    let joined = match out.get(v) {
                        Some(x) => join_rec(x, r),
                        None => r.clone(),
                    };
                    out.insert(v.clone(), joined);
                }
                Abs::Variants(out)
            }
            (Abs::Transfer(a), Abs::Transfer(b)) => Abs::Transfer(a.iter().map(|(n, x)| (n.clone(), b.iter().find(|(m, _)| m == n).map(|(_, y)| x.join(y)).unwrap_or(Abs::Any))).collect()),
            // every lambda of either (DESIGN 27.4)
            (Abs::Fns(a, ua), Abs::Fns(b, ub)) => {
                let mut ls = a.clone();
                for l in b {
                    if !ls.contains(l) {
                        ls.push(l.clone());
                    }
                }
                Abs::Fns(ls, join_unseen(ua, ub))
            }
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
            (Abs::Str(c), Value::Str(s)) => c.has(Rat::int(crate::text::count(s) as i128)),
            (Abs::List(c, e), Value::List(xs)) => c.has(Rat::int(xs.len() as i128)) && xs.iter().all(|x| e.has(x)),
            (Abs::Dict(c, k, x), Value::Dict(ps)) => c.has(Rat::int(ps.len() as i128)) && ps.iter().all(|(a, b)| k.has(a) && x.has(b)),
            (Abs::Members(c, e), Value::Set(xs)) => c.has(Rat::int(xs.len() as i128)) && xs.iter().all(|x| e.has(x)),
            (Abs::Record(r), Value::Record(fs)) => fs.iter().all(|(n, x)| r.fields.iter().find(|(m, _)| m == n).is_none_or(|(_, a)| a.has(x))),
            (Abs::Variants(vs), Value::Enum(x)) => vs.contains_key(x),
            (Abs::Variants(vs), Value::Variant(x, fs)) => vs.get(x).is_some_and(|r| fs.iter().all(|(n, v)| r.fields.iter().find(|(m, _)| m == n).is_none_or(|(_, a)| a.has(v)))),
            (Abs::Transfer(args), Value::Transfer(t)) => t.args.iter().all(|(n, x)| args.iter().find(|(m, _)| m == n).is_none_or(|(_, a)| a.has(x))),
            // a function value: one the analysis does not see, or a lambda of these whose names it
            // caught are among what the lambda caught (the last value of each name)
            (Abs::Fns(ls, unseen), Value::Closure(c)) => {
                unseen.is_some()
                    || ls.iter().any(|l| {
                        l.module == c.module
                            && l.expr == c.expr
                            && c.env.iter().all(|(n, v)| match l.env.iter().rev().find(|(m, _)| m == n) {
                                Some((_, a)) => a.has(v),
                                None => true,
                            })
                    })
            }
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
                Abs::Num(Iv { den: i.den.and_then(|d| d.checked_mul(f.den)).and_then(|d| d.checked_mul(shift.den)), open_den: i.open_den, ..x })
            }
            Abs::Opt(n, x) => Abs::Opt(*n, Box::new(x.scaled((f, shift)))),
            Abs::List(c, x) => Abs::List(*c, Box::new(x.scaled((f, shift)))),
            Abs::Members(c, x) => Abs::Members(*c, Box::new(x.scaled((f, shift)))),
            // the keys read as the interpreter reads them, which keeps their order
            Abs::Dict(c, k, x) => Abs::Dict(*c, Box::new(k.scaled((f, shift))), Box::new(x.scaled((f, shift)))),
            x => x.clone(),
        }
    }
}

/// What a `result` can be (DESIGN 26.3): `ok` with what its value can be, `err` with what its value can
/// be, each a value of a sum with one field (`value`, `error`).
pub fn result_abs(ok: Abs, err: Abs) -> Abs {
    Abs::Variants([("err".to_string(), Rec { fields: vec![("error".into(), err)], rels: Vec::new() }), ("ok".to_string(), Rec { fields: vec![("value".into(), ok)], rels: Vec::new() })].into())
}

/// Two records joined: each field of the first with the same field of the second (anything where
/// the second has none), and the relations both keep.
fn join_rec(a: &Rec, b: &Rec) -> Rec {
    let fields = a.fields.iter().map(|(n, x)| (n.clone(), b.fields.iter().find(|(m, _)| m == n).map(|(_, y)| x.join(y)).unwrap_or(Abs::Any))).collect();
    let rels = a.rels.iter().filter(|r| b.rels.contains(r)).cloned().collect();
    Rec { fields, rels }
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
    /// The check across the languages that decides it, by the word `ritsu check` gives it
    /// (`ritsu_ports::Crossing::check`), for the places that wait for ritsu-cross.
    pub fn across(&self) -> Option<&'static str> {
        crossing_of(self).map(|(c, _)| c.check())
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
    /// A place for int64 that every way the value can leave int64 at comes from the end of a type, a
    /// value whose range is not written (DESIGN 22.2): nothing is said and no example is looked for
    /// (one of the type's ends is always among the vectors); the generated code checks it when it
    /// runs, and the function's effect has `overflow`.
    Unbounded,
    /// A check across the languages that ritsu-cross decides, whose answer this run has not had.
    Pending,
}

/// A place where the generated code can fail (DESIGN 4.2), with the value it is given.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    /// The expression the value is, or the operation that computes it.
    pub expr: usize,
    /// What it is among the places of its expression (`param:amount`, `overflow`): with the expression,
    /// what it is kept by.
    pub key: String,
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
    /// The call's expression.
    pub expr: usize,
    pub module: PathBuf,
    pub function: String,
    /// Whether the analysis read the function's body again with the values the call gives (DESIGN
    /// 26.5): what the call brings to the effect is what that reading found ([`FnFacts::rplaces`] and
    /// [`FnFacts::rcalls`]), not the function's own effect.
    pub reread: bool,
}

/// A call of a function value the analysis does not see, whose type writes an effect (DESIGN 27.5): the
/// call's expression and line, and the effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Thru {
    pub expr: usize,
    pub line: usize,
    pub effect: FnEffect,
}

/// A place a call's reading again found (DESIGN 26.5): the module and the function it is in, and the
/// place, its values joined over every call of the function that reads it again.
#[derive(Clone, Debug, PartialEq)]
pub struct Reread {
    pub module: PathBuf,
    pub function: String,
    pub place: Place,
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
    /// The places its calls read again found (DESIGN 26.5), every reading's places and those of the
    /// readings in them, joined by the module, the expression and the key: what the effect counts in
    /// place of the called functions' own places.
    pub rplaces: Vec<Reread>,
    /// The calls of functions the readings again made that are not read again themselves: their
    /// effects are counted too.
    pub rcalls: Vec<FnCall>,
    /// The calls of function values the analysis does not see whose type writes an effect (DESIGN
    /// 27.5), which the function's own effect takes; those of its readings again too.
    pub thru: Vec<Thru>,
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

/// The step of a [`Sym`] that sums a list by the fields after it (DESIGN 32): `xs` `⅀` `amount` is
/// `sum(xs, x => x.amount)`. No field is so named, and a branch narrows no such name.
const SUM: &str = "\u{2140}";

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
    /// What the conditions in force say of how two names compare. A name can also be a sum of a list
    /// by a field of its elements (`sum(xs, x => x.amount)`, [`SUM`], DESIGN 32).
    facts: Vec<(Sym, Rel, Sym)>,
    /// The lambda a builtin walks the elements of `running_totals(xs, x => x.amount)` with, by its id,
    /// and the sum each element's `total` is (DESIGN 32): what binding its parameter says.
    elem_total: Option<(usize, Sym)>,
    /// The value an expression came to last, where a condition reads it to narrow.
    last: BTreeMap<usize, V>,
    places: BTreeMap<(usize, String), Place>,
    calls: Vec<FnCall>,
    boundaries: Vec<Boundary>,
    relations: Vec<Kept>,
    sites: Vec<CallSite>,
    outputs: BTreeMap<(PathBuf, String), Iv>,
    /// How deep the reading again of the function analysed now is: 0 for a function's own analysis,
    /// one more for each reading in a reading (DESIGN 26.5).
    depth: usize,
    rplaces: BTreeMap<(PathBuf, usize, String), Reread>,
    rcalls: Vec<FnCall>,
    thru: Vec<Thru>,
    /// Whether the `match` whose arm names a value now splits a `result` (`ok x`, `err e`).
    result_split: bool,
    /// The analysis of a function going on (its own, or a reading again: DESIGN 27.4), by a number no
    /// other has, and the function: what a lambda made in it is written in.
    frame: usize,
    owner: usize,
    /// What the analyses of one module's functions share: the numbers of the frames, what the bodies of
    /// lambdas read elsewhere give the frames that made them, and the module analysed and what its
    /// functions answer so far (what a lambda of it read in another module's body calls).
    frames: Rc<Cell<usize>>,
    sinks: Rc<RefCell<BTreeMap<usize, Given>>>,
    home: Option<Rc<(PathBuf, Vec<Option<Abs>>)>>,
}

/// What the body of a lambda read where a function value of it is called gives the frame that made it
/// (DESIGN 27.4): its places, the calls and the readings again it makes, the calls of function values
/// that write an effect, the calls into other languages, and the values of its expressions.
#[derive(Default)]
struct Given {
    places: Vec<Place>,
    rplaces: Vec<Reread>,
    calls: Vec<FnCall>,
    rcalls: Vec<FnCall>,
    thru: Vec<Thru>,
    relations: Vec<Kept>,
    sites: Vec<CallSite>,
    at: Vec<(usize, Abs)>,
}

/// How deep the readings again of a call go, one in another (DESIGN 26.5).
pub const REREAD: usize = 4;

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

/// How many times the analysis follows a record or an enum that holds itself along a path (DESIGN
/// 28.1): a value of it, and one inside it; one inside that is a value the analysis knows nothing of
/// ([`Abs::Any`]). A type that holds itself in several fields would be followed into each of them at
/// each step: two steps keep the values the analysis carries small.
const UNFOLD: usize = 2;

/// How the analysis reads the ends a place does not write (DESIGN 22.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Whole {
    /// As no end: a value nothing more is known of.
    No,
    /// As the ends of the type, unmarked: what a function's answer is cut to.
    Cut,
    /// As the ends of the type, marked: a parameter, and the answer of a function in a ring.
    Marked,
}

/// An interval with the ends it has none at those of `whole`, marked or not.
fn fill(iv: Iv, whole: Option<Range>, marked: bool) -> Iv {
    let Some(w) = whole else { return iv };
    let mut out = iv;
    if out.lo.is_none() {
        out.lo = w.min;
        out.open_lo = marked;
    }
    if out.hi.is_none() {
        out.hi = w.max;
        out.open_hi = marked;
    }
    out
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
            elem_total: None,
            last: BTreeMap::new(),
            places: BTreeMap::new(),
            calls: Vec::new(),
            boundaries: Vec::new(),
            relations: Vec::new(),
            sites: Vec::new(),
            outputs: BTreeMap::new(),
            depth: 0,
            rplaces: BTreeMap::new(),
            rcalls: Vec::new(),
            thru: Vec::new(),
            result_split: false,
            frame: 0,
            owner: 0,
            frames: Rc::new(Cell::new(0)),
            sinks: Rc::new(RefCell::new(BTreeMap::new())),
            home: None,
        }
    }

    /// An analyzer of the module `m` sharing this one's frames and sinks, and what the module analysed
    /// answers so far.
    fn beside(&self, m: &'a Checked) -> Analyzer<'a> {
        let mut an = Analyzer::new(m, self.p, self.suite);
        an.frames = self.frames.clone();
        an.sinks = self.sinks.clone();
        an.home = Some(self.home.clone().unwrap_or_else(|| Rc::new((self.m.canon.clone(), self.results.clone()))));
        an.depth = self.depth;
        an
    }

    /// What the functions of the module `canon` answer, as far as this analysis knows.
    fn results_of(&self, canon: &std::path::Path) -> Vec<Option<Abs>> {
        if canon == self.m.canon.as_path() {
            return self.results.clone();
        }
        if let Some(h) = &self.home
            && h.0 == canon
        {
            return h.1.clone();
        }
        let c = self.p.module(canon);
        match c.analysis.get() {
            Some(a) => a.fns.iter().map(|x| Some(x.result.clone())).collect(),
            None => vec![None; c.decls.fns.len()],
        }
    }

    // ── Types to values ─────────────────────────────────────────────────────

    /// Every value of a type: what a value nothing more is known of can be. A record of a module
    /// keeps the ranges its fields declare, which the interpreter holds every one to as it is built
    /// or comes in.
    pub fn top(&self, t: &Ty) -> Abs {
        self.top_in(t, 0, &[])
    }

    /// The values of records and enums that hold themselves (DESIGN 28.1) a value of `t` is inside,
    /// `nest`, with its own: None where it would be one more of one of them than the analysis follows
    /// ([`UNFOLD`]).
    fn nest_in(&self, t: &Ty, nest: &[Ty]) -> Option<Vec<Ty>> {
        let mut out = nest.to_vec();
        if let Some(k) = self.p.nest_key(t) {
            if nest.iter().filter(|x| **x == k).count() >= UNFOLD {
                return None;
            }
            out.push(k);
        }
        Some(out)
    }

    fn top_in(&self, t: &Ty, depth: usize, nest: &[Ty]) -> Abs {
        if depth > 8 {
            return Abs::Any;
        }
        let Some(inner) = self.nest_in(t, nest) else { return Abs::Any };
        let inner = inner.as_slice();
        match t {
            Ty::Num(n) => match n.konst {
                Some(k) => Abs::Num(Iv::exact(k)),
                None => Abs::Num(Iv { den: step_den(t), ..Iv::TOP }),
            },
            Ty::Date => Abs::Date(Iv { den: Some(1), ..Iv::TOP }),
            Ty::Bool => Abs::bools(),
            Ty::Enum(id) if self.p.is_sum(id) => self.variants_in(id, None, &|me: &Self, fs: &Slot| me.slot_in(fs, false, Whole::No, depth + 1, inner)),
            Ty::Enum(id) => Abs::Set(Some(self.p.values(id).0.iter().map(|v| v.text.clone()).collect())),
            Ty::Record(r @ RecId::Variant { value, .. }) => match r.sum() {
                Some(id) => self.variants_in(&id, Some(value), &|me: &Self, fs: &Slot| me.slot_in(fs, false, Whole::No, depth + 1, inner)),
                None => Abs::Any,
            },
            Ty::Str => Abs::Str(Iv::new(Some(Rat::zero()), None, Some(1))),
            Ty::Opt(x) => Abs::Opt(true, Box::new(self.top_in(x, depth, inner))),
            Ty::List(x) => Abs::List(Iv::new(Some(Rat::zero()), None, Some(1)), Box::new(self.top_in(x, depth, inner))),
            Ty::Dict(k, x) => Abs::Dict(Iv::new(Some(Rat::zero()), None, Some(1)), Box::new(self.top_in(k, depth, inner)), Box::new(self.top_in(x, depth, inner))),
            Ty::Set(x) => Abs::Members(Iv::new(Some(Rat::zero()), None, Some(1)), Box::new(self.top_in(x, depth, inner))),
            Ty::EmptyDict => Abs::Dict(Iv::ints(0, 0), Box::new(Abs::Never), Box::new(Abs::Never)),
            Ty::Record(id) => {
                let module = matches!(id, RecId::Module { .. });
                let fields = self
                    .p
                    .fields(id)
                    .iter()
                    .map(|(f, s)| (f.text.clone(), if module { self.slot_in(s, false, Whole::No, depth + 1, inner) } else { self.top_in(&s.ty, depth + 1, inner) }))
                    .collect();
                Abs::Record(Rec { fields, rels: Vec::new() })
            }
            Ty::Transfer(id) => Abs::Transfer(self.p.transfer(id).map(|(_, ps)| ps.iter().map(|(n, t)| (n.clone(), self.top_in(t, depth, inner))).collect()).unwrap_or_default()),
            // a value of a type parameter: anything, in the analysis of the function itself (DESIGN 26.5)
            Ty::Param(_) => Abs::Any,
            // a function the analysis does not see, with the effect its type writes (DESIGN 27.4)
            Ty::Fn(_, _, eff) => Abs::Fns(Vec::new(), Some(eff.clone())),
            // `ok` with what its value can be, `err` with what its value can be (DESIGN 26.3)
            Ty::Result(a, e) => result_abs(self.top_in(a, depth + 1, inner), self.top_in(e, depth + 1, inner)),
            Ty::None => Abs::none(),
            Ty::Empty => Abs::List(Iv::ints(0, 0), Box::new(Abs::Never)),
            Ty::Error => Abs::Any,
        }
    }

    /// The values a place declares (a parameter, a field, a result): its type, cut to its range and
    /// its count, and the ends it does not write those of the type (DESIGN 22.1). `reached`: a
    /// parameter's, where a rate with a step and no range is held to 0% to 100%, the fields of the
    /// records it reaches to their ranges (DESIGN 3.9, 4.4), and the type's ends are marked (DESIGN
    /// 22.2); a result's otherwise, its type's ends unmarked, as a function's answer is cut to them.
    pub fn slot(&self, s: &Slot, reached: bool) -> Abs {
        self.slot_in(s, reached, if reached { Whole::Marked } else { Whole::Cut }, 0, &[])
    }

    /// The same, inside the values of records and enums that hold themselves `nest` (DESIGN 28.1). A
    /// value of one is not held again where kaname calls a function, only where it is built or comes
    /// in, so a parameter reaching it takes what its fields declare, and a rate with a step and no
    /// range in it is not 0% to 100% (`reached` is not passed into it).
    fn slot_in(&self, s: &Slot, reached: bool, whole: Whole, depth: usize, nest: &[Ty]) -> Abs {
        if depth > 8 {
            return Abs::Any;
        }
        let Some(inner) = self.nest_in(&s.ty, nest) else { return Abs::Any };
        let inner = inner.as_slice();
        let reached = reached && !self.p.recursive(&s.ty);
        let marked = whole == Whole::Marked;
        match &s.ty {
            Ty::Opt(x) => Abs::Opt(true, Box::new(self.slot_in(&Slot { ty: (**x).clone(), ..s.clone() }, reached, whole, depth, inner))),
            Ty::List(x) => {
                let written = s.count.clone().unwrap_or_default();
                let mut count = Iv::new(written.min.or(Some(Rat::zero())), written.max, Some(1));
                if whole != Whole::No {
                    // a count not written is 0 to 2⁶³ − 1
                    if written.min.is_none() {
                        count.open_lo = marked;
                    }
                    if written.max.is_none() {
                        count = Iv { hi: Some(Rat::int(INT_MAX)), open_hi: marked, ..count };
                    }
                }
                let elem = self.slot_in(&Slot { ty: (**x).clone(), range: s.range.clone(), count: None }, reached, whole, depth, inner);
                Abs::List(count, Box::new(elem))
            }
            // a string's count is of its code points (DESIGN 25.6): written, or 0 to 2⁶³ − 1
            Ty::Str => {
                let written = s.count.clone().unwrap_or_default();
                let mut count = Iv::new(written.min.or(Some(Rat::zero())), written.max, Some(1));
                if whole != Whole::No {
                    if written.min.is_none() {
                        count.open_lo = marked;
                    }
                    if written.max.is_none() {
                        count = Iv { hi: Some(Rat::int(INT_MAX)), open_hi: marked, ..count };
                    }
                }
                Abs::Str(count)
            }
            // a dict's count is of its pairs, its range of its values; a set's of its members
            Ty::Dict(_, x) | Ty::Set(x) => {
                let written = s.count.clone().unwrap_or_default();
                let mut count = Iv::new(written.min.or(Some(Rat::zero())), written.max, Some(1));
                if whole != Whole::No {
                    if written.min.is_none() {
                        count.open_lo = marked;
                    }
                    if written.max.is_none() {
                        count = Iv { hi: Some(Rat::int(INT_MAX)), open_hi: marked, ..count };
                    }
                }
                let elem = self.slot_in(&Slot { ty: (**x).clone(), range: s.range.clone(), count: None }, reached, whole, depth, inner);
                match &s.ty {
                    Ty::Dict(k, _) => Abs::Dict(count, Box::new(self.slot_in(&Slot::of((**k).clone()), reached, whole, depth, inner)), Box::new(elem)),
                    _ => Abs::Members(count, Box::new(elem)),
                }
            }
            Ty::Num(n) => {
                let range = match (&s.range, reached && n.is_rate() && n.step.is_some()) {
                    (Some(r), _) => Some(r.clone()),
                    (None, true) => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
                    _ => None,
                };
                let iv = Iv { den: step_den(&s.ty), ..Iv::TOP };
                let iv = match range {
                    Some(r) => iv.cut(&r),
                    None => iv,
                };
                Abs::Num(if whole == Whole::No { iv } else { fill(iv, whole_range(&s.ty), marked) })
            }
            Ty::Date => {
                let iv = Iv { den: Some(1), ..Iv::TOP };
                let iv = match &s.range {
                    Some(r) => iv.cut(r),
                    None => iv,
                };
                Abs::Date(if whole == Whole::No { iv } else { fill(iv, whole_range(&Ty::Date), marked) })
            }
            Ty::Record(id @ RecId::Module { .. }) => {
                let fields = self.p.fields(id).iter().map(|(f, fs)| (f.text.clone(), self.slot_in(fs, reached, whole, depth + 1, inner))).collect();
                Abs::Record(Rec { fields, rels: Vec::new() })
            }
            // a value of a sum, each value's fields as a record's (DESIGN 23.7)
            Ty::Enum(id) if self.p.is_sum(id) => self.variants_in(id, None, &|me: &Self, fs: &Slot| me.slot_in(fs, reached, whole, depth + 1, inner)),
            Ty::Record(r @ RecId::Variant { value, .. }) => match r.sum() {
                Some(id) => self.variants_in(&id, Some(value), &|me: &Self, fs: &Slot| me.slot_in(fs, reached, whole, depth + 1, inner)),
                None => Abs::Any,
            },
            Ty::Result(a, e) => result_abs(self.slot_in(&Slot::of((**a).clone()), reached, whole, depth + 1, inner), self.slot_in(&Slot::of((**e).clone()), reached, whole, depth + 1, inner)),
            t => self.top_in(t, depth, nest),
        }
    }

    /// Every value of a sum (or the one value `only`), each with what its fields can be as `field`
    /// says.
    fn variants_in(&self, id: &EnumId, only: Option<&String>, field: &dyn Fn(&Self, &Slot) -> Abs) -> Abs {
        let mut out = BTreeMap::new();
        for v in self.p.values(id).0 {
            if only.is_some_and(|o| *o != v.text) {
                continue;
            }
            let fields = self.p.value_fields(id, &v.text).iter().map(|(f, fs)| (f.text.clone(), field(self, fs))).collect();
            out.insert(v.text.clone(), Rec { fields, rels: Vec::new() });
        }
        Abs::Variants(out)
    }

    /// A value of an enum written by its name: a value of a sum with no fields, or one of the values
    /// of an enum with none.
    fn enum_value(&self, id: &EnumId, v: &str) -> Abs {
        if self.p.is_sum(id) { Abs::variant(v, Vec::new()) } else { set_of(&[v]) }
    }

    /// What a function answers is cut to: what its result declares, the type's ends where it writes
    /// none (unmarked: an end the analysis found keeps its own mark).
    fn declared(&self, info: &crate::names::FnInfo) -> Abs {
        self.slot(&info.ret, false)
    }

    /// What a function answers when what it answers is not followed (a function in a ring of calls,
    /// DESIGN 5.4): what its result declares, and the whole of its type, marked, where it declares no
    /// range (DESIGN 22.2).
    fn ring_answer(&self, info: &crate::names::FnInfo) -> Abs {
        self.slot_in(&info.ret, false, Whole::Marked, 0, &[])
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

    /// A call of a builtin, written as a function or after a value: its name and its arguments, the
    /// value it is written after first (DESIGN 21.4).
    fn builtin_call<'e>(&self, e: &'e Expr) -> Option<(&'static str, Vec<&'e Expr>)> {
        let ExprKind::Call(callee, args) = &e.kind else { return None };
        match (rf(self.m, callee), &callee.kind) {
            (Ref::Builtin(name), _) => Some((name, args.iter().map(Arg::expr).collect())),
            (Ref::Method(name), ExprKind::Field(base, _, _)) => Some((name, std::iter::once(&**base).chain(args.iter().map(Arg::expr)).collect())),
            _ => None,
        }
    }

    /// The fields a lambda of one parameter answers of it: `x => x.amount` is `[amount]`, `x => x`
    /// is `[]`; None for any other lambda. Two such lambdas are the same function whatever their
    /// parameters are named.
    fn field_lambda(&self, lam: &Expr) -> Option<Vec<String>> {
        let ExprKind::Lambda(ps, body) = &lam.kind else { return None };
        let [(p, _)] = ps.as_slice() else { return None };
        let (n, fs) = self.sym(body)?;
        (n == *p).then_some(fs)
    }

    /// The name a sum of a list by a field of its elements is in the facts (DESIGN 32): the list's,
    /// then [`SUM`], then the fields. `sum(xs, x => x.amount)` and `xs.sum(l => l.amount)` are one
    /// sum; the list is a name, and a name means one value in a function (E006).
    fn sum_term(&self, list: &Expr, lam: &Expr) -> Option<Sym> {
        let (n, mut fs) = self.sym(list)?;
        fs.push(SUM.to_string());
        fs.extend(self.field_lambda(lam)?);
        Some((n, fs))
    }

    /// A call of `sum` by a field of the elements of a list that is a name, as a name the facts can
    /// hold.
    fn summed(&self, e: &Expr) -> Option<Sym> {
        match self.builtin_call(e)? {
            ("sum", xs) if xs.len() == 2 => self.sum_term(xs[0], xs[1]),
            _ => None,
        }
    }

    /// The sum each element's `total` is, where a builtin walks the list `running_totals(xs, x =>
    /// x.amount)` answers (DESIGN 32): `sum(xs, x => x.amount)`, read in the same type.
    fn totals_of(&self, list: &Expr) -> Option<Sym> {
        let ("running_totals", xs) = self.builtin_call(list)? else { return None };
        let [l, lam] = xs.as_slice() else { return None };
        self.sum_term(l, lam)
    }

    /// The list, the dict or the set a call of `count` with no condition counts, when it is a name:
    /// `count(xs)`, `xs.count()` (DESIGN 24.5).
    fn counted(&self, e: &Expr) -> Option<Sym> {
        let ExprKind::Call(callee, args) = &e.kind else { return None };
        match (rf(self.m, callee), args.as_slice(), &callee.kind) {
            (Ref::Builtin("count"), [a], _) => self.sym(a.expr()),
            (Ref::Method("count"), [], ExprKind::Field(base, _, _)) => self.sym(base),
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
                    // the field of each value of a sum that has it
                    Abs::Variants(vs) => Abs::Variants(
                        vs.iter()
                            .map(|(v, r)| {
                                let fields = r.fields.iter().map(|(n, x)| (n.clone(), if n == f { put(x, rest, to.clone()) } else { x.clone() })).collect();
                                (v.clone(), Rec { fields, rels: r.rels.clone() })
                            })
                            .collect(),
                    ),
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
                self.places.insert(k.clone(), Place { expr: e.id, key: k.1, span: e.span, kind, what, value, lin, ty: t.clone(), outcome, across: None });
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
            Abs::Num(i) => Abs::Num(i.held64()),
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
            Abs::Num(i) => i.meet(Iv { lo: range.min, hi: range.max, ..Iv::TOP }).map(Abs::Num),
            Abs::Date(i) => i.meet(Iv { lo: range.min, hi: range.max, ..Iv::TOP }).map(Abs::Date),
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
        self.function_with(f, ring, None)
    }

    /// The analysis of the function `f`, its parameters given the values `given` where the call reading
    /// it again gives them (DESIGN 26.5), else what they declare.
    fn function_with(&mut self, f: usize, ring: &BTreeSet<usize>, given: Option<Vec<Option<Abs>>>) -> FnFacts {
        self.ring = ring.clone();
        self.env.clear();
        self.facts.clear();
        self.last.clear();
        self.places.clear();
        self.calls.clear();
        self.boundaries.clear();
        self.relations.clear();
        self.sites.clear();
        self.rplaces.clear();
        self.rcalls.clear();
        self.thru.clear();
        self.frame = self.frames.get() + 1;
        self.frames.set(self.frame);
        self.owner = f;
        let m = self.m;
        let info = &m.decls.fns[f];
        let decl = &m.module.fns[f];
        for (i, prm) in info.params.iter().enumerate() {
            let abs = match given.as_ref().and_then(|g| g.get(i).cloned().flatten()) {
                Some(a) => a,
                None => self.slot(&prm.slot, true),
            };
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
        // what the bodies of its lambdas gave it where they were read elsewhere (DESIGN 27.4)
        self.take_given();
        let mut places: Vec<Place> = std::mem::take(&mut self.places).into_values().collect();
        for p in &mut places {
            if p.what.across().is_none() {
                p.outcome = if self.shown(p) {
                    Outcome::Shown
                } else if matches!(p.what, What::Overflow) && by_type(&p.value) {
                    Outcome::Unbounded
                } else {
                    Outcome::Undecided
                };
            }
        }
        places.sort_by_key(|p| (p.span.line, p.span.col, p.expr));
        // the places the readings again found, decided on the values every reading gave them
        let mut rplaces: Vec<Reread> = std::mem::take(&mut self.rplaces).into_values().collect();
        for r in &mut rplaces {
            if r.place.what.across().is_none() {
                r.place.outcome = if self.shown(&r.place) {
                    Outcome::Shown
                } else if matches!(r.place.what, What::Overflow) && by_type(&r.place.value) {
                    Outcome::Unbounded
                } else {
                    Outcome::Undecided
                };
            }
        }
        FnFacts {
            result,
            places,
            calls: std::mem::take(&mut self.calls),
            boundaries: std::mem::take(&mut self.boundaries),
            relations: std::mem::take(&mut self.relations),
            refusals: refusals_of(&self.sites),
            sites: std::mem::take(&mut self.sites),
            rplaces,
            rcalls: std::mem::take(&mut self.rcalls),
            thru: std::mem::take(&mut self.thru),
            effect: crate::effects::Effect::default(),
        }
    }

    /// What the bodies of the lambdas made in this frame gave it where they were read in another
    /// (DESIGN 27.4): their places and values joined with its own, their calls, sites and relations
    /// kept with its own.
    fn take_given(&mut self) {
        let Some(g) = self.sinks.borrow_mut().remove(&self.frame) else { return };
        for p in g.places {
            self.join_place(p);
        }
        for r in g.rplaces {
            self.rplace(r);
        }
        self.calls.extend(g.calls);
        for c in g.rcalls {
            if !self.rcalls.contains(&c) {
                self.rcalls.push(c);
            }
        }
        for t in g.thru {
            if !self.thru.contains(&t) {
                self.thru.push(t);
            }
        }
        self.relations.extend(g.relations);
        for site in g.sites {
            self.site(site);
        }
        for (i, a) in g.at {
            if let Some(slot) = self.at.get_mut(i) {
                *slot = Some(match slot.take() {
                    None => a,
                    Some(b) => b.join(&a),
                });
            }
        }
    }

    /// A place found elsewhere, joined with the one kept already by its expression and key.
    fn join_place(&mut self, p: Place) {
        let k = (p.expr, p.key.clone());
        match self.places.get_mut(&k) {
            Some(x) => {
                x.value = x.value.join(&p.value);
                if x.lin != p.lin {
                    x.lin = None;
                }
            }
            None => {
                self.places.insert(k, p);
            }
        }
    }

    /// The body of the function `g` of the module `callee`, read again with the values a call gives its
    /// parameters (DESIGN 26.5): what it answers then, cut to what its result declares. What the
    /// reading finds (its places, those of the readings in it, and the calls it makes that are not
    /// read again) is kept with the function analysed now, for its effect; nothing of it goes to the
    /// values of the callee's expressions, nor to the callee's places.
    fn reread(&mut self, callee: &'a Checked, g: usize, given: Vec<Option<Abs>>) -> Abs {
        let mut an = self.beside(callee);
        an.depth = self.depth + 1;
        an.results = self.results_of(&callee.canon);
        let ring: BTreeSet<usize> = crate::effects::rings(callee).into_iter().find(|(r, _)| r.contains(&g)).map(|(r, _)| r.into_iter().collect()).unwrap_or_else(|| [g].into());
        let facts = an.function_with(g, &ring, Some(given));
        let fname = callee.decls.fns[g].name.text.clone();
        for p in facts.places {
            self.rplace(Reread { module: callee.canon.clone(), function: fname.clone(), place: p });
        }
        for r in facts.rplaces {
            self.rplace(r);
        }
        for c in facts.calls.into_iter().filter(|c| !c.reread).chain(facts.rcalls) {
            if !self.rcalls.contains(&c) {
                self.rcalls.push(c);
            }
        }
        for t in facts.thru {
            if !self.thru.contains(&t) {
                self.thru.push(t);
            }
        }
        facts.result
    }

    /// A place a reading again found, joined with the one kept already by its module, expression and
    /// key.
    fn rplace(&mut self, r: Reread) {
        let k = (r.module.clone(), r.place.expr, r.place.key.clone());
        match self.rplaces.get_mut(&k) {
            Some(x) => {
                x.place.value = x.place.value.join(&r.place.value);
                if x.place.lin != r.place.lin {
                    x.place.lin = None;
                }
            }
            None => {
                self.rplaces.insert(k, r);
            }
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
        breach(self.p, a, s, reached).is_none()
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
            ExprKind::Str(x) => {
                let n = crate::text::count(x) as i128;
                V::of(Abs::Str(Iv::ints(n, n)))
            }
            ExprKind::Bool(b) => V::of(set_of(&[&b.to_string()])),
            ExprKind::None => V::of(Abs::none()),
            ExprKind::Var(w) => match rf(m, e) {
                Ref::EnumValue(id, v) => V::of(self.enum_value(id, v)),
                Ref::Local => self.lookup(w).cloned().unwrap_or(V::of(Abs::Any)),
                _ => V::of(Abs::Any),
            },
            ExprKind::Field(base, _, _) => match rf(m, e) {
                Ref::EnumValue(id, v) => V::of(self.enum_value(id, v)),
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
            ExprKind::Record(_, base, inits) => self.record(e, base.as_deref(), inits),
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
            // a dict written out: from one pair to as many as are written (two keys of a value may be
            // one), each key and value read in the dict's types (DESIGN 24.5)
            ExprKind::Dict(pairs) => {
                let (kt, vt) = match ty(m, e) {
                    Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
                    _ => (Ty::Error, Ty::Error),
                };
                let (mut keys, mut vals) = (Abs::Never, Abs::Never);
                for (k, v) in pairs {
                    let a = self.ev(k);
                    if matches!(a.abs, Abs::Never) {
                        return V::of(Abs::Never);
                    }
                    let a = self.conv(a, &ty(m, k).clone(), &kt, k);
                    let b = self.ev(v);
                    if matches!(b.abs, Abs::Never) {
                        return V::of(Abs::Never);
                    }
                    let b = self.conv(b, &ty(m, v).clone(), &vt, v);
                    keys = keys.join(&a.abs);
                    vals = vals.join(&b.abs);
                }
                let n = pairs.len() as i128;
                V::of(Abs::Dict(Iv::ints(n.min(1), n), Box::new(keys), Box::new(vals)))
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
            // a lambda made a value: itself, with what the names in scope are now (DESIGN 27.4)
            ExprKind::Lambda(..) => {
                let env = Rc::new(self.env.iter().map(|l| (l.name.clone(), l.v.abs.clone())).collect());
                V::of(Abs::Fns(vec![Lam { module: m.canon.clone(), expr: e.id, owner: self.owner, frame: self.frame, env }], None))
            }
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
                                Some(Rel::Le) => iv = iv.lift_lo(Some(Rat::zero()), false),
                                Some(Rel::Lt) => iv = iv.lift_lo(Some(step.unwrap_or(Rat::zero())), false),
                                None => {}
                            }
                            match self.relation(a, b) {
                                Some(Rel::Le) => iv = iv.drop_hi(Some(Rat::zero()), false),
                                Some(Rel::Lt) => iv = iv.drop_hi(Some(step.map(|s| Rat::zero().sub(s)).unwrap_or(Rat::zero())), false),
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
            // two values of a sum: no value the same, or the one value with no fields (DESIGN 23.7)
            (Abs::Variants(x), Abs::Variants(y)) if matches!(op, BinOp::Eq | BinOp::Ne) && x.keys().all(|k| !y.contains_key(k)) => Some(op == BinOp::Ne),
            (Abs::Variants(x), Abs::Variants(y)) if matches!(op, BinOp::Eq | BinOp::Ne) && x.len() == 1 && x.keys().eq(y.keys()) && x.values().chain(y.values()).all(|r| r.fields.is_empty()) => {
                Some(op == BinOp::Eq)
            }
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
                    && let Some(now) = self.sym_abs(&list)
                    && let Some(Abs::Num(other)) = self.last.get(&y.id).map(|v| v.abs.clone())
                {
                    let narrowed = match now {
                        Abs::List(c, el) => narrow_iv(c, o, other, Some(Rat::int(1))).map(|n| Abs::List(n, el)),
                        Abs::Dict(c, k, v) => narrow_iv(c, o, other, Some(Rat::int(1))).map(|n| Abs::Dict(n, k, v)),
                        Abs::Members(c, el) => narrow_iv(c, o, other, Some(Rat::int(1))).map(|n| Abs::Members(n, el)),
                        Abs::Str(c) => narrow_iv(c, o, other, Some(Rat::int(1))).map(Abs::Str),
                        _ => None,
                    };
                    if let Some(n) = narrowed {
                        self.narrow_to(&list, n);
                    }
                }
            }
        }
        let (sa, sb) = (self.sym(a), self.sym(b));
        // what the facts hold of a side: its name, or the sum of a list by a field (DESIGN 32)
        let (fa, fb) = (sa.clone().or_else(|| self.summed(a)), sb.clone().or_else(|| self.summed(b)));
        if fa.is_none() && fb.is_none() {
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
        if let (Some(p), Some(q), true) = (&fa, &fb, own && ordered) {
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

    /// A `match` (DESIGN 23.7): each arm reached by what its patterns take of the value, its block in
    /// the scope they narrow: `none` and `some x`, a value of an enum or a bool, a value with fields
    /// and the name the arm gives it, a literal, and `else`, which takes what the arms above leave.
    fn matching(&mut self, e: &Expr, s: &Expr, arms: &[Arm]) -> V {
        let m = self.m;
        let sv = self.ev(s);
        if matches!(sv.abs, Abs::Never) {
            return V::of(Abs::Never);
        }
        let st = ty(m, s).clone();
        let outer_split = self.result_split;
        let sym = self.sym(s);
        let mut out = Abs::Never;
        let mut from: Vec<Src> = Vec::new();
        // the values the arms above take, by their names: what `else` leaves
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for arm in arms {
            let (mark, fmark) = (self.env.len(), self.facts.len());
            let mut reached = false;
            // the literals of the arm the value can be, joined: what the value is narrowed to there
            // (none where a literal reached is no number or date)
            let mut lits: Option<Abs> = None;
            let mut lit_other = false;
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
                    Pattern::Else(_) => {
                        if let Some(rest) = rest_of(&sv.abs, &taken) {
                            reached = true;
                            if let (Some(sy), Some(a)) = (&sym, rest) {
                                self.narrow_to(sy, a);
                            }
                        }
                    }
                    Pattern::Lit(x) => {
                        let lit = crate::interp::literal(m, x, &st);
                        let exact = match (&sv.abs, &lit) {
                            (Abs::Num(i), Some(Value::Num(r))) => i.has(*r).then(|| Abs::Num(Iv::exact(*r))),
                            (Abs::Date(i), Some(Value::Date(d))) => i.has(Rat::int(*d as i128)).then(|| Abs::Date(Iv::ints(*d as i128, *d as i128))),
                            (Abs::Never, _) => None,
                            _ => Some(sv.abs.clone()),
                        };
                        if let Some(a) = exact {
                            reached = true;
                            if matches!(a, Abs::Num(_) | Abs::Date(_)) {
                                lits = Some(match lits {
                                    Some(b) => b.join(&a),
                                    None => a,
                                });
                            } else {
                                lit_other = true;
                            }
                        }
                    }
                    Pattern::Bool(..) | Pattern::Value(..) => {}
                }
            }
            if let (Some(a), false, Some(sy)) = (lits, lit_other, &sym) {
                self.narrow_to(sy, a);
            }
            let named: Vec<String> = arm
                .patterns
                .iter()
                .filter_map(|p| match p {
                    Pattern::Bool(b, _) => Some(b.to_string()),
                    Pattern::Value(path, _, _) => {
                        let word = &path[path.len() - 1].0;
                        match st.present() {
                            Ty::Enum(id) => self.p.values(id).0.iter().find(|n| n.is(word)).map(|n| n.text.clone()),
                            // `ok` and `err` (DESIGN 26.3)
                            Ty::Result(..) => Some(word.clone()),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect();
            if !named.is_empty() {
                self.result_split = matches!(st.present(), Ty::Result(..));
                match &sv.abs {
                    // the values of a sum the arm names that the value can be, each with its fields
                    Abs::Variants(vs) => {
                        let can: BTreeMap<String, Rec> = vs.iter().filter(|(v, _)| named.contains(v)).map(|(v, r)| (v.clone(), r.clone())).collect();
                        if !can.is_empty() {
                            reached = true;
                            if let Some(sy) = &sym {
                                self.narrow_to(sy, Abs::Variants(can.clone()));
                            }
                            self.bind_value(arm, &sv, Abs::Variants(can));
                        }
                    }
                    Abs::Set(Some(vs)) => {
                        let can: Vec<String> = named.iter().filter(|n| vs.contains(*n)).cloned().collect();
                        if !can.is_empty() {
                            reached = true;
                            if let Some(sy) = &sym {
                                self.narrow_to(sy, Abs::Set(Some(can.into_iter().collect())));
                            }
                        }
                    }
                    _ => {
                        reached = true;
                        if !matches!(st.present(), Ty::Enum(id) if self.p.is_sum(id))
                            && !matches!(st.present(), Ty::Result(..))
                            && let Some(sy) = &sym
                        {
                            self.narrow_to(sy, Abs::Set(Some(named.iter().cloned().collect())));
                        }
                        self.bind_value(arm, &sv, Abs::Any);
                    }
                }
                taken.extend(named);
            }
            for p in &arm.patterns {
                match p {
                    Pattern::None(_) => {
                        taken.insert("none".into());
                    }
                    Pattern::Some(..) => {
                        taken.insert("some".into());
                    }
                    _ => {}
                }
            }
            self.result_split = outer_split;
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

    /// The name an arm gives a value with fields (`placed p`): the value as the arm narrows it, the
    /// part of a parameter it is through its name (DESIGN 23.7).
    fn bind_value(&mut self, arm: &Arm, sv: &V, abs: Abs) {
        let [Pattern::Value(path, Some((x, _)), _)] = arm.patterns.as_slice() else { return };
        let word = &path[path.len() - 1].0;
        // `ok x` and `err e` name the value the `ok` or the `err` holds (DESIGN 26.3): its part of a
        // parameter is the value's field
        if self.result_split {
            let field = if word == "ok" { "value" } else { "error" };
            let path = sv.path.clone().map(|(i, mut steps)| {
                steps.push(Step::Value(word.clone()));
                steps.push(Step::Field(field.into()));
                (i, steps)
            });
            let a = match &abs {
                Abs::Any => Abs::Any,
                a => a.field(field),
            };
            self.bind(x, V { abs: a, path, lin: None, from: Vec::new() });
            return;
        }
        let path = sv.path.clone().map(|(i, mut steps)| {
            steps.push(Step::Value(word.clone()));
            (i, steps)
        });
        self.bind(x, V { abs, path, lin: None, from: Vec::new() });
    }

    // ── Records ─────────────────────────────────────────────────────────────

    /// A record built, or a value of a sum (DESIGN 23.7): each field written, the others taken from
    /// the value it is made from (`..x`), each held to what it declares where the record is a
    /// module's (the place of a field taken is at `..x`).
    fn record(&mut self, e: &Expr, base: Option<&Expr>, inits: &[FieldInit]) -> V {
        let m = self.m;
        let Ref::Record(id) = rf(m, e) else { return V::of(Abs::Any) };
        let fields = self.p.fields(id);
        let from = match base {
            Some(b) => {
                let v = self.ev(b);
                if matches!(v.abs, Abs::Never) {
                    return V::of(Abs::Never);
                }
                Some((v.abs, b))
            }
            None => None,
        };
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
        let module = matches!(id, RecId::Module { .. } | RecId::Variant { .. });
        let shown_rec = self.p.show_ty(m, &Ty::Record(id.clone()));
        let mut out = Vec::new();
        for (k, (f, slot)) in fields.iter().enumerate() {
            let held = module && (slot.range.is_some() || slot.count.is_some());
            let abs = match &given[k] {
                Some((a, lin, x)) => {
                    if held {
                        let what = What::Field { record: shown_rec.clone(), field: f.text.clone(), slot: slot.clone() };
                        self.place(x, format!("field:{}", f.text), FailKind::Range, what, a.clone(), lin.clone(), &slot.ty);
                        if let (Some(r), Ty::Num(_) | Ty::Date) = (&slot.range, &slot.ty) {
                            let (r, t) = (r.clone(), slot.ty.clone());
                            self.assume(x, &r, &t);
                        }
                    }
                    a.clone()
                }
                None => match &from {
                    Some((fa, b)) => {
                        let a = fa.field(&f.text);
                        if held {
                            let what = What::Field { record: shown_rec.clone(), field: f.text.clone(), slot: slot.clone() };
                            self.place(b, format!("field:{}", f.text), FailKind::Range, what, a.clone(), None, &slot.ty);
                        }
                        a
                    }
                    None => Abs::none(),
                },
            };
            let abs = match (&abs, &slot.range, module) {
                (Abs::Num(i), Some(r), true) => Abs::Num(i.cut(r)),
                (Abs::Date(i), Some(r), true) => Abs::Date(i.cut(r)),
                (a, _, _) => a.clone(),
            };
            out.push((f.text.clone(), abs));
        }
        // a record built keeps no relation between its fields: only the records of `running_totals`
        // have them, and no `{ … }` builds one of those (DESIGN 23.7)
        V::of(match id {
            RecId::Variant { value, .. } => Abs::variant(value, out),
            _ => Abs::Record(Rec { fields: out, rels: Vec::new() }),
        })
    }

    // ── Calls ───────────────────────────────────────────────────────────────

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> V {
        // a function value called (DESIGN 27.4): what it is, then its arguments from the left
        if let Ref::CallValue = rf(self.m, e) {
            let f = self.ev(callee);
            if matches!(f.abs, Abs::Never) {
                return V::of(Abs::Never);
            }
            let Some(vals) = self.args(args) else { return V::of(Abs::Never) };
            let ps = match ty(self.m, callee) {
                Ty::Fn(ps, _, _) => ps.clone(),
                _ => Vec::new(),
            };
            let mut given = Vec::new();
            for (i, (_, x, v)) in vals.into_iter().enumerate() {
                let to = ps.get(i).cloned().unwrap_or(Ty::Error);
                let v = self.conv(v, &ty(self.m, x).clone(), &to, x);
                given.push(v.abs);
            }
            let ret = ty(self.m, e).clone();
            return V::of(self.call_value(e, &f.abs, &given, &ret));
        }
        match rf(self.m, callee) {
            Ref::Fn(canon, name) => self.call_fn(e, canon, name, args),
            Ref::Rule(canon) => self.call_rule(e, callee, canon, args),
            Ref::Date(canon, date) => self.call_date(e, callee, canon, date, args),
            Ref::Transfer(id) => self.call_transfer(e, id, args),
            Ref::Builtin(name) => {
                let xs: Vec<&Expr> = args.iter().map(Arg::expr).collect();
                self.builtin(e, name, &xs)
            }
            // a builtin written after a value: the value is its first argument (DESIGN 21.4)
            Ref::Method(name) => {
                let ExprKind::Field(base, _, _) = &callee.kind else { return V::of(Abs::Any) };
                let mut xs: Vec<&Expr> = vec![&**base];
                xs.extend(args.iter().map(Arg::expr));
                self.builtin(e, name, &xs)
            }
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
        let callee: &'a Checked = self.p.module(canon);
        let Some(g) = callee.decls.fns.iter().position(|f| f.name.text == name) else { return V::of(Abs::Any) };
        let info = callee.decls.fns[g].clone();
        let same = canon == m.canon.as_path();
        // a generic function is read again with the values the call gives, where the readings in
        // readings are fewer than REREAD and it is not in the ring of the function analysed now
        // (DESIGN 26.5)
        let reread = info.reread() && self.depth < REREAD && !(same && self.ring.contains(&g));
        // the parameters with the types the call gives the type parameters (DESIGN 26.2)
        let names = crate::names::tnames(&info.tparams);
        let targs = m.typed.inst.get(&e.id).cloned().unwrap_or_default();
        let slots: Vec<Slot> = info.params.iter().map(|p| subst_slot(&p.slot, &names, &targs)).collect();
        let mut given: Vec<Option<Abs>> = vec![None; info.params.len()];
        for (n, x, v) in vals {
            let Some(n) = n else { continue };
            let Some(k) = info.params.iter().position(|p| p.name.is(n)) else { continue };
            let (prm, slot) = (&info.params[k], &slots[k]);
            let v = self.conv(v, &ty(m, x).clone(), &slot.ty, x);
            let what = What::Param { function: info.name.text.clone(), param: prm.name.text.clone(), slot: slot.clone() };
            let lin = v.sum();
            given[k] = Some(v.abs.clone());
            self.place(x, format!("param:{}", prm.name.text), FailKind::Range, what, v.abs, lin, &slot.ty);
            if let (Some(r), Ty::Num(_) | Ty::Date) = (&slot.range, &slot.ty) {
                let (r, t) = (r.clone(), slot.ty.clone());
                self.assume(x, &r, &t);
            }
        }
        self.calls.push(FnCall { line: e.span.line, expr: e.id, module: canon.to_path_buf(), function: info.name.text.clone(), reread });
        if reread {
            // a parameter no argument is given takes what it declares (an optional, none)
            for (k, prm) in info.params.iter().enumerate() {
                if given[k].is_none() {
                    given[k] = Some(self.slot(&slots[k], true));
                }
                let _ = prm;
            }
            return V::of(self.reread(callee, g, given));
        }
        // a function that takes functions, not read again: each lambda given read once, its parameters
        // the whole of their types, marked, so that its places are the caller's (DESIGN 27.4)
        for (k, a) in given.iter().enumerate() {
            let (Some(Abs::Fns(ls, _)), Ty::Fn(..)) = (a, &slots[k].ty) else { continue };
            for l in ls.clone() {
                self.apply_whole(&l);
            }
        }
        // what a function that takes functions answers, not read again, is what it declares: its own
        // analysis answered for functions it does not see (DESIGN 27.4)
        let result = if info.takes_fn() {
            None
        } else if same {
            if self.ring.contains(&g) { None } else { self.results[g].clone() }
        } else {
            callee.analysis.get().and_then(|a| a.fns.get(g)).map(|f| f.result.clone())
        };
        V::of(result.unwrap_or_else(|| self.ring_answer(&info)))
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
        // an end the rule does not bound is the end of the type, which the generated code returns
        // its answer in, as a parameter's with no range is (DESIGN 22.1, 32)
        let iv = fill(iv, whole_range(strip(&t)), true);
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

    /// A function value called with `args` at `at` (DESIGN 27.4): each lambda it can be read with them,
    /// the answers joined, read in `ret`; one the analysis does not see answers the whole of `ret`,
    /// marked, makes no place, and gives the function the effect its type writes, where it writes one.
    fn call_value(&mut self, at: &Expr, f: &Abs, args: &[Abs], ret: &Ty) -> Abs {
        let (lams, unseen) = match f {
            Abs::Fns(ls, u) => (ls.clone(), u.clone()),
            Abs::Never => return Abs::Never,
            _ => (Vec::new(), Some(None)),
        };
        let mut out = Abs::Never;
        for l in &lams {
            let a = self.apply_lam(l, args, Some(ret));
            out = out.join(&a);
        }
        if let Some(w) = unseen {
            if let Some(w) = w {
                let t = Thru { expr: at.id, line: at.span.line, effect: w };
                if !self.thru.contains(&t) {
                    self.thru.push(t);
                }
            }
            out = out.join(&self.slot_in(&Slot::of(ret.clone()), true, Whole::Marked, 0, &[]));
        }
        out
    }

    /// The body of the lambda `l` read with its parameters bound to `args` (DESIGN 27.4): in the scope it
    /// was made in, with no condition in force, in the ring of the function it is written in. Its answer
    /// is read in the type of the lambda's answer, then in `to`. Read in the frame that made it, it is
    /// read here; else by an analyzer of its module in that frame, which gives the frame its places, its
    /// calls and the values of its expressions.
    fn apply_lam(&mut self, l: &Lam, args: &[Abs], to: Option<&Ty>) -> Abs {
        let lm: &'a Checked = self.p.module(&l.module);
        let Some((_, lam)) = lm.lambda(l.expr) else { return Abs::Any };
        let ExprKind::Lambda(ps, body) = &lam.kind else { return Abs::Any };
        let answer = match ty(lm, lam) {
            Ty::Fn(_, r, _) => (**r).clone(),
            t => t.clone(),
        };
        let env: Vec<Local> = l.env.iter().map(|(n, a)| Local { name: n.clone(), v: V::of(a.clone()) }).chain(ps.iter().zip(args).map(|((p, _), a)| Local { name: p.clone(), v: V::of(a.clone()) })).collect();
        let here = l.frame == self.frame && l.module == self.m.canon;
        let v = if here {
            let saved = (std::mem::replace(&mut self.env, env), std::mem::take(&mut self.facts));
            let v = self.ev(body);
            let v = self.conv(v, &ty(lm, body).clone(), &answer, body);
            (self.env, self.facts) = saved;
            v
        } else {
            let mut an = self.beside(lm);
            an.results = self.results_of(&l.module);
            an.ring = crate::effects::rings(lm).into_iter().find(|(r, _)| r.contains(&l.owner)).map(|(r, _)| r.into_iter().collect()).unwrap_or_else(|| [l.owner].into());
            an.frame = l.frame;
            an.owner = l.owner;
            an.env = env;
            let v = an.ev(body);
            let v = an.conv(v, &ty(lm, body).clone(), &answer, body);
            an.give();
            v
        };
        match to {
            Some(t) => self.conv(v, &answer, t, lam).abs,
            None => v.abs,
        }
    }

    /// The body of the lambda `l` read once with its parameters the whole of their types, marked (DESIGN
    /// 27.4): where no call that reads it with the values it is given is read.
    fn apply_whole(&mut self, l: &Lam) {
        let lm: &'a Checked = self.p.module(&l.module);
        let Some((_, lam)) = lm.lambda(l.expr) else { return };
        let Ty::Fn(ps, _, _) = ty(lm, lam) else { return };
        let whole: Vec<Abs> = ps.iter().map(|t| self.slot_in(&Slot::of(t.clone()), true, Whole::Marked, 0, &[])).collect();
        self.apply_lam(l, &whole, None);
    }

    /// What an analyzer of a lambda's body found, given to the frame that made the lambda.
    fn give(self) {
        let mut sinks = self.sinks.borrow_mut();
        let g = sinks.entry(self.frame).or_default();
        g.places.extend(self.places.into_values());
        g.rplaces.extend(self.rplaces.into_values());
        g.calls.extend(self.calls);
        g.rcalls.extend(self.rcalls);
        g.thru.extend(self.thru);
        g.relations.extend(self.relations);
        g.sites.extend(self.sites);
        g.at.extend(self.at.into_iter().enumerate().filter_map(|(i, a)| a.map(|a| (i, a))));
    }

    /// A function given to a builtin, analysed with its parameters bound to `params`: its answer,
    /// read in `to` where the builtin reads it so. A function value given to a builtin is read as the
    /// lambda that calls it (`wrapped.rs`).
    fn lambda(&mut self, lam: &Expr, params: Vec<V>, to: Option<&Ty>) -> V {
        let ExprKind::Lambda(ps, body) = &lam.kind else {
            return self.ev(lam);
        };
        let (mark, fmark) = (self.env.len(), self.facts.len());
        for ((p, _), v) in ps.iter().zip(params) {
            self.bind(p, v);
        }
        self.elem_total_fact(lam, ps);
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

    /// Where `lam` walks the running totals of a list by a field (DESIGN 32): its parameter's `total`
    /// is the sum of the list by that field, a fact while its body is read.
    fn elem_total_fact(&mut self, lam: &Expr, ps: &[(String, Span)]) {
        if let Some((id, t)) = &self.elem_total
            && *id == lam.id
            && let Some((p, _)) = ps.first()
        {
            self.facts.push(((p.clone(), vec!["total".to_string()]), Rel::Eq, t.clone()));
        }
    }

    /// A condition given to a builtin (`filter`, `first`), analysed on an element: whether it can
    /// hold and fail, and the element where it holds.
    fn condition(&mut self, lam: &Expr, elem: V) -> (bool, bool, Abs) {
        let ExprKind::Lambda(ps, body) = &lam.kind else { return (true, true, elem.abs) };
        let (mark, fmark) = (self.env.len(), self.facts.len());
        let Some((p, _)) = ps.first() else { return (true, true, elem.abs) };
        self.bind(p, elem.clone());
        self.elem_total_fact(lam, ps);
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

    fn builtin(&mut self, e: &Expr, name: &str, xs: &[&Expr]) -> V {
        let m = self.m;
        let me = ty(m, e).clone();
        match (name, xs.len()) {
            (
                "count" | "sum" | "any" | "all" | "map" | "first" | "last" | "max_of" | "min_of" | "running_totals" | "sort_by" | "sort_by_desc" | "max_by" | "min_by" | "flat_map" | "filter_map" | "group_by",
                2,
            ) => self.walk(e, name, xs[0], xs[1], &me),
            ("filter", 2) => self.filter(e, xs[0], xs[1], &me),
            ("to_dict", 3) => self.to_dict(xs[0], xs[1], xs[2], &me),
            ("map_values", 2) => self.map_values(xs[0], xs[1], &me),
            ("merge_with", 3) => self.merge_with(xs[0], xs[1], xs[2], &me),
            ("reduce", 3) => self.reduce(e, xs[0], xs[1], xs[2]),
            _ => {
                let mut vals: Vec<V> = Vec::new();
                for x in xs {
                    let v = self.ev(x);
                    if matches!(v.abs, Abs::Never) {
                        return V::of(Abs::Never);
                    }
                    vals.push(v);
                }
                self.scalar(e, name, xs, vals, &me)
            }
        }
    }

    /// `filter` of a dict (DESIGN 24.5): the pairs its condition can keep, from none to as many as
    /// the dict has; of a list, the walk's.
    fn filter(&mut self, e: &Expr, x: &Expr, lam: &Expr, me: &Ty) -> V {
        if !matches!(me, Ty::Dict(..)) {
            return self.walk(e, "filter", x, lam, me);
        }
        let d = self.ev(x);
        let (c, k, v) = match d.abs {
            Abs::Dict(c, k, v) => (c, *k, *v),
            Abs::Never => return V::of(Abs::Never),
            _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any, Abs::Any),
        };
        let entry = Abs::Record(Rec { fields: vec![("key".into(), k), ("value".into(), v)], rels: Vec::new() });
        let (_, _, held) = self.condition(lam, V::of(entry));
        V::of(Abs::Dict(up_to(c), Box::new(held.field("key")), Box::new(held.field("value"))))
    }

    /// `xs.to_dict(x => key, x => value)`: from one pair to as many as the list has elements.
    fn to_dict(&mut self, list: &Expr, kf: &Expr, vf: &Expr, me: &Ty) -> V {
        let (count, elem) = self.walked(list);
        if count.hi.is_some_and(|h| h.num < 0) {
            return V::of(Abs::Never);
        }
        let (kt, vt) = dict_tys(me);
        let k = self.lambda(kf, vec![elem.clone()], Some(&kt));
        let v = self.lambda(vf, vec![elem.clone()], Some(&vt));
        if count.hi.is_some_and(|h| h.num <= 0) || matches!(elem.abs, Abs::Never) {
            return V::of(Abs::Dict(Iv::ints(0, 0), Box::new(Abs::Never), Box::new(Abs::Never)));
        }
        V::of(Abs::Dict(one_up(count), Box::new(k.abs), Box::new(v.abs)))
    }

    /// `d.map_values(v => …)`: the same count and keys, the values the function answers.
    fn map_values(&mut self, x: &Expr, lam: &Expr, me: &Ty) -> V {
        let d = self.ev(x);
        let (c, k, v) = match d.abs {
            Abs::Dict(c, k, v) => (c, *k, *v),
            Abs::Never => return V::of(Abs::Never),
            _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any, Abs::Any),
        };
        let (_, vt) = dict_tys(me);
        let u = self.lambda(lam, vec![V::of(v.clone())], Some(&vt));
        let none = c.hi.is_some_and(|h| h.num <= 0) || matches!(v, Abs::Never);
        V::of(Abs::Dict(c, Box::new(k), Box::new(if none { Abs::Never } else { u.abs })))
    }

    /// `d.merge_with(o, (a, b) => …)`: the pairs of both, a value of either or what the function
    /// answers for a key of both.
    fn merge_with(&mut self, x: &Expr, o: &Expr, lam: &Expr, me: &Ty) -> V {
        let a = self.ev(x);
        if matches!(a.abs, Abs::Never) {
            return V::of(Abs::Never);
        }
        let b = self.ev(o);
        if matches!(b.abs, Abs::Never) {
            return V::of(Abs::Never);
        }
        let (ca, ka, va) = dict_parts(&a.abs);
        let (cb, kb, vb) = dict_parts(&b.abs);
        let (_, vt) = dict_tys(me);
        let r = self.lambda(lam, vec![V::of(va.clone()), V::of(vb.clone())], Some(&vt));
        let values = va.join(&vb).join(&r.abs);
        V::of(Abs::Dict(merged(ca, cb), Box::new(ka.join(&kb)), Box::new(values)))
    }

    /// A builtin that walks a list with a lambda (`map`, `filter`, `sum`, …): where the list is what
    /// `running_totals(xs, x => x.amount)` answers, the lambda's parameter is an element whose `total`
    /// is `sum(xs, x => x.amount)` (DESIGN 32).
    fn walk(&mut self, e: &Expr, name: &str, list: &Expr, lam: &Expr, me: &Ty) -> V {
        let total = self.totals_of(list).map(|t| (lam.id, t));
        let outer = std::mem::replace(&mut self.elem_total, total);
        let v = self.walk_in(e, name, list, lam, me);
        self.elem_total = outer;
        v
    }

    fn walk_in(&mut self, e: &Expr, name: &str, list: &Expr, lam: &Expr, me: &Ty) -> V {
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
                    _ => up_to(count),
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
                V::of(Abs::List(up_to(count), Box::new(held)))
            }
            "first" | "last" => {
                let (_, _, held) = self.condition(lam, elem);
                // an element that is none and no element at all are one none (DESIGN 3.6)
                let held = match held {
                    Abs::Opt(_, x) => *x,
                    x => x,
                };
                V::of(Abs::Opt(true, Box::new(held)))
            }
            // the same elements in another order (DESIGN 24.5): the keys are worked out for their places
            "sort_by" | "sort_by_desc" => {
                let _ = self.lambda(lam, vec![elem.clone()], None);
                V::of(Abs::List(count, Box::new(if none_at_all { Abs::Never } else { elem.abs })))
            }
            "max_by" | "min_by" => {
                let _ = self.lambda(lam, vec![elem.clone()], None);
                let empty = count.lo.is_none_or(|l| l.num <= 0);
                V::of(opt_of(empty, if none_at_all { Abs::Never } else { elem.abs }))
            }
            "flat_map" => {
                let v = self.lambda(lam, vec![elem], Some(me));
                let (ci, ei) = match v.abs {
                    Abs::List(c, x) => (c, *x),
                    _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any),
                };
                if none_at_all {
                    return V::of(Abs::List(Iv::ints(0, 0), Box::new(Abs::Never)));
                }
                V::of(Abs::List(times_count(count, ci), Box::new(ei)))
            }
            "filter_map" => {
                let to = match me {
                    Ty::List(t) => Some(Ty::Opt(t.clone())),
                    _ => None,
                };
                let v = self.lambda(lam, vec![elem], to.as_ref());
                let kept = match v.abs {
                    Abs::Opt(_, x) => *x,
                    x => x,
                };
                V::of(Abs::List(up_to(count), Box::new(if none_at_all { Abs::Never } else { kept })))
            }
            // the elements of a key, from one to as many as the list has, in a dict of a pair for each
            // key, from one to as many as the list has
            "group_by" => {
                let (kt, _) = dict_tys(me);
                let k = self.lambda(lam, vec![elem.clone()], Some(&kt));
                if none_at_all {
                    return V::of(Abs::Dict(Iv::ints(0, 0), Box::new(Abs::Never), Box::new(Abs::Never)));
                }
                let group = Abs::List(count_iv((Some(Rat::int(1)), false), (count.hi, count.open_hi)), Box::new(elem.abs));
                V::of(Abs::Dict(one_up(count), Box::new(k.abs), Box::new(group)))
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
                let every = sums(up_to(count), w, step_den(me));
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
                let fewer = count.hi.map(|h| h.sub(Rat::int(1))).map(|h| if h.num < 0 { Rat::zero() } else { h });
                let before = sums(Iv::marked((Some(Rat::zero()), false), (fewer, count.open_hi), Some(1), false), w, den);
                let upto = sums(Iv::marked((Some(Rat::int(1)), false), (count.hi, count.open_hi), Some(1), false), w, den);
                // every partial sum and the whole is held to int64
                let every = Abs::Num(sums(up_to(count), w, den));
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
            // `ok(x)` and `err(e)`: the value it holds read in the type of the `result` (DESIGN 26.3)
            "ok" | "err" if vals.len() == 1 => {
                let to = match me.present() {
                    Ty::Result(a, er) => if name == "ok" { (**a).clone() } else { (**er).clone() },
                    _ => Ty::Error,
                };
                let v = self.conv(vals[0].clone(), &ty(m, xs[0]).clone(), &to, xs[0]);
                V::of(Abs::variant(name, vec![(if name == "ok" { "value" } else { "error" }.to_string(), v.abs)]))
            }
            "min" | "max" if vals.len() == 2 => {
                let mut it = vals.into_iter();
                let (a, b) = (it.next().unwrap_or(V::of(Abs::Any)), it.next().unwrap_or(V::of(Abs::Any)));
                let a = self.conv(a, &ty(m, xs[0]).clone(), me, xs[0]);
                let b = self.conv(b, &ty(m, xs[1]).clone(), me, xs[1]);
                match (&a.abs, &b.abs) {
                    (Abs::Num(x), Abs::Num(y)) | (Abs::Date(x), Abs::Date(y)) => {
                        let iv = if name == "min" { x.min_of(*y) } else { x.max_of(*y) };
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
                    // a divisor that is a value: `zero` where it can be 0; the remainder from 0 to
                    // one less than its greatest size (Euclid's, DESIGN 24.5)
                    None => {
                        let d = iv(&vals[1]);
                        self.place(e, "zero".into(), FailKind::Zero, What::Zero, vals[1].abs.clone(), vals[1].sum(), &ty(m, xs[1]).clone());
                        if d.is_exact().is_some_and(|k| k.num == 0) {
                            return V::of(Abs::Never);
                        }
                        let mag = |v: Rat| if v.num < 0 { Rat::zero().sub(v) } else { v };
                        let size = match (d.lo, d.hi) {
                            (Some(l), Some(h)) => Some(if cmp(mag(l), mag(h)).is_gt() { mag(l) } else { mag(h) }),
                            _ => None,
                        };
                        Iv::marked((Some(Rat::zero()), false), (size.map(|s| s.sub(Rat::int(1))), d.open_lo || d.open_hi), Some(1), false)
                    }
                };
                V::of(Abs::Num(out))
            }
            // `numbers(a, b)`: the numbers from the least `a` to the greatest `b`; the count, b − a + 1
            // where it is above 0, held to int64 (DESIGN 24.5)
            "numbers" if vals.len() == 2 => {
                let (a, b) = (iv(&vals[0]), iv(&vals[1]));
                let raw = b.minus(a).plus(Iv::exact(Rat::int(1)));
                let lo = raw.lo.map(|l| if l.num < 0 { Rat::zero() } else { l });
                let count = Iv { lo, open_lo: raw.open_lo && lo.is_some_and(|l| l.num > 0), ..raw };
                self.overflow(e, &Abs::Num(count), &Ty::Num(NumTy::number()));
                let count = count_iv((count.lo, count.open_lo), (count.hi, count.open_hi));
                let elems = Iv::marked((a.lo, a.open_lo), (b.hi, b.open_hi), Some(1), false);
                if count.hi.is_some_and(|h| h.num <= 0) {
                    return V::of(Abs::List(Iv::ints(0, 0), Box::new(Abs::Never)));
                }
                V::of(Abs::List(count, Box::new(Abs::Num(elems))))
            }
            "isqrt" if vals.len() == 1 => {
                let v = &vals[0];
                let x = iv(v);
                self.place(xs[0], "isqrt".into(), FailKind::Range, What::Isqrt, v.abs.clone(), v.sum(), ty(m, xs[0]));
                let t0 = ty(m, xs[0]).clone();
                self.assume(xs[0], &Range { min: Some(Rat::zero()), max: None }, &t0);
                let lo = x.lo.map(|l| if l.num < 0 { Rat::zero() } else { l }).and_then(num::isqrt).unwrap_or(Rat::zero());
                let hi = x.hi.and_then(num::isqrt);
                let lo_m = x.open_lo && x.lo.is_some_and(|l| l.num >= 0);
                V::of(Abs::Num(Iv::marked((Some(lo), lo_m), (hi, x.open_hi), Some(1), false)))
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
                Some(Abs::List(c, _) | Abs::Dict(c, _, _) | Abs::Members(c, _) | Abs::Str(c)) => Abs::Num(*c),
                _ => Abs::Num(Iv::new(Some(Rat::zero()), None, Some(1))),
            }),
            "starts_with" => V::of(Abs::bools()),
            // lists one after another: the counts added, the elements of every one (DESIGN 24.5)
            "concat" if matches!(me, Ty::List(_) | Ty::Empty) => {
                let mut count = Iv::ints(0, 0);
                let mut elem = Abs::Never;
                for (x, v) in xs.iter().zip(vals) {
                    let v = self.conv(v, &ty(m, x).clone(), me, x);
                    let (c, el) = list_parts(&v.abs);
                    count = add_counts(count, c);
                    elem = elem.join(&el);
                }
                V::of(Abs::List(count, Box::new(elem)))
            }
            // strings one after another: their counts added (DESIGN 25.6)
            "concat" => {
                let count = vals.iter().fold(Iv::ints(0, 0), |acc, v| add_counts(acc, str_count(&v.abs)));
                V::of(Abs::Str(count))
            }
            "text" => V::of(Abs::Str(text_count(&vals[0].abs, ty(m, xs[0])))),
            "char" => V::of(Abs::Str(Iv::ints(1, 1))),
            _ if matches!(ty(m, xs[0]), Ty::Str) || matches!(name, "to_text" | "join") => self.string(name, xs, vals, me),
            _ => self.collection(e, name, xs, vals, me),
        }
    }

    /// The builtins of a string (DESIGN 25.6), from what their arguments can be, the string first; and
    /// `join` of a list of strings and `to_text` of a list of code points. A string's abstract value is
    /// the interval of its count of code points.
    fn string(&mut self, name: &str, xs: &[&Expr], vals: Vec<V>, me: &Ty) -> V {
        let _ = (xs, me);
        let first = vals.first().map(|v| v.abs.clone()).unwrap_or(Abs::Any);
        let arg = |i: usize| -> Abs { vals.get(i + 1).map(|v| v.abs.clone()).unwrap_or(Abs::Any) };
        let iv = |i: usize| -> Iv { vals.get(i + 1).and_then(|v| v.abs.iv()).unwrap_or(Iv::TOP) };
        let n = str_count(&first);
        let num_of = |c: Iv| Abs::Num(Iv { den: Some(1), open_den: false, ..c });
        let out = match name {
            // the lists of a string: one element for each code point
            "chars" => Abs::List(n, Box::new(Abs::Str(Iv::ints(1, 1)))),
            "code_points" => Abs::List(n, Box::new(Abs::Num(Iv::ints(0, 0x10_ffff)))),
            // a string of as many code points as the list has numbers, or none
            "to_text" => opt_of(true, Abs::Str(list_parts(&first).0)),
            // cut to 0 and to the count, as a list's (DESIGN 24.5)
            "take" => {
                let k = iv(0);
                let lo = min_lo(n.lo, k.lo.map(non_neg).or(Some(Rat::zero())));
                let hi = min_hi((n.hi, n.open_hi), (k.hi.map(non_neg), k.open_hi));
                Abs::Str(count_iv((lo, false), hi))
            }
            "drop" => {
                let k = iv(0);
                let most = k.hi.map(non_neg);
                let least = k.lo.map(non_neg).unwrap_or(Rat::zero());
                let lo = match (n.lo, most) {
                    (Some(l), Some(h)) => Some(l.sub(h)),
                    _ => Some(Rat::zero()),
                };
                Abs::Str(count_iv((lo, false), (n.hi.map(|h| h.sub(least)), n.open_hi)))
            }
            "slice" => {
                let (a, b) = (iv(0), iv(1));
                let from = a.lo.map(non_neg).unwrap_or(Rat::zero());
                let most = b.hi.map(|h| non_neg(h).sub(from));
                Abs::Str(count_iv((Some(Rat::zero()), false), min_hi((n.hi, n.open_hi), (most, b.open_hi))))
            }
            // where it is found: from 0 to the count of `s` less the count of `t`; or none
            "index_of" => {
                let t = str_count(&arg(0));
                let hi = n.hi.map(|h| h.sub(t.lo.map(non_neg).unwrap_or(Rat::zero())));
                if hi.is_some_and(|h| h.num < 0) {
                    Abs::none()
                } else {
                    opt_of(true, num_of(Iv::marked((Some(Rat::zero()), false), (hi, n.open_hi), Some(1), false)))
                }
            }
            "contains" | "ends_with" => Abs::bools(),
            // from one piece (none for an empty string cut at each code point) to one more than the
            // count; each piece no longer than the string
            "split" => {
                let sep = str_count(&arg(0));
                let lo = if sep.lo.is_some_and(|l| l.num >= 1) { Some(Rat::int(1)) } else { min_lo(n.lo, Some(Rat::int(1))) };
                let count = count_iv((lo, false), (n.hi.map(|h| h.add(Rat::int(1))), n.open_hi));
                Abs::List(count, Box::new(Abs::Str(count_iv((Some(Rat::zero()), false), (n.hi, n.open_hi)))))
            }
            "words" => {
                let count = count_iv((Some(Rat::zero()), false), (n.hi.map(|h| h.add(Rat::int(1))), n.open_hi));
                Abs::List(count, Box::new(Abs::Str(count_iv((Some(Rat::zero()), false), (n.hi, n.open_hi)))))
            }
            // what a replacement makes is not followed: the whole of a count, its upper end marked
            "replace" => Abs::Str(count_iv((Some(Rat::zero()), false), (Some(Rat::int(INT_MAX)), true))),
            "trim" | "trim_start" | "trim_end" => Abs::Str(count_iv((Some(Rat::zero()), false), (n.hi, n.open_hi))),
            "lower_ascii" | "upper_ascii" => Abs::Str(n),
            // as long as the string, or as the width where the fill is not empty
            "pad_start" | "pad_end" => {
                let (w, fill) = (iv(0), str_count(&arg(1)));
                let lo = if fill.lo.is_some_and(|l| l.num >= 1) { most((n.lo, n.open_lo), (w.lo, w.open_lo)) } else { (n.lo, n.open_lo) };
                let hi = match (n.hi, w.hi) {
                    (Some(a), Some(b)) => higher((Some(a), n.open_hi), (Some(b), w.open_hi)),
                    _ => (None, n.open_hi || w.open_hi),
                };
                Abs::Str(count_iv(lo, hi))
            }
            // a number of int64, or a day of the calendar, or none; their ends the type's
            "parse_number" => opt_of(true, Abs::Num(Iv::whole(INT_MIN, INT_MAX, Some(1)))),
            "parse_date" => opt_of(true, Abs::Date(Iv::whole(crate::date::MIN as i128, crate::date::MAX as i128, Some(1)))),
            // the elements' counts and the separators': from the fewest of the shortest to the most of
            // the longest, the lower ends held to 0 and above
            "join" => {
                let (c, el) = list_parts(&first);
                let (e, sep) = (str_count(&el), str_count(&arg(0)));
                let joined = |count: End, each: End, between: End| -> End {
                    let count = count?;
                    let gaps = if count.num > 0 { count.sub(Rat::int(1)) } else { Rat::zero() };
                    count.checked_mul(each?)?.checked_add(gaps.checked_mul(between?)?)
                };
                let lo = joined(c.lo.map(non_neg), e.lo.map(non_neg), sep.lo.map(non_neg));
                let hi = joined(c.hi.map(non_neg), e.hi, sep.hi);
                Abs::Str(count_iv((lo.or(Some(Rat::zero())), false), (hi, c.open_hi || e.open_hi || sep.open_hi)))
            }
            _ => self.top(me),
        };
        V::of(out)
    }

    /// The builtins of a list, a dict and a set that take no function (DESIGN 24.5), from what their
    /// arguments can be, the list, the dict or the set first.
    fn collection(&mut self, _e: &Expr, name: &str, xs: &[&Expr], vals: Vec<V>, me: &Ty) -> V {
        let m = self.m;
        // the i-th argument after the first, read in the type `to`
        let arg = |me_: &mut Self, i: usize, to: &Ty| -> Abs {
            match (xs.get(i + 1), vals.get(i + 1)) {
                (Some(x), Some(v)) => me_.conv(v.clone(), &ty(m, x).clone(), to, x).abs,
                _ => Abs::Any,
            }
        };
        let iv = |i: usize| -> Iv { vals.get(i + 1).and_then(|v| v.abs.iv()).unwrap_or(Iv::TOP) };
        let first = vals.first().map(|v| v.abs.clone()).unwrap_or(Abs::Any);
        let t0 = ty(m, xs[0]).clone();
        match &t0 {
            Ty::Dict(kt, vt) => {
                let (c, k, v) = dict_parts(&first);
                match name {
                    "get" => {
                        let _ = arg(self, 0, kt);
                        V::of(opt_of(true, v))
                    }
                    // the key read in the dict's keys' unit, as the interpreter reads it
                    "has" => {
                        let _ = arg(self, 0, kt);
                        V::of(Abs::bools())
                    }
                    "put" => {
                        let (a, b) = (arg(self, 0, kt), arg(self, 1, vt));
                        let count = count_iv((c.lo.map(|l| if l.num < 1 { Rat::int(1) } else { l }), c.open_lo), (c.hi.map(|h| h.add(Rat::int(1))), c.open_hi));
                        V::of(Abs::Dict(count, Box::new(k.join(&a)), Box::new(v.join(&b))))
                    }
                    "remove" => {
                        let _ = arg(self, 0, kt);
                        V::of(Abs::Dict(count_iv((c.lo.map(|l| l.sub(Rat::int(1))), c.open_lo), (c.hi, c.open_hi)), Box::new(k), Box::new(v)))
                    }
                    "keys" => V::of(Abs::List(c, Box::new(k))),
                    "values" => V::of(Abs::List(c, Box::new(v))),
                    "entries" => V::of(Abs::List(c, Box::new(Abs::Record(Rec { fields: vec![("key".into(), k), ("value".into(), v)], rels: Vec::new() })))),
                    "merge" => {
                        let (cb, kb, vb) = dict_parts(&arg(self, 0, &t0));
                        V::of(Abs::Dict(merged(c, cb), Box::new(k.join(&kb)), Box::new(v.join(&vb))))
                    }
                    _ => V::of(self.top(me)),
                }
            }
            Ty::Set(et) => {
                let (c, x) = members_parts(&first);
                match name {
                    "has" => {
                        let _ = arg(self, 0, et);
                        V::of(Abs::bools())
                    }
                    "add" => {
                        let a = arg(self, 0, et);
                        V::of(Abs::Members(count_iv((c.lo.map(|l| if l.num < 1 { Rat::int(1) } else { l }), c.open_lo), (c.hi.map(|h| h.add(Rat::int(1))), c.open_hi)), Box::new(x.join(&a))))
                    }
                    "remove" => {
                        let _ = arg(self, 0, et);
                        V::of(Abs::Members(count_iv((c.lo.map(|l| l.sub(Rat::int(1))), c.open_lo), (c.hi, c.open_hi)), Box::new(x)))
                    }
                    "union" => {
                        let (cb, xb) = members_parts(&arg(self, 0, &t0));
                        V::of(Abs::Members(merged(c, cb), Box::new(x.join(&xb))))
                    }
                    "intersect" => {
                        let (cb, _) = members_parts(&arg(self, 0, &t0));
                        V::of(Abs::Members(count_iv((Some(Rat::zero()), false), min_hi((c.hi, c.open_hi), (cb.hi, cb.open_hi))), Box::new(x)))
                    }
                    "difference" => {
                        let (cb, _) = members_parts(&arg(self, 0, &t0));
                        let lo = match (c.lo, cb.hi) {
                            (Some(l), Some(h)) => Some(l.sub(h)),
                            _ => Some(Rat::zero()),
                        };
                        V::of(Abs::Members(count_iv((lo, false), (c.hi, c.open_hi)), Box::new(x)))
                    }
                    "to_list" => V::of(Abs::List(c, Box::new(x))),
                    _ => V::of(self.top(me)),
                }
            }
            Ty::List(et) => {
                let (c, el) = list_parts(&first);
                let lo_of = |c: &Iv| c.lo.map(|l| l.num).unwrap_or(0);
                match name {
                    "first" | "last" => V::of(opt_of(lo_of(&c) <= 0, el)),
                    "at" => {
                        let _ = arg(self, 0, &Ty::Num(NumTy::number()));
                        V::of(opt_of(true, el))
                    }
                    "append" => {
                        let a = arg(self, 0, et);
                        let count = count_iv((c.lo.map(|l| l.add(Rat::int(1))), c.open_lo), (c.hi.map(|h| h.add(Rat::int(1))), c.open_hi));
                        V::of(Abs::List(count, Box::new(el.join(&a))))
                    }
                    // cut to 0 and to the count (DESIGN 24.3)
                    "take" => {
                        let n = iv(0);
                        let lo = min_lo(c.lo, n.lo.map(|l| if l.num < 0 { Rat::zero() } else { l }).or(Some(Rat::zero())));
                        let hi = min_hi((c.hi, c.open_hi), (n.hi.map(|h| if h.num < 0 { Rat::zero() } else { h }), n.open_hi));
                        V::of(Abs::List(count_iv((lo, false), hi), Box::new(el)))
                    }
                    "drop" => {
                        let n = iv(0);
                        let most = n.hi.map(|h| if h.num < 0 { Rat::zero() } else { h });
                        let least = n.lo.map(|l| if l.num < 0 { Rat::zero() } else { l }).unwrap_or(Rat::zero());
                        let lo = match (c.lo, most) {
                            (Some(l), Some(h)) => Some(l.sub(h)),
                            _ => Some(Rat::zero()),
                        };
                        let hi = c.hi.map(|h| h.sub(least));
                        V::of(Abs::List(count_iv((lo, false), (hi, c.open_hi)), Box::new(el)))
                    }
                    "slice" => {
                        let (a, b) = (iv(0), iv(1));
                        let from = a.lo.map(|l| if l.num < 0 { Rat::zero() } else { l }).unwrap_or(Rat::zero());
                        let most = b.hi.map(|h| if h.num < 0 { Rat::zero() } else { h }.sub(from));
                        V::of(Abs::List(count_iv((Some(Rat::zero()), false), min_hi((c.hi, c.open_hi), (most, b.open_hi))), Box::new(el)))
                    }
                    "reverse" | "sort" => V::of(Abs::List(c, Box::new(el))),
                    "contains" => {
                        let _ = arg(self, 0, et);
                        V::of(Abs::bools())
                    }
                    "distinct" => V::of(Abs::List(one_up(c), Box::new(el))),
                    "to_set" => V::of(Abs::Members(one_up(c), Box::new(el))),
                    "flatten" => {
                        let (ci, ei) = list_parts(&el);
                        V::of(Abs::List(times_count(c, ci), Box::new(ei)))
                    }
                    "indexed" => {
                        let index = Iv::marked((Some(Rat::zero()), false), (c.hi.map(|h| h.sub(Rat::int(1))), c.open_hi), Some(1), false);
                        let index = if index.hi.is_some_and(|h| h.num < 0) { Iv::ints(0, 0) } else { index };
                        V::of(Abs::List(c, Box::new(Abs::Record(Rec { fields: vec![("index".into(), Abs::Num(index)), ("item".into(), el)], rels: Vec::new() }))))
                    }
                    _ => V::of(self.top(me)),
                }
            }
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
            } else if matches!(pl.outcome, Outcome::Unbounded) {
                // no code says it: what E401 notes is the kind (DESIGN 22.3)
                said.insert((f, i), (unbounded_head(&place_src(c, pl.span)), "`overflow`".to_string()));
            }
        }
    }
    diags.extend(crate::effects::written_holds(c, &fns, &rings, &said));
    diags.extend(crate::effects::passed_hold(c, &fns, &an.at, &rings));
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

// ── What a value is not shown to keep ───────────────────────────────────────

/// A step from a value to a part of it a place holds (DESIGN 32).
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// Each element of a list, member of a set, value of a dict.
    Elem,
    /// Each key of a dict.
    Key,
    /// A field of a record of a module: the record, the field's name.
    Field(Ty, String),
    /// A field of a value of a sum: the sum, the value's name, the field's.
    Of(Ty, String, String),
}

/// What a part of a value is held to, where the analysis cannot show it keeps it (DESIGN 32).
#[derive(Clone, Debug, PartialEq)]
pub enum Held {
    /// The count of a list, a set or a dict: as written, or none written (then int64's).
    Count(Option<Range>),
    /// The code points of a string, to the count written.
    Chars(Range),
    /// The range of a number or a date: as written (a rate with a step and none written, 0% to
    /// 100%), or none written, and then the type's (DESIGN 22.1). The type the range is read in.
    Range(Option<Range>, Ty),
    /// The value as a whole: one the analysis does not follow, or nested too deep to follow.
    Whole,
}

/// The first part of a value a place cannot be shown to keep (DESIGN 32): where it is in the value
/// (`[]` the value itself), what it is held to, and what the analysis finds of it — the interval of
/// the number or the date, or of the count.
#[derive(Clone, Debug, PartialEq)]
pub struct Breach {
    pub path: Vec<Part>,
    pub held: Held,
    pub found: Option<Iv>,
}

/// The first part of the values `a` a place declaring `s` is not shown to keep, as the interpreter
/// holds a value to it (`interp::outside`), or None where every value keeps it. `reached`: a
/// parameter's, whose ends and counts not written are the type's and whose records of a module are
/// held field by field (DESIGN 22.1).
pub fn breach(p: &Program, a: &Abs, s: &Slot, reached: bool) -> Option<Breach> {
    breach_in(p, a, s, reached, 0)
}

fn breach_in(p: &Program, a: &Abs, s: &Slot, reached: bool, depth: usize) -> Option<Breach> {
    let here = |held: Held, found: Option<Iv>| Some(Breach { path: Vec::new(), held, found });
    let under = |part: Part, b: Breach| {
        let mut path = vec![part];
        path.extend(b.path);
        Breach { path, ..b }
    };
    if depth > 8 {
        return here(Held::Whole, None);
    }
    let a = match a {
        Abs::Opt(_, x) => &**x,
        x => x,
    };
    if matches!(a, Abs::Never) {
        return None;
    }
    let mut t = &s.ty;
    while let Ty::Opt(x) = t {
        t = x;
    }
    // a list's elements, a set's members and a dict's values, each to the range, their count to
    // the count; a dict's keys, where a parameter reaches them, to their type (`interp::outside`)
    if reached
        && let (Ty::Dict(k, _), Abs::Dict(_, ka, _)) = (t, a)
        && let Some(b) = breach_in(p, ka, &Slot::of((**k).clone()), true, depth + 1)
    {
        return Some(under(Part::Key, b));
    }
    let (elem, elem_ty, inside) = match (t, a) {
        (Ty::List(x), Abs::List(c, e)) | (Ty::Set(x), Abs::Members(c, e)) | (Ty::Dict(_, x), Abs::Dict(c, _, e)) => {
            let count = if reached { filled(s.count.as_ref(), Some(whole_count())) } else { s.count.clone() };
            if count.as_ref().is_some_and(|r| !c.within(r)) {
                return here(Held::Count(s.count.clone()), Some(*c));
            }
            let mut et = &**x;
            while let Ty::Opt(y) = et {
                et = y;
            }
            (match &**e {
                Abs::Opt(_, v) => (**v).clone(),
                v => v.clone(),
            }, et, true)
        }
        (Ty::List(_) | Ty::Set(_) | Ty::Dict(..), _) => return here(Held::Whole, None),
        // a string's code points to the count it writes (DESIGN 25.1); a string holds no more than
        // 2⁶³ − 1 of them wherever it is
        (Ty::Str, Abs::Str(c)) => return s.count.as_ref().filter(|r| !c.within(r)).and_then(|r| here(Held::Chars(r.clone()), Some(*c))),
        (Ty::Str, _) => return s.count.as_ref().and_then(|r| here(Held::Chars(r.clone()), None)),
        _ => (a.clone(), t, false),
    };
    let at = |b: Breach| Some(if inside { under(Part::Elem, b) } else { b });
    if matches!(elem, Abs::Never) {
        return None;
    }
    // a value of a record or an enum that holds itself is not held again where kaname calls a
    // function (`interp::outside`'s `Hold::Call`, DESIGN 28.1)
    if reached && p.recursive(elem_ty) {
        return None;
    }
    let written = match (&s.range, elem_ty) {
        (Some(r), _) => Some(r.clone()),
        (None, Ty::Num(n)) if reached && n.is_rate() && n.step.is_some() => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
        _ => None,
    };
    // where a parameter reaches it, an end not written is the type's (DESIGN 22.1)
    let range = if reached { filled(written.as_ref(), whole_range(elem_ty)) } else { written.clone() };
    if let Some(r) = &range
        && !elem.iv().is_some_and(|i| i.within(r))
    {
        // what is said is the range written, where the value is not shown to keep it, and the
        // type's where only an end not written is the one not kept
        let said = written.filter(|w| !elem.iv().is_some_and(|i| i.within(w)));
        return at(Breach { path: Vec::new(), held: Held::Range(said, elem_ty.clone()), found: elem.iv() });
    }
    if reached && let Ty::Record(id @ RecId::Module { .. }) = elem_ty {
        for (f, fs) in p.fields(id) {
            if let Some(b) = breach_in(p, &elem.field(&f.text), &fs, true, depth + 1) {
                return at(under(Part::Field(elem_ty.clone(), f.text.clone()), b));
            }
        }
    }
    // a value of a sum: the fields of each value it can be (of every value, where it can be anything)
    let sum = match elem_ty {
        Ty::Enum(id) if p.is_sum(id) => Some((id.clone(), None)),
        Ty::Record(r @ RecId::Variant { value, .. }) => r.sum().map(|id| (id, Some(value.clone()))),
        _ => None,
    };
    if reached && let Some((id, only)) = sum {
        for v in p.values(&id).0 {
            if only.as_ref().is_some_and(|o| *o != v.text) {
                continue;
            }
            let rec: Option<&Rec> = match &elem {
                Abs::Variants(vs) => match vs.get(&v.text) {
                    Some(r) => Some(r),
                    None => continue,
                },
                _ => None,
            };
            for (f, fs) in p.value_fields(&id, &v.text) {
                let fa = match rec {
                    Some(r) => r.fields.iter().find(|(n, _)| *n == f.text).map(|(_, a)| a.clone()).unwrap_or(Abs::Any),
                    None => Abs::Any,
                };
                if let Some(b) = breach_in(p, &fa, &fs, true, depth + 1) {
                    return at(under(Part::Of(Ty::Enum(id.clone()), v.text.clone(), f.text.clone()), b));
                }
            }
        }
    }
    None
}

/// What a place declares, as a person reads it and as it is written: `range >=0 <=5`, `count <=30`.
/// An end not written is not said, though a parameter takes the type's there (DESIGN 22.1, 32).
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

/// The part of a value a breach is at, read from the value out: `` `xs` ``, each element of `xs`, the
/// `share` of each element of `xs` (DESIGN 32).
fn part_text(src: &str, path: &[Part]) -> Text {
    let (mut ja, mut en) = (format!("`{src}`"), format!("`{src}`"));
    for part in path {
        let j = crate::diag::ja_before(&ja);
        (ja, en) = match part {
            Part::Elem => (format!("{j}の要素"), format!("each element of {en}")),
            Part::Key => (format!("{j}の鍵"), format!("each key of {en}")),
            Part::Field(_, f) => (format!("{j}の `{f}`"), format!("the `{f}` of {en}")),
            Part::Of(_, v, f) => (format!("{j}が {v} のときの `{f}`"), format!("the `{f}` of {en} where it is {v}")),
        };
    }
    Text { ja, en }
}

/// What the analysis finds of the part of a value a breach is at, as W301's note says it (DESIGN 32):
/// its interval, or the interval of its count, where an end is more than the type's (or, for a count,
/// than 0); else that it finds no bound on it.
fn part_note(src: &str, b: &Breach) -> Text {
    let Text { ja, en } = part_text(src, &b.path);
    let jp = crate::diag::ja_before(&ja);
    let (count, chars) = (matches!(b.held, Held::Count(_) | Held::Chars(_)), matches!(b.held, Held::Chars(_)));
    let (jw, ew) = match (count, chars) {
        (true, true) => (format!("{jp}のコードポイントの数"), format!("the count of the code points of {en}")),
        (true, false) => (format!("{jp}の要素の数"), format!("the count of {en}")),
        // the value itself, which the headline quotes, is said as at the other places: the value
        _ if b.path.is_empty() => ("値".to_string(), "the value".to_string()),
        _ => (ja.clone(), en.clone()),
    };
    let bound = b.found.map(|i| Iv { lo: i.lo.filter(|l| !i.open_lo && !(count && l.num <= 0)), hi: i.hi.filter(|_| !i.open_hi), ..i }).filter(|i| i.lo.is_some() || i.hi.is_some());
    let jb = crate::diag::ja_before(&jw);
    match (bound, &b.held) {
        (Some(i), Held::Range(_, t)) => {
            let at = iv_text(i, t);
            tr!("範囲の解析では、{jb}は {} です。関数のベクタにも、外れる入力はありませんでした。", "The analysis of ranges finds {ew} in {}, and no input among the function's vectors takes it out.", at.ja; at.en)
        }
        (Some(i), _) => {
            let at = iv_text(i, &Ty::Num(NumTy::number()));
            tr!("範囲の解析では、{jb}は {} です。関数のベクタにも、外れる入力はありませんでした。", "The analysis of ranges finds {ew} in {}, and no input among the function's vectors takes it out.", at.ja; at.en)
        }
        (None, _) => tr!("範囲の解析では、{jb}の範囲が決まりません。関数のベクタにも、外れる入力はありませんでした。", "The analysis of ranges finds no bound on {ew}, and no input among the function's vectors takes it out."),
    }
}

/// The headline of W301 at a parameter (DESIGN 32): the part of the value it cannot be decided for —
/// the value, each of its elements, a field of them — and whether its range or its count is not
/// shown, with the range or count written there; where none is written, the values of the type, not
/// the ends of int64 the type stands for.
fn param_head(c: &Checked, p: &Program, src: &str, to: &Text, b: &Breach) -> Text {
    let Text { ja, en } = part_text(src, &b.path);
    // what writes the range or the count: the last field on the way, or the parameter
    let owner = match b.path.iter().rev().find(|x| matches!(x, Part::Field(..) | Part::Of(..))) {
        Some(Part::Field(r, f)) => Some(tr!("{} のフィールド `{f}`", "the field `{f}` of {}", p.show_ty(c, r))),
        Some(Part::Of(s, v, f)) => Some(tr!("{} の値 {v} のフィールド `{f}`", "the field `{f}` of the value {v} of {}", p.show_ty(c, s))),
        _ => None,
    };
    let jp = crate::diag::ja_before(&ja);
    let (tj, oj) = (crate::diag::ja_before(&to.ja), owner.as_ref().map(|o| crate::diag::ja_before(&o.ja)));
    let num = Ty::Num(NumTy::number());
    match (&b.held, &owner) {
        (Held::Range(Some(r), t), None) => {
            let r = crate::value::range_text(r, t);
            tr!("{jp}が、{tj}の範囲 {r} に収まるかを決められません", "Whether {en} stays in {r}, the range of {}, cannot be decided", ; to.en)
        }
        (Held::Range(Some(r), t), Some(o)) => {
            let (r, oj) = (crate::value::range_text(r, t), oj.unwrap_or_default());
            tr!("{tj}に渡す {jp}が、{oj}の範囲 {r} に収まるかを決められません", "Whether {en}, given to {}, stays in {r}, the range of {}, cannot be decided", ; to.en, o.en)
        }
        (Held::Range(None, t), _) => {
            let t = p.show_ty(c, t);
            tr!("{tj}に渡す {jp}が、{t} の値に収まるかを決められません", "Whether {en}, given to {}, stays in the values of {t} cannot be decided", ; to.en)
        }
        (Held::Count(Some(r)), None) => {
            let r = crate::value::range_text(r, &num);
            tr!("{jp}の要素の数が、{tj}の要素の数の範囲 {r} に収まるかを決められません", "Whether the count of {en} stays in {r}, the count of {}, cannot be decided", ; to.en)
        }
        (Held::Count(Some(r)), Some(o)) => {
            let (r, oj) = (crate::value::range_text(r, &num), oj.unwrap_or_default());
            tr!("{tj}に渡す {jp}の要素の数が、{oj}の要素の数の範囲 {r} に収まるかを決められません", "Whether the count of {en}, given to {}, stays in {r}, the count of {}, cannot be decided", ; to.en, o.en)
        }
        (Held::Count(None), _) => tr!("{tj}に渡す {jp}の要素の数が、int64 に収まるかを決められません", "Whether the count of {en}, given to {}, stays in int64 cannot be decided", ; to.en),
        (Held::Chars(r), None) => {
            let r = crate::value::range_text(r, &num);
            tr!("{jp}のコードポイントの数が、{tj}のコードポイントの数の範囲 {r} に収まるかを決められません", "Whether the count of the code points of {en} stays in {r}, the count of {}, cannot be decided", ; to.en)
        }
        (Held::Chars(r), Some(o)) => {
            let (r, oj) = (crate::value::range_text(r, &num), oj.unwrap_or_default());
            tr!("{tj}に渡す {jp}のコードポイントの数が、{oj}のコードポイントの数の範囲 {r} に収まるかを決められません", "Whether the count of the code points of {en}, given to {}, stays in {r}, the count of {}, cannot be decided", ; to.en, o.en)
        }
        (Held::Whole, _) => tr!("{jp}が、{tj}の宣言に収まるかを決められません", "Whether {en} stays in what {} declares cannot be decided", ; to.en),
    }
}

/// The text of a place's expression on its line.
pub fn place_src(c: &Checked, sp: Span) -> String {
    let line = c.src.lines().nth(sp.line.saturating_sub(1)).unwrap_or("");
    let cs: Vec<char> = line.chars().collect();
    let (a, b) = (sp.col.saturating_sub(1).min(cs.len()), sp.end.saturating_sub(1).min(cs.len()));
    cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
}

/// What is said of a place for int64 whose value can leave int64 only from the end of a type
/// (`Outcome::Unbounded`, DESIGN 22.3): no diagnostic, but the note of E401 and the page's line.
pub fn unbounded_head(src: &str) -> Text {
    tr!("`{src}` の値は、範囲を書かない値から来るので、int64 を超えることがあります", "The value of `{src}` can leave int64, as it comes from a value whose range is not written")
}

/// The diagnostic of a place that is not shown, and its headline (what E401 notes again).
fn place_diag(c: &Checked, p: &Program, f: usize, pl: &Place) -> Option<(crate::diag::Diag, Text)> {
    let code = code_of(pl)?;
    let example = code.starts_with('E');
    let src = place_src(c, pl.span);
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
            // where it cannot be decided, which part of the value and what of it (DESIGN 32); an
            // example's value shows which, and the headline says what is written
            let to = tr!("関数 `{function}` の引数 `{param}`", "the parameter `{param}` of `{function}`");
            match breach(p, &pl.value, slot, true) {
                Some(b) if !example && b.held != Held::Whole => param_head(c, p, &src, &to, &b),
                _ => {
                    let d = decl_text(slot);
                    let decl = if d.is_empty() { tr!("関数 `{function}` の引数 `{param}` の宣言", "what the parameter `{param}` of `{function}` declares") } else { tr!("関数 `{function}` の引数 `{param}` の宣言（{d}）", "what the parameter `{param}` of `{function}` declares ({d})") };
                    let ja = crate::diag::ja_before(&decl.ja);
                    if example {
                        tr!("`{src}` が、{ja}の外になることがあります", "`{src}` can be outside {}", ; decl.en)
                    } else {
                        tr!("`{src}` が、{ja}に収まるかを決められません", "Whether `{src}` stays in {} cannot be decided", ; decl.en)
                    }
                }
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
            // at a parameter, a field and an answer, what the analysis finds of the part of the value
            // it cannot decide for: the value, an element, a field of one; its range or its count
            let part = match &pl.what {
                What::Param { slot, .. } => breach(p, &pl.value, slot, true).filter(|b| b.held != Held::Whole),
                What::Field { slot, .. } | What::Answer { slot, .. } => breach(p, &pl.value, slot, false).filter(|b| b.held != Held::Whole),
                _ => None,
            };
            match (&pl.what, pl.value.present(), pl.value.present().iv()) {
                _ if part.is_some() => {
                    d = d.note(part_note(&src, part.as_ref().unwrap()));
                }
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
                // an end that is the type's, where no range is written, is no bound (DESIGN 22.1, 32)
                (_, _, Some(i)) if (i.lo.is_some() && !i.open_lo) || (i.hi.is_some() && !i.open_hi) => {
                    let i = Iv { lo: i.lo.filter(|_| !i.open_lo), hi: i.hi.filter(|_| !i.open_hi), ..i };
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

/// Whether every way a value can leave int64 comes from the end of a type (DESIGN 22.2): a place for
/// `overflow` the analysis does not show, whose value comes from values whose ranges are not written
/// (`Outcome::Unbounded`). Never for a value that can be anything.
fn by_type(a: &Abs) -> bool {
    match a {
        Abs::Num(i) | Abs::Date(i) => i.leaves_by_type(),
        Abs::Opt(_, x) => by_type(x),
        _ => false,
    }
}

/// From none to as many as `count` can be: what `filter` keeps of a list, and the count of every
/// partial sum; the upper end marked as the count's.
fn up_to(count: Iv) -> Iv {
    Iv::marked((Some(Rat::zero()), false), (count.hi, count.open_hi), Some(1), false)
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
/// is 5/6), so the elements' bound is no bound on the sum's (DESIGN 5.5). An end is a count times an
/// end of `w`, marked as a product is.
fn sums(count: Iv, w: Iv, den: Option<i128>) -> Iv {
    let c_lo = count.lo.unwrap_or(Rat::zero());
    // a count times an end of a value, marked where one is and the other is not 0
    let times = |c: End, cm: bool, v: Rat, vm: bool| -> Marked {
        let p = c.and_then(|c| c.checked_mul(v));
        (p, (cm && v.num != 0) || (vm && c.is_none_or(|c| c.num != 0)))
    };
    let (lo, lo_m) = match w.lo {
        Some(l) if l.num < 0 => times(count.hi, count.open_hi, l, w.open_lo),
        Some(l) => times(Some(c_lo), count.open_lo, l, w.open_lo),
        None => (None, w.open_lo),
    };
    let (hi, hi_m) = match w.hi {
        Some(h) if h.num > 0 => times(count.hi, count.open_hi, h, w.open_hi),
        Some(h) => times(Some(c_lo), count.open_lo, h, w.open_hi),
        None => (None, w.open_hi),
    };
    let (lo, lo_m) = match (lo, count.lo.is_none_or(|l| l.num <= 0)) {
        (Some(l), true) if l.num > 0 => (Some(Rat::zero()), false),
        (x, _) => (x, lo_m),
    };
    let (hi, hi_m) = match (hi, count.lo.is_none_or(|l| l.num <= 0)) {
        (Some(h), true) if h.num < 0 => (Some(Rat::zero()), false),
        (x, _) => (x, hi_m),
    };
    Iv::marked((lo, lo_m), (hi, hi_m), den, false)
}

/// A count from `lo` to `hi`, each with its mark: held to 0 and above, and to 2⁶³ − 1 at most
/// (DESIGN 24.5); no upper end where none is said.
fn count_iv(lo: Marked, hi: Marked) -> Iv {
    let cap = |v: Rat| if v.num < 0 { Rat::zero() } else if cmp(v, Rat::int(INT_MAX)).is_gt() { Rat::int(INT_MAX) } else { v };
    let l = lo.0.map(cap).unwrap_or(Rat::zero());
    let h = hi.0.map(cap);
    let h = h.map(|h| if cmp(h, l).is_lt() { l } else { h });
    Iv::marked((Some(l), lo.1), (h, hi.1), Some(1), false)
}

/// 0 for a value below 0.
fn non_neg(v: Rat) -> Rat {
    if v.num < 0 { Rat::zero() } else { v }
}

/// The interval of a string's count of code points; from 0 for what is no string the analysis follows.
fn str_count(a: &Abs) -> Iv {
    match a {
        Abs::Str(c) => *c,
        Abs::Opt(_, x) => str_count(x),
        Abs::Never => Iv::ints(0, 0),
        _ => Iv::new(Some(Rat::zero()), None, Some(1)),
    }
}

/// The count of the code points `text` writes of a value of `t` (DESIGN 25.6): a whole number of int64
/// from 1 to 20 (`-9223372036854775808`), with the spelling of its unit after it; a date 10; `true`
/// and `false` 4 and 5; a value of an enum its name; a string itself.
fn text_count(a: &Abs, t: &Ty) -> Iv {
    let names = |s: &BTreeSet<String>| -> Iv {
        let ns: Vec<i128> = s.iter().map(|v| crate::text::count(v) as i128).collect();
        match (ns.iter().min(), ns.iter().max()) {
            (Some(lo), Some(hi)) => Iv::ints(*lo, *hi),
            _ => Iv::ints(0, 0),
        }
    };
    match (t, a) {
        (Ty::Num(n), _) if n.is_rate() => Iv::new(Some(Rat::zero()), None, Some(1)),
        (Ty::Num(n), _) => {
            let u = match &n.unit.dim {
                Dim::Number | Dim::Count(_) => 0,
                _ => crate::text::count(&n.unit.unit) as i128,
            };
            Iv::ints(1 + u, 20 + u)
        }
        (_, Abs::Date(_)) => Iv::ints(10, 10),
        (_, Abs::Set(Some(s))) => names(s),
        (_, Abs::Str(c)) => *c,
        (_, Abs::Never) => Iv::ints(0, 0),
        _ => Iv::new(Some(Rat::zero()), None, Some(1)),
    }
}

/// From one to the count, where it can be one or more: the keys of a list's elements, its first of
/// each (DESIGN 24.5).
fn one_up(c: Iv) -> Iv {
    let lo = c.lo.map(|l| if l.num > 1 { Rat::int(1) } else { l });
    count_iv((lo, c.open_lo), (c.hi, c.open_hi))
}

/// The lower of two lower ends; no end where either is none (no end is below every end).
fn min_lo(a: End, b: End) -> End {
    match (a, b) {
        (Some(x), Some(y)) => Some(if cmp(x, y).is_le() { x } else { y }),
        _ => None,
    }
}

/// The lower of two upper ends, with its mark; no end is above every end.
fn min_hi(a: Marked, b: Marked) -> Marked {
    match (a.0, b.0) {
        (Some(x), Some(y)) => {
            if cmp(x, y).is_le() {
                a
            } else {
                b
            }
        }
        (Some(_), None) => a,
        (None, _) => b,
    }
}

/// The count of two dicts or two sets put together: at least the more of their least, at most the
/// sum of their most.
fn merged(a: Iv, b: Iv) -> Iv {
    let lo = match (a.lo, b.lo) {
        (Some(x), Some(y)) => Some(if cmp(x, y).is_ge() { x } else { y }),
        (x, y) => x.or(y),
    };
    let hi = a.hi.zip(b.hi).map(|(x, y)| x.add(y));
    count_iv((lo, a.open_lo || b.open_lo), (hi, a.open_hi || b.open_hi))
}

/// Two counts added: the lists of `concat` one after another.
fn add_counts(a: Iv, b: Iv) -> Iv {
    let lo = a.lo.zip(b.lo).map(|(x, y)| x.add(y));
    let hi = a.hi.zip(b.hi).map(|(x, y)| x.add(y));
    count_iv((lo, a.open_lo || b.open_lo), (hi, a.open_hi || b.open_hi))
}

/// The count of the lists of a list put one after another: a count times a count (the lower ends
/// taken from 0: a list has no fewer than none).
fn times_count(outer: Iv, inner: Iv) -> Iv {
    let from0 = |v: Rat| if v.num < 0 { Rat::zero() } else { v };
    let lo = outer.lo.zip(inner.lo).and_then(|(x, y)| from0(x).checked_mul(from0(y)));
    let hi = outer.hi.zip(inner.hi).and_then(|(x, y)| x.checked_mul(y));
    count_iv((lo, outer.open_lo || inner.open_lo), (hi, outer.open_hi || inner.open_hi))
}

/// A value that is none where `may` says or where it may be none itself (anything may be none).
fn opt_of(may: bool, a: Abs) -> Abs {
    match a {
        Abs::Opt(n, x) => Abs::Opt(may || n, x),
        Abs::Any => Abs::Any,
        x => Abs::Opt(may, Box::new(x)),
    }
}

/// A list's count and elements; any count and anything for what is no list the analysis follows.
fn list_parts(a: &Abs) -> (Iv, Abs) {
    match a {
        Abs::List(c, x) => (*c, (**x).clone()),
        Abs::Opt(_, x) => list_parts(x),
        Abs::Never => (Iv::ints(0, 0), Abs::Never),
        _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any),
    }
}

/// A dict's count, keys and values.
fn dict_parts(a: &Abs) -> (Iv, Abs, Abs) {
    match a {
        Abs::Dict(c, k, x) => (*c, (**k).clone(), (**x).clone()),
        Abs::Opt(_, x) => dict_parts(x),
        Abs::Never => (Iv::ints(0, 0), Abs::Never, Abs::Never),
        _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any, Abs::Any),
    }
}

/// A set's count and members.
fn members_parts(a: &Abs) -> (Iv, Abs) {
    match a {
        Abs::Members(c, x) => (*c, (**x).clone()),
        Abs::Opt(_, x) => members_parts(x),
        Abs::Never => (Iv::ints(0, 0), Abs::Never),
        _ => (Iv::new(Some(Rat::zero()), None, Some(1)), Abs::Any),
    }
}

/// The types of a dict's keys and values.
fn dict_tys(t: &Ty) -> (Ty, Ty) {
    match t.present() {
        Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
        _ => (Ty::Error, Ty::Error),
    }
}

/// The ends of `acc` that moved away from `start`, let go: an end let go is no end of a type, for
/// the analysis stopped following it there (DESIGN 22.2).
fn widen(start: &Abs, acc: &Abs) -> Abs {
    match (start, acc) {
        (Abs::Num(s), Abs::Num(a)) | (Abs::Date(s), Abs::Date(a)) => {
            let (lo, open_lo) = if a.lo == s.lo { (a.lo, a.open_lo) } else { (None, false) };
            let (hi, open_hi) = if a.hi == s.hi { (a.hi, a.open_hi) } else { (None, false) };
            let iv = Iv { lo, hi, open_lo, open_hi, ..*a };
            if matches!(acc, Abs::Num(_)) { Abs::Num(iv) } else { Abs::Date(iv) }
        }
        (Abs::Opt(_, s), Abs::Opt(n, a)) => Abs::Opt(*n, Box::new(widen(s, a))),
        (Abs::List(sc, se), Abs::List(ac, ae)) => {
            let (hi, open_hi) = if ac.hi == sc.hi { (ac.hi, ac.open_hi) } else { (None, false) };
            Abs::List(Iv { hi, open_hi, ..*ac }, Box::new(widen(se, ae)))
        }
        (Abs::Members(sc, se), Abs::Members(ac, ae)) => {
            let (hi, open_hi) = if ac.hi == sc.hi { (ac.hi, ac.open_hi) } else { (None, false) };
            Abs::Members(Iv { hi, open_hi, ..*ac }, Box::new(widen(se, ae)))
        }
        // a string's count, as a list's (DESIGN 25.6)
        (Abs::Str(sc), Abs::Str(ac)) => {
            let (hi, open_hi) = if ac.hi == sc.hi { (ac.hi, ac.open_hi) } else { (None, false) };
            Abs::Str(Iv { hi, open_hi, ..*ac })
        }
        (Abs::Dict(sc, sk, se), Abs::Dict(ac, ak, ae)) => {
            let (hi, open_hi) = if ac.hi == sc.hi { (ac.hi, ac.open_hi) } else { (None, false) };
            Abs::Dict(Iv { hi, open_hi, ..*ac }, Box::new(widen(sk, ak)), Box::new(widen(se, ae)))
        }
        (Abs::Record(s), Abs::Record(a)) => Abs::Record(widen_rec(s, a)),
        // each value of a sum, its fields as a record's; a value the start was not, every field let go
        (Abs::Variants(s), Abs::Variants(a)) => Abs::Variants(
            a.iter()
                .map(|(v, r)| {
                    let w = match s.get(v) {
                        Some(sr) => widen_rec(sr, r),
                        None => Rec { fields: r.fields.iter().map(|(n, _)| (n.clone(), Abs::Any)).collect(), rels: r.rels.clone() },
                    };
                    (v.clone(), w)
                })
                .collect(),
        ),
        (_, a) => a.clone(),
    }
}

fn widen_rec(s: &Rec, a: &Rec) -> Rec {
    Rec { fields: a.fields.iter().map(|(n, x)| (n.clone(), s.fields.iter().find(|(m, _)| m == n).map(|(_, y)| widen(y, x)).unwrap_or(Abs::Any))).collect(), rels: a.rels.clone() }
}

/// A value cut to what is declared of it: the intervals met, field by field.
fn cut(a: &Abs, d: &Abs) -> Abs {
    match (a, d) {
        (Abs::Num(x), Abs::Num(y)) => Abs::Num(x.meet(*y).map(|i| i.with_den(x.den)).unwrap_or(*x)),
        (Abs::Date(x), Abs::Date(y)) => Abs::Date(x.meet(*y).unwrap_or(*x)),
        (Abs::Opt(n, x), Abs::Opt(_, y)) => Abs::Opt(*n, Box::new(cut(x, y))),
        (Abs::List(c, x), Abs::List(e, y)) => Abs::List(c.meet(*e).unwrap_or(*c), Box::new(cut(x, y))),
        (Abs::Members(c, x), Abs::Members(e, y)) => Abs::Members(c.meet(*e).unwrap_or(*c), Box::new(cut(x, y))),
        (Abs::Str(c), Abs::Str(e)) => Abs::Str(c.meet(*e).unwrap_or(*c)),
        (Abs::Dict(c, k, x), Abs::Dict(e, l, y)) => Abs::Dict(c.meet(*e).unwrap_or(*c), Box::new(cut(k, l)), Box::new(cut(x, y))),
        (x, _) => x.clone(),
    }
}

/// What `else` takes of a value, the arms above taking the values `taken` names (DESIGN 23.7): None
/// where nothing is left (the arm is never reached); Some with what the value is narrowed to there,
/// where that is said (the values of a sum, an enum or a bool left).
fn rest_of(a: &Abs, taken: &BTreeSet<String>) -> Option<Option<Abs>> {
    match a {
        Abs::Never => None,
        Abs::Variants(vs) => {
            let left: BTreeMap<String, Rec> = vs.iter().filter(|(v, _)| !taken.contains(*v)).map(|(v, r)| (v.clone(), r.clone())).collect();
            (!left.is_empty()).then_some(Some(Abs::Variants(left)))
        }
        Abs::Set(Some(vs)) => {
            let left: BTreeSet<String> = vs.iter().filter(|v| !taken.contains(*v)).cloned().collect();
            (!left.is_empty()).then_some(Some(Abs::Set(Some(left))))
        }
        Abs::Opt(n, x) => {
            let none_left = *n && !taken.contains("none");
            let some_left = !matches!(**x, Abs::Never) && !taken.contains("some");
            (none_left || some_left).then_some(None)
        }
        _ => Some(None),
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
    // an end narrowed takes the mark of the end of `y` it comes from (DESIGN 22.2)
    let out = match op {
        BinOp::Lt => x.drop_hi(y.hi.map(below), y.open_hi),
        BinOp::Le => x.drop_hi(y.hi, y.open_hi),
        BinOp::Gt => x.lift_lo(y.lo.map(above), y.open_lo),
        BinOp::Ge => x.lift_lo(y.lo, y.open_lo),
        BinOp::Eq => x.meet(y)?,
        BinOp::Ne => match y.is_exact() {
            Some(v) if x.lo == Some(v) && step.is_some() => Iv { lo: Some(above(v)), open_lo: y.open_lo, ..x },
            Some(v) if x.hi == Some(v) && step.is_some() => Iv { hi: Some(below(v)), open_hi: y.open_hi, ..x },
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
