//! Tables out of a document, for the fragments a `file` source cites (§15.82).
//!
//! Only the formats rulec reads by itself are here: a CSV, the pipe tables of a Markdown
//! document, the sheets of a workbook and the tables of a Word document. A PDF or a scan needs
//! an extractor that is not this program — that is the adapter, and until it exists the message
//! says which formats are read rather than guessing at the bytes.
//!
//! What comes out is one grid of text per table, in document order. It is written beside the
//! document as a TSV, and that copy is what `check` holds the rule to: the extraction happens
//! once, in `rulec source fetch`, and every later check compares against the copy — which is
//! what lets an extractor be slow, or remote, or not a program at all.

use std::path::Path;

/// The fragment a citation names: `表3` is the third table of the document, counted from one,
/// and `table3` is the same in English. A document has no articles, so its fragments are its
/// tables; a heading is not a fragment yet (§15.82).
pub fn fragment(name: &str) -> Option<usize> {
    let rest = name.strip_prefix('表').or_else(|| name.strip_prefix("table"))?;
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok().filter(|n| *n > 0)
}

/// The file a fragment's copy is kept in, under the document's own `.fragments` directory.
pub fn fragment_file(name: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_control() || c == '/' || c == '\\' { '_' } else { c }).collect();
    format!("{safe}.tsv")
}

/// Where the copies of a document's fragments sit: beside the document, in a directory named
/// after it — the whole file name and not its stem, so that two documents of the same name in
/// two formats (`料金表.pdf` and `料金表.md`) do not share one directory and quietly hold each
/// other's tables. A law's copies live under `sources/law/` because e-Gov's document has no
/// path here; a file source has one, so the copies stand next to what they came from (§15.82).
pub fn copy_dir(doc: &Path) -> std::path::PathBuf {
    let name = doc.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into());
    doc.with_file_name(format!("{name}.fragments"))
}

/// The formats read here, as the messages name them.
pub const FORMATS: &str = "csv, md, xlsx, docx";

fn ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// Why a document's tables cannot be taken out of it here, when they cannot. The extension
/// alone answers, so `check` can say it without reading the document — which is the one thing
/// it does not do.
pub fn unreadable(path: &Path) -> Option<String> {
    match ext(path).as_str() {
        "csv" | "md" | "markdown" | "xlsx" | "docx" => None,
        "" => Some(tr!(
            "拡張子が無いので、どう読めばいいか分かりません（読めるのは {FORMATS}）",
            "the file has no extension, so there is no telling how to read it (the formats read here are {FORMATS})"
        )),
        other => Some(tr!(
            "`.{other}` からは表を取り出せません。読めるのは {FORMATS} です",
            "tables cannot be taken out of a `.{other}` here; the formats read are {FORMATS}"
        )),
    }
}

/// Every table of a document, in document order. The extension decides how it is read; a
/// format that needs an extractor of its own is an error that says so.
pub fn tables(path: &Path, bytes: &[u8]) -> Result<Vec<Vec<Vec<String>>>, String> {
    if let Some(why) = unreadable(path) {
        return Err(why);
    }
    match ext(path).as_str() {
        "csv" => {
            let g = grid(crate::import::parse_csv(&text_of(bytes)?));
            Ok(if g.is_empty() { Vec::new() } else { vec![g] })
        }
        "md" | "markdown" => Ok(markdown_tables(&text_of(bytes)?)),
        "xlsx" => Ok(crate::xlsx::grids(bytes)?.into_iter().map(|(_, g)| grid(g)).collect()),
        "docx" => Ok(crate::docx::tables(bytes)?.into_iter().map(grid).collect()),
        // `unreadable` refused every other extension above.
        _ => Ok(Vec::new()),
    }
}

/// The document as text. A copy that is not UTF-8 is not read: guessing an encoding here
/// would make the digest depend on the guess.
fn text_of(bytes: &[u8]) -> Result<String, String> {
    let s = std::str::from_utf8(bytes)
        .map_err(|_| tr!("文書が UTF-8 ではありません", "the document is not UTF-8"))?;
    Ok(s.strip_prefix('\u{feff}').unwrap_or(s).to_string())
}

/// A grid with its cells normalised and its empty trailing rows dropped.
fn grid(rows: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = rows.into_iter().map(|r| r.iter().map(|c| cell(c)).collect()).collect();
    while out.last().is_some_and(|r| r.iter().all(|c| c.is_empty())) {
        out.pop();
    }
    out
}

