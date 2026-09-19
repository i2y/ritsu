//! Sources (§15.68): the documents a rule transcribes, the fragments it
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
    /// As written: `第91条`, `第20条の2第3項`, `別表第一`, `附則（令和七年三月三一日法律第一三号）第3条`.
    pub name: String,
    /// The e-Gov `elm` parameter: `MainProvision-Article_91`, `AppdxTable[1]`,
    /// `SupplProvision-Article_3`. For an amending law's supplementary provisions it reads
    /// `SupplProvision[?]-Article_3` until the position is looked up (§15.71): e-Gov counts
    /// them in document order and does not take the amending law's number.
    pub elm: String,
    /// The amending law whose supplementary provisions these are, spelled as e-Gov's
    /// `AmendLawNum` spells it. Only the commands that read the network need its position.
    pub amend: Option<String>,
}

impl Fragment {
    /// The file the copy is kept in: the element path with the brackets made plain. The
    /// supplementary provisions of an amending law are filed under that law's number, which
    /// the rule wrote, rather than under a position only the network knows.
    pub fn file(&self) -> String {
        match &self.amend {
            Some(n) => format!("{}.xml", self.elm.replace("SupplProvision[?]", &format!("SupplProvision_{n}"))),
            None => format!("{}.xml", self.elm.replace('[', "_").replace(']', "")),
        }
    }

