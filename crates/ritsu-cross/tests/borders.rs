//! The decisions of X3 (a), X4 and X6 (`ritsu_cross::borders`) over what the languages hand over
//! through their ports, each to all three of its answers (held, an example, undecided). Nothing in
//! a project reaches them until dandori calls koyomi and chobo (PLAN E.5); these hold the
//! decisions to real answers of koyomi, chobo and rulec meanwhile.

use ritsu_cross::borders::{amount_fits, date_fits, day_text, days_fit, refusals_met, Unmet};
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
    assert!(matches!(days_fit(&Found::Undecided(why), (None, None)), Answer::Undecided(_)));
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
    // the refusals chobo's search finds with the amounts in the output's range, against the task
    let b = chobo::ports::Engine;
    let book = crates().join("chobo/examples/inventory/inventory.book");
    let found = b.refusals(&book, "reserve", (300, 800)).unwrap();
    let hold: Vec<String> = match &found {
        Found::Value(ops) => ops.iter().find(|(o, _)| o == "hold").map(|(_, r)| r.clone()).unwrap(),
        other => panic!("{other:?}"),
    };
    assert!(hold.contains(&"out_of_stock".to_string()), "{hold:?}");
    assert_eq!(refusals_met(&found, "hold", &hold), Answer::Holds);
    match refusals_met(&found, "hold", &["key_conflict".to_string(), "expired".to_string()]) {
        Answer::Fails(Unmet { unhandled, unfound }) => {
            assert!(unhandled.contains(&"out_of_stock".to_string()));
            assert_eq!(unfound, ["expired"]);
        }
        other => panic!("{other:?}"),
    }
    // a range with no amount chobo takes is not decided
    assert!(matches!(b.refusals(&book, "reserve", (-5, 0)).unwrap(), Found::Undecided(_)));
    assert!(matches!(refusals_met(&found, "nothing", &[]), Answer::Undecided(_)));
}

#[test]
fn x6_a_date_given_to_a_koyomi_date() {
    let facts = koyomi::ports::Engine.facts(Path::new(&payment_terms())).unwrap();
    // received runs from 2026-01-01 (20454) to 2026-12-20 (20807)
    assert_eq!(date_fits(&facts, "closing", &[("received".into(), Some((20454, 20807)))]), Answer::Holds);
    match date_fits(&facts, "payment", &[("received".into(), Some((20454, 20900)))]) {
        Answer::Fails((input, at, _)) => assert_eq!((input.as_str(), day_text(at).as_str()), ("received", "2027-03-23")),
        other => panic!("{other:?}"),
    }
    assert!(matches!(date_fits(&facts, "payment", &[("received".into(), None)]), Answer::Undecided(_)));
    assert!(matches!(date_fits(&facts, "due", &[]), Answer::Undecided(_)));
}
