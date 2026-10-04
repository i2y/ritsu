//! The checks across the languages (`ritsu_cross`), held to `RitsuCross` (DESIGN 11.2, 11.3).
//!
//! Each check is fed inputs the tests make, from the languages' own answers wherever there are
//! answers to take, and the Rust decision and the Lean model's are compared a line at a time:
//!
//! - X2, rulec's answer (`Rules::preconditions_hold`): rules with a `constraint` between two inputs,
//!   in each of the four comparisons and over inputs of one scale and of two (a rate in 0.1% steps
//!   against one in 1% steps, a rate against a number), asked about every pair of ranges a grid of
//!   ends makes, open ends and empty ranges among them.
//! - X2 at the call (`ritsu check`): small projects whose flow calls a rule with a `constraint`, or
//!   with an input whose range is a koyomi date, checked as `ritsu check` checks them; the values
//!   the call gives are what dandori's port says (`Flows::crossings`), and the answer is what
//!   `ritsu check` says of the precondition (E201 with its example, W201, or nothing).
//! - X3 (a): every date of every dates file of koyomi's examples and fixtures that passes its check
//!   (the port answers for no other: the examples that break a claim on purpose are left out) —
//!   the set of days koyomi's port hands over (`Dates::values`) against the set the model computes
//!   from the same file, and `days_fit` and `days_given` over the sets against ranges around them.
//! - X3 (b): the days a rulec certificate carries for an input whose range is a koyomi date
//!   (`range from koyomi`) against the set the model computes from the koyomi file.
//! - X4: `amount_fits`, `amounts_given` and `amounts_hull` over every numeric output of the rules of
//!   rulec's corpus and ranges around them, and over outputs written to reach every branch;
//!   `refusals_met` over what chobo's search finds for every transfer of chobo's examples and test
//!   books (as its check finds them, and for one transfer with the amounts held to a range),
//!   against handled sets made from it and the reasons of each book's bounds.
//! - X5: `held_until` over a grid of the fewest and most seconds a call comes after a hold and the
//!   seconds the hold expires after, around each edge.
//! - X6: `input_range` over every input of every dates file, and `days_given` of the days of each
//!   koyomi date against each date input's range.
//!
//! `-- --nocapture` shows a `compared cross <what>: <n> lines` line for each.

mod common;

use koyomi::check::{Checked, DEFAULT_BUDGET, check};
use ritsu_base::text::Lang;
use ritsu_cross::borders::{amount_fits, amounts_given, amounts_hull, days_fit, days_given, held_until, input_range, refusals_met, AmountFrom};
use ritsu_model::{compare, program};
use ritsu_ports::{Answer, Books, DateKind, Dates, Flows, Found, Origin, OutputValues, Precondition, Rules, Value as Wire, Values};
use ritsu_testkit::{Need, TempDir, ready};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The model's program, when the level lets the tests run it and `lake build` has made it.
fn model() -> Option<PathBuf> {
    let bin = program();
    ready(Need::Lean, || bin.exists(), "proofs/ is not built (lake build makes ritsu-model)").then_some(bin)
}

/// `rows` — each an input line and the Rust answer — through `ritsu-model cross <file>`, where
/// `file` is what the lines share; the number of lines compared.
fn run(bin: &Path, tmp: &TempDir, file: &Value, what: &str, rows: Vec<(String, String)>) -> usize {
    let path = tmp.write(&format!("{}.json", what.replace(['/', ' '], "_")), file.to_string());
    let c = compare(bin, "cross", &path, rows.into_iter(), &|a, b| a == b).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(c.differ, 0, "{}", c.report(what));
    println!("compared cross {what}: {} lines", c.lines);
    c.lines
}

/// An answer as `RitsuCross.answerJ` writes it.
fn answer<E>(a: &Answer<E>, fails: impl Fn(&E) -> Value) -> String {
    match a {
        Answer::Holds => "\"holds\"".into(),
        Answer::Undecided(_) => "\"undecided\"".into(),
        Answer::Fails(e) => json!({ "fails": fails(e) }).to_string(),
    }
}

fn end(x: Option<i128>) -> Value {
    x.map_or(Value::Null, |v| json!(v))
}

fn wire_int(v: &Wire) -> i128 {
    match v {
        Wire::Int(n) => *n,
        other => panic!("not an integer on the wire: {other:?}"),
    }
}

/// Values, as the model hands an example on: each name with how the value reads.
fn values_json(vs: &Values) -> Value {
    Value::Array(vs.iter().map(|(n, v)| json!([n, format!("{v:?}")])).collect())
}

// ── X2: rulec's answer ─────────────────────────────────────────────────────────────────────────

const OPS: &[&str] = &["<=", "<", ">=", ">"];

