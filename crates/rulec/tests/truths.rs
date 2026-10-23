//! The truth values of an expression, and the names with `True` and `False` inside them (§15.264).
//!
//! The text `Gen::expr` writes for an expression carried Python's `True` and `False`, and the other
//! languages put their own words in by turning every `True` of the text into `true`, inside an
//! identifier as well: `define isTrueFee` was declared `isTrueFee` and read `istrueFee`, and the
//! code did not build in TypeScript, JavaScript, Rust, Wasm, Ruby, PHP, Go, Swift or Java, or
//! answered otherwise. The truth values are marks now, which no identifier can hold, and each
//! language writes its own words for them.
//!
//! The materials are in `tests/truths`, English first, with a Japanese version beside each: a rule
//! with names that have `True` and `False` inside them, beside two applies that pass the truth
//! values as they are, and the rule they apply.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, side_by_side, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The rules, the English first and the Japanese version beside it, with the rule each applies.
const RULES: &[(&str, &str)] = &[("names_with_true_and_false", "真偽を含む名前"), ("special_fee", "特別の手数料")];

/// The identifiers of the names with `True` and `False` inside them.
const NAMES: &[&str] = &["isTrueFee", "hasFalseStart", "feeIfTrue"];

fn rulec(cwd: &Path, tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every rule here is listed above, with its Japanese version.
#[test]
fn every_rule_here_is_listed() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/truths"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    names.sort();
    let mut listed: Vec<String> = RULES.iter().flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/truths is listed in RULES, with its Japanese version");
}

/// Every file `rulec gen` writes, read as text, by its path under the output.
fn generated(dir: &Path) -> Vec<(String, String)> {
    fn walk(d: &Path, base: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else if let Ok(text) = std::fs::read_to_string(&p) {
                out.push((p.strip_prefix(base).unwrap().to_string_lossy().into_owned(), text));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The rules generated into one directory of their own.
fn generated_into(t: &TempDir, which: &[&str]) -> PathBuf {
    let tmp = tmpdir_in(t.path());
    let out = t.path().join("out");
    let mut args: Vec<String> = vec!["gen".into()];
    args.extend(which.iter().map(|s| format!("tests/truths/{s}.rule")));
    args.extend(["--out".into(), out.to_string_lossy().into_owned()]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, said, e) = rulec(&root(), &tmp, &refs);
    assert_eq!(code, 0, "{said}{e}");
    out
}

/// What every language writes for the names keeps their `True` and their `False`, and no file
/// holds a mark of a truth value: each language has written its own words for them.
#[test]
fn every_language_writes_the_names_as_they_are() {
    let t = TempDir::new("truths-gen");
    let out = generated_into(&t, &RULES.iter().flat_map(|(en, ja)| [*en, *ja]).collect::<Vec<_>>());
    let files = generated(&out);
    let mut failures = Vec::new();
    for (path, text) in &files {
        if text.contains('\u{1d}') {
            failures.push(format!("{path}: a mark of a truth value is left in the file"));
        }
        for n in NAMES {
            let lowered = n.replace("True", "true").replace("False", "false");
            if text.contains(&lowered) {
                failures.push(format!("{path}: {n} is written as {lowered}"));
            }
        }
    }
    // Each language that writes the rules wrote the three names somewhere.
    for lang in ["python", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java", "sql", "wasm"] {
        let written: Vec<&(String, String)> = files.iter().filter(|(p, _)| p.starts_with(&format!("{lang}/"))).collect();
        assert!(!written.is_empty(), "{lang}: nothing was written");
        for n in NAMES {
            if !written.iter().any(|(_, text)| text.contains(n)) {
                failures.push(format!("{lang}: {n} is written nowhere"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The generated code of the rules answers as the reference evaluator does in every language, for
/// the English rules and the Japanese versions alike.
#[test]
fn the_generated_code_agrees_in_every_language() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let dirs: Vec<Vec<&str>> = vec![vec![RULES[0].0, RULES[1].0], vec![RULES[0].1, RULES[1].1]];
    side_by_side(&dirs, |which| {
        let t = TempDir::new("truths");
        let tmp = tmpdir_in(t.path());
        let out = generated_into(&t, which);
        let o = out.to_string_lossy().into_owned();
        let (_, said, e) = rulec(&root(), &tmp, &["test", o.as_str(), "--format", "json"]);
        let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
        for why in j.get("skipped").and_then(Json::as_arr).unwrap_or_default() {
            skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
        }
        let field = |r: &Json, k: &str| r.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
        let results = j.get("results").and_then(Json::as_arr).unwrap_or_default();
        let broken: Vec<String> = results
            .iter()
            .filter(|r| r.get("ok").and_then(|v| v.as_bool()) != Some(true))
            .map(|r| format!("{} {}/{}: {}", field(r, "rule"), field(r, "lang"), field(r, "via"), field(r, "error")))
            .collect();
        assert!(broken.is_empty(), "{which:?}: these did not run or did not agree:\n{}", broken.join("\n"));
        let ran: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
        assert!(ran.iter().any(|l| l == "python"), "{which:?}: Python did not run: {ran:?}");
    });
}
