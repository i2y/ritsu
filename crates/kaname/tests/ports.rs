//! kaname's ports (DESIGN 14): what it answers ritsu's checks across the borders of a module
//! (`Programs`: every call into rulec, koyomi and chobo, what kaname finds each value given can be,
//! where it comes from and how the inputs of a call relate, and what the function's vectors give
//! there), and what a module holds and names (`Items`, `References`).

mod common;

use kaname::ports::Engine;
use ritsu_ports::{At, Items, Origin, Programs, References, Value};
use ritsu_testkit::TempDir;
use std::path::Path;

/// The crossings of an example, with every language joined.
fn crossings(f: &str) -> ritsu_ports::ProgramCrossings {
    Engine.crossings(Path::new(f), &common::ports()).unwrap_or_else(|e| panic!("{f}: {e:?}"))
}

#[test]
fn the_calls_of_the_payout_and_what_they_give() {
    let x = crossings("examples/payout/payout.kn");
    assert_eq!(x.rules.len(), 1);
    let fee = &x.rules[0];
    assert_eq!((fee.at, fee.function.as_str(), fee.name.as_str()), (At { line: 19, col: 13 }, "settle", "card_fee"));
    assert!(fee.rule.ends_with("rules/card_fee.rule"), "{:?}", fee.rule);
    let amount = fee.args.iter().find(|a| a.to == "amount").unwrap();
    assert_eq!((amount.shown.as_str(), amount.range), ("payment.amount", (Some(1), Some(100_000_000))));
    assert_eq!(amount.from, [Origin::Range(Some(1), Some(100_000_000))]);
    // the two transfers: the payment's amount, and the rule's fee carried unchanged
    assert_eq!(x.transfers.len(), 2);
    let (capture, charge) = (&x.transfers[0], &x.transfers[1]);
    assert_eq!((capture.transfer.as_str(), capture.op.as_str(), capture.name.as_str()), ("capture", "do", "ledger.capture.do"));
    assert_eq!(capture.amounts[0].range, (Some(1), Some(100_000_000)));
    let given = &charge.amounts[0];
    assert_eq!((given.to.as_str(), given.shown.as_str(), given.range), ("fee", "fee", (Some(49), Some(4_990_049))));
    assert!(matches!(&given.from[..], [Origin::Output { output, .. }] if output == "fee"), "{:?}", given.from);
    assert!(x.dates.is_empty());
}

#[test]
fn the_days_and_the_dates_of_the_late_payment() {
    let x = crossings("examples/late_payment/late_payment.kn");
    // the invoice's day given to the date of koyomi, in its range
    assert_eq!(x.dates.len(), 1);
    let due = &x.dates[0];
    assert_eq!((due.name.as_str(), due.date.as_str(), due.day.to.as_str()), ("terms.due", "due", "invoiced"));
    let day = |s: &str| ritsu_ports::day_of(s).map(i128::from);
    assert_eq!(due.day.range, (day("2026-01-01"), day("2027-12-31")));
    // the days late, narrowed by the `if` to the rule's range; the rate as a count of its step
    let charges = &x.rules[0];
    let late = charges.args.iter().find(|a| a.to == "days_late").unwrap();
    assert_eq!((late.shown.as_str(), late.range), ("late", (Some(1), Some(1431))));
    let rate = charges.args.iter().find(|a| a.to == "reference_rate").unwrap();
    assert_eq!(rate.range, (Some(0), Some(1500)));
}

#[test]
fn the_running_totals_keep_how_the_inputs_relate() {
    let x = crossings("examples/discount_split/discount_split.kn");
    let call = &x.rules[0];
    for want in [("before", "<=", "upto"), ("upto", "<=", "total")] {
        assert!(call.relations.iter().any(|(a, r, b)| (a.as_str(), r.as_str(), b.as_str()) == want), "{want:?} in {:?}", call.relations);
    }
    // the Japanese version gives the same, by the rule's own names
    let x = crossings("examples/discount_split/discount_split.ja.kn");
    assert!(!x.rules[0].relations.is_empty());
}

#[test]
fn a_module_whose_names_do_not_pass_has_no_crossings() {
    let t = TempDir::new("kaname-ports-wrong");
    let f = t.write("main.kn", "module main v1\n\nfn f(n: number range >=0 <=9) -> number\n  m + 1\n");
    let said = Engine.crossings(&f, &common::ports()).unwrap_err();
    assert_eq!(said.iter().map(|s| s.code.as_str()).collect::<Vec<_>>(), ["E101"]);
}

/// What the vectors give a place the ranges do not show (DESIGN 5.7, 14.2): the payout with the
/// fee taken from the amount the seller gets, where a payment of a cent gives the seller −48.
#[test]
fn what_the_vectors_give_a_place() {
    let t = TempDir::new("kaname-ports-seen");
    ritsu_testkit::tmp::copy_dir(Path::new("examples/payout"), t.path());
    let src = std::fs::read_to_string(t.path().join("payout.kn")).unwrap();
    let src = src.replace("seller: payment.seller, amount: payment.amount),", "seller: payment.seller, amount: payment.amount - fee),");
    let src = src.split("examples").next().unwrap().to_string();
    let f = t.write("payout.kn", src);
    let x = Engine.crossings(&f, &common::ports()).unwrap();
    let amount = &x.transfers[0].amounts[0];
    assert_eq!(amount.shown, "payment.amount - fee");
    assert_eq!(amount.from, [Origin::Range(Some(-4_990_048), Some(99_999_951))]);
    let seen = Engine.seen(&f, &common::ports(), "settle");
    let here: Vec<_> = seen.iter().filter(|s| s.at == amount.at).collect();
    assert!(!here.is_empty());
    let low = here.iter().find(|s| s.given == [("amount".to_string(), Value::Int(-48))]).unwrap_or_else(|| panic!("{here:?}"));
    assert!(low.call.starts_with("settle(payment: Payment { ") && low.call.contains("amount: 1USDc"), "{}", low.call);
    let input = ritsu_base::json::parse(&low.input).unwrap();
    assert!(low.input.contains("\"amount\":1"), "{}", low.input);
    assert!(matches!(input, ritsu_base::json::Json::Obj(_)));
    // a call of the rule is seen with every input it was given
    let call = &x.rules[0];
    assert!(seen.iter().any(|s| s.at == call.at && s.given.iter().any(|(n, _)| n == "international")));
    // no function of that name, nothing seen
    assert!(Engine.seen(&f, &common::ports(), "nowhere").is_empty());
}

