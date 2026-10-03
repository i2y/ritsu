//! The READMEs, `docs/` and the agent skill show what the tool does (PLAN D.3, D.4):
//!
//! - `docs/codes.md` and `docs/codes.ja.md` are what `koyomi explain --all --format markdown`
//!   prints, in English and in Japanese.
//! - Every line of `.cal` on the pages is a line of a `.cal` under `examples/` or `tests/`.
//! - Every `$ koyomi …` that shows what it prints prints that, and every diagnostic shown is in
//!   what `koyomi check` prints for its file.
//! - Every block of TypeScript, Python, Go, Rust and SQL is cut from what `koyomi gen` writes for
//!   an example, and every line of JSON Lines is a line of an example's vectors.
//! - The number of lines the READMEs say each target is compared on is the number of lines of
//!   the vectors the tests run through it, and the reference lists every keyword of `kw.rs`.

mod common;

use koyomi::check::{Checked, check};
use ritsu_base::text::Lang;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// The pages: the READMEs, `docs/` and the skill, without the codes pages, which are the
/// output of `explain` and are held to it whole.
fn pages() -> Vec<PathBuf> {
    let mut out = vec![root().join("README.md"), root().join("README.ja.md"), root().join("skills/README.md")];
    out.extend(files(&root().join("docs"), "md"));
    out.extend(files(&root().join("skills/koyomi"), "md"));
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
        let want = koyomi::codes::ledger().render_markdown(lang);
        let have = fs::read_to_string(root().join(page)).unwrap();
        assert!(have == want, "{page} is not what `koyomi explain --all --format markdown` prints; write it again with that command");
    }
}

