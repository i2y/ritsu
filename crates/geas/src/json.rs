#[derive(Debug, Clone)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

pub fn parse(s: &str) -> Result<J, String> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    let v = val(&b, &mut i)?;
    ws(&b, &mut i);
    if i < b.len() {
        return Err("trailing data after JSON value".into());
    }
    Ok(v)
}

fn ws(b: &[char], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], ' ' | '\t' | '\n' | '\r') {
        *i += 1;
    }
}

fn val(b: &[char], i: &mut usize) -> Result<J, String> {
    ws(b, i);
    match b.get(*i) {
        None => Err("unexpected end of JSON".into()),
        Some('{') => {
            *i += 1;
            let mut pairs = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&'}') {
                *i += 1;
                return Ok(J::Obj(pairs));
            }
            loop {
                ws(b, i);
                let k = jstr(b, i)?;
                ws(b, i);
                if b.get(*i) != Some(&':') {
                    return Err("expected `:` in object".into());
                }
                *i += 1;
                let v = val(b, i)?;
                pairs.push((k, v));
                ws(b, i);
                match b.get(*i) {
                    Some(',') => {
                        *i += 1;
                    }
                    Some('}') => {
                        *i += 1;
                        break;
                    }
                    _ => return Err("expected `,` or `}` in object".into()),
                }
            }
            Ok(J::Obj(pairs))
        }
        Some('[') => {
            *i += 1;
            let mut items = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&']') {
                *i += 1;
                return Ok(J::Arr(items));
            }
            loop {
                items.push(val(b, i)?);
                ws(b, i);
                match b.get(*i) {
                    Some(',') => {
                        *i += 1;
                    }
                    Some(']') => {
                        *i += 1;
                        break;
                    }
                    _ => return Err("expected `,` or `]` in array".into()),
                }
            }
            Ok(J::Arr(items))
        }
        Some('"') => Ok(J::Str(jstr(b, i)?)),
        Some('t') => lit(b, i, "true", J::Bool(true)),
        Some('f') => lit(b, i, "false", J::Bool(false)),
        Some('n') => lit(b, i, "null", J::Null),
        Some(c) if c.is_ascii_digit() || *c == '-' => {
            let start = *i;
            while *i < b.len()
                && matches!(b[*i], '0'..='9' | '-' | '+' | '.' | 'e' | 'E')
            {
                *i += 1;
            }
            let s: String = b[start..*i].iter().collect();
            s.parse::<f64>().map(J::Num).map_err(|_| format!("bad number `{}`", s))
        }
        Some(c) => Err(format!("unexpected character `{}` in JSON", c)),
    }
}

fn lit(b: &[char], i: &mut usize, word: &str, out: J) -> Result<J, String> {
    for w in word.chars() {
        if b.get(*i) != Some(&w) {
            return Err(format!("bad JSON literal (expected `{}`)", word));
        }
        *i += 1;
    }
    Ok(out)
}

