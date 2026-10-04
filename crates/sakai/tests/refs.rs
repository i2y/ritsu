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
        assert!(d.extra.0.len() >= 2, "{name}");
    }
}

/// A child workflow across the boundary that implements an open host service of the other side
/// crosses as that service (DESIGN 4.7): the relationship and its `through` hold it, as they hold a
/// reference to the service's `.proto` (E203 when `through` does not list its package); with no
/// service implemented, the same child is E209.
#[test]
fn a_child_that_implements_an_open_host_service_crosses_as_the_service() {
    let dir = common::TempDir::new("child");
    for (name, body) in sakai::codes::BASE {
        dir.write(name, body);
    }
    dir.write(
        "乙.ctx",
        "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\npublished language b.run.v1\n  proto \"b/run/v1/run.proto\"\n  open host service RunService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
    );
    dir.write(
        "b/run/v1/run.proto",
        "syntax = \"proto3\";\npackage b.run.v1;\nimport \"dandori/v1/options.proto\";\nservice RunService {\n  option (dandori.v1.workflow) = {name: \"c\", version: 1};\n  rpc Start(StartRequest) returns (StartResponse) {\n    option (dandori.v1.start) = {};\n  }\n}\nmessage StartRequest { string id = 1; }\nmessage StartResponse { string id = 1; }\n",
    );
    dir.write("b/c.flow", "workflow c v1 implements r1.RunService\n\nuse proto r1 from \"run/v1/run.proto\"\n\ninputs\n  id : string\n\noutputs\n  id : string\n\nflow\n  succeed id = id\n");
    dir.write("a/p.flow", "workflow p v1\n\nrecord 答え\n  id : string\n\ninputs\n  id : string\n\ntask run_child(id: string) -> 答え\n  flow \"../b/c.flow\"\n\nflow\n  let r = run_child(id: id)\n  succeed\n");
    let ctx = |through: &str| format!("context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 conformist\n  through {through}\n");
    dir.write("甲.ctx", &ctx("b.run.v1"));
    let os = check_dir(dir.path());
    assert!(codes(&os).is_empty(), "{:?}", codes(&os));
    let c = os[0].checked.as_ref().unwrap();
    let child: Vec<_> = c.crossings.iter().filter(|x| x.kind == sakai::refs::Kind::FlowChild).collect();
    assert_eq!(child.len(), 1);
    assert_eq!(child[0].allowed, Some(Allowed::Upstream(0)));
    // the service's package is what `through` has to list
    dir.write("甲.ctx", &ctx("b.v1"));
    assert_eq!(codes(&check_dir(dir.path())), ["E203"]);
    // a child that implements nothing is the other side's own workflow
    dir.write("甲.ctx", &ctx("b.run.v1"));
    dir.write("b/c.flow", "workflow c v1\n\ninputs\n  id : string\n\noutputs\n  id : string\n\nflow\n  succeed id = id\n");
    assert_eq!(codes(&check_dir(dir.path())), ["E209"]);
}
