//! Where a path written in a file leads (DESIGN 17.6), and how a dependency's files are seen.
//!
//! In a module, a path written in a file stays under the module's root once its `.` and `..` are
//! folded, or goes into a dependency by `@<name>/`: a path that leads out of the root, or into a
//! module below it that has a `ritsu.mod` of its own without an `@`, is refused. A path that starts
//! with `@` is read from the root of the module the file is in, not from the file's directory.
//! Outside a module — no `ritsu.mod` above the file, up to the nearest `.git` — a path is what it was
//! before: [`refer`] answers `dir.join(written)` and [`join`] answers [`crate::paths::join`], byte for
//! byte (M14).
//!
//! A dependency `<name>` of the module whose root is `R` is seen at `R/@<name>`, through the layer
//! that reads a project's files ([`crate::fs`]): nothing is made on the disk. [`map`] says where such
//! a path is: the dependency's directory, the store's entry, or a module's copy of it. A dependency's
//! dependencies are seen the same way inside it (`R/@pricing/@tax/…`), and the lock of `R`, the real
//! module the chain starts from, pins every version. A listing of `R` does not show `@<name>`, so that
//! a walk of a module does not go into its dependencies.

use super::graph::{self, Problem, Tree};
use super::manifest;
use super::store::{self, Store};
use crate::fs;
use crate::paths::{self, PathError};
use crate::text::Text;
use crate::tr;
use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

/// Why a path written in a file is refused.
#[derive(Clone, Debug)]
pub enum Refused {
    /// Outside a module, what [`crate::paths::join`] says ([`join`] only), and the path.
    Path(PathError, String),
    /// An absolute path, in a module.
    Absolute(String),
    /// An empty path, in a module.
    Empty,
    /// `@<name>` where the module the file is in requires no `<name>`.
    NotRequired { written: String, name: String, module: String },
    /// A path that leads out of the module, or out of a dependency by `..`.
    Outside { written: String, module: String },
    /// A path that goes into a module below without an `@`: the path, and that module's directory as
    /// a run writes it.
    Nested { written: String, nested: String },
    /// The tree of the module cannot be read (its diagnostics or what stops it): the first thing it
    /// says.
    Tree(Text),
}

impl Refused {
    /// The sentence each language says with its own code for a wrong reference (DESIGN 17.20).
    pub fn text(&self) -> Text {
        match self {
            Refused::Path(e, w) => e.text(w),
            Refused::Absolute(w) => tr!(
                "`{w}` は絶対パスです。モジュールの中では、書いたファイルのディレクトリからの相対パスか、`@<名前>/` で始まるパスで書いてください",
                "`{w}` is an absolute path; in a module, write a path from the directory of the file it is in, or one that starts with `@<name>/`"
            ),
            Refused::Empty => tr!("パスが空です", "the path is empty"),
            Refused::NotRequired { written, name, module } => tr!(
                "`{written}` の `@{name}` は依存ではありません。モジュール `{module}` の `ritsu.mod` に `require {name} …` がありません",
                "the `@{name}` of `{written}` is no dependency: the `ritsu.mod` of the module `{module}` has no `require {name} …`"
            ),
            Refused::Outside { written, module } => tr!(
                "`{written}` は、モジュール `{module}` の外を指します。モジュールの中のファイルが指せるのは、モジュールのルートの下と、`@<名前>/` で入る依存の中です",
                "`{written}` leads outside the module `{module}`; a file in a module names files under the module's root, and in a dependency by `@<name>/`"
            ),
            Refused::Nested { written, nested } => tr!(
                "`{written}` は、`ritsu.mod` を持つ下のディレクトリ `{nested}` の中を、`@` を通らずに指しています。そのモジュールを `require` に足し、`@<名前>/` で指してください",
                "`{written}` leads into `{nested}`, a directory below with a `ritsu.mod` of its own, without an `@`; require that module, and name its files by `@<name>/`"
            ),
            Refused::Tree(t) => tr!("依存の木を読めません。", "the tree of dependencies cannot be read: ").then(t),
        }
    }
}

/// The note a language adds when a path that starts with `@` names no file and no module is above
/// the file it is written in (M14): there, `@…` is a plain relative path.
pub fn outside_note() -> Text {
    tr!(
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです。このファイルはモジュールの中に無いので、ふつうの相対パスとして読みました",
        "a path that starts with `@` names a dependency only in a module, a directory with a `ritsu.mod`; this file is in none, so it was read as a plain relative path"
    )
}