fn jstr(b: &[char], i: &mut usize) -> Result<String, String> {
    if b.get(*i) != Some(&'"') {
        return Err("expected a JSON string".into());
    }
    *i += 1;
    let mut s = String::new();
    loop {
        match b.get(*i) {
            None => return Err("unterminated JSON string".into()),
            Some('"') => {
                *i += 1;
                return Ok(s);
            }
            Some('\\') => {
                *i += 1;
                let Some(e) = b.get(*i) else {
                    return Err("unterminated escape".into());
                };
                match e {
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    '/' => s.push('/'),
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    'b' => s.push('\u{0008}'),
                    'f' => s.push('\u{000C}'),
                    'u' => {
                        let h = hex4(b, *i + 1)?;
                        *i += 4;
                        let cp = if (0xD800..0xDC00).contains(&h) {
                            // surrogate pair: expect \uXXXX low half
                            if b.get(*i + 1) == Some(&'\\') && b.get(*i + 2) == Some(&'u') {
                                let lo = hex4(b, *i + 3)?;
                                *i += 6;
                                0x10000 + ((h - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                return Err("lone surrogate in JSON string".into());
                            }
                        } else {
                            h
                        };
                        match char::from_u32(cp) {
                            Some(c) => s.push(c),
                            None => return Err("bad \\u escape".into()),
                        }
                    }
                    other => return Err(format!("unknown escape \\{}", other)),
                }
                *i += 1;
            }
            Some(c) => {
                s.push(*c);
                *i += 1;
            }
        }
    }
}

fn hex4(b: &[char], at: usize) -> Result<u32, String> {
    let mut v = 0u32;
    for k in 0..4 {
        let Some(c) = b.get(at + k) else {
            return Err("truncated \\u escape".into());
        };
        let d = c.to_digit(16).ok_or("bad hex in \\u escape".to_string())?;
        v = v * 16 + d;
    }
    Ok(v)
}

/// Why a JSON path does not lead to a value, in words for either language.
#[derive(Debug, Clone, PartialEq)]
pub enum PathError {
    Empty,
    EmptyKey,
    NoKey(String),
    NotObject(String),
    Bracket,
    BadIndex,
    OutOfRange(usize),
    NotArray(usize),
    Near(char),
}

impl PathError {
    pub fn en(&self) -> String {
        match self {
            PathError::Empty => "empty JSON path".into(),
            PathError::EmptyKey => "empty key in JSON path".into(),
            PathError::NoKey(k) => format!("no key `{}` in JSON object", k),
            PathError::NotObject(k) => format!("`.{}` applied to a non-object", k),
            PathError::Bracket => "expected `]` in JSON path".into(),
            PathError::BadIndex => "bad index in JSON path".into(),
            PathError::OutOfRange(i) => format!("index {} out of range", i),
            PathError::NotArray(i) => format!("`[{}]` applied to a non-array", i),
            PathError::Near(c) => format!("bad JSON path near `{}`", c),
        }
    }

    pub fn ja(&self) -> String {
        match self {
            PathError::Empty => "JSON のパスが空です".into(),
            PathError::EmptyKey => "JSON のパスに空のキーがあります".into(),
            PathError::NoKey(k) => format!("JSON のオブジェクトに `{}` というキーがありません", k),
            PathError::NotObject(k) => format!("`.{}` をオブジェクトでない値に使っています", k),
            PathError::Bracket => "JSON のパスの `[` が `]` で閉じていません".into(),
            PathError::BadIndex => "JSON のパスのインデックスが数ではありません".into(),
            PathError::OutOfRange(i) => format!("インデックス {} の要素がありません", i),
            PathError::NotArray(i) => format!("`[{}]` を配列でない値に使っています", i),
            PathError::Near(c) => format!("JSON のパスが `{}` のところで読めません", c),
        }
    }
}

/// Path syntax: `.field`, `.a.b`, `.items[0].name`.
pub fn path_get<'a>(root: &'a J, path: &str) -> Result<&'a J, PathError> {
    let b: Vec<char> = path.chars().collect();
    let mut i = 0usize;
    let mut cur = root;
    if b.is_empty() {
        return Err(PathError::Empty);
    }
    while i < b.len() {
        match b[i] {
            '.' => {
                i += 1;
                let start = i;
                while i < b.len() && b[i] != '.' && b[i] != '[' {
                    i += 1;
                }
                let key: String = b[start..i].iter().collect();
                if key.is_empty() {
                    return Err(PathError::EmptyKey);
                }
                match cur {
                    J::Obj(pairs) => match pairs.iter().find(|(k, _)| *k == key) {
                        Some((_, v)) => cur = v,
                        None => return Err(PathError::NoKey(key)),
                    },
                    _ => return Err(PathError::NotObject(key)),
                }
            }
            '[' => {
                i += 1;
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if b.get(i) != Some(&']') {
                    return Err(PathError::Bracket);
                }
                let idx: usize = b[start..i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .map_err(|_| PathError::BadIndex)?;
                i += 1;
                match cur {
                    J::Arr(items) => match items.get(idx) {
                        Some(v) => cur = v,
                        None => return Err(PathError::OutOfRange(idx)),
                    },
                    _ => return Err(PathError::NotArray(idx)),
                }
            }
            c => return Err(PathError::Near(c)),
        }
    }
    Ok(cur)
}

/// Whether a path reads, before anything runs (E009). On failure, the index (from
/// 0) of the character where it stops reading, and why.
pub fn check_path(path: &str) -> Result<(), (usize, PathError)> {
    let b: Vec<char> = path.chars().collect();
    if b.is_empty() {
        return Err((0, PathError::Empty));
    }
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            '.' => {
                let dot = i;
                i += 1;
                let start = i;
                while i < b.len() && b[i] != '.' && b[i] != '[' {
                    i += 1;
                }
                if i == start {
                    return Err((dot, PathError::EmptyKey));
                }
            }
            '[' => {
                let open = i;
                i += 1;
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if b.get(i) != Some(&']') {
                    return Err((open, PathError::Bracket));
                }
                if b[start..i].iter().collect::<String>().parse::<usize>().is_err() {
                    return Err((open, PathError::BadIndex));
                }
                i += 1;
            }
            c => return Err((i, PathError::Near(c))),
        }
    }
    Ok(())
}