/// The two inputs of a relation: how each is declared, its declared range, a cell that splits it,
/// and the scale it travels at.
const SIDES: &[((&str, &str, &str, u32), (&str, &str, &str, u32))] = &[
    (("number", ">=-1000 <=1000", "0", 1), ("number", ">=-1000 <=1000", "0", 1)),
    (("rate[step 0.1%]", ">=0% <=100%", "50%", 1000), ("rate[step 1%]", ">=0% <=100%", "50%", 100)),
    (("rate[step 1%]", ">=0% <=100%", "50%", 100), ("number", ">=0 <=1000", "0", 1)),
];

fn relation_rule(op: &str, a: (&str, &str, &str, u32), b: (&str, &str, &str, u32)) -> String {
    format!(
        "rule pre v1\n\nenum verdict = low | high\n\ninputs\n  a : {}  range {}\n  b : {}  range {}\n\nconstraint a {op} b\n\noutputs\n  r : verdict\n\ntable pick\npolicy unique\n| a | -> r : verdict |\n| <={} | low |\n| >{} | high |\n",
        a.0, a.1, b.0, b.1, a.2, a.2
    )
}

#[test]
fn x2_rulecs_answer_over_two_ranges_is_the_models() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-corner");
    let grid: Vec<Option<i128>> = vec![None, Some(-7), Some(0), Some(1), Some(3), Some(10), Some(100), Some(1000)];
    let ranges: Vec<(Option<i128>, Option<i128>)> = grid.iter().flat_map(|lo| grid.iter().map(move |hi| (*lo, *hi))).collect();
    let engine = rulec::ports::Engine::new();
    let mut lines = 0;
    for (si, (a, b)) in SIDES.iter().enumerate() {
        for op in OPS {
            let rule = tmp.write(&format!("pre{si}{}.rule", op.replace('<', "l").replace('>', "g").replace('=', "e")), relation_rule(op, *a, *b));
            let mut rows = Vec::new();
            for l in &ranges {
                for r in &ranges {
                    let asked = vec![("a".to_string(), l.0, l.1), ("b".to_string(), r.0, r.1)];
                    let got = engine.preconditions_hold(&rule, &asked, None).unwrap_or_else(|e| panic!("{e:?}"));
                    let (_, said) = got.iter().find(|(p, _)| matches!(p, Precondition::Relation { .. })).expect("the relation is a precondition");
                    let line = json!({"corner": {"op": op, "left": [end(l.0), end(l.1)], "right": [end(r.0), end(r.1)], "scales": [a.3, b.3]}});
                    rows.push((line.to_string(), answer(said, |vs: &Values| json!([wire_int(&vs[0].1), wire_int(&vs[1].1)]))));
                }
            }
            lines += run(&bin, &tmp, &json!({}), &format!("x2 corner {} {op} {}", a.0, b.0), rows);
        }
    }
    assert!(lines >= 40_000, "fewer lines than there were: {lines}");
}

// ── X2: the decision at the call ──────────────────────────────────────────────────────────────

const CHECK_RULE: &str = "rule check v1\n\nenum verdict = low | high\n\ninputs\n  a : number  range >=-1000 <=1000\n  b : number  range >=-1000 <=1000\n\nconstraint a OP b\n\noutputs\n  r : verdict\n\ntable pick\npolicy unique\n| a    | -> r : verdict |\n| <=0  | low            |\n| >0   | high           |\n";

const CALL_FLOW: &str = "workflow call v1\ndescription \"Calls the rule with two values\"\n\nuse rule check from \"check.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:check\"\n\ninputs\n  x : int  range XR\n  y : int  range YR\n\ntask open_value() -> int\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:open-value\"\n  idempotent\n\nflow\n  let v = open_value()\n  let d = check(a: A, b: B)\n  match d.r\n    low => pass\n    high => pass\n";

/// A day as the number of days since 1970-01-01, from `YYYY-MM-DD` (Howard Hinnant's
/// days_from_civil, as rulec and koyomi count days).
fn day_number(s: &str) -> i128 {
    let p: Vec<i64> = s.split('-').map(|x| x.parse().unwrap()).collect();
    let (y, m, d) = (p[0], p[1], p[2]);
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i128
}

/// The example of an E201 as its first note says it, as the model writes an `Example`.
fn example_of(note: &str) -> Value {
    if let Some(i) = note.find(" and can be ") {
        let rest = &note[i + " and can be ".len()..];
        return json!({ "day": day_number(&rest[..10]) });
    }
    if let Some(i) = note.find("(for one, ") {
        let rest = &note[i + "(for one, ".len()..];
        let v = rest.rsplit(" = ").next().unwrap().trim_end_matches(')');
        return json!({ "same": v.parse::<i128>().ok() });
    }
    if let Some(i) = note.find(", and at ") {
        let rest = &note[i + ", and at ".len()..];
        let at = &rest[..rest.find(" `").unwrap()];
        let vals: Vec<i128> = at.split(", ").map(|kv| kv.split(" = ").nth(1).unwrap().parse().unwrap()).collect();
        return json!({ "at": [vals[0], vals[1]] });
    }
    panic!("an E201 note that gives no example: {note}");
}

