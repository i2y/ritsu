//! The checks across the borders where a module of kaname calls a rule, a koyomi date or a chobo
//! transfer (kaname's DESIGN 14.3, 16.5), held to the real answers of rulec, koyomi and chobo and to
//! what kaname hands over through its port: a precondition a relation kaname keeps shows (the
//! spreading of a discount), one with no relation that the corner of the ranges breaks (an example
//! where a run of the function's vectors breaks it, undecided where none does), the days of a koyomi
//! date and a range of days kaname finds, and an amount that is no output of a rule (the payment
//! with the fee taken from it). `crates/ritsu/tests/cross.rs` reaches the same decisions from
//! projects, as `ritsu check` says them.

use ritsu_cross::ProgramVerdicts;
use ritsu_ports::{Answer, CallVerdict, Crossing, Value, Verdicts};
use ritsu_project::Joined;
use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kaname/examples").join(name)
}

/// An example laid out in its own directory, with its module changed by `edits`.
fn changed(name: &str, edits: &[(&str, &str)]) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("cross-programs-{name}"));
    ritsu_testkit::tmp::copy_dir(&example(name), t.path());
    let kn = t.path().join(format!("{name}.kn"));
    let mut src = std::fs::read_to_string(&kn).unwrap();
    for (from, to) in edits {
        assert!(src.contains(from), "{name} has no {from:?}");
        src = src.replacen(from, to, 1);
    }
    // the example's `examples` are its own: a changed module drops them
    if !edits.is_empty() {
        src = src.split("\nexamples\n").next().unwrap().to_string() + "\n";
    }
    std::fs::write(&kn, src).unwrap();
    (t, kn)
}

fn verdicts(kn: &Path) -> Vec<CallVerdict> {
    ProgramVerdicts::new(&Joined::new()).verdicts(kn)
}

/// The verdicts of one check, by what each is about.
fn of(vs: &[CallVerdict], check: Crossing) -> Vec<(String, &Answer<ritsu_ports::Seen>)> {
    vs.iter().filter(|v| v.check == check).map(|v| (v.about.clone(), &v.outcome)).collect()
}

#[test]
fn x2_a_relation_kaname_keeps_shows_the_precondition() {
    let vs = verdicts(&example("discount_split/discount_split.kn"));
    let pre = of(&vs, Crossing::Precondition);
    assert_eq!(pre.len(), 2, "{vs:?}");
    assert!(pre.iter().all(|(_, a)| **a == Answer::Holds), "{pre:?}");
    // the Japanese version holds the same, by its own names
    let vs = verdicts(&example("discount_split/discount_split.ja.kn"));
    assert!(of(&vs, Crossing::Precondition).iter().all(|(_, a)| **a == Answer::Holds));
}

#[test]
fn x2_the_corner_of_the_ranges_and_the_runs_of_the_vectors() {
    // `upto` put together by `reduce`: no relation, the corner breaks both, and no run does
    let (_t, kn) = changed("discount_split", &[("upto: r.upto", "upto: reduce([r.item.amount], r.before, (acc, x) => acc + x)")]);
    let vs = verdicts(&kn);
    let pre = of(&vs, Crossing::Precondition);
    assert_eq!(pre.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["before <= upto", "upto <= total"]);
    for (p, a) in &pre {
        let Answer::Undecided(why) = a else { panic!("{p}: {a:?}") };
        assert!(why.en.contains("at the ends of the ranges") && why.en.contains("no input among the function's vectors gives such values"), "{}", why.en);
    }
    assert!(vs.iter().filter(|v| v.check == Crossing::Precondition).all(|v| v.said.as_ref().is_some_and(|(c, _)| c == "W201")));
    // `before` and `upto` swapped: a run gives the rule a `before` above its `upto`
    let (_t, kn) = changed("discount_split", &[("before: r.before, upto: r.upto", "before: r.upto, upto: r.before")]);
    let vs = verdicts(&kn);
    let pre = of(&vs, Crossing::Precondition);
    let Some((_, Answer::Fails(seen))) = pre.iter().find(|(p, _)| p == "before <= upto") else { panic!("{pre:?}") };
    let given = |n: &str| seen.given.iter().find(|(m, _)| m == n).map(|(_, v)| v.clone());
    let (Some(Value::Int(before)), Some(Value::Int(upto))) = (given("before"), given("upto")) else { panic!("{seen:?}") };
    assert!(before > upto, "{seen:?}");
    assert!(seen.call.starts_with("split(lines: ["), "{}", seen.call);
    // `upto <= total` still holds: the running totals keep `before <= total`
    assert!(matches!(pre.iter().find(|(p, _)| p == "upto <= total"), Some((_, Answer::Holds))), "{pre:?}");
}