/// The value a path that reads leads to, to change it; None when it leads nowhere.
pub fn path_get_mut<'a>(root: &'a mut J, path: &str) -> Option<&'a mut J> {
    let b: Vec<char> = path.chars().collect();
    if b.is_empty() {
        return None;
    }
    let mut i = 0usize;
    let mut cur = root;
    while i < b.len() {
        match b[i] {
            '.' => {
                i += 1;
                let start = i;
                while i < b.len() && b[i] != '.' && b[i] != '[' {
                    i += 1;
                }
                let key: String = b[start..i].iter().collect();
                cur = match cur {
                    J::Obj(pairs) => &mut pairs.iter_mut().find(|(k, _)| *k == key)?.1,
                    _ => return None,
                };
            }
            '[' => {
                i += 1;
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if b.get(i) != Some(&']') {
                    return None;
                }
                let idx: usize = b[start..i].iter().collect::<String>().parse().ok()?;
                i += 1;
                cur = match cur {
                    J::Arr(items) => items.get_mut(idx)?,
                    _ => return None,
                };
            }
            _ => return None,
        }
    }
    Some(cur)
}

pub fn render(j: &J) -> String {
    match j {
        J::Null => "null".into(),
        J::Bool(b) => b.to_string(),
        J::Num(n) => render_num(*n),
        J::Str(s) => format!("\"{}\"", esc(s)),
        J::Arr(items) => {
            let inner: Vec<String> = items.iter().map(render).collect();
            format!("[{}]", inner.join(","))
        }
        J::Obj(pairs) => {
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", esc(k), render(v)))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

pub fn render_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// `s` as a JSON string, quotes included.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", esc(s))
}

pub fn esc(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(src: &str) -> String {
        match parse(src).unwrap() {
            J::Str(s) => s,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn string_escapes() {
        assert_eq!(s(r#""a\"b\\c\/d""#), "a\"b\\c/d");
        assert_eq!(s(r#""\n\t\r\b\f""#), "\n\t\r\u{8}\u{c}");
        let e = format!("\"{}0041\"", "\\u");
        assert_eq!(s(&e), "A");
    }

    #[test]
    fn surrogate_pairs() {
        let pair = format!("\"{}d83d{}de00\"", "\\u", "\\u");
        assert_eq!(s(&pair), "\u{1F600}");
        let lone = format!("\"{}d83d x\"", "\\u");
        assert_eq!(parse(&lone).unwrap_err(), "lone surrogate in JSON string");
    }

    #[test]
    fn numbers() {
        for (src, want) in [("0", 0.0), ("-12", -12.0), ("1.5", 1.5), ("2e3", 2000.0), ("-1.25E-2", -0.0125)] {
            match parse(src).unwrap() {
                J::Num(n) => assert_eq!(n, want, "{src}"),
                other => panic!("{src}: {other:?}"),
            }
        }
        assert_eq!(render_num(12.0), "12");
        assert_eq!(render_num(-3.0), "-3");
        assert_eq!(render_num(1.5), "1.5");
    }

    #[test]
    fn objects_arrays_and_literals_render_back() {
        let src = r#"{ "a": [1, true, null, "x"], "b": { "c": false } }"#;
        assert_eq!(render(&parse(src).unwrap()), r#"{"a":[1,true,null,"x"],"b":{"c":false}}"#);
        assert_eq!(parse("[1,]").unwrap_err(), "unexpected character `]` in JSON");
        assert_eq!(parse("{} x").unwrap_err(), "trailing data after JSON value");
    }

    #[test]
    fn escaping_for_output() {
        assert_eq!(esc("a\"b\\c\nd\te\r"), "a\\\"b\\\\c\\nd\\te\\r");
        assert_eq!(esc("\u{1}"), format!("{}0001", "\\u"));
        assert_eq!(esc("日本語"), "日本語");
    }

    #[test]
    fn paths() {
        let v = parse(r#"{"message":"hi","items":[{"name":"a"},{"name":"b"}]}"#).unwrap();
        assert_eq!(render(path_get(&v, ".message").unwrap()), "\"hi\"");
        assert_eq!(render(path_get(&v, ".items[1].name").unwrap()), "\"b\"");
        assert_eq!(path_get(&v, ".nope").unwrap_err().en(), "no key `nope` in JSON object");
        assert_eq!(path_get(&v, ".items[5]").unwrap_err().en(), "index 5 out of range");
        assert_eq!(path_get(&v, ".message.x").unwrap_err().en(), "`.x` applied to a non-object");
        assert_eq!(path_get(&v, ".message[0]").unwrap_err().en(), "`[0]` applied to a non-array");
        assert_eq!(path_get(&v, "").unwrap_err().en(), "empty JSON path");
        assert_eq!(path_get(&v, "message").unwrap_err().en(), "bad JSON path near `m`");
        assert_eq!(path_get(&v, ".items[x]").unwrap_err().en(), "expected `]` in JSON path");
        assert_eq!(path_get(&v, ".nope").unwrap_err().ja(), "JSON のオブジェクトに `nope` というキーがありません");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn paths_checked_before_anything_runs() {
        assert_eq!(check_path(".items[1].name"), Ok(()));
        assert_eq!(check_path(""), Err((0, PathError::Empty)));
        assert_eq!(check_path("message"), Err((0, PathError::Near('m'))));
        assert_eq!(check_path(".a..b"), Err((2, PathError::EmptyKey)));
        assert_eq!(check_path(".a[x]"), Err((2, PathError::Bracket)));
        assert_eq!(check_path(".a[]"), Err((2, PathError::BadIndex)));
        assert_eq!(check_path(".a[1"), Err((2, PathError::Bracket)));
    }

    #[test]
    fn a_value_changed_through_its_path() {
        let mut v = parse(r#"{"id":7,"items":[{"at":"x"},{"at":"y"}]}"#).unwrap();
        *path_get_mut(&mut v, ".items[1].at").unwrap() = J::Str("z".into());
        *path_get_mut(&mut v, ".id").unwrap() = J::Null;
        assert!(path_get_mut(&mut v, ".nope").is_none());
        assert_eq!(render(&v), r#"{"id":null,"items":[{"at":"x"},{"at":"z"}]}"#);
    }
}
