//! JSON: the record format of §10.2, and every `--format json`.
//!
//! The value, the reader and the escaping are ritsu-base's `json` (ritsu's DESIGN 4.9), which
//! began as this file: no dependency (§12), a fraction kept as the digits that were written, no
//! exponent, no key twice in one object, no nesting past [`MAX_DEPTH`]. What stays here is
//! rulec's own: its words for what is wrong with a text, in the language the process prints in;
//! the names it gives the kinds of value; an object read and written back with its keys sorted;
//! and the writer every `--format json` goes through.
//!
//! The job of `fixtures lint` is to "point precisely at the broken record", and naive string
//! extraction is not enough for that: a single string value containing `"送料":` is all it
//! takes for the extraction to point somewhere else and pass silently.

pub use ritsu_base::json::{Json, MAX_DEPTH, esc, quote};
use ritsu_base::json::{Error, Problem};
use std::collections::BTreeMap;

/// Read one line. Anything other than the value left at the end is an error (the first half
/// is never read silently on its own).
pub fn parse(src: &str) -> Result<Json, String> {
    ritsu_base::json::parse(src).map_err(|e| said(&e))
}

/// What the reader stopped at, as rulec says it. A position is the byte's, counted from one.
fn said(e: &Error) -> String {
    let n = e.at + 1;
    match &e.what {
        Problem::Extra => tr!("{n} 文字目より後ろに余分なものがあります", "extra content after character {n}"),
        Problem::Expected(c) => tr!("{n} 文字目に `{c}` が要ります", "character {n}: expected `{c}`"),
        Problem::MissingValue => tr!("値がありません", "missing value"),
        Problem::TooDeep => tr!("{n} 文字目: ネストが {MAX_DEPTH} 段を超えています", "character {n}: nested deeper than {MAX_DEPTH} levels"),
        Problem::Unreadable => tr!("{n} 文字目が読めません", "cannot read character {n}"),
        Problem::NoDigitAfterPoint => tr!("{n} 文字目: 小数点の後ろに数字がありません", "character {n}: no digit after the decimal point"),
        Problem::Exponent => tr!("{n} 文字目: 指数表記は受け付けません", "character {n}: exponent notation is not accepted"),
        Problem::TooLarge => tr!("{n} 文字目: 整数が大きすぎます", "character {n}: integer too large"),
        Problem::BadUnicodeEscape => tr!("{n} 文字目: \\u の後ろが 16 進 4 桁ではありません", "character {n}: \\u is not followed by 4 hex digits"),
        Problem::LoneHighSurrogate => tr!("{n} 文字目: 上位サロゲートの後ろに下位サロゲートがありません", "character {n}: high surrogate not followed by a low surrogate"),
        Problem::NotLowSurrogate => tr!("{n} 文字目: 上位サロゲートの後ろが下位サロゲートではありません", "character {n}: a high surrogate followed by what is not a low surrogate"),
        Problem::BadCodePoint => tr!("使えないコードポイントです", "invalid code point"),
        Problem::UnknownEscape => tr!("{n} 文字目: 使えないエスケープです", "character {n}: unknown escape"),
        Problem::ControlCharacter => tr!("{n} 文字目: 文字列の中に制御文字がそのまま書かれています", "character {n}: a control character written as it is in a string"),
        Problem::NotUtf8 => tr!("{n} 文字目: UTF-8 として読めません", "character {n}: not valid UTF-8"),
        Problem::Unclosed => tr!("文字列が閉じていません", "unterminated string"),
        Problem::CommaOrBracket => tr!("{n} 文字目に `,` か `]` が要ります", "character {n}: expected `,` or `]`"),
        Problem::CommaOrBrace => tr!("{n} 文字目に `,` か `}}` が要ります", "character {n}: expected `,` or `}}`"),
        Problem::DuplicateKey(k) => tr!("キー `{k}` が二度あります", "key `{k}` appears twice"),
    }
}

/// A short type name for diagnostics.
pub fn kind(j: &Json) -> &'static str {
    match j {
        Json::Null => "null",
        Json::Bool(_) => if crate::i18n::ja() { "真偽" } else { "boolean" },
        Json::Int(_) => if crate::i18n::ja() { "整数" } else { "integer" },
        Json::Frac(_) => if crate::i18n::ja() { "小数" } else { "a fraction" },
        Json::Str(_) => if crate::i18n::ja() { "文字列" } else { "string" },
        Json::Arr(_) => if crate::i18n::ja() { "配列" } else { "array" },
        Json::Obj(_) => if crate::i18n::ja() { "オブジェクト" } else { "object" },
    }
}

/// A value as a message shows it: a scalar as it was written, a string without its quotes, an
/// array or an object by the name of its kind.
pub fn show(j: &Json) -> String {
    match j {
        Json::Null => "null".to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Int(n) => n.to_string(),
        Json::Frac(s) | Json::Str(s) => s.clone(),
        Json::Arr(_) | Json::Obj(_) => kind(j).to_string(),
    }
}

