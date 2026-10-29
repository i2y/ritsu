//! git, run for the dependencies from git repositories (DESIGN 17.12.1): the tags of a repository
//! (`git ls-remote`), and a version made from the commit its tag led to, without a checkout.
//!
//! Every git runs in a temporary bare repository under the store's `tmp/`, one for each URL (and
//! hash) in a run, removed when the run is done: `--git-dir` names it and the git runs in it, so that
//! the configuration of the repository ritsu was started in is not read. The variables that point
//! git at a repository (those `git rev-parse --local-env-vars` lists) are taken away, since ritsu
//! started from a hook of the repository at hand would have them; git is given
//! `GIT_ALLOW_PROTOCOL=https:ssh` (`:http` for a host of the loopback), `GIT_TERMINAL_PROMPT=0` and
//! `GIT_NO_LAZY_FETCH=1`, and `-c core.hooksPath=/dev/null -c protocol.version=2 -c
//! transfer.fsckObjects=true -c gc.auto=0 -c maintenance.auto=false` (the `git init` of the
//! temporary repository too, whose `HEAD` would run the person's reference-transaction hook). The
//! person's own global and system configuration is read: their credential helpers, `insteadOf` and
//! proxies work as they do for `git fetch`. `--end-of-options` comes before every URL and commit;
//! the standard input is closed but for `--stdin`. A git that runs over 300 s is stopped.
//!
//! A version is made in these steps: the commit and its trees, without the blobs
//! (`--filter=blob:none --depth=1`); the tree of the module (`ls-tree -r -t -z`); its `ritsu.mod`,
//! whose `module` and `version` have to be the name and the version asked for and whose `exclude`
//! lines apply; the files it carries, chosen by the rule the files of a directory are
//! ([`digest::carried_of_tree`]), their blobs fetched at once (`--stdin`); their bytes as git holds
//! them (`cat-file --batch`, with no filter and no change of the ends of the lines). A server that
//! does not take a filter or a blob by its name (protocol version 0) has the commit fetched whole
//! instead, with `--no-filter`: a fetch with a filter makes git keep the filter for the URL, and
//! use it for the next fetch from there. A symbolic link, a submodule and a Git LFS pointer stop it
//! (exit 1).
//!
//! Under WASI, git cannot be started: each command that needs it says so (`wasi::cannot_start("git")`,
//! DESIGN 17.22).

use super::store_write;
use ritsu_base::modules::artifact;
use ritsu_base::modules::digest;
use ritsu_base::modules::git::{self as base, Url};
use ritsu_base::modules::manifest::{self, Manifest};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::{self, Parts, Record, Store};
use ritsu_base::modules::tar;
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The variables that point git at a repository: those `git rev-parse --local-env-vars` lists.
pub const LOCAL_VARS: [&str; 15] = [
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
];

/// The settings every git is run with.
const SETTINGS: [&str; 5] = ["core.hooksPath=/dev/null", "protocol.version=2", "transfer.fsckObjects=true", "gc.auto=0", "maintenance.auto=false"];

/// The oldest git ritsu runs: `fetch --stdin` and `--no-write-fetch-head` came in 2.29.
pub const OLDEST: (u32, u32) = (2, 29);

/// How long one git may run.
const TIMEOUT: Duration = Duration::from_secs(300);

/// The lines of what git says on its standard error that a message quotes.
const SAID_LINES: usize = 20;

/// Why git did not give what was asked.
#[derive(Debug)]
pub enum Failed {
    /// What the repository holds is not what it has to be (exit 1).
    Refused(Text),
    /// The version's `ritsu.mod` names another module or another version (exit 1): its bytes, for
    /// the E306 `ritsu mod lock` says at its line (DESIGN 17.12.1), and the sentence.
    Mismatch(Vec<u8>, Text),
    /// git cannot be run, the repository cannot be asked, or it has no such tag (exit 2).
    Cannot(Text),
}

impl Failed {
    pub fn text(self) -> Text {
        match self {
            Failed::Refused(t) | Failed::Mismatch(_, t) | Failed::Cannot(t) => t,
        }
    }
}

/// A version made from a commit: the blobs of its OCI artifact, and the files.
pub struct Made {
    pub built: artifact::Built,
    pub files: Vec<(String, Vec<u8>)>,
}

