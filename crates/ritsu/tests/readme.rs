//! The READMEs at the root, README.md and README.ja.md, show what ritsu does, and this holds them
//! to what it does (PLAN F.3), as each language's tests/docs.rs holds its own pages:
//!
//! - Every `$ ritsu …` and `$ <language> …` in a `console` block is run, and prints what the page
//!   shows under it: the lines in that order, a line `…` standing for any number of lines. It is
//!   run where the page puts it: in the root of the repository, or in the directory of a `$ cd`
//!   before it in the block, or, when the code blocks above it in the same section are the files
//!   of a reproduction in ritsu's ledger, in those files. The commands are found the way a reader
//!   with a release unpacked on the PATH finds them: `ritsu`, and a link to it for each language.
//! - Every line of `.rule`, `.flow`, `.cal`, `.book`, `.geas`, `.req` and `.ctx` on the pages is a
//!   line of a file of that language in the repository, or of a reproduction in the ledger; the
//!   lines of one block come from one file, in its order, and `…` cuts a line short.
//! - A command the prose names as `rulec doc` or `ritsu check` is one.
//! - Every relative link leads to a file, and every `#anchor` to a heading of the page it is on.
//! - The table lists the seven languages as `ritsu` has them, the page names the seven links, the
//!   `cargo install` lines name this repository and a package of it that has a binary, and no
//!   page names the repository of one language by itself.
//! - What a link made by hand does, as the Commands section says it does.
//!
//! (tests/common/mod.rs holds what tests/skill.rs shares with this file.)

mod common;

use common::{Page, commands_named, link_targets, page, root, run_the_consoles, code_from_files, CODE};
use ritsu::cli::LANGUAGES;
use ritsu_testkit::TempDir;
use std::fs;
use std::process::Command;

fn pages() -> Vec<Page> {
    ["README.md", "README.ja.md"].iter().map(|n| page(n)).collect()
}

