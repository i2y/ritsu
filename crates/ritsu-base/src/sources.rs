//! The copies of sources (DESIGN 4.6): where a law's articles are kept beside the file that
//! cites them, the file each article is copied into, the text of a copy, the pin that holds a
//! citation to its copy, and the requests that bring a copy from e-Gov or the eCFR.
//!
//! rulec, koyomi and yuen keep the same copy of the same article at the same path:
//! `sources/law/<id>@<asof>/<element>.xml`, the element as e-Gov addresses it
//! (`第143条第2項` → `MainProvision-Article_143-Paragraph_2`, `別表第一` → `AppdxTable_1`, the
//! supplementary provisions of an amending law under that law's number, rulec's §15.71), and
//! for the eCFR `sources/law/29-CFR-1910@<asof>/1910.157.xml`. This module is what the three
//! wrote, once.
//!
//! The commands (`source fetch | pin | outdated`) stay each language's: the lines they rewrite
//! are the language's syntax. `check` never reads the network; only the functions under
//! "Requests" do, through `curl` as a child process (no TLS of ritsu's own, no dependency).

use crate::json::{self, Json};
use crate::text::Text;
use crate::tr;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a law is read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LawDb {
    /// e-Gov law API v2 (Japan).
    Egov,
    /// The eCFR versioner API (the US Code of Federal Regulations).
    Ecfr,
}

impl LawDb {
    /// As a file writes it: `egov`, `ecfr`.
    pub fn word(self) -> &'static str {
        match self {
            LawDb::Egov => "egov",
            LawDb::Ecfr => "ecfr",
        }
    }

    pub fn from_word(w: &str) -> Option<LawDb> {
        match w {
            "egov" => Some(LawDb::Egov),
            "ecfr" => Some(LawDb::Ecfr),
            _ => None,
        }
    }

    /// As a message names it: `e-Gov`, `eCFR`.
    pub fn title(self) -> &'static str {
        match self {
            LawDb::Egov => "e-Gov",
            LawDb::Ecfr => "eCFR",
        }
    }
}

// ── Where an article is ─────────────────────────────────────────────────────

/// A number as a law writes it: `百四十三`, `二十`, `千五十`, or ASCII digits. Zero is no
/// number of an article.
pub fn law_number(s: &str) -> Option<u32> {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        return s.parse().ok().filter(|n| *n > 0);
    }
    let digit = |c: char| "一二三四五六七八九".chars().position(|x| x == c).map(|i| i as u32 + 1);
    let unit = |c: char| match c {
        '十' => Some(10),
        '百' => Some(100),
        '千' => Some(1000),
        _ => None,
    };
    let mut total = 0u32;
    let mut pending: Option<u32> = None;
    let mut last_unit = 10_000u32;
    for c in s.chars() {
        if let Some(d) = digit(c) {
            if pending.is_some() {
                return None;
            }
            pending = Some(d);
        } else if let Some(u) = unit(c) {
            if u >= last_unit {
                return None;
            }
            total += pending.take().unwrap_or(1) * u;
            last_unit = u;
        } else {
            return None;
        }
    }
    total += pending.unwrap_or(0);
    if total == 0 { None } else { Some(total) }
}

/// `第20条の2第3項第4号` as the tail of an element path: `-Article_20_2-Paragraph_3-Item_4`.
fn article_suffix(s: &str) -> Option<String> {
    let rest = s.strip_prefix('第')?;
    let (art, rest) = rest.split_once('条')?;
    let mut elm = format!("-Article_{}", law_number(art)?);
    let mut rest = rest;
    // `第20条の2`: a sub-numbered article.
    if let Some(r) = rest.strip_prefix('の') {
        let end = r.find('第').unwrap_or(r.len());
        elm.push_str(&format!("_{}", law_number(&r[..end])?));
        rest = &r[end..];
    }
    if let Some(r) = rest.strip_prefix('第') {
        let (para, r) = r.split_once('項')?;
        elm.push_str(&format!("-Paragraph_{}", law_number(para)?));
        rest = r;
        if let Some(r) = rest.strip_prefix('第') {
            let (item, r) = r.split_once('号')?;
            elm.push_str(&format!("-Item_{}", law_number(item)?));
            rest = r;
        }
    }
    if !rest.is_empty() {
        return None;
    }
    Some(elm)
}