/// The kinds of place a value can come from, as `RitsuCross.Origin` reads them: the days koyomi
/// counts for each koyomi date it can be the day of, and `other` for the rest.
fn origins_json(joined: &ritsu_project::Joined, from: &[Origin]) -> Value {
    Value::Array(
        from.iter()
            .map(|o| match o {
                Origin::Day { file, date } => match joined.koyomi.values(file, date) {
                    Ok(Found::Value(set)) => json!({ "day": set.iter().collect::<Vec<_>>() }),
                    _ => json!({ "day": null }),
                },
                _ => json!("other"),
            })
            .collect(),
    )
}

fn arg_json(joined: &ritsu_project::Joined, a: Option<&ritsu_ports::CallArg>) -> Value {
    match a {
        None => Value::Null,
        Some(a) => json!({"shown": a.shown, "range": a.range.map_or(Value::Null, |(lo, hi)| json!([end(lo), end(hi)])), "origins": origins_json(joined, &a.from)}),
    }
}

/// The days file and the flows of the projects whose rule takes the days of a koyomi date.
const TERMS_CAL: &str = "dates payment_terms v1\ndescription \"Closes on the 20th and pays on the 10th of the next month\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20          # closes on the 20th\n\ndate payment = closing\n  day 10 of month +1    # pays on the 10th of the next month\n";

const DAYS_RULE: &str = "rule batch v1\ndescription \"The billing batch a payment day falls in\"\n\nenum run = spring | autumn\n\ninputs\n  pay_day : date  range from koyomi \"payment_terms.cal\" date DATE\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-08-31 | spring         |\n| >=2026-09-01 | autumn         |\n";

const DAYS_FLOW: &str = "workflow billing v1\ndescription \"Bills an order in the batch the rule picks for a day\"\n\nuse dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\nuse rule batch from \"batch.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:batch\"\n\ninputs\n  order    : string\n  received : date\n  at_once  : bool\n\ntask bill(order: string, due: date, run: batch.run)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:bill\"\n  key\n\nflow\n  let due = terms.payment(received: received)\n  let day = due.day\n  match at_once\n    true => let day = DAY\n    false => pass\n  let pick = batch(pay_day: day)\n  bill(order: order, due: day, run: pick.batch)\n";

