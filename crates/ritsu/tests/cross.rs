//! The checks across the languages, as `ritsu check` runs them on a project (DESIGN 7; PLAN E.4).
//! Each check has a project where it holds, and variants where there is an example that it does
//! not and where it cannot be decided; the text (English and Japanese) and the JSON's `borders`
//! are held to golden files in `tests/golden/cross/` (`RITSU_BLESS=1` writes them). The projects
//! are small and English: the ledger's reproductions (`ritsu_cross::codes`) are laid out from the
//! same files.
//!
//! - X2 (a rule's preconditions where a workflow calls it): `refund` — the amount asked is kept
//!   below what was paid by the ranges (held), can pass it (E201), or comes from a task with no
//!   range (W201).
//! - X3 (b) (`range from koyomi`, rulec's own check): `settlement` — a rule over the days a koyomi
//!   date comes to, checked by `ritsu check` with koyomi joined, passing and with a payment day no
//!   row takes (rulec's E101 with that day).

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn golden(name: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join("tests/golden/cross").join(name), got) {
        failures.push(e);
    }
}

/// The files of a reproduction in ritsu's ledger.
fn repro(code: &str) -> Vec<(&'static str, &'static str)> {
    let l = ritsu_cross::codes::ledger();
    match &l.find(code).expect("the code is in the ledger").repro {
        ritsu_base::ledger::Repro::Dir { files, .. } => files.clone(),
        other => panic!("{code} has no files: {other:?}"),
    }
}

fn lay_out(files: &[(&str, String)], tag: &str) -> TempDir {
    let t = TempDir::new(tag);
    for (name, body) in files {
        std::fs::write(t.path().join(name), body).unwrap();
    }
    t
}

/// The text in both languages, the exit, and the JSON's `borders`, each held to its golden file.
fn run(name: &str, files: &[(&str, String)], want_code: i32, borders: (u64, u64, u64), failures: &mut Vec<String>) {
    let t = lay_out(files, name);
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--lang", lang]);
        assert_eq!(code, want_code, "{name} ({lang}):\n{out}{err}");
        golden(&format!("{name}.{lang}.txt"), &out, failures);
    }
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let b = &v["borders"];
    assert_eq!((b["held"].as_u64(), b["failed"].as_u64(), b["undecided"].as_u64()), (Some(borders.0), Some(borders.1), Some(borders.2)), "{name}: {b}");
}

#[test]
fn x2_a_rules_preconditions_where_a_flow_calls_it() {
    let mut failures = Vec::new();
    let files = repro("E201");
    let rule = files.iter().find(|(n, _)| n.ends_with(".rule")).unwrap().1.to_string();
    let flow = files.iter().find(|(n, _)| n.ends_with(".flow")).unwrap().1.to_string();
    // held: what was paid is at least 5,000, and what is asked at most 5,000
    let held = flow.replace("paid  : int  range >=0 <=10000", "paid  : int  range >=5000 <=10000").replace("-> int range >=0 <=10000", "-> int range >=0 <=5000");
    run("x2-held", &[("refund_check.rule", rule.clone()), ("refund.flow", held)], 0, (1, 0, 0), &mut failures);
    // an example: what is asked can pass what was paid
    run("x2-E201", &[("refund_check.rule", rule.clone()), ("refund.flow", flow.clone())], 1, (0, 1, 0), &mut failures);
    // undecided: what is asked comes from a task whose answer has no range
    let open = repro("W201").iter().find(|(n, _)| n.ends_with(".flow")).unwrap().1.to_string();
    run("x2-W201", &[("refund_check.rule", rule.clone()), ("refund.flow", open)], 0, (0, 0, 1), &mut failures);
    // the same value on both sides of `<=` holds whatever it is
    let same = flow.replace("check(paid: paid, asked: asked)", "check(paid: asked, asked: asked)");
    run("x2-same-value", &[("refund_check.rule", rule), ("refund.flow", same)], 0, (1, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x3b_a_rule_over_koyomis_days() {
    let mut failures = Vec::new();
    let days = here().join("../rulec/tests/days");
    let cal = std::fs::read_to_string(days.join("payment_terms.cal")).unwrap();
    let rule = std::fs::read_to_string(days.join("settlement.rule")).unwrap();
    run("x3b-held", &[("payment_terms.cal", cal.clone()), ("settlement.rule", rule.clone())], 0, (1, 0, 0), &mut failures);
    let gap = rule.replace(">=2026-07-10 <=2026-12-10", ">=2026-07-11 <=2026-12-10").replace("| 2026-07-10 | second_half |\n", "");
    run("x3b-E101", &[("payment_terms.cal", cal), ("settlement.rule", gap)], 1, (0, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
