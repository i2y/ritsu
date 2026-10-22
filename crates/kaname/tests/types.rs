//! Every expression's type (DESIGN 2.4, 3, 6): rulec's arithmetic of units and rounding, what kaname
//! adds to it (a division by a value, `and`, `or`, `not`, `!=`, the difference of two dates, `in`),
//! the places a value goes to (E102, E103, E104) and the weights of `running_totals` (E103), the
//! operations a type does not have (E105), `match` (E106), calls (E107, E204), builtins (E109), the
//! words that read two ways (E110), and the examples (E602). The rules, the dates and the books are
//! the examples, read through the ports as `ritsu kaname` reads them.

mod common;

use kaname::ast::ExprKind;
use kaname::check::{Outcome, Run};
use kaname::ty::{NumTy, Ty};
use ritsu_base::text::Lang;
use ritsu_testkit::TempDir;
use ritsu_units::{Rat, Unit};
use std::path::Path;
use std::rc::Rc;

/// A directory with the examples' rules, dates file and book, and chobo's inventory, beside the module
/// a test writes.
fn project() -> TempDir {
    let t = TempDir::new("kaname-types");
    let d = Path::new("examples");
    for (from, to) in [
        ("payout/rules/card_fee.rule", "rules/card_fee.rule"),
        ("payout/books/ledger.book", "books/ledger.book"),
        ("late_payment/rules/late_payment_charges.rule", "rules/charges.rule"),
        ("late_payment/dates/payment_terms.cal", "dates/terms.cal"),
        ("media_mail/rules/media_mail.rule", "rules/media_mail.rule"),
        ("discount_split/rules/discount_share.rule", "rules/discount_share.rule"),
    ] {
        t.write(to, std::fs::read(d.join(from)).unwrap());
    }
    // the copies of the documents the rules cite, which rulec holds them to
    for x in ["payout", "late_payment", "media_mail", "discount_split"] {
        ritsu_testkit::tmp::copy_dir(&d.join(x).join("rules/sources"), &t.path().join("rules/sources"));
    }
    t.write("books/inventory.book", std::fs::read("../chobo/examples/inventory/inventory.book").unwrap());
    t.write("rules/earliest.rule", std::fs::read("../rulec/tests/corpus/最も早い期日.rule").unwrap());
    t
}

/// The module `src`, checked in `t` with every language joined.
fn check_in(t: &TempDir, src: &str) -> Rc<Outcome> {
    let path = t.write("main.kn", src);
    Run::new(common::joined()).file(path.to_str().unwrap()).unwrap()
}

fn text(o: &Outcome) -> String {
    o.diags.iter().map(|d| d.render(Lang::En)).collect()
}

/// The codes the module `body` says, with a head that reads every file of [`project`].
fn codes(t: &TempDir, body: &str) -> Vec<&'static str> {
    let head = "module m v1\n\nuse rule card_fee from \"rules/card_fee.rule\"\nuse rule charges from \"rules/charges.rule\"\nuse rule media_mail from \"rules/media_mail.rule\"\nuse rule shares from \"rules/discount_share.rule\"\nuse dates terms from \"dates/terms.cal\"\nuse book ledger from \"books/ledger.book\"\nuse book stock from \"books/inventory.book\"\n\n";
    let o = check_in(t, &format!("{head}{body}"));
    if !o.diags.is_empty() {
        eprintln!("{}", text(&o));
    }
    // what the analysis of ranges and the warning of decisions say is theirs to test (`tests/ranges.rs`,
    // `effects.rs`, `hatch.rs`)
    common::codes_before_analysis(&o)
}

fn none() -> Vec<&'static str> {
    Vec::new()
}

#[test]
fn the_examples_pass_with_every_language_joined() {
    for f in common::EXAMPLES {
        let o = common::check(f);
        assert!(o.diags.is_empty(), "{f}: {}", text(&o));
        assert!(o.checked.is_some(), "{f}");
    }
}

#[test]
fn the_types_of_the_examples() {
    // `paid_on - due` is the days between two dates, a whole number
    let o = common::check("examples/late_payment/late_payment.kn");
    let c = o.checked.as_ref().unwrap();
    let late = &c.module.fns[0].body.lets[1];
    assert_eq!(late.name, "late");
    assert_eq!(c.typed.types[late.value.id], Ty::Num(NumTy::number()));
    // the weight added up stays in ounces, whole
    let o = common::check("examples/media_mail/media_mail.kn");
    let c = o.checked.as_ref().unwrap();
    let w = &c.typed.types[c.module.fns[0].body.answer.id];
    assert_eq!(w, &Ty::Num(NumTy { unit: Unit::quantity("mass", "oz").unwrap(), step: Some(Rat::int(1)), konst: None, ratio: false }));
}

#[test]
fn literals_are_read_in_the_unit_they_meet() {
    let t = project();
    // 15lb beside ounces is 240 ounces; 1lb is no whole number of grams (DESIGN 3.2)
    assert_eq!(codes(&t, "fn f(w: mass[oz] range >=1oz <=160oz) -> bool\n  w <= 15lb\n"), none());
    assert_eq!(codes(&t, "fn f(w: mass[g] range >=1g <=10kg) -> bool\n  w <= 1lb\n"), ["E104"]);
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=0USDc <=100USDc) -> bool\n  a >= 35USD\n"), none());
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=0USD <=100USD) -> bool\n  a >= 3550USDc\n"), ["E104"]);
    // a literal is a whole number of the unit it is written in; a fraction is a rate
    assert_eq!(codes(&t, "fn f() -> money[USDc]\n  0.5USD\n"), ["E104"]);
    assert_eq!(codes(&t, "fn f(n: number range >=0 <=9) -> rate[step 1%]\n  n * 0.4\n"), ["E104"]);
    // zero is zero of anything
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=0USDc <=100USDc, r: rate[step 1%]) -> bool\n  a > 0 and r > 0\n"), none());
    // a literal takes the tax of what it meets
    assert_eq!(codes(&t, "fn f(a: money[GBP, incl_tax] range >=0GBP <=100GBP) -> money[GBP, incl_tax]\n  a + 40GBP\n"), none());
}

#[test]
fn amounts_rates_and_rounding() {
    let t = project();
    // an amount times a rate is unrounded, until a rounding says how (E103)
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc) -> money[USDc]\n  a * 3.49%\n"), ["E103"]);
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc) -> money[USDc]\n  half_up(a * 3.49%, 1USDc)\n"), none());
    // a `let` takes an unrounded value, which is rounded where it goes
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc) -> money[USDc]\n  let x : money[USDc] = a * 3%\n  down(x, 1USDc) + 49USDc\n"), none());
    // a step the place is a whole multiple of: an amount times 200% is still whole
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc) -> money[USDc]\n  a * 200%\n"), none());
    // the step of a rounding is read in the value's unit: 100 cents round dollars
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=1USD <=100USD) -> money[USD]\n  down(a * 3%, 100USDc)\n"), none());
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=1USD <=100USD) -> money[USD]\n  down(a * 3%, 1kg)\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=1USD <=100USD, s: money[USD] range >=1USD <=2USD) -> money[USD]\n  down(a * 3%, s)\n"), ["E104"]);
    // a rate goes to a rate's place on its step
    assert_eq!(codes(&t, "fn f(r: rate[step 0.01%] range >=0% <=10%) -> rate[step 1%]\n  r\n"), ["E103"]);
    assert_eq!(codes(&t, "fn f(r: rate[step 1%] range >=0% <=10%) -> rate[step 0.01%]\n  r + 8%\n"), none());
    // a number times a rate is a rate (rulec's)
    assert_eq!(codes(&t, "fn f(n: number range >=0 <=9) -> number\n  n * 50%\n"), ["E102"]);
}

#[test]
fn division_by_a_value_and_ratios() {
    let t = project();
    // a division by a value leaves the step unknown: unrounded
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc, n: number range >=1 <=9) -> money[USDc]\n  a / n\n"), ["E103"]);
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc, n: number range >=1 <=9) -> money[USDc]\n  half_even(a / n, 1USDc)\n"), none());
    // an amount over an amount of its unit is a ratio: a number or a rate, as its rounding says
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc, b: money[USDc] range >=1USDc <=100USDc) -> rate[step 0.1%]\n  half_up(a / b, 0.1%)\n"), none());
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc, b: money[USDc] range >=1USDc <=100USDc) -> number\n  down(a / b, 1)\n"), none());
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc, b: money[USDc] range >=1USDc <=100USDc) -> number\n  a / b\n"), ["E103"]);
    // a division by a constant keeps the step known
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=0USDc <=100USDc) -> money[USDc]\n  a * 2 / 2\n"), none());
    // what has no meaning: an amount over a weight, a number over an amount
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc, w: mass[g] range >=1g <=9g) -> number\n  a / w\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=100USDc) -> number\n  1 / a\n"), ["E105"]);
}

#[test]
fn units_converted_and_not() {
    let t = project();
    // `in` converts within a dimension; a dollar is 100 cents, so the value stays whole
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=1USD <=100USD) -> money[USDc]\n  a in USDc\n"), none());
    // ounces in pounds are a fraction
    assert_eq!(codes(&t, "fn f(w: mass[oz] range >=1oz <=99oz) -> mass[lb]\n  w in lb\n"), ["E103"]);
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=1USD <=100USD) -> money[GBP]\n  a in GBP\n"), ["E104"]);
    assert_eq!(codes(&t, "fn f(n: number range >=1 <=9) -> number\n  n in kg\n"), ["E104"]);
    // no conversion unasked (E102), and with tax and without are two types
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=1USD <=100USD) -> money[USDc]\n  a\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(a: money[GBP] range >=0GBP <=1GBP, b: money[GBP, incl_tax] range >=0GBP <=1GBP) -> bool\n  a < b\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(a: money[GBP] range >=0GBP <=1GBP, b: money[USD] range >=0USD <=1USD) -> bool\n  a < b\n"), ["E102"]);
    // `JPY` is `円`
    assert_eq!(codes(&t, "fn f(a: money[JPY] range >=0JPY <=100JPY) -> money[円]\n  a + 10円\n"), none());
}

