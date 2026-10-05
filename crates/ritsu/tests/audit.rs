//! What the audit (.github/workflows/audit.yml, DESIGN 3.6) reads, and what it lets pass. The
//! workflow looks up the crates in the RustSec advisory database (cargo-deny) and every lockfile of
//! the repository in OSV (osv-scanner); both need the network, so they run in CI. The tests here
//! hold, without it, what makes their answer the right one:
//!
//! - Every manifest the tests install from has its lockfile, and every requirements.txt pins each
//!   package it installs to one version, so the scan (`--no-resolve`) reads the versions CI installs.
//! - The versions `ritsu gen` writes into a package, and those the generated code names, are the
//!   ones the tools lock: a scan of the tools is a scan of what a package asks for (DESIGN 9.3).
//! - Every advisory let pass says why, and osv-scanner's until when.
//! - The workflow scans the whole repository, every day, and release.yml runs it before it builds.
//! - The binary a release hands out carries the list of its crates (cargo-auditable), for the
//!   scanners of those who install it.

use ritsu_testkit::TempDir;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// Every file under `dir` called `name`, but in the directories a tool installs into.
fn find(dir: &Path, name: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let file = e.file_name().to_string_lossy().to_string();
        if ["node_modules", ".venv", "target", "build", ".lake"].contains(&file.as_str()) || e.file_type().is_ok_and(|t| t.is_symlink()) {
            continue;
        }
        if p.is_dir() {
            find(&p, name, out);
        } else if file == name {
            out.push(p);
        }
    }
}

/// The tools' directories of every language: what CI installs (tools.yml) and the scan reads.
fn tools(name: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for c in std::fs::read_dir(root().join("crates")).unwrap().flatten() {
        find(&c.path().join("tools"), name, &mut out);
    }
    out.sort();
    out
}

fn shown(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).display().to_string()
}

/// The requirements of a requirements.txt: each line that names a package, without its hashes.
fn requirements(text: &str) -> Vec<String> {
    text.lines().filter(|l| !l.is_empty() && !l.starts_with([' ', '\t', '#', '-'])).map(|l| l.trim_end_matches('\\').trim().to_string()).collect()
}

/// `name==version` as the name (lower case, `_` and `.` as `-`, without extras) and the version.
fn pinned(req: &str) -> Option<(String, String)> {
    let (name, version) = req.split_once("==")?;
    let name = name.split('[').next()?.trim().to_lowercase().replace(['_', '.'], "-");
    let ok_name = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let ok_version = !version.is_empty() && !version.contains([' ', ';', ',', '*', '<', '>', '=', '~', '!']);
    (ok_name && ok_version).then(|| (name, version.to_string()))
}

/// The `require`s of a go.mod, by module.
fn go_requires(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut inside = false;
    for line in text.lines() {
        let l = line.trim();
        let one = l.strip_prefix("require ").filter(|r| !r.starts_with('('));
        if l == "require (" {
            inside = true;
        } else if l == ")" {
            inside = false;
        } else if inside || one.is_some() {
            let mut parts = one.unwrap_or(l).split_whitespace();
            if let (Some(m), Some(v)) = (parts.next(), parts.next()) {
                out.insert(m.to_string(), v.to_string());
            }
        }
    }
    out
}

#[test]
fn every_manifest_the_tests_install_from_is_locked() {
    let mut wrong = Vec::new();
    for p in tools("package.json") {
        if !p.with_file_name("package-lock.json").exists() {
            wrong.push(format!("{}: no package-lock.json beside it", shown(&p)));
        }
    }
    for p in tools("go.mod") {
        if !go_requires(&read(&p)).is_empty() && !p.with_file_name("go.sum").exists() {
            wrong.push(format!("{}: no go.sum beside it", shown(&p)));
        }
    }
    let reqs = tools("requirements.txt");
    // nine of them today: chobo's runner, five of dandori's, koyomi's, sakai's and yuen's
    assert!(reqs.len() >= 9, "the requirements of the tools: {reqs:?}");
    for p in reqs {
        for r in requirements(&read(&p)) {
            if pinned(&r).is_none() {
                wrong.push(format!("{}: `{r}` is not one version (name==version)", shown(&p)));
            }
        }
    }
    assert!(root().join("Cargo.lock").exists());
    assert!(wrong.is_empty(), "osv-scanner reads lockfiles as they are (--no-resolve), so each manifest needs one:\n{}", wrong.join("\n"));
}

/// The versions the tools lock, of the packages `ritsu gen` writes into a package: npm's (dandori's
/// Temporal runner, chobo's runner), PyPI's (dandori's Temporal runner for Python, chobo's) and Go's
/// (dandori's tools/temporal-go, chobo's tools/runner/go).
struct Locked {
    npm: BTreeMap<String, String>,
    pypi: BTreeMap<String, String>,
    go: BTreeMap<String, String>,
}

/// Temporal's packages from dandori's runners, the rest (TigerBeetle's) from chobo's.
fn of_temporal(name: &str) -> bool {
    name.starts_with("@temporalio/") || name == "temporalio"
}

