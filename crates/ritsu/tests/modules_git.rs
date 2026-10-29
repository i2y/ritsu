//! The dependencies from git repositories (DESIGN 17.12.1; PLAN 8 stage I), against a git server
//! the test starts (`ritsu_testkit::git`: `git http-backend` on the loopback, and nothing else):
//!
//! - tax and pricing, from the example (DESIGN 17.26), each in a repository of its own, tagged;
//!   the shop adds pricing, locks, fetches and checks, in English, in Japanese and in JSON (golden
//!   files, the port, the commits and the digests masked; `RITSU_BLESS=1` writes them).
//! - A module in a directory of a large repository: what the server sends stays small, and the
//!   digest is the one `ritsu mod push --dry-run` gives in a checkout of it.
//! - `versions` and `@latest`, the tags that are no versions left out; a tag moved (`fetch` keeps the
//!   lock's commit, `get` takes the new one, `report` says it moved); a server that takes no filter,
//!   and one of protocol version 0; a repository of SHA-256.
//! - What a commit holds that stops it (exit 1): a symbolic link, a submodule, a Git LFS pointer, a
//!   file over the limit, a `ritsu.mod` of another module or version; a tag or a directory not there
//!   (exit 2).
//! - A URL that is refused, and `--dir` with a source that is not a git URL, stop before git is
//!   started (a git first on the PATH writes down every call); what git is run with
//!   (`GIT_ALLOW_PROTOCOL`, `GIT_TERMINAL_PROMPT`, no `GIT_DIR`); a repository `GIT_DIR`,
//!   `GIT_INDEX_FILE` and `GIT_OBJECT_DIRECTORY` point at is left as it was; neither the hooks of the
//!   person's `core.hooksPath` nor a filter's driver runs; ssh, with an ssh of the test's own that
//!   runs `git upload-pack` here.
//! - `vendor`, `clean` (the lines of the git index too), `where` and `list`; `diff <name>@<version>`,
//!   `report`, `fetch --rev`; `push`, which refuses a module that requires one from git.

use ritsu_base::modules::store;
use ritsu_testkit::git::{GitServer, Options, Repo};
use ritsu_testkit::{TempDir, skip};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const LANG_VARS: [&str; 10] = ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"];

/// The variables of git a run of the test would pass on from the machine: those that point git at
/// a repository, and those that choose how it asks.
const GIT_VARS: [&str; 21] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    "GIT_PROTOCOL",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_ASKPASS",
    "GIT_ALLOW_PROTOCOL",
    "GIT_TERMINAL_PROMPT",
];

/// A git of the test's own, first on the PATH: it writes down how it is called (its arguments, and
/// the variables ritsu sets or takes away) and runs the real one.
const WRAPPED_GIT: &str = "#!/bin/sh\n{\n  printf 'git %s\\n' \"$*\"\n  printf '  GIT_ALLOW_PROTOCOL=%s GIT_TERMINAL_PROMPT=%s GIT_NO_LAZY_FETCH=%s GIT_DIR=%s GIT_INDEX_FILE=%s GIT_OBJECT_DIRECTORY=%s\\n' \"${GIT_ALLOW_PROTOCOL-unset}\" \"${GIT_TERMINAL_PROMPT-unset}\" \"${GIT_NO_LAZY_FETCH-unset}\" \"${GIT_DIR-unset}\" \"${GIT_INDEX_FILE-unset}\" \"${GIT_OBJECT_DIRECTORY-unset}\"\n} >> \"$WRAPPED_GIT_LOG\"\nexec \"$REAL_GIT\" \"$@\"\n";

/// An ssh of the test's own: it writes down its arguments and runs the command it is given here,
/// in the server's root (`git-upload-pack '<path>'`, an absolute path taken from the root).
const FAKE_SSH: &str = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$FAKE_SSH_LOG\"\nfor a in \"$@\"; do cmd=\"$a\"; done\ncd \"$FAKE_SSH_ROOT\" || exit 1\ncmd=$(printf '%s' \"$cmd\" | sed -e \"s#^git-upload-pack '/#git upload-pack '$FAKE_SSH_ROOT/#\" -e \"s#^git-upload-pack '#git upload-pack '#\")\nexec sh -c \"$cmd\"\n";

/// The function of [`World::app`]: the rate the rule vat_rate of tax gives a category.
const APP: &str = "# The VAT of the app: the rate the rule vat_rate of the module tax gives a category.\nmodule vat v1\ndescription \"The VAT rate of a category, as the rule vat_rate of the module tax gives it\"\n\nuse rule vat_rate from \"@tax/rules/vat_rate.rule\"\n\nfn rate(category: vat_rate.category) -> rate[step 1%]\n  vat_rate(category: category).rate\n\nexamples\n  rate(category: vat_rate.category.standard) = 20%\n  rate(category: vat_rate.category.zero) = 0%\n";

/// `path` made executable.
fn executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn said(o: Output) -> (i32, String, String) {
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

/// Whether git runs, with `http-backend`; a SKIP line when it does not.
fn has_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
        && Command::new("git").arg("http-backend").env("REQUEST_METHOD", "GET").env("GIT_PROJECT_ROOT", std::env::temp_dir()).env("PATH_INFO", "/none").output().is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("Status:"));
    if !ok {
        skip("git with http-backend is not on the PATH; a dependency from git is fetched with git");
    }
    ok
}

/// The real git: the first on the PATH.
fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).map(|d| d.join("git")).find(|p| p.is_file()).expect("git on the PATH")
}

