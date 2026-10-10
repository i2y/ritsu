//! Dates and periods (DESIGN 1.5, 5.5).
//!
//! A date is a day of the proleptic Gregorian calendar, with no time zone, from 0001-01-01 to
//! 9999-12-31 (koyomi's range), held as the number of days since 1970-01-01. Converting
//! between that number and a year, month and day is Howard Hinnant's `days_from_civil` and
//! `civil_from_days`, as in koyomi.

use std::fmt;

/// A date, as days since 1970-01-01.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub i32);

/// 0001-01-01.
pub const MIN: Day = Day(-719_162);
/// 9999-12-31.
pub const MAX: Day = Day(2_932_896);

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

impl Day {
    /// The day, when the year, month and day name one inside 0001-01-01..9999-12-31.
    pub fn from_ymd(y: i64, m: u32, d: u32) -> Option<Day> {
        if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > month_len(y, m) {
            return None;
        }
        Some(Day(days_from_civil(y, m, d) as i32))
    }

    /// `2026-10-03`, or None for anything else (the shape, or a day the calendar lacks).
    pub fn parse(s: &str) -> Option<Day> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let digits = |r: std::ops::Range<usize>| -> Option<u32> {
            if b[r.clone()].iter().all(|c| c.is_ascii_digit()) { s[r].parse().ok() } else { None }
        };
        Day::from_ymd(digits(0..4)? as i64, digits(5..7)?, digits(8..10)?)
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        let (y, m, d) = civil_from_days(self.0 as i64);
        (y as i32, m, d)
    }

    /// The day after, or None after 9999-12-31.
    pub fn next(self) -> Option<Day> {
        if self >= MAX { None } else { Some(Day(self.0 + 1)) }
    }

    /// The day before, or None before 0001-01-01.
    pub fn prev(self) -> Option<Day> {
        if self <= MIN { None } else { Some(Day(self.0 - 1)) }
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

/// `in force <from>..<to>`: both ends included, either open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    pub from: Option<Day>,
    pub to: Option<Day>,
}

impl Period {
    /// The first day, an open start standing for 0001-01-01.
    pub fn start(&self) -> Day {
        self.from.unwrap_or(MIN)
    }

    /// The last day, an open end standing for 9999-12-31.
    pub fn end(&self) -> Day {
        self.to.unwrap_or(MAX)
    }
}

impl fmt::Display for Period {
    /// As the `.req` writes it: `2026-10-01..`, `..2027-03-31`, `2026-10-01..2027-03-31`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let a = self.from.map(|d| d.to_string()).unwrap_or_default();
        let b = self.to.map(|d| d.to_string()).unwrap_or_default();
        write!(f, "{a}..{b}")
    }
}

/// The days from `a` to `b`, both included, as a person reads them: `2027-04-01` for one day,
/// `2027-04-01..2027-04-03` for more.
pub fn days(a: Day, b: Day) -> String {
    if a == b { a.to_string() } else { format!("{a}..{b}") }
}

/// Today in the machine's time zone, for `review` when `--date` is not given (DESIGN 4.2):
/// the one place yuen reads the clock.
pub fn today() -> Day {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let offset = local_offset(secs);
    Day((secs + offset).div_euclid(86_400) as i32)
}

/// The machine's offset from UTC at `secs`, in seconds, from the C library's `localtime_r`.
#[cfg(unix)]
fn local_offset(secs: i64) -> i64 {
    // `struct tm` on the 64-bit Unixes yuen is built for (macOS, Linux with glibc or musl):
    // nine ints, then `tm_gmtoff` (a long) and `tm_zone` (a pointer).
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: i64,
        zone: *const u8,
    }
    unsafe extern "C" {
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null() };
    let r = unsafe { localtime_r(&secs, &mut tm) };
    if r.is_null() { 0 } else { tm.gmtoff }
}

/// Built for WASI (ritsu's npm package, ritsu's DESIGN 8.8), whose C library knows no time zone:
/// the offset the loader read from the clock of the machine it runs on, which it hands over as
/// `RITSU_WASI_UTC_OFFSET` (seconds east of UTC, now); without it, UTC.
#[cfg(target_os = "wasi")]
fn local_offset(_secs: i64) -> i64 {
    std::env::var("RITSU_WASI_UTC_OFFSET").ok().and_then(|v| v.trim().parse::<i64>().ok()).filter(|o| o.abs() <= 18 * 3600).unwrap_or(0)
}

#[cfg(not(any(unix, target_os = "wasi")))]
fn local_offset(_secs: i64) -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_and_the_ends_hold() {
        assert_eq!(Day::from_ymd(1, 1, 1), Some(MIN));
        assert_eq!(Day::from_ymd(9999, 12, 31), Some(MAX));
        assert_eq!(Day::parse("2026-10-03").unwrap().to_string(), "2026-10-03");
        assert!(Day::parse("2026-02-30").is_none());
        assert!(Day::parse("2026-2-3").is_none());
        assert_eq!(Day::parse("2024-02-28").unwrap().next(), Day::parse("2024-02-29"));
        assert_eq!(MAX.next(), None);
        assert_eq!(MIN.prev(), None);
        let p = Period { from: Day::parse("2026-10-01"), to: None };
        assert_eq!(p.to_string(), "2026-10-01..");
    }
}
