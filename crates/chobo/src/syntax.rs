//! Lines, indentation and tokens, and the one table of keywords.

use crate::diag::{self, Diag};

/// Where a keyword is a keyword (DESIGN 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// starts a line
    Line,
    /// inside a line
    Modifier,
    /// the type of a parameter
    Type,
    /// after `expires after <n>`, and only there: elsewhere these words are names (`day`)
    Duration,
}

/// Every keyword, as a reader finds it in DESIGN 1.6. A phrase is two words that go together.
pub const KEYWORDS: &[(&str, Place)] = &[
    ("book", Place::Line),
    ("description", Place::Line),
    ("unit", Place::Line),
    ("account", Place::Line),
    ("transfer", Place::Line),
    ("key", Place::Line),
    ("pending", Place::Line),
    ("move", Place::Line),
    ("scale", Place::Modifier),
    ("outside", Place::Modifier),
    ("at least", Place::Modifier),
    ("at most", Place::Modifier),
    ("refused as", Place::Modifier),
    ("expires after", Place::Modifier),
    ("never expires", Place::Modifier),
    ("from", Place::Modifier),
    ("to", Place::Modifier),
    ("string", Place::Type),
    ("second", Place::Duration),
    ("seconds", Place::Duration),
    ("minute", Place::Duration),
    ("minutes", Place::Duration),
    ("hour", Place::Duration),
    ("hours", Place::Duration),
    ("day", Place::Duration),
    ("days", Place::Duration),
];

/// A word no name may be: every word of every keyword but the durations.
pub fn reserved(word: &str) -> bool {
    KEYWORDS.iter().any(|(k, p)| *p != Place::Duration && k.split(' ').any(|w| w == word))
}

