//! The titles of what a request shows from another file (DESIGN 4.8): the values of a rule's enum
//! a `choice from` shows are titled under the field in the file's language and labelled in the
//! others by `<field>.<value>`; a master's rows take no titles. The titles a screen needs and the
//! file does not give are E703 (the mutants), as are the stages and the ways of a shared route
//! written in another language.

mod common;

use ritsu_base::text::Lang;

/// A request that chooses from the rule's enum `authority.kind` (`goods`, `capex`, `software`),
/// with `under` as the lines under the field and `labels` as its `labels ja`.
fn kind_from_the_rule(under: &str, labels: &str) -> String {
    format!(
        "request example v1 \"Example\"\n\nuse rule authority from \"shared/authority.rule\"\n\nform\n  kind : choice from authority.kind \"Kind\"\n{under}\nroute\n  check  finance\n\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n\nlabels ja\n  request        \"例\"\n  kind           \"種類\"\n{labels}  stage check    \"確認\"\n  who finance    \"財務\"\n"
    )
}

const TITLES: &str = "    goods    \"Goods and services\"\n    capex    \"Capital expenditure\"\n    software \"Software\"\n";
const LABELS: &str = "  kind.goods     \"物品・サービス\"\n  kind.capex     \"設備投資\"\n  kind.software  \"ソフトウェア\"\n";

#[test]
fn a_rules_enum_is_titled_under_the_field_and_labelled_by_its_values() {
    let o = common::check_text("examples/example.ringi", &kind_from_the_rule(TITLES, LABELS));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
}

/// A label names a value of the enum by its name; one for a value the enum does not have is for
/// nothing (E007).
#[test]
fn a_label_for_a_value_the_enum_does_not_have_is_for_nothing() {
    let o = common::check_text("examples/example.ringi", &kind_from_the_rule(TITLES, &format!("{LABELS}  kind.services  \"役務\"\n")));
    assert_eq!(common::codes(&o), ["E007"], "{}", common::shown(&o, Lang::En));
    assert!(common::shown(&o, Lang::En).contains("The label `kind.services` is for nothing"), "{}", common::shown(&o, Lang::En));
    assert!(common::shown(&o, Lang::Ja).contains("`kind.services` の文言が指すものがありません"), "{}", common::shown(&o, Lang::Ja));
}

/// What the screens say of the request written as `src` before they are made (as `ritsu gen
/// --target requests` checks them), and the titles of the values of its field `kind`: None where the
/// request does not pass its check, which a label it lacks fails (E703). The request is
/// `example.ringi` beside the mutants' small organizations (`shared/small.ringi`, `shared/小さい会社
/// .ringi`) and the examples' rule `shared/authority.rule`, whose enum `kind` writes English names
/// (`goods "Goods and services"`, `capex "Capital expenditure"`, `software "Software"`).
fn beside_the_rule(name: &str, src: &str) -> (Vec<&'static str>, Option<Vec<(String, String)>>) {
    let t = ritsu_testkit::TempDir::new(name);
    let shared = t.path().join("shared");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("tests/mutants/shared/org-small"), &shared.join("org-small"));
    for (from, to) in [("tests/mutants/shared/small.ringi", "small.ringi"), ("tests/mutants/shared/小さい会社.ringi", "小さい会社.ringi"), ("examples/shared/authority.rule", "authority.rule")] {
        std::fs::copy(from, shared.join(to)).unwrap();
    }
    let path = t.path().join("example.ringi");
    std::fs::write(&path, src).unwrap();
    let codes = common::codes(&common::check_screens(path.to_str().unwrap()));
    let engine = ringi::ports::Engine::new(common::joined());
    engine.keep_listing_apart();
    let titles = engine.kinds(std::slice::from_ref(&path), t.path()).ok().map(|mut k| {
        let kind = k.remove(0);
        let f = kind.form.iter().find(|f| f.named.name == "kind").unwrap();
        f.choices.iter().map(|c| (c.name.clone(), c.label.clone())).collect()
    });
    (codes, titles)
}

