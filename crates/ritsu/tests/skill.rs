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
//!
//! It holds the nine skills of `skills/` together too, and the ways they are handed out
//! (PLAN F.3):
//!
//! - `skills/` holds a folder for ritsu and one for each language, and no other, each with a
//!   `SKILL.md` whose frontmatter names the folder, says in at most 1,024 characters when to use it,
//!   and names the repository's license;
//! - the marketplace of Claude Code is one file the site publishes (website/docs/marketplace.json),
//!   not the repository; it says what it must, in the version of the workspace, and its plugin is
//!   the folder skills/, fetched alone, with the nine skills and nothing else Claude Code would
//!   load; the pages that say how to install it add it at the URL the site publishes it at;
//! - the binary carries every file of the nine folders, `ritsu skills install` writes them as they
//!   are, refuses a file changed by hand unless `--force` is given, and writes only the skills
//!   named; `ritsu skills list` names the nine;
//! - the zip of a release (`packaging/skills.sh`) holds the same files;
//! - `skills/README.md` and `skills/README.ja.md` show what `ritsu skills list` prints, and their
//!   links lead somewhere.

mod common;

use common::{Page, commands_named, link_targets, lines_in_order, page, root, run_the_consoles, shown_lines, trimmed};
use ritsu::cli::LANGUAGES;
use ritsu::skills::SKILLS;
use ritsu_testkit::TempDir;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        let at = format!("skills/{name}/");
        assert!(p.text.contains(&format!("`{at}`")), "SKILL.md does not give `{at}`");
        // The skill of a language is that language's own, by name.
        let md = root().join(&at).join("SKILL.md");
        let front = frontmatter(&fs::read_to_string(&md).unwrap_or_else(|e| panic!("{at}SKILL.md: {e}")));
        assert_eq!(field(&front, "name").as_deref(), Some(name), "{at}SKILL.md is the skill of {name}");
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

// ---- the nine skills, and how they are handed out

/// A value of `[workspace.package]` in the root Cargo.toml: `version`, `license`, `repository`.
fn workspace(key: &str) -> String {
    let cargo = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    cargo.lines().find_map(|l| l.strip_prefix(&format!("{key} = \""))).and_then(|l| l.strip_suffix('"')).unwrap_or_else(|| panic!("the workspace names its {key}")).to_string()
}

/// The names of the nine skills: ritsu's, then the languages in the order `ritsu --help` lists them.
fn nine() -> Vec<&'static str> {
    let mut names = vec!["ritsu"];
    names.extend(LANGUAGES);
    names
}

/// The files of `skills/<name>/`, by name, sorted.
fn files_of(name: &str) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(root().join("skills").join(name)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    out.sort();
    out
}

