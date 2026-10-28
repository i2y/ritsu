//! `.importlinter` for Python (DESIGN 7.2; PLAN C.7): one `protected` contract a group. Its
//! `protected_modules` are the group's modules and its `allowed_importers` those of the groups
//! that may import it (DESIGN 7.1).
//!
//! A contract treats its modules as packages, so a module stands for everything under it. When a
//! group's directory holds another group (a layer under the inside, `delivery/acl/inventory`
//! under `delivery`), the group is the largest modules that hold no other group. When a package
//! that holds another group has an `__init__.py` of its own, that module cannot be named as a
//! package without the group under it; the contract then names every module, one by one
//! (`as_packages = False`), so that it says exactly what the map says.
//!
//! What the map's `except` leaves out inside a root package, grimp reads with the rest of the
//! package. Every contract ignores the imports from it and to it (`ignore_imports`, the other side
//! `**`), as dependency-cruiser's `exclude` and go-arch-lint's leave it out of what they read, and
//! says nothing when there are none (`unmatched_ignore_imports_alerting = none`). An `except`
//! outside every root package is not read at all. grimp 3.17 goes into no directory whose name is
//! not an identifier, and reads no hidden `.py` file and none with a second dot in its name; it
//! reads any other `.py` file, `old-stuff.py` too, but its expressions take names of letters,
//! digits and `_` only, so an `except` of such a file cannot be written (E501, [`unwritable`]).

use super::areas::{self, Areas};
use crate::check::Checked;
use crate::diag::{self, Diag};
use crate::paths;
use ritsu_base::text::Lang;
use std::collections::BTreeSet;

/// The `.py` files of the code directory, from it.
struct Tree<'a> {
    files: &'a BTreeSet<String>,
}

impl Tree<'_> {
    fn parent(p: &str) -> &str {
        p.rsplit_once('/').map(|(d, _)| d).unwrap_or(".")
    }

    fn has_init(&self, d: &str) -> bool {
        let init = if d == "." { "__init__.py".to_string() } else { format!("{d}/__init__.py") };
        self.files.contains(&init)
    }

    /// The modules of the files right in `d`, `__init__.py` aside.
    fn modules(&self, d: &str) -> Vec<String> {
        self.files.iter().filter(|f| Self::parent(f) == d && !f.ends_with("/__init__.py") && *f != "__init__.py").map(|f| module_of(f)).collect()
    }

    /// The directories right under `d` that hold a file.
    fn children(&self, d: &str) -> Vec<String> {
        let mut out: BTreeSet<String> = BTreeSet::new();
        for f in self.files {
            let rest = if d == "." { Some(f.as_str()) } else { f.strip_prefix(&format!("{d}/")) };
            if let Some(r) = rest
                && let Some((first, _)) = r.split_once('/')
            {
                out.insert(if d == "." { first.to_string() } else { format!("{d}/{first}") });
            }
        }
        out.into_iter().collect()
    }
}

/// `a/b/c.py` → `a.b.c`, `a/b/__init__.py` → `a.b`.
fn module_of(f: &str) -> String {
    let m = f.strip_suffix("/__init__.py").or_else(|| f.strip_suffix(".py")).unwrap_or(f);
    m.replace('/', ".")
}

fn dotted(d: &str) -> String {
    d.replace('/', ".")
}

/// The root packages (PLAN C.7): each directory under the code directory with an `__init__.py`
/// is one; a directory without one (a namespace package, as protoc writes) is gone down until a
/// directory has an `__init__.py` or modules of its own (`warehouse.v1`, `shop.ordering.v1`).
/// grimp 3.17 does not go into a directory without an `__init__.py` under a package (protoc's
/// `warehouse/v1` under a package `warehouse`): such a directory of modules is a root package of
/// its own, or its modules and the imports of them would not be seen at all.
fn root_packages(t: &Tree) -> Vec<String> {
    fn walk(t: &Tree, d: &str, reached: bool, out: &mut Vec<String>) {
        let reached_here = if reached && t.has_init(d) {
            true
        } else if t.has_init(d) || !t.modules(d).is_empty() {
            out.push(d.to_string());
            true
        } else {
            false
        };
        for c in t.children(d) {
            walk(t, &c, reached_here, out);
        }
    }
    let mut out = Vec::new();
    for c in t.children(".") {
        walk(t, &c, false, &mut out);
    }
    out
}

