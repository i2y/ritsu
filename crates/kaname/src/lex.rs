//! Cutting a `.kn` into lines of tokens (DESIGN 2.1, 2.3).
//!
//! The language is written a line at a time, and indentation makes the blocks, so the lexer keeps
//! lines: each has its indentation and its tokens. Inside parentheses, brackets and braces a line
//! break and the indentation of the next line are not read: the physical lines up to the closing
//! one are one line, so a long call, a record or a list can be written over several.
//!
//! The words, numbers, dates and strings are rulec's (rulec's reference §2): a number is cut out
//! with its multiplier and the unit written right after it (`100_000GBPc`, `1000万円`, `3.49%`), a
//! date is `YYYY-MM-DD`, a string has no escape. `×`, `÷`, `−`, `≦`, `≧` and the full-width digits
//! are read as `*`, `/`, `-`, `<=`, `>=` and the digits.
//!
//! What cannot be read is E001: a character the language does not have, a string not closed, a
//! tab, a full-width space, `==`, `&&`, `||`, a `!` that is not `!=`, a thousands separator, a date
//! that does not exist, a bracket that does not close or closes nothing. One error a physical line:
//! what comes after the first is noise.

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
    /// `GBPc`, `%`, `kg`, `円`; empty for a bare number.
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
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Dot,
    Question,
    Pipe,
    Eq,
    Ne,
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
    /// `=>`
    FatArrow,
}

