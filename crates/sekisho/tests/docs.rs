//! The READMEs, `docs/reference.md` and the agent skill show what sekisho does (DESIGN 12), as
//! sakai's and yuen's do:
//!
//! - Every line of `.gate` on the pages is a line of a `.gate` under `examples/` or `tests/`.
//! - Every line of `cedar` and `cedarschema` is a line of what `gen --target cedar` writes for the
//!   example, in English or in Japanese.
//! - Every `$ sekisho …` that shows what it prints prints that, run as `ritsu sekisho` runs it
//!   (every language joined); `gen` writes into a scratch directory in place of the `--out` the
//!   page shows. One shown without its output is a command. (`$ ritsu …` is ritsu's to run: its
//!   `tests/sekisho.rs` holds the `ritsu check` on the READMEs.)
//! - Every diagnostic shown is what a mutant gives, word for word as its golden file holds it
//!   (`tests/mutants.rs` holds the golden files to the runs).
//! - The numbers of mutants the READMEs say are counted again here, the reference lists every
//!   keyword of `kw.rs`, and every relative link leads to a file.
//!
//! (`docs/codes.md` and `docs/codes.ja.md` are held to `explain --all` by `tests/codes.rs`.)

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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

/// The pages: the READMEs, `docs/` and the skill, without the codes pages, which are the output of
/// `explain` and are held to it whole.
fn pages() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = ["README.md", "README.ja.md", "skills/README.md"].iter().map(PathBuf::from).collect();
    out.extend(files(Path::new("docs"), "md"));
    out.extend(files(Path::new("../../skills/sekisho"), "md"));
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

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The trimmed lines of every file with the extension under the directories.
fn lines_of(dirs: &[&str], ext: &str) -> BTreeSet<String> {
    let mut real = BTreeSet::new();
    for dir in dirs {
        for f in files(Path::new(dir), ext) {
            real.extend(read(&f).lines().map(|l| l.trim().to_string()));
        }
    }
    real
}

/// The lines of the blocks marked `info` on the pages that are not in `real`, and how many blocks
/// there were.
fn not_real(info: &[&str], real: &BTreeSet<String>) -> (usize, Vec<String>) {
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages() {
        for (i, lines) in fenced(&read(&f)) {
            if !info.contains(&i.as_str()) {
                continue;
            }
            seen += 1;
            for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "…") {
                if !real.contains(l) {
                    wrong.push(format!("{}: {l}", f.display()));
                }
            }
        }
    }
    (seen, wrong)
}

#[test]
fn the_gate_on_the_pages_is_from_the_files() {
    let (seen, wrong) = not_real(&["gate"], &lines_of(&["examples", "tests"], "gate"));
    assert!(seen >= 15, "{seen} blocks of .gate on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not a line of any .gate under examples/ or tests/:\n{}", wrong.join("\n"));
}

/// The command run as a function, as `ritsu sekisho` runs it: the exit code, and what it printed on
/// stdout and stderr.
fn joined(args: &[String]) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn the_cedar_on_the_pages_is_what_gen_writes() {
    let t = ritsu_testkit::TempDir::new("docs-cedar");
    for (gate, lang) in [("examples/refunds/refunds.gate", "en"), ("examples/refunds/refunds.ja.gate", "ja")] {
        let out = t.path().join(lang);
        let args: Vec<String> = ["gen", gate, "--target", "cedar", "--root", "examples/refunds", "--out", out.to_str().unwrap(), "--lang", lang].iter().map(|s| s.to_string()).collect();
        let (code, said, err) = joined(&args);
        assert_eq!(code, 0, "{said}{err}");
    }
    let mut real = lines_of(&[t.path().join("en").to_str().unwrap(), t.path().join("ja").to_str().unwrap()], "cedar");
    real.extend(lines_of(&[t.path().join("en").to_str().unwrap(), t.path().join("ja").to_str().unwrap()], "cedarschema"));
    let (seen, wrong) = not_real(&["cedar", "cedarschema"], &real);
    assert!(seen >= 5, "{seen} blocks of Cedar on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not a line of the Cedar gen writes for the example:\n{}", wrong.join("\n"));
}

