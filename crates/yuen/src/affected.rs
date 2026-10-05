//! `yuen affected` (DESIGN 8): what a diff touches, and who is to look at it. The diff's files
//! are taken one by one, from the root: a `.req` (the requirements whose blocks it changes), a
//! copy of a source (the requirements that cite the article, and the rules and calendars that
//! pin it), a file the links name (the requirements that name something in it), code (geas
//! answers which claims the change touches, through ritsu's port of claims, from the records of
//! the lines each claim ran, and yuen follows the claims to the requirements that name them), a
//! delta spec of an OpenSpec change not yet archived (what it does to the requirements of a spec
//! the project pins, DESIGN 20.5), and the rest. Each requirement touched is said once, with its owner, where it comes from and the
//! last decision on it. Nothing is run and nothing is written.

use crate::ast::{FromWhat, Side};
use crate::check::Checked;
use crate::cli::Args;
use crate::names::{Name, Tool};
use crate::project::Project;
use crate::sources::Resolved;
use crate::suite::Suite;
use ritsu_base::text::{Lang, Text, plural};
use ritsu_base::udiff::{self, Content, FileDiff, Mark};
use ritsu_ports::{Said, TouchedLines, Untouched};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// A requirement a part of the diff reaches, and the side of the link it reaches it by (None
/// for a citation).
type Reach = (usize, Option<Side>);

/// A `.req` the diff changes: the requirements whose blocks its lines fall in, each with whether
/// its text or period is among them, and whether lines outside every requirement change too.
struct ReqFile {
    fi: usize,
    reqs: Vec<(usize, bool)>,
    outside: bool,
}

/// A copy of a source the diff changes.
struct Copy {
    path: String,
    label: Text,
    cited: Vec<usize>,
    pinned: Vec<Name>,
}

/// A delta spec of an OpenSpec change not yet archived, for a spec the project reads (DESIGN
/// 20.5): what it does to each requirement, and the requirements that cite it.
struct Proposed {
    change: String,
    delta: String,
    spec: String,
    items: Vec<(ritsu_base::openspec::Op, String, Option<String>, Vec<usize>)>,
}

/// A claim of a spec the change touches.
struct ClaimHit {
    name: String,
    status: String,
    lines: Vec<TouchedLines>,
    reqs: Vec<Reach>,
}

/// What geas answers for one spec, and what yuen makes of it.
struct Spec {
    spec: Name,
    records: Vec<(String, String)>,
    claims: Vec<ClaimHit>,
    unclaimed: Vec<(Untouched, Vec<Reach>)>,
    deleted: Vec<(String, bool)>,
    spec_changed: Vec<Reach>,
    baseline: bool,
    refused: Option<Vec<Said>>,
}

/// A file the links name, that the diff changes.
struct Named {
    file: Name,
    reqs: Vec<Reach>,
}

/// A change no requirement reaches: lines no claim runs, in a file no link names; or a file in a
/// scope that no requirement leads to.
enum Unreached {
    Unclaimed { file: String, side: String, lines: String, why: String },
    Untraced { file: String, scope: String },
}

struct Answer {
    diff_shown: String,
    req_files: Vec<ReqFile>,
    copies: Vec<Copy>,
    proposed: Vec<Proposed>,
    specs: Vec<Spec>,
    named: Vec<Named>,
    unreached: Vec<Unreached>,
    others: Vec<String>,
}

impl Answer {
    /// Every requirement touched, in the order of the project.
    fn touched(&self) -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        for f in &self.req_files {
            out.extend(f.reqs.iter().map(|(r, _)| *r));
        }
        for c in &self.copies {
            out.extend(c.cited.iter().copied());
        }
        for x in &self.proposed {
            for (_, _, _, cited) in &x.items {
                out.extend(cited.iter().copied());
            }
        }
        for s in &self.specs {
            for c in &s.claims {
                out.extend(c.reqs.iter().map(|(r, _)| *r));
            }
            out.extend(s.spec_changed.iter().map(|(r, _)| *r));
            for (_, reqs) in &s.unclaimed {
                out.extend(reqs.iter().map(|(r, _)| *r));
            }
        }
        for n in &self.named {
            out.extend(n.reqs.iter().map(|(r, _)| *r));
        }
        out
    }

    fn refused(&self) -> bool {
        self.specs.iter().any(|s| s.refused.is_some())
    }

    fn exit(&self) -> u8 {
        if self.refused() {
            2
        } else if !self.unreached.is_empty() {
            1
        } else {
            0
        }
    }
}

fn refuse(w: &mut dyn Write, msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let _ = writeln!(w, "{head}: {}", msg.get(lang));
    2
}

