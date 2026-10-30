//! `implements proto "<file>" <Service>` (§15.274): the Connect service a rule is served as is one a
//! contract declares, so the numbers on the wire are the contract's.
//!
//! A rule's own `.proto` numbers its fields in the order the rule declares them (§15.112), so a
//! field added in the middle moves every number after it, which `ritsu mod diff` says as E310. The
//! numbers are a contract's to keep: a `.proto` keeps them through every change, with `reserved` for
//! what is taken out and a new number for what is added. So a rule may name the service it
//! implements instead, and `rulec check` holds the two together:
//!
//! - the service is shaped as rulec's own `.proto` would be: one unary method with no side effects;
//!   a request with an `optional` field for every input and a `repeated` message for every sequence;
//!   an answer with a field for every output, `optional` exactly where the rule may answer none, and
//!   the rows that answered in `trace`;
//! - a field is named by the alias, its type is the one rulec's own `.proto` writes ([`Kind`]), and an
//!   enum is the very enum the rule reads with `import proto`;
//! - what may differ is the package, the names of the service, the method and the messages, the
//!   numbers, the order, and the file each message is declared in.
//!
//! What does not agree is E076. A file that cannot be read, a service that is not there, and a line
//! that is not this shape are E013. `gen` then copies the contract into the module in place of the
//! rule's own `.proto` and serves the rule under the contract's names ([`served`]): the numbers are
//! in the stubs protobuf-py writes from the contract, and the generated code spells none of them.

use crate::ast::{EnumSource, ImplementsDecl, Name, RuleFile};
use crate::codegen::pub_name_of as pub_name;
use crate::diag::Diag;
use crate::types::{Checked, Ty};
use ritsu_proto::{Field, Label, Message, ProtoFile, Protos, Resolved, Type};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What one value of a rule is on the wire of its Connect service: the type rulec's own `.proto`
/// gives it, and the type the check of `implements` holds a contract's field to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Bool,
    /// A date, `YYYY-MM-DD` on this wire as on every other of rulec's, or a string.
    Str,
    /// Every other number: a whole number, an amount, a rate, a quantity, in its declared unit. Only
    /// a 64-bit integer is written as a string in protobuf's JSON, which `rulec api` says
    /// (`json_int64`) and dandori reads by.
    Int64,
    /// One of the rule's enums, by its name.
    Enum(String),
}

impl Kind {
    pub(crate) fn of(ty: &Ty) -> Kind {
        match ty {
            Ty::Opt(t) => Kind::of(t),
            Ty::Enum(n) => Kind::Enum(n.clone()),
            Ty::Bool => Kind::Bool,
            Ty::Date | Ty::Str => Kind::Str,
            _ => Kind::Int64,
        }
    }

    /// The scalar a `.proto` writes for it; an enum is a name of its own.
    pub(crate) fn scalar(&self) -> Option<&'static str> {
        match self {
            Kind::Bool => Some("bool"),
            Kind::Str => Some("string"),
            Kind::Int64 => Some("int64"),
            Kind::Enum(_) => None,
        }
    }
}

/// One value of the rule's wire: an input, a field of an element, or an output.
pub(crate) struct Slot<'f> {
    pub name: &'f Name,
    pub kind: Kind,
    /// Written `optional`: every field the caller sends, so that one left out is told apart from one
    /// set to zero (§15.160), and an answer the rule may give none of (§15.249).
    pub optional: bool,
}

/// The rule's side of its Connect service, in the order rulec's own `.proto` writes it: the inputs,
/// then each sequence with the fields of one element, and the outputs.
pub(crate) struct Wire<'f> {
    pub inputs: Vec<Slot<'f>>,
    pub sequences: Vec<(&'f Name, Vec<Slot<'f>>)>,
    pub outputs: Vec<Slot<'f>>,
}

pub(crate) fn wire<'f>(f: &'f RuleFile, c: &Checked) -> Wire<'f> {
    let ty = |n: &Name| c.ty_of(&n.text).unwrap_or(Ty::Unknown);
    let sent = |n: &'f Name| Slot { name: n, kind: Kind::of(&ty(n)), optional: true };
    Wire {
        inputs: f.inputs.iter().map(|i| sent(&i.name)).collect(),
        sequences: f.elements.iter().map(|el| (&el.name, el.fields.iter().map(|fd| sent(&fd.name)).collect())).collect(),
        outputs: f.outputs.iter().map(|o| Slot { name: &o.name, kind: Kind::of(&ty(&o.name)), optional: matches!(ty(&o.name), Ty::Opt(_)) }).collect(),
    }
}

/// Where a contract sits in the generated module: its package as directories, then its file name —
/// where buf's `PACKAGE_DIRECTORY_MATCH` puts it, whatever path the rule reached it by (§15.160).
pub(crate) fn import_path(file: &str, pkg: Option<&str>) -> String {
    let base = Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| file.to_string());
    match pkg {
        Some(p) => format!("{}/{base}", p.replace('.', "/")),
        None => base,
    }
}