#[test]
fn what_a_module_holds() {
    let root = Path::new("examples/payout");
    let items = Engine.items(root, "payout.kn").unwrap();
    let names: Vec<String> = items.iter().map(|i| i.naming.text()).collect();
    assert_eq!(
        names,
        [
            "kaname \"payout.kn\" record Payment",
            "kaname \"payout.kn\" record Payment field id",
            "kaname \"payout.kn\" record Payment field seller",
            "kaname \"payout.kn\" record Payment field amount",
            "kaname \"payout.kn\" record Payment field international",
            "kaname \"payout.kn\" record Settlement",
            "kaname \"payout.kn\" record Settlement field capture",
            "kaname \"payout.kn\" record Settlement field fee",
            "kaname \"payout.kn\" function settle",
        ]
    );
    // a definition without its comments and its alignment
    let amount = &items[3];
    assert_eq!((amount.lines, amount.text.as_str()), ((10, 10), "amount : money[USDc] range >=1USDc <=100_000_000USDc"));
    let settle = items.iter().find(|i| i.naming.text().ends_with("function settle")).unwrap();
    assert_eq!(settle.lines, (17, 23));
    assert!(settle.text.starts_with("fn settle(payment: Payment) -> Settlement total\n  description "), "{}", settle.text);
    // the Japanese version names its things in Japanese
    let items = Engine.items(Path::new("examples/late_payment"), "late_payment.ja.kn").unwrap();
    assert!(items.iter().any(|i| i.naming.text() == "kaname \"late_payment.ja.kn\" function 遅れの請求"), "{:?}", items.iter().map(|i| i.naming.text()).collect::<Vec<_>>());
    // an enum whose values have fields: the enum is its lines, a value with fields its line and its
    // fields', each field its line (DESIGN 23.1)
    let items = Engine.items(Path::new("tests/modules"), "sums.kn").unwrap();
    let of_order: Vec<(String, (usize, usize))> = items.iter().filter(|i| i.naming.text().contains(" enum Order")).map(|i| (i.naming.text(), i.lines)).collect();
    assert_eq!(
        of_order,
        [
            ("kaname \"sums.kn\" enum Order".to_string(), (9, 19)),
            ("kaname \"sums.kn\" enum Order value draft".to_string(), (10, 11)),
            ("kaname \"sums.kn\" enum Order value draft field items".to_string(), (11, 11)),
            ("kaname \"sums.kn\" enum Order value placed".to_string(), (12, 14)),
            ("kaname \"sums.kn\" enum Order value placed field items".to_string(), (13, 13)),
            ("kaname \"sums.kn\" enum Order value placed field on".to_string(), (14, 14)),
            ("kaname \"sums.kn\" enum Order value paid".to_string(), (15, 17)),
            ("kaname \"sums.kn\" enum Order value paid field items".to_string(), (16, 16)),
            ("kaname \"sums.kn\" enum Order value paid field paid".to_string(), (17, 17)),
            ("kaname \"sums.kn\" enum Order value cancelled".to_string(), (18, 19)),
            ("kaname \"sums.kn\" enum Order value cancelled field reason".to_string(), (19, 19)),
        ]
    );
    let on = items.iter().find(|i| i.naming.text().ends_with("value placed field on")).unwrap();
    assert_eq!(on.text, "on : date range >=2026-01-01 <=2027-12-31");
    let reopen = items.iter().find(|i| i.naming.text().ends_with("enum Event value reopen")).unwrap();
    assert_eq!((reopen.lines, reopen.text.as_str()), ((28, 28), "reopen"));
}

#[test]
fn what_a_module_names() {
    let refs = Engine.references(Path::new("examples"), "payout/payout.kn").unwrap();
    let shown: Vec<(usize, String, String)> = refs.iter().map(|r| (r.line, r.target.text(), r.how.clone())).collect();
    assert_eq!(
        shown,
        [(4, "rulec \"payout/rules/card_fee.rule\"".to_string(), "use rule".to_string()), (5, "chobo \"payout/books/ledger.book\"".to_string(), "use book".to_string())]
    );
    let t = TempDir::new("kaname-ports-refs");
    t.write("a.kn", "module a v1\n\nfn one() -> number\n  1\n");
    t.write("glue/b.kn", "module b v1\n\nuse module a from \"../a.kn\"\n\nfn two() -> number\n  a.one() + 1\n");
    let refs = Engine.references(t.path(), "glue/b.kn").unwrap();
    assert_eq!(refs.iter().map(|r| (r.target.text(), r.how.as_str())).collect::<Vec<_>>(), [("kaname \"a.kn\"".to_string(), "use module")]);
    // a file that does not parse is said so
    t.write("c.kn", "module c\n");
    assert!(Engine.references(t.path(), "c.kn").is_err());
}
