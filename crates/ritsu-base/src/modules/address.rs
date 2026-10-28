//! How the diffs name the two things they compare (DESIGN 17.9.1): `rulec diff`, `koyomi diff`,
//! `sekisho diff` and `ritsu mod diff` read their two sides the same way.
//!
//! | Written | Names |
//! |---|---|
//! | `<path>` | the file in the working tree (a dependency's at the lock's version) |
//! | `<path>@<git revision>` | the file at that revision of the repository |
//! | `@<name>/<path>` | the file of the dependency `<name>`, at the lock's version |
//! | `@<name>@<version>/<path>` | the file of the dependency `<name>` at the module version `<version>`, from the store |
//!
//! A spelling that starts with `@` is a module's; a second `@` before the first `/` gives the
//! version. Any other is split at its first `@` when the whole is no file (rulec's reading, which
//! also takes a rule's name before the `@`). `@<name>/<path>@<revision>` is refused: a revision is
//! for the files of the repository at hand, and a dependency's version is written as a version.
//!
//! Each side is read through files of its own ([`open`]): a git revision ([`fs::GitRevision`]),
//! whose `ritsu.mod` and `ritsu.lock` are the revision's and whose dependencies from a registry are
//! read from the store by that lock; or, for `@<name>@<version>`, the module at hand's `ritsu.mod`
//! and lock laid over as they would be if it required that version ([`fs::Overlay`]), so that the
//! `@` paths in the dependency's file are read at the versions Minimal Version Selection chooses then.
//!
//! Nothing here asks a registry. A version that no store has stops a side with what to fetch;
//! [`try_open`] tells it apart ([`Unopened::Missing`]), so that `ritsu mod diff` fetches it and
//! opens the side again.

use super::graph::{self, Problem};
use super::lock::Lock;
use super::manifest::{self, Source};
use super::names::ModuleName;
use super::refer::raw_module_root;
use super::semver::Version;
use super::store::Store;
use crate::fs;
use crate::text::Text;
use crate::tr;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A side of a diff, as it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Address {
    /// A file of the working tree, or a dependency's at the lock's version through the view.
    Path(String),
    /// `<path>@<revision>`: the part before the first `@`, and the revision.
    Revision { path: String, rev: String },
    /// `@<name>/<path>`.
    Dependency { name: String, path: String },
    /// `@<name>@<version>/<path>`.
    Version { name: String, version: Version, path: String },
}

impl Address {
    /// The path a language reads and shows the side by: the path, the part before the revision, and
    /// `@<name>/<path>` for a dependency's file at either version.
    pub fn path(&self) -> String {
        match self {
            Address::Path(p) | Address::Revision { path: p, .. } => p.clone(),
            Address::Dependency { name, path } | Address::Version { name, path, .. } => format!("@{name}/{path}"),
        }
    }
}