/// What `ritsu check` says of the one precondition at the one call of a project: the E201 with its
/// example, the W201, or nothing, as `RitsuCross.answerJ` writes it.
fn said_of_call(crossed: &ritsu_cross::Crossed, what: &str) -> String {
    let e: Vec<_> = crossed.findings.iter().filter(|f| f.code == "E201").collect();
    let w = crossed.findings.iter().filter(|f| f.code == "W201").count();
    assert!(e.len() + w <= 1, "{what}: one precondition at one call says one thing");
    match e.first() {
        Some(f) => {
            let note = match &f.json {
                ritsu_base::json::Json::Obj(kv) => match kv.iter().find(|(k, _)| k == "notes").map(|(_, v)| v) {
                    Some(ritsu_base::json::Json::Arr(ns)) => ns[0].as_str().unwrap().to_string(),
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            };
            json!({ "fails": example_of(&note) }).to_string()
        }
        None if w == 1 => "\"undecided\"".to_string(),
        None => "\"holds\"".to_string(),
    }
}

#[test]
fn x2_at_the_call_is_the_models_decision() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-call");
    let closed = ["0 10", "0 5", "5 10", "3 3", "-5 0"];
    let mut cases: Vec<(String, String, String, String)> = Vec::new();
    for xr in closed {
        for yr in closed {
            cases.push((xr.into(), yr.into(), "x".into(), "y".into()));
        }
    }
    // one value to both inputs; a value from a task with no range
    for xr in ["0 10", "-5 0"] {
        cases.push((xr.into(), "0 10".into(), "x".into(), "x".into()));
    }
    cases.push(("0 10".into(), "0 10".into(), "v".into(), "y".into()));
    cases.push(("0 10".into(), "0 10".into(), "x".into(), "v".into()));
    let range = |r: &str| {
        let (lo, hi) = r.split_once(' ').unwrap();
        format!(">={lo} <={hi}")
    };
    let mut rows = Vec::new();
    let mut n = 0;
    let mut project = |files: Vec<(&str, String)>| -> std::path::PathBuf {
        let dir = tmp.path().join(format!("p{n}"));
        n += 1;
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        dir
    };
    for op in OPS {
        for (xr, yr, a, b) in &cases {
            let dir = project(vec![
                ("check.rule", CHECK_RULE.replace("OP", op)),
                ("call.flow", CALL_FLOW.replace("XR", &range(xr)).replace("YR", &range(yr)).replace("A,", &format!("{a},")).replace("B)", &format!("{b})"))),
            ]);
            let p = ritsu_project::Project::load(&[dir.to_string_lossy().to_string()], None).unwrap_or_else(|e| panic!("{e:?}"));
            let joined = ritsu_project::Joined::new();
            let calls = joined.dandori.crossings(&dir.join("call.flow"), &joined.ports()).unwrap_or_else(|e| panic!("the flow does not pass dandori's check: {e:?}")).rules;
            assert_eq!(calls.len(), 1);
            let arg = |name: &str| calls[0].args.iter().find(|x| x.input == name);
            let crossed = ritsu_cross::check(&p, &joined, Lang::En);
            let line = json!({"x2": {"kind": "relation", "op": op, "left": arg_json(&joined, arg("a")), "right": arg_json(&joined, arg("b")), "scales": [1, 1]}});
            rows.push((line.to_string(), said_of_call(&crossed, &format!("{op} {xr} {yr} {a} {b}"))));
        }
    }
    // a date input over the days of a koyomi date: given those days, the days of another date,
    // and a day that can also be the one received
    let days_cases = [("payment", "due.day"), ("closing", "due.day"), ("payment", "received")];
    for (date, day) in days_cases {
        let dir = project(vec![
            ("payment_terms.cal", TERMS_CAL.to_string()),
            ("batch.rule", DAYS_RULE.replace("DATE", date)),
            ("billing.flow", DAYS_FLOW.replace("DAY", day)),
        ]);
        let p = ritsu_project::Project::load(&[dir.to_string_lossy().to_string()], None).unwrap_or_else(|e| panic!("{e:?}"));
        let joined = ritsu_project::Joined::new();
        let calls = joined.dandori.crossings(&dir.join("billing.flow"), &joined.ports()).unwrap_or_else(|e| panic!("the flow does not pass dandori's check: {e:?}")).rules;
        assert_eq!(calls.len(), 1);
        let facts = joined.rulec.facts(&calls[0].rule).unwrap_or_else(|e| panic!("{e:?}"));
        let days = facts.preconditions.iter().find_map(|q| match q {
            Precondition::Days { days, .. } => Some(days.iter().copied().collect::<Vec<_>>()),
            _ => None,
        }).expect("the rule takes the days of a koyomi date");
        let crossed = ritsu_cross::check(&p, &joined, Lang::En);
        let arg = calls[0].args.iter().find(|x| x.input == "pay_day");
        let line = json!({"x2": {"kind": "days", "days": days, "arg": arg_json(&joined, arg)}});
        rows.push((line.to_string(), said_of_call(&crossed, &format!("days {date} {day}"))));
    }
    let answers: Vec<String> = rows.iter().map(|r| r.1.clone()).collect();
    assert!(answers.iter().any(|a| a.contains("\"day\"")), "no day was given as an example");
    let lines = run(&bin, &tmp, &json!({}), "x2 at the call", rows);
    assert_eq!(lines, OPS.len() * cases.len() + days_cases.len());
}

// ── X3 (a): the days of every koyomi date ─────────────────────────────────────────────────────

fn koyomi_dir() -> PathBuf {
    crates().join("koyomi")
}

/// The dates files of koyomi's examples and fixtures that pass koyomi's check, English first, each
/// with its model.
fn dates_files() -> Vec<(PathBuf, koyomi::resolve::Model)> {
    let mut out = Vec::new();
    for dir in ["examples", "examples/calendars", "tests/fixtures", "tests/fixtures/calendars"] {
        let mut v: Vec<PathBuf> = std::fs::read_dir(koyomi_dir().join(dir))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "cal"))
            .collect();
        v.sort_by_key(|p| (!p.file_name().unwrap().to_string_lossy().is_ascii(), p.clone()));
        for p in v {
            let o = check(&p.to_string_lossy()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            if o.has_errors() {
                continue;
            }
            if let Some(Checked::Dates(m, _)) = o.checked {
                out.push((p, *m));
            }
        }
    }
    out
}

fn shown(p: &Path) -> String {
    p.strip_prefix(crates()).unwrap_or(p).display().to_string()
}

fn days_json(found: &Found<ritsu_ports::DaySet>) -> String {
    match found {
        Found::Value(s) => json!({ "days": s.iter().collect::<Vec<_>>() }).to_string(),
        Found::Undecided(_) => "\"undecided\"".into(),
    }
}