    /// The element path with the position of the supplementary provisions filled in.
    fn elm_at(&self, k: usize) -> String {
        self.elm.replace("[?]", &format!("[{k}]"))
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

/// The fragment a citation names. Articles, paragraphs and items of the main provisions;
/// appendix tables by their ordinal; and the supplementary provisions (§15.71): the law's
/// own as `附則第3条`, an amending law's as `附則（令和七年三月三一日法律第一三号）第3条` with
/// the number spelled as the law's own heading spells it, either also whole without the
/// article. Sub-items still wait for a rule that needs them.
pub fn fragment(name: &str) -> Option<Fragment> {
    if let Some(rest) = name.strip_prefix("別表第") {
        let n = number(rest)?;
        return Some(Fragment { name: name.to_string(), elm: format!("AppdxTable[{n}]"), amend: None });
    }
    if let Some(rest) = name.strip_prefix("附則") {
        let (amend, rest) = match rest.strip_prefix('（').or_else(|| rest.strip_prefix('(')) {
            Some(r) => {
                let end = r.find(|c: char| c == '）' || c == ')')?;
                if end == 0 {
                    return None;
                }
                let close = r[end..].chars().next()?.len_utf8();
                (Some(r[..end].to_string()), &r[end + close..])
            }
            None => (None, rest),
        };
        let suffix = if rest.is_empty() { String::new() } else { article_suffix(rest)? };
        let head = if amend.is_some() { "SupplProvision[?]" } else { "SupplProvision" };
        return Some(Fragment { name: name.to_string(), elm: format!("{head}{suffix}"), amend });
    }
    let suffix = article_suffix(name)?;
    Some(Fragment { name: name.to_string(), elm: format!("MainProvision{suffix}"), amend: None })
}

/// `第20条の2第3項第4号` as the tail of an element path: `-Article_20_2-Paragraph_3-Item_4`.
fn article_suffix(s: &str) -> Option<String> {
    let rest = s.strip_prefix('第')?;
    let (art, rest) = rest.split_once('条')?;
    let mut elm = format!("-Article_{}", number(art)?);
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
    Some(elm)
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
            if c.fragments.is_empty() {
                // Cited whole: what a `file` source allows, and what a law is told it cannot.
                out.push((c.source.clone(), String::new(), who.clone(), c.span.clone()));
            }
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
                    if frag.is_empty() {
                        out.push(
                            Diag::error("E037", tr!("法令 `{name}` の引用に箇所がありません", "A citation of the law `{name}` names no article"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("法令は条や別表の単位で写すので、`@{name} 第20条` のように、どこを引いたかを書いてください。", "A law is copied an article at a time, so say which one: `@{name} 第20条`."))
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos))),
                        );
                        continue;
                    }
                    let Some(fr) = fragment(frag) else {
                        out.push(
                            Diag::error("E037", tr!("引用箇所 `{frag}` の書き方が読めません", "The fragment `{frag}` cannot be read"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("書けるのは `第20条`、`第20条の2`、`第20条第2項`、`第20条第2項第3号`、`別表第一`、`附則第3条`、`附則（令和七年三月三一日法律第一三号）第3条` の形です。号の細分はまだ受けません。", "The forms are `第20条`, `第20条の2`, `第20条第2項`, `第20条第2項第3号`, `別表第一`, `附則第3条` and `附則（令和七年三月三一日法律第一三号）第3条`. Sub-items are not read yet."))
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

/// Whether two copies of a fragment say the same thing: the text, not the markup. e-Gov
/// rewrites the attributes and the line structure of articles an amendment did not touch —
/// 38 of the 103 byte-level differences found across five laws had no difference in the text
/// (§15.71) — and an amendment that changes no sentence is not one.
pub fn same_text(a: &[u8], b: &[u8]) -> bool {
    xml_text(&String::from_utf8_lossy(a)) == xml_text(&String::from_utf8_lossy(b))
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

/// One request, once.
fn curl_once(url: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "120", url])
        .output()
        .map_err(|e| tr!("curl を起動できません: {e}", "cannot run curl: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// One request, tried three times: e-Gov answers a busy hour with a refusal, and a scheduled
/// job that stops on one is a job nobody reads (§15.71).
fn curl(url: &str) -> Result<Vec<u8>, String> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
        }
        match curl_once(url) {
            Ok(body) => return Ok(body),
            Err(e) => last = e,
        }
    }
    Err(tr!("{url} を取れません（三度試しました）: {last}", "cannot fetch {url} (tried three times): {last}"))
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

/// One e-Gov reply as JSON. A busy e-Gov answers a request it accepted with a page that is
/// not JSON; that is tried again, with a pause, before it is an error.
fn fetch_json(url: &str) -> Result<crate::json::Json, String> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
        }
        let body = curl(url)?;
        let text = String::from_utf8_lossy(&body);
        match crate::json::parse(&text) {
            Ok(j) => return Ok(j),
            Err(e) => last = tr!("e-Gov の答えが JSON として読めません: {e}", "the e-Gov reply is not JSON: {e}"),
        }
    }
    Err(last)
}

/// The XML inside an e-Gov `law_data` reply, with the revision id it came from.
fn law_xml(j: &crate::json::Json) -> Result<(Vec<u8>, String), String> {
    let b64 = j.get("law_full_text").and_then(|x| x.as_str()).ok_or_else(|| tr!("e-Gov の答えに law_full_text がありません", "the e-Gov reply has no law_full_text"))?;
    let xml = base64_decode(b64).ok_or_else(|| tr!("law_full_text を base64 として読めません", "law_full_text is not base64"))?;
    let rev = j.get("revision_info").and_then(|r| r.get("law_revision_id")).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Ok((xml, rev))
}

/// The supplementary provisions of a law in document order, each with the number of the
/// amending law it came with (`None` for the law's own): what `SupplProvision[k]` counts.
pub fn suppl_ordinals(xml: &str) -> Vec<Option<String>> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<SupplProvision") {
        let after = &rest[i + "<SupplProvision".len()..];
        // `<SupplProvisionLabel>` begins the same way; only the element itself counts.
        if !after.starts_with(|c: char| c == ' ' || c == '>' || c == '\n' || c == '\t' || c == '/') {
            rest = after;
            continue;
        }
        let end = after.find('>').unwrap_or(after.len());
        let tag = &after[..end];
        let num = tag.find("AmendLawNum=\"").map(|p| {
            let v = &tag[p + "AmendLawNum=\"".len()..];
            v[..v.find('"').unwrap_or(v.len())].to_string()
        });
        out.push(num);
        rest = &after[end..];
    }
    out
}

