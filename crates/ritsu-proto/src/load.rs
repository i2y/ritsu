//! Many files: where an import is looked for, the files known without being read, and the names
//! of types across files, resolved by protobuf's rule — from the innermost scope outward, one
//! level of the package at a time, a name that starts with `.` as it is — over the file and the
//! files it imports (`import public` passing on what it imports).
//!
//! In a module of ritsu's (ritsu's DESIGN 17.6), an import that the file's own module does not have
//! is looked for in the module's dependencies, as protoc looks in its `-I` roots: each dependency's
//! root and the roots its `.proto` files' packages make, in the order the dependencies are written
//! the canonical way, the first that has it read ([`dependency_roots`]). One run of protoc reads
//! an import from one place for every file, which [`protoc_roots`] holds a set of files to.

use crate::model::*;
use crate::read::{ReadError, read};
use ritsu_base::paths;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};

/// Google's well-known types, `buf/validate` and dandori's options: imported by many a
/// `.proto`, distributed elsewhere, and known without their files.
pub fn is_known(import: &str) -> bool {
    import.starts_with("google/protobuf/") || import.starts_with("buf/validate/") || import == DANDORI_OPTIONS
}

/// How a `.proto` imports dandori's options (`dandori/v1/options.proto` of dandori's repository).
pub const DANDORI_OPTIONS: &str = "dandori/v1/options.proto";

/// The packages of the known files: a type in one of them is known though not read.
pub fn known_package(full: &str) -> bool {
    full.starts_with("google.protobuf.") || full.starts_with("buf.validate.") || full.starts_with("dandori.v1.")
}

/// Google's well-known types, which a `.proto` imports from `google/protobuf/…`.
pub const WELL_KNOWN: &[&str] = &[
    "google.protobuf.Timestamp",
    "google.protobuf.Duration",
    "google.protobuf.Struct",
    "google.protobuf.Value",
    "google.protobuf.ListValue",
    "google.protobuf.Any",
    "google.protobuf.Empty",
    "google.protobuf.FieldMask",
    "google.protobuf.DoubleValue",
    "google.protobuf.FloatValue",
    "google.protobuf.Int64Value",
    "google.protobuf.UInt64Value",
    "google.protobuf.Int32Value",
    "google.protobuf.UInt32Value",
    "google.protobuf.BoolValue",
    "google.protobuf.StringValue",
    "google.protobuf.BytesValue",
    "google.protobuf.NullValue",
];

/// Where an import is looked for, as paths from the root, in order: under each of `roots`; then,
/// as buf lays a module out, from the root of the module when the importing file sits where its
/// package says (`shop/v1/order.proto` for `package shop.v1`); last from the importing file's own
/// directory.
pub fn import_candidates(importer: &str, package: &str, roots: &[String], import: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |p: Result<String, paths::PathError>| {
        if let Ok(p) = p
            && !out.contains(&p)
        {
            out.push(p);
        }
    };
    for r in roots {
        add(paths::join(r, import));
    }
    let dir = paths::parent(importer);
    if !package.is_empty() {
        let at = package.replace('.', "/");
        if dir == at {
            add(paths::join(".", import));
        } else if let Some(root) = dir.strip_suffix(&format!("/{at}")) {
            add(paths::join(root, import));
        }
    }
    add(paths::join(&dir, import));
    out
}

/// The roots the imports of `file` (a path from `root`) are looked for under: `roots`, or, for a file
/// in a dependency of a module of ritsu's (`@pricing/…`, `@pricing/@tax/…` from the root), the
/// dependency's root, as the dependency itself reads them (ritsu's DESIGN 17.6). The roots of the
/// module that uses it are not the dependency's.
pub fn roots_for(root: &Path, file: &str, roots: &[String]) -> Vec<String> {
    match ritsu_base::modules::prefix_of(file) {
        Some(dep) if ritsu_base::modules::under_dependency(&paths::on_disk(root, file)) => vec![dep],
        _ => roots.to_vec(),
    }
}

