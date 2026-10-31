//! A rule's `implements` (DESIGN 26): the service a rule implements is an open host service of a
//! published language of its own context (E208), as a workflow's is. Rules that implement the
//! services of one contract are published as the contract (`tests/contract`: its two rules
//! implement the two services of `shop.pricing.v1`), and a rule is called across the boundary as
//! the contract's service, by `use rule … connect` as by `use proto` and `connect`. A context may
//! instead publish the one rule itself, in the contract's package. E004 and E006 show the
//! contract's form where rules meet in one published language or one package. The mutants hold
//! E208 word for word; these hold the cases they do not show.

mod common;

use common::{check_dir, codes, text, variant};
use ritsu_base::text::Lang;

/// The map, a tree of its own: the sakai of this crate checks every map of `tests/maps/`, and
/// reads no rule or workflow.
const CONTRACT: &str = "tests/contract";

const PRICING: &str = "ctx/pricing.ctx";

/// The block the map publishes the contract with.
const CONTRACT_BLOCK: &str = "published language shop.pricing.v1\n  proto \"../proto/shop/pricing/v1/pricing.proto\"\n  open host service ShippingService, TaxService\n";

/// A workflow of ordering's that calls nothing, and ordering with no relationship: what is left
/// is pricing's own.
fn alone(edits: &[(&'static str, &'static str, &'static str)]) -> common::TempDir {
    let mut all = vec![
        ("ordering/checkout.flow", "", "workflow checkout v1\n\ninputs\n  id : string\n\noutputs\n  id : string\n\nflow\n  succeed id = id\n"),
        ("ctx/ordering.ctx", "\nupstream Pricing conformist\n  through shop.pricing.v1\n", ""),
    ];
    all.extend_from_slice(edits);
    variant(CONTRACT, &all)
}

/// The notes of the one diagnostic with `code`, in a language.
fn notes(os: &[sakai::check::Outcome], code: &str, lang: Lang) -> Vec<String> {
    let ds: Vec<_> = os.iter().flat_map(|o| o.diags.iter()).filter(|d| d.code == code).collect();
    assert_eq!(ds.len(), 1, "{code}: {}", text(os, Lang::En));
    ds[0].notes.iter().map(|n| n.get(lang).to_string()).collect()
}

#[test]
fn two_rules_that_implement_one_contract_are_published_as_the_contract() {
    let dir = variant(CONTRACT, &[]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), Vec::<&str>::new(), "{}", text(&os, Lang::En));
    // the rule called by `use rule … connect` and the contract's service called by `connect`
    assert_eq!(
        text(&os, Lang::En),
        "contract.ctx: ok — 2 contexts, 1 relationship; 4 artifacts, each in one context; 3 crossings checked (dandori 3)\n"
    );
    assert_eq!(text(&os, Lang::Ja), "contract.ctx: ok — コンテキスト 2、関係 1。成果物 4 件は、どれも一つのコンテキストに属する。境界を越える参照 3 件を確かめた（dandori 3）\n");
    let c = os[0].checked.as_ref().unwrap();
    let rule = c.crossings.iter().find(|x| x.to == "pricing/rules/shipping.rule").expect("the rule's crossing");
    assert!(rule.allowed.is_some(), "{rule:?}");
}

#[test]
fn a_service_left_out_of_open_host_service_is_e208() {
    let dir = variant(CONTRACT, &[(PRICING, "open host service ShippingService, TaxService", "open host service ShippingService")]);
    let os = check_dir(dir.path());
    let mut got = codes(&os);
    got.sort();
    // the rule that implements it, and the workflow that calls it (E207)
    assert_eq!(got, ["E207", "E208"], "{}", text(&os, Lang::En));
    let en = text(&os, Lang::En);
    assert!(en.contains("error[E208]: pricing/rules/tax.rule:2:1: The rule pricing/rules/tax.rule of Pricing implements proto \"proto/shop/pricing/v1/pricing.proto\" service TaxService, which is no open host service of a published language of Pricing\n"), "{en}");
    assert_eq!(
        notes(&os, "E208", Lang::En),
        [
            "The .proto is in Pricing's published language shop.pricing.v1, and its `open host service` does not list TaxService.",
            "A service a rule implements is one the rule's context publishes: list the contract's .proto in its published language, and the service under `open host service`.",
        ]
    );
    let ja = text(&os, Lang::Ja);
    assert!(ja.contains("エラー[E208]: pricing/rules/tax.rule:2:1: 「Pricing」の pricing/rules/tax.rule が実装する proto \"proto/shop/pricing/v1/pricing.proto\" service TaxService は、「Pricing」の公表された言語の公開ホストサービスではありません\n"), "{ja}");
    assert_eq!(
        notes(&os, "E208", Lang::Ja),
        [
            "その proto は「Pricing」の公表された言語 shop.pricing.v1 にありますが、`open host service` に TaxService がありません。",
            "規則が実装するサービスは、その規則を持つコンテキストが公表するものです。自分の公表された言語に契約の proto を並べ、`open host service` にサービスを書いてください。",
        ]
    );
}

#[test]
fn a_contract_in_no_published_language_is_e208_for_each_rule() {
    let dir = alone(&[(PRICING, CONTRACT_BLOCK, "")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E208", "E208"], "{}", text(&os, Lang::En));
    for d in os.iter().flat_map(|o| o.diags.iter()) {
        assert_eq!(d.notes[0].en, "Neither the .proto nor the rule is in a published language of Pricing.");
        assert_eq!(d.notes[0].ja, "その proto も規則も、「Pricing」のどの公表された言語にも入っていません。");
    }
}

/// A rule called by `use rule … connect` whose contract is in no published language: E202 says
/// to publish the contract, not the rule.
#[test]
fn a_rule_called_by_connect_whose_contract_is_not_published_is_e202() {
    let internal = "syntax = \"proto3\";\n\npackage shop.pricing.internal.v1;\n\nmessage Fired {\n  string table = 1;\n  int32 row = 2;\n  string label = 3;\n}\n\nmessage ShippingRequest {\n  optional int64 subtotal = 1;\n}\n\nmessage ShippingResponse {\n  int64 shipping = 1;\n  repeated Fired trace = 2;\n}\n\nservice ShippingService {\n  rpc Quote(ShippingRequest) returns (ShippingResponse) {\n    option idempotency_level = NO_SIDE_EFFECTS;\n  }\n}\n";
    let dir = variant(
        CONTRACT,
        &[
            ("proto/shop/pricing/internal/v1/shipping.proto", "", internal),
            ("pricing/rules/shipping.rule", "\"../../proto/shop/pricing/v1/pricing.proto\"", "\"../../proto/shop/pricing/internal/v1/shipping.proto\""),
        ],
    );
    let os = check_dir(dir.path());
    let mut got = codes(&os);
    got.sort();
    // the rule, whose contract its context does not publish, and the workflow that calls it
    assert_eq!(got, ["E202", "E208"], "{}", text(&os, Lang::En));
    assert_eq!(notes(&os, "E208", Lang::En)[0], "Neither the .proto nor the rule is in a published language of Pricing.");
    assert_eq!(
        notes(&os, "E202", Lang::En),
        ["The rule pricing/rules/shipping.rule implements ShippingService of proto/shop/pricing/internal/v1/shipping.proto, and the .proto is in no published language of Pricing; to call the rule by Connect, Pricing lists the .proto in a published language, with ShippingService under `open host service`."]
    );
    assert_eq!(
        notes(&os, "E202", Lang::Ja),
        ["pricing/rules/shipping.rule は proto/shop/pricing/internal/v1/shipping.proto の ShippingService を実装する規則で、その proto は「Pricing」の公表された言語に入っていません。この規則を Connect で呼ぶには、「Pricing」がその proto を公表された言語に並べ、`open host service` に ShippingService を書く必要があります。"]
    );
}

/// The form 0.39.0 wrote for one rule: the rule itself as the published language, in the
/// contract's package (E302), with the service under its `open host service`.
#[test]
fn one_rule_published_as_itself_in_the_contracts_package_passes() {
    let tax_alone = "rule tax v1\ndescription \"A flat tax on an order of 100 dollars or more\"\n\ninputs\n  amount : money[USD]  range >=0USD <=100000USD\n\noutputs\n  tax : money[USD]  round down(1USD)\n\ntable flat\npolicy unique\n| amount    | -> tax |\n| >=100USD  | 8USD   |\n| <100USD   | 0USD   |\n";
    let flow = "workflow checkout v1\n\nuse rule shipping from \"../pricing/rules/shipping.rule\"\n  connect \"https://pricing.example.com\"\n\ninputs\n  subtotal : money[USD]  range >=0 <=100000\n\noutputs\n  shipping : money[USD]\n\nflow\n  let quoted = shipping(subtotal: subtotal)\n  succeed shipping = quoted.shipping\n";
    let block = "published language shop.pricing.v1\n  rulec \"../pricing/rules/shipping.rule\"\n  open host service ShippingService\n";
    let dir = variant(CONTRACT, &[(PRICING, CONTRACT_BLOCK, block), ("pricing/rules/tax.rule", "", tax_alone), ("ordering/checkout.flow", "", flow)]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), Vec::<&str>::new(), "{}", text(&os, Lang::En));
    // without its `open host service`: E208 for the rule, and E207 for the workflow that calls it
    let bare = "published language shop.pricing.v1\n  rulec \"../pricing/rules/shipping.rule\"\n";
    let dir = variant(CONTRACT, &[(PRICING, CONTRACT_BLOCK, bare), ("pricing/rules/tax.rule", "", tax_alone), ("ordering/checkout.flow", "", flow)]);
    let os = check_dir(dir.path());
    let mut got = codes(&os);
    got.sort();
    assert_eq!(got, ["E207", "E208"], "{}", text(&os, Lang::En));
    assert_eq!(notes(&os, "E208", Lang::En)[0], "The rule is in Pricing's published language shop.pricing.v1, and its `open host service` does not list ShippingService.");
    assert_eq!(notes(&os, "E208", Lang::Ja)[0], "その規則は「Pricing」の公表された言語 shop.pricing.v1 にありますが、`open host service` に ShippingService がありません。");
}

const CONTRACT_FORM_EN: &str = "Rules that implement services of one contract with `implements` are published as the contract, not one by one: under `published language <the contract's package>`, write `proto \"<contract>\"`, and list the services the rules implement under `open host service`. The rules need only be in `owns`; E208 makes sure the service each rule implements is listed there.";
const CONTRACT_FORM_JA: &str = "規則が一つの契約のサービスを `implements` で実装するなら、規則ごとにではなく、契約を公表してください。`published language <契約の package>` の下に `proto \"<契約>\"` を書き、規則が実装するサービスを `open host service` に並べます。規則は `owns` に入っていれば足り、各規則が実装するサービスがそこにあることは E208 が確かめます。";

#[test]
fn two_rules_in_one_published_language_are_e004_and_shown_the_contracts_form() {
    for block in [
        "published language shop.pricing.v1\n  rulec \"../pricing/rules/shipping.rule\"\n  rulec \"../pricing/rules/tax.rule\"\n  open host service ShippingService, TaxService\n",
        "published language shop.pricing.v1\n  proto \"../proto/shop/pricing/v1/pricing.proto\"\n  rulec \"../pricing/rules/shipping.rule\"\n  open host service ShippingService, TaxService\n",
        "published language shop.pricing.v1\n  rulec \"../pricing/rules/shipping.rule\"\n  proto \"../proto/shop/pricing/v1/pricing.proto\"\n  open host service ShippingService, TaxService\n",
    ] {
        let dir = variant(CONTRACT, &[(PRICING, CONTRACT_BLOCK, block)]);
        let os = check_dir(dir.path());
        assert_eq!(codes(&os), ["E004"], "{}", text(&os, Lang::En));
        assert_eq!(
            notes(&os, "E004", Lang::En),
            ["A rule publishes one Connect service of its own, so each rule gets its own `published language rulec.<alias>.v<n>`; a Rust crate, too, is a published language of its own.", CONTRACT_FORM_EN]
        );
        assert_eq!(
            notes(&os, "E004", Lang::Ja),
            ["rulec の規則は、規則ごとに一つの Connect のサービスを公表するので、規則ごとに `published language rulec.<別名>.v<版>` を分けてください。Rust のクレートも、クレートごとに一つの公表された言語にしてください。", CONTRACT_FORM_JA]
        );
    }
    // a document beside the `.proto` meets no rule: the note of the contract's form is not given
    let block = "published language shop.pricing.v1\n  proto \"../proto/shop/pricing/v1/pricing.proto\"\n  openapi \"../pricing/api.yaml\"\n";
    let dir = variant(CONTRACT, &[(PRICING, CONTRACT_BLOCK, block)]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E004"], "{}", text(&os, Lang::En));
    assert_eq!(notes(&os, "E004", Lang::En).len(), 1);
}

#[test]
fn a_package_two_rules_publish_is_e006_and_shown_the_contracts_form() {
    let blocks = "published language shop.pricing.v1\n  rulec \"../pricing/rules/shipping.rule\"\n  open host service ShippingService\n\npublished language shop.pricing.v1\n  rulec \"../pricing/rules/tax.rule\"\n  open host service TaxService\n";
    let dir = variant(CONTRACT, &[(PRICING, CONTRACT_BLOCK, blocks)]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E006"], "{}", text(&os, Lang::En));
    let en = text(&os, Lang::En);
    assert!(en.starts_with("error[E006]: ctx/pricing.ctx:12:20: The published language shop.pricing.v1 is declared twice\n"), "{en}");
    assert_eq!(notes(&os, "E006", Lang::En), ["This file publishes the same package twice; one package is one published language.", CONTRACT_FORM_EN]);
    assert_eq!(notes(&os, "E006", Lang::Ja), ["このファイルの中で、同じ package を二度公表しています。一つの package は、一つの公表された言語にまとめてください。", CONTRACT_FORM_JA]);
    // two contexts that publish one package of `.proto` files are told as they were
    let dir = variant(CONTRACT, &[("ctx/ordering.ctx", "  dir \"../ordering\"\n", "  dir \"../ordering\"\n\npublished language shop.pricing.v1\n  proto \"../proto/shop/pricing/v1/pricing.proto\"\n")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E006"], "{}", text(&os, Lang::En));
    assert_eq!(notes(&os, "E006", Lang::En), ["The context file ctx/pricing.ctx publishes the same package; a package is published by one context."]);
}

/// `sakai doc` says which rule implements each open host service, and what service a rule
/// implements.
#[test]
fn the_page_says_which_rule_implements_each_service() {
    let dir = variant(CONTRACT, &[]);
    let os = check_dir(dir.path());
    let suite = common::suite();
    let en = sakai::doc::write(&os[0], &suite, Lang::En, sakai::doc::Format::Markdown);
    assert!(en.contains("open host service `ShippingService` (`Quote`), implemented by `pricing/rules/shipping.rule`; open host service `TaxService` (`Quote`), implemented by `pricing/rules/tax.rule`"), "{en}");
    assert!(en.contains("inputs `subtotal`; outputs `shipping`; implements `ShippingService`"), "{en}");
    let ja = sakai::doc::write(&os[0], &suite, Lang::Ja, sakai::doc::Format::Markdown);
    assert!(ja.contains("公開ホストサービス `ShippingService`（`Quote`）。`pricing/rules/shipping.rule` が実装する"), "{ja}");
    assert!(ja.contains("入力 `subtotal`／出力 `shipping`／実装するサービス `ShippingService`"), "{ja}");
}
