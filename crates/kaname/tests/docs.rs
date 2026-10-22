//! The READMEs, `docs/reference.md` and the agent skill show what kaname does (DESIGN 16.1), as
//! sekisho's do:
//!
//! - Every line of `.kn` on the pages is a line of a `.kn` under `examples/` or `tests/`, and every line
//!   of a host (TypeScript, Python, Go) in the READMEs a line of a host of the examples.
//! - Every command shown without its output is a command of kaname's table. The ones shown with what
//!   they print are run by ritsu's `tests/kaname.rs`, as `ritsu` runs them: a module's effects, and
//!   ritsu's diagnostics across the languages, need the verdicts of ritsu-cross, which this crate
//!   does not have.
//! - The reference lists every keyword of `kw.rs`, and every relative link leads to a file.
//!
//! (`docs/codes.md` and `docs/codes.ja.md` are held to `explain --all` by `tests/codes.rs`.)

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
    out.extend(files(Path::new("../../skills/kaname"), "md"));
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
fn the_kn_on_the_pages_is_from_the_files() {
    let mut real = BTreeSet::new();
    for dir in ["examples", "tests"] {
        for f in files(Path::new(dir), "kn") {
            real.extend(read(&f).lines().map(|l| l.trim().to_string()));
        }
    }
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages() {
        for (info, lines) in fenced(&read(&f)) {
            if info != "kn" {
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
    assert!(seen >= 6, "{seen} blocks of .kn on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not a line of any .kn under examples/ or tests/:\n{}", wrong.join("\n"));
}

/// The code of a host in the READMEs (fenced `ts`, `python` or `go`) is lines of the hosts of the
/// examples (`examples/*/host/main.*`), which ritsu's `tests/kaname.rs` runs on the package `ritsu gen`
/// writes. (`docs/reference.md` shows the code generated, held to the generator by its own tests.)
#[test]
fn the_hosts_on_the_pages_are_from_the_files() {
    let mut real = BTreeSet::new();
    for ext in ["ts", "py", "go"] {
        for f in files(Path::new("examples"), ext) {
            real.extend(read(&f).lines().map(|l| l.trim().to_string()));
        }
    }
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in ["README.md", "README.ja.md"].map(PathBuf::from) {
        for (info, lines) in fenced(&read(&f)) {
            if !["ts", "python", "go"].contains(&info.as_str()) {
                continue;
            }
            seen += 1;
            for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
                if !real.contains(l) {
                    wrong.push(format!("{}: {l}", f.display()));
                }
            }
        }
    }
    assert!(seen >= 2, "{seen} blocks of a host on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "not a line of any host under examples/:\n{}", wrong.join("\n"));
}

#[test]
fn the_commands_listed_are_commands() {
    let table = kaname::cli::table();
    let commands: Vec<&str> = table.commands.iter().map(|c| c.name).collect();
    let mut wrong = Vec::new();
    let (mut listed, mut shown) = (0, 0);
    for f in pages() {
        for (info, lines) in fenced(&read(&f)) {
            if info != "console" {
                continue;
            }
            for (i, l) in lines.iter().enumerate() {
                let Some(cmd) = l.strip_prefix("$ ") else { continue };
                let Some(rest) = cmd.strip_prefix("ritsu kaname ").or_else(|| cmd.strip_prefix("kaname ")) else { continue };
                let first = rest.split_whitespace().next().unwrap_or("");
                if !commands.contains(&first) {
                    wrong.push(format!("{}: `kaname {first}` is not a command", f.display()));
                }
                // one followed by what it prints is run by ritsu's tests/kaname.rs
                if lines.get(i + 1).is_some_and(|n| !n.starts_with("$ ")) {
                    shown += 1;
                } else {
                    listed += 1;
                }
            }
        }
    }
    assert!(listed >= 12 && shown >= 6, "{listed} commands listed and {shown} shown with their output: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_reference_lists_every_keyword() {
    let page = read(Path::new("docs/reference.md"));
    let missing: Vec<&str> = kaname::kw::TABLE.iter().flat_map(|(_, ws)| ws.iter().copied()).filter(|w| !page.contains(&format!("`{w}`"))).collect();
    assert!(missing.is_empty(), "docs/reference.md does not list {missing:?}");
    let builtins: Vec<&str> = kaname::kw::BUILTINS.iter().copied().filter(|w| !page.contains(&format!("`{w}")) && !page.contains(&format!(" {w}("))).collect();
    assert!(builtins.is_empty(), "docs/reference.md does not list the builtins {builtins:?}");
    // a builtin written after a value, as it is written: `d.get(k)`, `xs.sort_by(x => …)`
    let methods: Vec<&str> = kaname::kw::METHODS.iter().copied().filter(|w| !page.contains(&format!(".{w}("))).collect();
    assert!(methods.is_empty(), "docs/reference.md does not list the builtins written after a value {methods:?}");
}

/// A page with its code left out: the blocks between fences, and the spans between backticks.
fn prose(text: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut code = false;
        for c in line.chars() {
            if c == '`' {
                code = !code;
            } else if !code {
                out.push(c);
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn the_pages_link_to_files_that_are_there() {
    let mut wrong = Vec::new();
    for f in pages().into_iter().chain([PathBuf::from("DESIGN.md")]) {
        // the prose alone: a generic function written in code (`first_or[T](xs: …)`) is no link
        let text = prose(&read(&f));
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
            // a link that leaves the crate (to ritsu's README, its licenses, the skill, ritsu's golden
            // page of an example) has to land on a file of the repository too
            if !dir.join(target).exists() {
                wrong.push(format!("{}: {target}", f.display()));
            }
        }
    }
    assert!(wrong.is_empty(), "links to nothing:\n{}", wrong.join("\n"));
}
