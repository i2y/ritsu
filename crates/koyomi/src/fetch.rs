//! `koyomi source fetch | pin | outdated` (DESIGN 9): the copies of a file's sources.
//!
//! - `fetch` takes each table from its `url` and writes the bytes as they come, and takes the
//!   articles a law source pins or cites from e-Gov law API v2, one `law_data` request each.
//! - `pin` writes the first 16 digits of each copy's SHA-256 into the `.cal`, and changes
//!   nothing else in it.
//! - `outdated` asks whether the original moved on: a table's `url` against the pin, with the
//!   rows that differ; a law's revisions enforced after its `asof`, each held to the copy by
//!   its text.
//!
//! These are the only commands that read the network, through `curl` as a child process (no
//! TLS of our own, no dependency). `check` never calls them. The e-Gov API is at `KOYOMI_EGOV`
//! when it is set, which is how the tests serve it from a local server.

use crate::ast::{Covers, File, Format, SourceKind};
use crate::date::Day;
use crate::holidays::{self, Row};
use crate::sources::{copy_dir, elm};
use ritsu_base::sha256;
use ritsu_base::sources::{self, Egov};
use ritsu_base::text::{Text, count};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a command did or found, a line each, and whether something moved on (`outdated`'s
/// exit code).
pub struct Outcome {
    pub lines: Vec<Text>,
    pub changed: bool,
}

/// Where e-Gov law API v2 is: `KOYOMI_EGOV`, else e-Gov itself. The requests, the retries and
/// the reading of a reply are ritsu-base's ([`ritsu_base::sources`]).
pub fn egov() -> Egov {
    Egov { base: sources::base_url("KOYOMI_EGOV", sources::EGOV) }
}

/// The articles of a law source: the pinned ones, then the cited ones that have no pin yet.
fn fragments(f: &File, name: &str, pins: &[crate::ast::LawPin]) -> Vec<String> {
    let mut out: Vec<String> = pins.iter().map(|p| p.fragment.clone()).collect();
    for c in crate::sources::citations(f) {
        if c.source == name {
            for (fr, _) in &c.fragments {
                if !out.contains(fr) {
                    out.push(fr.clone());
                }
            }
        }
    }
    out
}

fn short(b: &[u8]) -> String {
    sha256::short(b)
}

/// `koyomi source fetch`.
pub fn fetch(f: &File, dir: &Path) -> Result<Outcome, Text> {
    let mut lines = Vec::new();
    let mut changed = false;
    for s in &f.sources {
        let name = &s.name;
        match &s.kind {
            SourceKind::File { path, url, pin, .. } => {
                let Some(url) = url else {
                    lines.push(tr!("{name}: url が無いので取れません（`url \"…\"` を足すと取れます）", "{name}: no url, so the copy cannot be fetched (add `url \"…\"`)"));
                    continue;
                };
                let body = sources::curl(url)?;
                let dest = dir.join(path);
                let before = std::fs::read(&dest).ok();
                let h = short(&body);
                if before.as_deref() == Some(body.as_slice()) {
                    lines.push(tr!("{name}: 写し {path} は変わっていません（sha256:{h}）", "{name}: the copy {path} is unchanged (sha256:{h})"));
                    continue;
                }
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| tr!("{} を作れません: {e}", "cannot create {}: {e}", parent.display()))?;
                }
                std::fs::write(&dest, &body).map_err(|e| tr!("{} に書けません: {e}", "cannot write {}: {e}", dest.display()))?;
                changed |= before.is_some();
                let what = if before.is_some() {
                    tr!("{name}: 写し {path} を取り直しました（sha256:{h}）", "{name}: fetched the copy {path} again (sha256:{h})")
                } else {
                    tr!("{name}: 写し {path} を取りました（sha256:{h}）", "{name}: fetched the copy {path} (sha256:{h})")
                };
                lines.push(what);
                lines.push(match pin {
                    Some(p) if *p == h => tr!("  固定と合っています", "  it matches the pin"),
                    Some(p) => tr!(
                        "  固定は sha256:{p} です。何が変わったかを読んでから（git diff など）、koyomi source pin で固定し直してください",
                        "  the pin is sha256:{p}; read what changed (git diff, say), then pin it again with koyomi source pin"
                    ),
                    None => tr!("  固定がありません。koyomi source pin で固定してください", "  it has no pin; koyomi source pin writes one"),
                });
            }
            SourceKind::Law { id, asof, pins } => {
                let cdir = copy_dir(dir, id, *asof);
                for fr in fragments(f, name, pins) {
                    let Some(e) = elm(&fr) else {
                        lines.push(tr!("{name}: `{fr}` は条・項・号の書き方になっていないので取れません", "{name}: `{fr}` is not written as an article, a paragraph or an item, so it cannot be fetched"));
                        continue;
                    };
                    let (xml, rev) = egov().law_data(id, &asof.to_string(), Some(&e))?;
                    std::fs::create_dir_all(&cdir).map_err(|err| tr!("{} を作れません: {err}", "cannot create {}: {err}", cdir.display()))?;
                    let dest = cdir.join(format!("{e}.xml"));
                    let before = std::fs::read(&dest).ok();
                    // A copy whose text did not change keeps its bytes, and so its pin: e-Gov
                    // rewrites the markup of articles no amendment touched.
                    let same = before.as_deref().is_some_and(|b| sources::same_text(b, &xml));
                    if !same {
                        std::fs::write(&dest, &xml).map_err(|err| tr!("{} に書けません: {err}", "cannot write {}: {err}", dest.display()))?;
                    }
                    if !rev.is_empty() {
                        let rp = cdir.join("revision.txt");
                        let want = format!("{rev}\n");
                        if std::fs::read_to_string(&rp).ok().as_deref() != Some(want.as_str()) {
                            std::fs::write(&rp, want).map_err(|err| tr!("{} に書けません: {err}", "cannot write {}: {err}", rp.display()))?;
                        }
                    }
                    let h = short(if same { before.as_deref().unwrap() } else { &xml });
                    lines.push(match (&before, same) {
                        (None, _) => tr!("{name}: {fr} を取りました（sha256:{h}）", "{name}: fetched {fr} (sha256:{h})"),
                        (Some(_), true) => tr!("{name}: {fr} の本文は変わっていません（sha256:{h}）", "{name}: the text of {fr} is unchanged (sha256:{h})"),
                        (Some(_), false) => {
                            changed = true;
                            tr!(
                                "{name}: {fr} の本文が変わりました（いま sha256:{h}）。引いている行を読み直してから、固定し直してください",
                                "{name}: the text of {fr} changed (now sha256:{h}); reread the lines that cite it, then pin it"
                            )
                        }
                    });
                }
            }
        }
    }
    Ok(Outcome { lines, changed })
}

