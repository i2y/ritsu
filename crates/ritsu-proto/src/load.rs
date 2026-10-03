//! Many files: where an import is looked for, the files known without being read, and the names
//! of types across files, resolved by protobuf's rule — from the innermost scope outward, one
//! level of the package at a time, a name that starts with `.` as it is — over the file and the
//! files it imports (`import public` passing on what it imports).

use crate::model::*;
use crate::read::{ReadError, read};
use ritsu_base::paths;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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
        let src = std::fs::read_to_string(paths::on_disk(root, f)).unwrap_or_default();
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
    for f in read_files {
        let pf = ps.files[&f].clone();
        let mut at = Vec::new();
        for imp in &pf.imports {
            if is_known(&imp.path) {
                at.push(None);
                continue;
            }
            let tried = import_candidates(&f, &pf.package, roots, &imp.path);
            match tried.iter().find(|c| paths::on_disk(root, c).is_file()) {
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
    let src = std::fs::read_to_string(paths::on_disk(root, entry)).unwrap_or_default();
    follow(root, entry, &src, roots, embedded, &mut ps, &mut issues)?;
    Ok((ps, issues))
}

fn follow(root: &Path, file: &str, src: &str, roots: &[String], embedded: &[(&str, &str)], ps: &mut Protos, issues: &mut Vec<Issue>) -> Result<(), (String, ReadError)> {
    let pf = read(file, src).map_err(|e| (file.to_string(), e))?;
    let imports = pf.imports.clone();
    let package = pf.package.clone();
    ps.add(pf);
    let mut at = Vec::new();
    for imp in &imports {
        if let Some((path, text)) = embedded.iter().find(|(p, _)| *p == imp.path) {
            if !ps.files.contains_key(*path) {
                follow(root, path, text, roots, embedded, ps, issues)?;
            }
            at.push(Some(path.to_string()));
            continue;
        }
        if is_known(&imp.path) {
            at.push(None);
            continue;
        }
        let tried = import_candidates(file, &package, roots, &imp.path);
        let found = tried.iter().find_map(|c| std::fs::read_to_string(paths::on_disk(root, c)).ok().map(|s| (c.clone(), s)));
        match found {
            Some((c, text)) => {
                if !ps.files.contains_key(&c) {
                    follow(root, &c, &text, roots, embedded, ps, issues)?;
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
