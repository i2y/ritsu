//! A rule's `import enum` and `range from koyomi` (DESIGN 20): the code a rule is made into
//! imports the module made of the `enums` file it reads an enum from, and rulec checks a rule on
//! the days a date of a dates file comes to, so a rule reads an `enums` file or a dates file
//! across a boundary only from the two's shared kernel, as a workflow, a gate and a module read a
//! dates file. On both examples, `shop` and `shop.ja`, where Ordering and Delivery are partners,
//! Billing is the customer of Delivery, and the two share a kernel that lists their calendar.

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
    /// Delivery's `enums` file of regions, and Billing's of payment methods: each its path and its
    /// text.
    regions: (&'static str, &'static str),
    methods: (&'static str, &'static str),
    /// The enums of the two files, and the dates of the dates files the rules read: Delivery's
    /// ship date and Billing's payment terms (its path, from the root, and the date).
    region: &'static str,
    method: &'static str,
    ship: (&'static str, &'static str),
    terms: (&'static str, &'static str),
    /// Ordering's rule that reads Delivery's regions (line 4) and ship date (line 8).
    next_day: (&'static str, &'static str),
    /// Delivery's rule that reads Billing's payment methods (line 4) and payment terms (line 8).
    collect: (&'static str, &'static str),
    /// Billing's rule that reads Billing's own payment methods and payment terms.
    reminder: (&'static str, &'static str),
    /// The two context files that write the shared kernel of Billing and Delivery, the line of
    /// the calendar both list, and the lines of the payment terms and methods to list after it.
    kernel: [&'static str; 2],
    calendar: &'static str,
    listed: &'static str,
}

const EN: Example = Example {
    dir: "examples/shop",
    map: "shop.ctx",
    ordering: "Ordering",
    delivery: "Delivery",
    billing: "Billing",
    regions: ("delivery/rules/regions.rule", "enums regions v1\ndescription \"The regions a parcel is delivered to, shared by the rules that read them\"\n\nenum region = honshu | hokkaido | okinawa\n"),
    methods: ("billing/rules/payment_methods.rule", "enums payment_methods v1\ndescription \"How a customer pays for an order\"\n\nenum payment_method = card | bank_transfer | cash_on_delivery\n"),
    region: "region",
    method: "payment_method",
    ship: ("delivery/ship_date.cal", "ship"),
    terms: ("billing/payment_terms.cal", "payment"),
    next_day: (
        "ordering/next_day.rule",
        "rule next_day v1\ndescription \"Whether an order is offered next-day delivery, by delivery's regions and the day delivery ships it\"\n\nimport enum region from \"../delivery/rules/regions.rule\"\n\ninputs\n  destination : region\n  ship        : date  range from koyomi \"../delivery/ship_date.cal\" date ship\n\noutputs\n  offered : bool\n\ntable decide\npolicy unique\n| destination       | ship         | -> offered : bool |\n| honshu            | <=2027-03-31 | true              |\n| honshu            | >2027-03-31  | false             |\n| hokkaido, okinawa | -            | false             |\n",
    ),
    collect: (
        "delivery/rules/collect.rule",
        "rule collect v1\ndescription \"Whether the courier collects the payment of a parcel, by billing's payment methods and the day billing's payment terms make it due\"\n\nimport enum payment_method from \"../../billing/rules/payment_methods.rule\"\n\ninputs\n  paid_by : payment_method\n  due     : date  range from koyomi \"../../billing/payment_terms.cal\" date payment\n\noutputs\n  collect : bool\n\ntable decide\npolicy unique\n| paid_by             | due          | -> collect : bool |\n| cash_on_delivery    | <=2027-03-31 | true              |\n| cash_on_delivery    | >2027-03-31  | false             |\n| card, bank_transfer | -            | false             |\n",
    ),
    reminder: (
        "billing/rules/reminder.rule",
        "rule reminder v1\ndescription \"Whether billing reminds the customer of a payment before its due day, by the payment method and the day the payment terms make it due\"\n\nimport enum payment_method from \"payment_methods.rule\"\n\ninputs\n  paid_by : payment_method\n  due     : date  range from koyomi \"../payment_terms.cal\" date payment\n\noutputs\n  remind : bool\n\ntable decide\npolicy unique\n| paid_by                | due          | -> remind : bool |\n| bank_transfer          | <=2027-03-31 | true             |\n| bank_transfer          | >2027-03-31  | false            |\n| card, cash_on_delivery | -            | false            |\n",
    ),
    kernel: ["contexts/billing.ctx", "contexts/delivery.ctx"],
    calendar: "  koyomi \"../calendars/tokyo_business_days.cal\"\n",
    listed: "  koyomi \"../billing/payment_terms.cal\"\n  rulec \"../billing/rules/payment_methods.rule\"\n",
};

