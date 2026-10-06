//! `ritsu sekisho` and the `.gate` files of `ritsu check` (sekisho's DESIGN 11): sekisho's command,
//! with the languages a gate reads joined through the ports (the rules, the dates files and the
//! calendars, the books, the flows), which the binary of sekisho's own crate cannot (it says E209;
//! sekisho's `tests/cli.rs`); and `ritsu check` on a project that holds gates, which hands them to
//! sekisho after dandori.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn sekisho_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sekisho")
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu"))
        .current_dir(dir)
        .env_remove("RITSU_LANG")
        .env_remove("SEKISHO_LANG")
        .args(args)
        .output()
        .expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

const PASSES: &str = "ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation";

/// Both versions of sekisho's example read a rule, a dates file, a calendar and a flow: `ritsu
/// sekisho check` reads them in the same process.
#[test]
fn ritsu_sekisho_checks_the_example_with_every_language() {
    for f in ["examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"] {
        let (code, out, err) = ritsu_in(&sekisho_dir(), &["sekisho", "check", f]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, format!("{f}: {PASSES}\n"));
    }
    let (code, out, _) = ritsu_in(&sekisho_dir(), &["sekisho", "check", "examples/refunds/refunds.ja.gate", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert_eq!(out, "examples/refunds/refunds.ja.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1\n");
    // a mistake is sekisho's to say, as its own command says it
    let (code, out, _) = ritsu_in(&sekisho_dir(), &["sekisho", "check", "tests/mutants/E101_unknown_role.gate"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[E101]: tests/mutants/E101_unknown_role.gate:45:16: There is no role `clerck`\n"), "{out}");
    // the words after `sekisho` are sekisho's
    let (code, out, _) = ritsu_in(&sekisho_dir(), &["sekisho", "explain", "E101"]);
    assert!(code == 0 && out.starts_with("E101 (error) — A name names nothing\n"), "{out}");
    let (code, out, _) = ritsu_in(&sekisho_dir(), &["sekisho", "--version"]);
    assert_eq!((code, out), (0, format!("sekisho {}\n", env!("CARGO_PKG_VERSION"))));
    let (code, out, _) = ritsu_in(&sekisho_dir(), &["help", "sekisho"]);
    assert!(code == 0 && out.starts_with("sekisho "), "{out}");
}

/// `ritsu check` on sekisho's example: the rules, the calendars and the dates files, the flow, then
/// the two gates, each said as its language says it, and the line that sums them up.
#[test]
fn ritsu_check_hands_the_gates_to_sekisho() {
    let (code, out, err) = ritsu_in(&sekisho_dir(), &["check", "examples/refunds"]);
    assert_eq!(code, 0, "{out}{err}");
    let gates: Vec<&str> = out.lines().filter(|l| l.contains(".gate: ")).collect();
    assert_eq!(gates, vec![format!("examples/refunds/refunds.gate: {PASSES}"), format!("examples/refunds/refunds.ja.gate: {PASSES}")]);
    // after the flow, as the languages are checked: what gives facts first
    let flow = out.find("examples/refunds/flows/returns.flow: ok").expect("the flow is checked");
    assert!(flow < out.find("refunds.gate: ok").unwrap(), "{out}");
    assert!(out.ends_with("ritsu check: 8 files (rulec 2, koyomi 3, dandori 1, sekisho 2): all pass; borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let (code, out, _) = ritsu_in(&sekisho_dir(), &["check", "examples/refunds", "--format", "json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let gates: Vec<(&str, bool)> = v["files"].as_array().unwrap().iter().filter(|f| f["tool"] == "sekisho").map(|f| (f["file"].as_str().unwrap(), f["ok"].as_bool().unwrap())).collect();
    assert_eq!(gates, vec![("crates/sekisho/examples/refunds/refunds.gate", true), ("crates/sekisho/examples/refunds/refunds.ja.gate", true)]);
}

/// A gate with a mistake fails the project: its diagnostic has sekisho's word in its headline, and
/// its file and tool in the JSON.
#[test]
fn a_mistake_in_a_gate_fails_ritsu_check() {
    let t = TempDir::new("sekisho-example");
    ritsu_testkit::tmp::copy_dir(&sekisho_dir().join("examples/refunds"), t.path());
    let gate = t.path().join("refunds.gate");
    let src = std::fs::read_to_string(&gate).unwrap();
    std::fs::write(&gate, src.replacen("  principal in clerk\n  action refund_order\n", "  principal in clerck\n  action refund_order\n", 1)).unwrap();
    let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("error[sekisho E101]: refunds.gate:"), "{out}");
    assert!(out.contains("There is no role `clerck`"), "{out}");
    assert!(out.ends_with("ritsu check: 8 files (rulec 2, koyomi 3, dandori 1, sekisho 2): 1 fail (1 error); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let d = v["diagnostics"].as_array().unwrap().iter().find(|d| d["tool"] == "sekisho").expect("sekisho's diagnostic");
    assert_eq!((d["code"].as_str(), d["file"].as_str()), (Some("E101"), Some("refunds.gate")));
}

/// The commands of sekisho's stage B through `ritsu sekisho`, with every language joined: `gen
/// --target cedar` writes the four files of the example's Cedar, `vectors` prints its combinations
/// as the tests of `cedar run-tests`, and `api` its declarations as JSON.
#[test]
fn ritsu_sekisho_generates_the_cedar() {
    let t = TempDir::new("sekisho-gen");
    let out = t.path().to_string_lossy().to_string();
    let (code, said, err) = ritsu_in(&sekisho_dir(), &["sekisho", "gen", "examples/refunds/refunds.gate", "--target", "cedar", "--out", &out]);
    assert_eq!(code, 0, "{said}{err}");
    for f in ["refunds.cedar", "refunds.cedarschema", "refunds.cedarschema.json", "refunds.policies.json"] {
        assert!(said.contains(&format!("generated: {out}/cedar/{f}\n")), "{said}");
    }
    let (code, said, _) = ritsu_in(&sekisho_dir(), &["sekisho", "gen", "examples/refunds/refunds.gate", "--target", "cedar", "--out", &out, "--check"]);
    assert_eq!((code, said.as_str()), (0, ""));
    let (code, said, _) = ritsu_in(&sekisho_dir(), &["sekisho", "vectors", "examples/refunds/refunds.gate", "--action", "export_refunds"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&said).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 4);
    assert_eq!(v[0]["request"]["action"], "Shop::Action::\"export_refunds\"");
    let (code, said, _) = ritsu_in(&sekisho_dir(), &["sekisho", "api", "examples/refunds/refunds.ja.gate"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&said).unwrap();
    assert_eq!((v["namespace"].as_str(), v["policies"][2]["id"].as_str()), (Some("ShopJa"), Some("refunds_ja/clerks_refund_within_their_limit")));
}
