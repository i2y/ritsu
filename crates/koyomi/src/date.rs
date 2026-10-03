//! Dates and the operations on them that need no calendar (DESIGN 2.1, 2.2).
//!
//! A date is a day of the proleptic Gregorian calendar, with no time zone, from 0001-01-01 to
//! 9999-12-31, held as the number of days since 1970-01-01. Converting between that number
//! and a year, month and day is Howard Hinnant's `days_from_civil` and `civil_from_days`.
//!
//! Three operations can land on a day the month does not have (February 30th): adding
//! months, the N-th of a month k months away, and closing on the N-th. What then happens is
//! written in the `.cal` with `else` (DESIGN 1.7), and arrives here as a [`Missing`].

use std::fmt;

/// A date, as days since 1970-01-01.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub i32);

/// 0001-01-01.
pub const MIN: Day = Day(-719_162);
/// 9999-12-31.
pub const MAX: Day = Day(2_932_896);

/// What an operation does when it lands on a day the month does not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// The last day of that month (Catala's rounding down).
    EndOfMonth,
    /// The first day of the month after it (Catala's rounding up).
    StartOfNextMonth,
    /// Refuse: the check finds every input that gets here (E202).
    Reject,
    /// The operation cannot land on a missing day, as the check worked out from what was
    /// written. Landing there anyway is a bug in koyomi.
    Never,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DateError {
    /// The day the operation landed on, which its month does not have, under `else reject`.
    Missing { y: i32, m: u32, d: u32 },
    /// The result falls outside 0001-01-01..9999-12-31.
    OutOfRange,
    /// A missing day under [`Missing::Never`]: the check said it could not happen.
    Bug(String),
}

