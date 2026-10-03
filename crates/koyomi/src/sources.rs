//! Sources (DESIGN 1.5): the copies a `.cal` reads, and the pins that hold them.
//!
//! A table of holidays is one file beside the calendar, pinned whole. A law is copied an
//! article at a time from e-Gov, `sources/law/<law id>@<asof>/<element>.xml` beside the
//! `.cal`, each pinned on a line under the `source`, the way rulec keeps its copies. Checking
//! reads the copies and compares; it never reads the network (`source fetch|pin|outdated`
//! are stage C's).

use crate::ast::{Cite, Covers, File, Format, SourceDecl, SourceKind, Span};
use crate::date::Day;
use crate::diag::Diag;
use crate::holidays::{self, Row};
use crate::i18n::count;
use crate::sha256;
use std::path::{Path, PathBuf};

/// A table of holidays, read and checked.
#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    /// The copy's path as the `.cal` writes it.
    pub file: String,
    /// The copy's path from where koyomi runs.
    pub path: PathBuf,
    pub url: Option<String>,
    /// The sixteen digits pinned (equal to the copy's, or the check would have stopped).
    pub pin: String,
    /// The copy's whole digest.
    pub sha256: String,
    pub format: Format,
    pub covers: (Day, Day),
    pub listed_years: bool,
    pub rows: Vec<Row>,
}

/// What a `.cal`'s path is shown as, and where the files beside it are.
pub struct Origin<'a> {
    pub file: &'a File,
    /// The directory the `.cal` is in.
    pub dir: &'a Path,
}

impl Origin<'_> {
    fn err(&self, code: &'static str, s: Span, msg: crate::i18n::Text) -> Diag {
        Diag::error(code, &self.file.path, s.line, s.col, msg).source(&self.file.src)
    }

    fn warn(&self, code: &'static str, s: Span, msg: crate::i18n::Text) -> Diag {
        Diag::warning(code, &self.file.path, s.line, s.col, msg).source(&self.file.src)
    }
}

/// The source line with `sha256:<pin>` in place of whatever pin it had.
fn pinned_line(line: &str, pin: &str) -> String {
    let t = line.trim_end();
    match t.find("sha256:") {
        Some(i) => format!("{}sha256:{pin}{}", &t[..i], &t[(i + 23).min(t.len())..]),
        None => format!("{t} sha256:{pin}"),
    }
}

