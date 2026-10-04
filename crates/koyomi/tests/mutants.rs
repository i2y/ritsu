//! The mutants (PLAN B.9): every `tests/mutants/<CODE>_<what>.cal` gives its code, and what
//! `koyomi check` prints for it, in English and in Japanese, is its golden file in
//! `tests/golden/`. `KOYOMI_BLESS=1 cargo test` writes the golden files again; read the diff.

use koyomi::check::{check, render};
use ritsu_base::text::Lang;

fn mutants() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("tests/mutants")
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".cal"))
        .collect();
    v.sort();
    v
}

#[test]
fn every_mutant_gives_its_code_and_says_what_its_golden_files_say() {
    let mut failures = Vec::new();
    let mut n = 0;
    for name in mutants() {
        let path = format!("tests/mutants/{name}");
        let code = name.split('_').next().unwrap();
        let o = check(&path).unwrap();
        if !o.diags.iter().any(|d| d.code == code) {
            failures.push(format!("{path} does not give {code}: {:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>()));
        }
        let stem = name.trim_end_matches(".cal");
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let text = render(&o, lang);
            let golden = format!("tests/golden/{stem}.{tag}.txt");
            if let Err(e) = ritsu_testkit::golden::check(std::path::Path::new(&golden), &text) {
                failures.push(format!("{path} ({tag}): {e}"));
            }
        }
        n += 1;
    }
    assert!(n >= 39, "{n} mutants");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The JSON of a diagnostic keeps its keys in English, and lists every failing input.
#[test]
fn the_json_of_a_diagnostic() {
    let o = check("tests/mutants/E301_60日を超える.cal").unwrap();
    for lang in [Lang::En, Lang::Ja] {
        let v = koyomi::check::to_json(&o, lang);
        let d = &v["diagnostics"][0];
        for key in ["code", "severity", "file", "line", "col", "message", "notes", "inputs", "steps", "fails", "fix"] {
            assert!(d.get(key).is_some(), "{key} is missing");
        }
        assert_eq!(d["fails"].as_array().unwrap().len(), 10);
        assert_eq!(d["inputs"]["受領日"], "2026-01-01");
        assert_eq!(v["ok"], false);
    }
}

/// No golden file is left without its mutant.
#[test]
fn every_golden_file_has_its_mutant() {
    let ms: Vec<String> = mutants().iter().map(|n| n.trim_end_matches(".cal").to_string()).collect();
    for e in std::fs::read_dir("tests/golden").unwrap() {
        let e = e.unwrap();
        if e.path().is_dir() {
            continue;
        }
        let n = e.file_name().to_string_lossy().to_string();
        let stem = n.trim_end_matches(".en.txt").trim_end_matches(".ja.txt");
        assert!(ms.iter().any(|m| m == stem), "tests/golden/{n} has no mutant");
    }
}

/// What `the_json_of_a_diagnostic` checks, on the English mutant: the keys, and every failing run.
#[test]
fn the_json_of_a_diagnostic_in_english() {
    let o = check("tests/mutants/E301_over_60_days.cal").unwrap();
    for lang in [Lang::En, Lang::Ja] {
        let v = koyomi::check::to_json(&o, lang);
        let d = &v["diagnostics"][0];
        for key in ["code", "severity", "file", "line", "col", "message", "notes", "inputs", "steps", "fails", "fix"] {
            assert!(d.get(key).is_some(), "{key} is missing");
        }
        assert_eq!(d["fails"].as_array().unwrap().len(), 15);
        assert_eq!(d["inputs"]["received"], "2026-01-01");
        assert_eq!(v["ok"], false);
    }
}

/// Every mutant with a Japanese name has one with an English name beside it, under the same code
/// (`E001_閉じていない文字列.cal` and `E001_unclosed_string.cal`), that gives the same codes and
/// the same exit code: the English one shows the same behavior in English.
#[test]
fn every_japanese_mutant_has_an_english_one() {
    let said = |name: &str| -> (Vec<&'static str>, i32) {
        let o = check(&format!("tests/mutants/{name}")).unwrap();
        let mut codes: Vec<&'static str> = o.diags.iter().map(|d| d.code).collect();
        codes.sort();
        codes.dedup();
        (codes, if o.has_errors() { 1 } else { 0 })
    };
    let all = mutants();
    let english = |n: &str| n.trim_end_matches(".cal").split_once('_').is_some_and(|(_, what)| what.is_ascii());
    let mut missing = Vec::new();
    let mut pairs = 0;
    for j in all.iter().filter(|n| !english(n)) {
        let code = j.split('_').next().unwrap();
        let want = said(j);
        let twin = all.iter().filter(|n| english(n) && n.starts_with(&format!("{code}_"))).find(|n| said(n) == want);
        match twin {
            Some(_) => pairs += 1,
            None => missing.push(format!("{j} ({:?}, exit {})", want.0, want.1)),
        }
    }
    assert!(missing.is_empty(), "Japanese mutants with no English mutant that says the same:\n{}", missing.join("\n"));
    assert!(pairs >= 45, "{pairs} pairs");
}
