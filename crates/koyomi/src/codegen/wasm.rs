//! Wasm (DESIGN 6.2, 13): the module `rust/` gets, behind the canonical ABI of one function,
//! `call: func(input: string) -> string`, as rulec's Wasm target puts a rule (rulec's DESIGN
//! §15.64). `gen` writes four files under `wasm/`:
//!
//! - `<alias>_wasm.rs`, the crate root: the allocator, `call` and `cabi_post_call` of the
//!   canonical ABI, a reader of one JSON object, and the door that reads the inputs, calls the
//!   module's functions and writes the answer;
//! - `<alias>.rs`, the Rust module, byte for byte the one `rust/` gets;
//! - `<alias>.wit`, the world that exports `call`, so that `wasm-tools component new` makes a
//!   component of the module with no change to it;
//! - `<alias>_runner.mjs`, the runner Node runs: every line of `koyomi vectors` through `call`.
//!
//! `rustc` alone builds it ([`build_line`]), into a module that imports nothing. The input is a
//! JSON object with the inputs by name, a date as a `YYYY-MM-DD` string and an integer as a JSON
//! number (a line of `koyomi vectors`, with them under `"in"`, is read the same way), or
//! `{"date": "YYYY-MM-DD"}` for the calendar; the answer is the line `koyomi vectors` writes for
//! that input, `{"in":…,"out":…}`, or a refusal, `{"refused":"<kind>","error":"<sentence>"}`. The
//! kinds are the four of the other targets' errors and `input`, for an object that does not carry
//! the inputs. A host is answered, never trapped, whatever it sends.

use ritsu_emit::header::Comment;
use super::{Unit, rust};
use crate::ast::{Kind, Ty};
use ritsu_base::text::Lang;

/// How the module is built, after the line `koyomi api` gives: for size over speed, without
/// unwinding or symbols (rulec's flags), for the target that imports nothing.
pub const RUSTC_FLAGS: &str = "-C opt-level=s -C lto -C panic=abort -C strip=symbols --target wasm32-unknown-unknown --crate-type cdylib";

/// The line that builds the module, run in `wasm/`: `rustc` alone, no cargo and no crate.
pub fn build_line(alias: &str) -> String {
    format!("rustc --edition 2021 {RUSTC_FLAGS} {alias}_wasm.rs -o {alias}.wasm")
}

/// The two lines that make a component of the module with the `.wit` beside it, run in `wasm/`.
pub fn component_line(alias: &str) -> String {
    format!(
        "wasm-tools component embed {alias}.wit {alias}.wasm -o {alias}.embedded.wasm && wasm-tools component new {alias}.embedded.wasm -o {alias}.component.wasm"
    )
}

/// The words WIT keeps for itself (the component model's WIT.md, as wasm-tools 1.245.1 reads it):
/// a world or a package named like one is written with `%` before it.
pub const WIT_KEYWORDS: &[&str] = &[
    "as", "async", "bool", "borrow", "char", "constructor", "enum", "error-context", "export", "f32", "f64", "flags", "from", "func", "future", "import",
    "include", "interface", "list", "map", "option", "own", "package", "record", "resource", "result", "s16", "s32", "s64", "s8", "static", "stream",
    "string", "tuple", "type", "u16", "u32", "u64", "u8", "use", "variant", "with", "world",
];

/// A WIT identifier from an alias: its words, lowercase, joined by `-` (`payment_terms` gives
/// `payment-terms`; a run of `_` is one break, and one at either end none, since WIT puts a
/// letter or a digit on each side of every `-`). An alias starts with a letter, so the identifier
/// does, and a word after the first may start with a digit (`close-20th`), as WIT allows. One that
/// is a keyword of WIT is written `%`-escaped.
pub fn world(alias: &str) -> String {
    let id = alias.split('_').filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-");
    if WIT_KEYWORDS.contains(&id.as_str()) { format!("%{id}") } else { id }
}

/// The version of the package: `v1` gives `1.0.0`. A version past what a semver's major number
/// holds (more than 64 bits) gives none.
pub fn semver(v: &str) -> Option<String> {
    v.parse::<u64>().ok().map(|n| format!("{n}.0.0"))
}

/// `koyomi:payment-terms@1.0.0`, the package the `.wit` declares.
pub fn package(alias: &str, version: &str) -> String {
    match semver(version) {
        Some(s) => format!("koyomi:{}@{s}", world(alias)),
        None => format!("koyomi:{}", world(alias)),
    }
}

