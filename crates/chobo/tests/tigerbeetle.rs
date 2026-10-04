//! What only TigerBeetle has (PLAN C7, DESIGN 6): a book whose bounds changed under accounts
//! already opened is an error, not a quiet difference; a post of a hold TigerBeetle does not
//! have is answered no_such_hold without sending it, so that the hold can still be made and
//! posted with the same key; and an operation too big for one request stops the build (E060).
//!
//! One replica serves the whole test; it is stopped, and its data file removed, when the test
//! ends, failing too.

mod common;
use chobo::scenario;
use common::runners::*;
use common::servers::{Tb, TigerBeetle};
use common::*;
use serde_json::{Value, json};
use std::process::Command;

/// A case of the book at `path` with hand-written scenarios.
fn case_of(path: &Path, scenarios: Value) -> Case {
    let mut c = case(path);
    c.scenarios = scenarios.as_array().unwrap().iter().map(|v| scenario::from_json(&c.book, v).unwrap()).collect();
    c.reference = c.scenarios.iter().map(|s| scenario::run_json(&c.book, s).unwrap()).collect();
    c
}

/// Run the cases through one language's TigerBeetle client; what the runner printed.
fn run(tb: &TigerBeetle, lang: &str, cases: &[Case], work: &Path, go: Option<&GoRunner>) -> Option<Value> {
    let combo = format!("tigerbeetle-{lang}");
    let (cmd, clients): (Command, Vec<String>) = match lang {
        "typescript" => {
            if let Err(why) = node() {
                skip(&format!("{why}; the TypeScript client is not tried"));
                return None;
            }
            let dir = ts_dir(work).join(format!("tb-{}", cases[0].stem));
            let clients = build_all(cases, chobo::target::Target::TigerBeetleTypeScript, &dir).iter().map(|p| p.display().to_string()).collect();
            let mut cmd = Command::new("node");
            cmd.arg("--no-warnings").arg(runner_dir().join("runner.ts"));
            (cmd, clients)
        }
        "python" => {
            let py = match python() {
                Ok(p) => p,
                Err(why) => {
                    skip(&format!("{why}; the Python client is not tried"));
                    return None;
                }
            };
            let dir = work.join(format!("py-{}", cases[0].stem));
            let clients = build_all(cases, chobo::target::Target::TigerBeetlePython, &dir).iter().map(|p| p.display().to_string()).collect();
            let mut cmd = Command::new(py);
            cmd.arg(runner_dir().join("runner.py"));
            (cmd, clients)
        }
        _ => {
            let Some(g) = go else {
                skip("the Go runner is not built; the Go client is not tried");
                return None;
            };
            (Command::new(&g.bin), cases.iter().map(|c| g.keys[&(c.stem.clone(), "tigerbeetle")].clone()).collect())
        }
    };
    let books: Vec<Value> = cases.iter().zip(&clients).map(|(c, client)| book_input(c, client, &combo)).collect();
    let input = json!({"backend": "tigerbeetle", "tigerbeetle": tb.connection(), "books": books});
    Some(run_runner(cmd, &input, work, &format!("{combo}-{}", cases[0].stem)).unwrap_or_else(|e| panic!("{e}")))
}

/// 引当.post of a hold that is not there: refused with no_such_hold, and only the hold's first
/// transfer is read; nothing is sent, so the post's ID is still free, and once the hold is made
/// the same post goes through.
fn post_before_hold(tb: &TigerBeetle, lang: &str, work: &Path, go: Option<&GoRunner>) {
    let c = case_of(
        &root().join("tests/books/在庫.book"),
        json!([{"name": "post before hold", "steps": [
            {"op": "do", "kind": "入荷", "args": {"納品書": "納品書-1", "sku": "sku-2", "数": 5}},
            {"op": "post", "kind": "引当", "args": {"注文": "注文-3", "sku": "sku-2"}},
            {"op": "hold", "kind": "引当", "args": {"注文": "注文-3", "sku": "sku-2", "数": 2}},
            {"op": "post", "kind": "引当", "args": {"注文": "注文-3", "sku": "sku-2"}}
        ]}]),
    );
    let Some(out) = run(tb, lang, std::slice::from_ref(&c), work, go) else { return };
    let got = &out["books"][0]["scenarios"][0];
    assert!(got["error"].is_null(), "{lang}: {}", got["error"]);
    let answers: Vec<String> = got["result"]["steps"].as_array().unwrap().iter().map(|s| format!("{}{}", s["result"].as_str().unwrap(), s["reason"].as_str().map(|r| format!(" {r}")).unwrap_or_default())).collect();
    assert_eq!(answers, ["done", "refused no_such_hold", "done", "done"], "{lang}");
    let first_post = got["sent"].as_array().unwrap().iter().find(|s| s["step"] == 2).unwrap();
    let requests = first_post["requests"].as_array().unwrap();
    assert!(requests.len() == 1 && requests[0].get("lookup_transfers").is_some(), "{lang}: the post of a hold that is not there sent {requests:?}");
    assert!(agrees(&c.reference[0], &got["result"]), "{lang}");
    eprintln!("no_such_hold: {lang}: the post is not sent, and the same post goes through once the hold is made");
}

