//! What C.9 added from rulec's and dandori's readers, held to the values their own tests held
//! them to: the options as a tree (dandori), Protovalidate's rules (rulec), and a module's
//! `buf.yaml` and `buf.lock` (rulec).

use ritsu_proto::value::tree;
use ritsu_proto::{ProtoFile, buf, read};

fn one(src: &str) -> ProtoFile {
    read("t.proto", src).unwrap()
}

#[test]
fn the_options_of_an_element_make_one_tree() {
    let f = one(
        r#"syntax = "proto3"; package shop.v1;
        service S {
          option (dandori.v1.workflow) = {name: "fulfillment", version: 1};
          option (x.z) = {name: "b"; version: 2; inner { deep: [1, 2] } tag: "a;b]c"};
          rpc Start(A) returns (B) {
            option (dandori.v1.start) = {fails: ["OutOfStock", "DeliveryFailed"]};
            option idempotency_level = NO_SIDE_EFFECTS;
          }
          rpc Again(A) returns (B) { option (.dandori.v1.start) = {fails: "One" fails: "Two"}; }
        }
        message A {
          int32 apart = 1 [(buf.validate.field).int32.gte = 1, (buf.validate.field).int32.lt = 50, json_name = "x"];
          string s = 2 [(buf.validate.field).string.(my.rule) = true, (buf.validate.field).string.min_len = 2, deprecated = true];
        }
        message B {}"#,
    );
    let s = &f.services[0];
    assert_eq!(tree(&s.options).compact(), r#"{"dandori.v1.workflow":{"name":"fulfillment","version":1},"x.z":{"name":"b","version":2,"inner":{"deep":[1,2]},"tag":"a;b]c"}}"#);
    assert_eq!(tree(&s.methods[0].options).compact(), r#"{"dandori.v1.start":{"fails":["OutOfStock","DeliveryFailed"]},"idempotency_level":"NO_SIDE_EFFECTS"}"#);
    // A field written again is a list of its values; `(.x)` names the extension from the root.
    assert_eq!(tree(&s.methods[1].options).compact(), r#"{"dandori.v1.start":{"fails":["One","Two"]}}"#);
    let a = f.message("A").unwrap();
    // The rules written one at a time are laid over each other.
    assert_eq!(tree(&a.fields[0].options).compact(), r#"{"buf.validate.field":{"int32":{"gte":1,"lt":50}},"json_name":"x"}"#);
    // A predefined rule is no field the tree has; the rest are kept.
    assert_eq!(tree(&a.fields[1].options).compact(), r#"{"buf.validate.field":{"string":{"min_len":2}},"deprecated":true}"#);
}

#[test]
fn protovalidate_in_either_spelling() {
    let f = one(&std::fs::read_to_string("tests/fixtures/rulec/order.proto").unwrap());
    let field = |m: &str, n: &str| f.message(m).unwrap().fields.iter().find(|x| x.name == n).unwrap().rules.clone();
    let w = field("Order", "weight_g").int;
    assert_eq!((w.gte, w.lte), (Some(1), Some(40000)));
    let t = field("Order", "total_jpy").int;
    assert_eq!((t.gte, t.lte), (Some(0), Some(10_000_000)));
    let l = field("Order", "lines");
    assert_eq!((l.min_items, l.max_items), (Some(1), Some(50)));
    assert_eq!(field("Order", "zone").str_in, ["honshu", "hokkaido; okinawa"]);
    let tier = field("Order", "tier");
    assert!(tier.required);
    assert_eq!((tier.ignore.as_deref(), tier.cel.clone()), (Some("IGNORE_IF_ZERO_VALUE"), vec!["this > 0 && this != 7".to_string()]));
    let d = field("Order", "delta").int;
    assert_eq!((d.gte, d.lte, d.not_in), (Some(-5), Some(16), vec![3, 4]));
    assert_eq!(field("Order", "custom").unread, ["int64.(my.rule)"]);
    assert_eq!(field("Line", "amount").int.in_, [100, 200]);
}

#[test]
fn what_a_message_asks_and_a_message_that_validates_nothing() {
    let f = one(&std::fs::read_to_string("tests/fixtures/rulec/rules.proto").unwrap());
    let q = f.message("Quote").unwrap();
    assert_eq!(q.rules.cel, ["this.min_weight <= this.max_weight", "this.max_weight <= 30000"]);
    let groups: Vec<(Vec<String>, bool)> = q.rules.oneofs.iter().map(|o| (o.fields.clone(), o.required)).collect();
    assert_eq!(groups, [(vec!["coupon".to_string(), "points".to_string()], true), (vec!["card".to_string(), "bank".to_string()], true)]);
    assert_eq!(q.fields.iter().find(|x| x.name == "max_weight").unwrap().rules.cel, ["this >= 1", "this <= 50000"]);
    let old = f.message("Old").unwrap();
    assert!(old.rules.disabled);
    assert_eq!(old.fields[0].rules.unread, ["(buf.validate.message).disabled"]);
    assert_eq!(old.rules.oneofs.iter().map(|o| (o.fields.len(), o.required)).collect::<Vec<_>>(), [(2, false)]);
}

#[test]
fn the_deps_of_a_module_and_their_pins() {
    let read = |f: &str| std::fs::read_to_string(format!("tests/fixtures/rulec/buf/{f}")).unwrap();
    assert_eq!(buf::deps(&read("v2.buf.yaml")), ["buf.build/bufbuild/protovalidate", "buf.build/googleapis/googleapis"]);
    assert_eq!(buf::deps(&read("v1.buf.yaml")), ["buf.build/bufbuild/protovalidate", "buf.build/acme/x"]);
    assert!(buf::deps(&read("nodeps.buf.yaml")).is_empty());
    let (v, pins) = buf::lock(&read("v2.buf.lock"));
    assert_eq!((v.as_str(), pins.len()), ("v2", 2));
    assert_eq!(pins[0], buf::Pin { name: "buf.build/bufbuild/protovalidate".into(), commit: "511051f7f4374c3ca873b53ae68a9288".into(), digest: "b5:a4a2".into() });
    let (v, pins) = buf::lock(&read("v1.buf.lock"));
    assert_eq!((v.as_str(), pins[0].name.as_str(), pins[0].digest.as_str()), ("v1", "buf.build/bufbuild/protovalidate", "shake256:b911"));
}