fn locked() -> Locked {
    let tools = root().join("crates");
    let mut npm = BTreeMap::new();
    for (lock, temporal) in [("dandori/tools/temporal/package-lock.json", true), ("chobo/tools/runner/package-lock.json", false)] {
        let v: serde_json::Value = serde_json::from_str(&read(&tools.join(lock))).unwrap();
        for (k, p) in v["packages"].as_object().unwrap() {
            let Some(name) = k.strip_prefix("node_modules/").filter(|n| !n.contains("/node_modules/") && of_temporal(n) == temporal) else { continue };
            if let Some(version) = p["version"].as_str() {
                npm.insert(name.to_string(), version.to_string());
            }
        }
    }
    let mut pypi = BTreeMap::new();
    for (req, temporal) in [("dandori/tools/temporal-python/requirements.txt", true), ("chobo/tools/runner/requirements.txt", false)] {
        for r in requirements(&read(&tools.join(req))) {
            let (name, version) = pinned(&r).unwrap();
            if of_temporal(&name) == temporal {
                pypi.insert(name, version);
            }
        }
    }
    let mut go = BTreeMap::new();
    for gomod in ["dandori/tools/temporal-go/go.mod", "chobo/tools/runner/go/go.mod"] {
        for (m, v) in go_requires(&read(&tools.join(gomod))) {
            if let Some(other) = go.insert(m.clone(), v.clone()) {
                assert_eq!(other, v, "{m}: the two go.mod require it at two versions");
            }
        }
    }
    Locked { npm, pypi, go }
}

/// The package of the stockroom project in each language, with its books on PostgreSQL and on
/// TigerBeetle.
fn packages(t: &TempDir) -> Vec<PathBuf> {
    let project = root().join("crates/ritsu/tests/projects/stockroom");
    let mut outs = Vec::new();
    for books in ["postgres", "tigerbeetle"] {
        let out = t.path().join(books);
        let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(&project).args(["gen", "--root", ".", "--out", out.to_str().unwrap(), "--books", books]).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        outs.push(out);
    }
    outs
}

/// Each `<module> v<version>` a line of Go's comments names, of the module paths the lock knows
/// or that look like one (a host with a dot, then a path).
fn named_go_modules(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| l.trim_start().starts_with("//")) {
        let words: Vec<&str> = line.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')' || c == ';').filter(|w| !w.is_empty()).collect();
        for w in words.windows(2) {
            let host = w[0].split('/').next().unwrap_or("");
            let v = w[1].trim_end_matches('.');
            if w[0].contains('/') && host.contains('.') && v.starts_with('v') && v[1..].split('.').count() == 3 {
                out.push((w[0].to_string(), v.to_string()));
            }
        }
    }
    out
}