/// 与信 opened with an upper bound of 10000, then called with a book that says 20000: the room
/// account's opening is there with another amount, and the client stops with an error.
fn bounds_changed(tb: &TigerBeetle, lang: &str, work: &Path, v1: &Case, v2: &Case, go: (Option<&GoRunner>, Option<&GoRunner>)) {
    let Some(out) = run(tb, lang, std::slice::from_ref(v1), work, go.0) else { return };
    let got = &out["books"][0]["scenarios"][0];
    assert!(got["error"].is_null() && got["result"]["steps"][0]["result"] == "done", "{lang}: {got}");
    let Some(out) = run(tb, lang, std::slice::from_ref(v2), &work.join("v2"), go.1) else { return };
    let got = &out["books"][0]["scenarios"][0];
    let err = got["error"].as_str().unwrap_or("");
    assert!(err.contains("opened with another bound"), "{lang}: the book with other bounds answered {got}");
    eprintln!("bounds changed: {lang}: the opening with another amount stops the call with an error");
}

#[test]
fn what_only_tigerbeetle_has() {
    if !need(Need::TigerBeetle) {
        return;
    }
    // E060 needs no replica
    let out_dir = TempDir::new("e060");
    let out = chobo().args(["build", root().join("tests/fixtures/リクエスト.book").to_str().unwrap(), "--target", "tigerbeetle-typescript", "--out", out_dir.path().to_str().unwrap()]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.code() == Some(1) && err.contains("error[E060]"), "{err}");
    assert_eq!(std::fs::read_dir(out_dir.path()).unwrap().count(), 0, "nothing is written");
    eprintln!("E060: an operation too big for one request stops the build");

    let tb = match TigerBeetle::start() {
        Ok(tb) => tb,
        Err(why) => {
            skip(&format!("{why}; what only TigerBeetle has is not tried"));
            return;
        }
    };
    let work = TempDir::new("tigerbeetle-only");
    let one = json!([{"name": "open", "steps": [{"op": "do", "kind": "返済", "args": {"返済番号": "返済番号-1", "会員": "会員-2", "額": 5}}]}]);
    let v1 = case_of(&root().join("tests/books/与信.book"), one.clone());
    let v2_dir = work.path().join("v2-book");
    std::fs::create_dir_all(&v2_dir).unwrap();
    let src = std::fs::read_to_string(root().join("tests/books/与信.book")).unwrap();
    let changed = src.replace("at most 10000 refused as 前払い超過", "at most 20000 refused as 前払い超過");
    assert_ne!(src, changed, "tests/books/与信.book no longer has the bound the test changes");
    std::fs::write(v2_dir.join("与信.book"), changed).unwrap();
    let v2 = case_of(&v2_dir.join("与信.book"), one);
    let zaiko = case_of(&root().join("tests/books/在庫.book"), json!([]));
    let (go1, go2) = match go() {
        Ok(()) => (
            Some(build_go(&[zaiko, case_of(&root().join("tests/books/与信.book"), json!([]))], &work.path().join("go1")).unwrap_or_else(|e| panic!("{e}"))),
            Some(build_go(std::slice::from_ref(&v2), &work.path().join("go2")).unwrap_or_else(|e| panic!("{e}"))),
        ),
        Err(why) => {
            skip(&format!("{why}; the Go client is not tried"));
            (None, None)
        }
    };
    for lang in ["typescript", "python", "go"] {
        let w = work.path().join(lang);
        std::fs::create_dir_all(w.join("v2")).unwrap();
        post_before_hold(&tb, lang, &w, go1.as_ref());
        bounds_changed(&tb, lang, &w, &v1, &v2, (go1.as_ref(), go2.as_ref()));
    }
}

/// What `what_only_tigerbeetle_has` checks first, with the English book: an operation too big for
/// one request stops the build.
#[test]
fn an_operation_too_big_for_one_request_in_english() {
    if !need(Need::TigerBeetle) {
        return;
    }
    let out_dir = TempDir::new("e060-en");
    let out = chobo().args(["build", root().join("tests/fixtures/requests.book").to_str().unwrap(), "--target", "tigerbeetle-typescript", "--out", out_dir.path().to_str().unwrap()]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.code() == Some(1) && err.contains("error[E060]"), "{err}");
    assert_eq!(std::fs::read_dir(out_dir.path()).unwrap().count(), 0, "nothing is written");
}