/// `yuen affected <path>... --diff <file|-> [--map <spec.geas>=<record>]... [--format json]`.
pub fn command(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let Some(diff_arg) = a.get("--diff") else {
        return refuse(err, tr!("`yuen affected` には差分 `--diff <file|->` が要ります", "`yuen affected` needs a diff: `--diff <file|->`"), lang);
    };
    let (diff_shown, bytes) = if diff_arg == "-" {
        let mut b = Vec::new();
        if let Err(e) = std::io::stdin().read_to_end(&mut b) {
            return refuse(err, tr!("標準入力を読めません: {e}", "Cannot read standard input: {e}"), lang);
        }
        ("<stdin>".to_string(), b)
    } else {
        match std::fs::read(diff_arg) {
            Ok(b) => (diff_arg.to_string(), b),
            Err(e) => return refuse(err, tr!("`{diff_arg}` を読めません: {e}", "Cannot read `{diff_arg}`: {e}"), lang),
        }
    };
    let files = match udiff::parse(&bytes) {
        Ok(f) => f,
        Err((line, why)) => {
            let at = if line > 0 { format!("{diff_shown}:{line}") } else { diff_shown.clone() };
            return refuse(err, tr!("差分を読めません（{at}）: {}", "The diff cannot be read ({at}): {}", why.ja; why.en), lang);
        }
    };
    let mut maps: Vec<(PathBuf, String)> = Vec::new();
    for m in a.all("--map") {
        let Some((spec, record)) = m.split_once('=') else {
            return refuse(err, tr!("`--map {m}` は `<spec.geas>=<記録>` の形ではありません", "`--map {m}` is not `<spec.geas>=<record>`"), lang);
        };
        let Ok(abs) = std::fs::canonicalize(spec) else {
            return refuse(err, tr!("`--map` の spec `{spec}` がありません", "The spec `{spec}` of `--map` is not there"), lang);
        };
        maps.push((abs, record.to_string()));
    }
    let c = match crate::run::checked(a, lang, "affected", suite, err) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let answer = match answer(&c, &files, &bytes, &diff_shown, &maps) {
        Ok(x) => x,
        Err(msg) => return refuse(err, msg, lang),
    };
    let p = c.project.as_ref().unwrap();
    if a.get("--format") == Some("json") {
        let _ = writeln!(out, "{}", serde_json::to_string_pretty(&to_json(p, &answer)).unwrap());
    } else {
        let _ = write!(out, "{}", render(p, &answer, lang));
    }
    answer.exit()
}

/// The lines of a `.req` on disk the diff changes, by its own numbering, whichever side of the
/// change the file is: a removed line counts where it was, an added line where it is. None when
/// the file is neither side.
fn changed_lines(f: &FileDiff, disk: &Content) -> Option<BTreeSet<usize>> {
    let after = if udiff::fits(disk, f, true) {
        true
    } else if udiff::fits(disk, f, false) {
        false
    } else {
        return None;
    };
    let last = disk.lines.len().max(1);
    let mut out = BTreeSet::new();
    for h in &f.hunks {
        // a side with no lines names the line before the place
        let mut old = h.old_start as usize + usize::from(h.old_len == 0);
        let mut new = h.new_start as usize + usize::from(h.new_len == 0);
        for l in &h.lines {
            match l.mark {
                Mark::Context => {
                    old += 1;
                    new += 1;
                }
                Mark::Removed => {
                    out.insert((if after { new } else { old }).min(last));
                    old += 1;
                }
                Mark::Added => {
                    out.insert((if after { new } else { old }).min(last));
                    new += 1;
                }
            }
        }
    }
    Some(out)
}

/// The requirements of a spec a diff changes (DESIGN 20.5): both sides of the change are made from
/// the file on disk, whichever side it is, and a requirement whose block differs between them, or
/// that is on one side only, is changed. None when the file on disk is neither side.
fn spec_touched(disk: &Path, f: &FileDiff) -> Option<Vec<String>> {
    use ritsu_base::openspec::{Spec, read_spec};
    let bytes = ritsu_base::fs::read(disk).ok()?;
    let here = Content::of(&bytes);
    let after = if udiff::fits(&here, f, true) {
        true
    } else if udiff::fits(&here, f, false) {
        false
    } else {
        return None;
    };
    let there = udiff::other_side(&here, f, after).bytes();
    let read = |b: &[u8]| read_spec(b).unwrap_or(Spec { requirements: vec![] });
    let (before, now) = if after { (read(&there), read(&bytes)) } else { (read(&bytes), read(&there)) };
    let mut out: Vec<String> = Vec::new();
    for r in before.requirements.iter().chain(&now.requirements) {
        let same = matches!((before.get(&r.name), now.get(&r.name)), (Some(a), Some(b)) if a.block == b.block);
        if !same && !out.contains(&r.name) {
            out.push(r.name.clone());
        }
    }
    Some(out)
}

/// The lines each requirement of a file takes: from its line to its last indented line.
fn blocks(p: &Project, fi: usize) -> Vec<(usize, usize, usize)> {
    let lines: Vec<&str> = p.files[fi].src.lines().collect();
    let mut out = Vec::new();
    for r in (0..p.reqs.len()).filter(|r| p.reqs[*r].file == fi) {
        let first = p.decl(r).span.line;
        let mut last = first;
        for (i, l) in lines.iter().enumerate().skip(first) {
            let t = l.trim();
            if t.is_empty() || (t.starts_with('#') && l.starts_with(char::is_whitespace)) {
                continue;
            }
            if !l.starts_with(char::is_whitespace) {
                break;
            }
            last = i + 1;
        }
        out.push((r, first, last));
    }
    out
}

/// What the links name, each with the requirement and the side.
fn links(p: &Project) -> Vec<(Name, usize, Side)> {
    let mut out = Vec::new();
    for r in 0..p.reqs.len() {
        for (li, n) in p.names.links[r].iter().enumerate() {
            if let Some(n) = n {
                out.push((n.clone(), r, p.decl(r).links[li].side));
            }
        }
    }
    out
}

fn push(v: &mut Vec<Reach>, x: Reach) {
    if !v.contains(&x) {
        v.push(x);
    }
}

