//! The decisions of X3 (a), X4, X5 and X6 (`ritsu_cross::borders`) over what the languages hand
//! over through their ports, each to all three of its answers (held, an example, undecided), held
//! to real answers of koyomi, chobo and rulec. `crates/ritsu/tests/cross.rs` reaches the same
//! decisions from projects, where a flow calls them.

use ritsu_cross::borders::{amount_fits, amounts_given, amounts_hull, day_text, days_fit, days_given, held_until, input_range, refusals_met, AmountFrom, Unmet};
use ritsu_ports::{Answer, Books, Dates, Found, Rules};
use std::path::{Path, PathBuf};

fn crates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The days `payment` of rulec's test material comes to: the 10th of each month, 2026-02 to 2027-01.
fn payment_terms() -> PathBuf {
    crates().join("rulec/tests/days/payment_terms.cal")
}

#[test]
fn x3a_the_days_of_a_koyomi_date_against_a_rules_range() {
    let k = koyomi::ports::Engine;
    let days = k.values(&payment_terms(), "payment").unwrap();
    assert_eq!(days_fit(&days, (Some(20494), Some(20828))), Answer::Holds);
    // a rule that starts on 2026-03-01 misses the first payment day
    match days_fit(&days, (Some(20513), None)) {
        Answer::Fails(d) => assert_eq!(day_text(d), "2026-02-10"),
        other => panic!("{other:?}"),
    }
    let why = ritsu_base::tr!("数えられません", "not counted");
    assert!(matches!(days_fit(&Found::Undecided(why.clone()), (None, None)), Answer::Undecided(_)));
    // a value that can be the day of two dates, or come from somewhere else as well
    let two = [days.clone(), k.values(&payment_terms(), "closing").unwrap()];
    assert_eq!(days_given(&two, None, (Some(20454), Some(20828))), Answer::Holds);
    assert!(matches!(days_given(&two, None, (Some(20494), None)), Answer::Fails((1, 20473))));
    let input = ritsu_base::tr!("入力", "an input");
    assert_eq!(days_given(&two, Some(&input), (Some(20454), Some(20828))), Answer::Undecided(input.clone()));
    // an example wins over a place that says nothing
    assert!(matches!(days_given(&two, Some(&input), (Some(20494), None)), Answer::Fails((1, _))));
    assert!(matches!(days_given(&[Found::Undecided(why)], None, (None, None)), Answer::Undecided(_)));
    // the range rulec says a date input declares
    let t = ritsu_testkit::TempDir::new("x3a");
    std::fs::write(t.path().join("payment_terms.cal"), std::fs::read(payment_terms()).unwrap()).unwrap();
    std::fs::write(t.path().join("plain.rule"), "rule plain v1\n\nenum run = early | late\n\ninputs\n  pay_day : date  range >=2026-03-01 <=2027-01-31\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-06-30 | early          |\n| >=2026-07-01 | late           |\n").unwrap();
    let r = rulec::ports::Engine::new();
    let range = r.date_range(&t.path().join("plain.rule"), "pay_day").unwrap();
    assert_eq!(range, (Some(20513), Some(20849)));
    assert!(matches!(days_fit(&days, range), Answer::Fails(20494)));
}