impl Tok {
    /// The token as the source writes it, for a message.
    pub fn shown(&self) -> String {
        match self {
            Tok::Word(w) => w.clone(),
            Tok::Str(s) => format!("\"{s}\""),
            Tok::Num(n) => n.raw.clone(),
            Tok::Date(d) => crate::date::text(*d),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::LBracket => "[".into(),
            Tok::RBracket => "]".into(),
            Tok::LBrace => "{".into(),
            Tok::RBrace => "}".into(),
            Tok::Colon => ":".into(),
            Tok::Comma => ",".into(),
            Tok::Dot => ".".into(),
            Tok::Question => "?".into(),
            Tok::Pipe => "|".into(),
            Tok::Eq => "=".into(),
            Tok::Ne => "!=".into(),
            Tok::Lt => "<".into(),
            Tok::Le => "<=".into(),
            Tok::Gt => ">".into(),
            Tok::Ge => ">=".into(),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Star => "*".into(),
            Tok::Slash => "/".into(),
            Tok::Arrow => "->".into(),
            Tok::FatArrow => "=>".into(),
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

/// Whether a character continues a name: letters of any script, digits, `_`.
pub fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether a character starts a name: a letter or `_` (rulec's reference §2), or ℃ and ℉, which a
/// type's brackets take as a word (`temperature[℃]`).
fn starts_name(c: char) -> bool {
    c.is_alphabetic() || c == '_' || matches!(c, '℃' | '℉')
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
            if !matches!(c, '℃' | '℉') {
                while k < end && is_name_char(cs[k]) {
                    k += 1;
                }
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
            (',', _) => (Some(Tok::Comma), 1),
            ('.', _) => (Some(Tok::Dot), 1),
            ('?', _) => (Some(Tok::Question), 1),
            ('|', Some('|')) => (None, 2),
            ('|', _) => (Some(Tok::Pipe), 1),
            ('=', Some('=')) => (None, 2),
            ('=', Some('>')) => (Some(Tok::FatArrow), 2),
            ('=', _) => (Some(Tok::Eq), 1),
            ('!', Some('=')) => (Some(Tok::Ne), 2),
            ('<', Some('=')) => (Some(Tok::Le), 2),
            ('<', _) => (Some(Tok::Lt), 1),
            ('≦', _) => (Some(Tok::Le), 1),
            ('>', Some('=')) => (Some(Tok::Ge), 2),
            ('>', _) => (Some(Tok::Gt), 1),
            ('≧', _) => (Some(Tok::Ge), 1),
            ('+', _) => (Some(Tok::Plus), 1),
            ('-', Some('>')) => (Some(Tok::Arrow), 2),
            ('-' | '−', _) => (Some(Tok::Minus), 1),
            ('*' | '×', _) => (Some(Tok::Star), 1),
            ('/' | '÷', _) => (Some(Tok::Slash), 1),
            _ => (None, 1),
        };
        let Some(tok) = tok else {
            let written: String = cs[j..(j + n).min(end)].iter().collect();
            return Cut { tokens, error: Some(err(col, unknown_symbol(&written, next))) };
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
/// the word kaname writes instead.
fn unknown_symbol(written: &str, next: Option<char>) -> Text {
    match (written, next) {
        ("==", _) => tr!("`==` は kaname の字句にありません。等しいかは `=` で書いてください", "`==` is not part of the language; write `=` to compare"),
        ("&", Some('&')) | ("&", _) => tr!("`&&` は kaname の字句にありません。`and` と書いてください", "`&&` is not part of the language; write `and`"),
        ("||", _) => tr!("`||` は kaname の字句にありません。`or` と書いてください", "`||` is not part of the language; write `or`"),
        ("!", _) => tr!("`!` は kaname の字句にありません。`not` と書くか、等しくないことは `!=` で書いてください", "`!` is not part of the language; write `not`, or `!=` for not equal"),
        ("%", _) => tr!("`%` は数のすぐあとに付けて率を書く記号です（`3%`）。余りは `mod(x, n)` で書いてください", "`%` is written right after a number, as a rate (`3%`); a remainder is `mod(x, n)`"),
        (w, _) => tr!("`{w}` は kaname の字句にありません", "`{w}` is not part of the language"),
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

/// A date, or a number with its multiplier and unit, from column `j`: the token and where the next
/// one starts, or why it cannot be read (E001).
fn number(cs: &[char], j: usize, end: usize) -> Result<(Tok, usize), Text> {
    // `2026-04-01`
    let d = |k: usize| cs.get(k).copied().filter(|_| k < end).and_then(ascii_digit);
    if (0..4).all(|k| d(j + k).is_some()) && cs.get(j + 4) == Some(&'-') && d(j + 5).is_some() && d(j + 6).is_some() && cs.get(j + 7) == Some(&'-') && d(j + 8).is_some() && d(j + 9).is_some() {
        let n = |from: usize, len: usize| -> i64 { (from..from + len).filter_map(d).collect::<String>().parse().unwrap_or(0) };
        let (y, m, dd) = (n(j, 4), n(j + 5, 2), n(j + 8, 2));
        if cs.get(j + 10).is_some_and(|c| is_name_char(*c)) && j + 10 < end {
            let lit: String = cs[j..end.min(j + 12)].iter().collect();
            return Err(tr!("`{lit}` を読めません。日付は `2026-04-01` のように書きます", "`{lit}` cannot be read; a date is written as `2026-04-01`"));
        }
        if !crate::date::is_date(y, m, dd) {
            return Err(tr!("{y:04}-{m:02}-{dd:02} という日付はありません", "There is no date {y:04}-{m:02}-{dd:02}"));
        }
        return Ok((Tok::Date(crate::date::day_of(y, m, dd)), j + 10));
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
    // `1,000USD`: a thousands separator, as a document prints a figure (rulec's E049)
    if (1..=3).contains(&whole.len()) && k < end && matches!(cs[k], ',' | '，') && (1..=3).all(|x| d(k + x).is_some()) && d(k + 4).is_none() {
        let mut e = k;
        let mut fixed = whole.clone();
        while e < end && matches!(cs[e], ',' | '，') && (1..=3).all(|x| d(e + x).is_some()) && d(e + 4).is_none() {
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
        return Err(tr!(
            "`{raw}` を読めません。`{unit}` は単位の表にありません（通貨は ISO 4217 のコードか `円`、その 100 分の 1 はコードに `c` を付けて `USDc` のように書きます）",
            "`{raw}` cannot be read: `{unit}` is not a unit of the table (a currency is an ISO 4217 code or `円`, and its hundredth is the code with a `c`, as in `USDc`)"
        ));
    }
    Ok((Tok::Num(Num { value, unit, raw }), k))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        let (lines, diags) = lex("t.kn", src);
        assert!(diags.is_empty(), "{diags:?}");
        lines.into_iter().flat_map(|l| l.tokens.into_iter().map(|t| t.tok)).collect()
    }

    fn n(v: Rat, unit: &str, raw: &str) -> Tok {
        Tok::Num(Num { value: v, unit: unit.into(), raw: raw.into() })
    }

    #[test]
    fn numbers_carry_their_unit() {
        assert_eq!(
            toks("100_000GBPc 1000万円 3.49% 3"),
            vec![n(Rat::int(100_000), "GBPc", "100_000GBPc"), n(Rat::int(10_000_000), "円", "1000万円"), n(Rat::new(349, 100), "%", "3.49%"), n(Rat::int(3), "", "3")]
        );
        assert_eq!(toks("２kg"), vec![n(Rat::int(2), "kg", "２kg")]);
        assert_eq!(toks("2026-04-01"), vec![Tok::Date(crate::date::day_of(2026, 4, 1))]);
        assert_eq!(
            toks("a × b ÷ c − d ≦ e ≧ f"),
            vec![
                Tok::Word("a".into()),
                Tok::Star,
                Tok::Word("b".into()),
                Tok::Slash,
                Tok::Word("c".into()),
                Tok::Minus,
                Tok::Word("d".into()),
                Tok::Le,
                Tok::Word("e".into()),
                Tok::Ge,
                Tok::Word("f".into())
            ]
        );
    }

    #[test]
    fn a_year_minus_a_number_is_a_subtraction() {
        assert_eq!(toks("2026-1"), vec![n(Rat::int(2026), "", "2026"), Tok::Minus, n(Rat::int(1), "", "1")]);
    }

    #[test]
    fn brackets_join_lines() {
        let (lines, diags) = lex("t.kn", "x = f(a,\n      b)\ny\n");
        assert!(diags.is_empty());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].tokens.len(), 8);
    }

    #[test]
    fn what_cannot_be_read() {
        for src in ["x \"open", "50XYZ", "2026-02-30", "2026-1-1", "a == b", "\tx", "a\u{3000}b", "a && b", "1,000USD", "f(a]", ")", "3日"] {
            let (_, diags) = lex("t.kn", src);
            assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E001"], "{src}");
        }
    }
}
