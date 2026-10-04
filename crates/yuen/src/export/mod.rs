//! `yuen export` (DESIGN 12, 13, PLAN C.10, C.11): the project written out, as ReqIF for the
//! requirements tools of companies and as W3C PROV for provenance tools.
//!
//! Both read the graph the check made, through [`Graph`]: the requirement versions, the
//! articles and file sources they come from, the artifacts their links name, and every
//! relation between them with its record and its mark. Nothing here reads the network or a
//! clock: the same `.req` files give the same bytes.

pub mod prov;
pub mod reqif;

use crate::ast::*;
use crate::check::{Checked, Model};
use crate::copies;
use crate::marks::{LinkKind, LinkState};
use crate::names::Name;
use crate::project::Project;
use ritsu_base::sha256;
use crate::sources::Resolved;

/// Whether the check stopped the project from being written out: an error of the first four
/// stages — the words, the names, the sources, the artifacts (DESIGN 12). Without them some
/// end cannot be made. The later stages (cycles, periods, marks, coverage) are written out as
/// states.
pub fn blocked(c: &Checked) -> bool {
    c.model.is_none() || c.diags.iter().any(|d| d.is_error() && matches!(&d.code[..2], "E0" | "E1" | "E2"))
}

/// `yuen/1`, a kind and the parts that name a thing, joined by NUL, hashed: the first 32 hex
/// digits of the SHA-256 (DESIGN 12). The same thing gets the same digits however often it is
/// written out, so a tool that took it in before can update it.
pub fn digits(kind: &str, parts: &[&str]) -> String {
    let mut b: Vec<u8> = b"yuen/1".to_vec();
    b.push(0);
    b.extend_from_slice(kind.as_bytes());
    for p in parts {
        b.push(0);
        b.extend_from_slice(p.as_bytes());
    }
    sha256::hex(&b)[..32].to_string()
}

/// What tells two sources apart: an article of a law as of a date, or a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceKey {
    Law { db: LawDb, id: String, asof: String, fragment: String },
    File { path: String },
}

impl SourceKey {
    pub fn digits(&self) -> String {
        match self {
            SourceKey::Law { db, id, asof, fragment } => digits("source", &["law", db.word(), id, asof, fragment]),
            SourceKey::File { path } => digits("source", &["file", path]),
        }
    }
}

/// One article of a law, or one file source, as the first `.req` that declares it names it.
pub struct SourceNode {
    pub key: SourceKey,
    /// `民法 第142条`, `約款`.
    pub label: String,
    pub revision: Option<String>,
    /// The pinned hash.
    pub pin: Option<String>,
    /// For an article: its text, a line for each paragraph (DESIGN 12).
    pub lines: Vec<String>,
    /// For a file source.
    pub url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Source(usize),
    Req(usize),
    Artifact(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelKind {
    From,
    Satisfied,
    Verified,
    Replaces,
}

impl RelKind {
    /// The words of the `.req`: what the relation's type is called.
    pub fn words(self) -> &'static str {
        match self {
            RelKind::From => "from",
            RelKind::Satisfied => "satisfied by",
            RelKind::Verified => "verified by",
            RelKind::Replaces => "replaces",
        }
    }
}

/// A relation between a requirement version and what it comes from, what meets it, what
/// checks it, or what it replaces. A `from` line citing two articles is two relations.
pub struct Rel {
    pub kind: RelKind,
    pub req: usize,
    pub target: Target,
    /// The link it is part of: its record and its mark. None for `replaces`.
    pub state: Option<usize>,
    /// Which of the link's upper ends it is (its hash is the record's `up[k]`).
    pub k: usize,
}

/// The project as both writers read it.
pub struct Graph<'a> {
    pub p: &'a Project,
    pub m: &'a Model,
    pub sources: Vec<SourceNode>,
    /// The artifacts the links name, in the order they are first named.
    pub artifacts: Vec<Name>,
    pub rels: Vec<Rel>,
    /// What the files of the rules and calendars the links name pin (DESIGN 3.3), which PROV
    /// writes (`yuen:pins`).
    pub pins: Pins,
}

