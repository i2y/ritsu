//! Every code of the ledger (src/codes.rs) that is printed has an example that gives it, in English
//! names and in Japanese ones, checked as `ritsu sekisho check` checks it (E209's as the binary of
//! sekisho's own crate does; W401's run as the command `gen --authorizer avp` it shows); every such
//! code `check` prints has a mutant of each language in tests/mutants; and
//! `docs/codes.md` and `docs/codes.ja.md` are what `sekisho explain --all --format markdown` prints.

mod common;

use ritsu_base::ledger::{self, Repro};
use ritsu_base::text::Lang;
use sekisho::check::{Options, check_text};
use sekisho::suite::Suite;

/// The command of an example that is not `check` (W401's `gen --authorizer avp`), after `ritsu
/// sekisho`; None for one that `check` reads.
fn command_of(e: &ledger::Entry) -> Option<Vec<&'static str>> {
    match &e.repro {
        Repro::Dir { command, .. } if command.get(2).is_some_and(|c| *c != "check") => Some(command[2..].to_vec()),
        _ => None,
    }
}

#[test]
fn every_example_gives_its_code() {
    let scratch = ritsu_testkit::TempDir::new("codes");
    let failures = ledger::check_every(&sekisho::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.gate");
        // an example of `gen`: the command run on the file, with every language joined, writing in
        // the example's directory; the codes are what it prints
        if let Some(words) = command_of(e) {
            let mut args: Vec<String> = words.iter().map(|w| if *w == "example.gate" { path.to_string_lossy().to_string() } else { w.to_string() }).collect();
            args.extend(["--out".to_string(), dir.join("generated").to_string_lossy().to_string()]);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            sekisho::run::run(&args, common::joined(), &mut out, &mut err);
            let out = String::from_utf8_lossy(&out);
            return out.lines().filter_map(|l| l.split_once('[').and_then(|(_, r)| r.split_once(']')).map(|(c, _)| c.to_string())).collect();
        }
        let body = std::fs::read_to_string(&path).unwrap();
        let suite = if e.code == "E209" { Suite::default() } else { common::joined() };
        let o = check_text(path.to_str().unwrap(), &body, &suite, &Options::default());
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
fn the_ledger_has_each_code_once() {
    let l = sekisho::codes::ledger();
    assert!(ledger::duplicates(&l).is_empty(), "{:?}", ledger::duplicates(&l));
    // DESIGN 10: E001–E008, E101–E108, E201–E211 with W201, E301–E307, W301–W304, W401, and the
    // checks of security W901 and W910
    assert_eq!(l.entries.len(), 42);
}

#[test]
fn every_code_printed_has_a_mutant_in_each_language() {
    let names: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    let mut missing = Vec::new();
    // the codes `check` prints: a code of `gen` (W401) has its example, a command, in the ledger
    for e in sekisho::codes::ledger().entries.iter().filter(|e| !matches!(e.repro, Repro::Later) && command_of(e).is_none()) {
        let of = |ascii: bool| names.iter().any(|n| n.starts_with(&format!("{}_", e.code)) && n.ends_with(".gate") && n.is_ascii() == ascii);
        if !of(true) {
            missing.push(format!("{} (English)", e.code));
        }
        if !of(false) {
            missing.push(format!("{} (Japanese)", e.code));
        }
    }
    assert!(missing.is_empty(), "codes with no mutant in tests/mutants: {missing:?}");
}

#[test]
fn the_documents_of_the_codes_are_what_explain_prints() {
    let l = sekisho::codes::ledger();
    ritsu_testkit::golden("docs/codes.md", &l.render_markdown(Lang::En));
    ritsu_testkit::golden("docs/codes.ja.md", &l.render_markdown(Lang::Ja));
}