const JA: Example = Example {
    dir: "examples/shop.ja",
    map: "通販.ctx",
    ordering: "受注",
    delivery: "配送",
    billing: "請求",
    regions: ("delivery/rules/配送の地域.rule", "enums 配送の地域(delivery_regions) v1\ndescription \"荷物を届ける地域。この地域を読む規則が分け合う\"\n\nenum 地域(region) = 本州(honshu) | 北海道(hokkaido) | 沖縄(okinawa)\n"),
    methods: ("billing/rules/支払方法.rule", "enums 支払方法(payment_methods) v1\ndescription \"お客さまが注文の代金を払う方法\"\n\nenum 払い方(payment_method) = カード(card) | 振込(bank_transfer) | 代金引換(cash_on_delivery)\n"),
    region: "地域",
    method: "払い方",
    ship: ("delivery/出荷日.cal", "出荷日"),
    terms: ("billing/支払条件.cal", "支払日"),
    next_day: (
        "ordering/翌日配送.rule",
        "rule 翌日配送(next_day) v1\ndescription \"注文に翌日配送を出せるかどうか。配送の地域と、配送が出荷する日で決める\"\n\nimport enum 地域 from \"../delivery/rules/配送の地域.rule\"\n\ninputs\n  届け先(destination) : 地域\n  出す日(ship)        : date  range from koyomi \"../delivery/出荷日.cal\" date 出荷日\n\noutputs\n  出せる(offered) : bool\n\ntable 決める(decide)\npolicy unique\n| 届け先       | 出す日       | -> 出せる : bool |\n| 本州         | <=2027-03-31 | true            |\n| 本州         | >2027-03-31  | false           |\n| 北海道, 沖縄 | -            | false           |\n",
    ),
    collect: (
        "delivery/rules/代金の受け取り.rule",
        "rule 代金の受け取り(collect) v1\ndescription \"配達員が荷物の代金を受け取るかどうか。請求の支払方法と、請求の支払条件が決める期日による\"\n\nimport enum 払い方 from \"../../billing/rules/支払方法.rule\"\n\ninputs\n  支払(paid_by) : 払い方\n  期日(due)     : date  range from koyomi \"../../billing/支払条件.cal\" date 支払日\n\noutputs\n  受け取る(collect) : bool\n\ntable 決める(decide)\npolicy unique\n| 支払         | 期日         | -> 受け取る : bool |\n| 代金引換     | <=2027-03-31 | true              |\n| 代金引換     | >2027-03-31  | false             |\n| カード, 振込 | -            | false             |\n",
    ),
    reminder: (
        "billing/rules/支払の知らせ.rule",
        "rule 支払の知らせ(reminder) v1\ndescription \"期日の前に、お客さまに支払を知らせるかどうか。支払方法と、支払条件が決める期日による\"\n\nimport enum 払い方 from \"支払方法.rule\"\n\ninputs\n  支払(paid_by) : 払い方\n  期日(due)     : date  range from koyomi \"../支払条件.cal\" date 支払日\n\noutputs\n  知らせる(remind) : bool\n\ntable 決める(decide)\npolicy unique\n| 支払             | 期日         | -> 知らせる : bool |\n| 振込             | <=2027-03-31 | true              |\n| 振込             | >2027-03-31  | false             |\n| カード, 代金引換 | -            | false             |\n",
    ),
    kernel: ["contexts/請求.ctx", "contexts/配送.ctx"],
    calendar: "  koyomi \"../calendars/東京の営業日.cal\"\n",
    listed: "  koyomi \"../billing/支払条件.cal\"\n  rulec \"../billing/rules/支払方法.rule\"\n",
};