#[test]
fn x3a_the_days_of_every_koyomi_date_are_the_models() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-days");
    let k = koyomi::ports::Engine;
    let (mut files, mut dates, mut lines) = (0, 0, 0);
    for (p, m) in dates_files() {
        let mut rows = Vec::new();
        let mut sets: Vec<Found<ritsu_ports::DaySet>> = Vec::new();
        for (i, d) in m.dates.iter().enumerate() {
            let found = k.values(&p, &d.name).unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
            rows.push((json!({ "values": i }).to_string(), days_json(&found)));
            let set_json = match &found {
                Found::Value(s) => json!(s.iter().collect::<Vec<_>>()),
                Found::Undecided(_) => Value::Null,
            };
            let mut around: Vec<(Option<i64>, Option<i64>)> = vec![(None, None)];
            if let Found::Value(s) = &found {
                if let (Some(lo), Some(hi)) = (s.first().copied(), s.last().copied()) {
                    around.extend([(Some(lo), Some(hi)), (Some(lo + 1), None), (None, Some(hi - 1)), (Some(lo - 3), Some(hi + 3)), (Some(hi), Some(lo)), (Some((lo + hi) / 2), None)]);
                }
            }
            for r in around {
                let line = json!({"days_fit": {"days": set_json, "range": [r.0, r.1]}});
                rows.push((line.to_string(), answer(&days_fit(&found, r), |d| json!(d))));
            }
            dates += 1;
            sets.push(found);
        }
        // a value that can be the day of any of the file's dates, or of one, with a place that says
        // nothing of what day it is or without, against ranges around the days
        let all: Vec<i64> = sets.iter().filter_map(|f| match f { Found::Value(s) => Some(s.iter().copied().collect::<Vec<_>>()), _ => None }).flatten().collect();
        let mut ranges: Vec<(Option<i64>, Option<i64>)> = vec![(None, None)];
        if let (Some(lo), Some(hi)) = (all.iter().min().copied(), all.iter().max().copied()) {
            ranges.extend([(Some(lo), Some(hi)), (Some(lo + 1), None), (None, Some(hi - 1)), (Some((lo + hi) / 2), Some(hi))]);
        }
        let sets_json = |fs: &[Found<ritsu_ports::DaySet>]| Value::Array(fs.iter().map(|f| match f { Found::Value(s) => json!(s.iter().collect::<Vec<_>>()), Found::Undecided(_) => Value::Null }).collect());
        let why = ritsu_base::tr!("何日かは分かりません", "nothing says what day");
        for pick in [sets.clone(), sets.iter().rev().cloned().collect(), sets.iter().take(1).cloned().collect(), vec![]] {
            for other in [false, true] {
                for r in &ranges {
                    let line = json!({"days_given": {"dates": sets_json(&pick), "other": other, "range": [r.0, r.1]}});
                    let rust = answer(&days_given(&pick, other.then_some(&why), *r), |(i, d)| json!([i, d]));
                    rows.push((line.to_string(), rust));
                }
            }
        }
        let file = json!({"dates": common::dates(&m), "budget": DEFAULT_BUDGET});
        lines += run(&bin, &tmp, &file, &format!("x3a {}", shown(&p)), rows);
        files += 1;
    }
    assert!(files >= 12, "fewer dates files than there were: {files}");
    assert!(dates >= 25, "fewer dates than there were: {dates}");
    println!("x3a: {files} files, {dates} dates, {lines} lines");
}

// ── X3 (b): the days a certificate carries ────────────────────────────────────────────────────

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// The days the certificate of `rule` carries for `input`, with koyomi joined.
fn certificate_days(rule: &Path, input: &str) -> Vec<i64> {
    let src = std::fs::read_to_string(rule).unwrap();
    let path = rule.to_string_lossy().to_string();
    let port: rulec::days::Port = Arc::new(koyomi::ports::Engine);
    let cert = rulec::days::with(Some(port), || {
        let (f, c) = rulec::prepare(&src, &path).map_err(|d| format!("{d:?}")).unwrap();
        rulec::cert::certificate(&f, &c, &src, &path)
    });
    let v: Value = serde_json::from_str(&cert).unwrap();
    v["days"][input]["days"].as_array().unwrap_or_else(|| panic!("no days for {input}:\n{cert}")).iter().map(|d| d.as_str().unwrap().parse().unwrap()).collect()
}

