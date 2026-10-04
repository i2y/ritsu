//! chobo's reference interpreter, held to `ChoboModel` (DESIGN 11.3).
//!
//! Every book of chobo's examples and of its tests: each scenario chobo writes for it
//! (`scenarios::generate`) and each one written by hand beside it (`<book>.more.json`) is run by
//! `scenario::run_json` and by the Lean model, and the two results are compared — the steps and
//! their outcomes, every named account's balances, every hold's state, and with `together`, the
//! set of the ways it can come out. `-- --nocapture` shows a `compared chobo <book>: <n> lines`
//! line for each book.

use chobo::interp::{Call, Val};
use chobo::model::{Amount, Arg, Book, Expiry, Ref, Ty};
use chobo::scenario::{self, Scenario, Step};
use ritsu_model::{compare, program};
use ritsu_testkit::{Need, TempDir, ready};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn chobo_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../chobo")
}

/// Every book of the examples and of the tests, English first (the examples' English books, then
/// their Japanese twins, then the tests' books).
fn books() -> Vec<PathBuf> {
    let root = chobo_dir();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("examples")).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.push(root.join("tests/books"));
    let mut out = Vec::new();
    for d in dirs {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "book") && !p.to_string_lossy().ends_with(".before.book"))
            .collect();
        v.sort_by_key(|p| (p.to_string_lossy().ends_with(".ja.book"), p.clone()));
        out.extend(v);
    }
    out.sort_by_key(|p| (p.to_string_lossy().contains("tests/books"), p.to_string_lossy().ends_with(".ja.book"), p.clone()));
    out
}

fn stem(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().trim_end_matches(".book").to_string()
}

fn reference(r: &Ref) -> Value {
    json!({
        "kind": r.kind,
        "args": r.args.iter().map(|a| match a {
            Arg::Param(i) => json!({"param": i}),
            Arg::Lit(s) => json!({"lit": s}),
        }).collect::<Vec<_>>(),
    })
}

/// The book as `ChoboModel.readBook` reads it: everything by its place, the expiry in seconds.
fn book_json(b: &Book) -> Value {
    let bound = |x: &Option<chobo::model::Bound>| x.as_ref().map(|l| json!({"value": l.value as i64, "refusal": l.refusal}));
    json!({
        "accounts": b.accounts.iter().map(|a| json!({"name": a.name, "lower": bound(&a.lower), "upper": bound(&a.upper)})).collect::<Vec<_>>(),
        "transfers": b.transfers.iter().map(|t| json!({
            "name": t.name,
            "amounts": t.params.iter().map(|p| matches!(p.ty, Ty::Amount(_))).collect::<Vec<_>>(),
            "key": t.key,
            "pending": match t.pending {
                None => Value::Null,
                Some(Expiry::Never) => json!("never"),
                Some(Expiry::After(s)) => json!({"after": s}),
            },
            "moves": t.moves.iter().map(|m| json!({
                "amount": match m.amount { Amount::Param(i) => json!({"param": i}), Amount::Lit(v) => json!({"lit": v as i64}) },
                "src": reference(&m.from),
                "dst": reference(&m.to),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn call_json(c: &Call) -> Value {
    json!({
        "op": c.op.name(),
        "kind": c.kind,
        "args": c.args.iter().map(|a| match a {
            None => Value::Null,
            Some(Val::Str(s)) => json!({"str": s}),
            Some(Val::Amt(v)) => json!({"amt": *v as i64}),
        }).collect::<Vec<_>>(),
        "amounts": c.amounts.as_ref().map(|m| m.iter().map(|(i, v)| json!([i, *v as i64])).collect::<Vec<_>>()),
    })
}

/// A scenario as `ChoboModel.readSteps` reads it: calls by their place, `pass` in seconds.
fn scenario_json(s: &Scenario) -> String {
    let steps: Vec<Value> = s
        .steps
        .iter()
        .map(|st| match st {
            Step::Call(c) => json!({"call": call_json(c)}),
            Step::Pass(secs, _) => json!({"pass": secs}),
            Step::Together(cs) => json!({"together": cs.iter().map(|ops| ops.iter().map(call_json).collect::<Vec<_>>()).collect::<Vec<_>>()}),
        })
        .collect();
    json!({"steps": steps}).to_string()
}

/// The two results are the same: an error on both sides; with `together`, the same set of ways it
/// comes out; otherwise the same value.
fn same(ours: &str, model: &str) -> bool {
    let (Ok(a), Ok(b)) = (serde_json::from_str::<Value>(ours), serde_json::from_str::<Value>(model)) else { return false };
    if a.get("error").is_some() || b.get("error").is_some() {
        return a.get("error").is_some() && b.get("error").is_some();
    }
    match (a.get("outcomes").and_then(Value::as_array), b.get("outcomes").and_then(Value::as_array)) {
        (Some(x), Some(y)) => x.len() == y.len() && x.iter().all(|o| y.contains(o)),
        (None, None) => a == b,
        _ => false,
    }
}

#[test]
fn every_scenario_of_every_book_comes_out_as_the_model_says() {
    let bin = program();
    if !ready(Need::Lean, || bin.exists(), "proofs/ is not built (lake build makes ritsu-model)") {
        return;
    }
    let tmp = TempDir::new("model-chobo");
    let (mut books_seen, mut lines) = (0, 0);
    for p in books() {
        let (book, d) = chobo::model::load(&std::fs::read_to_string(&p).unwrap());
        assert!(d.is_empty(), "{} does not load", p.display());
        let book = book.unwrap();
        let mut scenarios = chobo::scenarios::generate(&book);
        let more = p.with_file_name(format!("{}.more.json", stem(&p)));
        if let Ok(text) = std::fs::read_to_string(&more) {
            let v: Value = serde_json::from_str(&text).unwrap();
            for s in v.as_array().unwrap() {
                scenarios.push(scenario::from_json(&book, s).unwrap_or_else(|e| panic!("{}: {e}", more.display())));
            }
        }
        assert!(!scenarios.is_empty(), "{}: no scenarios", p.display());
        let file = tmp.write(&format!("{}.json", stem(&p)), book_json(&book).to_string());
        let rows = scenarios.iter().map(|s| {
            let ours = match scenario::run_json(&book, s) {
                Ok(v) => v.to_string(),
                Err(e) => json!({"error": e}).to_string(),
            };
            (scenario_json(s), ours)
        });
        let shown = p.strip_prefix(chobo_dir()).unwrap().display().to_string();
        let c = compare(&bin, "chobo", &file, rows, &same).unwrap_or_else(|e| panic!("{shown}: {e}"));
        assert_eq!(c.differ, 0, "{}", c.report(&shown));
        println!("compared chobo {shown}: {} lines", c.lines);
        books_seen += 1;
        lines += c.lines;
    }
    assert!(books_seen >= 14, "the books are fewer than they were: {books_seen}");
    assert!(lines >= 300, "the scenarios are fewer than they were: {lines}");
}
