//! The command line (PLAN B.10, DESIGN 5): the table, `--help`, exit codes, and what is
//! refused with exit 2 rather than ignored.

mod common;

use common::TempDir;
use std::process::{Command, Output};

fn koyomi(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).env_remove("KOYOMI_LANG").output().unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

#[test]
fn no_arguments_and_help() {
    let o = koyomi(&[]);
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("koyomi check"), "{}", err(&o));
    let o = koyomi(&["--help"]);
    assert_eq!(code(&o), 0);
    for c in ["check", "eval", "gen", "vectors", "doc", "api", "source", "explain"] {
        assert!(out(&o).contains(&format!("koyomi {c} ")), "{c} is in --help");
    }
    assert_eq!(code(&koyomi(&["doc"])), 2);
    let o = koyomi(&["--version"]);
    assert_eq!(out(&o).trim(), format!("koyomi {}", env!("CARGO_PKG_VERSION")));
    assert_eq!(code(&koyomi(&["-V"])), 0);
}

#[test]
fn every_command_has_its_help() {
    for c in ["check", "eval", "gen", "vectors", "doc", "api", "source", "explain"] {
        let a = koyomi(&[c, "--help"]);
        let b = koyomi(&["help", c]);
        let h = koyomi(&[c, "-h"]);
        assert_eq!(code(&a), 0);
        assert_eq!(out(&a), out(&b));
        assert_eq!(out(&a), out(&h));
        assert!(out(&a).contains("Usage:") && out(&a).contains("Exit codes:") && out(&a).contains("Examples:"), "{c}");
        let ja = koyomi(&[c, "--help", "--lang", "ja"]);
        assert!(out(&ja).contains("使い方:"), "{c}");
    }
    assert!(out(&koyomi(&["check", "--help"])).contains("E305"));
}

#[test]
fn what_is_refused_with_exit_2() {
    let f = "examples/支払_20日締め翌月10日払い.cal";
    for args in [
        vec!["check", f, "--frmat", "json"],
        vec!["check", f, "--format", "yaml"],
        vec!["check", f, "--budget"],
        vec!["check", f, "--format", "json", "--format", "json"],
        vec!["check", f, "--budget", "many"],
        vec!["check", f, "-o", "x"],
        vec!["check"],
        vec!["nonsense"],
        vec!["explain", "E999"],
        vec!["explain"],
        vec!["eval", f],
        vec!["eval", f, "受領日=2026-04-31"],
        vec!["eval", f, "受取日=2026-04-01"],
        vec!["eval", "examples/calendars/東京の営業日.cal"],
        vec!["api", f, "examples/net30.cal"],
        vec!["check", "examples/none.cal"],
        vec!["check", f, "--lang", "fr"],
        vec!["explain", "--all=yes"],
        vec!["gen"],
        vec!["gen", f, "--target", "java"],
        vec!["gen", f, "--out"],
        vec!["gen", f, "--check=yes"],
        vec!["gen", "examples/none.cal"],
        vec!["vectors"],
        vec!["vectors", f, "--format", "json"],
        vec!["source", "fetch"],
        vec!["source", "update", f],
        vec!["source", "pin", "examples/none.cal"],
    ] {
        let o = koyomi(&args);
        assert_eq!(code(&o), 2, "{args:?}: {}{}", out(&o), err(&o));
        assert!(!err(&o).is_empty(), "{args:?} says why");
    }
}

#[test]
fn check_exit_codes_and_formats() {
    let o = koyomi(&["check", "examples/支払_20日締め翌月10日払い.cal"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).contains(": ok — 3 claims hold on all 689 days"), "{}", out(&o));
    let o = koyomi(&["check", "examples/支払_月末締め翌々月末払い.cal"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E301]: examples/支払_月末締め翌々月末払い.cal:16:3:"), "{}", out(&o));
    // One JSON object a file.
    let o = koyomi(&["check", "examples/calendars", "--format", "json"]);
    assert_eq!(code(&o), 0);
    let lines: Vec<serde_json::Value> = out(&o).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|v| v["ok"] == true && v["diagnostics"].as_array().unwrap().is_empty()));
    // A directory: every .cal under it; the two that fail on purpose make it 1.
    let o = koyomi(&["check", "examples"]);
    assert_eq!(code(&o), 1);
    assert_eq!(out(&o).matches(": ok — ").count(), 8, "{}", out(&o));
    // The budget.
    let o = koyomi(&["check", "examples/締め日と支払日を受け取る.cal", "--budget", "1000"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).contains("E305"));
}