/// The directory a file's own imports are followed from: the root of the module it is in, when the
/// file sits where its import path says, and else the directory it is in.
pub(crate) fn module_root(disk: &Path, import: &str) -> PathBuf {
    let up = Path::new(import).components().count();
    let mut root = disk.to_path_buf();
    for _ in 0..up {
        root.pop();
    }
    if root.join(import) == disk {
        root
    } else {
        disk.parent().map(|p| p.to_path_buf()).unwrap_or_default()
    }
}

/// Where the imports of one file of a contract are found: under the root of the module the file is
/// in ([`module_root`]), and in a module of ritsu's, what that does not have under the roots of the
/// module's dependencies, in their order (ritsu's DESIGN 17.6). `check` reads a contract and `gen`
/// copies one this one way, so the files `check` held a rule to are the files `gen` copies.
pub(crate) struct Imports {
    root: PathBuf,
    dir: PathBuf,
    /// The roots of the dependencies, read once they are needed.
    deps: Option<Vec<PathBuf>>,
}

impl Imports {
    /// The imports of the file at `disk`, which the module has at `at`.
    pub(crate) fn of(disk: &Path, at: &str) -> Imports {
        Imports { root: module_root(disk, at), dir: disk.parent().map(Path::to_path_buf).unwrap_or_default(), deps: None }
    }

    /// The file an import names, or None for one on no disk: a file of a BSR module (§15.161).
    pub(crate) fn find(&mut self, imp: &str) -> Option<PathBuf> {
        let p = self.root.join(imp);
        if ritsu_base::fs::is_file(&p) {
            return Some(p);
        }
        if imp.starts_with('@') {
            return None;
        }
        let dir = &self.dir;
        self.deps.get_or_insert_with(|| ritsu_proto::dependency_roots(dir)).iter().map(|r| r.join(imp)).find(|p| ritsu_base::fs::is_file(p))
    }
}

/// One file as the copies of the contracts tell files apart: the file its path reads, through a
/// dependency (`@lib/…`) the dependency's own.
fn file_key(p: &Path) -> PathBuf {
    ritsu_base::modules::map(p).unwrap_or_else(|| ritsu_base::paths::absolute(p))
}

/// A contract read whole: every file, each named by its path in the generated module — the path its
/// importers name it by — with the file it was read from.
struct Read {
    ps: Protos,
    disk: BTreeMap<String, PathBuf>,
    /// The contract itself, by its path in the module.
    entry: String,
}

enum Unread {
    /// The contract is not on the disk.
    Missing,
    /// A file of it does not read as a `.proto`: None for the contract itself, else the import
    /// that names the file; and why.
    Bad(Option<String>, ritsu_proto::ReadError),
}

/// Read the contract at `disk` (written `written` in the rule) and every file it imports that is on a
/// disk, through every level. Google's well-known types and Protovalidate's file are known without
/// being read; a file that is on no disk is a BSR module's, and its types are not known. A file that
/// does not read is not read in part (§15.166).
fn load(disk: &Path, written: &str) -> Result<Read, Unread> {
    let text = ritsu_base::fs::read_to_string(disk).map_err(|_| Unread::Missing)?;
    let mut first = ritsu_proto::read(written, &text).map_err(|e| Unread::Bad(None, e))?;
    let entry = import_path(written, (!first.package.is_empty()).then_some(first.package.as_str()));
    first.path = entry.clone();
    let mut r = Read { ps: Protos::default(), disk: BTreeMap::new(), entry: entry.clone() };
    let mut todo: Vec<(PathBuf, String, Option<ProtoFile>)> = vec![(disk.to_path_buf(), entry, Some(first))];
    while let Some((d, at, read)) = todo.pop() {
        if r.ps.files.contains_key(&at) {
            continue;
        }
        let pf = match read {
            Some(pf) => pf,
            None => ritsu_proto::read(&at, &ritsu_base::fs::read_to_string(&d).unwrap_or_default()).map_err(|e| Unread::Bad(Some(at.clone()), e))?,
        };
        let mut imports = Imports::of(&d, &at);
        let mut came = Vec::new();
        for imp in &pf.imports {
            if ritsu_proto::is_known(&imp.path) {
                came.push(None);
                continue;
            }
            match imports.find(&imp.path) {
                Some(p) => {
                    todo.push((p, imp.path.clone(), None));
                    came.push(Some(imp.path.clone()));
                }
                None => {
                    r.ps.unread.insert(at.clone());
                    came.push(None);
                }
            }
        }
        r.ps.imports.insert(at.clone(), came);
        r.disk.insert(at.clone(), d);
        r.ps.add(pf);
    }
    Ok(r)
}

impl Read {
    fn entry(&self) -> &ProtoFile {
        &self.ps.files[&self.entry]
    }