/// Where an amending law's supplementary provisions sit in the law as of a date: the `k` of
/// `SupplProvision[k]`, counted from one. One read of the whole law per date, kept for the
/// command's duration — 14 MB for the special taxation measures law, so not once a fragment.
fn suppl_index(id: &str, asof: &str, amend: &str, cache: &mut BTreeMap<String, Vec<Option<String>>>) -> Result<usize, String> {
    let key = format!("{id}@{asof}");
    if !cache.contains_key(&key) {
        let j = fetch_json(&format!("https://laws.e-gov.go.jp/api/2/law_data/{id}?asof={asof}&law_full_text_format=xml"))?;
        let (xml, _) = law_xml(&j)?;
        cache.insert(key.clone(), suppl_ordinals(&String::from_utf8_lossy(&xml)));
    }
    cache[&key]
        .iter()
        .position(|n| n.as_deref() == Some(amend))
        .map(|p| p + 1)
        .ok_or_else(|| tr!("法令 {id} の {asof} 時点に、附則（{amend}）がありません", "the law {id} as of {asof} has no supplementary provisions of {amend}"))
}

/// Where the supplementary provisions found at `k0` as of one date sit as of `date`: e-Gov's
/// order shifts by a few as laws come in (the 2025 income tax act's moved from 349 to 350
/// between two dates), so the positions around `k0` are tried first, each one a small
/// request, and the whole law is read only when none of them is it.
fn suppl_index_near(id: &str, date: &str, amend: &str, k0: usize, cache: &mut BTreeMap<String, Vec<Option<String>>>) -> Result<usize, String> {
    if !cache.contains_key(&format!("{id}@{date}")) {
        for delta in [0i64, 1, 2, -1, 3, -2, 4, -3] {
            let k = k0 as i64 + delta;
            if k < 1 {
                continue;
            }
            let Ok(body) = curl_once(&law_url(id, date, &format!("SupplProvision[{k}]"))) else { continue };
            let Ok(j) = crate::json::parse(&String::from_utf8_lossy(&body)) else { continue };
            let Ok((xml, _)) = law_xml(&j) else { continue };
            if suppl_ordinals(&String::from_utf8_lossy(&xml)).first().is_some_and(|n| n.as_deref() == Some(amend)) {
                return Ok(k as usize);
            }
        }
    }
    suppl_index(id, date, amend, cache)
}

/// Fetch one fragment as of `asof`: the XML and the revision id it came from. The position of
/// an amending law's supplementary provisions is looked up as of `index_asof`, the date the
/// rule reads the law at, and moved to `asof` from there when the two differ.
fn fetch_fragment(
    id: &str,
    asof: &str,
    index_asof: &str,
    fr: &Fragment,
    cache: &mut BTreeMap<String, Vec<Option<String>>>,
) -> Result<(Vec<u8>, String), String> {
    let elm = match &fr.amend {
        Some(a) => {
            let k0 = suppl_index(id, index_asof, a, cache)?;
            let k = if asof == index_asof { k0 } else { suppl_index_near(id, asof, a, k0, cache)? };
            fr.elm_at(k)
        }
        None => fr.elm.clone(),
    };
    law_xml(&fetch_json(&law_url(id, asof, &elm))?)
}

/// A law's number as a person names it, from its e-Gov id: `508AC0000000012` is
/// 令和8年法律第12号 (Act No. 12 of 2026). Anything that is not an act is left as the id.
pub fn law_num_text(id: &str) -> String {
    let b = id.as_bytes();
    if b.len() < 6 || !b[..3].iter().all(|c| c.is_ascii_digit()) || &id[3..5] != "AC" {
        return id.to_string();
    }
    let (era, base) = match b[0] {
        b'1' => ("明治", 1867),
        b'2' => ("大正", 1911),
        b'3' => ("昭和", 1925),
        b'4' => ("平成", 1988),
        b'5' => ("令和", 2018),
        _ => return id.to_string(),
    };
    let year: u32 = id[1..3].parse().unwrap_or(0);
    let num = id[5..].trim_start_matches('0');
    let num = if num.is_empty() { "0" } else { num };
    let ad = base + year;
    tr!("{era}{year}年法律第{num}号", "Act No. {num} of {ad}")
}