/// Read, pin and check one table (E101–E106).
pub fn load_table(o: &Origin, decl: &SourceDecl) -> Result<Table, Vec<Diag>> {
    let SourceKind::File { path, url, pin, format, covers } = &decl.kind else {
        unreachable!("load_table is called on tables only")
    };
    let (format, _) = format.clone().expect("the parser requires `format`");
    let (covers, covers_span) = covers.clone().expect("the parser requires `covers`");
    let name = &decl.name;
    let full = o.dir.join(path);
    let Ok(bytes) = std::fs::read(&full) else {
        let mut d = o.err("E101", decl.span, tr!("出典「{name}」の写し {path} がありません", "The copy of the source {name} is not there: {path}"));
        d = match url {
            Some(_) => d.note(tr!(
                "`koyomi source fetch` が url から取ってきて、そこに書きます。check は通信しません。",
                "`koyomi source fetch` takes it from the url and writes it there; check never reads the network."
            )),
            None => d.note(tr!(
                "写しは .cal からの相対パスで探します。ファイルを置くか、パスを直します。",
                "The copy is looked for relative to the .cal; put the file there or correct the path."
            )),
        };
        return Err(vec![d]);
    };
    let digest = sha256::hex(&bytes);
    let actual = &digest[..16];
    let line_text = o.file.src.lines().nth(decl.span.line - 1).unwrap_or("").to_string();
    match pin {
        None => {
            return Err(vec![
                o.err("E102", decl.span, tr!("出典「{name}」が固定されていません（`sha256:` がありません）", "The source {name} is not pinned (it has no `sha256:`)"))
                    .note(tr!(
                        "写しのバイト列の SHA-256 の先頭 16 桁を書いて固定します。いまの写しなら sha256:{actual} です（`koyomi source pin` でも書けます）。",
                        "Pin it with the first 16 digits of the SHA-256 of the copy's bytes; for the copy as it is, that is sha256:{actual} (`koyomi source pin` writes it too)."
                    ))
                    .fix(pinned_line(&line_text, actual)),
            ]);
        }
        Some(p) if p != actual => {
            return Err(vec![
                o.err("E103", decl.span, tr!(
                    "出典「{name}」の写しが固定と違います（固定は sha256:{p}、写しは sha256:{actual}）",
                    "The copy of the source {name} does not match its pin (pinned sha256:{p}, the copy is sha256:{actual})"
                ))
                .note(tr!(
                    "固定したあとで写しが変わりました。何が変わったかを読んでから（`koyomi source outdated`）、固定を書き換えます。",
                    "The copy changed after it was pinned. Read what changed (`koyomi source outdated`), then pin it again."
                ))
                .fix(pinned_line(&line_text, actual)),
            ]);
        }
        Some(_) => {}
    }
    let rows = match &format {
        Format::Csv { shift_jis } => holidays::read_csv(&bytes, *shift_jis),
        Format::GovUk { division } => holidays::read_govuk(&bytes, division),
    };
    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            let msg = match e.line {
                Some(l) => tr!("出典「{name}」の写し {path} の {l} 行目が読めません: {}", "The copy {path} of the source {name} cannot be read at line {l}: {}", e.why.ja; e.why.en),
                None => tr!("出典「{name}」の写し {path} が読めません: {}", "The copy {path} of the source {name} cannot be read: {}", e.why.ja; e.why.en),
            };
            return Err(vec![o.err("E104", decl.span, msg)]);
        }
    };
    if rows.is_empty() {
        return Err(vec![o.err("E104", decl.span, tr!("出典「{name}」の写し {path} に行が一つもありません", "The copy {path} of the source {name} has no rows"))]);
    }
    let (lo, hi, listed) = match covers {
        Covers::Range(a, b) => {
            let out: Vec<&Row> = rows.iter().filter(|r| r.day < a || r.day > b).collect();
            if !out.is_empty() {
                let shown: Vec<String> = out.iter().take(6).map(|r| if r.name.is_empty() { r.day.to_string() } else { format!("{} {}", r.day, r.name) }).collect();
                let more = out.len().saturating_sub(6);
                let (shown_ja, shown) = (shown.join("、"), shown.join(", "));
                let unit = if out.len() == 1 { "row" } else { "rows" };
                let mut d = o.err("E105", covers_span, tr!(
                    "表「{name}」に `covers {a}..{b}` の外の行が {} 行あります",
                    "The table {name} has {} {unit} outside `covers {a}..{b}`",
                    count(out.len() as u64)
                ))
                .note(if more > 0 {
                    tr!("外にある行: {shown_ja}、ほか {more} 行", "outside: {shown}, and {more} more")
                } else {
                    tr!("外にある行: {shown_ja}", "outside: {shown}")
                });
                let first = rows.first().unwrap().day;
                let last = rows.last().unwrap().day;
                let (y0, y1) = (first.year(), last.year());
                d = d
                    .note(tr!(
                        "`covers` は、表が休みを全部載せている範囲です。表が延びたのなら範囲も延ばします。表の行は {first}〜{last} にあります。",
                        "`covers` is the span the table lists every closed day of; when the table grew, so does the span. The table's rows run from {first} to {last}."
                    ))
                    .fix(format!("  covers {}..{}", a.min(Day::from_ymd(y0 as i64, 1, 1).unwrap()), b.max(Day::from_ymd(y1 as i64, 12, 31).unwrap())));
                return Err(vec![d]);
            }
            (a, b, false)
        }
        Covers::ListedYears => {
            let y0 = rows.first().unwrap().day.year();
            let y1 = rows.last().unwrap().day.year();
            let gaps: Vec<i32> = (y0..=y1).filter(|y| !rows.iter().any(|r| r.day.year() == *y)).collect();
            if !gaps.is_empty() {
                let shown: Vec<String> = gaps.iter().map(|y| y.to_string()).collect();
                let shown = shown.join(", ");
                return Err(vec![
                    o.err("E106", covers_span, tr!(
                        "`covers listed years` ですが、表「{name}」には {shown} 年の行がありません",
                        "`covers listed years`, but the table {name} has no rows in {shown}"
                    ))
                    .note(tr!(
                        "行の無い年が休みの無い年なのか、表から落ちた年なのかを、koyomi は決められません。範囲を日付で書きます（`covers {y0}-01-01..{y1}-12-31` のように）。",
                        "koyomi cannot tell a year with no closed days from a year missing from the table; write the span as dates (like `covers {y0}-01-01..{y1}-12-31`)."
                    )),
                ]);
            }
            (Day::from_ymd(y0 as i64, 1, 1).unwrap(), Day::from_ymd(y1 as i64, 12, 31).unwrap(), true)
        }
    };
    Ok(Table {
        name: name.clone(),
        file: path.clone(),
        path: full,
        url: url.clone(),
        pin: actual.to_string(),
        sha256: digest,
        format,
        covers: (lo, hi),
        listed_years: listed,
        rows,
    })
}

