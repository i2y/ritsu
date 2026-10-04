//! What a release hands out (DESIGN 13.2): the archive with `ritsu`, its seven links and the two
//! licenses, the formula, the packages, the action, and the workflow that makes them. The scripts of `packaging/`
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
/// licenses, the files of the repository.
#[test]
fn the_archive_holds_ritsu_a_link_for_each_language_and_the_licenses() {
    let t = TempDir::new("release-archive");
    let (names, dir) = unpacked(&t);
    let mut want: Vec<&str> = LANGUAGES.to_vec();
    want.extend(["ritsu", "LICENSE-MIT", "LICENSE-APACHE"]);
    want.sort();
    assert_eq!(names, want);
    assert!(std::fs::symlink_metadata(dir.join("ritsu")).unwrap().file_type().is_file());
    for license in ["LICENSE-MIT", "LICENSE-APACHE"] {
        assert!(std::fs::symlink_metadata(dir.join(license)).unwrap().file_type().is_file(), "{license} is a file");
        assert_eq!(std::fs::read(dir.join(license)).unwrap(), std::fs::read(root().join(license)).unwrap(), "{license} is the repository's");
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
    assert!(formula.contains("class Ritsu < Formula") && formula.contains(r#"license any_of: ["MIT", "Apache-2.0"]"#), "{formula}");
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
    // the SHA256SUMS holds the archive and both packages
    assert!(release.contains("sha256sum ritsu-*.tar.gz ritsu_*.deb ritsu-*.rpm"));
    // the packages are named as the script names them, the formula's test and the release's
    // checks hold every name to one version
    let linux = read("packaging/linux.sh");
    assert!(linux.contains("deb=\"ritsu_$version-1_$arch.deb\"") && linux.contains("rpm=\"ritsu-$version-1.$rpmarch.rpm\""));
}

/// Every crate takes the version of the workspace (DESIGN 13.1), and so every name says it. The
/// versions are made one at the end of stage F, all together, because the headers of what is
/// generated and the golden files hold them; this is what checks that it was done.
#[test]
#[ignore = "the versions are made one at the end of stage F (PLAN F.7); run with --ignored then"]
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
