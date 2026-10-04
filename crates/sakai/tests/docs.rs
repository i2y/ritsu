//! The READMEs, `docs/` and the agent skill show what sakai does (PLAN D.3, D.4), as koyomi's and
//! chobo's do:
//!
//! - `docs/codes.md` and `docs/codes.ja.md` are what `sakai explain --all --format markdown`
//!   prints, in English and in Japanese.
//! - Every line of `.ctx` on the pages is a line of a `.ctx` under `examples/` or `tests/`.
//! - Every `$ sakai …` that shows what it prints prints that, run as `ritsu sakai` runs it (every
//!   language joined); one shown without its output is a command.
//! - Every diagnostic shown is what a mutant gives, word for word as its golden file holds it
//!   (`tests/mutants.rs` holds the golden files to the runs).
//! - The settings shown in `docs/targets.md` are lines of the settings beside the example's code,
//!   and what it shows the linters say is in the linters' golden files (`tests/imports.rs`).
//! - The numbers the READMEs say were compared are counted again here, and the reference lists
//!   every keyword of `kw.rs`.

mod common;

use ritsu_base::text::Lang;
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

/// The pages: the READMEs (the crate's and the examples'), `docs/` and the skill, without the
/// codes pages, which are the output of `explain` and are held to it whole.
fn pages() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = ["README.md", "README.ja.md", "examples/shop/README.md", "examples/shop.ja/README.ja.md", "skills/README.md"].iter().map(PathBuf::from).collect();
    out.extend(files(Path::new("docs"), "md"));
    out.extend(files(Path::new("../../skills/sakai"), "md"));
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

#[test]
fn the_codes_pages_are_what_explain_prints() {
    for (page, lang) in [("docs/codes.md", Lang::En), ("docs/codes.ja.md", Lang::Ja)] {
        let want = sakai::codes::ledger().render_markdown(lang);
        assert!(read(Path::new(page)) == want, "{page} is not what `sakai explain --all --format markdown` prints; write it again with that command");
    }
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

#[test]
fn the_ctx_on_the_pages_is_from_the_files() {
    let real = lines_of(&["examples", "tests"], "ctx");
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages() {
        for (info, lines) in fenced(&read(&f)) {
            if info != "ctx" {
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
    assert!(seen >= 15, "{seen} blocks of .ctx on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not a line of any .ctx under examples/ or tests/:\n{}", wrong.join("\n"));
}

#[test]
fn the_commands_on_the_pages_print_what_they_show() {
    let commands: Vec<&str> = sakai::cli::commands().iter().map(|c| c.name).collect();
    let mut wrong = Vec::new();
    let (mut ran, mut listed) = (0, 0);
    for f in pages() {
        for (info, lines) in fenced(&read(&f)) {
            if info != "console" {
                continue;
            }
            // Each `$ sakai …`, and the lines under it up to the next `$`: what it prints.
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
                let Some(rest) = cmd.strip_prefix("sakai ") else { continue };
                let first = rest.split_whitespace().next().unwrap_or("");
                if out.is_empty() {
                    // A command shown without its output, in a list of commands: it is one.
                    if !commands.contains(&first) {
                        wrong.push(format!("{}: `sakai {first}` is not a command", f.display()));
                    }
                    listed += 1;
                    continue;
                }
                let args: Vec<&str> = rest.split_whitespace().collect();
                let (_, got, err) = common::joined(&args);
                let want = out.join("\n") + "\n";
                if got != want {
                    wrong.push(format!("{}: $ {cmd}\n--- the page shows\n{want}--- it prints\n{got}{err}", f.display()));
                }
                ran += 1;
            }
        }
    }
    assert!(ran >= 10 && listed >= 12, "{ran} commands run and {listed} listed: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_diagnostics_on_the_pages_are_what_the_mutants_give() {
    let heads = ["error[", "warning[", "エラー[", "警告["];
    let goldens: Vec<(PathBuf, String)> = fs::read_dir("tests/golden")
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .map(|p| {
            let s = read(&p);
            (p, s)
        })
        .collect();
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
            if !goldens.iter().any(|(_, g)| g.contains(&block)) {
                wrong.push(format!("{}: {first}", f.display()));
            }
        }
    }
    assert!(seen >= 4, "{seen} diagnostics on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not what any mutant gives (tests/golden/*.txt):\n{}", wrong.join("\n"));
}

#[test]
fn the_settings_and_what_the_linters_say_are_real() {
    let page = read(Path::new("docs/targets.md"));
    let settings = [
        ("ini", "examples/shop/py/.importlinter"),
        ("js", "examples/shop/ts/.dependency-cruiser.cjs"),
        ("java", "examples/shop/java/src/test/java/SakaiContextsTest.java"),
        ("yaml", "examples/shop/go/.go-arch-lint.yml"),
    ];
    let said: BTreeSet<String> = files(Path::new("tests/golden/imports"), "txt").iter().flat_map(|f| read(f).lines().map(|l| l.trim().to_string()).collect::<Vec<_>>()).collect();
    let mut wrong = Vec::new();
    let (mut shown, mut caught) = (BTreeSet::new(), 0);
    for (info, lines) in fenced(&page) {
        if let Some((_, file)) = settings.iter().find(|(i, _)| *i == info) {
            let real: BTreeSet<String> = read(Path::new(file)).lines().map(|l| l.trim().to_string()).collect();
            for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "…") {
                if !real.contains(l) {
                    wrong.push(format!("{file}: {l}"));
                }
            }
            shown.insert(info.clone());
        } else if info == "text" {
            caught += 1;
            for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
                if !said.contains(l) {
                    wrong.push(format!("what a linter says: {l}"));
                }
            }
        }
    }
    assert_eq!(shown.len(), 4, "the settings of the four linters are shown");
    assert_eq!(caught, 4, "what the four linters say is shown");
    assert!(wrong.is_empty(), "not real:\n{}", wrong.join("\n"));
}

