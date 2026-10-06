//! Diagnostics (DESIGN 10): ritsu-base's ([`ritsu_base::diag`]) — a code, a place, the message
//! in both languages, notes, the fixed line, and the JSON with its keys in English whatever the
//! language — with sekisho's own part, [`Example`]: a combination that shows what the diagnostic
//! says (DESIGN 4.2), which the checks of every combination fill.
//!
//! A message is spaced and capitalized as the suite's Japanese and English write it
//! ([`ritsu_base::text::spaced`]), as koyomi's and sakai's are: `permit clerks_refund は何も許しません`.

use ritsu_base::diag::Extra;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, spaced};

pub use ritsu_base::diag::Severity;

/// A diagnostic of sekisho's.
pub type Diag = ritsu_base::diag::Diag<Example>;

/// A combination that shows what a diagnostic says: each value by what it is the value of
/// (`principal`, `clerk`, `suspended`, `amount`, `refund_band`), in the order the check walks
/// them. Empty for a diagnostic that is not about combinations.
#[derive(Clone, Debug, Default)]
pub struct Example {
    pub values: Vec<(String, Text)>,
}

impl Extra for Example {
    /// The JSON has `example` between `notes` and `fix`: the values by name, or null.
    fn json(&self, lang: Lang) -> Vec<(String, Json)> {
        let v = if self.values.is_empty() { Json::Null } else { Json::obj(self.values.iter().map(|(k, v)| (k.clone(), Json::str(v.get(lang))))) };
        vec![("example".into(), v)]
    }

    fn say(&self, t: &Text, lang: Lang) -> String {
        spaced(t, lang)
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.is_error())
}

/// The diagnostics in the order a person reads them: by line, then column, then code.
pub fn sort(diags: &mut [Diag]) {
    diags.sort_by(|a, b| (a.line, a.col, a.code).cmp(&(b.line, b.col, b.code)));
}
