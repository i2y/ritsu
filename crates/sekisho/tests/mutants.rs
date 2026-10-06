//! The mutants (PLAN A1): every `tests/mutants/<CODE>_<what>.gate` gives its code, checked as
//! `ritsu sekisho check` checks it (an E209 mutant as the binary of sekisho's own crate does), with
//! the crate's directory as the root its references are written from (`common::root_of`), and
//! what `sekisho check` prints for it, in English and in Japanese, is its golden file in
//! `tests/golden/`. An English mutant and a Japanese one are a pair by the code at their start.
//! `SEKISHO_BLESS=1 cargo test -p sekisho --test mutants` writes the golden files again; read the diff.

mod common;

use ritsu_base::text::Lang;
use sekisho::check::check_file;
use sekisho::suite::Suite;

fn mutants() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".gate")).collect();
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
        let suite = if code == "E209" { Suite::default() } else { common::joined() };
        let o = check_file(&path, &suite, &common::options(&path)).unwrap();
        if !o.diags.iter().any(|d| d.code == code) {
            failures.push(format!("{path} does not give {code}: {:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>()));
        }
        let stem = name.trim_end_matches(".gate");
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let text = sekisho::check::render(&o, lang);
            let golden = format!("tests/golden/{stem}.{tag}.txt");
            if let Err(e) = ritsu_testkit::golden::check(std::path::Path::new(&golden), &text) {
                failures.push(format!("{path} ({tag}): {e}"));
            }
        }
        n += 1;
    }
    assert!(n >= 34, "{n} mutants");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// No golden file is left without its mutant.
#[test]
fn every_golden_file_has_its_mutant() {
    let ms: Vec<String> = mutants().iter().map(|n| n.trim_end_matches(".gate").to_string()).collect();
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

/// The JSON of a diagnostic keeps its keys in English, in the order of ritsu-base's, with
/// sekisho's `example` between `notes` and `fix`.
#[test]
fn the_json_of_a_diagnostic() {
    for path in ["tests/mutants/E101_unknown_role.gate", "tests/mutants/E101_知らない役割.gate"] {
        let o = check_file(path, &common::joined(), &common::options(path)).unwrap();
        for lang in [Lang::En, Lang::Ja] {
            let v = sekisho::check::to_json(&o, lang);
            assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
            let d = &v.get("diagnostics").and_then(|x| x.as_arr()).unwrap()[0];
            let keys: Vec<&str> = d.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(keys, vec!["code", "severity", "file", "line", "col", "message", "notes", "example", "fix"]);
        }
    }
}
