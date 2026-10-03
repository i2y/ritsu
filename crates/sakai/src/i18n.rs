//! The two languages every message is written in (DESIGN 5.1).
//!
//! A message is a [`Text`]: the Japanese and the English sentence side by side, made with
//! `tr!("日本語", "English")`. Neither can be written without the other. The language is not a
//! property of the process: whoever prints a `Text` says which half it wants, so a test can
//! render the English and the Japanese golden files of the same diagnostic in one process
//! (koyomi's `Text` works the same way).

/// The language a person reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ja,
}

impl Lang {
    /// `ja`, `en`, and their longer spellings (`ja_JP`, `en-US`).
    pub fn parse(s: &str) -> Option<Lang> {
        let s = s.trim().to_ascii_lowercase();
        if s.starts_with("ja") {
            Some(Lang::Ja)
        } else if s.starts_with("en") {
            Some(Lang::En)
        } else {
            None
        }
    }

    /// `--lang` wins, then `SAKAI_LANG`, then English. The system locale is not read: the output
    /// of a CI job must not change with the machine it runs on.
    pub fn pick(flag: Option<&str>) -> Lang {
        if let Some(l) = flag.and_then(Lang::parse) {
            return l;
        }
        std::env::var("SAKAI_LANG").ok().and_then(|s| Lang::parse(&s)).unwrap_or(Lang::En)
    }
}

/// One message in both languages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {
    pub ja: String,
    pub en: String,
}

impl Text {
    pub fn new(ja: impl Into<String>, en: impl Into<String>) -> Text {
        Text { ja: ja.into(), en: en.into() }
    }

    /// The same words in both languages: a name, a path, a line of a `.ctx`.
    pub fn same(s: impl Into<String>) -> Text {
        let s = s.into();
        Text { ja: s.clone(), en: s }
    }

    pub fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.en,
            Lang::Ja => &self.ja,
        }
    }

    /// The parts, each language joined with its own separator.
    pub fn join(parts: &[Text], ja_sep: &str, en_sep: &str) -> Text {
        Text {
            ja: parts.iter().map(|t| t.ja.as_str()).collect::<Vec<_>>().join(ja_sep),
            en: parts.iter().map(|t| t.en.as_str()).collect::<Vec<_>>().join(en_sep),
        }
    }

    /// A list the way each language says it: `a、b、c` and `a, b and c`.
    pub fn list(parts: &[Text]) -> Text {
        let ja = parts.iter().map(|t| t.ja.as_str()).collect::<Vec<_>>().join("、");
        let en = match parts.len() {
            0 => String::new(),
            1 => parts[0].en.clone(),
            n => format!("{} and {}", parts[..n - 1].iter().map(|t| t.en.as_str()).collect::<Vec<_>>().join(", "), parts[n - 1].en),
        };
        Text { ja, en }
    }
}

/// `tr!("日本語 {x}", "English {x}")`: a [`Text`]. Both halves are `format!` strings over the
/// same arguments, so an argument one language leaves out is a compile error. When the two
/// halves take different arguments, a `;` divides them: `tr!("{}", "{}", ja; en)`.
#[macro_export]
macro_rules! tr {
    ($ja:literal, $en:literal, $($a:expr),* ; $($b:expr),* $(,)?) => {
        $crate::i18n::Text { ja: format!($ja, $($a),*), en: format!($en, $($b),*) }
    };
    ($ja:literal, $en:literal $(,)?) => {
        $crate::i18n::Text { ja: format!($ja), en: format!($en) }
    };
    ($ja:literal, $en:literal, $($arg:expr),+ $(,)?) => {
        $crate::i18n::Text { ja: format!($ja, $($arg),+), en: format!($en, $($arg),+) }
    };
}

/// An English sentence starts with a capital, whatever the template it came from began with.
pub fn capitalize(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_ascii_lowercase() => c.to_ascii_uppercase().to_string() + cs.as_str(),
        _ => s.to_string(),
    }
}

fn is_kana_or_kanji(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0x3005)
}

/// Japanese text with a space between an ASCII word and the Japanese around it, as the Japanese
/// of the suite writes it: `OrderStatus を`. Digits are left alone (`40営業日`), and so is the
/// text between backquotes, which is written as it is to be typed.
pub fn ja_spacing(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    let mut in_code = false;
    for (i, c) in cs.iter().enumerate() {
        if *c == '`' {
            in_code = !in_code;
        }
        if i > 0 && !in_code {
            let p = cs[i - 1];
            if (p.is_ascii_alphabetic() && is_kana_or_kanji(*c)) || (is_kana_or_kanji(p) && c.is_ascii_alphabetic()) {
                out.push(' ');
            }
        }
        out.push(*c);
    }
    out
}

/// A message as the language prints it.
pub fn say(t: &Text, lang: Lang) -> String {
    match lang {
        Lang::En => capitalize(&t.en),
        Lang::Ja => ja_spacing(&t.ja),
    }
}

/// The columns a string takes in a terminal: two for the wide characters of East Asian scripts,
/// one for the rest.
pub fn width(s: &str) -> usize {
    s.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

fn is_wide(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF |
        0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 |
        0xFFE0..=0xFFE6 | 0x20000..=0x2FFFD | 0x30000..=0x3FFFD)
}

/// `s` padded with spaces to `w` columns.
pub fn pad(s: &str, w: usize) -> String {
    let n = width(s);
    if n >= w { s.to_string() } else { format!("{s}{}", " ".repeat(w - n)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_space_between_ascii_and_japanese_but_not_in_code() {
        assert_eq!(ja_spacing("OrderStatusの値"), "OrderStatus の値");
        // A digit is left alone, so a name such as `受領から60日以内` keeps its letters; a
        // template that puts a package (`warehouse.v1`) before Japanese writes the space itself.
        assert_eq!(ja_spacing("warehouse.v1の列挙"), "warehouse.v1の列挙");
        assert_eq!(ja_spacing("`a`の値"), "`a`の値");
        assert_eq!(ja_spacing("40営業日"), "40営業日");
    }

    #[test]
    fn a_list_in_each_language() {
        let t = Text::list(&[Text::same("a"), Text::same("b"), Text::same("c")]);
        assert_eq!(t.ja, "a、b、c");
        assert_eq!(t.en, "a, b and c");
    }
}
