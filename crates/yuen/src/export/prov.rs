//! W3C PROV (DESIGN 13, PLAN C.11): the provenance of a project, in PROV-N (the default, the
//! Recommendation a person reads) or PROV-JSON (`--format json`, what tools read). The two are
//! the same document: both are written from one list of records.
//!
//! A requirement version, an article of a source and an artifact are entities; a role is an
//! agent; a decision, a look at a link and a waiver are activities. A requirement comes from
//! its articles (`wasDerivedFrom` with `prov:type='prov:PrimarySource'`; PROV-JSON has no
//! `hadPrimarySource`), from the requirements it is read from, and from its versions before
//! (`prov:Revision`); its owner is an attribution; a decision influences it; an artifact that
//! meets it or checks it is influenced by it.

use super::{Graph, PinFile, PinSource, RelKind, SourceKey, SourceNode, Target, digits};
use serde_json::{Map, Value, json};

/// yuen's types and attributes. A word's IRI is this and the word (`…/ns/yuen#Requirement`), and
/// opening it leads to the word's entry on ritsu's site: `website/docs/ns/yuen.md` (in Japanese,
/// `website/docs-ja/ns/yuen.md`), published at `https://i2y.github.io/ritsu/ns/yuen/`. The words
/// are [`TERMS`]; ritsu's `tests/website.rs` holds the pages to the list, and yuen's
/// `tests/export.rs` holds the list to what the export writes.
pub const YUEN_NS: &str = "https://i2y.github.io/ritsu/ns/yuen#";
/// The identifiers of the things of a project.
pub const ID_NS: &str = "urn:yuen:";

/// What a word of [`YUEN_NS`] is to PROV.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The type of a record: the value of `prov:type` of an `entity`, an `agent` or an `activity`.
    Type,
    /// The sort of a relation: the value of `prov:type` of a `wasAttributedTo` or a `wasInfluencedBy`.
    Relation,
    /// An attribute of a record, written with a string value.
    Attribute,
}

/// One word of [`YUEN_NS`], and where it is written.
#[derive(Clone, Copy, Debug)]
pub struct Term {
    /// The word, as it follows `yuen:`.
    pub name: &'static str,
    pub kind: Kind,
    /// Where it is written. A type: the PROV record it is the type of (`entity`). A relation: the
    /// PROV record it is the sort of (`wasInfluencedBy`). An attribute: the types of the records
    /// that carry it.
    pub on: &'static [&'static str],
}

/// Every word the export writes in [`YUEN_NS`], in the order the page of the namespace gives them.
pub const TERMS: &[Term] = &[
    Term { name: "Requirement", kind: Kind::Type, on: &["entity"] },
    Term { name: "Source", kind: Kind::Type, on: &["entity"] },
    Term { name: "Artifact", kind: Kind::Type, on: &["entity"] },
    Term { name: "Role", kind: Kind::Type, on: &["agent"] },
    Term { name: "Decision", kind: Kind::Type, on: &["activity"] },
    Term { name: "Review", kind: Kind::Type, on: &["activity"] },
    Term { name: "Waiver", kind: Kind::Type, on: &["activity"] },
    Term { name: "owner", kind: Kind::Relation, on: &["wasAttributedTo"] },
    Term { name: "satisfies", kind: Kind::Relation, on: &["wasInfluencedBy"] },
    Term { name: "verifies", kind: Kind::Relation, on: &["wasInfluencedBy"] },
    Term { name: "pins", kind: Kind::Relation, on: &["wasInfluencedBy"] },
    Term { name: "version", kind: Kind::Attribute, on: &["Requirement"] },
    Term { name: "text", kind: Kind::Attribute, on: &["Requirement"] },
    Term { name: "inForce", kind: Kind::Attribute, on: &["Requirement"] },
    Term { name: "law", kind: Kind::Attribute, on: &["Source"] },
    Term { name: "asof", kind: Kind::Attribute, on: &["Source"] },
    Term { name: "revision", kind: Kind::Attribute, on: &["Source"] },
    Term { name: "file", kind: Kind::Attribute, on: &["Source"] },
    Term { name: "url", kind: Kind::Attribute, on: &["Source"] },
    Term { name: "end", kind: Kind::Attribute, on: &["Artifact"] },
    Term { name: "description", kind: Kind::Attribute, on: &["Role"] },
    Term { name: "why", kind: Kind::Attribute, on: &["Decision", "Waiver"] },
    Term { name: "side", kind: Kind::Attribute, on: &["Waiver"] },
    Term { name: "link", kind: Kind::Attribute, on: &["Review"] },
    Term { name: "up", kind: Kind::Attribute, on: &["Review"] },
    Term { name: "down", kind: Kind::Attribute, on: &["Review"] },
    Term { name: "status", kind: Kind::Attribute, on: &["Review", "Waiver"] },
    Term { name: "sha256", kind: Kind::Attribute, on: &["Requirement", "Source", "Artifact", "Waiver"] },
];