// ── pin ──────────────────────────────────────────────────────────────────────

/// What `pin` reads of a table, for the line that says it is pinned.
fn table_summary(bytes: &[u8], format: &Option<(Format, crate::ast::Span)>, covers: &Option<(Covers, crate::ast::Span)>) -> Option<Text> {
    let rows = match format.as_ref()?.0.clone() {
        Format::Csv { shift_jis } => holidays::read_csv(bytes, shift_jis).ok()?,
        Format::GovUk { division } => holidays::read_govuk(bytes, &division).ok()?,
    };
    let n = count(rows.len() as u64);
    Some(match &covers.as_ref()?.0 {
        Covers::Range(a, b) => tr!("表は {n} 行、covers {a}..{b}", "the table has {n} rows, covers {a}..{b}"),
        Covers::ListedYears => {
            let (y0, y1) = (rows.first()?.day.year(), rows.last()?.day.year());
            tr!("表は {n} 行、covers listed years = {y0}-01-01..{y1}-12-31", "the table has {n} rows, covers listed years = {y0}-01-01..{y1}-12-31")
        }
    })
}

/// `koyomi source pin`: the new text of the `.cal`, and what was done. Only the digits after
/// `sha256:` change (or a pin is added where there was none): every other byte of the file,
/// its line endings among them, stays.
pub fn pin(f: &File, dir: &Path, src: &str) -> Result<(String, Outcome), Text> {
    // The lines, each with its ending, so that joining them gives the file back.
    let mut lines: Vec<String> = src.split_inclusive('\n').map(|s| s.to_string()).collect();
    let mut inserts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut report = Vec::new();
    let mut changed = false;
    let split = |l: &str| -> (String, String) {
        let body = l.trim_end_matches(['\n', '\r']);
        (body.to_string(), l[body.len()..].to_string())
    };
    for s in &f.sources {
        let name = &s.name;
        match &s.kind {
            SourceKind::File { path, pin, format, covers, .. } => {
                let Ok(bytes) = std::fs::read(dir.join(path)) else {
                    report.push(tr!("{name}: 写し {path} が無いので固定できません。先に koyomi source fetch を走らせてください", "{name}: there is no copy {path} to pin; run koyomi source fetch first"));
                    continue;
                };
                let h = short(&bytes);
                let summary = table_summary(&bytes, format, covers).map(|t| tr!("。{}", "; {}", t.ja; t.en)).unwrap_or_default();
                if pin.as_deref() == Some(h.as_str()) {
                    report.push(tr!("{name}: sha256:{h} で固定済みです{}", "{name}: already pinned at sha256:{h}{}", summary.ja; summary.en));
                    continue;
                }
                let k = s.span.line - 1;
                let (body, end) = split(&lines[k]);
                lines[k] = sources::pinned(&body, &h) + &end;
                changed = true;
                report.push(tr!("{name}: sha256:{h} で固定しました{}", "{name}: pinned sha256:{h}{}", summary.ja; summary.en));
            }
            SourceKind::Law { id, asof, pins } => {
                let cdir = copy_dir(dir, id, *asof);
                let said = report.len();
                for p in pins {
                    let Some(e) = elm(&p.fragment) else { continue };
                    let Ok(bytes) = std::fs::read(cdir.join(format!("{e}.xml"))) else {
                        report.push(tr!("{name}: {} の写しが無いので固定できません。先に koyomi source fetch を走らせてください", "{name}: there is no copy of {} to pin; run koyomi source fetch first", p.fragment));
                        continue;
                    };
                    let h = short(&bytes);
                    if p.pin.as_deref() != Some(h.as_str()) {
                        let k = p.span.line - 1;
                        let (body, end) = split(&lines[k]);
                        lines[k] = sources::pinned(&body, &h) + &end;
                        changed = true;
                        report.push(tr!("{name}: {} を sha256:{h} で固定しました", "{name}: pinned {} at sha256:{h}", p.fragment));
                    }
                }
                // A cited article with no pin line gets one, after the last pin of its source.
                let (anchor, indent) = match pins.last() {
                    Some(p) => {
                        let l = &lines[p.span.line - 1];
                        (p.span.line - 1, l[..l.len() - l.trim_start().len()].to_string())
                    }
                    None => (s.span.line - 1, "  ".to_string()),
                };
                for fr in fragments(f, name, pins).into_iter().skip(pins.len()) {
                    let Some(e) = elm(&fr) else { continue };
                    let Ok(bytes) = std::fs::read(cdir.join(format!("{e}.xml"))) else {
                        report.push(tr!("{name}: {fr} の写しが無いので固定できません。先に koyomi source fetch を走らせてください", "{name}: there is no copy of {fr} to pin; run koyomi source fetch first"));
                        continue;
                    };
                    let h = short(&bytes);
                    let (_, end) = split(&lines[anchor]);
                    let end = if end.is_empty() { "\n".to_string() } else { end };
                    inserts.entry(anchor).or_default().push(format!("{indent}{fr} sha256:{h}{end}"));
                    changed = true;
                    report.push(tr!("{name}: 引いている {fr} の固定の行を足しました（sha256:{h}）", "{name}: added a pin line for the cited {fr} (sha256:{h})"));
                }
                if report.len() == said {
                    report.push(tr!("{name}: {} 条とも固定済みです", "{name}: all {} articles already pinned", pins.len()));
                }
            }
        }
    }
    let mut out = String::with_capacity(src.len() + 64);
    for (k, l) in lines.iter().enumerate() {
        if let Some(adds) = inserts.get(&k) {
            // The anchor may be the last line of a file with no final newline.
            if !l.ends_with('\n') {
                out.push_str(l);
                out.push('\n');
                for a in adds {
                    out.push_str(a);
                }
                continue;
            }
            out.push_str(l);
            for a in adds {
                out.push_str(a);
            }
            continue;
        }
        out.push_str(l);
    }
    Ok((out, Outcome { lines: report, changed }))
}

