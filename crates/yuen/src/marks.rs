//! Marks (DESIGN 4.2, 4.3, PLAN B.7): every link's record against the hashes of its ends now,
//! and what to say when they differ — what changed, the diff of it against what was looked at
//! (kept in `reviewed/`), and the order a person is best to look again in: the sources first,
//! then the requirements in the order they are read from one another, then the artifacts.

use crate::ast::*;
use crate::copies;
use crate::diag::{Diag, DiffLine, DiagExt};
use crate::diff;
use crate::ends::{End, Upper};
use ritsu_base::text::Text;
use crate::names::Name;
use crate::project::Project;
use std::collections::{BTreeMap, BTreeSet};

/// What an end is: what a mark groups by, and what it says changed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Thing {
    /// An article of a law a file declares, or a file source (`fragment` empty).
    Source { file: usize, source: String, fragment: String },
    Requirement(usize),
    /// An artifact's end: the naming it is the end of.
    Artifact(Name),
}

#[derive(Clone, Debug)]
pub struct EndInfo {
    pub thing: Thing,
    /// As a person reads it: `民法 第142条`, `満了日_142条`, `file "民法の期間.cal"`.
    pub label: String,
    pub end: End,
    /// For a copy of a law: the database and the copy's file, to read its text.
    pub law: Option<(LawDb, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkKind {
    From(usize),
    To(usize),
    Waiver(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// E301.
    Unreviewed,
    /// E302: which of the upper ends changed.
    UpChanged(Vec<usize>),
    /// E303.
    DownChanged,
    /// E304: no approval.
    Unapproved,
    /// E304: the requirement changed after the approval.
    WaiverChanged,
    /// E305: where the record is wrong, and how.
    BadRecord(usize, usize, Text),
    /// An end could not be made (stages 3–5 said why): not compared.
    Unreadable,
}

impl Status {
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Status::Ok | Status::Unreadable => None,
            Status::Unreviewed => Some("E301"),
            Status::UpChanged(_) => Some("E302"),
            Status::DownChanged => Some("E303"),
            Status::Unapproved | Status::WaiverChanged => Some("E304"),
            Status::BadRecord(..) => Some("E305"),
        }
    }

    /// The word `api` gives it (DESIGN 11).
    pub fn word(&self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Unreviewed => "unreviewed",
            Status::UpChanged(_) | Status::WaiverChanged => "up_changed",
            Status::DownChanged => "down_changed",
            Status::Unapproved => "unapproved",
            Status::BadRecord(..) => "bad_record",
            Status::Unreadable => "unreadable",
        }
    }
}

/// One link or waiver, its ends now, and how its record compares.
#[derive(Clone, Debug)]
pub struct LinkState {
    pub req: usize,
    pub kind: LinkKind,
    pub line: usize,
    pub col: usize,
    pub up: Option<Vec<EndInfo>>,
    pub down: Option<EndInfo>,
    pub status: Status,
}

impl LinkState {
    pub fn record<'a>(&self, p: &'a Project) -> Option<&'a RecordLine> {
        let d = p.decl(self.req);
        match self.kind {
            LinkKind::From(i) => d.from[i].record.as_ref(),
            LinkKind::To(i) => d.links[i].record.as_ref(),
            LinkKind::Waiver(i) => d.waivers[i].record.as_ref(),
        }
    }

    pub fn is_marked(&self) -> bool {
        self.status.code().is_some()
    }

    pub fn is_waiver(&self) -> bool {
        matches!(self.kind, LinkKind::Waiver(_))
    }
}

