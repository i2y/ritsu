//! The examples (DESIGN 17): `examples/` holds ten, each in English (`<name>.kn`) and in Japanese
//! (`<name>.ja.kn`). Four join no other language and carry a host in TypeScript, Python and Go that
//! calls the code `ritsu gen` writes of them (`host/`, which ritsu's `tests/kaname.rs` runs); five call
//! the rules, the dates files and the books beside them; `refund_split` is the example of the
//! properties (DESIGN 38). Every module passes `check` with every language joined and says nothing
//! but a W603 for each property, which only `--lean` shows (`tests/lean.rs` runs it), and the
//! `examples` it writes hold. Every rule cites the documents it transcribes, from copies beside it
//! that the rule pins (DESIGN 17.11), and a dates file, which can pin only a law of e-Gov, names the
//! copy the rule pins for it. The two versions answer alike on their vectors (`tests/vectors.rs`,
//! DESIGN 16.6), and with the verdicts of the checks across the languages every function of the five
//! that call the other languages is total (ritsu's `tests/kaname.rs`).

mod common;

use ritsu_base::text::Lang;
use std::path::{Path, PathBuf};

/// The examples, in the order DESIGN 17 gives them: the four that join no other language, the five
/// that call rules, dates and books, and the example of the properties.
const NAMES: [&str; 10] = ["order_flow", "stock_diff", "handles", "catalog", "payout", "late_payment", "media_mail", "discount_split", "order_bill", "refund_split"];

/// The examples that join no other language, each with its host.
const HOSTED: [&str; 4] = ["order_flow", "stock_diff", "handles", "catalog"];

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
    assert_eq!(dirs, want, "examples/ holds the examples of DESIGN 17");
    for name in NAMES {
        assert_eq!(files(&Path::new("examples").join(name), "kn"), [format!("{name}.ja.kn"), format!("{name}.kn")]);
    }
    let listed: Vec<String> = NAMES.iter().flat_map(|n| [format!("examples/{n}/{n}.kn"), format!("examples/{n}/{n}.ja.kn")]).collect();
    assert_eq!(common::EXAMPLES.to_vec(), listed, "the tests read every example, English first");
}

/// How many properties each example has (DESIGN 17.12), and how many of them rest on what a rule
/// answers beyond what its port says, which no proof is beside.
const PROPERTIES: [(&str, usize, usize); 10] =
    [("order_flow", 1, 0), ("stock_diff", 0, 0), ("handles", 0, 0), ("catalog", 0, 0), ("payout", 0, 0), ("late_payment", 1, 0), ("media_mail", 1, 0), ("discount_split", 2, 1), ("order_bill", 1, 0), ("refund_split", 3, 0)];

/// Without `--lean`, each property of an example is W603 and nothing else is said: the proof beside
/// it waits for `--lean`, but for the one that rests on a rule's answers, which has none.
#[test]
fn every_example_passes_and_its_examples_hold() {
    for f in common::EXAMPLES {
        let o = common::check(f);
        let name = f.split('/').nth(1).unwrap();
        let (_, properties, on_rules) = PROPERTIES.iter().find(|(n, _, _)| *n == name).copied().unwrap_or_else(|| panic!("{name}"));
        assert_eq!(common::codes(&o), vec!["W603"; properties], "{f}:\n{}", common::shown(&o, Lang::En));
        let waiting = o.diags.iter().filter(|d| d.render(Lang::En).contains("is checked with `--lean`")).count();
        assert_eq!(waiting, properties - on_rules, "{f}:\n{}", common::shown(&o, Lang::En));
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
        if !rules.exists() {
            continue;
        }
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

/// The four that join no other language read nothing with `use`, and each has its host: a program in
/// TypeScript, in Python and in Go that calls the module's generated code, and what the three print
/// (ritsu's `tests/kaname.rs` runs them on the package `ritsu gen` writes).
#[test]
fn the_examples_of_a_functional_core_have_hosts() {
    for name in HOSTED {
        for kn in [format!("{name}.kn"), format!("{name}.ja.kn")] {
            let src = std::fs::read_to_string(Path::new("examples").join(name).join(&kn)).unwrap();
            assert!(!src.lines().any(|l| l.starts_with("use ")), "{name}/{kn} reads another file");
        }
        let host = Path::new("examples").join(name).join("host");
        let mut got: Vec<String> = std::fs::read_dir(&host).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        got.sort();
        assert_eq!(got, ["main.go", "main.py", "main.ts", "output.txt"], "{name}/host");
        assert!(!std::fs::read_to_string(host.join("output.txt")).unwrap().is_empty(), "{name}/host/output.txt");
    }
    for name in NAMES.iter().filter(|n| !HOSTED.contains(n)) {
        assert!(!Path::new("examples").join(name).join("host").exists(), "{name}: the code that calls other languages is held by ritsu's tests of `ritsu gen`");
    }
}