/// The articles the files of the artifacts pin: each file that pins one (an artifact itself, or
/// written as one more, with the hash of its bytes), each article it pins (a source of the
/// project, or one more), and which file pins which.
#[derive(Default)]
pub struct Pins {
    pub files: Vec<(Name, String)>,
    pub sources: Vec<SourceNode>,
    pub edges: Vec<(PinFile, PinSource)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinFile {
    Artifact(usize),
    File(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinSource {
    Source(usize),
    More(usize),
}

impl<'a> Graph<'a> {
    pub fn state(&self, rel: &Rel) -> Option<&'a LinkState> {
        rel.state.map(|i| &self.m.states[i])
    }

    /// The record of a link, when it has one that reads.
    pub fn record(&self, st: &LinkState) -> Option<&'a Record> {
        st.record(self.p).and_then(|r| r.parsed.as_ref().ok())
    }

    /// The days something was done in the project: a decision, a look, an approval.
    pub fn days(&self) -> Vec<crate::date::Day> {
        let mut out = Vec::new();
        for r in 0..self.p.reqs.len() {
            let d = self.p.decl(r);
            out.extend(d.decided.iter().map(|x| x.date));
            let recs = d.from.iter().filter_map(|f| f.record.as_ref()).chain(d.links.iter().filter_map(|l| l.record.as_ref())).chain(d.waivers.iter().filter_map(|w| w.record.as_ref()));
            out.extend(recs.filter_map(|r| r.parsed.as_ref().ok()).map(|r| r.date));
        }
        out
    }

    /// The latest day something was done to one requirement version: its decisions, and the
    /// records of its links and waivers.
    pub fn last_day(&self, r: usize) -> Option<crate::date::Day> {
        let d = self.p.decl(r);
        let recs = d.from.iter().filter_map(|f| f.record.as_ref()).chain(d.links.iter().filter_map(|l| l.record.as_ref())).chain(d.waivers.iter().filter_map(|w| w.record.as_ref()));
        d.decided.iter().map(|x| x.date).chain(recs.filter_map(|r| r.parsed.as_ref().ok()).map(|r| r.date)).max()
    }

    /// A requirement version's mark: `ok` when every link and waiver of it is as it was looked
    /// at, else the words `api` gives the others, in that order, once each.
    pub fn req_status(&self, r: usize) -> String {
        const ORDER: [&str; 6] = ["unreviewed", "up_changed", "down_changed", "unapproved", "bad_record", "unreadable"];
        let words: Vec<&str> = self.m.states.iter().filter(|s| s.req == r).map(|s| s.status.word()).collect();
        let out: Vec<&str> = ORDER.iter().copied().filter(|w| words.contains(w)).collect();
        if out.is_empty() { "ok".to_string() } else { out.join(" ") }
    }

    /// What identifies a requirement version outside the `.req`: its alias (or its name, when
    /// that is already the shape of one) and the version, `first_day v1`.
    pub fn foreign_id(&self, r: usize) -> String {
        let v = &self.p.reqs[r];
        format!("{} v{}", self.alias(r), v.version)
    }

    pub fn alias(&self, r: usize) -> String {
        let v = &self.p.reqs[r];
        self.p.decl(r).alias.as_ref().map(|a| a.0.clone()).unwrap_or_else(|| v.name.clone())
    }

    /// The versions of a requirement before this one, the one right before it.
    pub fn previous_version(&self, r: usize) -> Option<usize> {
        let vs = self.p.by_name.get(&self.p.reqs[r].name)?;
        let i = vs.iter().position(|x| *x == r)?;
        if i == 0 { None } else { Some(vs[i - 1]) }
    }
}

fn state_index(m: &Model, r: usize, k: LinkKind) -> Option<usize> {
    m.states.iter().position(|s| s.req == r && s.kind == k)
}

/// Gather the graph of a checked project (one `blocked` does not stop).
pub fn graph<'a>(p: &'a Project, m: &'a Model) -> Graph<'a> {
    let mut sources: Vec<SourceNode> = Vec::new();
    let find = |sources: &Vec<SourceNode>, key: &SourceKey| sources.iter().position(|s| &s.key == key);
    for (fi, _) in p.files.iter().enumerate() {
        for (name, r) in &m.sources.files[fi] {
            match r {
                Resolved::Law { db, id, asof, revision, articles, .. } => {
                    for a in articles {
                        let key = SourceKey::Law { db: *db, id: id.clone(), asof: asof.clone(), fragment: a.fragment.clone() };
                        if find(&sources, &key).is_some() {
                            continue;
                        }
                        let file = a.abs.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let lines = a.bytes.as_ref().and_then(|b| std::str::from_utf8(b).ok()).map(|x| copies::quote_lines(*db, &file, x)).unwrap_or_default();
                        sources.push(SourceNode { key, label: format!("{name} {}", a.fragment), revision: revision.clone(), pin: a.pin.clone(), lines, url: None });
                    }
                }
                Resolved::File { name: n, url, pin, .. } => {
                    let key = SourceKey::File { path: n.path.clone() };
                    if find(&sources, &key).is_none() {
                        sources.push(SourceNode { key, label: name.clone(), revision: None, pin: pin.clone(), lines: vec![], url: url.clone() });
                    }
                }
                Resolved::NoPort { .. } | Resolved::Broken => {}
            }
        }
    }
    let mut artifacts: Vec<Name> = Vec::new();
    let mut rels: Vec<Rel> = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        let d = p.decl(r);
        for (i, f) in d.from.iter().enumerate() {
            let state = state_index(m, r, LinkKind::From(i));
            match &f.what {
                FromWhat::Cite { source, fragments, .. } => match m.sources.get(fi, source) {
                    Some(Resolved::Law { db, id, asof, .. }) => {
                        for (k, (fr, _)) in fragments.iter().enumerate() {
                            let key = SourceKey::Law { db: *db, id: id.clone(), asof: asof.clone(), fragment: fr.clone() };
                            if let Some(t) = find(&sources, &key) {
                                rels.push(Rel { kind: RelKind::From, req: r, target: Target::Source(t), state, k });
                            }
                        }
                    }
                    Some(Resolved::File { name: n, .. }) => {
                        if let Some(t) = find(&sources, &SourceKey::File { path: n.path.clone() }) {
                            rels.push(Rel { kind: RelKind::From, req: r, target: Target::Source(t), state, k: 0 });
                        }
                    }
                    _ => {}
                },
                FromWhat::Req(_) => {
                    if let Some(t) = p.names.from[r][i] {
                        rels.push(Rel { kind: RelKind::From, req: r, target: Target::Req(t), state, k: 0 });
                    }
                }
            }
        }
        for (i, l) in d.links.iter().enumerate() {
            let Some(n) = &p.names.links[r][i] else { continue };
            let t = match artifacts.iter().position(|a| a == n) {
                Some(t) => t,
                None => {
                    artifacts.push(n.clone());
                    artifacts.len() - 1
                }
            };
            let kind = match l.side {
                Side::Satisfied => RelKind::Satisfied,
                Side::Verified => RelKind::Verified,
            };
            rels.push(Rel { kind, req: r, target: Target::Artifact(t), state: state_index(m, r, LinkKind::To(i)), k: 0 });
        }
        for t in p.names.replaces[r].iter().flatten() {
            rels.push(Rel { kind: RelKind::Replaces, req: r, target: Target::Req(*t), state: None, k: 0 });
        }
    }
    let pins = pins(p, &sources, &artifacts);
    Graph { p, m, sources, artifacts, rels, pins }
}

