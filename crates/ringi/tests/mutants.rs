//! The mutants (PLAN B.7): every `tests/mutants/<CODE>_<what>.ringi` gives its code, checked as
//! `ritsu ringi check` checks it (an E008 mutant as the binary of ringi's own crate does, an E704
//! mutant as `ritsu gen --target requests` checks the screens), and what `ringi check` prints for it,
//! in English and in Japanese, is its golden file in `tests/golden/`.
//! An English mutant and a Japanese one are a pair by the code at their start; the files they read
//! are in `tests/mutants/shared/`. `RINGI_BLESS=1 cargo test -p ringi --test mutants` writes the
//! golden files again; read the diff.

mod common;

use ringi::check::Run;
use ringi::suite::Suite;
use ritsu_base::text::Lang;

fn mutants() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".ringi")).collect();
    v.sort();
    v
}

/// The code a mutant is for: the word before its first `_`.
fn code_of(name: &str) -> &str {
    name.split('_').next().unwrap()
}

/// Whether a name is written in Japanese: what tells the pair's two halves apart.
fn japanese(name: &str) -> bool {
    !name.is_ascii()
}

/// What a mutant for `code` is given: `ringi check`'s answer, as `ritsu ringi check` checks it, an
/// E008 mutant's as the binary of ringi's own crate does, and an E704 mutant's as `ritsu gen
/// --target requests` checks the screens before it makes them (DESIGN 4.8, 13.3).
fn outcome(path: &str, code: &str) -> std::rc::Rc<ringi::check::Outcome> {
    if code == "E704" {
        return common::check_screens(path);
    }
    let suite = if code == "E008" { Suite::default() } else { common::joined() };
    Run::new(suite).file(path).unwrap()
}

#[test]
fn every_mutant_gives_its_code_and_says_what_its_golden_files_say() {
    let mut failures = Vec::new();
    let mut n = 0;
    for name in mutants() {
        let path = format!("tests/mutants/{name}");
        let code = code_of(&name);
        let o = outcome(&path, code);
        if !o.diags.iter().any(|d| d.code == code) {
            failures.push(format!("{path} does not give {code}: {:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>()));
        }
        let stem = name.trim_end_matches(".ringi");
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let text = ringi::check::render(&o, lang);
            let golden = format!("tests/golden/{stem}.{tag}.txt");
            if let Err(e) = ritsu_testkit::golden::check(std::path::Path::new(&golden), &text) {
                failures.push(format!("{path} ({tag}): {e}"));
            }
        }
        n += 1;
    }
    assert!(n >= 100, "{n} mutants");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A mutant gives its code and no other: what it shows is about that code alone.
#[test]
fn every_mutant_gives_its_code_alone() {
    let mut noisy = Vec::new();
    for name in mutants() {
        let path = format!("tests/mutants/{name}");
        let code = code_of(&name);
        let o = outcome(&path, code);
        let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
        if codes.iter().any(|c| *c != code) {
            noisy.push(format!("{path}: {codes:?}"));
        }
    }
    assert!(noisy.is_empty(), "{}", noisy.join("\n"));
}

/// Every code the ledger prints by now has a mutant in English names and one in Japanese names.
#[test]
fn every_code_has_a_mutant_in_each_language() {
    let names = mutants();
    let mut missing = Vec::new();
    for e in ringi::codes::ledger().entries.iter().filter(|e| !matches!(e.repro, ritsu_base::ledger::Repro::Later)) {
        for ja in [false, true] {
            if !names.iter().any(|n| code_of(n) == e.code && japanese(n) == ja) {
                missing.push(format!("{} ({})", e.code, if ja { "ja" } else { "en" }));
            }
        }
    }
    assert!(missing.is_empty(), "no mutant for {}", missing.join(", "));
}

/// No golden file is left without its mutant.
#[test]
fn every_golden_file_has_its_mutant() {
    let ms: Vec<String> = mutants().iter().map(|n| n.trim_end_matches(".ringi").to_string()).collect();
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

/// A finding of the organization's data gives, in its `witness`, the versions, the stages and every
/// requester by ID with the person it came to, where the text names three of them.
#[test]
fn the_witness_of_a_finding_of_the_organization() {
    let o = common::check("tests/mutants/W506_one_person_checks_twice.ringi");
    let v = ringi::check::to_json(&o, std::path::Path::new("."), Lang::En);
    let d = &v.get("diagnostics").and_then(|x| x.as_arr()).unwrap()[0];
    let keys: Vec<&str> = d.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["code", "severity", "file", "line", "col", "message", "notes", "witness", "fix"]);
    let w = d.get("witness").unwrap();
    assert_eq!(w.get("versions").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|x| x.as_str()).collect::<Vec<_>>(), ["2026-04-01"]);
    assert_eq!(w.get("stages").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|x| x.as_str()).collect::<Vec<_>>(), ["check", "decide"]);
    let rs: Vec<(&str, &str)> = w.get("requesters").and_then(|x| x.as_arr()).unwrap().iter().map(|r| (r.get("id").and_then(|x| x.as_str()).unwrap(), r.get("to").and_then(|x| x.as_str()).unwrap())).collect();
    assert_eq!(rs, [("e01", "e04"), ("e02", "e04"), ("e03", "e04"), ("e04", "e04")]);
}

/// The JSON of a file keeps its keys in English, in the order of ritsu-base's, whatever the
/// language; its paths are written from the root.
#[test]
fn the_json_of_a_diagnostic() {
    for path in ["tests/mutants/E004_a_cell_not_in_who.ringi", "tests/mutants/E004_whoに無い承認者.ringi"] {
        let o = common::check(path);
        for lang in [Lang::En, Lang::Ja] {
            let v = ringi::check::to_json(&o, std::path::Path::new("."), lang);
            let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(keys, vec!["ringi", "file", "ok", "summary", "facts", "usable", "diagnostics"]);
            assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
            assert!(v.get("usable").is_some_and(|x| x.is_null()), "a file that fails says no range of days");
            assert_eq!(v.get("file").and_then(|x| x.as_str()), Some(path));
            let d = &v.get("diagnostics").and_then(|x| x.as_arr()).unwrap()[0];
            let keys: Vec<&str> = d.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(keys, vec!["code", "severity", "file", "line", "col", "message", "notes", "fix"]);
            assert_eq!(d.get("code").and_then(|x| x.as_str()), Some("E004"));
            assert!(d.get("fix").is_some_and(|x| x.is_null()), "E004 has no fixed line");
        }
    }
}
