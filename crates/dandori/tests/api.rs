//! `dandori api`: what a workflow declares, as JSON, for the tools and agents that do not read the
//! `.flow`.
//!
//! The JSON is built from the checked program, so the test holds it to the file it describes: on
//! every flow of the examples and of tests/flows it reads as JSON, its tasks are the `task` lines of
//! the file in order, its `fail`s name the errors the file writes, its inputs and outputs are the
//! file's, and no path of the machine that ran it is in it. A workflow that does not pass `check`
//! has no `api`, and says what `check` says. One golden file (tests/api/hotel_stay.json) holds the
//! whole shape for the hotel example; `DANDORI_BLESS=1` (or `RITSU_BLESS=1`) rewrites it, and the
//! diff is what to read.

use ritsu_testkit::{skip, TempDir};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The command with rulec's port, with the dates files and the books joined, as `ritsu dandori` runs it.
fn run(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let rules = Rc::new(rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine)));
    let code = dandori::sources::with_undecided(rules.clone(), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), Rc::new(Nothing), || dandori::cli::run(&args, rules.clone(), &mut out, &mut err));
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

struct Nothing;

impl ritsu_ports::Undecided for Nothing {
    fn preconditions(&self, _: &Path) -> Vec<ritsu_ports::UndecidedPrecondition> {
        vec![]
    }
}

fn flows_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut es: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    es.sort();
    for p in es {
        if p.is_dir() {
            flows_under(&p, out);
        } else if p.extension().is_some_and(|x| x == "flow") {
            out.push(p);
        }
    }
}

fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap().display().to_string()
}

/// The names the file writes after a keyword at the start of a line (`task create_intent(`).
fn declared(src: &str, keyword: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| l.strip_prefix(keyword))
        .filter_map(|r| r.strip_prefix(' '))
        .map(|r| r.split(|c: char| c == '(' || c == ' ' || c == ':').next().unwrap_or("").to_string())
        .collect()
}

#[test]
fn every_flow_of_the_examples_has_an_api_that_is_the_file() {
    let mut flows = Vec::new();
    flows_under(&root().join("examples"), &mut flows);
    flows_under(&root().join("tests/flows"), &mut flows);
    let mut seen = 0;
    let mut tasks = 0;
    for f in flows {
        let (code, out, err) = run(&["api", f.to_str().unwrap()]);
        // a flow of tests/flows that is there to fail check has no `api`
        if code != 0 {
            assert!(out.is_empty(), "{}: wrote JSON and exited {code}", rel(&f));
            assert!(code == 1 || code == 2, "{}: {code}", rel(&f));
            continue;
        }
        let j: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{}: not JSON: {e}\n{out}", rel(&f)));
        let src = std::fs::read_to_string(&f).unwrap();
        let name = |v: &Value| v["name"].as_str().unwrap().to_string();
        assert_eq!(j["dandori"], env!("CARGO_PKG_VERSION"));
        assert_eq!(j["workflow"]["name"].as_str().unwrap(), declared(&src, "workflow")[0], "{}", rel(&f));
        let names = |k: &str| -> Vec<String> { j[k].as_array().unwrap().iter().map(name).collect() };
        // the tasks, in the order the file writes them
        let written = declared(&src, "task");
        assert_eq!(names("tasks"), written, "{}", rel(&f));
        tasks += written.len();
        // the inputs and the outputs, as the file writes them under `inputs` and `outputs`
        for (key, head) in [("inputs", "inputs"), ("outputs", "outputs")] {
            let mut in_block = false;
            let mut want = Vec::new();
            for l in src.lines() {
                if l == head {
                    in_block = true;
                } else if in_block && l.starts_with("  ") {
                    want.push(l.trim().split([' ', ':']).next().unwrap().to_string());
                } else if in_block && !l.trim().is_empty() && !l.trim_start().starts_with('#') {
                    in_block = false;
                }
            }
            assert_eq!(names(key), want, "{} {key}", rel(&f));
        }
        // every `fail` the file writes is in `fails`, by its error
        let fails: Vec<String> = j["fails"].as_array().unwrap().iter().map(|x| x["error"].as_str().unwrap().to_string()).collect();
        for l in src.lines() {
            if let Some(i) = l.find("fail ").filter(|&i| l[..i].trim().is_empty() || l[..i].ends_with("=> ")) {
                let e = l[i + 5..].split_whitespace().next().unwrap();
                assert!(fails.iter().any(|x| x == e), "{}: `fail {e}` is not in fails {fails:?}", rel(&f));
            }
        }
        // no path of the machine that ran it
        let here = root().to_string_lossy().into_owned();
        assert!(!out.contains(&here), "{}: the path of the machine is in the JSON", rel(&f));
        // a `use` line for every `use` the file writes
        let uses = src.lines().filter(|l| l.starts_with("use ")).count();
        assert_eq!(j["uses"].as_array().unwrap().len(), uses, "{}", rel(&f));
        assert!(err.is_empty() || !err.contains("error"), "{}: {err}", rel(&f));
        seen += 1;
    }
    println!("compared: {seen} flows, {tasks} tasks");
    assert!(seen >= 20, "{seen}");
}