#[test]
fn x4_a_rules_output_as_an_amount() {
    let t = ritsu_testkit::TempDir::new("x4");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("fee.rule", "rule fee v1\n\nenum size = small | large\n\ninputs\n  parcel : size\n\noutputs\n  amount : number  round down(1)\n\ntable pick\npolicy unique\n| parcel | -> amount : number |\n| small  | 300                |\n| large  | 800                |\n");
    write("adjust.rule", "rule adjust v1\n\nenum kind = charge | refund\n\ninputs\n  what : kind\n\noutputs\n  amount : number  round down(1)\n\ntable pick\npolicy unique\n| what   | -> amount : number |\n| charge | 300                |\n| refund | -300               |\n");
    let r = rulec::ports::Engine::new();
    let fee = r.output_values(&t.path().join("fee.rule"), "amount").unwrap();
    assert_eq!(amount_fits(&fee), Answer::Holds);
    let adjust = r.output_values(&t.path().join("adjust.rule"), "amount").unwrap();
    match amount_fits(&adjust) {
        Answer::Fails((v, Some(input))) => {
            assert_eq!(v, -300);
            assert_eq!(input, vec![("what".to_string(), ritsu_ports::Value::Enum("refund".into()))]);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(r.output_values(&t.path().join("fee.rule"), "parcel").unwrap(), Found::Undecided(_)));
    // by where an amount can come from: rules' outputs, ranges dandori knows, places that say nothing
    assert_eq!(amounts_given(&[fee.clone()], &[(Some(1), Some(100))], None), Answer::Holds);
    assert!(matches!(amounts_given(&[fee.clone(), adjust.clone()], &[], None), Answer::Fails((-300, AmountFrom::Output(1, Some(_))))));
    // chobo takes 0 (a transfer that moves nothing); a negative amount is the first it does not take
    assert_eq!(amounts_given(&[fee.clone()], &[(Some(0), Some(100))], None), Answer::Holds);
    assert_eq!(amounts_given(&[fee.clone()], &[(Some(-1), Some(100))], None), Answer::Fails((-1, AmountFrom::Range(0))));
    let open = ritsu_base::tr!("範囲が無い", "no range");
    assert_eq!(amounts_given(&[fee.clone()], &[], Some(&open)), Answer::Undecided(open.clone()));
    assert!(matches!(amounts_given(&[fee.clone()], &[(Some(1), None)], None), Answer::Undecided(_)));
    assert_eq!(amounts_hull(&[fee.clone()], &[(Some(1), Some(100))]), Some((1, 800)));
    assert_eq!(amounts_hull(&[adjust.clone()], &[]), Some((0, 300)));
    assert_eq!(amounts_hull(&[fee.clone()], &[(Some(1), None)]), None);
    // the refusals chobo's search finds with the amounts in the output's range, against the task
    let b = chobo::ports::Engine;
    let book = crates().join("chobo/examples/inventory/inventory.book");
    let found = b.refusals(&book, "reserve", (300, 800)).unwrap();
    let hold: Vec<String> = match &found {
        Found::Value(ops) => ops.iter().find(|(o, _)| o == "hold").map(|(_, r)| r.clone()).unwrap(),
        other => panic!("{other:?}"),
    };
    assert!(hold.contains(&"out_of_stock".to_string()), "{hold:?}");
    // the reasons of the book's bounds: what turns on the amounts
    let bounds: Vec<String> = b.facts(&book).unwrap().accounts.iter().flat_map(|a| a.lower.iter().chain(a.upper.iter()).map(|x| x.refusal.clone())).collect();
    assert_eq!(bounds, ["out_of_stock"]);
    assert_eq!(refusals_met(&found, "hold", &hold, &bounds), Answer::Holds);
    assert_eq!(refusals_met(&found, "hold", &["out_of_stock".to_string()], &bounds), Answer::Holds);
    match refusals_met(&found, "hold", &["key_conflict".to_string(), "expired".to_string()], &["out_of_stock".to_string(), "expired".to_string()]) {
        Answer::Fails(Unmet { unhandled, unfound }) => {
            assert_eq!(unhandled, ["out_of_stock"]);
            assert_eq!(unfound, ["expired"]);
        }
        other => panic!("{other:?}"),
    }
    // a range with no amount chobo takes is not decided
    assert!(matches!(b.refusals(&book, "reserve", (-5, -1)).unwrap(), Found::Undecided(_)));
    assert!(matches!(b.refusals(&book, "reserve", (-5, 0)).unwrap(), Found::Value(_)));
    assert!(matches!(refusals_met(&found, "nothing", &[], &bounds), Answer::Undecided(_)));
    // a refusal the task handles that the search finds no run for is not decided
    let mut more = hold.clone();
    more.push("too_many".to_string());
    assert!(matches!(refusals_met(&found, "hold", &more, &["out_of_stock".to_string(), "too_many".to_string()]), Answer::Undecided(_)));
}

#[test]
fn x6_a_day_given_to_a_koyomi_date() {
    let k = koyomi::ports::Engine;
    let facts = k.facts(Path::new(&payment_terms())).unwrap();
    // received runs from 2026-01-01 (20454) to 2026-12-20 (20807)
    let range = input_range(&facts, "received").unwrap();
    assert_eq!(range, (20454, 20807));
    assert_eq!(input_range(&facts, "nothing"), None);
    // the closing days are the 20th of each month: inside; the payment days run into 2027: outside
    let closing = k.values(&payment_terms(), "closing").unwrap();
    assert_eq!(days_given(&[closing], None, (Some(range.0), Some(range.1))), Answer::Holds);
    let payment = k.values(&payment_terms(), "payment").unwrap();
    match days_given(&[payment], None, (Some(range.0), Some(range.1))) {
        Answer::Fails((0, at)) => assert_eq!(day_text(at), "2027-01-10"),
        other => panic!("{other:?}"),
    }
    let now = ritsu_base::tr!("どの日にも", "any day");
    assert!(matches!(days_given(&[], Some(&now), (Some(range.0), Some(range.1))), Answer::Undecided(_)));
    // the input at which a date comes to a day, for the example
    assert_eq!(k.input_for(&payment_terms(), "payment", 20828).unwrap(), Some(vec![("received".to_string(), 20778)]));
}

#[test]
fn x5_a_hold_against_its_expiry() {
    let day = 86_400;
    let open = ritsu_base::tr!("上限が無い", "no bound");
    // it always comes before: held
    assert_eq!(held_until(3 * day, &Ok(3 * day + 60), 14 * day), Answer::Holds);
    // it always comes after (at the very second the hold has expired): the fewest is the example
    assert_eq!(held_until(17 * day + 9 * 3600, &Err(open.clone()), 14 * day), Answer::Fails(17 * day + 9 * 3600));
    assert_eq!(held_until(14 * day, &Ok(15 * day), 14 * day), Answer::Fails(14 * day));
    // either side, or nothing bounds it: undecided
    assert!(matches!(held_until(day, &Ok(20 * day), 14 * day), Answer::Undecided(_)));
    assert_eq!(held_until(day, &Err(open.clone()), 14 * day), Answer::Undecided(open));
    assert!(matches!(held_until(0, &Ok(14 * day), 14 * day), Answer::Undecided(_)));
    // koyomi's days from received to payment, with the first received day that comes to each
    let k = koyomi::ports::Engine;
    match k.span(&payment_terms(), "payment").unwrap() {
        Found::Value(s) => assert_eq!((s.fewest, s.most, s.fewest_at.map(day_text), s.most_at.map(day_text)), (18, 51, Some("2026-02-20".into()), Some("2026-07-21".into()))),
        other => panic!("{other:?}"),
    }
}
