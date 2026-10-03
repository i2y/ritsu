//! The diagnostics of every book in tests/fixtures, in English and Japanese, against their
//! goldens; every code the checker has appears in them in both languages.

mod common;
use chobo::diag::Show;
use chobo::{check, codes, diffbase};
use ritsu_base::text::Lang;
use common::*;

/// The text `chobo check` prints for a fixture: compared with its `.before.book` when there
/// is one, as `--diff-base` would.
fn render(path: &std::path::Path, lang: Lang) -> String {
    let src = std::fs::read_to_string(path).unwrap();
    let mut c = check::check_source(&src);
    let before = path.with_file_name(format!("{}.before.book", stem(path)));
    if let (Ok(b), Some(book)) = (std::fs::read_to_string(&before), &c.book) {
        let (more, note) = diffbase::against(book, Some(&b), "HEAD");
        assert!(note.is_none());
        c.diags.extend(more);
        chobo::model::sort(&mut c.diags);
        if chobo::diag::has_errors(&c.diags) {
            c.report = None;
        }
    }
    let file = path.file_name().unwrap().to_string_lossy().to_string();
    // a fixture with a `.target` beside it: what `chobo build --target <it>` prints
    if let Ok(t) = std::fs::read_to_string(path.with_file_name(format!("{}.target", stem(path)))) {
        let target = chobo::target::Target::parse(t.trim()).unwrap_or_else(|| panic!("{}: no target {t}", path.display()));
        let book = c.book.as_ref().unwrap_or_else(|| panic!("{}: the book does not load", path.display()));
        assert!(!chobo::diag::has_errors(&c.diags), "{}: the book has errors before it is built", path.display());
        let mut d = c.diags.clone();
        d.extend(chobo::target::check(book, target));
        let mut out: String = d.iter().map(|x| x.shown(&file, &src, lang)).collect();
        out.push_str(&chobo::diag::summary(&file, &d, lang));
        out.push('\n');
        return out;
    }
    check::render(&file, &src, &c, lang)
}

#[test]
fn fixtures_match_their_goldens() {
    for p in books_in("tests/fixtures") {
        for (lang, ext) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let out = render(&p, lang);
            golden(&p.with_file_name(format!("{}.{ext}.txt", stem(&p))), &out);
        }
    }
}

#[test]
fn every_code_appears_in_both_languages() {
    let mut texts = [String::new(), String::new()];
    for p in books_in("tests/fixtures") {
        for (i, ext) in ["en", "ja"].iter().enumerate() {
            texts[i].push_str(&std::fs::read_to_string(p.with_file_name(format!("{}.{ext}.txt", stem(&p)))).unwrap_or_default());
        }
    }
    for e in codes::ledger() {
        for (i, ext) in ["en", "ja"].iter().enumerate() {
            assert!(texts[i].contains(&format!("[{}]", e.code)), "{} is in no {ext} golden", e.code);
        }
    }
}

#[test]
fn the_test_books_check_clean() {
    for p in books_in("tests/books") {
        let src = std::fs::read_to_string(&p).unwrap();
        let c = check::check_source(&src);
        let shown: Vec<String> = c.diags.iter().map(|d| d.shown(&stem(&p), &src, Lang::En)).collect();
        assert!(c.diags.is_empty(), "{}:\n{}", p.display(), shown.join(""));
        assert!(c.report.is_some());
    }
}

/// The book in DESIGN 1.1 is a book chobo accepts.
#[test]
fn the_design_book_checks() {
    let design = std::fs::read_to_string(root().join("DESIGN.md")).unwrap();
    let at = design.find("### 1.1").unwrap();
    let rest = &design[at..];
    let start = rest.find("```\n").unwrap() + 4;
    let end = start + rest[start..].find("```").unwrap();
    let src = &rest[start..end];
    let c = check::check_source(src);
    assert!(c.diags.is_empty(), "{:?}", c.diags.iter().map(|d| d.message.en.clone()).collect::<Vec<_>>());
}
