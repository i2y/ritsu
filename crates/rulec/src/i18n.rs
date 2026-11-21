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
//! reader-facing side of a CI job sets.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Inside [`making`], the language the run prints its text in ([`said`]): 0 = not inside.
    static SAID: Cell<u8> = const { Cell::new(0) };
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
/// many threads as it likes (ritsu's DESIGN 4.1). A thread rulec starts itself — the workers of
/// `strength`, the watch `verify` keeps on a silent adapter — is given the language of the thread
/// that started it, with this same call.
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

/// Run `f`, which makes what a command writes for others to read and keep (the code `gen` writes,
/// a page, the vectors), with what it makes in `made` (ritsu's DESIGN 4.1: `--lang` alone, English
/// without it), while what it prints for the person running it (a finding, an error, `generated: …`)
/// stays in the language the run prints in, through [`said`].
pub fn making<R>(made: Lang, f: impl FnOnce() -> R) -> R {
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SAID.with(|s| s.set(self.0));
        }
    }
    let _restore = Restore(SAID.with(|s| s.replace(code(current()))));
    with(made, f)
}

/// Run `f`, which makes text the run prints for the person running it, in the language the run
/// prints in: inside [`making`], that language rather than the language of what is made.
pub fn said<R>(f: impl FnOnce() -> R) -> R {
    match SAID.with(|s| s.get()) {
        1 => with(Lang::Ja, f),
        2 => with(Lang::En, f),
        _ => f(),
    }
}

/// The current language, as ritsu-base names it.
pub fn base() -> ritsu_base::text::Lang {
    if ja() { ritsu_base::text::Lang::Ja } else { ritsu_base::text::Lang::En }
}

/// The language `--lang` gives what a command writes for others to keep ([`making`]), as
/// ritsu-base reads it (`Lang::made`): English without the flag, whatever the environment says.
pub fn made(flag: Option<&str>) -> Lang {
    match ritsu_base::text::Lang::made(flag) {
        ritsu_base::text::Lang::Ja => Lang::Ja,
        ritsu_base::text::Lang::En => Lang::En,
    }
}

/// Stop a command before its work (ritsu's DESIGN 6.2, item 10): `t` written on `err` after the
/// word of an error in the language the run prints in, as every command of ritsu writes it, and 2.
pub fn refuse(err: &mut dyn std::io::Write, t: &ritsu_base::text::Text) -> u8 {
    said(|| ritsu_base::cli::refuse_said(err, &text(t), base()))
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

/// A sentence of ritsu-base's (`ritsu_base::text::Text`), in the current output language.
pub fn text(t: &ritsu_base::text::Text) -> String {
    if ja() { t.ja.clone() } else { t.en.clone() }
}