/// E307's sentence: a name that starts with `@` right under the root of a module.
pub fn at_name(name: &str) -> Text {
    tr!(
        "モジュールのルートに `{name}` があります。ルートのすぐ下の `@` で始まる名前は依存を指すので、ファイルやディレクトリには付けられません。名前を変えてください",
        "the root of the module has `{name}` in it; a name that starts with `@` right under the root names a dependency, so no file or directory takes one: rename it"
    )
}

fn starts_at(s: &OsStr) -> bool {
    s.as_encoded_bytes().first() == Some(&b'@')
}

/// Whether a path has a part that starts with `@`: the only paths [`map`] may change.
pub fn has_at(p: &Path) -> bool {
    p.components().any(|c| matches!(c, Component::Normal(s) if starts_at(s)))
}

fn raw_file(p: &Path) -> bool {
    fs::raw::metadata(p).is_ok_and(|m| m.is_file())
}

/// The root of the module the directory `dir` (absolute, on the files the layer reads, no view) is
/// in: the nearest directory at or above it with a `ritsu.mod`, not past the nearest one with a
/// `.git`.
pub fn raw_module_root(dir: &Path) -> Option<PathBuf> {
    for d in dir.ancestors() {
        if raw_file(&d.join(manifest::FILE)) {
            return Some(d.to_path_buf());
        }
        if fs::raw::metadata(&d.join(".git")).is_ok() {
            return None;
        }
    }
    None
}

// ── The tree of each module, kept on the thread ─────────────────────────

struct Cached {
    root: PathBuf,
    files: usize,
    store: Option<PathBuf>,
    inputs: graph::Inputs,
    tree: Rc<Result<Tree, Problem>>,
}

thread_local! {
    static TREES: RefCell<Vec<Cached>> = const { RefCell::new(Vec::new()) };
}

/// Which files the layer reads on this thread: the disk (0), or what [`crate::fs::with`] holds, seen
/// through a watcher of them ([`crate::fs::id`]).
fn files_id() -> usize {
    fs::id()
}

/// The tree of the module whose root is `root`, read once on this thread and kept while the files it
/// was read from (`ritsu.mod`, `ritsu.lock`, and the `ritsu.mod` of each module a path reaches) read
/// the same bytes, from the same files ([`crate::fs::with`]) and the same store.
pub fn tree(root: &Path) -> Rc<Result<Tree, Problem>> {
    let root = graph::fold(&paths::absolute(root));
    let (id, place) = (files_id(), store::location());
    let hit = TREES.with(|t| t.borrow().iter().find(|c| c.root == root && c.files == id && c.store == place).map(|c| (c.inputs.clone(), c.tree.clone())));
    if let Some((inputs, tree)) = hit
        && inputs.iter().all(|(p, b)| fs::raw::read(p).ok() == *b)
    {
        return tree;
    }
    let (r, inputs) = graph::load_with(&root, place.clone().map(Store::at).as_ref());
    let tree = Rc::new(r);
    TREES.with(|t| {
        let mut t = t.borrow_mut();
        t.retain(|c| !(c.root == root && c.files == id && c.store == place));
        t.push(Cached { root, files: id, store: place, inputs, tree: tree.clone() });
    });
    tree
}

/// Every tree kept on this thread let go: a page in the browser whose files changed, a test.
pub fn forget() {
    TREES.with(|t| t.borrow_mut().clear());
}

// ── The view ────────────────────────────────────────────────────────────

/// A path seen through the view: the real root of the module the chain starts from, the nodes of its
/// tree the chain reaches, and the parts after the chain.
struct Seen {
    head: PathBuf,
    tree: Rc<Result<Tree, Problem>>,
    node: usize,
    /// The parts of the chain, `@pricing`, `@tax`.
    chain: Vec<OsString>,
    rest: Vec<OsString>,
}

/// `p` (absolute, folded) seen through the view, when a part of it right under a module's root
/// starts with `@` and names a dependency of that module.
fn seen(abs: &Path) -> Option<Seen> {
    let comps: Vec<Component> = abs.components().collect();
    let mut prefix = PathBuf::new();
    for (i, c) in comps.iter().enumerate() {
        if let Component::Normal(s) = c
            && starts_at(s)
            && raw_file(&prefix.join(manifest::FILE))
        {
            let tree = tree(&prefix);
            let Ok(t) = tree.as_ref() else { return None };
            let mut node = 0;
            let mut j = i;
            while let Some(Component::Normal(s)) = comps.get(j) {
                match s.to_str().and_then(|s| s.strip_prefix('@')).and_then(|n| t.dep(node, n)) {
                    Some(d) => {
                        node = d;
                        j += 1;
                    }
                    None => break,
                }
            }
            if j == i {
                return None;
            }
            let part = |c: &Component| c.as_os_str().to_os_string();
            return Some(Seen { head: prefix, chain: comps[i..j].iter().map(part).collect(), rest: comps[j..].iter().map(part).collect(), tree, node });
        }
        prefix.push(c.as_os_str());
    }
    None
}