#[test]
fn the_language() {
    let f = "examples/支払_月末締め翌々月末払い.cal";
    let ja = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(["check", f]).env("KOYOMI_LANG", "ja").output().unwrap();
    assert!(out(&ja).starts_with("エラー[E301]"), "{}", out(&ja));
    let en = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(["check", f, "--lang", "en"]).env("KOYOMI_LANG", "ja").output().unwrap();
    assert!(out(&en).starts_with("error[E301]"), "--lang wins over KOYOMI_LANG");
    let o = koyomi(&["check", f, "--lang=ja"]);
    assert!(out(&o).starts_with("エラー[E301]"));
    // The keys of the JSON stay in English.
    let o = koyomi(&["check", f, "--format", "json", "--lang", "ja"]);
    let v: serde_json::Value = serde_json::from_str(out(&o).lines().next().unwrap()).unwrap();
    assert_eq!(v["diagnostics"][0]["code"], "E301");
    assert!(v["diagnostics"][0]["message"].as_str().unwrap().starts_with("条件"));
}

#[test]
fn eval() {
    let f = "examples/支払_20日締め翌月10日払い.cal";
    let o = koyomi(&["eval", f, "受領日=2026-04-01", "--lang", "ja"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).contains("2026-05-08（金）  休みなら前営業日: 2026-05-10 は日曜、2026-05-09 は土曜で休み"), "{}", out(&o));
    // The alias works as the name does.
    let o = koyomi(&["eval", f, "received=2026-04-01", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["dates"]["支払日"], "2026-05-08");
    assert_eq!(v["times"]["支払日"]["utc"], "2026-05-08T00:00:00Z");
    // Outside the range: the computation is refused, as the generated code would refuse it.
    let o = koyomi(&["eval", f, "受領日=2025-12-31"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).contains("outside its range"));
    // Past the table.
    let o = koyomi(&["eval", "examples/calendars/東京の営業日.cal", "2028-01-10"]);
    assert_eq!(code(&o), 1);
    let o = koyomi(&["eval", "examples/calendars/東京の営業日.cal", "2026-05-06", "--lang", "ja"]);
    assert_eq!(code(&o), 0);
    assert_eq!(out(&o), "2026-05-06（水）  休み: 休日\n");
    // A file with errors says them and computes nothing.
    let o = koyomi(&["eval", "tests/mutants/E201_無い日の扱いが無い.cal", "受領日=2026-01-29"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).contains("E201"));
    // Integer inputs.
    let o = koyomi(&["eval", "examples/民法の期間.cal", "起点=2026-01-22", "月数=1"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).contains("2026-02-23"), "{}", out(&o));
}

#[test]
fn explain() {
    let o = koyomi(&["explain", "E201"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).starts_with("E201 (error) — "), "{}", out(&o));
    assert!(out(&o).contains("+ 1 month"));
    let o = koyomi(&["explain", "e201", "--lang", "ja"]);
    assert!(out(&o).contains("いつ出るか"));
    let o = koyomi(&["explain", "--all"]);
    for e in koyomi::codes::ledger() {
        assert!(out(&o).contains(&format!("{} (", e.code)), "{}", e.code);
    }
    let o = koyomi(&["explain", "--all", "--format", "markdown"]);
    assert!(out(&o).starts_with("# Diagnostic codes"));
    assert_eq!(out(&o).matches("\n## ").count(), 39);
    let o = koyomi(&["explain", "W202", "--format", "markdown"]);
    assert!(out(&o).starts_with("<a id=\"w202\"></a>"), "{}", out(&o));
}

#[test]
fn api() {
    let o = koyomi(&["api", "examples/支払_20日締め翌月10日払い.cal"]);
    assert_eq!(code(&o), 0);
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["kind"], "dates");
    let o = koyomi(&["api", "examples/支払_月末締め翌々月末払い.cal"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).contains("E301"));
}

