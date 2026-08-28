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

/// Path syntax: `.field`, `.a.b`, `.items[0].name`.
pub fn path_get<'a>(root: &'a J, path: &str) -> Result<&'a J, String> {
    let b: Vec<char> = path.chars().collect();
    let mut i = 0usize;
    let mut cur = root;
    if b.is_empty() {
        return Err("empty JSON path".into());
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
                    return Err("empty key in JSON path".into());
                }
                match cur {
                    J::Obj(pairs) => match pairs.iter().find(|(k, _)| *k == key) {
                        Some((_, v)) => cur = v,
                        None => return Err(format!("no key `{}` in JSON object", key)),
                    },
                    _ => return Err(format!("`.{}` applied to a non-object", key)),
                }
            }
            '[' => {
                i += 1;
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if b.get(i) != Some(&']') {
                    return Err("expected `]` in JSON path".into());
                }
                let idx: usize = b[start..i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .map_err(|_| "bad index in JSON path".to_string())?;
                i += 1;
                match cur {
                    J::Arr(items) => match items.get(idx) {
                        Some(v) => cur = v,
                        None => return Err(format!("index {} out of range", idx)),
                    },
                    _ => return Err(format!("`[{}]` applied to a non-array", idx)),
                }
            }
            c => return Err(format!("bad JSON path near `{}`", c)),
        }
    }
    Ok(cur)
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
