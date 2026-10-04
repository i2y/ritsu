//! The check (PLAN B.8) and the numbers of PLAN B.12: what the prototype of stage A computed,
//! computed again by the reference interpreter.

use koyomi::check::{Checked, Options, Outcome, Report, check, check_text};
use koyomi::date::{Day, parse};
use koyomi::diag::Diag;
use ritsu_base::text::Lang;
use koyomi::resolve::Model;

fn d(s: &str) -> Day {
    parse(s).unwrap()
}

fn passing(path: &str) -> (Box<Model>, Report, Outcome) {
    let mut o = check(path).unwrap();
    let text: String = o.diags.iter().map(|x| x.render(Lang::En)).collect();
    assert!(!o.has_errors(), "{path} fails:\n{text}");
    match o.checked.take() {
        Some(Checked::Dates(m, r)) => (m, r, o),
        _ => panic!("{path} is a dates file"),
    }
}

/// The file with `from` replaced by `to`, checked where it is.
fn changed(path: &str, from: &str, to: &str) -> Outcome {
    let src = std::fs::read_to_string(path).unwrap();
    assert!(src.contains(from), "{path} has {from}");
    check_text(path, &src.replacen(from, to, 1), &Options::default())
}

fn only(o: &Outcome, code: &str) -> Diag {
    let ds: Vec<&Diag> = o.diags.iter().filter(|x| x.is_error()).collect();
    let text: String = o.diags.iter().map(|x| x.render(Lang::En)).collect();
    assert_eq!(ds.len(), 1, "one error expected:\n{text}");
    assert_eq!(ds[0].code, code, "{text}");
    ds[0].clone()
}

fn date_ix(m: &Model, name: &str) -> usize {
    m.dates.iter().position(|x| x.name == name).unwrap()
}

#[test]
fn twentieth_closing_tenth_of_next_month() {
    let path = "examples/payment_20th_close_next_10th.ja.cal";
    let (m, r, o) = passing(path);
    assert_eq!(r.combinations, 689);
    assert_eq!(o.ok.as_ref().unwrap().en, "3 claims hold on all 689 days of 受領日 (2026-01-01..2027-11-20); 2 examples match");
    assert_eq!(o.ok.as_ref().unwrap().ja, "3 つの条件が、受領日 2026-01-01〜2027-11-20 の 689 日のすべてで成り立ちます。例 2 行も合っています");
    let pay = date_ix(&m, "支払日");
    let (long, who) = r.dates[pay].longest.clone().unwrap();
    assert_eq!(long, 51);
    let who: Vec<String> = who.iter().map(|v| Day(v[0] as i32).to_string()).collect();
    assert_eq!(who, ["2026-07-21", "2026-12-21", "2027-07-21"]);
    assert_eq!(r.dates[pay].shortest.as_ref().unwrap().0, 18);
    // `roll preceding` moves six payment days.
    let moves: Vec<(String, String)> = r.ops[pay][1].moves.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let want = [
        ("2026-05-10", "2026-05-08"),
        ("2026-10-10", "2026-10-09"),
        ("2027-01-10", "2027-01-08"),
        ("2027-04-10", "2027-04-09"),
        ("2027-07-10", "2027-07-09"),
        ("2027-10-10", "2027-10-08"),
    ];
    assert_eq!(moves, want.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>());
}

#[test]
fn the_range_past_the_table() {
    // DESIGN 4.3: the same file, its range to the end of 2027.
    let o = changed("examples/payment_20th_close_next_10th.ja.cal", "<=2027-11-20", "<=2027-12-31");
    let e = only(&o, "E203");
    assert_eq!((e.line, e.col), (Some(6), Some(3)));
    assert_eq!(e.extra.inputs, vec![("受領日".to_string(), "2027-11-21".to_string())]);
    assert!(e.message.ja.contains("2028-01-10 が営業日か"), "{}", e.message.ja);
    assert_eq!(e.extra.fails.iter().map(|r| r.days()).sum::<i64>(), 41);
    assert_eq!((e.extra.fails[0].from, e.extra.fails[0].to), (d("2027-11-21"), d("2027-12-31")));
    assert_eq!(e.fixed_line(), Some("  受領日(received) : date  range >=2026-01-01 <=2027-11-20"));
    assert_eq!(d("2027-11-21").weekday(), 6);
}