/// A temperature read in the other unit with that unit's zero (DESIGN 3.2, 3.10). A constant there is
/// the value the zero gives it, on its own value as a literal is: `20℃ in ℉` is 68℉, which beside a
/// ℃ is 20℃ again; `41℉` given to a ℃ is 5℃, `77℉` in a field 25℃ and `14℉` in a `max` -10℃, each
/// on the step 1 of ℃. A value converted with `in` is on the step the shift between the zeros leaves
/// it: a whole number of ℃ in ℉ is 32 plus a multiple of 9/5, a multiple of 1/5.
#[test]
fn a_temperature_is_read_in_the_other_unit_with_its_zero() {
    let t = project();
    // 20℃ in ℉ is 68℉, not 36℉: beside a ℃ it is 20℃, not 20/9℃ (E104)
    assert_eq!(codes(&t, "fn f(t: temperature[℃] range >=-40℃ <=40℃) -> bool\n  if t >= 20℃ in ℉ then true else false\n"), none());
    assert_eq!(codes(&t, "fn f() -> temperature[℉]\n  100℃ in ℉\n"), none());
    // a constant of the other unit given to a ℃, held in a field of ℃, and with a ℃ in a `max`
    let g = "fn g(t: temperature[℃] range >=-40℃ <=40℃) -> bool\n  if t < 10℃ then true else false\n";
    assert_eq!(codes(&t, &format!("{g}fn f() -> bool\n  g(t: 41℉)\n")), none());
    assert_eq!(codes(&t, "record R\n  at : temperature[℃]  range >=-60℃ <=60℃\n\nfn f() -> R\n  R { at: 77℉ }\n"), none());
    assert_eq!(codes(&t, "fn f(t: temperature[℃] range >=-40℃ <=40℃) -> temperature[℃]\n  max(t, 14℉)\n"), none());
    // one that is no whole number of ℃ is still E104: 40℉ is 40/9℃
    assert_eq!(codes(&t, &format!("{g}fn f() -> bool\n  g(t: 40℉)\n")), ["E104"]);
    assert_eq!(codes(&t, "fn f(t: temperature[℃] range >=-40℃ <=40℃) -> bool\n  if t >= 40℉ then true else false\n"), ["E104"]);
    // a value converted with `in` is unrounded, on 1/5℉ from ℃ and on 5/9℃ from ℉ (the shift, -160/9,
    // is on it); rounded, it goes
    let o = check_in(&t, "module m v1\n\nfn f(t: temperature[℃] range >=-40℃ <=40℃) -> temperature[℉]\n  t in ℉\n");
    assert!(text(&o).contains("a value of temperature[℉] that comes with a fraction, in steps of 0.2"), "{}", text(&o));
    let o = check_in(&t, "module m v1\n\nfn f(t: temperature[℉] range >=-40℉ <=104℉) -> temperature[℃]\n  t in ℃\n");
    assert!(text(&o).contains("a value of temperature[℃] that comes with a fraction, in steps of 5/9"), "{}", text(&o));
    assert_eq!(codes(&t, "fn f(t: temperature[℃] range >=-40℃ <=40℃) -> temperature[℉]\n  half_up(t in ℉, 1℉)\n"), none());
}

#[test]
fn dates_have_their_difference_only() {
    let t = project();
    assert_eq!(codes(&t, "fn f(a: date range >=2026-01-01 <=2026-12-31, b: date range >=2026-01-01 <=2026-12-31) -> number\n  b - a\n"), none());
    assert_eq!(codes(&t, "fn f(a: date range >=2026-01-01 <=2026-12-31) -> date\n  a + 30\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(a: date range >=2026-01-01 <=2026-12-31) -> date\n  a - 30\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(a: date range >=2026-01-01 <=2026-12-31) -> bool\n  a >= 2026-04-01 and a != 2026-05-01\n"), none());
    assert_eq!(codes(&t, "fn f(a: date range >=2026-01-01 <=2026-12-31) -> date\n  max(a, 2026-06-30)\n"), none());
}

#[test]
fn operations_a_type_does_not_have() {
    let t = project();
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc, b: money[USDc] range >=1USDc <=9USDc) -> money[USDc]\n  a * b\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(c: temperature[℃] range >=0℃ <=9℃) -> temperature[℃]\n  c + 1℃\n"), ["E105"]);
    // strings compare in the order of their code points (DESIGN 25.1), and take no `+`
    assert_eq!(codes(&t, "fn f(a: string, b: string) -> bool\n  a < b\n"), none());
    assert_eq!(codes(&t, "fn f(a: bool, b: bool) -> bool\n  a < b\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(a: string, b: string) -> string\n  a + b\n"), ["E105"]);
    assert_eq!(codes(&t, "enum e = x | y\nfn f(a: e, b: e) -> bool\n  a < b\n"), ["E105"]);
    assert_eq!(codes(&t, "enum e = x < y\nfn f(a: e, b: e) -> bool\n  a < b and max(a, b) = y\n"), none());
    assert_eq!(codes(&t, "fn f(a: number? range >=0 <=9) -> bool\n  a > 1\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(a: bool) -> bool\n  a + 1 = 2\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(a: number range >=0 <=9) -> number\n  a(1)\n"), ["E105"]);
    // a condition is a bool
    assert_eq!(codes(&t, "fn f(a: number range >=0 <=9) -> number\n  if a then 1 else 2\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(a: bool, b: bool) -> bool\n  not a or (b and a != b)\n"), none());
}

#[test]
fn matches_cover_every_value() {
    let t = project();
    let tier = "enum tier = basic | silver | gold\n";
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic => 1\n    silver, gold => 2\n")), none());
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic => 1\n    silver => 2\n")), ["E106"]);
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic, silver => 1\n    silver, gold => 2\n")), ["E106"]);
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic => 1\n    silver => 2\n    gold => 3\n    platinum => 4\n")), ["E101"]);
    // the values may be named with their enum
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    tier.basic => 1\n    silver, tier.gold => 2\n")), none());
    // a value that may be none, named in the arm of `some`
    assert_eq!(codes(&t, "fn f(a: money[USDc]? range >=0USDc <=9USDc) -> money[USDc]\n  match a\n    none => 0USDc\n    some x => x + 1USDc\n"), none());
    assert_eq!(codes(&t, "fn f(a: money[USDc]? range >=0USDc <=9USDc) -> money[USDc]\n  match a\n    some x => x\n"), ["E106"]);
    assert_eq!(codes(&t, "fn f(a: bool) -> number\n  match a\n    true => 1\n    false => 0\n"), none());
    // a number is split by its literals: `true` is none of them
    assert_eq!(codes(&t, "fn f(a: number range >=0 <=9) -> number\n  match a\n    true => 1\n    false => 0\n"), ["E102", "E102"]);
    // a rule's enum, by its values
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic => 1\n    silver => 2\n    gold => true\n")), ["E102"]);
}

/// The values of an enum that have fields, the type of each value, and building them (DESIGN 23.1,
/// 23.2).
#[test]
fn values_with_fields_and_their_types() {
    let t = project();
    let order = "enum Order\n  draft\n    items : number  range >=0 <=9\n  placed\n    items : number  range >=1 <=9\n    on    : date    range >=2026-01-01 <=2026-12-31\n  cancelled\n\n";
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  Order.placed {{ items: 1, on: 2026-02-01 }}\n")), none());
    // the value by its name alone, where the place wants the enum
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  placed {{ items: 1, on: 2026-02-01 }}\n")), none());
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  cancelled\n")), none());
    // a value with fields is built with them; one with none is written alone
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  Order.placed\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  Order.placed {{ items: 1 }}\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  Order.cancelled {{ items: 1 }}\n")), ["E105"]);
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  Order.placed {{ items: 1, on: 2026-02-01, items: 2 }}\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{order}fn f() -> Order\n  Order.placed {{ items: 1, on: 2026-02-01, paid: 2 }}\n")), ["E101"]);
    // the type of a value: a parameter, an answer, a field; it goes where the enum goes
    assert_eq!(codes(&t, &format!("{order}fn on(p: Order.placed) -> date\n  p.on\nfn g(p: Order.placed) -> Order\n  p\n")), none());
    assert_eq!(codes(&t, &format!("{order}record Box\n  p : Order.placed\n\nfn f(b: Box) -> number\n  b.p.items\n")), none());
    assert_eq!(codes(&t, &format!("{order}fn f(p: Order.shipped) -> number\n  1\n")), ["E101"]);
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> Order.placed\n  o\n")), ["E102"]);
    // a field of a value of the enum is read once a `match` names it
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> date\n  o.on\n")), ["E105"]);
    // the listed form has no order; `default` and `open` are not written
    assert_eq!(codes(&t, &format!("{order}fn f(a: Order, b: Order) -> bool\n  a < b\n")), ["E105"]);
    assert_eq!(codes(&t, "enum Order\n  draft\n  placed\n    on : date  range >=2026-01-01 <=2026-12-31\n  default draft\n"), ["E003"]);
    // the names the generated code gives the values' types, and the field it tells them apart by
    assert_eq!(codes(&t, &format!("{order}record OrderPlaced\n  x : number  range >=0 <=9\n")), ["E006"]);
    assert_eq!(codes(&t, "enum Order\n  draft\n    kind : number  range >=0 <=9\n  cancelled\n"), ["E007"]);
    // a value that holds the enum it is a value of, with no way to stop
    assert_eq!(codes(&t, "enum Tree\n  node\n    left : Tree\n"), ["E108"]);
    // a record and a value of the same name, where the place wants the enum
    assert_eq!(codes(&t, &format!("{order}record placed\n  on : date  range >=2026-01-01 <=2026-12-31\n\nfn f() -> Order\n  placed {{ on: 2026-02-01 }}\n")), ["E110"]);
}

