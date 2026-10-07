//! Exact rationals. Every value a unit converts is exact: `0.5%` is the rational 1/200, a pound
//! is 45359237/100000 grams on the nose, and a conversion that does not land on a whole number
//! says so instead of rounding. rulec's checker keeps runtime values on one int64 at a static
//! rational scale, and needs the exact value to decide grid membership and to evaluate examples;
//! this is that value (moved from rulec's `src/num.rs`, DESIGN 5.1).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rat {
    pub num: i128,
    pub den: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    if a == 0 { 1 } else { a }
}

impl Rat {
    pub fn new(num: i128, den: i128) -> Self {
        assert!(den != 0, "division by zero");
        let s = if den < 0 { -1 } else { 1 };
        let g = gcd(num, den);
        Rat { num: s * num / g, den: s * den / g }
    }
    pub fn int(n: i128) -> Self {
        Rat { num: n, den: 1 }
    }
    pub fn zero() -> Self {
        Rat::int(0)
    }
    pub fn add(self, o: Rat) -> Rat {
        Rat::new(self.num * o.den + o.num * self.den, self.den * o.den)
    }
    pub fn sub(self, o: Rat) -> Rat {
        Rat::new(self.num * o.den - o.num * self.den, self.den * o.den)
    }
    pub fn mul(self, o: Rat) -> Rat {
        Rat::new(self.num * o.num, self.den * o.den)
    }
    pub fn div(self, o: Rat) -> Rat {
        Rat::new(self.num * o.den, self.den * o.num)
    }
    pub fn is_int(self) -> bool {
        self.den == 1
    }
    /// Is this value a multiple of `grid`? rulec's E106 asks exactly this of an output literal.
    pub fn on_grid(self, grid: Rat) -> bool {
        if grid.num == 0 {
            return true;
        }
        self.div(grid).is_int()
    }
    pub fn cmp_to(self, o: Rat) -> std::cmp::Ordering {
        (self.num * o.den).cmp(&(o.num * self.den))
    }

    /// The same arithmetic, `None` where the exact result does not fit in 128 bits.
    ///
    /// rulec's elimination (its §15.126) multiplies coefficients together step after step, and
    /// the plain operators above wrap silently in a release build. A wrapped product is a wrong
    /// answer that looks like a right one — the one thing a proof must never hand back — so the
    /// elimination uses these and gives up where they refuse.
    pub fn checked_new(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let (mut a, mut b) = (num.checked_abs()?, den.checked_abs()?);
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        let g = if a == 0 { 1 } else { a };
        let s = if den < 0 { -1 } else { 1 };
        Some(Rat { num: (num / g).checked_mul(s)?, den: (den / g).checked_mul(s)? })
    }
    pub fn checked_add(self, o: Rat) -> Option<Rat> {
        let a = self.num.checked_mul(o.den)?;
        let b = o.num.checked_mul(self.den)?;
        Rat::checked_new(a.checked_add(b)?, self.den.checked_mul(o.den)?)
    }
    pub fn checked_sub(self, o: Rat) -> Option<Rat> {
        self.checked_add(Rat { num: o.num.checked_neg()?, den: o.den })
    }
    pub fn checked_mul(self, o: Rat) -> Option<Rat> {
        Rat::checked_new(self.num.checked_mul(o.num)?, self.den.checked_mul(o.den)?)
    }
    pub fn checked_div(self, o: Rat) -> Option<Rat> {
        Rat::checked_new(self.num.checked_mul(o.den)?, self.den.checked_mul(o.num)?)
    }
    /// The comparison, `None` where the cross products do not fit.
    pub fn checked_cmp(self, o: Rat) -> Option<std::cmp::Ordering> {
        Some(self.num.checked_mul(o.den)?.cmp(&o.num.checked_mul(self.den)?))
    }
}

impl std::fmt::Display for Rat {
    /// The value written out: a whole number as it is, a decimal where the denominator is made
    /// of 2s and 5s (the values a rule file holds always are), and a fraction otherwise (`1/3`).
    ///
    /// The sign is written first and the magnitude after it. The whole part of a value between
    /// −1 and 0 is 0, and taking the sign from it dropped the sign: −1/2 came out `0.5` (2026-10-08).
    /// The arithmetic is checked, and a decimal too long for 128 bits is written as the fraction.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.den == 1 {
            return write!(f, "{}", self.num);
        }
        let sign = if self.num < 0 { "-" } else { "" };
        let (num, den) = (self.num.unsigned_abs(), self.den.unsigned_abs());
        let (mut rest, mut twos, mut fives) = (den, 0u32, 0u32);
        while rest % 2 == 0 {
            rest /= 2;
            twos += 1;
        }
        while rest % 5 == 0 {
            rest /= 5;
            fives += 1;
        }
        let places = twos.max(fives);
        let decimal = if rest == 1 {
            10u128.checked_pow(places).and_then(|scale| Some((num.checked_mul(scale)? / den, scale)))
        } else {
            None
        };
        match decimal {
            Some((v, scale)) => write!(f, "{sign}{}.{:0width$}", v / scale, v % scale, width = places as usize),
            None => write!(f, "{sign}{num}/{den}"),
        }
    }
}
