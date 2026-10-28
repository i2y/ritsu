//! The tree of modules (DESIGN 17.7): the module at hand, the modules its `require`s reach by a
//! path, and those they reach in a registry, each once, with the version its lock pins.
//!
//! [`load`] reads a tree as a check reads it: it chooses no version. A module from a path is read
//! where the path leads, one from a registry from the module's copy of the store or the store
//! itself, its files checked against the lock's digest (E305); the lock is held to `ritsu.mod`
//! (E304, W302). A module a path reaches wins over the same module in a registry, wherever in the
//! tree. [`choose`] chooses the versions, as `ritsu mod lock` does: Minimal Version Selection
//! ([`mvs`]) over every version reached, chosen or not.
//!
//! Each module has one way to write it ([`Node::prefix`]): the chain of `@<name>` with the fewest
//! steps from the module at hand, and of chains as short, the one a walk breadth first, in the
//! order of each `ritsu.mod`'s `require`s, reaches first.

use super::digest;
use super::lock::{self, Lock, Locked, Pinned};
use super::manifest::{self, Manifest, Source};
use super::names::{self, ModuleName};
use super::semver::Version;
use super::store::Store;
use super::vendor::{self, Where};
use super::{Diag, E302, E303, E304, E305, E306, E307, E308, W302};
use crate::fs;
use crate::text::Text;
use crate::tr;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};

/// One module of the tree.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: ModuleName,
    pub manifest: Manifest,
    /// Where its files are read: its directory, or the store's entry's `files/`.
    pub dir: PathBuf,
    pub at: Where,
    /// For a module a path reaches, the path from the root of the module at hand (`.` for that
    /// module itself).
    pub path: Option<String>,
    /// For a module from a registry, its line of the lock: the version, and the two digests.
    pub pinned: Option<Locked>,
    /// How its files are written from the root of the module at hand: empty for that module,
    /// `@pricing`, `@pricing/@tax`.
    pub prefix: String,
    /// Each `require`, by its name, and the node it reaches, in the order of the `require`s.
    pub deps: Vec<(String, usize)>,
    /// The signature lines its record says it was checked against.
    pub verified: Vec<String>,
}

impl Node {
    /// Its version: the lock's for a module from a registry, its `ritsu.mod`'s for another.
    pub fn version(&self) -> Option<&Version> {
        match &self.pinned {
            Some(l) => l.version(),
            None => self.manifest.version.as_ref().map(|v| &v.value),
        }
    }

    /// The digests of its OCI manifest and of its files, for one from a registry.
    pub fn digests(&self) -> Option<(&str, &str)> {
        match self.pinned.as_ref().map(|l| &l.pinned) {
            Some(Pinned::Oci { digest, files, .. }) => Some((digest, files)),
            _ => None,
        }
    }
}

/// A tree read.
#[derive(Clone, Debug)]
pub struct Tree {
    /// The root of the module at hand.
    pub root: PathBuf,
    /// The modules in the order of their prefixes' walk: the module at hand first.
    pub nodes: Vec<Node>,
    pub lock: Option<Lock>,
    /// What was found that does not stop a run: E307, W302.
    pub found: Vec<Diag>,
    /// The modules both a path and a registry were asked for, which the path gives.
    pub path_won: Vec<ModuleName>,
}

impl Tree {
    pub fn main(&self) -> &Node {
        &self.nodes[0]
    }

    /// The node `from` reaches by its `require` named `name`.
    pub fn dep(&self, from: usize, name: &str) -> Option<usize> {
        self.nodes.get(from)?.deps.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }

    /// The node a chain of names reaches from the module at hand (`["pricing", "tax"]`).
    pub fn walk(&self, chain: &[&str]) -> Option<usize> {
        chain.iter().try_fold(0, |at, n| self.dep(at, n))
    }

    /// The node of a module.
    pub fn by_name(&self, name: &ModuleName) -> Option<usize> {
        self.nodes.iter().position(|n| &n.name == name)
    }

    /// The node whose prefix is `prefix` (`@pricing/@tax`; empty for the module at hand).
    pub fn by_prefix(&self, prefix: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.prefix == prefix)
    }
}

/// Why a tree cannot be read.
#[derive(Clone, Debug)]
pub enum Problem {
    /// E301–E306, E308 (and whatever E307 there is beside them): diagnostics, exit 1.
    Diags(Vec<Diag>),
    /// What stops a run before anything is checked, exit 2: a module that is not in the store, a
    /// signature with no record of its check, a newer ritsu wanted, a lock of a newer format. Each
    /// says what to run.
    Stop(Vec<Text>),
}

