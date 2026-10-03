//! The agent skill (skills/dandori) stands on its own inside someone else's project: its references
//! are the site's English pages as skills/sync.sh copies them, no link in it leaves the directory,
//! and SKILL.md's frontmatter is what the Agent Skills format asks for. What the skill says about
//! the tool (its diagnostics, its lines of `.flow`) is held by tests/docs.rs, as the site is.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn skill() -> PathBuf {
    root().join("skills/dandori")
}

/// The names of the files in a directory, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

#[test]
fn the_references_are_the_pages_as_sync_copies_them() {
    let scratch = std::env::temp_dir().join(format!("dandori-skill-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let out = Command::new("sh").arg(root().join("skills/sync.sh")).arg(&scratch).output().expect("could not run sh");
    assert!(out.status.success(), "skills/sync.sh failed: {}", String::from_utf8_lossy(&out.stderr));
    let built = names(&scratch);
    let committed: Vec<String> = names(&skill()).into_iter().filter(|n| n != "SKILL.md").collect();
    assert_eq!(committed, built, "skills/dandori holds other files than skills/sync.sh builds, besides SKILL.md");
    let stale: Vec<&String> = built.iter().filter(|n| fs::read_to_string(scratch.join(n)).unwrap() != fs::read_to_string(skill().join(n)).unwrap()).collect();
    let _ = fs::remove_dir_all(&scratch);
    assert!(stale.is_empty(), "not what skills/sync.sh builds from website/docs; run skills/sync.sh: {stale:?}");
}

/// The targets of the Markdown links in a text, outside fenced code and inline code.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        // inline code can hold `](`; drop it before looking
        let plain: String = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        let mut rest = plain.as_str();
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let Some(j) = after.find(')') else { break };
            out.push(after[..j].to_string());
            rest = &after[j + 1..];
        }
    }
    out
}

#[test]
fn no_link_leaves_the_skill() {
    let mut wrong = Vec::new();
    let mut seen = 0;
    for name in names(&skill()) {
        let text = fs::read_to_string(skill().join(&name)).unwrap();
        for target in links(&text) {
            seen += 1;
            if target.starts_with("https://") || target.starts_with("http://") || target.starts_with('#') {
                continue;
            }
            let file = target.split('#').next().unwrap();
            if file.contains('/') || !skill().join(file).is_file() {
                wrong.push(format!("{name}: {target}"));
            }
        }
    }
    assert!(seen > 10, "hardly any links found in skills/dandori: {seen}");
    assert!(wrong.is_empty(), "links that leave skills/dandori, or lead nowhere in it:\n{}", wrong.join("\n"));
}

#[test]
fn skill_md_says_what_it_is() {
    let text = fs::read_to_string(skill().join("SKILL.md")).unwrap();
    let front = text.strip_prefix("---\n").and_then(|t| t.split_once("\n---\n")).map(|(f, _)| f).expect("SKILL.md starts with its frontmatter");
    let field = |k: &str| front.lines().find_map(|l| l.strip_prefix(&format!("{k}: ")).map(str::to_string));
    assert_eq!(field("name").as_deref(), Some("dandori"), "the name is the directory's");
    for (k, most) in [("description", 1024), ("compatibility", 500)] {
        let v = field(k).unwrap_or_else(|| panic!("SKILL.md has no {k}"));
        assert!(!v.is_empty() && v.chars().count() <= most, "the {k} has {} characters; 1 to {most}", v.chars().count());
        // a plain YAML scalar ends a key at ": ", so a value must not hold one
        assert!(!v.contains(": ") && !v.starts_with(['"', '\'', '>', '|']), "the {k} must stay a plain YAML scalar");
    }
    assert!(field("license").is_some(), "SKILL.md has no license");
}
