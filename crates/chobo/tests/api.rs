//! `chobo api` for each book in tests/books (the golden `<book>.api.json`), the hold's state
//! machine in the shape dandori's `src/rulec.rs` reads, and what every operation can be
//! refused with.

mod common;
use chobo::{api, check};
use common::*;
use serde_json::Value;

fn api_of(p: &std::path::Path) -> Value {
    let src = std::fs::read_to_string(p).unwrap();
    let c = check::check_source(&src);
    let book = c.book.as_ref().unwrap();
    api::api(book, &src, c.report.as_ref().unwrap(), &stem(p))
}

#[test]
fn the_api_of_each_book_is_its_golden() {
    for p in books_in("tests/books") {
        let v = api_of(&p);
        golden(&p.with_file_name(format!("{}.api.json", stem(&p))), &(serde_json::to_string_pretty(&v).unwrap() + "\n"));
    }
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}")).to_string()
}

/// Read a machine as dandori's `machine()` does, and answer, for an event in a state, the
/// state it goes to (None: it stays) and what the row says.
fn step(m: &Value, event: &str, state: &str) -> (Option<String>, String, String) {
    let cert = &m["certificate"];
    let cm = &cert["machine"];
    let table = s(&cm["table"]);
    let t = cert["tables"].as_array().unwrap().iter().find(|t| s(&t["table"]) == table).expect("the machine's table");
    assert_eq!(s(&t["policy"]), "first");
    let axes = t["axes"].as_array().unwrap();
    let state_axis = cm["axis"].as_u64().unwrap() as usize;
    let event_axis = 1 - state_axis;
    let coord = |axis: usize, name: &str| axes[axis]["coords"].as_array().unwrap().iter().position(|c| s(c) == name);
    let (e, st) = (coord(event_axis, event).expect("the event"), coord(state_axis, state).expect("the state"));
    let states: Vec<String> = m["states"].as_array().unwrap().iter().map(s).collect();
    for r in t["rows"].as_array().unwrap() {
        let acc = r["accepts"].as_array().unwrap();
        let has = |axis: usize, i: usize| acc[axis].as_array().unwrap().iter().any(|x| x.as_u64() == Some(i as u64));
        if has(event_axis, e) && has(state_axis, st) {
            let row = r["row"].as_u64().unwrap();
            let mv = cm["rows"].as_array().unwrap().iter().find(|x| x["row"].as_u64() == Some(row)).expect("where the row goes");
            let to = if mv["stay"].as_bool() == Some(true) { None } else { Some(states[mv["to"].as_u64().unwrap() as usize].clone()) };
            let p = r["produces"].as_array().unwrap();
            assert_eq!(p.len(), t["decides"].as_array().unwrap().len());
            return (to, s(&p[1]), s(&p[2]));
        }
    }
    panic!("no row takes {event} in {state}");
}

#[test]
fn a_hold_is_a_state_machine_dandori_can_read() {
    for p in books_in("tests/books") {
        let v = api_of(&p);
        let pending: Vec<&Value> = v["transfers"].as_array().unwrap().iter().filter(|t| !t["pending"].is_null()).collect();
        let machines = v["machines"].as_array().unwrap();
        assert_eq!(machines.len(), pending.len(), "{}", p.display());
        for m in machines {
            for key in ["name", "over", "carry", "held", "never", "once", "states", "initial", "final", "events", "external", "certificate"] {
                assert!(m.get(key).is_some(), "{}: the machine has no `{key}`", p.display());
            }
            for key in ["input", "output", "enum"] {
                assert!(m["carry"].get(key).is_some());
            }
            assert_eq!(s(&m["initial"]), "held");
            let expires = m["external"].as_array().unwrap().iter().any(|e| s(e) == "expire");
            // DESIGN 2.4, cell by cell
            assert_eq!(step(m, "post", "held"), (Some("posted".into()), "false".into(), "none".into()));
            assert_eq!(step(m, "void", "held"), (Some("voided".into()), "false".into(), "none".into()));
            assert_eq!(step(m, "post", "posted"), (None, "false".into(), "none".into()));
            assert_eq!(step(m, "void", "posted"), (None, "true".into(), "already_posted".into()));
            assert_eq!(step(m, "post", "voided"), (None, "true".into(), "already_voided".into()));
            assert_eq!(step(m, "void", "voided"), (None, "false".into(), "none".into()));
            assert_eq!(step(m, "post", "expired"), (None, "true".into(), "expired".into()));
            assert_eq!(step(m, "void", "expired"), (None, "true".into(), "expired".into()));
            if expires {
                assert_eq!(step(m, "expire", "held"), (Some("expired".into()), "false".into(), "none".into()));
                for st in ["posted", "voided", "expired"] {
                    assert_eq!(step(m, "expire", st), (None, "false".into(), "none".into()));
                }
            } else {
                assert!(!m["events"].as_array().unwrap().iter().any(|e| s(e) == "expire"));
            }
            // every final state is one a hold can end in, and none leads anywhere
            for f in m["final"].as_array().unwrap() {
                for e in m["events"].as_array().unwrap() {
                    assert_eq!(step(m, &s(e), &s(f)).0, None);
                }
            }
        }
    }
}

#[test]
fn every_operation_says_what_it_can_be_refused_with() {
    for p in books_in("tests/books") {
        let v = api_of(&p);
        for t in v["transfers"].as_array().unwrap() {
            let ops: Vec<&str> = if t["pending"].is_null() { vec!["do"] } else { vec!["hold", "post", "void"] };
            for op in ops {
                let rs = t["operations"][op]["refusals"].as_array().unwrap_or_else(|| panic!("{} {}.{op}", p.display(), t["name"]));
                assert!(!rs.is_empty(), "{}: {}.{op} lists nothing", p.display(), t["name"]);
                for r in rs {
                    // the example ends in that refusal
                    let last = r["example"].as_array().unwrap().last().unwrap();
                    assert_eq!(last["result"], "refused");
                    assert_eq!(last["reason"], r["name"]);
                }
            }
        }
    }
}