impl Problem {
    /// The first thing it says.
    pub fn text(&self) -> Text {
        match self {
            Problem::Diags(ds) => ds.first().map(|d| d.message.clone()).unwrap_or_default(),
            Problem::Stop(ts) => ts.first().cloned().unwrap_or_default(),
        }
    }
}

/// A file a tree was read from that a person may change while ritsu runs, and its bytes then
/// (none when it was not there): the module at hand's `ritsu.mod` and `ritsu.lock`, and the
/// `ritsu.mod` of each module a path reaches. A module from a registry is pinned by its digest.
pub type Inputs = Vec<(PathBuf, Option<Vec<u8>>)>;

/// The tree whose module at hand has its root at `root`, its modules from a registry read from the
/// store [`super::store::location`] says.
pub fn load(root: &Path) -> Result<Tree, Problem> {
    load_with(root, Store::located().as_ref()).0
}

/// The same, from the store given, with the files it was read from.
pub fn load_with(root: &Path, store: Option<&Store>) -> (Result<Tree, Problem>, Inputs) {
    let mut l = Loader::new(root, store);
    let r = l.load();
    (r, l.inputs)
}

/// `a` and `b` joined and their `.` and `..` folded by their letters, a `..` that leads above `a`
/// kept: the path from the root of the module at hand to a module a path reaches (`../tax`).
pub fn up_join(a: &str, b: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for c in a.split('/').chain(b.split('/')) {
        match c {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|l| *l != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            _ => parts.push(c),
        }
    }
    if parts.is_empty() { ".".to_string() } else { parts.join("/") }
}

/// A path with its `.` and `..` folded by their letters.
pub(crate) fn fold(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The path from `root` to `dir`, both folded, by their parts: `.` for `root` itself, `../tax`. A
/// path that leaves the root and comes back into it (`../shop/rules` from `shop`) is written the
/// short way, so that one directory has one path.
fn rel_from(root: &Path, dir: &Path) -> String {
    let r: Vec<Component> = root.components().collect();
    let d: Vec<Component> = dir.components().collect();
    let same = r.iter().zip(&d).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); r.len() - same];
    parts.extend(d[same..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    if parts.is_empty() { ".".to_string() } else { parts.join("/") }
}

/// `<chain>/@<name>`.
fn chain(at: &str, name: &str) -> String {
    if at.is_empty() { format!("@{name}") } else { format!("{at}/@{name}") }
}

/// A file of a module, as a diagnostic writes it.
fn shown_file(prefix: &str, file: &str) -> String {
    if prefix.is_empty() { file.to_string() } else { format!("{prefix}/{file}") }
}

/// The version of ritsu that runs.
pub fn running() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

fn newer(name: &str, wants: &Version) -> Text {
    let now = running();
    tr!(
        "`{name}` は ritsu {wants} 以上を求めています。走らせている ritsu は {now} です。ritsu を新しくしてください",
        "`{name}` asks for ritsu {wants} or later, and the ritsu that runs is {now}; run a newer ritsu"
    )
}

struct Loader<'a> {
    root: PathBuf,
    store: Option<&'a Store>,
    inputs: Inputs,
    diags: Vec<Diag>,
    found: Vec<Diag>,
    stops: Vec<Text>,
    nodes: Vec<Node>,
    /// How each node was first reached, as diagnostics write its files.
    shown: Vec<String>,
    /// The `require` that first reached each node: the node it is in, its line and its column.
    reached_by: Vec<Option<(usize, usize, usize)>>,
    by_name: BTreeMap<ModuleName, usize>,
    lock: Option<Lock>,
    lock_text: String,
    /// The lock is there and cannot be read (E303 said so).
    lock_bad: bool,
    path_won: BTreeSet<ModuleName>,
}

