//! GUI targets through the driver protocol (DESIGN §8.5), held to it with a fake
//! driver of a few dozen lines of Python (`tests/drivers/fake_driver.py`) playing
//! a small greeter: screen checks that hold and fail, with the screen in the
//! report; a refused action (E035); answers geas cannot read, none at all, and a
//! driver that exits (E037); a pin the driver refuses (E011); a command between two
//! actions; drift on screens, claimed, unclaimed and masked; `--json`.

mod common;
use common::*;

/// A scratch directory with a spec from `tests/gui/`, the fake driver, its app and
/// the echo fixture, under `g/`.
fn with_driver(name: &str, spec: &str) -> Scratch {
    let s = Scratch::new(name);
    s.write("g/fake_driver.py", &repo_file("tests/drivers/fake_driver.py"));
    s.write("g/greeter.json", &repo_file("tests/drivers/greeter.json"));
    s.write("g/echo.py", &repo_file("tests/impl/echo.py"));
    s.write(&format!("g/{spec}"), &repo_file(&format!("tests/gui/{spec}")));
    s
}

/// Runs geas with a pid log, and holds it to stopping every driver it started.
fn run_logged(s: &Scratch, args: &[&str]) -> (String, String, i32) {
    let log = pid_log(s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let r = run(s.path(), args, &[("GEAS_PID_LOG", log_s)]);
    no_process_left(&log);
    r
}

#[test]
fn screen_checks_hold_and_fail_with_the_screen_in_the_report() {
    if !python3("the fake driver") {
        return;
    }
    let s = with_driver("gui", "greeter.geas");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "g/greeter.geas"];
        args.extend(extra);
        let (out, err, code) = run_logged(&s, &args);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/gui/greeter.txt"), &out);
    }
    golden("en/gui/greeter.journal.jsonl", &s.read("g/.geas/greeter.journal.jsonl"));
    let (out, err, code) = run_logged(&s, &["check", "g/greeter.geas", "--json"]);
    assert_eq!((err.as_str(), code), ("", 1));
    let v = json(&out);
    let claims = v.get("claims").arr();
    let status: Vec<&str> = claims.iter().map(|c| c.get("status").str()).collect();
    assert_eq!(status, ["ok", "fail", "ok"]);
    // a failed screen check carries the screen it saw
    let failed = &claims[1].get("checks").arr()[0];
    assert_eq!(failed.get("actual").str(), "no such node");
    assert!(matches!(failed.get("screen"), Json::Obj(_)));
    golden("en/gui/greeter.json", &out);
    // -j2 prints and writes what -j1 does
    let one = (run_logged(&s, &["check", "g/greeter.geas"]), s.read("g/.geas/greeter.journal.jsonl"));
    let two = (run_logged(&s, &["check", "g/greeter.geas", "-j2"]), s.read("g/.geas/greeter.journal.jsonl"));
    assert_eq!(one, two);
}

/// Each way a driver's claim can end in an error, in both languages.
#[test]
fn a_refused_action_an_unreadable_answer_a_driver_gone_and_a_refused_pin() {
    if !python3("the fake driver") {
        return;
    }
    for (spec, code) in [("refused", "E035"), ("garbled", "E037"), ("gone", "E037"), ("pinned", "E011")] {
        let s = with_driver(&format!("gui-{spec}"), &format!("{spec}.geas"));
        for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
            let file = format!("g/{spec}.geas");
            let mut args = vec!["check", file.as_str()];
            args.extend(extra);
            let (out, err, exit) = run_logged(&s, &args);
            assert_eq!((err.as_str(), exit), ("", 1), "{out}");
            assert!(out.contains(&format!("[{code}]")), "{spec}: {out}");
            golden(&format!("{lang}/gui/{spec}.txt"), &out);
        }
    }
}

/// A driver that does not answer is stopped after 5 s.
#[test]
fn a_driver_that_does_not_answer() {
    if !python3("the fake driver") {
        return;
    }
    let s = with_driver("gui-silent", "silent.geas");
    let started = std::time::Instant::now();
    let (out, err, code) = run_logged(&s, &["check", "g/silent.geas"]);
    assert_eq!((err.as_str(), code), ("", 1), "{out}");
    assert!(started.elapsed().as_secs() >= 5);
    golden("en/gui/silent.txt", &out);
}

/// After a snap, the app greets with other words and shows a version: the text a
/// check names went (claimed), the new one came (unclaimed, but claimed where a
/// check names it by a part), and so did the version, which a mask then hides.
#[test]
fn drift_on_screens() {
    if !python3("the fake driver") {
        return;
    }
    let s = with_driver("gui-drift", "greeter.geas");
    let (out, err, code) = run_logged(&s, &["snap", "g/greeter.geas"]);
    assert_eq!((err.as_str(), code), ("", 1), "{out}");
    let baseline = s.read("g/.geas/greeter.baseline.jsonl");
    assert!(baseline.contains("\"kind\":\"screen\""), "{baseline}");
    let (out, err, code) = run_logged(&s, &["drift", "g/greeter.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "unchanged, drift is quiet: {out}");
    let app = s.read("g/greeter.json");
    let hello = "{\"role\": \"text\", \"name\": \"Hello, {1}!\"},";
    assert!(app.contains(hello), "greeter.json no longer has the greeting as the test writes it");
    s.write("g/greeter.json", &app.replace(hello, "{\"role\": \"text\", \"name\": \"Hi, {1}!\"},\n        {\"role\": \"text\", \"name\": \"version 2\"},"));
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", "g/greeter.geas"];
        args.extend(extra);
        let (out, err, code) = run_logged(&s, &args);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/gui/drift.txt"), &out);
    }
    let (out, _, _) = run_logged(&s, &["drift", "g/greeter.geas", "--json"]);
    let v = json(&out);
    assert_eq!((v.get("claimed").num(), v.get("unclaimed").num()), (3.0, 6.0));
    // a mask takes the version out of drift, with no new snap
    let spec = s.read("g/greeter.geas");
    s.write("g/greeter.geas", &spec.replace("target echo {", "mask screen text containing \"version\"\n\ntarget echo {"));
    let (out, err, code) = run_logged(&s, &["drift", "g/greeter.geas"]);
    assert_eq!((err.as_str(), code), ("", 1), "{out}");
    assert!(!out.contains("version"), "{out}");
    golden("en/gui/drift-masked.txt", &out);
    // and the journal writes the masked node as its role alone
    run_logged(&s, &["check", "g/greeter.geas"]);
    let journal = s.read("g/.geas/greeter.journal.jsonl");
    assert!(journal.contains("{\"role\":\"text\",\"masked\":true}"), "{journal}");
    assert!(!journal.contains("version 2"), "{journal}");
}
