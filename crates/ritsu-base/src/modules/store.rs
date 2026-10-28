//! The store (DESIGN 17.15): where the modules fetched are kept across runs, and the copy of it in a
//! module's `.ritsu/modules/` (`ritsu mod vendor`), which has the same shape without `refs` and
//! `tmp`.
//!
//! ```text
//! <store>/
//!   sha256/<hex of the OCI manifest's digest>/
//!     manifest.json   the OCI manifest's bytes
//!     ritsu.mod       the first layer
//!     files/          the module's files (read only; none for a version fetched for its ritsu.mod alone)
//!     record.json     {"module", "version", "files", "fetched", "verified": [{"policy", "on"}]}
//!   refs/<registry>/<repository>/<version>   one line: sha256:<hex>
//!   tmp/
//! ```
//!
//! What reads it is here, and [`place`], which puts an entry made in `tmp/` in its place by a
//! rename: the commands of `ritsu mod` that fetch write through it, and [`put_directory`] makes an
//! entry from a module's directory without a registry (what the tests of the stages after this one
//! stand on). Reading goes through [`crate::fs::raw`], so a copy held in memory (the page in the
//! browser) reads as one on the disk does.

use super::artifact;
use super::digest;
use super::manifest::{self, Manifest};
use super::names::ModuleName;
use super::semver::Version;
use super::tar;
use crate::fs;
use crate::json::{self, Json};
use crate::text::Text;
use crate::tr;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// The environment variable that says where the store is.
pub const VARIABLE: &str = "RITSU_MODULES";

/// Where a module's copy of the store is, from its root.
pub const VENDOR: &str = ".ritsu/modules";

thread_local! {
    static PLACE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Run `f` with the store at `dir` on this thread, whatever the environment says (a test's own
/// store, beside the others that run at the same time).
pub fn with_location<R>(dir: &Path, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            PLACE.with(|c| *c.borrow_mut() = before);
        }
    }
    let _restore = Restore(PLACE.with(|c| c.borrow_mut().replace(dir.to_path_buf())));
    f()
}

/// Where the store is: what [`with_location`] gave on this thread; else `RITSU_MODULES`; else, on
/// macOS, `~/Library/Caches/ritsu/modules`, and on the other systems `$XDG_CACHE_HOME/ritsu/modules`
/// (an absolute one) or `~/.cache/ritsu/modules`. Built for WASI, where the system is not known, the
/// macOS place is taken when `~/Library/Caches` is there. None when nothing says (no `HOME`), and in
/// a page in the browser, which has no store.
pub fn location() -> Option<PathBuf> {
    if let Some(p) = PLACE.with(|c| c.borrow().clone()) {
        return Some(p);
    }
    if cfg!(all(target_arch = "wasm32", target_os = "unknown")) {
        return None;
    }
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(p) = var(VARIABLE) {
        return Some(p);
    }
    let home = var("HOME")?;
    let mac = if cfg!(target_os = "macos") {
        true
    } else if cfg!(target_os = "wasi") {
        fs::raw::metadata(&home.join("Library/Caches")).is_ok_and(|m| m.is_dir())
    } else {
        false
    };
    if mac {
        return Some(home.join("Library/Caches/ritsu/modules"));
    }
    match var("XDG_CACHE_HOME").filter(|p| crate::paths::rooted(p)) {
        Some(x) => Some(x.join("ritsu/modules")),
        None => Some(home.join(".cache/ritsu/modules")),
    }
}

/// A store, or a module's copy of one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    pub root: PathBuf,
}

/// What `record.json` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub module: String,
    pub version: String,
    /// The digest of the files.
    pub files: String,
    /// The day it was fetched (`YYYY-MM-DD`).
    pub fetched: String,
    /// The signature lines it was checked against, each with the day it was.
    pub verified: Vec<(String, String)>,
}

impl Record {
    pub fn to_json(&self) -> Json {
        Json::obj([
            ("module", Json::str(&self.module)),
            ("version", Json::str(&self.version)),
            ("files", Json::str(&self.files)),
            ("fetched", Json::str(&self.fetched)),
            ("verified", Json::arr(self.verified.iter().map(|(p, on)| Json::obj([("policy", Json::str(p)), ("on", Json::str(on))])))),
        ])
    }