/// Whether `candidate` (a path from `root`) may be read for an import written `import`: in a module
/// of ritsu's, an import is not written with `@`, and is not read from a dependency by it, since
/// protoc does not see the dependencies (ritsu's DESIGN 17.6).
fn may_read(root: &Path, candidate: &str, import: &str) -> bool {
    !(import.starts_with('@') && ritsu_base::modules::under_dependency(&paths::on_disk(root, candidate)))
}

/// The roots an import of a `.proto` of the module the directory `dir` is in is looked for under
/// after the module's own (ritsu's DESIGN 17.6), as the view writes them (absolute): for each
/// dependency of the module, in the canonical order (the fewest `@` first, then the order of the
/// `require`s, breadth first), its root (`R/@lib`), then the root each of its `.proto` files' package
/// makes, as buf lays a module out (`R/@lib/proto` for `R/@lib/proto/acme/pricing/v1/money.proto`
/// of `package acme.pricing.v1`). A file of a dependency's has its module's dependencies, not the
/// module at hand's. Empty outside a module, and when the module's tree cannot be read (M14).
pub fn dependency_roots(dir: &Path) -> Vec<PathBuf> {
    use ritsu_base::modules;
    let Some(own) = modules::module_of(dir) else { return Vec::new() };
    // the real module the chain starts from, and the chain of names to the module `dir` is in
    let (head, chain): (PathBuf, Vec<String>) = match modules::refer::view_parts(&own) {
        Some((_, head, after)) => (head, after.components().filter_map(|c| if let Component::Normal(s) = c { s.to_str().map(|s| s.trim_start_matches('@').to_string()) } else { None }).collect()),
        None => (own.clone(), Vec::new()),
    };
    let tree = modules::tree(&head);
    let Ok(t) = tree.as_ref() else { return Vec::new() };
    let names: Vec<&str> = chain.iter().map(String::as_str).collect();
    let Some(node) = t.walk(&names) else { return Vec::new() };
    let mut order: Vec<usize> = Vec::new();
    let mut queue: VecDeque<usize> = t.nodes[node].deps.iter().map(|(_, d)| *d).collect();
    let mut seen: BTreeSet<usize> = BTreeSet::from([node]);
    while let Some(d) = queue.pop_front() {
        if !seen.insert(d) {
            continue;
        }
        order.push(d);
        queue.extend(t.nodes[d].deps.iter().map(|(_, x)| *x));
    }
    let mut out: Vec<PathBuf> = Vec::new();
    let add = |p: PathBuf, out: &mut Vec<PathBuf>| {
        if !out.contains(&p) {
            out.push(p);
        }
    };
    for d in order {
        let n = &t.nodes[d];
        add(head.join(&n.prefix), &mut out);
        let except: Vec<String> = n.manifest.excludes.iter().filter_map(|e| paths::join(&n.prefix, &e.value).ok()).collect();
        let (mut under, mut nested) = (Vec::new(), Vec::new());
        paths::walk_noting(&head, &n.prefix, &except, &mut under, &mut nested);
        for f in under.iter().filter(|f| f.ends_with(".proto")) {
            let Ok(src) = ritsu_base::fs::read_to_string(paths::on_disk(&head, f)) else { continue };
            let Ok(pf) = read(f, &src) else { continue };
            if let Some(r) = package_root(f, &pf.package) {
                add(paths::on_disk(&head, &r), &mut out);
            }
        }
    }
    out
}

/// The root a file (a path from a root) sits under where its package says, as buf lays a module out:
/// `proto` for `proto/acme/pricing/v1/money.proto` of `package acme.pricing.v1`, `.` for one right
/// under it. None for a file of no package, and for one elsewhere.
fn package_root(file: &str, package: &str) -> Option<String> {
    if package.is_empty() {
        return None;
    }
    let dir = paths::parent(file);
    let at = package.replace('.', "/");
    if dir == at {
        Some(".".to_string())
    } else {
        dir.strip_suffix(&format!("/{at}")).map(str::to_string)
    }
}

