//! ritsu's ports, as sakai answers them (ritsu's DESIGN 3.2): [`Engine`] gives what a `.ctx` holds
//! — a context and its terms — and the namings it writes of the artifacts outside it (`Items`,
//! `References`), and a map's contexts and relationships and the context a file belongs to
//! (`Maps`, which ritsu-cross holds the secrets a flow sends to: ritsu's X14). A short name of the
//! context's own published language (`means message Order`) is named in full from the `.proto`
//! that declares it, as sakai's check names it.

use crate::ast::{Element, File, Item as AstItem, RelKind, Str, Target};
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_ports::{Item, Reference, Said};
use std::path::Path;

/// sakai, as the ports reach it.
#[derive(Default)]
pub struct Engine;

fn read(root: &Path, file: &str) -> Result<(String, File), Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, file);
    let path = disk.to_string_lossy().to_string();
    let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
    match crate::parse::parse(file, &src) {
        (Some(f), ds) if !ds.iter().any(|d| d.is_error()) => Ok((src, f)),
        (_, ds) => Err(ds.iter().filter(|d| d.is_error()).map(|d| Said { file: path.clone(), ..Said::of(d) }).collect()),
    }
}

/// A line of a `.ctx` as a definition counts it: without its comment and the spaces around it.
fn plain(line: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => break,
            _ => {}
        }
        out.push(c);
    }
    out.trim().to_string()
}

