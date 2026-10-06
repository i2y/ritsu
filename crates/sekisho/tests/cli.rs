//! The command (DESIGN 11): what `sekisho` prints and exits with, as `ritsu sekisho` runs it (every
//! language joined) and as the binary of sekisho's own crate runs it (none joined: E209, exit 2).
//! The pages of `--help` are golden files in `tests/golden/cli/`.

mod common;

use sekisho::suite::Suite;

/// The command run as a function: the exit code, what it printed, and what it printed on stderr.
fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn version_and_help() {
    let (code, out, _) = run(&["--version"], Suite::default());
    assert_eq!((code, out.as_str()), (0, format!("sekisho {}\n", env!("CARGO_PKG_VERSION")).as_str()));
    for (lang, tag) in [("en", "en"), ("ja", "ja")] {
        let (code, out, _) = run(&["--help", "--lang", lang], Suite::default());
        assert_eq!(code, 0);
        ritsu_testkit::golden(format!("tests/golden/cli/help.{tag}.txt"), &out.replace(env!("CARGO_PKG_VERSION"), "<version>"));
        for cmd in ["check", "explain"] {
            let (code, out, _) = run(&[cmd, "--help", "--lang", lang], Suite::default());
            assert_eq!(code, 0);
            ritsu_testkit::golden(format!("tests/golden/cli/{cmd}.{tag}.txt"), &out);
            let (code, again, _) = run(&["help", cmd, "--lang", lang], Suite::default());
            assert_eq!((code, again), (0, out));
        }
    }
}

#[test]
fn check_exits_by_what_it_finds() {
    // every language joined: the example passes
    let (code, out, _) = run(&["check", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"], common::joined());
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.lines().filter(|l| l.contains(": ok — ")).count(), 2, "{out}");
    // an error
    let (code, out, _) = run(&["check", "tests/mutants/E101_unknown_role.gate"], common::joined());
    assert_eq!(code, 1, "{out}");
    // a language not joined: E209, exit 2
    let (code, out, _) = run(&["check", "examples/refunds/refunds.gate"], Suite::default());
    assert_eq!(code, 2);
    assert!(out.contains("error[E209]") && out.contains("ritsu sekisho check examples/refunds/refunds.gate"), "{out}");
    // a file that is not there, a budget that is no positive number, a flag it does not take
    for args in [&["check", "tests/no-such.gate"][..], &["check", "--budget", "0", "examples/refunds/refunds.gate"], &["check", "--fast", "x.gate"], &["frobnicate"], &["check"]] {
        let (code, _, err) = run(args, common::joined());
        assert_eq!(code, 2, "{args:?}");
        assert!(err.starts_with("error: "), "{args:?}: {err}");
    }
}

#[test]
fn check_in_json() {
    let (code, out, _) = run(&["check", "tests/mutants/E101_unknown_role.gate", "--format", "json"], common::joined());
    assert_eq!(code, 1);
    let v = ritsu_base::json::parse(&out).unwrap();
    let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["file", "ok", "summary", "diagnostics"]);
    assert_eq!(v.get("diagnostics").and_then(|d| d.as_arr()).map(|d| d.len()), Some(1));
    let (code, out, _) = run(&["check", "examples/refunds/refunds.gate", "--format", "json", "--lang", "ja"], common::joined());
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    assert_eq!(v.get("summary").and_then(|s| s.as_str()), Some("action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1"));
}

#[test]
fn explain() {
    let (code, out, _) = run(&["explain", "e101"], Suite::default());
    assert_eq!(code, 0);
    assert!(out.starts_with("E101 (error) — A name names nothing\n"), "{out}");
    let (code, out, _) = run(&["explain", "E101", "--lang", "ja"], Suite::default());
    assert_eq!(code, 0);
    assert!(out.contains("principal in 系"), "the Japanese example: {out}");
    let (code, out, _) = run(&["explain", "--all", "--format", "markdown"], Suite::default());
    assert_eq!((code, out), (0, std::fs::read_to_string("docs/codes.md").unwrap()));
    let (code, out, _) = run(&["explain", "--all", "--format", "markdown", "--lang", "ja"], Suite::default());
    assert_eq!((code, out), (0, std::fs::read_to_string("docs/codes.ja.md").unwrap()));
    let (code, out, _) = run(&["explain", "E101", "--format", "json"], Suite::default());
    assert_eq!(code, 0);
    assert_eq!(ritsu_base::json::parse(&out).unwrap().get("code").and_then(|c| c.as_str()), Some("E101"));
    let (code, _, err) = run(&["explain", "E999"], Suite::default());
    assert_eq!(code, 2);
    assert!(err.contains("E999"));
}

/// The binary itself: no other language is in it.
#[test]
fn the_binary_of_the_crate() {
    let bin = env!("CARGO_BIN_EXE_sekisho");
    let out = std::process::Command::new(bin).args(["check", "examples/refunds/refunds.gate"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("error[E209]"), "{text}");
    let out = std::process::Command::new(bin).args(["check", "tests/mutants/E101_unknown_role.gate"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let out = std::process::Command::new(bin).arg("--version").output().unwrap();
    assert_eq!((out.status.code(), String::from_utf8_lossy(&out.stdout).to_string()), (Some(0), format!("sekisho {}\n", env!("CARGO_PKG_VERSION"))));
}

/// What `ritsu check` prints for the files of sekisho (`check::checked`): what `sekisho check`
/// prints, a finding at a time, and a verdict for each file.
#[test]
fn checked_as_ritsu_check_prints_it() {
    let root = std::env::current_dir().unwrap();
    let files = vec!["examples/refunds/refunds.gate".to_string(), "tests/mutants/E101_unknown_role.gate".to_string(), "tests/no-such.gate".to_string()];
    let units = sekisho::check::checked(&root, &files, &common::joined(), ritsu_base::text::Lang::En);
    let verdicts: Vec<ritsu_ports::Verdict> = units.iter().map(|u| u.verdict).collect();
    assert_eq!(verdicts, vec![ritsu_ports::Verdict::Passes, ritsu_ports::Verdict::Fails, ritsu_ports::Verdict::Unchecked]);
    let (_, out, _) = run(&["check", "tests/mutants/E101_unknown_role.gate"], common::joined());
    assert_eq!(units[1].text(), out);
    let f = units[1].findings().next().unwrap();
    assert_eq!((f.code.as_str(), f.file.as_deref()), ("E101", Some("tests/mutants/E101_unknown_role.gate")));
    // a language not joined: not checked, as the command exits 2
    let units = sekisho::check::checked(&root, &files[..1], &Suite::default(), ritsu_base::text::Lang::En);
    assert_eq!(units[0].verdict, ritsu_ports::Verdict::Unchecked);
}
