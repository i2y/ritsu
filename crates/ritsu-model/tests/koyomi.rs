//! koyomi's reference interpreter, held to `KoyomiModel` (DESIGN 11.3).
//!
//! Every `.cal` of koyomi's examples and test fixtures that resolves (the two examples that break
//! a claim on purpose included: their dates are still computed): each line of `koyomi vectors` —
//! every input of the range and the inputs just outside it, or every day a calendar knows and the
//! days just outside its data — is given by the reference interpreter and by the Lean model, and
//! the two answers are compared. The rows are made while they are compared, as koyomi's own
//! `tests/targets.rs` does, and none is left out. `-- --nocapture` shows a
//! `compared koyomi <file>: <n> lines` line for each file.

use koyomi::ast::{At, Conv};
use koyomi::calendar::{Calendar, Reason};
use koyomi::check::{Checked, check};
use koyomi::date::{Day, Missing};
use koyomi::resolve::{A, Model, ROp, Start};
use koyomi::vectors::{self, Expect, Row};
use ritsu_model::{compare, program};
use ritsu_testkit::{Need, TempDir, ready};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn koyomi_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../koyomi")
}

/// The `.cal` files of a directory: the English ones (ASCII names) first.
fn cals(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(koyomi_dir().join(dir))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "cal"))
        .collect();
    v.sort_by_key(|p| (!p.file_name().unwrap().to_string_lossy().is_ascii(), p.clone()));
    v
}

fn a(x: &A) -> Value {
    match x {
        A::Lit(n) => json!({"lit": n}),
        A::Input(k) => json!({"input": k}),
    }
}

fn missing(m: &Option<Missing>) -> Value {
    match m {
        Some(Missing::EndOfMonth) => json!("end_of_month"),
        Some(Missing::StartOfNextMonth) => json!("start_of_next_month"),
        Some(Missing::Reject) => json!("reject"),
        Some(Missing::Never) | None => Value::Null,
    }
}

fn op(o: &ROp) -> Value {
    match o {
        ROp::Days(s, n) => json!({"days": {"sign": s, "n": a(n)}}),
        ROp::Business(f, n) => json!({"business": {"forward": f, "n": a(n)}}),
        ROp::Months { sign, n, per, missing: m } => json!({"months": {"sign": sign, "n": a(n), "per": per, "missing": missing(m)}}),
        ROp::DayOfMonth { n, sign, k, missing: m } => json!({"day_of_month": {"n": a(n), "sign": sign, "k": a(k), "missing": missing(m)}}),
        ROp::StartOfMonth(s, k) => json!({"start_of_month": {"sign": s, "k": a(k)}}),
        ROp::EndOfMonth(s, k) => json!({"end_of_month": {"sign": s, "k": a(k)}}),
        ROp::CloseDay(n, m) => json!({"close_day": {"n": a(n), "missing": missing(m)}}),
        ROp::CloseEndOfMonth => json!({"close_end_of_month": true}),
        ROp::Roll(c) => json!({"roll": match c {
            Conv::Following => "following",
            Conv::Preceding => "preceding",
            Conv::ModifiedFollowing => "modified_following",
            Conv::ModifiedPreceding => "modified_preceding",
        }}),
        ROp::IfClosed(inner) => json!({"if_closed": op(inner)}),
    }
}

/// A calendar as `KoyomiModel.readRules` reads it. The days its tables close are read back
/// through `reasons`, over its data: a calendar that reads no table has none.
fn calendar(c: &Calendar) -> Value {
    let span = |s: &koyomi::calendar::Stretch| json!([s.from.0, s.to.0]);
    let mut holidays = Vec::new();
    if !c.tables.is_empty() {
        for z in c.data.0.0..=c.data.1.0 {
            if c.reasons(Day(z)).iter().any(|r| matches!(r, Reason::Holiday { .. })) {
                holidays.push(z);
            }
        }
    }
    json!({
        "data": [c.data.0.0, c.data.1.0],
        "offset": c.offset,
        "weekly": c.weekly,
        "every": c.every.iter().map(|e| json!([e.from.0, e.from.1, e.to.0, e.to.1])).collect::<Vec<_>>(),
        "days": c.days.iter().map(span).collect::<Vec<_>>(),
        "opens": c.opens.iter().map(span).collect::<Vec<_>>(),
        "holidays": holidays,
    })
}

/// A dates file as `KoyomiModel.readFile` reads it.
fn dates(m: &Model) -> Value {
    json!({
        "kind": "dates",
        "inputs": m.inputs.iter().enumerate().map(|(k, i)| json!({"lo": i.lo, "hi": i.hi, "taken": m.dates.iter().any(|d| d.params.contains(&k))})).collect::<Vec<_>>(),
        "date_input": m.date_input,
        "dates": m.dates.iter().map(|d| json!({
            "start": match d.start { Start::Input => json!({"input": true}), Start::Date(s) => json!({"date": s}) },
            "ops": d.ops.iter().map(|o| op(&o.op)).collect::<Vec<_>>(),
            "at": d.at.map(|(at, _)| vectors::at_minutes(at)),
        })).collect::<Vec<_>>(),
        "order": m.order,
        "calendar": m.cal.as_ref().map(calendar),
    })
}

/// An answer as `KoyomiModel.answerLine` writes it.
fn answer(e: &Expect) -> String {
    match e {
        Expect::Values(vs) => format!("[{}]", vs.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(",")),
        Expect::Open(o) => format!("{{\"open\":{o}}}"),
        Expect::Error(k) => format!("{{\"error\":\"{k}\"}}"),
    }
}

fn input(r: &Row) -> String {
    format!("[{}]", r.vals.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","))
}

#[test]
fn every_line_of_every_vectors_comes_out_as_the_model_says() {
    let bin = program();
    if !ready(Need::Lean, || bin.exists(), "proofs/ is not built (lake build makes ritsu-model)") {
        return;
    }
    let _ = At::EndOfDay;
    let tmp = TempDir::new("model-koyomi");
    let files: Vec<PathBuf> = ["examples", "examples/calendars", "tests/fixtures", "tests/fixtures/calendars"].iter().flat_map(|d| cals(d)).collect();
    let (mut seen, mut lines) = (0, 0usize);
    for p in files {
        let shown = p.strip_prefix(koyomi_dir()).unwrap().display().to_string();
        let o = check(&p.to_string_lossy()).unwrap_or_else(|e| panic!("{shown}: {e}"));
        let Some(checked) = o.checked else {
            panic!("{shown} does not resolve");
        };
        let (json, rows): (Value, Box<dyn Iterator<Item = Row> + Send + '_>) = match &checked {
            Checked::Dates(m, _) => (dates(m), Box::new(vectors::DatesRows::new(m))),
            Checked::Calendar(c) => (json!({"kind": "calendar", "calendar": calendar(c)}), Box::new(vectors::calendar_rows(c))),
        };
        let file = tmp.write(&format!("{seen}.json"), json.to_string());
        let c = compare(&bin, "koyomi", &file, rows.map(|r| (input(&r), answer(&r.expect))), &|a, b| a == b).unwrap_or_else(|e| panic!("{shown}: {e}"));
        assert_eq!(c.differ, 0, "{}", c.report(&shown));
        println!("compared koyomi {shown}: {} lines", c.lines);
        seen += 1;
        lines += c.lines;
    }
    assert!(seen >= 19, "the files are fewer than they were: {seen}");
    assert!(lines >= 5_000_000, "the lines are fewer than they were: {lines}");
}
