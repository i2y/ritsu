//! Calendars (PLAN B.6, DESIGN 1.4, 1.8, 2.2, 2.4).

use koyomi::ast::Conv;
use koyomi::calendar::{Calendar, Outside, Reason};
use koyomi::check::{Checked, Options, check, check_text};
use koyomi::date::{Day, parse};

fn d(s: &str) -> Day {
    parse(s).unwrap()
}

fn load(path: &str) -> Calendar {
    let o = check(path).unwrap();
    assert!(!o.has_errors(), "{path}: {:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    match o.checked {
        Some(Checked::Calendar(c)) => *c,
        _ => panic!("{path} is a calendar"),
    }
}

#[test]
fn tokyo() {
    let c = load("examples/calendars/東京の営業日.cal");
    assert_eq!(c.tables[0].rows.len(), 1067);
    assert_eq!((c.data.0.to_string(), c.data.1.to_string()), ("1955-01-01".into(), "2027-12-31".into()));
    assert_eq!(c.open_days_in_year(2026), Ok(240));
    assert_eq!(c.longest_closed_run(d("2026-01-01"), d("2027-12-31")), Some((d("2026-12-29"), d("2027-01-03"))));
    assert_eq!(c.is_open(d("2026-05-04")), Ok(false));
    assert_eq!(c.reasons(d("2026-05-04")), vec![Reason::Holiday { table: "祝日".into(), name: "みどりの日".into() }]);
    assert_eq!(c.reasons(d("2026-05-06")), vec![Reason::Holiday { table: "祝日".into(), name: "休日".into() }]);
    assert_eq!(c.is_open(d("2026-05-08")), Ok(true));
    // A Sunday that is a holiday is closed for both.
    assert_eq!(c.reasons(d("2026-05-03")).len(), 2);
    assert_eq!(c.is_open(d("2028-01-10")), Err(Outside(d("2028-01-10"))));
    assert_eq!(c.offset_text().as_deref(), Some("+09:00"));
    assert_eq!(c.reasons(d("2026-12-31")), vec![Reason::Every { name: Some("年末年始".into()), text: "12-29..01-03".into() }]);
}

#[test]
fn business_days_on_a_calendar_without_a_table() {
    let c = load("tests/fixtures/calendars/土日.cal");
    assert_eq!((c.data.0, c.data.1), (koyomi::date::MIN, koyomi::date::MAX));
    // Counting starts the day after, open or closed (DESIGN 1.8, QuantLib 1.43's result).
    assert_eq!(c.add_business(d("2026-10-03"), 1, true).unwrap(), d("2026-10-05"));
    assert_eq!(c.add_business(d("2026-10-02"), 1, true).unwrap(), d("2026-10-05"));
    // Zero rolls to a business day in its direction.
    assert_eq!(c.add_business(d("2026-10-03"), 0, true).unwrap(), d("2026-10-05"));
    assert_eq!(c.add_business(d("2026-10-03"), 0, false).unwrap(), d("2026-10-02"));
    assert_eq!(c.add_business(d("2026-10-05"), 0, true).unwrap(), d("2026-10-05"));
    assert_eq!(c.add_business(d("2026-10-05"), 2, false).unwrap(), d("2026-10-01"));
    assert_eq!(c.add_business(d("2026-10-02"), 5, true).unwrap(), d("2026-10-09"));
}

#[test]
fn the_four_conventions() {
    let c = load("tests/fixtures/calendars/土日.cal");
    // 2026-01-31 is a Saturday; the next business day is in February.
    assert_eq!(c.roll(d("2026-01-31"), Conv::Following).unwrap(), d("2026-02-02"));
    assert_eq!(c.roll(d("2026-01-31"), Conv::ModifiedFollowing).unwrap(), d("2026-01-30"));
    assert_eq!(c.roll(d("2026-01-31"), Conv::Preceding).unwrap(), d("2026-01-30"));
    // 2026-08-01 is a Saturday; the business day before is in July.
    assert_eq!(c.roll(d("2026-08-01"), Conv::Preceding).unwrap(), d("2026-07-31"));
    assert_eq!(c.roll(d("2026-08-01"), Conv::ModifiedPreceding).unwrap(), d("2026-08-03"));
    // A business day stays.
    for conv in [Conv::Following, Conv::Preceding, Conv::ModifiedFollowing, Conv::ModifiedPreceding] {
        assert_eq!(c.roll(d("2026-10-02"), conv).unwrap(), d("2026-10-02"));
    }
    // Asking past the data range is an error, never a guess.
    let t = load("examples/calendars/東京の営業日.cal");
    assert_eq!(t.roll(d("2027-12-31"), Conv::Following), Err(koyomi::calendar::CalError::Outside(d("2028-01-01"))));
}