    /// The service the line names: by its name, or by its name with the file's package.
    fn service(&self, written: &str) -> Option<&ritsu_proto::Service> {
        let pf = self.entry();
        pf.services.iter().find(|s| s.name == written || (!pf.package.is_empty() && pf.full(&s.name) == written))
    }

    /// The message a type name written in `scope` of `file` comes to, with the file it is declared in.
    fn message(&self, file: &str, scope: &str, written: &str) -> Option<(&ProtoFile, &Message)> {
        match self.ps.resolve(file, scope, written) {
            Resolved::Found(s) if !s.is_enum => {
                let pf = self.ps.files.get(&s.file)?;
                pf.message(&s.name).map(|m| (pf, m))
            }
            _ => None,
        }
    }
}

/// A message of the contract as `gen` and `rulec api` name it.
#[derive(Clone, Debug)]
pub struct Msg {
    /// From its package: `QuoteRequest`, `SettleRequest.Coupon`.
    pub name: String,
    pub package: String,
    /// The path in the module of the file that declares it.
    pub file: String,
    /// The number of each field, by its name.
    pub numbers: BTreeMap<String, i64>,
}

impl Msg {
    fn of(pf: &ProtoFile, m: &Message) -> Msg {
        Msg { name: m.name.clone(), package: pf.package.clone(), file: pf.path.clone(), numbers: m.fields.iter().map(|x| (x.name.clone(), x.number)).collect() }
    }

    pub fn number(&self, field: &str) -> Option<i64> {
        self.numbers.get(field).copied()
    }
}

/// What an implementing rule is served as (§15.274): the contract's package, service and method, and
/// each message the service speaks, where it is declared and the number of each of its fields.
#[derive(Clone, Debug)]
pub struct Served {
    /// The contract as the rule writes it.
    pub file: String,
    /// Its path in the generated module, which `gen` copies it to (§15.160).
    pub import: String,
    pub package: String,
    pub service: String,
    pub method: String,
    pub request: Msg,
    pub response: Msg,
    /// The message of one row of `trace`.
    pub fired: Msg,
    /// The message of one element of each sequence, in the rule's order (§15.219).
    pub elements: Vec<Msg>,
}

impl Served {
    /// The path a Connect client posts to: `/shop.v1.MemberShippingService/Quote`.
    pub fn path(&self) -> String {
        if self.package.is_empty() { format!("/{}/{}", self.service, self.method) } else { format!("/{}.{}/{}", self.package, self.service, self.method) }
    }

    /// A message or an enum of the contract as the service's package names it: by its name from its
    /// package where that is the service's, and by its full name elsewhere.
    pub fn local(&self, package: &str, name: &str) -> String {
        if package == self.package || package.is_empty() { name.to_string() } else { format!("{package}.{name}") }
    }
}

/// The contract `f` implements, as `gen` serves it. `check` has held the rule to it, so it reads and
/// every name comes to something; None where it does not, which a rule that passed never meets.
pub fn served(f: &RuleFile, rule_path: &str) -> Option<Served> {
    let d = f.implements.as_ref()?;
    let read = load(&d.contract_path(rule_path).ok()?, &d.file).ok()?;
    let pf = read.entry();
    let svc = read.service(&d.service)?;
    let [m] = svc.methods.as_slice() else { return None };
    let (rf, req) = read.message(&pf.path, &pf.package, &m.input)?;
    let (sf, res) = read.message(&pf.path, &pf.package, &m.output)?;
    let Type::Named(t) = &res.fields.iter().find(|x| x.name == TRACE)?.ty else { return None };
    let (ff, fired) = read.message(&sf.path, &sf.full(&res.name), t)?;
    let mut elements = Vec::new();
    for el in &f.elements {
        let alias = pub_name(&el.name);
        let Type::Named(t) = &req.fields.iter().find(|x| x.name == alias)?.ty else { return None };
        let (ef, em) = read.message(&rf.path, &rf.full(&req.name), t)?;
        elements.push(Msg::of(ef, em));
    }
    Some(Served {
        file: d.file.clone(),
        import: read.entry.clone(),
        package: pf.package.clone(),
        service: svc.name.clone(),
        method: m.name.clone(),
        request: Msg::of(rf, req),
        response: Msg::of(sf, res),
        fired: Msg::of(ff, fired),
        elements,
    })
}

/// The field of the answer that carries the rows that matched (§15.112).
const TRACE: &str = "trace";