/// Where the imports of a set of files are looked for: [`import_candidates`] of each file's own
/// module, then the roots of its module's dependencies ([`dependency_roots`]), each module's kept
/// once it is known.
#[derive(Default)]
struct Search {
    deps: BTreeMap<PathBuf, Vec<String>>,
}

impl Search {
    /// The candidates of `import`, written in `importer` (a path from `root`), in order.
    fn candidates(&mut self, root: &Path, importer: &str, package: &str, roots: &[String], import: &str) -> Vec<String> {
        let mut out = import_candidates(importer, package, &roots_for(root, importer, roots), import);
        // an import written with `@` is not read from a dependency (ritsu's DESIGN 17.6)
        if import.starts_with('@') {
            return out;
        }
        for r in self.dependency_roots(root, importer) {
            if let Ok(c) = paths::join(&r, import)
                && !out.contains(&c)
            {
                out.push(c);
            }
        }
        out
    }

    /// [`dependency_roots`] of the module `importer` is in, as paths from `root`.
    fn dependency_roots(&mut self, root: &Path, importer: &str) -> Vec<String> {
        let dir = paths::on_disk(root, &paths::parent(importer));
        let Some(own) = ritsu_base::modules::module_of(&dir) else { return Vec::new() };
        self.deps.entry(own).or_insert_with(|| dependency_roots(&dir).iter().filter_map(|p| paths::from_root(root, p)).collect()).clone()
    }
}

/// A message or an enum: its full name, the file that declares it, and its name from the
/// package. Two files may declare the same full name (two copies of a contract); a name is
/// resolved to the one the file that names it can see.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol {
    pub full: String,
    pub file: String,
    pub name: String,
    pub is_enum: bool,
}

/// What a type name comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// A message or an enum of a file read.
    Found(Symbol),
    /// A type of a file known without being read (`google.protobuf.Value`).
    Known(String),
    /// Not found, and an import of the file was not read: it may be in that file.
    Unknown,
    /// Not found anywhere it could be.
    Missing,
}

/// The files read, what each import came to, and every message and enum by its full name.
#[derive(Clone, Debug, Default)]
pub struct Protos {
    pub files: BTreeMap<String, ProtoFile>,
    /// The files in the order they were read.
    pub order: Vec<String>,
    /// For each file, what each of its imports came to: a path from the root, or None (known,
    /// not found, or not to be read).
    pub imports: BTreeMap<String, Vec<Option<String>>>,
    /// The files with an import that was not read.
    pub unread: BTreeSet<String>,
    pub symbols: BTreeMap<String, Vec<Symbol>>,
}

impl Protos {
    pub fn add(&mut self, f: ProtoFile) {
        for m in &f.messages {
            let full = f.full(&m.name);
            self.symbols.entry(full.clone()).or_default().push(Symbol { full, file: f.path.clone(), name: m.name.clone(), is_enum: false });
        }
        for e in &f.enums {
            let full = f.full(&e.name);
            self.symbols.entry(full.clone()).or_default().push(Symbol { full, file: f.path.clone(), name: e.name.clone(), is_enum: true });
        }
        if !self.order.contains(&f.path) {
            self.order.push(f.path.clone());
        }
        self.files.insert(f.path.clone(), f);
    }

    /// The files whose types `file` can name: itself, what it imports, and what those pass on
    /// with `import public`, through every level.
    pub fn visible(&self, file: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        out.insert(file.to_string());
        for p in self.imports.get(file).into_iter().flatten().flatten() {
            self.with_public(p, &mut out);
        }
        out
    }

    fn with_public(&self, file: &str, out: &mut BTreeSet<String>) {
        if !out.insert(file.to_string()) {
            return;
        }
        let Some(f) = self.files.get(file) else { return };
        for (imp, at) in f.imports.iter().zip(self.imports.get(file).into_iter().flatten()) {
            if imp.public
                && let Some(p) = at
            {
                self.with_public(p, out);
            }
        }
    }

