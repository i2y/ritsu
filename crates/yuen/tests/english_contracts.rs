//! The elements of OpenAPI and AsyncAPI documents and of Cedar's files as the ends of links
//! (DESIGN 3.6), on the English material `refund_contracts`: a shop's refunds, met by an operation
//! and a property of its OpenAPI document, an operation of its AsyncAPI document, a policy and an
//! action of its Cedar files. The twin of contracts.rs, whose material is the same in Japanese.
//!
//! Each end is the element and what its `$ref`s reach, so a change to an element stops the links
//! to it and to what reaches it, and a change to another element of the same file stops none.

mod common;

use ritsu_base::text::Lang;
use std::collections::BTreeMap;
use std::path::Path;

const DIR: &str = "tests/fixtures/refund_contracts";

/// The links of `refunds.req`, by their lines.
const REFUND_ORDER: usize = 10;
const ACTION: usize = 12;
const AMOUNT: usize = 21;
const POLICY: usize = 30;
const EVENT: usize = 39;

/// The end of every artifact the links name, by its reference: the hash and the text.
fn ends(dir: &str) -> BTreeMap<String, (String, String)> {
    let c = common::check(dir);
    assert!(c.diags.is_empty(), "{dir}: {:?}", c.diags.iter().map(|d| (d.code, d.message.en.clone())).collect::<Vec<_>>());
    let m = c.model.as_ref().unwrap();
    m.artifacts.iter().map(|(n, e)| (n.text(), e.as_ref().map(|e| (e.hash.clone(), String::from_utf8_lossy(&e.bytes).to_string())).unwrap())).collect()
}

/// The codes and lines of a check of a copy of the material, after `change`.
fn marks(change: impl Fn(&Path)) -> Vec<(&'static str, usize)> {
    let t = common::fixture("refund_contracts");
    let d = t.path().join("refund_contracts");
    change(&d);
    let c = common::check(&d.to_string_lossy());
    c.diags.iter().map(|x| (x.code, x.line.unwrap_or(0))).collect()
}

fn edit(rel: &'static str, from: &'static str, to: &'static str) -> impl Fn(&Path) {
    move |d: &Path| common::edit(d, rel, from, to)
}

