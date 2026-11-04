//! Making a name of the generated code from a name of the source: the cases a target writes
//! a name in, a name moved aside from a word the target already uses, and names kept apart.

/// `payment_at` → `PaymentAt`: the parts between `_`, each with its first letter in upper case
/// (koyomi's Go functions).
pub fn pascal(alias: &str) -> String {
    alias
        .split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut cs = p.chars();
            match cs.next() {
                Some(c) => c.to_ascii_uppercase().to_string() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// `payment_terms` → `paymentterms`: a Go package's name from an alias.
pub fn go_package(alias: &str) -> String {
    alias.replace('_', "")
}

/// A name as Go exports it: `sku` is `Sku`, a name already in upper case is as it is, and a name
/// whose first letter has no capital, as Japanese has none, gets an `X` in front (`X引当`).
pub fn go_exported(s: &str) -> String {
    let mut cs = s.chars();
    let Some(first) = cs.next() else { return "X".into() };
    if first.is_uppercase() {
        return s.to_string();
    }
    let mut up = first.to_uppercase();
    if let (Some(u), None) = (up.next(), up.next())
        && u != first
        && u.is_uppercase()
    {
        return format!("{u}{}", cs.as_str());
    }
    format!("X{s}")
}

/// Whether a name is an ASCII identifier: a letter or `_`, then letters, digits and `_`.
pub fn is_ascii_ident(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether Rust takes `name`, a name of ritsu's languages, as a name by its characters: XID_Start
/// or `_` first, then XID_Continue (The Rust Reference, Identifiers). A keyword is taken too, as a
/// raw identifier ([`rust_raw`]); `_` alone is the caller's to say.
pub fn rust_takes(name: &str) -> bool {
    takes(name, crate::xid::NOT_FIRST, crate::xid::NOT_AFTER)
}

/// Whether Python takes `name`, by its characters: as Rust does (The Python Language Reference,
/// 2.3), as far as the Unicode of Python's `unicodedata` goes.
pub fn python_takes(name: &str) -> bool {
    takes(name, crate::xid::NOT_FIRST, crate::xid::NOT_AFTER)
}

/// Whether TypeScript takes `name`, by its characters: ID_Start or `_` first, then ID_Continue
/// (ECMA-262, IdentifierName). ID holds XID, and a few more that NFKC makes other characters:
/// TypeScript takes `ͺ` and the halfwidth `ﾞ`, which Rust and Python do not.
pub fn typescript_takes(name: &str) -> bool {
    takes(name, crate::xid::ID_NOT_FIRST, crate::xid::ID_NOT_AFTER)
}

/// Whether Go takes `name`, by its characters: a letter (Lu, Ll, Lt, Lm, Lo) or `_` first, then
/// letters, decimal digits (Nd) and `_` (The Go Programming Language Specification, Identifiers).
/// A mark and a number of another kind are not: Go takes neither `कीमत`, whose `ी` is a vowel
/// sign, nor `Ⅻ`.
pub fn go_takes(name: &str) -> bool {
    takes(name, crate::xid::GO_NOT_FIRST, crate::xid::GO_NOT_AFTER)
}

/// Whether a name begins with a letter or `_` that `not_first` does not hold, and goes on with
/// letters, numbers and `_` that `not_after` does not.
fn takes(name: &str, not_first: &[(u32, u32)], not_after: &[(u32, u32)]) -> bool {
    let mut cs = name.chars();
    matches!(cs.next(), Some(c) if c == '_' || (c.is_alphabetic() && !held(not_first, c))) && cs.all(|c| c == '_' || (c.is_alphanumeric() && !held(not_after, c)))
}

/// Whether one of the ranges of `table` holds `c`.
fn held(table: &[(u32, u32)], c: char) -> bool {
    let u = c as u32;
    table.binary_search_by(|&(lo, hi)| if hi < u { std::cmp::Ordering::Less } else if lo > u { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }).is_ok()
}

/// The words of Rust no raw identifier writes (`r#self` is no name): a name that is one of them is
/// moved aside instead.
pub const RUST_UNRAW: [&str; 5] = ["crate", "self", "super", "Self", "_"];

/// A name as Rust writes it: as it is, or a raw identifier where it is a keyword of Rust (`r#ref`,
/// `r#gen`, which edition 2021 writes too); None for a word no raw identifier writes
/// ([`RUST_UNRAW`]), which the caller moves aside.
pub fn rust_raw(name: &str) -> Option<String> {
    if RUST_UNRAW.contains(&name) {
        return None;
    }
    Some(if crate::words::rust::NAMES.contains(name) { format!("r#{name}") } else { name.to_string() })
}

/// [`rust_raw`], a word no raw identifier writes with `_` after it (`self_`, `Self_`), as chobo's
/// Rust clients write it.
pub fn rust_name(name: &str) -> String {
    rust_raw(name).unwrap_or_else(|| format!("{name}_"))
}

/// A name moved aside from the words `taken` holds: with `_` after it when it is one of them.
pub fn aside(s: &str, taken: impl Fn(&str) -> bool) -> String {
    if taken(s) { format!("{s}_") } else { s.to_string() }
}
