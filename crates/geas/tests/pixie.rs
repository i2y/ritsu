//! pixie, the replayed driver (DESIGN §8.3), on pixie's greeter: typing, clicking
//! and Enter with screen checks that hold; a check that fails, with the screen; a
//! refused click (E035) that names what the screen had to click; text holding a
//! comma; `open()` alone; drift quiet on the unchanged app. Run when
//! `GEAS_PIXIE_GREETER` names a built greeter, which the test links into its
//! scratch directory as `greeter`; SKIP otherwise. E012 and E013 need no app and
//! are with the other static codes, under `tests/specs/`.

mod common;
use common::*;
use std::path::PathBuf;

/// The greeter to run, or None after printing why the test does not run.
fn greeter() -> Option<PathBuf> {
    if !ritsu_testkit::need(ritsu_testkit::Need::Pixie) {
        return None;
    }
    match std::env::var_os("GEAS_PIXIE_GREETER").map(PathBuf::from) {
        Some(p) if p.is_file() => Some(p),
        Some(p) => {
            skip(&format!("GEAS_PIXIE_GREETER names {}, which is not a file; the pixie tests are not run", p.display()));
            None
        }
        None => {
            skip("GEAS_PIXIE_GREETER is not set to a built pixie greeter; the pixie tests are not run");
            None
        }
    }
}

/// A scratch directory with a spec from `tests/pixie/` and the app beside it.
fn with_app(name: &str, spec: &str, app: &PathBuf) -> Scratch {
    let s = Scratch::new(name);
    s.write(&format!("p/{spec}"), &repo_file(&format!("tests/pixie/{spec}")));
    std::os::unix::fs::symlink(app, s.path().join("p/greeter")).expect("link the greeter");
    s
}

fn run_logged(s: &Scratch, args: &[&str]) -> (String, String, i32) {
    let log = pid_log(s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let r = run(s.path(), args, &[("GEAS_PID_LOG", log_s)]);
    no_process_left(&log);
    r
}

#[test]
fn typing_clicking_and_enter_hold() {
    let Some(app) = greeter() else {
        return;
    };
    let s = with_app("pixie", "greeter.geas", &app);
    let (out, err, code) = run_logged(&s, &["check", "p/greeter.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/pixie/greeter.txt", &out);
    golden("en/pixie/greeter.journal.jsonl", &s.read("p/.geas/greeter.journal.jsonl"));
    // the transcripts pixie wrote are gone
    let left: Vec<String> = std::fs::read_dir(s.path().join("p/.geas"))
        .expect("the .geas directory")
        .map(|e| e.expect("an entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, ["greeter.journal.jsonl"]);
    // drift on the unchanged app is quiet
    let (out, err, code) = run_logged(&s, &["snap", "p/greeter.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let (out, err, code) = run_logged(&s, &["drift", "p/greeter.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.ends_with("drift: 6 interactions compared · 0 drifted · 0 unclaimed change(s) · 0 claimed\n"), "{out}");
    // and -j4 prints what -j1 prints
    let (four, _, _) = run_logged(&s, &["check", "p/greeter.geas", "-j4"]);
    let (one, _, _) = run_logged(&s, &["check", "p/greeter.geas"]);
    assert_eq!(four, one);
}

#[test]
fn a_check_that_fails_and_a_refused_click() {
    let Some(app) = greeter() else {
        return;
    };
    let s = with_app("pixie-fails", "fails.geas", &app);
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "p/fails.geas"];
        args.extend(extra);
        let (out, err, code) = run_logged(&s, &args);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        assert!(out.contains("[E035]") && out.contains("button \"greet\""), "{out}");
        golden(&format!("{lang}/pixie/fails.txt"), &out);
    }
}
