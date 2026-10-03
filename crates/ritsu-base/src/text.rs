//! The two languages every message is written in (DESIGN 4.1).
//!
//! A message is a [`Text`]: the Japanese and the English sentence side by side, made with
//! `tr!("日本語", "English")`. Neither can be written without the other. The language is not a
//! property of the process: whoever prints a `Text` says which half it wants. A test renders
//! the English and the Japanese golden files of the same diagnostic in one process, a page in
//! the browser asks for either on every call, and one language's crate can ask another's for
//! the language it prints in. (koyomi, chobo, yuen and sakai kept their messages this way;
//! rulec's `tr!` reads one language for the whole process, and keeps it until stage D.)

/// The language a person reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ja => "ja",
        }
    }

    /// The language a run prints in (DESIGN 4.1): `--lang` wins, then the tool's own variable
    /// (`KOYOMI_LANG`, say), then `RITSU_LANG`, then English. The system locale is not read: what
    /// a CI job prints must not change with the machine it runs on, and the generated code that
    /// is committed must not either.
    pub fn pick(flag: Option<&str>, tool_var: &str) -> Lang {
        let tool = std::env::var(tool_var).ok();
        let ritsu = std::env::var("RITSU_LANG").ok();
        Lang::choose(flag, tool.as_deref(), ritsu.as_deref())
    }

    /// [`Lang::pick`] over values already read. A value that names neither language is passed
    /// over, as one that is not there.
    pub fn choose(flag: Option<&str>, tool: Option<&str>, ritsu: Option<&str>) -> Lang {
        [flag, tool, ritsu].into_iter().flatten().find_map(Lang::parse).unwrap_or(Lang::En)
    }
}

/// One message in both languages.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Text {
    pub ja: String,
    pub en: String,
}

impl Text {
    pub fn new(ja: impl Into<String>, en: impl Into<String>) -> Text {
        Text { ja: ja.into(), en: en.into() }
    }

    /// The same words in both languages: a date, a name, a line of a file.
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

    pub fn is_empty(&self) -> bool {
        self.ja.is_empty() && self.en.is_empty()
    }

    /// Both halves of `self` followed by both halves of `other`.
    pub fn then(mut self, other: &Text) -> Text {
        self.ja.push_str(&other.ja);
        self.en.push_str(&other.en);
        self
    }

    /// `val` where the sentence has `{key}`, in each language its own words (chobo's). A
    /// sentence written with `tr!` spells the place `{{key}}`.
    pub fn sub(mut self, key: &str, val: &Text) -> Text {
        let k = format!("{{{key}}}");
        self.ja = self.ja.replace(&k, &val.ja);
        self.en = self.en.replace(&k, &val.en);
        self
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
/// halves take different arguments (one each of a pair of [`Text`]s, say), a `;` divides
/// them: `tr!("{}件", "{}", ja_count; en_count)`.
#[macro_export]
macro_rules! tr {
    ($ja:literal, $en:literal, $($a:expr),* ; $($b:expr),* $(,)?) => {
        $crate::text::Text { ja: format!($ja, $($a),*), en: format!($en, $($b),*) }
    };
    ($ja:literal, $en:literal $(,)?) => {
        $crate::text::Text { ja: format!($ja), en: format!($en) }
    };
    ($ja:literal, $en:literal, $($arg:expr),+ $(,)?) => {
        $crate::text::Text { ja: format!($ja, $($arg),+), en: format!($en, $($arg),+) }
    };
}

/// A message as written: the English with the case its template gave it, the Japanese with
/// the spaces its template gave it. yuen prints this way, since a sentence may start with a
/// name (`r1`, `file "a.txt"`) that must keep its case, and a name may hold a path that must
/// not gain a space in the middle.
pub fn as_written(t: &Text, lang: Lang) -> String {
    t.get(lang).to_string()
}

/// A message spaced as the suite's Japanese writes it and capitalized as an English sentence
/// starts (koyomi and sakai print this way): [`capitalize`] and [`ja_spacing`].
pub fn spaced(t: &Text, lang: Lang) -> String {
    match lang {
        Lang::En => capitalize(&t.en),
        Lang::Ja => ja_spacing(&t.ja),
    }
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
/// of the suite writes it: `OrderStatus を`, `invoice_date は`. Digits are left alone
/// (`第140条`, `40営業日`), and so is `_`, which joins a name like `満了日_翌日`, and the text
/// between backquotes, which is written as it is to be typed.
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

/// `1,067` and `871,596`: a count as both languages print it.
pub fn count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// An English noun with its count: `1 link`, `3 links`, `1,067 days`.
pub fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{} {many}", count(n as u64)) }
}

/// The columns a string takes in a terminal: two for the wide and full-width characters of
/// East Asian scripts (East Asian Width W and F), one for the rest. Used to line columns up.
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
        assert_eq!(ja_spacing("warehouse.v1の列挙"), "warehouse.v1の列挙");
        assert_eq!(ja_spacing("`a`の値"), "`a`の値");
        assert_eq!(ja_spacing("40営業日"), "40営業日");
        assert_eq!(ja_spacing("満了日_翌日"), "満了日_翌日");
    }

    #[test]
    fn a_list_in_each_language() {
        let t = Text::list(&[Text::same("a"), Text::same("b"), Text::same("c")]);
        assert_eq!((t.ja.as_str(), t.en.as_str()), ("a、b、c", "a, b and c"));
        assert_eq!(Text::list(&[Text::same("a")]).en, "a");
        assert_eq!(Text::list(&[]).en, "");
    }
}
