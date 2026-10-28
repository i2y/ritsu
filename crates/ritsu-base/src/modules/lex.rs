//! The words of one line of `ritsu.mod`, `ritsu.lock` and a list of modules (DESIGN 17.3): the
//! lexis of ritsu's languages. A word is a run of characters with no blank, `"` or `#` in it; a
//! string is `"…"`, whose only escapes are `\"` and `\\`; a `#` outside a string starts a comment
//! that runs to the end of the line. A full-width space outside a string is refused, as a naming
//! refuses it (DESIGN 6.2, item 5): it splits a word where the eye sees no break. So is a control
//! character other than the tab, which a terminal does not show as it is.
//!
//! Each word keeps the bytes it was read from, so that a command that changes one word of a line
//! (`ritsu mod get` changes a version) leaves every other byte of the line as it was.

use crate::text::Text;
use crate::tr;

/// What a word was written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Word,
    /// `"…"`, its escapes read.
    Str,
}

/// A word or a string of a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    /// The text: a string's without its quotes and with its escapes read.
    pub text: String,
    /// The bytes of the line it was read from, its quotes included.
    pub start: usize,
    pub end: usize,
    /// The column it starts at, counted in characters from 1.
    pub col: usize,
}

impl Token {
    pub fn is_word(&self, w: &str) -> bool {
        self.kind == Kind::Word && self.text == w
    }
}

/// A line read: its words, and where its comment starts (a byte), if it has one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub tokens: Vec<Token>,
    pub comment: Option<usize>,
}

/// Why a line cannot be read, and the column it is at (from 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    FullWidthSpace(usize),
    /// A control character other than the tab: the character.
    Control(usize, char),
    Unclosed(usize),
    /// An escape other than `\"` and `\\`: what follows the backslash.
    BadEscape(usize, String),
}

impl Error {
    pub fn col(&self) -> usize {
        match self {
            Error::FullWidthSpace(c) | Error::Control(c, _) | Error::Unclosed(c) | Error::BadEscape(c, _) => *c,
        }
    }

    pub fn text(&self) -> Text {
        match self {
            Error::FullWidthSpace(_) => tr!(
                "文字列の外に全角の空白があります。空白を含む値は `\"…\"` で囲んでください",
                "there is a full-width space outside a string; a value with a blank in it is written in `\"…\"`"
            ),
            Error::Control(_, c) => {
                let u = format!("U+{:04X}", *c as u32);
                tr!("制御文字 {u} は書けません（タブのほかの制御文字は使えません）", "the control character {u} cannot be written here (the tab is the only one taken)")
            }
            Error::Unclosed(_) => tr!("閉じていない文字列があります", "a string is not closed"),
            Error::BadEscape(_, e) => tr!(
                "文字列の中のエスケープ `\\{e}` は使えません。使えるのは `\\\"` と `\\\\` だけです",
                "the escape `\\{e}` is not taken in a string; only `\\\"` and `\\\\` are"
            ),
        }
    }
}

/// U+3000, the full-width space of Japanese text.
const FULL_WIDTH_SPACE: char = '\u{3000}';

fn control(c: char) -> bool {
    (c.is_control() && c != '\t') || c == '\u{7f}'
}

/// The words of one line, which has no line ending.
pub fn line(text: &str) -> Result<Line, Error> {
    let cs: Vec<(usize, char)> = text.char_indices().collect();
    let byte = |i: usize| cs.get(i).map_or(text.len(), |(b, _)| *b);
    let mut out = Line::default();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i].1;
        if c == FULL_WIDTH_SPACE {
            return Err(Error::FullWidthSpace(i + 1));
        } else if control(c) {
            return Err(Error::Control(i + 1, c));
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '#' {
            out.comment = Some(byte(i));
            // what follows is a comment, but a control character in it is still refused
            if let Some((j, ch)) = cs.iter().enumerate().skip(i).find(|(_, (_, ch))| control(*ch)) {
                return Err(Error::Control(j + 1, ch.1));
            }
            return Ok(out);
        } else if c == '"' {
            let start = i;
            i += 1;
            let mut s = String::new();
            loop {
                match cs.get(i).map(|(_, ch)| *ch) {
                    None => return Err(Error::Unclosed(start + 1)),
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some('\\') => match cs.get(i + 1).map(|(_, ch)| *ch) {
                        Some(e @ ('"' | '\\')) => {
                            s.push(e);
                            i += 2;
                        }
                        other => return Err(Error::BadEscape(i + 1, other.map(|ch| ch.to_string()).unwrap_or_default())),
                    },
                    Some(ch) if control(ch) => return Err(Error::Control(i + 1, ch)),
                    Some(ch) => {
                        s.push(ch);
                        i += 1;
                    }
                }
            }
            out.tokens.push(Token { kind: Kind::Str, text: s, start: byte(start), end: byte(i), col: start + 1 });
        } else {
            let start = i;
            while i < cs.len() {
                let ch = cs[i].1;
                if ch.is_whitespace() || ch == '"' || ch == '#' {
                    break;
                }
                if control(ch) {
                    return Err(Error::Control(i + 1, ch));
                }
                i += 1;
            }
            out.tokens.push(Token { kind: Kind::Word, text: cs[start..i].iter().map(|(_, ch)| *ch).collect(), start: byte(start), end: byte(i), col: start + 1 });
        }
    }
    Ok(out)
}

/// `"…"`, with `\"` and `\\` the only escapes: what a command writes for a string.
pub fn quote(s: &str) -> String {
    crate::naming::quote(s)
}

/// The lines of a text and the ending of each (`"\n"`, `"\r\n"`, or `""` for a last line with
/// none), so that a text split and joined again is the same bytes.
pub fn split(text: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find('\n') {
            Some(n) => {
                let (line, ending) = if n > 0 && rest.as_bytes()[n - 1] == b'\r' { (&rest[..n - 1], &rest[n - 1..=n]) } else { (&rest[..n], &rest[n..=n]) };
                out.push((line, ending));
                rest = &rest[n + 1..];
            }
            None => {
                out.push((rest, ""));
                rest = "";
            }
        }
    }
    out
}
