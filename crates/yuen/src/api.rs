//! `yuen api` (DESIGN 11, PLAN B.13): the whole graph of a project as JSON, so that sakai and
//! the other tools read yuen from its output alone (the suite's P2). The keys come in the
//! order DESIGN 11 writes them, whatever the language; marks and gaps are in it as states.

use crate::ast::*;
use crate::check::{self, Checked};
use crate::i18n::Lang;
use crate::marks::{LinkKind, Thing};
use crate::sha256;
use crate::sources::Resolved;
use serde_json::{Value, json};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn reviewed(r: Option<&RecordLine>) -> Value {
    match r.and_then(|r| r.parsed.as_ref().ok()) {
        Some(rc) => json!({"date": rc.date.to_string(), "by": rc.by, "up": rc.up, "down": rc.down}),
        None => Value::Null,
    }
}

fn approved(r: Option<&RecordLine>) -> Value {
    match r.and_then(|r| r.parsed.as_ref().ok()) {
        Some(rc) => json!({"date": rc.date.to_string(), "by": rc.by, "sha256": rc.up.first()}),
        None => Value::Null,
    }
}

pub fn api(c: &Checked, label: &str, lang: Lang) -> Option<Value> {
    let p = c.project.as_ref()?;
    let m = c.model.as_ref()?;
    let files: Vec<Value> = p
        .files
        .iter()
        .map(|f| json!({"path": f.rel, "name": f.ast.header.name, "version": f.ast.header.version.to_string(), "sha256": sha256::short(f.src.as_bytes())}))
        .collect();
    let roles: Vec<Value> = p.files.iter().flat_map(|f| f.ast.roles.iter().map(|r| json!({"name": r.name, "description": r.description}))).collect();
    let mut sources = Vec::new();
    for (fi, f) in p.files.iter().enumerate() {
        for (name, r) in &m.sources.files[fi] {
            sources.push(match r {
                Resolved::Law { db, id, asof, revision, articles, .. } => json!({
                    "file": f.rel, "name": name, "kind": "law", "db": db.word(), "id": id, "asof": asof, "revision": revision, "borrowed": null,
                    "pins": articles.iter().map(|a| json!({"fragment": a.fragment, "sha256": a.pin})).collect::<Vec<_>>(),
                }),
                Resolved::File { name: n, url, pin, .. } => json!({
                    "file": f.rel, "name": name, "kind": "file", "path": n.path, "url": url, "sha256": pin, "borrowed": null,
                }),
                Resolved::Borrowed { name: n } => json!({"file": f.rel, "name": name, "kind": "borrowed", "borrowed": n.to_json()}),
                Resolved::Broken => json!({"file": f.rel, "name": name, "kind": null, "borrowed": null}),
            });
        }
    }
    let mut requirements = Vec::new();
    for r in 0..p.reqs.len() {
        let d = p.decl(r);
        let v = &p.reqs[r];
        let mut from = Vec::new();
        for (i, f) in d.from.iter().enumerate() {
            let st = check::state(m, r, LinkKind::From(i));
            let (srcs, req) = match &f.what {
                FromWhat::Cite { source, fragments, .. } => {
                    let ends = st.and_then(|s| s.up.clone()).unwrap_or_default();
                    let list: Vec<Value> = if fragments.is_empty() {
                        vec![json!({"source": source, "fragment": null, "sha256": ends.first().map(|e| e.end.hash.clone())})]
                    } else {
                        fragments.iter().map(|(fr, _)| {
                            let h = ends.iter().find(|e| matches!(&e.thing, Thing::Source { fragment, .. } if fragment == fr)).map(|e| e.end.hash.clone());
                            json!({"source": source, "fragment": fr, "sha256": h})
                        }).collect()
                    };
                    (list, Value::Null)
                }
                FromWhat::Req(_) => {
                    let t = p.names.from[r][i];
                    (vec![], t.map(|t| json!({"name": p.reqs[t].name, "version": p.reqs[t].version, "sha256": m.req_ends[t].as_ref().map(|e| e.hash.clone())})).unwrap_or(Value::Null))
                }
            };
            from.push(json!({"line": f.span.line, "sources": srcs, "requirement": req, "reviewed": reviewed(f.record.as_ref()), "status": st.map(|s| s.status.word())}));
        }
        let links: Vec<Value> = d
            .links
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let st = check::state(m, r, LinkKind::To(i));
                json!({
                    "line": l.span.line, "role": l.side.word(), "artifact": p.names.links[r][i].as_ref().map(|n| n.to_json()),
                    "sha256": st.and_then(|s| s.down.as_ref()).map(|e| e.end.hash.clone()),
                    "reviewed": reviewed(l.record.as_ref()), "status": st.map(|s| s.status.word()),
                })
            })
            .collect();
        let waivers: Vec<Value> = d
            .waivers
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let st = check::state(m, r, LinkKind::Waiver(i));
                json!({"line": w.span.line, "role": w.side.word(), "why": w.why, "approved": approved(w.record.as_ref()), "status": st.map(|s| s.status.word())})
            })
            .collect();
        requirements.push(json!({
            "name": v.name,
            "alias": d.alias.as_ref().map(|a| a.0.clone()),
            "version": v.version,
            "file": p.files[v.file].rel,
            "line": d.span.line,
            "text": d.text.as_ref().map(|t| t.0.clone()),
            "in_force": d.in_force.map(|(per, _)| json!({"from": per.from.map(|x| x.to_string()), "to": per.to.map(|x| x.to_string())})),
            "owner": d.owner.as_ref().map(|o| o.0.clone()),
            "replaces": p.names.replaces[r].iter().flatten().map(|t| json!({"name": p.reqs[*t].name, "version": p.reqs[*t].version})).collect::<Vec<_>>(),
            "sha256": m.req_ends[r].as_ref().map(|e| e.hash.clone()),
            "from": from,
            "decided": d.decided.iter().map(|dc| json!({"date": dc.date.to_string(), "by": dc.by.0, "why": dc.why})).collect::<Vec<_>>(),
            "links": links,
            "waivers": waivers,
        }));
    }
    let artifacts: Vec<Value> = m
        .artifacts
        .iter()
        .map(|(n, e)| {
            let mut j = n.to_json();
            let o = j.as_object_mut().unwrap();
            o.insert("sha256".into(), json!(e.as_ref().ok().map(|e| e.hash.clone())));
            o.insert("end".into(), json!(if n.items.is_empty() { "file" } else { "item" }));
            o.insert("pins".into(), json!([]));
            j
        })
        .collect();
    let scopes: Vec<Value> = m
        .scopes
        .iter()
        .map(|s| json!({"file": p.files[s.file].rel, "line": p.files[s.file].ast.scopes[s.idx].span.line, "text": s.text, "artifacts": s.artifacts.len(), "untraced": s.untraced.iter().map(|n| n.to_json()).collect::<Vec<_>>()}))
        .collect();
    Some(json!({
        "yuen": VERSION,
        "root": p.root_shown,
        "files": files,
        "roles": roles,
        "sources": sources,
        "requirements": requirements,
        "artifacts": artifacts,
        "scopes": scopes,
        "check": check::to_json(c, label, lang).as_object().map(|o| {
            let mut x = o.clone();
            // `shift_remove` keeps the order DESIGN 11 gives (`ok`, `summary`, `diagnostics`).
            x.shift_remove("root");
            Value::Object(x)
        }).unwrap_or(Value::Null),
    }))
}
