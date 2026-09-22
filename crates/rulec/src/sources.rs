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

use crate::ast::{Cite, Item, LawDb, RuleFile, SourceDecl, SourceKind};
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

/// The fragment a citation names, read the way the database it comes from writes one.
pub fn fragment(db: LawDb, name: &str) -> Option<Fragment> {
    match db {
        LawDb::Egov => egov_fragment(name),
        LawDb::Ecfr => ecfr_fragment(name),
    }
}

/// A section of the Code of Federal Regulations: `§1910.157`, or the same without the sign.
///
/// The part is in the source's own id (`29 CFR 1910`), as a law id is, so what a citation
/// adds is the section — the unit the eCFR API serves and the unit an amendment is recorded
/// against. A paragraph of one (`(d)(2)`) is not addressed yet: the API has no way to ask for
/// one, so it would mean cutting the copy up here, and a copy that this program cut is worth
/// less as evidence than one the government served.
fn ecfr_fragment(name: &str) -> Option<Fragment> {
    let s = name.strip_prefix('§').unwrap_or(name).trim();
    let (part, sec) = s.split_once('.')?;
    if part.is_empty() || sec.is_empty() {
        return None;
    }
    if !part.chars().all(|c| c.is_ascii_digit()) || !sec.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(Fragment { name: name.to_string(), elm: s.to_string(), amend: None })
}

/// A fragment of a law on e-Gov. Articles, paragraphs and items of the main provisions;
/// appendix tables by their ordinal; and the supplementary provisions (§15.71): the law's
/// own as `附則第3条`, an amending law's as `附則（令和七年三月三一日法律第一三号）第3条` with
/// the number spelled as the law's own heading spells it, either also whole without the
/// article. Sub-items still wait for a rule that needs them.
fn egov_fragment(name: &str) -> Option<Fragment> {
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

/// What a citation of this database names, for the diagnostics that have to say so.
fn cite_shape(db: LawDb, name: &str) -> String {
    match db {
        LawDb::Egov => tr!(
            "法令は条や別表の単位で写すので、`@{name} 第20条` のように、どこを引いたかを書いてください。",
            "A law is copied an article at a time, so say which one: `@{name} 第20条`."
        ),
        LawDb::Ecfr => tr!(
            "CFR は section の単位で写すので、`@{name} §1910.157` のように、どこを引いたかを書いてください。",
            "The CFR is copied a section at a time, so say which one: `@{name} §1910.157`."
        ),
    }
}

/// The forms a fragment of this database may be written in.
fn fragment_shapes(db: LawDb) -> String {
    match db {
        LawDb::Egov => tr!(
            "書けるのは `第20条`、`第20条の2`、`第20条第2項`、`第20条第2項第3号`、`別表第一`、`附則第3条`、`附則（令和七年三月三一日法律第一三号）第3条` の形です。号の細分はまだ受けません。",
            "The forms are `第20条`, `第20条の2`, `第20条第2項`, `第20条第2項第3号`, `別表第一`, `附則第3条` and `附則（令和七年三月三一日法律第一三号）第3条`. Sub-items are not read yet."
        ),
        LawDb::Ecfr => tr!(
            "書けるのは `§1910.157` か `1910.157` の形です。項（`(d)(2)`）はまだ受けません — eCFR が section の単位でしか渡さないので、写しをこちらで切ることになるからです。",
            "The forms are `§1910.157` and `1910.157`. A paragraph of one (`(d)(2)`) is not read yet: the eCFR API serves a section at a time, so it would mean cutting the copy up here."
        ),
    }
}

/// The name of the database, as a message calls it.
fn db_name(db: LawDb) -> &'static str {
    match db {
        LawDb::Egov => "e-Gov",
        LawDb::Ecfr => "eCFR",
    }
}

/// The directory the copies of a law as of a date are kept in, beside the rule.
pub fn copy_dir(rule_path: &str, id: &str, asof: &str) -> PathBuf {
    // A CFR id is a citation with spaces in it (`29 CFR 1910`); an e-Gov id has none, so
    // this leaves those where they have always been.
    let id = id.replace([' ', '/'], "-");
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
            Item::Agg(_) => {}
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
    // A fragment a name cannot hold is quoted, as it is in the citation that names it. The
    // question is the lexer's, so it is asked there: `附則（…）第3条` is one word to it and
    // `§1910.157` is not.
    match crate::lex::is_bare_word(fragment) {
        true => format!("  {fragment} sha256:{hash}"),
        false => format!("  \"{fragment}\" sha256:{hash}"),
    }
}