/// An attribute's value: a string, or a qualified name (`'yuen:Requirement'`).
#[derive(Clone)]
enum Val {
    Str(String),
    QName(&'static str),
}

/// One record. `args` are what PROV-N writes between the parentheses before the attributes
/// (identifiers and times, `None` for `-`).
struct Rec {
    kind: &'static str,
    args: Vec<Option<String>>,
    attrs: Vec<(&'static str, Val)>,
}

/// Whether a name can be the local part of a qualified name as it is: what PROV-N's
/// `PN_CHARS_BASE`, digits and `_` allow. Anything else is named by a hash.
fn local_ok(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            let u = c as u32;
            c.is_ascii_alphanumeric()
                || c == '_'
                || matches!(u, 0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0x2FF | 0x370..=0x37D | 0x37F..=0x1FFF | 0x200C..=0x200D | 0x2070..=0x218F | 0x2C00..=0x2FEF | 0x3001..=0xD7FF | 0xF900..=0xFDCF | 0xFDF0..=0xFFFD | 0x10000..=0xEFFFF)
        })
}

fn role_id(name: &str) -> String {
    if local_ok(name) { format!("y:role/{name}") } else { format!("y:role/{}", digits("role", &[name])) }
}

struct Ids {
    reqs: Vec<String>,
    sources: Vec<String>,
    artifacts: Vec<String>,
}

fn time(d: crate::date::Day) -> String {
    format!("{d}T00:00:00")
}

