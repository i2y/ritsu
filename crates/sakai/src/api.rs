//! `sakai api` (DESIGN 9): the map, who owns each artifact, and the references that cross a
//! boundary, as JSON for other tools. yuen reads which context an artifact belongs to and who
//! owns it; a future dandori could read whether its references follow the map. Names are in the
//! form of DESIGN 2.6, paths from the root, and the keys come in the order written here.

use crate::proto::Naming;
use crate::diag::value;
use crate::ast::{Element, Role, Target, ValueTo};
use crate::check::Checked;
use crate::elements::At;
use crate::model::{Model, Own, RelK};
use crate::naming::{Name, Tool};
use crate::paths;
use crate::refs::Allowed;
use ritsu_base::sha256;
use serde_json::{Value, json};

fn own(o: &Own) -> Value {
    match (o.tool, o.contract) {
        (None, _) => json!({"dir": o.path}),
        (Some(t), None) => json!({"name": value(&Name::file(t, o.path.clone()).to_json())}),
        // an OpenAPI or AsyncAPI document: a file, and its kind (DESIGN 15.7)
        (Some(t), Some(k)) => json!({"name": value(&Name::file(t, o.path.clone()).to_json()), "contract": k.word()}),
    }
}

/// An element the map names: a name of DESIGN 2.6, or an element of an OpenAPI or AsyncAPI
/// document as its file and JSON Pointer (`{"pointer": "payments/api.yaml#/components/schemas/Charge"}`,
/// with the value of an enum beside it), since a document's elements have no name of DESIGN 2
/// yet (DESIGN 15.7, 15.10).
fn element(n: &Name) -> Value {
    match crate::elements::as_contract(n) {
        Some((f, p)) => {
            let mut v = json!({"pointer": crate::contracts::shown(f, p)});
            if let Some((_, value)) = n.items.get(1) {
                v["value"] = json!(value);
            }
            v
        }
        None => value(&n.to_json()),
    }
}

fn at(file: &str, line: usize) -> String {
    format!("{file}:{line}")
}

fn digest(m: &Model, p: &str) -> String {
    sha256::hex(&ritsu_base::fs::read(paths::on_disk(&m.root, p)).unwrap_or_default())
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
                    let mut from: Vec<Value> = p.protos.iter().map(|(f, _)| value(&Name::file(Tool::Proto, f.clone()).to_json())).collect();
                    if let Some((f, _)) = &p.rulec {
                        from.push(value(&Name::file(Tool::Rulec, f.clone()).to_json()));
                    }
                    if let Some((d, _)) = &p.krate {
                        from.push(value(&Name::file(Tool::File, crate::cargo::manifest_of(d)).to_json()));
                    }
                    from.extend(p.contracts.iter().map(|(_, f, _)| value(&Name::file(Tool::File, f.clone()).to_json())));
                    let mut v = json!({
                        "package": p.package,
                        "from": from,
                        "services": p.services.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
                        "generated": p.generated.iter().map(|(g, _)| g.clone()).collect::<Vec<_>>(),
                    });
                    // the OpenAPI and AsyncAPI documents: their kind, and the versions they say (DESIGN 15.7)
                    if !p.contracts.is_empty() {
                        v["contracts"] = json!(p.contracts.iter().map(|(k, f, _)| {
                            let d = c.contracts.docs.get(f);
                            json!({
                                "file": f,
                                "kind": k.word(),
                                "spec": d.map(|d| d.spec.clone()),
                                "title": d.map(|d| d.title.clone()),
                                "version": d.map(|d| d.version.clone()),
                            })
                        }).collect::<Vec<_>>());
                    }
                    v
                }).collect::<Vec<_>>(),
                "terms": a.terms.iter().enumerate().map(|(ti, t)| json!({
                    "name": t.name,
                    "definition": t.definition.as_ref().map(|s| s.value.clone()),
                    "also": t.also.iter().map(|s| s.value.clone()).collect::<Vec<_>>(),
                    "means": (0..t.means.len()).filter_map(|mi| c.elements.get(At::Means(ci, ti, mi))).map(element).collect::<Vec<_>>(),
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
                "name": value(&a.name().to_json()),
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
            let mut v = json!({
                "from": value(&cr.from_name().to_json()),
                "line": cr.line,
                "to": value(&cr.to_name().to_json()),
                "from_context": x.name,
                "to_context": m.contexts[cr.to_ctx].name,
                "via": cr.kind.via(),
                "elements": cr.reach.iter().map(|s| value(&s.naming().to_json())).collect::<Vec<_>>(),
                "allowed_by": allowed,
            });
            // a document's `$ref`: where it lands and what it reaches, as files and JSON Pointers
            if let Some(p) = &cr.pointer {
                v["pointer"] = json!(crate::contracts::shown(&cr.to, p));
                v["pointers"] = json!(cr.elements.iter().map(|(f, p)| crate::contracts::shown(f, p)).collect::<Vec<_>>());
            }
            v
        }).collect::<Vec<_>>(),
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
                            let from = c.elements.get(At::From(ci, ri, ei)).map(element).unwrap_or(Value::Null);
                            let (to, checked) = match &em.target {
                                Target::Name(n, _) => (json!({"name": n}), false),
                                Target::Element(e) => {
                                    let n = c.elements.get(At::To(ci, ri, ei));
                                    let checked = n.is_some_and(|n| n.tool == Tool::Proto || crate::elements::as_contract(n).is_some());
                                    let v = match (n, e) {
                                        (Some(n), _) => element(n),
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