impl<'a> Loader<'a> {
    fn new(root: &Path, store: Option<&'a Store>) -> Loader<'a> {
        Loader {
            root: fold(&crate::paths::absolute(root)),
            store,
            inputs: Vec::new(),
            diags: Vec::new(),
            found: Vec::new(),
            stops: Vec::new(),
            nodes: Vec::new(),
            shown: Vec::new(),
            reached_by: Vec::new(),
            by_name: BTreeMap::new(),
            lock: None,
            lock_text: String::new(),
            lock_bad: false,
            path_won: BTreeSet::new(),
        }
    }

    fn input(&mut self, p: &Path) -> Option<Vec<u8>> {
        let b = fs::raw::read(p).ok();
        self.inputs.push((p.to_path_buf(), b.clone()));
        b
    }

    /// Where a `require … path "<path>"` of node `i` leads: the directory, and the path to it from
    /// the root of the module at hand.
    fn path_target(&self, i: usize, path: &str) -> (PathBuf, String) {
        let dir = fold(&self.root.join(up_join(self.nodes[i].path.as_deref().unwrap_or("."), path)));
        let rel = rel_from(&self.root, &dir);
        (dir, rel)
    }

    /// A diagnostic at a line of the `ritsu.mod` of node `i`.
    fn at_mod(&self, code: &'static str, i: usize, line: usize, col: usize, message: Text) -> Diag {
        let f = shown_file(&self.shown[i], manifest::FILE);
        Diag::at(code, &f, line, col, message).rel(&f).source(&self.nodes[i].manifest.text())
    }

    /// A diagnostic at a line of the lock.
    fn at_lock(&self, code: &'static str, line: usize, message: Text) -> Diag {
        Diag::at(code, lock::FILE, line, 0, message).rel(lock::FILE).source(&self.lock_text)
    }

    /// The `ritsu.mod` in `dir`, read; none when it cannot be (its diagnostics, or a stop for a
    /// newer ritsu, said why).
    fn manifest(&mut self, dir: &Path, shown: &str, record: bool) -> Option<Manifest> {
        let file = dir.join(manifest::FILE);
        let bytes = if record { self.input(&file) } else { fs::raw::read(&file).ok() }?;
        match manifest::parse(&bytes, &shown_file(shown, manifest::FILE)) {
            Ok(m) => Some(m),
            Err(ds) => {
                match manifest::wants(&bytes).filter(|v| *v > running()) {
                    Some(v) => self.stops.push(newer(&shown_file(shown, manifest::FILE), &v)),
                    None => self.diags.extend(ds),
                }
                None
            }
        }
    }

    /// E307: a name that starts with `@` right under the root of a module on the disk.
    fn e307(&mut self, dir: &Path, shown: &str) {
        let Ok(entries) = fs::raw::read_dir(dir) else { return };
        let mut names: Vec<String> = entries.into_iter().filter_map(|(n, _)| n.to_str().map(str::to_string)).filter(|n| n.starts_with('@')).collect();
        names.sort();
        let f = shown_file(shown, manifest::FILE);
        for n in names {
            self.found.push(Diag::at(E307, &f, 0, 0, super::refer::at_name(&n)).rel(&f));
        }
    }

    fn add(&mut self, node: Node, shown: String, reached_by: Option<(usize, usize, usize)>) -> usize {
        let i = self.nodes.len();
        self.by_name.insert(node.name.clone(), i);
        self.nodes.push(node);
        self.shown.push(shown);
        self.reached_by.push(reached_by);
        i
    }

    /// The module at hand and every module its paths reach.
    fn locals(&mut self) -> bool {
        let root = self.root.clone();
        let Some(main) = self.manifest(&root, "", true) else { return false };
        self.e307(&root, "");
        let node = Node {
            name: main.module.value.clone(),
            manifest: main,
            dir: root.clone(),
            at: Where::Here,
            path: Some(".".to_string()),
            pinned: None,
            prefix: String::new(),
            deps: Vec::new(),
            verified: Vec::new(),
        };
        self.add(node, String::new(), None);
        let mut queue = VecDeque::from([0usize]);
        while let Some(i) = queue.pop_front() {
            let reqs = self.nodes[i].manifest.requires.clone();
            for r in reqs {
                let Source::Path { path } = &r.source else { continue };
                let (dir, rel) = self.path_target(i, path);
                let file = dir.join(manifest::FILE);
                if !fs::raw::metadata(&file).is_ok_and(|m| m.is_file()) {
                    self.inputs.push((file, None));
                    let d = self.at_mod(E302, i, r.line, r.col, tr!(
                        "`require {} path \"{path}\"` の指す `{rel}` に `ritsu.mod` がありません。パスの依存は、`ritsu.mod` のあるディレクトリを指します",
                        "`{rel}`, where `require {} path \"{path}\"` leads, has no `ritsu.mod`; a dependency by a path names a directory with a `ritsu.mod` in it",
                        r.name; r.name
                    ));
                    self.diags.push(d);
                    continue;
                }
                let shown = chain(&self.shown[i], &r.name);
                let Some(m) = self.manifest(&dir, &shown, true) else { continue };
                let name = m.module.value.clone();
                match self.by_name.get(&name).copied() {
                    Some(j) => {
                        // the same directory (the module at hand, when the path comes back to it) is
                        // the same module
                        let other = self.nodes[j].path.clone().unwrap_or_default();
                        if self.nodes[j].dir != dir {
                            let d = self.at_mod(E306, i, r.line, r.col, tr!(
                                "モジュール `{name}` が二か所から来ます。`{other}` と `{rel}` です。一つのモジュールは一か所から読みます",
                                "the module `{name}` comes from two places, `{other}` and `{rel}`; a module is read from one"
                            ));
                            self.diags.push(d);
                        }
                    }
                    None => {
                        self.e307(&dir, &shown);
                        let node = Node {
                            name,
                            manifest: m,
                            dir,
                            at: Where::Path,
                            path: Some(rel),
                            pinned: None,
                            prefix: String::new(),
                            deps: Vec::new(),
                            verified: Vec::new(),
                        };
                        let j = self.add(node, shown, Some((i, r.line, r.col)));
                        queue.push_back(j);
                    }
                }
            }
        }
        true
    }

    /// A module from a registry, at the version and with the digests of its lock's line.
    fn fetched(&mut self, name: &ModuleName, l: &Locked, shown: &str) -> Option<Node> {
        let Pinned::Oci { version, digest, files } = &l.pinned else { return None };
        let Some((store, at)) = vendor::find(&self.root, digest, self.store) else {
            self.stops.push(tr!(
                "依存 `{name}` {version} が、置き場にも同梱のコピーにもありません。`ritsu mod fetch` を走らせてください",
                "the dependency `{name}` {version} is neither in the store nor in this module's copy of it; run `ritsu mod fetch`"
            ));
            return None;
        };
        let fdir = store.files(digest);
        if digest::files(&fdir).ok().as_deref() != Some(files.as_str()) {
            let message = match at {
                Where::Vendor => tr!(
                    "同梱のコピーにある `{name}` {version} のファイルが、`ritsu.lock` のダイジェストと違います。`ritsu mod vendor` で置き直してください",
                    "the files of `{name}` {version} in this module's copy of the store differ from the digest of `ritsu.lock`; run `ritsu mod vendor` to copy them again"
                ),
                _ => tr!(
                    "置き場にある `{name}` {version} のファイルが、`ritsu.lock` のダイジェストと違います。`ritsu mod clean` で消してから `ritsu mod fetch` で取り直してください",
                    "the files of `{name}` {version} in the store differ from the digest of `ritsu.lock`; run `ritsu mod clean`, then `ritsu mod fetch` to fetch them again"
                ),
            };
            let d = self.at_lock(E305, l.line, message);
            self.diags.push(d);
            return None;
        }
        let Some(m) = self.manifest(&fdir, shown, false) else {
            if !fs::raw::metadata(&fdir.join(manifest::FILE)).is_ok_and(|m| m.is_file()) {
                let d = self.at_lock(E306, l.line, tr!(
                    "取ってきた `{name}` {version} のファイルに `ritsu.mod` がありません",
                    "the files of `{name}` {version} that were fetched have no `ritsu.mod`"
                ));
                self.diags.push(d);
            }
            return None;
        };
        let f = shown_file(shown, manifest::FILE);
        let mut ok = true;
        if &m.module.value != name {
            let other = &m.module.value;
            self.diags.push(Diag::at(E306, &f, m.module.line, m.module.col, tr!(
                "`{name}` として取ってきた `ritsu.mod` は、`module {other}` です。同じ名前のモジュールが二か所から来ています",
                "the `ritsu.mod` fetched as `{name}` says `module {other}`: two places give a module of one name"
            )).rel(&f).source(&m.text()));
            ok = false;
        }
        let theirs = m.version.as_ref().map(|v| v.value.to_string()).unwrap_or_else(|| "-".to_string());
        if m.version.as_ref().map(|v| &v.value) != Some(version) {
            let d = self.at_lock(E304, l.line, tr!(
                "`ritsu.lock` は `{name}` を {version} に固定していますが、取ってきた `ritsu.mod` の `version` は {theirs} です。`ritsu mod lock` を走らせてください",
                "`ritsu.lock` pins `{name}` at {version}, and the `version` of the `ritsu.mod` fetched is {theirs}; run `ritsu mod lock`"
            ));
            self.diags.push(d);
            ok = false;
        }
        for r in &m.requires {
            if let Source::Path { path } = &r.source {
                self.diags.push(Diag::at(E308, &f, r.line, r.col, tr!(
                    "レジストリから取ったモジュール `{name}` {version} が、`require {} path \"{path}\"` を持っています。パスの依存は使う側の手元に無いので読めません",
                    "the module `{name}` {version}, from a registry, has `require {} path \"{path}\"`; a dependency by a path is not where the module is used, so it cannot be read",
                    r.name; r.name
                )).rel(&f).source(&m.text()));
                ok = false;
            }
        }
        if !ok {
            return None;
        }
        let verified = store.record(digest).map(|r| r.verified.into_iter().map(|(p, _)| p).collect()).unwrap_or_default();
        Some(Node { name: name.clone(), manifest: m, dir: fdir, at, path: None, pinned: Some(l.clone()), prefix: String::new(), deps: Vec::new(), verified })
    }

    /// Every module from a registry the tree reaches, each held to the lock.
    fn registry(&mut self) {
        let mut queue: VecDeque<usize> = (0..self.nodes.len()).collect();
        let mut said_no_lock = false;
        while let Some(i) = queue.pop_front() {
            let reqs = self.nodes[i].manifest.requires.clone();
            for r in reqs {
                let Source::Oci { repository, min } = &r.source else { continue };
                let Ok(name) = ModuleName::parse(repository) else { continue };
                if let Some(j) = self.by_name.get(&name).copied()
                    && self.nodes[j].pinned.is_none()
                {
                    // a path reaches it, or it is the module at hand: the path wins
                    if j != 0 {
                        self.path_won.insert(name);
                    }
                    continue;
                }
                if self.lock_bad {
                    continue;
                }
                let Some(lock) = &self.lock else {
                    if !said_no_lock {
                        said_no_lock = true;
                        let d = self.at_mod(E303, i, r.line, r.col, tr!(
                            "`ritsu.lock` がありません。依存 `{}` はレジストリから取るので、選んだバージョンとダイジェストを `ritsu.lock` に固定します。`ritsu mod lock` を走らせてください",
                            "there is no `ritsu.lock`: the dependency `{}` comes from a registry, so the version chosen and its digests are pinned in `ritsu.lock`; run `ritsu mod lock`",
                            r.name; r.name
                        ));
                        self.diags.push(d);
                    }
                    continue;
                };
                let line = format!("require {} {}", r.name, r.source.words());
                let Some(l) = lock.get(&name).cloned() else {
                    let d = self.at_mod(E304, i, r.line, r.col, tr!(
                        "`ritsu.lock` が `ritsu.mod` に合いません。`{line}` のモジュールが lock にありません。`ritsu mod lock` を走らせてください",
                        "`ritsu.lock` does not match `ritsu.mod`: the lock has no line for `{line}`; run `ritsu mod lock`"
                    ));
                    self.diags.push(d);
                    continue;
                };
                let version = match &l.pinned {
                    Pinned::Oci { version, .. } => version.clone(),
                    Pinned::Path { path } => {
                        let d = self.at_mod(E304, i, r.line, r.col, tr!(
                            "`ritsu.lock` が `ritsu.mod` に合いません。`{line}` はレジストリから取りますが、lock は `{name}` をパス `{path}` から読みます。`ritsu mod lock` を走らせてください",
                            "`ritsu.lock` does not match `ritsu.mod`: `{line}` comes from a registry, and the lock reads `{name}` from the path `{path}`; run `ritsu mod lock`"
                        ));
                        self.diags.push(d);
                        continue;
                    }
                };
                if version < *min {
                    let d = self.at_mod(E304, i, r.line, r.col, tr!(
                        "`ritsu.lock` が `ritsu.mod` に合いません。`{line}` は {min} 以上を求めていますが、lock は {version} です。`ritsu mod lock` を走らせてください",
                        "`ritsu.lock` does not match `ritsu.mod`: `{line}` asks for {min} or later, and the lock has {version}; run `ritsu mod lock`"
                    ));
                    self.diags.push(d);
                    continue;
                }
                if self.by_name.contains_key(&name) {
                    continue;
                }
                let shown = chain(&self.shown[i], &r.name);
                if let Some(node) = self.fetched(&name, &l, &shown) {
                    let j = self.add(node, shown, Some((i, r.line, r.col)));
                    queue.push_back(j);
                } else {
                    // said why; the name is taken, so that it is said once
                    self.by_name.entry(name).or_insert(usize::MAX);
                }
            }
        }
    }

    /// Every line of the lock is a module the tree reaches, from where the tree reads it (E304,
    /// W302); every module a path reaches has its line, when there is a lock (E304).
    fn lines(&mut self) {
        let Some(lock) = self.lock.clone() else { return };
        for l in &lock.modules {
            let at = self.by_name.get(&l.name).copied().filter(|j| *j != usize::MAX);
            let name = l.name.to_string();
            match (at, &l.pinned) {
                (None, _) if !self.by_name.contains_key(&l.name) => {
                    let d = self.at_lock(W302, l.line, tr!(
                        "`ritsu.lock` に、もう求められていないモジュール `{name}` の行があります。`ritsu mod lock` で書き直してください",
                        "`ritsu.lock` has a line for `{name}`, which nothing requires now; run `ritsu mod lock` to write it again"
                    ));
                    self.found.push(d);
                }
                (Some(0), _) => {
                    let d = self.at_lock(E304, l.line, tr!(
                        "`ritsu.lock` に、手元のモジュール `{name}` 自身の行があります。`ritsu mod lock` を走らせてください",
                        "`ritsu.lock` has a line for `{name}`, the module at hand itself; run `ritsu mod lock`"
                    ));
                    self.diags.push(d);
                }
                (Some(j), Pinned::Path { path }) => {
                    let rel = self.nodes[j].path.clone();
                    if rel.is_none() || fold(&self.root.join(path)) != self.nodes[j].dir {
                        let rel = rel.unwrap_or_default();
                        let d = self.at_lock(E304, l.line, tr!(
                            "`ritsu.lock` が `ritsu.mod` に合いません。lock は `{name}` を `{path}` から読みますが、`ritsu.mod` の `require` がたどる先は `{rel}` です。`ritsu mod lock` を走らせてください",
                            "`ritsu.lock` does not match `ritsu.mod`: the lock reads `{name}` from `{path}`, and the `require`s of `ritsu.mod` lead to `{rel}`; run `ritsu mod lock`"
                        ));
                        self.diags.push(d);
                    }
                }
                (Some(j), Pinned::Oci { .. }) => {
                    if self.nodes[j].pinned.is_none() {
                        let rel = self.nodes[j].path.clone().unwrap_or_default();
                        let d = self.at_lock(E304, l.line, tr!(
                            "`ritsu.lock` が `ritsu.mod` に合いません。lock は `{name}` をレジストリから取りますが、この木ではパス `{rel}` から読みます。`ritsu mod lock` を走らせてください",
                            "`ritsu.lock` does not match `ritsu.mod`: the lock takes `{name}` from a registry, and this tree reads it from the path `{rel}`; run `ritsu mod lock`"
                        ));
                        self.diags.push(d);
                    }
                }
                _ => {}
            }
        }
        for j in 1..self.nodes.len() {
            if self.nodes[j].pinned.is_some() || lock.get(&self.nodes[j].name).is_some() {
                continue;
            }
            let (name, rel) = (self.nodes[j].name.to_string(), self.nodes[j].path.clone().unwrap_or_default());
            if let Some((i, line, col)) = self.reached_by[j] {
                let d = self.at_mod(E304, i, line, col, tr!(
                    "`ritsu.lock` が `ritsu.mod` に合いません。パスの依存 `{name}`（`{rel}`）の行が lock にありません。`ritsu mod lock` を走らせてください",
                    "`ritsu.lock` does not match `ritsu.mod`: the lock has no line for `{name}`, a dependency by the path `{rel}`; run `ritsu mod lock`"
                ));
                self.diags.push(d);
            }
        }
    }

    /// Every module's `ritsu >=`, and the record of the check of every signature that applies to a
    /// module from a registry: what stops a run.
    fn stops(&mut self) {
        let now = running();
        for i in 0..self.nodes.len() {
            if let Some(w) = &self.nodes[i].manifest.ritsu
                && w.value > now
            {
                let t = newer(&self.nodes[i].name.to_string(), &w.value);
                self.stops.push(t);
            }
        }
        for j in 0..self.nodes.len() {
            if self.nodes[j].pinned.is_none() {
                continue;
            }
            let name = self.nodes[j].name.to_string();
            // the lines of the module at hand, and of each module that requires this one
            let mut lines: Vec<String> = Vec::new();
            for i in 0..self.nodes.len() {
                let requires = i == 0 || self.nodes[i].deps.iter().any(|(_, k)| *k == j);
                if !requires {
                    continue;
                }
                for s in &self.nodes[i].manifest.signatures {
                    if names::covers(&s.prefix, &name) && !lines.contains(&s.text()) {
                        lines.push(s.text());
                    }
                }
            }
            for line in lines {
                if !self.nodes[j].verified.contains(&line) {
                    let v = self.nodes[j].version().map(|v| v.to_string()).unwrap_or_default();
                    self.stops.push(tr!(
                        "`{name}` {v} の署名を `{line}` で確かめた記録がありません。`ritsu mod fetch` を走らせてください",
                        "there is no record that the signature of `{name}` {v} was checked against `{line}`; run `ritsu mod fetch`"
                    ));
                }
            }
        }
    }

    /// Each `require` of each node, to the node it reaches.
    fn edges(&mut self) {
        for i in 0..self.nodes.len() {
            let mut deps = Vec::new();
            for r in &self.nodes[i].manifest.requires {
                let target = match &r.source {
                    Source::Path { path } => {
                        let (dir, _) = self.path_target(i, path);
                        self.nodes.iter().position(|n| n.path.is_some() && n.dir == dir)
                    }
                    Source::Oci { repository, .. } => ModuleName::parse(repository).ok().and_then(|n| self.by_name.get(&n).copied()).filter(|j| *j != usize::MAX),
                };
                if let Some(j) = target {
                    deps.push((r.name.clone(), j));
                }
            }
            self.nodes[i].deps = deps;
        }
    }

    /// The nodes in the order of the walk of their prefixes, each with its prefix.
    fn canonical(&mut self) {
        let n = self.nodes.len();
        let mut order: Vec<usize> = vec![0];
        let mut prefix: Vec<Option<String>> = vec![None; n];
        prefix[0] = Some(String::new());
        let mut at = 0;
        while at < order.len() {
            let i = order[at];
            at += 1;
            let deps = self.nodes[i].deps.clone();
            for (name, j) in deps {
                if prefix[j].is_none() {
                    prefix[j] = Some(chain(prefix[i].as_deref().unwrap_or(""), &name));
                    order.push(j);
                }
            }
        }
        // a node no walk reaches (none, when every node was reached by a `require`) keeps its place
        for (i, p) in prefix.iter().enumerate() {
            if p.is_none() {
                order.push(i);
            }
        }
        let mut new_at = vec![0usize; n];
        for (k, i) in order.iter().enumerate() {
            new_at[*i] = k;
        }
        let mut old: Vec<Option<Node>> = std::mem::take(&mut self.nodes).into_iter().map(Some).collect();
        for i in &order {
            let mut node = old[*i].take().unwrap_or_else(|| unreachable!());
            node.prefix = prefix[*i].clone().unwrap_or_else(|| self.shown[*i].clone());
            for d in &mut node.deps {
                d.1 = new_at[d.1];
            }
            self.nodes.push(node);
        }
    }

    fn load(&mut self) -> Result<Tree, Problem> {
        if !self.locals() {
            return Err(self.problem());
        }
        // the lock, when there is one
        let lock_file = self.root.join(lock::FILE);
        if let Some(bytes) = self.input(&lock_file) {
            self.lock_text = String::from_utf8_lossy(&bytes).into_owned();
            match lock::parse(&bytes, lock::FILE) {
                Ok(l) => self.lock = Some(l),
                Err(lock::Error::Newer(n)) => {
                    self.stops.push(lock::Error::Newer(n).text());
                    self.lock_bad = true;
                }
                Err(lock::Error::Unreadable(ds)) => {
                    self.diags.extend(ds);
                    self.lock_bad = true;
                }
            }
        }
        self.registry();
        self.lines();
        self.edges();
        self.stops();
        if !self.diags.is_empty() || !self.stops.is_empty() {
            return Err(self.problem());
        }
        self.canonical();
        Ok(Tree {
            root: self.root.clone(),
            nodes: std::mem::take(&mut self.nodes),
            lock: self.lock.take(),
            found: std::mem::take(&mut self.found),
            path_won: std::mem::take(&mut self.path_won).into_iter().collect(),
        })
    }

    fn problem(&mut self) -> Problem {
        if self.diags.is_empty() && !self.stops.is_empty() {
            return Problem::Stop(std::mem::take(&mut self.stops));
        }
        let mut ds = std::mem::take(&mut self.diags);
        ds.extend(self.found.iter().filter(|d| d.is_error()).cloned());
        Problem::Diags(ds)
    }
}

/// Minimal Version Selection: from the `require`s `roots`, every version reached through what
/// `requires` says each version requires (each read once), and for each module the highest version
/// any of them asked for. A version not chosen still counts what it requires. The answer depends on
/// what the versions require alone, not on the order they are read in.
pub fn mvs<K: Ord + Clone>(roots: &[(K, Version)], mut requires: impl FnMut(&K, &Version) -> Vec<(K, Version)>) -> BTreeMap<K, Version> {
    match mvs_try::<K, std::convert::Infallible>(roots, |k, v| Ok(requires(k, v))) {
        Ok(m) => m,
        Err(e) => match e {},
    }
}

/// [`mvs`], with a reading that can fail: the first failure is the answer.
pub fn mvs_try<K: Ord + Clone, E>(roots: &[(K, Version)], mut requires: impl FnMut(&K, &Version) -> Result<Vec<(K, Version)>, E>) -> Result<BTreeMap<K, Version>, E> {
    let mut chosen: BTreeMap<K, Version> = BTreeMap::new();
    let mut seen: BTreeSet<(K, Version)> = BTreeSet::new();
    let mut queue: VecDeque<(K, Version)> = VecDeque::new();
    for (k, v) in roots {
        if seen.insert((k.clone(), v.clone())) {
            queue.push_back((k.clone(), v.clone()));
        }
    }
    while let Some((k, v)) = queue.pop_front() {
        match chosen.get(&k) {
            Some(c) if *c >= v => {}
            _ => {
                chosen.insert(k.clone(), v.clone());
            }
        }
        for (k2, v2) in requires(&k, &v)? {
            if seen.insert((k2.clone(), v2.clone())) {
                queue.push_back((k2, v2));
            }
        }
    }
    Ok(chosen)
}

/// What `ritsu mod lock` chooses (DESIGN 17.7): the modules a path reaches, each with its path from
/// the root of the module at hand, and a version of each module from a registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub paths: Vec<(ModuleName, String)>,
    pub versions: BTreeMap<ModuleName, Version>,
}

