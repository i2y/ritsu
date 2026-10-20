//! `ritsu sakai` (DESIGN 8.6): sakai's command, with every language it reads joined through the
//! ports — a rule's enums, its Connect service and its names, what a rule, a calendar and a
//! workflow refer to outside themselves, and a book's accounts and transfers. What only this binary
//! can run is here: the binary, on sakai's example and on a change to it.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn sakai_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sakai")
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu"))
        .current_dir(dir)
        .env_remove("RITSU_LANG")
        .env_remove("SAKAI_LANG")
        .args(args)
        .output()
        .expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// sakai's example holds rules, calendars, a book and workflows: `ritsu sakai check` reads what
/// each of them refers to across a boundary, which the binary of sakai's own crate cannot (it says
/// E104 for each language; sakai's `tests/examples.rs`).
#[test]
fn ritsu_sakai_checks_the_example_with_every_language() {
    let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "check", "examples/shop.ja/通販.ctx"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "examples/shop.ja/通販.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)\n");
    let (code, out, _) = ritsu_in(&sakai_dir(), &["sakai", "check", "examples/shop.ja/通販.ctx", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.ends_with("境界を越える参照 9 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 5）\n"), "{out}");
    // the api names how each crossing refers, in the words of the file's language
    let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "api", "examples/shop.ja/通販.ctx", "--root", "examples/shop.ja"]);
    assert_eq!(code, 0, "{err}");
    let api: serde_json::Value = serde_json::from_str(&out).unwrap();
    let via: Vec<&str> = api["crossings"].as_array().unwrap().iter().map(|c| c["via"].as_str().unwrap()).collect();
    assert_eq!(via, ["proto import", "shape", "import proto", "use calendar", "use rule … connect", "use proto", "connect", "connect", "flow"]);
    // the settings the example keeps are what the map writes, read with every language joined
    for t in ["import-linter", "dependency-cruiser", "archunit", "go-arch-lint"] {
        let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "build", "examples/shop.ja/通販.ctx", "--target", t, "--check", "--lang", "ja"]);
        assert_eq!(code, 0, "{t}: {out}{err}");
        assert!(out.contains("いまの地図から書くものと同じ"), "{t}: {out}");
    }
    // the words after `sakai` are sakai's
    let (code, out, _) = ritsu_in(&sakai_dir(), &["sakai", "--version"]);
    assert!(code == 0 && out.starts_with("sakai ") && out.lines().count() == 1, "{out}");
}

/// A change to the example that only the other languages show: ordering's workflow runs
/// delivery's as its child, and the two stop being partners (E209); billing's rule takes in an
/// order status that ordering has added a value to, and rulec does not answer for it (E105).
#[test]
fn ritsu_sakai_catches_what_crosses_through_another_language() {
    let t = TempDir::new("sakai-example");
    ritsu_testkit::tmp::copy_dir(&sakai_dir().join("examples/shop.ja"), t.path());
    let edit = |file: &str, old: &str, new: &str| {
        let p = t.path().join(file);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{file} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    };
    edit("contexts/受注.ctx", "partnership with 配送\n", "upstream 配送 customer\n  through rulec.urgency.v1\n");
    edit("contexts/配送.ctx", "partnership with 受注\n", "downstream 受注 supplier\n");
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "check", "通販.ctx"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.starts_with("error[E209]: ordering/受注.flow:59:1: The workflow ordering/受注.flow of 受注 runs 配送's workflow delivery/配送の手配.flow as its child\n"), "{out}");
    edit("proto/shop/ordering/v1/order.proto", "  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n");
    let (code, out, _) = ritsu_in(t.path(), &["sakai", "check", "通販.ctx", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(out.contains("エラー[E105]: billing/rules/請求の要否.rule:5:1: billing/rules/請求の要否.rule が rulec の検査を通らないか、読めません\n"), "{out}");
    assert!(out.contains("  = rulec の診断: [E032] billing/rules/請求の要否.rule:5: "), "{out}");
}

// ── The English twins: the same on the English example ──

#[test]
fn ritsu_sakai_checks_the_english_example_with_every_language() {
    let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "check", "examples/shop/shop.ctx"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "examples/shop/shop.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)\n");
    let (code, out, _) = ritsu_in(&sakai_dir(), &["sakai", "check", "examples/shop/shop.ctx", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.ends_with("境界を越える参照 9 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 5）\n"), "{out}");
    let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "api", "examples/shop/shop.ctx", "--root", "examples/shop"]);
    assert_eq!(code, 0, "{err}");
    let api: serde_json::Value = serde_json::from_str(&out).unwrap();
    let mut via: Vec<&str> = api["crossings"].as_array().unwrap().iter().map(|c| c["via"].as_str().unwrap()).collect();
    // the Japanese example's order, which differs by its two rules' names only
    via.sort();
    let mut want = vec!["proto import", "shape", "import proto", "use calendar", "use rule … connect", "use proto", "connect", "connect", "flow"];
    want.sort();
    assert_eq!(via, want);
    for t in ["import-linter", "dependency-cruiser", "archunit", "go-arch-lint"] {
        let (code, out, err) = ritsu_in(&sakai_dir(), &["sakai", "build", "examples/shop/shop.ctx", "--target", t, "--check"]);
        assert_eq!(code, 0, "{t}: {out}{err}");
        assert!(out.contains("Up to date"), "{t}: {out}");
    }
}