impl Made {
    /// The made version put in `store` (DESIGN 17.15), as a fetch puts one: its entry, whose record
    /// names the URL and the commit, and the version's line of the git index (not `refs/`). The
    /// digest of its OCI manifest.
    pub fn put(&self, store: &Store, name: &ModuleName, version: &Version, url: &str, commit: &str) -> Result<String, Text> {
        let record = record(name, version, url, commit, &self.built.files_digest, Vec::new());
        put(store, &Parts { manifest: &self.built.manifest, module: &self.built.module, files: Some(&self.files), record }, name, version, commit)
    }
}

/// The record of a version made from the commit `commit` of `url`, fetched today.
pub fn record(name: &ModuleName, version: &Version, url: &str, commit: &str, files_digest: &str, verified: Vec<(String, String)>) -> Record {
    Record {
        module: name.to_string(),
        version: version.to_string(),
        files: files_digest.to_string(),
        fetched: store_write::today(),
        verified,
        git: Some((url.to_string(), commit.to_string())),
    }
}

/// The entry of a version made from a git commit put in `store`, and the version's line of the git
/// index: the digest of its OCI manifest.
pub fn put(store: &Store, parts: &Parts<'_>, name: &ModuleName, version: &Version, commit: &str) -> Result<String, Text> {
    let d = store::place(store, parts, None)?;
    store::put_git_index(store, name, version, commit, &d)?;
    Ok(d)
}

/// What a git that ran gave.
struct Ran {
    ok: bool,
    stdout: Vec<u8>,
    stderr: String,
}

/// The versions the tags of a module give, each with the commit its tag leads to, by the URL and
/// the `dir`.
type Tags = BTreeMap<(String, Option<String>), Vec<(Version, String)>>;

/// The git of one run of ritsu: its temporary repositories, and what it has learned.
pub struct Git {
    /// The temporary repositories, by the URL and whether the hash is SHA-256.
    repos: BTreeMap<(String, bool), PathBuf>,
    /// The tags of each URL, as `ls-remote` gave them.
    tags: Tags,
    checked: bool,
}

impl Drop for Git {
    fn drop(&mut self) {
        for d in self.repos.values() {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

impl Default for Git {
    fn default() -> Git {
        Git::new()
    }
}

/// The first lines of what git said, the users and passwords of URLs hidden.
fn quoted(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).take(SAID_LINES).collect();
    base::redacted(&lines.join("\n"))
}

impl Git {
    pub fn new() -> Git {
        Git { repos: BTreeMap::new(), tags: BTreeMap::new(), checked: false }
    }

    /// git's version, held to [`OLDEST`], once a run.
    fn check(&mut self) -> Result<(), Failed> {
        if self.checked {
            return Ok(());
        }
        if cfg!(target_os = "wasi") {
            return Err(Failed::Cannot(ritsu_base::wasi::cannot_start("git")));
        }
        let out = Command::new("git").arg("version").stdin(Stdio::null()).env_clear_local().output();
        let o = match out {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Failed::Cannot(tr!(
                    "git がありません。git のリポジトリから取る依存があるので、git を入れて PATH に置いてください",
                    "git is not there, and a dependency comes from a git repository; install git and put it on the PATH"
                )));
            }
            Err(e) => return Err(Failed::Cannot(ritsu_base::wasi::starting("git", &e))),
        };
        let said = String::from_utf8_lossy(&o.stdout).trim().to_string();
        let (major, minor) = version_of(&said).unwrap_or((0, 0));
        if (major, minor) < OLDEST {
            let (a, b) = OLDEST;
            return Err(Failed::Cannot(tr!(
                "git が古すぎます（`{said}`）。git の依存には git {a}.{b} 以上が要ります",
                "git is too old (`{said}`): a dependency from git needs git {a}.{b} or later"
            )));
        }
        self.checked = true;
        Ok(())
    }

