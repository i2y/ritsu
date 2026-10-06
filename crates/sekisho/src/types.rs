//! Units, constants and days (DESIGN 2.4, 5.4): a number's type is rulec's unit, read with
//! ritsu-units, and a constant is counted in the unit of what it is compared with, as the integer
//! that goes on the wire and into Cedar (ritsu's DESIGN 5.3, the way dandori counts the ends of a
//! range): `1kg` is 1000 in `mass[g]`, `5%` is 50 steps of `rate[step 0.1%]`, `100万円` is 1000000
//! in `money[JPY, incl_tax]`. A constant that does not come to a whole number, or that is past
//! what TypeScript's `number` holds exactly, cannot be written (E103).

use crate::ast::Num;
use ritsu_ports::Day;
use ritsu_units::{Dim, Problem, Rat, Unit};

/// The largest integer a value may be: TypeScript's `number` holds every integer up to 2⁵³ − 1
/// exactly, and the generated code passes a value to Cedar as a JSON number (DESIGN 5.4).
pub const MAX_SAFE: i128 = (1 << 53) - 1;

/// The unit of a number's type as rulec writes it (`money[GBP, incl_tax]`, `mass[kg]`,
/// `rate[step 0.1%]`, `rate`, `number`), or why it is not one.
pub fn unit(written: &str) -> Result<Unit, Problem> {
    Unit::parse(written)
}

/// The code a type that is not a unit is said with: E102 for a unit of another dimension
/// (`mass[cm]`), E101 for what is not a unit at all.
pub fn unit_code(p: &Problem) -> &'static str {
    match p {
        Problem::OtherDimension { .. } => "E102",
        _ => "E101",
    }
}

/// Why a constant is not a value of a unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Miss {
    /// The type has a unit and the constant has none (`50` for `money[GBP, incl_tax]`).
    NoUnit,
    /// The constant's unit is not one the table has (`50XYZ`).
    UnknownUnit,
    /// A unit of another dimension or currency (`50USD` for `money[GBP]`, `3kg` for `number`).
    OtherUnit,
    /// Counted in the type's unit, not a whole number (`0.5GBP` for `money[GBP]`, `1lb` for `mass[g]`).
    NotWhole(Rat),
    /// Past ±(2⁵³ − 1) once counted in the type's unit.
    TooLarge(Rat),
}

impl Miss {
    /// The code it is said with: E101 for an unknown unit, E102 for the wrong unit, E103 for a
    /// constant that is no integer of the type.
    pub fn code(&self) -> &'static str {
        match self {
            Miss::UnknownUnit => "E101",
            Miss::NoUnit | Miss::OtherUnit => "E102",
            Miss::NotWhole(_) | Miss::TooLarge(_) => "E103",
        }
    }
}

/// The constant counted in `to`, as the integer that goes on the wire.
pub fn count(n: &Num, to: &Unit) -> Result<i128, Miss> {
    if !n.unit.is_empty() && n.unit != "%" && ritsu_units::table::unit(&n.unit).is_none() {
        return Err(Miss::UnknownUnit);
    }
    let counted = match &to.dim {
        Dim::Number if n.unit.is_empty() => Some(n.value),
        Dim::Number | Dim::Count(_) => None,
        _ if n.unit.is_empty() => return Err(Miss::NoUnit),
        // a rate's constant in percent, counted in the type's steps (a whole rate with no step)
        Dim::Rate if n.unit == "%" => n.value.checked_div(Rat::int(100)).and_then(|base| base.checked_div(to.step.unwrap_or(Rat::int(1)))),
        Dim::Rate => None,
        Dim::Money(_) => Unit::money(&n.unit, to.tax).and_then(|from| from.convert(n.value, to)),
        d => Unit::quantity(d.word(), &n.unit).and_then(|from| from.convert(n.value, to)),
    };
    let Some(c) = counted else { return Err(Miss::OtherUnit) };
    if !c.is_int() {
        return Err(Miss::NotWhole(c));
    }
    if c.num.abs() > MAX_SAFE {
        return Err(Miss::TooLarge(c));
    }
    Ok(c.num)
}

/// The day of `y-m-d`, as days since 1970-01-01; None for a date the calendar does not have
/// (`2026-02-30`) and one outside 0001-01-01..9999-12-31.
pub fn day(y: i64, m: u32, d: u32) -> Option<Day> {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > month_len(y, m) {
        return None;
    }
    // Howard Hinnant's days_from_civil
    let (y, m, d) = (if m <= 2 { y - 1 } else { y }, m as i64, d as i64);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// The days of a month.
pub fn month_len(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// A day as `YYYY-MM-DD`.
pub fn day_text(d: Day) -> String {
    ritsu_ports::day_text(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: Rat, unit: &str) -> Num {
        Num { value: v, unit: unit.into(), raw: String::new() }
    }

    #[test]
    fn a_constant_is_counted_in_the_unit_of_its_type() {
        let gbp = unit("money[GBP, incl_tax]").unwrap();
        assert_eq!(count(&num(Rat::int(10_000), "GBP"), &gbp), Ok(10_000));
        assert_eq!(count(&num(Rat::int(50), "USD"), &gbp), Err(Miss::OtherUnit));
        assert_eq!(count(&num(Rat::int(50), ""), &gbp), Err(Miss::NoUnit));
        assert_eq!(count(&num(Rat::new(1, 2), "GBP"), &gbp), Err(Miss::NotWhole(Rat::new(1, 2))));
        assert_eq!(count(&num(Rat::int(50), "XYZ"), &gbp), Err(Miss::UnknownUnit));
        let grams = unit("mass[g]").unwrap();
        assert_eq!(count(&num(Rat::new(3, 2), "kg"), &grams), Ok(1500));
        let rate = unit("rate[step 0.1%]").unwrap();
        assert_eq!(count(&num(Rat::int(5), "%"), &rate), Ok(50));
        assert_eq!(count(&num(Rat::int(3), ""), &unit("number").unwrap()), Ok(3));
        let yen = unit("money[JPY, incl_tax]").unwrap();
        assert_eq!(count(&num(Rat::int(1_000_000), "円"), &yen), Ok(1_000_000));
        assert_eq!(count(&num(Rat::int(1 << 53), ""), &unit("number").unwrap()), Err(Miss::TooLarge(Rat::int(1 << 53))));
    }

    #[test]
    fn days_are_counted_from_1970() {
        assert_eq!(day(1970, 1, 1), Some(0));
        assert_eq!(day(2026, 10, 1).map(day_text).as_deref(), Some("2026-10-01"));
        assert_eq!(day(2028, 2, 29).map(day_text).as_deref(), Some("2028-02-29"));
        assert_eq!(day(2026, 2, 29), None);
        assert_eq!(day(2026, 13, 1), None);
    }
}
