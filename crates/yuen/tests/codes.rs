//! Every code of the ledger (src/codes.rs) has an example that, checked with what is beside
//! it, gives that code (DESIGN 6.2), and every code has at least one mutant in tests/mutants.
//! The codes that need the suite's tools to come out (E106, E107, E202–E205, W201) have their
//! examples added when yuen reads the tools (PLAN B.12).

mod common;

#[test]
fn every_example_gives_its_code() {
    let mut failures = Vec::new();
    let mut n = 0;
    for e in yuen::codes::ledger() {
        if e.later {
            assert!(e.example.is_empty(), "{} waits for the tools, and has an example", e.code);
            continue;
        }
        let dir = common::TempDir::new(&format!("codes-{}", e.code));
        for (name, body) in e.files {
            dir.write(name, body);
        }
        dir.write("example.req", e.example.as_bytes());
        let r = common::yuen(dir.path(), &["check", "example.req"]);
        let got = common::codes(&r.stdout);
        if !got.iter().any(|c| c == e.code) {
            failures.push(format!("{}: got {:?}\n{}{}", e.code, got, r.stdout, r.stderr));
        }
        n += 1;
    }
    assert_eq!(n, 36, "36 codes are reproduced in this stage");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_code_has_a_mutant() {
    let names: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    let missing: Vec<&str> = yuen::codes::ledger().iter().filter(|e| !e.later).map(|e| e.code).filter(|c| !names.iter().any(|n| n.starts_with(&format!("{c}_")))).collect();
    assert!(missing.is_empty(), "codes with no mutant in tests/mutants: {missing:?}");
}

#[test]
fn the_ledger_has_each_code_of_design_once_and_in_order() {
    let codes: Vec<&str> = yuen::codes::ledger().iter().map(|e| e.code).collect();
    let mut seen = std::collections::HashSet::new();
    for c in &codes {
        assert!(seen.insert(*c), "{c} is in the ledger twice");
    }
    // DESIGN 6.2's table, in its order.
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let table: Vec<String> = design.lines().filter_map(|l| l.strip_prefix("| ")).filter_map(|l| l.split(' ').next()).filter(|c| c.len() == 4 && (c.starts_with('E') || c.starts_with('W')) && c[1..].chars().all(|d| d.is_ascii_digit())).map(|s| s.to_string()).collect();
    assert_eq!(codes, table, "the ledger is DESIGN 6.2's table");
    assert_eq!(codes.len(), 43);
}
