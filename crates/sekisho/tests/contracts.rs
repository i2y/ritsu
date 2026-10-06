//! The operations an action guards and the values it gives a rule, beyond the mutants (PLAN A3):
//! an OpenAPI document's types, ranges, enums and parameters that may be absent (E203), the `from`
//! of an action (E204), a `.proto`'s methods and Protovalidate's ranges, an AsyncAPI channel's
//! parameters, the operations of a chobo book's transfers (E202), and a rule's precondition the
//! values given can break (E206). Each case is a small gate, checked with every language joined.

mod common;

use ritsu_base::text::Lang;
use sekisho::check::{Options, check_file};

/// A gate with what has to be beside it, checked as `ritsu sekisho check` checks it, with the
/// directory they are in as the root: the codes it gives, and what it prints.
fn codes(files: &[(&str, &str)]) -> (Vec<&'static str>, String) {
    let dir = ritsu_testkit::TempDir::new("contracts");
    for (name, body) in files {
        std::fs::write(dir.path().join(name), body).unwrap();
    }
    let path = dir.path().join("example.gate");
    let o = check_file(path.to_str().unwrap(), &common::joined(), &Options { root: Some(dir.path().to_path_buf()), ..Options::default() }).unwrap();
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
    // each is said of the operation's reference, its path from the root
    assert!(text.contains("error[E204]: example.gate:14:1: `order` is not a parameter of the path or the query of openapi \"orders.json\" operation refundOrder\n"), "{text}");
    assert!(text.contains("エラー[E204]: example.gate:14:1: `order` は openapi \"orders.json\" operation refundOrder のパスかクエリの引数にありません\n"), "{text}");
}

