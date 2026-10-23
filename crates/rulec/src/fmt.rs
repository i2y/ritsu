//! `rulec fmt` — the one and only formatter (§1.5).
//!
//! Columns are aligned to the widest content of each table, counting East Asian Width W and F
//! as two columns. The symbols written with an IME are written in ASCII where the lexer reads
//! them as symbols, and full-width digits where it reads no word; a name keeps every character it
//! was written with (§15.257). Formatting is confined to the table, so the diff of a realignment
//! is confined to the table too.

use crate::diag::width;
use crate::lex::{self, Kind};

/// Writes in ASCII the spellings that need an IME or a full-width keyboard, without changing a
/// word the lexer reads (§15.257): first the full-width digits the lexer cannot read, then the
/// symbols it reads as tokens of their own. String literals and `#` comments are left alone, and so
/// is a name: `松＋竹` and `第１案` are names, and stay as written.
fn normalize(line: &str) -> String {
    symbols_in_ascii(&digits_in_ascii(line))
}

/// A full-width digit where the lexer reads no word — where a word would start, which stops the
/// lexer at E002 — written in ASCII, one at a time, the line read again after each: `１０万円`
/// becomes the number `10万円` it was meant to be. A digit inside a name (`第１案`) is the name's,
/// and the lexer reads the name with it, so it stays.
fn digits_in_ascii(line: &str) -> String {
    let mut text = line.to_string();
    loop {
        let unread = lex::read_through(&text).into_iter().find_map(|(r, k)| {
            let mut cs = text[r.clone()].chars();
            match (k, cs.next(), cs.next()) {
                (None, Some(c @ '０'..='９'), None) => Some((r, c)),
                _ => None,
            }
        });
        let Some((r, c)) = unread else { return text };
        text.replace_range(r, &((b'0' + (c as u32 - '０' as u32) as u8) as char).to_string());
    }
}

/// The ASCII spelling of a symbol the lexer read as a token of its own, written with an IME.
fn ascii_symbol(written: &str, kind: &Kind) -> Option<&'static str> {
    Some(match (written, kind) {
        ("≦", Kind::Le) => "<=",
        ("≧", Kind::Ge) => ">=",
        ("＜", Kind::Lt) => "<",
        ("＞", Kind::Gt) => ">",
        ("＋", Kind::Plus) => "+",
        ("→", Kind::Arrow) => "->",
        ("・" | "、" | "，", Kind::Comma) => ",",
        _ => return None,
    })
}

/// The tokens of a line, in order: what the lexer reads of it.
fn words(line: &str) -> Vec<Option<Kind>> {
    lex::read_through(line).into_iter().map(|(_, k)| k).collect()
}

/// The symbols the lexer reads as tokens of their own — `≦ ≧ ＜ ＞ ＋`, the output marker `→`
/// and the set separators `・ 、 ，` — written `<= >= < > +`, `->` and `, `, and the ideographic
/// space between words as a space. Inside a word nothing moves: a `＋` the lexer reads as part of
/// a name, or a `，` it reads as part of a number (E049), stays. An ASCII spelling that would join
/// its neighbour into another token (`-＞` would be `->`, `＞=` would be `>=`) is set apart from it
/// by a space, so the lexer still reads the two words it read before.
fn symbols_in_ascii(line: &str) -> String {
    let pieces = lex::read_through(line);
    let read = words(line);
    let mut out = String::with_capacity(line.len());
    let mut end = 0;
    // after a set separator, the blanks up to the next word are dropped: it is written `, `
    let mut after_comma = false;
    let blanks = |gap: &str, out: &mut String| out.extend(gap.chars().map(|c| if c == '　' { ' ' } else { c }));
    for (r, k) in &pieces {
        if !after_comma {
            blanks(&line[end..r.start], &mut out);
        }
        after_comma = false;
        let written = &line[r.clone()];
        end = r.end;
        let Some(ascii) = k.as_ref().and_then(|k| ascii_symbol(written, k)) else {
            out.push_str(written);
            continue;
        };
        if ascii == "," {
            let kept = out.trim_end().len();
            out.truncate(kept);
            out.push_str(", ");
            after_comma = true;
            continue;
        }
        // The first spelling the lexer reads the line the same with: the symbol alone, or set
        // apart by a space from the word before, the word after, or both. Else it stays.
        let rest = &line[end..];
        let spaced = |before: bool, after: bool| format!("{}{ascii}{}", if before { " " } else { "" }, if after { " " } else { "" });
        let fits = [(false, false), (true, false), (false, true), (true, true)]
            .into_iter()
            .map(|(b, a)| spaced(b, a))
            .find(|s| words(&format!("{out}{s}{rest}")) == read);
        out.push_str(fits.as_deref().unwrap_or(written));
    }
    // The blanks after the last word, and the comment as it is written.
    let tail = &line[end..];
    let comment = tail.find('#').unwrap_or(tail.len());
    if !after_comma {
        blanks(&tail[..comment], &mut out);
    }
    out.push_str(&tail[comment..]);
    out
}

