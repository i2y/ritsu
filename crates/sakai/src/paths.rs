//! Paths (DESIGN 2.4, 1.3): how a path written in a `.ctx` becomes a path from the root, where
//! the root is, and which files of the scope are artifacts.
//!
//! A path from the root is a string: the components joined with `/`, with no `.` or `..`, and
//! `.` for the root itself. Every path sakai holds has this form, and so does every path of
//! `api` and of a name in JSON. A diagnostic writes it from where sakai was run ([`shown`]).

use crate::i18n::Text;
use std::path::{Path, PathBuf};

/// Why a written path cannot be a path from the root (E012).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    Absolute,
    Outside,
    /// `""`: the root is written `"."`, and an empty string names nothing.
    Empty,
}

impl PathError {
    pub fn text(&self, written: &str) -> Text {
        match self {
            PathError::Absolute => tr!(
                "絶対パス \"{written}\" は書けません。パスは、書いたファイルのディレクトリからの相対で書きます",
                "the absolute path \"{written}\" cannot be written; a path is written from the directory of the file it is in"
            ),
            PathError::Outside => tr!("パス \"{written}\" はルートの外に出ます", "the path \"{written}\" goes outside the root"),
            PathError::Empty => tr!("パスが空です。書いたファイルのディレクトリそのものは \".\" と書きます", "the path is empty; the directory of the file itself is written \".\""),
        }
    }
}

pub fn is_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    p.starts_with('/') || p.starts_with('\\') || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

/// `rel`, written in a file whose directory is `base` (a path from the root), as a path from the
/// root. `.` and `..` are folded by their letters; a symbolic link is not followed.
pub fn join(base: &str, rel: &str) -> Result<String, PathError> {
    if rel.is_empty() {
        return Err(PathError::Empty);
    }
    if is_absolute(rel) {
        return Err(PathError::Absolute);
    }
    let mut parts: Vec<&str> = if base == "." || base.is_empty() { Vec::new() } else { base.split('/').collect() };
    for c in rel.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(PathError::Outside);
                }
            }
            _ => parts.push(c),
        }
    }
    Ok(if parts.is_empty() { ".".to_string() } else { parts.join("/") })
}

/// The directory a path from the root is in.
pub fn parent(p: &str) -> String {
    match p.rsplit_once('/') {
        Some((d, _)) => d.to_string(),
        None => ".".to_string(),
    }
}

/// Whether the directory `dir` holds `p` (or is it).
pub fn contains(dir: &str, p: &str) -> bool {
    dir == "." || p == dir || (p.len() > dir.len() && p.starts_with(dir) && p.as_bytes()[dir.len()] == b'/')
}

/// How deep a path is: the root is 0, `a` is 1, `a/b` is 2.
pub fn depth(p: &str) -> usize {
    if p == "." { 0 } else { p.split('/').count() }
}

/// `p` as a path from `from` (both from the root): `../x` when it is beside it.
pub fn relative(from: &str, p: &str) -> String {
    let a: Vec<&str> = if from == "." { vec![] } else { from.split('/').collect() };
    let b: Vec<&str> = if p == "." { vec![] } else { p.split('/').collect() };
    let mut i = 0;
    while i < a.len() && i < b.len() && a[i] == b[i] {
        i += 1;
    }
    let mut out: Vec<&str> = std::iter::repeat_n("..", a.len() - i).collect();
    out.extend(&b[i..]);
    if out.is_empty() { ".".to_string() } else { out.join("/") }
}

/// The root (DESIGN 2.4): the nearest directory at or above `start` that holds a `.git` (a
/// directory, or the file a worktree has), found by looking, without running git; else `start`
/// itself, or the directory it is in when it is a file.
pub fn find_root(start: &Path) -> PathBuf {
    let abs = absolute(start);
    let dir = if abs.is_dir() { abs.clone() } else { abs.parent().map(Path::to_path_buf).unwrap_or_else(|| abs.clone()) };
    for d in dir.ancestors() {
        if d.join(".git").exists() {
            return d.to_path_buf();
        }
    }
    dir
}

