//! Pins (DESIGN §12): what each one gives the processes of a target, the journal's
//! `env` event, `env clean` under `geas map` (the coverage switches still reach the
//! program), and drift's note when the pins changed since the baseline. The refused
//! pins (E003, E010, E011) are with the other static codes, under `tests/specs/`.

mod common;
use common::*;

/// The variables the test sets for geas: one `env clean` must drop, one `env pass`
/// keeps.
const FROM_TEST: [(&str, &str); 2] = [("GEAS_FROM_TEST", "from the test"), ("GEAS_PASSED", "passed")];

fn with_fixtures(name: &str) -> Scratch {
    let s = Scratch::new(name);
    s.write("p/env.py", &repo_file("tests/impl/env.py"));
    s.write("p/pins.geas", &repo_file("tests/pins/pins.geas"));
    s
}

#[test]
fn each_pin_reaches_the_processes_of_its_targets() {
    if !python3("the pins' fixture") {
        return;
    }
    let s = with_fixtures("pins");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let mut env = FROM_TEST.to_vec();
    env.push(("GEAS_PID_LOG", log_s));
    let (out, err, code) = run(s.path(), &["check", "p/pins.geas"], &env);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/pins/report.txt", &out);
    // the `env` event comes at a claim's first use of a target that has pins
    let journal = s.read("p/.geas/pins.journal.jsonl");
    assert_eq!(journal.matches("\"event\":\"env\"").count(), 3, "{journal}");
    golden("en/pins/journal.jsonl", &journal);
    assert_eq!(no_process_left(&log), 4);
}

/// Under `geas map`, the coverage switches go on top of what the pins give: a
/// Python target with `env clean` is still recorded.
#[test]
fn env_clean_keeps_the_coverage_switches_in_map() {
    if !python3("the pins' fixture") {
        return;
    }
    let s = with_fixtures("pins-map");
    let (out, err, code) = run(s.path(), &["map", "p/pins.geas", "--root", "p"], &FROM_TEST);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let record = s.read("p/.geas/pins.map.jsonl");
    for target in ["plain", "pinned", "clean"] {
        assert!(record.contains(&format!("\"target\":\"{target}\",\"file\":\"env.py\"")), "{target}: {record}");
    }
    golden("en/pins/map.jsonl", &record);
}

/// After a snap, the time zone outside any target changes: drift notes the pins of
/// the two targets that take it, and the output that changed with them.
#[test]
fn drift_notes_pins_that_changed_since_the_baseline() {
    if !python3("the pins' fixture") {
        return;
    }
    let s = with_fixtures("pins-drift");
    let (out, err, code) = run(s.path(), &["snap", "p/pins.geas"], &FROM_TEST);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let baseline = s.read("p/.geas/pins.baseline.jsonl");
    assert!(baseline.starts_with("{\"geas_baseline\":1,\"pins\":{\"plain\":"), "{baseline}");
    golden("en/pins/baseline.jsonl", &baseline);
    let spec = s.read("p/pins.geas");
    assert!(spec.contains("\ntz \"UTC\"\n"));
    s.write("p/pins.geas", &spec.replace("\ntz \"UTC\"\n", "\ntz \"Europe/Berlin\"\n"));
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", "p/pins.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &FROM_TEST);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/pins/drift.txt"), &out);
    }
    // a baseline from before pins has no first line, and is read as one without pins
    let old: String = baseline.lines().skip(1).map(|l| format!("{l}\n")).collect();
    s.write("p/.geas/pins.baseline.jsonl", &old);
    let (out, _, code) = run(s.path(), &["drift", "p/pins.geas"], &FROM_TEST);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("note: the pins of `pinned` differ from the baseline's: {} → {"), "{out}");
}
