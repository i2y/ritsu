//! JSON without a dependency (DESIGN 4.9): a value whose objects keep the order their keys were
//! written in and whose integers are exact, a reader, and a writer whose output is byte for byte
//! what serde_json writes — compact (`to_string`) and pretty (`to_string_pretty`) — so a crate
//! that prints with serde_json today prints the same through this one.
//!
//! The reader is rulec's (`src/json.rs`): it refuses an exponent, keeps a number with a
//! fractional part as the digits that were written (never through a float), refuses a key that
//! appears twice in one object, and stops at a nesting of [`MAX_DEPTH`].

use crate::text::Text;
use crate::tr;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i128),
    /// A number with a fractional part, as the digits that were written.
    Frac(String),
    Str(String),
    Arr(Vec<Json>),
    /// The keys in the order they were written or added.
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn str(s: impl Into<String>) -> Json {
        Json::Str(s.into())
    }

    pub fn int(n: impl Into<i128>) -> Json {
        Json::Int(n.into())
    }

    pub fn arr(items: impl IntoIterator<Item = Json>) -> Json {
        Json::Arr(items.into_iter().collect())
    }

    /// An object, its keys in the order given.
    pub fn obj<K: Into<String>>(pairs: impl IntoIterator<Item = (K, Json)>) -> Json {
        Json::Obj(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// A string, or null.
    pub fn opt_str(s: Option<impl Into<String>>) -> Json {
        s.map(|s| Json::Str(s.into())).unwrap_or(Json::Null)
    }

    /// The value of `key`, when this is an object that has it.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i128> {
        match self {
            Json::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    /// What kind of value it is, for a diagnostic.
    pub fn kind(&self) -> Text {
        match self {
            Json::Null => Text::same("null"),
            Json::Bool(_) => tr!("真偽値", "a boolean"),
            Json::Int(_) => tr!("整数", "an integer"),
            Json::Frac(_) => tr!("小数", "a fraction"),
            Json::Str(_) => tr!("文字列", "a string"),
            Json::Arr(_) => tr!("配列", "an array"),
            Json::Obj(_) => tr!("オブジェクト", "an object"),
        }
    }

    /// On one line, no space anywhere: serde_json's `to_string`.
    pub fn compact(&self) -> String {
        let mut o = String::new();
        write(self, None, 0, &mut o);
        o
    }

    /// Two spaces a level, a key and its value on one line: serde_json's `to_string_pretty`.
    pub fn pretty(&self) -> String {
        let mut o = String::new();
        write(self, Some("  "), 0, &mut o);
        o
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.compact())
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Json {
        Json::Str(s.to_string())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Json {
        Json::Str(s)
    }
}

impl From<bool> for Json {
    fn from(b: bool) -> Json {
        Json::Bool(b)
    }
}

impl From<i64> for Json {
    fn from(n: i64) -> Json {
        Json::Int(n.into())
    }
}

impl From<u64> for Json {
    fn from(n: u64) -> Json {
        Json::Int(n.into())
    }
}

impl From<usize> for Json {
    fn from(n: usize) -> Json {
        Json::Int(n as i128)
    }
}

impl From<u32> for Json {
    fn from(n: u32) -> Json {
        Json::Int(n.into())
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(v: Option<T>) -> Json {
        v.map(Into::into).unwrap_or(Json::Null)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(v: Vec<T>) -> Json {
        Json::Arr(v.into_iter().map(Into::into).collect())
    }
}

// ── Writing ─────────────────────────────────────────────────────────────

/// The inside of a JSON string, escaped as serde_json escapes it: `"`, `\`, and the control
/// characters (`\b`, `\t`, `\n`, `\f`, `\r` by name, the others as `\u00xx`). Everything else,
/// Japanese included, is passed through as UTF-8.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\u{8}' => o.push_str("\\b"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\u{c}' => o.push_str("\\f"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// A quoted JSON string.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", esc(s))
}

fn write(v: &Json, indent: Option<&str>, level: usize, o: &mut String) {
    let newline = |o: &mut String, level: usize| {
        if let Some(i) = indent {
            o.push('\n');
            for _ in 0..level {
                o.push_str(i);
            }
        }
    };
    match v {
        Json::Null => o.push_str("null"),
        Json::Bool(b) => o.push_str(if *b { "true" } else { "false" }),
        Json::Int(n) => o.push_str(&n.to_string()),
        Json::Frac(s) => o.push_str(s),
        Json::Str(s) => o.push_str(&quote(s)),
        Json::Arr(a) if a.is_empty() => o.push_str("[]"),
        Json::Arr(a) => {
            o.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                newline(o, level + 1);
                write(x, indent, level + 1, o);
            }
            newline(o, level);
            o.push(']');
        }
        Json::Obj(m) if m.is_empty() => o.push_str("{}"),
        Json::Obj(m) => {
            o.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                newline(o, level + 1);
                o.push_str(&quote(k));
                o.push_str(if indent.is_some() { ": " } else { ":" });
                write(x, indent, level + 1, o);
            }
            newline(o, level);
            o.push('}');
        }
    }
}

// ── Reading ─────────────────────────────────────────────────────────────

/// What is wrong with a text that should be JSON, and the byte it is at: [`Error::what`] for a
/// tool that words it its own way (rulec), [`Error::message`] in ritsu-base's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub at: usize,
    pub what: Problem,
    pub message: Text,
}

/// What stops the reader, before it is said in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Something after the value, at the byte the value ended.
    Extra,
    /// The character that has to come here.
    Expected(char),
    /// No value where one has to be.
    MissingValue,
    /// Nested deeper than [`MAX_DEPTH`].
    TooDeep,
    /// A word that is not `true`, `false` or `null`, or a number with no digit.
    Unreadable,
    /// A decimal point with no digit after it.
    NoDigitAfterPoint,
    /// `1e3`: an exponent, which is not read.
    Exponent,
    /// An integer outside `i128`.
    TooLarge,
    /// `\u` without four hex digits after it.
    BadUnicodeEscape,
    /// A high surrogate with no `\u` after it.
    LoneHighSurrogate,
    /// A high surrogate whose `\u` after it is not a low surrogate.
    NotLowSurrogate,
    /// An escape that names no character.
    BadCodePoint,
    /// `\x` and the other escapes JSON does not have.
    UnknownEscape,
    /// A control character written as it is inside a string.
    ControlCharacter,
    /// Bytes that are not UTF-8.
    NotUtf8,
    /// A string with no closing quote.
    Unclosed,
    /// Neither `,` nor `]` after an element of an array.
    CommaOrBracket,
    /// Neither `,` nor `}` after a member of an object.
    CommaOrBrace,
    /// The key that appears twice in one object.
    DuplicateKey(String),
}