/// What a group comes to in modules: those that can stand for everything under them, and the
/// `__init__` modules of packages that hold another group.
#[derive(Default)]
struct Modules {
    packages: Vec<String>,
    inits: Vec<String>,
}

fn decompose(t: &Tree, roots: &[String], d: &str, excluded: &[String], out: &mut Modules) {
    if excluded.iter().any(|e| e == d) {
        return;
    }
    let under_root = roots.iter().any(|r| crate::paths::contains(r, d));
    let is_module = under_root && (roots.iter().any(|r| r == d) || t.has_init(d));
    let holds_other = excluded.iter().any(|e| e != d && crate::paths::contains(d, e));
    if !holds_other && is_module {
        out.packages.push(dotted(d));
        return;
    }
    if is_module && t.has_init(d) {
        out.inits.push(dotted(d));
    }
    if under_root {
        out.packages.extend(t.modules(d));
    }
    for c in t.children(d) {
        decompose(t, roots, &c, excluded, out);
    }
}

/// The modules of the groups `group` (a set), leaving out what other groups under them hold.
fn modules_of(t: &Tree, roots: &[String], a: &Areas, group: &[usize]) -> Modules {
    let mut out = Modules::default();
    for &i in group {
        for r in &a.areas[i].roots {
            let excluded: Vec<String> = a.nested(i, r).into_iter().filter(|n| !group.iter().any(|&g| a.areas[g].roots.contains(n))).collect();
            decompose(t, roots, r, &excluded, &mut out);
        }
    }
    out.packages.sort();
    out.packages.dedup();
    out.inits.sort();
    out.inits.dedup();
    out
}

/// Every module of the groups, one by one.
fn every_module(a: &Areas, group: &[usize]) -> Vec<String> {
    let mut out: Vec<String> = group.iter().flat_map(|&i| a.areas[i].files.iter().map(|f| module_of(f))).collect();
    out.sort();
    out.dedup();
    out
}

/// The modules, without those another of them holds (as packages, it stands for them).
fn outermost(ms: &[String]) -> Vec<String> {
    ms.iter().filter(|x| !ms.iter().any(|y| y != *x && x.starts_with(&format!("{y}.")))).cloned().collect()
}

fn list(key: &str, items: &[String]) -> String {
    let mut s = format!("{key} =\n");
    for i in items {
        s.push_str(&format!("    {i}\n"));
    }
    s
}

/// An `except` inside a root package that grimp reads: a directory stands for what is under it,
/// a `.py` file for its module.
struct Excepted {
    /// From the code directory.
    path: String,
    module: String,
    dir: bool,
}

/// The map's `except` that grimp reads inside a root package.
fn excepted(c: &Checked, a: &Areas, roots: &[String]) -> Vec<Excepted> {
    let mut out = Vec::new();
    for e in a.except.iter().filter(|e| roots.iter().any(|r| r != *e && paths::contains(r, e))) {
        let dir = ritsu_base::fs::is_dir(paths::on_disk(&c.model.root, &paths::join(&a.dir, e).unwrap_or_default()));
        let (dirs, file) = match e.rsplit_once('/') {
            Some((d, f)) if !dir => (d, Some(f)),
            _ => (e.as_str(), None),
        };
        if !dirs.split('/').all(areas::identifier) {
            continue;
        }
        if let Some(f) = file
            && (!f.ends_with(".py") || f.starts_with('.') || f.matches('.').count() > 1)
        {
            continue;
        }
        out.push(Excepted { path: e.clone(), module: module_of(e), dir });
    }
    out
}