// ── outdated ─────────────────────────────────────────────────────────────────

/// Up to six days, each with its name, and how many more.
fn some_days(rows: &[(Day, String)]) -> Text {
    let shown: Vec<String> = rows.iter().take(6).map(|(d, n)| if n.is_empty() { d.to_string() } else { format!("{d} {n}") }).collect();
    let more = rows.len().saturating_sub(6);
    if more > 0 {
        tr!("{}、ほか {more} 日", "{}, and {more} more", shown.join("、"); shown.join(", "))
    } else {
        tr!("{}", "{}", shown.join("、"); shown.join(", "))
    }
}

/// `2027 年の 17 日`, by year.
fn by_year(rows: &[(Day, String)]) -> Text {
    let mut years: BTreeMap<i32, usize> = BTreeMap::new();
    for (d, _) in rows {
        *years.entry(d.year()).or_default() += 1;
    }
    let ja: Vec<String> = years.iter().map(|(y, n)| format!("{y} 年の {n} 日")).collect();
    let en: Vec<String> = years.iter().map(|(y, n)| format!("{n} in {y}")).collect();
    Text::new(ja.join("、"), en.join(", "))
}

fn read_rows(bytes: &[u8], format: &Format) -> Result<Vec<Row>, Text> {
    match format {
        Format::Csv { shift_jis } => holidays::read_csv(bytes, *shift_jis),
        Format::GovUk { division } => holidays::read_govuk(bytes, division),
    }
    .map_err(|e| e.why)
}

