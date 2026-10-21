//! Cutting a `.gate` into lines of tokens (DESIGN 9).
//!
//! The language is written a line at a time, so the lexer keeps the lines: each has its
//! indentation, its tokens, and its text without the comment. A word is cut out as a word and told
//! apart from a keyword by the parser, by position. A number is cut out with the unit written
//! right after it (`50GBP`, `100万円`, `5%`, `-40℃`), as rulec and dandori write one, with the
//! characters ritsu-units reads a unit with (`ritsu_units::chars`; the full-width `％` is `%`); a run
//! of digits that goes on in Japanese and is no unit is a name (`3日以内`). ℃ and ℉, which are no
//! letters, are a word of their own right after a type's `[` (`temperature[℃]`), and never part of
//! a name, which Cedar and the code generated from the gate could not spell.
//!
//! What cannot be read is E001 (a character the language does not have, a sign of a unit where no
//! unit goes, a string not closed, a full-width space, a date or a number of the wrong shape); a
//! tab in the indentation is E004. One error a line: what comes after the first is noise.

use crate::ast::Num;
use crate::diag::Diag;
use ritsu_ports::Day;
use ritsu_units::Rat;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    Str(String),
    Num(Num),
    Date(Day),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    /// `::`, between the segments of a namespace.
    ColonColon,
    Comma,
    Dot,
    Pipe,
    Question,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    Minus,
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
    /// For a `today` line: the text after `offset`, as written, and its column. A time zone's name
    /// (`Europe/London`) is not made of tokens, and it deserves E107 rather than E001.
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

pub fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn digits_at(cs: &[char], i: usize, n: usize) -> bool {
    i + n <= cs.len() && cs[i..i + n].iter().all(|c| c.is_ascii_digit())
}

fn num(cs: &[char], i: usize, n: usize) -> i64 {
    cs[i..i + n].iter().collect::<String>().parse().unwrap_or(0)
}

/// `10_000`, `1.5`: the digits as a rational; None past what the language holds.
fn number_value(digits: &str) -> Option<Rat> {
    let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let whole: String = whole.chars().filter(|c| *c != '_').collect();
    if whole.len() + frac.len() > 30 {
        return None;
    }
    let n: i128 = format!("{whole}{frac}").parse().ok()?;
    Rat::checked_new(n, 10i128.checked_pow(frac.len() as u32)?)
}

/// What a number is followed by, when it is a unit: the multiplier (`万`, `億`) and the unit as
/// the table of units spells it. None when the letters are no unit.
fn unit_after(run: &str) -> Option<(i128, String)> {
    let (mult, rest) = match run.chars().next() {
        Some('万') => (10_000, &run['万'.len_utf8()..]),
        Some('億') => (100_000_000, &run['億'.len_utf8()..]),
        _ => (1, run),
    };
    let known = ritsu_units::table::unit(rest).is_some();
    (known && !rest.is_empty()).then(|| (mult, rest.to_string()))
}