/// The inputs the module reads: those some date takes, in the order they are declared.
fn taken(u: &Unit) -> Vec<usize> {
    (0..u.inputs.len()).filter(|k| u.dates.iter().any(|d| d.params.contains(k))).collect()
}

fn lit(s: &str) -> String {
    ritsu_emit::lit::json(s)
}

fn doc(lines: &[String], indent: &str) -> String {
    lines.iter().map(|l| format!("{indent}/// {l}\n")).collect()
}

/// The sentences of the door's own refusals, of kind `input`, in the language the code is
/// generated in: a format string over `name`, or plain text.
struct Said {
    not_object: &'static str,
    not_utf8: &'static str,
    missing: &'static str,
    not_string: &'static str,
    not_integer: &'static str,
}

fn said(lang: Lang) -> Said {
    match lang {
        Lang::En => Said {
            not_object: "the input is not a JSON object",
            not_utf8: "the input is not UTF-8",
            missing: "{name} is missing",
            not_string: "{name} is not a string",
            not_integer: "{name} is not an integer",
        },
        Lang::Ja => Said {
            not_object: "入力が JSON のオブジェクトではありません",
            not_utf8: "入力が UTF-8 ではありません",
            missing: "{name} がありません",
            not_string: "{name} が文字列ではありません",
            not_integer: "{name} が整数ではありません",
        },
    }
}

/// The canonical ABI of `call: func(input: string) -> string`, and the reader of one JSON object,
/// the same for every file but for the words of their comments and refusals.
fn abi_and_json(u: &Unit) -> String {
    let s = said(u.lang);
    let mut o = String::new();
    o.push_str(&doc(
        &[u.t(tr!(
            "canonical ABI のアロケーター。ホストは入力の文字列をここに置き、call が読み終えて解放する。",
            "The allocator of the canonical ABI: the host puts the input string here, and call frees it once read."
        ))],
        "",
    ));
    o.push_str(
        r#"#[unsafe(no_mangle)]
pub unsafe extern "C" fn cabi_realloc(old: *mut u8, old_size: usize, align: usize, new_size: usize) -> *mut u8 {
    if new_size == 0 {
        return align as *mut u8;
    }
    let p = unsafe {
        if old_size == 0 {
            alloc(Layout::from_size_align_unchecked(new_size, align))
        } else {
            realloc(old, Layout::from_size_align_unchecked(old_size, align), new_size)
        }
    };
    if p.is_null() {
        std::process::abort();
    }
    p
}

"#,
    );
    o.push_str(&doc(
        &[
            u.t(tr!(
                "`call: func(input: string) -> string`。入力は、このモジュールのメモリにある UTF-8 のポインターと長さ。返す値は、答えのポインターと長さの組を指す。その組は cabi_post_call が解放する。",
                "`call: func(input: string) -> string`: the input is the pointer and the length of UTF-8 in this module's memory, and what comes back points at the pointer and the length of the answer, which cabi_post_call frees."
            )),
        ],
        "",
    ));
    o.push_str(&format!(
        r#"#[unsafe(no_mangle)]
pub unsafe extern "C" fn call(ptr: *mut u8, len: usize) -> *mut [u32; 2] {{
    let bytes = if len == 0 {{ Vec::new() }} else {{ unsafe {{ Vec::from_raw_parts(ptr, len, len) }} }};
    let out = match std::str::from_utf8(&bytes) {{
        Ok(text) => _answer(text),
        Err(_) => _refusal("input", {not_utf8}),
    }};
    drop(bytes);
    let s = out.into_boxed_str();
    let n = s.len();
    let p = Box::into_raw(s) as *mut u8;
    Box::into_raw(Box::new([p as u32, n as u32]))
}}

"#,
        not_utf8 = lit(s.not_utf8)
    ));
    o.push_str(&doc(&[u.t(tr!("call が返した答えを解放する。", "Frees the answer call gave."))], ""));
    o.push_str(
        r#"#[unsafe(no_mangle)]
pub unsafe extern "C" fn cabi_post_call(ret: *mut [u32; 2]) {
    let r = unsafe { Box::from_raw(ret) };
    let (p, n) = (r[0] as *mut u8, r[1] as usize);
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, n) as *mut str) });
}

