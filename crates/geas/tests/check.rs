//! `geas check` on the examples: the reports, the JSON, the journals, a claim that
//! fails, and a claim that cannot run.

mod common;
use common::*;

#[test]
fn calc_holds() {
    if !python3("examples/calc") {
        return;
    }
    let s = Scratch::new("check-calc");
    copy_example("calc", &s);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "examples/calc/calc.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(err, "");
    assert_eq!(code, 0);
    assert_eq!(no_process_left(&log), 4, "one process per `when`");
    golden("en/check/calc.txt", &out);
    golden("en/check/calc.journal.jsonl", &s.read("examples/calc/.geas/calc.journal.jsonl"));
}

#[test]
fn greeter_holds() {
    if !python3("examples/greeter") {
        return;
    }
    let s = Scratch::new("check-greeter");
    copy_example("greeter", &s);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "examples/greeter/greeter.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(err, "");
    assert_eq!(code, 0);
    assert_eq!(no_process_left(&log), 4, "one service per claim");
    golden("en/check/greeter.txt", &out);
    golden(
        "en/check/greeter.journal.jsonl",
        &s.read("examples/greeter/.geas/greeter.journal.jsonl"),
    );
}

#[test]
fn greeter_as_json() {
    if !python3("examples/greeter") {
        return;
    }
    let s = Scratch::new("check-greeter-json");
    copy_example("greeter", &s);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(
        s.path(),
        &["check", "examples/greeter/greeter.geas", "--json"],
        &[("GEAS_PID_LOG", log_s)],
    );
    assert_eq!(err, "");
    assert_eq!(code, 0);
    no_process_left(&log);
    let v = json(&out);
    assert_eq!(v.get("geas").num(), 1.0);
    assert_eq!(v.get("claims").arr().len(), 4);
    golden("en/check/greeter.json", &out);
}

/// The greeter as an agent might first write it: no check for an empty name.
fn greeter_without_the_empty_name_check(s: &Scratch) {
    copy_example("greeter", s);
    let server = s.read("examples/greeter/server.py");
    let check = "            if not name:\n                self._send(400, \"name required\")\n                return\n";
    assert!(server.contains(check), "server.py no longer has the empty-name check");
    s.write("examples/greeter/server.py", &server.replace(check, ""));
}

#[test]
fn greeter_without_the_empty_name_check_fails_claim_2() {
    if !python3("examples/greeter") {
        return;
    }
    let s = Scratch::new("check-greeter-broken");
    greeter_without_the_empty_name_check(&s);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, args) in [
        ("en", vec!["check", "examples/greeter/greeter.geas"]),
        ("ja", vec!["check", "examples/greeter/greeter.geas", "--lang", "ja"]),
    ] {
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!(err, "");
        assert_eq!(code, 1);
        assert!(out.contains("not ok 2 - rejects an empty name"), "{out}");
        golden(&format!("{lang}/check/greeter-no-empty-name-check.txt"), &out);
    }
    let (out, _, code) = run(s.path(), &["check", "examples/greeter/greeter.geas", "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 1);
    let v = json(&out);
    let claim = &v.get("claims").arr()[1];
    assert_eq!(claim.get("status").str(), "fail");
    assert_eq!(claim.get("run").arr()[0].get("call").str(), "api.get(\"/greet?name=\")");
    golden("en/check/greeter-no-empty-name-check.json", &out);
    no_process_left(&log);
}

/// A service that exits before it opens its port makes its claim an error, not a
/// failed check; the run that gets there shows the `when` before it.
#[test]
fn a_service_that_exits_at_once_is_an_error() {
    if !python3("a service written in Python") {
        return;
    }
    let _port = port_lock();
    let s = Scratch::new("check-serve-exits");
    s.write(
        "exits/exits.geas",
        "target api {\n  serve \"python3 exits.py\"\n  port 8123\n}\n\ntarget hello {\n  run \"python3 -c print(42)\"\n}\n\nclaim \"answers\" {\n  when hello.run()\n  when api.get(\"/\")\n  then status is 200\n}\n",
    );
    s.write(
        "exits/exits.py",
        "import sys\nsys.stderr.write(\"cannot listen: address already in use\\n\")\nsys.exit(3)\n",
    );
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, args) in [
        ("en", vec!["check", "exits/exits.geas"]),
        ("ja", vec!["check", "exits/exits.geas", "--lang", "ja"]),
    ] {
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!(err, "");
        assert_eq!(code, 1);
        golden(&format!("{lang}/check/serve-exits.txt"), &out);
    }
    let (out, _, code) = run(s.path(), &["check", "exits/exits.geas", "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 1);
    let v = json(&out);
    let claim = &v.get("claims").arr()[0];
    assert_eq!(claim.get("error").get("code").str(), "E032");
    assert_eq!(claim.get("run").arr()[1].get("observed"), &Json::Null);
    golden("en/check/serve-exits.json", &out);
    golden("en/check/serve-exits.journal.jsonl", &s.read("exits/.geas/exits.journal.jsonl"));
    no_process_left(&log);
}

/// The run that gets there goes up to the last check that failed, and no further.
#[test]
fn the_run_goes_up_to_the_last_failed_check() {
    let s = Scratch::new("check-run");
    s.write(
        "say.geas",
        "target say {\n  run \"echo\"\n}\n\nclaim \"says what it is told\" {\n  when say.run(\"one\")\n  then stdout is \"uno\"\n  when say.run(\"two\")\n  then stdout is \"two\"\n  when say.run(\"three\")\n  then stdout is \"tres\"\n  when say.run(\"four\")\n  then stdout is \"four\"\n}\n",
    );
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "say.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 1));
    assert!(out.contains("say.run(\"three\")") && !out.contains("say.run(\"four\")"), "{out}");
    golden("en/check/run-to-the-last-failure.txt", &out);
    assert_eq!(no_process_left(&log), 4);
}
