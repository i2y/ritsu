//! Numbers cut into cells (DESIGN 4.1): a number the policies compare with constants is walked
//! as the cells those constants cut its range into, since within one cell every comparison has
//! one answer. A constant is first counted in the unit of what it is compared with
//! ([`crate::types::count`]; `50GBP` against `money[GBP, incl_tax]` is 50).

use crate::ast::Op;
use ritsu_base::text::Text;
use ritsu_units::{Dim, Rat, Unit};

/// Whether `a <op> b` holds.
pub fn holds<T: Ord>(op: Op, a: T, b: T) -> bool {
    match op {
        Op::Lt => a < b,
        Op::Le => a <= b,
        Op::Gt => a > b,
        Op::Ge => a >= b,
        Op::Is => a == b,
    }
}

/// A cell of a number's range: both ends in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cell {
    pub lo: i128,
    pub hi: i128,
}

impl Cell {
    pub fn contains(&self, v: i128) -> bool {
        self.lo <= v && v <= self.hi
    }
}

/// The cells `lo..=hi` is cut into by comparisons with constants (each already counted in the
/// value's unit), in order: within each cell, every comparison has one answer. A constant
/// outside the range cuts nothing (the comparison has one answer over the whole range).
pub fn cut(lo: i128, hi: i128, cmps: &[(Op, i128)]) -> Vec<Cell> {
    let mut starts: Vec<i128> = Vec::new();
    for &(op, c) in cmps {
        let at: &[i128] = match op {
            Op::Lt | Op::Ge => &[c],
            Op::Le | Op::Gt => &[c.saturating_add(1)],
            Op::Is => &[c, c.saturating_add(1)],
        };
        starts.extend(at.iter().copied().filter(|s| *s > lo && *s <= hi));
    }
    starts.sort_unstable();
    starts.dedup();
    let mut out = Vec::with_capacity(starts.len() + 1);
    let mut from = lo;
    for s in starts {
        out.push(Cell { lo: from, hi: s - 1 });
        from = s;
    }
    out.push(Cell { lo: from, hi });
    out
}

/// A number counted in `unit`, as a message shows it: `50GBP`, `10,000GBP`, `12` for a number.
pub fn show(v: i128, unit: &Unit) -> String {
    let n = if v < 0 { format!("-{}", ritsu_base::text::count(v.unsigned_abs() as u64)) } else { ritsu_base::text::count(v as u64) };
    match unit.dim {
        Dim::Number | Dim::Count(_) => n,
        Dim::Rate => match unit.step {
            Some(step) => {
                let r = Rat::int(v).mul(step).mul(Rat::int(100));
                format!("{}%", rat_text(r))
            }
            None => n,
        },
        _ => format!("{n}{}", unit.unit),
    }
}

/// A cell as a message shows it: one value, or both ends.
pub fn show_cell(c: Cell, unit: &Unit) -> Text {
    if c.lo == c.hi {
        Text::same(show(c.lo, unit))
    } else {
        let (a, b) = (show(c.lo, unit), show(c.hi, unit));
        Text { ja: format!("{a}〜{b}"), en: format!("{a} to {b}") }
    }
}

fn rat_text(r: Rat) -> String {
    if r.is_int() {
        return r.num.to_string();
    }
    // a step is a decimal fraction, so the value has a finite decimal expansion
    let mut out = format!("{}.", r.num / r.den);
    let mut rem = (r.num % r.den).abs();
    for _ in 0..18 {
        if rem == 0 {
            break;
        }
        rem *= 10;
        out.push_str(&(rem / r.den).to_string());
        rem %= r.den;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_cuts_where_its_answer_changes() {
        assert_eq!(cut(1, 10_000, &[(Op::Le, 50)]), vec![Cell { lo: 1, hi: 50 }, Cell { lo: 51, hi: 10_000 }]);
        assert_eq!(cut(1, 10_000, &[(Op::Lt, 50)]), vec![Cell { lo: 1, hi: 49 }, Cell { lo: 50, hi: 10_000 }]);
        assert_eq!(cut(1, 100, &[(Op::Is, 50)]), vec![Cell { lo: 1, hi: 49 }, Cell { lo: 50, hi: 50 }, Cell { lo: 51, hi: 100 }]);
        // outside the range, or at its ends, nothing to cut
        assert_eq!(cut(1, 100, &[(Op::Le, 100), (Op::Lt, 1), (Op::Gt, 500)]), vec![Cell { lo: 1, hi: 100 }]);
        // two constants, one cut twice
        assert_eq!(cut(0, 10, &[(Op::Ge, 5), (Op::Lt, 5), (Op::Le, 7)]), vec![Cell { lo: 0, hi: 4 }, Cell { lo: 5, hi: 7 }, Cell { lo: 8, hi: 10 }]);
    }

    #[test]
    fn a_number_is_shown_in_its_unit() {
        let gbp = Unit::parse("money[GBP, incl_tax]").unwrap();
        assert_eq!(show(10_000, &gbp), "10,000GBP");
        assert_eq!(show(125, &Unit::parse("rate[step 0.1%]").unwrap()), "12.5%");
        assert_eq!(show_cell(Cell { lo: 1, hi: 50 }, &gbp).en, "1GBP to 50GBP");
        assert_eq!(show_cell(Cell { lo: 1, hi: 50 }, &gbp).ja, "1GBP〜50GBP");
        assert_eq!(show(-3, &Unit::number()), "-3");
    }
}
