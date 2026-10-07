//! What tests/readme.rs and tests/skill.rs share: reading a page of Markdown, running the commands
//! it shows in `console` blocks, and holding the lines of its code blocks to the files of the
//! repository, the way each language's tests/docs.rs holds its pages.

#![allow(dead_code)]

use ritsu::cli::LANGUAGES;
use ritsu_base::ledger::Repro;
use ritsu_testkit::TempDir;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// The words after the opening fence of a block that hold the lines of one language.
pub const CODE: [&str; 8] = ["rule", "flow", "cal", "book", "geas", "req", "ctx", "gate"];

pub struct Block {
    pub info: String,
    pub lines: Vec<String>,
    /// The line of the opening fence, from 1.
    pub at: usize,
    /// How many headings come before it: blocks with the same number are in one section.
    pub section: usize,
}

pub struct Page {
    /// The path from the root of the repository.
    pub name: String,
    pub text: String,
    pub blocks: Vec<Block>,
    /// The headings as GitHub anchors them, or as the id the site's `attr_list` gives them
    /// (`## Types { #types }`).
    pub anchors: Vec<String>,
}

/// GitHub's anchor of a heading: lower case, letters, digits, `-` and `_` kept, spaces turned into
/// `-`, the rest dropped.
pub fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// The anchor of a heading: the id written after it as `{ #id }`, which the site's pages use where
/// the id has to be a word's own spelling (`### Requirement { #Requirement }`), else GitHub's.
pub fn heading_anchor(heading: &str) -> String {
    let h = heading.trim();
    match (h.rfind(" { #"), h.strip_suffix(" }")) {
        (Some(open), Some(_)) => h[open + 4..h.len() - 2].trim().to_string(),
        _ => anchor(h),
    }
}

/// The page at `name`, a path from the root of the repository.
pub fn page(name: &str) -> Page {
    let text = fs::read_to_string(root().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut blocks = Vec::new();
    let mut anchors = Vec::new();
    let mut open: Option<Block> = None;
    for (i, line) in text.lines().enumerate() {
        match (line.trim_start().strip_prefix("```"), open.take()) {
            (Some(_), Some(block)) => blocks.push(block),
            (Some(info), None) => open = Some(Block { info: info.trim().to_string(), lines: Vec::new(), at: i + 1, section: anchors.len() }),
            (None, Some(mut block)) => {
                block.lines.push(line.to_string());
                open = Some(block);
            }
            (None, None) => {
                if let Some(h) = line.strip_prefix('#') {
                    anchors.push(heading_anchor(h.trim_start_matches('#')));
                }
            }
        }
    }
    assert!(open.is_none(), "{name}: a fence is never closed");
    Page { name: name.to_string(), text, blocks, anchors }
}

// ---- the commands

/// A directory holding `ritsu` and a link to it for each language: what unpacking a release into a
/// directory on the PATH gives.
pub fn links() -> TempDir {
    let t = TempDir::new("pages-links");
    for name in std::iter::once("ritsu").chain(LANGUAGES) {
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ritsu"), t.path().join(name)).unwrap();
    }
    t
}

/// The command line as a shell reads it, in `dir`, with standard error where standard output is, as
/// a terminal shows them, and no language asked of the environment.
pub fn sh(links: &Path, dir: &Path, command: &str) -> String {
    let path = format!("{}:{}", links.display(), std::env::var("PATH").unwrap_or_default());
    let mut c = Command::new("sh");
    c.arg("-c").arg(format!("{command} 2>&1")).current_dir(dir).env("PATH", path);
    for var in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG"] {
        c.env_remove(var);
    }
    String::from_utf8_lossy(&c.output().expect("could not run sh").stdout).into_owned()
}

/// The lines of a text, without the spaces at their ends and the empty lines at the end: how two
/// texts are told apart.
pub fn lines_of(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.lines().map(|l| l.trim_end().to_string()).collect();
    while v.last().is_some_and(|l| l.is_empty()) {
        v.pop();
    }
    v
}