/// Types that hold themselves (DESIGN 28.1): a record through a list, an optional, a dict or a result,
/// an enum through its values' fields, two types through each other, a type through a type argument of
/// a generic one, a generic enum with its own type parameters, in another order, and a cycle that gives
/// a generic type a type with none. E108 where no value can be made, and where a type of the cycle is
/// given a type argument made from a type parameter.
#[test]
fn types_that_hold_themselves() {
    let t = project();
    assert_eq!(codes(&t, "record Category\n  name     : string\n  children : list[Category]\n"), none());
    assert_eq!(codes(&t, "record Node\n  value : number  range >=0 <=9\n  next  : Node?\n"), none());
    assert_eq!(codes(&t, "record Dir\n  entries : dict[string, Dir]\n"), none());
    assert_eq!(codes(&t, "record Step\n  next : result[Step, string]\n"), none());
    assert_eq!(codes(&t, "enum Expr\n  num\n    value : number  range >=0 <=9\n  add\n    left  : Expr\n    right : Expr\n"), none());
    assert_eq!(codes(&t, "enum Doc\n  words\n    s : string\n  object\n    members : list[Member]\n\nrecord Member\n  key   : string\n  value : Doc\n"), none());
    assert_eq!(codes(&t, "record Box[T]\n  value : T\n\nrecord Nest\n  inner : Box[Nest]?\n"), none());
    assert_eq!(codes(&t, "enum Tree[T]\n  leaf\n    value : T\n  node\n    children : list[Tree[T]]\n"), none());
    assert_eq!(codes(&t, "record Swap[T, U]\n  first : T\n  next  : Swap[U, T]?\n"), none());
    assert_eq!(codes(&t, "record A\n  b : B[number]?\n\nrecord B[T]\n  a : A?\n  t : T\n"), none());
    // no value can be made: the record alone, the two that hold each other each, through a list that
    // cannot be empty, through a type argument
    assert_eq!(codes(&t, "record Node\n  next : Node\n"), ["E108"]);
    assert_eq!(codes(&t, "record A\n  b : B\n\nrecord B\n  a : A\n"), ["E108", "E108"]);
    assert_eq!(codes(&t, "record Category\n  children : list[Category]  count >=1\n"), ["E108"]);
    assert_eq!(codes(&t, "record Box[T]\n  value : T\n\nrecord Nest\n  inner : Box[Nest]\n"), ["E108"]);
    // through the type of a value of an enum that can be made by another of its values
    assert_eq!(codes(&t, "enum E\n  leaf\n  node\n    r : R\n\nrecord R\n  e : E.node\n"), ["E108"]);
    // a type argument made from a type parameter, given in the cycle
    assert_eq!(codes(&t, "record Wrap[T]\n  inner : Wrap[list[T]]?\n"), ["E108"]);
    assert_eq!(codes(&t, "record A[T]\n  b : B[Box[T]]?\n\nrecord B[T]\n  a : A[T]?\n\nrecord Box[T]\n  value : T\n"), ["E108"]);
}

/// A `match` that names the value it takes, `else`, and literals (DESIGN 23.3).
#[test]
fn matches_name_values_and_split_by_literals() {
    let t = project();
    let order = "enum Order\n  draft\n    items : number  range >=0 <=9\n  placed\n    items : number  range >=1 <=9\n    on    : date    range >=2026-01-01 <=2026-12-31\n  cancelled\n\n";
    let tier = "enum tier = basic | silver | gold\n";
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> date\n  match o\n    placed p => p.on\n    draft, cancelled => 2026-01-01\n")), none());
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> number\n  match o\n    placed p => p.items\n    draft d => d.items\n    else => 0\n")), none());
    // every value has an arm, or `else` takes the rest
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> number\n  match o\n    placed p => p.items\n")), ["E106"]);
    // a name for a value of an enum with no fields, a name shared with another value
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic b => 1\n    else => 2\n")), ["E106"]);
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> number\n  match o\n    placed p, draft => 1\n    cancelled => 2\n")), ["E106"]);
    // `else` is the last arm, takes an arm of its own, and takes some value
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    else => 1\n    basic => 2\n")), ["E106"]);
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic, else => 1\n")), ["E106"]);
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic, silver, gold => 1\n    else => 2\n")), ["E106"]);
    // literals: numbers, amounts in the unit split, dates, strings; `else` is needed, and each
    // literal is taken once
    assert_eq!(codes(&t, "fn f(n: number range >=-9 <=9) -> string\n  match n\n    -1 => \"a\"\n    0, 1 => \"b\"\n    else => \"c\"\n"), none());
    assert_eq!(codes(&t, "fn f(c: money[USDc] range >=0USDc <=900USDc) -> string\n  match c\n    1USD => \"a\"\n    else => \"b\"\n"), none());
    assert_eq!(codes(&t, "fn f(c: money[USDc] range >=0USDc <=900USDc) -> string\n  match c\n    1USD, 100USDc => \"a\"\n    else => \"b\"\n"), ["E106"]);
    assert_eq!(codes(&t, "fn f(d: date range >=2026-01-01 <=2026-12-31) -> bool\n  match d\n    2026-01-01 => true\n    else => false\n"), none());
    assert_eq!(codes(&t, "fn f(s: string) -> number\n  match s\n    \"A\" => 1\n    \"A\" => 2\n    else => 3\n"), ["E106"]);
    assert_eq!(codes(&t, "fn f(n: number range >=0 <=9) -> string\n  match n\n    1 => \"a\"\n    2 => \"b\"\n"), ["E106"]);
    assert_eq!(codes(&t, "fn f(s: string) -> number\n  match s\n    1 => 1\n    else => 2\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(c: money[USDc] range >=0USDc <=900USDc) -> string\n  match c\n    1kg => \"a\"\n    else => \"b\"\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(c: money[USDc] range >=0USDc <=900USDc) -> string\n  match c\n    1USDc => \"a\"\n    9_223_372_036_854_775_808USDc => \"b\"\n    else => \"c\"\n"), ["E104"]);
}

/// A record made from another with some fields replaced (DESIGN 23.4), and `=` and `!=` of every
/// type (DESIGN 23.5).
#[test]
fn records_made_from_another_and_values_compared_whole() {
    let t = project();
    let item = "record Item\n  sku   : string\n  price : money[USDc]  range >=1USDc <=100USDc\n\n";
    let order = "enum Order\n  draft\n    items : number  range >=0 <=9\n  placed\n    items : number  range >=1 <=9\n    on    : date    range >=2026-01-01 <=2026-12-31\n  cancelled\n\n";
    assert_eq!(codes(&t, &format!("{item}fn f(i: Item) -> Item\n  Item {{ ..i, price: 2USDc }}\n")), none());
    assert_eq!(codes(&t, &format!("{order}fn f(p: Order.placed) -> Order\n  Order.placed {{ ..p, on: 2026-03-01 }}\n")), none());
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> Order\n  match o\n    placed p => placed {{ ..p, items: 2 }}\n    else => o\n")), none());
    // the value after `..` is of the type built, and each field is written once
    assert_eq!(codes(&t, &format!("{item}{order}fn f(i: Item, p: Order.placed) -> Item\n  Item {{ ..p, price: 2USDc }}\n")), ["E102"]);
    assert_eq!(codes(&t, &format!("{order}fn f(o: Order) -> Order\n  Order.placed {{ ..o, on: 2026-03-01 }}\n")), ["E102"]);
    assert_eq!(codes(&t, &format!("{item}fn f(i: Item) -> Item\n  Item {{ ..i, price: 2USDc, price: 3USDc }}\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{item}fn f(i: Item) -> Item\n  Item {{ price: 2USDc, ..i }}\n")), ["E001"]);
    // records, values of an enum with fields, lists, values that may be none: compared whole
    assert_eq!(codes(&t, &format!("{item}fn f(a: Item, b: Item) -> bool\n  a = b\n")), none());
    assert_eq!(codes(&t, &format!("{order}fn f(a: Order, p: Order.placed) -> bool\n  a != p and a != Order.cancelled\n")), none());
    assert_eq!(codes(&t, &format!("{item}fn f(a: list[Item] count <=3, b: list[Item] count <=3) -> bool\n  a = b or a = []\n")), none());
    assert_eq!(codes(&t, "fn f(a: number? range >=0 <=9, b: number? range >=0 <=9) -> bool\n  a = b or a = none or b = 2\n"), none());
    assert_eq!(codes(&t, &format!("{item}fn f(a: Item) -> bool\n  a = 1\n")), ["E102"]);
    // order is for what has one
    assert_eq!(codes(&t, &format!("{item}fn f(a: Item, b: Item) -> bool\n  a < b\n")), ["E105"]);
}

