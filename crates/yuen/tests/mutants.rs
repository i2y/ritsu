//! The mutants (PLAN B.12): every `tests/mutants/<CODE>_<what>/` gives its code, and what
//! `yuen check` prints for it, in English and in Japanese, is its golden file in
//! `tests/golden/`. Each mutant is its own root, so the golden files do not depend on where
//! the repository is. They are checked with every language joined, as `ritsu yuen` checks them
//! (some name a rule or a calendar). `YUEN_BLESS=1 cargo test` writes the golden files again;
//! read the diff.

mod common;

use yuen::check::render;
use ritsu_base::text::Lang;

fn check(paths: &[String], root: Option<&str>) -> Result<yuen::check::Checked, yuen::project::Refusal> {
    yuen::check::check_with(paths, root, common::suite())
}

fn mutants() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
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
        let c = match check(std::slice::from_ref(&path), Some(&path)) {
            Ok(c) => c,
            Err(r) => {
                failures.push(format!("{path} is refused: {}", r.0.en));
                continue;
            }
        };
        if !c.diags.iter().any(|d| d.code == code) {
            failures.push(format!("{path} does not give {code}: {:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>()));
        }
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let text = render(&c, &path, lang);
            common::golden(&format!("tests/golden/{name}.{tag}.txt"), &text, &mut failures);
        }
        n += 1;
    }
    assert!(n >= 41, "{n} mutants");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The JSON of a diagnostic keeps its keys in English, and carries its diff.
#[test]
fn the_json_of_a_diagnostic() {
    let path = "tests/mutants/E302_条が変わった".to_string();
    let c = check(std::slice::from_ref(&path), Some(&path)).unwrap();
    for lang in [Lang::En, Lang::Ja] {
        let v = yuen::check::to_json(&c, &path, lang);
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["root", "ok", "summary", "diagnostics"]);
        let d = &v["diagnostics"][0];
        let keys: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["code", "severity", "file", "line", "col", "message", "notes", "diff", "chain", "candidates", "fix"]);
        assert_eq!(d["code"], "E302");
        assert_eq!(d["file"], "民法の期間.req");
        assert!(d["diff"].as_array().unwrap().iter().any(|l| l["op"] == "+" && l["text"].as_str().unwrap().contains("翌々日")));
        assert_eq!(v["ok"], false);
    }
}

/// No golden file is left without its mutant.
#[test]
fn every_golden_file_has_its_mutant() {
    let ms = mutants();
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
