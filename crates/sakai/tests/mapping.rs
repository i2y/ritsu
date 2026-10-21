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
    assert_eq!(d.fixed_line(), Some("ORDER_STATUS_RETURNED -> refuse \"…\""));
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

/// A rule's enum as the target is read from what rulec says of the rule (DESIGN 1.7): a rule that
/// does not take the upstream enum in needs the value lines, each to a value of the rule's enum
/// (E403); one that takes it in with `import proto` is the mapping, and lines beside it agree with
/// it (E405, in the example's tests).
#[test]
fn a_rule_as_the_target_is_read_from_rulec() {
    let rule = "rule 請求の要否(billing_need) v1\n\nenum 注文の状態(order_status) = 受付(received) | 支払済(paid)\nenum 扱い(handling) = 待つ(wait) | 請求する(bill)\n\ninputs\n  状態(status) : 注文の状態\n\noutputs\n  扱い(handling) : 扱い\n\ntable t\npolicy unique\n| 状態   | -> 扱い  |\n| 受付   | 待つ     |\n| 支払済 | 請求する |\n";
    let dir = variant(
        "基本",
        &[
            ("proto/billing/acl/rules/請求の要否.rule", "", rule),
            ("ctx/請求.ctx", "enum OrderStatus -> enum BillingStatus\n    ORDER_STATUS_RECEIVED  -> BILLING_STATUS_WAIT\n", "enum OrderStatus -> rulec \"../proto/billing/acl/rules/請求の要否.rule\" enum 注文の状態\n    ORDER_STATUS_RECEIVED  -> 受取\n"),
        ],
    );
    let os = check_dir(dir.path());
    assert!(codes(&os).contains(&"E403"), "{:?}", codes(&os));
    let d = os.iter().flat_map(|o| o.diags.iter()).find(|d| d.code == "E403").unwrap();
    assert!(d.message.en.contains("The mapping maps to 受取, which is not a value of the enum 注文の状態"), "{}", d.message.en);
}

// ── The English twins ──

#[test]
fn a_value_the_upstream_adds_is_named_in_english() {
    let dir = common::mutant("E401_value_added_to_the_order_status");
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E401"]);
    let d = &os[0].diags[0];
    assert!(d.message.en.contains("ORDER_STATUS_RETURNED"), "{}", d.message.en);
    for other in ["ORDER_STATUS_RECEIVED", "ORDER_STATUS_PAID", "ORDER_STATUS_SHIPPED", "ORDER_STATUS_CANCELLED", "ORDER_STATUS_UNSPECIFIED"] {
        assert!(!d.message.en.contains(other), "{other} is mapped");
    }
    assert_eq!(d.fixed_line(), Some("ORDER_STATUS_RETURNED -> refuse \"…\""));
}

