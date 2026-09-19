//! Sources (DESIGN-draft §3, §15.68): the documents a rule transcribes, the fragments it
//! cites, and the digests that hold the two together.
//!
//! A `source … = law` names a law on e-Gov as of a date; each fragment the rule cites
//! (`@法 第91条`) has a copy beside the rule, `sources/law/<law id>@<asof>/<element>.xml`,
//! and a pinned digest under the `source` line. A `source … = file` is one file, pinned whole.
//! `check` reads the copies and compares; it never reads the network. Fetching, pinning and
//! asking e-Gov for later revisions are the `rulec source` commands, below.
//!
//! Reading happens here and nowhere deeper, the layering `enums.rs` set: the checker stays a
//! function from text to diagnostics, and the playground says a copy cannot be read.

use crate::ast::{Cite, Item, RuleFile, SourceDecl, SourceKind};
use crate::diag::{Diag, Span};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A fragment of a law, as the rule names it and as e-Gov addresses it.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment {
    /// As written: `第91条`, `第20条の2第3項`, `別表第一`.
    pub name: String,
    /// The e-Gov `elm` parameter: `MainProvision-Article_91`, `AppdxTable[1]`.
    pub elm: String,
}

impl Fragment {
    /// The file the copy is kept in: the element path with the brackets made plain.
    pub fn file(&self) -> String {
        format!("{}.xml", self.elm.replace('[', "_").replace(']', ""))
    }
}

/// A number as a law writes it: `一` … `九十九`, or ASCII digits.
fn number(s: &str) -> Option<u32> {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        return s.parse().ok();
    }
    let digit = |c: char| "一二三四五六七八九".find(c).map(|i| (i / '一'.len_utf8()) as u32 + 1);
    let mut n = 0u32;
    let mut seen = false;
    let mut chars = s.chars().peekable();
    let mut tens_done = false;
    while let Some(c) = chars.next() {
        if c == '十' {
            if tens_done {
                return None;
            }
            n = if seen { n * 10 } else { 10 };
            tens_done = true;
            seen = false;
            continue;
        }
        let d = digit(c)?;
        if seen {
            return None;
        }
        n += d;
        seen = true;
    }
    if n == 0 { None } else { Some(n) }
}

/// The fragment a citation names. Articles, paragraphs and items of the main provisions,
/// and appendix tables by their ordinal; supplementary provisions and sub-items wait for a
/// rule that needs them (DESIGN-draft §3.2).
pub fn fragment(name: &str) -> Option<Fragment> {
    if let Some(rest) = name.strip_prefix("別表第") {
        let n = number(rest)?;
        return Some(Fragment { name: name.to_string(), elm: format!("AppdxTable[{n}]") });
    }
    let rest = name.strip_prefix('第')?;
    let (art, rest) = rest.split_once('条')?;
    let mut elm = format!("MainProvision-Article_{}", number(art)?);
    let mut rest = rest;
    // `第20条の2`: a sub-numbered article.
    if let Some(r) = rest.strip_prefix('の') {
        let end = r.find('第').unwrap_or(r.len());
        elm.push_str(&format!("_{}", number(&r[..end])?));
        rest = &r[end..];
    }
    if let Some(r) = rest.strip_prefix('第') {
        let (para, r) = r.split_once('項')?;
        elm.push_str(&format!("-Paragraph_{}", number(para)?));
        rest = r;
        if let Some(r) = rest.strip_prefix('第') {
            let (item, r) = r.split_once('号')?;
            elm.push_str(&format!("-Item_{}", number(item)?));
            rest = r;
        }
    }
    if !rest.is_empty() {
        return None;
    }
    Some(Fragment { name: name.to_string(), elm })
}

