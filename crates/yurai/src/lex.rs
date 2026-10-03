//! Cutting a `.req` into lines of tokens (DESIGN 1.2, PLAN B.2).
//!
//! The language is written a line at a time, so the lexer keeps the lines: each has its
//! indentation, its tokens, and its text without the comment. A word is cut out as a name and
//! told apart from a keyword by the parser, by position.
//!
//! A naming (DESIGN 2) is cut differently: its names are any run of characters without a
//! space, `"` or `#` (`Order.Line`, `40営業日以内`), so the rest of a line is cut as a naming
//! once the line has begun the way a naming follows — `satisfied by`, `verified by`, `scope`,
//! and `source <name> =` before anything but `law` and `file`.

use crate::date::Day;
use crate::diag::Diag;
use crate::i18n::Text;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Word(String),
    Str(String),
    Date(Day),
    /// `sha256:e880059021fbb67d`: the sixteen hex digits.
    Sha(String),
    LParen,
    RParen,
    Comma,
    At,
    Eq,
    DotDot,
    Arrow,
}

impl Tok {
    /// The token as it would be written.
    pub fn spelled(&self) -> String {
        match self {
            Tok::Word(w) => w.clone(),
            Tok::Str(s) => crate::names::quote(s),
            Tok::Date(d) => d.to_string(),
            Tok::Sha(h) => format!("sha256:{h}"),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::Comma => ",".into(),
            Tok::At => "@".into(),
            Tok::Eq => "=".into(),
            Tok::DotDot => "..".into(),
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
    /// Where the naming starts among the tokens, when the line has one.
    pub naming: Option<usize>,
    /// The column after the line's text, the comment left out.
    pub end: usize,
    /// Whether the lexer found something it could not read on the line.
    pub bad: bool,
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

/// Where a line's comment starts: the first `#` outside a string, or the end.
fn comment_start(cs: &[char]) -> usize {
    let mut in_str = false;
    let mut k = 0;
    while k < cs.len() {
        match cs[k] {
            '\\' if in_str => k += 1,
            '"' => in_str = !in_str,
            '#' if !in_str => return k,
            _ => {}
        }
        k += 1;
    }
    cs.len()
}

/// A string starting at `cs[j] == '"'`: its text and the index after it, or the column and
/// message of what is wrong (E001).
fn string_at(cs: &[char], j: usize) -> Result<(String, usize), (usize, Text)> {
    let mut s = String::new();
    let mut k = j + 1;
    while k < cs.len() {
        match cs[k] {
            '"' => return Ok((s, k + 1)),
            '\\' => match cs.get(k + 1) {
                Some(c @ ('"' | '\\')) => {
                    s.push(*c);
                    k += 2;
                }
                other => {
                    let shown = match other {
                        Some(c) => format!("\\{c}"),
                        None => "\\".to_string(),
                    };
                    return Err((
                        k + 1,
                        tr!(
                            "文字列の中の `{shown}` は読めません。文字列のエスケープは `\\\"` と `\\\\` だけです",
                            "`{shown}` in a string cannot be read; the only escapes are `\\\"` and `\\\\`"
                        ),
                    ));
                }
            },
            c => {
                s.push(c);
                k += 1;
            }
        }
    }
    Err((j + 1, tr!("文字列が閉じていません（`\"` が足りません）", "The string is not closed (a `\"` is missing)")))
}

const WIDE_SPACE: char = '\u{3000}';

fn wide_space(col: usize) -> (usize, Text) {
    (col, tr!("全角の空白があります。語と語のあいだは半角のスペースで区切ります", "There is a full-width space; separate words with ASCII spaces"))
}

/// The tokens of a naming from `cs[j..]`: words and strings only (DESIGN 2.4).
fn naming_from(cs: &[char], mut j: usize, toks: &mut Vec<Token>) -> Result<(), (usize, Text)> {
    while j < cs.len() {
        let c = cs[j];
        if c == WIDE_SPACE {
            return Err(wide_space(j + 1));
        }
        if c.is_whitespace() {
            j += 1;
            continue;
        }
        let col = j + 1;
        if c == '"' {
            let (s, k) = string_at(cs, j)?;
            toks.push(Token { tok: Tok::Str(s), col, end: k + 1 });
            j = k;
            continue;
        }
        let mut k = j;
        while k < cs.len() && !cs[k].is_whitespace() && cs[k] != '"' {
            k += 1;
        }
        toks.push(Token { tok: Tok::Word(cs[j..k].iter().collect()), col, end: k + 1 });
        j = k;
    }
    Ok(())
}

/// The tokens of a naming written on its own (a line of `tests/fixtures/naming.tsv`).
pub fn naming_tokens(s: &str) -> Result<Vec<Token>, (usize, Text)> {
    let cs: Vec<char> = s.chars().collect();
    let end = comment_start(&cs);
    let mut toks = Vec::new();
    naming_from(&cs[..end], 0, &mut toks)?;
    Ok(toks)
}

/// The word that starts at `cs[j..]` after spaces, for looking ahead.
fn word_ahead(cs: &[char], mut j: usize) -> String {
    while j < cs.len() && cs[j] == ' ' {
        j += 1;
    }
    let mut k = j;
    while k < cs.len() && is_name_char(cs[k]) {
        k += 1;
    }
    cs[j..k].iter().collect()
}

/// Whether the tokens so far begin a naming: the rest of the line is then cut as one.
fn naming_begins(toks: &[Token], indent: usize, cs: &[char], next: usize) -> bool {
    let w = |i: usize, s: &str| toks.get(i).is_some_and(|t| t.tok.is_word(s));
    match toks.len() {
        1 => indent == 0 && w(0, crate::kw::SCOPE),
        2 => (w(0, crate::kw::SATISFIED) || w(0, crate::kw::VERIFIED)) && w(1, crate::kw::BY),
        3 => {
            indent == 0 && w(0, crate::kw::SOURCE) && matches!(toks[1].tok, Tok::Word(_)) && toks[2].tok == Tok::Eq && {
                let a = word_ahead(cs, next);
                a != crate::kw::LAW && a != crate::kw::FILE
            }
        }
        _ => false,
    }
}

/// Every line of the source, and what could not be read (E001, E005, E006). `file` is how
/// diagnostics name the file, `rel` the same from the root.
pub fn lex(file: &str, rel: &str, src: &str) -> (Vec<Line>, Vec<Diag>) {
    let mut lines = Vec::new();
    let mut diags = Vec::new();
    for (i, text) in src.lines().enumerate() {
        let no = i + 1;
        let all: Vec<char> = text.chars().collect();
        let err = |code: &'static str, col: usize, msg: Text| Diag::error(code, file, rel, no, col, msg).source(src);
        // Indentation is spaces. A tab is refused: how wide it is depends on the editor, and the
        // lines of a block are told apart by lining up (E005).
        let mut indent = 0;
        let mut bad = false;
        while indent < all.len() && (all[indent] == ' ' || all[indent] == '\t') {
            if all[indent] == '\t' && !bad {
                bad = true;
                diags.push(
                    err("E005", indent + 1, tr!("字下げにタブがあります。字下げはスペースで書きます", "The indentation has a tab; indent with spaces")).note(tr!(
                        "タブの幅はエディタによって違うので、行がそろっているかを決められません。",
                        "How wide a tab is depends on the editor, so whether the lines line up cannot be told."
                    )),
                );
            }
            indent += 1;
        }
        let end = comment_start(&all);
        let cs: Vec<char> = all[..end].to_vec();
        let mut toks: Vec<Token> = Vec::new();
        let mut naming = None;
        let mut j = indent;
        while j < cs.len() {
            if naming_begins(&toks, indent, &cs, j) {
                naming = Some(toks.len());
                if let Err((col, msg)) = naming_from(&cs, j, &mut toks) {
                    let code = "E001";
                    diags.push(err(code, col, msg));
                    bad = true;
                }
                break;
            }
            let c = cs[j];
            let col = j + 1;
            if c == ' ' || c == '\t' {
                j += 1;
                continue;
            }
            if c == WIDE_SPACE {
                let (col, msg) = wide_space(col);
                diags.push(err("E001", col, msg));
                bad = true;
                break;
            }
            let tok = if c == '"' {
                match string_at(&cs, j) {
                    Ok((s, k)) => {
                        j = k;
                        Tok::Str(s)
                    }
                    Err((col, msg)) => {
                        diags.push(err("E001", col, msg));
                        bad = true;
                        break;
                    }
                }
            } else if c.is_ascii_digit() {
                // A date, or a name that starts with digits and goes on in Japanese (`142条`).
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
                            bad = true;
                            break;
                        }
                    }
                } else {
                    let mut k = j;
                    while k < cs.len() && is_name_char(cs[k]) {
                        k += 1;
                    }
                    let w: String = cs[j..k].iter().collect();
                    if w.chars().all(|c| c.is_ascii_digit()) && cs.get(k) == Some(&'-') && cs.get(k + 1).is_some_and(|c| c.is_ascii_digit()) {
                        // `2026-1-3`: a date without its zeros.
                        let mut e = k;
                        while e < cs.len() && (cs[e].is_ascii_digit() || cs[e] == '-') {
                            e += 1;
                        }
                        let lit: String = cs[j..e].iter().collect();
                        diags.push(err("E001", col, tr!(
                            "日付 `{lit}` の形が崩れています。日付は `2026-10-03` のように、年 4 桁・月 2 桁・日 2 桁で書きます",
                            "The date `{lit}` is not written right; write a date as `2026-10-03`, with 4, 2 and 2 digits"
                        )));
                        bad = true;
                        break;
                    }
                    j = k;
                    if !w.is_ascii() {
                        Tok::Word(w)
                    } else {
                        diags.push(err("E001", col, tr!(
                            "`{w}` は読めません。名前は ASCII の数字だけで始められず、数のあとの語にはスペースが要ります",
                            "`{w}` cannot be read: a name in ASCII cannot start with a digit, and a number needs a space before the word after it"
                        )));
                        bad = true;
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
                if w == crate::kw::SHA256 && cs.get(k) == Some(&':') {
                    let mut e = k + 1;
                    while e < cs.len() && is_name_char(cs[e]) {
                        e += 1;
                    }
                    let hex: String = cs[k + 1..e].iter().collect();
                    j = e;
                    if hex.len() != 16 || !hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
                        diags.push(
                            err("E001", col, tr!("`sha256:{hex}` の形が崩れています。ハッシュは 16 桁の小文字の 16 進数です", "`sha256:{hex}` is not a hash: it is 16 lowercase hex digits"))
                                .note(tr!("SHA-256 の先頭 16 桁を書きます（rulec と koyomi の固定と同じ長さ）。", "It is the first 16 digits of a SHA-256, the length of rulec's and koyomi's pins.")),
                        );
                        bad = true;
                        break;
                    }
                    Tok::Sha(hex)
                } else {
                    Tok::Word(w)
                }
            } else {
                let next = cs.get(j + 1).copied();
                let (t, n) = match (c, next) {
                    ('(', _) => (Some(Tok::LParen), 1),
                    (')', _) => (Some(Tok::RParen), 1),
                    (',', _) => (Some(Tok::Comma), 1),
                    ('@', _) => (Some(Tok::At), 1),
                    ('=', _) => (Some(Tok::Eq), 1),
                    ('.', Some('.')) => (Some(Tok::DotDot), 2),
                    ('-', Some('>')) => (Some(Tok::Arrow), 2),
                    _ => (None, 1),
                };
                j += n;
                match t {
                    Some(t) => t,
                    None => {
                        diags.push(err("E001", col, tr!("`{c}` は yurai の字句にありません", "`{c}` is not part of the language")).note(tr!(
                            "名前に使えるのは文字・数字・`_` です。ほかの文字を含む条や名前は `\"…\"` で囲みます。",
                            "A name is letters, digits and `_`; quote an article or a name with other characters, as `\"…\"`."
                        )));
                        bad = true;
                        break;
                    }
                }
            };
            toks.push(Token { tok, col, end: j + 1 });
        }
        // A line that ends where a naming begins (`satisfied by` and nothing after it).
        if naming.is_none() && naming_begins(&toks, indent, &cs, cs.len()) {
            naming = Some(toks.len());
        }
        lines.push(Line { no, indent, tokens: toks, naming, end: cs.len() + 1, bad });
    }
    (lines, diags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(src: &str) -> Vec<Vec<String>> {
        let (lines, diags) = lex("t.req", "t.req", src);
        assert!(diags.is_empty(), "{:?}", diags.iter().map(|d| d.render(crate::i18n::Lang::En)).collect::<Vec<_>>());
        lines.iter().map(|l| l.tokens.iter().map(|t| t.tok.spelled()).collect()).collect()
    }

    #[test]
    fn a_naming_is_cut_as_words() {
        let w = words("  satisfied by proto \"shop/v1/order.proto\" message Order.Line field quantity # c\nscope koyomi \"a.cal\" claim 40営業日以内\n");
        assert_eq!(w[0], ["satisfied", "by", "proto", "\"shop/v1/order.proto\"", "message", "Order.Line", "field", "quantity"]);
        assert_eq!(w[1], ["scope", "koyomi", "\"a.cal\"", "claim", "40営業日以内"]);
    }

    #[test]
    fn a_source_is_cut_by_what_follows_the_equals() {
        let w = words("source 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第140条 sha256:e880059021fbb67d\nsource 民法 = koyomi \"民法の期間.cal\" source 民法\n");
        assert_eq!(w[0], ["source", "民法", "=", "law", "\"129AC0000000089\"", "asof", "2026-10-01"]);
        assert_eq!(w[1], ["第140条", "sha256:e880059021fbb67d"]);
        assert_eq!(w[2], ["source", "民法", "=", "koyomi", "\"民法の期間.cal\"", "source", "民法"]);
    }

    #[test]
    fn periods_records_and_aliases() {
        let w = words("requirement 満了日_142条(last_day_142) v1\n  in force 2026-10-01..\n  in force ..2027-03-31\n    reviewed 2026-10-03 by 法務 sha256:0575c131b9f08063, sha256:6950bdfb988439b6 -> sha256:465b83ed8c251406\n");
        assert_eq!(w[0], ["requirement", "満了日_142条", "(", "last_day_142", ")", "v1"]);
        assert_eq!(w[1], ["in", "force", "2026-10-01", ".."]);
        assert_eq!(w[2], ["in", "force", "..", "2027-03-31"]);
        assert_eq!(w[3][5], ",");
        assert_eq!(w[3][7], "->");
    }

    #[test]
    fn what_cannot_be_read() {
        for (src, code) in [
            ("text \"open\n", "E001"),
            ("text \"a\\nb\"\n", "E001"),
            ("x 30days\n", "E001"),
            ("x 2026-1-3\n", "E001"),
            ("x sha256:abc\n", "E001"),
            ("x 2026-02-30\n", "E006"),
            ("\tx\n", "E005"),
            ("x\u{3000}y\n", "E001"),
            ("x ! y\n", "E001"),
        ] {
            let (_, diags) = lex("t.req", "t.req", src);
            assert_eq!(diags.first().map(|d| d.code), Some(code), "{src:?}");
        }
    }
}