/// Where a path that goes through a dependency is (DESIGN 17.6): `R/@pricing/rules/fee.rule` is the
/// file `rules/fee.rule` of the directory the dependency `pricing` of the module `R` is read from. A
/// relative path whose first part starts with `@` is read from the root of the module the working
/// directory is in. None for any other path, and when the module's tree cannot be read.
pub fn map(p: &Path) -> Option<PathBuf> {
    if !has_at(p) {
        return None;
    }
    let s = seen(&graph::fold(&paths::absolute(p)))?;
    let t = s.tree.as_ref().as_ref().ok()?;
    let mut out = t.nodes[s.node].dir.clone();
    for r in &s.rest {
        out.push(r);
    }
    Some(out)
}

/// The same path made canonical as the view writes it, for [`crate::fs::canonicalize`]: the real root
/// of the module the chain starts from, then the chain and the rest. None as [`map`] is none.
pub fn view_parts(p: &Path) -> Option<(PathBuf, PathBuf, PathBuf)> {
    if !has_at(p) {
        return None;
    }
    let s = seen(&graph::fold(&paths::absolute(p)))?;
    let t = s.tree.as_ref().as_ref().ok()?;
    let mut real = t.nodes[s.node].dir.clone();
    let mut after = PathBuf::new();
    for c in &s.chain {
        after.push(c);
    }
    for r in &s.rest {
        real.push(r);
        after.push(r);
    }
    Some((real, s.head, after))
}

/// The root of the dependency `p` is in, as the view writes it (`R/@pricing`, `R/@pricing/@tax`):
/// what a `.proto` of the dependency reads its imports from (DESIGN 17.6). None for a path in no
/// dependency, and when the module's tree cannot be read.
pub fn dependency_root(p: &Path) -> Option<PathBuf> {
    let (_, mut root, after) = view_parts(p)?;
    for c in after.components() {
        match c {
            Component::Normal(s) if starts_at(s) => root.push(s),
            _ => break,
        }
    }
    Some(root)
}

/// Whether `p` is under a dependency as the view sees it, its tree read or not: a part that starts
/// with `@` right under a directory with a `ritsu.mod`. What is written there is refused.
pub fn under_dependency(p: &Path) -> bool {
    if !has_at(p) {
        return false;
    }
    let abs = graph::fold(&paths::absolute(p));
    let mut prefix = PathBuf::new();
    for c in abs.components() {
        if let Component::Normal(s) = c
            && starts_at(s)
            && raw_file(&prefix.join(manifest::FILE))
        {
            return true;
        }
        prefix.push(c.as_os_str());
    }
    false
}

/// Whether `d` is the root of a dependency as the view sees it (`R/@pricing`): not a root a search for
/// the root of a project stops at.
pub fn is_dependency_root(d: &Path) -> bool {
    d.file_name().is_some_and(starts_at) && map(d).is_some()
}

/// The root of the module the directory `dir` is in, as the view sees it: the nearest directory at or
/// above it that has a `ritsu.mod`, not past the nearest one with a `.git`. A dependency's root seen
/// through the view (`R/@pricing`) is the root of the dependency's files.
pub fn module_of(dir: &Path) -> Option<PathBuf> {
    let abs = graph::fold(&paths::absolute(dir));
    for d in abs.ancestors() {
        if fs::is_file(d.join(manifest::FILE)) {
            return Some(d.to_path_buf());
        }
        if fs::exists(d.join(".git")) {
            return None;
        }
    }
    None
}