/// A law and the articles of it a `.cal` pins.
#[derive(Clone, Debug)]
pub struct Law {
    pub name: String,
    pub id: String,
    pub asof: Day,
    pub dir: PathBuf,
    /// The revision e-Gov served, from `revision.txt` beside the copies.
    pub revision: Option<String>,
    pub pins: Vec<Pinned>,
}

#[derive(Clone, Debug)]
pub struct Pinned {
    pub fragment: String,
    pub elm: String,
    pub pin: String,
    pub path: PathBuf,
}

/// A number as a law writes it: `百四十三`, `二十`, `千五十`, or ASCII digits.
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

/// The e-Gov element an article, paragraph or item of the main provisions is addressed by:
/// `第20条の2第3項第4号` is `MainProvision-Article_20_2-Paragraph_3-Item_4`. The
/// supplementary provisions and the appended tables are not cited (DESIGN 1.5).
pub fn elm(fragment: &str) -> Option<String> {
    let rest = fragment.strip_prefix('第')?;
    let (art, rest) = rest.split_once('条')?;
    let mut e = format!("MainProvision-Article_{}", law_number(art)?);
    let mut rest = rest;
    if let Some(r) = rest.strip_prefix('の') {
        let end = r.find('第').unwrap_or(r.len());
        e.push_str(&format!("_{}", law_number(&r[..end])?));
        rest = &r[end..];
    }
    if let Some(r) = rest.strip_prefix('第') {
        let (para, r) = r.split_once('項')?;
        e.push_str(&format!("-Paragraph_{}", law_number(para)?));
        rest = r;
        if let Some(r) = rest.strip_prefix('第') {
            let (item, r) = r.split_once('号')?;
            e.push_str(&format!("-Item_{}", law_number(item)?));
            rest = r;
        }
    }
    if !rest.is_empty() {
        return None;
    }
    Some(e)
}

/// The directory a law's copies as of a date are kept in, beside the `.cal`.
pub fn copy_dir(dir: &Path, id: &str, asof: Day) -> PathBuf {
    dir.join("sources").join("law").join(format!("{id}@{asof}"))
}

/// Every citation in a file, with where it is.
pub fn citations(f: &File) -> Vec<&Cite> {
    let mut out = Vec::new();
    for r in &f.rules {
        out.extend(r.cite.as_ref());
    }
    for d in &f.dates {
        out.extend(d.cite.as_ref());
        for o in &d.ops {
            out.extend(o.cite.as_ref());
        }
        if let Some((_, _, c)) = &d.at {
            out.extend(c.as_ref());
        }
    }
    for c in &f.claims {
        out.extend(c.cite.as_ref());
    }
    out
}

