//! ritsu's ports, as sakai answers them (ritsu's DESIGN 3.2): [`Engine`] gives what a `.ctx` holds
//! — a context and its terms — and the namings it writes of the artifacts outside it (`Items`,
//! `References`). A short name of the context's own published language (`means message Order`)
//! is named in full from the `.proto` that declares it, as sakai's check names it.

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
    let src = std::fs::read_to_string(&disk).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
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
    /// `.proto` files, the rule, the services, the generated code), the layers and kernels of its
    /// relationships, the enums a layer maps to, and what each term means. A map: the contexts it
    /// reads, what it covers and leaves out, its proto roots and its code.
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
                        let src = std::fs::read_to_string(ritsu_base::paths::on_disk(root, &p)).ok()?;
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
