//! The mistakes other lines of a file hang on (ritsu's DESIGN 8.3): each file of `tests/chains/`
//! holds a few, English first and its Japanese version beside it, and what `ringi check` prints of
//! it, in English and in Japanese, is its golden file there (`<name>.en.txt` and `<name>.ja.txt` for
//! `<name>.ringi`). A name declared nowhere (E004), and a stage the route does not have (E308), is
//! said once, where the file first writes it, with the other lines that write it (`names::undeclared`,
//! `route::stage_refs`, `diag::sort`); a name of another kind, or declared in another place (the
//! columns of two rows), is another name; and the file still fails.
//! `RINGI_BLESS=1 cargo test -p ringi --test chains` writes the golden files again; read the diff.

mod common;

use std::path::Path;

fn cli(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The diagnostics of a printed check, each without its place: no two are the same sentence.
fn said_twice(text: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut twice = Vec::new();
    for l in text.lines().filter(|l| ["error[", "warning[", "エラー[", "警告["].iter().any(|h| l.starts_with(h))) {
        // `error[E004]: <file>:<line>:<column>: <sentence>`
        let mut parts = l.splitn(3, ": ");
        let (head, _place, sentence) = (parts.next().unwrap_or(""), parts.next(), parts.next().unwrap_or(""));
        if !seen.insert(format!("{head} {sentence}")) {
            twice.push(l.to_string());
        }
    }
    twice
}

#[test]
fn a_name_declared_nowhere_is_said_once_where_the_file_first_writes_it() {
    let dir = Path::new("tests/chains");
    let mut files: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".ringi")).collect();
    files.sort();
    assert!(files.len() >= 2, "{files:?}");
    let mut wrong = Vec::new();
    for file in &files {
        let stem = file.strip_suffix(".ringi").unwrap();
        let path = format!("tests/chains/{file}");
        for lang in ["en", "ja"] {
            let (code, out, err) = cli(&["check", &path, "--lang", lang]);
            let text = format!("{out}{err}");
            if code != 1 {
                wrong.push(format!("{file} ({lang}): exit {code}, not 1:\n{text}"));
            }
            for l in said_twice(&text) {
                wrong.push(format!("{file} ({lang}): said twice: {l}"));
            }
            if let Err(e) = ritsu_testkit::golden::check(&dir.join(format!("{stem}.{lang}.txt")), &text) {
                wrong.push(e);
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}
