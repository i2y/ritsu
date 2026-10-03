//! Dates and the operations that need no calendar (PLAN B.2, DESIGN 1.7, 2.1–2.3).

use koyomi::date::{self, Day, DateError, Missing, close_day, month_len, parse};

const WAYS: [Missing; 3] = [Missing::EndOfMonth, Missing::StartOfNextMonth, Missing::Reject];

fn d(s: &str) -> Day {
    parse(s).unwrap_or_else(|| panic!("{s} is a date"))
}

fn every_day(from: &str, to: &str) -> impl Iterator<Item = Day> {
    (d(from).0..=d(to).0).map(Day)
}

#[test]
fn every_day_of_the_range_goes_there_and_back() {
    let mut n = 0u64;
    for z in date::MIN.0..=date::MAX.0 {
        let (y, m, dd) = date::civil_from_days(z as i64);
        assert_eq!(date::days_from_civil(y, m, dd), z as i64, "{y}-{m}-{dd}");
        assert!((1..=12).contains(&m) && dd >= 1 && dd <= month_len(y, m));
        n += 1;
    }
    assert_eq!(n, 3_652_059);
}

#[test]
fn days_of_the_week_and_day_numbers() {
    // Monday is 0.
    assert_eq!(d("1970-01-01").weekday(), 3);
    assert_eq!(d("2000-02-29").weekday(), 1);
    assert_eq!(d("2026-10-02").weekday(), 4);
    assert_eq!(d("0001-01-01").weekday(), 0);
    assert_eq!(d("9999-12-31").weekday(), 4);
    assert_eq!(d("2026-01-01").0, 20454);
    assert_eq!(d("2027-11-20").0, 21142);
    assert_eq!(d("1955-01-01").0, -5479);
    assert_eq!(d("2027-12-31").0, 21183);
    assert_eq!(date::MIN, d("0001-01-01"));
    assert_eq!(date::MIN.0, -719_162);
    assert_eq!(date::MAX.0, 2_932_896);
}

/// Adding months the simple way: build the year, month and day, and see whether the month has it.
fn naive_add_months(z: Day, k: i64, p: Missing) -> Result<Day, DateError> {
    let (y, m, dd) = z.ymd();
    let total = y as i64 * 12 + (m as i64 - 1) + k;
    let (y2, m2) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
    if let Some(x) = Day::from_ymd(y2, m2, dd) {
        return Ok(x);
    }
    match p {
        Missing::EndOfMonth => {
            let mut e = dd;
            loop {
                e -= 1;
                if let Some(x) = Day::from_ymd(y2, m2, e) {
                    return Ok(x);
                }
            }
        }
        Missing::StartOfNextMonth => {
            let t = total + 1;
            Ok(Day::from_ymd(t.div_euclid(12), (t.rem_euclid(12) + 1) as u32, 1).unwrap())
        }
        _ => Err(DateError::Missing { y: y2 as i32, m: m2, d: dd }),
    }
}

#[test]
fn adding_months_agrees_with_the_simple_way() {
    let mut n = 0;
    for z in every_day("1900-01-01", "2100-12-31") {
        n += 1;
        for k in -24..=24 {
            for p in WAYS {
                assert_eq!(date::add_months(z, k, p), naive_add_months(z, k, p), "{z} {k:+} months under {p:?}");
            }
        }
    }
    assert_eq!(n, 73_414);
}

/// Catala's Theorems 3 and 4 (DESIGN 1.7): both ways of rounding are monotonic, rounding down
/// never passes rounding up, the three ways agree when no day is missing, and a year is
/// twelve months.
#[test]
fn the_properties_of_the_paper() {
    for k in -24i64..=24 {
        let mut prev: Option<(Day, Day)> = None;
        for z in every_day("1900-01-01", "2100-12-31") {
            let lo = date::add_months(z, k, Missing::EndOfMonth).unwrap();
            let hi = date::add_months(z, k, Missing::StartOfNextMonth).unwrap();
            assert!(lo <= hi, "{z} {k:+}: {lo} > {hi}");
            if let Ok(r) = date::add_months(z, k, Missing::Reject) {
                assert!(r == lo && r == hi, "{z} {k:+}: the three ways differ with no missing day");
            } else {
                assert!(lo < hi);
            }
            if let Some((plo, phi)) = prev {
                assert!(plo <= lo && phi <= hi, "{z} {k:+}: not monotonic");
            }
            prev = Some((lo, hi));
            if k % 12 == 0 {
                for p in WAYS {
                    let years = koyomi::resolve::ROp::Months { sign: 1, n: koyomi::resolve::A::Lit(k / 12), per: 12, missing: Some(p) };
                    let got = koyomi::interp::apply(None, z, &years, &[], &mut Vec::new()).map_err(|_| ());
                    assert_eq!(got, date::add_months(z, k, p).map_err(|_| ()), "{z} {} years", k / 12);
                }
            }
        }
    }
}

