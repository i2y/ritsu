//! Line-oriented tokenizer.
//!
//! §1.1 fixes the file as line-oriented with no cell newlines, so the lexer works
//! one line at a time and every token carries its line and byte column. That is what
//! lets §11's frames point at a cell.

use crate::diag::{Diag, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Num {
    pub neg: bool,
    /// Digits before the decimal point, with `_` removed.
    pub digits: String,
    /// Digits after the decimal point, with `_` removed. Empty when there is no point.
    /// §2.1 wants `0.5%` to be the exact rational 1/200, so the two halves are kept apart
    /// here and combined once, where the unit is known.
    pub frac: String,
    /// 1, 10_000 (万), 100_000_000 (億).
    pub mult: u64,
    /// 円 / 銭 / g / kg / cm / m / % — absent for a bare number, which §3 rejects in a cell.
    pub unit: Option<String>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Ident(String),
    Num(Num),
    Str(String),
    Date(i32, u32, u32),
    Pipe,
    Arrow,
    Colon,
    Eq,
    LParen,
    RParen,
    LBracket,
    RBracket,
    /// `,` — the set separator in a cell (§3, third unary test), and the separator of
    /// type attributes and call arguments elsewhere. `・` `、` `，` are read as the same
    /// token; `rulec fmt` writes `,`.
    Comma,
    Le,
    Ge,
    Lt,
    Gt,
    Plus,
    /// `-` or `−`. A cell holding only this is don't-care (§3, first unary test).
    Minus,
    Star,
    Slash,
    /// `?` — the optional marker (`区分?`).
    Question,
    /// `..` — always an error (§3.1 forbids range notation), lexed so E010 can point at it.
    DotDot,
    /// `@` — starts a citation: `@<source> <fragment>, …` at the end of a line (§15.68).
    At,
    /// `sha256:9e4edb5b6a1c0f42` — a pinned digest, one token so that the hex digits are
    /// not read as a number with a unit.
    Hash(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
}

impl Token {
    pub fn ident(&self) -> Option<&str> {
        match &self.kind {
            Kind::Ident(s) => Some(s),
            _ => None,
        }
    }
    pub fn is(&self, k: &Kind) -> bool {
        &self.kind == k
    }
}

/// Characters that end an identifier. Everything else — Latin, digits, kana, kanji,
/// `_` — is an identifier character. `-` is excluded on purpose: it is always an
/// operator or don't-care, never part of a name.
fn is_delim(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | ':'
                | '='
                | '|'
                | '#'
                | '"'
                | '<'
                | '>'
                | ','
                | '・'
                | '→'
                | '-'
                | '−'
                | '+'
                | '*'
                | '×'
                | '/'
                | '÷'
                | '≦'
                | '≧'
                | '＜'
                | '＞'
                | '.'
                | '?'
                | '、'
                | '，'
                | '@'
        )
}

/// Unit suffixes a numeric literal may carry (§2.1). `万`/`億` are multipliers and
/// are consumed before this.
fn is_unit_char(c: char) -> bool {
    // Any ASCII letter, so that a currency code is one token: with a whitelist of the seven
    // characters the metric units happened to need, `5USD` split into `5` and `USD` and
    // stopped at E014 — as a cell with two words in it, which says nothing about the cause.
    // An unknown run is a unit that is not known, and is reported as one.
    c.is_ascii_alphabetic() || matches!(c, '円' | '銭' | '%' | '％')
}

pub fn lex_line(line_no: usize, text: &str) -> Result<Vec<Token>, Diag> {
    let b = text.as_bytes();
    let mut i = 0usize;
    let mut out = Vec::new();

    while i < b.len() {
        let c = text[i..].chars().next().unwrap();
        let clen = c.len_utf8();

        if c.is_whitespace() {
            i += clen;
            continue;
        }
        // `#` starts a comment that runs to end of line.
        if c == '#' {
            break;
        }

        let start = i;
        let push = |kind: Kind, len: usize, out: &mut Vec<Token>| {
            out.push(Token { kind, span: Span::new(line_no, start, len) });
        };

        // Two-character operators first.
        if text[i..].starts_with("<=") {
            push(Kind::Le, 2, &mut out);
            i += 2;
            continue;
        }
        if text[i..].starts_with(">=") {
            push(Kind::Ge, 2, &mut out);
            i += 2;
            continue;
        }
        // `->` marks the first output column (§5.1). `→` is read as the same token and
        // `rulec fmt` writes `->`.
        if text[i..].starts_with("->") {
            push(Kind::Arrow, 2, &mut out);
            i += 2;
            continue;
        }
        if text[i..].starts_with("..") {
            let mut n = 2;
            while text[i + n..].starts_with('.') {
                n += 1;
            }
            push(Kind::DotDot, n, &mut out);
            i += n;
            continue;
        }

        let single = match c {
            '|' => Some(Kind::Pipe),
            '@' => Some(Kind::At),
            '→' => Some(Kind::Arrow),
            ':' => Some(Kind::Colon),
            '=' => Some(Kind::Eq),
            '(' => Some(Kind::LParen),
            ')' => Some(Kind::RParen),
            '[' => Some(Kind::LBracket),
            ']' => Some(Kind::RBracket),
            ',' | '、' | '，' | '・' => Some(Kind::Comma),
            '?' | '？' => Some(Kind::Question),
            '<' | '＜' => Some(Kind::Lt),
            '>' | '＞' => Some(Kind::Gt),
            '≦' => Some(Kind::Le),
            '≧' => Some(Kind::Ge),
            '+' | '＋' => Some(Kind::Plus),
            '*' | '×' => Some(Kind::Star),
            '/' | '÷' => Some(Kind::Slash),
            _ => None,
        };
        if let Some(k) = single {
            push(k, clen, &mut out);
            i += clen;
            continue;
        }

        // `-` / `−`: minus, or don't-care when it stands alone in a cell.
        if c == '-' || c == '−' {
            // A negative numeric literal binds the sign, so `>=-110万円` lexes as one number.
            let after = text[i + clen..].chars().next();
            if after.is_some_and(|d| d.is_ascii_digit()) {
                let (num, len) = lex_number(&text[i..], true)?;
                push(Kind::Num(num), len, &mut out);
                i += len;
                continue;
            }
            push(Kind::Minus, clen, &mut out);
            i += clen;
            continue;
        }

        if c == '"' {
            let rest = &text[i + 1..];
            let Some(end) = rest.find('"') else {
                return Err(Diag::error("E001", tr!("文字列が閉じていません", "Unterminated string"))
                    .mark(Span::new(line_no, start, 1), tr!("ここで始まった \" が閉じていません", "the \" opened here is never closed")));
            };
            let s = rest[..end].to_string();
            push(Kind::Str(s), end + 2, &mut out);
            i += end + 2;
            continue;
        }

        if c.is_ascii_digit() {
            // A date is `NNNN-NN-NN` with no spaces, which is why it is tried first.
            if let Some((y, m, d, len)) = try_date(&text[i..]) {
                push(Kind::Date(y, m, d), len, &mut out);
                i += len;
                continue;
            }
            let (num, len) = lex_number(&text[i..], false)?;
            push(Kind::Num(num), len, &mut out);
            i += len;
            continue;
        }

        // An identifier starts with a letter or `_`. Letting control characters or
        // symbols into identifiers would turn a typo into a name (a silent failure).
        if !(c.is_alphabetic() || c == '_') {
            return Err(Diag::error(
                "E002",
                tr!("読めない文字 U+{:04X} があります", "Unreadable character U+{:04X}", c as u32),
            )
            .mark(Span::new(line_no, start, clen), "")
            .note(tr!("識別子は文字か `_` で始まります。制御文字や記号は名前になれません。", "An identifier starts with a letter or `_`. Control characters and symbols cannot be names.")));
        }

        // Identifier: run to the next delimiter.
        let mut n = 0usize;
        for ch in text[i..].chars() {
            if is_delim(ch) {
                break;
            }
            n += ch.len_utf8();
        }
        if n == 0 {
            return Err(Diag::error("E002", tr!("読めない文字 `{c}` があります", "Unreadable character `{c}`"))
                .mark(Span::new(line_no, start, clen), ""));
        }
        let word = text[i..i + n].to_string();
        // A pinned digest is one token (§15.68). Read as a number, `9e4edb…` would become a
        // value with a unit.
        if word == "sha256" && text[i + n..].starts_with(':') {
            let hex: String = text[i + n + 1..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if !hex.is_empty() {
                let len = n + 1 + hex.len();
                push(Kind::Hash(hex), len, &mut out);
                i += len;
                continue;
            }
        }
        push(Kind::Ident(word), n, &mut out);
        i += n;
    }
    Ok(out)
}

fn try_date(s: &str) -> Option<(i32, u32, u32, usize)> {
    let b = s.as_bytes();
    if b.len() < 10 {
        return None;
    }
    let ok = b[..4].iter().all(|c| c.is_ascii_digit())
        && b[4] == b'-'
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[7] == b'-'
        && b[8..10].iter().all(|c| c.is_ascii_digit());
    if !ok {
        return None;
    }
    Some((
        s[0..4].parse().ok()?,
        s[5..7].parse().ok()?,
        s[8..10].parse().ok()?,
        10,
    ))
}

/// `120円` `2_000g` `100万円` `10%` `0.5%` `-110万円`. Returns the token and its byte length
/// including the leading sign when `neg` is set.
/// One number at the start of `s`, and how many bytes it took: `990円`, `1.5%`, `100万`. What
/// a document's cell says is read with this too, so that a copy and a table are read in one
/// language (§15.82).
pub fn number(s: &str) -> Option<(Num, usize)> {
    if !s.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    lex_number(s, false).ok().filter(|(n, _)| !n.digits.is_empty())
}

fn lex_number(s: &str, neg: bool) -> Result<(Num, usize), Diag> {
    let mut i = if neg { s.chars().next().unwrap().len_utf8() } else { 0 };
    let start_digits = i;
    while s[i..].chars().next().is_some_and(|c| c.is_ascii_digit() || c == '_') {
        i += 1;
    }
    let digits: String = s[start_digits..i].chars().filter(|c| *c != '_').collect();

    // A decimal point counts only when a digit follows it, so `1.` still ends the number at
    // `1` and the stray `.` is reported where it stands.
    let mut frac = String::new();
    if s[i..].starts_with('.') && s[i + 1..].chars().next().is_some_and(|c| c.is_ascii_digit()) {
        i += 1;
        let start_frac = i;
        while s[i..].chars().next().is_some_and(|c| c.is_ascii_digit() || c == '_') {
            i += 1;
        }
        frac = s[start_frac..i].chars().filter(|c| *c != '_').collect();
    }

    let mut mult = 1u64;
    if s[i..].starts_with('万') {
        mult = 10_000;
        i += '万'.len_utf8();
    } else if s[i..].starts_with('億') {
        mult = 100_000_000;
        i += '億'.len_utf8();
    }

    let unit_start = i;
    while s[i..].chars().next().is_some_and(is_unit_char) {
        i += s[i..].chars().next().unwrap().len_utf8();
    }
    let unit = if i > unit_start {
        Some(s[unit_start..i].replace('％', "%"))
    } else {
        None
    };

    Ok((
        Num { neg, digits, frac, mult, unit, raw: s[..i].to_string() },
        i,
    ))
}