/// An object's members, sorted by key, the way rulec reads an object: what a record holds does
/// not depend on the order its fields were written in.
pub fn members(j: &Json) -> Option<BTreeMap<&str, &Json>> {
    match j {
        Json::Obj(m) => Some(m.iter().map(|(k, v)| (k.as_str(), v)).collect()),
        _ => None,
    }
}

/// The members of `j[key]` (see [`members`]).
pub fn members_of<'a>(j: &'a Json, key: &str) -> Option<BTreeMap<&'a str, &'a Json>> {
    j.get(key).and_then(members)
}

// ── Writing ─────────────────────────────────────────────────────────────
//
// A rendered object keeps the order its writer chose, so output is built here instead of
// through a value. Every `--format json` surface goes through this, and through `quote`, which
// is what keeps the escaping in one place.

/// A value back to its text, compact, an object's keys sorted (the order rulec reads them in).
/// The reader keeps a fraction as the digits it was given, so it goes back out as they were.
pub fn unparse(j: &Json) -> String {
    match j {
        Json::Arr(a) => format!("[{}]", a.iter().map(unparse).collect::<Vec<_>>().join(",")),
        Json::Obj(_) => {
            let m = members(j).unwrap_or_default();
            format!("{{{}}}", m.iter().map(|(k, v)| format!("{}:{}", quote(k), unparse(v))).collect::<Vec<_>>().join(","))
        }
        scalar => scalar.compact(),
    }
}

/// A JSON array of already-encoded values.
pub fn arr<S: AsRef<str>>(items: &[S]) -> String {
    format!("[{}]", items.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(","))
}

/// A JSON array of strings.
pub fn strs<S: AsRef<str>>(items: &[S]) -> String {
    arr(&items.iter().map(|s| quote(s.as_ref())).collect::<Vec<_>>())
}

/// One object, written in the order the fields are added.
#[derive(Default)]
pub struct Obj {
    parts: Vec<String>,
}

impl Obj {
    pub fn new() -> Obj {
        Obj::default()
    }
    /// An already-encoded value.
    pub fn raw(mut self, k: &str, v: impl AsRef<str>) -> Obj {
        self.parts.push(format!("{}:{}", quote(k), v.as_ref()));
        self
    }
    pub fn str(self, k: &str, v: impl AsRef<str>) -> Obj {
        let q = quote(v.as_ref());
        self.raw(k, q)
    }
    pub fn int(self, k: &str, v: impl Into<i128>) -> Obj {
        let n: i128 = v.into();
        self.raw(k, n.to_string())
    }
    pub fn bool(self, k: &str, v: bool) -> Obj {
        self.raw(k, if v { "true" } else { "false" })
    }
    /// Omitted entirely when `None`. A field that is absent and a field that is `null` are
    /// different statements, and the readers of these files act on the difference.
    pub fn opt_str(self, k: &str, v: Option<impl AsRef<str>>) -> Obj {
        match v {
            Some(v) => self.str(k, v),
            None => self,
        }
    }
    pub fn opt_raw(self, k: &str, v: Option<impl AsRef<str>>) -> Obj {
        match v {
            Some(v) => self.raw(k, v),
            None => self,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
    pub fn finish(self) -> String {
        format!("{{{}}}", self.parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 値の中の鍵らしき文字列に騙されない() {
        // The shape in which naive string extraction silently points somewhere else.
        let v = parse(r#"{"tag":"\"送料\":9999","observed":{"送料":800}}"#).unwrap();
        assert_eq!(v.get("observed").unwrap().get("送料").unwrap().as_int(), Some(800));
    }

    #[test]
    fn 壊れた行は位置つきでエラーにする() {
        for bad in [r#"{"a":1"#, r#"{"a":}"#, r#"{"a":1}x"#, r#"{"a":1e3}"#, r#"{"a":1,"a":2}"#] {
            assert!(parse(bad).is_err(), "parsed although it should have failed: {bad}");
        }
        // A fraction reads back — rulec writes one itself (a match rate) — but it is not an
        // integer, which is how a decimal stays out of a fixtures record (§10.2).
        let v = parse(r#"{"a":1.5}"#).unwrap();
        assert_eq!(v.get("a").unwrap(), &Json::Frac("1.5".into()));
        assert_eq!(v.get("a").unwrap().as_int(), None);
    }

    #[test]
    fn 和名とエスケープを読む() {
        let v = parse(r#"{"届け先":"北海道","x":"a\nb","y":"日本"}"#).unwrap();
        assert_eq!(v.get("届け先").unwrap().as_str(), Some("北海道"));
        assert_eq!(v.get("x").unwrap().as_str(), Some("a\nb"));
        assert_eq!(v.get("y").unwrap().as_str(), Some("日本"));
    }

    /// The keys of an object are read, and written back, sorted.
    #[test]
    fn キーは文字コードの順に読み書きする() {
        let v = parse(r#"{"b":1,"a":{"d":true,"c":null}}"#).unwrap();
        assert_eq!(unparse(&v), r#"{"a":{"c":null,"d":true},"b":1}"#);
        assert_eq!(members(&v).unwrap().keys().copied().collect::<Vec<_>>(), ["a", "b"]);
    }
}