/// Every line of the source, and what could not be read (E001, E004).
pub fn lex(file: &str, src: &str) -> (Vec<Line>, Vec<Diag>) {
    let mut lines = Vec::new();
    let mut diags: Vec<Diag> = Vec::new();
    for (i, text) in src.lines().enumerate() {
        let no = i + 1;
        let all: Vec<char> = text.chars().collect();
        let err = |code: &'static str, col: usize, msg: ritsu_base::text::Text| Diag::error(code, file, no, col, msg).source(src);
        // Indentation is spaces. A tab is refused: how wide it is depends on the editor, and the
        // lines of a block are told apart by lining up (E004).
        let mut indent = 0;
        let mut tab = false;
        while indent < all.len() && (all[indent] == ' ' || all[indent] == '\t') {
            if all[indent] == '\t' {
                diags.push(
                    err("E004", indent + 1, tr!("字下げにタブがあります。字下げはスペースで書いてください", "The indentation has a tab; indent with spaces")).note(tr!(
                        "タブの幅はエディタによって違うので、行がそろっているかを決められません。",
                        "How wide a tab is depends on the editor, so whether the lines line up cannot be told."
                    )),
                );
                tab = true;
                break;
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
        let mut toks: Vec<Token> = Vec::new();
        let mut rest = None;
        let mut j = indent;
        while j < cs.len() && !tab {
            let c = cs[j];
            let col = j + 1;
            if c == ' ' || c == '\t' {
                j += 1;
                continue;
            }
            if c == '\u{3000}' {
                diags.push(err("E001", col, tr!("全角の空白があります。語と語のあいだは、半角のスペースで区切ってください", "There is a full-width space; separate words with ASCII spaces")));
                break;
            }
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
                match number(&cs, j) {
                    Ok((t, next)) => {
                        j = next;
                        t
                    }
                    Err(msg) => {
                        diags.push(err("E001", col, msg));
                        break;
                    }
                }
            } else if is_name_char(c) {
                let mut k = j;
                while k < cs.len() && is_name_char(cs[k]) {
                    k += 1;
                }
                let w: String = cs[j..k].iter().collect();
                j = k;
                // `today range … offset +00:00`: the offset is read as written (E107 for a zone's name)
                if w == crate::kw::OFFSET && indent == 0 && toks.first().is_some_and(|t| t.tok == Tok::Word(crate::kw::TODAY.into())) {
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
            } else if ritsu_units::chars::is_word_sign(c) && toks.last().is_some_and(|t| t.tok == Tok::LBracket) {
                // `temperature[℃]`: the unit of a type, a word of its own
                j += 1;
                Tok::Word(c.to_string())
            } else if ritsu_units::chars::is_sign(c) {
                diags.push(err("E001", col, misplaced_sign(c)));
                break;
            } else {
                let next = cs.get(j + 1).copied();
                let (t, n) = match (c, next) {
                    ('(', _) => (Some(Tok::LParen), 1),
                    (')', _) => (Some(Tok::RParen), 1),
                    ('[', _) => (Some(Tok::LBracket), 1),
                    (']', _) => (Some(Tok::RBracket), 1),
                    (':', Some(':')) => (Some(Tok::ColonColon), 2),
                    (':', _) => (Some(Tok::Colon), 1),
                    (',', _) => (Some(Tok::Comma), 1),
                    ('.', _) => (Some(Tok::Dot), 1),
                    ('|', Some('|')) => (None, 2),
                    ('|', _) => (Some(Tok::Pipe), 1),
                    ('?', _) => (Some(Tok::Question), 1),
                    ('=', Some('=')) => (None, 2),
                    ('=', _) => (Some(Tok::Eq), 1),
                    ('<', Some('=')) => (Some(Tok::Le), 2),
                    ('<', _) => (Some(Tok::Lt), 1),
                    ('>', Some('=')) => (Some(Tok::Ge), 2),
                    ('>', _) => (Some(Tok::Gt), 1),
                    ('-', _) => (Some(Tok::Minus), 1),
                    _ => (None, 1),
                };
                match t {
                    Some(t) => {
                        j += n;
                        t
                    }
                    None => {
                        let written: String = cs[j..(j + n).min(cs.len())].iter().collect();
                        diags.push(err("E001", col, unknown_symbol(&written, next)));
                        break;
                    }
                }
            };
            toks.push(Token { tok, col, end: j + 1 });
        }
        lines.push(Line { no, indent, tokens: toks, chars: cs, rest });
    }
    (lines, diags)
}

/// Whether `s` can stand bare as a word of a `.gate`: whether this lexer reads an indented line that
/// holds `s` alone back as that one word. The question is the lexer's, so it is asked of [`lex`]
/// itself (DESIGN 18): `refundOrder`, `get_order` and `3日以内` are words to it; `refund-order`,
/// `POST /orders`, `Orders/Refund`, `2fa` and `5kg` are not, and where a line takes a word or a
/// string, it writes one of them in quotes.
pub fn is_bare_word(s: &str) -> bool {
    let (lines, diags) = lex("", &format!("  {s}"));
    diags.is_empty() && matches!(lines.as_slice(), [l] if matches!(l.tokens.as_slice(), [t] if t.tok == Tok::Word(s.to_string())))
}

/// A word as a `.gate` line writes it where the line takes a word or a string: bare when
/// [`is_bare_word`], else in quotes (`"…"`, with `\"` and `\\` inside).
pub fn spelled_word(s: &str) -> String {
    if is_bare_word(s) { s.to_string() } else { ritsu_base::naming::quote(s) }
}

/// What a symbol the language does not have is said with; Cedar's operators get the word sekisho
/// writes instead.
fn unknown_symbol(written: &str, next: Option<char>) -> ritsu_base::text::Text {
    match (written, next) {
        ("==", _) => tr!("`==` は sekisho の字句にありません。等しいことは `is` で書いてください", "`==` is not part of the language; write `is` for the same"),
        ("!", Some('=')) => tr!("`!=` は sekisho の字句にありません。等しくないことは `is not` で書いてください", "`!=` is not part of the language; write `is not`"),
        ("&", Some('&')) => tr!("`&&` は sekisho の字句にありません。`and` と書いてください", "`&&` is not part of the language; write `and`"),
        ("||", _) => tr!("`||` は sekisho の字句にありません。`or` と書いてください", "`||` is not part of the language; write `or`"),
        ("!", _) => tr!("`!` は sekisho の字句にありません。`not` と書いてください", "`!` is not part of the language; write `not`"),
        (w, _) => tr!("`{w}` は sekisho の字句にありません", "`{w}` is not part of the language"),
    }
}

/// What a sign of a unit is said with where no unit goes: after no number, and, for ℃ and ℉, not
/// right after a type's `[`.
fn misplaced_sign(c: char) -> ritsu_base::text::Text {
    if ritsu_units::chars::is_word_sign(c) {
        tr!(
            "`{c}` は単位の記号です。数のすぐあと（`5{c}`）か、型の `[` のすぐあと（`temperature[{c}]`）に書いてください",
            "`{c}` is the sign of a unit: write it right after a number (`5{c}`) or right after a type's `[` (`temperature[{c}]`)"
        )
    } else {
        tr!(
            "`{c}` は、数のすぐあとに付けて率を書く記号です（`5%`）。率の型には、`rate[step 1%]` のように刻みを書いてください",
            "`{c}` is written right after a number, as a rate (`5%`); the type of a rate writes its step, as in `rate[step 1%]`"
        )
    }
}

/// A date, a number with its unit, or a name that starts with digits, from column `j`: the token
/// and where the next one starts, or why it cannot be read (E001).
fn number(cs: &[char], j: usize) -> Result<(Tok, usize), ritsu_base::text::Text> {
    // `2026-10-01`
    if digits_at(cs, j, 4) && cs.get(j + 4) == Some(&'-') && digits_at(cs, j + 5, 2) && cs.get(j + 7) == Some(&'-') && digits_at(cs, j + 8, 2) && !cs.get(j + 10).is_some_and(|c| is_name_char(*c)) {
        let (y, m, d) = (num(cs, j, 4), num(cs, j + 5, 2) as u32, num(cs, j + 8, 2) as u32);
        return match crate::types::day(y, m, d) {
            Some(day) => Ok((Tok::Date(day), j + 10)),
            None => Err(tr!("{y:04}-{m:02}-{d:02} という日付はありません", "There is no date {y:04}-{m:02}-{d:02}")),
        };
    }
    // the digits, with `_` between them and a fraction after a `.`
    let mut k = j;
    while k < cs.len() && (cs[k].is_ascii_digit() || (cs[k] == '_' && cs.get(k + 1).is_some_and(|c| c.is_ascii_digit()))) {
        k += 1;
    }
    if cs.get(k) == Some(&'-') && k - j == 4 && cs.get(k + 1).is_some_and(|c| c.is_ascii_digit()) {
        // `2026-1-1`: a date without its zeros
        let mut e = k;
        while e < cs.len() && (cs[e].is_ascii_digit() || cs[e] == '-') {
            e += 1;
        }
        let lit: String = cs[j..e].iter().collect();
        return Err(tr!(
            "日付 `{lit}` の形が崩れています。日付は `2026-01-01` のように、年 4 桁・月 2 桁・日 2 桁で書いてください",
            "The date `{lit}` is not written right; write a date as `2026-01-01`, with 4, 2 and 2 digits"
        ));
    }
    if cs.get(k) == Some(&'.') && cs.get(k + 1).is_some_and(|c| c.is_ascii_digit()) {
        k += 1;
        while k < cs.len() && cs[k].is_ascii_digit() {
            k += 1;
        }
    }
    let digits: String = cs[j..k].iter().collect();
    // what follows: a unit, or the rest of a name
    let mut e = k;
    if e < cs.len() && ritsu_units::chars::is_sign(cs[e]) {
        e += 1;
    } else {
        while e < cs.len() && is_name_char(cs[e]) {
            e += 1;
        }
    }
    let run: String = ritsu_units::chars::table_spelling(&cs[k..e].iter().collect::<String>());
    let raw: String = cs[j..e].iter().collect();
    let too_large = || tr!("数 `{raw}` が大きすぎます", "The number `{raw}` is too large");
    if run.is_empty() {
        let value = number_value(&digits).ok_or_else(too_large)?;
        return Ok((Tok::Num(Num { value, unit: String::new(), raw }), e));
    }
    if let Some((mult, unit)) = unit_after(&run) {
        let value = number_value(&digits).and_then(|v| v.checked_mul(Rat::int(mult))).ok_or_else(too_large)?;
        return Ok((Tok::Num(Num { value, unit, raw }), e));
    }
    // a name that starts with digits and goes on in Japanese (`3日以内`)
    if !run.is_ascii() && digits.chars().all(|c| c.is_ascii_digit()) {
        return Ok((Tok::Word(raw), e));
    }
    Err(tr!(
        "`{raw}` を読めません。`{run}` は単位の表にありません。名前は数字で始められません（日本語の名前は始められます）",
        "`{raw}` cannot be read: `{run}` is not a unit of the table, and a name cannot start with a digit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        let (lines, diags) = lex("t.gate", src);
        assert!(diags.is_empty(), "{diags:?}");
        lines.into_iter().flat_map(|l| l.tokens.into_iter().map(|t| t.tok)).collect()
    }

    fn n(v: i128, unit: &str, raw: &str) -> Tok {
        Tok::Num(Num { value: Rat::int(v), unit: unit.into(), raw: raw.into() })
    }

    #[test]
    fn numbers_carry_the_unit_written_after_them() {
        assert_eq!(toks("10_000GBP 100万円 5% 3"), vec![n(10_000, "GBP", "10_000GBP"), n(1_000_000, "円", "100万円"), n(5, "%", "5%"), n(3, "", "3")]);
        assert_eq!(toks("1.5kg"), vec![Tok::Num(Num { value: Rat::new(3, 2), unit: "kg".into(), raw: "1.5kg".into() })]);
        assert_eq!(toks("3日以内"), vec![Tok::Word("3日以内".into())]);
        assert_eq!(toks("2026-10-01"), vec![Tok::Date(crate::types::day(2026, 10, 1).unwrap())]);
        // the signs of the table after a number, the full-width `％` as `%`
        assert_eq!(toks("-40℃ 41℉ 5％"), vec![Tok::Minus, n(40, "℃", "40℃"), n(41, "℉", "41℉"), n(5, "%", "5％")]);
    }

    /// ℃ and ℉ are no letters, and stand as a word of their own right after a type's `[`, as rulec
    /// reads them: `temperature[℃]` is a type, as `mass[kg]` is.
    #[test]
    fn a_sign_of_temperature_is_a_word_after_a_bracket() {
        for (src, sign) in [("temperature[℃]", "℃"), ("temperature[ ℉ ]", "℉")] {
            assert_eq!(toks(src), vec![Tok::Word("temperature".into()), Tok::LBracket, Tok::Word(sign.into()), Tok::RBracket], "{src}");
        }
        assert_eq!(toks("rate[step 1%]"), vec![Tok::Word("rate".into()), Tok::LBracket, Tok::Word("step".into()), n(1, "%", "1%"), Tok::RBracket]);
    }

    #[test]
    fn what_cannot_be_read() {
        for (src, code) in [("x \"open", "E001"), ("50XYZ", "E001"), ("2026-02-30", "E001"), ("2026-1-1", "E001"), ("a == b", "E001"), ("\tx", "E004"), ("a\u{3000}b", "E001"), ("x℃ : bool", "E001"), ("rate[%]", "E001"), ("rate[％]", "E001"), ("℉", "E001")] {
            let (_, diags) = lex("t.gate", src);
            assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec![code], "{src}");
        }
    }

    /// A word is bare when the lexer reads it back; else a line writes it in quotes, and the line
    /// reads the quotes back as the word.
    #[test]
    fn a_word_is_bare_when_the_lexer_reads_it_back() {
        for s in ["refundOrder", "get_order", "Café", "today", "3日以内", "返金する"] {
            assert!(is_bare_word(s), "{s}");
            assert_eq!(spelled_word(s), s);
        }
        for s in ["refund-order", "POST /orders/{orderId}/refunds", "Orders/Refund", "receive.do", "2fa", "5kg", "10", "a#b", "a\"b", "a\\b", "保留、手動", "保留\u{3000}中", "℃", ""] {
            assert!(!is_bare_word(s), "{s}");
            let (lines, diags) = lex("t.gate", &format!("  guards orders {}\n", spelled_word(s)));
            assert!(diags.is_empty(), "{s}: {diags:?}");
            assert_eq!(lines[0].tokens[2].tok, Tok::Str(s.to_string()), "{s} reads back from its quotes");
        }
    }

    #[test]
    fn the_offset_is_read_as_written() {
        let (lines, diags) = lex("t.gate", "today range >=2026-10-01 <=2028-10-31 offset Europe/London # a zone");
        assert!(diags.is_empty());
        assert_eq!(lines[0].rest.as_ref().map(|r| r.0.as_str()), Some("Europe/London"));
    }
}