"#,
    );
    o.push_str(&doc(
        &[u.t(tr!(
            "JSON の値のうち、入口が読むもの。ファイルが読むのは、その入力がとる種類の値だけである。",
            "A JSON value, as far as the door reads it. A file reads only the kinds of value its inputs take."
        ))],
        "",
    ));
    o.push_str(
        r#"#[allow(dead_code)]
enum _Json {
    Str(String),
    Num(String),
    Obj(Vec<(String, _Json)>),
    Null,
    Other,
}

"#,
    );
    o.push_str(&doc(&[u.t(tr!("`text` が一つの JSON の値なら、その値。", "The one JSON value `text` is, if it is one."))], ""));
    o.push_str(
        r#"fn _parse(text: &str) -> Option<_Json> {
    let b = text.as_bytes();
    let mut i = 0;
    let v = _value(b, &mut i, 0)?;
    _space(b, &mut i);
    if i == b.len() { Some(v) } else { None }
}

fn _space(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn _value(b: &[u8], i: &mut usize, depth: usize) -> Option<_Json> {
    _space(b, i);
    if depth > 64 {
        return None;
    }
    match *b.get(*i)? {
        b'{' => {
            *i += 1;
            let mut pairs = Vec::new();
            _space(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Some(_Json::Obj(pairs));
            }
            loop {
                _space(b, i);
                let k = _string(b, i)?;
                _space(b, i);
                if b.get(*i) != Some(&b':') {
                    return None;
                }
                *i += 1;
                let v = _value(b, i, depth + 1)?;
                pairs.push((k, v));
                _space(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Some(_Json::Obj(pairs));
                    }
                    _ => return None,
                }
            }
        }
        b'[' => {
            *i += 1;
            _space(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Some(_Json::Other);
            }
            loop {
                _value(b, i, depth + 1)?;
                _space(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Some(_Json::Other);
                    }
                    _ => return None,
                }
            }
        }
        b'"' => _string(b, i).map(_Json::Str),
        b't' => _word(b, i, "true").map(|_| _Json::Other),
        b'f' => _word(b, i, "false").map(|_| _Json::Other),
        b'n' => _word(b, i, "null").map(|_| _Json::Null),
        _ => _number(b, i).map(_Json::Num),
    }
}

fn _word(b: &[u8], i: &mut usize, w: &str) -> Option<()> {
    if b[*i..].starts_with(w.as_bytes()) {
        *i += w.len();
        Some(())
    } else {
        None
    }
}

fn _number(b: &[u8], i: &mut usize) -> Option<String> {
    let start = *i;
    let digits = |i: &mut usize| {
        let s = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > s
    };
    if b.get(*i) == Some(&b'-') {
        *i += 1;
    }
    if b.get(*i) == Some(&b'0') {
        *i += 1;
    } else if !digits(i) {
        return None;
    }
    if b.get(*i) == Some(&b'.') {
        *i += 1;
        if !digits(i) {
            return None;
        }
    }
    if matches!(b.get(*i), Some(b'e' | b'E')) {
        *i += 1;
        if matches!(b.get(*i), Some(b'+' | b'-')) {
            *i += 1;
        }
        if !digits(i) {
            return None;
        }
    }
    Some(String::from_utf8_lossy(&b[start..*i]).into_owned())
}

"#,
    );
    o.push_str(&doc(
        &[u.t(tr!(
            "JSON の文字列。JSON のエスケープは全部読む（ホストの JSON のエンコーダーは、名前を \\u のエスケープで書くことがある）。",
            "A JSON string, with every escape JSON has read: a host's encoder may write a name with \\u escapes."
        ))],
        "",
    ));
    o.push_str(
        r#"fn _string(b: &[u8], i: &mut usize) -> Option<String> {
    if b.get(*i) != Some(&b'"') {
        return None;
    }
    *i += 1;
    let mut s: Vec<u8> = Vec::new();
    let hex = |j: usize| -> Option<u32> { std::str::from_utf8(b.get(j..j + 4)?).ok().and_then(|h| u32::from_str_radix(h, 16).ok()) };
    loop {
        let c = *b.get(*i)?;
        *i += 1;
        match c {
            b'"' => return String::from_utf8(s).ok(),
            b'\\' => {
                let e = *b.get(*i)?;
                *i += 1;
                let ch = match e {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'/' => '/',
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    b'u' => {
                        let mut cp = hex(*i)?;
                        *i += 4;
                        if (0xD800..0xDC00).contains(&cp) && b.get(*i) == Some(&b'\\') && b.get(*i + 1) == Some(&b'u') {
                            if let Some(lo) = hex(*i + 2).filter(|l| (0xDC00..0xE000).contains(l)) {
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                *i += 6;
                            }
                        }
                        char::from_u32(cp).unwrap_or('\u{FFFD}')
                    }
                    _ => return None,
                };
                let mut buf = [0u8; 4];
                s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            c if c < 0x20 => return None,
            c => s.push(c),
        }
    }
}

fn _json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

"#,
    );
    o.push_str(&doc(
        &[u.t(tr!(
            "日付やカレンダーの答えの代わりに返す、拒否の行 `{{\"refused\":\"<種類>\",\"error\":\"<文>\"}}`。",
            "The refusal, the answer in the place of the dates or the calendar's: `{{\"refused\":\"<kind>\",\"error\":\"<sentence>\"}}`."
        ))],
        "",
    ));
    o.push_str(
        r#"fn _refusal(kind: &str, error: &str) -> String {
    format!("{{\"refused\":{},\"error\":{}}}", _json_str(kind), _json_str(error))
}

