//! ReqIF (DESIGN 12, PLAN C.10): the project as one ReqIF 1.2 document, for the requirements
//! tools of companies.
//!
//! A requirement version, an article of a source and an artifact are each a `SPEC-OBJECT`; a
//! `from`, `satisfied by`, `verified by` and `replaces` is a `SPEC-RELATION`, its record and its
//! mark as attributes; each `.req` is a `SPECIFICATION`, and so are the sources and the
//! artifacts. The names of the attributes follow the ReqIF Implementor Forum's guide (v1.10):
//! `ReqIF.ForeignID`, `ReqIF.Name`, `ReqIF.Text`, and `yuen.` before yuen's own.
//!
//! Every identifier is a hash of what it names (`super::digits`), and every time is a day the
//! project wrote down, so the same `.req` files give the same bytes whenever they are written
//! out. The XML is written by hand, escaped here: yuen depends on nothing but serde_json.

use super::{Graph, RelKind, SourceKey, Target, digits};
use crate::i18n::Text;
use crate::project::Refusal;

pub const NAMESPACE: &str = "http://www.omg.org/spec/ReqIF/20110401/reqif.xsd";
const XHTML: &str = "http://www.w3.org/1999/xhtml";

/// The `MAX-LENGTH` of the string type (DESIGN 12): longer than anything yuen writes; a value
/// longer than this stops the export.
pub const MAX_LENGTH: usize = 100_000;

fn id(kind: &str, parts: &[&str]) -> String {
    format!("_{}", digits(kind, parts))
}

/// An attribute a type declares: its name, and whether it holds XHTML (else a string).
#[derive(Clone, Copy)]
struct Attr {
    name: &'static str,
    xhtml: bool,
}

const fn s(name: &'static str) -> Attr {
    Attr { name, xhtml: false }
}

const fn x(name: &'static str) -> Attr {
    Attr { name, xhtml: true }
}

const REQUIREMENT: &[Attr] = &[
    s("ReqIF.ForeignID"),
    x("ReqIF.Name"),
    x("ReqIF.Text"),
    s("yuen.inForce"),
    s("yuen.owner"),
    x("yuen.decided"),
    x("yuen.waived"),
    s("yuen.sha256"),
    s("yuen.status"),
];
const SOURCE: &[Attr] = &[
    s("ReqIF.ForeignID"),
    x("ReqIF.Name"),
    x("ReqIF.Text"),
    s("yuen.law"),
    s("yuen.asof"),
    s("yuen.revision"),
    s("yuen.file"),
    s("yuen.url"),
    s("yuen.sha256"),
];
const ARTIFACT: &[Attr] = &[s("ReqIF.ForeignID"), x("ReqIF.Name"), s("yuen.sha256"), s("yuen.end")];
const LINK: &[Attr] = &[s("yuen.reviewedOn"), s("yuen.reviewedBy"), s("yuen.up"), s("yuen.down"), s("yuen.status")];
const FILE: &[Attr] = &[s("yuen.file"), s("yuen.version"), s("yuen.description")];

/// The types: the element, the long name, the attributes.
const TYPES: &[(&str, &str, &[Attr])] = &[
    ("SPEC-OBJECT-TYPE", "yuen requirement", REQUIREMENT),
    ("SPEC-OBJECT-TYPE", "yuen source", SOURCE),
    ("SPEC-OBJECT-TYPE", "yuen artifact", ARTIFACT),
    ("SPEC-RELATION-TYPE", "yuen from", LINK),
    ("SPEC-RELATION-TYPE", "yuen satisfied by", LINK),
    ("SPEC-RELATION-TYPE", "yuen verified by", LINK),
    ("SPEC-RELATION-TYPE", "yuen replaces", &[]),
    ("SPECIFICATION-TYPE", "yuen requirements", FILE),
    ("SPECIFICATION-TYPE", "yuen list", &[]),
    ("RELATION-GROUP-TYPE", "yuen relation group", &[]),
];

fn type_id(long_name: &str) -> String {
    id("type", &[long_name])
}