/// DESIGN 1.7's table: what `else end_of_month` gives agrees with dateutil, Java, PostgreSQL,
/// Temporal, chrono and macOS's `date`.
#[test]
fn the_libraries_table() {
    let eom = |s: &str, k| date::add_months(d(s), k, Missing::EndOfMonth).unwrap().to_string();
    assert_eq!(eom("2023-01-31", 1), "2023-02-28");
    assert_eq!(eom("2024-01-31", 1), "2024-02-29");
    assert_eq!(eom("2023-05-31", -1), "2023-04-30");
    let twice = date::add_months(date::add_months(d("2023-03-31"), 1, Missing::EndOfMonth).unwrap(), 1, Missing::EndOfMonth).unwrap();
    assert_eq!(twice.to_string(), "2023-05-30");
    assert_eq!(eom("2023-03-31", 2), "2023-05-31");
    assert_eq!(date::add_months(d("2023-01-31"), 1, Missing::StartOfNextMonth).unwrap().to_string(), "2023-03-01");
    assert_eq!(date::add_months(d("2023-01-31"), 1, Missing::Reject), Err(DateError::Missing { y: 2023, m: 2, d: 31 }));
}

/// DESIGN 2.3: what does not hold.
#[test]
fn what_does_not_hold() {
    let eom = |z: Day, k| date::add_months(z, k, Missing::EndOfMonth).unwrap();
    // Months do not add up: one and one is not two.
    assert_eq!(eom(eom(d("2023-03-31"), 1), 1).to_string(), "2023-05-30");
    assert_eq!(eom(d("2023-03-31"), 2).to_string(), "2023-05-31");
    // A day and a month do not commute.
    assert_eq!(eom(d("2023-01-30").plus(1).unwrap(), 1).to_string(), "2023-02-28");
    assert_eq!(eom(d("2023-01-30"), 1).plus(1).unwrap().to_string(), "2023-03-01");
    // A month there and back is not where it started.
    assert_eq!(eom(d("2023-03-31"), 1).to_string(), "2023-04-30");
    assert_eq!(eom(eom(d("2023-03-31"), 1), -1).to_string(), "2023-03-30");
    // Going back a month under start_of_next_month can stay in the month.
    assert_eq!(date::add_months(d("2023-03-31"), -1, Missing::StartOfNextMonth).unwrap().to_string(), "2023-03-01");
}

/// `close day N`, against the closing days of every month laid out in a row (PLAN B.2).
#[test]
fn closing_on_the_nth() {
    let first = d("1900-01-01");
    let last = d("2100-12-31");
    for n in 1u32..=31 {
        for p in WAYS {
            // Every month's closing day, from the month before the first to the month after the last.
            let mut months: Vec<(i64, u32)> = Vec::new();
            let mut ym = (1899i64, 12u32);
            while ym <= (2101, 1) {
                months.push(ym);
                ym = date::shift_month(ym.0, ym.1, 1);
            }
            let closing: Vec<Option<Day>> = months.iter().map(|(y, m)| date::place(*y, *m, n, p).ok()).collect();
            let existing: Vec<Day> = closing.iter().flatten().copied().collect();
            let mut prev: Option<Day> = None;
            for z in (first.0..=last.0).map(Day) {
                let got = close_day(z, n, p);
                let naive = existing.iter().find(|c| **c >= z).copied();
                let want = match p {
                    Missing::Reject => {
                        // No closing day in a month from z's to the one found: the period has no end.
                        let (zy, zm, _) = z.ymd();
                        let c = naive.unwrap();
                        let (cy, cm, _) = c.ymd();
                        let gap = months.iter().zip(&closing).any(|((y, m), cl)| (*y, *m) >= (zy as i64, zm) && (*y, *m) <= (cy as i64, cm) && cl.is_none());
                        if gap { None } else { Some(c) }
                    }
                    _ => naive,
                };
                assert_eq!(got.clone().ok(), want, "{z} close day {n} under {p:?}");
                if let Ok(c) = got {
                    assert!(c >= z);
                    if let Some(q) = prev {
                        assert!(q <= c, "close day {n} under {p:?} is not monotonic at {z}");
                    }
                    prev = Some(c);
                }
            }
        }
    }
}

#[test]
fn the_operations_at_the_ends_of_the_range() {
    assert_eq!(date::MAX.plus(1), Err(DateError::OutOfRange));
    assert_eq!(date::MIN.plus(-1), Err(DateError::OutOfRange));
    assert_eq!(date::add_months(d("9999-12-15"), 1, Missing::EndOfMonth), Err(DateError::OutOfRange));
    assert_eq!(date::end_of_month(d("9999-12-15"), 0).unwrap(), date::MAX);
    assert_eq!(date::close_day(d("9999-12-31"), 31, Missing::EndOfMonth).unwrap(), date::MAX);
    assert_eq!(date::close_day(d("0001-01-01"), 31, Missing::StartOfNextMonth).unwrap().to_string(), "0001-01-31");
    assert_eq!(date::parse_table_date("1955/1/1"), Some(d("1955-01-01")));
    assert_eq!(date::parse("2026-02-29"), None);
}