"#,
    );
    o.push_str(&doc(
        &[u.t(tr!(
            "`{{\"in\":{{…}},\"out\":{{…}}}}`。読んだ入力と、その結果。どれも名前と JSON の値の組。",
            "`{{\"in\":{{…}},\"out\":{{…}}}}`: the inputs read and what they give, each a name and its JSON."
        ))],
        "",
    ));
    o.push_str(
        r#"fn _record(inputs: &[(&str, String)], out: &[(&str, String)]) -> String {
    let obj = |pairs: &[(&str, String)]| pairs.iter().map(|(k, v)| format!("{}:{v}", _json_str(k))).collect::<Vec<_>>().join(",");
    format!("{{\"in\":{{{}}},\"out\":{{{}}}}}", obj(inputs), obj(out))
}

"#,
    );
    o.push_str(&doc(
        &[u.t(tr!(
            "一回の呼び出しの入力。オブジェクトそのものか、\"in\" の下にオブジェクトがあればそれ（koyomi vectors の行の形）。同じキーが二度あれば、後の値を読む（JSON.parse と同じ）。",
            "The inputs of one call: the object itself, or the object under \"in\" when it has one, as a line of koyomi vectors does. A key written twice counts as its last value, as JSON.parse reads it."
        ))],
        "",
    ));
    o.push_str(&format!(
        r#"fn _inputs(input: &str) -> Result<Vec<(String, _Json)>, String> {{
    let mut top = match _parse(input) {{
        Some(_Json::Obj(top)) => top,
        _ => return Err(_refusal("input", {not_object})),
    }};
    if let Some(i) = top.iter().rposition(|(k, v)| k == "in" && matches!(v, _Json::Obj(_))) {{
        if let (_, _Json::Obj(inner)) = top.swap_remove(i) {{
            return Ok(inner);
        }}
    }}
    Ok(top)
}}

"#,
        not_object = lit(s.not_object)
    ));
    o
}

