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
/// any directory of the module. The file a path names, as a run tells files apart, reads them, finds
/// the root above them and names them from it, is [`opened`].
pub fn absolute(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in joined(p).components() {
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

/// `p` made absolute as [`absolute`] makes it, before anything is folded.
fn joined(p: &Path) -> PathBuf {
    if rooted(p) {
        return p.to_path_buf();
    }
    let cwd = crate::fs::current_dir().unwrap_or_default();
    let at = matches!(p.components().next(), Some(Component::Normal(s)) if s.as_encoded_bytes().first() == Some(&b'@'));
    let base = if at { crate::modules::refer::raw_module_root(&cwd).unwrap_or(cwd) } else { cwd };
    base.join(p)
}

/// The file a path names (DESIGN 4.15): the path made absolute as [`absolute`] makes it, and folded
/// so that it opens the file the system opens. `x/..` is folded by its letters where `x` is no
/// symbolic link; where `x` is one, it is replaced by where the link leads (read from the directory
/// it is in, and folded the same way) and the `..` is taken from there, as the system takes it. A
/// path that crosses no link with `..` is [`absolute`]'s, the path the person wrote: a link the path
/// goes through without `..` is not followed (`shared/booking.flow` stays that, wherever `shared`
/// leads). A part that is not there is no link, so a path to nothing is folded by its letters, and
/// so is every path of the files of a page, which hold no link ([`crate::fs::link`]). After 40 links
/// in one path, a loop, the rest is folded by its letters.
pub fn opened(p: &Path) -> PathBuf {
    opened_by(p, &|at| crate::fs::link(at))
}

/// [`opened`], with where each symbolic link leads asked of `link` in place of the files of the
/// thread: what files laid over others ask of the files under them, for a path that goes up out of
/// a directory only the files laid make (`crate::fs::Overlay`).
pub fn opened_by(p: &Path, link: &dyn Fn(&Path) -> Option<PathBuf>) -> PathBuf {
    let mut out = PathBuf::new();
    // the parts still to take, the next one last
    let mut todo: Vec<std::ffi::OsString> = Vec::new();
    take_parts(&joined(p), &mut out, &mut todo);
    let mut links = 0;
    while let Some(part) = todo.pop() {
        if part != ".." {
            out.push(&part);
            continue;
        }
        if links < 40
            && out.file_name().is_some()
            && let Some(to) = link(&out)
        {
            links += 1;
            out.pop();
            // the `..` again, once where the link leads is taken
            todo.push(part);
            take_parts(&to, &mut out, &mut todo);
            continue;
        }
        out.pop();
    }
    out
}

/// The parts of `p` after its root put on `todo`, the first one last (`.` left out), and `out` made
/// `p`'s root when `p` has one.
fn take_parts(p: &Path, out: &mut PathBuf, todo: &mut Vec<std::ffi::OsString>) {
    let at = todo.len();
    let mut head = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => head.push(c.as_os_str()),
            Component::CurDir => {}
            other => todo.push(other.as_os_str().to_os_string()),
        }
    }
    if !head.as_os_str().is_empty() {
        *out = head;
    }
    todo[at..].reverse();
}

/// Whether [`opened`] and [`absolute`] make one path of `p`: whether the file the system opens is
/// the one the letters of `p` name (DESIGN 4.15). What a tool that names a file by its letters
/// unless a `..` crosses a link asks of a path, as often as it joins one (rulec's contributions).
/// The files are asked only where `p` has a `..`. A relative path that goes through no module's
/// `@` is asked of its own parts, from the working directory: a `..` that climbs above the working
/// directory folds by its letters, for the working directory is named with no link in it
/// ([`crate::fs::Files::current_dir`]), and each `..` below is asked of the parts before it, read
/// from there. Where one of those crosses a link, and for an absolute path or one through an `@`,
/// the answer is [`opened`]'s and [`absolute`]'s, which read the working directory for a relative
/// path.
pub fn folds_by_letters(p: &Path) -> bool {
    if !p.components().any(|c| c == Component::ParentDir) {
        return true;
    }
    let through_a_module = p.components().any(|c| matches!(c, Component::Normal(s) if s.as_encoded_bytes().first() == Some(&b'@')));
    if !rooted(p) && !through_a_module {
        // the `..` that climb above the working directory, and the parts below where they lead
        let mut above = PathBuf::new();
        let mut out = PathBuf::new();
        let mut among = true;
        for c in p.components() {
            match c {
                Component::CurDir => {}
                Component::Normal(s) => out.push(s),
                Component::ParentDir if out.as_os_str().is_empty() => above.push(".."),
                Component::ParentDir if crate::fs::link(&above.join(&out)).is_none() => {
                    out.pop();
                }
                _ => {
                    among = false;
                    break;
                }
            }
        }
        if among {
            return true;
        }
    }
    let whole = joined(p);
    opened(&whole) == absolute(&whole)
}

