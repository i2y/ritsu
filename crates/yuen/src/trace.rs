//! `yuen trace` (DESIGN 9, PLAN B.11): why something is the way it is, from one requirement,
//! artifact or article back to the sources — the text of the article as copied, who decided
//! what and when, what meets the requirement and what checks it, with the records and the
//! marks of every link.

use crate::ast::*;
use crate::check::{Checked, Model};
use crate::copies;
use ritsu_base::text::{Lang, Text};
use crate::marks::{EndInfo, LinkKind, LinkState, Status, Thing};
use crate::names::{self, Name};
use crate::project::{Project, Refusal};
use crate::sources::Resolved;
use serde_json::{Value, json};

/// What to start from.
pub enum Start {
    Requirement(String),
    Artifact(String),
    Source(String),
}

/// One line of the text a person reads: its depth and its words.
struct Out {
    lines: Vec<(usize, Text)>,
}

impl Out {
    fn push(&mut self, depth: usize, t: Text) {
        self.lines.push((depth, t));
    }

    fn render(&self, lang: Lang) -> String {
        let mut s = String::new();
        for (d, t) in &self.lines {
            s.push_str(&"  ".repeat(*d));
            s.push_str(t.get(lang));
            s.push('\n');
        }
        s
    }
}

fn period_text(p: &crate::date::Period) -> Text {
    match (p.from, p.to) {
        (Some(a), Some(b)) => tr!("{a} から {b} まで", "in force from {a} to {b}"),
        (Some(a), None) => tr!("{a} から", "in force from {a}"),
        (None, Some(b)) => tr!("{b} まで", "in force until {b}"),
        (None, None) => Text::default(),
    }
}

fn status_text(st: &LinkState, rec: Option<&Record>) -> Text {
    let when = |verb_ja: &str, verb_en: &str| match rec {
        Some(r) => {
            let (d, b) = (r.date.to_string(), r.by.clone());
            tr!("{d} に {b} が{verb_ja}", "{verb_en} by {b} on {d}")
        }
        None => Text::default(),
    };
    let (looked_ja, looked_en) = if st.is_waiver() { ("承認した", "approved") } else { ("確かめた", "looked at") };
    let w = when(looked_ja, looked_en);
    match &st.status {
        Status::Ok => {
            if st.is_waiver() {
                w.then(&tr!("。いまもそのまま", "; as it was approved"))
            } else {
                w.then(&tr!("。いまもそのまま", "; as it was looked at"))
            }
        }
        Status::Unreviewed => tr!("まだ誰も確かめていない", "not looked at yet"),
        Status::Unapproved => tr!("まだ承認していない", "not approved yet"),
        Status::UpChanged(_) => w.then(&tr!("。そのあとでリンク元が変わった", "; the upper end changed since")),
        Status::DownChanged => w.then(&tr!("。そのあとでリンク先が変わった", "; the lower end changed since")),
        Status::WaiverChanged => w.then(&tr!("。そのあとで要件が変わった", "; the requirement changed since")),
        Status::BadRecord(..) => tr!("記録の形が崩れている", "the record is not written right"),
        Status::Unreadable => tr!("リンクの両端を読めない", "the ends of the link cannot be read"),
    }
}

fn rec_of<'a>(p: &'a Project, st: &LinkState) -> Option<&'a Record> {
    st.record(p).and_then(|r| r.parsed.as_ref().ok())
}

