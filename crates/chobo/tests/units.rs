//! The units of a book as ritsu's languages share them (ritsu's DESIGN 5.4, PLAN D.9): a unit
//! named for a currency is money, and may say whether it is with tax or without
//! (`unit 円 incl_tax`); a unit of the table's quantities is that quantity; any other unit is a
//! count with its name and nothing else. Inside the book a unit is still compared by its name, and
//! nothing the book does changes with the tax.

use ritsu_ports::Books;
use ritsu_units::{Dim, Tax, Unit};

fn units(src: &str) -> Vec<(String, Unit)> {
    let (book, diags) = chobo::model::load(src);
    assert!(diags.iter().all(|d| !d.is_error()), "{:?}", diags.iter().map(|d| d.message.en.clone()).collect::<Vec<_>>());
    book.unwrap().units.into_iter().map(|u| (u.name, u.ty)).collect()
}

fn one(line: &str) -> Unit {
    units(&format!("book b v1\n{line}\n"))[0].1.clone()
}

fn codes(src: &str) -> Vec<&'static str> {
    let (_, diags) = chobo::model::load(src);
    diags.iter().map(|d| d.code).collect()
}

#[test]
fn a_unit_is_money_a_quantity_or_a_count() {
    assert_eq!(one("unit 円"), Unit::money("円", None).unwrap());
    assert_eq!(one("unit 円 incl_tax"), Unit::money("円", Some(Tax::Incl)).unwrap());
    assert_eq!(one("unit JPY excl_tax"), Unit::money("JPY", Some(Tax::Excl)).unwrap());
    // a currency counted in hundredths is its hundredth, the cent
    assert_eq!(one("unit USD scale 2"), Unit::money("USDc", None).unwrap());
    assert_eq!(one("unit USD scale 2 excl_tax"), Unit::money("USDc", Some(Tax::Excl)).unwrap());
    assert_eq!(one("unit 銭"), Unit::money("銭", None).unwrap());
    // the hundredth of a yen is the sen: yen with scale 2 is chobo's own count, as is a currency
    // with any other scale
    assert_eq!(one("unit 円 scale 2"), Unit::count("円"));
    assert_eq!(one("unit EUR scale 3"), Unit::count("EUR"));
    assert_eq!(one("unit kg").dim, Dim::Mass);
    assert_eq!(one("unit L").dim, Dim::Volume);
    assert_eq!(one("unit kg scale 3"), Unit::count("kg"));
    assert_eq!(one("unit 個"), Unit::count("個"));
    assert_eq!(one("unit 席").dim, Dim::Count("席".into()));
}

/// What the units mean where an amount crosses into the book: a unit with tax is rulec's
/// `money[円, incl_tax]` (and `JPY` is `円`); a unit that says neither takes an amount that says
/// neither, not one with tax or without; seats are not items.
#[test]
fn a_unit_without_a_tax_is_the_same_only_as_an_amount_without_one() {
    let with = one("unit 円 incl_tax");
    let without = one("unit 円");
    let rule = |s: &str| Unit::parse(s).unwrap();
    assert!(with.same(&rule("money[JPY, incl_tax]")));
    assert!(!with.same(&rule("money[円, excl_tax]")));
    assert!(without.same(&rule("money[円]")));
    assert!(!without.same(&rule("money[円, incl_tax]")));
    assert!(!without.same(&rule("money[円, excl_tax]")));
    assert!(one("unit USD scale 2").same(&rule("money[USDc]")));
    assert!(!one("unit USD scale 2").same(&rule("money[USD]")));
    assert!(!one("unit 席").same(&one("unit 個")));
    assert!(one("unit kg").same(&rule("mass[kg]")));
}

/// Only money is with tax or without (E014), and the words are keywords: a name cannot be one.
#[test]
fn a_tax_is_only_on_money() {
    for line in ["unit 個 incl_tax", "unit 円 scale 2 incl_tax", "unit kg excl_tax", "unit EUR scale 3 excl_tax"] {
        assert_eq!(codes(&format!("book b v1\n{line}\n")), ["E014"], "{line}");
    }
    assert_eq!(codes("book b v1\nunit incl_tax\n"), ["E001"]);
    assert_eq!(codes("book b v1\nunit 円 tax\n"), ["E001"]);
    assert_eq!(codes("book b v1\nunit 円 incl_tax scale 0\n"), ["E001"]);
}

/// The book's facts hand the units over as ritsu's units, and `chobo api` says the tax of a unit
/// that has one, and nothing new of a unit that has none.
#[test]
fn the_facts_and_the_api_carry_the_units() {
    let dir = ritsu_testkit::TempDir::new("units");
    let path = dir.write("財布.book", "book 財布 v1\nunit 円 incl_tax\nunit pt\n\naccount 財布(会員: string) : 円\n  at least 0 refused as 残高不足\naccount 入金元 : 円 outside\n\ntransfer 入金(伝票: string, 会員: string, 額: 円)\n  key 伝票\n  move 額 from 入金元 to 財布(会員)\ntransfer 払う(伝票: string, 会員: string, 額: 円)\n  key 伝票\n  move 額 from 財布(会員) to 入金元\n");
    let facts = chobo::ports::Engine.facts(&path).unwrap();
    assert_eq!(facts.unit("円"), Some(&Unit::money("円", Some(Tax::Incl)).unwrap()));
    assert_eq!(facts.unit("pt"), Some(&Unit::count("pt")));
    assert_eq!(facts.units[0].scale, 0);
    let src = std::fs::read_to_string(&path).unwrap();
    let c = chobo::check::check_source(&src);
    let api = chobo::api::api(c.book.as_ref().unwrap(), &src, c.report.as_ref().unwrap(), "財布");
    assert_eq!(api["units"][0]["tax"], "incl_tax");
    assert!(api["units"][1].get("tax").is_none(), "{}", api["units"][1]);
}