/// Where the lexer reads the comment of a line to start: the first `#` after the last word it reads.
/// A `#` it reads in a string is the string's, and a `|` or a `,` in the comment is the comment's
/// (§1.5, §15.267).
fn comment_at(line: &str) -> Option<usize> {
    let end = lex::read_through(line).last().map_or(0, |(r, _)| r.end);
    line[end..].find('#').map(|h| end + h)
}

/// The bars of a line, where the lexer reads them: a `|` in a string or in the comment is none.
fn bars(line: &str) -> Vec<std::ops::Range<usize>> {
    lex::read_through(line).into_iter().filter(|(_, k)| matches!(k, Some(Kind::Pipe))).map(|(r, _)| r).collect()
}

/// A row is a `|` line, or a label followed by a `|` line: `r1 | 北海道 | S60 | 1150円 |`.
/// The label is one word — anything with a space before the first bar is a statement whose
/// string or expression happens to contain a bar. The bar is one the lexer reads, so a `|` in a
/// string or in the comment makes no row (§15.267).
fn is_row(line: &str) -> bool {
    match bars(line).first() {
        Some(bar) => {
            let head = line[..bar.start].trim();
            head.is_empty() || !head.contains(char::is_whitespace)
        }
        None => false,
    }
}

/// A row cut where the lexer cuts it (§1.5, §15.267): the label before the first bar, the text of
/// the cells from the first bar on, the citation from the first `@` after the last bar, and the
/// comment from the first `#` after the last word. A `|`, `@` or `#` the lexer reads in a string or
/// in the comment cuts nothing, so a comment stays as written (`| a |  # x|y`), and so does a
/// citation of a fragment in quotes (`@osha "§1910.157"`).
fn split_row(line: &str) -> (String, &str, Option<String>, Option<String>) {
    let pieces = lex::read_through(line);
    let end = pieces.last().map_or(0, |(r, _)| r.end);
    let comment = line[end..].find('#').map(|h| line[end + h..].trim_end().to_string());
    let bars: Vec<&std::ops::Range<usize>> = pieces.iter().filter(|(_, k)| matches!(k, Some(Kind::Pipe))).map(|(r, _)| r).collect();
    let (first, last) = (bars.first().map_or(0, |r| r.start), bars.last().map_or(0, |r| r.end));
    let cite = pieces.iter().find(|(r, k)| r.start >= last && matches!(k, Some(Kind::At))).map(|(r, _)| r.start);
    let label = line[..first].trim().to_string();
    let cells = &line[first..cite.unwrap_or(end).max(first)];
    (label, cells, cite.map(|a| line[a..end].trim().to_string()), comment)
}

/// A pinned fragment under a `source` line: one word and one digest.
fn is_pin(line: &str) -> bool {
    let ws: Vec<&str> = line.split_whitespace().collect();
    ws.len() == 2 && ws[1].starts_with("sha256:") && !line.trim_start().starts_with('|')
}