/// What a copy's `revision.txt` says, for a person: the date the text came into force and the
/// amending law, from a revision id like `332AC0000000026_20260401_508AC0000000012`.
pub fn revision_words(rev: &str) -> Option<String> {
    let mut it = rev.split('_');
    let (_, date, amend) = (it.next()?, it.next()?, it.next()?);
    if date.len() != 8 || !date.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let d = format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]);
    let law = law_num_text(amend);
    Some(tr!("{d} 施行、{law}による改正後", "in force from {d}, as amended by {law}"))
}

/// What changed between two copies, as lines: those only in the old copy with `-`, those only
/// in the new with `+`, in order, at most `cap` of each. A sentence replaced shows as one of
/// each — enough to see whether an amount or a date moved, which a digest cannot say.
pub fn text_diff(old: &str, new: &str, cap: usize) -> Vec<String> {
    let (a, b) = (xml_text(old), xml_text(new));
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let gone: Vec<&str> = al.iter().filter(|l| !bl.contains(l)).cloned().collect();
    let came: Vec<&str> = bl.iter().filter(|l| !al.contains(l)).cloned().collect();
    let mut out = Vec::new();
    for (sign, ls) in [("-", gone), ("+", came)] {
        for l in ls.iter().take(cap) {
            out.push(format!("{sign} {l}"));
        }
        if ls.len() > cap {
            let more = ls.len() - cap;
            out.push(tr!("{sign} …（あと {more} 行）", "{sign} … ({more} more lines)"));
        }
    }
    out
}

