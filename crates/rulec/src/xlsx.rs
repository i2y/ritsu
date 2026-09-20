//! Reading a spreadsheet's `.xlsx` (§1.4, §15.50): the sheet as a grid of text, which is
//! what `import` already knows how to draft from.
//!
//! One direction only, and a draft rather than a rule — that decision is §1.4's and this
//! module does not widen it. What it adds over `import csv` is that **no export step comes
//! first**: the file the business sends is the file that is read.
//!
//! The container it arrives in — a ZIP directory, DEFLATE and enough XML — is `ooxml`, which
//! a `.docx` reads through as well.
//!
//! What a cell *means* is where a spreadsheet differs from a CSV, and it is why this is not
//! a line of shell: a date is a number, a percentage is a hundredth, and the unit of a
//! money column lives in the number format and not in the cell. Those three are read from
//! the styles; everything else is taken as the text it is.

use crate::ooxml::{attr, entries, part, unescape, walk, Entry, Node};
use std::collections::BTreeMap;

// ---------------------------------------------------------------- what a cell means

/// What a number format does to the value stored in the cell. Three kinds matter; for
/// everything else the digits are taken as they are written.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Fmt {
    Plain,
    Date,
    Percent,
}

/// The built-in format ids that mean a date. 27–36 and 50–58 are the Japanese, Chinese and
/// Korean calendars' date formats, which is exactly where a tariff with dates turns up.
fn builtin(id: usize) -> (Fmt, String) {
    match id {
        14..=17 | 22 | 27..=36 | 50..=58 => (Fmt::Date, String::new()),
        9 | 10 => (Fmt::Percent, String::new()),
        _ => (Fmt::Plain, String::new()),
    }
}

