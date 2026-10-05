//! The things inside artifacts that a map names (DESIGN 2.1, 2.7; PLAN B.3): what a term
//! `means`, the upstream enum of a mapping and its target. A proto element is looked up in the
//! files read (E007 when it is not there); the short forms are looked up in the context's own
//! published language or, for the enum of a mapping, in the upstream's `through` packages. A
//! rule's element — an input, an output, an enum or a value of one — is looked up in what rulec
//! says the rule holds (its `Items`, kept in the project's index), once rulec has answered for
//! the rule (`Rules`).

use crate::ast::{Element, Pos, Target};
use crate::diag::{self, Diag};
use ritsu_base::text::Text;
use crate::model::{Model, RelK};
use crate::naming::{Name, Tool};
use crate::owners::Artifact;
use crate::paths;
use crate::contracts::Contracts;
use crate::proto::{ProtoFile, Protos};
use ritsu_ports::{Index, Lookup};
use std::collections::BTreeMap;

/// Where an element is named: a term's `means`, or a mapping of an upstream relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum At {
    /// (context, term, the `means` line)
    Means(usize, usize, usize),
    /// (context, relationship, enum mapping): the upstream enum
    From(usize, usize, usize),
    /// (context, relationship, enum mapping): the target
    To(usize, usize, usize),
}

#[derive(Clone, Debug, Default)]
pub struct Elements {
    pub found: BTreeMap<At, Name>,
    /// Whether every element named was found (or is not sakai's to look up yet). When one was
    /// not, what crosses is not known in full, and the glossary's warnings wait (W401).
    pub complete: bool,
}

impl Elements {
    pub fn get(&self, at: At) -> Option<&Name> {
        self.found.get(&at)
    }
}

/// Whether a proto file has the element the pairs name.
fn has(f: &ProtoFile, items: &[(String, String)]) -> Result<(), Text> {
    let (k, n) = &items[0];
    let child = items.get(1);
    let path = paths::shown(&f.path);
    let missing = || tr!("{path} に {k} {n} はありません", "There is no {k} {n} in {path}");
    match k.as_str() {
        "message" => {
            let m = f.message(n).ok_or_else(missing)?;
            if let Some((ck, cn)) = child
                && !m.fields.iter().any(|x| x.name == *cn)
            {
                return Err(tr!("message {n} に {ck} {cn} はありません", "The message {n} has no {ck} {cn}"));
            }
        }
        "enum" => {
            let e = f.enumeration(n).ok_or_else(missing)?;
            if let Some((ck, cn)) = child
                && !e.values.iter().any(|x| x.name == *cn)
            {
                return Err(tr!("enum {n} に {ck} {cn} はありません", "The enum {n} has no {ck} {cn}"));
            }
        }
        "service" => {
            let s = f.service(n).ok_or_else(missing)?;
            if let Some((ck, cn)) = child
                && !s.methods.iter().any(|x| x.name == *cn)
            {
                return Err(tr!("service {n} に {ck} {cn} はありません", "The service {n} has no {ck} {cn}"));
            }
        }
        _ => return Err(missing()),
    }
    Ok(())
}

/// A short name (`Order`, `Order.Line`, `warehouse.v1.Order`) looked up in `files`: the files it
/// is in, with its name from the file's package.
fn lookup(ps: &Protos, files: &[String], kind: &str, name: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for p in files {
        let Some(f) = ps.files.get(p) else { continue };
        let mut tries = vec![name.to_string()];
        if let Some(rest) = name.strip_prefix(&format!("{}.", f.package))
            && !f.package.is_empty()
        {
            tries.push(rest.to_string());
        }
        for t in tries {
            let there = match kind {
                "message" => f.message(&t).is_some(),
                "enum" => f.enumeration(&t).is_some(),
                "service" => f.service(&t).is_some(),
                _ => false,
            };
            if there && !out.contains(&(p.clone(), t.clone())) {
                out.push((p.clone(), t));
            }
        }
    }
    out
}

struct E<'a> {
    m: &'a Model,
    ps: &'a Protos,
    cs: &'a Contracts,
    arts: &'a [Artifact],
    read: &'a crate::suite::Read,
    diags: Vec<Diag>,
}

/// An element of an OpenAPI or AsyncAPI document, as sakai holds it (DESIGN 15.4): the file, and
/// the JSON Pointer as its one pair (`#`), with a value of an enum after it.
pub fn contract_name(file: &str, pointer: &str) -> Name {
    Name::file(Tool::File, file.to_string()).with("#", pointer.to_string())
}