/// The text with what changes between runs masked: the port of the server, each commit (40 or 64
/// hexadecimal digits, and the first 12 of one), and each digest (`sha256:` and 64).
fn masked(text: &str, port: u16) -> String {
    let text = text.replace(&format!("127.0.0.1:{port}"), "127.0.0.1:<port>");
    let mut out = String::new();
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if text_at(&b, i, "sha256:") {
            let n = b[i + 7..].iter().take_while(|c| c.is_ascii_hexdigit()).count();
            if n == 64 {
                out.push_str("sha256:<digest>");
                i += 7 + 64;
                continue;
            }
        }
        let boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if boundary {
            let n = b[i..].iter().take_while(|c| c.is_ascii_digit() || ('a'..='f').contains(c)).count();
            let after = b.get(i + n).is_none_or(|c| !c.is_ascii_alphanumeric());
            if after && matches!(n, 12 | 40 | 64) && b[i..i + n].iter().any(|c| c.is_ascii_alphabetic()) {
                out.push_str(if n == 12 { "<commit12>" } else { "<commit>" });
                i += n;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn text_at(b: &[char], i: usize, s: &str) -> bool {
    s.chars().enumerate().all(|(k, c)| b.get(i + k) == Some(&c))
}

/// A test's directory (the module at hand, the store, the configuration of git it is run with, and
/// what is first on the PATH) and its git server.
struct World {
    t: TempDir,
    s: GitServer,
}

impl World {
    fn new(what: &str) -> Option<World> {
        World::with(what, Options::hosted())
    }

    fn with(what: &str, options: Options) -> Option<World> {
        if !has_git() {
            return None;
        }
        let t = TempDir::new(what);
        t.write("gitconfig", "");
        std::fs::create_dir_all(t.path().join("bin")).unwrap();
        let s = GitServer::start(&t.path().join("git"), options);
        Some(World { t, s })
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.t.path().join(rel)
    }

    fn store(&self) -> PathBuf {
        self.path("store")
    }

    fn url(&self, repo: &str) -> String {
        self.s.url_of(repo)
    }

    /// The text with the server's port, the commits, the digests, the test's directory and the
    /// version of ritsu masked.
    fn masked(&self, text: &str) -> String {
        let v = env!("CARGO_PKG_VERSION");
        masked(text, self.s.port)
            .replace(&self.t.path().display().to_string(), "<tmp>")
            .replace(&format!("\"ritsu\": \"{v}\""), "\"ritsu\": \"<version>\"")
            .replace(&format!("ritsu {v}"), "ritsu <version>")
    }

    fn command(&self, dir: &str, args: &[&str], env: &[(&str, &str)]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(self.path(dir)).args(args);
        c.env(store::VARIABLE, self.store());
        for v in LANG_VARS.iter().chain(GIT_VARS.iter()) {
            c.env_remove(v);
        }
        c.env("GIT_CONFIG_NOSYSTEM", "1").env("GIT_CONFIG_GLOBAL", self.path("gitconfig"));
        c.env("PATH", format!("{}:{}", self.path("bin").display(), std::env::var("PATH").unwrap_or_default()));
        for (k, v) in env {
            c.env(k, v);
        }
        c
    }

    fn run_env(&self, dir: &str, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        said(self.command(dir, args, env).output().expect("could not run ritsu"))
    }

    fn run(&self, dir: &str, args: &[&str]) -> (i32, String, String) {
        self.run_env(dir, args, &[])
    }

    /// ritsu, which has to pass: what it printed.
    fn ok(&self, dir: &str, args: &[&str]) -> String {
        let (code, out, err) = self.run(dir, args);
        assert_eq!(code, 0, "ritsu {args:?} in {dir}:\n{out}{err}");
        out
    }

    /// The example's module `from` (a directory of `tests/projects/modules`) written in `repo`, its
    /// name the one the server's URL gives, and each `require … path` of another module of the
    /// example a `require … git` of its repository on the server.
    fn put_example(&self, repo: &Repo, from: &str) {
        let src = here().join("tests/projects/modules").join(from);
        for f in files_under(&src) {
            let rel = f.strip_prefix(&src).unwrap().to_string_lossy().into_owned();
            let bytes = std::fs::read(&f).unwrap();
            if rel == "ritsu.mod" {
                repo.write(&rel, self.renamed(&String::from_utf8(bytes).unwrap()));
            } else {
                repo.write(&rel, bytes);
            }
        }
    }

    /// A `ritsu.mod` of the example with the names and the requires of the server.
    fn renamed(&self, text: &str) -> String {
        text.replace("module example.com/acme/", "module 127.0.0.1/acme/")
            .replace("require tax path \"../tax\"", &format!("require tax git \"{}\" 1.0.0", self.url("acme/tax")))
            .replace("require tax path \"../../tax\"", &format!("require tax git \"{}\" 1.0.0", self.url("acme/tax")))
    }

    /// tax 1.0.0 (a tag without a message) and pricing 1.0.0 and 1.1.0 (tags with one), each in a
    /// repository of its own, pricing with its lock: the repositories. The locks are written with a
    /// store of their own (`setup-store`), so that the test's starts empty.
    fn tax_and_pricing(&self) -> (Repo, Repo) {
        let tax = self.s.repo("acme/tax");
        self.put_example(&tax, "tax");
        tax.commit("tax 1.0.0");
        tax.tag("v1.0.0");
        let pricing = self.s.repo("acme/pricing");
        for (from, version) in [("versions/pricing-1.0.0", "1.0.0"), ("pricing", "1.1.0")] {
            self.put_example(&pricing, from);
            self.locked(&pricing);
            pricing.commit(&format!("pricing {version}"));
            pricing.annotated(&format!("v{version}"));
        }
        (tax, pricing)
    }

    /// The lock of the module at the root of `repo` written (with the store `setup-store`), and
    /// added.
    fn locked(&self, repo: &Repo) {
        let setup = self.path("setup-store");
        let dir = format!("git/work/{}", repo.path);
        let (code, out, err) = self.run_env(&dir, &["mod", "lock"], &[(store::VARIABLE, setup.to_str().unwrap())]);
        assert_eq!(code, 0, "the lock of {}: {out}{err}", repo.path);
        repo.git(&["add", "ritsu.lock"]);
    }

    /// The example's shop, the module at hand, with no dependency yet.
    fn shop(&self) {
        ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/modules/shop"), &self.path("shop"));
        let text = self.t.read("shop/ritsu.mod").replace("require pricing path \"../pricing\"\n", "");
        self.t.write("shop/ritsu.mod", text);
    }

    /// A module of the test's own at `dir`, with no dependency yet, whose function reads the rule
    /// of a dependency named `tax`.
    fn app(&self, dir: &str) {
        self.t.write(&format!("{dir}/ritsu.mod"), format!("module example.com/acme/{dir}\nversion 0.1.0\n"));
        self.t.write(&format!("{dir}/vat.kn"), APP);
    }

    /// The git of the test's own, first on the PATH from now on, writing down each call in
    /// `git.log`.
    fn wrap_git(&self) -> Vec<(String, String)> {
        let p = self.t.write("bin/git", WRAPPED_GIT);
        executable(&p);
        vec![("WRAPPED_GIT_LOG".to_string(), self.path("git.log").display().to_string()), ("REAL_GIT".to_string(), real_git().display().to_string())]
    }

    /// The calls of the wrapped git so far.
    fn git_calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.path("git.log")).unwrap_or_default().lines().map(str::to_string).collect()
    }
}

/// The files under `dir`, in the order of their paths.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn golden(name: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join("tests/golden/modules").join(name), got) {
        failures.push(e);
    }
}

/// Bytes that look random, the same for a seed on every machine.
fn noise(seed: u64, n: usize) -> Vec<u8> {
    let mut x = seed.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// The commit a tag of `repo` leads to.
fn tagged(repo: &Repo, tag: &str) -> String {
    repo.git(&["rev-parse", &format!("{tag}^{{commit}}")]).trim().to_string()
}

/// Every file under `dir` with its bytes, in the order of their paths.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    files_under(dir).into_iter().map(|p| (p.clone(), std::fs::read(&p).unwrap_or_default())).collect()
}