/// Read a format code: what it does to the value, and the unit it writes after the number.
///
/// The unit is only taken when the workbook *states* it — a literal in quotes (`#,##0"円"`)
/// or a yen sign. A `$` is left alone on purpose: it is USD, CAD, AUD and several more, and
/// this importer does not decide which (§1.4 — it never decides an amount either).
fn read_format(code: &str) -> (Fmt, String) {
    let mut lit = String::new();
    let mut yen = false;
    let (mut date, mut percent) = (false, false);
    let (mut quoted, mut bracket, mut escape) = (false, false, false);
    for c in code.chars() {
        if escape {
            escape = false;
            lit.push(c);
            continue;
        }
        if quoted {
            if c == '"' {
                quoted = false;
            } else {
                lit.push(c);
            }
            continue;
        }
        if bracket {
            if c == ']' {
                bracket = false;
            } else if c == '¥' || c == '￥' {
                yen = true;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '\\' => escape = true,
            '[' => bracket = true,
            ';' => break, // the first section is the one positive numbers take
            '%' => percent = true,
            '¥' | '￥' => yen = true,
            // `m` is month *and* minute, and `e` is the exponent of a scientific format,
            // so the day and the year are what a date is recognised by. Every era format
            // (`[$-411]ggge"年"m"月"d"日"`) carries a `d` too.
            'y' | 'Y' | 'd' | 'D' => date = true,
            _ => {}
        }
    }
    if percent {
        return (Fmt::Percent, String::new());
    }
    if date {
        return (Fmt::Date, String::new());
    }
    let lit = lit.trim().to_string();
    if !lit.is_empty() && lit.chars().all(|c| c.is_alphanumeric()) {
        return (Fmt::Plain, lit);
    }
    (Fmt::Plain, if yen { "円".into() } else { String::new() })
}

/// The civil date `days` after 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A date cell's serial number as `YYYY-MM-DD`.
///
/// 1900-02-29 never existed; the serial 60 is a day Excel invented and kept for
/// compatibility with an older program. It is left as the number it is rather than turned
/// into a date that was never on a calendar.
fn serial_date(v: &str, date1904: bool) -> Option<String> {
    let n: i64 = v.split('.').next()?.parse().ok()?;
    let days = if date1904 {
        n - 24107
    } else if n >= 61 {
        n - 25569
    } else if (1..=59).contains(&n) {
        n - 25568
    } else {
        return None;
    };
    let (y, m, d) = civil(days);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// A decimal written as text, times a hundred, without going through a float: a percentage
/// is stored as its fraction and 0.1% steps are exactly what this importer has to keep.
fn times100(v: &str) -> Option<String> {
    let (sign, body) = match v.strip_prefix('-') {
        Some(b) => ("-", b),
        None => ("", v),
    };
    if !body.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    let digits = format!("{int}{frac:0<2}");
    let point = int.len() + 2;
    let (mut i, f) = digits.split_at(point.min(digits.len()));
    i = i.trim_start_matches('0');
    let f = f.trim_end_matches('0');
    let i = if i.is_empty() { "0" } else { i };
    Some(if f.is_empty() { format!("{sign}{i}") } else { format!("{sign}{i}.{f}") })
}

// ---------------------------------------------------------------- the workbook

/// The workbook's strings. A cell of text holds an index into this table, not the text.
///
/// `rPh` is the reading (furigana) Excel keeps beside a Japanese string; it is part of the
/// entry and not part of the value, so it is skipped rather than concatenated.
fn shared_strings(xml: &str) -> Vec<String> {
    let (mut out, mut cur) = (Vec::new(), String::new());
    let (mut in_si, mut in_ph, mut in_t) = (false, false, false);
    walk(xml, |n| match n {
        Node::Open { name: "si", .. } => {
            in_si = true;
            cur.clear();
        }
        Node::Open { name: "rPh", .. } => in_ph = true,
        Node::Open { name: "t", empty, .. } if in_si && !in_ph && !empty => in_t = true,
        Node::Text(t) if in_t => cur.push_str(&unescape(t)),
        Node::Close("t") => in_t = false,
        Node::Close("rPh") => in_ph = false,
        Node::Close("si") => {
            in_si = false;
            out.push(std::mem::take(&mut cur));
        }
        _ => {}
    });
    out
}

/// `cellXfs` in order: what each style index does to a number. `cellStyleXfs` holds `xf`
/// elements too and comes first, which is why the walk waits for the right parent.
fn read_styles(xml: &str) -> Vec<(Fmt, String)> {
    let mut custom: BTreeMap<usize, String> = BTreeMap::new();
    let mut ids: Vec<usize> = Vec::new();
    let mut in_cell_xfs = false;
    walk(xml, |n| match n {
        Node::Open { name: "numFmt", attrs, .. } => {
            if let (Some(id), Some(code)) = (attr(attrs, "numFmtId"), attr(attrs, "formatCode")) {
                if let Ok(id) = id.parse::<usize>() {
                    custom.insert(id, code);
                }
            }
        }
        Node::Open { name: "cellXfs", .. } => in_cell_xfs = true,
        Node::Open { name: "xf", attrs, .. } if in_cell_xfs => {
            ids.push(attr(attrs, "numFmtId").and_then(|s| s.parse().ok()).unwrap_or(0));
        }
        Node::Close("cellXfs") => in_cell_xfs = false,
        _ => {}
    });
    ids.iter()
        .map(|id| match custom.get(id) {
            Some(code) => read_format(code),
            None => builtin(*id),
        })
        .collect()
}

/// `rId3` → the part it names, as a path in the package.
fn relations(xml: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk(xml, |n| {
        if let Node::Open { name: "Relationship", attrs, .. } = n {
            if let (Some(id), Some(target)) = (attr(attrs, "Id"), attr(attrs, "Target")) {
                let target = match target.strip_prefix('/') {
                    Some(abs) => abs.to_string(),
                    None => format!("xl/{}", target.trim_start_matches("./")),
                };
                out.insert(id, target);
            }
        }
    });
    out
}

/// The letters of a cell reference as a 1-based column: `A` is 1, `AB` is 28.
fn ref_col(r: &str) -> Option<usize> {
    let mut n = 0usize;
    for c in r.chars().take_while(|c| c.is_ascii_alphabetic()) {
        n = n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    (n > 0).then_some(n)
}

/// A workbook read once: what every sheet needs to be read, and the sheets in the order the
/// workbook lists them. One read serves a whole book (§15.82), since a document's fragments
/// are its sheets and the shared strings are read once for all of them.
struct Book {
    es: Vec<Entry>,
    shared: Vec<String>,
    styles: Vec<(Fmt, String)>,
    sheets: Vec<(String, String)>,
    date1904: bool,
}

fn book(bytes: &[u8]) -> Result<Book, String> {
    let es = entries(bytes)?;
    let wb = part(bytes, &es, "xl/workbook.xml")?.ok_or_else(|| {
        tr!("xlsx ではありません（xl/workbook.xml がありません）", "not an xlsx (there is no xl/workbook.xml)")
    })?;
    let rels = relations(&part(bytes, &es, "xl/_rels/workbook.xml.rels")?.unwrap_or_default());
    let shared = shared_strings(&part(bytes, &es, "xl/sharedStrings.xml")?.unwrap_or_default());
    let styles = read_styles(&part(bytes, &es, "xl/styles.xml")?.unwrap_or_default());

    let mut sheets: Vec<(String, String)> = Vec::new();
    let mut date1904 = false;
    walk(&wb, |n| {
        if let Node::Open { name, attrs, .. } = n {
            match name {
                "workbookPr" => {
                    let v = attr(attrs, "date1904").unwrap_or_default();
                    date1904 = v == "1" || v == "true";
                }
                "sheet" => {
                    if let (Some(name), Some(id)) = (attr(attrs, "name"), attr(attrs, "id")) {
                        if let Some(target) = rels.get(&id) {
                            sheets.push((name, target.clone()));
                        }
                    }
                }
                _ => {}
            }
        }
    });
    if sheets.is_empty() {
        return Err(tr!("xlsx にシートがありません", "the xlsx has no sheets"));
    }
    Ok(Book { es, shared, styles, sheets, date1904 })
}

/// Every sheet of a workbook, in the order the workbook lists them: the name and the grid. An
/// empty sheet comes back as an empty grid, so that a sheet's position is what it looks like
/// in the book — which is what a citation of `表3` counts (§15.82).
pub fn grids(bytes: &[u8]) -> Result<Vec<(String, Vec<Vec<String>>)>, String> {
    let b = book(bytes)?;
    b.sheets.iter().map(|(n, t)| Ok((n.clone(), sheet(bytes, &b, t)?))).collect()
}

/// One sheet as a grid of text: the sheet's name, and its rows.
///
/// Rows and columns that hold nothing at all are left out, so a table that starts at C4 is
/// read as a table and not as two empty columns and three empty rows. What is left is
/// rectangular, and `import`'s first row is its header, exactly as with a CSV.
pub fn grid(bytes: &[u8], want: Option<&str>) -> Result<(String, Vec<Vec<String>>), String> {
    let b = book(bytes)?;
    let (name, target) = match want {
        None => b.sheets[0].clone(),
        Some(w) => b.sheets.iter().find(|(n, _)| n == w).cloned().ok_or_else(|| {
            let all: Vec<&str> = b.sheets.iter().map(|(n, _)| n.as_str()).collect();
            tr!(
                "`{w}` というシートはありません。あるのは: {}",
                "there is no sheet called `{w}`; the workbook has: {}",
                all.join(", ")
            )
        })?,
    };
    let g = sheet(bytes, &b, &target)?;
    if g.is_empty() {
        return Err(tr!("シート `{name}` は空です", "the sheet `{name}` is empty"));
    }
    Ok((name, g))
}

fn sheet(bytes: &[u8], b: &Book, target: &str) -> Result<Vec<Vec<String>>, String> {
    let (shared, styles, date1904) = (&b.shared, &b.styles, b.date1904);
    let xml = part(bytes, &b.es, target)?
        .ok_or_else(|| tr!("シート {target} が xlsx にありません", "the sheet {target} is not in the xlsx"))?;

    // Only cells that hold something are kept, so an empty row or column simply never
    // appears; `number` is where a spreadsheet stops being a CSV (dates, percentages, the
    // unit in the format).
    let number = |v: &str, style: usize| -> String {
        let (fmt, unit) = styles.get(style).cloned().unwrap_or((Fmt::Plain, String::new()));
        match fmt {
            Fmt::Date => serial_date(v, date1904).unwrap_or_else(|| v.to_string()),
            Fmt::Percent => times100(v).map(|n| format!("{n}%")).unwrap_or_else(|| v.to_string()),
            Fmt::Plain => format!("{v}{unit}"),
        }
    };

    let mut cells: BTreeMap<(usize, usize), String> = BTreeMap::new();
    let (mut row, mut col, mut style) = (0usize, 0usize, 0usize);
    let (mut ty, mut buf) = (String::new(), String::new());
    let mut taking = false;
    walk(&xml, |n| match n {
        Node::Open { name: "row", attrs, .. } => {
            row = attr(attrs, "r").and_then(|r| r.parse().ok()).unwrap_or(row + 1);
            col = 0;
        }
        Node::Open { name: "c", attrs, .. } => {
            col = attr(attrs, "r").and_then(|r| ref_col(&r)).unwrap_or(col + 1);
            ty = attr(attrs, "t").unwrap_or_default();
            style = attr(attrs, "s").and_then(|s| s.parse().ok()).unwrap_or(0);
            buf.clear();
        }
        // The cached value, and the text of an inline string. A formula's own text sits in
        // `<f>` and is not read: what the sheet *shows* is the value.
        Node::Open { name: "v" | "t", empty, .. } if !empty => taking = true,
        Node::Text(t) if taking => buf.push_str(&unescape(t)),
        Node::Close("v" | "t") => taking = false,
        Node::Close("c") => {
            let text = match ty.as_str() {
                "s" => buf.trim().parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                "inlineStr" | "str" | "e" => buf.clone(),
                "b" => if buf.trim() == "1" { "TRUE".into() } else { "FALSE".into() },
                _ => number(buf.trim(), style),
            };
            if !text.trim().is_empty() {
                cells.insert((row, col), text);
            }
        }
        _ => {}
    });
    if cells.is_empty() {
        return Ok(Vec::new());
    }

    let mut cols: Vec<usize> = cells.keys().map(|(_, c)| *c).collect();
    cols.sort_unstable();
    cols.dedup();
    let mut rows: Vec<usize> = cells.keys().map(|(r, _)| *r).collect();
    rows.sort_unstable();
    rows.dedup();
    Ok(rows
        .iter()
        .map(|r| cols.iter().map(|c| cells.get(&(*r, *c)).cloned().unwrap_or_default()).collect())
        .collect())
}
