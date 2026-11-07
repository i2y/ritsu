//! Holding what a service answers to its contract (DESIGN §20). The specs and the documents are
//! under `tests/contract/`; their service, `tests/impl/orders.py`, answers as the id of an order
//! says. What the spec says of the contract is said before anything runs; what an answer breaks is
//! one more check of its `when`, in the report, the JSON and the journal.

mod common;
use common::*;

/// A scratch directory with the service and every file under `tests/contract/`.
fn scratch(name: &str) -> Scratch {
    let s = Scratch::new(name);
    s.write("c/orders.py", &repo_file("tests/impl/orders.py"));
    for f in ["orders.yaml", "schemas.yaml", "holds.geas", "breaks.geas", "outside.geas", "unheld.geas", "unheld.yaml"] {
        s.write(&format!("c/{f}"), &repo_file(&format!("tests/contract/{f}")));
    }
    s
}

#[test]
fn an_answer_the_contract_allows_says_nothing() {
    if !python3("the service") {
        return;
    }
    let s = scratch("contract-holds");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "c/holds.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/contract/holds.txt", &out);
    no_process_left(&log);
    // each answer is one more check in the journal, on its `when`'s line
    let journal = s.read("c/.geas/holds.journal.jsonl");
    let held: Vec<Json> = journal.lines().map(json).filter(|e| e.get("event").str() == "check" && e.get("check").str() == "contract").collect();
    assert_eq!(held.len(), 4, "{journal}");
    assert!(held.iter().all(|e| e.get("ok") == &Json::Bool(true)), "{journal}");
    assert_eq!(held[0].get("expected").str(), "GET /orders/{id}");
    assert_eq!(held[0].get("actual").str(), "200");
    assert_eq!(held[2].get("expected").str(), "POST /orders");
}

#[test]
fn what_an_answer_breaks_fails_its_claim() {
    if !python3("the service") {
        return;
    }
    let s = scratch("contract-breaks");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "c/breaks.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/contract/breaks.txt"), &out);
    }
    let (out, _, code) = run(s.path(), &["check", "c/breaks.geas", "--json"], &[]);
    assert_eq!(code, 1);
    let v = json(&out);
    assert_eq!(v.get("ok"), &Json::Bool(false));
    let first = &v.get("claims").arr()[0];
    let failed: Vec<&Json> = first.get("checks").arr().iter().filter(|c| c.get("ok") == &Json::Bool(false)).collect();
    assert_eq!(failed.len(), 1, "{out}");
}

#[test]
fn a_call_the_contract_does_not_have_stops_the_spec() {
    let s = scratch("contract-outside");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "c/outside.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((out.as_str(), code), ("", 2), "{err}");
        golden(&format!("{lang}/contract/outside.txt"), &err);
    }
    assert!(!s.exists("c/.geas"), "nothing ran, so nothing was written");
}

#[test]
fn what_the_contract_says_and_geas_does_not_hold_is_said() {
    if !python3("the service") {
        return;
    }
    let s = scratch("contract-unheld");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "c/unheld.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        // the rest is held
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/contract/unheld.txt"), &out);
    }
    let (out, _, _) = run(s.path(), &["check", "c/unheld.geas", "--json"], &[]);
    assert_eq!(json(&out).get("diagnostics").arr()[0].get("code").str(), "W015");
}

#[test]
fn a_contract_that_cannot_be_read_stops_the_spec() {
    let s = scratch("contract-unread");
    let spec = |file: &str| format!("target api {{\n  serve \"python3 orders.py {{port}}\"\n  port auto\n  contract \"{file}\"\n}}\n");
    for (name, file, text, word) in [
        ("not-yaml", "a.yaml", "a: [", "a.yaml"),
        ("not-openapi", "b.yaml", "title: orders\n", "neither an OpenAPI"),
        ("swagger", "c.yaml", "swagger: \"2.0\"\n", "Swagger 2.0"),
        ("asyncapi", "d.yaml", "asyncapi: 3.0.0\n", "AsyncAPI"),
        ("no-ref", "e.yaml", "openapi: 3.1.0\npaths:\n  /a:\n    get:\n      responses:\n        \"200\":\n          description: a\n          content:\n            application/json:\n              schema: { $ref: \"#/nowhere\" }\n", "#/nowhere"),
        ("ref-to-a-url", "f.yaml", "openapi: 3.1.0\npaths:\n  /a:\n    get:\n      responses:\n        \"200\":\n          description: a\n          content:\n            application/json:\n              schema: { $ref: \"https://example.com/a.json\" }\n", "https://example.com/a.json"),
    ] {
        s.write(&format!("c/{file}"), text);
        s.write(&format!("c/{name}.geas"), &spec(file));
        let (out, err, code) = run(s.path(), &["check", &format!("c/{name}.geas")], &[]);
        assert_eq!((out.as_str(), code), ("", 2), "{name}: {err}");
        assert!(err.contains("[E015]") && err.contains(word), "{name}: {err}");
    }
}

