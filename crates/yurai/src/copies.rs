//! The copies of the sources (DESIGN 1.4): where a law's articles are kept beside the `.req`,
//! the file each article is copied into, and the text of a copy.
//!
//! The places and the names are rulec's and koyomi's, so that the three tools keep the same
//! copy of the same article at the same path: `sources/law/<id>@<asof>/<element>.xml`, the
//! element as e-Gov addresses it (`第143条第2項` → `MainProvision-Article_143-Paragraph_2`,
//! `別表第一` → `AppdxTable_1`, the supplementary provisions as rulec's §15.71 writes them),
//! and for the eCFR `sources/law/29-CFR-1910@<asof>/1910.157.xml`.

use crate::ast::LawDb;
use crate::i18n::Text;
use std::path::{Path, PathBuf};

/// A number as a law writes it: `百四十三`, `二十`, `千五十`, or ASCII digits (koyomi's).
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

/// The file an article of a law is copied into (without the directory), or None when the
/// article is not written in a form the database is read in.
pub fn fragment_file(db: LawDb, name: &str) -> Option<String> {
    match db {
        LawDb::Ecfr => {
            let s = name.strip_prefix('§').unwrap_or(name).trim();
            let (part, sec) = s.split_once('.')?;
            if part.is_empty() || sec.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) || !sec.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return None;
            }
            Some(format!("{s}.xml"))
        }
        LawDb::Egov => {
            if let Some(rest) = name.strip_prefix("別表第") {
                return Some(format!("AppdxTable_{}.xml", law_number(rest)?));
            }
            if let Some(rest) = name.strip_prefix("附則") {
                // `附則（令和七年三月三一日法律第一三号）第3条`: an amending law's, filed under its number.
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
                let head = match amend {
                    Some(n) => format!("SupplProvision_{n}"),
                    None => "SupplProvision".to_string(),
                };
                return Some(format!("{head}{suffix}.xml"));
            }
            Some(format!("MainProvision{}.xml", article_suffix(name)?))
        }
    }
}

/// The directory a law's copies as of a date are kept in, from the directory of the `.req`:
/// `sources/law/<id>@<asof>`, a CFR id's spaces made `-` (rulec's `copy_dir`).
pub fn copy_dir(id: &str, asof: &str) -> String {
    format!("sources/law/{}@{asof}", id.replace([' ', '/'], "-"))
}

/// What the forms of an article are, for a diagnostic.
pub fn fragment_shapes(db: LawDb) -> Text {
    match db {
        LawDb::Egov => tr!(
            "書けるのは `第20条`、`第20条の2`、`第20条第2項`、`第20条第2項第3号`、`別表第一`、`附則第3条`、`附則（令和七年三月三一日法律第一三号）第3条` の形です（漢数字でもよい。丸括弧を含むものは `\"…\"` で囲む）。",
            "The forms are `第20条`, `第20条の2`, `第20条第2項`, `第20条第2項第3号`, `別表第一`, `附則第3条` and `附則（令和七年三月三一日法律第一三号）第3条` (in kanji numerals too; quote one with parentheses)."
        ),
        LawDb::Ecfr => tr!(
            "書けるのは `\"§1910.157\"` か `\"1910.157\"` の形です（section の単位）。",
            "The forms are `\"§1910.157\"` and `\"1910.157\"` (a section)."
        ),
    }
}

/// The element a copy is expected to start with: `Article`, `Paragraph`, `Item`,
/// `AppdxTable`, `SupplProvision` for e-Gov, `DIV8` (a section) for the eCFR.
fn root_element(db: LawDb, file: &str) -> String {
    match db {
        LawDb::Ecfr => "DIV8".to_string(),
        LawDb::Egov => {
            let stem = file.trim_end_matches(".xml");
            let last = stem.rsplit('-').next().unwrap_or(stem);
            let name = last.split('_').next().unwrap_or(last);
            match name {
                "MainProvision" => "MainProvision".to_string(),
                n => n.to_string(),
            }
        }
    }
}