fn attr_id(type_name: &str, attr: &str) -> String {
    id("attribute", &[type_name, attr])
}

fn rel_type(k: RelKind) -> &'static str {
    match k {
        RelKind::From => "yuen from",
        RelKind::Satisfied => "yuen satisfied by",
        RelKind::Verified => "yuen verified by",
        RelKind::Replaces => "yuen replaces",
    }
}

/// A value: a string, or XHTML paragraphs.
enum V {
    S(String),
    X(Vec<String>),
}

/// Whether XML 1.0 can hold the character at all (escaped or not).
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

fn esc_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\r' => o.push_str("&#13;"),
            c => o.push(c),
        }
    }
    o
}

/// An attribute value in double quotes: `>` may stay as it is there.
fn esc_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '"' => o.push_str("&quot;"),
            '\t' => o.push_str("&#9;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            c => o.push(c),
        }
    }
    o
}

/// The document as it is written, and what stops it.
struct W {
    out: String,
    depth: usize,
    /// The first value the document cannot hold.
    refused: Option<Text>,
}

impl W {
    fn line(&mut self, s: &str) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn attrs(&mut self, attrs: &[(&str, &str)]) -> String {
        let mut o = String::new();
        for (k, v) in attrs {
            self.check(v);
            o.push_str(&format!(" {k}=\"{}\"", esc_attr(v)));
        }
        o
    }

    fn open(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        let a = self.attrs(attrs);
        self.line(&format!("<{tag}{a}>"));
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.line(&format!("</{tag}>"));
    }

    fn empty(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        let a = self.attrs(attrs);
        self.line(&format!("<{tag}{a}/>"));
    }

    fn leaf(&mut self, tag: &str, text: &str) {
        self.check(text);
        self.line(&format!("<{tag}>{}</{tag}>", esc_text(text)));
    }

    /// `<OUTER><REF>id</REF></OUTER>` on one line.
    fn reference(&mut self, outer: &str, inner: &str, to: &str) {
        self.line(&format!("<{outer}><{inner}>{to}</{inner}></{outer}>"));
    }

    fn check(&mut self, v: &str) {
        if self.refused.is_some() {
            return;
        }
        if let Some(c) = v.chars().find(|c| !xml_char(*c)) {
            let code = format!("U+{:04X}", c as u32);
            let head: String = v.chars().take(40).collect();
            self.refused = Some(tr!(
                "「{head}」に XML に書けない文字（{code}）があるので、書き出せません",
                "\"{head}\" holds a character XML cannot hold ({code}), so it cannot be written out"
            ));
        }
    }

    /// The values of an object, a relation or a specification, as its type declares them.
    fn values(&mut self, type_name: &str, decl: &[Attr], vals: &[(&str, V)]) {
        if vals.is_empty() {
            return;
        }
        self.open("VALUES", &[]);
        for (name, v) in vals {
            let def = attr_id(type_name, name);
            let a = decl.iter().find(|a| a.name == *name).copied().unwrap_or_else(|| panic!("{type_name} declares {name}"));
            match (v, a.xhtml) {
                (V::S(s), false) => {
                    let n = s.chars().count();
                    if n > MAX_LENGTH && self.refused.is_none() {
                        let (n, max) = (crate::i18n::count(n as u64), crate::i18n::count(MAX_LENGTH as u64));
                        self.refused = Some(tr!(
                            "{name} の値が {n} 文字あり、文字列の型の長さの上限 {max} を超えるので、書き出せません",
                            "the value of {name} is {n} characters, more than the {max} the string type holds, so it cannot be written out"
                        ));
                    }
                    self.open("ATTRIBUTE-VALUE-STRING", &[("THE-VALUE", s)]);
                    self.reference("DEFINITION", "ATTRIBUTE-DEFINITION-STRING-REF", &def);
                    self.close("ATTRIBUTE-VALUE-STRING");
                }
                (V::X(paras), true) => {
                    for p in paras {
                        self.check(p);
                    }
                    let body = match paras.as_slice() {
                        [one] => format!("<xhtml:div>{}</xhtml:div>", esc_text(one)),
                        many => format!("<xhtml:div>{}</xhtml:div>", many.iter().map(|p| format!("<xhtml:p>{}</xhtml:p>", esc_text(p))).collect::<String>()),
                    };
                    self.open("ATTRIBUTE-VALUE-XHTML", &[]);
                    self.reference("DEFINITION", "ATTRIBUTE-DEFINITION-XHTML-REF", &def);
                    self.line(&format!("<THE-VALUE>{body}</THE-VALUE>"));
                    self.close("ATTRIBUTE-VALUE-XHTML");
                }
                _ => panic!("{name} is written as its type declares it"),
            }
        }
        self.close("VALUES");
    }
}

