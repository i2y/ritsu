//! The warning of a decision written in code (DESIGN 7, W501): every shape DESIGN 7 warns of and
//! every one it does not, the shapes found at its edges, and the examples, which warn of
//! none.

mod common;

use ritsu_base::text::Lang;
use ritsu_testkit::TempDir;

/// A directory with the late payment's rule and dates file, and a rule of shipping that takes a
/// total and whether the order ships free.
fn project() -> TempDir {
    let t = TempDir::new("kaname-hatch");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/late_payment/rules"), &t.path().join("rules"));
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/late_payment/dates"), &t.path().join("dates"));
    t.write("rules/shipping.rule", include_str!("rules/shipping.rule"));
    t
}

const HEAD: &str = "module m v1\n\nuse rule shipping from \"rules/shipping.rule\"\nuse rule charges from \"rules/late_payment_charges.rule\"\nuse dates terms from \"dates/payment_terms.cal\"\n\nenum tier = basic | silver | gold\nenum band = basic | silver | gold\nenum level = low | high\n\nrecord Order\n  total : money[USD]  range >=0USD <=10_000USD\n\nrecord Fee\n  a : money[USD]\n  b : money[USD]\n\n";

/// The warnings of the function `body`, after the head that reads the rules: each its code and line
/// in the body.
fn warned(t: &TempDir, body: &str) -> Vec<(&'static str, usize)> {
    let f = t.write("m.kn", format!("{HEAD}{body}"));
    let o = common::check(f.to_str().unwrap());
    let lines_before = HEAD.lines().count();
    let codes: Vec<(&'static str, usize)> = o.diags.iter().filter(|d| d.code == "W501").map(|d| (d.code, d.line.unwrap_or(0) - lines_before)).collect();
    let others: Vec<&str> = o.diags.iter().filter(|d| d.code != "W501").map(|d| d.code).collect();
    assert!(others.is_empty(), "{body}\n{}", common::shown(&o, Lang::En));
    codes
}

#[test]
fn the_shapes_of_design_7() {
    let t = project();
    let w = |body: &str| warned(&t, body);
    // 1: an `if` that chooses between constants, a `match` that answers one for each value
    assert_eq!(w("fn shipping_fee(total: money[USD] range >=0USD <=10_000USD) -> money[USD]\n  if total >= 35USD then 0USD else 5USD\n"), [("W501", 2)]);
    assert_eq!(w("fn surcharge(t: tier) -> money[USD]\n  match t\n    gold => 0USD\n    silver, basic => 3USD\n"), [("W501", 2)]);
    // 2: a comparison with a threshold made a value
    assert_eq!(w("fn quote(order: Order) -> money[USD]\n  let free = order.total >= 35USD\n  shipping(total: order.total, free: free).fee\n"), [("W501", 2)]);
    // a branch that narrows the days to the rule's range, and answers none outside it: not warned of
    let claim = "fn claim(amount: money[GBPc] range >=1GBPc <=100_000_000GBPc, invoiced: date range >=2026-01-01 <=2027-12-31, paid_on: date range >=2026-01-01 <=2029-12-31) -> money[GBP]?\n  let late = paid_on - terms.due(invoiced: invoiced)\n  if late >= 1\n    charges(debt: amount, days_late: late, reference_rate: 0.5%).compensation\n  else\n";
    assert_eq!(w(&format!("{claim}    none\n")), []);
    // the same branch answering 0 pounds: the rule could decide that day too
    let f = t.write("m.kn", format!("{HEAD}{}", claim.replace("-> money[GBP]?", "-> money[GBP]") + "    0GBP\n"));
    let o = common::check(f.to_str().unwrap());
    assert_eq!(common::codes(&o), ["W501"]);
    let text = common::shown(&o, Lang::En);
    assert!(text.contains("This `if` chooses a constant of the business by the values it compares (`0GBP`): a decision, which belongs in a table\n"), "{text}");
    assert!(text.contains("  = Another branch calls a rule. If no answer is meant in this case, answer `none`; if the rule decides this case too, give it a row and widen the range of its input.\n"), "{text}");
    let ja = common::shown(&o, Lang::Ja);
    assert!(ja.contains("警告[W501]: ") && ja.contains("この `if` は、比べた値によって業務の定数（`0GBP`）を選んでいます。表に書く判断です\n"), "{ja}");
}

#[test]
fn what_is_not_warned_of() {
    let t = project();
    let w = |body: &str| warned(&t, body);
    let none: Vec<(&str, usize)> = Vec::new();
    // `none` and `some x` split, an optional's default
    assert_eq!(w("fn coupon(c: money[USDc]? range >=0USDc <=1000USDc) -> money[USDc]\n  match c\n    none => 0USDc\n    some d => d\n"), none);
    // one enum's values to those of the same names of another
    assert_eq!(w("fn to_band(t: tier) -> band\n  match t\n    basic => band.basic\n    silver => band.silver\n    gold => band.gold\n"), none);
    // a comparison with 0 made a value, a condition with 0 given to a builtin
    assert_eq!(w("fn paid(x: money[USD] range >=0USD <=100USD) -> bool\n  let p = x > 0USD\n  p\n"), none);
    assert_eq!(w("fn some_weight(xs: list[money[USD]] count >=0 <=9 range >=0USD <=9USD) -> number\n  count(xs, x => x > 0USD)\n"), none);
    // numbers answered by an `if`: the end of a recursion, the case of an empty list
    assert_eq!(w("fn countdown(n: number range >=0 <=100) -> number range >=0 <=100 diverges\n  if n <= 0 then 0 else min(countdown(n: n - 1) + 1, 100)\n"), none);
    assert_eq!(w("fn total(xs: list[money[USD]] count >=0 <=9 range >=0USD <=9USD) -> money[USD]\n  if count(xs) = 0 then 0 else sum(xs, x => x)\n"), none);
    // the answers of a rule chosen by its own output
    assert_eq!(w("fn fee(order: Order, free: bool) -> money[USD]?\n  let s = shipping(total: order.total, free: free)\n  if free then s.fee else none\n"), none);
    // a `match` that answers the values of an enum, `true` and `false`, numbers, a value with no
    // fields: kaname holds it to every value once (E106), and it is no table of amounts (DESIGN
    // 23.9)
    assert_eq!(w("fn points(t: tier) -> number\n  match t\n    basic => 1\n    silver => 2\n    gold => 3\n"), none);
    assert_eq!(w("fn to_level(t: tier) -> level\n  match t\n    basic => level.low\n    silver, gold => level.high\n"), none);
    assert_eq!(w("fn top(t: tier) -> bool\n  match t\n    gold => true\n    else => false\n"), none);
    assert_eq!(w("fn grade(code: string) -> number\n  match code\n    \"A\" => 3\n    else => 0\n"), none);
    let steps = "enum State\n  draft\n  sent\n    to : string\n  closed\n\nenum Event\n  send\n    to : string\n  close\n\n";
    assert_eq!(w(&format!("{steps}fn next(s: State, e: Event) -> State\n  match s\n    draft =>\n      match e\n        send x => State.sent {{ to: x.to }}\n        close => State.closed\n    sent => State.closed\n    closed => State.closed\n")), none);
    // a value with fields of no constant: a value moved from one state to the next
    assert_eq!(w(&format!("{steps}fn resend(s: State) -> State\n  match s\n    sent x => State.sent {{ ..x, to: \"again\" }}\n    else => s\n")), none);
}

#[test]
fn the_shapes_at_the_edges() {
    let t = project();
    let w = |body: &str| warned(&t, body);
    // `true` and `false` chosen by a comparison: the same decision as the comparison made a value
    assert_eq!(w("fn eligible(total: money[USD] range >=0USD <=10_000USD) -> bool\n  if total >= 35USD then true else false\n"), [("W501", 2)]);
    // a `match` that answers an amount, a date, a record of constants, a value with fields of
    // constants (DESIGN 23.9)
    assert_eq!(w("fn cutoff(t: tier) -> date\n  match t\n    gold => 2026-12-31\n    else => 2026-06-30\n"), [("W501", 2)]);
    assert_eq!(w("fn fees(t: tier) -> Fee\n  match t\n    gold => Fee { a: 0USD, b: 0USD }\n    else => Fee { a: 1USD, b: 2USD }\n"), [("W501", 2)]);
    assert_eq!(w("enum Charge\n  flat\n    fee : money[USD]\n  none_due\n\nfn charge(t: tier) -> Charge\n  match t\n    gold => Charge.none_due\n    else => Charge.flat { fee: 3USD }\n"), [("W501", 7)]);
    assert_eq!(w("fn by_code(code: string) -> money[USD]\n  match code\n    \"A\" => 1USD\n    else => 2USD\n"), [("W501", 2)]);
    // a chain of `else if`, warned of once
    assert_eq!(w("fn chain(x: money[USD] range >=0USD <=100USD) -> money[USD]\n  if x > 50USD\n    1USD\n  else if x > 20USD\n    2USD\n  else\n    3USD\n"), [("W501", 2)]);
    // a threshold in a condition given to a builtin, a negative threshold
    assert_eq!(w("fn heavy(xs: list[money[USD]] count >=0 <=9 range >=0USD <=9USD) -> number\n  count(xs, x => x > 2USD)\n"), [("W501", 2)]);
    assert_eq!(w("fn below(x: money[USD] range >=-100USD <=100USD) -> bool\n  x < -5USD\n"), [("W501", 2)]);
    // rates, dates and records of constants
    assert_eq!(w("fn surcharge(international: bool) -> rate[step 0.01%]\n  if international then 1.5% else 0%\n"), [("W501", 2)]);
    assert_eq!(w("fn cutoff(paid_on: date range >=2026-01-01 <=2027-12-31) -> date\n  if paid_on > 2026-12-31 then 2027-01-31 else paid_on\n"), [("W501", 2)]);
    assert_eq!(w("fn fees(x: money[USD] range >=0USD <=100USD) -> Fee\n  if x > 5USD then Fee { a: 1USD, b: 2USD } else Fee { a: 0USD, b: 0USD }\n"), [("W501", 2)]);
    // a `match` on a comparison is the `if` it reads as
    assert_eq!(w("fn split(x: money[USD] range >=0USD <=100USD) -> money[USD]\n  match x > 3USD\n    true => 1USD\n    false => 2USD\n"), [("W501", 2)]);
}

/// The examples, in English and in Japanese, warn of no decision (DESIGN 7, 17).
#[test]
fn the_examples_warn_of_none() {
    for f in common::EXAMPLES {
        let o = common::check(f);
        assert!(!common::codes(&o).contains(&"W501"), "{f}: {}", common::shown(&o, Lang::En));
    }
}