/// What a copy of a source is: how a person reads it, and for a law's article its database,
/// law and article.
struct Of {
    label: String,
    article: Option<(String, String, String)>,
    /// An OpenSpec spec: read a requirement at a time (DESIGN 20.5).
    spec: bool,
}

fn answer(c: &Checked, files: &[FileDiff], bytes: &[u8], diff_shown: &str, maps: &[(PathBuf, String)]) -> Result<Answer, Text> {
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let named_links = links(p);
    let mut consumed: BTreeSet<String> = BTreeSet::new();
    let paths_of = |f: &FileDiff| -> Vec<String> {
        let mut v: Vec<String> = [f.old_path.clone(), f.new_path.clone()].into_iter().flatten().collect();
        v.dedup();
        v
    };

    // 1. the `.req` files of the project
    let mut req_files = Vec::new();
    for f in files {
        for path in paths_of(f) {
            let Some(fi) = p.files.iter().position(|x| x.rel == path) else { continue };
            consumed.insert(path.clone());
            let disk = Content::of(p.files[fi].src.as_bytes());
            let Some(changed) = changed_lines(f, &disk) else {
                let shown = p.shown(&path);
                return Err(tr!(
                    "{shown} の差分が、ディスクのファイルにも、変更前のそのファイルにも合いません",
                    "the diff of {shown} fits neither the file on disk nor that file before the change"
                ));
            };
            let mut reqs = Vec::new();
            let mut inside = BTreeSet::new();
            for (r, first, last) in blocks(p, fi) {
                let hit: Vec<usize> = changed.iter().copied().filter(|l| (first..=last).contains(l)).collect();
                if hit.is_empty() {
                    continue;
                }
                inside.extend(hit.iter().copied());
                let d = p.decl(r);
                let wording = d.text.as_ref().map(|(_, s)| s.line).into_iter().chain(d.in_force.as_ref().map(|(_, s)| s.line)).any(|l| hit.contains(&l));
                reqs.push((r, wording));
            }
            let outside = changed.iter().any(|l| !inside.contains(l));
            if !reqs.is_empty() || outside {
                req_files.push(ReqFile { fi, reqs, outside });
            }
        }
    }

    // 2. the copies of the sources: the project's own and borrowed ones, and those the rules and
    // calendars the links name pin
    let mut copy_of: BTreeMap<String, Of> = BTreeMap::new();
    for f in &m.sources.files {
        for (sname, res) in f {
            match res {
                Resolved::Law { db, id, articles, .. } => {
                    for a in articles {
                        copy_of.entry(a.rel.clone()).or_insert(Of { label: format!("{sname} {}", a.fragment), article: Some((db.word().to_string(), id.clone(), a.fragment.clone())), spec: false });
                    }
                }
                Resolved::File { name, .. } => {
                    copy_of.entry(name.path.clone()).or_insert(Of { label: sname.clone(), article: None, spec: false });
                }
                Resolved::OpenSpec { name, .. } => {
                    copy_of.entry(name.path.clone()).or_insert(Of { label: sname.clone(), article: None, spec: true });
                }
                _ => {}
            }
        }
    }
    let mut pinning: Vec<(Name, Vec<crate::sources::Pinned>)> = Vec::new();
    for (n, _, _) in &named_links {
        let file = Name { tool: n.tool, path: n.path.clone(), items: vec![] };
        if matches!(n.tool, Tool::Rulec | Tool::Koyomi) && !pinning.iter().any(|(x, _)| *x == file) {
            let pins = crate::sources::pinned_by(p, &file);
            pinning.push((file, pins));
        }
    }
    for (_, pins) in &pinning {
        for x in pins {
            copy_of.entry(x.rel.clone()).or_insert(Of { label: format!("{} {}", x.source, x.fragment), article: Some((x.db.word().to_string(), x.id.clone(), x.fragment.clone())), spec: false });
        }
    }
    let mut copies = Vec::new();
    for f in files {
        for path in paths_of(f) {
            let Some(of) = copy_of.get(&path) else { continue };
            consumed.insert(path.clone());
            // A spec: the requirements whose blocks the diff's lines fall in, whichever side of the
            // change the file on disk is; every requirement when it is neither (DESIGN 20.5).
            let in_blocks: Option<Vec<String>> = if of.spec { spec_touched(&p.root.join(&path), f) } else { None };
            let mut cited = Vec::new();
            for r in 0..p.reqs.len() {
                let fi = p.reqs[r].file;
                for fl in &p.decl(r).from {
                    let FromWhat::Cite { source, fragments, .. } = &fl.what else { continue };
                    let hit = match (m.sources.get(fi, source), &of.article) {
                        (Some(Resolved::Law { db, id, .. }), Some((d, i, fr))) => db.word() == d && id == i && fragments.iter().any(|(x, _)| x == fr),
                        (Some(Resolved::File { name, .. }), None) => name.path == path,
                        (Some(Resolved::OpenSpec { name, .. }), None) => name.path == path && fragments.iter().any(|(x, _)| in_blocks.as_ref().is_none_or(|t| t.contains(x))),
                        _ => false,
                    };
                    if hit && !cited.contains(&r) {
                        cited.push(r);
                    }
                }
            }
            let pinned: Vec<Name> = match &of.article {
                Some((d, i, fr)) => pinning.iter().filter(|(_, pins)| pins.iter().any(|x| x.db.word() == d && x.id == *i && x.fragment == *fr)).map(|(n, _)| n.clone()).collect(),
                None => vec![],
            };
            let label = match &in_blocks {
                Some(t) if !t.is_empty() => {
                    let names: Vec<String> = t.iter().map(|n| crate::names::word_or_quote(n)).collect();
                    Text::new(format!("{} {}", of.label, names.join("、")), format!("{} {}", of.label, names.join(", ")))
                }
                _ => Text::same(of.label.clone()),
            };
            copies.push(Copy { path, label, cited, pinned });
        }
    }

    // 2b. the delta specs of OpenSpec changes not yet archived, for a spec the project reads
    let mut proposed = Vec::new();
    for f in files {
        for path in paths_of(f) {
            let Some((spec, change, touches)) = crate::openspec::of_delta(&p.root, &path) else { continue };
            let reading: Vec<(usize, &str)> = m
                .sources
                .files
                .iter()
                .enumerate()
                .flat_map(|(fi, srcs)| srcs.iter().filter(|(_, r)| matches!(r, Resolved::OpenSpec { name, .. } if name.path == spec)).map(move |(n, _)| (fi, n.as_str())))
                .collect();
            if reading.is_empty() {
                continue;
            }
            consumed.insert(path.clone());
            let items = touches
                .iter()
                .map(|t| {
                    let mut cited = Vec::new();
                    for r in 0..p.reqs.len() {
                        let fi = p.reqs[r].file;
                        let hit = p.decl(r).from.iter().any(|fl| {
                            matches!(&fl.what, FromWhat::Cite { source, fragments, .. } if reading.contains(&(fi, source.as_str())) && fragments.iter().any(|(x, _)| *x == t.name))
                        });
                        if hit && !cited.contains(&r) {
                            cited.push(r);
                        }
                    }
                    (t.op, t.name.clone(), t.to.clone(), cited)
                })
                .collect();
            proposed.push(Proposed { change, delta: path.clone(), spec, items });
        }
    }

    // 4. code, through geas's records: every spec a link names
    let mut specs = Vec::new();
    let mut spec_paths: Vec<String> = Vec::new();
    for (n, _, _) in &named_links {
        if n.tool == Tool::Geas && !spec_paths.contains(&n.path) {
            spec_paths.push(n.path.clone());
        }
    }
    for (abs, _) in maps {
        let known = spec_paths.iter().any(|s| std::fs::canonicalize(p.root.join(s)).is_ok_and(|x| x == *abs));
        if !known {
            let shown = abs.strip_prefix(&p.root).map(|r| p.shown(&r.to_string_lossy())).unwrap_or_else(|_| abs.display().to_string());
            return Err(tr!("`--map` の spec {shown} を、プロジェクトのどのリンクも名指していません", "No link of the project names the spec {shown} of `--map`"));
        }
    }
    let named_file = |path: &str| -> Vec<Reach> {
        let mut v = Vec::new();
        for (n, r, side) in &named_links {
            if n.path == path && n.tool != Tool::Geas {
                push(&mut v, (*r, Some(*side)));
            }
        }
        v
    };
    for spec in &spec_paths {
        let spec_abs = p.root.join(spec);
        let canon = std::fs::canonicalize(&spec_abs).unwrap_or_else(|_| spec_abs.clone());
        let records: Vec<String> = maps.iter().filter(|(a, _)| *a == canon).map(|(_, r)| r.clone()).collect();
        let spec_name = Name { tool: Tool::Geas, path: spec.clone(), items: vec![] };
        consumed.insert(spec.clone());
        let Some(claims) = &p.suite.claims else {
            let cmd = crate::check::with_ritsu(p);
            return Err(tr!(
                "yuen 単独のバイナリは geas の記録を読めません。ほかの言語を読むには、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください",
                "this yuen cannot read geas's records; run it with every language joined, through ritsu: `{cmd}`"
            ));
        };
        let of_spec = |claim: Option<&str>| -> Vec<Reach> {
            let mut v = Vec::new();
            for (n, r, side) in &named_links {
                if n.tool == Tool::Geas && n.path == *spec && (n.items.is_empty() || claim.is_none() || n.items.first().map(|(_, c)| c.as_str()) == claim) {
                    push(&mut v, (*r, Some(*side)));
                }
            }
            v
        };
        match claims.affected(&spec_abs, Some(&p.root), bytes, diff_shown, &records) {
            Ok(aff) => {
                for ct in &aff.claims {
                    for l in &ct.lines {
                        consumed.insert(l.file.clone());
                    }
                }
                let claims: Vec<ClaimHit> = aff.claims.iter().map(|ct| ClaimHit { name: ct.name.clone(), status: ct.status.clone(), lines: ct.lines.clone(), reqs: of_spec(Some(&ct.name)) }).collect();
                let mut unclaimed = Vec::new();
                for u in &aff.unclaimed {
                    consumed.insert(u.file.clone());
                    unclaimed.push((u.clone(), named_file(&u.file)));
                }
                for (d, _) in &aff.deleted {
                    consumed.insert(d.clone());
                }
                let spec_changed = if aff.spec_changed.is_empty() { vec![] } else { of_spec(None) };
                specs.push(Spec { spec: spec_name, records: aff.records.clone(), claims, unclaimed, deleted: aff.deleted.clone(), spec_changed, baseline: aff.baseline_changed, refused: None });
            }
            Err(said) => specs.push(Spec { spec: spec_name, records: vec![], claims: vec![], unclaimed: vec![], deleted: vec![], spec_changed: vec![], baseline: false, refused: Some(said) }),
        }
    }

    // 3. the files the links name
    let mut named: Vec<Named> = Vec::new();
    for f in files {
        for path in paths_of(f) {
            let mut by_file: Vec<Named> = Vec::new();
            for (n, r, side) in &named_links {
                if n.path != path || n.tool == Tool::Geas {
                    continue;
                }
                let file = Name { tool: n.tool, path: n.path.clone(), items: vec![] };
                match by_file.iter_mut().find(|x| x.file == file) {
                    Some(x) => push(&mut x.reqs, (*r, Some(*side))),
                    None => by_file.push(Named { file, reqs: vec![(*r, Some(*side))] }),
                }
            }
            if !by_file.is_empty() {
                consumed.insert(path.clone());
                named.extend(by_file);
            }
        }
    }

    // what no requirement reaches: lines no claim runs in a file no link names, and files of a
    // scope no requirement leads to
    let mut unreached = Vec::new();
    for s in &specs {
        for (u, reqs) in &s.unclaimed {
            if reqs.is_empty() {
                unreached.push(Unreached::Unclaimed { file: u.file.clone(), side: u.side.clone(), lines: u.lines.clone(), why: u.why.clone() });
            }
        }
    }
    for f in files {
        for path in paths_of(f) {
            for s in &m.scopes {
                if s.artifacts.iter().any(|a| a.path == path) {
                    consumed.insert(path.clone());
                }
                if s.untraced.iter().any(|a| a.path == path) && named_file(&path).is_empty() && !unreached.iter().any(|u| matches!(u, Unreached::Untraced { file, .. } if *file == path)) {
                    unreached.push(Unreached::Untraced { file: path.clone(), scope: s.text.clone() });
                }
            }
        }
    }

    // 5. the rest
    let mut others = Vec::new();
    for f in files {
        let path = f.path().to_string();
        if !paths_of(f).iter().any(|x| consumed.contains(x)) && !others.contains(&path) {
            others.push(path);
        }
    }
    Ok(Answer { diff_shown: diff_shown.to_string(), req_files, copies, proposed, specs, named, unreached, others })
}

