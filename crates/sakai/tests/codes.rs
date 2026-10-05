//! Every code of the ledger (src/codes.rs) that sakai prints has a reproduction that gives it
//! (DESIGN 5.2), the reproductions start from a map that passes, and every code has at least
//! one mutant in tests/mutants.

mod common;

use sakai::check::check_args;
use ritsu_base::text::Lang;

/// `check .` of the files, with every language joined (`ritsu sakai`), or with none for E104,
/// which only the binary of sakai's own crate gives.
fn run_joined(files: &[(&str, &str)], joined: bool) -> Vec<sakai::check::Outcome> {
    let dir = common::TempDir::new("check");
    for (name, body) in files {
        dir.write(name, body);
    }
    if joined { common::check_dir(dir.path()) } else { check_args(dir.path(), &[".".to_string()]).unwrap() }
}

fn run(files: &[(&str, &str)]) -> Vec<sakai::check::Outcome> {
    run_joined(files, true)
}

/// The codes a reproduction gives, and what it printed: `check .` in the library, with no other
/// language joined (`sakai check .`) or every one (`ritsu sakai check .`); another command
/// (`build …`) with the binary in the reproduction's directory.
fn codes_of(e: &ritsu_base::ledger::Entry) -> (Vec<String>, String) {
    let (command, files) = sakai::codes::reproduction(e);
    let joined = match command.as_slice() {
        ["check", "."] => Some(false),
        ["ritsu", "sakai", "check", "."] => Some(true),
        _ => None,
    };
    if let Some(joined) = joined {
        let os = run_joined(&files, joined);
        let codes = os.iter().flat_map(|o| o.diags.iter().map(|d| d.code.to_string())).collect();
        return (codes, os.iter().flat_map(|o| o.diags.iter().map(|d| d.render(Lang::En))).collect());
    }
    let dir = common::TempDir::new("command");
    for (name, body) in files {
        dir.write(name, body);
    }
    let o = common::sakai_in(dir.path(), &command);
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    (common::printed_codes(&out), out)
}

#[test]
fn the_base_of_the_reproductions_passes() {
    // both the base with Japanese names and the one with English names
    for base in [sakai::codes::BASE, sakai::codes::BASE_EN] {
        let os = run(base);
        let ds: Vec<String> = os.iter().flat_map(|o| o.diags.iter().map(|d| d.render(Lang::En))).collect();
        assert!(ds.is_empty(), "{}", ds.join(""));
        assert!(os.iter().any(|o| o.summary.is_some()));
    }
}

/// Every reproduction gives its code, in the names it is written in: the English one, which
/// `explain` shows in English, and the Japanese one, which it shows in Japanese.
#[test]
fn every_reproduction_gives_its_code() {
    let mut failures = Vec::new();
    let (mut n, mut japanese) = (0, 0);
    for e in sakai::codes::ledger().entries {
        if !sakai::codes::implemented(&e) {
            continue;
        }
        let mut shown = vec![(e.shown_in(Lang::En), "en")];
        if e.repro_ja.is_some() {
            shown.push((e.shown_in(Lang::Ja), "ja"));
        }
        for (entry, tag) in shown {
            let (codes, text) = codes_of(&entry);
            if !codes.iter().any(|c| c == e.code) {
                failures.push(format!("{} ({tag}): got {:?}\n{text}", e.code, codes));
            }
            n += 1;
            japanese += (tag == "ja") as usize;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(n >= 61, "{n} reproductions");
    assert_eq!(japanese, 61, "every reproduction has its Japanese twin");
}

/// Every code is printed and reproduced, but the one retired (N101): its entry stays, with why it
/// was retired, and its number is given to nothing else (ritsu's DESIGN 7.10).
#[test]
fn the_retired_code_stays_in_the_ledger() {
    for e in sakai::codes::ledger().entries {
        assert_eq!(sakai::codes::implemented(&e), e.code != "N101", "{}", e.code);
        assert_eq!(e.is_retired(), e.code == "N101", "{}", e.code);
    }
    let ledger = sakai::codes::ledger();
    let text = ledger.render_text(ledger.find("N101").unwrap(), Lang::En);
    assert!(text.contains("Retired in ritsu 0.23.0"), "{text}");
}

#[test]
fn every_code_has_a_mutant() {
    let names = common::mutants();
    let missing: Vec<&str> = sakai::codes::ledger().entries.iter().filter(|e| sakai::codes::implemented(e)).map(|e| e.code).filter(|c| !names.iter().any(|n| n.starts_with(&format!("{c}_")))).collect();
    assert!(missing.is_empty(), "codes with no mutant in tests/mutants: {missing:?}");
}

#[test]
fn the_ledger_has_each_code_of_design_once_and_in_order() {
    let codes: Vec<&str> = sakai::codes::ledger().entries.iter().map(|e| e.code).collect();
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let table: Vec<&str> = design.lines().filter_map(|l| l.strip_prefix("| ")).filter_map(|l| l.split(" |").next()).filter(|c| c.len() == 4 && c[1..].chars().all(|d| d.is_ascii_digit()) && "EWN".contains(&c[..1])).collect();
    assert_eq!(codes, table, "the ledger and the table of DESIGN 5.2");
}

/// Every mutant with a Japanese name has an English one that gives the same code, beside it
/// (the pairs are told by the code the names start with): a code has at least as many mutants of
/// English names as of Japanese ones.
#[test]
fn every_japanese_mutant_has_an_english_one() {
    let names = common::mutants();
    let mut codes: Vec<&str> = names.iter().map(|n| n.split('_').next().unwrap()).collect();
    codes.sort();
    codes.dedup();
    let mut failures = Vec::new();
    let mut japanese = 0;
    for code in codes {
        let of_code: Vec<&String> = names.iter().filter(|n| n.starts_with(&format!("{code}_"))).collect();
        let ja = of_code.iter().filter(|n| !n.is_ascii()).count();
        let en = of_code.iter().filter(|n| n.is_ascii()).count();
        japanese += ja;
        if en < ja {
            failures.push(format!("{code}: {ja} mutants of Japanese names, {en} of English ones"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // the 64 of before, and the 10 on the example of OpenAPI and AsyncAPI documents (DESIGN 15)
    assert_eq!(japanese, 74, "the Japanese mutants are all kept");
}