/// What the files of the rules and calendars the links name pin.
fn pins(p: &Project, sources: &[SourceNode], artifacts: &[Name]) -> Pins {
    let mut out = Pins::default();
    let mut files: Vec<Name> = Vec::new();
    for a in artifacts {
        let f = a.whole_file();
        if !files.contains(&f) {
            files.push(f);
        }
    }
    for f in files {
        let pinned = crate::sources::pinned_by(p, &f);
        if pinned.is_empty() {
            continue;
        }
        let from = match artifacts.iter().position(|a| *a == f) {
            Some(i) => PinFile::Artifact(i),
            None => {
                let hash = ritsu_base::fs::read(p.root.join(&f.path)).map(|b| sha256::short(&b)).unwrap_or_default();
                out.files.push((f.clone(), hash));
                PinFile::File(out.files.len() - 1)
            }
        };
        for x in pinned {
            let key = SourceKey::Law { db: x.db, id: x.id.clone(), asof: x.asof.clone(), fragment: x.fragment.clone() };
            let to = match sources.iter().position(|s| s.key == key) {
                Some(i) => PinSource::Source(i),
                None => match out.sources.iter().position(|s| s.key == key) {
                    Some(i) => PinSource::More(i),
                    None => {
                        let file = x.abs.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let lines = ritsu_base::fs::read(&x.abs).ok().and_then(|b| String::from_utf8(b).ok()).map(|t| copies::quote_lines(x.db, &file, &t)).unwrap_or_default();
                        let revision = x.abs.parent().and_then(copies::revision);
                        out.sources.push(SourceNode { key, label: format!("{} {}", x.source, x.fragment), revision, pin: Some(x.pin.clone()), lines, url: None });
                        PinSource::More(out.sources.len() - 1)
                    }
                },
            };
            out.edges.push((from, to));
        }
    }
    out
}
