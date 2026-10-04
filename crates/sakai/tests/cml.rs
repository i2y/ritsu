//! `sakai export cml` (DESIGN 8; PLAN C.14): the example's CML is its golden file, in English
//! and in Japanese, and Context Mapper 6.12.0's validator, with every check, finds nothing in
//! either. The CLI's `cm validate` reads the syntax only (DESIGN 0.4), so the test runs
//! `tools/cml/Validate.java` on the CLI's jars; a CML that breaks one of its semantic rules shows
//! that the validator is at work.

mod common;

use ritsu_base::text::Lang;
use std::process::Command;
use std::time::Duration;

fn example_cml(lang: Lang) -> String {
    example_cml_of(common::EXAMPLE, "通販.ctx", lang)
}

fn example_cml_of(example: &str, map: &str, lang: Lang) -> String {
    let ex = std::fs::canonicalize(example).unwrap();
    let o = sakai::check::check_map_with(&ex, map, &common::suite()).unwrap();
    assert!(!o.has_errors());
    sakai::cml::render(o.checked.as_ref().unwrap(), map, lang)
}

#[test]
fn the_cml_of_the_example_is_its_golden_file() {
    let mut failures = Vec::new();
    for (lang, f) in [(Lang::En, "tests/golden/cml/通販.cml"), (Lang::Ja, "tests/golden/cml/通販.ja.cml")] {
        if let Some(x) = common::golden(f, &example_cml(lang)) {
            failures.push(x);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The command writes what the library renders, to a file or to standard output.
#[test]
fn export_cml_on_the_command_line() {
    let dir = common::TempDir::new("export");
    let out = dir.path().join("shop.cml");
    // the example holds rules, calendars and workflows: every language joined, as `ritsu sakai`
    let (code, _, err) = common::joined(&["export", "cml", "examples/shop.ja/通販.ctx", "--out", out.to_str().unwrap(), "--root", "examples/shop.ja"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), example_cml(Lang::En));
    let (_, got, _) = common::joined(&["export", "cml", "examples/shop.ja/通販.ctx", "--lang", "ja", "--root", "examples/shop.ja"]);
    assert_eq!(got, example_cml(Lang::Ja));
    // `export` names the form it writes; a map with errors writes nothing.
    assert_eq!(common::sakai(&["export", "examples/shop.ja/通販.ctx"]).status.code(), Some(2));
    let bad = common::mutant("E401_注文の状態に値が増えた");
    let o = common::sakai_in(bad.path(), &["export", "cml", "基本.ctx"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty() && String::from_utf8_lossy(&o.stderr).contains("error[E401]"));
}

#[test]
fn context_mapper_finds_nothing_wrong_in_it() {
    if !common::linters() {
        return;
    }
    let (Some(java), Some(javac), Some(lib)) = (common::java("java"), common::java("javac"), common::cml_lib()) else {
        common::skip("Java or the Context Mapper CLI is not there (SAKAI_JAVA and SAKAI_JAVAC, or JAVA_HOME; SAKAI_CML_LIB, or tools/cml: tools/cml/fetch.sh)");
        return;
    };
    let dir = common::TempDir::new("context-mapper");
    let classes = dir.path().join("classes");
    let cp = format!("{}/*", lib.display());
    let limit = Duration::from_secs(180);
    let b = common::run(Command::new(&javac).arg("-cp").arg(&cp).arg("-d").arg(&classes).arg("tools/cml/Validate.java"), limit);
    assert!(b.ok, "javac: {}", b.stderr);
    dir.write("shop.cml", &example_cml(Lang::En));
    dir.write("shop.ja.cml", &example_cml(Lang::Ja));
    // A customer and supplier that conforms: Context Mapper's rules refuse it.
    dir.write("wrong.cml", "ContextMap m {\n  contains A, B\n  A [D,C,CF]<-[U,S] B\n}\nBoundedContext A\nBoundedContext B\n");
    let run = |f: &str| common::run(Command::new(&java).arg("-cp").arg(format!("{cp}:{}", classes.display())).arg("Validate").arg(f).current_dir(dir.path()), limit);
    for f in ["shop.cml", "shop.ja.cml"] {
        let r = run(f);
        assert!(r.ok && r.stdout.trim().is_empty(), "{f}: {}{}", r.stdout, r.stderr);
    }
    let r = run("wrong.cml");
    assert!(!r.ok && r.stdout.contains("The CONFORMIST pattern is not applicable for a Customer-Supplier relationship."), "{}", r.stdout);
}

// ── The English twins: the English example's CML, in English and in Japanese ──

#[test]
fn the_cml_of_the_english_example_is_its_golden_file() {
    let mut failures = Vec::new();
    for (lang, f) in [(Lang::En, "tests/golden/cml/shop.cml"), (Lang::Ja, "tests/golden/cml/shop.ja.cml")] {
        if let Some(x) = common::golden(f, &example_cml_of(common::EXAMPLE_EN, "shop.ctx", lang)) {
            failures.push(x);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn export_cml_on_the_command_line_in_english() {
    let dir = common::TempDir::new("export-en");
    let out = dir.path().join("shop.cml");
    let (code, _, err) = common::joined(&["export", "cml", "examples/shop/shop.ctx", "--out", out.to_str().unwrap(), "--root", "examples/shop"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), example_cml_of(common::EXAMPLE_EN, "shop.ctx", Lang::En));
    let (_, got, _) = common::joined(&["export", "cml", "examples/shop/shop.ctx", "--lang", "ja", "--root", "examples/shop"]);
    assert_eq!(got, example_cml_of(common::EXAMPLE_EN, "shop.ctx", Lang::Ja));
    assert_eq!(common::sakai(&["export", "examples/shop/shop.ctx"]).status.code(), Some(2));
    let bad = common::mutant("E401_value_added_to_the_order_status");
    let o = common::sakai_in(bad.path(), &["export", "cml", "basic.ctx"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty() && String::from_utf8_lossy(&o.stderr).contains("error[E401]"));
}

#[test]
fn context_mapper_finds_nothing_wrong_in_the_english_one() {
    if !common::linters() {
        return;
    }
    let (Some(java), Some(javac), Some(lib)) = (common::java("java"), common::java("javac"), common::cml_lib()) else {
        common::skip("Java or the Context Mapper CLI is not there (SAKAI_JAVA and SAKAI_JAVAC, or JAVA_HOME; SAKAI_CML_LIB, or tools/cml: tools/cml/fetch.sh)");
        return;
    };
    let dir = common::TempDir::new("context-mapper-en");
    let classes = dir.path().join("classes");
    let cp = format!("{}/*", lib.display());
    let limit = Duration::from_secs(180);
    let b = common::run(Command::new(&javac).arg("-cp").arg(&cp).arg("-d").arg(&classes).arg("tools/cml/Validate.java"), limit);
    assert!(b.ok, "javac: {}", b.stderr);
    dir.write("english.cml", &example_cml_of(common::EXAMPLE_EN, "shop.ctx", Lang::En));
    dir.write("english.ja.cml", &example_cml_of(common::EXAMPLE_EN, "shop.ctx", Lang::Ja));
    let run = |f: &str| common::run(Command::new(&java).arg("-cp").arg(format!("{cp}:{}", classes.display())).arg("Validate").arg(f).current_dir(dir.path()), limit);
    for f in ["english.cml", "english.ja.cml"] {
        let r = run(f);
        assert!(r.ok && r.stdout.trim().is_empty(), "{f}: {}{}", r.stdout, r.stderr);
    }
}