/// A request that chooses from the rule's enum `authority.kind` reading a small organization, with
/// `under` as the lines under the field.
fn kind_in_english(under: &str) -> String {
    format!(
        "request example v1 \"Example\"\n\nuse organization org from \"shared/small.ringi\"\nuse rule authority from \"shared/authority.rule\"\n\nform\n  kind : choice from authority.kind \"Kind\"\n{under}\nroute\n  check  manager\n\nwho\n  manager = manager of requester\n\nticket\n  self approval  not allowed\n"
    )
}

/// The display names the rule writes for its values title them where the rule is written in the
/// file's language (DESIGN 4.8): an English request reads the English rule's names with no lines
/// under the field, and a line under it titles its value over the rule's name. A Japanese request
/// does not read them, as the rule writes them in English: a value with no line under the field is
/// E703 there, which the check says, and the request makes no screens. (The small organization's
/// president has no manager, which the check says, E502.)
#[test]
fn a_rules_display_names_title_its_values_in_its_own_language() {
    let owned = |v: &[(&str, &str)]| Some(v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>());
    let (codes, titles) = beside_the_rule("ringi-titles-ruled", &kind_in_english(""));
    assert!(!codes.contains(&"E703"), "{codes:?}");
    assert_eq!(titles, owned(&[("goods", "Goods and services"), ("capex", "Capital expenditure"), ("software", "Software")]));
    let (codes, titles) = beside_the_rule("ringi-titles-ruled-line", &kind_in_english("    capex \"Equipment\"\n"));
    assert!(!codes.contains(&"E703"), "{codes:?}");
    assert_eq!(titles, owned(&[("goods", "Goods and services"), ("capex", "Equipment"), ("software", "Software")]));
    let ja = |under: &str| {
        format!(
            "request 例(example) v1 \"例\"\n\nuse organization 組織(org) from \"shared/小さい会社.ringi\"\nuse rule 権限表(authority) from \"shared/authority.rule\"\n\nform\n  種類(kind) : choice from 権限表.kind \"種類\"\n{under}\nroute\n  確認(check)  上長\n\nwho\n  上長(manager) = manager of requester\n\nticket\n  self approval  not allowed\n"
        )
    };
    let (codes, titles) = beside_the_rule("ringi-titles-ruled-ja", &ja("    goods \"物品・サービス\"\n"));
    assert_eq!(codes, ["E703", "E502"]);
    assert_eq!(titles, None);
    let (codes, titles) = beside_the_rule("ringi-titles-ruled-ja-all", &ja("    goods    \"物品・サービス\"\n    capex    \"設備投資\"\n    software \"ソフトウェア\"\n"));
    assert!(!codes.contains(&"E703"), "{codes:?}");
    assert_eq!(titles, owned(&[("goods", "物品・サービス"), ("capex", "設備投資"), ("software", "ソフトウェア")]));
}

/// The values of a master are its rows, which the screens show by their own columns: a line under
/// a `choice from` a master is refused (E001), in English and in Japanese.
#[test]
fn a_choice_from_a_master_takes_no_lines_under_it() {
    let src = "request example v1 \"Example\"\n\nuse masters m from \"shared/masters.ringi\"\n\nform\n  account : choice from m.accounts \"Account code\"\n    travel \"Travel\"\n\nroute\n  check  finance\n\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n";
    let o = common::check_text("examples/example.ringi", src);
    assert_eq!(common::codes(&o), ["E001"], "{}", common::shown(&o, Lang::En));
    let (en, ja) = (common::shown(&o, Lang::En), common::shown(&o, Lang::Ja));
    assert!(en.contains("A `choice from` the master `m.accounts` has no lines under it: its values are the master's rows") && en.contains(":7:"), "{en}");
    assert!(ja.contains("マスタから選ぶ `m.accounts` の下には、行を書けません。値はマスタの行です"), "{ja}");
}

/// A line under a field of another type is refused as before, and the words name every field that
/// takes lines under it.
#[test]
fn a_field_of_another_type_takes_no_lines_under_it() {
    let src = "request example v1 \"Example\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n    big \"Big\"\n\nroute\n  check  finance\n\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n";
    let o = common::check_alone("example.ringi", src);
    assert_eq!(common::codes(&o), ["E001"], "{}", common::shown(&o, Lang::En));
    assert!(common::shown(&o, Lang::En).contains("Only a `choice`, a `choice from` or a `rows` field has lines under it"), "{}", common::shown(&o, Lang::En));
}