/// Days from 1970-01-01 to the given date (Hinnant's `days_from_civil`).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The year, month and day of a day number (Hinnant's `civil_from_days`).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// The number of days in a month.
pub fn month_len(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

/// The month `k` months after `(y, m)`; `k` may be negative.
pub fn shift_month(y: i64, m: u32, k: i64) -> (i64, u32) {
    let total = y * 12 + (m as i64 - 1) + k;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

impl Day {
    /// The day, when the year, month and day name one inside 0001-01-01..9999-12-31.
    pub fn from_ymd(y: i64, m: u32, d: u32) -> Option<Day> {
        if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > month_len(y, m) {
            return None;
        }
        Some(Day(days_from_civil(y, m, d) as i32))
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        let (y, m, d) = civil_from_days(self.0 as i64);
        (y as i32, m, d)
    }

    pub fn year(self) -> i32 {
        self.ymd().0
    }

    /// Monday is 0, Sunday 6. 1970-01-01 was a Thursday.
    pub fn weekday(self) -> u32 {
        (self.0 as i64 + 3).rem_euclid(7) as u32
    }

    /// `n` days later (earlier when negative), inside the range dates have.
    pub fn plus(self, n: i64) -> Result<Day, DateError> {
        let z = self.0 as i64 + n;
        if z < MIN.0 as i64 || z > MAX.0 as i64 {
            return Err(DateError::OutOfRange);
        }
        Ok(Day(z as i32))
    }

    /// The day after, or None after 9999-12-31.
    pub fn next(self) -> Option<Day> {
        self.plus(1).ok()
    }

    pub fn prev(self) -> Option<Day> {
        self.plus(-1).ok()
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

/// `2026-04-01`: four digits, two, two, and a day that exists.
pub fn parse(s: &str) -> Option<Day> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| -> Option<u32> {
        let t = &s[r];
        if t.bytes().all(|c| c.is_ascii_digit()) { t.parse().ok() } else { None }
    };
    Day::from_ymd(digits(0..4)? as i64, digits(5..7)?, digits(8..10)?)
}

/// `1955/1/1` as the Cabinet Office's table writes it (and `2026-01-01` as well).
pub fn parse_table_date(s: &str) -> Option<Day> {
    if let Some(d) = parse(s) {
        return Some(d);
    }
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 || parts[0].len() != 4 {
        return None;
    }
    let num = |t: &str| -> Option<u32> {
        if !t.is_empty() && t.len() <= 2 && t.bytes().all(|c| c.is_ascii_digit()) { t.parse().ok() } else { None }
    };
    let y: u32 = if parts[0].bytes().all(|c| c.is_ascii_digit()) { parts[0].parse().ok()? } else { return None };
    Day::from_ymd(y as i64, num(parts[1])?, num(parts[2])?)
}

/// The day a month has on day `d`, or what `p` says to do when it has none.
pub fn place(y: i64, m: u32, d: u32, p: Missing) -> Result<Day, DateError> {
    if !(1..=9999).contains(&y) {
        return Err(DateError::OutOfRange);
    }
    let len = month_len(y, m);
    if d <= len {
        return Ok(Day(days_from_civil(y, m, d) as i32));
    }
    match p {
        Missing::EndOfMonth => Ok(Day(days_from_civil(y, m, len) as i32)),
        Missing::StartOfNextMonth => {
            let (ny, nm) = shift_month(y, m, 1);
            if !(1..=9999).contains(&ny) {
                return Err(DateError::OutOfRange);
            }
            Ok(Day(days_from_civil(ny, nm, 1) as i32))
        }
        Missing::Reject => Err(DateError::Missing { y: y as i32, m, d }),
        Missing::Never => Err(DateError::Bug(format!("{y:04}-{m:02}-{d:02} does not exist, and the check said it could not be reached"))),
    }
}

/// `+ n days` (`n` negative for `- n days`).
pub fn add_days(z: Day, n: i64) -> Result<Day, DateError> {
    z.plus(n)
}

/// `+ n months else p` (`n` negative for `- n months`): the same day `n` months away.
pub fn add_months(z: Day, n: i64, p: Missing) -> Result<Day, DateError> {
    let (y, m, d) = civil_from_days(z.0 as i64);
    let (y2, m2) = shift_month(y, m, n);
    place(y2, m2, d, p)
}

/// `day n of month +k else p`: the `n`-th of the month `k` months away.
pub fn day_of_month(z: Day, n: u32, k: i64, p: Missing) -> Result<Day, DateError> {
    let (y, m, _) = civil_from_days(z.0 as i64);
    let (y2, m2) = shift_month(y, m, k);
    place(y2, m2, n, p)
}

/// `start of month +k`.
pub fn start_of_month(z: Day, k: i64) -> Result<Day, DateError> {
    let (y, m, _) = civil_from_days(z.0 as i64);
    let (y2, m2) = shift_month(y, m, k);
    place(y2, m2, 1, Missing::Never)
}

/// `end of month +k`.
pub fn end_of_month(z: Day, k: i64) -> Result<Day, DateError> {
    let (y, m, _) = civil_from_days(z.0 as i64);
    let (y2, m2) = shift_month(y, m, k);
    if !(1..=9999).contains(&y2) {
        return Err(DateError::OutOfRange);
    }
    place(y2, m2, month_len(y2, m2), Missing::Never)
}

/// `close day n else p`: of the closing days `place(y', m', n, p)` of the months, the
/// earliest on or after `z`. Three months are enough. The next month's closing day is
/// inside that month or on the first of the one after, so always after `z`; the previous
/// month's matters only under `start_of_next_month`, which can move it to the first of
/// `z`'s month. Under `reject`, a month on the way with no `n`-th day has no closing day,
/// so the period `z` falls in has no end: that is the refusal.
pub fn close_day(z: Day, n: u32, p: Missing) -> Result<Day, DateError> {
    let (y, m, _) = civil_from_days(z.0 as i64);
    let first = if p == Missing::StartOfNextMonth { -1 } else { 0 };
    for k in first..=1 {
        let (y2, m2) = shift_month(y, m, k);
        if !(1..=9999).contains(&y2) {
            if k < 0 {
                // the month before 0001-01: its closing day is in December, before `z`
                continue;
            }
            return Err(DateError::OutOfRange);
        }
        let c = place(y2, m2, n, p)?;
        if c >= z {
            return Ok(c);
        }
    }
    // The next month's closing day is after `z` (see above); not reached.
    Err(DateError::Bug(format!("no closing day on the {n}th found after {z}")))
}

/// `close end of month`: the last day of `z`'s month.
pub fn close_end_of_month(z: Day) -> Day {
    let (y, m, _) = civil_from_days(z.0 as i64);
    Day(days_from_civil(y, m, month_len(y, m)) as i32)
}

/// The first day of the period whose closing day is `c` under `close day n else p`: the day
/// after the previous closing day. Only `eval` uses it, to say which period a day closed.
pub fn period_start(c: Day, n: u32, p: Missing) -> Day {
    // Every day of the period closes on `c`, and closing is monotonic: walk back while the
    // day before still closes on `c`. A period is at most two months long.
    let mut d = c;
    for _ in 0..70 {
        match d.prev().map(|b| (b, close_day(b, n, p))) {
            Some((b, Ok(x))) if x == c => d = b,
            _ => break,
        }
    }
    d
}

/// Mon … Sun.
pub const WEEKDAY_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub const WEEKDAY_EN_LONG: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
pub const WEEKDAY_JA: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ends_of_the_range() {
        assert_eq!(Day::from_ymd(1, 1, 1), Some(MIN));
        assert_eq!(Day::from_ymd(9999, 12, 31), Some(MAX));
        assert_eq!(parse("2026-02-29"), None);
        assert_eq!(parse("2024-02-29").map(|d| d.to_string()), Some("2024-02-29".into()));
        assert_eq!(parse_table_date("1955/1/1"), parse("1955-01-01"));
    }
}