/// A copy of the example with each file written whole and each edit made.
fn laid_out(x: &Example, files: &[(&str, &str)], edits: &[(&str, &str, &str)]) -> common::TempDir {
    let t = common::TempDir::new("rule-reads");
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

/// Ordering's rule reads Delivery's regions and ship date: the two are partners, and a
/// partnership lets either side use the other's published languages, which neither file is in
/// (E202, one for each line, each with the note of how a rule reads it).
#[test]
fn a_rule_that_reads_an_enums_file_or_a_dates_file_inside_a_partner_is_e202() {
    for x in [EN, JA] {
        let t = laid_out(&x, &[x.regions, x.next_day], &[]);
        let (codes, en, ja, _) = said(&x, &t);
        assert_eq!(codes, ["E202", "E202"], "{}: {en}", x.dir);
        let (rule, regions, ship) = (x.next_day.0, x.regions.0, x.ship.0);
        let (o, d) = (x.ordering, x.delivery);
        assert!(en.starts_with(&format!("error[E202]: {rule}:4:1: The file {rule} of {o} refers to {regions} (import enum), which is inside {d}\n")), "{en}");
        assert!(en.contains(&format!("error[E202]: {rule}:8:1: The file {rule} of {o} refers to {ship} (range from koyomi), which is inside {d}\n")), "{en}");
        assert!(en.contains(&format!("  = An `import enum` reads an enum whole from the `enums` file, and the code a rule is made into imports the module made of that file: the `enums` file itself, inside {d}. An enum across the boundary is read from an `enums` file in the two's shared kernel, or {d} puts it in a `.proto` of a published language and the rule takes it in with `import proto`.\n")), "{en}");
        assert!(en.contains(&format!("  = A `range from koyomi` holds the rule's input to the days a date of the dates file comes to: rulec checks the rule on those days, and the code it is made into takes no other day. That is the dates file itself, inside {d}. A dates file cannot be a published language, and one across the boundary is read only from the two's shared kernel.\n")), "{en}");
        assert!(en.contains(&format!("      {d}  rulec \"{regions}\" enum {}", x.region)), "{en}");
        assert!(en.contains(&format!("      {d}  koyomi \"{ship}\" date {}", x.ship.1)), "{en}");
        assert!(ja.starts_with(&format!("エラー[E202]: {rule}:4:1: 「{o}」の {rule} が、「{d}」の内側の {regions} を参照しています（import enum）\n")), "{ja}");
        assert!(ja.contains(&format!("エラー[E202]: {rule}:8:1: 「{o}」の {rule} が、「{d}」の内側の {ship} を参照しています（range from koyomi）\n")), "{ja}");
        assert!(ja.contains(&format!("  = `import enum` は、`enums` のファイルの列挙を丸ごと読みます。規則から生成したコードはそのファイルから生成したモジュールを読み込むので、使うのは `enums` のファイルそのもの（「{d}」の内側）です。境界の向こうの列挙は、`enums` のファイルを二つの共有カーネルに並べて読むか、「{d}」に公表された言語の proto に置いてもらい、規則から `import proto` で取り込んでください。\n")), "{ja}");
        assert!(ja.contains(&format!("  = `range from koyomi` は、日付のファイルの日付がとる日を、規則の入力の範囲にします。rulec はその日の上で規則を確かめ、規則から生成したコードもほかの日を受け付けません。使うのは日付のファイルそのもの（「{d}」の内側）です。日付のファイルは公表された言語にできないので、境界の向こうのものは、二つの共有カーネルに並べたときだけ読めます。\n")), "{ja}");
    }
}

/// Delivery's rule reads Billing's payment methods and payment terms: Billing is Delivery's
/// customer, the other way round (E201, for each line); listed in the two's shared kernel, both
/// pass, and the summary counts them as rulec's. The api says how each refers and what allows it.
#[test]
fn a_rule_reads_across_a_boundary_from_the_shared_kernel_only() {
    for x in [EN, JA] {
        let (rule, methods, terms) = (x.collect.0, x.methods.0, x.terms.0);
        let (d, b) = (x.delivery, x.billing);
        let t = laid_out(&x, &[x.methods, x.collect], &[]);
        let (codes, en, ja, _) = said(&x, &t);
        assert_eq!(codes, ["E201", "E201"], "{}: {en}", x.dir);
        assert!(en.starts_with(&format!("error[E201]: {rule}:4:1: The file {rule} of {d} refers to {methods} of {b} (import enum), which {d} has no relationship with\n")), "{en}");
        assert!(en.contains(&format!("error[E201]: {rule}:8:1: The file {rule} of {d} refers to {terms} of {b} (range from koyomi), which {d} has no relationship with\n")), "{en}");
        assert_eq!(en.matches(&format!("  = {b} is downstream of {d}, the other way round: an upstream relationship lets the downstream refer to the upstream.\n")).count(), 2, "{en}");
        assert!(ja.starts_with(&format!("エラー[E201]: {rule}:4:1: 「{d}」の {rule} が、関係の無い「{b}」の {methods} を参照しています（import enum）\n")), "{ja}");
        assert!(ja.contains(&format!("エラー[E201]: {rule}:8:1: 「{d}」の {rule} が、関係の無い「{b}」の {terms} を参照しています（range from koyomi）\n")), "{ja}");
        // in the shared kernel: two more crossings, both rulec's
        let listed = format!("{}{}", x.calendar, x.listed);
        let edits: Vec<(&str, &str, &str)> = x.kernel.iter().map(|f| (*f, x.calendar, listed.as_str())).collect();
        let t = laid_out(&x, &[x.methods, x.collect], &edits);
        let (codes, en, _, o) = said(&x, &t);
        assert!(codes.is_empty(), "{}: {en}", x.dir);
        let summary = o.summary.clone().expect("a summary");
        assert_eq!(summary.en, "5 contexts, 7 relationships; 81 artifacts, each in one context; 11 crossings checked (proto 1, rulec 4, koyomi 1, dandori 5)");
        assert_eq!(summary.ja, "コンテキスト 5、関係 7。成果物 81 件は、どれも一つのコンテキストに属する。境界を越える参照 11 件を確かめた（proto 1、rulec 4、koyomi 1、dandori 5）");
        let api = sakai::api::api(o.checked.as_ref().unwrap());
        let from_rule: Vec<&serde_json::Value> = api["crossings"].as_array().unwrap().iter().filter(|c| c["from"]["path"] == rule).collect();
        let got: Vec<(String, String, String, String)> = from_rule
            .iter()
            .map(|c| (c["to"]["text"].as_str().unwrap().to_string(), c["via"].as_str().unwrap().to_string(), c["allowed_by"]["relationship"].as_str().unwrap().to_string(), format!("{} {}", c["from_context"].as_str().unwrap(), c["to_context"].as_str().unwrap())))
            .collect();
        let pair = format!("{d} {b}");
        assert_eq!(
            got,
            [
                (format!("rulec \"{methods}\" enum {}", x.method), "import enum".to_string(), "shared_kernel".to_string(), pair.clone()),
                (format!("koyomi \"{terms}\" date {}", x.terms.1), "range from koyomi".to_string(), "shared_kernel".to_string(), pair.clone()),
            ]
        );
        // the JSON of the check says the same summary
        assert_eq!(sakai::check::to_json(&o, Lang::En)["summary"], summary.en.as_str());
    }
}

/// Billing's rule reads Billing's own payment methods and payment terms: no boundary, nothing to
/// say, and no crossing to count.
#[test]
fn a_rule_that_reads_its_own_contexts_files_crosses_nothing() {
    for x in [EN, JA] {
        let t = laid_out(&x, &[x.methods, x.reminder], &[]);
        let (codes, en, _, o) = said(&x, &t);
        assert!(codes.is_empty(), "{}: {en}", x.dir);
        assert_eq!(o.summary.unwrap().en, "5 contexts, 7 relationships; 81 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)");
    }
}

/// A rule that reads an `enums` file or a dates file outside the map's scope is told so (E103), as
/// a workflow that reads a dates file there is.
#[test]
fn an_enums_file_or_a_dates_file_outside_the_maps_scope_is_e103() {
    for x in [EN, JA] {
        let (path, body) = x.reminder;
        let (methods_file, terms_file) = (Path::new(x.methods.0).file_name().unwrap().to_string_lossy().into_owned(), Path::new(x.terms.0).file_name().unwrap().to_string_lossy().into_owned());
        let body = body.replace(&format!("from \"{methods_file}\""), "from \"../../archive/old_methods.rule\"").replace(&format!("\"../{terms_file}\""), "\"../../archive/old_terms.cal\"");
        let old_terms = std::fs::read_to_string(Path::new(x.dir).join(x.terms.0)).unwrap();
        let t = laid_out(&x, &[(path, &body), ("archive/old_methods.rule", x.methods.1), ("archive/old_terms.cal", &old_terms)], &[(x.map, "covers \".\"\n", "covers \".\"\nexcept \"archive\"\n")]);
        let (codes, en, ja, _) = said(&x, &t);
        assert_eq!(codes, ["E103", "E103"], "{}: {en}", x.dir);
        let (m, d) = (x.method, x.terms.1);
        assert!(en.starts_with(&format!("error[E103]: {path}:4:1: The file {path} refers to rulec \"archive/old_methods.rule\" enum {m}, which is outside the map's scope (import enum)\n")), "{en}");
        assert!(en.contains(&format!("error[E103]: {path}:8:1: The file {path} refers to koyomi \"archive/old_terms.cal\" date {d}, which is outside the map's scope (range from koyomi)\n")), "{en}");
        assert!(ja.starts_with(&format!("エラー[E103]: {path}:4:1: {path} が、地図の範囲の外の rulec \"archive/old_methods.rule\" enum {m} を参照しています（import enum）\n")), "{ja}");
        assert!(ja.contains(&format!("エラー[E103]: {path}:8:1: {path} が、地図の範囲の外の koyomi \"archive/old_terms.cal\" date {d} を参照しています（range from koyomi）\n")), "{ja}");
    }
}