#[test]
fn end_of_month_closing_two_months_later() {
    let path = "examples/eom_close_two_months_later.ja.cal";
    let o = check(path).unwrap();
    let e = only(&o, "E301");
    assert_eq!((e.line, e.col), (Some(16), Some(3)));
    assert_eq!(e.message.en, "The claim 受領から60日以内 fails for 648 of the 669 days of 受領日");
    let runs: Vec<(String, String, i64)> = e.extra.fails.iter().map(|r| (r.from.to_string(), r.to.to_string(), r.days())).collect();
    let want = [
        ("2026-01-01", "2026-01-29", 29),
        ("2026-02-01", "2026-03-29", 57),
        ("2026-04-01", "2026-08-30", 152),
        ("2026-09-01", "2026-10-28", 58),
        ("2026-11-01", "2026-11-29", 29),
        ("2026-12-01", "2026-12-27", 27),
        ("2027-01-01", "2027-01-29", 29),
        ("2027-02-01", "2027-05-30", 119),
        ("2027-06-01", "2027-08-29", 90),
        ("2027-09-01", "2027-10-28", 58),
    ];
    assert_eq!(runs, want.iter().map(|(a, b, n)| (a.to_string(), b.to_string(), *n)).collect::<Vec<_>>());
    assert!(e.notes.iter().any(|n| n.ja == "いちばん外れるのは受領日 2026-05-01 のときで、支払日 2026-07-31 は受領日の 91 日後"), "{:?}", e.notes);
    assert_eq!(e.extra.inputs, vec![("受領日".to_string(), "2026-01-01".to_string())]);
    assert!(e.render(Lang::Ja).contains("支払日は受領日の 89 日後で、条件は 60 日後まで"));
    // The payment day 2026-12-31 (a Thursday, closed for the new year) moves to Monday 2026-12-28.
    let Some(Checked::Dates(m, r)) = &o.checked else { panic!() };
    let pay = date_ix(m, "支払日");
    assert!(r.ops[pay][1].moves.contains(&(d("2026-12-31"), d("2026-12-28"))));
    // A month more of range asks about 2028-01-31.
    let o = changed(path, "<=2027-10-31", "<=2027-11-01");
    let e = only(&o, "E203");
    assert!(e.message.en.contains("whether 2028-01-31 is a business day"), "{}", e.message.en);
}

#[test]
fn the_civil_code_period() {
    let (m, r, _) = passing("examples/civil_code_period_end.ja.cal");
    assert_eq!(r.combinations, 4380);
    let latest = m.dates.iter().enumerate().filter_map(|(k, _)| r.dates[k].latest).max().unwrap();
    assert_eq!(latest, d("2027-12-31"));
}

#[test]
fn two_readings_of_the_civil_code() {
    let o = check("examples/civil_code_two_readings.ja.cal").unwrap();
    let es: Vec<&Diag> = o.diags.iter().filter(|x| x.is_error()).collect();
    assert_eq!(es.iter().map(|x| x.code).collect::<Vec<_>>(), ["E301", "E301"]);
    let Some(Checked::Dates(m, r)) = &o.checked else { panic!() };
    let by_month = |ci: usize| -> Vec<i64> {
        let mut v = vec![0i64; 12];
        for (ps, a, b) in &r.claims[ci].runs {
            v[(ps[0] - 1) as usize] += (b.0 - a.0) as i64 + 1;
        }
        v
    };
    // 142: the day after, and the day the closure ends.
    assert_eq!(r.claims[0].fails, 121);
    assert_eq!(by_month(0), vec![11, 10, 11, 11, 11, 11, 11, 11, 9, 9, 8, 8]);
    let first = r.claims[0].first.clone().unwrap();
    assert_eq!((Day(first[0] as i32).to_string(), first[1]), ("2026-01-22".to_string(), 1));
    let t = koyomi::interp::trace(m, &first, &(0..m.dates.len()).collect::<Vec<_>>(), false);
    let get = |n: &str| t.dates[date_ix(m, n)].unwrap().to_string();
    assert_eq!(get("満了日"), "2026-02-22");
    assert_eq!(get("満了日_翌日"), "2026-02-23");
    assert_eq!(get("満了日_翌営業日"), "2026-02-24");
    let month_one: Vec<(String, String)> = r.claims[0].runs.iter().filter(|(p, ..)| p[0] == 1).map(|(_, a, b)| (a.to_string(), b.to_string())).collect();
    let want = [
        ("2026-01-22", "2026-01-22"),
        ("2026-04-03", "2026-04-05"),
        ("2026-06-19", "2026-06-19"),
        ("2026-08-20", "2026-08-22"),
        ("2026-09-11", "2026-09-11"),
        ("2026-10-22", "2026-10-22"),
        ("2026-12-10", "2026-12-10"),
    ];
    assert_eq!(month_one, want.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>());
    // 143, against adding the months and rounding to the end of the month.
    assert_eq!(r.claims[1].fails, 39);
    assert_eq!(by_month(1), vec![5, 3, 3, 5, 1, 5, 2, 4, 4, 2, 5, 0]);
    let first = r.claims[1].first.clone().unwrap();
    assert_eq!((Day(first[0] as i32).to_string(), first[1]), ("2026-02-28".to_string(), 1));
    let t = koyomi::interp::trace(m, &first, &(0..m.dates.len()).collect::<Vec<_>>(), false);
    assert_eq!(t.dates[date_ix(m, "満了日")].unwrap().to_string(), "2026-03-31");
    assert_eq!(t.dates[date_ix(m, "月数を足して寄せる")].unwrap().to_string(), "2026-03-28");
    let month_one: Vec<String> = r.claims[1].runs.iter().filter(|(p, ..)| p[0] == 1).map(|(_, a, _)| a.to_string()).collect();
    assert_eq!(month_one, ["2026-02-28", "2026-04-30", "2026-06-30", "2026-09-30", "2026-11-30"]);
}