/// Holds the rule to the contract its `implements` line names: E013 where the contract cannot be read
/// or has no such service, E076 where the two do not agree.
pub fn check(f: &RuleFile, c: &Checked, rule_path: &str) -> Vec<Diag> {
    let Some(d) = &f.implements else { return Vec::new() };
    let here = format!("{rule_path}:{}", d.span.line);
    let followed = d.contract_path(rule_path);
    if let Some(why) = crate::vfs::refused(&followed) {
        return vec![Diag::error("E013", why).at(here).mark(d.span.clone(), "")];
    }
    let Ok(disk) = followed else { return Vec::new() };
    let read = match load(&disk, &d.file) {
        Ok(r) => r,
        Err(Unread::Missing) => {
            return vec![
                Diag::error("E013", tr!("`{}` を読めません", "Cannot read `{}`", d.file))
                    .at(here)
                    .mark(d.span.clone(), "")
                    .note(tr!(
                        "パスは規則ファイルのある場所からたどります（探した先: {}）。",
                        "The path is followed from the directory of the rule file (looked for: {}).",
                        disk.display()
                    ))
                    .maybe_note(crate::vfs::outside_note(d.dir(rule_path), &d.file)),
            ];
        }
        Err(Unread::Bad(at, e)) => {
            let title = match at {
                None => tr!("`{}` を読めません", "Cannot read `{}`", d.file),
                Some(at) => tr!("`{0}` が import する `{1}` を読めません", "Cannot read `{1}`, which `{0}` imports", d.file, at),
            };
            return vec![Diag::error("E013", title).at(here).mark(d.span.clone(), "").note(crate::proto::unreadable(&e))];
        }
    };
    let Some(svc) = read.service(&d.service) else {
        let names: Vec<&str> = read.entry().services.iter().map(|s| s.name.as_str()).collect();
        return vec![
            Diag::error("E013", tr!("`{}` にサービス `{}` はありません", "`{}` has no service `{}`", d.file, d.service))
                .at(here)
                .mark(d.span.clone(), "")
                .note(if names.is_empty() {
                    tr!("そのファイルにサービスは一つもありません。", "That file declares no service at all.")
                } else {
                    tr!("そのファイルにあるサービス: {}", "The services in that file: {}", names.join(", "))
                }),
        ];
    };
    // A rule whose types do not hold is not held to the contract yet: what its values are on the wire
    // is not known.
    if crate::has_error(&c.diags) {
        return Vec::new();
    }
    let mut h = Hold { f, read: &read, decl: d, rule_path, here, enums: BTreeMap::new(), told: BTreeSet::new(), not3: BTreeSet::new(), out: Vec::new() };
    h.read_enums();
    h.service(svc, &wire(f, c));
    h.out
}

/// Which message of the service a field is in, for what is said of it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Request,
    Answer,
    Element,
}

struct Hold<'a> {
    f: &'a RuleFile,
    read: &'a Read,
    decl: &'a ImplementsDecl,
    rule_path: &'a str,
    /// Where the `implements` line is, as a finding says it.
    here: String,
    /// Each enum of the rule's read with `import proto`: the file it is read from and its full name
    /// there, or None where that file does not read (which E013 has said).
    enums: BTreeMap<String, Option<(PathBuf, String)>>,
    /// The enums a finding has already said are not read from a contract.
    told: BTreeSet<String>,
    /// The files a finding has already said are not proto3.
    not3: BTreeSet<String>,
    out: Vec<Diag>,
}

impl<'a> Hold<'a> {
    fn read_enums(&mut self) {
        for im in self.f.enum_imports.iter().filter(|im| im.kind == EnumSource::Proto) {
            let id = im.file_path(self.rule_path).ok().and_then(|p| {
                let pf = ritsu_proto::read(&im.file, &ritsu_base::fs::read_to_string(&p).ok()?).ok()?;
                let e = pf.enums.iter().find(|e| e.name.rsplit('.').next() == Some(im.source.as_str()))?;
                Some((file_key(&p), pf.full(&e.name)))
            });
            self.enums.insert(im.target.text.clone(), id);
        }
    }

    /// A file of the contract as a finding names it: the contract as the rule writes it, and a file
    /// it imports as the import writes it.
    fn shown(&self, at: &str) -> String {
        if at == self.read.entry { self.decl.file.clone() } else { at.to_string() }
    }

    /// `line 12 of api/shop/v1/member_shipping.proto`.
    fn place(&self, pf: &ProtoFile, line: usize) -> String {
        tr!("{} の {} 行目", "line {1} of {0}", self.shown(&pf.path), line)
    }

    /// A finding about a name of the rule's, said at its declaration.
    fn at_name(&self, n: &Name, title: String) -> Diag {
        Diag::error("E076", title).at(format!("{}:{}", self.rule_path, n.span.line)).mark(n.span.clone(), "")
    }

    /// A finding about the contract alone, said at the `implements` line.
    fn at_line(&self, title: String) -> Diag {
        Diag::error("E076", title).at(self.here.clone()).mark(self.decl.span.clone(), "")
    }