// ── For a person ──────────────────────────────────────────────────────────

fn side_word(side: &str) -> Text {
    match side {
        "before" => tr!("変更前", "before"),
        "after" => tr!("変更後", "after"),
        s => tr!("{s}", "{s}"),
    }
}

/// The side of the change a record is of, as geas tells it: `before`, `after`, `either` (the
/// diff changes no code it covers) or `mixed`.
fn record_side(side: &str) -> Text {
    match side {
        "before" => tr!("変更前のもの", "before"),
        "after" => tr!("変更後のもの", "after"),
        "either" => tr!("どちらの側にも合うもの", "either side"),
        "mixed" => tr!("前と後が混ざったもの", "mixed"),
        s => tr!("{s}", "{s}"),
    }
}

/// `26 (before)`: lines of one file on one side, how they were found.
fn lines_text(l: &TouchedLines) -> Text {
    let side = side_word(&l.side);
    let mut t = tr!("{}の {} 行目", "{} ({})", side.ja, l.lines; l.lines, side.en);
    if l.how == "near" {
        t = t.then(&tr!("（消えた行なので、前後の行から探した）", " (a removed line, read by the lines around it)"));
    }
    if let Some(target) = &l.startup {
        t = t.then(&tr!("（{target} を起動する主張がどれも走らせる、起動時のコード）", " (startup code every claim that starts {target} runs)"));
    }
    t
}

