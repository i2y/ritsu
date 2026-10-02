//! Specs geas refuses before running anything (E001-E013). Each file under
//! `tests/specs/` is run by `geas check` in a scratch directory: its stderr in
//! English and in Japanese, and its `--json`, are held to goldens, and it exits 2.
//! A file named `E0nn-<what>.geas` has to give E0nn.

mod common;
use common::*;
use std::fs;

fn specs() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root().join("tests/specs"))
        .expect("tests/specs")
        .map(|e| e.expect("an entry").file_name().into_string().expect("a UTF-8 name"))
        .filter(|n| n.ends_with(".geas"))
        .collect();
    names.sort();
    names
}

#[test]
fn every_refused_spec_gives_its_diagnostics() {
    let names = specs();
    let mut failures = Vec::new();
    for name in &names {
        let stem = name.trim_end_matches(".geas");
        let s = Scratch::new("spec");
        fs::copy(root().join("tests/specs").join(name), s.path().join(name)).expect("copy the spec");
        for (lang, args) in [("en", vec!["check", name.as_str()]), ("ja", vec!["check", name.as_str(), "--lang", "ja"])] {
            let (out, err, code) = run(s.path(), &args, &[]);
            if code != 2 || !out.is_empty() {
                failures.push(format!("{name} ({lang}): exit {code}, stdout {out:?}, stderr {err:?}"));
                continue;
            }
            if let Some(code) = stem.get(..4).filter(|c| c.starts_with('E')) {
                let mark = format!("[{code}]");
                if !err.contains(&mark) {
                    failures.push(format!("{name} ({lang}): no {mark} in\n{err}"));
                }
            }
            if let Err(e) = std::panic::catch_unwind(|| golden(&format!("{lang}/spec/{stem}.txt"), &err)) {
                failures.push(e.downcast_ref::<String>().cloned().unwrap_or_default());
            }
        }
        let (out, err, code) = run(s.path(), &["check", name, "--json"], &[]);
        if code != 2 || !err.is_empty() {
            failures.push(format!("{name} (json): exit {code}, stderr {err:?}"));
            continue;
        }
        let v = json(&out);
        assert_eq!(v.get("ok"), &Json::Bool(false), "{name}");
        assert!(!v.get("diagnostics").arr().is_empty(), "{name}");
        if let Err(e) = std::panic::catch_unwind(|| golden(&format!("en/spec/{stem}.json"), &out)) {
            failures.push(e.downcast_ref::<String>().cloned().unwrap_or_default());
        }
        // nothing ran, so nothing was written
        if s.exists(".geas") {
            failures.push(format!("{name}: a refused spec left .geas/ behind"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every static code has at least one spec that gives it.
#[test]
fn every_static_code_has_a_spec() {
    let names = specs();
    for code in ["E001", "E002", "E003", "E004", "E005", "E006", "E007", "E008", "E009", "E010", "E011", "E012", "E013"] {
        assert!(names.iter().any(|f| f.starts_with(code)), "no spec under tests/specs gives {code}");
    }
}

/// One file with an error of every static kind: all of them, in line order.
#[test]
fn every_static_error_is_reported_in_line_order() {
    let s = Scratch::new("spec-all");
    let name = "every-static-error-at-once.geas";
    fs::copy(root().join("tests/specs").join(name), s.path().join(name)).expect("copy the spec");
    let (_, err, code) = run(s.path(), &["check", name], &[]);
    assert_eq!(code, 2);
    let heads: Vec<&str> = err.lines().filter(|l| l.starts_with("error[")).collect();
    let codes: Vec<&str> = heads.iter().map(|l| &l[6..10]).collect();
    assert_eq!(codes, ["E002", "E007", "E004", "E003", "E006", "E007", "E008", "E007"], "{err}");
    let places: Vec<(usize, usize)> = heads
        .iter()
        .map(|l| {
            let place: Vec<&str> = l.split(": ").nth(1).expect("a place").split(':').collect();
            (place[1].parse().expect("a line"), place[2].parse().expect("a column"))
        })
        .collect();
    let mut sorted = places.clone();
    sorted.sort();
    assert_eq!(places, sorted, "{err}");
}