#[test]
fn closing_and_payment_days_as_inputs() {
    let path = "examples/closing_and_payment_days_as_inputs.ja.cal";
    let (m, r, _) = passing(path);
    assert_eq!(r.combinations, 871_596);
    let pay = date_ix(&m, "支払");
    let (long, who) = r.dates[pay].longest.clone().unwrap();
    assert_eq!(long, 121);
    // 受領日 2026-05-02, 締め日 1, 支払の月 2, 支払の日 31: paid 2026-08-31.
    assert!(who.contains(&vec![d("2026-05-02").0 as i64, 1, 2, 31]), "{who:?}");
    let t = koyomi::interp::trace(&m, &[d("2026-05-02").0 as i64, 1, 2, 31], &[pay], false);
    assert_eq!(t.dates[pay], Some(d("2026-08-31")));
    let o = changed(path, "<=2027-10-01", "<=2027-10-02");
    let e = only(&o, "E203");
    assert!(e.message.en.contains("whether 2028-01-10 is a business day"), "{}", e.message.en);
}

#[test]
fn net_30() {
    let path = "examples/net30.cal";
    let (m, r, _) = passing(path);
    assert_eq!(r.combinations, 1064);
    let due = date_ix(&m, "due");
    let (long, who) = r.dates[due].longest.clone().unwrap();
    assert_eq!(long, 34);
    let who: Vec<String> = who.iter().map(|v| Day(v[0] as i32).to_string()).collect();
    assert_eq!(who, ["2026-03-04", "2026-11-25", "2027-02-24", "2027-11-25", "2028-03-15", "2028-11-23"]);
    let o = changed(path, "<=2028-11-29", "<=2028-11-30");
    let e = only(&o, "E203");
    assert!(e.message.en.contains("whether 2029-01-01 is a business day"), "{}", e.message.en);
    // `roll modified following` parts from `roll following` on 21 days, the first 2026-01-01.
    let src = std::fs::read_to_string(path).unwrap();
    let src = src
        .replace("closed\n\nclaims\n", "closed\n\ndate due_modified = invoice_date\n  + 30 days\n  roll modified following\n\nclaims\n  the_same : due = due_modified\n");
    let o = check_text(path, &src, &Options::default());
    let e = only(&o, "E301");
    assert!(e.message.en.contains("fails for 21 of the 1,064 days"), "{}", e.message.en);
    assert_eq!(e.extra.inputs, vec![("invoice_date".to_string(), "2026-01-01".to_string())]);
    let text = e.render(Lang::En);
    assert!(text.contains("2026-02-02") && text.contains("2026-01-30"), "{text}");
}

#[test]
fn eval_as_design_shows_it() {
    let path = "examples/payment_20th_close_next_10th.ja.cal";
    let mut loader = koyomi::calendar::Loader::default();
    let (m, _) = koyomi::check::prepare(path, &mut loader).map_err(|_| ()).unwrap();
    let vals = koyomi::eval::parse_inputs(&m, &["受領日=2026-04-01".to_string()]).unwrap();
    let (out, ok) = koyomi::eval::eval_dates(&m, &vals, Lang::Ja, false);
    assert!(ok);
    let want = "受領日  2026-04-01（水）
締め日  2026-04-20（月）  20 日締め: 2026-03-21〜2026-04-20 の期間の締め日
支払日  2026-05-10（日）  翌月 10 日
        2026-05-08（金）  休みなら前営業日: 2026-05-10 は日曜、2026-05-09 は土曜で休み
        時刻 2026-05-08T09:00:00+09:00（UTC で 2026-05-08T00:00:00Z）
";
    assert!(out.starts_with(want), "{out}");
    let (json, _) = koyomi::eval::eval_dates(&m, &vals, Lang::En, true);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["times"]["支払日"]["utc"], "2026-05-08T00:00:00Z");
    assert_eq!(v["dates"]["締め日"], "2026-04-20");
}

#[test]
fn the_examples_of_design_1_1() {
    let (m, _, _) = passing("examples/payment_20th_close_next_10th.ja.cal");
    for (input, closing, payment) in [("2026-04-01", "2026-04-20", "2026-05-08"), ("2026-12-21", "2027-01-20", "2027-02-10")] {
        let t = koyomi::interp::trace(&m, &[d(input).0 as i64], &[0, 1], false);
        assert_eq!(t.dates[0], Some(d(closing)));
        assert_eq!(t.dates[1], Some(d(payment)));
    }
}

/// DESIGN 4.3: the day to add a month to that has no such day in the next month.
#[test]
fn e201_names_the_first_input_that_lands_on_a_missing_day() {
    let src = "dates 一か月後の例(month_later_example) v1\n\ninputs\n  受領日(received) : date  range >=2026-01-01 <=2026-12-31\n\ndate 一か月後(month_later) = 受領日\n  + 1 month\n";
    let o = check_text("一か月後の例.cal", src, &Options::default());
    let e = only(&o, "E201");
    assert_eq!((e.line, e.col), (Some(7), Some(3)));
    assert_eq!(e.extra.inputs, vec![("受領日".to_string(), "2026-01-29".to_string())]);
    assert!(e.notes[0].ja.contains("2026-02-29"), "{:?}", e.notes);
    assert!(e.notes[1].ja.contains("2026-02-28 にする") && e.notes[1].ja.contains("2026-03-01 にする"), "{:?}", e.notes);
    assert_eq!(e.fixed_line(), Some("  + 1 month else end_of_month"));
    // In a range where no day is missing, it still asks, and says so.
    let o = check_text("t.cal", &src.replace("<=2026-12-31", "<=2026-01-28"), &Options::default());
    let e = only(&o, "E201");
    assert!(e.notes[0].en.contains("does not happen in the range"), "{:?}", e.notes);
}

