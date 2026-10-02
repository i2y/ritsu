//! The command line: help, version, where flags may go, several specs at once, and
//! what each spec writes beside it.

mod common;
use common::*;

#[test]
fn help_in_both_languages() {
    let s = Scratch::new("cli-help");
    for (lang, args) in [("en", vec!["--help"]), ("ja", vec!["-h", "--lang", "ja"])] {
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((err.as_str(), code), ("", 0));
        golden(&format!("{lang}/cli/help.txt"), &out);
    }
    // --help wins over a command, wherever it is
    let (out, _, code) = run(s.path(), &["check", "x.geas", "--help"], &[]);
    assert_eq!(code, 0);
    assert!(out.starts_with("geas: hold agent-written code"), "{out}");
}

#[test]
fn version() {
    let s = Scratch::new("cli-version");
    let (out, err, code) = run(s.path(), &["--version"], &[]);
    assert_eq!((err.as_str(), code), ("", 0));
    assert_eq!(out, format!("geas {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn a_flag_the_command_does_not_take() {
    let s = Scratch::new("cli-flag");
    let (out, err, code) = run(s.path(), &["drift", "x.geas", "--all"], &[]);
    assert_eq!((out.as_str(), code), ("", 2));
    assert!(err.starts_with("error[E080]: `--all` is not an option of `geas drift`"), "{err}");
}

/// Two specs in one directory keep two journals, the exit status is the worst of
/// the two, and flags may come before the command.
#[test]
fn two_specs_in_one_directory() {
    let s = Scratch::new("cli-two");
    s.write("d/echo.geas", "target echo {\n  run \"echo hello\"\n}\n\nclaim \"says hello\" {\n  when echo.run()\n  then stdout is \"hello\"\n}\n");
    s.write("d/fail.geas", "target f {\n  run \"false\"\n}\n\nclaim \"succeeds\" {\n  when f.run()\n  then exit is 0\n}\n");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["--lang", "en", "check", "d/echo.geas", "d/fail.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 1));
    golden("en/cli/two-specs.txt", &out);
    assert!(s.exists("d/.geas/echo.journal.jsonl"));
    assert!(s.exists("d/.geas/fail.journal.jsonl"));
    assert!(!s.exists("d/.geas/journal.jsonl"));
    // as JSON, one object a line
    let (out, _, code) = run(s.path(), &["check", "--json", "d/echo.geas", "d/fail.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 1);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(json(lines[0]).get("ok"), &Json::Bool(true));
    assert_eq!(json(lines[1]).get("ok"), &Json::Bool(false));
    // a spec that does not parse beside one that runs: 2 wins
    s.write("d/bad.geas", "claim \"x\" {\n}\n");
    let (_, err, code) = run(s.path(), &["check", "d/echo.geas", "d/bad.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 2);
    assert!(err.contains("error[E005]"), "{err}");
    assert_eq!(no_process_left(&log), 5);
}

/// A spec in the working directory writes `.geas/` there, named after it.
#[test]
fn a_spec_in_the_working_directory() {
    let s = Scratch::new("cli-here");
    s.write("here.geas", "target echo {\n  run \"echo hi\"\n}\n\nclaim \"says hi\" {\n  when echo.run()\n  then stdout is \"hi\"\n}\n");
    let (out, err, code) = run(s.path(), &["check", "here.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0));
    assert_eq!(out, "ok 1 - says hi\n1 claim · 1 ok · 0 failed · journal: .geas/here.journal.jsonl\n");
    assert!(s.exists(".geas/here.journal.jsonl"));
}