/// The lines a claim ran, file by file: `greeter/server.py 26 (after), 26 (before)`.
fn files_lines(p: &Project, ls: &[TouchedLines]) -> Text {
    let mut files: Vec<&str> = Vec::new();
    for l in ls {
        if !files.contains(&l.file.as_str()) {
            files.push(&l.file);
        }
    }
    let mut ja = Vec::new();
    let mut en = Vec::new();
    for f in files {
        let parts: Vec<Text> = ls.iter().filter(|l| l.file == f).map(lines_text).collect();
        let shown = p.shown(f);
        ja.push(format!("{shown} の{}", parts.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("と")));
        en.push(format!("{shown} {}", parts.iter().map(|t| t.en.clone()).collect::<Vec<_>>().join(", ")));
    }
    tr!("{}", "{}", ja.join("、"); en.join("; "))
}

/// `met by a, b; checked by c` for what reaches a thing.
fn reach_text(p: &Project, reqs: &[Reach]) -> Text {
    let names = |side: Option<Side>| -> Vec<String> { reqs.iter().filter(|(_, s)| *s == side).map(|(r, _)| p.req_label(*r)).collect() };
    let mut parts: Vec<Text> = Vec::new();
    let met = names(Some(Side::Satisfied));
    if !met.is_empty() {
        parts.push(tr!("満たす要件 {}", "met by {}", met.join("、"); met.join(", ")));
    }
    let checked = names(Some(Side::Verified));
    if !checked.is_empty() {
        parts.push(tr!("確かめる要件 {}", "checked by {}", checked.join("、"); checked.join(", ")));
    }
    let cited = names(None);
    if !cited.is_empty() {
        parts.push(tr!("引く要件 {}", "cited by {}", cited.join("、"); cited.join(", ")));
    }
    if parts.is_empty() {
        return tr!("どの要件も名指していない", "named by no requirement");
    }
    let ja: Vec<String> = parts.iter().map(|t| t.ja.clone()).collect();
    let en: Vec<String> = parts.iter().map(|t| t.en.clone()).collect();
    tr!("{}", "{}", ja.join("、"); en.join("; "))
}

