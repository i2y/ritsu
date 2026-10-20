//! A workflow's `use dates` (DESIGN 19): the code a workflow is made into calls the code made of
//! the dates file it reads, bundled or as a Lambda function, so a workflow reads a dates file
//! across a boundary only from the two's shared kernel, as a gate and a module do. On both
//! examples, `shop` and `shop.ja`, where Ordering and Delivery are partners, Billing is the
//! customer of Delivery, and the two share a kernel that lists their calendar.

mod common;

use ritsu_base::text::Lang;
use std::path::Path;

/// One example, the names its diagnostics say, and the files the tests lay over it, in its
/// language.
struct Example {
    dir: &'static str,
    map: &'static str,
    ordering: &'static str,
    delivery: &'static str,
    billing: &'static str,
    /// Ordering's workflow that reads Delivery's ship date: its path and its text.
    notice: (&'static str, &'static str),
    /// Delivery's workflow that reads Billing's payment terms, by a Lambda function.
    collect: (&'static str, &'static str),
    /// Billing's workflow that reads Billing's own payment terms.
    remind: (&'static str, &'static str),
    /// The two context files that write the shared kernel of Billing and Delivery, the line of
    /// the calendar both list, and the line of the payment terms to list after it.
    kernel: [&'static str; 2],
    calendar: &'static str,
    terms: &'static str,
}

const EN: Example = Example {
    dir: "examples/shop",
    map: "shop.ctx",
    ordering: "Ordering",
    delivery: "Delivery",
    billing: "Billing",
    notice: (
        "ordering/ship_notice.flow",
        "workflow ship_notice v1\ndescription \"Tells the customer the day an order ships, by delivery's ship date\"\n\nuse dates shipping from \"../delivery/ship_date.cal\"\n\ninputs\n  order   : string\n  ordered : date\n\ntask tell(order: string, day: date)\n  key\n\nflow\n  let ship = shipping.ship(ordered: ordered)\n  tell(order: order, day: ship.day)\n",
    ),
    collect: (
        "delivery/collect.flow",
        "workflow collect v1\ndescription \"Collects the payment of a cash-on-delivery parcel on the day billing's payment terms make it due\"\n\nuse dates terms from \"../billing/payment_terms.cal\"\n  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:payment-terms\"\n\ninputs\n  parcel   : string\n  received : date\n\ntask collect(parcel: string, due: date)\n  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:collect\"\n  key\n\nflow\n  let due = terms.payment(received: received)\n  collect(parcel: parcel, due: due.day)\n",
    ),
    remind: (
        "billing/remind.flow",
        "workflow remind v1\ndescription \"Reminds the customer of the day an invoice's payment is due\"\n\nuse dates terms from \"payment_terms.cal\"\n\ninputs\n  invoice  : string\n  received : date\n\ntask remind(invoice: string, due: date)\n  key\n\nflow\n  let due = terms.payment(received: received)\n  remind(invoice: invoice, due: due.day)\n",
    ),
    kernel: ["contexts/billing.ctx", "contexts/delivery.ctx"],
    calendar: "  koyomi \"../calendars/tokyo_business_days.cal\"\n",
    terms: "  koyomi \"../billing/payment_terms.cal\"\n",
};

const JA: Example = Example {
    dir: "examples/shop.ja",
    map: "通販.ctx",
    ordering: "受注",
    delivery: "配送",
    billing: "請求",
    notice: (
        "ordering/出荷の知らせ.flow",
        "workflow 出荷の知らせ v1\ndescription \"注文が出荷される日を、配送の出荷日のとおりにお客さまに知らせる\"\n\nuse dates 出荷 from \"../delivery/出荷日.cal\"\n\ninputs\n  注文   : string\n  受注日 : date\n\ntask 知らせる(注文: string, 日: date)\n  key\n\nflow\n  let 出す日 = 出荷.出荷日(受注日: 受注日)\n  知らせる(注文: 注文, 日: 出す日.day)\n",
    ),
    collect: (
        "delivery/代金の受け取り.flow",
        "workflow 代金の受け取り v1\ndescription \"代金引換の荷物の代金を、請求の支払条件で決まる期日に受け取る\"\n\nuse dates 支払条件 from \"../billing/支払条件.cal\"\n  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:payment-terms\"\n\ninputs\n  荷物   : string\n  受領日 : date\n\ntask 受け取る(荷物: string, 期日: date)\n  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:collect\"\n  key\n\nflow\n  let 期日 = 支払条件.支払日(受領日: 受領日)\n  受け取る(荷物: 荷物, 期日: 期日.day)\n",
    ),
    remind: (
        "billing/支払の知らせ.flow",
        "workflow 支払の知らせ v1\ndescription \"請求の支払期日をお客さまに知らせる\"\n\nuse dates 支払条件 from \"支払条件.cal\"\n\ninputs\n  請求書 : string\n  受領日 : date\n\ntask 知らせる(請求書: string, 期日: date)\n  key\n\nflow\n  let 期日 = 支払条件.支払日(受領日: 受領日)\n  知らせる(請求書: 請求書, 期日: 期日.day)\n",
    ),
    kernel: ["contexts/請求.ctx", "contexts/配送.ctx"],
    calendar: "  koyomi \"../calendars/東京の営業日.cal\"\n",
    terms: "  koyomi \"../billing/支払条件.cal\"\n",
};

/// A copy of the example with each file written whole and each edit made.
fn laid_out(x: &Example, files: &[(&str, &str)], edits: &[(&str, &str, &str)]) -> common::TempDir {
    let t = common::TempDir::new("dates");
    common::copy_dir(Path::new(x.dir), t.path());
    for (f, body) in files {
        t.write(f, body);
    }
    for (f, old, new) in edits {
        let p = t.path().join(f);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{f} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    }
    t
}

/// The codes the check of the map gives, what it prints in English and in Japanese, and the
/// outcome of the map.
fn said(x: &Example, t: &common::TempDir) -> (Vec<String>, String, String, sakai::check::Outcome) {
    let mut os = sakai::check::check_args_with(t.path(), &[x.map.to_string()], &common::suite()).unwrap();
    let codes = common::codes(&os).iter().map(|c| c.to_string()).collect();
    let (en, ja) = (common::text(&os, Lang::En), common::text(&os, Lang::Ja));
    (codes, en, ja, os.remove(0))
}

/// The edits that list the payment terms in the shared kernel of Billing and Delivery, on both
/// sides, after the calendar.
fn in_the_kernel(x: &Example) -> Vec<(&'static str, &'static str, String)> {
    x.kernel.iter().map(|f| (*f, x.calendar, format!("{}{}", x.calendar, x.terms))).collect()
}

/// Ordering's workflow reads Delivery's ship date: the two are partners, and a partnership lets
/// either side use the other's published languages, which a dates file cannot be in (E202).
#[test]
fn a_workflow_that_reads_a_dates_file_inside_a_partner_is_e202() {
    for x in [EN, JA] {
        let t = laid_out(&x, &[x.notice], &[]);
        let (codes, en, ja, _) = said(&x, &t);
        assert_eq!(codes, ["E202"], "{}: {en}", x.dir);
        let (flow, ship) = (x.notice.0, if x.dir.ends_with(".ja") { "delivery/出荷日.cal" } else { "delivery/ship_date.cal" });
        let (o, d) = (x.ordering, x.delivery);
        assert!(en.starts_with(&format!("error[E202]: {flow}:4:1: The file {flow} of {o} refers to {ship} (use dates), which is inside {d}\n")), "{en}");
        assert!(en.contains(&format!("  = The code a workflow is made into calls the code made of the dates file, bundled or as a Lambda function: the dates file itself, inside {d}. A dates file cannot be a published language, and one across the boundary is read only from the two's shared kernel.\n")), "{en}");
        assert!(en.contains(&format!("      {d}  koyomi \"{ship}\"")), "{en}");
        assert!(ja.starts_with(&format!("エラー[E202]: {flow}:4:1: 「{o}」の {flow} が、「{d}」の内側の {ship} を参照しています（use dates）\n")), "{ja}");
        assert!(ja.contains(&format!("  = ワークフローから生成したコードは、日付のファイルから生成したコードを、同梱して呼ぶか Lambda の関数として呼びます。使うのは日付のファイルそのもの（「{d}」の内側）です。日付のファイルは公表された言語にできないので、境界の向こうのものは、二つの共有カーネルに並べたときだけ読めます。\n")), "{ja}");
    }
}

/// Delivery's workflow reads Billing's payment terms, by a Lambda function: Billing is Delivery's
/// customer, the other way round (E201); listed in the two's shared kernel, it passes, and the
/// summary counts it as dandori's. The api says how it refers and what allows it.
#[test]
fn a_workflow_reads_a_dates_file_across_a_boundary_from_the_shared_kernel_only() {
    for x in [EN, JA] {
        let terms = if x.dir.ends_with(".ja") { "billing/支払条件.cal" } else { "billing/payment_terms.cal" };
        let (flow, d, b) = (x.collect.0, x.delivery, x.billing);
        let t = laid_out(&x, &[x.collect], &[]);
        let (codes, en, ja, _) = said(&x, &t);
        assert_eq!(codes, ["E201"], "{}: {en}", x.dir);
        assert!(en.starts_with(&format!("error[E201]: {flow}:4:1: The file {flow} of {d} refers to {terms} of {b} (use dates … lambda), which {d} has no relationship with\n")), "{en}");
        assert!(en.contains(&format!("  = {b} is downstream of {d}, the other way round: an upstream relationship lets the downstream refer to the upstream.\n")), "{en}");
        assert!(ja.starts_with(&format!("エラー[E201]: {flow}:4:1: 「{d}」の {flow} が、関係の無い「{b}」の {terms} を参照しています（use dates … lambda）\n")), "{ja}");
        // in the shared kernel: one more crossing, a workflow's
        let edits = in_the_kernel(&x);
        let edits: Vec<(&str, &str, &str)> = edits.iter().map(|(f, o, n)| (*f, *o, n.as_str())).collect();
        let t = laid_out(&x, &[x.collect], &edits);
        let (codes, en, _, o) = said(&x, &t);
        assert!(codes.is_empty(), "{}: {en}", x.dir);
        let summary = o.summary.clone().expect("a summary");
        assert_eq!(summary.en, "5 contexts, 7 relationships; 80 artifacts, each in one context; 10 crossings checked (proto 1, rulec 2, koyomi 1, dandori 6)");
        assert_eq!(summary.ja, "コンテキスト 5、関係 7。成果物 80 件は、どれも一つのコンテキストに属する。境界を越える参照 10 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 6）");
        let api = sakai::api::api(o.checked.as_ref().unwrap());
        let crossing = api["crossings"].as_array().unwrap().iter().find(|c| c["from"]["path"] == flow).expect("the workflow's crossing");
        assert_eq!(crossing["to"]["text"], format!("koyomi \"{terms}\""));
        assert_eq!(crossing["via"], "use dates … lambda");
        assert_eq!(crossing["allowed_by"]["relationship"], "shared_kernel");
        assert_eq!((crossing["from_context"].as_str(), crossing["to_context"].as_str()), (Some(d), Some(b)));
        // the JSON of the check says the same summary
        assert_eq!(sakai::check::to_json(&o, Lang::En)["summary"], summary.en.as_str());
    }
}

/// Billing's workflow reads Billing's own payment terms: no boundary, nothing to say, and no
/// crossing to count.
#[test]
fn a_workflow_that_reads_its_own_contexts_dates_file_crosses_nothing() {
    for x in [EN, JA] {
        let t = laid_out(&x, &[x.remind], &[]);
        let (codes, en, _, o) = said(&x, &t);
        assert!(codes.is_empty(), "{}: {en}", x.dir);
        assert_eq!(o.summary.unwrap().en, "5 contexts, 7 relationships; 80 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)");
    }
}

/// A workflow that reads a dates file outside the map's scope is told so (E103), as one that reads
/// a rule there is.
#[test]
fn a_dates_file_outside_the_maps_scope_is_e103() {
    for x in [EN, JA] {
        let (path, body) = x.remind;
        let body = body.replace("from \"payment_terms.cal\"", "from \"../archive/old_terms.cal\"").replace("from \"支払条件.cal\"", "from \"../archive/old_terms.cal\"");
        let src = if x.dir.ends_with(".ja") { "billing/支払条件.cal" } else { "billing/payment_terms.cal" };
        let old = std::fs::read_to_string(Path::new(x.dir).join(src)).unwrap();
        let t = laid_out(&x, &[(path, &body), ("archive/old_terms.cal", &old)], &[(x.map, "covers \".\"\n", "covers \".\"\nexcept \"archive\"\n")]);
        let (codes, en, ja, _) = said(&x, &t);
        assert_eq!(codes, ["E103"], "{}: {en}", x.dir);
        assert!(en.starts_with(&format!("error[E103]: {path}:4:1: The file {path} refers to koyomi \"archive/old_terms.cal\", which is outside the map's scope (use dates)\n")), "{en}");
        assert!(ja.starts_with(&format!("エラー[E103]: {path}:4:1: {path} が、地図の範囲の外の koyomi \"archive/old_terms.cal\" を参照しています（use dates）\n")), "{ja}");
    }
}