/// `rulec source fetch`: put a copy of every cited fragment beside the rule, saying which
/// ones changed against the copies already there. The one command that reads the network.
pub fn fetch(f: &RuleFile, rule_path: &str) -> Result<Outcome, String> {
    let cites = citations(f);
    let mut lines = Vec::new();
    let mut changed = false;
    let mut cache = BTreeMap::new();
    for d in &f.sources {
        let SourceKind::Law { id, asof } = &d.kind else { continue };
        let cdir = copy_dir(rule_path, id, asof);
        for (frag, _) in cited_from(&cites, &d.name.text) {
            if frag.is_empty() {
                continue;
            }
            let Some(fr) = fragment(&frag) else {
                lines.push(tr!("{}: 引用箇所 `{frag}` の書き方が読めません", "{}: the fragment `{frag}` cannot be read", d.name.text));
                continue;
            };
            let (xml, rev) = fetch_fragment(id, asof, asof, &fr, &mut cache)?;
            std::fs::create_dir_all(&cdir).map_err(|e| format!("{}: {e}", cdir.display()))?;
            let p = cdir.join(fr.file());
            let before = std::fs::read(&p).ok();
            // A copy whose text the fetch did not change keeps its bytes, so that the pin
            // stays true: only the markup differed, and rewriting it would fail the next
            // `check` (E038) over an amendment that changed nothing.
            let same = before.as_deref().is_some_and(|b| same_text(b, &xml));
            if !same {
                std::fs::write(&p, &xml).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            if !rev.is_empty() {
                let _ = std::fs::write(cdir.join("revision.txt"), format!("{rev}\n"));
            }
            let h = crate::sha256::short(if same { before.as_deref().unwrap_or(&xml) } else { &xml });
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
                    let Some(fr) = fragment(&frag).filter(|_| !frag.is_empty()) else { continue };
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
/// its `asof`, and say which cited fragments differ there from the copies — in their text,
/// what changed, and what to write next. The other command that reads the network; meant for
/// a scheduled job, since `check` cannot know of an amendment on its own.
pub fn outdated(f: &RuleFile, rule_path: &str) -> Result<Outcome, String> {
    let cites = citations(f);
    let mut lines = Vec::new();
    let mut changed = false;
    let mut cache = BTreeMap::new();
    // A rule that folds two periods reads the same law as of two dates (§15.71). An
    // amendment is measured against the latest reading; an earlier one is old by design.
    let mut latest: BTreeMap<&str, &str> = BTreeMap::new();
    for d in &f.sources {
        if let SourceKind::Law { id, asof } = &d.kind {
            let e = latest.entry(id.as_str()).or_insert(asof.as_str());
            if asof.as_str() > *e {
                *e = asof.as_str();
            }
        }
    }
    // The one date input, when there is exactly one: what the rows for the new text are keyed on.
    let date_inputs: Vec<&str> = f.inputs.iter().filter(|i| i.ty.base == crate::kw::DATE).map(|i| i.name.text.as_str()).collect();
    for d in &f.sources {
        let SourceKind::Law { id, asof } = &d.kind else { continue };
        if latest.get(id.as_str()).is_some_and(|l| *l != asof.as_str()) {
            lines.push(tr!(
                "{}: 同じ法令をもっと後の時点で引用している出典があるので、改正はそちらで確かめます",
                "{}: another source reads this law as of a later date; amendments are asked about there",
                d.name.text
            ));
            continue;
        }
        let j = fetch_json(&format!("https://laws.e-gov.go.jp/api/2/law_revisions/{id}"))?;
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
        // Date → the fragments that change on that day, each with what changed. Each date is
        // held to the text of the date before it, starting from the copy, so an amendment is
        // reported once, on the day it comes into force, and not again on every later day.
        let mut differs: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
        for (frag, _) in cited_from(&cites, &d.name.text) {
            let Some(fr) = fragment(&frag).filter(|_| !frag.is_empty()) else { continue };
            let mut prev = std::fs::read(cdir.join(fr.file())).ok();
            for (date, _) in &later {
                let (xml, _) = fetch_fragment(id, date, asof, &fr, &mut cache)?;
                // The text, not the bytes: a revision that only re-marked the article is
                // not an amendment of it.
                if !prev.as_deref().is_some_and(|n| same_text(n, &xml)) {
                    let diff = match &prev {
                        Some(n) => text_diff(&String::from_utf8_lossy(n), &String::from_utf8_lossy(&xml), 8),
                        None => Vec::new(),
                    };
                    differs.entry(date.clone()).or_default().push((frag.clone(), diff));
                }
                prev = Some(xml);
            }
        }
        for (date, _) in &later {
            match differs.get(date) {
                Some(frags) => {
                    changed = true;
                    let names: Vec<&str> = frags.iter().map(|(f, _)| f.as_str()).collect();
                    lines.push(tr!(
                        "{}: {date} 施行の改正で {} が変わります。その日から効く版には、読み直した規則が要ります",
                        "{}: the amendment enforced on {date} changes {}; the version in force from that day needs a reread rule",
                        d.name.text,
                        names.join(if crate::i18n::ja() { "、" } else { ", " })
                    ));
                    // Where the landing is (§15.71): a second reading of the law as of that
                    // day, and rows keyed on the date from which it applies.
                    let name = format!("{}_{}", d.name.text, date.replace('-', ""));
                    let rows = match date_inputs.as_slice() {
                        [one] => tr!("{one} >={date} の行", "the rows for {one} >={date}"),
                        _ => tr!("その日以後に効く行", "the rows in force from that day"),
                    };
                    lines.push(tr!(
                        "  `{} {name} = {} \"{id}\" {} {date}` を足し、{rows}をそこから写してください",
                        "  add `{} {name} = {} \"{id}\" {} {date}` and transcribe {rows} from it",
                        crate::kw::SOURCE,
                        crate::kw::LAW,
                        crate::kw::ASOF
                    ));
                    for (frag, diff) in frags {
                        for l in diff {
                            lines.push(format!("  {frag}: {l}"));
                        }
                    }
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
        assert!(fragment("第3条ただし書").is_none());
    }

    #[test]
    fn supplementary_provisions_are_named_by_their_amending_law() {
        let own = fragment("附則第3条").unwrap();
        assert_eq!(own.elm, "SupplProvision-Article_3");
        assert_eq!(own.file(), "SupplProvision-Article_3.xml");
        assert_eq!(fragment("附則").unwrap().elm, "SupplProvision");
        let a = fragment("附則（令和七年三月三一日法律第一三号）第3条第2項").unwrap();
        assert_eq!(a.elm, "SupplProvision[?]-Article_3-Paragraph_2");
        assert_eq!(a.amend.as_deref(), Some("令和七年三月三一日法律第一三号"));
        assert_eq!(a.file(), "SupplProvision_令和七年三月三一日法律第一三号-Article_3-Paragraph_2.xml");
        assert_eq!(a.elm_at(244), "SupplProvision[244]-Article_3-Paragraph_2");
        let whole = fragment("附則(令和六年三月三〇日法律第八号)").unwrap();
        assert_eq!(whole.elm, "SupplProvision[?]");
        assert_eq!(whole.file(), "SupplProvision_令和六年三月三〇日法律第八号.xml");
        assert!(fragment("附則（）第3条").is_none());
        assert!(fragment("附則の3").is_none());
    }

    #[test]
    fn supplementary_provisions_are_counted_in_document_order() {
        let xml = "<Law><MainProvision/><SupplProvision Extract=\"true\"><SupplProvisionLabel>附　則</SupplProvisionLabel></SupplProvision>\n<SupplProvision AmendLawNum=\"昭和四二年七月一三日法律第五六号\" Extract=\"true\"><SupplProvisionLabel>附　則</SupplProvisionLabel></SupplProvision><SupplProvision AmendLawNum=\"令和七年三月三一日法律第一三号\"></SupplProvision></Law>";
        let ords = suppl_ordinals(xml);
        assert_eq!(ords, vec![None, Some("昭和四二年七月一三日法律第五六号".into()), Some("令和七年三月三一日法律第一三号".into())]);
        assert_eq!(ords.iter().position(|n| n.as_deref() == Some("令和七年三月三一日法律第一三号")).map(|p| p + 1), Some(3));
    }

    #[test]
    fn a_law_number_reads_from_its_id() {
        let s = law_num_text("508AC0000000012");
        assert!(s == "令和8年法律第12号" || s == "Act No. 12 of 2026", "{s}");
        let s = law_num_text("430AC0000000007");
        assert!(s == "平成30年法律第7号" || s == "Act No. 7 of 2018", "{s}");
        assert_eq!(law_num_text("508CO0000000012"), "508CO0000000012");
        let w = revision_words("332AC0000000026_20260401_508AC0000000012").unwrap();
        assert!(w.starts_with("2026-04-01"), "{w}");
        assert!(revision_words("nonsense").is_none());
    }

    #[test]
    fn a_diff_says_which_lines_moved() {
        let a = "<Article><Sentence>平成二十六年四月一日から令和六年三月三十一日までの間</Sentence><Sentence>二百円</Sentence></Article>";
        let b = "<Article><Sentence>平成二十六年四月一日から令和九年三月三十一日までの間</Sentence><Sentence>二百円</Sentence></Article>";
        let d = text_diff(a, b, 8);
        assert_eq!(d, vec!["- 平成二十六年四月一日から令和六年三月三十一日までの間", "+ 平成二十六年四月一日から令和九年三月三十一日までの間"]);
        assert!(text_diff(a, a, 8).is_empty());
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
    fn markup_alone_is_not_a_change() {
        let a = "<Article Num=\"1\"><Paragraph Num=\"1\"><Sentence>甲は、乙とする。</Sentence></Paragraph></Article>";
        let b = "<Article Num=\"1\">\n  <Paragraph Num=\"1\">\n    <Sentence Num=\"1\" WritingMode=\"vertical\">甲は、乙とする。</Sentence>\n  </Paragraph>\n</Article>";
        assert!(same_text(a.as_bytes(), b.as_bytes()));
        assert!(!same_text(a.as_bytes(), a.replace("乙", "丙").as_bytes()));
    }

    #[test]
    fn xml_becomes_lines() {
        let x = "<Article Num=\"1\"><ArticleTitle>第一条</ArticleTitle><Paragraph Num=\"1\"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>";
        assert_eq!(xml_text(x), "第一条\n甲は、乙とする。");
    }
}
