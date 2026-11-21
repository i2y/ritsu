//! Cutting a `.ringi` into lines of tokens (DESIGN 3).
//!
//! The language is written a line at a time, and indentation makes the blocks, so the lexer keeps
//! lines: each has its indentation and its tokens. Inside parentheses, brackets and braces a line
//! break and the indentation of the next line are not read: the physical lines up to the closing
//! one are one line, so a long call or the rows of a scenario's form can be written over several.
//!
//! The words, numbers, dates and strings are rulec's, as kaname's are: a number is cut out with its
//! multiplier and the unit written right after it (`30_000JPY`, `1000万円`, `60%`), a date is
//! `YYYY-MM-DD`, a string has no escape. A time is `HH:MM` (`at 09:00`). `→`, `・`, `、`, `，`, `≦`,
//! `≧`, `＜`, `＞`, `＋`, `×`, `÷`, `−` and the full-width digits are read as rulec reads them: `->`,
//! `,` (the first three), `<=`, `>=`, `<`, `>`, `+`, `*`, `/`, `-` and the digits.
//!
//! What cannot be read is E001: a character the language does not have, a string not closed, a
//! tab, a full-width space, `==`, `!=`, `&&`, `||` used as an operator, a thousands separator, a
//! date or a time that does not exist, a bracket that does not close or closes nothing. One error a
//! physical line: what comes after the first is noise.

use crate::ast::Span;
use crate::diag::Diag;
use ritsu_base::text::Text;
use ritsu_units::Rat;

/// A number as written: its value before the unit, the unit (empty for a bare number), and the
/// text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Num {
    /// The magnitude, the multiplier applied (`100万円` is 1000000); never negative (a minus is an
    /// operator).
    pub value: Rat,
    /// `JPY`, `%`, `kg`, `円`; empty for a bare number.
    pub unit: String,
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Word(String),
    Str(String),
    Num(Num),
    /// Days since 1970-01-01.
    Date(i64),
    /// Minutes after midnight: `09:00`.
    Time(u32),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Dot,
    Pipe,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    /// `->`
    Arrow,
}

impl Tok {
    /// The token as the source writes it, for a message.
    pub fn shown(&self) -> String {
        match self {
            Tok::Word(w) => w.clone(),
            Tok::Str(s) => format!("\"{s}\""),
            Tok::Num(n) => n.raw.clone(),
            Tok::Date(d) => date_text(*d),
            Tok::Time(t) => time_text(*t),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::LBracket => "[".into(),
            Tok::RBracket => "]".into(),
            Tok::LBrace => "{".into(),
            Tok::RBrace => "}".into(),
            Tok::Colon => ":".into(),
            Tok::Comma => ",".into(),
            Tok::Dot => ".".into(),
            Tok::Pipe => "|".into(),
            Tok::Eq => "=".into(),
            Tok::Lt => "<".into(),
            Tok::Le => "<=".into(),
            Tok::Gt => ">".into(),
            Tok::Ge => ">=".into(),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Star => "*".into(),
            Tok::Slash => "/".into(),
            Tok::Arrow => "->".into(),
        }
    }

    pub fn is_word(&self, w: &str) -> bool {
        matches!(self, Tok::Word(x) if x == w)
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

/// A line as the parser reads it: one physical line, or several joined inside brackets.
#[derive(Clone, Debug)]
pub struct Line {
    /// The physical line it starts on, from 1.
    pub no: usize,
    /// Leading spaces of that line.
    pub indent: usize,
    pub tokens: Vec<Token>,
}

/// Whether a character continues a name: letters and numbers of any script, `_`. The letters and
/// numbers are those of the tables' Unicode (ritsu-emit's `ident::is_letter_or_number`), whichever
/// Rust built ringi.
pub fn is_name_char(c: char) -> bool {
    ritsu_emit::ident::is_letter_or_number(c) || c == '_'
}

/// Whether a character starts a name: a letter or `_` (rulec's reference §2), or `℃` or `℉`, which
/// Unicode files as symbols but which a unit is written with where a type's brackets take a word
/// (`temperature[℃]`), as rulec and kaname read them.
pub fn starts_name(c: char) -> bool {
    ritsu_emit::ident::is_letter(c) || c == '_' || matches!(c, '℃' | '℉')
}

/// A full-width digit as the digit it is.
fn ascii_digit(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some(c),
        '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32),
        _ => None,
    }
}

/// The characters a unit is written with after a number: ASCII letters (a currency code, `kg`),
/// `円`, `銭`, `坪`, `℃`, `℉`, `%` (and the full-width `％`); a digit after the first (`m2`).
fn is_unit_char(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '円' | '銭' | '坪' | '℃' | '℉' | '%' | '％')
}

