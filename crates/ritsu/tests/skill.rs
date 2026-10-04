//! The agent skill of ritsu, skills/ritsu (PLAN F.3). Its SKILL.md is written by hand and stands on
//! its own inside someone else's project, as the skills of the languages do; this holds it to what
//! ritsu does:
//!
//! - its frontmatter is what the Agent Skills format asks for, and its `license` is the
//!   repository's, as the root Cargo.toml names it;
//! - no link in it leaves the skill's directory, and every `#anchor` is a heading of it;
//! - the paths it gives for the skills of the languages are the shape those skills have, and the
//!   ones that exist hold a skill of that name;
//! - its table lists every code of ritsu's ledger, and no other;
//! - the output it shows (the blocks marked `text`) is lines of one of ritsu's golden files;
//! - the commands it names are commands.

mod common;

use common::{Page, commands_named, link_targets, lines_in_order, page, root, shown_lines, trimmed};
use ritsu::cli::LANGUAGES;
use std::fs;
use std::path::{Path, PathBuf};

fn skill() -> PathBuf {
    root().join("skills/ritsu")
}

fn skill_page() -> Page {
    page("skills/ritsu/SKILL.md")
}

/// The frontmatter of a SKILL.md, as its lines.
fn frontmatter(text: &str) -> Vec<String> {
    let front = text.strip_prefix("---\n").and_then(|t| t.split_once("\n---\n")).map(|(f, _)| f).expect("SKILL.md starts with its frontmatter");
    front.lines().map(str::to_string).collect()
}

fn field(front: &[String], key: &str) -> Option<String> {
    front.iter().find_map(|l| l.strip_prefix(&format!("{key}: ")).map(str::to_string))
}

#[test]
fn skill_md_says_what_it_is() {
    let front = frontmatter(&skill_page().text);
    // The keys the format has.
    for l in &front {
        let key = l.split(':').next().unwrap();
        assert!(["name", "description", "license", "compatibility", "metadata", "allowed-tools"].contains(&key), "`{key}` is not a key of the frontmatter");
    }
    assert_eq!(field(&front, "name").as_deref(), Some("ritsu"), "the name is the directory's");
    for (k, most) in [("description", 1024), ("compatibility", 500)] {
        let v = field(&front, k).unwrap_or_else(|| panic!("SKILL.md has no {k}"));
        assert!(!v.is_empty() && v.chars().count() <= most, "the {k} has {} characters; 1 to {most}", v.chars().count());
        // A plain YAML scalar ends a key at ": " and a comment at " #", so a value must hold neither.
        assert!(!v.contains(": ") && !v.contains(" #") && !v.starts_with(['"', '\'', '>', '|']), "the {k} must stay a plain YAML scalar");
    }
    // The repository's own license, as the root Cargo.toml names it.
    let cargo = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let license = cargo.lines().find_map(|l| l.strip_prefix("license = \"")).and_then(|l| l.strip_suffix('"')).expect("the workspace names its license");
    assert_eq!(field(&front, "license").as_deref(), Some(license), "SKILL.md names the repository's license");
}

#[test]
fn no_link_leaves_the_skill() {
    let p = skill_page();
    let mut wrong = Vec::new();
    for (line, target) in link_targets(&p.text) {
        if target.starts_with("https://") || target.starts_with("http://") {
            continue;
        }
        let (file, frag) = target.split_once('#').map_or((target.as_str(), None), |(f, a)| (f, Some(a)));
        if file.is_empty() {
            if !p.anchors.iter().any(|a| Some(a.as_str()) == frag) {
                wrong.push(format!("{}:{line}: no heading of the skill is anchored `#{}`", p.name, frag.unwrap_or("")));
            }
        } else if file.contains('/') || !skill().join(file).is_file() {
            wrong.push(format!("{}:{line}: {target} leaves the skill, or leads nowhere in it", p.name));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_skills_of_the_languages_are_where_it_says() {
    let p = skill_page();
    for name in LANGUAGES {
        let at = format!("crates/{name}/skills/{name}/");
        assert!(p.text.contains(&format!("`{at}`")), "SKILL.md does not give `{at}`");
        // The skill of a language is that language's own, by name. (yuen's and sakai's are written
        // by their stage D, PLAN F.1 and F.2: where there is none yet, there is nothing to hold.)
        let md = root().join(&at).join("SKILL.md");
        if md.is_file() {
            let front = frontmatter(&fs::read_to_string(&md).unwrap());
            assert_eq!(field(&front, "name").as_deref(), Some(name), "{at}SKILL.md is the skill of {name}");
        } else {
            assert!(["yuen", "sakai"].contains(&name), "{at}SKILL.md is missing");
        }
    }
}

#[test]
fn the_table_lists_every_code_ritsu_has_and_no_other() {
    let p = skill_page();
    // `| E201 | …`: a row whose first cell is a code.
    let mut listed: Vec<&str> = p
        .text
        .lines()
        .filter_map(|l| l.strip_prefix("| "))
        .filter_map(|l| l.split_once(" |"))
        .map(|(cell, _)| cell)
        .filter(|c| c.len() == 4 && c.starts_with(['E', 'W']) && c[1..].chars().all(|d| d.is_ascii_digit()))
        .collect();
    let ledger = ritsu_cross::codes::ledger();
    let mut have: Vec<&str> = ledger.entries.iter().filter(|e| !e.is_retired()).map(|e| e.code).collect();
    listed.sort();
    have.sort();
    assert_eq!(listed, have, "the table of SKILL.md lists the codes `ritsu explain --all` has");
}

/// The lines of every golden text file of ritsu's tests.
fn goldens() -> Vec<(String, Vec<String>)> {
    fn walk(dir: &Path, out: &mut Vec<(String, Vec<String>)>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "txt")
                && let Ok(text) = fs::read_to_string(&p)
            {
                out.push((p.display().to_string(), trimmed(&text)));
            }
        }
    }
    let mut out = Vec::new();
    walk(&root().join("crates/ritsu/tests/golden"), &mut out);
    out
}

#[test]
fn the_output_it_shows_is_what_ritsu_prints() {
    let p = skill_page();
    let goldens = goldens();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for b in p.blocks.iter().filter(|b| b.info == "text") {
        seen += 1;
        let shown = shown_lines(b);
        if !goldens.iter().any(|(_, file)| lines_in_order(&shown, file)) {
            wrong.push(format!("{}:{}: a ```text block that is the lines of no golden file of ritsu's tests, in order", p.name, b.at));
        }
    }
    // A check of two files and a run.
    assert!(seen >= 2, "{seen} blocks of output in the skill: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_commands_it_names_are_commands() {
    let (seen, wrong) = commands_named(&[skill_page()]);
    assert!(seen >= 10, "{seen} commands named in the skill: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