/// The shop adds pricing from its repository: the lock pins pricing and tax (which pricing requires
/// from its own) with the commits their tags lead to and the URLs; the lock alone fetches both into
/// an empty store, with their records and the lines of the git index; check passes and says where
/// each comes from. In English, in Japanese and in JSON (golden files). Once both are here, neither
/// a fetch nor a lock asks the server anything.
#[test]
fn a_module_from_git_is_added_locked_fetched_and_checked() {
    let Some(w) = World::new("git-flow") else { return };
    let (tax, pricing) = w.tax_and_pricing();
    w.shop();
    let mut failures = Vec::new();
    let out = w.ok("shop", &["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))]);
    golden("git-add.en.txt", &w.masked(&out), &mut failures);
    assert!(w.t.read("shop/ritsu.mod").contains(&format!("require pricing git \"{}\" 1.1.0\n", w.url("acme/pricing"))));
    let lock = w.t.read("shop/ritsu.lock");
    golden("git-lock.txt", &w.masked(&lock), &mut failures);
    assert!(lock.contains("git 127.0.0.1/acme/pricing 1.1.0 sha256:"), "{lock}");
    assert!(lock.contains(&format!(" commit {} \"{}\"\n", pricing.head(), w.url("acme/pricing"))), "{lock}");
    assert!(lock.contains(&format!(" commit {} \"{}\"\n", tax.head(), w.url("acme/tax"))), "{lock}");
    // the lock alone: an empty store each time
    for (args, name) in [(vec!["mod", "fetch"], "git-fetch.en.txt"), (vec!["mod", "fetch", "--lang", "ja"], "git-fetch.ja.txt"), (vec!["mod", "fetch", "--format", "json"], "git-fetch.json")] {
        std::fs::remove_dir_all(w.store()).unwrap();
        let out = w.ok("shop", &args);
        golden(name, &w.masked(&out), &mut failures);
    }
    // the store: each entry's record names the URL and the commit, and the git index has its line
    let index = std::fs::read_to_string(w.store().join("git/127.0.0.1/acme/pricing/1.1.0")).unwrap();
    assert!(index.starts_with(&format!("{} sha256:", pricing.head())), "{index}");
    let d = index.trim_end().rsplit(' ').next().unwrap().to_string();
    let rec = json(&std::fs::read_to_string(w.store().join("sha256").join(&d[7..]).join("record.json")).unwrap());
    assert_eq!((&rec["module"], &rec["version"], &rec["git"]), (&serde_json::json!("127.0.0.1/acme/pricing"), &serde_json::json!("1.1.0"), &serde_json::json!({"url": w.url("acme/pricing"), "commit": pricing.head()})));
    assert!(!w.store().join("refs").exists(), "a version from git has no line in refs/");
    // here: nothing asked
    w.s.clear_log();
    assert_eq!(w.ok("shop", &["mod", "fetch"]), "every module is here already (2 modules)\n");
    assert_eq!(w.ok("shop", &["mod", "lock"]), "ritsu.lock is up to date: 2 modules\n");
    assert_eq!(w.t.read("shop/ritsu.lock"), lock);
    assert!(w.s.log().is_empty(), "{:?}", w.s.log());
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    let v = json(&w.ok("shop", &["check", "--format", "json"]));
    golden("git-check-dependencies.json", &w.masked(&serde_json::to_string_pretty(&v["dependencies"]).unwrap()), &mut failures);
    // add in Japanese, the latest version (no version written); and in JSON, a version written
    for (dir, args, name) in [("shop2", vec!["--lang", "ja"], "git-add.ja.txt"), ("shop3", vec!["--format", "json"], "git-add.json")] {
        std::fs::create_dir_all(w.path(dir)).unwrap();
        w.ok(dir, &["mod", "init", &format!("example.com/acme/{dir}")]);
        let source = if dir == "shop2" { w.url("acme/tax") } else { format!("{}@1.0.0", w.url("acme/pricing")) };
        let mut a = vec!["mod", "add", "dep", source.as_str()];
        a.extend(args);
        golden(name, &w.masked(&w.ok(dir, &a)), &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A module in a directory of a large repository (`dir`, its tags `<dir>/v<version>`): what the
/// server sends is the commit's trees and the module's two files, not the repository's assets; the
/// digests are the ones `ritsu mod push --dry-run` gives in a checkout of it. A server that takes no
/// filter sends the whole commit, and the digests are the same.
#[test]
fn a_module_in_a_directory_of_a_large_repository() {
    let Some(w) = World::new("git-mono") else { return };
    let mono = w.s.repo("acme/mono");
    for i in 0..40 {
        mono.write(&format!("assets/blob-{i:02}.bin"), noise(i, 128 * 1024));
    }
    mono.write("modules/tax/ritsu.mod", format!("module 127.0.0.1/acme/mono/modules/tax\nversion 1.0.0\nritsu >={}\n", env!("CARGO_PKG_VERSION")));
    mono.write("modules/tax/rules/vat_rate.rule", std::fs::read(here().join("tests/projects/modules/tax/rules/vat_rate.rule")).unwrap());
    mono.write("README.md", "the repository of the Acme shop\n");
    mono.commit("tax 1.0.0, and the assets");
    mono.tag("modules/tax/v1.0.0");
    mono.tag("v3.0.0");
    w.app("shop");
    let out = w.ok("shop", &["mod", "add", "tax", &format!("{}@1.0.0", w.url("acme/mono")), "--dir", "modules/tax"]);
    assert_eq!(out, format!("added `require tax git \"{}\" 1.0.0 dir \"modules/tax\"` to ritsu.mod (127.0.0.1/acme/mono/modules/tax 1.0.0)\nwrote ritsu.lock: 1 module\n", w.url("acme/mono")));
    let answered = w.s.answered();
    assert!(answered < 256 * 1024, "{answered} bytes answered for a module of two files: {:?}", w.s.log());
    let v = json(&w.ok("git/work/acme/mono/modules/tax", &["mod", "push", "--dry-run", "--format", "json"]));
    let (d, files) = (v["digest"].as_str().unwrap().to_string(), v["files_digest"].as_str().unwrap().to_string());
    let line = format!("git 127.0.0.1/acme/mono/modules/tax 1.0.0 {d} files {files} commit {} \"{}\"\n", mono.head(), w.url("acme/mono"));
    assert!(w.t.read("shop/ritsu.lock").contains(&line), "{}\n{line}", w.t.read("shop/ritsu.lock"));
    assert_eq!(w.ok("shop", &["mod", "versions", &w.url("acme/mono"), "--dir", "modules/tax"]), "1.0.0\n");
    assert_eq!(w.ok("shop", &["mod", "versions", &w.url("acme/mono")]), "3.0.0\n");
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    // a server that takes no filter, on the same repositories
    let whole = GitServer::start(&w.path("git"), Options { filter: false, ..Options::hosted() });
    w.app("shop2");
    w.ok("shop2", &["mod", "add", "tax", &format!("{}@1.0.0", whole.url_of("acme/mono.git")), "--dir", "modules/tax"]);
    assert!(whole.answered() > 4 * 1024 * 1024, "{} bytes", whole.answered());
    let line = format!("git 127.0.0.1/acme/mono/modules/tax 1.0.0 {d} files {files} commit {} \"{}\"\n", mono.head(), whole.url_of("acme/mono.git"));
    assert!(w.t.read("shop2/ritsu.lock").contains(&line), "{}", w.t.read("shop2/ritsu.lock"));
}

/// `versions` lists the tags that are versions, lowest first (not `v01.0.0`, `v1.3.0+build.5`,
/// `1.4.0`, a tag of another directory, a branch named like a version); `add` without a version and
/// `@latest` take the highest that is no prerelease; `get` moves the dependency down and back up;
/// a version with no tag changes nothing (exit 2).
#[test]
fn versions_and_latest_read_the_tags() {
    let Some(w) = World::new("git-versions") else { return };
    let (_tax, pricing) = w.tax_and_pricing();
    for t in ["v01.0.0", "v1.3.0+build.5", "1.4.0", "modules/x/v9.0.0", "release-2"] {
        pricing.tag(t);
    }
    pricing.git(&["branch", "v8.0.0"]);
    pricing.write("NOTES.md", "a prerelease\n");
    pricing.commit("pricing 1.2.0-rc.1");
    pricing.tag("v1.2.0-rc.1");
    w.shop();
    let url = w.url("acme/pricing");
    assert_eq!(w.ok("shop", &["mod", "versions", &url]), "1.0.0\n1.1.0\n1.2.0-rc.1\n");
    let v = json(&w.ok("shop", &["mod", "versions", &url, "--format", "json"]));
    assert_eq!(v, serde_json::json!({"ritsu": env!("CARGO_PKG_VERSION"), "module": "127.0.0.1/acme/pricing", "versions": ["1.0.0", "1.1.0", "1.2.0-rc.1"], "latest": "1.1.0"}));
    assert_eq!(
        w.ok("shop", &["mod", "add", "pricing", &url]),
        format!("added `require pricing git \"{url}\" 1.1.0` to ritsu.mod (127.0.0.1/acme/pricing 1.1.0)\nwrote ritsu.lock: 2 modules\n")
    );
    assert_eq!(w.ok("shop", &["mod", "versions", "pricing"]), "1.0.0\n1.1.0\n1.2.0-rc.1\n");
    assert_eq!(w.ok("shop", &["mod", "get", "pricing@1.0.0"]), "changed the version of the dependency `pricing` from 1.1.0 to 1.0.0 (127.0.0.1/acme/pricing)\nwrote ritsu.lock: 2 modules\n");
    assert!(w.t.read("shop/ritsu.lock").contains(&format!(" commit {} \"{url}\"\n", tagged(&pricing, "v1.0.0"))));
    let v = json(&w.ok("shop", &["mod", "get", "pricing@latest", "--format", "json"]));
    assert_eq!(v["require"], serde_json::json!({"name": "pricing", "module": "127.0.0.1/acme/pricing", "version": "1.1.0", "source": "git", "path": null, "url": url, "dir": null}));
    assert_eq!(v["lock"], serde_json::json!("written"));
    let before = (w.t.read("shop/ritsu.mod"), w.t.read("shop/ritsu.lock"));
    let (code, out, err) = w.run("shop", &["mod", "get", "pricing@9.9.9"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert_eq!(err, format!("error: the git repository `{url}` has no tag `v9.9.9` of `127.0.0.1/acme/pricing` 9.9.9\n"));
    assert_eq!((w.t.read("shop/ritsu.mod"), w.t.read("shop/ritsu.lock")), before);
    let (code, _, err) = w.run("shop", &["mod", "versions", &w.url("acme/nothing")]);
    assert_eq!(code, 2, "{err}");
}

/// A tag moved to another commit: `lock` and `fetch` keep the lock's commit (which the server still
/// gives), `get` takes the new one, and `report` says the version moved, with both commits. Once the
/// server no longer has the lock's commit, `fetch` says the tag moved (exit 1), in each language.
#[test]
fn a_tag_moved() {
    let Some(w) = World::new("git-moved") else { return };
    let (_tax, pricing) = w.tax_and_pricing();
    w.shop();
    let url = w.url("acme/pricing");
    w.ok("shop", &["mod", "add", "pricing", &format!("{url}@1.1.0")]);
    let was = pricing.head();
    let lock = w.t.read("shop/ritsu.lock");
    // the shop's own repository, its lock committed
    let shop = Command::new("git").current_dir(w.path("shop")).args(["init", "--quiet", "--initial-branch=main"]).env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
    assert!(shop.status.success());
    let commit_shop = |message: &str| {
        for args in [vec!["add", "-A"], vec!["-c", "user.name=ritsu-test", "-c", "user.email=ritsu-test@example.com", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", message]] {
            let o = Command::new("git").current_dir(w.path("shop")).args(&args).env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        }
    };
    commit_shop("pricing 1.1.0");
    // pricing 1.1.0 again, in a commit that replaces the one the tag led to
    let fee = std::fs::read_to_string(pricing.work.join("rules/shipping_fee.rule")).unwrap();
    pricing.write("rules/shipping_fee.rule", format!("# Published again.\n{fee}"));
    pricing.git(&["commit", "--quiet", "--amend", "-m", "pricing 1.1.0, again"]);
    pricing.git(&["tag", "-f", "-a", "-m", "release v1.1.0", "v1.1.0"]);
    let now = pricing.head();
    assert_ne!(was, now);
    std::fs::remove_dir_all(w.store()).unwrap();
    let out = w.ok("shop", &["mod", "fetch"]);
    assert!(out.contains(&format!("fetched 127.0.0.1/acme/pricing 1.1.0 (commit {}, sha256:", &was[..12])), "{out}");
    w.ok("shop", &["mod", "lock"]);
    assert_eq!(w.t.read("shop/ritsu.lock"), lock);
    assert_eq!(w.ok("shop", &["mod", "get", "pricing@1.1.0"]), "the dependency `pricing` is at 1.1.0 already\nwrote ritsu.lock: 2 modules\n");
    let moved = w.t.read("shop/ritsu.lock");
    assert!(moved.contains(&format!(" commit {now} \"{url}\"\n")), "{moved}");
    let v = json(&w.ok("shop", &["mod", "report", "--base", "HEAD", "--format", "json"]));
    let s = serde_json::to_string(&v).unwrap();
    assert!(s.contains(&was) && s.contains(&now), "{s}");
    let out = w.ok("shop", &["mod", "report", "--base", "HEAD"]);
    assert!(out.contains(&format!("(its commit went from {} to {}, its digest from sha256:", &was[..12], &now[..12])), "{out}");
    // the server without the lock's commit
    w.t.write("shop/ritsu.lock", &lock);
    pricing.git(&["reflog", "expire", "--expire=now", "--all"]);
    pricing.git(&["gc", "--quiet", "--prune=now"]);
    for (lang, want) in [
        ("en", format!("the tag `v1.1.0` of `127.0.0.1/acme/pricing` 1.1.0 was moved from the lock's commit {} to {}, and the git repository `{url}` does not give the lock's commit; see what changed, then run `ritsu mod get` to take it again", &was[..12], &now[..12])),
        ("ja", format!("`127.0.0.1/acme/pricing` 1.1.0 のタグ `v1.1.0` が、lock のコミット {} から {} に動かされていて、git のリポジトリ `{url}` は lock のコミットを渡しません。何が変わったかを確かめてから、`ritsu mod get` で取り直してください", &was[..12], &now[..12])),
    ] {
        std::fs::remove_dir_all(w.store()).unwrap();
        let (code, out, err) = w.run("shop", &["mod", "fetch", "--lang", lang]);
        assert_eq!((code, out.as_str()), (1, ""), "{err}");
        assert!(err.contains(&want), "{err}");
    }
}

/// A server that takes no filter, one of protocol version 0 that takes one, and one of version 0
/// that takes none: each gives the same commits, made into the same versions.
#[test]
fn servers_without_a_filter_and_of_protocol_version_0() {
    let Some(w) = World::new("git-servers") else { return };
    w.tax_and_pricing();
    w.shop();
    w.ok("shop", &["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))]);
    let pins = |lock: &str| -> Vec<String> { lock.lines().filter(|l| l.starts_with("git ")).map(|l| l.rsplit_once(" \"").unwrap().0.to_string()).collect() };
    let want = pins(&w.t.read("shop/ritsu.lock"));
    for (k, options) in [Options { filter: false, ..Options::hosted() }, Options { v0: true, ..Options::hosted() }, Options { filter: false, v0: true, basic: None }].into_iter().enumerate() {
        let s = GitServer::start(&w.path("git"), options.clone());
        let dir = format!("app-{k}");
        w.app(&dir);
        // tax from this server too: the module at hand decides where a module comes from
        w.ok(&dir, &["mod", "add", "tax", &format!("{}@1.0.0", s.url_of("acme/tax"))]);
        let store = w.path(&format!("store-{k}"));
        let o = w.command(&dir, &["mod", "add", "pricing", &format!("{}@1.1.0", s.url_of("acme/pricing"))], &[(store::VARIABLE, store.to_str().unwrap())]).output().unwrap();
        let (code, out, err) = said(o);
        assert_eq!(code, 0, "{options:?}: {out}{err}");
        assert_eq!(pins(&w.t.read(&format!("{dir}/ritsu.lock"))), want, "{options:?}");
        let (code, out, err) = w.run_env(&dir, &["check"], &[(store::VARIABLE, store.to_str().unwrap())]);
        assert_eq!(code, 0, "{options:?}: {out}{err}");
    }
}

/// A repository whose objects are named by SHA-256: its commits have 64 digits, in the lock and in
/// the git index.
#[test]
fn a_repository_of_sha256() {
    let Some(w) = World::new("git-sha256") else { return };
    let tax = w.s.repo_sha256("acme/tax");
    w.put_example(&tax, "tax");
    tax.commit("tax 1.0.0");
    tax.annotated("v1.0.0");
    assert_eq!(tax.head().len(), 64);
    w.app("shop");
    w.ok("shop", &["mod", "add", "tax", &format!("{}@1.0.0", w.url("acme/tax"))]);
    assert!(w.t.read("shop/ritsu.lock").contains(&format!(" commit {} \"{}\"\n", tax.head(), w.url("acme/tax"))), "{}", w.t.read("shop/ritsu.lock"));
    std::fs::remove_dir_all(w.store()).unwrap();
    let out = w.ok("shop", &["mod", "fetch"]);
    assert!(out.contains(&format!("(commit {}, sha256:", &tax.head()[..12])), "{out}");
    let index = std::fs::read_to_string(w.store().join("git/127.0.0.1/acme/tax/1.0.0")).unwrap();
    assert!(index.starts_with(&format!("{} sha256:", tax.head())), "{index}");
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
}

/// A commit that keeps a version from being a module: its version, what the commit changes, the
/// exit code of `add`, and what ritsu says in English and in Japanese.
type Case = (&'static str, Box<dyn Fn(&Repo)>, i32, &'static str, &'static str);

/// What a commit holds that keeps it from being a module, and what is not there: `add` stops (exit
/// 2, as for any tree that cannot be read), each said in each language, with the module's `ritsu.mod`
/// and lock left as they were and nothing in the store; `fetch` of a lock that pins such a commit
/// stops with exit 1.
#[test]
fn what_a_commit_holds_that_stops_it() {
    let Some(w) = World::new("git-refused") else { return };
    let (tax, _) = w.tax_and_pricing();
    let base = tax.head();
    w.app("shop");
    let url = w.url("acme/tax");
    // each case: a commit on main, tagged with its version
    let cases: Vec<Case> = vec![
        ("1.0.1", Box::new(|r: &Repo| {
            r.symlink("rules/link.rule", "vat_rate.rule");
        }), 2, "`rules/link.rule` of `127.0.0.1/acme/tax` 1.0.1 is a symbolic link; a module carries files only", "`127.0.0.1/acme/tax` 1.0.1 の `rules/link.rule` はシンボリックリンクです。モジュールが運ぶのはふつうのファイルだけです"),
        ("1.0.2", Box::new({
            let base = base.clone();
            move |r: &Repo| {
                r.submodule("rules/shared", &base);
            }
        }), 2, "`rules/shared` of `127.0.0.1/acme/tax` 1.0.2 is a submodule; a module carries files only, and what is in a submodule is not fetched", "`127.0.0.1/acme/tax` 1.0.2 の `rules/shared` はサブモジュールです。モジュールが運ぶのはふつうのファイルだけなので、サブモジュールの中は取りません"),
        ("1.0.3", Box::new(|r: &Repo| {
            r.write("rules/rates.rule", "version https://git-lfs.github.com/spec/v1\noid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\nsize 12345\n");
        }), 2, "`rules/rates.rule` of `127.0.0.1/acme/tax` 1.0.3 is a Git LFS pointer; ritsu does not fetch LFS files, so commit the files of a module without LFS", "`127.0.0.1/acme/tax` 1.0.3 の `rules/rates.rule` は Git LFS のポインタのファイルです。ritsu は LFS のファイルを取りません。モジュールのファイルは、LFS に入れずにコミットしてください"),
        ("1.0.4", Box::new(|r: &Repo| {
            r.write("rules/big.rule", vec![b'#'; 16 * 1024 * 1024 + 1]);
        }), 2, "`rules/big.rule` of `127.0.0.1/acme/tax` 1.0.4 is 16777217 bytes, and a file is 16777216 bytes at most", "`127.0.0.1/acme/tax` 1.0.4 の `rules/big.rule` は 16777217 バイトあります。一つのファイルは 16777216 バイトまでです"),
    ];
    for (version, change, code_wanted, en, ja) in &cases {
        tax.git(&["checkout", "--quiet", "--detach", &base]);
        let text = tax.work.join("ritsu.mod");
        let m = std::fs::read_to_string(&text).unwrap().replace("version 1.0.0", &format!("version {version}"));
        tax.write("ritsu.mod", m);
        change(&tax);
        tax.commit(&format!("tax {version}"));
        tax.tag(&format!("v{version}"));
        let before = (w.t.read("shop/ritsu.mod"), w.t.exists("shop/ritsu.lock"));
        for (lang, want) in [("en", en), ("ja", ja)] {
            let (code, out, err) = w.run("shop", &["mod", "add", "tax", &format!("{url}@{version}"), "--lang", lang]);
            assert_eq!((code, out.as_str()), (*code_wanted, ""), "{version} {lang}: {err}");
            assert!(err.contains(*want), "{version} {lang}: {err}");
            assert_eq!((w.t.read("shop/ritsu.mod"), w.t.exists("shop/ritsu.lock")), before);
        }
    }
    assert!(!w.store().join("sha256").exists() || std::fs::read_dir(w.store().join("sha256")).unwrap().next().is_none(), "the store has an entry");
    // the `ritsu.mod` of another module, or of another version
    tax.git(&["checkout", "--quiet", "--detach", &base]);
    tax.write("ritsu.mod", std::fs::read_to_string(tax.work.join("ritsu.mod")).unwrap().replace("version 1.0.0", "version 1.0.5").replace("module 127.0.0.1/acme/tax", "module 127.0.0.1/acme/other"));
    tax.commit("another module");
    tax.tag("v1.0.5");
    tax.git(&["checkout", "--quiet", "--detach", &base]);
    tax.write("ritsu.mod", std::fs::read_to_string(tax.work.join("ritsu.mod")).unwrap().replace("version 1.0.0", "version 1.0.7"));
    tax.commit("a version that its tag is not");
    tax.tag("v1.0.6");
    for (version, want) in [
        ("1.0.5", "error[ritsu E306]: 127.0.0.1/acme/tax@1.0.5/ritsu.mod:2:8: the `ritsu.mod` of `127.0.0.1/acme/tax` 1.0.5 says `module 127.0.0.1/acme/other`"),
        ("1.0.6", "error[ritsu E306]: 127.0.0.1/acme/tax@1.0.6/ritsu.mod:3: the `ritsu.mod` of the tag 1.0.6 of `127.0.0.1/acme/tax` has the `version` 1.0.7"),
    ] {
        let (code, out, err) = w.run("shop", &["mod", "add", "tax", &format!("{url}@{version}")]);
        assert_eq!((code, out.as_str()), (2, ""), "{version}: {err}");
        assert!(err.starts_with(want), "{version}: {err}");
    }
    // a tag that is not there (exit 2), and a directory with no module (exit 1)
    let (code, _, err) = w.run("shop", &["mod", "add", "tax", &format!("{url}@2.0.0")]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains(&format!("the git repository `{url}` has no tag `v2.0.0` of `127.0.0.1/acme/tax` 2.0.0")), "{err}");
    tax.git(&["checkout", "--quiet", "main"]);
    tax.tag("rules/v1.0.0");
    let (code, _, err) = w.run("shop", &["mod", "add", "tax", &format!("{url}@1.0.0"), "--dir", "rules"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("has no `ritsu.mod` in `rules`"), "{err}");
    let (code, _, err) = w.run("shop", &["mod", "add", "tax", &format!("{url}@1.0.0"), "--dir", "none"]);
    assert_eq!((code, err), (2, format!("error: the git repository `{url}` has no tag `none/v1.0.0` of `127.0.0.1/acme/tax/none` 1.0.0\n")));
    // a lock that pins a commit with a symbolic link in it, as version 1.0.0: fetch stops (exit 1)
    w.ok("shop", &["mod", "add", "tax", &format!("{url}@1.0.0")]);
    tax.git(&["checkout", "--quiet", "--detach", &base]);
    tax.symlink("rules/link.rule", "vat_rate.rule");
    let bad = tax.commit("a symbolic link");
    let lock = w.t.read("shop/ritsu.lock").replace(&base, &bad);
    w.t.write("shop/ritsu.lock", lock);
    std::fs::remove_dir_all(w.store()).unwrap();
    for (lang, want) in [("en", "`rules/link.rule` of `127.0.0.1/acme/tax` 1.0.0 is a symbolic link; a module carries files only"), ("ja", "`127.0.0.1/acme/tax` 1.0.0 の `rules/link.rule` はシンボリックリンクです")] {
        let (code, out, err) = w.run("shop", &["mod", "fetch", "--lang", lang]);
        assert_eq!((code, out.as_str()), (1, ""), "{err}");
        assert!(err.contains(want), "{err}");
    }
}

/// A URL ritsu does not take stops before git is started: a git of the test's own first on the PATH
/// writes down nothing. In `add`, and in a `require` that `lock` reads (E302, the URL with a
/// credential in it not shown); `--dir` with a source that is not a git URL stops too.
#[test]
fn a_url_refused_stops_before_git_starts() {
    let Some(w) = World::new("git-urls") else { return };
    w.shop();
    let env = w.wrap_git();
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    for (source, want) in [
        ("https://user:planted-s3cret@example.com/acme/rates@1.0.0", "the git URL has a user or a password in it"),
        ("file:///srv/rates@1.0.0", "a `file://` URL is not taken"),
        ("git://example.com/acme/rates@1.0.0", "a `git://` URL is not taken"),
        ("ext::ssh-proxy@1.0.0", "is of the form `ext::`"),
        ("http://example.com/acme/rates@1.0.0", "is plain http, which is taken for a server on this machine only"),
        ("https://example.com/Acme/rates@1.0.0", "has an uppercase letter in it"),
        ("git@-oProxyCommand=x:acme/rates@1.0.0", "the host of the git URL `git@-oProxyCommand=x:acme/rates` starts with a `-`"),
    ] {
        let (code, out, err) = w.run_env("shop", &["mod", "add", "rates", source], &env);
        assert_eq!((code, out.as_str()), (2, ""), "{source}: {err}");
        assert!(err.contains(want), "{source}: {err}");
        assert!(!err.contains("planted-s3cret"), "{err}");
    }
    // `--dir` goes with a git URL alone, whatever the source is
    for args in [["mod", "add", "tax", "../nowhere", "--dir", "rules"], ["mod", "add", "tax", "example.com/acme/tax@1.0.0", "--dir", "rules"]] {
        let (code, out, err) = w.run_env("shop", &args, &env);
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}: {err}");
        assert!(err.contains("`--dir` goes with a dependency from a git repository alone"), "{args:?}: {err}");
    }
    let (code, _, err) = w.run_env("shop", &["mod", "versions", "tax", "--dir", "rules"], &env);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("`--dir` goes with a git URL alone"), "{err}");
    let text = w.t.read("shop/ritsu.mod");
    w.t.write("shop/ritsu.mod", format!("{text}\nrequire rates git \"https://user:planted-s3cret@example.com/acme/rates\" 1.0.0\n"));
    let (code, out, err) = w.run_env("shop", &["mod", "lock"], &env);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.starts_with("error[ritsu E302]: ritsu.mod:8:19: the git URL has a user or a password in it") && !err.contains("planted-s3cret") && !err.contains("example.com"), "{err}");
    assert_eq!(w.git_calls(), Vec::<String>::new());
}

/// What git is run with: each call names its temporary repository (`--git-dir`) and the settings,
/// puts `--end-of-options` before a URL or a commit, and has `GIT_ALLOW_PROTOCOL` (with http, for a
/// server on the loopback), `GIT_TERMINAL_PROMPT=0` and `GIT_NO_LAZY_FETCH=1`, and no `GIT_DIR`,
/// `GIT_INDEX_FILE` or `GIT_OBJECT_DIRECTORY` though ritsu was given them.
#[test]
fn what_git_is_run_with() {
    let Some(w) = World::new("git-env") else { return };
    w.tax_and_pricing();
    w.shop();
    let mut env = w.wrap_git();
    let victim = w.path("victim/.git").display().to_string();
    env.extend([("GIT_DIR".to_string(), victim.clone()), ("GIT_INDEX_FILE".to_string(), format!("{victim}/index")), ("GIT_OBJECT_DIRECTORY".to_string(), format!("{victim}/objects"))]);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (code, out, err) = w.run_env("shop", &["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))], &env);
    assert_eq!(code, 0, "{out}{err}");
    let calls = w.git_calls();
    assert!(calls.len() >= 8, "{calls:#?}");
    let mut asked = 0;
    for pair in calls.chunks(2) {
        let (call, vars) = (&pair[0], &pair[1]);
        assert!(vars.contains("GIT_DIR=unset GIT_INDEX_FILE=unset GIT_OBJECT_DIRECTORY=unset"), "{call}\n{vars}");
        if call == "git version" {
            continue;
        }
        assert!(call.contains("-c core.hooksPath=/dev/null -c protocol.version=2 -c transfer.fsckObjects=true -c gc.auto=0 -c maintenance.auto=false "), "{call}");
        if call.contains(" init --bare ") {
            continue;
        }
        assert!(call.starts_with("git --git-dir="), "{call}");
        assert!(vars.contains("GIT_ALLOW_PROTOCOL=https:ssh:http GIT_TERMINAL_PROMPT=0 GIT_NO_LAZY_FETCH=1"), "{call}\n{vars}");
        if call.contains(" ls-remote ") || call.contains(" fetch ") || call.contains(" ls-tree ") {
            assert!(call.contains(" --end-of-options "), "{call}");
            asked += 1;
        }
    }
    assert!(asked >= 4, "{calls:#?}");
}

/// A repository `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and `GIT_OBJECT_DIRECTORY` point at
/// (as in a hook of a repository) is left as it was by `add`, `fetch` and `lock`.
#[test]
fn a_repository_git_is_pointed_at_is_left_as_it_was() {
    let Some(w) = World::new("git-pointed") else { return };
    w.tax_and_pricing();
    w.shop();
    let victim = w.s.repo("victim");
    victim.write("README.md", "a repository of its own\n");
    victim.commit("first");
    let before = snapshot(&victim.served);
    let work_before = snapshot(&victim.work);
    let gd = victim.served.display().to_string();
    let wt = victim.work.display().to_string();
    let env = [("GIT_DIR", gd.as_str()), ("GIT_WORK_TREE", wt.as_str()), ("GIT_INDEX_FILE", &format!("{gd}/index")), ("GIT_OBJECT_DIRECTORY", &format!("{gd}/objects"))];
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, *v)).collect();
    for args in [vec!["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))], vec!["mod", "lock"], vec!["mod", "fetch"]] {
        let (code, out, err) = w.run_env("shop", &args.iter().map(|s| s.as_ref()).collect::<Vec<&str>>(), &env);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
        std::fs::remove_dir_all(w.store()).ok();
    }
    assert_eq!(snapshot(&victim.served), before);
    assert_eq!(snapshot(&victim.work), work_before);
}

/// Neither the hooks of the person's `core.hooksPath` nor a filter's driver runs, though git runs
/// both for a fetch that writes a ref and for a checkout (said first, with the same configuration):
/// the files are the bytes git holds.
#[test]
fn hooks_and_filters_do_not_run() {
    let Some(w) = World::new("git-hooks") else { return };
    let (tax, _) = w.tax_and_pricing();
    tax.write(".gitattributes", "* filter=mark\n");
    tax.write("ritsu.mod", std::fs::read_to_string(tax.work.join("ritsu.mod")).unwrap().replace("version 1.0.0", "version 1.0.1"));
    tax.commit("a filter for every file");
    tax.tag("v1.0.1");
    let hooks = w.path("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let ran = w.path("ran.log");
    for h in ["reference-transaction", "post-checkout", "post-merge", "post-index-change", "pre-auto-gc"] {
        let p = w.t.write(&format!("hooks/{h}"), format!("#!/bin/sh\necho {h} >> '{}'\n", ran.display()));
        executable(&p);
    }
    let mark = w.t.write("bin/mark", format!("#!/bin/sh\necho \"filter $1\" >> '{}'\ncat\n", ran.display()));
    executable(&mark);
    w.t.write("gitconfig", format!("[core]\n\thooksPath = {}\n[filter \"mark\"]\n\tsmudge = {} smudge\n\tclean = {} clean\n", hooks.display(), mark.display(), mark.display()));
    // git runs both, with this configuration
    let o = Command::new("git")
        .args(["clone", "--quiet", "--branch", "v1.0.1", &w.url("acme/tax")])
        .arg(w.path("checkout"))
        .env("GIT_CONFIG_GLOBAL", w.path("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let said = std::fs::read_to_string(&ran).unwrap_or_default();
    assert!(said.contains("reference-transaction") && said.contains("filter smudge"), "{said}");
    std::fs::remove_file(&ran).unwrap();
    // ritsu runs neither
    w.app("shop");
    w.ok("shop", &["mod", "add", "tax", &format!("{}@1.0.1", w.url("acme/tax"))]);
    std::fs::remove_dir_all(w.store()).unwrap();
    w.ok("shop", &["mod", "fetch"]);
    assert!(!ran.exists(), "{}", std::fs::read_to_string(&ran).unwrap_or_default());
    let v = json(&w.ok("shop", &["check", "--format", "json"]));
    assert_eq!(v["dependencies"][0]["ok"], serde_json::json!(true), "{v}");
}

/// ssh, with an ssh of the test's own that writes down how git calls it and runs `git upload-pack`
/// here: the URL's user, host, port and path as git gives them (`ssh://` absolute, the scp form from
/// the home), `GIT_PROTOCOL` sent on, and `GIT_ALLOW_PROTOCOL` that has no http.
#[test]
fn ssh_is_called_as_git_calls_it() {
    let Some(w) = World::new("git-ssh") else { return };
    let tax = w.s.repo("acme/ssh-tax");
    w.put_example(&tax, "tax");
    tax.write("ritsu.mod", std::fs::read_to_string(tax.work.join("ritsu.mod")).unwrap().replace("module 127.0.0.1/acme/tax", "module example.com/acme/ssh-tax"));
    tax.commit("tax 1.0.0");
    tax.tag("v1.0.0");
    w.app("shop");
    let ssh = w.t.write("bin/ssh", FAKE_SSH);
    executable(&ssh);
    let mut env = w.wrap_git();
    env.extend([
        ("GIT_SSH_COMMAND".to_string(), ssh.display().to_string()),
        ("FAKE_SSH_LOG".to_string(), w.path("ssh.log").display().to_string()),
        ("FAKE_SSH_ROOT".to_string(), w.path("git/served").display().to_string()),
    ]);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (code, out, err) = w.run_env("shop", &["mod", "add", "tax", "ssh://git@example.com:2222/acme/ssh-tax.git@1.0.0"], &env);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "added `require tax git \"ssh://git@example.com:2222/acme/ssh-tax.git\" 1.0.0` to ritsu.mod (example.com/acme/ssh-tax 1.0.0)\nwrote ritsu.lock: 1 module\n");
    let (code, out, err) = w.run_env("shop", &["mod", "versions", "git@example.com:acme/ssh-tax.git"], &env);
    assert_eq!((code, out.as_str()), (0, "1.0.0\n"), "{err}");
    let calls = std::fs::read_to_string(w.path("ssh.log")).unwrap();
    let lines: Vec<&str> = calls.lines().collect();
    assert!(lines.contains(&"-o SendEnv=GIT_PROTOCOL -p 2222 git@example.com git-upload-pack '/acme/ssh-tax.git'"), "{calls}");
    assert!(lines.contains(&"-o SendEnv=GIT_PROTOCOL git@example.com git-upload-pack 'acme/ssh-tax.git'"), "{calls}");
    assert!(w.git_calls().iter().filter(|l| l.contains("GIT_ALLOW_PROTOCOL=")).all(|l| l.contains("GIT_ALLOW_PROTOCOL=https:ssh ") || l.contains("GIT_ALLOW_PROTOCOL=unset")), "{:#?}", w.git_calls());
    std::fs::remove_dir_all(w.store()).unwrap();
    let (code, out, err) = w.run_env("shop", &["mod", "fetch"], &env);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
}

/// `vendor` copies the versions from git into the module (no git index there), and `check` reads
/// them with an empty store; `vendor --check` holds the copy to the lock; `where` and `list` say where
/// each is and where it comes from (golden files); `clean` removes the entries no lock keeps, and
/// their lines of the git index.
#[test]
fn vendor_clean_where_and_list() {
    let Some(w) = World::new("git-vendor") else { return };
    let (_tax, pricing) = w.tax_and_pricing();
    w.shop();
    w.ok("shop", &["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))]);
    let mut failures = Vec::new();
    for (args, name) in [(vec!["mod", "list"], "git-list.en.txt"), (vec!["mod", "list", "--lang", "ja"], "git-list.ja.txt"), (vec!["mod", "list", "--format", "json"], "git-list.json")] {
        golden(name, &w.masked(&w.ok("shop", &args)), &mut failures);
    }
    let out = w.ok("shop", &["mod", "where", "@pricing/rules/order_total.rule"]);
    assert!(out.starts_with(&w.store().join("sha256").display().to_string()) && out.trim_end().ends_with("/files/rules/order_total.rule"), "{out}");
    assert_eq!(w.ok("shop", &["mod", "vendor"]), "ritsu mod vendor: .ritsu/modules holds the 2 modules of the lock (2 placed, 0 removed)\n");
    assert!(w.path("shop/.ritsu/modules/sha256").is_dir() && !w.path("shop/.ritsu/modules/git").exists() && !w.path("shop/.ritsu/modules/refs").exists());
    w.ok("shop", &["mod", "vendor", "--check"]);
    let empty = w.path("empty-store");
    let (code, out, err) = w.run_env("shop", &["check"], &[(store::VARIABLE, empty.to_str().unwrap())]);
    assert_eq!(code, 0, "{out}{err}");
    // down to 1.0.0: 1.1.0 is in no lock
    w.ok("shop", &["mod", "get", "pricing@1.0.0"]);
    let lock = w.path("shop/ritsu.lock").display().to_string();
    let out = w.ok("shop", &["mod", "clean", "--keep", &lock]);
    assert!(out.contains("(127.0.0.1/acme/pricing 1.1.0)") && out.ends_with("ritsu mod clean: removed 1 entry of the store, kept 2\n"), "{out}");
    assert!(!w.store().join("git/127.0.0.1/acme/pricing/1.1.0").exists());
    let index = std::fs::read_to_string(w.store().join("git/127.0.0.1/acme/pricing/1.0.0")).unwrap();
    assert!(index.starts_with(&tagged(&pricing, "v1.0.0")), "{index}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `diff <name>@<version>` of a dependency from git (the commits at the head), `fetch --rev` of a
/// lock in a git revision, and `push`, which refuses a module that requires one from git (exit 1).
#[test]
fn diff_fetch_rev_and_push() {
    let Some(w) = World::new("git-diff") else { return };
    let (_tax, pricing) = w.tax_and_pricing();
    w.shop();
    w.ok("shop", &["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))]);
    let (code, out, err) = w.run("shop", &["mod", "diff", "127.0.0.1/acme/pricing@1.0.0", "127.0.0.1/acme/pricing@1.1.0", "--format", "json"]);
    assert_eq!(code, 1, "{out}{err}");
    let v = json(&out);
    assert_eq!((&v["old"]["commit"], &v["new"]["commit"]), (&serde_json::json!(tagged(&pricing, "v1.0.0")), &serde_json::json!(tagged(&pricing, "v1.1.0"))));
    let s = serde_json::to_string(&v).unwrap();
    assert!(s.contains(&tagged(&pricing, "v1.0.0")) && s.contains(&tagged(&pricing, "v1.1.0")), "{s}");
    let (code, out, err) = w.run("shop", &["mod", "diff", "127.0.0.1/acme/pricing@1.0.0", "127.0.0.1/acme/pricing@1.1.0"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.starts_with("ritsu mod diff: 127.0.0.1/acme/pricing 1.0.0 → 1.1.0\n") && !out.contains("not compared"), "{out}");
    // fetch --rev: the lock of a revision
    let git = |args: &[&str]| {
        let o = Command::new("git").current_dir(w.path("shop")).args(["-c", "user.name=ritsu-test", "-c", "user.email=ritsu-test@example.com", "-c", "commit.gpgsign=false"]).args(args).env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    git(&["add", "-A"]);
    git(&["commit", "--quiet", "-m", "pricing 1.1.0"]);
    w.ok("shop", &["mod", "get", "pricing@1.0.0"]);
    std::fs::remove_dir_all(w.store()).unwrap();
    let out = w.ok("shop", &["mod", "fetch", "--rev", "HEAD"]);
    assert!(out.contains(&format!("fetched 127.0.0.1/acme/pricing 1.1.0 (commit {}", &tagged(&pricing, "v1.1.0")[..12])), "{out}");
    // push
    let text = w.t.read("shop/ritsu.mod");
    w.t.write("shop/ritsu.mod", text.replace("version 0.1.0\n", &format!("version 0.1.0\nritsu >={}\n", env!("CARGO_PKG_VERSION"))));
    let (code, out, err) = w.run("shop", &["mod", "push", "--dry-run"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains(&format!("is `require pricing git \"{}\" 1.0.0`, and a module with a dependency from git cannot be pushed to a registry", w.url("acme/pricing"))), "{err}");
}

/// One place for each module in a tree: the module at hand decides between two URLs of one name;
/// two dependencies that disagree, with nothing said by the module at hand, are E306; a path wins.
/// A `signature` line over a module from git is E309 for `fetch` and `check` (exit 1); `lock`, which
/// checks no signature, writes the lock.
#[test]
fn where_a_module_comes_from_is_one_place() {
    let Some(w) = World::new("git-origins") else { return };
    let (tax, _) = w.tax_and_pricing();
    let other = w.s.repo("acme/other");
    other.write("ritsu.mod", format!("module 127.0.0.1/acme/other\nversion 1.0.0\n\nrequire tax git \"{}\" 1.0.0\n", w.url("acme/tax.git")));
    other.commit("other 1.0.0");
    other.tag("v1.0.0");
    w.shop();
    // the module at hand names tax at another URL: that one
    w.ok("shop", &["mod", "add", "tax", &format!("{}@1.0.0", w.url("acme/tax.git"))]);
    w.ok("shop", &["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))]);
    let lock = w.t.read("shop/ritsu.lock");
    assert!(lock.contains(&format!(" commit {} \"{}\"\n", tax.head(), w.url("acme/tax.git"))), "{lock}");
    // two dependencies that disagree, the module at hand saying nothing
    w.ok("shop", &["mod", "remove", "tax"]);
    let (code, out, err) = w.run("shop", &["mod", "add", "other", &format!("{}@1.0.0", w.url("acme/other"))]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.contains("E306") && err.contains("the module `127.0.0.1/acme/tax` comes from two sources"), "{err}");
    // a path wins
    ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/modules/tax"), &w.path("tax"));
    let m = w.t.read("tax/ritsu.mod").replace("module example.com/acme/tax", "module 127.0.0.1/acme/tax");
    w.t.write("tax/ritsu.mod", m);
    w.ok("shop", &["mod", "add", "tax", "../tax"]);
    w.ok("shop", &["mod", "add", "other", &format!("{}@1.0.0", w.url("acme/other"))]);
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("by a path") || out.contains("by a path") || out.contains("a git repository"), "{out}{err}");
    // a signature line over the modules from git
    w.ok("shop", &["mod", "remove", "other"]);
    let text = w.t.read("shop/ritsu.mod");
    w.t.write("cosign.pub", "-----BEGIN PUBLIC KEY-----\n-----END PUBLIC KEY-----\n");
    w.t.write("shop/cosign.pub", "-----BEGIN PUBLIC KEY-----\n-----END PUBLIC KEY-----\n");
    w.t.write("shop/ritsu.mod", format!("{text}signature \"127.0.0.1/acme/\" key \"cosign.pub\"\n"));
    w.ok("shop", &["mod", "lock"]);
    std::fs::remove_dir_all(w.store()).unwrap();
    let (code, out, err) = w.run("shop", &["mod", "fetch"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    let line = w.t.read("shop/ritsu.mod").lines().count();
    assert!(err.starts_with(&format!("error[ritsu E309]: ritsu.mod:{line}: the start `127.0.0.1/acme/` of a `signature` line covers `127.0.0.1/acme/")), "{err}");
    assert!(!w.store().join("sha256").exists(), "fetch stopped before it put anything in the store");
    w.t.write("shop/ritsu.mod", &text);
    w.ok("shop", &["mod", "fetch"]);
    w.t.write("shop/ritsu.mod", format!("{text}signature \"127.0.0.1/acme/\" key \"cosign.pub\"\n"));
    let (code, out, err) = w.run("shop", &["check"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(format!("{out}{err}").contains("E309"), "{out}{err}");
}

const USER: &str = "ritsu-fake-user";
const PASSWORD: &str = "ritsu-fake-git-password-5d1e";

/// A credential helper of the test's own: the planted user and password (`HELPER_PASSWORD`, else
/// the one the server takes) for a `get`, nothing for the rest.
const HELPER: &str = "#!/bin/sh\n[ \"$1\" = get ] || exit 0\ncat > /dev/null\necho username=ritsu-fake-user\necho \"password=${HELPER_PASSWORD:-ritsu-fake-git-password-5d1e}\"\n";

/// Every file under the test's directory but the configuration and the helper that hold the planted
/// secrets, with any of `secrets` in it.
fn holding(w: &World, secrets: &[&str]) -> Vec<PathBuf> {
    files_under(w.t.path())
        .into_iter()
        .filter(|p| !p.ends_with("gitconfig") && !p.ends_with("bin/helper"))
        .filter(|p| {
            let b = std::fs::read(p).unwrap_or_default();
            secrets.iter().any(|s| b.windows(s.len()).any(|x| x == s.as_bytes()))
        })
        .collect()
}

/// The credentials of a git server that asks for them (`Basic`), from git's credential helper and
/// from an `insteadOf` URL that carries them: git sends them, and every secret the test plants
/// (`ritsu-fake-…`, and the base64 of the user and the password) is nowhere else: not on the
/// standard output or error (an error of git's that names the URL too), not in the JSON, not in
/// `ritsu.mod`, the lock or the store, not in the arguments git was given.
#[test]
fn credentials_go_to_git_and_nowhere_else() {
    let Some(w) = World::with("git-credentials", Options { basic: Some((USER.to_string(), PASSWORD.to_string())), ..Options::hosted() }) else { return };
    let (_tax, _pricing) = {
        // the repositories are made on the disk; their locks need no credential (the setup asks the
        // server of the test with the helper)
        let helper = w.t.write("bin/helper", HELPER);
        executable(&helper);
        w.t.write("gitconfig", format!("[credential]\n\thelper = {}\n", helper.display()));
        w.tax_and_pricing()
    };
    let basic = ritsu_testkit::registry::base64_encode(format!("{USER}:{PASSWORD}").as_bytes());
    let secrets = [PASSWORD, basic.as_str()];
    let mut said = String::new();
    let env = w.wrap_git();
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    // the helper
    w.shop();
    for args in [vec!["mod", "add", "pricing", &format!("{}@1.1.0", w.url("acme/pricing"))], vec!["mod", "fetch", "--format", "json"], vec!["mod", "list", "--format", "json"], vec!["check", "--format", "json"]] {
        let args: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
        let (code, out, err) = w.run_env("shop", &args, &env);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
        said.push_str(&out);
        said.push_str(&err);
    }
    assert!(w.s.log().iter().any(|s| s.authorization.as_deref() == Some(&format!("Basic {basic}"))), "git sent no credentials");
    // `insteadOf`, with the user and the password in the URL it gives
    let at = w.s.url.replacen("http://", &format!("http://{USER}:{PASSWORD}@"), 1);
    w.t.write("gitconfig", format!("[url \"{at}/\"]\n\tinsteadOf = {}/\n", w.s.url));
    std::fs::remove_dir_all(w.store()).unwrap();
    let (code, out, err) = w.run_env("shop", &["mod", "fetch"], &env);
    assert_eq!(code, 0, "{out}{err}");
    said.push_str(&out);
    said.push_str(&err);
    // a repository that is not there: git's error names the URL
    for lang in ["en", "ja"] {
        let (code, out, err) = w.run_env("shop", &["mod", "versions", &w.url("acme/nothing"), "--lang", lang], &env);
        assert_eq!(code, 2, "{out}{err}");
        assert!(err.contains("acme/nothing"), "{err}");
        said.push_str(&out);
        said.push_str(&err);
    }
    for s in secrets {
        assert!(!said.contains(s), "{s} was said:\n{said}");
    }
    assert_eq!(holding(&w, &secrets), Vec::<PathBuf>::new());
}

/// A server that asks for credentials, and none at hand: git does not prompt (`GIT_TERMINAL_PROMPT`
/// is 0), and the command stops at once (exit 2); a password the server refuses stops it the same,
/// the password not said.
#[test]
fn no_credential_stops_at_once() {
    let Some(w) = World::with("git-no-credential", Options { basic: Some((USER.to_string(), PASSWORD.to_string())), ..Options::hosted() }) else { return };
    let tax = w.s.repo("acme/tax");
    w.put_example(&tax, "tax");
    tax.commit("tax 1.0.0");
    tax.tag("v1.0.0");
    w.app("app");
    let url = w.url("acme/tax");
    let started = std::time::Instant::now();
    let (code, out, err) = w.run("app", &["mod", "add", "tax", &format!("{url}@1.0.0")]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.starts_with(&format!("error: cannot read the tags of the git repository `{url}`; git said:\n")), "{err}");
    assert!(started.elapsed() < std::time::Duration::from_secs(60), "{:?}", started.elapsed());
    let helper = w.t.write("bin/helper", HELPER);
    executable(&helper);
    w.t.write("gitconfig", format!("[credential]\n\thelper = {}\n", helper.display()));
    let wrong = "ritsu-fake-wrong-password-0a9b";
    let (code, out, err) = w.run_env("app", &["mod", "add", "tax", &format!("{url}@1.0.0"), "--lang", "ja"], &[("HELPER_PASSWORD", wrong)]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.starts_with(&format!("エラー: git のリポジトリ `{url}` のタグを読めません。git の出力：\n")), "{err}");
    assert!(!err.contains(wrong), "{err}");
    assert!(!w.t.read("app/ritsu.mod").contains("require"));
}

/// A site whose list names a module of a git repository (`git "<URL>"`): its versions are its tags,
/// and the page of each says the commit, the digest and how to require it from git. A `signature`
/// line that covers it makes the list one that cannot be read (exit 2), and nothing is written.
#[test]
fn a_site_lists_a_module_from_git() {
    let Some(w) = World::new("git-site") else { return };
    let (_tax, pricing) = w.tax_and_pricing();
    let url = w.url("acme/pricing");
    w.t.write("acme.list", format!("title \"The modules of Acme\"\ngit \"{url}\"\n"));
    let (code, out, err) = w.run("", &["mod", "site", "acme.list", "--out", "out"]);
    assert_eq!(code, 0, "{out}{err}");
    let pages: Vec<String> = files_under(&w.path("out")).iter().map(|p| p.strip_prefix(w.path("out")).unwrap().display().to_string()).filter(|p| p.ends_with(".html")).collect();
    let text = |p: &str| std::fs::read_to_string(w.path("out").join(p)).unwrap_or_default();
    let all: String = pages.iter().map(|p| text(p)).collect();
    assert!(all.contains(&tagged(&pricing, "v1.0.0")) && all.contains(&tagged(&pricing, "v1.1.0")), "{pages:#?}");
    assert!(all.contains(&format!("require pricing git &quot;{url}&quot; 1.1.0")) || all.contains(&format!("require pricing git \"{url}\" 1.1.0")), "{pages:#?}");
    // a signature line over it
    w.t.write("cosign.pub", "-----BEGIN PUBLIC KEY-----\n-----END PUBLIC KEY-----\n");
    w.t.write("signed.list", format!("git \"{url}\"\nsignature \"127.0.0.1/acme/\" key \"cosign.pub\"\n"));
    let (code, out, err) = w.run("", &["mod", "site", "signed.list", "--out", "out2"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.contains("the start `127.0.0.1/acme/` of a `signature` line covers `127.0.0.1/acme/pricing`, a module from a git repository"), "{err}");
    assert!(!w.path("out2").exists());
}
