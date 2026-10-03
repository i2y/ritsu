//! The scenarios chobo writes for each book in tests/books: every kind that applies is there,
//! the same book gives the same scenarios, and each one runs; their results are the golden
//! `<book>.runs.json`. The scenarios written by hand beside a book of tests/books or of the
//! examples (`<book>.more.json`) run too, and their results are the golden `<book>.more.runs.json`.

mod common;
use chobo::model::{Amount, Book, Expiry};
use chobo::{scenario, scenarios};
use common::*;
use serde_json::{Value, json};

fn load(p: &std::path::Path) -> Book {
    let (b, d) = chobo::model::load(&std::fs::read_to_string(p).unwrap());
    assert!(d.is_empty(), "{}", p.display());
    b.unwrap()
}

/// The kinds of scenario PLAN B7 asks for that apply to the book, as the beginnings of names.
fn expected(book: &Book) -> Vec<String> {
    let mut want = Vec::new();
    let bounded = |m: &chobo::model::Move| book.accounts[m.from.kind].lower.is_some() || book.accounts[m.to.kind].upper.is_some();
    if book.transfers.iter().any(|t| t.moves.iter().any(bounded)) {
        want.push("bound: ".to_string());
        want.push("together: ".to_string());
    }
    for t in &book.transfers {
        let op = if t.is_pending() { "hold" } else { "do" };
        want.push(format!("key: {}.{op} twice", t.name));
        if !t.amount_params().is_empty() {
            want.push(format!("key: {}.{op} again with another", t.name));
        }
        if t.moves.iter().any(bounded) {
            want.push(format!("key: {}.{op} refused with", t.name));
        }
        if t.is_pending() {
            for what in ["posted in full", "voided", "posted, then voided", "voided, then posted", "posted before it is held", "posted twice"] {
                want.push(format!("hold: {} {what}", t.name));
            }
            if !t.amount_params().is_empty() {
                want.push(format!("hold: {} posted in part", t.name));
                want.push(format!("hold: {} posted for more than it holds", t.name));
            }
        }
        if matches!(t.pending, Some(Expiry::After(_))) {
            want.push(format!("pass: {} expires", t.name));
            want.push(format!("pass: {} held again", t.name));
        }
        if t.moves.len() > 1 && t.moves.iter().any(bounded) {
            want.push(format!("moves: {}.{op}", t.name));
        }
        if t.moves.iter().any(|m| m.from.kind == m.to.kind && m.from != m.to && !matches!(m.amount, Amount::Lit(0))) {
            want.push(format!("same_account: {}.{op}", t.name));
        }
    }
    want
}

#[test]
fn every_kind_that_applies_is_there_and_every_scenario_runs() {
    for p in books_in("tests/books") {
        let book = load(&p);
        let list = scenarios::generate(&book);
        let again = scenarios::generate(&book);
        assert_eq!(list, again, "{}: two runs gave different scenarios", p.display());
        let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        for w in expected(&book) {
            assert!(names.iter().any(|n| n.starts_with(&w)), "{}: no scenario `{w}…`", p.display());
        }
        let mut out = Vec::new();
        for s in &list {
            let v = scenario::to_json(&book, s);
            // what is written out reads back as the same scenario
            assert_eq!(&scenario::from_json(&book, &v).unwrap(), s, "{}", s.name);
            let r = scenario::run_json(&book, s).unwrap_or_else(|e| panic!("{}: {e}", s.name));
            if scenario::has_together(s) {
                assert!(r["outcomes"].as_array().unwrap().len() >= 2, "{}", s.name);
            }
            out.push(json!({"scenario": v, "result": r}));
        }
        golden(&p.with_file_name(format!("{}.runs.json", stem(&p))), &(serde_json::to_string_pretty(&Value::Array(out)).unwrap() + "\n"));
    }
}

/// The values of a scenario are all different (PLAN B7): two strings of different parameters
/// never meet, so a target that mixed them up would show it.
#[test]
fn the_strings_of_a_scenario_are_all_different() {
    for p in books_in("tests/books") {
        let book = load(&p);
        for s in scenarios::generate(&book) {
            let v = scenario::to_json(&book, &s);
            let mut seen: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
            let mut calls: Vec<Value> = Vec::new();
            for st in v["steps"].as_array().unwrap() {
                match st["op"].as_str() {
                    Some("together") => calls.extend(st["callers"].as_array().unwrap().iter().flat_map(|c| c.as_array().unwrap().clone())),
                    Some("pass") => {}
                    _ => calls.push(st.clone()),
                }
            }
            for c in calls {
                for (param, val) in c["args"].as_object().unwrap() {
                    if let Some(sv) = val.as_str() {
                        if let Some(prev) = seen.insert(sv.to_string(), param.clone()) {
                            // a value may come back under the same name, or as the account it names
                            let stem = |x: &str| x.rsplit_once('-').map(|(a, _)| a.to_string()).unwrap_or_default();
                            assert!(prev == *param || stem(sv) == prev || stem(sv) == *param, "{}: `{sv}` is both {prev} and {param}", s.name);
                        }
                    }
                }
            }
        }
    }
}

/// The scenarios written by hand, for what the generated ones do not reach: each one runs, and
/// what it comes to is fixed, as the generated ones are.
#[test]
fn the_scenarios_written_by_hand_run_and_their_results_are_fixed() {
    let mut found = 0;
    for p in books_in("tests/books").into_iter().chain(example_books()) {
        let more = p.with_file_name(format!("{}.more.json", stem(&p)));
        let Ok(text) = std::fs::read_to_string(&more) else { continue };
        let book = load(&p);
        let list: Vec<Value> = serde_json::from_str(&text).unwrap();
        let mut out = Vec::new();
        for v in &list {
            let s = scenario::from_json(&book, v).unwrap_or_else(|e| panic!("{}: {e}", more.display()));
            assert!(s.name.starts_with("more: "), "{}: a scenario written by hand is named `more: …`", s.name);
            let r = scenario::run_json(&book, &s).unwrap_or_else(|e| panic!("{}: {e}", s.name));
            out.push(json!({"scenario": scenario::to_json(&book, &s), "result": r}));
            found += 1;
        }
        golden(&p.with_file_name(format!("{}.more.runs.json", stem(&p))), &(serde_json::to_string_pretty(&Value::Array(out)).unwrap() + "\n"));
    }
    assert!(found >= 6, "only {found} scenarios written by hand");
}