/// Whether `want` (a page's lines, a line `…` standing for any number of lines) is `got`.
pub fn same(want: &[String], got: &[String]) -> bool {
    match want.split_first() {
        None => got.is_empty(),
        Some((w, rest)) if w == "…" => (0..=got.len()).any(|i| same(rest, &got[i..])),
        Some((w, rest)) => got.first() == Some(w) && same(rest, &got[1..]),
    }
}

/// The reproductions of ritsu's ledger that are files in a directory: each one's files, as (name,
/// text).
pub fn repros() -> Vec<Vec<(&'static str, &'static str)>> {
    ritsu_cross::codes::ledger()
        .entries
        .iter()
        .flat_map(|e| std::iter::once(&e.repro).chain(e.repro_ja.iter()))
        .filter_map(|r| match r {
            Repro::Dir { files, .. } => Some(files.clone()),
            _ => None,
        })
        .collect()
}

/// The files of the reproduction whose every file is a code block of the page above `at`, in the
/// same section: where a command shown after such blocks is run.
fn reproduction_above(p: &Page, at: &Block) -> Option<Vec<(&'static str, &'static str)>> {
    let above: Vec<Vec<String>> = p.blocks.iter().filter(|b| b.section == at.section && b.at < at.at && CODE.contains(&b.info.as_str())).map(|b| lines_of(&b.lines.join("\n"))).collect();
    repros().into_iter().find(|files| files.iter().all(|(_, body)| above.contains(&lines_of(body))))
}

/// The runs a block shows: each command, and the lines under it up to the next `$`.
pub fn runs(block: &Block) -> Vec<(String, Vec<String>)> {
    let mut runs: Vec<(String, Vec<String>)> = Vec::new();
    for l in &block.lines {
        match l.strip_prefix("$ ") {
            Some(cmd) => runs.push((cmd.to_string(), Vec::new())),
            None => {
                if let Some((_, out)) = runs.last_mut() {
                    out.push(l.clone());
                }
            }
        }
    }
    runs
}