    /// The temporary repository of `url` for a hash: made the first time it is asked for.
    fn repo(&mut self, url: &str, sha256: bool) -> Result<PathBuf, Failed> {
        self.check()?;
        if let Some(d) = self.repos.get(&(url.to_string(), sha256)) {
            return Ok(d.clone());
        }
        let tmp = store_write::scratch().map_err(Failed::Cannot)?;
        let dir = ritsu_base::paths::absolute(&store_write::unique(&tmp, "git"));
        let mut c = Command::new("git");
        c.env_clear_local().current_dir(&tmp);
        for s in SETTINGS {
            c.args(["-c", s]);
        }
        c.args(["init", "--bare", "--quiet", "--template="]);
        if sha256 {
            c.arg("--object-format=sha256");
        }
        c.arg(&dir);
        let ran = run(c, None)?;
        self.repos.insert((url.to_string(), sha256), dir.clone());
        if !ran.ok {
            let said = quoted(&ran.stderr);
            return Err(Failed::Cannot(tr!("git の一時のリポジトリを作れません。git の出力：\n{said}", "cannot make git's temporary repository; git said:\n{said}")));
        }
        Ok(dir)
    }

    /// git, in the temporary repository `dir`, set as every git of the dependencies is.
    fn command(dir: &Path, url: &Url) -> Command {
        let mut c = Command::new("git");
        c.env_clear_local();
        c.env("GIT_ALLOW_PROTOCOL", if url.loopback() { "https:ssh:http" } else { "https:ssh" });
        c.env("GIT_TERMINAL_PROMPT", "0");
        c.env("GIT_NO_LAZY_FETCH", "1");
        c.arg(format!("--git-dir={}", dir.display()));
        for s in SETTINGS {
            c.args(["-c", s]);
        }
        c.current_dir(dir);
        c
    }

    /// The versions the tags of the module at `dir` of the repository `url` give, each with the
    /// commit its tag leads to, in the order of the versions.
    pub fn versions(&mut self, url: &str, dir: Option<&str>) -> Result<Vec<(Version, String)>, Failed> {
        let key = (url.to_string(), dir.map(str::to_string));
        if let Some(v) = self.tags.get(&key) {
            return Ok(v.clone());
        }
        let u = base::parse(url).map_err(|p| Failed::Cannot(p.text))?;
        let repo = self.repo(url, false)?;
        let mut c = Git::command(&repo, &u);
        c.args(["ls-remote", "--tags", "--end-of-options", url, &base::tag_pattern(dir)]);
        let ran = run(c, None)?;
        if !ran.ok {
            let said = quoted(&ran.stderr);
            return Err(Failed::Cannot(tr!("git のリポジトリ `{url}` のタグを読めません。git の出力：\n{said}", "cannot read the tags of the git repository `{url}`; git said:\n{said}")));
        }
        let versions = base::versions_of(&String::from_utf8_lossy(&ran.stdout), dir);
        self.tags.insert(key, versions.clone());
        Ok(versions)
    }

    /// The commit the tag of `version` leads to now. Err (exit 2) when there is no such tag.
    pub fn tag(&mut self, name: &ModuleName, url: &str, dir: Option<&str>, version: &Version) -> Result<String, Failed> {
        match self.versions(url, dir)?.into_iter().find(|(v, _)| v == version) {
            Some((_, c)) => Ok(c),
            None => {
                let tag = base::tag_of(dir, version);
                Err(Failed::Cannot(tr!(
                    "git のリポジトリ `{url}` に、`{name}` {version} のタグ `{tag}` がありません",
                    "the git repository `{url}` has no tag `{tag}` of `{name}` {version}"
                )))
            }
        }
    }