#[test]
fn the_cal_on_the_pages_is_from_the_files() {
    let mut real = BTreeSet::new();
    for dir in ["examples", "tests"] {
        for f in files(&root().join(dir), "cal") {
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
    let mut seen = 0;
    for f in pages() {
        for (info, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
            if info != "cal" {
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
    assert!(seen >= 10, "{seen} blocks of .cal on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not a line of any .cal under examples/ or tests/:\n{}", wrong.join("\n"));
}

fn koyomi(args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).current_dir(root()).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap();
    String::from_utf8_lossy(&o.stdout).to_string()
}

#[test]
fn the_commands_on_the_pages_print_what_they_show() {
    let commands: Vec<&str> = koyomi::cli::commands().iter().map(|c| c.name).collect();
    let mut wrong = Vec::new();
    let mut ran = 0;
    let mut listed = 0;
    for f in pages() {
        for (info, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
            if info != "console" {
                continue;
            }
            // Each `$ koyomi …`, and the lines under it up to the next `$`: what it prints.
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
                let Some(rest) = cmd.strip_prefix("koyomi ") else { continue };
                if out.is_empty() {
                    // A command shown without its output, in a list of commands: it is one.
                    let first = rest.split_whitespace().next().unwrap_or("");
                    if !commands.contains(&first) && !first.starts_with("--") {
                        wrong.push(format!("{}: `koyomi {first}` is not a command", shown(&f)));
                    }
                    listed += 1;
                    continue;
                }
                let args: Vec<&str> = rest.split_whitespace().collect();
                let got = koyomi(&args);
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
            let Some(first) = lines.first() else { continue };
            if !heads.iter().any(|h| first.starts_with(h)) {
                continue;
            }
            seen += 1;
            // `error[E301]: examples/x.cal:16:3: …`: the file, checked in the block's language.
            let after = &first[first.find("]: ").unwrap() + 3..];
            let file = &after[..after.find(".cal:").unwrap() + 4];
            let lang = if first.starts_with("エラー") || first.starts_with("警告") { "ja" } else { "en" };
            let got = koyomi(&["check", file, "--lang", lang]);
            if !got.contains(&lines.join("\n")) {
                wrong.push(format!("{}: {first}", shown(&f)));
            }
        }
    }
    // The diagnostics under a `$ koyomi check` are held to it whole, as a command's output.
    assert!(seen >= 1, "{seen} diagnostics on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not what `koyomi check` prints for its file:\n{}", wrong.join("\n"));
}

/// Every example that passes check, generated in every target and both languages, and the
/// lines of the vectors of the smaller ones.
fn generated() -> (Vec<(String, String)>, BTreeSet<String>) {
    let mut code = Vec::new();
    let mut vectors = BTreeSet::new();
    for f in files(&root().join("examples"), "cal") {
        let p = f.to_string_lossy().to_string();
        let mut o = check(&p).unwrap();
        if o.has_errors() {
            continue;
        }
        let checked = o.checked.take().unwrap();
        for lang in [Lang::En, Lang::Ja] {
            let u = koyomi::codegen::unit_of(&checked, lang);
            for t in koyomi::naming::TARGETS {
                for (_, body) in koyomi::codegen::files(&u, t) {
                    code.push((t.key().to_string(), body));
                }
            }
        }
        if let Checked::Dates(m, _) = &checked
            && m.combinations() < 100_000
        {
            let w = koyomi::vectors::DatesLines::new(m);
            vectors.extend(koyomi::vectors::DatesRows::new(m).map(|r| w.line(&r)));
        }
    }
    (code, vectors)
}

#[test]
fn the_code_on_the_pages_is_what_gen_writes() {
    let (code, vectors) = generated();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages() {
        for (info, lines) in fenced(&fs::read_to_string(&f).unwrap()) {
            let key = match info.as_str() {
                "ts" => "typescript",
                "python" | "go" | "rust" | "sql" => info.as_str(),
                "jsonl" => {
                    for l in &lines {
                        seen += 1;
                        if !vectors.contains(l) {
                            wrong.push(format!("{}: {l} is not a line of an example's vectors", shown(&f)));
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            seen += 1;
            // A line `…` cuts the block; the pieces come in this order in one generated file.
            let mut pieces: Vec<String> = vec![String::new()];
            for l in &lines {
                if l.trim() == "…" {
                    pieces.push(String::new());
                } else {
                    let p = pieces.last_mut().unwrap();
                    p.push_str(l);
                    p.push('\n');
                }
            }
            let ok = code.iter().any(|(k, body)| {
                k == key && {
                    let mut at = 0;
                    pieces.iter().filter(|p| !p.is_empty()).all(|p| match body[at..].find(p.as_str()) {
                        Some(i) => {
                            at += i + p.len();
                            true
                        }
                        None => false,
                    })
                }
            });
            if !ok {
                wrong.push(format!("{}: a ```{info} block that gen does not write:\n{}", shown(&f), lines.join("\n")));
            }
        }
    }
    assert!(seen >= 12, "{seen} blocks of code on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// `5341318` as the pages write it, `5,341,318`.
fn with_commas(n: u64) -> String {
    ritsu_base::text::count(n)
}

#[test]
fn the_readmes_count_the_lines_the_targets_are_held_to() {
    // The vectors of every file tests/targets.rs runs through each target: every input of the
    // range and the inputs just outside it, counted without making them.
    let mut lines: u64 = 0;
    for p in common::TARGET_FILES {
        let o = check(p).unwrap();
        lines += match o.checked.as_ref().unwrap() {
            Checked::Dates(m, _) => m.combinations() as u64 + koyomi::vectors::outside(m).len() as u64,
            Checked::Calendar(c) => {
                let (a, b, edges) = koyomi::vectors::calendar_span(c);
                (b.0 - a.0) as u64 + 1 + if edges { 2 } else { 0 }
            }
        };
    }
    let n = with_commas(lines);
    for page in ["README.md", "README.ja.md"] {
        let text = fs::read_to_string(root().join(page)).unwrap();
        assert!(text.matches(&format!("| {n} |")).count() == 5, "{page} must say each of the five targets is compared on {n} lines");
    }
}

#[test]
fn the_reference_lists_every_keyword() {
    let text = fs::read_to_string(root().join("docs/reference.md")).unwrap();
    let table = &text[text.find("### Keywords").unwrap()..text.find("## Calendars").unwrap()];
    for (_, words) in koyomi::kw::TABLE {
        for w in *words {
            assert!(table.contains(&format!("`{w}`")), "docs/reference.md does not list the keyword {w}");
        }
    }
}
