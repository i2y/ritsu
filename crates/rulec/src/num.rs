//! Exact rationals. §7.1 keeps runtime values on a single int64 with a static
//! rational scale; the checker needs the exact value to decide grid membership (E106)
//! and to evaluate examples, so it carries a full rational.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        assert!(den != 0, "零除算");
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
    /// Is this value a multiple of `grid`? E106 asks exactly this of an output literal.
    pub fn on_grid(self, grid: Rat) -> bool {
        if grid.num == 0 {
            return true;
        }
        self.div(grid).is_int()
    }
    pub fn cmp_to(self, o: Rat) -> std::cmp::Ordering {
        (self.num * o.den).cmp(&(o.num * self.den))
    }
}

impl std::fmt::Display for Rat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            // Decimal when the denominator allows it; the values a rule file holds always do.
            let mut den = self.den;
            let mut digits = 0;
            while den % 2 == 0 {
                den /= 2;
                digits += 1;
            }
            let mut d5 = den;
            let mut p5 = 0;
            while d5 % 5 == 0 {
                d5 /= 5;
                p5 += 1;
            }
            if d5 == 1 {
                let places = digits.max(p5);
                let scale = 10i128.pow(places as u32);
                let v = self.num * scale / self.den;
                let (int, frac) = (v / scale, (v % scale).abs());
                write!(f, "{int}.{frac:0width$}", width = places as usize)
            } else {
                write!(f, "{}/{}", self.num, self.den)
            }
        }
    }
}

/// §7.3 の四モード。負の向きまで仕様で固定されている（Python の `//` は −∞ 方向、
/// Go の整数除算は 0 方向で食い違うので、言語の素の除算には任せない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundMode {
    /// 0 から遠ざける（-4.2 → -5）
    Up,
    /// 0 へ寄せる（-4.8 → -4）
    Down,
    /// 半分ちょうどは 0 から遠ざける
    Half,
    /// 半分ちょうどは偶数へ
    Bankers,
}

impl RoundMode {
    pub fn parse(s: &str) -> Option<RoundMode> {
        Some(match s {
            "切り上げ" => RoundMode::Up,
            "切り捨て" => RoundMode::Down,
            "四捨五入" => RoundMode::Half,
            "銀行家丸め" => RoundMode::Bankers,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            RoundMode::Up => "切り上げ",
            RoundMode::Down => "切り捨て",
            RoundMode::Half => "四捨五入",
            RoundMode::Bankers => "銀行家丸め",
        }
    }
}

impl Rat {
    /// 0 方向へ切った整数部と、残りの絶対値（分子・分母）。
    fn split(self) -> (i128, i128, i128) {
        let q = self.num / self.den;
        let r = self.num - q * self.den;
        (q, r.abs(), self.den)
    }

    /// `grid` の倍数へ丸める。grid が 0 なら素通し。
    pub fn round_to(self, mode: RoundMode, grid: Rat) -> Rat {
        if grid.num == 0 {
            return self;
        }
        let q = self.div(grid);
        let (t, rn, rd) = q.split();
        if rn == 0 {
            return grid.mul(Rat::int(t));
        }
        let neg = q.num < 0;
        let away = if neg { t - 1 } else { t + 1 };
        let k = match mode {
            RoundMode::Down => t,
            RoundMode::Up => away,
            RoundMode::Half => {
                if 2 * rn >= rd { away } else { t }
            }
            RoundMode::Bankers => {
                if 2 * rn > rd {
                    away
                } else if 2 * rn < rd {
                    t
                } else if t % 2 == 0 {
                    t
                } else {
                    away
                }
            }
        };
        grid.mul(Rat::int(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d)
    }

    #[test]
    fn 負の向きが仕様どおり() {
        let one = Rat::int(1);
        // §7.3 の例をそのまま
        assert_eq!(r(-42, 10).round_to(RoundMode::Up, one), Rat::int(-5));
        assert_eq!(r(-48, 10).round_to(RoundMode::Down, one), Rat::int(-4));
        // 半分ちょうど
        assert_eq!(r(5, 10).round_to(RoundMode::Half, one), Rat::int(1));
        assert_eq!(r(-5, 10).round_to(RoundMode::Half, one), Rat::int(-1));
        assert_eq!(r(5, 10).round_to(RoundMode::Bankers, one), Rat::int(0));
        assert_eq!(r(15, 10).round_to(RoundMode::Bankers, one), Rat::int(2));
        assert_eq!(r(-15, 10).round_to(RoundMode::Bankers, one), Rat::int(-2));
    }

    #[test]
    fn 格子は1円とは限らない() {
        let ten = Rat::int(10);
        assert_eq!(Rat::int(701).round_to(RoundMode::Up, ten), Rat::int(710));
        assert_eq!(Rat::int(701).round_to(RoundMode::Down, ten), Rat::int(700));
        assert_eq!(Rat::int(705).round_to(RoundMode::Half, ten), Rat::int(710));
    }
}

#[cfg(test)]
mod readme_tests {
    use super::*;

    /// README の「範囲と丸め」に載せた例が、実装と一致すること。
    /// 手で書いた表は腐る（生成コードと描画の抜粋と同じ理屈）。
    #[test]
    fn readmeの丸めの例は実装と一致する() {
        let cases: &[(RoundMode, i128, i128, i128, i128)] = &[
            // (モード, 値の分子, 分母, 格子, 期待)
            (RoundMode::Up, -42, 10, 1, -5),
            (RoundMode::Down, -48, 10, 1, -4),
            (RoundMode::Half, -45, 10, 1, -5),
            (RoundMode::Bankers, 25, 10, 1, 2),
            (RoundMode::Bankers, 35, 10, 1, 4),
            // 括弧の中が格子。切り上げ(10円) なら −4.2 円 は −10 円。
            (RoundMode::Up, -42, 10, 10, -10),
        ];
        for (m, num, den, grid, want) in cases {
            let got = Rat { num: *num, den: *den }.round_to(*m, Rat::int(*grid));
            assert_eq!(
                got.num / got.den,
                *want,
                "{m:?} の {num}/{den} を格子 {grid} で丸めると {want} のはず"
            );
        }
    }
}
