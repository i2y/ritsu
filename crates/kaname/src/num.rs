//! Numbers as the reference interpreter holds them (DESIGN 3.3, 5.6, 8): an exact rational in the
//! unit of its type ([`Rat`], ritsu-units' reduced fraction of two `i128`s), the arithmetic that
//! holds every value to the rule of int64, and the five ways of rounding.
//!
//! The rule of int64 (DESIGN 5.6): a value, as a reduced fraction, has a numerator from −2⁶³ to
//! 2⁶³ − 1 and a denominator from 1 to 2⁶³ − 1; a value that leaves it is the failure `overflow`. A
//! rounded value has the denominator 1 in its unit (a rate's, its step's), so it is the value itself
//! that fits. Every operation is worked in `i128` from operands that keep the rule, where no product
//! of two of them overflows, and its answer is reduced and then held to the rule.
//!
//! The rounding is the one integer procedure of DESIGN 3.3, which the generated code repeats: the
//! value `n/d` on the step `gn/gd` is `|n·gd|` divided by `d·gn`, the quotient taken or one past it
//! by the way of rounding, the sign put back and the step multiplied in. It never leans on a target
//! language's own division, which rounds toward −∞ in Python and toward 0 in Go. `tests/rounding.rs`
//! holds it to rulec's `round_to`.

pub use ritsu_units::Rat;
use std::cmp::Ordering;

/// Why an operation has no answer: a value that leaves int64, or a division by zero (DESIGN 4.2,
/// `overflow` and `zero`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arith {
    Overflow,
    Zero,
}

/// The largest numerator and denominator the rule of int64 lets in.
const I64_MAX: i128 = i64::MAX as i128;
const I64_MIN: i128 = i64::MIN as i128;

/// Whether a value keeps the rule of int64 (DESIGN 5.6).
pub fn fits(r: Rat) -> bool {
    (I64_MIN..=I64_MAX).contains(&r.num) && (1..=I64_MAX).contains(&r.den)
}

/// A value, held to the rule of int64.
pub fn held(r: Option<Rat>) -> Result<Rat, Arith> {
    match r {
        Some(r) if fits(r) => Ok(r),
        _ => Err(Arith::Overflow),
    }
}

pub fn add(a: Rat, b: Rat) -> Result<Rat, Arith> {
    held(a.checked_add(b))
}

pub fn sub(a: Rat, b: Rat) -> Result<Rat, Arith> {
    held(a.checked_sub(b))
}

pub fn mul(a: Rat, b: Rat) -> Result<Rat, Arith> {
    held(a.checked_mul(b))
}

pub fn div(a: Rat, b: Rat) -> Result<Rat, Arith> {
    if b.num == 0 {
        return Err(Arith::Zero);
    }
    held(a.checked_div(b))
}

pub fn neg(a: Rat) -> Result<Rat, Arith> {
    held(a.num.checked_neg().map(|n| Rat { num: n, den: a.den }))
}

/// The order of two values. Both keep the rule of int64, so the cross products fit in `i128`.
pub fn cmp(a: Rat, b: Rat) -> Ordering {
    a.checked_cmp(b).unwrap_or_else(|| a.cmp_to(b))
}

/// The five ways of rounding, rulec's (DESIGN 3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// Away from 0.
    Up,
    /// Toward 0.
    Down,
    /// Half away from 0.
    HalfUp,
    /// Half toward 0.
    HalfDown,
    /// Half to the even neighbour.
    HalfEven,
}

impl Rounding {
    pub const ALL: [Rounding; 5] = [Rounding::Up, Rounding::Down, Rounding::HalfUp, Rounding::HalfDown, Rounding::HalfEven];

    /// The builtin that rounds this way.
    pub fn word(self) -> &'static str {
        match self {
            Rounding::Up => "up",
            Rounding::Down => "down",
            Rounding::HalfUp => "half_up",
            Rounding::HalfDown => "half_down",
            Rounding::HalfEven => "half_even",
        }
    }

    pub fn parse(w: &str) -> Option<Rounding> {
        Rounding::ALL.into_iter().find(|r| r.word() == w)
    }
}