/// Every file under `dir`, as (path from `dir`, contents).
fn tree(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn walk(base: &std::path::Path, d: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut es: Vec<_> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()).collect();
        es.sort();
        for e in es {
            if e.is_dir() {
                walk(base, &e, out);
            } else {
                out.push((e.strip_prefix(base).unwrap().to_string_lossy().to_string(), std::fs::read(&e).unwrap()));
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

#[test]
fn gen_writes_and_checks() {
    let t = TempDir::new("gen");
    let out_dir = t.path().to_string_lossy().to_string();
    let good = ["examples/支払_20日締め翌月10日払い.cal", "examples/calendars/東京の営業日.cal"];
    // A file that fails check is refused with its diagnostics; the others are written.
    let o = koyomi(&["gen", good[0], "examples/支払_月末締め翌々月末払い.cal", good[1], "--out", &out_dir]);
    assert_eq!(code(&o), 1, "{}{}", out(&o), err(&o));
    assert!(out(&o).contains("error[E301]"), "{}", out(&o));
    assert!(err(&o).contains("`examples/支払_月末締め翌々月末払い.cal` does not pass check, so nothing is generated from it"), "{}", err(&o));
    let written = tree(t.path());
    let names: Vec<&str> = written.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        names,
        [
            "go/paymentterms/paymentterms.go",
            "go/paymentterms/paymentterms_runner_test.go",
            "go/tokyo/tokyo.go",
            "go/tokyo/tokyo_runner_test.go",
            "python/payment_terms.py",
            "python/payment_terms_runner.py",
            "python/tokyo.py",
            "python/tokyo_runner.py",
            "rust/payment_terms.rs",
            "rust/payment_terms_runner.rs",
            "rust/tokyo.rs",
            "rust/tokyo_runner.rs",
            "sql/payment_terms.sql",
            "sql/payment_terms_runner.sql",
            "sql/tokyo.sql",
            "sql/tokyo_runner.sql",
            "typescript/payment_terms.ts",
            "typescript/payment_terms_runner.ts",
            "typescript/tokyo.ts",
            "typescript/tokyo_runner.ts",
        ]
    );
    // What the command writes is what the library generates.
    let mut o = koyomi::check::check(good[0]).unwrap();
    let u = koyomi::codegen::unit_of(&o.checked.take().unwrap(), koyomi::i18n::Lang::En);
    for t2 in koyomi::naming::TARGETS {
        for (rel, body) in koyomi::codegen::files(&u, t2) {
            assert_eq!(std::fs::read_to_string(t.path().join(&rel)).unwrap(), body, "{rel}");
        }
    }
    // Generated again: nothing changes, nothing is said.
    let o = koyomi(&["gen", good[0], good[1], "--out", &out_dir]);
    assert_eq!(code(&o), 0);
    assert_eq!(out(&o), "");
    // --check: the same files pass; an edited file or a missing one is 1, and nothing is written.
    assert_eq!(code(&koyomi(&["gen", good[0], good[1], "--out", &out_dir, "--check"])), 0);
    let edited = t.path().join("python/payment_terms.py");
    let before = std::fs::read_to_string(&edited).unwrap();
    std::fs::write(&edited, before.replace("roll preceding", "roll following")).unwrap();
    std::fs::remove_file(t.path().join("sql/tokyo.sql")).unwrap();
    let o = koyomi(&["gen", good[0], good[1], "--out", &out_dir, "--check"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).contains(&format!("differs from what gen writes: {}", edited.display())), "{}", out(&o));
    assert!(out(&o).contains("missing: "), "{}", out(&o));
    assert_ne!(std::fs::read_to_string(&edited).unwrap(), before, "--check writes nothing");
    let o = koyomi(&["gen", good[0], good[1], "--out", &out_dir, "--check", "--lang", "ja"]);
    assert!(out(&o).contains("生成し直すと変わります: "), "{}", out(&o));
    // One target only, and the comments in Japanese.
    let t3 = TempDir::new("gen-py");
    let o = koyomi(&["gen", "examples/net30.cal", "--target", "python", "--lang", "ja", "--out", &t3.path().to_string_lossy()]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let names: Vec<String> = tree(t3.path()).into_iter().map(|(p, _)| p).collect();
    assert_eq!(names, ["python/net30.py", "python/net30_runner.py"]);
    let py = std::fs::read_to_string(t3.path().join("python/net30.py")).unwrap();
    assert!(py.contains("# もと: net30.cal（dates net30 v1、sha256:"), "{py}");
    assert!(py.contains("受け取る範囲は invoice_date 2026-01-01〜2028-11-29 で、"), "{py}");
}

#[test]
fn gen_refuses_two_files_with_one_alias() {
    let t = TempDir::new("gen-alias");
    let a = t.path().join("a.cal");
    let b = t.path().join("b.cal");
    let body = "dates 同じ(same) v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate later = d\n  + 1 day\n";
    std::fs::write(&a, body).unwrap();
    std::fs::write(&b, body).unwrap();
    let o = koyomi(&["gen", &a.to_string_lossy(), &b.to_string_lossy(), "--out", &t.path().join("out").to_string_lossy()]);
    assert_eq!(code(&o), 2, "{}{}", out(&o), err(&o));
    assert!(err(&o).contains("both write typescript/same.ts"), "{}", err(&o));
    assert!(!t.path().join("out").exists(), "nothing is written");
}