/// What one physical line was cut into, and what stopped it.
struct Cut {
    tokens: Vec<Token>,
    error: Option<Diag>,
}

/// Every line of the source as the parser reads it, and what could not be read (E001).
pub fn lex(file: &str, src: &str) -> (Vec<Line>, Vec<Diag>) {
    let mut lines: Vec<Line> = Vec::new();
    let mut diags: Vec<Diag> = Vec::new();
    // the brackets open across physical lines: the character and where it was opened
    let mut open: Vec<(char, Span)> = Vec::new();
    let mut joining: Option<Line> = None;
    for (i, text) in src.lines().enumerate() {
        let no = i + 1;
        let chars: Vec<char> = text.chars().collect();
        let mut indent = 0;
        while indent < chars.len() && chars[indent] == ' ' {
            indent += 1;
        }
        let cut = cut_line(file, src, no, &chars, indent, &mut open);
        if let Some(d) = cut.error {
            diags.push(d);
            // what the line opened is noise after the error: the line ends where it stopped
            open.clear();
        }
        match joining.as_mut() {
            Some(l) => l.tokens.extend(cut.tokens),
            None if cut.tokens.is_empty() => {}
            None => joining = Some(Line { no, indent, tokens: cut.tokens }),
        }
        if open.is_empty()
            && let Some(l) = joining.take()
        {
            lines.push(l);
        }
    }
    if let Some((c, at)) = open.first().copied() {
        diags.push(
            Diag::error("E001", file, at.line, at.col, tr!("`{c}` が閉じていません", "The `{c}` is never closed"))
                .source(src)
                .note(tr!("括弧の中は何行にも書けますが、ファイルの終わりまでに閉じてください。", "What is inside brackets may run over several lines, and has to close before the file ends.")),
        );
        if let Some(l) = joining.take() {
            lines.push(l);
        }
    }
    (lines, diags)
}

fn closing(c: char) -> char {
    match c {
        '(' => ')',
        '[' => ']',
        _ => '}',
    }
}

