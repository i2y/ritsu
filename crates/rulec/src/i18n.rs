//! Output language (Japanese or English) for everything the tool prints:
//! diagnostics, reports, the rendered document, and the prose inside
//! generated code.
//!
//! The language is chosen explicitly — `--lang` on the command line, else the
//! `RULEC_LANG` environment variable, else `RITSU_LANG` (the one every language
//! of ritsu reads, ritsu's DESIGN 4.1), else English. The system locale is
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
//!
//! The command line fixes one language for the process (`set`). A program that
//! holds rulec as a library asks for one per call instead, on its own thread,
//! with `with` (ritsu's DESIGN 4.1): the English and the Japanese page of the
//! same rule can be drawn at the same time.

use std::cell::Cell;
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

thread_local! {
    /// The language `with` asked for on this thread, which wins over the process's: 0 = none,
    /// 1 = Japanese, 2 = English.
    static HERE: Cell<u8> = const { Cell::new(0) };
}

fn code(l: Lang) -> u8 {
    if l == Lang::Ja { 1 } else { 2 }
}

/// Run `f` with everything it prints in `l`, on this thread only, then go back to what this
/// thread printed in before (also when `f` panics).
///
/// The CLI fixes one language for the process with [`set`], and nothing changes for it. A
/// program that holds rulec as a library — dandori, yuen, a test that renders the English and
/// the Japanese of the same page side by side — asks for a language per call instead, on as
/// many threads as it likes (ritsu's DESIGN 4.1). rulec starts no thread of its own, so every
/// sentence `f` builds is built on this one.
pub fn with<R>(l: Lang, f: impl FnOnce() -> R) -> R {
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            HERE.with(|h| h.set(self.0));
        }
    }
    let _restore = Restore(HERE.with(|h| h.replace(code(l))));
    f()
}

/// Fix the language for the rest of the process. The CLI calls this before
/// anything is printed; tests call it to render in a specific language. A thread
/// inside [`with`] keeps the language `with` gave it.
pub fn set(l: Lang) {
    LANG.store(code(l), Ordering::Relaxed);
}

/// The language the environment asks for: `RULEC_LANG`, else `RITSU_LANG`, else English. A value
/// that names neither language is passed over, as one that is not there (ritsu-base's
/// `Lang::pick`, which the other languages of ritsu read theirs with).
pub fn from_env() -> Lang {
    match ritsu_base::text::Lang::pick(None, "RULEC_LANG") {
        ritsu_base::text::Lang::Ja => Lang::Ja,
        ritsu_base::text::Lang::En => Lang::En,
    }
}

pub fn current() -> Lang {
    match HERE.with(|h| h.get()) {
        1 => return Lang::Ja,
        2 => return Lang::En,
        _ => {}
    }
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