/// A fragment of a law, as a citation names it and as the database addresses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment {
    /// As written: `第91条`, `第20条の2第3項`, `別表第一`, `附則（令和七年三月三一日法律第一三号）第3条`,
    /// `§1910.157`.
    pub name: String,
    /// The element path: e-Gov's `elm` (`MainProvision-Article_91`, `AppdxTable[1]`), with
    /// `[?]` where the position of an amending law's supplementary provisions goes — e-Gov
    /// counts them in document order and does not take the amending law's number — or the
    /// eCFR's section (`1910.157`).
    pub elm: String,
    /// The amending law whose supplementary provisions these are, as e-Gov's `AmendLawNum`
    /// spells it.
    pub amend: Option<String>,
}

impl Fragment {
    /// The file the copy is kept in: the element path with its brackets made plain. The
    /// supplementary provisions of an amending law are filed under that law's number.
    pub fn file(&self) -> String {
        match &self.amend {
            Some(n) => format!("{}.xml", self.elm.replace("SupplProvision[?]", &format!("SupplProvision_{n}"))),
            None => format!("{}.xml", self.elm.replace('[', "_").replace(']', "")),
        }
    }

    /// The element path with the position of the supplementary provisions filled in.
    pub fn elm_at(&self, k: usize) -> String {
        self.elm.replace("[?]", &format!("[{k}]"))
    }

    /// An article, paragraph or item of the main provisions: what koyomi cites.
    pub fn is_main_provision(&self) -> bool {
        self.elm.starts_with("MainProvision")
    }
}

/// The fragment a citation names, read the way its database writes one; None when it is not
/// written in a form the database is read in.
pub fn fragment(db: LawDb, name: &str) -> Option<Fragment> {
    match db {
        LawDb::Egov => egov_fragment(name),
        LawDb::Ecfr => ecfr_fragment(name),
    }
}

/// The file a fragment is copied into, without the directory.
pub fn fragment_file(db: LawDb, name: &str) -> Option<String> {
    fragment(db, name).map(|f| f.file())
}

/// A section of the CFR: `§1910.157`, or the same without the sign. A paragraph of one
/// (`(d)(2)`) is not addressed: the eCFR serves a section at a time.
fn ecfr_fragment(name: &str) -> Option<Fragment> {
    let s = name.strip_prefix('§').unwrap_or(name).trim();
    let (part, sec) = s.split_once('.')?;
    if part.is_empty() || sec.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) || !sec.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(Fragment { name: name.to_string(), elm: s.to_string(), amend: None })
}

