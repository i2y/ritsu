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
///
/// Beside it, for each group of computed values an input was run through the rules for, that
/// input ([`crate::walk::Confirmed`]).
#[derive(Clone, Debug, Default)]
pub struct Example {
    pub values: Vec<(String, Text)>,
    pub inputs: Vec<crate::walk::Confirmed>,
}

impl Extra for Example {
    /// The JSON has `example` between `notes` and `fix`: the values by name, or null. After it,
    /// where an input was run for computed values of the example, `example_inputs`: for each
    /// group of them, the values it explains (`computed`), whether it is one input or a region
    /// every input of which comes to them (`kind`: `point` or `region`), and each attribute and
    /// input the rules are given (`given`: its `name` as the gate writes it, the `type` whose
    /// attribute it is or null for an input, its `value`).
    fn json(&self, lang: Lang) -> Vec<(String, Json)> {
        let v = if self.values.is_empty() { Json::Null } else { Json::obj(self.values.iter().map(|(k, v)| (k.clone(), Json::str(v.get(lang))))) };
        let mut out = vec![("example".to_string(), v)];
        if !self.inputs.is_empty() {
            let one = |c: &crate::walk::Confirmed| {
                let given = c.given.iter().map(|x| Json::obj([("name", Json::str(x.name.clone())), ("type", x.ty.clone().map(Json::str).unwrap_or(Json::Null)), ("value", Json::str(x.value.get(lang)))]));
                Json::obj([("computed", Json::arr(c.computed.iter().map(|n| Json::str(n.clone())))), ("kind", Json::str(if c.region { "region" } else { "point" })), ("given", Json::arr(given))])
            };
            out.push(("example_inputs".to_string(), Json::arr(self.inputs.iter().map(one))));
        }
        out
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
