//! The names (PLAN B.3): a short element becomes its long name, a name two packages have is
//! refused with both, paths are folded by their letters, and E006 to E012 come where they should
//! (their mutants are in tests/mutants; these hold what the mutants do not show).

mod common;

use common::{check_dir, codes, variant};
use sakai::elements::At;

fn elements(dir: &std::path::Path) -> (sakai::model::Model, sakai::elements::Elements) {
    let mut os = check_dir(dir);
    let c = os.remove(0).checked.expect("the map is checked through");
    (c.model, c.elements)
}

#[test]
fn a_short_element_becomes_its_long_name() {
    let dir = variant("基本", &[]);
    let (m, el) = elements(dir.path());
    let inv = m.ctx("在庫").unwrap();
    assert_eq!(el.get(At::Means(inv, 0, 0)).unwrap().text(), "proto \"proto/warehouse/v1/stock.proto\" message ReserveResponse");
    let ord = m.ctx("受注").unwrap();
    assert_eq!(el.get(At::Means(ord, 1, 0)).unwrap().text(), "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus value ORDER_STATUS_CANCELLED");
    let bil = m.ctx("請求").unwrap();
    assert_eq!(el.get(At::From(bil, 0, 0)).unwrap().text(), "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus");
    assert_eq!(el.get(At::To(bil, 0, 0)).unwrap().text(), "proto \"proto/shop/billing/v1/billing.proto\" enum BillingStatus");
}

#[test]
fn a_name_two_packages_have_is_written_with_its_package() {
    let v2 = "syntax = \"proto3\";\n\npackage warehouse.v2;\n\nmessage ReserveResponse {\n  string sku = 1;\n}\n";
    let both = "  open host service StockService, PackingService\n\npublished language warehouse.v2\n  proto \"../proto/warehouse/v2/stock.proto\"\n";
    let dir = variant("基本", &[("proto/warehouse/v2/stock.proto", "", v2), ("ctx/在庫.ctx", "  open host service StockService, PackingService\n", both)]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E007"]);
    let d = &os[0].diags[0];
    assert!(d.message.en.contains("warehouse.v1.ReserveResponse, warehouse.v2.ReserveResponse"), "{}", d.message.en);
    assert!(d.notes[0].en.contains("`message warehouse.v1.ReserveResponse`"), "{:?}", d.notes);
    // Written with its package, it is found.
    let dir = variant(
        "基本",
        &[
            ("proto/warehouse/v2/stock.proto", "", v2),
            ("ctx/在庫.ctx", "  open host service StockService, PackingService\n", both),
            ("ctx/在庫.ctx", "means message ReserveResponse", "means message warehouse.v1.ReserveResponse"),
        ],
    );
    assert_eq!(codes(&check_dir(dir.path())), Vec::<&str>::new());
}

#[test]
fn a_path_in_a_name_is_folded_by_its_letters() {
    let dir = variant("基本", &[("ctx/在庫.ctx", "means message ReserveResponse", "means proto \"../proto/./warehouse/v1/../v1/stock.proto\" message ReserveResponse")]);
    let (m, el) = elements(dir.path());
    assert_eq!(el.get(At::Means(m.ctx("在庫").unwrap(), 0, 0)).unwrap().text(), "proto \"proto/warehouse/v1/stock.proto\" message ReserveResponse");
}

#[test]
fn a_child_that_is_not_there_and_a_child_of_the_wrong_kind() {
    let dir = variant("基本", &[("ctx/受注.ctx", "value ORDER_STATUS_CANCELLED", "value ORDER_STATUS_CANCELED")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E007"]);
    assert!(os[0].diags[0].message.en.contains("The enum OrderStatus has no value ORDER_STATUS_CANCELED"));
    let dir = variant("基本", &[("ctx/受注.ctx", "means message Order\n", "means message Order value X\n")]);
    assert_eq!(codes(&check_dir(dir.path())), ["E011"]);
}

#[test]
fn an_absolute_path_and_a_kind_dandori_does_not_have() {
    let dir = variant("基本", &[("ctx/在庫.ctx", "dir \"../proto/warehouse\", \"../py/inventory\"", "dir \"../proto/warehouse\", \"../py/inventory\", \"/etc\"")]);
    assert_eq!(codes(&check_dir(dir.path())), ["E012"]);
    let dir = variant("基本", &[("ctx/受注.ctx", "means message Order\n", "means dandori \"../py/ordering/fulfill.py\" task reserve\n")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E011"]);
    assert!(os[0].diags[0].message.en.contains("dandori has no kinds yet"), "{}", os[0].diags[0].message.en);
}

#[test]
fn the_names_and_aliases_of_the_contexts_all_differ() {
    let dir = variant("基本", &[("ctx/請求.ctx", "context 請求(billing) v1", "context 請求(ordering) v1")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E006"]);
    assert!(os[0].diags[0].message.en.contains("The alias ordering is declared twice"));
}