/// A `-` before a constant of a unit: a constant below zero, of every unit (DESIGN 3.10).
#[test]
fn constants_below_zero_of_every_unit() {
    let t = project();
    assert_eq!(codes(&t, "fn f(t: temperature[℃] range >=-40℃ <=40℃) -> bool\n  if t <= -18℃ then true else false\n"), none());
    assert_eq!(codes(&t, "fn f() -> temperature[℃]\n  -18℃\n"), none());
    // read in the other unit with its zero: -20℃ is -4℉; -18℃ is no whole number of ℉
    assert_eq!(codes(&t, "fn f(t: temperature[℉] range >=-40℉ <=104℉) -> bool\n  if t <= -20℃ then true else false\n"), none());
    assert_eq!(codes(&t, "fn f(t: temperature[℉] range >=-40℉ <=104℉) -> bool\n  if t <= -18℃ then true else false\n"), ["E104"]);
    assert_eq!(codes(&t, "fn f(s: sound[dB] range >=-20dB <=140dB) -> bool\n  if s < -10dB then true else false\n"), none());
    // a value of a temperature takes no `-`: it has no arithmetic
    assert_eq!(codes(&t, "fn f(t: temperature[℃] range >=-40℃ <=40℃) -> temperature[℃]\n  -t\n"), ["E105"]);
    assert_eq!(codes(&t, "fn f(s: sound[dB] range >=-20dB <=140dB) -> sound[dB]\n  -s\n"), ["E105"]);
}

#[test]
fn calls_of_functions_rules_and_dates() {
    let t = project();
    let double = "fn double(n: number range >=0 <=9) -> number\n  n * 2\n";
    assert_eq!(codes(&t, &format!("{double}fn f() -> number\n  double(n: 2)\n")), none());
    assert_eq!(codes(&t, &format!("{double}fn f() -> number\n  double()\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{double}fn f() -> number\n  double(2)\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{double}fn f() -> number\n  double(n: 1, n: 2)\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{double}fn f() -> number\n  double(m: 2)\n")), ["E107", "E107"]);
    // a function written where its value cannot go: a function's answer (DESIGN 27.1)
    assert_eq!(codes(&t, &format!("{double}fn f() -> number\n  double\n")), ["E113"]);
    assert_eq!(codes(&t, "fn f() -> number\n  nothing(n: 2)\n"), ["E101"]);
    // a parameter written by its alias, as a name may be
    assert_eq!(codes(&t, "fn 倍(double)(数(n): number range >=0 <=9) -> number\n  数 * 2\nfn f() -> number\n  倍(n: 2)\n"), none());
    // a rule: its inputs by name, its outputs read as a record's fields
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc) -> money[USDc]\n  card_fee(amount: a, international: false).fee\n"), none());
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc) -> money[USDc]\n  card_fee(amount: a).fee\n"), ["E107"]);
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc) -> money[USDc]\n  card_fee(amount: a, international: false).fees\n"), ["E101"]);
    assert_eq!(codes(&t, "fn f(a: money[USD] range >=1USD <=9USD) -> money[USDc]\n  card_fee(amount: a, international: false).fee\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc) -> charges.outputs\n  card_fee(amount: a, international: false)\n"), ["E102"]);
    // a date: its inputs by name, its answer a date
    assert_eq!(codes(&t, "fn f(d: date range >=2026-01-01 <=2026-12-31) -> date\n  terms.due(invoiced: d)\n"), none());
    assert_eq!(codes(&t, "fn f(d: date range >=2026-01-01 <=2026-12-31) -> date\n  terms.when(invoiced: d)\n"), ["E101"]);
    assert_eq!(codes(&t, "fn f(s: string) -> date\n  terms.due(invoiced: s)\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f() -> date\n  terms(invoiced: 2026-01-01)\n"), ["E107"]);
    // a rule's optional output: a value, or none, which goes only where an optional value goes
    assert_eq!(codes(&t, "fn f(w: mass[oz] range >=1oz <=240oz) -> money[USDc]?\n  media_mail(weight: w, media_only: true).price\n"), none());
    assert_eq!(codes(&t, "fn f(w: mass[oz] range >=1oz <=240oz) -> money[USDc]\n  media_mail(weight: w, media_only: true).price\n"), ["E102"]);
}

#[test]
fn transfers_of_a_book() {
    let t = project();
    // a transfer done at once takes `do`, with every argument
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc) -> ledger.capture.do\n  ledger.capture.do(payment: \"p\", seller: \"s\", amount: a)\n"), none());
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc) -> ledger.capture.do\n  ledger.capture.do(payment: \"p\", amount: a)\n"), ["E204"]);
    assert_eq!(codes(&t, "fn f(a: money[USDc] range >=1USDc <=9USDc) -> ledger.capture.do\n  ledger.capture.hold(payment: \"p\", seller: \"s\", amount: a)\n"), ["E204"]);
    assert_eq!(codes(&t, "fn f() -> number\n  ledger.refund.do(payment: \"p\")\n"), ["E204"]);
    // one that holds first takes `hold`, `post` and `void`: `post` and `void` the key, `post` every
    // amount or none; a count with a name is a `number`
    assert_eq!(codes(&t, "fn f(q: number range >=1 <=9) -> stock.reserve.hold\n  stock.reserve.hold(order: \"o\", sku: \"s\", qty: q)\n"), none());
    assert_eq!(codes(&t, "fn f() -> stock.reserve.post\n  stock.reserve.post(order: \"o\", sku: \"s\")\n"), none());
    assert_eq!(codes(&t, "fn f(q: number range >=1 <=9) -> stock.reserve.post\n  stock.reserve.post(order: \"o\", sku: \"s\", qty: q)\n"), none());
    assert_eq!(codes(&t, "fn f() -> stock.reserve.void\n  stock.reserve.void(order: \"o\", sku: \"s\")\n"), none());
    assert_eq!(codes(&t, "fn f() -> stock.reserve.do\n  stock.reserve.do(order: \"o\", sku: \"s\", qty: 1)\n"), ["E101", "E204"]);
    // the amount in dollars where the book counts cents (DESIGN 17.6)
    let head = "module m v1\n\nuse book ledger from \"books/ledger.book\"\n\n";
    let o = check_in(&t, &format!("{head}fn f(fee: money[USD] range >=0USD <=9USD) -> ledger.processing_fee.do\n  ledger.processing_fee.do(payment: \"p\", seller: \"s\", fee: fee)\n"));
    assert_eq!(common::codes(&o), ["E102"]);
    let said = text(&o);
    assert!(said.contains("The argument `fee` of the transfer `ledger.processing_fee.do` is money[USDc]; this is money[USD]"), "{said}");
    assert!(said.contains("The unit `USD scale 2` of the book `ledger` counts money[USDc]."), "{said}");
    assert!(said.contains("`<value> in USDc` converts it"), "{said}");
    // two modules that read one book have one type of each of its transfers
    t.write("other.kn", format!("{head}fn g(a: money[USDc] range >=1USDc <=9USDc) -> ledger.capture.do\n  ledger.capture.do(payment: \"p\", seller: \"s\", amount: a)\n"));
    let o = check_in(&t, &format!("{head}use module other from \"other.kn\"\n\nfn f() -> ledger.capture.do\n  other.g(a: 5USDc)\n"));
    assert!(o.diags.is_empty(), "{}", text(&o));
}

#[test]
fn records_lists_and_optionals() {
    let t = project();
    let line = "record Line\n  label  : string\n  amount : money[USDc]  range >=1USDc <=99USDc\n  note   : string?\n";
    assert_eq!(codes(&t, &format!("{line}fn f() -> Line\n  Line {{ label: \"a\", amount: 5USDc }}\n")), none());
    assert_eq!(codes(&t, &format!("{line}fn f() -> Line\n  Line {{ label: \"a\" }}\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{line}fn f() -> Line\n  Line {{ label: \"a\", amount: 5USDc, size: 1 }}\n")), ["E101"]);
    assert_eq!(codes(&t, &format!("{line}fn f() -> Line\n  Line {{ label: \"a\", label: \"b\", amount: 5USDc }}\n")), ["E107"]);
    assert_eq!(codes(&t, &format!("{line}fn f(l: Line?) -> string\n  l.label\n")), ["E105"]);
    assert_eq!(codes(&t, &format!("{line}fn f(l: Line) -> string\n  l.size\n")), ["E101"]);
    // a list of one type, `none` and `[]` meet what they go to
    assert_eq!(codes(&t, &format!("{line}fn f(l: Line) -> list[Line]\n  [l, l]\n")), none());
    assert_eq!(codes(&t, "fn f() -> list[number]\n  []\n"), none());
    assert_eq!(codes(&t, "fn f() -> list[number]\n  [1, \"a\"]\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f(a: bool) -> number?\n  if a then 1 else none\n"), none());
    assert_eq!(codes(&t, "fn f(a: bool) -> number\n  if a then 1 else none\n"), ["E102"]);
    assert_eq!(codes(&t, "fn f() -> number\n  let x = none\n  1\n"), ["E102"]);
    // a rule's element, built for the list the rule walks
    assert_eq!(codes(&t, &format!("{line}fn f(l: Line) -> shares.outputs\n  shares.lines {{ amount: l.amount }}\n")), ["E101"]);
}