/// The `source` line of a file source with its digest.
pub fn file_line(d: &SourceDecl, hash: &str) -> String {
    let SourceKind::File { path, url, .. } = &d.kind else { return String::new() };
    let name = match &d.name.ascii {
        Some(a) => format!("{}({a})", d.name.text),
        None => d.name.text.clone(),
    };
    let u = url.as_ref().map(|u| format!(" {} \"{u}\"", crate::kw::URL)).unwrap_or_default();
    format!("{} {name} = {} \"{path}\"{u} sha256:{hash}", crate::kw::SOURCE, crate::kw::FILE)
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
            SourceKind::File { path, hash, .. } => {
                let p = dir.join(path);
                let cited = cited_from(&cites, name);
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
                // The fragments of a document are held to their copies as a law's are
                // (§15.82): the document itself is pinned above, each cited table beside it.
                let cdir = crate::extract::copy_dir(&p);
                for (frag, whos) in &cited {
                    // A document may be cited whole — a law is told it cannot be.
                    if frag.is_empty() {
                        continue;
                    }
                    if crate::extract::fragment(frag).is_none() {
                        out.push(
                            Diag::error("E037", tr!("引用箇所 `{frag}` の書き方が読めません", "The fragment `{frag}` cannot be read"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("文書から引けるのは表なので、引用箇所は `表3`（文書順に三つめの表）か `table3` と書きます。見出しで指す書き方はまだ受けません。", "A document's fragments are its tables: write `表3` (the third table in document order) or `table3`. Naming a heading is not read yet."))
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos))),
                        );
                        continue;
                    }
                    let fp = cdir.join(crate::extract::fragment_file(frag));
                    let Ok(fb) = std::fs::read(&fp) else {
                        // Which of the two it is, the extension already says: a format with no
                        // reader here will never have a copy, and the rule should cite the
                        // document whole until there is an extractor to plug in (§15.82).
                        let how = match crate::extract::unreadable(&p) {
                            Some(why) => tr!(
                                "{why}。抽出器を繋げるまでは、この文書は `@{name}` と丸ごと引いてください。",
                                "{why}. Until an extractor can be plugged in, cite this document whole: `@{name}`."
                            ),
                            None => tr!(
                                "`rulec source fetch {rule_path}` が文書から取り出して隣に置きます。check は文書の中身までは見ません。",
                                "`rulec source fetch {rule_path}` takes it out of the document and puts it beside it. check does not read the document itself."
                            ),
                        };
                        out.push(
                            Diag::error("E039", tr!("出典 `{name}` の `{frag}` の写しがありません", "There is no copy of fragment `{frag}` of source `{name}`"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("探した先: {}", "Looked for: {}", fp.display()))
                                .note(how)
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos))),
                        );
                        continue;
                    };
                    let fh = crate::sha256::short(&fb);
                    match d.pins.iter().find(|p| p.fragment == *frag) {
                        None => out.push(
                            Diag::error("E037", tr!("出典 `{name}` の `{frag}` のハッシュが固定されていません", "Fragment `{frag}` of source `{name}` is not pinned"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos)))
                                .note(tr!("写しのハッシュは sha256:{fh} です。この内容で承認するなら、`{}` の行の下に次の行を足してください（`rulec source pin` でも書けます）。", "The copy's digest is sha256:{fh}. To pin it as the one approved, add the following line under the `{}` line (`rulec source pin` writes it too).", crate::kw::SOURCE))
                                .fix(crate::diag::FixKind::PinSource, pin_line(frag, &fh)),
                        ),
                        Some(pin) if pin.hash != fh => out.push(
                            Diag::error("E038", tr!("出典 `{name}` の `{frag}` が変わっています", "Fragment `{frag}` of source `{name}` has changed"))
                                .at(at(pin.span.line, name))
                                .mark(pin.span.clone(), tr!("固定: sha256:{}", "pinned: sha256:{}", pin.hash))
                                .note(tr!("いまの写し: sha256:{fh}", "The copy now: sha256:{fh}"))
                                .note(tr!("読み直す定義: {}", "Definitions to reread: {}", whos_text(whos)))
                                .note(tr!("写しの差分（{}）を読み、写した行がまだ正しければ、この行を次のとおり書き換えて固定し直してください。", "Read the copy's diff ({}), and if what was transcribed still holds, rewrite this line as follows to pin the new copy.", fp.display()))
                                .fix(crate::diag::FixKind::PinSource, pin_line(frag, &fh)),
                        ),
                        _ => {}
                    }
                }
                unused_pins(d, &cited, name, rule_path, &mut out);
            }
            SourceKind::Law { db, id, asof } => {
                let db = *db;
                let cdir = copy_dir(rule_path, id, asof);
                let cited = cited_from(&cites, name);
                for (frag, whos) in &cited {
                    if frag.is_empty() {
                        out.push(
                            Diag::error("E037", tr!("法令 `{name}` の引用に箇所がありません", "A citation of the law `{name}` names no article"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(cite_shape(db, name))
                                .note(tr!("引いている: {}", "Cited by: {}", whos_text(whos))),
                        );
                        continue;
                    }
                    let Some(fr) = fragment(db, frag) else {
                        out.push(
                            Diag::error("E037", tr!("引用箇所 `{frag}` の書き方が読めません", "The fragment `{frag}` cannot be read"))
                                .at(at(d.span.line, name))
                                .mark(d.span.clone(), "")
                                .note(fragment_shapes(db))
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
                                .note(tr!(
                                    "`rulec source fetch {rule_path}` が {d} から取ってきて写しに置きます。check は通信しません。",
                                    "`rulec source fetch {rule_path}` fetches it from {d} into the copies. check never reads the network.",
                                    d = db_name(db)
                                ))
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
                unused_pins(d, &cited, name, rule_path, &mut out);
            }
        }
    }
    out.extend(transcription(f, rule_path));
    out
}

/// E116 and W120 (§15.82): the rows against the table they say they were transcribed from.
///
/// Two directions, read with different leniency on purpose. **An amount a row writes has to
/// be somewhere in the copy** — looked for in every number the copy shows anywhere, so that a
/// value the document does show is never called missing. **A number the copy states as a
/// whole cell has to be accounted for by the table** — by a value some row writes, or by a
/// comparison some cell makes (a table that merges `1kg`, `2kg` and `3kg` into `<=3kg` has
/// transcribed all three) — and only cells that are nothing but a number are asked about, so
/// that `2026年4月1日改定` is not read as an amount somebody forgot.
///
/// E119 (§15.124) is the third direction and the one that reaches a condition. **A threshold
/// is rewritten as it is transcribed** — `1,949,000円まで` becomes `<=1949000円` — so the two
/// cannot be compared as text. What survives the rewriting is **which of the two bands the
/// boundary value itself falls in**, and that is the half of a threshold that moves money.
fn transcription(f: &RuleFile, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let mut copies: BTreeMap<(String, String), Option<Vec<Vec<String>>>> = BTreeMap::new();
    // The copy of one cited table, read once however many rows cite it.
    let mut copy = |src: &str, frag: &str| -> Option<Vec<Vec<String>>> {
        let key = (src.to_string(), frag.to_string());
        if let Some(g) = copies.get(&key) {
            return g.clone();
        }
        let g = (|| {
            let d = f.sources.iter().find(|d| d.name.text == src)?;
            // A source that came in with an applied rule was held to its copies there.
            if d.base.is_some() {
                return None;
            }
            fragment_grid(rule_path, d, frag)
        })();
        copies.insert(key, g.clone());
        g
    };
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        if t.applied.is_some() {
            continue;
        }
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let word = if t.clause { tr!("節", "clause") } else { tr!("表", "table") };
        for r in &t.rows {
            let cite = r.cite.as_ref().or(t.cite.as_ref());
            let Some(c) = cite else { continue };
            let mut grids = Vec::new();
            for frag in &c.fragments {
                if let Some(g) = copy(&c.source, frag) {
                    grids.push((frag.clone(), g));
                }
            }
            if grids.is_empty() {
                continue;
            }
            let shown: Vec<_> = grids.iter().flat_map(|(_, g)| crate::extract::shown(g)).collect();
            let missing: Vec<(String, Span)> = amounts(r)
                .into_iter()
                .filter(|(_, v, _)| !shown.iter().any(|s| crate::types::same_value(s, v)))
                .map(|(text, _, span)| (text, span))
                .collect();
            let frags = grids.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(sep());
            let rn = match &r.label {
                Some(l) => tr!("行{}（{}）", "row {} ({})", r.index, l.text),
                None => tr!("行{}", "row {}", r.index),
            };
            boundaries(r, &grids, &rn, &frags, &c.source, rule_path, &word, &name, &mut out);
            if missing.is_empty() {
                continue;
            }
            let values = missing.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join(sep());
            out.push(
                Diag::error("E116", tr!("{rn} の金額が、引いた写しにありません", "The amount of {rn} is not in the copy it cites"))
                    .at(tr!("{rule_path}:{} {word} {name} {rn}", "{rule_path}:{} {word} {name} {rn}", r.span.line))
                    .table(name.clone())
                    .row(r.index)
                    .mark(missing[0].1.clone(), tr!("写しに無い: {values}", "not in the copy: {values}"))
                    .note(tr!("引いた写し: {} {frags}", "The copy cited: {} {frags}", c.source))
                    .note(tr!(
                        "金額は写すときに書き換わらないので、これは写し間違いか、その値が別のところから来たかのどちらかです。別のところから来たのなら、この行の引用を外し、どこから来たかを行末のコメントに書いてください。",
                        "An amount is not rewritten as it is transcribed, so either it was mistyped or it came from somewhere else. If it came from somewhere else, take the citation off this row and say in a comment at the end of it where the value came from."
                    )),
            );
        }
        // The other direction, and only for a table that says the whole fragment is what it
        // transcribes: a row that cites on its own says nothing about the rest of the table.
        let Some(c) = t.cite.as_ref() else { continue };
        for frag in &c.fragments {
            let Some(g) = copy(&c.source, frag) else { continue };
            let unused: Vec<String> = crate::extract::stated(&g)
                .into_iter()
                .filter(|(_, v)| !accounted(t, v))
                .map(|(text, _)| text)
                .collect();
            if unused.is_empty() {
                continue;
            }
            let mut names: Vec<String> = Vec::new();
            for u in &unused {
                if !names.contains(u) {
                    names.push(u.clone());
                }
            }
            let shown_n = names.len().min(8);
            let more = names.len() - shown_n;
            let list = names[..shown_n].join(sep())
                + &if more > 0 { tr!("（あと {more} 個）", " ({more} more)") } else { String::new() };
            out.push(
                Diag::warning("W120", tr!("写しの {frag} の値を、どの行も使っていません", "The copy of {frag} states values no row uses"))
                    .at(tr!("{rule_path}:{} {word} {name}", "{rule_path}:{} {word} {name}", t.span.line))
                    .table(name.clone())
                    .mark(c.span.clone(), "")
                    .note(tr!("どの行にも出てこない値: {list}", "Stated in the copy, used by no row: {list}"))
                    .note(tr!(
                        "行を落としていないか確かめてください。{frag} のうち一部だけを写したのなら、引用を `table` の行から、写した行それぞれの末尾へ移すと、残りは問われなくなります。",
                        "Check that no row was left out. If this table transcribes only part of {frag}, moving the citation from the table onto the rows that came from it leaves the rest unasked."
                    )),
            );
        }
    }
    out
}

/// E119 (§15.124): the boundaries of one row, against the copy the row cites.
///
/// What is compared is not the operator but **the side** — which of the two bands the
/// boundary value itself falls in — because a rule may write one boundary from either end
/// (`<=60cm` and `>60cm` are its two halves) and both say the same thing about 60. A number
/// the copy bounds one way here and the other way there is left alone, and so is one the
/// copy states with no bounding word at all: `18 to 20` names the numbers that bound a band
/// without saying which band holds them.
#[allow(clippy::too_many_arguments)]
fn boundaries(
    r: &crate::ast::Row,
    grids: &[(String, Vec<Vec<String>>)],
    rn: &str,
    frags: &str,
    src: &str,
    rule_path: &str,
    word: &str,
    table: &str,
    out: &mut Vec<Diag>,
) {
    use crate::ast::{Cell, CmpOp, Lit};
    use crate::extract::Side;
    let marks: Vec<crate::extract::Bound> = grids.iter().flat_map(|(_, g)| crate::extract::bounds(g)).collect();
    if marks.is_empty() {
        return;
    }
    let sides = |s: Side| match s {
        Side::Lower => tr!("小さいほう", "the smaller side"),
        Side::Upper => tr!("大きいほう", "the larger side"),
    };
    // A bound read from a heading is quoted as one, because the cell the reader has to look
    // at is not the cell the number is in.
    let quote = |b: &crate::extract::Bound| match b.column {
        true => tr!("「{}」の欄", "the \u{201c}{}\u{201d} column", b.cell),
        false => tr!("「{}」", "\u{201c}{}\u{201d}", b.cell),
    };
    for (k, cell) in r.cells.iter().enumerate() {
        let Cell::Cmp(ops) = cell else { continue };
        for (j, (op, lit)) in ops.iter().enumerate() {
            let Lit::Num(n) = lit else { continue };
            let Some(v) = crate::types::comparable(n) else { continue };
            let said: Vec<&crate::extract::Bound> =
                marks.iter().filter(|b| crate::types::same_value(&b.value, &v)).collect();
            let Some(first) = said.first() else { continue };
            // A copy that puts the same number on one side here and the other side there is
            // not read at all: two tables of one fragment, or a number that means two things.
            if said.iter().any(|b| b.side != first.side) {
                continue;
            }
            let mine = match op {
                CmpOp::Le | CmpOp::Gt => Side::Lower,
                CmpOp::Lt | CmpOp::Ge => Side::Upper,
            };
            if mine == first.side {
                continue;
            }
            // The same cell with this one boundary's strictness toggled: the direction is the
            // table's geometry and stays, the side is what the copy decides.
            let fixed: String = ops
                .iter()
                .enumerate()
                .map(|(i, (o, l))| {
                    let o = if i != j {
                        *o
                    } else {
                        match o {
                            CmpOp::Le => CmpOp::Lt,
                            CmpOp::Lt => CmpOp::Le,
                            CmpOp::Ge => CmpOp::Gt,
                            CmpOp::Gt => CmpOp::Ge,
                        }
                    };
                    format!("{}{}", o.word(), lit_text(l))
                })
                .collect::<Vec<_>>()
                .join(" ");
            let here = format!("{}{}", op.word(), n.raw);
            let span = r.cell_spans.get(k).cloned().unwrap_or_else(|| r.span.clone());
            out.push(
                Diag::error("E119", tr!("{rn} の境界が、引いた写しと反対側です", "The boundary of {rn} falls on the other side from the copy it cites"))
                    .at(tr!("{rule_path}:{} {word} {table} {rn}", "{rule_path}:{} {word} {table} {rn}", r.span.line))
                    .table(table.to_string())
                    .row(r.index)
                    .mark(span, tr!("写し: {}", "the copy: {}", quote(first)))
                    .note(tr!("引いた写し: {src} {frags}", "The copy cited: {src} {frags}"))
                    .note(tr!(
                        "ちょうど {} のとき、写しの{}は{}に入れ、`{here}` は{}に入れます。変わるのはこの一点だけです。",
                        "At exactly {}, the copy's {} takes it with {}, and `{here}` takes it with {}. That one point is the whole of the difference.",
                        n.raw,
                        quote(first),
                        sides(first.side),
                        sides(mine)
                    ))
                    .note(tr!(
                        "閾値は写すときに書き換わる（`60cmまで` は `<=60cm` になる）ので、比べているのは境界の値がどちらに入るかだけです。写し間違いなら向きを直してください。境界が別のところ（後の通知、本文の但し書き）から来たのなら、この行の引用を外し、どこから来たかを行末のコメントに書いてください。",
                        "A threshold is rewritten as it is transcribed (`60cmまで` becomes `<=60cm`), so the one thing held to the copy here is which side the boundary value falls on. If it was mistyped, correct it. If the boundary came from somewhere else — a later notice, a proviso in the text — take the citation off this row and say in a comment at the end of it where it came from."
                    ))
                    .fix(crate::diag::FixKind::FlipBound, fixed),
            );
        }
    }
}

/// A literal as it was written, for a cell rebuilt in a fix.
fn lit_text(l: &crate::ast::Lit) -> String {
    match l {
        crate::ast::Lit::Num(n) => n.raw.clone(),
        crate::ast::Lit::Word(w) => w.clone(),
        crate::ast::Lit::Str(s) => format!("\"{s}\""),
        crate::ast::Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

/// The amounts a row writes: the literals of its output cells, with the text as written and
/// where it stands. A cell holding a name carries no value of its own.
fn amounts(r: &crate::ast::Row) -> Vec<(String, (Option<String>, crate::num::Rat), Span)> {
    let mut out = Vec::new();
    for (k, o) in r.outs.iter().enumerate() {
        let crate::ast::OutCell::Lit(crate::ast::Lit::Num(n)) = o else { continue };
        if let Some(v) = crate::types::comparable(n) {
            let span = r.out_spans.get(k).cloned().unwrap_or_else(|| r.span.clone());
            out.push((n.raw.clone(), v, span));
        }
    }
    out
}

/// Whether a table accounts for a value the copy states: some cell writes it, or some cell's
/// comparison takes it in.
fn accounted(t: &crate::ast::Table, v: &(Option<String>, crate::num::Rat)) -> bool {
    use crate::ast::{Cell, Lit, OutCell};
    let num = |l: &Lit| match l {
        Lit::Num(n) => crate::types::comparable(n),
        _ => None,
    };
    for r in &t.rows {
        for o in &r.outs {
            if let OutCell::Lit(l) = o {
                if num(l).is_some_and(|w| crate::types::same_value(&w, v)) {
                    return true;
                }
            }
        }
        for (k, c) in r.cells.iter().enumerate() {
            // A cell that takes the whole column accounts for every value the column counts,
            // and what it counts is what its own cells say — the only place the units of a
            // column are visible without the types (§15.82).
            let whole = || column_dim(t, k).is_some_and(|dim| v.0.is_none() || dim == v.0);
            let inside = match c {
                Cell::Lit(l) => num(l).is_some_and(|w| crate::types::same_value(&w, v)),
                Cell::Set(ls) => ls.iter().any(|l| num(l).is_some_and(|w| crate::types::same_value(&w, v))),
                // `-` covers the column; `not: 3kg` covers it but for what it names, and a
                // value it names is one the table wrote down all the same.
                Cell::DontCare | Cell::Not(_) => whole(),
                Cell::Cmp(ops) => {
                    let mut all = true;
                    let mut any = false;
                    for (op, l) in ops {
                        let Some(w) = num(l) else { continue };
                        // A bound in another dimension says nothing about this value.
                        if !(w.0.is_none() || v.0.is_none() || w.0 == v.0) {
                            all = false;
                            break;
                        }
                        any = true;
                        let c = v.1.cmp_to(w.1);
                        let ok = match op {
                            crate::ast::CmpOp::Le => c.is_le(),
                            crate::ast::CmpOp::Lt => c.is_lt(),
                            crate::ast::CmpOp::Ge => c.is_ge(),
                            crate::ast::CmpOp::Gt => c.is_gt(),
                        };
                        if !ok {
                            all = false;
                            break;
                        }
                    }
                    any && all
                }
                _ => false,
            };
            if inside {
                return true;
            }
        }
    }
    false
}

/// The dimension a column of a table is written in, as its own cells show it: the first
/// number any row writes there. A column whose cells hold no number has none, and a `-` in it
/// accounts for no amount.
fn column_dim(t: &crate::ast::Table, k: usize) -> Option<Option<String>> {
    use crate::ast::{Cell, Lit};
    for r in &t.rows {
        let lits: Vec<&Lit> = match r.cells.get(k) {
            Some(Cell::Lit(l)) => vec![l],
            Some(Cell::Set(ls)) | Some(Cell::Not(ls)) => ls.iter().collect(),
            Some(Cell::Cmp(ops)) => ops.iter().map(|(_, l)| l).collect(),
            _ => continue,
        };
        for l in lits {
            if let Lit::Num(n) = l {
                if let Some((dim, _)) = crate::types::comparable(n) {
                    return Some(dim);
                }
            }
        }
    }
    None
}

fn sep() -> &'static str {
    if crate::i18n::ja() { "、" } else { ", " }
}

/// W119: a pin whose citation is gone. The same for a law's articles and a document's tables.
fn unused_pins(d: &SourceDecl, cited: &[(String, Vec<&str>)], name: &str, rule_path: &str, out: &mut Vec<Diag>) {
    let at = |line: usize| tr!("{rule_path}:{line} 出典 {name}", "{rule_path}:{line} source {name}");
    for pin in &d.pins {
        if !cited.iter().any(|(frag, _)| *frag == pin.fragment) {
            out.push(
                Diag::warning("W119", tr!("出典 `{name}` の `{}` はハッシュが固定されていますが、引用されていません", "Fragment `{}` of source `{name}` is pinned but not cited", pin.fragment))
                    .at(at(pin.span.line))
                    .mark(pin.span.clone(), "")
                    .note(tr!("引用を消したあとの残りです。`rulec source pin` が消します。", "It is what remains after a citation was removed. `rulec source pin` removes it.")),
            );
        }
    }
}

/// A fragment's copy as the approver's page quotes it: an article as its text, the XML with
/// its tags removed and one line per paragraph, item or table row; a document's table as a
/// Markdown table (§15.82).
pub fn fragment_text(rule_path: &str, d: &SourceDecl, frag: &str) -> Option<String> {
    // A source inherited through an `apply` keeps its copies beside the rule it came from.
    let base = d.base.as_deref().unwrap_or(rule_path);
    match &d.kind {
        SourceKind::Law { db, id, asof } => {
            let fr = fragment(*db, frag)?;
            let xml = std::fs::read_to_string(copy_dir(base, id, asof).join(fr.file())).ok()?;
            Some(xml_text(&xml))
        }
        // A document's fragment is a table, and it is quoted as one (§15.82).
        SourceKind::File { .. } => Some(crate::extract::markdown(&fragment_grid(rule_path, d, frag)?)),
    }
}

/// The copy of one fragment of a document, as the grid it is. `None` for a law, whose
/// fragments are articles and not tables, and for a copy that has not been fetched.
pub fn fragment_grid(rule_path: &str, d: &SourceDecl, frag: &str) -> Option<Vec<Vec<String>>> {
    let base = d.base.as_deref().unwrap_or(rule_path);
    let SourceKind::File { path, .. } = &d.kind else { return None };
    crate::extract::fragment(frag)?;
    let doc = Path::new(base).parent().unwrap_or(Path::new(".")).join(path);
    let tsv = std::fs::read_to_string(crate::extract::copy_dir(&doc).join(crate::extract::fragment_file(frag))).ok()?;
    Some(crate::extract::from_tsv(&tsv))
}

/// How many of a table's own boundaries the copy it cites really words, for the line the
/// approver's page adds (§15.124). A document that writes its bands as `18 to 20`, or with
/// `円以上` over a column of its own, words none of them — and a page that claimed the
/// boundaries had been held would be claiming nothing.
pub fn boundaries_held(rule_path: &str, d: &SourceDecl, frags: &[&str], t: &crate::ast::Table) -> usize {
    use crate::ast::{Cell, Lit};
    let marks: Vec<crate::extract::Bound> = frags
        .iter()
        .filter_map(|f| fragment_grid(rule_path, d, f))
        .flat_map(|g| crate::extract::bounds(&g))
        .collect();
    let mut held: Vec<crate::num::Rat> = Vec::new();
    for r in &t.rows {
        for c in &r.cells {
            let Cell::Cmp(ops) = c else { continue };
            for (_, l) in ops {
                let Lit::Num(n) = l else { continue };
                let Some(v) = crate::types::comparable(n) else { continue };
                if marks.iter().any(|b| crate::types::same_value(&b.value, &v)) && !held.contains(&v.1) {
                    held.push(v.1);
                }
            }
        }
    }
    held.len()
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
        // `--compressed` is not an optimisation here: the eCFR answers 406 to a request that
        // does not say it can take a compressed reply.
        .args(["-fsSL", "--compressed", "--max-time", "120", url])
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

/// One request to an API that answers JSON, with the headers GitHub asks for. A token in
/// `GITHUB_TOKEN` or `GH_TOKEN` is passed on when there is one: without it the rate limit is
/// sixty requests an hour, which a scheduled job will reach.
fn curl_json(url: &str) -> Result<crate::json::Json, String> {
    let mut args: Vec<String> = ["-fsSL", "--max-time", "120", "-H", "Accept: application/vnd.github+json"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    if let Some(t) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")).ok().filter(|t| !t.is_empty()) {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {t}"));
    }
    args.push(url.to_string());
    let out = std::process::Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| tr!("curl を起動できません: {e}", "cannot run curl: {e}"))?;
    if !out.status.success() {
        return Err(tr!(
            "{url} を問い合わせられません: {}",
            "cannot query {url}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    crate::json::parse(&String::from_utf8_lossy(&out.stdout))
}

/// A GitHub raw URL, split into owner, repo, revision and path. `Some` only when the revision
/// is a commit: a URL that names a branch answers with whatever is on that branch today, so it
/// is not a point in time and the question has to be asked of the bytes instead.
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

/// Whether a raw URL names a branch rather than a commit, which is worth saying out loud: the
/// same URL will answer differently next year, so the copy cannot be brought back.
fn raw_on_a_branch(url: &str) -> bool {
    url.starts_with("https://raw.githubusercontent.com/") && github_raw(url).is_none()
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
    db: LawDb,
    id: &str,
    asof: &str,
    index_asof: &str,
    fr: &Fragment,
    cache: &mut BTreeMap<String, Vec<Option<String>>>,
) -> Result<(Vec<u8>, String), String> {
    if db == LawDb::Ecfr {
        // The eCFR serves one section as XML, already the fragment: there is no envelope to
        // open and no revision id to record — which version it is, is the date it was asked
        // for, and `outdated` asks the versions endpoint about amendments since.
        let (title, part) = cfr_id(id)?;
        let url = format!(
            "https://www.ecfr.gov/api/versioner/v1/full/{asof}/title-{title}.xml?part={part}&section={}",
            fr.elm
        );
        return Ok((curl(&url)?, String::new()));
    }
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

/// The title and the part of a CFR id: `29 CFR 1910` is title 29, part 1910.
fn cfr_id(id: &str) -> Result<(String, String), String> {
    let mut it = id.split_whitespace();
    let bad = || {
        tr!(
            "CFR の id は `29 CFR 1910` の形（title と part）で書いてください: `{id}`",
            "a CFR id is written `29 CFR 1910` — a title and a part: `{id}`"
        )
    };
    let title = it.next().ok_or_else(bad)?;
    let cfr = it.next().ok_or_else(bad)?;
    let part = it.next().ok_or_else(bad)?;
    if !cfr.eq_ignore_ascii_case("cfr") || it.next().is_some() {
        return Err(bad());
    }
    if !title.chars().all(|c| c.is_ascii_digit()) || !part.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(bad());
    }
    Ok((title.to_string(), part.to_string()))
}

/// The dates after `asof` on which a cited section of the CFR was amended, each with what
/// the eCFR calls the amendment.
///
/// The endpoint answers per section, and it says whether a version was **substantive**: a
/// re-issue that only moved the markup is not an amendment of the text, the same distinction
/// e-Gov's revisions get (§15.71).
fn ecfr_revisions(id: &str, asof: &str, frags: &[Fragment]) -> Result<Vec<(String, String)>, String> {
    let (title, part) = cfr_id(id)?;
    let mut out: Vec<(String, String)> = Vec::new();
    for fr in frags {
        let url = format!(
            "https://www.ecfr.gov/api/versioner/v1/versions/title-{title}.json?part={part}&section={}",
            fr.elm
        );
        let j = fetch_json_plain(&url)?;
        let Some(crate::json::Json::Arr(vs)) = j.get("content_versions") else { continue };
        for v in vs {
            let date = v.get("amendment_date").and_then(|x| x.as_str()).unwrap_or("");
            let substantive = matches!(v.get("substantive"), Some(crate::json::Json::Bool(true)));
            if substantive && !date.is_empty() && date > asof {
                out.push((date.to_string(), fr.name.clone()));
            }
        }
    }
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    Ok(out)
}

/// One request to an API that answers JSON and nothing else. e-Gov's own replies go through
/// `fetch_json`, which tries again when a busy hour answers with a page.
fn fetch_json_plain(url: &str) -> Result<crate::json::Json, String> {
    let body = curl(url)?;
    crate::json::parse(&String::from_utf8_lossy(&body))
        .map_err(|e| tr!("{url} の答えが JSON として読めません: {e}", "the reply from {url} is not JSON: {e}"))
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
pub fn fetch(f: &RuleFile, rule_path: &str, via: &[String]) -> Result<Outcome, String> {
    let cites = citations(f);
    let mut lines = Vec::new();
    let mut changed = false;
    let mut cache = BTreeMap::new();
    for d in &f.sources {
        let SourceKind::Law { db, id, asof } = &d.kind else { continue };
        let cdir = copy_dir(rule_path, id, asof);
        for (frag, _) in cited_from(&cites, &d.name.text) {
            if frag.is_empty() {
                continue;
            }
            let Some(fr) = fragment(*db, &frag) else {
                lines.push(tr!("{}: 引用箇所 `{frag}` の書き方が読めません", "{}: the fragment `{frag}` cannot be read", d.name.text));
                continue;
            };
            let (xml, rev) = fetch_fragment(*db, id, asof, asof, &fr, &mut cache)?;
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
    // A file source with a `url`: bring the copy again from where it came from (§15.76). A
    // copy whose bytes did not move keeps them, as a fragment's do, so the pin stays true.
    let dir = Path::new(rule_path).parent().unwrap_or(Path::new("."));
    for d in &f.sources {
        let SourceKind::File { path, url, .. } = &d.kind else { continue };
        if d.base.is_some() {
            continue;
        }
        let p = dir.join(path);
        let Some(url) = url else {
            lines.push(tr!(
                "{}: `url` が無いので取り直せません（`{} \"…\"` を足すと取り直せます）",
                "{}: no url, so the copy cannot be brought again (add `{} \"…\"` to it)",
                d.name.text,
                crate::kw::URL
            ));
            // A document handed over by a person has no address, but it is here, and the
            // tables the rule cites still have to come out of it (§15.82).
            changed |= fragments(d, &p, &cites, via, &mut lines)?;
            continue;
        };
        let body = curl(url)?;
        let before = std::fs::read(&p).ok();
        let same = before.as_deref() == Some(body.as_slice());
        if !same {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::write(&p, &body).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        let h = crate::sha256::short(&body);
        lines.push(match before {
            None => tr!("{}: {path} を取りました（sha256:{h}）", "{}: fetched {path} (sha256:{h})", d.name.text),
            Some(_) if same => tr!("{}: {path} は変わっていません（sha256:{h}）", "{}: {path} is unchanged (sha256:{h})", d.name.text),
            Some(_) => {
                changed = true;
                tr!(
                    "{}: {path} が変わりました（いま sha256:{h}）。引いている行を読み直し、`rulec source pin` で承認してください",
                    "{}: {path} changed (now sha256:{h}); reread the rows that cite it, then approve it with `rulec source pin`",
                    d.name.text
                )
            }
        });
        if raw_on_a_branch(url) {
            lines.push(tr!(
                "  この URL はブランチを指しています。コミットを指す URL なら、来年取り直しても同じ写しが返ります",
                "  this URL names a branch; one that names a commit answers with the same copy next year"
            ));
        }
        changed |= fragments(d, &p, &cites, via, &mut lines)?;
    }
    Ok(Outcome { lines, changed })
}

/// The tables a rule cites from a document, taken out of it and written beside it as copies
/// (§15.82). It happens here and never in `check`, which is what lets the extractor be slow,
/// or remote, or not a program at all: what is checked afterwards is the copy.
fn fragments(
    d: &SourceDecl,
    doc: &Path,
    cites: &[(String, String, String, Span)],
    via: &[String],
    lines: &mut Vec<String>,
) -> Result<bool, String> {
    let name = &d.name.text;
    let frags: Vec<String> =
        cited_from(cites, name).into_iter().map(|(f, _)| f).filter(|f| !f.is_empty()).collect();
    if frags.is_empty() {
        return Ok(false);
    }
    // An extractor of its own is used only where this program cannot read the document
    // itself (§15.82): the formats it does read are read the same way every time, and a
    // `--via` that quietly rewrote those copies would make the pins depend on who ran it.
    let outside = crate::extract::unreadable(doc).is_some() && !via.is_empty();
    let (by, tables) = if outside {
        // An extractor that was asked to run and broke stops the command, the way a failing
        // `curl` does. A format with no reader here is a different thing — a statement about
        // the rule, not a failure of the run — so it is one line and the other sources of the
        // rule still get their copies.
        let (id, ts) = crate::extract::via(via, doc).map_err(|e| format!("{name}: {e}"))?;
        (Some(id), ts)
    } else {
        let read = std::fs::read(doc).map_err(|e| e.to_string()).and_then(|b| crate::extract::tables(doc, &b));
        match read {
            Ok(ts) => (None, ts.into_iter().map(|g| (None, g)).collect()),
            Err(why) => {
                lines.push(format!("{name}: {why}"));
                return Ok(false);
            }
        }
    };
    let cdir = crate::extract::copy_dir(doc);
    let mut changed = false;
    for frag in frags {
        let Some(n) = crate::extract::fragment(&frag) else {
            lines.push(tr!(
                "{name}: 引用箇所 `{frag}` の書き方が読めません（`表3` の形です）",
                "{name}: the fragment `{frag}` cannot be read (the form is `表3`)"
            ));
            continue;
        };
        let Some((page, g)) = tables.get(n - 1).filter(|(_, g)| !g.is_empty()) else {
            lines.push(tr!(
                "{name}: {frag} がありません。この文書にある表は {} 個です",
                "{name}: there is no {frag}; the document has {} tables",
                tables.len()
            ));
            continue;
        };
        let text = crate::extract::tsv(g);
        std::fs::create_dir_all(&cdir).map_err(|e| format!("{}: {e}", cdir.display()))?;
        let fp = cdir.join(crate::extract::fragment_file(&frag));
        let before = std::fs::read_to_string(&fp).ok();
        if before.as_deref() != Some(text.as_str()) {
            std::fs::write(&fp, &text).map_err(|e| format!("{}: {e}", fp.display()))?;
        }
        let h = crate::sha256::short(text.as_bytes());
        let (rows, cols) = (g.len(), g.iter().map(|r| r.len()).max().unwrap_or(0));
        let at = page.map(|p| tr!("{p} ページ、", "page {p}, ")).unwrap_or_default();
        lines.push(match before {
            None => tr!(
                "{name}: {frag} を取り出しました（{at}{rows} 行 × {cols} 列、sha256:{h}）",
                "{name}: took out {frag} ({at}{rows} rows by {cols} columns, sha256:{h})"
            ),
            Some(b) if b == text => tr!("{name}: {frag} は変わっていません（sha256:{h}）", "{name}: {frag} is unchanged (sha256:{h})"),
            Some(_) => {
                changed = true;
                tr!(
                    "{name}: {frag} が変わりました（いま sha256:{h}）。引いている行を読み直し、`rulec source pin` で承認してください",
                    "{name}: {frag} changed (now sha256:{h}); reread the rows that cite it, then approve it with `rulec source pin`"
                )
            }
        });
    }
    // Who read the document, beside the copies it produced — the same place a law's copies
    // say which revision e-Gov served. A reader of the copy can then tell a grid that was
    // already a grid from one a model read out of a scan.
    if let Some(id) = by {
        let _ = std::fs::write(cdir.join("extractor.txt"), format!("{id}\n"));
        lines.push(tr!("{name}: {id} が読みました", "{name}: read by {id}"));
    }
    Ok(changed)
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
            SourceKind::File { path, hash, .. } => {
                let p = dir.join(path);
                let bytes = std::fs::read(&p).map_err(|e| format!("{path}: {e}"))?;
                let h = crate::sha256::short(&bytes);
                if hash.as_deref() != Some(h.as_str()) {
                    let line = lines.get(d.span.line - 1).copied().unwrap_or("");
                    edits.push((d.span.line - 1, 1, vec![set_hash(line, &h)]));
                    report.push(tr!("{}: sha256:{h} を固定しました", "{}: pinned sha256:{h}", d.name.text));
                }
                // The tables cited from the document, each pinned at its copy (§15.82). The
                // document's own digest stays on the line above: the copy is a step away
                // from it, and a step that an extractor may take differently.
                let cdir = crate::extract::copy_dir(&p);
                let mut new_pins: Vec<String> = Vec::new();
                for (frag, _) in cited_from(&cites, &d.name.text) {
                    if frag.is_empty() || crate::extract::fragment(&frag).is_none() {
                        continue;
                    }
                    match std::fs::read(cdir.join(crate::extract::fragment_file(&frag))) {
                        Ok(bytes) => new_pins.push(pin_line(&frag, &crate::sha256::short(&bytes))),
                        Err(_) => {
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
            SourceKind::Law { db, id, asof } => {
                let cdir = copy_dir(rule_path, id, asof);
                let mut new_pins: Vec<String> = Vec::new();
                for (frag, _) in cited_from(&cites, &d.name.text) {
                    let Some(fr) = fragment(*db, &frag).filter(|_| !frag.is_empty()) else { continue };
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
        if let SourceKind::Law { id, asof, .. } = &d.kind {
            let e = latest.entry(id.as_str()).or_insert(asof.as_str());
            if asof.as_str() > *e {
                *e = asof.as_str();
            }
        }
    }
    // The one date input, when there is exactly one: what the rows for the new text are keyed on.
    let date_inputs: Vec<&str> = f.inputs.iter().filter(|i| i.ty.base == crate::kw::DATE).map(|i| i.name.text.as_str()).collect();
    for d in &f.sources {
        let SourceKind::Law { db, id, asof } = &d.kind else { continue };
        let db = *db;
        if latest.get(id.as_str()).is_some_and(|l| *l != asof.as_str()) {
            lines.push(tr!(
                "{}: 同じ法令をもっと後の時点で引用している出典があるので、改正はそちらで確かめます",
                "{}: another source reads this law as of a later date; amendments are asked about there",
                d.name.text
            ));
            continue;
        }
        let mut later: Vec<(String, String)> = match db {
            LawDb::Egov => {
                let j = fetch_json(&format!("https://laws.e-gov.go.jp/api/2/law_revisions/{id}"))?;
                let mut v = Vec::new();
                if let Some(crate::json::Json::Arr(revs)) = j.get("revisions") {
                    for r in revs {
                        let date = r.get("amendment_enforcement_date").and_then(|x| x.as_str()).unwrap_or("");
                        let rid = r.get("law_revision_id").and_then(|x| x.as_str()).unwrap_or("");
                        if !date.is_empty() && date > asof.as_str() {
                            v.push((date.to_string(), rid.to_string()));
                        }
                    }
                }
                v
            }
            // The eCFR records an amendment against the section, so the question is asked of
            // the sections this rule cites rather than of the part as a whole.
            LawDb::Ecfr => {
                let frags: Vec<Fragment> = cited_from(&cites, &d.name.text)
                    .into_iter()
                    .filter(|(f, _)| !f.is_empty())
                    .filter_map(|(f, _)| fragment(db, &f))
                    .collect();
                ecfr_revisions(id, asof, &frags)?
            }
        };
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
            let Some(fr) = fragment(db, &frag).filter(|_| !frag.is_empty()) else { continue };
            let mut prev = std::fs::read(cdir.join(fr.file())).ok();
            for (date, _) in &later {
                let (xml, _) = fetch_fragment(db, id, date, asof, &fr, &mut cache)?;
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
                    // The line to add names the database when the rule's own line does, so
                    // that what is offered can be pasted as it stands.
                    let which = match db {
                        LawDb::Egov => String::new(),
                        x => format!(" {}", x.word()),
                    };
                    lines.push(tr!(
                        "  `{} {name} = {}{which} \"{id}\" {} {date}` を足し、{rows}をそこから写してください",
                        "  add `{} {name} = {}{which} \"{id}\" {} {date}` and transcribe {rows} from it",
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
    // A file source (§15.76). What can be asked depends on what the `url` names: a commit
    // cannot go stale, so the repository is asked what it has done to that path since; anything
    // else is asked of its bytes.
    let dir = Path::new(rule_path).parent().unwrap_or(Path::new("."));
    for d in &f.sources {
        let SourceKind::File { path, url, hash } = &d.kind else { continue };
        if d.base.is_some() {
            continue;
        }
        let name = &d.name.text;
        let Some(url) = url else {
            lines.push(tr!(
                "{name}: `url` が無いので、元の文書が変わったかを確かめられません（`{} \"…\"` を足すと確かめられます）",
                "{name}: no url, so whether the original moved on cannot be asked (add `{} \"…\"` to it)",
                crate::kw::URL
            ));
            continue;
        };
        match github_raw(url) {
            Some((owner, repo, rev, p)) => {
                let cmp = curl_json(&format!("https://api.github.com/repos/{owner}/{repo}/compare/{rev}...HEAD"))?;
                let touched = match cmp.get("files") {
                    Some(crate::json::Json::Arr(fs)) => {
                        fs.iter().any(|f| f.get("filename").and_then(|v| v.as_str()) == Some(p.as_str()))
                    }
                    _ => false,
                };
                if !touched {
                    lines.push(tr!(
                        "{name}: {rev} 以後、{p} は変わっていません",
                        "{name}: {p} has not changed since {rev}"
                    ));
                    continue;
                }
                changed = true;
                lines.push(tr!("{name}: {rev} 以後、{p} が変わっています", "{name}: {p} has changed since {rev}"));
                let commits =
                    curl_json(&format!("https://api.github.com/repos/{owner}/{repo}/commits?path={}&per_page=5", urlq(&p)))?;
                let mut newest = String::new();
                if let crate::json::Json::Arr(cs) = &commits {
                    for c in cs {
                        let sha = c.get("sha").and_then(|v| v.as_str()).unwrap_or("");
                        let date = c
                            .get("commit")
                            .and_then(|v| v.get("author"))
                            .and_then(|v| v.get("date"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let msg = c
                            .get("commit")
                            .and_then(|v| v.get("message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .lines()
                            .next()
                            .unwrap_or("");
                        if newest.is_empty() {
                            newest = sha.to_string();
                        }
                        let short: String = sha.chars().take(7).collect();
                        lines.push(format!("  {short} {} {msg}", date.split('T').next().unwrap_or(date)));
                    }
                }
                if !newest.is_empty() {
                    // Which of the cited tables the newer copy moved. One more request, only
                    // when the path was touched, and only to say it more precisely; a failure
                    // here leaves the answer above standing.
                    let raw = format!("https://raw.githubusercontent.com/{owner}/{repo}/{newest}/{p}");
                    if let Ok(body) = curl(&raw) {
                        lines.extend(fragment_lines(moved_fragments(d, path, &body, &cites)));
                    }
                    lines.push(tr!(
                        "  引いている行を読み直してから、`{} \"{raw}\"` に固定し直し、`rulec source fetch` と `rulec source pin` を走らせてください",
                        "  once the rows are reread, repin it at `{} \"{raw}\"`, then `rulec source fetch` and `rulec source pin`",
                        crate::kw::URL
                    ));
                }
            }
            None => {
                let body = curl(url)?;
                let now = crate::sha256::short(&body);
                let before = hash
                    .clone()
                    .or_else(|| std::fs::read(dir.join(path)).ok().map(|b| crate::sha256::short(&b)));
                match before {
                    None => lines.push(tr!(
                        "{name}: 固定も写しも無いので比べられません（いま sha256:{now}）",
                        "{name}: nothing to compare against, neither a pin nor a copy (it is sha256:{now} now)"
                    )),
                    Some(b) if b == now => {
                        lines.push(tr!("{name}: {path} は変わっていません（sha256:{now}）", "{name}: {path} is unchanged (sha256:{now})"))
                    }
                    Some(b) => {
                        changed = true;
                        lines.push(tr!(
                            "{name}: 元の文書が変わっています（固定: sha256:{b}、いま: sha256:{now}）。`rulec source fetch` で取り直し、引いている行を読み直してから `rulec source pin` で承認してください",
                            "{name}: the original has changed (pinned: sha256:{b}, now: sha256:{now}); bring it again with `rulec source fetch`, reread the rows, then `rulec source pin`"
                        ));
                        let moved = moved_fragments(d, path, &body, &cites);
                        let told = moved.is_some();
                        lines.extend(fragment_lines(moved));
                        // A format nothing here can read is a format nothing here can diff —
                        // unless the rule cites tables out of it, which were just compared.
                        if opaque(path) && !told {
                            lines.push(tr!(
                                "  この形式では中身の差分を取れないので、変わったということしか言えません",
                                "  nothing here can diff this format, so all it can say is that it changed"
                            ));
                        }
                    }
                }
                if raw_on_a_branch(url) {
                    lines.push(tr!(
                        "  この URL はブランチを指しています。コミットを指す URL なら、何がいつ変えたかまで言えます",
                        "  this URL names a branch; one that names a commit would say what changed it, and when"
                    ));
                }
            }
        }
    }
    Ok(Outcome { lines, changed })
}

/// Percent-encode a path for a query parameter, leaving what GitHub accepts bare.
fn urlq(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// A document nothing here can diff: all that can be said of it is that it changed.
fn opaque(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    [".pdf", ".xlsx", ".xls", ".docx", ".doc", ".zip", ".png", ".jpg"].iter().any(|e| p.ends_with(e))
}

/// Which of the cited tables differ in a newer copy of a document, and which do not: the
/// question `outdated` could not ask of a file before, when all it could say was that the
/// bytes had moved (§15.82). An empty answer with fragments cited means the document changed
/// somewhere the rule does not transcribe — which is worth saying out loud.
fn moved_fragments(
    d: &SourceDecl,
    path: &str,
    body: &[u8],
    cites: &[(String, String, String, Span)],
) -> Option<Vec<String>> {
    let frags: Vec<String> =
        cited_from(cites, &d.name.text).into_iter().map(|(f, _)| f).filter(|f| !f.is_empty()).collect();
    if frags.is_empty() {
        return None;
    }
    let tables = crate::extract::tables(Path::new(path), body).ok()?;
    let mut out = Vec::new();
    for frag in frags {
        let Some(n) = crate::extract::fragment(&frag) else { continue };
        let now = tables.get(n - 1).map(|g| crate::sha256::short(crate::extract::tsv(g).as_bytes()));
        let pinned = d.pins.iter().find(|p| p.fragment == frag).map(|p| p.hash.clone());
        if now.is_none() || now != pinned {
            out.push(frag);
        }
    }
    Some(out)
}

/// What `outdated` says about the cited tables of a document that moved.
fn fragment_lines(moved: Option<Vec<String>>) -> Vec<String> {
    let sep = if crate::i18n::ja() { "、" } else { ", " };
    match moved {
        None => Vec::new(),
        Some(fs) if fs.is_empty() => vec![tr!(
            "  引いている表は変わっていません。動いたのは、この規則が写していないところです",
            "  the tables it cites are unchanged: what moved is somewhere this rule does not transcribe"
        )],
        Some(fs) => vec![tr!(
            "  引いている表のうち {} が変わります。読み直すのはその表を引いている行だけです",
            "  of the tables it cites, {} changed; the rows that cite them are all there is to reread",
            fs.join(sep)
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A raw URL is a point in time only when its revision is a commit. A branch answers with
    /// whatever is on it today, so it must not be mistaken for one — `outdated` would then ask
    /// the repository a question about a moving target and report nothing.
    #[test]
    fn 生のurlはコミットのときだけ時点を指す() {
        let commit = "https://raw.githubusercontent.com/o/r/a1b2c3d4e5f60718293a4b5c6d7e8f9012345678/docs/t.md";
        let got = github_raw(commit).expect("コミットの URL が読めない");
        assert_eq!(got.0, "o");
        assert_eq!(got.1, "r");
        assert_eq!(got.2, "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678");
        assert_eq!(got.3, "docs/t.md");
        // A short sha is still a commit; a branch, a tag and a foreign host are not.
        assert!(github_raw("https://raw.githubusercontent.com/o/r/a1b2c3d/t.md").is_some());
        for not in [
            "https://raw.githubusercontent.com/o/r/main/t.md",
            "https://raw.githubusercontent.com/o/r/v1.2.0/t.md",
            "https://raw.githubusercontent.com/o/r/abc/t.md",
            "https://raw.githubusercontent.com/o/r/a1b2c3d",
            "https://example.com/o/r/a1b2c3d/t.md",
        ] {
            assert!(github_raw(not).is_none(), "{not} をコミットとして読んでしまう");
        }
        assert!(raw_on_a_branch("https://raw.githubusercontent.com/o/r/main/t.md"));
        assert!(!raw_on_a_branch(commit));
        assert!(!raw_on_a_branch("https://example.com/t.pdf"));
    }

    #[test]
    fn 問い合わせのパスはパーセント符号化される() {
        assert_eq!(urlq("docs/料金表.md"), "docs/%E6%96%99%E9%87%91%E8%A1%A8.md");
        assert_eq!(urlq("a b/c.md"), "a%20b/c.md");
        assert_eq!(urlq("docs/t-1_2.md"), "docs/t-1_2.md");
    }

    #[test]
    fn fragments_are_named_as_the_law_names_them() {
        assert_eq!(fragment(LawDb::Egov, "第91条").unwrap().elm, "MainProvision-Article_91");
        assert_eq!(fragment(LawDb::Egov, "第20条の2").unwrap().elm, "MainProvision-Article_20_2");
        assert_eq!(fragment(LawDb::Egov, "第20条第2項").unwrap().elm, "MainProvision-Article_20-Paragraph_2");
        assert_eq!(fragment(LawDb::Egov, "第20条第2項第3号").unwrap().elm, "MainProvision-Article_20-Paragraph_2-Item_3");
        assert_eq!(fragment(LawDb::Egov, "第二十条第二項").unwrap().elm, "MainProvision-Article_20-Paragraph_2");
        assert_eq!(fragment(LawDb::Egov, "別表第一").unwrap().elm, "AppdxTable[1]");
        assert_eq!(fragment(LawDb::Egov, "別表第一").unwrap().file(), "AppdxTable_1.xml");
        assert_eq!(fragment(LawDb::Egov, "別表第十二").unwrap().elm, "AppdxTable[12]");
        assert!(fragment(LawDb::Egov, "第3条ただし書").is_none());
    }

    #[test]
    fn supplementary_provisions_are_named_by_their_amending_law() {
        let own = fragment(LawDb::Egov, "附則第3条").unwrap();
        assert_eq!(own.elm, "SupplProvision-Article_3");
        assert_eq!(own.file(), "SupplProvision-Article_3.xml");
        assert_eq!(fragment(LawDb::Egov, "附則").unwrap().elm, "SupplProvision");
        let a = fragment(LawDb::Egov, "附則（令和七年三月三一日法律第一三号）第3条第2項").unwrap();
        assert_eq!(a.elm, "SupplProvision[?]-Article_3-Paragraph_2");
        assert_eq!(a.amend.as_deref(), Some("令和七年三月三一日法律第一三号"));
        assert_eq!(a.file(), "SupplProvision_令和七年三月三一日法律第一三号-Article_3-Paragraph_2.xml");
        assert_eq!(a.elm_at(244), "SupplProvision[244]-Article_3-Paragraph_2");
        let whole = fragment(LawDb::Egov, "附則(令和六年三月三〇日法律第八号)").unwrap();
        assert_eq!(whole.elm, "SupplProvision[?]");
        assert_eq!(whole.file(), "SupplProvision_令和六年三月三〇日法律第八号.xml");
        assert!(fragment(LawDb::Egov, "附則（）第3条").is_none());
        assert!(fragment(LawDb::Egov, "附則の3").is_none());
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