/// The file and the pointer of an element of a document, if the name is one.
pub fn as_contract(n: &Name) -> Option<(&str, &str)> {
    match (n.tool, n.items.first()) {
        (Tool::File, Some((k, p))) if k == "#" => Some((&n.path, p)),
        _ => None,
    }
}

/// An element as a person reads it: a name of DESIGN 2, or an element of a document as its file
/// and its JSON Pointer (`payments/api.yaml#/components/schemas/Charge value refunded`).
pub fn display(n: &Name) -> String {
    match as_contract(n) {
        Some((f, p)) => {
            let mut s = crate::contracts::shown(f, p);
            for (k, v) in &n.items[1..] {
                s.push_str(&format!(" {k} {}", ritsu_base::naming::word_or_quote(v)));
            }
            s
        }
        None => n.text(),
    }
}

/// What a diagnostic says is involved, for an element: its name, or for an element of a
/// document the line its key is at.
pub fn refer(cs: &Contracts, context: Option<&str>, n: &Name, what: Text) -> crate::diag::Ref {
    match as_contract(n) {
        Some((f, p)) => {
            let line = cs.key_line(f, p).unwrap_or(1);
            // the file is the place the line names; what is there is its pointer
            let mut shown = format!("#{p}");
            for (k, v) in &n.items[1..] {
                shown.push_str(&format!(" {k} {}", ritsu_base::naming::word_or_quote(v)));
            }
            let (wj, we) = (what.ja.clone(), what.en.clone());
            let what = if wj.is_empty() { Text::same(shown.clone()) } else { tr!("{shown}（{wj}）", "{shown}, {we}") };
            crate::diag::Ref::line(context, f, line, what)
        }
        None => crate::diag::Ref::name(context, n.clone(), what),
    }
}

/// Whether a rule has the element a name names — an input, an output, an enum, or a value of one —
/// looked up in the project's index, where rulec says what the rule holds (its `Items`; ritsu's
/// DESIGN 6.4, X10). What is not there is sakai's E007, in sakai's words.
fn rule_has(index: &Index, root: &std::path::Path, name: &Name) -> Result<(), Text> {
    let (k, n) = &name.items[0];
    let sf = paths::shown(&name.path);
    let missing = || tr!("{sf} に {k} {n} はありません", "There is no {k} {n} in {sf}");
    if !matches!(k.as_str(), "input" | "output" | "enum") {
        return Err(missing());
    }
    let found = |nm: &Name| matches!(index.find(root, nm), Lookup::Found(Some(_)));
    if !found(&Name { items: name.items[..1].to_vec(), ..name.clone() }) {
        return Err(missing());
    }
    match name.items.get(1) {
        Some((ck, cn)) if !found(name) => Err(tr!("enum {n} に {ck} {cn} はありません", "The enum {n} has no {ck} {cn}")),
        _ => Ok(()),
    }
}

