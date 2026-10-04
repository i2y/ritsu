//! `yuen source fetch | pin | outdated` (DESIGN 14, PLAN C.12): the copies of a project's own
//! sources, as rulec and koyomi keep theirs.
//!
//! - `fetch` takes every article a `law` source pins or cites — from e-Gov law API v2, one
//!   `law_data` request each, or from the eCFR — and a `file` source from its `url`, and writes
//!   them beside the `.req`. A copy whose text did not change keeps its bytes, and so its pin:
//!   e-Gov rewrites the markup of articles no amendment touched.
//! - `pin` writes the first 16 digits of each copy's SHA-256 into the `.req`, and nothing
//!   else: every other byte stays, the line endings among them.
//! - `outdated` asks whether the original moved on: the revisions of a law that come into
//!   force after its `asof`, each held to the text before it, and a `file` source's `url`
//!   against its pin. For what changed it says which requirements cite it, who owns them, and
//!   how many links and waivers will need a look again.
//!
//! These are the only commands that read the network, through `curl` as a child process (no
//! TLS of yuen's own, no dependency); `check` never calls them. They read a project as far
//! as its names, and check nothing more: a copy that is missing, unpinned or unlike its pin
//! is what they are for. A source borrowed from rulec or koyomi is that tool's to fetch and
//! pin.

use crate::ast::*;
use crate::copies;
use crate::diag::DiffLine;
use crate::project::Project;
use ritsu_base::sha256;
use ritsu_base::sources::{self, Ecfr, Egov};
use ritsu_base::text::{Text, plural};
use std::collections::BTreeMap;
use std::path::Path;

/// What a command did or found, a line each, and whether an original moved on (`outdated`'s
/// exit code).
pub struct Outcome {
    pub lines: Vec<Text>,
    pub changed: bool,
}

/// ritsu-base's message, its English starting with a capital as yuen's sentences do (yuen
/// prints a message as written).
fn cap(t: Text) -> Text {
    Text { en: ritsu_base::text::capitalize(&t.en), ja: t.ja }
}

/// e-Gov law API v2: `YUEN_EGOV`, else e-Gov itself. The requests, the retries and the reading
/// of a reply are ritsu-base's (`ritsu_base::sources`).
pub fn egov() -> Egov {
    Egov { base: sources::base_url("YUEN_EGOV", sources::EGOV) }
}

/// The eCFR's versioner API: `YUEN_ECFR`, else the eCFR itself.
pub fn ecfr() -> Ecfr {
    Ecfr { base: sources::base_url("YUEN_ECFR", sources::ECFR) }
}

/// One request: three tries over HTTP and HTTPS, two seconds before the second and four
/// before the third (a busy server refuses now and then; a scheduled job that stops on one
/// refusal is one nobody reads), and one for a `file://` URL (DESIGN 14).
pub fn curl(url: &str) -> Result<Vec<u8>, Text> {
    sources::curl(url).map_err(cap)
}

/// Whether two copies say the same: their text, not their markup (rulec's §15.71).
pub fn same_text(a: &[u8], b: &[u8]) -> bool {
    sources::same_text(a, b)
}

/// What the network is asked for one command: the positions of supplementary provisions, a
/// whole law per date at most once.
#[derive(Default)]
struct Net {
    suppl: sources::SupplCache,
}

impl Net {
    /// An article of a law as of a date, and the revision it came from (empty for the eCFR,
    /// whose version is the date asked for). The position of an amending law's supplementary
    /// provisions is looked up as of the same date.
    fn article(&mut self, db: LawDb, id: &str, asof: &str, file: &str) -> Result<(Vec<u8>, String), Text> {
        match db {
            LawDb::Egov => {
                let (elm, amend) = sources::element_of_file(file);
                egov().element(id, asof, asof, &elm, amend.as_deref(), &mut self.suppl).map_err(cap)
            }
            LawDb::Ecfr => Ok((ecfr().section(id, asof, file.trim_end_matches(".xml")).map_err(cap)?, String::new())),
        }
    }
}