#[test]
fn x6_and_x3a_the_days_kaname_finds_and_the_days_of_koyomi() {
    // the invoice's day, in the range of the date's input: shown
    let vs = verdicts(&example("late_payment/late_payment.kn"));
    assert_eq!(of(&vs, Crossing::DateDay), [("invoiced".to_string(), &Answer::Holds)]);
    // the day paid given to the date instead: a run gives it a day past the input's range
    let (_t, kn) = changed("late_payment", &[("terms.due(invoiced: invoice.invoiced)", "terms.due(invoiced: paid_on)")]);
    let vs = verdicts(&kn);
    let days = of(&vs, Crossing::DateDay);
    let [(_, Answer::Fails(seen))] = &days[..] else { panic!("{days:?}") };
    let Some(Value::Date(d)) = seen.given.first().map(|(_, v)| v.clone()) else { panic!("{seen:?}") };
    assert!(d.as_str() > "2027-12-31", "{d}");
    // a koyomi date's day given to a rule whose range starts after the first of them (X3 (a))
    let t = TempDir::new("cross-programs-billing");
    t.write("payment_terms.cal", "dates payment_terms v1\ndescription \"Closes on the 20th and pays on the 10th of the next month\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20\n\ndate payment = closing\n  day 10 of month +1\n");
    let rule = "rule batch v1\ndescription \"The billing batch a payment day falls in\"\n\nenum run = spring | autumn\n\ninputs\n  pay_day : date  range >=2026-03-01 <=2027-01-31\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-08-31 | spring         |\n| >=2026-09-01 | autumn         |\n";
    t.write("batch.rule", rule);
    let kn = t.write("billing.kn", "module billing v1\n\nuse dates terms from \"payment_terms.cal\"\nuse rule batch from \"batch.rule\"\n\nfn pick(received: date range >=2026-01-01 <=2026-12-20) -> batch.run\n  let due = terms.payment(received: received)\n  batch(pay_day: due).batch\n");
    let vs = verdicts(&kn);
    let [(input, Answer::Fails(seen))] = &of(&vs, Crossing::RuleDay)[..] else { panic!("{vs:?}") };
    assert_eq!((input.as_str(), seen.call.as_str()), ("pay_day", "pick(received: 2026-01-01)"));
    assert_eq!(seen.given, [("pay_day".to_string(), Value::Date("2026-02-10".into()))]);
    // the rule's range holding every payment day: shown, from koyomi's days
    t.write("batch.rule", rule.replace(">=2026-03-01", ">=2026-02-01"));
    let vs = verdicts(&kn);
    assert_eq!(of(&vs, Crossing::RuleDay), [("pay_day".to_string(), &Answer::Holds)]);
    // narrowed by an `if` to the rule's range: shown, though koyomi's days reach before it
    t.write("batch.rule", rule);
    let kn = t.write("billing.kn", "module billing v1\n\nuse dates terms from \"payment_terms.cal\"\nuse rule batch from \"batch.rule\"\n\nfn pick(received: date range >=2026-01-01 <=2026-12-20) -> batch.run?\n  let due = terms.payment(received: received)\n  if due >= 2026-03-01\n    batch(pay_day: due).batch\n  else\n    none\n");
    let vs = verdicts(&kn);
    assert_eq!(of(&vs, Crossing::RuleDay), [("pay_day".to_string(), &Answer::Holds)]);
}

#[test]
fn x4_every_amount_wherever_it_comes_from_and_its_refusals() {
    // the payment's amount and the rule's fee: both amounts chobo takes, and the reasons found
    let vs = verdicts(&example("payout/payout.kn"));
    assert!(of(&vs, Crossing::Amount).iter().all(|(_, a)| **a == Answer::Holds), "{vs:?}");
    let refusals: Vec<(String, Option<Vec<String>>)> = vs.iter().filter(|v| v.check == Crossing::Refusals).map(|v| (v.about.clone(), v.refusals.clone())).collect();
    assert_eq!(
        refusals,
        [
            ("capture.do".to_string(), Some(vec!["key_conflict".to_string()])),
            ("processing_fee.do".to_string(), Some(vec!["insufficient_balance".to_string(), "key_conflict".to_string(), "already_refused".to_string()])),
        ]
    );
    // the fee taken from the amount the seller gets: a payment of a cent comes to −48
    let net = [("amount: payment.amount),", "amount: payment.amount - fee),")];
    let (_t, kn) = changed("payout", &net);
    let vs = verdicts(&kn);
    let amounts = of(&vs, Crossing::Amount);
    let Some((_, Answer::Fails(seen))) = amounts.iter().find(|(p, _)| p == "amount") else { panic!("{amounts:?}") };
    assert_eq!(seen.given, [("amount".to_string(), Value::Int(-48))]);
    assert!(seen.call.contains("amount: 1USDc"), "{}", seen.call);
    // with payments of a dollar or more no run comes below 0, though the ranges do: undecided
    let (_t, kn) = changed("payout", &[net[0], ("range >=1USDc <=100_000_000USDc", "range >=100USDc <=100_000_000USDc")]);
    let vs = verdicts(&kn);
    let amounts = of(&vs, Crossing::Amount);
    let Some((_, Answer::Undecided(why))) = amounts.iter().find(|(p, _)| p == "amount") else { panic!("{amounts:?}") };
    assert!(why.en.contains("in >=-4989949 <=99999951"), "{}", why.en);
}

/// A module whose names do not pass has no verdicts: kaname says what is wrong with it.
#[test]
fn a_module_that_does_not_pass_its_names_has_none() {
    let t = TempDir::new("cross-programs-wrong");
    let kn = t.write("main.kn", "module main v1\n\nfn f(n: number range >=0 <=9) -> number\n  m + 1\n");
    assert!(verdicts(&kn).is_empty());
}
