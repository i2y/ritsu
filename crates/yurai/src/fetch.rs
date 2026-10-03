//! `yurai source fetch | pin | outdated` (DESIGN 14, PLAN C.12): the copies of a project's own
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
//! TLS of yurai's own, no dependency); `check` never calls them. They read a project as far
//! as its names, and check nothing more: a copy that is missing, unpinned or unlike its pin
//! is what they are for. A source borrowed from rulec or koyomi is that tool's to fetch and
//! pin.

use crate::ast::*;
use crate::copies;
use crate::diag::DiffLine;
use crate::i18n::{Text, plural};
use crate::project::Project;
use crate::sha256;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// What a command did or found, a line each, and whether an original moved on (`outdated`'s
/// exit code).
pub struct Outcome {
    pub lines: Vec<Text>,
    pub changed: bool,
}

/// e-Gov law API v2: `YURAI_EGOV`, else e-Gov itself.
pub fn egov() -> String {
    base("YURAI_EGOV", "https://laws.e-gov.go.jp/api/2")
}

/// The eCFR's versioner API: `YURAI_ECFR`, else the eCFR itself.
pub fn ecfr() -> String {
    base("YURAI_ECFR", "https://www.ecfr.gov/api/versioner/v1")
}

fn base(var: &str, default: &str) -> String {
    std::env::var(var).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| default.to_string()).trim_end_matches('/').to_string()
}

/// One run of curl: the body, what curl said when it failed, or that curl itself cannot be run.
fn curl_once(url: &str) -> Result<Vec<u8>, Result<String, Text>> {
    let out = match Command::new("curl").args(["-fsSL", "--max-time", "120", url]).output() {
        Ok(o) => o,
        Err(e) => return Err(Err(tr!("curl を走らせられません（PATH にありますか）: {e}", "Cannot run curl (is it on the PATH?): {e}"))),
    };
    if out.status.success() { Ok(out.stdout) } else { Err(Ok(String::from_utf8_lossy(&out.stderr).trim().to_string())) }
}

/// One request: three tries over HTTP and HTTPS, two seconds before the second and four
/// before the third (a busy server refuses now and then; a scheduled job that stops on one
/// refusal is one nobody reads), and one for a `file://` URL (DESIGN 14).
pub fn curl(url: &str) -> Result<Vec<u8>, Text> {
    let tries = if url.starts_with("http://") || url.starts_with("https://") { 3 } else { 1 };
    let mut last = String::new();
    for attempt in 0..tries {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
        }
        match curl_once(url) {
            Ok(b) => return Ok(b),
            Err(Ok(e)) => last = e,
            Err(Err(no_curl)) => return Err(no_curl),
        }
    }
    Err(tr!("{url} を取れません（{tries} 回試しました）: {last}", "Cannot fetch {url} (tried {tries} times): {last}"))
}

fn json_of(url: &str) -> Result<Value, Text> {
    let body = curl(url)?;
    serde_json::from_slice(&body).map_err(|e| tr!("{url} のレスポンスが JSON として読めません: {e}", "The response of {url} is not JSON: {e}"))
}

/// Whether two copies say the same: their text, not their markup (rulec's §15.71).
pub fn same_text(a: &[u8], b: &[u8]) -> bool {
    copies::xml_text(&String::from_utf8_lossy(a)) == copies::xml_text(&String::from_utf8_lossy(b))
}

// ── Where an article is ─────────────────────────────────────────────────────

/// What e-Gov's `elm` asks for, from the file an article is copied into: the element path,
/// with `[1]` for an appendix table, and for an amending law's supplementary provisions the
/// law's number, whose position e-Gov alone knows (rulec's `Fragment`).
struct Element {
    /// With `{k}` where the position of an amending law's supplementary provisions goes.
    template: String,
    amend: Option<String>,
}

