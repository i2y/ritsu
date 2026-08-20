//! 最小の JSON 読み取り（§10.2 の記録形式のため）。
//!
//! 外部の依存を増やさない方針（§12）なので自前で持つ。書き出しは各所が
//! 正準形を組み立てるので、ここにあるのは読み取りだけ。
//!
//! `fixtures lint` は「壊れた記録を正確に指す」のが仕事なので、素朴な文字列抽出
//! では足りない。値の中に `"送料":` を含む文字列が一つあるだけで、抽出は
//! 別の場所を指して黙って通る。

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// 数は整数だけ受ける。ワイヤは正準単位の整数と決めてある（§10.1）ので、
    /// 小数が来たらそれ自体が報告に値する誤りである。
    Int(i128),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(key),
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
    pub fn as_obj(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }
    /// 診断に載せるための短い型名。
    pub fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "真偽",
            Json::Int(_) => "整数",
            Json::Str(_) => "文字列",
            Json::Arr(_) => "配列",
            Json::Obj(_) => "オブジェクト",
        }
    }
}

impl fmt::Display for Json {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Json::Null => write!(w, "null"),
            Json::Bool(b) => write!(w, "{b}"),
            Json::Int(n) => write!(w, "{n}"),
            Json::Str(s) => write!(w, "{s}"),
            Json::Arr(_) => write!(w, "配列"),
            Json::Obj(_) => write!(w, "オブジェクト"),
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

/// 一行を読む。末尾に値以外が残っていたらエラー（黙って前半だけ読まない）。
pub fn parse(src: &str) -> Result<Json, String> {
    let mut p = P { b: src.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("{} 文字目より後ろに余分なものがあります", p.i + 1));
    }
    Ok(v)
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.i < self.b.len() && self.b[self.i] == c {
            self.i += 1;
            return Ok(());
        }
        Err(format!("{} 文字目に `{}` が要ります", self.i + 1, c as char))
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        let Some(&c) = self.b.get(self.i) else {
            return Err("値がありません".into());
        };
        match c {
            b'{' => self.obj(),
            b'[' => self.arr(),
            b'"' => self.string().map(Json::Str),
            b't' | b'f' | b'n' => self.word(),
            _ => self.number(),
        }
    }
    fn word(&mut self) -> Result<Json, String> {
        for (w, v) in [("true", Json::Bool(true)), ("false", Json::Bool(false)), ("null", Json::Null)] {
            if self.b[self.i..].starts_with(w.as_bytes()) {
                self.i += w.len();
                return Ok(v);
            }
        }
        Err(format!("{} 文字目が読めません", self.i + 1))
    }
    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i == start || (self.i == start + 1 && self.b[start] == b'-') {
            return Err(format!("{} 文字目が読めません", start + 1));
        }
        // 小数や指数は受けない。ワイヤは正準単位の整数（§10.1）。
        if matches!(self.b.get(self.i), Some(b'.') | Some(b'e') | Some(b'E')) {
            return Err(format!(
                "{} 文字目: 小数は受け付けません。値は正準単位の整数で書いてください",
                start + 1
            ));
        }
        std::str::from_utf8(&self.b[start..self.i])
            .ok()
            .and_then(|s| s.parse::<i128>().ok())
            .map(Json::Int)
            .ok_or_else(|| format!("{} 文字目: 整数が大きすぎます", start + 1))
    }
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&c) = self.b.get(self.i) else {
                return Err("文字列が閉じていません".into());
            };
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(&e) = self.b.get(self.i) else {
                        return Err("文字列が閉じていません".into());
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
                            let h = self
                                .b
                                .get(self.i..self.i + 4)
                                .and_then(|s| std::str::from_utf8(s).ok())
                                .and_then(|s| u32::from_str_radix(s, 16).ok())
                                .ok_or_else(|| format!("{} 文字目: \\u の後ろが 16 進 4 桁ではありません", self.i + 1))?;
                            self.i += 4;
                            // 代理対。JSON は BMP 外をこの形でしか書けない。
                            let ch = if (0xD800..0xDC00).contains(&h) {
                                if self.b.get(self.i..self.i + 2) != Some(b"\\u") {
                                    return Err(format!("{} 文字目: 上位代理の後ろに下位代理がありません", self.i + 1));
                                }
                                self.i += 2;
                                let lo = self
                                    .b
                                    .get(self.i..self.i + 4)
                                    .and_then(|s| std::str::from_utf8(s).ok())
                                    .and_then(|s| u32::from_str_radix(s, 16).ok())
                                    .ok_or_else(|| format!("{} 文字目: \\u の後ろが 16 進 4 桁ではありません", self.i + 1))?;
                                self.i += 4;
                                0x10000 + ((h - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                h
                            };
                            out.push(char::from_u32(ch).ok_or_else(|| "使えない符号位置です".to_string())?);
                        }
                        _ => return Err(format!("{} 文字目: 知らないエスケープです", self.i)),
                    }
                }
                _ => {
                    // UTF-8 の続きバイトをそのまま送る。
                    let len = utf8_len(c);
                    let end = (self.i - 1 + len).min(self.b.len());
                    match std::str::from_utf8(&self.b[self.i - 1..end]) {
                        Ok(s) => {
                            out.push_str(s);
                            self.i = end;
                        }
                        Err(_) => return Err(format!("{} 文字目: UTF-8 として読めません", self.i)),
                    }
                }
            }
        }
    }
    fn arr(&mut self) -> Result<Json, String> {
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
                _ => return Err(format!("{} 文字目に `,` か `]` が要ります", self.i + 1)),
            }
        }
    }
    fn obj(&mut self) -> Result<Json, String> {
        self.eat(b'{')?;
        let mut out = BTreeMap::new();
        self.ws();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Json::Obj(out));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.eat(b':')?;
            let v = self.value()?;
            if out.insert(k.clone(), v).is_some() {
                return Err(format!("鍵 `{k}` が二度あります"));
            }
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(out));
                }
                _ => return Err(format!("{} 文字目に `,` か `}}` が要ります", self.i + 1)),
            }
        }
    }
}

fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 値の中の鍵らしき文字列に騙されない() {
        // 素朴な文字列抽出が黙って別の場所を指す形。
        let v = parse(r#"{"tag":"\"送料\":9999","observed":{"送料":800}}"#).unwrap();
        assert_eq!(v.get("observed").unwrap().get("送料").unwrap().as_int(), Some(800));
    }

    #[test]
    fn 壊れた行は位置つきで断る() {
        for bad in [r#"{"a":1"#, r#"{"a":}"#, r#"{"a":1}x"#, r#"{"a":1.5}"#, r#"{"a":1,"a":2}"#] {
            assert!(parse(bad).is_err(), "通ってしまった: {bad}");
        }
    }

    #[test]
    fn 和名とエスケープを読む() {
        let v = parse(r#"{"届け先":"北海道","x":"a\nb","y":"日本"}"#).unwrap();
        assert_eq!(v.get("届け先").unwrap().as_str(), Some("北海道"));
        assert_eq!(v.get("x").unwrap().as_str(), Some("a\nb"));
        assert_eq!(v.get("y").unwrap().as_str(), Some("日本"));
    }
}
