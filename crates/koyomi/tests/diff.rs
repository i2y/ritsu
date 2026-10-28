//! `koyomi diff` (ritsu's DESIGN 17.9.3, PLAN 8.3 C.2), on the materials of `tests/diff/`: an office
//! calendar with a table of days off, and a dates file that pays on the 10th of the next month on that
//! calendar, each with versions beside it (English first, the Japanese versions with Japanese names).
//!
//! - Golden files of the three formats (`tests/golden/diff/`; `KOYOMI_BLESS=1` writes them): a
//!   calendar whose version adds a day off, renames one, knows another year and gains an offset, and
//!   dates files whose version reads that calendar, takes fewer receipts and adds a date.
//! - Each change alone, and what stops a diff (exit 2): a side that does not pass check, two of
//!   different kinds, more combinations than `--budget`.
//! - Brute force: for every pair of versions of a kind, each day (each combination of the inputs) is
//!   computed by both versions one at a time, and what differs is held to the intervals of the diff
//!   undone. The number of days and combinations compared is printed (`--nocapture`).

use koyomi::calendar::Calendar;
use koyomi::check::Checked;
use koyomi::date::Day;
use koyomi::diff::{self, Diff, Side, Val};
use koyomi::resolve::Model;
use koyomi::vectors::{self, Expect};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/diff")
}

fn koyomi(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).current_dir(dir()).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

/// The names of the materials in one language.
struct Names {
    lang: &'static str,
    office: &'static str,
    added: &'static str,
    renamed: &'static str,
    longer: &'static str,
    offset: &'static str,
    office_v2: &'static str,
    payment: &'static str,
    later: &'static str,
    narrow: &'static str,
    by_month: &'static str,
    broken: &'static str,
    payment_v2: &'static str,
    by_day: &'static str,
    by_day_later: &'static str,
    /// The words the JSON writes: the date input, the date that changes, the day off added and the
    /// day renamed (old and new).
    received: &'static str,
    paid: &'static str,
    founding: &'static str,
    renamed_from: &'static str,
    renamed_to: &'static str,
    month_input: &'static str,
}

const EN: Names = Names {
    lang: "en",
    office: "office.cal",
    added: "office_added.cal",
    renamed: "office_renamed.cal",
    longer: "office_longer.cal",
    offset: "office_offset.cal",
    office_v2: "office_v2.cal",
    payment: "payment.cal",
    later: "payment_later.cal",
    narrow: "payment_narrow.cal",
    by_month: "payment_by_month.cal",
    broken: "payment_broken.cal",
    payment_v2: "payment_v2.cal",
    by_day: "payment_by_day.cal",
    by_day_later: "payment_by_day_later.cal",
    received: "received",
    paid: "payment",
    founding: "Founding day",
    renamed_from: "Early May bank holiday",
    renamed_to: "May Day",
    month_input: "payment_month",
};

const JA: Names = Names {
    lang: "ja",
    office: "事務所.cal",
    added: "事務所_休みを足す.cal",
    renamed: "事務所_名前を変える.cal",
    longer: "事務所_延ばす.cal",
    offset: "事務所_時差.cal",
    office_v2: "事務所_二版.cal",
    payment: "支払日.cal",
    later: "支払日_休みを足す.cal",
    narrow: "支払日_範囲を狭める.cal",
    by_month: "支払日_月を受け取る.cal",
    broken: "支払日_例が合わない.cal",
    payment_v2: "支払日_二版.cal",
    by_day: "支払日_日を受け取る.cal",
    by_day_later: "支払日_日を受け取る_休みを足す.cal",
    received: "受領日",
    paid: "支払日",
    founding: "創立記念日",
    renamed_from: "みどりの日",
    renamed_to: "春の休み",
    month_input: "支払の月",
};