/// The operations an action guards are said by their references (ritsu's DESIGN 6.2), the path of
/// the contract from the root: what is not in it by the pairs written (as yuen's E202 says it), the
/// rest by the reference of the operation found — by its `operationId`, however the `guards` line
/// writes it.
#[test]
fn the_operations_are_said_by_their_references() {
    let (c, text) = codes(&[("example.gate", &refund_gate("    amount : money[GBP]  range >=1GBP <=10_000GBP\n").replace("guards orders refundOrder", "guards orders refundOrders")), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E202"], "{text}");
    assert!(text.contains("error[E202]: example.gate:12:1: There is no operation refundOrders in orders.json\n"), "{text}");
    assert!(text.contains("エラー[E202]: example.gate:12:1: orders.json に operation refundOrders はありません\n"), "{text}");
    // an input it does not take, the operation written by its method and path
    let by_path = refund_gate("    note : bool\n").replace("guards orders refundOrder", "guards orders \"POST /orders/{orderId}/refunds\"");
    let (c, text) = codes(&[("example.gate", &by_path), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E203"], "{text}");
    assert!(text.contains("The input `note` is neither a parameter nor a field of the body of openapi \"orders.json\" operation refundOrder\n"), "{text}");
    assert!(text.contains("input `note` は openapi \"orders.json\" operation refundOrder の引数にも本文のフィールドにもありません\n"), "{text}");
    // two actions that guard it, one by its operationId, the other by its method and path
    let two = refund_gate("    amount : money[GBP]  range >=1GBP <=10_000GBP\n").replace(
        "permit users_refund",
        "action refund_again\n  guards orders \"POST /orders/{orderId}/refunds\"\n  principal User\n  resource Order\n\npermit users_refund_again\n  principal is User\n  action refund_again\n\npermit users_refund",
    );
    let (c, text) = codes(&[("example.gate", &two), ("orders.json", ORDERS)]);
    assert_eq!(c, vec!["E205"], "{text}");
    assert!(text.contains(": Two actions, `refund_order` and `refund_again`, guard openapi \"orders.json\" operation refundOrder\n"), "{text}");
    assert!(text.contains(": openapi \"orders.json\" operation refundOrder を、`refund_order` と `refund_again` の二つの action が守ります\n"), "{text}");
}

/// A contract outside the root has no reference to name its operations by (E201): the root is
/// `--root`, else the nearest directory above that holds `.git`, else the directory of the file.
#[test]
fn a_contract_outside_the_root() {
    let dir = ritsu_testkit::TempDir::new("contracts");
    std::fs::create_dir(dir.path().join("gates")).unwrap();
    std::fs::write(dir.path().join("orders.json"), ORDERS).unwrap();
    let gate = refund_gate("    amount : money[GBP]  range >=1GBP <=10_000GBP\n").replace("\"orders.json\"", "\"../orders.json\"");
    let path = dir.path().join("gates/example.gate");
    std::fs::write(&path, gate).unwrap();
    let check = |root: &std::path::Path| check_file(path.to_str().unwrap(), &common::joined(), &Options { root: Some(root.to_path_buf()), ..Options::default() }).unwrap();
    let o = check(&dir.path().join("gates"));
    assert_eq!(o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E201"]);
    assert_eq!(o.diags[0].line, Some(3));
    let said: String = [Lang::En, Lang::Ja].iter().map(|l| o.diags[0].render(*l)).collect::<String>().replace(&format!("{}/", dir.path().display()), "");
    assert!(said.starts_with("error[E201]: gates/example.gate:3:1: `../orders.json` is outside the root\n"), "{said}");
    assert!(said.contains("--root changes it") && said.contains("エラー[E201]: gates/example.gate:3:1: `../orders.json` はルートの外にあります\n") && said.contains("--root で替えられます"), "{said}");
    // the directory above as the root: the operation from there
    let o = check(dir.path());
    assert!(o.diags.is_empty(), "{:?}", o.diags.iter().map(|d| d.render(Lang::En)).collect::<Vec<_>>());
    let refs = o.walked.as_ref().unwrap().gate.actions[0].references();
    assert_eq!(refs.iter().map(|(n, at)| (n.text(), *at)).collect::<Vec<_>>(), vec![("openapi \"orders.json\" operation refundOrder".to_string(), 12)]);
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
    assert!(text.contains("There is no service Orders method Refnd in orders.proto\n") && text.contains("Its methods are Orders/Refund."), "{text}");
    // a method is named by its service's name in the file's package, however the line writes it
    let (c, text) = codes(&[("example.gate", &proto_gate("\"shop.v1.Orders/Refund\"", "orderId", "    flag : bool\n")), ("orders.proto", PROTO)]);
    assert_eq!(c, vec!["E203"], "{text}");
    assert!(text.contains("The input `flag` is not a field of the request of proto \"orders.proto\" service Orders method Refund\n"), "{text}");
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
    assert!(text.contains("There is no operation refundAsked in events.yaml\n"), "{text}");
    let (c, text) = codes(&[("example.gate", &gate("refundRequested", "order")), ("events.yaml", EVENTS)]);
    assert_eq!(c, vec!["E204"], "{text}");
    assert!(text.contains("of asyncapi \"events.yaml\" operation refundRequested\n"), "{text}");
}

#[test]
fn an_operation_of_a_transfer_of_a_book() {
    let book = std::fs::read_to_string("../chobo/examples/inventory/inventory.book").unwrap();
    let gate = |guard: &str| {
        format!(
            "gate stock v1\n\nuse book inventory from \"inventory.book\"\n\nprincipal User\n\nresource Delivery\n\naction receive_delivery\n  guards inventory {guard}\n  principal User\n  resource Delivery\n\npermit users_receive\n  principal is User\n  action receive_delivery\n"
        )
    };
    let (c, text) = codes(&[("example.gate", &gate("receive.do")), ("inventory.book", &book)]);
    assert!(c.is_empty(), "{text}");
    let (c, text) = codes(&[("example.gate", &gate("receive.hold")), ("inventory.book", &book)]);
    assert_eq!(c, vec!["E202"], "{text}");
    assert!(text.contains("There is no transfer receive operation hold in inventory.book\n") && text.contains("receive.do"), "{text}");
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