/// The name of the first element of an XML document, after its declaration and comments.
fn first_element(xml: &str) -> Option<&str> {
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

/// Whether a copy is what e-Gov or the eCFR serves for the article (E104): UTF-8 XML whose
/// first element is the one the article is addressed by, with some text in it.
pub fn readable(db: LawDb, file: &str, bytes: &[u8]) -> Result<(), Text> {
    let Ok(xml) = std::str::from_utf8(bytes) else {
        return Err(tr!("UTF-8 として読めません", "it is not UTF-8"));
    };
    let want = root_element(db, file);
    match first_element(xml) {
        None => Err(tr!("XML ではありません", "it is not XML")),
        Some(got) if got != want => {
            let db = db.title();
            Err(tr!("{db} の写しなら最初の要素は <{want}> ですが、<{got}> です", "a copy from {db} starts with <{want}>, and this one starts with <{got}>"))
        }
        Some(_) if xml_text(xml).is_empty() => Err(tr!("本文がありません", "it has no text")),
        Some(_) => Ok(()),
    }
}

/// The text of a law's XML, with the tags removed: a line for each paragraph, item, sentence
/// and table row (rulec's `xml_text`). What `check` diffs when an article changed, and what
/// two copies are compared by.
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

/// U+3000, the space a Japanese law puts after an article's or a paragraph's number.
const WIDE_SPACE: char = '　';

/// An article's XML as the law prints it (koyomi's `article_lines`): the caption on a line of
/// its own, the article's number and its first paragraph on the next, and a line for every
/// other paragraph, item and subitem. Ruby readings are left out.
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

/// The lines `trace` quotes a copy by: an e-Gov article as the law prints it, anything else
/// (a table, a section of the CFR) a line for each line of its text.
pub fn quote_lines(db: LawDb, file: &str, xml: &str) -> Vec<String> {
    if db == LawDb::Egov && (file.starts_with("MainProvision") || file.contains("-Article_")) {
        article_lines(xml)
    } else {
        xml_text(xml).lines().map(|s| s.to_string()).collect()
    }
}

/// `revision.txt` beside the copies, when there is one: the version of the law e-Gov served.
pub fn revision(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("revision.txt")).ok().map(|r| r.trim().to_string()).filter(|r| !r.is_empty())
}

/// The full path of a copy.
pub fn copy_path(req_dir: &Path, id: &str, asof: &str, file: &str) -> PathBuf {
    req_dir.join(copy_dir(id, asof)).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_articles_are_copied_into() {
        let e = |s: &str| fragment_file(LawDb::Egov, s);
        assert_eq!(e("第140条").as_deref(), Some("MainProvision-Article_140.xml"));
        assert_eq!(e("第百四十三条第二項").as_deref(), Some("MainProvision-Article_143-Paragraph_2.xml"));
        assert_eq!(e("第20条の2第3項第4号").as_deref(), Some("MainProvision-Article_20_2-Paragraph_3-Item_4.xml"));
        assert_eq!(e("別表第一").as_deref(), Some("AppdxTable_1.xml"));
        assert_eq!(e("附則第3条").as_deref(), Some("SupplProvision-Article_3.xml"));
        assert_eq!(e("附則").as_deref(), Some("SupplProvision.xml"));
        assert_eq!(e("附則（令和七年三月三一日法律第一三号）第3条第2項").as_deref(), Some("SupplProvision_令和七年三月三一日法律第一三号-Article_3-Paragraph_2.xml"));
        assert_eq!(e("140条"), None);
        assert_eq!(fragment_file(LawDb::Ecfr, "§1910.157").as_deref(), Some("1910.157.xml"));
        assert_eq!(fragment_file(LawDb::Ecfr, "1910"), None);
        assert_eq!(copy_dir("29 CFR 1910", "2026-01-01"), "sources/law/29-CFR-1910@2026-01-01");
        assert_eq!(root_element(LawDb::Egov, "MainProvision-Article_143-Paragraph_2.xml"), "Paragraph");
        assert_eq!(root_element(LawDb::Egov, "AppdxTable_1.xml"), "AppdxTable");
        assert_eq!(root_element(LawDb::Egov, "SupplProvision.xml"), "SupplProvision");
        assert_eq!(root_element(LawDb::Egov, "MainProvision-Article_140.xml"), "Article");
    }
}