/// The three formats of three diffs in each language, held to golden files.
#[test]
fn the_diffs_held_to_golden_files() {
    let mut failures = Vec::new();
    for n in [&EN, &JA] {
        for (old, new, name) in [(n.office, n.office_v2, "calendar"), (n.payment, n.payment_v2, "dates"), (n.by_day, n.by_day_later, "dates_by_day")] {
            for (format, ext) in [("text", "txt"), ("markdown", "md"), ("json", "json")] {
                let (code, out, err) = koyomi(&["diff", old, new, "--format", format, "--lang", n.lang]);
                assert_eq!((code, err.as_str()), (1, ""), "{old} {new} {format}: {out}");
                let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/diff/{name}.{}.{ext}", n.lang));
                if let Err(e) = ritsu_testkit::golden::check(&golden, &out) {
                    failures.push(e);
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Each change alone: a day off added, a day renamed, a year more known, an offset, the same file;
/// a payment a day earlier, receipts taken up to June, an input only one version takes.
#[test]
fn each_change_alone() {
    for n in [&EN, &JA] {
        let diff_json = |old: &str, new: &str| -> (i32, serde_json::Value) {
            let (code, out, err) = koyomi(&["diff", old, new, "--format", "json", "--lang", n.lang]);
            assert!(err.is_empty(), "{old} {new}: {err}");
            (code, json(&out))
        };
        let (code, j) = diff_json(n.office, n.added);
        assert_eq!(code, 1, "{}", n.lang);
        assert_eq!(
            j["changes"],
            serde_json::json!([{"from": "2026-06-10", "to": "2026-06-10", "days": 1, "old": "open", "new": "closed", "old_reasons": [], "new_reasons": [n.founding]}])
        );
        assert_eq!((j["renamed"].as_array().map(Vec::len), &j["offset"], j["total"]["changed_days"].as_u64()), (Some(0), &serde_json::Value::Null, Some(1)));

        let (code, j) = diff_json(n.office, n.renamed);
        assert_eq!(code, 1, "{}", n.lang);
        assert_eq!(j["changes"], serde_json::json!([]));
        assert_eq!(j["renamed"], serde_json::json!([{"from": "2026-05-04", "to": "2026-05-04", "old_reasons": [n.renamed_from], "new_reasons": [n.renamed_to]}]));

        let (code, j) = diff_json(n.office, n.longer);
        assert_eq!(code, 1, "{}", n.lang);
        assert_eq!((j["changes"].as_array().map(Vec::len), j["renamed"].as_array().map(Vec::len)), (Some(0), Some(0)));
        assert_eq!(j["compared"], serde_json::json!({"from": "2026-01-01", "to": "2026-12-31", "days": 365}));
        assert_eq!(j["known"], serde_json::json!({"old": {"from": "2026-01-01", "to": "2026-12-31"}, "new": {"from": "2026-01-01", "to": "2027-12-31"}}));

        let (code, j) = diff_json(n.office, n.offset);
        assert_eq!(code, 1, "{}", n.lang);
        let offset = if n.lang == "en" { "+01:00" } else { "+09:00" };
        assert_eq!(j["offset"], serde_json::json!({"old": "none", "new": offset}));

        let (code, j) = diff_json(n.office, n.office);
        assert_eq!((code, &j["same_elsewhere"], j["total"]["changed_days"].as_u64()), (0, &serde_json::json!(true), Some(0)));

        let (code, j) = diff_json(n.payment, n.later);
        assert_eq!(code, 1, "{}", n.lang);
        assert_eq!(
            j["changes"],
            serde_json::json!([{"date": n.paid, "inputs": {n.received: {"from": "2026-04-21", "to": "2026-05-20"}},
                                "old": {"from": "2026-06-10", "to": "2026-06-10"}, "new": {"from": "2026-06-09", "to": "2026-06-09"}, "shift_days": -1, "combinations": 30}])
        );
        assert_eq!((j["comparable"].as_bool(), j["combinations"].as_u64(), j["total"]["combinations_changed"].as_u64()), (Some(true), Some(304), Some(30)));

        let (code, j) = diff_json(n.payment, n.narrow);
        assert_eq!(code, 1, "{}", n.lang);
        assert_eq!(j["changes"], serde_json::json!([]));
        let r = &j["inputs"]["compared"][0];
        assert_eq!((&r["to"], &r["old"]["to"], &r["new"]["to"]), (&serde_json::json!("2026-06-30"), &serde_json::json!("2026-10-31"), &serde_json::json!("2026-06-30")));

        let (code, j) = diff_json(n.payment, n.by_month);
        assert_eq!(code, 1, "{}", n.lang);
        assert_eq!((j["comparable"].as_bool(), j["combinations"].as_u64()), (Some(false), Some(0)));
        assert_eq!(j["inputs"]["only_new"], serde_json::json!([{"name": n.month_input, "type": "int"}]));
        assert_eq!(j["changes"], serde_json::json!([]));

        let (code, j) = diff_json(n.payment, n.payment);
        assert_eq!((code, j["total"]["combinations_changed"].as_u64()), (0, Some(0)));
    }
}

/// What stops a diff (exit 2), in the words of each language.
#[test]
fn what_stops_a_diff() {
    let (code, out, err) = koyomi(&["diff", EN.payment, EN.later, "--budget", "10"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: the inputs both versions take have 304 combinations, over the budget of 10; give a larger `--budget`\n");
    let (code, _, err) = koyomi(&["diff", JA.payment, JA.later, "--budget", "10", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: 両方の版が受け取る入力の組み合わせは 304 通りで、上限 10 を超えます。`--budget` を大きくしてください\n"));
    let (code, _, err) = koyomi(&["diff", EN.payment, EN.broken]);
    assert_eq!((code, err.as_str()), (2, "error: `payment_broken.cal` does not pass check (1 error); run `koyomi check` on it\n"));
    let (code, _, err) = koyomi(&["diff", JA.broken, JA.payment, "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `支払日_例が合わない.cal` は検査を通りません（エラー 1 件）。`koyomi check` で確かめてください\n"));
    let (code, _, err) = koyomi(&["diff", EN.payment, EN.office]);
    assert_eq!((code, err.as_str()), (2, "error: `payment.cal` and `office.cal` are of different kinds (a calendar and a dates file); a diff compares two of a kind\n"));
    let (code, _, err) = koyomi(&["diff", EN.payment]);
    assert_eq!((code, err.as_str()), (2, "error: `koyomi diff` takes two sides, the old and the new\n"));
    let (code, _, err) = koyomi(&["diff", "no_such.cal", EN.payment]);
    assert_eq!(code, 2, "{err}");
    assert!(err.starts_with("error: cannot read `no_such.cal`"), "{err}");
}

/// A material read and checked, shown by its name.
fn checked(name: &str) -> Side {
    let path = dir().join(name);
    diff::side(&path.to_string_lossy(), name, None, koyomi::check::DEFAULT_BUDGET).unwrap_or_else(|t| panic!("{}", t.en))
}

fn calendar(s: &Side) -> &Calendar {
    match &s.outcome.checked {
        Some(Checked::Calendar(c)) => c,
        _ => panic!("{} is no calendar", s.shown),
    }
}

fn model(s: &Side) -> &Model {
    match &s.outcome.checked {
        Some(Checked::Dates(m, _)) => m,
        _ => panic!("{} is no dates file", s.shown),
    }
}

/// Every day both calendars know, compared one at a time: what differs, by the day.
fn calendar_by_brute_force(o: &Calendar, n: &Calendar) -> (u64, BTreeMap<i32, String>) {
    let (from, to) = (o.data.0.max(n.data.0), o.data.1.min(n.data.1));
    let mut out = BTreeMap::new();
    let mut days = 0;
    for z in from.0..=to.0 {
        days += 1;
        let d = Day(z);
        let (a, b) = (o.is_open(d).unwrap(), n.is_open(d).unwrap());
        // why a day is closed; an open day has no reason
        let why = |c: &Calendar, open: bool| if open { Vec::new() } else { c.reasons(d) };
        let (ra, rb) = (why(o, a), why(n, b));
        if a != b || ra != rb {
            out.insert(z, format!("{a} {ra:?} -> {b} {rb:?}"));
        }
    }
    (days, out)
}

/// The same, from the intervals of the diff, undone.
fn calendar_undone(d: &diff::CalDiff) -> BTreeMap<i32, String> {
    let mut out = BTreeMap::new();
    for c in &d.changes {
        for z in c.from.0..=c.to.0 {
            assert!(out.insert(z, format!("{} {:?} -> {} {:?}", c.old, c.old_reasons, c.new, c.new_reasons)).is_none(), "a day in two intervals");
        }
    }
    for r in &d.renamed {
        for z in r.from.0..=r.to.0 {
            assert!(out.insert(z, format!("false {:?} -> false {:?}", r.old_reasons, r.new_reasons)).is_none(), "a day in two intervals");
        }
    }
    out
}

fn val(e: &Expect, out: &[Day], m: &Model, k: usize) -> Val {
    match e {
        Expect::Values(vs) => {
            // the place of the date's value among the values, and its time after it
            let timed = m.cal.as_ref().and_then(|c| c.offset).is_some();
            let mut at = 0;
            for d in &m.dates[..k] {
                at += if d.at.is_some() && timed { 2 } else { 1 };
            }
            Val::Day(out[k], (m.dates[k].at.is_some() && timed).then(|| vs[at + 1].clone()))
        }
        Expect::Error(kind) => Val::Stop(kind),
        Expect::Open(_) => unreachable!(),
    }
}

/// Every combination of the inputs both dates files take, computed one at a time by both: what
/// differs, by the date and the combination (the inputs in the new file's order).
fn dates_by_brute_force(o: &Model, n: &Model) -> (u64, BTreeMap<(String, Vec<i64>), (Val, Val)>) {
    let axes: Vec<(String, i64, i64)> = n
        .inputs
        .iter()
        .map(|i| {
            let x = o.inputs.iter().find(|x| x.name == i.name).unwrap();
            (i.name.clone(), x.lo.max(i.lo), x.hi.min(i.hi))
        })
        .collect();
    let mut out = BTreeMap::new();
    let mut combos = 0;
    let mut vals: Vec<i64> = axes.iter().map(|a| a.1).collect();
    'all: loop {
        combos += 1;
        let pick = |m: &Model| -> Vec<i64> { m.inputs.iter().map(|i| vals[axes.iter().position(|a| a.0 == i.name).unwrap()]).collect() };
        let (vo, vn) = (pick(o), pick(n));
        let (mut out_o, mut out_n) = (vec![Day(0); o.dates.len()], vec![Day(0); n.dates.len()]);
        let (eo, en) = (vectors::expect(o, &vo, &mut out_o), vectors::expect(n, &vn, &mut out_n));
        for (kn, d) in n.dates.iter().enumerate() {
            let Some(ko) = o.dates.iter().position(|x| x.name == d.name) else { continue };
            let (a, b) = (val(&eo, &out_o, o, ko), val(&en, &out_n, n, kn));
            if a != b {
                out.insert((d.name.clone(), vals.clone()), (a, b));
            }
        }
        for k in (0..vals.len()).rev() {
            if vals[k] < axes[k].2 {
                vals[k] += 1;
                continue 'all;
            }
            vals[k] = axes[k].1;
        }
        break;
    }
    (combos, out)
}

/// The same, from the runs of the diff, undone: each combination of a run, with the run's shift;
/// the values at its two ends.
fn dates_undone(d: &diff::DatesDiff, n: &Model, brute: &BTreeMap<(String, Vec<i64>), (Val, Val)>) -> Vec<(String, Vec<i64>)> {
    let da = d.date_axis.unwrap();
    let mut out = Vec::new();
    for c in &d.changes {
        for z in c.first.0..=c.first.1 {
            let mut vals = vec![0i64; n.inputs.len()];
            for (k, a) in d.axes.iter().enumerate() {
                let at = n.inputs.iter().position(|i| i.name == a.name).unwrap();
                vals[at] = if k == da { z } else { c.others.iter().find(|(x, _)| *x == k).unwrap().1 };
            }
            let key = (c.date.clone(), vals);
            let (a, b) = brute.get(&key).unwrap_or_else(|| panic!("{key:?} is in a run of the diff, and the same in both versions"));
            let shift = match (a, b) {
                (Val::Day(x, _), Val::Day(y, _)) => Some((y.0 - x.0) as i64),
                _ => None,
            };
            assert_eq!(shift, c.shift, "{key:?}");
            if z == c.first.0 {
                assert_eq!((a, b), (&c.old.0, &c.new.0), "{key:?}");
            }
            if z == c.first.1 {
                assert_eq!((a, b), (&c.old.1, &c.new.1), "{key:?}");
            }
            out.push(key);
        }
    }
    out
}

/// Every pair of versions of a kind, in both languages: the diff undone is what the brute force
/// finds, day by day and combination by combination.
#[test]
fn the_diffs_are_what_the_brute_force_finds() {
    let (mut days, mut combos, mut pairs) = (0u64, 0u64, 0u64);
    for n in [&EN, &JA] {
        let calendars = [n.office, n.added, n.renamed, n.longer, n.offset, n.office_v2];
        for a in calendars {
            for b in calendars {
                let (o, x) = (checked(a), checked(b));
                let Ok(Diff::Calendar(d)) = diff::diff(&o, &x, koyomi::check::DEFAULT_BUDGET) else { panic!("{a} {b}") };
                let (k, brute) = calendar_by_brute_force(calendar(&o), calendar(&x));
                assert_eq!(calendar_undone(&d), brute, "{a} → {b}");
                days += k;
                pairs += 1;
            }
        }
        let dates = [n.payment, n.later, n.narrow, n.payment_v2];
        let by_day = [n.by_day, n.by_day_later];
        for group in [&dates[..], &by_day[..]] {
            for a in group {
                for b in group {
                    let (o, x) = (checked(a), checked(b));
                    let Ok(Diff::Dates(d)) = diff::diff(&o, &x, koyomi::check::DEFAULT_BUDGET) else { panic!("{a} {b}") };
                    let (k, brute) = dates_by_brute_force(model(&o), model(&x));
                    let mut undone = dates_undone(&d, model(&x), &brute);
                    undone.sort();
                    let mut found: Vec<(String, Vec<i64>)> = brute.keys().cloned().collect();
                    found.sort();
                    assert_eq!(undone, found, "{a} → {b}");
                    assert_eq!(d.combinations, k as u128, "{a} → {b}");
                    combos += k;
                    pairs += 1;
                }
            }
        }
    }
    eprintln!("koyomi diff, brute force: {pairs} pairs, {days} days of calendars and {combos} combinations of dates files, each computed by both versions");
}

/// A side named with a git revision, read from git with the calendar it uses at that revision.
#[test]
fn a_side_at_a_git_revision() {
    if Command::new("git").arg("--version").output().is_err() {
        ritsu_testkit::skip("git is not on the PATH; a side at a revision is read with it");
        return;
    }
    let t = ritsu_testkit::TempDir::new("koyomi-diff-git");
    let d = t.path();
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .arg("-C")
            .arg(d)
            .args(["-c", "user.name=koyomi-test", "-c", "user.email=koyomi-test@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    std::fs::create_dir_all(d.join("data")).unwrap();
    for f in ["office.cal", "payment.cal", "data/days_off.csv", "data/days_off_added.csv"] {
        std::fs::copy(dir().join(f), d.join(f)).unwrap();
    }
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "office"]);
    // the calendar of the working tree adds the founding day; the dates file is the same file
    std::fs::copy(dir().join("office_added.cal"), d.join("office.cal")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(["diff", "payment.cal@HEAD", "payment.cal", "--format", "json"]).current_dir(d).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    let j = json(&String::from_utf8_lossy(&o.stdout));
    assert_eq!(j["total"]["combinations_changed"].as_u64(), Some(30));
    assert_eq!((j["old"]["file"].as_str(), j["new"]["file"].as_str()), (Some("payment.cal@HEAD"), Some("payment.cal")));
    let o = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(["diff", "payment.cal@no-such-revision", "payment.cal"]).current_dir(d).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&o.stderr), "error: the git revision `no-such-revision` cannot be resolved\n");
}
