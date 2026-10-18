//! The examples (DESIGN 17): `examples/` holds four, each in English (`<name>.kn`) and in Japanese
//! (`<name>.ja.kn`), beside the rules, the dates files and the books they read. Every module passes
//! `check` with every language joined and says nothing, not even a warning, and the `examples` it
//! writes hold. Every rule it reads cites the documents it transcribes, from copies beside it that
//! the rule pins (DESIGN 17.6), and a dates file, which can pin only a law of e-Gov, names the copy
//! the rule pins for it. The two versions answer alike on their vectors (`tests/vectors.rs`, DESIGN
//! 16.6), and with the verdicts of the checks across the languages every function is total
//! (ritsu's `tests/kaname.rs`).

mod common;

use ritsu_base::text::Lang;
use std::path::{Path, PathBuf};

/// The examples, in the order DESIGN 17 gives them.
const NAMES: [&str; 4] = ["payout", "late_payment", "media_mail", "discount_split"];

/// The names of the files of `dir` whose extension is `ext`, in order.
fn files(dir: &Path, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|f| Path::new(f).extension().is_some_and(|x| x == ext))
        .collect();
    out.sort();
    out
}

#[test]
fn every_example_is_in_english_and_in_japanese() {
    let mut dirs: Vec<String> = std::fs::read_dir("examples").unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    dirs.sort();
    let mut want: Vec<String> = NAMES.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(dirs, want, "examples/ holds the four of DESIGN 17");
    for name in NAMES {
        assert_eq!(files(&Path::new("examples").join(name), "kn"), [format!("{name}.ja.kn"), format!("{name}.kn")]);
    }
    let listed: Vec<String> = NAMES.iter().flat_map(|n| [format!("examples/{n}/{n}.kn"), format!("examples/{n}/{n}.ja.kn")]).collect();
    assert_eq!(common::EXAMPLES.to_vec(), listed, "the tests read every example, English first");
}

#[test]
fn every_example_passes_and_its_examples_hold() {
    for f in common::EXAMPLES {
        let o = common::check(f);
        assert!(o.diags.is_empty(), "{f}:\n{}", common::shown(&o, Lang::En));
        let c = o.checked.as_ref().unwrap_or_else(|| panic!("{f} is not checked"));
        // `check` ran every line of `examples` in the reference interpreter, and none said E601
        assert!(!c.module.examples.is_empty(), "{f} writes no examples");
    }
}

/// A rule's `source` lines: the name a citation uses (the alias aside), the copy, and its pin.
fn sources(rule: &str) -> Vec<(String, String, String)> {
    rule.lines()
        .filter_map(|l| l.strip_prefix("source "))
        .map(|l| {
            let name = l.split([' ', '(', '=']).next().unwrap().to_string();
            let copy = l.split("file \"").nth(1).and_then(|r| r.split('"').next()).unwrap_or_else(|| panic!("not a file: {l}")).to_string();
            let pin = l.split("sha256:").nth(1).map(|r| r.chars().take(16).collect()).unwrap_or_else(|| panic!("not pinned: {l}"));
            (name, copy, pin)
        })
        .collect()
}

#[test]
fn every_rule_cites_its_documents_from_pinned_copies() {
    let mut cited = 0;
    for name in NAMES {
        let rules: PathBuf = Path::new("examples").join(name).join("rules");
        for f in files(&rules, "rule") {
            let src = std::fs::read_to_string(rules.join(&f)).unwrap();
            let said = sources(&src);
            assert!(!said.is_empty(), "{name}/rules/{f} cites no document");
            for (source, copy, pin) in said {
                // the copy is the rule's own, under sources/, and its bytes are what the pin says
                assert!(copy.starts_with("sources/"), "{name}/rules/{f}: {copy}");
                let bytes = std::fs::read(rules.join(&copy)).unwrap_or_else(|e| panic!("{name}/rules/{copy}: {e}"));
                assert_eq!(&ritsu_base::sha256::hex(&bytes)[..16], pin, "{name}/rules/{copy}");
                // a copy says where it was copied from, and when
                let text = String::from_utf8(bytes).unwrap();
                assert!(text.contains("Copied from") && text.contains("2026"), "{name}/rules/{copy} does not say where it came from");
                // and the rule cites it
                assert!(src.contains(&format!("@{source}")), "{name}/rules/{f} pins {source} and cites nothing of it");
                cited += 1;
            }
        }
    }
    assert_eq!(cited, 10, "the sources the eight rules pin");
}

#[test]
fn a_dates_file_names_the_copy_the_rule_pins() {
    for name in NAMES {
        let dates = Path::new("examples").join(name).join("dates");
        if !dates.exists() {
            continue;
        }
        for f in files(&dates, "cal") {
            let src = std::fs::read_to_string(dates.join(&f)).unwrap();
            // koyomi pins only the laws of e-Gov in a dates file; the rule pins the copy it reads
            assert!(!src.lines().any(|l| l.starts_with("source ")), "{name}/dates/{f}");
            let named: Vec<&str> = src.split_whitespace().filter(|w| w.starts_with("rules/sources/")).collect();
            assert!(!named.is_empty(), "{name}/dates/{f} names no copy");
            for copy in named {
                let copy = copy.trim_end_matches([',', '.', '、', '。']);
                assert!(Path::new("examples").join(name).join(copy).exists(), "{name}/dates/{f}: {copy}");
            }
        }
    }
}