    /// A type name written in `scope` (the full name of the message it is in, or the package) of
    /// `file`.
    pub fn resolve(&self, file: &str, scope: &str, written: &str) -> Resolved {
        let vis = self.visible(file);
        let found = |full: &str| self.symbols.get(full).and_then(|ss| ss.iter().find(|s| vis.contains(&s.file))).cloned();
        if let Some(abs) = written.strip_prefix('.') {
            if let Some(f) = found(abs) {
                return Resolved::Found(f);
            }
            if known_package(abs) {
                return Resolved::Known(abs.to_string());
            }
        } else {
            let mut s = scope.to_string();
            loop {
                let full = if s.is_empty() { written.to_string() } else { format!("{s}.{written}") };
                if let Some(f) = found(&full) {
                    return Resolved::Found(f);
                }
                if s.is_empty() {
                    break;
                }
                s = s.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default();
            }
            if known_package(written) {
                return Resolved::Known(written.to_string());
            }
        }
        if self.unread.contains(file) { Resolved::Unknown } else { Resolved::Missing }
    }

    /// Every type a field of a message names, resolved: (field, what it comes to).
    pub fn field_types(&self, file: &str, msg: &Message) -> Vec<(String, Resolved)> {
        let f = &self.files[file];
        let scope = f.full(&msg.name);
        let mut out = Vec::new();
        for fl in &msg.fields {
            let named = match &fl.ty {
                Type::Named(n) => Some(n),
                Type::Map(_, v) => match v.as_ref() {
                    Type::Named(n) => Some(n),
                    _ => None,
                },
                Type::Scalar(_) => None,
            };
            if let Some(n) = named {
                out.push((fl.name.clone(), self.resolve(file, &scope, n)));
            }
        }
        out
    }

    /// Whether `field` of a message in `file` is marked to be redacted (DESIGN 16.6): written
    /// `[debug_redact = true]`, or given a custom option whose value is an enum value marked so.
    /// A custom option is looked up as an extension of `google.protobuf.FieldOptions` among the
    /// files `file` sees (itself, what it imports, what those import publicly), by protobuf's
    /// rule for names from the scope of the field's message outward; its enum type the same way
    /// from where the extension is declared. `debug_redact = false`, an option that is not found,
    /// and one whose value is no enum value marked so, mark nothing.
    pub fn redaction(&self, file: &str, field: &Field) -> Option<Redaction> {
        if field.options.iter().any(|o| o.name.trim() == "debug_redact" && is_true(o)) {
            return Some(Redaction::Direct { line: field.line });
        }
        let f = self.files.get(file)?;
        let msg = f.messages.iter().find(|m| m.fields.iter().any(|x| x == field))?;
        let scope = f.full(&msg.name);
        for o in &field.options {
            let written = o.name.trim();
            let Some(ext) = written.strip_prefix('(').and_then(|x| x.strip_suffix(')')) else { continue };
            let Some(crate::value::Value::Id(value)) = &o.value else { continue };
            let Some((xfile, x)) = self.field_option(file, &scope, ext.trim()) else { continue };
            let Type::Named(ty) = &x.ty else { continue };
            let xf = &self.files[&xfile];
            let xscope = match x.name.rsplit_once('.') {
                Some((outer, _)) => xf.full(outer),
                None => xf.package.clone(),
            };
            let Resolved::Found(sym) = self.resolve(&xfile, &xscope, ty) else { continue };
            if !sym.is_enum {
                continue;
            }
            let Some(e) = self.files.get(&sym.file).and_then(|ef| ef.enumeration(&sym.name)) else { continue };
            if let Some(v) = e.values.iter().find(|v| &v.name == value)
                && v.options.iter().any(|vo| vo.name.trim() == "debug_redact" && is_true(vo))
            {
                return Some(Redaction::ByOption { option: written.to_string(), value: value.clone(), file: sym.file.clone(), line: v.line });
            }
        }
        None
    }

