//! Strings (DESIGN 25): a string is a sequence of Unicode scalar values, its code points, and every
//! builtin of strings counts, cuts, finds and compares in code points. The three languages of the
//! generated code disagree on all of it when their own functions are used (TypeScript counts UTF-16
//! units and Go bytes, the white space each `trim` takes off differs, and so do the cases and the
//! numbers each reads), so kaname says what each builtin does here, step by step, and the helper
//! modules of the generated code (`glue/kaname.*`) and the model in Lean take the same steps.
//!
//! - White space is the 25 code points of Unicode's White_Space ([`is_space`]); upper and lower case
//!   are A to Z and a to z alone, and nothing is normalized.
//! - What is cut past either end is cut to the string (`slice`, `take`, `drop`), what is not found is
//!   none (`index_of`), and what cannot be read is none (`parse_number`, `parse_date`): no builtin of
//!   strings fails.

/// The code points of Unicode's White_Space, which `trim` and `words` take for white space: U+0009
/// to U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F and
/// U+3000 (DESIGN 25.3).
pub const SPACES: [char; 25] = [
    '\u{9}', '\u{a}', '\u{b}', '\u{c}', '\u{d}', ' ', '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}',
    '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
];

/// Whether a code point is white space (DESIGN 25.3).
pub fn is_space(c: char) -> bool {
    SPACES.contains(&c)
}

/// The number of code points.
pub fn count(s: &str) -> usize {
    s.chars().count()
}

/// The code points from `from` to just before `to`, each cut to 0 and to the length first; none
/// where `from` is not before `to`.
pub fn slice(s: &str, from: i128, to: i128) -> String {
    let n = count(s) as i128;
    let (a, b) = (from.clamp(0, n), to.clamp(0, n));
    if a >= b {
        return String::new();
    }
    s.chars().skip(a as usize).take((b - a) as usize).collect()
}

/// The first `n` code points (all of them where there are fewer; none for `n` below 1).
pub fn take(s: &str, n: i128) -> String {
    slice(s, 0, n)
}

/// The code points after the first `n`.
pub fn drop(s: &str, n: i128) -> String {
    slice(s, n, count(s) as i128)
}

/// Where `t` first stands in `s`, in code points from 0; 0 for an empty `t`, none where it is not
/// in `s`.
pub fn index_of(s: &str, t: &str) -> Option<usize> {
    s.find(t).map(|byte| s[..byte].chars().count())
}

/// `s` cut at each `sep` from the left, none of two cuts overlapping, the empty strings before the
/// first and after the last kept; one string for each code point where `sep` is empty.
pub fn split(s: &str, sep: &str) -> Vec<String> {
    if sep.is_empty() {
        return s.chars().map(String::from).collect();
    }
    s.split(sep).map(String::from).collect()
}

