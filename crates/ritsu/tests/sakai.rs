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

// ── A dates file and an `enums` file across a boundary: a workflow's `use dates` and a rule's
// `range from koyomi` and `import enum`, held as a gate's and a module's `use dates` ──

/// Three files that read a dates file, one of each language: a workflow, a gate and a module, as
/// (path, text, the line of the `use dates`).
type Readers = [(&'static str, &'static str, usize); 3];

/// A rule that reads the same dates file with `range from koyomi`, and an `enums` file of the
/// same context with `import enum`: the `enums` file and the rule, each its path and its text, and
/// the lines of the rule's `import enum` and `range from koyomi`.
struct RuleReads {
    enums: (&'static str, &'static str),
    rule: (&'static str, &'static str),
    import_line: usize,
    range_line: usize,
}

/// Delivery's rule, reading Billing's payment methods and payment terms; Ordering's, reading
/// Delivery's regions and ship date.
const DELIVERY_RULE: RuleReads = RuleReads {
    enums: ("billing/rules/payment_methods.rule", "enums payment_methods v1\ndescription \"How a customer pays for an order\"\n\nenum payment_method = card | bank_transfer | cash_on_delivery\n"),
    rule: (
        "delivery/rules/collect.rule",
        "rule collect v1\ndescription \"Whether the courier collects the payment of a parcel, by billing's payment methods and the day billing's payment terms make it due\"\n\nimport enum payment_method from \"../../billing/rules/payment_methods.rule\"\n\ninputs\n  paid_by : payment_method\n  due     : date  range from koyomi \"../../billing/payment_terms.cal\" date payment\n\noutputs\n  collect : bool\n\ntable decide\npolicy unique\n| paid_by             | due          | -> collect : bool |\n| cash_on_delivery    | <=2027-03-31 | true              |\n| cash_on_delivery    | >2027-03-31  | false             |\n| card, bank_transfer | -            | false             |\n",
    ),
    import_line: 4,
    range_line: 8,
};
const ORDERING_RULE: RuleReads = RuleReads {
    enums: ("delivery/rules/regions.rule", "enums regions v1\ndescription \"The regions a parcel is delivered to, shared by the rules that read them\"\n\nenum region = honshu | hokkaido | okinawa\n"),
    rule: (
        "ordering/next_day.rule",
        "rule next_day v1\ndescription \"Whether an order is offered next-day delivery, by delivery's regions and the day delivery ships it\"\n\nimport enum region from \"../delivery/rules/regions.rule\"\n\ninputs\n  destination : region\n  ship        : date  range from koyomi \"../delivery/ship_date.cal\" date ship\n\noutputs\n  offered : bool\n\ntable decide\npolicy unique\n| destination       | ship         | -> offered : bool |\n| honshu            | <=2027-03-31 | true              |\n| honshu            | >2027-03-31  | false             |\n| hokkaido, okinawa | -            | false             |\n",
    ),
    import_line: 4,
    range_line: 8,
};
const DELIVERY_RULE_JA: RuleReads = RuleReads {
    enums: ("billing/rules/支払方法.rule", "enums 支払方法(payment_methods) v1\ndescription \"お客さまが注文の代金を払う方法\"\n\nenum 払い方(payment_method) = カード(card) | 振込(bank_transfer) | 代金引換(cash_on_delivery)\n"),
    rule: (
        "delivery/rules/代金の受け取り.rule",
        "rule 代金の受け取り(collect) v1\ndescription \"配達員が荷物の代金を受け取るかどうか。請求の支払方法と、請求の支払条件が決める期日による\"\n\nimport enum 払い方 from \"../../billing/rules/支払方法.rule\"\n\ninputs\n  支払(paid_by) : 払い方\n  期日(due)     : date  range from koyomi \"../../billing/支払条件.cal\" date 支払日\n\noutputs\n  受け取る(collect) : bool\n\ntable 決める(decide)\npolicy unique\n| 支払         | 期日         | -> 受け取る : bool |\n| 代金引換     | <=2027-03-31 | true              |\n| 代金引換     | >2027-03-31  | false             |\n| カード, 振込 | -            | false             |\n",
    ),
    import_line: 4,
    range_line: 8,
};
const ORDERING_RULE_JA: RuleReads = RuleReads {
    enums: ("delivery/rules/配送の地域.rule", "enums 配送の地域(delivery_regions) v1\ndescription \"荷物を届ける地域。この地域を読む規則が分け合う\"\n\nenum 地域(region) = 本州(honshu) | 北海道(hokkaido) | 沖縄(okinawa)\n"),
    rule: (
        "ordering/翌日配送.rule",
        "rule 翌日配送(next_day) v1\ndescription \"注文に翌日配送を出せるかどうか。配送の地域と、配送が出荷する日で決める\"\n\nimport enum 地域 from \"../delivery/rules/配送の地域.rule\"\n\ninputs\n  届け先(destination) : 地域\n  出す日(ship)        : date  range from koyomi \"../delivery/出荷日.cal\" date 出荷日\n\noutputs\n  出せる(offered) : bool\n\ntable 決める(decide)\npolicy unique\n| 届け先       | 出す日       | -> 出せる : bool |\n| 本州         | <=2027-03-31 | true            |\n| 本州         | >2027-03-31  | false           |\n| 北海道, 沖縄 | -            | false           |\n",
    ),
    import_line: 4,
    range_line: 8,
};

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