/// The tokens of one physical line, from column `indent`, with the brackets still open before it.
fn cut_line(file: &str, src: &str, no: usize, cs: &[char], indent: usize, open: &mut Vec<(char, Span)>) -> Cut {
    let mut tokens: Vec<Token> = Vec::new();
    let err = |col: usize, msg: Text| -> Diag { Diag::error("E001", file, no, col, msg).source(src) };
    let mut j = indent;
    // the end of the line before its comment: a `#` outside a string
    let mut end = cs.len();
    {
        let mut in_str = false;
        for (k, c) in cs.iter().enumerate() {
            match c {
                '"' => in_str = !in_str,
                '#' if !in_str => {
                    end = k;
                    break;
                }
                _ => {}
            }
        }
    }
    let at = |from: usize, to: usize| Span { line: no, col: from + 1, end: to + 1 };
    while j < end {
        let c = cs[j];
        let col = j + 1;
        if c == ' ' {
            j += 1;
            continue;
        }
        if c == '\t' {
            return Cut {
                tokens,
                error: Some(
                    err(col, tr!("タブがあります。字下げと区切りは半角のスペースで書いてください", "There is a tab; indent and separate with spaces"))
                        .note(tr!("タブの幅はエディタによって違うので、行がそろっているかを決められません。", "How wide a tab is depends on the editor, so whether the lines line up cannot be told.")),
                ),
            };
        }
        if c == '\u{3000}' {
            return Cut {
                tokens,
                error: Some(err(col, tr!("全角の空白があります。語と語のあいだは半角のスペースで区切ってください", "There is a full-width space; separate words with ASCII spaces"))),
            };
        }
        // a string: every character up to the next `"`, with no escape (rulec's)
        if c == '"' {
            let mut k = j + 1;
            while k < cs.len() && cs[k] != '"' {
                k += 1;
            }
            if k >= cs.len() {
                return Cut {
                    tokens,
                    error: Some(
                        err(col, tr!("文字列が閉じていません（`\"` が足りません）", "The string is not closed (a `\"` is missing)"))
                            .note(tr!("文字列は同じ行のうちに閉じてください。文字列の中に `\"` は書けません。", "A string closes on the line it starts on, and cannot hold a `\"`.")),
                    ),
                };
            }
            let s: String = cs[j + 1..k].iter().collect();
            tokens.push(Token { tok: Tok::Str(s), span: at(j, k + 1) });
            j = k + 1;
            continue;
        }
        if ascii_digit(c).is_some() {
            match number(cs, j, end) {
                Ok((tok, next)) => {
                    tokens.push(Token { tok, span: at(j, next) });
                    j = next;
                    continue;
                }
                Err(msg) => return Cut { tokens, error: Some(err(col, msg)) },
            }
        }
        if starts_name(c) {
            let mut k = j + 1;
            while k < end && is_name_char(cs[k]) {
                k += 1;
            }
            let w: String = cs[j..k].iter().collect();
            tokens.push(Token { tok: Tok::Word(w), span: at(j, k) });
            j = k;
            continue;
        }
        let next = cs.get(j + 1).copied().filter(|_| j + 1 < end);
        let (tok, n): (Option<Tok>, usize) = match (c, next) {
            ('(', _) => (Some(Tok::LParen), 1),
            (')', _) => (Some(Tok::RParen), 1),
            ('[', _) => (Some(Tok::LBracket), 1),
            (']', _) => (Some(Tok::RBracket), 1),
            ('{', _) => (Some(Tok::LBrace), 1),
            ('}', _) => (Some(Tok::RBrace), 1),
            (':', _) => (Some(Tok::Colon), 1),
            (',' | '、' | '，' | '・', _) => (Some(Tok::Comma), 1),
            ('.', _) => (Some(Tok::Dot), 1),
            ('|', _) => (Some(Tok::Pipe), 1),
            ('=', Some('=')) => (None, 2),
            ('=', _) => (Some(Tok::Eq), 1),
            ('!', Some('=')) => (None, 2),
            ('<', Some('=')) => (Some(Tok::Le), 2),
            ('<' | '＜', _) => (Some(Tok::Lt), 1),
            ('≦', _) => (Some(Tok::Le), 1),
            ('>', Some('=')) => (Some(Tok::Ge), 2),
            ('>' | '＞', _) => (Some(Tok::Gt), 1),
            ('≧', _) => (Some(Tok::Ge), 1),
            ('+' | '＋', _) => (Some(Tok::Plus), 1),
            ('-', Some('>')) => (Some(Tok::Arrow), 2),
            ('→', _) => (Some(Tok::Arrow), 1),
            ('-' | '−', _) => (Some(Tok::Minus), 1),
            ('*' | '×', _) => (Some(Tok::Star), 1),
            ('/' | '÷', _) => (Some(Tok::Slash), 1),
            ('&', Some('&')) => (None, 2),
            _ => (None, 1),
        };
        let Some(tok) = tok else {
            let written: String = cs[j..(j + n).min(end)].iter().collect();
            return Cut { tokens, error: Some(err(col, unknown_symbol(&written))) };
        };
        let span = at(j, j + n);
        match tok {
            Tok::LParen | Tok::LBracket | Tok::LBrace => open.push((c, span)),
            Tok::RParen | Tok::RBracket | Tok::RBrace => match open.pop() {
                Some((o, _)) if closing(o) == c => {}
                Some((o, opened)) => {
                    let (want, l) = (closing(o), opened.line);
                    open.clear();
                    return Cut {
                        tokens,
                        error: Some(err(col, tr!("`{c}` は、{l} 行目の `{o}` を閉じられません（`{want}` で閉じます）", "This `{c}` cannot close the `{o}` of line {l}, which `{want}` closes"))),
                    };
                }
                None => return Cut { tokens, error: Some(err(col, tr!("`{c}` が、開いていない括弧を閉じています", "This `{c}` closes a bracket that is not open"))) },
            },
            _ => {}
        }
        tokens.push(Token { tok, span });
        j += n;
    }
    Cut { tokens, error: None }
}