#[test]
fn x3b_the_days_a_certificate_carries_are_the_models() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-cert");
    // rulec's own material, and a rule over the days of koyomi's example in English and in Japanese
    let dir = tmp.path().join("examples");
    copy_dir(&koyomi_dir().join("examples"), &dir);
    let rule = |cal: &str, date: &str| format!(
        "rule batches v1\n\nenum run = this_year | next_year\n\ninputs\n  pay_day : date  range from koyomi \"{cal}\" date {date}\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-12-31 | this_year      |\n| >=2027-01-01 | next_year      |\n"
    );
    std::fs::write(dir.join("batches.rule"), rule("payment_20th_close_next_10th.cal", "payment")).unwrap();
    std::fs::write(dir.join("batches.ja.rule"), rule("payment_20th_close_next_10th.ja.cal", "支払日")).unwrap();
    let cases = [
        (crates().join("rulec/tests/days/settlement.rule"), crates().join("rulec/tests/days/payment_terms.cal"), "payment"),
        (dir.join("batches.rule"), dir.join("payment_20th_close_next_10th.cal"), "payment"),
        (dir.join("batches.ja.rule"), dir.join("payment_20th_close_next_10th.ja.cal"), "支払日"),
    ];
    let mut lines = 0;
    for (rule, cal, date) in cases {
        let days = certificate_days(&rule, "pay_day");
        assert!(!days.is_empty(), "{}", rule.display());
        let o = check(&cal.to_string_lossy()).unwrap();
        let Some(Checked::Dates(m, _)) = o.checked else { panic!("{} is not a dates file", cal.display()) };
        let k = m.dates.iter().position(|d| d.name == date).unwrap();
        // the certificate's days are koyomi's set, as koyomi's port hands it over
        let found = koyomi::ports::Engine.values(&cal, date).unwrap();
        assert_eq!(days_json(&found), json!({ "days": days }).to_string(), "{}", rule.display());
        let file = json!({"dates": common::dates(&m), "budget": DEFAULT_BUDGET});
        let rows = vec![(json!({ "values": k }).to_string(), json!({ "days": days }).to_string())];
        lines += run(&bin, &tmp, &file, &format!("x3b {}", rule.file_name().unwrap().to_string_lossy()), rows);
    }
    assert_eq!(lines, 3);
}

// ── X4: a rule's output as an amount ──────────────────────────────────────────────────────────

fn output_json(out: &Found<OutputValues>) -> Value {
    match out {
        Found::Undecided(_) => Value::Null,
        Found::Value(o) => json!({
            "min": end(o.min),
            "max": end(o.max),
            "values": o.values.as_ref().map_or(Value::Null, |vs| json!(vs)),
            "examples": o.examples.iter().map(|(v, ex)| json!([v, values_json(ex)])).collect::<Vec<_>>(),
        }),
    }
}

fn amount_row(out: &Found<OutputValues>) -> (String, String) {
    let line = json!({"amount_fits": {"out": output_json(out)}});
    let rust = answer(&amount_fits(out), |(v, ex)| json!([v, ex.as_ref().map_or(Value::Null, values_json)]));
    (line.to_string(), rust)
}

#[test]
fn x4_the_amounts_of_every_numeric_output_are_the_models() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-amounts");
    let engine = rulec::ports::Engine::new();
    let mut rules: Vec<PathBuf> = std::fs::read_dir(crates().join("rulec/tests/corpus")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rule")).collect();
    rules.sort_by_key(|p| (!p.file_name().unwrap().to_string_lossy().is_ascii(), p.clone()));
    let mut rows = Vec::new();
    let mut outputs = 0;
    for r in &rules {
        let Ok(facts) = engine.facts(r) else { continue };
        for o in &facts.outputs {
            let out = engine.output_values(r, &o.name).unwrap_or_else(|e| panic!("{}: {e:?}", r.display()));
            rows.push(amount_row(&out));
            outputs += 1;
        }
    }
    // every branch: open ends, below 1 among the rows' numbers or only at the low end, above
    // 2⁶³ − 1, an example or none
    let why = ritsu_base::tr!("分かりません", "not known");
    let ex = |v: i128| (v, vec![("what".to_string(), Wire::Enum(format!("e{v}")))]);
    let written = [
        Found::Undecided(why),
        Found::Value(OutputValues { min: None, max: Some(5), values: None, examples: vec![] }),
        Found::Value(OutputValues { min: Some(1), max: None, values: None, examples: vec![] }),
        Found::Value(OutputValues { min: Some(1), max: Some(9_223_372_036_854_775_807), values: None, examples: vec![] }),
        Found::Value(OutputValues { min: Some(0), max: Some(10), values: Some(vec![0, 10]), examples: vec![ex(0), ex(10)] }),
        Found::Value(OutputValues { min: Some(-300), max: Some(300), values: Some(vec![300, -300]), examples: vec![ex(-300)] }),
        Found::Value(OutputValues { min: Some(-5), max: Some(5), values: Some(vec![2, 3]), examples: vec![ex(-5)] }),
        Found::Value(OutputValues { min: Some(-5), max: Some(5), values: None, examples: vec![] }),
        Found::Value(OutputValues { min: Some(1), max: Some(9_223_372_036_854_775_808), values: None, examples: vec![ex(9_223_372_036_854_775_808)] }),
        Found::Value(OutputValues { min: Some(7), max: Some(3), values: None, examples: vec![] }),
    ];
    for out in &written {
        rows.push(amount_row(out));
    }
    // the amounts a value can be: outputs of rules (the corpus's and the written ones, a few at a
    // time), numbers dandori knows the range of, and a place that says nothing
    let mut all: Vec<Found<OutputValues>> = written.to_vec();
    for r in rules.iter().take(12) {
        let Ok(facts) = engine.facts(r) else { continue };
        for o in &facts.outputs {
            all.push(engine.output_values(r, &o.name).unwrap());
        }
    }
    let range_sets: Vec<Vec<(Option<i128>, Option<i128>)>> = vec![
        vec![],
        vec![(Some(1), Some(100))],
        vec![(Some(0), Some(100))],
        vec![(Some(1), None)],
        vec![(None, Some(5))],
        vec![(Some(5), Some(9_223_372_036_854_775_808))],
        vec![(Some(1), Some(10)), (Some(-3), Some(4))],
    ];
    let why = ritsu_base::tr!("分かりません", "not known");
    let from_json = |f: &AmountFrom| match f {
        AmountFrom::Output(i, ex) => json!({"output": [i, ex.as_ref().map_or(Value::Null, values_json)]}),
        AmountFrom::Range(i) => json!({"range": i}),
    };
    let ends_json = |rs: &[(Option<i128>, Option<i128>)]| Value::Array(rs.iter().map(|(lo, hi)| json!([end(*lo), end(*hi)])).collect());
    for k in 0..all.len() {
        for width in [0usize, 1, 2, 3] {
            let outs: Vec<Found<OutputValues>> = all.iter().cycle().skip(k).take(width).cloned().collect();
            for rs in &range_sets {
                for other in [false, true] {
                    let line = json!({"amounts_given": {"outputs": outs.iter().map(output_json).collect::<Vec<_>>(), "ranges": ends_json(rs), "other": other}});
                    let rust = answer(&amounts_given(&outs, rs, other.then_some(&why)), |(v, f)| json!([v, from_json(f)]));
                    rows.push((line.to_string(), rust));
                }
                let line = json!({"amounts_hull": {"outputs": outs.iter().map(output_json).collect::<Vec<_>>(), "ranges": ends_json(rs)}});
                let rust = match amounts_hull(&outs, rs) {
                    Some((lo, hi)) => json!([lo, hi]).to_string(),
                    None => "null".to_string(),
                };
                rows.push((line.to_string(), rust));
            }
        }
    }
    assert!(outputs >= 40, "fewer outputs than there were: {outputs}");
    run(&bin, &tmp, &json!({}), "x4 amounts", rows);
}