#[test]
fn the_budget() {
    let path = "examples/closing_and_payment_days_as_inputs.ja.cal";
    let src = std::fs::read_to_string(path).unwrap();
    let o = check_text(path, &src, &Options { budget: 871_595 });
    let e = only(&o, "E305");
    assert!(e.message.en.contains("871,596"), "{}", e.message.en);
    let o = check_text(path, &src, &Options { budget: 871_596 });
    assert!(!o.has_errors());
}

#[test]
fn every_example_checks_as_designed() {
    // The six that fail on purpose (on England and Wales, the English versions of the Japanese
    // examples, and the Japanese versions), and the rest.
    for f in std::fs::read_dir("examples").unwrap().chain(std::fs::read_dir("examples/calendars").unwrap()) {
        let p = f.unwrap().path();
        if p.extension().is_none_or(|e| e != "cal") {
            continue;
        }
        let o = check(p.to_str().unwrap()).unwrap();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let fails_on_purpose = [
            "close_eom_pay_two_months_on.cal",
            "period_of_months_two_readings.cal",
            "eom_close_two_months_later.cal",
            "civil_code_two_readings.cal",
            "eom_close_two_months_later.ja.cal",
            "civil_code_two_readings.ja.cal",
        ]
        .contains(&name.as_str());
        let text: String = o.diags.iter().map(|x| x.render(Lang::En)).collect();
        assert_eq!(o.has_errors(), fails_on_purpose, "{name}:\n{text}");
        assert!(o.diags.iter().all(|x| x.is_error()), "{name} has warnings:\n{text}");
    }
}

#[test]
fn names_the_generated_code_leans_on_are_refused() {
    // Go's `err`, the Python builtins the generated Python calls, PL/pgSQL's reserved words
    // (PLAN C, src/naming.rs and src/reserved.rs): E009, before any code is generated.
    for a in ["err", "range", "str", "isinstance", "frozenset", "begin", "declare", "execute", "foreach", "strict"] {
        let src = format!("dates 試験(test) v1\n\ninputs\n  入力({a}) : date  range >=2026-01-01 <=2026-01-02\n\ndate 翌日(next_day) = 入力\n  + 1 day\n");
        let o = check_text("tests/試験.cal", &src, &Options::default());
        assert!(o.diags.iter().any(|d| d.code == "E009"), "{a} is refused:\n{}", o.diags.iter().map(|d| d.render(Lang::En)).collect::<String>());
    }
    // A file's alias names a module and a schema.
    for a in ["json", "datetime", "std", "pg_catalog"] {
        let src = format!("dates 試験({a}) v1\n\ninputs\n  入力(day_in) : date  range >=2026-01-01 <=2026-01-02\n\ndate 翌日(next_day) = 入力\n  + 1 day\n");
        let o = check_text("tests/試験.cal", &src, &Options::default());
        assert!(o.diags.iter().any(|d| d.code == "E009"), "the file alias {a} is refused");
    }
}

// ── The English versions of the Japanese examples: the same terms with English names, on the
// English version of the same calendar, give the same numbers ────────────────────────────────

#[test]
fn twentieth_closing_tenth_of_next_month_in_english() {
    let path = "examples/payment_20th_close_next_10th.cal";
    let (m, r, o) = passing(path);
    assert_eq!(r.combinations, 689);
    assert_eq!(o.ok.as_ref().unwrap().en, "3 claims hold on all 689 days of received (2026-01-01..2027-11-20); 2 examples match");
    assert_eq!(o.ok.as_ref().unwrap().ja, "3 つの条件が、received 2026-01-01〜2027-11-20 の 689 日のすべてで成り立ちます。例 2 行も合っています");
    let pay = date_ix(&m, "payment");
    let (long, who) = r.dates[pay].longest.clone().unwrap();
    assert_eq!(long, 51);
    let who: Vec<String> = who.iter().map(|v| Day(v[0] as i32).to_string()).collect();
    assert_eq!(who, ["2026-07-21", "2026-12-21", "2027-07-21"]);
    assert_eq!(r.dates[pay].shortest.as_ref().unwrap().0, 18);
    let moves: Vec<(String, String)> = r.ops[pay][1].moves.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let want = [
        ("2026-05-10", "2026-05-08"),
        ("2026-10-10", "2026-10-09"),
        ("2027-01-10", "2027-01-08"),
        ("2027-04-10", "2027-04-09"),
        ("2027-07-10", "2027-07-09"),
        ("2027-10-10", "2027-10-08"),
    ];
    assert_eq!(moves, want.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>());
}