/// The root of a project (DESIGN 6.2, item 3; 17.6): the nearest directory at or above `start`
/// that holds a `.git` (a directory, or the file a worktree has) or a `ritsu.mod`, found by looking,
/// without running git; else `start` itself, or the directory it is in when it is a file. `start` is
/// the file the system opens ([`opened`], DESIGN 4.15): a path whose `..` crosses a symbolic link
/// finds the root above where the link leads, as the path written to there does. The root of a
/// dependency seen through a module (`R/@pricing`) is passed: the root is the module's. `--root` is
/// the caller's to read first.
pub fn find_root(start: &Path) -> PathBuf {
    let abs = opened(start);
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
/// not under the root. The file is the one the system opens ([`opened`], DESIGN 4.15): a path whose
/// `..` crosses a symbolic link is named by where it leads, and two paths of one file have one name.
pub fn from_root(root: &Path, file: &Path) -> Option<String> {
    let abs = opened(file);
    let root = opened(root);
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

/// The path from the directory `file` is in to `other`, both folded as [`absolute`] folds them: how
/// a file names what it reads wherever the two are held, so that a copy of a tree at another place
/// names the same files.
pub fn seen_from(file: &Path, other: &Path) -> String {
    let file = absolute(file);
    between(file.parent().unwrap_or(&file), &absolute(other))
}

/// How a run writes a path from the root for a person (DESIGN 6.2, item 8): from where the
/// tool runs, the way the path given to it was written — the shortest relative path when it
/// was relative, absolute when it was absolute.
///
/// Run in a module, a dependency's file is written as it is from the root, `@pricing/rules/fee.rule`,
/// from any directory of the module and however the path was given (DESIGN 17.6): that is the path a
/// command run anywhere in the module takes ([`absolute`] reads it from the module's root). Run from
/// a directory that is not in the root's module, it is written from there as any other file is
/// (`shop/@pricing/rules/fee.rule`), the way a command run there takes it. Run in a module, a file
/// below the directory whose first part there starts with `@`, which a command would read from the
/// module's root, is written `./@notes.rule`.
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
        let s = if self.absolute { target.to_string_lossy().to_string() } else { from_where_run(&self.cwd, &target, || crate::modules::refer::raw_module_root(&self.cwd).is_some()) };
        format!("{s}{slash}")
    }
}

/// The path from the directory a run is in to a file (both absolute), the shortest way, as a command
/// run there takes it back: in a module, a path whose first part starts with `@` is read from the
/// module's root, into a dependency (DESIGN 17.6), so a file below the directory whose first part
/// starts with `@` is written `./@notes.rule`. `in_module` is asked only then.
fn from_where_run(cwd: &Path, file: &Path, in_module: impl FnOnce() -> bool) -> String {
    let s = between(cwd, file);
    if s.starts_with('@') && in_module() { format!("./{s}") } else { s }
}

/// How a run writes the paths it holds (DESIGN 4.15): the directory it runs in, whether the first
/// path given to it was absolute, and the module it runs in.
#[derive(Clone, Debug)]
struct Way {
    cwd: PathBuf,
    absolute: bool,
    module: Option<PathBuf>,
}

thread_local! {
    static WAY: std::cell::RefCell<Option<Way>> = const { std::cell::RefCell::new(None) };
}

/// What [`run_from`] answers: while it is held, the thread's run writes its paths the way the
/// first path given to it was written; dropped, the way before comes back.
pub struct Run {
    before: Option<Way>,
}

impl Drop for Run {
    fn drop(&mut self) {
        let before = self.before.take();
        WAY.with(|w| *w.borrow_mut() = before);
    }
}

/// Said by a command before it reads a file (DESIGN 4.15): the paths the run holds are written
/// from the directory it runs in, absolute when `given`, the first path given to it (`.` when
/// none was), is absolute. Every file the run reaches, however it was reached (as the person
/// wrote it, from the root, joined to a path a file wrote), is then written one way, the way
/// [`Shown`] writes a path from the root.
pub fn run_from(given: &str) -> Run {
    let cwd = absolute(Path::new("."));
    let module = crate::modules::refer::raw_module_root(&cwd);
    let way = Way { cwd, absolute: rooted(Path::new(given)), module };
    Run { before: WAY.with(|w| w.borrow_mut().replace(way)) }
}