/// The readers of the inputs a file takes: `_take`, and `_date_in` and `_int_in` when they are
/// used.
fn readers(u: &Unit, dates: bool, ints: bool) -> String {
    let s = said(u.lang);
    let mut o = String::new();
    if !dates && !ints {
        return o;
    }
    o.push_str(&doc(
        &[u.t(tr!(
            "`name` の値。無ければ（null も無いものとみなす）拒否の行を返す。",
            "The value under `name`, or the refusal when it is not there (null counts as not there)."
        ))],
        "",
    ));
    o.push_str(&format!(
        r#"fn _take<'a>(d: &'a [(String, _Json)], name: &str) -> Result<&'a _Json, String> {{
    match d.iter().rev().find(|(k, _)| k == name) {{
        Some((_, _Json::Null)) | None => Err(_refusal("input", &format!({missing}))),
        Some((_, v)) => Ok(v),
    }}
}}

"#,
        missing = lit(s.missing)
    ));
    if dates {
        o.push_str(&doc(
            &[u.t(tr!(
                "日付の入力。文字列を、モジュールの Date で読む（日付でない文字列は、Date が返す種類 date のエラーになる）。",
                "A date input: a string, read by the module's own Date (a string that is not a date is its error of kind date)."
            ))],
            "",
        ));
        o.push_str(&format!(
            r#"fn _date_in(d: &[(String, _Json)], name: &str) -> Result<k::Date, String> {{
    match _take(d, name)? {{
        _Json::Str(s) => s.parse::<k::Date>().map_err(|e| _refusal(e.kind, &e.message)),
        _ => Err(_refusal("input", &format!({not_string}))),
    }}
}}

"#,
            not_string = lit(s.not_string)
        ));
    }
    if ints {
        o.push_str(&doc(
            &[u.t(tr!(
                "整数の入力。値が整数の JSON の数（`10`、`10.0`、`1e1` のどの書き方でも）で、64 ビットに収まるもの。",
                "An integer input: a JSON number whose value is a whole number, however it is written (`10`, `10.0`, `1e1`), within 64 bits."
            ))],
            "",
        ));
        o.push_str(&format!(
            r#"fn _int_in(d: &[(String, _Json)], name: &str) -> Result<i64, String> {{
    match _take(d, name)? {{
        _Json::Num(n) => _whole(n).ok_or_else(|| _refusal("input", &format!({not_integer}))),
        _ => Err(_refusal("input", &format!({not_integer}))),
    }}
}}

fn _whole(v: &str) -> Option<i64> {{
    let b = v.as_bytes();
    let (neg, b) = match b.first() {{
        Some(b'-') => (true, &b[1..]),
        _ => (false, b),
    }};
    let e = b.iter().position(|&c| c == b'e' || c == b'E').unwrap_or(b.len());
    let (mant, ex) = b.split_at(e);
    let mut shift: i64 = 0;
    if let Some((_, ex)) = ex.split_first() {{
        let (down, digits) = match ex.first() {{
            Some(b'-') => (true, &ex[1..]),
            Some(b'+') => (false, &ex[1..]),
            _ => (false, ex),
        }};
        for &d in digits {{
            shift = (shift * 10 + i64::from(d - b'0')).min(1 << 40);
        }}
        if down {{
            shift = -shift;
        }}
    }}
    let (int, frac) = match mant.iter().position(|&c| c == b'.') {{
        Some(k) => (&mant[..k], &mant[k + 1..]),
        None => (mant, &mant[mant.len()..]),
    }};
    shift -= frac.len() as i64;
    let (mut n, mut zeros) = (0i64, 0i64);
    for &d in int.iter().chain(frac) {{
        if d == b'0' {{
            zeros += 1;
            continue;
        }}
        for _ in 0..zeros {{
            n = n.checked_mul(10)?;
        }}
        zeros = 0;
        n = n.checked_mul(10)?.checked_sub(i64::from(d - b'0'))?;
    }}
    shift += zeros;
    if n == 0 {{
        return Some(0);
    }}
    if !(0..=19).contains(&shift) {{
        return None;
    }}
    for _ in 0..shift {{
        n = n.checked_mul(10)?;
    }}
    if neg {{ Some(n) }} else {{ n.checked_neg() }}
}}

"#,
            not_integer = lit(s.not_integer)
        ));
    }
    o
}

/// `_is_open`: the answer to `{"date": …}`, as the calendar's vectors write it.
fn is_open(u: &Unit) -> String {
    let c = u.cal.as_ref().unwrap();
    let mut o = doc(
        &[u.t(tr!(
            "`{{\"date\": \"YYYY-MM-DD\"}}` への答え。その日がカレンダー「{}」の営業日か（koyomi vectors がカレンダーに書く行の形）。",
            "The answer to `{{\"date\": \"YYYY-MM-DD\"}}`: whether that day is a business day of the calendar {}, as the calendar's koyomi vectors write it.",
            c.name
        ))],
        "",
    );
    o.push_str(
        r#"fn _is_open(d: &[(String, _Json)]) -> String {
    let day = match _date_in(d, "date") {
        Ok(v) => v,
        Err(r) => return r,
    };
    match k::is_open(day) {
        Ok(open) => _record(&[("date", _json_str(&day.to_string()))], &[("open", open.to_string())]),
        Err(e) => _refusal(e.kind, &e.message),
    }
}

"#,
    );
    o
}

