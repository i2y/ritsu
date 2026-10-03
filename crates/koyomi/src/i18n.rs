//! The two languages every message is written in (DESIGN 4.1).
//!
//! A message is a [`Text`]: the Japanese and the English sentence side by side, made with
//! `tr!("日本語", "English")`. Neither can be written without the other. The language is
//! not a property of the process: whoever prints a `Text` says which half it wants. That
//! lets a test render the English and the Japanese golden files of the same diagnostic in
//! the same process, at the same time (dandori's `Diag` keeps its two sentences the same
//! way; rulec's `tr!` reads one language for the whole process instead).

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

    /// `--lang` wins, then `KOYOMI_LANG`, then English. The system locale is not read: the
    /// output of a CI job must not change with the machine it runs on.
    pub fn pick(flag: Option<&str>) -> Lang {
        if let Some(l) = flag.and_then(Lang::parse) {
            return l;
        }
        std::env::var("KOYOMI_LANG").ok().and_then(|s| Lang::parse(&s)).unwrap_or(Lang::En)
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ja => "ja",
        }
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

    /// The same words in both languages: a date, a name, a line of the `.cal`.
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
            n => format!(
                "{} and {}",
                parts[..n - 1].iter().map(|t| t.en.as_str()).collect::<Vec<_>>().join(", "),
                parts[n - 1].en
            ),
        };
        Text { ja, en }
    }
}

/// `tr!("日本語 {x}", "English {x}")`: a [`Text`]. Both halves are `format!` strings over the
/// same arguments, so an argument one language leaves out is a compile error. When the two
/// halves take different arguments (one each of a pair of [`Text`]s, say), a `;` divides
/// them: `tr!("{}曜", "{}", ja_name; en_name)`.
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

/// The columns a string takes in a terminal: two for the wide characters of East Asian
/// scripts, one for the rest. Used to line the steps of a computation up under each other.
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
