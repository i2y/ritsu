//! ritsu's ports, as yuen answers them (ritsu's DESIGN 3.2): [`Engine`] gives what a `.req` holds
//! — its requirements and its sources — and the namings it writes of the things outside it
//! (`Items`, `References`). The definition of a requirement is its end (DESIGN 4.1), the bytes a
//! link to it is reviewed against; the definition of a source is the line of each article it pins.

use crate::ast::SourceKind;
use crate::project::{self, Project};
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_ports::{Item, Reference, Said};
use std::path::Path;

/// yuen, as the ports reach it.
#[derive(Default)]
pub struct Engine;

/// The file, read as a project of its own under `root`, with its names resolved.
fn load(root: &Path, file: &str) -> Result<Project, Vec<Said>> {
    let disk = ritsu_base::paths::on_disk(root, file).to_string_lossy().to_string();
    let r = root.to_string_lossy().to_string();
    match project::load(std::slice::from_ref(&disk), Some(&r)) {
        Err(project::Refusal(t)) => Err(vec![Said { code: String::new(), file: disk, line: None, message: t }]),
        Ok((None, diags)) => Err(diags.iter().filter(|d| d.is_error()).map(Said::of).collect()),
        Ok((Some(mut p), _)) => {
            project::check_names(&mut p);
            Ok(p)
        }
    }
}

/// The last line of a requirement's block.
fn req_end(d: &crate::ast::ReqDecl) -> usize {
    let mut end = d.span.line;
    let mut at = |l: usize| end = end.max(l);
    for l in [d.text.as_ref().map(|t| t.1.line), d.in_force.as_ref().map(|t| t.1.line), d.owner.as_ref().map(|t| t.1.line)].into_iter().flatten() {
        at(l);
    }
    d.replaces.iter().for_each(|r| at(r.span.line));
    for f in &d.from {
        at(f.span.line);
        if let Some(r) = &f.record {
            at(r.line);
        }
    }
    d.decided.iter().for_each(|x| at(x.span.line));
    for l in &d.links {
        at(l.span.line);
        if let Some(r) = &l.record {
            at(r.line);
        }
    }
    for w in &d.waivers {
        at(w.span.line);
        if let Some(r) = &w.record {
            at(r.line);
        }
    }
    end
}

impl ritsu_ports::Items for Engine {
    /// Each requirement (the latest version a file writes of it, which is what its name names)
    /// and each source. A requirement's definition is its end: its text, a line for each upper
    /// end it is read from with that end's hash, and its period (DESIGN 4.1); it is empty when
    /// the end cannot be made (an upper end that cannot be read, a ring of `from`s). A source's is
    /// a line for each article it pins, as a `from` line writes it, in the order written.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let p = load(root, file)?;
        let (srcs, _) = crate::sources::check_sources(&p);
        let (_, in_cycle) = crate::graph::cycles(&p);
        let ends = crate::ends::requirement_ends(&p, &srcs, &in_cycle);
        let f = &p.files[0];
        let naming = |kind: &str, name: &str| Naming::file(Tool::Yuen, file).with(kind, name);
        let mut out = Vec::new();
        for (name, versions) in &p.by_name {
            let Some(&r) = versions.iter().filter(|r| p.reqs[**r].file == 0).max_by_key(|r| p.reqs[**r].version) else { continue };
            let d = p.decl(r);
            let text = ends[r].as_ref().map(|e| String::from_utf8_lossy(&e.bytes).to_string()).unwrap_or_default();
            out.push(Item { naming: naming("requirement", name), lines: (d.span.line, req_end(d)), text });
        }
        for (si, s) in f.ast.sources.iter().enumerate() {
            let (end, text) = match &s.kind {
                SourceKind::Law { db, id, pins, .. } => (
                    pins.iter().map(|x| x.span.line).fold(s.span.line, usize::max),
                    pins.iter().filter_map(|x| x.pin.as_ref().map(|h| format!("from law {} {id} {} sha256:{h}\n", db.word(), x.fragment))).collect::<String>(),
                ),
                SourceKind::File { pin, .. } => {
                    let path = p.names.sources[0].get(si).and_then(|n| n.as_ref()).map(|n| n.path.clone()).unwrap_or_default();
                    (s.span.line, pin.as_ref().map(|h| format!("from file {path} sha256:{h}\n")).unwrap_or_default())
                }
                SourceKind::Borrowed { .. } => (s.span.line, p.names.sources[0].get(si).and_then(|n| n.as_ref()).map(|n| format!("{}\n", n.text())).unwrap_or_default()),
            };
            out.push(Item { naming: naming("source", &s.name), lines: (s.span.line, end), text });
        }
        out.sort_by_key(|i| i.lines.0);
        Ok(out)
    }
}

impl ritsu_ports::References for Engine {
    /// The namings a `.req` writes: what meets a requirement and what checks it
    /// (`satisfied by`, `verified by`), what a `scope` gathers, a source a rule or a calendar
    /// pins, and the file a `file` source copies.
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        let p = load(root, file)?;
        let f = &p.files[0];
        let mut out = Vec::new();
        for (r, v) in p.reqs.iter().enumerate().filter(|(_, v)| v.file == 0) {
            let d = &f.ast.requirements[v.idx];
            for (l, n) in d.links.iter().zip(&p.names.links[r]) {
                if let Some(n) = n {
                    out.push(Reference { line: l.span.line, target: n.clone(), how: format!("{} by", l.side.word()) });
                }
            }
        }
        for (s, n) in f.ast.scopes.iter().zip(&p.names.scopes[0]) {
            if let Some((n, _)) = n {
                out.push(Reference { line: s.span.line, target: n.clone(), how: "scope".into() });
            }
        }
        for (s, n) in f.ast.sources.iter().zip(&p.names.sources[0]) {
            if let Some(n) = n {
                out.push(Reference { line: s.span.line, target: n.clone(), how: "source".into() });
            }
        }
        out.sort_by_key(|r| r.line);
        Ok(out)
    }
}