/// The cells of a row, cut at each bar outside a string: `starts_with "a|b"` is one cell, as the
/// lexer reads the string whole (§15.257).
fn cells(row: &str) -> Vec<String> {
    let t = row.trim();
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut out = Vec::new();
    let (mut start, mut in_str) = (0, false);
    for (i, c) in inner.char_indices() {
        match c {
            '"' => in_str = !in_str,
            '|' if !in_str => {
                out.push(inner[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(inner[start..].trim().to_string());
    out
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

/// Whether a line under `apply` that starts with a line head is a binding of a callee input named
/// like one that is not reserved (`prerequisites = …`, §15.222).
fn binding(line: &str) -> bool {
    let mut ws = line.split_whitespace();
    let w = ws.next().unwrap_or("");
    !crate::kw::RESERVED.contains(&w) && ws.next().is_some_and(|n| n == "=" || n == "->" || n == "→")
}

/// Whether a line belongs to the block under a `prerequisites` heading (§15.222): not blank, not a
/// row, and not a line head — except on a value's own line, which a value named like one has.
fn prereq_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || is_row(line) {
        return false;
    }
    let mut ws = t.split_whitespace();
    let w = ws.next().unwrap_or("");
    ws.next() == Some(crate::kw::REQUIRES) || !crate::kw::LINE_HEAD.contains(&w)
}

/// The block under `prerequisites` (§15.222): two spaces in, the value of each line — and `start` —
/// padded to the widest, so that `requires` and the start values line up; the rest one space
/// apart, a list's comma against the word before it, the reason as written. A comment line stays
/// as it is, two spaces in.
fn prereq_body(lines: &[String]) -> Vec<String> {
    // (the first word, the rest), or the comment line as it is
    let parts: Vec<Result<(String, String), String>> = lines
        .iter()
        .map(|l| {
            let t = l.trim();
            if t.starts_with('#') {
                return Err(format!("  {t}"));
            }
            let b = clause_body(l);
            let b = b.trim_start();
            let (word, rest) = match b.find(' ') {
                Some(p) => (b[..p].to_string(), b[p + 1..].trim_start().to_string()),
                None => (b.to_string(), String::new()),
            };
            Ok((word, lists(&rest)))
        })
        .collect();
    let aligned = |w: &str, rest: &str| w == crate::kw::START || rest.split_whitespace().next() == Some(crate::kw::REQUIRES);
    let pad = parts.iter().filter_map(|p| p.as_ref().ok()).filter(|(w, r)| aligned(w, r)).map(|(w, _)| width(w)).max().unwrap_or(0);
    parts
        .into_iter()
        .map(|p| match p {
            Err(comment) => comment,
            Ok((w, rest)) if rest.is_empty() => format!("  {w}"),
            Ok((w, rest)) if aligned(&w, &rest) => format!("  {w}{} {rest}", " ".repeat(pad - width(&w))),
            Ok((w, rest)) => format!("  {w} {rest}"),
        })
        .collect()
}

/// A list written `a, b or c`: the comma against the word before it and one space after, outside
/// the string and the comment.
fn lists(rest: &str) -> String {
    let cut = rest.find(['"', '#']).unwrap_or(rest.len());
    let (words, tail) = rest.split_at(cut);
    let words: Vec<String> = words.split(',').map(|w| w.trim().to_string()).collect();
    let mut o = words.join(", ");
    if !tail.is_empty() {
        // A comment keeps its distance of two, as everywhere else.
        if !o.is_empty() {
            o.push_str(if tail.starts_with('#') { "  " } else { " " });
        }
        o.push_str(tail);
    }
    o.trim_end().to_string()
}

/// A line under `machine` (§15.148): two spaces in, the keyword padded to the width of the
/// longest one so the values start in one column, and the rest one space apart as a clause's
/// body is.
fn machine_body(line: &str) -> String {
    let b = clause_body(line);
    let t = b.trim_start();
    let (word, rest) = match t.find(' ') {
        Some(p) => (&t[..p], t[p + 1..].trim_start()),
        None => (t, ""),
    };
    let w = MACHINE_LINES.iter().map(|k| k.len()).max().unwrap_or(0);
    // A list is written `配達済, 取消`: the comma against the word before it. The comment and the
    // commas are where the lexer reads them, so a `#` or a ` ,` in a string stays (§15.267).
    let (words, comment) = match comment_at(rest) {
        Some(h) => (commas_against(rest[..h].trim_end()), format!("  {}", &rest[h..])),
        None => (commas_against(rest), String::new()),
    };
    let rest = format!("{words}{comment}");
    let rest = rest.trim_start();
    if rest.is_empty() {
        format!("  {word}")
    } else if rest.starts_with('#') {
        format!("  {word}  {rest}")
    } else {
        format!("  {word:<w$} {rest}")
    }
}

/// Each comma the lexer reads, against the word before it: the blanks before it go, unless the line
/// would then read other words (`1 ,000` would be the one number `1,000`, E049). A `,` in a string is
/// the string's, and stays where it is (§15.267).
fn commas_against(text: &str) -> String {
    let read = words(text);
    let mut out = String::with_capacity(text.len());
    let mut end = 0;
    for (r, k) in lex::read_through(text) {
        let against = matches!(k, Some(Kind::Comma)) && words(&format!("{out}{}", &text[r.start..])) == read;
        if !against {
            out.push_str(&text[end..r.start]);
        }
        out.push_str(&text[r.clone()]);
        end = r.end;
    }
    out.push_str(&text[end..]);
    out
}

/// The words that start a line under `machine`.
const MACHINE_LINES: &[&str] =
    &[crate::kw::CARRY, crate::kw::HELD, crate::kw::INITIAL, crate::kw::FINAL, crate::kw::NEVER, crate::kw::ONCE, crate::kw::ALWAYS];

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
    // The lines under a `machine`, up to the next blank line or line head.
    let mut in_machine = false;
    // The terms under an `open` value or an `into` (§15.227), up to the next blank line or line
    // head: one name each, two spaces in.
    let mut in_terms = false;
    while i < lines.len() {
        if !is_row(&lines[i]) {
            let w = first_word(&lines[i]);
            if in_terms && !lines[i].trim().is_empty() && !w.starts_with('#') && !crate::kw::LINE_HEAD.contains(&w) && !crate::kw::CONTRIBUTION_HEAD.contains(&w) {
                out.push(clause_body(&lines[i]));
                i += 1;
                continue;
            }
            in_terms = (w == crate::kw::OPEN || w == crate::kw::INTO) && !lines[i].starts_with(char::is_whitespace) && !binding(&lines[i]);
            if in_machine && MACHINE_LINES.contains(&w) {
                out.push(machine_body(&lines[i]));
                i += 1;
                continue;
            }
            in_machine = w == crate::kw::MACHINE;
            // The lines under `prerequisites` (§15.222), aligned as one block.
            if w == crate::kw::PREREQUISITES && !lines[i].starts_with(char::is_whitespace) {
                in_clause = false;
                in_apply = false;
                out.push(lines[i].trim_end().to_string());
                i += 1;
                let start = i;
                while i < lines.len() && prereq_line(&lines[i]) {
                    i += 1;
                }
                out.extend(prereq_body(&lines[start..i]));
                continue;
            }
            if in_apply && !lines[i].trim().is_empty() && (w == crate::kw::EXCEPT || !crate::kw::LINE_HEAD.contains(&w) || binding(&lines[i])) && !w.starts_with('#') {
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
        in_machine = false;
        in_terms = false;
        // A run of consecutive `|` lines is one table. Widths are decided within this block only.
        let start = i;
        while i < lines.len() && is_row(&lines[i]) {
            i += 1;
        }
        let block: Vec<(String, Vec<String>, Option<String>, Option<String>)> = lines[start..i]
            .iter()
            .map(|l| {
                let (label, body, cite, com) = split_row(l);
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
