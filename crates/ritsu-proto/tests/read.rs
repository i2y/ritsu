//! Reading one file and many: the examples sakai's reader was tested with when it was sakai's
//! (PLAN B.5), and what C.9 added — every option as written and as a tree, Protovalidate's
//! rules, a `oneof` as declared, the files a file imports through every level.

use ritsu_base::text::Lang;
use ritsu_proto::{Label, Problem, ProtoFile, Protos, Resolved, Type, Value, import_candidates, load_from, read};
use std::path::Path;

fn one(src: &str) -> ProtoFile {
    read("t.proto", src).unwrap()
}

#[test]
fn nested_messages_fields_and_options_are_read() {
    let f = one(
        "syntax = \"proto3\";\npackage shop.v1;\nimport public \"a/b.proto\";\nmessage Order {\n  message Line { string sku = 1 [json_name = \"品番\"]; }\n  repeated Line lines = 1;\n  map<string, Line> by_sku = 2;\n  oneof pay { string card = 3; string bank = 4; }\n  enum Status { STATUS_UNSPECIFIED = 0; STATUS_OPEN = 1; }\n  reserved 9 to 11;\n}\nservice S {\n  option (dandori.v1.workflow) = {name: \"引当\", version: 1};\n  rpc Do(stream Order) returns (Order.Line) { option idempotency_level = NO_SIDE_EFFECTS; }\n}\n",
    );
    assert_eq!(f.package, "shop.v1");
    assert_eq!((f.syntax.as_str(), f.syntax_line), ("proto3", Some(1)));
    assert!(f.imports[0].public);
    assert_eq!(f.imports[0].line, 3);
    assert_eq!(f.messages.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["Order", "Order.Line"]);
    let o = f.message("Order").unwrap();
    assert_eq!(o.fields.len(), 4);
    assert_eq!(o.fields[2].oneof.as_deref(), Some("pay"));
    assert_eq!((o.oneofs[0].name.as_str(), o.oneofs[0].fields.clone()), ("pay", vec!["card".to_string(), "bank".to_string()]));
    assert_eq!(f.message("Order.Line").unwrap().fields[0].json_name.as_deref(), Some("品番"));
    assert_eq!(f.enums[0].name, "Order.Status");
    let w = &f.services[0].options[0];
    assert_eq!((w.name.as_str(), w.text.as_str()), ("(dandori.v1.workflow)", "{name: \"引当\", version: 1}"));
    assert_eq!(w.value, Some(Value::Msg(vec![("name".into(), Value::Str("引当".into())), ("version".into(), Value::Num("1".into()))])));
    assert!(f.services[0].methods[0].client_streaming);
    assert_eq!(f.services[0].methods[0].options[0].value, Some(Value::Id("NO_SIDE_EFFECTS".into())));
}

#[test]
fn a_group_is_refused() {
    let e = read("t.proto", "syntax = \"proto2\";\nmessage M {\n  optional group G = 1 { optional int32 a = 2; }\n}\n").unwrap_err();
    assert_eq!((e.line, e.what.clone()), (3, Problem::Group));
}

/// The one message that names the program that read the file.
#[test]
fn a_syntax_it_does_not_know_is_told_in_the_programs_name() {
    let e = read("t.proto", "syntax = \"proto4\";\n").unwrap_err();
    assert_eq!(e.message("sakai").get(Lang::En), "syntax \"proto4\" is not one sakai knows");
    assert_eq!(e.message("dandori").get(Lang::Ja), "syntax \"proto4\" は知りません");
    let e = read("t.proto", "message M { string a = 1 }\n").unwrap_err();
    assert_eq!((e.line, e.col), (1, 26));
    assert_eq!(e.message("sakai").get(Lang::En), "`;` is expected where `}` is");
    assert_eq!(read("t.proto", "message M {").unwrap_err().message("sakai").get(Lang::En), "`}` is expected where the end of the file is");
}