/// The article an end is, with its copy quoted: the header line and the quoted lines.
fn article(p: &Project, m: &Model, fi: usize, e: &EndInfo) -> (Text, Vec<String>, Value) {
    let Thing::Source { source, fragment, .. } = &e.thing else { return (Text::same(e.label.clone()), vec![], Value::Null) };
    match m.sources.get(fi, source) {
        Some(Resolved::Law { db, id, asof, revision, articles, .. }) => {
            let a = articles.iter().find(|a| &a.fragment == fragment);
            let rel = a.map(|a| a.rel.clone()).unwrap_or_default();
            let shown = p.shown(&rel);
            let rev = match revision {
                Some(r) => tr!("、版 {r}", ", revision {r}"),
                None => Text::default(),
            };
            let dbt = db.title();
            let head = tr!(
                "{source} {fragment}（{dbt} {id}、{asof} 時点{}。写しは {shown}）",
                "{source} {fragment} ({dbt} {id} as of {asof}{}; the copy {shown})",
                rev.ja;
                rev.en
            );
            let file = a.and_then(|a| a.abs.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let quoted = std::str::from_utf8(&e.end.bytes).map(|x| copies::quote_lines(*db, &file, x)).unwrap_or_default();
            let j = json!({
                "source": source, "fragment": fragment, "db": db.word(), "id": id, "asof": asof,
                "revision": revision, "copy": rel, "sha256": e.end.hash, "text": quoted,
            });
            (head, quoted, j)
        }
        Some(Resolved::File { name, url, .. }) => {
            let path = &p.shown(&name.path);
            let head = match url {
                Some(u) => tr!("{source}（ファイル {path}、元は {u}）", "{source} (the file {path}, from {u})"),
                None => tr!("{source}（ファイル {path}）", "{source} (the file {path})"),
            };
            let j = json!({"source": source, "fragment": null, "file": name.path, "url": url, "sha256": e.end.hash});
            (head, vec![], j)
        }
        _ => (Text::same(e.label.clone()), vec![], Value::Null),
    }
}

/// A requirement's trace, into `o` at `depth`, and as JSON.
fn requirement(p: &Project, m: &Model, r: usize, depth: usize, o: &mut Out, seen: &mut Vec<usize>) -> Value {
    let d = p.decl(r);
    let fi = p.reqs[r].file;
    let (file, line) = p.req_place(r);
    let me = p.req_label(r);
    let owner = d.owner.as_ref().map(|x| x.0.clone()).unwrap_or_default();
    let period = d.in_force.map(|(per, _)| period_text(&per)).unwrap_or_default();
    let tail = if period.is_empty() { Text::default() } else { tr!("、{}", ", {}", period.ja; period.en) };
    o.push(depth, tr!("{me}（{file}:{line}）持ち主 {owner}{}", "{me} ({file}:{line}), owned by {owner}{}", tail.ja; tail.en));
    let text = d.text.as_ref().map(|t| t.0.clone()).unwrap_or_default();
    o.push(depth + 1, tr!("「{text}」", "\"{text}\""));
    let versions = p.by_name.get(&p.reqs[r].name).map(|v| v.len()).unwrap_or(1);
    if versions > 1 {
        let v = p.reqs[r].version;
        o.push(depth + 1, tr!("版 v{v}（この要件の版は {versions} 個）", "version v{v} of {versions}"));
    }
    let mut replaces = Vec::new();
    for (i, t) in p.names.replaces[r].iter().enumerate() {
        if let Some(t) = t {
            let l = p.req_label(*t);
            o.push(depth + 1, tr!("{l} を置き換える", "replaces {l}"));
            replaces.push(json!({"name": p.reqs[*t].name, "version": p.reqs[*t].version, "line": d.replaces[i].span.line}));
        }
    }
    seen.push(r);
    let mut from = Vec::new();
    for (i, f) in d.from.iter().enumerate() {
        let st = crate::check::state(m, r, LinkKind::From(i));
        let rec = st.and_then(|s| rec_of(p, s));
        let status = st.map(|s| status_text(s, rec)).unwrap_or_default();
        match &f.what {
            FromWhat::Cite { .. } => {
                let ends: Vec<EndInfo> = st.and_then(|s| s.up.clone()).unwrap_or_default();
                if ends.is_empty() {
                    let FromWhat::Cite { source, fragments, .. } = &f.what else { unreachable!() };
                    let frs: Vec<&str> = fragments.iter().map(|x| x.0.as_str()).collect();
                    let l = format!("{source} {}", frs.join(", "));
                    o.push(depth + 1, tr!("出どころ: {l}（写しを読めない）", "comes from {l} (its copy cannot be read)"));
                }
                let mut arts = Vec::new();
                for e in &ends {
                    let (head, quoted, j) = article(p, m, fi, e);
                    o.push(depth + 1, tr!("出どころ: {}", "comes from {}", head.ja; head.en));
                    for q in &quoted {
                        o.push(depth + 2, Text::same(format!("> {q}")));
                    }
                    arts.push(j);
                }
                o.push(depth + 2, status.clone());
                from.push(json!({"line": f.span.line, "sources": arts, "requirement": null, "reviewed": rec.map(record_json), "status": st.map(|s| s.status.word())}));
            }
            FromWhat::Req(_) => {
                let Some(t) = p.names.from[r][i] else { continue };
                let l = p.req_label(t);
                o.push(depth + 1, tr!("出どころ: 要件 {l}", "comes from the requirement {l}"));
                o.push(depth + 2, status.clone());
                let sub = if seen.contains(&t) {
                    o.push(depth + 2, tr!("（上に出ています）", "(shown above)"));
                    Value::Null
                } else {
                    requirement(p, m, t, depth + 2, o, seen)
                };
                from.push(json!({"line": f.span.line, "sources": [], "requirement": sub, "reviewed": rec.map(record_json), "status": st.map(|s| s.status.word())}));
            }
        }
    }
    let mut decided = Vec::new();
    if !d.decided.is_empty() {
        o.push(depth + 1, tr!("決めたこと:", "decided:"));
        let mut ds: Vec<&Decided> = d.decided.iter().collect();
        ds.sort_by_key(|x| x.date);
        for dc in ds {
            let (date, by, why) = (dc.date.to_string(), dc.by.0.clone(), dc.why.clone());
            o.push(depth + 2, tr!("{date} {by}: {why}", "{date} {by}: {why}"));
            decided.push(json!({"date": date, "by": by, "why": why}));
        }
    }
    let mut links = Vec::new();
    for side in [Side::Satisfied, Side::Verified] {
        for (i, l) in d.links.iter().enumerate().filter(|(_, l)| l.side == side) {
            let st = crate::check::state(m, r, LinkKind::To(i));
            let rec = st.and_then(|s| rec_of(p, s));
            let status = st.map(|s| status_text(s, rec)).unwrap_or_default();
            let name = p.names.links[r][i].as_ref();
            let a = name.map(|n| n.text()).unwrap_or_default();
            let line = match side {
                Side::Satisfied => tr!("満たすもの: {a} — {}", "met by {a} — {}", status.ja; status.en),
                Side::Verified => tr!("確かめるもの: {a} — {}", "checked by {a} — {}", status.ja; status.en),
            };
            o.push(depth + 1, line);
            links.push(json!({"line": l.span.line, "role": side.word(), "artifact": name.map(|n| crate::diag::value(&n.to_json())), "reviewed": rec.map(record_json), "status": st.map(|s| s.status.word())}));
        }
        for (i, w) in d.waivers.iter().enumerate().filter(|(_, w)| w.side == side) {
            let st = crate::check::state(m, r, LinkKind::Waiver(i));
            let rec = st.and_then(|s| rec_of(p, s));
            let status = st.map(|s| status_text(s, rec)).unwrap_or_default();
            let why = &w.why;
            let line = match side {
                Side::Satisfied => tr!("満たすものを置かない:「{why}」— {}", "not satisfied: \"{why}\" — {}", status.ja; status.en),
                Side::Verified => tr!("確かめるものを置かない:「{why}」— {}", "not verified: \"{why}\" — {}", status.ja; status.en),
            };
            o.push(depth + 1, line);
            links.push(json!({"line": w.span.line, "role": format!("not {}", side.word()), "why": why, "approved": rec.map(record_json), "status": st.map(|s| s.status.word())}));
        }
    }
    json!({
        "name": p.reqs[r].name,
        "alias": d.alias.as_ref().map(|a| a.0.clone()),
        "version": p.reqs[r].version,
        "file": p.file_of(r).rel,
        "line": line,
        "text": text,
        "in_force": d.in_force.map(|(per, _)| json!({"from": per.from.map(|x| x.to_string()), "to": per.to.map(|x| x.to_string())})),
        "owner": owner,
        "replaces": replaces,
        "sha256": m.req_ends[r].as_ref().map(|e| e.hash.clone()),
        "from": from,
        "decided": decided,
        "links": links,
    })
}

fn record_json(r: &Record) -> Value {
    json!({"date": r.date.to_string(), "by": r.by, "up": r.up, "down": r.down})
}

pub struct Traced {
    text: Out,
    pub json: Value,
}

impl Traced {
    pub fn render(&self, lang: Lang) -> String {
        self.text.render(lang)
    }
}

pub fn trace(c: &Checked, start: &Start) -> Result<Traced, Refusal> {
    let p = c.project.as_ref().expect("a checked project");
    let m = c.model.as_ref().expect("names known");
    let mut o = Out { lines: vec![] };
    let json = match start {
        Start::Requirement(spec) => {
            let rs = p.find_req(spec).map_err(Refusal)?;
            let mut out = Vec::new();
            for r in rs {
                let mut seen = Vec::new();
                out.push(requirement(p, m, r, 0, &mut o, &mut seen));
            }
            json!({"requirements": out})
        }
        Start::Artifact(text) => {
            let n: Name = names::parse_one(text).map_err(|e| {
                let msg = e.msg;
                Refusal(tr!("`--artifact {text}` を名指しとして読めません: {}", "`--artifact {text}` is not a naming: {}", msg.ja; msg.en))
            })?;
            let label = n.text();
            let mut found: Vec<usize> = Vec::new();
            for r in 0..p.reqs.len() {
                if p.names.links[r].iter().flatten().any(|l| *l == n || l.contains(&n) || n.contains(l)) && !found.contains(&r) {
                    found.push(r);
                }
            }
            let in_scope = m.scopes.iter().any(|s| s.artifacts.contains(&n));
            if found.is_empty() && !in_scope {
                return Err(Refusal(tr!("{label} を名指すリンクも範囲もありません", "No link or scope names {label}")));
            }
            o.push(0, Text::same(label.clone()));
            if found.is_empty() {
                o.push(1, tr!("範囲にありますが、どの要件からも辿れません（E404）", "in scope, and no requirement leads to it (E404)"));
            }
            let mut out = Vec::new();
            for r in found {
                let mut seen = Vec::new();
                out.push(requirement(p, m, r, 1, &mut o, &mut seen));
            }
            json!({"artifact": crate::diag::value(&n.to_json()), "in_scope": in_scope, "requirements": out})
        }
        Start::Source(spec) => {
            let s = spec.trim().strip_prefix('@').unwrap_or(spec.trim());
            let (name, fragment) = match s.split_once(char::is_whitespace) {
                Some((n, f)) => (n.trim(), f.trim().trim_matches('"')),
                None => (s, ""),
            };
            let mut out = Vec::new();
            let mut any = false;
            for (fi, f) in p.files.iter().enumerate() {
                if !f.ast.sources.iter().any(|x| x.name == name) {
                    continue;
                }
                any = true;
                // The article, from the first link that reads it.
                let mut header_done = false;
                let mut art = Value::Null;
                let mut reqs = Vec::new();
                for r in 0..p.reqs.len() {
                    if p.reqs[r].file != fi {
                        continue;
                    }
                    for (i, fl) in p.decl(r).from.iter().enumerate() {
                        let FromWhat::Cite { source, fragments, .. } = &fl.what else { continue };
                        if source != name || !(fragment.is_empty() && fragments.is_empty() || fragments.iter().any(|x| x.0 == fragment)) {
                            continue;
                        }
                        let st = crate::check::state(m, r, LinkKind::From(i));
                        if !header_done {
                            if let Some(e) = st.and_then(|s| s.up.as_ref()).and_then(|ups| ups.iter().find(|e| matches!(&e.thing, Thing::Source { fragment: fr, .. } if fr == fragment))) {
                                let (head, quoted, j) = article(p, m, fi, e);
                                o.push(0, head);
                                for q in &quoted {
                                    o.push(1, Text::same(format!("> {q}")));
                                }
                                art = j;
                            } else {
                                o.push(0, Text::same(format!("{name} {fragment}").trim().to_string()));
                            }
                            o.push(0, tr!("引いている要件:", "cited by:"));
                            header_done = true;
                        }
                        let rec = st.and_then(|s| rec_of(p, s));
                        let status = st.map(|s| status_text(s, rec)).unwrap_or_default();
                        let (file, line) = p.req_place(r);
                        let me = p.req_label(r);
                        let owner = p.decl(r).owner.as_ref().map(|x| x.0.clone()).unwrap_or_default();
                        o.push(1, tr!("{me}（{file}:{line}）持ち主 {owner} — {}", "{me} ({file}:{line}), owned by {owner} — {}", status.ja; status.en));
                        reqs.push(json!({"name": p.reqs[r].name, "version": p.reqs[r].version, "file": f.rel, "line": line, "owner": owner, "status": st.map(|s| s.status.word())}));
                    }
                }
                if !header_done {
                    o.push(0, Text::same(format!("{name} {fragment}").trim().to_string()));
                    o.push(1, tr!("どの要件も引いていません", "no requirement cites it"));
                }
                out.push(json!({"file": f.rel, "article": art, "requirements": reqs}));
            }
            if !any {
                return Err(Refusal(tr!("出典「{name}」はこのプロジェクトで宣言されていません", "The source {name} is not declared in this project")));
            }
            json!({"source": name, "fragment": if fragment.is_empty() { Value::Null } else { json!(fragment) }, "found": out})
        }
    };
    Ok(Traced { text: o, json })
}
