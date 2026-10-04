//! `sakai api` (PLAN B.11, DESIGN 9): the JSON of the maps of B, held to its golden files in
//! `tests/golden/api/`, with its keys in the order DESIGN 9 gives.

mod common;

#[test]
fn the_api_of_each_map_is_its_golden_file() {
    let mut failures = Vec::new();
    for name in ["基本", "パターン"] {
        let dir = common::variant(name, &[]);
        let os = common::check_dir(dir.path());
        let c = os.iter().find_map(|o| o.checked.as_ref()).unwrap();
        let text = serde_json::to_string_pretty(&sakai::api::api(c)).unwrap() + "\n";
        if let Some(f) = common::golden(&format!("tests/golden/api/{name}.json"), &text) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_keys_come_in_their_order() {
    let dir = common::variant("基本", &[]);
    let os = common::check_dir(dir.path());
    let v = sakai::api::api(os[0].checked.as_ref().unwrap());
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["sakai", "map", "covers", "except", "contexts", "relationships", "artifacts", "crossings"]);
    let a = &v["artifacts"][0];
    let keys: Vec<&String> = a.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["name", "context", "by", "sha256"]);
    let n: Vec<&String> = a["name"].as_object().unwrap().keys().collect();
    assert_eq!(n, ["text", "tool", "path", "items"]);
    // Every artifact of the scope is there, with its owner.
    assert_eq!(v["artifacts"].as_array().unwrap().len(), 9);
    assert_eq!(v["crossings"].as_array().unwrap().len(), 3);
}

/// The command prints what the library gives, with the root from --root.
#[test]
fn the_command_prints_the_same() {
    let o = common::sakai(&["api", "tests/maps/基本/基本.ctx", "--root", "tests/maps/基本"]);
    let want = std::fs::read_to_string("tests/golden/api/基本.json").unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), want);
}

/// Each crossing says how its file refers to the other side, in the words of the file's language
/// (DESIGN 9): the example's map, with every language joined, as `ritsu sakai api`.
#[test]
fn each_crossing_says_how_it_refers() {
    let (code, out, err) = common::joined(&["api", "examples/shop.ja/通販.ctx", "--root", "examples/shop.ja"]);
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let got: Vec<String> = v["crossings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| format!("{} {} -> {} ({}, {})", c["from"]["text"].as_str().unwrap(), c["line"], c["to"]["text"].as_str().unwrap(), c["via"].as_str().unwrap(), c["allowed_by"]["relationship"].as_str().unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            "proto \"proto/shop/ordering/v1/fulfillment.proto\" 15 -> proto \"proto/warehouse/v1/stock.proto\" (proto import, upstream_downstream)",
            "rulec \"billing/rules/出荷の送料.rule\" 7 -> proto \"proto/shop/delivery/v1/shipment.proto\" message CreateShipmentRequest (shape, upstream_downstream)",
            "rulec \"billing/rules/請求の要否.rule\" 4 -> proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus (import proto, upstream_downstream)",
            "koyomi \"delivery/出荷日.cal\" 3 -> koyomi \"calendars/東京の営業日.cal\" (use calendar, shared_kernel)",
            "dandori \"ordering/受注.flow\" 6 -> rulec \"delivery/rules/出荷の急ぎ.rule\" (use rule … connect, partnership)",
            "dandori \"ordering/受注.flow\" 9 -> proto \"proto/warehouse/v1/stock.proto\" (use proto, upstream_downstream)",
            "dandori \"ordering/受注.flow\" 47 -> proto \"proto/warehouse/v1/stock.proto\" service StockService method Reserve (connect, upstream_downstream)",
            "dandori \"ordering/受注.flow\" 53 -> proto \"proto/warehouse/v1/stock.proto\" service StockService method Release (connect, upstream_downstream)",
            "dandori \"ordering/受注.flow\" 59 -> dandori \"delivery/配送の手配.flow\" (flow, partnership)",
        ]
    );
    // a rule's enum and a shape's message cross with what they reach
    let shape = &v["crossings"][1]["elements"];
    assert_eq!(shape.as_array().unwrap().len(), 4, "{shape}");
    // what is not checked is said where it is: a map that does not pass is not printed (ritsu's
    // stage E took the always empty `not_checked` out; DESIGN 9)
    assert!(v.get("not_checked").is_none());
}