/// Whether grimp's expressions can name the module: each part of letters, digits and `_`.
fn nameable(module: &str) -> bool {
    module.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

/// The imports to ignore for what is excepted, from it and to it.
fn ignored(xs: &[Excepted]) -> Vec<String> {
    let mut out = Vec::new();
    for x in xs.iter().filter(|x| nameable(&x.module)) {
        let m = &x.module;
        if x.dir {
            out.extend([format!("{m} -> **"), format!("{m}.** -> **"), format!("** -> {m}"), format!("** -> {m}.**")]);
        } else {
            out.extend([format!("{m} -> **"), format!("** -> {m}")]);
        }
    }
    out
}

/// Why the map's `except` cannot be written for import-linter (E501), at each entry that cannot.
pub fn unwritable(c: &Checked, a: &Areas) -> Vec<Diag> {
    let m = &c.model;
    let files: BTreeSet<String> = a.areas.iter().flat_map(|x| x.files.iter().cloned()).collect();
    let roots = root_packages(&Tree { files: &files });
    let mut out = Vec::new();
    for x in excepted(c, a, &roots).iter().filter(|x| !nameable(&x.module)) {
        let Some((p, s)) = m.map.except.iter().zip(&m.map.ast.except).find(|(p, _)| paths::contains(&a.dir, p) && paths::relative(&a.dir, p) == x.path) else {
            continue;
        };
        let (se, sm) = (paths::shown(p), &x.module);
        out.push(diag::at("E501", &m.map.file, s.pos.line, s.pos.col, tr!("`except` の {se} はモジュール `{sm}` として読まれますが、import-linter の設定にはその名前を書けません", "The `except` {se} is read as the module `{sm}`, a name the settings of import-linter cannot hold")).source(&m.map.src).note(tr!(
            "import-linter（grimp）は、名前に `-` や空白のある `.py` のファイルも、その名前のモジュールとして読みます。`ignore_imports` の式は文字、数字、`_` の名前しか受け付けないので、このファイルだけを外す書き方がありません。ファイルの名前を変えるか、外すファイルをディレクトリにまとめ、そのディレクトリを `except` に書いてください。",
            "import-linter (grimp) reads a `.py` file with `-` or a space in its name as a module of that name, and the expressions of `ignore_imports` take names of letters, digits and `_` only, so no setting leaves out this one file. Rename the file, or put the files to leave out in a directory, and write that directory in `except`."
        )));
    }
    out
}

pub fn render(c: &Checked, a: &Areas, lang: Lang) -> (String, usize) {
    let m = &c.model;
    let files: BTreeSet<String> = a.areas.iter().flat_map(|x| x.files.iter().cloned()).collect();
    let t = Tree { files: &files };
    let roots = root_packages(&t);
    let ignored = ignored(&excepted(c, a, &roots));
    let mut s = String::from("[importlinter]\n");
    s.push_str(&list("root_packages", &roots.iter().map(|r| dotted(r)).collect::<Vec<_>>()));
    let verb = tr!("import する", "imported");
    for (i, area) in a.areas.iter().enumerate() {
        let importers = a.importers(i);
        let protected = modules_of(&t, &roots, a, &[i]);
        let allowed = modules_of(&t, &roots, a, &importers);
        s.push_str(&format!("\n[importlinter:contract:{}]\n", areas::name(m, area)));
        s.push_str(&format!("name = {}\n", areas::rule_text(m, a, i, &verb, lang)));
        s.push_str("type = protected\n");
        // import-linter takes a protected module and what is under it as allowed to import it, and
        // no other protected module: the group's own modules are listed among the importers too.
        if protected.inits.is_empty() && allowed.inits.is_empty() {
            s.push_str(&list("protected_modules", &outermost(&protected.packages)));
            s.push_str(&list("allowed_importers", &outermost(&allowed.packages)));
        } else {
            s.push_str("as_packages = False\n");
            s.push_str(&list("protected_modules", &every_module(a, &[i])));
            s.push_str(&list("allowed_importers", &every_module(a, &importers)));
        }
        if !ignored.is_empty() {
            s.push_str(&list("ignore_imports", &ignored));
            s.push_str("unmatched_ignore_imports_alerting = none\n");
        }
    }
    (s, a.areas.len())
}
