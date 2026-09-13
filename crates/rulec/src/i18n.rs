//! Output language (Japanese or English) for everything the tool prints:
//! diagnostics, reports, the rendered document, and the prose inside
//! generated code.
//!
//! The language is chosen explicitly — `--lang` on the command line, else the
//! `RULEC_LANG` environment variable, else English. The system locale is
//! deliberately ignored: generated artifacts are committed and checked with
//! `gen --check`, and CI logs are diffed, so the output must not change with
//! the machine it runs on.
//!
//! English is the default because the first reader of this tool is an agent
//! (§11 principle 7). Japanese comes back with one setting, which is what the
//! approver-facing side of a CI job sets.
//!
//! Every user-facing string is written twice, next to each other, with the
//! `tr!` macro (see `lib.rs`). Message *codes* (E101, W105, …) are language
//! independent and remain the stable API (§11 principle 5).

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    /// `ja`, `en`, and their common longer spellings (`ja_JP`, `en-US`).
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
            Lang::Ja => "ja",
            Lang::En => "en",
        }
    }
}

/// 0 = not decided yet, 1 = Japanese, 2 = English.
static LANG: AtomicU8 = AtomicU8::new(0);

/// Fix the language for the rest of the process. The CLI calls this before
/// anything is printed; tests call it to render in a specific language.
pub fn set(l: Lang) {
    LANG.store(if l == Lang::Ja { 1 } else { 2 }, Ordering::Relaxed);
}

/// The language `RULEC_LANG` asks for, or English when it is unset or unknown.
pub fn from_env() -> Lang {
    std::env::var("RULEC_LANG").ok().and_then(|s| Lang::parse(&s)).unwrap_or(Lang::En)
}

pub fn current() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Ja,
        2 => Lang::En,
        _ => {
            let l = from_env();
            set(l);
            l
        }
    }
}

pub fn ja() -> bool {
    current() == Lang::Ja
}
