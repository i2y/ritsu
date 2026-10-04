//! ritsu's ports, as koyomi answers them (ritsu's DESIGN 3.2, PLAN D.2): the facts of a dates
//! file are what `koyomi api` says of it; the days a date comes to and the days from its input
//! are the walk `koyomi check` and `koyomi vectors` make; an evaluation is the reference
//! interpreter's; and a `.cal` names its items and its references by ritsu's naming.

use koyomi::ports::Engine;
use ritsu_ports::{DateKind, DateValue, Dates, Found, Items, References};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The dates files of the examples (the calendars beside them are read through them).
fn dates_files() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(root().join("examples"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cal"))
        .map(|p| format!("examples/{}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    out.sort();
    out
}

fn day(s: &str) -> i64 {
    let (y, rest) = s.split_once('-').unwrap();
    let (m, d) = rest.split_once('-').unwrap();
    koyomi::date::Day::from_ymd(y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap()).unwrap().0 as i64
}

#[test]
fn the_facts_are_what_koyomi_api_says() {
    let mut files = 0;
    for path in dates_files() {
        let (m, _) = koyomi::check::prepare(&path, &mut koyomi::calendar::Loader::default()).unwrap_or_else(|_| panic!("{path}"));
        // a file whose check fails (an example of a claim that does not hold) has no facts: what
        // check says comes back instead
        let checked = koyomi::check::check(&path).unwrap();
        if checked.has_errors() {
            let said = Engine.facts(Path::new(&path)).unwrap_err();
            assert_eq!(said.iter().map(|s| s.code.clone()).collect::<Vec<_>>(), checked.diags.iter().filter(|d| d.is_error()).map(|d| d.code.to_string()).collect::<Vec<_>>(), "{path}");
            continue;
        }
        let api = koyomi::api::dates_json(&m);
        let f = Engine.facts(Path::new(&path)).unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert_eq!(f.name, api["name"].as_str().unwrap(), "{path}");
        assert_eq!(Some(f.alias.as_str()), api["alias"].as_str(), "{path}");
        assert_eq!(f.version, api["version"].as_str().unwrap(), "{path}");
        assert_eq!(f.sha256, api["source_sha256"].as_str().unwrap(), "{path}");
        let inputs = api["inputs"].as_array().unwrap();
        assert_eq!(f.inputs.len(), inputs.len(), "{path}");
        for (i, j) in f.inputs.iter().zip(inputs) {
            assert_eq!((i.name.as_str(), i.alias.as_str()), (j["name"].as_str().unwrap(), j["alias"].as_str().unwrap()), "{path}");
            match i.kind {
                DateKind::Date => assert_eq!((i.min, i.max), (day(j["range"]["min"].as_str().unwrap()), day(j["range"]["max"].as_str().unwrap())), "{path}"),
                DateKind::Int => assert_eq!((i.min, i.max), (j["range"]["min"].as_i64().unwrap(), j["range"]["max"].as_i64().unwrap()), "{path}"),
            }
        }
        let dates = api["dates"].as_array().unwrap();
        assert_eq!(f.functions.len(), dates.len(), "{path}");
        for (d, j) in f.functions.iter().zip(dates) {
            assert_eq!((d.name.as_str(), d.alias.as_str()), (j["name"].as_str().unwrap(), j["alias"].as_str().unwrap()), "{path}");
            let params: Vec<&str> = j["params"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
            assert_eq!(d.params, params, "{path}");
            let at = j.get("at").map(|a| a["time"].as_str().unwrap().to_string());
            let want = at.map(|t| if t == "end of day" { 1440 } else { t[..2].parse::<u32>().unwrap() * 60 + t[3..].parse::<u32>().unwrap() });
            assert_eq!(d.at, want, "{path}: {}", d.name);
        }
        let claims: Vec<(String, String)> = api["claims"].as_array().unwrap().iter().map(|c| (c["name"].as_str().unwrap().to_string(), c["text"].as_str().unwrap().to_string())).collect();
        assert_eq!(f.claims, claims, "{path}");
        match (&f.calendar, api["calendar"].as_object()) {
            (None, None) => {}
            (Some(c), Some(j)) => {
                assert_eq!(c.name, j["name"].as_str().unwrap(), "{path}");
                assert_eq!(c.data, (day(j["data_range"]["from"].as_str().unwrap()), day(j["data_range"]["to"].as_str().unwrap())), "{path}");
            }
            (a, b) => panic!("{path}: {a:?} {b:?}"),
        }
        files += 1;
    }
    assert!(files >= 5, "{files} files");
}

/// The example DESIGN 7.5 counts: what 支払日 comes to over the 689 days of 受領日, and the days
/// from 受領日 to it.
#[test]
fn a_dates_values_and_its_days_are_counted_exactly() {
    let file = Path::new("examples/payment_20th_close_next_10th.ja.cal");
    let Found::Value(set) = Engine.values(file, "支払日").unwrap() else { panic!("undecided") };
    assert_eq!(set.len(), 23);
    assert_eq!(*set.first().unwrap(), day("2026-02-10"));
    assert_eq!(*set.last().unwrap(), day("2027-12-10"));
    for d in &set {
        let (_, _, dd) = koyomi::date::Day(*d as i32).ymd();
        assert!([8, 9, 10].contains(&dd), "{dd}");
    }
    assert_eq!(Engine.days(file, "支払日").unwrap(), Found::Value((18, 51)));
    // the same walk as the vectors: every row's 支払日 is in the set
    let (m, _) = koyomi::check::prepare(&file.to_string_lossy(), &mut koyomi::calendar::Loader::default()).unwrap();
    let k = m.dates.iter().position(|d| d.name == "支払日").unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for row in koyomi::vectors::DatesRows::new(&m) {
        if let koyomi::vectors::Expect::Values(vs) = &row.expect {
            seen.insert(day(&vs[k]));
        }
    }
    assert_eq!(seen, set);
    // a date the file does not have
    assert!(Engine.values(file, "無い日").is_err());
}

/// An evaluation is the reference interpreter's: for every row of the vectors, the same days.
#[test]
fn an_evaluation_is_what_the_vectors_say() {
    let mut rows = 0;
    for path in dates_files() {
        if koyomi::check::check(&path).unwrap().has_errors() {
            continue;
        }
        let (m, _) = koyomi::check::prepare(&path, &mut koyomi::calendar::Loader::default()).unwrap();
        let timed = m.cal.as_ref().and_then(|c| c.offset).is_some();
        for row in koyomi::vectors::DatesRows::new(&m).step_by(37).take(40) {
            let inputs: Vec<(String, i64)> = m.inputs.iter().zip(&row.vals).map(|(i, v)| (i.name.clone(), *v)).collect();
            let got = Engine.eval(Path::new(&path), &inputs);
            match &row.expect {
                koyomi::vectors::Expect::Values(vs) => {
                    let got = got.unwrap_or_else(|e| panic!("{path}: {e:?}"));
                    // the vectors put a date's time after it, when it has one
                    let days: Vec<&String> = m.dates.iter().scan(0usize, |at, d| {
                        let v = &vs[*at];
                        *at += if d.at.is_some() && timed { 2 } else { 1 };
                        Some(v)
                    }).collect();
                    for ((name, v), want) in got.iter().zip(days) {
                        assert_eq!(*v, DateValue::Day(day(want)), "{path}: {name} at {:?}", row.vals);
                    }
                }
                koyomi::vectors::Expect::Error("range") => assert!(got.is_err(), "{path}: {:?} is outside the range", row.vals),
                koyomi::vectors::Expect::Error(_) => assert!(got.unwrap().iter().any(|(_, v)| matches!(v, DateValue::Stopped(_))), "{path}"),
                koyomi::vectors::Expect::Open(_) => unreachable!("a dates file's rows are dates"),
            }
            rows += 1;
        }
    }
    assert!(rows > 100, "{rows} rows");
}

/// What a `.cal` holds and what it names: its inputs, dates, claims and sources, each by its
/// naming; a date's definition is its block without the comments; the calendar a file reads.
#[test]
fn a_cal_names_its_items_and_its_references() {
    let file = "examples/payment_20th_close_next_10th.ja.cal";
    let items = Engine.items(&root(), file).unwrap();
    let kinds: Vec<(&str, &str)> = items.iter().map(|i| (i.kind(), i.name())).collect();
    assert!(kinds.contains(&("input", "受領日")), "{kinds:?}");
    assert!(kinds.contains(&("date", "支払日")), "{kinds:?}");
    assert!(items.iter().any(|i| i.kind() == "claim"), "{kinds:?}");
    let pay = items.iter().find(|i| i.name() == "支払日").unwrap();
    assert!(pay.text.starts_with("date 支払日"), "{}", pay.text);
    assert!(pay.lines.1 > pay.lines.0, "{:?}", pay.lines);
    assert!(!pay.text.contains('#'), "{}", pay.text);
    assert_eq!(pay.naming.text(), format!("koyomi \"{file}\" date 支払日"));
    let refs = Engine.references(&root(), file).unwrap();
    assert!(refs.iter().any(|r| r.how == "use calendar" && r.target.tool == ritsu_base::naming::Tool::Koyomi), "{refs:?}");
    // a calendar names the table it reads its holidays from
    let cal = Engine.references(&root(), "examples/calendars/東京の営業日.cal").unwrap();
    assert!(cal.iter().any(|r| r.how == "source" && r.target.tool == ritsu_base::naming::Tool::File), "{cal:?}");
    // every file of the examples: items within their lines, every naming reads back the same
    let mut all = Vec::new();
    ritsu_base::paths::walk(&root(), "examples", &[], &mut all);
    for f in all.iter().filter(|f| f.ends_with(".cal")) {
        let n = std::fs::read_to_string(root().join(f)).unwrap().lines().count();
        for it in Engine.items(&root(), f).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{f}: {it:?}");
            assert_eq!(ritsu_base::naming::parse_one(&it.naming.text()).map(|x| x.text()).ok(), Some(it.naming.text()), "{f}");
        }
        Engine.references(&root(), f).unwrap();
    }
}

/// What `a_dates_values_and_its_days_are_counted_exactly` counts, on the English version: the same
/// 23 days over the same 689 days of received, and on England and Wales, 35 days over 1,055.
#[test]
fn a_dates_values_and_its_days_are_counted_exactly_in_english() {
    let file = Path::new("examples/payment_20th_close_next_10th.cal");
    let Found::Value(set) = Engine.values(file, "payment").unwrap() else { panic!("undecided") };
    assert_eq!(set.len(), 23);
    assert_eq!(*set.first().unwrap(), day("2026-02-10"));
    assert_eq!(*set.last().unwrap(), day("2027-12-10"));
    assert_eq!(Engine.days(file, "payment").unwrap(), Found::Value((18, 51)));
    assert!(Engine.values(file, "no_such_date").is_err());
    let file = Path::new("examples/close_20th_pay_10th.cal");
    let Found::Value(set) = Engine.values(file, "payment").unwrap() else { panic!("undecided") };
    assert_eq!(set.len(), 35);
    assert_eq!(*set.first().unwrap(), day("2026-02-10"));
    assert_eq!(*set.last().unwrap(), day("2028-12-08"));
    for d in &set {
        let (_, _, dd) = koyomi::date::Day(*d as i32).ymd();
        assert!([8, 9, 10].contains(&dd), "{dd}");
    }
    assert_eq!(Engine.days(file, "payment").unwrap(), Found::Value((18, 51)));
}

#[test]
fn a_cal_names_its_items_and_its_references_in_english() {
    let file = "examples/payment_20th_close_next_10th.cal";
    let items = Engine.items(&root(), file).unwrap();
    let kinds: Vec<(&str, &str)> = items.iter().map(|i| (i.kind(), i.name())).collect();
    assert!(kinds.contains(&("input", "received")), "{kinds:?}");
    assert!(kinds.contains(&("date", "payment")), "{kinds:?}");
    assert!(items.iter().any(|i| i.kind() == "claim"), "{kinds:?}");
    let pay = items.iter().find(|i| i.name() == "payment").unwrap();
    assert!(pay.text.starts_with("date payment"), "{}", pay.text);
    assert!(!pay.text.contains('#'), "{}", pay.text);
    assert_eq!(pay.naming.text(), format!("koyomi \"{file}\" date payment"));
    let refs = Engine.references(&root(), file).unwrap();
    assert!(refs.iter().any(|r| r.how == "use calendar" && r.target.tool == ritsu_base::naming::Tool::Koyomi), "{refs:?}");
    for cal in ["examples/calendars/tokyo_business_days.cal", "examples/calendars/england_and_wales.cal"] {
        let refs = Engine.references(&root(), cal).unwrap();
        assert!(refs.iter().any(|r| r.how == "source" && r.target.tool == ritsu_base::naming::Tool::File), "{cal}: {refs:?}");
    }
}