#[test]
fn the_ends_of_the_elements_in_english() {
    let e = ends(DIR);
    let mut failures = Vec::new();
    for (naming, hash, golden) in [
        ("openapi \"api/orders.yaml\" operation refundOrder", "3ce78f035097e095", "openapi-refund-order.txt"),
        ("openapi \"api/orders.yaml\" schema Refund property amount", "55d566e37d48e6f7", "openapi-refund-amount.txt"),
        ("asyncapi \"events/orders.yaml\" operation sendOrderRefunded", "80f62fd6c2d2bb47", "asyncapi-send-order-refunded.txt"),
        ("cedar \"policies/refunds.cedar\" policy clerks_refund_within_their_limit", "0c7317f0e764cd08", "cedar-clerks-limit.txt"),
        ("cedar \"policies/shop.cedarschema\" action refund_order", "76a958d088eb5d8d", "cedar-refund-order.txt"),
    ] {
        let (h, text) = e.get(naming).unwrap_or_else(|| panic!("no end for {naming}: {:?}", e.keys().collect::<Vec<_>>()));
        assert_eq!(h, hash, "{naming}");
        common::golden(&format!("tests/golden/ends/{golden}"), text, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A schema an operation takes is in the operation's end, and so is a parameter of its path item;
/// another operation, a schema only another reaches, and a part of the document only it reaches,
/// stop nothing.
#[test]
fn an_element_of_a_document_stops_its_links_and_no_others_in_english() {
    assert_eq!(marks(edit("api/orders.yaml", "maximum: 10000", "maximum: 20000")), [("E303", REFUND_ORDER), ("E303", AMOUNT)]);
    assert_eq!(marks(edit("api/orders.yaml", "description: In pence", "description: In pence, VAT included")), [("E303", REFUND_ORDER), ("E303", AMOUNT)]);
    assert_eq!(marks(edit("api/orders.yaml", "required: [amount, reason]", "required: [reason]")), [("E303", REFUND_ORDER), ("E303", AMOUNT)], "a property no longer required");
    assert_eq!(marks(edit("api/orders.yaml", "enum: [damaged, late, unwanted]", "enum: [damaged, late, unwanted, lost]")), [("E303", REFUND_ORDER)], "the reason is not the amount");
    assert_eq!(marks(edit("api/orders.yaml", "      required: true\n      schema:\n        type: string", "      required: true\n      schema:\n        type: integer")), [("E303", REFUND_ORDER)], "the path's parameter");
    assert!(marks(edit("api/orders.yaml", "summary: Read an order", "summary: Read one order")).is_empty(), "another operation");
    assert!(marks(edit("api/orders.yaml", "enum: [placed, paid, shipped, refunded]", "enum: [placed, paid, shipped, refunded, lost]")).is_empty(), "a schema only another operation reaches");
    assert!(marks(edit("common/money.yaml", "enum: [GBP, USD, EUR]", "enum: [GBP, USD]")).is_empty(), "a part of the document only another operation reaches");
    assert!(marks(edit("api/orders.yaml", "openapi: 3.1.0\ninfo:", "openapi: 3.1.0\n# the shop's orders\ninfo:")).is_empty(), "a comment is no value");
    // the order of the keys is no value either
    assert!(marks(edit("api/orders.yaml", "          type: integer\n          minimum: 1\n          maximum: 10000", "          maximum: 10000\n          minimum: 1\n          type: integer")).is_empty());

    assert_eq!(marks(edit("events/orders.yaml", "        amount:\n          type: integer\n          minimum: 1", "        amount:\n          type: integer\n          minimum: 100")), [("E303", EVENT)], "the payload of the message the channel carries");
    assert!(marks(edit("events/orders.yaml", "address: orders.shipped", "address: orders.dispatched")).is_empty(), "another channel");
}

/// A policy's end is the policy without its comments, laid out as `cedar format` lays it out; an
/// action's, its declaration with the common types it uses.
#[test]
fn a_policy_and_an_action_stop_their_links_and_no_others_in_english() {
    assert_eq!(marks(edit("policies/refunds.cedar", "context.amount <= principal.refund_limit", "context.amount < principal.refund_limit")), [("E303", POLICY)]);
    assert!(marks(edit("policies/refunds.cedar", "permit (\n  principal in Shop::Role::\"clerk\",", "permit (\n  // a clerk, or a role that includes one\n  principal in Shop::Role::\"clerk\",")).is_empty(), "a comment");
    assert!(marks(edit("policies/refunds.cedar", "when { context.amount <= principal.refund_limit };", "when {\n  context.amount <= principal.refund_limit\n};")).is_empty(), "the layout");
    assert!(marks(edit("policies/refunds.cedar", "principal in Shop::Role::\"manager\"", "principal in Shop::Role::\"owner\"")).is_empty(), "another policy");
    assert_eq!(marks(edit("policies/shop.cedarschema", "  type RefundContext = {\n    amount: Long\n  };", "  type RefundContext = {\n    amount: Long,\n    reason: String\n  };")), [("E303", ACTION)], "the context the action takes");
    assert!(marks(edit("policies/shop.cedarschema", "    suspended: Bool\n", "    suspended: Bool,\n    team: String\n")).is_empty(), "an entity type the action names");
    assert!(marks(edit("policies/shop.cedarschema", "  action \"view_order\" appliesTo {", "  action \"view_order\" in [\"refund_order\"] appliesTo {")).is_empty(), "another action");
}

fn said(change: impl Fn(&Path)) -> Vec<String> {
    let t = common::fixture("refund_contracts");
    let d = t.path().join("refund_contracts");
    change(&d);
    let c = common::check(&d.to_string_lossy());
    c.diags.iter().map(|x| x.render(Lang::En)).collect()
}

/// What is not in the file is E202, with the things of its kind no link names; a file that does
/// not read as the tool's, E205; a document or a policy checks nothing, so `verified by` does not
/// take it (E403).
#[test]
fn what_is_not_there_and_what_does_not_read_in_english() {
    // written wrong, the one whose end is what was looked at is said to be the one renamed (DESIGN 4.5)
    let d = said(edit("refunds.req", "operation refundOrder\n", "operation refundOrders\n"));
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].starts_with("error[E202]: ") && d[0].contains("api/orders.yaml has no operation refundOrders"), "{}", d[0]);
    assert!(d[0].contains("It looks renamed") && d[0].contains("Did you mean: openapi \"api/orders.yaml\" operation refundOrder\n"), "{}", d[0]);
    let d = said(edit("refunds.req", "operation refundOrder\n", "operation \"POST /orders/{orderId}/refunds\"\n"));
    assert!(d[0].contains("[E202]") && d[0].contains("Did you mean: openapi \"api/orders.yaml\" operation refundOrder\n"), "an operation with an operationId is named by it: {}", d[0]);
    let d = said(edit("refunds.req", "schema Refund property amount", "schema Refund property total"));
    assert!(d[0].contains("[E202]") && d[0].contains("Did you mean: openapi \"api/orders.yaml\" schema Refund property amount\n"), "{}", d[0]);
    // renamed in the document, the end holds the name, so the candidates are the ones no link names
    let d = said(edit("api/orders.yaml", "operationId: refundOrder", "operationId: refundAnOrder"));
    assert!(d[0].contains("[E202]") && d[0].contains("Did you mean: openapi \"api/orders.yaml\" operation getOrder, openapi \"api/orders.yaml\" operation refundAnOrder"), "{}", d[0]);
    let d = said(edit("refunds.req", "satisfied by asyncapi \"events/orders.yaml\"", "satisfied by openapi \"events/orders.yaml\""));
    assert!(d[0].starts_with("error[E205]: ") && d[0].contains("events/orders.yaml does not read as an OpenAPI document: it is an AsyncAPI document, named with `asyncapi \"…\"`"), "{}", d[0]);
    let d = said(edit("api/orders.yaml", "  title: Orders\n", "  title: Orders\n\ttabbed: no\n"));
    assert!(d.iter().all(|x| x.starts_with("error[E205]: ")) && d.len() == 2, "{d:?}");
    let d = said(edit("refunds.req", "cedar \"policies/refunds.cedar\" policy", "cedar \"refunds.req\" policy"));
    assert!(d[0].contains("[E205]") && d[0].contains("yuen reads policies (.cedar) and schemas (.cedarschema, .cedarschema.json)"), "{}", d[0]);
    let d = said(edit("refunds.req", "  not verified \"This material has no claim that asks Cedar\"\n    approved 2026-10-06 by payments sha256:ac26306142b38a02\n", "  verified by cedar \"policies/refunds.cedar\" policy managers_refund_any_amount\n"));
    assert!(d[0].starts_with("error[E403]: ") && d[0].contains("cedar \"policies/refunds.cedar\" policy managers_refund_any_amount checks nothing"), "{}", d[0]);
    // an action declared in two namespaces of one schema is not one thing
    let d = said(|d: &Path| {
        let s = std::fs::read_to_string(d.join("policies/shop.cedarschema")).unwrap();
        std::fs::write(d.join("policies/shop.cedarschema"), format!("{s}namespace Warehouse {{\n  action \"refund_order\";\n}}\n")).unwrap();
    });
    assert!(d[0].contains("[E202]") && d[0].contains("declares action refund_order in more than one namespace (Shop, Warehouse)"), "{}", d[0]);
}

/// A scope of a tool's directory takes the documents of the tool (by what their top says) or the
/// Cedar files, and with a kind, the elements of that kind: what no requirement leads to is E404.
#[test]
fn a_scope_of_documents_and_policies_in_english() {
    let t = common::fixture("refund_contracts");
    let d = t.path().join("refund_contracts");
    common::edit(&d, "refunds.req", "role payments \"Decides how refunds go\"\n", "role payments \"Decides how refunds go\"\n\nscope openapi \".\" operation\nscope asyncapi \"events\" operation\nscope cedar \"policies\" policy\nscope openapi \"common\"\n");
    let c = common::check(&d.to_string_lossy());
    let untraced: Vec<String> = c.diags.iter().filter(|x| x.code == "E404").map(|x| x.message.en.clone()).collect();
    assert_eq!(
        untraced,
        [
            "openapi \"api/orders.yaml\" operation getOrder is in scope, and no requirement leads to it",
            "asyncapi \"events/orders.yaml\" operation sendOrderShipped is in scope, and no requirement leads to it",
            "cedar \"policies/refunds.cedar\" policy managers_refund_any_amount is in scope, and no requirement leads to it",
            "cedar \"policies/refunds.cedar\" policy suspended_staff_do_nothing is in scope, and no requirement leads to it",
        ],
        "{:?}",
        c.diags.iter().map(|x| x.render(Lang::En)).collect::<Vec<_>>()
    );
}