/// A day as `xsd:dateTime`: the start of it, in UTC (DESIGN 12).
fn day_time(d: crate::date::Day) -> String {
    format!("{d}T00:00:00Z")
}

/// `--time`: an RFC 3339 date and time with its offset, as `xsd:dateTime` takes it. None when
/// it is not one.
pub fn read_time(s: &str) -> Option<String> {
    let t = s.trim().to_ascii_uppercase();
    let b = t.as_bytes();
    if b.len() < 20 || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    crate::date::Day::parse(&t[..10])?;
    let num = |r: std::ops::Range<usize>, max: u32| -> Option<()> {
        let v: u32 = t.get(r.clone())?.parse().ok().filter(|_| t[r].bytes().all(|c| c.is_ascii_digit()))?;
        (v <= max).then_some(())
    };
    num(11..13, 23)?;
    num(14..16, 59)?;
    num(17..19, 59)?;
    let mut rest = &t[19..];
    if let Some(r) = rest.strip_prefix('.') {
        let n = r.bytes().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 {
            return None;
        }
        rest = &r[n..];
    }
    let zone_ok = rest == "Z" || {
        let z = rest.as_bytes();
        z.len() == 6 && (z[0] == b'+' || z[0] == b'-') && z[3] == b':' && rest[1..3].parse::<u32>().is_ok_and(|h| h <= 23) && rest[4..6].parse::<u32>().is_ok_and(|m| m <= 59) && rest[1..3].bytes().chain(rest[4..6].bytes()).all(|c| c.is_ascii_digit())
    };
    zone_ok.then_some(t)
}

