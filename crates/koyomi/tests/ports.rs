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

/// The calendars of the examples, English first.
const CALENDARS: [&str; 5] = [
    "examples/calendars/england_and_wales.cal",
    "examples/calendars/tokyo_business_days.cal",
    "examples/calendars/civil_code_142_days.cal",
    "examples/calendars/東京の営業日.cal",
    "examples/calendars/民法142条の休日.cal",
];

/// A calendar's facts are what `koyomi api` says of it, and the days it closes are the days
/// `koyomi vectors` writes as not open. For England and Wales they are also, counted apart from
/// koyomi, every Saturday and Sunday and every day GOV.UK lists for the division in the copy the
/// calendar reads.
#[test]
fn a_calendar_is_what_koyomi_api_and_its_vectors_say() {
    use koyomi::vectors::Expect;
    for path in CALENDARS {
        let o = koyomi::check::check(path).unwrap();
        let Some(koyomi::check::Checked::Calendar(c)) = &o.checked else { panic!("{path} is not a calendar") };
        let api = koyomi::api::calendar_json(c, Path::new("examples/calendars"));
        let f = Engine.calendar(Path::new(path)).unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert_eq!(f.name, api["name"].as_str().unwrap(), "{path}");
        assert_eq!(Some(f.alias.as_str()), api["alias"].as_str(), "{path}");
        assert_eq!(f.data, (day(api["data_range"]["from"].as_str().unwrap()), day(api["data_range"]["to"].as_str().unwrap())), "{path}");
        assert_eq!(f.offset, c.offset, "{path}");
        assert_eq!(api["offset"].as_str().map(String::from), f.offset.map(koyomi::calendar::offset_text), "{path}");
        let closed: ritsu_ports::DaySet = koyomi::vectors::calendar_rows(c).filter(|r| matches!(r.expect, Expect::Open(false))).map(|r| r.vals[0]).collect();
        assert_eq!(f.closed, closed, "{path}");
        assert!(!f.closed.is_empty() && f.closed.iter().all(|d| f.data.0 <= *d && *d <= f.data.1), "{path}");
    }
    // England and Wales, counted apart: weekends, and the division's days of the copy
    let f = Engine.calendar(Path::new(CALENDARS[0])).unwrap();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string("examples/calendars/data/bank-holidays.json").unwrap()).unwrap();
    let holidays: ritsu_ports::DaySet = json["england-and-wales"]["events"].as_array().unwrap().iter().map(|e| day(e["date"].as_str().unwrap())).collect();
    // 1970-01-01 was a Thursday: Saturday and Sunday are 2 and 3 days on, in each week
    let want: ritsu_ports::DaySet = (f.data.0..=f.data.1).filter(|d| matches!(d.rem_euclid(7), 2 | 3) || holidays.contains(d)).collect();
    assert_eq!(f.closed, want);
    assert_eq!((ritsu_ports::day_text(f.data.0), ritsu_ports::day_text(f.data.1)), ("2019-01-01".to_string(), "2028-12-31".to_string()));
    // a dates file is no calendar, and a file that is not there is read by no one
    let said = Engine.calendar(Path::new("examples/net30.cal")).unwrap_err();
    assert!(said[0].message.en.contains("is a dates file, not a calendar") && !said[0].message.ja.is_empty(), "{said:?}");
    assert!(Engine.calendar(Path::new("examples/calendars/nowhere.cal")).is_err());
}

/// The page drawn through the port is the one `koyomi doc` draws: the Markdown of its golden files
/// in both languages, and the HTML the command prints, when asked to name the file as the command
/// does (by its file name). Asked to name it otherwise, only the file in the first line of the
/// stamp changes.
#[test]
fn the_page_is_the_one_koyomi_doc_draws() {
    use ritsu_base::text::Lang;
    let examples = [
        "examples/calendars/england_and_wales.cal",
        "examples/net30.cal",
        "examples/period_of_months_two_readings.cal",
        "examples/civil_code_period_end.cal",
        "examples/calendars/東京の営業日.cal",
        "examples/payment_20th_close_next_10th.ja.cal",
    ];
    for p in examples {
        let path = Path::new(p);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let md = Engine.doc(path, &name, false, lang).unwrap_or_else(|e| panic!("{p}: {e:?}"));
            let golden = std::fs::read_to_string(format!("tests/golden/doc/{stem}.{tag}.md")).unwrap();
            assert!(md == golden, "{p} ({tag}): the Markdown is not the golden page");
            let html = Engine.doc(path, &name, true, lang).unwrap();
            let out = std::process::Command::new(env!("CARGO_BIN_EXE_koyomi")).args(["doc", p, "--format", "html", "--lang", tag]).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap();
            assert!(out.status.success(), "{p}");
            assert!(html == String::from_utf8(out.stdout).unwrap(), "{p} ({tag}): the HTML is not what `koyomi doc` prints");
            // named as a caller from elsewhere reaches it
            let shown = format!("calendars/{name}");
            let other = Engine.doc(path, &shown, false, lang).unwrap();
            assert!(other.contains(&format!("`{shown}`")) && other.replacen(&shown, &name, 1) == golden, "{p} ({tag})");
        }
    }
    // a file with an error other than a claim that fails has no page: what check says comes back
    let dir = ritsu_testkit::TempDir::new("koyomi-port-doc");
    let bad = dir.write("bad.cal", "dates broken v1\n\ninputs\n  invoice_date : date  range >=2026-01-01 <=2026-12-31\n\ndate due = invoice_date\n  + 1 fortnight\n");
    let said = Engine.doc(&bad, "bad.cal", false, Lang::En).unwrap_err();
    assert!(!said.is_empty() && said.iter().all(|s| !s.code.is_empty()), "{said:?}");
}

