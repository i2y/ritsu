//! The operations an action guards and the values it gives a rule, beyond the mutants (PLAN A3):
//! an OpenAPI document's types, ranges, enums and parameters that may be absent (E203), the `from`
//! of an action (E204), a `.proto`'s methods and Protovalidate's ranges, an AsyncAPI channel's
//! parameters, the operations of a chobo book's transfers (E202), and a rule's precondition the
//! values given can break (E206). Each case is a small gate, checked with every language joined.

mod common;

use ritsu_base::text::Lang;
use sekisho::check::{Options, check_file};

/// A gate with what has to be beside it, checked as `ritsu sekisho check` checks it: the codes it
/// gives, and what it prints.
fn codes(files: &[(&str, &str)]) -> (Vec<&'static str>, String) {
    let dir = ritsu_testkit::TempDir::new("contracts");
    for (name, body) in files {
        std::fs::write(dir.path().join(name), body).unwrap();
    }
    let path = dir.path().join("example.gate");
    let o = check_file(path.to_str().unwrap(), &common::joined(), &Options::default()).unwrap();
    // in English, then in Japanese, the directory left out (it differs from run to run)
    let shown = format!("{}/", dir.path().display());
    let text: String = [Lang::En, Lang::Ja].iter().flat_map(|l| o.diags.iter().map(move |d| d.render(*l))).collect();
    (o.diags.iter().map(|d| d.code).collect(), text.replace(&shown, ""))
}

const ORDERS: &str = r#"{
  "openapi": "3.1.0",
  "info": { "title": "Orders", "version": "1" },
  "paths": {
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [
          { "name": "orderId", "in": "path", "required": true, "schema": { "type": "string" } },
          { "name": "reason", "in": "query", "schema": { "type": "string", "enum": ["damaged", "late"] } }
        ],
        "requestBody": { "required": true, "content": { "application/json": { "schema": {
          "type": "object", "required": ["amount", "share"],
          "properties": {
            "amount": { "type": "integer", "minimum": 1, "maximum": 10000 },
            "share": { "type": "number" },
            "urgent": { "type": "boolean" }
          } } } } },
        "responses": { "201": { "description": "Refunded" } }
      }
    }
  }
}
"#;

/// A gate that guards refundOrder, with the lines under `input` given.
fn refund_gate(input: &str) -> String {
    format!(
        "gate shop v1\n\nuse openapi orders from \"orders.json\"\n\nenum reason = damaged | late\n\nprincipal User\n\nresource Order\n\naction refund_order\n  guards orders refundOrder\n  principal User\n  resource Order from orderId\n  input\n{input}\npermit users_refund\n  principal is User\n  action refund_order\n"
    )
}

