//! `koyomi vectors` (PLAN C.7, DESIGN 6.3): one JSON object a line for every input of the
//! range, in the order the check walks them, and the inputs just outside the range.

use std::process::{Command, Output};

fn koyomi(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap()
}

fn lines(path: &str) -> Vec<String> {
    let o = koyomi(&["vectors", path]);
    assert_eq!(o.status.code(), Some(0), "{path}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().lines().map(|l| l.to_string()).collect()
}

#[test]
fn the_number_of_lines_of_every_example() {
    // PLAN C.8: the range, and the inputs just outside it.
    for (path, n) in [
        ("examples/支払_20日締め翌月10日払い.cal", 691),
        ("examples/民法の期間.cal", 4_384),
        ("examples/締め日と支払日を受け取る.cal", 871_604),
        ("examples/net30.cal", 1_066),
        ("examples/calendars/東京の営業日.cal", 26_665),
        ("examples/calendars/民法142条の休日.cal", 26_665),
        ("examples/calendars/england_and_wales.cal", 3_655),
        ("examples/payment_20th_close_next_10th.cal", 691),
    ] {
        assert_eq!(lines(path).len(), n, "{path}");
    }
}

#[test]
fn the_english_twins_say_what_the_japanese_examples_say() {
    // PLAN D.2: the same terms with English names give the same dates on every input, line for
    // line, once the names are read across.
    let ja = lines("examples/支払_20日締め翌月10日払い.cal");
    let en = lines("examples/payment_20th_close_next_10th.cal");
    assert_eq!(ja.len(), en.len());
    for (a, b) in ja.iter().zip(&en) {
        let a = a.replace("\"受領日\"", "\"received\"").replace("\"締め日\"", "\"closing\"").replace("\"支払日\"", "\"payment\"").replace("\"支払日.at\"", "\"payment.at\"");
        assert_eq!(&a, b);
    }
    // The twins that break their claim on purpose break it on the same days.
    let fails = |p: &str| -> Vec<serde_json::Value> {
        let o = koyomi(&["check", p, "--format", "json"]);
        assert_eq!(o.status.code(), Some(1), "{p}");
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["diagnostics"].as_array().unwrap().len(), 1, "{p}");
        assert_eq!(v["diagnostics"][0]["code"], "E301");
        assert_eq!(v["diagnostics"][0]["line"], 16);
        v["diagnostics"][0]["fails"].as_array().unwrap().clone()
    };
    let (a, b) = (fails("examples/支払_月末締め翌々月末払い.cal"), fails("examples/eom_close_two_months_later.cal"));
    assert_eq!(a.len(), 10);
    assert_eq!(a, b);
}