    /// The version `version` of `name` made from `commit` of the repository `url` (its module at
    /// `dir`), checked as the steps of DESIGN 17.12.1 check it. Nothing is put in the store.
    pub fn make(&mut self, name: &ModuleName, version: &Version, url: &str, dir: Option<&str>, commit: &str) -> Result<Made, Failed> {
        let u = base::parse(url).map_err(|p| Failed::Cannot(p.text))?;
        let repo = self.repo(url, commit.len() == 64)?;
        let what = || format!("`{name}` {version}");
        // the commit and its trees; a server that refuses gets the commit asked for whole
        let mut whole = false;
        if let Err(said) = fetch(&repo, &u, url, &["--depth=1", "--filter=blob:none"], commit, None)? {
            whole = true;
            if let Err(said2) = fetch(&repo, &u, url, &["--depth=1", "--no-filter"], commit, None)? {
                return Err(self.not_fetched(name, version, url, dir, commit, if said2.is_empty() { said } else { said2 }));
            }
        }
        let listing = ls_tree(&repo, &u, commit, dir).map_err(|t| {
            let w = what();
            match dir {
                Some(d) => Failed::Refused(tr!(
                    "{w} のコミット {commit} に、ディレクトリ `{d}` がありません（{t}）",
                    "the commit {commit} of {w} has no directory `{d}` ({t})"
                )),
                None => Failed::Cannot(tr!("{w} のコミット {commit} の木を読めません（{t}）", "cannot read the tree of the commit {commit} of {w} ({t})")),
            }
        })?;
        // its `ritsu.mod`, first
        let Some(m_at) = listing.iter().position(|e| e.path == manifest::FILE && e.kind == "blob") else {
            let w = what();
            return Err(Failed::Refused(match dir {
                Some(d) => tr!("{w} のコミット {commit} の `{d}` に `ritsu.mod` がありません", "the commit {commit} of {w} has no `ritsu.mod` in `{d}`"),
                None => tr!("{w} のコミット {commit} のルートに `ritsu.mod` がありません", "the commit {commit} of {w} has no `ritsu.mod` at its root"),
            }));
        };
        if listing[m_at].mode == "120000" {
            let w = what();
            return Err(Failed::Refused(tr!(
                "{w} の `ritsu.mod` はシンボリックリンクです。モジュールが運ぶのはふつうのファイルだけです",
                "the `ritsu.mod` of {w} is a symbolic link; a module carries files only"
            )));
        }
        let mod_oid = listing[m_at].oid.clone();
        if !whole && self.missing(&repo, &u, std::slice::from_ref(&mod_oid))?.len() == 1 {
            whole = self.blobs(&repo, &u, url, std::slice::from_ref(&mod_oid), commit)?;
        }
        let mod_bytes = read_blobs(&repo, &u, std::slice::from_ref(&mod_oid))?.pop().unwrap_or_default();
        let m = manifest::parse(&mod_bytes, manifest::FILE).map_err(|ds| {
            let first = ds.first().map(|d| d.message.clone()).unwrap_or_default();
            let w = what();
            Failed::Refused(tr!("{w} の `ritsu.mod` を読めません。", "the `ritsu.mod` of {w} cannot be read: ", ).then(&first))
        })?;
        if let Err(t) = held_to(&m, name, version) {
            return Err(Failed::Mismatch(mod_bytes, t));
        }
        // the files it carries, by the rule of a directory's
        let entries: Vec<(&str, bool)> = listing.iter().map(|e| (e.path.as_str(), e.kind == "tree")).collect();
        let chosen = digest::carried_of_tree(&entries, &digest::excludes_of(&m)).map_err(Failed::Refused)?;
        for &i in &chosen {
            let e = &listing[i];
            let p = &e.path;
            let w = what();
            match e.mode.as_str() {
                "120000" => {
                    return Err(Failed::Refused(tr!(
                        "{w} の `{p}` はシンボリックリンクです。モジュールが運ぶのはふつうのファイルだけです",
                        "`{p}` of {w} is a symbolic link; a module carries files only"
                    )));
                }
                "160000" => {
                    return Err(Failed::Refused(tr!(
                        "{w} の `{p}` はサブモジュールです。モジュールが運ぶのはふつうのファイルだけなので、サブモジュールの中は取りません",
                        "`{p}` of {w} is a submodule; a module carries files only, and what is in a submodule is not fetched"
                    )));
                }
                "100644" | "100755" => {}
                other => {
                    return Err(Failed::Refused(tr!("{w} の `{p}` は、ふつうのファイルではありません（mode {other}）", "`{p}` of {w} is not a file (mode {other})")));
                }
            }
        }
        let paths: Vec<&str> = chosen.iter().map(|&i| listing[i].path.as_str()).collect();
        tar::check_set(&paths).map_err(|e| Failed::Refused(tr!("{} のファイル: ", "the files of {}: ", what(); what()).then(&e.text())))?;
        let oids: Vec<String> = chosen.iter().map(|&i| listing[i].oid.clone()).collect();
        if !whole {
            let lacking = self.missing(&repo, &u, &oids)?;
            if !lacking.is_empty() {
                self.blobs(&repo, &u, url, &lacking, commit)?;
            }
        }
        // their sizes, then their bytes as git holds them
        for (k, size) in sizes(&repo, &u, &oids)?.into_iter().enumerate() {
            if size > tar::MAX_FILE {
                let (p, max, w) = (paths[k], tar::MAX_FILE, what());
                return Err(Failed::Refused(tr!("{w} の `{p}` は {size} バイトあります。一つのファイルは {max} バイトまでです", "`{p}` of {w} is {size} bytes, and a file is {max} bytes at most")));
            }
        }
        let bytes = read_blobs(&repo, &u, &oids)?;
        let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(paths.len());
        for (p, b) in paths.iter().zip(bytes) {
            if base::is_lfs_pointer(&b) {
                let w = what();
                return Err(Failed::Refused(tr!(
                    "{w} の `{p}` は Git LFS のポインタのファイルです。ritsu は LFS のファイルを取りません。モジュールのファイルは、LFS に入れずにコミットしてください",
                    "`{p}` of {w} is a Git LFS pointer; ritsu does not fetch LFS files, so commit the files of a module without LFS"
                )));
            }
            files.push((p.to_string(), b));
        }
        let built = artifact::build(&m, &mod_bytes, &files).map_err(|t| Failed::Refused(tr!("{}: ", "{}: ", what(); what()).then(&t)))?;
        Ok(Made { built, files })
    }