    pub fn from_json(j: &Json) -> Option<Record> {
        let s = |k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
        let verified = j
            .get("verified")?
            .as_arr()?
            .iter()
            .map(|v| Some((v.get("policy")?.as_str()?.to_string(), v.get("on")?.as_str()?.to_string())))
            .collect::<Option<Vec<_>>>()?;
        Some(Record { module: s("module")?, version: s("version")?, files: s("files")?, fetched: s("fetched")?, verified })
    }
}

impl Store {
    pub fn at(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }

    /// The store [`location`] says, if any.
    pub fn located() -> Option<Store> {
        location().map(Store::at)
    }

    /// The copy of the store in the module whose root is `module_root`.
    pub fn vendored(module_root: &Path) -> Store {
        Store::at(module_root.join(VENDOR))
    }

    /// The directory of the entry of a digest of an OCI manifest.
    pub fn entry(&self, digest: &str) -> PathBuf {
        self.root.join("sha256").join(digest::hex(digest).unwrap_or("-"))
    }

    /// The directory of the entry's files.
    pub fn files(&self, digest: &str) -> PathBuf {
        self.entry(digest).join("files")
    }

    /// Whether the entry has its files (not only its `ritsu.mod`).
    pub fn has_files(&self, digest: &str) -> bool {
        fs::raw::metadata(&self.files(digest)).is_ok_and(|m| m.is_dir())
    }

    /// Whether the entry is there, with its files or without.
    pub fn has(&self, digest: &str) -> bool {
        fs::raw::metadata(&self.entry(digest).join(manifest::FILE)).is_ok_and(|m| m.is_file())
    }

    /// The entry's `ritsu.mod` (the first layer), which an entry without files has too.
    pub fn module_bytes(&self, digest: &str) -> Option<Vec<u8>> {
        fs::raw::read(&self.entry(digest).join(manifest::FILE)).ok()
    }

    /// The entry's `record.json`, read.
    pub fn record(&self, digest: &str) -> Option<Record> {
        let bytes = fs::raw::read(&self.entry(digest).join("record.json")).ok()?;
        Record::from_json(&json::parse(std::str::from_utf8(&bytes).ok()?).ok()?)
    }

    /// The file of the version index of a module's version.
    pub fn ref_file(&self, name: &ModuleName, version: &Version) -> Option<PathBuf> {
        match name {
            ModuleName::Published { registry, repository } => Some(self.root.join("refs").join(registry).join(repository).join(version.to_string())),
            ModuleName::Local(_) => None,
        }
    }

    /// The digest the version index has for a module's version.
    pub fn find(&self, name: &ModuleName, version: &Version) -> Option<String> {
        let bytes = fs::raw::read(&self.ref_file(name, version)?).ok()?;
        let s = String::from_utf8(bytes).ok()?;
        let d = s.strip_suffix('\n').unwrap_or(&s);
        digest::is_digest(d).then(|| d.to_string())
    }
}

/// An entry to place: the OCI manifest's bytes, `ritsu.mod`, the files (none for a version fetched
/// for its `ritsu.mod` alone), and the record.
#[derive(Clone, Debug)]
pub struct Parts<'a> {
    pub manifest: &'a [u8],
    pub module: &'a [u8],
    pub files: Option<&'a [(String, Vec<u8>)]>,
    pub record: Record,
}

