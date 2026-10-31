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
//!   with an input whose range is a koyomi date — given the day of a koyomi date, or a date input
//!   with a range of days — checked as `ritsu check` checks them; the values the call gives are what
//!   dandori's port says (`Flows::crossings`), and the answer is what `ritsu check` says of the
//!   precondition (E201 with its example, W201, or nothing).
//! - X2 at a call of kaname (`ProgramVerdicts`): small modules that call a rule with a `constraint`,
//!   under a condition that keeps a relation between the two values, with none, with the values
//!   swapped, and with one value to both; the relations, ranges and runs are what kaname's port says
//!   (`Programs::crossings` and `seen`), and the answer is the verdict ritsu-cross hands kaname
//!   (shown, the run that breaks it, or undecided).
//! - X3 (a): every date of every dates file of koyomi's examples and fixtures that passes its check
//!   (the port answers for no other: the examples that break a claim on purpose are left out) —
//!   the set of days koyomi's port hands over (`Dates::values`) against the set the model computes
//!   from the same file, and `days_fit` and `days_given` over the sets, and over ranges of days a
//!   `.flow` writes beside them, against ranges around them.
//! - X3 (b): the days a rulec certificate carries for an input whose range is a koyomi date
//!   (`range from koyomi`) against the set the model computes from the koyomi file.
//! - X4: `amount_fits`, `amounts_given` and `amounts_hull` over every numeric output of the rules of
//!   rulec's corpus and ranges around them, the same with each enum input held to each of its
//!   values (`Rules::output_values_over`), and over outputs written to reach every branch;
//!   `refusals_met` over what chobo's search finds for every transfer of chobo's examples and test
//!   books (as its check finds them, and for one transfer with the amounts held to a range),
//!   against handled sets made from it and the reasons of each book's bounds.
//! - X4 at the call (`ritsu check`): small projects whose flow gives a rule's output to a transfer,
//!   with and without a `match` before it; the calls are what dandori's port says of the runs that
//!   reach the transfer (`Amount::calls`), each asked of rulec over what it gives the rule, and the
//!   answer is what `ritsu check` says of the amount (E203 with its example, W203, or nothing).
//! - X5: `held_until` over a grid of the fewest and most seconds a call comes after a hold and the
//!   seconds the hold expires after, around each edge.
//! - X6: `input_range` over every input of every dates file, and `days_given` of the days of each
//!   koyomi date, and of ranges of days and of moments read as days at an offset
//!   (`borders::moment_days`), against each date input's range.
//!
//! `-- --nocapture` shows a `compared cross <what>: <n> lines` line for each.

mod common;

use koyomi::check::{Checked, DEFAULT_BUDGET, check};
use ritsu_base::text::Lang;
use ritsu_cross::borders::{amount_fits, amounts_given, amounts_hull, days_fit, days_given, held_until, input_range, moment_days, refusals_met, AmountFrom, DayFrom};
use ritsu_model::{compare, program};
use ritsu_ports::{Amount, Answer, Books, ColumnType, Crossing, DateKind, Dates, Flows, Found, Held, Origin, OutputCall, OutputValues, Precondition, Programs, Rules, Value as Wire, Values, Verdicts};
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

