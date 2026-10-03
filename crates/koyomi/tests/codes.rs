//! Every code of the ledger (src/codes.rs) has an example that, checked with what is beside
//! it, gives that code (DESIGN 4.2), and every code has at least one mutant in tests/mutants.

use koyomi::check::{Options, check_text};
use ritsu_base::ledger;
use ritsu_base::text::Lang;

#[test]
fn every_example_gives_its_code() {
    let scratch = ritsu_testkit::TempDir::new("codes");
    let failures = ledger::check_every(&koyomi::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.cal");
        let body = std::fs::read_to_string(&path).unwrap();
        let o = check_text(path.to_str().unwrap(), &body, &Options::default());
        let codes: Vec<String> = o.diags.iter().map(|d| d.code.to_string()).collect();
        if !codes.iter().any(|c| c == e.code) {
            let text: String = o.diags.iter().map(|d| d.render(Lang::En)).collect();
            eprintln!("{}:\n{text}", e.code);
        }
        codes
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_code_has_a_mutant() {
    let names: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    let missing: Vec<&str> = koyomi::codes::ledger().entries.iter().map(|e| e.code).filter(|c| !names.iter().any(|n| n.starts_with(&format!("{c}_")))).collect();
    assert!(missing.is_empty(), "codes with no mutant in tests/mutants: {missing:?}");
}

#[test]
fn the_ledger_has_each_code_once_and_in_order() {
    let codes: Vec<&str> = koyomi::codes::ledger().entries.iter().map(|e| e.code).collect();
    let mut seen = std::collections::HashSet::new();
    for c in &codes {
        assert!(seen.insert(*c), "{c} is in the ledger twice");
    }
    assert_eq!(codes.len(), 39, "DESIGN 4.2 has 39 codes");
}
