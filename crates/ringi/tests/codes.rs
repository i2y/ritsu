//! Every code of the ledger (src/codes.rs) that is printed by now has an example that gives it, in
//! English names and in Japanese ones, checked as `ritsu ringi check` checks it (E008's as the
//! binary of ringi's own crate does); and `docs/codes.md` and `docs/codes.ja.md` are what `ringi
//! explain --all --format markdown` prints.

mod common;

use ringi::check::Run;
use ringi::suite::Suite;
use ritsu_base::ledger::{self, Repro};
use ritsu_base::text::Lang;

/// What the code of `e` is given for its example in `dir`: `ringi check`'s answer, as `ritsu ringi
/// check` checks it, E008's as the binary of ringi's own crate does, and E704's as `ritsu gen
/// --target requests` checks the screens before it makes them (DESIGN 4.8, 13.3).
fn run_example(e: &ledger::Entry, dir: &std::path::Path) -> std::rc::Rc<ringi::check::Outcome> {
    let path = dir.join("example.ringi");
    if e.code == "E704" {
        return common::check_screens(path.to_str().unwrap());
    }
    let suite = if e.code == "E008" { Suite::default() } else { common::joined() };
    Run::new(suite).file(path.to_str().unwrap()).unwrap()
}

#[test]
fn every_example_gives_its_code() {
    let scratch = ritsu_testkit::TempDir::new("ringi-codes");
    let failures = ledger::check_every(&ringi::codes::ledger(), scratch.path(), |e, dir| {
        let o = run_example(e, dir);
        let codes: Vec<String> = o.diags.iter().map(|d| d.code.to_string()).collect();
        if !codes.iter().any(|c| c == e.code) {
            let text: String = o.diags.iter().map(|d| d.render(Lang::En)).collect();
            eprintln!("{}:\n{text}", e.code);
        }
        codes
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// An example prints its code and no other: what the reader sees is about that code alone.
#[test]
fn every_example_gives_its_code_alone() {
    let scratch = ritsu_testkit::TempDir::new("ringi-codes-alone");
    let mut noisy = Vec::new();
    ledger::check_every(&ringi::codes::ledger(), scratch.path(), |e, dir| {
        let o = run_example(e, dir);
        let codes: Vec<String> = o.diags.iter().map(|d| d.code.to_string()).collect();
        if codes.iter().any(|c| c != e.code) {
            noisy.push(format!("{}: {codes:?}", e.code));
        }
        codes
    });
    assert!(noisy.is_empty(), "{}", noisy.join("\n"));
}

/// The codes with no example: checks of the screens no file gives, said only of a bug in ringi
/// (DESIGN 13.3).
const LATER: [&str; 2] = ["E701", "W702"];

#[test]
fn the_ledger_has_each_code_once() {
    let l = ringi::codes::ledger();
    assert!(ledger::duplicates(&l).is_empty(), "{:?}", ledger::duplicates(&l));
    // DESIGN 9.5: E001–E009, E101–W109, E201–E203 and W201, E301–E309, E401–E409, W500–E511,
    // E601–E604, E701–E704, W901
    assert_eq!(l.entries.len(), 63);
    // every code is printed with its examples, but the two of the screens no file gives
    let later: Vec<&str> = l.entries.iter().filter(|e| matches!(e.repro, Repro::Later)).map(|e| e.code).collect();
    assert_eq!(later, LATER);
    for e in l.entries.iter().filter(|e| !LATER.contains(&e.code)) {
        assert!(e.repro_ja.is_some(), "{} has no example in Japanese names", e.code);
    }
}

#[test]
fn the_documents_of_the_codes_are_what_explain_prints() {
    let l = ringi::codes::ledger();
    ritsu_testkit::golden("docs/codes.md", &l.render_markdown(Lang::En));
    ritsu_testkit::golden("docs/codes.ja.md", &l.render_markdown(Lang::Ja));
}

/// What `ringi check` says of each example, in English and in Japanese: the text a person reads,
/// held to `tests/golden/codes/` (`RINGI_BLESS=1` writes them again). The directory the example is
/// laid out in is left out of the paths.
#[test]
fn the_examples_say_what_the_golden_files_hold() {
    let scratch = ritsu_testkit::TempDir::new("ringi-codes-golden");
    let mut failures = Vec::new();
    ledger::check_every(&ringi::codes::ledger(), scratch.path(), |e, dir| {
        let o = run_example(e, dir);
        let ja = e.repro_ja.is_none() && dir.file_name().is_some_and(|n| n.to_string_lossy().ends_with("-ja"));
        let lang = if ja { Lang::Ja } else { Lang::En };
        let prefix = format!("{}/", dir.display());
        let said = ringi::check::render(&o, lang).replace(&prefix, "");
        let name = format!("tests/golden/codes/{}.{}.txt", e.code, lang.code());
        if let Err(x) = ritsu_testkit::golden::check(std::path::Path::new(&name), &said) {
            failures.push(x);
        }
        o.diags.iter().map(|d| d.code.to_string()).collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