/// Run every `$ ritsu …` and `$ <language> …` of the `console` blocks of the pages that has its
/// output under it, and say what differs. How many were run, and the differences.
///
/// A command is run in the root of the repository, in the directory of a `$ cd` before it in its
/// block, or in the files of a reproduction of the ledger when the code blocks above it in the same
/// section are those files. A command shown without its output, in a list, is only held to be a
/// command of `ritsu` when it starts with that. A `gen` writes into a scratch directory in place
/// of the `--out <dir>` the page shows, and what it prints is read with that `<dir>` in the scratch
/// directory's place, so the repository is not written into; a `gen` shown without `--out` is
/// wrong.
pub fn run_the_consoles(pages: &[Page]) -> (usize, Vec<String>) {
    let links = links();
    let scratch = TempDir::new("pages-out");
    let commands: Vec<&str> = ritsu::cli::commands().iter().map(|c| c.name).collect();
    let mut wrong = Vec::new();
    let mut ran = 0;
    for p in pages {
        for block in p.blocks.iter().filter(|b| b.info == "console") {
            // The files of a reproduction above the block, laid out for the commands in it.
            let files = reproduction_above(p, block).map(|files| {
                let t = TempDir::new("pages-files");
                for (name, body) in files {
                    t.write(name, body);
                }
                t
            });
            let mut cwd = files.as_ref().map_or_else(root, |t| t.path().to_path_buf());
            for (cmd, out) in runs(block) {
                let first = cmd.split_whitespace().next().unwrap_or("");
                if first == "cd" {
                    cwd = root().join(cmd.trim_start_matches("cd").trim());
                    assert!(cwd.is_dir(), "{}:{}: `$ {cmd}` is no directory of the repository", p.name, block.at);
                    continue;
                }
                if first != "ritsu" && !LANGUAGES.contains(&first) {
                    continue;
                }
                if out.is_empty() {
                    // A command shown without its output, in a list: `ritsu` is followed by one.
                    let word = cmd.split_whitespace().nth(1).unwrap_or("");
                    if first == "ritsu" && !commands.contains(&word) && !word.starts_with("--") {
                        wrong.push(format!("{}:{}: `ritsu {word}` is not a command", p.name, block.at));
                    }
                    continue;
                }
                assert!(!cmd.contains(['|', ';', '>', '<', '`', '$', '"', '\'']), "{}:{}: `$ {cmd}` is more than a command to run", p.name, block.at);
                // `ritsu check` in the root would check every file of the repository, and run the
                // claims of every `.geas` there: it is run on the files the blocks above it show.
                if cwd == root() && first == "ritsu" && cmd.split_whitespace().nth(1) == Some("check") {
                    wrong.push(format!("{}:{}: `$ {cmd}` is shown after no files of a reproduction in the ledger, and in no directory of the repository", p.name, block.at));
                    continue;
                }
                // A `gen` writes into the scratch directory in place of the `--out` the page shows
                // (without one, it would write `generated/` into the repository), and the paths it
                // prints are read as the page shows them.
                let mut words: Vec<String> = cmd.split_whitespace().map(String::from).collect();
                let mut shown_out = None;
                if let Some(i) = words.iter().position(|w| w == "--out")
                    && let Some(dir) = words.get_mut(i + 1)
                {
                    let scratch_out = scratch.path().join(format!("out{ran}")).display().to_string();
                    shown_out = Some((scratch_out.clone(), std::mem::replace(dir, scratch_out)));
                } else if words.iter().take(3).any(|w| w == "gen") {
                    wrong.push(format!("{}:{}: `$ {cmd}` would write into the repository: show it with `--out <dir>`", p.name, block.at));
                    continue;
                }
                let mut got = sh(links.path(), &cwd, &words.join(" "));
                if let Some((scratch_out, shown)) = &shown_out {
                    got = got.replace(scratch_out.as_str(), shown);
                }
                if !same(&lines_of(&out.join("\n")), &lines_of(&got)) {
                    let shown: String = got.lines().take(40).map(|l| format!("{l}\n")).collect();
                    wrong.push(format!("{}:{}: $ {cmd}\n--- the page shows\n{}\n--- it prints\n{shown}", p.name, block.at, out.join("\n")));
                }
                ran += 1;
            }
        }
    }
    (ran, wrong)
}

// ---- the code

/// A line cut short with `…` stands for a real line that has its pieces in that order.
pub fn line_is(shown: &str, real: &str) -> bool {
    let mut at = 0;
    shown.split('…').map(str::trim).filter(|p| !p.is_empty()).all(|p| match real[at..].find(p) {
        Some(i) => {
            at += i + p.len();
            true
        }
        None => false,
    })
}

/// Whether the lines of a block are lines of `file`, each after the one before it.
pub fn lines_in_order(shown: &[&str], file: &[String]) -> bool {
    let mut at = 0;
    shown.iter().all(|l| match file[at..].iter().position(|r| line_is(l, r)) {
        Some(i) => {
            at += i + 1;
            true
        }
        None => false,
    })
}

/// The lines of a text without the spaces at their ends, the empty ones left out.
pub fn trimmed(text: &str) -> Vec<String> {
    text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// The lines of a block as it shows them: no spaces at the ends, no empty ones, no `…` alone.
pub fn shown_lines(b: &Block) -> Vec<&str> {
    b.lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "…").collect()
}

