//! Every code of the ledger (src/codes.rs) has an example that, checked with what is beside
//! it, gives that code (DESIGN 6.2), and every code has at least one mutant in tests/mutants.
//! The examples are checked with every language joined, as `ritsu yuen` checks them: some need a
//! rule or a calendar to come out (E106, E107, E203). The retired codes (E204, W201) keep their
//! entries and print nothing.

mod common;

#[test]
fn every_example_gives_its_code() {
    let ledger = yuen::codes::ledger();
    let scratch = ritsu_testkit::TempDir::new("codes");
    let mut n = 0;
    let failures = ritsu_base::ledger::check_every(&ledger, scratch.path(), |e, dir| {
        n += 1;
        let example = dir.join("example.req").to_string_lossy().to_string();
        let r = common::run(&["check", &example]);
        let got = common::codes(&r.stdout);
        if !got.iter().any(|c| c == e.code) {
            eprintln!("{}:\n{}{}", e.code, r.stdout, r.stderr);
        }
        got
    });
    assert_eq!(n, 41, "every code but the two retired ones is reproduced");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_code_has_a_mutant() {
    let names: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    let missing: Vec<&str> = yuen::codes::ledger().entries.iter().filter(|e| matches!(e.repro, ritsu_base::ledger::Repro::File { .. })).map(|e| e.code).filter(|c| !names.iter().any(|n| n.starts_with(&format!("{c}_")))).collect();
    assert!(missing.is_empty(), "codes with no mutant in tests/mutants: {missing:?}");
}

/// A retired code stays in the ledger with why it was retired, and nothing prints it: the codes
/// the check can give are the others (ritsu's DESIGN 7.10).
#[test]
fn the_retired_codes_stay_in_the_ledger() {
    let ledger = yuen::codes::ledger();
    let retired: Vec<&str> = ledger.entries.iter().filter(|e| e.is_retired()).map(|e| e.code).collect();
    assert_eq!(retired, ["E204", "W201"]);
    for code in retired {
        let e = ledger.find(code).unwrap();
        let text = ledger.render_text(e, ritsu_base::text::Lang::En);
        assert!(text.contains("Retired in ritsu 0.23.0"), "{text}");
    }
    // no source file of yuen prints them
    for f in std::fs::read_dir("src").unwrap().chain(std::fs::read_dir("src/export").unwrap()) {
        let path = f.unwrap().path();
        if path.extension().is_some_and(|x| x == "rs") && !path.ends_with("codes.rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains("\"E204\"") && !text.contains("\"W201\""), "{} prints a retired code", path.display());
        }
    }
}

#[test]
fn the_ledger_has_each_code_of_design_once_and_in_order() {
    let codes: Vec<&str> = yuen::codes::ledger().entries.iter().map(|e| e.code).collect();
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