/// sakai holds a workflow's `use dates` and a rule's `range from koyomi` and `import enum` to the
/// map as it holds a gate's and a module's `use dates` (sakai's DESIGN 19, 20): Delivery's three
/// and its rule read Billing's payment terms, and the rule Billing's payment methods too, and
/// Billing is Delivery's customer, the other way round (E201, the same words for each but the
/// file, the line and how it refers); Ordering's read Delivery's ship date and regions, a partner's
/// inside (E202, the same words, each with the note of its language); and with the payment terms
/// and methods in the shared kernel of Billing and Delivery, Delivery's pass and are counted by
/// their languages, the rule's two as rulec's.
fn read_alike(shop: &Shop, delivery: (&Readers, &RuleReads), ordering: (&Readers, &RuleReads)) {
    let [o, d, b] = shop.names;
    let lay = |readers: &[(&Readers, &RuleReads)], kernel: bool| {
        let t = TempDir::new("sakai-dates");
        ritsu_testkit::tmp::copy_dir(&sakai_dir().join(shop.dir), t.path());
        for (files, rule) in readers {
            for (f, body, _) in files.iter() {
                t.write(f, body);
            }
            for (f, body) in [rule.enums, rule.rule] {
                t.write(f, body);
            }
        }
        if kernel {
            let listed = format!("{}  koyomi \"../{}\"\n  rulec \"../{}\"\n", shop.calendar, shop.terms, delivery.1.enums.0);
            for f in shop.kernel {
                let p = t.path().join(f);
                let s = std::fs::read_to_string(&p).unwrap();
                assert!(s.contains(shop.calendar), "{f} has no {:?}", shop.calendar);
                std::fs::write(&p, s.replacen(shop.calendar, &listed, 1)).unwrap();
            }
        }
        t
    };
    let t = lay(&[delivery, ordering], false);
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "check", shop.map]);
    assert_eq!(code, 1, "{out}{err}");
    let mut got: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    let e201 = |f: &str, l: usize, to: &str, via: &str| format!("error[E201]: {f}:{l}:1: The file {f} of {d} refers to {to} of {b} ({via}), which {d} has no relationship with");
    let e202 = |f: &str, l: usize, to: &str, via: &str| format!("error[E202]: {f}:{l}:1: The file {f} of {o} refers to {to} ({via}), which is inside {d}");
    let mut want: Vec<String> = delivery.0.iter().map(|(f, _, l)| e201(f, *l, shop.terms, "use dates")).collect();
    let (dr, or) = (delivery.1, ordering.1);
    want.extend([e201(dr.rule.0, dr.range_line, shop.terms, "range from koyomi"), e201(dr.rule.0, dr.import_line, dr.enums.0, "import enum")]);
    want.extend(ordering.0.iter().map(|(f, _, l)| e202(f, *l, shop.ship, "use dates")));
    want.extend([e202(or.rule.0, or.range_line, shop.ship, "range from koyomi"), e202(or.rule.0, or.import_line, or.enums.0, "import enum")]);
    got.sort();
    want.sort();
    assert_eq!(got, want, "{out}");
    assert_eq!(out.matches(&format!("  = {b} is downstream of {d}, the other way round: an upstream relationship lets the downstream refer to the upstream.\n")).count(), 5, "{out}");
    for note in [
        format!("  = The code a workflow is made into calls the code made of the dates file, bundled or as a Lambda function: the dates file itself, inside {d}. A dates file cannot be a published language, and one across the boundary is read only from the two's shared kernel.\n"),
        "  = The code a gate is made into calls the code made of the dates and the calendar; a dates file and a calendar cannot be a published language, and one across the boundary is read only from the two's shared kernel.\n".to_string(),
        format!("  = The code a module of kaname is made into calls the code made of the rules, the dates and the other modules it reads: the thing itself, inside {d}. One across the boundary is read from the two's shared kernel.\n"),
        format!("  = A `range from koyomi` holds the rule's input to the days a date of the dates file comes to: rulec checks the rule on those days, and the code it is made into takes no other day. That is the dates file itself, inside {d}. A dates file cannot be a published language, and one across the boundary is read only from the two's shared kernel.\n"),
        format!("  = An `import enum` reads an enum whole from the `enums` file, and the code a rule is made into imports the module made of that file: the `enums` file itself, inside {d}. An enum across the boundary is read from an `enums` file in the two's shared kernel, or {d} puts it in a `.proto` of a published language and the rule takes it in with `import proto`.\n"),
    ] {
        assert!(out.contains(&note), "{note}{out}");
    }
    // in the shared kernel: five more crossings, a workflow's, a gate's, a module's and the rule's two
    let t = lay(&[delivery], true);
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "check", shop.map]);
    assert_eq!((code, out.as_str()), (0, format!("{}: ok — 5 contexts, 7 relationships; 84 artifacts, each in one context; 14 crossings checked (proto 1, rulec 4, koyomi 1, dandori 6, sekisho 1, kaname 1)\n", shop.map).as_str()), "{err}");
    let (_, out, _) = ritsu_in(t.path(), &["sakai", "check", shop.map, "--lang", "ja"]);
    assert!(out.ends_with("境界を越える参照 14 件を確かめた（proto 1、rulec 4、koyomi 1、dandori 6、sekisho 1、kaname 1）\n"), "{out}");
    let (code, out, err) = ritsu_in(t.path(), &["sakai", "api", shop.map, "--root", "."]);
    assert_eq!(code, 0, "{err}");
    let api: serde_json::Value = serde_json::from_str(&out).unwrap();
    let through = |to: &str| -> Vec<(String, String, String)> {
        let mut v: Vec<(String, String, String)> = api["crossings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["to"]["path"] == to)
            .map(|c| (c["from"]["tool"].as_str().unwrap().to_string(), c["via"].as_str().unwrap().to_string(), c["allowed_by"]["relationship"].as_str().unwrap().to_string()))
            .collect();
        v.sort();
        v
    };
    let kernel = |tool: &str, via: &str| (tool.to_string(), via.to_string(), "shared_kernel".to_string());
    assert_eq!(through(shop.terms), [kernel("dandori", "use dates"), kernel("kaname", "use dates"), kernel("rulec", "range from koyomi"), kernel("sekisho", "use dates")]);
    assert_eq!(through(dr.enums.0), [kernel("rulec", "import enum")]);
}