fn text(src: &str, from: usize, to: usize) -> String {
    src.lines().skip(from.saturating_sub(1)).take(to + 1 - from).map(plain).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// The last line of a term's block: its line and the lines under it, up to the next line that is
/// indented no more than it.
fn block_end(src: &str, line: usize) -> usize {
    let lines: Vec<&str> = src.lines().collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let Some(first) = lines.get(line.saturating_sub(1)) else { return line };
    let mine = indent(first);
    let mut end = line;
    for (i, l) in lines.iter().enumerate().skip(line) {
        if plain(l).is_empty() {
            continue;
        }
        if indent(l) <= mine {
            break;
        }
        end = i + 1;
    }
    end
}

impl ritsu_ports::Items for Engine {
    /// A context file holds its context and its terms; a map holds neither. The context's
    /// definition is its file, and a term's its block (the term, its definition, and what it
    /// means), each line without its comment and the spaces around it.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let (src, f) = read(root, file)?;
        let File::Context(c) = f else { return Ok(vec![]) };
        let last = src.lines().enumerate().filter(|(_, l)| !plain(l).is_empty()).map(|(i, _)| i + 1).last().unwrap_or(1);
        let mut out = vec![Item { naming: Naming::file(Tool::Sakai, file).with("context", &c.heading.name), lines: (c.heading.pos.line, last), text: text(&src, c.heading.pos.line, last) }];
        for t in &c.terms {
            let end = block_end(&src, t.pos.line);
            out.push(Item { naming: Naming::file(Tool::Sakai, file).with("term", &t.name), lines: (t.pos.line, end), text: text(&src, t.pos.line, end) });
        }
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The namings a `.ctx` writes. A context: what it owns, its published language (the
    /// `.proto` files, the rule, the OpenAPI and AsyncAPI documents, the services, the generated
    /// code), the layers and kernels of its relationships, the enums a layer maps to, and what
    /// each term means. A map: the contexts it reads, what it covers and leaves out, its proto
    /// roots and its code.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        let (_, f) = read(root, file)?;
        let dir = ritsu_base::paths::parent(file);
        let at = |s: &Str| ritsu_base::paths::join(&dir, &s.value).ok();
        let mut out = Vec::new();
        let path = |s: &Str, tool: Tool, how: &str, out: &mut Vec<Reference>| {
            if let Some(p) = at(s) {
                out.push(Reference { line: s.pos.line, target: Naming::file(tool, p), how: how.into() });
            }
        };
        match &f {
            File::Map(m) => {
                m.uses.iter().for_each(|s| path(s, Tool::Sakai, "use context", &mut out));
                m.covers.iter().for_each(|s| path(s, Tool::File, "covers", &mut out));
                m.except.iter().for_each(|s| path(s, Tool::File, "except", &mut out));
                m.proto_roots.iter().for_each(|s| path(s, Tool::File, "proto root", &mut out));
                for c in &m.code {
                    path(&c.path, Tool::File, "code", &mut out);
                }
            }
            File::Context(c) => {
                let item = |it: &AstItem, how: &str, out: &mut Vec<Reference>| {
                    if let Some(p) = at(&it.path) {
                        out.push(Reference { line: it.path.pos.line, target: Naming::file(it.tool.unwrap_or(Tool::File), p), how: how.into() });
                    }
                };
                c.owns.iter().for_each(|it| item(it, "owns", &mut out));
                // the context's own published language: its `.proto` files, read for the short names
                let protos: Vec<(String, ritsu_proto::ProtoFile)> = c
                    .published
                    .iter()
                    .flat_map(|p| p.protos.iter())
                    .filter_map(|s| {
                        let p = at(s)?;
                        let src = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, &p)).ok()?;
                        Some((p.clone(), ritsu_proto::read(&p, &src).ok()?))
                    })
                    .collect();
                let short = |kind: &str, name: &str| -> Option<Naming> {
                    let mut found: Vec<(String, String)> = Vec::new();
                    for (p, f) in &protos {
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
                            if there && !found.contains(&(p.clone(), t.clone())) {
                                found.push((p.clone(), t));
                            }
                        }
                    }
                    // a name in two packages is sakai's E007, and names nothing here
                    (found.len() == 1).then(|| Naming::file(Tool::Proto, found[0].0.clone()).with(kind, found[0].1.clone()))
                };
                let element = |e: &Element| -> Option<Naming> {
                    match e {
                        Element::Long { written, .. } => {
                            let p = ritsu_base::paths::join(&dir, &written.path).ok()?;
                            let mut n = Naming::file(written.tool, p);
                            for (k, _, v, _) in &written.items {
                                n = n.with(k, v.clone());
                            }
                            Some(n)
                        }
                        Element::Short { kind, name, child, .. } => {
                            let mut n = short(kind, name)?;
                            if let Some((ck, cn)) = child {
                                n = n.with(ck, cn.clone());
                            }
                            Some(n)
                        }
                    }
                };
                for p in &c.published {
                    for s in &p.protos {
                        path(s, Tool::Proto, "published language", &mut out);
                    }
                    if let Some(s) = &p.rulec {
                        path(s, Tool::Rulec, "published language", &mut out);
                    }
                    // the OpenAPI and AsyncAPI documents (DESIGN 15.4): a file of the published
                    // language, which `ritsu check` reads for the keys written in it (W901)
                    for (k, s) in &p.contracts {
                        path(s, k.tool(), "published language", &mut out);
                    }
                    for (svc, pos) in &p.services {
                        if let Some(n) = short("service", svc) {
                            out.push(Reference { line: pos.line, target: n, how: "open host service".into() });
                        }
                    }
                    for s in &p.generated {
                        path(s, Tool::File, "generated", &mut out);
                    }
                }
                for r in &c.relations {
                    match &r.kind {
                        RelKind::SharedKernel(items) => items.iter().for_each(|it| item(it, "shared kernel", &mut out)),
                        RelKind::Upstream(u) => {
                            u.layer.iter().for_each(|it| item(it, "layer", &mut out));
                            for em in &u.enums {
                                if let Target::Element(e) = &em.target
                                    && let Some(n) = element(e)
                                {
                                    out.push(Reference { line: em.pos.line, target: n, how: "enum".into() });
                                }
                            }
                        }
                        _ => {}
                    }
                }
                for t in &c.terms {
                    for e in &t.means {
                        if let Some(n) = element(e) {
                            out.push(Reference { line: e.pos().line, target: n, how: "means".into() });
                        }
                    }
                }
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }
}

/// What the stages of sakai's check say is wrong, as the ports say it: the file from the root.
fn said(ds: &[crate::diag::Diag]) -> Vec<Said> {
    ds.iter().filter(|d| d.is_error()).map(|d| Said { file: d.rel.clone(), ..Said::of(d) }).collect()
}

/// A map read as far as the stages of sakai's check that decide what the port answers: its
/// contexts and their relationships (the names, the sections and the paths, E001 to E012) and who
/// owns what (E101 to E103). None for a context file; what those stages say, when one of them
/// does not pass. The stages after them (the references, the patterns, the mappings) say nothing
/// of which context a file is in, and are left to the map's own check.
fn map_model(root: &Path, map: &str) -> Result<Option<crate::model::Model>, Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, map);
    let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(map, &e.to_string())])?;
    match crate::parse::kind_of(&src) {
        Some(k) if k == crate::kw::MAP => {}
        Some(_) => return Ok(None),
        None => return Err(said(&crate::parse::parse(map, &src).1)),
    }
    let loaded = crate::resolve::load(root, map).map_err(|t| vec![Said { code: String::new(), file: map.to_string(), line: None, message: t }])?;
    if loaded.diags.iter().any(|d| d.is_error()) {
        return Err(said(&loaded.diags));
    }
    let Some(m) = loaded.model else { return Err(said(&loaded.diags)) };
    let (_, owned) = crate::owners::own(&m);
    // who owns what reads the documents' `$ref`s, and keeps them for a check that is not coming
    crate::contracts::forget();
    if owned.iter().any(|d| d.is_error()) {
        return Err(said(&owned));
    }
    Ok(Some(m))
}