/// `x` rounded to a whole number of the positive step `g` (DESIGN 3.3), held to the rule of int64.
pub fn round(x: Rat, way: Rounding, g: Rat) -> Result<Rat, Arith> {
    debug_assert!(g.num > 0 && g.den > 0, "a step of rounding is positive");
    let big = |o: Option<i128>| o.ok_or(Arith::Overflow);
    // x = n/d on the step gn/gd: the number of steps is (n·gd) / (d·gn)
    let top = big(x.num.checked_mul(g.den))?;
    let bottom = big(x.den.checked_mul(g.num))?;
    let (q, r) = (top.unsigned_abs() / bottom.unsigned_abs(), top.unsigned_abs() % bottom.unsigned_abs());
    let y = bottom.unsigned_abs();
    let past = r != 0
        && match way {
            Rounding::Down => false,
            Rounding::Up => true,
            Rounding::HalfUp => 2 * r >= y,
            Rounding::HalfDown => 2 * r > y,
            Rounding::HalfEven => 2 * r > y || (2 * r == y && q % 2 == 1),
        };
    let k = i128::try_from(q + u128::from(past)).map_err(|_| Arith::Overflow)?;
    let k = if top < 0 { -k } else { k };
    held(Rat::int(k).checked_mul(g))
}

/// `allocate(t, c, s)`: `t × c / s` rounded toward 0 to a whole unit (rulec's §15.102), each
/// value on the way held to the rule of int64; `zero` when `s` is 0.
pub fn allocate(t: Rat, c: Rat, s: Rat) -> Result<Rat, Arith> {
    let part = div(mul(t, c)?, s)?;
    round(part, Rounding::Down, Rat::int(1))
}

/// `mod(x, n)`: the remainder of the whole part of `x` by the positive whole `n`, from 0 to `n − 1`
/// whatever the sign of `x` (rulec's §15.214).
pub fn modulo(x: Rat, n: i128) -> Rat {
    let whole = x.num.div_euclid(x.den);
    Rat::int(whole.rem_euclid(n))
}

/// `isqrt(x)`: the largest whole number whose square is not above `x` (rulec's §15.215); None below
/// zero, where there is none.
pub fn isqrt(x: Rat) -> Option<Rat> {
    if x.num < 0 {
        return None;
    }
    let v = x.num.div_euclid(x.den);
    let (mut lo, mut hi) = (0i128, v.min(1 << 63));
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        if mid * mid <= v {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(Rat::int(lo))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d)
    }

    #[test]
    fn the_rule_of_int64() {
        assert!(fits(Rat::int(i64::MAX as i128)));
        assert!(!fits(Rat::int(i64::MAX as i128 + 1)));
        assert!(fits(Rat::int(i64::MIN as i128)));
        assert_eq!(add(Rat::int(i64::MAX as i128), Rat::int(1)), Err(Arith::Overflow));
        assert_eq!(div(Rat::int(1), Rat::zero()), Err(Arith::Zero));
        assert_eq!(neg(Rat::int(i64::MIN as i128)), Err(Arith::Overflow));
        assert_eq!(mul(r(1, 3), r(3, 1)), Ok(Rat::int(1)));
    }

    #[test]
    fn rounding_by_the_integer_procedure() {
        let one = Rat::int(1);
        assert_eq!(round(r(-42, 10), Rounding::Up, one), Ok(Rat::int(-5)));
        assert_eq!(round(r(-48, 10), Rounding::Down, one), Ok(Rat::int(-4)));
        assert_eq!(round(r(5, 10), Rounding::HalfUp, one), Ok(Rat::int(1)));
        assert_eq!(round(r(-5, 10), Rounding::HalfDown, one), Ok(Rat::int(0)));
        assert_eq!(round(r(15, 10), Rounding::HalfEven, one), Ok(Rat::int(2)));
        assert_eq!(round(r(25, 10), Rounding::HalfEven, one), Ok(Rat::int(2)));
        assert_eq!(round(Rat::int(701), Rounding::Up, Rat::int(10)), Ok(Rat::int(710)));
        assert_eq!(round(r(349, 10), Rounding::HalfUp, r(1, 100)), Ok(r(349, 10)));
    }

    #[test]
    fn the_builtins_of_rulec() {
        assert_eq!(allocate(Rat::int(1000), Rat::int(5000), Rat::int(15000)), Ok(Rat::int(333)));
        assert_eq!(allocate(Rat::int(1000), Rat::int(1), Rat::zero()), Err(Arith::Zero));
        assert_eq!(modulo(Rat::int(-1), 3), Rat::int(2));
        assert_eq!(isqrt(Rat::int(17)), Some(Rat::int(4)));
        assert_eq!(isqrt(Rat::int(-1)), None);
    }
}
