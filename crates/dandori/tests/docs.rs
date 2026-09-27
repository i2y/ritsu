//! The README and the site (website/docs, website/docs-ja) show what the tool does: every
//! diagnostic on them is word for word in a golden file of tests/fixtures, which the fixtures
//! test holds to the checker; every line of `.flow` on them is a line of an example or a test
//! flow; the codes pages list every code the checker has, and no other; and the highlighter of
//! the site knows the keywords of the language as src/syntax.rs has them.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The files under `dir` with the extension `ext`, in a stable order.
fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// The README and the pages of both languages, each with the name the messages give it.
fn pages() -> Vec<(String, Vec<PathBuf>)> {
    vec![
        ("README.md".into(), vec![root().join("README.md")]),
        ("website/docs".into(), files(&root().join("website/docs"), "md")),
        ("website/docs-ja".into(), files(&root().join("website/docs-ja"), "md")),
    ]
}

/// The fenced blocks of a Markdown page: the word after the opening fence, and the lines.
fn fenced(text: &str) -> Vec<(String, Vec<String>)> {
    let mut blocks = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;
    for line in text.lines() {
        match (line.trim_start().strip_prefix("```"), open.take()) {
            (Some(_), Some(block)) => blocks.push(block),
            (Some(info), None) => open = Some((info.trim().to_string(), Vec::new())),
            (None, Some(mut block)) => {
                block.1.push(line.to_string());
                open = Some(block);
            }
            (None, None) => {}
        }
    }
    blocks
}

fn shown(f: &Path) -> String {
    f.strip_prefix(root()).unwrap().display().to_string()
}

#[test]
fn the_diagnostics_on_the_pages_are_what_the_checker_prints() {
    let goldens: Vec<String> = files(&root().join("tests/fixtures"), "txt").iter().map(|p| fs::read_to_string(p).unwrap()).collect();
    let heads = ["error[", "warning[", "エラー[", "警告["];
    let mut wrong = Vec::new();
    for (place, files) in pages() {
        let mut seen = 0;
        for f in files {
            for (_, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
                if !lines.first().is_some_and(|l| heads.iter().any(|h| l.starts_with(h))) {
                    continue;
                }
                seen += 1;
                if !goldens.iter().any(|g| g.contains(&lines.join("\n"))) {
                    wrong.push(format!("{}: {}", shown(&f), lines[0]));
                }
            }
        }
        assert!(seen > 0, "no diagnostic in {place}: were the fences changed?");
    }
    assert!(wrong.is_empty(), "not what any golden file in tests/fixtures holds:\n{}", wrong.join("\n"));
}

#[test]
fn the_flow_on_the_pages_is_from_the_examples() {
    let mut real = BTreeSet::new();
    for dir in ["examples", "tests"] {
        for f in files(&root().join(dir), "flow") {
            real.extend(fs::read_to_string(&f).unwrap().lines().map(|l| l.trim().to_string()));
        }
    }
    // A line cut short with `…` stands for a real line that has its pieces in that order.
    let found = |line: &str| {
        let pieces: Vec<&str> = line.split('…').map(str::trim).filter(|p| !p.is_empty()).collect();
        real.iter().any(|r| {
            let mut at = 0;
            pieces.iter().all(|p| match r[at..].find(p) {
                Some(i) => {
                    at += i + p.len();
                    true
                }
                None => false,
            })
        })
    };
    let mut wrong = Vec::new();
    for (place, files) in pages() {
        let mut seen = 0;
        for f in files {
            for (info, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
                if info != "flow" {
                    continue;
                }
                seen += 1;
                for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "…") {
                    if !found(l) {
                        wrong.push(format!("{}: {l}", shown(&f)));
                    }
                }
            }
        }
        assert!(seen > 0, "no flow in {place}: were the fences changed?");
    }
    assert!(wrong.is_empty(), "not a line of any .flow under examples/ or tests/:\n{}", wrong.join("\n"));
}

/// The codes the checker can give: every `"E…"` and `"W…"` of three digits in src/.
fn codes() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for f in files(&root().join("src"), "rs") {
        let text = fs::read_to_string(&f).unwrap();
        let b = text.as_bytes();
        for i in 0..b.len().saturating_sub(5) {
            if b[i] == b'"' && matches!(b[i + 1], b'E' | b'W') && b[i + 2..i + 5].iter().all(u8::is_ascii_digit) && b[i + 5] == b'"' {
                out.insert(text[i + 1..i + 5].to_string());
            }
        }
    }
    out
}

#[test]
fn the_codes_pages_list_every_code_and_no_other() {
    let codes = codes();
    for page in ["website/docs/reference/codes.md", "website/docs-ja/reference/codes.md"] {
        let listed: BTreeSet<String> = fs::read_to_string(root().join(page))
            .unwrap()
            .lines()
            .filter_map(|l| l.strip_prefix("| ")?.split(' ').next())
            .filter(|c| c.len() == 4 && (c.starts_with('E') || c.starts_with('W')))
            .map(String::from)
            .collect();
        assert_eq!(listed, codes, "{page} must list every code the checker has, and no other");
    }
    let n = codes.len();
    for (page, says) in [
        ("README.md", format!("all {n} codes")),
        ("website/docs/checks.md", format!("lists all {n}")),
        ("website/docs-ja/checks.md", format!("{n} 種類すべて")),
    ] {
        let text = fs::read_to_string(root().join(page)).unwrap().split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains(&says), "{page} must say how many codes there are: {says:?}");
    }
}

/// The quoted words between `start` and the first `end` after it.
fn quoted(text: &str, start: &str, end: &str) -> Vec<String> {
    let from = text.find(start).unwrap_or_else(|| panic!("no {start:?}")) + start.len();
    let body = &text[from..from + text[from..].find(end).unwrap()];
    body.split('"').skip(1).step_by(2).map(String::from).collect()
}

#[test]
fn the_site_highlights_the_keywords_of_the_language() {
    let ours = quoted(&fs::read_to_string(root().join("src/syntax.rs")).unwrap(), "const KEYWORDS: &[&str] = &[", "];");
    let site = quoted(&fs::read_to_string(root().join("website/tools/flowlexer.py")).unwrap(), "KEYWORDS = (", ")");
    assert!(ours.len() > 50, "src/syntax.rs's KEYWORDS were not found");
    assert_eq!(site, ours, "website/tools/flowlexer.py's KEYWORDS must be src/syntax.rs's, word for word");
}
