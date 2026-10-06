//! The one entry (DESIGN 8.1, 8.2): `ritsu --help` and `--version`, `ritsu <language>` for each of
//! the eight, and ritsu called by a language's name.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// `program` (ritsu, or a link to it), run in `dir`, with no language asked of the environment.
fn run_in(program: &Path, dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(program);
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run it");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn ritsu(args: &[&str]) -> (i32, String, String) {
    run_in(Path::new(env!("CARGO_BIN_EXE_ritsu")), &crates(), args)
}

/// What `ritsu` has, and what it says of what it has not.
#[test]
fn ritsu_says_what_it_has() {
    let (code, out, _) = ritsu(&["--version"]);
    assert_eq!((code, out), (0, format!("ritsu {}\n", env!("CARGO_PKG_VERSION"))));
    let (code, out, err) = ritsu(&["--help"]);
    assert!(code == 0 && err.is_empty(), "{out}");
    for line in ["ritsu check [<path>...]", "ritsu rulec <command> ...", "ritsu dandori <command> ...", "ritsu sakai <command> ...", "ritsu sekisho <command> ...", "Write the rules a system must follow in small languages"] {
        assert!(out.contains(line), "{line}: {out}");
    }
    let (code, out, _) = ritsu(&["--lang", "ja", "--help"]);
    assert!(code == 0 && out.contains("システムが守るべき決まりを小さな言語で書き") && out.contains("ritsu check [<path>...]"), "{out}");
    let (code, out, err) = ritsu(&[]);
    assert!(code == 2 && out.is_empty() && err.contains("Usage:"), "{err}");
    assert_eq!(ritsu(&["nope"]), (2, String::new(), "error: there is no command `nope`; run `ritsu --help`\n".into()));
    let (code, out, _) = ritsu(&["check", "--help"]);
    assert!(code == 0 && out.starts_with("ritsu check — check the files of a project"), "{out}");
    let (code, out, _) = ritsu(&["help", "check", "--lang", "ja"]);
    assert!(code == 0 && out.starts_with("ritsu check — "), "{out}");
}

/// `ritsu <language>` is the language's own command, for every one of the eight.
#[test]
fn ritsu_is_every_language() {
    for name in ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "sekisho"] {
        let (code, out, err) = ritsu(&[name, "--version"]);
        assert!(code == 0 && out.starts_with(&format!("{name} ")) && out.lines().count() == 1, "{name}: {out}{err}");
    }
    let rule = "ritsu/tests/projects/通販/delivery/rules/出荷の急ぎ.rule";
    assert_eq!(ritsu(&["rulec", "check", rule]), (0, format!("ok {rule}\n"), String::new()));
    let cal = "ritsu/tests/projects/通販/delivery/出荷日.cal";
    let (code, out, _) = ritsu(&["koyomi", "check", cal, "--lang", "ja"]);
    assert!(code == 0 && out.starts_with(&format!("{cal}: ok — ")), "{out}");
    let (code, out, _) = ritsu(&["chobo", "explain", "E014"]);
    assert!(code == 0 && out.starts_with("error[E014]: A tax on a unit that is not money"), "{out}");
    let (code, _, err) = ritsu(&["geas", "check"]);
    assert!(code == 2 && err.contains("`geas check` needs at least one spec"), "{err}");
    // a gate that takes a rule's answer and dates as conditions: the languages it reads are joined
    let gate = "sekisho/examples/refunds/refunds.gate";
    let (code, out, err) = ritsu(&["sekisho", "check", gate, "--root", "sekisho/examples/refunds"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, format!("{gate}: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation\n").as_str(), ""));
}

/// Called by a language's name, ritsu is that command: what the release's links do (DESIGN 2.3).
#[test]
fn ritsu_called_by_a_language_s_name() {
    let t = TempDir::new("links");
    for name in ["koyomi", "rulec"] {
        let link = t.path().join(name);
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ritsu"), &link).unwrap();
        let (code, out, _) = run_in(&link, &crates(), &["--version"]);
        assert!(code == 0 && out.starts_with(&format!("{name} ")), "{name}: {out}");
    }
    let (code, out, _) = run_in(&t.path().join("rulec"), &crates(), &["check", "ritsu/tests/projects/通販/billing/rules/決済手数料.rule"]);
    assert!(code == 0 && out.starts_with("ok "), "{out}");
}

/// The twin of what `ritsu_is_every_language` and `ritsu_called_by_a_language_s_name` run on the
/// project with Japanese names, on the project with English names.
#[test]
fn ritsu_is_every_language_on_the_english_project() {
    let rule = "ritsu/tests/projects/shop/delivery/rules/urgency.rule";
    assert_eq!(ritsu(&["rulec", "check", rule]), (0, format!("ok {rule}\n"), String::new()));
    let cal = "ritsu/tests/projects/shop/delivery/ship_date.cal";
    let (code, out, _) = ritsu(&["koyomi", "check", cal, "--lang", "ja"]);
    assert!(code == 0 && out.starts_with(&format!("{cal}: ok — ")), "{out}");
    let t = TempDir::new("links-en");
    let link = t.path().join("rulec");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ritsu"), &link).unwrap();
    let (code, out, _) = run_in(&link, &crates(), &["check", "ritsu/tests/projects/shop/billing/rules/payment_fee.rule"]);
    assert!(code == 0 && out.starts_with("ok "), "{out}");
}