/// An entry put in `store` under the digest of its OCI manifest, and its version in the index
/// (`refs`) when `name` and `version` are given: made under `tmp/`, then renamed into its place, so
/// that two runs at once leave no entry half made (the later one throws its own away). Its files are
/// read only (0444) and its directories 0755.
pub fn place(store: &Store, parts: &Parts<'_>, index: Option<(&ModuleName, &Version)>) -> Result<String, Text> {
    let digest = digest::of(parts.manifest);
    let disk = |p: &Path, e: std::io::Error| {
        let d = p.display();
        tr!("{d} に書けません（{e}）", "cannot write {d} ({e})")
    };
    let target = store.entry(&digest);
    let done = store.has(&digest) && (parts.files.is_none() || store.has_files(&digest));
    if !done {
        let tmp_root = store.root.join("tmp");
        std::fs::create_dir_all(&tmp_root).map_err(|e| disk(&tmp_root, e))?;
        let tmp = unique(&tmp_root);
        std::fs::create_dir(&tmp).map_err(|e| disk(&tmp, e))?;
        let made = (|| -> Result<(), Text> {
            std::fs::write(tmp.join("manifest.json"), parts.manifest).map_err(|e| disk(&tmp, e))?;
            std::fs::write(tmp.join(manifest::FILE), parts.module).map_err(|e| disk(&tmp, e))?;
            if let Some(files) = parts.files {
                let fdir = tmp.join("files");
                std::fs::create_dir(&fdir).map_err(|e| disk(&fdir, e))?;
                tar::unpack(files, &fdir).map_err(|e| e.text())?;
                read_only(&fdir).map_err(|e| disk(&fdir, e))?;
            }
            std::fs::write(tmp.join("record.json"), parts.record.to_json().pretty() + "\n").map_err(|e| disk(&tmp, e))?;
            Ok(())
        })();
        if let Err(e) = made {
            let _ = remove(&tmp);
            return Err(e);
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| disk(parent, e))?;
        }
        // an entry without files gives way to one with them
        if store.has(&digest) && !store.has_files(&digest) {
            let _ = remove(&target);
        }
        if std::fs::rename(&tmp, &target).is_err() {
            // another run placed it first
            let _ = remove(&tmp);
            if !store.has(&digest) {
                let t = target.display();
                return Err(tr!("{t} に置けません", "cannot place {t}"));
            }
        }
    }
    if let Some((name, version)) = index
        && let Some(f) = store.ref_file(name, version)
    {
        if let Some(parent) = f.parent() {
            std::fs::create_dir_all(parent).map_err(|e| disk(parent, e))?;
        }
        std::fs::write(&f, format!("{digest}\n")).map_err(|e| disk(&f, e))?;
    }
    Ok(digest)
}

/// A new name under `dir` no other run takes at the same time.
fn unique(dir: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    dir.join(format!("{}-{nanos}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}

/// The files under `dir` made read only (0444), its directories 0755.
fn read_only(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let t = e.file_type()?;
            if t.is_dir() {
                read_only(&e.path())?;
                std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o755))?;
            } else if t.is_file() {
                std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o444))?;
            }
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// A directory removed, read-only files in it too.
pub fn remove(dir: &Path) -> std::io::Result<()> {
    std::fs::remove_dir_all(dir)
}

/// What [`put_directory`] placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub name: ModuleName,
    pub version: Version,
    pub digest: String,
    pub files_digest: String,
}

impl Placed {
    /// The line of a lock that pins it.
    pub fn locked(&self) -> super::lock::Locked {
        super::lock::Locked {
            name: self.name.clone(),
            pinned: super::lock::Pinned::Oci { version: self.version.clone(), digest: self.digest.clone(), files: self.files_digest.clone() },
            line: 0,
        }
    }
}

/// The module whose root is `dir` made into its OCI artifact and placed in `store`, as a fetch would
/// place it, with no registry: what the tests of the commands that read the store make their store
/// with. Its `ritsu.mod` has to name it with a published name and give its version. `today` is the
/// day the record says it was fetched (`YYYY-MM-DD`), and `verified` the signature lines it says
/// were checked.
pub fn put_directory(store: &Store, dir: &Path, today: &str, verified: &[String]) -> Result<Placed, Text> {
    let file = dir.join(manifest::FILE);
    let bytes = fs::raw::read(&file).map_err(|e| {
        let f = file.display();
        tr!("{f} を読めません（{e}）", "cannot read {f} ({e})")
    })?;
    let m: Manifest = manifest::parse(&bytes, manifest::FILE).map_err(|ds| ds.first().map(|d| d.message.clone()).unwrap_or_default())?;
    let files = digest::module_files(dir, &m)?;
    let built = artifact::build(&m, &bytes, &files)?;
    let version = m.version.as_ref().map(|v| v.value.clone()).unwrap_or_else(|| Version::new(0, 0, 0));
    let record = Record {
        module: m.module.value.to_string(),
        version: version.to_string(),
        files: built.files_digest.clone(),
        fetched: today.to_string(),
        verified: verified.iter().map(|p| (p.clone(), today.to_string())).collect(),
    };
    let digest = place(store, &Parts { manifest: &built.manifest, module: &bytes, files: Some(&files), record }, Some((&m.module.value, &version)))?;
    Ok(Placed { name: m.module.value.clone(), version, digest, files_digest: built.files_digest })
}