/// `_answer`: the door of the file.
fn answer(u: &Unit) -> String {
    let mut o = String::new();
    if u.kind == Kind::Calendar {
        o.push_str(&doc(&[u.t(tr!("一回の呼び出しの答え。", "The answer to one call."))], ""));
        o.push_str("fn _answer(input: &str) -> String {\n    match _inputs(input) {\n        Ok(d) => _is_open(&d),\n        Err(r) => r,\n    }\n}\n");
        return o;
    }
    let taken = taken(u);
    let what = if u.cal.is_some() {
        u.t(tr!(
            "一回の呼び出しの答え。日付ごとの値（koyomi vectors の行の形）か、`{{\"date\": …}}` ならカレンダーの答え。",
            "The answer to one call: every date, as a line of koyomi vectors has them, or for `{{\"date\": …}}` the calendar's answer."
        ))
    } else {
        u.t(tr!("一回の呼び出しの答え。日付ごとの値（koyomi vectors の行の形）。", "The answer to one call: every date, as a line of koyomi vectors has them."))
    };
    o.push_str(&doc(&[what], ""));
    o.push_str("fn _answer(input: &str) -> String {\n");
    let binding = if taken.is_empty() && u.cal.is_none() { "_d" } else { "d" };
    o.push_str(&format!("    let {binding} = match _inputs(input) {{\n        Ok(d) => d,\n        Err(r) => return r,\n    }};\n"));
    if u.cal.is_some() {
        o.push_str("    if d.iter().any(|(k, _)| k == \"date\") {\n        return _is_open(&d);\n    }\n");
    }
    for &k in &taken {
        let i = &u.inputs[k];
        let reader = if i.ty == Ty::Date { "_date_in" } else { "_int_in" };
        o.push_str(&format!("    let in{k} = match {reader}(&d, {}) {{\n        Ok(v) => v,\n        Err(r) => return r,\n    }};\n", lit(&i.name)));
    }
    let read: Vec<String> = taken
        .iter()
        .map(|&k| {
            let i = &u.inputs[k];
            match i.ty {
                Ty::Date => format!("({}, _json_str(&in{k}.to_string()))", lit(&i.name)),
                Ty::Int => format!("({}, in{k}.to_string())", lit(&i.name)),
            }
        })
        .collect();
    let read = format!("&[{}]", read.join(", "));
    if u.dates.is_empty() {
        o.push_str(&format!("    _record({read}, &[])\n}}\n"));
        return o;
    }
    o.push_str("    let out = (|| -> Result<Vec<(&str, String)>, k::KoyomiError> {\n        Ok(vec![\n");
    for (k, at) in u.outputs() {
        let d = &u.dates[k];
        let args: Vec<String> = d.params.iter().map(|p| format!("in{p}")).collect();
        if at {
            let f = crate::naming::at_alias(&d.alias);
            o.push_str(&format!("            ({}, _json_str(&k::{f}({})?)),\n", lit(&format!("{}.at", d.name)), args.join(", ")));
        } else {
            o.push_str(&format!("            ({}, _json_str(&k::{}({})?.to_string())),\n", lit(&d.name), d.alias, args.join(", ")));
        }
    }
    o.push_str("        ])\n    })();\n");
    o.push_str(&format!("    match out {{\n        Ok(out) => _record({read}, &out),\n        Err(e) => _refusal(e.kind, &e.message),\n    }}\n}}\n"));
    o
}

/// `wasm/<alias>_wasm.rs`, the crate root.
pub fn root(u: &Unit) -> String {
    let a = &u.alias;
    let mut o = String::new();
    for h in &u.header {
        o.push_str(&Comment::Slashes.line(h));
    }
    o.push('\n');
    let what = match u.kind {
        Kind::Dates => {
            let n = super::named(&u.name, &u.alias, false);
            u.t(tr!(
                "{}の日付を計算する Wasm のモジュール。call は入力の JSON のオブジェクトを受け取り、koyomi vectors がその入力に書く行を返す。",
                "The dates of {} as one Wasm module: call takes a JSON object of the inputs and answers the line koyomi vectors writes for them.",
                n.ja; n.en
            ))
        }
        Kind::Calendar => {
            let n = super::named(&u.name, &u.alias, true);
            u.t(tr!(
                "ある日がカレンダー{}の営業日かを返す Wasm のモジュール。call は `{{\"date\": \"YYYY-MM-DD\"}}` を受け取り、koyomi vectors がその日に書く行を返す。",
                "Whether a day is a business day of the calendar {}, as one Wasm module: call takes `{{\"date\": \"YYYY-MM-DD\"}}` and answers the line koyomi vectors writes for that day.",
                n.ja; n.en
            ))
        }
    };
    o.push_str(&format!("//! {what}\n\n"));
    o.push_str(&format!(
        "// {}\n#[allow(dead_code)]\n#[path = \"{a}.rs\"]\nmod k;\n\nuse std::alloc::{{Layout, alloc, realloc}};\n\n",
        u.t(tr!("rust/ に書くものと同じモジュール。この入口が使うのは、その一部だけ。", "The module rust/ gets, as it is; the door uses only some of it."))
    ));
    o.push_str(&abi_and_json(u));
    let taken = taken(u);
    let dates = u.cal.is_some() || taken.iter().any(|k| u.inputs[*k].ty == Ty::Date);
    let ints = taken.iter().any(|k| u.inputs[*k].ty == Ty::Int);
    o.push_str(&readers(u, dates, ints));
    if u.cal.is_some() {
        o.push_str(&is_open(u));
    }
    o.push_str(&answer(u));
    o
}