// ── What a file's sources are ───────────────────────────────────────────────

/// The articles of a law source: the pinned ones, then the cited ones without a pin yet.
fn fragments(f: &ReqFile, name: &str, pins: &[PinLine]) -> Vec<String> {
    let mut out: Vec<String> = pins.iter().map(|p| p.fragment.clone()).collect();
    for r in &f.requirements {
        for fl in &r.from {
            if let FromWhat::Cite { source, fragments, .. } = &fl.what
                && source == name
            {
                for (fr, _) in fragments {
                    if !out.contains(fr) {
                        out.push(fr.clone());
                    }
                }
            }
        }
    }
    out
}

fn borrowed_from(naming: &crate::names::Written) -> String {
    naming.tool.text.clone()
}

/// The lines of one file, under its name when the project has more than one.
fn under(p: &Project, fi: usize, lines: Vec<Text>, out: &mut Vec<Text>) {
    if p.files.len() > 1 {
        let d = &p.files[fi].display;
        out.push(Text::same(format!("{d}:")));
        out.extend(lines.into_iter().map(|t| Text::new(format!("  {}", t.ja), format!("  {}", t.en))));
    } else {
        out.extend(lines);
    }
}

// ── fetch ───────────────────────────────────────────────────────────────────

fn write_file(dest: &Path, bytes: &[u8], p: &Project) -> Result<(), Text> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            let d = shown_abs(p, parent);
            tr!("{d} を作れません: {e}", "Cannot make {d}: {e}")
        })?;
    }
    std::fs::write(dest, bytes).map_err(|e| {
        let d = shown_abs(p, dest);
        tr!("{d} に書けません: {e}", "Cannot write {d}: {e}")
    })
}

/// An absolute path under the root, as the text of a command shows it (DESIGN 2.2).
fn shown_abs(p: &Project, abs: &Path) -> String {
    match abs.strip_prefix(&p.root) {
        Ok(rel) => p.shown(&rel.to_string_lossy().replace('\\', "/")),
        Err(_) => abs.display().to_string(),
    }
}