#[test]
fn the_packages_ask_for_the_versions_the_tools_lock() {
    let lock = locked();
    let t = TempDir::new("audit-packages");
    let mut checked = 0;
    for out in packages(&t) {
        // TypeScript: the dependencies of package.json
        let pkg: serde_json::Value = serde_json::from_str(&read(&out.join("typescript/package.json"))).unwrap();
        for (name, v) in pkg["dependencies"].as_object().unwrap() {
            assert_eq!(lock.npm.get(name).map(String::as_str), v.as_str(), "typescript/package.json asks for {name} {v}");
            checked += 1;
        }
        // Python: the dependencies of pyproject.toml
        let toml = read(&out.join("python/pyproject.toml"));
        let deps = toml.split("dependencies = [").nth(1).and_then(|r| r.split(']').next()).unwrap();
        for d in deps.split(',').map(|d| d.trim().trim_matches('"')).filter(|d| !d.is_empty()) {
            let (name, v) = pinned(d).unwrap_or_else(|| panic!("python/pyproject.toml asks for `{d}`, not one version"));
            assert_eq!(lock.pypi.get(&name), Some(&v), "python/pyproject.toml asks for {name} {v}");
            checked += 1;
        }
        // Go: what every comment of the package names, doc.go's list the first
        let doc = read(&out.join("go/doc.go"));
        assert!(doc.contains("go.temporal.io/sdk v"), "{doc}");
        for (rel, text) in walk(&out.join("go")) {
            for (m, v) in named_go_modules(&text) {
                assert_eq!(lock.go.get(&m), Some(&v), "go/{rel} names {m} {v}");
                checked += 1;
            }
        }
        // and what the clients of the books say they call TigerBeetle through
        for (rel, text) in walk(&out) {
            let says = [
                ("through tigerbeetle-node ", &lock.npm, "tigerbeetle-node", ""),
                ("through the tigerbeetle package ", &lock.pypi, "tigerbeetle", ""),
                ("through tigerbeetle-go ", &lock.go, "github.com/tigerbeetle/tigerbeetle-go", "v"),
            ];
            for (phrase, lock_of, name, v_) in says {
                if let Some(rest) = text.split(phrase).nth(1) {
                    let v = rest.trim_start_matches('v').split(|c: char| !(c.is_ascii_digit() || c == '.')).next().unwrap().trim_end_matches('.');
                    assert_eq!(lock_of.get(name).map(String::as_str), Some(format!("{v_}{v}").as_str()), "{rel} says {phrase}{v_}{v}");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 16, "{checked} versions checked");
}

/// Every file under `dir`, by its path from it, with its text.
fn walk(dir: &Path) -> Vec<(String, String)> {
    let mut paths = Vec::new();
    fn all(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            if e.path().is_dir() {
                all(&e.path(), out);
            } else {
                out.push(e.path());
            }
        }
    }
    all(dir, &mut paths);
    paths.sort();
    paths.into_iter().filter_map(|p| Some((p.strip_prefix(dir).unwrap().display().to_string(), std::fs::read_to_string(&p).ok()?))).collect()
}

/// The days from 1970-01-01 to `y-m-d` (the proleptic Gregorian calendar).
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

#[test]
fn every_advisory_let_pass_says_why_and_until_when() {
    let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 / 86_400;
    let osv = read(&root().join("osv-scanner.toml"));
    let entries: Vec<&str> = osv.split("[[IgnoredVulns]]").skip(1).collect();
    for e in &entries {
        let field = |k: &str| e.lines().find_map(|l| l.trim().strip_prefix(k).and_then(|r| r.trim().strip_prefix('=')).map(|v| v.trim().trim_matches('"').to_string()));
        let id = field("id").unwrap_or_else(|| panic!("an entry with no id: {e}"));
        assert!(field("reason").is_some_and(|r| r.len() > 20), "{id}: no reason");
        let until = field("ignoreUntil").unwrap_or_else(|| panic!("{id}: no ignoreUntil"));
        let ymd: Vec<i64> = until.split('-').map(|p| p.parse().unwrap()).collect();
        let left = days(ymd[0], ymd[1], ymd[2]) - today;
        assert!(left <= 366, "{id}: let pass until {until}, more than a year from now; read the reason again within a year");
    }
    // cargo-deny's: each one a table with its reason
    let deny = read(&root().join("deny.toml"));
    let ignore = deny.split("\nignore = [").nth(1).and_then(|r| r.split(']').next()).expect("deny.toml says what it ignores");
    for e in ignore.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        assert!(e.starts_with("{ id = \"") && e.contains("reason = \""), "deny.toml ignores `{e}` without a reason");
    }
}

#[test]
fn the_audit_scans_everything_every_day_and_before_a_release() {
    let audit = read(&root().join(".github/workflows/audit.yml"));
    for want in [
        "push:",
        "pull_request:",
        "schedule:",
        "workflow_call:",
        "cargo-deny --locked check --show-stats",
        "osv-scanner scan source --config osv-scanner.toml --no-resolve --recursive .",
        "sha256sum -c -",
    ] {
        assert!(audit.contains(want), "audit.yml has no `{want}`");
    }
    // only GitHub's own actions
    for line in audit.lines().filter(|l| l.trim_start().starts_with("- uses:") || l.trim_start().starts_with("uses:")) {
        assert!(line.contains("actions/"), "audit.yml uses an action from outside GitHub: {line}");
    }
    let release = read(&root().join(".github/workflows/release.yml"));
    let build = release.split("\n  build:\n").nth(1).expect("release.yml has the job build");
    assert!(release.contains("  audit:\n    uses: ./.github/workflows/audit.yml\n"), "release.yml does not run the audit");
    assert!(build.lines().take(2).any(|l| l.trim() == "needs: audit"), "release.yml builds before the audit");
}

/// The binary a release hands out carries the list of the crates it is made of (cargo-auditable),
/// which a scanner of what is installed reads; each platform's build fetches the cargo-auditable of
/// the machine it runs on, held to the SHA-256 its release gives, and the job looks for the
/// section in the binary before it archives it.
#[test]
fn the_released_binary_carries_its_crates() {
    let release = read(&root().join(".github/workflows/release.yml"));
    assert!(release.contains("run: cargo auditable build --release --locked --target ${{ matrix.target }} -p ritsu"), "release.yml builds without cargo-auditable");
    assert!(!release.contains("run: cargo build --release"), "release.yml builds a binary without cargo-auditable");
    assert!(release.contains("releases/download/v0.7.7/cargo-auditable-${{ matrix.host }}.tar.xz"));
    assert!(release.contains("readelf -S \"$b\" | grep -q '\\.dep-v0'") && release.contains("otool -l \"$b\" | grep -q 'sectname \\.dep-v0'"));
    let matrix = release.split("        include:\n").nth(1).unwrap().split("    runs-on:").next().unwrap();
    let entries: Vec<&str> = matrix.split("          - target: ").skip(1).collect();
    assert_eq!(entries.len(), 4, "the four platforms");
    for e in entries {
        let field = |k: &str| e.lines().find_map(|l| l.trim().strip_prefix(k)).map(str::trim);
        let host = field("host:").unwrap_or_else(|| panic!("no host: {e}"));
        assert!(["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin"].contains(&host), "{host}");
        let sum = field("auditable_sha256:").unwrap_or_else(|| panic!("no auditable_sha256: {e}"));
        assert!(sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit()), "{sum}");
    }
}