#[test]
fn a_workflow_a_gate_a_module_and_a_rule_read_across_a_boundary_alike() {
    let shop = Shop {
        dir: "examples/shop",
        map: "shop.ctx",
        names: ["Ordering", "Delivery", "Billing"],
        terms: "billing/payment_terms.cal",
        ship: "delivery/ship_date.cal",
        kernel: ["contexts/billing.ctx", "contexts/delivery.ctx"],
        calendar: "  koyomi \"../calendars/tokyo_business_days.cal\"\n",
    };
    read_alike(&shop, (&DELIVERY_READS, &DELIVERY_RULE), (&ORDERING_READS, &ORDERING_RULE));
}

/// The same on the example of Japanese names.
#[test]
fn a_workflow_a_gate_a_module_and_a_rule_read_across_a_boundary_alike_in_japanese() {
    let shop = Shop {
        dir: "examples/shop.ja",
        map: "通販.ctx",
        names: ["受注", "配送", "請求"],
        terms: "billing/支払条件.cal",
        ship: "delivery/出荷日.cal",
        kernel: ["contexts/請求.ctx", "contexts/配送.ctx"],
        calendar: "  koyomi \"../calendars/東京の営業日.cal\"\n",
    };
    read_alike(&shop, (&DELIVERY_READS_JA, &DELIVERY_RULE_JA), (&ORDERING_READS_JA, &ORDERING_RULE_JA));
}