/// The directory the copies of a law as of a date are kept in, beside the rule.
pub fn copy_dir(rule_path: &str, id: &str, asof: &str) -> PathBuf {
    Path::new(rule_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join("sources")
        .join("law")
        .join(format!("{id}@{asof}"))
}

/// Who cites what: for every citation, the source, the fragment, and the definition it is
/// on, named the way a diagnostic names it. Tables and clauses cite for all their rows; a row
/// with a citation of its own adds it.
pub fn citations(f: &RuleFile) -> Vec<(String, String, String, Span)> {
    let mut out = Vec::new();
    let mut push = |c: &Option<Cite>, who: String| {
        if let Some(c) = c {
            for frag in &c.fragments {
                out.push((c.source.clone(), frag.clone(), who.clone(), c.span.clone()));
            }
        }
    };
    for it in &f.items {
        match it {
            Item::Table(t) => {
                let n = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                let kind = if t.clause { tr!("節", "clause") } else { tr!("表", "table") };
                push(&t.cite, format!("{kind} {n}"));
                for r in &t.rows {
                    let rn = match &r.label {
                        Some(l) => tr!("行{}（{}）", "row {} ({})", r.index, l.text),
                        None => tr!("行{}", "row {}", r.index),
                    };
                    push(&r.cite, format!("{kind} {n} {rn}"));
                }
            }
            Item::Derived(d) => push(&d.cite, tr!("導出 {}", "derived value {}", d.name.text)),
            Item::Define(d) => push(&d.cite, tr!("定義 {}", "definition {}", d.name.text)),
            Item::Count(_) => {}
        }
    }
    out
}

/// The fragments cited from one source, in order of first citation, each with who cites it.
fn cited_from<'a>(cites: &'a [(String, String, String, Span)], source: &str) -> Vec<(String, Vec<&'a str>)> {
    let mut out: Vec<(String, Vec<&str>)> = Vec::new();
    for (s, frag, who, _) in cites {
        if s != source {
            continue;
        }
        match out.iter_mut().find(|(f, _)| f == frag) {
            Some((_, whos)) => whos.push(who),
            None => out.push((frag.clone(), vec![who])),
        }
    }
    out
}

/// The pin line of a fragment, as `rulec source pin` writes it and as `fix.text` offers it.
/// The line with its `sha256:` token set to `hash` — replaced where there is one, added
/// before the comment where there is none.
fn set_hash(line: &str, hash: &str) -> String {
    let (code, comment) = match line.find('#') {
        Some(h) => (&line[..h], Some(&line[h..])),
        None => (line, None),
    };
    let code = code.trim_end();
    let code = match code.find("sha256:") {
        Some(p) => {
            let rest = &code[p + "sha256:".len()..];
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            format!("{}sha256:{hash}{}", &code[..p], &rest[end..])
        }
        None => format!("{code} sha256:{hash}"),
    };
    match comment {
        Some(c) => format!("{code}  {c}"),
        None => code,
    }
}

pub fn pin_line(fragment: &str, hash: &str) -> String {
    format!("  {fragment} sha256:{hash}")
}

/// The `source` line of a file source with its digest.
pub fn file_line(d: &SourceDecl, hash: &str) -> String {
    let SourceKind::File { path, .. } = &d.kind else { return String::new() };
    let name = match &d.name.ascii {
        Some(a) => format!("{}({a})", d.name.text),
        None => d.name.text.clone(),
    };
    format!("{} {name} = {} \"{path}\" sha256:{hash}", crate::kw::SOURCE, crate::kw::FILE)
}