    /// The extension of `google.protobuf.FieldOptions` that `written` (an option's name inside
    /// its parentheses) comes to from `scope` of `file`, and the file that declares it.
    fn field_option(&self, file: &str, scope: &str, written: &str) -> Option<(String, Extension)> {
        let vis = self.visible(file);
        let found = |full: &str| {
            vis.iter().find_map(|vf| {
                let pf = self.files.get(vf)?;
                pf.extensions
                    .iter()
                    .find(|x| x.extendee.trim_start_matches('.') == "google.protobuf.FieldOptions" && pf.full(&x.name) == full)
                    .map(|x| (vf.clone(), x.clone()))
            })
        };
        if let Some(abs) = written.strip_prefix('.') {
            return found(abs);
        }
        let mut s = scope.to_string();
        loop {
            let full = if s.is_empty() { written.to_string() } else { format!("{s}.{written}") };
            if let Some(x) = found(&full) {
                return Some(x);
            }
            if s.is_empty() {
                return None;
            }
            s = s.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default();
        }
    }

    /// Every message and enum `start` reaches through the types of fields, `start` among them,
    /// in the order they are reached.
    pub fn reach(&self, start: &[Symbol]) -> Vec<Symbol> {
        let mut out: Vec<Symbol> = Vec::new();
        let mut todo: Vec<Symbol> = start.to_vec();
        todo.reverse();
        while let Some(sym) = todo.pop() {
            if out.contains(&sym) {
                continue;
            }
            out.push(sym.clone());
            if sym.is_enum {
                continue;
            }
            let Some(m) = self.files.get(&sym.file).and_then(|f| f.message(&sym.name)) else { continue };
            let mut next: Vec<Symbol> = Vec::new();
            for (_, r) in self.field_types(&sym.file, m) {
                if let Resolved::Found(n) = r
                    && !out.contains(&n)
                {
                    next.push(n);
                }
            }
            next.reverse();
            todo.extend(next);
        }
        out
    }
}

/// An option set to `true` (`debug_redact = true`).
fn is_true(o: &Opt) -> bool {
    matches!(&o.value, Some(crate::value::Value::Id(v)) if v == "true" || v == "True")
}

/// What reading a set of `.proto` files came across.
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// The file cannot be read.
    Unreadable { file: String, err: ReadError },
    /// An import is on the disk in none of the places looked in.
    NotFound { file: String, import: Import, tried: Vec<String> },
    /// An import is a file outside the files to read (`load`'s `files`).
    OutOfScope { file: String, import: Import, at: String },
}

/// Read the `.proto` files `files` (paths from the root) and what each import comes to, looking
/// under `roots` first. A known file ([`is_known`]) is not read; an import that is not found,
/// or is outside `files`, is told, and its types are not known.
pub fn load(root: &Path, files: &[String], roots: &[String]) -> (Protos, Vec<Issue>) {
    let mut ps = Protos::default();
    let mut issues = Vec::new();
    let mut unreadable = BTreeSet::new();
    for f in files {
        let src = ritsu_base::fs::read_to_string(paths::on_disk(root, f)).unwrap_or_default();
        match read(f, &src) {
            Ok(pf) => ps.add(pf),
            Err(err) => {
                unreadable.insert(f.clone());
                issues.push(Issue::Unreadable { file: f.clone(), err });
            }
        }
    }
    let in_scope: BTreeSet<&String> = files.iter().collect();
    let read_files: Vec<String> = ps.files.keys().cloned().collect();
    let mut search = Search::default();
    for f in read_files {
        let pf = ps.files[&f].clone();
        let mut at = Vec::new();
        for imp in &pf.imports {
            if is_known(&imp.path) {
                at.push(None);
                continue;
            }
            let tried = search.candidates(root, &f, &pf.package, roots, &imp.path);
            match tried.iter().find(|c| may_read(root, c, &imp.path) && ritsu_base::fs::is_file(paths::on_disk(root, c))) {
                Some(c) if in_scope.contains(c) && !unreadable.contains(c) => at.push(Some(c.clone())),
                Some(c) if in_scope.contains(c) => {
                    ps.unread.insert(f.clone());
                    at.push(None);
                }
                Some(c) => {
                    ps.unread.insert(f.clone());
                    issues.push(Issue::OutOfScope { file: f.clone(), import: imp.clone(), at: c.clone() });
                    at.push(None);
                }
                None => {
                    ps.unread.insert(f.clone());
                    issues.push(Issue::NotFound { file: f.clone(), import: imp.clone(), tried });
                    at.push(None);
                }
            }
        }
        ps.imports.insert(f, at);
    }
    (ps, issues)
}