#[test]
fn the_commands_on_the_pages_print_what_they_show() {
    let (ran, wrong) = run_the_consoles(&pages());
    // `koyomi check` and `ritsu check`, on each of the two pages.
    assert!(ran >= 4, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_code_on_the_pages_is_from_the_files() {
    let (seen, wrong) = code_from_files(&pages());
    // The seven languages and the refund rule and flow, on two pages.
    assert!(seen >= 2 * (CODE.len() + 2), "{seen} blocks of code on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_commands_the_prose_names_are_commands() {
    let (seen, wrong) = commands_named(&pages());
    // `check` of three languages, `doc` of four and `yuen trace`, on two pages.
    assert!(seen >= 2 * 8, "{seen} commands named in the prose: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_links_on_the_pages_lead_somewhere() {
    let mut wrong = Vec::new();
    let mut seen = 0;
    for p in pages() {
        for (line, target) in link_targets(&p.text) {
            seen += 1;
            if target.starts_with("https://") || target.starts_with("http://") {
                continue;
            }
            let (file, frag) = target.split_once('#').map_or((target.as_str(), None), |(f, a)| (f, Some(a)));
            if file.is_empty() {
                if !p.anchors.iter().any(|a| Some(a.as_str()) == frag) {
                    wrong.push(format!("{}:{line}: no heading of the page is anchored `#{}`", p.name, frag.unwrap_or("")));
                }
            } else if !root().join(file).exists() {
                wrong.push(format!("{}:{line}: {file} is no file of the repository", p.name));
            }
        }
    }
    // The seven rows of the table, on two pages.
    assert!(seen >= 2 * LANGUAGES.len(), "{seen} links on the pages: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_table_and_the_links_are_the_seven_languages_as_ritsu_has_them() {
    for p in pages() {
        // `| [rulec](#…) | `.rule` | …`: the first cell of each row under the header.
        let names: Vec<&str> = p.text.lines().filter_map(|l| l.strip_prefix("| [")).filter_map(|l| l.split_once("](")).map(|(name, _)| name).collect();
        assert_eq!(names, LANGUAGES, "{}: the table names the languages in the order `ritsu --help` lists them", p.name);
        // The Commands section names the seven links, in that order, as a list.
        let comma = if p.name.ends_with(".ja.md") { "、" } else { ", " };
        let listed = LANGUAGES.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(comma);
        assert!(p.text.replace('\n', " ").contains(&listed), "{}: the seven links are not named as {listed}", p.name);
    }
}

/// The names of the packages of the workspace that have a binary, from their Cargo.toml.
fn packages_with_a_binary() -> Vec<String> {
    let mut out = Vec::new();
    for e in fs::read_dir(root().join("crates")).unwrap().flatten() {
        let Ok(text) = fs::read_to_string(e.path().join("Cargo.toml")) else { continue };
        let name = text.lines().find_map(|l| l.strip_prefix("name = \"")).and_then(|l| l.strip_suffix('"')).expect("a package has a name");
        if e.path().join("src/main.rs").is_file() || text.contains("[[bin]]") {
            out.push(name.to_string());
        }
    }
    out
}

#[test]
fn the_install_lines_name_this_repository_and_what_it_installs() {
    let cargo = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let repository = cargo.lines().find_map(|l| l.strip_prefix("repository = \"")).and_then(|l| l.strip_suffix('"')).expect("the workspace names its repository");
    let with_a_binary = packages_with_a_binary();
    let mut wrong = Vec::new();
    let mut installed = Vec::new();
    for p in pages() {
        for (n, line) in p.text.lines().enumerate() {
            if let Some(rest) = line.trim().strip_prefix("cargo install ") {
                let words: Vec<&str> = rest.split_whitespace().collect();
                let url = words.iter().position(|w| *w == "--git").and_then(|i| words.get(i + 1));
                let package = words.last().unwrap();
                if url != Some(&repository) || !words.contains(&"--locked") || !with_a_binary.iter().any(|name| name == package) {
                    wrong.push(format!("{}:{}: `{line}` is not `cargo install --git {repository} --locked <a package with a binary>`", p.name, n + 1));
                }
                installed.push(package.to_string());
            }
            // The repository of a language by itself is not public: only ritsu's is named.
            let mut rest = line;
            while let Some(pos) = rest.find("github.com/i2y/") {
                let name: String = rest[pos + 15..].chars().take_while(|c| c.is_alphanumeric() || *c == '-').collect();
                if name != "ritsu" {
                    wrong.push(format!("{}:{}: names github.com/i2y/{name}", p.name, n + 1));
                }
                rest = &rest[pos + 15..];
            }
        }
    }
    // ritsu, and one language alone, on each of the two pages.
    installed.sort();
    assert_eq!(installed, ["ritsu", "ritsu", "rulec", "rulec"], "the install lines of the pages");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// `cargo install` installs the binaries of the package, which for ritsu is the one, `ritsu`: the
/// links of the Commands section are what the release archive adds, or a reader's own.
#[test]
fn cargo_installs_ritsu_alone_and_a_link_made_by_hand_is_a_language() {
    let manifest = root().join("crates/ritsu");
    let toml = fs::read_to_string(manifest.join("Cargo.toml")).unwrap();
    assert!(manifest.join("src/main.rs").is_file() && !manifest.join("src/bin").exists() && !toml.contains("[[bin]]"), "ritsu has one binary, and `cargo install` makes no link");
    // `ln -s "$(command -v ritsu)" ~/.cargo/bin/rulec`, and the same for each of the others.
    let t = TempDir::new("readme-by-hand");
    for name in LANGUAGES {
        let link = t.path().join(name);
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ritsu"), &link).unwrap();
        let o = Command::new(&link).arg("--version").env_remove("RITSU_LANG").output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(o.status.success() && out.starts_with(&format!("{name} ")) && out.lines().count() == 1, "{name}: {out}");
    }
}