/// The records of the project, in the order both forms write them.
fn records(g: &Graph) -> Vec<Rec> {
    let p = g.p;
    let ids = Ids {
        reqs: (0..p.reqs.len()).map(|r| format!("y:requirement/{}/v{}", g.alias(r), p.reqs[r].version)).collect(),
        sources: g.sources.iter().map(|s| format!("y:source/{}", s.key.digits())).collect(),
        artifacts: g.artifacts.iter().map(|a| format!("y:artifact/{}", digits("artifact", &[&a.text()]))).collect(),
    };
    let target = |t: Target| -> String {
        match t {
            Target::Source(i) => ids.sources[i].clone(),
            Target::Req(i) => ids.reqs[i].clone(),
            Target::Artifact(i) => ids.artifacts[i].clone(),
        }
    };
    let mut out: Vec<Rec> = Vec::new();
    // Agents: the roles.
    for f in &p.files {
        for r in &f.ast.roles {
            let mut attrs = vec![("prov:type", Val::QName("yuen:Role")), ("prov:label", Val::Str(r.name.clone()))];
            if let Some(d) = &r.description {
                attrs.push(("yuen:description", Val::Str(d.clone())));
            }
            out.push(Rec { kind: "agent", args: vec![Some(role_id(&r.name))], attrs });
        }
    }
    // Entities: the sources, the artifacts, the requirement versions. The articles the files of
    // the artifacts pin and no `.req` declares come after the sources, and those files, when no
    // link names them whole, after the artifacts.
    let more_sources: Vec<String> = g.pins.sources.iter().map(|s| format!("y:source/{}", s.key.digits())).collect();
    let pin_files: Vec<String> = g.pins.files.iter().map(|(f, _)| format!("y:artifact/{}", digits("artifact", &[&f.text()]))).collect();
    let all_sources: Vec<(&SourceNode, &String)> = g.sources.iter().zip(&ids.sources).chain(g.pins.sources.iter().zip(&more_sources)).collect();
    for (s, sid) in all_sources {
        let mut attrs = vec![("prov:type", Val::QName("yuen:Source")), ("prov:label", Val::Str(s.label.clone()))];
        match &s.key {
            SourceKey::Law { db, id, asof, .. } => {
                attrs.push(("yuen:law", Val::Str(format!("{} {id}", db.word()))));
                attrs.push(("yuen:asof", Val::Str(asof.clone())));
                if let Some(rv) = &s.revision {
                    attrs.push(("yuen:revision", Val::Str(rv.clone())));
                }
            }
            SourceKey::File { path } => {
                attrs.push(("yuen:file", Val::Str(path.clone())));
                if let Some(u) = &s.url {
                    attrs.push(("yuen:url", Val::Str(u.clone())));
                }
            }
        }
        if let Some(pin) = &s.pin {
            attrs.push(("yuen:sha256", Val::Str(pin.clone())));
        }
        out.push(Rec { kind: "entity", args: vec![Some(sid.clone())], attrs });
    }
    for (i, a) in g.artifacts.iter().enumerate() {
        let mut attrs = vec![("prov:type", Val::QName("yuen:Artifact")), ("prov:label", Val::Str(a.text()))];
        if let Some(Ok(e)) = g.m.artifacts.get(a) {
            attrs.push(("yuen:sha256", Val::Str(e.hash.clone())));
        }
        attrs.push(("yuen:end", Val::Str(if a.items.is_empty() { "file".into() } else { "item".into() })));
        out.push(Rec { kind: "entity", args: vec![Some(ids.artifacts[i].clone())], attrs });
    }
    for ((f, hash), fid) in g.pins.files.iter().zip(&pin_files) {
        let attrs = vec![("prov:type", Val::QName("yuen:Artifact")), ("prov:label", Val::Str(f.text())), ("yuen:sha256", Val::Str(hash.clone())), ("yuen:end", Val::Str("file".into()))];
        out.push(Rec { kind: "entity", args: vec![Some(fid.clone())], attrs });
    }
    for r in 0..p.reqs.len() {
        let d = p.decl(r);
        let mut attrs = vec![
            ("prov:type", Val::QName("yuen:Requirement")),
            ("prov:label", Val::Str(p.reqs[r].name.clone())),
            ("yuen:version", Val::Str(p.reqs[r].version.to_string())),
        ];
        if let Some((t, _)) = &d.text {
            attrs.push(("yuen:text", Val::Str(t.clone())));
        }
        if let Some((per, _)) = &d.in_force {
            attrs.push(("yuen:inForce", Val::Str(per.to_string())));
        }
        if let Some(e) = &g.m.req_ends[r] {
            attrs.push(("yuen:sha256", Val::Str(e.hash.clone())));
        }
        out.push(Rec { kind: "entity", args: vec![Some(ids.reqs[r].clone())], attrs });
    }
    // What each requirement version comes from, who owns it, who decided what, what meets it
    // and checks it, and who looked.
    for r in 0..p.reqs.len() {
        let d = p.decl(r);
        let me = ids.reqs[r].clone();
        if let Some((o, _)) = &d.owner {
            out.push(Rec { kind: "wasAttributedTo", args: vec![Some(me.clone()), Some(role_id(o))], attrs: vec![("prov:type", Val::QName("yuen:owner"))] });
        }
        let mine: Vec<&super::Rel> = g.rels.iter().filter(|x| x.req == r).collect();
        for rel in mine.iter().filter(|x| x.kind == RelKind::From) {
            let attrs = match rel.target {
                Target::Source(_) => vec![("prov:type", Val::QName("prov:PrimarySource"))],
                _ => vec![],
            };
            out.push(Rec { kind: "wasDerivedFrom", args: vec![Some(me.clone()), Some(target(rel.target))], attrs });
        }
        if let Some(prev) = g.previous_version(r) {
            out.push(Rec { kind: "wasDerivedFrom", args: vec![Some(me.clone()), Some(ids.reqs[prev].clone())], attrs: vec![("prov:type", Val::QName("prov:Revision"))] });
        }
        for rel in mine.iter().filter(|x| x.kind == RelKind::Replaces) {
            out.push(Rec { kind: "wasDerivedFrom", args: vec![Some(me.clone()), Some(target(rel.target))], attrs: vec![("prov:type", Val::QName("prov:Revision"))] });
        }
        // The articles and file sources the requirement cites, once each: what a decision used.
        let mut cited: Vec<String> = Vec::new();
        for rel in mine.iter().filter(|x| x.kind == RelKind::From) {
            if let Target::Source(_) = rel.target {
                let t = target(rel.target);
                if !cited.contains(&t) {
                    cited.push(t);
                }
            }
        }
        let mut seen: Vec<String> = Vec::new();
        for dc in &d.decided {
            let date = dc.date.to_string();
            let mut did = format!("y:decision/{}", digits("decision", &[&p.reqs[r].name, &p.reqs[r].version.to_string(), &date, &dc.by.0, &dc.why]));
            let mut n = 1;
            while seen.contains(&did) {
                n += 1;
                did = format!("y:decision/{}", digits("decision", &[&p.reqs[r].name, &p.reqs[r].version.to_string(), &date, &dc.by.0, &dc.why, &n.to_string()]));
            }
            seen.push(did.clone());
            out.push(Rec { kind: "activity", args: vec![Some(did.clone()), Some(time(dc.date)), None], attrs: vec![("prov:type", Val::QName("yuen:Decision")), ("yuen:why", Val::Str(dc.why.clone()))] });
            out.push(Rec { kind: "wasAssociatedWith", args: vec![Some(did.clone()), Some(role_id(&dc.by.0)), None], attrs: vec![] });
            out.push(Rec { kind: "wasInfluencedBy", args: vec![Some(me.clone()), Some(did.clone())], attrs: vec![] });
            for c in &cited {
                out.push(Rec { kind: "used", args: vec![Some(did.clone()), Some(c.clone()), None], attrs: vec![] });
            }
        }
        for rel in mine.iter().filter(|x| matches!(x.kind, RelKind::Satisfied | RelKind::Verified)) {
            let ty = if rel.kind == RelKind::Satisfied { "yuen:satisfies" } else { "yuen:verifies" };
            out.push(Rec { kind: "wasInfluencedBy", args: vec![Some(target(rel.target)), Some(me.clone())], attrs: vec![("prov:type", Val::QName(ty))] });
        }
        // A look at a link: one per `from` line and per link that has a record.
        let mut looked: Vec<usize> = Vec::new();
        for rel in mine.iter().filter(|x| x.kind != RelKind::Replaces) {
            let Some(si) = rel.state else { continue };
            if looked.contains(&si) {
                continue;
            }
            looked.push(si);
            let st = &g.m.states[si];
            let Some(rc) = g.record(st) else { continue };
            // The ends of the link: for `from`, every article (or the requirement) the line
            // names, then this requirement; for a link, this requirement, then the artifact.
            let ups: Vec<String> = match rel.kind {
                RelKind::From => mine.iter().filter(|x| x.state == Some(si)).map(|x| target(x.target)).collect(),
                _ => vec![me.clone()],
            };
            let down = if rel.kind == RelKind::From { me.clone() } else { target(rel.target) };
            // A look is an event: the link, who looked, when, and the hashes they saw. Looking
            // again is another look, with another identifier.
            let (date, recorded) = (rc.date.to_string(), format!("{} -> {}", rc.up.join(", "), rc.down.clone().unwrap_or_default()));
            let mut rid = format!("y:review/{}", digits("review", &[rel.kind.words(), &ups.join(" "), &down, &date, &rc.by, &recorded]));
            let mut n = 1;
            while seen.contains(&rid) {
                n += 1;
                rid = format!("y:review/{}", digits("review", &[rel.kind.words(), &ups.join(" "), &down, &date, &rc.by, &recorded, &n.to_string()]));
            }
            seen.push(rid.clone());
            let mut attrs = vec![("prov:type", Val::QName("yuen:Review")), ("yuen:link", Val::Str(rel.kind.words().to_string())), ("yuen:up", Val::Str(rc.up.join(" ")))];
            if let Some(h) = &rc.down {
                attrs.push(("yuen:down", Val::Str(h.clone())));
            }
            attrs.push(("yuen:status", Val::Str(st.status.word().to_string())));
            out.push(Rec { kind: "activity", args: vec![Some(rid.clone()), Some(time(rc.date)), None], attrs });
            for u in &ups {
                out.push(Rec { kind: "used", args: vec![Some(rid.clone()), Some(u.clone()), None], attrs: vec![] });
            }
            out.push(Rec { kind: "used", args: vec![Some(rid.clone()), Some(down), None], attrs: vec![] });
            out.push(Rec { kind: "wasAssociatedWith", args: vec![Some(rid), Some(role_id(&rc.by)), None], attrs: vec![] });
        }
        for (i, wv) in d.waivers.iter().enumerate() {
            let side = format!("not {}", wv.side.word());
            let st = g.m.states.iter().find(|s| s.req == r && s.kind == crate::marks::LinkKind::Waiver(i));
            let rc = st.and_then(|s| g.record(s));
            // An approval, like a look, is an event: approving again is another one.
            let approval = rc.map(|rc| format!("{} {} {}", rc.date, rc.by, rc.up.join(", "))).unwrap_or_default();
            let mut wid = format!("y:waiver/{}", digits("waiver", &[&p.reqs[r].name, &p.reqs[r].version.to_string(), &side, &wv.why, &approval]));
            let mut n = 1;
            while seen.contains(&wid) {
                n += 1;
                wid = format!("y:waiver/{}", digits("waiver", &[&p.reqs[r].name, &p.reqs[r].version.to_string(), &side, &wv.why, &approval, &n.to_string()]));
            }
            seen.push(wid.clone());
            let mut attrs = vec![("prov:type", Val::QName("yuen:Waiver")), ("yuen:side", Val::Str(side)), ("yuen:why", Val::Str(wv.why.clone()))];
            if let Some(h) = rc.and_then(|r| r.up.first()) {
                attrs.push(("yuen:sha256", Val::Str(h.clone())));
            }
            if let Some(s) = st {
                attrs.push(("yuen:status", Val::Str(s.status.word().to_string())));
            }
            out.push(Rec { kind: "activity", args: vec![Some(wid.clone()), rc.map(|r| time(r.date)), None], attrs });
            out.push(Rec { kind: "used", args: vec![Some(wid.clone()), Some(me.clone()), None], attrs: vec![] });
            if let Some(rc) = rc {
                out.push(Rec { kind: "wasAssociatedWith", args: vec![Some(wid), Some(role_id(&rc.by)), None], attrs: vec![] });
            }
        }
    }
    // What the files of the rules and calendars pin (DESIGN 3.3).
    for (from, to) in &g.pins.edges {
        let f = match from {
            PinFile::Artifact(i) => ids.artifacts[*i].clone(),
            PinFile::File(i) => pin_files[*i].clone(),
        };
        let t = match to {
            PinSource::Source(i) => ids.sources[*i].clone(),
            PinSource::More(i) => more_sources[*i].clone(),
        };
        out.push(Rec { kind: "wasInfluencedBy", args: vec![Some(f), Some(t)], attrs: vec![("prov:type", Val::QName("yuen:pins"))] });
    }
    out
}