/// A path a tool holds (as it was given, joined to what a file wrote, from the root on the disk) as
/// the run writes it to the person (DESIGN 4.15): `.` and `..` folded as [`opened`] folds them, so
/// that the path opens, from the directory the run is in, the file the run read; the shortest way
/// from that directory, or absolute when the run's first path was. Run in a module, a
/// dependency's file is `@pricing/rules/fee.rule`, as [`Shown`] writes it, and a file of the module
/// below the directory whose first part there starts with `@` is `./@notes.rule`. Outside a run (a
/// library's call, a test), the path is written absolute when it is held absolute.
pub fn show(held: &Path) -> String {
    let abs = opened(held);
    let (cwd, absolute_run, module) = match WAY.with(|w| w.borrow().clone()) {
        Some(w) => (w.cwd, w.absolute, w.module),
        None => {
            let cwd = absolute(Path::new("."));
            let module = crate::modules::refer::raw_module_root(&cwd);
            (cwd, rooted(held), module)
        }
    };
    if let Some(m) = &module
        && let Ok(rest) = abs.strip_prefix(m)
        && rest.components().next().is_some_and(|c| c.as_os_str().as_encoded_bytes().first() == Some(&b'@'))
    {
        return rest.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/");
    }
    if absolute_run { abs.to_string_lossy().to_string() } else { from_where_run(&cwd, &abs, || module.is_some()) }
}

/// [`show`] of a path held as a string.
pub fn show_str(held: &str) -> String {
    show(Path::new(held))
}

/// A path given to a command, as the run holds it and writes it (DESIGN 4.15, 6.2 item 8): [`show`]
/// of it, so that `./fee.rule`, `x/../fee.rule` and `fee.rule` are one path, read and said alike,
/// and a path whose `..` climbs out of a symbolic link is the file the system opens there. What a
/// command takes in the place of a path but is no path of the files is kept as written: `-` (the
/// standard input), a path through a module's dependency (`@pricing/…`, and a side of `diff` at a
/// version, `@pricing@1.0.0/…`, DESIGN 17.6 and 17.9.1), and an address (`https://…`), and so is a
/// path the system does not open as it is written ([`opens_as_written`]).
pub fn show_given(given: &str) -> String {
    if no_path(given) || !opens_as_written(given) {
        return given.to_string();
    }
    show_str(given)
}

/// What a command takes in the place of a path that is no path of the files ([`show_given`]).
fn no_path(given: &str) -> bool {
    given.is_empty() || given == "-" || given.starts_with('@') || given.contains("://")
}

/// Whether the system opens a path given as it is written: what each `..` climbs out of is a
/// directory (or a link to one), and a path that ends with `/` is no file. The system opens neither
/// `x/../fee.rule` where there is no `x` nor `fee.rule/`; folded, they would be `fee.rule`, and the
/// run would read what it was not given. Kept as written, they are read and refused as they were
/// before the paths given were folded. A path that is not there at all is folded: it is refused
/// alike either way, and an `--out` is made.
fn opens_as_written(given: &str) -> bool {
    let mut before = PathBuf::new();
    for c in Path::new(given).components() {
        if c == Component::ParentDir && !before.as_os_str().is_empty() && !crate::fs::is_dir(&before) {
            return false;
        }
        before.push(c);
    }
    !(given.len() > 1 && given.ends_with('/') && crate::fs::is_file(Path::new(given.trim_end_matches('/'))))
}

/// A path a flag takes (`--out`, `--scenario`), as the run holds it and writes it (DESIGN 4.15, 6.2
/// item 8): `.` and `..` folded as [`opened`] folds them, so that `./generated`, `generated/` and
/// `x/../generated` are `generated`; from the directory the run is in, the shortest way, when it was
/// given relative, and absolute when it was given absolute, whichever way the run writes the files
/// it reads. What is no path of the files, and a path the system does not open as it is written,
/// are kept as written, as [`show_given`] keeps them.
pub fn fold_given(given: &str) -> String {
    if no_path(given) || !opens_as_written(given) {
        return given.to_string();
    }
    let p = Path::new(given);
    let abs = opened(p);
    if rooted(p) {
        return abs.to_string_lossy().to_string();
    }
    let cwd = absolute(Path::new("."));
    from_where_run(&cwd, &abs, || crate::modules::refer::raw_module_root(&cwd).is_some())
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