/// Every file of the repository of one of the languages of `CODE`, by extension, with its lines.
/// Nothing behind a symbolic link (the tools of the languages) and nothing a build or a package
/// manager made.
pub fn files_by_language() -> BTreeMap<&'static str, Vec<(String, Vec<String>)>> {
    let mut out: BTreeMap<&'static str, Vec<(String, Vec<String>)>> = CODE.iter().map(|e| (*e, Vec::new())).collect();
    let mut todo = vec![root()];
    while let Some(d) = todo.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let (p, kind) = (e.path(), e.file_type().unwrap());
            let name = e.file_name().to_string_lossy().into_owned();
            if kind.is_dir() {
                if !name.starts_with('.') && !["target", "node_modules", "venv"].contains(&name.as_str()) {
                    todo.push(p);
                }
            } else if kind.is_file()
                && let Some(ext) = p.extension().and_then(|x| x.to_str())
                && let Some(files) = out.get_mut(ext)
                && let Ok(text) = fs::read_to_string(&p)
            {
                files.push((p.display().to_string(), trimmed(&text)));
            }
        }
    }
    // The files of a reproduction are in the ledger's code.
    for (name, body) in repros().into_iter().flatten() {
        if let Some(files) = name.rsplit_once('.').and_then(|(_, ext)| out.get_mut(ext)) {
            files.push((name.to_string(), trimmed(body)));
        }
    }
    out
}

/// The blocks of code of the pages that are not the lines of one file, in order: how many blocks
/// there were, and what is wrong with each of the others.
pub fn code_from_files(pages: &[Page]) -> (usize, Vec<String>) {
    let files = files_by_language();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for p in pages {
        for b in p.blocks.iter().filter(|b| CODE.contains(&b.info.as_str())) {
            seen += 1;
            let real = &files[b.info.as_str()];
            let shown = shown_lines(b);
            if !real.iter().any(|(_, file)| lines_in_order(&shown, file)) {
                // A line of no file at all says what to look at; else the lines come from several.
                let lone = shown.iter().find(|l| !real.iter().any(|(_, file)| file.iter().any(|r| line_is(l, r))));
                let why = lone.map_or("each of its lines is a line of some file".to_string(), |l| format!("`{l}` is a line of none"));
                wrong.push(format!("{}:{}: a ```{} block that is the lines of no one file, in order; {why}", p.name, b.at, b.info));
            }
        }
    }
    (seen, wrong)
}

// ---- the links

/// The targets of the Markdown links in a text, outside fenced code and inline code, with the line.
pub fn link_targets(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence = false;
    for (i, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        // Inline code can hold `](`; drop it before looking.
        let plain: String = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        let mut rest = plain.as_str();
        while let Some(j) = rest.find("](") {
            let after = &rest[j + 2..];
            let Some(k) = after.find(')') else { break };
            out.push((i + 1, after[..k].to_string()));
            rest = &after[k + 1..];
        }
    }
    out
}

/// The inline code on a page, outside fenced code, with the line.
pub fn inline_code(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence = false;
    for (i, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        } else if !fence {
            out.extend(line.split('`').skip(1).step_by(2).map(|s| (i + 1, s.to_string())));
        }
    }
    out
}

/// A command the prose names as `rulec doc` or `ritsu check` is one: the language's `--help` lists
/// it, or ritsu's table of commands has it. How many there were, and the ones that are not.
pub fn commands_named(pages: &[Page]) -> (usize, Vec<String>) {
    let links = links();
    let commands: Vec<&str> = ritsu::cli::commands().iter().map(|c| c.name).collect();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for p in pages {
        for (line, code) in inline_code(&p.text) {
            let words: Vec<&str> = code.split(' ').collect();
            let [first, word] = words[..] else { continue };
            if !word.chars().all(|c| c.is_ascii_lowercase()) {
                continue;
            }
            if first == "ritsu" {
                seen += 1;
                if !commands.contains(&word) {
                    wrong.push(format!("{}:{line}: `{code}`: ritsu has no such command", p.name));
                }
            } else if LANGUAGES.contains(&first) {
                seen += 1;
                let help = sh(links.path(), &root(), &format!("{first} --help"));
                if !help.lines().any(|l| l.trim_start().starts_with(&format!("{first} {word}"))) {
                    wrong.push(format!("{}:{line}: `{code}`: `{first} --help` lists no such command", p.name));
                }
            }
        }
    }
    (seen, wrong)
}