fn json(rel: &str) -> Value {
    let text = fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

#[test]
fn skills_holds_the_nine_each_with_its_frontmatter() {
    let mut dirs: Vec<String> = fs::read_dir(root().join("skills")).unwrap().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    dirs.sort();
    let mut want = nine();
    want.sort();
    assert_eq!(dirs, want, "skills/ holds a folder for ritsu and one for each language, and no other");
    let license = workspace("license");
    for name in nine() {
        let md = root().join("skills").join(name).join("SKILL.md");
        let front = frontmatter(&fs::read_to_string(&md).unwrap_or_else(|e| panic!("{}: {e}", md.display())));
        assert_eq!(field(&front, "name").as_deref(), Some(name), "skills/{name}/SKILL.md names its folder");
        let d = field(&front, "description").unwrap_or_else(|| panic!("skills/{name}/SKILL.md has no description"));
        assert!(!d.is_empty() && d.chars().count() <= 1024, "skills/{name}: the description has {} characters; 1 to 1,024", d.chars().count());
        assert_eq!(field(&front, "license").as_deref(), Some(license.as_str()), "skills/{name}/SKILL.md names the repository's license");
    }
}

/// The URL ritsu's site is published at (`site_url` in website/zensical.toml), ending in `/`.
fn site_url() -> String {
    let toml = fs::read_to_string(root().join("website/zensical.toml")).unwrap();
    toml.lines().find_map(|l| l.strip_prefix("site_url = \"")).and_then(|l| l.strip_suffix('"')).expect("website/zensical.toml names its site_url").to_string()
}

#[test]
fn the_marketplace_on_the_site_hands_out_the_folder_of_skills_alone() {
    // Were the repository the marketplace, adding it would clone all of ritsu, and installing would
    // copy all of it again, to read the nine folders of skills/. The marketplace is one file the
    // site publishes instead, and the plugin it lists is skills/, which Claude Code fetches alone.
    assert!(!root().join(".claude-plugin").exists(), ".claude-plugin/ makes the repository a marketplace, the adding of which clones all of it; the marketplace is website/docs/marketplace.json");
    let market = json("website/docs/marketplace.json");
    assert_eq!(market["name"], "ritsu");
    assert_eq!(market["owner"]["name"], "i2y");
    assert!(market["owner"].get("email").is_none(), "the marketplace names no address");
    assert!(market["description"].as_str().is_some_and(|d| !d.is_empty()), "the marketplace says what it is");
    let plugins = market["plugins"].as_array().expect("the marketplace lists its plugins");
    assert_eq!(plugins.len(), 1, "one plugin, ritsu");
    let plugin = &plugins[0];
    assert_eq!(plugin["name"], "ritsu", "`/plugin install ritsu@ritsu` names it");
    assert_eq!(plugin["version"].as_str(), Some(workspace("version").as_str()), "the plugin takes the version of the workspace");
    assert!(plugin["description"].as_str().is_some_and(|d| !d.is_empty()), "the plugin says what it is");
    assert_eq!(plugin["author"]["name"], "i2y");
    assert!(plugin["author"].get("email").is_none(), "the plugin names no address");
    assert_eq!(plugin["license"].as_str(), Some(workspace("license").as_str()));
    let repository = workspace("repository");
    assert_eq!(plugin["repository"].as_str(), Some(repository.as_str()));
    assert_eq!(plugin["homepage"].as_str(), Some(site_url().as_str()), "the plugin's homepage is ritsu's site");
    // Claude Code checks out skills/ alone, with a sparse checkout, over HTTPS (a GitHub `owner/repo`
    // would have it try SSH first, which fails where there is no key).
    let source = &plugin["source"];
    assert_eq!(source["source"], "git-subdir");
    assert_eq!(source["url"].as_str(), Some(format!("{repository}.git").as_str()));
    assert_eq!(source["path"], "skills");
    assert!(source.get("ref").is_none() && source.get("sha").is_none(), "the plugin follows the default branch, and its version decides when an install takes a new copy");
    // skills/ holds no plugin.json, so the entry is the manifest: the skills are the folders at the
    // root of skills/, and the plugin is nothing else.
    assert_eq!(plugin["skills"], serde_json::json!(["./"]), "the skills are the folders at the root of skills/");
    for key in ["commands", "agents", "hooks", "mcpServers", "lspServers", "outputStyles", "workflows"] {
        assert!(plugin.get(key).is_none(), "the entry sets `{key}`: the plugin is the folders of skills/ alone");
    }
    for part in [".claude-plugin", "commands", "agents", "hooks", "bin", "skills", "output-styles", "workflows", "themes", "monitors", ".mcp.json", ".lsp.json", "settings.json", "SKILL.md", "CLAUDE.md"] {
        assert!(!root().join("skills").join(part).exists(), "skills/{part} would be a part of the plugin `ritsu` too");
    }

    // The pages that say how to install it add the marketplace at the URL the site publishes it at:
    // the READMEs and the top pages of the site say so, and no page of the repository adds it any
    // other way (the notes of DESIGN.md and PLAN.md record what it was).
    let add = format!("/plugin marketplace add {}marketplace.json", site_url());
    for page in ["README.md", "README.ja.md", "skills/README.md", "skills/README.ja.md", "website/docs/index.md", "website/docs-ja/index.md"] {
        let text = fs::read_to_string(root().join(page)).unwrap();
        assert!(text.contains(&add), "{page} does not say `{add}`");
    }
    // Every Markdown page under the root, leaving out what tools put there.
    fn markdown(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !["target", "node_modules", ".venv", "build", ".cache", ".lake", ".git"].contains(&name.as_str()) {
                    markdown(base, &p, out);
                }
            } else if name.ends_with(".md") {
                out.push(p.strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut pages = Vec::new();
    markdown(&root(), &root(), &mut pages);
    let mut wrong = Vec::new();
    for page in pages.iter().filter(|p| !p.ends_with("DESIGN.md") && !p.ends_with("PLAN.md")) {
        let Ok(text) = fs::read_to_string(root().join(page)) else { continue };
        for (i, _) in text.match_indices("/plugin marketplace add ") {
            if !text[i..].starts_with(&add) {
                wrong.push(format!("{page}: `{}`", text[i..].lines().next().unwrap_or_default()));
            }
        }
    }
    assert!(wrong.is_empty(), "these add the marketplace some other way than `{add}`:\n{}", wrong.join("\n"));
}

#[test]
fn the_binary_carries_every_file_of_the_nine() {
    assert_eq!(SKILLS.iter().map(|s| s.name).collect::<Vec<_>>(), nine(), "the skills ritsu carries, in order");
    for s in SKILLS {
        assert_eq!(s.files[0].0, "SKILL.md", "{}: SKILL.md first", s.name);
        let mut carried: Vec<String> = s.files.iter().map(|(n, _)| n.to_string()).collect();
        carried.sort();
        assert_eq!(carried, files_of(s.name), "src/skills.rs carries every file of skills/{}/, and no other", s.name);
        for (file, text) in s.files {
            let on_disk = fs::read(root().join("skills").join(s.name).join(file)).unwrap();
            assert!(on_disk == text.as_bytes(), "skills/{}/{file}: the binary carries another text (built before it changed?)", s.name);
        }
    }
}

/// `ritsu …` run in `dir`, with `HOME` set to `home`: the exit code, stdout and stderr.
fn ritsu_in(dir: &Path, home: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).args(args).current_dir(dir).env("HOME", home).env_remove("RITSU_LANG").output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every file under `dir`, as a path from it, sorted.
fn every_file(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    if dir.exists() {
        walk(dir, dir, &mut out);
    }
    out.sort();
    out
}

/// The files of the skills named, as `ritsu skills install` writes them under its directory.
fn files_of_skills(names: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = names.iter().flat_map(|n| files_of(n).into_iter().map(move |f| format!("{n}/{f}"))).collect();
    out.sort();
    out
}

/// Each file written under `dir` is the file of `skills/`, to the byte.
fn same_as_skills(dir: &Path) {
    for f in every_file(dir) {
        assert!(fs::read(dir.join(&f)).unwrap() == fs::read(root().join("skills").join(&f)).unwrap(), "{f} is not skills/{f}");
    }
}

#[test]
fn ritsu_skills_install_writes_the_nine_as_they_are() {
    let t = TempDir::new("skills-install");
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["skills", "install"]);
    assert_eq!(code, 0, "{out}{err}");
    let dir = t.path().join(".claude/skills");
    assert_eq!(every_file(&dir), files_of_skills(&nine()), "the project's .claude/skills/ holds the nine, and nothing else");
    same_as_skills(&dir);
    assert_eq!(out.lines().count(), 9, "{out}");
    assert!(out.starts_with("wrote .claude/skills/ritsu (1 file)\nwrote .claude/skills/rulec ("), "{out}");
    // Again: every file is already the same, and nothing is written.
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["skills", "install"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.lines().all(|l| l.contains(" is already the same (")), "{out}");
}

#[test]
fn ritsu_skills_install_refuses_a_file_changed_by_hand_and_force_writes_over_it() {
    let t = TempDir::new("skills-by-hand");
    assert_eq!(ritsu_in(t.path(), t.path(), &["skills", "install"]).0, 0);
    let dir = t.path().join(".claude/skills");
    let changed = dir.join("rulec/SKILL.md");
    let mut text = fs::read_to_string(&changed).unwrap();
    text.push_str("\nA line of the project's own.\n");
    fs::write(&changed, &text).unwrap();
    let mine = t.write(".claude/skills/rulec/notes.md", "the project's notes\n");
    fs::remove_dir_all(dir.join("dandori")).unwrap();
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["skills", "install"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.is_empty(), "{out}");
    assert!(err.starts_with("error: 1 file there differs from the one this ritsu carries") && err.contains("`--force` writes over it") && err.contains("\n  .claude/skills/rulec/SKILL.md\n"), "{err}");
    assert_eq!(fs::read_to_string(&changed).unwrap(), text, "the file changed by hand is left as it is");
    assert!(!dir.join("dandori").exists(), "nothing is written when a file is refused");
    // With --force, the file is ritsu's again, the missing skill is written, and the project's own
    // file is left as it is.
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["skills", "install", "--force"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("wrote .claude/skills/rulec (1 file)") && out.contains("wrote .claude/skills/dandori ("), "{out}");
    assert_eq!(fs::read_to_string(&mine).unwrap(), "the project's notes\n");
    fs::remove_file(&mine).unwrap();
    assert_eq!(every_file(&dir), files_of_skills(&nine()));
    same_as_skills(&dir);
}

#[test]
fn ritsu_skills_install_writes_only_the_skills_named_where_it_is_told() {
    let t = TempDir::new("skills-named");
    let home = t.path().join("home");
    fs::create_dir_all(&home).unwrap();
    // --dir, for another agent
    let (code, out, err) = ritsu_in(t.path(), &home, &["skills", "install", "rulec", "dandori", "--dir", "agent/skills"]);
    assert_eq!(code, 0, "{out}{err}");
    let dir = t.path().join("agent/skills");
    assert_eq!(every_file(&dir), files_of_skills(&["rulec", "dandori"]));
    same_as_skills(&dir);
    assert!(!t.path().join(".claude").exists(), "--dir writes nowhere else");
    // --user, into ~/.claude/skills
    let (code, out, err) = ritsu_in(t.path(), &home, &["skills", "install", "geas", "--user"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(every_file(&home.join(".claude/skills")), files_of_skills(&["geas"]));
    same_as_skills(&home.join(".claude/skills"));
    // a skill that does not exist, and the two places at once, are bad arguments that write nothing
    let (code, _, err) = ritsu_in(t.path(), &home, &["skills", "install", "rulec", "nope"]);
    assert_eq!(code, 2);
    assert_eq!(err, "error: there is no skill `nope`; the skills are ritsu, rulec, dandori, koyomi, chobo, geas, yuen, sakai, sekisho\n");
    let (code, _, err) = ritsu_in(t.path(), &home, &["skills", "install", "--dir", "x", "--user"]);
    assert_eq!((code, err.as_str()), (2, "error: give `--dir` or `--user`, not both\n"));
    assert!(!t.path().join(".claude").exists() && !t.path().join("x").exists());
}

#[test]
fn ritsu_skills_list_names_the_nine() {
    let t = TempDir::new("skills-list");
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["skills", "list"]);
    assert_eq!(code, 0, "{err}");
    let names: Vec<&str> = out.lines().filter_map(|l| l.split_whitespace().next()).collect();
    assert_eq!(names, nine(), "{out}");
    let (code, out, _) = ritsu_in(t.path(), t.path(), &["skills", "list", "--lang", "ja"]);
    assert!(code == 0 && out.contains("rulec    業務の規則（.rule）"), "{out}");
    // `skills` is in the table, and so in `ritsu --help`
    let (code, out, _) = ritsu_in(t.path(), t.path(), &["--help"]);
    assert!(code == 0 && out.contains("ritsu skills list | install [<name>...]"), "{out}");
    let (code, out, _) = ritsu_in(t.path(), t.path(), &["skills"]);
    assert!(code == 2 && out.is_empty());
}

#[test]
fn the_release_zip_holds_the_nine_as_they_are() {
    let t = TempDir::new("skills-zip");
    let dist = t.path().join("dist");
    let o = Command::new("sh").args(["packaging/skills.sh", "v0.0.0", dist.to_str().unwrap()]).current_dir(root()).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{out}{}", String::from_utf8_lossy(&o.stderr));
    let zip = dist.join("ritsu-skills-v0.0.0.zip");
    assert_eq!(out.trim_end(), zip.to_str().unwrap(), "the script says what it wrote");
    let into = t.path().join("unzipped");
    let st = Command::new("unzip").args(["-q", zip.to_str().unwrap(), "-d", into.to_str().unwrap()]).status().expect("unzip");
    assert!(st.success());
    let mut want = files_of_skills(&nine());
    want.extend(["LICENSE-APACHE".to_string(), "LICENSE-MIT".to_string()]);
    want.sort();
    assert_eq!(every_file(&into), want, "the zip holds the nine folders at its top, and the two licenses");
    for f in every_file(&into) {
        let from = if f.starts_with("LICENSE-") { root().join(&f) } else { root().join("skills").join(&f) };
        assert!(fs::read(into.join(&f)).unwrap() == fs::read(from).unwrap(), "{f} in the zip is not the repository's");
    }
}

#[test]
fn the_readmes_of_the_skills_show_what_ritsu_prints() {
    let pages: Vec<Page> = ["skills/README.md", "skills/README.ja.md"].iter().map(|n| page(n)).collect();
    let (ran, wrong) = run_the_consoles(&pages);
    assert!(ran >= 2, "{ran} commands run: `ritsu skills list` is shown with what it prints, on each page");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let mut bad = Vec::new();
    for p in &pages {
        for (line, target) in link_targets(&p.text) {
            if target.starts_with("https://") || target.starts_with("http://") {
                continue;
            }
            let (file, frag) = target.split_once('#').map_or((target.as_str(), None), |(f, a)| (f, Some(a)));
            let to = root().join("skills").join(file);
            if !to.exists() {
                bad.push(format!("{}:{line}: {target} leads nowhere", p.name));
            } else if let (Some(frag), true) = (frag, file.ends_with(".md")) {
                let q = page(to.strip_prefix(root()).unwrap().to_str().unwrap());
                if !q.anchors.iter().any(|a| a == frag) {
                    bad.push(format!("{}:{line}: {file} has no heading anchored `#{frag}`", p.name));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