/// The ends of every link and waiver, and how each compares with its record. `artifact` is
/// the end of a link's artifact, when it can be read.
pub fn link_states(p: &Project, s: &crate::sources::Sources, req_ends: &[Option<End>], artifact: &dyn Fn(&Name) -> Option<End>) -> Vec<LinkState> {
    let mut out = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        let d = p.decl(r);
        let req_end = req_ends[r].as_ref().map(|e| EndInfo { thing: Thing::Requirement(r), label: p.req_label(r), end: e.clone(), law: None });
        let ups = crate::ends::uppers(p, s, r);
        for (i, f) in d.from.iter().enumerate() {
            let up = match &ups[i] {
                Some(Upper::Cited(cs)) => {
                    let FromWhat::Cite { source, fragments, .. } = &f.what else { unreachable!() };
                    Some(
                        cs.iter()
                            .enumerate()
                            .map(|(k, c)| EndInfo {
                                thing: Thing::Source { file: fi, source: source.clone(), fragment: fragments.get(k).map(|x| x.0.clone()).unwrap_or_default() },
                                label: c.label.clone(),
                                end: End { bytes: c.bytes.clone(), hash: c.hash.clone() },
                                law: c.law.clone(),
                            })
                            .collect(),
                    )
                }
                Some(Upper::Req(t)) => req_ends[*t].as_ref().map(|e| vec![EndInfo { thing: Thing::Requirement(*t), label: p.req_label(*t), end: e.clone(), law: None }]),
                None => None,
            };
            let status = status(f.record.as_ref(), up.as_deref(), req_end.as_ref(), false);
            out.push(LinkState { req: r, kind: LinkKind::From(i), line: f.span.line, col: f.span.col, up, down: req_end.clone(), status });
        }
        for (i, l) in d.links.iter().enumerate() {
            let down = p.names.links[r][i].as_ref().and_then(|n| artifact(n).map(|e| EndInfo { thing: Thing::Artifact(n.clone()), label: n.text(), end: e, law: None }));
            let up = req_end.clone().map(|e| vec![e]);
            let status = status(l.record.as_ref(), up.as_deref(), down.as_ref(), false);
            out.push(LinkState { req: r, kind: LinkKind::To(i), line: l.span.line, col: l.span.col, up, down, status });
        }
        for (i, w) in d.waivers.iter().enumerate() {
            let up = req_end.clone().map(|e| vec![e]);
            let status = status(w.record.as_ref(), up.as_deref(), None, true);
            out.push(LinkState { req: r, kind: LinkKind::Waiver(i), line: w.span.line, col: w.span.col, up, down: None, status });
        }
    }
    out
}

fn status(rec: Option<&RecordLine>, up: Option<&[EndInfo]>, down: Option<&EndInfo>, waiver: bool) -> Status {
    let Some(up) = up else { return Status::Unreadable };
    if !waiver && down.is_none() {
        return Status::Unreadable;
    }
    let Some(rec) = rec else { return if waiver { Status::Unapproved } else { Status::Unreviewed } };
    let rc = match &rec.parsed {
        Err((col, why)) => return Status::BadRecord(rec.line, *col, why.clone()),
        Ok(rc) => rc,
    };
    if rc.up.len() != up.len() {
        let (k, n) = (rc.up.len(), up.len());
        let l = ends_label(&up.iter().collect::<Vec<_>>());
        let (ja, en) = (l.ja, l.en);
        return Status::BadRecord(
            rec.line,
            rec.indent + 1,
            if waiver {
                tr!("承認の記録には要件のハッシュが一つ要りますが、{k} 個あります", "An approval holds one hash, the requirement's, and this one holds {k}")
            } else {
                tr!("記録にはリンク元のハッシュが {k} 個ありますが、リンク元は {n} 個（{ja}）です", "The record holds {k} hashes for the upper ends, and the link has {n} ({en})")
            },
        );
    }
    let changed: Vec<usize> = (0..up.len()).filter(|i| rc.up[*i] != up[*i].end.hash).collect();
    if !changed.is_empty() {
        return if waiver { Status::WaiverChanged } else { Status::UpChanged(changed) };
    }
    if let (Some(d), Some(rd)) = (down, &rc.down)
        && &d.end.hash != rd
    {
        return Status::DownChanged;
    }
    Status::Ok
}

/// The content of `reviewed/<hash>` beside a file, when it is there (DESIGN 4.4).
pub fn reviewed_content(p: &Project, fi: usize, hash: &str) -> Option<Vec<u8>> {
    let dir = p.files[fi].abs.parent()?;
    ritsu_base::fs::read(dir.join("reviewed").join(hash)).ok()
}