    fn service(&mut self, svc: &ritsu_proto::Service, w: &Wire) {
        let pf = self.read.entry();
        if !self.proto3(pf) {
            return;
        }
        let mut wrong = Vec::new();
        let mut msgs = None;
        match svc.methods.as_slice() {
            [m] => {
                if m.client_streaming || m.server_streaming {
                    wrong.push(tr!(
                        "`{}` はストリームです。規則は一つの要求に一つの答えを返します。",
                        "`{}` streams, and a rule answers one request with one answer.",
                        m.name
                    ));
                }
                if !m.options.iter().any(|o| o.name.trim() == "idempotency_level" && o.text.trim() == "NO_SIDE_EFFECTS") {
                    wrong.push(tr!(
                        "`{}` に `option idempotency_level = NO_SIDE_EFFECTS;` がありません。規則は副作用の無い関数で、生成したサービスは GET でも呼ばれます。",
                        "`{}` does not say `option idempotency_level = NO_SIDE_EFFECTS;`. A rule is a function with no side effects, and the generated service is called with GET as well.",
                        m.name
                    ));
                }
                let req = self.read.message(&pf.path, &pf.package, &m.input);
                let res = self.read.message(&pf.path, &pf.package, &m.output);
                for (written, found) in [(&m.input, req.is_some()), (&m.output, res.is_some())] {
                    if !found {
                        wrong.push(tr!("`{}` は、契約が宣言するメッセージではありません。", "`{}` is not a message the contract declares.", written));
                    }
                }
                msgs = req.zip(res);
            }
            ms => wrong.push(tr!("手続きが {} 個あります。規則のサービスの手続きは一つです。", "It declares {} methods, and a rule's service has one.", ms.len())),
        }
        if !wrong.is_empty() {
            let mut d = self
                .at_line(tr!("契約のサービス `{}` は、規則のサービスの形ではありません", "The contract's service `{}` is not shaped as a rule's service", svc.name))
                .note(tr!("`{}` は {}にあります。", "`{}` is on {}.", svc.name, self.place(pf, svc.line)));
            for n in wrong {
                d = d.note(n);
            }
            self.out.push(d);
        }
        let Some(((rf, req), (sf, res))) = msgs else { return };
        if !self.proto3(rf) || !self.proto3(sf) {
            return;
        }
        // The request: an input by its alias, and each sequence a repeated message of its elements.
        let skip: Vec<String> = w.sequences.iter().map(|(n, _)| pub_name(n)).collect();
        self.fields(rf, req, &w.inputs, Side::Request, &skip);
        for (seq, fields) in &w.sequences {
            let alias = pub_name(seq);
            let Some(fl) = req.fields.iter().find(|x| x.name == alias) else {
                let d = self
                    .at_name(seq, tr!("並び `{}` が、契約の `{}` にありません", "The sequence `{}` is not in the contract's `{}`", seq.text, req.name))
                    .note(tr!("`{}` は {}にあります。", "`{}` is on {}.", req.name, self.place(rf, req.line)))
                    .note(tr!(
                        "契約の `{0}` に `repeated <要素のメッセージ> {1} = <番号>;` を足してください。番号は、`{0}` のどのフィールドもまだ使ったことのないものにします。要素のメッセージには、要素のフィールドを一つずつ `optional` で書きます。",
                        "Add `repeated <Message> {1} = <number>;` to the contract's `{0}`, with a number no field of `{0}` has ever used, the message holding each field of one element as an `optional` field.",
                        req.name,
                        alias
                    ));
                self.out.push(d);
                continue;
            };
            let element = match (&fl.ty, fl.label) {
                (Type::Named(t), Label::Repeated) if fl.json_name.is_none() && fl.oneof.is_none() => self.read.message(&rf.path, &rf.full(&req.name), t),
                _ => None,
            };
            match element {
                Some((ef, em)) => {
                    if self.proto3(ef) {
                        self.fields(ef, em, fields, Side::Element, &[]);
                    }
                }
                None => {
                    let d = self
                        .at_name(seq, tr!("`{}` の型が、契約と違います", "`{}` has another type in the contract", seq.text))
                        .note(tr!(
                            "規則のサービスでは、並びは要素一件のメッセージの `repeated` です。契約の `{}` には `{}` とあります（{}）。",
                            "A rule's service sends a sequence as a `repeated` message of one element. The contract's `{}` has `{}` ({}).",
                            req.name,
                            written(fl),
                            self.place(rf, fl.line)
                        ));
                    self.out.push(d);
                }
            }
        }
        // The answer: an output by its alias, and the rows that matched.
        self.fields(sf, res, &w.outputs, Side::Answer, &[TRACE.to_string()]);
        self.trace(sf, res);
    }

    /// Whether the file is proto3, said where it is not: proto2 and editions tell a field left out by
    /// other rules.
    fn proto3(&mut self, pf: &ProtoFile) -> bool {
        if pf.syntax == "proto3" {
            return true;
        }
        let shown = self.shown(&pf.path);
        if !self.not3.insert(pf.path.clone()) {
            return false;
        }
        let d = self
            .at_line(tr!("契約 `{}` は proto3 ではありません", "The contract `{}` is not proto3", shown))
            .note(if pf.syntax_line.is_some() {
                tr!("`{}` には `{}` と書かれています。", "`{}` says `{}`.", shown, pf.syntax)
            } else {
                tr!("`{}` に `syntax` の行が無いので、proto2 です。", "`{}` has no `syntax` line, so it is proto2.", shown)
            })
            .note(tr!(
                "読むのは proto3 だけです。フィールドが省かれたかどうかの決まりが、proto2 と editions では proto3 と違うからです。",
                "Only proto3 is read: proto2 and editions tell a field left out by other rules than proto3's."
            ));
        self.out.push(d);
        false
    }