#[test]
fn builtins() {
    let t = project();
    let line = "record Line\n  sku    : string\n  qty    : number  range >=1 <=9\n  amount : money[USDc]  range >=1USDc <=99USDc\n";
    let lines = "lines: list[Line] count >=1 <=9";
    for (body, want) in [
        ("count(lines) + count(lines, l => l.qty > 1)", "number"),
        ("sum(lines, l => l.amount * l.qty)", "money[USDc]"),
        ("max_of(lines, l => l.amount)", "money[USDc]?"),
        ("first(lines, l => starts_with(l.sku, \"A\"))", "Line?"),
        ("map(filter(lines, l => any(lines, m => m.qty = l.qty)), l => l.amount)", "list[money[USDc]]"),
        ("reduce(lines, 0, (acc, l) => acc + l.qty)", "number"),
        ("concat(\"x\", text(count(lines)), text(2026-01-01), text(sum(lines, l => l.amount)))", "string"),
        ("all(lines, l => l.qty >= 1)", "bool"),
        ("mod(sum(lines, l => l.qty), 7) + isqrt(9)", "number"),
        ("allocate(100USDc, 3USDc, 7USDc)", "money[USDc]"),
        ("min(1USDc, max(2USDc, 3USDc))", "money[USDc]"),
        ("map(running_totals(lines, l => l.amount), r => r.upto - r.before)", "list[money[USDc]]"),
    ] {
        assert_eq!(codes(&t, &format!("{line}fn f({lines}) -> {want}\n  {body}\n")), none(), "{body}");
    }
    for (body, want, code) in [
        ("sum(lines)", "money[USDc]", "E107"),
        ("sum(lines, 3)", "money[USDc]", "E109"),
        ("sum(lines, (a, b) => a)", "money[USDc]", "E109"),
        ("sum(lines, l => l.sku)", "money[USDc]", "E109"),
        ("count(xs: lines)", "number", "E107"),
        ("text(3%)", "string", "E109"),
        ("text(sum(lines, l => l.amount * 3%))", "string", "E109"),
        ("mod(3, 2USDc)", "number", "E109"),
        ("concat(\"a\", 1)", "string", "E102"),
        ("sum(3, l => 1)", "number", "E102"),
        ("reduce(lines, 0, (acc, l) => l.sku)", "number", "E102"),
        ("min(1USDc, 2kg)", "money[USDc]", "E102"),
    ] {
        assert_eq!(codes(&t, &format!("{line}fn f({lines}) -> {want}\n  {body}\n")), [code], "{body}");
    }
}

/// Dicts, sets and the builtins written after a value (DESIGN 21.4, 24): the type of each, a key of
/// a type that cannot be one (E114), a builtin in the form it does not take (E101), a dict literal's
/// key given twice (E107), an unrounded value a dict or a set holds (E103), the whole numbers the
/// builtins of lists take (E109).
#[test]
fn dicts_sets_and_the_builtins_written_after_a_value() {
    let t = project();
    let line = "record Line\n  sku    : string\n  qty    : number  range >=1 <=9\n  amount : money[USDc]  range >=1USDc <=99USDc\n";
    let params = "lines: list[Line] count >=1 <=9, prices: dict[string, money[USDc]] count <=9, skus: set[string]";
    for (body, want) in [
        ("prices.get(\"tea\")", "money[USDc]?"),
        ("prices.put(\"tea\", 3USDc).remove(\"cake\")", "dict[string, money[USDc]]"),
        ("prices.has(\"tea\") and skus.has(\"tea\")", "bool"),
        ("prices.keys().concat(skus.to_list())", "list[string]"),
        ("prices.values()", "list[money[USDc]]"),
        ("prices.entries()", "list[entry[string, money[USDc]]]"),
        ("prices.entries().map(e => e.key)", "list[string]"),
        ("prices.count() + skus.count() + count(prices)", "number"),
        ("prices.map_values(p => p * 2)", "dict[string, money[USDc]]"),
        ("prices.filter(e => e.value > 1USDc).merge(prices)", "dict[string, money[USDc]]"),
        ("prices.merge_with(prices, (a, b) => a + b)", "dict[string, money[USDc]]"),
        ("skus.add(\"a\").union(skus).intersect(skus).difference(skus).remove(\"b\")", "set[string]"),
        ("[\"a\": 1, \"b\": 2]", "dict[string, number]"),
        ("[:]", "dict[string, number]"),
        ("[:].put(\"a\", 1USDc)", "dict[string, money[USDc]]"),
        ("[\"a\"].to_set()", "set[string]"),
        ("[].to_set()", "set[string]"),
        ("lines.reverse().take(2).drop(1).slice(0, 1).concat(lines).append(Line { sku: \"a\", qty: 1, amount: 5USDc })", "list[Line]"),
        ("lines.at(0)", "Line?"),
        ("lines.first()", "Line?"),
        ("lines.last(l => l.qty > 1)", "Line?"),
        ("lines.sort_by(l => l.amount).sort_by_desc(l => l.sku)", "list[Line]"),
        ("lines.sort_by(l => l)", "list[Line]"),
        ("lines.map(l => l.qty).sort().distinct()", "list[number]"),
        ("lines.max_by(l => l.qty)", "Line?"),
        ("lines.min_by(l => l.sku)", "Line?"),
        ("lines.map(l => l.sku).contains(\"a\")", "bool"),
        ("lines.map(l => [l.qty]).flatten()", "list[number]"),
        ("lines.flat_map(l => [l.qty, l.qty])", "list[number]"),
        ("lines.filter_map(l => if l.qty > 1 then l.sku else none)", "list[string]"),
        ("lines.indexed().map(i => i.index + i.item.qty)", "list[number]"),
        ("lines.map(l => l.sku).to_set()", "set[string]"),
        ("lines.to_dict(l => l.sku, l => l.amount)", "dict[string, money[USDc]]"),
        ("lines.group_by(l => l.sku)", "dict[string, list[Line]]"),
        ("lines.map(l => l.sku).join(\", \")", "string"),
        ("lines.sum(l => l.amount)", "money[USDc]"),
        ("numbers(1, count(lines))", "list[number]"),
        ("mod(count(lines), -7) + mod(count(lines), count(lines))", "number"),
    ] {
        assert_eq!(codes(&t, &format!("{line}fn f({params}) -> {want}\n  {body}\n")), none(), "{body}");
    }
    for (body, want, code) in [
        // a key of a type that cannot be one
        ("lines.sort()", "list[Line]", "E114"),
        ("lines.map(l => [l.qty]).to_set().count()", "number", "E114"),
        ("lines.group_by(l => l.sku = \"a\" or l.qty > 1).count() + lines.sort_by(l => [l.qty]).count()", "number", "E114"),
        // a key or a value a dict holds that is not rounded
        ("lines.group_by(l => l.amount / 3).count()", "number", "E103"),
        ("lines.to_dict(l => l.sku, l => l.amount * 8%)", "dict[string, money[USDc]]", "E103"),
        // the form a builtin does not take
        ("keys(prices)", "list[string]", "E101"),
        ("lines.numbers(3)", "list[number]", "E101"),
        ("lines.explode(\",\")", "list[string]", "E101"),
        ("lines.split(\",\")", "list[string]", "E102"),
        // a dict literal's key twice, the arguments after a value
        ("[\"a\": 1, \"a\": 2]", "dict[string, number]", "E107"),
        ("prices.put(\"a\")", "dict[string, money[USDc]]", "E107"),
        ("lines.sort_by(l => l.sku, 1)", "list[Line]", "E107"),
        // the types the builtins take
        ("prices.get(1)", "money[USDc]?", "E102"),
        ("skus.get(\"a\")", "string?", "E102"),
        ("prices.union(prices)", "dict[string, money[USDc]]", "E102"),
        ("[\"a\": 1, 2: 2]", "dict[string, number]", "E102"),
        ("lines.take(1USDc)", "list[Line]", "E109"),
        ("numbers(1, 2USDc)", "list[number]", "E109"),
        ("mod(3USDc, 2)", "number", "E109"),
        ("lines.sort_by(1)", "list[Line]", "E109"),
    ] {
        assert_eq!(codes(&t, &format!("{line}fn f({params}) -> {want}\n  {body}\n")), [code], "{body}");
    }
    // the types written: a key a dict or a set cannot have, a count on them
    assert_eq!(codes(&t, &format!("{line}fn f(d: dict[Line, number]) -> number\n  d.count()\n")), ["E114"]);
    assert_eq!(codes(&t, "fn f(s: set[number?]) -> number\n  s.count()\n"), ["E114"]);
    assert_eq!(codes(&t, "fn f(s: set[number] range >=0 <=9 count >=1 <=3, d: dict[string, date] range >=2026-01-01 <=2026-12-31 count <=3) -> number\n  s.count() + d.count()\n"), none());
    // `numbers` is a builtin's name; the names of the builtins written after a value are free
    assert_eq!(codes(&t, "fn numbers() -> number\n  1\n"), ["E006"]);
    assert_eq!(codes(&t, "fn get(n: number range >=0 <=9) -> number\n  n\n\nfn sort_by() -> number\n  get(n: 1)\n"), none());
}