/// How deep a requirement is in the chain of `from`s: those it is read from come first.
fn depths(p: &Project) -> Vec<usize> {
    let n = p.reqs.len();
    let mut d = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for r in 0..n {
            for t in p.names.from[r].iter().flatten() {
                if *t != r && d[r] < d[*t] + 1 && d[*t] < n {
                    d[r] = d[*t] + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    d
}

/// Why a mark is there, as one of the groups DESIGN 4.3 orders: sources, requirements in the
/// order of `from`, artifacts, then what no one has looked at yet.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Changed(Thing),
    Unlooked,
}

fn group_rank(g: &Group, first: &BTreeMap<Thing, (usize, usize)>, depth: &[usize]) -> (u8, usize, usize, usize) {
    match g {
        Group::Changed(t @ Thing::Source { .. }) => {
            let (f, l) = first.get(t).copied().unwrap_or((usize::MAX, 0));
            (0, f, l, 0)
        }
        Group::Changed(Thing::Requirement(r)) => (1, depth[*r], *r, 0),
        Group::Changed(t @ Thing::Artifact(_)) => {
            let (f, l) = first.get(t).copied().unwrap_or((usize::MAX, 0));
            (2, f, l, 0)
        }
        Group::Unlooked => (3, 0, 0, 0),
    }
}

/// What a requirement's end changing comes from: the first of its upper ends that changed
/// (an article, or what changed in a requirement it is read from), else the requirement.
fn cause_of(p: &Project, by_req: &BTreeMap<usize, Vec<&LinkState>>, r: usize, guard: usize) -> Thing {
    if let Some(sts) = by_req.get(&r) {
        for st in sts {
            if let (LinkKind::From(i), Status::UpChanged(idx)) = (st.kind, &st.status) {
                let up = st.up.as_ref().expect("a changed link has its ends");
                return match &p.decl(r).from[i].what {
                    FromWhat::Cite { .. } => up[idx[0]].thing.clone(),
                    FromWhat::Req(_) => match &up[0].thing {
                        Thing::Requirement(t) if guard < p.reqs.len() => cause_of(p, by_req, *t, guard + 1),
                        other => other.clone(),
                    },
                };
            }
        }
    }
    Thing::Requirement(r)
}

/// What changed in a requirement's end between `old` and `new`.
enum Why {
    /// Only the hashes of upper ends changed: these, by label.
    Upstream(Vec<String>),
    /// The diff of the end, and how many lines of it are not shown.
    Diff(Vec<DiffLine>, usize),
    /// What was looked at is not in `reviewed/`.
    Unknown,
}

fn without_hash(l: &str) -> &str {
    match l.rfind(" sha256:") {
        Some(i) => &l[..i],
        None => l,
    }
}

/// The labels of a requirement's `from` lines as its end writes them.
fn from_labels(p: &Project, s: &crate::sources::Sources, req_ends: &[Option<End>], r: usize) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for u in crate::ends::uppers(p, s, r).into_iter().flatten() {
        match u {
            Upper::Cited(cs) => {
                for c in cs {
                    m.insert(without_hash(&c.line).to_string(), c.label);
                }
            }
            Upper::Req(t) => {
                if req_ends[t].is_some() {
                    let v = &p.reqs[t];
                    m.insert(format!("from requirement {} v{}", v.name, v.version), p.req_label(t));
                }
            }
        }
    }
    m
}

fn explain_req(p: &Project, ctx: &Ctx, fi: usize, r: usize, old: &str, new: &End) -> Why {
    let Some(old_bytes) = reviewed_content(p, fi, old) else { return Why::Unknown };
    let (a, b) = (String::from_utf8_lossy(&old_bytes).to_string(), String::from_utf8_lossy(&new.bytes).to_string());
    let mut gone: Vec<&str> = a.lines().collect();
    let mut came: Vec<&str> = Vec::new();
    for l in b.lines() {
        match gone.iter().position(|x| *x == l) {
            Some(i) => {
                gone.remove(i);
            }
            None => came.push(l),
        }
    }
    let all_from = gone.iter().chain(came.iter()).all(|l| l.starts_with("from "));
    let mut g: Vec<&str> = gone.iter().map(|l| without_hash(l)).collect();
    let mut c: Vec<&str> = came.iter().map(|l| without_hash(l)).collect();
    g.sort();
    c.sort();
    if all_from && g == c && !c.is_empty() {
        let labels = from_labels(p, ctx.sources, ctx.req_ends, r);
        return Why::Upstream(c.iter().map(|l| labels.get(*l).cloned().unwrap_or_else(|| l.trim_start_matches("from ").to_string())).collect());
    }
    let (d, more) = diff::unified(&a, &b);
    Why::Diff(d, more)
}

/// What the marks need to say what changed.
pub struct Ctx<'a> {
    pub sources: &'a crate::sources::Sources,
    pub req_ends: &'a [Option<End>],
}