/// The three files of the example of Net 30 (the dates file, the calendar it uses and the table of
/// holidays that calendar reads), laid out as the example lays them out.
const NET30: [&str; 3] = ["net30.cal", "calendars/england_and_wales.cal", "calendars/data/bank-holidays.json"];

fn due_of(file: &Path, input: &str, day: i64) -> i64 {
    match Engine.eval(file, &[(input.to_string(), day)]).unwrap().into_iter().find(|(n, _)| n == "due") {
        Some((_, DateValue::Day(d))) => d,
        other => panic!("{other:?}"),
    }
}

/// A file the ports are asked about many times is checked once, and its check is kept while every
/// file that check read reads the same: the answers are the ones a check of its own gives (on a
/// thread that keeps nothing). A calendar it uses closing one more day, and the table of holidays
/// that calendar reads changing, each make it checked again.
#[test]
fn a_file_is_checked_once_while_nothing_it_read_changes() {
    let dir = ritsu_testkit::TempDir::new("koyomi-port-kept");
    for f in NET30 {
        dir.write(f, std::fs::read(root().join("examples").join(f)).unwrap());
    }
    let file = dir.path().join("net30.cal");
    let before = Engine.checks();
    let f = Engine.facts(&file).unwrap();
    let input = f.inputs[0].clone();
    let days: Vec<i64> = (input.min..input.min + 200).collect();
    let answers: Vec<_> = days.iter().map(|d| Engine.eval(&file, &[(input.name.clone(), *d)]).unwrap()).collect();
    assert!(matches!(Engine.values(&file, "due").unwrap(), Found::Value(_)));
    Engine.days(&file, "due").unwrap();
    Engine.doc(&file, "net30.cal", false, ritsu_base::text::Lang::En).unwrap();
    assert_eq!(Engine.checks() - before, 1, "one check for every question about the same file");
    let fresh = {
        let (file, input, days) = (file.clone(), input.clone(), days.clone());
        std::thread::spawn(move || {
            let answers: Vec<_> = days.iter().map(|d| Engine.eval(&file, &[(input.name.clone(), *d)]).unwrap()).collect();
            (answers, Engine.checks())
        })
        .join()
        .unwrap()
    };
    assert_eq!(fresh, (answers, 1), "a thread that keeps nothing checks once and answers the same");

    // the calendar closes the day the first invoice falls due: checked again, and it falls due later
    let first = due_of(&file, &input.name, input.min);
    let cal = dir.path().join(NET30[1]);
    let text = std::fs::read_to_string(&cal).unwrap();
    std::fs::write(&cal, format!("{text}closed {} \"A day closed for the test\"\n", ritsu_ports::day_text(first))).unwrap();
    let before = Engine.checks();
    assert!(due_of(&file, &input.name, input.min) > first);
    assert_eq!(Engine.checks() - before, 1);
    // the table of holidays changes under its pin: the calendar no longer passes, nor the file
    let table = dir.path().join(NET30[2]);
    let mut json = std::fs::read(&table).unwrap();
    json.push(b'\n');
    std::fs::write(&table, json).unwrap();
    let said = Engine.facts(&file).unwrap_err();
    assert!(!said.is_empty() && said.iter().all(|s| !s.code.is_empty()), "{said:?}");
    assert_eq!(Engine.checks() - before, 2);
}

/// The check kept is the one of what the thread reads: files in memory (what a page in the browser
/// hands over) are checked from memory and checked again when the memory holds another text, and
/// the same path on the disk, where nothing is, is not answered from memory.
#[test]
fn a_kept_check_is_of_what_the_thread_reads() {
    let mem = std::rc::Rc::new(ritsu_base::fs::Memory::new("/project"));
    for f in NET30 {
        mem.add(f, std::fs::read(root().join("examples").join(f)).unwrap());
    }
    let file = Path::new("/project/net30.cal");
    let a = ritsu_base::fs::with(mem.clone(), || Engine.facts(file)).unwrap();
    let again = ritsu_base::fs::with(mem.clone(), || Engine.facts(file)).unwrap();
    assert_eq!(a, again);
    let text = String::from_utf8(std::fs::read(root().join("examples").join(NET30[0])).unwrap()).unwrap();
    mem.add(NET30[0], text.replace("Net 30: due 30 days", "Net 30, as held in memory: due 30 days"));
    let b = ritsu_base::fs::with(mem.clone(), || Engine.facts(file)).unwrap();
    assert_ne!(a.sha256, b.sha256);
    assert!(Engine.facts(file).is_err(), "there is no /project/net30.cal on the disk");
}
