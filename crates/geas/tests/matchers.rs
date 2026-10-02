//! The matchers and the `header` subject (DESIGN §9): every matcher on every kind
//! of subject it takes, holding once and failing once (`tests/matchers/`), and
//! drift claiming a header's change when a check names it. The refused forms
//! (E008, E009) are with the other static codes, under `tests/specs/`.

mod common;
use common::*;

/// A scratch directory holding a spec from `tests/matchers/` and the fixtures it
/// runs.
fn with_fixtures(name: &str, spec: &str) -> Scratch {
    let s = Scratch::new(name);
    for f in ["echo.py", "answer.py"] {
        s.write(&format!("m/{f}"), &repo_file(&format!("tests/impl/{f}")));
    }
    s.write(&format!("m/{spec}"), &repo_file(&format!("tests/matchers/{spec}")));
    s
}

#[test]
fn every_matcher_holds_once_and_fails_once() {
    if !python3("the matchers' fixtures") {
        return;
    }
    let s = with_fixtures("matchers", "matchers.geas");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "m/matchers.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/matchers/report.txt"), &out);
    }
    // every claim named "… holds" holds, and every one named "… fails" fails
    let (out, _, code) = run(s.path(), &["check", "m/matchers.geas", "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 1);
    let v = json(&out);
    for c in v.get("claims").arr() {
        let name = c.get("name").str();
        let want = if name.ends_with(" holds") { "ok" } else { "fail" };
        assert_eq!(c.get("status").str(), want, "{name}");
    }
    golden("en/matchers/report.json", &out);
    golden("en/matchers/journal.jsonl", &s.read("m/.geas/matchers.journal.jsonl"));
    no_process_left(&log);
}

/// After a snap, the service changes the header a check names and adds one no
/// check names: the first change is claimed, the second unclaimed.
#[test]
fn a_header_change_is_claimed_when_a_check_names_it() {
    if !python3("the matchers' fixtures") {
        return;
    }
    let s = with_fixtures("matchers-drift", "drift.geas");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["snap", "m/drift.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/matchers/drift.baseline.jsonl", &s.read("m/.geas/drift.baseline.jsonl"));
    let answer = s.read("m/answer.py");
    let version = "    \"X-Version\": \"1.2.3\",\n";
    assert!(answer.contains(version), "answer.py no longer sends X-Version 1.2.3");
    s.write("m/answer.py", &answer.replace(version, "    \"X-Version\": \"1.3.0\",\n    \"X-Debug\": \"on\",\n"));
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", "m/drift.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/matchers/drift.txt"), &out);
    }
    let (out, _, code) = run(s.path(), &["drift", "m/drift.geas", "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 1);
    let v = json(&out);
    assert_eq!((v.get("claimed").num(), v.get("unclaimed").num()), (1.0, 1.0));
    no_process_left(&log);
}