    /// The blobs `oids` that are not in the repository `dir`.
    fn missing(&self, dir: &Path, u: &Url, oids: &[String]) -> Result<Vec<String>, Failed> {
        let input: String = oids.iter().map(|o| format!("{o}\n")).collect();
        let mut c = Git::command(dir, u);
        c.args(["cat-file", "--batch-check=%(objectname) %(objecttype)"]);
        let ran = run(c, Some(input.as_bytes()))?;
        if !ran.ok {
            let said = quoted(&ran.stderr);
            return Err(Failed::Cannot(tr!("git の cat-file が止まりました。git の出力：\n{said}", "git's cat-file stopped; git said:\n{said}")));
        }
        Ok(String::from_utf8_lossy(&ran.stdout).lines().filter_map(|l| l.strip_suffix(" missing").map(str::to_string)).collect())
    }

    /// The blobs `oids` fetched at once; a server that does not give a blob by its name (protocol
    /// version 0) gets the commit asked for whole. Whether the commit was fetched whole.
    fn blobs(&mut self, dir: &Path, u: &Url, url: &str, oids: &[String], commit: &str) -> Result<bool, Failed> {
        let input: String = oids.iter().map(|o| format!("{o}\n")).collect();
        if fetch(dir, u, url, &["--filter=blob:none", "--stdin"], "", Some(input.as_bytes()))?.is_ok() && self.missing(dir, u, oids)?.is_empty() {
            return Ok(false);
        }
        match fetch(dir, u, url, &["--depth=1", "--no-filter"], commit, None)? {
            Ok(()) => Ok(true),
            Err(said) => Err(Failed::Cannot(tr!("git のリポジトリ `{url}` からファイルを取れません。git の出力：\n{said}", "cannot fetch the files from the git repository `{url}`; git said:\n{said}"))),
        }
    }

    /// What a commit that cannot be fetched comes to: its tag moved (exit 1), or the repository
    /// cannot be asked or does not have it (exit 2).
    fn not_fetched(&mut self, name: &ModuleName, version: &Version, url: &str, dir: Option<&str>, commit: &str, said: String) -> Failed {
        if let Ok(now) = self.tag(name, url, dir, version)
            && now != commit
        {
            let tag = base::tag_of(dir, version);
            let (was, is) = (&commit[..12.min(commit.len())], &now[..12.min(now.len())]);
            return Failed::Refused(tr!(
                "`{name}` {version} のタグ `{tag}` が、lock のコミット {was} から {is} に動かされていて、git のリポジトリ `{url}` は lock のコミットを渡しません。何が変わったかを確かめてから、`ritsu mod get` で取り直してください",
                "the tag `{tag}` of `{name}` {version} was moved from the lock's commit {was} to {is}, and the git repository `{url}` does not give the lock's commit; see what changed, then run `ritsu mod get` to take it again"
            ));
        }
        Failed::Cannot(tr!(
            "git のリポジトリ `{url}` から `{name}` {version}（コミット {commit}）を取れません。git の出力：\n{said}",
            "cannot fetch `{name}` {version} (the commit {commit}) from the git repository `{url}`; git said:\n{said}"
        ))
    }
}

