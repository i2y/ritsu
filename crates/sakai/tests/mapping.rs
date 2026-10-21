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

// ── The values a diagnostic shows (DESIGN 22) ──

/// A value added to an enum of the example's documents, and whether a diagnostic shows it bare.
type Shown = (&'static str, bool);

/// Values added to the example's enums. A diagnostic shows each as a value line writes it: in
/// quotes when it holds a blank, a `,`, a `->`, a `#` or a `"`; a `-`, an apostrophe and a `.`
/// stay bare.
const SHOWN: &[Shown] = &[("in-transit", true), ("O'Brien", true), ("Re.quote", true), ("on hold", false), ("hold,manual", false), ("a->b", false), ("#1", false), ("say \"hi\"", false)];

/// The same shapes in Japanese, and a `、`, which a list in Japanese puts between its items: a
/// value that holds one is shown in quotes as well.
const SHOWN_JA: &[Shown] = &[("係争中", true), ("再-審査", true), ("保留 中", false), ("保留,手動", false), ("確認->出荷", false), ("確認、保留", false)];

/// How a diagnostic shows the values.
fn shown(values: &[Shown]) -> Vec<String> {
    values.iter().map(|&(v, bare)| if bare { v.to_string() } else { sakai::naming::quote(v) }).collect()
}

/// The values as the items of a YAML list.
fn yaml_items(values: &[Shown]) -> String {
    values.iter().map(|(v, _)| serde_json::to_string(v).unwrap()).collect::<Vec<_>>().join(", ")
}

fn edit(path: &std::path::Path, old: &str, new: &str) {
    let s = std::fs::read_to_string(path).unwrap();
    assert!(s.contains(old), "{} has no {old:?}", path.display());
    std::fs::write(path, s.replacen(old, new, 1)).unwrap();
}

fn diag<'a>(os: &'a [sakai::check::Outcome], code: &str) -> &'a sakai::diag::Diag {
    os.iter().flat_map(|o| o.diags.iter()).find(|d| d.code == code).unwrap_or_else(|| panic!("no {code}: {:?}", codes(os)))
}

/// What E401 names and what the note of E402 lists are the upstream's values as a value line
/// writes them: each, written into a line of the mapping as the note gives it, maps its value.
fn upstream_values_shown(example: &str, ctx: &str, last: &str, values: &[Shown], lang: ritsu_base::text::Lang) {
    use ritsu_base::text::Lang;
    let dir = common::TempDir::new("shown-upstream");
    common::copy_dir(std::path::Path::new(example), dir.path());
    let list = "enum: [pending, succeeded, failed, refunded]";
    edit(&dir.path().join("payments/api/payments.yaml"), list, &format!("enum: [pending, succeeded, failed, refunded, {}]", yaml_items(values)));
    // the values added upstream left unmapped (E401), and a line for a value the enum lacks (E402)
    let file = dir.path().join("contexts").join(ctx);
    let mapping = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, mapping.replacen(last, &format!("{last}    \"not a value\" -> hold\n"), 1)).unwrap();
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E401", "E402"]);
    let (sep, want) = (if lang == Lang::Ja { "、" } else { ", " }, shown(values));
    let (head, tail) = if lang == Lang::Ja { ("の値 ", " がありません") } else { ("maps no value for ", " of ") };
    assert_eq!(common::listed(diag(&os, "E401").message.get(lang), head, tail, sep), want, "E401 names the values as a line writes them");
    let e402 = diag(&os, "E402");
    let (head, tail) = if lang == Lang::Ja { ("対応の ", " は、") } else { ("names ", ", which") };
    assert_eq!(common::listed(e402.message.get(lang), head, tail, sep), ["\"not a value\""]);
    let (head, tail) = if lang == Lang::Ja { ("の値は ", " です。") } else { (" are ", ".") };
    let listed = common::listed(e402.notes[0].get(lang), head, tail, sep);
    assert_eq!(listed[..4], ["pending", "succeeded", "failed", "refunded"]);
    assert_eq!(listed[4..], want[..], "the note of E402 lists the values as a line writes them");
    let lines: String = listed[4..].iter().map(|v| format!("    {v} -> hold\n")).collect();
    std::fs::write(&file, mapping.replacen(last, &format!("{last}{lines}"), 1)).unwrap();
    assert_eq!(codes(&check_dir(dir.path())), Vec::<&str>::new(), "each value, written as the note lists it, maps its value");
}

