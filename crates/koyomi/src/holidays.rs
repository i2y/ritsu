//! Reading a table of holidays (DESIGN 1.5): a CSV of `<date>,<name>` lines in UTF-8 or
//! Shift_JIS, or GOV.UK's `bank-holidays.json`. What cannot be read is said with its place
//! (E104); nothing is skipped quietly.

use crate::date::{self, Day};
use ritsu_base::text::Text;
use serde_json::Value;
use std::collections::HashMap;

/// One closed day of a table, and the line of the copy it came from (0 for JSON).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub day: Day,
    pub name: String,
    pub line: usize,
}

/// Why a copy could not be read.
#[derive(Clone, Debug)]
pub struct ReadError {
    /// The line of the copy (1-based), when there is one.
    pub line: Option<usize>,
    pub why: Text,
}

fn at(line: usize, why: Text) -> ReadError {
    ReadError { line: Some(line), why }
}

/// The 1-based line a byte offset is on. In Shift_JIS a newline byte is never the second byte
/// of a character, so counting the bytes is enough.
fn line_of(bytes: &[u8], offset: usize) -> usize {
    bytes[..offset].iter().filter(|b| **b == b'\n').count() + 1
}

/// The values of one CSV line; a value may be quoted, with `""` for a quote inside.
fn fields(line: &str) -> Result<Vec<String>, Text> {
    let mut out = Vec::new();
    let cs: Vec<char> = line.chars().collect();
    let mut i = 0;
    loop {
        let mut v = String::new();
        if cs.get(i) == Some(&'"') {
            i += 1;
            loop {
                match cs.get(i) {
                    None => return Err(tr!("`\"` で始めた値が閉じていません", "a quoted value is not closed")),
                    Some('"') if cs.get(i + 1) == Some(&'"') => {
                        v.push('"');
                        i += 2;
                    }
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some(c) => {
                        v.push(*c);
                        i += 1;
                    }
                }
            }
            match cs.get(i) {
                None => {
                    out.push(v);
                    return Ok(out);
                }
                Some(',') => {
                    i += 1;
                    out.push(v);
                }
                Some(_) => return Err(tr!("`\"…\"` の値のあとに `,` 以外の文字があります", "a quoted value is followed by something other than `,`")),
            }
        } else {
            while let Some(c) = cs.get(i) {
                if *c == ',' {
                    break;
                }
                v.push(*c);
                i += 1;
            }
            out.push(v.trim().to_string());
            if i >= cs.len() {
                return Ok(out);
            }
            i += 1;
        }
    }
}

/// A CSV of `<date>,<name>` lines (DESIGN 1.5). The first line is a heading when its first
/// value is not a date.
pub fn read_csv(bytes: &[u8], shift_jis: bool) -> Result<Vec<Row>, ReadError> {
    let text = if shift_jis {
        crate::sjis::decode(bytes).map_err(|b| {
            let hex: Vec<String> = b.bytes.iter().map(|x| format!("0x{x:02X}")).collect();
            let hex = hex.join(" ");
            at(
                line_of(bytes, b.offset),
                tr!(
                    "{} バイト目の {hex} は Shift_JIS として読めません",
                    "the bytes {hex} at byte {} are not Shift_JIS",
                    b.offset + 1
                ),
            )
        })?
    } else {
        let body = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        match std::str::from_utf8(body) {
            Ok(s) => s.to_string(),
            Err(e) => {
                let off = e.valid_up_to() + (bytes.len() - body.len());
                let nth = off + 1;
                return Err(at(
                    line_of(bytes, off),
                    tr!(
                        "{nth} バイト目から UTF-8 として読めません。Shift_JIS なら `format csv shift_jis` と書いてください",
                        "the bytes from byte {nth} on are not UTF-8; for Shift_JIS, write `format csv shift_jis`"
                    ),
                ));
            }
        }
    };
    let mut rows = Vec::new();
    let mut seen: HashMap<Day, usize> = HashMap::new();
    let mut first = true;
    for (i, raw) in text.split('\n').enumerate() {
        let no = i + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() {
            continue;
        }
        let vals = fields(line).map_err(|why| at(no, why))?;
        let head = vals[0].trim();
        let Some(day) = date::parse_table_date(head) else {
            if first {
                first = false;
                continue;
            }
            return Err(at(no, tr!(
                "最初の値 `{head}` が日付として読めません（`2026-01-01` か `2026/1/1` の形）",
                "the first value `{head}` is not a date (`2026-01-01` or `2026/1/1`)"
            )));
        };
        first = false;
        if vals.len() > 2 {
            return Err(at(no, tr!(
                "値が {} 個あります。一行は `<日付>,<名前>` です",
                "the line has {} values; a line is `<date>,<name>`",
                vals.len()
            )));
        }
        if let Some(prev) = seen.get(&day) {
            return Err(at(no, tr!("{day} が {prev} 行目にもあります", "{day} is on line {prev} as well")));
        }
        seen.insert(day, no);
        rows.push(Row { day, name: vals.get(1).map(|s| s.trim().to_string()).unwrap_or_default(), line: no });
    }
    rows.sort_by_key(|r| r.day);
    Ok(rows)
}

/// The divisions of GOV.UK's file, in its order.
pub const GOVUK_DIVISIONS: [&str; 3] = ["england-and-wales", "scotland", "northern-ireland"];

/// GOV.UK's `bank-holidays.json`, one division of it. A day's name is its title, with the
/// notes in parentheses when there are any (`Boxing Day (Substitute day)`).
pub fn read_govuk(bytes: &[u8], division: &str) -> Result<Vec<Row>, ReadError> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| ReadError {
        line: Some(e.line()),
        why: tr!("JSON として読めません: {e}", "it is not JSON: {e}"),
    })?;
    let Some(div) = v.get(division) else {
        let have: Vec<String> = v.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        let have = have.join(", ");
        return Err(ReadError { line: None, why: tr!("地域「{division}」がありません（あるのは {have}）", "it has no division `{division}` (it has {have})") });
    };
    let Some(events) = div.get("events").and_then(|e| e.as_array()) else {
        return Err(ReadError { line: None, why: tr!("地域「{division}」に `events` の並びがありません", "the division `{division}` has no `events` list") });
    };
    let mut rows = Vec::new();
    let mut seen: HashMap<Day, usize> = HashMap::new();
    for (i, e) in events.iter().enumerate() {
        let k = i + 1;
        let ds = e.get("date").and_then(|d| d.as_str()).unwrap_or("");
        let Some(day) = date::parse(ds) else {
            return Err(ReadError { line: None, why: tr!("{k} 件目の `date` `{ds}` が日付として読めません", "event {k} has `date` `{ds}`, which is not a date") });
        };
        if seen.contains_key(&day) {
            return Err(ReadError { line: None, why: tr!("{day} が二度あります", "{day} is listed twice") });
        }
        seen.insert(day, k);
        let title = e.get("title").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
        let notes = e.get("notes").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
        let name = if notes.is_empty() { title } else { format!("{title} ({notes})") };
        rows.push(Row { day, name, line: 0 });
    }
    rows.sort_by_key(|r| r.day);
    Ok(rows)
}