/// What a symbol the language does not have is said with: the operators of other languages get
/// the words ringi writes instead.
fn unknown_symbol(written: &str) -> Text {
    match written {
        "==" => tr!("`==` は ringi の字句にありません。等しいかは `is` で書いてください", "`==` is not part of the language; write `is` to compare"),
        "!=" => tr!("`!=` は ringi の字句にありません。等しくないかは `is not` で書いてください", "`!=` is not part of the language; write `is not`"),
        "&&" | "&" => tr!("`&&` は ringi の字句にありません。`and` と書いてください", "`&&` is not part of the language; write `and`"),
        "!" => tr!("`!` は ringi の字句にありません。`not` と書いてください", "`!` is not part of the language; write `not`"),
        "（" | "）" => tr!("全角の括弧は字句にありません。名前の別名は `種別(category)` のように半角の括弧で書いてください", "A full-width bracket is not part of the language; write an ASCII name in ASCII brackets, as in `種別(category)`"),
        "：" => tr!("全角のコロンは字句にありません。半角の `:` で書いてください", "A full-width colon is not part of the language; write `:`"),
        "｜" => tr!("全角の縦棒は字句にありません。表の区切りは半角の `|` で書いてください", "A full-width bar is not part of the language; separate the cells of a table with `|`"),
        w => tr!("`{w}` は ringi の字句にありません", "`{w}` is not part of the language"),
    }
}

/// `10_000`, `1.5`: the digits as a rational; None past what the language holds.
fn number_value(whole: &str, frac: &str) -> Option<Rat> {
    if whole.len() + frac.len() > 30 {
        return None;
    }
    let n: i128 = format!("{whole}{frac}").parse().ok()?;
    Rat::checked_new(n, 10i128.checked_pow(frac.len() as u32)?)
}

/// The digits from column `k`, full-width ones read as ASCII, with `_` between them: the digits and
/// where they end.
fn digits(cs: &[char], mut k: usize, end: usize) -> (String, usize) {
    let mut out = String::new();
    while k < end {
        if let Some(d) = ascii_digit(cs[k]) {
            out.push(d);
            k += 1;
        } else if cs[k] == '_' && k + 1 < end && ascii_digit(cs[k + 1]).is_some() && !out.is_empty() {
            k += 1;
        } else {
            break;
        }
    }
    (out, k)
}

