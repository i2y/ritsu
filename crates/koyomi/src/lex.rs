//! Cutting a `.cal` into lines of tokens (DESIGN 1.2, PLAN B.3).
//!
//! The language is written a line at a time, so the lexer keeps the lines: each has its
//! indentation, its tokens, and its text without the comment. A word is cut out as a name
//! and told apart from a keyword by the parser, by position.

use crate::date::{self, Day};
use crate::diag::Diag;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    Int(i64),
    Date(Day),
    /// `12-29`: a month and a day, for the closures of every year.
    MonthDay(u32, u32),
    /// `09:00`.
    Time(u32, u32),
    Str(String),
    /// `sha256:cec37a743c96995c`: the sixteen hex digits.
    Sha(String),
    LParen,
    RParen,
    Colon,
    Comma,
    Pipe,
    At,
    DotDot,
    Arrow,
    Plus,
    Minus,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    /// 1-based, in characters.
    pub col: usize,
    /// The column after the token.
    pub end: usize,
}

#[derive(Clone, Debug)]
pub struct Line {
    /// 1-based.
    pub no: usize,
    /// Leading spaces.
    pub indent: usize,
    pub tokens: Vec<Token>,
    /// The characters of the line before the comment.
    pub chars: Vec<char>,
    /// For an `offset` line: the text after the word, as written, and its column. A time zone
    /// name (`Asia/Tokyo`) is not made of tokens, and it deserves E107 rather than E001.
    pub rest: Option<(String, usize)>,
}

impl Line {
    /// The text from column `from` up to (not including) column `to`, trimmed.
    pub fn slice(&self, from: usize, to: usize) -> String {
        let a = from.saturating_sub(1).min(self.chars.len());
        let b = to.saturating_sub(1).min(self.chars.len()).max(a);
        self.chars[a..b].iter().collect::<String>().trim().to_string()
    }

