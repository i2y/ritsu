//! kaname's dates (DESIGN 3.4, 16.3): the day number of every day from 0001-01-01 to 9999-12-31, and
//! the day each number is, are ritsu-ports' (`day_text`), koyomi's (`days_from_civil`,
//! `civil_from_days`) and rulec's (`date_ord`); and the difference of two dates counts the days as
//! koyomi counts them from a date's input to the date.

mod common;

use kaname::date;
use kaname::interp::Interp;
use kaname::value::{Program, Value};
use ritsu_ports::{Dates, Found};
use std::path::Path;

#[test]
fn every_day_is_the_same_day_in_every_language() {
    let mut compared = 0u64;
    for day in date::MIN..=date::MAX {
        let (y, m, d) = date::civil(day);
        assert_eq!(date::day_of(y, m, d), day, "{day}");
        assert_eq!(date::text(day), ritsu_ports::day_text(day), "{day}");
        let (ky, km, kd) = koyomi::date::civil_from_days(day);
        assert_eq!((y, m, d), (ky, km as i64, kd as i64), "{day}");
        assert_eq!(koyomi::date::days_from_civil(y, m as u32, d as u32), day, "{day}");
        assert_eq!(rulec::types::date_ord(y as i32, m as u32, d as u32), ritsu_units::Rat::int(day as i128), "{day}");
        assert_eq!(date::parse(&date::text(day)), Some(day));
        compared += 1;
    }
    eprintln!("dates: {compared} days compared with ritsu-ports, koyomi and rulec");
    assert_eq!(compared, 3_652_059);
    assert_eq!(date::parse("0000-12-31"), None);
    assert_eq!(date::parse("10000-01-01"), None);
}

/// `due - invoiced` for every day of the dates file's input is the days koyomi says lie between the
/// input and the date (`Dates::days`): 30.
#[test]
fn the_difference_of_two_dates_is_koyomis_count() {
    let c = common::check_text("t.kn", "module gap v1\n\nuse dates terms from \"examples/late_payment/dates/payment_terms.cal\"\n\nfn gap(on: date range >=2026-01-01 <=2027-12-31) -> number\n  terms.due(invoiced: on) - on\n").checked.clone().unwrap();
    let file = Path::new("examples/late_payment/dates/payment_terms.cal");
    let Found::Value((fewest, most)) = koyomi::ports::Engine.days(file, "due").unwrap() else { panic!("koyomi counts no days") };
    let p = Program::of(&c);
    let suite = common::joined();
    let (lo, hi) = (date::day_of(2026, 1, 1), date::day_of(2027, 12, 31));
    let mut compared = 0;
    for day in lo..=hi {
        let mut it = Interp::new(&p, &suite);
        let v = it.call(&c, 0, vec![Value::Date(day)], 0).unwrap();
        let Value::Num(n) = v else { panic!("{v:?}") };
        assert!(n.is_int() && (fewest as i128..=most as i128).contains(&n.num), "{}: {n} outside {fewest}..{most}", date::text(day));
        compared += 1;
    }
    eprintln!("date differences: {compared} days, each within koyomi's {fewest}..{most}");
    assert_eq!(compared, 730);
}