/// A date, a time, or a number with its multiplier and unit, from column `j`: the token and where
/// the next one starts, or why it cannot be read (E001).
fn number(cs: &[char], j: usize, end: usize) -> Result<(Tok, usize), Text> {
    let d = |k: usize| cs.get(k).copied().filter(|_| k < end).and_then(ascii_digit);
    // `2026-04-01`
    if (0..4).all(|k| d(j + k).is_some()) && cs.get(j + 4) == Some(&'-') && d(j + 5).is_some() && d(j + 6).is_some() && cs.get(j + 7) == Some(&'-') && d(j + 8).is_some() && d(j + 9).is_some() {
        let n = |from: usize, len: usize| -> i64 { (from..from + len).filter_map(d).collect::<String>().parse().unwrap_or(0) };
        let (y, m, dd) = (n(j, 4), n(j + 5, 2), n(j + 8, 2));
        if cs.get(j + 10).is_some_and(|c| is_name_char(*c)) && j + 10 < end {
            let lit: String = cs[j..end.min(j + 12)].iter().collect();
            return Err(tr!("`{lit}` を読めません。日付は `2026-04-01` のように書きます", "`{lit}` cannot be read; a date is written as `2026-04-01`"));
        }
        if !is_date(y, m, dd) {
            return Err(tr!("{y:04}-{m:02}-{dd:02} という日付はありません", "There is no date {y:04}-{m:02}-{dd:02}"));
        }
        return Ok((Tok::Date(day_of(y, m, dd)), j + 10));
    }
    // `09:00`
    if d(j).is_some() && d(j + 1).is_some() && cs.get(j + 2) == Some(&':') && d(j + 3).is_some() && d(j + 4).is_some() && d(j + 5).is_none() {
        let n = |from: usize| -> u32 { (from..from + 2).filter_map(d).collect::<String>().parse().unwrap_or(0) };
        let (h, m) = (n(j), n(j + 3));
        if cs.get(j + 5).is_some_and(|c| is_name_char(*c)) && j + 5 < end {
            let lit: String = cs[j..end.min(j + 7)].iter().collect();
            return Err(tr!("`{lit}` を読めません。時刻は `09:00` のように書きます", "`{lit}` cannot be read; a time is written as `09:00`"));
        }
        if h > 23 || m > 59 {
            return Err(tr!("{h:02}:{m:02} という時刻はありません（00:00 から 23:59 まで）", "There is no time {h:02}:{m:02} (00:00 to 23:59)"));
        }
        return Ok((Tok::Time(h * 60 + m), j + 5));
    }
    let (whole, mut k) = digits(cs, j, end);
    // `2026-1-1`: a date with its zeros left out (two dashes; `2026-1` is a subtraction)
    let dashes = (k..end.min(k + 6)).filter(|x| cs[*x] == '-').count();
    if cs.get(k) == Some(&'-') && k - j == 4 && d(k + 1).is_some() && dashes >= 2 {
        let mut e = k;
        while e < end && (d(e).is_some() || cs[e] == '-') {
            e += 1;
        }
        let lit: String = cs[j..e].iter().collect();
        return Err(tr!(
            "日付 `{lit}` の形が崩れています。日付は `2026-01-01` のように、年 4 桁・月 2 桁・日 2 桁で書いてください",
            "The date `{lit}` is not written right; write a date as `2026-01-01`, with 4, 2 and 2 digits"
        ));
    }
    // `9:00`: a time with its zero left out
    if cs.get(k) == Some(&':') && k - j == 1 && d(k + 1).is_some() {
        let mut e = k + 1;
        while e < end && d(e).is_some() {
            e += 1;
        }
        let lit: String = cs[j..e].iter().collect();
        return Err(tr!("時刻 `{lit}` の形が崩れています。時刻は `09:00` のように、時 2 桁・分 2 桁で書いてください", "The time `{lit}` is not written right; write a time as `09:00`, with 2 and 2 digits"));
    }
    // `1,000JPY`: a thousands separator, as a document prints a figure (rulec's E049)
    if (1..=3).contains(&whole.len()) && k < end && cs[k] == ',' && (1..=3).all(|x| d(k + x).is_some()) && d(k + 4).is_none() {
        let mut e = k;
        let mut fixed = whole.clone();
        while e < end && cs[e] == ',' && (1..=3).all(|x| d(e + x).is_some()) && d(e + 4).is_none() {
            fixed.extend((1..=3).filter_map(|x| d(e + x)));
            e += 4;
        }
        let mut u = e;
        while u < end && (is_unit_char(cs[u]) || (u > e && cs[u].is_ascii_digit())) {
            u += 1;
        }
        let unit: String = cs[e..u].iter().collect();
        let written: String = cs[j..u].iter().collect();
        let fixed = format!("{fixed}{unit}");
        return Err(tr!(
            "桁区切りのカンマは書けません。`{written}` は `{fixed}` と書いてください（桁は `_` で区切れます。`1_000`）",
            "A thousands separator cannot be written: write `{written}` as `{fixed}` (`_` groups the digits: `1_000`)"
        ));
    }
    let mut frac = String::new();
    if cs.get(k) == Some(&'.') && k < end && d(k + 1).is_some() {
        let (f, next) = digits(cs, k + 1, end);
        frac = f;
        k = next;
    }
    let mut mult: i128 = 1;
    match cs.get(k).filter(|_| k < end) {
        Some('万') => {
            mult = 10_000;
            k += 1;
        }
        Some('億') => {
            mult = 100_000_000;
            k += 1;
        }
        Some('兆') => {
            mult = 1_000_000_000_000;
            k += 1;
        }
        _ => {}
    }
    let unit_start = k;
    if k < end && is_unit_char(cs[k]) {
        k += 1;
        while k < end && (is_unit_char(cs[k]) || cs[k].is_ascii_digit()) {
            k += 1;
        }
    }
    let unit: String = cs[unit_start..k].iter().collect::<String>().replace('％', "%");
    let raw: String = cs[j..k].iter().collect();
    // what follows the number goes on as a name (`3日`): no unit, and no name starts with a digit
    if k < end && is_name_char(cs[k]) {
        let mut e = k;
        while e < end && is_name_char(cs[e]) {
            e += 1;
        }
        let w: String = cs[j..e].iter().collect();
        return Err(tr!(
            "`{w}` を読めません。数のあとに単位でないものが続いています。名前は数字で始められません",
            "`{w}` cannot be read: what follows the number is no unit, and a name cannot start with a digit"
        ));
    }
    let too_large = || tr!("数 `{raw}` が大きすぎます", "The number `{raw}` is too large");
    let value = number_value(&whole, &frac).and_then(|v| v.checked_mul(Rat::int(mult))).ok_or_else(too_large)?;
    if !unit.is_empty() && unit != "%" && ritsu_units::table::unit(&unit).is_none() {
        let codes = ritsu_units::table::CURRENCIES.join(" ");
        return Err(tr!(
            "`{raw}` を読めません。`{unit}` は単位の表にありません（通貨は `円` か ISO 4217 のコード {codes} で、100 分の 1 は `円` なら `銭`、コードなら `USDc` のように `c` を付けます）",
            "`{raw}` cannot be read: `{unit}` is not a unit of the table (a currency is `円` or one of the ISO 4217 codes {codes}; the hundredth of `円` is `銭`, and of a code the code with a `c`, as in `USDc`)"
        ));
    }
    Ok((Tok::Num(Num { value, unit, raw }), k))
}

