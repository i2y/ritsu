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

/// A name moved aside from the words `taken` holds: with `_` after it when it is one of them.
pub fn aside(s: &str, taken: impl Fn(&str) -> bool) -> String {
    if taken(s) { format!("{s}_") } else { s.to_string() }
}

/// Names made unique: the second of two that come out the same gets `_2`, the third `_3`;
/// `taken` are names already used.
pub fn unique(names: Vec<String>, taken: &[&str]) -> Vec<String> {
    let mut seen: Vec<String> = taken.iter().map(|s| s.to_string()).collect();
    let mut out = Vec::new();
    for n in names {
        let mut m = n.clone();
        let mut i = 2;
        while seen.contains(&m) {
            m = format!("{n}_{i}");
            i += 1;
        }
        seen.push(m.clone());
        out.push(m);
    }
    out
}