    /// Holds the fields of the contract's `msg` to `want`, by name: each with its type and whether it
    /// is `optional`, and no field the rule does not have but those in `skip`, which are checked apart.
    fn fields(&mut self, pf: &ProtoFile, msg: &Message, want: &[Slot], side: Side, skip: &[String]) {
        let scope = pf.full(&msg.name);
        for w in want {
            let alias = pub_name(w.name);
            let Some(fl) = msg.fields.iter().find(|x| x.name == alias) else {
                self.missing(pf, msg, w, side);
                continue;
            };
            let mut odd = Vec::new();
            if let Some(j) = &fl.json_name {
                odd.push(tr!(
                    "`json_name = \"{}\"` を付けています。規則のサービスは、JSON のフィールド名を別名の lowerCamelCase で読み書きします。`rulec api` と dandori もそう読みます。",
                    "It sets `json_name = \"{}\"`. A rule's service reads and writes the JSON name of a field as the lowerCamelCase of the alias, and `rulec api` and dandori read it so.",
                    j
                ));
            }
            if let Some(o) = &fl.oneof {
                odd.push(tr!(
                    "`oneof {}` の中にあります。規則の値は、どれも別々に送ります。",
                    "It is in `oneof {}`, and a rule's values are each sent on their own.",
                    o
                ));
            }
            if matches!(fl.ty, Type::Map(..)) {
                odd.push(tr!("`map` です。規則の値に `map` はありません。", "It is a `map`, which no value of a rule's is."));
            }
            if !odd.is_empty() {
                let mut d = self
                    .at_name(w.name, tr!("契約の `{}` の `{}` は、規則のサービスが読まない書き方です", "The contract's `{}` writes `{}` in a way a rule's service does not", msg.name, alias))
                    .note(tr!("`{}` は {}にあります。", "`{}` is on {}.", written(fl), self.place(pf, fl.line)));
                for n in odd {
                    d = d.note(n);
                }
                self.out.push(d);
                continue;
            }
            if let Kind::Enum(e) = &w.kind {
                match self.enums.get(e) {
                    None => {
                        self.not_from_contract(e, w.name);
                        continue;
                    }
                    // the file the enum is read from does not read, which E013 has said
                    Some(None) => continue,
                    Some(Some(_)) => {}
                }
            }
            let same = matches!(fl.label, Label::None | Label::Optional) && self.same_type(pf, &scope, fl, &w.kind);
            if !same {
                let mut d = self
                    .at_name(w.name, tr!("`{}` の型が、契約と違います", "`{}` has another type in the contract", w.name.text))
                    .note(tr!(
                        "規則のサービスでは `{}` です。契約の `{}` には `{}` とあります（{}）。",
                        "A rule's service has it as `{}`. The contract's `{}` has `{}` ({}).",
                        self.wanted(w),
                        msg.name,
                        written(fl),
                        self.place(pf, fl.line)
                    ));
                d = match &w.kind {
                    Kind::Enum(e) => {
                        let (file, full) = self.f.enum_imports.iter().find(|im| im.target.text == *e).map(|im| (im.file.clone(), self.enum_full(e))).unwrap_or_default();
                        d.note(tr!(
                            "規則は列挙 `{}` を `{}` の `{}` から読んでいます。契約のフィールドは、その列挙そのものでなければなりません。",
                            "The rule reads the enum `{}` from `{2}` in `{1}`, and the contract's field has to be that very enum.",
                            e,
                            file,
                            full
                        ))
                    }
                    _ => d.note(tr!(
                        "真偽は `bool`、日付と文字列は `string`、そのほかの数（整数、金額、率、単位のある量）は `int64` です。",
                        "A truth value is `bool`, a date and a string are `string`, and every other number (a whole number, an amount, a rate, a quantity) is `int64`."
                    )),
                };
                self.out.push(d);
                continue;
            }
            if (fl.label == Label::Optional) != w.optional {
                let why = match side {
                    Side::Request | Side::Element => tr!(
                        "要求のフィールドには `optional` が要ります。省いたフィールドを 0 や false を入れたフィールドと区別して、規則が要る値を省いた要求を断るためです。",
                        "A field of the request needs `optional`: it is how one left out is told apart from one set to 0 or false, and a request that leaves out a value the rule needs is refused."
                    ),
                    Side::Answer if w.optional => tr!(
                        "`{}` は値の無いことがある出力です。値の無いことはフィールドを入れないことで返すので、答えのフィールドに `optional` が要ります。",
                        "`{}` is an output that may have no value, which the answer says by leaving the field unset, so the field needs `optional`.",
                        w.name.text
                    ),
                    Side::Answer => tr!(
                        "`{}` にはいつも値があるので、答えのフィールドに `optional` は付けません。",
                        "`{}` always has a value, so the field of the answer is not `optional`.",
                        w.name.text
                    ),
                };
                let d = self
                    .at_name(w.name, tr!("`{}` の `optional` が、契約と合っていません", "Whether `{}` is `optional` does not agree with the contract", w.name.text))
                    .note(tr!("契約の `{}` には `{}` とあります（{}）。", "The contract's `{}` has `{}` ({}).", msg.name, written(fl), self.place(pf, fl.line)))
                    .note(why);
                self.out.push(d);
            }
        }
        // what the contract has and the rule does not
        for fl in &msg.fields {
            if want.iter().any(|w| pub_name(w.name) == fl.name) || skip.contains(&fl.name) {
                continue;
            }
            let why = match side {
                Side::Request | Side::Element => tr!(
                    "サービスは、このフィールドに送られた値を読まずに捨てます。呼ぶ側は、その値が答えに効いたと思い込みます。",
                    "The service drops whatever is sent in it unread, and a caller takes it that the value counted."
                ),
                Side::Answer => tr!(
                    "サービスはこのフィールドを書かないので、呼ぶ側は 0・false・未設定の値を答えとして読みます。",
                    "The service never writes it, so a caller reads a 0, a false or an unset value as the answer."
                ),
            };
            let d = self
                .at_line(tr!("契約の `{}` の `{}` が、規則にありません", "The contract's `{}` has `{}`, which the rule does not", msg.name, fl.name))
                .note(tr!("`{}` は {}にあります。", "`{}` is on {}.", written(fl), self.place(pf, fl.line)))
                .note(why)
                .note(tr!(
                    "規則に `{}` を足すか、契約から消してその番号を `reserved` にしてください。受け取るだけで使わない値なら、規則に `T?` の入力として書き、`contract_only` を付けます。",
                    "Add `{}` to the rule, or take it out of the contract and make its number `reserved`. A value the service only takes and never reads is an input of `T?` marked `contract_only` in the rule.",
                    fl.name
                ));
            self.out.push(d);
        }
    }

