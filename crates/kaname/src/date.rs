//! Dates (DESIGN 3.4): a day of the proleptic Gregorian calendar, with no time zone, from
//! 0001-01-01 to 9999-12-31, held as the number of days since 1970-01-01, and written `YYYY-MM-DD`.
//!
//! The day number and the year, month and day are turned into each other by Howard Hinnant's
//! `days_from_civil` and `civil_from_days`, the steps ritsu-ports' `day_text`, rulec and koyomi take
//! (`tests/dates.rs` holds kaname's to theirs on every day of the range). The difference of two dates
//! is the difference of their day numbers: `2026-04-01 - 2026-03-02` is 30. kaname adds no days to a
//! date: that is koyomi's (DESIGN 3.4).

/// The day number of 0001-01-01.
pub const MIN: i64 = -719_162;
/// The day number of 9999-12-31.
pub const MAX: i64 = 2_932_896;

/// Whether the day exists in the proleptic Gregorian calendar, from year 1.
pub fn is_date(y: i64, m: i64, d: i64) -> bool {
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    y >= 1 && (1..=days).contains(&d)
}

/// The day number of a date (days since 1970-01-01): Hinnant's `days_from_civil`.
pub fn day_of(y: i64, m: i64, d: i64) -> i64 {
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The year, month and day of a day number: Hinnant's `civil_from_days`.
pub fn civil(day: i64) -> (i64, i64, i64) {
    let z = day + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A day number as a date, `YYYY-MM-DD`.
pub fn text(day: i64) -> String {
    let (y, m, d) = civil(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A date written `YYYY-MM-DD` as its day number: None for anything else, and for a day that does
/// not exist or lies outside 0001-01-01 to 9999-12-31.
pub fn parse(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| -> Option<i64> {
        let part = &s[r];
        if part.bytes().all(|c| c.is_ascii_digit()) { part.parse().ok() } else { None }
    };
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    is_date(y, m, d).then(|| day_of(y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_dates() {
        assert_eq!(day_of(1970, 1, 1), 0);
        assert_eq!(day_of(1, 1, 1), MIN);
        assert_eq!(day_of(9999, 12, 31), MAX);
        assert_eq!(text(day_of(2026, 4, 1)), "2026-04-01");
        assert_eq!(parse("2026-04-01"), Some(day_of(2026, 4, 1)));
        assert_eq!(parse("2026-02-29"), None);
        assert_eq!(parse("0000-12-31"), None);
        assert_eq!(parse("2026-4-01"), None);
        assert_eq!(day_of(2026, 4, 1) - day_of(2026, 3, 2), 30);
    }
}