/// What the note of E403 lists are the target's values as the right side of a value line takes
/// them, a value named `refuse` in quotes: each, written there as the note gives it, is the value
/// it names, and not a refusal.
fn target_values_shown(example: &str, ctx: &str, last: &str, values: &[Shown], lang: ritsu_base::text::Lang) {
    use ritsu_base::text::Lang;
    let dir = common::TempDir::new("shown-target");
    common::copy_dir(std::path::Path::new(example), dir.path());
    let mut values = values.to_vec();
    values.push(("refuse", false));
    edit(&dir.path().join("shipping/api/shipping.yaml"), "enum: [hold, release]", &format!("enum: [hold, release, {}]", yaml_items(&values)));
    let file = dir.path().join("contexts").join(ctx);
    let mapping = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, mapping.replacen("    pending   -> hold\n", "    pending   -> \"not a value\"\n", 1)).unwrap();
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E403"]);
    let e403 = diag(&os, "E403");
    let sep = if lang == Lang::Ja { "、" } else { ", " };
    let (head, tail) = if lang == Lang::Ja { ("の値は ", " です。") } else { (" are ", ".") };
    let listed = common::listed(e403.notes[0].get(lang), head, tail, sep);
    assert_eq!(listed[..2], ["hold", "release"]);
    assert_eq!(listed[2..], shown(&values)[..], "the note of E403 lists the values as the right side takes them");
    // a value added upstream for each value added to the target, mapped to it as the note lists it
    let extra: Vec<String> = (0..values.len()).map(|i| format!("extra_{i}")).collect();
    edit(&dir.path().join("payments/api/payments.yaml"), "enum: [pending, succeeded, failed, refunded]", &format!("enum: [pending, succeeded, failed, refunded, {}]", extra.join(", ")));
    let lines: String = listed[2..].iter().zip(&extra).map(|(v, x)| format!("    {x} -> {v}\n")).collect();
    std::fs::write(&file, mapping.replacen(last, &format!("{last}{lines}"), 1)).unwrap();
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), Vec::<&str>::new(), "each value, written as the note lists it, is a value of the target");
    let api = sakai::api::api(os[0].checked.as_ref().unwrap());
    let rel = api["relationships"].as_array().unwrap().iter().find(|r| r["enums"].as_array().is_some_and(|e| !e.is_empty())).unwrap();
    let mapped = rel["enums"][0]["values"].as_array().unwrap();
    for (x, (v, _)) in extra.iter().zip(&values) {
        let line = mapped.iter().find(|m| m["from"] == x.as_str()).unwrap_or_else(|| panic!("no line for {x}"));
        assert_eq!(*line, serde_json::json!({"from": x, "to": v}), "{x} maps to {v:?}");
    }
}

#[test]
fn the_values_a_diagnostic_shows_are_written_as_a_line_takes_them() {
    let last = "    refunded  -> refuse \"A refunded order is not shipped\"\n";
    upstream_values_shown("examples/webshop", "shipping.ctx", last, SHOWN, ritsu_base::text::Lang::En);
    target_values_shown("examples/webshop", "shipping.ctx", last, SHOWN, ritsu_base::text::Lang::En);
}

#[test]
fn the_values_a_diagnostic_shows_are_written_as_a_line_takes_them_in_japanese() {
    let last = "    refunded  -> refuse \"返金した注文は出荷しない\"\n";
    upstream_values_shown("examples/webshop.ja", "配送.ctx", last, SHOWN_JA, ritsu_base::text::Lang::Ja);
    target_values_shown("examples/webshop.ja", "配送.ctx", last, SHOWN_JA, ritsu_base::text::Lang::Ja);
}