/// Articles, paragraphs and items of the main provisions; appendix tables by their ordinal;
/// the supplementary provisions, the law's own as `附則第3条` and an amending law's as
/// `附則（令和七年三月三一日法律第一三号）第3条`, either also whole without the article.
fn egov_fragment(name: &str) -> Option<Fragment> {
    if let Some(rest) = name.strip_prefix("別表第") {
        let n = law_number(rest)?;
        return Some(Fragment { name: name.to_string(), elm: format!("AppdxTable[{n}]"), amend: None });
    }
    if let Some(rest) = name.strip_prefix("附則") {
        let (amend, rest) = match rest.strip_prefix('（').or_else(|| rest.strip_prefix('(')) {
            Some(r) => {
                let end = r.find(['）', ')'])?;
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

/// What e-Gov's `elm` asks for, from the file an article is copied into (the other way from
/// [`Fragment::file`]): the element path, `[?]` where the position of an amending law's
/// supplementary provisions goes, and that law's number.
pub fn element_of_file(file: &str) -> (String, Option<String>) {
    let stem = file.trim_end_matches(".xml");
    if let Some(rest) = stem.strip_prefix("AppdxTable_") {
        return (format!("AppdxTable[{rest}]"), None);
    }
    if let Some(rest) = stem.strip_prefix("SupplProvision_") {
        let (num, tail) = match rest.find("-Article_") {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        return (format!("SupplProvision[?]{tail}"), Some(num.to_string()));
    }
    (stem.to_string(), None)
}

/// The directory a law's copies as of a date are kept in, from the directory of the file
/// that cites it: `sources/law/<id>@<asof>`, a CFR id's spaces and slashes made `-`.
pub fn copy_dir(id: &str, asof: &str) -> String {
    format!("sources/law/{}@{asof}", id.replace([' ', '/'], "-"))
}

/// The full path of a copy, beside the directory `dir` of the file that cites it.
pub fn copy_path(dir: &Path, id: &str, asof: &str, file: &str) -> PathBuf {
    dir.join(copy_dir(id, asof)).join(file)
}

/// `revision.txt` beside the copies, when there is one: the version of the law e-Gov served.
pub fn revision(dir: &Path) -> Option<String> {
    crate::fs::read_to_string(dir.join("revision.txt")).ok().map(|r| r.trim().to_string()).filter(|r| !r.is_empty())
}

// ── The text of a copy ──────────────────────────────────────────────────────

/// The text of a law's XML, with the tags removed: a line for each paragraph, item, sentence
/// and table row, the cells of a row spaced apart, blanks folded, a line that repeats the one
/// before it dropped. What a diff of two copies shows, and what two copies are compared by.
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
        if !t.is_empty() && lines.last() != Some(&t) {
            lines.push(t);
        }
    }
    lines.join("\n")
}

/// Whether two copies say the same: their text, not their markup. e-Gov rewrites the
/// attributes and the line structure of articles an amendment did not touch (rulec's §15.71
/// found 38 of 103 byte differences across five laws with no difference in the text).
pub fn same_text(a: &[u8], b: &[u8]) -> bool {
    xml_text(&String::from_utf8_lossy(a)) == xml_text(&String::from_utf8_lossy(b))
}

/// U+3000, the space a Japanese law puts after an article's or a paragraph's number.
const WIDE_SPACE: char = '\u{3000}';

/// An article's XML as the law prints it, for a page a person reads: the caption on a line of
/// its own, the article's number and its first paragraph on the next, and then a line for every
/// other paragraph, item and subitem, its number and its sentences together. Ruby readings are
/// left out. [`xml_text`] keeps a line a sentence instead.
pub fn article_lines(xml: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_rt = 0usize;
    let mut rest = xml;
    let space = |cur: &mut String| {
        if !cur.is_empty() && !cur.ends_with(WIDE_SPACE) {
            cur.push(WIDE_SPACE);
        }
    };
    let finish = |cur: &mut String, lines: &mut Vec<String>| {
        let t = cur.trim_end_matches(WIDE_SPACE).to_string();
        if !t.is_empty() {
            lines.push(t);
        }
        cur.clear();
    };
    while let Some(lt) = rest.find('<') {
        if in_rt == 0 {
            cur.push_str(rest[..lt].trim());
        }
        let Some(gt) = rest[lt..].find('>') else { break };
        let tag = rest[lt + 1..lt + gt].trim();
        rest = &rest[lt + gt + 1..];
        if tag.ends_with('/') || tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        let (closing, name) = match tag.strip_prefix('/') {
            Some(n) => (true, n.trim()),
            None => (false, tag.split_whitespace().next().unwrap_or("")),
        };
        if name == "Rt" {
            in_rt = if closing { in_rt.saturating_sub(1) } else { in_rt + 1 };
            continue;
        }
        if !closing {
            continue;
        }
        match name {
            "ArticleCaption" | "ParagraphCaption" => finish(&mut cur, &mut lines),
            "ArticleTitle" | "ParagraphNum" | "ItemTitle" | "Subitem1Title" | "Subitem2Title" | "Column" => space(&mut cur),
            "ParagraphSentence" | "ItemSentence" | "Subitem1Sentence" | "Subitem2Sentence" => finish(&mut cur, &mut lines),
            _ => {}
        }
    }
    if in_rt == 0 {
        cur.push_str(rest.trim());
    }
    finish(&mut cur, &mut lines);
    lines
}

/// The lines a copy is quoted by: an e-Gov article as the law prints it, anything else (a
/// table, a section of the CFR) a line for each line of its text.
pub fn quote_lines(db: LawDb, file: &str, xml: &str) -> Vec<String> {
    if db == LawDb::Egov && (file.starts_with("MainProvision") || file.contains("-Article_")) {
        article_lines(xml)
    } else {
        xml_text(xml).lines().map(|s| s.to_string()).collect()
    }
}

/// The name of the first element of an XML document, after its declaration and comments.
pub fn first_element(xml: &str) -> Option<&str> {
    let mut rest = xml.trim_start_matches('\u{feff}');
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("<?") {
            rest = &r[r.find("?>")? + 2..];
        } else if let Some(r) = rest.strip_prefix("<!--") {
            rest = &r[r.find("-->")? + 3..];
        } else if let Some(r) = rest.strip_prefix('<') {
            let end = r.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(r.len());
            return Some(&r[..end]);
        } else {
            return None;
        }
    }
}

/// The element a copy starts with: `Article`, `Paragraph`, `Item`, `AppdxTable`,
/// `SupplProvision` for e-Gov, `DIV8` (a section) for the eCFR.
pub fn root_element(db: LawDb, file: &str) -> String {
    match db {
        LawDb::Ecfr => "DIV8".to_string(),
        LawDb::Egov => {
            let stem = file.trim_end_matches(".xml");
            let last = stem.rsplit('-').next().unwrap_or(stem);
            last.split('_').next().unwrap_or(last).to_string()
        }
    }
}

/// Whether a copy is what the database serves for the article: UTF-8 XML whose first element
/// is the one the article is addressed by, with some text in it.
pub fn readable(db: LawDb, file: &str, bytes: &[u8]) -> Result<(), Text> {
    let Ok(xml) = std::str::from_utf8(bytes) else {
        return Err(tr!("UTF-8 として読めません", "it is not UTF-8"));
    };
    let want = root_element(db, file);
    match first_element(xml) {
        None => Err(tr!("XML ではありません", "it is not XML")),
        Some(got) if got != want => {
            let db = db.title();
            Err(tr!("{db} から取ったコピーなら最初の要素は <{want}> ですが、<{got}> です", "a copy from {db} starts with <{want}>, and this one starts with <{got}>"))
        }
        Some(_) if xml_text(xml).is_empty() => Err(tr!("本文がありません", "it has no text")),
        Some(_) => Ok(()),
    }
}

/// The supplementary provisions of a law in document order, each with the number of the
/// amending law it came with (None for the law's own): what `SupplProvision[k]` counts.
pub fn suppl_ordinals(xml: &str) -> Vec<Option<String>> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<SupplProvision") {
        let after = &rest[i + "<SupplProvision".len()..];
        // `<SupplProvisionLabel>` begins the same way; only the element itself counts.
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

/// The lines only in `old` and the lines only in `new`, each in order.
pub fn diff_lines<'a>(old: &'a str, new: &'a str) -> (Vec<&'a str>, Vec<&'a str>) {
    let o: Vec<&str> = old.lines().collect();
    let n: Vec<&str> = new.lines().collect();
    let gone = o.iter().filter(|l| !n.contains(l)).copied().collect();
    let came = n.iter().filter(|l| !o.contains(l)).copied().collect();
    (gone, came)
}

/// What changed between the texts of two copies, a line each: `- ` before a line only in the
/// old, `+ ` before a line only in the new, at most `cap` of each, and a line that says how
/// many more (rulec's). A sentence replaced shows as one of each, enough to see whether an
/// amount or a date moved, which a digest cannot say.
pub fn text_diff(old: &str, new: &str, cap: usize) -> Vec<Text> {
    let (gone, came) = diff_lines(old, new);
    let mut out = Vec::new();
    for (sign, ls) in [("-", gone), ("+", came)] {
        for l in ls.iter().take(cap) {
            out.push(Text::same(format!("{sign} {l}")));
        }
        if ls.len() > cap {
            let more = ls.len() - cap;
            out.push(tr!("{sign} …（あと {more} 行）", "{sign} … ({more} more lines)"));
        }
    }
    out
}

/// A law's number as a person names it, from its e-Gov id: `508AC0000000012` is
/// 令和8年法律第12号 (Act No. 12 of 2026). Anything that is not an act is left as the id.
pub fn law_num_text(id: &str) -> Text {
    let b = id.as_bytes();
    if b.len() < 6 || !b[..3].iter().all(|c| c.is_ascii_digit()) || &id[3..5] != "AC" {
        return Text::same(id);
    }
    let (era, base) = match b[0] {
        b'1' => ("明治", 1867),
        b'2' => ("大正", 1911),
        b'3' => ("昭和", 1925),
        b'4' => ("平成", 1988),
        b'5' => ("令和", 2018),
        _ => return Text::same(id),
    };
    let year: u32 = id[1..3].parse().unwrap_or(0);
    let num = id[5..].trim_start_matches('0');
    let num = if num.is_empty() { "0" } else { num };
    let ad = base + year;
    tr!("{era}{year}年法律第{num}号", "Act No. {num} of {ad}")
}

/// What a copy's `revision.txt` says, for a person: the date the text came into force and the
/// amending law, from a revision id like `332AC0000000026_20260401_508AC0000000012`.
pub fn revision_words(rev: &str) -> Option<Text> {
    let mut it = rev.split('_');
    let (_, date, amend) = (it.next()?, it.next()?, it.next()?);
    if date.len() != 8 || !date.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let d = format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]);
    let law = law_num_text(amend);
    Some(tr!("{d} 施行、{}による改正後", "in force from {d}, as amended by {}", law.ja; law.en))
}