/// Holds every source to its copies: a cited fragment is pinned (E037), the pin is the copy's
/// digest (E038), the copy is there (E039), and a pin is still cited (W119). A citation of a
/// source the rule does not declare is E012.
pub fn check(f: &RuleFile, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let cites = citations(f);
    let at = |line: usize, name: &str| tr!("{rule_path}:{line} 出典 {name}", "{rule_path}:{line} source {name}");
    for (s, _, who, span) in &cites {
        if !f.sources.iter().any(|d| d.name.text == *s) {
            out.push(
                Diag::error("E012", tr!("出典 `{s}` は宣言されていません", "The source `{s}` is not declared"))
                    .at(tr!("{rule_path}:{} {who}", "{rule_path}:{} {who}", span.line))
                    .mark(span.clone(), "")
                    .note(tr!(
                        "`{} {s} = {} \"<法令ID>\" {} <日付>` か `{} {s} = {} \"<ファイル>\"` を `import` の次に書いてください。",
                        "Declare it after `import`: `{} {s} = {} \"<law id>\" {} <date>` or `{} {s} = {} \"<file>\"`.",
                        crate::kw::SOURCE,
                        crate::kw::LAW,
                        crate::kw::ASOF,
                        crate::kw::SOURCE,
                        crate::kw::FILE
                    )),
            );
        }
    }
    let dir = Path::new(rule_path).parent().unwrap_or(Path::new(".")).to_path_buf();
    for d in &f.sources {
        // A source that came in with an applied rule was checked with that rule (§15.69).
        if d.base.is_some() {
            continue;
        }
        let name = &d.name.text;
        let whos_text = |whos: &[&str]| whos.join(if crate::i18n::ja() { "、" } else { ", " });
        match &d.kind {
            SourceKind::File { path, hash } => {
                let p = dir.join(path);
                let Ok(bytes) = std::fs::read(&p) else {
                    out.push(
                        Diag::error("E039", tr!("出典 `{name}` の写し `{path}` を読めません", "The copy `{path}` of source `{name}` cannot be read"))
                            .at(at(d.span.line, name))
                            .mark(d.span.clone(), "")
                            .note(tr!("パスは規則ファイルのある場所からたどります（探した先: {}）。", "The path is followed from the directory of the rule file (looked for: {}).", p.display())),
                    );
                    continue;
                };
                let h = crate::sha256::short(&bytes);
                let whos: Vec<&str> = cites.iter().filter(|(s, ..)| s == name).map(|(_, _, w, _)| w.as_str()).collect();
                match hash {
                    None => out.push(
                        Diag::error("E037", tr!("出典 `{name}` のハッシュが固定されていません", "The digest of source `{name}` is not pinned"))
                            .at(at(d.span.line, name))
                            .mark(d.span.clone(), "")
                            .note(tr!("いまの写しのハッシュは sha256:{h} です。この内容で承認するなら、次のとおり書き換えてください。", "The copy's digest is sha256:{h}. To pin it as the one approved, rewrite the line as follows."))
                            .fix(crate::diag::FixKind::PinSource, file_line(d, &h)),
                    ),
                    Some(p) if *p != h => out.push(
                        Diag::error("E038", tr!("出典 `{name}` の写しが変わっています", "The copy of source `{name}` has changed"))
                            .at(at(d.span.line, name))
                            .mark(d.span.clone(), tr!("固定: sha256:{p}", "pinned: sha256:{p}"))
                            .note(tr!("いまの写し: sha256:{h}", "The copy now: sha256:{h}"))
                            .note(if whos.is_empty() {
                                tr!("この出典を引いている定義はありません。", "No definition cites this source.")
                            } else {
                                tr!("読み直す定義: {}", "Definitions to reread: {}", whos_text(&whos))
                            })
                            .note(tr!("原本を読み直し、写した行がまだ正しければ、次の行に書き換えて固定し直してください。", "Reread the document; if what was transcribed still holds, rewrite the line as follows to pin the new copy."))
                            .fix(crate::diag::FixKind::PinSource, file_line(d, &h)),
                    ),
                    _ => {}
                }
            }
            SourceKind::Law { id, asof } => {
                let cdir = copy_dir(rule_path, id, asof);
                let cited = cited_from(&cites, name);
                for (frag, whos) in &cited {
                    let Some(fr) = fragment(frag) else {
                        out.push(
                            Diag::error("E037", tr!("引用箇所 `{frag}` の書き方が読めません", "The fragment `{frag}` cannot be read"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("書けるのは `第20条`、`第20条の2`、`第20条第2項`、`第20条第2項第3号`、`別表第一` の形です。附則と号の細分はまだ受けません。", "The forms are `第20条`, `第20条の2`, `第20条第2項`, `第20条第2項第3号` and `別表第一`. Supplementary provisions and sub-items are not read yet."))
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos))),
                        );
                        continue;
                    };
                    let p = cdir.join(fr.file());
                    let Ok(bytes) = std::fs::read(&p) else {
                        out.push(
                            Diag::error("E039", tr!("出典 `{name}` の `{frag}` の写しがありません", "There is no copy of fragment `{frag}` of source `{name}`"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("探した先: {}", "Looked for: {}", p.display()))
                                .note(tr!("`rulec source fetch {rule_path}` が e-Gov から取ってきて写しに置きます。check は通信しません。", "`rulec source fetch {rule_path}` fetches it from e-Gov into the copies. check never reads the network."))
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos))),
                        );
                        continue;
                    };
                    let h = crate::sha256::short(&bytes);
                    match d.pins.iter().find(|p| p.fragment == *frag) {
                        None => out.push(
                            Diag::error("E037", tr!("出典 `{name}` の `{frag}` のハッシュが固定されていません", "Fragment `{frag}` of source `{name}` is not pinned"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos)))
                                .note(tr!("写しのハッシュは sha256:{h} です。この内容で承認するなら、`{}` の行の下に次の行を足してください（`rulec source pin` でも書けます）。", "The copy's digest is sha256:{h}. To pin it as the one approved, add the following line under the `{}` line (`rulec source pin` writes it too).", crate::kw::SOURCE))
                                .fix(crate::diag::FixKind::PinSource, pin_line(frag, &h)),
                        ),
                        Some(pin) if pin.hash != h => out.push(
                            Diag::error("E038", tr!("出典 `{name}` の `{frag}` が変わっています", "Fragment `{frag}` of source `{name}` has changed"))
                                .at(at(pin.span.line, name))
                                .mark(pin.span.clone(), tr!("固定: sha256:{}", "pinned: sha256:{}", pin.hash))
                                .note(tr!("いまの写し: sha256:{h}", "The copy now: sha256:{h}"))
                                .note(tr!("読み直す定義: {}", "Definitions to reread: {}", whos_text(whos)))
                                .note(tr!("写しの差分（{}）を読み、写した行がまだ正しければ、この行を次のとおり書き換えて固定し直してください。", "Read the copy's diff ({}), and if what was transcribed still holds, rewrite this line as follows to pin the new copy.", p.display()))
                                .fix(crate::diag::FixKind::PinSource, pin_line(frag, &h)),
                        ),
                        _ => {}
                    }
                }
                for pin in &d.pins {
                    if !cited.iter().any(|(frag, _)| *frag == pin.fragment) {
                        out.push(
                            Diag::warning("W119", tr!("出典 `{name}` の `{}` はハッシュが固定されていますが、引用されていません", "Fragment `{}` of source `{name}` is pinned but not cited", pin.fragment))
                                .at(at(pin.span.line, name))
                                .mark(pin.span.clone(), "")
                                .note(tr!("引用を消したあとの残りです。`rulec source pin` が消します。", "It is what remains after a citation was removed. `rulec source pin` removes it.")),
                        );
                    }
                }
            }
        }
    }
    out
}

