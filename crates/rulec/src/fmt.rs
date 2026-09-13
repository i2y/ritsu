//! `rulec fmt` — the one and only formatter (§1.5).
//!
//! Columns are aligned to the widest content of each table, counting East Asian Width W and F
//! as two columns. Full-width digits are normalized to ASCII, and the comparison signs
//! ≦ ≧ ＜ ＞ to their ASCII forms. Formatting is confined to the table, so the diff of a
//! realignment is confined to the table too.

use crate::diag::width;

/// Normalizes the spellings that need an IME or a full-width keyboard: full-width digits,
/// `≦ ≧ ＜ ＞ ＋`, the output marker `→` (written `->`), and the set separators `・ 、 ，`
/// (written `, `). String literals and `#` comments are left alone.
fn normalize(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_str = false;
    let mut skip_ws = false;
    for (i, c) in line.char_indices() {
        if in_str {
            out.push(c);
            if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c == '"' {
            in_str = true;
            skip_ws = false;
            out.push(c);
            continue;
        }
        if c == '#' {
            // A comment keeps its prose arrows and punctuation as written.
            out.push_str(&line[i..]);
            break;
        }
        if skip_ws && c.is_whitespace() {
            continue;
        }
        skip_ws = false;
        match c {
            '０'..='９' => out.push((b'0' + (c as u32 - '０' as u32) as u8) as char),
            '≦' => out.push_str("<="),
            '≧' => out.push_str(">="),
            '＜' => out.push('<'),
            '＞' => out.push('>'),
            '＋' => out.push('+'),
            '　' => out.push(' '),
            '→' => out.push_str("->"),
            '・' | '、' | '，' => {
                let kept = out.trim_end().len();
                out.truncate(kept);
                out.push_str(", ");
                skip_ws = true;
            }
            _ => out.push(c),
        }
    }
    out
}

fn is_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// Splits off the trailing comment. In a table row, only what follows the last `|` counts.
fn split_comment(line: &str) -> (String, Option<String>) {
    let Some(bar) = line.rfind('|') else {
        return (line.to_string(), None);
    };
    let tail = &line[bar + 1..];
    match tail.find('#') {
        Some(h) => (
            line[..bar + 1 + h].trim_end().to_string(),
            Some(tail[h..].trim_end().to_string()),
        ),
        None => (line.trim_end().to_string(), None),
    }
}

fn cells(row: &str) -> Vec<String> {
    let t = row.trim();
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

pub fn format(src: &str) -> String {
    let lines: Vec<String> = src.lines().map(normalize).collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if !is_row(&lines[i]) {
            out.push(lines[i].trim_end().to_string());
            i += 1;
            continue;
        }
        // A run of consecutive `|` lines is one table. Widths are decided within this block only.
        let start = i;
        while i < lines.len() && is_row(&lines[i]) {
            i += 1;
        }
        let block: Vec<(Vec<String>, Option<String>)> = lines[start..i]
            .iter()
            .map(|l| {
                let (body, com) = split_comment(l);
                (cells(&body), com)
            })
            .collect();
        // §5.1: `->` marks the boundary between inputs and outputs once. Marks written on the
        // second column onward are accepted, then folded into the canonical form. The parser
        // reads the extra marks too, so this is the formatter's job: "either form is accepted,
        // but saving yields one form".
        let mut block = block;
        if let Some((head, _)) = block.first_mut() {
            let mut seen = false;
            for c in head.iter_mut() {
                let is_arrow = c.starts_with("->");
                if is_arrow && seen {
                    *c = c.trim_start_matches("->").trim_start().to_string();
                }
                seen |= is_arrow;
            }
        }
        let ncol = block.iter().map(|(c, _)| c.len()).max().unwrap_or(0);
        let mut w = vec![0usize; ncol];
        for (cs, _) in &block {
            for (k, c) in cs.iter().enumerate() {
                w[k] = w[k].max(width(c));
            }
        }
        for (cs, com) in &block {
            let mut s = String::from("|");
            for (k, c) in cs.iter().enumerate() {
                let pad = w[k].saturating_sub(width(c));
                s.push(' ');
                s.push_str(c);
                s.push_str(&" ".repeat(pad));
                s.push_str(" |");
            }
            if let Some(c) = com {
                s.push_str("  ");
                s.push_str(c);
            }
            out.push(s);
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}