#[test]
fn an_input_is_what_the_openapi_operation_takes() {
    // what the operation takes, a value it does not require written with `?`
    let ok = refund_gate("    amount : money[GBP]  range >=1GBP <=10_000GBP\n    reason : reason?\n    urgent : bool?\n");
    let (c, text) = codes(&[("example.gate", &ok), ("orders.json", ORDERS)]);
    assert!(c.is_empty(), "{text}");
    // a range past the operation's
    let (c, text) = codes(&[("example.gate", &refund_gate("    amount : money[GBP]  range >=1GBP <=20_000GBP\n")), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E203"], "{text}");
    assert!(text.contains("its range is 1GBP..20,000GBP, and the operation takes 1GBP..10,000GBP"), "{text}");
    // a number the operation takes with a fraction
    let (c, text) = codes(&[("example.gate", &refund_gate("    share : number  range >=0 <=10\n")), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E203"], "{text}");
    // a value the operation does not require, without `?`
    let (c, text) = codes(&[("example.gate", &refund_gate("    reason : reason\n")), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E203"], "{text}");
    assert!(text.contains("write `?` after its type"), "{text}");
    // an enum with fewer values than the operation takes
    let fewer = refund_gate("    reason : only_damaged?\n").replace("enum reason = damaged | late\n", "enum reason = damaged | late\nenum only_damaged = damaged\n");
    let (c, text) = codes(&[("example.gate", &fewer), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E203"], "{text}");
    assert!(text.contains("late"), "{text}");
    // `from` an argument the operation does not have
    let (c, text) = codes(&[("example.gate", &refund_gate("    amount : money[GBP]  range >=1GBP <=10_000GBP\n").replace("from orderId", "from order")), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E204"], "{text}");
}

const PROTO: &str = r#"syntax = "proto3";
package shop.v1;

import "buf/validate/validate.proto";

service Orders {
  rpc Refund(RefundRequest) returns (RefundReply);
}

message RefundRequest {
  string order_id = 1;
  int64 amount = 2 [(buf.validate.field).int64 = {gte: 1, lte: 10000}];
  bool urgent = 3;
}

message RefundReply {
  string id = 1;
}
"#;

fn proto_gate(guard: &str, from: &str, input: &str) -> String {
    format!(
        "gate shop v1\n\nuse proto shop from \"orders.proto\"\n\nprincipal User\n\nresource Order\n\naction refund_order\n  guards shop {guard}\n  principal User\n  resource Order from {from}\n  input\n{input}\npermit users_refund\n  principal is User\n  action refund_order\n"
    )
}

#[test]
fn a_proto_method_its_fields_and_their_ranges() {
    let ok = proto_gate("\"Orders/Refund\"", "orderId", "    amount : number  range >=1 <=10_000\n    urgent : bool\n");
    let (c, text) = codes(&[("example.gate", &ok), ("orders.proto", PROTO)]);
    assert!(c.is_empty(), "{text}");
    let (c, text) = codes(&[("example.gate", &proto_gate("\"Orders/Refnd\"", "orderId", "    urgent : bool\n")), ("orders.proto", PROTO)]);
    assert_eq!(c, vec!["E202"], "{text}");
    assert!(text.contains("Orders/Refund"), "{text}");
    let (c, text) = codes(&[("example.gate", &proto_gate("\"Orders/Refund\"", "id", "    urgent : bool\n")), ("orders.proto", PROTO)]);
    assert_eq!(c, vec!["E204"], "{text}");
    // Protovalidate's range is what the method takes
    let (c, text) = codes(&[("example.gate", &proto_gate("\"Orders/Refund\"", "order_id", "    amount : number  range >=0 <=10_000\n")), ("orders.proto", PROTO)]);
    assert_eq!(c, vec!["E203"], "{text}");
}

const EVENTS: &str = r##"asyncapi: 3.0.0
info:
  title: Events
  version: "1"
channels:
  refunds:
    address: "orders/{orderId}/refunds"
    parameters:
      orderId: {}
    messages:
      requested:
        payload:
          type: object
          required: [amount]
          properties:
            amount:
              type: integer
              minimum: 1
              maximum: 10000
operations:
  refundRequested:
    action: receive
    channel:
      $ref: "#/channels/refunds"
    messages:
      - $ref: "#/channels/refunds/messages/requested"
"##;

#[test]
fn an_asyncapi_operation_and_its_channel() {
    let gate = |guard: &str, from: &str| {
        format!(
            "gate shop v1\n\nuse asyncapi events from \"events.yaml\"\n\nprincipal User\n\nresource Order\n\naction refund_order\n  guards events {guard}\n  principal User\n  resource Order from {from}\n  input\n    amount : number  range >=1 <=10_000\n\npermit users_refund\n  principal is User\n  action refund_order\n"
        )
    };
    let (c, text) = codes(&[("example.gate", &gate("refundRequested", "orderId")), ("events.yaml", EVENTS)]);
    assert!(c.is_empty(), "{text}");
    let (c, text) = codes(&[("example.gate", &gate("refundAsked", "orderId")), ("events.yaml", EVENTS)]);
    assert_eq!(c, vec!["E202"], "{text}");
    let (c, text) = codes(&[("example.gate", &gate("refundRequested", "order")), ("events.yaml", EVENTS)]);
    assert_eq!(c, vec!["E204"], "{text}");
}

#[test]
fn an_operation_of_a_transfer_of_a_book() {
    let book = std::fs::canonicalize("../chobo/examples/inventory/inventory.book").unwrap();
    let gate = |guard: &str| {
        format!(
            "gate stock v1\n\nuse book inventory from \"{}\"\n\nprincipal User\n\nresource Delivery\n\naction receive_delivery\n  guards inventory {guard}\n  principal User\n  resource Delivery\n\npermit users_receive\n  principal is User\n  action receive_delivery\n",
            book.display()
        )
    };
    let (c, text) = codes(&[("example.gate", &gate("receive.do"))]);
    assert!(c.is_empty(), "{text}");
    let (c, text) = codes(&[("example.gate", &gate("receive.hold"))]);
    assert_eq!(c, vec!["E202"], "{text}");
    assert!(text.contains("receive.do"), "{text}");
}

const ORDERED: &str = "rule ordered v1\ndescription \"Whether a span is wide; it asks that low is not above high\"\n\nenum width = narrow | wide\n\ninputs\n  low  : number  range >=0 <=100\n  high : number  range >=0 <=100\n\nconstraint low <= high\n\noutputs\n  span : width\n\nderive gap : number = high - low  range >=-100 <=100\n\ntable decide\npolicy unique\n| gap  | -> span : width |\n| <=10 | narrow          |\n| >10  | wide            |\n";

#[test]
fn values_that_can_break_a_precondition_of_the_rule() {
    let gate = |low: &str, high: &str| {
        format!(
            "gate spans v1\n\nuse rule ordered from \"ordered.rule\"\n\nprincipal User\n  attributes\n    low  : number  range {low}\n    high : number  range {high}\n\nresource Doc\n\naction read_doc\n  principal User\n  resource Doc\n  context\n    span = ordered(low: principal.low, high: principal.high).span\n\npermit wide_reads\n  principal is User\n  action read_doc\n  when span is wide\n"
        )
    };
    // low can be above high
    let (c, text) = codes(&[("example.gate", &gate(">=0 <=100", ">=0 <=100")), ("ordered.rule", ORDERED)]);
    assert_eq!(c, vec!["E206"], "{text}");
    ritsu_testkit::golden("tests/walk/golden/E206_precondition.txt", &text);
    // low is never above high
    let (c, text) = codes(&[("example.gate", &gate(">=0 <=50", ">=50 <=100")), ("ordered.rule", ORDERED)]);
    assert!(c.is_empty(), "{text}");
}

const PERIOD: &str = "rule order_period v1\ndescription \"The period in force from the order date\"\n\nenum period = before | spring | normal | year_end\n\ninputs\n  order_date : date  range >=2026-01-01 <=2026-12-31\n\noutputs\n  kind : period\n\ntable pick\npolicy unique\n| order_date                | -> kind : period |\n| <=2026-03-31              | before           |\n| >=2026-04-01 <=2026-06-30 | spring           |\n| >=2026-07-01 <=2026-11-30 | normal           |\n| >=2026-12-01              | year_end         |\n";

#[test]
fn the_range_of_a_date_given_to_a_rule_is_what_rulec_is_asked_over() {
    // ordered in May to August: the period is spring or normal, never the year's end
    let gate = |when: &str| {
        format!(
            "gate orders v1\n\nuse rule order_period from \"period.rule\"\n\nprincipal User\n\nresource Order\n  attributes\n    ordered_on : date  range >=2026-05-01 <=2026-08-31\n\naction view_order\n  principal User\n  resource Order\n  context\n    kind = order_period(order_date: resource.ordered_on).kind\n\npermit users_view\n  principal is User\n  action view_order\n  when kind is {when}\n"
        )
    };
    let (c, text) = codes(&[("example.gate", &gate("spring")), ("period.rule", PERIOD)]);
    assert!(c.is_empty(), "{text}");
    // the permit for the year's end never applies, and nothing else allows: rulec was asked over May
    // to August, two values of the period
    let (c, text) = codes(&[("example.gate", &gate("year_end")), ("period.rule", PERIOD)]);
    assert_eq!(c, vec!["E301", "E303"], "{text}");
    assert!(text.contains("It comes to 2 combinations."), "{text}");
}

const STAY: &str = "rule stay v1\ndescription \"Whether a stay starts in spring; it asks that the arrival is not after the leaving\"\n\nenum season = spring | other\n\ninputs\n  arrive : date  range >=2026-01-01 <=2026-12-31\n  leave  : date  range >=2026-01-01 <=2026-12-31\n\nconstraint arrive <= leave\n\noutputs\n  start : season\n\ntable pick\npolicy unique\n| arrive                    | -> start : season |\n| >=2026-04-01 <=2026-06-30 | spring            |\n| <=2026-03-31              | other             |\n| >=2026-07-01              | other             |\n";

#[test]
fn days_that_can_break_a_precondition_of_the_rule() {
    let gate = |arrive: &str, leave: &str| {
        format!(
            "gate stays v1\n\nuse rule stay from \"stay.rule\"\n\nprincipal User\n\nresource Booking\n  attributes\n    arrive_on : date  range {arrive}\n    leave_on  : date  range {leave}\n\naction view_booking\n  principal User\n  resource Booking\n  context\n    start = stay(arrive: resource.arrive_on, leave: resource.leave_on).start\n\npermit spring_view\n  principal is User\n  action view_booking\n  when start is spring\n"
        )
    };
    let (c, text) = codes(&[("example.gate", &gate(">=2026-03-01 <=2026-05-31", ">=2026-03-15 <=2026-06-30")), ("stay.rule", STAY)]);
    assert_eq!(c, vec!["E206"], "{text}");
    // the arrival as late as it gets against the leaving as early: days, not day numbers
    assert!(text.contains("arrive = 2026-05-31, leave = 2026-03-15"), "{text}");
    let (c, text) = codes(&[("example.gate", &gate(">=2026-03-01 <=2026-04-30", ">=2026-05-01 <=2026-06-30")), ("stay.rule", STAY)]);
    assert!(c.is_empty(), "{text}");
}