/// The twin of `ritsu_sakai_catches_what_crosses_through_another_language`.
#[test]
fn ritsu_sakai_catches_what_crosses_through_another_language_in_english() {
    let t = TempDir::new("sakai-example-en");
    ritsu_testkit::tmp::copy_dir(&sakai_dir().join("examples/shop"), t.path());
    let edit = |file: &str, old: &str, new: &str| {
        let p = t.path().join(file);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{file} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    };
    edit("contexts/ordering.ctx", "partnership with Delivery\n", "upstream Delivery customer\n  through rulec.urgency.v1\n");
    edit("contexts/delivery.ctx", "partnership with Ordering\n", "downstream Ordering supplier\n");
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "check", "shop.ctx"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.starts_with("error[E209]: ordering/fulfillment.flow:59:1: The workflow ordering/fulfillment.flow of Ordering runs Delivery's workflow delivery/arrange_delivery.flow as its child\n"), "{out}");
    edit("proto/shop/ordering/v1/order.proto", "  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n");
    let (code, out, _) = ritsu_in(t.path(), &["sakai", "check", "shop.ctx", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(out.contains("エラー[E105]: billing/rules/billing_need.rule:5:1: billing/rules/billing_need.rule が rulec の検査を通らないか、読めません\n"), "{out}");
    assert!(out.contains("  = rulec の診断: [E032] billing/rules/billing_need.rule:5: "), "{out}");
}

// ── A dates file across a boundary: a workflow's `use dates`, held as a gate's and a module's ──

/// Three files that read a dates file, one of each language: a workflow, a gate and a module, as
/// (path, text, the line of the `use dates`).
type Readers = [(&'static str, &'static str, usize); 3];

/// Delivery's three, reading Billing's payment terms; Ordering's three, reading Delivery's ship date.
const DELIVERY_READS: Readers = [
    (
        "delivery/collect.flow",
        "workflow collect v1\ndescription \"Collects the payment of a cash-on-delivery parcel on the day billing's payment terms make it due\"\n\nuse dates terms from \"../billing/payment_terms.cal\"\n\ninputs\n  parcel   : string\n  received : date\n\ntask collect(parcel: string, due: date)\n  key\n\nflow\n  let due = terms.payment(received: received)\n  collect(parcel: parcel, due: due.day)\n",
        4,
    ),
    (
        "delivery/gates/collect.gate",
        "gate collect v1\n\nuse dates terms from \"../../billing/payment_terms.cal\"\n\nrole courier\n\nprincipal Courier\n  roles courier\n\nresource Parcel\n\naction collect\n  principal Courier\n  resource Parcel\n\npermit couriers_collect\n  principal in courier\n  action collect\n",
        3,
    ),
    (
        "delivery/collect.kn",
        "module collect v1\ndescription \"The day a cash-on-delivery parcel's payment is due, by billing's payment terms\"\n\nuse dates terms from \"../billing/payment_terms.cal\"\n\nfn due(received: date range >=2026-01-01 <=2027-11-20) -> date\n  terms.payment(received: received)\n",
        4,
    ),
];
const ORDERING_READS: Readers = [
    (
        "ordering/ship_notice.flow",
        "workflow ship_notice v1\ndescription \"Tells the customer the day an order ships, by delivery's ship date\"\n\nuse dates shipping from \"../delivery/ship_date.cal\"\n\ninputs\n  order   : string\n  ordered : date\n\ntask tell(order: string, day: date)\n  key\n\nflow\n  let ship = shipping.ship(ordered: ordered)\n  tell(order: order, day: ship.day)\n",
        4,
    ),
    (
        "ordering/gates/ship_notice.gate",
        "gate ship_notice v1\n\nuse dates shipping from \"../../delivery/ship_date.cal\"\n\nrole clerk\n\nprincipal Clerk\n  roles clerk\n\nresource Order\n\naction tell\n  principal Clerk\n  resource Order\n\npermit clerks_tell\n  principal in clerk\n  action tell\n",
        3,
    ),
    (
        "ordering/ship_notice.kn",
        "module ship_notice v1\ndescription \"The day an order ships, by delivery's ship date\"\n\nuse dates shipping from \"../delivery/ship_date.cal\"\n\nfn ship_day(ordered: date range >=2026-01-01 <=2027-12-20) -> date\n  shipping.ship(ordered: ordered)\n",
        4,
    ),
];
const DELIVERY_READS_JA: Readers = [
    (
        "delivery/代金の受け取り.flow",
        "workflow 代金の受け取り v1\ndescription \"代金引換の荷物の代金を、請求の支払条件で決まる期日に受け取る\"\n\nuse dates 支払条件 from \"../billing/支払条件.cal\"\n\ninputs\n  荷物   : string\n  受領日 : date\n\ntask 受け取る(荷物: string, 期日: date)\n  key\n\nflow\n  let 期日 = 支払条件.支払日(受領日: 受領日)\n  受け取る(荷物: 荷物, 期日: 期日.day)\n",
        4,
    ),
    (
        "delivery/gates/代金の受け取り.gate",
        "gate 代金の受け取り(collect_ja) v1\n\nuse dates 支払条件 from \"../../billing/支払条件.cal\"\n\nrole 配達員(courier)\n\nprincipal 配達の担当(Courier)\n  roles 配達員\n\nresource 荷物(Parcel)\n\naction 受け取る(collect)\n  principal 配達の担当\n  resource 荷物\n\npermit 配達員は受け取れる(couriers_collect)\n  principal in 配達員\n  action 受け取る\n",
        3,
    ),
    (
        "delivery/代金の受け取り.kn",
        "module 代金の受け取り(collect_ja) v1\ndescription \"代金引換の荷物の代金の期日を、請求の支払条件で決める\"\n\nuse dates 支払条件 from \"../billing/支払条件.cal\"\n\nfn 期日(due)(受領日(received): date range >=2026-01-01 <=2027-11-20) -> date\n  支払条件.支払日(受領日: 受領日)\n",
        4,
    ),
];
const ORDERING_READS_JA: Readers = [
    (
        "ordering/出荷の知らせ.flow",
        "workflow 出荷の知らせ v1\ndescription \"注文が出荷される日を、配送の出荷日のとおりにお客さまに知らせる\"\n\nuse dates 出荷 from \"../delivery/出荷日.cal\"\n\ninputs\n  注文   : string\n  受注日 : date\n\ntask 知らせる(注文: string, 日: date)\n  key\n\nflow\n  let 出す日 = 出荷.出荷日(受注日: 受注日)\n  知らせる(注文: 注文, 日: 出す日.day)\n",
        4,
    ),
    (
        "ordering/gates/出荷の知らせ.gate",
        "gate 出荷の知らせ(ship_notice_ja) v1\n\nuse dates 出荷 from \"../../delivery/出荷日.cal\"\n\nrole 係(clerk)\n\nprincipal 受注の係(Clerk)\n  roles 係\n\nresource 注文(Order)\n\naction 知らせる(tell)\n  principal 受注の係\n  resource 注文\n\npermit 係は知らせられる(clerks_tell)\n  principal in 係\n  action 知らせる\n",
        3,
    ),
    (
        "ordering/出荷の知らせ.kn",
        "module 出荷の知らせ(ship_notice_ja) v1\ndescription \"注文が出荷される日を、配送の出荷日のとおりに決める\"\n\nuse dates 出荷 from \"../delivery/出荷日.cal\"\n\nfn 出す日(ship_day)(受注日(ordered): date range >=2026-01-01 <=2027-12-20) -> date\n  出荷.出荷日(受注日: 受注日)\n",
        4,
    ),
];

/// One example and its names: the directory, the map, Ordering, Delivery and Billing as the
/// diagnostics say them, Billing's payment terms and Delivery's ship date, and the line of the
/// calendar the shared kernel of Billing and Delivery lists, on each side.
struct Shop {
    dir: &'static str,
    map: &'static str,
    names: [&'static str; 3],
    terms: &'static str,
    ship: &'static str,
    kernel: [&'static str; 2],
    calendar: &'static str,
}

/// sakai holds a workflow's `use dates` to the map as it holds a gate's and a module's (sakai's
/// DESIGN 19): Delivery's three read Billing's payment terms, and Billing is Delivery's customer,
/// the other way round (E201, the same words for each but the file and the line); Ordering's three
/// read Delivery's ship date, a partner's inside (E202, the same words, each with the note of its
/// language); and with the payment terms in the shared kernel of Billing and Delivery, Delivery's
/// three pass and are counted by their languages, each by `use dates`.
fn a_dates_file_is_read_alike(shop: &Shop, delivery: &Readers, ordering: &Readers) {
    let [o, d, b] = shop.names;
    let lay = |readers: &[&Readers], kernel: bool| {
        let t = TempDir::new("sakai-dates");
        ritsu_testkit::tmp::copy_dir(&sakai_dir().join(shop.dir), t.path());
        for (f, body, _) in readers.iter().flat_map(|r| r.iter()) {
            t.write(f, body);
        }
        if kernel {
            for f in shop.kernel {
                let p = t.path().join(f);
                let s = std::fs::read_to_string(&p).unwrap();
                assert!(s.contains(shop.calendar), "{f} has no {:?}", shop.calendar);
                std::fs::write(&p, s.replacen(shop.calendar, &format!("{}  koyomi \"../{}\"\n", shop.calendar, shop.terms), 1)).unwrap();
            }
        }
        t
    };
    let t = lay(&[delivery, ordering], false);
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "check", shop.map]);
    assert_eq!(code, 1, "{out}{err}");
    let mut got: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    let mut want: Vec<String> = delivery.iter().map(|(f, _, l)| format!("error[E201]: {f}:{l}:1: The file {f} of {d} refers to {} of {b} (use dates), which {d} has no relationship with", shop.terms)).collect();
    want.extend(ordering.iter().map(|(f, _, l)| format!("error[E202]: {f}:{l}:1: The file {f} of {o} refers to {} (use dates), which is inside {d}", shop.ship)));
    got.sort();
    want.sort();
    assert_eq!(got, want, "{out}");
    assert_eq!(out.matches(&format!("  = {b} is downstream of {d}, the other way round: an upstream relationship lets the downstream refer to the upstream.\n")).count(), 3, "{out}");
    for note in [
        format!("  = The code a workflow is made into calls the code made of the dates file, bundled or as a Lambda function: the dates file itself, inside {d}. A dates file cannot be a published language, and one across the boundary is read only from the two's shared kernel.\n"),
        "  = The code a gate is made into calls the code made of the dates and the calendar; a dates file and a calendar cannot be a published language, and one across the boundary is read only from the two's shared kernel.\n".to_string(),
        format!("  = The code a module of kaname is made into calls the code made of the rules, the dates and the other modules it reads: the thing itself, inside {d}. One across the boundary is read from the two's shared kernel.\n"),
    ] {
        assert!(out.contains(&note), "{note}{out}");
    }
    // in the shared kernel: three more crossings, one of each language
    let t = lay(&[delivery], true);
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "check", shop.map]);
    assert_eq!((code, out.as_str()), (0, format!("{}: ok — 5 contexts, 7 relationships; 82 artifacts, each in one context; 12 crossings checked (proto 1, rulec 2, koyomi 1, dandori 6, sekisho 1, kaname 1)\n", shop.map).as_str()), "{err}");
    let (_, out, _) = ritsu_in(t.path(), &["sakai", "check", shop.map, "--lang", "ja"]);
    assert!(out.ends_with("境界を越える参照 12 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 6、sekisho 1、kaname 1）\n"), "{out}");
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "api", shop.map, "--root", "."]);
    assert_eq!(code, 0, "{err}");
    let api: serde_json::Value = serde_json::from_str(&out).unwrap();
    let mut through: Vec<(String, String, String)> = api["crossings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["to"]["path"] == shop.terms)
        .map(|c| (c["from"]["tool"].as_str().unwrap().to_string(), c["via"].as_str().unwrap().to_string(), c["allowed_by"]["relationship"].as_str().unwrap().to_string()))
        .collect();
    through.sort();
    let kernel = |tool: &str| (tool.to_string(), "use dates".to_string(), "shared_kernel".to_string());
    assert_eq!(through, [kernel("dandori"), kernel("kaname"), kernel("sekisho")]);
}