fn codes(path: &str, src: &str) -> Vec<&'static str> {
    check_text(path, src, &Options::default()).diags.iter().map(|d| d.code).collect()
}

#[test]
fn what_is_wrong_with_a_calendar() {
    let p = "tests/fixtures/calendars/t.cal";
    assert_eq!(codes(p, "calendar t v1\noffset Asia/Tokyo\n\nclosed weekly sat, sun\n"), vec!["E107"]);
    assert_eq!(codes(p, "calendar t v1\noffset UTC\n"), vec!["E107"]);
    assert_eq!(codes(p, "calendar t v1\noffset 9\n"), vec!["E107"]);
    assert_eq!(codes(p, "calendar t v1\noffset +25:00\n"), vec!["E006"]);
    assert_eq!(codes(p, "calendar t v1\n\nclosed weekly mon, tue, wed, thu, fri, sat, sun\n"), vec!["E108"]);
    assert_eq!(codes(p, "calendar t v1\n\nclosed every 01-01..12-31\n"), vec!["E108"]);
    assert_eq!(codes(p, "calendar t v1\n\nclosed weekly sat, sun\nopen 2026-12-28 \"臨時営業\"\n"), vec!["W101"]);
    assert_eq!(codes(p, "calendar t v1\n\nclosed weekly sat, sun\nopen 2026-12-26 \"臨時営業\"\n"), Vec::<&str>::new());
    assert_eq!(codes(p, "calendar t v1\n\nclosed 祝日\n"), vec!["E008"]);
}

#[test]
fn a_calendar_read_with_use_calendar() {
    let p = "tests/fixtures/calendars/t.cal";
    // The calendar it reads, and its own closures on top.
    let o = check_text(p, "calendar 夏休み(summer) v1\nuse calendar \"土日.cal\"\n\nclosed 2026-08-13..2026-08-15 \"夏季休業\"\nopen 2026-08-15 \"出勤日\"\n", &Options::default());
    assert!(!o.has_errors(), "{:?}", o.diags.iter().map(|d| d.render(koyomi::i18n::Lang::En)).collect::<Vec<_>>());
    let Some(Checked::Calendar(c)) = o.checked else { panic!() };
    assert_eq!(c.offset, Some(9 * 60));
    assert_eq!(c.used.len(), 1);
    assert_eq!(c.is_open(d("2026-08-13")), Ok(false));
    assert_eq!(c.is_open(d("2026-08-15")), Ok(true), "open wins: 2026-08-15 is a Saturday and closed for summer, and opened");
    assert_eq!(c.is_open(d("2026-08-16")), Ok(false));
    // E015: not there, not a calendar, round in a circle, an offset that differs.
    assert_eq!(codes(p, "calendar t v1\nuse calendar \"none.cal\"\n"), vec!["E015"]);
    assert_eq!(codes("examples/t.cal", "calendar t v1\nuse calendar \"net30.cal\"\n"), vec!["E015"]);
    assert_eq!(codes(p, "calendar t v1\noffset +08:00\nuse calendar \"土日.cal\"\n"), vec!["E015"]);
    let dir = std::env::temp_dir().join(format!("koyomi-cycle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.cal"), "calendar a v1\nuse calendar \"b.cal\"\n").unwrap();
    std::fs::write(dir.join("b.cal"), "calendar b v1\nuse calendar \"a.cal\"\n").unwrap();
    let a = dir.join("a.cal");
    let got = codes(a.to_str().unwrap(), &std::fs::read_to_string(&a).unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(got.contains(&"E015"), "{got:?}");
}

#[test]
fn england_and_wales() {
    let c = load("examples/calendars/england_and_wales.cal");
    assert_eq!((c.data.0.to_string(), c.data.1.to_string()), ("2019-01-01".into(), "2028-12-31".into()));
    assert_eq!(c.offset, None);
    assert_eq!(c.reasons(d("2026-12-28")), vec![Reason::Holiday { table: "bank_holidays".into(), name: "Boxing Day (Substitute day)".into() }]);
}
