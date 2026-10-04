//! `ritsu sakai` (DESIGN 8.6): sakai's command, with every language it reads joined through the
//! ports — a rule's enums, its Connect service and its names, what a rule, a calendar and a
//! workflow refer to outside themselves, and a book's accounts and transfers. What only this binary
//! can run is here: the binary, on sakai's example and on a change to it.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn sakai_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sakai")
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu"))
        .current_dir(dir)
        .env_remove("RITSU_LANG")
        .env_remove("SAKAI_LANG")
        .args(args)
        .output()
        .expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// sakai's example holds rules, calendars, a book and workflows: `ritsu sakai check` reads what
/// each of them refers to across a boundary, which the binary of sakai's own crate cannot (it says
/// E104 for each language; sakai's `tests/examples.rs`).
#[test]
fn ritsu_sakai_checks_the_example_with_every_language() {
    let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "check", "examples/通販/通販.ctx"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "examples/通販/通販.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)\n");
    let (code, out, _) = ritsu_in(&sakai_dir(), &["sakai", "check", "examples/通販/通販.ctx", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.ends_with("境界を越える参照 9 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 5）\n"), "{out}");
    // the api names how each crossing refers, in the words of the file's language
    let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "api", "examples/通販/通販.ctx", "--root", "examples/通販"]);
    assert_eq!(code, 0, "{err}");
    let api: serde_json::Value = serde_json::from_str(&out).unwrap();
    let via: Vec<&str> = api["crossings"].as_array().unwrap().iter().map(|c| c["via"].as_str().unwrap()).collect();
    assert_eq!(via, ["proto import", "shape", "import proto", "use calendar", "use rule … connect", "use proto", "connect", "connect", "flow"]);
    // the settings the example keeps are what the map writes, read with every language joined
    for t in ["import-linter", "dependency-cruiser", "archunit", "go-arch-lint"] {
        let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "build", "examples/通販/通販.ctx", "--target", t, "--check", "--lang", "ja"]);
        assert_eq!(code, 0, "{t}: {out}{err}");
        assert!(out.contains("いまの地図から書くものと同じ"), "{t}: {out}");
    }
    // the words after `sakai` are sakai's
    let (code, out, _) = ritsu_in(&sakai_dir(), &["sakai", "--version"]);
    assert!(code == 0 && out.starts_with("sakai ") && out.lines().count() == 1, "{out}");
}

/// A change to the example that only the other languages show: ordering's workflow runs
/// delivery's as its child, and the two stop being partners (E209); billing's rule takes in an
/// order status that ordering has added a value to, and rulec does not answer for it (E105).
#[test]
fn ritsu_sakai_catches_what_crosses_through_another_language() {
    let t = TempDir::new("sakai-example");
    ritsu_testkit::tmp::copy_dir(&sakai_dir().join("examples/通販"), t.path());
    let edit = |file: &str, old: &str, new: &str| {
        let p = t.path().join(file);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{file} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    };
    edit("contexts/受注.ctx", "partnership with 配送\n", "upstream 配送 customer\n  through rulec.urgency.v1\n");
    edit("contexts/配送.ctx", "partnership with 受注\n", "downstream 受注 supplier\n");
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "check", "通販.ctx"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.starts_with("error[E209]: ordering/受注.flow:59:1: The workflow ordering/受注.flow of 受注 runs 配送's workflow delivery/配送の手配.flow as its child\n"), "{out}");
    edit("proto/shop/ordering/v1/order.proto", "  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n");
    let (code, out, _) = ritsu_in(t.path(), &["sakai", "check", "通販.ctx", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(out.contains("エラー[E105]: billing/rules/請求の要否.rule:5:1: billing/rules/請求の要否.rule が rulec の検査を通らないか、読めません\n"), "{out}");
    assert!(out.contains("  = rulec の診断: [E032] billing/rules/請求の要否.rule:5: "), "{out}");
}