#[test]
fn a_workflow_a_gate_and_a_module_read_a_dates_file_across_a_boundary_alike() {
    let shop = Shop {
        dir: "examples/shop",
        map: "shop.ctx",
        names: ["Ordering", "Delivery", "Billing"],
        terms: "billing/payment_terms.cal",
        ship: "delivery/ship_date.cal",
        kernel: ["contexts/billing.ctx", "contexts/delivery.ctx"],
        calendar: "  koyomi \"../calendars/tokyo_business_days.cal\"\n",
    };
    a_dates_file_is_read_alike(&shop, &DELIVERY_READS, &ORDERING_READS);
}

/// The same on the example of Japanese names.
#[test]
fn a_workflow_a_gate_and_a_module_read_a_dates_file_across_a_boundary_alike_in_japanese() {
    let shop = Shop {
        dir: "examples/shop.ja",
        map: "通販.ctx",
        names: ["受注", "配送", "請求"],
        terms: "billing/支払条件.cal",
        ship: "delivery/出荷日.cal",
        kernel: ["contexts/請求.ctx", "contexts/配送.ctx"],
        calendar: "  koyomi \"../calendars/東京の営業日.cal\"\n",
    };
    a_dates_file_is_read_alike(&shop, &DELIVERY_READS_JA, &ORDERING_READS_JA);
}
