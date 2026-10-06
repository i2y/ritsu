//! The marks that make a schema's property secret (DESIGN 16.6): `x-data-classification` at
//! `confidential` or `restricted` (and when it says no sensitivity), `x-sensitive-data`, and
//! `format: password`; not `public`, `internal`, a sensitivity outside the four, or nothing.

use ritsu_base::marks::{SchemaMark, schema_mark};

fn mark(keyword: &'static str, detail: &str) -> Option<SchemaMark> {
    Some(SchemaMark { keyword, detail: detail.to_string() })
}

#[test]
fn x_data_classification_marks_confidential_and_restricted() {
    assert_eq!(schema_mark(None, false, Some(("PII", Some("confidential")))), mark("x-data-classification", "PII, confidential"));
    assert_eq!(schema_mark(None, false, Some(("PCI", Some("restricted")))), mark("x-data-classification", "PCI, restricted"));
    assert_eq!(schema_mark(None, false, Some(("PII", Some("Restricted")))), mark("x-data-classification", "PII, Restricted"), "as written");
    // no sensitivity written is confidential
    assert_eq!(schema_mark(None, false, Some(("PII", None))), mark("x-data-classification", "PII, confidential"));
    assert_eq!(schema_mark(None, false, Some(("", None))), mark("x-data-classification", "confidential"));
    // public and internal are not marks, nor a sensitivity the registry does not have
    for s in ["public", "internal", "high"] {
        assert_eq!(schema_mark(None, false, Some(("PII", Some(s)))), None, "{s}");
    }
}

#[test]
fn x_sensitive_data_and_format_password_mark_too() {
    assert_eq!(schema_mark(None, true, None), mark("x-sensitive-data", ""));
    assert_eq!(schema_mark(Some("password"), false, None), mark("format: password", ""));
    assert_eq!(schema_mark(Some("email"), false, None), None);
    assert_eq!(schema_mark(None, false, None), None);
    // the classification first, then x-sensitive-data, then the format
    assert_eq!(schema_mark(Some("password"), true, Some(("PII", Some("restricted")))).unwrap().keyword, "x-data-classification");
    assert_eq!(schema_mark(Some("password"), true, Some(("PII", Some("internal")))).unwrap().keyword, "x-sensitive-data");
    assert_eq!(schema_mark(Some("password"), false, Some(("PII", Some("public")))).unwrap().keyword, "format: password");
}
