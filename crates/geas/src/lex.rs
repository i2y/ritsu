//! Tokens. A string is one line long, `#` starts a comment, and anything else
//! outside the tokens below is E001.

use crate::diag::{self, Diag};

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Ident,
    Str,
    Num,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Comma,
    Dot,
    Colon,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Tok {
    pub kind: Kind,
    pub text: String,
    pub line: usize,
    pub col: usize,
    /// For a string: the column each of its characters starts at, an escape at its
    /// backslash, and last the column of the closing quote, so that a message about
    /// a command or a pattern can point inside the string. Empty for other tokens.
    pub cols: Vec<usize>,
    /// For a string: the column and the letter of its first backslash before a
    /// character other than `n`, `t`, `"` and `\\`. Both characters are kept in the
    /// text: a pattern reads `\\d` as it is written, and any other string is E001
    /// there (DESIGN §9).
    pub odd: Option<(usize, char)>,
}

impl Tok {
    /// The token as a message quotes it: a string with its quotes, the end of the
    /// file in words.
    pub fn shown(&self) -> (String, String) {
        match self.kind {
            Kind::Eof => ("the end of the file".into(), "ファイルの終わり".into()),
            Kind::Str => {
                let q = format!("`{}`", crate::json::quote(&self.text));
                (q.clone(), q)
            }
            _ => (format!("`{}`", self.text), format!("`{}`", self.text)),
        }
    }
}

fn shown_char(c: char) -> String {
    if c.is_control() || c.is_whitespace() {
        format!("U+{:04X}", c as u32)
    } else {
        format!("`{}`", c)
    }
}

