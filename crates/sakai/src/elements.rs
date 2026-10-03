//! The things inside artifacts that a map names (DESIGN 2.1, 2.7; PLAN B.3): what a term
//! `means`, the upstream enum of a mapping and its target. A proto element is looked up in the
//! files read (E007 when it is not there); the short forms are looked up in the context's own
//! published language or, for the enum of a mapping, in the upstream's `through` packages. A
//! rule's element is checked from stage C, with `rulec api`.

use crate::ast::{Element, Pos, Target};
use crate::diag::{self, Diag};
use ritsu_base::text::Text;
use crate::model::{Model, RelK};
use crate::naming::{Name, Tool};
use crate::owners::Artifact;
use crate::paths;
use crate::proto::{ProtoFile, Protos};
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
    arts: &'a [Artifact],
    diags: Vec<Diag>,
}

impl E<'_> {
    fn err(&mut self, c: usize, p: Pos, code: &'static str, msg: Text) -> &mut Diag {
        let cx = &self.m.contexts[c];
        self.diags.push(diag::at(code, &cx.file, p.line, p.col, msg).source(&cx.src));
        self.diags.last_mut().unwrap()
    }

    /// An element in its long or short form; `files` are where a short one is looked up, and
    /// `where_` says what those files are, for the message when it is not there.
    fn element(&mut self, c: usize, e: &Element, files: &[String], where_: &Text) -> Option<Name> {
        match e {
            Element::Long { written, pos } => {
                let path = paths::join(&self.m.contexts[c].dir, &written.path).ok()?;
                let items: Vec<(String, String)> = written.items.iter().map(|(k, _, n, _)| (k.clone(), n.clone())).collect();
                let name = Name { tool: written.tool, path: path.clone(), items };
                if written.tool != Tool::Proto {
                    // A rule's names are read from `rulec api`, from stage C; the other tools' names
                    // are taken as written (DESIGN 2.2).
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
                let found = lookup(self.ps, files, kind, name);
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
                            .push(tr!("package から書きます（`{kind} {first}`）。", "Write it with its package (`{kind} {first}`)."));
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

/// Resolve every element the map names.
pub fn resolve(m: &Model, ps: &Protos, arts: &[Artifact]) -> (Elements, Vec<Diag>) {
    let mut e = E { m, ps, arts, diags: Vec::new() };
    let mut out = Elements { complete: true, ..Elements::default() };
    for (ci, c) in m.contexts.iter().enumerate() {
        let own = own_published(m, ci);
        let own_pkgs: Vec<String> = c.published.iter().filter(|p| !p.protos.is_empty()).map(|p| p.package.clone()).collect();
        let own_text = packages_text(&own_pkgs);
        for (ti, t) in c.ast.terms.iter().enumerate() {
            for (mi, me) in t.means.iter().enumerate() {
                if let Some(n) = e.element(ci, me, &own, &own_text) {
                    out.found.insert(At::Means(ci, ti, mi), n);
                }
            }
        }
        for (ri, r) in c.rels.iter().enumerate() {
            let RelK::Upstream { through, enums, .. } = &r.kind else { continue };
            let up = &m.contexts[r.partner];
            let pkgs: Vec<String> = through.iter().map(|(p, _)| p.clone()).collect();
            let files: Vec<String> = up.published.iter().filter(|p| pkgs.contains(&p.package)).flat_map(|p| p.protos.iter().map(|(f, _)| f.clone())).collect();
            let through_text = packages_text(&pkgs);
            for (ei, em) in enums.iter().enumerate() {
                let from = Element::Short { kind: "enum".into(), name: em.from.clone(), child: None, pos: em.from_pos };
                if let Some(n) = e.element(ci, &from, &files, &through_text) {
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
                            "`proto \"<パス>\" enum <列挙>`、自分の公表された言語の `enum <列挙>`、`rulec \"<パス>\" enum <列挙>` のどれかか、名前だけを書きます。",
                            "Write `proto \"<path>\" enum <enum>`, `enum <enum>` of the context's own published language, `rulec \"<path>\" enum <enum>`, or a name only."
                        ));
                        continue;
                    }
                    if let Some(n) = e.element(ci, te, &own, &own_text) {
                        out.found.insert(At::To(ci, ri, ei), n);
                    }
                }
            }
        }
    }
    out.complete = e.diags.is_empty() && !m.contexts.iter().flat_map(|c| c.published.iter()).flat_map(|p| p.protos.iter()).any(|(f, _)| !ps.files.contains_key(f));
    (out, e.diags)
}