/// One cell as the copy holds it: a tab or a line break inside a cell becomes a space, so
/// that one row of the table is one line of the TSV and nothing needs escaping.
fn cell(s: &str) -> String {
    s.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).collect::<String>().trim().to_string()
}

/// The pipe tables of a Markdown document, in document order. A table is a row line whose
/// next line is the delimiter (`| --- | --- |`), and it runs to the first line without a bar.
fn markdown_tables(src: &str) -> Vec<Vec<Vec<String>>> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if is_row(lines[i]) && lines.get(i + 1).is_some_and(|l| is_delim(l)) {
            let mut rows = vec![split_row(lines[i])];
            let mut k = i + 2;
            while k < lines.len() && is_row(lines[k]) {
                rows.push(split_row(lines[k]));
                k += 1;
            }
            // The header decides the width: a row that ran short is filled, one that ran long
            // is kept whole, so that nothing the document says is dropped on the way in.
            let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            for r in &mut rows {
                r.resize(w, String::new());
            }
            out.push(grid(rows));
            i = k;
            continue;
        }
        i += 1;
    }
    out
}

/// A row line holds a bar. The leading and trailing bars are optional, as they are in
/// Markdown itself, and what tells a table from a line with a bar in it is the delimiter.
fn is_row(line: &str) -> bool {
    line.contains('|')
}

/// `| --- | :---: |`: the line under a table's header, and the only thing that tells a table
/// from a line that happens to have bars in it. It has to hold a bar too — otherwise the
/// `---` under a Setext heading would turn the line above it into a table.
fn is_delim(line: &str) -> bool {
    if !is_row(line) {
        return false;
    }
    let cells = split_row(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim_start_matches(':').trim_end_matches(':');
            !c.is_empty() && c.chars().all(|ch| ch == '-')
        })
}

/// One Markdown row into its cells. `\|` inside a cell is a bar, not a border.
fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for c in t.chars() {
        match c {
            '\\' if !escaped => escaped = true,
            '|' if !escaped => cells.push(std::mem::take(&mut cur)),
            _ => {
                if escaped && c != '|' {
                    cur.push('\\');
                }
                escaped = false;
                cur.push(c);
            }
        }
    }
    cells.push(cur);
    cells.into_iter().map(|c| cell(&c)).collect()
}

/// The copy of a fragment: one row per line, cells separated by tabs. Cells hold no tab and
/// no line break (`cell` saw to that), so nothing is escaped and a diff reads a row at a time.
pub fn tsv(grid: &[Vec<String>]) -> String {
    let mut o = String::new();
    for row in grid {
        o.push_str(&row.join("\t"));
        o.push('\n');
    }
    o
}

/// A copy read back.
pub fn from_tsv(src: &str) -> Vec<Vec<String>> {
    src.lines().map(|l| l.split('\t').map(|c| c.to_string()).collect()).collect()
}