    /// A name of the rule's the contract's message does not have (the first form of E076).
    fn missing(&mut self, pf: &ProtoFile, msg: &Message, w: &Slot, side: Side) {
        let what = match side {
            Side::Request => tr!("入力", "input"),
            Side::Element => tr!("要素のフィールド", "field of an element"),
            Side::Answer => tr!("出力", "output"),
        };
        let line = format!("{} {} = <{}>;", self.wanted_in(w, pf), pub_name(w.name), tr!("番号", "number"));
        let d = self
            .at_name(w.name, tr!("{} `{}` が、契約の `{}` にありません", "The {} `{}` is not in the contract's `{}`", what, w.name.text, msg.name))
            .note(tr!("`{}` は {}にあります。", "`{}` is on {}.", msg.name, self.place(pf, msg.line)))
            .note(tr!(
                "契約の `{0}` に `{1}` を足してください。番号は、`{0}` のどのフィールドもまだ使ったことのないものにします。規則の名前と契約のフィールドは、別名で対応します。",
                "Add `{1}` to the contract's `{0}`, with a number no field of `{0}` has ever used. A name of the rule's goes by its alias in the contract.",
                msg.name,
                line
            ));
        self.out.push(d);
    }

    /// An enum on the wire that is not read from a contract, said once for each, at the first value
    /// that carries it.
    fn not_from_contract(&mut self, e: &str, at: &Name) {
        if !self.told.insert(e.to_string()) {
            return;
        }
        let d = self
            .at_name(at, tr!("列挙 `{}` は、契約から読んだものではありません", "The enum `{}` is not read from a contract", e))
            .note(tr!(
                "`implements` を書いた規則が送る列挙は、`import proto` で `.proto` から読んだものだけです。値の番号を、規則ではなく契約が決めるからです。",
                "A rule that implements a contract sends only the enums it reads from a `.proto` with `import proto`: the numbers of the values are the contract's, not the rule's."
            ))
            .note(tr!(
                "列挙を契約の `.proto` に書き、`import proto \"<ファイル>\" <列挙> -> {}` で読んでください。",
                "Declare the enum in the contract's `.proto`, and read it with `import proto \"<file>\" <Enum> -> {}`.",
                e
            ));
        self.out.push(d);
    }

