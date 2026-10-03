//! A string of the source as a literal of the target.

/// In double quotes, escaped as JSON escapes it: TypeScript, JavaScript, Python, Go and Rust
/// all read it as the same string (koyomi's five targets, chobo's TypeScript).
pub fn json(s: &str) -> String {
    ritsu_base::json::quote(s)
}

/// Python, in single quotes: `\\`, `\'`, `\n`, `\r`, `\t`, and `\xNN` for the other control
/// characters.
pub fn python(s: &str) -> String {
    let mut o = String::from("'");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\'' => o.push_str("\\'"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('\'');
    o
}

/// Go, in double quotes: `\\`, `\"`, `\n`, `\r`, `\t`, and `\xNN` for the other control
/// characters and DEL.
pub fn go(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// SQL: `'本店'`, a `'` written twice.
pub fn sql(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// A name as SQL quotes it: `"在庫"`, a `"` written twice.
pub fn sql_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