#[test]
fn every_command_that_runs_claims_reads_the_contract_first() {
    let s = scratch("contract-commands");
    for args in [vec!["snap", "c/outside.geas"], vec!["map", "c/outside.geas", "--root", "c"]] {
        let (_, err, code) = run(s.path(), &args, &[]);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.contains("[E016]"), "{args:?}: {err}");
    }
}

#[test]
fn an_action_on_a_page_is_not_held() {
    // an action on a service's page in a browser observes a screen, and a contract says nothing of it
    let s = Scratch::new("contract-page");
    s.write("c/orders.yaml", &repo_file("tests/contract/orders.yaml"));
    s.write("c/schemas.yaml", &repo_file("tests/contract/schemas.yaml"));
    s.write("c/page.geas", "target api {\n  serve \"python3 orders.py {port}\"\n  port auto\n  contract \"orders.yaml\"\n}\n\nclaim \"a page\" {\n  when api.open(\"/nowhere\")\n  then screen contains heading \"Orders\"\n}\n");
    // what stops it, if anything, is that there is no Chrome or no service: the path is nobody's call
    let (_, err, _) = run(s.path(), &["check", "c/page.geas"], &[("GEAS_CHROME", "/nowhere/chrome")]);
    assert!(!err.contains("[E016]") && !err.contains("[E015]"), "{err}");
}

/// In a module, the path is what any path written in a file is (ritsu's DESIGN 17.6): under the root
/// of the module, or in a dependency by `@<name>/`, and refused otherwise.
#[test]
fn in_a_module_a_contract_follows_the_paths_of_a_module() {
    if !python3("the service") {
        return;
    }
    let s = scratch("contract-module");
    s.write("shop/.git", "");
    s.write("shop/ritsu.mod", "module example.com/acme/shop\nversion 0.1.0\n\nrequire orders path \"../orders\"\n");
    s.write("orders/ritsu.mod", "module example.com/acme/orders\nversion 1.0.0\n");
    s.write("orders/api/orders.yaml", &repo_file("tests/contract/orders.yaml"));
    s.write("orders/api/schemas.yaml", &repo_file("tests/contract/schemas.yaml"));
    s.write("shop/orders.py", &repo_file("tests/impl/orders.py"));
    let spec = |file: &str| format!("target api {{\n  serve \"python3 orders.py {{port}}\"\n  port auto\n  contract \"{file}\"\n}}\n\nclaim \"an order\" {{\n  when api.get(\"/orders/ok\")\n  then status is 200\n}}\n");
    // from any directory of the module, into a dependency
    s.write("shop/shop.geas", &spec("@orders/api/orders.yaml"));
    let (out, err, code) = run(&s.path().join("shop"), &["check", "shop.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    // out of the module, and into a module it does not require
    s.write("shop/out.geas", &spec("../orders/api/orders.yaml"));
    let (_, err, code) = run(&s.path().join("shop"), &["check", "out.geas"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("[E015]") && err.contains("leads outside the module"), "{err}");
    s.write("shop/other.geas", &spec("@other/api/orders.yaml"));
    let (_, err, code) = run(&s.path().join("shop"), &["check", "other.geas"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("[E015]") && err.contains("is no dependency"), "{err}");
}

/// Side by side or one at a time, the same answers are the same bytes, in the report and in the
/// journal, what breaks the contract included.
#[test]
fn the_same_with_four_jobs() {
    if !python3("the service") {
        return;
    }
    let s = scratch("contract-jobs");
    for (spec, stem, exit) in [("c/holds.geas", "holds", 0), ("c/breaks.geas", "breaks", 1)] {
        let (one, _, code) = run(s.path(), &["check", spec, "-j", "1"], &[]);
        assert_eq!(code, exit, "{one}");
        let journal = s.read(&format!("c/.geas/{stem}.journal.jsonl"));
        let (four, _, code) = run(s.path(), &["check", spec, "-j", "4"], &[]);
        assert_eq!(code, exit, "{four}");
        assert_eq!(one, four);
        assert_eq!(journal, s.read(&format!("c/.geas/{stem}.journal.jsonl")));
    }
}