/// `yuen source fetch`.
pub fn fetch(p: &Project) -> Result<Outcome, Text> {
    let mut out = Vec::new();
    let mut net = Net::default();
    for (fi, f) in p.files.iter().enumerate() {
        let req_dir = f.abs.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut lines = Vec::new();
        for (si, s) in f.ast.sources.iter().enumerate() {
            let name = &s.name;
            match &s.kind {
                SourceKind::Borrowed { naming } => {
                    let tool = borrowed_from(naming);
                    lines.push(tr!(
                        "{name}: {tool} のファイルから借りた出典なので、yuen は取りません。{tool} source fetch で取ります",
                        "{name}: borrowed from a {tool} file, so yuen does not fetch it; {tool} source fetch does"
                    ));
                }
                SourceKind::File { url, pin, .. } => {
                    let Some(n) = &p.names.sources[fi][si] else { continue };
                    let path = p.shown(&n.path);
                    let Some(url) = url else {
                        lines.push(tr!("{name}: url が無いので取れません（`url \"…\"` を足すと取れます）", "{name}: no url, so the copy cannot be fetched (add `url \"…\"`)"));
                        continue;
                    };
                    let body = curl(url)?;
                    let dest = p.root.join(&n.path);
                    let before = std::fs::read(&dest).ok();
                    let h = sha256::short(&body);
                    if before.as_deref() == Some(body.as_slice()) {
                        lines.push(tr!("{name}: 写し {path} は変わっていません（sha256:{h}）", "{name}: the copy {path} is unchanged (sha256:{h})"));
                        continue;
                    }
                    write_file(&dest, &body, p)?;
                    lines.push(if before.is_some() {
                        tr!("{name}: 写し {path} を取り直しました（sha256:{h}）", "{name}: fetched the copy {path} again (sha256:{h})")
                    } else {
                        tr!("{name}: 写し {path} を取りました（sha256:{h}）", "{name}: fetched the copy {path} (sha256:{h})")
                    });
                    lines.push(match pin {
                        Some(pn) if *pn == h => tr!("  固定と合っています", "  it matches the pin"),
                        Some(pn) => tr!(
                            "  固定は sha256:{pn} です。何が変わったかを読んでから、yuen source pin で固定し直します",
                            "  the pin is sha256:{pn}; read what changed, then pin it again with yuen source pin"
                        ),
                        None => tr!("  固定がありません。yuen source pin で固定します", "  it has no pin; yuen source pin writes one"),
                    });
                }
                SourceKind::Law { db, id, asof, pins } => {
                    let asof = asof.to_string();
                    let dir_rel = copies::copy_dir(id, &asof);
                    let cdir = req_dir.join(&dir_rel);
                    let mut revision = None;
                    for fr in fragments(&f.ast, name, pins) {
                        let Some(file) = copies::fragment_file(*db, &fr) else {
                            lines.push(tr!("{name}: `{fr}` は条の書き方になっていないので取れません", "{name}: `{fr}` is not written as an article, so it cannot be fetched"));
                            continue;
                        };
                        let (xml, rev) = net.article(*db, id, &asof, &file)?;
                        let dest = cdir.join(&file);
                        let before = std::fs::read(&dest).ok();
                        let same = before.as_deref().is_some_and(|b| same_text(b, &xml));
                        if !same {
                            write_file(&dest, &xml, p)?;
                        }
                        if !rev.is_empty() {
                            revision = Some(rev);
                        }
                        let h = sha256::short(if same { before.as_deref().unwrap() } else { &xml });
                        lines.push(match (&before, same) {
                            (None, _) => tr!("{name}: {fr} を取りました（sha256:{h}）", "{name}: fetched {fr} (sha256:{h})"),
                            (Some(_), true) => tr!("{name}: {fr} の本文は変わっていません（sha256:{h}）", "{name}: the text of {fr} is unchanged (sha256:{h})"),
                            (Some(_), false) => tr!(
                                "{name}: {fr} の本文が変わりました（いま sha256:{h}）。引いている要件を読み直してから固定します",
                                "{name}: the text of {fr} changed (now sha256:{h}); reread the requirements that cite it, then pin it"
                            ),
                        });
                    }
                    if let Some(rev) = revision {
                        let rp = cdir.join("revision.txt");
                        let want = format!("{rev}\n");
                        if std::fs::read_to_string(&rp).ok().as_deref() != Some(want.as_str()) {
                            write_file(&rp, want.as_bytes(), p)?;
                        }
                    }
                }
            }
        }
        under(p, fi, lines, &mut out);
    }
    Ok(Outcome { lines: out, changed: false })
}

// ── pin ─────────────────────────────────────────────────────────────────────

/// An article as a pin line writes it: bare when it is a name, else in quotes (`"§1910.157"`).
fn fragment_word(fr: &str) -> String {
    if fr.chars().all(|c| c.is_alphanumeric() || c == '_') { fr.to_string() } else { crate::names::quote(fr) }
}