fn element(file: &str) -> Element {
    let stem = file.trim_end_matches(".xml");
    if let Some(rest) = stem.strip_prefix("AppdxTable_") {
        return Element { template: format!("AppdxTable[{rest}]"), amend: None };
    }
    if let Some(rest) = stem.strip_prefix("SupplProvision_") {
        let (num, tail) = match rest.find("-Article_") {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        return Element { template: format!("SupplProvision[{{k}}]{tail}"), amend: Some(num.to_string()) };
    }
    Element { template: stem.to_string(), amend: None }
}

fn query(s: &str) -> String {
    s.replace('[', "%5B").replace(']', "%5D")
}

/// The supplementary provisions of a law in document order, each with the amending law it
/// came with (rulec's `suppl_ordinals`): what `SupplProvision[k]` counts.
fn suppl_ordinals(xml: &str) -> Vec<Option<String>> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<SupplProvision") {
        let after = &rest[i + "<SupplProvision".len()..];
        if !after.starts_with([' ', '>', '\n', '\t', '/']) {
            rest = after;
            continue;
        }
        let end = after.find('>').unwrap_or(after.len());
        let tag = &after[..end];
        out.push(tag.find("AmendLawNum=\"").map(|p| {
            let v = &tag[p + "AmendLawNum=\"".len()..];
            v[..v.find('"').unwrap_or(v.len())].to_string()
        }));
        rest = &after[end..];
    }
    out
}

/// What a law's `law_data` holds: the XML inside `law_full_text`, and the revision.
fn law_xml(j: &Value, url: &str) -> Result<(Vec<u8>, String), Text> {
    let b64 = j.get("law_full_text").and_then(|x| x.as_str()).ok_or_else(|| tr!("{url} のレスポンスに law_full_text がありません", "The response of {url} has no law_full_text"))?;
    let xml = crate::base64::decode(b64).ok_or_else(|| tr!("{url} の law_full_text が base64 として読めません", "The law_full_text of {url} is not base64"))?;
    let rev = j.get("revision_info").and_then(|r| r.get("law_revision_id")).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Ok((xml, rev))
}

/// What the network is asked for one command: the positions of supplementary provisions, a
/// whole law per date at most once.
#[derive(Default)]
struct Net {
    suppl: BTreeMap<String, Vec<Option<String>>>,
}

impl Net {
    fn suppl_index(&mut self, id: &str, asof: &str, amend: &str) -> Result<usize, Text> {
        let key = format!("{id}@{asof}");
        if !self.suppl.contains_key(&key) {
            let url = format!("{}/law_data/{id}?asof={asof}&law_full_text_format=xml", egov());
            let (xml, _) = law_xml(&json_of(&url)?, &url)?;
            self.suppl.insert(key.clone(), suppl_ordinals(&String::from_utf8_lossy(&xml)));
        }
        self.suppl[&key]
            .iter()
            .position(|n| n.as_deref() == Some(amend))
            .map(|p| p + 1)
            .ok_or_else(|| tr!("法令 {id} の {asof} 時点に、附則（{amend}）がありません", "The law {id} as of {asof} has no supplementary provisions of {amend}"))
    }

    /// An article of a law as of a date, and the revision it came from (empty for the eCFR,
    /// whose version is the date asked for).
    fn article(&mut self, db: LawDb, id: &str, asof: &str, file: &str) -> Result<(Vec<u8>, String), Text> {
        match db {
            LawDb::Egov => {
                let e = element(file);
                let elm = match &e.amend {
                    Some(a) => e.template.replace("{k}", &self.suppl_index(id, asof, a)?.to_string()),
                    None => e.template.clone(),
                };
                let url = format!("{}/law_data/{id}?asof={asof}&elm={}&law_full_text_format=xml", egov(), query(&elm));
                law_xml(&json_of(&url)?, &url)
            }
            LawDb::Ecfr => {
                let (title, part) = cfr_id(id)?;
                let section = file.trim_end_matches(".xml");
                let url = format!("{}/full/{asof}/title-{title}.xml?part={part}&section={section}", ecfr());
                Ok((curl(&url)?, String::new()))
            }
        }
    }
}