// ── Pins ────────────────────────────────────────────────────────────────────

/// The byte offset of the `sha256:` of a line and of the `#` that starts its comment, each
/// outside the strings of the line (a string may hold `\"`).
fn pin_marks(line: &str) -> (Option<usize>, Option<usize>) {
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

/// The line with `sha256:<pin>` in place of the hex digits it has, or with ` sha256:<pin>` added
/// after its last word (before a comment). Nothing else on the line changes, its line ending
/// and its spaces among them: what `source pin` writes (koyomi's and yuen's).
pub fn pinned(line: &str, pin: &str) -> String {
    let (at, comment) = pin_marks(line);
    if let Some(i) = at {
        let start = i + "sha256:".len();
        let end = start + line[start..].bytes().take_while(|b| b.is_ascii_hexdigit()).count();
        return format!("{}{pin}{}", &line[..start], &line[end..]);
    }
    let body_end = comment.unwrap_or(line.len());
    let content = line[..body_end].trim_end();
    format!("{content} sha256:{pin}{}", &line[content.len()..])
}

/// The line as a diagnostic offers it fixed: its end trimmed, and `sha256:` with the sixteen
/// characters after it replaced by the pin, or the pin added at its end (koyomi's and yuen's
/// "the line, fixed").
pub fn fixed_pin_line(line: &str, pin: &str) -> String {
    let t = line.trim_end();
    match t.find("sha256:") {
        Some(i) => format!("{}sha256:{pin}{}", &t[..i], &t[(i + 23).min(t.len())..]),
        None => format!("{t} sha256:{pin}"),
    }
}

/// The line with its `sha256:` word set to `hash`, replaced where there is one and added where
/// there is none, a comment kept two spaces after it (rulec's `rulec source pin`, which does not
/// look into strings).
pub fn pinned_spaced(line: &str, hash: &str) -> String {
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

// ── Base64 ──────────────────────────────────────────────────────────────────

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// RFC 4648, the standard alphabet with `=` padding.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

/// The bytes of standard base64 with its padding, or None when the text is not that.
/// Whitespace (a line break every 76 characters, as some servers send) is skipped (yuen's).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut vals = Vec::with_capacity(text.len());
    let mut pad = 0;
    for c in text.bytes() {
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'=' {
            pad += 1;
            continue;
        }
        if pad > 0 {
            return None;
        }
        vals.push(ALPHABET.iter().position(|a| *a == c)? as u32);
    }
    if pad > 2 || (vals.len() + pad) % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4);
    for chunk in vals.chunks(4) {
        let mut n = 0u32;
        for (i, v) in chunk.iter().enumerate() {
            n |= v << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// The bytes of base64 in either alphabet (`+/` or `-_`), padded or not (rulec's and koyomi's):
/// what a reply that is not quite standard still decodes as.
pub fn base64_decode_lenient(s: &str) -> Option<Vec<u8>> {
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
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        if matches!(c, b'=' | b'\n' | b'\r' | b' ') {
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

// ── Requests ────────────────────────────────────────────────────────────────

/// e-Gov law API v2.
pub const EGOV: &str = "https://laws.e-gov.go.jp/api/2";
/// The eCFR's versioner API.
pub const ECFR: &str = "https://www.ecfr.gov/api/versioner/v1";

/// A base URL from an environment variable (a test points it at a server of its own), else
/// `default`; no `/` at the end.
pub fn base_url(var: &str, default: &str) -> String {
    std::env::var(var).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| default.to_string()).trim_end_matches('/').to_string()
}

/// Whether a text is a date written `YYYY-MM-DD`, as the APIs write one.
fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// One run of curl: the body, what curl said when it failed, or that curl cannot be run.
/// `--compressed` is not an optimisation: the eCFR answers 406 to a request that does not say
/// it can take a compressed reply (rulec's).
fn curl_once(url: &str) -> Result<Vec<u8>, Result<String, Text>> {
    let out = match Command::new("curl").args(["-fsSL", "--compressed", "--max-time", "120", url]).output() {
        Ok(o) => o,
        Err(e) => return Err(Err(tr!("curl を走らせられません（PATH にありますか）: {e}", "cannot run curl (is it on the PATH?): {e}"))),
    };
    if out.status.success() { Ok(out.stdout) } else { Err(Ok(String::from_utf8_lossy(&out.stderr).trim().to_string())) }
}

/// One request: three tries over HTTP and HTTPS, two seconds before the second and four
/// before the third (a busy server refuses now and then, and a scheduled job that stops on one
/// refusal is one nobody reads), and one for a `file://` URL.
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
    Err(tr!("{url} を取れません（{tries} 回試しました）: {last}", "cannot fetch {url} (tried {tries} times): {last}"))
}

/// One request whose reply is JSON.
pub fn curl_json(url: &str) -> Result<Json, Text> {
    let body = curl(url)?;
    json::parse(&String::from_utf8_lossy(&body)).map_err(|e| {
        let m = e.message;
        tr!("{url} のレスポンスが JSON として読めません: {}", "the response of {url} is not JSON: {}", m.ja; m.en)
    })
}

/// One request to GitHub's API, with the headers it asks for. A token in `GITHUB_TOKEN` or
/// `GH_TOKEN` is passed on when there is one: without it the rate limit is sixty requests an
/// hour, which a scheduled job reaches (rulec's).
pub fn github_json(url: &str) -> Result<Json, Text> {
    let mut args: Vec<String> = ["-fsSL", "--max-time", "120", "-H", "Accept: application/vnd.github+json"].iter().map(|s| s.to_string()).collect();
    if let Some(t) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")).ok().filter(|t| !t.is_empty()) {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {t}"));
    }
    args.push(url.to_string());
    let out = Command::new("curl").args(&args).output().map_err(|e| tr!("curl を走らせられません（PATH にありますか）: {e}", "cannot run curl (is it on the PATH?): {e}"))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(tr!("{url} に問い合わせられません: {said}", "cannot query {url}: {said}"));
    }
    json::parse(&String::from_utf8_lossy(&out.stdout)).map_err(|e| {
        let m = e.message;
        tr!("{url} のレスポンスが JSON として読めません: {}", "the response of {url} is not JSON: {}", m.ja; m.en)
    })
}