/// A side read: `spelled` as [`Address`] reads it. Err, with what to say (exit 2), for a spelling
/// of a module's file that names none, and for a revision written after a dependency's file.
pub fn parse(spelled: &str) -> Result<Address, Text> {
    let Some(rest) = spelled.strip_prefix('@') else {
        if fs::exists(spelled) {
            return Ok(Address::Path(spelled.to_string()));
        }
        return Ok(match spelled.split_once('@') {
            Some((path, rev)) if !path.is_empty() && !rev.is_empty() => Address::Revision { path: path.to_string(), rev: rev.to_string() },
            _ => Address::Path(spelled.to_string()),
        });
    };
    let (head, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (name, version) = match head.split_once('@') {
        Some((n, v)) => (n, Some(v)),
        None => (head, None),
    };
    if name.is_empty() || path.is_empty() || version.is_some_and(str::is_empty) {
        return Err(tr!(
            "`{spelled}` は依存のファイルを指していません。`@<名前>/<パス>` か `@<名前>@<版>/<パス>` と書いてください",
            "`{spelled}` names no file of a dependency; write `@<name>/<path>` or `@<name>@<version>/<path>`"
        ));
    }
    if path.contains('@') && !fs::exists(spelled) {
        let file = path.split('@').next().unwrap_or(path);
        return Err(tr!(
            "`{spelled}`：依存のファイルには git の版を付けられません。git の版は手元のリポジトリのファイルのためのものです。依存の版は `@{name}@<版>/{file}` と書いてください",
            "`{spelled}`: a dependency's file takes no git revision, which is for the files of the repository at hand; write a version of the dependency as `@{name}@<version>/{file}`"
        ));
    }
    match version {
        None => Ok(Address::Dependency { name: name.to_string(), path: path.to_string() }),
        Some(v) => {
            let version = Version::parse(v).map_err(|_| {
                tr!(
                    "`{spelled}` の `{v}` は版ではありません。`@<名前>@1.2.0/<パス>` のように書いてください",
                    "the `{v}` of `{spelled}` is no version; write it as `@<name>@1.2.0/<path>`"
                )
            })?;
            Ok(Address::Version { name: name.to_string(), version, path: path.to_string() })
        }
    }
}

/// A side made ready to read: the path to read it by, and the files to read it and everything it
/// reads through (None for the files as they are).
pub struct Opened {
    pub path: String,
    pub files: Option<Rc<dyn fs::Files>>,
}

impl Opened {
    /// `f` run with the side's files.
    pub fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        match &self.files {
            Some(files) => fs::with(files.clone(), f),
            None => f(),
        }
    }
}

/// Why a side cannot be opened.
#[derive(Debug)]
pub enum Unopened {
    /// A version that no store has: once it is fetched, the side may open.
    Missing(Box<Missing>),
    /// Anything else.
    Said(Text),
}

/// A version of a module that is in neither the store nor the module's copy of it, and what to say
/// of it (`ritsu mod diff` fetches it; the diffs of the languages, which do not fetch, say it).
#[derive(Debug)]
pub struct Missing {
    pub module: ModuleName,
    pub version: Version,
    pub said: Text,
}

impl Unopened {
    fn missing(module: ModuleName, version: Version, said: Text) -> Unopened {
        Unopened::Missing(Box::new(Missing { module, version, said }))
    }
}

impl From<Text> for Unopened {
    fn from(t: Text) -> Unopened {
        Unopened::Said(t)
    }
}

impl From<Unopened> for Text {
    fn from(u: Unopened) -> Text {
        match u {
            Unopened::Missing(m) => m.said,
            Unopened::Said(t) => t,
        }
    }
}

/// The side `a` made ready to read. Err, with what to say (exit 2): git cannot start, the revision
/// is not one, `@<name>@<version>` outside a module, of a name the module does not require, or of a
/// version that is in neither the store nor the module's copy of it.
pub fn open(a: &Address) -> Result<Opened, Text> {
    try_open(a).map_err(Text::from)
}

/// [`open`], with a version that is in no store told apart from what else stops it.
pub fn try_open(a: &Address) -> Result<Opened, Unopened> {
    match a {
        Address::Path(p) => Ok(Opened { path: p.clone(), files: None }),
        Address::Dependency { .. } => Ok(Opened { path: a.path(), files: None }),
        Address::Revision { path, rev } => {
            let here = fs::current_dir().unwrap_or_default();
            let g = fs::GitRevision::open(&here, rev)?;
            Ok(Opened { path: path.clone(), files: Some(Rc::new(g)) })
        }
        Address::Version { name, version, .. } => {
            let here = fs::current_dir().unwrap_or_default();
            let Some(root) = raw_module_root(&here) else {
                return Err(tr!(
                    "`@{name}@{version}/…` は、`ritsu.mod` のあるモジュールの中でだけ書けます",
                    "`@{name}@{version}/…` is written only in a module, a directory with a `ritsu.mod`"
                )
                .into());
            };
            let overlay = at_version(&root, name, version)?;
            Ok(Opened { path: a.path(), files: Some(Rc::new(overlay)) })
        }
    }
}