/// The text of a fragment's copy, for the approver's page: the XML with its tags removed,
/// one line per paragraph, item or table row.
pub fn fragment_text(rule_path: &str, d: &SourceDecl, frag: &str) -> Option<String> {
    let SourceKind::Law { id, asof } = &d.kind else { return None };
    let fr = fragment(frag)?;
    // A source inherited through an `apply` keeps its copies beside the rule it came from.
    let base = d.base.as_deref().unwrap_or(rule_path);
    let xml = std::fs::read_to_string(copy_dir(base, id, asof).join(fr.file())).ok()?;
    Some(xml_text(&xml))
}

/// Strip the tags of a law XML fragment into readable lines.
pub fn xml_text(xml: &str) -> String {
    let breaks = ["Paragraph", "Item", "Subitem1", "ArticleCaption", "ArticleTitle", "AppdxTableTitle", "RelatedArticleNum", "TableRow", "Sentence"];
    let mut out = String::new();
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else { break };
        let tag = &rest[lt + 1..lt + gt];
        if let Some(name) = tag.strip_prefix('/') {
            let name = name.trim();
            if breaks.contains(&name) {
                out.push('\n');
            } else if name == "TableColumn" || name == "Column" {
                out.push(' ');
            }
        }
        rest = &rest[lt + gt + 1..];
    }
    out.push_str(rest);
    let mut lines: Vec<String> = Vec::new();
    for l in out.lines() {
        let t: String = l.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() && lines.last().map(|x: &String| x != &t).unwrap_or(true) {
            lines.push(t);
        }
    }
    lines.join("\n")
}

