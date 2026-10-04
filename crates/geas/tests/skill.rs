//! The agent skill (`skills/geas`) and `geas skill`: the binary carries the folder as it is and
//! installs it; `codes.md` is what `skills/sync.sh` writes from `geas explain --all`; no link in the
//! skill leaves the folder, since a copy of it sits in someone else's project; `SKILL.md`'s
//! frontmatter is what the Agent Skills format asks for, with the repository's license;
//! and the driver `gui.md` gives runs. What the pages say about the tool (its claims files, its
//! outputs, its commands, options and codes) is held by `tests/docs.rs`.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn skill() -> PathBuf {
    root().join("../../skills/geas")
}

/// The names of the files in a directory, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> =
        std::fs::read_dir(dir).expect("a directory").map(|e| e.expect("an entry").file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

#[test]
fn geas_skill_prints_skill_md() {
    let s = Scratch::new("skill-print");
    let (out, err, code) = run(s.path(), &["skill"], &[]);
    assert_eq!((err.as_str(), code), ("", 0));
    assert_eq!(out, repo_file("../../skills/geas/SKILL.md"));
}

/// What `geas skill --install` writes is the folder, file for file and byte for byte; a second
/// install is refused without `--force`, and with it writes the files again and leaves a file of
/// the person's own.
#[test]
fn the_install_writes_the_folder_as_it_is() {
    let s = Scratch::new("skill-install");
    let (out, err, code) = run(s.path(), &["skill", "--install", "here/skills"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert_eq!(out, "wrote the geas skill to here/skills/geas (7 files)\n");
    let installed = s.path().join("here/skills/geas");
    assert_eq!(names(&installed), names(&skill()), "the binary carries other files than skills/geas holds");
    for n in names(&skill()) {
        assert!(
            std::fs::read(installed.join(&n)).expect("an installed file") == std::fs::read(skill().join(&n)).expect("a skill file"),
            "{n} in the binary is not skills/geas/{n}; build again"
        );
    }
    s.write("here/skills/geas/notes.md", "mine\n");
    s.write("here/skills/geas/SKILL.md", "changed\n");
    let (_, err, code) = run(s.path(), &["skill", "--install", "here/skills"], &[]);
    assert_eq!(code, 2);
    assert!(err.starts_with("error[E081]: here/skills/geas is already there"), "{err}");
    assert_eq!(s.read("here/skills/geas/SKILL.md"), "changed\n", "a refused install wrote over the folder");
    let (_, err, code) = run(s.path(), &["skill", "--install", "here/skills", "--force"], &[]);
    assert_eq!((err.as_str(), code), ("", 0));
    assert_eq!(s.read("here/skills/geas/SKILL.md"), repo_file("../../skills/geas/SKILL.md"));
    assert_eq!(s.read("here/skills/geas/notes.md"), "mine\n");
    // what the command does not take
    let (_, err, code) = run(s.path(), &["skill", "--force"], &[]);
    assert_eq!(code, 2);
    assert!(err.starts_with("error[E080]: `--force` goes with `--install <dir>`"), "{err}");
    let (_, err, code) = run(s.path(), &["skill", "x.geas"], &[]);
    assert_eq!(code, 2);
    assert!(err.starts_with("error[E080]: `geas skill` takes no file"), "{err}");
}

#[test]
fn codes_md_is_what_sync_writes() {
    let s = Scratch::new("skill-sync");
    let out = Command::new("sh")
        .arg(root().join("skills/sync.sh"))
        .arg(s.path())
        .env("GEAS", geas())
        .output()
        .expect("run sh");
    assert!(out.status.success(), "skills/sync.sh failed: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(names(s.path()), ["codes.md"]);
    assert!(
        s.read("codes.md") == repo_file("../../skills/geas/codes.md"),
        "../../skills/geas/codes.md is not what `geas explain --all` prints now; run `cargo build` and `skills/sync.sh`"
    );
}

/// The targets of the Markdown links in a text, outside code.
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
        for target in links(&std::fs::read_to_string(skill().join(&name)).expect("a page")) {
            seen += 1;
            if target.starts_with("https://") || target.starts_with("http://") || target.starts_with('#') {
                continue;
            }
            let file = target.split('#').next().expect("a path");
            if file.contains('/') || !skill().join(file).is_file() {
                wrong.push(format!("{name}: {target}"));
            }
        }
    }
    assert!(seen > 10, "hardly any links in skills/geas: {seen}");
    assert!(wrong.is_empty(), "links that leave skills/geas, or lead nowhere in it:\n{}", wrong.join("\n"));
}

#[test]
fn skill_md_says_what_it_is() {
    let text = repo_file("../../skills/geas/SKILL.md");
    let front = text
        .strip_prefix("---\n")
        .and_then(|t| t.split_once("\n---\n"))
        .map(|(f, _)| f)
        .expect("SKILL.md starts with its frontmatter");
    let field = |k: &str| front.lines().find_map(|l| l.strip_prefix(&format!("{k}: ")).map(str::to_string));
    assert_eq!(field("name").as_deref(), Some("geas"), "the name is the directory's");
    for (k, most) in [("description", 1024), ("compatibility", 500)] {
        let v = field(k).unwrap_or_else(|| panic!("SKILL.md has no {k}"));
        assert!(!v.is_empty() && v.chars().count() <= most, "the {k} has {} characters; 1 to {most}", v.chars().count());
        // a plain YAML scalar ends a key at ": ", so a value must not hold one
        assert!(!v.contains(": ") && !v.starts_with(['"', '\'', '>', '|', '`']), "the {k} must stay a plain YAML scalar");
    }
    for line in front.lines() {
        let key = line.split(':').next().unwrap_or("");
        assert!(["name", "description", "compatibility", "license", "metadata", "allowed-tools"].contains(&key), "SKILL.md's frontmatter has a field the format does not know: {key}");
    }
    // the repository's own license, as Cargo.toml names it
    assert_eq!(field("license").as_deref(), Some("MIT OR Apache-2.0"), "SKILL.md names the repository's license");
    assert!(repo_file("Cargo.toml").contains("license = \"MIT OR Apache-2.0\""), "Cargo.toml names the same license");
}

/// The driver `gui.md` gives, with the claims it gives for it, holds.
#[test]
fn the_driver_in_gui_md_runs() {
    if !python3("the driver in skills/geas/gui.md") {
        return;
    }
    let page = repo_file("../../skills/geas/gui.md");
    let block = |info: &str, holding: &str| -> String {
        let mut found = None;
        let mut open: Option<(String, Vec<&str>)> = None;
        for line in page.lines() {
            match (line.strip_prefix("```"), open.take()) {
                (Some(_), Some((i, lines))) => {
                    let text = lines.join("\n") + "\n";
                    if i == info && text.contains(holding) {
                        found = Some(text);
                    }
                }
                (Some(i), None) => open = Some((i.to_string(), Vec::new())),
                (None, Some((i, mut lines))) => {
                    lines.push(line);
                    open = Some((i, lines));
                }
                (None, None) => {}
            }
        }
        found.unwrap_or_else(|| panic!("no {info} block holding {holding:?} in gui.md"))
    };
    let s = Scratch::new("skill-driver");
    s.write("counter_driver.py", &block("python", "def screen()"));
    s.write("counter.geas", &block("geas", "driver \"python3 counter_driver.py\""));
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "counter.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.ends_with("1 claim · 1 ok · 0 failed · journal: .geas/counter.journal.jsonl\n"), "{out}");
    assert_eq!(no_process_left(&log), 1, "one driver for the one claim");
}
