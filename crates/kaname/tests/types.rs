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
    assert_eq!(codes(&t, "fn f(a: string, b: string) -> bool\n  a < b\n"), ["E105"]);
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
    assert_eq!(codes(&t, "fn f(a: number range >=0 <=9) -> number\n  match a\n    true => 1\n    false => 0\n"), ["E105"]);
    // a rule's enum, by its values
    assert_eq!(codes(&t, &format!("{tier}fn f(t: tier) -> number\n  match t\n    basic => 1\n    silver => 2\n    gold => true\n")), ["E102"]);
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
    assert_eq!(codes(&t, &format!("{double}fn f() -> number\n  double\n")), ["E107"]);
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
    // the amount in dollars where the book counts cents (DESIGN 17.1)
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
        ("mod(3, 0)", "number", "E109"),
        ("concat(\"a\", 1)", "string", "E102"),
        ("sum(3, l => 1)", "number", "E102"),
        ("reduce(lines, 0, (acc, l) => l.sku)", "number", "E102"),
        ("min(1USDc, 2kg)", "money[USDc]", "E102"),
    ] {
        assert_eq!(codes(&t, &format!("{line}fn f({lines}) -> {want}\n  {body}\n")), [code], "{body}");
    }
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
