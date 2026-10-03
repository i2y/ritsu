//! Every code of the ledger (src/codes.rs) that sakai prints has a reproduction that gives it
//! (DESIGN 5.2), the reproductions start from a map that passes, and every code has at least
//! one mutant in tests/mutants.

mod common;

use sakai::check::check_args;
use sakai::i18n::Lang;

fn run(files: &[(&str, &str)]) -> Vec<sakai::check::Outcome> {
    let dir = common::TempDir::new();
    for (name, body) in files {
        dir.write(name, body);
    }
    check_args(dir.path(), &[".".to_string()]).unwrap()
}

/// The codes a reproduction gives, and what it printed: `check .` in the library, another command
/// (`build …`) with the binary in the reproduction's directory.
fn codes_of(e: &sakai::codes::Entry) -> (Vec<String>, String) {
    if e.command.is_empty() {
        let os = run(&e.reproduction());
        let codes = os.iter().flat_map(|o| o.diags.iter().map(|d| d.code.to_string())).collect();
        return (codes, os.iter().flat_map(|o| o.diags.iter().map(|d| d.render(Lang::En))).collect());
    }
    let dir = common::TempDir::new();
    for (name, body) in e.reproduction() {
        dir.write(name, body);
    }
    let o = common::sakai_in(dir.path(), &e.run());
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    (common::printed_codes(&out), out)
}

#[test]
fn the_base_of_the_reproductions_passes() {
    let os = run(sakai::codes::BASE);
    let ds: Vec<String> = os.iter().flat_map(|o| o.diags.iter().map(|d| d.render(Lang::En))).collect();
    assert!(ds.is_empty(), "{}", ds.join(""));
    assert!(os.iter().any(|o| o.summary.is_some()));
}

#[test]
fn every_reproduction_gives_its_code() {
    let mut failures = Vec::new();
    let mut n = 0;
    for e in sakai::codes::ledger() {
        if !e.implemented() {
            continue;
        }
        let (codes, text) = codes_of(&e);
        if !codes.iter().any(|c| c == e.code) {
            failures.push(format!("{}: got {:?}\n{text}", e.code, codes));
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(n >= 51, "{n} reproductions");
}

/// The codes with no reproduction are those of the suite's tools, which sakai does not read yet
/// (PLAN C.1 to C.5); every other code is printed, and reproduced.
#[test]
fn the_codes_not_printed_yet_are_the_suites() {
    let later = ["E104", "E105", "N101", "E405"];
    for e in sakai::codes::ledger() {
        assert_eq!(e.implemented(), !later.contains(&e.code), "{}", e.code);
    }
}

#[test]
fn every_code_has_a_mutant() {
    let names = common::mutants();
    let missing: Vec<&str> = sakai::codes::ledger().iter().filter(|e| e.implemented()).map(|e| e.code).filter(|c| !names.iter().any(|n| n.starts_with(&format!("{c}_")))).collect();
    assert!(missing.is_empty(), "codes with no mutant in tests/mutants: {missing:?}");
}

#[test]
fn the_ledger_has_each_code_of_design_once_and_in_order() {
    let codes: Vec<&str> = sakai::codes::ledger().iter().map(|e| e.code).collect();
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let table: Vec<&str> = design.lines().filter_map(|l| l.strip_prefix("| ")).filter_map(|l| l.split(" |").next()).filter(|c| c.len() == 4 && c[1..].chars().all(|d| d.is_ascii_digit()) && "EWN".contains(&c[..1])).collect();
    assert_eq!(codes, table, "the ledger and the table of DESIGN 5.2");
}
