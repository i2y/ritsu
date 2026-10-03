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
    /// `.` — the step of a path into the caller's object (`order.shipping.prefecture`),
    /// and nowhere else: a decimal point is read inside the number, and `..` is read above
    /// this (§15.125).
    Dot,
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

/// Whether a word can stand unquoted where the language expects a name — every character an
/// identifier character, and the first one a letter.
///
/// `rulec source pin` asks: a fragment the lexer would not read as one word is written in
/// quotes, and the quotes are what a reader sees in the citation too.
pub fn is_bare_word(s: &str) -> bool {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_alphabetic() || c == '_' || matches!(c, '℃' | '℉') => !s.chars().any(is_delim),
        _ => false,
    }
}

/// Unit suffixes a numeric literal may carry (§2.1). `万`/`億` are multipliers and
/// are consumed before this.
fn is_unit_char(c: char) -> bool {
    // Any ASCII letter, so that a currency code is one token: with a whitelist of the seven
    // characters the metric units happened to need, `5USD` split into `5` and `USD` and
    // stopped at E014 — as a cell with two words in it, which says nothing about the cause.
    // An unknown run is a unit that is not known, and is reported as one.
    c.is_ascii_alphabetic() || matches!(c, '円' | '銭' | '坪' | '℃' | '℉' | '%' | '％')
}

/// A digit inside a unit, for `m2` and `m3` (§15.83). It may not *start* one: the digits of
/// the number itself are read first, so a unit that could begin with a digit would swallow
/// whatever followed a `万` multiplier. Before this, `100m2` was cut into `100m` and `2`,
/// and the leftover `2` was dropped by every reader that walks a line looking for a word —
/// `range >=0m2 <=100m2` passed as `>=0m <=100m`. E047 now stops such a leftover, and this
/// keeps the unit whole so that there is none.
fn is_unit_tail(c: char) -> bool {
    is_unit_char(c) || c.is_ascii_digit()
}

pub fn lex_line(line_no: usize, text: &str) -> Result<Vec<Token>, Diag> {
    let (ts, soft) = lex_line_soft(line_no, text)?;
    match soft.into_iter().next() {
        Some(d) => Err(d),
        None => Ok(ts),
    }
}

/// The tokens of a line, with the errors that do not stop it. A number written with thousands
/// separators is reported (E049) and read as the number it plainly is: cutting the line off
/// there would end the table it stands in, and every row below would be reported as well.
pub fn lex_line_soft(line_no: usize, text: &str) -> Result<(Vec<Token>, Vec<Diag>), Diag> {
    let b = text.as_bytes();
    let mut i = 0usize;
    let mut out = Vec::new();
    let mut soft = Vec::new();

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
            '.' => Some(Kind::Dot),
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
                if let Some((e, whole, n)) = separated(&text[i..], &num, len, line_no, start) {
                    soft.push(e);
                    push(Kind::Num(whole), n, &mut out);
                    i += n;
                    continue;
                }
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
                // The shape alone let `2026-01-99` through: the checker counted it as a day
                // number (the ninety-ninth day from January 1st), and the generated Python
                // stopped at run time where JavaScript's `Date` would have rolled it over
                // (§15.156). The token stays, so the rest of the line is still read.
                if !is_date(y, m, d) {
                    soft.push(
                        Diag::error("E062", tr!("`{}` という日付はありません", "There is no such date as `{}`", &text[i..i + len]))
                            .mark(Span::new(line_no, start, len), tr!("月は 1〜12、日はその月の日数までです", "a month is 1 to 12, a day at most the days of its month")),
                    );
                }
                push(Kind::Date(y, m, d), len, &mut out);
                i += len;
                continue;
            }
            let (num, len) = lex_number(&text[i..], false)?;
            if let Some((e, whole, n)) = separated(&text[i..], &num, len, line_no, start) {
                soft.push(e);
                push(Kind::Num(whole), n, &mut out);
                i += n;
                continue;
            }
            push(Kind::Num(num), len, &mut out);
            i += len;
            continue;
        }

        // An identifier starts with a letter or `_`. Letting control characters or
        // symbols into identifiers would turn a typo into a name (a silent failure).
        //
        // ℃ and ℉ are the exception: Unicode files them as symbols rather than letters, and
        // a unit has to be writable where a type's brackets take a word (`temperature[℃]`)
        // as well as after a number. 円 and 銭 are Han and never needed this.
        if !(c.is_alphabetic() || c == '_' || matches!(c, '℃' | '℉')) {
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
    Ok((out, soft))
}

/// Whether the day exists in the proleptic Gregorian calendar, from year 1.
pub fn is_date(y: i32, m: u32, d: u32) -> bool {
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    y >= 1 && (1..=days).contains(&d)
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
/// E049: a number written with thousands separators, as a document writes it — `1,000`,
/// `1,949,000円`. `,` separates the members of a set, so the figure would be read as more than
/// one value, and in a cell it was: `<=1,000` held `<=1` and let `000` fall away. One to three
/// digits followed by groups of exactly three after `,` is taken for the separator it almost
/// always is, and refused with the literal as it has to be written — which is also what the
/// rest of the line is read with: the number, and how many bytes the text spent on it.
fn separated(text: &str, num: &Num, len: usize, line_no: usize, start: usize) -> Option<(Diag, Num, usize)> {
    if num.unit.is_some() || !num.frac.is_empty() || num.mult != 1 || !(1..=3).contains(&num.digits.len()) || num.raw.contains('_') {
        return None;
    }
    let rest = &text[len..];
    let mut j = 0;
    loop {
        let r = &rest[j..];
        let sep = if r.starts_with(',') {
            1
        } else if r.starts_with('，') {
            '，'.len_utf8()
        } else {
            break;
        };
        let d = r[sep..].as_bytes();
        if d.len() >= 3 && d[..3].iter().all(u8::is_ascii_digit) && !d.get(3).is_some_and(u8::is_ascii_digit) {
            j += sep + 3;
        } else {
            break;
        }
    }
    if j == 0 {
        return None;
    }
    let digits = format!("{}{}", &text[..len], rest[..j].replace([',', '，'], ""));
    // What follows the last group — a decimal, a multiplier, a unit — is part of the literal.
    let (whole, n) = lex_number(&format!("{digits}{}", &rest[j..]), num.neg).ok()?;
    let written = &text[..len + j + (n - digits.len())];
    let fixed = whole.raw.clone();
    Some((
        Diag::error("E049", tr!("桁区切りのカンマは書けません", "A thousands separator cannot be written"))
            .mark(Span::new(line_no, start, written.len()), tr!("`{fixed}` と書きます", "write `{fixed}`"))
            .note(tr!(
                "`,` は集合の要素を区切る記号なので、`{written}` のままでは二つ以上の値に読まれます。桁を区切りたいときは `_` を使えます（`1_000`）。",
                "`,` separates the members of a set, so `{written}` as it stands reads as more than one value. To group digits, use `_` (`1_000`)."
            ))
            .fix(crate::diag::FixKind::RewriteLiteral, fixed),
        whole,
        written.len(),
    ))
}

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
    if s[i..].chars().next().is_some_and(is_unit_char) {
        i += s[i..].chars().next().unwrap().len_utf8();
        while s[i..].chars().next().is_some_and(is_unit_tail) {
            i += s[i..].chars().next().unwrap().len_utf8();
        }
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