/// Where the day of an example of `days_given` comes from, as `RitsuCross.daysGivenLine` writes it.
fn given_example(&(from, day): &(DayFrom, i64)) -> Value {
    match from {
        DayFrom::Date(i) => json!([{ "date": i }, day]),
        DayFrom::Range(i) => json!([{ "range": i }, day]),
    }
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
/// counts for each koyomi date it can be the day of, the ends of each range of days it can be in,
/// and `other` for the rest.
fn origins_json(joined: &ritsu_project::Joined, from: &[Origin]) -> Value {
    Value::Array(
        from.iter()
            .map(|o| match o {
                Origin::Day { file, date } => match joined.koyomi.values(file, date) {
                    Ok(Found::Value(set)) => json!({ "day": set.iter().collect::<Vec<_>>() }),
                    _ => json!({ "day": null }),
                },
                Origin::Range(lo, hi) => json!({ "range": [end(*lo), end(*hi)] }),
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

const DAYS_FLOW: &str = "workflow billing v1\ndescription \"Bills an order in the batch the rule picks for a day\"\n\nuse dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\nuse rule batch from \"batch.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:batch\"\n\ninputs\n  order    : string\n  received : date\n  at_once  : bool\n  paid_on  : date  range >=2026-02-10 <=2026-02-10\n  paid_by  : date  range >=2026-02-10 <=2026-03-10\n\ntask bill(order: string, due: date, run: batch.run)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:bill\"\n  key\n\nflow\n  let due = terms.payment(received: received)\n  let day = due.day\n  match at_once\n    true => let day = DAY\n    false => pass\n  let pick = batch(pay_day: day)\n  bill(order: order, due: day, run: pick.batch)\n";

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
    // a date input over the days of a koyomi date: given those days, the days of another date, a
    // day that can also be the one received, and a day that can also be one of a range of days the
    // flow's input takes, inside the rule's days and not
    let days_cases = [("payment", "due.day"), ("closing", "due.day"), ("payment", "received"), ("payment", "paid_on"), ("payment", "paid_by")];
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

// ── X2: the decision at a call of kaname ──────────────────────────────────────────────────────

/// A module of kaname that calls the rule `check` with what `BODY` gives it.
const CALL_MODULE: &str = "module call v1\n\nuse rule check from \"check.rule\"\n\nfn f(x: number range XR, y: number range YR) -> check.verdict?\n  BODY\n";

/// What a call of a module of kaname gives one input, as `RitsuCross.Arg` reads it: as the module
/// writes it, and the range kaname finds (every value it gives a number comes from kaname's own
/// ranges, a place that says nothing of what day it is).
fn given_json(g: Option<&ritsu_ports::Given>) -> Value {
    match g {
        None => Value::Null,
        Some(g) => json!({"shown": g.shown, "range": [end(g.range.0), end(g.range.1)], "origins": g.from.iter().map(|_| json!("other")).collect::<Vec<_>>()}),
    }
}

#[test]
fn x2_at_a_call_of_kaname_is_the_models_decision() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-kaname");
    // a relation kept by a condition (`<=`, `<`, `=`), none, the values swapped, one value to both,
    // and two values the corner of their ranges breaks though no run does
    let bodies = [
        ("kept", "if x <= y then check(a: x, b: y).r else none"),
        ("strict", "if x < y then check(a: x, b: y).r else none"),
        ("equal", "if x = y then check(a: x, b: y).r else none"),
        ("none", "check(a: x, b: y).r"),
        ("swapped", "if x <= y then check(a: y, b: x).r else none"),
        ("same", "check(a: x, b: x).r"),
        ("bounded", "check(a: min(x, y), b: max(x, y)).r"),
    ];
    let ranges = [("0 10", "0 10"), ("0 5", "5 10"), ("-5 0", "0 10"), ("10 20", "0 5")];
    let range = |r: &str| {
        let (lo, hi) = r.split_once(' ').unwrap();
        format!(">={lo} <={hi}")
    };
    let mut rows = Vec::new();
    let mut n = 0;
    let (mut held, mut failed, mut undecided, mut by_relation, mut unreached) = (0, 0, 0, 0, 0);
    for op in OPS {
        for (what, body) in bodies {
            for (xr, yr) in ranges {
                let dir = tmp.path().join(format!("k{n}"));
                n += 1;
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("check.rule"), CHECK_RULE.replace("OP", op)).unwrap();
                let kn = dir.join("call.kn");
                std::fs::write(&kn, CALL_MODULE.replace("XR", &range(xr)).replace("YR", &range(yr)).replace("BODY", body)).unwrap();
                let joined = ritsu_project::Joined::new();
                let calls = joined.kaname.crossings(&kn, &joined.ports()).unwrap_or_else(|e| panic!("{what}: the module does not pass kaname's check: {e:?}")).rules;
                // a branch the ranges never reach makes no call
                let Some(call) = calls.first() else {
                    unreached += 1;
                    continue;
                };
                assert_eq!(calls.len(), 1, "{what}");
                let arg = |name: &str| call.args.iter().find(|g| g.to == name);
                // what the runs of the function's vectors gave the two inputs at the call
                let int = |v: &Wire| match v {
                    Wire::Int(i) => Some(*i),
                    _ => None,
                };
                let runs: Vec<Value> = joined
                    .kaname
                    .seen(&kn, &joined.ports(), &call.function)
                    .iter()
                    .filter(|s| s.at == call.at)
                    .filter_map(|s| {
                        let of = |n: &str| s.given.iter().find(|(m, _)| m == n).and_then(|(_, v)| int(v));
                        Some(json!([of("a")?, of("b")?]))
                    })
                    .collect();
                let verdicts = ritsu_cross::ProgramVerdicts::new(&joined).verdicts(&kn);
                let pre: Vec<_> = verdicts.iter().filter(|v| v.check == Crossing::Precondition).collect();
                assert_eq!(pre.len(), 1, "{what}: one precondition at one call");
                let rust = match &pre[0].outcome {
                    Answer::Holds => {
                        held += 1;
                        "\"holds\"".to_string()
                    }
                    Answer::Undecided(_) => {
                        undecided += 1;
                        "\"undecided\"".to_string()
                    }
                    Answer::Fails(seen) => {
                        failed += 1;
                        let of = |n: &str| seen.given.iter().find(|(m, _)| m == n).and_then(|(_, v)| int(v)).unwrap();
                        json!({"fails": {"at": [of("a"), of("b")]}}).to_string()
                    }
                };
                let rels: Vec<Value> = call.relations.iter().map(|(a, r, b)| json!([a, r, b])).collect();
                if !rels.is_empty() && rust == "\"holds\"" {
                    by_relation += 1;
                }
                let line = json!({"x2k": {"op": op, "names": ["a", "b"], "left": given_json(arg("a")), "right": given_json(arg("b")), "scales": [1, 1], "rels": rels, "runs": runs}});
                rows.push((line.to_string(), rust));
            }
        }
    }
    println!("x2 at a call of kaname: {held} shown ({by_relation} with a relation kept), {failed} broken by a run, {undecided} undecided; {unreached} calls in a branch never reached");
    assert!(held > 0 && failed > 0 && undecided > 0 && by_relation > 0, "every verdict comes: {held} {failed} {undecided} {by_relation}");
    let lines = run(&bin, &tmp, &json!({}), "x2 at a call of kaname", rows);
    assert_eq!(lines + unreached, OPS.len() * bodies.len() * ranges.len());
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
        // ranges of days the value can be in beside the dates, as a `.flow` writes them (a date's
        // range, or a time's read as days): none, the days' own, one a day wider, open at an end,
        // and two of which the second is past the days
        let mut spans: Vec<Vec<(Option<i64>, Option<i64>)>> = vec![vec![]];
        if let (Some(lo), Some(hi)) = (all.iter().min().copied(), all.iter().max().copied()) {
            spans.extend([vec![(Some(lo), Some(hi))], vec![(Some(lo - 1), Some(hi))], vec![(None, Some(hi))], vec![(Some(lo), None)], vec![(Some(lo), Some(lo)), (Some(hi + 1), Some(hi + 2))]]);
        }
        for pick in [sets.clone(), sets.iter().rev().cloned().collect(), sets.iter().take(1).cloned().collect(), vec![]] {
            for other in [false, true] {
                for span in &spans {
                    for r in &ranges {
                        let line = json!({"days_given": {"dates": sets_json(&pick), "ranges": span.iter().map(|x| json!([x.0, x.1])).collect::<Vec<_>>(), "other": other, "range": [r.0, r.1]}});
                        let rust = answer(&days_given(&pick, span, other.then_some(&why), *r), given_example);
                        rows.push((line.to_string(), rust));
                    }
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
    let rust = answer(&amount_fits(out), |(v, ex)| json!([v, values_json(ex)]));
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
    // three rules of the shapes where an example decides: a refund the vectors reach (an example), a
    // difference a constraint keeps at 0 or more, which rulec counts from -50 (none), and the same
    // difference less 5, which never comes to the low end of its range and comes to -5 over the
    // vectors (an example from the fewest they come to, rulec DESIGN §15.270)
    let written_rules = tmp.path().join("rules");
    std::fs::create_dir_all(&written_rules).unwrap();
    std::fs::write(written_rules.join("adjust.rule"), "rule adjust v1\n\nenum kind = charge | refund\n\ninputs\n  what : kind\n\noutputs\n  amount : number  round down(1)\n\ntable pick\npolicy unique\n| what   | -> amount : number |\n| charge | 300                |\n| refund | -300               |\n").unwrap();
    std::fs::write(written_rules.join("tied.rule"), "rule tied v1\n\ninputs\n  asked   : number  range >=0 <=100\n  holding : number  range >=0 <=50\n\nconstraint holding <= asked\n\noutputs\n  needed : number  round down(1)\n\nresult needed = asked - holding\n").unwrap();
    std::fs::write(written_rules.join("short.rule"), "rule short v1\n\ninputs\n  asked   : number  range >=0 <=100\n  holding : number  range >=0 <=50\n\nconstraint holding <= asked\n\noutputs\n  needed : number  round down(1)\n\nresult needed = asked - holding - 5\n").unwrap();
    for (file, name) in [("adjust.rule", "amount"), ("tied.rule", "needed"), ("short.rule", "needed")] {
        let out = engine.output_values(&written_rules.join(file), name).unwrap();
        rows.push(amount_row(&out));
    }
    // the same rules with inputs held, as the runs of a flow can hold what a call gives them
    // (rulec DESIGN §15.273): the charge alone (holds) and the refund alone (-300, an example); the
    // seats asked 50 or more (0 to 100) and 50 or fewer (-50 to 50, which no vector comes below 0
    // at); with five kept back, 50 or more asked and 40 or fewer held (5 to 95)
    let held = |r: &[(&str, i128, i128)]| r.iter().map(|(n, l, h)| (n.to_string(), Some(*l), Some(*h))).collect::<Vec<_>>();
    let mut over: Vec<Found<OutputValues>> = Vec::new();
    for (file, name, r) in [
        ("adjust.rule", "amount", held(&[("what", 0, 0)])),
        ("adjust.rule", "amount", held(&[("what", 1, 1)])),
        ("tied.rule", "needed", held(&[("asked", 50, 100)])),
        ("tied.rule", "needed", held(&[("asked", 0, 50)])),
        ("short.rule", "needed", held(&[("asked", 50, 100), ("holding", 0, 40)])),
    ] {
        over.push(engine.output_values_over(&written_rules.join(file), name, &r).unwrap());
    }
    // and each numeric output of the corpus's rules with each enum input held to each of its values
    let mut corpus_over = 0;
    for r in &rules {
        let Ok(facts) = engine.facts(r) else { continue };
        for o in &facts.outputs {
            if !matches!(engine.output_values(r, &o.name), Ok(Found::Value(_))) {
                continue;
            }
            for i in &facts.inputs {
                let ColumnType::Enum { name, .. } = &i.ty else { continue };
                let values = facts.enums.iter().find(|e| e.name == *name).map_or(0, |e| e.values.len());
                for p in 0..values as i128 {
                    over.push(engine.output_values_over(r, &o.name, &[(i.name.clone(), Some(p), Some(p))]).unwrap_or_else(|e| panic!("{}: {e:?}", r.display())));
                    corpus_over += 1;
                }
            }
        }
    }
    assert!(corpus_over >= 20, "fewer enum values held than there were: {corpus_over}");
    for out in &over {
        rows.push(amount_row(out));
    }
    // every branch: open ends, both ends inside, an example below 0 (the smallest of several, the
    // first of equal ones, wherever it is among the examples), above 2⁶³ − 1 (the largest, and only
    // with none below 0), an example where an end is open, and ends outside with every example
    // inside, which is undecided
    let why = ritsu_base::tr!("分かりません", "not known");
    let ex = |v: i128| (v, vec![("what".to_string(), Wire::Enum(format!("e{v}")))]);
    let tagged = |v: i128, tag: &str| (v, vec![("what".to_string(), Wire::Enum(tag.to_string()))]);
    let past = 9_223_372_036_854_775_808_i128;
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
        Found::Value(OutputValues { min: Some(-5), max: Some(5), values: None, examples: vec![ex(3), ex(-2)] }),
        Found::Value(OutputValues { min: Some(-5), max: Some(5), values: None, examples: vec![ex(-1), ex(-4), ex(2)] }),
        Found::Value(OutputValues { min: Some(-5), max: Some(past), values: None, examples: vec![ex(past), ex(-1)] }),
        Found::Value(OutputValues { min: Some(1), max: Some(past + 5), values: None, examples: vec![ex(past), ex(past + 5), ex(3)] }),
        Found::Value(OutputValues { min: Some(-5), max: Some(5), values: None, examples: vec![tagged(-4, "first"), tagged(-4, "second")] }),
        Found::Value(OutputValues { min: Some(1), max: Some(past + 5), values: None, examples: vec![tagged(past + 5, "first"), tagged(past + 5, "second")] }),
        Found::Value(OutputValues { min: None, max: Some(5), values: None, examples: vec![ex(-3)] }),
        Found::Value(OutputValues { min: Some(1), max: None, values: None, examples: vec![ex(past)] }),
        Found::Value(OutputValues { min: Some(-5), max: Some(past), values: Some(vec![0, 5]), examples: vec![ex(0), ex(5)] }),
    ];
    for out in &written {
        rows.push(amount_row(out));
    }
    // the amounts a value can be: outputs of rules (the corpus's and the written ones, a few at a
    // time), numbers dandori knows the range of, and a place that says nothing
    let mut all: Vec<Found<OutputValues>> = written.to_vec();
    all.extend(over.iter().take(5).cloned());
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
        AmountFrom::Output(i, ex) => json!({"output": [i, values_json(ex)]}),
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

// ── X4: the decision at the call ──────────────────────────────────────────────────────────────

/// What a call gives the inputs of a rule, as `Rules::output_values_over` takes it: a number's
/// range on the wire, and each value of an enum or a bool by its place in the rule's order, a range
/// of its own (the port takes an input named more than once as the union).
fn held_at(joined: &ritsu_project::Joined, call: &OutputCall) -> Vec<(String, Option<i128>, Option<i128>)> {
    let facts = joined.rulec.facts(&call.rule).unwrap_or_else(|e| panic!("{e:?}"));
    let mut out = Vec::new();
    for i in &call.inputs {
        match &i.held {
            None => {}
            Some(Held::Range(lo, hi)) => out.push((i.input.clone(), *lo, *hi)),
            Some(Held::Values(vs)) => {
                let order: Vec<String> = match facts.inputs.iter().find(|c| c.name == i.input).map(|c| &c.ty) {
                    Some(ColumnType::Bool) => vec!["false".into(), "true".into()],
                    Some(ColumnType::Enum { name, .. }) => facts.enums.iter().find(|e| e.name == *name).unwrap().values.iter().map(|v| v.name.clone()).collect(),
                    other => panic!("{} is held to values, and is {other:?}", i.input),
                };
                for v in vs {
                    let p = order.iter().position(|o| o == v).unwrap_or_else(|| panic!("{v} is no value of {}", i.input)) as i128;
                    out.push((i.input.clone(), Some(p), Some(p)));
                }
            }
        }
    }
    out
}

/// Whether an amount can be a rule's output on a run that reaches its transfer, as the port says.
fn from_rule(a: &Amount) -> bool {
    match &a.calls {
        Some(calls) => !calls.is_empty(),
        None => a.from.iter().any(|o| matches!(o, Origin::Output { .. })),
    }
}

/// The places an amount can come from, as `RitsuCross.amountsGiven` reads them, from what the ports
/// say: each call of a rule dandori follows to the transfer, asked of rulec over what the call gives
/// the rule (where it does not follow them, the outputs over every input); the ranges dandori knows;
/// and whether the amount can come from a place that says nothing.
fn given_at(joined: &ritsu_project::Joined, a: &Amount) -> (Vec<Found<OutputValues>>, Vec<(Option<i128>, Option<i128>)>, bool) {
    let (mut outs, mut ranges, mut other) = (Vec::new(), Vec::new(), false);
    for call in a.calls.iter().flatten() {
        let held = held_at(joined, call);
        let got = if held.is_empty() { joined.rulec.output_values(&call.rule, &call.output) } else { joined.rulec.output_values_over(&call.rule, &call.output, &held) };
        outs.push(got.unwrap_or_else(|e| panic!("{e:?}")));
    }
    for o in &a.from {
        match o {
            Origin::Output { .. } if a.calls.is_some() => {}
            Origin::Output { rule, output } => outs.push(joined.rulec.output_values(rule, output).unwrap_or_else(|e| panic!("{e:?}"))),
            Origin::Range(lo, hi) => ranges.push((*lo, *hi)),
            _ => other = true,
        }
    }
    (outs, ranges, other)
}

/// An input as the notes of `ritsu check` write it: `event_kind = handback, late = true`.
fn shown_at(vs: &Values) -> String {
    vs.iter()
        .map(|(n, v)| {
            let x = match v {
                Wire::Int(i) => i.to_string(),
                Wire::Bool(b) => b.to_string(),
                Wire::Str(s) => format!("\"{s}\""),
                Wire::Date(d) | Wire::Enum(d) => d.clone(),
                Wire::None => "none".into(),
                Wire::List(xs) => format!("[{} elements]", xs.len()),
            };
            format!("{n} = {x}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// What `ritsu check` says of the one amount of a project that can be a rule's output, as
/// `RitsuCross.answerJ` writes `amountsGiven`'s answer: the E203 with its amount and the output and
/// example its first note names (found among `outs`, what the ports give), the W203, or nothing.
fn said_of_amount(crossed: &ritsu_cross::Crossed, outs: &[Found<OutputValues>], what: &str) -> String {
    let e: Vec<_> = crossed.findings.iter().filter(|f| f.code == "E203").collect();
    let w = crossed.findings.iter().filter(|f| f.code == "W203").count();
    assert!(e.len() + w <= 1, "{what}: one amount says one thing");
    let Some(f) = e.first() else {
        return if w == 1 { "\"undecided\"".into() } else { "\"holds\"".into() };
    };
    let note = match &f.json {
        ritsu_base::json::Json::Obj(kv) => match kv.iter().find(|(k, _)| k == "notes").map(|(_, v)| v) {
            Some(ritsu_base::json::Json::Arr(ns)) => ns[0].as_str().unwrap().to_string(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    let (_, rest) = note.split_once(", which comes to ").unwrap_or_else(|| panic!("{what}: a note that names no output: {note}"));
    let (v, at) = rest.split_once(" at ").unwrap();
    let v: i128 = v.parse().unwrap();
    let (i, ex) = outs
        .iter()
        .enumerate()
        .find_map(|(i, o)| match o {
            Found::Value(o) => o.examples.iter().find(|(x, ex)| *x == v && shown_at(ex) == at).map(|(_, ex)| (i, ex.clone())),
            Found::Undecided(_) => None,
        })
        .unwrap_or_else(|| panic!("{what}: no output the ports give comes to {v} at {at}"));
    json!({ "fails": [v, { "output": [i, values_json(&ex)] }] }).to_string()
}

/// X4 at the call (DESIGN 7.6): small projects whose flow gives a rule's output to a transfer of
/// chobo, checked as `ritsu check` checks them. The calls are what dandori's port says of the runs
/// that reach the transfer (`Amount::calls`), each asked of rulec over what the call gives the rule
/// (`Rules::output_values_over`); the model's `amountsGiven` over them is what `ritsu check` says
/// of the amount (E203 with its amount and example, W203, or nothing). The shapes: the seats given
/// back by a handback, given to the transfer with no branch, in the arm without the handback, after
/// an arm that ends in `fail`, from a call in the arm, from an arm that still takes the handback,
/// after another value is put in the variable the call was given, and from a call given a value
/// written in the flow; the seats still to give, for a number asked of 50 or more, for the same
/// number given to both inputs, and with five kept back.
#[test]
fn x4_at_the_call_is_the_models_decision() {
    let Some(bin) = model() else { return };
    let tmp = TempDir::new("model-cross-x4-call");
    let ledger = ritsu_cross::codes::ledger();
    let repro: Vec<(String, String)> = match &ledger.find("E203").expect("E203 is in the ledger").repro {
        ritsu_base::ledger::Repro::Dir { files, .. } => files.iter().filter(|(n, _)| !n.ends_with(".kn")).map(|(n, b)| (n.to_string(), b.to_string())).collect(),
        other => panic!("E203 has no files: {other:?}"),
    };
    let body = "  let need = seats(event_kind: kind)\n  give_seats(event: event, count: need.needed)\n";
    let with = |edits: &[(&str, &str)]| -> Vec<(String, String)> {
        repro
            .iter()
            .map(|(n, b)| {
                let mut b = b.clone();
                if n == "booking.flow" {
                    for (from, to) in edits {
                        assert!(b.contains(from), "the flow has no {from:?}");
                        b = b.replacen(from, to, 1);
                    }
                }
                (n.clone(), b)
            })
            .collect()
    };
    let book = repro.iter().find(|(n, _)| n == "hall.book").unwrap().1.clone();
    let head = "inputs\n  asked   : number  range >=0 <=100\n  holding : number  range >=0 <=50\n\nconstraint holding <= asked\n\noutputs\n  needed : number  round down(1)\n";
    let tied = format!("rule seats v1\n\n{head}\nresult needed = asked - holding\n");
    let short = format!("rule seats v1\n\n{head}\nresult needed = asked - holding - 5\n");
    let asked = "workflow booking v1\ndescription \"Gives an event the seats it still needs\"\n\nuse rule seats from \"seats.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:seats\"\nuse book hall from \"hall.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:hall\"\n\ninputs\n  event   : string\n  asked   : int  range >=50 <=100\n  holding : int  range >=0 <=50\n\ntask give_seats(event: string, count: int)\n  book hall.assign.do\n\nflow\n  let need = seats(asked: asked, holding: holding)\n  give_seats(event: event, count: need.needed)\n".to_string();
    let same = asked
        .replace("  asked   : int  range >=50 <=100\n  holding : int  range >=0 <=50\n", "  count   : int  range >=0 <=50\n")
        .replace("seats(asked: asked, holding: holding)", "seats(asked: count, holding: count)");
    let seats = |rule: &str, flow: &str| vec![("seats.rule".to_string(), rule.to_string()), ("hall.book".to_string(), book.clone()), ("booking.flow".to_string(), flow.to_string())];
    let shapes: Vec<(&str, Vec<(String, String)>, &str)> = vec![
        ("no branch", with(&[]), "fails"),
        ("in the arm", with(&[(body, "  let need = seats(event_kind: kind)\n  match kind\n    handback => pass\n    workshop, talk => give_seats(event: event, count: need.needed)\n")]), "holds"),
        ("after a fail", with(&[(body, "  let need = seats(event_kind: kind)\n  match kind\n    handback => fail Handback \"a handback gives no seats\"\n    workshop, talk => pass\n  give_seats(event: event, count: need.needed)\n")]), "holds"),
        ("called in the arm", with(&[(body, "  match kind\n    handback => pass\n    workshop, talk =>\n      let need = seats(event_kind: kind)\n      give_seats(event: event, count: need.needed)\n")]), "holds"),
        ("still in the arm", with(&[(body, "  let need = seats(event_kind: kind)\n  match kind\n    talk => pass\n    handback, workshop => give_seats(event: event, count: need.needed)\n")]), "fails"),
        (
            "another value",
            with(&[("  kind  : seats.kind\n", "  kind  : seats.kind\n  other : seats.kind\n"), (body, "  let k = kind\n  let need = seats(event_kind: k)\n  let k = other\n  match k\n    handback => pass\n    workshop, talk => give_seats(event: event, count: need.needed)\n")]),
            "fails",
        ),
        ("written", with(&[(body, "  let need = seats(event_kind: workshop)\n  give_seats(event: event, count: need.needed)\n")]), "holds"),
        ("asked 50 or more", seats(&tied, &asked), "holds"),
        ("the same number", seats(&tied, &same), "undecided"),
        ("five kept back", seats(&short, &asked), "fails"),
    ];
    let mut rows = Vec::new();
    for (k, (what, files, want)) in shapes.iter().enumerate() {
        let dir = tmp.path().join(format!("p{k}"));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in files {
            std::fs::write(dir.join(name), text).unwrap();
        }
        let p = ritsu_project::Project::load(&[dir.to_string_lossy().to_string()], None).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        let joined = ritsu_project::Joined::new();
        let crossings = joined.dandori.crossings(&dir.join("booking.flow"), &joined.ports()).unwrap_or_else(|e| panic!("{what}: the flow does not pass dandori's check: {e:?}"));
        let amounts: Vec<&Amount> = crossings.transfers.iter().flat_map(|t| t.amounts.iter()).filter(|a| from_rule(a)).collect();
        assert_eq!(amounts.len(), 1, "{what}: one amount from a rule");
        assert!(amounts[0].calls.is_some(), "{what}: dandori follows the runs to the transfer");
        let (outs, ranges, other) = given_at(&joined, amounts[0]);
        let crossed = ritsu_cross::check(&p, &joined, Lang::En);
        let said = said_of_amount(&crossed, &outs, what);
        assert!(said.contains(want), "{what}: {said}, not {want}");
        let line = json!({"amounts_given": {
            "outputs": outs.iter().map(output_json).collect::<Vec<_>>(),
            "ranges": ranges.iter().map(|(lo, hi)| json!([end(*lo), end(*hi)])).collect::<Vec<_>>(),
            "other": other,
        }});
        rows.push((line.to_string(), said));
    }
    let lines = run(&bin, &tmp, &json!({}), "x4 at the call", rows);
    assert_eq!(lines, shapes.len());
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
            // the moments of the input's days, as a time's range or the period the workflow runs in
            // can be, read as days in UTC and nine hours east; and ranges of days open at an end
            let moments = (Some(lo * 86_400), Some((hi + 1) * 86_400 - 1));
            let spans: Vec<Vec<(Option<i64>, Option<i64>)>> = vec![vec![], vec![moment_days(moments, 0)], vec![moment_days(moments, 540)], vec![moment_days(moments, -300)], vec![(Some(lo), None)], vec![(None, Some(hi))]];
            for pick in [&sets[..], &sets[..1], &sets[sets.len() - 1..]] {
                for other in [false, true] {
                    for span in &spans {
                        let line = json!({"days_given": {"dates": pick.iter().map(|f| match f { Found::Value(s) => json!(s.iter().collect::<Vec<_>>()), Found::Undecided(_) => Value::Null }).collect::<Vec<_>>(), "ranges": span.iter().map(|x| json!([x.0, x.1])).collect::<Vec<_>>(), "other": other, "range": [lo, hi]}});
                        let rust = answer(&days_given(pick, span, other.then_some(&why), (Some(lo), Some(hi))), given_example);
                        rows.push((line.to_string(), rust));
                    }
                }
            }
        }
    }
    assert!(inputs >= 40, "fewer inputs than there were: {inputs}");
    run(&bin, &tmp, &json!({}), "x6 days given", rows);
}
