//! Every code of the ledger (src/codes.rs) that is printed has an example that gives it, in English
//! names and in Japanese ones, checked as `ritsu kaname check` checks it (E202's as the binary of
//! kaname's own crate does); and `docs/codes.md` and `docs/codes.ja.md` are what `kaname explain
//! --all --format markdown` prints.

mod common;

use kaname::check::Run;
use kaname::suite::Suite;
use ritsu_base::ledger::{self, Repro};
use ritsu_base::text::Lang;

#[test]
fn every_example_gives_its_code() {
    let scratch = ritsu_testkit::TempDir::new("codes");
    let failures = ledger::check_every(&kaname::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.kn");
        let suite = if e.code == "E202" { Suite::default() } else { common::joined() };
        let o = Run::new(suite).file(path.to_str().unwrap()).unwrap();
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
    let scratch = ritsu_testkit::TempDir::new("codes-alone");
    let mut noisy = Vec::new();
    ledger::check_every(&kaname::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.kn");
        let suite = if e.code == "E202" { Suite::default() } else { common::joined() };
        let o = Run::new(suite).file(path.to_str().unwrap()).unwrap();
        let codes: Vec<String> = o.diags.iter().map(|d| d.code.to_string()).collect();
        if codes.iter().any(|c| c != e.code) {
            noisy.push(format!("{}: {codes:?}", e.code));
        }
        codes
    });
    assert!(noisy.is_empty(), "{}", noisy.join("\n"));
}

#[test]
fn the_ledger_has_each_code_once() {
    let l = kaname::codes::ledger();
    assert!(ledger::duplicates(&l).is_empty(), "{:?}", ledger::duplicates(&l));
    // DESIGN 12: E001–E007, E101–E111, E201–E204, E301–W303, E401, W501, E601, E602, W901
    assert_eq!(l.entries.len(), 33);
    // every code is printed by now, and has its example
    let later: Vec<&str> = l.entries.iter().filter(|e| matches!(e.repro, Repro::Later)).map(|e| e.code).collect();
    assert!(later.is_empty(), "{later:?}");
}

#[test]
fn the_documents_of_the_codes_are_what_explain_prints() {
    let l = kaname::codes::ledger();
    ritsu_testkit::golden("docs/codes.md", &l.render_markdown(Lang::En));
    ritsu_testkit::golden("docs/codes.ja.md", &l.render_markdown(Lang::Ja));
}

/// What `kaname check` says of each example, in English and in Japanese: the text a person reads,
/// held to `tests/golden/codes/` (`KANAME_BLESS=1` writes them again). The directory the example is
/// laid out in is left out of the paths.
#[test]
fn the_examples_say_what_the_golden_files_hold() {
    let scratch = ritsu_testkit::TempDir::new("codes-golden");
    let mut failures = Vec::new();
    ledger::check_every(&kaname::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.kn");
        let suite = if e.code == "E202" { Suite::default() } else { common::joined() };
        let o = Run::new(suite).file(path.to_str().unwrap()).unwrap();
        let ja = e.repro_ja.is_none() && dir.file_name().is_some_and(|n| n.to_string_lossy().ends_with("-ja"));
        let lang = if ja { Lang::Ja } else { Lang::En };
        let prefix = format!("{}/", dir.display());
        let said = kaname::check::render(&o, lang).replace(&prefix, "");
        let name = format!("tests/golden/codes/{}.{}.txt", e.code, lang.code());
        if let Err(x) = ritsu_testkit::golden::check(std::path::Path::new(&name), &said) {
            failures.push(x);
        }
        o.diags.iter().map(|d| d.code.to_string()).collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
