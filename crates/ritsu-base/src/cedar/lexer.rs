//! The tokens of Cedar's policies and schemas, as the grammars of Cedar 4.13.0 match them
//! (`cedar-policy-core/src/parser/grammar.lalrpop` and `validator/cedar_schema/grammar.lalrpop`):
//! white space is Unicode's, a comment runs from `//` to the end of its line, an identifier is
//! `[_a-zA-Z][_a-zA-Z0-9]*`, a number is `[0-9]+` (a minus is an operator), and a string is
//! `"(\\.|[^"\\])*"`, where `.` is any character but a line feed.

use super::Error;
use crate::tr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tok {
    /// An identifier or a keyword: which, the parser decides.
    Ident(String),
    /// The digits as written.
    Num(String),
    /// The text between the quotes, its escapes not yet read.
    Str(String),
    /// `?principal`, `?resource`, or another `?name` (the policy grammar's `OTHER_SLOT`).
    Slot(String),
    /// Punctuation and operators.
    P(&'static str),
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub tok: Tok,
    pub start: usize,
    pub end: usize,
}

/// Which grammar's punctuation to read: the schema grammar has `?`, `=` and `<` `>` but none of
/// the policy's operators.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grammar {
    Policy,
    Schema,
}

/// The punctuation of each grammar, the longer before the shorter that starts it.
const POLICY_PUNCT: [&str; 27] = [
    "::", "==", "!=", "<=", ">=", "||", "&&", "@", ".", ",", ";", ":", "(", ")", "{", "}", "[", "]", "<", ">", "+", "-",
    "*", "/", "%", "!", "=",
];
const SCHEMA_PUNCT: [&str; 15] = ["::", ",", ";", ":", "{", "}", "[", "]", "<", ">", "=", "?", "@", "(", ")"];

/// Lines and columns from 1, a column counting characters (as [`crate::yaml`] counts them).
pub(crate) struct Lines<'a> {
    src: &'a str,
    starts: Vec<usize>,
    /// Whether each line is ASCII, where a column is a byte.
    ascii: Vec<bool>,
}

impl<'a> Lines<'a> {
    pub fn new(src: &'a str) -> Lines<'a> {
        let mut starts = vec![0];
        let mut ascii = vec![true];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
                ascii.push(true);
            } else if !b.is_ascii() {
                *ascii.last_mut().unwrap() = false;
            }
        }
        Lines { src, starts, ascii }
    }

    pub fn at(&self, i: usize) -> (usize, usize) {
        let i = i.min(self.src.len());
        let line = match self.starts.binary_search(&i) {
            Ok(l) => l,
            Err(l) => l - 1,
        };
        let start = self.starts[line];
        let col = if self.ascii[line] { i - start + 1 } else { self.src[start..i].chars().count() + 1 };
        (line + 1, col)
    }

    pub fn err(&self, i: usize, message: crate::text::Text) -> Error {
        let (line, col) = self.at(i);
        Error { line, col, message }
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// The tokens of a text, and where the text between them is (comments and white space).
pub(crate) fn lex(src: &str, grammar: Grammar) -> Result<Vec<Token>, Error> {
    let lines = Lines::new(src);
    let mut out = Vec::new();
    let mut i = 0;
    let b = src.as_bytes();
    while i < src.len() {
        let c = src[i..].chars().next().unwrap();
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        if src[i..].starts_with("//") {
            while i < src.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += src[i..].chars().next().unwrap().len_utf8();
            }
            continue;
        }
        let start = i;
        if is_ident_start(c) {
            let mut j = i;
            while j < src.len() && is_ident_char(b[j] as char) {
                j += 1;
            }
            out.push(Token { tok: Tok::Ident(src[i..j].to_string()), start, end: j });
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < src.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            out.push(Token { tok: Tok::Num(src[i..j].to_string()), start, end: j });
            i = j;
            continue;
        }
        if c == '"' {
            let mut j = i + 1;
            loop {
                if j >= src.len() {
                    return Err(lines.err(start, tr!("閉じていない文字列があります", "a string is not closed")));
                }
                let d = src[j..].chars().next().unwrap();
                if d == '"' {
                    break;
                }
                if d == '\\' {
                    let e = src[j + 1..].chars().next();
                    match e {
                        Some(e) if e != '\n' => j += 1 + e.len_utf8(),
                        _ => return Err(lines.err(j, tr!("文字列の中の `\\` のあとに、文字がありません", "a `\\` in a string with no character after it"))),
                    }
                    continue;
                }
                j += d.len_utf8();
            }
            out.push(Token { tok: Tok::Str(src[i + 1..j].to_string()), start, end: j + 1 });
            i = j + 1;
            continue;
        }
        if c == '?' && grammar == Grammar::Policy {
            let rest = &src[i + 1..];
            if rest.chars().next().is_some_and(is_ident_start) {
                let mut j = i + 1;
                while j < src.len() && is_ident_char(b[j] as char) {
                    j += 1;
                }
                out.push(Token { tok: Tok::Slot(src[i..j].to_string()), start, end: j });
                i = j;
                continue;
            }
            return Err(lines.err(start, tr!("`?` のあとに名前がありません（スロットは `?principal` と `?resource` です）", "a `?` with no name after it (the slots are `?principal` and `?resource`)")));
        }
        let punct: &[&'static str] = match grammar {
            Grammar::Policy => &POLICY_PUNCT,
            Grammar::Schema => &SCHEMA_PUNCT,
        };
        match punct.iter().find(|p| src[i..].starts_with(**p)) {
            Some(p) => {
                out.push(Token { tok: Tok::P(p), start, end: i + p.len() });
                i += p.len();
            }
            None => {
                // a character that does not show is named by its code point
                let shown = if c.is_control() || matches!(c as u32, 0x00A0 | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x2064 | 0xFEFF) {
                    format!("U+{:04X}", c as u32)
                } else {
                    format!("`{c}`")
                };
                return Err(lines.err(start, tr!("{shown} は Cedar で書けない文字です", "{shown} is not a token of Cedar")));
            }
        }
    }
    Ok(out)
}
