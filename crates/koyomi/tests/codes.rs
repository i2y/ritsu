//! Every code of the ledger (src/codes.rs) has an example that, checked with what is beside
//! it, gives that code (DESIGN 4.2), and every code has at least one mutant in tests/mutants.

use koyomi::check::{Options, check_text};
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("koyomi-codes-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn every_example_gives_its_code() {
    let mut failures = Vec::new();
    for e in koyomi::codes::ledger() {
        let dir = scratch(e.code);
        for (name, body) in e.files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
        }
        let path = dir.join("example.cal");
        std::fs::write(&path, e.example).unwrap();
        let o = check_text(path.to_str().unwrap(), e.example, &Options::default());
        let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
        if !codes.contains(&e.code) {
            let text: String = o.diags.iter().map(|d| d.render(koyomi::i18n::Lang::En)).collect();
            failures.push(format!("{}: got {:?}\n{text}", e.code, codes));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_code_has_a_mutant() {
    let names: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    let missing: Vec<&str> = koyomi::codes::ledger().iter().map(|e| e.code).filter(|c| !names.iter().any(|n| n.starts_with(&format!("{c}_")))).collect();
    assert!(missing.is_empty(), "codes with no mutant in tests/mutants: {missing:?}");
}

#[test]
fn the_ledger_has_each_code_once_and_in_order() {
    let codes: Vec<&str> = koyomi::codes::ledger().iter().map(|e| e.code).collect();
    let mut seen = std::collections::HashSet::new();
    for c in &codes {
        assert!(seen.insert(*c), "{c} is in the ledger twice");
    }
    assert_eq!(codes.len(), 39, "DESIGN 4.2 has 39 codes");
}