/// Every book of chobo's examples and tests, English first.
fn books() -> Vec<PathBuf> {
    let root = crates().join("chobo");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("examples")).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.push(root.join("tests/books"));
    let mut out = Vec::new();
    for d in dirs {
        out.extend(std::fs::read_dir(&d).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "book") && !p.to_string_lossy().ends_with(".before.book")));
    }
    out.sort_by_key(|p| (p.to_string_lossy().contains("tests/books"), !p.file_name().unwrap().to_string_lossy().is_ascii(), p.to_string_lossy().ends_with(".ja.book"), p.clone()));
    out
}

/// The rows of `refusals_met` for one answer of chobo's search: each operation it names and one it
/// does not, against the reasons it found, none, the same reversed, one fewer, and one more, each
/// held to the book's bounds' reasons, to none, and to every reason found.
fn refusal_rows(found: &Found<Vec<(String, Vec<String>)>>, bounds: &[String], rows: &mut Vec<(String, String)>) {
    let found_json = match found {
        Found::Value(ops) => json!(ops.iter().map(|(o, rs)| json!([o, rs])).collect::<Vec<_>>()),
        Found::Undecided(_) => Value::Null,
    };
    let mut ops: Vec<String> = match found {
        Found::Value(v) => v.iter().map(|(o, _)| o.clone()).collect(),
        Found::Undecided(_) => vec![],
    };
    ops.push("nothing".into());
    for op in ops {
        let reasons: Vec<String> = match found {
            Found::Value(v) => v.iter().find(|(o, _)| *o == op).map(|(_, r)| r.clone()).unwrap_or_default(),
            Found::Undecided(_) => vec![],
        };
        let mut handled_sets: Vec<Vec<String>> = vec![reasons.clone(), vec![], reasons.iter().rev().cloned().collect()];
        if !reasons.is_empty() {
            handled_sets.push(reasons[1..].to_vec());
        }
        let mut more = reasons.clone();
        more.push("expired_long_ago".into());
        handled_sets.push(more);
        let every: Vec<String> = match found {
            Found::Value(v) => v.iter().flat_map(|(_, r)| r.clone()).chain(["expired_long_ago".to_string()]).collect(),
            Found::Undecided(_) => vec![],
        };
        for handled in handled_sets {
            for b in [bounds.to_vec(), vec![], every.clone()] {
                let line = json!({"refusals_met": {"found": found_json, "op": op, "handled": handled, "bounds": b}});
                let rust = answer(&refusals_met(found, &op, &handled, &b), |u| json!([u.unhandled, u.unfound]));
                rows.push((line.to_string(), rust));
            }
        }
    }
}