/// The weights of `running_totals` fill the fields `before`, `upto` and `total` of the records it
/// answers, which take a rounded value (DESIGN 3.2, 3.6): an unrounded weight is E103, said where it
/// is written, with the way to round it. The module is the shape the generated code could not hold
/// (`1 / x`, whose running totals are halves and thirds; PLAN 10), in English names and in Japanese.
#[test]
fn the_weights_of_running_totals_are_rounded() {
    let t = project();
    let parts = "module weights v1\n\nfn parts(xs: list[number] count >=1 <=3 range >=2 <=3) -> list[number]\n  map(running_totals(xs, x => 1 / x), r => down(r.upto, 1))\n";
    let o = check_in(&t, parts);
    assert_eq!(common::codes(&o), ["E103"], "{}", text(&o));
    let said = text(&o);
    assert!(said.contains("main.kn:4:31: An unrounded value goes to the weight of `running_totals`: a value of number divided by a value"), "{said}");
    assert!(said.contains("Write the way of rounding and the step, as in `half_up(1 / x, 1)`."), "{said}");
    let ja = "module 重み(weights) v1\n\nfn 部分(parts)(数(xs): list[number] count >=1 <=3 range >=2 <=3) -> list[number]\n  map(running_totals(数, 一つ => 1 / 一つ), 途中 => down(途中.upto, 1))\n";
    let o = check_in(&t, ja);
    assert_eq!(common::codes(&o), ["E103"], "{}", text(&o));
    let said: String = o.diags.iter().map(|d| d.render(Lang::Ja)).collect();
    assert!(said.contains("main.kn:4:31: 丸めていない値を、`running_totals` の重みに渡しています（値で割った number の値）"), "{said}");
    assert!(said.contains("丸め方と刻みを、`half_up(1 / 一つ, 1)` のように書いてください。"), "{said}");
    // a whole weight goes: an amount rounded, or one times a number; a rate goes as a count of the
    // step its type says, `0%` too; an amount times a rate, a rate divided by a value and a number
    // divided by a constant are unrounded, said at the branch that is
    let line = "record Line\n  qty    : number  range >=1 <=9\n  amount : money[USDc]  range >=1USDc <=99USDc\n  share  : rate[step 0.01%]  range >=0% <=50%\n  free   : bool\n";
    for (weight, want) in [
        ("l => half_up(l.amount * 3%, 1USDc)", none()),
        ("l => l.amount * 2", none()),
        ("l => if l.free then 0 else l.amount", none()),
        ("l => l.share", none()),
        ("l => l.share * l.qty", none()),
        ("l => 0%", none()),
        ("l => l.amount * 3%", vec!["E103"]),
        ("l => if l.free then 0 else l.amount * 3%", vec!["E103"]),
        ("l => l.share / l.qty", vec!["E103"]),
        ("l => l.qty / 2", vec!["E103"]),
    ] {
        assert_eq!(codes(&t, &format!("{line}fn f(lines: list[Line] count >=1 <=9) -> number\n  count(running_totals(lines, {weight}))\n")), want, "{weight}");
    }
    let head = "module m v1\n\n";
    let call = "  count(running_totals(lines, l => if l.free then 0 else l.amount * 3%))";
    let o = check_in(&t, &format!("{head}{line}\nfn f(lines: list[Line] count >=1 <=9) -> number\n{call}\n"));
    let col = call.find("l.amount * 3%").unwrap() + 1;
    assert!(o.diags.iter().any(|d| d.code == "E103" && d.line == Some(10) && d.col == Some(col)), "{}", text(&o));
    let o = check_in(&t, &format!("{head}{line}\nfn f(lines: list[Line] count >=1 <=9) -> number\n  count(running_totals(lines, l => l.share / l.qty))\n"));
    assert!(text(&o).contains("as in `half_up(l.share / l.qty, 1%)`"), "{}", text(&o));
    // the records of an unrounded weight are of no type: the place they go to says nothing more
    assert_eq!(codes(&t, &format!("{line}fn f(lines: list[Line] count >=1 <=9) -> list[running[Line, money[USDc]]]\n  running_totals(lines, l => l.amount * 3%)\n")), ["E103"]);
}

#[test]
fn locals_and_words_that_read_two_ways() {
    let t = project();
    // a name means one thing in a function (E006)
    assert_eq!(codes(&t, "fn f(x: number range >=0 <=9) -> number\n  let x = 1\n  x\n"), ["E006"]);
    assert_eq!(codes(&t, "fn f(x: number range >=0 <=9) -> number\n  let ledger = 1\n  x\n"), ["E006"]);
    assert_eq!(codes(&t, "fn f(xs: list[number] count <=3 range >=0 <=9) -> number\n  sum(xs, xs => 1)\n"), ["E006"]);
    // a call names a function or a builtin, even where a local has its name: no local is called
    assert_eq!(codes(&t, "fn f(xs: list[number] count <=3 range >=0 <=9) -> number\n  let count = count(xs)\n  count + count(xs, x => x > 1)\n"), none());
    // the same name in two blocks side by side is two names
    assert_eq!(codes(&t, "fn f(a: bool) -> number\n  if a\n    let y = 1\n    y\n  else\n    let y = 2\n    y\n"), none());
    // a word that is a name and a value of the enum where it is read (E110)
    assert_eq!(codes(&t, "enum tier = basic | gold\nfn f(t: tier, gold: bool) -> bool\n  t = gold\n"), ["E110"]);
    assert_eq!(codes(&t, "enum tier = basic | gold\nfn f(t: tier, gold: bool) -> bool\n  t = tier.gold and gold\n"), none());
    // a value of an enum where no type says which: `tier.gold`
    let o = check_in(&t, "module m v1\n\nenum tier = basic | gold\n\nfn f() -> bool\n  let g = gold\n  true\n");
    assert_eq!(common::codes(&o), ["E101"]);
    assert!(text(&o).contains("write `tier.gold` where no type says which enum"), "{}", text(&o));
}

#[test]
fn examples_are_calls_with_answers_of_their_type() {
    let t = project();
    // the answer is above its range at 5 only, which no vector tries: the analysis cannot decide it
    // (W301), and the example that fails there runs
    let peak = "fn peak(q: number range >=1 <=8) -> number range >=0 <=24\n  q * (10 - q)\n";
    let o = check_in(&t, &format!("module m v1\n\n{peak}examples\n  peak(q: 2) = 16\n  peak(q: 5) fails range\n"));
    assert_eq!(common::codes(&o), ["W301"], "{}", text(&o));
    assert_eq!(codes(&t, &format!("{peak}examples\n  peak(q: 5) = 25\n")), ["E601"]);
    let double = "fn double(n: number range >=0 <=9) -> number range >=0 <=20\n  n * 2\n";
    assert_eq!(codes(&t, &format!("{double}examples\n  double(n: 2) = true\n")), ["E602"]);
    assert_eq!(codes(&t, &format!("{double}examples\n  card_fee(amount: 1USDc, international: true) = 1\n")), ["E602"]);
    assert_eq!(codes(&t, &format!("{double}examples\n  double(m: 2) = 4\n")), ["E107", "E107"]);
}

/// A rule that walks a list (rulec's `elements`): the list is given as a list of its elements'
/// records, its inputs, its list and its fields by their names or their aliases, and its enum is
/// split by its values' names or aliases.
#[test]
fn a_rule_that_walks_a_list() {
    let t = project();
    let head = "module m v1\n\nuse rule due from \"rules/earliest.rule\"\n\nrecord Task\n  on   : date  range >=2026-01-01 <=2027-12-31\n  done : bool\n\n";
    let body = "fn level(today: date range >=2026-01-01 <=2027-12-31, tasks: list[Task] count <=50) -> due.緊急度\n  due(今日: today, 週末: today, 作業: map(tasks, t => due.作業 { 期日: t.on, 完了: t.done })).区分\n\nfn by_alias(today: date range >=2026-01-01 <=2027-12-31, tasks: list[Task] count <=50) -> due.urgency\n  due(today: today, week_end: today, tasks: map(tasks, t => due.tasks { due: t.on, done: t.done })).level\n\nfn urgent(today: date range >=2026-01-01 <=2027-12-31, tasks: list[Task] count <=50) -> bool\n  match level(today: today, tasks: tasks)\n    overdue, 今週中 => true\n    due.urgency.later => false\n";
    let o = check_in(&t, &format!("{head}{body}"));
    assert!(common::codes_before_analysis(&o).is_empty(), "{}", text(&o));
    // the list is a list of the rule's elements, not of the module's records
    let o = check_in(&t, &format!("{head}fn f(today: date range >=2026-01-01 <=2027-12-31, tasks: list[Task] count <=50) -> due.outputs\n  due(today: today, week_end: today, tasks: tasks)\n"));
    assert_eq!(common::codes(&o), ["E102"]);
    // the rule writes its enum with `|`: its values have no order
    let o = check_in(&t, &format!("{head}fn f(a: due.urgency, b: due.urgency) -> bool\n  a < b\n"));
    assert_eq!(common::codes(&o), ["E105"]);
}

