//! The mappings of an anticorruption layer, held to the upstream's enums (PLAN B.8).

mod common;

use common::{check_dir, codes, variant};

#[test]
fn a_value_the_upstream_adds_is_named() {
    let dir = common::mutant("E401_注文の状態に値が増えた");
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E401"]);
    let d = &os[0].diags[0];
    assert!(d.message.en.contains("ORDER_STATUS_RETURNED"), "{}", d.message.en);
    for other in ["ORDER_STATUS_RECEIVED", "ORDER_STATUS_PAID", "ORDER_STATUS_SHIPPED", "ORDER_STATUS_CANCELLED", "ORDER_STATUS_UNSPECIFIED"] {
        assert!(!d.message.en.contains(other), "{other} is mapped");
    }
    assert_eq!(d.fix.as_deref(), Some("ORDER_STATUS_RETURNED -> refuse \"…\""));
}

#[test]
fn the_value_zero_that_says_nothing_is_set_needs_no_mapping_and_a_real_zero_does() {
    let dir = common::mutant("W402_値が無いことを表す値");
    assert_eq!(codes(&check_dir(dir.path())), ["W402"]);
    let dir = variant("基本", &[("proto/shop/ordering/v1/order.proto", "ORDER_STATUS_UNSPECIFIED = 0;", "ORDER_STATUS_NEW = 0;")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E401"]);
    assert!(os[0].diags[0].message.en.contains("ORDER_STATUS_NEW"));
}

#[test]
fn every_value_missing_is_named_in_the_order_of_the_proto() {
    let dir = variant("基本", &[("ctx/請求.ctx", "    ORDER_STATUS_RECEIVED  -> BILLING_STATUS_WAIT\n    ORDER_STATUS_PAID      -> BILLING_STATUS_BILL\n", "")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E401"]);
    assert!(os[0].diags[0].message.en.contains("ORDER_STATUS_RECEIVED, ORDER_STATUS_PAID of"), "{}", os[0].diags[0].message.en);
}

#[test]
fn a_target_that_is_a_name_only_is_taken_as_written() {
    let dir = variant("基本", &[("ctx/請求.ctx", "enum OrderStatus -> enum BillingStatus", "enum OrderStatus -> 請求の扱い")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), Vec::<&str>::new());
    let api = sakai::api::api(os[0].checked.as_ref().unwrap());
    let e = &api["relationships"][2]["enums"][0];
    assert_eq!(e["to"], serde_json::json!({"name": "請求の扱い"}));
    assert_eq!(e["checked"], false);
}

#[test]
fn a_rule_as_the_target_is_not_read_until_stage_c() {
    // A rule's values come from `rulec api` (stage C): until then sakai says nothing of them.
    let dir = variant(
        "基本",
        &[
            ("proto/billing/acl/rules/請求の要否.rule", "", "rule 請求の要否(billing_need) v1\n"),
            ("ctx/請求.ctx", "enum OrderStatus -> enum BillingStatus\n    ORDER_STATUS_RECEIVED  -> BILLING_STATUS_WAIT\n", "enum OrderStatus -> rulec \"../proto/billing/acl/rules/請求の要否.rule\" enum 注文の状態\n"),
        ],
    );
    assert_eq!(codes(&check_dir(dir.path())), Vec::<&str>::new());
}