impl E<'_> {
    fn err(&mut self, c: usize, p: Pos, code: &'static str, msg: Text) -> &mut Diag {
        let cx = &self.m.contexts[c];
        self.diags.push(diag::at(code, &cx.file, p.line, p.col, msg).source(&cx.src));
        self.diags.last_mut().unwrap()
    }

    /// An element in its long or short form; `files` and `contract_files` (the `.proto` files and
    /// the OpenAPI and AsyncAPI documents) are where a short one is looked up, and `where_` says
    /// what those files are, for the message when it is not there.
    fn element(&mut self, c: usize, e: &Element, files: &[String], contract_files: &[String], where_: &Text) -> Option<Name> {
        match e {
            Element::Long { written, pos } => {
                let path = paths::join(&self.m.contexts[c].dir, &written.path).ok()?;
                let items: Vec<(String, String)> = written.items.iter().map(|(k, _, n, _)| (k.clone(), n.clone())).collect();
                let name = Name { tool: written.tool, path: path.clone(), items };
                if written.tool == Tool::Rulec && !name.items.is_empty() {
                    // A rule's names, as rulec says them; a rule rulec did not answer for is told
                    // as E104 or E105 already, and its names are taken as written.
                    if self.read.facts.contains_key(&path)
                        && let Err(msg) = rule_has(&self.read.index, &self.m.root, &name)
                    {
                        self.err(c, *pos, "E007", msg);
                        return None;
                    }
                    return Some(name);
                }
                if written.tool != Tool::Proto {
                    // The other tools' names are taken as written (DESIGN 2.2).
                    return Some(name);
                }
                match self.ps.files.get(&path) {
                    Some(f) => {
                        if name.items.is_empty() {
                            return Some(name);
                        }
                        match has(f, &name.items) {
                            Ok(()) => Some(name),
                            Err(msg) => {
                                self.err(c, *pos, "E007", msg);
                                None
                            }
                        }
                    }
                    None => {
                        if !self.arts.iter().any(|a| a.path == path) {
                            let t = name.text();
                            self.err(c, *pos, "E103", tr!("{t} は地図の範囲の外の proto です", "The name {t} points at a proto outside the map's scope"));
                        }
                        // An artifact of the scope that could not be read is told as E106 already.
                        None
                    }
                }
            }
            Element::Short { kind, name, child, pos } => {
                let found = if matches!(kind.as_str(), "message" | "enum" | "service") { lookup(self.ps, files, kind, name) } else { vec![] };
                // the elements of the OpenAPI and AsyncAPI documents (DESIGN 15.4)
                let docs: Vec<(String, String)> = self.cs.find(contract_files, kind, name);
                if found.is_empty() && docs.len() == 1 {
                    let (f, p) = &docs[0];
                    let mut n = contract_name(f, p);
                    if let Some((ck, cn)) = child {
                        if ck != "value" || kind != "enum" {
                            self.err(c, *pos, "E011", tr!("`{ck}` は `{kind}` の下に書けません", "`{ck}` cannot come under `{kind}`"));
                            return None;
                        }
                        let have = self.cs.enum_values(f, p).map(|e| e.values.iter().any(|v| v.name == *cn)).unwrap_or(false);
                        if !have {
                            self.err(c, *pos, "E007", tr!("enum {name} に value {cn} はありません", "The enum {name} has no value {cn}"));
                            return None;
                        }
                        n = n.with("value", cn.clone());
                    }
                    return Some(n);
                }
                // a name that is in a document and somewhere else (a `.proto`, or another document);
                // one that two packages of `.proto` files have is told below, as before
                if !docs.is_empty() && found.len() + docs.len() > 1 {
                    let mut cands: Vec<String> = found.iter().map(|(p, r)| self.ps.files[p].full(r)).collect();
                    cands.extend(docs.iter().map(|(f, p)| crate::contracts::shown(f, p)));
                    let list = cands.join("、");
                    let list_en = cands.join(", ");
                    self.err(c, *pos, "E007", tr!("{kind} {name} は、公表された言語の二つ以上の要素に当たります（{list}）", "The name {kind} {name} is in more than one place of the published languages ({list_en})")).notes.push(tr!(
                        "どれか一つに決まるように、名前を変えるか、要素を一つの公表された言語にまとめてください。",
                        "Rename one of them, or keep the element in one published language, so that the name finds one."
                    ));
                    return None;
                }
                if found.is_empty() && contract_files.iter().any(|f| self.cs.unread.contains(f)) {
                    // a document that could not be read is told as E108 already
                    return None;
                }
                match found.len() {
                    // A file that could not be read is told as E106 already; what is in it is not known.
                    0 if files.iter().any(|f| !self.ps.files.contains_key(f)) => None,
                    0 => {
                        let w = where_.clone();
                        self.err(c, *pos, "E007", tr!("{} に {kind} {name} はありません", "There is no {kind} {name} in {}", w.ja; w.en));
                        None
                    }
                    1 => {
                        let (p, rel) = &found[0];
                        let mut n = Name::file(Tool::Proto, p.clone()).with(kind, rel.clone());
                        if let Some((ck, cn)) = child {
                            n = n.with(ck, cn.clone());
                            if let Err(msg) = has(&self.ps.files[p], &n.items) {
                                self.err(c, *pos, "E007", msg);
                                return None;
                            }
                        }
                        Some(n)
                    }
                    _ => {
                        let cands: Vec<String> = found.iter().map(|(p, r)| self.ps.files[p].full(r)).collect();
                        let list = cands.join("、");
                        let list_en = cands.join(", ");
                        let first = &cands[0];
                        self.err(c, *pos, "E007", tr!("{kind} {name} は二つ以上の package に当たります（{list}）", "The name {kind} {name} is in more than one package ({list_en})"))
                            .notes
                            .push(tr!("package から書いてください（`{kind} {first}`）。", "Write it with its package (`{kind} {first}`)."));
                        None
                    }
                }
            }
        }
    }
}

/// The proto files of a context's own published languages.
pub fn own_published(m: &Model, c: usize) -> Vec<String> {
    m.contexts[c].published.iter().flat_map(|p| p.protos.iter().map(|(f, _)| f.clone())).collect()
}

