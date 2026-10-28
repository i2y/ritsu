//! `ritsu mod lock [--check]` (DESIGN 17.4, 17.7): the versions chosen by Minimal Version Selection,
//! pinned in `ritsu.lock` with the digests the store (or the module's copy of it) has for them, and
//! a line for each module a path reaches. A tree with no module from a registry needs no lock: none
//! is written, and one that is there is removed (★). `ritsu mod add` and `ritsu mod remove` write
//! the lock again through [`plan`] when the tree changes.
//!
//! The `ritsu.mod` of each version reached is read from the store's index of versions or from the
//! lock's line that pins the same version, in the module's copy of the store or in the store; a
//! version in neither is what [`super::fetch::for_choosing`] answers. The line of the ritsu that
//! wrote the lock is a record (DESIGN 17.4): a lock that would differ from the one there in that line
//! alone is the same, and is not written again.

use super::{Ctx, fetch};
use ritsu_base::cli::Args;
use ritsu_base::json::Json;
use ritsu_base::modules::graph::{self, Problem};
use ritsu_base::modules::lock::{self as lockfile, Lock, Locked, Pinned};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::Store;
use ritsu_base::text::{Text, plural};
use ritsu_base::tr;
use std::path::Path;

/// What a run does with `ritsu.lock`, or would do under `--check`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    /// The lock there is the one the tree wants.
    Same,
    /// The lock is written (again).
    Written,
    /// The tree needs no lock, and the one there is removed.
    Removed,
    /// The tree needs no lock, and there is none.
    None,
}

impl Done {
    /// The word the JSON writes.
    pub fn word(self) -> &'static str {
        match self {
            Done::Same => "same",
            Done::Written => "written",
            Done::Removed => "removed",
            Done::None => "none",
        }
    }
}

/// The lock the tree wants, and what writing it does to the one there.
pub struct Plan {
    /// None for a tree with no module from a registry, which needs no lock.
    pub wanted: Option<Lock>,
    /// The lock there, read, when it can be.
    pub old: Option<Lock>,
    pub done: Done,
}

/// The lock the tree whose root is `root` (canonical) wants, with the store `store`.
pub fn plan(root: &Path, store: Option<&Store>) -> Result<Plan, Problem> {
    let there = std::fs::read(root.join(lockfile::FILE)).ok();
    let old = match there.as_deref().map(|b| lockfile::parse(b, lockfile::FILE)) {
        Some(Ok(l)) => Some(l),
        Some(Err(lockfile::Error::Newer(n))) => return Err(Problem::Stop(vec![lockfile::Error::Newer(n).text()])),
        // a lock that cannot be read is written again
        Some(Err(lockfile::Error::Unreadable(_))) | None => None,
    };
    let chosen = graph::choose(root, |name, version| ritsu_mod(root, store, old.as_ref(), name, version))?;
    if chosen.versions.is_empty() {
        let done = if there.is_some() { Done::Removed } else { Done::None };
        return Ok(Plan { wanted: None, old, done });
    }
    let mut wanted = graph::lock_of(&chosen, |name, version| digests(root, store, old.as_ref(), name, version)).map_err(|t| Problem::Stop(vec![t]))?;
    if let Some(o) = &old {
        let mut as_there = wanted.clone();
        as_there.ritsu = o.ritsu.clone();
        if there.as_deref() == Some(as_there.text().as_bytes()) {
            wanted.ritsu = o.ritsu.clone();
            return Ok(Plan { wanted: Some(wanted), old, done: Done::Same });
        }
    }
    Ok(Plan { wanted: Some(wanted), old, done: Done::Written })
}

impl Plan {
    /// What it plans, done: the lock written, or removed.
    pub fn write(&self, root: &Path) -> Result<(), Text> {
        let file = root.join(lockfile::FILE);
        let r = match (self.done, &self.wanted) {
            (Done::Written, Some(l)) => std::fs::write(&file, l.text()),
            (Done::Removed, _) => std::fs::remove_file(&file),
            _ => Ok(()),
        };
        r.map_err(|e| tr!("ritsu.lock に書けません（{e}）", "cannot write ritsu.lock ({e})"))
    }

    /// How many modules the lock wanted pins.
    fn count(&self) -> usize {
        self.wanted.as_ref().map_or(0, |l| l.modules.len())
    }

    /// The line a command says of the lock, in both languages; `check` when nothing was written.
    pub fn said(&self, check: bool) -> Text {
        let n = self.count();
        let modules = plural(n, "module", "modules");
        match (self.done, check) {
            (Done::Same, _) => tr!("ritsu.lock は書き直すところがありません（モジュール {n} 個）", "ritsu.lock is up to date: {modules}"),
            (Done::Written, false) => tr!("ritsu.lock を書きました（モジュール {n} 個）", "wrote ritsu.lock: {modules}"),
            (Done::Written, true) if self.old.is_none() => tr!(
                "ritsu.lock がありませんが、この木には要ります。`ritsu mod lock` を走らせてください",
                "there is no ritsu.lock, and the tree needs one; run `ritsu mod lock`"
            ),
            (Done::Written, true) => tr!("ritsu.lock は書き直しになります。`ritsu mod lock` を走らせてください", "ritsu.lock would be written again; run `ritsu mod lock`"),
            (Done::Removed, false) => tr!(
                "ritsu.lock を消しました。レジストリから取る依存が無いので、lock は要りません",
                "removed ritsu.lock: no dependency comes from a registry, so the tree needs no lock"
            ),
            (Done::Removed, true) => tr!(
                "ritsu.lock は消すことになります。レジストリから取る依存がありません。`ritsu mod lock` を走らせてください",
                "ritsu.lock would be removed: no dependency comes from a registry; run `ritsu mod lock`"
            ),
            (Done::None, _) => tr!("lock は要りません。レジストリから取る依存がありません", "no lock is needed: no dependency comes from a registry"),
        }
    }