#[test]
fn the_range_past_the_table_in_english() {
    let o = changed("examples/payment_20th_close_next_10th.cal", "<=2027-11-20", "<=2027-12-31");
    let e = only(&o, "E203");
    assert_eq!((e.line, e.col), (Some(6), Some(3)));
    assert_eq!(e.extra.inputs, vec![("received".to_string(), "2027-11-21".to_string())]);
    assert!(e.message.ja.contains("2028-01-10 が営業日か"), "{}", e.message.ja);
    assert_eq!(e.extra.fails.iter().map(|r| r.days()).sum::<i64>(), 41);
    assert_eq!((e.extra.fails[0].from, e.extra.fails[0].to), (d("2027-11-21"), d("2027-12-31")));
    assert_eq!(e.fixed_line(), Some("  received : date  range >=2026-01-01 <=2027-11-20"));
}

#[test]
fn end_of_month_closing_two_months_later_in_english() {
    let path = "examples/eom_close_two_months_later.cal";
    let o = check(path).unwrap();
    let e = only(&o, "E301");
    assert_eq!((e.line, e.col), (Some(16), Some(3)));
    assert_eq!(e.message.en, "The claim within_60_days_of_receipt fails for 648 of the 669 days of received");
    let runs: Vec<(String, String, i64)> = e.extra.fails.iter().map(|r| (r.from.to_string(), r.to.to_string(), r.days())).collect();
    let want = [
        ("2026-01-01", "2026-01-29", 29),
        ("2026-02-01", "2026-03-29", 57),
        ("2026-04-01", "2026-08-30", 152),
        ("2026-09-01", "2026-10-28", 58),
        ("2026-11-01", "2026-11-29", 29),
        ("2026-12-01", "2026-12-27", 27),
        ("2027-01-01", "2027-01-29", 29),
        ("2027-02-01", "2027-05-30", 119),
        ("2027-06-01", "2027-08-29", 90),
        ("2027-09-01", "2027-10-28", 58),
    ];
    assert_eq!(runs, want.iter().map(|(a, b, n)| (a.to_string(), b.to_string(), *n)).collect::<Vec<_>>());
    assert!(e.notes.iter().any(|n| n.en == "The farthest is received 2026-05-01, where payment 2026-07-31 is 91 days after received"), "{:?}", e.notes);
    assert_eq!(e.extra.inputs, vec![("received".to_string(), "2026-01-01".to_string())]);
    assert!(e.render(Lang::En).contains("payment is 89 days after received, and the claim allows at most 60 days after"));
    let Some(Checked::Dates(m, r)) = &o.checked else { panic!() };
    let pay = date_ix(m, "payment");
    assert!(r.ops[pay][1].moves.contains(&(d("2026-12-31"), d("2026-12-28"))));
    let o = changed(path, "<=2027-10-31", "<=2027-11-01");
    let e = only(&o, "E203");
    assert!(e.message.en.contains("whether 2028-01-31 is a business day"), "{}", e.message.en);
}

#[test]
fn the_civil_code_period_in_english() {
    let (m, r, _) = passing("examples/civil_code_period_end.cal");
    assert_eq!(r.combinations, 4380);
    let latest = m.dates.iter().enumerate().filter_map(|(k, _)| r.dates[k].latest).max().unwrap();
    assert_eq!(latest, d("2027-12-31"));
}

#[test]
fn two_readings_of_the_civil_code_in_english() {
    let o = check("examples/civil_code_two_readings.cal").unwrap();
    let es: Vec<&Diag> = o.diags.iter().filter(|x| x.is_error()).collect();
    assert_eq!(es.iter().map(|x| x.code).collect::<Vec<_>>(), ["E301", "E301"]);
    let Some(Checked::Dates(m, r)) = &o.checked else { panic!() };
    let by_month = |ci: usize| -> Vec<i64> {
        let mut v = vec![0i64; 12];
        for (ps, a, b) in &r.claims[ci].runs {
            v[(ps[0] - 1) as usize] += (b.0 - a.0) as i64 + 1;
        }
        v
    };
    assert_eq!(r.claims[0].fails, 121);
    assert_eq!(by_month(0), vec![11, 10, 11, 11, 11, 11, 11, 11, 9, 9, 8, 8]);
    let first = r.claims[0].first.clone().unwrap();
    assert_eq!((Day(first[0] as i32).to_string(), first[1]), ("2026-01-22".to_string(), 1));
    let t = koyomi::interp::trace(m, &first, &(0..m.dates.len()).collect::<Vec<_>>(), false);
    let get = |n: &str| t.dates[date_ix(m, n)].unwrap().to_string();
    assert_eq!(get("last_day"), "2026-02-22");
    assert_eq!(get("next_day"), "2026-02-23");
    assert_eq!(get("next_business_day"), "2026-02-24");
    assert_eq!(r.claims[1].fails, 39);
    assert_eq!(by_month(1), vec![5, 3, 3, 5, 1, 5, 2, 4, 4, 2, 5, 0]);
    let first = r.claims[1].first.clone().unwrap();
    assert_eq!((Day(first[0] as i32).to_string(), first[1]), ("2026-02-28".to_string(), 1));
    let t = koyomi::interp::trace(m, &first, &(0..m.dates.len()).collect::<Vec<_>>(), false);
    assert_eq!(t.dates[date_ix(m, "last_day")].unwrap().to_string(), "2026-03-31");
    assert_eq!(t.dates[date_ix(m, "months_added")].unwrap().to_string(), "2026-03-28");
}