/// A GitHub raw URL, split into owner, repository, revision and path; Some only when the
/// revision is a commit. A URL that names a branch answers with whatever is on the branch today,
/// so it is not a point in time.
pub fn github_raw(url: &str) -> Option<(String, String, String, String)> {
    let rest = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut it = rest.splitn(4, '/');
    let (owner, repo, rev, path) = (it.next()?, it.next()?, it.next()?, it.next()?);
    let commit = (7..=40).contains(&rev.len()) && rev.chars().all(|c| c.is_ascii_hexdigit());
    if !commit || owner.is_empty() || repo.is_empty() || path.is_empty() {
        return None;
    }
    Some((owner.into(), repo.into(), rev.into(), path.into()))
}

/// Whether a raw URL names a branch rather than a commit: the same URL answers differently next
/// year, so the copy cannot be brought back.
pub fn raw_on_a_branch(url: &str) -> bool {
    url.starts_with("https://raw.githubusercontent.com/") && github_raw(url).is_none()
}

/// A path for a query, percent-encoded but for the unreserved characters and `/`.
pub fn urlq(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// The positions of supplementary provisions already looked up, by `<id>@<asof>`: one read of
/// the whole law per date, kept for a command's duration (14 MB for the special taxation
/// measures law, so not once a fragment).
pub type SupplCache = BTreeMap<String, Vec<Option<String>>>;

/// e-Gov law API v2 at a base URL.
#[derive(Clone, Debug)]
pub struct Egov {
    pub base: String,
}

impl Egov {
    /// `law_data` of a law as of a date: the whole law, or one element of it.
    pub fn law_data_url(&self, id: &str, asof: &str, elm: Option<&str>) -> String {
        match elm {
            Some(e) => format!("{}/law_data/{id}?asof={asof}&elm={}&law_full_text_format=xml", self.base, e.replace('[', "%5B").replace(']', "%5D")),
            None => format!("{}/law_data/{id}?asof={asof}&law_full_text_format=xml", self.base),
        }
    }

    /// One reply as JSON. A busy e-Gov answers a request it accepted with a page that is not
    /// JSON; that is tried again, with a pause, before it is an error (rulec's).
    pub fn json(&self, url: &str) -> Result<Json, Text> {
        let mut last = Text::default();
        for attempt in 0..3u64 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
            }
            let body = curl(url)?;
            match json::parse(&String::from_utf8_lossy(&body)) {
                Ok(j) => return Ok(j),
                Err(e) => {
                    let m = e.message;
                    last = tr!("{url} のレスポンスが JSON として読めません: {}", "the response of {url} is not JSON: {}", m.ja; m.en);
                }
            }
            if !url.starts_with("http") {
                break;
            }
        }
        Err(last)
    }

    /// The XML inside a `law_data` reply, and the revision it came from.
    pub fn law_xml(j: &Json, url: &str) -> Result<(Vec<u8>, String), Text> {
        let b64 = j.get("law_full_text").and_then(|x| x.as_str()).ok_or_else(|| tr!("{url} のレスポンスに law_full_text がありません", "the response of {url} has no law_full_text"))?;
        let xml = base64_decode(b64).ok_or_else(|| tr!("{url} の law_full_text が base64 として読めません", "the law_full_text of {url} is not base64"))?;
        let rev = j.get("revision_info").and_then(|r| r.get("law_revision_id")).and_then(|x| x.as_str()).unwrap_or("").to_string();
        Ok((xml, rev))
    }

    /// A law, or one element of it, as of a date: the XML and the revision.
    pub fn law_data(&self, id: &str, asof: &str, elm: Option<&str>) -> Result<(Vec<u8>, String), Text> {
        let url = self.law_data_url(id, asof, elm);
        Egov::law_xml(&self.json(&url)?, &url)
    }

    /// Where an amending law's supplementary provisions sit in the law as of a date: the `k` of
    /// `SupplProvision[k]`, counted from one.
    pub fn suppl_index(&self, id: &str, asof: &str, amend: &str, cache: &mut SupplCache) -> Result<usize, Text> {
        let key = format!("{id}@{asof}");
        if !cache.contains_key(&key) {
            let (xml, _) = self.law_data(id, asof, None)?;
            cache.insert(key.clone(), suppl_ordinals(&String::from_utf8_lossy(&xml)));
        }
        cache[&key]
            .iter()
            .position(|n| n.as_deref() == Some(amend))
            .map(|p| p + 1)
            .ok_or_else(|| tr!("法令 {id} の {asof} 時点に、附則（{amend}）がありません", "the law {id} as of {asof} has no supplementary provisions of {amend}"))
    }

    /// Where the supplementary provisions found at `k0` as of one date sit as of `date`. e-Gov's
    /// order shifts by a few as laws come in (rulec saw the 2025 income tax act's move from 349
    /// to 350 between two dates), so the positions around `k0` are tried first, a small request
    /// each, and the whole law is read only when none of them is it.
    pub fn suppl_index_near(&self, id: &str, date: &str, amend: &str, k0: usize, cache: &mut SupplCache) -> Result<usize, Text> {
        if !cache.contains_key(&format!("{id}@{date}")) {
            for delta in [0i64, 1, 2, -1, 3, -2, 4, -3] {
                let k = k0 as i64 + delta;
                if k < 1 {
                    continue;
                }
                let url = self.law_data_url(id, date, Some(&format!("SupplProvision[{k}]")));
                let Ok(body) = curl_once(&url) else { continue };
                let Ok(j) = json::parse(&String::from_utf8_lossy(&body)) else { continue };
                let Ok((xml, _)) = Egov::law_xml(&j, &url) else { continue };
                if suppl_ordinals(&String::from_utf8_lossy(&xml)).first().is_some_and(|n| n.as_deref() == Some(amend)) {
                    return Ok(k as usize);
                }
            }
        }
        self.suppl_index(id, date, amend, cache)
    }

    /// One element as of `asof`, its supplementary provisions' position (when `amend` says it
    /// has one, `[?]` in `elm`) looked up as of `index_asof` — the date the file reads the law
    /// at — and moved to `asof` from there when the two differ.
    pub fn element(&self, id: &str, asof: &str, index_asof: &str, elm: &str, amend: Option<&str>, cache: &mut SupplCache) -> Result<(Vec<u8>, String), Text> {
        let elm = match amend {
            Some(a) => {
                let k0 = self.suppl_index(id, index_asof, a, cache)?;
                let k = if asof == index_asof { k0 } else { self.suppl_index_near(id, asof, a, k0, cache)? };
                elm.replace("[?]", &format!("[{k}]"))
            }
            None => elm.to_string(),
        };
        self.law_data(id, asof, Some(&elm))
    }

    /// The revisions of a law that came into force after `asof`, as (the date, the revision
    /// id), one a day: several revisions can come into force on one day, and the text as of that
    /// day is one.
    pub fn later_revisions(&self, id: &str, asof: &str) -> Result<Vec<(String, String)>, Text> {
        let j = self.json(&format!("{}/law_revisions/{id}", self.base))?;
        let mut v = Vec::new();
        for r in j.get("revisions").and_then(|r| r.as_arr()).unwrap_or(&[]) {
            let date = r.get("amendment_enforcement_date").and_then(|x| x.as_str()).unwrap_or("");
            let rid = r.get("law_revision_id").and_then(|x| x.as_str()).unwrap_or("");
            if is_date(date) && date > asof {
                v.push((date.to_string(), rid.to_string()));
            }
        }
        v.sort();
        v.dedup_by(|a, b| a.0 == b.0);
        Ok(v)
    }
}