/// The diff of two ends a person reads as text: a law's copies by their text, anything else
/// by its lines when it is UTF-8 and at most 1 MiB. None when it is not text.
fn text_diff(old: &[u8], new: &EndInfo) -> Option<(Vec<DiffLine>, usize)> {
    const MAX: usize = 1 << 20;
    if old.len() > MAX || new.end.bytes.len() > MAX {
        return None;
    }
    let (a, b) = (std::str::from_utf8(old).ok()?, std::str::from_utf8(&new.end.bytes).ok()?);
    Some(match &new.law {
        Some(_) => diff::unified(&copies::xml_text(a), &copies::xml_text(b)),
        None => diff::unified(a, b),
    })
}

/// Several upper ends as a person reads them: the articles of one source as
/// `民法 第141条、第143条` (`民法 第141条, 第143条`), anything else one after another.
fn ends_label(ends: &[&EndInfo]) -> Text {
    let source = match ends.first().map(|e| &e.thing) {
        Some(Thing::Source { source, .. }) => Some(source.clone()),
        _ => None,
    };
    if let Some(src) = &source
        && ends.iter().all(|e| matches!(&e.thing, Thing::Source { source, fragment, .. } if source == src && !fragment.is_empty()))
    {
        let frs: Vec<&str> = ends.iter().filter_map(|e| match &e.thing {
            Thing::Source { fragment, .. } => Some(fragment.as_str()),
            _ => None,
        }).collect();
        return Text::new(format!("{src} {}", frs.join("、")), format!("{src} {}", frs.join(", ")));
    }
    let ls: Vec<String> = ends.iter().map(|e| e.label.clone()).collect();
    Text::new(ls.join("、"), ls.join(", "))
}

fn review_cmd(p: &Project, st: &LinkState) -> Text {
    let f = p.file_of(st.req);
    let args: Vec<String> = p.args.iter().map(|a| if a.contains(' ') { crate::names::quote(a) } else { a.clone() }).collect();
    let mut args = args.join(" ");
    if let Some(r) = &p.root_flag {
        args.push_str(&format!(" --root {}", if r.contains(' ') { crate::names::quote(r) } else { r.clone() }));
    }
    let at = format!("{}:{}", f.display, st.line);
    tr!("yuen review {args} --at {at} --by <役割>", "yuen review {args} --at {at} --by <role>")
}

/// The marks, as diagnostics in the order DESIGN 4.3 gives, each with a W301 after it when
/// what was looked at is not in `reviewed/`.
pub fn mark_diags(p: &Project, ctx: &Ctx, states: &[LinkState]) -> Vec<Diag> {
    let depth = depths(p);
    let mut by_req: BTreeMap<usize, Vec<&LinkState>> = BTreeMap::new();
    for st in states {
        by_req.entry(st.req).or_default().push(st);
    }
    // Where each source and artifact first appears, for the order of the groups.
    let mut first: BTreeMap<Thing, (usize, usize)> = BTreeMap::new();
    for st in states {
        let at = (p.reqs[st.req].file, st.line);
        for e in st.up.iter().flatten().chain(st.down.iter()) {
            if matches!(e.thing, Thing::Source { .. } | Thing::Artifact(_)) {
                let v = first.entry(e.thing.clone()).or_insert(at);
                if at < *v {
                    *v = at;
                }
            }
        }
    }
    let mut marked: Vec<(Group, &LinkState)> = Vec::new();
    for st in states.iter().filter(|s| s.is_marked()) {
        let g = match (&st.status, st.kind) {
            (Status::Unreviewed | Status::Unapproved | Status::BadRecord(..), _) => Group::Unlooked,
            (Status::UpChanged(idx), LinkKind::From(i)) => match &p.decl(st.req).from[i].what {
                FromWhat::Cite { .. } => Group::Changed(st.up.as_ref().unwrap()[idx[0]].thing.clone()),
                FromWhat::Req(_) => match &st.up.as_ref().unwrap()[0].thing {
                    Thing::Requirement(t) => Group::Changed(cause_of(p, &by_req, *t, 0)),
                    other => Group::Changed(other.clone()),
                },
            },
            (Status::DownChanged, LinkKind::To(_)) => Group::Changed(st.down.as_ref().unwrap().thing.clone()),
            _ => Group::Changed(cause_of(p, &by_req, st.req, 0)),
        };
        marked.push((g, st));
    }
    marked.sort_by_key(|(g, st)| (group_rank(g, &first, &depth), depth[st.req], p.reqs[st.req].file, st.line));
    let mut shown: BTreeMap<(Thing, String, String), (String, usize)> = BTreeMap::new();
    let mut out = Vec::new();
    for (_, st) in marked {
        let (d, w301) = mark_diag(p, ctx, &by_req, st, &mut shown);
        out.push(d);
        if let Some(w) = w301 {
            out.push(w);
        }
    }
    out
}