// ── The commands: fetch, pin, outdated ────────────────────────────────────────

/// One line of what a command did or found.
pub struct Outcome {
    pub lines: Vec<String>,
    /// Something changed or is out of date: the exit code says so.
    pub changed: bool,
}

fn curl(url: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "120", url])
        .output()
        .map_err(|e| tr!("curl を起動できません: {e}", "cannot run curl: {e}"))?;
    if !out.status.success() {
        return Err(tr!("{url} を取れません: {}", "cannot fetch {url}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        })
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' || c == b' ' {
            continue;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

fn law_url(id: &str, asof: &str, elm: &str) -> String {
    let elm = elm.replace('[', "%5B").replace(']', "%5D");
    format!("https://laws.e-gov.go.jp/api/2/law_data/{id}?asof={asof}&elm={elm}&law_full_text_format=xml")
}

/// Fetch one fragment: the XML and the revision id it came from.
fn fetch_fragment(id: &str, asof: &str, fr: &Fragment) -> Result<(Vec<u8>, String), String> {
    let body = curl(&law_url(id, asof, &fr.elm))?;
    let text = String::from_utf8_lossy(&body);
    let j = crate::json::parse(&text).map_err(|e| tr!("e-Gov の答えが JSON として読めません: {e}", "the e-Gov reply is not JSON: {e}"))?;
    let b64 = j.get("law_full_text").and_then(|x| x.as_str()).ok_or_else(|| tr!("e-Gov の答えに law_full_text がありません", "the e-Gov reply has no law_full_text"))?;
    let xml = base64_decode(b64).ok_or_else(|| tr!("law_full_text を base64 として読めません", "law_full_text is not base64"))?;
    let rev = j.get("revision_info").and_then(|r| r.get("law_revision_id")).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Ok((xml, rev))
}

/// `rulec source fetch`: put a copy of every cited fragment beside the rule, saying which
/// ones changed against the copies already there. The one command that reads the network.
pub fn fetch(f: &RuleFile, rule_path: &str) -> Result<Outcome, String> {
    let cites = citations(f);
    let mut lines = Vec::new();
    let mut changed = false;
    for d in &f.sources {
        let SourceKind::Law { id, asof } = &d.kind else { continue };
        let cdir = copy_dir(rule_path, id, asof);
        for (frag, _) in cited_from(&cites, &d.name.text) {
            let Some(fr) = fragment(&frag) else {
                lines.push(tr!("{}: 引用箇所 `{frag}` の書き方が読めません", "{}: the fragment `{frag}` cannot be read", d.name.text));
                continue;
            };
            let (xml, rev) = fetch_fragment(id, asof, &fr)?;
            std::fs::create_dir_all(&cdir).map_err(|e| format!("{}: {e}", cdir.display()))?;
            let p = cdir.join(fr.file());
            let before = std::fs::read(&p).ok();
            let same = before.as_deref() == Some(xml.as_slice());
            std::fs::write(&p, &xml).map_err(|e| format!("{}: {e}", p.display()))?;
            if !rev.is_empty() {
                let _ = std::fs::write(cdir.join("revision.txt"), format!("{rev}\n"));
            }
            let h = crate::sha256::short(&xml);
            lines.push(match before {
                None => tr!("{}: {frag} を取りました（sha256:{h}）", "{}: fetched {frag} (sha256:{h})", d.name.text),
                Some(_) if same => tr!("{}: {frag} は変わっていません（sha256:{h}）", "{}: {frag} is unchanged (sha256:{h})", d.name.text),
                Some(_) => {
                    changed = true;
                    tr!("{}: {frag} が変わりました（いま sha256:{h}）。引いている行を読み直してください", "{}: {frag} changed (now sha256:{h}); reread the rows that cite it", d.name.text)
                }
            });
        }
    }
    Ok(Outcome { lines, changed })
}

/// `rulec source pin`: rewrite the pins from the copies — the fragments cited, in order, each
/// with its copy's digest — and the digest of every file source. Only those lines move.
pub fn pin(f: &RuleFile, rule_path: &str, src: &str) -> Result<(String, Outcome), String> {
    let cites = citations(f);
    let lines: Vec<&str> = src.lines().collect();
    // (line index, how many lines to replace, replacement lines)
    let mut edits: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut report = Vec::new();
    let dir = Path::new(rule_path).parent().unwrap_or(Path::new("."));
    // The rules this one applies (§15.69): the heading's digest is rewritten in place, so a
    // comment on the line stays.
    for a in &f.applies {
        let bytes = std::fs::read(dir.join(&a.path)).map_err(|e| format!("{}: {e}", a.path))?;
        let h = crate::sha256::short(&bytes);
        if a.hash.as_deref() != Some(h.as_str()) {
            let line = lines.get(a.span.line - 1).copied().unwrap_or("");
            edits.push((a.span.line - 1, 1, vec![set_hash(line, &h)]));
            report.push(tr!("{}: 元の規則 `{}` のハッシュを sha256:{h} に固定しました", "{}: pinned the callee `{}` at sha256:{h}", a.name.text, a.path));
        }
    }
    for d in &f.sources {
        if d.base.is_some() {
            continue;
        }
        match &d.kind {
            SourceKind::File { path, hash } => {
                let bytes = std::fs::read(dir.join(path)).map_err(|e| format!("{path}: {e}"))?;
                let h = crate::sha256::short(&bytes);
                if hash.as_deref() != Some(h.as_str()) {
                    let line = lines.get(d.span.line - 1).copied().unwrap_or("");
                    edits.push((d.span.line - 1, 1, vec![set_hash(line, &h)]));
                    report.push(tr!("{}: sha256:{h} を固定しました", "{}: pinned sha256:{h}", d.name.text));
                }
            }
            SourceKind::Law { id, asof } => {
                let cdir = copy_dir(rule_path, id, asof);
                let mut new_pins: Vec<String> = Vec::new();
                for (frag, _) in cited_from(&cites, &d.name.text) {
                    let Some(fr) = fragment(&frag) else { continue };
                    match std::fs::read(cdir.join(fr.file())) {
                        Ok(bytes) => new_pins.push(pin_line(&frag, &crate::sha256::short(&bytes))),
                        Err(_) => {
                            // No copy yet: keep what was pinned, if anything, and say so.
                            if let Some(p) = d.pins.iter().find(|p| p.fragment == frag) {
                                new_pins.push(pin_line(&frag, &p.hash));
                            }
                            report.push(tr!("{}: {frag} の写しがありません。先に `rulec source fetch` を走らせてください", "{}: no copy of {frag}; run `rulec source fetch` first", d.name.text));
                        }
                    }
                }
                let old: Vec<String> = d.pins.iter().map(|p| pin_line(&p.fragment, &p.hash)).collect();
                if old != new_pins {
                    edits.push((d.span.line, d.pins.len(), new_pins.clone()));
                    report.push(tr!("{}: {} 箇所のハッシュを固定しました", "{}: pinned {} fragments", d.name.text, new_pins.len()));
                }
            }
        }
    }
    let changed = !edits.is_empty();
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    for (at, n, repl) in edits {
        out.splice(at..at + n, repl);
    }
    let mut text = out.join("\n");
    text.push('\n');
    Ok((text, Outcome { lines: report, changed }))
}

/// `rulec source outdated`: ask e-Gov for the revisions of every law source enforced after
/// its `asof`, and say which cited fragments differ there from the copies. The other command
/// that reads the network; meant for a scheduled job, since `check` cannot know of an
/// amendment on its own.
pub fn outdated(f: &RuleFile, rule_path: &str) -> Result<Outcome, String> {
    let cites = citations(f);
    let mut lines = Vec::new();
    let mut changed = false;
    for d in &f.sources {
        let SourceKind::Law { id, asof } = &d.kind else { continue };
        let body = curl(&format!("https://laws.e-gov.go.jp/api/2/law_revisions/{id}"))?;
        let text = String::from_utf8_lossy(&body);
        let j = crate::json::parse(&text).map_err(|e| tr!("e-Gov の答えが JSON として読めません: {e}", "the e-Gov reply is not JSON: {e}"))?;
        let mut later: Vec<(String, String)> = Vec::new();
        if let Some(crate::json::Json::Arr(revs)) = j.get("revisions") {
            for r in revs {
                let date = r.get("amendment_enforcement_date").and_then(|x| x.as_str()).unwrap_or("");
                let rid = r.get("law_revision_id").and_then(|x| x.as_str()).unwrap_or("");
                if !date.is_empty() && date > asof.as_str() {
                    later.push((date.to_string(), rid.to_string()));
                }
            }
        }
        later.sort();
        // Several revisions can be enforced on one day; the text as of that day is one.
        later.dedup_by(|a, b| a.0 == b.0);
        if later.is_empty() {
            lines.push(tr!("{}: {asof} より後に施行された改正はありません", "{}: no amendment enforced after {asof}", d.name.text));
            continue;
        }
        let cdir = copy_dir(rule_path, id, asof);
        let mut differs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (date, _) in &later {
            for (frag, _) in cited_from(&cites, &d.name.text) {
                let Some(fr) = fragment(&frag) else { continue };
                let (xml, _) = fetch_fragment(id, date, &fr)?;
                let now = std::fs::read(cdir.join(fr.file())).ok();
                if now.as_deref() != Some(xml.as_slice()) {
                    differs.entry(date.clone()).or_default().push(frag.clone());
                }
            }
        }
        for (date, _) in &later {
            match differs.get(date) {
                Some(frags) => {
                    changed = true;
                    lines.push(tr!(
                        "{}: {date} 施行の改正で {} が変わります。その日から効く版には、読み直した規則が要ります",
                        "{}: the amendment enforced on {date} changes {}; the version in force from that day needs a reread rule",
                        d.name.text,
                        frags.join(if crate::i18n::ja() { "、" } else { ", " })
                    ));
                }
                None => lines.push(tr!("{}: {date} 施行の改正では、引用している箇所は変わりません", "{}: the amendment enforced on {date} leaves the cited fragments unchanged", d.name.text)),
            }
        }
    }
    Ok(Outcome { lines, changed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_are_named_as_the_law_names_them() {
        assert_eq!(fragment("第91条").unwrap().elm, "MainProvision-Article_91");
        assert_eq!(fragment("第20条の2").unwrap().elm, "MainProvision-Article_20_2");
        assert_eq!(fragment("第20条第2項").unwrap().elm, "MainProvision-Article_20-Paragraph_2");
        assert_eq!(fragment("第20条第2項第3号").unwrap().elm, "MainProvision-Article_20-Paragraph_2-Item_3");
        assert_eq!(fragment("第二十条第二項").unwrap().elm, "MainProvision-Article_20-Paragraph_2");
        assert_eq!(fragment("別表第一").unwrap().elm, "AppdxTable[1]");
        assert_eq!(fragment("別表第一").unwrap().file(), "AppdxTable_1.xml");
        assert_eq!(fragment("別表第十二").unwrap().elm, "AppdxTable[12]");
        assert!(fragment("附則第3条").is_none());
        assert!(fragment("第3条ただし書").is_none());
    }

    #[test]
    fn numbers() {
        assert_eq!(number("九"), Some(9));
        assert_eq!(number("十"), Some(10));
        assert_eq!(number("十二"), Some(12));
        assert_eq!(number("二十"), Some(20));
        assert_eq!(number("九十九"), Some(99));
        assert_eq!(number("12"), Some(12));
        assert_eq!(number(""), None);
    }

    #[test]
    fn base64_roundtrip() {
        assert_eq!(base64_decode("YWJj").unwrap(), b"abc");
        assert_eq!(base64_decode("YWI=").unwrap(), b"ab");
        assert_eq!(base64_decode("YQ==").unwrap(), b"a");
    }

    #[test]
    fn xml_becomes_lines() {
        let x = "<Article Num=\"1\"><ArticleTitle>第一条</ArticleTitle><Paragraph Num=\"1\"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>";
        assert_eq!(xml_text(x), "第一条\n甲は、乙とする。");
    }
}