#[test]
fn names_resolve_from_the_innermost_scope() {
    let mut ps = Protos::default();
    let a = read("a/v1/a.proto", "syntax = \"proto3\";\npackage a.v1;\nmessage Outer { message In {} In x = 1; }\nmessage Top { Outer.In y = 1; .a.v1.Outer z = 2; b.v1.B w = 3; }\n").unwrap();
    let b = read("b/v1/b.proto", "syntax = \"proto3\";\npackage b.v1;\nmessage B {}\n").unwrap();
    ps.add(a);
    ps.add(b);
    ps.imports.insert("a/v1/a.proto".into(), vec![Some("b/v1/b.proto".into())]);
    let full = |r: Resolved| match r {
        Resolved::Found(s) => s.full,
        other => format!("{other:?}"),
    };
    assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Outer", "In")), "a.v1.Outer.In");
    assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Top", "Outer.In")), "a.v1.Outer.In");
    assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Top", ".a.v1.Outer")), "a.v1.Outer");
    assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Top", "b.v1.B")), "b.v1.B");
    assert_eq!(ps.resolve("a/v1/a.proto", "a.v1.Top", "c.v1.C"), Resolved::Missing);
    assert_eq!(ps.resolve("a/v1/a.proto", "a.v1.Top", "google.protobuf.Value"), Resolved::Known("google.protobuf.Value".into()));
    let Resolved::Found(top) = ps.resolve("a/v1/a.proto", "a.v1", "Top") else { panic!() };
    let reached: Vec<String> = ps.reach(&[top]).into_iter().map(|s| s.full).collect();
    assert_eq!(reached, vec!["a.v1.Top", "a.v1.Outer.In", "a.v1.Outer", "b.v1.B"]);
}

#[test]
fn where_an_import_is_looked_for() {
    assert_eq!(
        import_candidates("proto/shop/v1/order.proto", "shop.v1", &["proto".into()], "warehouse/v1/stock.proto"),
        vec!["proto/warehouse/v1/stock.proto", "proto/shop/v1/warehouse/v1/stock.proto"]
    );
    assert_eq!(import_candidates("specs/f.proto", "shop.ja.v1", &[], "warehouse.proto"), vec!["specs/warehouse.proto"]);
}

#[test]
fn a_file_and_what_it_imports_through_every_level() {
    let root = Path::new("tests/fixtures/dandori");
    let (ps, issues) = load_from(root, "buf/shop/v1/order.proto", &[], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(ps.order, ["buf/shop/v1/order.proto", "buf/common/v1/money.proto"]);
    let Resolved::Found(m) = ps.resolve("buf/shop/v1/order.proto", "shop.v1.Order", "common.v1.Money") else { panic!() };
    assert_eq!(m.file, "buf/common/v1/money.proto");
    // An import on the disk nowhere is told, and the reading goes on; a known file is not read.
    let (ps, issues) = load_from(root, "gone/specs/order.proto", &[], &[]).unwrap();
    let missing: Vec<&str> = issues.iter().filter_map(|i| if let ritsu_proto::Issue::NotFound { import, .. } = i { Some(import.path.as_str()) } else { None }).collect();
    assert_eq!(missing, ["google/api/annotations.proto", "google/type/money.proto"]);
    assert_eq!(ps.resolve("gone/specs/order.proto", "shop.v1.Order", "google.type.Money"), Resolved::Unknown);
    // dandori's options, given as text, are read as a file.
    let (ps, _) = load_from(root, "known.proto", &[], &[("dandori/v1/options.proto", "syntax = \"proto3\";\npackage dandori.v1;\nmessage Status { optional int32 at = 1; }\n")]).unwrap();
    assert!(matches!(ps.resolve("known.proto", "shop.v1.A", "dandori.v1.Status"), Resolved::Found(s) if s.file == "dandori/v1/options.proto"));
    // A file that is there and does not read stops it.
    let (file, e) = load_from(root, "bad/specs/order.proto", &[], &[]).unwrap_err();
    assert_eq!((file.as_str(), e.line), ("bad/specs/money.proto", 1));
}

#[test]
fn presence_and_the_name_in_json() {
    let f = one("syntax = \"proto3\";\nmessage M { optional int32 a = 1; M b = 2; repeated M c = 3; E d = 4; oneof o { string e = 5; } string weight_g = 6; string f = 7 [json_name = \"x\"]; }\nenum E { E_UNSPECIFIED = 0; }\n");
    let m = f.message("M").unwrap();
    let is_enum = |n: &str| n == "E";
    let p: Vec<bool> = m.fields.iter().map(|x| x.has_presence(is_enum)).collect();
    assert_eq!(p, [true, true, false, false, true, false, false]);
    assert_eq!(m.fields[5].json(), "weightG");
    assert_eq!(m.fields[6].json(), "x");
    assert_eq!(m.fields[2].label, Label::Repeated);
    assert!(matches!(&m.fields[3].ty, Type::Named(n) if n == "E"));
}