fn said_text(p: &Project, s: &Said) -> Text {
    let at = match s.line {
        Some(l) => format!("{}:{l}", p.shown_any(&s.file)),
        None => p.shown_any(&s.file),
    };
    let code = if s.code.is_empty() { String::new() } else { format!("[{}] ", s.code) };
    tr!("{code}{at}: {}", "{code}{at}: {}", s.message.ja; s.message.en)
}

fn render(p: &Project, a: &Answer, lang: Lang) -> String {
    let mut lines: Vec<Text> = vec![tr!("差分: {}", "diff: {}", a.diff_shown; a.diff_shown)];
    for f in &a.req_files {
        let file = &p.files[f.fi].display;
        lines.push(tr!("差分が変える要件（{file}）:", "requirements the diff changes ({file}):"));
        for (r, wording) in &f.reqs {
            let me = p.req_label(*r);
            lines.push(if *wording {
                tr!("  {me}: 文か期間が変わるので、この要件のリンクを確かめ直す必要があります", "  {me}: its text or period changes, so its links are to be looked at again")
            } else {
                tr!("  {me}", "  {me}")
            });
        }
        if f.outside {
            lines.push(tr!("  どの要件にも入らない行（役割、出典、範囲）", "  lines outside every requirement (roles, sources, scopes)"));
        }
    }
    if !a.copies.is_empty() {
        lines.push(tr!("差分が触る出典のコピー:", "copies of sources the diff touches:"));
        for c in &a.copies {
            let shown = p.shown(&c.path);
            let label = &c.label;
            let cited: Vec<Reach> = c.cited.iter().map(|r| (*r, None)).collect();
            let who = reach_text(p, &cited);
            let mut t = tr!("  {shown}（{}）: {}", "  {shown} ({}): {}", label.ja, who.ja; label.en, who.en);
            if !c.pinned.is_empty() {
                let ns: Vec<String> = c.pinned.iter().map(|n| n.text()).collect();
                t = t.then(&tr!("、固定している成果物 {}", "; pinned by {}", ns.join("、"); ns.join(", ")));
            }
            lines.push(t);
        }
    }
    if !a.proposed.is_empty() {
        lines.push(tr!("差分が触る OpenSpec の変更の提案（まだ archive していないもの）:", "OpenSpec changes the diff touches, not yet archived:"));
        for x in &a.proposed {
            let (c, delta, spec) = (&x.change, p.shown(&x.delta), p.shown(&x.spec));
            lines.push(tr!("  {c}（{delta}、仕様は {spec}）:", "  {c} ({delta}, for {spec}):"));
            for (op, name, to, cited) in &x.items {
                let (w, n) = (op.word(), crate::names::word_or_quote(name));
                let what = match to {
                    Some(t) => format!("{w} {n} -> {}", crate::names::word_or_quote(t)),
                    None => format!("{w} {n}"),
                };
                let who = if cited.is_empty() {
                    match op {
                        ritsu_base::openspec::Op::Added => tr!("まだどの要件も読んでいません", "no requirement reads it yet"),
                        _ => tr!("どの要件も引いていません", "cited by no requirement"),
                    }
                } else {
                    reach_text(p, &cited.iter().map(|r| (*r, None)).collect::<Vec<_>>())
                };
                lines.push(tr!("    {what}: {}", "    {what}: {}", who.ja; who.en));
            }
        }
    }
    for s in &a.specs {
        let spec = s.spec.text();
        if s.records.is_empty() {
            lines.push(tr!("差分が触る主張（{spec}）:", "the claims the change touches ({spec}):"));
        } else {
            let ja: Vec<String> = s.records.iter().map(|(r, side)| format!("{} は{}", p.shown_any(r), record_side(side).ja)).collect();
            let en: Vec<String> = s.records.iter().map(|(r, side)| format!("{} ({})", p.shown_any(r), record_side(side).en)).collect();
            lines.push(tr!("差分が触る主張（{spec}、記録: {}）:", "the claims the change touches ({spec}; records: {}):", ja.join("、"); en.join(", ")));
        }
        if let Some(said) = &s.refused {
            for x in said {
                let t = said_text(p, x);
                lines.push(tr!("  geas から答えを得られません: {}", "  geas cannot answer: {}", t.ja; t.en));
            }
            lines.push(tr!("  記録が変更に合わないので、この spec のコードから主張に辿れません", "  the records do not fit the change, so the code of this spec reaches no claim"));
            continue;
        }
        if s.claims.is_empty() && s.unclaimed.is_empty() && s.spec_changed.is_empty() {
            lines.push(tr!("  なし", "  none"));
        }
        for c in &s.claims {
            let ls = files_lines(p, &c.lines);
            let claim = format!("claim {}", crate::names::word_or_quote(&c.name));
            let status = if c.status == "ok" { Text::default() } else { tr!("（記録を取ったときは {}）", " (it was {} when recorded)", c.status; c.status) };
            lines.push(tr!("  {claim}{}: {}", "  {claim}{}: {}", status.ja, ls.ja; status.en, ls.en));
            let who = reach_text(p, &c.reqs);
            lines.push(tr!("    {}", "    {}", who.ja; who.en));
        }
        if !s.spec_changed.is_empty() {
            let who = reach_text(p, &s.spec_changed);
            lines.push(tr!("  spec そのものが変わります（{}）", "  the spec itself changes: {}", who.ja; who.en));
        }
        for (u, reqs) in &s.unclaimed {
            let file = p.shown(&u.file);
            let side = side_word(&u.side);
            let why = unclaimed_why(&u.why);
            let mut t = tr!("  どの主張も走らせない行: {file} の{}の {} 行目（{}）", "  lines no claim runs: {file} {} ({}; {})", side.ja, u.lines, why.ja; u.lines, side.en, why.en);
            if !reqs.is_empty() {
                let who = reach_text(p, reqs);
                t = t.then(&tr!("。このファイルを名指す要件があります（{}）", "; the file is named: {}", who.ja; who.en));
            }
            lines.push(t);
        }
        for (d, known) in &s.deleted {
            let file = p.shown(d);
            lines.push(if *known { tr!("  消えるファイル: {file}（記録にある）", "  a file the diff deletes: {file} (the records have it)") } else { tr!("  消えるファイル: {file}（記録に無い）", "  a file the diff deletes: {file} (no record has it)") });
        }
        if s.baseline {
            lines.push(tr!("  spec の基準（baseline）が変わります", "  the spec's baseline changes"));
        }
    }
    if !a.named.is_empty() {
        lines.push(tr!("要件が名指すファイルで、差分が触るもの:", "files that requirements name, that the diff touches:"));
        for n in &a.named {
            let t = n.file.text();
            let who = reach_text(p, &n.reqs);
            lines.push(tr!("  {t}: {}", "  {t}: {}", who.ja; who.en));
        }
    }
    if a.unreached.is_empty() {
        lines.push(tr!("どの要件にも届かない変更: なし", "changes no requirement reaches: none"));
    } else {
        lines.push(tr!("どの要件にも届かない変更:", "changes no requirement reaches:"));
        for u in &a.unreached {
            lines.push(match u {
                Unreached::Unclaimed { file, side, lines: ls, why } => {
                    let file = p.shown(file);
                    let side = side_word(side);
                    let why = unclaimed_why(why);
                    tr!("  {file} の{}の {} 行目（{}）", "  {file} {} ({}; {})", side.ja, ls, why.ja; ls, side.en, why.en)
                }
                Unreached::Untraced { file, scope } => {
                    let file = p.shown(file);
                    tr!("  {file}（範囲 {scope} にあり、どの要件からも辿れない）", "  {file} (in the scope {scope}, and no requirement leads to it)")
                }
            });
        }
    }
    if !a.others.is_empty() {
        let shown: Vec<String> = a.others.iter().map(|o| p.shown(o)).collect();
        lines.push(tr!("要件に関わらないファイル: {}", "other files the diff touches: {}", shown.join("、"); shown.join(", ")));
    }
    let touched = a.touched();
    if !touched.is_empty() {
        lines.push(tr!("触る要件:", "requirements touched:"));
        for r in &touched {
            lines.push(requirement_line(p, *r));
        }
    }
    let owners = owners(p, &touched);
    lines.push(match touched.len() {
        0 => tr!("触る要件はありません", "no requirement is touched"),
        n => {
            let (ja, en) = (owners.join("、"), owners.join(", "));
            if owners.is_empty() {
                tr!("触る要件は {n} 件です", "{}", ; plural(n, "requirement touched", "requirements touched"))
            } else {
                tr!("触る要件は {n} 件です。{ja} に見てもらってください", "{}; ask {en}", ; plural(n, "requirement touched", "requirements touched"))
            }
        }
    });
    let mut out = String::new();
    for l in lines {
        out.push_str(l.get(lang));
        out.push('\n');
    }
    out
}