/// `yuen source pin`: the new text of each `.req` that changes, and what was done.
pub fn pin(p: &Project) -> (Vec<(usize, String)>, Outcome) {
    let mut out = Vec::new();
    let mut texts = Vec::new();
    for (fi, f) in p.files.iter().enumerate() {
        let req_dir = f.abs.parent().map(Path::to_path_buf).unwrap_or_default();
        let src = &f.src;
        let mut lines: Vec<String> = src.split_inclusive('\n').map(|s| s.to_string()).collect();
        let mut inserts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        let mut report = Vec::new();
        let split = |l: &str| -> (String, String) {
            let body = l.trim_end_matches(['\n', '\r']);
            (body.to_string(), l[body.len()..].to_string())
        };
        for (si, s) in f.ast.sources.iter().enumerate() {
            let name = &s.name;
            match &s.kind {
                SourceKind::Borrowed { naming } => {
                    let tool = borrowed_from(naming);
                    report.push(tr!(
                        "{name}: {tool} のファイルから借りた出典なので、yuen は固定しません。{tool} source pin で固定します",
                        "{name}: borrowed from a {tool} file, so yuen does not pin it; {tool} source pin does"
                    ));
                }
                SourceKind::File { pin, .. } => {
                    let Some(n) = &p.names.sources[fi][si] else { continue };
                    let path = p.shown(&n.path);
                    let Ok(bytes) = std::fs::read(p.root.join(&n.path)) else {
                        report.push(tr!("{name}: 写し {path} が無いので固定できません。先に yuen source fetch を走らせます", "{name}: there is no copy {path} to pin; run yuen source fetch first"));
                        continue;
                    };
                    let h = sha256::short(&bytes);
                    if pin.as_deref() == Some(h.as_str()) {
                        report.push(tr!("{name}: sha256:{h} で固定済みです", "{name}: already pinned at sha256:{h}"));
                        continue;
                    }
                    let k = s.span.line - 1;
                    let (body, end) = split(&lines[k]);
                    lines[k] = sources::pinned(&body, &h) + &end;
                    report.push(tr!("{name}: sha256:{h} で固定しました", "{name}: pinned at sha256:{h}"));
                }
                SourceKind::Law { db, id, asof, pins } => {
                    let cdir = req_dir.join(copies::copy_dir(id, &asof.to_string()));
                    let said = report.len();
                    for pl in pins {
                        let Some(file) = copies::fragment_file(*db, &pl.fragment) else { continue };
                        let Ok(bytes) = std::fs::read(cdir.join(&file)) else {
                            let fr = &pl.fragment;
                            report.push(tr!("{name}: {fr} の写しが無いので固定できません。先に yuen source fetch を走らせます", "{name}: there is no copy of {fr} to pin; run yuen source fetch first"));
                            continue;
                        };
                        let h = sha256::short(&bytes);
                        if pl.pin.as_deref() != Some(h.as_str()) {
                            let k = pl.span.line - 1;
                            let (body, end) = split(&lines[k]);
                            lines[k] = sources::pinned(&body, &h) + &end;
                            let fr = &pl.fragment;
                            report.push(tr!("{name}: {fr} を sha256:{h} で固定しました", "{name}: pinned {fr} at sha256:{h}"));
                        }
                    }
                    // A cited article without a pin line gets one, after the last pin of its source.
                    let (anchor, indent) = match pins.last() {
                        Some(pl) => {
                            let l = &lines[pl.span.line - 1];
                            (pl.span.line - 1, l[..l.len() - l.trim_start().len()].to_string())
                        }
                        None => (s.span.line - 1, "  ".to_string()),
                    };
                    for fr in fragments(&f.ast, name, pins).into_iter().skip(pins.len()) {
                        let Some(file) = copies::fragment_file(*db, &fr) else { continue };
                        let Ok(bytes) = std::fs::read(cdir.join(&file)) else {
                            report.push(tr!("{name}: {fr} の写しが無いので固定できません。先に yuen source fetch を走らせます", "{name}: there is no copy of {fr} to pin; run yuen source fetch first"));
                            continue;
                        };
                        let h = sha256::short(&bytes);
                        let (_, end) = split(&lines[anchor]);
                        let end = if end.is_empty() { "\n".to_string() } else { end };
                        inserts.entry(anchor).or_default().push(format!("{indent}{} sha256:{h}{end}", fragment_word(&fr)));
                        report.push(tr!("{name}: 引いている {fr} の固定の行を足しました（sha256:{h}）", "{name}: added a pin line for the cited {fr} (sha256:{h})"));
                    }
                    if report.len() == said {
                        let n = pins.len();
                        report.push(if n == 1 {
                            tr!("{name}: 固定済みです", "{name}: already pinned")
                        } else {
                            tr!("{name}: {n} 条とも固定済みです", "{name}: all {n} articles already pinned")
                        });
                    }
                }
            }
        }
        let mut text = String::with_capacity(src.len() + 64);
        for (k, l) in lines.iter().enumerate() {
            text.push_str(l);
            if let Some(adds) = inserts.get(&k) {
                // The anchor may be the last line of a file with no line break at its end.
                if !l.ends_with('\n') {
                    text.push('\n');
                }
                for a in adds {
                    text.push_str(a);
                }
            }
        }
        if text != *src {
            texts.push((fi, text));
        }
        under(p, fi, report, &mut out);
    }
    (texts, Outcome { lines: out, changed: false })
}

