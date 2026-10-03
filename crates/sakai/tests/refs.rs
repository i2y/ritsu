//! The references that cross a boundary, from the imports of the `.proto` files (PLAN B.7).

mod common;

use common::{check_dir, codes, variant};
use sakai::refs::Allowed;

fn crossings(base: &str) -> Vec<(String, String, String)> {
    let dir = variant(base, &[]);
    let os = check_dir(dir.path());
    let c = os[0].checked.as_ref().unwrap();
    c.crossings
        .iter()
        .map(|x| {
            let how = match &x.allowed {
                Some(Allowed::Kernel(_)) => "shared kernel".to_string(),
                Some(Allowed::Partnership) => "partnership".to_string(),
                Some(Allowed::Upstream(ri)) => c.model.contexts[x.from_ctx].rels[*ri].roles().iter().map(|r| r.word()).collect::<Vec<_>>().join(", "),
                None => "not allowed".to_string(),
            };
            (x.from.clone(), x.to.clone(), how)
        })
        .collect()
}

#[test]
fn the_three_crossings_of_the_basic_map() {
    let want = [
        ("proto/billing/acl/v1/order_view.proto", "proto/shop/ordering/v1/order.proto", "anticorruption layer"),
        ("proto/shop/ordering/v1/fulfillment_lite.proto", "proto/warehouse/v1/stock.proto", "conformist"),
        ("proto/warehouse/v1/stock.proto", "proto/shop/common/v1/money.proto", "shared kernel"),
    ];
    let got = crossings("基本");
    assert_eq!(got.iter().map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str())).collect::<Vec<_>>(), want);
}

#[test]
fn the_crossings_of_the_patterns() {
    let want = [
        ("proto/contracts/acl/v1/plan_view.proto", "proto/pricing/v1/price.proto", "customer, anticorruption layer"),
        ("proto/notices/v1/notice.proto", "proto/quotes/v1/estimate.proto", "partnership"),
        ("proto/quotes/v1/estimate.proto", "proto/pricing/v1/price.proto", "customer"),
    ];
    let got = crossings("パターン");
    assert_eq!(got.iter().map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str())).collect::<Vec<_>>(), want);
}

#[test]
fn what_crosses_with_a_message() {
    let dir = variant("基本", &[]);
    let os = check_dir(dir.path());
    let c = os[0].checked.as_ref().unwrap();
    let x = c.crossings.iter().find(|x| x.from.ends_with("fulfillment_lite.proto")).unwrap();
    let names: Vec<&str> = x.reach.iter().map(|s| s.full.as_str()).collect();
    assert_eq!(names, ["warehouse.v1.ReserveResponse", "warehouse.v1.Stock", "shop.common.v1.Money"]);
}

#[test]
fn a_reference_against_the_relationship_is_told_the_other_way_round() {
    // 在庫, upstream of 受注, imports the order of 受注.
    let dir = variant("基本", &[("proto/warehouse/v1/stock.proto", "import \"shop/common/v1/money.proto\";\n", "import \"shop/common/v1/money.proto\";\nimport \"shop/ordering/v1/order.proto\";\n")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E201"]);
    assert!(os[0].diags[0].notes[0].en.contains("the other way round"));
    assert_eq!((os[0].diags[0].file.as_str(), os[0].diags[0].line), ("proto/warehouse/v1/stock.proto", Some(6)));
}

#[test]
fn every_code_of_the_crossings_points_at_the_import() {
    for (name, code) in [
        ("E201_関係の無い在庫を参照", "E201"),
        ("E202_在庫の内側を参照", "E202"),
        ("E203_through_に無い_package", "E203"),
        ("E204_腐敗防止層の外", "E204"),
        ("E205_公表された言語に上流の型", "E205"),
        ("E206_別々の道の相手を参照", "E206"),
    ] {
        let dir = common::mutant(name);
        let os = check_dir(dir.path());
        let d = os.iter().flat_map(|o| o.diags.iter()).find(|d| d.code == code).unwrap();
        assert!(d.file.ends_with(".proto"), "{name}: {}", d.file);
        assert!(d.src.as_deref().is_some_and(|s| s.starts_with("import ")), "{name}: {:?}", d.src);
        assert!(d.refs.len() >= 2, "{name}");
    }
}
