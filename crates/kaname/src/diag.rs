//! Diagnostics (DESIGN 12): ritsu-base's ([`ritsu_base::diag`]) — a code, a place, the message in
//! both languages, notes, the fixed line, and the JSON with its keys in English whatever the
//! language.
//!
//! A message is spaced and capitalized as the suite's Japanese and English write it
//! ([`ritsu_base::text::spaced`]), as koyomi's, sakai's and sekisho's are: `規則 charges の入力
//! days_late は…`.

use ritsu_base::diag::Extra;
use ritsu_base::text::{Lang, Text, spaced};

pub use ritsu_base::diag::Severity;

/// What kaname adds to ritsu-base's diagnostic: nothing yet but the way a message is printed.
#[derive(Clone, Debug, Default)]
pub struct Spaced;

impl Extra for Spaced {
    fn say(&self, t: &Text, lang: Lang) -> String {
        spaced(t, lang)
    }
}

/// A diagnostic of kaname's.
pub type Diag = ritsu_base::diag::Diag<Spaced>;

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.is_error())
}

/// The diagnostics in the order a person reads them: by line, then column, then code.
pub fn sort(diags: &mut Vec<Diag>) {
    diags.sort_by(|a, b| (a.line, a.col, a.code).cmp(&(b.line, b.col, b.code)));
    // the same diagnostic said twice (a name read on two paths) is said once
    diags.dedup_by(|a, b| a.code == b.code && a.line == b.line && a.col == b.col && a.message == b.message);
}

/// Japanese text that a particle follows: with a space after it when it ends with a name in
/// backquotes or an ASCII word (`振替 \`ledger.capture.do\` は`), as the suite writes it; as it is
/// when it ends in Japanese (`関数 \`f\` の答えは`).
pub fn ja_before(t: &str) -> String {
    match t.chars().last() {
        Some(c) if c == '`' || c.is_ascii_alphanumeric() || c == ']' || c == ')' => format!("{t} "),
        _ => t.to_string(),
    }
}