/// Whether the year, month and day are a date of the proleptic Gregorian calendar, 0001 to 9999.
pub fn is_date(y: i64, m: i64, d: i64) -> bool {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 {
        return false;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    d <= days[(m - 1) as usize]
}

/// The day number of a date: days since 1970-01-01 (as rulec, koyomi and kaname count them).
pub fn day_of(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day number, as `YYYY-MM-DD`.
pub fn date_text(n: i64) -> String {
    let z = n + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// A time as `HH:MM`.
pub fn time_text(t: u32) -> String {
    format!("{:02}:{:02}", t / 60, t % 60)
}

/// A rational as a decimal when it is one (`0.1`, `1000`), else as a fraction (`1/3`).
pub fn decimal(r: Rat) -> String {
    if r.den == 1 {
        return r.num.to_string();
    }
    // a decimal ends when the denominator has no prime factor but 2 and 5
    let (mut d, mut places) = (r.den, 0u32);
    while d % 10 == 0 || d % 2 == 0 || d % 5 == 0 {
        if d % 10 == 0 {
            d /= 10;
        } else if d % 2 == 0 {
            d /= 2;
        } else {
            d /= 5;
        }
        places += 1;
        if places > 30 {
            break;
        }
    }
    if d != 1 {
        return format!("{}/{}", r.num, r.den);
    }
    let scale = 10i128.pow(places);
    let scaled = r.num * (scale / r.den);
    let sign = if scaled < 0 { "-" } else { "" };
    let a = scaled.abs();
    let (whole, frac) = (a / scale, a % scale);
    let frac = format!("{frac:0width$}", width = places as usize);
    format!("{sign}{whole}.{}", frac.trim_end_matches('0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        let (lines, diags) = lex("t.ringi", src);
        assert!(diags.is_empty(), "{diags:?}");
        lines.into_iter().flat_map(|l| l.tokens.into_iter().map(|t| t.tok)).collect()
    }

    fn n(v: Rat, unit: &str, raw: &str) -> Tok {
        Tok::Num(Num { value: v, unit: unit.into(), raw: raw.into() })
    }

    #[test]
    fn numbers_carry_their_unit() {
        assert_eq!(toks("30_000JPY"), [n(Rat::int(30_000), "JPY", "30_000JPY")]);
        assert_eq!(toks("1000万円"), [n(Rat::int(10_000_000), "円", "1000万円")]);
        assert_eq!(toks("60%"), [n(Rat::int(60), "%", "60%")]);
        assert_eq!(toks("0.1%"), [n(Rat::new(1, 10), "%", "0.1%")]);
        assert_eq!(toks("２０"), [n(Rat::int(20), "", "２０")]);
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(toks("2026-11-16 10:00"), [Tok::Date(day_of(2026, 11, 16)), Tok::Time(600)]);
        assert_eq!(date_text(day_of(2026, 11, 16)), "2026-11-16");
        assert_eq!(date_text(0), "1970-01-01");
        assert_eq!(toks("at 09:00"), [Tok::Word("at".into()), Tok::Time(540)]);
    }

    #[test]
    fn the_symbols_of_rulec() {
        assert_eq!(toks("| a → b |"), [Tok::Pipe, Tok::Word("a".into()), Tok::Arrow, Tok::Word("b".into()), Tok::Pipe]);
        assert_eq!(toks("a、b・c"), [Tok::Word("a".into()), Tok::Comma, Tok::Word("b".into()), Tok::Comma, Tok::Word("c".into())]);
        assert_eq!(toks("≦ ≧ ＜ ＞"), [Tok::Le, Tok::Ge, Tok::Lt, Tok::Gt]);
        // an empty cell between two bars
        assert_eq!(toks("||"), [Tok::Pipe, Tok::Pipe]);
    }

    #[test]
    fn a_japanese_name_with_digits_is_one_word() {
        assert_eq!(toks("エージェントが決裁するのは3万円まで"), [Tok::Word("エージェントが決裁するのは3万円まで".into())]);
    }

    #[test]
    fn brackets_join_lines() {
        let (lines, diags) = lex("t.ringi", "form a = [{x: 1},\n  {x: 2}]\nnext");
        assert!(diags.is_empty());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].no, 3);
    }

    #[test]
    fn what_cannot_be_read() {
        for src in ["a == b", "a != b", "\"open", "1,000JPY", "2026-02-30", "25:00", "9:00", "a\tb", "a\u{3000}b", "3日", "(", ")"] {
            let (_, diags) = lex("t.ringi", src);
            assert_eq!(diags.len(), 1, "{src}: {diags:?}");
            assert_eq!(diags[0].code, "E001");
        }
    }

    #[test]
    fn decimals() {
        assert_eq!(decimal(Rat::new(1, 10)), "0.1");
        assert_eq!(decimal(Rat::int(1000)), "1000");
        assert_eq!(decimal(Rat::new(1, 3)), "1/3");
        assert_eq!(decimal(Rat::new(-5, 2)), "-2.5");
    }
}