/// The ReqIF document of a project. `time` is `--time`, already read: the time of what has no
/// day of its own, and of the header; without it, the latest day the project wrote down.
pub fn write(g: &Graph, time: Option<&str>, version: &str) -> Result<String, Refusal> {
    let p = g.p;
    let default_time = match time {
        Some(t) => t.to_string(),
        None => match g.days().into_iter().max() {
            Some(d) => day_time(d),
            None => {
                return Err(Refusal(tr!(
                    "このプロジェクトには、決めた日も確かめた日も一つも無いので、ReqIF の時刻を決められません。`--time 2026-10-03T00:00:00Z` のように渡します",
                    "This project writes down no day anything was decided or looked at, so ReqIF has no time to give; pass one, as `--time 2026-10-03T00:00:00Z`"
                )));
            }
        },
    };
    let t = default_time.as_str();
    let heads: Vec<String> = p.files.iter().map(|f| f.ast.header.name.clone()).collect();
    let title = heads.join(", ");
    let tool = format!("yuen {version}");
    let mut w = W { out: String::new(), depth: 0, refused: None };
    w.line("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    w.open("REQ-IF", &[("xmlns", NAMESPACE), ("xmlns:xhtml", XHTML)]);
    // The header.
    w.open("THE-HEADER", &[]);
    let hid = id("header", &[&title]);
    w.open("REQ-IF-HEADER", &[("IDENTIFIER", &hid)]);
    w.leaf("CREATION-TIME", t);
    w.leaf("REQ-IF-TOOL-ID", &tool);
    w.leaf("REQ-IF-VERSION", "1.0");
    w.leaf("SOURCE-TOOL-ID", &tool);
    w.leaf("TITLE", &title);
    w.close("REQ-IF-HEADER");
    w.close("THE-HEADER");
    w.open("CORE-CONTENT", &[]);
    w.open("REQ-IF-CONTENT", &[]);
    // The datatypes.
    let (dt_string, dt_xhtml) = (id("datatype", &["string"]), id("datatype", &["xhtml"]));
    w.open("DATATYPES", &[]);
    let max = MAX_LENGTH.to_string();
    w.empty("DATATYPE-DEFINITION-STRING", &[("IDENTIFIER", &dt_string), ("LAST-CHANGE", t), ("LONG-NAME", "yuen string"), ("MAX-LENGTH", &max)]);
    w.empty("DATATYPE-DEFINITION-XHTML", &[("IDENTIFIER", &dt_xhtml), ("LAST-CHANGE", t), ("LONG-NAME", "yuen xhtml")]);
    w.close("DATATYPES");
    // The types and their attributes.
    w.open("SPEC-TYPES", &[]);
    for (el, name, attrs) in TYPES {
        let tid = type_id(name);
        if attrs.is_empty() {
            w.empty(el, &[("IDENTIFIER", &tid), ("LAST-CHANGE", t), ("LONG-NAME", name)]);
            continue;
        }
        w.open(el, &[("IDENTIFIER", &tid), ("LAST-CHANGE", t), ("LONG-NAME", name)]);
        w.open("SPEC-ATTRIBUTES", &[]);
        for a in *attrs {
            let aid = attr_id(name, a.name);
            let (def_el, ref_el, dt) = if a.xhtml {
                ("ATTRIBUTE-DEFINITION-XHTML", "DATATYPE-DEFINITION-XHTML-REF", &dt_xhtml)
            } else {
                ("ATTRIBUTE-DEFINITION-STRING", "DATATYPE-DEFINITION-STRING-REF", &dt_string)
            };
            w.open(def_el, &[("IDENTIFIER", &aid), ("LAST-CHANGE", t), ("LONG-NAME", a.name)]);
            w.reference("TYPE", ref_el, dt);
            w.close(def_el);
        }
        w.close("SPEC-ATTRIBUTES");
        w.close(el);
    }
    w.close("SPEC-TYPES");
    // The objects: the requirement versions, the sources, the artifacts.
    let req_ids: Vec<String> = (0..p.reqs.len()).map(|r| id("requirement", &[&p.reqs[r].name, &p.reqs[r].version.to_string()])).collect();
    let req_times: Vec<String> = (0..p.reqs.len()).map(|r| g.last_day(r).map(day_time).unwrap_or_else(|| t.to_string())).collect();
    let source_ids: Vec<String> = g.sources.iter().map(|s| format!("_{}", s.key.digits())).collect();
    let artifact_ids: Vec<String> = g.artifacts.iter().map(|a| id("artifact", &[&a.text()])).collect();
    w.open("SPEC-OBJECTS", &[]);
    for r in 0..p.reqs.len() {
        let d = p.decl(r);
        let mut vals: Vec<(&str, V)> = vec![("ReqIF.ForeignID", V::S(g.foreign_id(r))), ("ReqIF.Name", V::X(vec![p.req_label(r)]))];
        if let Some((text, _)) = &d.text {
            vals.push(("ReqIF.Text", V::X(vec![text.clone()])));
        }
        if let Some((period, _)) = &d.in_force {
            vals.push(("yuen.inForce", V::S(period.to_string())));
        }
        if let Some((o, _)) = &d.owner {
            vals.push(("yuen.owner", V::S(o.clone())));
        }
        if !d.decided.is_empty() {
            vals.push(("yuen.decided", V::X(d.decided.iter().map(|dc| format!("decided {} by {} {}", dc.date, dc.by.0, crate::names::quote(&dc.why))).collect())));
        }
        if !d.waivers.is_empty() {
            let mut paras = Vec::new();
            for wv in &d.waivers {
                paras.push(format!("not {} {}", wv.side.word(), crate::names::quote(&wv.why)));
                if let Some(rc) = wv.record.as_ref().and_then(|r| r.parsed.as_ref().ok()) {
                    paras.push(format!("approved {} by {} {}", rc.date, rc.by, rc.up.iter().map(|h| format!("sha256:{h}")).collect::<Vec<_>>().join(", ")));
                }
            }
            vals.push(("yuen.waived", V::X(paras)));
        }
        if let Some(e) = &g.m.req_ends[r] {
            vals.push(("yuen.sha256", V::S(e.hash.clone())));
        }
        vals.push(("yuen.status", V::S(g.req_status(r))));
        object(&mut w, &req_ids[r], &req_times[r], "yuen requirement", REQUIREMENT, &vals);
    }
    for (i, s) in g.sources.iter().enumerate() {
        let mut vals: Vec<(&str, V)> = vec![("ReqIF.ForeignID", V::S(s.label.clone())), ("ReqIF.Name", V::X(vec![s.label.clone()]))];
        if !s.lines.is_empty() {
            vals.push(("ReqIF.Text", V::X(s.lines.clone())));
        }
        match &s.key {
            SourceKey::Law { db, id, asof, .. } => {
                vals.push(("yuen.law", V::S(format!("{} {id}", db.word()))));
                vals.push(("yuen.asof", V::S(asof.clone())));
                if let Some(rv) = &s.revision {
                    vals.push(("yuen.revision", V::S(rv.clone())));
                }
            }
            SourceKey::File { path } => {
                vals.push(("yuen.file", V::S(path.clone())));
                if let Some(u) = &s.url {
                    vals.push(("yuen.url", V::S(u.clone())));
                }
            }
        }
        if let Some(pin) = &s.pin {
            vals.push(("yuen.sha256", V::S(pin.clone())));
        }
        object(&mut w, &source_ids[i], t, "yuen source", SOURCE, &vals);
    }
    for (i, a) in g.artifacts.iter().enumerate() {
        let text = a.text();
        let mut vals: Vec<(&str, V)> = vec![("ReqIF.ForeignID", V::S(text.clone())), ("ReqIF.Name", V::X(vec![text]))];
        if let Some(Ok(e)) = g.m.artifacts.get(a) {
            vals.push(("yuen.sha256", V::S(e.hash.clone())));
        }
        vals.push(("yuen.end", V::S(if a.items.is_empty() { "file".into() } else { "item".into() })));
        object(&mut w, &artifact_ids[i], t, "yuen artifact", ARTIFACT, &vals);
    }
    w.close("SPEC-OBJECTS");
    // The relations.
    let target_id = |tg: Target| -> String {
        match tg {
            Target::Source(i) => source_ids[i].clone(),
            Target::Req(i) => req_ids[i].clone(),
            Target::Artifact(i) => artifact_ids[i].clone(),
        }
    };
    let mut rel_ids: Vec<String> = Vec::new();
    for rel in &g.rels {
        let base = id("relation", &[rel_type(rel.kind), &req_ids[rel.req], &target_id(rel.target)]);
        // The same relation written twice in a `.req` (a link repeated) is two relations.
        let mut rid = base.clone();
        let mut n = 1;
        while rel_ids.contains(&rid) {
            n += 1;
            rid = id("relation", &[rel_type(rel.kind), &req_ids[rel.req], &target_id(rel.target), &n.to_string()]);
        }
        rel_ids.push(rid);
    }
    if !g.rels.is_empty() {
        w.open("SPEC-RELATIONS", &[]);
        for (ri, rel) in g.rels.iter().enumerate() {
            let mut vals: Vec<(&str, V)> = Vec::new();
            let mut when = t.to_string();
            if let Some(st) = g.state(rel) {
                if let Some(rc) = g.record(st) {
                    when = day_time(rc.date);
                    vals.push(("yuen.reviewedOn", V::S(rc.date.to_string())));
                    vals.push(("yuen.reviewedBy", V::S(rc.by.clone())));
                    if let Some(h) = rc.up.get(rel.k) {
                        vals.push(("yuen.up", V::S(h.clone())));
                    }
                    if let Some(h) = &rc.down {
                        vals.push(("yuen.down", V::S(h.clone())));
                    }
                }
                vals.push(("yuen.status", V::S(st.status.word().to_string())));
            }
            let tn = rel_type(rel.kind);
            let decl: &[Attr] = if rel.kind == RelKind::Replaces { &[] } else { LINK };
            w.open("SPEC-RELATION", &[("IDENTIFIER", &rel_ids[ri]), ("LAST-CHANGE", &when)]);
            w.values(tn, decl, &vals);
            w.reference("SOURCE", "SPEC-OBJECT-REF", &req_ids[rel.req]);
            w.reference("TARGET", "SPEC-OBJECT-REF", &target_id(rel.target));
            w.reference("TYPE", "SPEC-RELATION-TYPE-REF", &type_id(tn));
            w.close("SPEC-RELATION");
        }
        w.close("SPEC-RELATIONS");
    }
    // The specifications: a `.req` each, then the sources and the artifacts.
    let spec_ids: Vec<String> = heads.iter().map(|h| id("specification", &["requirements", h])).collect();
    let (sources_spec, artifacts_spec) = (id("specification", &["sources"]), id("specification", &["artifacts"]));
    w.open("SPECIFICATIONS", &[]);
    for (fi, f) in p.files.iter().enumerate() {
        let mut vals: Vec<(&str, V)> = vec![("yuen.file", V::S(f.rel.clone())), ("yuen.version", V::S(f.ast.header.version.to_string()))];
        if let Some((desc, _)) = &f.ast.description {
            vals.push(("yuen.description", V::S(desc.clone())));
        }
        let children: Vec<(String, String)> = (0..p.reqs.len()).filter(|r| p.reqs[*r].file == fi).map(|r| (req_ids[r].clone(), req_times[r].clone())).collect();
        let when = children.iter().map(|c| c.1.clone()).max().unwrap_or_else(|| t.to_string());
        specification(&mut w, &spec_ids[fi], &when, &f.ast.header.name, "yuen requirements", FILE, &vals, &children);
    }
    if !g.sources.is_empty() {
        let children: Vec<(String, String)> = source_ids.iter().map(|i| (i.clone(), t.to_string())).collect();
        specification(&mut w, &sources_spec, t, "sources", "yuen list", &[], &[], &children);
    }
    if !g.artifacts.is_empty() {
        let children: Vec<(String, String)> = artifact_ids.iter().map(|i| (i.clone(), t.to_string())).collect();
        specification(&mut w, &artifacts_spec, t, "artifacts", "yuen list", &[], &[], &children);
    }
    w.close("SPECIFICATIONS");
    // The relations between specifications, a group for each pair (the guide's 2.11): a `.req`
    // to the sources, to the artifacts, to another `.req`.
    let mut groups: Vec<(usize, Target, Vec<usize>)> = Vec::new();
    for (ri, rel) in g.rels.iter().enumerate() {
        let from = p.reqs[rel.req].file;
        let to = match rel.target {
            Target::Source(_) => Target::Source(0),
            Target::Artifact(_) => Target::Artifact(0),
            Target::Req(r) if p.reqs[r].file != from => Target::Req(p.reqs[r].file),
            Target::Req(_) => continue,
        };
        match groups.iter_mut().find(|g| g.0 == from && g.1 == to) {
            Some(gr) => gr.2.push(ri),
            None => groups.push((from, to, vec![ri])),
        }
    }
    if !groups.is_empty() {
        w.open("SPEC-RELATION-GROUPS", &[]);
        for (from, to, members) in &groups {
            let (to_id, to_name) = match to {
                Target::Source(_) => (sources_spec.clone(), "sources".to_string()),
                Target::Artifact(_) => (artifacts_spec.clone(), "artifacts".to_string()),
                Target::Req(fi) => (spec_ids[*fi].clone(), heads[*fi].clone()),
            };
            let gid = id("relation-group", &[&spec_ids[*from], &to_id]);
            let long = format!("{} -> {to_name}", heads[*from]);
            let when = members.iter().map(|ri| g.state(&g.rels[*ri]).and_then(|st| g.record(st)).map(|rc| day_time(rc.date)).unwrap_or_else(|| t.to_string())).max().unwrap_or_else(|| t.to_string());
            w.open("RELATION-GROUP", &[("IDENTIFIER", &gid), ("LAST-CHANGE", &when), ("LONG-NAME", &long)]);
            w.reference("SOURCE-SPECIFICATION", "SPECIFICATION-REF", &spec_ids[*from]);
            w.open("SPEC-RELATIONS", &[]);
            for ri in members {
                w.line(&format!("<SPEC-RELATION-REF>{}</SPEC-RELATION-REF>", rel_ids[*ri]));
            }
            w.close("SPEC-RELATIONS");
            w.reference("TARGET-SPECIFICATION", "SPECIFICATION-REF", &to_id);
            w.reference("TYPE", "RELATION-GROUP-TYPE-REF", &type_id("yuen relation group"));
            w.close("RELATION-GROUP");
        }
        w.close("SPEC-RELATION-GROUPS");
    }
    w.close("REQ-IF-CONTENT");
    w.close("CORE-CONTENT");
    w.close("REQ-IF");
    match w.refused {
        Some(why) => Err(Refusal(why)),
        None => Ok(w.out),
    }
}

fn object(w: &mut W, oid: &str, when: &str, type_name: &str, decl: &[Attr], vals: &[(&str, V)]) {
    w.open("SPEC-OBJECT", &[("IDENTIFIER", oid), ("LAST-CHANGE", when)]);
    w.values(type_name, decl, vals);
    w.reference("TYPE", "SPEC-OBJECT-TYPE-REF", &type_id(type_name));
    w.close("SPEC-OBJECT");
}

#[allow(clippy::too_many_arguments)]
fn specification(w: &mut W, sid: &str, when: &str, long_name: &str, type_name: &str, decl: &[Attr], vals: &[(&str, V)], children: &[(String, String)]) {
    w.open("SPECIFICATION", &[("IDENTIFIER", sid), ("LAST-CHANGE", when), ("LONG-NAME", long_name)]);
    w.values(type_name, decl, vals);
    if !children.is_empty() {
        w.open("CHILDREN", &[]);
        for (oid, t) in children {
            let hid = id("hierarchy", &[sid, oid]);
            w.open("SPEC-HIERARCHY", &[("IDENTIFIER", &hid), ("LAST-CHANGE", t)]);
            w.reference("OBJECT", "SPEC-OBJECT-REF", oid);
            w.close("SPEC-HIERARCHY");
        }
        w.close("CHILDREN");
    }
    w.reference("TYPE", "SPECIFICATION-TYPE-REF", &type_id(type_name));
    w.close("SPECIFICATION");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(read_time("2026-10-03T00:00:00Z").as_deref(), Some("2026-10-03T00:00:00Z"));
        assert_eq!(read_time("2026-10-03t09:30:00+09:00").as_deref(), Some("2026-10-03T09:30:00+09:00"));
        assert_eq!(read_time("2026-10-03T09:30:00.250-05:00").as_deref(), Some("2026-10-03T09:30:00.250-05:00"));
        assert_eq!(read_time("2026-10-03"), None);
        assert_eq!(read_time("2026-10-03T24:00:00Z"), None);
        assert_eq!(read_time("2026-02-30T00:00:00Z"), None);
        assert_eq!(read_time("2026-10-03T00:00:00"), None);
        assert_eq!(read_time("2026-10-03T00:00:00+9:00"), None);
    }

    #[test]
    fn escaping() {
        assert_eq!(esc_attr("a\"b<c>&\td"), "a&quot;b&lt;c>&amp;&#9;d");
        assert_eq!(esc_text("x<y & z>"), "x&lt;y &amp; z&gt;");
        assert!(!xml_char('\u{1}'));
        assert!(xml_char('民'));
    }
}