    /// The lines of the lock that change, `+` for a line it gains and `-` for one it loses.
    fn changes(&self) -> Vec<String> {
        let lines = |l: Option<&Lock>| -> Vec<String> { l.map(|l| l.modules.iter().map(Locked::text).collect()).unwrap_or_default() };
        let (old, new) = (lines(self.old.as_ref()), lines(self.wanted.as_ref()));
        let mut out: Vec<String> = old.iter().filter(|l| !new.contains(l)).map(|l| format!("- {l}")).collect();
        out.extend(new.iter().filter(|l| !old.contains(l)).map(|l| format!("+ {l}")));
        out
    }
}

/// The stores a version is looked up in: the module's copy first, then the store.
fn stores(root: &Path, store: Option<&Store>) -> Vec<Store> {
    let mut out = vec![Store::vendored(root)];
    out.extend(store.cloned());
    out
}

/// The digest of the OCI manifest of a version: the store's index of versions, where a fetch put the
/// tag, when the entry is there; else the lock's line, when it pins the same version.
fn digest_of(root: &Path, store: Option<&Store>, old: Option<&Lock>, name: &ModuleName, version: &Version) -> Option<String> {
    let all = stores(root, store);
    if let Some(d) = store.and_then(|s| s.find(name, version)).filter(|d| all.iter().any(|s| s.has(d))) {
        return Some(d);
    }
    match old.and_then(|l| l.get(name)).map(|l| &l.pinned) {
        Some(Pinned::Oci { version: v, digest, .. }) if v == version => Some(digest.clone()),
        _ => None,
    }
}

/// The `ritsu.mod` of a version, to choose the versions by.
fn ritsu_mod(root: &Path, store: Option<&Store>, old: Option<&Lock>, name: &ModuleName, version: &Version) -> Result<Vec<u8>, Problem> {
    if let Some(d) = digest_of(root, store, old, name, version)
        && let Some(b) = stores(root, store).iter().find_map(|s| s.module_bytes(&d))
    {
        return Ok(b);
    }
    let d = fetch::for_choosing(store, name, version).map_err(|t| Problem::Stop(vec![t]))?;
    store.and_then(|s| s.module_bytes(&d)).ok_or_else(|| {
        Problem::Stop(vec![tr!(
            "依存 `{name}` {version} の `ritsu.mod` が、置き場にありません",
            "the `ritsu.mod` of the dependency `{name}` {version} is not in the store"
        )])
    })
}

/// The two digests of a version for its line of the lock: the OCI manifest's, and the files' from
/// the record of its entry (or from the lock's line that pins it).
fn digests(root: &Path, store: Option<&Store>, old: Option<&Lock>, name: &ModuleName, version: &Version) -> Option<(String, String)> {
    let d = digest_of(root, store, old, name, version)?;
    if let Some(r) = stores(root, store).iter().find_map(|s| s.record(&d)) {
        return Some((d, r.files));
    }
    match old.and_then(|l| l.get(name)).map(|l| &l.pinned) {
        Some(Pinned::Oci { digest, files, .. }) if *digest == d => Some((d, files.clone())),
        _ => None,
    }
}

/// A line of the lock, as the JSON writes it.
pub fn locked_json(l: &Locked) -> Json {
    let (source, version, path, digest, files) = match &l.pinned {
        Pinned::Oci { version, digest, files } => ("oci", Some(version.to_string()), None, Some(digest.as_str()), Some(files.as_str())),
        Pinned::Path { path } => ("path", None, Some(path.as_str()), None, None),
    };
    Json::obj([
        ("name", Json::str(l.name.to_string())),
        ("source", Json::str(source)),
        ("version", Json::opt_str(version)),
        ("path", Json::opt_str(path)),
        ("digest", Json::opt_str(digest)),
        ("files_digest", Json::opt_str(files)),
    ])
}

/// `ritsu mod lock [--check] [--format text|json]`: the exit code.
pub fn lock(ctx: &Ctx, a: &Args) -> u8 {
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let store = Store::located();
    let plan = match plan(&root, store.as_ref()) {
        Ok(p) => p,
        Err(p) => return super::stopped(ctx, &root, &p),
    };
    let check = a.has("--check");
    if !check && let Err(t) = plan.write(&root) {
        return ctx.refuse(t);
    }
    if ctx.json() {
        let modules = plan.wanted.as_ref().map(|l| {
            let mut ms: Vec<&Locked> = l.modules.iter().collect();
            ms.sort_by(|a, b| a.name.text().as_bytes().cmp(b.name.text().as_bytes()));
            ms.into_iter().map(locked_json).collect::<Vec<_>>()
        });
        let o = Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("root", Json::str(ctx.shown(&root))),
            ("check", Json::Bool(check)),
            ("lock", Json::str(plan.done.word())),
            ("modules", Json::arr(modules.unwrap_or_default())),
        ]);
        println!("{}", o.pretty());
    } else {
        if check {
            for l in plan.changes() {
                println!("{l}");
            }
        }
        println!("{}", ctx.say(&plan.said(check)));
    }
    u8::from(check && matches!(plan.done, Done::Written | Done::Removed))
}