/// Seconds in one of a duration word, or None.
pub fn duration(word: &str) -> Option<u64> {
    match word {
        "second" | "seconds" => Some(1),
        "minute" | "minutes" => Some(60),
        "hour" | "hours" => Some(3600),
        "day" | "days" => Some(86400),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    /// a number as written: an optional `-`, digits, an optional `.` and digits
    Num(String),
    Str(String),
    LParen,
    RParen,
    Colon,
    Comma,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    /// column of the first character, from 1, in characters
    pub col: usize,
    /// length in characters
    pub len: usize,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub no: usize,
    /// spaces before the first token
    pub indent: usize,
    pub toks: Vec<Token>,
}

/// A combining mark that a name copied from a decomposed file name would carry
/// (U+0300–U+036F, and the kana voiced sound marks U+3099, U+309A).
pub fn combining(c: char) -> bool {
    matches!(c, '\u{300}'..='\u{36f}' | '\u{3099}' | '\u{309a}')
}

fn name_start(c: char) -> bool {
    !combining(c) && (c.is_alphabetic() || c == '_')
}

fn name_rest(c: char) -> bool {
    !combining(c) && (c.is_alphanumeric() || c == '_')
}

/// Split the source into lines of tokens. A line that cannot be read is reported (E001)
/// and left out.
pub fn lex(src: &str) -> (Vec<Line>, Vec<Diag>) {
    let mut lines = Vec::new();
    let mut diags = Vec::new();
    for (i, raw) in src.split('\n').enumerate() {
        let no = i + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let chars: Vec<char> = raw.chars().collect();
        if no == 1 && chars.first() == Some(&'\u{feff}') {
            diags.push(diag::error(
                "E001",
                1,
                1,
                tr!("ファイルの先頭に BOM があります。BOM を付けずに UTF-8 で保存してください", "the file starts with a byte order mark; save it as UTF-8 without one"),
            ));
            continue;
        }
        match lex_line(&chars, no) {
            Ok(Some(line)) => lines.push(line),
            Ok(None) => {}
            Err(d) => diags.push(d),
        }
    }
    (lines, diags)
}

fn lex_line(chars: &[char], no: usize) -> Result<Option<Line>, Diag> {
    let mut i = 0;
    while i < chars.len() && chars[i] == ' ' {
        i += 1;
    }
    let indent = i;
    let mut toks = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        let col = i + 1;
        if c == ' ' {
            i += 1;
            continue;
        }
        if c == '#' {
            break;
        }
        if c == '\t' {
            return Err(diag::error("E001", no, col, tr!("タブがあります。字下げと区切りには空白を使ってください", "a tab: indent and separate with spaces")));
        }
        if combining(c) {
            return Err(diag::error(
                "E001",
                no,
                col,
                tr!(
                    "結合文字 U+{:04X} があります。濁点やアクセントを組み合わせた一文字（合成済みの文字）で書いてください",
                    "a combining mark U+{:04X}: write the precomposed character instead",
                    c as u32
                ),
            ));
        }
        let single = match c {
            '(' => Some(Tok::LParen),
            ')' => Some(Tok::RParen),
            ':' => Some(Tok::Colon),
            ',' => Some(Tok::Comma),
            _ => None,
        };
        if let Some(t) = single {
            toks.push(Token { tok: t, col, len: 1 });
            i += 1;
            continue;
        }
        if c == '"' {
            let mut s = String::new();
            let mut j = i + 1;
            let mut closed = false;
            while j < chars.len() {
                match chars[j] {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => match chars.get(j + 1) {
                        Some('"') => {
                            s.push('"');
                            j += 2;
                        }
                        Some('\\') => {
                            s.push('\\');
                            j += 2;
                        }
                        _ => {
                            return Err(diag::error(
                                "E001",
                                no,
                                j + 1,
                                tr!(
                                    "文字列の中で \\ のあとに書けるのは \" と \\ だけです",
                                    "inside a string, \\ may only be followed by \" or \\"
                                ),
                            ));
                        }
                    },
                    ch => {
                        s.push(ch);
                        j += 1;
                    }
                }
            }
            if !closed {
                return Err(diag::error("E001", no, col, tr!("文字列が閉じていません", "the string is not closed")));
            }
            toks.push(Token { tok: Tok::Str(s), col, len: j + 1 - i });
            i = j + 1;
            continue;
        }
        let digit_at = |k: usize| chars.get(k).is_some_and(|d| d.is_ascii_digit());
        if c.is_ascii_digit() || (c == '-' && digit_at(i + 1)) {
            let mut j = i + 1;
            while digit_at(j) {
                j += 1;
            }
            if chars.get(j) == Some(&'.') && digit_at(j + 1) {
                j += 1;
                while digit_at(j) {
                    j += 1;
                }
            }
            if chars.get(j).is_some_and(|d| name_rest(*d) || *d == '.') {
                return Err(diag::error("E001", no, col, tr!("数のすぐあとに文字が続いています", "a number runs into the next word")));
            }
            toks.push(Token { tok: Tok::Num(chars[i..j].iter().collect()), col, len: j - i });
            i = j;
            continue;
        }
        if name_start(c) {
            let mut j = i + 1;
            while j < chars.len() && name_rest(chars[j]) {
                j += 1;
            }
            if chars.get(j).is_some_and(|d| combining(*d)) {
                let d = chars[j];
                return Err(diag::error(
                    "E001",
                    no,
                    j + 1,
                    tr!(
                        "名前に結合文字 U+{:04X} があります。濁点やアクセントを組み合わせた一文字（合成済みの文字）で書いてください",
                        "the name has a combining mark U+{:04X}: write the precomposed character instead",
                        d as u32
                    ),
                ));
            }
            toks.push(Token { tok: Tok::Word(chars[i..j].iter().collect()), col, len: j - i });
            i = j;
            continue;
        }
        return Err(diag::error("E001", no, col, tr!("読めない文字 `{c}` があります", "an unexpected character `{c}`")));
    }
    if toks.is_empty() {
        return Ok(None);
    }
    Ok(Some(Line { no, indent, toks }))
}