const SHAPES: (&str, &str) = (
    "書けるのは本則の条・項・号で、`第143条`、`第143条第2項`、`第143条第2項第1号`、`第20条の2`（漢数字でもよい）の形です。附則と別表はまだ引けません。",
    "A citation names an article, paragraph or item of the main provisions: `第143条`, `第143条第2項`, `第143条第2項第1号`, `第20条の2` (in kanji numerals too). Supplementary provisions and appended tables are not cited yet.",
);

/// Check a file's laws and citations: the pins (E101–E103), what is cited (E111), and what is
/// pinned but not cited (W102).
pub fn check_laws(o: &Origin) -> (Vec<Law>, Vec<crate::diag::Diag>) {
    let f = o.file;
    let mut diags = Vec::new();
    let mut laws = Vec::new();
    let cites = citations(f);
    for s in &f.sources {
        let SourceKind::Law { id, asof, pins } = &s.kind else { continue };
        let dir = copy_dir(o.dir, id, *asof);
        let rel = format!("sources/law/{id}@{asof}");
        let revision = std::fs::read_to_string(dir.join("revision.txt")).ok().map(|r| r.trim().to_string());
        let mut law = Law { name: s.name.clone(), id: id.clone(), asof: *asof, dir: dir.clone(), revision, pins: vec![] };
        for p in pins {
            let Some(e) = elm(&p.fragment) else {
                diags.push(o.err("E111", p.span, tr!("`{}` は条・項・号の書き方になっていません", "`{}` is not written as an article, a paragraph or an item", p.fragment)).note(tr!("{}", "{}", SHAPES.0; SHAPES.1)));
                continue;
            };
            let path = dir.join(format!("{e}.xml"));
            let Ok(bytes) = std::fs::read(&path) else {
                diags.push(
                    o.err("E101", p.span, tr!("{} {}の写し {rel}/{e}.xml がありません", "The copy of {} {} is not there: {rel}/{e}.xml", s.name, p.fragment))
                        .note(tr!(
                            "`koyomi source fetch` が e-Gov から取ってきて、そこに書きます。check は通信しません。",
                            "`koyomi source fetch` takes it from e-Gov and writes it there; check never reads the network."
                        )),
                );
                continue;
            };
            let actual = sha256::short(&bytes);
            let line_text = f.src.lines().nth(p.span.line - 1).unwrap_or("").to_string();
            match &p.pin {
                None => {
                    diags.push(
                        o.err("E102", p.span, tr!("{} {}が固定されていません（`sha256:` がありません）", "{} {} is not pinned (it has no `sha256:`)", s.name, p.fragment))
                            .note(tr!("いまの写しなら sha256:{actual} です。", "For the copy as it is, that is sha256:{actual}."))
                            .fix(pinned_line(&line_text, &actual)),
                    );
                }
                Some(pin) if *pin != actual => {
                    diags.push(
                        o.err("E103", p.span, tr!(
                            "{} {}の写しが固定と違います（固定は sha256:{pin}、写しは sha256:{actual}）",
                            "The copy of {} {} does not match its pin (pinned sha256:{pin}, the copy is sha256:{actual})",
                            s.name,
                            p.fragment
                        ))
                        .note(tr!(
                            "固定したあとで写しが変わりました。条文の何が変わったかを読んでから、固定を書き換えます。",
                            "The copy changed after it was pinned. Read what changed in the text, then pin it again."
                        ))
                        .fix(pinned_line(&line_text, &actual)),
                    );
                }
                Some(pin) => law.pins.push(Pinned { fragment: p.fragment.clone(), elm: e, pin: pin.clone(), path }),
            }
        }
        // W102: pinned, and cited nowhere.
        for p in pins {
            let cited = cites.iter().any(|c| c.source == s.name && c.fragments.iter().any(|(fr, _)| fr == &p.fragment));
            if !cited {
                diags.push(
                    o.warn("W102", p.span, tr!("{} {}は固定されていますが、どこからも引かれていません", "{} {} is pinned but cited nowhere", s.name, p.fragment))
                        .note(tr!("引用を消したあとの残りです。固定の行を消します。", "It is what is left after a citation was removed; delete the pin line.")),
                );
            }
        }
        laws.push(law);
    }
    // E111: what a citation names.
    for c in cites {
        let Some(s) = f.sources.iter().find(|s| s.name == c.source) else {
            diags.push(o.err("E111", c.span, tr!("出典「{}」は宣言されていません", "The source {} is not declared", c.source)).note(tr!(
                "引く法令は `source {} = law \"<法令ID>\" asof <日付>` で宣言し、その下に引く条を固定します。",
                "Declare the law with `source {} = law \"<law id>\" asof <date>`, and pin the articles cited under it.",
                c.source
            )));
            continue;
        };
        let SourceKind::Law { id, asof, pins } = &s.kind else {
            diags.push(o.err("E111", c.span, tr!(
                "「{}」は祝日の表で、`@` では引けません。`@` で引けるのは法令だけです",
                "{} is a table of holidays, which `@` does not cite; `@` cites a law",
                c.source
            )));
            continue;
        };
        if c.fragments.is_empty() {
            diags.push(o.err("E111", c.span, tr!(
                "法令は条の単位で写すので、`@{} 第143条` のように、どこを引いたかを書きます",
                "A law is copied an article at a time, so say which one: `@{} 第143条`",
                c.source
            )));
            continue;
        }
        for (fr, sp) in &c.fragments {
            let Some(e) = elm(fr) else {
                diags.push(o.err("E111", *sp, tr!("`{fr}` は条・項・号の書き方になっていません", "`{fr}` is not written as an article, a paragraph or an item")).note(tr!("{}", "{}", SHAPES.0; SHAPES.1)));
                continue;
            };
            if !pins.iter().any(|p| &p.fragment == fr) {
                let path = copy_dir(o.dir, id, *asof).join(format!("{e}.xml"));
                let fix = match std::fs::read(&path) {
                    Ok(b) => format!("  {fr} sha256:{}", sha256::short(&b)),
                    Err(_) => format!("  {fr} sha256:<koyomi source fetch, then koyomi source pin>"),
                };
                diags.push(
                    o.err("E111", *sp, tr!("{} {fr}を引いていますが、固定されていません", "{} {fr} is cited but not pinned", c.source))
                        .note(tr!(
                            "引く条は、出典の下に固定の行を書きます。どの版の条文を見て書いたかを、固定で残すためです。",
                            "Every article cited has a pin line under its source, which records the version of the text the line was written against."
                        ))
                        .fix(fix),
                );
            }
        }
    }
    (laws, diags)
}