/// `module` and `version` of a version's `ritsu.mod`: those asked for; else what differs.
fn held_to(m: &Manifest, name: &ModuleName, version: &Version) -> Result<(), Text> {
    if &m.module.value != name {
        let other = &m.module.value;
        return Err(tr!(
            "`{name}` {version} として取った `ritsu.mod` は、`module {other}` です。git の依存の `module` は、URL と `dir` から決まる名前と同じにしてください",
            "the `ritsu.mod` fetched as `{name}` {version} says `module {other}`; the `module` of a dependency from git is the name its URL and `dir` give"
        ));
    }
    if m.version.as_ref().map(|v| &v.value) != Some(version) {
        let theirs = m.version.as_ref().map(|v| v.value.to_string()).unwrap_or_else(|| "-".to_string());
        return Err(tr!(
            "`{name}` のタグ {version} の `ritsu.mod` の `version` は {theirs} です。タグと `version` を同じ版にしてください",
            "the `ritsu.mod` of the tag {version} of `{name}` has the `version` {theirs}; tag the version its `ritsu.mod` says"
        ));
    }
    Ok(())
}

/// `git fetch` of `commit` (or of what the standard input names) from `url`, with the flags `how`.
/// Err of the outer Result when git cannot be run; Err of the inner one, with what git said, when it
/// ran and did not fetch.
fn fetch(dir: &Path, u: &Url, url: &str, how: &[&str], commit: &str, input: Option<&[u8]>) -> Result<Result<(), String>, Failed> {
    let mut c = Git::command(dir, u);
    c.args(["fetch", "--quiet", "--no-tags", "--no-write-fetch-head", "--recurse-submodules=no"]);
    c.args(how);
    c.args(["--end-of-options", url]);
    if !commit.is_empty() {
        c.arg(commit);
    }
    let ran = run(c, input)?;
    Ok(if ran.ok { Ok(()) } else { Err(quoted(&ran.stderr)) })
}

/// An entry of `ls-tree`.
struct Entry {
    mode: String,
    kind: String,
    oid: String,
    path: String,
}

/// The entries under the module's root at `commit` (`<commit>:<dir>`), with their paths from it.
fn ls_tree(dir: &Path, u: &Url, commit: &str, sub: Option<&str>) -> Result<Vec<Entry>, String> {
    let mut c = Git::command(dir, u);
    let tree = match sub {
        Some(d) => format!("{commit}:{d}"),
        None => commit.to_string(),
    };
    c.args(["ls-tree", "-r", "-t", "-z", "--end-of-options", &tree]);
    let ran = run(c, None).map_err(|f| f.text().en)?;
    if !ran.ok {
        return Err(quoted(&ran.stderr));
    }
    let mut out = Vec::new();
    for rec in ran.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let Some(tab) = rec.iter().position(|b| *b == b'\t') else { return Err("ls-tree".to_string()) };
        let head = String::from_utf8_lossy(&rec[..tab]).to_string();
        let path = String::from_utf8(rec[tab + 1..].to_vec()).map_err(|_| "a name that is not UTF-8".to_string())?;
        let mut w = head.split(' ');
        let (Some(mode), Some(kind), Some(oid)) = (w.next(), w.next(), w.next()) else { return Err("ls-tree".to_string()) };
        out.push(Entry { mode: mode.to_string(), kind: kind.to_string(), oid: oid.to_string(), path });
    }
    Ok(out)
}

/// The sizes of the blobs `oids`.
fn sizes(dir: &Path, u: &Url, oids: &[String]) -> Result<Vec<u64>, Failed> {
    let input: String = oids.iter().map(|o| format!("{o}\n")).collect();
    let mut c = Git::command(dir, u);
    c.args(["cat-file", "--batch-check=%(objectsize)"]);
    let ran = run(c, Some(input.as_bytes()))?;
    let out = String::from_utf8_lossy(&ran.stdout).to_string();
    let sizes: Vec<u64> = out.lines().filter_map(|l| l.trim().parse().ok()).collect();
    if !ran.ok || sizes.len() != oids.len() {
        let said = quoted(&ran.stderr);
        return Err(Failed::Cannot(tr!("git のファイルの大きさを読めません。git の出力：\n{said}", "cannot read the sizes of git's files; git said:\n{said}")));
    }
    Ok(sizes)
}