#[test]
fn closing_and_payment_days_as_inputs_in_english() {
    let path = "examples/closing_and_payment_days_as_inputs.cal";
    let (m, r, _) = passing(path);
    assert_eq!(r.combinations, 871_596);
    let pay = date_ix(&m, "payment");
    let (long, who) = r.dates[pay].longest.clone().unwrap();
    assert_eq!(long, 121);
    assert!(who.contains(&vec![d("2026-05-02").0 as i64, 1, 2, 31]), "{who:?}");
    let t = koyomi::interp::trace(&m, &[d("2026-05-02").0 as i64, 1, 2, 31], &[pay], false);
    assert_eq!(t.dates[pay], Some(d("2026-08-31")));
    let o = changed(path, "<=2027-10-01", "<=2027-10-02");
    let e = only(&o, "E203");
    assert!(e.message.en.contains("whether 2028-01-10 is a business day"), "{}", e.message.en);
}

#[test]
fn eval_as_design_shows_it_in_english() {
    let path = "examples/payment_20th_close_next_10th.cal";
    let mut loader = koyomi::calendar::Loader::default();
    let (m, _) = koyomi::check::prepare(path, &mut loader).map_err(|_| ()).unwrap();
    let vals = koyomi::eval::parse_inputs(&m, &["received=2026-04-01".to_string()]).unwrap();
    let (out, ok) = koyomi::eval::eval_dates(&m, &vals, Lang::En, false);
    assert!(ok);
    let want = "received  2026-04-01 Wed
closing   2026-04-20 Mon  close day 20: closes the period 2026-03-21..2026-04-20
payment   2026-05-10 Sun  day 10 of month +1
          2026-05-08 Fri  roll preceding: 2026-05-10 (Sunday) and 2026-05-09 (Saturday) are closed
          time 2026-05-08T09:00:00+09:00 (2026-05-08T00:00:00Z in UTC)
";
    assert!(out.starts_with(want), "{out}");
    let (json, _) = koyomi::eval::eval_dates(&m, &vals, Lang::En, true);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["times"]["payment"]["utc"], "2026-05-08T00:00:00Z");
    assert_eq!(v["dates"]["closing"], "2026-04-20");
}

#[test]
fn the_examples_of_design_1_1_in_english() {
    let (m, _, _) = passing("examples/payment_20th_close_next_10th.cal");
    for (input, closing, payment) in [("2026-04-01", "2026-04-20", "2026-05-08"), ("2026-12-21", "2027-01-20", "2027-02-10")] {
        let t = koyomi::interp::trace(&m, &[d(input).0 as i64], &[0, 1], false);
        assert_eq!(t.dates[0], Some(d(closing)));
        assert_eq!(t.dates[1], Some(d(payment)));
    }
}

#[test]
fn e201_names_the_first_input_that_lands_on_a_missing_day_in_english() {
    let src = "dates month_later_example v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-31\n\ndate month_later = received\n  + 1 month\n";
    let o = check_text("month_later_example.cal", src, &Options::default());
    let e = only(&o, "E201");
    assert_eq!((e.line, e.col), (Some(7), Some(3)));
    assert_eq!(e.extra.inputs, vec![("received".to_string(), "2026-01-29".to_string())]);
    assert!(e.notes[0].en.contains("2026-02-29"), "{:?}", e.notes);
    assert_eq!(e.fixed_line(), Some("  + 1 month else end_of_month"));
    let o = check_text("t.cal", &src.replace("<=2026-12-31", "<=2026-01-28"), &Options::default());
    let e = only(&o, "E201");
    assert!(e.notes[0].en.contains("does not happen in the range"), "{:?}", e.notes);
}

#[test]
fn the_budget_in_english() {
    let path = "examples/closing_and_payment_days_as_inputs.cal";
    let src = std::fs::read_to_string(path).unwrap();
    let o = check_text(path, &src, &Options { budget: 871_595 });
    let e = only(&o, "E305");
    assert!(e.message.en.contains("871,596"), "{}", e.message.en);
    let o = check_text(path, &src, &Options { budget: 871_596 });
    assert!(!o.has_errors());
}

#[test]
fn names_the_generated_code_leans_on_are_refused_in_english() {
    for a in ["err", "range", "str", "isinstance", "frozenset", "begin", "declare", "execute", "foreach", "strict"] {
        let src = format!("dates test v1\n\ninputs\n  input({a}) : date  range >=2026-01-01 <=2026-01-02\n\ndate next_day = input\n  + 1 day\n");
        let o = check_text("tests/test.cal", &src, &Options::default());
        assert!(o.diags.iter().any(|d| d.code == "E009"), "{a} is refused:\n{}", o.diags.iter().map(|d| d.render(Lang::En)).collect::<String>());
    }
    for a in ["json", "datetime", "std", "pg_catalog"] {
        let src = format!("dates test({a}) v1\n\ninputs\n  input(day_in) : date  range >=2026-01-01 <=2026-01-02\n\ndate next_day = input\n  + 1 day\n");
        let o = check_text("tests/test.cal", &src, &Options::default());
        assert!(o.diags.iter().any(|d| d.code == "E009"), "the file alias {a} is refused");
    }
}