    /// The rows that matched: `repeated <message> trace`, the message of exactly `string table`,
    /// `int32 row` and `string label`, under any name and numbers.
    fn trace(&mut self, pf: &ProtoFile, res: &Message) {
        let shape = tr!(
            "答えは、当てはまった行を `repeated <メッセージ> trace` で返します。メッセージは `string table`、`int32 row`、`string label` の三つだけで、名前と番号は自由です。`rulec test` が評価器と比べ、`--record` が記録に書くのは、この行です。",
            "The answer carries the rows that matched as `repeated <Message> trace`, a message of `string table`, `int32 row` and `string label` and nothing else, under any name and numbers. They are what `rulec test` holds to the evaluator and what `--record` writes."
        );
        let Some(fl) = res.fields.iter().find(|x| x.name == TRACE) else {
            let d = self
                .at_line(tr!("契約の `{}` に `trace` がありません", "The contract's `{}` has no `trace`", res.name))
                .note(tr!("`{}` は {}にあります。", "`{}` is on {}.", res.name, self.place(pf, res.line)))
                .note(shape);
            self.out.push(d);
            return;
        };
        let mut wrong = Vec::new();
        let row = match (&fl.ty, fl.label) {
            (Type::Named(t), Label::Repeated) if fl.json_name.is_none() && fl.oneof.is_none() => self.read.message(&pf.path, &pf.full(&res.name), t),
            _ => None,
        };
        match row {
            None => wrong.push(tr!("`trace` は `{}` です（{}）。", "`trace` is `{}` ({}).", written(fl), self.place(pf, fl.line))),
            Some((rf, rm)) => {
                for (name, ty) in [("table", "string"), ("row", "int32"), ("label", "string")] {
                    match rm.fields.iter().find(|x| x.name == name) {
                        Some(x) if x.label == Label::None && x.json_name.is_none() && x.oneof.is_none() && matches!(&x.ty, Type::Scalar(s) if s == ty) => {}
                        Some(x) => wrong.push(tr!("`{}` の `{}` は `{}` です（{}）。", "`{}` has `{}` as `{}` ({}).", rm.name, name, written(x), self.place(rf, x.line))),
                        None => wrong.push(tr!("`{}` に `{} {}` がありません（{}）。", "`{}` has no `{} {}` ({}).", rm.name, ty, name, self.place(rf, rm.line))),
                    }
                }
                for x in rm.fields.iter().filter(|x| !["table", "row", "label"].contains(&x.name.as_str())) {
                    wrong.push(tr!("`{}` に `{}` があります（{}）。", "`{}` has `{}` ({}).", rm.name, written(x), self.place(rf, x.line)));
                }
            }
        }
        if wrong.is_empty() {
            return;
        }
        let mut d = self.at_line(tr!("契約の `{}` の `trace` の形が違います", "The `trace` of the contract's `{}` is not shaped as a rule's", res.name));
        for n in wrong {
            d = d.note(n);
        }
        self.out.push(d.note(shape));
    }

    /// Whether a field of the contract is of the type the rule's value is on the wire.
    fn same_type(&self, pf: &ProtoFile, scope: &str, fl: &Field, kind: &Kind) -> bool {
        match (kind, &fl.ty) {
            (Kind::Enum(e), Type::Named(n)) => match (self.enums.get(e), self.read.ps.resolve(&pf.path, scope, n)) {
                (Some(Some((disk, full))), Resolved::Found(s)) => s.is_enum && s.full == *full && self.read.disk.get(&s.file).is_some_and(|d| file_key(d) == *disk),
                _ => false,
            },
            (k, Type::Scalar(s)) => k.scalar() == Some(s.as_str()),
            _ => false,
        }
    }

    /// The full name of the enum the rule reads `e` from, as the file has it.
    fn enum_full(&self, e: &str) -> String {
        self.enums.get(e).cloned().flatten().map(|(_, full)| full).unwrap_or_else(|| e.to_string())
    }

    /// The field a rule's service has for `w`, label and type: `optional int64`.
    fn wanted(&self, w: &Slot) -> String {
        let ty = match &w.kind {
            Kind::Enum(e) => self.enum_full(e),
            k => k.scalar().unwrap_or_default().to_string(),
        };
        if w.optional { format!("optional {ty}") } else { ty }
    }

    /// The same, the enum named as a field of a message in `pf` names it.
    fn wanted_in(&self, w: &Slot, pf: &ProtoFile) -> String {
        let full = self.wanted(w);
        match full.rsplit_once(' ') {
            Some((label, ty)) => format!("{label} {}", local_to(pf, ty)),
            None => local_to(pf, &full),
        }
    }
}

/// A full name as a file of `pf`'s package writes it.
fn local_to(pf: &ProtoFile, full: &str) -> String {
    if pf.package.is_empty() {
        return full.to_string();
    }
    full.strip_prefix(&format!("{}.", pf.package)).unwrap_or(full).to_string()
}

/// A field as the contract writes it, without its options: `optional int64 subtotal = 5;`.
fn written(fl: &Field) -> String {
    let label = match fl.label {
        Label::None => "",
        Label::Optional => "optional ",
        Label::Required => "required ",
        Label::Repeated => "repeated ",
    };
    format!("{label}{} {} = {};", type_text(&fl.ty), fl.name, fl.number)
}

fn type_text(t: &Type) -> String {
    match t {
        Type::Scalar(s) | Type::Named(s) => s.clone(),
        Type::Map(k, v) => format!("map<{k}, {}>", type_text(v)),
    }
}
