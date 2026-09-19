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

/// A row is a `|` line, or a label followed by a `|` line: `r1 | 北海道 | S60 | 1150円 |`.
/// The label is one word — anything with a space before the first bar is a statement whose
/// string or expression happens to contain a bar.
fn is_row(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('|') {
        return true;
    }
    match t.find('|') {
        Some(p) => {
            let head = t[..p].trim_end();
            !head.is_empty() && !head.contains(char::is_whitespace) && !head.starts_with('#')
        }
        None => false,
    }
}

/// The label written before the first `|`, if any, and the rest of the row.
fn split_label(row: &str) -> (String, &str) {
    let t = row.trim();
    match t.find('|') {
        Some(p) if p > 0 => (t[..p].trim().to_string(), &t[p..]),
        _ => (String::new(), t),
    }
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

/// The citation after the last bar of a row (`|  @法 第20条`), split off so the cells end
/// at the bar as they should.
fn split_cite_row(body: &str) -> (String, Option<String>) {
    let Some(bar) = body.rfind('|') else { return (body.to_string(), None) };
    let tail = &body[bar + 1..];
    match tail.find('@') {
        Some(a) => (body[..bar + 1].to_string(), Some(tail[a..].trim().to_string())),
        None => (body.to_string(), None),
    }
}

/// A pinned fragment under a `source` line: one word and one digest.
fn is_pin(line: &str) -> bool {
    let ws: Vec<&str> = line.split_whitespace().collect();
    ws.len() == 2 && ws[1].starts_with("sha256:") && !line.trim_start().starts_with('|')
}

fn cells(row: &str) -> Vec<String> {
    let t = row.trim();
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

/// The body of a clause — `when`, `then`, `overrides` — is indented by two spaces and its
/// words are one space apart, outside string literals; the comment keeps its distance of two.
fn clause_body(line: &str) -> String {
    let t = line.trim();
    let mut body = String::new();
    let mut comment: Option<&str> = None;
    let mut in_str = false;
    let mut pending_space = false;
    for (i, ch) in t.char_indices() {
        if in_str {
            body.push(ch);
            if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => {
                if pending_space {
                    body.push(' ');
                    pending_space = false;
                }
                in_str = true;
                body.push(ch);
            }
            '#' => {
                comment = Some(t[i..].trim_end());
                break;
            }
            c if c.is_whitespace() => pending_space = !body.is_empty(),
            c => {
                if pending_space {
                    body.push(' ');
                    pending_space = false;
                }
                body.push(c);
            }
        }
    }
    let mut o = String::from("  ");
    o.push_str(body.trim_end());
    if let Some(c) = comment {
        o.push_str("  ");
        o.push_str(c);
    }
    o
}

fn first_word(line: &str) -> &str {
    line.trim_start().split(|c: char| c.is_whitespace()).next().unwrap_or("")
}

pub fn format(src: &str) -> String {
    let lines: Vec<String> = src.lines().map(normalize).collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    let mut in_clause = false;
    // The body of an `apply` — bindings, `except`, output names — is indented like a clause's,
    // up to the next blank line or line head.
    let mut in_apply = false;
    while i < lines.len() {
        if !is_row(&lines[i]) {
            let w = first_word(&lines[i]);
            if in_apply && !lines[i].trim().is_empty() && (w == crate::kw::EXCEPT || !crate::kw::LINE_HEAD.contains(&w)) && !w.starts_with('#') {
                out.push(clause_body(&lines[i]));
                i += 1;
                continue;
            }
            in_apply = w == crate::kw::APPLY;
            if w == crate::kw::CLAUSE {
                in_clause = true;
                out.push(lines[i].trim_end().to_string());
            } else if in_clause && (w == crate::kw::WHEN || w == crate::kw::THEN || w == crate::kw::OVERRIDES) {
                out.push(clause_body(&lines[i]));
            } else if w == crate::kw::SOURCE {
                in_clause = false;
                out.push(lines[i].trim_end().to_string());
                i += 1;
                // The pinned fragments under it, two spaces in, the digests aligned.
                let start = i;
                while i < lines.len() && is_pin(&lines[i]) {
                    i += 1;
                }
                let pins: Vec<(String, String)> = lines[start..i]
                    .iter()
                    .map(|l| {
                        let mut it = l.split_whitespace();
                        (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string())
                    })
                    .collect();
                let pw = pins.iter().map(|(f, _)| width(f)).max().unwrap_or(0);
                for (f, h) in &pins {
                    out.push(format!("  {f}{} {h}", " ".repeat(pw - width(f))));
                }
                continue;
            } else {
                in_clause = false;
                out.push(lines[i].trim_end().to_string());
            }
            i += 1;
            continue;
        }
        in_clause = false;
        in_apply = false;
        // A run of consecutive `|` lines is one table. Widths are decided within this block only.
        let start = i;
        while i < lines.len() && is_row(&lines[i]) {
            i += 1;
        }
        let block: Vec<(String, Vec<String>, Option<String>, Option<String>)> = lines[start..i]
            .iter()
            .map(|l| {
                let (body, com) = split_comment(l);
                let (body, cite) = split_cite_row(&body);
                let (label, body) = split_label(&body);
                (label, cells(body), cite, com)
            })
            .collect();
        // §5.1: `->` marks the boundary between inputs and outputs once. Marks written on the
        // second column onward are accepted, then folded into the canonical form. The parser
        // reads the extra marks too, so this is the formatter's job: "either form is accepted,
        // but saving yields one form".
        let mut block = block;
        if let Some((_, head, _, _)) = block.first_mut() {
            let mut seen = false;
            for c in head.iter_mut() {
                let is_arrow = c.starts_with("->");
                if is_arrow && seen {
                    *c = c.trim_start_matches("->").trim_start().to_string();
                }
                seen |= is_arrow;
            }
        }
        let ncol = block.iter().map(|(_, c, _, _)| c.len()).max().unwrap_or(0);
        let mut w = vec![0usize; ncol];
        for (_, cs, _, _) in &block {
            for (k, c) in cs.iter().enumerate() {
                w[k] = w[k].max(width(c));
            }
        }
        // Labels form a column of their own before the first bar. A block without any keeps
        // its bars at the left edge, so a file without labels does not move.
        let lw = block.iter().map(|(l, _, _, _)| width(l)).max().unwrap_or(0);
        for (label, cs, cite, com) in &block {
            let mut s = String::new();
            if lw > 0 {
                s.push_str(label);
                s.push_str(&" ".repeat(lw.saturating_sub(width(label)) + 1));
            }
            s.push('|');
            for (k, c) in cs.iter().enumerate() {
                let pad = w[k].saturating_sub(width(c));
                s.push(' ');
                s.push_str(c);
                s.push_str(&" ".repeat(pad));
                s.push_str(" |");
            }
            if let Some(c) = cite {
                s.push_str("  ");
                s.push_str(c);
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