// ── The examples on England and Wales: the numbers of an independent prototype (Python's
// datetime and GOV.UK's bank holidays), computed again by the reference interpreter ────────────

#[test]
fn twentieth_closing_tenth_of_next_month_in_england_and_wales() {
    let path = "examples/close_20th_pay_10th.cal";
    let (m, r, o) = passing(path);
    assert_eq!(r.combinations, 1055);
    assert_eq!(o.ok.as_ref().unwrap().en, "3 claims hold on all 1,055 days of received (2026-01-01..2028-11-20); 2 examples match");
    assert_eq!(o.ok.as_ref().unwrap().ja, "3 つの条件が、received 2026-01-01〜2028-11-20 の 1,055 日のすべてで成り立ちます。例 2 行も合っています");
    let pay = date_ix(&m, "payment");
    let (long, who) = r.dates[pay].longest.clone().unwrap();
    assert_eq!(long, 51);
    let who: Vec<String> = who.iter().map(|v| Day(v[0] as i32).to_string()).collect();
    assert_eq!(who, ["2026-07-21", "2026-12-21", "2027-07-21", "2027-12-21"]);
    assert_eq!(r.dates[pay].shortest.as_ref().unwrap().0, 18);
    // `roll preceding` moves nine payment days.
    let moves: Vec<(String, String)> = r.ops[pay][1].moves.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let want = [
        ("2026-05-10", "2026-05-08"),
        ("2026-10-10", "2026-10-09"),
        ("2027-01-10", "2027-01-08"),
        ("2027-04-10", "2027-04-09"),
        ("2027-07-10", "2027-07-09"),
        ("2027-10-10", "2027-10-08"),
        ("2028-06-10", "2028-06-09"),
        ("2028-09-10", "2028-09-08"),
        ("2028-12-10", "2028-12-08"),
    ];
    assert_eq!(moves, want.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>());
}

#[test]
fn the_range_past_the_table_in_england_and_wales() {
    // DESIGN 4.3: the same file, its range to the end of 2028, where GOV.UK's table ends.
    let o = changed("examples/close_20th_pay_10th.cal", "<=2028-11-20", "<=2028-12-31");
    let e = only(&o, "E203");
    assert_eq!((e.line, e.col), (Some(6), Some(3)));
    assert_eq!(e.extra.inputs, vec![("received".to_string(), "2028-11-21".to_string())]);
    assert!(e.message.ja.contains("2029-01-10 が営業日か"), "{}", e.message.ja);
    assert_eq!(e.extra.fails.iter().map(|r| r.days()).sum::<i64>(), 41);
    assert_eq!((e.extra.fails[0].from, e.extra.fails[0].to), (d("2028-11-21"), d("2028-12-31")));
    assert_eq!(e.fixed_line(), Some("  received : date  range >=2026-01-01 <=2028-11-20"));
    assert_eq!(d("2028-11-21").weekday(), 1);
}

#[test]
fn end_of_month_closing_two_months_later_in_england_and_wales() {
    let path = "examples/close_eom_pay_two_months_on.cal";
    let o = check(path).unwrap();
    let e = only(&o, "E301");
    assert_eq!((e.line, e.col), (Some(16), Some(3)));
    assert_eq!(e.message.en, "The claim within_60_days_of_receipt fails for 1,008 of the 1,035 days of received");
    let runs: Vec<(String, String, i64)> = e.extra.fails.iter().map(|r| (r.from.to_string(), r.to.to_string(), r.days())).collect();
    let want = [
        ("2026-01-01", "2026-01-29", 29),
        ("2026-02-01", "2026-03-29", 57),
        ("2026-04-01", "2026-06-28", 89),
        ("2026-07-01", "2026-08-30", 61),
        ("2026-09-01", "2026-11-29", 90),
        ("2026-12-01", "2026-12-27", 27),
        ("2027-01-01", "2027-01-29", 29),
        ("2027-02-01", "2027-03-28", 56),
        ("2027-04-01", "2027-05-30", 60),
        ("2027-06-01", "2027-08-29", 90),
        ("2027-09-01", "2027-12-30", 121),
        ("2028-01-01", "2028-01-30", 30),
        ("2028-02-01", "2028-02-27", 27),
        ("2028-03-01", "2028-07-30", 152),
        ("2028-08-01", "2028-10-29", 90),
    ];
    assert_eq!(runs, want.iter().map(|(a, b, n)| (a.to_string(), b.to_string(), *n)).collect::<Vec<_>>());
    assert!(e.notes.iter().any(|n| n.ja == "いちばん外れるのはreceived 2026-05-01 のときで、payment 2026-07-31 はreceivedの 91 日後"), "{:?}", e.notes);
    assert_eq!(e.extra.inputs, vec![("received".to_string(), "2026-01-01".to_string())]);
    assert!(e.render(Lang::Ja).contains("payment は received の 89 日後で、条件は 60 日後まで"));
    // The payment day 2027-05-31 (a Monday, the spring bank holiday) moves to Friday 2027-05-28.
    let Some(Checked::Dates(m, r)) = &o.checked else { panic!() };
    let pay = date_ix(m, "payment");
    assert!(r.ops[pay][1].moves.contains(&(d("2027-05-31"), d("2027-05-28"))));
    // A month more of range asks about 2029-01-31.
    let o = changed(path, "<=2028-10-31", "<=2028-11-01");
    let e = only(&o, "E203");
    assert!(e.message.en.contains("whether 2029-01-31 is a business day"), "{}", e.message.en);
}