    /// The text from column `from` to the end of the line (before the comment), trimmed.
    pub fn rest_from(&self, from: usize) -> String {
        self.slice(from, self.chars.len() + 1)
    }
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn digits_at(cs: &[char], i: usize, n: usize) -> bool {
    i + n <= cs.len() && cs[i..i + n].iter().all(|c| c.is_ascii_digit())
}

fn num(cs: &[char], i: usize, n: usize) -> u32 {
    cs[i..i + n].iter().collect::<String>().parse().unwrap_or(0)
}

/// Every line of the source, and what could not be read (E001, E005, E006).
pub fn lex(file: &str, src: &str) -> (Vec<Line>, Vec<Diag>) {
    let mut lines = Vec::new();
    let mut diags = Vec::new();
    for (i, text) in src.lines().enumerate() {
        let no = i + 1;
        let all: Vec<char> = text.chars().collect();
        let err = |code: &'static str, col: usize, msg: ritsu_base::text::Text| Diag::error(code, file, no, col, msg).source(src);
        // Indentation is spaces. A tab is refused: how wide it is depends on the editor, and the
        // lines of a block are told apart by lining up (E005).
        let mut indent = 0;
        while indent < all.len() && (all[indent] == ' ' || all[indent] == '\t') {
            if all[indent] == '\t' {
                diags.push(
                    err("E005", indent + 1, tr!("字下げにタブがあります。字下げはスペースで書いてください", "The indentation has a tab; indent with spaces"))
                        .note(tr!(
                            "タブの幅はエディタによって違うので、行がそろっているかを決められません。",
                            "How wide a tab is depends on the editor, so whether the lines line up cannot be told."
                        )),
                );
            }
            indent += 1;
        }
        // The comment: from a `#` outside a string to the end of the line.
        let mut end = all.len();
        let mut in_str = false;
        let mut k = 0;
        while k < all.len() {
            match all[k] {
                '\\' if in_str => k += 1,
                '"' => in_str = !in_str,
                '#' if !in_str => {
                    end = k;
                    break;
                }
                _ => {}
            }
            k += 1;
        }
        let cs: Vec<char> = all[..end].to_vec();
        let mut toks = Vec::new();
        let mut rest = None;
        let mut j = indent;
        let errors_before = diags.len();
        while j < cs.len() {
            // One error a line: what comes after the first is noise.
            if diags.len() > errors_before && diags[errors_before..].iter().any(|d: &Diag| d.code != "E005") {
                break;
            }
            let c = cs[j];
            let col = j + 1;
            if c == ' ' || c == '\t' {
                j += 1;
                continue;
            }
            if c == '\u{3000}' {
                diags.push(err(
                    "E001",
                    col,
                    tr!("全角の空白があります。語と語のあいだは、半角のスペースで区切ってください", "There is a full-width space; separate words with ASCII spaces"),
                ));
                j += 1;
                continue;
            }
            let start = j;
            let tok = if c == '"' {
                let mut s = String::new();
                let mut k = j + 1;
                let mut closed = false;
                while k < cs.len() {
                    match cs[k] {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' if k + 1 < cs.len() && (cs[k + 1] == '"' || cs[k + 1] == '\\') => {
                            s.push(cs[k + 1]);
                            k += 2;
                        }
                        ch => {
                            s.push(ch);
                            k += 1;
                        }
                    }
                }
                if !closed {
                    diags.push(err("E001", col, tr!("文字列が閉じていません（`\"` が足りません）", "The string is not closed (a `\"` is missing)")));
                    break;
                }
                j = k + 1;
                Tok::Str(s)
            } else if c.is_ascii_digit() {
                // A date, a month and day, a time, a number, or a name that starts with digits
                // and goes on in Japanese (`40営業日以内`).
                if digits_at(&cs, j, 4) && cs.get(j + 4) == Some(&'-') && digits_at(&cs, j + 5, 2) && cs.get(j + 7) == Some(&'-') && digits_at(&cs, j + 8, 2)
                    && !cs.get(j + 10).is_some_and(|c| is_name_char(*c))
                {
                    let (y, m, d) = (num(&cs, j, 4), num(&cs, j + 5, 2), num(&cs, j + 8, 2));
                    j += 10;
                    match Day::from_ymd(y as i64, m, d) {
                        Some(day) => Tok::Date(day),
                        None => {
                            diags.push(
                                err("E006", col, tr!("{y:04}-{m:02}-{d:02} という日付はありません", "There is no date {y:04}-{m:02}-{d:02}"))
                                    .note(tr!("扱える日付は 0001-01-01〜9999-12-31 の暦にある日です。", "A date is a day of the calendar from 0001-01-01 to 9999-12-31.")),
                            );
                            continue;
                        }
                    }
                } else if digits_at(&cs, j, 2) && cs.get(j + 2) == Some(&'-') && digits_at(&cs, j + 3, 2) && !cs.get(j + 5).is_some_and(|c| is_name_char(*c)) {
                    let (m, d) = (num(&cs, j, 2), num(&cs, j + 3, 2));
                    j += 5;
                    if !(1..=12).contains(&m) || d == 0 || d > date::month_len(2000, m) {
                        diags.push(err("E006", col, tr!("{m:02}-{d:02} という月日はありません", "There is no month and day {m:02}-{d:02}")));
                        continue;
                    }
                    Tok::MonthDay(m, d)
                } else if digits_at(&cs, j, 2) && cs.get(j + 2) == Some(&':') && digits_at(&cs, j + 3, 2) && !cs.get(j + 5).is_some_and(|c| is_name_char(*c)) {
                    let (h, mi) = (num(&cs, j, 2), num(&cs, j + 3, 2));
                    j += 5;
                    if h > 23 || mi > 59 {
                        diags.push(err("E006", col, tr!("{h:02}:{mi:02} という時刻はありません", "There is no time {h:02}:{mi:02}"))
                            .note(tr!("時刻は 00:00〜23:59 です。その日の終わりは `at end of day` と書いてください。", "A time is 00:00 to 23:59; the end of the day is `at end of day`.")));
                        continue;
                    }
                    Tok::Time(h, mi)
                } else {
                    let mut k = j;
                    while k < cs.len() && is_name_char(cs[k]) {
                        k += 1;
                    }
                    let w: String = cs[j..k].iter().collect();
                    j = k;
                    if w.chars().all(|c| c.is_ascii_digit()) {
                        if w.len() == 4 && cs.get(k) == Some(&'-') && cs.get(k + 1).is_some_and(|c| c.is_ascii_digit()) {
                            // `2026-1-1`: a date without its zeros.
                            let mut e = k;
                            while e < cs.len() && (cs[e].is_ascii_digit() || cs[e] == '-') {
                                e += 1;
                            }
                            let lit: String = cs[start..e].iter().collect();
                            diags.push(err("E001", col, tr!("日付 `{lit}` の形が崩れています。日付は `2026-01-01` のように、年 4 桁・月 2 桁・日 2 桁で書いてください", "The date `{lit}` is not written right; write a date as `2026-01-01`, with 4, 2 and 2 digits")));
                            j = e;
                            continue;
                        }
                        match w.parse::<i64>() {
                            Ok(n) if n <= 1_000_000_000 => Tok::Int(n),
                            _ => {
                                diags.push(err("E001", col, tr!("数 {w} が大きすぎます", "The number {w} is too large")));
                                continue;
                            }
                        }
                    } else if !w.is_ascii() {
                        Tok::Word(w)
                    } else {
                        diags.push(err("E001", col, tr!("`{w}` は読めません。名前は数字で始められず、数と語のあいだにはスペースが要ります", "`{w}` cannot be read: a name cannot start with a digit, and a number needs a space before the word after it")));
                        continue;
                    }
                }
            } else if is_name_char(c) {
                let mut k = j;
                while k < cs.len() && is_name_char(cs[k]) {
                    k += 1;
                }
                let w: String = cs[j..k].iter().collect();
                j = k;
                if w == crate::kw::SHA256 && cs.get(k) == Some(&':') {
                    let mut e = k + 1;
                    while e < cs.len() && is_name_char(cs[e]) {
                        e += 1;
                    }
                    let hex: String = cs[k + 1..e].iter().collect();
                    j = e;
                    if hex.len() != 16 || !hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
                        diags.push(err("E001", col, tr!("`sha256:{hex}` の形が崩れています。固定は 16 桁の小文字の 16 進数です", "`sha256:{hex}` is not a pin: a pin is 16 lowercase hex digits"))
                            .note(tr!("固定には、コピーの SHA-256 の先頭 16 桁を書いてください（rulec と同じ長さです）。", "It is the first 16 digits of the SHA-256 of the copy, the length rulec uses.")));
                        continue;
                    }
                    Tok::Sha(hex)
                } else {
                    if toks.is_empty() && w == crate::kw::OFFSET && indent == 0 {
                        let mut e = j;
                        while e < cs.len() && cs[e] == ' ' {
                            e += 1;
                        }
                        let r: String = cs[e..].iter().collect::<String>().trim_end().to_string();
                        rest = Some((r, e + 1));
                        toks.push(Token { tok: Tok::Word(w), col, end: j + 1 });
                        break;
                    }
                    Tok::Word(w)
                }
            } else {
                let next = cs.get(j + 1).copied();
                let (t, n) = match (c, next) {
                    ('(', _) => (Some(Tok::LParen), 1),
                    (')', _) => (Some(Tok::RParen), 1),
                    (':', _) => (Some(Tok::Colon), 1),
                    (',', _) => (Some(Tok::Comma), 1),
                    ('|', _) => (Some(Tok::Pipe), 1),
                    ('@', _) => (Some(Tok::At), 1),
                    ('=', _) => (Some(Tok::Eq), 1),
                    ('+', _) => (Some(Tok::Plus), 1),
                    ('.', Some('.')) => (Some(Tok::DotDot), 2),
                    ('-', Some('>')) => (Some(Tok::Arrow), 2),
                    ('-', _) => (Some(Tok::Minus), 1),
                    ('<', Some('=')) => (Some(Tok::Le), 2),
                    ('<', _) => (Some(Tok::Lt), 1),
                    ('>', Some('=')) => (Some(Tok::Ge), 2),
                    ('>', _) => (Some(Tok::Gt), 1),
                    _ => (None, 1),
                };
                j += n;
                match t {
                    Some(t) => t,
                    None => {
                        diags.push(err("E001", col, tr!("`{c}` は koyomi の字句にありません", "`{c}` is not part of the language")));
                        continue;
                    }
                }
            };
            toks.push(Token { tok, col, end: j + 1 });
        }
        lines.push(Line { no, indent, tokens: toks, chars: cs, rest });
    }
    (lines, diags)
}