/// A rule's enum as the port says it (DESIGN 3.5, 14.1): by the name and the alias the rule writes
/// (not the name of the generated type), in order where the rule writes `<` between its values, and
/// one type for an enum two rules read whole from one `enums` file.
#[test]
fn the_enums_of_rules() {
    let t = project();
    let corpus = Path::new("../rulec/tests/corpus");
    for f in ["member_tier_shipping.rule", "product_tax.rule", "product_shipping.rule", "shared/product_kinds.rule"] {
        t.write(&format!("rules/{f}"), std::fs::read(corpus.join(f)).unwrap());
    }
    // ordered: compared, and taken by `max`
    let tiers = "module m v1\n\nuse rule ship from \"rules/member_tier_shipping.rule\"\n\nfn higher(a: ship.tier, b: ship.tier) -> ship.tier\n  if a < b then b else a\n\nfn top(a: ship.tier, b: ship.tier) -> ship.tier\n  max(a, b)\n";
    let o = check_in(&t, tiers);
    assert!(common::codes_before_analysis(&o).is_empty(), "{}", text(&o));
    // the alias the rule writes names it; the generated type's name does not
    let o = check_in(&t, "module m v1\n\nuse rule due from \"rules/earliest.rule\"\n\nfn f(a: due.Urgency) -> bool\n  true\n");
    assert_eq!(common::codes(&o), ["E101"]);
    // one enum of one `enums` file, read whole by two rules: a value of the one goes to the other
    let kinds = "module m v1\n\nuse rule tax from \"rules/product_tax.rule\"\nuse rule ship from \"rules/product_shipping.rule\"\n\nfn parcel(k: tax.kind) -> ship.parcel\n  ship(item_kind: k).ships_as\n\nfn same(a: tax.kind, b: ship.kind) -> bool\n  a = b\n";
    let o = check_in(&t, kinds);
    assert!(common::codes_before_analysis(&o).is_empty(), "{}", text(&o));
    // two enums of their own rules are two types, whatever their values
    let o = check_in(&t, "module m v1\n\nuse rule ship from \"rules/member_tier_shipping.rule\"\nuse rule due from \"rules/earliest.rule\"\n\nfn f(a: ship.tier, b: due.urgency) -> bool\n  a = b\n");
    assert_eq!(common::codes(&o), ["E102"]);
}

/// An optional of an optional is one optional (DESIGN 3.6): `first` over a list of values that may be
/// none answers `T?`, as its value does, and the arm of `some x` has the value itself.
#[test]
fn an_optional_of_an_optional_is_one() {
    let t = project();
    let o = check_in(&t, "module m v1\n\nfn f(xs: list[number?] count >=0 <=3 range >=0 <=9) -> number\n  match first(xs, x => true)\n    none => 0\n    some v => v + 1\n");
    assert!(common::codes_before_analysis(&o).is_empty(), "{}", text(&o));
    let c = o.analysed.clone().unwrap();
    let ExprKind::Match(s, _) = &c.module.fns[0].body.answer.kind else { panic!() };
    assert_eq!(c.typed.types[s.id], Ty::Opt(Box::new(Ty::Num(NumTy::number()))));
}

/// The builtins of strings (DESIGN 25.2): written after a string (after a list of code points,
/// `to_text`), `count`, `starts_with`, `concat` and `text` in either form, `char` as a function of a
/// constant scalar value; a string's count of code points declared with `count`.
#[test]
fn the_builtins_of_strings() {
    let t = project();
    let params = "s: string count <=20, t: string, n: number range >=0 <=9, ns: list[number] count <=3";
    for (body, want) in [
        ("s.count() + count(s) + s.chars().count() + s.code_points().count()", "number"),
        ("ns.to_text()", "string?"),
        ("[char(10), char(128077)].join(\"\")", "string"),
        ("concat(s.slice(n, 3), s.take(n), s.drop(1))", "string"),
        ("s.index_of(t)", "number?"),
        ("s.contains(t) and starts_with(s, t) and s.starts_with(t) and s.ends_with(t)", "bool"),
        ("s.split(\",\").concat(s.words())", "list[string]"),
        ("s.replace(t, \"-\").trim().trim_start().trim_end().lower_ascii().upper_ascii()", "string"),
        ("s.pad_start(n, \"0\").pad_end(8, t)", "string"),
        ("s.parse_number()", "number?"),
        ("s.parse_date()", "date?"),
        ("[s < t, s <= t, s > t, s >= t, s = t, s != t]", "list[bool]"),
        ("[min(s, t), max(s, \"a\")]", "list[string]"),
        ("[s, t].sort()", "list[string]"),
        ("s.concat(t, text(n))", "string"),
    ] {
        assert_eq!(codes(&t, &format!("fn f({params}) -> {want}\n  {body}\n")), none(), "{body}");
    }
    for (body, want, code) in [
        // a builtin of strings written as a function, `char` after a value
        ("trim(s)", "string", "E101"),
        ("split(s, \",\")", "list[string]", "E101"),
        ("n.char()", "string", "E101"),
        // what a builtin of strings is not called on, and is not given
        ("n.trim()", "string", "E102"),
        ("ns.words()", "list[string]", "E102"),
        ("s.to_text()", "string?", "E102"),
        ("[s].to_text()", "string?", "E102"),
        ("s.index_of(n)", "number?", "E102"),
        ("s.slice(\"a\", 2)", "string", "E109"),
        ("s.pad_start(1USDc, \"0\")", "string", "E109"),
        ("count(s, c => true)", "number", "E102"),
        ("min(s, n)", "string", "E102"),
        // `char` of a constant scalar value only
        ("char(n)", "string", "E109"),
        ("char(55296)", "string", "E109"),
        ("char(1114112)", "string", "E109"),
        ("char(\"a\")", "string", "E109"),
        // the arguments after the string
        ("s.replace(t)", "string", "E107"),
        ("s.trim(t)", "string", "E107"),
    ] {
        assert_eq!(codes(&t, &format!("fn f({params}) -> {want}\n  {body}\n")), [code], "{body}");
    }
    // a count of code points on a string, its fields and its answer; not on a number
    assert_eq!(codes(&t, "record R\n  name : string  count >=1 <=40\n\nfn f(r: R, s: string? count <=3) -> string count <=40\n  r.name\n"), none());
    assert_eq!(codes(&t, "fn f(n: number count <=3) -> number\n  n\n"), ["E108"]);
    // a module's function may take the name of a builtin written after a value, not of `char`
    assert_eq!(codes(&t, "fn split(s: string) -> list[string]\n  s.split(\",\")\n\nfn g() -> list[string]\n  split(s: \"a,b\")\n"), none());
    assert_eq!(codes(&t, "fn char(n: number range >=0 <=9) -> number\n  n\n"), ["E006"]);
}