/// The words: the runs of what is not white space, none of them empty.
pub fn words(s: &str) -> Vec<String> {
    s.split(is_space).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// Every `old` from the left, none of two overlapping, made `new`; `s` as it is where `old` is empty.
pub fn replace(s: &str, old: &str, new: &str) -> String {
    if old.is_empty() {
        return s.to_string();
    }
    s.replace(old, new)
}

/// `s` with the white space at both ends taken off.
pub fn trim(s: &str) -> String {
    s.trim_matches(is_space).to_string()
}

/// `s` with the white space at its start taken off.
pub fn trim_start(s: &str) -> String {
    s.trim_start_matches(is_space).to_string()
}

/// `s` with the white space at its end taken off.
pub fn trim_end(s: &str) -> String {
    s.trim_end_matches(is_space).to_string()
}

/// A to Z made a to z; nothing else changed.
pub fn lower_ascii(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// a to z made A to Z; nothing else changed.
pub fn upper_ascii(s: &str) -> String {
    s.to_ascii_uppercase()
}

/// What `fill` repeated puts before `s` (at its end with `at_end`) to make it `width` code points
/// long, the last repetition cut where it goes past; `s` as it is where `fill` is empty or `s` is
/// `width` long already.
pub fn pad(s: &str, width: i128, fill: &str, at_end: bool) -> String {
    let n = count(s) as i128;
    if fill.is_empty() || n >= width {
        return s.to_string();
    }
    let added: String = fill.chars().cycle().take((width - n) as usize).collect();
    if at_end { format!("{s}{added}") } else { format!("{added}{s}") }
}

/// The number `s` writes: one `+` or `-` at most, then one ASCII digit or more, nothing else, within
/// int64 (DESIGN 25.4). None for white space, `_`, other digits, a point, an exponent, and a number
/// past int64.
pub fn parse_number(s: &str) -> Option<i128> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut v: i128 = 0;
    for b in digits.bytes() {
        v = v.checked_mul(10)?.checked_add((b - b'0') as i128)?;
        if v > i64::MAX as i128 + 1 {
            return None;
        }
    }
    let v = if s.starts_with('-') { -v } else { v };
    (i64::MIN as i128..=i64::MAX as i128).contains(&v).then_some(v)
}

/// The date `s` writes: `YYYY-MM-DD` alone, a day of 0001-01-01 to 9999-12-31 (DESIGN 25.4).
pub fn parse_date(s: &str) -> Option<i64> {
    crate::date::parse(s)
}

/// Whether a number is a Unicode scalar value: 0 to U+10FFFF, but for the surrogates.
pub fn is_scalar(n: i128) -> bool {
    (0..=0x10_ffff).contains(&n) && !(0xd800..=0xdfff).contains(&n)
}

/// The string of these code points; none where one is no scalar value.
pub fn of_code_points(ns: &[i128]) -> Option<String> {
    ns.iter().map(|n| if is_scalar(*n) { char::from_u32(*n as u32) } else { None }).collect()
}

/// The longest code points this module lets a string be made of by padding (`pad_start`,
/// `pad_end`): a width past it stops the evaluation, as a list of more numbers does (DESIGN 25.2,
/// 24.3). A string that long runs out of memory long before it is made, outside the language's
/// meaning.
pub const LONGEST: i128 = 1_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_space_is_unicodes() {
        assert_eq!(SPACES.len(), 25);
        for c in '\0'..=char::MAX {
            assert_eq!(is_space(c), c.is_whitespace(), "{:x}", c as u32);
        }
        assert_eq!(trim("\u{85}a\u{3000}"), "a");
        assert_eq!(trim("\u{feff}a"), "\u{feff}a");
        assert_eq!(trim("\u{1c}a"), "\u{1c}a");
        assert_eq!(words("  a\u{a0} b\tc  "), ["a", "b", "c"]);
        assert!(words(" \u{2028} ").is_empty());
    }

    #[test]
    fn code_points_not_units() {
        let s = "\u{1f44d}\u{1f3fd}x";
        assert_eq!(count(s), 3);
        assert_eq!(slice(s, 1, 2), "\u{1f3fd}");
        assert_eq!(slice(s, -5, 99), s);
        assert_eq!(slice(s, 2, 1), "");
        assert_eq!(take(s, 1), "\u{1f44d}");
        assert_eq!(drop(s, 2), "x");
        assert_eq!(index_of(s, "x"), Some(2));
        assert_eq!(index_of(s, ""), Some(0));
        assert_eq!(index_of(s, "y"), None);
        assert_eq!(pad("\u{e9}", 4, "\u{1f600}-", false), "\u{1f600}-\u{1f600}\u{e9}");
        assert_eq!(pad("ab", 5, "xy", true), "abxyx");
        assert_eq!(pad("ab", 1, "x", true), "ab");
        assert_eq!(pad("ab", 9, "", true), "ab");
    }

    #[test]
    fn cuts_and_replacements() {
        assert_eq!(split(",a,,b,", ","), ["", "a", "", "b", ""]);
        assert_eq!(split("", ","), [""]);
        assert_eq!(split("ab", ""), ["a", "b"]);
        assert!(split("", "").is_empty());
        assert_eq!(split("aaa", "aa"), ["", "a"]);
        assert_eq!(replace("aaa", "aa", "b"), "ba");
        assert_eq!(replace("ab", "", "x"), "ab");
        assert_eq!(replace("a$&b", "$&", "$1"), "a$1b");
        assert_eq!(lower_ascii("\u{130}ABC\u{df}"), "\u{130}abc\u{df}");
        assert_eq!(upper_ascii("\u{df}abc"), "\u{df}ABC");
    }

    #[test]
    fn numbers_and_dates_are_read_one_way() {
        assert_eq!(parse_number("12"), Some(12));
        assert_eq!(parse_number("+12"), Some(12));
        assert_eq!(parse_number("-12"), Some(-12));
        assert_eq!(parse_number("007"), Some(7));
        assert_eq!(parse_number("-9223372036854775808"), Some(i64::MIN as i128));
        for s in ["", "+", "-", " 12", "12 ", "1_000", "\u{ff11}\u{ff12}", "\u{661}\u{662}", "1.0", "1e3", "9223372036854775808", "--1", "+-1"] {
            assert_eq!(parse_number(s), None, "{s:?}");
        }
        assert_eq!(parse_date("2024-02-29"), Some(crate::date::day_of(2024, 2, 29)));
        assert_eq!(parse_date("2026-02-29"), None);
        assert_eq!(parse_date(" 2026-01-01"), None);
        assert_eq!(of_code_points(&[0x61, 0x1f600]), Some("a\u{1f600}".into()));
        assert_eq!(of_code_points(&[0xd800]), None);
        assert_eq!(of_code_points(&[0x110000]), None);
        assert_eq!(of_code_points(&[-1]), None);
    }
}
