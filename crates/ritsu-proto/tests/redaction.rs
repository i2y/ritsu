//! Fields marked to be redacted (DESIGN 16.6): `debug_redact = true` written on the field, and a
//! custom option whose value is an enum value marked so — the option and its enum in one file or
//! in two, an extension declared inside a message — looked up through the files the field's file
//! sees; and what marks nothing: `debug_redact = false`, a value not marked, an option declared in
//! a file not imported. The files are in `tests/redaction/`, out of the readers' goldens.

use ritsu_proto::{Extension, Protos, Redaction, Type, load_from, read};
use std::path::Path;

fn protos(entry: &str) -> Protos {
    let (ps, issues) = load_from(Path::new("tests/redaction"), entry, &[], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    ps
}

fn redactions(ps: &Protos, file: &str, message: &str) -> Vec<(String, Option<Redaction>)> {
    let m = ps.files[file].message(message).unwrap();
    m.fields.iter().map(|f| (f.name.clone(), ps.redaction(file, f))).collect()
}

fn by_option(option: &str, value: &str, file: &str, line: usize) -> Option<Redaction> {
    Some(Redaction::ByOption { option: option.into(), value: value.into(), file: file.into(), line })
}

#[test]
fn the_fields_of_an_extend_are_read() {
    let ps = protos("bank/v1/account.proto");
    let x = |name: &str, ty: &str, number: i64, line: usize| Extension { extendee: "google.protobuf.FieldOptions".into(), name: name.into(), ty: Type::Named(ty.into()), number, line };
    assert_eq!(ps.files["acme/v1/sensitivity.proto"].extensions, [x("sensitivity", "Sensitivity", 50001, 14), x("Holder.tag", "Sensitivity", 50003, 20)]);
    assert_eq!(ps.files["acme/v1/options.proto"].extensions, [x("level", "Level", 50002, 10)]);
    // the message an extend is declared in keeps its fields to itself
    assert!(ps.files["acme/v1/sensitivity.proto"].message("Holder").unwrap().fields.is_empty());
    // a file without an extend has none, and an extend of something else is read the same
    assert!(ps.files["bank/v1/account.proto"].extensions.is_empty());
    let f = read("t.proto", "syntax = \"proto2\";\nmessage A { extensions 100 to 199; }\nextend A {\n  optional int32 b = 100;\n  repeated string c = 101 [packed = false];\n}\n").unwrap();
    let got: Vec<(&str, &str, i64, usize)> = f.extensions.iter().map(|e| (e.extendee.as_str(), e.name.as_str(), e.number, e.line)).collect();
    assert_eq!(got, [("A", "b", 100, 4), ("A", "c", 101, 5)]);
    assert_eq!(f.extensions[0].ty, Type::Scalar("int32".into()));
}

#[test]
fn a_field_is_marked_directly_or_through_a_custom_option() {
    let ps = protos("bank/v1/account.proto");
    assert_eq!(
        redactions(&ps, "bank/v1/account.proto", "BankAccount"),
        [
            ("id".to_string(), None),
            ("number".to_string(), Some(Redaction::Direct { line: 11 })),
            ("holder".to_string(), by_option("(acme.v1.sensitivity)", "PERSONAL", "acme/v1/sensitivity.proto", 10)),
            ("branch".to_string(), None),
            // the option in options.proto, its enum in levels.proto, which options.proto imports
            ("iban".to_string(), by_option("(acme.v1.level)", "SECRET", "acme/v1/levels.proto", 8)),
            ("nickname".to_string(), None),
            // declared inside the message Holder
            ("note".to_string(), by_option("(acme.v1.Holder.tag)", "PERSONAL", "acme/v1/sensitivity.proto", 10)),
            ("swift".to_string(), None),
            ("token".to_string(), Some(Redaction::Direct { line: 18 })),
        ]
    );
}

#[test]
fn an_option_the_file_does_not_see_marks_nothing() {
    let ps = protos("bank/v1/unseen.proto");
    assert_eq!(
        redactions(&ps, "bank/v1/unseen.proto", "Card"),
        [("pan".to_string(), None), ("name".to_string(), by_option("(.acme.v1.sensitivity)", "PERSONAL", "acme/v1/sensitivity.proto", 10))]
    );
}