#[test]
fn the_period_of_months() {
    let (m, r, _) = passing("examples/period_of_months.cal");
    assert_eq!(r.combinations, 4380);
    let latest = m.dates.iter().enumerate().filter_map(|(k, _)| r.dates[k].latest).max().unwrap();
    assert_eq!(latest, d("2027-12-31"));
}

#[test]
fn two_readings_of_a_period() {
    let o = check("examples/period_of_months_two_readings.cal").unwrap();
    let es: Vec<&Diag> = o.diags.iter().filter(|x| x.is_error()).collect();
    assert_eq!(es.iter().map(|x| x.code).collect::<Vec<_>>(), ["E301", "E301"]);
    let Some(Checked::Dates(m, r)) = &o.checked else { panic!() };
    let by_month = |ci: usize| -> Vec<i64> {
        let mut v = vec![0i64; 12];
        for (ps, a, b) in &r.claims[ci].runs {
            v[(ps[0] - 1) as usize] += (b.0 - a.0) as i64 + 1;
        }
        v
    };
    // The very next day, and the next business day.
    assert_eq!(r.claims[0].fails, 709);
    assert_eq!(by_month(0), vec![61, 58, 61, 58, 60, 57, 60, 59, 58, 60, 57, 60]);
    let first = r.claims[0].first.clone().unwrap();
    assert_eq!((Day(first[0] as i32).to_string(), first[1]), ("2026-01-07".to_string(), 1));
    let t = koyomi::interp::trace(m, &first, &(0..m.dates.len()).collect::<Vec<_>>(), false);
    let get = |n: &str| t.dates[date_ix(m, n)].unwrap().to_string();
    assert_eq!(get("last_day"), "2026-02-07");
    assert_eq!(get("next_day"), "2026-02-08");
    assert_eq!(get("next_business_day"), "2026-02-09");
    let month_one: Vec<(String, String)> = r.claims[0].runs.iter().filter(|(p, ..)| p[0] == 1).map(|(_, a, b)| (a.to_string(), b.to_string())).collect();
    assert_eq!(month_one.len(), 50);
    let want = [
        ("2026-01-07", "2026-01-07"),
        ("2026-01-14", "2026-01-14"),
        ("2026-01-21", "2026-01-21"),
        ("2026-01-28", "2026-01-31"),
        ("2026-02-07", "2026-02-07"),
    ];
    assert_eq!(month_one[..5], want.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>());
    // Easter: the ends on Good Friday, Saturday and Easter Sunday part the two readings.
    assert!(month_one.contains(&("2026-03-03".to_string(), "2026-03-05".to_string())), "{month_one:?}");
    // The day before the corresponding day, against adding the months and rounding down.
    assert_eq!(r.claims[1].fails, 39);
    assert_eq!(by_month(1), vec![5, 3, 3, 5, 1, 5, 2, 4, 4, 2, 5, 0]);
    let first = r.claims[1].first.clone().unwrap();
    assert_eq!((Day(first[0] as i32).to_string(), first[1]), ("2026-02-28".to_string(), 1));
    let t = koyomi::interp::trace(m, &first, &(0..m.dates.len()).collect::<Vec<_>>(), false);
    assert_eq!(t.dates[date_ix(m, "last_day")].unwrap().to_string(), "2026-03-31");
    assert_eq!(t.dates[date_ix(m, "months_added")].unwrap().to_string(), "2026-03-28");
    let month_one: Vec<String> = r.claims[1].runs.iter().filter(|(p, ..)| p[0] == 1).map(|(_, a, _)| a.to_string()).collect();
    assert_eq!(month_one, ["2026-02-28", "2026-04-30", "2026-06-30", "2026-09-30", "2026-11-30"]);
}

#[test]
fn closing_and_payment_days_as_inputs_in_england_and_wales() {
    let path = "examples/close_and_pay_on_given_days.cal";
    let (m, r, _) = passing(path);
    assert_eq!(r.combinations, 872_960);
    let pay = date_ix(&m, "payment");
    let (long, who) = r.dates[pay].longest.clone().unwrap();
    assert_eq!(long, 121);
    // received 2027-05-02, closing_day 1, payment_month 2, payment_day 31: paid 2027-08-31.
    assert!(who.contains(&vec![d("2027-05-02").0 as i64, 1, 2, 31]), "{who:?}");
    let t = koyomi::interp::trace(&m, &[d("2027-05-02").0 as i64, 1, 2, 31], &[pay], false);
    assert_eq!(t.dates[pay], Some(d("2027-08-31")));
    let o = changed(path, "<=2028-10-01", "<=2028-10-02");
    let e = only(&o, "E203");
    assert!(e.message.en.contains("whether 2029-01-10 is a business day"), "{}", e.message.en);
}