fn mark_diag(p: &Project, ctx: &Ctx, by_req: &BTreeMap<usize, Vec<&LinkState>>, st: &LinkState, shown: &mut BTreeMap<(Thing, String, String), (String, usize)>) -> (Diag, Option<Diag>) {
    let fi = p.reqs[st.req].file;
    let f = &p.files[fi];
    let span = Span { line: st.line, col: st.col };
    let rec = st.record(p).and_then(|r| r.parsed.as_ref().ok());
    let who_when = rec.map(|r| (r.by.clone(), r.date.to_string()));
    let me = p.req_label(st.req);
    let cmd = review_cmd(p, st);
    let w301 = |hash: &str, date: &str| -> Diag {
        let file = f.display.clone();
        p.warn(fi, "W301", span, tr!(
            "{date} に確かめたときの中身が reviewed/ に無いので、変わったところを見せられません",
            "What was looked at on {date} is not in reviewed/, so what changed cannot be shown"
        ))
        .note(tr!(
            "{file} の隣の reviewed/{hash} がありません。リンクを確かめたときに `yuen review` が書くファイルなので、git に入れておいてください。",
            "reviewed/{hash} beside {file} is missing: `yuen review` writes it when a link is looked at, and it is kept in git."
        ))
    };
    let same_note = |at: &(String, usize)| {
        let at = format!("{}:{}", at.0, at.1);
        tr!("{at} と同じ変更です。差分はそちらの診断に出ています。", "The same change as at {at}; the diff is not shown again.")
    };
    match &st.status {
        Status::Unreviewed => {
            let what = match st.kind {
                LinkKind::From(_) => {
                    let ups: Vec<&EndInfo> = st.up.iter().flatten().collect();
                    let l = ends_label(&ups);
                    tr!("{} から {me} へのリンクを、まだ誰も確かめていません", "No one has looked at the link from {} to {me} yet", l.ja; l.en)
                }
                _ => {
                    let a = st.down.as_ref().map(|e| e.label.clone()).unwrap_or_default();
                    tr!("{me} から {a} へのリンクを、まだ誰も確かめていません", "No one has looked at the link from {me} to {a} yet")
                }
            };
            let d = p.err(fi, "E301", span, what)
                .note(tr!(
                    "両端を読んで、つながりが正しいことを確かめたら、`yuen review` を走らせてください。リンクの下に、確かめた人と日付と両端のハッシュが書かれます。",
                    "Once a person has read both ends and the link holds, `yuen review` writes who looked, when, and the hashes of both ends under the link."
                ))
                .fix_command(cmd);
            (d, None)
        }
        Status::Unapproved => {
            let d = p.err(fi, "E304", span, tr!("この見送りは、まだ承認されていません", "This waiver has not been approved yet"))
                .note(tr!(
                    "見送りも、人が決めることです。持ち主が理由を読んで承認したら、`yuen review` を走らせてください。見送りの下に、承認した人と日付と要件のハッシュが書かれます。",
                    "A waiver is a decision too: once the owner has read the reason and approves it, `yuen review` writes who approved it, when, and the requirement's hash under it."
                ))
                .fix_command(cmd);
            (d, None)
        }
        Status::BadRecord(line, col, why) => {
            let d = p.err(fi, "E305", Span { line: *line, col: *col }, why.clone())
                .note(tr!("記録は `yuen review` が書くものです。確かめ直してから、`yuen review` で書き直してください。", "A record is what `yuen review` writes; look again, and let it write the record anew."))
                .fix_command(cmd);
            (d, None)
        }
        Status::UpChanged(idx) => {
            let (who, date) = who_when.clone().unwrap_or_default();
            let rc = rec.unwrap();
            let up = st.up.as_ref().unwrap();
            match st.kind {
                LinkKind::From(i) if matches!(p.decl(st.req).from[i].what, FromWhat::Cite { .. }) => {
                    let changed: Vec<&EndInfo> = idx.iter().map(|k| &up[*k]).collect();
                    let l = ends_label(&changed);
                    let mut d = p.err(fi, "E302", span, tr!("{} は、{date} に {who} がこのリンクを確かめたあとで変わりました", "{} changed after {who} looked at this link on {date}", l.ja; l.en));
                    let mut lines: Vec<DiffLine> = Vec::new();
                    let mut missing = None;
                    let mut more_total = 0;
                    for k in idx {
                        let e = &up[*k];
                        let (old, new) = (&rc.up[*k], &e.end.hash);
                        let l = &e.label;
                        d = d.note(tr!("{l} はいま sha256:{new} です。確かめたときは sha256:{old} でした。", "{l} is now sha256:{new}; it was sha256:{old} when it was looked at."));
                        let key = (e.thing.clone(), old.clone(), new.clone());
                        if let Some(at) = shown.get(&key) {
                            d = d.note(same_note(at));
                            continue;
                        }
                        shown.insert(key, (f.display.clone(), st.line));
                        match reviewed_content(p, fi, old) {
                            None => missing = Some(old.clone()),
                            Some(b) => {
                                if let Some((dl, more)) = text_diff(&b, e) {
                                    if idx.len() > 1 {
                                        lines.push(DiffLine { op: '@', text: format!("== {l} ==") });
                                    }
                                    lines.extend(dl);
                                    more_total += more;
                                }
                            }
                        }
                    }
                    if !lines.is_empty() {
                        let copy = match (&p.decl(st.req).from[i].what, &up[idx[0]].thing) {
                            (FromWhat::Cite { source, .. }, Thing::Source { fragment, .. }) if up[idx[0]].law.is_some() => match ctx.sources.get(fi, source) {
                                Some(crate::sources::Resolved::Law { articles, .. }) => articles.iter().find(|a| &a.fragment == fragment).map(|a| p.shown(&a.rel)),
                                _ => None,
                            },
                            _ => None,
                        };
                        let head = match copy {
                            Some(c) => tr!("条文の変わったところ（コピーは {c}）", "what changed in the text (the copy {c})"),
                            None => tr!("変わったところ", "what changed"),
                        };
                        d = d.diff(head, lines);
                        if more_total > 0 {
                            d = d.note(tr!("差分はほかに {more_total} 行あります。", "{more_total} more lines of the diff are not shown."));
                        }
                    }
                    d = d.fix_command(cmd);
                    (d, missing.map(|h| w301(&h, &date)))
                }
                LinkKind::From(_) => {
                    // A requirement this one is read from changed.
                    let e = &up[0];
                    let Thing::Requirement(t) = e.thing else { unreachable!() };
                    let parent = &e.label;
                    let mut d = p.err(fi, "E302", span, tr!("{parent} は、{date} に {who} がこのリンクを確かめたあとで変わりました", "{parent} changed after {who} looked at this link on {date}"));
                    let (dd, missing) = explain_into(p, ctx, by_req, fi, t, &rc.up[0], &e.end, d, shown, (&f.display, st.line), &date);
                    d = dd.fix_command(cmd);
                    (d, missing.map(|h| w301(&h, &date)))
                }
                _ => {
                    // This requirement changed: its end is the upper end of the link.
                    let e = &up[0];
                    let (d, missing) = req_changed(p, ctx, by_req, fi, st, &rc.up[0], e, false, shown, &who, &date, &me);
                    (d.fix_command(cmd), missing.map(|h| w301(&h, &date)))
                }
            }
        }
        Status::WaiverChanged => {
            let (who, date) = who_when.clone().unwrap_or_default();
            let rc = rec.unwrap();
            let e = &st.up.as_ref().unwrap()[0];
            let (d, missing) = req_changed(p, ctx, by_req, fi, st, &rc.up[0], e, true, shown, &who, &date, &me);
            (d.fix_command(cmd), missing.map(|h| w301(&h, &date)))
        }
        Status::DownChanged => {
            let (who, date) = who_when.clone().unwrap_or_default();
            let rc = rec.unwrap();
            let e = st.down.as_ref().unwrap();
            let old = rc.down.clone().unwrap_or_default();
            match st.kind {
                LinkKind::From(_) => {
                    // The requirement changed, and the source did not.
                    let (d, missing) = req_changed(p, ctx, by_req, fi, st, &old, e, false, shown, &who, &date, &me);
                    (d.fix_command(cmd), missing.map(|h| w301(&h, &date)))
                }
                _ => {
                    let a = &e.label;
                    let mut d = p.err(fi, "E303", span, tr!("{a} は、{date} に {who} がこのリンクを確かめたあとで変わりました", "{a} changed after {who} looked at this link on {date}"));
                    let new = &e.end.hash;
                    let key = (e.thing.clone(), old.clone(), new.clone());
                    let mut missing = None;
                    if let Some(at) = shown.get(&key) {
                        d = d.note(same_note(at));
                    } else {
                        shown.insert(key, (f.display.clone(), st.line));
                        match reviewed_content(p, fi, &old) {
                            None => {
                                d = d.note(tr!("いまは sha256:{new} です。確かめたときは sha256:{old} でした。", "It is now sha256:{new}; it was sha256:{old} when it was looked at."));
                                missing = Some(old.clone());
                            }
                            Some(b) => match text_diff(&b, e) {
                                Some((dl, more)) => {
                                    // a file by its path, a thing in it by its naming (DESIGN 3.2)
                                    let path = match &e.thing {
                                        Thing::Artifact(n) if n.items.is_empty() => p.shown(&n.path),
                                        _ => a.clone(),
                                    };
                                    d = d.diff(tr!("{path} の変わったところ", "what changed in {path}"), dl);
                                    if more > 0 {
                                        d = d.note(tr!("差分はほかに {more} 行あります。", "{more} more lines of the diff are not shown."));
                                    }
                                }
                                None => {
                                    let size = ritsu_base::text::count(e.end.bytes.len() as u64);
                                    d = d.note(tr!(
                                        "テキストではないので、差分を出せません。いまは {size} バイト、sha256:{new} です。確かめたときは sha256:{old}（{} バイト）でした。",
                                        "It is not text, so no diff is shown: it is now {size} bytes, sha256:{new}; it was sha256:{old} ({} bytes) when it was looked at.",
                                        ritsu_base::text::count(b.len() as u64)
                                    ));
                                }
                            },
                        }
                    }
                    (d.fix_command(cmd), missing.map(|h| w301(&h, &date)))
                }
            }
        }
        Status::Ok | Status::Unreadable => unreachable!("only marks are said"),
    }
}