fn unclaimed_why(why: &str) -> Text {
    if why == "not reported" { tr!("どのランタイムも報告しないファイル", "no runtime reports the file") } else { tr!("どの主張も走らせない", "no claim runs them") }
}

/// One requirement touched: where it is, its owner, where it comes from, the last decision.
fn requirement_line(p: &Project, r: usize) -> Text {
    let d = p.decl(r);
    let me = p.req_label(r);
    let (file, line) = p.req_place(r);
    let owner = d.owner.as_ref().map(|o| o.0.clone()).unwrap_or_default();
    let from = from_labels(p, r);
    let (fja, fen) = (from.join("、"), from.join(", "));
    // a requirement a person decided, with no `from`, says only its decision
    let mut t = if from.is_empty() {
        tr!("  {me}（{file}:{line}）持ち主 {owner}", "  {me} ({file}:{line}): owner {owner}")
    } else {
        tr!("  {me}（{file}:{line}）持ち主 {owner}、出どころ {fja}", "  {me} ({file}:{line}): owner {owner}; from {fen}")
    };
    if let Some(dc) = d.decided.iter().max_by_key(|dc| dc.date) {
        let (date, by, why) = (dc.date.to_string(), &dc.by.0, &dc.why);
        t = t.then(&tr!("。{date} に {by} が「{why}」と決めました", "; decided {date} by {by}: \"{why}\""));
    }
    t
}

/// What a requirement comes from, as a person reads it: `民法 第142条`, `約款`, `支払日 v1`.
fn from_labels(p: &Project, r: usize) -> Vec<String> {
    let mut out = Vec::new();
    for (i, f) in p.decl(r).from.iter().enumerate() {
        match &f.what {
            FromWhat::Cite { source, fragments, .. } => {
                if fragments.is_empty() {
                    out.push(source.clone());
                } else {
                    // a name that is not a word (a requirement of an OpenSpec spec) in quotes
                    let frs: Vec<String> = fragments.iter().map(|(x, _)| crate::names::word_or_quote(x)).collect();
                    out.push(format!("{source} {}", frs.join(", ")));
                }
            }
            FromWhat::Req(_) => {
                if let Some(t) = p.names.from[r][i] {
                    out.push(p.req_label(t));
                }
            }
        }
    }
    out
}