/// The title and the part of a CFR id: `29 CFR 1910` is title 29, part 1910.
fn cfr_id(id: &str) -> Result<(String, String), Text> {
    let words: Vec<&str> = id.split_whitespace().collect();
    match words.as_slice() {
        [t, c, part] if c.eq_ignore_ascii_case("cfr") && t.chars().all(|x| x.is_ascii_digit()) && part.chars().all(|x| x.is_ascii_alphanumeric()) => Ok((t.to_string(), part.to_string())),
        _ => Err(tr!("CFR の ID は `29 CFR 1910` の形（title と part）で書きます: `{id}`", "A CFR id is written `29 CFR 1910`, a title and a part: `{id}`")),
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

/// `yurai source fetch`.
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
                        "{name}: {tool} のファイルから借りた出典なので、yurai は取りません。{tool} source fetch で取ります",
                        "{name}: borrowed from a {tool} file, so yurai does not fetch it; {tool} source fetch does"
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
                            "  固定は sha256:{pn} です。何が変わったかを読んでから、yurai source pin で固定し直します",
                            "  the pin is sha256:{pn}; read what changed, then pin it again with yurai source pin"
                        ),
                        None => tr!("  固定がありません。yurai source pin で固定します", "  it has no pin; yurai source pin writes one"),
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

/// The byte offset of the `sha256:` of a line and of the `#` that starts its comment, each
/// outside the strings of the line (koyomi's).
fn marks(line: &str) -> (Option<usize>, Option<usize>) {
    let mut in_str = false;
    let mut escaped = false;
    let mut pin = None;
    for (i, c) in line.char_indices() {
        if in_str {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '#' => return (pin, Some(i)),
            's' if pin.is_none() && line[i..].starts_with("sha256:") => pin = Some(i),
            _ => {}
        }
    }
    (pin, None)
}

/// The line with `sha256:<pin>` in place of the digits it has, or with ` sha256:<pin>` added
/// after its last word (before a comment). Nothing else on the line changes.
pub fn pinned(line: &str, pin: &str) -> String {
    let (at, comment) = marks(line);
    if let Some(i) = at {
        let start = i + "sha256:".len();
        let end = start + line[start..].bytes().take_while(|b| b.is_ascii_hexdigit()).count();
        return format!("{}{pin}{}", &line[..start], &line[end..]);
    }
    let body_end = comment.unwrap_or(line.len());
    let content = line[..body_end].trim_end();
    format!("{content} sha256:{pin}{}", &line[content.len()..])
}

/// An article as a pin line writes it: bare when it is a name, else in quotes (`"§1910.157"`).
fn fragment_word(fr: &str) -> String {
    if fr.chars().all(|c| c.is_alphanumeric() || c == '_') { fr.to_string() } else { crate::names::quote(fr) }
}

/// `yurai source pin`: the new text of each `.req` that changes, and what was done.
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
                        "{name}: {tool} のファイルから借りた出典なので、yurai は固定しません。{tool} source pin で固定します",
                        "{name}: borrowed from a {tool} file, so yurai does not pin it; {tool} source pin does"
                    ));
                }
                SourceKind::File { pin, .. } => {
                    let Some(n) = &p.names.sources[fi][si] else { continue };
                    let path = p.shown(&n.path);
                    let Ok(bytes) = std::fs::read(p.root.join(&n.path)) else {
                        report.push(tr!("{name}: 写し {path} が無いので固定できません。先に yurai source fetch を走らせます", "{name}: there is no copy {path} to pin; run yurai source fetch first"));
                        continue;
                    };
                    let h = sha256::short(&bytes);
                    if pin.as_deref() == Some(h.as_str()) {
                        report.push(tr!("{name}: sha256:{h} で固定済みです", "{name}: already pinned at sha256:{h}"));
                        continue;
                    }
                    let k = s.span.line - 1;
                    let (body, end) = split(&lines[k]);
                    lines[k] = pinned(&body, &h) + &end;
                    report.push(tr!("{name}: sha256:{h} で固定しました", "{name}: pinned at sha256:{h}"));
                }
                SourceKind::Law { db, id, asof, pins } => {
                    let cdir = req_dir.join(copies::copy_dir(id, &asof.to_string()));
                    let said = report.len();
                    for pl in pins {
                        let Some(file) = copies::fragment_file(*db, &pl.fragment) else { continue };
                        let Ok(bytes) = std::fs::read(cdir.join(&file)) else {
                            let fr = &pl.fragment;
                            report.push(tr!("{name}: {fr} の写しが無いので固定できません。先に yurai source fetch を走らせます", "{name}: there is no copy of {fr} to pin; run yurai source fetch first"));
                            continue;
                        };
                        let h = sha256::short(&bytes);
                        if pl.pin.as_deref() != Some(h.as_str()) {
                            let k = pl.span.line - 1;
                            let (body, end) = split(&lines[k]);
                            lines[k] = pinned(&body, &h) + &end;
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
                            report.push(tr!("{name}: {fr} の写しが無いので固定できません。先に yurai source fetch を走らせます", "{name}: there is no copy of {fr} to pin; run yurai source fetch first"));
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

/// The revisions of a law enforced after `asof`, as (enforcement date, revision id), one a day.
fn later_revisions(id: &str, asof: &str) -> Result<Vec<(String, String)>, Text> {
    let j = json_of(&format!("{}/law_revisions/{id}", egov()))?;
    let mut v = Vec::new();
    if let Some(revs) = j.get("revisions").and_then(|r| r.as_array()) {
        for r in revs {
            let date = r.get("amendment_enforcement_date").and_then(|x| x.as_str()).unwrap_or("");
            let rid = r.get("law_revision_id").and_then(|x| x.as_str()).unwrap_or("");
            if crate::date::Day::parse(date).is_some() && date > asof {
                v.push((date.to_string(), rid.to_string()));
            }
        }
    }
    v.sort();
    // Several revisions can come into force on one day; the text as of that day is one.
    v.dedup_by(|a, b| a.0 == b.0);
    Ok(v)
}

/// The days after `asof` on which a cited section of the CFR was amended in substance (the
/// eCFR says whether a version only moved the markup), as (date, "").
fn ecfr_amendments(id: &str, asof: &str, sections: &[String]) -> Result<Vec<(String, String)>, Text> {
    let (title, part) = cfr_id(id)?;
    let mut out: Vec<(String, String)> = Vec::new();
    for s in sections {
        let j = json_of(&format!("{}/versions/title-{title}.json?part={part}&section={s}", ecfr()))?;
        for v in j.get("content_versions").and_then(|x| x.as_array()).into_iter().flatten() {
            let date = v.get("amendment_date").and_then(|x| x.as_str()).unwrap_or("");
            let substantive = v.get("substantive").and_then(|x| x.as_bool()).unwrap_or(false);
            if substantive && crate::date::Day::parse(date).is_some() && date > asof {
                out.push((date.to_string(), String::new()));
            }
        }
    }
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    Ok(out)
}

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

/// `yurai source outdated`.
pub fn outdated(p: &Project) -> Result<Outcome, Text> {
    let mut out = Vec::new();
    let mut changed = false;
    let mut net = Net::default();
    for (fi, f) in p.files.iter().enumerate() {
        let req_dir = f.abs.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut lines = Vec::new();
        for (si, s) in f.ast.sources.iter().enumerate() {
            let name = &s.name;
            match &s.kind {
                SourceKind::Borrowed { .. } => {}
                SourceKind::File { url, pin, .. } => {
                    let Some(n) = &p.names.sources[fi][si] else { continue };
                    let Some(url) = url else {
                        lines.push(tr!("{name}: url が無いので、元が変わったかを問えません", "{name}: no url, so there is nothing to ask whether it moved on"));
                        continue;
                    };
                    let body = curl(url)?;
                    let h = sha256::short(&body);
                    if pin.as_deref() == Some(h.as_str()) {
                        lines.push(tr!("{name}: 変わっていません（{url} は sha256:{h} で、固定と同じ）", "{name}: unchanged ({url} is sha256:{h}, as pinned)"));
                        continue;
                    }
                    changed = true;
                    let was = pin.as_deref().map(|x| format!("sha256:{x}")).unwrap_or_else(|| "-".into());
                    lines.push(tr!("{name}: {url} が変わりました（固定は {was}、いまは sha256:{h}）", "{name}: {url} moved on (pinned {was}, now sha256:{h})"));
                    let copy = std::fs::read(p.root.join(&n.path)).ok();
                    if let (Some(old), Ok(new)) = (copy.as_deref().and_then(|b| std::str::from_utf8(b).ok()), std::str::from_utf8(&body))
                        && old.len() <= 1 << 20
                        && new.len() <= 1 << 20
                    {
                        let path = p.shown(&n.path);
                        lines.push(tr!("  写し {path} からの差分:", "  what changed from the copy {path}:"));
                        diff_lines(old, new, &mut lines);
                    }
                    reach_lines(p, &reach(p, fi, name, None), &mut lines);
                    lines.push(tr!(
                        "  読んでから yurai source fetch で取り直して yurai source pin で固定すると、yurai check がこれらに印を付けます",
                        "  read it, then yurai source fetch takes it and yurai source pin pins it; yurai check then marks these"
                    ));
                }
                SourceKind::Law { db, id, asof, pins } => {
                    let asof = asof.to_string();
                    let frs = fragments(&f.ast, name, pins);
                    let files: Vec<(String, String)> = frs.iter().filter_map(|fr| copies::fragment_file(*db, fr).map(|file| (fr.clone(), file))).collect();
                    let later = match db {
                        LawDb::Egov => later_revisions(id, &asof)?,
                        LawDb::Ecfr => ecfr_amendments(id, &asof, &files.iter().map(|(_, file)| file.trim_end_matches(".xml").to_string()).collect::<Vec<_>>())?,
                    };
                    if later.is_empty() {
                        lines.push(match db {
                            LawDb::Egov => tr!("{name}: {asof} より後に施行される版はありません", "{name}: no revision comes into force after {asof}"),
                            LawDb::Ecfr => tr!("{name}: {asof} より後の改正はありません", "{name}: no amendment after {asof}"),
                        });
                        continue;
                    }
                    let cdir = req_dir.join(copies::copy_dir(id, &asof));
                    // Each date is held to the text of the date before it, starting from the
                    // copy, so an amendment is said once, on the day it comes into force.
                    let mut differs: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
                    for (fr, file) in &files {
                        let Ok(mut prev) = std::fs::read(cdir.join(file)) else {
                            lines.push(tr!("{name}: {fr} の写しが無いので比べられません。先に yurai source fetch を走らせます", "{name}: there is no copy of {fr} to compare; run yurai source fetch first"));
                            continue;
                        };
                        for (date, _) in &later {
                            let (xml, _) = net.article(*db, id, date, file)?;
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
                                    diff_lines(a, b, &mut lines);
                                    reach_lines(p, &reach(p, fi, name, Some(fr)), &mut lines);
                                }
                                lines.push(tr!(
                                    "  asof を {date} に進めて yurai source fetch と yurai source pin を走らせると、yurai check がこれらに印を付けます",
                                    "  move asof to {date}, then run yurai source fetch and yurai source pin; yurai check then marks these"
                                ));
                            }
                        }
                    }
                }
            }
        }
        under(p, fi, lines, &mut out);
    }
    Ok(Outcome { lines: out, changed })
}

/// The `.req` files of a project that borrow a source: what `outdated` cannot ask about yet,
/// since a borrowed source's pins are read from its tool's JSON (PLAN C.7).
pub fn borrowed(p: &Project) -> Option<(usize, usize, String)> {
    for (fi, f) in p.files.iter().enumerate() {
        for (si, s) in f.ast.sources.iter().enumerate() {
            if let SourceKind::Borrowed { .. } = &s.kind
                && let Some(n) = &p.names.sources[fi][si]
            {
                return Some((fi, s.span.line, n.text()));
            }
        }
    }
    None
}