/// A mark that is there because this link's requirement changed: what to say, by what
/// changed in it.
#[allow(clippy::too_many_arguments)]
fn req_changed(
    p: &Project,
    ctx: &Ctx,
    by_req: &BTreeMap<usize, Vec<&LinkState>>,
    fi: usize,
    st: &LinkState,
    old: &str,
    e: &EndInfo,
    waiver: bool,
    shown: &mut BTreeMap<(Thing, String, String), (String, usize)>,
    who: &str,
    date: &str,
    me: &str,
) -> (Diag, Option<String>) {
    let span = Span { line: st.line, col: st.col };
    let code = match (&st.status, waiver) {
        (_, true) => "E304",
        (Status::DownChanged, _) => "E303",
        _ => "E302",
    };
    let r = st.req;
    let key = (Thing::Requirement(r), old.to_string(), e.end.hash.clone());
    let f = &p.files[fi];
    let looked = if waiver {
        tr!("{date} に {who} が承認した見送りです。", "{who} approved this waiver on {date}.")
    } else {
        tr!("{date} に {who} が確かめたリンクです。", "{who} looked at this link on {date}.")
    };
    if let Some(at) = shown.get(&key).cloned() {
        let at_s = format!("{}:{}", at.0, at.1);
        let msg = if waiver {
            tr!("{me} が変わったので、この見送りを承認し直す必要があります（{at_s} と同じ変更）", "{me} changed, so this waiver needs approving again (the same change as at {at_s})")
        } else {
            tr!("{me} が変わったので、このリンクを確かめ直す必要があります（{at_s} と同じ変更）", "{me} changed, so this link needs a look again (the same change as at {at_s})")
        };
        return (p.err(fi, code, span, msg).note(looked), None);
    }
    shown.insert(key, (f.display.clone(), st.line));
    match explain_req(p, ctx, fi, r, old, &e.end) {
        Why::Upstream(labels) => {
            let (ja, en) = (labels.join("、"), labels.join(", "));
            let msg = if waiver {
                tr!("{me} の出どころ（{ja}）が変わったので、この見送りを承認し直す必要があります", "{me} comes from something that changed ({en}), so this waiver needs approving again")
            } else {
                tr!("{me} の出どころ（{ja}）が変わったので、このリンクを確かめ直す必要があります", "{me} comes from something that changed ({en}), so this link needs a look again")
            };
            (p.err(fi, code, span, msg).note(looked).note(tr!("変わったものを、その診断で先に確かめてください。", "Look at what changed first, where its own diagnostic shows it.")), None)
        }
        Why::Diff(dl, more) => {
            let msg = if waiver {
                tr!("{me} は、{date} に {who} がこの見送りを承認したあとで変わりました", "{me} changed after {who} approved this waiver on {date}")
            } else {
                tr!("{me} は、{date} に {who} がこのリンクを確かめたあとで変わりました", "{me} changed after {who} looked at this link on {date}")
            };
            let mut d = p.err(fi, code, span, msg).diff(tr!("{me} の変わったところ", "what changed in {me}"), dl);
            if more > 0 {
                d = d.note(tr!("差分はほかに {more} 行あります。", "{more} more lines of the diff are not shown."));
            }
            (d, None)
        }
        Why::Unknown => {
            let by_req_cause = cause_of(p, by_req, r, 0);
            let msg = match (&by_req_cause, waiver) {
                (Thing::Requirement(x), false) if *x == r => tr!("{me} は、{date} に {who} がこのリンクを確かめたあとで変わりました", "{me} changed after {who} looked at this link on {date}"),
                (Thing::Requirement(x), true) if *x == r => tr!("{me} は、{date} に {who} がこの見送りを承認したあとで変わりました", "{me} changed after {who} approved this waiver on {date}"),
                (t, w) => {
                    let l = thing_label(p, t);
                    if w {
                        tr!("{me} の出どころ（{l}）が変わったので、この見送りを承認し直す必要があります", "{me} comes from something that changed ({l}), so this waiver needs approving again")
                    } else {
                        tr!("{me} の出どころ（{l}）が変わったので、このリンクを確かめ直す必要があります", "{me} comes from something that changed ({l}), so this link needs a look again")
                    }
                }
            };
            let new = &e.end.hash;
            (p.err(fi, code, span, msg).note(looked).note(tr!("{me} はいま sha256:{new} です。確かめたときは sha256:{old} でした。", "{me} is now sha256:{new}; it was sha256:{old} when it was looked at.")), Some(old.to_string()))
        }
    }
}