/// The stores a version is looked for in: the module's copy of the store, then the store.
fn stores(root: Option<&Path>) -> Vec<Store> {
    let mut out: Vec<Store> = root.map(Store::vendored).into_iter().collect();
    out.extend(Store::located());
    out
}

/// The entry of a module's version: the store it is in and the digest of its OCI manifest, from the
/// store's index of versions (the module at `root`'s copy of the store first). None when no store
/// has it with its files.
pub fn find_version(root: Option<&Path>, module: &ModuleName, version: &Version) -> Option<(Store, String)> {
    let located = Store::located();
    for s in stores(root) {
        let d = s.find(module, version).or_else(|| located.as_ref().and_then(|l| l.find(module, version)));
        if let Some(d) = d
            && s.has_files(&d)
        {
            return Some((s, d));
        }
    }
    None
}

/// What to run for a version the store does not have: `ritsu mod fetch <name>@<version>`.
pub fn fetch_version(name: &str, version: &Version) -> Text {
    tr!(
        "`{name}` {version} が、置き場にも同梱のコピーにもありません。`ritsu mod fetch {name}@{version}` を走らせてください",
        "`{name}` {version} is neither in the store nor in the module's copy of it; run `ritsu mod fetch {name}@{version}`"
    )
}

/// The module the dependency `name` of the module at `root` is: its name in a registry, or the name
/// in the `ritsu.mod` its path leads to.
fn module_of_dependency(root: &Path, m: &manifest::Manifest, name: &str) -> Result<ModuleName, Text> {
    let Some(r) = m.require(name) else {
        let module = m.module.value.to_string();
        return Err(tr!(
            "モジュール `{module}` の `ritsu.mod` に `require {name} …` がありません",
            "the `ritsu.mod` of the module `{module}` has no `require {name} …`"
        ));
    };
    match &r.source {
        Source::Oci { repository, .. } => ModuleName::parse(repository).map_err(|e| e.text()),
        Source::Path { path } => {
            let file = graph::fold(&root.join(path)).join(manifest::FILE);
            let bytes = fs::raw::read(&file).map_err(|_| {
                tr!("依存 `{name}` の `ritsu.mod` を読めません", "cannot read the `ritsu.mod` of the dependency `{name}`")
            })?;
            manifest::parse(&bytes, manifest::FILE).map(|m| m.module.value).map_err(|ds| ds.first().map(|d| d.message.clone()).unwrap_or_default())
        }
    }
}