/// reserve.post of a hold that is not there, with the English book (`post_before_hold`'s twin).
fn post_before_hold_in_english(tb: &TigerBeetle, lang: &str, work: &Path, go: Option<&GoRunner>) {
    let c = case_of(
        &root().join("tests/books/stock_reservation.book"),
        json!([{"name": "post before hold", "steps": [
            {"op": "do", "kind": "receive", "args": {"delivery": "delivery-1", "sku": "sku-2", "qty": 5}},
            {"op": "post", "kind": "reserve", "args": {"order": "order-3", "sku": "sku-2"}},
            {"op": "hold", "kind": "reserve", "args": {"order": "order-3", "sku": "sku-2", "qty": 2}},
            {"op": "post", "kind": "reserve", "args": {"order": "order-3", "sku": "sku-2"}}
        ]}]),
    );
    let Some(out) = run(tb, lang, std::slice::from_ref(&c), work, go) else { return };
    let got = &out["books"][0]["scenarios"][0];
    assert!(got["error"].is_null(), "{lang}: {}", got["error"]);
    let answers: Vec<String> = got["result"]["steps"].as_array().unwrap().iter().map(|s| format!("{}{}", s["result"].as_str().unwrap(), s["reason"].as_str().map(|r| format!(" {r}")).unwrap_or_default())).collect();
    assert_eq!(answers, ["done", "refused no_such_hold", "done", "done"], "{lang}");
    let first_post = got["sent"].as_array().unwrap().iter().find(|s| s["step"] == 2).unwrap();
    let requests = first_post["requests"].as_array().unwrap();
    assert!(requests.len() == 1 && requests[0].get("lookup_transfers").is_some(), "{lang}: the post of a hold that is not there sent {requests:?}");
    assert!(agrees(&c.reference[0], &got["result"]), "{lang}");
    eprintln!("no_such_hold (English book): {lang}: the post is not sent, and the same post goes through once the hold is made");
}

/// The twin of `what_only_tigerbeetle_has`, on the English books: credit opened with an upper
/// bound of 10000, then called with a book that says 20000; a post of a hold that is not there.
#[test]
fn what_only_tigerbeetle_has_in_english() {
    if !need(Need::TigerBeetle) {
        return;
    }
    let tb = match TigerBeetle::start() {
        Ok(tb) => tb,
        Err(why) => {
            skip(&format!("{why}; what only TigerBeetle has is not tried"));
            return;
        }
    };
    let work = TempDir::new("tigerbeetle-only-en");
    let one = json!([{"name": "open", "steps": [{"op": "do", "kind": "repayment", "args": {"repayment_id": "repayment_id-1", "member": "member-2", "amount": 5}}]}]);
    let v1 = case_of(&root().join("tests/books/credit.book"), one.clone());
    let v2_dir = work.path().join("v2-book");
    std::fs::create_dir_all(&v2_dir).unwrap();
    let src = std::fs::read_to_string(root().join("tests/books/credit.book")).unwrap();
    let changed = src.replace("at most 10000 refused as over_prepayment", "at most 20000 refused as over_prepayment");
    assert_ne!(src, changed, "tests/books/credit.book no longer has the bound the test changes");
    std::fs::write(v2_dir.join("credit.book"), changed).unwrap();
    let v2 = case_of(&v2_dir.join("credit.book"), one);
    let stock_reservation = case_of(&root().join("tests/books/stock_reservation.book"), json!([]));
    let (go1, go2) = match go() {
        Ok(()) => (
            Some(build_go(&[stock_reservation, case_of(&root().join("tests/books/credit.book"), json!([]))], &work.path().join("go1")).unwrap_or_else(|e| panic!("{e}"))),
            Some(build_go(std::slice::from_ref(&v2), &work.path().join("go2")).unwrap_or_else(|e| panic!("{e}"))),
        ),
        Err(why) => {
            skip(&format!("{why}; the Go client is not tried"));
            (None, None)
        }
    };
    for lang in ["typescript", "python", "go"] {
        let w = work.path().join(lang);
        std::fs::create_dir_all(w.join("v2")).unwrap();
        post_before_hold_in_english(&tb, lang, &w, go1.as_ref());
        bounds_changed(&tb, lang, &w, &v1, &v2, (go1.as_ref(), go2.as_ref()));
    }
}