/// The text of an article's XML, with the tags removed: a line for each paragraph, item and
/// sentence (rulec's `xml_text`). `source outdated` compares it and shows its lines; the
/// approver's page quotes [`article_lines`] instead.
pub fn xml_text(xml: &str) -> String {
    let breaks = ["Paragraph", "Item", "Subitem1", "ArticleCaption", "ArticleTitle", "Sentence"];
    let mut out = String::new();
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else { break };
        let tag = &rest[lt + 1..lt + gt];
        if let Some(name) = tag.strip_prefix('/')
            && breaks.contains(&name.trim())
        {
            out.push('\n');
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

/// The text of a pinned article, from its copy.
pub fn article_text(p: &Pinned) -> Option<String> {
    std::fs::read_to_string(&p.path).ok().map(|x| xml_text(&x))
}

/// U+3000, the space a Japanese law puts after an article's or a paragraph's number.
const WIDE_SPACE: char = '　';

/// An article's XML as the law prints it, for the approver's page: the caption on a line of
/// its own, the article's number and its first paragraph on the next, and then a line for
/// every other paragraph, item and subitem, its number and its sentences together
/// (`２　週、月又は年の初めから…。ただし、…。`). Ruby readings are left out. [`xml_text`] keeps
/// a line a sentence instead, which is what `source outdated` compares.
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
        if tag.ends_with('/') {
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