/// The module at `root` as it would be if it required its dependency `name` at `version`: its
/// `ritsu.mod` with that `require` from the registry at that version, and the lock Minimal Version
/// Selection chooses then, that version held (DESIGN 17.9.1), laid over the files of this thread.
/// Err, [`Unopened::Missing`] for the first version the choice reaches that no store has.
pub fn at_version(root: &Path, name: &str, version: &Version) -> Result<fs::Overlay, Unopened> {
    let file = root.join(manifest::FILE);
    let bytes = fs::raw::read(&file).map_err(|_| tr!("`ritsu.mod` を読めません", "cannot read `ritsu.mod`"))?;
    let m = manifest::parse(&bytes, manifest::FILE).map_err(|ds| ds.first().map(|d| d.message.clone()).unwrap_or_default())?;
    let module = module_of_dependency(root, &m, name)?;
    if find_version(Some(root), &module, version).is_none() {
        return Err(Unopened::missing(module, version.clone(), fetch_version(name, version)));
    }
    let without = m.removed(name).map_err(|e| e.text())?;
    let without = manifest::parse(without.as_bytes(), manifest::FILE).map_err(|ds| ds.first().map(|d| d.message.clone()).unwrap_or_default())?;
    let text = without.added(name, &Source::Oci { repository: module.to_string(), min: version.clone() }).map_err(|e| e.text())?;
    let mut only_mod = fs::Overlay::new(fs::current());
    only_mod.put(&file, text.clone());
    let mut missing: Option<(ModuleName, Version)> = None;
    let chosen = fs::with(Rc::new(only_mod), || {
        graph::choose(root, |n, v| {
            let Some((s, d)) = find_version(Some(root), n, v) else {
                missing = Some((n.clone(), v.clone()));
                return Err(Problem::Stop(vec![fetch_version(&n.to_string(), v)]));
            };
            s.module_bytes(&d).ok_or_else(|| Problem::Stop(vec![fetch_version(&n.to_string(), v)]))
        })
    });
    let mut chosen = chosen.map_err(|p| match missing.take() {
        Some((n, v)) => Unopened::missing(n, v, p.text()),
        None => Unopened::Said(p.text()),
    })?;
    chosen.versions.insert(module, version.clone());
    let lock: Lock = graph::lock_of(&chosen, |n, v| {
        let (s, d) = find_version(Some(root), n, v)?;
        let files = s.record(&d)?.files;
        Some((d, files))
    })
    .map_err(|t| t.then(&tr!("。置き場の記録を確かめてください", "; check the records of the store")))?;
    let mut overlay = fs::Overlay::new(fs::current());
    overlay.put(&file, text);
    overlay.put(&root.join(super::lock::FILE), lock.text());
    Ok(overlay)
}

/// What stops the tree of the module the file `path` is in, read through the files of this thread
/// (a side's): a dependency to fetch, a newer ritsu wanted. None when the tree reads (or has only
/// diagnostics, which a language says with its own), and outside a module.
pub fn stopped(path: &str) -> Option<Text> {
    let abs = crate::paths::absolute(Path::new(path));
    let root = raw_module_root(abs.parent()?)?;
    match super::refer::tree(&root).as_ref() {
        Err(Problem::Stop(ts)) => ts.first().cloned(),
        _ => None,
    }
}

/// The directory a module's version is read from as a module at hand (`ritsu mod diff <module>@<version>`):
/// its entry's files, which carry its own `ritsu.mod` and `ritsu.lock`. Err, saying what to fetch,
/// when no store has it.
pub fn version_dir(root: Option<&Path>, module: &ModuleName, version: &Version) -> Result<PathBuf, Text> {
    match find_version(root, module, version) {
        Some((s, d)) => Ok(s.files(&d)),
        None => Err(fetch_version(&module.to_string(), version)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spellings() {
        let p = |s: &str| parse(s).map_err(|t| t.en);
        assert_eq!(p("@pricing/rules/fee.rule"), Ok(Address::Dependency { name: "pricing".into(), path: "rules/fee.rule".into() }));
        assert_eq!(
            p("@pricing@1.0.0/rules/fee.rule"),
            Ok(Address::Version { name: "pricing".into(), version: Version::parse("1.0.0").unwrap(), path: "rules/fee.rule".into() })
        );
        assert_eq!(p("no/such/file.rule@HEAD~1"), Ok(Address::Revision { path: "no/such/file.rule".into(), rev: "HEAD~1".into() }));
        assert_eq!(p("no/such/file.rule"), Ok(Address::Path("no/such/file.rule".into())));
        assert!(p("@pricing/rules/fee.rule@HEAD").unwrap_err().contains("takes no git revision"));
        assert!(p("@pricing@x/rules/fee.rule").unwrap_err().contains("no version"));
        assert!(p("@pricing").unwrap_err().contains("names no file"));
        assert!(p("@pricing@/a").unwrap_err().contains("names no file"));
        assert_eq!(Address::Version { name: "pricing".into(), version: Version::parse("1.0.0").unwrap(), path: "a.rule".into() }.path(), "@pricing/a.rule");
    }
}