/// The bytes of the blobs `oids`, as git holds them (`cat-file --batch`: no filter, no change of the
/// ends of the lines).
fn read_blobs(dir: &Path, u: &Url, oids: &[String]) -> Result<Vec<Vec<u8>>, Failed> {
    let input: String = oids.iter().map(|o| format!("{o}\n")).collect();
    let mut c = Git::command(dir, u);
    c.args(["cat-file", "--batch"]);
    let ran = run(c, Some(input.as_bytes()))?;
    let unread = |said: &str| Failed::Cannot(tr!("git のファイルを読めません。git の出力：\n{said}", "cannot read git's files; git said:\n{said}"));
    if !ran.ok {
        return Err(unread(&quoted(&ran.stderr)));
    }
    let (b, mut at, mut out) = (&ran.stdout, 0usize, Vec::with_capacity(oids.len()));
    for _ in oids {
        let Some(nl) = b[at..].iter().position(|c| *c == b'\n') else { return Err(unread("")) };
        let head = String::from_utf8_lossy(&b[at..at + nl]).to_string();
        at += nl + 1;
        let size: usize = match head.rsplit(' ').next().and_then(|s| s.parse().ok()) {
            Some(s) if !head.ends_with(" missing") => s,
            _ => return Err(unread(&head)),
        };
        if at + size > b.len() {
            return Err(unread(&head));
        }
        out.push(b[at..at + size].to_vec());
        at += size + 1;
    }
    Ok(out)
}

/// git's version from `git version`'s line: `git version 2.50.1 (Apple Git-155)` gives (2, 50).
pub fn version_of(line: &str) -> Option<(u32, u32)> {
    let v = line.strip_prefix("git version ")?.split_whitespace().next()?;
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()?;
    Some((major, minor))
}

/// Running git with the variables that point it at a repository taken away.
trait ClearLocal {
    fn env_clear_local(&mut self) -> &mut Self;
}

impl ClearLocal for Command {
    fn env_clear_local(&mut self) -> &mut Command {
        for v in LOCAL_VARS {
            self.env_remove(v);
        }
        self
    }
}

/// `c` run with `input` on its standard input (closed when none), stopped after [`TIMEOUT`].
fn run(mut c: Command, input: Option<&[u8]>) -> Result<Ran, Failed> {
    c.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Failed::Cannot(tr!(
                "git がありません。git のリポジトリから取る依存があるので、git を入れて PATH に置いてください",
                "git is not there, and a dependency comes from a git repository; install git and put it on the PATH"
            )));
        }
        Err(e) => return Err(Failed::Cannot(ritsu_base::wasi::starting("git", &e))),
    };
    let writer = match (child.stdin.take(), input) {
        (Some(mut stdin), Some(b)) => {
            let b = b.to_vec();
            Some(std::thread::spawn(move || {
                let _ = stdin.write_all(&b);
            }))
        }
        _ => None,
    };
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_end(&mut b);
        }
        b
    });
    let err_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_end(&mut b);
        }
        b
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                let secs = TIMEOUT.as_secs();
                return Err(Failed::Cannot(tr!("git が {secs} 秒を超えたので止めました", "git ran for over {secs} s, and was stopped")));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(Failed::Cannot(ritsu_base::wasi::starting("git", &e))),
        }
    };
    if let Some(w) = writer {
        let _ = w.join();
    }
    let stdout = reader.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&err_reader.join().unwrap_or_default()).into_owned();
    Ok(Ran { ok: status.success(), stdout, stderr })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_of_git() {
        assert_eq!(version_of("git version 2.50.1 (Apple Git-155)"), Some((2, 50)));
        assert_eq!(version_of("git version 2.29.0.windows.1"), Some((2, 29)));
        assert_eq!(version_of("git version 2.28.0"), Some((2, 28)));
        assert!(version_of("git version 2.28.0").unwrap() < OLDEST);
        assert_eq!(version_of("hub version 1"), None);
    }
}