fn packages_text(ps: &[String]) -> Text {
    if ps.is_empty() {
        return tr!("公表された言語", "the published language");
    }
    let j = ps.join("、");
    let e = ps.join(", ");
    tr!("公表された言語 {j}", "the published language {e}")
}

/// The OpenAPI and AsyncAPI documents of a context's published languages named `packages` (all of
/// them for None), with the parts they reach that are the context's.
pub fn published_contracts(m: &Model, cs: &Contracts, arts: &[Artifact], c: usize, packages: Option<&[String]>) -> Vec<String> {
    let owner = |p: &str| arts.iter().find(|a| a.path == p).and_then(|a| a.ctx());
    let listed: Vec<String> = m.contexts[c].published.iter().filter(|p| packages.is_none_or(|ks| ks.contains(&p.package))).flat_map(|p| p.contracts.iter().map(|(_, f, _)| f.clone())).collect();
    cs.published_files(&listed, &owner, c).into_iter().collect()
}

/// Resolve every element the map names.
pub fn resolve(m: &Model, ps: &Protos, cs: &Contracts, arts: &[Artifact], read: &crate::suite::Read) -> (Elements, Vec<Diag>) {
    let mut e = E { m, ps, cs, arts, read, diags: Vec::new() };
    let mut out = Elements { complete: true, ..Elements::default() };
    for (ci, c) in m.contexts.iter().enumerate() {
        let own = own_published(m, ci);
        let own_docs = published_contracts(m, cs, arts, ci, None);
        let own_pkgs: Vec<String> = c.published.iter().filter(|p| !p.protos.is_empty() || !p.contracts.is_empty()).map(|p| p.package.clone()).collect();
        let own_text = packages_text(&own_pkgs);
        for (ti, t) in c.ast.terms.iter().enumerate() {
            for (mi, me) in t.means.iter().enumerate() {
                if let Some(n) = e.element(ci, me, &own, &own_docs, &own_text) {
                    out.found.insert(At::Means(ci, ti, mi), n);
                }
            }
        }
        for (ri, r) in c.rels.iter().enumerate() {
            let RelK::Upstream { through, enums, .. } = &r.kind else { continue };
            let up = &m.contexts[r.partner];
            let pkgs: Vec<String> = through.iter().map(|(p, _)| p.clone()).collect();
            let files: Vec<String> = up.published.iter().filter(|p| pkgs.contains(&p.package)).flat_map(|p| p.protos.iter().map(|(f, _)| f.clone())).collect();
            let docs = published_contracts(m, cs, arts, r.partner, Some(&pkgs));
            let through_text = packages_text(&pkgs);
            for (ei, em) in enums.iter().enumerate() {
                let from = Element::Short { kind: "enum".into(), name: em.from.clone(), child: None, pos: em.from_pos };
                if let Some(n) = e.element(ci, &from, &files, &docs, &through_text) {
                    out.found.insert(At::From(ci, ri, ei), n);
                }
                if let Target::Element(te) = &em.target {
                    // The target is an enum: a proto enum or a rule's.
                    let ok_kind = match te {
                        Element::Long { written, .. } => written.items.len() == 1 && written.items[0].0 == "enum" && matches!(written.tool, Tool::Proto | Tool::Rulec),
                        Element::Short { kind, child, .. } => kind == "enum" && child.is_none(),
                    };
                    if !ok_kind {
                        e.err(ci, te.pos(), "E011", tr!("対応の先にできるのは、proto の列挙か rulec の規則の列挙です", "A mapping's target is a proto enum or a rule's enum")).notes.push(tr!(
                            "`proto \"<パス>\" enum <列挙>`、自分の公表された言語の `enum <列挙>`、`rulec \"<パス>\" enum <列挙>` のどれかか、名前だけを書いてください。",
                            "Write `proto \"<path>\" enum <enum>`, `enum <enum>` of the context's own published language, `rulec \"<path>\" enum <enum>`, or a name only."
                        ));
                        continue;
                    }
                    if let Some(n) = e.element(ci, te, &own, &own_docs, &own_text) {
                        out.found.insert(At::To(ci, ri, ei), n);
                    }
                }
            }
        }
    }
    out.complete = e.diags.is_empty()
        && !m.contexts.iter().flat_map(|c| c.published.iter()).flat_map(|p| p.protos.iter()).any(|(f, _)| !ps.files.contains_key(f))
        && cs.unread.is_empty();
    (out, e.diags)
}