fn dirs_in(p: &str) -> usize {
    fs::read_dir(p).map(|r| r.flatten().filter(|e| e.path().is_dir()).count()).unwrap_or(0)
}

#[test]
fn the_numbers_the_readmes_compare_are_counted_again() {
    let mutants = common::mutants();
    let japanese = mutants.iter().filter(|m| !m.is_ascii()).count();
    let rust = mutants.len() - 2 * japanese;
    // each tool: the example and the nested map, in two languages, as they are and with each
    // import added that its tests/code directories hold
    let per_tool: Vec<usize> = ["go", "java", "python", "typescript"].iter().map(|t| 4 + dirs_in(&format!("tests/code/{t}")) + dirs_in(&format!("tests/code/入れ子/{t}")) + dirs_in(&format!("tests/code/nested/{t}"))).collect();
    assert!(per_tool.iter().all(|n| *n == per_tool[0]), "the tools run on different numbers of copies: {per_tool:?}");
    let copies = per_tool.iter().sum::<usize>();
    let en = read(Path::new("README.md"));
    let ja = read(Path::new("README.ja.md"));
    for (want, en_says, ja_says) in [
        (mutants.len(), format!("{} mutants", mutants.len()), format!("{} の変異", mutants.len())),
        (japanese, format!("{japanese} of them with Japanese names"), format!("日本語の名前のもの {japanese}")),
        (rust, format!("{rust} for Rust"), format!("Rust のものが {rust}")),
        (copies, format!("{copies} copies ({} for each tool)", per_tool[0]), format!("{copies} の写し（ツールごとに {}）", per_tool[0])),
    ] {
        assert!(en.contains(&en_says), "README.md does not say `{en_says}` ({want} counted)");
        assert!(ja.contains(&ja_says), "README.ja.md does not say `{ja_says}` ({want} counted)");
    }
}

#[test]
fn the_reference_lists_every_keyword() {
    let page = read(Path::new("docs/reference.md"));
    let missing: Vec<&str> = sakai::kw::TABLE.iter().flat_map(|(_, ws)| ws.iter().copied()).filter(|w| !page.contains(&format!("`{w}`"))).collect();
    assert!(missing.is_empty(), "docs/reference.md does not list {missing:?}");
}

/// Whether a link from a page in `dir` goes out of the crate, to the rest of ritsu (its README, its
/// own map): those are other parts of the repository, with their own tests.
fn leaves_the_crate(dir: &Path, target: &str) -> bool {
    let mut depth = dir.components().filter(|c| !matches!(c, std::path::Component::CurDir)).count() as i64;
    for part in target.split('/') {
        match part {
            ".." => depth -= 1,
            "." | "" => {}
            _ => depth += 1,
        }
        if depth < 0 {
            return true;
        }
    }
    false
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
            if target.is_empty() || target.starts_with("http") || leaves_the_crate(&dir, target) {
                continue;
            }
            if !dir.join(target).exists() {
                wrong.push(format!("{}: {target}", f.display()));
            }
        }
    }
    assert!(wrong.is_empty(), "links to nothing:\n{}", wrong.join("\n"));
}