#[test]
fn the_value_zero_that_says_nothing_is_set_needs_no_mapping_and_a_real_zero_does_in_english() {
    let dir = common::mutant("W402_value_that_says_nothing_is_set");
    assert_eq!(codes(&check_dir(dir.path())), ["W402"]);
    let dir = variant("basic", &[("proto/shop/ordering/v1/order.proto", "ORDER_STATUS_UNSPECIFIED = 0;", "ORDER_STATUS_NEW = 0;")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E401"]);
    assert!(os[0].diags[0].message.en.contains("ORDER_STATUS_NEW"));
}

#[test]
fn every_value_missing_is_named_in_the_order_of_the_proto_in_english() {
    let dir = variant("basic", &[("ctx/billing.ctx", "    ORDER_STATUS_RECEIVED  -> BILLING_STATUS_WAIT\n    ORDER_STATUS_PAID      -> BILLING_STATUS_BILL\n", "")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E401"]);
    assert!(os[0].diags[0].message.en.contains("ORDER_STATUS_RECEIVED, ORDER_STATUS_PAID of"), "{}", os[0].diags[0].message.en);
}

#[test]
fn a_target_that_is_a_name_only_is_taken_as_written_in_english() {
    let dir = variant("basic", &[("ctx/billing.ctx", "enum OrderStatus -> enum BillingStatus", "enum OrderStatus -> billing_handling")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), Vec::<&str>::new());
    let api = sakai::api::api(os[0].checked.as_ref().unwrap());
    let e = &api["relationships"][2]["enums"][0];
    assert_eq!(e["to"], serde_json::json!({"name": "billing_handling"}));
    assert_eq!(e["checked"], false);
}

#[test]
fn a_rule_as_the_target_is_read_from_rulec_in_english() {
    let rule = "rule billing_need v1\n\nenum order_status = received | paid\nenum handling = wait | bill\n\ninputs\n  status : order_status\n\noutputs\n  handling : handling\n\ntable t\npolicy unique\n| status   | -> handling |\n| received | wait        |\n| paid     | bill        |\n";
    let dir = variant(
        "basic",
        &[
            ("proto/billing/acl/rules/billing_need.rule", "", rule),
            ("ctx/billing.ctx", "enum OrderStatus -> enum BillingStatus\n    ORDER_STATUS_RECEIVED  -> BILLING_STATUS_WAIT\n", "enum OrderStatus -> rulec \"../proto/billing/acl/rules/billing_need.rule\" enum order_status\n    ORDER_STATUS_RECEIVED  -> taken\n"),
        ],
    );
    let os = check_dir(dir.path());
    assert!(codes(&os).contains(&"E403"), "{:?}", codes(&os));
    let d = os.iter().flat_map(|o| o.diags.iter()).find(|d| d.code == "E403").unwrap();
    assert!(d.message.en.contains("The mapping maps to taken, which is not a value of the enum order_status"), "{}", d.message.en);
}

/// An upstream value, and whether the line of a mapping writes it bare.
type Spelled = (&'static str, bool);

/// The values added to the enum `ChargeStatus` of the upstream's OpenAPI document. A `,`, a `->`, a
/// blank and a `#` end a word of a `.ctx`, and a value with one of them is written in quotes; a
/// `-`, a `.`, an apostrophe and digits first do not.
const VALUES: &[Spelled] = &[
    ("disputed", true),
    ("in-transit", true),
    ("Re.quote", true),
    ("O'Brien", true),
    ("2FA", true),
    ("hold,manual", false),
    ("a->b", false),
    ("on hold", false),
    ("#1", false),
];

/// The same shapes in Japanese, and a full-width space; a full-width `＃` is a character of a word.
const VALUES_JA: &[Spelled] = &[
    ("係争中", true),
    ("再-審査", true),
    ("2段階", true),
    ("＃1", true),
    ("保留,手動", false),
    ("確認->出荷", false),
    ("保留 中", false),
    ("保留\u{3000}中", false),
];

/// The fix of E401 is a line the mapping takes (DESIGN 21): with the value written into the
/// example's mapping as the fix gives it, the check reads the line and maps the value.
fn the_fix_of_a_value_left_out(example: &str, ctx: &str, last: &str, values: &[Spelled]) {
    for &(v, bare) in values {
        let dir = common::TempDir::new("fix-of-e401");
        common::copy_dir(std::path::Path::new(example), dir.path());
        let yaml = dir.path().join("payments/api/payments.yaml");
        let s = std::fs::read_to_string(&yaml).unwrap();
        let list = "enum: [pending, succeeded, failed, refunded]";
        assert!(s.contains(list));
        std::fs::write(&yaml, s.replacen(list, &format!("enum: [pending, succeeded, failed, refunded, {}]", serde_json::to_string(v).unwrap()), 1)).unwrap();
        let os = check_dir(dir.path());
        assert_eq!(codes(&os), ["E401"], "{v}");
        let fix = os.iter().flat_map(|o| o.diags.iter()).find(|d| d.code == "E401").and_then(|d| d.fixed_line()).unwrap().to_string();
        let written = if bare { v.to_string() } else { sakai::naming::quote(v) };
        assert_eq!(fix, format!("{written} -> refuse \"…\""), "{v}");
        let file = dir.path().join("contexts").join(ctx);
        let s = std::fs::read_to_string(&file).unwrap();
        assert!(s.contains(last), "{ctx} has no {last:?}");
        std::fs::write(&file, s.replacen(last, &format!("{last}    {fix}\n"), 1)).unwrap();
        let os = check_dir(dir.path());
        assert_eq!(codes(&os), Vec::<&str>::new(), "{v}: the line of the fix is read, and maps the value");
    }
}

#[test]
fn the_fix_of_a_value_left_out_is_a_line_the_check_reads() {
    the_fix_of_a_value_left_out("examples/webshop", "shipping.ctx", "    refunded  -> refuse \"A refunded order is not shipped\"\n", VALUES);
}

#[test]
fn the_fix_of_a_value_left_out_is_a_line_the_check_reads_in_japanese() {
    the_fix_of_a_value_left_out("examples/webshop.ja", "配送.ctx", "    refunded  -> refuse \"返金した注文は出荷しない\"\n", VALUES_JA);
}