#[test]
fn what_the_api_says_of_a_task_is_what_the_task_declares() {
    let f = root().join("examples/hotel/temporal/hotel.flow");
    let (code, out, _) = run(&["api", f.to_str().unwrap()]);
    assert_eq!(code, 0);
    let j: Value = serde_json::from_str(&out).unwrap();
    let task = |n: &str| j["tasks"].as_array().unwrap().iter().find(|t| t["name"] == n).unwrap_or_else(|| panic!("no task {n}")).clone();
    let create = task("create_intent");
    assert_eq!(create["kind"], "http");
    assert_eq!(create["calls"]["method"], "POST");
    assert_eq!(create["retry"]["times"], 2);
    assert_eq!(create["retry"]["every"], 2);
    assert_eq!(create["key"], true);
    assert_eq!(create["params"][0]["range"]["text"], ">=50 <=99999999");
    assert_eq!(create["machine"]["role"], "starts");
    let confirm = task("confirm_intent");
    assert_eq!(confirm["errors"][0]["name"], "card_declined");
    assert_eq!(confirm["errors"][0]["status"], 402);
    assert_eq!(confirm["refused_as"], "unexpected_state");
    let wait = task("customer_authenticated");
    assert_eq!(wait["kind"], "event");
    assert_eq!(wait["timeout"], 3600);
    assert_eq!(j["cases"][0]["name"], "pi");
    assert_eq!(j["cases"][0]["external"], serde_json::json!(["authenticate", "settle", "expire"]));
    assert_eq!(j["on_cancel"].as_u64().is_some(), true);
}

#[test]
fn a_flow_that_does_not_pass_check_has_no_api() {
    let t = TempDir::new("api");
    let f = t.path().join("broken.flow");
    std::fs::write(&f, "workflow broken v1\n\ninputs\n  x : nothing\n\nflow\n  succeed\n").unwrap();
    let (code, out, err) = run(&["api", f.to_str().unwrap()]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.is_empty());
    assert!(err.contains("E"), "{err}");
    let (code, out, _) = run(&["api", f.to_str().unwrap(), "--format", "json"]);
    assert_eq!(code, 1);
    let j: Value = serde_json::from_str(&out).unwrap();
    assert!(j["diagnostics"].as_array().is_some_and(|d| !d.is_empty()));
    // a file that cannot be read, no file, two files
    assert_eq!(run(&["api", t.path().join("none.flow").to_str().unwrap()]).0, 2);
    assert_eq!(run(&["api"]).0, 2);
    assert_eq!(run(&["api", f.to_str().unwrap(), f.to_str().unwrap()]).0, 2);
}

#[test]
fn the_hotel_api_is_the_golden_file() {
    let f = root().join("examples/hotel/temporal/hotel.flow");
    let (code, out, _) = run(&["api", "examples/hotel/temporal/hotel.flow"]);
    assert_eq!(code, 0, "{}", f.display());
    let golden = root().join("tests/api/hotel_stay.json");
    if std::env::var_os("DANDORI_BLESS").is_some() || std::env::var_os("RITSU_BLESS").is_some() {
        std::fs::write(&golden, &out).unwrap();
        skip("the golden file is being rewritten; run this test again without DANDORI_BLESS");
        return;
    }
    let want = std::fs::read_to_string(&golden).expect("tests/api/hotel_stay.json; write it with DANDORI_BLESS=1");
    assert_eq!(out, want, "`dandori api` of the hotel example is not tests/api/hotel_stay.json; rewrite it with DANDORI_BLESS=1 and read the diff");
}