/// The deepest nesting read. The reader is recursive, and a line of some twenty thousand `[`
/// ran rulec's out of stack (rulec's §15.156).
pub const MAX_DEPTH: usize = 256;

/// One value, and nothing after it but whitespace.
pub fn parse(src: &str) -> Result<Json, Error> {
    let mut p = P { b: src.as_bytes(), i: 0, depth: 0 };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        let n = p.i + 1;
        return Err(p.err(Problem::Extra, tr!("{n} バイト目から後ろに余分なものがあります", "extra content from byte {n} on")));
    }
    Ok(v)
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
    depth: usize,
}

impl P<'_> {
    fn err(&self, what: Problem, message: Text) -> Error {
        Error { at: self.i, what, message }
    }

    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), Error> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            return Ok(());
        }
        let (n, c) = (self.i + 1, c as char);
        Err(self.err(Problem::Expected(c), tr!("{n} バイト目に `{c}` が要ります", "byte {n}: expected `{c}`")))
    }

    fn value(&mut self) -> Result<Json, Error> {
        self.ws();
        let Some(&c) = self.b.get(self.i) else {
            return Err(self.err(Problem::MissingValue, tr!("値がありません", "a value is missing")));
        };
        match c {
            b'{' | b'[' => {
                if self.depth == MAX_DEPTH {
                    let n = self.i + 1;
                    return Err(self.err(Problem::TooDeep, tr!("{n} バイト目: 入れ子が {MAX_DEPTH} 段を超えています", "byte {n}: nested deeper than {MAX_DEPTH} levels")));
                }
                self.depth += 1;
                let v = if c == b'{' { self.obj() } else { self.arr() };
                self.depth -= 1;
                v
            }
            b'"' => self.string().map(Json::Str),
            b't' | b'f' | b'n' => self.word(),
            _ => self.number(),
        }
    }

    fn word(&mut self) -> Result<Json, Error> {
        for (w, v) in [("true", Json::Bool(true)), ("false", Json::Bool(false)), ("null", Json::Null)] {
            if self.b[self.i..].starts_with(w.as_bytes()) {
                self.i += w.len();
                return Ok(v);
            }
        }
        let n = self.i + 1;
        Err(self.err(Problem::Unreadable, tr!("{n} バイト目が読めません", "cannot read byte {n}")))
    }

    fn number(&mut self) -> Result<Json, Error> {
        let start = self.i;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        let n = start + 1;
        if self.i == start || (self.i == start + 1 && self.b[start] == b'-') {
            self.i = start;
            return Err(self.err(Problem::Unreadable, tr!("{n} バイト目が読めません", "cannot read byte {n}")));
        }
        let mut frac = false;
        if self.b.get(self.i) == Some(&b'.') {
            frac = true;
            self.i += 1;
            let digits = self.i;
            while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                self.i += 1;
            }
            if self.i == digits {
                return Err(Error { at: start, what: Problem::NoDigitAfterPoint, message: tr!("{n} バイト目: 小数点のあとに数字がありません", "byte {n}: no digit after the decimal point") });
            }
        }
        if matches!(self.b.get(self.i), Some(b'e') | Some(b'E')) {
            return Err(Error { at: start, what: Problem::Exponent, message: tr!("{n} バイト目: 指数の書き方は読めません", "byte {n}: an exponent is not read") });
        }
        let text = std::str::from_utf8(&self.b[start..self.i]).unwrap_or_default();
        if frac {
            return Ok(Json::Frac(text.to_string()));
        }
        text.parse::<i128>().map(Json::Int).map_err(|_| Error { at: start, what: Problem::TooLarge, message: tr!("{n} バイト目: 整数が大きすぎます", "byte {n}: the integer is too large") })
    }

    fn hex4(&mut self) -> Result<u32, Error> {
        let h = self.b.get(self.i..self.i + 4).and_then(|s| std::str::from_utf8(s).ok()).and_then(|s| u32::from_str_radix(s, 16).ok());
        match h {
            Some(h) => {
                self.i += 4;
                Ok(h)
            }
            None => {
                let n = self.i + 1;
                Err(self.err(Problem::BadUnicodeEscape, tr!("{n} バイト目: \\u のあとが 16 進の 4 桁ではありません", "byte {n}: \\u is not followed by four hex digits")))
            }
        }
    }

    fn string(&mut self) -> Result<String, Error> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&c) = self.b.get(self.i) else {
                return Err(self.err(Problem::Unclosed, tr!("文字列が閉じていません", "a string is not closed")));
            };
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(&e) = self.b.get(self.i) else {
                        return Err(self.err(Problem::Unclosed, tr!("文字列が閉じていません", "a string is not closed")));
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let h = self.hex4()?;
                            // A surrogate pair: JSON has no other way to write a character
                            // outside the Basic Multilingual Plane.
                            let ch = if (0xD800..0xDC00).contains(&h) {
                                if self.b.get(self.i..self.i + 2) != Some(b"\\u") {
                                    let n = self.i + 1;
                                    return Err(self.err(Problem::LoneHighSurrogate, tr!("{n} バイト目: 上位サロゲートのあとに下位サロゲートがありません", "byte {n}: a high surrogate without its low surrogate")));
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    let n = self.i - 3;
                                    return Err(self.err(Problem::NotLowSurrogate, tr!("{n} バイト目: 下位サロゲートではありません", "byte {n}: not a low surrogate")));
                                }
                                0x10000 + ((h - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                h
                            };
                            match char::from_u32(ch) {
                                Some(c) => out.push(c),
                                None => return Err(self.err(Problem::BadCodePoint, tr!("使えない符号位置です", "not a code point that can be used"))),
                            }
                        }
                        _ => {
                            let n = self.i;
                            return Err(Error { at: n - 1, what: Problem::UnknownEscape, message: tr!("{n} バイト目: 使えないエスケープです", "byte {n}: an escape that is not known") });
                        }
                    }
                }
                c if c < 0x20 => {
                    let n = self.i;
                    return Err(Error { at: n - 1, what: Problem::ControlCharacter, message: tr!("{n} バイト目: 文字列の中に制御文字があります", "byte {n}: a control character in a string") });
                }
                _ => {
                    // Pass a character's UTF-8 bytes through as they are.
                    let len = match c {
                        0x00..=0x7F => 1,
                        0xC0..=0xDF => 2,
                        0xE0..=0xEF => 3,
                        _ => 4,
                    };
                    let end = (self.i - 1 + len).min(self.b.len());
                    match std::str::from_utf8(&self.b[self.i - 1..end]) {
                        Ok(s) => {
                            out.push_str(s);
                            self.i = end;
                        }
                        Err(_) => {
                            let n = self.i;
                            return Err(Error { at: n - 1, what: Problem::NotUtf8, message: tr!("{n} バイト目: UTF-8 として読めません", "byte {n}: not UTF-8") });
                        }
                    }
                }
            }
        }
    }

    fn arr(&mut self) -> Result<Json, Error> {
        self.eat(b'[')?;
        let mut out = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(Json::Arr(out));
        }
        loop {
            out.push(self.value()?);
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(out));
                }
                _ => {
                    let n = self.i + 1;
                    return Err(self.err(Problem::CommaOrBracket, tr!("{n} バイト目に `,` か `]` が要ります", "byte {n}: expected `,` or `]`")));
                }
            }
        }
    }

    fn obj(&mut self) -> Result<Json, Error> {
        self.eat(b'{')?;
        let mut out: Vec<(String, Json)> = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Json::Obj(out));
        }
        loop {
            self.ws();
            let at = self.i;
            let k = self.string()?;
            self.eat(b':')?;
            let v = self.value()?;
            if out.iter().any(|(x, _)| *x == k) {
                return Err(Error { at, what: Problem::DuplicateKey(k.clone()), message: tr!("キー `{k}` が二度あります", "the key `{k}` appears twice") });
            }
            out.push((k, v));
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(out));
                }
                _ => {
                    let n = self.i + 1;
                    return Err(self.err(Problem::CommaOrBrace, tr!("{n} バイト目に `,` か `}}` が要ります", "byte {n}: expected `,` or `}}`")));
                }
            }
        }
    }
}
