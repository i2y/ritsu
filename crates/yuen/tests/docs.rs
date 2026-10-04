//! The READMEs, `docs/`, the examples' READMEs and the agent skill show what yuen does (PLAN D.3,
//! D.4):
//!
//! - `docs/codes.md` and `docs/codes.ja.md` are what `yuen explain --all --format markdown`
//!   prints, in English and in Japanese.
//! - Every line of a ```` ```req ```` block is a line of a `.req` under `examples/` or `tests/`
//!   (a line cut short with `…` stands for a real line with its pieces in that order).
//! - Every `$ ritsu yuen …` (or `$ yuen …`) shown with what it prints prints that, run from the
//!   crate's directory with every language joined, as `ritsu yuen` runs it; one shown without
//!   output is a command yuen has. Every diagnostic shown on its own is in what `check` prints for
//!   its file.
//! - Every relative link leads to a file that is there.
//! - The number of codes the READMEs give is the ledger's, and the reference's table of keywords
//!   is `kw.rs`'s.

mod common;

use ritsu_base::text::Lang;
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
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd {
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

/// The pages: the READMEs, `docs/`, the examples' READMEs and the skill, without the codes
/// pages, which are the output of `explain` and are held to it whole.
fn pages() -> Vec<PathBuf> {
    let mut out = vec![root().join("README.md"), root().join("README.ja.md"), root().join("skills/README.md")];
    out.extend(files(&root().join("docs"), "md"));
    out.extend(files(&root().join("../../skills/yuen"), "md"));
    out.extend(files(&root().join("examples"), "md"));
    out.retain(|p| p.file_name().is_none_or(|n| n != "codes.md" && n != "codes.ja.md"));
    out
}

/// The fenced blocks of a page: the word after the opening fence, and the lines.
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

fn shown(p: &Path) -> String {
    p.strip_prefix(root()).unwrap().display().to_string()
}

#[test]
fn the_codes_pages_are_what_explain_prints() {
    for (page, lang) in [("docs/codes.md", Lang::En), ("docs/codes.ja.md", Lang::Ja)] {
        let want = yuen::codes::ledger().render_markdown(lang);
        let have = fs::read_to_string(root().join(page)).unwrap();
        assert!(have == want, "{page} is not what `yuen explain --all --format markdown` prints; write it again with that command");
    }
}

#[test]
fn the_req_on_the_pages_is_from_the_files() {
    let mut real = BTreeSet::new();
    for dir in ["examples", "tests"] {
        for f in files(&root().join(dir), "req") {
            real.extend(fs::read_to_string(&f).unwrap().lines().map(|l| l.trim().to_string()));
        }
    }
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
    let mut seen = 0;
    for f in pages() {
        for (info, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
            if info != "req" {
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
    assert!(seen >= 10, "{seen} blocks of .req on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not a line of any .req under examples/ or tests/:\n{}", wrong.join("\n"));
}

/// A command line split as a shell would: spaces, and single quotes around a word with spaces.
fn split(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for ch in cmd.chars() {
        match ch {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

#[test]
fn the_commands_on_the_pages_print_what_they_show() {
    let commands: Vec<&str> = yuen::cli::commands().iter().map(|c| c.name).collect();
    let mut wrong = Vec::new();
    let mut ran = 0;
    let mut listed = 0;
    for f in pages() {
        for (info, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
            if info != "console" {
                continue;
            }
            let mut runs: Vec<(String, Vec<String>)> = Vec::new();
            for l in &lines {
                match l.strip_prefix("$ ") {
                    Some(cmd) => runs.push((cmd.to_string(), Vec::new())),
                    None => {
                        if let Some((_, out)) = runs.last_mut() {
                            out.push(l.clone());
                        }
                    }
                }
            }
            for (cmd, out) in runs {
                let Some(rest) = cmd.strip_prefix("ritsu yuen ").or_else(|| cmd.strip_prefix("yuen ")) else { continue };
                if out.is_empty() {
                    let first = rest.split_whitespace().next().unwrap_or("");
                    if !commands.contains(&first) && !first.starts_with("--") {
                        wrong.push(format!("{}: `yuen {first}` is not a command", shown(&f)));
                    }
                    listed += 1;
                    continue;
                }
                let args = split(rest);
                let words: Vec<&str> = args.iter().map(|a| a.as_str()).collect();
                let got = common::run(&words).stdout;
                let want = out.join("\n") + "\n";
                if got != want {
                    wrong.push(format!("{}: $ {cmd}\n--- the page shows\n{want}--- it prints\n{got}", shown(&f)));
                }
                ran += 1;
            }
        }
    }
    assert!(ran >= 8 && listed >= 8, "{ran} commands run and {listed} listed: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_diagnostics_on_the_pages_are_what_check_prints() {
    let heads = ["error[", "warning[", "エラー[", "警告["];
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages() {
        for (_, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
            // a diagnostic under a `$ ritsu yuen check` is held to it whole, as a command's output
            seen += lines.iter().filter(|l| heads.iter().any(|h| l.starts_with(h))).count();
            let Some(first) = lines.first() else { continue };
            if !heads.iter().any(|h| first.starts_with(h)) {
                continue;
            }
            // `error[E303]: examples/x/x.req:40:3: …`: the file, checked from its directory as the
            // root, in the block's language
            let after = &first[first.find("]: ").unwrap() + 3..];
            let file = &after[..after.find(".req:").unwrap() + 4];
            let dir = Path::new(file).parent().unwrap().to_string_lossy().to_string();
            let lang = if first.starts_with("エラー") || first.starts_with("警告") { "ja" } else { "en" };
            let got = common::run(&["check", file, "--root", &dir, "--lang", lang]).stdout;
            if !got.contains(&lines.join("\n")) {
                wrong.push(format!("{}: {first}\n--- check prints\n{got}", shown(&f)));
            }
        }
    }
    assert!(seen >= 2, "{seen} diagnostics on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not what `yuen check` prints for its file:\n{}", wrong.join("\n"));
}

#[test]
fn every_relative_link_leads_to_a_file() {
    let mut wrong = Vec::new();
    let mut seen = 0;
    let mut all = pages();
    all.extend([root().join("docs/codes.md"), root().join("docs/codes.ja.md"), root().join("THIRD_PARTY_NOTICES.md")]);
    for f in all {
        let text = fs::read_to_string(&f).unwrap();
        let mut rest = text.as_str();
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let end = after.find(')').unwrap_or(after.len());
            let target = &after[..end];
            rest = &after[end.min(after.len())..];
            if target.starts_with("http:") || target.starts_with("https:") || target.starts_with('#') || target.starts_with("mailto:") {
                continue;
            }
            let path = target.split('#').next().unwrap();
            seen += 1;
            if !f.parent().unwrap().join(path).exists() {
                wrong.push(format!("{}: {target}", shown(&f)));
            }
        }
    }
    assert!(seen >= 10, "{seen} relative links");
    assert!(wrong.is_empty(), "links to nothing:\n{}", wrong.join("\n"));
}

#[test]
fn the_readmes_count_the_codes_of_the_ledger() {
    let n = yuen::codes::ledger().entries.len();
    let en = fs::read_to_string(root().join("README.md")).unwrap();
    let ja = fs::read_to_string(root().join("README.ja.md")).unwrap();
    assert!(en.contains(&format!("{n} diagnostic codes")), "README.md does not say {n} diagnostic codes");
    assert!(ja.contains(&format!("診断のコードは {n} 個")), "README.ja.md does not say 診断のコードは {n} 個");
}

#[test]
fn the_reference_lists_every_keyword_of_kw_rs() {
    let text = fs::read_to_string(root().join("docs/reference.md")).unwrap();
    let at = text.find("### Keywords").expect("the reference has a table of keywords");
    let rows: Vec<(String, Vec<String>)> = text[at..]
        .lines()
        .skip_while(|l| !l.starts_with("| Where"))
        .skip(2)
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            let words = cells[1].split('`').enumerate().filter(|(i, _)| i % 2 == 1).map(|(_, w)| w.to_string()).collect();
            (cells[0].to_string(), words)
        })
        .collect();
    let want: Vec<(String, Vec<String>)> = yuen::kw::TABLE.iter().map(|(k, ws)| (k.to_string(), ws.iter().map(|w| w.to_string()).collect())).collect();
    assert_eq!(rows, want, "the table of keywords in docs/reference.md is not kw.rs's TABLE");
}