/// A string as PROV-N writes it: in double quotes, `"` and `\` and the line breaks escaped.
fn provn_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn provn_attrs(attrs: &[(&'static str, Val)]) -> String {
    let parts: Vec<String> = attrs
        .iter()
        .map(|(k, v)| match v {
            Val::Str(s) => format!("{k}={}", provn_string(s)),
            Val::QName(q) => format!("{k}='{q}'"),
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

/// The document in PROV-N.
pub fn provn(g: &Graph) -> String {
    let mut o = String::from("document\n");
    o.push_str(&format!("  prefix yuen <{YUEN_NS}>\n"));
    o.push_str(&format!("  prefix y <{ID_NS}>\n\n"));
    for r in records(g) {
        let mut args: Vec<String> = r.args.iter().map(|a| a.clone().unwrap_or_else(|| "-".into())).collect();
        // `wasDerivedFrom` and `wasInfluencedBy` and `wasAttributedTo` take the attributes after
        // their optional arguments; the short forms leave those out when nothing follows.
        if r.kind == "wasDerivedFrom" && !r.attrs.is_empty() {
            args.extend(["-".to_string(), "-".to_string(), "-".to_string()]);
        }
        if !r.attrs.is_empty() {
            args.push(provn_attrs(&r.attrs));
        }
        o.push_str(&format!("  {}({})\n", r.kind, args.join(", ")));
    }
    o.push_str("endDocument\n");
    o
}

fn json_attrs(m: &mut Map<String, Value>, attrs: &[(&'static str, Val)]) {
    for (k, v) in attrs {
        let v = match v {
            Val::Str(s) => json!(s),
            Val::QName(q) => json!({"$": q, "type": "prov:QUALIFIED_NAME"}),
        };
        m.insert((*k).to_string(), v);
    }
}

/// The document in PROV-JSON.
pub fn provjson(g: &Graph) -> String {
    let mut groups: Map<String, Value> = Map::new();
    groups.insert("prefix".into(), json!({"yuen": YUEN_NS, "y": ID_NS}));
    let mut n = 0;
    for r in records(g) {
        let mut m = Map::new();
        let key = match r.kind {
            "entity" | "agent" => {
                json_attrs(&mut m, &r.attrs);
                r.args[0].clone().unwrap_or_default()
            }
            "activity" => {
                if let Some(t) = &r.args[1] {
                    m.insert("prov:startTime".into(), json!(t));
                }
                if let Some(t) = &r.args[2] {
                    m.insert("prov:endTime".into(), json!(t));
                }
                json_attrs(&mut m, &r.attrs);
                r.args[0].clone().unwrap_or_default()
            }
            kind => {
                let names: [&str; 2] = match kind {
                    "wasDerivedFrom" => ["prov:generatedEntity", "prov:usedEntity"],
                    "wasAttributedTo" => ["prov:entity", "prov:agent"],
                    "wasInfluencedBy" => ["prov:influencee", "prov:influencer"],
                    "wasAssociatedWith" => ["prov:activity", "prov:agent"],
                    "used" => ["prov:activity", "prov:entity"],
                    other => unreachable!("{other} is written above"),
                };
                for (k, a) in names.iter().zip(&r.args) {
                    if let Some(a) = a {
                        m.insert((*k).to_string(), json!(a));
                    }
                }
                json_attrs(&mut m, &r.attrs);
                n += 1;
                format!("_:r{n}")
            }
        };
        let group = groups.entry(r.kind.to_string()).or_insert_with(|| Value::Object(Map::new()));
        group.as_object_mut().unwrap().insert(key, Value::Object(m));
    }
    let mut s = serde_json::to_string_pretty(&Value::Object(groups)).unwrap();
    s.push('\n');
    s
}
