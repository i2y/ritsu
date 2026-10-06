//! The elements of OpenAPI and AsyncAPI documents and of Cedar's files as the ends of links
//! (DESIGN 3.6), on the Japanese material `contracts`: the twin of english_contracts.rs, the same
//! shop with Japanese names (a schema `返金` and its property `金額`, the operation `返金する`, the
//! Cedar action `返金` and the policy `係は自分の上限まで返金できる`), and the diagnostics in
//! Japanese.

mod common;

use ritsu_base::text::Lang;
use std::collections::BTreeMap;
use std::path::Path;

const DIR: &str = "tests/fixtures/contracts";

const REFUND_ORDER: usize = 10;
const ACTION: usize = 12;
const AMOUNT: usize = 21;
const POLICY: usize = 30;
const EVENT: usize = 39;

fn ends(dir: &str) -> BTreeMap<String, (String, String)> {
    let c = common::check(dir);
    assert!(c.diags.is_empty(), "{dir}: {:?}", c.diags.iter().map(|d| (d.code, d.message.ja.clone())).collect::<Vec<_>>());
    let m = c.model.as_ref().unwrap();
    m.artifacts.iter().map(|(n, e)| (n.text(), e.as_ref().map(|e| (e.hash.clone(), String::from_utf8_lossy(&e.bytes).to_string())).unwrap())).collect()
}

fn marks(change: impl Fn(&Path)) -> Vec<(&'static str, usize)> {
    let t = common::fixture("contracts");
    let d = t.path().join("contracts");
    change(&d);
    let c = common::check(&d.to_string_lossy());
    c.diags.iter().map(|x| (x.code, x.line.unwrap_or(0))).collect()
}

fn said(change: impl Fn(&Path)) -> Vec<String> {
    let t = common::fixture("contracts");
    let d = t.path().join("contracts");
    change(&d);
    let c = common::check(&d.to_string_lossy());
    c.diags.iter().map(|x| x.render(Lang::Ja)).collect()
}

fn edit(rel: &'static str, from: &'static str, to: &'static str) -> impl Fn(&Path) {
    move |d: &Path| common::edit(d, rel, from, to)
}

#[test]
fn the_ends_of_the_elements() {
    let e = ends(DIR);
    let mut failures = Vec::new();
    for (naming, golden) in [
        ("openapi \"api/注文.yaml\" operation 返金する", "openapi-返金する.txt"),
        ("openapi \"api/注文.yaml\" schema 返金 property 金額", "openapi-返金の金額.txt"),
        ("asyncapi \"events/注文.yaml\" operation 返金を知らせる", "asyncapi-返金を知らせる.txt"),
        ("cedar \"policies/返金.cedar\" policy 係は自分の上限まで返金できる", "cedar-係の上限.txt"),
        ("cedar \"policies/店.cedarschema\" action 返金", "cedar-返金.txt"),
    ] {
        let (_, text) = e.get(naming).unwrap_or_else(|| panic!("no end for {naming}: {:?}", e.keys().collect::<Vec<_>>()));
        common::golden(&format!("tests/golden/ends/{golden}"), text, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn an_element_of_a_document_stops_its_links_and_no_others() {
    assert_eq!(marks(edit("api/注文.yaml", "maximum: 100000", "maximum: 200000")), [("E303", REFUND_ORDER), ("E303", AMOUNT)]);
    assert_eq!(marks(edit("api/注文.yaml", "required: [金額, 理由]", "required: [理由]")), [("E303", REFUND_ORDER), ("E303", AMOUNT)], "求められなくなった項目");
    assert_eq!(marks(edit("api/注文.yaml", "enum: [破損, 遅延, 不要]", "enum: [破損, 遅延, 不要, 紛失]")), [("E303", REFUND_ORDER)]);
    assert!(marks(edit("api/注文.yaml", "summary: 注文を読む", "summary: 一つの注文を読む")).is_empty(), "ほかの操作");
    assert!(marks(edit("common/金額.yaml", "enum: [JPY, USD, EUR]", "enum: [JPY, USD]")).is_empty(), "ほかの操作だけが読む、文書の一部");
    assert_eq!(marks(edit("events/注文.yaml", "        金額:\n          type: integer\n          minimum: 1", "        金額:\n          type: integer\n          minimum: 100")), [("E303", EVENT)]);
    assert!(marks(edit("events/注文.yaml", "address: orders.shipped", "address: orders.dispatched")).is_empty(), "ほかのチャネル");
}

#[test]
fn a_policy_and_an_action_stop_their_links_and_no_others() {
    assert_eq!(marks(edit("policies/返金.cedar", "context.amount <= principal.refund_limit", "context.amount < principal.refund_limit")), [("E303", POLICY)]);
    assert!(marks(edit("policies/返金.cedar", "// だれが注文を返金できるか、いくらまでか。", "// だれが注文を返金できるか、いくらまでか（2026 年 10 月から）。")).is_empty(), "コメント");
    assert!(marks(edit("policies/返金.cedar", "principal in Shop::Role::\"manager\"", "principal in Shop::Role::\"owner\"")).is_empty(), "ほかのポリシー");
    assert_eq!(marks(edit("policies/店.cedarschema", "  type RefundContext = {\n    amount: Long\n  };", "  type RefundContext = {\n    amount: Long,\n    reason: String\n  };")), [("E303", ACTION)]);
    assert!(marks(edit("policies/店.cedarschema", "  action \"注文を見る\" appliesTo {", "  action \"注文を見る\" in [\"返金\"] appliesTo {")).is_empty(), "ほかのアクション");
}

#[test]
fn what_is_not_there_and_what_does_not_read() {
    let d = said(edit("返金.req", "operation 返金する\n", "operation 返金\n"));
    assert!(d[0].starts_with("エラー[E202]: ") && d[0].contains("api/注文.yaml に operation 返金 はありません"), "{}", d[0]);
    assert!(d[0].contains("openapi \"api/注文.yaml\" operation 返金する"), "{}", d[0]);
    let d = said(edit("返金.req", "satisfied by asyncapi \"events/注文.yaml\"", "satisfied by openapi \"events/注文.yaml\""));
    assert!(d[0].starts_with("エラー[E205]: ") && d[0].contains("events/注文.yaml を OpenAPI の文書として読めません: AsyncAPI の文書です。`asyncapi \"…\"` で指してください"), "{}", d[0]);
    let d = said(|d: &Path| {
        let s = std::fs::read_to_string(d.join("policies/店.cedarschema")).unwrap();
        std::fs::write(d.join("policies/店.cedarschema"), format!("{s}namespace Warehouse {{\n  action \"返金\";\n}}\n")).unwrap();
    });
    assert!(d[0].contains("[E202]") && d[0].contains("action 返金 を二つ以上の名前空間（Shop、Warehouse）で宣言しているので、一つに決まりません"), "{}", d[0]);
}