/// `wasm/<alias>.wit`: the world that exports `call`, with what to send and what comes back in its
/// doc comments.
pub fn wit(u: &Unit) -> String {
    let w = world(&u.alias);
    let word = if u.kind == Kind::Dates { crate::kw::DATES } else { crate::kw::CALENDAR };
    let version = &u.version;
    let mut o = String::new();
    for h in &u.header {
        o.push_str(&Comment::Slashes.line(h));
    }
    o.push('\n');
    o.push_str(&format!("package {};\n\n", package(&u.alias, version)));
    let mut head = format!("{word} {} v{version}", u.name);
    if let Some(d) = &u.description {
        head.push_str(&format!(": {}", ritsu_emit::header::one_line(d)));
    }
    o.push_str(&format!("/// {head}\nworld {w} {{\n"));
    let mut lines: Vec<String> = Vec::new();
    let taken = taken(u);
    match u.kind {
        Kind::Dates => {
            lines.push(u.t(tr!(
                "一回の呼び出し。input は、入力の名前をキーにした JSON のオブジェクト。日付は \"YYYY-MM-DD\" の文字列で、整数は JSON の数で書く。koyomi vectors の行のように、入力を \"in\" の下に置いたものも同じに読む。",
                "One call. `input` is a JSON object with the inputs by name, a date as a \"YYYY-MM-DD\" string and an integer as a JSON number; a line of koyomi vectors, with them under \"in\", is read the same way."
            )));
            if u.cal.is_some() {
                lines.push(u.t(tr!(
                    "{{\"date\": \"YYYY-MM-DD\"}} は、その日がカレンダーの営業日かを問う。そのときの out は open（true か false）。",
                    "{{\"date\": \"YYYY-MM-DD\"}} asks whether that day is a business day of the calendar; out then holds open, true or false."
                )));
            }
        }
        Kind::Calendar => lines.push(u.t(tr!(
            "一回の呼び出し。input は {{\"date\": \"YYYY-MM-DD\"}} で、その日がカレンダーの営業日かを問う。koyomi vectors の行のように、\"in\" の下に置いたものも同じに読む。",
            "One call. `input` is {{\"date\": \"YYYY-MM-DD\"}}, asking whether that day is a business day of the calendar; a line of koyomi vectors, with it under \"in\", is read the same way."
        ))),
    }
    lines.push(u.t(tr!(
        "返すのは、koyomi vectors がその入力に書く行 {{\"in\":…,\"out\":…}} か、拒否の行 {{\"refused\":\"<種類>\",\"error\":\"<文>\"}} である。種類は、input（入力が無いか、JSON の値の種類が違う）、range、data、reject、date のどれか。",
        "Back comes the line koyomi vectors writes for that input, {{\"in\":…,\"out\":…}}, or a refusal, {{\"refused\":\"<kind>\",\"error\":\"<sentence>\"}}, the kind being input (an input missing, or a JSON value of the wrong kind), range, data, reject or date."
    )));
    lines.push(String::new());
    if u.kind == Kind::Dates {
        if !taken.is_empty() {
            lines.push(u.t(tr!("入力:", "Inputs:")));
            for &k in &taken {
                let i = &u.inputs[k];
                let (lo, hi) = (i.show(i.lo), i.show(i.hi));
                lines.push(match i.ty {
                    Ty::Date => u.t(tr!("  {}: 日付（YYYY-MM-DD）、{lo}〜{hi}", "  {}: a date (YYYY-MM-DD), {lo}..{hi}", i.name)),
                    Ty::Int => u.t(tr!("  {}: 整数、{lo}〜{hi}", "  {}: an integer, {lo}..{hi}", i.name)),
                });
            }
        }
        if !u.dates.is_empty() {
            lines.push(u.t(tr!("結果（out）:", "Out:")));
            for (k, at) in u.outputs() {
                let d = &u.dates[k];
                lines.push(if at {
                    u.t(tr!("  {}.at: 時刻（RFC 3339 の UTC。Z で終わる）", "  {}.at: a time, RFC 3339 in UTC, ending in Z", d.name))
                } else {
                    u.t(tr!("  {}: 日付（YYYY-MM-DD）", "  {}: a date (YYYY-MM-DD)", d.name))
                });
            }
        }
    }
    if u.kind == Kind::Calendar {
        lines.push(u.t(tr!("結果（out）:", "Out:")));
        lines.push(u.t(tr!("  open: true か false", "  open: true or false")));
    }
    for l in &lines {
        if l.is_empty() {
            o.push_str("    ///\n");
        } else {
            o.push_str(&format!("    /// {l}\n"));
        }
    }
    o.push_str(&format!("    export {};\n}}\n", crate::naming::CALL_SIGNATURE));
    o
}

