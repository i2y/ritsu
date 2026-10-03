//! `geas snap` and `geas drift` on the greeter: quiet on unchanged code, the
//! refactor's unclaimed changes, a claimed change, a claim that cannot run during
//! drift, and no baseline.

mod common;
use common::*;

const SPEC: &str = "examples/greeter/greeter.geas";

/// A copy of the greeter with a baseline snapped from the unchanged server.
fn snapped(name: &str) -> Scratch {
    let s = Scratch::new(name);
    copy_example("greeter", &s);
    let (out, err, code) = run(s.path(), &["snap", SPEC], &[]);
    assert_eq!(err, "");
    assert_eq!(code, 0, "{out}");
    golden("en/drift/snap.txt", &out);
    assert!(s.exists("examples/greeter/.geas/greeter.baseline.jsonl"));
    s
}

#[test]
fn snap_in_japanese_and_as_json() {
    if !python3("examples/greeter") {
        return;
    }
    let s = Scratch::new("drift-snap");
    copy_example("greeter", &s);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["snap", SPEC, "--lang", "ja"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0));
    golden("ja/drift/snap.txt", &out);
    let (out, err, code) = run(s.path(), &["snap", SPEC, "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0));
    assert_eq!(json(&out).get("ok"), &Json::Bool(true));
    assert_eq!(no_process_left(&log), 8);
}

#[test]
fn drift_on_the_unchanged_greeter_is_quiet() {
    if !python3("examples/greeter") {
        return;
    }
    let s = snapped("drift-quiet");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["drift", SPEC], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(err, "");
    assert_eq!(code, 0);
    assert_eq!(no_process_left(&log), 4);
    golden("en/drift/quiet.txt", &out);
}

#[test]
fn the_refactor_drifts_where_no_claim_looks() {
    if !python3("examples/greeter") {
        return;
    }
    let s = snapped("drift-refactor");
    s.write(
        "examples/greeter/server.py",
        &s.read("examples/greeter/server_refactored.py"),
    );
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", SPEC];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!(err, "");
        assert_eq!(code, 1);
        golden(&format!("{lang}/drift/refactored.txt"), &out);
    }
    let (out, err, code) = run(s.path(), &["drift", SPEC, "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 1));
    let v = json(&out);
    assert_eq!(v.get("unclaimed").num(), 7.0);
    assert_eq!(v.get("changes").arr()[0].get("new"), &Json::Obj(vec![("handler".into(), Json::Str("greet_v2".into()))]));
    golden("en/drift/refactored.json", &out);
    no_process_left(&log);
}

#[test]
fn a_total_off_by_one_is_a_claimed_change() {
    if !python3("examples/greeter") {
        return;
    }
    let s = snapped("drift-off-by-one");
    let server = s.read("examples/greeter/server.py");
    let total = "self._send(200, str(total))";
    assert!(server.contains(total), "server.py no longer answers /total this way");
    s.write(
        "examples/greeter/server.py",
        &server.replace(total, "self._send(200, str(total + 1))"),
    );
    let (out, err, code) = run(s.path(), &["drift", SPEC], &[]);
    assert_eq!(err, "");
    assert_eq!(code, 1);
    golden("en/drift/off-by-one.txt", &out);
}

/// A claim that cannot run during drift is shown as the error it is, and drift
/// exits 2: what it did not run, it did not compare.
#[test]
fn a_claim_that_cannot_run_stops_drift_with_2() {
    if !python3("examples/greeter") {
        return;
    }
    let s = snapped("drift-error");
    s.write(
        "examples/greeter/server.py",
        "import sys\nsys.stderr.write(\"ImportError: no module named 'greetings'\\n\")\nsys.exit(1)\n",
    );
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", SPEC];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!(err, "");
        assert_eq!(code, 2);
        golden(&format!("{lang}/drift/claim-error.txt"), &out);
    }
    let (out, _, code) = run(s.path(), &["drift", SPEC, "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 2);
    assert_eq!(json(&out).get("diagnostics").arr().len(), 4);
    no_process_left(&log);
}

#[test]
fn drift_without_a_baseline_stops_before_running() {
    let s = Scratch::new("drift-no-baseline");
    copy_example("greeter", &s);
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", SPEC];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!(out, "");
        assert_eq!(code, 2);
        golden(&format!("{lang}/drift/no-baseline.txt"), &err);
    }
    assert!(!s.exists("examples/greeter/.geas"), "drift ran something without a baseline");
}
