//! ritsu's ledger (DESIGN 4.3, 7.1): no code twice, and `docs/codes.md` and `docs/codes.ja.md`
//! are what `ritsu explain --all --format markdown` writes. That every reproduction prints its
//! code is `crates/ritsu/tests/codes.rs`, which runs the `ritsu` binary on them.

use ritsu_base::text::Lang;
use std::path::Path;

#[test]
fn no_code_is_in_the_ledger_twice() {
    let l = ritsu_cross::codes::ledger();
    assert!(ritsu_base::ledger::duplicates(&l).is_empty(), "{:?}", ritsu_base::ledger::duplicates(&l));
    assert!(l.entries.iter().all(|e| e.code.starts_with(['E', 'W', 'N']) && e.code.len() == 4), "a code is a letter and three digits");
}

/// The pages are the ledger's Markdown, written by `ritsu explain --all --format markdown`; with
/// RITSU_BLESS=1 the test writes them.
#[test]
fn the_pages_of_codes_are_the_ledger() {
    let l = ritsu_cross::codes::ledger();
    let mut failures = Vec::new();
    for (file, lang) in [("docs/codes.md", Lang::En), ("docs/codes.ja.md", Lang::Ja)] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
        if let Err(e) = ritsu_testkit::golden::check(&path, &l.render_markdown(lang)) {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