/// The versions the module at hand's tree chooses: its `ritsu.mod` and those the paths reach, read
/// from the disk, and `ritsu_mod` the `ritsu.mod` of each version of a module from a registry that is
/// reached (from the store, or a registry), each asked for once.
pub fn choose(root: &Path, mut ritsu_mod: impl FnMut(&ModuleName, &Version) -> Result<Vec<u8>, Problem>) -> Result<Chosen, Problem> {
    let mut l = Loader::new(root, None);
    if !l.locals() || !l.diags.is_empty() || !l.stops.is_empty() {
        return Err(l.problem());
    }
    let local: BTreeSet<ModuleName> = l.nodes.iter().map(|n| n.name.clone()).collect();
    let mut roots: Vec<(ModuleName, Version)> = Vec::new();
    for n in &l.nodes {
        for r in &n.manifest.requires {
            if let Source::Oci { repository, min } = &r.source
                && let Ok(name) = ModuleName::parse(repository)
                && !local.contains(&name)
            {
                roots.push((name, min.clone()));
            }
        }
    }
    let versions = mvs_try(&roots, |name, version| {
        let bytes = ritsu_mod(name, version)?;
        let shown = format!("{name}@{version}/{}", manifest::FILE);
        let m = manifest::parse(&bytes, &shown).map_err(Problem::Diags)?;
        let mut ds = Vec::new();
        if &m.module.value != name {
            let other = &m.module.value;
            ds.push(Diag::at(E306, &shown, m.module.line, m.module.col, tr!(
                "`{name}` {version} の `ritsu.mod` は、`module {other}` です。同じ名前のモジュールが二か所から来ています",
                "the `ritsu.mod` of `{name}` {version} says `module {other}`: two places give a module of one name"
            )).rel(&shown).source(&m.text()));
        }
        if m.version.as_ref().map(|v| &v.value) != Some(version) {
            let theirs = m.version.as_ref().map(|v| v.value.to_string()).unwrap_or_else(|| "-".to_string());
            ds.push(Diag::at(E306, &shown, m.version.as_ref().map_or(0, |v| v.line), 0, tr!(
                "`{name}` のタグ {version} の `ritsu.mod` の `version` は {theirs} です",
                "the `ritsu.mod` of the tag {version} of `{name}` has the `version` {theirs}"
            )).rel(&shown).source(&m.text()));
        }
        for r in &m.requires {
            if let Source::Path { path } = &r.source {
                ds.push(Diag::at(E308, &shown, r.line, r.col, tr!(
                    "レジストリから取ったモジュール `{name}` {version} が、`require {} path \"{path}\"` を持っています。パスの依存は使う側の手元に無いので読めません",
                    "the module `{name}` {version}, from a registry, has `require {} path \"{path}\"`; a dependency by a path is not where the module is used, so it cannot be read",
                    r.name; r.name
                )).rel(&shown).source(&m.text()));
            }
        }
        if !ds.is_empty() {
            return Err(Problem::Diags(ds));
        }
        Ok(m.requires
            .iter()
            .filter_map(|r| match &r.source {
                Source::Oci { repository, min } => ModuleName::parse(repository).ok().filter(|n| !local.contains(n)).map(|n| (n, min.clone())),
                Source::Path { .. } => None,
            })
            .collect())
    })?;
    let paths = l.nodes.iter().skip(1).map(|n| (n.name.clone(), n.path.clone().unwrap_or_default())).collect();
    Ok(Chosen { paths, versions })
}

/// The lock of what was chosen: a `path` line for each module a path reaches, and an `oci` line
/// with the digests `digests` gives for each version chosen.
pub fn lock_of(chosen: &Chosen, mut digests: impl FnMut(&ModuleName, &Version) -> Option<(String, String)>) -> Result<Lock, Text> {
    let mut modules = Vec::new();
    for (name, path) in &chosen.paths {
        modules.push(Locked { name: name.clone(), pinned: Pinned::Path { path: path.clone() }, line: 0 });
    }
    for (name, version) in &chosen.versions {
        let Some((digest, files)) = digests(name, version) else {
            return Err(tr!("`{name}` {version} のダイジェストが分かりません", "the digests of `{name}` {version} are not known"));
        };
        modules.push(Locked { name: name.clone(), pinned: Pinned::Oci { version: version.clone(), digest, files }, line: 0 });
    }
    Ok(Lock::new(modules))
}