pub fn lex(src: &str) -> Result<Vec<Tok>, Diag> {
    let b: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut col = 1usize;
    while i < b.len() {
        let c = b[i];
        let (l, co) = (line, col);
        match c {
            '\n' => {
                i += 1;
                line += 1;
                col = 1;
            }
            ' ' | '\t' | '\r' => {
                i += 1;
                col += 1;
            }
            '#' => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            }
            '{' | '}' | '(' | ')' | ',' | '.' | ':' => {
                let kind = match c {
                    '{' => Kind::LBrace,
                    '}' => Kind::RBrace,
                    '(' => Kind::LParen,
                    ')' => Kind::RParen,
                    ',' => Kind::Comma,
                    '.' => Kind::Dot,
                    _ => Kind::Colon,
                };
                toks.push(Tok { kind, text: c.to_string(), line: l, col: co, cols: vec![], odd: None });
                i += 1;
                col += 1;
            }
            '"' => {
                i += 1;
                col += 1;
                let mut s = String::new();
                let mut cols = Vec::new();
                let mut odd = None;
                loop {
                    if i >= b.len() || b[i] == '\n' {
                        return Err(diag::error(
                            "E001",
                            l,
                            co,
                            tr!("文字列がこの行のうちに閉じていません", "the string is not closed on this line"),
                        )
                        .note(tr!(
                            "文字列は一行のうちに `\"` で開いて閉じます。中の改行は `\\n` と書きます",
                            "a string opens and closes with `\"` on one line; a line break inside it is written `\\n`",
                        )));
                    }
                    let d = b[i];
                    cols.push(col);
                    if d == '"' {
                        i += 1;
                        col += 1;
                        break;
                    }
                    if d == '\\' {
                        if i + 1 >= b.len() {
                            return Err(diag::error(
                                "E001",
                                line,
                                col,
                                tr!("エスケープの途中でファイルが終わっています", "the file ends in the middle of an escape"),
                            ));
                        }
                        let e = b[i + 1];
                        match e {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            '"' => s.push('"'),
                            '\\' => s.push('\\'),
                            // the line ends inside the string: the check above says so
                            '\n' => {
                                i += 1;
                                continue;
                            }
                            _ => {
                                odd = odd.or(Some((col, e)));
                                s.push('\\');
                                s.push(e);
                                cols.push(col + 1);
                            }
                        }
                        i += 2;
                        col += 2;
                    } else {
                        s.push(d);
                        i += 1;
                        col += 1;
                    }
                }
                toks.push(Tok { kind: Kind::Str, text: s, line: l, col: co, cols, odd });
            }
            c if c.is_ascii_digit()
                || (c == '-' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) =>
            {
                let mut s = String::new();
                s.push(c);
                i += 1;
                col += 1;
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                    s.push(b[i]);
                    i += 1;
                    col += 1;
                }
                toks.push(Tok { kind: Kind::Num, text: s, line: l, col: co, cols: vec![], odd: None });
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut s = String::new();
                s.push(c);
                i += 1;
                col += 1;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_') {
                    s.push(b[i]);
                    i += 1;
                    col += 1;
                }
                toks.push(Tok { kind: Kind::Ident, text: s, line: l, col: co, cols: vec![], odd: None });
            }
            other => {
                let c = shown_char(other);
                return Err(diag::error(
                    "E001",
                    l,
                    co,
                    tr!(
                        "{c} は、文字列とコメントの外には書けない文字です",
                        "{c} is not a character geas reads outside a string or a comment",
                    ),
                ));
            }
        }
    }
    toks.push(Tok { kind: Kind::Eof, text: String::new(), line, col, cols: vec![], odd: None });
    Ok(toks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(Kind, String)> {
        lex(src).unwrap().into_iter().map(|t| (t.kind, t.text)).collect()
    }

    #[test]
    fn a_target_line_and_its_positions() {
        let toks = lex("target calc {\n  run \"python3 calc.py\"\n}\n").unwrap();
        let got: Vec<(Kind, &str, usize, usize)> =
            toks.iter().map(|t| (t.kind.clone(), t.text.as_str(), t.line, t.col)).collect();
        assert_eq!(
            got,
            vec![
                (Kind::Ident, "target", 1, 1),
                (Kind::Ident, "calc", 1, 8),
                (Kind::LBrace, "{", 1, 13),
                (Kind::Ident, "run", 2, 3),
                (Kind::Str, "python3 calc.py", 2, 7),
                (Kind::RBrace, "}", 3, 1),
                (Kind::Eof, "", 4, 1),
            ]
        );
    }

    #[test]
    fn comments_run_to_the_end_of_the_line() {
        assert_eq!(
            kinds("# a comment\nclaim # another\n"),
            vec![(Kind::Ident, "claim".into()), (Kind::Eof, String::new())]
        );
    }

    #[test]
    fn punctuation() {
        let k: Vec<Kind> = kinds("{}(),.:").into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            k,
            vec![
                Kind::LBrace,
                Kind::RBrace,
                Kind::LParen,
                Kind::RParen,
                Kind::Comma,
                Kind::Dot,
                Kind::Colon,
                Kind::Eof
            ]
        );
    }

    #[test]
    fn string_escapes() {
        let src = "\"a\\nb\\tc\\\"d\\\\e\"";
        assert_eq!(kinds(src)[0], (Kind::Str, "a\nb\tc\"d\\e".into()));
    }

    #[test]
    fn numbers_and_names() {
        assert_eq!(
            kinds("8123 -1 1.5 body_json x2"),
            vec![
                (Kind::Num, "8123".into()),
                (Kind::Num, "-1".into()),
                (Kind::Num, "1.5".into()),
                (Kind::Ident, "body_json".into()),
                (Kind::Ident, "x2".into()),
                (Kind::Eof, String::new()),
            ]
        );
    }

    #[test]
    fn an_unknown_escape_is_kept_and_marked() {
        // the parser reads `\d` in a pattern and refuses it anywhere else
        let toks = lex("\"a\\d\\qb\"").unwrap();
        assert_eq!(toks[0].text, "a\\d\\qb");
        assert_eq!(toks[0].odd, Some((3, 'd')));
        assert_eq!(toks[0].cols, vec![2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_string_knows_the_column_of_each_character() {
        // `\"` and `\\` are one character each, at their backslash; the last column is
        // the closing quote's
        let toks = lex("run \"a\\\"b\\\\c\"").unwrap();
        assert_eq!(toks[1].text, "a\"b\\c");
        assert_eq!(toks[1].cols, vec![6, 7, 9, 10, 12, 13]);
    }

    fn err(src: &str) -> (usize, usize, String) {
        let d = lex(src).unwrap_err();
        assert_eq!(d.code, "E001");
        (d.line.unwrap_or(0), d.col.unwrap_or(0), d.message.en)
    }

    #[test]
    fn errors_name_their_place() {
        assert_eq!(err("claim \"adds {\n"), (1, 7, "the string is not closed on this line".into()));
        assert_eq!(err("\"a\\"), (1, 3, "the file ends in the middle of an escape".into()));
        assert_eq!(err("\"a\\\nb\"").2, "the string is not closed on this line");
        assert_eq!(err("\n  @"), (2, 3, "`@` is not a character geas reads outside a string or a comment".into()));
        assert_eq!(err("\u{7}").2, "U+0007 is not a character geas reads outside a string or a comment");
    }
}