/// The owners of the requirements touched, each once, in the order of the requirements.
fn owners(p: &Project, touched: &BTreeSet<usize>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in touched {
        if let Some((o, _)) = &p.decl(*r).owner
            && !out.contains(o)
        {
            out.push(o.clone());
        }
    }
    out
}

// ── For a program ─────────────────────────────────────────────────────────

fn reach_json(p: &Project, reqs: &[Reach]) -> Value {
    Value::Array(reqs.iter().map(|(r, side)| json!({"name": p.reqs[*r].name, "version": p.reqs[*r].version, "role": side.map(|s| s.word())})).collect())
}

/// A path another language gave, or one given on the command line, from the root when it is
/// under it.
fn rel_of(p: &Project, path: &str) -> String {
    let file = Path::new(path);
    let abs = if file.is_absolute() { file.to_path_buf() } else { p.cwd.join(file) };
    let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
    abs.strip_prefix(&p.root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| path.to_string())
}

fn said_json(p: &Project, s: &Said) -> Value {
    json!({"code": s.code, "file": rel_of(p, &s.file), "line": s.line, "message": s.message.en})
}

fn to_json(p: &Project, a: &Answer) -> Value {
    let touched = a.touched();
    json!({
        "diff": a.diff_shown,
        "root": p.root_shown,
        "requirement_files": a.req_files.iter().map(|f| json!({
            "file": p.files[f.fi].rel,
            "requirements": f.reqs.iter().map(|(r, w)| json!({"name": p.reqs[*r].name, "version": p.reqs[*r].version, "line": p.decl(*r).span.line, "wording_changed": w})).collect::<Vec<_>>(),
            "outside": f.outside,
        })).collect::<Vec<_>>(),
        "copies": a.copies.iter().map(|c| json!({
            "path": c.path,
            "source": c.label.en,
            "cited_by": reach_json(p, &c.cited.iter().map(|r| (*r, None)).collect::<Vec<_>>()),
            "pinned_by": c.pinned.iter().map(|n| crate::diag::value(&n.to_json())).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "openspec_changes": a.proposed.iter().map(|x| json!({
            "change": x.change,
            "delta": x.delta,
            "spec": x.spec,
            "requirements": x.items.iter().map(|(op, name, to, cited)| json!({
                "op": op.word(),
                "name": name,
                "to": to,
                "cited_by": reach_json(p, &cited.iter().map(|r| (*r, None)).collect::<Vec<_>>()),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "specs": a.specs.iter().map(|s| json!({
            "spec": crate::diag::value(&s.spec.to_json()),
            "records": s.records.iter().map(|(r, side)| json!({"path": rel_of(p, r), "side": side})).collect::<Vec<_>>(),
            "claims": s.claims.iter().map(|c| json!({
                "name": c.name,
                "status": c.status,
                "lines": c.lines.iter().map(|l| json!({"file": l.file, "side": l.side, "lines": l.lines, "how": l.how, "startup": l.startup})).collect::<Vec<_>>(),
                "requirements": reach_json(p, &c.reqs),
            })).collect::<Vec<_>>(),
            "spec_changed": reach_json(p, &s.spec_changed),
            "unclaimed": s.unclaimed.iter().map(|(u, reqs)| json!({"file": u.file, "side": u.side, "lines": u.lines, "why": u.why, "requirements": reach_json(p, reqs)})).collect::<Vec<_>>(),
            "deleted": s.deleted.iter().map(|(d, known)| json!({"file": d, "recorded": known})).collect::<Vec<_>>(),
            "baseline_changed": s.baseline,
            "refused": s.refused.as_ref().map(|said| said.iter().map(|x| said_json(p, x)).collect::<Vec<_>>()),
        })).collect::<Vec<_>>(),
        "files": a.named.iter().map(|n| json!({"artifact": crate::diag::value(&n.file.to_json()), "requirements": reach_json(p, &n.reqs)})).collect::<Vec<_>>(),
        "unreached": a.unreached.iter().map(|u| match u {
            Unreached::Unclaimed { file, side, lines, why } => json!({"file": file, "why": "unclaimed", "side": side, "lines": lines, "detail": why, "scope": null}),
            Unreached::Untraced { file, scope } => json!({"file": file, "why": "untraced", "side": null, "lines": null, "detail": null, "scope": scope}),
        }).collect::<Vec<_>>(),
        "others": a.others,
        "requirements": touched.iter().map(|r| {
            let d = p.decl(*r);
            let last = d.decided.iter().max_by_key(|dc| dc.date);
            json!({
                "name": p.reqs[*r].name,
                "version": p.reqs[*r].version,
                "file": p.files[p.reqs[*r].file].rel,
                "line": d.span.line,
                "owner": d.owner.as_ref().map(|o| o.0.clone()),
                "from": from_labels(p, *r),
                "decided": last.map(|dc| json!({"date": dc.date.to_string(), "by": dc.by.0, "why": dc.why})),
            })
        }).collect::<Vec<_>>(),
        "owners": owners(p, &touched),
        "exit": a.exit(),
    })
}