/// The explanation of a requirement this one is read from changing, added to `d`.
#[allow(clippy::too_many_arguments)]
fn explain_into(
    p: &Project,
    ctx: &Ctx,
    by_req: &BTreeMap<usize, Vec<&LinkState>>,
    fi: usize,
    t: usize,
    old: &str,
    new: &End,
    mut d: Diag,
    shown: &mut BTreeMap<(Thing, String, String), (String, usize)>,
    at: (&str, usize),
    _date: &str,
) -> (Diag, Option<String>) {
    let key = (Thing::Requirement(t), old.to_string(), new.hash.clone());
    if let Some(prev) = shown.get(&key) {
        let at_s = format!("{}:{}", prev.0, prev.1);
        return (d.note(tr!("{at_s} と同じ変更です。差分はそちらの診断に出ています。", "The same change as at {at_s}; the diff is not shown again.")), None);
    }
    shown.insert(key, (at.0.to_string(), at.1));
    let label = p.req_label(t);
    match explain_req(p, ctx, fi, t, old, new) {
        Why::Upstream(labels) => {
            let (ja, en) = (labels.join("、"), labels.join(", "));
            d = d.note(tr!("{label} の出どころ（{ja}）が変わりました。変わったものを、その診断で先に確かめてください。", "{label} comes from something that changed ({en}); look at that first, where its own diagnostic shows it."));
            (d, None)
        }
        Why::Diff(dl, more) => {
            d = d.diff(tr!("{label} の変わったところ", "what changed in {label}"), dl);
            if more > 0 {
                d = d.note(tr!("差分はほかに {more} 行あります。", "{more} more lines of the diff are not shown."));
            }
            (d, None)
        }
        Why::Unknown => {
            let c = cause_of(p, by_req, t, 0);
            if c != Thing::Requirement(t) {
                let l = thing_label(p, &c);
                d = d.note(tr!("{label} の出どころ（{l}）が変わりました。", "{label} comes from something that changed ({l})."));
            }
            (d, Some(old.to_string()))
        }
    }
}

fn thing_label(p: &Project, t: &Thing) -> String {
    match t {
        Thing::Source { source, fragment, .. } => {
            if fragment.is_empty() {
                source.clone()
            } else {
                format!("{source} {fragment}")
            }
        }
        Thing::Requirement(r) => p.req_label(*r),
        Thing::Artifact(n) => n.text(),
    }
}

/// Every hash the records of a set of `.req` files point at, for clearing `reviewed/`.
pub fn hashes_in(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = src;
    while let Some(i) = rest.find("sha256:") {
        let h = &rest[i + 7..];
        let hex: String = h.chars().take(16).collect();
        if hex.len() == 16 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            out.insert(hex);
        }
        rest = &rest[i + 7..];
    }
    out
}