/// Read `entry` (a path from the root) and every file it imports, through every level, each
/// file before the files it imports and those in the order written (dandori's way). An import
/// is read from the first of its candidates on the disk ([`import_candidates`]); `embedded` are
/// files read from the text given rather than from the disk (dandori's options, which dandori
/// carries), by the path an import writes; Google's well-known types and `buf/validate` are
/// known and not read. An import found nowhere is passed over and told in [`Issue::NotFound`].
/// A file that is there and does not read stops it: the file, and why.
pub fn load_from(root: &Path, entry: &str, roots: &[String], embedded: &[(&str, &str)]) -> Result<(Protos, Vec<Issue>), (String, ReadError)> {
    let mut ps = Protos::default();
    let mut issues = Vec::new();
    let src = ritsu_base::fs::read_to_string(paths::on_disk(root, entry)).unwrap_or_default();
    follow(root, entry, &src, roots, embedded, &mut ps, &mut issues, &mut Search::default())?;
    Ok((ps, issues))
}

#[allow(clippy::too_many_arguments)]
fn follow(root: &Path, file: &str, src: &str, roots: &[String], embedded: &[(&str, &str)], ps: &mut Protos, issues: &mut Vec<Issue>, search: &mut Search) -> Result<(), (String, ReadError)> {
    let pf = read(file, src).map_err(|e| (file.to_string(), e))?;
    let imports = pf.imports.clone();
    let package = pf.package.clone();
    ps.add(pf);
    let mut at = Vec::new();
    for imp in &imports {
        if let Some((path, text)) = embedded.iter().find(|(p, _)| *p == imp.path) {
            if !ps.files.contains_key(*path) {
                follow(root, path, text, roots, embedded, ps, issues, search)?;
            }
            at.push(Some(path.to_string()));
            continue;
        }
        if is_known(&imp.path) {
            at.push(None);
            continue;
        }
        let tried = search.candidates(root, file, &package, roots, &imp.path);
        let found = tried.iter().filter(|c| may_read(root, c, &imp.path)).find_map(|c| ritsu_base::fs::read_to_string(paths::on_disk(root, c)).ok().map(|s| (c.clone(), s)));
        match found {
            Some((c, text)) => {
                if !ps.files.contains_key(&c) {
                    follow(root, &c, &text, roots, embedded, ps, issues, search)?;
                }
                at.push(Some(c));
            }
            None => {
                ps.unread.insert(file.to_string());
                issues.push(Issue::NotFound { file: file.to_string(), import: imp.clone(), tried });
                at.push(None);
            }
        }
    }
    ps.imports.insert(file.to_string(), at);
    Ok(())
}

/// What one run of protoc is given for a `.proto` and the files it imports, through every level
/// ([`protoc_roots`]): the `-I` roots, in order, each a directory on the disk, and the name the file
/// is given by, from the first root it is under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Protoc {
    pub includes: Vec<PathBuf>,
    pub entry: String,
}

/// Why one run of protoc cannot read a set of files as each of them reads its imports (ritsu's
/// DESIGN 17.6). The paths are from the root.
#[derive(Clone, Debug, PartialEq)]
pub enum Clash {
    /// One import names two files whose text differs: protoc reads the first, `read`, for every file
    /// that writes it, and the file `by` reads the second, `meant`.
    TwoFiles { import: String, read: String, meant: String, by: String },
    /// One file is reached by two names: protoc reads it twice, and what it declares is declared
    /// twice.
    TwoNames { file: String, names: [String; 2] },
    /// A file does not read as a `.proto`.
    Unreadable { file: String, err: ReadError },
}

