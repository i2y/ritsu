//! The marks that make a property of an OpenAPI, AsyncAPI or JSON Schema schema secret (DESIGN
//! 16.6): the extensions OpenAPI's registry keeps, `x-data-classification` (at `confidential` or
//! `restricted`, which is what it is when it does not say) and `x-sensitive-data`, and the format
//! `password` of OpenAPI's format registry. Each language reads the three keywords from its own
//! JSON (serde_json's in dandori, ritsu-base's yaml in sakai) and asks here what they come to.

/// A mark that makes a schema's property secret (DESIGN 16.6).
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaMark {
    /// "x-data-classification", "x-sensitive-data" or "format: password".
    pub keyword: &'static str,
    /// For x-data-classification, "<category>, <sensitivity>" ("PII, confidential"), the
    /// sensitivity as written or, when none is, `confidential`; else "".
    pub detail: String,
}

/// What the three keywords of a schema say, as the caller reads them from its own JSON: its
/// `format`; whether it has `x-sensitive-data` (there, and not `false`); and
/// `x-data-classification`'s `category` and `sensitivity` (None: not there; Some((category,
/// None)): no sensitivity written, which is confidential). Some when the property is secret:
/// x-data-classification at confidential or restricted, x-sensitive-data, or format password,
/// in that order when there are more. `public` and `internal` are not marks (a value an
/// organisation may handle within itself), nor is a sensitivity outside the registry's four, nor
/// `writeOnly`, which the caller does not pass.
pub fn schema_mark(format: Option<&str>, sensitive_data: bool, classification: Option<(&str, Option<&str>)>) -> Option<SchemaMark> {
    if let Some((category, sensitivity)) = classification {
        let level = sensitivity.map(str::trim).unwrap_or("confidential");
        if level.eq_ignore_ascii_case("confidential") || level.eq_ignore_ascii_case("restricted") {
            let category = category.trim();
            let detail = if category.is_empty() { level.to_string() } else { format!("{category}, {level}") };
            return Some(SchemaMark { keyword: "x-data-classification", detail });
        }
    }
    if sensitive_data {
        return Some(SchemaMark { keyword: "x-sensitive-data", detail: String::new() });
    }
    if format.is_some_and(|f| f.trim() == "password") {
        return Some(SchemaMark { keyword: "format: password", detail: String::new() });
    }
    None
}
