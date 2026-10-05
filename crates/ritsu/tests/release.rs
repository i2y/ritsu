//! What a release hands out (DESIGN 13.2): the archive with `ritsu`, its seven links, the two
//! licenses and THIRD_PARTY_NOTICES, the formula, the packages, the zip of the skills, the action,
//! and the workflow that makes them; and THIRD_PARTY_NOTICES itself, held to `cargo tree` and to
//! the texts it quotes. The scripts of `packaging/`
//! are run on the binary of this crate; the files that say the same thing in five places — the
//! seven names, the four platforms — are held to one another here, as the tests of the sites hold
//! the pages to the commands.

use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The seven languages, as `cli.rs` of this crate lists them: the names of the links.
const LANGUAGES: [&str; 7] = ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai"];

/// The four platforms a release is built for.
const TARGETS: [&str; 4] = ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl", "x86_64-apple-darwin", "aarch64-apple-darwin"];

/// The files of the repository that go with the binary wherever it goes: its two licenses, and the
/// notices and licenses of what it holds from others.
const DOCS: [&str; 3] = ["LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn read(rel: &str) -> String {
    let p = root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn sh(args: &[&str]) -> (i32, String, String) {
    let o = Command::new("sh").args(args).current_dir(root()).output().expect("could not run sh");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The archive `packaging/archive.sh` writes for the binary of this crate, unpacked: its files, and
/// the directory they are in.
fn unpacked(t: &TempDir) -> (Vec<String>, PathBuf) {
    let dist = t.path().join("dist");
    let (code, out, err) = sh(&["packaging/archive.sh", "v0.0.0", "test-target", env!("CARGO_BIN_EXE_ritsu"), dist.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    let archive = dist.join("ritsu-v0.0.0-test-target.tar.gz");
    assert_eq!(out.trim_end(), archive.to_str().unwrap(), "the script says what it wrote");
    let listed = Command::new("tar").args(["-tzf", archive.to_str().unwrap()]).output().unwrap();
    assert!(listed.status.success());
    let mut names: Vec<String> = String::from_utf8_lossy(&listed.stdout).lines().map(|l| l.trim_end_matches('/').to_string()).collect();
    names.sort();
    let into = t.path().join("unpacked");
    std::fs::create_dir_all(&into).unwrap();
    let st = Command::new("tar").args(["-xzf", archive.to_str().unwrap(), "-C", into.to_str().unwrap()]).status().unwrap();
    assert!(st.success());
    (names, into)
}

/// The archive is flat: `ritsu`, beside it a link for each of the seven languages, each relative
/// (`rulec -> ritsu`), so that unpacking it into a directory on the PATH is the install, and the two
/// licenses and THIRD_PARTY_NOTICES, the files of the repository.
#[test]
fn the_archive_holds_ritsu_a_link_for_each_language_and_the_licenses() {
    let t = TempDir::new("release-archive");
    let (names, dir) = unpacked(&t);
    let mut want: Vec<&str> = LANGUAGES.to_vec();
    want.push("ritsu");
    want.extend(DOCS);
    want.sort();
    assert_eq!(names, want);
    assert!(std::fs::symlink_metadata(dir.join("ritsu")).unwrap().file_type().is_file());
    for doc in DOCS {
        assert!(std::fs::symlink_metadata(dir.join(doc)).unwrap().file_type().is_file(), "{doc} is a file");
        assert_eq!(std::fs::read(dir.join(doc)).unwrap(), std::fs::read(root().join(doc)).unwrap(), "{doc} is the repository's");
    }
    for l in LANGUAGES {
        let p = dir.join(l);
        assert!(std::fs::symlink_metadata(&p).unwrap().file_type().is_symlink(), "{l} is a link");
        assert_eq!(std::fs::read_link(&p).unwrap(), Path::new("ritsu"), "{l} points at ritsu, beside it");
    }
    // unpacked, it runs: each name says its own, and the one binary is what they all reach
    for n in LANGUAGES.iter().chain(["ritsu"].iter()) {
        let out = Command::new(dir.join(n)).arg("--version").output().unwrap();
        assert!(out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with(&format!("{n} ")), "{n}");
    }
}

/// `packaging/smoke.sh`, which release.yml runs on the unpacked archive and linux.sh on each
/// package, runs every one of the seven names on an example of its language, and `ritsu check` on a
/// project of them all.
#[test]
fn the_smoke_run_reads_an_example_under_every_name() {
    let t = TempDir::new("release-smoke");
    let (_, dir) = unpacked(&t);
    let (code, out, err) = sh(&["packaging/smoke.sh", dir.to_str().unwrap(), root().join("crates").to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    let ok: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("ok   ")).filter_map(|l| l.split(' ').next()).collect();
    let mut want: Vec<&str> = LANGUAGES.to_vec();
    want.push("ritsu");
    want.sort();
    let mut got = ok.clone();
    got.sort();
    assert_eq!(got, want, "{out}");
    assert!(out.lines().any(|l| l.starts_with("ok   ritsu check ")), "{out}");
    // a directory with no links is a failure that names the first thing that did not run
    let empty = TempDir::new("release-smoke-empty");
    let (code, _, err) = sh(&["packaging/smoke.sh", empty.path().to_str().unwrap(), root().join("crates").to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(err.starts_with("FAIL rulec check "), "{err}");
}

/// The formula is written from the sums published with the release: one url and one sum for each
/// of the four platforms, the install of the binary and the seven links, a test that holds every
/// name to the version.
#[test]
fn the_formula_is_written_from_the_sums() {
    let t = TempDir::new("release-formula");
    let sums: String = TARGETS.iter().enumerate().map(|(i, target)| format!("{}  ritsu-v9.9.9-{target}.tar.gz\n", char::from(b'a' + i as u8).to_string().repeat(64))).collect();
    let file = t.write("SHA256SUMS", sums);
    let (code, formula, err) = sh(&["packaging/homebrew.sh", "v9.9.9", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(formula.starts_with("# Written by packaging/homebrew.sh in i2y/ritsu for v9.9.9"), "{formula}");
    assert!(formula.contains("class Ritsu < Formula") && formula.contains("  license all_of: [\n    { any_of: [\"MIT\", \"Apache-2.0\"] },\n"), "{formula}");
    for (i, target) in TARGETS.iter().enumerate() {
        let sum = char::from(b'a' + i as u8).to_string().repeat(64);
        let url = format!("url \"https://github.com/i2y/ritsu/releases/download/v9.9.9/ritsu-v9.9.9-{target}.tar.gz\"\n      sha256 \"{sum}\"");
        assert!(formula.contains(&url), "{target}: {formula}");
    }
    assert!(formula.contains("bin.install \"ritsu\""), "{formula}");
    assert!(formula.contains(&format!("%w[{}].each do |language|\n      bin.install_symlink \"ritsu\" => language", LANGUAGES.join(" "))), "{formula}");
    assert!(formula.contains("assert_equal \"#{language} #{version}\", shell_output(\"#{bin}/#{language} --version\").strip"), "{formula}");
    // a sum missing from the file is not guessed
    let some = t.write("SOME", format!("{}  ritsu-v9.9.9-{}.tar.gz\n", "a".repeat(64), TARGETS[0]));
    let (code, out, err) = sh(&["packaging/homebrew.sh", "v9.9.9", some.to_str().unwrap()]);
    assert!(code != 0 && out.is_empty() && err.contains("has no line for ritsu-v9.9.9-"), "{out}{err}");
}

/// What the script printed is Ruby brew can read.
#[test]
fn the_formula_is_ruby() {
    if !ready(Need::Ruby, || ritsu_testkit::tools::runs("ruby", &["--version"]), "no ruby to read the formula with") {
        return;
    }
    let t = TempDir::new("release-formula-ruby");
    let sums: String = TARGETS.iter().map(|target| format!("{}  ritsu-v9.9.9-{target}.tar.gz\n", "f".repeat(64))).collect();
    let file = t.write("SHA256SUMS", sums);
    let (code, formula, err) = sh(&["packaging/homebrew.sh", "v9.9.9", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    let rb = t.write("ritsu.rb", formula);
    let o = Command::new("ruby").args(["-c", rb.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// The seven names are said in the script of the archive, the formula, the packages, the smoke
/// run, the workflow and `cli.rs`; the four platforms in the workflow, the action, the formula
/// and the packages. One test holds them to the list above, so a language added in one place and
/// not another fails here.
#[test]
fn the_names_and_the_platforms_are_the_same_in_every_file() {
    let names = LANGUAGES.join(" ");
    // cli.rs, which makes ritsu answer to the names
    let cli = read("crates/ritsu/src/cli.rs");
    let quoted: Vec<String> = LANGUAGES.iter().map(|l| format!("\"{l}\"")).collect();
    assert!(cli.contains(&format!("pub const LANGUAGES: [&str; 7] = [{}];", quoted.join(", "))), "cli.rs");
    // the archive
    assert!(read("packaging/archive.sh").contains(&format!("languages=\"{names}\"")), "archive.sh");
    // the formula: the install and the test
    let formula = read("packaging/homebrew.sh");
    assert_eq!(formula.matches(&format!("%w[{names}]")).count(), 2, "homebrew.sh");
    // the packages: a link for each, and no other file in /usr/bin
    let nfpm = read("packaging/nfpm.yaml");
    let dsts: Vec<&str> = nfpm.lines().filter_map(|l| l.trim().strip_prefix("dst: /usr/bin/")).collect();
    let mut want: Vec<&str> = LANGUAGES.to_vec();
    want.push("ritsu");
    assert_eq!(dsts.len(), 8, "nfpm.yaml: {dsts:?}");
    for n in &want {
        assert!(dsts.contains(n), "nfpm.yaml has no /usr/bin/{n}");
    }
    assert_eq!(nfpm.matches("type: symlink").count(), 7, "nfpm.yaml: seven links");
    // the smoke run: one step for each name, and `ritsu check`
    let smoke = read("packaging/smoke.sh");
    for l in LANGUAGES {
        assert!(smoke.lines().any(|s| s.starts_with(&format!("step {l} "))), "smoke.sh has no step for {l}");
    }
    assert!(smoke.lines().any(|s| s.starts_with("step ritsu check ")), "smoke.sh has no step for ritsu check");
    // the workflow and the package run: every name is asked its version, and every package is
    // removed with all eight files
    let all = format!("ritsu {names}");
    assert_eq!(read(".github/workflows/release.yml").matches(&format!("for n in {all}; do")).count(), 2, "release.yml");
    assert_eq!(read("packaging/linux.sh").matches(&format!("for n in {all}; do")).count(), 2, "linux.sh");
    // DESIGN 2.3 lists them in this order too
    assert!(read("DESIGN.md").contains(&format!("`{}`", LANGUAGES.join("`、`"))), "DESIGN 2.3 lists the seven names");
    // the four platforms
    for (file, text) in [
        ("release.yml", read(".github/workflows/release.yml")),
        ("action.yml", read("action.yml")),
        ("homebrew.sh", formula),
        ("linux.sh", read("packaging/linux.sh")),
    ] {
        let wanted: &[&str] = if file == "linux.sh" { &TARGETS[..2] } else { &TARGETS };
        for target in wanted {
            assert!(text.contains(target), "{file} does not name {target}");
        }
        if file == "linux.sh" {
            for target in &TARGETS[2..] {
                assert!(!text.contains(target), "{file} builds packages for Linux only: {target}");
            }
        }
    }
}

/// The two licenses and THIRD_PARTY_NOTICES go with the binary everywhere a release puts it: in the
/// archive (above), in /usr/share/doc/ritsu of the .deb and the .rpm, whose lists of files linux.sh
/// reads before it installs them, and in the keg of the formula, whose test looks for them there
/// (brew moves LICENSE-MIT and LICENSE-APACHE into the keg by their names; the formula moves
/// THIRD_PARTY_NOTICES beside them).
#[test]
fn the_licenses_and_the_notices_go_with_the_binary_in_every_package() {
    let names = DOCS.join(" ");
    assert!(read("packaging/archive.sh").contains(&format!("licenses=\"{names}\"")), "archive.sh");
    let nfpm = read("packaging/nfpm.yaml");
    for doc in DOCS {
        assert!(nfpm.contains(&format!("  - src: {doc}\n    dst: /usr/share/doc/ritsu/{doc}\n    file_info:\n      mode: 0644\n")), "nfpm.yaml does not put {doc} in /usr/share/doc/ritsu");
        assert!(root().join(doc).is_file(), "{doc} is not at the root");
    }
    assert_eq!(nfpm.matches("dst: /usr/share/doc/ritsu/").count(), DOCS.len(), "nfpm.yaml: /usr/share/doc/ritsu holds these and nothing else");
    let linux = read("packaging/linux.sh");
    assert_eq!(linux.matches(&format!("for f in {names}; do")).count(), 2, "linux.sh reads both packages' lists of files");
    assert!(linux.contains("dpkg-deb -c \"/out/$1\" | grep -q \" ./usr/share/doc/ritsu/$f\\$\""), "linux.sh: the .deb");
    assert!(linux.contains("rpm -qlp \"/out/$1\" | grep -qx \"/usr/share/doc/ritsu/$f\""), "linux.sh: the .rpm");
    let formula = read("packaging/homebrew.sh");
    assert!(formula.contains("    prefix.install \"THIRD_PARTY_NOTICES\"\n"), "the formula does not put THIRD_PARTY_NOTICES in the keg");
    assert!(formula.contains(&format!("%w[{names}].each do |file|\n      assert_path_exists prefix/file\n")), "the formula's test does not look for the three files");
}

/// The packages take the place of the `rulec` package of the releases before ritsu, so a machine
/// that has it installs ritsu over it and keeps a working `/usr/bin/rulec` (DESIGN 13.2).
#[test]
fn the_packages_take_the_place_of_the_rulec_package() {
    let nfpm = read("packaging/nfpm.yaml");
    for key in ["replaces", "conflicts", "provides"] {
        assert!(nfpm.contains(&format!("\n{key}:\n  - rulec\n")), "nfpm.yaml does not say `{key}: rulec`");
    }
}

/// Each workflow runs the scripts of `packaging/` that exist, and the action unpacks the archive
/// the script writes, under the name the script gives it.
#[test]
fn the_workflows_run_the_scripts_and_the_action_unpacks_what_they_write() {
    let release = read(".github/workflows/release.yml");
    let packages = read(".github/workflows/packages.yml");
    for (workflow, script) in [
        (&release, "packaging/archive.sh"),
        (&release, "packaging/smoke.sh"),
        (&release, "packaging/linux.sh"),
        (&release, "packaging/homebrew.sh"),
        (&release, "packaging/skills.sh"),
        (&packages, "packaging/archive.sh"),
        (&packages, "packaging/smoke.sh"),
        (&packages, "packaging/linux.sh"),
    ] {
        assert!(workflow.contains(script), "a workflow does not run {script}");
        assert!(root().join(script).is_file(), "{script} is not there");
    }
    // `ritsu-<tag>-<target>.tar.gz`: the script writes it, the action and the formula fetch it
    assert!(read("packaging/archive.sh").contains("ritsu-$tag-$target.tar.gz"));
    assert!(read("action.yml").contains("name=\"ritsu-$ver-$target.tar.gz\""));
    assert!(read("packaging/homebrew.sh").contains("ritsu-$tag-$1.tar.gz"));
    // the SHA256SUMS holds the archive, both packages and the zip of the skills, and the release
    // hands out each of them (tests/skill.rs holds what is in the zip)
    assert!(release.contains("sha256sum ritsu-*.tar.gz ritsu_*.deb ritsu-*.rpm ritsu-skills-*.zip"));
    assert!(release.contains("dist/ritsu-*.tar.gz dist/ritsu_*.deb dist/ritsu-*.rpm dist/ritsu-skills-*.zip dist/SHA256SUMS"));
    assert!(read("packaging/skills.sh").contains("ritsu-skills-$tag.zip"));
    // the packages are named as the script names them, the formula's test and the release's
    // checks hold every name to one version
    let linux = read("packaging/linux.sh");
    assert!(linux.contains("deb=\"ritsu_$version-1_$arch.deb\"") && linux.contains("rpm=\"ritsu-$version-1.$rpmarch.rpm\""));
}

/// Every crate takes the version of the workspace (DESIGN 13.1), and so every name says it, as
/// the release checks the tag against each of them.
#[test]
fn one_version_for_the_workspace_and_every_crate() {
    let top = read("Cargo.toml");
    assert!(top.contains("[workspace.package]") && top.lines().any(|l| l.starts_with("version = \"")), "the workspace has a version");
    let mut crates: Vec<_> = std::fs::read_dir(root().join("crates")).unwrap().flatten().map(|d| d.path()).filter(|p| p.join("Cargo.toml").is_file()).collect();
    crates.sort();
    for dir in crates {
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(manifest.lines().any(|l| l.trim() == "version.workspace = true"), "{} does not take the version of the workspace", dir.display());
    }
    let v = format!("ritsu {}", Command::new(env!("CARGO_BIN_EXE_ritsu")).arg("--version").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap().strip_prefix("ritsu ").unwrap());
    for l in LANGUAGES {
        let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).arg(l).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), v.replacen("ritsu", l, 1), "{l} says another version than ritsu");
    }
}

// ── THIRD_PARTY_NOTICES ────────────────────────────────────────────────────

/// The line above and below the title of each section of THIRD_PARTY_NOTICES.
const RULE: &str = "--------------------------------------------------------------------------------";

/// A section of THIRD_PARTY_NOTICES: its title, its head (the paragraph under the title) and its
/// text (the rest of it, the license as its source gives it), as they are.
struct Notice {
    title: String,
    head: String,
    text: String,
}

impl Notice {
    /// The value of a `Key:` line of the head.
    fn field(&self, key: &str) -> Option<&str> {
        self.head.lines().find_map(|l| l.strip_prefix(&format!("{key}:"))).map(str::trim)
    }
}

/// THIRD_PARTY_NOTICES: the text before the first section (the list of what the binary holds),
/// and the sections.
fn notices() -> (String, Vec<Notice>) {
    let text = read("THIRD_PARTY_NOTICES");
    let rule = format!("\n{RULE}\n");
    let mut parts = text.split(rule.as_str());
    let intro = parts.next().unwrap().to_string();
    let rest: Vec<&str> = parts.collect();
    assert!(!rest.is_empty() && rest.len() % 2 == 0, "every section has its title between two rules");
    let sections = rest
        .chunks(2)
        .map(|pair| {
            let (title, body) = (pair[0], pair[1]);
            let body = body.strip_prefix('\n').unwrap_or_else(|| panic!("{title}: no blank line under the title"));
            let (head, text) = body.split_once("\n\n").unwrap_or_else(|| panic!("{title}: a head, a blank line, then the text"));
            Notice { title: title.to_string(), head: head.to_string(), text: text.to_string() }
        })
        .collect();
    (intro, sections)
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

/// The crates from outside the workspace that `package` is built from, as `cargo tree` gives them
/// (normal dependencies only, for `target`, or the machine's own): name, version and license.
fn built_from(package: &str, target: Option<&str>) -> Vec<(String, String, String)> {
    let mut cmd = cargo();
    cmd.args(["tree", "-p", package, "-e", "normal", "--prefix", "none", "--format", "{p}\t{l}", "--offline"]).current_dir(root());
    if let Some(t) = target {
        cmd.args(["--target", t]);
    }
    let o = cmd.output().expect("could not run cargo tree");
    assert!(o.status.success(), "cargo tree: {}", String::from_utf8_lossy(&o.stderr));
    let mut out: Vec<(String, String, String)> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| {
            let (p, license) = l.trim_end_matches(" (*)").split_once('\t')?;
            let mut words = p.split(' ');
            let (name, version) = (words.next()?, words.next()?.strip_prefix('v')?);
            // a crate of the workspace says where it is: `ritsu-base v0.23.0 (/…/crates/ritsu-base)`
            (words.next().is_none()).then(|| (name.to_string(), version.to_string(), license.to_string()))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// `cargo metadata` of the workspace on this machine's platform: its packages, by name and version.
/// Only this platform's: the six crates of Cargo.lock no target builds (deny.toml) are not
/// downloaded by a build, and `--offline` cannot read what is not there.
fn packages() -> std::collections::BTreeMap<(String, String), serde_json::Value> {
    let v = cargo().arg("-vV").output().expect("could not run cargo -vV");
    let host = String::from_utf8_lossy(&v.stdout).lines().find_map(|l| l.strip_prefix("host: ").map(str::to_string)).expect("cargo -vV says the host");
    let o = cargo().args(["metadata", "--format-version", "1", "--offline", "--filter-platform", &host]).current_dir(root()).output().expect("could not run cargo metadata");
    assert!(o.status.success(), "cargo metadata: {}", String::from_utf8_lossy(&o.stderr));
    let meta: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    meta["packages"].as_array().unwrap().iter().map(|p| ((p["name"].as_str().unwrap().to_string(), p["version"].as_str().unwrap().to_string()), p.clone())).collect()
}

/// The sections of THIRD_PARTY_NOTICES that are crates: `<name> <version>`, with a `Source:` on
/// crates.io.
fn crate_sections(sections: &[Notice]) -> Vec<&Notice> {
    sections.iter().filter(|s| s.field("Source").is_some_and(|u| u.starts_with("https://crates.io/crates/"))).collect()
}

/// THIRD_PARTY_NOTICES names the crates `ritsu` and `ritsu.wasm` are built from, each at the version
/// and under the license `cargo tree` gives, on each of the four platforms and in wasm32; and its
/// list at the top has a line for every section.
#[test]
fn the_notices_name_the_crates_the_binary_is_built_from() {
    let (intro, sections) = notices();
    let host = built_from("ritsu", None);
    assert!(!host.is_empty(), "ritsu is built from no crate outside the workspace?");
    for (package, target) in TARGETS.iter().map(|t| ("ritsu", Some(*t))).chain([("ritsu-wasm", None), ("ritsu-wasm", Some("wasm32-unknown-unknown"))]) {
        assert_eq!(built_from(package, target), host, "{package} for {target:?} is built from other crates than ritsu here");
    }
    let named: Vec<(String, String, String)> = {
        let mut v: Vec<_> = crate_sections(&sections)
            .iter()
            .map(|s| {
                let (name, version) = s.title.split_once(' ').unwrap_or_else(|| panic!("`{}` is not `<name> <version>`", s.title));
                let license = s.field("License").unwrap_or_else(|| panic!("{}: no License", s.title));
                let license = license.strip_suffix(", taken under MIT").unwrap_or(license);
                (name.to_string(), version.to_string(), license.to_string())
            })
            .collect();
        v.sort();
        v
    };
    assert_eq!(named, host, "THIRD_PARTY_NOTICES and `cargo tree -p ritsu -e normal` name other crates, versions or licenses");
    // the list at the top: one line for each section, in the same order, with its license
    let rows: Vec<&str> = intro.lines().filter(|l| l.starts_with("  ")).collect();
    assert_eq!(rows.len(), sections.len(), "the list at the top has a line for each section: {rows:#?}");
    for (row, s) in rows.iter().zip(&sections) {
        assert!(row.starts_with(&format!("  {}  ", s.title)), "the list says `{row}` where the section is `{}`", s.title);
        if let Some(license) = s.field("License") {
            assert!(row.ends_with(license.strip_suffix(", taken under MIT").unwrap_or(license)), "`{row}`: the license of `{}` is {license}", s.title);
        }
    }
}

/// The text of each crate is its own license file, as Cargo keeps the crate, to the byte; its head
/// names where it comes from, its repository and its authors as the crate's Cargo.toml does. Of
/// the licenses a crate offers, ritsu takes MIT, so the file is the crate's LICENSE-MIT.
#[test]
fn the_text_of_each_crate_is_its_own_license_file() {
    let (_, sections) = notices();
    let packages = packages();
    let crates = crate_sections(&sections);
    assert!(!crates.is_empty());
    for s in crates {
        let (name, version) = s.title.split_once(' ').unwrap();
        let p = packages.get(&(name.to_string(), version.to_string())).unwrap_or_else(|| panic!("{name} {version} is not in Cargo.lock"));
        assert_eq!(s.field("Source"), Some(format!("https://crates.io/crates/{name}/{version}").as_str()), "{name}");
        assert_eq!(s.field("Repository"), p["repository"].as_str(), "{name}: the repository its Cargo.toml names");
        let authors: Vec<&str> = p["authors"].as_array().unwrap().iter().filter_map(|a| a.as_str()).collect();
        assert_eq!(s.field("Authors"), (!authors.is_empty()).then(|| authors.join(", ")).as_deref(), "{name}: the authors its Cargo.toml names");
        let license = p["license"].as_str().unwrap();
        let taken = if license == "MIT" { String::new() } else { ", taken under MIT".to_string() };
        assert_eq!(s.field("License"), Some(format!("{license}{taken}").as_str()), "{name}");
        assert!(license.split(" OR ").any(|l| l == "MIT"), "{name} offers no MIT: {license}");
        assert_eq!(s.field("Text"), Some("LICENSE-MIT, as the crate has it"), "{name}");
        let dir = Path::new(p["manifest_path"].as_str().unwrap()).parent().unwrap();
        let file = std::fs::read_to_string(dir.join("LICENSE-MIT")).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(s.text == file, "the text of {name} {version} is not its LICENSE-MIT ({}):\n{}", dir.display(), first_difference(&s.text, &file));
    }
}

/// The line where two texts first differ, for a message.
fn first_difference(a: &str, b: &str) -> String {
    let n = a.lines().zip(b.lines()).take_while(|(x, y)| x == y).count();
    format!("line {}: `{}` against `{}`", n + 1, a.lines().nth(n).unwrap_or("(the end)"), b.lines().nth(n).unwrap_or("(the end)"))
}

/// The sections of CLDR and of the WHATWG table are the texts rulec and koyomi carry for them, as
/// they are, and the one of zmij's upstream is the code zmij's README says it is a port of.
#[test]
fn the_sections_of_the_data_are_the_notices_of_rulec_and_koyomi() {
    let (_, sections) = notices();
    let section = |title: &str| sections.iter().find(|s| s.title == title).unwrap_or_else(|| panic!("THIRD_PARTY_NOTICES has no section `{title}`"));
    let rulec = read("crates/rulec/THIRD_PARTY_NOTICES");
    let rulec = rulec.strip_prefix("Third-party notices for rulec\n=============================\n\n").expect("rulec's notices begin with their title");
    let cldr = section("Unicode CLDR 48.2");
    assert!(cldr.text == rulec, "the section of CLDR is not crates/rulec/THIRD_PARTY_NOTICES: {}", first_difference(&cldr.text, rulec));
    assert!(rulec.contains("Unicode CLDR 48.2 (") && rulec.contains("SPDX-License-Identifier: Unicode-3.0"), "the title says the version rulec's notices give");
    let koyomi = read("crates/koyomi/THIRD_PARTY_NOTICES.md");
    let whatwg_text = koyomi.split_once("\n## The Shift_JIS table, from the WHATWG Encoding Standard\n\n").map(|(_, t)| t).expect("koyomi's notices have the section of the Shift_JIS table");
    assert!(!whatwg_text.contains("\n## "), "the section of the Shift_JIS table is the last of koyomi's notices");
    let whatwg = section("The WHATWG Encoding Standard");
    assert!(whatwg.text == whatwg_text, "the section of the WHATWG table is not koyomi's: {}", first_difference(&whatwg.text, whatwg_text));
    // the code zmij is a port of, at the commit its README names
    let zmij = crate_sections(&sections).into_iter().find(|s| s.title.starts_with("zmij ")).expect("zmij is in THIRD_PARTY_NOTICES");
    let (_, version) = zmij.title.split_once(' ').unwrap();
    let packages = packages();
    let dir = Path::new(packages[&("zmij".to_string(), version.to_string())]["manifest_path"].as_str().unwrap()).parent().unwrap().to_path_buf();
    let readme = std::fs::read_to_string(dir.join("README.md")).unwrap();
    let upstream = section("Żmij, by Victor Zverovich");
    let source = upstream.field("Source").expect("the upstream's section says where it is");
    assert!(readme.contains("line-by-line port of Victor Zverovich's") && readme.contains(&format!("[upstream]: {source}\n")), "zmij {version}'s README names another upstream than {source}");
    assert!(upstream.head.contains(&format!("zmij {version} says")), "{}", upstream.head);
    assert!(upstream.text.starts_with("MIT License\n\nCopyright (c) ") && upstream.field("License") == Some("MIT"), "{}", upstream.text);
}

// ── The license of what a release hands out ────────────────────────────────

/// The `license` of a manifest, as written.
fn license_of(manifest: &str) -> String {
    let text = read(manifest);
    text.lines().find_map(|l| l.strip_prefix("license = \"").and_then(|l| l.strip_suffix('"'))).unwrap_or_else(|| panic!("{manifest} names no license")).to_string()
}

/// An expression of the form `(A OR B) AND C AND D`, or `A OR B`, as its first term and the terms
/// after it.
fn terms(expr: &str) -> (String, Vec<String>) {
    let mut parts = expr.split(" AND ");
    let first = parts.next().unwrap();
    let first = first.strip_prefix('(').and_then(|f| f.strip_suffix(')')).unwrap_or(first).to_string();
    (first, parts.map(str::to_string).collect())
}

/// What a release hands out is under the workspace's license and what the crates of the workspace
/// it is built from hold beyond it (rulec the names from Unicode CLDR, koyomi the table from the
/// WHATWG): ritsu and ritsu-wasm say all of it, in one order, and so do the packages, the formula
/// (in Homebrew's words) and THIRD_PARTY_NOTICES. A crate of the workspace says only what it
/// holds itself; the crates from outside keep their own (THIRD_PARTY_NOTICES).
#[test]
fn the_license_of_the_binary_is_the_workspace_s_with_what_its_crates_hold() {
    let workspace = read("Cargo.toml").lines().find_map(|l| l.strip_prefix("license = \"").and_then(|l| l.strip_suffix('"')).map(str::to_string)).expect("the workspace names its license");
    let o = cargo().args(["tree", "-p", "ritsu", "-e", "normal", "--prefix", "none", "--format", "{p}\t{l}", "--offline"]).current_dir(root()).output().unwrap();
    assert!(o.status.success());
    // what each crate of the workspace in the binary holds beyond the workspace's license
    let mut held: Vec<String> = Vec::new();
    for line in String::from_utf8_lossy(&o.stdout).lines() {
        let Some((p, license)) = line.trim_end_matches(" (*)").split_once('\t') else { continue };
        if !p.contains(" (") {
            continue; // a crate from outside
        }
        let (first, more) = terms(license);
        assert_eq!(first, workspace, "{p}: `{license}` is not the workspace's license and what the crate holds");
        if p.starts_with("ritsu ") {
            continue;
        }
        for t in more {
            if !held.contains(&t) {
                held.push(t);
            }
        }
    }
    assert!(held.contains(&"Unicode-3.0".to_string()) && held.contains(&"BSD-3-Clause".to_string()), "{held:?}");
    let ritsu = license_of("crates/ritsu/Cargo.toml");
    let (first, mut more) = terms(&ritsu);
    assert_eq!(first, workspace, "ritsu: {ritsu}");
    let mut want = held.clone();
    want.sort();
    more.sort();
    assert_eq!(more, want, "ritsu's license says other than what its crates hold: {ritsu}");
    assert_eq!(license_of("crates/ritsu-wasm/Cargo.toml"), ritsu, "ritsu-wasm, the module of the page, is ritsu's");
    assert!(read("packaging/nfpm.yaml").contains(&format!("\nlicense: {ritsu}\n")), "nfpm.yaml");
    // Homebrew's words: all_of and any_of, in the order of ritsu's, a line each (brew's audit asks
    // for a nested license to be split onto lines)
    let (first, more) = terms(&ritsu);
    let any: Vec<String> = first.split(" OR ").map(|l| format!("\"{l}\"")).collect();
    let all: String = more.iter().map(|l| format!("    \"{l}\",\n")).collect();
    let words = format!("  license all_of: [\n    {{ any_of: [{}] }},\n{all}  ]\n", any.join(", "));
    assert!(read("packaging/homebrew.sh").contains(&words), "homebrew.sh does not say {words}");
    let (intro, _) = notices();
    assert!(intro.split_whitespace().collect::<Vec<_>>().join(" ").contains(&format!(" are under {ritsu}, ")), "THIRD_PARTY_NOTICES gives another license than {ritsu}");
}

/// Every `include_bytes!` of a crate's sources that takes a copy of a law (a file under
/// `sources/law/`) into the binary, as a path from the root of the repository.
fn laws_built_in() -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for c in std::fs::read_dir(root().join("crates")).unwrap().flatten() {
        walk(&c.path().join("src"), &mut files);
    }
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        const OPEN: &str = "include_bytes!(\"";
        for (i, _) in text.match_indices(OPEN) {
            let rel = text[i + OPEN.len()..].split('"').next().unwrap();
            if rel.contains("/sources/law/") {
                let p = f.parent().unwrap().join(rel).canonicalize().unwrap_or_else(|e| panic!("{}: {rel}: {e}", f.display()));
                out.push(p.strip_prefix(root()).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The copies of laws the ledgers of `explain` take into the binary are the ones the section of
/// laws lists; it gives e-Gov's revision as the copies' revision.txt does, says which copy was
/// changed on purpose (the one that differs from its pair), and says of the CFR what yuen's
/// notices say.
#[test]
fn the_notices_list_the_copies_of_laws_the_binary_holds() {
    let held = laws_built_in();
    assert!(!held.is_empty(), "no copy of a law is built in?");
    let (_, sections) = notices();
    let laws = sections.iter().find(|s| s.title == "Laws from e-Gov and the eCFR").expect("THIRD_PARTY_NOTICES has the section of laws");
    let listed: Vec<&str> = laws.text.lines().filter_map(|l| l.strip_prefix("  ")).filter(|l| l.starts_with("crates/")).collect();
    let mut sorted: Vec<String> = listed.iter().map(|l| l.to_string()).collect();
    sorted.sort();
    assert_eq!(sorted, held, "the copies of laws the crates build in are not the ones THIRD_PARTY_NOTICES lists");
    // e-Gov's revision, as the copies' revision.txt says it, and the copy that was changed
    let civil: Vec<&&str> = listed.iter().filter(|l| l.contains("/129AC0000000089@")).collect();
    let revision = std::fs::read_to_string(root().join(civil[0]).parent().unwrap().join("revision.txt")).expect("revision.txt beside the first copy");
    assert!(laws.text.contains(&format!("revision `{}`", revision.trim())) && laws.text.contains(&format!("版 {}）", revision.trim())), "the section gives another revision than {}", revision.trim());
    let first = std::fs::read(root().join(civil[0])).unwrap();
    let changed: Vec<usize> = civil.iter().enumerate().filter(|(_, l)| std::fs::read(root().join(l)).unwrap() != first).map(|(i, _)| i + 1).collect();
    assert_eq!(changed, [4], "the section says the fourth copy of the Civil Code is the one changed on purpose");
    assert!(laws.text.contains("The fourth copy is edited"), "{}", laws.text);
    let cfr: Vec<&&str> = listed.iter().filter(|l| l.contains("CFR")).collect();
    let pair: Vec<&&&str> = cfr.iter().filter(|l| l.ends_with("1910.157.xml")).collect();
    assert_eq!(pair.len(), 2);
    assert!(std::fs::read(root().join(pair[0])).unwrap() != std::fs::read(root().join(pair[1])).unwrap() && listed.last() == Some(*pair[1]), "the last copy is the CFR's changed one");
    let flat = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let cfr_words = "The Code of Federal Regulations is a work of the United States government and is not subject to copyright in the United States (17 U.S.C. 105).";
    assert!(flat(&read("crates/yuen/THIRD_PARTY_NOTICES.md")).contains(cfr_words) && flat(&laws.text).contains(cfr_words), "the section says of the CFR what yuen's notices say");
}
