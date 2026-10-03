//! `sakai api` (DESIGN 9): the map, who owns each artifact, and the references that cross a
//! boundary, as JSON for other tools. yurai reads which context an artifact belongs to and who
//! owns it; a future dandori could read whether its references follow the map. Names are in the
//! form of DESIGN 2.6, paths from the root, and the keys come in the order written here.

use crate::ast::{Element, Role, Target, ValueTo};
use crate::check::Checked;
use crate::elements::At;
use crate::model::{Model, Own, RelK};
use crate::naming::{Name, Tool};
use crate::paths;
use crate::refs::Allowed;
use crate::sha256;
use serde_json::{Value, json};

fn own(o: &Own) -> Value {
    match o.tool {
        None => json!({"dir": o.path}),
        Some(t) => json!({"name": Name::file(t, o.path.clone()).to_json()}),
    }
}

fn at(file: &str, line: usize) -> String {
    format!("{file}:{line}")
}

fn digest(m: &Model, p: &str) -> String {
    sha256::hex(&std::fs::read(paths::on_disk(&m.root, p)).unwrap_or_default())
}

pub fn api(c: &Checked) -> Value {
    let m = &c.model;
    let h = &m.map.ast.heading;
    let contexts: Vec<Value> = m
        .contexts
        .iter()
        .enumerate()
        .map(|(ci, x)| {
            let a = &x.ast;
            json!({
                "name": x.name,
                "alias": x.alias,
                "version": a.heading.version,
                "file": x.file,
                "source_sha256": sha256::hex(x.src.as_bytes()),
                "description": a.description.as_ref().map(|s| s.value.clone()),
                "owner": a.owner.as_ref().map(|s| s.value.clone()),
                "also": a.also.iter().map(|s| s.value.clone()).collect::<Vec<_>>(),
                "owns": x.owns.iter().map(own).collect::<Vec<_>>(),
                "published": x.published.iter().map(|p| {
                    let mut from: Vec<Value> = p.protos.iter().map(|(f, _)| Name::file(Tool::Proto, f.clone()).to_json()).collect();
                    if let Some((f, _)) = &p.rulec {
                        from.push(Name::file(Tool::Rulec, f.clone()).to_json());
                    }
                    json!({
                        "package": p.package,
                        "from": from,
                        "services": p.services.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
                        "generated": p.generated.iter().map(|(g, _)| g.clone()).collect::<Vec<_>>(),
                    })
                }).collect::<Vec<_>>(),
                "terms": a.terms.iter().enumerate().map(|(ti, t)| json!({
                    "name": t.name,
                    "definition": t.definition.as_ref().map(|s| s.value.clone()),
                    "also": t.also.iter().map(|s| s.value.clone()).collect::<Vec<_>>(),
                    "means": (0..t.means.len()).filter_map(|mi| c.elements.get(At::Means(ci, ti, mi))).map(Name::to_json).collect::<Vec<_>>(),
                    "as": t.as_term.as_ref().map(|x| json!({"context": x.context, "term": x.term})),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "sakai": env!("CARGO_PKG_VERSION"),
        "map": {
            "name": h.name,
            "alias": h.alias,
            "version": h.version,
            "file": m.map.file,
            "source_sha256": sha256::hex(m.map.src.as_bytes()),
            "description": m.map.ast.description.as_ref().map(|s| s.value.clone()),
        },
        "covers": m.map.covers,
        "except": m.map.except,
        "contexts": contexts,
        "relationships": relationships(c),
        "artifacts": c.artifacts.iter().map(|a| {
            let (ci, oi) = a.owner.expect("api is printed for a map whose artifacts all have an owner");
            let x = &m.contexts[ci];
            json!({
                "name": a.name().to_json(),
                "context": x.name,
                "by": at(&x.file, x.owns[oi].pos.line),
                "sha256": digest(m, &a.path)[..16].to_string(),
            })
        }).collect::<Vec<_>>(),
        "crossings": c.crossings.iter().map(|cr| {
            let x = &m.contexts[cr.from_ctx];
            let allowed = match &cr.allowed {
                Some(Allowed::Kernel(k)) => {
                    let r = m.rels(*k, if *k == cr.from_ctx { cr.to_ctx } else { cr.from_ctx }).find(|r| matches!(r.kind, RelK::Kernel(_))).unwrap();
                    json!({"relationship": "shared_kernel", "declared": at(&m.contexts[*k].file, r.pos.line)})
                }
                Some(Allowed::Upstream(ri)) => {
                    let r = &x.rels[*ri];
                    json!({"relationship": "upstream_downstream", "roles": r.roles().iter().map(|r| r.key()).collect::<Vec<_>>(), "declared": at(&x.file, r.pos.line)})
                }
                Some(Allowed::Partnership) => {
                    let r = m.rels(cr.from_ctx, cr.to_ctx).find(|r| matches!(r.kind, RelK::Partnership)).unwrap();
                    json!({"relationship": "partnership", "declared": at(&x.file, r.pos.line)})
                }
                None => Value::Null,
            };
            json!({
                "from": cr.from_name().to_json(),
                "line": cr.line,
                "to": cr.to_name().to_json(),
                "from_context": x.name,
                "to_context": m.contexts[cr.to_ctx].name,
                "via": "proto import",
                "elements": cr.reach.iter().map(|s| s.naming().to_json()).collect::<Vec<_>>(),
                "allowed_by": allowed,
            })
        }).collect::<Vec<_>>(),
        "not_checked": Vec::<Value>::new(),
    })
}

fn relationships(c: &Checked) -> Vec<Value> {
    let m = &c.model;
    let mut out = Vec::new();
    let mut pairs: Vec<(usize, usize, &'static str)> = Vec::new();
    for (ci, x) in m.contexts.iter().enumerate() {
        for (ri, r) in x.rels.iter().enumerate() {
            let y = &m.contexts[r.partner];
            match &r.kind {
                RelK::Upstream { roles, through, layer, enums, terms } => {
                    let mut declared = vec![at(&x.file, r.pos.line)];
                    let mut up_roles: Vec<&str> = Vec::new();
                    if roles.iter().any(|(x, _)| *x == Role::Customer)
                        && let Some(s) = y.rels.iter().find(|s| s.partner == ci && matches!(s.kind, RelK::Downstream))
                    {
                        up_roles.push("supplier");
                        declared.push(at(&y.file, s.pos.line));
                    }
                    let ohs = y.published.iter().any(|p| through.iter().any(|(t, _)| *t == p.package) && !p.services.is_empty());
                    if ohs {
                        up_roles.push("open_host_service");
                    }
                    up_roles.push("published_language");
                    let enums: Vec<Value> = enums
                        .iter()
                        .enumerate()
                        .map(|(ei, em)| {
                            let from = c.elements.get(At::From(ci, ri, ei)).map(Name::to_json).unwrap_or(Value::Null);
                            let (to, checked) = match &em.target {
                                Target::Name(n, _) => (json!({"name": n}), false),
                                Target::Element(e) => {
                                    let n = c.elements.get(At::To(ci, ri, ei));
                                    let checked = n.is_some_and(|n| n.tool == Tool::Proto);
                                    let v = match (n, e) {
                                        (Some(n), _) => n.to_json(),
                                        (None, Element::Long { .. } | Element::Short { .. }) => Value::Null,
                                    };
                                    (v, checked)
                                }
                            };
                            json!({
                                "from": from,
                                "to": to,
                                "checked": checked,
                                "values": em.values.iter().map(|v| match &v.to {
                                    ValueTo::Value(x, _) => json!({"from": v.from, "to": x}),
                                    ValueTo::Refuse(why, _) => json!({"from": v.from, "refuse": why.as_ref().map(|s| s.value.clone())}),
                                }).collect::<Vec<_>>(),
                            })
                        })
                        .collect();
                    out.push(json!({
                        "kind": "upstream_downstream",
                        "upstream": y.name,
                        "downstream": x.name,
                        "roles": {"upstream": up_roles, "downstream": roles.iter().map(|(r, _)| r.key()).collect::<Vec<_>>()},
                        "through": through.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>(),
                        "layer": layer.iter().map(own).collect::<Vec<_>>(),
                        "enums": enums,
                        "terms": terms.iter().map(|t| json!({"from": t.from, "to": t.to})).collect::<Vec<_>>(),
                        "declared": declared,
                    }));
                }
                RelK::Downstream => {}
                k => {
                    let key = (ci.min(r.partner), ci.max(r.partner), k.words());
                    if pairs.contains(&key) {
                        continue;
                    }
                    pairs.push(key);
                    let (a, b) = (ci, r.partner);
                    let back = m.rels(b, a).find(|s| s.kind.words() == k.words());
                    let mut declared = vec![at(&x.file, r.pos.line)];
                    if let Some(s) = back {
                        declared.push(at(&y.file, s.pos.line));
                    }
                    let kind = match k {
                        RelK::Kernel(_) => "shared_kernel",
                        RelK::Partnership => "partnership",
                        _ => "separate_ways",
                    };
                    let mut v = json!({"kind": kind, "between": [x.name, y.name]});
                    if let RelK::Kernel(items) = k {
                        let mut sides = vec![json!({"context": x.name, "items": items.iter().map(own).collect::<Vec<_>>()})];
                        if let Some(RelK::Kernel(bi)) = back.map(|s| &s.kind) {
                            sides.push(json!({"context": y.name, "items": bi.iter().map(own).collect::<Vec<_>>()}));
                        }
                        v["sides"] = json!(sides);
                    }
                    v["declared"] = json!(declared);
                    out.push(v);
                }
            }
        }
    }
    out
}
