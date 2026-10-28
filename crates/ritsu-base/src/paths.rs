//! Paths (DESIGN 4.7, 6.2): where the root of a project is, how a path written in a file
//! becomes a path from the root, how a path is shown to the person who ran the tool, and
//! which names a walk through a directory passes over.
//!
//! A path from the root is a string: the components joined with `/`, with no `.` or `..`, and
//! `.` for the root itself. A name in JSON carries it, and so does every path a project holds.
//! What a person reads is written from where the tool runs ([`Shown`]), the shortest way: a
//! file under the directory the tool runs in is not written up to the root and down again.
//!
//! geas, yuen and sakai each looked for the root; this is sakai's and yuen's way, one function.

use crate::text::Text;
use crate::tr;
use std::path::{Component, Path, PathBuf};

/// Why a written path cannot be a path from the root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    Absolute,
    /// It climbs above the root.
    Outside,
    /// `""`: the root is written `"."`, and an empty string names nothing.
    Empty,
}

impl PathError {
    pub fn text(&self, written: &str) -> Text {
        match self {
            PathError::Absolute => tr!(
                "`{written}` は絶対パスです。書いたファイルのディレクトリからの相対パスで書いてください",
                "`{written}` is an absolute path; write it from the directory of the file it is in"
            ),
            PathError::Outside => tr!("`{written}` はルートの外に出ます", "`{written}` goes outside the root"),
            PathError::Empty => tr!("パスが空です。書いたファイルのディレクトリそのものは、`\".\"` と書いてください", "the path is empty; the directory of the file itself is written `\".\"`"),
        }
    }
}

/// `/…`, `\…`, or a drive letter (`C:`).
pub fn is_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    p.starts_with('/') || p.starts_with('\\') || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