/// The leading parts of a path from the root that start with `@`, joined: the dependency a file of
/// the tree is in, as the JSON writes it (`@pricing/@tax`). None for a file of the module at hand.
pub fn prefix_of(rel: &str) -> Option<String> {
    let parts: Vec<&str> = rel.split('/').take_while(|p| p.starts_with('@')).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The disk path a path written in a file leads to (DESIGN 17.6), the file's directory being `dir`:
/// what each language calls where it joined `dir` and the path before. Outside a module, `dir.join(
/// written)`, byte for byte (M14). In a module, the canonical form: the root of the real module the
/// chain starts from (absolute), then the path from it ([`graph::Node::prefix`] and the rest).
pub fn refer(dir: &Path, written: &str) -> Result<PathBuf, Refused> {
    let Some(m) = module_of(dir) else { return Ok(dir.join(written)) };
    resolve(&graph::fold(&paths::absolute(dir)), &m, written)
}

/// [`refer`]'s answer as a language writes it that keeps one path both to read a file and to show it,
/// a path from the working directory as the path given to the run was (DESIGN 17.6). Outside a
/// module, `dir.join(written)`, byte for byte (M14). A file of the module's own is `dir.join(written)`
/// too, as it was written before there were modules. A file in a dependency is written from the root
/// of the module the chain starts from, `@pricing/rules/fee.rule`, when the working directory is in
/// that module: from any of its directories, that path names the file. From anywhere else, it is the
/// path from the working directory to the file (absolute when `dir` is), which names it from there.
pub fn refer_shown(dir: &Path, written: &str) -> Result<PathBuf, Refused> {
    let target = refer(dir, written)?;
    if module_of(dir).is_none() {
        return Ok(target);
    }
    let Some((_, head, after)) = view_parts(&target) else { return Ok(dir.join(written)) };
    if paths::rooted(dir) {
        return Ok(target);
    }
    let cwd = graph::fold(&paths::absolute(Path::new(".")));
    if raw_module_root(&cwd).is_some_and(|r| r == head) { Ok(after) } else { Ok(PathBuf::from(paths::between(&cwd, &target))) }
}

/// [`outside_note`], when it is what a language adds to its "no such file": `written` starts with `@`,
/// and no module is above the directory `dir` it was written in.
pub fn outside_note_for(dir: &Path, written: &str) -> Option<Text> {
    (written.starts_with('@') && module_of(dir).is_none()).then(outside_note)
}

/// A path given to a run on the command line (DESIGN 17.6). In a module (the working directory's),
/// one that starts with `@` is read as a path written in a file of the working directory is read
/// ([`refer`]): from the module's root, into the dependency it names. Err says what is wrong with it
/// in the words a path written in a file is refused with ([`Refused::text`], [`missing`]), rather
/// than that it cannot be read: a dependency the module does not require, a path that leads out of
/// the module or into a module below without an `@`, a file the dependency does not have, a tree of
/// dependencies that cannot be read. Any other path, and any path outside a module, is the run's to
/// read as it did before (M14).
pub fn given(path: &str) -> Result<(), Text> {
    let here = Path::new(".");
    if !path.starts_with('@') || module_of(here).is_none() {
        return Ok(());
    }
    let target = refer(here, path).map_err(|r| r.text())?;
    if fs::exists(&target) {
        return Ok(());
    }
    missing(&target).map_or(Ok(()), Err)
}

/// [`given`] for each of the paths given: what is wrong with the first one that is refused.
pub fn given_all<S: AsRef<str>>(paths: &[S]) -> Result<(), Text> {
    paths.iter().try_for_each(|p| given(p.as_ref()))
}

/// The path from the root `root` that a path written in the file `from` (a path from the root) leads
/// to: what the ports of the languages called [`crate::paths::join`] for. Outside a module, that,
/// byte for byte (M14); in a module, the canonical form, from the root.
pub fn join(root: &Path, from: &str, written: &str) -> Result<String, Refused> {
    let dir = paths::parent(from);
    let disk = paths::on_disk(root, &dir);
    let Some(m) = module_of(&disk) else {
        return paths::join(&dir, written).map_err(|e| Refused::Path(e, written.to_string()));
    };
    let target = resolve(&graph::fold(&paths::absolute(&disk)), &m, written)?;
    match paths::from_root(root, &target) {
        Some(r) => Ok(r),
        None => Err(Refused::Outside { written: written.to_string(), module: module_name(&m) }),
    }
}

/// The path `path` (from the root `root`, as [`join`] answers it) as the module of the file `from`
/// (from the same root) writes it from its own root, in the canonical form that module would give
/// were it the module at hand: a dependency's own files without the `@<name>/` they are seen through
/// (`@pricing/rules/fee.rule` is `rules/fee.rule` for a file of `@pricing`), the files of its
/// dependencies by its own `require` names, the fewest `@` first and then the order of its
/// `require`s (`@pricing/@tax/x` and `@tax/x` are both `@tax/x` for a file of `@pricing` that requires
/// `tax`). What a language keeps that has to be the same whether a module is the one at hand or a
/// dependency, such as a hash of what a file says (DESIGN 17.6). A file of the module at hand, a root
/// with no `ritsu.mod` (M14), and a tree that cannot be read: `path` as it is.
pub fn from_own_root(root: &Path, from: &str, path: &str) -> String {
    let as_is = || path.to_string();
    let Some(own) = prefix_of(from) else { return as_is() };
    if !raw_file(&root.join(manifest::FILE)) {
        return as_is();
    }
    let tree = tree(root);
    let Ok(t) = tree.as_ref() else { return as_is() };
    let (Some(m), at) = (t.by_prefix(&own), prefix_of(path).unwrap_or_default()) else { return as_is() };
    let Some(target) = t.by_prefix(&at) else { return as_is() };
    let rest = if at.is_empty() { path } else { path.get(at.len() + 1..).unwrap_or("") };
    // the fewest `@` from the module of `from`, the `require`s in their order
    let mut seen = vec![m];
    let mut level: Vec<(usize, Vec<&str>)> = vec![(m, Vec::new())];
    while !level.is_empty() {
        if let Some((_, chain)) = level.iter().find(|(n, _)| *n == target) {
            let mut out: Vec<String> = chain.iter().map(|c| format!("@{c}")).collect();
            if !rest.is_empty() {
                out.push(rest.to_string());
            }
            return if out.is_empty() { ".".to_string() } else { out.join("/") };
        }
        let mut next = Vec::new();
        for (n, chain) in &level {
            for (name, d) in &t.nodes[*n].deps {
                if !seen.contains(d) {
                    seen.push(*d);
                    let mut c = chain.clone();
                    c.push(name.as_str());
                    next.push((*d, c));
                }
            }
        }
        level = next;
    }
    as_is()
}

/// The name of the module whose root (as the view sees it) is `m`, for a sentence.
fn module_name(m: &Path) -> String {
    fs::read(m.join(manifest::FILE))
        .ok()
        .and_then(|b| manifest::parse(&b, manifest::FILE).ok())
        .map(|x| x.module.value.to_string())
        .unwrap_or_else(|| m.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// `written`, written in a file of the directory `dir` (absolute, folded), whose module's root is `m`.
/// A path that stays in the module's own files does not need its tree: a module whose tree cannot be
/// read (a dependency that is not there) still reads its own files, and only a path that goes through
/// a dependency says why the tree cannot be read.
fn resolve(dir: &Path, m: &Path, written: &str) -> Result<PathBuf, Refused> {
    if written.is_empty() {
        return Err(Refused::Empty);
    }
    if paths::is_absolute(written) {
        return Err(Refused::Absolute(written.to_string()));
    }
    // the real module the chain starts from, and the chain to the module the file is in
    let (head, chain): (PathBuf, Vec<String>) = match seen(m) {
        Some(s) if s.rest.is_empty() => (s.head, s.chain.iter().map(|c| c.to_string_lossy().trim_start_matches('@').to_string()).collect()),
        // a module below a dependency's files, with a `ritsu.mod` of its own: read as itself
        Some(_) => (map(m).unwrap_or_else(|| m.to_path_buf()), Vec::new()),
        None => (m.to_path_buf(), Vec::new()),
    };
    // the parts from the module's root: the file's directory, unless the path starts with `@`; each
    // with whether it is an `@<name>` at the front, which `..` does not take away
    let mut parts: Vec<(String, bool)> = Vec::new();
    let mut out = false;
    if !written.starts_with('@') {
        match dir.strip_prefix(m) {
            Ok(rel) => parts.extend(rel.components().map(|c| (c.as_os_str().to_string_lossy().into_owned(), false))),
            Err(_) => out = true,
        }
    }
    for c in written.split('/') {
        if out {
            break;
        }
        match c {
            "" | "." => {}
            ".." => match parts.last() {
                Some((_, false)) => {
                    parts.pop();
                }
                _ => out = true,
            },
            _ => {
                let boundary = c.starts_with('@') && parts.iter().all(|(_, b)| *b);
                parts.push((c.to_string(), boundary));
            }
        }
    }
    let through = !chain.is_empty() || parts.first().is_some_and(|(_, b)| *b);
    if !through {
        // the module's own file
        if out {
            return Err(Refused::Outside { written: written.to_string(), module: module_name(&head) });
        }
        let rest: Vec<&str> = parts.iter().map(|(p, _)| p.as_str()).collect();
        nested(&head, "", &rest, written)?;
        let mut target = head;
        for r in rest {
            target.push(r);
        }
        return Ok(target);
    }
    let tree = tree(&head);
    let t = match tree.as_ref() {
        Ok(t) => t,
        Err(p) => return Err(Refused::Tree(p.text())),
    };
    let chain_refs: Vec<&str> = chain.iter().map(String::as_str).collect();
    let Some(mut node) = t.walk(&chain_refs) else {
        return Err(Refused::Tree(tr!("依存の連なり `{}` をたどれません", "the chain `{}` of dependencies does not lead anywhere", chain.join("/"); chain.join("/"))));
    };
    if out {
        return Err(Refused::Outside { written: written.to_string(), module: t.nodes[node].name.to_string() });
    }
    // into the dependencies by the leading `@<name>`s
    let mut k = 0;
    while let Some((p, true)) = parts.get(k) {
        let name = &p[1..];
        match t.dep(node, name) {
            Some(d) => node = d,
            None => {
                return Err(Refused::NotRequired { written: written.to_string(), name: name.to_string(), module: t.nodes[node].name.to_string() });
            }
        }
        k += 1;
    }
    let rest: Vec<&str> = parts[k..].iter().map(|(p, _)| p.as_str()).collect();
    nested(&t.nodes[node].dir, &t.nodes[node].prefix, &rest, written)?;
    let mut target = head;
    if !t.nodes[node].prefix.is_empty() {
        target.push(&t.nodes[node].prefix);
    }
    for r in rest {
        target.push(r);
    }
    Ok(target)
}

/// A module below, with a `ritsu.mod` of its own, that the parts `rest` enter without an `@`, from
/// the directory `real` of a module whose files are written after `prefix`.
fn nested(real: &Path, prefix: &str, rest: &[&str], written: &str) -> Result<(), Refused> {
    for n in 1..=rest.len() {
        let below = rest[..n].join("/");
        if raw_file(&real.join(&below).join(manifest::FILE)) {
            let nested = if prefix.is_empty() { below } else { format!("{prefix}/{below}") };
            return Err(Refused::Nested { written: written.to_string(), nested });
        }
    }
    Ok(())
}

/// The module of the dependency the file `p` is in, as a page writes it beside the file's path
/// (DESIGN 17.8): `module example.com/acme/pricing 1.1.0`, the words the head of a file generated
/// from it names the module with. A page gives no link to such a file, which is not beside the page.
/// None for a path in no dependency, and when the module's tree cannot be read.
pub fn module_words(p: &Path) -> Option<Text> {
    if !has_at(p) {
        return None;
    }
    let s = seen(&graph::fold(&paths::absolute(p)))?;
    let t = s.tree.as_ref().as_ref().ok()?;
    let n = &t.nodes[s.node];
    let of = match n.version() {
        Some(v) => format!("{} {v}", n.name),
        None => n.name.to_string(),
    };
    Some(tr!("モジュール {of}", "module {of}"))
}

/// [`module_words`] for the file `to` that the page of the file `page` names, when `to` is not of the
/// page's own module: a dependency's file, which a page names by its path as it is written and this,
/// with no link (DESIGN 17.8). None for a file of the page's module, and outside every module.
pub fn module_words_from(page: &Path, to: &Path) -> Option<Text> {
    if dependency_root(to) == dependency_root(page) {
        return None;
    }
    module_words(to)
}

/// What a language says when a file in a dependency is not there: the dependency, its module and its
/// version (or its path), and the file. None for a path that is not in a dependency.
pub fn missing(p: &Path) -> Option<Text> {
    if !has_at(p) {
        return None;
    }
    let s = seen(&graph::fold(&paths::absolute(p)))?;
    let t = s.tree.as_ref().as_ref().ok()?;
    let n = &t.nodes[s.node];
    let rest = s.rest.iter().map(|r| r.to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    let (prefix, name) = (&n.prefix, n.name.to_string());
    let from = match (n.version(), &n.path) {
        (Some(v), None) => format!("`{name}` {v}"),
        (_, Some(path)) => format!("`{name}`、`{path}`"),
        (None, None) => format!("`{name}`"),
    };
    let from_en = from.replace('、', ", ");
    Some(tr!("依存 `{prefix}`（{from}）に `{rest}` はありません", "the dependency `{prefix}` ({from_en}) has no `{rest}`"))
}