#[test]
fn x4_the_refusals_of_every_transfer_are_the_models() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-refusals");
    let b = chobo::ports::Engine;
    let mut rows = Vec::new();
    let mut transfers = 0;
    // what chobo's check finds for every transfer of every book (the search over its own amounts)
    // the reasons of a book's bounds: the refusals that turn on the amounts, as `ritsu check` reads them
    let bounds_of = |facts: &ritsu_ports::BookFacts| -> Vec<String> { facts.accounts.iter().flat_map(|a| a.lower.iter().chain(a.upper.iter()).map(|x| x.refusal.clone())).collect() };
    for book in books() {
        let facts = b.facts(&book).unwrap_or_else(|e| panic!("{}: {e:?}", book.display()));
        let bounds = bounds_of(&facts);
        for t in &facts.transfers {
            transfers += 1;
            refusal_rows(&Found::Value(t.refusals.clone()), &bounds, &mut rows);
        }
    }
    // and what it finds with the amounts held to a rule's output: inside a range, a range that is
    // only 0 (chobo takes a transfer of 0, which moves nothing), and a range with no amount chobo
    // takes, which it does not decide
    let inventory = crates().join("chobo/examples/inventory/inventory.book");
    let bounds = bounds_of(&b.facts(&inventory).unwrap());
    for amounts in [(300, 800), (0, 0), (-5, -1)] {
        let found = b.refusals(&inventory, "reserve", amounts).unwrap_or_else(|e| panic!("{e:?}"));
        refusal_rows(&found, &bounds, &mut rows);
    }
    assert!(transfers >= 20, "fewer transfers than there were: {transfers}");
    run(&bin, &tmp, &json!({}), "x4 refusals", rows);
}

// ── X5: a hold, against when it expires ──────────────────────────────────────────────────────

#[test]
fn x5_a_hold_against_its_expiry_is_the_models() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-holds");
    let why = ritsu_base::tr!("上限がありません", "nothing bounds it");
    let mut rows = Vec::new();
    let marks: [u64; 9] = [0, 1, 59, 60, 61, 3_600, 1_209_599, 1_209_600, 1_209_601];
    for expiry in [0u64, 1, 60, 1_209_600] {
        for least in marks {
            for most in marks.iter().map(|m| Ok(*m)).chain([Err(why.clone())]) {
                let line = json!({"held_until": {"least": least, "most": most.as_ref().ok(), "expiry": expiry}});
                rows.push((line.to_string(), answer(&held_until(least, &most, expiry), |v| json!(v))));
            }
        }
    }
    run(&bin, &tmp, &json!({}), "x5 holds", rows);
}

// ── X6: the day given to a koyomi date ──────────────────────────────────────────────────────

#[test]
fn x6_the_day_given_to_every_koyomi_date_is_the_models() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-given");
    let k = koyomi::ports::Engine;
    let mut rows = Vec::new();
    let mut inputs = 0;
    let files = dates_files();
    // the days of every date of the first files, to give to the date inputs of every file
    let mut sets: Vec<Found<ritsu_ports::DaySet>> = Vec::new();
    for (p, m) in files.iter().take(4) {
        for d in &m.dates {
            sets.push(k.values(p, &d.name).unwrap());
        }
    }
    let why = ritsu_base::tr!("何日かは分かりません", "nothing says what day");
    for (p, _) in &files {
        let facts = k.facts(p).unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
        let inputs_json: Vec<Value> = facts.inputs.iter().map(|i| json!([i.name, i.kind == DateKind::Date, i.min, i.max])).collect();
        let mut names: Vec<String> = facts.inputs.iter().map(|i| i.name.clone()).collect();
        names.push("no_such_input".into());
        for name in &names {
            inputs += 1;
            let line = json!({"input_range": {"inputs": inputs_json, "input": name}});
            let range = input_range(&facts, name);
            rows.push((line.to_string(), range.map_or("null".to_string(), |(lo, hi)| json!([lo, hi]).to_string())));
            let Some((lo, hi)) = range else { continue };
            for pick in [&sets[..], &sets[..1], &sets[sets.len() - 1..]] {
                for other in [false, true] {
                    let line = json!({"days_given": {"dates": pick.iter().map(|f| match f { Found::Value(s) => json!(s.iter().collect::<Vec<_>>()), Found::Undecided(_) => Value::Null }).collect::<Vec<_>>(), "other": other, "range": [lo, hi]}});
                    let rust = answer(&days_given(pick, other.then_some(&why), (Some(lo), Some(hi))), |(i, d)| json!([i, d]));
                    rows.push((line.to_string(), rust));
                }
            }
        }
    }
    assert!(inputs >= 40, "fewer inputs than there were: {inputs}");
    run(&bin, &tmp, &json!({}), "x6 days given", rows);
}