#[test]
fn what_a_line_says() {
    let ls = lines("examples/支払_20日締め翌月10日払い.cal");
    assert_eq!(ls[0], r#"{"in":{"受領日":"2026-01-01"},"out":{"締め日":"2026-01-20","支払日":"2026-02-10","支払日.at":"2026-02-10T00:00:00Z"}}"#);
    // The line DESIGN 6.3 shows.
    assert!(ls.contains(&r#"{"in":{"受領日":"2026-04-01"},"out":{"締め日":"2026-04-20","支払日":"2026-05-08","支払日.at":"2026-05-08T00:00:00Z"}}"#.to_string()));
    // The day before the range and the day after it, refused.
    assert_eq!(ls[689], r#"{"in":{"受領日":"2025-12-31"},"error":"range"}"#);
    assert_eq!(ls[690], r#"{"in":{"受領日":"2027-11-21"},"error":"range"}"#);
    for l in &ls {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert!(v["in"].is_object() && (v["out"].is_object() != v["error"].is_string()), "{l}");
    }
}

#[test]
fn the_order_is_the_check_s() {
    // The integer inputs outside, the date inside: 月数 1 for the 365 days of 2026, then 2.
    let ls = lines("examples/民法の期間.cal");
    assert!(ls[0].starts_with(r#"{"in":{"起点":"2026-01-01","月数":1}"#), "{}", ls[0]);
    assert!(ls[364].starts_with(r#"{"in":{"起点":"2026-12-31","月数":1}"#), "{}", ls[364]);
    assert!(ls[365].starts_with(r#"{"in":{"起点":"2026-01-01","月数":2}"#), "{}", ls[365]);
    // The example of DESIGN 1.8: 起点 2026-01-22, 月数 1.
    assert_eq!(ls[21], r#"{"in":{"起点":"2026-01-22","月数":1},"out":{"起算日":"2026-01-23","満了日":"2026-02-22","満了日_142条":"2026-02-23"}}"#);
    // Outside: the date first, the integers at their least; then each integer, the date first.
    assert_eq!(
        ls[4380..].to_vec(),
        [
            r#"{"in":{"起点":"2025-12-31","月数":1},"error":"range"}"#,
            r#"{"in":{"起点":"2027-01-01","月数":1},"error":"range"}"#,
            r#"{"in":{"起点":"2026-01-01","月数":0},"error":"range"}"#,
            r#"{"in":{"起点":"2026-01-01","月数":13},"error":"range"}"#,
        ]
    );
    // Several integers: the last declared turns fastest.
    let o = koyomi(&["vectors", "examples/締め日と支払日を受け取る.cal"]);
    let text = String::from_utf8(o.stdout).unwrap();
    let second_combination: Vec<&str> = text.lines().skip(639).take(1).collect();
    assert!(second_combination[0].starts_with(r#"{"in":{"受領日":"2026-01-01","締め日":1,"支払の月":1,"支払の日":11}"#), "{}", second_combination[0]);
}

#[test]
fn a_calendar_s_vectors() {
    let ls = lines("examples/calendars/東京の営業日.cal");
    assert_eq!(ls[0], r#"{"in":{"date":"1955-01-01"},"out":{"open":false}}"#);
    assert!(ls.contains(&r#"{"in":{"date":"2026-05-08"},"out":{"open":true}}"#.to_string()));
    assert!(ls.contains(&r#"{"in":{"date":"2026-05-06"},"out":{"open":false}}"#.to_string()));
    assert_eq!(ls[26663], r#"{"in":{"date":"1954-12-31"},"error":"data"}"#);
    assert_eq!(ls[26664], r#"{"in":{"date":"2028-01-01"},"error":"data"}"#);
    // A calendar with no table knows every day; its vectors are 1900–2100, with no edge rows.
    let ls = lines("tests/fixtures/calendars/休みの書き方を全部使う.cal");
    assert_eq!(ls.len(), 73_414);
    assert!(ls[0].starts_with(r#"{"in":{"date":"1900-01-01"}"#));
    assert!(ls[73_413].starts_with(r#"{"in":{"date":"2100-12-31"}"#));
}

#[test]
fn the_library_and_the_command_agree() {
    let path = "examples/net30.cal";
    let mut o = koyomi::check::check(path).unwrap();
    let Some(koyomi::check::Checked::Dates(m, _)) = o.checked.take() else { panic!() };
    let w = koyomi::vectors::DatesLines::new(&m);
    let mine: Vec<String> = koyomi::vectors::DatesRows::new(&m).map(|r| w.line(&r)).collect();
    assert_eq!(mine, lines(path));
}

#[test]
fn no_vectors_from_a_file_that_fails() {
    let o = koyomi(&["vectors", "examples/支払_月末締め翌々月末払い.cal"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("error[E301]"), "{}", String::from_utf8_lossy(&o.stdout));
    assert!(String::from_utf8_lossy(&o.stderr).contains("does not pass check, so it has no vectors"));
    assert_eq!(koyomi(&["vectors"]).status.code(), Some(2));
    assert_eq!(koyomi(&["vectors", "examples/net30.cal", "examples/net30.cal"]).status.code(), Some(2));
}