/// A grid as a Markdown table, for the rendering the approver reads (`rulec doc`).
pub fn markdown(grid: &[Vec<String>]) -> String {
    let w = grid.iter().map(|r| r.len()).max().unwrap_or(0);
    if w == 0 {
        return String::new();
    }
    let row = |cells: &[String]| {
        let mut r: Vec<String> = cells.iter().map(|c| c.replace('|', "\\|")).collect();
        r.resize(w, String::new());
        format!("| {} |", r.join(" | "))
    };
    let mut o = String::new();
    let mut rows = grid.iter();
    if let Some(head) = rows.next() {
        o.push_str(&row(head));
        o.push('\n');
        o.push_str(&format!("|{}\n", "---|".repeat(w)));
    }
    for r in rows {
        o.push_str(&row(r));
        o.push('\n');
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_are_tables_by_ordinal() {
        assert_eq!(fragment("表3"), Some(3));
        assert_eq!(fragment("table3"), Some(3));
        assert_eq!(fragment("表"), None);
        assert_eq!(fragment("表0"), None);
        assert_eq!(fragment("第91条"), None);
        assert_eq!(fragment("別表第一"), None);
    }

    #[test]
    fn markdown_tables_in_order() {
        let src = "# 運賃\n\nよくある前書き。\n\n| あて先 | S60 |\n| --- | --- |\n| 近畿 | 990円 |\n| 関東 | 880円 |\n\n本文。\n\n| 区分 | 率 |\n|---|---|\n| 甲 | 5% |\n";
        let ts = markdown_tables(src);
        assert_eq!(ts.len(), 2);
        assert_eq!(ts[0], vec![vec!["あて先", "S60"], vec!["近畿", "990円"], vec!["関東", "880円"]]);
        assert_eq!(ts[1], vec![vec!["区分", "率"], vec!["甲", "5%"]]);
    }

    #[test]
    fn a_line_with_bars_is_not_a_table() {
        assert!(markdown_tables("| これは表ではない |\nただの行\n").is_empty());
        // The `---` under a Setext heading is not a delimiter row.
        assert!(markdown_tables("見出し | ではない\n---\n本文\n").is_empty());
    }

    #[test]
    fn the_outer_bars_are_optional() {
        let ts = markdown_tables("あて先 | 運賃\n--- | ---\n近畿 | 990円\n");
        assert_eq!(ts, vec![vec![vec!["あて先", "運賃"], vec!["近畿", "990円"]]]);
    }

    #[test]
    fn a_bar_inside_a_cell_survives() {
        let ts = markdown_tables("| a | b |\n| --- | --- |\n| x \\| y | z |\n");
        assert_eq!(ts[0][1], vec!["x | y", "z"]);
    }

    #[test]
    fn tsv_round_trips() {
        let g = vec![vec!["あて先".to_string(), "運賃".to_string()], vec!["近畿".to_string(), "990円".to_string()]];
        assert_eq!(from_tsv(&tsv(&g)), g);
    }
}

// --- What a copy says, as values (§15.82) -----------------------------------------------

/// Every number a copy shows, wherever it shows it — inside `990円（税込）` and `60cm以下` as
/// well as alone in a cell. This is the side a rule's amount is looked for in, so it is read
/// leniently: a value the copy does show must not be reported as missing.
pub fn shown(grid: &[Vec<String>]) -> Vec<(Option<String>, crate::num::Rat)> {
    let mut out = Vec::new();
    for row in grid {
        for c in row {
            // A thousands separator is dropped first: a document writes `1,210円` where the
            // rule writes `1210円`, and they are the same amount.
            let plain = c.replace(',', "");
            let mut i = 0;
            while i < plain.len() {
                // A continuation byte of a multi-byte character is never an ASCII digit, so
                // walking bytes finds exactly the places a number can start.
                if plain.as_bytes()[i].is_ascii_digit() {
                    if let Some((n, len)) = crate::lex::number(&plain[i..]) {
                        if let Some(v) = crate::types::comparable(&n) {
                            out.push(v);
                        }
                        i += len.max(1);
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    out
}

/// The numbers a copy states as whole cells: `990円` yes, `2026年4月1日改定` no. This is the
/// side the rule has to account for, so it is read strictly: a cell that is not simply a
/// number is not an amount somebody forgot to transcribe.
pub fn stated(grid: &[Vec<String>]) -> Vec<(String, (Option<String>, crate::num::Rat))> {
    let mut out = Vec::new();
    for row in grid {
        for c in row {
            let plain = c.replace(',', "");
            if !plain.starts_with(|ch: char| ch.is_ascii_digit()) {
                continue;
            }
            let Some((n, len)) = crate::lex::number(&plain) else { continue };
            if len != plain.len() {
                continue;
            }
            if let Some(v) = crate::types::comparable(&n) {
                out.push((c.clone(), v));
            }
        }
    }
    out
}

/// Which of the two bands a boundary number belongs to, as a document says it.
///
/// A threshold separates two bands, and the one thing a transcription can get wrong without
/// any other check noticing is **which band the boundary value itself falls in**: the copy
/// says `60cm以下` and the row says `<60cm`, the number 60 is used either way so W120 is
/// quiet, and no gap and no overlap appears, so E101 and E105 are quiet too. Only the
/// customer's page shows it, and only to a reader who compares two pages (§15.124).
///
/// Reading the side rather than the operator is what makes this comparable at all. A rule
/// may write a band from either end — `<=60cm` and `>60cm` are the two halves of one
/// boundary — and both say that 60 is in the band below. **Negating a comparison swaps its
/// direction and keeps its side**, so the side survives every rewriting a transcription
/// does, which the operator does not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The number is in the band below it: `60cm以下`, `60cmを超え`, `over £125,140`.
    Lower,
    /// The number is in the band above it: `60cm未満`, `18歳以上`, `21 and over`.
    Upper,
}

/// A boundary the copy states: the number, which side of it the document puts the number on,
/// and the words that said so.
pub struct Bound {
    /// The cell as the copy holds it. The message quotes it, because what a reader has to do
    /// with this diagnostic is read that cell again.
    pub cell: String,
    pub value: (Option<String>, crate::num::Rat),
    pub side: Side,
    pub word: &'static str,
    /// The bound was read from a heading over the number's column, not from its own cell.
    /// The message says so, because what the reader has to look at is a different cell.
    pub column: bool,
}

/// The words that stand **after** a number, and which side they put it on. Japanese writes
/// them all here.
///
/// A negation is not read and does not need to be: `60円以下` and `60円を超え` are the two
/// sides of one boundary and both leave 60 in the band below.
const AFTER: &[(&str, Side)] = &[
    ("以下", Side::Lower),
    ("まで", Side::Lower),
    ("以内", Side::Lower),
    ("を超え", Side::Lower),
    ("超", Side::Lower),
    ("より大きい", Side::Lower),
    ("未満", Side::Upper),
    ("に満たない", Side::Upper),
    ("より小さい", Side::Upper),
    ("以上", Side::Upper),
    ("or less", Side::Lower),
    ("or under", Side::Lower),
    ("and under", Side::Lower),
    ("or below", Side::Lower),
    ("and below", Side::Lower),
    ("or more", Side::Upper),
    ("and over", Side::Upper),
    ("or over", Side::Upper),
    ("and above", Side::Upper),
    ("or above", Side::Upper),
];

/// The words that stand **before** a number. English only, and a different list from the one
/// above for the same words: `over £125,140` starts the band past the number and `21 and
/// over` starts it at the number.
const BEFORE: &[(&str, Side)] = &[
    ("up to", Side::Lower),
    ("not over", Side::Lower),
    ("not more than", Side::Lower),
    ("no more than", Side::Lower),
    ("at most", Side::Lower),
    ("more than", Side::Lower),
    ("greater than", Side::Lower),
    ("exceeding", Side::Lower),
    ("over", Side::Lower),
    ("above", Side::Lower),
    ("not less than", Side::Upper),
    ("no less than", Side::Upper),
    ("at least", Side::Upper),
    ("less than", Side::Upper),
    ("under", Side::Upper),
    ("below", Side::Upper),
];

/// Every boundary a copy states. Read strictly on purpose: a number with no word of the two
/// lists next to it, or with words of both, yields nothing — `18 to 20` and `60〜80` say
/// which numbers bound a band but not which band holds them, and a guess here would be an
/// error against a rule that is right.
pub fn bounds(grid: &[Vec<String>]) -> Vec<Bound> {
    let mut out = Vec::new();
    let heads = headings(grid);
    for row in grid {
        for (ci, c) in row.iter().enumerate() {
            let plain = c.replace(',', "");
            // Where every number of the cell stands, so that the words next to one are never
            // read as the words next to the one after it: `Over $11,925 but not over $48,475`
            // has a bound on each, and each takes only the text up to its neighbour.
            let mut nums: Vec<(usize, usize, (Option<String>, crate::num::Rat))> = Vec::new();
            let mut i = 0;
            while i < plain.len() {
                if plain.as_bytes()[i].is_ascii_digit() {
                    if let Some((n, len)) = crate::lex::number(&plain[i..]) {
                        if let Some(v) = crate::types::comparable(&n) {
                            nums.push((i, i + digits_len(&plain[i..]), v));
                        }
                        i += len.max(1);
                        continue;
                    }
                }
                i += 1;
            }
            let col = heads.get(ci).and_then(|h| h.as_ref());
            for (k, (start, digits_end, v)) in nums.iter().enumerate() {
                // A currency written before the digits is the number's unit, and reading it
                // is what lines `$11,925` up with a rule that counts cents. Only the three
                // signs that name one currency each are read; `¥` is left out because a
                // Japanese document writes `円` after the number, where the lexer already
                // sees it, and reading a sign that names several currencies would be a guess.
                let v = &sigil(&plain[..*start]).map_or_else(|| v.clone(), |u| in_unit(u, &v.1));
                let from = if k == 0 { 0 } else { nums[k - 1].1 };
                let upto = nums.get(k + 1).map_or(plain.len(), |n| n.0);
                let said = [read_side(&plain[from..*start], BEFORE), read_side(&plain[*digits_end..upto], AFTER)];
                let mut found = None;
                for s in said.into_iter().flatten() {
                    match found {
                        None => found = Some(s),
                        Some(had) if had.0 == s.0 => {}
                        // A number the cell bounds both ways is not a boundary this can read.
                        Some(_) => {
                            found = None;
                            break;
                        }
                    }
                }
                match (found, col) {
                    (Some((side, word)), _) => out.push(Bound { cell: c.clone(), value: v.clone(), side, word, column: false }),
                    // A number whose own cell says nothing takes what the heading over its
                    // column says, when there is one.
                    (None, Some((side, word, head))) => {
                        out.push(Bound { cell: head.clone(), value: v.clone(), side: *side, word, column: true })
                    }
                    (None, None) => {}
                }
            }
        }
    }
    out
}

/// The currency a sign right before the digits names, when it names exactly one.
fn sigil(before: &str) -> Option<&'static str> {
    match before.chars().next_back()? {
        '$' => Some("USD"),
        '£' => Some("GBP"),
        '€' => Some("EUR"),
        _ => None,
    }
}

/// A magnitude read back as an amount in `unit`, the way a rule that wrote the same number
/// with that unit would be read — so that a document's `$11,925` and a rule's `1192500USDc`
/// are one value. One of the unit is read for its scale, rather than the table of factors
/// being reached into: the conversion stays in the one place that owns it (§2.1).
fn in_unit(unit: &str, v: &crate::num::Rat) -> (Option<String>, crate::num::Rat) {
    let one = crate::lex::Num {
        neg: false,
        digits: "1".into(),
        frac: String::new(),
        mult: 1,
        unit: Some(unit.into()),
        raw: String::new(),
    };
    match crate::types::comparable(&one) {
        Some((dim, f)) => (dim, v.mul(f)),
        None => (None, *v),
    }
}

/// The bound each column of a copy is headed with, where it is headed with one.
///
/// A Japanese premium or tax table is written this way — `円以上` and `円未満` stand over two
/// columns and the numbers underneath are bare — and reading only the cell a number sits in
/// sees nothing at all in it. The pension table of the corpus is 31 boundaries of exactly
/// this shape (§15.124).
///
/// A heading counts only when it is **a bounding word and a unit and nothing else**, so that
/// `円以上` is read and `超過額` is not: the word is taken out and what is left has to be a
/// unit the tool knows, or nothing. A column headed two ways is headed no way.
fn headings(grid: &[Vec<String>]) -> Vec<Option<(Side, &'static str, String)>> {
    let width = grid.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut out = vec![None; width];
    let mut clash = vec![false; width];
    for row in grid {
        for (i, c) in row.iter().enumerate() {
            if c.chars().any(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let Some((side, word)) = read_side(c, AFTER).or_else(|| read_side(c, BEFORE)) else { continue };
            if !only_unit_left(c, word) {
                continue;
            }
            match out[i] {
                _ if clash[i] => {}
                None => out[i] = Some((side, word, c.clone())),
                Some((had, _, _)) if had == side => {}
                Some(_) => {
                    clash[i] = true;
                    out[i] = None;
                }
            }
        }
    }
    out
}

/// Whether a heading is the bounding word and a unit and nothing else.
fn only_unit_left(cell: &str, word: &str) -> bool {
    let rest: String = cell
        .to_lowercase()
        .replace(word, "")
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '(' | ')' | '（' | '）' | '・' | '.'))
        .collect();
    if rest.is_empty() {
        return true;
    }
    // A unit is known when a number written in it can be read as a value.
    in_unit(&rest, &crate::num::Rat::int(1)).0.is_some()
}

/// The side one stretch of text next to a number says, and the word that said it. Text that
/// holds words of both sides says nothing.
fn read_side(text: &str, words: &[(&'static str, Side)]) -> Option<(Side, &'static str)> {
    let t = text.to_lowercase();
    let mut found: Option<(Side, &'static str)> = None;
    for (w, s) in words {
        if !holds(&t, w) {
            continue;
        }
        match found {
            None => found = Some((*s, w)),
            Some((had, _)) if had == *s => {}
            Some(_) => return None,
        }
    }
    found
}

/// Whether a stretch of text holds a bounding word. An English word has to stand on its own —
/// `over` is in `cover charge` and in `however`, and neither bounds anything — while a
/// Japanese one is looked for as it is, there being no letter either side of it to check.
fn holds(text: &str, word: &str) -> bool {
    if !word.is_ascii() {
        return text.contains(word);
    }
    let edge = |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric());
    text.match_indices(word).any(|(i, _)| {
        edge(text[..i].chars().next_back()) && edge(text[i + word.len()..].chars().next())
    })
}

/// How many bytes at the front of `s` are the number's own digits, its unit left out. The
/// word that bounds a number stands after the unit as often as after the digits (`60cm以下`,
/// but `18歳未満`, where `歳` is no unit the lexer knows and `1万円以下`, where the
/// multiplier sits in between), so the text read for it starts where the digits end.
fn digits_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' && b.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
        i += 1;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
            i += 1;
        }
    }
    i
}

// --- An extractor that is not this program (§15.82) --------------------------------------

/// Run an extractor over a document and read the tables it hands back.
///
/// ```text
/// rulec → docling-rulec 料金表.pdf
///       ← {"rulec":"extract/1","impl":"docling 2.4.0"}
///       ← {"block":"table","page":12,"grid":[["あて先","運賃"],["近畿","990円"]]}
///       ← {"done":true}
/// ```
///
/// One direction and one shot, which is all an extraction is: the document's path is an
/// argument, the blocks come back as JSON Lines on stdout, and the first line names the
/// extractor. That name is written beside the copies, because **who read the document is part
/// of what the copy is**: a table a model read out of a scan is evidence of a different kind
/// from a table that was already a grid.
///
/// The stream has to end with `done`. An extractor that dies half way would otherwise hand
/// back the tables it managed, and `表3` would quietly be a different table.
pub fn via(cmd: &[String], doc: &Path) -> Result<(String, Vec<(Option<i64>, Vec<Vec<String>>)>), String> {
    use std::io::BufRead;
    let (bin, args) = cmd.split_first().ok_or_else(|| tr!("抽出器のコマンドがありません", "No extractor command was given"))?;
    let mut child = std::process::Command::new(bin)
        .args(args)
        .arg(doc)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| tr!("抽出器を起動できません: {e}", "Cannot start the extractor: {e}"))?;
    let so = child.stdout.take().ok_or_else(|| tr!("stdout を掴めません", "Cannot open the extractor's stdout"))?;

    let mut impl_id = String::new();
    let mut tables: Vec<(Option<i64>, Vec<Vec<String>>)> = Vec::new();
    let mut done = false;
    let mut bad: Option<String> = None;
    for line in std::io::BufReader::new(so).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let j = match crate::json::parse(&line) {
            Ok(j) => j,
            Err(e) => {
                bad = Some(tr!("抽出器の出した行が JSON ではありません: {e}", "A line from the extractor is not JSON: {e}"));
                break;
            }
        };
        if impl_id.is_empty() {
            match (j.get("rulec").and_then(|v| v.as_str()), j.get("impl").and_then(|v| v.as_str())) {
                (Some("extract/1"), Some(i)) if !i.is_empty() => impl_id = i.to_string(),
                _ => {
                    bad = Some(tr!(
                        "抽出器が最初の行で名乗っていません（`{{\"rulec\":\"extract/1\",\"impl\":\"…\"}}` が要ります）: {}",
                        "The extractor did not name itself on its first line (`{{\"rulec\":\"extract/1\",\"impl\":\"…\"}}` is required): {}",
                        line.trim()
                    ));
                    break;
                }
            }
            continue;
        }
        if j.get("done").is_some() {
            done = true;
            break;
        }
        // Blocks other than tables are read and let go: a fragment is a table, and an
        // extractor that also says where the headings are should not have to know that.
        if j.get("block").and_then(|v| v.as_str()) != Some("table") {
            continue;
        }
        let Some(crate::json::Json::Arr(rows)) = j.get("grid") else {
            bad = Some(tr!("表の `grid` がありません: {}", "A table block has no `grid`: {}", line.trim()));
            break;
        };
        let grid: Vec<Vec<String>> = rows
            .iter()
            .map(|r| match r {
                crate::json::Json::Arr(cs) => cs.iter().map(|c| cell(c.as_str().unwrap_or(""))).collect(),
                _ => Vec::new(),
            })
            .collect();
        tables.push((j.get("page").and_then(|v| v.as_int()).map(|n| n as i64), grid));
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if let Some(why) = bad {
        return Err(why);
    }
    if !status.success() {
        return Err(tr!(
            "抽出器が失敗しました（終了コード {}）",
            "The extractor failed (exit code {})",
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".into())
        ));
    }
    if impl_id.is_empty() || !done {
        return Err(tr!(
            "抽出器が最後まで出していません（`{{\"done\":true}}` で終わります）。途中までの表を写しにすると、`表3` が別の表になります",
            "The extractor did not finish (the stream ends with `{{\"done\":true}}`). Copying what arrived would make `表3` a different table"
        ));
    }
    Ok((impl_id, tables))
}

/// The template `rulec adapter --template docling` prints: a document in, blocks out. The
/// rule is read for the line that runs it — which documents of this rule need an extractor at
/// all is a question only the rule can answer.
pub fn template(f: &crate::ast::RuleFile, rule_path: &str) -> String {
    let docs: Vec<String> = f
        .sources
        .iter()
        .filter_map(|d| match &d.kind {
            crate::ast::SourceKind::File { path, .. } if unreadable(Path::new(path)).is_some() => Some(path.clone()),
            _ => None,
        })
        .collect();
    let which = if docs.is_empty() {
        tr!(
            "# この規則には、自分で読めない形式の出典はいまのところ無い（読めるのは {FORMATS}）。\n",
            "# No source of this rule is in a format this program cannot read (it reads {FORMATS}).\n"
        )
    } else {
        tr!("# この規則が引いていて、抽出器が要る文書: {}\n", "# The documents of this rule that need an extractor: {}\n", docs.join(", "))
    };
    which + &body(rule_path)
}

fn body(rule_path: &str) -> String {
    tr!(
        "#!/usr/bin/env python3\n\
         # rulec の抽出アダプタのテンプレート（extract/1）。\n\
         # 文書のパスを引数で受け取り、表を JSON Lines で標準出力に書くだけ。\n\
         # 使い方: rulec source fetch {rule_path} --via ./extract.py\n\
         import json, sys\n\n\
         path = sys.argv[1]\n\n\
         # ここを好きな抽出器に差し替える（docling・marker・MinerU・クラウドの API など）。\n\
         # 名乗りはそのまま写しの隣に残るので、版まで書く。\n\
         from docling.document_converter import DocumentConverter  # type: ignore\n\
         import docling  # type: ignore\n\n\
         print(json.dumps({{\"rulec\": \"extract/1\", \"impl\": f\"docling {{docling.__version__}}\"}}), flush=True)\n\n\
         doc = DocumentConverter().convert(path).document\n\
         for t in doc.tables:\n    \
         df = t.export_to_dataframe()\n    \
         grid = [[str(c) for c in df.columns]] + [[str(c) for c in row] for row in df.values.tolist()]\n    \
         page = (t.prov[0].page_no if t.prov else None)\n    \
         print(json.dumps({{\"block\": \"table\", \"page\": page, \"grid\": grid}}, ensure_ascii=False), flush=True)\n\n\
         print(json.dumps({{\"done\": True}}), flush=True)\n",
        "#!/usr/bin/env python3\n\
         # A rulec extraction adapter (extract/1).\n\
         # It takes the document's path as an argument and writes its tables to stdout as JSON Lines.\n\
         # Use it with: rulec source fetch {rule_path} --via ./extract.py\n\
         import json, sys\n\n\
         path = sys.argv[1]\n\n\
         # Swap in whichever extractor you use (docling, marker, MinerU, a cloud API).\n\
         # The name stays beside the copies, so give the version too.\n\
         from docling.document_converter import DocumentConverter  # type: ignore\n\
         import docling  # type: ignore\n\n\
         print(json.dumps({{\"rulec\": \"extract/1\", \"impl\": f\"docling {{docling.__version__}}\"}}), flush=True)\n\n\
         doc = DocumentConverter().convert(path).document\n\
         for t in doc.tables:\n    \
         df = t.export_to_dataframe()\n    \
         grid = [[str(c) for c in df.columns]] + [[str(c) for c in row] for row in df.values.tolist()]\n    \
         page = (t.prov[0].page_no if t.prov else None)\n    \
         print(json.dumps({{\"block\": \"table\", \"page\": page, \"grid\": grid}}, ensure_ascii=False), flush=True)\n\n\
         print(json.dumps({{\"done\": True}}), flush=True)\n"
    )
}