/// `wasm/<alias>_runner.mjs`: the lines of `koyomi vectors` on standard input, each through `call`,
/// and what the module answers on standard output in the runners' form.
pub fn runner(u: &Unit) -> String {
    let a = &u.alias;
    let mut o = String::new();
    o.push_str(&Comment::Slashes.line(&u.header[0]));
    o.push_str(&format!(
        "// {}\n",
        u.t(tr!(
            "koyomi vectors の行を標準入力から読み、モジュールの答えを一行ずつ書く（日付と時刻を空白で区切る。カレンダーなら true か false。拒否なら error <種類>）",
            "Reads the lines of koyomi vectors on standard input and writes what the module answers, a line each (the dates and the times separated by a space, true or false for a calendar; error <kind> for a refusal)"
        ))
    ));
    o.push_str("import { readFileSync } from \"node:fs\";\nimport { dirname, join } from \"node:path\";\nimport { createInterface } from \"node:readline\";\nimport { fileURLToPath } from \"node:url\";\n\n");
    o.push_str(&format!(
        "// {}\n",
        u.t(tr!(
            "{a}_wasm.rs から組んだモジュール（koyomi api の wasm.build）を、このファイルの隣から読む。モジュールは何も import しない。",
            "The module built from {a}_wasm.rs (koyomi api's wasm.build), beside this file. It imports nothing."
        ))
    ));
    o.push_str(&format!(
        "const here = dirname(fileURLToPath(import.meta.url));\nconst {{ instance }} = await WebAssembly.instantiate(readFileSync(join(here, \"{a}.wasm\")), {{}});\nconst ex = instance.exports;\nconst enc = new TextEncoder();\nconst dec = new TextDecoder();\n\n"
    ));
    o.push_str(&format!(
        "// {}\n",
        u.t(tr!(
            "canonical ABI の call: func(input: string) -> string を一回呼ぶ。メモリは呼び出しの中で大きくなることがあるので、メモリの view は呼んだあとに作る。",
            "One call through the canonical ABI of call: func(input: string) -> string. The views of the memory are made after the call, since the memory may grow during it."
        ))
    ));
    o.push_str(
        r#"function call(text) {
  const bytes = enc.encode(text);
  const ptr = ex.cabi_realloc(0, 0, 1, bytes.length);
  new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes);
  const ret = ex.call(ptr, bytes.length);
  const [p, n] = new Uint32Array(ex.memory.buffer, ret, 2);
  const out = dec.decode(new Uint8Array(ex.memory.buffer, p, n));
  ex.cabi_post_call(ret);
  return out;
}

let buf = [];
for await (const line of createInterface({ input: process.stdin, crlfDelay: Infinity })) {
  if (line === "") continue;
  const answer = JSON.parse(call(line));
  buf.push("refused" in answer ? "error " + answer.refused : Object.values(answer.out).join(" "));
  if (buf.length >= 4096) {
    process.stdout.write(buf.join("\n") + "\n");
    buf = [];
  }
}
if (buf.length > 0) process.stdout.write(buf.join("\n") + "\n");
"#,
    );
    o
}

/// The Rust module beside the crate root: the file `rust/` gets.
pub fn module(u: &Unit) -> String {
    rust::module(u)
}