/// What changed between two readings of a table: the days added, removed and renamed, and
/// the `covers` to write.
fn table_diff(name: &str, old: &[Row], new: &[Row], covers: &Covers) -> (Vec<Text>, bool) {
    let mut lines = Vec::new();
    let om: BTreeMap<Day, &str> = old.iter().map(|r| (r.day, r.name.as_str())).collect();
    let nm: BTreeMap<Day, &str> = new.iter().map(|r| (r.day, r.name.as_str())).collect();
    let added: Vec<(Day, String)> = nm.iter().filter(|(d, _)| !om.contains_key(d)).map(|(d, n)| (*d, n.to_string())).collect();
    let removed: Vec<(Day, String)> = om.iter().filter(|(d, _)| !nm.contains_key(d)).map(|(d, n)| (*d, n.to_string())).collect();
    let renamed: Vec<(Day, String)> = nm.iter().filter_map(|(d, n)| om.get(d).filter(|o| *o != n).map(|o| (*d, format!("{o} → {n}")))).collect();
    if !added.is_empty() {
        let y = by_year(&added);
        let s = some_days(&added);
        lines.push(tr!("  {}が増えます: {}", "  {} days are added ({}): {}", y.ja, s.ja; added.len(), y.en, s.en));
    }
    if !removed.is_empty() {
        let s = some_days(&removed);
        lines.push(tr!("  {} 日が消えます: {}", "  {} days are removed: {}", removed.len(), s.ja; removed.len(), s.en));
    }
    if !renamed.is_empty() {
        let s = some_days(&renamed);
        lines.push(tr!("  {} 日の名前が変わります: {}", "  {} days are renamed: {}", renamed.len(), s.ja; renamed.len(), s.en));
    }
    let moved = !(added.is_empty() && removed.is_empty() && renamed.is_empty());
    if let (Some(first), Some(last)) = (new.first(), new.last()) {
        let (y0, y1) = (first.day.year(), last.day.year());
        match covers {
            Covers::Range(a, b) => {
                let a2 = (*a).min(Day::from_ymd(y0 as i64, 1, 1).unwrap());
                let b2 = (*b).max(Day::from_ymd(y1 as i64, 12, 31).unwrap());
                if (a2, b2) != (*a, *b) {
                    lines.push(tr!(
                        "  表が延びるので、covers も延ばしてください: `covers {a2}..{b2}`（いまは {a}..{b}）",
                        "  the table grew, so does covers: `covers {a2}..{b2}` (now {a}..{b})"
                    ));
                }
            }
            Covers::ListedYears => {
                let (o0, o1) = (old.first().map(|r| r.day.year()), old.last().map(|r| r.day.year()));
                if (o0, o1) != (Some(y0), Some(y1)) {
                    let was = match (o0, o1) {
                        (Some(a), Some(b)) => format!("{a}..{b}"),
                        _ => "-".into(),
                    };
                    lines.push(tr!(
                        "  covers listed years の範囲は {was} 年から {y0}..{y1} 年に変わります",
                        "  covers listed years moves from {was} to {y0}..{y1}"
                    ));
                }
            }
        }
    }
    let _ = name;
    (lines, moved)
}

/// The lines that differ between two texts, as `- old` and `+ new`, up to `cap` in all (the
/// lines are ritsu-base's [`sources::diff_lines`]; koyomi keeps its own cap, across both sides).
fn text_diff(old: &str, new: &str, cap: usize) -> Vec<String> {
    let (gone, came) = sources::diff_lines(old, new);
    let mut out: Vec<String> = gone.iter().map(|l| format!("- {l}")).chain(came.iter().map(|l| format!("+ {l}"))).collect();
    out.truncate(cap);
    out
}