/// `rel`, written in a file whose directory is `base` (a path from the root, `.` or `""` for the
/// root), as a path from the root. `.` and `..` are folded by their letters, empty parts and a
/// trailing `/` dropped; a symbolic link is not followed.
pub fn join(base: &str, rel: &str) -> Result<String, PathError> {
    if rel.is_empty() {
        return Err(PathError::Empty);
    }
    if is_absolute(rel) {
        return Err(PathError::Absolute);
    }
    let mut parts: Vec<&str> = if base == "." || base.is_empty() { Vec::new() } else { base.split('/').filter(|s| !s.is_empty() && *s != ".").collect() };
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

/// The directory a path from the root is in (`.` for a file at the root).
pub fn parent(p: &str) -> String {
    match p.rsplit_once('/') {
        Some((d, _)) => d.to_string(),
        None => ".".to_string(),
    }
}

/// Whether the directory `dir` holds `p`, or is it (both from the root).
pub fn contains(dir: &str, p: &str) -> bool {
    dir == "." || p == dir || (p.len() > dir.len() && p.starts_with(dir) && p.as_bytes()[dir.len()] == b'/')
}

/// How deep a path is: the root is 0, `a` is 1, `a/b` is 2.
pub fn depth(p: &str) -> usize {
    if p == "." { 0 } else { p.split('/').count() }
}

/// `p` seen from the directory `from`, both from the root: `../x` when it is beside it.
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

/// `Path::is_absolute`, on every target: wasm32-unknown-unknown's std says false of every path,
/// since it knows no root without a drive, and there a path that starts at the root is absolute
/// too (the files a page in the browser hands over are under one, `crate::fs::Memory`). Built
/// for WASI (the npm package, DESIGN 8.8), std's own answer is already that one: a path is
/// absolute when it starts at the root, as on Unix.
pub fn rooted(p: &Path) -> bool {
    if cfg!(all(target_arch = "wasm32", target_os = "unknown")) { p.has_root() } else { p.is_absolute() }
}

/// A path made absolute against the working directory, `.` and `..` folded by their letters
/// (a symbolic link is not followed, so the path stays the one the person wrote). A relative path
/// whose first part starts with `@` is made absolute against the root of the module the working
/// directory is in, when it is in one (DESIGN 17.6): `@pricing/rules/fee.rule` is the same file from
/// any directory of the module.
pub fn absolute(p: &Path) -> PathBuf {
    let joined = if rooted(p) {
        p.to_path_buf()
    } else {
        let cwd = crate::fs::current_dir().unwrap_or_default();
        let at = matches!(p.components().next(), Some(Component::Normal(s)) if s.as_encoded_bytes().first() == Some(&b'@'));
        let base = if at { crate::modules::refer::raw_module_root(&cwd).unwrap_or(cwd) } else { cwd };
        base.join(p)
    };
    let mut out = PathBuf::new();
    for c in joined.components() {
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

/// The root of a project (DESIGN 6.2, item 3; 17.6): the nearest directory at or above `start`
/// that holds a `.git` (a directory, or the file a worktree has) or a `ritsu.mod`, found by looking,
/// without running git; else `start` itself, or the directory it is in when it is a file. The root
/// of a dependency seen through a module (`R/@pricing`) is passed: the root is the module's. `--root`
/// is the caller's to read first.
pub fn find_root(start: &Path) -> PathBuf {
    let abs = absolute(start);
    let dir = if crate::fs::is_dir(&abs) { abs.clone() } else { abs.parent().map(Path::to_path_buf).unwrap_or_else(|| abs.clone()) };
    for d in dir.ancestors() {
        if crate::modules::refer::is_dependency_root(d) {
            continue;
        }
        if crate::fs::exists(d.join(".git")) || crate::fs::is_file(d.join(crate::modules::manifest::FILE)) {
            return d.to_path_buf();
        }
    }
    dir
}

/// A file on the disk as a path from `root` (both absolute or both made so); None when it is
/// not under the root.
pub fn from_root(root: &Path, file: &Path) -> Option<String> {
    let abs = absolute(file);
    let root = absolute(root);
    let rel = abs.strip_prefix(&root).ok()?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    Some(if parts.is_empty() { ".".to_string() } else { parts.join("/") })
}

/// A path from the root, on the disk.
pub fn on_disk(root: &Path, p: &str) -> PathBuf {
    if p == "." { root.to_path_buf() } else { root.join(p) }
}

/// The path from the directory `from` to `to`, both absolute, the shortest way: `a/b`,
/// `../c`, or `.`.
pub fn between(from: &Path, to: &Path) -> String {
    let parts = |p: &Path| -> Vec<String> { p.components().filter(|c| matches!(c, Component::Normal(_))).map(|c| c.as_os_str().to_string_lossy().to_string()).collect() };
    let (a, b) = (parts(from), parts(to));
    let mut i = 0;
    while i < a.len() && i < b.len() && a[i] == b[i] {
        i += 1;
    }
    let mut out: Vec<String> = std::iter::repeat_n("..".to_string(), a.len() - i).collect();
    out.extend(b[i..].iter().cloned());
    if out.is_empty() { ".".to_string() } else { out.join("/") }
}

/// How a run writes a path from the root for a person (DESIGN 6.2, item 8): from where the
/// tool runs, the way the path given to it was written — the shortest relative path when it
/// was relative, absolute when it was absolute.
///
/// Run in a module, a dependency's file is written as it is from the root, `@pricing/rules/fee.rule`,
/// from any directory of the module and however the path was given (DESIGN 17.6): that is the path a
/// command run anywhere in the module takes ([`absolute`] reads it from the module's root). Run from
/// a directory that is not in the root's module, it is written from there as any other file is
/// (`shop/@pricing/rules/fee.rule`), the way a command run there takes it.
#[derive(Clone, Debug)]
pub struct Shown {
    root: PathBuf,
    cwd: PathBuf,
    absolute: bool,
    /// The root has a `ritsu.mod`, and the run is in that module (not above it, beside it, or in a
    /// module below it).
    module: bool,
}

impl Shown {
    /// For a run whose root is `root` and whose first path was written `given`, run in the
    /// working directory.
    pub fn new(root: &Path, given: &str) -> Shown {
        Shown::run_in(root, &absolute(Path::new(".")), given)
    }

    /// The same, run in `cwd`.
    pub fn run_in(root: &Path, cwd: &Path, given: &str) -> Shown {
        let root = absolute(root);
        let cwd = absolute(cwd);
        let module = crate::fs::is_file(root.join(crate::modules::manifest::FILE)) && crate::modules::refer::raw_module_root(&cwd).is_some_and(|m| m == root);
        Shown { root, cwd, absolute: rooted(Path::new(given)), module }
    }

    /// The root itself, as the run writes it: `.` when the tool runs there, `../..` two above.
    pub fn root(&self) -> String {
        self.path(".")
    }

    /// A path from the root, as the run writes it. A directory may end with `/`, which it keeps.
    pub fn path(&self, p: &str) -> String {
        let (p, slash) = match p.strip_suffix('/') {
            Some(q) if !q.is_empty() => (q, "/"),
            _ => (p, ""),
        };
        if self.module && p.starts_with('@') {
            return format!("{p}{slash}");
        }
        let target = on_disk(&self.root, p);
        let s = if self.absolute { target.to_string_lossy().to_string() } else { between(&self.cwd, &target) };
        format!("{s}{slash}")
    }
}

/// The names a walk passes over wherever they are in a path (DESIGN 4.7): what tools keep their
/// state in, installed dependencies, and build outputs. A name that starts with `.` is passed
/// over too (`.git`, `.venv`, `.geas`).
pub const SKIPPED: &[&str] = &["node_modules", "site-packages", "__pycache__", "target"];

pub fn skipped_name(n: &str) -> bool {
    n.starts_with('.') || SKIPPED.contains(&n)
}

/// Whether a path from the root has a part a walk always passes over.
pub fn has_skipped_part(p: &str) -> bool {
    p != "." && p.split('/').any(skipped_name)
}

/// Every file under `dir` (a path from the root), in path order, passing over what
/// [`skipped_name`] names and what `except` holds. Symbolic links are not followed. A directory
/// below the root with a `ritsu.mod` of its own is another module (DESIGN 17.6), and is passed over
/// too ([`walk_noting`] says which).
pub fn walk(root: &Path, dir: &str, except: &[String], out: &mut Vec<String>) {
    let mut modules = Vec::new();
    walk_noting(root, dir, except, out, &mut modules);
}

/// [`walk`], with each module it passed over (a directory below the root with a `ritsu.mod` of its
/// own, by its path from the root) in `modules`. The root of a dependency seen through the root's
/// module (`@pricing`, `@pricing/@tax`) is walked: it is the dependency's files.
pub fn walk_noting(root: &Path, dir: &str, except: &[String], out: &mut Vec<String>, modules: &mut Vec<String>) {
    let disk = on_disk(root, dir);
    let Ok(meta) = crate::fs::symlink_metadata(&disk) else { return };
    if meta.is_file() {
        if !except.iter().any(|e| contains(e, dir)) && !has_skipped_part(dir) {
            out.push(dir.to_string());
        }
        return;
    }
    if !meta.is_dir() {
        return;
    }
    if nested_module(root, dir) {
        modules.push(dir.to_string());
        return;
    }
    let Ok(rd) = crate::fs::read_dir(&disk) else { return };
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
        let Ok(m) = crate::fs::symlink_metadata(on_disk(root, &p)) else { continue };
        if m.is_dir() {
            walk_noting(root, &p, except, out, modules);
        } else if m.is_file() {
            out.push(p);
        }
    }
}

/// For a tool that walks a directory a person named in its own way (`rulec check rules/`, `koyomi
/// check .`, `yuen check .`): whether a directory the walk met below where it started is one it does
/// not enter (DESIGN 17.6). That is another module (a directory with a `ritsu.mod` of its own; the root
/// of a dependency seen through a module, `shop/@pricing`, is the dependency's files, and is entered),
/// or the `.ritsu` beside a `ritsu.mod`, which holds the module's copies of its dependencies. Each tool
/// keeps the names it passed over before: outside a module, nothing changes.
pub fn not_entered(dir: &Path) -> Option<NotEntered> {
    let file = crate::modules::manifest::FILE;
    if dir.file_name().is_some_and(|n| n == ".ritsu") && dir.parent().is_some_and(|p| crate::fs::is_file(p.join(file))) {
        return Some(NotEntered::State);
    }
    if crate::fs::is_file(dir.join(file)) && !crate::modules::refer::is_dependency_root(dir) {
        return Some(NotEntered::Module);
    }
    None
}

/// What [`not_entered`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotEntered {
    /// Another module: said in one line ([`other_module_note`]) when the walk did not start in a module.
    Module,
    /// The `.ritsu` beside a `ritsu.mod`: passed over without a word.
    State,
}

/// The line a walk says of a module it passed over (DESIGN 17.6), when where it started is in no
/// module: `dir` as the run writes it, and the command that reads it (`ritsu check`, `rulec check`).
pub fn other_module_note(dir: &str, command: &str) -> Text {
    tr!(
        "備考: `{dir}` は自分の `ritsu.mod` を持つ別のモジュールなので、読んでいません。読むには `{command} {dir}` を実行してください",
        "note: `{dir}` has a `ritsu.mod` of its own: it is another module, and was not read; run `{command} {dir}` to read it"
    )
}

/// Whether the directory `dir` (a path from the root) is a module of its own below the root: it has
/// a `ritsu.mod`, and it is not the root, nor a dependency's root seen through the root's module.
fn nested_module(root: &Path, dir: &str) -> bool {
    let file = crate::modules::manifest::FILE;
    if dir == "." || !crate::fs::is_file(on_disk(root, dir).join(file)) {
        return false;
    }
    !(dir.split('/').all(|p| p.starts_with('@')) && crate::fs::is_file(root.join(file)))
}
