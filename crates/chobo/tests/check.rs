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

/// Every fixture with a Japanese name has one with an English name (DESIGN 10.1): the same book
/// written in English, which gives the same codes, and fails or passes as the Japanese one does.
const TWINS: &[(&str, &str)] = &[
    ("型", "types"),
    ("数", "numbers"),
    ("キー", "keys"),
    ("二度", "twice"),
    ("単位", "units"),
    ("名前", "names"),
    ("境界", "bounds"),
    ("変更", "changes"),
    ("構文", "syntax"),
    ("順序", "order"),
    ("たまる", "piles_up"),
    ("税区分", "tax_kinds"),
    ("仮押さえ", "holds"),
    ("効かない", "no_effect"),
    ("同じ勘定", "same_account"),
    ("引数の数", "argument_count"),
    ("拒否される", "refused"),
    ("移動なし", "no_moves"),
    ("リクエスト", "requests"),
    ("使われない", "unused"),
    ("名前の長さ", "name_length"),
    ("仮押さえの順序", "hold_order"),
    ("W901_文字列の鍵", "W901_key_in_a_string"),
    ("W901_コメントの鍵", "W901_key_in_a_comment"),
    ("W901_テスト用の鍵", "W901_test_secret"),
];

#[test]
fn every_japanese_fixture_has_an_english_one() {
    let said = |name: &str| -> (Vec<String>, bool) {
        let text = render(&root().join(format!("tests/fixtures/{name}.book")), Lang::En);
        let mut codes: Vec<String> = text.match_indices('[').filter_map(|(i, _)| text.get(i + 1..i + 5)).filter(|c| c.len() == 4 && (c.starts_with('E') || c.starts_with('W')) && c[1..].chars().all(|d| d.is_ascii_digit())).map(str::to_string).collect();
        codes.sort();
        codes.dedup();
        (codes, text.contains("error["))
    };
    let japanese: Vec<String> = books_in("tests/fixtures").iter().map(|p| stem(p)).filter(|s| !s.is_ascii()).collect();
    for j in &japanese {
        let Some((_, e)) = TWINS.iter().find(|(a, _)| a == j) else { panic!("{j}.book has no English fixture in TWINS") };
        assert_eq!(said(j), said(e), "{j}.book and {e}.book do not say the same");
        if root().join(format!("tests/fixtures/{j}.before.book")).exists() {
            assert!(root().join(format!("tests/fixtures/{e}.before.book")).exists(), "{e}.before.book");
        }
        if root().join(format!("tests/fixtures/{j}.target")).exists() {
            assert_eq!(std::fs::read_to_string(root().join(format!("tests/fixtures/{j}.target"))).unwrap(), std::fs::read_to_string(root().join(format!("tests/fixtures/{e}.target"))).unwrap());
        }
    }
    assert_eq!(japanese.len(), TWINS.len());
}