// ── The English twins: the same checks on the English maps and example ──

#[test]
fn the_api_of_each_map_is_its_golden_file_in_english() {
    let mut failures = Vec::new();
    for name in ["basic", "patterns"] {
        let dir = common::variant(name, &[]);
        let os = common::check_dir(dir.path());
        let c = os.iter().find_map(|o| o.checked.as_ref()).unwrap();
        let text = serde_json::to_string_pretty(&sakai::api::api(c)).unwrap() + "\n";
        if let Some(f) = common::golden(&format!("tests/golden/api/{name}.json"), &text) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_keys_come_in_their_order_in_english() {
    let dir = common::variant("basic", &[]);
    let os = common::check_dir(dir.path());
    let v = sakai::api::api(os[0].checked.as_ref().unwrap());
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["sakai", "map", "covers", "except", "contexts", "relationships", "artifacts", "crossings"]);
    let a = &v["artifacts"][0];
    let keys: Vec<&String> = a.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["name", "context", "by", "sha256"]);
    let n: Vec<&String> = a["name"].as_object().unwrap().keys().collect();
    assert_eq!(n, ["text", "tool", "path", "items"]);
    assert_eq!(v["artifacts"].as_array().unwrap().len(), 9);
    assert_eq!(v["crossings"].as_array().unwrap().len(), 3);
}

#[test]
fn the_command_prints_the_same_in_english() {
    let o = common::sakai(&["api", "tests/maps/basic/basic.ctx", "--root", "tests/maps/basic"]);
    let want = std::fs::read_to_string("tests/golden/api/basic.json").unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), want);
}

#[test]
fn each_crossing_says_how_it_refers_in_english() {
    let (code, out, err) = common::joined(&["api", "examples/shop/shop.ctx", "--root", "examples/shop"]);
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let got: Vec<String> = v["crossings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| format!("{} {} -> {} ({}, {})", c["from"]["text"].as_str().unwrap(), c["line"], c["to"]["text"].as_str().unwrap(), c["via"].as_str().unwrap(), c["allowed_by"]["relationship"].as_str().unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            "proto \"proto/shop/ordering/v1/fulfillment.proto\" 16 -> proto \"proto/warehouse/v1/stock.proto\" (proto import, upstream_downstream)",
            "rulec \"billing/rules/billing_need.rule\" 4 -> proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus (import proto, upstream_downstream)",
            "rulec \"billing/rules/shipment_fee.rule\" 7 -> proto \"proto/shop/delivery/v1/shipment.proto\" message CreateShipmentRequest (shape, upstream_downstream)",
            "koyomi \"delivery/ship_date.cal\" 3 -> koyomi \"calendars/tokyo_business_days.cal\" (use calendar, shared_kernel)",
            "dandori \"ordering/fulfillment.flow\" 6 -> rulec \"delivery/rules/urgency.rule\" (use rule … connect, partnership)",
            "dandori \"ordering/fulfillment.flow\" 9 -> proto \"proto/warehouse/v1/stock.proto\" (use proto, upstream_downstream)",
            "dandori \"ordering/fulfillment.flow\" 47 -> proto \"proto/warehouse/v1/stock.proto\" service StockService method Reserve (connect, upstream_downstream)",
            "dandori \"ordering/fulfillment.flow\" 53 -> proto \"proto/warehouse/v1/stock.proto\" service StockService method Release (connect, upstream_downstream)",
            "dandori \"ordering/fulfillment.flow\" 59 -> dandori \"delivery/arrange_delivery.flow\" (flow, partnership)",
        ]
    );
    let shape = &v["crossings"][2]["elements"];
    assert_eq!(shape.as_array().unwrap().len(), 4, "{shape}");
    assert!(v.get("not_checked").is_none());
}
