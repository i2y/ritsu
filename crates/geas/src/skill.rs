//! `geas skill`: the guide for coding agents, carried in the binary. The files are
//! `skills/geas/`, taken in as they are, so an agent that has only the binary reads
//! the guide the repository has (DESIGN §14). `geas skill` prints `SKILL.md`;
//! `geas skill --install <dir>` writes the folder as `<dir>/geas/`, which is where
//! agents that read Agent Skills look (`.claude/skills/` in a project, or a user's
//! own skills directory). `tests/skill.rs` holds the copy to the folder.

use crate::diag::{t, Diag};
use std::path::{Path, PathBuf};

/// The skill's files, `SKILL.md` first, as they are in `skills/geas/`.
pub const FILES: &[(&str, &str)] = &[
    ("SKILL.md", include_str!("../skills/geas/SKILL.md")),
    ("language.md", include_str!("../skills/geas/language.md")),
    ("commands.md", include_str!("../skills/geas/commands.md")),
    ("gui.md", include_str!("../skills/geas/gui.md")),
    ("map.md", include_str!("../skills/geas/map.md")),
    ("examples.md", include_str!("../skills/geas/examples.md")),
    ("codes.md", include_str!("../skills/geas/codes.md")),
];

/// What `geas skill` prints.
pub fn guide() -> &'static str {
    FILES[0].1
}

fn e081(en: String, ja: String) -> Diag {
    Diag::error("E081", 0, 0, t(en, ja))
}

/// Writes the skill's files to `<dir>/geas/`, making the directories it needs, and
/// returns that folder. A folder already there is written over only with `force`,
/// and then only the skill's own files: geas does not remove what it did not write.
pub fn install(dir: &Path, force: bool) -> Result<PathBuf, Diag> {
    let folder = dir.join("geas");
    let shown = folder.display().to_string();
    if folder.exists() && !force {
        return Err(e081(
            format!("{shown} is already there; `geas skill --install` writes over it only with `--force`"),
            format!("{shown} はすでにあります。`geas skill --install` が上書きするのは、`--force` を付けたときだけです"),
        )
        .note(t(
            "with `--force`, the skill's files are written again and any other file in the folder is left as it is",
            "`--force` を付けると、スキルのファイルを書き直し、フォルダーにあるほかのファイルはそのまま残します",
        )));
    }
    if let Err(e) = std::fs::create_dir_all(&folder) {
        return Err(e081(format!("cannot make {shown}: {e}"), format!("{shown} を作れません: {e}")));
    }
    for (name, text) in FILES {
        let p = folder.join(name);
        if let Err(e) = std::fs::write(&p, text) {
            let p = p.display();
            return Err(e081(format!("cannot write {p}: {e}"), format!("{p} を書けません: {e}")));
        }
    }
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guide_is_skill_md_with_its_frontmatter() {
        assert!(guide().starts_with("---\nname: geas\n"), "{}", &guide()[..40.min(guide().len())]);
    }

    #[test]
    fn every_file_is_a_page_of_its_own() {
        let mut names: Vec<&str> = FILES.iter().map(|(n, _)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), FILES.len());
        assert!(FILES.iter().all(|(n, text)| n.ends_with(".md") && !text.is_empty()));
    }
}