/// A path made absolute against the working directory, `.` and `..` folded by their letters.
pub fn absolute(p: &Path) -> PathBuf {
    let joined = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A file on the disk, as a path from `root`; None when it is not under it.
pub fn from_root(root: &Path, file: &Path) -> Option<String> {
    let abs = absolute(file);
    let rel = abs.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    Some(if parts.is_empty() { ".".to_string() } else { parts.join("/") })
}

/// A path from the root, on the disk.
pub fn on_disk(root: &Path, p: &str) -> PathBuf {
    if p == "." { root.to_path_buf() } else { root.join(p) }
}

/// How the diagnostics write a path (DESIGN 2.4, 5.1): as the suite's tools do, from where sakai
/// was run, the way the paths given to it are written — relative to the working directory when
/// the path given was relative, absolute when it was absolute. A name in JSON (`api`, and the
/// `name` of a reference in `--format json`) keeps its path from the root.
#[derive(Clone, Debug)]
pub struct Shown {
    root: PathBuf,
    cwd: PathBuf,
    absolute: bool,
}

impl Shown {
    /// For a run whose root is `root` (absolute) and whose first path was written `given`.
    pub fn new(root: &Path, given: &str) -> Shown {
        Shown { root: root.to_path_buf(), cwd: absolute(Path::new(".")), absolute: Path::new(given).is_absolute() }
    }

    /// A path from the root, as the run writes it. A directory may end with `/`, which it keeps.
    pub fn path(&self, p: &str) -> String {
        let (p, slash) = match p.strip_suffix('/') {
            Some(q) if !q.is_empty() => (q, "/"),
            _ => (p, ""),
        };
        let target = on_disk(&self.root, p);
        let s = if self.absolute { target.to_string_lossy().to_string() } else { between(&self.cwd, &target) };
        format!("{s}{slash}")
    }
}

/// The path from the directory `from` to `to`, both absolute: `a/b`, `../c`, or `.`.
pub fn between(from: &Path, to: &Path) -> String {
    let parts = |p: &Path| -> Vec<String> { p.components().filter(|c| matches!(c, std::path::Component::Normal(_))).map(|c| c.as_os_str().to_string_lossy().to_string()).collect() };
    let (a, b) = (parts(from), parts(to));
    let mut i = 0;
    while i < a.len() && i < b.len() && a[i] == b[i] {
        i += 1;
    }
    let mut out: Vec<String> = std::iter::repeat_n("..".to_string(), a.len() - i).collect();
    out.extend(b[i..].iter().cloned());
    if out.is_empty() { ".".to_string() } else { out.join("/") }
}

static SHOWN: std::sync::OnceLock<Shown> = std::sync::OnceLock::new();

/// Said once, by the command line, before anything is checked. The library and the tests that
/// call it leave it unsaid, and their paths are written from the root: the same paths, when the
/// command is run at the root.
pub fn show_from(s: Shown) {
    let _ = SHOWN.set(s);
}

/// A path from the root, as the diagnostics write it.
pub fn shown(p: &str) -> String {
    match SHOWN.get() {
        Some(s) => s.path(p),
        None => p.to_string(),
    }
}

/// The names that put a file out of the scope wherever they are in its path (DESIGN 1.3): the
/// state of tools, installed dependencies and build outputs. A name that starts with `.` is out
/// too (`.git`, `.venv`, `.geas`), as geas's `map` leaves it out.
pub const SKIPPED: &[&str] = &["node_modules", "site-packages", "__pycache__", "target"];

pub fn skipped_name(n: &str) -> bool {
    n.starts_with('.') || SKIPPED.contains(&n)
}

/// Whether a path from the root has a component the scope always leaves out.
pub fn has_skipped_part(p: &str) -> bool {
    p != "." && p.split('/').any(skipped_name)
}

/// Every file under `dir` (a path from the root), in path order, leaving out what
/// [`skipped_name`] names and what `except` holds. Symbolic links are not followed.
pub fn walk(root: &Path, dir: &str, except: &[String], out: &mut Vec<String>) {
    let disk = on_disk(root, dir);
    let Ok(meta) = std::fs::symlink_metadata(&disk) else { return };
    if meta.is_file() {
        if !except.iter().any(|e| contains(e, dir)) && !has_skipped_part(dir) {
            out.push(dir.to_string());
        }
        return;
    }
    if !meta.is_dir() {
        return;
    }
    let Ok(rd) = std::fs::read_dir(&disk) else { return };
    let mut names: Vec<String> = rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    for n in names {
        if skipped_name(&n) {
            continue;
        }
        let p = if dir == "." { n.clone() } else { format!("{dir}/{n}") };
        if except.iter().any(|e| contains(e, &p)) {
            continue;
        }
        let Ok(m) = std::fs::symlink_metadata(on_disk(root, &p)) else { continue };
        if m.is_dir() {
            walk(root, &p, except, out);
        } else if m.is_file() {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_path_becomes_a_path_from_the_root() {
        assert_eq!(join("contexts", "../inventory"), Ok("inventory".into()));
        assert_eq!(join(".", "./calendars/../支払条件.cal"), Ok("支払条件.cal".into()));
        assert_eq!(join(".", "."), Ok(".".into()));
        assert_eq!(join("a/b", "../../.."), Err(PathError::Outside));
        assert_eq!(join(".", "/etc/hosts"), Err(PathError::Absolute));
        assert_eq!(join(".", "C:/x"), Err(PathError::Absolute));
    }

    #[test]
    fn containment_and_depth() {
        assert!(contains(".", "a/b"));
        assert!(contains("a", "a/b"));
        assert!(contains("a", "a"));
        assert!(!contains("a", "ab/c"));
        assert_eq!(depth("."), 0);
        assert_eq!(depth("a/b"), 2);
        assert_eq!(relative("ctx", "proto/x.proto"), "../proto/x.proto");
        assert_eq!(relative(".", "proto"), "proto");
    }
}