/// The title and the part of a CFR id: `29 CFR 1910` is title 29, part 1910.
pub fn cfr_id(id: &str) -> Result<(String, String), Text> {
    let words: Vec<&str> = id.split_whitespace().collect();
    match words.as_slice() {
        [t, c, part] if c.eq_ignore_ascii_case("cfr") && t.chars().all(|x| x.is_ascii_digit()) && part.chars().all(|x| x.is_ascii_alphanumeric()) => Ok((t.to_string(), part.to_string())),
        _ => Err(tr!("CFR の ID は、`29 CFR 1910` の形（title と part）で書いてください: `{id}`", "a CFR id is written `29 CFR 1910`, a title and a part: `{id}`")),
    }
}

/// The eCFR's versioner API at a base URL.
#[derive(Clone, Debug)]
pub struct Ecfr {
    pub base: String,
}

impl Ecfr {
    /// A section as of a date, as the eCFR serves it: already the fragment, with no envelope and
    /// no revision id (which version it is, is the date asked for).
    pub fn section(&self, id: &str, asof: &str, section: &str) -> Result<Vec<u8>, Text> {
        let (title, part) = cfr_id(id)?;
        curl(&format!("{}/full/{asof}/title-{title}.xml?part={part}&section={section}", self.base))
    }

    /// The days after `asof` on which a cited section was amended in substance, as (the date,
    /// the section), one a day. The eCFR says whether a version only moved the markup: a
    /// re-issue that changes no text is not an amendment, the same line e-Gov's revisions get.
    pub fn amendments(&self, id: &str, asof: &str, sections: &[String]) -> Result<Vec<(String, String)>, Text> {
        let (title, part) = cfr_id(id)?;
        let mut out: Vec<(String, String)> = Vec::new();
        for s in sections {
            let j = curl_json(&format!("{}/versions/title-{title}.json?part={part}&section={s}", self.base))?;
            for v in j.get("content_versions").and_then(|x| x.as_arr()).unwrap_or(&[]) {
                let date = v.get("amendment_date").and_then(|x| x.as_str()).unwrap_or("");
                let substantive = v.get("substantive").and_then(|x| x.as_bool()).unwrap_or(false);
                if substantive && is_date(date) && date > asof {
                    out.push((date.to_string(), s.clone()));
                }
            }
        }
        out.sort();
        out.dedup_by(|a, b| a.0 == b.0);
        Ok(out)
    }
}