/// What one run of protoc is given for `entry` (a path from `root`) and every file it imports, read
/// as [`load_from`] reads them: `roots` (for a file of a dependency's, the dependency's root), then
/// the root each import was found under (a package's root, a dependency's root, a file's directory),
/// in the order they are first used, each a directory on the disk ([`ritsu_base::modules::map`] makes
/// one seen through a dependency the directory its files are read from). protoc reads an import from
/// the first root that has it, for every file at once: Err when that is not the file a file reads
/// it as and the two differ in their text, or when a file is reached by two names (ritsu's DESIGN
/// 17.6). The known files (Google's, `buf/validate`, dandori's options) are protoc's own concern.
pub fn protoc_roots(root: &Path, entry: &str, roots: &[String]) -> Result<Protoc, Clash> {
    let (ps, _) = load_from(root, entry, roots, &[]).map_err(|(file, err)| Clash::Unreadable { file, err })?;
    let own = roots_for(root, entry, roots);
    // the root the entry is named from: the first of its roots it is under, else its directory
    let mut entry_roots: Vec<String> = own.clone();
    if let Some(r) = ps.files.get(entry).and_then(|f| package_root(entry, &f.package)) {
        entry_roots.insert(0, r);
    }
    entry_roots.push(paths::parent(entry));
    let under = |r: &str, f: &str| -> Option<String> { if r == "." { Some(f.to_string()) } else { f.strip_prefix(&format!("{r}/")).map(str::to_string) } };
    let (entry_root, entry_name) = entry_roots.iter().find_map(|r| under(r, entry).map(|n| (r.clone(), n))).unwrap_or_else(|| (".".to_string(), entry.to_string()));
    let mut includes: Vec<String> = vec![entry_root];
    for r in own {
        if !includes.contains(&r) {
            includes.push(r);
        }
    }
    // each import read, with the file that reads it and the file it reads
    let mut reads: Vec<(String, String, String)> = Vec::new();
    for f in &ps.order {
        let Some(pf) = ps.files.get(f) else { continue };
        for (imp, at) in pf.imports.iter().zip(ps.imports.get(f).into_iter().flatten()) {
            let Some(x) = at else { continue };
            let r = if *x == imp.path { ".".to_string() } else { x.strip_suffix(&format!("/{}", imp.path)).unwrap_or(".").to_string() };
            if !includes.contains(&r) {
                includes.push(r);
            }
            reads.push((f.clone(), imp.path.clone(), x.clone()));
        }
    }
    let mut names: BTreeMap<String, String> = BTreeMap::from([(entry.to_string(), entry_name.clone())]);
    let text = |f: &str| ritsu_base::fs::read_to_string(paths::on_disk(root, f)).ok();
    for (by, imp, x) in &reads {
        let read = includes.iter().filter_map(|r| paths::join(r, imp).ok()).find(|c| ritsu_base::fs::is_file(paths::on_disk(root, c)));
        // the file protoc reads: the one meant, or one of the same text
        let file = match read {
            Some(r) if r == *x || text(&r).is_some_and(|t| Some(t) == text(x)) => r,
            r => return Err(Clash::TwoFiles { import: imp.clone(), read: r.unwrap_or_default(), meant: x.clone(), by: by.clone() }),
        };
        match names.get(&file) {
            Some(n) if n != imp => return Err(Clash::TwoNames { file, names: [n.clone(), imp.clone()] }),
            Some(_) => {}
            None => {
                names.insert(file, imp.clone());
            }
        }
    }
    let includes = includes
        .iter()
        .map(|r| {
            let p = paths::on_disk(root, r);
            ritsu_base::modules::map(&p).unwrap_or(p)
        })
        .collect();
    Ok(Protoc { includes, entry: entry_name })
}