#[test]
fn the_commands_on_the_pages_print_what_they_show() {
    let table = sekisho::cli::table();
    let commands: Vec<&str> = table.commands.iter().map(|c| c.name).collect();
    let scratch = ritsu_testkit::TempDir::new("docs-commands");
    let mut wrong = Vec::new();
    let (mut ran, mut listed) = (0, 0);
    for f in pages() {
        for (info, lines) in fenced(&read(&f)) {
            if info != "console" {
                continue;
            }
            // Each `$ sekisho …`, and the lines under it up to the next `$`: what it prints.
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
                let Some(rest) = cmd.strip_prefix("sekisho ").or_else(|| cmd.strip_prefix("ritsu sekisho ")) else { continue };
                let first = rest.split_whitespace().next().unwrap_or("");
                if out.is_empty() {
                    // A command shown without its output, in a list of commands: it is one.
                    if !commands.contains(&first) {
                        wrong.push(format!("{}: `sekisho {first}` is not a command", f.display()));
                    }
                    listed += 1;
                    continue;
                }
                // `--out <dir>` writes into the scratch directory, and is said as the page says it.
                let mut args: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
                let mut shown_out = None;
                if let Some(i) = args.iter().position(|a| a == "--out") {
                    let dir = scratch.path().join(format!("out{ran}"));
                    shown_out = Some((dir.display().to_string(), args[i + 1].clone()));
                    args[i + 1] = dir.display().to_string();
                }
                let (_, mut got, err) = joined(&args);
                got.push_str(&err);
                if let Some((dir, page)) = &shown_out {
                    got = got.replace(dir.as_str(), page);
                }
                let want = out.join("\n") + "\n";
                if got != want {
                    wrong.push(format!("{}: $ {cmd}\n--- the page shows\n{want}--- it prints\n{got}", f.display()));
                }
                ran += 1;
            }
        }
    }
    assert!(ran >= 6 && listed >= 18, "{ran} commands run and {listed} listed: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_diagnostics_on_the_pages_are_what_the_mutants_give() {
    let heads = ["error[", "warning[", "エラー[", "警告["];
    let goldens: Vec<String> = files(Path::new("tests/golden"), "txt").iter().map(|p| read(p)).collect();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages() {
        for (_, lines) in fenced(&read(&f)) {
            let Some(first) = lines.first() else { continue };
            if !heads.iter().any(|h| first.starts_with(h)) {
                continue;
            }
            seen += 1;
            let block = lines.join("\n") + "\n";
            if !goldens.iter().any(|g| g.contains(&block)) {
                wrong.push(format!("{}: {first}", f.display()));
            }
        }
    }
    assert!(seen >= 5, "{seen} diagnostics on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not what any mutant gives (tests/golden/*.txt):\n{}", wrong.join("\n"));
}

#[test]
fn the_numbers_the_readmes_say_are_counted_again() {
    let mutants: Vec<String> = fs::read_dir("tests/mutants").unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".gate")).collect();
    let japanese = mutants.iter().filter(|m| !m.is_ascii()).count();
    // each mutant with a Japanese name has a twin with an English name, by its code
    let code = |m: &String| m.split('_').next().unwrap().to_string();
    for c in mutants.iter().filter(|m| !m.is_ascii()).map(code).collect::<BTreeSet<_>>() {
        let (ja, en) = mutants.iter().filter(|m| code(m) == c).fold((0, 0), |(j, e), m| if m.is_ascii() { (j, e + 1) } else { (j + 1, e) });
        assert!(en >= ja, "{c}: {ja} mutants with Japanese names, {en} with English ones");
    }
    let en = read(Path::new("README.md"));
    let ja = read(Path::new("README.ja.md"));
    for (en_says, ja_says) in [
        (format!("{} mutants", mutants.len()), format!("{} の変異", mutants.len())),
        (format!("{japanese} of them with Japanese names"), format!("日本語の名前のもの {japanese}")),
    ] {
        assert!(en.contains(&en_says), "README.md does not say `{en_says}`");
        assert!(ja.contains(&ja_says), "README.ja.md does not say `{ja_says}`");
    }
}

#[test]
fn the_reference_lists_every_keyword() {
    let page = read(Path::new("docs/reference.md"));
    let missing: Vec<&str> = sekisho::kw::TABLE.iter().flat_map(|(_, ws)| ws.iter().copied()).filter(|w| !page.contains(&format!("`{w}`"))).collect();
    assert!(missing.is_empty(), "docs/reference.md does not list {missing:?}");
}

#[test]
fn the_pages_link_to_files_that_are_there() {
    let mut wrong = Vec::new();
    for f in pages() {
        let text = read(&f);
        let dir = f.parent().unwrap().to_path_buf();
        let mut rest = text.as_str();
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let end = after.find(')').unwrap_or(after.len());
            let target = after[..end].split('#').next().unwrap_or("");
            rest = &after[end..];
            if target.is_empty() || target.starts_with("http") {
                continue;
            }
            // a link that leaves the crate (to ritsu's README, its licenses, the skill) has to land
            // on a file of the repository too
            if !dir.join(target).exists() {
                wrong.push(format!("{}: {target}", f.display()));
            }
        }
    }
    assert!(wrong.is_empty(), "links to nothing:\n{}", wrong.join("\n"));
}
