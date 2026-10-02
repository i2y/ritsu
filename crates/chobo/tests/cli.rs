//! The command: `--help` from the one table, exit 2 for a flag it does not know, `explain`
//! for every code with an example that shows it, and each command run end to end.

mod common;
use chobo::{check, codes, diffbase};
use common::*;
use serde_json::Value;

fn run(args: &[&str]) -> (u8, String, String) {
    let out = chobo().args(args).output().unwrap();
    (out.status.code().unwrap() as u8, String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

#[test]
fn every_code_is_explained_in_both_languages_with_an_example_that_shows_it() {
    for e in codes::ledger() {
        for (lang, when) in [("en", "When"), ("ja", "いつ出るか")] {
            let (code, out, _) = run(&["explain", e.code, "--lang", lang]);
            assert_eq!(code, 0, "{}", e.code);
            assert!(out.contains(&format!("[{}]", e.code)) && out.contains(when), "{}:\n{out}", e.code);
            assert!(out.contains(e.example.lines().last().unwrap()), "{}: the example is not shown", e.code);
        }
        let c = check::check_source(e.example);
        let mut found: Vec<&str> = c.diags.iter().map(|d| d.code).collect();
        let mut extra = Vec::new();
        if let Some(before) = e.before {
            let book = c.book.as_ref().unwrap_or_else(|| panic!("{}: the example does not load", e.code));
            let (more, note) = diffbase::against(book, Some(before), "HEAD");
            assert!(note.is_none());
            extra = more;
        }
        found.extend(extra.iter().map(|d| d.code));
        if let Some(t) = e.target {
            let book = c.book.as_ref().unwrap_or_else(|| panic!("{}: the example does not load", e.code));
            assert!(!chobo::diag::has_errors(&c.diags), "{}: the example has errors before it is built", e.code);
            found.extend(chobo::target::check(book, chobo::target::Target::parse(t).unwrap()).iter().map(|d| d.code));
        }
        assert!(found.contains(&e.code), "{}: its example gives {found:?}", e.code);
    }
}

#[test]
fn explain_all_as_markdown_has_every_code() {
    for lang in ["en", "ja"] {
        let (code, out, _) = run(&["explain", "--all", "--format", "markdown", "--lang", lang]);
        assert_eq!(code, 0);
        for e in codes::ledger() {
            assert!(out.contains(&format!("\n## {}\n", e.code)), "{lang}: {}", e.code);
        }
    }
    let (code, _, err) = run(&["explain", "E999"]);
    assert_eq!(code, 2);
    assert!(err.contains("E999"));
}

#[test]
fn help_comes_from_the_table() {
    let (code, out, _) = run(&["--help"]);
    assert_eq!(code, 0);
    for cmd in ["check", "run", "scenarios", "build", "doc", "api", "explain"] {
        assert!(out.contains(&format!("  chobo {cmd} ")), "{cmd}");
        let (code, page, _) = run(&[cmd, "--help"]);
        assert_eq!(code, 0, "{cmd}");
        assert!(page.starts_with(&format!("chobo {cmd} ")), "{cmd}");
        let (code, page_ja, _) = run(&[cmd, "--help", "--lang", "ja"]);
        assert_eq!(code, 0);
        assert!(page_ja.contains("終了コード"), "{cmd}");
    }
    let (code, _, err) = run(&[]);
    assert_eq!(code, 2);
    assert!(err.contains("Usage:"));
    let (code, out, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), format!("chobo {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn a_flag_it_does_not_take_stops_it() {
    let book = root().join("tests/books/在庫.book");
    let b = book.to_str().unwrap();
    for args in [
        vec!["check", b, "--bogus"],
        vec!["check", b, "--format", "yaml"],
        vec!["check", b, "--format"],
        vec!["check", b, "--format", "json", "--format", "json"],
        vec!["check", b, "--scenario", "x.json"],
        vec!["run", b, "--out", "x"],
        vec!["frobnicate", b],
        vec!["check", b, "--lang"],
    ] {
        let (code, _, err) = run(&args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(!err.is_empty(), "{args:?}");
    }
}

#[test]
fn each_command_runs() {
    let tmp = TempDir::new("cli");
    let book = root().join("tests/books/在庫.book");
    let b = book.to_str().unwrap();

    let (code, out, _) = run(&["check", b]);
    assert_eq!(code, 0);
    assert!(out.contains(": ok"));
    let (code, out, _) = run(&["check", b, "--format", "json"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["v"], 1);
    assert!(v["files"][0]["report"].as_array().unwrap().iter().any(|r| r["kind"] == "引当" && r["op"] == "hold"));

    let dir = tmp.path().join("sc");
    let (code, _, err) = run(&["scenarios", b, "--out", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    let first = dir.join("001.json");
    assert!(first.exists());
    let (code, out, _) = run(&["run", b, "--scenario", first.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.contains("1  入荷.do(") && out.contains("accounts:"), "{out}");
    let (code, out, _) = run(&["run", b, "--scenario", first.to_str().unwrap(), "--format", "json", "--lang", "ja"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v["steps"].is_array() && v["accounts"].is_array());

    // the list `scenarios` prints, run as it is
    let (code, list, _) = run(&["scenarios", b]);
    assert_eq!(code, 0);
    let all = tmp.path().join("all.json");
    std::fs::write(&all, list).unwrap();
    let (code, out, _) = run(&["run", b, "--scenario", all.to_str().unwrap(), "--format", "json"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v.as_array().unwrap().len() > 10);

    let (code, out, _) = run(&["api", b]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["v"], 1);
    assert_eq!(v["book"], "在庫");

    // a book with errors: check says so with 1, and the others do nothing with it
    let bad = root().join("tests/fixtures/境界.book");
    let bad = bad.to_str().unwrap();
    assert_eq!(run(&["check", bad]).0, 1);
    for cmd in [vec!["scenarios", bad], vec!["api", bad], vec!["run", bad, "--scenario", first.to_str().unwrap()]] {
        let (code, out, err) = run(&cmd);
        assert_eq!(code, 1, "{cmd:?}");
        assert!(out.is_empty() && err.contains("E020"), "{cmd:?}");
    }
    // a scenario that does not fit the book is a mistake in the call
    let wrong = tmp.path().join("wrong.json");
    std::fs::write(&wrong, r#"{"name": "", "steps": [{"op": "do", "kind": "無い", "args": {}}]}"#).unwrap();
    assert_eq!(run(&["run", b, "--scenario", wrong.to_str().unwrap()]).0, 2);
    assert_eq!(run(&["check", "no-such-file.book"]).0, 2);
}

#[test]
fn the_language_comes_from_the_flag_then_the_environment() {
    let book = root().join("tests/books/在庫.book");
    let b = book.to_str().unwrap();
    let out = chobo().args(["check", b]).env("CHOBO_LANG", "ja").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("断られうる理由"));
    let out = chobo().args(["check", b, "--lang", "en"]).env("CHOBO_LANG", "ja").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("may be refused"));
}

#[test]
fn build_writes_each_target() {
    let tmp = TempDir::new("cli-build");
    let book = root().join("tests/books/在庫.book");
    let b = book.to_str().unwrap();
    let wants: &[(&str, &[&str])] = &[
        ("postgres", &["在庫.sql"]),
        ("postgres-typescript", &["在庫.ts"]),
        ("postgres-python", &["在庫.py"]),
        ("postgres-go", &["book/book.go", "book/runtime.go"]),
        ("tigerbeetle-typescript", &["在庫.ts"]),
        ("tigerbeetle-python", &["在庫.py"]),
        ("tigerbeetle-go", &["book/book.go", "book/runtime.go"]),
    ];
    for (target, files) in wants {
        let out = tmp.path().join(target);
        let (code, _, err) = run(&["build", b, "--target", target, "--out", out.to_str().unwrap()]);
        assert_eq!(code, 0, "{target}: {err}");
        for f in *files {
            assert!(out.join(f).is_file(), "{target}: no {f}");
            assert!(err.contains(f), "{target}: {err}");
        }
    }
    assert_eq!(run(&["build", b]).0, 2);
    assert_eq!(run(&["build", b, "--target", "sqlite"]).0, 2);
    // a book with errors builds nothing
    let bad = root().join("tests/fixtures/境界.book");
    let (code, _, err) = run(&["build", bad.to_str().unwrap(), "--target", "postgres", "--out", tmp.path().join("bad").to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(err.contains("E020") && !tmp.path().join("bad").exists());
}

#[test]
fn run_shows_what_a_client_sends() {
    let tmp = TempDir::new("cli-show");
    let book = root().join("tests/books/与信.book");
    let b = book.to_str().unwrap();
    let (_, checked) = check::check_file(&book).unwrap();
    let model = checked.book.unwrap();
    for (i, s) in chobo::scenarios::generate(&model).iter().enumerate() {
        let file = tmp.path().join(format!("{i}.json"));
        std::fs::write(&file, serde_json::to_string(&chobo::scenario::to_json(&model, s)).unwrap()).unwrap();
        let f = file.to_str().unwrap();
        // what TigerBeetle is sent: for a do or a hold, what stage B's chains golden has
        let (code, out, err) = run(&["run", b, "--scenario", f, "--show", "tigerbeetle", "--format", "json"]);
        assert_eq!(code, 0, "{err}");
        let v: Value = serde_json::from_str(&out).unwrap();
        let chains = chobo::ids::scenario_chains(&model, "", s);
        for (shown, chained) in v["operations"].as_array().unwrap().iter().zip(chains.as_array().unwrap()) {
            if matches!(shown["op"].as_str(), Some("do" | "hold")) {
                assert_eq!(shown["sent"], chained["sent"], "{}", s.name);
            }
        }
        let (code, out, _) = run(&["run", b, "--scenario", f, "--show", "postgres"]);
        assert_eq!(code, 0);
        assert!(out.contains("select * from \"与信\"."), "{out}");
        let (code, out, _) = run(&["run", b, "--scenario", f, "--show", "tigerbeetle", "--lang", "ja"]);
        assert_eq!(code, 0);
        assert!(out.contains("TigerBeetle に送るもの"), "{out}");
    }
}

/// `chobo doc` run as a person runs it, from the root of the repository: the pages beside the
/// examples are what it prints, and `--out` writes the page under the book's name.
#[test]
fn doc_writes_the_pages_beside_the_examples() {
    let in_root = |args: &[&str]| {
        let out = chobo().current_dir(root()).args(args).output().unwrap();
        (out.status.code().unwrap() as u8, String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
    };
    for ex in ["inventory", "marketplace", "points", "refunds"] {
        let (code, out, _) = in_root(&["doc", &format!("examples/{ex}/{ex}.book")]);
        assert_eq!(code, 0, "{ex}");
        assert_eq!(out, std::fs::read_to_string(root().join(format!("examples/{ex}/doc.md"))).unwrap(), "examples/{ex}/doc.md is not what `chobo doc` prints");
        let (code, out, _) = in_root(&["doc", &format!("examples/{ex}/{ex}.ja.book"), "--lang", "ja"]);
        assert_eq!(code, 0, "{ex}");
        assert_eq!(out, std::fs::read_to_string(root().join(format!("examples/{ex}/doc.ja.md"))).unwrap(), "examples/{ex}/doc.ja.md is not what `chobo doc --lang ja` prints");
    }
    let dir = TempDir::new("doc");
    let d = dir.path().to_str().unwrap();
    let (code, _, err) = in_root(&["doc", "examples/refunds/refunds.ja.book", "--lang", "ja", "--format", "html", "--out", d]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("refunds.ja.html を書きました"), "{err}");
    let page = std::fs::read_to_string(dir.path().join("refunds.ja.html")).unwrap();
    assert_eq!(page, std::fs::read_to_string(root().join("tests/doc/refunds.ja.html")).unwrap());
    // a book with errors is not drawn
    let (code, out, err) = in_root(&["doc", "tests/fixtures/境界.book"]);
    assert_eq!((code, out.is_empty()), (1, true), "{err}");
    assert!(err.contains("error[E020]"), "{err}");
}