// ── outdated ────────────────────────────────────────────────────────────────

/// Who an article or a file source reaches: the requirement versions that cite it, those
/// read from them in turn, and how many links and waivers each change will mark.
struct Reach {
    citing: Vec<usize>,
    derived: Vec<usize>,
    links: usize,
    waivers: usize,
}

fn reach(p: &Project, fi: usize, source: &str, fragment: Option<&str>) -> Reach {
    let mut citing = Vec::new();
    for r in 0..p.reqs.len() {
        if p.reqs[r].file != fi {
            continue;
        }
        let cites = p.decl(r).from.iter().any(|fl| match &fl.what {
            FromWhat::Cite { source: s, fragments, .. } => s == source && fragment.is_none_or(|fr| fragments.iter().any(|(x, _)| x == fr)),
            FromWhat::Req(_) => false,
        });
        if cites {
            citing.push(r);
        }
    }
    // Those read from them, and from those, in turn: their ends change too (DESIGN 4.1).
    let mut all = citing.clone();
    let mut derived = Vec::new();
    loop {
        let mut grew = false;
        for r in 0..p.reqs.len() {
            if all.contains(&r) {
                continue;
            }
            if p.names.from[r].iter().flatten().any(|t| all.contains(t)) {
                all.push(r);
                derived.push(r);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    // A requirement whose end changes marks every link it is an end of: its `from` lines (the
    // one citing what changed, and the others, whose lower end moved), its links, its waivers.
    let links = all.iter().map(|r| p.decl(*r).from.len() + p.decl(*r).links.len()).sum();
    let waivers = all.iter().map(|r| p.decl(*r).waivers.len()).sum();
    Reach { citing, derived, links, waivers }
}

fn reach_lines(p: &Project, re: &Reach, lines: &mut Vec<Text>) {
    let one = |r: usize| -> (String, String) {
        let (file, line) = p.req_place(r);
        let owner = p.decl(r).owner.as_ref().map(|o| o.0.clone()).unwrap_or_default();
        let me = p.req_label(r);
        (format!("{me}（持ち主 {owner}、{file}:{line}）"), format!("{me} (owned by {owner}, {file}:{line})"))
    };
    if re.citing.is_empty() {
        lines.push(tr!("  どの要件も引いていません", "  no requirement cites it"));
        return;
    }
    let (ja, en): (Vec<String>, Vec<String>) = re.citing.iter().map(|r| one(*r)).unzip();
    lines.push(tr!("  引いている要件: {}", "  cited by: {}", ja.join("、"); en.join(", ")));
    if !re.derived.is_empty() {
        let (ja, en): (Vec<String>, Vec<String>) = re.derived.iter().map(|r| one(*r)).unzip();
        lines.push(tr!("  それを元にした要件: {}", "  read from those: {}", ja.join("、"); en.join(", ")));
    }
    let (l, w) = (re.links, re.waivers);
    lines.push(match w {
        0 => tr!("  確かめ直すもの: リンク {l} 本", "  to look at again: {}", ; plural(l, "link", "links")),
        _ => tr!("  確かめ直すもの: リンク {l} 本と見送り {w} 件", "  to look at again: {} and {}", ; plural(l, "link", "links"), plural(w, "waiver", "waivers")),
    });
}

fn diff_lines(old: &str, new: &str, lines: &mut Vec<Text>) {
    let (d, more): (Vec<DiffLine>, usize) = crate::diff::unified(old, new);
    for l in d {
        let s = if l.op == '@' { format!("      {}", l.text) } else { format!("      {} {}", l.op, l.text) };
        lines.push(Text::same(s.trim_end().to_string()));
    }
    if more > 0 {
        lines.push(tr!("      （差分はほかに {more} 行あります）", "      ({more} more lines of the diff are not shown)"));
    }
}

/// The rules and calendars the links name that pin an article (DESIGN 3.3): they are to be
/// looked at when it changes, as the requirements that cite it are.
fn pinned_by(p: &Project, db: LawDb, id: &str, fragment: &str) -> Vec<String> {
    let mut files: Vec<crate::names::Name> = Vec::new();
    for n in p.names.links.iter().flatten().flatten() {
        let file = n.whole_file();
        if !files.contains(&file) {
            files.push(file);
        }
    }
    files.into_iter().filter(|f| crate::sources::pinned_by(p, f).iter().any(|x| x.db == db && x.id == id && x.fragment == fragment)).map(|f| f.text()).collect()
}

/// Where a source is read from, for `outdated`: one of the project's own, or one a rule or a
/// calendar pins, which is to be fetched and pinned again there.
enum Whose<'a> {
    Own,
    Borrowed(&'a crate::names::Name),
}

/// One law, as of its `asof`, against every revision that comes into force after it: the
/// articles (each with its copy's file), the copies' directory, and whose they are.
#[allow(clippy::too_many_arguments)]
fn law_outdated(p: &Project, fi: usize, name: &str, db: LawDb, id: &str, asof: &str, files: &[(String, String)], cdir: &Path, whose: Whose, net: &mut Net, lines: &mut Vec<Text>) -> Result<bool, Text> {
    let mut changed = false;
    let later = match db {
        LawDb::Egov => egov().later_revisions(id, asof).map_err(cap)?,
        LawDb::Ecfr => ecfr().amendments(id, asof, &files.iter().map(|(_, file)| file.trim_end_matches(".xml").to_string()).collect::<Vec<_>>()).map_err(cap)?.into_iter().map(|(d, _)| (d, String::new())).collect(),
    };
    if later.is_empty() {
        lines.push(match db {
            LawDb::Egov => tr!("{name}: {asof} より後に施行される版はありません", "{name}: no revision comes into force after {asof}"),
            LawDb::Ecfr => tr!("{name}: {asof} より後の改正はありません", "{name}: no amendment after {asof}"),
        });
        return Ok(false);
    }
    // Each date is held to the text of the date before it, starting from the copy, so an
    // amendment is said once, on the day it comes into force.
    let mut differs: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    for (fr, file) in files {
        let Ok(mut prev) = std::fs::read(cdir.join(file)) else {
            lines.push(match whose {
                Whose::Own => tr!("{name}: {fr} の写しが無いので比べられません。先に yuen source fetch を走らせます", "{name}: there is no copy of {fr} to compare; run yuen source fetch first"),
                Whose::Borrowed(n) => {
                    let t = n.text();
                    tr!("{name}: {t} の {fr} の写しが無いので比べられません", "{name}: there is no copy of {fr} of {t} to compare")
                }
            });
            continue;
        };
        for (date, _) in &later {
            let (xml, _) = net.article(db, id, date, file)?;
            if !same_text(&prev, &xml) {
                let (a, b) = (copies::xml_text(&String::from_utf8_lossy(&prev)), copies::xml_text(&String::from_utf8_lossy(&xml)));
                differs.entry(date.clone()).or_default().push((fr.clone(), a, b));
            }
            prev = xml;
        }
    }
    for (date, rid) in &later {
        let rev = if rid.is_empty() { Text::default() } else { tr!("（{rid}）", " ({rid})") };
        match differs.get(date) {
            None => lines.push(match db {
                LawDb::Egov => tr!("{name}: {date} 施行の版{}では、引いている条は変わりません", "{name}: the revision in force from {date}{} leaves the cited articles as they are", rev.ja; rev.en),
                LawDb::Ecfr => tr!("{name}: {date} の改正では、引いている条は変わりません", "{name}: the amendment of {date} leaves the cited sections as they are"),
            }),
            Some(frs) => {
                changed = true;
                let names: Vec<&str> = frs.iter().map(|(f, _, _)| f.as_str()).collect();
                lines.push(match db {
                    LawDb::Egov => tr!("{name}: {date} 施行の版{}で {} が変わります", "{name}: the revision in force from {date}{} changes {}", rev.ja, names.join("、"); rev.en, names.join(", ")),
                    LawDb::Ecfr => tr!("{name}: {date} の改正で {} が変わります", "{name}: the amendment of {date} changes {}", names.join("、"); names.join(", ")),
                });
                for (fr, a, b) in frs {
                    lines.push(tr!("  {fr} の本文の差分:", "  what changed in the text of {fr}:"));
                    diff_lines(a, b, lines);
                    reach_lines(p, &reach(p, fi, name, Some(fr)), lines);
                    let pinned = pinned_by(p, db, id, fr);
                    if !pinned.is_empty() {
                        lines.push(tr!("  固定している成果物: {}", "  pinned by: {}", pinned.join("、"); pinned.join(", ")));
                    }
                }
                lines.push(match whose {
                    Whose::Own => tr!(
                        "  asof を {date} に進めて yuen source fetch と yuen source pin を走らせると、yuen check がこれらに印を付けます",
                        "  move asof to {date}, then run yuen source fetch and yuen source pin; yuen check then marks these"
                    ),
                    Whose::Borrowed(n) => {
                        let (t, tool) = (n.text(), n.tool.word());
                        tr!(
                            "  {t} で asof を {date} に進めて {tool} source fetch と {tool} source pin を走らせると、yuen check がこれらに印を付けます",
                            "  in {t}, move asof to {date}, then run {tool} source fetch and {tool} source pin; yuen check then marks these"
                        )
                    }
                });
            }
        }
    }
    Ok(changed)
}

/// A file source against its `url` (DESIGN 14): unchanged, or what changed from the copy, and
/// who it reaches.
#[allow(clippy::too_many_arguments)]
fn file_outdated(p: &Project, fi: usize, name: &str, url: Option<&str>, pin: Option<&str>, copy_rel: &str, whose: Whose, lines: &mut Vec<Text>) -> Result<bool, Text> {
    let Some(url) = url else {
        lines.push(tr!("{name}: url が無いので、元が変わったかを問えません", "{name}: no url, so there is nothing to ask whether it moved on"));
        return Ok(false);
    };
    let body = curl(url)?;
    let h = sha256::short(&body);
    if pin == Some(h.as_str()) {
        lines.push(tr!("{name}: 変わっていません（{url} は sha256:{h} で、固定と同じ）", "{name}: unchanged ({url} is sha256:{h}, as pinned)"));
        return Ok(false);
    }
    let was = pin.map(|x| format!("sha256:{x}")).unwrap_or_else(|| "-".into());
    lines.push(tr!("{name}: {url} が変わりました（固定は {was}、いまは sha256:{h}）", "{name}: {url} moved on (pinned {was}, now sha256:{h})"));
    let copy = std::fs::read(p.root.join(copy_rel)).ok();
    if let (Some(old), Ok(new)) = (copy.as_deref().and_then(|b| std::str::from_utf8(b).ok()), std::str::from_utf8(&body))
        && old.len() <= 1 << 20
        && new.len() <= 1 << 20
    {
        let path = p.shown(copy_rel);
        lines.push(tr!("  写し {path} からの差分:", "  what changed from the copy {path}:"));
        diff_lines(old, new, lines);
    }
    reach_lines(p, &reach(p, fi, name, None), lines);
    lines.push(match whose {
        Whose::Own => tr!(
            "  読んでから yuen source fetch で取り直して yuen source pin で固定すると、yuen check がこれらに印を付けます",
            "  read it, then yuen source fetch takes it and yuen source pin pins it; yuen check then marks these"
        ),
        Whose::Borrowed(n) => {
            let (t, tool) = (n.text(), n.tool.word());
            tr!(
                "  読んでから {t} で {tool} source fetch と {tool} source pin を走らせると、yuen check がこれらに印を付けます",
                "  read it, then run {tool} source fetch and {tool} source pin in {t}; yuen check then marks these"
            )
        }
    });
    Ok(true)
}

/// `yuen source outdated`. A borrowed source is asked about as its own are, from the pins and
/// the copies of the rule or the calendar it is borrowed from (`Sources`); it is fetched and
/// pinned again there.
pub fn outdated(p: &Project) -> Result<Outcome, Text> {
    let mut out = Vec::new();
    let mut changed = false;
    let mut net = Net::default();
    let resolved = crate::sources::check_sources(p).0;
    for (fi, f) in p.files.iter().enumerate() {
        let req_dir = f.abs.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut lines = Vec::new();
        for (si, s) in f.ast.sources.iter().enumerate() {
            let name = &s.name;
            match &s.kind {
                SourceKind::Borrowed { .. } => match resolved.get(fi, name) {
                    Some(crate::sources::Resolved::Law { db, id, asof, dir, articles, borrowed: Some(n), .. }) => {
                        let files: Vec<(String, String)> = articles.iter().filter_map(|a| copies::fragment_file(*db, &a.fragment).map(|file| (a.fragment.clone(), file))).collect();
                        changed |= law_outdated(p, fi, name, *db, id, asof, &files, dir, Whose::Borrowed(n), &mut net, &mut lines)?;
                    }
                    Some(crate::sources::Resolved::File { name: copy, url, pin, borrowed: Some(n), .. }) => {
                        changed |= file_outdated(p, fi, name, url.as_deref(), pin.as_deref(), &copy.path, Whose::Borrowed(n), &mut lines)?;
                    }
                    Some(crate::sources::Resolved::NoPort { name: n }) => {
                        let (t, tool) = (n.text(), n.tool.word());
                        let at = format!("{}:{}", f.display, s.span.line);
                        let cmd = crate::check::with_ritsu(p);
                        return Err(tr!(
                            "この yuen は {tool} の出典を読めません: {t}（{at}）。ほかの言語を読むところは、すべての言語をつないだ `{cmd}` のように ritsu で走らせます",
                            "this yuen cannot read {tool} sources: {t} ({at}); run it with every language joined, through ritsu: `{cmd}`"
                        ));
                    }
                    _ => lines.push(tr!("{name}: 借りた出典を読めないので、問えません（理由は yuen check が言います）", "{name}: the borrowed source cannot be read, so it cannot be asked about (yuen check says why)")),
                },
                SourceKind::File { url, pin, .. } => {
                    let Some(n) = &p.names.sources[fi][si] else { continue };
                    changed |= file_outdated(p, fi, name, url.as_deref(), pin.as_deref(), &n.path, Whose::Own, &mut lines)?;
                }
                SourceKind::Law { db, id, asof, pins } => {
                    let asof = asof.to_string();
                    let frs = fragments(&f.ast, name, pins);
                    let files: Vec<(String, String)> = frs.iter().filter_map(|fr| copies::fragment_file(*db, fr).map(|file| (fr.clone(), file))).collect();
                    let cdir = req_dir.join(copies::copy_dir(id, &asof));
                    changed |= law_outdated(p, fi, name, *db, id, &asof, &files, &cdir, Whose::Own, &mut net, &mut lines)?;
                }
            }
        }
        under(p, fi, lines, &mut out);
    }
    Ok(Outcome { lines: out, changed })
}