impl ritsu_ports::Maps for Engine {
    /// The map's contexts, in the order of its `use context`, and every relationship as the
    /// `.ctx` of the context it starts from writes it.
    fn map(&self, root: &Path, map: &str) -> Result<Option<ritsu_ports::MapFacts>, Vec<Said>> {
        let Some(m) = map_model(root, map)? else { return Ok(None) };
        let mut relationships = Vec::new();
        for c in &m.contexts {
            for r in &c.rels {
                relationships.push(ritsu_ports::MapRelationship {
                    from: c.name.clone(),
                    to: m.contexts[r.partner].name.clone(),
                    words: r.kind.words().to_string(),
                    separate: matches!(r.kind, crate::model::RelK::Separate),
                    file: c.file.clone(),
                    line: r.pos.line,
                });
            }
        }
        Ok(Some(ritsu_ports::MapFacts { file: map.to_string(), contexts: m.contexts.iter().map(|c| c.name.clone()).collect(), relationships }))
    }

    /// The context of the deepest entry of `owns` that holds `file`, as sakai's check gives every
    /// artifact its context; None for a file outside the map's scope or in no context's `owns`.
    fn context_of(&self, root: &Path, map: &str, file: &str) -> Result<Option<String>, Vec<Said>> {
        let Some(m) = map_model(root, map)? else { return Ok(None) };
        if !crate::owners::in_scope(&m, file) {
            return Ok(None);
        }
        Ok(crate::owners::context_of(&m, file).map(|c| m.contexts[c].name.clone()))
    }

    /// What each context opens to the others, from the names its `open host service` lines list
    /// (sekisho's X15): a service of a `.proto` of the same published language is each of its
    /// methods; an operation of an OpenAPI document of it, by its `operationId` (or its method and
    /// path), is that operation, open to anyone when its document says so (`security: []`, or a
    /// requirement that asks for nothing). A channel of an AsyncAPI document, a rule's Connect
    /// service, and a name the published language does not have (sakai's E301) are left out.
    fn published_operations(&self, root: &Path, map: &str) -> Result<Vec<ritsu_ports::PublishedOperation>, Vec<Said>> {
        let Some(m) = map_model(root, map)? else { return Ok(Vec::new()) };
        let text = |f: &str| ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, f)).ok();
        let mut protos: std::collections::BTreeMap<String, Option<ritsu_proto::ProtoFile>> = std::collections::BTreeMap::new();
        let mut docs: std::collections::BTreeMap<String, Option<ritsu_base::openapi::Document>> = std::collections::BTreeMap::new();
        let mut out = Vec::new();
        for c in &m.contexts {
            for p in &c.published {
                for (listed, pos) in &p.services {
                    let at = |operation: Naming, open_to_anyone: bool| ritsu_ports::PublishedOperation { context: c.name.clone(), operation, open_to_anyone, file: c.file.clone(), line: pos.line };
                    for (f, _) in &p.protos {
                        let pf = protos.entry(f.clone()).or_insert_with(|| ritsu_proto::read(f, &text(f)?).ok());
                        if let Some(s) = pf.as_ref().and_then(|pf| pf.service(listed)) {
                            for x in &s.methods {
                                out.push(at(Naming::file(Tool::Proto, f.clone()).with("service", s.name.clone()).with("method", x.name.clone()), false));
                            }
                        }
                    }
                    for (kind, f, _) in &p.contracts {
                        if *kind != crate::contracts::Kind::OpenApi {
                            continue;
                        }
                        let doc = docs.entry(f.clone()).or_insert_with(|| {
                            let disk = ritsu_base::paths::on_disk(root, f);
                            let load = |q: &str| ritsu_base::fs::read_to_string(Path::new(q)).ok();
                            ritsu_base::openapi::read_with(&disk.to_string_lossy(), &text(f)?, &load).ok()
                        });
                        if let Some(op) = doc.as_ref().and_then(|d| d.operation(listed)).filter(|op| !op.webhook) {
                            out.push(at(Naming::file(Tool::Openapi, f.clone()).with("operation", op.name()), op.open_to_anyone()));
                        }
                    }
                }
            }
        }
        Ok(out)
    }
}
