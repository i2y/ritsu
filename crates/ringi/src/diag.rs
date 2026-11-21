//! Diagnostics (DESIGN 9.5): ritsu-base's ([`ritsu_base::diag`]) — a code, a place, the message in
//! both languages, notes, the fixed line, and the JSON with its keys in English whatever the
//! language.
//!
//! A message is spaced and capitalized as the suite's Japanese and English write it
//! ([`ritsu_base::text::spaced`]), as kaname's, koyomi's and sekisho's are: `ステップ second の
//! 議決に要る数は…`.

use ritsu_base::diag::Extra;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, spaced};

pub use ritsu_base::diag::Severity;

/// What ringi adds to ritsu-base's diagnostic: the way a message is printed, and, for a diagnostic
/// of a lowered file said again in ringi's words (DESIGN 6.5, 16.2), the example request
/// (`witness`), the rows of the request it is about (`rows`) and the code the lowered file's
/// language gave (`from`), which the JSON writes between the notes and the fix.
#[derive(Clone, Debug, Default)]
pub struct Spaced {
    pub witness: Option<Json>,
    pub rows: Option<Json>,
    pub from: Option<String>,
}

impl Extra for Spaced {
    fn say(&self, t: &Text, lang: Lang) -> String {
        spaced(t, lang)
    }

    fn json(&self, _lang: Lang) -> Vec<(String, Json)> {
        let mut out = Vec::new();
        if let Some(w) = &self.witness {
            out.push(("witness".to_string(), w.clone()));
        }
        if let Some(r) = &self.rows {
            out.push(("rows".to_string(), r.clone()));
        }
        if let Some(f) = &self.from {
            out.push(("from".to_string(), Json::str(f)));
        }
        out
    }
}

/// A diagnostic of ringi's.
pub type Diag = ritsu_base::diag::Diag<Spaced>;

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.is_error())
}

/// The diagnostics in the order a person reads them: by file (the file checked first, then the
/// files of data it read, by their paths), line, then column, then code; the same diagnostic said
/// twice (a name read on two paths) is said once, and so is a name declared nowhere, where the file
/// first writes it, with the other lines that write it in a note. Two that differ only in their notes are two
/// findings (one person at two stages, for each value of a rule's output that names the second),
/// and both are said. The sort is stable, so that diagnostics at one place keep the order they were
/// said in.
pub fn sort(diags: &mut Vec<Diag>) {
    // a name declared nowhere, once (ritsu-base's `once`; `names::undeclared`)
    ritsu_base::diag::once(diags);
    let first = diags.iter().find(|d| d.file.ends_with(".ringi")).map(|d| d.file.clone()).unwrap_or_default();
    diags.sort_by(|a, b| (a.file != first, &a.file, a.line, a.col, a.code).cmp(&(b.file != first, &b.file, b.line, b.col, b.code)));
    diags.dedup_by(|a, b| a.code == b.code && a.file == b.file && a.line == b.line && a.col == b.col && a.message == b.message && a.notes == b.notes);
}