/// Generic functions, records and enums, `result` and `try` (DESIGN 26): the type arguments worked
/// out from the arguments, then from the place, or written after the name; `K: key` for what orders
/// or keys a value of a type parameter; a ring of calls of generic functions, which gives them their
/// type parameters as they are, swapped, or types that name none; E112 for a type argument that
/// cannot be worked out, given in the wrong number, of two types or of a type whose values may be
/// none, and for a type argument made from a type parameter given in a ring of calls; E114, E105 and
/// E108 for what a value of a type parameter cannot be or take; E115 for `try` where it cannot go.
#[test]
fn generics_results_and_try() {
    let t = project();
    let decls = "record Page[T]\n  items : list[T]  count <=100\n  next  : string?\n\nenum Found[T]\n  hit\n    value : T\n  miss\n\nfn first_or[T](xs: list[T], fallback: T) -> T\n  match xs.first()\n    none => fallback\n    some x => x\n\nfn empty_page[T]() -> Page[T]\n  Page { items: [], next: none }\n\nfn largest[K: key](xs: list[K]) -> K?\n  xs.sort().last()\n\nfn half(n: number range >=0 <=9) -> result[number, string]\n  if n = 0 then err(\"zero\") else ok(n)\n\n";
    for body in [
        // worked out from the arguments, from the place, written after the name
        "fn f(xs: list[money[USDc]]) -> money[USDc]\n  first_or(xs: xs, fallback: 0USDc)\n",
        "fn f() -> Page[string]\n  empty_page()\n",
        "fn f() -> number\n  let p = empty_page[string]()\n  p.items.count()\n",
        "fn f() -> number\n  let p : Page[number] = empty_page()\n  p.items.count()\n",
        "fn f(xs: list[string]) -> Found[string]\n  match xs.first()\n    none => Found.miss\n    some x => Found.hit { value: x }\n",
        "fn f(p: Page[Page[number]]) -> number\n  p.items.count()\n",
        // a type parameter held to keys, given keys
        "fn f(xs: list[date]) -> date?\n  largest(xs: xs)\n",
        "fn smaller[K: key](a: K, b: K) -> K\n  if a < b then a else b\n\nfn g(a: string, b: string) -> string\n  smaller(a: a, b: b)\n",
        "fn count_of[K: key](xs: list[K]) -> dict[K, number]\n  xs.group_by(x => x).map_values(g => g.count())\n",
        // values of a type parameter passed on, held, compared with `=`
        "fn has[T](xs: list[T], x: T) -> bool\n  xs.contains(x) or xs.first() = x\n",
        "fn wrap[T](x: T) -> result[list[T], string]\n  ok([x])\n",
        // results made, matched, passed on with `try`, in a branch the answer comes from
        "fn f(n: number range >=0 <=9) -> result[number, string]\n  let h = try half(n: n)\n  ok(h + 1)\n",
        "fn f(n: number range >=0 <=9, strict: bool) -> result[number, string]\n  if strict\n    let h = try half(n: n)\n    ok(h)\n  else\n    ok(0)\n",
        "fn f(xs: list[number]) -> number?\n  let x = try xs.first()\n  x + 1\n",
        "fn f(r: result[number, string]) -> string\n  match r\n    ok n => text(n)\n    err e => e\n",
        "fn f(a: result[number, string], b: result[number, string]) -> bool\n  a = b\n",
        // a ring of calls gives its generic functions their type parameters as they are, swapped, or
        // types that name none, directly, through another function and as a value
        "fn dive[T](x: T, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else dive(x: x, n: n - 1)\n",
        "fn spin[A, B](a: A, b: B, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else spin(a: b, b: a, n: n - 1)\n",
        "fn settle[T](x: T, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else settle(x: n, n: n - 1)\n",
        "fn ping[T](x: T, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else pong(x: x, n: n - 1)\n\nfn pong[U](x: U, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else ping(x: [1], n: n - 1)\n",
        "fn sizes[T](xs: list[T]) -> list[number]\n  [xs].map(lengths)\n\nfn lengths[T](xs: list[T]) -> number\n  sizes(xs: xs).count()\n",
        // outside a ring, a type made from a type parameter goes anywhere
        "fn wrapped[T](x: T) -> number\n  first_or(xs: [[x]], fallback: []).count()\n",
    ] {
        assert_eq!(codes(&t, &format!("{decls}{body}")), none(), "{body}");
    }
    // a type argument made from a type parameter, given in a ring of calls: directly, through another
    // function, as a value (one E112 at each call that gives one)
    for (body, n) in [
        ("fn wrap_deep[T](x: T, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else wrap_deep(x: [x], n: n - 1)\n", 1),
        ("fn ping[T](x: T, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else pong(x: [x], n: n - 1)\n\nfn pong[U](x: U, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else ping(x: x, n: n - 1)\n", 1),
        ("fn sizes[T](xs: list[T]) -> list[number]\n  [[xs]].map(lengths)\n\nfn lengths[T](xs: list[T]) -> number\n  sizes(xs: xs).count()\n", 1),
        ("fn boxes[T](x: T, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else boxes(x: Page { items: [x], next: none }, n: n - 1) + boxes(x: Found.hit { value: x }, n: n - 1)\n", 2),
    ] {
        assert_eq!(codes(&t, &format!("{decls}{body}")), vec!["E112"; n], "{body}");
    }
    for (body, code) in [
        // a type parameter no argument and no place says, two types for one, the wrong number, `T?`
        ("fn f() -> number\n  let p = empty_page()\n  1\n", "E112"),
        ("fn f(d: list[money[USD]], c: money[USDc]) -> money[USD]\n  first_or(xs: d, fallback: c)\n", "E112"),
        ("fn f() -> number\n  let p = empty_page[number, string]()\n  1\n", "E112"),
        ("fn f(p: Page) -> number\n  1\n", "E112"),
        ("fn f(xs: list[number?]) -> number?\n  first_or(xs: xs, fallback: none)\n", "E112"),
        ("fn f(p: Page[number?]) -> number\n  1\n", "E112"),
        ("fn f() -> string\n  ok(\"a\")\n", "E112"),
        // what a value of a type parameter cannot be or take
        ("fn f[T](a: T, b: T) -> bool\n  a < b\n", "E114"),
        ("fn f[T](xs: list[T]) -> list[T]\n  xs.sort()\n", "E114"),
        ("fn f(xs: list[Page[number]]) -> Page[number]?\n  largest(xs: xs)\n", "E114"),
        ("fn f[T](x: T) -> T\n  x + x\n", "E105"),
        ("fn f[T](x: T range >=1) -> T\n  x\n", "E108"),
        // `try` where it cannot go
        ("fn f(n: number range >=0 <=9) -> result[number, string]\n  let b =\n    let h = try half(n: n)\n    h\n  ok(b)\n", "E115"),
        ("fn f(n: number range >=0 <=9) -> number\n  let h = try half(n: n)\n  h\n", "E115"),
        ("fn f(n: number range >=0 <=9) -> result[number, list[string]]\n  let h = try half(n: n)\n  ok(h)\n", "E115"),
        ("fn f(n: number range >=0 <=9) -> number?\n  let m = try n\n  m\n", "E115"),
        ("fn f(xs: list[number]) -> list[number]\n  xs.map(x => try half(n: x))\n", "E115"),
    ] {
        assert_eq!(codes(&t, &format!("{decls}{body}")), [code], "{body}");
    }
}

/// Function values (DESIGN 27): lambdas whose parameters take their types from the place or are
/// written, functions given by their names, function values named by a `let`, chosen by an `if`,
/// passed on and called; where a function value goes (E113), and how its types are held (E102, E105,
/// E107, E109, E112); `private` (DESIGN 28.2) and the types what is not `private` names (E108).
#[test]
fn function_values_and_what_is_private() {
    let t = project();
    let decls = "record Line\n  sku  : string  count >=1 <=8\n  unit : money[USDc]  range >=0USDc <=10_000USDc\n  qty  : number  range >=0 <=100\n\nfn total_by[T](xs: list[T], amount: (T) -> money[USDc]) -> money[USDc]\n  xs.sum(x => amount(x))\n\nfn apply_each[T](xs: list[T], step: (T) -> T) -> list[T]\n  xs.map(x => step(x))\n\nfn twice(step: (number) -> number, x: number range >=0 <=9) -> number\n  step(step(x))\n\nfn line_cost(l: Line) -> money[USDc]\n  l.unit * l.qty\n\nfn id[T](x: T) -> T\n  x\n\n";
    for body in [
        // a lambda given, its parameters' types from the place; a function given by its name
        "fn f(lines: list[Line] count <=9) -> money[USDc]\n  total_by(xs: lines, amount: l => l.unit * l.qty)\n",
        "fn f(lines: list[Line] count <=9) -> money[USDc]\n  total_by(xs: lines, amount: line_cost)\n",
        "fn f(lines: list[Line] count <=9) -> list[money[USDc]]\n  lines.map(line_cost)\n",
        "fn f(xs: list[number] count <=9 range >=0 <=9) -> list[number]\n  apply_each(xs: xs, step: x => x + 1)\n",
        // a lambda a `let` names, with its parameters' types written, chosen by an `if`, called, given
        "fn f(up: bool, x: number range >=0 <=9) -> number\n  let g = if up then (y: number) => y + 1 else (y: number) => y - 1\n  g(g(x))\n",
        "fn f(xs: list[number] count <=9 range >=0 <=9) -> list[number]\n  let g = (y: number) => y * 2\n  xs.map(g)\n",
        "fn f(x: number range >=0 <=9) -> number\n  let g = (y: number) => y * 2\n  twice(step: g, x: x)\n",
        // a function value passed on, and named by a `let`
        "fn f(g: (number) -> number, x: number range >=0 <=9) -> number\n  twice(step: g, x: x)\n",
        "fn f(g: (number) -> number) -> number\n  let h = g\n  h(1)\n",
        // effects written on function types
        "fn f(xs: list[number] count <=9, test: (number) -> bool total) -> bool\n  xs.all(x => test(x))\n",
        "fn f(g: (number) -> number fails overflow, x: number range >=0 <=9) -> number\n  g(x)\n",
        // what only the module uses
        "private record Bounds\n  low : number  range >=0 <=9\n\nprivate enum side = below | above\n\nprivate fn low(b: Bounds) -> side\n  if b.low < 5 then side.below else side.above\n\nfn f() -> bool\n  low(b: Bounds { low: 1 }) = side.below\n",
    ] {
        assert_eq!(codes(&t, &format!("{decls}{body}")), none(), "{body}");
    }
    for (body, code) in [
        // where a function value cannot go
        ("fn f(n: number range >=0 <=9) -> (number) -> number\n  x => x + n\n", "E113"),
        ("record Step\n  run : (number) -> number\n", "E113"),
        ("fn f(g: (number) -> number, h: (number) -> number) -> bool\n  g = h\n", "E113"),
        ("fn f() -> number\n  let gs = [(y: number) => y + 1]\n  1\n", "E113"),
        ("fn f(up: bool) -> number\n  let g = if up then (y: number) => y + 1 else none\n  1\n", "E113"),
        ("fn f() -> number\n  let g = apply_each\n  1\n", "E113"),
        ("fn f(l: Line) -> money[USDc]\n  line_cost + l.unit\n", "E113"),
        // the types of a lambda's parameters that nothing says; a function type as a type argument
        ("fn f() -> number\n  let g = x => x * 2\n  1\n", "E112"),
        ("fn f() -> number\n  let g = id(x: (y: number) => y + 1)\n  1\n", "E112"),
        ("fn f() -> number\n  let g = id(x: line_cost)\n  1\n", "E112"),
        // function types that do not agree, and what is no function given to one
        ("fn f() -> number\n  twice(step: (s: string) => 1, x: 1)\n", "E102"),
        ("fn f() -> number\n  twice(step: 3, x: 1)\n", "E102"),
        ("fn f(lines: list[Line] count <=9) -> money[USDc]\n  total_by(xs: lines, amount: (l: Line) => l.qty)\n", "E102"),
        ("fn f() -> number\n  twice(step: (a, b) => a, x: 1)\n", "E102"),
        // a function value called with names, with the wrong number of arguments; a value that is no
        // function called
        ("fn f(g: (number) -> number) -> number\n  g(x: 1)\n", "E107"),
        ("fn f(g: (number) -> number) -> number\n  g(1, 2)\n", "E107"),
        ("fn f(n: number range >=0 <=9) -> number\n  n(2)\n", "E105"),
        // a builtin as a value
        ("fn f(xs: list[number] count <=9 range >=0 <=99) -> list[number]\n  xs.map(isqrt)\n", "E109"),
        // a `private` type in what is not `private`
        ("private record Bounds\n  low : number  range >=0 <=9\n\nfn f(b: Bounds) -> number\n  b.low\n", "E108"),
        ("private enum side = below | above\n\nfn f() -> side\n  side.below\n", "E108"),
        ("private enum side = below | above\n\nrecord Mark\n  at : side\n", "E108"),
    ] {
        assert_eq!(codes(&t, &format!("{decls}{body}")), [code], "{body}");
    }
}
