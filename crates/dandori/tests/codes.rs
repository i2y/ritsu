//! The ledger of the diagnostic codes (src/codes.rs, ritsu's DESIGN 4.3): every code the checker
//! can give is in it once, in order, and on the site's table; every entry's example, checked with
//! what is beside it (and built for every platform, for the two codes a build finds), gives its
//! code; and `dandori explain` prints it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The codes the checker can give: every `"E…"` and `"W…"` of three digits in src/ but the ledger.
fn given() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root().join("src")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") || p.ends_with("src/codes.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            let b = text.as_bytes();
            for i in 0..b.len().saturating_sub(5) {
                if b[i] == b'"' && matches!(b[i + 1], b'E' | b'W') && b[i + 2..i + 5].iter().all(u8::is_ascii_digit) && b[i + 5] == b'"' {
                    out.insert(text[i + 1..i + 5].to_string());
                }
            }
        }
    }
    out
}

#[test]
fn every_code_the_checker_gives_is_in_the_ledger_once_and_in_order() {
    let ledger = dandori::codes::ledger();
    let codes: Vec<&str> = ledger.entries.iter().map(|e| e.code).collect();
    assert!(ritsu_base::ledger::duplicates(&ledger).is_empty(), "{codes:?}");
    let mut sorted = codes.clone();
    sorted.sort_by_key(|c| (c.starts_with('W'), c.to_string()));
    assert_eq!(codes, sorted, "the ledger goes by code, the errors first");
    assert_eq!(codes.iter().map(|c| c.to_string()).collect::<BTreeSet<_>>(), given(), "the ledger has every code the checker gives, and no other");
    for e in &ledger.entries {
        for r in e.related {
            assert!(codes.contains(r), "{} names {r}, which is not in the ledger", e.code);
        }
    }
}

/// The codes the example of an entry gives: the check's, and when it passes, every build's. With
/// `joined`, the rules are read through rulec; without (E018), as the dandori binary reads them.
fn codes_of(dir: &Path, joined: bool) -> Vec<String> {
    let rules: Rc<dyn ritsu_ports::Rules> = if joined { Rc::new(rulec::ports::Engine::new()) } else { Rc::new(dandori::sources::NoRules) };
    dandori::sources::with_rules(rules, || {
        let (_, c) = dandori::check::check_file(&dir.join("example.flow")).unwrap();
        let mut codes: Vec<String> = c.diags.iter().map(|d| d.code.to_string()).collect();
        if let Some(m) = &c.model {
            for t in dandori::commands::TARGETS {
                if let Some(Err(ds)) = dandori::commands::build(m, t) {
                    codes.extend(ds.iter().map(|d| d.code.to_string()));
                }
            }
        }
        codes
    })
}

#[test]
fn every_example_gives_its_code() {
    let ledger = dandori::codes::ledger();
    let scratch = ritsu_testkit::TempDir::new("codes");
    let failures = ritsu_base::ledger::check_every(&ledger, scratch.path(), |e, dir| codes_of(dir, e.code != "E018"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn explain(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = std::iter::once("explain").chain(args.iter().copied()).map(String::from).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = dandori::cli::run(&args, Rc::new(dandori::sources::NoRules), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn explain_prints_the_ledger() {
    let ledger = dandori::codes::ledger();
    let (code, out, _) = explain(&["E014"]);
    assert_eq!((code, out.as_str()), (0, ledger.render_text(ledger.find("E014").unwrap(), ritsu_base::text::Lang::En).as_str()));
    // a code in either case, in Japanese
    let (code, out, _) = explain(&["w104", "--lang", "ja"]);
    assert!(code == 0 && out.starts_with("W104 (警告) — 範囲の分からない値を渡しています\n"), "{out}");
    // every code, in Markdown and as JSON
    let (code, md, _) = explain(&["--all", "--format", "markdown"]);
    assert_eq!((code, md), (0, ledger.render_markdown(ritsu_base::text::Lang::En)));
    let (code, json, _) = explain(&["--all", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!((code, v.as_array().map(|a| a.len())), (0, Some(ledger.entries.len())));
    assert_eq!(v[0]["code"], "E001");
    // what there is not
    let (code, _, err) = explain(&["E999"]);
    assert_eq!((code, err.as_str()), (2, "error: there is no diagnostic code `E999`; `dandori explain --all` lists them\n"));
    assert_eq!(explain(&[]).0, 2);
    assert_eq!(explain(&["--all", "E001"]).0, 2);
}