/// `koyomi source outdated`.
pub fn outdated(f: &File, dir: &Path) -> Result<Outcome, Text> {
    let mut lines = Vec::new();
    let mut changed = false;
    for s in &f.sources {
        let name = &s.name;
        match &s.kind {
            SourceKind::File { path, url, pin, format, covers } => {
                let Some(url) = url else {
                    lines.push(tr!("{name}: url が無いので、元が変わったかを問えません", "{name}: no url, so there is nothing to ask whether it moved on"));
                    continue;
                };
                let body = sources::curl(url)?;
                let h = short(&body);
                if pin.as_deref() == Some(h.as_str()) {
                    lines.push(tr!("{name}: 変わっていません（{url} は sha256:{h} で、固定と同じ）", "{name}: unchanged ({url} is sha256:{h}, as pinned)"));
                    continue;
                }
                changed = true;
                let was = pin.as_deref().map(|p| format!("sha256:{p}")).unwrap_or_else(|| "-".into());
                lines.push(tr!("{name}: {url} が変わりました（固定は {was}、いまは sha256:{h}）", "{name}: {url} moved on (pinned {was}, now sha256:{h})"));
                let (Some((fmt, _)), Some((cov, _))) = (format, covers) else { continue };
                let new = match read_rows(&body, fmt) {
                    Ok(r) => r,
                    Err(why) => {
                        lines.push(tr!("  取ってきたファイルが表として読めません: {}", "  what came is not a table koyomi reads: {}", why.ja; why.en));
                        continue;
                    }
                };
                let old = match std::fs::read(dir.join(path)) {
                    Ok(b) => read_rows(&b, fmt).unwrap_or_default(),
                    Err(_) => {
                        lines.push(tr!("  写し {path} が無いので、行を比べられません", "  there is no copy {path}, so the rows cannot be compared"));
                        continue;
                    }
                };
                let (more, moved) = table_diff(name, &old, &new, cov);
                if !moved {
                    lines.push(tr!("  行は同じで、バイト列だけが変わりました", "  the rows are the same; only the bytes changed"));
                }
                lines.extend(more);
                lines.push(tr!(
                    "  読んでから koyomi source fetch で取り直し、koyomi source pin で固定してください",
                    "  read it, then koyomi source fetch takes it and koyomi source pin pins it"
                ));
            }
            SourceKind::Law { id, asof, pins } => {
                let later = egov().later_revisions(id, &asof.to_string())?;
                if later.is_empty() {
                    lines.push(tr!("{name}: {asof} より後に施行される版はありません", "{name}: no revision comes into force after {asof}"));
                    continue;
                }
                let cdir = copy_dir(dir, id, *asof);
                // Each date is held to the text of the date before it, starting from the copy,
                // so an amendment is said once, on the day it comes into force.
                let mut differs: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
                for fr in fragments(f, name, pins) {
                    let Some(e) = elm(&fr) else { continue };
                    let Ok(mut prev) = std::fs::read(cdir.join(format!("{e}.xml"))) else {
                        lines.push(tr!("{name}: {fr} の写しが無いので比べられません。先に koyomi source fetch を走らせてください", "{name}: there is no copy of {fr} to compare; run koyomi source fetch first"));
                        continue;
                    };
                    for (date, _) in &later {
                        let (xml, _) = egov().law_data(id, date, Some(&e))?;
                        if !sources::same_text(&prev, &xml) {
                            let diff = text_diff(&sources::xml_text(&String::from_utf8_lossy(&prev)), &sources::xml_text(&String::from_utf8_lossy(&xml)), 8);
                            differs.entry(date.clone()).or_default().push((fr.clone(), diff));
                        }
                        prev = xml;
                    }
                }
                for (date, rid) in &later {
                    match differs.get(date) {
                        Some(frs) => {
                            changed = true;
                            let names: Vec<&str> = frs.iter().map(|(f, _)| f.as_str()).collect();
                            lines.push(tr!(
                                "{name}: {date} 施行の版（{rid}）で {} が変わります。その日から効く計算を読み直してください",
                                "{name}: the revision in force from {date} ({rid}) changes {}; reread what the lines that cite it compute from that day",
                                names.join("、");
                                names.join(", ")
                            ));
                            for (fr, diff) in frs {
                                for l in diff {
                                    lines.push(Text::same(format!("  {fr}: {l}")));
                                }
                            }
                        }
                        None => lines.push(tr!("{name}: {date} 施行の版（{rid}）では、引いている条は変わりません", "{name}: the revision in force from {date} ({rid}) leaves the cited articles as they are")),
                    }
                }
            }
        }
    }
    Ok(Outcome { lines, changed })
}

/// The directory a `.cal`'s copies are found from.
pub fn dir_of(path: &str) -> PathBuf {
    Path::new(path).parent().unwrap_or(Path::new("")).to_path_buf()
}
