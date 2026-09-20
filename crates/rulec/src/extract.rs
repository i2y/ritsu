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
