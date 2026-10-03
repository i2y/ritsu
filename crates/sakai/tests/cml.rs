//! `sakai export cml` (DESIGN 8; PLAN C.14): the example's CML is its golden file, in English
//! and in Japanese, and Context Mapper 6.12.0's validator, with every check, finds nothing in
//! either. The CLI's `cm validate` reads the syntax only (DESIGN 0.4), so the test runs
//! `tools/cml/Validate.java` on the CLI's jars; a CML that breaks one of its semantic rules shows
//! that the validator is at work.

mod common;

use sakai::i18n::Lang;
use std::process::Command;
use std::time::Duration;

fn example_cml(lang: Lang) -> String {
    let ex = std::fs::canonicalize(common::EXAMPLE).unwrap();
    let o = sakai::check::check_map(&ex, "通販.ctx").unwrap();
    assert!(!o.has_errors());
    sakai::cml::render(o.checked.as_ref().unwrap(), "通販.ctx", lang)
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
    let dir = common::TempDir::new();
    let out = dir.path().join("shop.cml");
    let o = common::sakai(&["export", "cml", "examples/通販/通販.ctx", "--out", out.to_str().unwrap(), "--root", "examples/通販"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), example_cml(Lang::En));
    let o = common::sakai(&["export", "cml", "examples/通販/通販.ctx", "--lang", "ja", "--root", "examples/通販"]);
    assert_eq!(String::from_utf8_lossy(&o.stdout), example_cml(Lang::Ja));
    // `export` names the form it writes; a map with errors writes nothing.
    assert_eq!(common::sakai(&["export", "examples/通販/通販.ctx"]).status.code(), Some(2));
    let bad = common::mutant("E401_注文の状態に値が増えた");
    let o = common::sakai_in(bad.path(), &["export", "cml", "基本.ctx"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty() && String::from_utf8_lossy(&o.stderr).contains("error[E401]"));
}

#[test]
fn context_mapper_finds_nothing_wrong_in_it() {
    let (Some(java), Some(javac), Some(lib)) = (common::java("java"), common::java("javac"), common::cml_lib()) else {
        common::skip("Java or the Context Mapper CLI is not there (SAKAI_JAVA and SAKAI_JAVAC, or JAVA_HOME; SAKAI_CML_LIB, or tools/cml: tools/cml/fetch.sh)");
        return;
    };
    let dir = common::TempDir::new();
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
